//! Airfields: a base whose works stand on a flat island beside its harbour. A runway
//! runs along the island, with a taxiway inside it and a row of concrete shelters
//! behind that, each housing one aircraft. Two hangars build the aircraft, the base's
//! factories feed them, a control tower sends them up in wings, and a fuel depot runs
//! trucks out to the shelters. Light bombers and torpedo planes roll out, taxi to the
//! end of the runway and take off, and that is the only time her guns can reach them
//! short of the air: the shelters turn every round.
//!
//! Without its tower the field sends its wings up far less often, and the base's
//! factories raise a fallen tower again, quicker while its own drydock across the water
//! still keeps it supplied. A plane fuels at a truck in a few seconds, and from the
//! shelter's own line, more slowly, when no truck comes.
use super::{Battle, Body, Effect, EffectKind, Events, Port, body};
use glam::{Vec2, Vec3, Vec3Swizzles};

mod warplane;
pub use warplane::Warplane;
#[cfg(test)]
mod tests;

/// The `works` of a harbour site that is an airfield.
pub const AIRFIELD_WORKS: u8 = 4;
/// Metres across the flat island an airfield stands on, from its middle to its edge.
pub const AIRFIELD_RADIUS: f32 = 56.;
/// Height of the island's level top, where everything on it stands.
pub const FIELD_LEVEL: f32 = 3.2;

/// The runway's ends along the island and its line out from the shore, and its half
/// width. Aircraft take off along the island one way and land the other.
pub const RUNWAY: (f32, f32) = (-42., 42.);
pub const RUNWAY_LINE: f32 = 14.;
pub const RUNWAY_HALF_WIDTH: f32 = 3.5;
/// The taxiway's line, between the runway and the shelters, and where it ends.
pub const TAXIWAY_LINE: f32 = 0.;
pub const TAXIWAY_END: f32 = 36.;
/// Each shelter's place along the island, its line, and its half size and height:
/// along, then out from the shore. Its door faces the taxiway.
pub const SHELTERS: [f32; 5] = [-24., -12., 0., 12., 24.];
pub const SHELTER_LINE: f32 = -14.;
pub const SHELTER_SIZE: (f32, f32, f32) = (4., 4.5, 3.4);
/// The two hangars at the ends of the shelters, their half size and height.
pub const HANGARS: [(f32, f32); 2] = [(-42., -10.), (42., -10.)];
pub const HANGAR_SIZE: (f32, f32, f32) = (6., 5.5, 7.5);
/// The control tower behind the shelters, its half size and height.
pub const TOWER: (f32, f32) = (-14., -32.);
pub const TOWER_SIZE: (f32, f32, f32) = (2.2, 2.2, 11.);
/// The fuel depot behind the shelters: three tanks in a row, its half size and height.
pub const DEPOT: (f32, f32) = (10., -32.);
pub const DEPOT_SIZE: (f32, f32, f32) = (8., 3.5, 4.5);
/// The factories behind the hangars.
pub const FACTORIES: [(f32, f32); 2] = [(-30., -28.), (30., -28.)];
/// The light guns round the seaward side, and the two heavy batteries.
pub const LIGHT_GUNS: [(f32, f32); 4] = [(-46., 20.), (46., 20.), (-14., 44.), (14., 44.)];
pub const HEAVY_GUNS: [(f32, f32); 2] = [(-32., 34.), (32., 34.)];
/// The road the trucks run along in front of the depot, and the lanes between the
/// shelters down to their doors.
const DEPOT_ROAD: f32 = -24.;
const DOOR_ROAD: f32 = -6.;

/// What a hangar, the tower and the depot stand before they fall.
const HANGAR_STRENGTH: u16 = 20;
const TOWER_STRENGTH: u16 = 10;
const DEPOT_STRENGTH: u16 = 8;
/// Seconds one hangar takes to build an aircraft with no factory feeding it. Each
/// standing factory builds it half as fast again.
const AIRFRAME_SECONDS: f32 = 30.;
/// Seconds a plane takes to fuel at a truck, and from the shelter's own line.
const TRUCK_FUELLING: f32 = 4.;
const HYDRANT_FUELLING: f32 = 18.;
/// Trucks at the depot, seconds the depot takes to send out another for one lost, and
/// seconds a truck takes to fill up there.
const TRUCKS: usize = 3;
const TRUCK_REPLACEMENT: f32 = 30.;
const TRUCK_LOADING: f32 = 3.;
const TRUCK_SPEED: f32 = 6.;
/// Seconds between wings with the tower standing, and the share of its pace the count
/// toward the next keeps while the tower is down: every second without it holds the
/// next wing back.
const WING_INTERVAL: f32 = 50.;
const TOWERLESS: f32 = 0.4;
/// Seconds after the field stirs before its first wing goes up, and how long a short
/// wing waits for more aircraft before it goes up as it is.
const FIRST_WING: f32 = 15.;
const SHORT_WING_WAIT: f32 = 20.;
/// Seconds between one plane of a wing leaving its shelter and the next.
const STAGGER: f32 = 3.;
/// Seconds the factories take to raise a fallen tower with one of them standing, and
/// how much quicker while the harbour's drydock keeps the field supplied: half a minute
/// with both factories and the dock, two minutes with one factory and no dock.
const TOWER_REBUILD: f32 = 120.;
const SUPPLIED: f32 = 2.;
/// Taxiing speed, the runway's acceleration and the speed a plane lifts off at,
/// and how fast one touches down.
const TAXI_SPEED: f32 = 7.;
const ROLL_ACCELERATION: f32 = 9.;
pub(super) const LIFT_OFF: f32 = 34.;
pub(super) const TOUCH_DOWN: f32 = 26.;
/// Metres out from where a round bursts inside which its splinters wreck a truck or an
/// aircraft on the ground, beside the inch or so a calibre inch adds.
const SPLINTERS: f32 = 2.5;
/// Metres round a burning truck, plane or the depot inside which the blast wrecks
/// the trucks and aircraft on the ground as well.
const TRUCK_BLAST: f32 = 6.;
const DEPOT_BLAST: f32 = 14.;
/// Metres round an aircraft on the ground and a truck that rounds strike, and their heights.
pub const PLANE_REACH: f32 = 2.2;
pub const PLANE_HEIGHT: f32 = 1.8;
pub const TRUCK_REACH: f32 = 1.6;
pub const TRUCK_HEIGHT: f32 = 2.2;

/// An airfield's own aircraft.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlaneKind {
    /// A light bomber, which glides in and drops a stick of two bombs across her.
    Bomber,
    /// A torpedo plane, which comes in low on her beam and drops a torpedo.
    Torpedo,
}

/// The parts of an airfield her rounds strike.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AirfieldPart {
    Hangar(usize),
    Tower,
    Depot,
    Shelter(usize),
    Truck(usize),
    /// An aircraft on the ground, by its place among those taxiing.
    Plane(usize),
}

/// A building of the airfield that comes down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldWork {
    Hangar,
    Tower,
}

/// The island's own frame: along the runway, and out from the shore.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Field {
    pub centre: Vec2,
    pub along: Vec2,
    pub seaward: Vec2,
}
impl Field {
    pub fn at(&self, along: f32, out: f32) -> Vec2 {
        self.centre + self.along * along + self.seaward * out
    }
    pub fn place(&self, (along, out): (f32, f32)) -> Vec2 {
        self.at(along, out)
    }
    /// Where `p` lies in the island's frame.
    pub fn local(&self, p: Vec2) -> Vec2 {
        let d = p - self.centre;
        Vec2::new(d.dot(self.along), d.dot(self.seaward))
    }
    /// The compass heading of a direction given in the island's frame.
    pub fn heading(&self, local: Vec2) -> f32 {
        shipvivor_compass::heading(self.along * local.x + self.seaward * local.y)
    }
}

/// One shelter: the aircraft inside, and the one that flew from it and is due back.
#[derive(Clone, Copy, Debug, Default)]
pub struct Shelter {
    pub plane: Option<Parked>,
    away: Option<u64>,
}
/// An aircraft in its shelter and how far it has fuelled, from empty to full.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parked {
    pub kind: PlaneKind,
    pub fuel: f32,
}

/// A fuel truck and what it is about.
#[derive(Clone, Debug)]
pub struct Truck {
    pub body: Body,
    /// Fuel aboard, from empty to full.
    pub load: f32,
    route: Vec<Vec2>,
    job: Job,
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Job {
    /// At the depot, filling up.
    Idle,
    /// Bound for a shelter.
    Out(usize),
    /// Fuelling at a shelter's door.
    Fuelling(usize),
    /// Back to the depot.
    Back,
}
impl Truck {
    /// At a shelter's door, fuelling the plane inside.
    pub fn fuelling(&self) -> bool {
        matches!(self.job, Job::Fuelling(_))
    }
    pub fn moving(&self) -> bool {
        matches!(self.job, Job::Out(_) | Job::Back)
    }
}

/// How an aircraft on the ground is moving.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Roll {
    /// Out of its shelter and along the taxiway to the end of the runway.
    Out,
    /// Down the runway to lift off.
    TakeOff,
    /// Down from the air and back to its shelter.
    In,
}

/// An aircraft moving on the ground: the only time her guns can hit it short of the air.
#[derive(Clone, Debug)]
pub struct Taxi {
    pub body: Body,
    pub kind: PlaneKind,
    pub shelter: usize,
    pub speed: f32,
    pub roll: Roll,
    route: Vec<Vec2>,
    leg: usize,
    /// Seconds it waits in its shelter before it rolls out.
    wait: f32,
}
impl Taxi {
    pub fn position_3d(&self) -> Vec3 {
        self.body.pos.extend(FIELD_LEVEL + 0.9).xzy()
    }
    /// Still inside its shelter, waiting its turn, where no round can reach it.
    pub fn sheltered(&self) -> bool {
        self.roll == Roll::Out && self.wait > 0.
    }
    pub fn velocity(&self) -> Vec2 {
        self.body.forward() * self.speed
    }
}

/// An airfield's works and the aircraft on its ground.
#[derive(Clone, Debug)]
pub struct Airfield {
    pub field: Field,
    hangars: [u16; 2],
    tower: u16,
    /// How far the factories have raised a fallen tower, from nothing to standing.
    tower_raised: f32,
    depot: u16,
    shelters: [Shelter; SHELTERS.len()],
    trucks: Vec<Truck>,
    taxiing: Vec<Taxi>,
    /// How far the hangars are through the next aircraft, from nothing to done.
    build: f32,
    next_kind: PlaneKind,
    /// Seconds until the next wing may go up, and how long it has waited for aircraft.
    wing_due: f32,
    short: f32,
    truck_due: f32,
    stirred: bool,
}

impl Airfield {
    /// An airfield on `island`, whose heading points out from the shore, with every
    /// shelter full and fuelled and its trucks at the depot.
    pub(super) fn new(island: Vec2, out: f32, serial: &mut u64) -> Self {
        let seaward = shipvivor_compass::direction(out);
        let field = Field {
            centre: island,
            along: -seaward.perp(),
            seaward,
        };
        let shelters = std::array::from_fn(|i| Shelter {
            plane: Some(Parked {
                kind: if i % 2 == 0 {
                    PlaneKind::Bomber
                } else {
                    PlaneKind::Torpedo
                },
                fuel: 1.,
            }),
            away: None,
        });
        let trucks = (0..TRUCKS)
            .map(|k| Truck {
                body: body(serial, field.at(bay(k), DEPOT_ROAD), field.heading(Vec2::X)),
                load: 1.,
                route: vec![],
                job: Job::Idle,
            })
            .collect();
        Self {
            field,
            hangars: [HANGAR_STRENGTH; 2],
            tower: TOWER_STRENGTH,
            tower_raised: 0.,
            depot: DEPOT_STRENGTH,
            shelters,
            trucks,
            taxiing: vec![],
            build: 0.,
            next_kind: PlaneKind::Bomber,
            wing_due: FIRST_WING,
            short: 0.,
            truck_due: TRUCK_REPLACEMENT,
            stirred: false,
        }
    }

    pub fn hangar_live(&self, i: usize) -> bool {
        self.hangars[i] > 0
    }
    pub fn live_hangars(&self) -> usize {
        self.hangars.iter().filter(|&&s| s > 0).count()
    }
    pub fn tower_live(&self) -> bool {
        self.tower > 0
    }
    /// How far the factories have raised a fallen tower, from nothing to standing.
    pub fn tower_raised(&self) -> Option<f32> {
        (!self.tower_live()).then_some(self.tower_raised)
    }
    pub fn depot_live(&self) -> bool {
        self.depot > 0
    }
    pub fn shelters(&self) -> &[Shelter] {
        &self.shelters
    }
    pub fn trucks(&self) -> &[Truck] {
        &self.trucks
    }
    pub fn taxiing(&self) -> &[Taxi] {
        &self.taxiing
    }
    /// Aircraft fuelled and ready in their shelters.
    pub fn ready(&self) -> usize {
        self.shelters
            .iter()
            .filter(|s| s.plane.is_some_and(|p| p.fuel >= 1.))
            .count()
    }
    /// Seconds until the next wing may go up.
    pub fn wing_due(&self) -> f32 {
        self.wing_due
    }
    pub fn factory_position(&self, index: usize) -> Vec2 {
        self.field.place(FACTORIES[index.min(FACTORIES.len() - 1)])
    }
    pub fn hangar_position(&self, index: usize) -> Vec2 {
        self.field.place(HANGARS[index])
    }
    pub fn tower_position(&self) -> Vec2 {
        self.field.place(TOWER)
    }
    pub fn depot_position(&self) -> Vec2 {
        self.field.place(DEPOT)
    }
    pub fn shelter_position(&self, index: usize) -> Vec2 {
        self.field.at(SHELTERS[index], SHELTER_LINE)
    }
    /// The hangar's roof, standing or fallen.
    pub fn hangar_top(&self, index: usize) -> f32 {
        FIELD_LEVEL
            + if self.hangar_live(index) {
                HANGAR_SIZE.2
            } else {
                1.6
            }
    }
    pub fn tower_top(&self) -> f32 {
        FIELD_LEVEL + if self.tower_live() { TOWER_SIZE.2 } else { 2. }
    }
    pub fn depot_top(&self) -> f32 {
        FIELD_LEVEL + if self.depot_live() { DEPOT_SIZE.2 } else { 1. }
    }

    /// Where its guns stand, in the island's frame: the light mounts, then the heavy
    /// batteries.
    pub(super) fn arm(&self, port: &mut Port) {
        let base = port.body;
        let f = base.forward();
        let mut index = port.turrets.len();
        let mut station = |(along, out): (f32, f32)| {
            let at = self.field.at(along, out);
            let d = at - base.pos;
            let heading = shipvivor_compass::heading(at - self.field.centre);
            index += 1;
            (d.dot(f.perp()), d.dot(f), heading, index - 1)
        };
        for spot in LIGHT_GUNS {
            let (side, fore, heading, i) = station(spot);
            port.turrets
                .push(super::ShoreTurret::new(side, fore, 2, heading, i));
        }
        for spot in HEAVY_GUNS {
            let (side, fore, heading, i) = station(spot);
            port.turrets
                .push(super::ShoreTurret::heavy(side, fore, heading, i));
        }
    }

    /// Build aircraft into the empty shelters, raise a fallen tower, run the trucks and
    /// fuel what waits in the shelters. `factories` stand, `supplied` says whether its
    /// drydock still keeps the field supplied, and a field that is `wrecked` does
    /// nothing more.
    pub(super) fn work(
        &mut self,
        dt: f32,
        (factories, supplied, wrecked): (usize, bool, bool),
        serial: &mut u64,
    ) {
        if wrecked {
            return;
        }
        // The hangars build, each faster with every factory feeding them.
        let hangars = self.live_hangars() as f32;
        let empty = self
            .shelters
            .iter()
            .position(|s| s.plane.is_none() && s.away.is_none());
        if hangars > 0. && empty.is_some() {
            self.build += dt * hangars * (1. + 0.5 * factories as f32) / AIRFRAME_SECONDS;
        }
        if self.build >= 1.
            && let Some(i) = empty
        {
            self.build = 0.;
            self.shelters[i].plane = Some(Parked {
                kind: self.next_kind,
                fuel: 0.,
            });
            self.next_kind = match self.next_kind {
                PlaneKind::Bomber => PlaneKind::Torpedo,
                PlaneKind::Torpedo => PlaneKind::Bomber,
            };
        }
        // A fallen tower goes up again while a factory stands.
        if !self.tower_live() && factories > 0 {
            let pace = if supplied { SUPPLIED } else { 1. };
            self.tower_raised += dt * factories as f32 * pace / TOWER_REBUILD;
            if self.tower_raised >= 1. {
                self.tower = TOWER_STRENGTH;
                self.tower_raised = 0.;
            }
        }
        self.run_trucks(dt, serial);
        // What no truck stands at fuels slowly from the shelter's own line.
        for (i, shelter) in self.shelters.iter_mut().enumerate() {
            let served = self.trucks.iter().any(|t| t.job == Job::Fuelling(i));
            if let Some(plane) = &mut shelter.plane
                && !served
            {
                plane.fuel = (plane.fuel + dt / HYDRANT_FUELLING).min(1.);
            }
        }
    }

    fn run_trucks(&mut self, dt: f32, serial: &mut u64) {
        let field = self.field;
        // The depot sends out another truck for each one lost, while it stands.
        if self.depot_live() && self.trucks.len() < TRUCKS {
            self.truck_due -= dt;
            if self.truck_due <= 0. {
                self.truck_due = TRUCK_REPLACEMENT;
                let k = self.trucks.len();
                self.trucks.push(Truck {
                    body: body(serial, field.at(bay(k), DEPOT_ROAD), field.heading(Vec2::X)),
                    load: 0.,
                    route: vec![],
                    job: Job::Idle,
                });
            }
        }
        for k in 0..self.trucks.len() {
            let job = self.trucks[k].job;
            match job {
                Job::Idle => {
                    let truck = &mut self.trucks[k];
                    if self.depot > 0 {
                        truck.load = (truck.load + dt / TRUCK_LOADING).min(1.);
                    }
                    if truck.load < 1. {
                        continue;
                    }
                    // The thirstiest shelter no other truck is bound for.
                    let wanted = (0..SHELTERS.len())
                        .filter(|&i| {
                            self.shelters[i].plane.is_some_and(|p| p.fuel < 1.)
                                && !self.trucks.iter().any(
                                    |t| matches!(t.job, Job::Out(j) | Job::Fuelling(j) if j == i),
                                )
                        })
                        .min_by(|&a, &b| {
                            let fuel = |i: usize| self.shelters[i].plane.map_or(1., |p| p.fuel);
                            fuel(a).total_cmp(&fuel(b))
                        });
                    if let Some(i) = wanted {
                        let truck = &mut self.trucks[k];
                        truck.route = truck_route(&field, truck.body.pos, i);
                        truck.job = Job::Out(i);
                    }
                }
                Job::Out(i) => {
                    if drive(&mut self.trucks[k], dt) {
                        self.trucks[k].job = Job::Fuelling(i);
                    }
                }
                Job::Fuelling(i) => {
                    let truck = &mut self.trucks[k];
                    let take = (dt / TRUCK_FUELLING).min(truck.load);
                    let done = match &mut self.shelters[i].plane {
                        Some(plane) if plane.fuel < 1. => {
                            plane.fuel = (plane.fuel + take).min(1.);
                            truck.load -= take;
                            plane.fuel >= 1. || truck.load <= 0.
                        }
                        _ => true,
                    };
                    if done {
                        let mut back = truck_route(&field, field.at(bay(k), DEPOT_ROAD), i);
                        back.reverse();
                        back.push(field.at(bay(k), DEPOT_ROAD));
                        truck.route = back;
                        truck.job = Job::Back;
                    }
                }
                Job::Back => {
                    if drive(&mut self.trucks[k], dt) {
                        self.trucks[k].job = Job::Idle;
                    }
                }
            }
        }
    }

    /// Send up a wing of `size` when one is due: the fuelled aircraft roll out of their
    /// shelters one after another, nearest the runway first. Without its tower the
    /// count toward the next wing runs far slower.
    pub(super) fn send_wing(&mut self, dt: f32, size: usize, serial: &mut u64) {
        if !self.stirred {
            return;
        }
        self.wing_due -= dt * if self.tower_live() { 1. } else { TOWERLESS };
        if self.wing_due > 0. || !self.taxiing.iter().all(|t| t.roll == Roll::In) {
            return;
        }
        let ready = self.ready();
        if ready == 0 {
            return;
        }
        if ready < size {
            self.short += dt;
            if self.short < SHORT_WING_WAIT {
                return;
            }
        }
        self.short = 0.;
        self.wing_due = WING_INTERVAL;
        let mut order: Vec<usize> = (0..SHELTERS.len())
            .filter(|&i| self.shelters[i].plane.is_some_and(|p| p.fuel >= 1.))
            .collect();
        order.sort_by(|&a, &b| SHELTERS[a].total_cmp(&SHELTERS[b]));
        for (n, &i) in order.iter().take(size).enumerate() {
            let Some(plane) = self.shelters[i].plane.take() else {
                continue;
            };
            let taxi = Taxi {
                body: body(
                    serial,
                    self.shelter_position(i),
                    self.field.heading(Vec2::Y),
                ),
                kind: plane.kind,
                shelter: i,
                speed: 0.,
                roll: Roll::Out,
                route: self.taxi_out(i),
                leg: 0,
                wait: n as f32 * STAGGER,
            };
            self.shelters[i].away = Some(taxi.body.id);
            self.taxiing.push(taxi);
        }
    }

    /// Roll everything on the ground on for a step. A plane at the end of the runway
    /// with the runway clear takes off, and each that lifts off is handed back to fly.
    pub(super) fn roll(&mut self, dt: f32) -> Vec<Taxi> {
        let field = self.field;
        // One aircraft on the runway at a time: one rolling to take off, or one just down
        // and not yet off it.
        let mut runway_busy = self
            .taxiing
            .iter()
            .any(|t| t.roll == Roll::TakeOff || (t.roll == Roll::In && t.leg == 0));
        let mut airborne = vec![];
        let mut home = vec![];
        for (n, taxi) in self.taxiing.iter_mut().enumerate() {
            match taxi.roll {
                Roll::Out if taxi.wait > 0. => taxi.wait -= dt,
                Roll::Out => {
                    taxi.speed = TAXI_SPEED;
                    if follow(taxi, dt) && !runway_busy {
                        runway_busy = true;
                        taxi.roll = Roll::TakeOff;
                        taxi.body.heading = field.heading(Vec2::X);
                    }
                }
                Roll::TakeOff => {
                    taxi.speed += ROLL_ACCELERATION * dt;
                    taxi.body.pos += taxi.body.forward() * taxi.speed * dt;
                    if taxi.speed >= LIFT_OFF {
                        airborne.push(n);
                    }
                }
                Roll::In => {
                    taxi.speed = (taxi.speed - ROLL_ACCELERATION * dt).max(TAXI_SPEED);
                    if follow(taxi, dt) {
                        home.push(n);
                    }
                }
            }
        }
        for &n in &home {
            let taxi = &self.taxiing[n];
            let shelter = &mut self.shelters[taxi.shelter];
            shelter.away = None;
            shelter.plane = Some(Parked {
                kind: taxi.kind,
                fuel: 0.,
            });
        }
        let mut lifted = vec![];
        let mut n = 0;
        self.taxiing.retain(|taxi| {
            let keep = !home.contains(&n) && !airborne.contains(&n);
            if airborne.contains(&n) {
                lifted.push(taxi.clone());
            }
            n += 1;
            keep
        });
        lifted
    }

    /// Bring an aircraft that has touched down back to its shelter.
    pub(super) fn land(&mut self, plane: &Warplane, serial: &mut u64) {
        let route = self.taxi_in(plane.shelter);
        self.taxiing.push(Taxi {
            body: body(serial, plane.body.pos, self.field.heading(Vec2::NEG_X)),
            kind: plane.kind,
            shelter: plane.shelter,
            speed: TOUCH_DOWN,
            roll: Roll::In,
            route,
            leg: 0,
            wait: 0.,
        });
    }

    /// An aircraft from shelter `shelter` was lost: the shelter is free for a new one.
    pub(super) fn lost(&mut self, shelter: usize) {
        self.shelters[shelter].away = None;
    }

    /// The field stirs with its base: its first wing goes up a little after.
    pub(super) fn stir(&mut self) {
        if !self.stirred {
            self.stirred = true;
            self.wing_due = FIRST_WING;
        }
    }

    /// From shelter `i`'s door, out to the taxiway and along it to the end of the runway.
    fn taxi_out(&self, i: usize) -> Vec<Vec2> {
        let f = &self.field;
        let along = SHELTERS[i];
        vec![
            f.at(along, SHELTER_LINE + SHELTER_SIZE.1),
            f.at(along, TAXIWAY_LINE),
            f.at(-TAXIWAY_END, TAXIWAY_LINE),
            f.at(RUNWAY.0, RUNWAY_LINE - 6.),
            f.at(RUNWAY.0, RUNWAY_LINE),
        ]
    }
    /// From touchdown down the runway, off it onto the taxiway, and into shelter `i`.
    fn taxi_in(&self, i: usize) -> Vec<Vec2> {
        let f = &self.field;
        let along = SHELTERS[i];
        vec![
            f.at(-10., RUNWAY_LINE),
            f.at(-16., TAXIWAY_LINE),
            f.at(along, TAXIWAY_LINE),
            f.at(along, SHELTER_LINE),
        ]
    }

    /// Where an aircraft coming home lines up to land: off the far end of the runway,
    /// and the way it lands.
    pub fn approach(&self) -> (Vec2, Vec2) {
        (
            self.field.at(RUNWAY.1 + 110., RUNWAY_LINE),
            -self.field.along,
        )
    }
    pub fn threshold(&self) -> Vec2 {
        self.field.at(RUNWAY.1, RUNWAY_LINE)
    }

    /// The airfield's buildings, trucks and aircraft on the ground that the line from
    /// `a` to `b` crosses, with the heights each stands between.
    pub(super) fn contacts(
        &self,
        a: Vec2,
        b: Vec2,
        mut visit: impl FnMut((f32, f32), f32, f32, AirfieldPart),
    ) {
        let f = &self.field;
        let block = |centre: Vec2, (half_along, half_out, _): (f32, f32, f32)| {
            box_span(f, centre, Vec2::new(half_along, half_out), a, b)
        };
        for i in 0..HANGARS.len() {
            if let Some(span) = block(self.hangar_position(i), HANGAR_SIZE) {
                visit(
                    span,
                    FIELD_LEVEL,
                    self.hangar_top(i),
                    AirfieldPart::Hangar(i),
                );
            }
        }
        if let Some(span) = block(self.tower_position(), TOWER_SIZE) {
            visit(span, FIELD_LEVEL, self.tower_top(), AirfieldPart::Tower);
        }
        if let Some(span) = block(self.depot_position(), DEPOT_SIZE) {
            visit(span, FIELD_LEVEL, self.depot_top(), AirfieldPart::Depot);
        }
        for i in 0..SHELTERS.len() {
            if let Some(span) = block(self.shelter_position(i), SHELTER_SIZE) {
                let top = FIELD_LEVEL + SHELTER_SIZE.2;
                visit(span, FIELD_LEVEL, top, AirfieldPart::Shelter(i));
            }
        }
        for (i, truck) in self.trucks.iter().enumerate() {
            if let Some(span) = super::footprint(a, b, truck.body.pos, TRUCK_REACH) {
                let top = FIELD_LEVEL + TRUCK_HEIGHT;
                visit(span, FIELD_LEVEL, top, AirfieldPart::Truck(i));
            }
        }
        for (i, taxi) in self.taxiing.iter().enumerate() {
            if taxi.sheltered() {
                continue;
            }
            if let Some(span) = super::footprint(a, b, taxi.body.pos, PLANE_REACH) {
                let top = FIELD_LEVEL + PLANE_HEIGHT;
                visit(span, FIELD_LEVEL, top, AirfieldPart::Plane(i));
            }
        }
    }

    /// Targets for her wing guns: where each standing part is, and the height to aim at.
    pub(super) fn targets(&self) -> Vec<(AirfieldPart, Vec2, f32)> {
        let mut out = vec![];
        for (i, taxi) in self.taxiing.iter().enumerate() {
            if !taxi.sheltered() {
                out.push((AirfieldPart::Plane(i), taxi.body.pos, FIELD_LEVEL + 0.9));
            }
        }
        for (i, truck) in self.trucks.iter().enumerate() {
            out.push((AirfieldPart::Truck(i), truck.body.pos, FIELD_LEVEL + 1.1));
        }
        for i in 0..HANGARS.len() {
            if self.hangar_live(i) {
                let at = self.hangar_position(i);
                out.push((
                    AirfieldPart::Hangar(i),
                    at,
                    FIELD_LEVEL + HANGAR_SIZE.2 * 0.5,
                ));
            }
        }
        if self.tower_live() {
            out.push((AirfieldPart::Tower, self.tower_position(), FIELD_LEVEL + 6.));
        }
        if self.depot_live() {
            out.push((AirfieldPart::Depot, self.depot_position(), FIELD_LEVEL + 2.));
        }
        out
    }
}

/// Where truck `k` waits at the depot, along the island.
fn bay(k: usize) -> f32 {
    DEPOT.0 - 5. + 5. * k as f32
}

/// From `from` along the depot road, down the lane beside shelter `i` and along to its
/// door.
fn truck_route(field: &Field, from: Vec2, i: usize) -> Vec<Vec2> {
    // The lane on the gap beside the shelter, between it and its neighbour.
    let along = SHELTERS[i];
    let lane = if along < 20. { along + 6. } else { along - 6. };
    let start = field.local(from);
    vec![
        field.at(start.x, DEPOT_ROAD),
        field.at(lane, DEPOT_ROAD),
        field.at(lane, DOOR_ROAD),
        field.at(along + 2.5 * (lane - along).signum(), DOOR_ROAD),
    ]
}

/// Drive a truck along its route. True once it has arrived.
fn drive(truck: &mut Truck, dt: f32) -> bool {
    let mut step = TRUCK_SPEED * dt;
    while let Some(&next) = truck.route.first() {
        let to = next - truck.body.pos;
        let d = to.length();
        if d > 1e-3 {
            truck.body.heading = shipvivor_compass::heading(to);
        }
        if d > step {
            truck.body.pos += to / d * step;
            return false;
        }
        truck.body.pos = next;
        step -= d;
        truck.route.remove(0);
    }
    true
}

/// Taxi along the route. True once at its end.
fn follow(taxi: &mut Taxi, dt: f32) -> bool {
    let mut step = taxi.speed * dt;
    while let Some(&next) = taxi.route.get(taxi.leg) {
        let to = next - taxi.body.pos;
        let d = to.length();
        if d > 1e-3 {
            taxi.body.heading = shipvivor_compass::heading(to);
        }
        if d > step {
            taxi.body.pos += to / d * step;
            return false;
        }
        taxi.body.pos = next;
        step -= d;
        taxi.leg += 1;
    }
    true
}

/// Where the line from `a` to `b` crosses a box of half size `half` at `centre`, set
/// square to the island, as fractions of the way along it.
fn box_span(field: &Field, centre: Vec2, half: Vec2, a: Vec2, b: Vec2) -> Option<(f32, f32)> {
    let (la, lb) = (
        field.local(a) - field.local(centre),
        field.local(b) - field.local(centre),
    );
    let d = lb - la;
    let (mut low, mut high) = (0_f32, 1_f32);
    for axis in 0..2 {
        let (p, q, h) = (la[axis], d[axis], half[axis]);
        if q.abs() < 1e-6 {
            if p.abs() > h {
                return None;
            }
            continue;
        }
        let (t0, t1) = ((-h - p) / q, (h - p) / q);
        low = low.max(t0.min(t1));
        high = high.min(t0.max(t1));
        if low > high {
            return None;
        }
    }
    Some((low, high))
}

impl Battle {
    /// Every airfield on the chart: the flat island each airfield base's harbour stands
    /// beside, with its works, its guns and its aircraft.
    pub(super) fn site_airfields(&mut self, works: &[u8]) {
        for (i, &kind) in works.iter().enumerate() {
            if kind != AIRFIELD_WORKS {
                continue;
            }
            let harbour = self.ports[i].body.pos;
            let Some(island) = self
                .landmarks
                .iter()
                .filter(|l| l.kind == super::LandmarkKind::Airfield)
                .min_by(|a, b| {
                    a.body
                        .pos
                        .distance_squared(harbour)
                        .total_cmp(&b.body.pos.distance_squared(harbour))
                })
            else {
                continue;
            };
            let field = Airfield::new(island.body.pos, island.body.heading, &mut self.serial);
            field.arm(&mut self.ports[i]);
            self.ports[i].airfield = Some(Box::new(field));
        }
    }

    /// Every airfield's aircraft, shelters, trucks and tower for a step, and the wings it
    /// sends up, until her guns or its loss ground it.
    pub(super) fn run_airfields(&mut self, dt: f32) {
        let size = if self.squadron_round >= 5 { 4 } else { 3 };
        // Room in the air for another wing, counting those already rolling out.
        let rolling: usize = (self.ports.iter())
            .filter_map(|port| port.airfield())
            .map(|field| field.taxiing.iter().filter(|t| t.roll != Roll::In).count())
            .sum();
        let mut room = warplane::MOST_ALOFT.saturating_sub(self.warplanes.len() + rolling);
        let mut lifted = vec![];
        for (i, port) in self.ports.iter_mut().enumerate() {
            let wrecked = port.wrecked();
            let roused = port.roused();
            let factories = port.live_factories();
            let supplied = port.live_docks() > 0;
            let Some(field) = port.airfield.as_deref_mut() else {
                continue;
            };
            field.work(dt, (factories, supplied, wrecked), &mut self.serial);
            if roused && !wrecked {
                field.stir();
                if room >= size.min(2) {
                    let before = field.taxiing.len();
                    field.send_wing(dt, size.min(room), &mut self.serial);
                    room = room.saturating_sub(field.taxiing.len() - before);
                }
            }
            for taxi in field.roll(dt) {
                lifted.push((i, taxi));
            }
        }
        for (base, taxi) in lifted {
            let plane = Warplane::lift_off(base, &taxi);
            self.warplanes.push(plane);
        }
    }

    /// A round of `damage` strikes part `part` of base `port`'s airfield at `at`. What
    /// it brings down, and the effect it leaves.
    pub(super) fn strike_airfield(
        &mut self,
        port: usize,
        part: AirfieldPart,
        damage: u16,
        at: Vec3,
        events: &mut Events,
    ) -> (EffectKind, Vec3) {
        let level = |p: Vec2| p.extend(FIELD_LEVEL).xzy();
        let Some(field) = self.ports[port].airfield.as_deref_mut() else {
            return (EffectKind::Burst, at);
        };
        let fell = |strength: &mut u16| {
            let was = *strength > 0;
            *strength = strength.saturating_sub(damage);
            was && *strength == 0
        };
        // What the round brought down, if anything: a building, or fuel going up.
        enum Down {
            Nothing,
            Work(FieldWork, Vec2),
            Fuel(Vec2, f32, bool),
        }
        let down = match part {
            AirfieldPart::Hangar(i) if fell(&mut field.hangars[i]) => {
                Down::Work(FieldWork::Hangar, field.hangar_position(i))
            }
            AirfieldPart::Tower if fell(&mut field.tower) => {
                field.tower_raised = 0.;
                Down::Work(FieldWork::Tower, field.tower_position())
            }
            AirfieldPart::Depot if fell(&mut field.depot) => {
                Down::Fuel(field.depot_position(), DEPOT_BLAST, true)
            }
            AirfieldPart::Truck(i) => field.trucks.get(i).map_or(Down::Nothing, |t| {
                Down::Fuel(t.body.pos, TRUCK_BLAST, false)
            }),
            AirfieldPart::Plane(i) => field.taxiing.get(i).map_or(Down::Nothing, |t| {
                Down::Fuel(t.body.pos, TRUCK_BLAST, false)
            }),
            // Concrete turns every round, and what stands takes the hit.
            _ => Down::Nothing,
        };
        match down {
            Down::Nothing => (EffectKind::Burst, at),
            Down::Work(work, centre) => {
                if work == FieldWork::Hangar {
                    events.recovered += self.rewards.at_once(super::FACTORY_STORES);
                    self.ports[port].settle();
                }
                (EffectKind::Felled(work), level(centre))
            }
            Down::Fuel(centre, reach, depot) => {
                self.ground_blast(port, centre, reach, events);
                (EffectKind::FuelBlast { depot }, level(centre))
            }
        }
    }

    /// Splinters from a round of `calibre` inches bursting at `at` wreck any truck or
    /// aircraft on the ground close by, on every airfield.
    pub(super) fn splinter_airfields(&mut self, at: Vec3, calibre: f32, events: &mut Events) {
        if at.y > FIELD_LEVEL + 6. {
            return;
        }
        let reach = SPLINTERS + 0.6 * calibre;
        for port in 0..self.ports.len() {
            let Some(field) = self.ports[port].airfield.as_deref() else {
                continue;
            };
            let near = |p: Vec2| p.distance(at.xz()) <= reach;
            let hit = field
                .trucks
                .iter()
                .map(|t| t.body.pos)
                .chain(
                    field
                        .taxiing
                        .iter()
                        .filter(|t| !t.sheltered())
                        .map(|t| t.body.pos),
                )
                .find(|&p| near(p));
            if let Some(centre) = hit {
                self.ground_blast(port, centre, TRUCK_BLAST, events);
                let height = FIELD_LEVEL;
                self.effects.push(Effect {
                    body: body(&mut self.serial, centre, 0.),
                    height,
                    kind: EffectKind::FuelBlast { depot: false },
                    age: 0.,
                });
            }
        }
    }

    /// Fuel goes up at `centre` on base `port`'s airfield: every truck and aircraft on
    /// the ground within `reach` goes with it, each a blast of its own.
    pub(super) fn ground_blast(
        &mut self,
        port: usize,
        centre: Vec2,
        reach: f32,
        events: &mut Events,
    ) {
        let mut pending = vec![(centre, reach)];
        while let Some((at, reach)) = pending.pop() {
            let Some(field) = self.ports[port].airfield.as_deref_mut() else {
                return;
            };
            let near = |p: Vec2| p.distance(at) <= reach;
            let mut lost = vec![];
            field.trucks.retain(|t| {
                if near(t.body.pos) {
                    lost.push(t.body.pos);
                    false
                } else {
                    true
                }
            });
            events.vehicles += lost.len() as u16;
            let mut planes = vec![];
            field.taxiing.retain(|t| {
                if !t.sheltered() && near(t.body.pos) {
                    planes.push((t.body.pos, t.shelter));
                    false
                } else {
                    true
                }
            });
            for &(_, shelter) in &planes {
                field.lost(shelter);
            }
            events.planes_downed += planes.len() as u16;
            for p in lost.into_iter().chain(planes.into_iter().map(|(p, _)| p)) {
                if p.distance(centre) > 0.5 {
                    self.effects.push(Effect {
                        body: body(&mut self.serial, p, 0.),
                        height: FIELD_LEVEL,
                        kind: EffectKind::FuelBlast { depot: false },
                        age: 0.,
                    });
                }
                pending.push((p, TRUCK_BLAST));
            }
        }
    }

    /// The airfields' aircraft in the air.
    pub fn warplanes(&self) -> &[Warplane] {
        &self.warplanes
    }
}
