//! Speed-driven spray where the water peels off each shoulder of the bow.
use crate::{
    camera::FollowCamera,
    seascape::Sea,
    state::{Campaign, Mode, PlayerHelm},
    surfaces::sample,
};
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology, VertexAttributeValues},
    prelude::*,
};

const DROPS: usize = 212;
#[derive(Component)]
pub(crate) struct BowSpray;

pub(super) fn spawn(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    // Four feathered triangles share a bright centre. All droplets draw as one mesh.
    let edges = [0.72, 0.83, 0.84, 0.];
    let centre = [0.81, 0.87, 0.85, 0.83];
    let colours = [centre, edges, edges, edges, edges].repeat(DROPS);
    let indices = (0..DROPS as u32)
        .flat_map(|i| [0, 1, 2, 0, 2, 3, 0, 3, 4, 0, 4, 1].map(|j| i * 5 + j))
        .collect();
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.; 3]; DROPS * 5])
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0., 1., 0.]; DROPS * 5])
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
    .with_inserted_indices(Indices::U32(indices));
    commands.spawn((
        BowSpray,
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            unlit: true,
            ..default()
        })),
        Visibility::Hidden,
        NoFrustumCulling,
        NotShadowCaster,
    ));
}

pub(crate) fn animate(
    run: Res<Campaign>,
    helm: Res<PlayerHelm>,
    sea: Res<Sea>,
    clock: Res<Time>,
    camera: Single<&Transform, (With<FollowCamera>, Without<BowSpray>)>,
    mut spray: Single<(&Mesh3d, &mut Transform, &mut Visibility), With<BowSpray>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let (handle, root, shown) = &mut *spray;
    let strength = ((helm.speed - 2.) / 7.).clamp(0., 1.);
    if strength == 0. || !(run.mode == Mode::Title || run.under_way()) {
        shown.set_if_neq(Visibility::Hidden);
        return;
    }
    shown.set_if_neq(Visibility::Inherited);
    let hull = run.hull_shape();
    let stem = helm.position + helm.forward() * (hull.bow * 0.89);
    root.translation = Vec3::new(stem.x, 0., stem.y);
    root.rotation = Quat::from_rotation_y(-helm.heading);
    let eye = root.rotation.inverse() * (camera.translation - root.translation);
    let Some(mut mesh) = meshes.get_mut(&handle.0) else {
        return;
    };
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    else {
        return;
    };
    let time = clock.elapsed_secs();
    for (i, verts) in positions.chunks_exact_mut(5).enumerate() {
        let side = if i % 2 == 0 { 1. } else { -1. };
        let i = i as u32;
        let cycle = time * (1.05 + 0.6 * sample(i, 0, 1)) + sample(i, 0, 2);
        let age = cycle.fract();
        let noise = |seed| sample(i, cycle.floor() as u32, seed);
        // A drop clears the crest, travels aft relative to the ship and falls back through the sheet.
        let launch = 0.20 + 3.3 * noise(3).powi(2);
        let spread = (0.55 + 1.55 * noise(4)) * strength;
        let rise = (0.14 + 0.72 * noise(5).powi(2)) * strength;
        let aft = 0.8 + noise(6) * 2.6;
        let mut p = Vec3::new(
            side * ((launch + 0.6).powf(0.88) * hull.radius * 0.20 + age * spread),
            0.,
            launch + age * aft,
        );
        p.y = sea.height((root.translation + root.rotation * p).xz())
            + rise * (4. * age * (1. - age))
            - 0.055;
        let tangent = Vec3::new(side * spread, 4. * rise * (1. - 2. * age), aft).normalize();
        let width = (eye - p).cross(tangent).normalize_or(Vec3::X);
        let envelope = (age * 10.).clamp(0., 1.) * ((1. - age) * 7.).clamp(0., 1.);
        let radius = (0.010 + 0.029 * noise(8)) * envelope * strength;
        let along = tangent * (radius * 1.85);
        let across = width * radius;
        verts.copy_from_slice(
            &[p, p + along, p + across, p - along, p - across].map(|v| v.to_array()),
        );
    }
}
