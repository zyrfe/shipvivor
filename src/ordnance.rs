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
    let rings = [
        (-1.08, 0., cap),
        (-0.89, 0.13, cap),
        (-0.62, 0.28, steel),
        (-0.32, 0.355, steel),
        (0.37, 0.355, band),
        (0.40, 0.38, band),
        (0.52, 0.38, steel),
        (0.56, 0.35, steel),
        (0.72, 0.31, base),
        (0.72, 0., base),
    ];
    let mut facets = Facets::default();
    for segment in rings.windows(2) {
        let [(z0, r0, paint), (z1, r1, _)] = [segment[0], segment[1]];
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
                facets.triangle([a, b, c], paint);
            }
            if r1 > 0. {
                facets.triangle([a, c, d], paint);
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
