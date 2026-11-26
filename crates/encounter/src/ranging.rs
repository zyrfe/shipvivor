//! A heavy shore battery's fire control.
//!
//! The heaviest base of a sector mounts a battery of long guns that reach far past its
//! light mounts and fire slowly. Each salvo is laid where her present course and speed
//! will carry her, but the battery does not yet know her range: the first salvo falls
//! well over or well short along the line of fire. Watching each fall, the battery closes
//! the error by about half a salvo while she holds her course and speed, so its columns
//! walk in over, short, over, until they straddle her. A real change of course or speed
//! since the last salvo throws the plot out and the error grows back. A floatplane or
//! another base reporting her position spots the fall of shot from close by: the first
//! salvo falls closer and the plot closes faster.
//!
//! A heavy shell flies slower over the water than its report crosses it, on a long lob, so
//! she hears the deep report of a salvo before its rush comes down on her.
use super::{
    Battery, Battle, Body, Chart, Course, Orders, Vec2, bearing,
    mounts::train_full,
    port::ShoreTurret,
    projectile::{FlightPath, lead},
};
use glam::Vec3Swizzles;
use std::f32::consts::{PI, TAU};

/// Metres a heavy battery reaches: five times as far as the light shore guns, and across
/// the reach before its base.
pub const HEAVY_RANGE: f32 = 1600.;
/// Seconds between its salvos.
pub const HEAVY_RELOAD: f32 = 9.;
/// What each heavy shell does on a hit, before plate.
pub const HEAVY_DAMAGE: u16 = 12;
/// Inches across a heavy shell. Four armor plate cards (13 inches) cannot turn it, five
/// can, as with the Parliament's guns.
pub const HEAVY_CALIBRE: f32 = 15.;
/// Metres per second a heavy shell makes over the water, and the pull its lob falls by.
/// It is slower than sound in the scaled sea, so its report arrives before it does.
pub(super) const HEAVY_SPEED: f32 = 140.;
pub(super) const HEAVY_DROP: f32 = 16.;
/// How far past the battery's reach a salvo laid over can still fall.
pub(super) const HEAVY_REACH: f32 = HEAVY_RANGE + FIRST_ERROR.0 + 60.;

/// Metres the first salvo of a plot falls off along the line of fire, without and with
/// a spotter.
const FIRST_ERROR: (f32, f32) = (150., 90.);
/// Share of its error each salvo keeps while she holds course and speed, without and
/// with a spotter.
const CLOSING: (f32, f32) = (0.5, 0.35);
/// The finest the plot gets. The shells' own scatter does the rest.
const FINEST: f32 = 4.;
/// A turn this sharp since the last salvo throws the plot out, as does a change of a
/// third of her speed, but never one of less than `SPEED_SLACK` metres per second.
const THROWN_TURN: f32 = 25. * PI / 180.;
const THROWN_SPEED: f32 = 1. / 3.;
const SPEED_SLACK: f32 = 1.5;
/// A plot thrown out multiplies its error by this, and grows back to at least this share
/// of a first salvo's error, and to no more than all of it.
const REGROW: f32 = 4.;
const REGROWN: f32 = 0.5;
/// Seconds without her before the plot is stale, and the next salvo is a first again.
const STALE: f32 = 25.;
/// Metres between the falls of a salvo's two shells along the line of fire, the scatter
/// of each along it and across it.
const SPREAD: f32 = 20.;
const SCATTER: (f32, f32) = (8., 3.);
/// Degrees a second the heavy gunhouse trains.
const TRAIN: f32 = 9.;
/// Height of the aim on her hull.
const AIM: f32 = 0.9;
/// Height of the battery's rangefinder above the water.
pub(super) const RANGEFINDER: f32 = 14.;

/// The plot a heavy battery keeps of her: how far its last salvo fell from her and the
/// course and speed it was laid on.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ranging {
    /// Metres the last salvo was laid off along the line of fire: beyond her is positive.
    error: f32,
    /// Her heading and signed speed when the last salvo was laid, or `None` when the
    /// battery has no plot of her.
    laid: Option<(f32, f32)>,
    /// Salvos laid since the plot began.
    salvos: u32,
    /// Seconds since the battery last had her.
    unseen: f32,
    /// Has her, through its own rangefinder or its base's word. The lookouts set it.
    pub(super) in_sight: bool,
    /// A floatplane or another base is reporting her position. The lookouts set it.
    pub(super) spotted: bool,
}
impl Ranging {
    /// Salvos laid on her since the plot began, or zero while there is none.
    pub fn salvos(&self) -> u32 {
        self.salvos
    }
    /// Metres the last salvo was laid beyond her, negative when short, or `None` before
    /// the first.
    pub fn error(&self) -> Option<f32> {
        self.laid.map(|_| self.error)
    }
    /// Laying its guns on her, with a plot that has not gone stale.
    pub fn ranging(&self) -> bool {
        self.laid.is_some()
    }
    /// A spotter is reporting her position to the battery.
    pub fn spotted(&self) -> bool {
        self.spotted
    }
    /// Has her in sight, its own or by report.
    pub fn in_sight(&self) -> bool {
        self.in_sight
    }

    /// Time passes. A plot left without her for long enough goes stale.
    pub(super) fn watch(&mut self, dt: f32) {
        if self.in_sight {
            self.unseen = 0.;
            return;
        }
        self.unseen += dt;
        if self.unseen > STALE {
            self.laid = None;
            self.salvos = 0;
        }
    }

    /// Lay the next salvo on her `heading` and signed `speed`: metres beyond her along
    /// the line of fire it is laid, negative when short. The first falls wide and each
    /// after it closer while she holds on. Over and short take turns, starting `over`.
    pub(super) fn lay(&mut self, heading: f32, speed: f32, over: bool) -> f32 {
        let spotted = usize::from(self.spotted);
        let first = [FIRST_ERROR.0, FIRST_ERROR.1][spotted];
        let closing = [CLOSING.0, CLOSING.1][spotted];
        let size = match self.laid {
            None => first,
            Some((was_heading, was_speed)) => {
                let turned = signed(heading - was_heading).abs() >= THROWN_TURN;
                let most = was_speed.abs().max(speed.abs());
                let changed = (speed - was_speed).abs() >= (THROWN_SPEED * most).max(SPEED_SLACK);
                if turned || changed {
                    (self.error.abs() * REGROW).clamp(REGROWN * first, first)
                } else {
                    (self.error.abs() * closing).max(FINEST)
                }
            }
        };
        self.salvos += 1;
        let beyond = over == (self.salvos % 2 == 1);
        self.error = if beyond { size } else { -size };
        self.laid = Some((heading, speed));
        self.error
    }
}

fn signed(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

/// A number from -1 to 1 that a salvo's shell always draws the same, so a seed sails the
/// same waters.
fn jitter(seed: u64, salvo: u32, draw: u64) -> f32 {
    let mut z = seed
        .wrapping_add(u64::from(salvo).wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add(draw.wrapping_mul(0xD1B5_4A32_D192_ED03));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 / (1_u64 << 23) as f32 - 1.
}

/// A number from -1 to 1 that gathers about nothing, as a gunlayer's errors do: the
/// mean of two draws, so half fall within a third of the way out. A seed always draws
/// the same.
pub(super) fn spread(seed: u64, salvo: u32, draw: u64) -> f32 {
    0.5 * (jitter(seed, salvo, 2 * draw) + jitter(seed, salvo, 2 * draw + 1))
}

/// Her as the battery takes her: where she is, her way over the ground, and her heading
/// and signed speed through the water.
#[derive(Clone, Copy)]
pub(super) struct Mark {
    pub at: Vec2,
    pub way: Vec2,
    pub heading: f32,
    pub speed: f32,
}

impl ShoreTurret {
    /// Train on her and, once loaded and bearing, lay the next salvo of the plot. Only a
    /// heavy battery fires here, and only while its base is `roused` and it has her.
    pub(super) fn fire_heavy(
        &mut self,
        dt: f32,
        base: &Body,
        roused: bool,
        her: Mark,
        mut launch: impl FnMut(FlightPath),
    ) {
        let Some(mut plot) = self.ranging else {
            return;
        };
        self.reload = (self.reload - dt).max(0.);
        plot.watch(dt);
        self.ranging = Some(plot);
        if !self.live() || !roused || !plot.in_sight {
            return;
        }
        let at = self.position(base);
        let ahead = lead(at, her.at, her.way, Battery::Heavy);
        let Some(dir) = (ahead - at).try_normalize() else {
            return;
        };
        let trained = train_full(&mut self.heading, bearing(dir), TRAIN.to_radians() * dt);
        if !trained || self.reload > 0. || !self.covers(base, her.at) {
            return;
        }
        let over = base.id % 2 == 0;
        let error = plot.lay(her.heading, her.speed, over);
        self.ranging = Some(plot);
        let range = at.distance(ahead);
        let side = dir.perp();
        let scale = self.mount_scale();
        let muzzle_line = at + dir * 8.25 * scale;
        for tube in 0..self.barrels {
            let offset = Self::barrel_x(self.barrels, tube) * scale;
            let muzzle = (muzzle_line + side * offset)
                .extend(4. + 2.65 * scale)
                .xzy();
            // The two shells of a salvo fall a little apart along the line of fire, so
            // once the plot is fine one falls over her and the other short.
            let pair = if self.barrels > 1 {
                f32::from(tube) / f32::from(self.barrels - 1) - 0.5
            } else {
                0.
            };
            let salvo = plot.salvos;
            let draw = u64::from(tube) * 2;
            let along = error + SPREAD * pair + SCATTER.0 * jitter(base.id, salvo, draw);
            let across = SCATTER.1 * jitter(base.id, salvo, draw + 1);
            // However short it is laid, a salvo still goes out to sea.
            let along = along.max(-0.6 * range);
            let target = (ahead + dir * along + side * across).extend(AIM).xzy();
            if let Some(path) = FlightPath::from_barrel(muzzle, target, Battery::Heavy, 0.) {
                launch(path);
            }
        }
        self.reload = HEAVY_RELOAD;
    }
}

impl Battle {
    /// On a passage, the heaviest base of the sector mounts a heavy battery: the base
    /// with the most factories, and of those the deepest up the channel, which is the
    /// heavy yard at its head. A buoy circuit has none.
    pub(super) fn with_heavy_battery(mut self, chart: &Chart) -> Self {
        if chart.course != Course::Passage {
            return self;
        }
        let heaviest = (self.ports.iter_mut().enumerate())
            .max_by_key(|(i, port)| (port.factories().len(), port.stage(), *i))
            .map(|(_, port)| port);
        if let Some(port) = heaviest {
            port.mount_heavy_battery();
        }
        self
    }

    /// Every heavy battery whose base is roused lays its salvos on her.
    pub(super) fn range_heavy_guns(&mut self, dt: f32, orders: &Orders, way: Vec2) {
        let her = Mark {
            at: orders.player,
            way,
            heading: orders.heading,
            speed: orders.speed,
        };
        for port in &mut self.ports {
            let roused = port.roused;
            let base = port.body;
            for gun in &mut port.turrets {
                gun.fire_heavy(dt, &base, roused, her, |path| {
                    path.launch(
                        &mut self.shots,
                        &mut self.effects,
                        &mut self.serial,
                        Battery::Heavy,
                        HEAVY_DAMAGE,
                    );
                });
            }
        }
    }
}

#[cfg(test)]
mod tests;
