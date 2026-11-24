//! The sounds a place makes when nobody is shooting at it.
//!
//! The sea breaks on every shore, seabirds keep to the land, ice splits and now and
//! then a glacier drops part of its front into the water, a yard at work is riveting,
//! and the ship's own frames creak as she works in the swell. None of it is steady.
//! Each is a sound made once, at its own place, at a moment nobody chose, and it is
//! heard like any other: late, dulled by the air, from where it was made.
use crate::{Cue, Cues, Sound, filter::Rng};
use glam::Vec3;
use std::f32::consts::TAU;

/// What kind of place it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Haunt {
    /// Land. The sea breaks on its shore and seabirds keep to it.
    Island,
    /// One stretch of a long shore. It has an island's surf, and fewer birds than an
    /// island gathers.
    Coast,
    /// Land under an ice cap that runs down to the sea. It has the shore and the
    /// birds of any island, and its ice splits and calves.
    Glacier,
    /// Ice adrift. The sea laps at it, and it splits more rarely.
    Berg,
    /// A yard at work.
    Yard,
}

/// A place that makes sounds of its own: its middle at the height it is heard from,
/// and its radius at the waterline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub haunt: Haunt,
    pub position: Vec3,
    pub radius: f32,
}

/// Where on a place a sound is made.
#[derive(Clone, Copy)]
enum Spot {
    /// At the waterline, on the side towards the listener, within this many radians
    /// either way.
    Shore(f32),
    /// Over it, between these heights above its middle.
    Above(f32, f32),
    /// At its middle.
    Middle,
}

/// One sound a kind of place makes.
struct Habit {
    sound: Sound,
    /// Times a second, for a place of `GIRTH`. A bigger one makes more, up to `MOST`
    /// times as many.
    rate: f32,
    /// Metres from its edge past which it is not worth making.
    earshot: f32,
    spot: Spot,
    /// Whether more of the place makes more of the sound.
    grows: bool,
}
const fn habit(sound: Sound, rate: f32, earshot: f32, spot: Spot, grows: bool) -> Habit {
    Habit {
        sound,
        rate,
        earshot,
        spot,
        grows,
    }
}
/// Radius of the island the rates are given for, and how many times those rates the
/// largest place makes.
const GIRTH: f32 = 26.;
const MOST: f32 = 2.5;

const FACING: Spot = Spot::Shore(1.4);
const ALOFT: Spot = Spot::Above(6., 30.);
const ISLAND: &[Habit] = &[
    habit(Sound::Breaker, 0.3, 320., FACING, true),
    habit(Sound::Gull, 0.1, 380., ALOFT, true),
];
const COAST: &[Habit] = &[
    habit(Sound::Breaker, 0.3, 320., FACING, true),
    habit(Sound::Gull, 0.04, 380., ALOFT, true),
];
const GLACIER: &[Habit] = &[
    habit(Sound::Breaker, 0.15, 320., FACING, true),
    habit(Sound::Gull, 0.05, 380., ALOFT, true),
    habit(Sound::IceCrack, 0.06, 700., Spot::Shore(0.8), false),
    habit(Sound::Calving, 0.008, 1500., Spot::Shore(0.5), false),
];
const BERG: &[Habit] = &[
    habit(Sound::Breaker, 0.1, 200., FACING, true),
    habit(Sound::IceCrack, 0.02, 500., Spot::Middle, true),
];
const YARD: &[Habit] = &[habit(Sound::Rivets, 0.3, 600., Spot::Middle, false)];
impl Haunt {
    fn habits(self) -> &'static [Habit] {
        match self {
            Self::Island => ISLAND,
            Self::Coast => COAST,
            Self::Glacier => GLACIER,
            Self::Berg => BERG,
            Self::Yard => YARD,
        }
    }
}

/// Nothing is made nearer the listener than this many metres. Right alongside, the
/// ship's own wash is all there is to hear of the sea, and birds keep their distance.
/// The levels of these sounds are set to carry, and would be far too much up close.
const ALONGSIDE: f32 = 40.;
/// Times a second the ship's frames creak when she lies stopped, and how many more
/// at full speed.
const WORKING: (f32, f32) = (0.03, 0.22);

/// Decides when each place makes its sounds.
pub struct Ambience {
    rng: Rng,
    serial: u64,
}
impl Default for Ambience {
    fn default() -> Self {
        Self::new(0xA3B1)
    }
}
impl Ambience {
    /// The same seed makes the same sounds at the same moments.
    pub fn new(seed: u32) -> Self {
        Self {
            rng: Rng::new(seed),
            serial: 0,
        }
    }

    /// Let `secs` go by and send whatever was made in them. `ears` is where the
    /// listener stands, and `way` the ship's speed as a share of her full speed.
    pub fn pass(
        &mut self,
        secs: f32,
        ears: Vec3,
        way: f32,
        places: impl IntoIterator<Item = Place>,
        cues: &Cues,
    ) {
        for place in places {
            let off = (ears - place.position).with_y(0.);
            let girth = (place.radius / GIRTH).min(MOST);
            for habit in place.haunt.habits() {
                let rate = habit.rate * if habit.grows { girth } else { 1. };
                // Drawn whether or not it is in earshot, so that what one place does
                // never depends on where the ship is to another.
                let made = self.rng.unit() < rate * secs;
                let (turn, lift) = (self.rng.noise(), self.rng.unit());
                if !made || off.length() - place.radius > habit.earshot {
                    continue;
                }
                let towards = off.normalize_or(Vec3::X);
                let position = match habit.spot {
                    Spot::Shore(arc) => {
                        let (sin, cos) = (arc * turn).sin_cos();
                        let facing = Vec3::new(
                            towards.x * cos - towards.z * sin,
                            0.,
                            towards.x * sin + towards.z * cos,
                        );
                        (place.position + place.radius * facing).with_y(0.5)
                    }
                    Spot::Above(low, high) => {
                        let (sin, cos) = (TAU * turn).sin_cos();
                        let (out, up) = (place.radius * lift.sqrt(), (lift * 7.3).fract());
                        place.position + Vec3::new(out * cos, low + (high - low) * up, out * sin)
                    }
                    Spot::Middle => place.position,
                };
                if position.distance(ears) >= ALONGSIDE {
                    self.emit(habit.sound, position, cues);
                }
            }
        }
        let working = WORKING.0 + WORKING.1 * way.clamp(0., 1.);
        if self.rng.unit() < working * secs {
            self.emit(Sound::Creak, ears, cues);
        }
    }

    fn emit(&mut self, sound: Sound, position: Vec3, cues: &Cues) {
        self.serial += 1;
        cues.send(Cue::Emit {
            sound,
            position,
            serial: self.serial,
        });
    }
}
