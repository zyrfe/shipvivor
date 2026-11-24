use crate::state::{Campaign, Screen};
use bevy::{
    asset::RenderAssetUsages,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology, VertexAttributeValues},
    prelude::*,
};

const COLUMNS: usize = 22;
const ROWS: usize = 32;
const HEIGHT: f32 = 1.90;
const FOREDECK_Z: f32 = -14.05;

#[derive(Component)]
pub(crate) struct ProwCaptain {
    width: f32,
}

/// A painted figure on a flexible mesh. The UVs also locate each lock of hair
/// and the free ends of the scarf and coat, keeping the feet planted on deck.
pub(super) fn spawn(
    cmd: &mut Commands,
    hull: Entity,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    image: Handle<Image>,
    aspect: f32,
) {
    let width = HEIGHT * aspect;
    let uv: Vec<_> = (0..=ROWS)
        .flat_map(|row| {
            (0..=COLUMNS).map(move |col| [col as f32 / COLUMNS as f32, row as f32 / ROWS as f32])
        })
        .collect();
    let positions: Vec<_> = uv.iter().map(|&[u, v]| position(u, v, width, 0.)).collect();
    let normals = vec![[0., 0., 1.]; uv.len()];
    let indices = (0..ROWS)
        .flat_map(|row| {
            (0..COLUMNS).flat_map(move |col| {
                let i = (row * (COLUMNS + 1) + col) as u32;
                let below = i + COLUMNS as u32 + 1;
                [i, below, i + 1, i + 1, below, below + 1]
            })
        })
        .collect();
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U32(indices));
    cmd.spawn((
        ProwCaptain { width },
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(image),
            alpha_mode: AlphaMode::Mask(0.4),
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(0.08, crate::scene::deck_height(FOREDECK_Z), FOREDECK_Z)
            .with_rotation(Quat::from_rotation_y(2.48)),
        Visibility::Inherited,
        NotShadowCaster,
        ChildOf(hull),
    ));
}

fn position(u: f32, v: f32, width: f32, t: f32) -> [f32; 3] {
    // Fade the flex to zero across the scarf knot, cheek and coat seams.
    let free = |edge: f32, span: f32| ((edge - u) / span).clamp(0., 1.);
    let band = |centre: f32, half: f32| (1. - ((v - centre) / half).abs()).clamp(0., 1.);
    let scarf = free(0.58, 0.50) * band(0.265, 0.093);
    let hair = free(0.50, 0.22) * band(0.096, 0.095);
    let coat = free(0.44, 0.37) * band(0.695, 0.13);
    let gust = (5.6 * t + 9. * u).sin() + 0.36 * (9.3 * t + 6. * v + 5. * u).sin();
    let lift = gust * (0.085 * scarf + 0.038 * hair + 0.037 * coat);
    [
        (u - 0.5) * width + lift * 0.27,
        (1. - v) * HEIGHT + lift,
        lift * 0.66,
    ]
}

pub fn wind(
    run: Res<Campaign>,
    time: Res<Time>,
    captain: Single<(&ProwCaptain, &Mesh3d, &mut Visibility)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let (figure, handle, mut visibility) = captain.into_inner();
    if !matches!(run.screen(), Screen::Title | Screen::Options) {
        visibility.set_if_neq(Visibility::Hidden);
        return;
    }
    visibility.set_if_neq(Visibility::Inherited);
    let Some(mut mesh) = meshes.get_mut(&handle.0) else {
        return;
    };
    let Some(VertexAttributeValues::Float32x3(vertices)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    else {
        return;
    };
    let t = time.elapsed_secs();
    for (row, line) in vertices.chunks_exact_mut(COLUMNS + 1).enumerate() {
        for (col, vertex) in line.iter_mut().enumerate() {
            *vertex = position(
                col as f32 / COLUMNS as f32,
                row as f32 / ROWS as f32,
                figure.width,
                t,
            );
        }
    }
}
