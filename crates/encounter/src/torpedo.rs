//! Breakwater's arcade broadside spreads. The sight and launcher share each rack's sector.
use super::{Battle, Body, Effect, EffectKind, Events, ISLAND_RADIUS, body, entry};
use glam::Vec2;
use shipvivor_compass::direction;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

pub const TORPEDO_RANGE: f32 = 420.;
pub const TORPEDO_SPEED: f32 = 38.;
pub const RACK_HALF_ARC: f32 = 50_f32.to_radians();
pub const SPREAD_HALF_ARC: f32 = 4_f32.to_radians();
/// Integrity a torpedo's warhead takes off the ice it strikes: enough to split a berg.
const WARHEAD_ON_ICE: f32 = 160.;

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
    pub(super) fn advance_torpedoes(&mut self, dt: f32, events: &mut Events) {
        for reload in &mut self.racks {
            *reload = (*reload - dt).max(0.);
        }
        self.torpedoes.retain_mut(|fish| {
            let travel = (dt * TORPEDO_SPEED).min(fish.remaining);
            let from = fish.body.pos;
            let to = from + fish.body.forward() * travel;
            let land = self
                .ports
                .iter()
                .map(|p| (p.body.pos, ISLAND_RADIUS))
                .chain(self.landmarks.iter().map(|l| (l.body.pos, l.radius)))
                .filter_map(|(at, r)| entry(from, to, at, r))
                .min_by(f32::total_cmp);
            let hull = self
                .raiders
                .iter()
                .enumerate()
                .filter(|(_, r)| r.afloat())
                .filter_map(|(i, r)| Some((r.hull_span(from, to)?.0, i)))
                .min_by(|a, b| a.0.total_cmp(&b.0));
            // Most of a berg lies under water, so a running torpedo meets its whole width.
            let ice = self
                .floe
                .icebergs
                .iter()
                .enumerate()
                .filter_map(|(i, b)| Some((entry(from, to, b.body.pos, b.radius)?, i)))
                .min_by(|a, b| a.0.total_cmp(&b.0));
            let struck = match (hull, ice) {
                (Some((t, i)), _)
                    if land.is_none_or(|l| t < l) && ice.is_none_or(|(c, _)| t < c) =>
                {
                    self.raiders[i].hit_hull(from.lerp(to, t), 6);
                    events.hits += 1;
                    Some((t, EffectKind::Burst))
                }
                (_, Some((t, i))) if land.is_none_or(|l| t < l) => {
                    self.floe.icebergs[i].integrity -= WARHEAD_ON_ICE;
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
                    height: 0.7,
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
