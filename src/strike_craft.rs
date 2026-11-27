//! Breakwater's catapult, floatplane and missile launchers: the gear on deck, the plane
//! in the air and the missiles with their smoke.
use crate::battle_view::spotter_model;
use crate::camera::{self, FollowCamera};
use crate::scene::Facets;
use crate::state::{Campaign, Encounter, Mode, Screen};
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use shipvivor_campaign::DockModule;
use shipvivor_encounter::{catapult_layout, launcher_layout, plane_index, planes_on};
use shipvivor_shipyard::StrikeCraft;
use std::collections::HashMap;

/// The floatplane is drawn at this share of the harbours' spotter, to suit the ship.
const PLANE_SCALE: f32 = 0.17;
/// In the air it is drawn a little larger, so it can be followed at a distance.
const AIRBORNE_SCALE: f32 = 0.25;
/// Height of the top of the catapult's trolley, where the plane's keel rests, its floats
/// hanging clear either side of the girder.
const TROLLEY: f32 = 2.32;
/// Seconds a puff of missile smoke lasts, and between puffs.
const PUFF_LIFE: f32 = 1.1;
const PUFF_EVERY: f32 = 0.035;
/// Bombs that can be falling at once: a full stick from every plane.
const BOMBS: usize = 40;

/// The plane on one catapult. It is hidden while the plane is flying.
#[derive(Component)]
pub struct CatapultPlane {
    instance: usize,
    slot: usize,
}
/// One catapult or launcher of a section's gear, by its place in the section's layout.
/// Those the section is not fitted with are hidden.
#[derive(Component)]
pub struct Rig {
    kind: DockModule,
    /// Which section of its kind it stands on, counted from the bow.
    instance: usize,
    slot: usize,
}
/// A catapult's girder and rails, shortened when catapults stand side by side.
#[derive(Component)]
pub struct Girder;
/// The crane that hoists the planes back aboard.
#[derive(Component)]
pub struct Crane;
/// The trained part of a missile launcher.
#[derive(Component)]
pub struct LauncherMount {
    instance: usize,
    slot: usize,
}
/// A launcher's box of cells, drawn for one size of salvo.
#[derive(Component)]
pub struct Cells(usize);
/// A floatplane in the air, by its order among the ship's planes.
#[derive(Component)]
pub struct Airborne(usize);
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

/// The most catapults a plane section carries, and launchers a rack.
const RIGS: usize = 4;
/// The most floatplanes she flies, from every plane section together.
const MOST_PLANES: usize = shipvivor_campaign::limits::PLANES as usize + 1;
/// The salvos a launcher's box is drawn for, smallest first.
const SALVOS: [usize; 5] = [2, 3, 4, 5, 6];

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
/// stretched across a widened beam. Every catapult or launcher the section could carry
/// is built, and [`rig`] shows those it is fitted with.
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
    let group = |commands: &mut Commands, parent: Entity, pose: Transform| {
        commands
            .spawn((pose, Visibility::Inherited, ChildOf(parent)))
            .id()
    };
    let at = |x, y, z, sx, sy, sz| Transform::from_xyz(x, y, z).with_scale(Vec3::new(sx, sy, sz));
    match kind {
        DockModule::Floatplane => {
            for slot in 0..RIGS {
                let rig = commands
                    .spawn((
                        Rig {
                            kind,
                            instance,
                            slot,
                        },
                        Transform::default(),
                        Visibility::Hidden,
                        ChildOf(gear),
                    ))
                    .id();
                // A turntable, the girder of the catapult along the centreline and its rails.
                part(
                    commands,
                    rig,
                    &parts.drum,
                    &parts.iron,
                    at(0., 1.68, 0.7, 1.6, 0.26, 1.6),
                );
                let girder = group(commands, rig, Transform::from_xyz(0., 0., -0.1));
                commands.entity(girder).insert(Girder);
                part(
                    commands,
                    girder,
                    &parts.cube,
                    &parts.steel,
                    at(0., 1.98, 0., 0.46, 0.3, 4.7),
                );
                for x in [-0.16, 0.16] {
                    part(
                        commands,
                        girder,
                        &parts.cube,
                        &parts.pale,
                        at(x, 2.15, 0., 0.05, 0.05, 4.7),
                    );
                }
                part(
                    commands,
                    rig,
                    &parts.cube,
                    &parts.iron,
                    at(0., TROLLEY - 0.08, 0.25, 0.62, 0.16, 0.95),
                );
                let plane = part(
                    commands,
                    rig,
                    &parts.plane,
                    &parts.airframe,
                    Transform::from_xyz(0., TROLLEY + spotter_model::KEEL * PLANE_SCALE, 0.15)
                        .with_scale(Vec3::splat(PLANE_SCALE)),
                );
                commands
                    .entity(plane)
                    .insert(CatapultPlane { instance, slot });
                commands.spawn((
                    Mesh3d(parts.propeller.clone()),
                    MeshMaterial3d(parts.airframe.clone()),
                    Transform::from_translation(spotter_model::HUB),
                    ChildOf(plane),
                ));
            }
            // The crane that hoists the planes back aboard, its jib over the catapults.
            let crane = group(commands, gear, Transform::from_xyz(1.85, 0., 1.7));
            commands.entity(crane).insert(Crane);
            part(
                commands,
                crane,
                &parts.drum,
                &parts.steel,
                at(0., 2.4, 0., 0.22, 1.7, 0.22),
            );
            let (root, tip) = (Vec3::new(0., 3.2, 0.), Vec3::new(-1.5, 3.55, -0.9));
            part(
                commands,
                crane,
                &parts.cube,
                &parts.steel,
                Transform::from_translation(root.midpoint(tip))
                    .looking_to(tip - root, Vec3::Y)
                    .with_scale(Vec3::new(0.13, 0.13, root.distance(tip))),
            );
            part(
                commands,
                crane,
                &parts.cube,
                &parts.dark,
                at(-1.5, 3.15, -0.9, 0.03, 0.8, 0.03),
            );
        }
        DockModule::Missiles => {
            for slot in 0..RIGS {
                let rig = commands
                    .spawn((
                        Rig {
                            kind,
                            instance,
                            slot,
                        },
                        Transform::default(),
                        Visibility::Hidden,
                        ChildOf(gear),
                    ))
                    .id();
                part(
                    commands,
                    rig,
                    &parts.drum,
                    &parts.iron,
                    at(0., 1.75, 0.3, 1.4, 0.42, 1.4),
                );
                let mount = commands
                    .spawn((
                        LauncherMount { instance, slot },
                        Transform::from_xyz(0., 1.96, 0.3),
                        Visibility::default(),
                        ChildOf(rig),
                    ))
                    .id();
                part(
                    commands,
                    mount,
                    &parts.cube,
                    &parts.steel,
                    at(0., 0.3, 0., 1.35, 0.45, 0.7),
                );
                // The box of cells, raised toward the bow: two cells abreast, three
                // abreast, two over two, or three over two or three.
                for cells in SALVOS {
                    let (width, height) = match cells {
                        2 => (1.2, 0.84),
                        3 => (1.75, 0.84),
                        4 => (1.2, 1.2),
                        _ => (1.75, 1.2),
                    };
                    let cradle = commands
                        .spawn((
                            Cells(cells),
                            Transform::from_xyz(0., 0.62, 0.)
                                .with_rotation(Quat::from_rotation_x(0.3)),
                            Visibility::Hidden,
                            ChildOf(mount),
                        ))
                        .id();
                    let middle = height / 2.;
                    part(
                        commands,
                        cradle,
                        &parts.cube,
                        &parts.pale,
                        at(0., middle, 0., width, height, 2.9),
                    );
                    let doors: &[(f32, f32)] = match cells {
                        2 => &[(-0.29, 0.42), (0.29, 0.42)],
                        3 => &[(-0.56, 0.42), (0., 0.42), (0.56, 0.42)],
                        4 => &[(-0.29, 0.33), (0.29, 0.33), (-0.29, 0.87), (0.29, 0.87)],
                        5 => &[
                            (-0.56, 0.33),
                            (0., 0.33),
                            (0.56, 0.33),
                            (-0.29, 0.87),
                            (0.29, 0.87),
                        ],
                        _ => &[
                            (-0.56, 0.33),
                            (0., 0.33),
                            (0.56, 0.33),
                            (-0.56, 0.87),
                            (0., 0.87),
                            (0.56, 0.87),
                        ],
                    };
                    // Two rows of cells are each a little lower than one.
                    let door = if cells > 3 { 0.44 } else { 0.58 };
                    for &(x, y) in doors {
                        part(
                            commands,
                            cradle,
                            &parts.cube,
                            &parts.dark,
                            at(x, y, -1.46, 0.44, door, 0.04),
                        );
                    }
                    for z in [-0.9, 0.9] {
                        part(
                            commands,
                            cradle,
                            &parts.cube,
                            &parts.iron,
                            at(0., middle, z, width + 0.06, height + 0.06, 0.12),
                        );
                    }
                }
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
    for index in 0..MOST_PLANES {
        let plane = commands
            .spawn((
                Airborne(index),
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
    }
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

/// How the strike craft are fitted out: as planned in the yard, as shipped at sea.
/// Plane sections aboard the ship as drawn.
fn plane_sections(run: &Campaign) -> usize {
    run.display_design()
        .count(shipvivor_shipyard::Component::Catapult)
}

fn fitted(run: &Campaign, battle: &Encounter) -> StrikeCraft {
    if matches!(run.screen(), Screen::Drydock(_)) {
        run.display_fittings().strike_craft()
    } else {
        battle.strike_craft()
    }
}

/// Stand each catapult and launcher a section is fitted with at its place, give each
/// launcher the box for its salvo, and bring the crane to stand clear of the catapults.
#[allow(clippy::type_complexity)]
pub fn rig(
    run: Res<Campaign>,
    battle: Res<Encounter>,
    mut rigs: Query<(&Rig, &mut Transform, &mut Visibility)>,
    mut girders: Query<&mut Transform, (With<Girder>, Without<Rig>, Without<Crane>)>,
    mut cranes: Query<&mut Transform, (With<Crane>, Without<Rig>, Without<Girder>)>,
    mut boxes: Query<(&Cells, &mut Visibility), Without<Rig>>,
) {
    let craft = fitted(&run, &battle);
    // The planes are shared out between the plane sections.
    let sections = plane_sections(&run);
    let planes = |instance| planes_on(craft.planes, sections, instance);
    for (rig, mut pose, mut shown) in &mut rigs {
        let layout = match rig.kind {
            DockModule::Floatplane => catapult_layout(planes(rig.instance)),
            _ => launcher_layout(craft.launchers),
        };
        let Some(&(fore, abeam)) = layout.get(rig.slot) else {
            shown.set_if_neq(Visibility::Hidden);
            continue;
        };
        shown.set_if_neq(Visibility::Inherited);
        pose.translation = Vec3::new(abeam, 0., -fore);
        // Launchers side by side are built smaller, so their boxes clear one another.
        let size = match (rig.kind, craft.launchers) {
            (DockModule::Missiles, 2) => 0.85,
            (DockModule::Missiles, 3) => 0.75,
            (DockModule::Missiles, 4..) => 0.7,
            _ => 1.,
        };
        pose.scale = Vec3::splat(size);
    }
    // Catapults side by side are shorter, so their ends stay on the section.
    let crowded = planes(0) > 1;
    let girder = if crowded { 0.8 } else { 1. };
    for mut pose in &mut girders {
        pose.scale.z = girder;
    }
    for mut pose in &mut cranes {
        *pose = if crowded {
            // On the centreline abaft the catapults, its jib swung out to port clear of
            // the planes.
            Transform::from_xyz(0., 0., 2.25).with_rotation(Quat::from_rotation_arc(
                Vec3::new(-1.5, 0., -0.9).normalize(),
                Vec3::NEG_X,
            ))
        } else {
            Transform::from_xyz(1.85, 0., 1.7)
        };
    }
    let salvo = craft.salvo.clamp(SALVOS[0], SALVOS[SALVOS.len() - 1]);
    for (cells, mut shown) in &mut boxes {
        shown.set_if_neq(if cells.0 == salvo {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
}

/// Keep each plane on its catapult while it is aboard, and lay each launcher.
pub fn stow(
    run: Res<Campaign>,
    battle: Res<Encounter>,
    mut planes: Query<(&CatapultPlane, &mut Visibility)>,
    mut mounts: Query<(&LauncherMount, &mut Transform)>,
) {
    let sea = at_sea(&run);
    let craft = battle.strike_craft();
    let sections = plane_sections(&run);
    for (plane, mut shown) in &mut planes {
        let index = plane_index(craft.planes, sections, plane.instance, plane.slot);
        let aloft = sea
            && index
                .and_then(|index| battle.floatplanes().get(index))
                .is_some_and(|plane| !plane.aboard());
        shown.set_if_neq(if aloft {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
    }
    for (mount, mut pose) in &mut mounts {
        let index = mount.instance * craft.launchers + mount.slot;
        let train = if sea {
            battle.launchers().get(index).map_or(0., |l| l.train)
        } else {
            0.
        };
        pose.rotation = Quat::from_rotation_y(-train);
    }
}

/// The floatplanes in the air, their propellers turning.
pub fn fly(
    time: Res<Time>,
    run: Res<Campaign>,
    battle: Res<Encounter>,
    mut planes: Query<(&Airborne, &mut Transform, &mut Visibility)>,
    mut spinners: Query<&mut Transform, (With<Spinner>, Without<Airborne>)>,
    lens: Option<
        Single<
            (&Transform, &Projection),
            (With<FollowCamera>, Without<Airborne>, Without<Spinner>),
        >,
    >,
) {
    let sea = at_sea(&run);
    for (airborne, mut pose, mut shown) in &mut planes {
        let flying = battle
            .floatplanes()
            .get(airborne.0)
            .filter(|p| sea && !p.aboard());
        let Some(flying) = flying else {
            shown.set_if_neq(Visibility::Hidden);
            continue;
        };
        let at = flying.position_3d();
        let clear = lens.as_ref().map_or(1., |lens| {
            camera::lens_clearance(lens.0, lens.1, at, spotter_model::REACH * AIRBORNE_SCALE)
        });
        *pose = Transform::from_translation(at)
            .with_rotation(
                Quat::from_rotation_y(-flying.heading) * Quat::from_rotation_z(flying.bank),
            )
            .with_scale(Vec3::splat(AIRBORNE_SCALE * clear));
        shown.set_if_neq(Visibility::Inherited);
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
