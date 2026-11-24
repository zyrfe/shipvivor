//! Cloth flies from the hoist and a train of soft billows marks each working chimney.
use crate::camera::FollowCamera;
use crate::ensign::{Colours, Side};
use crate::seascape::Sea;
use crate::state::{Campaign, Encounter};
use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    light::NotShadowCaster,
    mesh::PrimitiveTopology,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use shipvivor_ensign::Flag;
use std::f32::consts::TAU;

const BILLOWS: u8 = 14;
const CLOTH_FRAMES: usize = 32;
/// Texels the painted cloth spans from hoist to fly and from head to foot.
const CLOTH: (u32, u32) = (112, 40);

#[derive(Component)]
pub(crate) enum Signal {
    Flag,
    Billow(u8),
}

pub(super) struct SignalKit {
    flags: [Handle<Mesh>; CLOTH_FRAMES],
    /// Each side's painted cloth, indexed by `Side`. `dye` repaints them.
    ensigns: [Handle<StandardMaterial>; 2],
    card: Handle<Mesh>,
    smoke: [Handle<StandardMaterial>; 32],
}

impl SignalKit {
    pub fn new(
        meshes: &mut Assets<Mesh>,
        paint: &mut Assets<StandardMaterial>,
        images: &mut Assets<Image>,
    ) -> Self {
        let texture = images.add(billow_texture());
        Self {
            flags: std::array::from_fn(|i| meshes.add(flag(i as f32 * TAU / CLOTH_FRAMES as f32))),
            ensigns: std::array::from_fn(|_| {
                paint.add(StandardMaterial {
                    cull_mode: None,
                    unlit: true,
                    ..default()
                })
            }),
            card: meshes.add(Rectangle::new(1., 1.)),
            smoke: std::array::from_fn(|i| {
                let age = i as f32 / 31.;
                let alpha = ((1. - age) * 1.85).min(1.) * (0.45 + age * 12.).min(1.) * 0.81;
                // Dense soot at the lip catches more sky light as the plume entrains air.
                let light = 0.71 + 1.22 * age;
                paint.add(StandardMaterial {
                    base_color_texture: Some(texture.clone()),
                    base_color: Color::linear_rgba(light * 1.02, light, light * 0.94, alpha),
                    alpha_mode: AlphaMode::Blend,
                    fog_enabled: false,
                    unlit: true,
                    ..default()
                })
            }),
        }
    }

    /// A side's flag hoisted on `root`, with the head of its hoist at `hoist`.
    pub fn spawn_flag(&self, cmd: &mut Commands, root: Entity, side: Side, hoist: Transform) {
        cmd.spawn((
            Signal::Flag,
            hoist,
            Mesh3d(self.flags[0].clone()),
            MeshMaterial3d(self.ensigns[side as usize].clone()),
            NotShadowCaster,
            ChildOf(root),
        ));
    }

    pub fn spawn_smoke(&self, cmd: &mut Commands, root: Entity) {
        for i in 0..BILLOWS {
            cmd.spawn((
                Signal::Billow(i),
                Mesh3d(self.card.clone()),
                MeshMaterial3d(self.smoke[0].clone()),
                NotShadowCaster,
                ChildOf(root),
            ));
        }
    }
}

// A forked fly and travelling folds; the two corners on the pole stay fixed. The cloth
// is painted with a side's colours, from the hoist at `u = 0` and the head at `v = 0`,
// and the folds shade it.
fn flag(phase: f32) -> Mesh {
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut shades = Vec::new();
    for i in 0..12 {
        let vertex = |j: u32, hem: f32| {
            let u = j as f32 / 12.;
            let fold = (8.1 * u - phase).sin() + 0.33 * (13. * u - 2. * phase + hem * 0.6).sin();
            let notch = if hem == 0. { 1. } else { 0. };
            Vec3::new(
                5.5 * u - notch * (u - 0.75).max(0.) * 5.4,
                -0.98 + hem * (0.98 - u * 0.15) - u * (0.34 + 0.14 * fold),
                u * fold * 0.92,
            )
        };
        for hem in [-1., 0.] {
            let corners = [(i, hem), (i + 1, hem), (i + 1, hem + 1.), (i, hem + 1.)];
            let shade = 0.83 + 0.16 * (8.1 * (i as f32 + 0.5) / 12. - phase).cos();
            let shade = Color::srgb(shade, shade, shade).to_linear().to_f32_array();
            for (j, hem) in [0, 1, 2, 0, 2, 3].map(|k| corners[k]) {
                positions.push(vertex(j, hem).to_array());
                uvs.push([j as f32 / 12., (1. - hem) * 0.5]);
                shades.push(shade);
            }
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, shades)
    .with_computed_flat_normals()
}

fn cloth(flag: &Flag) -> Image {
    let (width, height) = CLOTH;
    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        flag.paint(width, height),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::linear();
    image
}

/// Paint each side's cloth afresh when the colours change. A new image, rather than one
/// rewritten in place, makes certain the material picks it up.
pub(crate) fn dye(
    colours: Res<Colours>,
    kit: Res<super::Kit>,
    mut images: ResMut<Assets<Image>>,
    mut paint: ResMut<Assets<StandardMaterial>>,
) {
    if !colours.is_changed() {
        return;
    }
    for side in [Side::Breakwater, Side::Enemy] {
        let painted = images.add(cloth(colours.flag(side)));
        if let Some(mut material) = paint.get_mut(&kit.signals.ensigns[side as usize]) {
            material.base_color_texture = Some(painted);
        }
    }
}

fn billow_texture() -> Image {
    const SIDE: u32 = 96;
    let mut rgba = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let p = (Vec2::new(x as f32, y as f32) + 0.5) * (2. / SIDE as f32) - 1.;
            // A lit cross-section through overlapping rolls of exhaust. Turbulence warps
            // both the outline and the lit edge where one roll overtakes the next.
            let uv = (p + 1.) * 0.5;
            let noise = |n, seed| super::surfaces::grain(uv, IVec2::splat(n), seed) - 0.5;
            let p = p
                + 0.23 * Vec2::new(noise(5, 4), noise(6, 92))
                + 0.10 * Vec2::new(noise(16, 7), noise(15, 3));
            let mut depth = 0_f32;
            let mut alpha = 0_f32;
            let mut light = 0_f32;
            for (cx, cy, r) in [
                (0., 0., 0.58),
                (-0.37, 0.13, 0.45),
                (-0.20, -0.35, 0.43),
                (0.27, -0.28, 0.48),
                (0.36, 0.18, 0.45),
                (0., 0.40, 0.41),
            ] {
                let q = (p - Vec2::new(cx, cy)) / r;
                let z = (1. - q.length_squared()).max(0.).sqrt();
                alpha = alpha.max(z);
                if z * r > depth {
                    depth = z * r;
                    light = (-0.40 * q.x - 0.60 * q.y + 0.39 * z).clamp(-0.3, 1.);
                }
            }
            let alpha =
                (alpha * 0.94 + alpha * (0.45 * noise(8, 19) + 0.22 * noise(15, 77))).clamp(0., 1.);
            let ash =
                (102. + 46. * light + 28. * noise(8, 19) + 17. * noise(18, 54) + 8. * noise(36, 5))
                    as u8;
            rgba.extend([
                ash + 6,
                ash + 4,
                ash,
                (255. * alpha * alpha * (3. - 2. * alpha)) as u8,
            ]);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: SIDE,
            height: SIDE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::linear();
    image
}

pub(crate) fn animate(
    kit: Res<super::Kit>,
    sea: Res<Sea>,
    run: Res<Campaign>,
    battle: Res<Encounter>,
    camera: Single<&Transform, (With<FollowCamera>, Without<Signal>)>,
    anchors: Query<(&Transform, &super::Actor), Without<Signal>>,
    mut signals: Query<(
        &Signal,
        &ChildOf,
        &mut Transform,
        &mut Mesh3d,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
) {
    let weather = sea.weather();
    let wind = shipvivor_compass::direction(weather.wind).extend(0.).xzy();
    let across = wind.cross(Vec3::Y);
    // The breeze in metres a second, as Breakwater's pennant feels it.
    let breeze = shipvivor_compass::direction(weather.wind) * (4. + 8. * weather.sea);
    for (signal, parent, mut xf, mut mesh, mut paint) in &mut signals {
        // Signal parents sit at the lip or the halyard with their axes aligned to the
        // chart, or are the hull a flag is hoisted on.
        let Ok((anchor, actor)) = anchors.get(parent.parent()) else {
            continue;
        };
        let seed = anchor.translation.dot(Vec3::new(0.018, 0., 0.037));
        match signal {
            Signal::Flag => {
                // A hull's flag keeps its own time, not one set by where she is.
                let seed = (actor.0 % 89) as f32 * 0.113;
                let cycle = (run.elapsed * (1.05 + 0.95 * weather.sea) + seed).rem_euclid(1.);
                let frame = (cycle * CLOTH_FRAMES as f32) as usize % CLOTH_FRAMES;
                mesh.set_if_neq(Mesh3d(kit.signals.flags[frame].clone()));
                // A raider's flag streams with the wind over her deck, her own way included,
                // and turns with her hull.
                let way = match actor.1 {
                    super::Shape::Ship(_) => battle
                        .raiders()
                        .iter()
                        .find(|r| r.body.id == actor.0)
                        .map_or(Vec2::ZERO, |r| r.motion()),
                    _ => Vec2::ZERO,
                };
                let flow = breeze - way;
                let hull = anchor.rotation * Vec3::X;
                xf.rotation = Quat::from_rotation_y(hull.z.atan2(hull.x) - flow.y.atan2(flow.x));
            }
            Signal::Billow(i) => {
                let t = (run.elapsed * 0.115 + seed + f32::from(*i) / f32::from(BILLOWS))
                    .rem_euclid(1.);
                let turn = f32::from(*i) * 2.40;
                let eddy = (9. * t + seed * TAU).sin() + 0.43 * (5. * t + turn).cos();
                xf.translation = Vec3::Y * (0.3 + t * (19. + 25. * t))
                    + wind * ((13. + weather.sea * 31.) * t * t)
                    + across * (3.5 * t * eddy);
                xf.rotation =
                    camera.rotation * Quat::from_rotation_z(0.31 * eddy + 0.81 * turn.sin());
                xf.scale = Vec3::splat(2.85 + (15.6 + 2.4 * turn.sin()) * t);
                let shade = (t * 31.).round() as usize;
                paint.set_if_neq(MeshMaterial3d(kit.signals.smoke[shade].clone()));
            }
        }
    }
}

/// The commissioning pennant streams with the wind felt on deck, including the ship's way.
pub(crate) fn fly_pennant(
    kit: Res<super::Kit>,
    sea: Res<Sea>,
    clock: Res<Time>,
    run: Res<Campaign>,
    helm: Res<crate::state::PlayerHelm>,
    mut masthead: Single<
        (
            &mut Transform,
            &mut Mesh3d,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        With<crate::scene::Masthead>,
    >,
) {
    let docked = matches!(run.screen(), crate::state::Screen::Drydock(_));
    let (heading, speed) = if docked {
        (0., 0.)
    } else {
        (helm.heading, helm.speed)
    };
    let weather = sea.weather();
    let wind = shipvivor_compass::direction(weather.wind - heading) * (4. + 8. * weather.sea);
    let apparent = wind + Vec2::Y * speed;
    let phase = (clock.elapsed_secs_wrapped() * 1.8).fract();
    let (pose, mesh, material) = &mut *masthead;
    pose.rotation = Quat::from_rotation_y(-apparent.y.atan2(apparent.x));
    let frame = (phase * CLOTH_FRAMES as f32) as usize % CLOTH_FRAMES;
    mesh.set_if_neq(Mesh3d(kit.signals.flags[frame].clone()));
    material.set_if_neq(MeshMaterial3d(
        kit.signals.ensigns[Side::Breakwater as usize].clone(),
    ));
}
