//! Breakwater's own strike craft: the floatplane a catapult launches to bomb raiders
//! near the ship, and the racks of homing missiles that fire on their own.
//!
//! Both are fitted from the design and fight only while their compartment's gear is
//! in service. The renderer reads the plane, its bombs and the missiles from here.
use super::{Battle, Body, Effect, EffectKind, Events, ISLAND_RADIUS, bearing, body, coast, entry};
use glam::{Vec2, Vec3};
use shipvivor_compass::direction;
use shipvivor_shipyard::{
    BOMB_DAMAGE, Berth, Component, Design, FLOATPLANE_REACH, MISSILE_DAMAGE, MISSILE_RANGE,
    MISSILE_RELOAD, MISSILE_SALVO, SORTIE_SECONDS, Ship,
};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Metres per second the floatplane flies, and the radians per second it can turn.
const PLANE_SPEED: f32 = 44.;
const PLANE_TURN: f32 = 1.1;
/// Height of the circuit flown around the ship, and of a bombing run.
const CRUISE_ALTITUDE: f32 = 42.;
const ATTACK_ALTITUDE: f32 = 18.;
/// Metres per second the plane climbs or descends.
const CLIMB: f32 = 9.;
/// Radius of the circuit the plane flies around the ship while it has nothing to bomb.
const CIRCUIT: f32 = 110.;
/// Height of the plane sitting on its catapult, above the ship's waterline.
pub const CATAPULT_DECK: f32 = 2.75;
/// The plane is hoisted aboard once it is this close to the catapult.
const RECOVERY: f32 = 12.;
/// A bomb falls under this acceleration, a little faster than a shell's arc reads.
const BOMB_GRAVITY: f32 = 14.;
/// Seconds aboard before the first launch of a voyage.
const FIRST_LAUNCH: f32 = 2.5;

/// Missiles fly at this speed, turn this fast and skim the sea at this height.
const MISSILE_SPEED: f32 = 95.;
const MISSILE_TURN: f32 = 2.4;
const MISSILE_CRUISE: f32 = 5.;
/// A missile runs this far past its rated range before it falls into the sea.
const MISSILE_ENDURANCE: f32 = 1.3;
/// Seconds between the missiles of one salvo.
const RIPPLE: f32 = 0.35;
/// Radians per second the box launcher trains, and how close it must be to fire.
const LAUNCHER_TRAIN: f32 = 1.6;
const LAUNCHER_LAID: f32 = 0.3;
/// Height of a launcher's cells above the waterline.
const LAUNCHER_DECK: f32 = 2.9;

/// What the floatplane is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Flight {
    /// On the catapult, being armed. It is launched once this many seconds are up.
    Aboard { rearm: f32 },
    /// Flying the circuit around the ship.
    Patrol,
    /// Running in on a raider to drop its bomb.
    Attack { target: u64 },
    /// Flying back to be recovered and armed again.
    Return,
}

/// The ship's floatplane.
#[derive(Clone, Debug)]
pub struct Floatplane {
    pub position: Vec2,
    pub heading: f32,
    pub altitude: f32,
    pub bank: f32,
    pub flight: Flight,
    /// It carries a bomb.
    pub armed: bool,
    catapult: Berth,
    /// The catapult's place on deck: metres forward of the ship's position, then to starboard.
    station: (f32, f32),
    in_service: bool,
    /// Angle around the circuit, in radians.
    phase: f32,
}
impl Floatplane {
    pub fn aboard(&self) -> bool {
        matches!(self.flight, Flight::Aboard { .. })
    }
    pub fn position_3d(&self) -> Vec3 {
        Vec3::new(self.position.x, self.altitude, self.position.y)
    }
    /// The catapult can launch and recover the plane.
    pub fn in_service(&self) -> bool {
        self.in_service
    }
    /// Fly toward `goal` at the plane's speed, turning no faster than it can.
    fn steer(&mut self, dt: f32, goal: Vec2) {
        let offset = goal - self.position;
        let error = wrap(bearing(offset) - self.heading);
        let turn_radius = PLANE_SPEED / PLANE_TURN;
        // A point inside the turning circle is passed and come back to, not orbited.
        let hold = offset.length() < 2.2 * turn_radius && error.abs() > 1.2;
        let turn = if hold {
            0.
        } else {
            error.clamp(-PLANE_TURN * dt, PLANE_TURN * dt)
        };
        self.heading = (self.heading + turn).rem_euclid(TAU);
        let rate = if dt > 0. {
            turn / (dt * PLANE_TURN)
        } else {
            0.
        };
        // Turning to starboard, the starboard wing goes down.
        self.bank += (-0.6 * rate - self.bank) * (4. * dt).min(1.);
        self.position += direction(self.heading) * PLANE_SPEED * dt;
    }
    fn climb_toward(&mut self, dt: f32, height: f32) {
        self.altitude += (height - self.altitude).clamp(-CLIMB * dt, CLIMB * dt);
    }
}

/// A bomb on its way down.
#[derive(Clone, Copy, Debug)]
pub struct Bomb {
    pub position: Vec3,
    hostile: bool,
    /// Metres per second downward.
    fall: f32,
}
impl Bomb {
    pub(super) fn new(position: Vec3, hostile: bool) -> Self {
        Self {
            position,
            hostile,
            fall: 0.,
        }
    }
}

/// One trainable box launcher.
#[derive(Clone, Debug)]
pub struct Launcher {
    berth: Berth,
    /// Metres forward of the ship's position, then to starboard.
    station: (f32, f32),
    /// Bearing of the cells relative to the ship's head, in radians.
    pub train: f32,
    /// Seconds before the next salvo can be fired.
    pub reload: f32,
    in_service: bool,
    /// Missiles of the salvo now firing still to go, and the seconds before the next.
    salvo: (usize, f32),
    target: Option<u64>,
}
impl Launcher {
    pub fn in_service(&self) -> bool {
        self.in_service
    }
}

/// A missile in flight.
#[derive(Clone, Copy, Debug)]
pub struct Missile {
    pub body: Body,
    pub altitude: f32,
    /// Seconds since it left the launcher.
    pub age: f32,
    target: u64,
    remaining: f32,
}

fn wrap(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

/// Where a point given as metres forward and to starboard of the ship stands on the chart.
fn on_deck(player: Vec2, heading: f32, (fore, abeam): (f32, f32)) -> Vec2 {
    player + direction(heading) * fore + direction(heading + FRAC_PI_2) * abeam
}

/// Every fitted component of a kind, and where it stands on deck.
fn stations(design: &Design, component: Component) -> Vec<(Berth, (f32, f32))> {
    design
        .fittings()
        .into_iter()
        .filter(|f| f.component == component)
        .map(|f| (f.berth, (super::Hull::CRUISER.bow - f.aft, f.abeam)))
        .collect()
}

impl Battle {
    pub fn floatplane(&self) -> Option<&Floatplane> {
        self.floatplane.as_ref()
    }
    pub fn bombs(&self) -> &[Bomb] {
        &self.bombs
    }
    pub fn launchers(&self) -> &[Launcher] {
        &self.launchers
    }
    pub fn missiles(&self) -> &[Missile] {
        &self.missiles
    }

    /// Ship the catapult and missile racks a design carries. A plane or rack still
    /// in the same place keeps its state; one in a new place starts fresh.
    pub(super) fn fit_strike_craft(&mut self, design: &Design) {
        self.floatplane =
            stations(design, Component::Catapult)
                .first()
                .map(|&(catapult, station)| match self.floatplane.take() {
                    Some(plane) if plane.station == station => Floatplane { catapult, ..plane },
                    _ => Floatplane {
                        position: Vec2::ZERO,
                        heading: 0.,
                        altitude: CATAPULT_DECK,
                        bank: 0.,
                        flight: Flight::Aboard {
                            rearm: FIRST_LAUNCH,
                        },
                        armed: true,
                        catapult,
                        station,
                        in_service: true,
                        phase: 0.,
                    },
                });
        let old = std::mem::take(&mut self.launchers);
        self.launchers = stations(design, Component::MissileRack)
            .into_iter()
            .map(|(berth, station)| {
                old.iter().find(|l| l.station == station).map_or_else(
                    || Launcher {
                        berth,
                        station,
                        train: 0.,
                        reload: 0.,
                        in_service: true,
                        salvo: (0, 0.),
                        target: None,
                    },
                    |l| Launcher { berth, ..l.clone() },
                )
            })
            .collect();
    }

    pub(super) fn strike_craft_service(&mut self, ship: &Ship) {
        if let Some(plane) = &mut self.floatplane {
            plane.in_service = ship.ready(plane.catapult);
        }
        for launcher in &mut self.launchers {
            launcher.in_service = ship.ready(launcher.berth);
        }
    }

    /// The nearest afloat raider within `reach` of `from` that the sea leads to.
    fn quarry(&self, from: Vec2, reach: f32) -> Option<u64> {
        self.raiders
            .iter()
            .filter(|r| r.afloat() && r.body.pos.distance_squared(from) <= reach * reach)
            .filter(|r| coast::open_water(from, r.body.pos, &self.ports, &self.landmarks))
            .min_by(|a, b| {
                a.body
                    .pos
                    .distance_squared(from)
                    .total_cmp(&b.body.pos.distance_squared(from))
            })
            .map(|r| r.body.id)
    }

    pub(super) fn fly_floatplane(
        &mut self,
        dt: f32,
        player: Vec2,
        heading: f32,
        events: &mut Events,
    ) {
        self.drop_bombs(dt, player, events);
        let quarry = self.quarry(player, FLOATPLANE_REACH);
        let Some(plane) = &mut self.floatplane else {
            return;
        };
        let catapult = on_deck(player, heading, plane.station);
        match plane.flight {
            Flight::Aboard { rearm } => {
                plane.position = catapult;
                plane.heading = heading;
                plane.altitude = CATAPULT_DECK;
                plane.bank = 0.;
                if !plane.in_service {
                    return;
                }
                let rearm = rearm - dt;
                plane.flight = if rearm > 0. {
                    Flight::Aboard { rearm }
                } else {
                    plane.armed = true;
                    plane.phase = bearing(direction(heading)) - FRAC_PI_2;
                    Flight::Patrol
                };
            }
            Flight::Patrol => {
                if !plane.armed && plane.in_service {
                    plane.flight = Flight::Return;
                } else if let (true, Some(target)) = (plane.armed, quarry) {
                    plane.flight = Flight::Attack { target };
                }
                plane.phase = (plane.phase + dt * PLANE_SPEED / CIRCUIT).rem_euclid(TAU);
                // Aim a little ahead around the circuit, so the plane flies round it.
                let ahead = plane.phase + 0.5;
                let goal = player + Vec2::new(ahead.cos(), ahead.sin()) * CIRCUIT;
                plane.steer(dt, goal);
                plane.climb_toward(dt, CRUISE_ALTITUDE);
            }
            Flight::Attack { target } => {
                let Some(raider) = self
                    .raiders
                    .iter()
                    .find(|r| r.body.id == target && r.afloat())
                    .filter(|r| r.body.pos.distance(player) < FLOATPLANE_REACH + 150.)
                else {
                    plane.flight = Flight::Patrol;
                    return;
                };
                // The bomb drops straight down, so aim where the raider will be when it lands.
                let fall = (2. * plane.altitude / BOMB_GRAVITY).sqrt();
                let aim = raider.body.pos + raider.way * fall;
                plane.steer(dt, aim);
                plane.climb_toward(dt, ATTACK_ALTITUDE);
                let offset = aim - plane.position;
                let ahead = direction(plane.heading);
                let along = offset.dot(ahead);
                let across = offset.dot(ahead.perp()).abs();
                let low = plane.altitude <= ATTACK_ALTITUDE + 4.;
                if low && along < 1. && along > -2. && across < 2. {
                    self.bombs.push(Bomb::new(plane.position_3d(), false));
                    plane.armed = false;
                    plane.flight = Flight::Return;
                }
            }
            Flight::Return => {
                if !plane.in_service {
                    // With nowhere to land, the plane waits over the ship.
                    plane.flight = Flight::Patrol;
                    return;
                }
                plane.steer(dt, catapult);
                let near = plane.position.distance(catapult);
                plane.climb_toward(
                    dt,
                    if near < 140. {
                        CATAPULT_DECK + 2.
                    } else {
                        CRUISE_ALTITUDE
                    },
                );
                if near < RECOVERY {
                    plane.flight = Flight::Aboard {
                        rearm: SORTIE_SECONDS,
                    };
                }
            }
        }
    }

    fn drop_bombs(&mut self, dt: f32, player: Vec2, events: &mut Events) {
        let mut bursts = Vec::new();
        self.bombs.retain_mut(|bomb| {
            bomb.fall += BOMB_GRAVITY * dt;
            bomb.position.y -= bomb.fall * dt;
            if bomb.position.y > 1.5 {
                return true;
            }
            bursts.push((Vec2::new(bomb.position.x, bomb.position.z), bomb.hostile));
            false
        });
        for (at, hostile) in bursts {
            let kind = if hostile {
                let (forward, hull) = self.keel;
                let offset = at - player;
                let local = Vec2::new(offset.dot(forward.perp()), -offset.dot(forward));
                if super::plating::clip_hull(&hull.outline().map(Vec2::from), local, Vec2::ZERO)
                    .is_some()
                {
                    let damage = 12;
                    events.blows.push(super::Blow {
                        aft: hull.bow + local.y,
                        abeam: local.x,
                        damage,
                        calibre: None,
                        reach: shipvivor_shipyard::Reach::Bays,
                    });
                    events.damage = events.damage.saturating_add(damage);
                    EffectKind::Burst
                } else {
                    EffectKind::Splash
                }
            } else if let Some(raider) = self.raiders.iter_mut().find(|r| {
                let along = r.body.forward() * 0.4;
                r.afloat() && r.hull_span(at - along, at + along).is_some()
            }) {
                raider.hit_hull(at, BOMB_DAMAGE);
                events.hits += 1;
                EffectKind::Burst
            } else {
                EffectKind::Splash
            };
            self.effects.push(Effect {
                body: body(&mut self.serial, at, 0.),
                kind,
                age: 0.,
                height: if kind == EffectKind::Burst { 1.6 } else { 0. },
            });
        }
    }

    pub(super) fn fire_missiles(&mut self, dt: f32, player: Vec2, heading: f32) {
        let quarry = self.quarry(player, MISSILE_RANGE);
        for i in 0..self.launchers.len() {
            let launcher = &mut self.launchers[i];
            launcher.reload = (launcher.reload - dt).max(0.);
            let firing = launcher.salvo.0 > 0;
            if !launcher.in_service {
                launcher.salvo = (0, 0.);
                continue;
            }
            // Between salvos the cells follow the nearest raider in range.
            if !firing {
                launcher.target = quarry;
            }
            let target = launcher.target.and_then(|id| {
                self.raiders
                    .iter()
                    .find(|r| r.body.id == id && r.afloat())
                    .map(|r| r.body.pos)
            });
            let launcher = &mut self.launchers[i];
            let cells = on_deck(player, heading, launcher.station);
            // Lay the cells on the target, or bring them back fore and aft.
            let laid = target.map_or(0., |at| wrap(bearing(at - cells) - heading));
            let error = wrap(laid - launcher.train);
            launcher.train += error.clamp(-LAUNCHER_TRAIN * dt, LAUNCHER_TRAIN * dt);
            let Some(at) = target else {
                launcher.salvo = (0, 0.);
                continue;
            };
            if !firing && launcher.reload == 0. && error.abs() < LAUNCHER_LAID {
                launcher.salvo = (MISSILE_SALVO, 0.);
                launcher.reload = MISSILE_RELOAD;
            }
            let (left, wait) = launcher.salvo;
            if left == 0 {
                continue;
            }
            if wait > dt {
                launcher.salvo.1 -= dt;
                continue;
            }
            launcher.salvo = (left - 1, RIPPLE);
            let side = if left % 2 == 0 { -0.3 } else { 0.3 };
            let line = heading + launcher.train;
            let from = cells + direction(line + FRAC_PI_2) * side;
            let target = launcher.target.expect("a salvo has a target");
            self.missiles.push(Missile {
                body: body(&mut self.serial, from, bearing(at - from)),
                altitude: LAUNCHER_DECK,
                age: 0.,
                target,
                remaining: MISSILE_RANGE * MISSILE_ENDURANCE,
            });
        }
    }

    pub(super) fn advance_missiles(&mut self, dt: f32, events: &mut Events) {
        let mut missiles = std::mem::take(&mut self.missiles);
        missiles.retain_mut(|missile| {
            missile.age += dt;
            // It climbs off the rail, then settles to skim the sea.
            let height = if missile.age < 0.5 {
                LAUNCHER_DECK + 6. * missile.age
            } else {
                MISSILE_CRUISE
            };
            missile.altitude += (height - missile.altitude) * (5. * dt).min(1.);
            if let Some(raider) = self
                .raiders
                .iter()
                .find(|r| r.body.id == missile.target && r.afloat())
            {
                let aim = raider.body.pos + raider.way * 0.25;
                let error = wrap(bearing(aim - missile.body.pos) - missile.body.heading);
                missile.body.heading = (missile.body.heading
                    + error.clamp(-MISSILE_TURN * dt, MISSILE_TURN * dt))
                .rem_euclid(TAU);
            }
            let travel = (MISSILE_SPEED * dt).min(missile.remaining);
            let from = missile.body.pos;
            let to = from + missile.body.forward() * travel;
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
            let struck = match hull {
                Some((t, i)) if land.is_none_or(|l| t < l) => {
                    self.raiders[i].hit_hull(from.lerp(to, t), MISSILE_DAMAGE);
                    events.hits += 1;
                    Some((t, EffectKind::Burst, 1.8))
                }
                _ => land.map(|t| (t, EffectKind::Burst, missile.altitude)),
            };
            let spent = missile.remaining <= travel;
            let ended = struck.or(spent.then_some((1., EffectKind::Splash, 0.)));
            if let Some((t, kind, height)) = ended {
                self.effects.push(Effect {
                    body: body(&mut self.serial, from.lerp(to, t), missile.body.heading),
                    kind,
                    age: 0.,
                    height,
                });
                return false;
            }
            missile.body.pos = to;
            missile.remaining -= travel;
            true
        });
        self.missiles = missiles;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EnemyClass, MainShell, TurretLayout};
    use shipvivor_shipyard::{Frame, Module};

    const TICK: f32 = 1. / 60.;
    /// Well clear of the harbours and islands of the test route.
    const OPEN: Vec2 = Vec2::splat(4000.);

    fn armed(extra: &[Component]) -> Ship {
        let mut design = Design::breakwater();
        for &part in extra {
            let at = design.modules().len() - 2;
            design
                .insert(at, Module::new(Frame::Straight, [part]))
                .unwrap();
        }
        Ship::new(design)
    }

    fn battle(ship: &Ship) -> Battle {
        let mut b = Battle::for_route(
            14.,
            [
                (0., -152.),
                (168., -424.),
                (496., -300.),
                (416., 76.),
                (72., 164.),
            ]
            .map(Vec2::from),
        );
        b.raiders.clear();
        b.landmarks.clear();
        b.fit_design(ship.design(), |_| (MainShell::STOCK, TurretLayout::Twin));
        b.update_mount_service(ship);
        b
    }

    /// Run the plane and its bombs for `seconds` with the ship stopped at `OPEN`.
    fn fly(b: &mut Battle, seconds: f32) -> Events {
        let mut events = Events::default();
        for _ in 0..(seconds / TICK) as usize {
            b.fly_floatplane(TICK, OPEN, 0., &mut events);
        }
        events
    }

    fn shoot(b: &mut Battle, seconds: f32) -> Events {
        let mut events = Events::default();
        for _ in 0..(seconds / TICK) as usize {
            b.fire_missiles(TICK, OPEN, 0.);
            b.advance_missiles(TICK, &mut events);
        }
        events
    }

    #[test]
    fn a_ship_without_the_gear_carries_neither_plane_nor_rack() {
        let b = battle(&armed(&[]));
        assert!(b.floatplane().is_none() && b.launchers().is_empty());
        let b = battle(&armed(&[Component::Catapult, Component::MissileRack]));
        assert!(b.floatplane().is_some_and(Floatplane::aboard));
        assert_eq!(b.launchers().len(), 1);
    }

    #[test]
    fn the_floatplane_bombs_a_raider_in_reach_and_comes_back_to_rearm() {
        let ship = armed(&[Component::Catapult]);
        let mut b = battle(&ship);
        // With nothing to bomb, it is launched and flies the circuit.
        fly(&mut b, 8.);
        let plane = b.floatplane().unwrap();
        assert_eq!(plane.flight, Flight::Patrol);
        assert!(plane.altitude > 20. && plane.position.distance(OPEN) < CIRCUIT + 60.);

        b.spawn(EnemyClass::Freighter, OPEN + Vec2::new(380., -120.), OPEN);
        let hull = b.raiders[0].hull();
        let mut hits = 0;
        let mut landed = false;
        for _ in 0..60 {
            hits += fly(&mut b, 1.).hits;
            landed |= b.floatplane().unwrap().aboard();
            if hits > 0 && landed {
                break;
            }
        }
        assert_eq!(hits, 1);
        assert!(b.raiders[0].hull() < hull);
        assert!(landed, "the plane comes back aboard to rearm");
        let plane = b.floatplane().unwrap();
        assert!(!plane.armed || plane.aboard());
        // It goes out again once armed.
        fly(&mut b, SORTIE_SECONDS + 1.);
        assert!(!b.floatplane().unwrap().aboard());
    }

    #[test]
    fn a_raider_beyond_reach_is_left_alone() {
        let mut b = battle(&armed(&[Component::Catapult]));
        b.spawn(
            EnemyClass::Freighter,
            OPEN + Vec2::new(FLOATPLANE_REACH + 120., 0.),
            OPEN,
        );
        let events = fly(&mut b, 30.);
        assert_eq!(events.hits, 0);
        assert!(b.bombs().is_empty());
        assert_eq!(b.floatplane().unwrap().flight, Flight::Patrol);
    }

    #[test]
    fn a_wrecked_catapult_grounds_the_plane_and_cannot_recover_it() {
        let mut ship = armed(&[Component::Catapult]);
        let berth = stations(ship.design(), Component::Catapult)[0].0;
        ship.strike_fitting(berth, 1000.);
        assert!(!ship.ready(berth));
        let mut b = battle(&ship);
        fly(&mut b, 10.);
        assert!(b.floatplane().unwrap().aboard());

        // Aloft when the catapult is lost, it circles without landing.
        let mut ship = armed(&[Component::Catapult]);
        let mut b = battle(&ship);
        fly(&mut b, 6.);
        assert!(!b.floatplane().unwrap().aboard());
        ship.strike_fitting(berth, 1000.);
        b.update_mount_service(&ship);
        b.floatplane.as_mut().unwrap().armed = false;
        fly(&mut b, 20.);
        assert_eq!(b.floatplane().unwrap().flight, Flight::Patrol);
    }

    #[test]
    fn a_rack_fires_a_salvo_that_strikes_and_then_waits_out_its_reload() {
        let mut b = battle(&armed(&[Component::MissileRack]));
        b.spawn(EnemyClass::Battleship, OPEN + Vec2::new(520., 260.), OPEN);
        let hull = b.raiders[0].hull();
        let events = shoot(&mut b, 9.);
        assert_eq!(events.hits, MISSILE_SALVO as u16);
        assert!(b.raiders[0].hull() < hull);
        assert!(b.missiles().is_empty());
        // No second salvo until the rack is loaded again.
        let reload = b.launchers()[0].reload;
        assert!(reload > 0. && reload < MISSILE_RELOAD);
        let early = shoot(&mut b, reload - 0.5);
        assert_eq!(early.hits, 0);
        assert!(b.missiles().is_empty());
        let late = shoot(&mut b, 10.);
        assert_eq!(late.hits, MISSILE_SALVO as u16);
    }

    #[test]
    fn a_rack_holds_fire_beyond_its_range_and_when_out_of_service() {
        let mut ship = armed(&[Component::MissileRack]);
        let mut b = battle(&ship);
        b.spawn(
            EnemyClass::Cruiser,
            OPEN + Vec2::new(MISSILE_RANGE + 60., 0.),
            OPEN,
        );
        assert_eq!(shoot(&mut b, 5.).hits, 0);
        assert!(b.missiles().is_empty());

        let mut b = battle(&ship);
        b.spawn(EnemyClass::Cruiser, OPEN + Vec2::new(400., 0.), OPEN);
        let berth = stations(ship.design(), Component::MissileRack)[0].0;
        ship.strike_fitting(berth, 1000.);
        b.update_mount_service(&ship);
        assert_eq!(shoot(&mut b, 5.).hits, 0);
        assert!(b.missiles().is_empty());
    }

    #[test]
    fn two_racks_fire_two_salvos_and_a_missile_without_a_mark_falls_in_the_sea() {
        let mut b = battle(&armed(&[Component::MissileRack, Component::MissileRack]));
        assert_eq!(b.launchers().len(), 2);
        b.spawn(EnemyClass::Battleship, OPEN + Vec2::new(0., -600.), OPEN);
        let events = shoot(&mut b, 9.);
        assert_eq!(events.hits, 2 * MISSILE_SALVO as u16);

        // Its mark sunk under it, a missile runs on and falls in the sea.
        let mut b = battle(&armed(&[Component::MissileRack]));
        b.spawn(EnemyClass::Cruiser, OPEN + Vec2::new(600., 0.), OPEN);
        shoot(&mut b, 1.6);
        assert!(!b.missiles().is_empty());
        b.raiders.clear();
        shoot(&mut b, 20.);
        assert!(b.missiles().is_empty());
        assert!(b.effects.iter().any(|fx| fx.kind == EffectKind::Splash));
    }
}
