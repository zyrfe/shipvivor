use super::{
    Battery, Body, Effect, EnemyClass, HarbourSite, Shot, Vec2, bearing, body, entry,
    mounts::train_full,
    projectile::{FlightPath, lead},
};
use glam::Vec3Swizzles;

pub const ISLAND_RADIUS: f32 = 26.;
pub const SPAWN_STANDOFF: f32 = 275.;
pub const SHORE_RANGE: f32 = 320.;

pub const YARD_CAPACITY: u8 = 12;
const YARD_STRENGTH: u16 = 24;
const FACTORY_STRENGTH: u16 = 8;

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

/// A single independently trained, damageable mount on the harbour apron.
pub struct ShoreTurret {
    // Starboard and forward of the harbour centre, in the port's rotating frame.
    station: Vec2,
    pub barrels: u8,
    pub heading: f32,
    pub(super) strength: u16,
    reload: f32,
}
impl ShoreTurret {
    const STRENGTH_PER_BARREL: u16 = 6;
    fn new(side: f32, fore: f32, barrels: u8, heading: f32, index: usize) -> Self {
        Self {
            station: Vec2::new(side, fore),
            barrels,
            heading,
            strength: Self::STRENGTH_PER_BARREL * u16::from(barrels),
            reload: 2. + 1.6 * index as f32,
        }
    }
    pub fn strength(&self) -> u16 {
        self.strength
    }
    pub fn max_strength(&self) -> u16 {
        Self::STRENGTH_PER_BARREL * u16::from(self.barrels)
    }
    pub fn live(&self) -> bool {
        self.strength > 0
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
        at.distance_squared(target) <= SHORE_RANGE * SHORE_RANGE
            && entry(at, target, base.pos, ISLAND_RADIUS).is_none()
    }
    fn fire(
        &mut self,
        dt: f32,
        base: &Body,
        player: Option<Vec2>,
        way: Vec2,
        mut launch: impl FnMut(FlightPath, u16),
    ) {
        if !self.live() {
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
        let damage = if self.barrels == 1 { 9 } else { 6 };
        for tube in 0..self.barrels {
            let offset = Self::barrel_x(self.barrels, tube);
            let muzzle = (muzzle_line + side * offset).extend(6.65).xzy();
            // Outer rounds bracket the contact, leaving space to manoeuvre between them.
            let target = (ahead + side * (offset * 4.)).extend(1.8).xzy();
            if let Some(path) = FlightPath::from_barrel(muzzle, target, Battery::Shore, 0.) {
                launch(path, damage);
            }
        }
        self.reload = 6. + 0.9 * f32::from(self.barrels - 1);
    }
}

pub struct Port {
    pub has_contact: bool,
    pub stock: u8,
    strength: u16,
    factories: Vec<Factory>,
    // Unfinished fraction of the next unit of material. A full storehouse stops production.
    material_progress: f64,
    tier: usize,
    order: (EnemyClass, u8),
    order_index: usize,
    pub(super) slipway_clock: f32,
    pub(super) convoy_clock: f32,
    pub body: Body,
    pub(super) turrets: Vec<ShoreTurret>,
}
impl Port {
    /// The classic layout: one harbour outside each of three gates, facing the fairway.
    pub(crate) fn classic_sites(gates: [Vec2; 5]) -> [HarbourSite; 3] {
        let center = gates.iter().sum::<Vec2>() / gates.len() as f32;
        [1, 2, 4].map(|i| {
            let outward = (gates[i] - center).normalize();
            HarbourSite {
                pos: gates[i] + outward * 105.,
                heading: bearing(-outward),
            }
        })
    }
    /// Base A has one factory, B two and C three. Their convoys first sail in that order.
    pub(crate) fn chart(
        sites: &[HarbourSite; 3],
        wave_interval: f32,
        serial: &mut u64,
    ) -> [Self; 3] {
        [(0, 25.), (1, 38.), (2, 64.)].map(|(index, convoy_clock)| {
            let HarbourSite { pos, heading } = sites[index];
            let works = index + 1;
            let stations: &[(f32, f32, u8)] = match index {
                0 => &[(-14., 23., 1)],
                1 => &[(-14., 23., 2), (16., -22., 2)],
                _ => &[(-14., 23., 4), (23., -17.5, 3), (-16., -22., 3)],
            };
            Self {
                body: body(serial, pos, heading),
                turrets: stations
                    .iter()
                    .enumerate()
                    .map(|(i, &(side, fore, barrels))| {
                        ShoreTurret::new(side, fore, barrels, heading + side.atan2(fore), i)
                    })
                    .collect(),
                has_contact: false,
                stock: 2,
                factories: (0..works)
                    .map(|_| Factory {
                        strength: FACTORY_STRENGTH,
                    })
                    .collect(),
                material_progress: 0.,
                tier: 0,
                order: (EnemyClass::Patrol, 2),
                order_index: 0,
                slipway_clock: works as f32 * wave_interval,
                strength: YARD_STRENGTH,
                convoy_clock,
            }
        })
    }

    pub fn yard_position(&self) -> Vec2 {
        let forward = self.body.forward();
        self.body.pos + forward * 4. + forward.perp() * 6.
    }

    pub(super) fn yard_span(&self, a: Vec2, b: Vec2) -> Option<(f32, f32)> {
        self.building_span(self.yard_position(), (5.5, 7.), a, b)
    }

    pub fn factories(&self) -> &[Factory] {
        &self.factories
    }
    pub fn live_factories(&self) -> usize {
        self.factories
            .iter()
            .filter(|factory| factory.live())
            .count()
    }
    pub fn factory_position(&self, index: usize) -> Vec2 {
        // The twin mills occupy the same shingle bank, with a conveyor between them.
        // At the heavy yard the three furnace halls take separate faces of the shore.
        let sites: &[(f32, f32)] = match self.factories.len() {
            1 => &[(-20.5, -7.)],
            2 => &[(19.5, -9.), (19.5, 9.)],
            _ => &[(-20.5, -7.), (19.5, 7.), (6., 20.)],
        };
        let (side, aft) = sites[index];
        let f = self.body.forward();
        self.body.pos + f.perp() * side - f * aft
    }
    pub fn factory_top(&self, index: usize) -> f32 {
        if self.factories[index].live() {
            12.2
        } else {
            4.5
        }
    }
    pub(super) fn factory_span(&self, index: usize, a: Vec2, b: Vec2) -> Option<(f32, f32)> {
        self.building_span(self.factory_position(index), (4., 4.), a, b)
    }
    pub(super) fn damage_factory(&mut self, index: usize, damage: u16) -> bool {
        let factory = &mut self.factories[index];
        let was_live = factory.live();
        factory.strength = factory.strength.saturating_sub(damage);
        was_live && !factory.live()
    }
    pub fn production_per_minute(&self) -> f32 {
        if self.strength == 0 {
            0.
        } else {
            1. + 3. * self.live_factories() as f32
        }
    }
    /// The slip keeps the order even when the bins are too empty to launch it.
    pub fn build_order(&self) -> (EnemyClass, u8) {
        self.order
    }

    fn next_order(&self) -> (EnemyClass, u8) {
        const PATROL: (EnemyClass, u8) = (EnemyClass::Patrol, 2);
        const DESTROYER: (EnemyClass, u8) = (EnemyClass::Destroyer, 4);
        const CRUISER: (EnemyClass, u8) = (EnemyClass::Cruiser, 7);
        const BATTLESHIP: (EnemyClass, u8) = (EnemyClass::Battleship, YARD_CAPACITY);
        let rotation: &[(EnemyClass, u8)] = match (self.tier, self.factories.len()) {
            (0, _) => &[PATROL],
            (1, 1) => &[DESTROYER, PATROL],
            (1, _) => &[DESTROYER, DESTROYER, PATROL],
            (_, 1) => &[DESTROYER],
            (2, _) | (_, 2) => &[CRUISER, CRUISER, DESTROYER],
            _ => &[BATTLESHIP, BATTLESHIP, CRUISER],
        };
        rotation[self.order_index % rotation.len()]
    }
    pub(super) fn work(&mut self, dt: f32, tier: usize) {
        if self.strength == 0 {
            return;
        }
        if self.tier != tier {
            self.tier = tier;
            self.order_index = 0;
        }
        self.slipway_clock = (self.slipway_clock - dt).max(0.);
        let supply =
            self.material_progress + f64::from(dt) * f64::from(self.production_per_minute()) / 60.;
        self.stock = self.stock.saturating_add(supply as u8).min(YARD_CAPACITY);
        self.material_progress = if self.stock < YARD_CAPACITY {
            supply.fract()
        } else {
            0.
        };
    }
    pub(super) fn launch(&mut self, cost: u8, next_slip: f32) {
        self.stock -= cost;
        // A tier unlock changes what the yard lays down after this launch. The hull
        // already on the slip keeps its shape, bill and accumulated material.
        self.order = self.next_order();
        self.order_index = self.order_index.wrapping_add(1);
        self.slipway_clock = next_slip;
    }
    fn building_span(
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
    pub fn yard_state(&self) -> YardState {
        match self.strength {
            0 => YardState::Ruined,
            1..=8 => YardState::Burning,
            9..=16 => YardState::Damaged,
            _ => YardState::Working,
        }
    }

    pub fn yard_top(&self) -> f32 {
        match self.yard_state() {
            YardState::Working => 20.,
            YardState::Damaged => 15.,
            YardState::Burning => 9.,
            YardState::Ruined => 4.5,
        }
    }
    pub(super) fn needs_delivery(&self) -> bool {
        self.strength > 0
            && (self.stock < YARD_CAPACITY
                || self.strength < YARD_STRENGTH
                || self.turrets.iter().any(|g| g.strength < g.max_strength()))
    }
    pub(super) fn damage_yard(&mut self, damage: u16) -> bool {
        let was_live = self.strength > 0;
        self.strength = self.strength.saturating_sub(damage);
        if self.strength == 0 {
            self.stock = 0;
            for gun in &mut self.turrets {
                gun.strength = 0;
            }
            self.material_progress = 0.;
            for factory in &mut self.factories {
                factory.strength = 0;
            }
        }
        was_live && self.strength == 0
    }
    pub(super) fn unload(&mut self) {
        if self.strength > 0 {
            self.stock = (self.stock + 6).min(YARD_CAPACITY);
            self.strength = (self.strength + 8).min(YARD_STRENGTH);
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
        }
    }
    pub fn turrets(&self) -> &[ShoreTurret] {
        &self.turrets
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

    pub(super) fn land_volumes(&self) -> [(Vec2, f32, f32); 2] {
        let c = self.body.pos;
        let f = self.body.forward();
        [
            (c, ISLAND_RADIUS, 3.2),
            (c - f * 7. - f.perp() * 5., 14., 10.),
        ]
    }

    /// A ship appears in the water alongside the outer end of the quay.
    pub fn berth(&self) -> Vec2 {
        self.body.pos + self.body.forward() * 46. + self.body.forward().perp() * 14.
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
        let mut pos = clear_circle(pos, self.body.pos, ISLAND_RADIUS + clearance, forward);
        for gun in &self.turrets {
            pos = clear_circle(pos, gun.position(&self.body), 5. + clearance, forward);
        }
        let side = forward.perp();
        let pier_root = self.body.pos + side * 6.;
        // The foundry's occupied berth has wider quays ending closer to shore.
        let (end, width) = if self.factories.len() == 3 {
            (30., 4.8)
        } else {
            (44., 3.)
        };
        let nearest = pier_root + forward * (pos - pier_root).dot(forward).clamp(20., end);
        clear_circle(pos, nearest, width + clearance, side)
    }
}
