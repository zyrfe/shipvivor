//! A flat strike rings and sheds flakes. A breach throws a plate and vents soot.
use super::{Actor, Shape, pose};
use crate::scene::Facets;
use bevy::prelude::*;
use shipvivor_encounter::{Battery, Effect, Impact};
use std::collections::HashMap;

const WHITE: Color = Color::srgb(1., 0.97, 0.77);
const AMBER: Color = Color::srgb(1., 0.58, 0.15);
const COPPER: Color = Color::srgb(0.74, 0.22, 0.055);

// Each tongue tapers in three dimensions so a side-on strike still has a bright core.
fn tongue(mesh: &mut Facets, start: Vec3, end: Vec3, radius: f32, tip: Color) {
    let axis = (end - start).normalize();
    let a = axis.any_orthonormal_vector() * radius;
    let b = axis.cross(a);
    let shoulder = start.lerp(end, 0.23);
    let ring = [shoulder + a, shoulder + b, shoulder - a, shoulder - b];
    for i in 0..4 {
        let j = (i + 1) % 4;
        mesh.triangle_colors([start, ring[j], ring[i]], [WHITE, AMBER, WHITE]);
        mesh.triangle_colors([ring[i], ring[j], end], [WHITE, AMBER, tip]);
    }
}

pub fn flash_mesh() -> Mesh {
    let mut mesh = Facets::default();
    for i in 0..11 {
        let a = i as f32 * 2.4;
        let dir = Vec3::new(a.cos(), 0.15 + 0.86 * a.sin(), 0.3 + 0.49 * (a * 1.7).cos());
        let reach = 0.78 + 0.39 * (a * 0.7).sin();
        tongue(
            &mut mesh,
            dir * 0.04,
            dir * reach,
            0.19 + 0.08 * a.cos(),
            COPPER,
        );
    }
    tongue(
        &mut mesh,
        Vec3::ZERO,
        Vec3::new(-0.04, 0.46, 0.87),
        0.38,
        AMBER,
    );
    mesh.finish()
}

pub fn spark_mesh() -> Mesh {
    let mut mesh = Facets::default();
    tongue(&mut mesh, Vec3::NEG_Z * 0.45, Vec3::Z * 1.25, 1., COPPER);
    mesh.finish()
}

/// A mark a round left on plate, in local XY, facing +Z, a unit across: a crater where
/// the plate held, or a hole where it went in.
pub fn scar(open: bool) -> Mesh {
    if open { hole() } else { crater() }
}

/// Where a round burst against plate that held it: a shallow dish blackened by the
/// burst, ringed by a lip of bare steel the paint was blasted off, inside a star of
/// soot thrown out across the paint.
fn crater() -> Mesh {
    let mut mesh = Facets::default();
    let soot = Color::srgb_u8(38, 40, 42);
    let scuff = Color::srgb_u8(176, 178, 170);
    let bruise = Color::srgb_u8(92, 94, 92);
    let floor = Color::srgb_u8(46, 46, 48);
    const SIDES: usize = 14;
    let at = |i: usize, radius: f32, lift: f32| {
        let a = i as f32 * std::f32::consts::TAU / SIDES as f32;
        // No crater is quite round.
        let r = radius * (1. + 0.09 * (a * 3.).sin() + 0.05 * (a * 5. + 1.).cos());
        Vec3::new(a.cos() * r, a.sin() * r, lift)
    };
    for i in 0..SIDES {
        let j = (i + 1) % SIDES;
        // The star of soot: spokes of uneven length.
        let reach = 0.95 + 0.25 * ((i * 7 % 5) as f32 / 4.);
        let mid = at(i, 0.8, 0.004).lerp(at(j, 0.8, 0.004), 0.5);
        let tip = mid.normalize() * reach + Vec3::Z * 0.004;
        mesh.triangle([at(j, 0.8, 0.004), at(i, 0.8, 0.004), tip], soot);
        // The lip rises from the paint to its crest, and drops into the dish.
        mesh.quad(
            [
                at(i, 0.86, 0.006),
                at(j, 0.86, 0.006),
                at(j, 0.64, 0.12),
                at(i, 0.64, 0.12),
            ],
            bruise,
        );
        mesh.quad(
            [
                at(i, 0.64, 0.12),
                at(j, 0.64, 0.12),
                at(j, 0.36, 0.05),
                at(i, 0.36, 0.05),
            ],
            if i % 3 == 0 { bruise } else { scuff },
        );
        mesh.triangle(
            [at(i, 0.36, 0.05), at(j, 0.36, 0.05), Vec3::Z * 0.035],
            floor,
        );
    }
    mesh.finish()
}

/// Where a round went in: a black hole into her, its torn edges curled back out of it
/// in bare and burnt steel, inside a halo of scorched paint.
fn hole() -> Mesh {
    let mut mesh = Facets::default();
    let halo = Color::srgb_u8(30, 29, 28);
    let singed = Color::srgb_u8(64, 60, 56);
    let torn = Color::srgb_u8(156, 112, 80);
    let burnt = Color::srgb_u8(70, 52, 42);
    let dark = Color::srgb_u8(6, 6, 7);
    const SIDES: usize = 16;
    let at = |i: usize, radius: f32, lift: f32| {
        let a = i as f32 * std::f32::consts::TAU / SIDES as f32;
        let r = radius * (1. + 0.14 * (a * 3. + 0.5).sin() + 0.08 * (a * 7.).cos());
        Vec3::new(a.cos() * r, a.sin() * r, lift)
    };
    for i in 0..SIDES {
        let j = (i + 1) % SIDES;
        // The scorch, ragged at its edge.
        let edge = 0.95 + 0.25 * ((i * 5 % 7) as f32 / 6.);
        let (out_i, out_j) = (at(i, 1., 0.004) * edge, at(j, 1., 0.004) * edge);
        mesh.triangle_colors([out_i, out_j, at(j, 0.62, 0.004)], [singed, singed, halo]);
        mesh.triangle_colors(
            [out_i, at(j, 0.62, 0.004), at(i, 0.62, 0.004)],
            [singed, halo, halo],
        );
        // The petals of torn plate, every other one bent farther back.
        let curl = if i % 2 == 0 { 0.2 } else { 0.11 };
        let tip = at(i, 0.74, curl).lerp(at(j, 0.74, curl), 0.5) * Vec3::new(1.08, 1.08, 1.);
        mesh.triangle_colors(
            [at(j, 0.42, 0.015), at(i, 0.42, 0.015), tip],
            [burnt, burnt, torn],
        );
        mesh.triangle([at(i, 0.42, 0.015), at(i, 0.62, 0.008), tip], burnt);
        mesh.triangle([tip, at(j, 0.62, 0.008), at(j, 0.42, 0.015)], burnt);
        // The hole, black right through.
        mesh.triangle(
            [at(i, 0.42, 0.012), at(j, 0.42, 0.012), Vec3::Z * 0.012],
            dark,
        );
    }
    mesh.finish()
}

pub(super) fn opened(outcome: Impact) -> bool {
    outcome.opened()
}

pub(super) fn draw(
    e: &Effect,
    outcome: Impact,
    bore: u16,
    battery: Battery,
    wanted: &mut HashMap<Actor, Transform>,
) {
    let age = e.age;
    let size = (1. + 0.12 * f32::from(bore))
        * if battery == Battery::Secondary {
            0.58
        } else {
            1.
        };
    let t = age / e.kind.lifetime();
    let contact = pose(&e.body, e.height);
    let origin = contact.translation;
    let forward = e.body.forward().extend(0.).xzy();
    let side = Vec3::Y.cross(forward);
    let breached = opened(outcome);
    if !breached && t < 0.43 {
        let span = (0.35 + 6. * t) * size;
        let ring = Transform::from_translation(origin + forward * 0.15)
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, forward))
            .with_scale(Vec3::new(span, (1. - t / 0.43) * 1.45, span));
        wanted.insert(Actor(e.body.id, Shape::DentRing), ring);
    }
    let flash_time = if breached { 0.27 } else { 0.13 };
    if age < flash_time {
        let span = if outcome == Impact::Penetrated {
            4.3
        } else {
            2.5
        };
        let grow = (0.4 + age * 20.).min(1.);
        let mut flare =
            contact.with_scale(Vec3::splat(size * span * grow * (1. - age / flash_time)));
        flare.rotate_local_y(std::f32::consts::PI);
        if !breached {
            flare.scale *= Vec3::new(0.66, 0.66, 0.8);
        }
        wanted.insert(Actor(e.body.id, Shape::ImpactFlash), flare);
    }
    // The first ricochet streak carries on along the reflected bearing.
    let count = if breached { 5 } else { 8 };
    for i in 0..count {
        let seed = (e.body.id % 29) as f32 + f32::from(i) * 2.40;
        let v = if i == 0 && outcome == Impact::Bounced {
            forward * 71. + Vec3::Y * 10.
        } else {
            forward * (6. + 4. * seed.cos())
                + side * seed.sin() * 13.
                + Vec3::Y * (7. + (seed * 1.7).cos() * 7.)
        } * size;
        let at = origin + v * age - Vec3::Y * 10. * age * age;
        if at.y < 0. || age > 0.66 {
            continue;
        }
        let motion = v - Vec3::Y * 20. * age;
        let radius = (0.06 + 0.10 * (1. - t)) * size;
        let xf = Transform::from_translation(at)
            .looking_to(motion, Vec3::Y)
            .with_scale(Vec3::new(
                radius,
                radius,
                (motion.length() * 0.085).max(0.72),
            ));
        wanted.insert(Actor(e.body.id, Shape::Streak(i)), xf);
    }
    if breached {
        let plume =
            Transform::from_translation(origin + forward * (1.2 + t) * size + Vec3::Y * (t * 4.4))
                .with_scale(Vec3::splat(
                    size * (0.4 + 3.0 * (std::f32::consts::PI * t).sin()),
                ));
        wanted.insert(Actor(e.body.id, Shape::Smoke), plume);
        if outcome == Impact::Penetrated {
            for i in 0..4 {
                let k = f32::from(i);
                let at = origin
                    + (forward * (4. + k) + side * (k - 1.5) * 4. + Vec3::Y * (5. + 1.7 * k))
                        * age
                        * size
                    - Vec3::Y * 10. * age * age;
                if at.y <= 0. {
                    continue;
                }
                let mut chunk = Transform::from_translation(at)
                    .with_scale(Vec3::new(0.8, 0.24, 1.2) * size * (1. - t));
                chunk.rotate_x(k + age * 6.);
                chunk.rotate_z(age * 4. + k);
                wanted.insert(Actor(e.body.id, Shape::Debris(i)), chunk);
            }
        }
    }
}
