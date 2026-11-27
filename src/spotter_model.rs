//! A naval observation floatplane. Local -Z is the propeller end.
use crate::scene::Facets;
use bevy::prelude::*;

const CANVAS: Color = Color::srgb(0.83, 0.76, 0.58);
const GOLD: Color = Color::srgb(0.94, 0.64, 0.17);
const RED: Color = Color::srgb(0.76, 0.17, 0.12);
const INK: Color = Color::srgb(0.15, 0.22, 0.24);
const SEA: Color = Color::srgb(0.32, 0.44, 0.44);
const GLASS: Color = Color::srgb(0.21, 0.44, 0.54);

/// Aircraft, canopy and floats are skinned around elliptical frames along the keel.
pub(crate) fn skin(mesh: &mut Facets, x: f32, stations: &[(f32, f32, f32, f32, Color)]) {
    for pair in stations.windows(2) {
        let point = |(z, y, width, height, _), rib: usize| {
            let (s, c) = (rib as f32 * std::f32::consts::TAU / 12.).sin_cos();
            Vec3::new(x + c * width, y + s * height, z)
        };
        for i in 0..12 {
            mesh.quad(
                [
                    point(pair[0], i),
                    point(pair[0], i + 1),
                    point(pair[1], i + 1),
                    point(pair[1], i),
                ],
                pair[0].4,
            );
        }
    }
}

/// Camber, clipped tips and the outer recognition band read from the bridge below.
pub(crate) fn wing(mesh: &mut Facets, half_span: f32, y: f32, z: f32, chord: f32, color: Color) {
    for side in [-1., 1.] {
        let section = |span: f32| {
            let tip = ((span - 0.88) / 0.12).max(0.);
            let x = side * span * half_span;
            let lead = z + (0.13 * span + 0.20 * tip) * chord;
            let back = z + (1. - 0.20 * span - 0.16 * tip) * chord;
            let h = y + 0.28 * span;
            [
                Vec3::new(x, h, lead),
                Vec3::new(x, h + 0.14, (2. * lead + back) / 3.),
                Vec3::new(x, h, back),
                Vec3::new(x, h - 0.08, (2. * lead + back) / 3.),
            ]
        };
        for (a, b, paint) in [
            (0., 0.74, color),
            (0.74, 0.82, RED),
            (0.82, 0.88, CANVAS),
            (0.88, 1., color),
        ] {
            let (near, far) = (section(a), section(b));
            for i in 0..4 {
                let j = (i + 1) % 4;
                let mut panel = [near[i], near[j], far[j], far[i]];
                if side < 0. {
                    panel.reverse();
                }
                mesh.quad(panel, paint);
            }
        }
        let mut cap = section(1.);
        if side > 0. {
            cap.reverse();
        }
        mesh.quad(cap, color);
    }
}

pub fn plane() -> Mesh {
    airframe(false)
}

pub fn raider() -> Mesh {
    airframe(true)
}

fn airframe(attack: bool) -> Mesh {
    let mut m = Facets::default();
    let v = Vec3::new;
    skin(
        &mut m,
        0.,
        &[
            (-5.45, 0., 0.01, 0.01, INK),
            (-5.32, 0., 0.73, 0.76, INK),
            (-4.92, 0., 0.91, 0.9, GOLD),
            (-4.1, 0., 0.93, 0.9, SEA),
            (-1.3, 0., 0.97, 0.96, SEA),
            (1.95, 0.08, 0.78, 0.72, SEA),
            (3.65, 0.22, 0.49, 0.49, CANVAS),
            (4.05, 0.23, 0.43, 0.42, RED),
            (4.4, 0.25, 0.37, 0.37, SEA),
            (6.42, 0.38, 0.04, 0.08, SEA),
        ],
    );
    skin(
        &mut m,
        0.,
        &[
            (-2.25, 0.54, 0.52, 0.18, GLASS),
            (-1.62, 0.73, 0.73, 0.72, GLASS),
            (-0.6, 0.76, 0.75, 0.79, INK),
            (-0.41, 0.76, 0.75, 0.79, GLASS),
            (1.05, 0.66, 0.63, 0.60, GLASS),
            (1.75, 0.49, 0.45, 0.17, SEA),
        ],
    );
    if attack {
        wing(&mut m, 8.2, -0.30, -2.1, 3.25, RED);
    } else {
        wing(&mut m, 8.6, 2.05, -2.4, 2.8, GOLD);
        wing(&mut m, 7.25, -0.68, -1.76, 2.65, CANVAS);
    }
    wing(&mut m, 3.05, 0.42, 4.13, 1.75, SEA);
    for x in [-0.09, 0.09] {
        let mut rudder = [
            v(x, 0.45, 3.7),
            v(x, 2.64, 5.25),
            v(x, 2.43, 6.12),
            v(x, 0.41, 6.48),
        ];
        if x > 0. {
            rudder.reverse();
        }
        m.quad(rudder, RED);
    }
    for side in [-1., 1.] {
        // Roundels sit on the outer panels, visible from the deck as well as from above.
        for (span, height, z, normal) in [
            (5.20, if attack { 0.01 } else { 2.38 }, -1.24, Vec3::Y),
            (4.6, if attack { -0.22 } else { -0.60 }, -0.72, Vec3::NEG_Y),
        ] {
            let mut center = v(side * span, height, z);
            for (radius, paint) in [(0.72, INK), (0.55, CANVAS), (0.31, RED)] {
                m.tube(center, center + normal * 0.012, radius, radius, paint);
                center += normal * 0.014;
            }
        }
        let x = side * 2.03;
        skin(
            &mut m,
            x,
            &[
                (-4.41, -2.34, 0.03, 0.05, CANVAS),
                (-3.27, -2.55, 0.48, 0.41, SEA),
                (-1.27, -2.63, 0.56, 0.52, SEA),
                (1.51, -2.66, 0.49, 0.35, SEA),
                (3.58, -2.48, 0.11, 0.13, INK),
                (3.68, -2.48, 0.02, 0.03, INK),
            ],
        );
        for z in [-1.93, 1.15] {
            m.tube(
                v(side * 0.45, -0.52, z),
                v(x, -2.44, z + 0.36),
                0.10,
                0.10,
                INK,
            );
        }
        if attack {
            let muzzle = v(side * 2.45, -0.24, -3.2);
            m.tube(muzzle, muzzle + Vec3::Z * 1.8, 0.16, 0.18, INK);
            continue;
        }
        for x in [side * 2.45, side * 5.85] {
            let foot = v(x, -0.49, 0.11);
            m.tube(foot, v(x + 0.25 * side, 2.25, -1.76), 0.09, 0.09, INK);
            m.tube(foot, v(x + 0.25 * side, 2.25, -0.15), 0.08, 0.08, INK);
        }
    }
    m.finish()
}
pub const HUB: Vec3 = Vec3::new(0., 0., -5.50);
/// The model is 17 m across the wings, which beside a 30 m ship is a giant. A base's
/// spotter is drawn at this share of that: 3 m across and 2 m long.
pub const ALOFT: f32 = 0.17;
/// Model metres from the origin to the farthest wingtip, of either airframe.
pub const REACH: f32 = 9.1;
/// Model metres from the origin down to the keel of the fuselage.
pub const KEEL: f32 = 0.96;
pub fn propeller() -> Mesh {
    let mut m = Facets::default();
    for i in 0..3 {
        let r = Quat::from_rotation_z(i as f32 * std::f32::consts::TAU / 3.);
        let blade = [
            Vec3::new(-0.10, 0.10, 0.),
            Vec3::new(0.14, 0.16, 0.),
            Vec3::new(0.32, 1.67, 0.07),
            Vec3::new(0.11, 1.77, 0.07),
        ]
        .map(|p| r * p);
        m.quad(blade, INK);
        let mut back = blade;
        back.reverse();
        m.quad(back, INK);
        m.tube(
            r * Vec3::new(0.21, 1.55, 0.06),
            r * Vec3::new(0.21, 1.72, 0.07),
            0.12,
            0.09,
            GOLD,
        );
    }
    m.tube(Vec3::Z * 0.08, Vec3::NEG_Z * 0.38, 0.25, 0.09, GOLD);
    m.finish()
}
