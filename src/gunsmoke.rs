//! A shore battery giving fire: the flash, its glare and the cordite smoke it leaves.
//!
//! The harbour guns are small at the ranges they are fought at, and their icons crowd
//! together over a base, so each round they fire is made to show. The flash throws a
//! fireball down the line of fire and a glare that never draws smaller than a set
//! angle, however far off, and lights the apron around it for an instant.
//!
//! Each round also leaves a few puffs of smoke that the blast throws out ahead of the
//! muzzle, then rise a little and blow away downwind, growing and thinning as they go.
//! The puffs are drawn as light-scattering volumes: the sun lights their tops, the sky
//! their sides, and every gun flash nearby lights them from inside, brightest when the
//! flash is behind them and shining through toward the eye.
use crate::{
    camera::FollowCamera,
    seascape::Sea,
    state::{Campaign, Encounter, Mode, Screen},
};
use bevy::{
    asset::{RenderAssetUsages, load_internal_asset, uuid_handle},
    light::NotShadowCaster,
    mesh::MeshTag,
    prelude::*,
    render::render_resource::{AsBindGroup, Extent3d, ShaderType, TextureDimension, TextureFormat},
    shader::ShaderRef,
};
use shipvivor_encounter::{Battery, EffectKind};

const SHADER: Handle<Shader> = uuid_handle!("6c1f0e9a-3b57-4d2e-9a84-f0d3c27b5e61");

/// The most flashes the smoke is lit by at once. Keep in step with `gunsmoke.wesl`.
const FLASHES: usize = 16;
/// Seconds a light gun's flash and a heavy gun's burn.
const FLASH_LIFE: (f32, f32) = (0.24, 0.42);
/// Metres the fireball runs out ahead of the muzzle at its fullest, and its width.
const FIREBALL: [(f32, f32); 2] = [(9., 3.4), (16., 6.)];
/// Metres across the glare at its brightest, and the least angle in radians it ever
/// draws at, so that a gun far off still shows a spark of some size.
const GLARE: [(f32, f32); 2] = [(6., 0.035), (11., 0.05)];
/// Metres within which the glare gives way to the fireball, which shows well enough
/// close to, and past which it shows at full strength.
const GLARE_FADE: (f32, f32) = (90., 450.);
/// Light thrown on the apron, in lumens at the flash, and the metres it reaches.
const FLARE: [(f32, f32); 2] = [(2.4e7, 90.), (6e7, 160.)];
/// Metres nearer the eye than it stands that a flash is drawn as being, so that it
/// shows over its own smoke and the rest of the cloud about the gun: the glare, then
/// the outer tongue, the inner and the core.
const OVER_SMOKE: f32 = 40.;
/// Gun flashes lighting the apron at once. The rest show only their fireball.
const LIGHTS: usize = 6;
/// Metres past which a flash lights nothing around it.
const LIT_RANGE: f32 = 1400.;

/// Seconds a puff hangs before it has thinned away, and the metres across it starts.
const PUFF_LIFE: [f32; 2] = [13., 20.];
const PUFF_SIZE: [f32; 2] = [2.2, 3.6];
/// The most puffs aloft. The oldest give way first.
const PUFFS: usize = 220;
/// Metres per second the blast throws a puff ahead, and the share of that left after
/// each second as the air takes it.
const THROW: (f32, f32) = (16., 0.18);
/// Metres per second the warm smoke first rises.
const RISE: f32 = 0.9;
/// Seconds the smoke takes to pick up the wind.
const CATCH: f32 = 1.6;

/// The smoke from the shore guns, and the flashes still lighting it.
#[derive(Resource, Default)]
pub struct Gunsmoke {
    flashes: Vec<Flash>,
    puffs: Vec<Puff>,
}

/// A gun's flash, at its muzzle and along its line of fire.
struct Flash {
    at: Vec3,
    aim: Vec3,
    age: f32,
    /// One of a heavy battery's long guns, rather than a light mount.
    heavy: bool,
    /// A shore gun, which is drawn with a fireball and glare. Others only light smoke.
    shore: bool,
}
impl Flash {
    fn life(&self) -> f32 {
        if self.heavy {
            FLASH_LIFE.1
        } else {
            FLASH_LIFE.0
        }
    }
    /// From 1 as it fires to 0 as it dies: the fireball swells at once and fades fast.
    fn glow(&self) -> f32 {
        let t = (self.age / self.life()).clamp(0., 1.);
        (1. - t).powf(1.5) * (0.5 + self.age / 0.03).min(1.)
    }
}

struct Puff {
    at: Vec3,
    /// Metres per second the blast and the warm gas give it, apart from the wind.
    thrown: Vec3,
    drift: Vec3,
    age: f32,
    life: f32,
    size: f32,
    seed: u8,
    /// How thick it starts, from 0 to 1.
    thick: f32,
}
impl Puff {
    /// Metres from its centre to its edge.
    fn radius(&self) -> f32 {
        self.size * (1. + 0.85 * self.age.sqrt())
    }
    /// How thick it is now: it thins as it spreads, and fades out at the end.
    fn density(&self) -> f32 {
        let t = self.age / self.life;
        let spread = (self.size / self.radius()).powf(0.7);
        let fade = 1. - ((t - 0.5) / 0.5).clamp(0., 1.).powi(2);
        // It thickens as the flash dies, so the fire shows through the first of it.
        let gather = ((self.age - 0.04) / 0.2).clamp(0., 1.);
        self.thick * spread * fade * gather
    }
    /// The look of the puff, packed for the shader: thickness, the seed of its shape,
    /// and how fresh it is, which tints new smoke the brown of burnt cordite.
    fn tag(&self) -> u32 {
        let thick = (self.density() * 255.).clamp(0., 255.) as u32;
        let fresh = ((1. - self.age / 4.).clamp(0., 1.) * 255.) as u32;
        thick | (u32::from(self.seed) << 8) | (fresh << 16)
    }
}

/// What the smoke shader is told each frame: the flashes lighting it.
#[derive(ShaderType, Clone, Debug, Default)]
struct SmokeLight {
    /// Each flash: where it is, and how bright.
    flashes: [Vec4; FLASHES],
    /// The flash's colour, and how many flashes are in use.
    glow: Vec4,
    /// Seconds, for the slow churn of the smoke.
    clock: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, Default)]
pub struct Smoke {
    #[uniform(0)]
    light: SmokeLight,
}
impl Material for Smoke {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
}

#[derive(Resource)]
struct Kit {
    /// A cube about each puff, in which the shader finds the sphere of smoke.
    cube: Handle<Mesh>,
    quad: Handle<Mesh>,
    smoke: Handle<Smoke>,
    glare: Handle<Image>,
    halo: Handle<Image>,
    flame: Handle<Image>,
}

/// A puff of smoke drawn in the world, by its place in the list.
#[derive(Component)]
struct PuffModel(usize);
/// A shore gun's flash drawn in the world, by its place among those now burning. Its
/// fireball, glare and light are its children, and go with it.
#[derive(Component)]
struct FlashModel {
    index: usize,
    fire: [(Entity, Handle<StandardMaterial>); 3],
    glare: (Entity, Handle<StandardMaterial>),
    light: Entity,
}

pub struct GunsmokePlugin;
impl Plugin for GunsmokePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<Smoke>::default());
        load_internal_asset!(app, SHADER, "gunsmoke.wesl", Shader::from_wesl);
        app.init_resource::<Gunsmoke>()
            .add_systems(Startup, setup)
            .add_systems(
                FixedUpdate,
                gather.after(crate::advance_ship).run_if(crate::under_way),
            )
            .add_systems(Update, draw.after(crate::camera::follow));
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut smoke: ResMut<Assets<Smoke>>,
    mut images: ResMut<Assets<Image>>,
) {
    commands.insert_resource(Kit {
        cube: meshes.add(Cuboid::new(2., 2., 2.)),
        quad: meshes.add(Rectangle::new(1., 1.)),
        smoke: smoke.add(Smoke::default()),
        glare: images.add(glare_image()),
        halo: images.add(halo_image()),
        flame: images.add(flame_image()),
    });
}

/// A broad soft disc, solid at the heart, that is laid over the scene to show a far
/// flash as a ball of colour, with faint rays out from it.
fn halo_image() -> Image {
    const SIDE: usize = 64;
    let mut data = Vec::with_capacity(SIDE * SIDE * 4);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let u = (x as f32 + 0.5) / SIDE as f32 * 2. - 1.;
            let v = (y as f32 + 0.5) / SIDE as f32 * 2. - 1.;
            let r = (u * u + v * v).sqrt();
            let disc = (1. - r / 0.62).clamp(0., 1.).powf(0.7);
            let rays = (-(u.abs() * v.abs()).sqrt() * 18.).exp() * (1. - r).max(0.) * 0.5;
            let a = (disc + rays).min(1.);
            data.extend([255, 255, 255, (a * 255.) as u8]);
        }
    }
    Image::new(
        Extent3d {
            width: SIDE as u32,
            height: SIDE as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// A tongue of flame lying along the image from the muzzle at the left: narrow where
/// it leaves the barrel, swelling a third of the way out and burning away to the tip,
/// brightest along its middle.
fn flame_image() -> Image {
    const LONG: usize = 128;
    const WIDE: usize = 64;
    let mut data = Vec::with_capacity(LONG * WIDE * 4);
    for y in 0..WIDE {
        for x in 0..LONG {
            let u = (x as f32 + 0.5) / LONG as f32;
            let v = (y as f32 + 0.5) / WIDE as f32 * 2. - 1.;
            let width = 0.18 + 0.82 * (std::f32::consts::PI * u.powf(0.7)).sin().max(0.).powf(0.8);
            let across = (-(v / width.max(0.05)).powi(2) * 2.6).exp();
            let along = (u / 0.05).min(1.) * (1. - u).max(0.).powf(0.7);
            let a = (across * along * 1.25).min(1.);
            data.extend([255, 255, 255, (a * 255.) as u8]);
        }
    }
    Image::new(
        Extent3d {
            width: LONG as u32,
            height: WIDE as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// A soft round glow with a faint four-pointed star through it, white at the heart.
fn glare_image() -> Image {
    const SIDE: usize = 128;
    let mut data = Vec::with_capacity(SIDE * SIDE * 4);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let u = (x as f32 + 0.5) / SIDE as f32 * 2. - 1.;
            let v = (y as f32 + 0.5) / SIDE as f32 * 2. - 1.;
            let r = (u * u + v * v).sqrt();
            let core = (-r * r * 40.).exp();
            let halo = (-r * 4.6).exp() * 0.5;
            let star = (-(u.abs() * v.abs()).sqrt() * 26.).exp() * (1. - r).max(0.) * 0.55;
            let edge = (1. - r).clamp(0., 1.).powf(0.6);
            let a = ((core + halo + star) * edge).min(1.);
            data.extend([255, 255, 255, (a * 255.) as u8]);
        }
    }
    Image::new(
        Extent3d {
            width: SIDE as u32,
            height: SIDE as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// The breeze the smoke drifts on, in metres a second.
fn breeze(sea: &Sea) -> Vec3 {
    let weather = sea.weather();
    let along = shipvivor_compass::direction(weather.wind) * (1.6 + 4. * weather.sea);
    Vec3::new(along.x, 0., along.y)
}

/// Take up each gun that fired this tick, and move the smoke and flashes on.
pub(crate) fn gather(
    clock: Res<Time<Fixed>>,
    battle: Res<Encounter>,
    sea: Res<Sea>,
    mut smoke: ResMut<Gunsmoke>,
) {
    let dt = clock.delta_secs();
    let wind = breeze(&sea);
    let smoke = &mut *smoke;
    for flash in &mut smoke.flashes {
        flash.age += dt;
    }
    smoke.flashes.retain(|flash| flash.age < flash.life());
    for puff in &mut smoke.puffs {
        puff.age += dt;
        let catch = 1. - (-dt / CATCH).exp();
        puff.drift += (wind - puff.drift) * catch;
        puff.thrown *= THROW.1.powf(dt);
        puff.at += (puff.thrown + puff.drift) * dt;
    }
    smoke.puffs.retain(|puff| puff.age < puff.life);
    for fx in battle.effects().iter().filter(|fx| fx.age == 0.) {
        let EffectKind::Muzzle(battery) = fx.kind else {
            continue;
        };
        let heavy = battery == Battery::Heavy;
        let shore = matches!(battery, Battery::Shore | Battery::Heavy);
        let forward = fx.body.forward();
        let aim = Vec3::new(forward.x, 0., forward.y);
        let at = Vec3::new(fx.body.pos.x, fx.height, fx.body.pos.y);
        smoke.flashes.push(Flash {
            at,
            aim,
            age: 0.,
            heavy,
            shore,
        });
        if shore {
            puff_out(&mut smoke.puffs, fx.body.id, at, aim, heavy);
        }
    }
    if smoke.flashes.len() > 4 * FLASHES {
        let extra = smoke.flashes.len() - 4 * FLASHES;
        smoke.flashes.drain(..extra);
    }
    if smoke.puffs.len() > PUFFS {
        let extra = smoke.puffs.len() - PUFFS;
        smoke.puffs.drain(..extra);
    }
}

/// The smoke of one round: a thick ball at the muzzle and two the blast throws out
/// ahead, the farthest the thinnest. A heavy gun's is larger and lasts longer.
fn puff_out(puffs: &mut Vec<Puff>, id: u64, at: Vec3, aim: Vec3, heavy: bool) {
    let kind = usize::from(heavy);
    let across = Vec3::Y.cross(aim);
    for k in 0..3_u8 {
        let seed = (id.wrapping_mul(0x9E37_79B9).wrapping_add(u64::from(k) * 77) >> 7) as u8;
        let wobble = (f32::from(seed) / 255. - 0.5) * 2.;
        let reach = f32::from(k);
        puffs.push(Puff {
            at: at + aim * (reach * 0.8) + across * wobble * 0.6,
            thrown: aim * THROW.0 * (0.25 + 0.55 * reach) * (1. + 0.6 * kind as f32)
                + across * wobble * 2.
                + Vec3::Y * RISE * (1. + 0.3 * reach),
            drift: Vec3::ZERO,
            age: 0.,
            life: PUFF_LIFE[kind] * (1. - 0.15 * reach),
            size: PUFF_SIZE[kind] * (1. - 0.18 * reach),
            seed,
            thick: 1. - 0.2 * reach,
        });
    }
}

/// The battle is drawn on patrol. In the yard and on the chart the smoke is let go.
fn at_sea(run: &Campaign) -> bool {
    !matches!(run.mode, Mode::Title | Mode::Trial)
        && !matches!(run.screen(), Screen::Drydock(_) | Screen::Chart)
}

/// Draw the smoke and the flashes where they are, and tell the shader what lights it.
#[allow(clippy::too_many_arguments)]
fn draw(
    mut commands: Commands,
    run: Res<Campaign>,
    time: Res<Time>,
    kit: Res<Kit>,
    mut smoke: ResMut<Gunsmoke>,
    camera: Single<&Transform, With<FollowCamera>>,
    mut puffs: Query<(Entity, &PuffModel, &mut Transform, &mut MeshTag), Without<FollowCamera>>,
    flashes: Query<(Entity, &FlashModel)>,
    mut parts: Query<
        &mut Transform,
        (
            Without<FlashModel>,
            Without<PuffModel>,
            Without<FollowCamera>,
        ),
    >,
    mut lights: Query<&mut PointLight>,
    mut paint: ResMut<Assets<StandardMaterial>>,
    mut shaders: ResMut<Assets<Smoke>>,
) {
    if !at_sea(&run) {
        smoke.flashes.clear();
        smoke.puffs.clear();
    }
    let eye = camera.translation;
    // Each puff keeps its entity while it lasts, by its place in the list.
    let mut drawn = vec![false; smoke.puffs.len()];
    for (entity, model, mut pose, mut tag) in &mut puffs {
        let Some(puff) = smoke.puffs.get(model.0) else {
            commands.entity(entity).despawn();
            continue;
        };
        drawn[model.0] = true;
        *pose = Transform::from_translation(puff.at).with_scale(Vec3::splat(puff.radius()));
        tag.set_if_neq(MeshTag::new(puff.tag()));
    }
    for (i, puff) in smoke.puffs.iter().enumerate().filter(|(i, _)| !drawn[*i]) {
        commands.spawn((
            PuffModel(i),
            Mesh3d(kit.cube.clone()),
            MeshMaterial3d(kit.smoke.clone()),
            Transform::from_translation(puff.at).with_scale(Vec3::splat(puff.radius())),
            MeshTag::new(puff.tag()),
            NotShadowCaster,
        ));
    }

    // The shore guns' flashes, each with its fireball, its glare and its light.
    let shown: Vec<usize> = (smoke.flashes.iter().enumerate())
        .filter(|(_, flash)| flash.shore)
        .map(|(i, _)| i)
        .collect();
    let lit: Vec<usize> = shown
        .iter()
        .rev()
        .filter(|&&i| smoke.flashes[i].at.distance(eye) < LIT_RANGE)
        .take(LIGHTS)
        .copied()
        .collect();
    let mut have = vec![false; shown.len()];
    for (entity, model) in &flashes {
        let Some(&i) = shown.get(model.index) else {
            commands.entity(entity).despawn();
            continue;
        };
        have[model.index] = true;
        let flash = &smoke.flashes[i];
        pose_flash(flash, model, eye, camera.rotation, &mut parts, &mut paint);
        if let Ok(mut light) = lights.get_mut(model.light) {
            let kind = usize::from(flash.heavy);
            let on = lit.contains(&i);
            light.intensity = if on { FLARE[kind].0 * flash.glow() } else { 0. };
            light.range = FLARE[kind].1;
        }
    }
    for (slot, &i) in shown.iter().enumerate().filter(|(slot, _)| !have[*slot]) {
        spawn_flash(&mut commands, &kit, &mut paint, &smoke.flashes[i], slot);
    }

    // The shader is told of the brightest flashes, nearest the eye first among equals.
    let mut bright: Vec<(f32, Vec4)> = (smoke.flashes.iter())
        .map(|flash| {
            let power = flash.glow() * if flash.heavy { 2.4 } else { 1. };
            let rank = power / (1. + flash.at.distance(eye) / 600.);
            (rank, flash.at.extend(power))
        })
        .filter(|(rank, _)| *rank > 0.001)
        .collect();
    bright.sort_by(|a, b| b.0.total_cmp(&a.0));
    bright.truncate(FLASHES);
    if let Some(mut material) = shaders.get_mut(&kit.smoke) {
        let light = &mut material.light;
        light.flashes = [Vec4::ZERO; FLASHES];
        for (slot, (_, flash)) in light.flashes.iter_mut().zip(&bright) {
            *slot = *flash;
        }
        light.glow = Vec4::new(1., 0.45, 0.13, bright.len() as f32);
        light.clock = Vec4::new(time.elapsed_secs_wrapped(), 0., 0., 0.);
    }
}

/// A flash's fireball, glare and light, hidden until it is first posed.
fn spawn_flash(
    commands: &mut Commands,
    kit: &Kit,
    paint: &mut Assets<StandardMaterial>,
    flash: &Flash,
    index: usize,
) {
    let kind = usize::from(flash.heavy);
    let root = commands
        .spawn((Transform::default(), Visibility::default()))
        .id();
    // Two tongues of flame along the line of fire, and a round core over the muzzle.
    // The outer tongue is laid over what is behind it, so it reads as orange even
    // against pale rock. The inner one and the core shine on top.
    let fire = [
        (&kit.flame, AlphaMode::Blend, 1.),
        (&kit.flame, AlphaMode::Add, 2.),
        (&kit.glare, AlphaMode::Add, 3.),
    ]
    .map(|(image, alpha_mode, layer)| {
        let material = paint.add(StandardMaterial {
            base_color_texture: Some(image.clone()),
            unlit: true,
            alpha_mode,
            depth_bias: OVER_SMOKE + layer,
            fog_enabled: false,
            double_sided: true,
            cull_mode: None,
            ..default()
        });
        let entity = commands
            .spawn((
                Mesh3d(kit.quad.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(flash.at).with_scale(Vec3::splat(0.001)),
                NotShadowCaster,
                ChildOf(root),
            ))
            .id();
        (entity, material)
    });
    let glare_paint = paint.add(StandardMaterial {
        base_color_texture: Some(kit.halo.clone()),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        depth_bias: OVER_SMOKE,
        fog_enabled: false,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let glare = commands
        .spawn((
            Mesh3d(kit.quad.clone()),
            MeshMaterial3d(glare_paint.clone()),
            Transform::from_translation(flash.at).with_scale(Vec3::splat(0.001)),
            NotShadowCaster,
            ChildOf(root),
        ))
        .id();
    let light = commands
        .spawn((
            PointLight {
                color: Color::srgb(1., 0.72, 0.42),
                intensity: 0.,
                range: FLARE[kind].1,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(flash.at + Vec3::Y * 1.5),
            ChildOf(root),
        ))
        .id();
    commands.entity(root).insert(FlashModel {
        index,
        fire,
        glare: (glare, glare_paint),
        light,
    });
}

/// The fireball rolls out ahead of the muzzle, white at the heart and orange at the
/// edge. Its tongues lie along the line of fire, turned to the eye about it, and the
/// round core grows as the eye looks down the barrel and the tongues go edge on. The
/// glare turns to the eye too, never drawn smaller than its least angle.
fn pose_flash(
    flash: &Flash,
    model: &FlashModel,
    eye: Vec3,
    facing: Quat,
    parts: &mut Query<
        &mut Transform,
        (
            Without<FlashModel>,
            Without<PuffModel>,
            Without<FollowCamera>,
        ),
    >,
    paint: &mut Assets<StandardMaterial>,
) {
    let kind = usize::from(flash.heavy);
    let glow = flash.glow();
    let t = (flash.age / flash.life()).clamp(0., 1.);
    let (length, width) = FIREBALL[kind];
    // Swells to full in a few hundredths of a second, then rolls on and dies down.
    let swell = (flash.age / 0.035).min(1.).sqrt() * (1. - 0.35 * t);
    let toward = (eye - flash.at).normalize_or_zero();
    // How nearly the eye looks down the barrel.
    let head_on = toward.dot(flash.aim).abs();
    // The tongues' faces turn to the eye about the line of fire.
    let face = (toward - flash.aim * toward.dot(flash.aim))
        .try_normalize()
        .unwrap_or(Vec3::Y);
    let lay = Quat::from_mat3(&Mat3::from_cols(flash.aim, face.cross(flash.aim), face));
    // Orange outer tongue, white-yellow inner one, and the white-hot core.
    let layers = [
        (1., 1., Vec3::new(6., 1.5, 0.25)),
        (0.62, 0.5, Vec3::new(4.6, 3.6, 1.8)),
    ];
    let side_on = 1. - head_on * 0.6;
    for (layer, ((entity, material), (long, wide, colour))) in
        model.fire.iter().zip(layers).enumerate()
    {
        let reach = length * long * swell * (0.8 + 0.4 * t);
        if let Ok(mut pose) = parts.get_mut(*entity) {
            // Set toward the eye, so the fire is drawn over the smoke it makes, and the
            // inner tongue over the outer.
            let nearer = 2. + width * 0.6 + layer as f32;
            pose.translation = flash.at + flash.aim * reach * 0.5 + toward * nearer;
            pose.rotation = lay;
            pose.scale = Vec3::new(reach, width * wide * swell, 1.).max(Vec3::splat(0.001));
        }
        if let Some(mut paint) = paint.get_mut(material) {
            paint.base_color = if layer == 0 {
                Color::linear_rgba(colour.x, colour.y, colour.z, glow.sqrt() * side_on)
            } else {
                let c = colour * glow * side_on;
                Color::linear_rgb(c.x, c.y, c.z)
            };
        }
    }
    let (across, angle) = GLARE[kind];
    let distance = flash.at.distance(eye);
    let span = across.max(distance * angle) * (0.35 + 0.65 * glow.sqrt());
    // Drawn a little toward the eye, so the gunhouse and apron do not cut it in half,
    // and the core nearer still, over it.
    let lift = (span * 0.5).min(30.);
    let (core, core_paint) = &model.fire[2];
    if let Ok(mut pose) = parts.get_mut(*core) {
        let size = (width * swell * (1.1 + 1.4 * head_on)).max(span * 0.3);
        pose.translation = flash.at + flash.aim * length * 0.15 * swell + toward * (lift + 2.);
        pose.rotation = facing;
        pose.scale = Vec3::splat(size.max(0.001));
    }
    if let Some(mut paint) = paint.get_mut(core_paint) {
        let c = Vec3::new(5., 4.2, 3.) * glow;
        paint.base_color = Color::linear_rgb(c.x, c.y, c.z);
    }
    if let Ok(mut pose) = parts.get_mut(model.glare.0) {
        pose.translation = flash.at + flash.aim * length * 0.25 + toward * lift;
        pose.rotation = facing;
        pose.scale = Vec3::splat(span.max(0.001));
    }
    if let Some(mut paint) = paint.get_mut(&model.glare.1) {
        let far = ((distance - GLARE_FADE.0) / (GLARE_FADE.1 - GLARE_FADE.0)).clamp(0., 1.);
        let alpha = glow.sqrt() * (0.15 + 0.85 * far);
        // Bright enough to hold its colour through the tone mapping, red enough to
        // stay orange rather than bleach to cream.
        paint.base_color = Color::linear_rgba(8., 1.6, 0.25, alpha);
    }
    // The light stands just behind the muzzle, over the gunhouse and its apron.
    if let Ok(mut pose) = parts.get_mut(model.light) {
        pose.translation = flash.at + Vec3::Y * 1.5 - flash.aim * 2.;
    }
}
