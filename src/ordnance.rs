//! Small shared meshes keep rounds and impact silhouettes legible in motion.
use crate::scene::Facets;
use bevy::prelude::*;

pub const SHELL_MESH_DIAMETER: f32 = 0.76;

/// Revolve the painted silhouette around its direction of travel (local -Z).
pub fn shell() -> Mesh {
    let steel = Color::srgb_u8(83, 101, 108);
    let cap = Color::srgb_u8(202, 207, 195);
    let band = Color::srgb_u8(211, 149, 72);
    let base = Color::srgb_u8(45, 57, 65);
    lathe(
        &[
            (-1.08, 0., cap),
            (-0.89, 0.13, cap),
            (-0.62, 0.28, steel),
            (-0.32, 0.355, steel),
            (0.37, 0.355, band),
            (0.40, 0.38, band),
            (0.52, 0.38, steel),
            (0.56, 0.35, steel),
            (0.72, SHELL_BASE.1, base),
            (SHELL_BASE.0, 0., base),
        ],
        false,
    )
}

/// Where the shell's flat base sits along its axis, and its radius there.
const SHELL_BASE: (f32, f32) = (0.72, 0.31);

/// The tracer burning in a shell's base: white hot and as wide as the base where it
/// leaves her, thinning and fading to nothing astern. Its wide, bright end has to be
/// the shell's end, or the trail reads as a pointed body flying tail first.
pub fn trail(tint: Color) -> Mesh {
    // Brighter than white, so the tone mapping leaves it glowing instead of painted.
    let hot = Color::linear_rgb(4., 3.4, 2.2);
    let burn = |gain: f32, alpha: f32| Color::from(tint.to_linear() * gain).with_alpha(alpha);
    let (z, r) = SHELL_BASE;
    lathe(
        &[
            (z, r * 0.9, hot),
            (z + 0.45, r * 0.86, burn(2.2, 1.)),
            (z + 3.6, r * 0.55, burn(1.6, 0.7)),
            (z + 8., r * 0.18, burn(1.2, 0.)),
        ],
        true,
    )
}

/// The glow of the tracer in a shell's base, as a disc a unit in radius facing +Z, to
/// be turned to the camera: white hot at its heart, fading to nothing at its rim. It is
/// what the eye follows up and over the arc when the shell is too small to see.
pub fn glow(tint: Color) -> Mesh {
    let burn = |gain: f32, alpha: f32| Color::from(tint.to_linear() * gain).with_alpha(alpha);
    let rings = [
        (0., Color::linear_rgb(6., 5.4, 4.)),
        (0.3, Color::linear_rgb(5., 4.2, 2.6)),
        (0.5, burn(2.6, 0.9)),
        (0.75, burn(2., 0.35)),
        (1., burn(1.6, 0.)),
    ];
    let mut facets = Facets::default();
    let at = |n: usize, r: f32| {
        let a = n as f32 * std::f32::consts::TAU / 16.;
        Vec3::new(a.cos() * r, a.sin() * r, 0.)
    };
    for ring in rings.windows(2) {
        let [(r0, inner), (r1, outer)] = [ring[0], ring[1]];
        for i in 0..16 {
            let (a, b) = (at(i, r0), at(i + 1, r0));
            let (c, d) = (at(i + 1, r1), at(i, r1));
            if r0 > 0. {
                facets.triangle_colors([a, d, b], [inner, outer, inner]);
            }
            facets.triangle_colors([b, d, c], [inner, outer, outer]);
        }
    }
    facets.finish()
}

/// How many straight lengths a shell's ghost trail is drawn in, so it bends with the arc.
pub const GHOST_LENGTHS: usize = 3;

/// One length of the thin glowing line a shell leaves hanging on its path: a unit long
/// from its newer end at the origin back along -Z, a unit in radius at the trail's
/// head. The whole trail thins and fades from the shell to nothing at its far end.
pub fn ghost(tint: Color, length: usize) -> Mesh {
    let along = |i: usize| i as f32 / GHOST_LENGTHS as f32;
    let paint =
        |t: f32| Color::from(tint.to_linear() * (2.4 - t)).with_alpha(0.75 * (1. - t) * (1. - t));
    let (near, far) = (along(length), along(length + 1));
    let (r0, r1) = (1. - 0.6 * near, 1. - 0.6 * far);
    let mut facets = Facets::default();
    let at = |n: usize, r: f32, z: f32| {
        let a = n as f32 * std::f32::consts::TAU / 6.;
        Vec3::new(a.cos() * r, a.sin() * r, z)
    };
    for i in 0..6 {
        let (a, b) = (at(i, r0, 0.), at(i + 1, r0, 0.));
        let (c, d) = (at(i + 1, r1, -1.), at(i, r1, -1.));
        facets.triangle_colors([a, d, b], [paint(near), paint(far), paint(near)]);
        facets.triangle_colors([b, d, c], [paint(near), paint(far), paint(far)]);
    }
    facets.finish()
}

/// Revolve rings of (z, radius, paint) around Z. Each band takes its leading ring's
/// paint, or shades from one ring's to the next when `graded`.
fn lathe(rings: &[(f32, f32, Color)], graded: bool) -> Mesh {
    let mut facets = Facets::default();
    for segment in rings.windows(2) {
        let [(z0, r0, paint), (z1, r1, next)] = [segment[0], segment[1]];
        let next = if graded { next } else { paint };
        for i in 0..16 {
            let vertex = |n: usize, r, z| {
                let a = n as f32 * std::f32::consts::TAU / 16.;
                Vec3::new(a.cos() * r, a.sin() * r, z)
            };
            let a = vertex(i, r0, z0);
            let b = vertex(i + 1, r0, z0);
            let c = vertex(i + 1, r1, z1);
            let d = vertex(i, r1, z1);
            if r0 > 0. {
                facets.triangle_colors([a, b, c], [paint, paint, next]);
            }
            if r1 > 0. {
                facets.triangle_colors([a, c, d], [paint, next, next]);
            }
        }
    }
    facets.finish()
}

pub fn spray() -> Mesh {
    let mut facets = Facets::default();
    let tip = |i: usize| {
        let a = i as f32 * std::f32::consts::TAU / 20.;
        let ray = Vec3::new(a.cos(), 0., a.sin());
        let h = if i.is_multiple_of(2) {
            0.62 + 0.35 * (i as f32 * 2.3).sin().abs()
        } else {
            0.24
        };
        (ray * 0.42, ray * (0.6 + h * 0.35) + Vec3::Y * h)
    };
    for i in 0..20 {
        let (a, b) = tip(i);
        let (c, d) = tip((i + 1) % 20);
        let col = if i.is_multiple_of(2) {
            Color::srgb_u8(201, 235, 232)
        } else {
            Color::srgb_u8(139, 197, 209)
        };
        facets.triangle([a, b, d], col);
        facets.triangle([a, d, c], col);
    }
    facets.finish()
}

/// A compact cruiser gunhouse with a raked face, chamfered cheeks and a flat roof.
pub fn cruiser_gunhouse() -> Mesh {
    let outline = [
        (-0.91, -1.36),
        (0.91, -1.36),
        (1.42, -0.82),
        (1.42, 0.88),
        (1.15, 1.30),
        (-1.15, 1.30),
        (-1.42, 0.88),
        (-1.42, -0.82),
    ];
    let base = outline.map(|(x, z)| Vec3::new(x, 0.20, z));
    let roof =
        outline.map(|(x, z)| Vec3::new(x * 0.90, 1.31, if z < 0. { z * 0.64 } else { z * 0.94 }));
    let mut facets = Facets::default();
    for i in 0..outline.len() {
        let j = (i + 1) % outline.len();
        let face = if i == 0 {
            Color::srgb_u8(139, 160, 164)
        } else {
            Color::srgb_u8(119, 143, 151)
        };
        facets.triangle([base[i], roof[j], base[j]], face);
        facets.triangle([base[i], roof[i], roof[j]], face);
        facets.triangle(
            [Vec3::Y * 1.31, roof[j], roof[i]],
            Color::srgb_u8(180, 192, 184),
        );
    }
    facets.finish()
}
