//! The wakes of the enemy hulls, drawn wherever they are. The wake patch reaches
//! only a fraction of a gunshot around the ship, so beyond it these are all that
//! tell a ship under way from one lying stopped, and which way she is going.
//!
//! Each hull's track is kept as a few points astern of her, laid at even
//! seconds, so her white water follows her turns and fades as it ages. The
//! shader draws each track as a band of foam and each bow as a breaking V.
use bevy::{prelude::*, render::render_resource::ShaderType};
use shipvivor_encounter::Hull;
use std::collections::{HashMap, VecDeque};

#[cfg(test)]
mod tests;

/// Hulls whose wakes are drawn, nearest the ship first, and the points along
/// each one's track, her stern now among them. The shader mirrors both.
pub const WAKES: usize = 32;
pub const TRACK: usize = 11;
/// White water astern of a hull at her full speed. Far off, foam must stand
/// well above 1 to show as more than a smear.
const WHITE: f32 = 1.3;

/// What the shader is told of the enemy wakes.
#[derive(ShaderType, Clone, Debug)]
pub struct Wakes {
    /// Two for each hull: where she is and the way she points, then her
    /// half-beam, her bow, the reach of her bow wave and how hard she is
    /// steaming. All zero once she has gone.
    hulls: [Vec4; 2 * WAKES],
    /// A circle round all she has left on the water, then how many of her track points are kept.
    bounds: [Vec4; WAKES],
    /// Her track from the stern back: east, south, white water and half-width.
    track: [Vec4; WAKES * TRACK],
    count: u32,
}
impl Default for Wakes {
    fn default() -> Self {
        Self {
            hulls: [Vec4::ZERO; 2 * WAKES],
            bounds: [Vec4::ZERO; WAKES],
            track: [Vec4::ZERO; WAKES * TRACK],
            count: 0,
        }
    }
}

/// A hull afloat on this tick.
#[derive(Clone, Copy, Debug)]
pub struct Steaming {
    pub id: u64,
    pub pos: Vec2,
    pub heading: f32,
    pub hull: Hull,
    /// Metres a second she is making, and the share of her full speed that is.
    pub way: f32,
    pub pace: f32,
}

#[derive(Clone, Copy, Debug)]
struct Afloat {
    pos: Vec2,
    ahead: Vec2,
    bow: f32,
    stern: f32,
    reach: f32,
    wash: f32,
}

struct Track {
    half_beam: f32,
    /// Seconds her white water lasts.
    life: f32,
    /// Points astern of her, newest first: where, the second it was laid, and how hard she was steaming.
    crumbs: VecDeque<(Vec2, f32, f32)>,
    /// Where she is on this tick, while she is afloat.
    afloat: Option<Afloat>,
}

/// Where each enemy hull has been lately.
#[derive(Default)]
pub struct Tracks {
    seconds: f32,
    hulls: HashMap<u64, Track>,
}

/// How hard a hull is steaming for the share of her full speed she is making.
/// A slow battleship at her best throws as full a wake as a torpedo boat: it is
/// how the player reads her.
fn wash(pace: f32) -> f32 {
    if pace < 0.02 { 0. } else { pace.min(1.).powf(0.6) }
}

/// Seconds a hull's white water lasts: longer behind a bigger one, so each
/// trails a hundred metres or so at her full speed.
fn life(hull: &Hull) -> f32 {
    10. + 0.15 * (hull.bow + hull.stern)
}

/// Metres aft of her stem that the arms of a hull's bow wave run. They reach farther at speed.
fn reach(hull: &Hull, way: f32, pace: f32) -> f32 {
    (hull.bow + hull.stern) * (0.3 + 0.35 * pace.min(1.)) + 4. * way
}

impl Tracks {
    /// Undisturbed water, as at the start of a voyage.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Run the tracks on by `dt` seconds with these hulls afloat. A hull no
    /// longer among them leaves her white water where it lies until it fades.
    pub fn step(&mut self, dt: f32, afloat: impl IntoIterator<Item = Steaming>) {
        self.seconds += dt;
        let now = self.seconds;
        for track in self.hulls.values_mut() {
            track.afloat = None;
        }
        for ship in afloat {
            let track = self.hulls.entry(ship.id).or_insert_with(|| Track {
                half_beam: ship.hull.radius,
                life: life(&ship.hull),
                crumbs: VecDeque::new(),
                afloat: None,
            });
            let ahead = shipvivor_compass::direction(ship.heading);
            let wash = wash(ship.pace);
            track.afloat = Some(Afloat {
                pos: ship.pos,
                ahead,
                bow: ship.hull.bow,
                stern: ship.hull.stern,
                reach: reach(&ship.hull, ship.way, ship.pace),
                wash,
            });
            let spacing = track.life / (TRACK - 1) as f32;
            if (track.crumbs.front()).is_none_or(|&(_, laid, _)| now - laid >= spacing) {
                track.crumbs.push_front((ship.pos - ahead * ship.hull.stern, now, wash));
                track.crumbs.truncate(TRACK - 1);
            }
        }
        self.hulls.retain(|_, track| {
            track.afloat.is_some()
                || (track.crumbs.front()).is_some_and(|&(_, laid, _)| now - laid < track.life)
        });
    }

    /// The wakes nearest `focus`, as the shader reads them.
    pub fn draw(&self, focus: Vec2) -> Wakes {
        let mut nearest: Vec<(f32, &Track)> = (self.hulls.values())
            .filter_map(|track| {
                let at = (track.afloat.map(|a| a.pos)).or(track.crumbs.front().map(|c| c.0))?;
                Some((at.distance_squared(focus), track))
            })
            .collect();
        nearest.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut wakes = Wakes::default();
        for (slot, (_, track)) in nearest.into_iter().take(WAKES).enumerate() {
            let (mut low, mut high) = (Vec2::INFINITY, Vec2::NEG_INFINITY);
            let mut cover = |at: Vec2, reach: f32| {
                low = low.min(at - reach);
                high = high.max(at + reach);
            };
            if let Some(a) = track.afloat {
                wakes.hulls[2 * slot] = a.pos.extend(a.ahead.x).extend(a.ahead.y);
                wakes.hulls[2 * slot + 1] = Vec4::new(track.half_beam, a.bow, a.reach, a.wash);
                // The arms of the V, at their widest, with their broken edges.
                cover(a.pos + a.ahead * a.bow, 1.5 * a.reach + 2. * track.half_beam + 1.);
            }
            let live = track.afloat.map(|a| (a.pos - a.ahead * a.stern, 0., a.wash));
            let laid = (track.crumbs.iter()).map(|&(at, laid, wash)| (at, self.seconds - laid, wash));
            let mut points = 0;
            for (at, age, wash) in live.into_iter().chain(laid).take(TRACK) {
                let age = (age / track.life).min(1.);
                let white = WHITE * wash * (1. - age).powf(1.5);
                // Turbulent water spreads as it ages, and even a launch's is a few metres across.
                let width = track.half_beam * (0.9 + 1.6 * age) + 0.6;
                wakes.track[slot * TRACK + points] = Vec4::new(at.x, at.y, white, width);
                cover(at, 1.6 * width);
                points += 1;
            }
            let centre = (low + high) * 0.5;
            wakes.bounds[slot] = (centre.extend(0.5 * (high - low).length())).extend(points as f32);
            wakes.count += 1;
        }
        wakes
    }
}
