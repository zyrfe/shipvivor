//! Presentation of the current encounter. Actor ids keep meshes stable across fixed ticks.
#[path = "armor_fx.rs"]
mod armor_fx;
#[path = "foreshore.rs"]
mod foreshore;
#[path = "harbour_models.rs"]
mod harbour_models;
#[path = "harbour_signals.rs"]
mod harbour_signals;
#[path = "mainland.rs"]
mod mainland;
#[path = "spotter_model.rs"]
pub(crate) mod spotter_model;
use crate::camera::{self, FollowCamera};
use crate::ensign::Side;
use crate::scene;
use crate::state::{Campaign, Encounter, Mode, PlayerHelm};
use crate::surfaces;
use bevy::prelude::*;
use foreshore::Coast;
pub(crate) use foreshore::shoals;
use harbour_models::Harbour;
pub(crate) use harbour_signals::{
    animate as animate_harbour_signals, dye as dye_ensigns, fly_pennant, stream_funnel_smoke,
};
pub(crate) use mainland::MainlandPlugin;
use shipvivor_encounter::{
    Airframe, Battery, Body, EffectKind, EnemyClass, ISLAND_RADIUS, LandmarkKind, YardState,
};
use std::collections::HashMap;

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
enum Shape {
    Spotter,
    AttackPlane,
    MooredPlane,
    /// A harrier lying at a base's mooring, by its berth there.
    MooredHarrier(u8),
    Hangar(bool),
    Propeller,
    Coast(Coast),
    Yard(Harbour, YardState),
    YardFlag,
    Conveyor,
    Cradle(u8),
    Lighter(u8),
    PlateRigging,
    SuspendedPlate,
    Factory(Harbour, u8, bool),
    Exhaust(u8),
    GunPlatform(u8),
    SecondaryMount(u8),
    Tow(u8),
    ShoreGun(u8, u8),
    ShoreShell,
    Torpedo,
    /// An enemy torpedo, red in the head.
    EnemyTorpedo,
    /// A stretch of the white bubble track an enemy torpedo leaves, nearest it first.
    Track(u8),
    /// A torpedo boat's bow wave either side and the wash under her stern, at speed.
    Wash(u8),
    Ship(EnemyClass),
    /// The lip of one of a ship's funnels, which her smoke streams from.
    Stack(EnemyClass, u8),
    /// One of a ship's turrets, and the barrels in it.
    Gun(u8, EnemyClass, u8),
    Socket(u8),
    GunFire(u8),
    Soot(u8),
    Primer(u8),
    Blast(u8),
    ImpactFlash,
    Streak(u8),
    Scar(u8, bool),
    /// A mark where a round struck a shore gunhouse and left it standing: the gun, the
    /// mark, and whether the round holed the plate.
    Pock(u8, u8, bool),
    Director,
    DentRing,

    Cargo,
    MainShot,
    CloseShot,
    EnemyShot,
    /// A light AA round: all tracer, no visible body.
    Tracer,
    /// Dark smoke from a flak burst or a burning aircraft.
    FlakPuff(u8),
    Flash,
    Muzzle,
    Smoke,
    FunnelSmoke(u8),
    FoamRing,
    Splash,
    Debris(u8),
    /// A lump of ice thrown up where a berg breaks.
    Shard(u8),
    Fire,
}
#[derive(Component, Clone, Copy, Hash, PartialEq, Eq)]
pub(crate) struct Actor(u64, Shape);

/// The scene key for drydock `dock` of the port with body `id`: the first dock keeps the
/// port's own, and the others are set apart in bits no serial number reaches.
fn dock_key(id: u64, dock: usize) -> u64 {
    id | ((dock as u64) << 48)
}
#[derive(Resource)]
pub struct Kit {
    spotter: Handle<Mesh>,
    attack_plane: Handle<Mesh>,
    hangars: [Handle<Mesh>; 2],
    propeller: Handle<Mesh>,
    hull: Handle<Mesh>,
    scars: [Handle<Mesh>; 2],
    impact_flash: Handle<Mesh>,
    streak: Handle<Mesh>,
    incandescent: Handle<StandardMaterial>,
    fleet: HashMap<EnemyClass, VesselMeshes>,
    slipways: HashMap<Harbour, Handle<Mesh>>,
    coasts: HashMap<Coast, Handle<Mesh>>,
    ground: Handle<StandardMaterial>,
    construction: Handle<StandardMaterial>,
    // Index zero is the silenced mount; live meshes are indexed by barrel count.
    shore_battery: [Handle<Mesh>; 5],
    gun_platform: Handle<Mesh>,
    yards: HashMap<(Harbour, YardState), Handle<Mesh>>,
    factories: HashMap<(Harbour, bool), Handle<Mesh>>,
    conveyor: Handle<Mesh>,
    signals: harbour_signals::SignalKit,
    cradles: [Handle<Mesh>; 3],
    lighters: [Handle<Mesh>; 3],
    rigging: Handle<Mesh>,
    plate: Handle<Mesh>,
    cube: Handle<Mesh>,
    shell: Handle<Mesh>,
    spray: Handle<Mesh>,
    ring: Handle<Mesh>,
    plume: Handle<Mesh>,
    casing: Handle<StandardMaterial>,
    gunmetal: Handle<StandardMaterial>,
    spray_paint: Handle<StandardMaterial>,
    smoke: Handle<StandardMaterial>,
    flak: Handle<StandardMaterial>,
    ball: Handle<Mesh>,
    drum: Handle<Mesh>,
    terrain: Handle<StandardMaterial>,
    quay: Handle<StandardMaterial>,
    steel: Handle<StandardMaterial>,
    dark: Handle<StandardMaterial>,
    red: Handle<StandardMaterial>,
    gold: Handle<StandardMaterial>,
    flame: Handle<StandardMaterial>,
    foam: Handle<StandardMaterial>,
}
struct VesselMeshes {
    deckwork: Handle<Mesh>,
    /// Her turret with one barrel, two, three and four.
    guns: [Handle<Mesh>; 4],
    masthead: f32,
    /// Each funnel's lip on her deckwork, and the width of its mouth.
    funnels: Vec<(Vec3, f32)>,
}
impl Kit {
    pub fn masthead(&self, class: EnemyClass) -> f32 {
        self.fleet[&class].masthead
    }
}
pub fn setup(
    mut cmd: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut paint: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let casing = paint.add(StandardMaterial {
        metallic: 0.28,
        perceptual_roughness: 0.43,
        ..default()
    });
    let incandescent = paint.add(StandardMaterial {
        base_color: Color::linear_rgb(3.8, 3.5, 2.9),
        unlit: true,
        cull_mode: None,
        ..default()
    });
    let gunmetal = paint.add(StandardMaterial {
        emissive: LinearRgba::rgb(0.028, 0.032, 0.036),
        metallic: 0.12,
        perceptual_roughness: 0.62,
        ..default()
    });
    let spray_paint = paint.add(StandardMaterial {
        unlit: true,
        cull_mode: None,
        ..default()
    });
    let smoke = paint.add(StandardMaterial {
        base_color: Color::srgba_u8(86, 92, 98, 152),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let flak = paint.add(StandardMaterial {
        base_color: Color::srgba_u8(34, 34, 38, 214),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let signals = harbour_signals::SignalKit::new(&mut meshes, &mut paint, &mut images);
    let mut yards = HashMap::new();
    let mut factories = HashMap::new();
    for harbour in Harbour::ALL {
        for state in [
            YardState::Working,
            YardState::Damaged,
            YardState::Burning,
            YardState::Ruined,
        ] {
            yards.insert(
                (harbour, state),
                meshes.add(harbour_models::dock(harbour, state)),
            );
        }
        for live in [false, true] {
            factories.insert(
                (harbour, live),
                meshes.add(harbour_models::factory(harbour, live)),
            );
        }
    }
    let ground = paint.add(foreshore::material(&mut images));
    let construction = paint.add(harbour_models::material(&mut images));
    let coasts = Harbour::ALL
        .map(Coast::Yard)
        .into_iter()
        .chain(
            [
                LandmarkKind::Beacon,
                LandmarkKind::Pines,
                LandmarkKind::Stacks,
                LandmarkKind::Glacier,
                LandmarkKind::Mainland,
            ]
            .map(Coast::Landmark),
        )
        .map(|coast| (coast, meshes.add(foreshore::mesh(coast))))
        .collect();
    let mut material = |r, g, b, unlit| {
        paint.add(StandardMaterial {
            base_color: Color::srgb_u8(r, g, b),
            unlit,
            ..default()
        })
    };
    cmd.insert_resource(Kit {
        spotter: meshes.add(spotter_model::plane()),
        attack_plane: meshes.add(spotter_model::raider()),
        hangars: [false, true].map(|live| meshes.add(harbour_models::hangar(live))),
        propeller: meshes.add(spotter_model::propeller()),
        hull: meshes.add(scene::hull_mesh()),
        impact_flash: meshes.add(armor_fx::flash_mesh()),
        streak: meshes.add(armor_fx::spark_mesh()),
        incandescent,
        scars: [false, true].map(|open| meshes.add(armor_fx::scar(open))),
        slipways: Harbour::ALL
            .map(|harbour| (harbour, meshes.add(harbour_models::slipway(harbour))))
            .into_iter()
            .collect(),
        coasts,
        ground,
        construction,
        shore_battery: [0, 1, 2, 3, 4].map(|barrels| meshes.add(harbour_models::battery(barrels))),
        gun_platform: meshes.add(harbour_models::gun_platform()),
        yards,
        factories,
        signals,
        conveyor: meshes.add(harbour_models::conveyor()),
        cradles: [0, 1, 2].map(|progress| meshes.add(harbour_models::cradle(progress))),
        lighters: [0, 1, 2].map(|progress| meshes.add(harbour_models::lighter(progress))),
        rigging: meshes.add(harbour_models::plate_rigging()),
        plate: meshes.add(harbour_models::suspended_plate()),
        fleet: EnemyClass::ALL
            .into_iter()
            .map(|class| {
                use bevy::camera::primitives::MeshAabb;
                let deckwork = crate::fleet_models::superstructure(class);
                let bounds = deckwork.get_aabb().expect("deckwork has vertices");
                (
                    class,
                    VesselMeshes {
                        masthead: bounds.max().y,
                        deckwork: meshes.add(deckwork),
                        guns: [1, 2, 3, 4]
                            .map(|barrels| meshes.add(crate::fleet_models::gun(class, barrels))),
                        funnels: crate::fleet_models::funnels(class),
                    },
                )
            })
            .collect(),
        cube: meshes.add(Cuboid::default()),
        shell: meshes.add(crate::ordnance::shell()),
        spray: meshes.add(crate::ordnance::spray()),
        ring: meshes.add(Torus::new(0.95, 1.)),
        plume: meshes.add(Cone::new(0.5, 1.).mesh().resolution(8)),
        casing,
        gunmetal,
        spray_paint,
        smoke,
        flak,
        ball: meshes.add(Sphere::new(1.).mesh().ico(1).unwrap()),
        drum: meshes.add(Cylinder::new(0.5, 1.).mesh().resolution(8)),
        terrain: material(255, 255, 255, true),
        quay: material(151, 121, 89, false),
        steel: material(202, 201, 174, false),
        dark: material(49, 55, 62, false),
        red: material(211, 70, 50, true),
        gold: material(249, 189, 70, true),
        flame: material(255, 117, 37, true),
        foam: material(196, 240, 222, true),
    });
}
/// The air station's strafer is the attack plane's airframe at this share of its size,
/// 5 m across. A harrier is drawn at the second share, a little over 3 m across.
const STRAFER: f32 = 0.315;
const HARRIER: f32 = 0.2;
/// Moored, an airframe rides with its origin this many model metres above the water,
/// which puts its floats just under.
const AFLOAT: f32 = 2.94;
/// Seconds at the end of a downed aircraft's effect over which its wreck goes under,
/// and the model metres it settles by then, enough to take the tail down too.
const SINKING: f32 = 1.8;
const SUNK: f32 = 9.;
/// Metres of the bubble track drawn behind an enemy torpedo, in stretches, and how wide
/// it is at the torpedo and at its far end.
const TRACK: (f32, u8) = (72., 4);
const TRACK_WIDTH: (f32, f32) = (0.55, 1.5);

/// The white track an enemy torpedo leaves along its run, widening as its bubbles rise
/// and spread behind it. It is what gives the torpedo away at range.
fn enemy_track(
    fish: &shipvivor_encounter::Torpedo,
    sea: &crate::seascape::Sea,
    wanted: &mut HashMap<Actor, Transform>,
) {
    let (length, stretches) = TRACK;
    let stretch = length / f32::from(stretches);
    let back = -fish.body.forward();
    for k in 0..stretches {
        let near = f32::from(k) * stretch + 1.5;
        let far = (near + stretch).min(fish.run());
        if far <= near {
            break;
        }
        let middle = fish.body.pos + back * (0.5 * (near + far));
        let spread = (0.5 * (near + far) / length).min(1.);
        let width = TRACK_WIDTH.0 + (TRACK_WIDTH.1 - TRACK_WIDTH.0) * spread;
        let at = Body {
            pos: middle,
            ..fish.body
        };
        let xf = pose(&at, sea.height(middle) + 0.04).with_scale(Vec3::new(
            width,
            0.07,
            0.5 * (far - near),
        ));
        wanted.insert(Actor(fish.body.id, Shape::Track(k)), xf);
    }
}

fn pose(b: &Body, h: f32) -> Transform {
    Transform::from_xyz(b.pos.x, h, b.pos.y).with_rotation(Quat::from_rotation_y(-b.heading))
}
/// Shrink each aircraft flying close past the camera, and the propeller turning on its
/// nose, so that it never blots out the view (see `camera::lens_clearance`).
fn clear_lens(wanted: &mut HashMap<Actor, Transform>, lens: &Transform, projection: &Projection) {
    let airframes: Vec<(Actor, Transform)> = (wanted.iter())
        .filter(|(key, _)| {
            matches!(
                key.1,
                Shape::Spotter | Shape::AttackPlane | Shape::MooredPlane | Shape::MooredHarrier(_)
            )
        })
        .map(|(key, frame)| (*key, *frame))
        .collect();
    for (key, frame) in airframes {
        let reach = spotter_model::REACH * frame.scale.x;
        let clear = camera::lens_clearance(lens, projection, frame.translation, reach);
        if clear >= 1. {
            continue;
        }
        let mut shrink = |part: Actor| {
            if let Some(xf) = wanted.get_mut(&part) {
                xf.translation = frame.translation + (xf.translation - frame.translation) * clear;
                xf.scale *= clear;
            }
        };
        shrink(key);
        // Moored airframes carry their propeller with them; one in the air has it apart.
        if matches!(key.1, Shape::Spotter | Shape::AttackPlane) {
            shrink(Actor(key.0, Shape::Propeller));
        }
    }
}
fn create(cmd: &mut Commands, kit: &Kit, key: Actor, xf: Transform) {
    let root = cmd.spawn((key, xf, Visibility::default())).id();
    let mut part =
        |mesh: &Handle<Mesh>, mat: &Handle<StandardMaterial>, position: Vec3, scale: Vec3| {
            cmd.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(mat.clone()),
                Transform::from_translation(position).with_scale(scale),
                ChildOf(root),
            ));
        };
    let v = Vec3::new;
    match key.1 {
        Shape::Spotter => part(&kit.spotter, &kit.gunmetal, Vec3::ZERO, Vec3::ONE),
        Shape::AttackPlane | Shape::MooredPlane | Shape::MooredHarrier(_) => {
            part(&kit.attack_plane, &kit.gunmetal, Vec3::ZERO, Vec3::ONE);
            if key.1 != Shape::AttackPlane {
                part(&kit.propeller, &kit.gunmetal, spotter_model::HUB, Vec3::ONE);
            }
        }
        Shape::Hangar(live) => part(
            &kit.hangars[usize::from(live)],
            &kit.construction,
            Vec3::ZERO,
            Vec3::ONE,
        ),
        Shape::Propeller => part(&kit.propeller, &kit.gunmetal, Vec3::ZERO, Vec3::ONE),
        Shape::Coast(coast) => {
            part(&kit.coasts[&coast], &kit.ground, Vec3::ZERO, Vec3::ONE);
            match coast {
                Coast::Yard(harbour) => {
                    part(
                        &kit.slipways[&harbour],
                        &kit.construction,
                        Vec3::ZERO,
                        Vec3::ONE,
                    );
                }
                Coast::Landmark(LandmarkKind::Beacon) => {
                    part(&kit.drum, &kit.steel, v(0., 9., 0.), v(4.8, 12., 4.8));
                    part(&kit.drum, &kit.red, v(0., 11., 0.), v(4.9, 2.6, 4.9));
                    part(&kit.drum, &kit.dark, v(0., 15., 0.), v(6., 1., 6.));
                    part(&kit.drum, &kit.gold, v(0., 16.6, 0.), v(3.7, 2.4, 3.7));
                    part(&kit.plume, &kit.red, v(0., 18.6, 0.), v(5.6, 2., 5.6));
                }
                Coast::Landmark(_) => {}
            }
        }
        Shape::Yard(harbour, state) => part(
            &kit.yards[&(harbour, state)],
            &kit.construction,
            Vec3::ZERO,
            Vec3::ONE,
        ),
        Shape::YardFlag => {
            part(&kit.drum, &kit.steel, v(0., 1.2, 0.), v(0.22, 2.4, 0.22));
            kit.signals
                .spawn_flag(cmd, root, Side::Enemy, Transform::from_xyz(0., 2.4, 0.));
        }
        Shape::Exhaust(_) => kit.signals.spawn_smoke(cmd, root),
        Shape::Stack(class, stack) => {
            let (_, mouth) = kit.fleet[&class].funnels[usize::from(stack)];
            kit.signals.spawn_funnel_smoke(cmd, root, mouth);
        }
        Shape::Conveyor => part(&kit.conveyor, &kit.construction, Vec3::ZERO, Vec3::ONE),
        Shape::Factory(harbour, _, live) => part(
            &kit.factories[&(harbour, live)],
            &kit.construction,
            Vec3::ZERO,
            Vec3::ONE,
        ),
        Shape::Cradle(stage) => part(
            &kit.cradles[usize::from(stage)],
            &kit.construction,
            Vec3::ZERO,
            Vec3::ONE,
        ),
        Shape::Lighter(stage) => part(
            &kit.lighters[usize::from(stage)],
            &kit.construction,
            Vec3::ZERO,
            Vec3::ONE,
        ),
        Shape::PlateRigging => part(&kit.rigging, &kit.construction, Vec3::ZERO, Vec3::ONE),
        Shape::SuspendedPlate => part(&kit.plate, &kit.construction, Vec3::ZERO, Vec3::ONE),
        Shape::GunPlatform(_) => part(&kit.gun_platform, &kit.construction, Vec3::ZERO, Vec3::ONE),
        Shape::ShoreGun(_, barrels) => part(
            &kit.shore_battery[usize::from(barrels)],
            &kit.construction,
            Vec3::ZERO,
            Vec3::ONE,
        ),
        Shape::SecondaryMount(_) => part(
            &kit.fleet[&EnemyClass::Patrol].guns[0],
            &kit.gunmetal,
            Vec3::ZERO,
            Vec3::ONE,
        ),
        Shape::Tow(_) => part(&kit.cube, &kit.quay, Vec3::ZERO, Vec3::ONE),
        Shape::Torpedo | Shape::EnemyTorpedo => {
            let head = if key.1 == Shape::Torpedo {
                &kit.gold
            } else {
                &kit.red
            };
            part(&kit.ball, &kit.dark, v(0., 0., 0.), v(0.29, 0.28, 1.68));
            part(&kit.ball, head, v(0., 0., -1.23), v(0.27, 0.24, 0.52));
            part(&kit.ball, &kit.foam, v(0., 0., 4.), v(0.35, 0.12, 3.7));
            for side in [-1., 1.] {
                part(
                    &kit.ball,
                    &kit.foam,
                    v(side * 0.48, 0., 8.),
                    v(0.25, 0.09, 2.5),
                );
            }
        }
        Shape::Ship(class) => {
            let p = class.profile();
            part(
                &kit.hull,
                &kit.casing,
                Vec3::ZERO,
                v(p.hull.radius / 2.75, p.deck / 1.55, p.hull.bow / 16.),
            );
            part(
                &kit.fleet[&class].deckwork,
                &kit.gunmetal,
                Vec3::ZERO,
                Vec3::ONE,
            );
            // Her colours stay hoisted as she goes down.
            for (hoist, scale) in crate::fleet_models::ensigns(class) {
                let at = Transform::from_translation(hoist).with_scale(Vec3::splat(scale));
                kit.signals.spawn_flag(cmd, root, Side::Enemy, at);
            }
        }
        Shape::Gun(_, class, barrels) => {
            let gun = &kit.fleet[&class].guns[usize::from(barrels.clamp(1, 4) - 1)];
            part(gun, &kit.gunmetal, Vec3::ZERO, Vec3::ONE);
            if class.is_flagship() {
                part(&kit.drum, &kit.gold, v(0., 1.36, 0.), v(1.18, 0.11, 1.18));
            }
        }
        Shape::Socket(_) => part(&kit.drum, &kit.dark, Vec3::ZERO, Vec3::ONE),
        Shape::Primer(_) => part(&kit.ball, &kit.foam, Vec3::ZERO, v(0.5, 0.35, 0.5)),
        Shape::Scar(_, open) | Shape::Pock(_, _, open) => part(
            &kit.scars[usize::from(open)],
            &kit.terrain,
            Vec3::ZERO,
            Vec3::ONE,
        ),
        Shape::Streak(_) => part(&kit.streak, &kit.incandescent, Vec3::ZERO, Vec3::ONE),
        Shape::ImpactFlash => part(&kit.impact_flash, &kit.incandescent, Vec3::ZERO, Vec3::ONE),
        Shape::DentRing => part(&kit.ring, &kit.foam, Vec3::ZERO, Vec3::ONE),
        Shape::Director => {
            part(&kit.cube, &kit.dark, Vec3::ZERO, v(4.8, 0.7, 1.));
            for side in [-1., 1.] {
                part(
                    &kit.ball,
                    &kit.gold,
                    v(side * 2.2, 0.1, 0.),
                    v(0.62, 0.42, 0.56),
                );
            }
        }
        Shape::Cargo => {
            part(&kit.cube, &kit.gold, v(0., 0.9, 0.), v(3., 2., 3.));
            part(&kit.cube, &kit.dark, v(0., 1., 0.), v(0.55, 2.25, 3.05));
            part(&kit.cube, &kit.dark, v(0., 1., 0.), v(3.05, 2.25, 0.55));
            part(&kit.drum, &kit.foam, v(0., -0.1, 0.), v(5., 0.16, 5.));
        }
        Shape::MainShot | Shape::CloseShot | Shape::EnemyShot | Shape::ShoreShell => {
            part(&kit.shell, &kit.casing, Vec3::ZERO, Vec3::ONE);
            let hot = if matches!(key.1, Shape::EnemyShot | Shape::ShoreShell) {
                &kit.flame
            } else {
                &kit.gold
            };
            part(&kit.ball, hot, v(0., 0., 5.5), v(0.20, 0.20, 4.6));
            part(&kit.ball, &kit.foam, v(0., 0., 1.35), v(0.23, 0.23, 0.72));
        }
        Shape::Muzzle => {
            part(&kit.ball, &kit.flame, v(0., 0., -1.45), v(1.34, 1.15, 2.2));
            part(&kit.ball, &kit.gold, v(0., 0., -1.8), v(0.98, 0.94, 2.6));
            part(&kit.ball, &kit.foam, v(0., 0., -1.5), v(0.5, 0.50, 2.5));
        }
        Shape::Tracer => {
            part(&kit.ball, &kit.gold, v(0., 0., 1.1), v(0.075, 0.075, 2.2));
            part(&kit.ball, &kit.foam, v(0., 0., 0.15), v(0.09, 0.09, 0.3));
        }
        Shape::FlakPuff(_) => {
            part(&kit.ball, &kit.flak, Vec3::ZERO, v(1., 0.86, 0.94));
            part(&kit.ball, &kit.flak, v(0.55, 0.3, -0.2), v(0.7, 0.62, 0.66));
            part(
                &kit.ball,
                &kit.flak,
                v(-0.45, 0.18, 0.4),
                v(0.62, 0.56, 0.6),
            );
        }
        Shape::Smoke | Shape::FunnelSmoke(_) | Shape::Soot(_) => {
            part(&kit.ball, &kit.smoke, Vec3::ZERO, v(1.2, 0.95, 1.1));
            part(
                &kit.ball,
                &kit.smoke,
                v(0.85, 0.42, 0.24),
                v(0.8, 0.66, 0.72),
            );
        }
        Shape::Flash | Shape::Blast(_) => {
            part(&kit.ball, &kit.flame, Vec3::ZERO, v(1.15, 1., 1.));
            part(
                &kit.ball,
                &kit.gold,
                v(0.45, 0.30, 0.17),
                v(0.65, 0.75, 0.68),
            );
        }
        Shape::FoamRing => part(&kit.ring, &kit.foam, Vec3::ZERO, Vec3::ONE),
        Shape::Track(_) | Shape::Wash(_) => part(&kit.ball, &kit.foam, Vec3::ZERO, Vec3::ONE),
        Shape::Fire | Shape::GunFire(_) => {
            part(&kit.ball, &kit.flame, Vec3::ZERO, v(1.2, 2.4, 1.4));
            part(&kit.ball, &kit.gold, v(-0.6, 0., -0.45), v(0.7, 1.3, 1.));
            part(&kit.plume, &kit.gold, v(0.35, 1.12, 0.22), v(0.9, 2.8, 1.1));
        }
        Shape::Splash => {
            part(&kit.spray, &kit.spray_paint, Vec3::ZERO, Vec3::ONE);
            part(&kit.plume, &kit.foam, v(0., 0.54, 0.), v(0.26, 1.08, 0.28));
        }
        Shape::Debris(_) => part(&kit.cube, &kit.dark, Vec3::ZERO, v(0.8, 0.7, 1.3)),
        Shape::Shard(_) => part(&kit.cube, &kit.foam, Vec3::ZERO, v(1., 0.75, 1.25)),
    }
}
pub fn sync(
    mut cmd: Commands,
    kit: Res<Kit>,
    run: Res<Campaign>,
    battle: Res<Encounter>,
    helm: Res<PlayerHelm>,
    sea: Res<crate::seascape::Sea>,
    mut actors: Query<(Entity, &Actor, &mut Transform)>,
    lens: Option<Single<(&Transform, &Projection), (With<FollowCamera>, Without<Actor>)>>,
) {
    let mut wanted = HashMap::new();
    if !matches!(run.mode, Mode::Title | Mode::Trial) {
        // The mainland is raised as one terrain (see `mainland`), not piece by piece.
        for island in battle
            .landmarks()
            .iter()
            .filter(|l| l.kind != LandmarkKind::Mainland)
        {
            let scale = island.radius / ISLAND_RADIUS;
            wanted.insert(
                Actor(island.body.id, Shape::Coast(Coast::Landmark(island.kind))),
                pose(&island.body, 0.).with_scale(Vec3::new(scale, 1., scale)),
            );
        }
        if let Some(plane) = battle.air_station().and_then(|base| base.aircraft.as_ref()) {
            let mut airframe = pose(&plane.body, plane.altitude).with_scale(Vec3::splat(STRAFER));
            airframe.rotate_local_z(plane.bank);
            wanted.insert(Actor(plane.body.id, Shape::AttackPlane), airframe);
            let spin = Transform::from_translation(spotter_model::HUB)
                .with_rotation(Quat::from_rotation_z(battle.at_sea() * 42.));
            wanted.insert(
                Actor(plane.body.id, Shape::Propeller),
                airframe.mul_transform(spin),
            );
        } else if let Some(base) = battle.air_station() {
            let port = &battle.ports()[base.port];
            if base.operational(port) {
                let mooring = Body {
                    pos: base.slip(port),
                    ..port.body
                };
                wanted.insert(
                    Actor(port.body.id, Shape::MooredPlane),
                    pose(&mooring, AFLOAT * STRAFER + sea.height(mooring.pos))
                        .with_scale(Vec3::splat(STRAFER)),
                );
            }
        }
        // A harrier is the attack plane drawn small, and its propeller turns faster.
        for plane in battle.harriers() {
            let mut airframe = pose(&plane.body, plane.altitude).with_scale(Vec3::splat(HARRIER));
            airframe.rotate_local_z(plane.bank);
            wanted.insert(Actor(plane.body.id, Shape::AttackPlane), airframe);
            let spin = Transform::from_translation(spotter_model::HUB)
                .with_rotation(Quat::from_rotation_z(battle.at_sea() * 47.));
            wanted.insert(
                Actor(plane.body.id, Shape::Propeller),
                airframe.mul_transform(spin),
            );
        }
        // Each base's harriers lie at its mooring until they take off, wherever her hull
        // has shoved them.
        for plane in battle.moored_harriers() {
            let afloat = AFLOAT * HARRIER + sea.height(plane.body.pos);
            wanted.insert(
                Actor(plane.body.id, Shape::MooredHarrier(plane.slot as u8)),
                pose(&plane.body, afloat).with_scale(Vec3::splat(HARRIER)),
            );
        }
        for (base_index, (port, plane)) in battle.ports().iter().zip(battle.spotters()).enumerate()
        {
            // A spotter shot down is drawn falling by its effect, and then is gone.
            if plane.airborne() {
                let rotation =
                    Quat::from_rotation_y(-plane.heading) * Quat::from_rotation_z(plane.bank);
                let airframe = Transform::from_translation(plane.position_3d())
                    .with_rotation(rotation)
                    .with_scale(Vec3::splat(spotter_model::ALOFT));
                wanted.insert(Actor(port.body.id, Shape::Spotter), airframe);
                let spin = Transform::from_translation(spotter_model::HUB)
                    .with_rotation(Quat::from_rotation_z(battle.at_sea() * 38.));
                wanted.insert(
                    Actor(port.body.id, Shape::Propeller),
                    airframe.mul_transform(spin),
                );
            }
            let harbour = Harbour::for_factories(port.factories().len());
            for (index, dock) in port.docks().iter().enumerate() {
                let key = dock_key(port.body.id, index);
                let yard = Body {
                    pos: port.dock_position(index),
                    ..port.body
                };
                let state = dock.state();
                let yard_foot = pose(&yard, 3.2);
                wanted.insert(Actor(key, Shape::Yard(harbour, state)), yard_foot);
                if state != YardState::Ruined {
                    let hoist = yard_foot.transform_point(harbour_models::flag_hoist(state));
                    wanted.insert(
                        Actor(key, Shape::YardFlag),
                        Transform::from_translation(hoist),
                    );
                    let (order, _) = dock.order();
                    let progress = ((dock.progress() * 3.) as u8).min(2);
                    if harbour == Harbour::Foundry {
                        let center = yard_foot.transform_point(Vec3::new(0., 0., -4.)).xz();
                        let heave = sea.swell.height(center) * 0.36;
                        let lighter = Transform::from_translation(center.extend(heave).xzy())
                            .with_rotation(yard_foot.rotation);
                        wanted.insert(Actor(key, Shape::Lighter(progress)), lighter);
                        if state == YardState::Working && port.live_factories() > 0 {
                            let mut crosshead = yard_foot;
                            crosshead.translation =
                                yard_foot.transform_point(Vec3::new(0., 14.25, -3.8));
                            let drop = 6.6 + (run.elapsed * 0.64 + index as f32 * 2.1).sin();
                            wanted.insert(
                                Actor(key, Shape::PlateRigging),
                                crosshead.with_scale(Vec3::new(1., drop, 1.)),
                            );
                            crosshead.translation.y -= drop;
                            wanted.insert(Actor(key, Shape::SuspendedPlate), crosshead);
                        }
                    } else {
                        let scale = match order {
                            EnemyClass::Patrol => Vec3::new(0.68, 0.85, 0.63),
                            EnemyClass::Destroyer => Vec3::new(0.78, 0.95, 0.81),
                            EnemyClass::Cruiser => Vec3::new(0.94, 1.1, 0.96),
                            _ => Vec3::new(1.20, 1.35, 1.12),
                        };
                        wanted.insert(
                            Actor(key, Shape::Cradle(progress)),
                            pose(&yard, 3.84).with_scale(scale),
                        );
                    }
                }
                if matches!(state, YardState::Damaged | YardState::Burning) {
                    let t = (run.elapsed * 0.58 + index as f32 * 0.37).fract();
                    let mut xf = pose(&yard, dock.top() + t * 6.);
                    xf.translation.x += t * t * 3.;
                    wanted.insert(
                        Actor(key, Shape::Smoke),
                        xf.with_scale(Vec3::splat(0.6 + 1.8 * (t * std::f32::consts::PI).sin())),
                    );
                    if state == YardState::Burning {
                        wanted.insert(
                            Actor(key, Shape::GunFire(0)),
                            pose(&yard, 6.6).with_scale(Vec3::splat(2.1)),
                        );
                    }
                }
            }
            if harbour == Harbour::TwinMills && port.live_factories() == 2 {
                let (a, b) = (port.factory_position(0), port.factory_position(1));
                let center = a.midpoint(b).extend(3.2).xzy();
                let bridge = Transform::from_translation(center)
                    .with_rotation(pose(&port.body, 3.2).rotation)
                    .with_scale(Vec3::new(1., 1., a.distance(b)));
                wanted.insert(Actor(port.body.id, Shape::Conveyor), bridge);
            }
            for (index, factory) in port.factories().iter().enumerate() {
                let mill = Body {
                    pos: port.factory_position(index),
                    ..port.body
                };
                let i = index as u8;
                let live = factory.live();
                let foot = pose(&mill, 3.2);
                if index == 0
                    && battle
                        .air_station()
                        .is_some_and(|base| base.port == base_index)
                {
                    wanted.insert(Actor(port.body.id, Shape::Hangar(live)), foot);
                    continue;
                }
                wanted.insert(Actor(port.body.id, Shape::Factory(harbour, i, live)), foot);
                if live {
                    let chimney = foot.transform_point(harbour.chimney());
                    wanted.insert(
                        Actor(port.body.id, Shape::Exhaust(i)),
                        Transform::from_translation(chimney),
                    );
                }
            }
            wanted.insert(
                Actor(port.body.id, Shape::Coast(Coast::Yard(harbour))),
                pose(&port.body, 0.),
            );
            for (index, gun) in port.turrets().iter().enumerate() {
                let pos = gun.position(&port.body);
                let platform = Body {
                    pos,
                    heading: shipvivor_compass::heading(pos - port.body.pos),
                    ..port.body
                };
                let i = index as u8;
                // A heavy battery's gunhouse is drawn larger, on a wider apron.
                let (apron, scale) = (gun.radius() / 5., gun.mount_scale());
                wanted.insert(
                    Actor(port.body.id, Shape::GunPlatform(i)),
                    pose(&platform, 0.).with_scale(Vec3::new(apron, 1., apron)),
                );
                let mount = Body {
                    heading: gun.heading,
                    ..platform
                };
                let tubes = if gun.live() { gun.barrels } else { 0 };
                let house = pose(&mount, 4.).with_scale(Vec3::splat(scale));
                wanted.insert(Actor(port.body.id, Shape::ShoreGun(i, tubes)), house);
                // Each round that struck the gunhouse and left it standing shows where
                // it hit, the heavier the wider, so a damaged gun reads as one.
                for (k, pock) in gun.pocks().iter().enumerate().filter(|_| gun.live()) {
                    let toward = Vec3::new(pock.at.x, pock.at.y - 4., -pock.at.z) / scale;
                    // Rounds aimed at the middle of the shield strike about it, and
                    // clear of the mantlet that stands out from it round the barrels.
                    let spread = (k as f32 * 2.39).sin();
                    let swing = spread.signum() * (0.22 + 0.33 * spread.abs());
                    let toward = Quat::from_rotation_y(swing) * toward
                        + Vec3::Y * 0.55 * (k as f32 * 1.71).cos();
                    let (at, facing) = harbour_models::gunhouse_face(gun.barrels, toward);
                    // A main-battery round holes the plate; a light gun's only dents it.
                    let open = pock.calibre >= 3.5;
                    let size = (0.22 + 0.03 * pock.calibre) / scale;
                    let mark = Transform::from_translation(at + facing * 0.04)
                        .with_rotation(
                            Quat::from_rotation_arc(Vec3::Z, facing)
                                * Quat::from_rotation_z(k as f32 * 2.3),
                        )
                        .with_scale(Vec3::splat(size));
                    wanted.insert(
                        Actor(port.body.id, Shape::Pock(i, k as u8, open)),
                        house.mul_transform(mark),
                    );
                }
            }
        }
        for r in battle.raiders() {
            let p = r.class.profile();
            wanted.insert(Actor(r.body.id, Shape::Ship(r.class)), pose(&r.body, 0.));
            let ship = pose(&r.body, 0.);
            // Smoke streams from every funnel of a ship with steam up, and trails back
            // along her track to show which way she is going. Disabled, she burns
            // instead (below).
            if !r.is_disabled() {
                for (stack, &(lip, _)) in kit.fleet[&r.class].funnels.iter().enumerate() {
                    wanted.insert(
                        Actor(r.body.id, Shape::Stack(r.class, stack as u8)),
                        Transform::from_translation(ship.transform_point(lip)),
                    );
                }
            }
            // A torpedo boat at speed throws a bow wave either side and leaves a white
            // wash astern, so her dash reads at range.
            let pace = (r.motion().length() / p.speed()).min(1.);
            if r.class == EnemyClass::TorpedoBoat && pace > 0.25 {
                let bow = -p.hull.bow + 1.2;
                let wash = [
                    (
                        Vec3::new(-0.95, 0.1, bow + 1.6),
                        0.4,
                        Vec3::new(0.45, 0.55, 2.),
                    ),
                    (
                        Vec3::new(0.95, 0.1, bow + 1.6),
                        -0.4,
                        Vec3::new(0.45, 0.55, 2.),
                    ),
                    (
                        Vec3::new(0., 0.05, p.hull.stern + 4.),
                        0.,
                        Vec3::new(1.4, 0.14, 5.),
                    ),
                ];
                for (i, (at, splay, size)) in wash.into_iter().enumerate() {
                    let part = Transform::from_translation(at)
                        .with_rotation(Quat::from_rotation_y(splay))
                        .with_scale(size * pace);
                    wanted.insert(
                        Actor(r.body.id, Shape::Wash(i as u8)),
                        ship.mul_transform(part),
                    );
                }
            }
            for (i, scar) in r.scars().iter().enumerate() {
                let open = armor_fx::opened(scar.impact);
                let radius = scar.scale * if open { 1.34 } else { 0.90 };
                let surface = Transform::from_translation(scar.position + scar.normal * 0.08)
                    .with_rotation(Quat::from_rotation_arc(Vec3::Z, scar.normal))
                    .with_scale(Vec3::splat(radius));
                wanted.insert(
                    Actor(r.body.id, Shape::Scar(i as u8, open)),
                    ship.mul_transform(surface),
                );
            }
            if r.class == EnemyClass::Battleship {
                let height = p.deck + 2.48 * p.gun_scale;
                if r.director_lost {
                    let life = (run.elapsed * 0.65).fract();
                    let mut soot = pose(&r.body, height + life * 4.);
                    soot.translation.x += life * life * 2.;
                    soot.scale = Vec3::splat(0.3 + life * (1. - life) * 4.);
                    wanted.insert(Actor(r.body.id, Shape::Smoke), soot);
                } else {
                    wanted.insert(Actor(r.body.id, Shape::Director), pose(&r.body, height));
                }
            }
            for (i, gun) in r.secondaries.iter().enumerate() {
                let base = gun.station(&r.body, p);
                let mount = Body {
                    pos: Vec2::new(base.x, base.z),
                    heading: gun.bearing(&r.body),
                    ..r.body
                };
                wanted.insert(
                    Actor(
                        r.body.id,
                        if gun.live() {
                            Shape::SecondaryMount(i as u8)
                        } else {
                            Shape::GunFire((32 + i) as u8)
                        },
                    ),
                    pose(&mount, base.y).with_scale(Vec3::splat(0.72)),
                );
            }
            for (i, gun) in r.guns().iter().enumerate() {
                let i = i as u8;
                let base = gun.base(&r.body, p);
                let bearing = Body {
                    pos: Vec2::new(base.x, base.z),
                    heading: gun.bearing(&r.body),
                    ..r.body
                };
                let mut socket = pose(&bearing, p.deck + gun.riser() * 0.5 + 0.08);
                socket.scale =
                    Vec3::new(2.26 * p.gun_scale, gun.riser() + 0.24, 2.26 * p.gun_scale);
                wanted.insert(Actor(r.body.id, Shape::Socket(i)), socket);
                if gun.live() {
                    wanted.insert(
                        Actor(r.body.id, Shape::Gun(i, r.class, r.barrels())),
                        pose(&bearing, base.y).with_scale(Vec3::splat(p.gun_scale)),
                    );
                    if gun.charging() && !r.is_disabled() {
                        let pulse = 0.8 + 0.35 * (run.elapsed * 18.).sin();
                        wanted.insert(
                            Actor(r.body.id, Shape::Primer(i)),
                            pose(&bearing, gun.top(p) + 0.28)
                                .with_scale(Vec3::splat(pulse * p.gun_scale)),
                        );
                    }
                } else {
                    let flame =
                        p.gun_scale * (0.52 + 0.12 * (run.elapsed * 8. + f32::from(i)).sin());
                    wanted.insert(
                        Actor(r.body.id, Shape::GunFire(i)),
                        pose(&bearing, base.y + 0.45).with_scale(Vec3::splat(flame)),
                    );
                    let t = (run.elapsed * 0.58 + f32::from(i) * 0.43).fract();
                    let mut soot = pose(&bearing, base.y + p.gun_scale * (0.85 + 2.3 * t));
                    soot.translation.x += t * t * p.gun_scale * 1.4;
                    soot.scale =
                        Vec3::splat(p.gun_scale * (0.12 + 0.52 * (std::f32::consts::PI * t).sin()));
                    wanted.insert(Actor(r.body.id, Shape::Soot(i)), soot);
                }
            }
            if r.is_disabled() {
                let mut xf = pose(&r.body, p.deck + 0.9);
                xf.scale = Vec3::splat(p.gun_scale);
                xf.scale.y *= 1. + 0.16 * (run.elapsed * 9.).sin();
                wanted.insert(Actor(r.body.id, Shape::Fire), xf);
                for puff in 0..3 {
                    let life = (run.elapsed * 0.58 + f32::from(puff) / 3.).fract();
                    let mut cloud = pose(&r.body, p.deck + p.gun_scale * (1.6 + 5.8 * life));
                    cloud.translation.x += life * life * 3.;
                    cloud.translation.z += 1.4 * life;
                    let swell = 0.12 + (std::f32::consts::PI * life).sin() * (0.7 + 1.7 * life);
                    cloud.scale = Vec3::splat(swell * p.gun_scale);
                    wanted.insert(Actor(r.body.id, Shape::FunnelSmoke(puff)), cloud);
                }
            }
        }
        for b in battle.salvage() {
            let size = 0.50 + 0.13 * (f32::from(b.stores).sqrt() - 1.);
            let mut xf = pose(&b.body, 0.12 * (run.elapsed * 2.).sin());
            xf.rotate_y(run.elapsed * 0.6);
            xf.scale = Vec3::splat(size);
            wanted.insert(Actor(b.body.id, Shape::Cargo), xf);
            if b.hooked {
                let from = (helm.position - helm.forward() * (11. + run.added_length()))
                    .extend(2.6)
                    .xzy();
                let to = xf.transform_point(Vec3::Y * 1.5);
                let mid = (from + to) * 0.5 - Vec3::Y * 0.95;
                for (i, (a, z)) in [(from, mid), (mid, to)].into_iter().enumerate() {
                    let line = Transform::from_translation((a + z) * 0.5)
                        .looking_at(z, Vec3::Y)
                        .with_scale(Vec3::new(0.11, 0.11, a.distance(z)));
                    wanted.insert(Actor(b.body.id, Shape::Tow(i as u8)), line);
                }
            }
        }
        for fish in battle.torpedoes() {
            let shape = if fish.hostile() {
                Shape::EnemyTorpedo
            } else {
                Shape::Torpedo
            };
            wanted.insert(
                Actor(fish.body.id, shape),
                pose(&fish.body, sea.height(fish.body.pos) + 0.12),
            );
            if fish.hostile() {
                enemy_track(fish, &sea, &mut wanted);
            }
        }
        for s in battle.shots() {
            let shape = match s.battery {
                Battery::Main => Shape::MainShot,
                Battery::Secondary => Shape::CloseShot,
                Battery::AntiAir => Shape::Tracer,
                Battery::Raider | Battery::EnemySecondary | Battery::Aircraft => Shape::EnemyShot,
                Battery::Shore | Battery::Heavy => Shape::ShoreShell,
            };
            let size = if shape == Shape::Tracer {
                1.
            } else {
                s.diameter / crate::ordnance::SHELL_MESH_DIAMETER
            };
            let xf = Transform::from_translation(s.position())
                .looking_to(s.tangent(), Vec3::Y)
                .with_scale(Vec3::splat(size));
            wanted.insert(Actor(s.body.id, shape), xf);
        }
        for e in battle.effects() {
            let age = e.age;
            match e.kind {
                EffectKind::MountLost(class) => {
                    let t = age / e.kind.lifetime();
                    let mut xf = pose(&e.body, e.height + 12. * age - 14. * age * age);
                    let side = e.body.forward().perp() * age * 8.;
                    xf.translation += Vec3::new(side.x, 0., side.y);
                    xf.rotate_local_z(age * 4.7);
                    xf.rotate_local_x(age * 2.9);
                    xf.scale = Vec3::splat(class.profile().gun_scale * (1. - t * t));
                    let barrels = class.profile().barrels;
                    wanted.insert(Actor(e.body.id, Shape::Gun(0, class, barrels)), xf);
                }
                EffectKind::Armor {
                    outcome,
                    bore,
                    battery,
                } => armor_fx::draw(e, outcome, bore, battery, &mut wanted),
                EffectKind::Wreck(class) => {
                    let p = class.profile();
                    let mut xf = pose(&e.body, -age * age * 0.68);
                    xf.rotate_local_z(age * 0.42);
                    xf.rotate_local_x(age * 0.12);
                    wanted.insert(Actor(e.body.id, Shape::Ship(class)), xf);
                    if class.is_flagship() {
                        for (i, offset) in [-22., 15., -7., 26., 0.].into_iter().enumerate() {
                            let t = (age - i as f32 * 0.43) / 1.3;
                            if (0.0..1.).contains(&t) {
                                let point = xf.transform_point(Vec3::new(0., p.deck, -offset));
                                let size = 0.2 + 7. * (std::f32::consts::PI * t).sin();
                                wanted.insert(
                                    Actor(e.body.id, Shape::Blast(i as u8)),
                                    Transform::from_translation(point + Vec3::Y * t * 5.)
                                        .with_scale(Vec3::splat(size)),
                                );
                            }
                        }
                    }
                    if age < 1.3 {
                        for i in 0..4 {
                            let a = i as f32 * 1.7 + e.body.heading;
                            let mut shard = pose(&e.body, 2. + 10. * age - 9. * age * age);
                            shard.translation += Vec3::new(a.cos(), 0., a.sin()) * age * 7.;
                            shard.rotate_local_x(age * 4.);
                            wanted.insert(Actor(e.body.id, Shape::Debris(i)), shard);
                        }
                    }
                }
                EffectKind::Crunch => {
                    // White water thrown up between the hulls, and plating torn loose.
                    let t = age / e.kind.lifetime();
                    let lift = (std::f32::consts::PI * t.powf(0.63)).sin().max(0.);
                    let mut spray = pose(&e.body, 0.05);
                    spray.scale = Vec3::new(1.6 + 2.2 * t, 0.2 + 3.4 * lift, 1.6 + 2.2 * t);
                    wanted.insert(Actor(e.body.id, Shape::Splash), spray);
                    let mut ring = pose(&e.body, 0.06);
                    ring.scale = Vec3::new(2. + t * 7., 0.9 * (1. - t), 2. + t * 7.);
                    wanted.insert(Actor(e.body.id, Shape::FoamRing), ring);
                    for i in 0..4 {
                        let a = i as f32 * 1.7 + e.body.heading;
                        let mut shard = pose(&e.body, 1.5 + 9. * age - 11. * age * age);
                        shard.translation += Vec3::new(a.cos(), 0., a.sin()) * age * 6.;
                        shard.rotate_local_x(age * 5.);
                        wanted.insert(Actor(e.body.id, Shape::Debris(i)), shard);
                    }
                }
                EffectKind::Flak => {
                    // A quick flash, then a dark puff that blooms at once, spreads a little
                    // as it hangs, and thins away.
                    let t = age / e.kind.lifetime();
                    if age < 0.14 {
                        let mut flash = pose(&e.body, e.height);
                        flash.scale = Vec3::splat(1.5 * (1. - age / 0.14) + 0.05);
                        wanted.insert(Actor(e.body.id, Shape::Flash), flash);
                    }
                    let bloom = 1. - (-age * 16.).exp();
                    let thin = 1. - ((t - 0.62) / 0.38).clamp(0., 1.).powi(2);
                    let mut puff = pose(&e.body, e.height + 0.6 * t);
                    puff.rotate_local_y(e.body.id as f32);
                    puff.scale = Vec3::splat((1.9 * bloom + 0.9 * t) * thin + 0.01);
                    wanted.insert(Actor(e.body.id, Shape::FlakPuff(0)), puff);
                }
                EffectKind::Downed(frame) if e.height > 0. => {
                    // Nose dropping and rolling as she falls, burning, trailing black smoke.
                    let (shape, scale) = match frame {
                        Airframe::Spotter => (Shape::Spotter, spotter_model::ALOFT),
                        Airframe::Attack => (Shape::AttackPlane, STRAFER),
                        Airframe::Harrier => (Shape::AttackPlane, HARRIER),
                    };
                    let mut airframe = pose(&e.body, e.height).with_scale(Vec3::splat(scale));
                    let roll = if e.body.id % 2 == 0 { 1. } else { -1. };
                    airframe.rotate_local_x(-(0.12 + 0.75 * age).min(1.15));
                    airframe.rotate_local_z(roll * (0.4 + 2.1 * age));
                    wanted.insert(Actor(e.body.id, shape), airframe);
                    let mut fire = Transform::from_translation(
                        airframe.transform_point(Vec3::new(0., 0.6, -1.5)),
                    );
                    fire.scale = Vec3::splat(0.45 + 0.08 * (age * 23.).sin());
                    wanted.insert(Actor(e.body.id, Shape::Fire), fire);
                    let back = -e.body.forward();
                    for i in 0..5_u8 {
                        let k = f32::from(i) + 1.;
                        let behind = Vec3::new(back.x, 0., back.y) * (2.6 * k);
                        let lift = Vec3::Y * (0.5 + 0.7 * k * (0.3 + age));
                        let mut puff =
                            Transform::from_translation(airframe.translation + behind + lift);
                        puff.scale = Vec3::splat((0.55 + 0.32 * k) * age.min(0.5) * 2.);
                        wanted.insert(Actor(e.body.id, Shape::FlakPuff(i)), puff);
                    }
                }
                EffectKind::Downed(frame) => {
                    // Down on the water, the wreck lies over on a broken float, then
                    // settles by the nose and goes under.
                    let (shape, scale) = match frame {
                        Airframe::Spotter => (Shape::Spotter, spotter_model::ALOFT),
                        Airframe::Attack => (Shape::AttackPlane, STRAFER),
                        Airframe::Harrier => (Shape::AttackPlane, HARRIER),
                    };
                    let under = (1. - (e.kind.lifetime() - age) / SINKING).clamp(0., 1.);
                    let mut wreck = pose(&e.body, (AFLOAT - SUNK * under * under) * scale)
                        .with_scale(Vec3::splat(scale));
                    let roll = if e.body.id % 2 == 0 { 1. } else { -1. };
                    wreck.rotate_local_z(roll * 0.42);
                    wreck.rotate_local_x(-(0.15 + 0.5 * under));
                    wanted.insert(Actor(e.body.id, shape), wreck);
                }
                // The shore guns' flashes and smoke are drawn by `gunsmoke`.
                EffectKind::Muzzle(Battery::Shore | Battery::Heavy) => {}
                EffectKind::Muzzle(_) => {
                    let t = age / e.kind.lifetime();
                    let mut xf = pose(&e.body, e.height);
                    xf.scale = Vec3::splat((1. - t).powi(2) * 1.3);
                    wanted.insert(Actor(e.body.id, Shape::Muzzle), xf);
                    let mut vapor = pose(&e.body, e.height + t);
                    vapor.scale = Vec3::splat(0.5 + t * 1.35);
                    wanted.insert(Actor(e.body.id, Shape::Smoke), vapor);
                }
                // A heavy shell's column climbs fast to some 25 metres, three or four
                // times a splash, hangs, and falls back wide, so the fall of shot reads
                // from far off.
                EffectKind::Column => {
                    let t = age / e.kind.lifetime();
                    let lift = (std::f32::consts::PI * t.powf(0.45)).sin().max(0.);
                    let mut xf = pose(&e.body, 0.05);
                    xf.scale = Vec3::new(2.4 + 5. * t, 0.3 + 24. * lift, 2.4 + 5. * t);
                    wanted.insert(Actor(e.body.id, Shape::Splash), xf);
                    let mut ring = pose(&e.body, 0.06);
                    ring.scale = Vec3::new(2. + t * 12., 0.9 * (1. - t), 2. + t * 12.);
                    wanted.insert(Actor(e.body.id, Shape::FoamRing), ring);
                }
                EffectKind::Torpedoed => {
                    // A wide column of white water thrown up over her side, higher than
                    // her masts, with a little dark smoke from the warhead in it.
                    let t = age / e.kind.lifetime();
                    let lift = (std::f32::consts::PI * t.powf(0.5)).sin().max(0.);
                    let mut column = pose(&e.body, 0.05);
                    column.scale = Vec3::new(3.6 + 4. * t, 0.4 + 19. * lift, 3.6 + 4. * t);
                    wanted.insert(Actor(e.body.id, Shape::Splash), column);
                    let mut ring = pose(&e.body, 0.06);
                    ring.scale = Vec3::new(3. + t * 13., 1.2 * (1. - t), 3. + t * 13.);
                    wanted.insert(Actor(e.body.id, Shape::FoamRing), ring);
                    if age < 0.9 {
                        let mut smoke = pose(&e.body, 2. + age * 6.);
                        smoke.scale =
                            Vec3::splat(0.4 + 2.6 * (std::f32::consts::PI * age / 0.9).sin());
                        wanted.insert(Actor(e.body.id, Shape::Smoke), smoke);
                    }
                }
                EffectKind::Splash => {
                    let t = age / e.kind.lifetime();
                    let lift = (std::f32::consts::PI * t.powf(0.63)).sin().max(0.);
                    let mut xf = pose(&e.body, 0.05);
                    xf.scale = Vec3::new(1.1 + 2.7 * t, 0.2 + 6.7 * lift, 1.1 + 2.7 * t);
                    wanted.insert(Actor(e.body.id, Shape::Splash), xf);
                    let mut ring = pose(&e.body, 0.06);
                    ring.scale = Vec3::new(1. + t * 5., 0.9 * (1. - t), 1. + t * 5.);
                    wanted.insert(Actor(e.body.id, Shape::FoamRing), ring);
                }
                EffectKind::Shatter(radius) => {
                    // White water and brash burst out of the broken ice, as wide as it
                    // was, and lumps of it are flung up and fall back into the sea.
                    let r = f32::from(radius.max(1));
                    let t = age / e.kind.lifetime();
                    let lift = (std::f32::consts::PI * t.powf(0.5)).sin().max(0.);
                    let mut spray = pose(&e.body, 0.05);
                    let wide = 0.45 * r + 1.6 * r * t.sqrt();
                    spray.scale = Vec3::new(wide, 0.3 + (1.2 + 0.3 * r) * lift, wide);
                    wanted.insert(Actor(e.body.id, Shape::Splash), spray);
                    let mut ring = pose(&e.body, 0.06);
                    ring.scale =
                        Vec3::new(r * (0.9 + 1.6 * t), 1.1 * (1. - t), r * (0.9 + 1.6 * t));
                    wanted.insert(Actor(e.body.id, Shape::FoamRing), ring);
                    let (up, out) = (4. + 0.3 * r, 2.5 + 0.35 * r);
                    for i in 0..6_u8 {
                        let a = f32::from(i) * 1.05 + e.body.heading;
                        let rise = 0.4 * r + up * age - 4.9 * age * age;
                        if rise < 0. {
                            continue;
                        }
                        let mut shard = pose(&e.body, rise);
                        let reach = 0.5 * r + out * age;
                        shard.translation += Vec3::new(a.cos(), 0., a.sin()) * reach;
                        shard.rotate_local_x(age * (3. + f32::from(i)));
                        shard.rotate_local_z(age * 2.3);
                        shard.scale = Vec3::splat((0.12 + 0.02 * f32::from(i % 3)) * r);
                        wanted.insert(Actor(e.body.id, Shape::Shard(i)), shard);
                    }
                }
                EffectKind::Demolition { dock } => {
                    let size = if dock { 1.65 } else { 1. };
                    let t = age / e.kind.lifetime();
                    let origin = pose(&e.body, e.height);
                    if age < 0.58 {
                        let flash = (age * 10.).min(1.) * (1. - age / 0.58);
                        wanted.insert(
                            Actor(e.body.id, Shape::ImpactFlash),
                            origin.with_scale(Vec3::splat((1.8 + flash * 5.5) * size)),
                        );
                    }
                    for i in 0..7 {
                        let angle = f32::from(i) * 2.4 + e.body.heading;
                        let outward = Vec3::new(angle.cos(), 0., angle.sin());
                        let mut beam = origin;
                        let height =
                            (4. + (12. + f32::from(i % 3) * 3.) * age - 11. * age * age).max(0.32);
                        beam.translation +=
                            (outward * age.min(1.45) * 4.3 + Vec3::Y * height) * size;
                        beam.rotate_local_x(age * 3.9 + angle);
                        beam.rotate_local_z(age * 5. + angle);
                        beam.scale = Vec3::new(0.8, 0.8, 2.1) * size * (1. - t * t);
                        wanted.insert(Actor(e.body.id, Shape::Debris(i)), beam);
                    }
                    for i in 0..3 {
                        let angle = f32::from(i) * 2.1;
                        let radius = size * (0.6 + 4.2 * (t * std::f32::consts::PI).sin());
                        let mut dust = origin;
                        dust.translation += size
                            * Vec3::new(
                                angle.cos() * (1. + 4.7 * t) + t * t * 5.,
                                1. + t * (9. + f32::from(i)),
                                angle.sin() * (1. + 4. * t),
                            );
                        dust.scale = Vec3::splat(radius);
                        wanted.insert(Actor(e.body.id, Shape::Soot(i)), dust);
                    }
                }
                EffectKind::Burst => {
                    let t = age / e.kind.lifetime();
                    let mut xf = pose(&e.body, e.height);
                    xf.scale = Vec3::splat(2.5 * (1. - t * 2.8).max(0.) + 0.01);
                    wanted.insert(Actor(e.body.id, Shape::Flash), xf);
                    let mut smoke = pose(&e.body, e.height + t * 2.7);
                    smoke.scale = Vec3::splat(0.15 + 2.2 * (std::f32::consts::PI * t).sin());
                    wanted.insert(Actor(e.body.id, Shape::Smoke), smoke);
                }
            }
        }
    }
    // The poses above are drawn on flat water. Whatever floats is then carried
    // to where the sea has it, with everything mounted on a hull moving as one.
    if !wanted.is_empty() {
        let mut afloat = HashMap::new();
        let level = |at: Vec2| Transform::from_xyz(0., sea.height(at), 0.);
        for r in battle.raiders() {
            let hull = &r.class.profile().hull;
            afloat.insert(r.body.id, sea.lift(r.body.pos, r.body.heading, hull));
        }
        for b in battle.salvage() {
            afloat.insert(b.body.id, level(b.body.pos));
        }
        for e in battle.effects() {
            match e.kind {
                EffectKind::Wreck(class) => {
                    let hull = &class.profile().hull;
                    afloat.insert(e.body.id, sea.lift(e.body.pos, e.body.heading, hull));
                }
                EffectKind::Splash
                | EffectKind::Column
                | EffectKind::Crunch
                | EffectKind::Torpedoed
                | EffectKind::Shatter(_) => {
                    afloat.insert(e.body.id, level(e.body.pos));
                }
                EffectKind::Downed(_) if e.height == 0. => {
                    afloat.insert(e.body.id, level(e.body.pos));
                }
                _ => {}
            }
        }
        for (key, xf) in &mut wanted {
            if let Some(lift) = afloat.get(&key.0) {
                *xf = lift.mul_transform(*xf);
            }
        }
    }
    if let Some(lens) = lens {
        clear_lens(&mut wanted, lens.0, lens.1);
    }
    for (entity, key, mut xf) in &mut actors {
        if let Some(pose) = wanted.remove(key) {
            *xf = pose;
        } else {
            cmd.entity(entity).despawn();
        }
    }
    for (key, xf) in wanted {
        create(&mut cmd, &kit, key, xf);
    }
}
