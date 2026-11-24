//! A warship's diesel, as it reaches the bridge through the ship.
//!
//! The engine is the kind the diesel cruisers of the 1930s carried: nine cylinders in
//! line, two-stroke and double-acting, so each cylinder fires above its piston and
//! again below it on every turn of the crank. That is eighteen firings a turn, at up to
//! 450 turns a minute.
//!
//! Nothing here is an oscillator. Each firing is a blow, and what is heard is what the
//! blows add up to. Were all eighteen alike it would be one steady tone at the firing
//! rate. They are not: the chamber under a piston is weaker than the one above it, no
//! two cylinders are quite matched, and each sits at its own place along the block.
//! What comes of it is a set of steady low tones one crank-turn apart, which is what a
//! ship's engine measures as from a cabin or the deck. The way from the engine room to
//! the bridge passes little above 160 Hz, and its shape here was fitted to recordings
//! made aboard ships under way.
//!
//! The exhaust takes another way: up the funnel and down to the bridge through the
//! open air, which keeps the middle of the sound that the hull loses. A slow diesel
//! recorded in the open is only some 8 dB weaker between 300 Hz and 1 kHz than at its
//! firing tones, and that part is what the ear takes for the engine.
//!
//! A damaged engine has cylinders that no longer fire. The turn of the crank goes
//! lame, the revolutions sag and stumble, and fuel that went through unburnt lights in
//! the exhaust trunk and goes off up the funnel. Recordings of engines doing this show
//! bangs that stand 10 to 20 dB over the running engine, are over in a hundredth of a
//! second, and come in twos and threes with seconds of nothing between.
use crate::{
    RATE,
    filter::{Biquad, Rng, pole},
};
use std::f32::consts::PI;

const CYLINDERS: usize = 9;
/// Two chambers to a cylinder.
const CHAMBERS: usize = 2 * CYLINDERS;
/// Turns of the crank a second at idle and at full revolutions: 180 and 450 a minute.
const TURNS: (f32, f32) = (3., 7.5);
/// The cylinders in the order they fire, counted along the block. Each fires below
/// its piston half a turn after it fires above.
const ORDER: [usize; CYLINDERS] = [0, 5, 6, 2, 8, 1, 3, 7, 4];
/// The piston rod takes up part of the lower chamber, which hits this much as hard.
const BELOW: f32 = 0.8;
/// How far one chamber's blow may differ from another's, and how far one firing's
/// timing and weight may differ from the last. An engine under a governor is steady.
const UNEVEN: f32 = 0.5;
const RAGGED: (f32, f32) = (0.004, 0.05);
/// How quickly the governor's hunting wanders, as the share of the way it moves towards
/// a new random value each sample, and how far that carries the revolutions.
const HUNT: (f32, f32) = (2e-5, 0.6);
/// Seconds for the pressure of one firing to fall away.
const STROKE: f32 = 0.004;
/// The ways from the engine to the bridge: hertz, sharpness and level. Each chamber
/// drives each of them differently, by where along the block it sits.
const WAYS: usize = 5;
const PATH: [(f32, f32, f32); WAYS] = [
    (46., 1.6, 0.5),
    (68., 1.8, 0.9),
    (92., 2., 1.),
    (128., 2., 0.9),
    (165., 2.2, 0.6),
];
/// Share of every way that all chambers drive alike.
const ALIKE: f32 = 0.15;
/// Above and below these the ship passes almost nothing of the engine.
const PASSED: (f32, f32) = (48., 210.);
/// Exhaust let out at each firing, as noise with no pitch of its own: its level against
/// the blows, the hertz the stack booms at, and the seconds a puff takes to clear it.
const EXHAUST: (f32, f32, f32) = (1.5, 110., 0.012);
/// The same puffs as the funnel lets them out into the open: nothing below the first
/// of these in hertz, falling away above the second and faster above the third, and
/// how much faster than the blows themselves they grow as the engine is worked harder.
const FUNNEL: (f32, f32, f32, f32) = (200., 400., 1600., 0.5);

/// Cylinders a damaged engine has lost, counted along the block from a place of its
/// own, and how often one of the others misses as well.
const LOST: usize = 2;
const MISSES: f32 = 0.12;
/// What a chamber that does not fire still gives: the air it squeezed, let go again.
const SQUEEZE: f32 = 0.08;
/// How much further one firing of a damaged engine may differ from the last, and how
/// many times wider its governor hunts.
const ROUGH: (f32, f32) = (0.3, 4.);
/// Share of its revolutions a damaged engine loses at each miss and at each bang in
/// its exhaust, and the hertz at which the governor gets them back.
const STUMBLE: (f32, f32, f32) = (0.008, 0.03, 1.5);
/// Fuel goes off in the trunk in fits. This is how quickly the fits come and go, as
/// the share of the way they move towards a new random value each firing, the seconds
/// after one bang in which another is likely, and how many times as likely.
const FITS: (f32, f32, f32) = (0.008, 0.07, 12.);
/// A bang in the exhaust: its level against a firing, the seconds it takes to die
/// away, and how much of it is one push of pressure, the rest being the roar of the
/// flame. The push is what makes it a bang and not a hiss. It is nearly as loud from
/// an engine that is idling as from one worked hard, and the last number is the power
/// of the load by which it grows.
const BANG: (f32, f32, f32, f32) = (7., 0.009, 0.5, 0.3);
/// What the trunk and the funnel do to it: the two frequencies above which it falls
/// off, and the hertz below which none of it gets out.
const VENT: (f32, f32, f32) = (250., 900., 90.);
/// How hard it shakes the block.
const JOLT: f32 = 3.;

pub(crate) struct Diesel {
    rng: Rng,
    /// How hard each chamber drives each way, in the order they fire, and the chambers
    /// that stop firing when the engine is damaged.
    drive: [[f32; WAYS]; CHAMBERS],
    lost: [bool; CHAMBERS],
    /// Share of the way to the next firing, the chamber that fired last and how much
    /// early or late the next one runs.
    turn: f32,
    chamber: usize,
    gait: f32,
    hunt: f32,
    press: [f32; WAYS],
    puff: f32,
    /// Share of each that is left a sample later, and of the bang.
    left: (f32, f32, f32),
    /// The governor's pull, the funnel's two losses of highs and the bang's two.
    poles: [f32; 5],
    ways: [Biquad; WAYS],
    passed: [Biquad; 3],
    stack: Biquad,
    /// The funnel's own filters: what it loses of the highs, and of the lows.
    open: [f32; 2],
    mouth: Biquad,
    /// Fuel lying unburnt in the trunk, how far the present fit of bangs has come on,
    /// the seconds for which another bang is likely, and the revolutions lost.
    unburnt: f32,
    fit: f32,
    hot: f32,
    stagger: f32,
    /// The bang now dying away, and its filters.
    bang: f32,
    dull: [f32; 2],
    vent: Biquad,
}
impl Diesel {
    /// Engines with different seeds are different engines of the same make.
    pub fn new(seed: u32) -> Self {
        let mut rng = Rng::new(seed);
        // Firings alternate: above one piston, then below the piston half a turn round.
        let mut drive = [[0.; WAYS]; CHAMBERS];
        let mut lost = [false; CHAMBERS];
        // A hit takes cylinders that stand together.
        let hit = (rng.unit() * (CYLINDERS - LOST + 1) as f32) as usize;
        for (firing, drive) in drive.iter_mut().enumerate() {
            let below = firing % 2;
            let place = ORDER[(firing / 2 + below * (CYLINDERS / 2 + 1)) % CYLINDERS];
            lost[firing] = (hit..hit + LOST).contains(&place);
            let along = (place as f32 + 0.5) / CYLINDERS as f32;
            let trim = [1., BELOW][below] * (1. + UNEVEN * rng.noise());
            // The block bends in ever shorter waves, and a chamber drives each one by
            // how far it sits from that wave's still points.
            for (wave, (drive, (_, _, level))) in drive.iter_mut().zip(PATH).enumerate() {
                let shape = (PI * (wave + 1) as f32 * along).cos();
                *drive = trim * level * (ALIKE + (1. - ALIKE) * shape);
            }
        }
        Self {
            drive,
            lost,
            turn: rng.unit(),
            chamber: 0,
            gait: 1.,
            hunt: 0.,
            press: [0.; WAYS],
            puff: 0.,
            left: (
                (-1. / (STROKE * RATE)).exp(),
                (-1. / (EXHAUST.2 * RATE)).exp(),
                (-1. / (BANG.1 * RATE)).exp(),
            ),
            poles: [STUMBLE.2, FUNNEL.1, FUNNEL.2, VENT.0, VENT.1].map(pole),
            ways: PATH.map(|(hz, q, _)| Biquad::band_pass(hz, q)),
            passed: [
                Biquad::high_pass(PASSED.0, 0.7),
                Biquad::low_pass(PASSED.1, 0.6),
                Biquad::low_pass(3. * PASSED.1, 0.7),
            ],
            stack: Biquad::band_pass(EXHAUST.1, 0.7),
            open: [0.; 2],
            mouth: Biquad::high_pass(FUNNEL.0, 0.7),
            unburnt: 0.,
            fit: 0.,
            hot: 0.,
            stagger: 0.,
            bang: 0.,
            dull: [0.; 2],
            vent: Biquad::high_pass(VENT.2, 0.7),
            rng,
        }
    }

    /// The next sample: what comes through the hull, and what comes from the funnel
    /// through the open air. `revs` runs from 0 at idle to 1 at full revolutions, and
    /// below 0 for an engine running down. `load` is how hard it is being made to work.
    /// `fault` is 0 for a sound engine and 1 for a damaged one, which lets off `bangs`
    /// in its exhaust each second, taken over a long while.
    pub fn run(&mut self, revs: f32, load: f32, fault: f32, bangs: f32) -> (f32, f32) {
        self.hunt += HUNT.0 * (self.rng.noise() - self.hunt);
        self.stagger -= self.stagger * self.poles[0];
        self.hot -= 1. / RATE;
        // Below idle the engine is running down, and at nothing it has stopped.
        let turns = (TURNS.0 + (TURNS.1 - TURNS.0) * revs).max(0.);
        let hunting = HUNT.1 * (1. + ROUGH.1 * fault) * self.hunt;
        let rate = CHAMBERS as f32 * turns * (1. + hunting) * (1. - self.stagger);
        self.turn += rate * self.gait / RATE;
        if self.turn >= 1. {
            self.turn -= 1.;
            self.chamber = (self.chamber + 1) % CHAMBERS;
            self.gait = 1. + RAGGED.0 * self.rng.noise();
            let missed =
                fault > 0. && (self.lost[self.chamber] || self.rng.unit() < MISSES * fault);
            let blow = if missed {
                self.unburnt += fault;
                self.stagger += STUMBLE.0 * fault;
                SQUEEZE * load
            } else {
                load * (1. + (RAGGED.1 + ROUGH.0 * fault) * self.rng.noise())
            };
            for (press, drive) in self.press.iter_mut().zip(self.drive[self.chamber]) {
                *press += blow * drive;
            }
            self.puff += blow * load.powf(FUNNEL.3);
            if fault > 0. {
                self.backfire(rate, load, bangs);
            }
        }
        let mut heard = 0.;
        for (press, way) in self.press.iter_mut().zip(&mut self.ways) {
            *press *= self.left.0;
            heard += way.run(*press);
        }
        self.puff *= self.left.1;
        let gas = self.puff * self.rng.noise();
        heard += EXHAUST.0 * self.stack.run(gas);
        for pass in &mut self.passed {
            heard = pass.run(heard);
        }
        self.open[0] += self.poles[1] * (gas - self.open[0]);
        self.open[1] += self.poles[2] * (self.open[0] - self.open[1]);
        let mut aloft = self.mouth.run(self.open[1]);
        if self.bang > 1e-5 {
            self.bang *= self.left.2;
            let flash = self.bang * (BANG.2 + (1. - BANG.2) * self.rng.noise());
            self.dull[0] += self.poles[3] * (flash - self.dull[0]);
            self.dull[1] += self.poles[4] * (self.dull[0] - self.dull[1]);
            aloft += BANG.0 * self.vent.run(self.dull[1]);
        }
        (heard, aloft)
    }

    /// Whether the fuel lying in the trunk lights at this firing. `rate` is firings a
    /// second.
    fn backfire(&mut self, rate: f32, load: f32, bangs: f32) {
        // Slowed that much, noise that filled -1..1 keeps this much of its spread.
        let spread = (FITS.0 / (3. * (2. - FITS.0))).sqrt();
        self.fit += FITS.0 * (self.rng.noise() - self.fit);
        let fits = (1. + self.fit / spread).clamp(0., 3.);
        let chain = if self.hot > 0. { FITS.2 } else { 1. };
        if self.unburnt >= 1. && self.rng.unit() * rate.max(1.) < bangs * fits * chain {
            let weight = self.rng.within(0.4, 1.) * (self.unburnt / 3.).min(1.).sqrt();
            self.bang += weight * load.powf(BANG.3);
            // It goes off against the pistons too, and the whole block feels it.
            for (press, (_, _, level)) in self.press.iter_mut().zip(PATH) {
                *press += JOLT * weight * load * level * self.rng.noise();
            }
            self.puff += weight * load;
            self.stagger += STUMBLE.1 * weight;
            (self.unburnt, self.hot) = (0., FITS.1);
        } else {
            // What does not light is carried out a little at each firing.
            self.unburnt *= 0.9;
        }
    }
}
