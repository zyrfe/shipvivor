//! The open sea as a few trochoidal wave trains.
//!
//! The renderer moves its vertices with the same trains, so a hull that asks how
//! it rides here sits on the water that is drawn.
use glam::{FloatExt, Vec2, Vec3};
use shipvivor_compass::direction;
use std::f32::consts::TAU;

/// Trains in a sea. The first two are the long swell and the rest are wind waves.
pub const TRAINS: usize = 8;
/// How far water is drawn toward a crest, as a fraction of the full trochoid.
/// It sharpens crests and flattens troughs. At 1 the steepest seas would fold over.
pub const CHOP: f32 = 0.85;
const GRAVITY: f32 = 9.81;
/// Wavelength in metres of the shortest train.
const SHORTEST: f32 = 3.2;
/// How far each wind wave runs off the wind, in radians.
const VEER: [f32; TRAINS - 2] = [0.5, -0.7, 0.2, -0.35, 0.9, -1.05];

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Train {
    /// Direction of travel scaled by the wavenumber, in radians per metre.
    pub wave: Vec2,
    /// Height of a crest above calm water.
    pub amplitude: f32,
    /// How far the train has run, in radians. A crest sits where `wave · p` equals it.
    pub phase: f32,
    /// Radians per second, from the deep-water speed of a wave this long.
    rate: f32,
}
impl Train {
    fn new(length: f32, steepness: f32, heading: f32, phase: f32) -> Self {
        let number = TAU / length;
        Self {
            wave: direction(heading) * number,
            amplitude: steepness / number,
            phase,
            rate: (GRAVITY * number).sqrt(),
        }
    }
}

/// How a hull sits on the swell.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ride {
    /// Metres the hull is lifted.
    pub heave: f32,
    /// Radians the bow is raised.
    pub pitch: f32,
    /// Radians the starboard side is raised.
    pub roll: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Swell {
    trains: [Train; TRAINS],
}
impl Default for Swell {
    fn default() -> Self {
        Self::new(0.3, 0.)
    }
}
impl Swell {
    /// A sea for a state from 0, nearly flat, to 1, a storm, with waves running down `wind`.
    pub fn new(state: f32, wind: f32) -> Self {
        let state = state.clamp(0., 1.);
        let long = 22_f32.lerp(58., state);
        let roll = 0.016_f32.lerp(0.1, state);
        let fetch = 9_f32.lerp(21., state);
        let chop = 0.02_f32.lerp(0.08, state);
        Self {
            trains: std::array::from_fn(|i| {
                // Each train starts out of step with the last, so no point begins under every crest.
                let phase = (i as f32 * 2.4).rem_euclid(TAU);
                match i {
                    // Two trains close in length and heading beat into sets of larger waves.
                    0 => Train::new(long, roll, wind + 0.14, phase),
                    1 => Train::new(long * 0.74, roll * 0.9, wind - 0.31, phase),
                    _ => {
                        let n = (i - 2) as f32 / (TRAINS - 3) as f32;
                        let length = fetch * (SHORTEST / fetch).powf(n);
                        Train::new(length, chop * (1. - 0.3 * n), wind + VEER[i - 2], phase)
                    }
                }
            }),
        }
    }

    pub fn trains(&self) -> &[Train; TRAINS] {
        &self.trains
    }

    pub fn advance(&mut self, dt: f32) {
        for train in &mut self.trains {
            train.phase = (train.phase + train.rate * dt).rem_euclid(TAU);
        }
    }

    /// Where the water resting at `rest` has been carried: east, up and south.
    pub fn displacement(&self, rest: Vec2) -> Vec3 {
        let mut moved = Vec3::ZERO;
        for train in &self.trains {
            let (sin, cos) = (train.wave.dot(rest) - train.phase).sin_cos();
            let reach = train.wave * (CHOP * train.amplitude / train.wave.length_squared().sqrt());
            moved += Vec3::new(-reach.x * sin, train.amplitude * cos, -reach.y * sin);
        }
        moved
    }

    /// Height of the surface over a point on the chart.
    pub fn height(&self, point: Vec2) -> f32 {
        // The water over a point came from a little way off. Step back to where it rests.
        let mut rest = point;
        for _ in 0..3 {
            let moved = self.displacement(rest);
            rest = point - Vec2::new(moved.x, moved.z);
        }
        self.displacement(rest).y
    }

    /// How a hull centred on `point` rides. A wave shorter than the hull lifts
    /// one part as it drops another, so only the longer trains move a ship.
    pub fn ride(&self, point: Vec2, heading: f32, half_length: f32, half_beam: f32) -> Ride {
        let ahead = direction(heading);
        let abeam = ahead.perp();
        let (mut heave, mut fore, mut side) = (0., 0., 0.);
        for train in &self.trains {
            let (along, across) = (train.wave.dot(ahead), train.wave.dot(abeam));
            let felt = train.amplitude * mean(along * half_length) * mean(across * half_beam);
            let (sin, cos) = (train.wave.dot(point) - train.phase).sin_cos();
            heave += felt * cos;
            fore -= felt * along * sin;
            side -= felt * across * sin;
        }
        Ride {
            heave,
            pitch: fore.atan(),
            roll: side.atan(),
        }
    }
}

/// Mean of a unit wave over a span reaching `x` radians either side of its centre.
fn mean(x: f32) -> f32 {
    if x.abs() < 1e-3 { 1. } else { x.sin() / x }
}
