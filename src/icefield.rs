//! Ice adrift: growlers and bergs drawn where the encounter has carried them, riding
//! the swell. Each shape comes in a few cuts so neighbouring pieces differ.
use crate::scene::Facets;
use crate::state::{Campaign, Encounter, Mode};
use bevy::prelude::*;
use shipvivor_encounter::IceShape;
use std::collections::HashMap;
use std::f32::consts::TAU;

/// Different cuts of each shape. A piece keeps its cut for as long as it floats.
const CUTS: u64 = 3;
const SHAPES: [IceShape; 4] = [
    IceShape::Growler,
    IceShape::Dome,
    IceShape::Pinnacle,
    IceShape::Tabular,
];

pub struct IcefieldPlugin;
impl Plugin for IcefieldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, prepare)
            .add_systems(Update, drift.after(crate::seascape::follow));
    }
}

#[derive(Resource)]
struct IceKit {
    cuts: HashMap<(IceShape, u64), Handle<Mesh>>,
    ice: Handle<StandardMaterial>,
}

/// The rendered piece of ice with this encounter id.
#[derive(Component)]
struct Floe(u64);

fn prepare(
    mut cmd: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut paint: ResMut<Assets<StandardMaterial>>,
) {
    let cuts = SHAPES
        .into_iter()
        .flat_map(|shape| (0..CUTS).map(move |cut| (shape, cut)))
        .map(|(shape, cut)| ((shape, cut), meshes.add(mesh(shape, cut))))
        .collect();
    // Old ice is dense and glassy. A little of the light it takes in comes back out of the
    // shaded faces, so they glow blue rather than going grey.
    let ice = paint.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.38,
        reflectance: 0.42,
        emissive: LinearRgba::rgb(0.09, 0.18, 0.24),
        ..default()
    });
    cmd.insert_resource(IceKit { cuts, ice });
}

fn drift(
    mut cmd: Commands,
    kit: Res<IceKit>,
    run: Res<Campaign>,
    battle: Res<Encounter>,
    sea: Res<crate::seascape::Sea>,
    mut floes: Query<(Entity, &Floe, &mut Transform)>,
) {
    let shown = !matches!(run.mode, Mode::Title | Mode::Trial);
    let mut wanted: HashMap<u64, Transform> = HashMap::new();
    if shown {
        for berg in battle.icebergs() {
            let ride = sea
                .swell
                .ride(berg.body.pos, berg.body.heading, berg.radius, berg.radius);
            // Most of a berg is under water. The bigger it is, the less the swell moves it.
            let steady = (berg.radius / 25.).clamp(0., 0.8);
            let heave = ride.heave * (1. - 0.7 * steady);
            let tilt = 1. - steady;
            let pose = Transform::from_xyz(berg.body.pos.x, heave, berg.body.pos.y)
                .with_rotation(
                    Quat::from_rotation_y(-berg.body.heading)
                        * Quat::from_rotation_x(-ride.pitch * tilt)
                        * Quat::from_rotation_z(ride.roll * tilt),
                )
                .with_scale(Vec3::splat(berg.radius));
            wanted.insert(berg.body.id, pose);
        }
    }
    for (entity, floe, mut transform) in &mut floes {
        match wanted.remove(&floe.0) {
            Some(pose) => *transform = pose,
            None => cmd.entity(entity).despawn(),
        }
    }
    for berg in battle.icebergs() {
        if let Some(pose) = wanted.remove(&berg.body.id) {
            let cut = berg.body.id % CUTS;
            cmd.spawn((
                Floe(berg.body.id),
                Mesh3d(kit.cuts[&(berg.shape, cut)].clone()),
                MeshMaterial3d(kit.ice.clone()),
                pose,
            ));
        }
    }
}

/// Repeatable noise in [0, 1) for a cut of ice.
fn grain(cut: u64, i: u32, j: u32) -> f32 {
    let mut z = cut
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(u64::from(i) << 20)
        .wrapping_add(u64::from(j));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((z ^ (z >> 31)) >> 40) as f32 / (1_u64 << 24) as f32
}

/// A piece of ice one metre in radius at the waterline, built in rings from below the
/// water up to its crown. Heights follow the encounter's freeboard for the shape.
fn mesh(shape: IceShape, cut: u64) -> Mesh {
    let top = shape.freeboard();
    // (radius, height) of each ring, as shares of the waterline radius and the freeboard.
    let rings: &[(f32, f32)] = match shape {
        IceShape::Growler => &[
            (0.92, -0.9),
            (1., 0.),
            (0.92, 0.45),
            (0.62, 0.85),
            (0.2, 1.),
        ],
        IceShape::Dome => &[
            (0.94, -0.6),
            (1., 0.),
            (0.94, 0.3),
            (0.8, 0.62),
            (0.52, 0.88),
            (0.18, 1.),
        ],
        IceShape::Pinnacle => &[
            (0.95, -0.35),
            (1., 0.),
            (0.82, 0.18),
            (0.55, 0.42),
            (0.38, 0.68),
            (0.2, 0.9),
            (0.04, 1.),
        ],
        IceShape::Tabular => &[
            (0.97, -0.9),
            (1., 0.),
            (0.99, 0.35),
            (0.97, 0.92),
            (0.9, 1.),
        ],
    };
    // Slabs break along a few straight faces. Other ice is lumpier and rounder.
    let sides: u32 = match shape {
        IceShape::Tabular => 7 + (cut as u32 % 3),
        IceShape::Growler => 9,
        _ => 12,
    };
    let lean = Vec2::new(grain(cut, 99, 0) - 0.5, grain(cut, 99, 1) - 0.5) * 0.5;
    let vertex = |ring: usize, i: u32| {
        let i = i % sides;
        let (share, rise) = rings[ring];
        let wobble = if shape == IceShape::Tabular {
            0.2
        } else {
            0.32
        };
        let a = (i as f32 + 0.6 * (grain(cut, i, 7) - 0.5)) * TAU / sides as f32;
        let r = share * (1. - wobble * grain(cut, i, ring as u32 / 2));
        // Spires lean, so pinnacles are not cones.
        let drift = if shape == IceShape::Pinnacle {
            lean * rise.max(0.)
        } else {
            Vec2::ZERO
        };
        let p = Vec2::from_angle(a) * r + drift;
        // The crown is broken, not level, except across a slab's flat top.
        let rough = if ring + 1 == rings.len() && shape != IceShape::Tabular {
            0.9 + 0.1 * grain(cut, i, 77)
        } else if ring > 0 && rise > 0. && rise < 1. {
            1. + 0.08 * (grain(cut, i, 55 + ring as u32) - 0.5)
        } else {
            1.
        };
        Vec3::new(p.x, rise * top * rough, p.y)
    };
    let mut facets = Facets::default();
    let colour = |v: [Vec3; 3], k: f32| {
        let mid = (v[0] + v[1] + v[2]) / 3.;
        let normal = (v[1] - v[0]).cross(v[2] - v[0]).normalize_or_zero();
        let rgb = if mid.y < -0.02 {
            // Under water the ice shows turquoise through the sea.
            [62., 168., 178.]
        } else if mid.y < 0.12 * top {
            // The waterline is worn into a smooth, wet notch.
            [128., 206., 214.]
        } else if normal.y.abs() > 0.62 {
            [240., 247., 251.]
        } else {
            [172., 222., 240.]
        };
        let gain = 0.9 + 0.16 * k;
        Color::srgb_u8(
            (rgb[0] * gain).min(255.) as u8,
            (rgb[1] * gain).min(255.) as u8,
            (rgb[2] * gain).min(255.) as u8,
        )
    };
    let mut face = |v: [Vec3; 3], k: f32| facets.triangle(v, colour(v, k));
    for ring in 1..rings.len() {
        for i in 0..sides {
            let [a, b, c, d] = [
                vertex(ring - 1, i),
                vertex(ring - 1, i + 1),
                vertex(ring, i + 1),
                vertex(ring, i),
            ];
            let k = grain(cut, i, 40 + ring as u32);
            face([a, d, b], k);
            face([b, d, c], k);
        }
    }
    let crown = rings.len() - 1;
    let centre = (0..sides).map(|i| vertex(crown, i)).sum::<Vec3>() / sides as f32
        + Vec3::Y
            * if shape == IceShape::Tabular {
                0.
            } else {
                0.06 * top
            };
    for i in 0..sides {
        face([vertex(crown, i), centre, vertex(crown, i + 1)], 0.5);
    }
    let bottom = (0..sides).map(|i| vertex(0, i)).sum::<Vec3>() / sides as f32;
    for i in 0..sides {
        face([vertex(0, i + 1), bottom, vertex(0, i)], 0.5);
    }
    facets.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_cut_stands_on_the_waterline_at_its_freeboard() {
        for shape in SHAPES {
            for cut in 0..CUTS {
                let ice = mesh(shape, cut);
                let points = ice
                    .attribute(Mesh::ATTRIBUTE_POSITION)
                    .unwrap()
                    .as_float3()
                    .unwrap();
                let top = points.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
                let reach = points
                    .iter()
                    .filter(|p| p[1].abs() < 1e-4)
                    .map(|p| Vec2::new(p[0], p[2]).length())
                    .fold(0., f32::max);
                assert!(
                    (top - shape.freeboard()).abs() < 0.08 * shape.freeboard() + 0.01,
                    "{shape:?} cut {cut} stands {top}"
                );
                // The waterline stays within the encounter's radius for the piece.
                assert!(
                    reach <= 1.05 && reach > 0.6,
                    "{shape:?} cut {cut} reaches {reach}"
                );
            }
        }
    }
}
