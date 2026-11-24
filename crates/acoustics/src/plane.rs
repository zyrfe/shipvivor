//! An aeroplane: a radial engine and the propeller it turns.
//!
//! The engine is the kind a naval floatplane of the 1930s carried: nine cylinders in
//! a ring, each firing once in two turns of the crank, and a two-bladed propeller on
//! the end of that crank. At 1900 turns a minute a blade passes 63 times a second,
//! which is the note of the drone, and a cylinder fires 142 times a second, which is
//! the bark of the exhaust stubs over it. No two cylinders are quite alike, so the
//! exhaust has a pattern that comes round every two turns and gives a radial its throb.
//!
//! Recordings of biplanes flying past put the blade note between 48 and 66 Hz, with
//! its first two tones the strongest and the rest some 10 dB under them. Far off,
//! four fifths of what is heard is those tones. Close to, most of it is the rush of
//! the air the blades tear and the exhaust.
//!
//! It is heard down a [`Line`], from where it was when the sound left it. So it is
//! heard late, sharp while it closes and flat once it has passed.
use crate::{
    Air, RATE,
    filter::{Biquad, Rng},
    line::Line,
    voice::Ears,
};
use glam::Vec3;
use std::f32::consts::TAU;

/// Turns of the crank a second.
const TURNS: f32 = 31.7;
const BLADES: f32 = 2.;
const CYLINDERS: usize = 9;
/// No two aeroplanes run at quite the same speed. This is how far one may differ,
/// how quickly its governor's hunting wanders as the share of the way it moves towards
/// a new random value each sample, and how far that carries the revolutions.
const TRIM: f32 = 0.03;
const HUNT: (f32, f32) = (3e-5, 1.5);
/// Strength of each tone of the blade note.
const BLADE: [f32; 10] = [0.9, 1., 0.45, 0.4, 0.3, 0.3, 0.2, 0.15, 0.1, 0.08];
/// How far one cylinder's exhaust may differ from another's, and one firing from the
/// last.
const UNEVEN: f32 = 0.5;
const RAGGED: f32 = 0.15;
/// The exhaust: its level against the blade note, the hertz below and above which the
/// stubs let little out, the seconds a puff takes to clear them, and how much of a
/// puff is the push of the gas, the rest being its hiss.
const EXHAUST: (f32, f32, f32, f32, f32) = (3.5, 90., 900., 0.0025, 0.6);
/// The air the blades tear: its level, where it is centred in hertz, and how deeply
/// it beats with their passing.
const TORN: (f32, f32, f32) = (4.5, 500., 0.6);
/// Level close to.
const LEVEL: f32 = 0.3;
/// Metres past which the engine is not worth making, and the seconds it takes to
/// come in and to die away at that range or when it is cut.
const REACH: f32 = 900.;
const OPENING: f32 = 1.5;
/// Seconds for the place the mixer has it to settle on where the game says it is.
/// Between words from the game it is carried on at its speed.
const HOLD: f32 = 0.3;

pub(crate) struct Plane {
    /// The last word from the game, and the seconds since it.
    position: Vec3,
    velocity: Vec3,
    since: f32,
    flying: bool,
    /// Where it is now, as near as the mixer can tell.
    here: Vec3,
    /// How far the engine has come in, from 0 to 1.
    power: f32,
    /// Turns of the crank since a cylinder last began its two, and the cylinder that
    /// fired last.
    turn: f32,
    fired: usize,
    trim: f32,
    hunt: f32,
    /// How hard each cylinder fires, in the order they fire.
    fires: [f32; CYLINDERS],
    puff: f32,
    left: f32,
    stubs: [Biquad; 2],
    torn: Biquad,
    rng: Rng,
    line: Line,
}
impl Plane {
    /// Aeroplanes with different seeds are different machines of the same make.
    pub fn new(seed: u32) -> Self {
        let mut rng = Rng::new(seed);
        Self {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            since: 0.,
            flying: false,
            here: Vec3::ZERO,
            power: 0.,
            turn: 2. * rng.unit(),
            fired: 0,
            trim: 1. + TRIM * rng.noise(),
            hunt: 0.,
            fires: [(); CYLINDERS].map(|()| 1. + UNEVEN * rng.noise()),
            puff: 0.,
            left: (-1. / (EXHAUST.3 * RATE)).exp(),
            stubs: [
                Biquad::high_pass(EXHAUST.1, 0.7),
                Biquad::low_pass(EXHAUST.2, 0.6),
            ],
            torn: Biquad::band_pass(TORN.1, 0.5),
            line: Line::new(seed ^ 0xA1F7),
            rng,
        }
    }

    /// Word from the game: where it is, how it is moving and whether its engine runs.
    pub fn track(&mut self, position: Vec3, velocity: Vec3, flying: bool) {
        if !self.flying && !self.sounding() {
            // One that has been silent is taken up where it now is.
            self.here = position;
        }
        (self.position, self.velocity, self.since) = (position, velocity, 0.);
        self.flying = flying;
    }

    /// Cut the engine where it is.
    pub fn cut(&mut self) {
        self.flying = false;
    }

    /// Its engine is heard, or will be when what it made has crossed the water.
    pub fn sounding(&self) -> bool {
        self.power > 0. || !self.line.spent()
    }

    /// Gone, with nothing left in the air.
    pub fn clear(&mut self) {
        self.line.clear();
        (self.flying, self.power, self.puff) = (false, 0., 0.);
    }

    /// Fly on through one block and add what the listener hears of it.
    pub fn sing(&mut self, ears: &Ears, air: &Air, out: &mut [[f32; 2]], send: &mut [[f32; 2]]) {
        let frames = out.len();
        let secs = frames as f32 / RATE;
        self.since += secs;
        // The game speaks sixty times a second and never quite on the beat. Carried
        // on at its own speed and only eased towards each word, the aeroplane moves
        // smoothly, and its note does not warble.
        let said = self.position + self.velocity * self.since;
        self.here += self.velocity * secs;
        self.here += (said - self.here) * (1. - (-secs / HOLD).exp());
        let apart = self.here.distance(ears.position);
        let wanted = f32::from(self.flying && apart < REACH);
        let was = self.power;
        self.power += (wanted - was).clamp(-secs / OPENING, secs / OPENING);
        if was == 0. && self.power == 0. {
            if self.line.spent() {
                // Out of earshot or cut, with nothing left to arrive. When it is next
                // heard it is heard afresh.
                return self.line.renew();
            }
            self.line.rest(frames);
        } else {
            self.run(frames, was);
        }
        // What is heard now left it some while back, from where it then was.
        let mut then = self.here;
        for _ in 0..2 {
            then = self.here - self.velocity * (then.distance(ears.position) / air.speed);
        }
        self.line.cross(then, ears, air, out, send);
    }

    /// Run the engine through one block and keep what it puts out.
    fn run(&mut self, frames: usize, was: f32) {
        // The blade note at full strength comes to one, as near as matters.
        let whole = BLADE.iter().map(|w| w * w).sum::<f32>().sqrt();
        let rate = TURNS * self.trim / RATE;
        let now = self.power;
        let (turn, fired, hunt, puff) = (
            &mut self.turn,
            &mut self.fired,
            &mut self.hunt,
            &mut self.puff,
        );
        let (fires, left, rng) = (self.fires, self.left, &mut self.rng);
        let (stubs, torn) = (&mut self.stubs, &mut self.torn);
        self.line.keep(frames, |n| {
            *hunt += HUNT.0 * (rng.noise() - *hunt);
            *turn += rate * (1. + HUNT.1 * *hunt);
            if *turn >= 2. {
                *turn -= 2.;
            }
            // Each tone of the blades is found from the two below it.
            let (sin, cos) = (TAU * BLADES * *turn).sin_cos();
            let (mut under, mut tone, mut blades) = (0., sin, 0.);
            for weight in BLADE {
                blades += weight * tone;
                (under, tone) = (tone, 2. * cos * tone - under);
            }
            // Nine cylinders fire in turn through two turns of the crank.
            let firing = ((*turn * 0.5 * CYLINDERS as f32) as usize).min(CYLINDERS - 1);
            if firing != *fired {
                *fired = firing;
                *puff += fires[firing] * (1. + RAGGED * rng.noise());
            }
            *puff *= left;
            let gas = *puff * (EXHAUST.4 + (1. - EXHAUST.4) * rng.noise());
            let gas = stubs[0].run(gas);
            let gas = stubs[1].run(gas);
            let rush = torn.run(rng.noise()) * (1. + TORN.2 * sin);
            let power = was + (now - was) * n as f32 / frames as f32;
            LEVEL * power * (blades / whole + EXHAUST.0 * gas + TORN.0 * rush)
        });
    }
}
