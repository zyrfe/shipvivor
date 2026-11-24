//! The ship's own noise: machinery below, the sea along the hull, the wind over the
//! bridge and the open water around it. Unlike a cast it is made as it plays, from the
//! speed and the engine order.
use crate::{
    RATE,
    engine::Diesel,
    filter::{Biquad, Rng, pole},
};
use std::f32::consts::TAU;

/// Metres per second at which the wash along the hull is at full strength.
const FLANK: f32 = 13.6;
/// Level of the engines, and how much of each shaft's pair is heard on the other side
/// of the ship. They come up through the same hull, so neither is far to one side.
const ENGINE: f32 = 0.56;
const ACROSS: f32 = 0.38;
/// Level of their exhaust from the funnel, which stands behind the bridge and is
/// heard alike in both ears.
const STACK: f32 = 0.45;
/// Bangs a second in the exhaust of all the damaged engines together, at idle and at
/// full revolutions.
const BANGS: (f32, f32) = (0.5, 1.6);
/// Seconds for an engine to go from sound to damaged in what is heard of it.
const FAILING: f32 = 0.3;
/// Level past which the hull gives no further. Now and then every cylinder of every
/// engine pulls the same way at once, and this keeps that from being a thump.
const BRACE: f32 = 0.5;
/// Seconds for engines this size to come round to a new order, and how far round each
/// runs from the order given, as a share of the way from idle to full. Engines geared
/// to one shaft through fluid couplings are never in step. Recordings aboard show
/// each tone of a ship's engines as a close pair or cluster a percent or two wide, and
/// the slow beating inside those clusters is most of what the rumble sounds like.
const SPOOL: f32 = 2.2;
const OUT_OF_STEP: [f32; 4] = [0., 0.019, -0.012, 0.031];
/// How hard the engines work with no way to make, and how much harder while the
/// revolutions are still climbing to a new order.
const IDLING: f32 = 0.55;
const LABOUR: f32 = 1.2;
/// The fans that feed them air, heard as long as they run.
const BLOWER: f32 = 0.03;
/// The water and the wind were fitted to a recording on the deck of a ship under way,
/// where half the sound lies below 200 Hz and nine tenths below 500. The wash is a low
/// roar centred here in hertz when stopped and this much higher at `FLANK`, the spray
/// thrown aside is a small part of it, and the highs fall away steadily above the first
/// of `VEIL` until almost nothing is left above the second.
const WASH: f32 = 0.9;
const ROAR: (f32, f32) = (220., 120.);
const FIZZ: f32 = 0.05;
const SEA: f32 = 0.2;
const VEIL: (f32, f32) = (1000., 3200.);
/// Wind over the bridge when the ship makes `FLANK` into the breeze, and the breeze
/// in metres per second that blows when she lies stopped.
const WIND: f32 = 0.2;
const BREEZE: f32 = 4.;
/// Swells met each second when stopped and how many more at `FLANK`, how much the
/// wash lifts as the bow goes into one, and the weight of the blow.
const SWELL: (f32, f32) = (0.16, 0.12);
const PLUNGE: f32 = 0.5;
const SLAM: f32 = 0.2;

pub(crate) struct Hull {
    rng: Rng,
    /// Nothing is heard of the ship until the game first says where it stands.
    afloat: bool,
    /// What the game last asked for.
    speed: f32,
    throttle: f32,
    lit: f32,
    /// How many of the engines are damaged, and how many knocked out.
    rough: usize,
    stopped: usize,
    /// What the machinery and the water have come round to.
    way: f32,
    revs: f32,
    steam: [f32; 4],
    fault: [f32; 4],
    engines: [Diesel; 4],
    surge: [f32; 2],
    swell: f32,
    pace: f32,
    gust: [f32; 2],
    slam: Biquad,
    wind: [Biquad; 2],
    rigging: [Biquad; 2],
    blower: Biquad,
    wash: [Biquad; 2],
    veil: [Biquad; 2],
    soft: [f32; 2],
    fizz: [Biquad; 2],
    sea: [Biquad; 2],
}
impl Hull {
    pub fn new() -> Self {
        Self {
            rng: Rng::new(0x5EA),
            afloat: false,
            speed: 0.,
            throttle: 0.,
            lit: 0.,
            rough: 0,
            stopped: 0,
            way: 0.,
            revs: 0.,
            steam: [0.; 4],
            fault: [0.; 4],
            engines: [0xD1E5, 0x5E1, 0xB0A7, 0xC0DE].map(Diesel::new),
            surge: [0.; 2],
            swell: 0.,
            pace: 0.,
            gust: [0.; 2],
            slam: Biquad::low_pass(85., 0.7),
            wind: [Biquad::band_pass(180., 0.5); 2],
            rigging: [Biquad::band_pass(1000., 2.5); 2],
            blower: Biquad::band_pass(430., 0.5),
            wash: [Biquad::band_pass(ROAR.0, 0.5); 2],
            veil: [Biquad::low_pass(VEIL.1, 0.7); 2],
            soft: [0.; 2],
            fizz: [Biquad::band_pass(2200., 0.8); 2],
            sea: [Biquad::band_pass(240., 0.5); 2],
        }
    }

    /// Under way: speed through the water in metres per second, and the engine order
    /// as a share of full ahead.
    pub fn way(&mut self, speed: f32, throttle: f32) {
        (self.speed, self.throttle) = (speed.abs(), throttle.abs().min(1.));
        (self.lit, self.afloat) = (1., true);
    }

    /// The state of the engines, as shares of however many the ship has: those
    /// damaged and still running, and those knocked out. The four heard here stand
    /// for all of them.
    pub fn plant(&mut self, rough: f32, stopped: f32) {
        // Any at all is heard, and nothing short of all of them is silence.
        let some = |share: f32, most: usize| match share {
            share if share <= 0. => 0,
            share if share >= 1. => most,
            share => ((4. * share).round() as usize).clamp(1, 3).min(most),
        };
        self.stopped = some(stopped, 4);
        self.rough = some(rough, 4 - self.stopped);
    }

    /// Machinery shut down. Only the sea is left.
    pub fn moor(&mut self) {
        (self.speed, self.throttle, self.lit, self.afloat) = (0., 0., 0., true);
    }

    /// Write the next frames of the ship's noise.
    pub fn sing(&mut self, out: &mut [[f32; 2]]) {
        if !self.afloat {
            return out.fill([0.; 2]);
        }
        // Engines take a moment to answer the telegraph. The wash follows the ship.
        let frames = out.len() as f32;
        let toward = |secs: f32| 1. - (-frames / (secs * RATE)).exp();
        // They work as the square of their revolutions against the water, and harder
        // still while they are being brought up to an order they have not reached.
        let climbing = (self.throttle - self.revs).max(0.);
        self.revs += (self.throttle - self.revs) * toward(SPOOL);
        self.way += ((self.speed / FLANK).min(1.3) - self.way) * toward(0.4);
        // The last engines are the first knocked out, and the first the first damaged.
        for (n, (steam, fault)) in self.steam.iter_mut().zip(&mut self.fault).enumerate() {
            let lit = if n + self.stopped < 4 { self.lit } else { 0. };
            *steam += (lit - *steam) * toward(0.9);
            *fault += (f32::from(n < self.rough) - *fault) * toward(FAILING);
        }
        let (revs, way) = (self.revs, self.way);
        let load = IDLING + (1. - IDLING) * revs * revs + LABOUR * climbing;
        let bangs = (BANGS.0 + (BANGS.1 - BANGS.0) * revs) / self.rough.max(1) as f32;
        let (steam, fault) = (self.steam, self.fault);
        // More speed brightens the wash.
        for wash in &mut self.wash {
            wash.retune(Biquad::band_pass(ROAR.0 + ROAR.1 * way, 0.5));
        }
        let (wash, fizz) = (WASH * way.powf(1.5), FIZZ * way * way);
        let (heave, soften) = (pole(0.6), pole(VEIL.0));
        // The wind is the breeze and the ship's own speed together. It grows with the
        // square of both, and sings higher in the rigging as it rises.
        let blow = (BREEZE / FLANK + way) / (BREEZE / FLANK + 1.);
        let wind = WIND * blow * blow;
        for rigging in &mut self.rigging {
            rigging.retune(Biquad::band_pass(700. + 700. * blow, 2.5));
        }
        let (squall, shift) = (pole(0.35), pole(0.2));
        for frame in out {
            // The bow goes into each swell with a blow and throws the sea aside. Swells
            // do not come evenly, and they come sooner at speed.
            self.pace += shift * (self.rng.noise() - self.pace);
            let met = (SWELL.0 + SWELL.1 * way) * (1. + 12. * self.pace).clamp(0.5, 1.6);
            self.swell = (self.swell + met / RATE).fract();
            let crest = (TAU * self.swell).sin().max(0.).powi(4);
            let plunge = PLUNGE * crest * way;
            let slam = SLAM * way * way * crest * crest * self.slam.run(self.rng.noise());
            let (mut each, mut funnel) = ([0.; 4], 0.);
            for (n, (engine, off)) in self.engines.iter_mut().zip(OUT_OF_STEP).enumerate() {
                // Shut down, it runs down as well as dying away.
                let turning = (revs + off) * steam[n] - (1. - steam[n]);
                let (through, aloft) = engine.run(turning, load, fault[n], bangs);
                each[n] = steam[n] * through;
                funnel += steam[n] * aloft;
            }
            let (port, starboard) = (each[0] + each[2], each[1] + each[3]);
            let running = 0.25 * steam.iter().sum::<f32>();
            let fans = BLOWER * running * self.blower.run(self.rng.noise());
            let machinery = [
                port + ACROSS * starboard + fans,
                starboard + ACROSS * port + fans,
            ];
            let funnel = STACK * funnel;
            for side in 0..2 {
                // Each side of the ship meets its own water, in slow uneven surges.
                self.surge[side] += heave * (self.rng.noise() - self.surge[side]);
                let lift = (1. + self.surge[side] * 110.).clamp(0.45, 1.8);
                let spray = self.rng.noise();
                self.gust[side] += squall * (self.rng.noise() - self.gust[side]);
                let gust = wind * (1. + self.gust[side] * 160.).clamp(0.3, 2.2);
                let air = self.rng.noise();
                let outside = (lift + plunge) * wash * self.wash[side].run(spray)
                    + (1. + 2. * plunge) * fizz * self.fizz[side].run(spray)
                    + lift * SEA * self.sea[side].run(self.rng.noise())
                    + gust * (self.wind[side].run(air) + 0.06 * self.rigging[side].run(air));
                self.soft[side] += soften * (self.veil[side].run(outside) - self.soft[side]);
                frame[side] = BRACE * (ENGINE * machinery[side] / BRACE).tanh()
                    + funnel
                    + slam
                    + self.soft[side];
            }
        }
    }
}
