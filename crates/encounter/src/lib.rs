//! Deterministic arcade encounters. The renderer reads these actors and effects.
mod chart;
pub use chart::{Chart, HarbourSite, Ice, Islet, STRAIT, Waters, leg_seed};
mod coast;
pub use coast::{Landmark, LandmarkKind};
mod current;
pub use current::{Currents, MAX_CURRENT, Set};
mod iceberg;
pub use iceberg::{AVOIDED_RADIUS, IceShape, Iceberg};
mod survey;
pub use survey::{Defect, GATE_HALF_WIDTH, Metrics, Survey};
mod supply;
pub use supply::{Package, TOW_RANGE};
mod fleet;
pub use fleet::{EnemyClass, EnemyGun, Profile, Raider};
mod mounts;
mod plating;
mod port;
pub use mounts::{DeckOffsets, MainMount, MountStatus, Station, WingGun};
pub use plating::{HULL_OUTLINE, Scar, breakwater_outline};
mod projectile;
mod ram;
mod recon;
pub use recon::{AIR_SIGHT, ALERT_SECONDS, BASE_SIGHT, Spotter};
mod torpedo;
use glam::{Vec2, Vec3, Vec3Swizzles};
pub use port::{
    Factory, ISLAND_RADIUS, Port, SHORE_RANGE, SPAWN_STANDOFF, ShoreTurret, YARD_CAPACITY,
    YardState,
};
pub use projectile::{SIGHT_LIMIT, Shot, Sighting};
use projectile::{fire, footprint};
pub use ram::Hull;
pub use shipvivor_artillery::{Ammo, Impact, MainShell, Secondaries, TurretLayout};
use shipvivor_compass::heading as bearing;
pub use torpedo::{RACK_HALF_ARC, Rack, SPREAD_HALF_ARC, TORPEDO_RANGE, TORPEDO_SPEED, Torpedo};

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
    Raider,
    Shore,
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
    /// Two hulls striking hard enough to do damage.
    Crunch,
}
impl EffectKind {
    /// Seconds before the effect is removed.
    pub fn lifetime(self) -> f32 {
        match self {
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
            Self::Crunch => 0.9,
        }
    }
}
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
    pub damage: u16,
}
#[derive(Default, Debug)]
pub struct Events {
    /// Each hit on the player this tick. `damage` is their sum.
    pub blows: Vec<Blow>,
    pub hits: u16,
    pub sunk: u16,
    pub regent_sunk: bool,
    pub recovered: u16,
    pub damage: u16,
    /// Change in the player's position from hulls pushing apart.
    pub shove: Vec2,
    /// Change in the player's signed speed from ramming or being rammed.
    pub jolt: f32,
}
/// Range ahead and offset to starboard of the patrol screen when a sector opens.
const PICKETS: [(f32, f32); 4] = [(310., 38.), (390., -126.), (-96., -279.), (32., 297.)];
const COMBATANT_LIMIT: usize = 16;
/// Seconds into a sector when destroyers, then cruisers, then battleships join the raids.
pub const RAID_TIERS: [f32; 3] = [65., 160., 310.];
/// Seconds into a sector when the flagship puts to sea, between the cruisers and the
/// battleships. She is slow, so she comes into range near the end of the escalation.
pub const FLAGSHIP_SORTIE: f32 = 270.;
const _: () = assert!(FLAGSHIP_SORTIE > RAID_TIERS[1] && FLAGSHIP_SORTIE < RAID_TIERS[2]);

/// The flagship's part in a sector that has one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sortie {
    /// Fitting out. She puts to sea this many seconds into the sector, once a yard is
    /// clear for her.
    Due(f32),
    /// She put to sea from this port this many seconds into the sector.
    Sailed { port: usize, at: f32 },
}
/// Seconds between barrels while the trigger is held.
pub const RIPPLE: f32 = 0.3;
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
pub struct Battle {
    ports: [Port; 3],
    raiders: Vec<Raider>,
    spotters: [Spotter; 3],
    patrol_center: Vec2,
    shots: Vec<Shot>,
    torpedoes: Vec<Torpedo>,
    racks: [f32; 2],
    salvage: Vec<Package>,
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
    wing_guns: Vec<WingGun>,
    secondaries: Secondaries,
    last_strike: Option<(Impact, f32)>,
    /// The player's heading and hull this tick, for placing hits along her length.
    keel: (Vec2, Hull),
    next_wave: f32,
    wave_interval: f32,
    serial: u64,
    /// Seconds since the ship entered the sector. Raids grow heavier with it.
    at_sea: f32,
    next_dock: usize,
    flagship: Option<Sortie>,
    /// The current, and the ice it carries.
    floe: iceberg::Floe,
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
        Self::patrolled(wave_interval, chart).with_sea(chart)
    }
    /// The harbours, islands and patrols of a chart, in still and open water. The survey
    /// checks hundreds of these and needs neither the current nor the ice.
    pub(crate) fn patrolled(wave_interval: f32, chart: &Chart) -> Self {
        let mut b = Self::chart_sector(wave_interval, chart);
        let ahead = chart.gates[0].normalize_or(Vec2::NEG_Y);
        // These boats are already on patrol when the player enters the sector.
        for ((range, lateral), class) in PICKETS.into_iter().zip([
            EnemyClass::Patrol,
            EnemyClass::Patrol,
            EnemyClass::Destroyer,
            EnemyClass::Destroyer,
        ]) {
            let at = b.navigable_position(
                ahead * range + ahead.perp() * lateral,
                class.profile().hull.bow,
            );
            b.spawn(class, at, Vec2::ZERO);
        }
        b
    }

    fn chart_sector(wave_interval: f32, chart: &Chart) -> Self {
        let mut serial = 0;
        let ports = Port::chart(&chart.harbours, wave_interval, &mut serial);
        let ahead = chart.gates[0].normalize_or(Vec2::NEG_Y);
        let landmarks = chart
            .islets
            .iter()
            .map(|islet| Landmark {
                body: body(&mut serial, islet.pos, islet.heading),
                radius: islet.radius,
                kind: islet.kind,
            })
            .collect();
        let mut b = Self {
            spotters: std::array::from_fn(|i| Spotter::patrol(&ports[i], i)),
            patrol_center: chart.gates.iter().sum::<Vec2>() / chart.gates.len() as f32,
            landmarks,
            ports,
            raiders: vec![],
            shots: vec![],
            torpedoes: vec![],
            racks: [0.; 2],
            salvage: vec![],
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
            wing_guns: WingGun::battery(Secondaries::STOCK.per_side),
            secondaries: Secondaries::STOCK,
            last_strike: None,
            keel: (Vec2::NEG_Y, Hull::CRUISER),
            next_wave: wave_interval,
            wave_interval,
            serial,
            at_sea: 0.,
            next_dock: 0,
            flagship: None,
            floe: iceberg::Floe::slack(),
        };
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
    /// The flagship is fitting out in one of the yards and puts to sea partway through.
    pub fn with_flagship(mut self) -> Self {
        self.flagship = Some(Sortie::Due(FLAGSHIP_SORTIE));
        self
    }
    /// The Regent is already at sea with a screen. Find water wide enough for her
    /// whole hull, even on a chart whose first gate points straight at a headland.
    pub fn regent_encounter(wave_interval: f32, chart: &Chart) -> Self {
        let mut battle = Self::chart_sector(wave_interval, chart);
        let approach = chart.gates[0].normalize_or(Vec2::NEG_Y);
        // The leading boats can be drawn away from the slower battle line.
        let screen = [
            (EnemyClass::Patrol, -95., -98.),
            (EnemyClass::Destroyer, -60., 145.),
            (EnemyClass::Destroyer, 72., -157.),
            (EnemyClass::Cruiser, 86., 80.),
        ];
        let class = EnemyClass::Regent;
        let hull = class.profile().hull;
        let mut range = 480.;
        let station = 'search: loop {
            for sector in [0, 1, -1, 2, -2, 3, -3, 4, -4, 5, -5, 6] {
                let axis =
                    Vec2::from_angle(sector as f32 * std::f32::consts::TAU / 12.).rotate(approach);
                let at = axis * range;
                let clear =
                    |pos, room| battle.navigable_position(pos, room).distance_squared(pos) < 0.01;
                if clear(at, hull.bow)
                    && screen.iter().all(|(escort, fore, abeam)| {
                        clear(
                            at + axis * *fore + axis.perp() * *abeam,
                            escort.profile().hull.bow,
                        )
                    })
                {
                    break 'search at;
                }
            }
            range += hull.bow;
        };
        let axis = station.normalize();
        for (escort, fore, abeam) in screen {
            let at = station + axis * fore + axis.perp() * abeam;
            battle.spawn(escort, at, Vec2::ZERO);
        }
        battle.spawn(class, station, station + axis.perp());
        battle.with_sea(chart)
    }

    fn combatants(&self) -> usize {
        self.raiders
            .iter()
            .filter(|r| r.class != EnemyClass::Freighter)
            .count()
    }
    pub fn flagship(&self) -> Option<Sortie> {
        self.flagship
    }
    /// Seconds since the ship entered the sector.
    pub fn at_sea(&self) -> f32 {
        self.at_sea
    }
    /// Where the flagship would be met leaving this port, her stern clear of the quay.
    fn sortie_station(&self, port: &Port) -> Vec2 {
        let hull = EnemyClass::Parliament.profile().hull;
        self.navigable_position(port.berth() + port.body.forward() * hull.stern, hull.radius)
    }
    /// Once she is due, the flagship leaves the working yard farthest from the player
    /// whose approaches are clear of other ships.
    fn sortie(&mut self, player: Vec2) {
        let Some(Sortie::Due(due)) = self.flagship else {
            return;
        };
        if self.at_sea < due {
            return;
        }
        let class = EnemyClass::Parliament;
        let bow = class.profile().hull.bow;
        let clear = |at: Vec2| {
            at.distance(player) >= SPAWN_STANDOFF
                && self
                    .raiders
                    .iter()
                    .all(|r| r.body.pos.distance(at) > r.class.profile().hull.bow + bow + 2.)
                && self
                    .icebergs()
                    .iter()
                    .all(|b| b.body.pos.distance(at) > b.radius + bow + 4.)
        };
        let station = self
            .ports
            .iter()
            .enumerate()
            .filter(|(_, port)| port.yard_state() != YardState::Ruined)
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
        }
    }

    pub fn ports(&self) -> &[Port; 3] {
        &self.ports
    }
    pub fn ruined_yards(&self) -> usize {
        self.ports
            .iter()
            .filter(|port| port.yard_state() == YardState::Ruined)
            .count()
    }
    pub fn raiders(&self) -> &[Raider] {
        &self.raiders
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
    pub fn fit_secondaries(&mut self, fit: Secondaries) {
        let wanted = WingGun::battery(fit.per_side);
        self.wing_guns.truncate(wanted.len());
        let aboard = self.wing_guns.len();
        self.wing_guns.extend(wanted.into_iter().skip(aboard));
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
            self.main_mounts[i].fit(layout);
        }
    }
    pub fn update_mount_service(&mut self, ship: &shipvivor_shipyard::Ship) {
        self.gun_train_rate = ship.gun_train_rate();
        for mount in &mut self.main_mounts {
            mount.in_service = ship.ready(mount.position.berth);
        }
    }
    pub fn fit_turrets(&mut self, layout: TurretLayout) {
        for mount in &mut self.main_mounts {
            mount.fit(layout);
        }
    }
    /// The barrel a single shot would fire now: the mounts alternate, then each pair.
    pub fn next_barrel(&self) -> Option<(Station, usize)> {
        (0..self.main_mounts.len()).find_map(|k| {
            let station = Station::SLOTS[(self.next_mount + k) % self.main_mounts.len()];
            let mount = self.main_mount(station);
            (mount.status() == MountStatus::Ready)
                .then(|| mount.next_tube())
                .flatten()
                .map(|tube| (station, tube))
        })
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
        let mount = &mut self.main_mounts[station.index()];
        let shell = mount.round(orders.shell);
        let reload = orders.reload * shell.calibre_inches() / orders.shell.calibre_inches();
        let Some(path) = mount.fire(reload, orders.shell.ammo) else {
            return false;
        };
        path.launch(
            &mut self.shots,
            &mut self.effects,
            &mut self.serial,
            Battery::Main,
            shell.breach_force(),
        );
        self.next_mount = (station.index() + 1) % self.main_mounts.len();
        self.since_main = 0.;
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
        if self.combatants() >= COMBATANT_LIMIT {
            return;
        }
        for i in 0..self.ports.len() {
            let index = (self.next_dock + i) % self.ports.len();
            let port = &self.ports[index];
            let (class, cost) = port.build_order();
            if port.slipway_clock > 0.
                || port.stock < cost
                || port.yard_state() == YardState::Ruined
            {
                continue;
            }
            let berth = port.berth()
                + port.body.forward() * (class.profile().hull.stern - Hull::RAIDER.stern);
            if berth.distance(player) < SPAWN_STANDOFF
                || self.raiders.iter().any(|r| {
                    r.body.pos.distance(berth)
                        <= r.class.profile().hull.bow + class.profile().hull.bow + 2.
                })
                || self
                    .floe
                    .icebergs
                    .iter()
                    .any(|b| b.body.pos.distance(berth) <= b.radius + class.profile().hull.bow + 2.)
            {
                continue;
            }
            let facing = berth + port.body.forward();
            self.ports[index].launch(cost, self.wave_interval * self.ports.len() as f32);
            self.spawn(class, berth, facing);
            self.next_dock = (index + 1) % self.ports.len();
            return;
        }
    }

    fn spawn(&mut self, class: EnemyClass, pos: Vec2, facing: Vec2) {
        self.raiders.push(Raider::new(
            body(&mut self.serial, pos, bearing(facing - pos)),
            class,
        ));
    }
    /// Let effects already in flight finish while the sector clock and combat are stopped.
    pub fn advance_effects(&mut self, dt: f32) {
        self.effects.retain_mut(|fx| {
            fx.age += dt;
            if matches!(fx.kind, EffectKind::Wreck(_)) {
                fx.body.pos += fx.body.forward() * dt * 1.4;
            }
            fx.age < fx.kind.lifetime()
        });
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
        let tier = RAID_TIERS.iter().filter(|&&t| self.at_sea >= t).count();
        for port in &mut self.ports {
            port.work(dt, tier);
        }
        self.next_wave -= dt;
        if self.next_wave <= 0. {
            // Staggered slipways dispatch independently once funded and clear of traffic.
            self.next_wave = 0.25;
            self.reinforce(orders.player);
        }
        self.sortie(orders.player);
        self.send_convoys(dt, orders.player);
        self.update_lookouts(dt, orders.player);
        // Every enemy gun leads the player by her present course and speed, and the set.
        let way = shipvivor_compass::direction(orders.heading) * orders.speed
            + self.floe.currents.at(orders.player);
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
        for raider in &mut self.raiders {
            let destination = raider.cargo_run.map_or_else(
                || raider.last_sighting.unwrap_or(self.patrol_center),
                |(i, origin)| {
                    let port = &self.ports[i];
                    if port.yard_state() == YardState::Ruined {
                        origin
                    } else {
                        port.berth()
                    }
                },
            );
            let stream = self.floe.currents.at(raider.body.pos);
            raider.sail(
                dt,
                destination,
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
        let nearest = self.nearest_ship(orders.player);
        let aim = match orders.aim {
            Aim::Hold => None,
            Aim::Sight(p) | Aim::Track(p) => Some(p),
            Aim::At(p) => {
                let height = if self.ports.iter().any(|port| {
                    port.turrets()
                        .iter()
                        .any(|g| g.live() && g.position(&port.body).distance_squared(p) < 25.)
                }) {
                    5.7
                } else if let Some(port) = self.ports.iter().find(|p_| p_.yard_span(p, p).is_some())
                {
                    (3.2 + port.yard_top()) * 0.5
                } else if let Some(top) = self.ports.iter().find_map(|port| {
                    (0..port.factories().len())
                        .find(|&i| port.factory_span(i, p, p).is_some())
                        .map(|i| port.factory_top(i))
                }) {
                    (3.2 + top) * 0.5
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
                        .map(|g| g.position(&p.body).extend(5.7).xzy())
                }))
                .chain(
                    self.ports
                        .iter()
                        .filter(|p| p.yard_state() != YardState::Ruined)
                        .map(|p| p.yard_position().extend((p.yard_top() + 3.2) * 0.5).xzy()),
                )
                .min_by(|a, b| {
                    a.xz()
                        .distance_squared(orders.player)
                        .total_cmp(&b.xz().distance_squared(orders.player))
                }),
        };
        let held = matches!(orders.aim, Aim::At(_) | Aim::Sight(_) | Aim::Nearest);
        for mount in &mut self.main_mounts {
            mount.train(dt, self.gun_train_rate, orders, aim);
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
        // A held trigger walks the battery one barrel at a time.
        if held && self.since_main >= RIPPLE {
            self.fire_main(orders);
        }
        for (i, gun) in self.wing_guns.iter_mut().enumerate() {
            gun.set_position(i, self.deck);
            gun.reload = (gun.reload - dt).max(0.);
            let target = self
                .raiders
                .iter()
                .filter(|r| r.afloat())
                .map(|r| r.body.pos + r.way * (r.body.pos.distance(orders.player) / 240.))
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
            if let Some(target) = target
                && self.gun_train_rate > 0.
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
                    let Some(mut path) =
                        projectile::FlightPath::to(pivot, target, Battery::Secondary)
                    else {
                        continue;
                    };
                    path.offset_muzzle((barrel as f32 - (barrels - 1) as f32 * 0.5) * 0.36);
                    path.launch(
                        &mut self.shots,
                        &mut self.effects,
                        &mut self.serial,
                        Battery::Secondary,
                        self.secondaries.damage,
                    );
                }
                gun.reload = self.secondaries.reload;
            }
        }
        self.advance_shots(dt, orders.player, &mut events);
        self.advance_torpedoes(dt, &mut events);
        self.raiders.retain(|r| {
            if r.afloat() {
                return true;
            }
            events.sunk += 1;
            events.regent_sunk |= r.class == EnemyClass::Regent;
            self.effects.push(Effect {
                body: body(&mut self.serial, r.body.pos, r.body.heading),
                kind: EffectKind::Wreck(r.class),
                height: 0.,
                age: 0.,
            });
            let stores = r.class.profile().stores;
            self.salvage.push(Package::new(
                body(&mut self.serial, r.body.pos, r.body.heading),
                stores,
            ));
            false
        });
        self.receive_convoys();
        self.salvage.retain_mut(|package| {
            let stream = self.floe.currents.at(package.body.pos);
            if package.haul(orders.player, dt, &self.ports, &self.landmarks, stream) {
                events.recovered += u16::from(package.stores);
                return false;
            }
            true
        });
        events
    }
}
fn entry(from: Vec2, to: Vec2, center: Vec2, radius: f32) -> Option<f32> {
    footprint(from, to, center, radius).map(|(enter, _)| enter)
}

#[cfg(test)]
mod tests;
