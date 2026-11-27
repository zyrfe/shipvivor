//! Deterministic arcade encounters. The renderer reads these actors and effects.
mod chart;
pub use chart::{Chart, Course, HarbourSite, Ice, Islet, STRAIT, Waters, leg_seed};
mod coast;
pub use coast::{
    FOOTING, Feature, FeatureKind, Landmark, LandmarkKind, MAINLAND_CLIFF, MAINLAND_CREST, Site,
    mainland_top,
};
mod current;
mod dev;
pub use current::{Currents, Gyre, MAX_CURRENT, Set};
mod iceberg;
pub use iceberg::{AVOIDED_RADIUS, IceShape, Iceberg};
mod survey;
pub use survey::{Defect, GATE_HALF_WIDTH, Metrics, Survey};
mod supply;
pub use supply::{
    COASTER_INTERVAL, COASTER_LIMIT, DRYDOCK_STORES, FACTORY_STORES, MOORED_COASTERS, Package,
    Rewards, TOW_RANGE,
};
mod fleet;
pub use fleet::{COASTER_CARGO, EnemyClass, EnemyGun, FREIGHTER_CARGO, Profile, Raider};
mod weight;
pub use weight::Weight;
mod doctrine;
pub use doctrine::{Doctrine, RUN_ROUNDS, Surprise};
mod formation;
pub use formation::{DEPLOY, LINE_RANGE, SQUADRON_PACE, close_order, line_range};
mod flak;
pub use flak::{AaMount, Airframe, BURST_RADIUS};
mod industrial;
mod mounts;
mod pilot;
mod plating;
mod port;
pub use industrial::IndustrialYard;
pub use mounts::{Control, DeckOffsets, MainMount, MountStatus, Station, WingGun};
pub use plating::{HULL_OUTLINE, STRAKE_RISE, STRAKES_A_SIDE, Scar, Strake, breakwater_outline};
mod air_station;
mod airfield;
pub use airfield::{
    AIRFIELD_RADIUS, AIRFIELD_WORKS, Airfield, AirfieldPart, DEPOT, DEPOT_SIZE, FACTORIES,
    FIELD_LEVEL, Field, FieldWork, HANGAR_SIZE, HANGARS, HEAVY_GUNS, LIGHT_GUNS, PLANE_HEIGHT,
    PLANE_REACH, Parked, PlaneKind, RUNWAY, RUNWAY_HALF_WIDTH, RUNWAY_LINE, Roll, SHELTER_LINE,
    SHELTER_SIZE, SHELTERS, Shelter, TAXIWAY_END, TAXIWAY_LINE, TOWER, TOWER_SIZE, TRUCK_HEIGHT,
    TRUCK_REACH, Taxi, Truck, Warplane,
};
mod projectile;
mod ram;
mod ranging;
mod recon;
pub use air_station::{AirRaid, AirStation, AttackPlane};
mod harrier;
pub use harrier::{
    FIRST_FORMATION, FIRST_SORTIE, Harrier, LAST_FORMATION, MOST_ALOFT, MooredHarrier, PASSES,
    REFIT, ROUND_DAMAGE, ROUNDS, RUN_DOWN, SORTIE_INTERVAL, formation_side,
};
pub use recon::{AIR_SIGHT, ALERT_SECONDS, BASE_SIGHT, Spotter};
mod aviation;
mod torpedo;
mod torpedo_boat;
pub use aviation::{
    Bomb, CATAPULT_DECK, Flight, Floatplane, Launcher, Missile, catapult_layout, launcher_layout,
    plane_index, planes_on,
};
use glam::{Vec2, Vec3, Vec3Swizzles};
pub use port::{
    DOCK_STATIONS, Dock, Factory, ISLAND_RADIUS, Pock, Port, REBUILD_SECONDS, SHORE_RANGE,
    SPAWN_STANDOFF, ShoreTurret, YARD_CAPACITY, YardState, dock_count,
};
pub use projectile::{SIGHT_LIMIT, Shot, Sighting, Trajectory};
use projectile::{fire, footprint};
pub use ram::Hull;
pub use ranging::{HEAVY_CALIBRE, HEAVY_DAMAGE, HEAVY_RANGE, HEAVY_RELOAD, Ranging};
pub use shipvivor_artillery::{
    AntiAir, ArmorGrade, Impact, MainShell, Round, Secondaries, TurretLayout,
};
use shipvivor_compass::heading as bearing;
pub use torpedo::{
    RACK_HALF_ARC, Rack, SPREAD_HALF_ARC, TORPEDO_RANGE, TORPEDO_SPEED, Torpedo, intercept,
};
pub use torpedo_boat::{
    AIM_ERROR, FIRING_RANGE, FIRST_RUN, HARRYING_RANGE, MOORED_PAIR, MOST_OUT, RELOAD, REPLACEMENT,
    RERUN, RUN_INTERVAL, TORPEDOES, WARHEAD,
};

#[derive(Clone, Copy, Debug)]
pub struct Body {
    pub id: u64,
    pub pos: Vec2,
    pub heading: f32,
}
impl Body {
    pub fn forward(&self) -> Vec2 {
        shipvivor_compass::direction(self.heading)
    }
}
/// Rate per second at which the water takes the way off a rammed raider.
const KNOCK_DRAG: f32 = 1.6;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Battery {
    Main,
    Secondary,
    EnemySecondary,
    Aircraft,
    Raider,
    Shore,
    /// Breakwater's light anti-aircraft guns. Their rounds burst on a time fuse.
    AntiAir,
    /// A heavy shore battery's long guns, which walk their salvos in on her.
    Heavy,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectKind {
    /// The flash of a gun, and which kind of gun fired.
    Muzzle(Battery),
    Wreck(EnemyClass),
    MountLost(EnemyClass),
    Demolition {
        dock: bool,
    },
    Armor {
        outcome: Impact,
        bore: u16,
        battery: Battery,
    },
    Burst,
    Splash,
    /// The tall column a heavy shell throws up where it falls in the sea.
    Column,
    /// Two hulls striking hard enough to do damage.
    Crunch,
    /// A time-fused anti-aircraft round bursting in the air.
    Flak,
    /// An enemy aircraft shot down, falling from where it was hit. It glides on along
    /// its heading as it drops, and throws up a splash where it meets the water.
    Downed(Airframe),
    /// An enemy torpedo bursting against Breakwater's side below the waterline, and the
    /// column of water it throws up over her.
    Torpedoed,
    /// Ice breaking apart, and how big it was: its waterline radius in whole metres.
    Shatter(u8),
    /// One of a capital ship's side strakes shot loose and thrown clear of her: her
    /// class, and the strake's index among hers.
    StrakeLost(EnemyClass, u8),
    /// An airfield's hangar or control tower coming down.
    Felled(FieldWork),
    /// Fuel going up on an airfield: a truck or an aircraft on the ground, or the whole
    /// depot, in a ball of fire that rolls up into a cloud of smoke.
    FuelBlast {
        depot: bool,
    },
    /// A sea stack's tier shot away, or a lighthouse toppling, where it came down.
    Toppled(FeatureKind),
}
impl EffectKind {
    /// Seconds before the effect is removed.
    pub fn lifetime(self) -> f32 {
        match self {
            // A heavy gun's flash hangs on in a cloud of its smoke.
            Self::Muzzle(Battery::Heavy) => 0.6,
            Self::Muzzle(_) => 0.18,
            Self::Wreck(class) if class.is_flagship() => 7.,
            Self::Wreck(_) => 3.2,
            Self::MountLost(_) => 1.2,
            Self::Demolition { dock: true } => 3.6,
            Self::Demolition { dock: false } => 2.2,
            Self::Armor { outcome, .. } => match outcome {
                Impact::Bounced => 0.62,
                Impact::Embedded => 0.9,
                Impact::Penetrated => 1.4,
                Impact::PassedThrough => 1.0,
                Impact::Overpenetrated => 0.8,
            },
            Self::Burst => 0.85,
            Self::Splash => 0.85,
            Self::Column => 2.6,
            Self::Crunch => 0.9,
            Self::Flak => 1.6,
            Self::Downed(_) => 3.4,
            Self::Torpedoed => 2.4,
            Self::Shatter(_) => 1.4,
            Self::StrakeLost(..) => 1.6,
            Self::Felled(_) => 3.,
            Self::FuelBlast { depot: false } => 6.,
            Self::FuelBlast { depot: true } => 9.,
            Self::Toppled(FeatureKind::Lighthouse) => 3.,
            Self::Toppled(_) => 2.2,
        }
    }
}
#[derive(Clone)]
pub struct Effect {
    pub body: Body,
    pub kind: EffectKind,
    pub age: f32,
    pub height: f32,
}
/// A damaging hit on the player's hull.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blow {
    /// Metres aft of the bow tip.
    pub aft: f32,
    /// Metres to starboard of the centreline. Negative is to port.
    pub abeam: f32,
    pub damage: u16,
    /// Inches across the round that struck, or `None` for a blow no plate can turn:
    /// a ram, an iceberg or a bomb through the deck.
    pub calibre: Option<f32>,
    /// How far in it got. A round reaches only the bays its path crosses below the
    /// deck. A ram, an iceberg or a bomb reaches every bay of the module it lands on.
    pub reach: shipvivor_shipyard::Reach,
    /// A torpedo's blow, which tears her open below the waterline so the sea pours in
    /// faster than any pump, until the crew shores the hole.
    pub breach: bool,
}
#[derive(Default, Debug)]
pub struct Events {
    /// Each hit on the player this tick. `damage` is their sum.
    pub blows: Vec<Blow>,
    pub hits: u16,
    pub sunk: u16,
    pub regent_sunk: bool,
    /// Enemy aircraft brought down this tick: shot down by the AA guns, or wrecked where
    /// they lay at their mooring by her guns or her stem.
    pub planes_downed: u16,
    /// Drydocks brought down this tick. A base's factories may raise one again.
    pub docks_felled: u16,
    /// Harbour guns silenced this tick. Each fights on apart from its yard until then.
    pub guns_silenced: u16,
    /// Stores brought aboard: packages towed alongside, or under direct rewards the
    /// stores of each ship sunk and each yard building wrecked.
    pub recovered: u16,
    /// Coasters among the ships sunk this tick.
    pub coasters: u16,
    /// Torpedo boats among the ships sunk this tick.
    pub torpedo_boats: u16,
    /// Enemy torpedoes that struck her this tick. Each is among the `blows`.
    pub torpedoed: u16,
    /// Cargo sent down this tick, in dock points: what hits knocked out of a cargo
    /// ship's holds, in proportion to the hull they cost her, and all she still carried
    /// when she sank.
    pub cargo: f32,
    pub damage: u16,
    /// Change in the player's position from hulls pushing apart.
    pub shove: Vec2,
    /// Change in the player's signed speed from ramming or being rammed.
    pub jolt: f32,
    /// Fuel trucks blown up on an airfield this tick.
    pub vehicles: u16,
}
/// Range ahead and offset to starboard of the patrol screen when a classic sector opens.
/// The destroyers lie abeam, where their shells land amidships on the engine room, so they
/// start well outside their own gun range and give the player time to come round. A
/// coastal passage sets its own screen along the course (see `Chart::pickets`).
const PICKETS: [(f32, f32); 4] = [(310., 38.), (390., -126.), (-153., -444.), (50., 467.)];
/// Each boat on patrol when a sector opens lies at her charted station or at one of
/// these others, in metres farther out from where the player enters and to starboard
/// across that line. Never nearer, so none starts inside her standoff.
const PICKET_SHIFTS: [(f32, f32); 3] = [(0., 0.), (60., 80.), (60., -80.)];
const COMBATANT_LIMIT: usize = 16;
/// The warships lying at anchor off a base at the mouth of the channel in the first
/// round of mixed waters, which is every base of a sector charted in a single stage.
/// Each base's squadron is its waters' [`Doctrine::squadron`]: launched from the base's
/// own docks before she came, and replaced by none but them. Two or more sail as one, in
/// close order on the heaviest and in line of battle once they have her in reach (see
/// `formation`).
const GARRISON: [EnemyClass; 1] = [EnemyClass::Patrol];
/// Metres off at the least that the Regent's screen lies when the ship enters Crown
/// Deep: past the reach of its guns, and inside its lookouts' sight.
const REGENT_RANGE: f32 = 800.;
/// The Regent's screen at Crown Deep, which meets the ship first while the Regent and her
/// battleships hold back over the horizon. It forms its line on its leading cruiser.
#[rustfmt::skip]
const REGENT_SCREEN: [EnemyClass; 9] = {
    use EnemyClass::*;
    [Cruiser, Cruiser, Cruiser, Cruiser, Destroyer, Destroyer, Destroyer, Destroyer, Destroyer]
};
/// The battle line that comes up the deep with the Regent and forms on her.
const REGENT_LINE: [EnemyClass; 3] = [EnemyClass::Battleship; 3];
/// Ships of her screen still afloat when the Regent comes up: two in three of it sunk.
const REGENT_SCREEN_LEFT: usize = 3;
/// Seconds into Crown Deep after which she comes up whatever is left of her screen.
const REGENT_PATIENCE: f32 = 240.;
/// Metres off the ship at which the Regent comes over the horizon, as far up the deep as
/// she finds room for her line: past the reach of every gun on either side, and well
/// into the haze.
const REGENT_SIGHTING: (f32, f32) = (1400., 2400.);
/// Hull points a second, with every round landing, of the main battery the Regent was
/// built to meet. A heavier battery finds her built up against it, though not quite in
/// step, so a ship that brings more guns still finishes her sooner (see
/// [`regent_hardening`]).
pub const REGENT_MATCH: f32 = 2.5;
/// The most main battery rounds a second a ship is reckoned to put out, however quick her
/// loaders: past it her barrels wait to bear, and a held trigger walks them one at a time,
/// the smallest gun that can hurt her first.
pub const MOST_ROUNDS_A_SECOND: f32 = 20.;
/// How closely her build follows the weight of fire she meets: 1 would make every fight
/// with her as long as the one against [`REGENT_MATCH`].
const REGENT_MATCH_EXPONENT: f32 = 0.85;

/// How many times as stout as designed the Regent comes up the deep, against a main
/// battery of `weight_of_fire` hull points a second: never less than designed.
pub fn regent_hardening(weight_of_fire: f32) -> f32 {
    (weight_of_fire / REGENT_MATCH)
        .max(1.)
        .powf(REGENT_MATCH_EXPONENT)
}
/// Seconds a base stays at alert after the base before it up the channel collapses: its
/// lookout and radio keep its squadron, its batteries and its torpedo boats on her.
pub const SCRAMBLE_ALERT: f32 = 60.;
/// Where a squadron may lie in its base's harbour off the slip, in metres out along the
/// slip and across it, best first. The roadstead closer in is the coasters'. A battle
/// fleet's heavy ships take the outer berths, which have room for their length.
const GARRISON_STATIONS: [(f32, f32); 25] = [
    (150., 70.),
    (150., -70.),
    (205., 25.),
    (205., -25.),
    (120., 105.),
    (120., -105.),
    (265., 75.),
    (265., -75.),
    (290., 0.),
    (180., 160.),
    (180., -160.),
    (340., 110.),
    (340., -110.),
    (370., 0.),
    (250., 190.),
    (250., -190.),
    (420., 90.),
    (420., -90.),
    (330., 220.),
    (330., -220.),
    (460., 0.),
    (500., 120.),
    (500., -120.),
    (420., 230.),
    (420., -230.),
];
/// Where a boss's escort may put out, in metres off her bow beyond her own length and
/// across it, best first: a screen ahead of her, widening as it fills.
const ESCORT_STATIONS: [(f32, f32); 16] = [
    (40., 45.),
    (40., -45.),
    (95., 0.),
    (100., 85.),
    (100., -85.),
    (150., 40.),
    (150., -40.),
    (60., 120.),
    (60., -120.),
    (195., 95.),
    (195., -95.),
    (220., 0.),
    (150., 160.),
    (150., -160.),
    (270., 50.),
    (270., -50.),
];
/// Raid intervals a slip stands empty after a launch. Three yards taking turns put a
/// ship to sea about once an interval.
const SLIP_TURNS: f32 = 3.;

/// Where picket `index` is on patrol: her charted `station`, or one of the others the
/// chart's seed picks for her, so a seed always meets the same screen. A hand-set chart
/// keeps its stations.
fn picket_station(seed: Option<u64>, index: usize, station: Vec2) -> Vec2 {
    let Some(seed) = seed else {
        return station;
    };
    // A splitmix step, so neighbouring seeds and pickets choose independently.
    let mut z = seed.wrapping_add((index as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    let (out, across) = PICKET_SHIFTS[(z % PICKET_SHIFTS.len() as u64) as usize];
    let ahead = station.normalize_or(Vec2::NEG_Y);
    station + ahead * out + ahead.perp() * across
}
/// Seconds into a sector when destroyers, then cruisers, then battleships join the raids.
pub const RAID_TIERS: [f32; 3] = [65., 160., 310.];
/// Seconds into a sector when the flagship puts to sea, between the cruisers and the
/// battleships. She is slow, so she comes into range near the end of the escalation.
pub const FLAGSHIP_SORTIE: f32 = 270.;
const _: () = assert!(FLAGSHIP_SORTIE > RAID_TIERS[1] && FLAGSHIP_SORTIE < RAID_TIERS[2]);
/// The soonest a flagship puts to sea, however far a sector's raids have escalated, so
/// the ship has a minute and a half in the water before she comes out.
pub const FLAGSHIP_EARLIEST: f32 = 90.;
/// Seconds before a boss is due that smoke goes up over her yard and the ship is warned
/// she is raising steam.
pub const STEAM_WARNING: f32 = 30.;

/// The flagship's part in a sector that has one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sortie {
    /// Fitting out. She puts to sea this many seconds into the sector, once a yard is
    /// clear for her.
    Due(f32),
    /// She put to sea from this port this many seconds into the sector.
    Sailed { port: usize, at: f32 },
    /// The Regent and her battle line, holding back over the horizon at Crown Deep while
    /// her screen comes on. She comes up once most of it is sunk.
    Holding,
    /// The Regent came over the horizon this many seconds into the sector.
    Sighted { at: f32 },
}

/// The Regent waiting over the horizon at Crown Deep.
#[derive(Clone, Debug)]
struct Holding {
    /// The screen she sent on ahead, by id.
    screen: Vec<u64>,
}
/// Seconds between barrels while the trigger is held, for guns that load slower than
/// this per barrel. `ripple` closes it up for guns that load faster.
pub const RIPPLE: f32 = 0.3;

/// Seconds between barrels for guns that load `rounds` a second between them: `RIPPLE`,
/// or closer for guns that load faster than that, so every barrel fires once each time
/// it loads and none waits loaded for its turn. A closer ripple is cut to whole steps,
/// rounding down, and half a step under, so the step never makes it longer.
fn ripple(rounds: f32, dt: f32) -> f32 {
    let even = rounds.recip();
    if even >= RIPPLE {
        return RIPPLE;
    }
    ((even / dt).floor().max(1.) - 0.5) * dt
}
/// Seconds a clicked order stands while its guns finish training or loading.
pub const VOLLEY_WINDOW: f32 = 0.4;

/// A click on the main battery trigger.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Volley {
    /// The next barrel in the firing order that is loaded and bears.
    Single,
    /// Every barrel that is loaded and bears.
    Salvo,
}

#[derive(Clone, Copy, Default)]
pub enum Aim {
    #[default]
    Hold,
    At(Vec2),
    Sight(Vec3),
    /// Follow the sight with the trigger released.
    Track(Vec3),
    Nearest,
}
pub struct Orders {
    pub player: Vec2,
    pub heading: f32,
    /// Signed speed along the heading in metres per second.
    pub speed: f32,
    pub hull: Hull,
    /// Where the main battery trains. `At`, `Sight` and `Nearest` also hold the trigger.
    pub aim: Aim,
    /// Seconds for one main battery barrel to reload.
    pub reload: f32,
    pub shell: MainShell,
}
#[derive(Clone)]
pub struct Battle {
    ports: Vec<Port>,
    air_station: Option<AirStation>,
    harriers: Vec<Harrier>,
    /// The harriers lying at each base's mooring, and the berths left empty there.
    moorings: harrier::Moorings,
    /// Seconds into the sector before each base may send up its next pair of harriers.
    harrier_due: Vec<f32>,
    /// Seconds into the sector until which each base stays at alert, scrambled when the
    /// base before it collapsed.
    alert_until: Vec<f32>,
    /// The round of the run whose squadrons lie in the harbours, from one.
    squadron_round: usize,
    /// How many times over the bases moor their squadrons and launch their ships: 2 on
    /// a punishing passage (see [`Battle::with_strength`]).
    strength: usize,
    /// What holds these waters: what the harbours moor, the yards build and the boss
    /// sails with, and how heavily its warships are gunned.
    doctrine: Doctrine,
    /// The fleet the scouts missed, if one lies here.
    surprise: Option<Surprise>,
    /// The ships that sail as one, each led by her heaviest.
    squadrons: Vec<formation::Squadron>,
    /// How hard every enemy blow lands on her, as a share of its own weight, and the
    /// fraction of a hull point carried over to the next (see [`Battle::with_pressure`]).
    pressure: f32,
    pressure_carry: f32,
    /// The weights the sector's warships are gunned in, by its round. Without one, each
    /// ship carries her class's stock guns.
    muster: Option<weight::Muster>,
    raiders: Vec<Raider>,
    /// Ships going down: each one's wreck effect, by its id, and the ship as she was
    /// when she sank, every hole and burnt-out turret on her still there.
    sunk: Vec<(u64, Raider)>,
    spotters: Vec<Spotter>,
    patrol_center: Vec2,
    /// The start and the marks of a passage in order, the road its ships follow when
    /// land stands between them and where they are bound. Empty on a circuit.
    fairway: Vec<Vec2>,
    shots: Vec<Shot>,
    torpedoes: Vec<Torpedo>,
    racks: [f32; 2],
    floatplanes: Vec<Floatplane>,
    bombs: Vec<Bomb>,
    launchers: Vec<Launcher>,
    /// How the strike craft are fitted out: launchers and salvos, planes and bombs.
    strike: shipvivor_shipyard::StrikeCraft,
    missiles: Vec<Missile>,
    salvage: Vec<Package>,
    rewards: Rewards,
    landmarks: Vec<Landmark>,
    effects: Vec<Effect>,
    main_mounts: Vec<MainMount>,
    gun_train_rate: f32,
    deck: DeckOffsets,
    /// A clicked order and how long it still stands.
    volley: Option<(Volley, f32)>,
    /// Seconds since a main battery barrel last fired.
    since_main: f32,
    /// The mount offered the next shot. The two take turns.
    next_mount: usize,
    /// The ship the captain's sight is on, whose plate decides which gun the trigger
    /// offers first.
    marked: Option<u64>,
    /// The captain is looking through the scope, so turrets on scope control are hers
    /// wherever they bear on the sight.
    scope_up: bool,
    wing_guns: Vec<WingGun>,
    /// Where her secondaries stand on the hull she has now, in the order they are fitted.
    wing_stations: Vec<shipvivor_shipyard::WingStation>,
    /// Where her AA mounts stand on the hull she has now, in the order they are fitted.
    aa_stations: Vec<shipvivor_shipyard::AaStation>,
    secondaries: Secondaries,
    aa_mounts: Vec<AaMount>,
    anti_air: AntiAir,
    /// Ranks of training gear on every main gunhouse.
    training_gear: u16,
    last_strike: Option<(Impact, f32)>,
    /// The player's heading and hull this tick, for placing hits along her length.
    keel: (Vec2, Hull),
    next_wave: f32,
    wave_interval: f32,
    serial: u64,
    /// Seconds since the ship entered the sector. Raids grow heavier with it.
    at_sea: f32,
    /// Seconds the raids begin ahead of [`RAID_TIERS`], in waters deeper into a run.
    escalation: f32,
    next_dock: usize,
    flagship: Option<Sortie>,
    /// The class that sails as this sector's boss.
    boss: EnemyClass,
    /// The Regent holding back at Crown Deep until her screen is broken.
    regent: Option<Holding>,
    /// The current, and the ice it carries.
    floe: iceberg::Floe,
    /// Each base's torpedo boats: where they moor and when it sends them out. Empty in
    /// a sector that has none.
    flotillas: Vec<torpedo_boat::Flotilla>,
    /// The airfields' aircraft in the air.
    warplanes: Vec<Warplane>,
}
fn body(serial: &mut u64, pos: Vec2, heading: f32) -> Body {
    *serial += 1;
    Body {
        id: *serial,
        pos,
        heading,
    }
}
impl Battle {
    /// The classic harbours and islands around a buoy circuit.
    pub fn for_route(wave_interval: f32, gates: [Vec2; 5]) -> Self {
        Self::for_chart(wave_interval, &Chart::classic(gates))
    }
    /// The sector a chart sets out, its current running and its ice adrift.
    pub fn for_chart(wave_interval: f32, chart: &Chart) -> Self {
        let mut b = Self::patrolled(wave_interval, chart)
            .with_heavy_battery(chart)
            .with_sea(chart)
            .with_coasters()
            .with_torpedo_boats();
        b.fill_moorings();
        b
    }
    /// The harbours, islands and patrols of a chart, in still and open water. The survey
    /// checks hundreds of these and needs neither the current nor the ice.
    pub(crate) fn patrolled(wave_interval: f32, chart: &Chart) -> Self {
        let mut b = Self::chart_sector(wave_interval, chart);
        // These boats are already on patrol when the player enters the sector.
        for (index, (station, class)) in chart
            .pickets
            .into_iter()
            .zip([
                EnemyClass::Patrol,
                EnemyClass::Patrol,
                EnemyClass::Destroyer,
                EnemyClass::Destroyer,
            ])
            .enumerate()
        {
            let station = picket_station(chart.seed, index, station);
            let at = b.navigable_position(station, class.profile().hull.bow);
            b.spawn(class, at, Vec2::ZERO);
        }
        b.garrison();
        b
    }

    fn chart_sector(wave_interval: f32, chart: &Chart) -> Self {
        let mut serial = 0;
        let ports = Port::chart(&chart.harbours, wave_interval, &mut serial);
        let ahead = chart.gates[0].normalize_or(Vec2::NEG_Y);
        let landmarks = chart
            .islets
            .iter()
            .chain(&chart.coast)
            .map(|islet| {
                let at = body(&mut serial, islet.pos, islet.heading);
                Landmark::new(at, islet.radius, islet.kind)
            })
            .collect();
        let mut b = Self {
            air_station: None,
            harriers: Vec::new(),
            moorings: harrier::Moorings::default(),
            harrier_due: vec![0.; ports.len()],
            alert_until: vec![f32::NEG_INFINITY; ports.len()],
            squadron_round: 1,
            strength: 1,
            doctrine: Doctrine::Mixed,
            surprise: None,
            squadrons: Vec::new(),
            pressure: 1.,
            pressure_carry: 0.,
            muster: None,
            spotters: ports
                .iter()
                .enumerate()
                .map(|(i, port)| Spotter::patrol(port, i))
                .collect(),
            patrol_center: chart.gates.iter().sum::<Vec2>() / chart.gates.len() as f32,
            fairway: match chart.course {
                Course::Circuit => vec![],
                Course::Passage => std::iter::once(Vec2::ZERO)
                    .chain(chart.gates.iter().copied())
                    .collect(),
            },
            landmarks,
            ports,
            raiders: vec![],
            sunk: vec![],
            shots: vec![],
            torpedoes: vec![],
            racks: [0.; 2],
            floatplanes: vec![],
            bombs: vec![],
            launchers: vec![],
            strike: shipvivor_shipyard::StrikeCraft::STOCK,
            missiles: vec![],
            salvage: vec![],
            rewards: Rewards::Packages,
            effects: vec![],
            deck: DeckOffsets::for_design(&shipvivor_shipyard::Design::breakwater()),
            gun_train_rate: 1.,
            main_mounts: shipvivor_shipyard::Design::breakwater()
                .gun_stations()
                .into_iter()
                .map(MainMount::new)
                .collect(),
            volley: None,
            since_main: RIPPLE,
            next_mount: 0,
            marked: None,
            scope_up: false,
            wing_guns: mounts::wing_berths(
                &shipvivor_shipyard::Design::breakwater().wing_stations(),
                2 * Secondaries::STOCK.per_side,
                Secondaries::STOCK,
                &shipvivor_shipyard::Design::breakwater()
                    .gun_stations()
                    .into_iter()
                    .map(MainMount::new)
                    .collect::<Vec<_>>(),
            )
            .into_iter()
            .map(WingGun::new)
            .collect(),
            wing_stations: shipvivor_shipyard::Design::breakwater().wing_stations(),
            secondaries: Secondaries::STOCK,
            aa_mounts: shipvivor_shipyard::Design::breakwater()
                .aa_stations()
                .into_iter()
                .take(AntiAir::STOCK.mounts)
                .map(AaMount::new)
                .collect(),
            aa_stations: shipvivor_shipyard::Design::breakwater().aa_stations(),
            anti_air: AntiAir::STOCK,
            training_gear: 0,
            last_strike: None,
            keel: (Vec2::NEG_Y, Hull::CRUISER),
            next_wave: wave_interval,
            wave_interval,
            serial,
            at_sea: 0.,
            escalation: 0.,
            next_dock: 0,
            flagship: None,
            boss: EnemyClass::Parliament,
            regent: None,
            floe: iceberg::Floe::slack(),
            flotillas: Vec::new(),
            warplanes: Vec::new(),
        };
        let works: Vec<u8> = chart.harbours.iter().map(|site| site.works).collect();
        b.site_airfields(&works);
        for range in [28., 145., 265.] {
            let pos = b.navigable_position(ahead * range, 3.);
            b.salvage
                .push(Package::new(body(&mut b.serial, pos, 0.), 1));
        }
        for port in &b.ports {
            let f = port.body.forward();
            for offset in [-12., 12.] {
                b.salvage.push(Package::new(
                    body(
                        &mut b.serial,
                        port.berth() + f * 25. + f.perp() * offset,
                        0.,
                    ),
                    1,
                ));
            }
        }
        b
    }
    /// Pay stores the moment their ship or building falls, or leave packages to tow in.
    /// A sector is charted with its loose packages, and direct rewards clear them.
    pub fn with_rewards(mut self, rewards: Rewards) -> Self {
        self.rewards = rewards;
        if rewards == Rewards::Direct {
            self.salvage.clear();
        }
        self
    }
    pub fn rewards(&self) -> Rewards {
        self.rewards
    }
    /// The flagship is fitting out in one of the yards and puts to sea partway through.
    pub fn with_flagship(self) -> Self {
        self.with_boss(EnemyClass::Parliament)
    }
    /// A boss of this class is fitting out in one of the yards and puts to sea partway
    /// through, as a flagship does.
    pub fn with_boss(mut self, class: EnemyClass) -> Self {
        self.boss = class;
        self.flagship = Some(Sortie::Due(self.flagship_due()));
        self
    }
    /// The class of this sector's boss: the one fitting out or sailed, or the Regent
    /// already at sea. `None` for a sector with no boss.
    pub fn boss(&self) -> Option<EnemyClass> {
        self.flagship.map(|_| self.boss).or_else(|| {
            self.raiders
                .iter()
                .find(|r| r.class.is_boss())
                .map(|r| r.class)
        })
    }
    /// The boss afloat in the sector, once she has put to sea.
    pub fn boss_ship(&self) -> Option<&Raider> {
        self.raiders.iter().find(|r| r.class.is_boss())
    }
    /// Seconds until a boss still fitting out is due to sail, once smoke is up over her
    /// yard: within [`STEAM_WARNING`] of it, or past it while she waits for a clear slip.
    pub fn raising_steam(&self) -> Option<f32> {
        match self.flagship {
            Some(Sortie::Due(due)) if self.at_sea >= due - STEAM_WARNING => {
                Some((due - self.at_sea).max(0.))
            }
            _ => None,
        }
    }
    /// Raids that start `head_start` seconds along their escalation, for waters deeper
    /// into a run: destroyers, cruisers and battleships join sooner, and a flagship
    /// fitting out puts to sea that much earlier, though never in the first
    /// [`FLAGSHIP_EARLIEST`] seconds.
    pub fn escalated(mut self, head_start: f32) -> Self {
        self.escalation = head_start.max(0.);
        let due_at = self.flagship_due();
        if let Some(Sortie::Due(due)) = &mut self.flagship {
            *due = due_at;
        }
        self
    }
    /// How far ahead of the sector's clock its raids began.
    /// Every blow the enemy lands on her hurts `pressure` times as much: shells, rams,
    /// bombs and torpedoes alike. It is the one dial that turns the whole leg harder or
    /// easier without changing what she meets. Fractions of a hull point carry from blow
    /// to blow, so light hits scale as truly as heavy ones.
    pub fn with_pressure(mut self, pressure: f32) -> Self {
        self.pressure = pressure.max(0.);
        self
    }
    pub fn pressure(&self) -> f32 {
        self.pressure
    }
    pub fn escalation(&self) -> f32 {
        self.escalation
    }
    /// Shore guns of no more than `most` barrels, so the bases arm up round by round:
    /// singles in the first, twins in the second, and so on.
    pub fn turrets_capped(mut self, most: u8) -> Self {
        for port in &mut self.ports {
            port.cap_turrets(most);
        }
        self
    }
    /// Warships gunned from the roster of round `round` of a run, counting from one:
    /// light destroyers, cruisers and battleships early, heavy and ultra heavy ones late.
    /// The ships already on station or at anchor take their weights now, in the order
    /// they were charted, and each launched after takes the next.
    pub fn ships_of_round(mut self, round: usize) -> Self {
        let mut muster = weight::Muster::of_round(round);
        for raider in &mut self.raiders {
            if !raider.class.is_boss() {
                raider.reweigh(muster.draw(raider.class));
            }
        }
        self.muster = Some(muster);
        self
    }
    fn flagship_due(&self) -> f32 {
        (FLAGSHIP_SORTIE - self.escalation).max(FLAGSHIP_EARLIEST)
    }
    /// Crown Deep. The Regent's screen of cruisers and destroyers is at sea, bearing down
    /// on her from beyond the reach of its guns but in sight of her, so she sees its line
    /// form before the first salvo. The Regent and her battleships hold back over the
    /// horizon until it is broken (see [`Sortie::Holding`]).
    pub fn regent_encounter(wave_interval: f32, chart: &Chart) -> Self {
        let mut battle = Self::chart_sector(wave_interval, chart);
        let approach = chart.gates[0].normalize_or(Vec2::NEG_Y);
        let (at, axis) =
            battle.fleet_station(&REGENT_SCREEN, Vec2::ZERO, &[], approach, REGENT_RANGE);
        let screen = battle.put_fleet_to_sea(&REGENT_SCREEN, at, axis);
        // Her scouts have reported the ship coming in: the screen comes on to meet her.
        for ship in battle
            .raiders
            .iter_mut()
            .filter(|r| screen.contains(&r.body.id))
        {
            ship.last_sighting = Some(Vec2::ZERO);
        }
        battle.boss = EnemyClass::Regent;
        battle.flagship = Some(Sortie::Holding);
        battle.regent = Some(Holding { screen });
        battle.with_heavy_battery(chart).with_sea(chart)
    }
    /// Where a fleet of `classes` in close order on the first of them lies off `centre`,
    /// facing it, and the way from `centre` out to it: the first of `preferred` with
    /// clear water for every hull of it, or else the nearest clear water `range` metres
    /// or more off along one of twelve bearings, those nearest `ahead` first. Every ship
    /// of it lies at least [`SPAWN_STANDOFF`] off `centre`. Found even on a chart whose
    /// first gate points straight at a headland.
    fn fleet_station(
        &self,
        classes: &[EnemyClass],
        centre: Vec2,
        preferred: &[Vec2],
        ahead: Vec2,
        mut range: f32,
    ) -> (Vec2, Vec2) {
        let offsets = fleet_offsets(classes);
        let fits = |at: Vec2, axis: Vec2| {
            classes.iter().zip(&offsets).all(|(class, &offset)| {
                let spot = close_order_spot(at, axis, offset);
                let room = class.profile().hull.bow;
                spot.distance(centre) >= SPAWN_STANDOFF
                    && self.navigable_position(spot, room).distance_squared(spot) < 0.01
            })
        };
        for &at in preferred {
            let axis = (at - centre).normalize_or(ahead);
            if fits(at, axis) {
                return (at, axis);
            }
        }
        loop {
            for sector in [0, 1, -1, 2, -2, 3, -3, 4, -4, 5, -5, 6] {
                let axis =
                    Vec2::from_angle(sector as f32 * std::f32::consts::TAU / 12.).rotate(ahead);
                let at = centre + axis * range;
                if fits(at, axis) {
                    return (at, axis);
                }
            }
            range += classes[0].profile().hull.bow;
        }
    }
    /// A fleet of `classes` put to sea in close order on the first of them at `at`,
    /// facing back along `axis`, and formed into one squadron. Their ids, the guide's
    /// first.
    fn put_fleet_to_sea(&mut self, classes: &[EnemyClass], at: Vec2, axis: Vec2) -> Vec<u64> {
        let mut fleet = Vec::new();
        for (&class, offset) in classes.iter().zip(fleet_offsets(classes)) {
            let spot = close_order_spot(at, axis, offset);
            self.spawn(class, spot, spot - axis);
            fleet.push(self.serial);
        }
        self.form_squadron(fleet.clone());
        fleet
    }
    /// At Crown Deep the Regent holds back over the horizon until all but
    /// [`REGENT_SCREEN_LEFT`] of her screen are sunk, or she has waited
    /// [`REGENT_PATIENCE`]. Then she comes up the deep with her battle line, beyond the
    /// reach of every gun, built up against the weight of fire the ship carries.
    fn bring_up_regent(&mut self, orders: &Orders) {
        let Some(holding) = &self.regent else {
            return;
        };
        let afloat = |id: &u64| self.raiders.iter().any(|r| r.body.id == *id && r.afloat());
        let left = holding.screen.iter().filter(|id| afloat(id)).count();
        if left > REGENT_SCREEN_LEFT && self.at_sea < REGENT_PATIENCE {
            return;
        }
        let player = orders.player;
        let classes: Vec<EnemyClass> = std::iter::once(EnemyClass::Regent)
            .chain(REGENT_LINE)
            .collect();
        let (nearest, farthest) = REGENT_SIGHTING;
        // As far up the deep as she has room, inside her sighting range.
        let mut preferred = Vec::new();
        let length: f32 = self.fairway.windows(2).map(|w| w[0].distance(w[1])).sum();
        let here = fairway_run(&self.fairway, player);
        let mut run = length;
        while run > here {
            let at = fairway_point(&self.fairway, run);
            if (nearest..=farthest).contains(&at.distance(player)) {
                preferred.push(at);
            }
            run -= 40.;
        }
        let ahead = match self.fairway.last() {
            Some(&head) if self.fairway.len() >= 2 => head - player,
            _ => self.patrol_center - player,
        }
        .normalize_or(Vec2::NEG_Y);
        let (at, axis) = self.fleet_station(&classes, player, &preferred, ahead, nearest);
        let factor = regent_hardening(self.weight_of_fire(orders));
        let fleet = self.put_fleet_to_sea(&classes, at, axis);
        for ship in self
            .raiders
            .iter_mut()
            .filter(|r| fleet.contains(&r.body.id))
        {
            // Her screen has reported the ship: she comes on knowing where to find her.
            ship.last_sighting = Some(player);
            if ship.class == EnemyClass::Regent {
                ship.harden(factor);
            }
        }
        self.flagship = Some(Sortie::Sighted { at: self.at_sea });
        self.regent = None;
    }
    /// While the Regent holds back at Crown Deep, how many more of her screen must be
    /// sunk before she comes up.
    pub fn regent_screen(&self) -> Option<usize> {
        let holding = self.regent.as_ref()?;
        let afloat = (holding.screen.iter())
            .filter(|&&id| self.raiders.iter().any(|r| r.body.id == id && r.afloat()))
            .count();
        Some(afloat.saturating_sub(REGENT_SCREEN_LEFT))
    }
    /// How many times as stout as her class was designed `ship` came into the fight: more
    /// than one for a Regent built up against the battery she met.
    pub fn built_up(ship: &Raider) -> f32 {
        ship.vessel().design().structure() / ship.class.design().structure()
    }
    /// Hull points a second her main battery would put into a ship with every round
    /// landing: each barrel's breach force over its reload, every barrel aboard, but no
    /// more than [`MOST_ROUNDS_A_SECOND`] rounds a second between them.
    pub fn weight_of_fire(&self, orders: &Orders) -> f32 {
        let (rounds, weight) = (self.main_mounts.iter()).fold((0., 0.), |(rounds, weight), m| {
            let rate = m.layout().count() as f32 / m.reload(orders);
            let force = f32::from(m.round(orders.shell).breach_force());
            (rounds + rate, weight + rate * force)
        });
        weight * (MOST_ROUNDS_A_SECOND / rounds).min(1.)
    }

    /// Warships under way. A garrison at anchor off a sleeping base is not yet one.
    fn combatants(&self) -> usize {
        self.raiders
            .iter()
            .filter(|r| !r.class.is_merchant() && r.moored.is_none())
            .count()
    }
    /// A base's garrison weighs once the base is roused, or once she is in sight. Its
    /// torpedo boats wait for their base to send them.
    fn rouse_garrisons(&mut self) {
        for r in &mut self.raiders {
            if let Some(home) = r.moored
                && !r.class.is_merchant()
                && r.attack.is_none()
                && (self.ports[home].roused || r.has_contact)
            {
                r.cast_off();
            }
        }
    }
    /// The squadron base `base` keeps in its harbour: heavier the further up the channel
    /// it stands and the deeper into the run. The heavy yard's is the heaviest: the base
    /// of the last stage with the most works. A base paired with it keeps one from
    /// between.
    pub fn squadron(&self, base: usize) -> &'static [EnemyClass] {
        let place = self.place(base);
        let doctrine = if self.surprised(base) {
            self.surprise.map_or(self.doctrine, |s| s.doctrine)
        } else {
            self.doctrine
        };
        doctrine.squadron(self.squadron_round, place)
    }
    /// Where base `base` stands up the channel: 0 at its mouth, 2 the heavy yard (the
    /// base of the last stage with the most works), 1 between.
    fn place(&self, base: usize) -> usize {
        let last = self.ports.iter().map(Port::stage).max().unwrap_or(0);
        let works = |port: &Port| port.factories().len();
        let heavy = (self.ports.iter())
            .filter(|p| p.stage() == last)
            .map(works)
            .max()
            .unwrap_or(0);
        match self.ports[base].stage() {
            0 => 0,
            stage if stage == last && works(&self.ports[base]) == heavy => 2,
            _ => 1,
        }
    }
    /// The squadrons of round `round` of a run, counting from one, in place of the first
    /// round's.
    pub fn with_squadrons(mut self, round: usize) -> Self {
        self.squadron_round = round.clamp(1, RUN_ROUNDS);
        // Weigh the squadrons charted with the sector and moor these in their place. The
        // torpedo boats and coasters keep their moorings.
        self.raiders
            .retain(|r| r.moored.is_none() || r.class.is_merchant() || r.attack.is_some());
        self.squadrons.clear();
        self.garrison();
        self
    }
    /// Bases that hold `times` the ships: each moors its squadron that many times over,
    /// as one, and its yards work that much quicker, steel and slips alike, so they
    /// launch that many times as often. Up to that many times as many warships are
    /// under way at once. The boss, its escort, the torpedo boats, the pickets and the
    /// air stay as charted. Comes after [`Battle::with_squadrons`] and before the boss.
    pub fn with_strength(mut self, times: usize) -> Self {
        let times = times.max(1);
        if times == self.strength {
            return self;
        }
        let quicker = times as f32 / self.strength as f32;
        self.strength = times;
        self.wave_interval /= quicker;
        for port in &mut self.ports {
            port.quicken(times as f32);
        }
        let round = self.squadron_round;
        self.with_squadrons(round)
    }
    /// How many times over the bases moor and launch their ships.
    pub fn strength(&self) -> usize {
        self.strength
    }
    /// Waters held by `doctrine`: the yards lay down its ships and each base moors its
    /// torpedo boats. Its squadrons and the boss's escort follow from
    /// [`Battle::with_squadrons`], which comes after, and from the round it sets. How
    /// quickly its yards launch is the sector's to set, in the interval it charts.
    pub fn with_doctrine(mut self, doctrine: Doctrine) -> Self {
        let boats = self.doctrine.torpedo_boats();
        self.doctrine = doctrine;
        for port in &mut self.ports {
            port.doctrine = doctrine;
        }
        if doctrine.torpedo_boats() != boats && !self.flotillas.is_empty() {
            self.raiders.retain(|r| r.attack.is_none());
            self.flotillas.clear();
            self = self.with_torpedo_boats();
        }
        self
    }
    pub fn doctrine(&self) -> Doctrine {
        self.doctrine
    }
    /// Hide `surprise` in these waters, ahead of [`Battle::with_squadrons`]. A sector with
    /// no base between the mouth and the heavy yard hides it at the heavy yard.
    pub fn with_surprise(mut self, mut surprise: Surprise) -> Self {
        if !(0..self.ports.len()).any(|base| self.place(base) == surprise.place) {
            surprise.place = 2;
        }
        self.surprise = Some(surprise);
        self
    }
    /// The fleet the scouts missed, if one lies here.
    pub fn surprise(&self) -> Option<Surprise> {
        self.surprise
    }
    /// Base `base` moors the surprise's squadron.
    pub fn surprised(&self, base: usize) -> bool {
        self.surprise.is_some_and(|s| self.place(base) == s.place)
    }
    /// Each squadron afloat: its ships' ids, and whether it is in line of battle.
    pub fn squadrons(&self) -> impl Iterator<Item = (&[u64], bool)> {
        (self.squadrons.iter()).map(|squadron| (squadron.ships(), squadron.deployed()))
    }
    /// Whether base `index` is at alert: scrambled when the base before it collapsed.
    pub fn at_alert(&self, index: usize) -> bool {
        self.at_sea < self.alert_until[index]
    }
    /// Moor each base's [`Battle::squadron`] in its harbour: warships at anchor clear of
    /// its slip and of her entry, which sail with the base when it is roused, as one.
    fn garrison(&mut self) {
        for base in 0..self.ports.len() {
            let (berth, forward) = (self.ports[base].berth(), self.ports[base].body.forward());
            let mut moored = Vec::new();
            let squadron = self.squadron(base);
            for &class in squadron.iter().cycle().take(squadron.len() * self.strength) {
                let hull = class.profile().hull;
                // A berth a heavy ship had no room for may suit a lighter one.
                let spot = GARRISON_STATIONS
                    .into_iter()
                    .map(|(out, across)| berth + forward * out + forward.perp() * across)
                    .find(|&spot| {
                        spot.length() >= SPAWN_STANDOFF
                            && self
                                .navigable_position(spot, hull.radius + 6.)
                                .distance(spot)
                                < 0.01
                            && self
                                .floe
                                .icebergs
                                .iter()
                                .all(|b| b.body.pos.distance(spot) > b.radius + hull.bow + 4.)
                            && self.raiders.iter().all(|r| {
                                r.body.pos.distance(spot)
                                    > r.class.profile().hull.bow + hull.bow + 4.
                            })
                    });
                if let Some(spot) = spot {
                    self.spawn(class, spot, spot + forward);
                    let ship = self.raiders.last_mut().unwrap();
                    ship.moor(base);
                    moored.push(ship.body.id);
                }
            }
            self.form_squadron(moored);
        }
    }
    pub fn flagship(&self) -> Option<Sortie> {
        self.flagship
    }
    /// Seconds since the ship entered the sector.
    pub fn at_sea(&self) -> f32 {
        self.at_sea
    }
    /// Where ships with no word of the ship patrol: off the nearest roused yard still
    /// standing on a passage, so the patrols hold the front as it moves up the channel,
    /// or the middle of a circuit.
    fn patrol_ground(&self) -> Vec2 {
        if self.fairway.len() < 2 {
            return self.patrol_center;
        }
        self.ports
            .iter()
            .filter(|p| p.roused && !p.wrecked())
            .map(|p| fairway_run(&self.fairway, p.body.pos))
            .min_by(f32::total_cmp)
            .map_or(self.patrol_center, |run| fairway_point(&self.fairway, run))
    }
    /// Where the boss would be met leaving this port, her stern clear of the quay.
    fn sortie_station(&self, port: &Port) -> Vec2 {
        let hull = self.boss.profile().hull;
        self.navigable_position(port.berth() + port.body.forward() * hull.stern, hull.radius)
    }
    /// Once she is due, the flagship leaves the roused yard farthest from the player
    /// whose approaches are clear of other ships.
    fn sortie(&mut self, player: Vec2) {
        let Some(Sortie::Due(due)) = self.flagship else {
            return;
        };
        if self.at_sea < due {
            return;
        }
        let class = self.boss;
        let bow = class.profile().hull.bow;
        let clear = |at: Vec2| {
            at.distance(player) >= SPAWN_STANDOFF
                && self.raiders.iter().all(|r| {
                    r.moored.is_some()
                        || r.body.pos.distance(at) > r.class.profile().hull.bow + bow + 2.
                })
                && self
                    .icebergs()
                    .iter()
                    .all(|b| b.body.pos.distance(at) > b.radius + bow + 4.)
        };
        let station = self
            .ports
            .iter()
            .enumerate()
            .filter(|(_, port)| port.roused && !port.wrecked())
            .map(|(index, port)| (index, self.sortie_station(port), port.body.forward()))
            .filter(|&(_, at, _)| clear(at))
            .max_by(|a, b| {
                a.1.distance_squared(player)
                    .total_cmp(&b.1.distance_squared(player))
            });
        if let Some((port, at, out)) = station {
            self.spawn(class, at, at + out);
            self.flagship = Some(Sortie::Sailed {
                port,
                at: self.at_sea,
            });
            self.sail_escort(at, out, player);
        }
    }
    /// The boss's escort puts to sea with her, from her own yard, onto the open water
    /// off her bow, and keeps station on her as one squadron. A ship with no clear water
    /// to put out onto stays in the yard.
    fn sail_escort(&mut self, at: Vec2, out: Vec2, player: Vec2) {
        let boss = self.serial;
        let mut squadron = vec![boss];
        let room = self.boss.profile().hull.bow;
        for &class in self.doctrine.escort(self.squadron_round) {
            let hull = class.profile().hull;
            let spot = ESCORT_STATIONS
                .into_iter()
                .map(|(ahead, across)| at + out * (room + ahead) + out.perp() * across)
                .find(|&spot| {
                    spot.distance(player) >= SPAWN_STANDOFF
                        && (self.navigable_position(spot, hull.radius + 6.)).distance(spot) < 0.01
                        && (self.floe.icebergs.iter())
                            .all(|b| b.body.pos.distance(spot) > b.radius + hull.bow + 4.)
                        && self.raiders.iter().all(|r| {
                            r.body.pos.distance(spot) > r.class.profile().hull.bow + hull.bow + 4.
                        })
                });
            if let Some(spot) = spot {
                self.spawn(class, spot, spot + out);
                let ship = self.raiders.last_mut().unwrap();
                ship.has_contact = true;
                squadron.push(ship.body.id);
            }
        }
        self.form_squadron(squadron);
    }
    /// The escort the boss sails with in these waters, in the order it puts to sea.
    pub fn escort(&self) -> &'static [EnemyClass] {
        if self.flagship.is_some() {
            self.doctrine.escort(self.squadron_round)
        } else {
            &[]
        }
    }

    pub fn ports(&self) -> &[Port] {
        &self.ports
    }
    /// Bases wrecked: every drydock and every factory there down, so nothing is left
    /// to raise a dock again. Their guns may still stand.
    pub fn ruined_yards(&self) -> usize {
        self.ports.iter().filter(|port| port.wrecked()).count()
    }
    pub fn raiders(&self) -> &[Raider] {
        &self.raiders
    }
    /// The ship going down as the wreck effect with id `wreck`, as she was when she
    /// sank.
    pub fn sinking(&self, wreck: u64) -> Option<&Raider> {
        self.sunk
            .iter()
            .find_map(|(id, ship)| (*id == wreck).then_some(ship))
    }
    pub fn nearest_ship(&self, player: Vec2) -> Option<&Raider> {
        self.raiders.iter().min_by(|a, b| {
            a.body
                .pos
                .distance_squared(player)
                .total_cmp(&b.body.pos.distance_squared(player))
        })
    }
    pub fn ship(&self, id: u64) -> Option<&Raider> {
        self.raiders.iter().find(|r| r.body.id == id)
    }
    pub fn shots(&self) -> &[Shot] {
        &self.shots
    }
    pub fn landmarks(&self) -> &[Landmark] {
        &self.landmarks
    }
    /// Where a ship at `from` bound for `goal` heads, as the raiders pilot: straight for
    /// it, or along the fairway round the shore that hides it.
    pub fn pilot(&self, from: Vec2, goal: Vec2) -> Vec2 {
        pilotage(&self.fairway, &self.landmarks, from, goal)
    }
    /// Whether the water between two points is clear of the land and the harbours.
    pub fn open_water(&self, from: Vec2, to: Vec2) -> bool {
        coast::open_water(from, to, &self.ports, &self.landmarks)
    }
    pub fn salvage(&self) -> &[Package] {
        &self.salvage
    }
    pub fn salvo_report(&self) -> Option<Impact> {
        self.last_strike.map(|(outcome, _)| outcome)
    }
    pub fn effects(&self) -> &[Effect] {
        &self.effects
    }
    pub fn wing_guns(&self) -> &[WingGun] {
        &self.wing_guns
    }
    pub fn secondaries(&self) -> Secondaries {
        self.secondaries
    }
    /// Mount, arm and drill the secondaries as fitted. Guns already aboard keep their state.
    /// Guns past the room her deck has wait ashore until a longer hull makes room. Each
    /// trains only as far as its fire clears the guns beside it and the main turrets.
    pub fn fit_secondaries(&mut self, fit: Secondaries) {
        let stations = mounts::wing_berths(
            &self.wing_stations,
            2 * fit.per_side,
            fit,
            &self.main_mounts,
        );
        self.wing_guns.truncate(stations.len());
        for (gun, &station) in self.wing_guns.iter_mut().zip(&stations) {
            gun.place(station);
        }
        let aboard = self.wing_guns.len();
        self.wing_guns
            .extend(stations[aboard..].iter().copied().map(WingGun::new));
        self.secondaries = fit;
    }
    pub fn main_mount(&self, station: Station) -> &MainMount {
        &self.main_mounts[station.index()]
    }
    pub fn main_mounts(&self) -> &[MainMount] {
        &self.main_mounts
    }
    /// Installing new barrels at sea leaves existing reloads and train intact.
    pub fn fit_design(
        &mut self,
        design: &shipvivor_shipyard::Design,
        fitting: impl Fn(&shipvivor_shipyard::GunStation) -> (MainShell, TurretLayout),
    ) {
        self.deck = DeckOffsets::for_design(design);
        self.aa_stations = design.aa_stations();
        self.fit_anti_air(self.anti_air);
        let mut positions = design.gun_stations();
        positions.truncate(Station::SLOTS.len());
        self.main_mounts.truncate(positions.len());
        for (i, position) in positions.into_iter().enumerate() {
            let (shell, layout) = fitting(&position);
            if let Some(mount) = self.main_mounts.get_mut(i) {
                if mount.position != position {
                    *mount = MainMount::new(position);
                }
            } else {
                self.main_mounts.push(MainMount::new(position));
            }
            self.main_mounts[i].fitted_bore = Some(shell.bore);
            self.main_mounts[i].gear = self.training_gear;
            self.main_mounts[i].fit(layout);
        }
        // The secondaries stand where this hull has room for them, clear of her turrets.
        self.wing_stations = design.wing_stations();
        self.fit_secondaries(self.secondaries);
        self.fit_strike_craft(design);
    }
    pub fn update_mount_service(&mut self, ship: &shipvivor_shipyard::Ship) {
        self.gun_train_rate = ship.gun_train_rate();
        for mount in &mut self.main_mounts {
            mount.in_service = ship.ready(mount.position.berth);
        }
        self.strike_craft_service(ship);
    }
    /// Training gear on every main gunhouse, now and as the yard adds them.
    pub fn fit_training(&mut self, gear: u16) {
        self.training_gear = gear;
        for mount in &mut self.main_mounts {
            mount.gear = gear;
        }
    }
    pub fn training_gear(&self) -> u16 {
        self.training_gear
    }
    pub fn fit_turrets(&mut self, layout: TurretLayout) {
        for mount in &mut self.main_mounts {
            mount.fit(layout);
        }
    }
    /// Who lays each main turret, by its number from the bow less one, and whether the
    /// captain's scope is up. A turret past the end of `controls` is manual.
    pub fn direct(&mut self, controls: &[Control], scope_up: bool) {
        let positions: Vec<_> = self.main_mounts.iter().map(|m| m.position).collect();
        for (&station, mount) in Station::SLOTS.iter().zip(&mut self.main_mounts) {
            let number = station.number(&positions);
            mount.control = controls.get(number - 1).copied().unwrap_or_default();
        }
        self.scope_up = scope_up;
    }
    /// The mount at each turret number from the bow, the foremost first.
    pub fn numbered(&self) -> Vec<Station> {
        let positions: Vec<_> = self.main_mounts.iter().map(|m| m.position).collect();
        let mut stations = Station::SLOTS[..positions.len()].to_vec();
        stations.sort_by_key(|s| s.number(&positions));
        stations
    }
    /// The ship the captain's sight is on, by her id, or `None` for anything else.
    pub fn mark(&mut self, ship: Option<u64>) {
        self.marked = ship;
    }
    /// The barrel a single shot would fire now, of those loaded and bearing on turrets
    /// the crew doesn't have. The smallest gun that can hurt the marked ship goes first,
    /// then the next smallest while it reloads, and so on up. A big round is wasted on a
    /// small ship, and a small one on plate it can't open, so guns that can't open hers
    /// come last. Turrets of one bore take turns, and so do each one's barrels.
    pub fn next_barrel(&self) -> Option<(Station, usize)> {
        let plate = self.marked.and_then(|id| self.ship(id)).map(Raider::telling_plate);
        let count = self.main_mounts.len();
        (0..count)
            .filter_map(|k| {
                let station = Station::SLOTS[(self.next_mount + k) % count];
                let mount = self.main_mount(station);
                let tube = (mount.status() == MountStatus::Ready && !mount.crewed())
                    .then(|| mount.next_tube())
                    .flatten()?;
                let wasted = plate.is_some_and(|grade| !mount.hurts(grade));
                Some(((wasted, mount.calibre().round() as u16, k), (station, tube)))
            })
            .min_by_key(|&(order, _)| order)
            .map(|(_, barrel)| barrel)
    }
    /// Order a click on the trigger. It stands for `VOLLEY_WINDOW` while guns come to bear,
    /// and a salvo replaces a single shot that has not gone yet.
    pub fn order_volley(&mut self, volley: Volley) {
        if self.volley.is_none_or(|(standing, _)| standing <= volley) {
            self.volley = Some((volley, VOLLEY_WINDOW));
        }
    }
    /// A clicked order still waiting for its guns.
    pub fn standing_volley(&self) -> Option<Volley> {
        self.volley.map(|(volley, _)| volley)
    }
    fn fire_main(&mut self, orders: &Orders) -> bool {
        let Some((station, _)) = self.next_barrel() else {
            return false;
        };
        if !self.fire_turret(station, orders) {
            return false;
        }
        self.next_mount = (station.index() + 1) % self.main_mounts.len();
        self.since_main = 0.;
        true
    }
    /// Fire the next loaded barrel of one turret, if it bears.
    fn fire_turret(&mut self, station: Station, orders: &Orders) -> bool {
        let mount = &mut self.main_mounts[station.index()];
        let shell = mount.round(orders.shell);
        let reload = mount.reload(orders);
        let Some(path) = mount.fire(reload) else {
            return false;
        };
        path.launch(
            &mut self.shots,
            &mut self.effects,
            &mut self.serial,
            Battery::Main,
            shell.breach_force(),
        );
        true
    }
    pub fn navigable_position(&self, pos: Vec2, clearance: f32) -> Vec2 {
        let pos = self
            .landmarks
            .iter()
            .fold(pos, |p, i| i.clear(p, clearance));
        self.ports
            .iter()
            .fold(pos, |p, port| port.clear_land(p, clearance))
    }

    /// Keep Breakwater's centre and both ends of her keel clear of shoals and piers.
    pub fn navigable_hull(&self, position: Vec2, forward: Vec2, hull: Hull) -> Vec2 {
        let mut safe = self.navigable_position(position, 6.);
        for end in [hull.bow - hull.radius, hull.radius - hull.stern] {
            let probe = safe + forward * end;
            safe += self.navigable_position(probe, hull.radius) - probe;
        }
        safe
    }

    fn reinforce(&mut self, player: Vec2) {
        if self.combatants() >= COMBATANT_LIMIT * self.strength {
            return;
        }
        // Steel just landed goes into the hulls before anything is launched.
        for port in &mut self.ports {
            port.fit_out();
        }
        // The bases take turns, and each launches from the first of its slips that is
        // ready and has clear water off it.
        for i in 0..self.ports.len() {
            let index = (self.next_dock + i) % self.ports.len();
            let port = &self.ports[index];
            if !port.roused {
                continue;
            }
            let launch = (0..port.docks().len()).find_map(|dock| {
                if !port.docks()[dock].ready() {
                    return None;
                }
                let (class, _) = port.docks()[dock].order();
                let bow = class.profile().hull.bow;
                let berth = port.dock_berth(dock)
                    + port.body.forward() * (class.profile().hull.stern - Hull::RAIDER.stern);
                // A coaster at anchor in the roadstead lies clear of the slip.
                let blocked = berth.distance(player) < SPAWN_STANDOFF
                    || self.raiders.iter().any(|r| {
                        r.moored.is_none()
                            && r.body.pos.distance(berth) <= r.class.profile().hull.bow + bow + 2.
                    })
                    || (self.floe.icebergs.iter())
                        .any(|b| b.body.pos.distance(berth) <= b.radius + bow + 2.);
                (!blocked).then_some((dock, class, berth))
            });
            let Some((dock, class, berth)) = launch else {
                continue;
            };
            let facing = berth + port.body.forward();
            self.ports[index].launch(dock, self.wave_interval * SLIP_TURNS);
            self.spawn(class, berth, facing);
            self.next_dock = (index + 1) % self.ports.len();
            return;
        }
    }

    fn spawn(&mut self, class: EnemyClass, pos: Vec2, facing: Vec2) {
        let mut raider = Raider::new(body(&mut self.serial, pos, bearing(facing - pos)), class);
        if let Some(muster) = &mut self.muster {
            raider.reweigh(muster.draw(class));
        }
        self.raiders.push(raider);
    }
    /// Let effects already in flight finish while the sector clock and combat are stopped.
    pub fn advance_effects(&mut self, dt: f32) {
        for port in &mut self.ports {
            port.advance_collapse(dt);
        }
        let mut landed = Vec::new();
        self.effects.retain_mut(|fx| {
            fx.age += dt;
            match fx.kind {
                EffectKind::Wreck(_) => fx.body.pos += fx.body.forward() * dt * 1.4,
                EffectKind::Downed(_) if fx.height > 0. => {
                    // A stricken aircraft glides on, slowing, and falls ever faster.
                    fx.body.pos += fx.body.forward() * dt * (34. - 8. * fx.age).max(6.);
                    fx.height = (fx.height - (5. + 26. * fx.age) * dt).max(0.);
                    if fx.height == 0. {
                        landed.push((fx.body.pos, fx.body.heading));
                    }
                }
                _ => {}
            }
            fx.age < fx.kind.lifetime()
        });
        // A ship is gone once her wreck has gone under.
        let effects = &self.effects;
        self.sunk
            .retain(|(id, _)| effects.iter().any(|fx| fx.body.id == *id));
        for (pos, heading) in landed {
            self.effects.push(Effect {
                body: body(&mut self.serial, pos, heading),
                height: 0.,
                kind: EffectKind::Splash,
                age: 0.,
            });
        }
    }

    pub fn step(&mut self, dt: f32, orders: &Orders) -> Events {
        let mut events = Events::default();
        self.keel = (shipvivor_compass::direction(orders.heading), orders.hull);
        self.last_strike = self
            .last_strike
            .and_then(|(outcome, left)| (left > dt).then_some((outcome, left - dt)));
        self.advance_effects(dt);
        self.drift_ice(dt, orders.player);
        self.at_sea += dt;
        let raids = self.at_sea + self.escalation;
        let tier = RAID_TIERS.iter().filter(|&&t| raids >= t).count();
        // A yard that sleeps still builds, and holds what it builds on its slip until it
        // is roused, so each stage wakes ready to answer.
        for port in &mut self.ports {
            port.work(dt, tier);
        }
        self.rouse_garrisons();
        self.next_wave -= dt;
        if self.next_wave <= 0. {
            // Staggered slipways dispatch independently once funded and clear of traffic.
            self.next_wave = 0.25;
            self.reinforce(orders.player);
        }
        self.sortie(orders.player);
        self.bring_up_regent(orders);
        self.send_convoys(dt, orders.player);
        self.ply_coasters(dt, orders.player);
        self.update_lookouts(dt, orders.player);
        // Every enemy gun leads the player by her present course and speed, and the set.
        let way = shipvivor_compass::direction(orders.heading) * orders.speed
            + self.floe.currents.at(orders.player);
        self.fly_air_raid(dt, orders.player, way);
        self.fly_harriers(dt, orders.player, way);
        self.run_airfields(dt);
        self.fly_warplanes(dt, orders.player, way, &mut events);
        for port in &mut self.ports {
            port.fire_turrets(
                dt,
                port.has_contact.then_some(orders.player),
                way,
                &mut self.shots,
                &mut self.effects,
                &mut self.serial,
            );
        }
        self.range_heavy_guns(dt, orders, way);
        self.work_torpedo_boats(dt, orders.player, orders.heading, way);
        self.keep_formation(dt, orders.player);
        let ground = self.patrol_ground();
        for raider in &mut self.raiders {
            let charted = raider.cargo_run.map_or_else(
                || raider.last_sighting.unwrap_or(ground),
                |(i, origin)| {
                    let port = &self.ports[i];
                    if port.wrecked() { origin } else { port.berth() }
                },
            );
            // A torpedo boat is bound wherever her run takes her.
            let bound = raider.attack.as_ref().map_or(charted, |run| run.goal);
            let destination = pilotage(&self.fairway, &self.landmarks, raider.body.pos, bound);
            let stream = self.floe.currents.at(raider.body.pos);
            raider.sail(
                dt,
                destination,
                destination != bound,
                &self.ports,
                &self.landmarks,
                &self.floe.icebergs,
                stream,
            );
            if !raider.has_contact {
                raider.stand_down();
                continue;
            }
            raider.fire_guns(
                dt,
                orders.player,
                way,
                &mut self.shots,
                &mut self.effects,
                &mut self.serial,
            );
        }
        self.resolve_contacts(orders, &mut events);
        self.ice_contacts(orders, &mut events);
        self.jostle_moorings(dt, orders, &mut events);
        let nearest = self.nearest_ship(orders.player);
        let aim = match orders.aim {
            Aim::Hold => None,
            Aim::Sight(p) | Aim::Track(p) => Some(p),
            Aim::At(p) => {
                let height = if let Some(height) = self.ports.iter().find_map(|port| {
                    port.turrets()
                        .iter()
                        .find(|g| g.live() && g.position(&port.body).distance_squared(p) < 25.)
                        .map(ShoreTurret::aim_height)
                }) {
                    height
                } else if let Some(top) = self.ports.iter().find_map(|port| {
                    (0..port.docks().len())
                        .find(|&i| port.dock_span(i, p, p).is_some())
                        .map(|i| port.dock_top(i))
                }) {
                    (3.2 + top) * 0.5
                } else if let Some(top) = self.ports.iter().find_map(|port| {
                    (0..port.factories().len())
                        .find(|&i| port.factory_span(i, p, p).is_some())
                        .map(|i| (port.factory_top(i) + port.factory_bottom(i)) * 0.5)
                }) {
                    top
                } else {
                    1.7
                };
                Some(p.extend(height).xzy())
            }
            Aim::Nearest => nearest
                .map(|r| r.aim_point(orders.player))
                .into_iter()
                .chain(self.ports.iter().flat_map(|p| {
                    p.turrets()
                        .iter()
                        .filter(|g| g.live())
                        .map(|g| g.position(&p.body).extend(g.aim_height()).xzy())
                }))
                .chain(self.ports.iter().flat_map(|p| {
                    (0..p.docks().len())
                        .filter(|&i| p.docks()[i].live())
                        .map(|i| p.dock_position(i).extend((p.dock_top(i) + 3.2) * 0.5).xzy())
                }))
                .min_by(|a, b| {
                    a.xz()
                        .distance_squared(orders.player)
                        .total_cmp(&b.xz().distance_squared(orders.player))
                }),
        };
        let held = matches!(orders.aim, Aim::At(_) | Aim::Sight(_) | Aim::Nearest);
        // Each turret is the trigger's or the crew's for this step. A crew with nothing of
        // its own to engage follows the sight, so the turret is laid when she wants it.
        let laid: Vec<(bool, Option<Vec3>)> = (self.main_mounts.iter())
            .map(|mount| {
                let captain = match mount.control {
                    Control::Manual => true,
                    Control::Auto => false,
                    Control::Scope => {
                        self.scope_up
                            && aim.is_some_and(|p| mount.bears_on(orders.player, orders.heading, p))
                    }
                };
                let own = (!captain)
                    .then(|| self.crew_target(mount, orders))
                    .flatten();
                (!captain, own)
            })
            .collect();
        for (mount, (crewed, own)) in self.main_mounts.iter_mut().zip(laid) {
            mount.crewed = crewed;
            mount.engaged = own.is_some();
            mount.train(dt, self.gun_train_rate, orders, own.or(aim));
        }
        // The crew fires a turret laid on its own target as each barrel loads, a ripple apart.
        for k in 0..self.main_mounts.len() {
            let mount = &self.main_mounts[k];
            let rounds = mount.layout().count() as f32 / mount.reload(orders);
            if mount.engaged() && mount.since_fired >= ripple(rounds, dt) {
                self.fire_turret(Station::SLOTS[k], orders);
            }
        }
        self.since_main += dt;
        if let Some((volley, left)) = self.volley.take() {
            // A salvo keeps standing to catch a mount that comes to bear late.
            let done = match volley {
                Volley::Single => self.fire_main(orders),
                Volley::Salvo => {
                    while self.fire_main(orders) {}
                    false
                }
            };
            if !done && left > dt {
                self.volley = Some((volley, left - dt));
            }
        }
        // A held trigger walks the battery one barrel at a time, as quick as the barrels it
        // has in the fight load between them, so a fast battery fires every barrel each
        // time it loads, its big guns too, rather than the small ones over and over.
        if held {
            let rounds: f32 = (self.main_mounts.iter())
                .filter(|m| {
                    !m.crewed()
                        && !matches!(m.status(), MountStatus::Disabled | MountStatus::Masked)
                })
                .map(|m| m.layout().count() as f32 / m.reload(orders))
                .sum();
            if self.since_main >= ripple(rounds, dt) {
                self.fire_main(orders);
            }
        }
        for (index, gun) in self.wing_guns.iter_mut().enumerate() {
            gun.reload = (gun.reload - dt).max(0.);
            let target = self
                .raiders
                .iter()
                .filter(|r| r.afloat())
                // Led from where this gun stands, which on a long hull is well off her
                // middle.
                .map(|r| {
                    let from = gun.pivot(orders.player, orders.heading).xz();
                    projectile::lead(from, r.body.pos, r.way, Battery::Secondary)
                })
                .filter(|&p| {
                    p.distance_squared(orders.player) <= self.secondaries.range.powi(2)
                        && gun.covers(orders.player, orders.heading, p)
                        && coast::open_water(orders.player, p, &self.ports, &self.landmarks)
                })
                .min_by(|a, b| {
                    a.distance_squared(orders.player)
                        .total_cmp(&b.distance_squared(orders.player))
                })
                .map(|p| p.extend(1.7).xzy())
                .or_else(|| {
                    gun.shore_target(
                        orders,
                        self.secondaries.range,
                        &self.raiders,
                        &self.ports,
                        &self.landmarks,
                        &self.floe.icebergs,
                    )
                });
            let Some(target) = target else {
                // With nothing in its arc the crew stands easy, and takes a moment of its
                // own to open fire when something comes into it.
                if gun.reload == 0. {
                    gun.reload = self.secondaries.reload
                        * mounts::STAND_EASY
                        * projectile::luck(self.serial, index as u64);
                }
                continue;
            };
            if self.gun_train_rate > 0.
                && let Some(pivot) = gun.track(
                    dt * self.gun_train_rate,
                    orders.player,
                    orders.heading,
                    target.xz(),
                )
                && gun.reload == 0.
            {
                // A twin mount fires both barrels together, side by side.
                let barrels = self.secondaries.barrels;
                for barrel in 0..barrels {
                    // No two rounds fall in quite the same place.
                    let fall = projectile::strewn(pivot, target, self.serial);
                    let Some(mut path) =
                        projectile::FlightPath::to(pivot, fall, Battery::Secondary)
                    else {
                        continue;
                    };
                    path.offset_muzzle((barrel as f32 - (barrels - 1) as f32 * 0.5) * 0.36);
                    path.rebore(self.secondaries.bore());
                    path.launch(
                        &mut self.shots,
                        &mut self.effects,
                        &mut self.serial,
                        Battery::Secondary,
                        self.secondaries.damage,
                    );
                }
                // Each round is loaded a little quicker or slower than the last.
                let ragged = 2. * projectile::luck(self.serial, index as u64) - 1.;
                gun.reload = self.secondaries.reload * (1. + mounts::RAGGED * ragged);
            }
        }
        self.fire_anti_air(dt, orders);
        self.advance_shots(dt, orders.player, &mut events);
        self.advance_torpedoes(dt, orders.player, &mut events);
        self.fly_floatplanes(dt, orders.player, orders.heading, &mut events);
        self.fire_missiles(dt, orders.player, orders.heading);
        self.advance_missiles(dt, &mut events);
        // Whatever hurt a cargo ship this tick put some of her cargo over the side.
        for r in &mut self.raiders {
            events.cargo += r.spill_cargo();
        }
        let mut index = 0;
        while index < self.raiders.len() {
            if self.raiders[index].afloat() {
                index += 1;
                continue;
            }
            let mut r = self.raiders.remove(index);
            events.sunk += 1;
            events.coasters += u16::from(r.class == EnemyClass::Coaster);
            events.torpedo_boats += u16::from(r.class == EnemyClass::TorpedoBoat);
            // Her base readies another in her place.
            if let Some(run) = &r.attack {
                self.flotillas[run.base].lost += 1;
            }
            events.cargo += r.founder_cargo();
            events.regent_sunk |= r.class == EnemyClass::Regent;
            let wreck = body(&mut self.serial, r.body.pos, r.body.heading);
            self.effects.push(Effect {
                body: wreck,
                kind: EffectKind::Wreck(r.class),
                height: 0.,
                age: 0.,
            });
            let stores = r.class.profile().stores;
            match self.rewards {
                Rewards::Direct => events.recovered += u16::from(stores),
                // A coaster carries no stores, and leaves nothing to tow.
                Rewards::Packages if stores > 0 => self.salvage.push(Package::new(
                    body(&mut self.serial, r.body.pos, r.body.heading),
                    stores,
                )),
                Rewards::Packages => {}
            }
            // She goes down as she was, with every mark of the fight on her.
            self.sunk.push((wreck.id, r));
        }
        self.receive_convoys();
        self.salvage.retain_mut(|package| {
            let stream = self.floe.currents.at(package.body.pos);
            if package.haul(orders.player, dt, &self.ports, &self.landmarks, stream) {
                events.recovered += u16::from(package.stores);
                return false;
            }
            true
        });
        if self.pressure != 1. {
            for blow in &mut events.blows {
                let weight = f32::from(blow.damage) * self.pressure + self.pressure_carry;
                let whole = weight.floor();
                self.pressure_carry = weight - whole;
                blow.damage = whole as u16;
            }
            events.damage = events.blows.iter().map(|b| b.damage).sum();
        }
        events
    }
}
fn entry(from: Vec2, to: Vec2, center: Vec2, radius: f32) -> Option<f32> {
    footprint(from, to, center, radius).map(|(enter, _)| enter)
}

/// Metres along the fairway a ship bound round a headland looks ahead for her next mark.
const ROAD_LEAD: f32 = 170.;

/// Where a ship at `from` bound for `goal` heads: straight for it when no shore stands
/// between, or else along the fairway towards it, so she follows the channel round its
/// bends rather than pressing against the land that hides her goal. Islands are left
/// to her helm.
fn pilotage(fairway: &[Vec2], landmarks: &[Landmark], from: Vec2, goal: Vec2) -> Vec2 {
    if fairway.len() < 2 {
        return goal;
    }
    let ashore = landmarks
        .iter()
        .filter(|l| matches!(l.kind, LandmarkKind::Mainland | LandmarkKind::Glacier))
        .any(|l| entry(from, goal, l.body.pos, l.radius).is_some());
    if !ashore {
        return goal;
    }
    let here = fairway_run(fairway, from);
    let there = fairway_run(fairway, goal);
    fairway_point(fairway, here + (there - here).clamp(-ROAD_LEAD, ROAD_LEAD))
}

/// How far along a fairway the point on it nearest `p` lies.
fn fairway_run(fairway: &[Vec2], p: Vec2) -> f32 {
    let mut run = 0.;
    let mut best = (0., f32::MAX);
    for pair in fairway.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let ab = b - a;
        let u = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0., 1.);
        let off = p.distance_squared(a + ab * u);
        if off < best.1 {
            best = (run + ab.length() * u, off);
        }
        run += ab.length();
    }
    best.0
}

/// Each ship's place in close order in a fleet of `classes` in line order, the guide's
/// first at her own station: metres ahead of the guide and to her starboard.
fn fleet_offsets(classes: &[EnemyClass]) -> Vec<(f32, f32)> {
    let hulls: Vec<Hull> = classes.iter().map(|c| c.profile().hull).collect();
    std::iter::once((0., 0.)).chain(close_order(&hulls)).collect()
}
/// Where a ship `(ahead, starboard)` of a guide at `at` lies, the guide facing back
/// along `axis`.
fn close_order_spot(at: Vec2, axis: Vec2, (ahead, starboard): (f32, f32)) -> Vec2 {
    let forward = -axis;
    at + forward * ahead + forward.perp() * starboard
}
/// The point `run` metres along a fairway, held to its ends.
fn fairway_point(fairway: &[Vec2], mut run: f32) -> Vec2 {
    for pair in fairway.windows(2) {
        let leg = pair[0].distance(pair[1]);
        if run <= leg {
            return pair[0].lerp(pair[1], (run / leg.max(1e-3)).max(0.));
        }
        run -= leg;
    }
    fairway.last().copied().unwrap_or(Vec2::ZERO)
}

#[cfg(test)]
mod tests;
