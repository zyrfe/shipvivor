//! Faceted hull contact and ship-local hit marks. The hull outline is shared with the mesh.
use super::{EnemyClass, Raider};
use glam::{Vec2, Vec3, Vec3Swizzles};
use shipvivor_artillery::{Ammo, ArmorGrade, ArmorPanel, Cell, ContactAngle, Impact, ShellClass};

pub const HULL_OUTLINE: [(f32, f32); 11] = [
    (0., -16.),
    (1.65, -11.7),
    (2.7, -5.),
    (2.75, 6.),
    (2.1, 12.7),
    (1.4, 14.1),
    (-1.4, 14.1),
    (-2.1, 12.7),
    (-2.75, 6.),
    (-2.7, -5.),
    (-1.65, -11.7),
];

/// Breakwater's deck edge after the yard inserts frames ahead of the after gun.
pub fn breakwater_outline(extension: f32) -> [(f32, f32); 11] {
    HULL_OUTLINE.map(|(x, z)| (x, super::Hull::deck_z(z, extension)))
}

pub(super) fn clip_hull(vertices: &[Vec2], a: Vec2, delta: Vec2) -> Option<(f32, f32)> {
    let (mut lo, mut hi) = (0_f32, 1_f32);
    for (i, start) in vertices.iter().enumerate() {
        let edge = vertices[(i + 1) % vertices.len()] - start;
        let inside = edge.perp_dot(a - start);
        let step = edge.perp_dot(delta);
        if step.abs() < 1e-6 {
            if inside < 0. {
                return None;
            }
        } else {
            let cross = -inside / step;
            if step > 0. {
                lo = lo.max(cross);
            } else {
                hi = hi.min(cross);
            }
            if hi < lo {
                return None;
            }
        }
    }
    Some((lo, hi))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Surface {
    Belt,
    Deck,
    Tower,
}
pub struct Scar {
    pub position: Vec3,
    pub normal: Vec3,
    pub impact: Impact,
    pub scale: f32,
    surface: Surface,
    cell: Cell,
}

pub(super) struct Plating {
    belt: ArmorPanel,
    deck: ArmorPanel,
    tower: ArmorPanel,
    strakes: Vec<Strake>,
    pub scars: Vec<Scar>,
}

/// Strakes of side armour hung along each side of a battleship or a flagship.
pub const STRAKES_A_SIDE: usize = 7;
/// A strake's foot and its height, as shares of the height of her deck.
pub const STRAKE_RISE: (f32, f32) = (0.4, 0.67);
/// Share of a round's wear a strake takes when the round glances off it.
const GLANCING_WEAR: f32 = 0.25;

/// One of the plates of armour hung along a capital ship's side over her belt. Every
/// main-battery round that strikes it wears it, whether her plate holds the round or
/// not, and enough of them shake it loose. The hull behind it lies bare from then on:
/// any round there goes straight in, and a flagship's turrets screen none of it.
#[derive(Clone, Copy, Debug)]
pub struct Strake {
    /// Metres to starboard of her centreline, negative to port.
    pub abeam: f32,
    /// Metres abaft amidships of its middle, negative forward.
    pub aft: f32,
    /// Its length fore and aft, in metres.
    pub length: f32,
    /// Wear it stands before it comes away.
    strength: f32,
}
impl Strake {
    /// Still hung on her side.
    pub fn on(&self) -> bool {
        self.strength > 0.
    }
    /// Whether the point `local` on her side, in her deck's frame, lies behind it.
    fn covers(&self, local: Vec2) -> bool {
        local.x * self.abeam > 0. && (local.y - self.aft).abs() <= 0.5 * self.length
    }
}

/// What became of a round on her hull.
pub(super) struct Struck {
    pub impact: Impact,
    /// The outward normal of the face it met, in the world.
    pub normal: Vec3,
    /// The grade of the plate there.
    pub grade: ArmorGrade,
    /// It went in where a strake has been shot off her side.
    pub bare: bool,
    /// The strake it shook loose, by its index.
    pub loosed: Option<usize>,
}
impl EnemyClass {
    /// The grade of her side belt, which a round has to open to reach her hull.
    pub fn belt(self) -> ArmorGrade {
        if self == EnemyClass::Battleship {
            ArmorGrade::Capital
        } else {
            ArmorGrade::Light
        }
    }
    /// The grade of plate on her main turrets.
    pub fn turret_plate(self) -> ArmorGrade {
        if matches!(self, EnemyClass::Battleship | EnemyClass::Commodore) {
            ArmorGrade::Reinforced
        } else {
            ArmorGrade::Light
        }
    }
    /// Wear each of her side strakes stands: two heavy rounds, or a handful of light
    /// ones. Classes that hang none have none.
    fn strake_strength(self) -> f32 {
        match self {
            EnemyClass::Battleship | EnemyClass::Commodore => 12.,
            EnemyClass::Parliament => 18.,
            EnemyClass::Regent => 36.,
            _ => 0.,
        }
    }
    /// Whether she hangs strakes of armour along her sides: the battleships and the
    /// flagships do.
    pub fn plated(self) -> bool {
        self.strake_strength() > 0.
    }
    /// Where her strake `index` hangs: metres to starboard of her centreline, metres
    /// abaft amidships, and its length. The port strakes come first, each side's from
    /// forward aft. The deckwork mesh is built around the same places.
    pub fn strake_station(self, index: usize) -> (f32, f32, f32) {
        let p = self.profile();
        let side = if index < STRAKES_A_SIDE { -1. } else { 1. };
        let t = ((index % STRAKES_A_SIDE) as f32 - 3.) / 4.;
        (
            side * p.hull.radius * (0.93 - 0.20 * t.abs()),
            t * p.hull.stern,
            p.hull.stern * 0.21,
        )
    }
}
impl Raider {
    /// The plate a round has to open to hurt her: her belt, or while a flagship's
    /// turrets still screen her hull, theirs.
    pub fn telling_plate(&self) -> ArmorGrade {
        if self.screened() {
            self.class.turret_plate()
        } else {
            self.class.belt()
        }
    }
    /// The strakes along her sides, in [`EnemyClass::strake_station`] order.
    pub fn strakes(&self) -> &[Strake] {
        &self.plate.strakes
    }
    /// The middle of her strake `index`, in the world.
    pub fn strake_at(&self, index: usize) -> Vec3 {
        let s = &self.plate.strakes[index];
        let f = self.body.forward();
        let (foot, height) = STRAKE_RISE;
        (self.body.pos + f.perp() * s.abeam - f * s.aft)
            .extend((foot + 0.5 * height) * self.class.profile().deck)
            .xzy()
    }
}

impl Plating {
    pub fn new(class: EnemyClass) -> Self {
        let heavy = class == EnemyClass::Battleship;
        Self {
            belt: ArmorPanel::new(class.belt(), 88, 4),
            deck: ArmorPanel::new(
                if heavy {
                    ArmorGrade::Reinforced
                } else {
                    ArmorGrade::Light
                },
                24,
                40,
            ),
            tower: ArmorPanel::new(ArmorGrade::Light, 16, 4),
            strakes: if class.plated() {
                (0..2 * STRAKES_A_SIDE)
                    .map(|i| {
                        let (abeam, aft, length) = class.strake_station(i);
                        Strake {
                            abeam,
                            aft,
                            length,
                            strength: class.strake_strength(),
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            },
            scars: Vec::new(),
        }
    }
}

fn bin(fraction: f32, divisions: u16) -> u16 {
    (fraction.clamp(0., 1.) * f32::from(divisions))
        .floor()
        .min(f32::from(divisions - 1)) as u16
}
fn radial_cell(n: Vec2, turn: f32) -> u16 {
    bin(
        ((n.x.atan2(-n.y) - turn) / std::f32::consts::TAU).rem_euclid(1.),
        16,
    )
}
fn incidence(travel: Vec3, normal: Vec3) -> ContactAngle {
    // A shallow scrape slides away. A square blow lets the plate grade decide.
    let threshold = if normal.y > 0.85 { 0.048 } else { 0.17 };
    if travel.dot(normal).abs() < threshold {
        ContactAngle::Glancing
    } else {
        ContactAngle::Direct
    }
}

impl Raider {
    pub fn scars(&self) -> &[Scar] {
        &self.plate.scars
    }

    pub(super) fn deck_local(&self, point: Vec2) -> Vec2 {
        let d = point - self.body.pos;
        let f = self.body.forward();
        Vec2::new(d.dot(f.perp()), -d.dot(f))
    }
    pub(super) fn world_normal(&self, n: Vec3) -> Vec3 {
        let f = self.body.forward();
        (f.perp() * n.x - f * n.z).extend(n.y).xzy()
    }
    fn hull_vertices(&self) -> [Vec2; 11] {
        let h = self.class.profile().hull;
        HULL_OUTLINE.map(|(x, z)| Vec2::new(x * h.radius / 2.75, z * h.bow / 16.))
    }

    pub(super) fn hull_span(&self, a: Vec2, b: Vec2) -> Option<(f32, f32)> {
        let a = self.deck_local(a);
        let delta = self.deck_local(b) - a;
        let vertices = self.hull_vertices();
        clip_hull(&vertices, a, delta)
    }

    /// What became of a round on her hull. `wear` is what it takes out of a strake it
    /// strikes on her side, held or not.
    pub(super) fn strike_hull(
        &mut self,
        point: Vec3,
        travel: Vec3,
        (ammo, shell): (Ammo, ShellClass),
        bore: u16,
        tower: bool,
        wear: f32,
    ) -> Struck {
        let scale = super::MainShell {
            bore,
            ..super::MainShell::STOCK
        }
        .scale();
        let p = self.class.profile();
        let local = self.deck_local(point.xz());
        let mut anchor = local.extend(point.y).xzy();
        let (surface, cell, normal) = if tower {
            let n = local.normalize_or_zero();
            let top = p.deck + if self.class.is_flagship() { 3.5 } else { 2.6 } * p.gun_scale;
            let normal = if (point.y - top).abs() < 0.035 {
                Vec3::Y
            } else {
                n.extend(0.).xzy()
            };
            (
                Surface::Tower,
                (
                    radial_cell(n, 0.),
                    bin((point.y - p.deck) / (top - p.deck), 4),
                ),
                normal,
            )
        } else if (point.y - p.deck).abs() < 0.035 {
            (
                Surface::Deck,
                (
                    bin(0.5 + 0.5 * local.x / p.hull.radius, 24),
                    bin((local.y + p.hull.bow) / (p.hull.bow + p.hull.stern), 40),
                ),
                Vec3::Y,
            )
        } else {
            let vertices = self.hull_vertices();
            let mut nearest = (f32::INFINITY, 0, 0., Vec2::ZERO);
            for (i, start) in vertices.iter().enumerate() {
                let edge = vertices[(i + 1) % vertices.len()] - start;
                let t = ((local - start).dot(edge) / edge.length_squared()).clamp(0., 1.);
                let dist = local.distance_squared(start + edge * t);
                if dist < nearest.0 {
                    nearest = (dist, i, t, -edge.perp().normalize());
                }
            }
            let (_, face, t, n) = nearest;
            // The rendered sides narrow below the deck. Seat the mark against that skin.
            let level = (point.y / p.deck).clamp(0., 1.);
            let taper = if level < 0.245 {
                0.86 + 0.04 * level / 0.245
            } else {
                0.9 + 0.1 * (level - 0.245) / 0.755
            };
            let side = vertices[face].lerp(vertices[(face + 1) % vertices.len()], t) * taper;
            anchor = side.extend(point.y.min(p.deck)).xzy();
            (
                Surface::Belt,
                (face as u16 * 8 + bin(t, 8), bin(level, 4)),
                n.extend(0.).xzy(),
            )
        };
        let world = self.world_normal(normal);
        let angle = incidence(travel, world);
        let strake = if surface == Surface::Belt {
            self.plate.strakes.iter().position(|s| s.covers(local))
        } else {
            None
        };
        let bare = strake.is_some_and(|i| !self.plate.strakes[i].on());
        let (outcome, grade) = if bare {
            // Nothing is left on her side here to keep a round out.
            (Impact::Penetrated, ArmorGrade::Light)
        } else {
            let panel = match surface {
                Surface::Belt => &mut self.plate.belt,
                Surface::Deck => &mut self.plate.deck,
                Surface::Tower => &mut self.plate.tower,
            };
            let grade = panel.grade();
            let outcome = panel
                .resolve_ammo(ammo, shell, angle, &[cell], bore)
                .expect("local armor cell");
            (outcome, grade)
        };
        let mut loosed = None;
        if let Some(i) = strake.filter(|_| !bare) {
            let s = &mut self.plate.strakes[i];
            let share = if outcome == Impact::Bounced {
                GLANCING_WEAR
            } else {
                1.
            };
            s.strength = (s.strength - wear * share).max(0.);
            if !s.on() {
                loosed = Some(i);
            }
        }
        // Tower skin is a simplified spotting volume. Put permanent marks on the hull mesh.
        if surface != Surface::Tower {
            let scars = &mut self.plate.scars;
            if let Some(old) = scars
                .iter_mut()
                .find(|s| s.surface == surface && s.cell == cell)
            {
                if outcome == Impact::Penetrated {
                    old.impact = outcome;
                    old.scale = scale;
                    old.position = anchor;
                }
            } else {
                if scars.len() >= 32 {
                    scars.remove(0);
                }
                scars.push(Scar {
                    position: anchor,
                    normal,
                    impact: outcome,
                    scale,
                    surface,
                    cell,
                });
            }
        }
        Struck {
            impact: outcome,
            normal: world,
            grade,
            bare,
            loosed,
        }
    }

    /// As [`Raider::strike_hull`], for a round on her main gun `i`.
    pub(super) fn strike_mount(
        &mut self,
        i: usize,
        point: Vec3,
        travel: Vec3,
        (ammo, shell): (Ammo, ShellClass),
        bore: u16,
    ) -> (Impact, Vec3, ArmorGrade) {
        let profile = self.class.profile();
        let gun = &mut self.guns[i];
        let base = gun.base(&self.body, profile);
        let n = (point.xz() - base.xz()).normalize_or_zero();
        let roof = (point.y - gun.top(profile)).abs() < 0.035;
        let normal = if roof { Vec3::Y } else { n.extend(0.).xzy() };
        let row = if roof {
            3
        } else {
            bin(
                (point.y - profile.deck) / (gun.top(profile) - profile.deck),
                3,
            )
        };
        let cell = (radial_cell(n, gun.bearing(&self.body)), row);
        let outcome = gun
            .plate
            .resolve_ammo(ammo, shell, incidence(travel, normal), &[cell], bore)
            .expect("local mount cell");
        (outcome, normal, gun.plate.grade())
    }
}
