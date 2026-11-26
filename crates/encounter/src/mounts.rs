//! Deck-relative training for Breakwater's mounts, with each barrel loaded on its own.
use super::{
    Ammo, MainShell, Orders, Secondaries, TurretLayout,
    projectile::{FlightPath, Trajectory},
};
use glam::{FloatExt, Vec2, Vec3, Vec3Swizzles};
use shipvivor_artillery::ArmorGrade;
use shipvivor_compass::direction;
use shipvivor_shipyard::GunStation;
use std::f32::consts::{PI, TAU};

const CRUISER_TRAVERSE: f32 = shipvivor_artillery::STOCK_TRAVERSE.to_radians();
const ELEVATION_RATE: f32 = 12_f32.to_radians();
const RECOIL: f32 = 0.22;

fn signed(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

// Unrestricted mounts may cross the wrap at north. Return true when on bearing.
pub(super) fn train_full(angle: &mut f32, wanted: f32, max_step: f32) -> bool {
    let delta = signed(wanted - *angle);
    *angle = signed(*angle + delta.clamp(-max_step, max_step));
    delta.abs() <= max_step
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Station {
    Bow,
    Stern,
    Third,
    Fourth,
    Fifth,
    Sixth,
}
impl Station {
    /// The two stations on a stock Breakwater.
    pub const ALL: [Self; 2] = [Self::Bow, Self::Stern];
    /// Every main turret a hull can carry, as many as a capital beam takes.
    pub const SLOTS: [Self; 6] = [
        Self::Bow,
        Self::Stern,
        Self::Third,
        Self::Fourth,
        Self::Fifth,
        Self::Sixth,
    ];
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn label(self, positions: &[GunStation]) -> &'static str {
        let pos = positions[self.index()];
        let first = positions.first().unwrap().berth.module == pos.berth.module;
        let last = positions.last().unwrap().berth.module == pos.berth.module;
        match (first, last, pos.abeam.total_cmp(&0.)) {
            (true, _, std::cmp::Ordering::Less) => "AP",
            (true, _, std::cmp::Ordering::Greater) => "AS",
            (_, true, std::cmp::Ordering::Less) => "YP",
            (_, true, std::cmp::Ordering::Greater) => "YS",
            (_, _, std::cmp::Ordering::Less) => "P",
            (_, _, std::cmp::Ordering::Greater) => "S",
            (true, _, _) => "A",
            (_, true, _) => "Y",
            _ if self.index() == 1 => "B",
            _ => "X",
        }
    }
    /// The turret's number, counting from 1 at the bow, port before starboard abreast.
    pub fn number(self, positions: &[GunStation]) -> usize {
        let me = self.index();
        let ahead = |j: usize| {
            let (a, b) = (positions[j], positions[me]);
            (b.fore.total_cmp(&a.fore))
                .then(a.abeam.total_cmp(&b.abeam))
                .then(j.cmp(&me))
                .is_lt()
        };
        1 + (0..positions.len()).filter(|&j| ahead(j)).count()
    }
}

/// Who lays and fires a main turret.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Control {
    /// The sight lays it and only the trigger fires it.
    #[default]
    Manual,
    /// The captain's while the scope is up and it bears on the sight, the crew's otherwise.
    Scope,
    /// The crew lays it on a target of its own and fires as it loads.
    Auto,
}
impl Control {
    /// The order Z steps through.
    pub fn next(self) -> Self {
        match self {
            Self::Manual => Self::Scope,
            Self::Scope => Self::Auto,
            Self::Auto => Self::Manual,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Manual => "MANUAL",
            Self::Scope => "SCOPE",
            Self::Auto => "AUTO",
        }
    }
}
/// Where a mount stands. `Ready` means it bears on the aim with at least one barrel loaded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MountStatus {
    Training,
    Disabled,
    Masked,
    Loading,
    Ready,
}

#[derive(Clone, Copy, Default)]
struct Tube {
    /// Seconds until the next round is rammed home.
    reload_left: f32,
    /// The reload `reload_left` is counting down from.
    reload: f32,
    recoil_left: f32,
    /// The round in the barrel, or the one being loaded. A round goes in as whatever is
    /// selected while it loads, and a loaded barrel keeps its round.
    ammo: Ammo,
}

pub struct MainMount {
    pub position: GunStation,
    pub(super) in_service: bool,
    pub(super) fitted_bore: Option<u16>,
    /// Ranks of training gear fitted to the gunhouse.
    pub(super) gear: u16,
    // Signed displacement from the mount's home stop, in deck coordinates.
    train: f32,
    pitch: f32,
    scale: f32,
    range: f32,
    mark_height: f32,
    status: MountStatus,
    layout: TurretLayout,
    tubes: [Tube; TurretLayout::Quad.count()],
    next_tube: usize,
    /// The flight from this mount to the aim, once it bears.
    solution: Option<FlightPath>,
    pub(super) control: Control,
    /// The crew has the turret this step: it is on automatic, or on scope while the
    /// captain's scope is down or it cannot bear on the sight.
    pub(super) crewed: bool,
    /// The crew has a target of its own and fires on it as the barrels load.
    pub(super) engaged: bool,
    /// Seconds since this turret last fired, which spaces the crew's rounds.
    pub(super) since_fired: f32,
    /// The round this turret's bore fires, with the ship's caps and charges, as it was
    /// last laid.
    shell: MainShell,
}
impl MainMount {
    pub(super) fn new(position: GunStation) -> Self {
        Self {
            position,
            in_service: true,
            fitted_bore: None,
            gear: 0,
            train: 0.,
            pitch: 0.,
            scale: super::MainShell::STOCK.mount_scale(),
            range: 90.,
            mark_height: 0.,
            status: MountStatus::Training,
            layout: TurretLayout::Twin,
            tubes: [Tube::default(); TurretLayout::Quad.count()],
            next_tube: 0,
            solution: None,
            control: Control::Manual,
            crewed: false,
            engaged: false,
            since_fired: 0.,
            shell: MainShell::STOCK,
        }
    }
    pub fn control(&self) -> Control {
        self.control
    }
    /// The crew, not the trigger, has the turret this step.
    pub fn crewed(&self) -> bool {
        self.crewed
    }
    /// The crew has the turret and a target of its own to fire on.
    pub fn engaged(&self) -> bool {
        self.crewed && self.engaged
    }
    /// Whether `target` lies inside the turret's arc, clear of its stops and masks.
    pub fn bears_on(&self, player: Vec2, heading: f32, target: Vec3) -> bool {
        let from = self.pivot(player, heading).xz();
        target.xz().distance_squared(from) > 1e-4
            && self
                .position
                .bears(shipvivor_compass::heading(target.xz() - from) - heading)
    }
    pub fn deck_bearing(&self) -> f32 {
        self.position.home + self.train
    }
    pub(super) fn pivot(&self, player: Vec2, heading: f32) -> Vec3 {
        let forward = direction(heading);
        (player + forward * self.position.fore + forward.perp() * self.position.abeam)
            .extend(1.72 + self.position.lift() + 0.63 * self.scale)
            .xzy()
    }
    pub fn layout(&self) -> TurretLayout {
        self.layout
    }
    pub(super) fn fit(&mut self, layout: TurretLayout) {
        self.tubes[self.layout.count().min(layout.count())..].fill(Tube::default());
        self.layout = layout;
        self.next_tube %= layout.count();
        if self.solution.is_some() {
            self.status = self.settle();
        }
    }
    pub fn round(&self, round: MainShell) -> MainShell {
        MainShell {
            bore: self.fitted_bore.unwrap_or(round.bore),
            ..round
        }
    }
    /// Whether the round in the barrel this turret fires next, sent along the flight it
    /// is laid on, would open sound plate of `grade`. A turret not laid can't say, and
    /// counts as able.
    pub fn hurts(&self, grade: ArmorGrade) -> bool {
        let (Some(path), Some(tube)) = (self.solution, self.next_tube()) else {
            return true;
        };
        let round = MainShell {
            ammo: self.tubes[tube].ammo,
            ..self.shell
        };
        round.at_contact(path.duration).opens(grade)
    }
    /// Inches of bore this turret fires, as it was last laid.
    pub fn calibre(&self) -> f32 {
        self.shell.calibre_inches()
    }
    /// Seconds each of this turret's barrels takes to load. The ordered reload is for the
    /// ship's own bore. A turret of another bore loads on the same curve, with the same
    /// drill.
    pub(super) fn reload(&self, orders: &Orders) -> f32 {
        orders.reload * self.round(orders.shell).load_seconds() / orders.shell.load_seconds()
    }
    /// Radians per second this gunhouse trains at full power. A heavier bore trains
    /// more slowly, and training gear wins it back.
    pub fn traverse(&self, round: MainShell) -> f32 {
        CRUISER_TRAVERSE
            * shipvivor_artillery::training_rate(self.round(round).calibre_inches(), self.gear)
    }
    /// The flight a round from this mount would take to `target`, from the centre of
    /// the gunhouse: the same flight the mount trains on and launches. A target beyond
    /// the round's reach draws it down at its reach instead.
    pub fn trajectory(
        &self,
        player: Vec2,
        heading: f32,
        target: Vec3,
        round: MainShell,
    ) -> Option<Trajectory> {
        let shell = self.round(round);
        let pivot = self.pivot(player, heading);
        FlightPath::main(pivot, target, shell).map(|path| path.trajectory())
    }
    pub fn pitch(&self) -> f32 {
        self.pitch
    }
    pub fn status(&self) -> MountStatus {
        self.status
    }
    pub fn loaded(&self, tube: usize) -> bool {
        self.tubes[tube].reload_left == 0.
    }
    /// Seconds until this barrel is loaded.
    pub fn reload_left(&self, tube: usize) -> f32 {
        self.tubes[tube].reload_left
    }
    /// How far this barrel's reload has come, from 0 just fired to 1 loaded.
    pub fn load_fraction(&self, tube: usize) -> f32 {
        let t = self.tubes[tube];
        if t.reload_left == 0. {
            1.
        } else {
            1. - t.reload_left / t.reload
        }
    }
    pub fn recoil(&self, tube: usize) -> f32 {
        self.tubes[tube].recoil_left / RECOIL
    }
    pub fn ammo(&self, tube: usize) -> Ammo {
        self.tubes[tube].ammo
    }
    /// The loaded barrel this mount fires next, cycling through the fitted tubes.
    pub fn next_tube(&self) -> Option<usize> {
        let count = self.layout.count();
        (0..count)
            .map(|i| (self.next_tube + i) % count)
            .find(|&t| self.loaded(t))
    }
    pub fn aim_mark(&self, player: Vec2, heading: f32) -> Vec3 {
        let pivot = self.pivot(player, heading);
        (pivot.xz() + direction(heading + self.deck_bearing()) * self.range)
            .extend(self.mark_height)
            .xzy()
    }
    /// Load, then train onto the target. The battery decides which ready barrels fire.
    pub(super) fn train(
        &mut self,
        dt: f32,
        train_rate: f32,
        orders: &Orders,
        target: Option<Vec3>,
    ) {
        self.since_fired += dt;
        if !self.in_service {
            self.solution = None;
            self.status = MountStatus::Disabled;
            return;
        }
        for tube in &mut self.tubes[..self.layout.count()] {
            if tube.reload_left > 0. {
                tube.ammo = orders.shell.ammo;
            }
            tube.reload_left = (tube.reload_left - dt).max(0.);
            tube.recoil_left = (tube.recoil_left - dt).max(0.);
        }
        self.solution = None;
        let shell = self.round(orders.shell);
        self.shell = shell;
        self.scale = shell.mount_scale();
        let pivot = self.pivot(orders.player, orders.heading);
        let Some((target, path)) =
            target.and_then(|p| Some((p, FlightPath::main(pivot, p, shell)?)))
        else {
            self.status = MountStatus::Training;
            return;
        };
        let wanted = signed(path.heading - orders.heading - self.position.home);
        let beyond_stop = wanted.abs() > self.position.half_arc;
        let masked = !self.position.bears(path.heading - orders.heading);
        // Stay at the nearer existing stop while a target passes through the blind sector.
        // To change sides the mount has to travel back through its home position.
        let stop = if wanted.abs() < self.position.inner_mask {
            let side = if self.train.abs() > 0.01 {
                self.train
            } else {
                wanted
            };
            self.position.inner_mask.copysign(side)
        } else if beyond_stop && self.train.abs() > 0.01 {
            self.train.signum() * self.position.half_arc
        } else {
            wanted.clamp(-self.position.half_arc, self.position.half_arc)
        };
        let motion_dt =
            dt * train_rate * shipvivor_artillery::training_rate(shell.calibre_inches(), self.gear);
        self.train = self.train.move_towards(stop, CRUISER_TRAVERSE * motion_dt);
        self.pitch = self
            .pitch
            .move_towards(path.pitch, ELEVATION_RATE * motion_dt);
        self.range = (pivot.xz().distance(target.xz())).min(shell.reach());
        self.mark_height = target.y + self.range * (self.pitch - path.pitch).sin();
        self.status = if masked {
            MountStatus::Masked
        } else if (wanted - self.train).abs() > 0.0001 || (self.pitch - path.pitch).abs() > 0.0001 {
            MountStatus::Training
        } else {
            self.solution = Some(path);
            self.settle()
        };
    }
    fn settle(&self) -> MountStatus {
        if self.next_tube().is_some() {
            MountStatus::Ready
        } else {
            MountStatus::Loading
        }
    }
    /// Fire the next loaded barrel if the mount bears, and start its reload.
    /// Fire the next loaded barrel if the mount bears, and start loading `next` into it.
    pub(super) fn fire(&mut self, reload: f32, next: Ammo) -> Option<FlightPath> {
        if self.status != MountStatus::Ready {
            return None;
        }
        let mut path = self.solution?;
        let tube = self.next_tube()?;
        path.offset_muzzle(self.layout.barrel_x(tube) * self.scale);
        path.load(self.tubes[tube].ammo);
        self.tubes[tube] = Tube {
            reload_left: reload,
            reload,
            recoil_left: RECOIL,
            ammo: next,
        };
        self.next_tube = (tube + 1) % self.layout.count();
        self.status = self.settle();
        self.since_fired = 0.;
        Some(path)
    }
}

/// Relocate stock deck furniture with its compartment when the yard moves a module.
#[derive(Clone, Copy, Debug)]
pub struct DeckOffsets {
    pub bridge_z: f32,
    pub machinery_z: f32,
    pub beam_scale: f32,
}
impl DeckOffsets {
    pub fn for_design(design: &shipvivor_shipyard::Design) -> Self {
        use shipvivor_shipyard::Component;
        let shift = |component, stock_module| {
            design
                .modules()
                .iter()
                .position(|m| m.fit.contains(&component))
                .map_or(0., |index| design.span(index).0 - stock_module as f32 * 5.)
        };
        Self {
            bridge_z: shift(Component::Director, 2),
            machinery_z: shift(Component::TorpedoRack, 3),
            beam_scale: design.beam_factor(),
        }
    }
    pub fn wing_position(self, index: usize) -> (f32, f32) {
        let (x, fore) = WingGun::station(index);
        let shift = match WING_STATIONS[(index / 2).min(WING_STATIONS.len() - 1)].mounting {
            Mounting::Bridge => self.bridge_z,
            Mounting::Machinery => self.machinery_z,
            Mounting::Hull => 0.,
        };
        (x * self.beam_scale, fore - shift)
    }
}

/// What a wing gun's deck goes with when the yard moves a deckhouse.
#[derive(Clone, Copy)]
enum Mounting {
    Bridge,
    Machinery,
    /// The forecastle, which never moves.
    Hull,
}

/// Where a pair of wing guns stands: metres out from the centreline and forward of
/// amidships, the bearing from the bow of the middle of the starboard gun's arc, and how
/// far either way of that the guns train.
struct WingStation {
    x: f32,
    fore: f32,
    home: f32,
    half_arc: f32,
    mounting: Mounting,
}
/// A beam gun covers its own side, short of four degrees from the bow and the stern.
const BEAM: (f32, f32) = (90_f32.to_radians(), 86_f32.to_radians());
/// A bow gun covers from four degrees across the bow round to its own beam, so the pair
/// overlap dead ahead.
const BOW: (f32, f32) = (43_f32.to_radians(), 47_f32.to_radians());
/// Wing gun stations in the order they are fitted, a pair to each. She is built with
/// guns amidships and on the forecastle, between the forward turret and the capstans.
/// More go beside the bridge, then aft of the launches.
const WING_STATIONS: [WingStation; Secondaries::MOST_PER_SIDE] = [
    WingStation {
        x: 2.18,
        fore: 0.,
        home: BEAM.0,
        half_arc: BEAM.1,
        mounting: Mounting::Bridge,
    },
    WingStation {
        x: 1.35,
        fore: 10.6,
        home: BOW.0,
        half_arc: BOW.1,
        mounting: Mounting::Hull,
    },
    WingStation {
        x: 2.18,
        fore: 3.8,
        home: BEAM.0,
        half_arc: BEAM.1,
        mounting: Mounting::Bridge,
    },
    WingStation {
        x: 2.18,
        fore: -6.9,
        home: BEAM.0,
        half_arc: BEAM.1,
        mounting: Mounting::Machinery,
    },
];

/// A shielded close-range gun on the bow or the beam, worked by the crew.
pub struct WingGun {
    pub(super) reload: f32,
    abeam: f32,
    fore: f32,
    /// Bearing from the bow of the middle of its arc, and how far either way it trains.
    home: f32,
    half_arc: f32,
    /// Bearing from `home`.
    train: f32,
}
impl WingGun {
    /// The guns for `per_side` stations a side, port before starboard at each.
    pub(super) fn battery(per_side: usize) -> Vec<Self> {
        (0..per_side.min(Secondaries::MOST_PER_SIDE) * 2)
            .map(|index| {
                let (x, fore) = Self::station(index);
                Self {
                    abeam: x,
                    fore,
                    home: Self::home(index),
                    half_arc: WING_STATIONS[index / 2].half_arc,
                    train: 0.,
                    reload: 0.,
                }
            })
            .collect()
    }
    /// Where the gun at `index` in fitting order stands: metres to starboard and forward.
    pub fn station(index: usize) -> (f32, f32) {
        let side = if index.is_multiple_of(2) { -1. } else { 1. };
        let station = &WING_STATIONS[index / 2];
        (side * station.x, station.fore)
    }
    /// The bearing from the bow of the middle of the arc of the gun at `index`, where it
    /// lies trained fore and aft.
    pub fn home(index: usize) -> f32 {
        let side = if index.is_multiple_of(2) { -1. } else { 1. };
        side * WING_STATIONS[index / 2].home
    }
    pub(super) fn set_position(&mut self, index: usize, deck: DeckOffsets) {
        (self.abeam, self.fore) = deck.wing_position(index);
    }
    pub fn deck_x(&self) -> f32 {
        self.abeam
    }
    /// Metres forward of amidships.
    pub fn deck_fore(&self) -> f32 {
        self.fore
    }
    pub fn deck_bearing(&self) -> f32 {
        self.home + self.train
    }
    pub(super) fn pivot(&self, player: Vec2, heading: f32) -> Vec3 {
        (player + direction(heading).perp() * self.deck_x() + direction(heading) * self.fore)
            .extend(2.)
            .xzy()
    }

    /// The bearing of `target` from the middle of the gun's arc.
    fn off_home(&self, player: Vec2, heading: f32, target: Vec2) -> f32 {
        let delta = target - self.pivot(player, heading).xz();
        signed(shipvivor_compass::heading(delta) - heading - self.home)
    }
    pub(super) fn covers(&self, player: Vec2, heading: f32, target: Vec2) -> bool {
        self.off_home(player, heading, target).abs() <= self.half_arc
    }
    pub(super) fn track(
        &mut self,
        dt: f32,
        player: Vec2,
        heading: f32,
        target: Vec2,
    ) -> Option<Vec3> {
        let pivot = self.pivot(player, heading);
        let wanted = self.off_home(player, heading, target);
        let limit = self.half_arc;
        self.train = self
            .train
            .move_towards(wanted.clamp(-limit, limit), 68_f32.to_radians() * dt);
        ((self.train - wanted).abs() < 0.0001).then_some(pivot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Aim;
    use shipvivor_shipyard::Design;

    #[test]
    fn bow_changes_sides_via_the_bow_when_the_bridge_is_in_the_shorter_path() {
        let mut mount = MainMount::new(Design::breakwater().gun_stations()[0]);
        let orders = Orders {
            player: Vec2::ZERO,
            heading: 0.,
            speed: 0.,
            hull: crate::Hull::CRUISER,
            aim: Aim::Hold,
            shell: crate::MainShell::STOCK,
            reload: 3.2,
        };
        let pivot = mount.pivot(Vec2::ZERO, 0.);
        let target = |degrees: f32| {
            let position = direction(degrees.to_radians()) * 175.;
            pivot + position.extend(0.).xzy()
        };
        mount.train(10., 1., &orders, Some(target(135.)));
        assert_eq!(mount.status(), MountStatus::Ready);
        let before = mount.deck_bearing();
        mount.train(1., 1., &orders, Some(target(-135.)));
        assert!(mount.fire(orders.reload, Ammo::He).is_none());
        assert!((before - mount.deck_bearing() - CRUISER_TRAVERSE).abs() < 0.001);
        assert_eq!(mount.status(), MountStatus::Training);
        assert!((0..mount.layout().count()).all(|t| mount.loaded(t)));
    }

    #[test]
    fn each_barrel_loads_on_its_own_and_the_pair_take_turns() {
        let mut mount = MainMount::new(Design::breakwater().gun_stations()[0]);
        let orders = Orders {
            player: Vec2::ZERO,
            heading: 0.,
            speed: 0.,
            hull: crate::Hull::CRUISER,
            aim: Aim::Hold,
            shell: crate::MainShell::STOCK,
            reload: 6.4,
        };
        let ahead = Some(Vec3::NEG_Z * 150.);
        mount.train(10., 1., &orders, ahead);
        assert!(mount.fire(orders.reload, Ammo::He).is_some());
        assert_eq!(mount.status(), MountStatus::Ready);
        assert_eq!(mount.next_tube(), Some(1));
        assert!(mount.recoil(0) > 0.99 && mount.recoil(1) == 0.);
        mount.train(1., 1., &orders, ahead);
        assert!(mount.fire(orders.reload, Ammo::He).is_some());
        assert_eq!(mount.status(), MountStatus::Loading);
        assert!(mount.fire(orders.reload, Ammo::He).is_none());
        // The left barrel fired a second before the right and comes ready a second sooner.
        assert!((mount.reload_left(0) - 5.4).abs() < 1e-4);
        assert!((mount.load_fraction(1) - 0.).abs() < 1e-4);
        mount.train(5.5, 1., &orders, ahead);
        assert_eq!(mount.status(), MountStatus::Ready);
        assert_eq!(mount.next_tube(), Some(0));
        assert!(!mount.loaded(1) && (mount.load_fraction(1) - 5.5 / 6.4).abs() < 1e-4);
    }
    #[test]
    fn turrets_number_from_the_bow_with_port_before_starboard() {
        let stock = Design::breakwater().gun_stations();
        assert_eq!(Station::Bow.number(&stock), 1);
        assert_eq!(Station::Stern.number(&stock), 2);
        // A wing pair abreast amidships comes between them, port first, whatever the
        // order the yard lists them in.
        let wing = |abeam: f32| GunStation {
            fore: 0.,
            abeam,
            ..stock[0]
        };
        let ship = [stock[0], stock[1], wing(2.5), wing(-2.5)];
        let numbers: Vec<_> = Station::SLOTS[..ship.len()]
            .iter()
            .map(|s| s.number(&ship))
            .collect();
        assert_eq!(numbers, [1, 4, 3, 2]);
    }

    /// Out at sea, clear of the route's harbours and their guns.
    const OFFING: Vec2 = Vec2::new(2000., 2000.);
    fn off(x: f32, z: f32) -> Vec3 {
        Vec3::new(OFFING.x + x, 1.7, OFFING.y + z)
    }
    /// A battle with nothing at sea and no secondaries to steal the main battery's shots,
    /// Breakwater in the offing heading north.
    fn quiet() -> (crate::Battle, Orders) {
        let mut b = crate::Battle::for_route(
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
        b.wing_guns.clear();
        b.aa_mounts.clear();
        b.next_wave = 1000.;
        let orders = Orders {
            player: OFFING,
            heading: 0.,
            speed: 0.,
            hull: crate::Hull::CRUISER,
            aim: Aim::Hold,
            shell: crate::MainShell::STOCK,
            reload: 3.2,
        };
        (b, orders)
    }
    fn fired(b: &crate::Battle) -> Vec<Vec2> {
        (b.effects.iter())
            .filter(|fx| fx.kind == crate::EffectKind::Muzzle(crate::Battery::Main) && fx.age == 0.)
            .map(|fx| fx.body.pos)
            .collect()
    }

    #[test]
    fn an_automatic_turret_fights_a_ship_in_its_arc_and_the_trigger_keeps_the_rest() {
        let (mut b, mut orders) = quiet();
        b.spawn(crate::EnemyClass::Cruiser, off(150., -40.).xz(), OFFING);
        // The sight looks out to port, where the manual stern turret follows it.
        orders.aim = Aim::Track(off(-175., 0.));
        b.direct(&[Control::Auto, Control::Manual], false);
        let mut bow_shots = 0;
        for _ in 0..480 {
            b.raiders[0].has_contact = true;
            b.step(1. / 60., &orders);
            assert_ne!(b.next_barrel().map(|(s, _)| s), Some(Station::Bow));
            let shots = fired(&b);
            assert!(
                shots.iter().all(|p| p.y < OFFING.y),
                "only the bow fires: {shots:?}"
            );
            bow_shots += shots.len();
        }
        let bow = b.main_mount(Station::Bow);
        assert!(bow.crewed() && bow.engaged());
        assert!(bow_shots >= 2, "{bow_shots}");
        assert!(bow.deck_bearing() > 0.5, "trained to starboard");
        let stern = b.main_mount(Station::Stern);
        assert!(!stern.crewed());
        assert_eq!(stern.status(), MountStatus::Ready);
        assert!(
            stern.aim_mark(OFFING, 0.).x < OFFING.x - 100.,
            "laid on the sight to port"
        );
    }

    #[test]
    fn a_turret_on_scope_is_the_triggers_through_the_scope_while_it_bears() {
        let (mut b, mut orders) = quiet();
        let beam = off(175., 0.);
        let astern = off(0., 175.);
        let scope = [Control::Scope, Control::Scope];
        let crewed = |b: &crate::Battle| Station::ALL.map(|s| b.main_mount(s).crewed());

        orders.aim = Aim::Track(beam);
        b.direct(&scope, true);
        b.step(1. / 60., &orders);
        assert_eq!(crewed(&b), [false, false], "both bear on the beam");

        // The bridge stands between the forward turret and the water astern.
        assert!(!b.main_mount(Station::Bow).bears_on(OFFING, 0., astern));
        orders.aim = Aim::Track(astern);
        b.step(1. / 60., &orders);
        assert_eq!(crewed(&b), [true, false]);

        // With the scope down the crew has them both, and with nothing to fight they
        // stay on the sight without firing.
        b.direct(&scope, false);
        for _ in 0..300 {
            b.step(1. / 60., &orders);
            assert!(fired(&b).is_empty());
        }
        assert_eq!(crewed(&b), [true, true]);
        assert!(!b.main_mount(Station::Stern).engaged());
        assert_eq!(b.main_mount(Station::Stern).status(), MountStatus::Ready);
        assert_eq!(b.next_barrel(), None, "the trigger has no turret");
    }

    #[test]
    fn moving_a_deckhouse_opens_a_bearing_and_the_gun_still_answers_for_its_own_berth() {
        use shipvivor_shipyard::{Frame, Module, Ship};
        let mut design = Design::breakwater();
        design.insert(3, Module::new(Frame::Straight, [])).unwrap();
        let mut mount = MainMount::new(design.gun_stations()[0]);
        let orders = Orders {
            player: Vec2::ZERO,
            heading: 0.,
            speed: 0.,
            hull: crate::Hull::breakwater(&design),
            aim: Aim::Hold,
            shell: crate::MainShell::STOCK,
            reload: 3.2,
        };
        let target =
            mount.pivot(Vec2::ZERO, 0.) + (direction(147_f32.to_radians()) * 175.).extend(0.).xzy();
        mount.train(12., 1., &orders, Some(target));
        assert_eq!(mount.status(), MountStatus::Masked);
        design.move_module(3, 2).unwrap();
        mount = MainMount::new(design.gun_stations()[0]);
        mount.train(12., 1., &orders, Some(target));
        assert_eq!(mount.status(), MountStatus::Ready);

        let mut ship = Ship::new(design);
        ship.strike_fitting(mount.position.berth, 20.);
        mount.in_service = ship.ready(mount.position.berth);
        mount.train(1., 1., &orders, Some(target));
        assert_eq!(mount.status(), MountStatus::Disabled);
        assert!(mount.fire(orders.reload, Ammo::He).is_none());
    }
}
