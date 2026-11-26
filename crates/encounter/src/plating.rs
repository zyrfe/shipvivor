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
    pub scars: Vec<Scar>,
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
}
impl Raider {
    /// The plate a round has to open to hurt her: her belt, or while a flagship's
    /// turrets still screen her hull, theirs.
    pub fn telling_plate(&self) -> ArmorGrade {
        if self.protected() {
            self.class.turret_plate()
        } else {
            self.class.belt()
        }
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

    /// What became of a round on her hull, the outward normal it met, and the grade of
    /// the plate there.
    pub(super) fn strike_hull(
        &mut self,
        point: Vec3,
        travel: Vec3,
        (ammo, shell): (Ammo, ShellClass),
        bore: u16,
        tower: bool,
    ) -> (Impact, Vec3, ArmorGrade) {
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
        let panel = match surface {
            Surface::Belt => &mut self.plate.belt,
            Surface::Deck => &mut self.plate.deck,
            Surface::Tower => &mut self.plate.tower,
        };
        let grade = panel.grade();
        let outcome = panel
            .resolve_ammo(ammo, shell, angle, &[cell], bore)
            .expect("local armor cell");
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
        (outcome, world, grade)
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
