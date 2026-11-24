//! Deck-relative training for Breakwater's mounts, with each barrel loaded on its own.
use super::{Ammo, Orders, Secondaries, TurretLayout, projectile::FlightPath};
use glam::{FloatExt, Vec2, Vec3, Vec3Swizzles};
use shipvivor_compass::direction;
use shipvivor_shipyard::GunStation;
use std::f32::consts::{PI, TAU};

const CRUISER_TRAVERSE: f32 = 18_f32.to_radians();
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
}
impl Station {
    /// The two stations on a stock Breakwater.
    pub const ALL: [Self; 2] = [Self::Bow, Self::Stern];
    pub const SLOTS: [Self; 4] = [Self::Bow, Self::Stern, Self::Third, Self::Fourth];
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
}
impl MainMount {
    pub(super) fn new(position: GunStation) -> Self {
        Self {
            position,
            in_service: true,
            fitted_bore: None,
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
        }
    }
    pub fn deck_bearing(&self) -> f32 {
        self.position.home + self.train
    }
    fn pivot(&self, player: Vec2, heading: f32) -> Vec3 {
        let forward = direction(heading);
        (player + forward * self.position.fore + forward.perp() * self.position.abeam)
            .extend(1.72 + 0.63 * self.scale)
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
    pub fn round(&self, round: super::MainShell) -> super::MainShell {
        super::MainShell {
            bore: self.fitted_bore.unwrap_or(round.bore),
            ..round
        }
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
        let motion_dt = dt * train_rate;
        self.train = self.train.move_towards(stop, CRUISER_TRAVERSE * motion_dt);
        self.pitch = self
            .pitch
            .move_towards(path.pitch, ELEVATION_RATE * motion_dt);
        self.range = (pivot.xz().distance(target.xz())).min(super::projectile::MAIN_RANGE);
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
        let shift = if index / 2 == 2 {
            self.machinery_z
        } else {
            self.bridge_z
        };
        (x * self.beam_scale, fore - shift)
    }
}

/// Metres out from the centreline to each wing gun.
const WING_X: f32 = 2.18;
/// Wing gun stations along each beam in the order they are fitted, in metres forward of
/// amidships: amidships, then beside the bridge, then aft of the launches.
const WING_STATIONS: [f32; Secondaries::MOST_PER_SIDE] = [0., 3.8, -6.9];

/// A shielded close-range gun on the beam, worked by the crew.
pub struct WingGun {
    pub(super) reload: f32,
    side: f32,
    abeam: f32,
    fore: f32,
    train: f32,
}
impl WingGun {
    /// The guns for `per_side` stations a beam, port before starboard at each.
    pub(super) fn battery(per_side: usize) -> Vec<Self> {
        (0..per_side.min(Secondaries::MOST_PER_SIDE) * 2)
            .map(|index| {
                let (x, fore) = Self::station(index);
                Self {
                    side: x.signum(),
                    abeam: x,
                    fore,
                    train: 0.,
                    reload: 0.,
                }
            })
            .collect()
    }
    /// Where the gun at `index` in fitting order stands: metres to starboard and forward.
    pub fn station(index: usize) -> (f32, f32) {
        let side = if index.is_multiple_of(2) { -1. } else { 1. };
        (side * WING_X, WING_STATIONS[index / 2])
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
        self.side * std::f32::consts::FRAC_PI_2 + self.train
    }
    pub(super) fn pivot(&self, player: Vec2, heading: f32) -> Vec3 {
        (player + direction(heading).perp() * self.deck_x() + direction(heading) * self.fore)
            .extend(2.)
            .xzy()
    }

    pub(super) fn covers(&self, player: Vec2, heading: f32, target: Vec2) -> bool {
        let beam = direction(heading).perp() * self.side;
        let delta = target - self.pivot(player, heading).xz();
        delta.dot(beam) >= delta.length() * 4_f32.to_radians().sin()
    }
    pub(super) fn track(
        &mut self,
        dt: f32,
        player: Vec2,
        heading: f32,
        target: Vec2,
    ) -> Option<Vec3> {
        let pivot = self.pivot(player, heading);
        let wanted = signed(
            shipvivor_compass::heading(target - pivot.xz())
                - heading
                - self.side * std::f32::consts::FRAC_PI_2,
        );
        let limit = 86_f32.to_radians();
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
