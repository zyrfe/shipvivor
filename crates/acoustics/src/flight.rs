//! A shell in flight, heard from where it was when the sound left it.
//!
//! A shell slower than sound is heard once, a little behind where it is. A shell faster
//! than sound outruns its own noise: nothing is heard until the cone of its shock
//! sweeps over the listener, and then two sounds arrive together. One is the stretch of
//! flight just past, falling away after the shell. The other is the approach, heard
//! backwards and running off towards the gun.
use crate::{
    Air, RATE,
    filter::{Biquad, Rng},
    voice::Ears,
};
use glam::Vec3;
use std::f32::consts::TAU;

/// Seconds without word of a shell before it is taken to have landed.
const STALE: f32 = 0.08;
/// Seconds a landed shell is kept in case its sound has yet to arrive.
const LINGER: f32 = 6.;
/// Level of the rush of a six-inch shell at no distance at all, and the metres at
/// which it has fallen to a quarter. A shell is far quieter than a gun, so only one
/// that passes close is heard.
const RUSH: f32 = 9.;
pub(crate) const CLOSE: f32 = 35.;
/// Centre of that rush in hertz when the shell is neither closing nor going away, and
/// the whistle that rides above it.
const TONE: f32 = 380.;
const WHISTLE: (f32, f32) = (3.1, 0.5);
/// How far closing or receding may shift the pitch.
const SHIFT: (f32, f32) = (0.2, 8.);
/// Beats a second of a shell's wobble about its line of flight, and their depth.
const WOBBLE: (f32, f32) = (11., 0.35);
const FAINT: f32 = 1e-4;

pub(crate) struct Flight {
    pub serial: u64,
    position: Vec3,
    velocity: Vec3,
    weight: f32,
    /// Seconds of flight up to the last word from the game, and seconds since it.
    flown: f32,
    since: f32,
    /// The shell has been seen coming with its sound still behind it, and has since
    /// been heard to pass.
    coming: bool,
    passed: bool,
    rng: Rng,
    wakes: [Wake; 2],
}

/// One of the two stretches of flight that can be heard at once.
#[derive(Clone, Copy)]
struct Wake {
    gain: f32,
    pan: [f32; 2],
    pole: f32,
    send: f32,
    shift: f32,
    rush: Biquad,
    whistle: Biquad,
    lows: f32,
    wobble: f32,
}

/// The shock of a shell's passing, for the mixer to sound.
pub(crate) struct Passing {
    pub serial: u64,
    pub position: Vec3,
    pub weight: f32,
}

impl Flight {
    pub fn new(serial: u64, position: Vec3, velocity: Vec3, weight: f32) -> Self {
        let wake = Wake {
            gain: 0.,
            pan: [0.; 2],
            pole: 1.,
            send: 0.,
            shift: 1.,
            rush: Biquad::band_pass(TONE, 0.6),
            whistle: Biquad::band_pass(TONE * WHISTLE.0, 5.),
            lows: 0.,
            wobble: 0.,
        };
        Self {
            serial,
            position,
            velocity,
            weight,
            flown: 0.,
            since: 0.,
            coming: false,
            passed: false,
            rng: Rng::new(serial as u32 ^ 0x5E11),
            wakes: [wake; 2],
        }
    }

    /// Word from the game that the shell is still in the air.
    pub fn track(&mut self, position: Vec3, velocity: Vec3) {
        (self.position, self.velocity) = (position, velocity);
        (self.flown, self.since) = (self.flown + self.since, 0.);
    }

    /// Add the shell's noise to the block. Returns false once nothing more of it can
    /// arrive, and the shock of its passing on the block that happens.
    pub fn sing(
        &mut self,
        ears: &Ears,
        air: &Air,
        out: &mut [[f32; 2]],
        send: &mut [[f32; 2]],
    ) -> (bool, Option<Passing>) {
        let frames = out.len();
        self.since += frames as f32 / RATE;
        // Past the last word the shell is carried on along its line. If it has in fact
        // landed, only what left it before then can still be on the way.
        let silent = (self.since - STALE).max(0.);
        let made = silent..=self.flown + self.since;
        let here = self.position + self.velocity * self.since;
        let apart = here - ears.position;
        // Sound that left the shell `ago` seconds back has had `speed * ago` metres to
        // cover the gap to where the shell then was.
        let excess = self.velocity.length_squared() - air.speed * air.speed;
        let closing = apart.dot(self.velocity);
        let inside = closing * closing - excess * apart.length_squared();
        // Once a landed shell's last sound has arrived there is nothing to wait for.
        // For a fast shell that is when the approach, heard backwards, reaches the gun.
        let (mut heard, mut over) = ([None; 2], self.since > LINGER);
        if inside >= 0. && excess.abs() > 1. {
            let root = inside.sqrt();
            let ago = [(closing - root) / excess, (closing + root) / excess];
            heard = ago.map(|ago| made.contains(&ago).then_some(ago));
            over |= ago[0] < silent && (excess < 0. || ago[1] > *made.end());
        }
        // Both arriving at once is the cone of the shock crossing the listener.
        let mut passing = None;
        match heard {
            [Some(late), Some(early)] if self.coming && !self.passed => {
                self.passed = true;
                passing = Some(Passing {
                    serial: self.serial,
                    position: here - self.velocity * (0.5 * (late + early)),
                    weight: self.weight,
                });
            }
            [None, None] if silent == 0. => self.coming = true,
            _ => {}
        }

        let tone = TONE / self.weight.max(0.1).sqrt();
        let mut sounding = false;
        for (wake, ago) in self.wakes.iter_mut().zip(heard) {
            let from = *wake;
            wake.gain = 0.;
            if let Some(ago) = ago {
                let left = here - self.velocity * ago;
                let toward = (ears.position - left).normalize_or_zero();
                let squeeze = 1. - self.velocity.dot(toward) / air.speed;
                let reaches = ears.hear(left, 0., air);
                wake.shift = (1. / squeeze.abs().max(1e-3)).clamp(SHIFT.0, SHIFT.1);
                // A raised pitch is a wider band of noise, and is turned down to match.
                // The sharp instant of the shock is the mixer's to sound.
                let near = CLOSE / (CLOSE + air.speed * ago);
                wake.gain = RUSH * self.weight * near * near / wake.shift.sqrt();
                (wake.pan, wake.pole, wake.send) = (reaches.pan, reaches.pole, reaches.send);
                let hz = (tone * wake.shift).clamp(40., 9000.);
                wake.rush.retune(Biquad::band_pass(hz, 0.6));
                wake.whistle
                    .retune(Biquad::band_pass((hz * WHISTLE.0).min(12_000.), 5.));
            }
            if from.gain < FAINT && wake.gain < FAINT {
                continue;
            }
            sounding = true;
            // A wake newly heard has no past to glide from.
            let from = if from.gain < FAINT {
                Wake { gain: 0., ..*wake }
            } else {
                from
            };
            let depth = WOBBLE.1 * self.weight.min(1.);
            for (n, (frame, send)) in out.iter_mut().zip(send.iter_mut()).enumerate() {
                let k = n as f32 / frames as f32;
                let glide = |a: f32, b: f32| a + (b - a) * k;
                let noise = self.rng.noise();
                wake.wobble = (wake.wobble + WOBBLE.0 * wake.shift / RATE).fract();
                let beat = 1. + depth * (TAU * wake.wobble).sin();
                let torn = wake.rush.run(noise) + WHISTLE.1 * wake.whistle.run(noise);
                let x = glide(from.gain, wake.gain) * beat * torn;
                wake.lows += glide(from.pole, wake.pole) * (x - wake.lows);
                let share = glide(from.send, wake.send);
                for ear in 0..2 {
                    let y = wake.lows * glide(from.pan[ear], wake.pan[ear]);
                    frame[ear] += y;
                    send[ear] += share * y;
                }
            }
        }
        (!(over && silent > 0. && !sounding), passing)
    }
}
