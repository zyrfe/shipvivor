//! Breakwater's catapult, floatplane and missile launchers: the gear on deck, the plane
//! in the air and the missiles with their smoke.
use crate::battle_view::spotter_model;
use crate::scene::Facets;
use crate::state::{Campaign, Encounter, Mode, Screen};
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use shipvivor_campaign::DockModule;
use std::collections::HashMap;

/// The floatplane is drawn at this share of the harbours' spotter, to suit the ship.
const PLANE_SCALE: f32 = 0.34;
/// In the air it is drawn a little larger, so it can be followed at a distance.
const AIRBORNE_SCALE: f32 = 0.5;
/// Height of the catapult's trolley above the waterline, where the plane's floats rest.
const TROLLEY: f32 = 2.36;
/// Seconds a puff of missile smoke lasts, and between puffs.
const PUFF_LIFE: f32 = 1.1;
const PUFF_EVERY: f32 = 0.035;
/// Bombs that can be falling at once.
const BOMBS: usize = 4;

/// The plane on the catapult. It is hidden while the plane is flying.
#[derive(Component)]
pub struct CatapultPlane;
/// The trained part of a missile launcher, by its order from the bow.
#[derive(Component)]
pub struct LauncherMount(usize);
#[derive(Component)]
pub struct Airborne;
#[derive(Component)]
pub struct Spinner;
#[derive(Component)]
pub struct MissileModel(u64);
#[derive(Component)]
pub struct BombModel(usize);
#[derive(Component)]
pub struct Puff {
    age: f32,
    size: f32,
}

/// Meshes and paint for the deck gear, made once when the ship is built.
pub(crate) struct Parts {
    cube: Handle<Mesh>,
    drum: Handle<Mesh>,
    plane: Handle<Mesh>,
    propeller: Handle<Mesh>,
    steel: Handle<StandardMaterial>,
    iron: Handle<StandardMaterial>,
    pale: Handle<StandardMaterial>,
    dark: Handle<StandardMaterial>,
    airframe: Handle<StandardMaterial>,
}
impl Parts {
    pub(crate) fn new(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        let mut paint = |r, g, b| materials.add(Color::srgb_u8(r, g, b));
        Self {
            cube: meshes.add(Cuboid::default()),
            drum: meshes.add(Cylinder::new(0.5, 1.0).mesh().resolution(12)),
            plane: meshes.add(spotter_model::plane()),
            propeller: meshes.add(spotter_model::propeller()),
            steel: paint(108, 134, 142),
            iron: paint(47, 65, 73),
            pale: paint(196, 204, 192),
            dark: paint(24, 30, 34),
            airframe: materials.add(StandardMaterial::default()),
        }
    }
}

/// Fit the gear of one yard section to `gear`, which is placed on the module but not
/// stretched across a widened beam.
pub(crate) fn equip(
    commands: &mut Commands,
    gear: Entity,
    kind: DockModule,
    instance: usize,
    parts: &Parts,
) {
    let part = |commands: &mut Commands,
                parent: Entity,
                mesh: &Handle<Mesh>,
                paint: &Handle<StandardMaterial>,
                pose: Transform| {
        commands
            .spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(paint.clone()),
                pose,
                ChildOf(parent),
            ))
            .id()
    };
    let at = |x, y, z, sx, sy, sz| Transform::from_xyz(x, y, z).with_scale(Vec3::new(sx, sy, sz));
    match kind {
        DockModule::Floatplane => {
            // A turntable, the girder of the catapult along the centreline and its rails.
            part(
                commands,
                gear,
                &parts.drum,
                &parts.iron,
                at(0., 1.68, 0.7, 1.6, 0.26, 1.6),
            );
            part(
                commands,
                gear,
                &parts.cube,
                &parts.steel,
                at(0., 1.98, -0.1, 0.46, 0.3, 4.7),
            );
            for x in [-0.16, 0.16] {
                part(
                    commands,
                    gear,
                    &parts.cube,
                    &parts.pale,
                    at(x, 2.15, -0.1, 0.05, 0.05, 4.7),
                );
            }
            part(
                commands,
                gear,
                &parts.cube,
                &parts.iron,
                at(0., 2.24, 0.25, 0.62, 0.16, 0.95),
            );
            // The crane that hoists the plane back aboard, on the starboard quarter.
            part(
                commands,
                gear,
                &parts.drum,
                &parts.steel,
                at(1.85, 2.4, 1.7, 0.22, 1.7, 0.22),
            );
            let (root, tip) = (Vec3::new(1.85, 3.2, 1.7), Vec3::new(0.35, 3.55, 0.8));
            part(
                commands,
                gear,
                &parts.cube,
                &parts.steel,
                Transform::from_translation(root.midpoint(tip))
                    .looking_to(tip - root, Vec3::Y)
                    .with_scale(Vec3::new(0.13, 0.13, root.distance(tip))),
            );
            part(
                commands,
                gear,
                &parts.cube,
                &parts.dark,
                at(0.35, 3.15, 0.8, 0.03, 0.8, 0.03),
            );
            let plane = part(
                commands,
                gear,
                &parts.plane,
                &parts.airframe,
                Transform::from_xyz(0., TROLLEY + 0.2, 0.15).with_scale(Vec3::splat(PLANE_SCALE)),
            );
            commands.entity(plane).insert(CatapultPlane);
            commands.spawn((
                Mesh3d(parts.propeller.clone()),
                MeshMaterial3d(parts.airframe.clone()),
                Transform::from_translation(spotter_model::HUB),
                ChildOf(plane),
            ));
        }
        DockModule::Missiles => {
            part(
                commands,
                gear,
                &parts.drum,
                &parts.iron,
                at(0., 1.75, 0.3, 1.4, 0.42, 1.4),
            );
            let mount = commands
                .spawn((
                    LauncherMount(instance),
                    Transform::from_xyz(0., 1.96, 0.3),
                    Visibility::default(),
                    ChildOf(gear),
                ))
                .id();
            part(
                commands,
                mount,
                &parts.cube,
                &parts.steel,
                at(0., 0.3, 0., 1.35, 0.45, 0.7),
            );
            // The box of cells, raised toward the bow.
            let cradle = commands
                .spawn((
                    Transform::from_xyz(0., 0.62, 0.).with_rotation(Quat::from_rotation_x(0.3)),
                    Visibility::default(),
                    ChildOf(mount),
                ))
                .id();
            part(
                commands,
                cradle,
                &parts.cube,
                &parts.pale,
                at(0., 0.42, 0., 1.2, 0.84, 2.9),
            );
            for x in [-0.29, 0.29] {
                part(
                    commands,
                    cradle,
                    &parts.cube,
                    &parts.dark,
                    at(x, 0.42, -1.46, 0.44, 0.58, 0.04),
                );
            }
            for z in [-0.9, 0.9] {
                part(
                    commands,
                    cradle,
                    &parts.cube,
                    &parts.iron,
                    at(0., 0.42, z, 1.26, 0.9, 0.12),
                );
            }
        }
        _ => {}
    }
}

#[derive(Resource)]
pub struct Kit {
    plane: Handle<Mesh>,
    propeller: Handle<Mesh>,
    airframe: Handle<StandardMaterial>,
    missile: Handle<Mesh>,
    flame: Handle<Mesh>,
    flame_paint: Handle<StandardMaterial>,
    bomb: Handle<Mesh>,
    puff: Handle<Mesh>,
    smoke: Handle<StandardMaterial>,
}

fn missile_mesh() -> Mesh {
    let mut m = Facets::default();
    let body = Color::srgb_u8(226, 228, 214);
    let band = Color::srgb_u8(196, 62, 48);
    let ink = Color::srgb_u8(46, 58, 62);
    let r = 0.16;
    m.tube(Vec3::Z * 0.2, Vec3::Z * 1.0, r, r, body);
    m.tube(Vec3::Z * 0.2, Vec3::NEG_Z * 0.6, r, r, body);
    m.tube(Vec3::NEG_Z * 0.6, Vec3::NEG_Z * 0.8, r, r * 0.95, band);
    m.tube(Vec3::NEG_Z * 0.8, Vec3::NEG_Z * 1.25, r * 0.95, 0.01, band);
    for i in 0..4 {
        let turn = Quat::from_rotation_z(i as f32 * std::f32::consts::FRAC_PI_2 + 0.785);
        let fin = [
            Vec3::new(0., r, 0.45),
            Vec3::new(0., r + 0.32, 0.85),
            Vec3::new(0., r + 0.32, 1.02),
            Vec3::new(0., r, 1.0),
        ]
        .map(|p| turn * p);
        m.quad(fin, ink);
        let mut back = fin;
        back.reverse();
        m.quad(back, ink);
    }
    m.finish()
}

fn bomb_mesh() -> Mesh {
    let mut m = Facets::default();
    let iron = Color::srgb_u8(40, 46, 48);
    m.tube(Vec3::ZERO, Vec3::NEG_Z * 0.35, 0.14, 0.02, iron);
    m.tube(Vec3::ZERO, Vec3::Z * 0.4, 0.14, 0.08, iron);
    m.finish()
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let kit = Kit {
        plane: meshes.add(spotter_model::plane()),
        propeller: meshes.add(spotter_model::propeller()),
        airframe: materials.add(StandardMaterial::default()),
        missile: meshes.add(missile_mesh()),
        flame: meshes.add(Sphere::new(0.5).mesh().ico(1).expect("a low sphere")),
        flame_paint: materials.add(StandardMaterial {
            base_color: Color::srgb(1., 0.72, 0.32),
            emissive: LinearRgba::rgb(6., 3.2, 0.9),
            unlit: true,
            ..default()
        }),
        bomb: meshes.add(bomb_mesh()),
        puff: meshes.add(Sphere::new(0.5).mesh().ico(1).expect("a low sphere")),
        smoke: materials.add(StandardMaterial {
            base_color: Color::srgba_u8(214, 214, 208, 120),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
    };
    let plane = commands
        .spawn((
            Airborne,
            Mesh3d(kit.plane.clone()),
            MeshMaterial3d(kit.airframe.clone()),
            Transform::default(),
            Visibility::Hidden,
        ))
        .id();
    commands.spawn((
        Spinner,
        Mesh3d(kit.propeller.clone()),
        MeshMaterial3d(kit.airframe.clone()),
        Transform::from_translation(spotter_model::HUB),
        ChildOf(plane),
    ));
    for index in 0..BOMBS {
        commands.spawn((
            BombModel(index),
            Mesh3d(kit.bomb.clone()),
            MeshMaterial3d(kit.airframe.clone()),
            Transform::default(),
            Visibility::Hidden,
        ));
    }
    commands.insert_resource(kit);
}

/// The battle is drawn on patrol. In the yard the ship lies far from it.
fn at_sea(run: &Campaign) -> bool {
    !matches!(run.mode, Mode::Title | Mode::Trial) && !matches!(run.screen(), Screen::Drydock(_))
}

/// Keep the plane on its catapult while it is aboard, and lay each launcher.
pub fn stow(
    run: Res<Campaign>,
    battle: Res<Encounter>,
    mut planes: Query<&mut Visibility, With<CatapultPlane>>,
    mut mounts: Query<(&LauncherMount, &mut Transform)>,
) {
    let sea = at_sea(&run);
    let aloft = sea && battle.floatplane().is_some_and(|plane| !plane.aboard());
    for mut shown in &mut planes {
        shown.set_if_neq(if aloft {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
    }
    for (mount, mut pose) in &mut mounts {
        let train = if sea {
            battle.launchers().get(mount.0).map_or(0., |l| l.train)
        } else {
            0.
        };
        pose.rotation = Quat::from_rotation_y(-train);
    }
}

/// The floatplane in the air, its propeller turning.
pub fn fly(
    time: Res<Time>,
    run: Res<Campaign>,
    battle: Res<Encounter>,
    mut plane: Single<(&mut Transform, &mut Visibility), With<Airborne>>,
    mut spinners: Query<&mut Transform, (With<Spinner>, Without<Airborne>)>,
) {
    let sea = at_sea(&run);
    let (pose, shown) = &mut *plane;
    match battle.floatplane().filter(|p| sea && !p.aboard()) {
        Some(flying) => {
            **pose = Transform::from_translation(flying.position_3d())
                .with_rotation(
                    Quat::from_rotation_y(-flying.heading) * Quat::from_rotation_z(flying.bank),
                )
                .with_scale(Vec3::splat(AIRBORNE_SCALE));
            shown.set_if_neq(Visibility::Inherited);
        }
        None => {
            shown.set_if_neq(Visibility::Hidden);
        }
    }
    for mut spin in &mut spinners {
        spin.rotation = Quat::from_rotation_z(time.elapsed_secs() * 38.);
    }
}

/// Missiles in flight, each trailing smoke from its motor.
pub fn trail(
    mut commands: Commands,
    time: Res<Time>,
    kit: Res<Kit>,
    run: Res<Campaign>,
    battle: Res<Encounter>,
    mut missiles: Query<(Entity, &MissileModel, &mut Transform)>,
    mut puffs: Query<(Entity, &mut Puff, &mut Transform), Without<MissileModel>>,
    mut trailed: Local<HashMap<u64, f32>>,
) {
    let dt = time.delta_secs();
    let sea = at_sea(&run);
    let flying: HashMap<u64, &shipvivor_encounter::Missile> = if sea {
        battle.missiles().iter().map(|m| (m.body.id, m)).collect()
    } else {
        HashMap::new()
    };
    let tail = |m: &shipvivor_encounter::Missile| {
        let back = shipvivor_compass::direction(m.body.heading) * -1.1;
        Vec3::new(m.body.pos.x + back.x, m.altitude, m.body.pos.y + back.y)
    };
    let pose_of = |m: &shipvivor_encounter::Missile| {
        Transform::from_xyz(m.body.pos.x, m.altitude, m.body.pos.y)
            .with_rotation(Quat::from_rotation_y(-m.body.heading))
    };
    let mut drawn = Vec::new();
    for (entity, model, mut pose) in &mut missiles {
        match flying.get(&model.0) {
            Some(m) => {
                *pose = pose_of(m);
                drawn.push(model.0);
            }
            None => commands.entity(entity).despawn(),
        }
    }
    let puff = |commands: &mut Commands, at: Vec3, size: f32| {
        commands.spawn((
            Puff { age: 0., size },
            Mesh3d(kit.puff.clone()),
            MeshMaterial3d(kit.smoke.clone()),
            Transform::from_translation(at).with_scale(Vec3::splat(size * 0.3)),
            NotShadowCaster,
        ));
    };
    for (&id, m) in &flying {
        if !drawn.contains(&id) {
            let body = commands
                .spawn((
                    MissileModel(id),
                    Mesh3d(kit.missile.clone()),
                    MeshMaterial3d(kit.airframe.clone()),
                    pose_of(m),
                ))
                .id();
            commands.spawn((
                Mesh3d(kit.flame.clone()),
                MeshMaterial3d(kit.flame_paint.clone()),
                Transform::from_xyz(0., 0., 1.15).with_scale(Vec3::new(0.26, 0.26, 0.5)),
                NotShadowCaster,
                ChildOf(body),
            ));
            // The booster leaves a cloud on the launcher as it fires.
            for k in 0..6 {
                let spread = Vec3::new(
                    (k as f32 * 2.1).sin(),
                    0.2 * k as f32,
                    (k as f32 * 1.3).cos(),
                );
                puff(&mut commands, tail(m) + spread * 0.6, 2.2);
            }
        }
        let since = trailed.entry(id).or_insert(0.);
        *since += dt;
        while *since >= PUFF_EVERY {
            *since -= PUFF_EVERY;
            puff(&mut commands, tail(m), 1.);
        }
    }
    trailed.retain(|id, _| flying.contains_key(id));
    for (entity, mut puff, mut pose) in &mut puffs {
        puff.age += dt;
        let t = puff.age / PUFF_LIFE;
        if t >= 1. {
            commands.entity(entity).despawn();
            continue;
        }
        // The smoke swells as it slows, then thins away.
        let swell = 0.3 + 1.4 * t.sqrt();
        let thin = 1. - ((t - 0.6) / 0.4).clamp(0., 1.);
        pose.scale = Vec3::splat(puff.size * swell * thin.max(0.05));
        pose.translation.y += 0.4 * dt;
    }
}

/// Bombs on their way down from the floatplane.
pub fn fall(
    run: Res<Campaign>,
    battle: Res<Encounter>,
    mut bombs: Query<(&BombModel, &mut Transform, &mut Visibility)>,
) {
    let sea = at_sea(&run);
    let falling = if sea { battle.bombs() } else { &[] };
    for (model, mut pose, mut shown) in &mut bombs {
        match falling.get(model.0) {
            Some(bomb) => {
                *pose = Transform::from_translation(bomb.position)
                    .with_rotation(Quat::from_rotation_x(-1.2))
                    .with_scale(Vec3::splat(1.6));
                shown.set_if_neq(Visibility::Inherited);
            }
            None => {
                shown.set_if_neq(Visibility::Hidden);
            }
        }
    }
}
