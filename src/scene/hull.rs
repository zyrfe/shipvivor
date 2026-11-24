//! Breakwater's plated sides, raised forecastle and ground tackle.
use super::{Facets, Hull, paint};
use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;

pub(super) fn deck_height(z: f32) -> f32 {
    // The sheer starts ahead of the forward gun and lifts the stem clear of the bow wave.
    1.55 + ((-z - 7.) / 9.).clamp(0., 1.).powi(2) * 1.15
}

pub(super) fn extended_hull_mesh(hull: Hull) -> Mesh {
    let outline = hull.outline();
    let mut mesh = Facets::default();
    let mut uv = Vec::new();
    let mut plate_normals = Vec::new();
    let paint = Color::srgb_u8;
    for (i, &(ax, az)) in outline.iter().enumerate() {
        let (bx, bz) = outline[(i + 1) % outline.len()];
        let rows = |x: f32, z: f32| {
            let deck = deck_height(z);
            [
                Vec3::new(0.62 * x, -1.9, 0.62 * z),
                Vec3::new(0.885 * x, 0.18, 0.885 * z),
                Vec3::new(0.91 * x, 0.48, 0.91 * z),
                Vec3::new(0.997 * x, deck - 0.105, 0.997 * z),
                Vec3::new(x, deck, z),
            ]
        };
        let a = rows(ax, az);
        let b = rows(bx, bz);
        let (px, pz) = outline[(i + outline.len() - 1) % outline.len()];
        let (cx, cz) = outline[(i + 2) % outline.len()];
        let previous = rows(px, pz);
        let next = rows(cx, cz);
        for (j, color) in [
            paint(148, 66, 47),
            paint(48, 61, 67),
            paint(85, 112, 133),
            paint(183, 189, 168),
        ]
        .into_iter()
        .enumerate()
        {
            let start = mesh.positions.len();
            mesh.quad([a[j], a[j + 1], b[j + 1], b[j]], color);
            uv.extend(
                mesh.positions[start..]
                    .iter()
                    .map(|p| paint::shell_uv((*p).into())),
            );
            // Fair the light along each strake, retaining the creases at the stem and transom.
            let side_normal = |p: &[Vec3; 5], q: &[Vec3; 5]| {
                ((p[j + 1] - p[j]) + (q[j + 1] - q[j]))
                    .cross(q[j] - p[j])
                    .normalize()
            };
            let n = side_normal(&a, &b);
            let join = |other: Vec3| {
                if n.dot(other) > 0.65 {
                    (other + n).normalize()
                } else {
                    n
                }
            };
            let na = join(side_normal(&previous, &a));
            let nb = join(side_normal(&b, &next));
            plate_normals.push((start, [na, na, nb, na, nb, nb]));
        }

        // Set each scuttle and the anchor pocket on the actual sloping shell plate.
        let upper = b[3] - a[3];
        let normal = (a[3] - a[2]).cross(upper).normalize();
        let along = upper.normalize();
        let up = along.cross(normal).normalize();
        // Interpolate on the two rendered triangles. The flared bow quad is not planar.
        let point = |t: f32, h: f32| {
            if h >= t {
                a[2] * (1. - h) + a[3] * (h - t) + b[3] * t
            } else {
                a[2] * (1. - t) + b[3] * h + b[2] * (t - h)
            }
        };
        let at_bow = az.min(bz) < -15.;
        if at_bow {
            // Alternating draft ticks follow the rake of the stem on both cheeks.
            for mark in 0..7 {
                let h = 0.045 + 0.084 * mark as f32;
                let width = if mark % 2 == 0 { 0.052 } else { 0.028 };
                let from = if az < bz { 0.27 } else { 0.73 - width };
                let dash = |t, y| point(t, y) + normal * 0.014;
                mesh.quad(
                    [
                        dash(from, h),
                        dash(from, h + 0.018),
                        dash(from + width, h + 0.018),
                        dash(from + width, h),
                    ],
                    paint(196, 195, 166),
                );
            }
            let center = point(if az < bz { 0.70 } else { 0.30 }, 0.77) + normal * 0.035;
            eyelet(&mut mesh, center, along * 0.34, up * 0.22, normal);
            // A stockless anchor stowed crown down, with the two flukes folded to the hull.
            let stock = center + normal * 0.075;
            let crown = stock - up * 0.63;
            mesh.tube(crown, stock, 0.075, 0.063, paint(45, 55, 59));
            mesh.tube(
                crown - along * 0.25,
                crown + along * 0.25,
                0.081,
                0.081,
                paint(58, 70, 72),
            );
            for sign in [-1., 1.] {
                let base = crown + along * sign * 0.18;
                let tip = base + along * sign * 0.19 + up * 0.30;
                let corners = [
                    base - up * 0.06,
                    base + normal * 0.14,
                    tip,
                    base + along * sign * 0.12,
                ];
                mesh.quad(corners, paint(64, 75, 79));
                mesh.quad(
                    [corners[3], corners[2], corners[1], corners[0]],
                    paint(80, 91, 92),
                );
            }
        } else if (bx + ax).abs() > 1. {
            let count = (upper.length() / 1.8).floor() as usize;
            for j in 0..count {
                let p = point((j as f32 + 0.5) / count as f32, 0.68) + normal * 0.028;
                eyelet(&mut mesh, p, along * 0.125, up * 0.12, normal);
            }
        }
        uv.resize(mesh.positions.len(), paint::PLAIN);
    }

    let rail = paint(201, 204, 185);
    // Transverse deck strips follow the sheer across the full beam.
    for pair in outline[..=outline.len() / 2].windows(2) {
        let [(ax, az), (bx, bz)] = [pair[0], pair[1]];
        let a = Vec3::new(ax, deck_height(az), az);
        let b = Vec3::new(bx, deck_height(bz), bz);
        let mirror = Vec3::new(-1., 1., 1.);
        let decking = mesh.positions.len();
        mesh.triangle([a * mirror, b * mirror, b], paint(159, 149, 126));
        if ax > 0. {
            mesh.triangle([a * mirror, b, a], paint(159, 149, 126));
        }
        uv.extend(
            mesh.positions[decking..]
                .iter()
                .map(|p| paint::teak_uv((*p).into())),
        );
        if az >= -7. {
            continue;
        }
        for side in [-1., 1.] {
            let edge = Vec3::new(0.97 * side, 1., 1.);
            let start = a * edge;
            let end = if bz > -7. {
                (a * edge).lerp(b * edge, (-7. - az) / (bz - az))
            } else {
                b * edge
            };
            for h in [0.37, 0.75] {
                mesh.tube(start + Vec3::Y * h, end + Vec3::Y * h, 0.031, 0.031, rail);
            }
            let posts = ((end - start).length() / 1.65).ceil() as usize;
            for j in 0..posts {
                if side < 0. && ax == 0. && j == 0 {
                    continue;
                }
                let foot = start.lerp(end, j as f32 / posts as f32);
                mesh.tube(foot, foot + Vec3::Y * 0.77, 0.040, 0.031, rail);
            }
        }
        uv.resize(mesh.positions.len(), paint::PLAIN);
    }
    let mut mesh = mesh
        .finish()
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv);
    if let Some(VertexAttributeValues::Float32x3(normals)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_NORMAL)
    {
        for (start, plate) in plate_normals {
            normals[start..start + 6].copy_from_slice(&plate.map(|n| n.to_array()));
        }
    }
    mesh
}

/// Rolled metal lip around a dark recess. Both sides of the hull use an outward-facing frame.
fn eyelet(mesh: &mut Facets, center: Vec3, along: Vec3, up: Vec3, normal: Vec3) {
    let ring = |i: usize| {
        let (s, c) = (i as f32 * std::f32::consts::TAU / 12.).sin_cos();
        along * c + up * s
    };
    for i in 0..12 {
        let a = ring(i);
        let b = ring(i + 1);
        let inset = center + normal * 0.018;
        mesh.quad(
            [center + b, center + a, inset + a * 0.72, inset + b * 0.72],
            Color::srgb_u8(170, 159, 127),
        );
        mesh.triangle(
            [inset, inset + b * 0.72, inset + a * 0.72],
            Color::srgb_u8(33, 49, 57),
        );
    }
}
