//! Painted deck fittings, batched into the ship's existing meshes.
use super::Facets;
use bevy::prelude::*;

const IRON: Color = Color::srgb(0.18, 0.22, 0.24);
const SOOT: Color = Color::srgb(0.075, 0.087, 0.093);
const STEEL: Color = Color::srgb(0.40, 0.48, 0.50);
const LIP: Color = Color::srgb(0.48, 0.51, 0.50);

/// A raked oval uptake. The lip is directly above `foot`, where the smoke starts.
pub(crate) fn funnel(mesh: &mut Facets, foot: Vec3, size: Vec3, livery: Color, fleet_bands: bool) {
    raked_funnel(mesh, foot, size, livery, fleet_bands, 0.17);
}

/// [`funnel`] with its foot `rake` of its length forward of its lip.
pub(crate) fn raked_funnel(
    mesh: &mut Facets,
    foot: Vec3,
    size: Vec3,
    livery: Color,
    fleet_bands: bool,
    rake: f32,
) {
    let point = |angle: f32, h: f32, width: f32| {
        let (sin, cos) = angle.sin_cos();
        foot + Vec3::new(
            size.x * 0.5 * cos * width,
            size.y * h,
            size.z * (0.5 * sin * width - (1. - h) * rake),
        )
    };
    let course = [
        (0., 1.03, IRON),
        (0.065, 1.03, STEEL),
        (0.10, 0.95, STEEL),
        (0.34, 0.92, if fleet_bands { LIP } else { STEEL }),
        (0.38, 0.92, STEEL),
        (0.71, 0.88, livery),
        (0.83, 0.87, IRON),
        (0.92, 0.91, SOOT),
        (0.96, 0.99, IRON),
        (1., 0.99, LIP),
        (1., 0.80, SOOT),
        (0.77, 0.76, SOOT),
    ];
    for i in 0..24 {
        let angle = |j| std::f32::consts::TAU * j as f32 / 24.;
        let (a, b) = (angle(i), angle(i + 1));
        for pair in course.windows(2) {
            let [(y, w, paint), (h, r, _)] = [pair[0], pair[1]];
            mesh.quad(
                [
                    point(a, y, w),
                    point(a, h, r),
                    point(b, h, r),
                    point(b, y, w),
                ],
                paint,
            );
        }
        let (h, w, _) = course[course.len() - 1];
        mesh.triangle([point(a, h, 0.), point(b, h, w), point(a, h, w)], SOOT);
    }
    // Narrow steam pipe runs up the forward cheek, strapped to the casing.
    let pipe = |h| point(std::f32::consts::FRAC_PI_2 * 3., h, 0.98);
    let r = size.x * 0.046;
    mesh.tube(pipe(0.06), pipe(0.94), r, r, IRON);
    for h in [0.25, 0.65, 0.91] {
        mesh.tube(pipe(h), pipe(h + 0.026), r * 1.28, r * 1.28, LIP);
    }
}

/// A stowed launch in two chocks, under a canvas cover with two lashings.
pub(crate) fn ship_boat(mesh: &mut Facets, foot: Vec3, size: Vec3) {
    let at = |x, y, z| foot + Vec3::new(x, y, z) * size;
    let canvas = Color::srgb_u8(163, 163, 140);
    let rail = Color::srgb_u8(123, 93, 64);
    // Half-breadths along the keel. Both ends rise above the waist.
    let sections = [
        (-0.50, 0.013, 0.84),
        (-0.33, 0.34, 0.74),
        (-0.10, 0.49, 0.68),
        (0.16, 0.50, 0.68),
        (0.38, 0.38, 0.73),
        (0.49, 0.24, 0.77),
    ];
    let frame = |(z, w, sheer): (f32, f32, f32), side| {
        [
            at(side * w * 0.24, 0.16, z * 0.93),
            at(side * w * 0.81, 0.43, z * 0.98),
            at(side * w, sheer - 0.065, z),
            at(side * w, sheer, z),
            at(0., sheer + w * 0.38, z),
        ]
    };
    let paints = [STEEL, Color::srgb_u8(199, 196, 165), rail, canvas];
    for side in [-1., 1.] {
        for i in 0..sections.len() - 1 {
            let a = frame(sections[i], side);
            let b = frame(sections[i + 1], side);
            for (j, paint) in paints.into_iter().enumerate() {
                let mut q = [a[j], a[j + 1], b[j + 1], b[j]];
                if side < 0. {
                    q.reverse();
                }
                mesh.quad(q, paint);
            }
        }
        for (z, w, sheer) in [sections[2], sections[4]] {
            let edge = at(side * w, sheer, z);
            mesh.tube(
                edge,
                at(0., sheer + 0.38 * w, z),
                size.x * 0.013,
                size.x * 0.013,
                IRON,
            );
        }
    }
    for frames in sections.windows(2) {
        let a = frame(frames[0], -1.)[0];
        let b = frame(frames[0], 1.)[0];
        let c = frame(frames[1], 1.)[0];
        let d = frame(frames[1], -1.)[0];
        mesh.quad([a, b, c, d], IRON);
    }
    for (idx, reverse) in [(0, true), (sections.len() - 1, false)] {
        let left = frame(sections[idx], -1.);
        let right = frame(sections[idx], 1.);
        for j in 0..3 {
            let mut q = [left[j], right[j], right[j + 1], left[j + 1]];
            if reverse {
                q.reverse();
            }
            mesh.quad(q, paints[j]);
        }
        let mut cover = [left[3], right[3], right[4]];
        if reverse {
            cover.reverse();
        }
        mesh.triangle(cover, canvas);
    }
    for z in [-0.24, 0.25] {
        for side in [-1., 1.] {
            mesh.tube(
                at(side * 0.38, 0., z),
                at(side * 0.25, 0.40, z),
                size.x * 0.055,
                size.x * 0.055,
                IRON,
            );
        }
    }
}

/// Two guardwires on spaced stanchions, following the deck edge between chocks.
pub(crate) fn guardwires(mesh: &mut Facets, edge: &[Vec3], size: f32, enamel: Color) {
    let wire = 0.031 * size;
    for segment in edge.windows(2) {
        let (a, b) = (segment[0], segment[1]);
        let posts = (a.distance(b) / (2.1 * size)).ceil() as usize;
        for post in 0..posts {
            let foot = a.lerp(b, post as f32 / posts as f32);
            mesh.tube(
                foot,
                foot + Vec3::Y * 0.74 * size,
                wire * 1.25,
                wire,
                enamel,
            );
        }
        for h in [0.36, 0.72] {
            let rise = Vec3::Y * h * size;
            mesh.tube(a + rise, b + rise, wire, wire, enamel);
        }
    }
}

/// Breakwater's after deckhouse light and taffrail, in a frame that follows a refit.
pub(super) fn quarterdeck() -> Mesh {
    let mut mesh = Facets::default();
    let at = Vec3::new;
    let enamel = Color::srgb_u8(179, 196, 190);
    // The two guardwires pick up the ends of the waist rails, at stock station 8.1.
    let side = |x: f32| {
        [
            at(2.55 * x, 0., 0.),
            at(2.04 * x, 0., 4.6),
            at(1.36 * x, 0., 6.0),
        ]
    };
    let [a, b, c] = side(1.);
    let [f, e, d] = side(-1.);
    guardwires(&mut mesh, &[a, b, c, d, e, f], 1., enamel);
    // A low engine-room skylight, with six panes set in a pitched steel coaming.
    for side in [-1., 1.] {
        let panel = |x: f32, z: f32| at(side * x, 0.51 - 0.26 * x, 3.75 + z);
        let face = |mut q: [Vec3; 4]| {
            if side < 0. {
                q.reverse();
            }
            q
        };
        let eave = |z| panel(0.69, z);
        let foot = |z: f32| at(side * 0.71, -0.02, 3.75 + z);
        mesh.quad(
            face([foot(-0.79), eave(-0.79), eave(0.79), foot(0.79)]),
            STEEL,
        );
        mesh.quad(
            face([panel(0., -0.79), panel(0., 0.79), eave(0.79), eave(-0.79)]),
            enamel,
        );
        for i in 0..3 {
            let z = -0.72 + 0.50 * i as f32;
            let glass = |x, z| panel(x, z) + Vec3::Y * 0.009;
            mesh.quad(
                face([
                    glass(0.05, z),
                    glass(0.05, z + 0.43),
                    glass(0.63, z + 0.43),
                    glass(0.63, z),
                ]),
                Color::srgb_u8(58, 87, 98),
            );
        }
    }
    for z in [-0.79_f32, 0.79] {
        let x = 0.69 * z.signum();
        let edge = |x, y| at(x, y, 3.75 + z);
        mesh.quad(
            [
                edge(-x, -0.02),
                edge(x, -0.02),
                edge(x, 0.33),
                edge(-x, 0.33),
            ],
            STEEL,
        );
        mesh.triangle([edge(-x, 0.33), edge(x, 0.33), edge(0., 0.51)], STEEL);
    }
    mesh.finish()
}

/// Barrel in local mount coordinates, with a reinforced root, tapered chase and recessed bore.
pub(crate) fn gun_tube(mesh: &mut Facets, breech: Vec3, length: f32, radius: f32) {
    let courses = [
        (0., 1., STEEL),
        (0.29, 1., IRON),
        (0.31, 0.76, STEEL),
        (0.48, 0.72, IRON),
        (0.50, 0.60, IRON),
        (0.94, 0.49, STEEL),
        (0.955, 0.57, STEEL),
        (1., 0.54, LIP),
        (1., 0.34, SOOT),
        (0.955, 0.32, SOOT),
    ];
    let at = |i: usize, depth: f32, r: f32| {
        let (s, c) = (std::f32::consts::TAU * i as f32 / 12.).sin_cos();
        breech + Vec3::new(c * r * radius, s * r * radius, -depth * length)
    };
    for i in 0..12 {
        for pair in courses.windows(2) {
            let [(a, r, paint), (b, s, _)] = [pair[0], pair[1]];
            mesh.quad(
                [at(i, a, r), at(i, b, s), at(i + 1, b, s), at(i + 1, a, r)],
                paint,
            );
        }
        mesh.triangle(
            [at(i, 0.955, 0.), at(i + 1, 0.955, 0.32), at(i, 0.955, 0.32)],
            SOOT,
        );
    }
}

const SHIELD: Color = Color::srgb(0.69, 0.75, 0.725);
const SHIELD_ROOF: Color = Color::srgb(0.77, 0.81, 0.78);
const GRATING: Color = Color::srgb(0.25, 0.30, 0.32);
const MANTLET: Color = Color::srgb(0.32, 0.39, 0.41);

/// Height above her deck of a secondary's trunnions: enough that its barrels clear the
/// top of the deck rail when they are trained out over it.
const TRUNNION: f32 = 0.9;
/// Metres ahead of the trunnions of the foot of a secondary's shield.
const SHIELD_FOOT: f32 = 0.44;
/// How far the shield's front corners are bent back, across and aft.
const SHIELD_BEND: f32 = 0.13;
/// Metres ahead of the trunnions of the front of the mantlet, and how far it stands
/// out either side of the outer barrels.
const MANTLET_FRONT: f32 = 0.52;
const MANTLET_SIDE: f32 = 0.13;

/// Metres across from the middle of a shielded secondary to each of its barrels.
fn barrel_spread(barrels: usize) -> &'static [f32] {
    match barrels {
        0 | 1 => &[0.],
        2 => &[-0.18, 0.18],
        3 => &[-0.3, 0., 0.3],
        _ => &[-0.39, -0.13, 0.13, 0.39],
    }
}

/// Half the breadth of the shield over this many barrels.
fn shield_half_width(barrels: usize) -> f32 {
    let outer = barrel_spread(barrels).last().copied().unwrap_or(0.);
    (outer + 0.27).max(0.42)
}

/// Metres from a shielded secondary's pivot to the farthest of its shield, trained any
/// way: what must stay inboard of the deck rail.
pub(crate) fn shield_reach(barrels: usize) -> f32 {
    let w = shield_half_width(barrels);
    let outer = barrel_spread(barrels).last().copied().unwrap_or(0.);
    (w - SHIELD_BEND)
        .hypot(SHIELD_FOOT)
        .max(w.hypot(SHIELD_FOOT - SHIELD_BEND))
        .max((outer + MANTLET_SIDE).hypot(MANTLET_FRONT))
}

/// A face turned away from `inside`, a point within the solid it bounds.
fn outward(mesh: &mut Facets, mut face: [Vec3; 4], inside: Vec3, paint: Color) {
    // The diagonals give the facing even where two corners meet in a point.
    let normal = (face[2] - face[0]).cross(face[3] - face[1]);
    let middle = face.iter().copied().sum::<Vec3>() / 4.;
    if normal.dot(middle - inside) < 0. {
        face.reverse();
    }
    mesh.quad(face, paint);
}

/// A triangle turned away from `inside`.
fn outward_tri(mesh: &mut Facets, [a, mut b, mut c]: [Vec3; 3], inside: Vec3, paint: Color) {
    if (b - a).cross(c - a).dot((a + b + c) / 3. - inside) < 0. {
        std::mem::swap(&mut b, &mut c);
    }
    mesh.triangle([a, b, c], paint);
}

/// A convex block from a loop of four lower corners and the four upper corners above
/// them, each face turned out.
fn block(mesh: &mut Facets, low: [Vec3; 4], high: [Vec3; 4], paint: Color) {
    let inside = low.iter().chain(&high).copied().sum::<Vec3>() / 8.;
    for face in [
        low,
        high,
        [low[0], low[1], high[1], high[0]],
        [low[1], low[2], high[2], high[1]],
        [low[2], low[3], high[3], high[2]],
        [low[3], low[0], high[0], high[3]],
    ] {
        outward(mesh, face, inside, paint);
    }
}

/// A box square to the axes, between two opposite corners.
fn cuboid(mesh: &mut Facets, min: Vec3, max: Vec3, paint: Color) {
    let ring = |y| {
        [
            Vec3::new(min.x, y, min.z),
            Vec3::new(max.x, y, min.z),
            Vec3::new(max.x, y, max.z),
            Vec3::new(min.x, y, max.z),
        ]
    };
    block(mesh, ring(min.y), ring(max.y), paint);
}

/// An upright twelve-sided drum, narrowing or flaring from `r0` at `y0` to `r1` at
/// `y1`, closed at both ends.
fn frustum(mesh: &mut Facets, (y0, r0): (f32, f32), (y1, r1): (f32, f32), paint: Color) {
    let at = |i: usize, y: f32, r: f32| {
        let (s, c) = (std::f32::consts::TAU * i as f32 / 12.).sin_cos();
        Vec3::new(c * r, y, s * r)
    };
    let inside = Vec3::Y * 0.5 * (y0 + y1);
    for i in 0..12 {
        outward(
            mesh,
            [
                at(i, y0, r0),
                at(i + 1, y0, r0),
                at(i + 1, y1, r1),
                at(i, y1, r1),
            ],
            inside,
            paint,
        );
        for (y, r) in [(y0, r0), (y1, r1)] {
            let centre = Vec3::Y * y;
            outward_tri(mesh, [centre, at(i, y, r), at(i + 1, y, r)], inside, paint);
        }
    }
}

/// A secondary of this many barrels side by side, on a pedestal behind a bent shield
/// with a half roof, open at the back, in the frame it trains in: its pivot at her deck
/// and its muzzles towards -z. The barrels clear the top of her rail; the shield stays
/// within [`shield_reach`] of the pivot.
pub(crate) fn shielded_mount(barrels: usize) -> Mesh {
    let mut mesh = Facets::default();
    let v = Vec3::new;
    let spread = barrel_spread(barrels);
    let outer = spread.last().copied().unwrap_or(0.);
    let w = shield_half_width(barrels);
    // The pedestal bolted to the deck, and the platform the crew stand on.
    frustum(&mut mesh, (0., 0.27), (0.06, 0.27), IRON);
    frustum(&mut mesh, (0.06, 0.23), (0.09, 0.21), LIP);
    frustum(&mut mesh, (0.09, 0.19), (0.42, 0.15), STEEL);
    frustum(&mut mesh, (0.42, 0.44), (0.47, 0.44), GRATING);
    // The cradle's side brackets, with a handwheel on each, and the trunnions across.
    for side in [-1., 1.] {
        let (out, inner) = (side * (outer + 0.15), side * (outer + 0.10));
        let ring = |y, z0, z1| {
            [
                v(out, y, z0),
                v(inner, y, z0),
                v(inner, y, z1),
                v(out, y, z1),
            ]
        };
        block(
            &mut mesh,
            ring(0.47, -0.14, 0.2),
            ring(TRUNNION, -0.08, 0.1),
            STEEL,
        );
        mesh.tube(
            v(out, 0.7, 0.06),
            v(out + side * 0.05, 0.7, 0.06),
            0.075,
            0.075,
            IRON,
        );
    }
    mesh.tube(
        v(-(outer + 0.19), TRUNNION, 0.),
        v(outer + 0.19, TRUNNION, 0.),
        0.055,
        0.055,
        IRON,
    );
    // Each barrel with its breech block and the recoil cylinder above it.
    for &x in spread {
        gun_tube(&mut mesh, v(x, TRUNNION, 0.2), 2.0, 0.08);
        cuboid(
            &mut mesh,
            v(x - 0.085, TRUNNION - 0.08, 0.2),
            v(x + 0.085, TRUNNION + 0.08, 0.42),
            IRON,
        );
        mesh.tube(
            v(x, TRUNNION + 0.11, 0.16),
            v(x, TRUNNION + 0.11, -0.3),
            0.04,
            0.04,
            STEEL,
        );
    }
    // The shield's face leans back from its foot, its corners bent back to the cheeks.
    // The cheeks run back level with the roof, then fall away to the open rear.
    let (foot, top, plate) = (0.46, 1.24, 0.04);
    let lean = 0.13;
    let bend = SHIELD_BEND;
    let front = |y: f32| -SHIELD_FOOT + lean * (y - foot) / (top - foot);
    let face = |y: f32| {
        let z = front(y);
        [
            v(-(w - bend), y, z),
            v(w - bend, y, z),
            v(w - bend, y, z + plate),
            v(-(w - bend), y, z + plate),
        ]
    };
    block(&mut mesh, face(foot), face(top), SHIELD);
    for side in [-1., 1.] {
        // Square across the bend, a plate's thickness inboard of its face.
        let inward = v(-side, 0., 1.) * (plate * std::f32::consts::FRAC_1_SQRT_2);
        let corner = |y: f32| {
            let z = front(y);
            let (a, b) = (v(side * (w - bend), y, z), v(side * w, y, z + bend));
            [a, b, b + inward, a + inward]
        };
        block(&mut mesh, corner(foot), corner(top), SHIELD);
        let (out, inner) = (side * w, side * (w - plate));
        let ring = |y, z0, z1| {
            [
                v(out, y, z0),
                v(inner, y, z0),
                v(inner, y, z1),
                v(out, y, z1),
            ]
        };
        block(
            &mut mesh,
            ring(foot, front(foot) + bend, 0.05),
            ring(top, front(top) + bend, 0.05),
            SHIELD,
        );
        block(
            &mut mesh,
            ring(foot, 0.05, 0.3),
            [
                v(out, top, 0.05),
                v(inner, top, 0.05),
                v(inner, 0.96, 0.3),
                v(out, 0.96, 0.3),
            ],
            SHIELD,
        );
        // A sighting hood each side of the roof, for the layer and the trainer.
        let (a, b) = (side * (w - 0.3), side * (w - 0.15));
        let (x0, x1) = (a.min(b), a.max(b));
        cuboid(
            &mut mesh,
            v(x0, top, -0.3),
            v(x1, top + 0.08, -0.13),
            SHIELD,
        );
        outward(
            &mut mesh,
            [
                v(x0 + 0.02, top + 0.025, -0.302),
                v(x1 - 0.02, top + 0.025, -0.302),
                v(x1 - 0.02, top + 0.055, -0.302),
                v(x0 + 0.02, top + 0.055, -0.302),
            ],
            v(0.5 * (x0 + x1), top + 0.04, 0.),
            SOOT,
        );
    }
    // The half roof, its front corners bent back with the face beneath.
    let lid = |y: f32| {
        let z = front(top) - 0.015;
        [
            v(-w - 0.01, y, z + bend),
            v(-(w - bend), y, z),
            v(w - bend, y, z),
            v(w + 0.01, y, z + bend),
        ]
    };
    block(&mut mesh, lid(top - 0.02), lid(top + 0.02), SHIELD_ROOF);
    let aft = |y: f32| {
        let z = front(top) - 0.015 + bend;
        [
            v(-w - 0.01, y, z),
            v(w + 0.01, y, z),
            v(w + 0.01, y, 0.07),
            v(-w - 0.01, y, 0.07),
        ]
    };
    block(&mut mesh, aft(top - 0.02), aft(top + 0.02), SHIELD_ROOF);
    // A stiffener across the face, and the mantlet the barrels run out through.
    cuboid(
        &mut mesh,
        v(-(w - bend) + 0.02, 0.6, -0.432),
        v(w - bend - 0.02, 0.64, -0.4),
        LIP,
    );
    cuboid(
        &mut mesh,
        v(-(outer + MANTLET_SIDE), TRUNNION - 0.08, -MANTLET_FRONT),
        v(outer + MANTLET_SIDE, TRUNNION + 0.1, -0.34),
        MANTLET,
    );
    mesh.finish()
}

/// A gun sponson built out from her starboard side, in the frame of the gun's pivot at
/// deck height with +x outboard, a metre in radius: drawn to the reach of the mount it
/// carries. Its deck is level with hers and its breastwork low enough for the shield to
/// train over; beneath, it is faired back into her plating. Inboard of her side it runs
/// on under her deck, out of sight wherever she is broad.
pub(super) fn sponson() -> Mesh {
    use super::paint::{PLAIN, shell_uv, teak_uv};
    use std::f32::consts::PI;
    let mut mesh = Facets::default();
    let mut uv: Vec<[f32; 2]> = Vec::new();
    let mut mapped = |mesh: &Facets, map: &dyn Fn(Vec3) -> [f32; 2]| {
        let done = uv.len();
        uv.extend(mesh.positions[done..].iter().map(|p| map(Vec3::from(*p))));
    };
    // Her plating's pattern, as though the sponson stood on the stock deck height.
    let shell = |p: Vec3| shell_uv(p + Vec3::Y * super::deck_height(0.));
    // Painted as her side, with the pale strake along her deck edge carried round.
    let plating = Color::srgb_u8(85, 112, 133);
    let rim = Color::srgb_u8(183, 189, 168);
    let rail = Color::srgb_u8(201, 204, 185);
    let decking = Color::srgb_u8(159, 149, 126);
    const STEPS: usize = 12;
    const INBOARD: f32 = -1.8;
    let (top, bottom, wall, coaming) = (-0.012, -0.12, 0.3, 0.05);
    let arc = |k: usize, r: f32, y: f32| {
        let (s, c) = (-0.5 * PI + PI * k as f32 / STEPS as f32).sin_cos();
        Vec3::new(c * r, y, s * r)
    };
    // The outline of its deck: round outboard, square where it runs in under hers.
    let outline = |y: f32| {
        let mut points: Vec<Vec3> = (0..=STEPS).map(|k| arc(k, 1., y)).collect();
        points.extend([Vec3::new(INBOARD, y, 1.), Vec3::new(INBOARD, y, -1.)]);
        points
    };
    let middle = Vec3::new(-0.2, 0.5 * (top + bottom), 0.);
    let (upper, lower) = (outline(top), outline(bottom));
    for i in 0..upper.len() {
        let j = (i + 1) % upper.len();
        let centre = Vec3::new(-0.2, top, 0.);
        outward_tri(&mut mesh, [centre, upper[i], upper[j]], middle, decking);
    }
    mapped(&mesh, &teak_uv);
    for i in 0..upper.len() {
        let j = (i + 1) % upper.len();
        outward(
            &mut mesh,
            [upper[i], upper[j], lower[j], lower[i]],
            middle,
            rim,
        );
    }
    // Faired down and in to her side beneath it.
    let (depth, root) = (-0.75, -1.6);
    let foot = |k: usize| arc(k, 0.3, depth) + Vec3::X * root;
    let within = Vec3::new(-0.9, -0.3, 0.);
    for k in 0..STEPS {
        outward(
            &mut mesh,
            [lower[k], lower[k + 1], foot(k + 1), foot(k)],
            within,
            plating,
        );
    }
    for (k, z) in [(0, -1.), (STEPS, 1.)] {
        outward(
            &mut mesh,
            [
                Vec3::new(INBOARD, bottom, z),
                lower[k],
                foot(k),
                Vec3::new(INBOARD, depth, z * 0.3),
            ],
            within,
            plating,
        );
    }
    // The breastwork round its outboard edge, running a little way in at either end.
    let ring = |k: usize, y: f32, inner: f32, outer: f32| {
        [
            arc(k, inner, y),
            arc(k, outer, y),
            arc(k + 1, outer, y),
            arc(k + 1, inner, y),
        ]
    };
    for k in 0..STEPS {
        block(
            &mut mesh,
            ring(k, top, 1. - coaming, 1.),
            ring(k, wall, 1. - coaming, 1.),
            plating,
        );
    }
    for z in [-1., 1.] {
        let run = |y| {
            [
                Vec3::new(-0.15, y, z),
                Vec3::new(0., y, z),
                Vec3::new(0., y, z * (1. - coaming)),
                Vec3::new(-0.15, y, z * (1. - coaming)),
            ]
        };
        block(&mut mesh, run(top), run(wall), plating);
    }
    mapped(&mesh, &shell);
    for k in 0..STEPS {
        block(
            &mut mesh,
            ring(k, wall, 1. - coaming - 0.02, 1.015),
            ring(k, wall + 0.03, 1. - coaming - 0.02, 1.015),
            rail,
        );
    }
    mapped(&mesh, &|_| PLAIN);
    mesh.finish()
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
}
