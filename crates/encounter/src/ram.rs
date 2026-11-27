//! Hull contact between ships: who gives way, who is hurt, and by how much.
//!
//! Each ship absorbs the closing speed in proportion to the other's share of the
//! combined mass. A heavy ship ploughs through a light one and barely feels it.
//! The same blow stops a light ship dead and costs it far more hull.
use super::{Battle, Blow, Effect, EffectKind, Events, Orders, Raider, bearing, body};
use glam::Vec2;
use shipvivor_compass::direction;

/// Closing speed in metres per second below which a contact is a harmless bump.
pub(super) const BUMP: f32 = 2.;
/// Seconds after a damaging blow before the same raider can be hurt by another.
/// Hulls grinding together would otherwise take damage on every tick.
pub(super) const RAM_REST: f32 = 1.;
/// Fraction of the closing speed returned as rebound.
const REBOUND: f32 = 0.2;
/// A blow taken on the stem costs this fraction of one taken anywhere else.
const STEM: f32 = 0.5;
/// Cosine of the widest angle off the bow that still counts as a stem strike.
const STEM_ARC: f32 = 0.7;

/// The waterline shape and heft of a class of ship.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hull {
    /// Half the beam in metres.
    pub radius: f32,
    /// Distance from the ship's position to the tip of the bow.
    pub bow: f32,
    /// Distance from the ship's position to the stern.
    pub stern: f32,
    /// Relative displacement. Only the ratio between two ships matters.
    pub mass: f32,
    /// Metres per second of absorbed shock that cost one hull point.
    pub shock_per_point: f32,
}
impl Hull {
    /// Breakwater: 30 metres long with 100 hull points.
    pub const CRUISER: Self = Self {
        radius: 2.75,
        bow: 16.,
        stern: 14.1,
        mass: 8.,
        shock_per_point: 0.25,
    };
    /// Breakwater grows aft of the original machinery space. Her bow and bridge stay put.
    pub fn breakwater(design: &shipvivor_shipyard::Design) -> Self {
        Self {
            mass: design.mass(),
            radius: design.beam() / 2.,
            stern: Self::CRUISER.stern
                + (design.length() - shipvivor_shipyard::Design::BREAKWATER_LENGTH),
            ..Self::CRUISER
        }
    }

    /// Translate original deck stations abaft the yard splice, in metres aft of amidships.
    pub fn deck_z(stock_z: f32, extension: f32) -> f32 {
        if stock_z >= Self::SPLICE_Z {
            stock_z + extension
        } else {
            stock_z
        }
    }

    /// Deck outline used by both collision and rendering after a beam refit.
    pub fn outline(self) -> [(f32, f32); 11] {
        let factor = self.radius / Self::CRUISER.radius;
        super::breakwater_outline(self.extension()).map(|(x, z)| (x * factor, z))
    }

    pub const SPLICE_Z: f32 = shipvivor_shipyard::HULL_SPLICE;

    pub fn extension(self) -> f32 {
        self.stern - Self::CRUISER.stern
    }

    /// Half the cruiser's length, so an eighth of its displacement, with 4 hull points.
    pub const RAIDER: Self = Self {
        radius: 2.5,
        bow: 8.,
        stern: 7.,
        mass: 1.,
        shock_per_point: 2.5,
    };
}

/// One ship at the instant of a contact check.
pub(super) struct Ship {
    pub hull: Hull,
    pub pos: Vec2,
    pub forward: Vec2,
    pub velocity: Vec2,
}
impl Ship {
    /// The hull is every point within `radius` of this line.
    fn keel(&self) -> [Vec2; 2] {
        [
            self.pos + self.forward * (self.hull.bow - self.hull.radius),
            self.pos - self.forward * (self.hull.stern - self.hull.radius),
        ]
    }
}

/// The outcome of two overlapping hulls. Paired fields are `[first, second]`.
pub(super) struct Contact {
    pub point: Vec2,
    /// Unit vector from the first ship toward the second.
    pub normal: Vec2,
    /// Speed at which the hulls were approaching. Zero when they are parting.
    pub closing: f32,
    /// Change in position that separates the hulls.
    pub give: [Vec2; 2],
    /// Change in velocity.
    pub kick: [Vec2; 2],
    /// Hull points lost. Zero for a bump.
    pub damage: [u16; 2],
}

pub(super) fn contact(a: &Ship, b: &Ship) -> Option<Contact> {
    let [a_bow, a_stern] = a.keel();
    let [b_bow, b_stern] = b.keel();
    let (on_a, on_b) = nearest(a_bow, a_stern, b_bow, b_stern);
    let gap = on_b - on_a;
    let reach = a.hull.radius + b.hull.radius;
    if gap.length_squared() >= reach * reach {
        return None;
    }
    // Crossed keels leave no gap to measure, so part the ships centre from centre.
    let normal = gap
        .try_normalize()
        .or_else(|| (b.pos - a.pos).try_normalize())
        .unwrap_or(a.forward.perp());
    let depth = reach - gap.length();
    let total = a.hull.mass + b.hull.mass;
    let share = [b.hull.mass / total, a.hull.mass / total];
    let closing = (a.velocity - b.velocity).dot(normal).max(0.);
    let blow = |ship: &Ship, toward: Vec2, share: f32| {
        if closing < BUMP {
            return 0;
        }
        let stem = if ship.forward.dot(toward) > STEM_ARC {
            STEM
        } else {
            1.
        };
        (closing * share * stem / ship.hull.shock_per_point) as u16
    };
    let recoil = closing * (1. + REBOUND);
    Some(Contact {
        point: on_a + normal * a.hull.radius,
        normal,
        closing,
        give: [-normal * depth * share[0], normal * depth * share[1]],
        kick: [-normal * recoil * share[0], normal * recoil * share[1]],
        damage: [blow(a, normal, share[0]), blow(b, -normal, share[1])],
    })
}

/// Closest points between segments `p1..q1` and `p2..q2`, neither of zero length.
fn nearest(p1: Vec2, q1: Vec2, p2: Vec2, q2: Vec2) -> (Vec2, Vec2) {
    let d1 = q1 - p1;
    let d2 = q2 - p2;
    let r = p1 - p2;
    let a = d1.length_squared();
    let e = d2.length_squared();
    let b = d1.dot(d2);
    let c = d1.dot(r);
    let f = d2.dot(r);
    let denom = a * e - b * b;
    let mut s = if denom > 1e-6 {
        ((b * f - c * e) / denom).clamp(0., 1.)
    } else {
        0.
    };
    let mut t = (b * s + f) / e;
    if t < 0. {
        t = 0.;
        s = (-c / a).clamp(0., 1.);
    } else if t > 1. {
        t = 1.;
        s = ((b - c) / a).clamp(0., 1.);
    }
    (p1 + d1 * s, p2 + d2 * t)
}

pub(super) fn afloat(raider: &Raider) -> Ship {
    Ship {
        hull: raider.class.profile().hull,
        pos: raider.body.pos,
        forward: raider.body.forward(),
        velocity: raider.way,
    }
}

impl Battle {
    /// Part every pair of overlapping hulls and settle the damage of each new blow.
    pub(super) fn resolve_contacts(&mut self, orders: &Orders, events: &mut Events) {
        let forward = direction(orders.heading);
        let mut pos = orders.player;
        let mut speed = orders.speed;
        for raider in &mut self.raiders {
            let player = Ship {
                hull: orders.hull,
                pos,
                forward,
                velocity: forward * speed,
            };
            let Some(hit) = contact(&player, &afloat(raider)) else {
                continue;
            };
            pos += hit.give[0];
            // The helm only carries way along the keel.
            speed += hit.kick[0].dot(forward);
            raider.body.pos += hit.give[1];
            raider.knock += hit.kick[1];
            if hit.closing >= BUMP && raider.ram_rest == 0. {
                raider.ram_rest = RAM_REST;
                raider.hit_hull(hit.point, f32::from(hit.damage[1]));
                events.damage = events.damage.saturating_add(hit.damage[0]);
                if hit.damage[0] > 0 {
                    let reach = orders.hull.bow + orders.hull.stern;
                    let ahead = (hit.point - pos).dot(forward);
                    events.blows.push(Blow {
                        aft: (orders.hull.bow - ahead).clamp(0., reach),
                        abeam: (hit.point - pos).dot(forward.perp()),
                        damage: hit.damage[0],
                        calibre: None,
                        reach: shipvivor_shipyard::Reach::Bays,
                        breach: false,
                    });
                }
                self.effects.push(Effect {
                    body: body(&mut self.serial, hit.point, bearing(hit.normal)),
                    kind: EffectKind::Crunch,
                    height: 0.,
                    age: 0.,
                });
            }
        }
        events.shove = pos - orders.player;
        events.jolt = speed - orders.speed;
        // Raiders shoulder each other apart without harm.
        for i in 1..self.raiders.len() {
            let (ahead, rest) = self.raiders.split_at_mut(i);
            let b = &mut rest[0];
            for a in ahead {
                if let Some(hit) = contact(&afloat(a), &afloat(b)) {
                    a.body.pos += hit.give[0];
                    b.body.pos += hit.give[1];
                }
            }
        }
    }
}
