//! The sea and sky. They share a light direction, the weather and world-space shading.
//!
//! `shipvivor_sea` decides where the water is. This module draws it: a lattice
//! that follows the ship and is finest around her, moved by the swell and by the
//! wake patch, with the smaller ripples left to the shader.
use crate::state::{Campaign, Embarked, Encounter, Mode, PlayerHelm, Screen};
use bevy::{
    asset::{RenderAssetUsages, load_internal_asset, uuid_handle},
    camera::visibility::NoFrustumCulling,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    light::{CascadeShadowConfig, CascadeShadowConfigBuilder, NotShadowCaster},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    reflect::TypePath,
    render::render_resource::{AsBindGroup, Extent3d, ShaderType, TextureDimension, TextureFormat},
    shader::ShaderRef,
};
use shipvivor_encounter::{EffectKind, Hull};
use shipvivor_sea::{CELL, CHOP, FOAM_SIDE, Keel, SIDE, Swell, TRAINS, Wake};

mod wakes;
use wakes::{Steaming, Tracks, Wakes};

const SHADER: Handle<Shader> = uuid_handle!("55e88192-7610-44f4-a48d-3c64e8dc3912");

/// Cells across each ring of the lattice, and the size in metres of the finest.
/// Every ring has cells twice the size of the one inside it.
const CELLS: i32 = 256;
const FINEST: f32 = 0.25;
/// Enough rings to pass the far plane.
const RINGS: u32 = 9;
/// Cells by which a ring reaches inside the one it surrounds. Rings settle on
/// their own grids, so their edges move against each other by up to two cells.
const OVERLAP: i32 = 3;
/// Seconds after which the ripples repeat. The shader rounds their rates to fit.
const RIPPLE_PERIOD: f64 = 512.;
/// The wake hull of the player's ship. Raiders use the ids the encounter gives them.
const FLAGSHIP: u64 = u64::MAX;

/// The conditions a stage is sailed in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weather {
    /// From 0, nearly flat, to 1, a storm swell.
    pub sea: f32,
    /// Heading the waves run toward, in radians.
    pub wind: f32,
    /// From 0, broken cloud and sun, to 1, a dark overcast.
    pub overcast: f32,
}
impl Weather {
    // A quartering breeze carries the masthead pennant outboard in the title shot.
    const TITLE: Self = Self {
        sea: 0.31,
        wind: 325_f32.to_radians(),
        overcast: 0.,
    };
    /// A light breeze for the helm trial.
    pub const FAIR: Self = Self {
        sea: 0.12,
        wind: 2.2,
        overcast: 0.,
    };
}

/// The water under the current voyage.
#[derive(Resource)]
pub struct Sea {
    pub swell: Swell,
    pub wake: Wake,
    /// The enemy hulls' tracks, drawn as wakes far beyond the patch.
    tracks: Tracks,
    weather: Weather,
    seconds: f64,
    /// The wake has moved since its textures were written.
    stirred: bool,
    surface: Handle<Image>,
    foam: Handle<Image>,
    shoals: Handle<Image>,
    chart: Vec4,
}
impl FromWorld for Sea {
    fn from_world(world: &mut World) -> Self {
        let mut images = world.get_resource_or_init::<Assets<Image>>();
        let mut patch = |side: usize, format: TextureFormat, texel: &[u8]| {
            let mut image = Image::new_fill(
                Extent3d {
                    width: side as u32,
                    height: side as u32,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                texel,
                format,
                RenderAssetUsages::default(),
            );
            // Both patches wrap, so a point on the chart always finds its cell.
            image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v: ImageAddressMode::Repeat,
                ..ImageSamplerDescriptor::linear()
            });
            images.add(image)
        };
        let weather = Weather::TITLE;
        Self {
            surface: patch(SIDE, TextureFormat::Rgba16Float, &[0; 8]),
            foam: patch(FOAM_SIDE, TextureFormat::Rg8Unorm, &[0; 2]),
            shoals: patch(1, TextureFormat::R8Unorm, &[0]),
            chart: Vec4::ZERO,
            swell: Swell::new(weather.sea, weather.wind),
            wake: Wake::default(),
            tracks: Tracks::default(),
            weather,
            seconds: 0.,
            stirred: false,
        }
    }
}
impl Sea {
    pub fn weather(&self) -> Weather {
        self.weather
    }

    /// Height of the water over a point on the chart, wake included.
    pub fn height(&self, point: Vec2) -> f32 {
        self.swell.height(point) + self.wake.height(point)
    }

    /// Where a hull at a chart position rides the swell. A ship's own wake does not lift her.
    pub fn float(&self, pos: Vec2, heading: f32, hull: &Hull) -> Transform {
        let ride = self
            .swell
            .ride(pos, heading, (hull.bow + hull.stern) * 0.5, hull.radius);
        Transform::from_xyz(pos.x, ride.heave, pos.y).with_rotation(
            Quat::from_rotation_y(-heading)
                * Quat::from_rotation_x(ride.pitch)
                * Quat::from_rotation_z(ride.roll),
        )
    }

    /// The motion that carries anything drawn on a hull lying on flat water to where that hull rides.
    pub fn lift(&self, pos: Vec2, heading: f32, hull: &Hull) -> Transform {
        let riding = self.float(pos, heading, hull);
        let turn = riding.rotation * Quat::from_rotation_y(heading);
        Transform {
            translation: riding.translation - turn * Vec3::new(pos.x, 0., pos.y),
            rotation: turn,
            scale: Vec3::ONE,
        }
    }

    pub fn set_weather(&mut self, weather: Weather) {
        self.weather = weather;
        self.swell = Swell::new(weather.sea, weather.wind);
    }
}

#[derive(ShaderType, Clone, Debug, Default)]
struct Waters {
    /// Each swell train: its wave vector, amplitude and phase.
    trains: [Vec4; TRAINS],
    /// The ship, east and south, then one over the span of the wave and foam patches.
    focus: Vec4,
    /// Fractions of a half-span between which a patch fades out, then the chop.
    fade: Vec4,
    /// Size of the finest cell, and the distance from the ship at which cells start to grow.
    lattice: Vec4,
    /// Sea state, overcast, seconds, and seconds within the ripple period.
    weather: Vec4,
    /// Waterline stem position, half-beam and forward speed for the breaking bow wave.
    bow_flow: Vec4,
    chart: Vec4,
    sky: u32,
    wakes: Wakes,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct Seascape {
    #[uniform(0)]
    waters: Waters,
    #[texture(1)]
    #[sampler(2)]
    surface: Handle<Image>,
    #[texture(3)]
    #[sampler(4)]
    foam: Handle<Image>,
    #[texture(5)]
    #[sampler(6)]
    shoals: Handle<Image>,
}
impl Material for Seascape {
    fn enable_shadows() -> bool {
        false
    }
    fn enable_prepass() -> bool {
        false
    }
    /// The sea is drawn after everything solid, so it can mirror the picture of them.
    fn reads_view_transmission_texture(&self) -> bool {
        true
    }
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

pub struct SeascapePlugin;
impl Plugin for SeascapePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<Seascape>::default());
        load_internal_asset!(app, SHADER, "seascape.wesl", Shader::from_wesl);
    }
}

/// A ring of the sea lattice, counted outward from the finest.
#[derive(Component)]
pub struct Ring(u32);
#[derive(Component)]
pub struct Dome;

/// A square lattice of unit cells centred on the origin, without the cells within `hole` of the centre.
fn lattice(hole: i32) -> Mesh {
    let half = CELLS / 2;
    let line = 0..=CELLS;
    let positions: Vec<[f32; 3]> = line
        .clone()
        .flat_map(|z| {
            line.clone()
                .map(move |x| [(x - half) as f32, 0., (z - half) as f32])
        })
        .collect();
    let covered = |cell: i32| (half - hole..half + hole).contains(&cell);
    let indices = (0..CELLS)
        .flat_map(|z| (0..CELLS).map(move |x| (x, z)))
        .filter(|&(x, z)| !(covered(x) && covered(z)))
        .flat_map(|(x, z)| {
            let corner = (z * (CELLS + 1) + x) as u32;
            let below = corner + CELLS as u32 + 1;
            [corner, below, corner + 1, corner + 1, below, below + 1]
        })
        .collect();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_indices(Indices::U32(indices))
}

pub fn setup(
    mut commands: Commands,
    sea: Res<Sea>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut surfaces: ResMut<Assets<Seascape>>,
) {
    let mut surface = |sky| {
        MeshMaterial3d(surfaces.add(Seascape {
            waters: Waters { sky, ..default() },
            surface: sea.surface.clone(),
            foam: sea.foam.clone(),
            shoals: sea.shoals.clone(),
        }))
    };
    let dome = Sphere::new(crate::camera::FAR_CLIP - 340.)
        .mesh()
        .uv(64, 32)
        .with_inverted_winding()
        .expect("triangle mesh");
    commands.spawn((Dome, Mesh3d(meshes.add(dome)), surface(1), NotShadowCaster));
    let water = surface(0);
    let centre = meshes.add(lattice(0));
    let surround = meshes.add(lattice(CELLS / 4 - OVERLAP));
    for ring in 0..RINGS {
        let cell = FINEST * (1 << ring) as f32;
        commands.spawn((
            Ring(ring),
            Mesh3d(if ring == 0 {
                centre.clone()
            } else {
                surround.clone()
            }),
            water.clone(),
            Transform::from_scale(Vec3::new(cell, 1., cell)),
            NotShadowCaster,
            // The swell lifts the water out of the flat box the mesh was culled by.
            NoFrustumCulling,
        ));
    }
}

/// Keep the lattice and the sky around the ship. Each ring moves two of its own
/// cells at a time, so its vertices stay put on the water between moves.
pub fn follow(
    run: Res<Campaign>,
    helm: Res<PlayerHelm>,
    mut rings: Query<(&Ring, &mut Transform), Without<Dome>>,
    mut dome: Single<&mut Transform, With<Dome>>,
) {
    let center = if matches!(run.screen(), Screen::Drydock(_)) {
        crate::dockyard::BERTH.xz()
    } else {
        helm.position
    };
    dome.translation = center.extend(0.).xzy();
    for (ring, mut place) in &mut rings {
        let stride = 2. * FINEST * (1 << ring.0) as f32;
        let settled = (center / stride).floor() * stride;
        place.translation = Vec3::new(settled.x, 0., settled.y);
    }
}

/// Metres of water a hull draws for each metre of her half beam as she was built.
const DRAUGHT_PER_RADIUS: f32 = 0.32;

fn keel(id: u64, pos: Vec2, heading: f32, hull: &Hull, draught: f32) -> Keel {
    Keel {
        id,
        pos,
        heading,
        half_beam: hull.radius,
        bow: hull.bow,
        stern: hull.stern,
        draught,
    }
}

/// The hulls and shell splashes of this tick stir the wake patch.
pub fn churn(
    clock: Res<Time<Fixed>>,
    helm: Res<PlayerHelm>,
    run: Res<Campaign>,
    battle: Res<Encounter>,
    mut sea: ResMut<Sea>,
) {
    // The yard widens Breakwater by adding frame weight in step with the waterplane, so
    // she floats no deeper than she was built. Drawing more water in proportion to her
    // new beam would raise a bow wave over her forecastle at speed.
    let mut keels = vec![keel(
        FLAGSHIP,
        helm.position,
        helm.heading,
        &run.hull_shape(),
        DRAUGHT_PER_RADIUS * Hull::CRUISER.radius,
    )];
    if run.screen() == Screen::Sailing {
        for raider in battle.raiders() {
            let body = raider.body;
            let hull = &raider.class.profile().hull;
            keels.push(keel(
                body.id,
                body.pos,
                body.heading,
                hull,
                DRAUGHT_PER_RADIUS * hull.radius,
            ));
        }
        for effect in battle.effects() {
            let body = effect.body;
            match effect.kind {
                // A wreck draws less water as it goes, and the sea closes over it.
                EffectKind::Wreck(class) => {
                    let afloat = 1. - effect.age / effect.kind.lifetime();
                    let hull = &class.profile().hull;
                    let draught = DRAUGHT_PER_RADIUS * hull.radius * afloat;
                    keels.push(keel(body.id, body.pos, body.heading, hull, draught));
                }
                // An effect made on this tick has not aged yet.
                EffectKind::Splash if effect.age == 0. => sea.wake.splash(body.pos, 2.2),
                EffectKind::Column if effect.age == 0. => sea.wake.splash(body.pos, 4.5),
                EffectKind::Crunch if effect.age == 0. => sea.wake.splash(body.pos, 3.),
                EffectKind::Torpedoed if effect.age == 0. => sea.wake.splash(body.pos, 5.),
                EffectKind::Shatter(radius) if effect.age == 0. => {
                    sea.wake.splash(body.pos, 1.5 + 0.3 * f32::from(radius))
                }
                _ => {}
            }
        }
        let afloat = battle.raiders().iter().map(|raider| {
            let profile = raider.class.profile();
            let way = raider.motion().length();
            Steaming {
                id: raider.body.id,
                pos: raider.body.pos,
                heading: raider.body.heading,
                hull: profile.hull,
                way,
                pace: way / profile.speed(),
            }
        });
        sea.tracks.step(clock.delta_secs(), afloat);
    }
    sea.wake.step(clock.delta_secs(), helm.position, &keels);
    sea.stirred = true;
}

/// A new voyage starts on undisturbed water, in the weather of its stage.
pub fn embark(
    _: On<Embarked>,
    run: Res<Campaign>,
    battle: Res<Encounter>,
    mut sea: ResMut<Sea>,
    mut images: ResMut<Assets<Image>>,
) {
    if run.mode == Mode::Patrol {
        let (image, chart) = crate::battle_view::shoals(&battle);
        *images
            .get_mut(&sea.shoals)
            .expect("sea owns the shoal chart") = image;
        sea.chart = chart;
    } else {
        sea.chart = Vec4::ZERO;
    }
    sea.wake.clear();
    sea.tracks.clear();
    sea.stirred = true;
    let weather = match run.mode {
        Mode::Title => Weather::TITLE,
        Mode::Trial => Weather::FAIR,
        Mode::Patrol | Mode::Paused => crate::passage::Passage::of(run.stage).weather,
    };
    if weather != sea.weather {
        sea.set_weather(weather);
    }
}

/// How far from the eye shadows are cast, and the share the nearest cascade takes. Ships
/// lie on the water hundreds of metres off, and the ship's own hull is about 45 m away.
const SHADOW_CASCADES: CascadeShadowConfigBuilder = CascadeShadowConfigBuilder {
    num_cascades: 4,
    minimum_distance: 0.1,
    maximum_distance: 1100.,
    first_cascade_far_bound: 60.,
    overlap_proportion: 0.2,
};

/// Cloud takes the sun off the ships and the distance as it does off the water.
pub fn light(
    sea: Res<Sea>,
    run: Res<Campaign>,
    mut lit: Local<Option<(Weather, bool)>>,
    mut sun: Single<(
        &mut DirectionalLight,
        &mut Transform,
        &mut CascadeShadowConfig,
    )>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut fog: Single<&mut DistanceFog>,
) {
    let docked = matches!(run.screen(), Screen::Drydock(_));
    let setup = (sea.weather, docked);
    if lit.replace(setup) == Some(setup) {
        return;
    }
    let cloud = sea.weather.overcast;
    let blend = |clear: Color, dull: [f32; 3]| {
        let clear = clear.to_linear().to_f32_array();
        Color::linear_rgb(
            clear[0].lerp(dull[0], cloud),
            clear[1].lerp(dull[1], cloud),
            clear[2].lerp(dull[2], cloud),
        )
    };
    let (sun, pose, cascades) = &mut *sun;
    **cascades = SHADOW_CASCADES.build();
    if docked {
        **pose = Transform::from_xyz(-32., 44., -24.).looking_at(Vec3::ZERO, Vec3::Y);
        sun.illuminance = 18000.;
        sun.color = Color::srgb(1.0, 0.94, 0.83);
        ambient.brightness = 455.;
        fog.color = crate::camera::SKY;
        return;
    }
    **pose = Transform::from_xyz(51., 36., -84.).looking_at(Vec3::ZERO, Vec3::Y);
    sun.illuminance = 9500. * (1. - 0.7 * cloud);
    sun.color = blend(Color::srgb(1., 0.87, 0.72), [0.62, 0.70, 0.78]);
    ambient.brightness = 260. * (1. - 0.25 * cloud);
    fog.color = blend(crate::camera::SKY, [0.36, 0.40, 0.44]);
}

/// Run the swell on, and tell the shader where the water now is.
pub fn roll(
    time: Res<Time>,
    helm: Res<PlayerHelm>,
    run: Res<Campaign>,
    mut sea: ResMut<Sea>,
    mut surfaces: ResMut<Assets<Seascape>>,
    mut images: ResMut<Assets<Image>>,
) {
    let sea = &mut *sea;
    sea.swell.advance(time.delta_secs());
    sea.seconds += time.delta_secs_f64();
    if std::mem::take(&mut sea.stirred) {
        if let Some(mut image) = images.get_mut(&sea.surface)
            && let Some(texels) = image.data.as_mut()
        {
            sea.wake.write_surface(texels);
        }
        if let Some(mut image) = images.get_mut(&sea.foam)
            && let Some(texels) = image.data.as_mut()
        {
            sea.wake.write_foam(texels);
        }
    }
    let (_, fade) = sea.wake.window();
    let hull = run.hull_shape();
    let stem = helm.position + helm.forward() * (hull.bow * 0.89);
    let speed = if run.mode == Mode::Title || run.under_way() {
        helm.speed.max(0.)
    } else {
        0.
    };
    let waters = Waters {
        trains: sea
            .swell
            .trains()
            .map(|train| train.wave.extend(train.amplitude).extend(train.phase)),
        focus: helm
            .position
            .extend(1. / (SIDE as f32 * CELL))
            .extend(1. / (FOAM_SIDE as f32 * CELL)),
        fade: Vec4::new(fade.0, fade.1, CHOP, 0.),
        lattice: Vec4::new(FINEST, FINEST * CELLS as f32 / 4., 0., 0.),
        weather: Vec4::new(
            sea.weather.sea,
            sea.weather.overcast,
            sea.seconds as f32,
            (sea.seconds % RIPPLE_PERIOD) as f32,
        ),
        bow_flow: stem.extend(hull.radius).extend(speed),
        chart: sea.chart,
        sky: 0,
        // The wakes lie wherever the battle view draws the hulls that made them.
        wakes: if matches!(run.mode, Mode::Title | Mode::Trial) {
            Wakes::default()
        } else {
            sea.tracks.draw(helm.position)
        },
    };
    // Writing a material also rebinds the patch textures written above.
    for (_, surface) in surfaces.iter_mut() {
        surface.waters = Waters {
            sky: surface.waters.sky,
            ..waters.clone()
        };
    }
}
