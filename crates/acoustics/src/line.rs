//! A sound that goes on and on from one place, heard across the water.
//!
//! What its source puts out is kept for as long as sound takes to cross the water,
//! and the listener hears the part that left when it should have. So such a sound is
//! heard to start late and to stop late, and its pitch leans with whatever way the
//! source and the listener have towards each other or apart.
use crate::{
    Air, RATE,
    filter::Rng,
    voice::{Crossing, Ears, Heard},
};
use glam::Vec3;
use std::f32::consts::TAU;

/// Samples of output kept, a power of two. Five and a half seconds, which in the
/// game's air is 928 m. A source further off than that is heard a little early.
pub(crate) const KEPT: usize = 1 << 18;
/// Wind and uneven air make a distant steady sound come and go. How deep that runs
/// far off, the metres at which it is half as deep, and how many times a second.
const FADING: (f32, f32, f32) = (0.3, 400., 0.6);
/// Seconds for the place it is heard from to settle on where its source is said to
/// be, which hides the game's frame steps. A step in the distance would be a step in
/// the time of arrival, and a tone cannot take that without warbling.
const SETTLE: f32 = 0.06;

pub(crate) struct Line {
    /// What the source has put out, and where the next sample goes.
    kept: Box<[f32]>,
    head: usize,
    /// Samples since it last put out anything.
    quiet: usize,
    /// Where it is heard from, and how it was heard at the end of the last block.
    stands: Vec3,
    heard: Option<Heard>,
    crossing: Crossing,
    /// How the air is carrying it, and the gain that gave at the end of the last block.
    wander: f32,
    faded: f32,
    rng: Rng,
}
impl Line {
    pub fn new(seed: u32) -> Self {
        Self {
            kept: vec![0.; KEPT].into_boxed_slice(),
            head: 0,
            quiet: KEPT,
            stands: Vec3::ZERO,
            heard: None,
            crossing: Crossing::new(),
            wander: 0.,
            faded: 1.,
            rng: Rng::new(seed),
        }
    }

    /// Nothing the source made is still on its way.
    pub fn spent(&self) -> bool {
        self.quiet >= KEPT
    }

    /// Hear it afresh, wherever it and the ears now are.
    pub fn renew(&mut self) {
        self.heard = None;
    }

    /// Leave nothing in the air.
    pub fn clear(&mut self) {
        if self.quiet < KEPT {
            self.kept.fill(0.);
        }
        (self.quiet, self.heard, self.crossing) = (KEPT, None, Crossing::new());
    }

    /// Keep the next frames of what the source puts out. `make` is asked for each in
    /// turn.
    pub fn keep(&mut self, frames: usize, mut make: impl FnMut(usize) -> f32) {
        self.quiet = 0;
        for n in 0..frames {
            self.kept[(self.head + n) % KEPT] = make(n);
        }
    }

    /// Keep the next frames as silence. What was made before is still crossing.
    pub fn rest(&mut self, frames: usize) {
        for n in 0..frames.min(KEPT - self.quiet.min(KEPT)) {
            self.kept[(self.head + n) % KEPT] = 0.;
        }
        self.quiet = (self.quiet + frames).min(KEPT);
    }

    /// Add what the listener hears of it over one block, with its source at
    /// `position`, and move on to the next block.
    pub fn cross(
        &mut self,
        position: Vec3,
        ears: &Ears,
        air: &Air,
        out: &mut [[f32; 2]],
        send: &mut [[f32; 2]],
    ) {
        let frames = out.len();
        let pull = 1. - (-(frames as f32) / (SETTLE * RATE)).exp();
        self.stands = match self.heard {
            Some(_) => self.stands + (position - self.stands) * pull,
            None => position,
        };
        let next = ears.hear(self.stands, 0., air);
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
