//! An airfield's light bomber and torpedo plane: low-winged single-engined monoplanes on
//! fixed, spatted wheels, in drab with the enemy's red bands, one with a bomb slung under
//! its belly and the other a torpedo. Built like the spotter, local -Z the propeller end,
//! so its propeller turns at the same hub.
use super::spotter_model::{skin, wing};
use crate::scene::Facets;
use bevy::prelude::*;

const DRAB: Color = Color::srgb(0.36, 0.40, 0.30);
const BELLY: Color = Color::srgb(0.55, 0.58, 0.54);
const RED: Color = Color::srgb(0.74, 0.15, 0.11);
const INK: Color = Color::srgb(0.13, 0.15, 0.15);
const GLASS: Color = Color::srgb(0.21, 0.44, 0.54);
const STEEL: Color = Color::srgb(0.30, 0.32, 0.33);
const TYRE: Color = Color::srgb(0.07, 0.07, 0.07);

/// Model metres from the origin down to the bottom of the wheels, which stand on the
/// ground when it rolls.
pub const WHEELS: f32 = 2.5;
/// How large each is drawn, as a share of the model: about 6 m across the wings.
pub const BOMBER: f32 = 0.34;
pub const TORPEDO_PLANE: f32 = 0.33;

pub fn bomber() -> Mesh {
    airframe(false)
}

pub fn torpedo_plane() -> Mesh {
    airframe(true)
}

fn airframe(torpedo: bool) -> Mesh {
    let mut m = Facets::default();
    let v = Vec3::new;
    skin(
        &mut m,
        0.,
        &[
            (-5.45, 0., 0.01, 0.01, INK),
            (-5.32, 0., 0.78, 0.8, INK),
            (-4.92, 0., 0.96, 0.94, RED),
            (-4.1, 0., 0.98, 0.95, DRAB),
            (-1.3, 0., 1.02, 1.0, DRAB),
            (2.2, 0.1, 0.8, 0.74, DRAB),
            (3.7, 0.22, 0.5, 0.5, RED),
            (4.3, 0.25, 0.4, 0.4, DRAB),
            (6.6, 0.4, 0.04, 0.08, DRAB),
        ],
    );
    // A long glasshouse over the crew: pilot, and a gunner or the observer behind.
    skin(
        &mut m,
        0.,
        &[
            (-2.4, 0.56, 0.52, 0.18, GLASS),
            (-1.8, 0.78, 0.74, 0.74, GLASS),
            (-0.7, 0.8, 0.76, 0.8, INK),
            (-0.5, 0.8, 0.76, 0.8, GLASS),
            (1.6, 0.74, 0.66, 0.66, GLASS),
            (2.5, 0.52, 0.48, 0.2, DRAB),
        ],
    );
    // Low wings, and the tailplane.
    wing(&mut m, 8.4, -0.55, -2.2, 3.4, DRAB);
    wing(&mut m, 3.2, 0.44, 4.3, 1.8, DRAB);
    for x in [-0.09, 0.09] {
        let mut rudder = [
            v(x, 0.48, 3.8),
            v(x, 2.75, 5.4),
            v(x, 2.55, 6.3),
            v(x, 0.44, 6.66),
        ];
        if x > 0. {
            rudder.reverse();
        }
        m.quad(rudder, RED);
    }
    for side in [-1., 1.] {
        // The enemy's red discs, under and over the wings.
        for (span, height, z, normal) in
            [(5.2, -0.26, -1.3, Vec3::Y), (4.6, -0.5, -0.8, Vec3::NEG_Y)]
        {
            let mut center = v(side * span, height, z);
            for (radius, paint) in [(0.72, BELLY), (0.5, RED)] {
                m.tube(center, center + normal * 0.012, radius, radius, paint);
                center += normal * 0.014;
            }
        }
        // A fixed leg under each wing, with its wheel in a spat.
        let x = side * 2.1;
        m.tube(
            v(side * 1.2, -0.6, -1.6),
            v(x, -1.9, -1.4),
            0.12,
            0.1,
            STEEL,
        );
        skin(
            &mut m,
            x,
            &[
                (-2.05, -2.0, 0.05, 0.05, DRAB),
                (-1.75, -2.0, 0.32, 0.5, DRAB),
                (-1.15, -2.0, 0.32, 0.5, DRAB),
                (-0.75, -2.0, 0.05, 0.05, DRAB),
            ],
        );
        m.tube(
            v(x - 0.12, -2.05, -1.45),
            v(x + 0.12, -2.05, -1.45),
            0.45,
            0.45,
            TYRE,
        );
    }
    m.tube(v(0., -0.6, 6.2), v(0., -1.0, 6.3), 0.06, 0.06, STEEL);
    if torpedo {
        // The torpedo, slung long under the belly.
        let (nose, tail) = (v(0., -1.25, -3.1), v(0., -1.25, 3.0));
        m.tube(nose, tail, 0.3, 0.26, STEEL);
        m.tube(nose, nose - Vec3::Z * 0.4, 0.3, 0.05, INK);
        for (x, y) in [(0.55, 0.), (-0.55, 0.), (0., 0.55), (0., -0.55)] {
            m.quad(
                [
                    tail,
                    tail + v(x, y, 0.),
                    tail + v(x, y, 0.6),
                    tail + Vec3::Z * 0.6,
                ],
                INK,
            );
        }
        m.tube(v(0., -0.9, -2.6), v(0., -1.0, 1.9), 0.06, 0.06, STEEL);
    } else {
        // A bomb on its crutch under the belly.
        let (nose, tail) = (v(0., -1.35, -0.9), v(0., -1.35, 1.4));
        m.tube(nose, tail, 0.36, 0.22, INK);
        m.tube(nose, nose - Vec3::Z * 0.5, 0.36, 0.06, INK);
        for (x, y) in [(0.5, 0.), (-0.5, 0.), (0., 0.5), (0., -0.5)] {
            m.quad(
                [
                    tail,
                    tail + v(x, y, 0.),
                    tail + v(x, y, 0.5),
                    tail + Vec3::Z * 0.5,
                ],
                RED,
            );
        }
        m.tube(v(0., -0.95, -0.4), v(0., -1.05, 0.9), 0.1, 0.1, STEEL);
    }
    m.finish()
}
