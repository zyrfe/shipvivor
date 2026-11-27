//! Breakwater's light anti-aircraft guns. Each fires a time-fused round that bursts where
//! the crew leads an enemy aircraft to be, and an aircraft inside the burst takes its
//! damage. Bursts scatter about the aim, so a steady pass is hit often but not always.
use super::{
    AntiAir, Battery, Battle, Effect, EffectKind, Events, Orders, body,
    projectile::{AA_SPEED, FlightPath},
};
use glam::{Vec2, Vec3, Vec3Swizzles};
use shipvivor_compass::direction;
use shipvivor_shipyard::AaStation;

/// Metres from a burst inside which an aircraft is hit.
pub const BURST_RADIUS: f32 = 7.;
/// Metres the bursts of a well-laid gun scatter about its aim.
const SCATTER: f32 = 10.;
/// Hits a reconnaissance floatplane takes before it goes down.
pub(super) const SPOTTER_STRENGTH: u16 = 5;
/// Hits a strafer or bomber takes before it goes down.
pub(super) const ATTACK_STRENGTH: u16 = 8;
/// Hits a harrier takes before it goes down: enough that it takes a moment, and most
/// get a strafing pass in before they fall.
pub(super) const HARRIER_STRENGTH: u16 = 3;
/// Seconds an air station takes to ready a fresh aircraft for one that was shot down.
pub(super) const REPLACEMENT: f32 = 45.;
/// Radians per second an AA mount trains and elevates.
const TRAINING: f32 = 100_f32.to_radians();
/// Metres above the waterline of an AA mount's trunnions.
const MOUNT_HEIGHT: f32 = 3.5;

/// What kind of aircraft went down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Airframe {
    /// A harbour's reconnaissance floatplane.
    Spotter,
    /// An air station's strafer or bomber.
    Attack,
    /// A light harrier from a base's mooring.
    Harrier,
    /// An airfield's light bomber.
    Bomber,
    /// An airfield's torpedo plane.
    TorpedoPlane,
}

/// One light anti-aircraft mount, worked by its crew.
pub struct AaMount {
    reload: f32,
    abeam: f32,
    fore: f32,
    /// Bearing from the bow, and elevation above the horizon.
    train: f32,
    elevation: f32,
    /// Rounds fired, which sets where the next one scatters.
    rounds: u32,
}
impl AaMount {
    /// A mount standing at `station`, trained out on its beam: see
    /// `Design::aa_stations` for where they stand.
    pub(super) fn new(station: AaStation) -> Self {
        Self {
            reload: 0.,
            abeam: station.abeam,
            fore: station.fore,
            train: station.abeam.signum() * std::f32::consts::FRAC_PI_2,
            elevation: 0.35,
            rounds: 0,
        }
    }
    /// Stand at `station`, where the yard has moved it, keeping its load and laying.
    pub(super) fn place(&mut self, station: AaStation) {
        self.abeam = station.abeam;
        self.fore = station.fore;
    }
    pub fn deck_x(&self) -> f32 {
        self.abeam
    }
    /// Metres forward of amidships.
    pub fn deck_fore(&self) -> f32 {
        self.fore
    }
    /// Where the guns point, relative to the bow.
    pub fn deck_bearing(&self) -> f32 {
        self.train
    }
    pub fn elevation(&self) -> f32 {
        self.elevation
    }
    fn pivot(&self, player: Vec2, heading: f32) -> Vec3 {
        (player + direction(heading).perp() * self.abeam + direction(heading) * self.fore)
            .extend(MOUNT_HEIGHT)
            .xzy()
    }
}

/// An enemy aircraft the guns can engage: where it is and where it is heading.
struct Quarry {
    at: Vec3,
    way: Vec2,
}

impl Battle {
    pub fn anti_air(&self) -> AntiAir {
        self.anti_air
    }
    pub fn aa_mounts(&self) -> &[AaMount] {
        &self.aa_mounts
    }
    /// Mount, arm and drill the AA battery as fitted. Mounts already aboard keep their
    /// state. Mounts past the room her deck has wait ashore until a longer hull makes room.
    pub fn fit_anti_air(&mut self, fit: AntiAir) {
        let wanted = fit.mounts.min(self.aa_stations.len());
        self.aa_mounts.truncate(wanted);
        for (mount, &station) in self.aa_mounts.iter_mut().zip(&self.aa_stations) {
            mount.place(station);
        }
        let aboard = self.aa_mounts.len();
        self.aa_mounts.extend(
            self.aa_stations[aboard..wanted]
                .iter()
                .copied()
                .map(AaMount::new),
        );
        self.anti_air = fit;
    }

    fn aircraft_aloft(&self) -> Vec<Quarry> {
        let raid = self
            .air_station
            .as_ref()
            .and_then(|station| station.aircraft.as_ref())
            .map(|plane| Quarry {
                at: plane.position_3d(),
                way: plane.way,
            });
        let spotters = self
            .spotters
            .iter()
            .filter(|plane| plane.airborne())
            .map(|plane| Quarry {
                at: plane.position_3d(),
                way: plane.way,
            });
        let harriers = self.harriers.iter().map(|plane| Quarry {
            at: plane.position_3d(),
            way: plane.way,
        });
        let warplanes = self.warplanes.iter().map(|plane| Quarry {
            at: plane.position_3d(),
            way: plane.way,
        });
        // Aircraft rolling out or taking off are in reach of the AA as well.
        let rolling = self
            .ports
            .iter()
            .filter_map(|port| port.airfield())
            .flat_map(|field| field.taxiing())
            .filter(|taxi| !taxi.sheltered())
            .map(|taxi| Quarry {
                at: taxi.position_3d(),
                way: taxi.velocity(),
            });
        raid.into_iter()
            .chain(spotters)
            .chain(harriers)
            .chain(warplanes)
            .chain(rolling)
            .collect()
    }

    /// Each mount trains on the nearest enemy aircraft in reach and fires a round fused
    /// to burst where its lead puts the aircraft.
    pub(super) fn fire_anti_air(&mut self, dt: f32, orders: &Orders) {
        if self.aa_mounts.is_empty() {
            return;
        }
        let quarry = self.aircraft_aloft();
        let fit = self.anti_air;
        let power = self.gun_train_rate;
        for (i, mount) in self.aa_mounts.iter_mut().enumerate() {
            mount.reload = (mount.reload - dt).max(0.);
            let pivot = mount.pivot(orders.player, orders.heading);
            let Some(target) = quarry
                .iter()
                .filter(|q| q.at.xz().distance_squared(pivot.xz()) <= fit.range * fit.range)
                .min_by(|a, b| {
                    a.at.distance_squared(pivot)
                        .total_cmp(&b.at.distance_squared(pivot))
                })
            else {
                continue;
            };
            if power <= 0. {
                continue;
            }
            // Lead the aircraft along its present track for the round's time of flight.
            let mut aim = target.at;
            for _ in 0..3 {
                let flight = aim.xz().distance(pivot.xz()) / AA_SPEED;
                aim = target.at + (target.way * flight).extend(0.).xzy();
            }
            let offset = aim - pivot;
            let bearing = shipvivor_compass::heading(offset.xz()) - orders.heading;
            let elevation = offset.y.atan2(offset.xz().length());
            let step = TRAINING * dt * power;
            let on_bearing = super::mounts::train_full(&mut mount.train, bearing, step);
            mount.elevation = mount.elevation + (elevation - mount.elevation).clamp(-step, step);
            let elevated = (elevation - mount.elevation).abs() <= 0.0001;
            if !(on_bearing && elevated) || mount.reload > 0. {
                continue;
            }
            mount.rounds += 1;
            let burst = aim + scatter(mount.rounds, i);
            if let Some(path) = FlightPath::to(pivot, burst, Battery::AntiAir) {
                path.launch(
                    &mut self.shots,
                    &mut self.effects,
                    &mut self.serial,
                    Battery::AntiAir,
                    fit.damage,
                );
                mount.reload = fit.reload;
            }
        }
    }

    /// A fused round bursts. Every enemy aircraft inside it takes the damage, and one
    /// with nothing left goes down.
    pub(super) fn burst_flak(&mut self, at: Vec3, damage: u16, events: &mut Events) {
        self.effects.push(Effect {
            body: body(&mut self.serial, at.xz(), 0.),
            height: at.y,
            kind: EffectKind::Flak,
            age: 0.,
        });
        let caught = |plane: Vec3| plane.distance_squared(at) <= BURST_RADIUS * BURST_RADIUS;
        if let Some(station) = &mut self.air_station
            && let Some(plane) = &mut station.aircraft
            && caught(plane.position_3d())
        {
            plane.health = plane.health.saturating_sub(damage);
            if plane.health == 0 {
                let (pos, heading, height) = (plane.body.pos, plane.body.heading, plane.altitude);
                station.aircraft = None;
                station.rearm = REPLACEMENT;
                events.planes_downed += 1;
                self.effects.push(Effect {
                    body: body(&mut self.serial, pos, heading),
                    height,
                    kind: EffectKind::Downed(Airframe::Attack),
                    age: 0.,
                });
            }
        }
        for plane in self.spotters.iter_mut().filter(|p| p.airborne()) {
            if !caught(plane.position_3d()) {
                continue;
            }
            plane.health = plane.health.saturating_sub(damage);
            if plane.health == 0 {
                plane.shoot_down();
                events.planes_downed += 1;
                self.effects.push(Effect {
                    body: body(&mut self.serial, plane.position, plane.heading),
                    height: plane.altitude,
                    kind: EffectKind::Downed(Airframe::Spotter),
                    age: 0.,
                });
            }
        }
        let mut downed = Vec::new();
        self.harriers.retain_mut(|plane| {
            if caught(plane.position_3d()) {
                plane.health = plane.health.saturating_sub(damage);
            }
            if plane.health > 0 {
                return true;
            }
            downed.push((plane.body.pos, plane.body.heading, plane.altitude));
            false
        });
        for (pos, heading, height) in downed {
            events.planes_downed += 1;
            self.effects.push(Effect {
                body: body(&mut self.serial, pos, heading),
                height,
                kind: EffectKind::Downed(Airframe::Harrier),
                age: 0.,
            });
        }
        for n in (0..self.warplanes.len()).rev() {
            let plane = &mut self.warplanes[n];
            if caught(plane.position_3d()) {
                plane.health = plane.health.saturating_sub(damage);
                if plane.health == 0 {
                    self.down_warplane(n, events);
                }
            }
        }
        // A burst over an airfield wrecks an aircraft rolling there, and its fuel goes up.
        let rolling: Vec<(usize, Vec2)> = (self.ports.iter().enumerate())
            .filter_map(|(i, port)| port.airfield().map(|field| (i, field)))
            .flat_map(|(i, field)| {
                field
                    .taxiing()
                    .iter()
                    .filter(|taxi| !taxi.sheltered() && caught(taxi.position_3d()))
                    .map(move |taxi| (i, taxi.body.pos))
            })
            .collect();
        for (port, at) in rolling {
            self.ground_blast(port, at, 0.5, events);
            self.effects.push(Effect {
                body: body(&mut self.serial, at, 0.),
                height: super::FIELD_LEVEL,
                kind: EffectKind::FuelBlast { depot: false },
                age: 0.,
            });
        }
    }
}

/// Where a round bursts about its aim: a fixed spiral, different on every gun, that
/// fills a disc across the line of fire and a little above and below.
fn scatter(round: u32, gun: usize) -> Vec3 {
    let n = round as f32 + 7.3 * gun as f32;
    let spin = n * 2.399_963;
    let radius = SCATTER * (n * 0.618_034).fract().sqrt();
    Vec3::new(
        radius * spin.cos(),
        0.35 * radius * (1.7 * spin).sin(),
        radius * spin.sin(),
    )
}
