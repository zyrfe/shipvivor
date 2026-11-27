//! Breakwater's arcade broadside spreads. The sight and launcher share each rack's sector.
//! The enemy's torpedo boats fire the same torpedo, one at a time, at her.
use super::torpedo_boat::WARHEAD;
use super::{Battle, Blow, Body, Effect, EffectKind, Events, body, entry};
use glam::Vec2;
use shipvivor_compass::direction;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

pub const TORPEDO_RANGE: f32 = 420.;
pub const TORPEDO_SPEED: f32 = 38.;
pub const RACK_HALF_ARC: f32 = 50_f32.to_radians();
pub const SPREAD_HALF_ARC: f32 = 4_f32.to_radians();
/// Integrity a torpedo's warhead takes off the ice it strikes: enough to split the
/// biggest slab at a blow.
const WARHEAD_ON_ICE: f32 = 12.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rack {
    Port,
    Starboard,
}
impl Rack {
    pub const BOTH: [Self; 2] = [Self::Port, Self::Starboard];
    pub fn center(self, heading: f32) -> f32 {
        heading
            + if self == Self::Port {
                -FRAC_PI_2
            } else {
                FRAC_PI_2
            }
    }
    pub fn origin(self, player: Vec2, heading: f32) -> Vec2 {
        player - direction(heading) * 1.9 + direction(self.center(heading)) * 2.7
    }
    pub fn for_aim(heading: f32, aim: f32) -> Option<Self> {
        Self::BOTH.into_iter().find(|side| {
            let delta = (aim - side.center(heading) + PI).rem_euclid(TAU) - PI;
            delta.abs() <= RACK_HALF_ARC - SPREAD_HALF_ARC
        })
    }
}
pub struct Torpedo {
    pub body: Body,
    remaining: f32,
    /// The torpedo boat that fired it, or `None` for one of Breakwater's own.
    pub fired_by: Option<u64>,
}
impl Torpedo {
    /// Fired at Breakwater by a torpedo boat. It runs for her and for nothing else.
    pub fn hostile(&self) -> bool {
        self.fired_by.is_some()
    }
    /// Metres it has run since it left the tube.
    pub fn run(&self) -> f32 {
        TORPEDO_RANGE - self.remaining
    }
    /// Seconds until it passes closest to a ship at `ship` making `way`, if it will pass
    /// within `reach` metres of her before it runs out. Neither turns in the meantime.
    pub fn closing(&self, ship: Vec2, way: Vec2, reach: f32) -> Option<f32> {
        let relative = self.body.forward() * TORPEDO_SPEED - way;
        let offset = self.body.pos - ship;
        let seconds = -offset.dot(relative) / relative.length_squared().max(1e-6);
        let miss = (offset + relative * seconds).length();
        (seconds > 0. && seconds * TORPEDO_SPEED <= self.remaining && miss <= reach)
            .then_some(seconds)
    }
}

/// Where a torpedo fired from `from` at `speed` meets a ship at `ship` holding her
/// course and making `way`, or `None` if it can never catch her.
pub fn intercept(from: Vec2, ship: Vec2, way: Vec2, speed: f32) -> Option<Vec2> {
    // |ship + way t - from| = speed t, solved for the first t after the launch.
    let offset = ship - from;
    let a = way.length_squared() - speed * speed;
    let b = 2. * offset.dot(way);
    let c = offset.length_squared();
    let seconds = if a.abs() < 1e-6 {
        (b < 0.).then(|| -c / b)?
    } else {
        let disc = b * b - 4. * a * c;
        if disc < 0. {
            return None;
        }
        let root = disc.sqrt();
        [(-b - root) / (2. * a), (-b + root) / (2. * a)]
            .into_iter()
            .filter(|t| *t > 0.)
            .min_by(f32::total_cmp)?
    };
    Some(ship + way * seconds)
}

impl Battle {
    pub fn rack_origin(&self, rack: Rack, player: Vec2, heading: f32) -> Vec2 {
        player - direction(heading) * (1.9 + self.deck.machinery_z)
            + direction(rack.center(heading)) * (1.95 * self.deck.beam_scale + 0.75)
    }
    pub fn torpedoes(&self) -> &[Torpedo] {
        &self.torpedoes
    }
    pub fn rack_reload(&self, rack: Rack) -> f32 {
        self.racks[rack as usize]
    }
    pub fn launch_spread(&mut self, player: Vec2, heading: f32, aim: f32) -> bool {
        let Some(rack) = Rack::for_aim(heading, aim) else {
            return false;
        };
        if self.rack_reload(rack) > 0. {
            return false;
        }
        let origin = self.rack_origin(rack, player, heading);
        for offset in [-SPREAD_HALF_ARC, 0., SPREAD_HALF_ARC] {
            self.torpedoes.push(Torpedo {
                body: body(&mut self.serial, origin, aim + offset),
                remaining: TORPEDO_RANGE,
                fired_by: None,
            });
        }
        self.effects.push(Effect {
            body: body(&mut self.serial, origin, aim),
            kind: EffectKind::Splash,
            age: 0.,
            height: 0.2,
        });
        self.racks[rack as usize] = 14.;
        true
    }
    /// A torpedo boat at `from`, with the id `boat`, puts her one torpedo in the water on
    /// `heading`.
    pub(super) fn fire_torpedo(&mut self, from: Vec2, heading: f32, boat: u64) {
        self.torpedoes.push(Torpedo {
            body: body(&mut self.serial, from, heading),
            remaining: TORPEDO_RANGE,
            fired_by: Some(boat),
        });
    }
    /// Run every torpedo on for a step. Breakwater's strike the enemy's ships, and the
    /// enemy's strike her, at `player`, and pass under their own side. Land and ice
    /// stop both.
    pub(super) fn advance_torpedoes(&mut self, dt: f32, player: Vec2, events: &mut Events) {
        for reload in &mut self.racks {
            *reload = (*reload - dt).max(0.);
        }
        let (forward, keel) = self.keel;
        let outline = keel.outline().map(Vec2::from);
        let local =
            |p: Vec2| Vec2::new((p - player).dot(forward.perp()), -(p - player).dot(forward));
        self.torpedoes.retain_mut(|fish| {
            let travel = (dt * TORPEDO_SPEED).min(fish.remaining);
            let from = fish.body.pos;
            let to = from + fish.body.forward() * travel;
            let land = self
                .ports
                .iter()
                .flat_map(|p| p.shores())
                .chain(self.landmarks.iter().map(|l| (l.body.pos, l.radius)))
                .filter_map(|(at, r)| entry(from, to, at, r))
                .min_by(f32::total_cmp);
            // `None` for the ship it is running at: Breakwater, for an enemy's torpedo.
            let hull = if fish.hostile() {
                let start = local(from);
                super::plating::clip_hull(&outline, start, local(to) - start)
                    .map(|(enter, _)| (enter, None))
            } else {
                self.raiders
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| r.afloat())
                    .filter_map(|(i, r)| Some((r.hull_span(from, to)?.0, Some(i))))
                    .min_by(|a, b| a.0.total_cmp(&b.0))
            };
            // Most of a berg lies under water, so a running torpedo meets its whole width.
            let ice = self
                .floe
                .icebergs
                .iter()
                .enumerate()
                .filter_map(|(i, b)| Some((entry(from, to, b.body.pos, b.radius)?, i)))
                .min_by(|a, b| a.0.total_cmp(&b.0));
            let struck = match (hull, ice) {
                (Some((t, ship)), _)
                    if land.is_none_or(|l| t < l) && ice.is_none_or(|(c, _)| t < c) =>
                {
                    let at = from.lerp(to, t);
                    if let Some(i) = ship {
                        self.raiders[i].hit_hull(at, 6.);
                        events.hits += 1;
                        Some((t, EffectKind::Burst))
                    } else {
                        // A blow under her waterline that no plate turns. It opens the
                        // bays of the module it lands on to the sea, and the sea pours
                        // into the module until the hole is shored.
                        let side = local(at);
                        events.blows.push(Blow {
                            aft: (keel.bow + side.y).clamp(0., keel.bow + keel.stern),
                            abeam: side.x,
                            damage: WARHEAD,
                            calibre: None,
                            reach: shipvivor_shipyard::Reach::Bays,
                            breach: true,
                        });
                        events.damage = events.damage.saturating_add(WARHEAD);
                        events.torpedoed += 1;
                        Some((t, EffectKind::Torpedoed))
                    }
                }
                (_, Some((t, i))) if land.is_none_or(|l| t < l) => {
                    self.floe.icebergs[i].strike(WARHEAD_ON_ICE, to - from);
                    Some((t, EffectKind::Burst))
                }
                _ => land.map(|t| (t, EffectKind::Splash)),
            };
            if let Some((t, kind)) = struck {
                if kind == EffectKind::Burst {
                    self.effects.push(Effect {
                        body: body(&mut self.serial, from.lerp(to, t), fish.body.heading),
                        age: 0.,
                        height: 0.2,
                        kind: EffectKind::Splash,
                    });
                }
                self.effects.push(Effect {
                    body: body(&mut self.serial, from.lerp(to, t), fish.body.heading),
                    age: 0.,
                    height: if kind == EffectKind::Torpedoed {
                        0.
                    } else {
                        0.7
                    },
                    kind,
                });
                return false;
            }
            fish.body.pos = to;
            fish.remaining -= travel;
            fish.remaining > 0.
        });
    }
}
