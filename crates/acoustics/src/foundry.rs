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
            Sound::ShoreGun => gun(0.012, 0.5, 3.8).cast(rng),
            Sound::Splash => splash(rng),
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
        }
        .map(finish);
        let longest = layers.iter().map(Vec::len).max().unwrap_or(0);
        Self { layers, longest }
    }
}

fn render(secs: f32, mut voice: impl FnMut(f32) -> f32) -> Vec<f32> {
    (0..(secs * RATE) as usize)
        .map(|n| voice(n as f32 / RATE))
        .collect()
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

fn splash(rng: &mut Rng) -> [Vec<f32>; 3] {
    // Spray hisses as the column rises, then droplets patter back as it collapses.
    let mut hiss = Biquad::high_pass(3000., 0.7);
    let mut spray = render(1.5, |t| {
        0.3 * hiss.run(rng.noise()) * (t / 0.15).min(1.) * (-t / 0.35).exp()
    });
    scatter(
        &mut spray,
        110,
        (0.1, 1.3),
        (0.003, 0.009),
        rng,
        |at| (at / 0.55).min(1.) * (-(at - 0.55).max(0.) / 0.3).exp(),
        |_, colour, t| (TAU * (1800. + 4700. * colour) * t * (1. + 20. * t)).sin(),
    );
    let mut slap = Biquad::band_pass(1100., 0.7);
    let mut rush = Biquad::band_pass(500., 1.2);
    let mut n = 0;
    let body = render(1., |t| {
        if n % 32 == 0 {
            rush.retune(Biquad::band_pass(500. + 1700. * (t / 0.45).min(1.), 1.2));
        }
        n += 1;
        let rise = if t < 0.6 {
            (PI * t / 0.6).sin().powi(2)
        } else {
            0.
        };
        slap.run(rng.noise()) * (-t / 0.045).exp() + 0.5 * rush.run(rng.noise()) * rise
    });
    // The shell's own blow on the water, and the hollow it leaves closing.
    let mut deep = Biquad::low_pass(160., 0.7);
    let thud = render(1.2, |t| {
        0.5 * blast(t, 0.012) + (t / 0.003).min(1.) * 6. * deep.run(rng.noise()) * (-t / 0.16).exp()
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
    let wash = |t: f32| 0.35 * ((t - worst) / 0.8).clamp(0., 1.) * (-(t - worst).max(0.) / 6.).exp();

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
    let mut foam = render(SECS, |t| 0.5 * hiss.run(rng.noise()) * wave((t - 0.05).max(0.)));
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
    let (mut click, mut knock) = (
        Biquad::high_pass(2500., 0.7),
        Biquad::band_pass(300., 0.8),
    );
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
