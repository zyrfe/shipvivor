//! A base's siren, heard across the water.
//!
//! A siren is a fan that chops its own air. A motor spins a drum with ports cut in it
//! inside a casing with ports to match, and each time they line up a puff gets out.
//! The note is the number of ports times the turns a second, so the pitch is the speed
//! of the rotor and nothing else. This one has two rows, of ten ports and twelve, which
//! sound a minor third apart.
//!
//! The wail is the motor being switched on and off. Under power the rotor gains speed
//! steadily until the motor has no more to give. With the power off, the air it throws
//! slows it: quickly at first and ever more slowly. Those two rates, the strength of the
//! overtones and how the level follows the speed were all measured from recordings of
//! sirens winding up and running down.
//!
//! What the siren puts out is kept for as long as sound takes to cross the water, and
//! the listener hears the part that left when it should have. So a siren is heard to
//! start late, its pitch leans with the ship's own way towards it or from it, and it
//! wails on for some seconds after the power to it has gone.
use crate::{
    Air, RATE, Rotor,
    filter::Rng,
    voice::{Crossing, Ears, Heard},
};
use glam::Vec3;
use std::f32::consts::TAU;

/// One make of siren.
#[derive(Clone, Copy)]
struct Make {
    /// Ports in the two rows of the rotor.
    ports: [f32; 2],
    /// Turns a second at full speed.
    full: f32,
    /// Share of full speed gained each second under power.
    spin: f32,
    /// Share of its speed the rotor loses each second at full speed with the power
    /// off. That is the load of the fan, and it falls with the square of the speed.
    drag: f32,
    /// Level at full speed, close to.
    level: f32,
}
impl Rotor {
    fn make(self) -> Make {
        match self {
            // 2850 turns a minute, which sounds 475 and 570 Hz. Its rates are the
            // measured ones.
            Self::Shore => Make {
                ports: [10., 12.],
                full: 47.5,
                spin: 0.27,
                drag: 0.115,
                level: 0.6,
            },
            // 1800 turns a minute, which sounds 240 and 300 Hz. No recording of one
            // was measured: it is the shore siren with a heavier drum, which is slower
            // both to gather speed and to lose it.
            Self::Ship => Make {
                ports: [8., 10.],
                full: 30.,
                spin: 0.19,
                drag: 0.08,
                level: 0.75,
            },
        }
    }
}
/// No two sirens run at quite the same speed. This is how far one may differ.
const TRIM: f32 = 0.02;
/// Seconds the motor takes to ease onto full speed at the last.
const EASE: f32 = 0.2;
/// Share of full speed the bearings take each second whatever the speed, which is what
/// stops a rotor in the end.
const BEARINGS: f32 = 0.012;
/// The wail: seconds the motor is left on once the rotor is up to speed, and seconds
/// it is then left off.
const WAIL: (f32, f32) = (1., 4.);
const UP_TO_SPEED: f32 = 0.97;
/// Strength of each tone's overtones against the tone itself.
const OVERTONES: [f32; 6] = [1., 0.63, 0.4, 0.2, 0.14, 0.16];
/// The power of the speed by which the level falls as the rotor slows.
const SWELL: f32 = 1.6;
/// Samples of output kept, a power of two. Five and a half seconds, which in the
/// game's air is 928 m. A siren further off than that is heard a little early.
const KEPT: usize = 1 << 18;
/// Wind and uneven air make a distant steady sound come and go. How deep that runs
/// far off, the metres at which it is half as deep, and how many times a second.
const FADING: (f32, f32, f32) = (0.3, 400., 0.6);

pub(crate) struct Siren {
    position: Vec3,
    /// The alarm is up, and the motor has power.
    raised: bool,
    powered: bool,
    /// Seconds of the present spell: at speed under power, or with the power off.
    spell: f32,
    make: Make,
    /// Speed as a share of full, how far round the rotor is in turns, and how far this
    /// siren's full speed is from its make's.
    speed: f32,
    angle: f32,
    trim: f32,
    level: f32,
    /// What it has put out, and where the next sample goes.
    kept: Box<[f32]>,
    head: usize,
    /// Samples since it last put out anything.
    quiet: usize,
    heard: Option<Heard>,
    crossing: Crossing,
    /// How the air is carrying it, and the gain that gave at the end of the last block.
    wander: f32,
    faded: f32,
    rng: Rng,
}
impl Siren {
    /// Sirens with different seeds are different sirens of the same make.
    pub fn new(seed: u32) -> Self {
        let mut rng = Rng::new(seed);
        Self {
            position: Vec3::ZERO,
            raised: false,
            powered: false,
            spell: 0.,
            make: Rotor::Shore.make(),
            speed: 0.,
            angle: rng.unit(),
            trim: 1. + TRIM * rng.noise(),
            level: 0.,
            kept: vec![0.; KEPT].into_boxed_slice(),
            head: 0,
            quiet: KEPT,
            heard: None,
            crossing: Crossing::new(),
            wander: 0.,
            faded: 1.,
            rng,
        }
    }

    /// Raise or lower the alarm of a siren of this make, where it now stands. One
    /// aboard a ship moves with her.
    pub fn order(&mut self, rotor: Rotor, position: Vec3, raised: bool) {
        // One that has been silent is heard afresh, wherever it and the ears now are.
        // A rotor that is turning cannot be swapped for another.
        if self.spent() {
            (self.heard, self.make) = (None, rotor.make());
        }
        if raised && !self.raised {
            (self.powered, self.spell) = (true, 0.);
        }
        (self.position, self.raised) = (position, raised);
    }

    /// Lower the alarm where it stands.
    pub fn lower(&mut self) {
        self.raised = false;
    }

    /// Nothing is turning and nothing it made is still on its way.
    pub fn spent(&self) -> bool {
        !self.raised && self.speed == 0. && self.quiet >= KEPT
    }

    /// Stop dead, with nothing left in the air.
    pub fn clear(&mut self) {
        if self.quiet < KEPT {
            self.kept.fill(0.);
        }
        (self.raised, self.powered, self.spell) = (false, false, 0.);
        (self.speed, self.level, self.quiet) = (0., 0., KEPT);
        (self.heard, self.crossing) = (None, Crossing::new());
    }

    /// Turn the rotor through one block and add what the listener hears of it.
    pub fn sing(&mut self, ears: &Ears, air: &Air, out: &mut [[f32; 2]], send: &mut [[f32; 2]]) {
        let frames = out.len();
        self.turn(frames);
        let next = ears.hear(self.position, 0., air);
        let from = self.heard.replace(next).unwrap_or(next);
        // Far off, the air lets it through in waves.
        let depth = FADING.0 * next.distance / (next.distance + FADING.1);
        let slow = 1. - (-TAU * FADING.2 * frames as f32 / RATE).exp();
        self.wander += slow * (self.rng.noise() - self.wander);
        // Slowed that much, noise that filled -1..1 keeps this much of its spread.
        let spread = (slow / (3. * (2. - slow))).sqrt();
        let (was, now) = (
            self.faded,
            1. + depth * (self.wander / spread).clamp(-2., 2.),
        );
        self.faded = now;
        // The four samples about the one wanted must all have been made and kept.
        let reach = |heard: &Heard| {
            let furthest = (KEPT - frames - 8) as f32 - heard.bounce;
            f64::from(air.delay(heard.distance).clamp(2., furthest))
        };
        let (left, arrives) = (reach(&from), reach(&next));
        for (n, (frame, send)) in out.iter_mut().zip(send).enumerate() {
            let k = n as f32 / frames as f32;
            let glide = |a: f32, b: f32| a + (b - a) * k;
            let at = (self.head + n) as f64 - (left + (arrives - left) * f64::from(k));
            let back = at - f64::from(glide(from.bounce, next.bounce));
            let gain = glide(from.gains[1], next.gains[1]) * glide(was, now);
            self.crossing.carry(
                (&from, &next, k),
                gain * self.read(at),
                gain * self.read(back),
                frame,
                send,
            );
        }
        self.head = (self.head + frames) % KEPT;
    }

    /// Work the switch, spin the rotor and keep what it puts out.
    fn turn(&mut self, frames: usize) {
        let secs = frames as f32 / RATE;
        if !self.raised {
            self.powered = false;
        } else if self.powered {
            // The spell of power is counted from when the rotor is up to speed, so
            // the first one, from rest, is as long as it needs to be.
            if self.speed >= UP_TO_SPEED {
                self.spell += secs;
            }
            if self.spell >= WAIL.0 {
                (self.powered, self.spell) = (false, 0.);
            }
        } else {
            self.spell += secs;
            if self.spell >= WAIL.1 {
                (self.powered, self.spell) = (true, 0.);
            }
        }
        if self.speed == 0. && !self.powered {
            // At rest it makes nothing, but what it made is still crossing.
            for n in 0..frames.min(KEPT - self.quiet.min(KEPT)) {
                self.kept[(self.head + n) % KEPT] = 0.;
            }
            self.quiet = (self.quiet + frames).min(KEPT);
            return;
        }
        self.quiet = 0;
        // Both rows together at full speed come to one, as near as matters.
        let whole = OVERTONES.iter().map(|w| w * w).sum::<f32>().sqrt();
        let was = self.level;
        for n in 0..frames {
            let change = if self.powered {
                self.make.spin.min((1. - self.speed) / EASE)
            } else {
                -(self.make.drag * self.speed * self.speed + BEARINGS)
            };
            self.speed = (self.speed + change / RATE).clamp(0., 1.);
            self.angle = (self.angle + self.make.full * self.trim * self.speed / RATE).fract();
            if n == 0 {
                self.level = self.make.level / whole * self.speed.powf(SWELL);
            }
            // Each overtone is found from the two below it, which costs one sine and
            // one cosine a row.
            let mut puff = 0.;
            for ports in self.make.ports {
                let (sin, cos) = (TAU * ports * self.angle).sin_cos();
                let (mut under, mut tone) = (0., sin);
                for weight in OVERTONES {
                    puff += weight * tone;
                    (under, tone) = (tone, 2. * cos * tone - under);
                }
            }
            let level = was + (self.level - was) * n as f32 / frames as f32;
            self.kept[(self.head + n) % KEPT] = level * puff;
        }
    }

    /// What was kept at a place between two samples, by a curve through four.
    fn read(&self, at: f64) -> f32 {
        let whole = at.floor();
        let (i, t) = (whole as isize, (at - whole) as f32);
        let [a, b, c, d] =
            [-1, 0, 1, 2].map(|k| self.kept[(i + k).rem_euclid(KEPT as isize) as usize]);
        let c1 = 0.5 * (c - a);
        let c2 = a - 2.5 * b + 2. * c - 0.5 * d;
        let c3 = 0.5 * (d - a) + 1.5 * (b - c);
        ((c3 * t + c2) * t + c1) * t + b
    }
}
