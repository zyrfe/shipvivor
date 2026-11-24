//! Filters and noise shared by the foundry, the hull and the mixer.
use crate::RATE;
use std::f32::consts::TAU;

/// Per-sample coefficient of a one-pole low-pass.
pub(crate) fn pole(hz: f32) -> f32 {
    1. - (-TAU * hz / RATE).exp()
}

/// Second-order section in transposed direct form II.
#[derive(Clone, Copy, Default)]
pub(crate) struct Biquad {
    b: [f32; 3],
    a: [f32; 2],
    z: [f32; 2],
}
impl Biquad {
    fn design(hz: f32, q: f32, shape: impl Fn(f32, f32) -> [f32; 3]) -> Self {
        let (sin, cos) = (TAU * hz.min(RATE * 0.45) / RATE).sin_cos();
        let alpha = sin / (2. * q);
        let a0 = 1. + alpha;
        Self {
            b: shape(cos, alpha).map(|b| b / a0),
            a: [-2. * cos / a0, (1. - alpha) / a0],
            z: [0.; 2],
        }
    }
    pub fn low_pass(hz: f32, q: f32) -> Self {
        Self::design(hz, q, |cos, _| [(1. - cos) / 2., 1. - cos, (1. - cos) / 2.])
    }
    pub fn high_pass(hz: f32, q: f32) -> Self {
        Self::design(hz, q, |cos, _| {
            [(1. + cos) / 2., -(1. + cos), (1. + cos) / 2.]
        })
    }
    pub fn band_pass(hz: f32, q: f32) -> Self {
        Self::design(hz, q, |_, alpha| [alpha, 0., -alpha])
    }

    /// Take a new tuning without disturbing the signal already inside.
    pub fn retune(&mut self, tuned: Self) {
        (self.b, self.a) = (tuned.b, tuned.a);
    }

    pub fn run(&mut self, x: f32) -> f32 {
        let y = self.b[0] * x + self.z[0];
        self.z[0] = self.b[1] * x - self.a[0] * y + self.z[1];
        self.z[1] = self.b[2] * x - self.a[1] * y;
        y
    }
}

/// A small fast generator. The same seed always gives the same noise.
pub(crate) struct Rng(u32);
impl Rng {
    pub fn new(seed: u32) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9) | 1)
    }
    /// Uniform in 0..1.
    pub fn unit(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / (1 << 24) as f32
    }
    pub fn within(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.unit()
    }
    /// White noise in -1..1.
    pub fn noise(&mut self) -> f32 {
        self.within(-1., 1.)
    }
}
