//! Low-poly fleet fittings. Static deckwork and each moving gun use one mesh apiece.
use crate::scene::{Facets, fittings};
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

/// What fire leaves of paint: the colour a wrecked ship's upperworks burn down to.
const SOOT: Color = Color::srgb(0.085, 0.08, 0.075);

/// `paint` scorched toward soot by `burnt`, from 0 for none to 1 for black.
fn scorched(paint: Color, burnt: f32) -> Color {
    let (a, b) = (paint.to_srgba(), SOOT.to_srgba());
    let k = burnt.clamp(0., 1.);
    Color::srgb(
        a.red + (b.red - a.red) * k,
        a.green + (b.green - a.green) * k,
        a.blue + (b.blue - a.blue) * k,
    )
}

/// A turn of `angle` radians about the line through `pivot` along `axis`.
fn bent(pivot: Vec3, axis: Vec3, angle: f32) -> bevy::math::Affine3A {
    bevy::math::Affine3A::from_translation(pivot)
        * bevy::math::Affine3A::from_axis_angle(axis.normalize(), angle)
        * bevy::math::Affine3A::from_translation(-pivot)
}

const HOUSE: [(f32, f32); 8] = [
    (-0.35, -0.5),
    (0.35, -0.5),
    (0.5, -0.34),
    (0.5, 0.34),
    (0.35, 0.5),
    (-0.35, 0.5),
    (-0.5, 0.34),
    (-0.5, -0.34),
];

#[derive(Default)]
struct Metal(Facets);
impl Metal {
    /// An eight-sided casting. Sizes are full beam and length.
    fn tier(&mut self, center: Vec3, size: Vec3, taper: f32, wall: Color, roof: Color) {
        self.tier_bent(
            center,
            size,
            taper,
            wall,
            roof,
            bevy::math::Affine3A::IDENTITY,
        );
    }
    /// As [`Metal::tier`], moved by `bend`: knocked askew by a shell.
    fn tier_bent(
        &mut self,
        center: Vec3,
        size: Vec3,
        taper: f32,
        wall: Color,
        roof: Color,
        bend: bevy::math::Affine3A,
    ) {
        let foot = HOUSE.map(|(x, z)| center + Vec3::new(x * size.x, 0., z * size.z));
        let cap = center + Vec3::Y * size.y;
        let top = foot.map(|p| bend.transform_point3(cap + (p - center) * taper));
        let (foot, cap) = (
            foot.map(|p| bend.transform_point3(p)),
            bend.transform_point3(cap),
        );
        for i in 0..8 {
            let j = (i + 1) % 8;
            self.0.triangle([foot[i], top[j], foot[j]], wall);
            self.0.triangle([foot[i], top[i], top[j]], wall);
            self.0.triangle([cap, top[j], top[i]], roof);
        }
    }
    /// A row of distinct panes follows the splay of the bridge front and wings.
    /// The brow reflects the sky; the sill picks up the darker deck below.
    fn windows(&mut self, sill: Vec3, size: Vec3, burnt: f32) {
        let frame = scorched(PALE, burnt);
        let low = scorched(GLASS, burnt);
        let sky = scorched(Color::srgb_u8(104, 145, 151), burnt);
        self.tier(sill, size, 1.04, frame, IRON);
        for i in 0..HOUSE.len() {
            let face = |(x, z): (f32, f32)| Vec3::new(x * size.x, 0., z * size.z);
            let a = face(HOUSE[i]);
            let b = face(HOUSE[(i + 1) % HOUSE.len()]);
            let out = Vec3::Y.cross(b - a).normalize() * size.y * 0.012;
            let at = |t, h| sill + a.lerp(b, t) * (1. + 0.04 * h) + Vec3::Y * size.y * h + out;
            let panes = if i % 2 == 0 { 3 } else { 1 };
            let margin = (0.055 * size.y / (b - a).length()).min(0.08);
            for pane in 0..panes {
                let left = pane as f32 / panes as f32 + margin;
                let right = (pane + 1) as f32 / panes as f32 - margin;
                let [a, b, c, d] = [
                    at(left, 0.11),
                    at(left, 0.90),
                    at(right, 0.90),
                    at(right, 0.11),
                ];
                if burnt > 0. && (pane + i) % 3 != 0 {
                    self.0.quad([a, b, c, d], SOOT);
                    continue;
                }
                self.0.triangle_colors([a, b, c], [low, sky, sky]);
                self.0.triangle_colors([a, c, d], [low, sky, low]);
            }
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

/// Everything she carries above her deck in one mesh: her deckwork with her upperworks
/// standing sound on it. Its top is her masthead.
pub fn superstructure(class: EnemyClass) -> Mesh {
    match class {
        Coaster => coaster(),
        TorpedoBoat => torpedo_boat(),
        _ => {
            let mut m = Metal::default();
            deckwork_into(&mut m, class);
            upperworks_into(&mut m, class, 0);
            m.0.finish()
        }
    }
}

/// What she carries above her deck that shells don't knock about: her funnels, and a
/// freighter's deck cargo. A coaster's and a torpedo boat's take in their bridges too.
pub fn deckwork(class: EnemyClass) -> Mesh {
    match class {
        Coaster => coaster(),
        TorpedoBoat => torpedo_boat(),
        _ => {
            let mut m = Metal::default();
            deckwork_into(&mut m, class);
            m.0.finish()
        }
    }
}

/// Her bridge, its mast and any spotting top or signal gantry over it, at `stage`: 0
/// sound, 1 battered, scorched, its windows blown in and its mast leaning, and 2
/// wrecked, shot down to a blackened stump. `None` for a class whose bridge is part
/// of its deckwork.
pub fn upperworks(class: EnemyClass, stage: u8) -> Option<Mesh> {
    if matches!(class, Coaster | TorpedoBoat) {
        return None;
    }
    let mut m = Metal::default();
    upperworks_into(&mut m, class, stage);
    Some(m.0.finish())
}

/// Whether her colours fly from her upperworks, so they go when those are battered.
/// A patrol boat wears hers on a staff aft, and a coaster and a torpedo boat theirs
/// on their deckwork.
pub fn colours_aloft(class: EnemyClass) -> bool {
    !matches!(class, Patrol | Coaster | TorpedoBoat)
}

fn upperworks_into(m: &mut Metal, class: EnemyClass, stage: u8) {
    let p = class.profile();
    let v = Vec3::new;
    let d = p.deck;
    let s = p.gun_scale;
    let bridge = |x, y, z| v(x * s, d + y * s, z * s);
    // How far fire has blackened her paint, and how far her bridge house is crushed.
    let burnt = [0., 0.5, 0.85][usize::from(stage.min(2))];
    let paint = |c: Color| scorched(c, burnt);
    let crushed = if stage >= 2 { 0.6 } else { 1. };
    // A patrol launch has a short cabin. On the larger ships a long bridge trunk
    // steps up from the forward deck, with open lookout wings projecting either side.
    let trunk = match class {
        Patrol => 3.05,
        Freighter => 3.65,
        _ => 4.75,
    };
    m.tier(
        bridge(0., 0., -0.25),
        v(2.50, 0.72 * crushed, trunk) * s,
        0.97,
        paint(STEEL),
        paint(IRON),
    );
    m.tier(
        bridge(0., 0.70 * crushed, -0.25),
        v(2.56, 0.12 * crushed, trunk + 0.10) * s,
        1.,
        paint(PALE),
        paint(STEEL),
    );
    m.tier(
        bridge(0., 0.82 * crushed, -0.27),
        v(2.24, 0.45 * crushed, 2.78) * s,
        0.94,
        paint(STEEL),
        paint(IRON),
    );
    // Watertight doors and the slats of the intake grilles give the crew deck scale.
    for side in [-1., 1.] {
        let wall = |y, z, relief| {
            bridge(
                side * (1.25 * (1. - 0.03 * y / 0.72) + relief),
                y * crushed,
                z,
            )
        };
        let panel = |y, top, z: f32, wide: f32, r| {
            let mut q = [
                wall(y, z - wide, r),
                wall(top, z - wide, r),
                wall(top, z + wide, r),
                wall(y, z + wide, r),
            ];
            if side < 0. {
                q.reverse();
            }
            q
        };
        m.0.quad(panel(0.015, 0.65, -0.74, 0.22, 0.010), paint(IRON));
        m.0.quad(panel(0.045, 0.62, -0.74, 0.18, 0.019), paint(STEEL));
        m.0.quad(panel(0.31, 0.35, -0.80, 0.065, 0.024), paint(PALE));
        m.0.quad(panel(0.21, 0.59, 0.52, 0.35, 0.012), paint(IRON));
        for i in 0..4 {
            let y = 0.24 + 0.088 * i as f32;
            m.0.quad(panel(y, y + 0.034, 0.52, 0.32, 0.023), paint(STEEL));
        }
    }
    let roof = 1.25 * crushed;
    if stage < 2 {
        // Solid splinter screens at the wing ends, open to the rear.
        let span = if class == Patrol { 1.40 } else { 1.61 };
        m.tier(
            bridge(0., 1.25, -0.22),
            v(span * 2., 0.10, 2.46) * s,
            0.99,
            paint(PALE),
            paint(IRON),
        );
        for side in [-1., 1.] {
            m.tier(
                bridge(side * (span - 0.06), 1.35, -0.28),
                v(0.12, 0.25, 1.61) * s,
                0.95,
                paint(STEEL),
                paint(PALE),
            );
        }
        m.windows(bridge(0., 1.35, -0.32), v(2.18, 0.53, 2.31) * s, burnt);
        let buckle = if stage == 1 {
            bent(bridge(-1.2, 1.88, 0.), Vec3::Z, 0.2)
        } else {
            bevy::math::Affine3A::IDENTITY
        };
        m.tier_bent(
            bridge(0., 1.88, -0.32),
            v(2.52, 0.19, 2.64) * s,
            0.94,
            paint(RED),
            paint(PALE),
            buckle,
        );
    } else {
        let torn = bent(bridge(0.8, roof, 0.), v(0.4, 0., 1.), -0.55);
        m.tier_bent(
            bridge(0.3, roof, 0.3),
            v(1.5, 0.12, 1.6) * s,
            1.,
            SOOT,
            paint(RED),
            torn,
        );
    }
    if class == Patrol {
        // Her ensign flies from the stern staff.
    } else if stage < 2 {
        let (platform, mast) = if stage == 1 {
            (
                bent(bridge(0.7, 2.07, -0.2), Vec3::Z, -0.22),
                bent(bridge(0., 2.58, -0.2), v(1., 0., 0.4), 0.75),
            )
        } else {
            (
                bevy::math::Affine3A::IDENTITY,
                bevy::math::Affine3A::IDENTITY,
            )
        };
        m.tier_bent(
            bridge(0., 2.07, -0.2),
            v(1.5, 0.50, 1.55) * s,
            0.78,
            paint(STEEL),
            paint(PALE),
            platform,
        );
        let at = |x, y, z| mast.transform_point3(bridge(x, y, z));
        m.0.tube(
            at(0., 2.55, -0.2),
            at(0., 4.58, -0.2),
            0.11 * s,
            0.041 * s,
            paint(IRON),
        );
        for side in [-1., 1.] {
            let yard = at(side * 1.24, 3.80, -0.2);
            m.0.tube(at(0., 3.75, -0.2), yard, 0.070 * s, 0.039 * s, paint(PALE));
            if stage == 0 {
                m.0.tube(
                    bridge(side * 0.91, 2.00, 0.76),
                    at(0., 3.53, -0.2),
                    0.065 * s,
                    0.048 * s,
                    STEEL,
                );
                m.0.tube(at(0., 4.31, -0.2), yard, 0.022 * s, 0.022 * s, IRON);
            }
        }
    } else {
        // The broken foot of her mast.
        let snapped = bent(bridge(0., roof, -0.2), Vec3::X, 0.3);
        m.tier_bent(
            bridge(0., roof, -0.2),
            v(0.2, 0.55, 0.2) * s,
            0.5,
            SOOT,
            IRON,
            snapped,
        );
    }
    if class.is_flagship() {
        flagship_works(m, class, stage);
    }
    if class == Regent {
        regent_works(m, stage);
    }
}

/// The flagship's glazed galleries and high signal gantry, which carries her forked
/// standard over the three stacks.
fn flagship_works(m: &mut Metal, class: EnemyClass, stage: u8) {
    let p = class.profile();
    let v = Vec3::new;
    let d = p.deck;
    let s = p.gun_scale;
    let bridge = |x, y, z| v(x * s, d + y * s, z * s);
    let burnt = [0., 0.5, 0.85][usize::from(stage.min(2))];
    let paint = |c: Color| scorched(c, burnt);
    // Glazed galleries under the fork give the flagship a bridge visible above its batteries.
    m.tier(
        bridge(0., 2.10, 0.),
        v(2.75, 0.72, 3.) * s,
        0.89,
        paint(PALE),
        IRON,
    );
    if stage >= 2 {
        return;
    }
    m.windows(bridge(0., 2.82, 0.), v(2.85, 0.48, 3.05) * s, burnt);
    let sag = if stage == 0 {
        bevy::math::Affine3A::IDENTITY
    } else {
        bent(bridge(1.6, 3.30, 0.), Vec3::Z, 0.18)
    };
    m.tier_bent(
        bridge(0., 3.30, 0.),
        v(3.15, 0.18, 3.28) * s,
        0.93,
        paint(RED),
        paint(PALE),
        sag,
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
    // Battered, the port leg of the gantry is shot through and the whole of it leans.
    let lean = if stage == 0 {
        bevy::math::Affine3A::IDENTITY
    } else {
        bent(v(2.7, d + 3.5 * s, 0.), Vec3::Z, 0.35)
    };
    for x in [-2.7, 2.7] {
        m.tier_bent(
            v(x, d + 3.5 * s, 0.),
            v(0.53, h - d - 3.5 * s, 0.68),
            0.68,
            IRON,
            PALE,
            lean,
        );
        m.tier_bent(v(x, h, 0.), v(0.32, 2.5, 0.32), 0.8, PALE, PALE, lean);
    }
    m.tier_bent(
        v(0., h - 1.0, 0.),
        v(7.2, 0.85, 2.1),
        1.,
        paint(RED),
        IRON,
        lean,
    );
}

/// The Regent's second armoured bridges and her tall twin-legged tower with its gold
/// crossbar, which mark her out at range.
fn regent_works(m: &mut Metal, stage: u8) {
    let d = Regent.profile().deck;
    let v = Vec3::new;
    let burnt = [0., 0.5, 0.85][usize::from(stage.min(2))];
    let paint = |c: Color| scorched(c, burnt);
    for z in [-40., 43.] {
        let crushed = if stage >= 2 { 0.55 } else { 1. };
        m.tier(
            v(0., d, z),
            v(12., 5. * crushed, 13.),
            0.75,
            paint(STEEL),
            IRON,
        );
        if stage < 2 {
            m.windows(v(0., d + 5., z), v(9.3, 2., 8.), burnt);
            m.tier(v(0., d + 7., z), v(10., 0.9, 8.8), 0.97, paint(GOLD), PALE);
        }
    }
    // The tower's legs: whole, leaning over with the crossbar on them, or snapped off.
    let reach = if stage >= 2 { 9. } else { 24. };
    let lean = if stage == 1 {
        bent(v(0., d, 0.), Vec3::Z, 0.2)
    } else {
        bevy::math::Affine3A::IDENTITY
    };
    for x in [-6.2, 6.2] {
        let (foot, top) = (v(x, d, 0.), v(x * (1. - 0.44 * reach / 24.), d + reach, 0.));
        m.0.tube(
            lean.transform_point3(foot),
            lean.transform_point3(top),
            1.2,
            0.55 + 0.65 * (1. - reach / 24.),
            paint(STEEL),
        );
    }
    if stage < 2 {
        m.tier_bent(
            v(0., d + 23., 0.),
            v(17., 1.7, 3.5),
            0.93,
            paint(GOLD),
            IRON,
            lean,
        );
        m.tier_bent(
            v(0., d + 24.7, 0.),
            v(4.6, 2.5, 3.4),
            0.80,
            paint(GLASS),
            PALE,
            lean,
        );
    }
}

/// Quarterdeck guardwires follow the narrowing stern. Their height stays close to
/// a sailor's waist even on the Regent, giving the eye a scale for the larger hulls.
fn quarterdeck(m: &mut Metal, class: EnemyClass) {
    let p = class.profile();
    let size = (p.hull.radius / 2.75).clamp(0.55, 1.20);
    for side in [-1., 1.] {
        let edge = |(x, z): (f32, f32)| {
            Vec3::new(
                x * (side * 0.97 * p.hull.radius / 2.75),
                p.deck,
                z * p.hull.bow / 16.,
            )
        };
        // Cross only half the transom here; the other side meets it on centreline.
        let outline = shipvivor_encounter::HULL_OUTLINE;
        let points = [
            edge(outline[3]),
            edge(outline[4]),
            edge(outline[5]),
            edge((0., outline[5].1)),
        ];
        fittings::guardwires(&mut m.0, &points, size, PALE);
    }
}

fn deckwork_into(m: &mut Metal, class: EnemyClass) {
    let p = class.profile();
    let v = Vec3::new;
    let d = p.deck;
    let s = p.gun_scale;
    if class == Patrol {
        let (aft, height) = PATROL_STAFF;
        m.tier(v(0., d, aft), v(0.13, height, 0.13), 0.7, IRON, PALE);
    }
    for &z in stacks(class) {
        let width = if class.is_flagship() { 1.42 } else { 1.2 };
        let height = stack_height(class, z) + 0.52 * s;
        fittings::funnel(
            &mut m.0,
            v(0., d, z),
            v(width * s, height, 1.52 * s),
            RED,
            class.is_boss(),
        );
    }
    quarterdeck(m, class);
    if matches!(class, Destroyer | Cruiser | Battleship | Commodore) {
        // The boat deck sits between the bridge and the after battery. The boats are
        // kept inboard of the secondary mounts on the larger hulls.
        let after = *stacks(class).last().unwrap();
        let beam = p.hull.radius;
        m.tier(
            v(0., d, after * 0.5),
            v(beam * 1.36, 0.30 * s, after + 1.7 * s),
            0.96,
            STEEL,
            IRON,
        );
        let z = if class == Destroyer {
            1.96
        } else {
            after * 0.59
        };
        for side in [-1., 1.] {
            fittings::ship_boat(
                &mut m.0,
                v(side * beam * 0.55, d + 0.30 * s, z),
                v(0.63 * s, 0.56 * s, 2.55 * s),
            );
        }
    }
    // A capital ship's deck-edge armour hangs in strakes of its own (see `strake`),
    // which can be shot off her.
    if class == Freighter {
        for fore in [4.5, 8.4, -5.5, -9.4] {
            for x in [-1.1, 1.1] {
                m.tier(v(x, d, -fore), v(1.9, 1.7, 3.5), 1., GOLD, PALE);
                m.tier(v(x, d + 1.7, -fore), v(0.28, 0.10, 3.5), 1., IRON, IRON);
            }
        }
        // Her derrick, right aft.
        m.tier(v(0., d, 11.8), v(0.35, 6.6, 0.35), 1., GOLD, GOLD);
        m.tier(v(0., d + 6.3, 9.4), v(0.36, 0.32, 5.1), 1., GOLD, GOLD);
    }
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
    m.windows(v(0., d + 0.72, 1.8), v(1.4, 0.27, 1.02), 0.);
    m.tier(v(0., d + 0.99, 1.8), v(1.66, 0.11, 1.26), 0.95, RED, PALE);
    // A stubby funnel behind it, with a red band under the cap.
    fittings::funnel(&mut m.0, v(0., d, 2.8), v(0.44, 1.21, 0.5), RED, false);
    quarterdeck(&mut m, Coaster);
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
    m.windows(v(0., d + 0.7, -1.7), v(1.3, 0.26, 1.2), 0.);
    m.tier(v(0., d + 0.96, -1.7), v(1.55, 0.1, 1.45), 0.95, RED, PALE);
    // A raked funnel abaft it with a red band under the cap.
    fittings::funnel(&mut m.0, v(0., d, 0.42), v(0.62, 1.36, 0.80), RED, false);
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
    m.0.tube(Vec3::ZERO, Vec3::Y * 0.26, 1.09, 1.09, IRON);
    // The deep mantlet sits low on the sloping face. The roof sweeps back above
    // it, while the rear wall stands nearly plumb over the turntable.
    let lower = HOUSE.map(|(x, z)| v(x * 2.27, 0.24, z * 2.24));
    let roof = HOUSE.map(|(x, z)| {
        v(
            x * 1.85,
            1.28 + 0.08 * z,
            if z < 0. { z * 1.33 } else { z * 2.13 },
        )
    });
    for i in 0..HOUSE.len() {
        let j = (i + 1) % HOUSE.len();
        let at = |k: usize, t| lower[k].lerp(roof[k], t);
        for (lo, hi, paint) in [(0., 0.72, STEEL), (0.72, 0.82, stripe), (0.82, 1., STEEL)] {
            m.0.quad([at(i, lo), at(i, hi), at(j, hi), at(j, lo)], paint);
        }
        m.0.triangle([v(0., 1.28, 0.), roof[j], roof[i]], PALE);
    }
    m.tier(v(0.38, 1.29, 0.43), v(0.48, 0.06, 0.45), 0.93, IRON, STEEL);
    let width = 0.42 + 0.55 * f32::from(barrels.saturating_sub(1));
    m.tier(v(0., 0.50, -0.91), v(width, 0.45, 0.30), 0.98, IRON, STEEL);
    for i in 0..barrels {
        let x = (f32::from(i) - f32::from(barrels - 1) * 0.5) * 0.55;
        fittings::gun_tube(&mut m.0, v(x, 0.73, -0.85), 2.37, 0.21);
    }
    m.0.finish()
}
