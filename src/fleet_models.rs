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
/// Canvas lashed over a coaster's deck cargo.
const TARP: Color = Color::srgb(0.42, 0.45, 0.30);
/// A coaster flies her colours from her masthead: its distance abaft amidships and its
/// height above the deck.
const COASTER_MAST: (f32, f32) = (-2.2, 2.7);
/// A patrol boat has no mast, so she wears her flag on a staff aft: its distance abaft
/// amidships and its height above the deck.
const PATROL_STAFF: (f32, f32) = (5.5, 2.4);
/// A torpedo boat wears hers on a short staff right aft, over the tube's breech.
const TORPEDO_BOAT_STAFF: (f32, f32) = (5.1, 1.9);

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
    } else if class == Coaster {
        let (aft, height) = COASTER_MAST;
        (Vec3::new(0., d + height - 0.05, aft), 0.3)
    } else if class == TorpedoBoat {
        let (aft, height) = TORPEDO_BOAT_STAFF;
        (Vec3::new(0., d + height - 0.05, aft), 0.3)
    } else {
        (Vec3::new(0., d + 4.55 * s, -0.2 * s), 0.44 * s)
    }];
    if class.is_flagship() {
        let h = d + 5.0 * s;
        flags.extend([-2.7, 2.7].map(|x| (Vec3::new(x, h + 2.45, 0.), 1.1)));
    }
    flags
}

/// Where along the hull each funnel stands, in metres abaft amidships, for the classes
/// whose deckwork is built around the common bridge. The coaster and the torpedo boat
/// carry theirs in their own deckwork.
fn stacks(class: EnemyClass) -> &'static [f32] {
    match class {
        Freighter => &[-8.],
        Coaster | TorpedoBoat => &[],
        Patrol => &[2.7],
        // One funnel between the bridge and the after gun.
        Destroyer => &[2.3],
        Cruiser => &[3.0, 5.35],
        Battleship => &[4.4, 9.2],
        // Close together between the bridge and the superfiring after pair.
        Commodore => &[0.8, 5.],
        Parliament => &[-13., 8., 17.],
        Regent => &[-27., -14., 14., 27.],
    }
}

/// The height of a funnel's casing at `z`, under its red band and cap.
fn stack_height(class: EnemyClass, z: f32) -> f32 {
    let s = class.profile().gun_scale;
    if class.is_flagship() {
        (2.4 + 0.8 * (1. - z.abs() / 20.)) * s
    } else {
        1.65 * s
    }
}

/// The open lip of each of a class's funnels, in hull coordinates (bow towards -z), and
/// the width of its mouth. Smoke rises from here.
pub fn funnels(class: EnemyClass) -> Vec<(Vec3, f32)> {
    let p = class.profile();
    let (d, s) = (p.deck, p.gun_scale);
    match class {
        Coaster => vec![(Vec3::new(0., d + 1.21, 2.8), 0.4)],
        TorpedoBoat => vec![(Vec3::new(0., d + 1.36, 0.42), 0.52)],
        _ => stacks(class)
            .iter()
            .map(|&z| {
                let lip = d + stack_height(class, z) + 0.52 * s;
                (Vec3::new(0., lip, z), 1.08 * s)
            })
            .collect(),
    }
}

pub fn superstructure(class: EnemyClass) -> Mesh {
    if class == Coaster {
        return coaster();
    }
    if class == TorpedoBoat {
        return torpedo_boat();
    }
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
    for &z in stacks(class) {
        // The flagship has split, flared scarlet funnel caps.
        let w = if class.is_flagship() { 1.42 } else { 1.2 };
        let foot = v(0., d, z);
        let height = stack_height(class, z);
        m.tier(foot, v(w * s, height, 1.48 * s), 0.89, STEEL, IRON);
        // A boss's funnels are banded in pale paint.
        if class.is_boss() {
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
    // A capital ship's deck-edge armour hangs in strakes of its own (see `strake`),
    // which can be shot off her.
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

/// One of the strakes of exposed deck-edge armour along a battleship's or a flagship's
/// side, by its index as [`EnemyClass::strake_station`] counts them: a dark plate with
/// a light cap, two ports, and on every other strake a red identification stripe. It
/// is built about its middle, so that one shot loose tumbles about it.
pub fn strake(class: EnemyClass, index: usize) -> Mesh {
    let mut m = Metal::default();
    let p = class.profile();
    let v = Vec3::new;
    let (d, s) = (p.deck, p.gun_scale);
    let (abeam, _, length) = class.strake_station(index);
    let side = abeam.signum();
    let (_, rise) = shipvivor_encounter::STRAKE_RISE;
    let base = v(0., -0.5 * d * rise, 0.);
    m.tier(base, v(0.48 * s, d * rise, length), 0.96, IRON, PALE);
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
    // The middle strake of each side wears one, and every other one from it.
    if !(index % shipvivor_encounter::STRAKES_A_SIDE).is_multiple_of(2) {
        m.tier(
            base + v(side * 0.24 * s, 0., 0.),
            v(0.14 * s, d * 0.76, 0.6 * s),
            1.,
            RED,
            PALE,
        );
    }
    m.0.finish()
}

/// A small coastal trader: a raised forecastle, a mast and derrick, a deck cargo of
/// crates, drums and a tarpaulin-covered stack, and her wheelhouse and funnel aft. The
/// wheelhouse roof and funnel band wear the same red as every enemy bridge.
fn coaster() -> Mesh {
    let mut m = Metal::default();
    let p = Coaster.profile();
    let v = Vec3::new;
    let d = p.deck;
    // The forecastle stands a little above the well deck.
    m.tier(v(0., d - 0.05, -2.9), v(1.22, 0.36, 1.25), 0.9, STEEL, PALE);
    // The mast, its crosstree, and the derrick topped up over the hold.
    let (mast, height) = COASTER_MAST;
    m.tier(v(0., d, mast), v(0.14, height, 0.14), 0.65, IRON, PALE);
    m.tier(v(0., d + 2.0, mast), v(0.95, 0.07, 0.08), 1., PALE, PALE);
    m.0.tube(
        v(0., d + 0.35, mast + 0.12),
        v(0., d + 1.75, -0.55),
        0.055,
        0.035,
        IRON,
    );
    // Crates two abreast, lashed down, with canvas over the after stack and drums abaft it.
    for z in [-1.2, 0.15] {
        for x in [-0.42, 0.42] {
            m.tier(v(x, d, z), v(0.78, 0.62, 1.2), 1., GOLD, PALE);
        }
        m.tier(v(0., d + 0.62, z), v(1.62, 0.05, 0.16), 1., IRON, IRON);
    }
    m.tier(v(0., d + 0.62, -1.15), v(0.82, 0.48, 1.05), 1., GOLD, PALE);
    m.tier(v(0., d + 0.62, 0.2), v(1.3, 0.42, 1.1), 0.82, TARP, TARP);
    for x in [-0.8, 0.8] {
        m.tier(v(x, d, 0.98), v(0.36, 0.5, 0.36), 1., IRON, PALE);
    }
    // The wheelhouse aft: steel walls, a band of windows and a red roof.
    m.tier(v(0., d, 1.8), v(1.55, 0.72, 1.15), 0.94, STEEL, PALE);
    m.tier(v(0., d + 0.72, 1.8), v(1.4, 0.27, 1.02), 1., GLASS, IRON);
    m.tier(v(0., d + 0.99, 1.8), v(1.66, 0.11, 1.26), 0.95, RED, PALE);
    // A stubby funnel behind it, with a red band under the cap.
    m.tier(v(0., d, 2.8), v(0.44, 0.92, 0.5), 0.92, STEEL, IRON);
    m.tier(v(0., d + 0.92, 2.8), v(0.42, 0.22, 0.48), 1.06, RED, IRON);
    m.tier(v(0., d + 1.14, 2.8), v(0.4, 0.07, 0.45), 1., IRON, IRON);
    m.0.finish()
}

/// A torpedo boat: low and lean, a turtle-backed forecastle, a small bridge with the
/// enemy's red roof, one raked funnel and the single long tube on its mount aft,
/// trained fore and aft, with the ensign staff over its breech.
fn torpedo_boat() -> Mesh {
    let mut m = Metal::default();
    let p = TorpedoBoat.profile();
    let v = Vec3::new;
    let d = p.deck;
    // The turtleback sheds the sea she throws up at speed.
    m.tier(v(0., d - 0.1, -4.), v(1.5, 0.34, 3.), 0.7, STEEL, PALE);
    // The bridge: steel walls, a band of windows and a red roof.
    m.tier(v(0., d, -1.7), v(1.45, 0.7, 1.35), 0.92, STEEL, PALE);
    m.tier(v(0., d + 0.7, -1.7), v(1.3, 0.26, 1.2), 1., GLASS, IRON);
    m.tier(v(0., d + 0.96, -1.7), v(1.55, 0.1, 1.45), 0.95, RED, PALE);
    // A raked funnel abaft it with a red band under the cap.
    m.tier(v(0., d, 0.3), v(0.62, 1.05, 0.8), 0.86, STEEL, IRON);
    m.tier(v(0., d + 1.05, 0.38), v(0.56, 0.24, 0.72), 1.04, RED, IRON);
    m.tier(v(0., d + 1.29, 0.42), v(0.52, 0.07, 0.66), 1., IRON, IRON);
    // The tube on its turntable, its muzzle forward and its breech over the stern.
    m.tier(v(0., d, 2.9), v(0.95, 0.32, 0.95), 0.9, IRON, PALE);
    m.0.tube(v(0., d + 0.5, 1.1), v(0., d + 0.5, 4.7), 0.25, 0.25, IRON);
    m.tier(v(0., d + 0.25, 4.6), v(0.58, 0.5, 0.3), 1., STEEL, PALE);
    let (aft, height) = TORPEDO_BOAT_STAFF;
    m.tier(v(0., d, aft), v(0.11, height, 0.11), 0.7, IRON, PALE);
    m.0.finish()
}

/// One of her turrets, with `barrels` guns side by side: from one on a light ship to
/// four on an ultra heavy one.
pub fn gun(class: EnemyClass, barrels: u8) -> Mesh {
    let mut m = Metal::default();
    let v = Vec3::new;
    // A boss's turrets carry gold bands, the rest red.
    let stripe = if class.is_boss() { GOLD } else { RED };
    m.tier(Vec3::ZERO, v(2.3, 0.24, 2.3), 1., IRON, IRON);
    m.tier(v(0., 0.22, 0.), v(2.22, 0.72, 2.05), 0.88, STEEL, PALE);
    m.tier(v(0., 0.92, 0.), v(1.98, 0.31, 1.86), 0.86, stripe, stripe);
    m.tier(v(0., 1.22, 0.), v(1.67, 0.14, 1.57), 0.92, PALE, PALE);
    for i in 0..barrels {
        let x = (f32::from(i) - f32::from(barrels - 1) * 0.5) * 0.55;
        m.tier(v(x, 0.58, -1.9), v(0.26, 0.27, 2.6), 1., IRON, STEEL);
        m.tier(v(x, 0.53, -1.24), v(0.40, 0.39, 1.15), 0.95, STEEL, PALE);
        m.tier(v(x, 0.56, -3.05), v(0.32, 0.32, 0.31), 1., PALE, PALE);
        m.tier(v(x, 0.62, -3.205), v(0.20, 0.18, 0.04), 1., IRON, IRON);
    }
    m.0.finish()
}
