//! A shore-based floatplane flies one pass, alights at its slip and rearms.
use super::{Battery, Battle, Body, YardState, aviation::Bomb, bearing, body, fire};
use glam::{Vec2, Vec3, Vec3Swizzles};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AirRaid {
    Strafe,
    Bomb,
}
impl AirRaid {
    fn pass_height(self) -> f32 {
        match self {
            Self::Strafe => 14.,
            Self::Bomb => 24.,
        }
    }
}

pub struct AirStation {
    /// The hangar occupies the first factory site at this harbour.
    pub port: usize,
    pub aircraft: Option<AttackPlane>,
    bombers: bool,
    next: AirRaid,
    pub(super) rearm: f32,
}

pub struct AttackPlane {
    pub body: Body,
    pub altitude: f32,
    pub bank: f32,
    pub raid: AirRaid,
    /// Hits it can still take before it goes down.
    pub health: u16,
    /// Metres per second over the water, as it last flew.
    pub way: Vec2,
    flight: Flight,
}

impl AttackPlane {
    pub fn position_3d(&self) -> Vec3 {
        self.body.pos.extend(self.altitude).xzy()
    }

    pub fn inbound(&self) -> bool {
        !matches!(self.flight, Flight::Home(_))
    }
}

impl AirStation {
    pub fn operational(&self, port: &super::Port) -> bool {
        port.yard_state() != YardState::Ruined && port.factories()[0].live()
    }

    pub fn slip(&self, port: &super::Port) -> Vec2 {
        port.factory_position(0) - port.body.forward().perp() * 11.
    }
}

enum Flight {
    Out(Transit),
    Pass { mark: Vec2, released: usize },
    Home(Transit),
}

/// A curved departure or recovery with its end tangent aligned to the next leg.
struct Transit {
    points: [Vec2; 4],
    duration: f32,
    elapsed: f32,
}

const SPEED: f32 = 46.;
impl Transit {
    fn new(start: Vec2, outbound: Vec2, end: Vec2, inbound: Vec2) -> Self {
        let reach = (start.distance(end) * 0.45).clamp(40., 100.);
        let a = start + outbound * reach;
        let b = end - inbound * reach;
        Self {
            points: [start, a, b, end],
            duration: (start.distance(end) + start.distance(a) + a.distance(b) + b.distance(end))
                / (2. * SPEED),
            elapsed: 0.,
        }
    }

    fn step(&mut self, dt: f32) -> (Vec2, Vec2, f32) {
        self.elapsed += dt;
        let t = (self.elapsed / self.duration).min(1.);
        let [a, b, c, d] = self.points;
        let p = a.lerp(b, t).lerp(b.lerp(c, t), t);
        let q = b.lerp(c, t).lerp(c.lerp(d, t), t);
        (p.lerp(q, t), q - p, t)
    }
}

impl Battle {
    pub fn open_air_station(&mut self, first_raid: AirRaid) {
        self.air_station = Some(AirStation {
            port: 0,
            aircraft: None,
            bombers: first_raid == AirRaid::Bomb,
            next: first_raid,
            rearm: 10.,
        });
    }

    pub fn air_station(&self) -> Option<&AirStation> {
        self.air_station.as_ref()
    }

    pub(super) fn fly_air_raid(&mut self, dt: f32, player: Vec2, way: Vec2) {
        let Some(base) = &mut self.air_station else {
            return;
        };
        let port = &self.ports[base.port];
        let forward = port.body.forward();
        let slip = base.slip(port);
        let Some(plane) = &mut base.aircraft else {
            base.rearm = (base.rearm - dt).max(0.);
            if base.rearm == 0. && base.operational(port) && port.has_contact {
                let axis = (player - slip).normalize_or(forward);
                let entry = player - axis * 150. + axis.perp() * 42.;
                base.aircraft = Some(AttackPlane {
                    body: body(&mut self.serial, slip, port.body.heading),
                    altitude: 2.,
                    bank: 0.,
                    raid: base.next,
                    health: super::flak::ATTACK_STRENGTH,
                    way: Vec2::ZERO,
                    flight: Flight::Out(Transit::new(slip, forward, entry, axis)),
                });
                base.next = if base.bombers && base.next == AirRaid::Strafe {
                    AirRaid::Bomb
                } else {
                    AirRaid::Strafe
                };
            }
            return;
        };
        // Rise on the approach to an island, especially the stacks in Pilot's Cut.
        let clearance = |at: Vec2| {
            self.landmarks
                .iter()
                .flat_map(|island| island.volumes())
                .map(|(center, radius, top)| {
                    top + 4. - 0.48 * (at.distance(center) - radius).max(0.)
                })
                .fold(0., f32::max)
        };
        let before = plane.body.pos;
        match &mut plane.flight {
            Flight::Out(path) | Flight::Home(path) => {
                let (position, tangent, t) = path.step(dt);
                let heading = bearing(tangent);
                let turn = (heading - plane.body.heading).sin();
                plane.bank = (turn / dt.max(0.001)).clamp(-0.55, 0.55);
                plane.body.pos = position;
                plane.body.heading = heading;
                let outward = matches!(plane.flight, Flight::Out(_));
                let end_height = plane.raid.pass_height();
                let fraction = if outward { t } else { 1. - t };
                let height =
                    2. + (end_height - 2.) * fraction + 25. * (std::f32::consts::PI * t).sin();
                plane.altitude = height.max(clearance(position));
                if t == 1. {
                    if outward {
                        let distance = plane.body.pos.distance(player);
                        let lead = match plane.raid {
                            AirRaid::Strafe => ((distance - 64.) / SPEED).max(0.) + 0.3,
                            AirRaid::Bomb => distance / SPEED + 1.8,
                        };
                        let mark = player + way * lead;
                        plane.body.heading = bearing(mark - plane.body.pos);
                        plane.flight = Flight::Pass { mark, released: 0 };
                    } else {
                        base.aircraft = None;
                        base.rearm = 22.;
                    }
                }
            }
            Flight::Pass { mark, released } => {
                let axis = plane.body.forward();
                plane.bank = 0.;
                plane.body.pos += axis * SPEED * dt;
                plane.altitude = plane.raid.pass_height().max(clearance(plane.body.pos));
                let left = (*mark - plane.body.pos).dot(axis);
                match plane.raid {
                    AirRaid::Strafe => {
                        // The burst walks across the fix committed to on the way in.
                        const RANGES: [f32; 5] = [92., 78., 64., 50., 36.];
                        while *released < RANGES.len() && left <= RANGES[*released] {
                            let strike = *mark + axis * ((*released as f32 - 2.) * 6.);
                            let muzzle =
                                Vec3::new(plane.body.pos.x, plane.altitude, plane.body.pos.y);
                            fire(
                                &mut self.shots,
                                &mut self.effects,
                                &mut self.serial,
                                muzzle,
                                strike.extend(1.).xzy(),
                                Battery::Aircraft,
                                2,
                            );
                            *released += 1;
                        }
                    }
                    AirRaid::Bomb if *released == 0 && left <= 0. => {
                        self.bombs
                            .push(Bomb::new(plane.body.pos.extend(plane.altitude).xzy(), true));
                        *released = 1;
                    }
                    AirRaid::Bomb => {}
                }
                if left < -65. {
                    plane.flight = Flight::Home(Transit::new(plane.body.pos, axis, slip, -forward));
                }
            }
        }
        if let Some(plane) = &mut base.aircraft
            && dt > 0.
        {
            plane.way = (plane.body.pos - before) / dt;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Chart, Events, Hull};

    #[test]
    fn reported_contacts_draw_dodgeable_passes_and_a_ruined_station_cannot_rearm() {
        for raid in [AirRaid::Strafe, AirRaid::Bomb] {
            for dodge in [false, true] {
                let chart = Chart::classic([
                    Vec2::NEG_Y * 180.,
                    Vec2::X * 270.,
                    Vec2::Y * 340.,
                    Vec2::NEG_X * 290.,
                    Vec2::new(-220., -170.),
                ]);
                let mut battle = Battle::for_chart(200., &chart);
                battle.raiders.clear();
                battle.landmarks.clear();
                battle.open_air_station(raid);
                let port = &battle.ports[0];
                let mark = port.factory_position(0) + port.body.forward() * 185.
                    - port.body.forward().perp() * 55.;
                battle.fly_air_raid(12., mark, Vec2::ZERO);
                assert!(battle.air_station().unwrap().aircraft.is_none());
                battle.ports[0].has_contact = true;
                battle.keel = (Vec2::NEG_Y, Hull::CRUISER);
                let mut player = mark;
                let mut seen = false;
                let mut weapon_released = false;
                let mut hits = Events::default();
                let dt = 1. / 60.;
                for _ in 0..60 * 64 {
                    battle.fly_air_raid(dt, player, Vec2::ZERO);
                    if let Some(plane) = &battle.air_station().unwrap().aircraft {
                        seen = true;
                        assert_eq!(plane.raid, raid);
                        if matches!(plane.flight, Flight::Pass { .. }) {
                            if dodge {
                                player = mark + plane.body.forward().perp() * 46.;
                            }
                            // A sortie already aloft finishes its pass.
                            battle.ports[0].damage_factory(0, u16::MAX);
                        }
                    }
                    weapon_released |= match raid {
                        AirRaid::Bomb => !battle.bombs.is_empty(),
                        AirRaid::Strafe => !battle.shots.is_empty(),
                    };
                    battle.advance_shots(dt, player, &mut hits);
                    battle.fly_floatplane(dt, player, 0., &mut hits);
                }
                assert!(seen && weapon_released);
                assert_eq!(hits.damage > 0, !dodge, "{raid:?}, dodge {dodge}: {hits:?}");
                assert_eq!(
                    hits.blows.iter().map(|hit| hit.damage).sum::<u16>(),
                    hits.damage
                );
                assert!(battle.air_station().unwrap().aircraft.is_none());
            }
        }
    }
}
