//! Deck-relative training for Breakwater's mounts, with each barrel loaded on its own.
use super::{
    Gunhouses, MainShell, Orders, Secondaries, TurretLayout,
    projectile::{FlightPath, Trajectory},
};
use glam::{FloatExt, Vec2, Vec3, Vec3Swizzles};
use shipvivor_artillery::ArmorGrade;
use shipvivor_compass::direction;
use shipvivor_shipyard::{Design, Emplacement, GunStation, Obstruction, WingLayout, WingStation};
use std::f32::consts::{PI, TAU};

const CRUISER_TRAVERSE: f32 = shipvivor_artillery::STOCK_TRAVERSE.to_radians();
const ELEVATION_RATE: f32 = 12_f32.to_radians();
const RECOIL: f32 = 0.22;
/// How far past the middle of a blind sector a target must stand before a turret waiting
/// at one stop swings round to the other, so a sight wandering about dead ahead doesn't
/// send it to and fro.
const SIDE_CHANGE: f32 = 6_f32.to_radians();

pub(super) fn signed(angle: f32) -> f32 {
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
    Seventh,
    Eighth,
}
impl Station {
    /// The two stations on a stock Breakwater.
    pub const ALL: [Self; 2] = [Self::Bow, Self::Stern];
    /// Every main turret a hull can carry, as many as a heavy battleship takes.
    pub const SLOTS: [Self; 8] = [
        Self::Bow,
        Self::Stern,
        Self::Third,
        Self::Fourth,
        Self::Fifth,
        Self::Sixth,
        Self::Seventh,
        Self::Eighth,
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
}

#[derive(Clone)]

pub struct MainMount {
    pub position: GunStation,
    pub(super) in_service: bool,
    pub(super) fitted_bore: Option<u16>,
    /// Ranks of training gear fitted to the gunhouse.
    pub(super) gear: u16,
    // Signed displacement from the mount's home stop, in deck coordinates.
    train: f32,
    /// The side of home, -1 or +1, where the mount waits for a target in a blind sector:
    /// the side the target was last clearly on. 0 until it has had one.
    side: f32,
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
            side: 0.,
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
    /// The room the gunhouse takes up on deck at the bore it is fitted with, for the
    /// secondaries beside it to keep their fire clear of.
    pub(super) fn standing(&self) -> Obstruction {
        Obstruction {
            abeam: self.position.abeam,
            fore: self.position.fore,
            radius: TURRET_BERTH * self.round(MainShell::STOCK).mount_scale(),
        }
    }
    pub fn round(&self, round: MainShell) -> MainShell {
        MainShell {
            bore: self.fitted_bore.unwrap_or(round.bore),
            ..round
        }
    }
    /// Whether the round this turret fires next, sent along the flight it is laid on,
    /// would open sound plate of `grade`. A turret not laid can't say, and counts as able.
    pub fn hurts(&self, grade: ArmorGrade) -> bool {
        let (Some(path), Some(_)) = (self.solution, self.next_tube()) else {
            return true;
        };
        self.shell.round_at(path.duration).opens(grade)
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
        let inside_mask = wanted.abs() < self.position.inner_mask;
        // A target in a blind sector draws the mount to the stop on the target's own side,
        // laid ready for it to come clear. Changing sides means a swing back through home
        // the long way, so while the target sits near the middle of the sector the mount
        // keeps to the side it chose.
        let middle = if beyond_stop { PI } else { 0. };
        let undecided = (beyond_stop || inside_mask)
            && self.side != 0.
            && (wanted.abs() - middle).abs() < SIDE_CHANGE;
        if !undecided {
            self.side = if wanted < 0. { -1. } else { 1. };
        }
        let stop = if inside_mask {
            self.side * self.position.inner_mask
        } else if beyond_stop {
            self.side * self.position.half_arc
        } else {
            wanted
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
    pub(super) fn fire(&mut self, reload: f32) -> Option<FlightPath> {
        if self.status != MountStatus::Ready {
            return None;
        }
        let mut path = self.solution?;
        let tube = self.next_tube()?;
        path.offset_muzzle(self.layout.barrel_x(tube) * self.scale);
        self.tubes[tube] = Tube {
            reload_left: reload,
            reload,
            recoil_left: RECOIL,
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
        let shift = |at: Option<usize>, stock_module| {
            at.map_or(0., |index| design.span(index).0 - stock_module as f32 * 5.)
        };
        let carrying =
            |component| design.modules().iter().position(|m| m.fit.contains(&component));
        Self {
            bridge_z: shift(carrying(Component::Director), 2),
            machinery_z: shift(design.machinery(), 3),
            beam_scale: design.beam_factor(),
        }
    }
}

/// Metres round a deck gun's shielded mount that its neighbours' fire keeps clear of, and
/// round a secondary gunhouse of four inches and less, six and eight, each broader than
/// the last.
const WING_BERTH: (f32, f32, f32, f32) = (1.5, 1.75, 1.9, 2.15);
/// Metres round a main turret that the secondaries' fire keeps clear of, for a six-inch
/// gunhouse: its ring and a little over.
const TURRET_BERTH: f32 = 1.5;
/// Share of its loading time each round from a secondary comes early or late by, so a
/// row of them never settles into firing as one.
pub(super) const RAGGED: f32 = 0.18;
/// Share of its loading time a crew with nothing in its arc takes, at most, to open fire
/// when something comes into it. Each crew takes its own time.
pub(super) const STAND_EASY: f32 = 0.5;

/// Where her deck guns and gunhouses stand on `design`, with each number of gunhouses a
/// side, from none to as many as her deck has room for.
pub(super) fn wing_layouts(design: &Design) -> std::sync::Arc<[WingLayout]> {
    (0..=design.gunhouse_room())
        .map(|houses| design.wing_layout(houses))
        .collect()
}

/// The deck guns `fit` has, then the gunhouses `houses` has, where `layout` stands them,
/// as far as it has room. Each gun on deck trains only as far as its fire clears the guns
/// and gunhouses beside it and the main turrets. A casemate's arc is cut by the port in
/// her side, and nothing on deck stands in its way.
pub(super) fn wing_berths(
    layout: &WingLayout,
    fit: Secondaries,
    houses: Gunhouses,
    turrets: &[MainMount],
) -> Vec<WingStation> {
    let deck = &layout.deck[..(2 * fit.per_side).min(layout.deck.len())];
    let gunhouses = &layout.gunhouses[..(2 * houses.per_side).min(layout.gunhouses.len())];
    let house_berth = match houses.bore {
        ..=4 => WING_BERTH.1,
        5..=6 => WING_BERTH.2,
        _ => WING_BERTH.3,
    };
    let berth = |s: &WingStation| match s.emplacement {
        Emplacement::Gunhouse => house_berth,
        _ => WING_BERTH.0,
    };
    let standing: Vec<Obstruction> = turrets.iter().map(MainMount::standing).collect();
    let on_deck: Vec<&WingStation> = deck
        .iter()
        .chain(gunhouses)
        .filter(|s| s.emplacement != Emplacement::Casemate)
        .collect();
    deck.iter()
        .chain(gunhouses)
        .map(|station| {
            if station.emplacement == Emplacement::Casemate {
                return *station;
            }
            let others: Vec<Obstruction> = on_deck
                .iter()
                .filter(|other| !std::ptr::eq(**other, station))
                .map(|other| Obstruction {
                    abeam: other.abeam,
                    fore: other.fore,
                    radius: berth(other),
                })
                .chain(standing.iter().copied())
                .collect();
            shipvivor_shipyard::clear_arcs(std::slice::from_ref(station), 0., &others)[0]
        })
        .collect()
}

/// A shielded close-range gun on the bow or the beam, worked by the crew.
#[derive(Clone)]
pub struct WingGun {
    pub(super) reload: f32,
    abeam: f32,
    fore: f32,
    /// Bearing from the bow it rests on, and how far it trains anticlockwise and
    /// clockwise of that: short of the guns and turrets standing beside it.
    home: f32,
    arc: (f32, f32),
    /// Bearing from `home`.
    train: f32,
    /// Times it has fired, every barrel at once.
    fired: u32,
    /// A deck gun on deck or in a casemate, or a secondary gunhouse.
    emplacement: Emplacement,
    /// Something was in its arc and reach at the last look.
    pub(super) engaged: bool,
}
impl WingGun {
    /// A gun standing at `station`, trained fore and aft and loaded.
    pub(super) fn new(station: WingStation) -> Self {
        Self {
            abeam: station.abeam,
            fore: station.fore,
            home: station.home,
            arc: station.arc,
            train: 0.,
            reload: 0.,
            fired: 0,
            emplacement: station.emplacement,
            engaged: false,
        }
    }
    /// Stand at `station`, where the yard has moved it, keeping its load and as much of
    /// its training as the arc there allows.
    pub(super) fn place(&mut self, station: WingStation) {
        self.abeam = station.abeam;
        self.fore = station.fore;
        self.home = station.home;
        self.arc = station.arc;
        self.train = self.train.clamp(station.arc.0, station.arc.1);
        if self.emplacement != station.emplacement {
            self.emplacement = station.emplacement;
            self.reload = 0.;
        }
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
    /// Times the gun has fired, every barrel at once. It counts on from wherever the
    /// battle was rewound to.
    pub fn fired(&self) -> u32 {
        self.fired
    }
    /// A deck gun on deck or in a casemate, or a secondary gunhouse.
    pub fn emplacement(&self) -> Emplacement {
        self.emplacement
    }
    /// It had something in its arc and reach at the last look. A casemate's crew opens
    /// its port only then.
    pub fn engaged(&self) -> bool {
        self.engaged
    }
    pub(super) fn count_round(&mut self) {
        self.fired = self.fired.wrapping_add(1);
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
        let off = self.off_home(player, heading, target);
        self.arc.0 <= off && off <= self.arc.1
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
        let (least, most) = self.arc;
        self.train = self
            .train
            .move_towards(wanted.clamp(least, most), 68_f32.to_radians() * dt);
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
        assert!(mount.fire(orders.reload).is_none());
        assert!((before - mount.deck_bearing() - CRUISER_TRAVERSE).abs() < 0.001);
        assert_eq!(mount.status(), MountStatus::Training);
        assert!((0..mount.layout().count()).all(|t| mount.loaded(t)));
    }

    #[test]
    fn a_turret_waits_at_the_stop_on_the_targets_side_of_its_blind_sector() {
        let mut mount = MainMount::new(Design::breakwater().gun_stations()[1]);
        assert_eq!(mount.position.home, PI, "the stern turret looks aft");
        let orders = Orders {
            player: Vec2::ZERO,
            heading: 0.,
            speed: 0.,
            hull: crate::Hull::CRUISER,
            aim: Aim::Hold,
            shell: crate::MainShell::STOCK,
            reload: 3.2,
        };
        // Half the width of the sector the bridge hides from it over the bow.
        let blind = PI - mount.position.half_arc;
        assert!(blind > 2. * SIDE_CHANGE, "{}", blind.to_degrees());
        let pivot = mount.pivot(Vec2::ZERO, 0.);
        let off_bow = |bearing: f32| Some(pivot + (direction(bearing) * 175.).extend(0.).xzy());
        let laid = |mount: &MainMount| signed(mount.deck_bearing());

        mount.train(30., 1., &orders, off_bow(-blind / 2.));
        assert_eq!(mount.status(), MountStatus::Masked);
        assert!((laid(&mount) + blind).abs() < 1e-3, "waits on the port bow");

        // The target crosses to the starboard bow, still hidden: the turret goes round by
        // the stern to meet it there.
        mount.train(1., 1., &orders, off_bow(blind / 2.));
        assert!(laid(&mount) < -blind - 0.1, "trains aft: {}", laid(&mount));
        mount.train(60., 1., &orders, off_bow(blind / 2.));
        assert!(
            (laid(&mount) - blind).abs() < 1e-3,
            "waits on the starboard bow"
        );

        // A sight wandering a little across dead ahead leaves it where it is.
        mount.train(30., 1., &orders, off_bow(-SIDE_CHANGE / 2.));
        assert!((laid(&mount) - blind).abs() < 1e-3);
        // It comes clear on the port bow, and the turret swings back round to bear.
        mount.train(30., 1., &orders, off_bow(-blind - 0.2));
        assert_eq!(mount.status(), MountStatus::Ready);
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
        assert!(mount.fire(orders.reload).is_some());
        assert_eq!(mount.status(), MountStatus::Ready);
        assert_eq!(mount.next_tube(), Some(1));
        assert!(mount.recoil(0) > 0.99 && mount.recoil(1) == 0.);
        mount.train(1., 1., &orders, ahead);
        assert!(mount.fire(orders.reload).is_some());
        assert_eq!(mount.status(), MountStatus::Loading);
        assert!(mount.fire(orders.reload).is_none());
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
        assert!(mount.fire(orders.reload).is_none());
    }
}
