//! Authored arcade hulls and deck stations. The sight, hit tests, gunfire and
//! renderer all use the same station positions in the ship's rotating frame.
use super::{Battery, Body, Effect, Hull, ISLAND_RADIUS, KNOCK_DRAG, Port, Shot, bearing, entry};
use super::{mounts::train_full, projectile::FlightPath};
use glam::{Vec2, Vec3, Vec3Swizzles};
use shipvivor_artillery::{ArmorGrade, ArmorPanel};
use shipvivor_shipyard::{Berth, Component, Design, State};
use std::f32::consts::PI;
use std::sync::{Arc, OnceLock};

const REGENT_WARNING: f32 = 3.2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EnemyClass {
    Freighter,
    Patrol,
    Destroyer,
    Cruiser,
    Battleship,
    Parliament,
    Regent,
}
use EnemyClass::*;

pub struct Profile {
    pub hull: Hull,
    pub deck: f32,
    pub gun_scale: f32,
    pub barrels: u8,
    speed: f32,
    stand_off: f32,
    range: f32,
    traverse: f32,
    reload: f32,
    damage: u16,
    pub stores: u8,
}

impl EnemyClass {
    pub fn is_flagship(self) -> bool {
        matches!(self, Parliament | Regent)
    }

    pub fn lookout_range(self) -> f32 {
        match self {
            Freighter => 0.,
            Patrol => 520.,
            Destroyer => 650.,
            Cruiser => 770.,
            Battleship | Parliament | Regent => 900.,
        }
    }

    pub fn profile(self) -> &'static Profile {
        match self {
            Freighter => &Profile {
                hull: Hull::CRUISER,
                deck: 2.2,
                gun_scale: 1.,
                barrels: 0,
                speed: 8.,
                stand_off: 0.,
                range: 0.,
                traverse: 0.,
                reload: 0.,
                damage: 0,
                stores: 4,
            },
            Patrol => &Profile {
                hull: Hull::RAIDER,
                deck: 1.65,
                gun_scale: 0.8,
                barrels: 1,
                speed: 5.4,
                stand_off: 38.,
                range: 210.,
                traverse: 52.,
                reload: 4.5,
                damage: 7,
                stores: 1,
            },
            Destroyer => &Profile {
                hull: Hull {
                    radius: 2.2,
                    bow: 11.5,
                    stern: 10.5,
                    mass: 4.,
                    shock_per_point: 0.6,
                },
                deck: 1.8,
                gun_scale: 1.,
                barrels: 1,
                speed: 7.6,
                stand_off: 80.,
                range: 285.,
                traverse: 40.,
                reload: 4.8,
                damage: 4,
                stores: 2,
            },
            Cruiser => &Profile {
                hull: Hull::CRUISER,
                deck: 2.,
                gun_scale: 1.28,
                barrels: 2,
                speed: 6.6,
                stand_off: 118.,
                range: 350.,
                traverse: 31.,
                reload: 6.,
                damage: 4,
                stores: 3,
            },
            Battleship => &Profile {
                hull: Hull {
                    radius: 5.8,
                    bow: 27.,
                    stern: 24.,
                    mass: 26.,
                    shock_per_point: 2.8,
                },
                deck: 3.1,
                gun_scale: 2.05,
                barrels: 3,
                speed: 3.3,
                stand_off: 185.,
                range: 440.,
                traverse: 19.,
                reload: 10.5,
                damage: 7,
                stores: 7,
            },
            Regent => &Profile {
                hull: Hull {
                    radius: 18.5,
                    bow: 89.,
                    stern: 79.,
                    mass: 210.,
                    shock_per_point: 3.8,
                },
                deck: 5.8,
                gun_scale: 3.55,
                barrels: 3,
                speed: 3.4,
                stand_off: 290.,
                range: 505.,
                traverse: 19.,
                reload: 13.,
                damage: 4,
                stores: 18,
            },
            Parliament => &Profile {
                hull: Hull {
                    radius: 10.4,
                    bow: 43.,
                    stern: 38.,
                    mass: 85.,
                    shock_per_point: 1.8,
                },
                deck: 4.1,
                gun_scale: 2.65,
                barrels: 3,
                speed: 2.7,
                stand_off: 252.,
                range: 475.,
                traverse: 23.,
                reload: 11.5,
                damage: 3,
                stores: 12,
            },
        }
    }

    /// The module layout that hits on this class land on.
    pub fn design(self) -> Arc<Design> {
        // In the order of the classes.
        static YARD: OnceLock<[Arc<Design>; 7]> = OnceLock::new();
        YARD.get_or_init(|| {
            [
                Design::freighter(),
                Design::patrol_boat(),
                Design::destroyer(),
                Design::cruiser(),
                Design::battleship(),
                Design::flagship(),
                Design::iron_regent(),
            ]
            .map(Arc::new)
        })[self as usize]
            .clone()
    }

    pub fn main_count(self) -> usize {
        self.stations().len()
    }

    // (starboard, forward, height of raised base above the deck)
    fn stations(self) -> &'static [(f32, f32, f32)] {
        match self {
            Freighter => &[],
            Patrol => &[(0., 4.2, 0.)],
            Destroyer => &[(0., 5., 0.), (0., -4.5, 0.)],
            Cruiser => &[(0., 8.7, 0.), (0., -8.5, 0.)],
            Battleship => &[(0., 17.5, 0.), (0., 9., 2.8), (0., -16., 0.)],
            Regent => &[
                (0., 64., 1.2),
                (12., -35., 1.),
                (-11., 38., 1.),
                (12.5, 23., 2.),
                (-12.5, 3., 2.),
                (0., -68., 1.2),
                (-12., -35., 1.),
                (11., 38., 1.),
                (-12.5, 23., 2.),
                (12.5, 3., 2.),
                (-10., -50., 0.),
                (10., -50., 0.),
            ],
            Parliament => &[
                (0., 31., 0.),
                (5., -18., 0.),
                (-6.6, 7., 1.6),
                (5., 18., 0.),
                (-6.6, -7., 1.6),
                (0., -29., 1.),
                (-5., 18., 0.),
                (6.6, -7., 1.6),
                (-5., -18., 0.),
                (6.6, 7., 1.6),
            ],
        }
    }
}

fn home(side: f32, fore: f32) -> f32 {
    if side != 0. {
        side.signum() * PI * 0.5
    } else if fore < 0. {
        PI
    } else {
        0.
    }
}

pub struct EnemyGun {
    offset: (f32, f32, f32),
    train: f32,
    berth: Berth,
    /// Knocked out, or standing on a wrecked module.
    out: bool,
    reload: f32,
    charging: bool,
    /// The Regent commits to this patch of water when its warning light comes on.
    committed: Option<Vec2>,
    pub(super) plate: ArmorPanel,
}
impl EnemyGun {
    pub fn live(&self) -> bool {
        !self.out
    }
    pub fn charging(&self) -> bool {
        self.charging && self.live()
    }
    pub fn bearing(&self, body: &Body) -> f32 {
        body.heading + self.train
    }
    /// Centre of the gun ring at deck level (or atop its stepped base).
    pub fn base(&self, body: &Body, profile: &Profile) -> Vec3 {
        let (side, fore, rise) = self.offset;
        let f = body.forward();
        (body.pos + f.perp() * side + f * fore)
            .extend(profile.deck + rise)
            .xzy()
    }
    pub fn radius(&self, profile: &Profile) -> f32 {
        1.17 * profile.gun_scale
    }
    pub fn top(&self, profile: &Profile) -> f32 {
        profile.deck + self.offset.2 + 1.4 * profile.gun_scale
    }
    pub fn riser(&self) -> f32 {
        self.offset.2
    }
}

pub struct SecondaryMount {
    pub(super) strength: u16,
    offset: Vec2,
    train: f32,
    reload: f32,
    director: usize,
}
impl SecondaryMount {
    pub fn live(&self) -> bool {
        self.strength > 0
    }
    pub fn station(&self, body: &Body, profile: &Profile) -> Vec3 {
        let forward = body.forward();
        (body.pos + forward.perp() * self.offset.x + forward * self.offset.y)
            .extend(profile.deck)
            .xzy()
    }
    pub fn bearing(&self, body: &Body) -> f32 {
        body.heading + self.train
    }
}

pub struct Raider {
    pub secondaries: Vec<SecondaryMount>,
    pub(super) cargo_run: Option<(usize, Vec2)>,
    pub body: Body,
    pub class: EnemyClass,
    pub has_contact: bool,
    pub(super) last_sighting: Option<Vec2>,
    pub(super) guns: Vec<EnemyGun>,
    pub(super) ship: shipvivor_shipyard::Ship,
    director: Option<Berth>,
    pub(super) plate: super::plating::Plating,
    pub director_lost: bool,
    pub(super) knock: Vec2,
    pub(super) way: Vec2,
    pub(super) ram_rest: f32,
}
impl Raider {
    pub(super) fn new(body: Body, class: EnemyClass) -> Self {
        let p = class.profile();
        let stations = class.stations();
        // Each gun and the director belong to the module beneath them.
        let design = class.design();
        let afts: Vec<f32> = stations.iter().map(|s| p.hull.bow - s.1).collect();
        let berths = design
            .berths_for(Component::MainGun, &afts)
            .expect("a berth under every gun station");
        let director = design
            .berths_for(Component::Director, &[p.hull.bow])
            .map(|found| found[0]);
        let guns = stations
            .iter()
            .enumerate()
            .map(|(i, &offset)| EnemyGun {
                offset,
                train: home(offset.0, offset.1),
                berth: berths[i],
                out: false,
                reload: 1.4 + p.reload * i as f32 / stations.len() as f32,
                charging: false,
                committed: None,
                plate: ArmorPanel::new(
                    if class == Battleship {
                        ArmorGrade::Reinforced
                    } else {
                        ArmorGrade::Light
                    },
                    16,
                    4,
                ),
            })
            .collect();
        let pairs = match class {
            Freighter | Patrol => 0,
            Destroyer => 1,
            Cruiser => 2,
            Battleship => 6,
            Parliament => 10,
            Regent => 12,
        };
        let mut secondaries = Vec::new();
        for bay in 0..pairs {
            for side in [-1., 1.] {
                let fore = ((bay as f32 + 0.5) / pairs as f32 - 0.5) * 1.34 * p.hull.stern;
                let offset = Vec2::new(
                    side * p.hull.radius * (0.91 - 0.15 * (fore / p.hull.stern).abs()),
                    fore,
                );
                let director = stations
                    .iter()
                    .enumerate()
                    .filter(|(_, station)| !class.is_flagship() || station.0.signum() == side)
                    .min_by(|(_, a), (_, b)| (a.1 - fore).abs().total_cmp(&(b.1 - fore).abs()))
                    .unwrap()
                    .0;
                secondaries.push(SecondaryMount {
                    strength: 2,
                    offset,
                    train: side * PI * 0.5,
                    reload: 0.4 + bay as f32 * 0.36,
                    director,
                });
            }
        }
        Self {
            secondaries,
            cargo_run: None,
            has_contact: false,
            last_sighting: None,
            body,
            class,
            guns,
            ship: shipvivor_shipyard::Ship::new(design),
            director,
            plate: super::plating::Plating::new(class),
            director_lost: false,
            knock: Vec2::ZERO,
            way: Vec2::ZERO,
            ram_rest: 0.,
        }
    }
    pub(super) fn disable_director(&mut self) {
        if let Some(director) = self.director {
            self.ship.strike_fitting(director, f32::MAX);
            self.sync();
        }
    }
    /// Bring the guns and the director into line with the ship's condition.
    fn sync(&mut self) {
        for gun in &mut self.guns {
            gun.out = self.ship.state(gun.berth) == State::Out;
        }
        let lost = self
            .director
            .is_some_and(|director| self.ship.state(director) == State::Out);
        if lost && !self.director_lost {
            for gun in &mut self.guns {
                gun.reload *= 1.65;
            }
        }
        self.director_lost = lost;
    }
    /// Hull points left across the ship's modules.
    pub fn hull(&self) -> f32 {
        self.ship.hull()
    }
    pub fn afloat(&self) -> bool {
        self.ship.afloat()
    }
    /// The ship's modules, what is fitted to them and the state of each.
    pub fn vessel(&self) -> &shipvivor_shipyard::Ship {
        &self.ship
    }
    pub fn guns(&self) -> &[EnemyGun] {
        &self.guns
    }
    pub fn motion(&self) -> Vec2 {
        self.way
    }
    pub fn destination_port(&self) -> Option<usize> {
        self.cargo_run.map(|(index, _)| index)
    }
    pub fn live_guns(&self) -> usize {
        self.guns.iter().filter(|g| g.live()).count()
    }
    pub fn protected(&self) -> bool {
        self.class.is_flagship() && self.guns.iter().any(EnemyGun::live)
    }
    /// A penetrating hit at a point on the hull. It lands on the module there.
    pub(super) fn hit_hull(&mut self, at: Vec2, damage: u16) {
        if self.protected() {
            return;
        }
        let ahead = (at - self.body.pos).dot(self.body.forward());
        let aft = self.class.profile().hull.bow - ahead;
        self.ship.strike_hull(aft, f32::from(damage));
        self.sync();
    }
    /// A penetrating hit on one gun. True if it knocked the gun out.
    pub(super) fn hit_gun(&mut self, index: usize, damage: u16) -> bool {
        let was_live = self.guns[index].live();
        self.ship
            .strike_fitting(self.guns[index].berth, f32::from(damage));
        self.sync();
        was_live && !self.guns[index].live()
    }
    /// Adrift: no engine is turning. The stripped flagship counts as well.
    pub fn is_disabled(&self) -> bool {
        if self.class.is_flagship() {
            !self.protected()
        } else {
            self.ship.propulsion() == 0.
        }
    }
    pub fn aim_point(&self, viewer: Vec2) -> Vec3 {
        let profile = self.class.profile();
        if self.protected() {
            self.guns
                .iter()
                .filter(|g| g.live())
                .map(|gun| gun.base(&self.body, profile) + Vec3::Y * profile.gun_scale)
                .min_by(|a, b| {
                    a.xz()
                        .distance_squared(viewer)
                        .total_cmp(&b.xz().distance_squared(viewer))
                })
                .unwrap()
        } else {
            // Once the middle of the hull is wrecked, the nearest sound module is the mark.
            let holds = self.ship.compartments();
            let design = self.ship.design();
            let bow = profile.hull.bow;
            let ahead = if holds[design.module_at(bow)].wrecked() {
                (0..holds.len())
                    .filter(|&i| !holds[i].wrecked())
                    .map(|i| {
                        let (fore, aft) = design.span(i);
                        bow - 0.5 * (fore + aft)
                    })
                    .min_by(|a, b| a.abs().total_cmp(&b.abs()))
                    .unwrap_or(0.)
            } else {
                0.
            };
            (self.body.pos + self.body.forward() * ahead)
                .extend(1.7)
                .xzy()
        }
    }

    /// Steer for `player`, clear of land and of ice big enough to hurt, while `stream`,
    /// the current where she lies, sets her along with it.
    pub(super) fn sail(
        &mut self,
        dt: f32,
        player: Vec2,
        ports: &[Port],
        coast: &[super::Landmark],
        ice: &[super::Iceberg],
        stream: Vec2,
    ) {
        let p = self.class.profile();
        // The crew mends what it can, and the repair party goes where it is needed most.
        self.ship.tend(dt);
        if self.ship.design().repair > 0. && self.ship.party().is_none() {
            let urgent = self.ship.triage();
            self.ship.assign(urgent);
        }
        self.sync();
        self.ram_rest = (self.ram_rest - dt).max(0.);
        self.body.pos += self.knock * dt;
        self.knock *= (-KNOCK_DRAG * dt).exp();
        let ahead = self.body.forward();
        let drive = if self.is_disabled() {
            ahead * 0.65
        } else {
            let offset = player - self.body.pos;
            let dir = offset.normalize_or_zero();
            let mut along = if self.class == Freighter {
                dir
            } else if self.class == Patrol {
                if offset.length() > p.stand_off {
                    let lane = if self.body.id.is_multiple_of(2) {
                        1.
                    } else {
                        -1.
                    };
                    let sweep = ((offset.length() - 65.) / 145.).clamp(0., 1.);
                    (dir + dir.perp() * lane * 0.66 * sweep).normalize_or_zero()
                } else {
                    dir.perp()
                        * if self.body.id.is_multiple_of(2) {
                            1.
                        } else {
                            -1.
                        }
                }
            } else {
                // A circling course keeps the long gun decks in profile.
                let mut closing = ((offset.length() - p.stand_off) / 44.).clamp(-0.85, 1.);
                let mut side = if self.body.id.is_multiple_of(2) {
                    1.
                } else {
                    -1.
                };
                if self.class == Regent {
                    // Keep the great hull oblique on the approach. As batteries fall,
                    // wheel the surviving broadside into view, giving both ships a shot.
                    closing = ((offset.length() - p.stand_off) / 220.).clamp(-0.75, 0.72);
                    let balance: i32 = self
                        .guns
                        .iter()
                        .filter(|g| g.live())
                        .map(|g| i32::from(g.offset.0 > 0.) - i32::from(g.offset.0 < 0.))
                        .sum();
                    if balance.abs() >= 2 {
                        side = -balance.signum() as f32;
                    }
                }
                (dir * closing + dir.perp() * side * (1. - closing.abs())).normalize_or_zero()
            };
            for (center, radius) in ports
                .iter()
                .map(|p| (p.body.pos, ISLAND_RADIUS))
                .chain(coast.iter().map(|p| (p.body.pos, p.radius)))
                .chain(
                    ice.iter()
                        .filter(|b| b.radius >= super::AVOIDED_RADIUS)
                        .map(|b| (b.body.pos, b.radius)),
                )
            {
                let from = self.body.pos;
                let radial = (from - center).normalize_or_zero();
                if radial.dot(along) < 0.
                    && entry(
                        from,
                        from + along * (32. + p.hull.bow),
                        center,
                        radius + p.hull.radius + 4.,
                    )
                    .is_some()
                {
                    let tangent = radial.perp();
                    along = if tangent.dot(dir) >= 0. {
                        tangent
                    } else {
                        -tangent
                    };
                }
            }
            if self.class == Patrol {
                self.body.heading = bearing(along);
            } else {
                train_full(
                    &mut self.body.heading,
                    bearing(along),
                    (p.speed / p.hull.bow) * dt,
                );
            }
            // Damaged engines and flooded modules take way off her.
            let pace = if self.class.is_flagship() {
                1.
            } else {
                self.ship.propulsion()
            };
            self.body.forward() * p.speed * pace
        };
        self.way = self.knock + drive + stream;
        self.body.pos += (drive + stream) * dt;
        for island in coast {
            self.body.pos = island.clear(self.body.pos, p.hull.radius);
        }
        for port in ports {
            self.body.pos = port.clear_land(self.body.pos, p.hull.radius.max(3.));
        }
    }

    pub(super) fn stand_down(&mut self) {
        for gun in &mut self.guns {
            gun.charging = false;
            if gun.committed.take().is_some() {
                gun.reload = REGENT_WARNING;
            }
        }
    }

    /// Lay every gun that bears on where the player will be, and fire those loaded.
    /// `way` is the player's velocity, so a ship holding her course is led exactly.
    pub(super) fn fire_guns(
        &mut self,
        dt: f32,
        player: Vec2,
        way: Vec2,
        shots: &mut Vec<Shot>,
        effects: &mut Vec<Effect>,
        serial: &mut u64,
    ) {
        // A ship without power cannot work her guns, nor can the stripped flagship.
        if !self.ship.powered() || (self.class.is_flagship() && !self.protected()) {
            return;
        }
        let p = self.class.profile();
        let close_range = if self.class == Regent {
            175.
        } else {
            p.range * 0.80
        };
        for (index, gun) in self.secondaries.iter_mut().enumerate() {
            gun.reload = (gun.reload - dt).max(0.);
            if !gun.live() || !self.guns[gun.director].live() {
                continue;
            }
            let at = gun.station(&self.body, p) + Vec3::Y * 0.7;
            let ahead = super::projectile::lead(at.xz(), player, way, Battery::EnemySecondary);
            let delta = ahead - at.xz();
            let normal = self.body.forward().perp() * gun.offset.x.signum();
            if delta.dot(normal) < delta.length() * 0.08
                || delta.length_squared() > close_range.powi(2)
            {
                continue;
            }
            let trained = train_full(
                &mut gun.train,
                bearing(delta) - self.body.heading,
                76_f32.to_radians() * dt,
            );
            if trained && gun.reload == 0. {
                let bracket = ((*serial + index as u64 * 3) % 9) as f32 - 4.;
                let mark = ahead + delta.normalize_or_zero().perp() * bracket * 2.2;
                super::fire(
                    shots,
                    effects,
                    serial,
                    at,
                    mark.extend(1.5).xzy(),
                    Battery::EnemySecondary,
                    1,
                );
                gun.reload =
                    if self.class == Regent { 5.4 } else { 2.8 } + (index % 3) as f32 * 0.3;
            }
        }
        for gun in &mut self.guns {
            gun.charging = false;
            if !gun.live() {
                continue;
            }
            let pivot = gun.base(&self.body, p) + Vec3::Y * p.gun_scale * 0.72;
            let ahead = gun.committed.unwrap_or_else(|| {
                super::projectile::lead(pivot.xz(), player, way, Battery::Raider)
            });
            let offset = ahead - pivot.xz();
            let heading = self.body.forward();
            let direction = offset.normalize_or_zero();
            let (side, fore, _) = gun.offset;
            // Wing turrets command their own side. A stripped flank stays quiet.
            let clear_arc = if side != 0. {
                direction.dot(heading.perp()) * side.signum() >= -0.10
            } else {
                self.class == Patrol || direction.dot(heading) * fore.signum() > -0.58
            };
            let wanted = if clear_arc {
                bearing(offset) - self.body.heading
            } else {
                home(side, fore)
            };
            let trained = train_full(&mut gun.train, wanted, p.traverse.to_radians() * dt);
            if !clear_arc || offset.length_squared() > p.range * p.range {
                if gun.committed.take().is_some() {
                    gun.reload = REGENT_WARNING;
                }
                continue;
            }
            if !trained {
                gun.charging = gun.committed.is_some();
                continue;
            }
            gun.reload -= dt;
            if self.class == Regent && gun.reload <= REGENT_WARNING && gun.committed.is_none() {
                gun.reload = REGENT_WARNING;
                gun.committed = Some(super::projectile::lead(
                    pivot.xz(),
                    player + way * REGENT_WARNING,
                    way,
                    Battery::Raider,
                ));
            }
            gun.charging = gun.committed.is_some() || gun.reload < 0.85;
            if gun.reload > 0. {
                continue;
            }
            // Outer rounds bracket the player, leaving room to steer through the fan.
            let side = offset.normalize_or_zero().perp();
            for tube in 0..p.barrels {
                let index = f32::from(tube) - 0.5 * f32::from(p.barrels - 1);
                let from = pivot + (side * index * 0.55 * p.gun_scale).extend(0.).xzy();
                // Twin mounts straddle the current position more tightly.
                let spread = match self.class {
                    Parliament | Regent => 9.,
                    Battleship => 5.5,
                    _ => 2.5,
                };
                let target = (ahead + side * index * spread).extend(1.8).xzy();
                if let Some(path) =
                    FlightPath::from_barrel(from, target, Battery::Raider, 3.2 * p.gun_scale)
                {
                    path.launch(shots, effects, serial, Battery::Raider, p.damage);
                }
            }
            gun.reload = p.reload * if self.director_lost { 1.65 } else { 1. };
            gun.charging = false;
            gun.committed = None;
        }
    }
}
