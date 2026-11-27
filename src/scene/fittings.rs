//! Painted deck fittings, batched into the ship's existing meshes.
use super::Facets;
use bevy::prelude::*;

const IRON: Color = Color::srgb(0.18, 0.22, 0.24);
const SOOT: Color = Color::srgb(0.075, 0.087, 0.093);
const STEEL: Color = Color::srgb(0.40, 0.48, 0.50);
const LIP: Color = Color::srgb(0.48, 0.51, 0.50);

/// A raked oval uptake. The lip is directly above `foot`, where the smoke starts.
pub(crate) fn funnel(mesh: &mut Facets, foot: Vec3, size: Vec3, livery: Color, fleet_bands: bool) {
    let point = |angle: f32, h: f32, width: f32| {
        let (sin, cos) = angle.sin_cos();
        foot + Vec3::new(
            size.x * 0.5 * cos * width,
            size.y * h,
            size.z * (0.5 * sin * width - (1. - h) * 0.17),
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
