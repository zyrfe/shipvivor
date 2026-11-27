use super::IndustrialYard;
use super::{
    Battery, Body, Effect, EnemyClass, HarbourSite, Shot, Vec2, bearing, body, entry,
    mounts::train_full,
    projectile::{FlightPath, lead},
    ranging::{HEAVY_RANGE, Ranging},
};
use glam::Vec3Swizzles;

pub const ISLAND_RADIUS: f32 = 26.;
pub const SPAWN_STANDOFF: f32 = 275.;
pub const SHORE_RANGE: f32 = 320.;

pub const YARD_CAPACITY: u8 = 12;
const DOCK_STRENGTH: u16 = 24;
const FACTORY_STRENGTH: u16 = 8;
/// Seconds one standing factory takes to raise a fallen drydock again. Two take half as
/// long, three a third. With no factory standing, a fallen dock stays down.
pub const REBUILD_SECONDS: f32 = 75.;
/// Metres off the dock house's line, outboard, where a hull it launches swims.
const BERTH_OUTBOARD: f32 = 8.;
/// Where each drydock's house stands, starboard and forward of the harbour centre. The
/// first is every base's. The second stands to port of it with room for both slips.
pub const DOCK_STATIONS: [(f32, f32); 2] = [(6., 4.), (-9., 4.)];

/// Drydocks at a base with `works` factories: the cannery has one, the twin mills and
/// the heavy yard two.
pub fn dock_count(works: usize) -> usize {
    if works >= 2 { 2 } else { 1 }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum YardState {
    Working,
    Damaged,
    Burning,
    Ruined,
}

pub struct Factory {
    strength: u16,
}
impl Factory {
    pub fn live(&self) -> bool {
        self.strength > 0
    }
}

/// One drydock and its slip. Each lays down its own hull and launches it, and when it
/// falls the base's factories set about raising it again.
pub struct Dock {
    /// Starboard and forward of the harbour centre, in the port's rotating frame: where
    /// the dock house stands. Its slip and quay run straight out ahead of it.
    station: Vec2,
    strength: u16,
    order: (EnemyClass, u8),
    /// Steel laid into the hull on the slip, of the `order`'s bill.
    laid: u8,
    pub(super) slipway_clock: f32,
    /// The whole turn the slip was last set to fit out, for how far along it is.
    slipway_turn: f32,
    /// How far the factories have raised it again since it fell, from 0 to 1.
    rebuilt: f32,
}
impl Dock {
    fn new(side: f32, fore: f32, slip: f32) -> Self {
        Self {
            station: Vec2::new(side, fore),
            strength: DOCK_STRENGTH,
            order: (EnemyClass::Patrol, 2),
            laid: 0,
            slipway_clock: slip,
            slipway_turn: slip,
            rebuilt: 0.,
        }
    }
    pub fn live(&self) -> bool {
        self.strength > 0
    }
    pub fn strength(&self) -> u16 {
        self.strength
    }
    pub fn state(&self) -> YardState {
        match self.strength {
            0 => YardState::Ruined,
            1..=8 => YardState::Burning,
            9..=16 => YardState::Damaged,
            _ => YardState::Working,
        }
    }
    /// The height of the gantry, for rounds and sightlines. It drops with each stage of
    /// damage and leaves rubble when the dock falls.
    pub fn top(&self) -> f32 {
        match self.state() {
            YardState::Working => 20.,
            YardState::Damaged => 15.,
            YardState::Burning => 9.,
            YardState::Ruined => 4.5,
        }
    }
    /// The hull on the slip and its bill of steel. The slip keeps the order even when
    /// the bins are too empty to launch it.
    pub fn order(&self) -> (EnemyClass, u8) {
        self.order
    }
    /// Steel laid into the hull on the slip so far.
    pub fn laid(&self) -> u8 {
        self.laid
    }
    /// Seconds before the slip has fitted out the last hull and can launch the next.
    pub fn slipway_clock(&self) -> f32 {
        self.slipway_clock
    }
    /// How far the hull on the slip is toward launching, from 0 to 1: the lesser of its
    /// steel and the slip's fitting out. At 1 she goes as soon as the water is clear.
    pub fn progress(&self) -> f32 {
        if !self.live() {
            return 0.;
        }
        let steel = f32::from(self.laid) / f32::from(self.order.1.max(1));
        let slip = if self.slipway_turn > 0. {
            1. - self.slipway_clock / self.slipway_turn
        } else {
            1.
        };
        steel.min(slip).clamp(0., 1.)
    }
    /// Funded and fitted out: she launches as soon as the water off the slip is clear.
    pub fn ready(&self) -> bool {
        self.live() && self.laid >= self.order.1 && self.slipway_clock <= 0.
    }
    /// How far a fallen dock has been raised again, from 0 to 1, or `None` while it
    /// stands.
    pub fn rebuilding(&self) -> Option<f32> {
        (!self.live()).then_some(self.rebuilt)
    }
    fn fall(&mut self) {
        self.strength = 0;
        self.laid = 0;
        self.rebuilt = 0.;
        self.slipway_clock = 0.;
        self.slipway_turn = 0.;
    }
}

/// Where a round struck a gunhouse and left it standing, so the mark turns with it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pock {
    /// Metres to starboard of the mount's centre, up from the water, and ahead along its
    /// barrels, in the gunhouse's own frame.
    pub at: glam::Vec3,
    /// Inches across the round that struck.
    pub calibre: f32,
}

/// A single independently trained, damageable mount on the harbour apron.
pub struct ShoreTurret {
    // Starboard and forward of the harbour centre, in the port's rotating frame.
    station: Vec2,
    pub(super) elevation: f32,
    pub(super) deployment: Option<f32>,
    pub barrels: u8,
    pub heading: f32,
    pub(super) strength: u16,
    pub(super) reload: f32,
    /// A heavy battery's plot of her. `None` on a light mount.
    pub(super) ranging: Option<Ranging>,
    /// Its place on the apron, which with the base sets the run of its laying errors.
    index: u8,
    /// Salvos a light mount has fired, which draws each one's error afresh.
    salvos: u32,
    /// Rounds that struck the gunhouse and left it standing, oldest first.
    pocks: Vec<Pock>,
}
impl ShoreTurret {
    const STRENGTH_PER_BARREL: u16 = 6;
    /// A heavy gunhouse stands half as much again per barrel, and is drawn this much
    /// larger than a light one.
    const HEAVY_STRENGTH: u16 = 9;
    const HEAVY_SCALE: f32 = 1.6;
    /// What each light shell does on a hit, from a single gun and from each barrel of a
    /// twin or more, before plate.
    const DAMAGE: (u16, u16) = (11, 8);
    /// Seconds a light mount takes to load, and the more for each barrel past the first.
    const RELOAD: (f32, f32) = (5., 0.8);
    /// Metres a light mount's salvo is laid off her, along the line of fire and across
    /// it, at point blank and at its full reach. The layers' error grows with the square
    /// of the range, so at the edge of its reach a mount scatters its rounds about her length and
    /// often misses, while one close aboard seldom does.
    const LAY_ALONG: (f32, f32) = (1., 30.);
    const LAY_ACROSS: (f32, f32) = (1., 12.);
    /// Metres each shell of a salvo strays from where the salvo was laid, at full reach.
    const STRAY: (f32, f32) = (5., 1.5);
    /// The most marks a gunhouse keeps. Past this the oldest weather away.
    const POCKS: usize = 14;
    pub(super) fn new(side: f32, fore: f32, barrels: u8, heading: f32, index: usize) -> Self {
        Self {
            station: Vec2::new(side, fore),
            barrels,
            elevation: 0.,
            deployment: None,
            heading,
            strength: Self::STRENGTH_PER_BARREL * u16::from(barrels),
            reload: 2. + 1.6 * index as f32,
            ranging: None,
            index: index.min(255) as u8,
            salvos: 0,
            pocks: Vec::new(),
        }
    }
    /// Rounds that struck the gunhouse and left it standing, oldest first.
    pub fn pocks(&self) -> &[Pock] {
        &self.pocks
    }
    /// Take a round of `calibre` inches at `point` on the gunhouse, standing at `at`.
    pub(super) fn pock(&mut self, at: Vec2, point: glam::Vec3, calibre: f32) {
        let ahead = shipvivor_compass::direction(self.heading);
        let offset = point.xz() - at;
        let local = glam::Vec3::new(offset.dot(ahead.perp()), point.y, offset.dot(ahead));
        if self.pocks.len() >= Self::POCKS {
            self.pocks.remove(0);
        }
        self.pocks.push(Pock { at: local, calibre });
    }
    /// A heavy battery of two long guns, which ranges on her from far off.
    pub(super) fn heavy(side: f32, fore: f32, heading: f32, index: usize) -> Self {
        let mut gun = Self::new(side, fore, 2, heading, index);
        gun.ranging = Some(Ranging::default());
        gun.strength = gun.max_strength();
        gun
    }
    /// One of a sector's heavy batteries, rather than a light mount.
    pub fn heavy_battery(&self) -> bool {
        self.ranging.is_some()
    }
    /// A heavy battery's plot of her, or `None` on a light mount.
    pub fn ranging(&self) -> Option<&Ranging> {
        self.ranging.as_ref()
    }
    /// Metres the mount can fire to.
    pub fn reach(&self) -> f32 {
        if self.cliff_battery() {
            375.
        } else if self.heavy_battery() {
            HEAVY_RANGE
        } else {
            SHORE_RANGE
        }
    }
    /// How much larger than a light mount the gunhouse is drawn and stands.
    pub fn mount_scale(&self) -> f32 {
        if self.heavy_battery() {
            Self::HEAVY_SCALE
        } else {
            1.
        }
    }
    /// Metres from the mount's centre to the edge of its apron, and the height of the
    /// gunhouse roof, for rounds and sightlines.
    pub fn radius(&self) -> f32 {
        5. * self.mount_scale().sqrt()
    }
    /// Height a gunner lays on to put a round through the gunhouse.
    pub fn aim_height(&self) -> f32 {
        self.elevation + 3.9 + 1.8 * self.mount_scale()
    }

    pub fn elevation(&self) -> f32 {
        self.elevation
    }

    pub fn cliff_battery(&self) -> bool {
        self.deployment.is_some()
    }

    pub fn deployment(&self) -> f32 {
        self.deployment.unwrap_or(1.)
    }

    pub fn top(&self) -> f32 {
        self.elevation
            + if self.heavy_battery() {
                3.9 + 3.6 * Self::HEAVY_SCALE
            } else {
                7.5
            }
    }
    /// Mount no more than `most` barrels, with the strength of the smaller gunhouse.
    fn cap(&mut self, most: u8) {
        if self.barrels > most.max(1) {
            self.barrels = most.max(1);
            self.strength = self.strength.min(self.max_strength());
        }
    }
    pub fn strength(&self) -> u16 {
        self.strength
    }
    pub fn max_strength(&self) -> u16 {
        let each = if self.heavy_battery() {
            Self::HEAVY_STRENGTH
        } else {
            Self::STRENGTH_PER_BARREL
        };
        each * u16::from(self.barrels)
    }
    pub fn live(&self) -> bool {
        self.strength > 0 && self.deployment() >= 1.
    }
    pub fn position(&self, base: &Body) -> Vec2 {
        let f = base.forward();
        base.pos + f.perp() * self.station.x + f * self.station.y
    }
    /// Space the modelled tubes and their muzzle flashes on the same trunnion line.
    pub fn barrel_x(barrels: u8, tube: u8) -> f32 {
        (f32::from(tube) - 0.5 * (f32::from(barrels) - 1.)) * 1.30
    }
    pub(super) fn covers(&self, base: &Body, target: Vec2) -> bool {
        let at = self.position(base);
        self.live()
            && at.distance_squared(target) <= self.reach() * self.reach()
            && (self.elevation > 0. || entry(at, target, base.pos, ISLAND_RADIUS).is_none())
    }
    fn fire(
        &mut self,
        dt: f32,
        base: &Body,
        player: Option<Vec2>,
        way: Vec2,
        mut launch: impl FnMut(FlightPath, u16),
    ) {
        // A heavy battery lays its own salvos: see `ranging`.
        if !self.live() || self.heavy_battery() {
            return;
        }
        self.reload = (self.reload - dt).max(0.);
        let Some(player) = player else {
            return;
        };
        let at = self.position(base);
        let ahead = lead(at, player, way, Battery::Shore);
        let Some(dir) = (ahead - at).try_normalize() else {
            return;
        };
        let trained = train_full(&mut self.heading, bearing(dir), 11_f32.to_radians() * dt);
        if !trained || self.reload > 0. || !self.covers(base, player) {
            return;
        }
        let side = dir.perp();
        let muzzle_line = at + dir * 8.25;
        let damage = if self.barrels == 1 {
            Self::DAMAGE.0
        } else {
            Self::DAMAGE.1
        };
        // The whole salvo is laid off by the same error, worse the farther she is.
        let reach = (at.distance(ahead) / self.reach()).clamp(0., 1.).powi(2);
        let seed = base.id.wrapping_mul(31).wrapping_add(u64::from(self.index));
        let salvo = self.salvos;
        let spread = |draw: u64| super::ranging::spread(seed, salvo, draw);
        let grow = |(near, far): (f32, f32)| near + (far - near) * reach;
        let laid = ahead
            + dir * grow(Self::LAY_ALONG) * spread(0)
            + side * grow(Self::LAY_ACROSS) * spread(1);
        for tube in 0..self.barrels {
            let offset = Self::barrel_x(self.barrels, tube);
            let muzzle = (muzzle_line + side * offset)
                .extend(6.65 + self.elevation)
                .xzy();
            // Outer rounds bracket the contact, leaving space to manoeuvre between them.
            let draw = 2 + 2 * u64::from(tube);
            let stray = dir * Self::STRAY.0 * reach * spread(draw)
                + side * Self::STRAY.1 * reach * spread(draw + 1);
            // Laid on her side below the deck, so a round a little over still strikes.
            let target = (laid + side * (offset * 4.) + stray).extend(1.).xzy();
            if let Some(path) = FlightPath::from_barrel(muzzle, target, Battery::Shore, 0.) {
                launch(path, damage);
            }
        }
        self.salvos = self.salvos.wrapping_add(1);
        self.reload = Self::RELOAD.0 + Self::RELOAD.1 * f32::from(self.barrels - 1);
    }
}

pub struct Port {
    pub(super) industry: Option<IndustrialYard>,
    pub(super) collapse: Option<f32>,
    pub has_contact: bool,
    /// Steel in the storehouse, beyond what is laid into the hulls on the slips.
    pub stock: u8,
    factories: Vec<Factory>,
    docks: Vec<Dock>,
    // Unfinished fraction of the next unit of material. A full storehouse stops production.
    material_progress: f64,
    tier: usize,
    order_index: usize,
    /// What holds the waters, which decides what the yard lays down at each tier.
    pub(super) doctrine: super::Doctrine,
    pub(super) convoy_clock: f32,
    /// Seconds until the yard sends out its next group of coasters.
    pub(super) coaster_clock: f32,
    /// The group of coasters putting out: the yard they ply to, and how many are still
    /// to leave the slip.
    pub(super) coasters_due: Option<(usize, u8)>,
    pub body: Body,
    pub(super) turrets: Vec<ShoreTurret>,
    /// The stage of the passage the base belongs to.
    stage: u8,
    /// Stirred to the fight. Until then the yard holds its steel and its slip, its
    /// floatplane stays on the water and its guns stay quiet.
    pub(super) roused: bool,
    /// Seconds into the sector when it was roused.
    roused_at: f32,
    /// Has sighted the ship or been hit since the last look round, which rouses it.
    pub(super) alarm: bool,
    /// The airfield beside the harbour, where the base is one: its runway, shelters,
    /// hangars and tower stand on a flat island of their own, and its factories with
    /// them. Its drydock and quay stay on the harbour island.
    pub(super) airfield: Option<Box<super::Airfield>>,
}
impl Port {
    /// The classic layout: one harbour outside each of three gates, facing the fairway.
    pub(crate) fn classic_sites(gates: [Vec2; 5]) -> [HarbourSite; 3] {
        let center = gates.iter().sum::<Vec2>() / gates.len() as f32;
        [(1, 1), (2, 2), (4, 3)].map(|(i, works)| {
            let outward = (gates[i] - center).normalize();
            HarbourSite {
                pos: gates[i] + outward * 105.,
                heading: bearing(-outward),
                works,
                stage: 0,
            }
        })
    }
    /// A cannery has one factory, one gun and one drydock, the twin mills two of each,
    /// and the heavy yard three factories, three heavy mounts and two drydocks. The
    /// second dock stands to port of the first, with its own slip and quay, which moves
    /// the forward gun out to the shingle's edge. The light bases send their first
    /// convoys soonest, and a second base of the same kind a little after the first.
    pub(crate) fn chart(sites: &[HarbourSite], wave_interval: f32, serial: &mut u64) -> Vec<Self> {
        let mut kindred = [0_u8; 3];
        sites
            .iter()
            .map(
                |&HarbourSite {
                     pos,
                     heading,
                     works,
                     stage,
                 }| {
                    // An airfield's harbour is laid out as a cannery's: one dock and one
                    // gun. Its two factories stand on the airfield, with its other guns.
                    let field = works == super::AIRFIELD_WORKS;
                    let industry = IndustrialYard::from_works(works);
                    let built = if field {
                        2
                    } else {
                        usize::from(works.clamp(1, 3))
                    };
                    let works = if field { 1 } else { built };
                    let convoy_clock =
                        [25., 38., 64.][works - 1] + 7. * f32::from(kindred[works - 1]);
                    kindred[works - 1] += 1;
                    let stations: &[(f32, f32, u8)] = match works {
                        1 => &[(-14., 23., 1)],
                        2 => &[(-19.5, 23., 2), (16., -22., 2)],
                        _ => &[(-19.5, 23., 4), (23., -17.5, 3), (-16., -22., 3)],
                    };
                    let first_slip = works as f32 * wave_interval;
                    let docks = DOCK_STATIONS[..dock_count(works)]
                        .iter()
                        .enumerate()
                        .map(|(i, &(side, fore))| {
                            Dock::new(side, fore, first_slip * (1. + 0.5 * i as f32))
                        })
                        .collect();
                    Self {
                        body: body(serial, pos, heading),
                        industry,
                        collapse: None,
                        turrets: if industry.is_some() {
                            super::industrial::batteries(heading)
                        } else {
                            stations
                                .iter()
                                .enumerate()
                                .map(|(i, &(side, fore, barrels))| {
                                    ShoreTurret::new(
                                        side,
                                        fore,
                                        barrels,
                                        heading + side.atan2(fore),
                                        i,
                                    )
                                })
                                .collect()
                        },
                        has_contact: false,
                        stock: 2,
                        factories: (0..built)
                            .map(|_| Factory {
                                strength: FACTORY_STRENGTH,
                            })
                            .collect(),
                        docks,
                        material_progress: 0.,
                        tier: 0,
                        order_index: 0,
                        doctrine: super::Doctrine::Mixed,
                        convoy_clock,
                        coaster_clock: super::COASTER_INTERVAL,
                        coasters_due: None,
                        stage,
                        roused: stage == 0,
                        roused_at: 0.,
                        alarm: false,
                        airfield: None,
                    }
                },
            )
            .collect()
    }

    /// The stage of the passage the base belongs to.
    pub fn stage(&self) -> u8 {
        self.stage
    }
    /// Stirred to the fight, and shown on the chart. A base sleeps until every base of
    /// the stage before it is wrecked, or until it sights the ship or is shelled.
    pub fn roused(&self) -> bool {
        self.roused
    }
    /// Seconds into the sector when the base was roused, or `None` while it sleeps. The
    /// first stage is roused from the start.
    pub fn roused_at(&self) -> Option<f32> {
        self.roused.then_some(self.roused_at)
    }
    pub(super) fn rouse(&mut self, at: f32) {
        if !self.roused {
            self.roused = true;
            self.roused_at = at;
        }
    }

    /// A point in the port's rotating frame: starboard and forward of the centre.
    pub(super) fn local(&self, station: Vec2) -> Vec2 {
        let f = self.body.forward();
        self.body.pos + f.perp() * station.x + f * station.y
    }
    pub fn docks(&self) -> &[Dock] {
        &self.docks
    }
    pub fn live_docks(&self) -> usize {
        self.docks.iter().filter(|dock| dock.live()).count()
    }
    /// Where drydock `index`'s house stands.
    pub fn dock_position(&self, index: usize) -> Vec2 {
        self.local(self.docks[index].station)
    }
    pub fn dock_top(&self, index: usize) -> f32 {
        self.docks[index].top()
    }
    pub(super) fn dock_span(&self, index: usize, a: Vec2, b: Vec2) -> Option<(f32, f32)> {
        self.building_span(self.dock_position(index), (5.5, 7.), a, b)
    }
    /// Every drydock down and every factory with it: nothing is left to build a ship or
    /// raise a dock again, and the base counts as destroyed. Its guns are their own
    /// matter and may fight on.
    pub fn wrecked(&self) -> bool {
        self.live_docks() == 0
            && self.live_factories() == 0
            && self.airfield.as_ref().is_none_or(|field| field.live_hangars() == 0)
    }
    /// The airfield the base works, if it is one.
    pub fn airfield(&self) -> Option<&super::Airfield> {
        self.airfield.as_deref()
    }
    /// The tallest structure still standing and its height, from which the base keeps
    /// its lookout: a drydock's gantry, else a factory, else a gun. `None` once nothing
    /// stands.
    pub fn lookout(&self) -> Option<(Vec2, f32)> {
        let docks = (0..self.docks.len())
            .filter(|&i| self.docks[i].live())
            .map(|i| (self.dock_position(i), self.dock_top(i)));
        let factories = (0..self.production_works().len())
            .filter(|&i| self.factories[i].live())
            .map(|i| (self.factory_position(i), self.factory_top(i)));
        let guns = (self.turrets.iter())
            .filter(|g| g.live())
            .map(|g| (g.position(&self.body), g.top()));
        docks
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .or_else(|| factories.max_by(|a, b| a.1.total_cmp(&b.1)))
            .or_else(|| guns.max_by(|a, b| a.1.total_cmp(&b.1)))
    }

    pub fn factories(&self) -> &[Factory] {
        &self.factories
    }
    /// The portion of the factory target list that keeps this base operating.
    pub fn production_works(&self) -> &[Factory] {
        if self.industry.is_some() {
            &self.factories[..IndustrialYard::FUEL_TANK]
        } else {
            &self.factories
        }
    }
    /// Working factories or governors that can supply steel and raise a lost dock.
    /// Fuel storage is a separate target and makes no steel.
    pub fn live_factories(&self) -> usize {
        self.production_works()
            .iter()
            .filter(|factory| factory.live())
            .count()
    }
    pub fn factory_position(&self, index: usize) -> Vec2 {
        // The twin mills occupy the same shingle bank, with a conveyor between them.
        // At the heavy yard the three furnace halls take separate faces of the shore.
        if let Some(industry) = self.industry {
            return self.local(industry.factory_station(index));
        }
        if let Some(field) = &self.airfield {
            return field.factory_position(index);
        }
        let sites: &[(f32, f32)] = match self.factories.len() {
            1 => &[(-20.5, -7.)],
            2 => &[(19.5, -9.), (19.5, 9.)],
            _ => &[(-20.5, -7.), (19.5, 7.), (6., 20.)],
        };
        let (side, aft) = sites[index];
        let f = self.body.forward();
        self.body.pos + f.perp() * side - f * aft
    }
    pub fn factory_bottom(&self, index: usize) -> f32 {
        if self.industry.is_some()
            && index < IndustrialYard::FUEL_TANK
            && self.factories[index].live()
        {
            6.
        } else {
            3.2
        }
    }

    pub fn factory_top(&self, index: usize) -> f32 {
        if self.industry.is_some() {
            return match (
                index == IndustrialYard::FUEL_TANK,
                self.factories[index].live(),
            ) {
                (true, true) => 15.2,
                (true, false) => 6.9,
                (false, true) => 11.6,
                (false, false) => 7.2,
            };
        }
        if self.factories[index].live() {
            12.2
        } else {
            4.5
        }
    }
    pub(super) fn factory_span(&self, index: usize, a: Vec2, b: Vec2) -> Option<(f32, f32)> {
        if self.industry.is_some() {
            let radius = if index == IndustrialYard::FUEL_TANK {
                6.
            } else {
                2.9
            };
            super::footprint(a, b, self.factory_position(index), radius)
        } else {
            self.building_span(self.factory_position(index), (4., 4.), a, b)
        }
    }
    pub(super) fn damage_factory(&mut self, index: usize, damage: u16) -> bool {
        let factory = &mut self.factories[index];
        let was_live = factory.live();
        factory.strength = factory.strength.saturating_sub(damage);
        let fell = was_live && !factory.live();
        if fell {
            if self.industry.is_some()
                && index < IndustrialYard::FUEL_TANK
                && self.live_factories() == 0
            {
                self.collapse = Some(0.);
            }
            self.settle();
        }
        fell
    }
    /// Steel a minute. Each standing drydock makes a little of its own, and each standing
    /// factory three, unless a dock is down: then the factories' work goes into raising
    /// it, and only the docks still standing make steel.
    pub fn production_per_minute(&self) -> f32 {
        let docks = self.live_docks() as f32;
        if docks == 0. || self.raising() {
            docks
        } else {
            docks + 3. * self.live_factories() as f32
        }
    }
    /// A drydock is down and a factory stands to raise it.
    pub fn raising(&self) -> bool {
        self.live_factories() > 0 && self.live_docks() < self.docks.len()
    }
    /// Seconds until the factories have raised the fallen dock nearest done, at their
    /// present strength. `None` while every dock stands or no factory does.
    pub fn rebuild_eta(&self) -> Option<f32> {
        let factories = self.live_factories() as f32;
        let nearest = (self.docks.iter())
            .filter_map(Dock::rebuilding)
            .max_by(f32::total_cmp)?;
        (factories > 0.).then(|| (1. - nearest) * REBUILD_SECONDS / factories)
    }

    fn next_order(&self) -> (EnemyClass, u8) {
        let rotation = (self.doctrine).rotation(self.tier, self.factories.len());
        rotation[self.order_index % rotation.len()]
    }
    pub(super) fn work(&mut self, dt: f32, tier: usize) {
        if self.wrecked() {
            return;
        }
        if self.tier != tier {
            self.tier = tier;
            self.order_index = 0;
        }
        for dock in self.docks.iter_mut().filter(|dock| dock.live()) {
            dock.slipway_clock = (dock.slipway_clock - dt).max(0.);
        }
        // The factories raise one fallen dock at a time, the one furthest along first.
        let hands = self.live_factories() as f32;
        let fallen = (0..self.docks.len())
            .filter(|&i| !self.docks[i].live())
            .max_by(|&a, &b| self.docks[a].rebuilt.total_cmp(&self.docks[b].rebuilt));
        if hands > 0.
            && let Some(i) = fallen
        {
            self.docks[i].rebuilt += dt * hands / REBUILD_SECONDS;
            if self.docks[i].rebuilt >= 1. {
                // Raised again, it lays down whatever the yard builds now.
                let order = self.next_order();
                self.order_index = self.order_index.wrapping_add(1);
                let dock = &mut self.docks[i];
                dock.rebuilt = 0.;
                dock.strength = DOCK_STRENGTH;
                dock.order = order;
            }
        }
        let supply =
            self.material_progress + f64::from(dt) * f64::from(self.production_per_minute()) / 60.;
        self.stock = self.stock.saturating_add(supply as u8).min(YARD_CAPACITY);
        self.material_progress = if self.stock < YARD_CAPACITY {
            supply.fract()
        } else {
            0.
        };
        self.fit_out();
    }
    /// Steel goes from the storehouse into the hulls on the slips, a plate at a time to
    /// whichever standing hull is nearest its bill, so one is finished before the next.
    pub(super) fn fit_out(&mut self) {
        while self.stock > 0 {
            let Some(dock) = (self.docks.iter_mut())
                .filter(|dock| dock.live() && dock.laid < dock.order.1)
                .min_by_key(|dock| dock.order.1 - dock.laid)
            else {
                break;
            };
            dock.laid += 1;
            self.stock -= 1;
        }
    }
    pub(super) fn launch(&mut self, index: usize, next_slip: f32) {
        // A tier unlock changes what the yard lays down after this launch. The hull
        // already on the slip keeps its shape, bill and accumulated material.
        let order = self.next_order();
        self.order_index = self.order_index.wrapping_add(1);
        let dock = &mut self.docks[index];
        dock.laid = 0;
        dock.order = order;
        dock.slipway_clock = next_slip;
        dock.slipway_turn = next_slip;
        self.fit_out();
    }
    /// Clear every slip's fitting out, so each launches as soon as it is funded.
    #[cfg(test)]
    pub(crate) fn clear_slips(&mut self) {
        for dock in &mut self.docks {
            dock.slipway_clock = 0.;
        }
    }
    /// Strip the steel from every hull on the slips.
    #[cfg(test)]
    pub(crate) fn empty_hulls(&mut self) {
        for dock in &mut self.docks {
            dock.laid = 0;
        }
    }
    pub(super) fn building_span(
        &self,
        center: Vec2,
        (along, across): (f32, f32),
        a: Vec2,
        b: Vec2,
    ) -> Option<(f32, f32)> {
        let f = self.body.forward();
        let mut enter = 0_f32;
        let mut leave = 1_f32;
        for (axis, half) in [(f, along), (f.perp(), across)] {
            let start = (a - center).dot(axis);
            let delta = (b - a).dot(axis);
            if delta.abs() < 1e-6 {
                if start.abs() > half {
                    return None;
                }
            } else {
                let t0 = (-half - start) / delta;
                let t1 = (half - start) / delta;
                enter = enter.max(t0.min(t1));
                leave = leave.min(t0.max(t1));
            }
        }
        (enter <= leave).then_some((enter, leave))
    }
    pub(super) fn needs_delivery(&self) -> bool {
        !self.wrecked()
            && (self.stock < YARD_CAPACITY
                || self
                    .docks
                    .iter()
                    .any(|d| d.live() && d.strength < DOCK_STRENGTH)
                || self.turrets.iter().any(|g| g.strength < g.max_strength()))
    }
    /// Shell drydock `index`. True if this blow brought it down: the hull on its slip is
    /// lost with it.
    pub(super) fn damage_dock(&mut self, index: usize, damage: u16) -> bool {
        let dock = &mut self.docks[index];
        let was_live = dock.live();
        dock.strength = dock.strength.saturating_sub(damage);
        let fell = was_live && !dock.live();
        if fell {
            dock.fall();
            self.settle();
        }
        fell
    }
    /// Once the base is wrecked its storehouse is lost and its works stop for good.
    pub(super) fn settle(&mut self) {
        if self.wrecked() {
            self.stock = 0;
            self.material_progress = 0.;
        }
    }
    /// Bring down every drydock and every factory, leaving the guns standing.
    #[cfg(test)]
    pub(crate) fn wreck(&mut self) {
        for i in 0..self.docks.len() {
            self.damage_dock(i, u16::MAX);
        }
        for i in 0..self.factories.len() {
            self.damage_factory(i, u16::MAX);
        }
    }
    /// Silence every gun on the apron.
    #[cfg(test)]
    pub(crate) fn silence(&mut self) {
        for gun in &mut self.turrets {
            gun.strength = 0;
        }
    }
    pub(super) fn unload(&mut self) {
        if self.wrecked() {
            return;
        }
        self.stock = (self.stock + 6).min(YARD_CAPACITY);
        // The delivery's yard stores go to the worst-hit dock still standing.
        if let Some(dock) = (self.docks.iter_mut())
            .filter(|d| d.live())
            .min_by_key(|d| d.strength)
        {
            dock.strength = (dock.strength + 8).min(DOCK_STRENGTH);
        }
        // One delivery carries three points of spares across the whole battery.
        let mut spares = 3;
        for gun in &mut self.turrets {
            let mend = spares.min(gun.max_strength() - gun.strength);
            gun.strength += mend;
            spares -= mend;
            if spares == 0 {
                break;
            }
        }
        self.fit_out();
    }
    pub fn turrets(&self) -> &[ShoreTurret] {
        &self.turrets
    }
    /// Mount a heavy battery on the outer apron, off the beam clear of the factories: to
    /// starboard at the heavy yard and the cannery, and to port at the mills.
    pub(super) fn mount_heavy_battery(&mut self) {
        let (side, fore) = if self.factories.len() == 2 {
            (-26., 10.)
        } else {
            (26., 10.)
        };
        let heading = self.body.heading + f32::atan2(side, fore);
        let index = self.turrets.len();
        self.turrets
            .push(ShoreTurret::heavy(side, fore, heading, index));
    }
    /// Arm every gun on the apron with no more than `most` barrels.
    pub(super) fn cap_turrets(&mut self, most: u8) {
        for gun in &mut self.turrets {
            gun.cap(most);
        }
    }

    pub(super) fn fire_turrets(
        &mut self,
        dt: f32,
        player: Option<Vec2>,
        way: Vec2,
        shots: &mut Vec<Shot>,
        effects: &mut Vec<Effect>,
        serial: &mut u64,
    ) {
        for gun in &mut self.turrets {
            gun.fire(dt, &self.body, player, way, |path, damage| {
                path.launch(shots, effects, serial, Battery::Shore, damage);
            });
        }
    }

    /// The shelf the yard stands on, then the bluff behind it in tiers that step up its
    /// dome as the coast draws it, peaking near 9.6 m and back to the shelf 12 m out. A
    /// round clears the hill wherever the eye sees over it. Centre, radius and top.
    pub(super) fn land_volumes(&self) -> impl Iterator<Item = (Vec2, f32, f32)> + '_ {
        let c = self.body.pos;
        let f = self.body.forward();
        let bluff = c - f * 7. - f.perp() * 5.;
        [
            (c, ISLAND_RADIUS, 3.2),
            (bluff, 9., 4.8),
            (bluff, 7., 6.2),
            (bluff, 5., 7.4),
            (bluff, 3., 8.6),
        ]
        .into_iter()
        .chain(self.industrial_rocks())
    }

    pub(super) fn shores(&self) -> impl Iterator<Item = (Vec2, f32)> + '_ {
        std::iter::once((self.body.pos, ISLAND_RADIUS))
            .chain(self.industrial_rocks().map(|(at, radius, _)| (at, radius)))
    }

    /// The quay's root and outer end along the slip, and its half-width. The foundry's
    /// occupied berth has wider quays ending closer to shore.
    fn quay(&self) -> (f32, f32, f32) {
        if self.factories.len() == 3 {
            (20., 30., 4.8)
        } else {
            (20., 44., 3.)
        }
    }

    /// A circle round each quay, for ships steering clear of it: centre and radius.
    pub(super) fn piers(&self) -> impl Iterator<Item = (Vec2, f32)> + '_ {
        let (root, end, width) = self.quay();
        self.docks.iter().map(move |dock| {
            let centre = self.local(Vec2::new(dock.station.x, (root + end) * 0.5));
            (centre, (end - root) * 0.5 + width)
        })
    }

    /// A ship appears in the water alongside the outer end of the main quay.
    pub fn berth(&self) -> Vec2 {
        self.dock_berth(0)
    }
    /// Where drydock `index` floats out the hull it launches: in the water alongside the
    /// outer end of its quay, on the side away from the base's other slip.
    pub fn dock_berth(&self, index: usize) -> Vec2 {
        let side = self.docks[index].station.x;
        self.local(Vec2::new(side + BERTH_OUTBOARD * side.signum(), 46.))
    }

    pub(crate) fn clear_land(&self, pos: Vec2, clearance: f32) -> Vec2 {
        let forward = self.body.forward();
        let clear_circle = |point: Vec2, center: Vec2, radius: f32, normal: Vec2| {
            let offset = point - center;
            if offset.length_squared() < radius * radius {
                center + offset.try_normalize().unwrap_or(normal) * radius
            } else {
                point
            }
        };
        let mut pos = pos;
        for (shore, radius) in self.shores() {
            pos = clear_circle(pos, shore, radius + clearance, forward);
        }
        for gun in &self.turrets {
            pos = clear_circle(
                pos,
                gun.position(&self.body),
                gun.radius() + clearance,
                forward,
            );
        }
        let side = forward.perp();
        let (root, end, width) = self.quay();
        for dock in &self.docks {
            let pier_root = self.body.pos + side * dock.station.x;
            let nearest = pier_root + forward * (pos - pier_root).dot(forward).clamp(root, end);
            pos = clear_circle(pos, nearest, width + clearance, side);
        }
        pos
    }
}
