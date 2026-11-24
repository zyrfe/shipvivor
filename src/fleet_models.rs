//! Low-poly fleet fittings. Static deckwork and each moving gun use one mesh apiece.
use crate::scene::Facets;
use EnemyClass::*;
use bevy::prelude::*;
use shipvivor_encounter::EnemyClass;

const STEEL: Color = Color::srgb(0.38, 0.50, 0.52);
const PALE: Color = Color::srgb(0.75, 0.78, 0.70);
const IRON: Color = Color::srgb(0.16, 0.21, 0.24);
const RED: Color = Color::srgb(0.80, 0.23, 0.16);
const GOLD: Color = Color::srgb(1.0, 0.72, 0.23);
const GLASS: Color = Color::srgb(0.09, 0.22, 0.26);
/// A patrol boat has no mast, so she wears her flag on a staff aft: its distance abaft
/// amidships and its height above the deck.
const PATROL_STAFF: (f32, f32) = (5.5, 2.4);

#[derive(Default)]
struct Metal(Facets);
impl Metal {
    /// An eight-sided casting. Sizes are full beam and length.
    fn tier(&mut self, center: Vec3, size: Vec3, taper: f32, wall: Color, roof: Color) {
        let outline = [
            (-0.35, -0.5),
            (0.35, -0.5),
            (0.5, -0.34),
            (0.5, 0.34),
            (0.35, 0.5),
            (-0.35, 0.5),
            (-0.5, 0.34),
            (-0.5, -0.34),
        ];
        let foot = outline.map(|(x, z)| center + Vec3::new(x * size.x, 0., z * size.z));
        let cap = center + Vec3::Y * size.y;
        let top = foot.map(|p| cap + (p - center) * taper);
        for i in 0..8 {
            let j = (i + 1) % 8;
            self.0.triangle([foot[i], top[j], foot[j]], wall);
            self.0.triangle([foot[i], top[i], top[j]], wall);
            self.0.triangle([cap, top[j], top[i]], roof);
        }
    }
}

/// Where each of a class's flags is hoisted, and the scale of the cloth: the top of the
/// bridge mast, a patrol boat's stern staff, and the flagship's twin standards over her
/// signal gantry. The cloth hangs and streams from its hoist.
pub fn ensigns(class: EnemyClass) -> Vec<(Vec3, f32)> {
    let p = class.profile();
    let (d, s) = (p.deck, p.gun_scale);
    let mut flags = vec![if class == Patrol {
        let (aft, height) = PATROL_STAFF;
        (Vec3::new(0., d + height - 0.05, aft), 0.42 * s)
    } else {
        (Vec3::new(0., d + 4.55 * s, -0.2 * s), 0.44 * s)
    }];
    if class.is_flagship() {
        let h = d + 5.0 * s;
        flags.extend([-2.7, 2.7].map(|x| (Vec3::new(x, h + 2.45, 0.), 1.1)));
    }
    flags
}

pub fn superstructure(class: EnemyClass) -> Mesh {
    let mut m = Metal::default();
    let p = class.profile();
    let v = Vec3::new;
    let d = p.deck;
    let s = p.gun_scale;
    let bridge = |x, y, z| v(x * s, d + y * s, z * s);
    // Stacked wings and a continuous dark window strip survive the distant silhouette.
    m.tier(
        bridge(0., 0., 0.),
        v(2.45, 1.30, 3.1) * s,
        0.87,
        STEEL,
        PALE,
    );
    m.tier(
        bridge(0., 1.25, -0.2),
        v(2.1, 0.55, 2.25) * s,
        1.,
        GLASS,
        IRON,
    );
    m.tier(
        bridge(0., 1.8, -0.2),
        v(2.45, 0.28, 2.55) * s,
        0.94,
        RED,
        PALE,
    );
    if class != Patrol {
        m.tier(
            bridge(0., 2.07, -0.2),
            v(1.5, 0.52, 1.55) * s,
            0.80,
            STEEL,
            PALE,
        );
        m.tier(
            bridge(0., 2.58, -0.2),
            v(0.16, 2., 0.17) * s,
            0.6,
            IRON,
            IRON,
        );
        m.tier(
            bridge(0., 3.7, -0.2),
            v(2.15, 0.12, 0.15) * s,
            1.,
            PALE,
            PALE,
        );
    } else {
        let (aft, height) = PATROL_STAFF;
        m.tier(v(0., d, aft), v(0.13, height, 0.13), 0.7, IRON, PALE);
    }
    let stacks: &[f32] = match class {
        Freighter => &[-8.],
        Patrol => &[2.7],
        // One funnel between the bridge and the after gun.
        Destroyer => &[2.3],
        Cruiser => &[3.0, 5.35],
        Battleship => &[4.4, 9.2],
        Parliament => &[-13., 8., 17.],
        Regent => &[-27., -14., 14., 27.],
    };
    for &z in stacks {
        // The flagship has split, flared scarlet funnel caps.
        let w = if class.is_flagship() { 1.42 } else { 1.2 };
        let foot = v(0., d, z);
        let height = if class.is_flagship() {
            (2.4 + 0.8 * (1. - z.abs() / 20.)) * s
        } else {
            1.65 * s
        };
        m.tier(foot, v(w * s, height, 1.48 * s), 0.89, STEEL, IRON);
        if class.is_flagship() {
            for y in [0.32, 0.78] {
                m.tier(
                    foot + Vec3::Y * height * y,
                    v(w * s, 0.14 * s, 1.48 * s),
                    1.,
                    PALE,
                    IRON,
                );
            }
        }
        m.tier(
            foot + Vec3::Y * height,
            v(w * s, 0.38 * s, 1.5 * s),
            1.10,
            RED,
            IRON,
        );
        m.tier(
            foot + Vec3::Y * (height + 0.38 * s),
            v(0.9 * w * s, 0.14 * s, 1.2 * s),
            1.,
            IRON,
            IRON,
        );
    }
    if matches!(class, Battleship | Parliament | Regent) {
        // Exposed deck-edge armour with light caps and repeated red identification stripes.
        for side in [-1., 1.] {
            for bay in -3_i8..=3 {
                let t = f32::from(bay) / 4.;
                let x = side * p.hull.radius * (0.93 - 0.20 * t.abs());
                let z = t * p.hull.stern;
                let base = v(x, d * 0.4, z);
                let length = p.hull.stern * 0.21;
                m.tier(base, v(0.48 * s, d * 0.67, length), 0.96, IRON, PALE);
                let rail = base + v(side * 0.20 * s, d * 0.48, 0.);
                m.tier(rail, v(0.14 * s, 0.08 * s, length), 1., PALE, PALE);
                for dz in [-0.24, 0.24] {
                    m.tier(
                        rail + v(side * 0.08 * s, -0.3 * s, length * dz),
                        v(0.10 * s, 0.20 * s, 0.32 * s),
                        0.85,
                        GLASS,
                        PALE,
                    );
                }
                if bay % 2 == 0 {
                    m.tier(
                        base + v(side * 0.24 * s, 0., 0.),
                        v(0.14 * s, d * 0.76, 0.6 * s),
                        1.,
                        RED,
                        PALE,
                    );
                }
            }
        }
    }
    if class.is_flagship() {
        // A high signal gantry carries the forked standard over the three stacks.
        // Glazed galleries under the fork give the flagship a bridge visible above its batteries.
        m.tier(
            bridge(0., 2.10, 0.),
            v(2.75, 0.72, 3.) * s,
            0.89,
            PALE,
            IRON,
        );
        m.tier(
            bridge(0., 2.82, 0.),
            v(2.85, 0.48, 3.05) * s,
            0.96,
            GLASS,
            IRON,
        );
        m.tier(
            bridge(0., 3.30, 0.),
            v(3.15, 0.18, 3.28) * s,
            0.93,
            RED,
            PALE,
        );
        for side in [-1., 1.] {
            for z in [-0.8, 0., 0.8] {
                m.tier(
                    bridge(side * 1.40, 2.8, z),
                    v(0.12, 0.54, 0.12) * s,
                    1.,
                    PALE,
                    PALE,
                );
            }
        }
        let h = d + 5.0 * s;
        for x in [-2.7, 2.7] {
            m.tier(
                v(x, d + 3.5 * s, 0.),
                v(0.53, h - d - 3.5 * s, 0.68),
                0.68,
                IRON,
                PALE,
            );
            m.tier(v(x, h, 0.), v(0.32, 2.5, 0.32), 0.8, PALE, PALE);
        }
        m.tier(v(0., h - 1.0, 0.), v(7.2, 0.85, 2.1), 1., RED, IRON);
    }
    if class == Regent {
        // A second armoured bridge and the high gold crossbar distinguish the Regent at range.
        for z in [-40., 43.] {
            m.tier(v(0., d, z), v(12., 5., 13.), 0.75, STEEL, IRON);
            m.tier(v(0., d + 5., z), v(9.3, 2., 8.), 0.91, GLASS, PALE);
            m.tier(v(0., d + 7., z), v(10., 0.9, 8.8), 0.97, GOLD, PALE);
        }
        for x in [-6.2, 6.2] {
            m.0.tube(v(x, d, 0.), v(x * 0.56, d + 24., 0.), 1.2, 0.55, STEEL);
        }
        m.tier(v(0., d + 23., 0.), v(17., 1.7, 3.5), 0.93, GOLD, IRON);
        m.tier(v(0., d + 24.7, 0.), v(4.6, 2.5, 3.4), 0.80, GLASS, PALE);
    }
    if class == Freighter {
        for fore in [4.5, 8.4, -5.5, -9.4] {
            for x in [-1.1, 1.1] {
                m.tier(v(x, d, -fore), v(1.9, 1.7, 3.5), 1., GOLD, PALE);
                m.tier(v(x, d + 1.7, -fore), v(0.28, 0.10, 3.5), 1., IRON, IRON);
            }
        }
        m.tier(v(0., d, 11.8), v(0.35, 6.6, 0.35), 1., GOLD, GOLD);
        m.tier(v(0., d + 6.3, 9.4), v(0.36, 0.32, 5.1), 1., GOLD, GOLD);
    }
    m.0.finish()
}

pub fn gun(class: EnemyClass) -> Mesh {
    let mut m = Metal::default();
    let v = Vec3::new;
    let stripe = if class.is_flagship() { GOLD } else { RED };
    m.tier(Vec3::ZERO, v(2.3, 0.24, 2.3), 1., IRON, IRON);
    m.tier(v(0., 0.22, 0.), v(2.22, 0.72, 2.05), 0.88, STEEL, PALE);
    m.tier(v(0., 0.92, 0.), v(1.98, 0.31, 1.86), 0.86, stripe, stripe);
    m.tier(v(0., 1.22, 0.), v(1.67, 0.14, 1.57), 0.92, PALE, PALE);
    let barrels = class.profile().barrels;
    for i in 0..barrels {
        let x = (f32::from(i) - f32::from(barrels - 1) * 0.5) * 0.55;
        m.tier(v(x, 0.58, -1.9), v(0.26, 0.27, 2.6), 1., IRON, STEEL);
        m.tier(v(x, 0.53, -1.24), v(0.40, 0.39, 1.15), 0.95, STEEL, PALE);
        m.tier(v(x, 0.56, -3.05), v(0.32, 0.32, 0.31), 1., PALE, PALE);
        m.tier(v(x, 0.62, -3.205), v(0.20, 0.18, 0.04), 1., IRON, IRON);
    }
    m.0.finish()
}
