//! One sound on its way to the listener, and the ears that hear it.
use crate::{
    Air, NEAR, RATE,
    filter::pole,
    foundry::{Cast, STRETCH},
};
use glam::{Vec3, Vec3Swizzles};
use std::f32::consts::FRAC_PI_4;

/// Seconds for the ears to settle on a new position, which hides the game's frame steps.
const EAR_LAG: f32 = 0.06;
/// How far a sound from abeam leans to its own side by level alone.
const LEAN: f32 = 0.75;
/// Samples for sound to cross half a head, which sets how late the far ear hears.
/// Heads are not scaled with the sea, so this uses the real speed of sound.
const HEAD: f32 = 0.0875 / 343. * RATE;
/// Frequency the head hides from the far ear when the sound is square on the beam.
const SHADOW: f32 = 2200.;
/// Share of the highs lost by a sound from dead astern.
const ASTERN: f32 = 0.45;
/// Share of a sound that comes back off the sea, and the frequency above which the
/// chop would scatter it if it struck square on. A sound that only grazes the water
/// comes back whole to much higher up, so a far gun is coloured by its copy and a
/// near one only gains weight.
const SKIM: f32 = 0.6;
const CHOP: f32 = 160.;
/// Share of a sound sent on to the tail when it is made alongside and when it is far off.
/// A distant report is mostly its own roll.
const SEND: (f32, f32) = (0.25, 1.);
/// Samples of a voice's own past kept for the late ear. A power of two above the
/// longest wait.
const PAST: usize = 64;

#[derive(Clone, Copy)]
pub(crate) struct Ears {
    pub position: Vec3,
    right: Vec3,
    wanted: (Vec3, Vec3),
    pub placed: bool,
}
impl Default for Ears {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            right: Vec3::X,
            wanted: (Vec3::ZERO, Vec3::X),
            placed: false,
        }
    }
}
impl Ears {
    pub fn turn(&mut self, position: Vec3, right: Vec3) {
        self.wanted = (position, right.normalize_or(Vec3::X));
        if !self.placed {
            (self.position, self.right) = self.wanted;
            self.placed = true;
        }
    }

    pub fn settle(&mut self, frames: usize) {
        let pull = 1. - (-(frames as f32) / (EAR_LAG * RATE)).exp();
        self.position += (self.wanted.0 - self.position) * pull;
        self.right = (self.right + (self.wanted.1 - self.right) * pull).normalize_or(self.wanted.1);
    }

    /// How a sound leaving `source` reaches the ears. An echo has already come `lead`
    /// metres by the time it leaves.
    pub fn hear(&self, source: Vec3, lead: f32, air: &Air) -> Heard {
        let offset = source - self.position;
        let reach = offset.length();
        let distance = lead + reach;
        // A source close alongside is in both ears, and no source is entirely in one.
        let abeam = offset.dot(self.right) / (reach + 4.);
        let astern = (-offset.dot(Vec3::Y.cross(self.right)) / (reach + 4.)).max(0.);
        let (starboard, port) = ((LEAN * abeam + 1.) * FRAC_PI_4).sin_cos();
        // The far ear hears late by the way round the head, and without the highs.
        let late = HEAD * (abeam.abs().asin() + abeam.abs());
        let shade = 1. - (1. - pole(SHADOW)) * abeam.abs();
        let (lag, shade) = if abeam > 0. {
            ([late, 0.], [shade, 1.])
        } else {
            ([0., late], [1., shade])
        };
        // The sea is a mirror: the second path runs to the source's image below it.
        let drop = self.position.y.max(0.) + source.y.max(0.);
        let mirrored = (offset.xz().length_squared() + drop * drop).sqrt();
        let bounce = (mirrored - reach).max(0.);
        let spread = distance.max(NEAR) / (distance + bounce).max(NEAR);
        let square = (drop / mirrored.max(1e-3)).clamp(0.02, 1.);
        Heard {
            distance,
            gains: air.gains(distance),
            // Two matched poles reach -3 dB at 0.644 of their corner frequency.
            pole: pole(air.ceiling(distance) / 0.644 * (1. - ASTERN * astern)),
            pan: [port, starboard],
            lag,
            shade,
            bounce: air.delay(bounce),
            skim: SKIM * spread.powf(air.rolloff),
            chop: pole(CHOP / square),
            send: SEND.0 + (SEND.1 - SEND.0) * distance / (distance + 150.),
        }
    }
}

/// How one voice reaches the ears at one moment.
#[derive(Clone, Copy)]
pub(crate) struct Heard {
    /// Metres the sound has come.
    pub distance: f32,
    pub gains: [f32; 3],
    pub pole: f32,
    pub pan: [f32; 2],
    /// Samples each ear waits, and the pole that dulls it.
    lag: [f32; 2],
    shade: [f32; 2],
    /// Samples the copy off the sea runs behind, and its level.
    pub bounce: f32,
    skim: f32,
    chop: f32,
    pub send: f32,
}
impl Heard {
    /// Carried by the ship: at once, in both ears alike, with nothing answering.
    pub const ABOARD: Self = Self {
        distance: 0.,
        gains: [1.; 3],
        pole: 1.,
        pan: [std::f32::consts::FRAC_1_SQRT_2; 2],
        lag: [0.; 2],
        shade: [1.; 2],
        bounce: 0.,
        skim: 0.,
        chop: 1.,
        send: 0.,
    };
}

/// What the crossing has done to a sound so far: the air's dulling, the sea's copy
/// and the wait round the head. Whatever makes the sound, this is how it is heard.
pub(crate) struct Crossing {
    lows: [f32; 2],
    chop: f32,
    shade: [f32; 2],
    past: [f32; PAST],
    cursor: usize,
}
impl Crossing {
    pub const fn new() -> Self {
        Self {
            lows: [0.; 2],
            chop: 0.,
            shade: [0.; 2],
            past: [0.; PAST],
            cursor: 0,
        }
    }

    /// Carry one sample into the two ears. `x` came straight and `off_sea` by way of
    /// the water, and both are heard `k` of the way from `from` to `next`. `send`
    /// takes the share that goes on to the tail.
    #[inline]
    pub fn carry(
        &mut self,
        (from, next, k): (&Heard, &Heard, f32),
        mut x: f32,
        off_sea: f32,
        frame: &mut [f32; 2],
        send: &mut [f32; 2],
    ) {
        let glide = |a: f32, b: f32| a + (b - a) * k;
        self.chop += glide(from.chop, next.chop) * (off_sea - self.chop);
        x += glide(from.skim, next.skim) * self.chop;
        let pole = glide(from.pole, next.pole);
        self.lows[0] += pole * (x - self.lows[0]);
        self.lows[1] += pole * (self.lows[0] - self.lows[1]);
        self.past[self.cursor] = self.lows[1];
        let share = glide(from.send, next.send);
        for ear in 0..2 {
            let lag = glide(from.lag[ear], next.lag[ear]);
            let whole = lag as usize;
            let tap = |back: usize| self.past[self.cursor.wrapping_sub(back) % PAST];
            let late = tap(whole) + (tap(whole + 1) - tap(whole)) * (lag - whole as f32);
            let shade = glide(from.shade[ear], next.shade[ear]);
            self.shade[ear] += shade * (late - self.shade[ear]);
            let y = self.shade[ear] * glide(from.pan[ear], next.pan[ear]);
            frame[ear] += y;
            send[ear] += share * y;
        }
        self.cursor = (self.cursor + 1) % PAST;
    }
}

pub(crate) struct Voice {
    cast: usize,
    levels: [f32; 3],
    position: Vec3,
    /// Metres the sound had come before it left `position`. Zero unless it is an echo.
    lead: f32,
    aboard: bool,
    /// Samples since the sound was made.
    age: f64,
    speed: f64,
    heard: Heard,
    crossing: Crossing,
}
impl Voice {
    pub fn new(cast: usize, levels: [f32; 3], position: Vec3, speed: f64, heard: Heard) -> Self {
        Self {
            cast,
            levels,
            position,
            lead: 0.,
            aboard: false,
            age: 0.,
            speed,
            heard,
            crossing: Crossing::new(),
        }
    }

    /// The same sound thrown back by land after `lead` metres.
    pub fn echo(mut self, lead: f32) -> Self {
        self.lead = lead;
        self
    }

    /// The same sound carried through the ship.
    pub fn aboard(mut self) -> Self {
        self.aboard = true;
        self
    }

    /// The same sound with its front already at the listener.
    pub fn arriving(mut self, air: &Air) -> Self {
        self.age = f64::from(air.delay(self.heard.distance));
        self
    }

    pub fn cast(&self) -> usize {
        self.cast
    }

    /// Whether the sound is crossing the air, as opposed to being carried by the ship.
    pub fn airborne(&self) -> bool {
        !self.aboard
    }

    /// Samples since the front reached the listener. Negative while it is on its way.
    pub fn spent(&self, air: &Air) -> f64 {
        self.age - f64::from(air.delay(self.heard.distance))
    }

    /// How loud the voice is at the listener now, near enough: each layer at its level
    /// and the air's gain, through the cast's envelope where the sound has got to. One
    /// still on its way counts as loud as it will be when it arrives.
    pub fn loudness(&self, cast: &Cast, air: &Air) -> f32 {
        let at = (self.spent(air) * self.speed).max(0.) as usize / STRETCH;
        (0..3)
            .map(|layer| {
                let envelope = cast.envelope[layer].get(at).copied().unwrap_or(0.);
                self.levels[layer] * self.heard.gains[layer] * envelope
            })
            .sum()
    }

    pub fn hearing(&self, ears: &Ears, air: &Air) -> Heard {
        if self.aboard {
            Heard::ABOARD
        } else {
            ears.hear(self.position, self.lead, air)
        }
    }

    /// Add this voice to the block, gliding from how it was last heard to how it is
    /// heard now. `send` takes its share for the tail. Returns false once the whole
    /// cast has passed the listener.
    pub fn sing(
        &mut self,
        cast: &Cast,
        next: Heard,
        air: &Air,
        out: &mut [[f32; 2]],
        send: &mut [[f32; 2]],
    ) -> bool {
        let frames = out.len();
        let from = self.heard;
        let (was, now) = (
            f64::from(air.delay(from.distance)),
            f64::from(air.delay(next.distance)),
        );
        // Nothing is heard until the front of the sound has crossed the gap.
        if (self.age + frames as f64 - now) * self.speed > -2. {
            let mirrored = from.skim > 0. || next.skim > 0.;
            for (n, (frame, send)) in out.iter_mut().zip(send).enumerate() {
                let k = n as f32 / frames as f32;
                let glide = |a: f32, b: f32| a + (b - a) * k;
                let delay = was + (now - was) * f64::from(k);
                let at = (self.age + n as f64 - delay) * self.speed;
                let back = at - f64::from(glide(from.bounce, next.bounce)) * self.speed;
                let (mut x, mut off_sea) = (0., 0.);
                for layer in 0..3 {
                    let gain = self.levels[layer] * glide(from.gains[layer], next.gains[layer]);
                    x += gain * read(&cast.layers[layer], at);
                    if mirrored {
                        off_sea += gain * read(&cast.layers[layer], back);
                    }
                }
                self.crossing
                    .carry((&from, &next, k), x, off_sea, frame, send);
            }
        }
        self.age += frames as f64;
        self.heard = next;
        // Leave the copy off the sea its time, and the filters a moment to ring out.
        let ends = cast.longest as f64 + 0.02 * f64::from(RATE);
        (self.age - now) * self.speed - f64::from(next.bounce) < ends
    }
}

/// Read a layer at a fractional sample by cubic interpolation. Silence surrounds it.
fn read(layer: &[f32], at: f64) -> f32 {
    let whole = at.floor();
    if whole < -2. || whole > layer.len() as f64 {
        return 0.;
    }
    let (i, t) = (whole as isize, (at - whole) as f32);
    // A tap before the start wraps to an index past the end, which reads as silence too.
    let [a, b, c, d] = [-1, 0, 1, 2].map(|k| layer.get((i + k) as usize).copied().unwrap_or(0.));
    let c1 = 0.5 * (c - a);
    let c2 = a - 2.5 * b + 2. * c - 0.5 * d;
    let c3 = 0.5 * (d - a) + 1.5 * (b - c);
    ((c3 * t + c2) * t + c1) * t + b
}
