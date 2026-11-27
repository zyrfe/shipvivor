//! Casts the sound bank at startup.
//!
//! These are synthesized stand-ins with the anatomy of the real thing: a gun is a crack,
//! a body and a roll. A recorded layer can replace any of them without touching the mixer.
use crate::{
    RATE, Sound,
    filter::{Biquad, Rng, pole},
};
use std::{
    f32::consts::{PI, TAU},
    sync::{Arc, OnceLock},
};

pub(crate) const VARIATIONS: usize = 5;

/// One take of a sound: detail, body and rumble, each at unit peak.
pub(crate) struct Cast {
    pub layers: [Vec<f32>; 3],
    pub longest: usize,
    /// How loud each layer is, or is about to be, through each [`STRETCH`] of it.
    pub envelope: [Vec<f32>; 3],
}

/// Samples in each stretch of a cast's envelope, and the stretches after each that it
/// looks ahead to. A sound that starts a beat late, or swells, is as loud as it will
/// shortly be.
pub(crate) const STRETCH: usize = 512;
const LOOK: usize = 8;

/// The envelope of a layer: in each stretch, the most power in it or in the few after
/// it, as a root mean square.
fn envelope(layer: &[f32]) -> Vec<f32> {
    let power: Vec<f32> = (layer.chunks(STRETCH))
        .map(|stretch| (stretch.iter().map(|x| x * x).sum::<f32>() / stretch.len() as f32).sqrt())
        .collect();
    (0..power.len())
        .map(|n| {
            power[n..(n + LOOK).min(power.len())]
                .iter()
                .fold(0_f32, |most, &x| most.max(x))
        })
        .collect()
}

pub struct Bank {
    casts: Vec<Cast>,
}
impl Bank {
    /// The game's bank, cast on first use and shared after that.
    pub fn standard() -> Arc<Self> {
        static BANK: OnceLock<Arc<Bank>> = OnceLock::new();
        BANK.get_or_init(|| {
            let casts = Sound::ALL
                .iter()
                .flat_map(|&sound| (0..VARIATIONS).map(move |take| Cast::of(sound, take)))
                .collect();
            Arc::new(Self { casts })
        })
        .clone()
    }

    pub(crate) fn index(sound: Sound, serial: u64) -> usize {
        sound as usize * VARIATIONS + (serial % VARIATIONS as u64) as usize
    }

    pub(crate) fn cast(&self, index: usize) -> &Cast {
        &self.casts[index]
    }
}

impl Cast {
    fn of(sound: Sound, take: usize) -> Self {
        let rng = &mut Rng::new(Bank::index(sound, take as u64) as u32 + 1);
        let gun = |push, roar, roll| Gun { push, roar, roll };
        let layers = match sound {
            Sound::MainGun => gun(0.009, 0.42, 3.4).cast(rng),
            Sound::SecondaryGun => gun(0.0035, 0.16, 1.6).cast(rng),
            Sound::RaiderGun => gun(0.005, 0.22, 2.).cast(rng),
            // A shore gun pushes twice as long as her own main battery: any longer and
            // its blast would ring like a struck drum.
            Sound::ShoreGun => gun(0.017, 0.65, 4.6).cast(rng),
            Sound::Splash => splash(rng, 1.),
            Sound::Burst => burst(rng),
            Sound::Trumpet => trumpet(),
            Sound::Wreck => wreck(rng),
            Sound::ArmorRing => plate_strike(rng, true),
            Sound::ArmorKnock => plate_strike(rng, false),
            Sound::Crack => crack(rng),
            Sound::Recoil => recoil(rng, 1.),
            Sound::Kick => recoil(rng, 0.55),
            Sound::Collapse => Fall::BUILDING.cast(rng),
            Sound::Ruin => Fall::YARD.cast(rng),
            Sound::Fanfare => fanfare(rng),
            Sound::Breaker => breaker(rng),
            Sound::IceCrack => ice_crack(rng),
            Sound::Calving => calving(rng),
            Sound::Gull => gull(rng),
            Sound::Rivets => rivets(rng),
            Sound::Creak => creak(rng),
            Sound::Telegraph => telegraph(rng),
            Sound::Upgrade => upgrade(),
            Sound::UnrollChart => chart_paper(rng, true),
            Sound::RollChart => chart_paper(rng, false),
            Sound::Klaxon => klaxon(rng),
            Sound::Chime => chime(),
            // A heavy gun pushes half as long again as the light shore guns.
            Sound::HeavyGun => gun(0.034, 0.9, 7.).cast(rng),
            Sound::Column => splash(rng, 1.8),
            Sound::TorpedoLaunch => torpedo_launch(rng),
            Sound::TorpedoAlarm => torpedo_alarm(rng),
            Sound::Torpedoed => torpedoed(rng),
            Sound::Detent => detent(rng),
            Sound::EngineHurt => engine_hurt(rng),
            Sound::EngineStall => engine_stall(rng),
            Sound::Hammer => hammer(rng),
            Sound::Torch => torch(rng),
            Sound::Ratchet => ratchet(rng),
            Sound::Tick => tick(rng, take % 2 == 1),
            Sound::Tap => tap(rng),
            // A clank is the burst every hit on her made before plate and calibre were
            // heard. It was liked, so it is kept as it was.
            Sound::Clank => burst(rng),
            Sound::Pierce => pierce(rng),
            Sound::Blast => inner_blast(rng),
            Sound::Gutted => gutted(rng),
            Sound::Shudder => shudder(rng),
        }
        .map(finish);
        let longest = layers.iter().map(Vec::len).max().unwrap_or(0);
        let envelope = layers.each_ref().map(|layer| envelope(layer));
        Self {
            layers,
            longest,
            envelope,
        }
    }
}

fn render(secs: f32, mut voice: impl FnMut(f32) -> f32) -> Vec<f32> {
    (0..(secs * RATE) as usize)
        .map(|n| voice(n as f32 / RATE))
        .collect()
}

/// The hiss of stiff paper sliding against itself, with a flutter as each turn
/// comes free. The narrowing roll brightens into a short flick as it closes.
fn chart_paper(rng: &mut Rng, opening: bool) -> [Vec<f32>; 3] {
    let secs = if opening {
        crate::CHART_UNROLL_SECONDS
    } else {
        crate::CHART_ROLL_SECONDS
    };
    let count = (secs * RATE) as usize;
    let mut layers = [(); 3].map(|_| vec![0.; count]);
    let mut fibre = Biquad::band_pass(3400., 0.85);
    let mut fold = Biquad::band_pass(620., 0.95);
    let mut phase = rng.unit();
    for n in 0..count {
        let t = n as f32 / count as f32;
        let tight = if opening { 1. - t } else { t };
        phase += (16. + 24. * tight) / RATE;
        let flutter = 0.55 + 0.45 * (TAU * phase).cos();
        let sweep = (PI * t).sin().powf(if opening { 0.85 } else { 1.5 });
        let flick = ((t - 0.84) / 0.055).powi(2);
        let grain = rng.noise();
        layers[0][n] = fibre.run(grain) * (sweep * flutter * (0.3 + tight) + (-flick).exp() * 0.7);
        layers[1][n] = fold.run(grain) * sweep * (0.45 + flutter) * (1.1 - 0.55 * tight);
    }
    layers
}

/// Scale to a unit peak and close the last 20 ms to silence.
fn finish(mut layer: Vec<f32>) -> Vec<f32> {
    let peak = layer.iter().fold(1e-9_f32, |peak, x| peak.max(x.abs()));
    let (len, fade) = (layer.len(), 0.02 * RATE);
    for (n, x) in layer.iter_mut().enumerate() {
        *x *= ((len - n) as f32 / fade).min(1.) / peak;
    }
    layer
}

/// Pressure of a blast wave passing a point: an instant rise, a fall through zero after
/// `push` seconds and a shallow suction behind. It carries weight without any pitch.
fn blast(t: f32, push: f32) -> f32 {
    if t < 0. {
        return 0.;
    }
    let x = t / push;
    (1. - x) * (-x).exp()
}

/// A blast and its returns off nearby surfaces. Each return is weaker, later and
/// more smeared than the wave that made it.
struct Shock {
    push: f32,
    returns: Vec<(f32, f32, f32)>,
}
impl Shock {
    fn new(push: f32, count: usize, span: (f32, f32), rng: &mut Rng) -> Self {
        let returns = (0..count)
            .map(|_| {
                let at = span.0 + (span.1 - span.0) * rng.unit().powf(1.5);
                let side = if rng.unit() < 0.5 { -1. } else { 1. };
                let gain = side * 0.75 * (-2. * at / span.1).exp();
                (at, gain, push * rng.within(1., 2.))
            })
            .collect();
        Self { push, returns }
    }
    fn pressure(&self, t: f32) -> f32 {
        let returned: f32 = self
            .returns
            .iter()
            .map(|&(at, gain, push)| gain * blast(t - at, push))
            .sum();
        blast(t, self.push) + returned
    }
}

/// The body of an explosion: the pressure wave, then burning gas roaring and tearing
/// as it dies away over `roar` seconds. Driven hard enough to press against full scale.
fn boom(push: f32, roar: f32, rng: &mut Rng) -> Vec<f32> {
    // Returns come off surfaces a few metres away, and a longer wave needs more room
    // between them or they run together into a wobble.
    let shock = Shock::new(
        push,
        7,
        (0.006_f32.max(push * 0.7), 0.09_f32.max(push * 10.)),
        rng,
    );
    let mut band = [Biquad::high_pass(70., 0.7), Biquad::low_pass(1400., 0.6)];
    let mut deep = Biquad::low_pass(130., 0.7);
    let (mut flutter, churn) = (0_f32, pole(45.));
    render(roar * 6., |t| {
        flutter += churn * (rng.noise() - flutter);
        let tear = 1. + (flutter * 20.).clamp(-0.8, 2.);
        let gas = band[0].run(rng.noise());
        let gas = band[1].run(gas);
        let fall = 0.6 * (-t / (roar * 0.25)).exp() + 0.4 * (-t / roar).exp();
        let weight = 6. * deep.run(rng.noise()) * (-t / (roar * 0.5)).exp();
        let onset = (t / 0.002).min(1.);
        (2.5 * (shock.pressure(t) + onset * (2.2 * gas * tear * fall + weight))).tanh()
    })
}

/// Low noise that swells, rolls unevenly and dies away.
fn rumble(secs: f32, hz: f32, rng: &mut Rng) -> Vec<f32> {
    let mut low = [Biquad::low_pass(hz, 0.6), Biquad::low_pass(hz * 0.6, 0.6)];
    let (mut swell, sway) = (0_f32, pole(4.));
    render(secs, |t| {
        swell += sway * (rng.noise() - swell);
        let roll = (1. + swell * 40.).clamp(0.3, 1.9);
        let swell_in = 1. - (-t / 0.05).exp();
        let dark = low[0].run(rng.noise());
        low[1].run(dark) * roll * swell_in * (-t * 5. / secs).exp()
    })
}

/// Add short events to a layer at random moments. `weigh` sets how loud an event at a
/// given time may be, and `grain` gives each one its sample `n` samples in.
fn scatter(
    layer: &mut [f32],
    count: usize,
    span: (f32, f32),
    ring: (f32, f32),
    rng: &mut Rng,
    weigh: impl Fn(f32) -> f32,
    mut grain: impl FnMut(&mut Rng, f32, f32) -> f32,
) {
    for _ in 0..count {
        let at = rng.within(span.0, span.1);
        let weight = weigh(at) * rng.within(0.3, 1.);
        let ring = rng.within(ring.0, ring.1);
        let colour = rng.unit();
        let start = ((at * RATE) as usize).min(layer.len());
        for (n, x) in layer[start..]
            .iter_mut()
            .take((ring * 6. * RATE) as usize)
            .enumerate()
        {
            let t = n as f32 / RATE;
            *x += weight * grain(rng, colour, t) * (-t / ring).exp();
        }
    }
}

struct Gun {
    /// Seconds the muzzle blast pushes before its pressure falls through zero.
    /// A bigger gun pushes for longer, which is what makes it sound heavy.
    push: f32,
    /// Seconds for the roar of the propellant gas to die away.
    roar: f32,
    /// Length of the roll across the water in seconds.
    roll: f32,
}
impl Gun {
    fn cast(self, rng: &mut Rng) -> [Vec<f32>; 3] {
        let push = self.push * rng.within(0.9, 1.1);
        // The crack is the shock front alone, slapping back off the deck and the sea.
        let front = Shock::new(push * 0.12, 3, (0.002, 0.02), rng);
        let mut hiss = Biquad::high_pass(1500., 0.7);
        let crack = render(0.14, |t| {
            0.9 * front.pressure(t) + hiss.run(rng.noise()) * (-t / (push * 1.3)).exp()
        });
        let body = boom(push, self.roar, rng);
        // The report comes back across the water in uneven surges.
        let mut roll = rumble(self.roll, 320., rng);
        let surges = [(); 4].map(|_| {
            (
                rng.within(0.2, self.roll * 0.5),
                rng.within(0.05, 0.14),
                rng.within(0.5, 1.4),
            )
        });
        for (n, x) in roll.iter_mut().enumerate() {
            let t = n as f32 / RATE;
            let lift: f32 = surges
                .iter()
                .map(|(at, width, lift)| lift * (-((t - at) / width).powi(2)).exp())
                .sum();
            *x *= 1. + lift;
        }
        [crack, body, roll]
    }
}

/// A shell landing in the sea. Recordings of big splashes are broad, with as much in
/// each third of an octave from 100 Hz to 5 kHz and half their power below 650 Hz.
/// The blow and the column going up are loud for half a second and low in pitch. What
/// went up then comes down, 8 dB quieter and far brighter, for a second and more.
///
/// `heft` is 1 for a shell of a cruiser's size. A heavier shell throws its column up for
/// longer and lower in pitch, and the water is longer coming down.
fn splash(rng: &mut Rng, heft: f32) -> [Vec<f32>; 3] {
    let secs = 2.3 * heft;
    let up = rng.within(0.28, 0.4) * heft;
    let down = rng.within(0.55, 0.75) * heft;
    let (pour, settle) = (0.3 * heft, 0.45 * heft);
    // How much water is going up, and how much of it is coming back down.
    let column = |t: f32| (t / 0.02).min(1.) * (-t / up).exp();
    let rain =
        |t: f32| ((t - down) / pour).clamp(0., 1.) * (-(t - down - pour).max(0.) / settle).exp();
    // Spray hisses as the column rises, then droplets patter back as it collapses.
    let mut hiss = Biquad::high_pass(3000., 0.7);
    let mut spray = render(secs, |t| {
        hiss.run(rng.noise()) * (0.3 * (t / 0.15).min(1.) * column(t) + 0.2 * rain(t))
    });
    scatter(
        &mut spray,
        (170. * heft) as usize,
        (0.1, 2. * heft),
        (0.003, 0.009),
        rng,
        |at| 0.5 * column(at) + rain(at),
        |_, colour, t| (TAU * (1800. + 4700. * colour) * t * (1. + 20. * t)).sin(),
    );
    // The column is a roar with its weight low down. The fall is the same water in
    // smaller pieces.
    let mut roar = [
        Biquad::high_pass(150. / heft, 0.7),
        Biquad::low_pass(1500. / heft, 0.6),
    ];
    let mut fall = Biquad::band_pass(1600., 0.5);
    let mut slap = Biquad::band_pass(1100. / heft, 0.7);
    let (mut low, soften) = (0_f32, pole(1000. / heft));
    let body = render(secs, |t| {
        let water = roar[0].run(rng.noise());
        low += soften * (roar[1].run(water) - low);
        0.6 * slap.run(rng.noise()) * (-t / 0.045).exp()
            + 2.2 * low * column(t)
            + 0.9 * fall.run(rng.noise()) * rain(t)
    });
    // The shell's own blow on the water, and the hollow it leaves closing.
    let mut deep = Biquad::low_pass(160. / heft, 0.7);
    let thud = render(1.2 * heft, |t| {
        0.5 * blast(t, 0.012 * heft)
            + (t / 0.003).min(1.) * 6. * deep.run(rng.noise()) * (-t / (0.16 * heft)).exp()
    });
    [spray, body, thud]
}

fn plate_strike(rng: &mut Rng, grazing: bool) -> [Vec<f32>; 3] {
    let pitch = rng.within(0.92, 1.08);
    let ring = if grazing { 0.20 } else { 0.08 };
    let detail = render(0.80, |t| {
        let tone = (TAU * pitch * (1510. * t - 610. * t * t)).sin()
            + 0.45 * (TAU * pitch * 2460. * t).sin();
        tone * (-t / ring).exp() + 0.80 * rng.noise() * (-t / 0.008).exp()
    });
    let body = render(0.66, |t| {
        ((TAU * pitch * 267. * t).sin() + 0.55 * (TAU * pitch * 427. * t).sin())
            * (-t / (ring * 0.75)).exp()
    });
    [detail, body, rumble(0.4, 170., rng)]
}

fn burst(rng: &mut Rng) -> [Vec<f32>; 3] {
    // Struck plate rings in a handful of inharmonic modes. The higher ones die first.
    let modes = [
        (412., 0.32),
        (687., 0.26),
        (1043., 0.2),
        (1620., 0.15),
        (2315., 0.1),
        (3180., 0.07),
        (4470., 0.05),
    ]
    .map(|(hz, ring): (f32, f32)| (hz * rng.within(0.96, 1.04), ring));
    let mut click = Biquad::high_pass(2500., 0.7);
    let plate = render(0.9, |t| {
        let ringing: f32 = modes
            .iter()
            .enumerate()
            .map(|(i, (hz, ring))| {
                (TAU * hz * t).sin() * (-t / ring).exp() / (1. + i as f32).sqrt()
            })
            .sum();
        0.35 * ringing + click.run(rng.noise()) * (-t / 0.004).exp()
    });
    [plate, boom(0.004, 0.1, rng), rumble(1.6, 380., rng)]
}

fn wreck(rng: &mut Rng) -> [Vec<f32>; 3] {
    let mut hiss = Biquad::high_pass(1500., 0.7);
    let mut debris = render(1.6, |t| hiss.run(rng.noise()) * (-t / 0.02).exp());
    // Wreckage keeps striking the water for a second after the blast.
    scatter(
        &mut debris,
        70,
        (0.05, 1.4),
        (0.001, 0.004),
        rng,
        |at| (-at / 0.5).exp(),
        |rng, _, _| rng.noise(),
    );
    [debris, boom(0.02, 0.6, rng), rumble(4.5, 260., rng)]
}

/// The shock a shell drags behind it when it flies faster than sound: pressure jumps,
/// falls in a straight line to as far below and jumps back. Too short to have weight.
fn crack(rng: &mut Rng) -> [Vec<f32>; 3] {
    let wave = |t: f32, secs: f32| if t < secs { 1. - 2. * t / secs } else { 0. };
    let long = rng.within(0.0011, 0.0016);
    let mut hiss = Biquad::high_pass(2200., 0.7);
    let snap = render(0.06, |t| {
        wave(t, long) + 0.5 * hiss.run(rng.noise()) * (-t / 0.004).exp()
    });
    // What is left of it after the first few metres, and the air torn along the path.
    let mut soft = Biquad::low_pass(900., 0.7);
    let mut torn = Biquad::band_pass(700., 0.5);
    let whip = render(0.25, |t| {
        soft.run(wave(t, long * 2.2)) + 0.7 * torn.run(rng.noise()) * (-t / 0.03).exp()
    });
    [snap, whip, rumble(0.3, 240., rng)]
}

/// The ship's own gun as the ship carries it to the bridge: the hull takes the blow and
/// everything loose aboard answers. `heft` is 1 for the main battery.
fn recoil(rng: &mut Rng, heft: f32) -> [Vec<f32>; 3] {
    // Fittings, plating and glass chatter for a moment after the shock.
    let mut rattle = vec![0.; (0.7 * RATE) as usize];
    scatter(
        &mut rattle,
        (60. * heft) as usize,
        (0.004, 0.5 * heft),
        (0.003, 0.016),
        rng,
        |at| (-at / (0.16 * heft)).exp(),
        |_, colour, t| (TAU * (700. + 4300. * colour * colour) * t).sin(),
    );
    // The hull is struck once and flexes in a few slow modes that die at once.
    let modes = [(41., 0.11), (63., 0.08), (97., 0.06), (148., 0.04)]
        .map(|(hz, ring): (f32, f32)| (hz * rng.within(0.93, 1.07) / heft.sqrt(), ring * heft));
    let mut dull = Biquad::low_pass(210., 0.7);
    let thud = render(0.5, |t| {
        let flex: f32 = modes
            .iter()
            .map(|(hz, ring)| (TAU * hz * t).sin() * (-t / ring).exp())
            .sum();
        let knock = 5. * dull.run(rng.noise()) * (-t / (0.035 * heft)).exp();
        (1.5 * ((t / 0.0015).min(1.) * (0.6 * flex + knock))).tanh()
    });
    [rattle, thud, rumble(0.9 * heft, 110., rng)]
}

// A short bridge call with two tongued pickups and a held high note.
fn trumpet() -> [Vec<f32>; 3] {
    std::array::from_fn(|layer| {
        render(1.08, |t| {
            let (start, length, hz) = if t < 0.18 {
                (0., 0.16, 392.)
            } else if t < 0.37 {
                (0.18, 0.16, 493.88)
            } else {
                (0.37, 0.68, 587.33)
            };
            let age = t - start;
            let envelope = (age / 0.017).min(1.) * ((length - age) / 0.085).clamp(0., 1.);
            let attack = (-age / 0.035).exp();
            let phase = TAU * hz * age - 0.9 * (1. - attack)
                + 0.23 * (age * TAU * 5.6).sin() * (age / 0.4).min(1.);
            let voice = match layer {
                0 => {
                    ((4. * phase).sin() + 0.7 * (5. * phase).sin() + 0.3 * (7. * phase).sin())
                        * (0.5 + attack)
                }
                1 => phase.sin() + 0.82 * (2. * phase).sin() + 0.56 * (3. * phase).sin(),
                _ => phase.sin() * (1. - 0.25 * age),
            };
            envelope * voice
        })
    })
}

/// A structure coming down. Recordings of buildings falling show what to make: not one
/// crash dying away but several a few tenths of a second apart, the heaviest well after
/// the first, with ten to thirty-five separate knocks a second among them and most of
/// the sound between 125 Hz and 2 kHz.
struct Fall {
    /// Seconds from the hit to the last of the debris.
    secs: f32,
    /// How many crashes it comes down in, and the seconds after the hit of the worst.
    crashes: usize,
    worst: f32,
    /// Girders among the masonry.
    steel: bool,
}
impl Fall {
    const BUILDING: Self = Self {
        secs: 2.6,
        crashes: 5,
        worst: 0.7,
        steel: false,
    };
    const YARD: Self = Self {
        secs: 4.2,
        crashes: 8,
        worst: 1.5,
        steel: true,
    };

    fn cast(self, rng: &mut Rng) -> [Vec<f32>; 3] {
        // When each crash lands, how heavy it is and how long it takes to settle.
        let mut at = rng.within(0.1, 0.2);
        let crashes: Vec<(f32, f32, f32)> = (0..self.crashes)
            .map(|_| {
                let off = (at - self.worst) / (0.5 * self.worst + 0.25);
                let crash = (
                    at,
                    ((-off * off).exp() * rng.within(0.6, 1.)).max(0.15),
                    rng.within(0.14, 0.3),
                );
                at += 0.7 * self.secs / self.crashes as f32 * rng.within(0.6, 1.4);
                crash
            })
            .collect();
        // How much is falling at any moment: the crashes, and a trickle that outlasts them.
        let falling = |t: f32| -> f32 {
            let crashing: f32 = crashes
                .iter()
                .filter(|crash| t >= crash.0)
                .map(|&(at, weight, settle)| {
                    weight * ((t - at) / 0.025).min(1.) * (-(t - at) / settle).exp()
                })
                .sum();
            crashing + 0.08 * (t / 0.3).min(1.) * (-3. * t / self.secs).exp()
        };

        // Dust and grit pour with each crash, and bricks, tiles and glass clatter after it.
        let mut hiss = Biquad::high_pass(2200., 0.7);
        let mut clatter = render(self.secs, |t| {
            0.08 * hiss.run(rng.noise()) * falling((t - 0.05).max(0.))
        });
        scatter(
            &mut clatter,
            (60. * self.secs) as usize,
            (0.12, self.secs * 0.95),
            (0.001, 0.005),
            rng,
            |at| falling((at - 0.08).max(0.)) + 0.3 * (1. - at / self.secs),
            |rng, colour, t| 0.7 * (TAU * (1200. + 4800. * colour) * t).sin() + 0.3 * rng.noise(),
        );

        // The shell that did it, then masonry going over in a mass with the heavier
        // pieces knocking against one another.
        let mut band = [Biquad::high_pass(110., 0.7), Biquad::low_pass(1500., 0.6)];
        let mut mass = render(self.secs, |t| {
            let fall = band[0].run(rng.noise());
            2.5 * band[1].run(fall) * falling(t)
        });
        for (x, burst) in mass.iter_mut().zip(boom(0.004, 0.1, rng)) {
            *x += 0.8 * burst;
        }
        scatter(
            &mut mass,
            (40. * self.secs) as usize,
            (0.1, self.secs * 0.85),
            (0.006, 0.03),
            rng,
            falling,
            |rng, colour, t| {
                0.6 * (TAU * (220. + 1200. * colour * colour) * t).sin() + 0.4 * rng.noise()
            },
        );

        // Each crash lands with a blow on the ground, over the roll of the whole.
        let mut deep = Biquad::low_pass(120., 0.7);
        let mut ground = rumble(self.secs + 0.6, 210., rng);
        for (n, x) in ground.iter_mut().enumerate() {
            let t = n as f32 / RATE;
            let blows: f32 = crashes
                .iter()
                .map(|&(at, weight, _)| weight * blast(t - at, 0.018))
                .sum();
            *x = 0.5 * *x + 5. * deep.run(rng.noise()) * falling(t) + 0.6 * blows;
        }

        if self.steel {
            // A girder rings when it lands, each at its own pitch. This part is not
            // from any recording.
            const MODES: [(f32, f32); 7] = [
                (173., 0.55),
                (291., 0.5),
                (468., 0.42),
                (733., 0.34),
                (1127., 0.26),
                (1690., 0.2),
                (2540., 0.14),
            ];
            for &(at, weight, _) in &crashes {
                let pitch = rng.within(0.75, 1.3);
                let start = (at * RATE) as usize;
                for (i, (hz, ring)) in MODES.into_iter().enumerate() {
                    let layer = if hz < 900. { &mut mass } else { &mut clatter };
                    let level = 0.45 * weight / (1. + i as f32).sqrt();
                    for (n, x) in layer.iter_mut().skip(start).enumerate() {
                        let t = n as f32 / RATE;
                        *x += level * (TAU * pitch * hz * t).sin() * (-t / ring).exp();
                    }
                }
            }
        }
        [clatter, mass, ground]
    }
}

/// Brass and drums for a base destroyed: three quick notes up to a held one, an answer,
/// and a full chord of G to finish. Heard aboard, so it carries its own hall.
fn fanfare(rng: &mut Rng) -> [Vec<f32>; 3] {
    const SECS: f32 = 3.4;
    const G: f32 = 392.;
    // Start, length, hertz and loudness of every note of every player.
    let (b, d) = (G * 1.2599, G * 1.4983);
    let (first, long, last) = (0.1, 0.49, 1.27);
    let notes = [
        // First trumpet carries the tune.
        (first, 0.11, G, 1.),
        (first + 0.13, 0.11, G, 1.),
        (first + 0.26, 0.11, G, 1.),
        (long, 0.42, d, 1.),
        (0.93, 0.15, b, 1.),
        (1.1, 0.15, d, 1.),
        (last, 1.5, 2. * G, 1.),
        // Second and third play along with it, and fill the chords under the long notes.
        (first, 0.11, G, 0.8),
        (first + 0.13, 0.11, G, 0.8),
        (first + 0.26, 0.11, G, 0.8),
        (long, 0.42, b, 0.7),
        (long, 0.42, G, 0.6),
        (0.93, 0.15, G, 0.7),
        (1.1, 0.15, b, 0.7),
        (last, 1.5, d, 0.75),
        (last, 1.5, b, 0.7),
        (last, 1.5, G, 0.6),
        // Trombones below them.
        (first, 0.11, G / 2., 0.9),
        (first + 0.13, 0.11, G / 2., 0.9),
        (first + 0.26, 0.11, G / 2., 0.9),
        (long, 0.42, G / 2., 0.8),
        (0.93, 0.15, G / 2., 0.8),
        (1.1, 0.15, G / 2., 0.8),
        (last, 1.5, G / 2., 0.9),
        (last, 1.5, d / 4., 0.6),
        (last, 1.5, G / 4., 0.6),
    ];
    let mut layers = [(); 3].map(|_| vec![0.; (SECS * RATE) as usize]);
    for (start, length, hz, loud) in notes {
        // No two players are quite in tune or quite together.
        let hz = hz * (1. + 0.004 * rng.noise());
        let late = (start + 0.008 * rng.unit()) * RATE;
        let wobble = TAU * (5.2 + 0.5 * rng.unit());
        for i in 0..((length + 0.1) * RATE) as usize {
            let t = i as f32 / RATE;
            let blown = (t / 0.018).min(1.) * ((length - t) / 0.07).clamp(0., 1.);
            // A long note swells a little and takes on a vibrato once it has settled.
            let swell = 1. + 0.25 * (t / length).min(1.) * f32::from(length > 0.4);
            let tongue = (-t / 0.035).exp();
            let turn = TAU * hz * t - 0.9 * (1. - tongue)
                + 0.004 * hz / 5.5 * (wobble * t).sin() * ((t - 0.3) / 0.4).clamp(0., 1.);
            // Brass is brighter the harder it is blown, and brightest as it is tongued.
            let bright = 1.1 + 1.3 * (1. - loud * blown * swell * (0.7 + 0.5 * tongue)).max(0.);
            let at = late as usize + i;
            for partial in 1..=14 {
                let pitch = partial as f32 * hz;
                if pitch > 9000. || at >= layers[0].len() {
                    break;
                }
                let layer = (pitch > 260.) as usize + (pitch > 1700.) as usize;
                layers[2 - layer][at] += loud * blown * swell * (partial as f32 * turn).sin()
                    / (partial as f32).powf(bright);
            }
        }
    }
    // Kettledrums under both long notes, and a cymbal on the last.
    let mut skin = Biquad::low_pass(220., 0.7);
    let mut shimmer = Biquad::high_pass(3800., 0.7);
    for (n, t) in (0..layers[0].len()).map(|n| (n, n as f32 / RATE)) {
        for (struck, weight) in [(long, 0.7), (last, 1.)] {
            let since = t - struck;
            if since >= 0. {
                let drum = (TAU * 98. * (since + 0.004 * (1. - (-since / 0.03).exp()))).sin();
                layers[2][n] += weight * 2.2 * drum * (-since / 0.45).exp();
                layers[1][n] += weight * 0.5 * (TAU * 147. * since).sin() * (-since / 0.2).exp();
            }
        }
        let thump = skin.run(rng.noise());
        let crash = shimmer.run(rng.noise());
        layers[2][n] += 3. * thump * (-(t - last).abs() / 0.03).exp();
        if t >= last {
            layers[0][n] += 1.6 * crash * ((t - last) / 0.004).min(1.) * (-(t - last) / 0.55).exp();
        }
    }
    layers.map(hall)
}

/// Put a sound in a hall: four echoes that go round and round, smeared by two more.
fn hall(dry: Vec<f32>) -> Vec<f32> {
    const ROUND: [(usize, f32); 4] = [(1687, 0.8), (1601, 0.79), (2053, 0.77), (2251, 0.75)];
    const SMEAR: [usize; 2] = [347, 113];
    const WET: f32 = 0.16;
    let mut rounds = ROUND.map(|(len, _)| vec![0.; len]);
    let mut smears = SMEAR.map(|len| vec![0.; len]);
    let mut damp = [0.; 4];
    (dry.iter().enumerate())
        .map(|(n, &x)| {
            let mut wet = 0.;
            for (i, (round, (len, keep))) in rounds.iter_mut().zip(ROUND).enumerate() {
                let back = round[n % len];
                damp[i] += 0.45 * (back - damp[i]);
                round[n % len] = x + keep * damp[i];
                wet += back;
            }
            for (smear, len) in smears.iter_mut().zip(SMEAR) {
                let back = smear[n % len];
                let through = back - 0.6 * wet;
                smear[n % len] = wet + 0.6 * through;
                wet = through;
            }
            x + WET * wet
        })
        .collect()
}

/// Ice splitting. Recordings show one to three cracks a few tenths of a second apart.
/// Each is a snap that is over in a hundredth of a second, and under it the mass of
/// the ice answers between 100 and 450 Hz for a tenth to a third of a second.
fn ice_crack(rng: &mut Rng) -> [Vec<f32>; 3] {
    const SECS: f32 = 1.8;
    let mut at = 0.01;
    let cracks: Vec<(f32, f32, f32)> = (0..1 + (rng.unit() * 3.) as usize)
        .map(|n| {
            let weight = if n == 0 { 1. } else { rng.within(0.4, 1.) };
            let crack = (at, weight, rng.within(0.05, 0.14));
            at += rng.within(0.12, 0.45);
            crack
        })
        .collect();
    let each = |t: f32, fall: f32| -> f32 {
        let since = cracks.iter().filter(|crack| t >= crack.0);
        since
            .map(|&(at, weight, ring)| weight * (-(t - at) / (fall * ring)).exp())
            .sum()
    };
    let mut hiss = Biquad::high_pass(2500., 0.7);
    let mut snap = render(SECS, |t| hiss.run(rng.noise()) * each(t, 0.07));
    // Splinters go on falling for a moment.
    scatter(
        &mut snap,
        24,
        (0.02, 0.9),
        (0.001, 0.004),
        rng,
        |at| 0.25 * (-at / 0.3).exp(),
        |_, colour, t| (TAU * (2400. + 5000. * colour) * t).sin(),
    );
    let mut mass = [Biquad::band_pass(250., 0.8), Biquad::low_pass(480., 0.7)];
    let body = render(SECS, |t| {
        let push: f32 = cracks
            .iter()
            .map(|&(at, weight, _)| weight * blast(t - at, 0.002))
            .sum();
        let ring = mass[0].run(rng.noise());
        0.5 * push + 6. * mass[1].run(ring) * each(t, 1.)
    });
    let mut deep = Biquad::low_pass(140., 0.7);
    let under = render(SECS, |t| deep.run(rng.noise()) * each(t, 2.5));
    [snap, body, under]
}

/// A glacier dropping part of its front into the sea. Two recordings of it agree: a
/// crack or two, then a roar that builds for a second or more as the ice goes over,
/// with most of its power below 200 Hz and almost none above 1 kHz. It falls 11 dB in
/// the second after its peak, and then the sea it threw up washes about among the
/// broken ice for ten seconds and more, brighter than the fall was.
fn calving(rng: &mut Rng) -> [Vec<f32>; 3] {
    const SECS: f32 = 12.;
    let snaps = [(0.02, 1.), (rng.within(0.3, 0.7), rng.within(0.4, 0.9))];
    let (start, rise) = (rng.within(0.8, 1.1), rng.within(1.2, 2.4));
    let worst = start + rise;
    // The pieces going in, the biggest of them about the peak.
    let pieces: Vec<(f32, f32, f32)> = (0..9)
        .map(|_| {
            let at = start + 1.25 * rise * rng.unit().sqrt();
            let off = (at - worst) / (0.5 * rise);
            let weight = ((-off * off).exp() * rng.within(0.5, 1.)).max(0.12);
            (at, weight, rng.within(0.25, 0.6))
        })
        .collect();
    let falling = |t: f32| -> f32 {
        let gone = pieces.iter().filter(|piece| t >= piece.0);
        gone.map(|&(at, weight, settle)| {
            weight * ((t - at) / 0.06).min(1.) * (-(t - at) / settle).exp()
        })
        .sum()
    };
    let wash =
        |t: f32| 0.35 * ((t - worst) / 0.8).clamp(0., 1.) * (-(t - worst).max(0.) / 6.).exp();

    let mut hiss = [Biquad::high_pass(2500., 0.7), Biquad::high_pass(1800., 0.7)];
    let mut spray = render(SECS, |t| {
        let snapped: f32 = (snaps.iter().filter(|snap| t >= snap.0))
            .map(|&(at, weight)| weight * (-(t - at) / 0.008).exp())
            .sum();
        hiss[0].run(rng.noise()) * snapped
            + hiss[1].run(rng.noise()) * (0.3 * falling((t - 0.1).max(0.)) + 0.5 * wash(t))
    });
    scatter(
        &mut spray,
        160,
        (start, worst + 3.),
        (0.002, 0.008),
        rng,
        |at| 0.4 * falling(at) + wash(at),
        |_, colour, t| (TAU * (1500. + 4500. * colour) * t).sin(),
    );

    let mut band = [Biquad::high_pass(110., 0.7), Biquad::low_pass(900., 0.6)];
    let mut surge = Biquad::band_pass(450., 0.6);
    let mut n = 0;
    let body = render(SECS, |t| {
        if n % 64 == 0 {
            let on = ((t - worst) / 6.).clamp(0., 1.);
            surge.retune(Biquad::band_pass(450. + 450. * on, 0.6));
        }
        n += 1;
        let push: f32 = snaps
            .iter()
            .map(|&(at, weight)| weight * blast(t - at, 0.003))
            .sum();
        let ice = band[0].run(rng.noise());
        0.3 * push + 2.5 * band[1].run(ice) * falling(t) + 3. * surge.run(rng.noise()) * wash(t)
    });

    let mut deep = [Biquad::low_pass(180., 0.7), Biquad::low_pass(110., 0.7)];
    let under = render(SECS, |t| {
        let blows: f32 = pieces
            .iter()
            .map(|&(at, weight, _)| weight * blast(t - at, 0.03))
            .sum();
        5. * deep[0].run(rng.noise()) * falling(t)
            + 0.5 * blows
            + 2. * deep[1].run(rng.noise()) * wash(t)
    });
    [spray, body, under]
}

/// A kittiwake calling. The bird says its own name: two short notes and a long one
/// that falls away. Recordings put the notes between 550 and 850 Hz, the long one
/// falling a fifth of its pitch over a third of a second, with as much sound in the
/// overtones near 1.7 kHz as in the note itself. A bird calls one to three times,
/// rather less than a second apart.
fn gull(rng: &mut Rng) -> [Vec<f32>; 3] {
    // Start, length, and the pitch it begins and ends on, for a bird of middling voice.
    const NOTES: [(f32, f32, f32, f32); 3] = [
        (0., 0.09, 700., 720.),
        (0.14, 0.1, 760., 740.),
        (0.3, 0.34, 690., 540.),
    ];
    /// Strength of each overtone, which peaks again about the third.
    const VOICE: [f32; 7] = [1., 0.55, 0.8, 0.45, 0.25, 0.15, 0.1];
    let calls = 1 + (rng.unit() * 3.) as usize;
    let pitch = rng.within(0.88, 1.15);
    let every = rng.within(0.75, 1.05);
    let mut layers = [(); 3].map(|_| vec![0.; ((calls as f32 * every + 0.2) * RATE) as usize]);
    for call in 0..calls {
        let loud = rng.within(0.6, 1.);
        let slow = rng.within(0.9, 1.2);
        for (start, length, from, to) in NOTES {
            let (start, length) = (call as f32 * every + start * slow, length * slow);
            let rasp = TAU * rng.within(70., 110.);
            let mut turn = 0.;
            let mut waver = 0.;
            for i in 0..(length * RATE) as usize {
                let t = i as f32 / RATE;
                let along = t / length;
                // The long note rises a little before it falls.
                let hz = pitch * (from + (to - from) * along * along + 30. * (PI * along).sin());
                waver += 0.01 * (rng.noise() - waver);
                turn += TAU * hz * (1. + 0.6 * waver) / RATE;
                let blown = (t / 0.015).min(1.) * ((length - t) / 0.04).clamp(0., 1.);
                let harsh = 1. - 0.35 * (0.5 + 0.5 * (rasp * t).sin());
                let at = (start * RATE) as usize + i;
                for (n, weight) in VOICE.into_iter().enumerate() {
                    let partial = (n + 1) as f32;
                    let layer = usize::from(partial * hz < 1400.);
                    layers[layer][at] += loud * blown * harsh * weight * (partial * turn).sin();
                }
            }
        }
    }
    layers
}

/// The sea breaking on a shore. Recordings of waves on rocks are a broad hiss, level
/// from 400 Hz to 2 kHz, with half its power below 1.1 kHz and a tenth below 300. A
/// wave rears for a quarter of a second, slaps, loses 6 dB in the next quarter and
/// washes back for three seconds.
fn breaker(rng: &mut Rng) -> [Vec<f32>; 3] {
    const SECS: f32 = 3.6;
    let rear = rng.within(0.2, 0.45);
    let slap = rng.within(0.5, 1.);
    let run = rng.within(1.5, 2.5);
    let wave = |t: f32| {
        if t < rear {
            0.5 * (t / rear).powi(2)
        } else {
            slap * (-(t - rear) / 0.12).exp() + 0.6 * (-(t - rear) / run).exp()
        }
    };
    // Foam hisses as the water runs back, and drops off the rocks fall into it.
    let mut hiss = Biquad::high_pass(2000., 0.7);
    let mut foam = render(SECS, |t| {
        0.5 * hiss.run(rng.noise()) * wave((t - 0.05).max(0.))
    });
    scatter(
        &mut foam,
        60,
        (rear, SECS * 0.8),
        (0.002, 0.007),
        rng,
        |at| 0.5 * wave(at),
        |_, colour, t| (TAU * (1800. + 4000. * colour) * t * (1. + 12. * t)).sin(),
    );
    let mut band = [Biquad::high_pass(300., 0.6), Biquad::low_pass(3500., 0.6)];
    let (mut surge, sway) = (0_f32, pole(3.));
    let body = render(SECS, |t| {
        surge += sway * (rng.noise() - surge);
        let uneven = (1. + surge * 30.).clamp(0.5, 1.6);
        let water = band[0].run(rng.noise());
        band[1].run(water) * wave(t) * uneven
    });
    let mut deep = Biquad::low_pass(200., 0.7);
    let thud = render(SECS, |t| {
        deep.run(rng.noise()) * (t / rear).min(1.).powi(2) * (-(t - rear).max(0.) / 0.3).exp()
    });
    [foam, body, thud]
}

/// A riveting gang at work. Two recordings of riveting guns agree on sixteen blows a
/// second, each a click on a plate that is still ringing when the next one lands,
/// and brightest between 1.6 and 3 kHz. A gang drives a rivet in a second or two.
fn rivets(rng: &mut Rng) -> [Vec<f32>; 3] {
    /// The plate's own tones, how loud each is and the seconds it rings.
    const PLATE: [(f32, f32, f32); 5] = [
        (620., 0.3, 0.05),
        (1650., 0.8, 0.07),
        (2250., 1., 0.06),
        (3050., 0.8, 0.05),
        (4100., 0.4, 0.03),
    ];
    let secs = rng.within(1.2, 2.8);
    let every = 1. / rng.within(15., 16.5);
    let pitch = rng.within(0.88, 1.12);
    let mut layers = [(); 3].map(|_| vec![0.; ((secs + 0.5) * RATE) as usize]);
    let (mut click, mut knock) = (Biquad::high_pass(2500., 0.7), Biquad::band_pass(300., 0.8));
    let mut deep = Biquad::low_pass(150., 0.7);
    for blow in 0..(secs / every) as usize {
        let start = ((blow as f32 + 0.1 * rng.noise()) * every).max(0.);
        let weight = rng.within(0.7, 1.);
        let start = (start * RATE) as usize;
        for (hz, level, ring) in PLATE {
            let layer = usize::from(hz < 1400.);
            let turn = TAU * rng.unit();
            for (n, x) in (layers[layer].iter_mut().skip(start))
                .take((6. * ring * RATE) as usize)
                .enumerate()
            {
                let t = n as f32 / RATE;
                *x += weight * level * (TAU * pitch * hz * t + turn).sin() * (-t / ring).exp();
            }
        }
        for n in 0..(0.08 * RATE) as usize {
            let t = n as f32 / RATE;
            layers[0][start + n] += 2. * weight * click.run(rng.noise()) * (-t / 0.002).exp();
            layers[1][start + n] += weight * knock.run(rng.noise()) * (-t / 0.012).exp();
            layers[2][start + n] += weight * deep.run(rng.noise()) * (-t / 0.02).exp();
        }
    }
    layers
}

/// The ship's frames working in a seaway. A recording of a hull creaking is a groan
/// two seconds long with nearly all of it below 330 Hz. It is steel sticking and
/// slipping thirty or forty times a second, which sets the frames ringing near 190
/// and 320 Hz, and it swells for half a second before it eases.
fn creak(rng: &mut Rng) -> [Vec<f32>; 3] {
    /// What rings, how loud and how sharply.
    const FRAMES: [(f32, f32, f32); 4] = [
        (193., 0.7, 14.),
        (322., 1., 18.),
        (826., 0.35, 20.),
        (955., 0.3, 20.),
    ];
    let secs = rng.within(1.2, 2.4);
    let pitch = rng.within(0.8, 1.25);
    let jerks = rng.within(26., 44.);
    let eases = rng.within(0.5, 0.9);
    let mut ring = FRAMES.map(|(hz, _, sharp)| Biquad::band_pass(pitch * hz, sharp));
    let mut deep = Biquad::low_pass(90., 0.7);
    let (mut turn, mut slip, mut rate) = (0_f32, 0_f32, jerks);
    let mut layers = [(); 3].map(|_| Vec::new());
    for n in 0..(secs * RATE) as usize {
        let t = n as f32 / RATE;
        let strain = (t / 0.45).min(1.) * (-(t - 0.45).max(0.) / (0.35 * secs)).exp();
        turn += rate / RATE;
        if turn >= 1. {
            turn -= 1.;
            // No two slips are alike, and they come slower as the strain comes off.
            slip = rng.within(0.4, 1.);
            rate = jerks * rng.within(0.8, 1.2) * (eases + (1. - eases) * strain);
        }
        let jerk = slip * strain * (-turn * 12.).exp();
        let mut rung = [0.; 4];
        for (rung, ring) in rung.iter_mut().zip(&mut ring) {
            *rung = ring.run(jerk);
        }
        layers[0].push(FRAMES[2].1 * rung[2] + FRAMES[3].1 * rung[3]);
        layers[1].push(FRAMES[0].1 * rung[0] + FRAMES[1].1 * rung[1]);
        layers[2].push(deep.run(jerk));
    }
    layers
}

/// The engine-room telegraph answering the bridge. A recording of one is a small
/// gong struck every quarter of a second or so as the handle goes over, with tones at
/// 891, 1538, 2256, 3052, 3940, 4922 and 5990 Hz, of which the upper are the loudest.
fn telegraph(rng: &mut Rng) -> [Vec<f32>; 3] {
    /// Each tone of the gong, how loud it is and the seconds it rings.
    const GONG: [(f32, f32, f32); 7] = [
        (891., 0.5, 0.45),
        (1538., 0.8, 0.4),
        (2256., 0.45, 0.32),
        (3052., 0.9, 0.28),
        (3940., 0.4, 0.22),
        (4922., 1., 0.18),
        (5990., 0.6, 0.12),
    ];
    let strokes = 3 + (rng.unit() * 3.) as usize;
    let mut layers = [(); 3].map(|_| vec![0.; ((0.3 * strokes as f32 + 1.4) * RATE) as usize]);
    let mut ratchet = Biquad::band_pass(420., 1.);
    let mut at = 0.01;
    for _ in 0..strokes {
        let weight = rng.within(0.6, 1.);
        let start = (at * RATE) as usize;
        for (hz, level, ring) in GONG {
            let layer = usize::from(hz < 1400.);
            for (n, x) in (layers[layer].iter_mut().skip(start))
                .take((6. * ring * RATE) as usize)
                .enumerate()
            {
                let t = n as f32 / RATE;
                *x += weight * level * (TAU * hz * t).sin() * (-t / ring).exp();
            }
        }
        // The handle's pawl drops into its notch with each stroke.
        for n in 0..(0.06 * RATE) as usize {
            let t = n as f32 / RATE;
            layers[1][start + n] += 2. * weight * ratchet.run(rng.noise()) * (-t / 0.008).exp();
        }
        at += rng.within(0.16, 0.3);
    }
    layers
}

/// Something gained: a short note and a longer one a fifth above it, ba-BOOP. Each is
/// struck and left to ring, and is bent up into its pitch as it starts, which is the
/// consonant. The second has its octave below under it for weight. This one is not
/// from any recording.
fn upgrade() -> [Vec<f32>; 3] {
    /// When each note starts, the seconds it rings, its pitch and its loudness.
    const NOTES: [(f32, f32, f32, f32); 2] = [(0.01, 0.05, 523.25, 0.7), (0.115, 0.15, 783.99, 1.)];
    /// How far below its pitch a note starts, and the seconds it takes to get there.
    const BEND: (f32, f32) = (0.08, 0.012);
    let layers = std::array::from_fn(|layer| {
        render(0.9, |t| {
            let notes = NOTES.into_iter().filter(|note| t >= note.0);
            notes
                .map(|(start, ring, hz, loud)| {
                    let age = t - start;
                    let bent = age + BEND.0 * BEND.1 * ((-age / BEND.1).exp() - 1.);
                    let turn = TAU * hz * bent;
                    let struck = loud * (age / 0.003).min(1.) * (-age / ring).exp();
                    struck
                        * match layer {
                            // The edge of the note is gone before the note is.
                            0 => {
                                ((3. * turn).sin() + 0.5 * (4. * turn).sin())
                                    * (-age / (0.3 * ring)).exp()
                            }
                            1 => turn.sin() + 0.3 * (2. * turn).sin(),
                            _ => f32::from(hz > 600.) * (0.5 * turn).sin(),
                        }
                })
                .sum()
        })
    });
    layers.map(hall)
}

/// A small brass handbell struck once by its clapper. A handbell is tuned so that its
/// strongest overtone is the twelfth, three times its note, which is what makes it
/// sweet where a ship's bell clangs. Measured handbells put their next modes near five
/// and eight times the note, out of tune with it, and these and the knock of the clapper
/// are gone within a few hundredths of a second. The twelfth fades in a quarter of a
/// second and the note itself rings on. No bell is quite round, so each mode is a pair
/// a few hertz apart, and the note shimmers as it dies away.
fn chime() -> [Vec<f32>; 3] {
    /// The note: E6, a sixth above the upgrade's upper note.
    const NOTE: f32 = 1318.5;
    /// Each mode as a multiple of the note, its loudness, the seconds it rings, and how
    /// far apart its pair stands in hertz.
    const MODES: [(f32, f32, f32, f32); 5] = [
        (1., 1., 0.62, 1.7),
        (3.012, 0.42, 0.24, 2.6),
        (5.21, 0.14, 0.065, 4.),
        (8.37, 0.08, 0.032, 0.),
        (11.9, 0.04, 0.018, 0.),
    ];
    /// The clapper meets the lip: its knock's centre, and the seconds it lasts.
    const KNOCK: (f32, f32) = (5200., 0.0016);
    /// Seconds rendered, and the last stretch of them in which the ring is let go.
    const SECS: f32 = 2.2;
    const RELEASE: f32 = 0.45;
    let mut rng = Rng::new(0xB311);
    let mut knock = Biquad::band_pass(KNOCK.0, 1.3);
    let count = (SECS * RATE) as usize;
    let mut layers = [(); 3].map(|_| vec![0.; count]);
    for n in 0..count {
        let t = n as f32 / RATE;
        let release = ((SECS - t) / RELEASE).min(1.);
        let fade = release * release * (3. - 2. * release);
        let onset = (t / 0.0012).min(1.);
        for (ratio, loud, ring, split) in MODES {
            let hz = NOTE * ratio;
            let pair = (TAU * hz * t).sin() + 0.35 * (TAU * (hz + split) * t + 1.1).sin();
            let x = loud * onset * (-t / ring).exp() * pair * fade;
            layers[usize::from(ratio < 4.)][n] += x;
        }
        layers[0][n] += 0.9 * knock.run(rng.noise()) * (-t / KNOCK.1).exp();
    }
    layers.map(hall)
}

/// A klaxon: a motor spins a toothed wheel against a stud on a steel diaphragm, and a
/// horn throws out the rattle. Its note is the teeth going by, so it climbs as the
/// motor gathers speed and sags as it runs down, which is the ah-OO-gah. Three
/// recordings of them, a car's and two submarines' diving alarms, agree: up to 400 Hz
/// or a little over in a quarter of a second, held for a second, then down through
/// 300, 200 and 140 Hz in the half second after the button is let go. The overtones
/// are as strong as the note itself up to 1.3 kHz and carry on past 4 kHz.
fn klaxon(rng: &mut Rng) -> [Vec<f32>; 3] {
    /// Teeth a second at full speed, the seconds the button is held, and the seconds
    /// the motor takes to gather speed and to lose it.
    const NOTE: f32 = 400.;
    const HELD: f32 = 1.15;
    const MOTOR: (f32, f32) = (0.09, 0.45);
    /// Below this share of full speed the wheel no longer reaches the stud.
    const REACH: f32 = 0.3;
    /// The horn's own tones in hertz, how sharply it favours each, and how much.
    const HORN: [(f32, f32, f32); 3] = [(1250., 4., 1.), (2700., 5., 0.6), (4000., 5., 0.35)];
    let top = NOTE * rng.within(0.98, 1.06);
    let mut horn = HORN.map(|(hz, sharp, _)| Biquad::band_pass(hz, sharp));
    let mut plate = [Biquad::high_pass(250., 0.7), Biquad::low_pass(1500., 0.7)];
    let mut edge = Biquad::high_pass(1800., 0.7);
    let (mut speed, mut turn, mut ring) = (0_f32, 0_f32, 0_f32);
    let mut layers = [(); 3].map(|_| Vec::new());
    for n in 0..(2. * RATE) as usize {
        let t = n as f32 / RATE;
        let (wanted, secs) = if t < HELD {
            (1., MOTOR.0)
        } else {
            (0., MOTOR.1)
        };
        speed += (wanted - speed) / (secs * RATE);
        turn += top * speed / RATE;
        if turn >= 1. {
            turn -= 1.;
            // Each tooth knocks the diaphragm, harder the faster it comes, and no two
            // knocks land quite alike.
            let reaches = ((speed - REACH) / (1. - REACH)).max(0.);
            ring += reaches.powf(1.5) * (1. + 0.15 * rng.noise());
        }
        // The diaphragm is steel and stops ringing at once.
        ring *= (-1. / (0.0004 * RATE)).exp();
        let thrown: f32 = (horn.iter_mut().zip(HORN))
            .map(|(tone, (_, _, weight))| weight * tone.run(ring))
            .sum();
        let body = plate[0].run(ring);
        layers[0].push(edge.run(thrown));
        layers[1].push(plate[1].run(body) + 1.2 * thrown);
        layers[2].push(0.);
    }
    layers
}

/// A torpedo leaving a deck tube. A small charge of powder or a blast of air throws it
/// out: a dull cough with no crack in it, for the tube is open at the muzzle and the
/// torpedo is a loose plug in it, and a hiss as the gas follows it out. A third of a
/// second later the torpedo strikes the water nearly flat, a short slap with a little
/// spray, far less water than a shell throws up.
fn torpedo_launch(rng: &mut Rng) -> [Vec<f32>; 3] {
    const SECS: f32 = 1.4;
    let drop = rng.within(0.28, 0.38);
    let gas = |t: f32| (t / 0.004).min(1.) * (0.8 * (-t / 0.05).exp() + 0.35 * (-t / 0.22).exp());
    let entry = |t: f32| {
        let after = t - drop;
        if after < 0. {
            0.
        } else {
            (after / 0.008).min(1.) * (-after / 0.16).exp()
        }
    };
    let mut hiss = Biquad::high_pass(2600., 0.7);
    let mut air = Biquad::band_pass(5200., 0.6);
    let mut wet = Biquad::high_pass(3200., 0.7);
    let mut detail = render(SECS, |t| {
        (0.8 * hiss.run(rng.noise()) + 0.3 * air.run(rng.noise())) * gas(t)
            + 0.5 * wet.run(rng.noise()) * entry(t)
    });
    // What it threw up patters back.
    scatter(
        &mut detail,
        60,
        (drop + 0.05, drop + 0.8),
        (0.002, 0.006),
        rng,
        |at| (-(at - drop) / 0.25).exp(),
        |_, colour, t| (TAU * (2000. + 4000. * colour) * t * (1. + 15. * t)).sin(),
    );
    let mut cough = [Biquad::band_pass(380., 0.9), Biquad::low_pass(900., 0.7)];
    let mut slap = Biquad::band_pass(1000., 0.8);
    let body = render(SECS, |t| {
        let puff = cough[0].run(rng.noise());
        let puff = cough[1].run(puff);
        let puff = puff * (t / 0.003).min(1.) * (-t / 0.07).exp();
        let after = t - drop;
        let hit = slap.run(rng.noise())
            * if after < 0. {
                0.
            } else {
                (-after / 0.035).exp()
            };
        2.4 * puff + 0.6 * blast(t, 0.005) + 0.9 * hit
    });
    let mut deep = Biquad::low_pass(140., 0.7);
    let thud = render(0.9, |t| {
        let after = t - drop;
        let entry = if after < 0. {
            0.
        } else {
            0.5 * (-after / 0.06).exp()
        };
        let push = (t / 0.004).min(1.) * (-t / 0.09).exp();
        0.7 * blast(t, 0.009) + 4. * deep.run(rng.noise()) * (push + entry)
    });
    [detail, body, thud]
}

/// The bridge's alarm bell. An electric bell's clapper is a hammer on a spring that a
/// magnet pulls against the gong and lets go, fifteen to twenty times a second, so the
/// gong never stops ringing between blows and its note trills. Ships' alarm bells of
/// six or eight inches ring near 1 kHz, with partials out of tune above it near 2.4, 4
/// and 5.7 times the note, the lowest of them split in a pair, for no gong is quite
/// round. The bell is held for a second and a half and rings on as it is let go.
fn torpedo_alarm(rng: &mut Rng) -> [Vec<f32>; 3] {
    const NOTE: f32 = 1046.5;
    /// Each mode as a multiple of the note, how hard a blow sets it ringing and the
    /// seconds it rings.
    const MODES: [(f32, f32, f32); 5] = [
        (1., 1., 0.5),
        (2.41, 0.6, 0.3),
        (2.46, 0.3, 0.3),
        (4.05, 0.38, 0.14),
        (5.72, 0.2, 0.06),
    ];
    const BLOWS: f32 = 17.;
    const HELD: f32 = 1.5;
    const SECS: f32 = 2.4;
    let count = (SECS * RATE) as usize;
    let mut layers = [(); 3].map(|_| vec![0.; count]);
    let keep = MODES.map(|(_, _, ring)| (-1. / (ring * RATE)).exp());
    let mut ringing = [0_f32; MODES.len()];
    let mut knock = Biquad::band_pass(4200., 1.2);
    let (mut next, mut click) = (0.005, 0_f32);
    for n in 0..count {
        let t = n as f32 / RATE;
        if t >= next && t < HELD {
            let weight = rng.within(0.75, 1.);
            for (mode, (_, struck, _)) in ringing.iter_mut().zip(MODES) {
                *mode += weight * struck;
            }
            click += weight;
            next += 1. / (BLOWS * rng.within(0.96, 1.04));
        }
        // The hammer's knock on the gong is over at once.
        click *= (-1. / (0.0006 * RATE)).exp();
        layers[0][n] += 0.5 * knock.run(rng.noise()) * click;
        for (i, (ratio, _, _)) in MODES.into_iter().enumerate() {
            ringing[i] *= keep[i];
            let x = ringing[i] * (TAU * NOTE * ratio * t).sin();
            layers[usize::from(ratio < 2.)][n] += x;
        }
    }
    layers.map(hall)
}

/// A torpedo bursting against her side under water. The sea carries the blast to her
/// as one heavy shove with no crack in it, felt through her frames as much as heard,
/// and her plating answers it with a low clang. The column it threw up comes down over
/// her a second later, a heavy rain of water on her deck that lasts a second and more.
/// The scope's magnification ring clicking into a detent. A sprung steel ball rides up
/// the side of its notch in the brass ring and drops into the next: a faint scuff, then a
/// short, high tick from the ball and its spring over a softer knock from the ring and
/// the tube it turns on. All of it is gone within a few hundredths of a second.
fn detent(rng: &mut Rng) -> [Vec<f32>; 3] {
    /// The ball and spring, then the ring and tube: each tone, how loud it is, and the
    /// seconds it rings.
    const TICK: [(f32, f32, f32); 3] = [
        (3150., 1., 0.006),
        (5400., 0.55, 0.0035),
        (7900., 0.3, 0.002),
    ];
    const KNOCK: [(f32, f32, f32); 3] =
        [(540., 0.5, 0.016), (1180., 1., 0.012), (1950., 0.6, 0.008)];
    /// Seconds the ball rides up before it drops, and the seconds rendered.
    const RIDE: f32 = 0.009;
    const SECS: f32 = 0.11;
    let count = (SECS * RATE) as usize;
    let mut layers = [(); 3].map(|_| vec![0.; count]);
    let mut scuff = Biquad::band_pass(2600., 1.2);
    let mut snap = Biquad::band_pass(4300., 0.9);
    let tune = rng.within(0.96, 1.04);
    let weight = rng.within(0.85, 1.);
    for n in 0..count {
        let t = n as f32 / RATE;
        let grain = rng.noise();
        if t < RIDE {
            // The ball scuffing up the ramp, louder as it climbs.
            layers[0][n] = 0.06 * t / RIDE * scuff.run(grain);
            continue;
        }
        let s = t - RIDE;
        let mut tick = 0.8 * snap.run(grain) * (-s / 0.0007).exp();
        for (hz, loud, ring) in TICK {
            tick += weight * loud * (TAU * hz * tune * s).sin() * (-s / ring).exp();
        }
        layers[0][n] = tick;
        for (hz, loud, ring) in KNOCK {
            layers[1][n] += weight * loud * (TAU * hz * tune * s).sin() * (-s / ring).exp();
        }
    }
    layers
}

/// One beat of the bridge clock. A clock's escapement lets its wheel go a tooth at a
/// time: the pallet's snap, the brass of the movement ringing high for a few
/// milliseconds, and the case and the bulkhead it is screwed to answering with a short,
/// dry knock. The tooth lands on the far pallet a few milliseconds later, softer. No two
/// pallets are quite alike, so the tock is a little lower and duller than the tick.
fn tick(rng: &mut Rng, tock: bool) -> [Vec<f32>; 3] {
    /// The movement's brass: each tone, how loud it is, and the seconds it rings.
    const RING: [(f32, f32, f32); 3] = [
        (2650., 1., 0.009),
        (4480., 0.6, 0.005),
        (6900., 0.35, 0.003),
    ];
    /// The case and the bulkhead behind it.
    const KNOCK: [(f32, f32, f32); 2] = [(820., 1., 0.014), (1640., 0.5, 0.009)];
    /// Seconds from the snap to the tooth landing, and how loud the landing is.
    const LANDING: (f32, f32) = (0.0045, 0.45);
    const SECS: f32 = 0.09;
    let count = (SECS * RATE) as usize;
    let mut layers = [(); 3].map(|_| vec![0.; count]);
    let mut snap = Biquad::band_pass(3800., 0.8);
    let (tune, bright) = if tock { (0.86, 0.75) } else { (1., 1.) };
    let tune = tune * rng.within(0.99, 1.01);
    for n in 0..count {
        let t = n as f32 / RATE;
        let (mut burst, mut ring, mut knock) = (0., 0., 0.);
        for (at, weight) in [(0., 1.), LANDING] {
            let s = t - at;
            if s < 0. {
                continue;
            }
            burst += weight * (-s / 0.0006).exp();
            for (hz, loud, decay) in RING {
                ring += weight * bright * loud * (TAU * hz * tune * s).sin() * (-s / decay).exp();
            }
            for (hz, loud, decay) in KNOCK {
                knock += weight * loud * (TAU * hz * tune * s).sin() * (-s / decay).exp();
            }
        }
        layers[0][n] = 0.8 * burst * snap.run(rng.noise()) + ring;
        layers[1][n] = knock;
    }
    layers
}

fn torpedoed(rng: &mut Rng) -> [Vec<f32>; 3] {
    const SECS: f32 = 3.6;
    /// The plating's modes: hertz, loudness and the seconds each rings.
    const CLANG: [(f32, f32, f32); 4] = [
        (118., 0.6, 0.5),
        (187., 0.45, 0.35),
        (263., 0.3, 0.22),
        (431., 0.2, 0.12),
    ];
    let down = rng.within(0.9, 1.15);
    let rain =
        |t: f32| ((t - down) / 0.25).clamp(0., 1.) * (-(t - down - 0.25).max(0.) / 0.6).exp();
    let mut hiss = Biquad::high_pass(2400., 0.7);
    let mut spray = render(SECS, |t| {
        let up = 0.25 * (t / 0.02).min(1.) * (-t / 0.3).exp();
        hiss.run(rng.noise()) * (up + 0.6 * rain(t))
    });
    scatter(
        &mut spray,
        220,
        (down, down + 1.6),
        (0.003, 0.012),
        rng,
        rain,
        |_, colour, t| (TAU * (900. + 3500. * colour) * t * (1. + 10. * t)).sin(),
    );
    // The water smothers the blow, and her plating rings low under it.
    let blow = boom(0.03, 0.55, rng);
    let mut smother = Biquad::low_pass(520., 0.7);
    let mut fall = Biquad::band_pass(900., 0.5);
    let body = render(SECS, |t| {
        let shove = smother.run(blow.get((t * RATE) as usize).copied().unwrap_or(0.));
        let ring: f32 = (CLANG.iter())
            .map(|&(hz, loud, ring)| loud * (TAU * hz * t).sin() * (-t / ring).exp())
            .sum();
        1.6 * shove + 0.7 * (t / 0.004).min(1.) * ring + 0.5 * fall.run(rng.noise()) * rain(t)
    });
    let mut deep = Biquad::low_pass(70., 0.7);
    let mut heave = rumble(SECS, 150., rng);
    for (n, x) in heave.iter_mut().enumerate() {
        let t = n as f32 / RATE;
        let shove = (t / 0.006).min(1.) * (-t / 0.35).exp();
        *x = 0.6 * *x + 0.9 * blast(t, 0.05) + 5. * deep.run(rng.noise()) * shove;
    }
    [spray, body, heave]
}

/// Ring struck metal into `layer` from `at` seconds: each tone's hertz, level and the
/// seconds it rings, raised by `pitch` and scaled by `weight`. All but `held` of the
/// blow is gone in the first few milliseconds, taken up by whatever holds the metal.
fn strike(
    layer: &mut [f32],
    at: f32,
    tones: &[(f32, f32, f32)],
    (weight, pitch, held): (f32, f32, f32),
    rng: &mut Rng,
) {
    let start = ((at * RATE) as usize).min(layer.len());
    for &(hz, level, ring) in tones {
        let turn = TAU * rng.unit();
        let span = (6. * ring.max(0.004) * RATE) as usize;
        for (n, x) in layer[start..].iter_mut().take(span).enumerate() {
            let t = n as f32 / RATE;
            let fade = (1. - held) * (-t / 0.004).exp() + held * (-t / ring).exp();
            *x += weight * level * (TAU * pitch * hz * t + turn).sin() * fade;
        }
    }
}

/// The tones of `tones` that sound at or above `hz` once raised by `pitch`, and those below.
fn parted(tones: &[(f32, f32, f32)], pitch: f32, hz: f32) -> [Vec<(f32, f32, f32)>; 2] {
    let (high, low) = tones.iter().partition(|tone| tone.0 * pitch >= hz);
    [high, low]
}

/// Filtered noise added to `layer` from `at` seconds for `secs`, shaped by `envelope`
/// of the seconds since it began.
fn breath(
    layer: &mut [f32],
    at: f32,
    secs: f32,
    filters: &mut [Biquad],
    rng: &mut Rng,
    envelope: impl Fn(f32) -> f32,
) {
    let start = ((at * RATE) as usize).min(layer.len());
    for (n, x) in layer[start..]
        .iter_mut()
        .take((secs * RATE) as usize)
        .enumerate()
    {
        let mut air = rng.noise();
        for filter in filters.iter_mut() {
            air = filter.run(air);
        }
        *x += air * envelope(n as f32 / RATE);
    }
}

/// An engine hurt but still running. A recording of a heavy steel part struck in a
/// machine clanks at 1730 and 2310 Hz over a thump between 60 and 250 Hz, down 10 dB
/// in 75 ms and 20 dB in 180 ms. A recording of a machine breaking down clatters on for
/// two seconds and more, a blow every 50 to 600 ms, with its sound spread evenly from
/// 60 Hz to 3 kHz, and metal rubbing on metal is a rasp strongest from 1 to 5 kHz.
/// Here the engine room clangs as the hit goes home, whatever came loose is struck by
/// the running gear once a turn for a second or so, grinding as it goes, and air
/// hisses from a broken pipe.
fn engine_hurt(rng: &mut Rng) -> [Vec<f32>; 3] {
    const SECS: f32 = 2.6;
    /// What rings when the hit goes home: hertz, level and the seconds it rings.
    const CLANG: [(f32, f32, f32); 7] = [
        (96., 0.8, 0.25),
        (151., 0.6, 0.2),
        (238., 0.55, 0.15),
        (412., 0.4, 0.1),
        (690., 0.35, 0.08),
        (1730., 1., 0.075),
        (2310., 0.45, 0.06),
    ];
    /// What came loose, as the running gear strikes it.
    const LOOSE: [(f32, f32, f32); 4] = [
        (480., 0.8, 0.02),
        (820., 1., 0.016),
        (1310., 0.7, 0.012),
        (2050., 0.4, 0.008),
    ];
    /// Turns of the crank a second, the seconds over which the knocking and the
    /// grinding die down as the loose part is thrown clear or jams, and the stick and
    /// slip of the rubbing in jerks a second.
    const TURN: f32 = 5.2;
    const EASE: f32 = 1.;
    const JERKS: f32 = 130.;
    let count = (SECS * RATE) as usize;
    let mut layers = [(); 3].map(|_| vec![0.; count]);
    let pitch = rng.within(0.9, 1.1);
    let [high, low] = parted(&CLANG, pitch, 1000.);
    strike(&mut layers[0], 0., &high, (0.5, pitch, 0.6), rng);
    strike(&mut layers[1], 0., &low, (1., pitch, 0.6), rng);
    let mut deep = Biquad::low_pass(110., 0.7);
    for (n, x) in layers[2].iter_mut().enumerate() {
        let t = n as f32 / RATE;
        *x = blast(t, 0.012) + 3. * deep.run(rng.noise()) * (t / 0.003).min(1.) * (-t / 0.12).exp();
    }
    // The running gear comes round on the loose part, a little early or late each turn.
    let (mut at, turn) = (rng.within(0.1, 0.18), 1. / (TURN * rng.within(0.9, 1.1)));
    let mut thud = Biquad::low_pass(160., 0.7);
    while at < 1.8 {
        let weight = rng.within(0.4, 1.) * (-at / EASE).exp();
        strike(&mut layers[1], at, &LOOSE, (0.8 * weight, pitch, 0.5), rng);
        breath(
            &mut layers[2],
            at,
            0.08,
            std::slice::from_mut(&mut thud),
            rng,
            |t| 1.5 * weight * (-t / 0.015).exp(),
        );
        at += turn * rng.within(0.85, 1.15);
    }
    // It rasps hardest where the turn drags it across, and catches and slips as it goes.
    let mut rasp = [Biquad::high_pass(900., 0.7), Biquad::low_pass(4500., 0.7)];
    let mut chatter = Biquad::band_pass(1100. * pitch, 4.);
    let (mut slip, mut since, sway) = (0_f32, 0_f32, rng.unit());
    for n in 0..count {
        let t = n as f32 / RATE;
        let swell = (t / 0.05).min(1.) * (-t / EASE).exp();
        let drag = 0.3 + 0.7 * (TAU * (sway + t / turn)).sin().max(0.).powi(3);
        let mut grit = rng.noise();
        for filter in &mut rasp {
            grit = filter.run(grit);
        }
        since += JERKS * rng.within(0.6, 1.4) / RATE;
        if since >= 1. {
            since -= 1.;
            slip = rng.within(0.3, 1.);
        }
        let jerk = slip * (-since * 14.).exp();
        layers[0][n] += 2.6 * grit * swell * drag;
        layers[1][n] += 1.6 * chatter.run(jerk) * swell * drag;
    }
    // Air or steam lets go from a fractured pipe, and is shut off.
    let mut pipe = [Biquad::high_pass(2500., 0.7)];
    let hiss = rng.within(0.15, 0.3);
    breath(&mut layers[0], 0.01, 1.2, &mut pipe, rng, |t| {
        0.5 * (t / 0.01).min(1.) * (-t / hiss).exp()
    });
    layers
}

/// An engine giving out. Something in it lets go with a clank and a rattle, and with
/// no fuel burning its gearing coasts down under its own whine until the crank rocks
/// against the last compression and stops with a clunk. A recording of a diesel failing
/// takes two and a half seconds to die once it begins to go. Then the air in her
/// charges bleeds out of it: a recording of air let go from a receiver is a hiss
/// strongest above 2 kHz that sinks 8 to 10 dB a second.
fn engine_stall(rng: &mut Rng) -> [Vec<f32>; 3] {
    const SECS: f32 = 4.6;
    /// The part letting go: hertz, level and the seconds each tone rings.
    const LETS_GO: [(f32, f32, f32); 5] = [
        (140., 0.6, 0.12),
        (330., 0.7, 0.08),
        (760., 0.8, 0.05),
        (1520., 1., 0.04),
        (2380., 0.5, 0.03),
    ];
    /// The crank coming to rest against the frames.
    const SEIZE: [(f32, f32, f32); 4] = [
        (58., 1., 0.18),
        (97., 0.7, 0.14),
        (186., 0.5, 0.1),
        (305., 0.35, 0.07),
    ];
    /// The gears' mesh at the revolutions she was making, and how they coast: the
    /// seconds the speed falls by while the air holds them back, and the share of the
    /// starting speed that friction takes off as well. That stops them in 2.6 s.
    const WHINE: f32 = 720.;
    const COAST: (f32, f32) = (1.5, 0.22);
    let count = (SECS * RATE) as usize;
    let mut layers = [(); 3].map(|_| vec![0.; count]);
    let pitch = rng.within(0.9, 1.12);
    let lag = rng.within(1.3, 1.7);
    let speed = |t: f32| ((1. + COAST.1) * (-t / lag).exp() - COAST.1).max(0.);
    let stop = lag * ((1. + COAST.1) / COAST.1).ln();
    // It lets go, and what broke rattles about for a moment.
    let [high, low] = parted(&LETS_GO, pitch, 1000.);
    strike(&mut layers[0], 0., &high, (0.7, pitch, 0.5), rng);
    strike(&mut layers[1], 0., &low, (0.7, pitch, 0.5), rng);
    let mut at = 0.;
    for _ in 0..3 + (rng.unit() * 3.) as usize {
        at += rng.within(0.03, 0.08);
        let weight = rng.within(0.2, 0.5);
        strike(
            &mut layers[1],
            at,
            &low,
            (weight, pitch * rng.within(1., 1.3), 0.3),
            rng,
        );
    }
    // The last of the fuel coughs in the cylinders, ever further apart.
    let mut puff = [Biquad::high_pass(90., 0.7), Biquad::low_pass(450., 0.7)];
    let mut at = rng.within(0.3, 0.5);
    for cough in 0..2 + (rng.unit() * 2.) as usize {
        let weight = rng.within(0.5, 0.9) / (1. + cough as f32);
        breath(&mut layers[1], at, 0.2, &mut puff, rng, |t| {
            1.6 * weight * (t / 0.004).min(1.) * (-t / 0.035).exp()
        });
        breath(&mut layers[2], at, 0.2, &mut puff, rng, |t| {
            weight * (t / 0.004).min(1.) * (-t / 0.05).exp()
        });
        at += rng.within(0.35, 0.6) * (1. + 0.6 * cough as f32);
    }
    // The gearing whines down, rough in its teeth and rattling in its lash at the end.
    let (mut turn, mut rough, churn) = (rng.unit(), 0_f32, pole(30.));
    for n in 0..count {
        let t = n as f32 / RATE;
        let going = speed(t);
        turn = (turn + WHINE * pitch * going / RATE).fract();
        rough += churn * (rng.noise() - rough);
        let teeth = (TAU * turn).sin() + 0.45 * (2. * TAU * turn).sin();
        let loud = going * (t / 0.2).min(1.) * (1. + 12. * rough);
        layers[1][n] += 0.9 * teeth * loud;
        layers[0][n] += 0.35 * (3. * TAU * turn).sin() * loud;
    }
    let lash = (stop - 0.6, stop - 0.05);
    scatter(
        &mut layers[0],
        14,
        lash,
        (0.002, 0.005),
        rng,
        |at| 0.3 * ((at - lash.0) / (lash.1 - lash.0)).clamp(0., 1.),
        |_, colour, t| (TAU * (1800. + 1600. * colour) * t).sin(),
    );
    // The crank rocks back against compression and comes to rest.
    strike(&mut layers[1], stop, &SEIZE, (1., pitch, 0.7), rng);
    let back = stop + rng.within(0.11, 0.16);
    strike(&mut layers[1], back, &SEIZE, (0.35, pitch, 0.7), rng);
    let mut deep = Biquad::low_pass(120., 0.7);
    let thump = (stop * RATE) as usize;
    for (n, x) in layers[2].iter_mut().skip(thump).enumerate() {
        let t = n as f32 / RATE;
        *x += 0.8 * blast(t, 0.018)
            + 2. * deep.run(rng.noise()) * (t / 0.004).min(1.) * (-t / 0.1).exp();
    }
    // Air sighs out of it, brightest at first.
    let mut air = [Biquad::high_pass(1800., 0.7), Biquad::high_pass(1800., 0.7)];
    let sigh = stop - 0.1;
    breath(&mut layers[0], sigh, SECS - sigh, &mut air, rng, |t| {
        0.45 * (t / 0.08).min(1.) * (-t / 0.9).exp()
    });
    layers
}

/// The repair party knocking a fitting true with a hand hammer. Recordings of a hammer
/// tapping steel plate have a blow every quarter of a second or so, in runs. Each falls
/// 15 dB in its first hundredth of a second as the hammer's face and the hand holding
/// the work take it up, then rings on 0.07 s at the plate's own tones: 585, 1170, 1650,
/// 2020, 2935, 3130, 3915 and 4160 Hz, loudest near 2 and 3 kHz. A heavier fitting
/// answers lower and dies sooner, between 220 and 700 Hz and at 1350 Hz, nearly gone in
/// 25 ms. Now and then the hammer bounces and strikes again, lighter.
fn hammer(rng: &mut Rng) -> [Vec<f32>; 3] {
    const PLATE: [(f32, f32, f32); 8] = [
        (585., 0.41, 0.09),
        (1170., 0.56, 0.08),
        (1650., 0.56, 0.07),
        (2020., 1., 0.07),
        (2935., 0.84, 0.06),
        (3130., 0.7, 0.05),
        (3915., 0.47, 0.04),
        (4160., 0.38, 0.035),
    ];
    const FITTING: [(f32, f32, f32); 7] = [
        (217., 0.75, 0.03),
        (300., 0.8, 0.03),
        (390., 0.85, 0.025),
        (525., 1., 0.025),
        (595., 0.75, 0.02),
        (705., 0.6, 0.02),
        (1355., 0.7, 0.015),
    ];
    let tones: &[(f32, f32, f32)] = if rng.unit() < 0.6 { &PLATE } else { &FITTING };
    let pitch = rng.within(0.85, 1.18);
    let every = rng.within(0.2, 0.3);
    let mut blows = Vec::new();
    let mut at = 0.01;
    for _ in 0..2 + (rng.unit() * 5.) as usize {
        let weight = rng.within(0.65, 1.);
        blows.push((at, weight));
        if rng.unit() < 0.2 {
            blows.push((at + rng.within(0.055, 0.08), 0.35 * weight));
        }
        at += every * rng.within(0.85, 1.15);
    }
    let secs = at + 0.4;
    let mut layers = [(); 3].map(|_| vec![0.; (secs * RATE) as usize]);
    let [high, low] = parted(tones, pitch, 1400.);
    let mut face = [Biquad::high_pass(2000., 0.7)];
    let mut head = [Biquad::band_pass(350., 1.)];
    for (at, weight) in blows {
        let pitch = pitch * rng.within(0.99, 1.01);
        strike(&mut layers[0], at, &high, (weight, pitch, 0.18), rng);
        strike(&mut layers[1], at, &low, (weight, pitch, 0.18), rng);
        breath(&mut layers[0], at, 0.01, &mut face, rng, |t| {
            0.7 * weight * (-t / 0.0006).exp()
        });
        breath(&mut layers[1], at, 0.04, &mut head, rng, |t| {
            2. * weight * (-t / 0.005).exp()
        });
    }
    layers
}

/// The repair party's cutting torch. Recordings of a lit torch are a roar within a few
/// dB from 250 Hz to 4 kHz that falls away below 200 Hz and above 5 kHz, as steady as a
/// flame is, within 2 or 3 dB. Molten steel spits from the cut in small crackles, and a
/// torch lit again goes off with a pop 10 to 20 dB over its flame.
fn torch(rng: &mut Rng) -> [Vec<f32>; 3] {
    let secs = rng.within(1.5, 3.);
    let (open, close) = (rng.within(0.08, 0.18), rng.within(0.2, 0.35));
    let swell = move |t: f32| (t / open).min(1.) * ((secs - t) / close).clamp(0., 1.);
    let count = (secs * RATE) as usize;
    let mut layers = [(); 3].map(|_| vec![0.; count]);
    let mut flame = [Biquad::high_pass(220., 0.7), Biquad::low_pass(2000., 0.7)];
    let mut jet = [Biquad::high_pass(2000., 0.7), Biquad::low_pass(6000., 0.7)];
    let (mut flicker, shiver) = (0_f32, pole(8.));
    for n in 0..count {
        let t = n as f32 / RATE;
        flicker += shiver * (rng.noise() - flicker);
        let level = swell(t) * (1. + 10. * flicker);
        let gas = rng.noise();
        let (mut low, mut high) = (gas, gas);
        for filter in &mut flame {
            low = filter.run(low);
        }
        for filter in &mut jet {
            high = filter.run(high);
        }
        layers[1][n] = low * level;
        layers[0][n] = high * level;
    }
    let spits = (secs * rng.within(3., 8.)) as usize;
    scatter(
        &mut layers[0],
        spits,
        (0.1, secs - 0.1),
        (0.0004, 0.0015),
        rng,
        |at| 1.2 * swell(at),
        |rng, _, _| rng.noise(),
    );
    if rng.unit() < 0.4 {
        let mut pop = [Biquad::high_pass(100., 0.7), Biquad::low_pass(900., 0.7)];
        breath(&mut layers[1], 0., 0.2, &mut pop, rng, |t| {
            1.6 * blast(t, 0.004) + 2.5 * (-t / 0.025).exp()
        });
    }
    layers
}

/// The repair party's spanner. Recordings of a ratchet being worked have strokes of 0.1
/// to 0.25 s, 0.15 to 0.5 s apart, and in each stroke the pawl snaps over the teeth 15 to
/// 90 times a second, quickest in the middle of the swing. Each snap is over in a
/// millisecond or two and rings near 6.2 and 10 kHz, with less near 3 and 4.4 kHz from
/// the handle and the socket. Across a deck the air takes the highest of it, and what
/// is left to hear is mostly the handle. Now and then the spanner is put down on the
/// steel with a clank, which a recording of a light metal clank has at 1730 and 2310 Hz.
fn ratchet(rng: &mut Rng) -> [Vec<f32>; 3] {
    const PAWL: [(f32, f32, f32); 4] = [
        (6200., 1., 0.0012),
        (9950., 0.75, 0.0008),
        (3100., 0.6, 0.002),
        (4400., 0.45, 0.0015),
    ];
    const CLANK: [(f32, f32, f32); 4] = [
        (870., 0.25, 0.05),
        (1730., 1., 0.07),
        (2310., 0.45, 0.05),
        (3400., 0.3, 0.03),
    ];
    let top = rng.within(45., 95.);
    let pitch = rng.within(0.92, 1.08);
    let mut snaps = Vec::new();
    let mut at = 0.02;
    for _ in 0..1 + (rng.unit() * 4.) as usize {
        let stroke = rng.within(0.1, 0.25);
        let (mut along, mut tooth) = (0_f32, 0_f32);
        while along < stroke {
            along += 1. / RATE * 16.;
            tooth += top * (0.3 + 0.7 * (PI * along / stroke).sin()) / RATE * 16.;
            if tooth >= 1. {
                tooth -= 1.;
                snaps.push((at + along, rng.within(0.6, 1.)));
            }
        }
        at += stroke + rng.within(0.15, 0.45);
    }
    let clank = rng.unit() < 0.4;
    let secs = at + if clank { 0.5 } else { 0.1 };
    let mut layers = [(); 3].map(|_| vec![0.; (secs * RATE) as usize]);
    let [high, low] = parted(&PAWL, pitch, 5000.);
    let mut tick = [Biquad::high_pass(4000., 0.7)];
    for (at, weight) in snaps {
        strike(&mut layers[0], at, &high, (weight, pitch, 1.), rng);
        strike(&mut layers[1], at, &low, (weight, pitch, 1.), rng);
        breath(&mut layers[0], at, 0.004, &mut tick, rng, |t| {
            0.8 * weight * (-t / 0.0003).exp()
        });
    }
    if clank {
        let pitch = pitch * rng.within(0.9, 1.1);
        strike(&mut layers[1], at - 0.05, &CLANK, (1.4, pitch, 0.5), rng);
    }
    layers
}

/// Add `part` into `layer` from its start, scaled so that its own peak is `weight`.
fn blend(layer: &mut [f32], part: &[f32], weight: f32) {
    let peak = part.iter().fold(1e-9_f32, |peak, x| peak.max(x.abs()));
    for (x, y) in layer.iter_mut().zip(part) {
        *x += weight * y / peak;
    }
}

/// Steel straining or folding. It sticks and slips `jerks` times a second, faster while
/// the strain is greatest, and each slip sets her frames ringing low. The strain comes
/// on at once and eases over `secs`.
fn groan(secs: f32, pitch: f32, jerks: f32, rng: &mut Rng) -> Vec<f32> {
    /// What rings, how loud and how sharply.
    const FRAMES: [(f32, f32, f32); 3] = [(96., 1., 10.), (151., 0.8, 12.), (247., 0.5, 14.)];
    let mut ring = FRAMES.map(|(hz, _, sharp)| Biquad::band_pass(pitch * hz, sharp));
    let (mut turn, mut slip, mut rate) = (0_f32, 0_f32, jerks);
    render(secs, |t| {
        let strain = (t / 0.12).min(1.) * (-t * 3. / secs).exp();
        turn += rate / RATE;
        if turn >= 1. {
            turn -= 1.;
            slip = rng.within(0.4, 1.);
            rate = jerks * rng.within(0.75, 1.25) * (0.5 + 0.5 * strain);
        }
        let jerk = slip * strain * (-turn * 10.).exp();
        (ring.iter_mut().zip(FRAMES))
            .map(|(ring, (_, loud, _))| loud * ring.run(jerk))
            .sum()
    })
}

/// A light round glancing off plate far thicker than it needs. The round is on the
/// plate for a fraction of a millisecond, which reaches only its high modes, and plate
/// that thick and that well backed damps them within a few hundredths of a second. The
/// round goes on tumbling, and whirrs as it turns over and falls away.
fn tap(rng: &mut Rng) -> [Vec<f32>; 3] {
    const SECS: f32 = 0.5;
    /// The plate's high modes: hertz, level and the seconds each rings.
    const RING: [(f32, f32, f32); 5] = [
        (1870., 0.6, 0.03),
        (2640., 1., 0.024),
        (3710., 0.7, 0.018),
        (5230., 0.45, 0.012),
        (7100., 0.3, 0.008),
    ];
    /// What answers in the frame behind the plate.
    const KNOCK: [(f32, f32, f32); 2] = [(640., 1., 0.014), (1010., 0.5, 0.01)];
    let mut layers = [(); 3].map(|_| vec![0.; (SECS * RATE) as usize]);
    let pitch = rng.within(0.93, 1.07);
    strike(&mut layers[0], 0., &RING, (1., pitch, 0.6), rng);
    strike(&mut layers[1], 0., &KNOCK, (1., pitch, 0.7), rng);
    let mut click = [Biquad::high_pass(3500., 0.7)];
    breath(&mut layers[0], 0., 0.01, &mut click, rng, |t| {
        1.4 * (-t / 0.0012).exp()
    });
    // It turns over a few dozen times a second, and its whirr falls as it goes.
    let (spin, off) = (rng.within(26., 38.), rng.within(0.015, 0.03));
    let mut whirr = Biquad::band_pass(1500. * pitch, 2.5);
    for (n, x) in layers[0].iter_mut().enumerate().skip((off * RATE) as usize) {
        let t = n as f32 / RATE - off;
        whirr.retune(Biquad::band_pass(1500. * pitch / (1. + 1.5 * t), 2.5));
        let turning = 0.35 + 0.65 * (TAU * spin * t).sin().powi(2);
        *x += 2.2 * whirr.run(rng.noise()) * turning * (t / 0.02).min(1.) * (-t / 0.11).exp();
    }
    let mut dull = Biquad::low_pass(240., 0.7);
    for (n, x) in layers[2].iter_mut().enumerate().take((0.2 * RATE) as usize) {
        let t = n as f32 / RATE;
        *x = 0.4 * blast(t, 0.0015)
            + 3. * dull.run(rng.noise()) * (t / 0.001).min(1.) * (-t / 0.018).exp();
    }
    layers
}

/// A round punching through plate too thin to stop it. Steel that fractures gives way
/// at once, in a crack sharper than any blow that only bends it, and the torn edges
/// part in a crackle of small breaks for a few hundredths of a second while the body of
/// the round scrapes through. A holed plate hardly rings, and the steel shoved in
/// behind it thumps low.
fn pierce(rng: &mut Rng) -> [Vec<f32>; 3] {
    const SECS: f32 = 0.6;
    /// The torn edges' brief ring, and the holed plate's dull one: hertz, level and
    /// the seconds each rings.
    const EDGES: [(f32, f32, f32); 3] =
        [(2280., 1., 0.012), (3150., 0.8, 0.009), (4400., 0.5, 0.006)];
    const HOLED: [(f32, f32, f32); 4] = [
        (330., 1., 0.035),
        (520., 0.8, 0.028),
        (845., 0.6, 0.02),
        (1290., 0.4, 0.014),
    ];
    let count = (SECS * RATE) as usize;
    let mut layers = [(); 3].map(|_| vec![0.; count]);
    let pitch = rng.within(0.92, 1.08);
    let mut hiss = Biquad::high_pass(2500., 0.7);
    let crack = render(0.03, |t| {
        blast(t, 0.0003) + 0.8 * hiss.run(rng.noise()) * (-t / 0.0025).exp()
    });
    let mut edges = vec![0.; count];
    strike(&mut edges, 0.001, &EDGES, (1., pitch, 0.5), rng);
    // Many small breaks, thick at first and thinning out.
    let tear = rng.within(0.035, 0.06);
    let mut torn = vec![0.; count];
    scatter(
        &mut torn,
        140,
        (0.001, 2.5 * tear),
        (0.0003, 0.0012),
        rng,
        |at| (-at / tear).exp(),
        |rng, _, _| rng.noise(),
    );
    blend(&mut layers[0], &crack, 1.);
    blend(&mut layers[0], &edges, 0.35);
    blend(&mut layers[0], &torn, 0.7);
    let mut holed = vec![0.; count];
    strike(&mut holed, 0., &HOLED, (1., pitch, 0.5), rng);
    let mut scrape = vec![0.; count];
    let mut band = [Biquad::band_pass(1300. * pitch, 0.9)];
    breath(&mut scrape, 0.002, 0.25, &mut band, rng, |t| {
        (t / 0.004).min(1.) * (-t / 0.045).exp()
    });
    blend(&mut layers[1], &holed, 1.);
    blend(&mut layers[1], &scrape, 0.6);
    let mut shove = Biquad::low_pass(190., 0.7);
    for (n, x) in layers[2].iter_mut().enumerate() {
        let t = n as f32 / RATE;
        *x = 0.8 * blast(t, 0.005)
            + 4. * shove.run(rng.noise()) * (t / 0.002).min(1.) * (-t / 0.035).exp();
    }
    layers
}

/// A round bursting inside her a beat after it went in. Heard from outside the
/// compartment, the steel round it keeps the highs in, so the burst is a boom with
/// little above a few hundred hertz and the box itself drumming under it. What gets out
/// is what the burst does: frames buckling in a crunch for a third of a second,
/// splinters rattling off the bulkheads, and a puff of flame and smoke out of the hole
/// the round made.
fn inner_blast(rng: &mut Rng) -> [Vec<f32>; 3] {
    const SECS: f32 = 2.;
    /// Seconds from the round going in to its burst.
    const FUZE: f32 = 0.025;
    /// The compartment's lowest modes: hertz, level and the seconds each drums.
    const BOX: [(f32, f32, f32); 3] = [(37., 1., 0.22), (58., 0.75, 0.17), (86., 0.5, 0.12)];
    let fuze = FUZE * rng.within(0.8, 1.3);
    let count = (SECS * RATE) as usize;
    let at = (fuze * RATE) as usize;
    let mut layers = [(); 3].map(|_| vec![0.; count]);
    let mut smother = [Biquad::low_pass(560., 0.7), Biquad::low_pass(560., 0.7)];
    let smothered: Vec<f32> = (boom(0.009, 0.3, rng).into_iter())
        .map(|x| smother.iter_mut().fold(x, |x, pass| pass.run(x)))
        .collect();
    let crunch = rng.within(0.25, 0.4);
    let mut buckling = vec![0.; count];
    scatter(
        &mut buckling,
        40,
        (fuze + 0.01, fuze + crunch),
        (0.004, 0.015),
        rng,
        |at| (-(at - fuze) / (0.5 * crunch)).exp(),
        |rng, colour, t| (TAU * (160. + 1100. * colour) * t).sin() * (0.6 + 0.4 * rng.noise()),
    );
    blend(&mut layers[1][at..], &smothered, 1.);
    blend(&mut layers[1], &buckling, 0.55);
    let mut splinters = vec![0.; count];
    scatter(
        &mut splinters,
        45,
        (fuze + 0.004, fuze + 0.3),
        (0.002, 0.007),
        rng,
        |at| (-(at - fuze) / 0.1).exp(),
        |_, colour, t| (TAU * (1100. + 1800. * colour) * t).sin(),
    );
    // What comes out of the hole is not smothered.
    let mut puff = vec![0.; count];
    let mut vent = [Biquad::high_pass(900., 0.7), Biquad::low_pass(5000., 0.7)];
    breath(&mut puff, fuze, 0.2, &mut vent, rng, |t| {
        (t / 0.002).min(1.) * (-t / 0.03).exp()
    });
    blend(&mut layers[0], &splinters, 0.6);
    blend(&mut layers[0], &puff, 1.);
    let mut drum = vec![0.; count];
    strike(&mut drum, fuze, &BOX, (1., rng.within(0.9, 1.1), 0.9), rng);
    let mut deep = Biquad::low_pass(95., 0.7);
    let mut shove = vec![0.; count];
    for (n, x) in shove.iter_mut().enumerate().skip(at) {
        let t = n as f32 / RATE - fuze;
        *x = 0.8 * blast(t, 0.03)
            + 6. * deep.run(rng.noise()) * (t / 0.004).min(1.) * (-t / 0.2).exp();
    }
    blend(&mut layers[2], &shove, 1.);
    blend(&mut layers[2], &drum, 0.6);
    layers
}

/// A section of her giving way. Its frames fold in a long groan, steel sticking and
/// slipping as it does in a seaway but lower, harder and all at once. Gear inside breaks
/// loose and crashes down, the heaviest not always first, and the sea roars in through
/// the holes and swells as it fills her.
fn gutted(rng: &mut Rng) -> [Vec<f32>; 3] {
    const SECS: f32 = 3.4;
    /// Heavy gear striking the deck: hertz, level and the seconds each rings.
    const CRASH: [(f32, f32, f32); 5] = [
        (88., 1., 0.3),
        (142., 0.8, 0.22),
        (233., 0.6, 0.15),
        (390., 0.45, 0.1),
        (655., 0.3, 0.06),
    ];
    let count = (SECS * RATE) as usize;
    let mut layers = [(); 3].map(|_| vec![0.; count]);
    let pitch = rng.within(0.88, 1.12);
    let start = (rng.within(0.08, 0.15) * RATE) as usize;
    let folding = groan(rng.within(1.6, 2.2), pitch, rng.within(18., 30.), rng);
    blend(&mut layers[1][start..], &folding, 0.8);
    let (mut fallen, mut thuds) = (vec![0.; count], vec![0.; count]);
    let mut high = vec![0.; count];
    let mut dull = [Biquad::low_pass(200., 0.7)];
    for _ in 0..3 + (rng.unit() * 3.) as usize {
        let at = rng.within(0.15, 1.6);
        let weight = rng.within(0.5, 1.);
        let tune = pitch * rng.within(0.85, 1.15);
        let [bright, low] = parted(&CRASH, tune, 300.);
        strike(&mut fallen, at, &low, (weight, tune, 0.6), rng);
        strike(&mut high, at, &bright, (weight, tune, 0.5), rng);
        breath(&mut thuds, at, 0.3, &mut dull, rng, |t| {
            weight * (t / 0.003).min(1.) * (-t / 0.05).exp()
        });
    }
    blend(&mut layers[1], &fallen, 1.);
    blend(&mut layers[0], &high, 0.6);
    // The sea pours in, swelling over half a second and surging as it fills her.
    let pour = rng.within(0.2, 0.35);
    let flow = |t: f32| ((t - pour) / 0.6).clamp(0., 1.) * (-(t - pour - 0.6).max(0.) / 1.4).exp();
    let mut sea = [Biquad::high_pass(220., 0.7), Biquad::low_pass(1300., 0.6)];
    let mut hiss = Biquad::high_pass(2500., 0.7);
    let (mut swirl, sway) = (0_f32, pole(3.));
    let (mut rush, mut spray) = (vec![0.; count], vec![0.; count]);
    for n in 0..count {
        let t = n as f32 / RATE;
        swirl += sway * (rng.noise() - swirl);
        let surge = (1. + 25. * swirl).clamp(0.4, 1.6);
        let water = sea.iter_mut().fold(rng.noise(), |x, pass| pass.run(x));
        rush[n] = water * flow(t) * surge;
        spray[n] = hiss.run(rng.noise()) * flow(t);
    }
    blend(&mut layers[1], &rush, 0.45);
    blend(&mut layers[0], &spray, 0.25);
    let mut deep = Biquad::low_pass(75., 0.7);
    let mut weight = vec![0.; count];
    for (n, x) in weight.iter_mut().enumerate() {
        let t = n as f32 / RATE;
        let heave = 0.8 * (t / 0.01).min(1.) * (-t / 0.6).exp() + 0.3 * flow(t);
        *x = 0.6 * blast(t, 0.04) + 5. * deep.run(rng.noise()) * heave;
    }
    blend(&mut layers[2], &weight, 1.);
    blend(&mut layers[2], &thuds, 0.7);
    layers
}

/// Her frames taking a heavy blow, carried to the bridge by the ship. It has the shape
/// of her own main battery's recoil, one blow and the hull flexing under it, but it is
/// heavier, the loose gear about the bridge rattles for longer, and her frames groan as
/// she settles. It comes a moment after the round went in, when it burst.
fn shudder(rng: &mut Rng) -> [Vec<f32>; 3] {
    const LEAD: f32 = 0.03;
    let count = (1.6 * RATE) as usize;
    let at = (LEAD * RATE) as usize;
    let mut layers = [(); 3].map(|_| vec![0.; count]);
    let [rattle, thud, roll] = recoil(rng, 1.5);
    blend(&mut layers[0][at..], &rattle, 1.);
    blend(&mut layers[1][at..], &thud, 1.);
    blend(&mut layers[2][at..], &roll, 1.);
    let settling = groan(0.9, rng.within(0.85, 1.05), rng.within(24., 34.), rng);
    blend(
        &mut layers[1][at + (0.12 * RATE) as usize..],
        &settling,
        0.45,
    );
    layers
}
