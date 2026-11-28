//! A big ship's horn, heard across the water.
//!
//! A ship's air horn is a diaphragm that compressed air sets shaking at the throat of
//! a long flared trumpet. The trumpet's length sets the note, so the bigger the ship,
//! the bigger the horn and the deeper it sounds. The rules of the road say how deep:
//! a fundamental of 70 to 200 Hz for a ship of 200 m and more, 130 to 350 Hz from
//! 75 m and 250 to 700 Hz below that. Across those bands the note falls about as the
//! length to the power 0.62, and this horn follows that law down to 84 Hz for the
//! longest ship afloat here. The rules also make the bigger horns louder.
//!
//! Recordings of three big ships' horns, two cruise ships and a cargo ship, set the
//! rest. The note is steady to a part in a hundred and comes in about 3% sharp, which
//! settles within a third of a second as the air behind the diaphragm steadies. The
//! second tone is as strong as the first or stronger, the third about as strong, and
//! the trumpet's bell lifts the fifth over the fourth. Almost all of it is tone: the
//! rush of the air is a few percent. A blast opens in a tenth of a second, brightens
//! as the pressure comes up, and closes in about as long, sagging a little in pitch.
//!
//! A ship putting out sounds prolonged blasts, each of four to six seconds. A longer
//! ship blows longer, and the longest sounds the three of a departure.
//!
//! It is heard down a [`Line`], so it starts late, leans in pitch with the ship's
//! own way, and its last blast is still heard after the valve has shut.
use crate::{
    Air, RATE,
    filter::{Biquad, Rng},
    line::Line,
    voice::Ears,
};
use glam::Vec3;
use std::f32::consts::TAU;

/// The longest ship afloat, in metres, and the note of her horn in hertz.
const LONGEST: (f32, f32) = (168., 84.);
/// How steeply the note falls with the ship's length, from the rules' three bands.
const DEEPENS: f32 = 0.62;
/// Level of the longest ship's horn close to, and how steeply a shorter ship's falls
/// short of it. A horn of the biggest class is made to be heard two sea miles off, so
/// at 400 m it stands a few decibels over the ship's own noise at full ahead, and
/// close to it is as loud as gunfire. The rules' least levels fall half again as
/// steeply with length, but each of these ships is a fighting ship whose horn is
/// meant to be heard.
const LOUDEST: f32 = 3.5;
const LOUDENS: f32 = 0.5;
/// Strength of each tone against the first, in decibels: the mean of three recordings
/// of big ships' horns, each over several seconds of steady blast.
pub(crate) const TONES_DB: [f32; 16] = [
    0., 3., 0., -6., -3., -9., -10., -18., -14., -18., -18., -20., -21., -19., -23., -28.,
];
/// Seconds for the pressure behind the diaphragm to come up when the valve opens, and
/// to fall when it shuts.
const OPENING: f32 = 0.05;
const CLOSING: f32 = 0.08;
/// How sharp the note comes in, and the seconds that takes to settle.
const SHARP: (f32, f32) = (0.035, 0.12);
/// How far the note sags as the pressure falls away.
const SAG: f32 = 0.015;
/// How far each tone above the first lags the pressure: at half pressure the fifth
/// tone is half as strong again as it would be. A blast brightens as it opens.
const BRASS: f32 = 0.25;
/// How far the note wanders, and the share of the way it moves towards a new random
/// value each sample.
const WANDER: (f32, f32) = (0.003, 2e-5);
/// The rush of the air: its level against the tone, and the multiple of the note and
/// the sharpness of the band it is heard in.
const BREATH: (f32, f32, f32) = (0.5, 5., 0.7);
/// Seconds from one blast's end to the next one's start.
const GAP: f32 = 1.5;

/// Pressure below which a shut horn has fallen silent.
const SILENT: f32 = 1e-4;

/// What a ship of a given length blows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Signal {
    /// The fundamental, in hertz.
    pub note: f32,
    /// Level close to.
    pub level: f32,
    /// Seconds of each blast.
    pub blast: f32,
    pub blasts: u8,
}
impl Signal {
    /// The horn of a ship this many metres long and the signal she sounds putting out.
    pub fn of(length: f32) -> Self {
        let length = length.max(1.);
        Self {
            note: LONGEST.1 * (LONGEST.0 / length).powf(DEEPENS),
            level: LOUDEST * (length / LONGEST.0).min(1.).powf(LOUDENS),
            blast: (3.2 + length / 60.).clamp(4., 6.),
            blasts: if length >= 120. { 3 } else { 2 },
        }
    }
    /// Seconds from the first blast opening to the last one shutting.
    pub fn span(&self) -> f32 {
        f32::from(self.blasts) * (self.blast + GAP) - GAP
    }
    /// Whether the valve is open this many seconds into the signal.
    fn open(&self, secs: f32) -> bool {
        let blast = (secs / (self.blast + GAP)).floor();
        secs >= 0.
            && blast < f32::from(self.blasts)
            && secs - blast * (self.blast + GAP) < self.blast
    }
}

pub(crate) struct Horn {
    position: Vec3,
    signal: Signal,
    /// Seconds into the signal, while it is being sounded.
    sounding: Option<f32>,
    /// The pressure behind the diaphragm, from 0 to 1, and how sharp the note still is
    /// from the last time the valve opened, from 1 just opened to 0 settled.
    pressure: f32,
    settling: f32,
    valve: bool,
    /// How far round the fundamental is, in turns, and how far its note has wandered.
    angle: f32,
    wander: f32,
    /// The strength of each tone at the end of the last block, as the pressure
    /// brightened it.
    tones: [f32; TONES_DB.len()],
    breath: Biquad,
    rng: Rng,
    line: Line,
}
impl Horn {
    /// Horns with different seeds are different horns of the same make.
    pub fn new(seed: u32) -> Self {
        let mut rng = Rng::new(seed);
        Self {
            position: Vec3::ZERO,
            signal: Signal::of(LONGEST.0),
            sounding: None,
            pressure: 0.,
            settling: 0.,
            valve: false,
            angle: rng.unit(),
            wander: 0.,
            tones: [0.; TONES_DB.len()],
            breath: Biquad::default(),
            rng,
            line: Line::new(seed ^ 0xB1A5),
        }
    }

    /// Sound the signal of a ship this long where she now stands. A horn still
    /// blowing starts its signal again.
    pub fn blow(&mut self, position: Vec3, length: f32) {
        if self.spent() {
            self.line.renew();
        }
        self.signal = Signal::of(length);
        self.breath = Biquad::band_pass(self.signal.note * BREATH.1, BREATH.2);
        (self.position, self.sounding) = (position, Some(0.));
    }

    /// Carry the horn to where its ship now is.
    pub fn follow(&mut self, position: Vec3) {
        self.position = position;
    }

    /// Nothing is blowing and nothing it blew is still on its way.
    pub fn spent(&self) -> bool {
        self.sounding.is_none() && self.line.spent()
    }

    /// Stop dead, with nothing left in the air.
    pub fn clear(&mut self) {
        self.line.clear();
        (self.sounding, self.valve) = (None, false);
        (self.pressure, self.settling) = (0., 0.);
        self.tones = [0.; TONES_DB.len()];
    }

    /// Blow through one block and add what the listener hears of it.
    pub fn sing(&mut self, ears: &Ears, air: &Air, out: &mut [[f32; 2]], send: &mut [[f32; 2]]) {
        self.blow_block(out.len());
        self.line.cross(self.position, ears, air, out, send);
    }

    fn blow_block(&mut self, frames: usize) {
        let Some(secs) = self.sounding else {
            return self.line.rest(frames);
        };
        let signal = self.signal;
        let open = signal.open(secs);
        if open && !self.valve {
            self.settling = 1.;
        }
        self.valve = open;
        let elapsed = secs + frames as f32 / RATE;
        // The tones brighten with the pressure. They are set once a block and glided
        // across it, which is fine enough for a tenth of a second's opening.
        let whole = TONES_DB
            .iter()
            .map(|db| 10_f32.powf(db / 10.))
            .sum::<f32>()
            .sqrt();
        let was = self.tones;
        let mut now = [0.; TONES_DB.len()];
        let lift = self.pressure.max(SILENT);
        for (k, (tone, db)) in now.iter_mut().zip(TONES_DB).enumerate() {
            *tone = 10_f32.powf(db / 20.) / whole * lift.powf(BRASS * k as f32);
        }
        self.tones = now;
        let target = f32::from(open);
        let rate = 1. / (if open { OPENING } else { CLOSING } * RATE);
        let settle = 1. / (SHARP.1 * RATE);
        let level = signal.level;
        let (pressure, settling, angle, wander) = (
            &mut self.pressure,
            &mut self.settling,
            &mut self.angle,
            &mut self.wander,
        );
        let (rng, breath) = (&mut self.rng, &mut self.breath);
        // Slowed that much, noise that filled -1..1 keeps this much of its spread.
        let spread = (WANDER.1 / (3. * (2. - WANDER.1))).sqrt();
        self.line.keep(frames, |n| {
            *pressure += (target - *pressure) * rate;
            *settling -= *settling * settle;
            *wander += WANDER.1 * (rng.noise() - *wander);
            let pitch = 1. + SHARP.0 * *settling - SAG * (1. - *pressure)
                + WANDER.0 * (*wander / spread).clamp(-2., 2.);
            *angle = (*angle + signal.note * pitch / RATE).fract();
            let k = n as f32 / frames as f32;
            // Each tone is found from the two below it, which costs one sine and one
            // cosine a sample.
            let (sin, cos) = (TAU * *angle).sin_cos();
            let (mut under, mut tone, mut sound) = (0., sin, 0.);
            for (from, to) in was.iter().zip(&now) {
                sound += (from + (to - from) * k) * tone;
                (under, tone) = (tone, 2. * cos * tone - under);
            }
            let rush = BREATH.0 * breath.run(rng.noise());
            level * *pressure * (sound + rush)
        });
        // The signal is over once the last blast has shut and its pressure is spent.
        let done = elapsed >= signal.span() && self.pressure < SILENT;
        self.sounding = (!done).then_some(elapsed);
        if done {
            (self.pressure, self.valve) = (0., false);
        }
    }
}
