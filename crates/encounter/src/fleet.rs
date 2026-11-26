//! Authored arcade hulls and deck stations. The sight, hit tests, gunfire and
//! renderer all use the same station positions in the ship's rotating frame.
use super::{Battery, Body, Effect, Hull, ISLAND_RADIUS, KNOCK_DRAG, Port, Shot, bearing};
use super::{
    mounts::train_full,
    pilot::{Helm, clear_course},
    projectile::FlightPath,
    torpedo_boat::HARRYING_SCATTER,
};
use glam::{Vec2, Vec3, Vec3Swizzles};
use shipvivor_artillery::ArmorPanel;
use shipvivor_shipyard::{Berth, Component, Design, State};
use std::f32::consts::PI;
use std::sync::{Arc, OnceLock};

const REGENT_WARNING: f32 = 3.2;
/// Damage the Parliament's bridge, the tower amidships, stands before it is smashed:
/// three stock rounds, or two heavier ones. It can be hit while her turrets protect
/// the hull.
const BRIDGE_STRENGTH: u16 = 5;
/// Seconds she holds her course at reduced way once her bridge is smashed, until she
/// is conned from aft.
pub const CONN_LOST: f32 = 40.;
/// Share of her speed she keeps while nobody has the conn.
pub const CONN_PACE: f32 = 0.35;
/// Metres a second a ship adrift keeps once her engines stop.
const DRIFT: f32 = 0.65;
/// Share of a main-battery round, a torpedo or a bomb into a flagship's hull that
/// reaches it while her turrets screen it (see [`Raider::screened`]).
pub const SCREENED: f32 = 0.25;
/// How much farther than her true range a warship's main guns reach, as a share of it.
/// With [`LONG_LAY`], the simulator's bold captain takes a tenth to a third more damage
/// a minute than with no long fire. Half again her range, with an error that grew with
/// the square, doubled it.
const LONG_RANGE: f32 = 1.3;
/// How far a salvo fired at her greatest reach is laid off along the line of fire and
/// across it, at most. Nearer, the error shrinks in step with how far past her true
/// range the mark lies, to nothing at it. The whole salvo shares the error.
const LONG_LAY: (f32, f32) = (45., 18.);

/// The share of [`LONG_LAY`] a salvo is laid off by at `distance`, from guns laid true
/// out to `range` that reach to `reach`: none at her true range, all of it at her
/// reach, and in step between.
pub(super) fn long_lay(distance: f32, range: f32, reach: f32) -> f32 {
    ((distance - range) / (reach - range).max(1.)).clamp(0., 1.)
}
/// Dock points' worth of cargo in a coaster's hold, and in a freighter's.
pub const COASTER_CARGO: f32 = 0.25;
pub const FREIGHTER_CARGO: f32 = 1.;
/// Cargo is reckoned in these shares of a dock point. Every share is a whole number of
/// them and a power of two divides it, so the shares of a sector add up exactly.
const CARGO_GRAIN: f32 = 1024.;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EnemyClass {
    Freighter,
    /// A small coastal trader, a quarter of a freighter's length. She is unarmed, keeps
    /// no lookout and plies between the yards with a deck cargo.
    Coaster,
    Patrol,
    /// A fast, fragile boat with one torpedo and no guns. She dashes out from her base's
    /// mooring, puts her torpedo in the water across Breakwater's course and runs home to
    /// reload (see `torpedo_boat`).
    TorpedoBoat,
    Destroyer,
    Cruiser,
    Battleship,
    Parliament,
    Regent,
    /// The boss of the second and third rounds: a battleship with a fourth turret and
    /// gold bands on her turrets, about a fifth more gunfire than a battleship and a
    /// third more hull. She has no turret screen and sinks as any battleship does.
    Commodore,
}
use EnemyClass::*;

pub struct Profile {
    pub hull: Hull,
    pub deck: f32,
    pub gun_scale: f32,
    pub barrels: u8,
    speed: f32,
    /// Radius in metres of the circle she turns in with the rudder hard over. At full
    /// speed it sets the fastest she can swing; with less way on she swings slower.
    turning: f32,
    stand_off: f32,
    /// Metres out to which her main guns are laid true. Past it, out to her class's
    /// [`EnemyClass::reach`], they still fire, laid off by an error that grows to the end.
    range: f32,
    traverse: f32,
    reload: f32,
    damage: u16,
    pub stores: u8,
}
impl Profile {
    /// Metres a second she makes at full speed.
    pub fn speed(&self) -> f32 {
        self.speed
    }
    /// Metres out to which her main guns are laid true.
    pub fn range(&self) -> f32 {
        self.range
    }
    /// Metres off the ship at which she circles her on her own.
    pub fn stand_off(&self) -> f32 {
        self.stand_off
    }
}

impl EnemyClass {
    /// Every class, for whatever is built for each of them up front, such as the battle
    /// view's meshes. `listed` below keeps it whole.
    pub const ALL: [EnemyClass; 10] = [
        Freighter,
        Coaster,
        Patrol,
        TorpedoBoat,
        Destroyer,
        Cruiser,
        Battleship,
        Parliament,
        Regent,
        Commodore,
    ];

    /// Never called. The match is exhaustive, so a new class stops the build here until
    /// she is added to it, and to `ALL` beside it.
    #[allow(dead_code)]
    fn listed(self) {
        match self {
            Freighter | Coaster | Patrol | TorpedoBoat | Destroyer | Cruiser | Battleship
            | Parliament | Regent | Commodore => {}
        }
    }

    /// A flagship proper: her turrets screen her hull, and she is stripped rather than
    /// sunk.
    pub fn is_flagship(self) -> bool {
        matches!(self, Parliament | Regent)
    }
    /// The ship a leg builds up to: she is announced, charted and must be sunk before
    /// the leg is won. The Commodore in the early rounds, the flagships after.
    pub fn is_boss(self) -> bool {
        matches!(self, Commodore | Parliament | Regent)
    }

    /// An unarmed cargo ship: she keeps no lookout, never gets word of the player and
    /// is no part of a raid.
    pub fn is_merchant(self) -> bool {
        matches!(self, Freighter | Coaster)
    }

    /// Dock points' worth of cargo she carries when full.
    pub fn cargo(self) -> f32 {
        match self {
            Freighter => FREIGHTER_CARGO,
            Coaster => COASTER_CARGO,
            _ => 0.,
        }
    }
    fn cargo_grains(self) -> u32 {
        (self.cargo() * CARGO_GRAIN).round() as u32
    }

    /// The farthest her main guns fire, laid off more the farther past her true range.
    /// A torpedo boat's little gun harries her only from close aboard.
    pub fn reach(self) -> f32 {
        let range = self.profile().range;
        if self == TorpedoBoat {
            range
        } else {
            range * LONG_RANGE
        }
    }

    pub fn lookout_range(self) -> f32 {
        match self {
            Freighter | Coaster => 0.,
            Patrol | TorpedoBoat => 520.,
            Destroyer => 650.,
            Cruiser => 770.,
            Battleship | Commodore | Parliament | Regent => 900.,
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
                turning: 30.,
                stand_off: 0.,
                range: 0.,
                traverse: 0.,
                reload: 0.,
                damage: 0,
                stores: 4,
            },
            // A quarter of the freighter: 8 m, beamy for her length and light. She pays
            // in cargo and experience, and carries no stores.
            Coaster => &Profile {
                hull: Hull {
                    radius: 1.25,
                    bow: 4.2,
                    stern: 3.8,
                    mass: 0.15,
                    shock_per_point: 1.2,
                },
                deck: 0.95,
                gun_scale: 0.5,
                barrels: 0,
                // As tight a turn as any class may swing at full speed: a destroyer's.
                speed: 7.,
                turning: 19.,
                stand_off: 0.,
                range: 0.,
                traverse: 0.,
                reload: 0.,
                damage: 0,
                stores: 0,
            },
            Patrol => &Profile {
                hull: Hull::RAIDER,
                deck: 1.65,
                gun_scale: 0.8,
                barrels: 1,
                speed: 5.4,
                turning: 16.,
                stand_off: 38.,
                range: 210.,
                traverse: 52.,
                reload: 4.5,
                damage: 7,
                stores: 1,
            },
            // Twelve metres, slim and low, and half again a destroyer's speed. She turns
            // as tightly as any class may at full speed. Her torpedoes are her weapon;
            // the little gun on her forecastle harries the ship between runs and after
            // them, a pinprick that keeps her in mind. She carries no stores.
            TorpedoBoat => &Profile {
                hull: Hull {
                    radius: 1.35,
                    bow: 6.4,
                    stern: 5.6,
                    mass: 0.2,
                    shock_per_point: 1.2,
                },
                deck: 1.2,
                gun_scale: 0.6,
                barrels: 1,
                speed: 11.,
                turning: 29.,
                stand_off: 0.,
                range: super::torpedo_boat::HARRYING_RANGE + 40.,
                traverse: 70.,
                reload: 4.5,
                damage: 1,
                stores: 0,
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
                turning: 20.,
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
                turning: 26.,
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
                turning: 40.,
                stand_off: 185.,
                range: 440.,
                traverse: 19.,
                reload: 10.5,
                damage: 7,
                stores: 7,
            },
            // A battleship stretched by a turret, a little quicker on her helm and her
            // loading, and better found.
            Commodore => &Profile {
                hull: Hull {
                    radius: 6.2,
                    bow: 31.,
                    stern: 27.,
                    mass: 34.,
                    shock_per_point: 3.,
                },
                deck: 3.3,
                gun_scale: 2.15,
                barrels: 3,
                speed: 3.6,
                turning: 42.,
                stand_off: 195.,
                range: 450.,
                traverse: 21.,
                reload: 10.,
                damage: 6,
                stores: 9,
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
                turning: 110.,
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
                turning: 60.,
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
        static YARD: OnceLock<[Arc<Design>; 10]> = OnceLock::new();
        YARD.get_or_init(|| {
            [
                Design::freighter(),
                Design::coaster(),
                Design::patrol_boat(),
                Design::torpedo_boat(),
                Design::destroyer(),
                Design::cruiser(),
                Design::battleship(),
                Design::flagship(),
                Design::iron_regent(),
                Design::commodore(),
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
            Freighter | Coaster => &[],
            Patrol => &[(0., 4.2, 0.)],
            // On the turtleback, forward of her bridge.
            TorpedoBoat => &[(0., 4., 0.24)],
            Destroyer => &[(0., 5., 0.), (0., -4.5, 0.)],
            Cruiser => &[(0., 8.7, 0.), (0., -8.5, 0.)],
            Battleship => &[(0., 17.5, 0.), (0., 9., 2.8), (0., -16., 0.)],
            // A and B forward, X and Y aft, the inner pair superfiring.
            Commodore => &[
                (0., 20.5, 0.),
                (0., 12.5, 2.8),
                (0., -9., 2.8),
                (0., -16.5, 0.),
            ],
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
    /// The module she stands on. The gun goes with it when it is wrecked.
    module: usize,
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
    /// The base a coaster lies at anchor off, until it is roused and she weighs.
    pub(super) moored: Option<usize>,
    /// Cargo still in her holds, in grains.
    pub(super) cargo: u32,
    pub body: Body,
    pub class: EnemyClass,
    /// How heavily she is gunned: the barrels in each of her turrets.
    weight: super::Weight,
    pub has_contact: bool,
    pub(super) last_sighting: Option<Vec2>,
    pub(super) guns: Vec<EnemyGun>,
    pub(super) ship: shipvivor_shipyard::Ship,
    director: Option<Berth>,
    pub(super) plate: super::plating::Plating,
    pub director_lost: bool,
    /// What the Parliament's bridge has left. Other classes have none to hit.
    bridge: u16,
    /// Seconds left before she is under command again after losing her bridge.
    conn_lost: f32,
    helm: Helm,
    pub(super) knock: Vec2,
    pub(super) way: Vec2,
    pub(super) ram_rest: f32,
    /// A torpedo boat's runs from her base's mooring. Other classes have none.
    pub(super) attack: Option<super::torpedo_boat::Attack>,
    /// Her part in her squadron this tick, if she sails in one (see `formation`).
    pub(super) order: Option<super::formation::Order>,
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
                plate: ArmorPanel::new(class.turret_plate(), 16, 4),
            })
            .collect();
        let pairs = match class {
            Freighter | Coaster | Patrol | TorpedoBoat => 0,
            Destroyer => 1,
            Cruiser => 2,
            Battleship => 6,
            Commodore => 8,
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
                secondaries.push(SecondaryMount {
                    strength: 2,
                    offset,
                    train: side * PI * 0.5,
                    reload: 0.4 + bay as f32 * 0.36,
                    module: design.module_at(p.hull.bow - fore),
                });
            }
        }
        Self {
            secondaries,
            cargo_run: None,
            moored: None,
            cargo: class.cargo_grains(),
            has_contact: false,
            last_sighting: None,
            body,
            class,
            weight: super::Weight::stock(class),
            guns,
            ship: shipvivor_shipyard::Ship::new(design),
            director,
            plate: super::plating::Plating::new(class),
            director_lost: false,
            bridge: if class == Parliament {
                BRIDGE_STRENGTH
            } else {
                0
            },
            conn_lost: 0.,
            // She is already under way when she comes into the sector or off the slip.
            helm: Helm::under_way(p.speed),
            knock: Vec2::ZERO,
            way: Vec2::ZERO,
            ram_rest: 0.,
            attack: None,
            order: None,
        }
    }
    /// Metres a second of way she has on through the water.
    pub(super) fn way_on(&self) -> f32 {
        self.helm.speed
    }
    /// Whether she sails in a squadron this tick, as its guide or keeping her station.
    pub fn in_squadron(&self) -> bool {
        self.order.is_some()
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
        let holds = self.ship.compartments();
        for gun in &mut self.secondaries {
            if holds[gun.module].wrecked() {
                gun.strength = 0;
            }
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
    /// The base she lies at anchor off, if she has not yet weighed.
    pub fn moored_at(&self) -> Option<usize> {
        self.moored
    }
    /// Lie at anchor off base `base`, with no way on.
    pub(super) fn moor(&mut self, base: usize) {
        self.moored = Some(base);
        self.helm = Helm::under_way(0.);
    }
    /// Let go her anchor off base `base` as she comes in under way. She carries her way
    /// off as she rides up to it, rather than stopping dead.
    pub(super) fn anchor(&mut self, base: usize) {
        self.moored = Some(base);
    }
    /// Weigh anchor to sail as a warship does.
    pub(super) fn cast_off(&mut self) {
        self.moored = None;
    }
    /// Weigh anchor for the yard `bound`, and come back to `origin` should it fall.
    pub(super) fn weigh(&mut self, bound: usize, origin: Vec2) {
        self.moored = None;
        self.cargo_run = Some((bound, origin));
    }
    /// Dock points' worth of cargo still aboard.
    pub fn cargo(&self) -> f32 {
        self.cargo as f32 / CARGO_GRAIN
    }
    /// Cargo lost with the hull she has lost since the last count, in dock points. Her
    /// holds empty in proportion to her hull, and a crew that mends her hull cannot mend
    /// what has gone over the side.
    pub(super) fn spill_cargo(&mut self) -> f32 {
        let left =
            (self.class.cargo_grains() as f32 * self.ship.integrity().clamp(0., 1.)).round() as u32;
        let spilt = self.cargo.saturating_sub(left);
        self.cargo -= spilt;
        spilt as f32 / CARGO_GRAIN
    }
    /// Everything still in her holds, which goes down with her.
    pub(super) fn founder_cargo(&mut self) -> f32 {
        std::mem::take(&mut self.cargo) as f32 / CARGO_GRAIN
    }
    pub fn live_guns(&self) -> usize {
        self.guns.iter().filter(|g| g.live()).count()
    }
    /// How heavily she is gunned. Classes that come in no weights are standard.
    pub fn weight(&self) -> super::Weight {
        self.weight
    }
    /// Gun her as a ship of `weight`. Classes that come in no weights stay as they are.
    pub(super) fn reweigh(&mut self, weight: super::Weight) {
        if super::Weight::row(self.class).is_some() {
            self.weight = weight;
        }
    }
    /// Barrels in each of her main turrets: her weight's, or her class's own for a class
    /// that comes in no weights.
    pub fn barrels(&self) -> u8 {
        if super::Weight::row(self.class).is_some() {
            self.weight.barrels()
        } else {
            self.class.profile().barrels
        }
    }
    /// A flagship with a turret still standing. Once the last is gone she is stripped:
    /// silent and adrift.
    pub fn protected(&self) -> bool {
        self.class.is_flagship() && self.guns.iter().any(EnemyGun::live)
    }
    /// A flagship with four in five of her turrets standing, which screen her hull.
    /// Light guns can't hurt it, and anything heavier does [`SCREENED`] of its damage,
    /// except where a strake has been shot off her side.
    pub fn screened(&self) -> bool {
        self.class.is_flagship() && self.live_guns() * 5 >= self.guns.len() * 4
    }
    /// A penetrating hit at a point on the hull. It lands on the plating there and
    /// the module behind it, cut to [`SCREENED`] while her turrets screen her.
    pub(super) fn hit_hull(&mut self, at: Vec2, damage: u16) {
        let share = if self.screened() { SCREENED } else { 1. };
        self.strike_hull_at(at, f32::from(damage) * share);
    }
    /// A penetrating hit where a strake has been shot off her side: no turret screens it.
    pub(super) fn hit_bare_hull(&mut self, at: Vec2, damage: u16) {
        self.strike_hull_at(at, f32::from(damage));
    }
    fn strike_hull_at(&mut self, at: Vec2, damage: f32) {
        let offset = at - self.body.pos;
        let forward = self.body.forward();
        let aft = self.class.profile().hull.bow - offset.dot(forward);
        // Her plate grid has already let the round in, so no calibre is given here.
        self.ship
            .strike_side(aft, offset.dot(forward.perp()), damage, None);
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
    /// Whether she has a bridge still standing to be shot away.
    pub fn bridge_standing(&self) -> bool {
        self.bridge > 0
    }
    /// Seconds left before she is under command again, or zero when she is.
    pub fn helm_lost(&self) -> f32 {
        self.conn_lost
    }
    /// A penetrating hit on her bridge. True if it smashed it: she holds her course at
    /// a fraction of her speed for [`CONN_LOST`] seconds. A bridge is smashed only once.
    pub(super) fn hit_bridge(&mut self, damage: u16) -> bool {
        let standing = self.bridge_standing();
        self.bridge = self.bridge.saturating_sub(damage);
        let smashed = standing && !self.bridge_standing();
        if smashed {
            self.conn_lost = CONN_LOST;
            // The wheel goes with the bridge, and her rudder trails amidships.
            self.helm.rudder = 0.;
        }
        smashed
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
    /// the current where she lies, sets her along with it. `mark` says `player` is only
    /// a fairway mark leading her round a headland towards her quarry: she makes
    /// straight for it rather than holding her stand-off from it.
    pub(super) fn sail(
        &mut self,
        dt: f32,
        player: Vec2,
        mark: bool,
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
        self.conn_lost = (self.conn_lost - dt).max(0.);
        self.body.pos += self.knock * dt;
        self.knock *= (-KNOCK_DRAG * dt).exp();
        // A heavier hull takes longer to gather way, or to lose it.
        let gather = p.speed / (4. + 2. * p.hull.mass.cbrt());
        let anchored = self.moored.is_some();
        let stream = if anchored { Vec2::ZERO } else { stream };
        if anchored {
            // At anchor she rides to her cable: no way on, and the stream holds her.
            self.helm.make_way(0., gather, dt);
            self.helm.amidships(dt);
        } else if self.is_disabled() {
            // Adrift, she carries her way off and her rudder trails amidships.
            self.helm.make_way(DRIFT, gather, dt);
            self.helm.amidships(dt);
        } else {
            use super::formation::Order;
            // In a squadron she keeps her station on the guide, wherever her quarry is.
            let keeping = match self.order {
                Some(Order::Keep(station)) => Some(station),
                _ => None,
            };
            let player = keeping.map_or(player, |station| station.at);
            let offset = player - self.body.pos;
            let dir = offset.normalize_or_zero();
            // A torpedo boat makes straight for her firing position or her mooring.
            let along = if let Some(station) = keeping {
                station.course(self.body.pos)
            } else if mark || self.class.is_merchant() || self.class == TorpedoBoat {
                dir
            } else if self.class == Patrol {
                let lane = if self.body.id.is_multiple_of(2) {
                    1.
                } else {
                    -1.
                };
                // Far off she sweeps in on a slant. Nearer, she bends from her run-in onto
                // a circle at her stand-off, and from inside it she opens out again.
                let range = offset.length();
                let sweep = ((range - 65.) / 145.).clamp(0., 1.);
                let closing = ((range - p.stand_off) / (65. - p.stand_off)).clamp(-0.85, 1.);
                (dir * closing + dir.perp() * lane * (0.66 * sweep + 1. - closing.abs()))
                    .normalize_or_zero()
            } else if let Some(Order::Guide {
                ring: Some(ring), ..
            }) = self.order
            {
                // The guide of a line of battle wheels about her at its range, in a wide
                // sweep the line can follow, and gives little ground when she closes.
                let closing = ((offset.length() - ring) / 90.).clamp(-0.45, 1.);
                let side = if self.body.id.is_multiple_of(2) {
                    1.
                } else {
                    -1.
                };
                (dir * closing + dir.perp() * side * (1. - closing.abs())).normalize_or_zero()
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
            // Land and heavy ice she keeps clear of, with room for her hull. A quay juts
            // from its island, so a ship leaving the slip for water on the quay's side
            // steers round its end rather than into it. She closes a hazard only as near
            // as her destination lies to it, so she can come alongside a berth.
            let hazards = ports
                .iter()
                .map(|p| (p.body.pos, ISLAND_RADIUS))
                .chain(ports.iter().flat_map(Port::piers))
                .chain(coast.iter().map(|p| (p.body.pos, p.radius)))
                .chain(
                    ice.iter()
                        .filter(|b| b.radius >= super::AVOIDED_RADIUS)
                        .map(|b| (b.body.pos, b.radius)),
                )
                .map(|(centre, radius)| {
                    let hull = radius + p.hull.radius;
                    (
                        centre,
                        (hull + 4.).min(centre.distance(player) - 1.).max(hull + 1.),
                    )
                });
            // She looks far enough ahead to come round in time, but not past her goal. A
            // station moves on ahead of her, so she looks ahead all the way.
            let ahead = 32. + p.hull.bow + p.turning;
            let reach = if keeping.is_some() {
                ahead
            } else {
                ahead.min(offset.length())
            };
            let course = clear_course(self.body.pos, along, self.body.forward(), reach, hazards);
            if self.conn_lost > 0. {
                // With her bridge shot away nobody answers the helm: she holds her course.
            } else if course == Vec2::ZERO {
                self.helm.amidships(dt);
            } else {
                self.helm
                    .steer(self.body.heading, bearing(course), p.turning, dt);
            }
            // Damaged engines and flooded modules take way off her.
            let pace = if self.conn_lost > 0. {
                CONN_PACE
            } else if self.class.is_flagship() {
                1.
            } else {
                self.ship.propulsion()
            };
            // A torpedo boat eases her way as she comes up to her mooring. A squadron
            // keeps the pace of its slowest ship, and each of the rest her station.
            let throttle = match self.order {
                Some(Order::Keep(station)) => station.throttle(self.body.pos, p.speed),
                Some(Order::Guide { pace, .. }) => pace,
                None => self.attack.as_ref().map_or(1., |run| run.throttle),
            };
            self.helm.make_way(p.speed * pace * throttle, gather, dt);
        }
        // The rudder turns her only as fast as her way through the water allows.
        self.body.heading = self.helm.turn(self.body.heading, p.turning, dt);
        let drive = self.body.forward() * self.helm.speed;
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
        // Each gun on the side she lies on follows her from the moment her lookouts have
        // her, and opens fire once she comes within its range. Each is worked by its own
        // crew, so it fights on whatever becomes of the turrets beside it.
        for (index, gun) in self.secondaries.iter_mut().enumerate() {
            gun.reload = (gun.reload - dt).max(0.);
            if !gun.live() {
                continue;
            }
            let at = gun.station(&self.body, p) + Vec3::Y * 0.7;
            let ahead = super::projectile::lead(at.xz(), player, way, Battery::EnemySecondary);
            let delta = ahead - at.xz();
            let normal = self.body.forward().perp() * gun.offset.x.signum();
            let step = 76_f32.to_radians() * dt;
            if delta.dot(normal) < delta.length() * 0.08 {
                // She is on the other side: the gun comes back to the beam.
                train_full(&mut gun.train, gun.offset.x.signum() * PI * 0.5, step);
                continue;
            }
            let trained = train_full(&mut gun.train, bearing(delta) - self.body.heading, step);
            if trained && gun.reload == 0. && delta.length_squared() <= close_range.powi(2) {
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
        let reach = self.class.reach();
        let barrels = self.barrels();
        for (number, gun) in self.guns.iter_mut().enumerate() {
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
            if !clear_arc || offset.length_squared() > reach * reach {
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
            // Past her true range the whole salvo is laid off, worse the farther out. The
            // Regent's guns fall on the patch of water they committed to.
            let past = long_lay(offset.length(), p.range, reach);
            // A torpedo boat's little gun is laid by eye from a lively deck: harassing
            // fire that walks about her more often than it finds her.
            let ahead = if self.class == TorpedoBoat {
                let long = ((*serial * 7) % 9) as f32 - 4.;
                let across = ((*serial * 3) % 5) as f32 - 2.;
                ahead + direction * long * HARRYING_SCATTER.0 + side * across * HARRYING_SCATTER.1
            } else if past > 0. && gun.committed.is_none() {
                let seed = self.body.id.wrapping_mul(31).wrapping_add(number as u64);
                let spread = |draw| super::ranging::spread(seed, *serial as u32, draw);
                ahead
                    + direction * LONG_LAY.0 * past * spread(0)
                    + side * LONG_LAY.1 * past * spread(1)
            } else {
                ahead
            };
            for tube in 0..barrels {
                let index = f32::from(tube) - 0.5 * f32::from(barrels - 1);
                let from = pivot + (side * index * 0.55 * p.gun_scale).extend(0.).xzy();
                // Twin mounts straddle the current position more tightly.
                let spread = match self.class {
                    Parliament | Regent => 9.,
                    Battleship | Commodore => 5.5,
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
