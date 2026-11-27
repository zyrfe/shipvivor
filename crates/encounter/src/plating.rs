//! Faceted hull contact and ship-local hit marks. The hull outline is shared with the mesh.
use super::{EnemyClass, Raider};
use glam::{Vec2, Vec3, Vec3Swizzles};
use shipvivor_artillery::{ArmorGrade, ArmorPanel, Cell, ContactAngle, Impact, Round, Strike};

pub use shipvivor_shipyard::HULL_OUTLINE;

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
/// A mark a round left on her plating: a crater where the plate held it, or a hole
/// where it went in. Positions and normals are in her deck's frame.
#[derive(Clone, Copy)]
pub struct Scar {
    pub position: Vec3,
    pub normal: Vec3,
    pub impact: Impact,
    pub scale: f32,
    /// Seconds into the battle that a round last went in here, or that the mark was
    /// made, if none has. A fresh hole smoulders.
    pub at: f32,
    surface: Surface,
    cell: Cell,
}

pub(super) struct Plating {
    belt: ArmorPanel,
    /// The plate of the strakes hung over her belt, where they still hang.
    strake_plate: ArmorPanel,
    /// The thin plating behind the strakes, bared where one has been shot off.
    inner: ArmorPanel,
    deck: ArmorPanel,
    tower: ArmorPanel,
    strakes: Vec<Strake>,
    pub scars: Vec<Scar>,
}
/// Scars a hull keeps. The oldest mark goes when a new one would pass this.
const SCARS: usize = 32;

/// Strakes of side armour hung along each side of a battleship or a flagship.
pub const STRAKES_A_SIDE: usize = 7;
/// A strake's foot and its height, as shares of the height of her deck.
pub const STRAKE_RISE: (f32, f32) = (0.4, 0.67);
/// Share of a round's wear a strake takes when the round glances off it.
const GLANCING_WEAR: f32 = 0.25;

/// One of the plates of armour hung along a capital ship's side over her belt. Every
/// round that strikes it wears it, whether its plate holds the round or not, even a
/// light gun's, and enough of them shake it loose. Behind it lies only thin plating:
/// from then on light rounds open her there, and a flagship's turrets screen none of it.
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
    /// Wear it stood when it was hung.
    full: f32,
}
impl Strake {
    /// Still hung on her side.
    pub fn on(&self) -> bool {
        self.strength > 0.
    }
    /// How far it has been shaken loose: 0 when sound, 1 once it has come away.
    pub fn loose(&self) -> f32 {
        if self.full > 0. {
            1. - self.strength / self.full
        } else {
            0.
        }
    }
    /// Whether the point `local` on her side, in her deck's frame, lies behind it.
    fn covers(&self, local: Vec2) -> bool {
        local.x * self.abeam > 0. && (local.y - self.aft).abs() <= 0.5 * self.length
    }
}

/// What became of a round on her hull.
pub(super) struct Struck {
    pub impact: Impact,
    /// The share of the round's damage that reached behind the plate.
    pub share: f32,
    /// Inches of plate the round met there.
    pub plate: f32,
    /// The outward normal of the face it met, in the world.
    pub normal: Vec3,
    /// It went in where a strake has been shot off her side.
    pub bare: bool,
    /// The strake it shook loose, by its index.
    pub loosed: Option<usize>,
}
impl EnemyClass {
    /// The grade of her side belt, which a round has to open to reach her hull: capital
    /// on the battleship and on every boss, which are at least as well found as she is.
    pub fn belt(self) -> ArmorGrade {
        if self.plated() {
            ArmorGrade::Capital
        } else {
            ArmorGrade::Light
        }
    }
    /// The grade of the strakes hung over her belt, for a class that hangs them: as heavy
    /// as the belt itself.
    pub fn strake_plate(self) -> ArmorGrade {
        ArmorGrade::Capital
    }
    /// The grade of plate on her main turrets: reinforced on the battleship and the
    /// bosses, light on the smaller classes.
    pub fn turret_plate(self) -> ArmorGrade {
        if self.plated() {
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
    /// Whether she hangs strakes of armour along her sides: the battleship and the
    /// bosses do, and they carry capital plate.
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
    /// The plate a round has to open to do her its full damage: while a flagship's
    /// turrets still screen her hull, theirs, or else her side, strakes and all.
    pub fn telling_plate(&self) -> ArmorGrade {
        if self.screened() {
            self.class.turret_plate()
        } else if self.plate.strakes.iter().any(Strake::on) {
            let (belt, strake) = (self.class.belt(), self.class.strake_plate());
            if belt.inches() > strake.inches() {
                belt
            } else {
                strake
            }
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
        let heavy = class.plated();
        Self {
            belt: ArmorPanel::new(class.belt(), 88, 4),
            strake_plate: ArmorPanel::new(class.strake_plate(), 88, 4),
            inner: ArmorPanel::new(ArmorGrade::Light, 88, 4),
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
                            full: class.strake_strength(),
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
    /// strikes on her side, held or not. `luck`, from 0 to 1, decides whether a round
    /// with penetration to spare goes clean through, and `clock` is the battle's time.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn strike_hull(
        &mut self,
        point: Vec3,
        travel: Vec3,
        round: Round,
        tower: bool,
        wear: f32,
        luck: f32,
        clock: f32,
    ) -> Struck {
        let scale = round.calibre / 6.;
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
        let panel = match (surface, strake) {
            // A strake still hung takes the round on its own plate. Where one has been
            // shot off, only the thin plating behind it is left to keep a round out.
            (Surface::Belt, Some(_)) if bare => &mut self.plate.inner,
            (Surface::Belt, Some(_)) => &mut self.plate.strake_plate,
            (Surface::Belt, None) => &mut self.plate.belt,
            (Surface::Deck, _) => &mut self.plate.deck,
            (Surface::Tower, _) => &mut self.plate.tower,
        };
        let plate = panel.plate_at(cell);
        let Strike { impact, share } = panel
            .strike(round, angle, cell, luck)
            .expect("local armor cell");
        let mut loosed = None;
        if let Some(i) = strake.filter(|_| !bare) {
            let s = &mut self.plate.strakes[i];
            let worn = if impact == Impact::Bounced {
                GLANCING_WEAR
            } else {
                1.
            };
            s.strength = (s.strength - wear * worn).max(0.);
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
                // A crater opened up into a hole, or a hole a heavier round tore wider.
                if impact.opened() && (!old.impact.opened() || scale > old.scale) {
                    old.impact = impact;
                    old.position = anchor;
                }
                old.scale = old.scale.max(scale);
                if impact.opened() {
                    old.at = clock;
                }
            } else {
                if scars.len() >= SCARS {
                    scars.remove(0);
                }
                scars.push(Scar {
                    position: anchor,
                    normal,
                    impact,
                    scale,
                    at: clock,
                    surface,
                    cell,
                });
            }
        }
        Struck {
            impact,
            share,
            plate,
            normal: world,
            bare,
            loosed,
        }
    }

    /// As [`Raider::strike_hull`], for a round on her main gun `i`: what became of it,
    /// the outward normal of the face it met, the grade of the plate there and the
    /// inches of it the round met.
    pub(super) fn strike_mount(
        &mut self,
        i: usize,
        point: Vec3,
        travel: Vec3,
        round: Round,
        luck: f32,
    ) -> (Strike, Vec3, ArmorGrade, f32) {
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
        let plate = gun.plate.plate_at(cell);
        let strike = gun
            .plate
            .strike(round, incidence(travel, normal), cell, luck)
            .expect("local mount cell");
        (strike, normal, gun.plate.grade(), plate)
    }
}
