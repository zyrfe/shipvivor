//! Breakwater's own strike craft: the floatplanes her catapults launch to bomb raiders
//! near the ship, and the racks of homing missiles that fire on their own.
//!
//! Both are fitted from the design and fight only while their compartment's gear is
//! in service. The yard can put up to three catapults on the plane section and three
//! launchers on each rack, all sharing their section's gear, and give each plane a
//! stick of bombs and each launcher a bigger salvo. The renderer reads the planes, their
//! bombs and the missiles from here.
use super::{Battle, Body, Effect, EffectKind, Events, bearing, body, coast, entry};
use glam::{Vec2, Vec3};
use shipvivor_compass::direction;
use shipvivor_shipyard::{
    BOMB_DAMAGE, Berth, Component, Design, FLOATPLANE_REACH, MISSILE_DAMAGE, MISSILE_RANGE,
    MISSILE_RELOAD, SORTIE_SECONDS, Ship, StrikeCraft,
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
/// Seconds aboard before the first launch of a voyage, and between the first launches
/// of the planes of one section, so they go up one after another.
const FIRST_LAUNCH: f32 = 2.5;
const LAUNCH_STAGGER: f32 = 2.;
/// Seconds between the bombs of one stick: about seven metres apart at the plane's speed.
const STICK_INTERVAL: f32 = 0.16;

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

/// Where each catapult on a plane section stands, in metres forward of the section's
/// middle and to starboard, for as many as the section carries. Two stand abreast,
/// staggered so their wings clear, and a third on the centreline goes ahead of them.
pub fn catapult_layout(planes: usize) -> &'static [(f32, f32)] {
    match planes {
        0 => &[],
        1 => &[(0., 0.)],
        2 => &[(0.55, -1.45), (-0.55, 1.45)],
        _ => &[(-0.75, -1.6), (0.9, 0.), (-0.75, 1.6)],
    }
}
/// Planes on plane section `section` of `sections`, when she carries `planes` in all.
/// They are shared out as evenly as they go, the forward sections taking any over.
pub fn planes_on(planes: usize, sections: usize, section: usize) -> usize {
    if sections == 0 {
        return 0;
    }
    planes / sections + usize::from(section < planes % sections)
}
/// Her place in the ship's list of floatplanes of the plane on catapult `slot` of plane
/// section `section`, if that catapult carries one.
pub fn plane_index(planes: usize, sections: usize, section: usize, slot: usize) -> Option<usize> {
    (slot < planes_on(planes, sections, section)).then(|| {
        (0..section)
            .map(|s| planes_on(planes, sections, s))
            .sum::<usize>()
            + slot
    })
}
/// Where each launcher on a missile rack stands, in metres forward of the section's
/// middle and to starboard. Two stand abreast, a third on the centreline ahead, and a
/// fourth makes two pairs, each far enough from the others that their boxes clear on
/// any bearing.
pub fn launcher_layout(launchers: usize) -> &'static [(f32, f32)] {
    match launchers {
        0 => &[],
        1 => &[(0., 0.)],
        2 => &[(0., -1.4), (0., 1.4)],
        3 => &[(-1.15, -1.35), (-1.15, 1.35), (1.15, 0.)],
        _ => &[(-1.2, -1.35), (-1.2, 1.35), (1.2, -1.35), (1.2, 1.35)],
    }
}

/// One of the ship's floatplanes.
#[derive(Clone, Debug)]
pub struct Floatplane {
    pub position: Vec2,
    pub heading: f32,
    pub altitude: f32,
    pub bank: f32,
    pub flight: Flight,
    /// Bombs it carries.
    pub bombs: usize,
    /// It has begun dropping its stick, and the seconds before the next bomb goes.
    stick: Option<f32>,
    catapult: Berth,
    /// The catapult's place on deck: metres forward of the ship's position, then to starboard.
    station: (f32, f32),
    in_service: bool,
    /// Angle around the circuit, in radians.
    phase: f32,
}
impl Floatplane {
    /// A plane on its catapult, launched after `rearm` seconds with `bombs` aboard.
    fn on_catapult(catapult: Berth, station: (f32, f32), rearm: f32, bombs: usize) -> Self {
        Self {
            position: Vec2::ZERO,
            heading: 0.,
            altitude: CATAPULT_DECK,
            bank: 0.,
            flight: Flight::Aboard { rearm },
            bombs,
            stick: None,
            catapult,
            station,
            in_service: true,
            phase: 0.,
        }
    }
    /// It carries a bomb still to drop.
    pub fn armed(&self) -> bool {
        self.bombs > 0
    }
    /// The catapult's place on deck: metres forward of the ship's position, then to
    /// starboard.
    pub fn station(&self) -> (f32, f32) {
        self.station
    }
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
    /// Metres forward of the ship's position, then to starboard.
    pub fn station(&self) -> (f32, f32) {
        self.station
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
    pub fn floatplanes(&self) -> &[Floatplane] {
        &self.floatplanes
    }
    /// How the strike craft are fitted out.
    pub fn strike_craft(&self) -> StrikeCraft {
        self.strike
    }
    /// Fit the strike craft out so. It takes hold at the next [`Battle::fit_design`].
    pub fn fit_strike(&mut self, craft: StrikeCraft) {
        self.strike = craft;
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

    /// Ship the catapults and missile racks a design carries, as many to a section as
    /// the strike craft are fitted with. A plane or launcher still in the same place
    /// keeps its state; one in a new place starts fresh.
    pub(super) fn fit_strike_craft(&mut self, design: &Design) {
        let craft = self.strike;
        // Each piece of gear stands at its place on its section.
        let spread = |component, layout: &'static [(f32, f32)]| {
            stations(design, component)
                .into_iter()
                .flat_map(move |(berth, (fore, abeam))| {
                    layout
                        .iter()
                        .map(move |&(f, a)| (berth, (fore + f, abeam + a)))
                })
        };
        // The planes are shared out between the plane sections.
        let sections = stations(design, Component::Catapult);
        let count = sections.len();
        let catapults =
            sections
                .into_iter()
                .enumerate()
                .flat_map(move |(k, (berth, (fore, abeam)))| {
                    catapult_layout(planes_on(craft.planes, count, k))
                        .iter()
                        .map(move |&(f, a)| (berth, (fore + f, abeam + a)))
                });
        let old = std::mem::take(&mut self.floatplanes);
        self.floatplanes = catapults
            .take(craft.planes)
            .enumerate()
            .map(|(i, (catapult, station))| {
                old.iter().find(|p| p.station == station).map_or_else(
                    || {
                        let rearm = FIRST_LAUNCH + LAUNCH_STAGGER * i as f32;
                        Floatplane::on_catapult(catapult, station, rearm, craft.bombs)
                    },
                    |p| Floatplane {
                        catapult,
                        ..p.clone()
                    },
                )
            })
            .collect();
        let old = std::mem::take(&mut self.launchers);
        self.launchers = spread(Component::MissileRack, launcher_layout(craft.launchers))
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
        for plane in &mut self.floatplanes {
            plane.in_service = ship.ready(plane.catapult);
        }
        for launcher in &mut self.launchers {
            launcher.in_service = ship.ready(launcher.berth);
        }
    }

    /// The afloat raiders within `reach` of `from` that the sea leads to, nearest first.
    fn quarries(&self, from: Vec2, reach: f32) -> Vec<u64> {
        let mut near: Vec<_> = self
            .raiders
            .iter()
            .filter(|r| r.afloat() && r.body.pos.distance_squared(from) <= reach * reach)
            .filter(|r| coast::open_water(from, r.body.pos, &self.ports, &self.landmarks))
            .map(|r| (r.body.pos.distance_squared(from), r.body.id))
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        near.into_iter().map(|(_, id)| id).collect()
    }
    /// The nearest afloat raider within `reach` of `from` that the sea leads to.
    fn quarry(&self, from: Vec2, reach: f32) -> Option<u64> {
        self.quarries(from, reach).first().copied()
    }

    pub(super) fn fly_floatplanes(
        &mut self,
        dt: f32,
        player: Vec2,
        heading: f32,
        events: &mut Events,
    ) {
        self.drop_bombs(dt, player, events);
        let quarries = self.quarries(player, FLOATPLANE_REACH);
        let load = self.strike.bombs;
        let mut planes = std::mem::take(&mut self.floatplanes);
        for i in 0..planes.len() {
            // Each plane goes for the nearest raider no other plane is already after.
            let taken = |id: u64| {
                planes
                    .iter()
                    .enumerate()
                    .any(|(j, p)| j != i && p.flight == Flight::Attack { target: id })
            };
            let quarry = quarries
                .iter()
                .copied()
                .find(|&id| !taken(id))
                .or(quarries.first().copied());
            self.fly_floatplane(&mut planes[i], dt, player, heading, quarry, load);
        }
        self.floatplanes = planes;
    }

    fn fly_floatplane(
        &mut self,
        plane: &mut Floatplane,
        dt: f32,
        player: Vec2,
        heading: f32,
        quarry: Option<u64>,
        load: usize,
    ) {
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
                    plane.bombs = load;
                    plane.stick = None;
                    plane.phase = bearing(direction(heading)) - FRAC_PI_2;
                    Flight::Patrol
                };
            }
            Flight::Patrol => {
                if !plane.armed() && plane.in_service {
                    plane.flight = Flight::Return;
                } else if let (true, Some(target)) = (plane.armed(), quarry) {
                    plane.flight = Flight::Attack { target };
                }
                plane.phase = (plane.phase + dt * PLANE_SPEED / CIRCUIT).rem_euclid(TAU);
                // Aim a little ahead around the circuit, so the plane flies round it.
                let ahead = plane.phase + 0.5;
                let goal = player + Vec2::new(ahead.cos(), ahead.sin()) * CIRCUIT;
                plane.steer(dt, goal);
                plane.climb_toward(dt, CRUISE_ALTITUDE);
            }
            // Once the stick has begun the plane holds its run until the last bomb goes.
            Flight::Attack { .. } if plane.stick.is_some() => {
                let straight = plane.position + direction(plane.heading) * 100.;
                plane.steer(dt, straight);
                let wait = plane.stick.unwrap_or(0.) - dt;
                if wait > 0. {
                    plane.stick = Some(wait);
                    return;
                }
                self.bombs.push(Bomb::new(plane.position_3d(), false));
                plane.bombs = plane.bombs.saturating_sub(1);
                plane.stick = Some(wait + STICK_INTERVAL);
                if plane.bombs == 0 {
                    plane.stick = None;
                    plane.flight = Flight::Return;
                }
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
                // The bombs drop straight down, so aim where the raider will be when they
                // land, and open the stick early so that its middle falls on her.
                let fall = (2. * plane.altitude / BOMB_GRAVITY).sqrt();
                let aim = raider.body.pos + raider.way * fall;
                plane.steer(dt, aim);
                plane.climb_toward(dt, ATTACK_ALTITUDE);
                let offset = aim - plane.position;
                let ahead = direction(plane.heading);
                let along = offset.dot(ahead);
                let across = offset.dot(ahead.perp()).abs();
                let low = plane.altitude <= ATTACK_ALTITUDE + 4.;
                let lead = PLANE_SPEED * STICK_INTERVAL * plane.bombs.saturating_sub(1) as f32 / 2.;
                if low && along < 1. + lead && along > -2. + lead && across < 2. {
                    self.bombs.push(Bomb::new(plane.position_3d(), false));
                    plane.bombs = plane.bombs.saturating_sub(1);
                    if plane.bombs == 0 {
                        plane.flight = Flight::Return;
                    } else {
                        plane.stick = Some(STICK_INTERVAL);
                    }
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
                        breach: false,
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
                raider.hit_hull(at, f32::from(BOMB_DAMAGE));
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
        let salvo = self.strike.salvo;
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
                launcher.salvo = (salvo, 0.);
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
                .flat_map(|p| p.shores())
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
                    self.raiders[i].hit_hull(from.lerp(to, t), f32::from(MISSILE_DAMAGE));
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
    use shipvivor_shipyard::{Frame, MISSILE_SALVO, Module};

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
        fitted(ship, StrikeCraft::STOCK)
    }

    /// A battle with her strike craft fitted out as `craft`.
    fn fitted(ship: &Ship, craft: StrikeCraft) -> Battle {
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
        b.fit_strike(craft);
        b.fit_design(ship.design(), |_| (MainShell::STOCK, TurretLayout::Twin));
        b.update_mount_service(ship);
        b
    }

    /// Run the plane and its bombs for `seconds` with the ship stopped at `OPEN`.
    fn fly(b: &mut Battle, seconds: f32) -> Events {
        let mut events = Events::default();
        for _ in 0..(seconds / TICK) as usize {
            b.fly_floatplanes(TICK, OPEN, 0., &mut events);
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
        assert!(b.floatplanes().first().is_none() && b.launchers().is_empty());
        let b = battle(&armed(&[Component::Catapult, Component::MissileRack]));
        assert!(b.floatplanes().first().is_some_and(Floatplane::aboard));
        assert_eq!(b.launchers().len(), 1);
    }

    #[test]
    fn the_floatplane_bombs_a_raider_in_reach_and_comes_back_to_rearm() {
        let ship = armed(&[Component::Catapult]);
        let mut b = battle(&ship);
        // With nothing to bomb, it is launched and flies the circuit.
        fly(&mut b, 8.);
        let plane = b.floatplanes().first().unwrap();
        assert_eq!(plane.flight, Flight::Patrol);
        assert!(plane.altitude > 20. && plane.position.distance(OPEN) < CIRCUIT + 60.);

        b.spawn(EnemyClass::Freighter, OPEN + Vec2::new(380., -120.), OPEN);
        let hull = b.raiders[0].hull();
        let mut hits = 0;
        let mut landed = false;
        for _ in 0..60 {
            hits += fly(&mut b, 1.).hits;
            landed |= b.floatplanes().first().unwrap().aboard();
            if hits > 0 && landed {
                break;
            }
        }
        assert_eq!(hits, 1);
        assert!(b.raiders[0].hull() < hull);
        assert!(landed, "the plane comes back aboard to rearm");
        let plane = b.floatplanes().first().unwrap();
        assert!(!plane.armed() || plane.aboard());
        // It goes out again once armed.
        fly(&mut b, SORTIE_SECONDS + 1.);
        assert!(!b.floatplanes().first().unwrap().aboard());
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
        assert_eq!(b.floatplanes().first().unwrap().flight, Flight::Patrol);
    }

    #[test]
    fn a_wrecked_catapult_grounds_the_plane_and_cannot_recover_it() {
        let mut ship = armed(&[Component::Catapult]);
        let berth = stations(ship.design(), Component::Catapult)[0].0;
        ship.strike_fitting(berth, 1000.);
        assert!(!ship.ready(berth));
        let mut b = battle(&ship);
        fly(&mut b, 10.);
        assert!(b.floatplanes().first().unwrap().aboard());

        // Aloft when the catapult is lost, it circles without landing.
        let mut ship = armed(&[Component::Catapult]);
        let mut b = battle(&ship);
        fly(&mut b, 6.);
        assert!(!b.floatplanes().first().unwrap().aboard());
        ship.strike_fitting(berth, 1000.);
        b.update_mount_service(&ship);
        b.floatplanes[0].bombs = 0;
        fly(&mut b, 20.);
        assert_eq!(b.floatplanes().first().unwrap().flight, Flight::Patrol);
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

    #[test]
    fn more_planes_go_up_one_after_another_and_hunt_different_raiders() {
        let ship = armed(&[Component::Catapult]);
        let craft = StrikeCraft {
            planes: 3,
            ..StrikeCraft::STOCK
        };
        let mut b = fitted(&ship, craft);
        let planes = b.floatplanes();
        assert_eq!(planes.len(), 3);
        // Each has a catapult of its own on the section, all sharing its gear.
        let stations: Vec<_> = planes.iter().map(Floatplane::station).collect();
        assert!(stations[0] != stations[1] && stations[1] != stations[2]);
        assert!(planes.iter().all(|p| p.catapult == planes[0].catapult));
        // The first is launched before the others.
        fly(&mut b, FIRST_LAUNCH + 0.2);
        let aloft = |b: &Battle| b.floatplanes().iter().filter(|p| !p.aboard()).count();
        assert_eq!(aloft(&b), 1);
        fly(&mut b, 2. * LAUNCH_STAGGER);
        assert_eq!(aloft(&b), 3);

        b.spawn(EnemyClass::Freighter, OPEN + Vec2::new(380., -120.), OPEN);
        b.spawn(EnemyClass::Freighter, OPEN + Vec2::new(-300., 260.), OPEN);
        fly(&mut b, 0.5);
        let targets: Vec<_> = b
            .floatplanes()
            .iter()
            .filter_map(|p| match p.flight {
                Flight::Attack { target } => Some(target),
                _ => None,
            })
            .collect();
        assert_eq!(targets.len(), 3);
        assert!(targets.contains(&b.raiders[0].body.id) && targets.contains(&b.raiders[1].body.id));
        // A wrecked catapult grounds every plane on the section.
        let mut ship = ship;
        ship.strike_fitting(b.floatplanes()[0].catapult, 1000.);
        b.update_mount_service(&ship);
        assert!(b.floatplanes().iter().all(|p| !p.in_service()));
    }

    #[test]
    fn a_plane_with_more_bombs_drops_them_in_a_stick_on_one_pass() {
        let ship = armed(&[Component::Catapult]);
        let craft = StrikeCraft {
            bombs: 3,
            ..StrikeCraft::STOCK
        };
        let mut b = fitted(&ship, craft);
        fly(&mut b, FIRST_LAUNCH + 0.2);
        assert_eq!(b.floatplanes()[0].bombs, 3);
        b.spawn(EnemyClass::Battleship, OPEN + Vec2::new(380., -120.), OPEN);
        let hull = b.raiders[0].hull();
        let mut hits = 0;
        let mut most_falling = 0;
        for _ in 0..(40. / TICK) as usize {
            hits += fly(&mut b, TICK).hits;
            most_falling = most_falling.max(b.bombs().len());
            if b.floatplanes()[0].flight == Flight::Return && b.bombs().is_empty() {
                break;
            }
        }
        // All three are falling at once, and the plane turns for home with none left.
        assert_eq!(most_falling, 3);
        assert_eq!(b.floatplanes()[0].bombs, 0);
        assert_eq!(b.floatplanes()[0].flight, Flight::Return);
        assert!(hits >= 1 && b.raiders[0].hull() < hull);
        let fell = b
            .effects
            .iter()
            .filter(|fx| matches!(fx.kind, EffectKind::Burst | EffectKind::Splash))
            .count();
        assert_eq!(fell, 3);
    }

    #[test]
    fn more_launchers_stand_on_the_rack_and_fire_bigger_salvos() {
        let ship = armed(&[Component::MissileRack]);
        let craft = StrikeCraft {
            launchers: 3,
            salvo: 4,
            ..StrikeCraft::STOCK
        };
        let mut b = fitted(&ship, craft);
        assert_eq!(b.launchers().len(), 3);
        let stations: Vec<_> = b.launchers().iter().map(Launcher::station).collect();
        assert!(
            stations[0] != stations[1] && stations[1] != stations[2] && stations[0] != stations[2]
        );
        assert_eq!(stations.len(), launcher_layout(3).len());
        b.spawn(EnemyClass::Battleship, OPEN + Vec2::new(520., 260.), OPEN);
        // Every launcher ripples off its four within a few seconds.
        let early = shoot(&mut b, 3.5);
        assert_eq!(usize::from(early.hits) + b.missiles().len(), 3 * 4);
        // Nine hits are enough to sink her, and what is still flying runs on past her.
        let hits = early.hits + shoot(&mut b, 8.).hits;
        assert!(hits >= 9);
        assert!(!b.raiders[0].afloat() || hits == 12);
        // Refitted back to stock, the rack keeps the launcher in the middle of the section.
        b.fit_strike(StrikeCraft::STOCK);
        b.fit_design(ship.design(), |_| (MainShell::STOCK, TurretLayout::Twin));
        assert_eq!(b.launchers().len(), 1);
        assert_eq!(MISSILE_SALVO, StrikeCraft::STOCK.salvo);
    }
}
