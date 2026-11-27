//! Presentation of the current encounter. Actor ids keep meshes stable across fixed ticks.
#[path = "armor_fx.rs"]
mod armor_fx;
#[path = "foreshore.rs"]
mod foreshore;
#[path = "harbour_models.rs"]
mod harbour_models;
#[path = "harbour_signals.rs"]
mod harbour_signals;
#[path = "industrial_view.rs"]
mod industrial_view;
#[path = "mainland.rs"]
mod mainland;
#[path = "airfield_models.rs"]
mod airfield_models;
#[path = "warplane_model.rs"]
mod warplane_model;
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
use harbour_models::{Harbour, IndustrialPart};
pub(crate) use harbour_signals::{
    animate as animate_harbour_signals, dye as dye_ensigns, fly_pennant, stream_funnel_smoke,
};
pub(crate) use mainland::MainlandPlugin;
use shipvivor_encounter::{
    Airframe, Battery, Body, EffectKind, EnemyClass, FOOTING, FeatureKind, ISLAND_RADIUS,
    IndustrialYard, Landmark, LandmarkKind, YardState,
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
    Industrial(IndustrialYard, IndustrialPart, u8),
    IndustrialRock(u8),
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
    /// One of a capital ship's side strakes, by its index among hers.
    Strake(EnemyClass, u8),
    /// A ship's bridge and mast, sound, battered or wrecked: see `Raider::works_stage`.
    Upperworks(EnemyClass, u8),
    /// Smoke curling out of a fresh shell hole in a ship: the hole's place among her
    /// scars, and in the top bits, which of its wisps.
    Smoulder(u8),
    /// Fire glowing in a fresh shell hole, by the hole's place among her scars.
    Ember(u8),
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
    /// The glow in a shell's base, turned to the camera: the enemy's if set.
    ShotGlow(bool),
    /// A length of the thin line a shell leaves on its path, counted back from it.
    ShotGhost(bool, u8),
    /// A light AA round: all tracer, no visible body.
    Tracer,
    /// Dark smoke from a flak burst or a burning aircraft.
    FlakPuff(u8),
    Flash,
    Muzzle,
    TankLid,
    Smoke,
    FunnelSmoke(u8),
    FoamRing,
    Splash,
    Debris(u8),
    /// A lump of ice thrown up where a berg breaks.
    Shard(u8),
    Fire,
    /// An airfield's paving and markings, laid on its island.
    FieldGround,
    /// One of an airfield's concrete shelters, by its place in the row.
    Shelter(u8),
    AirHangar(bool),
    Tower(bool),
    Depot(bool),
    Truck,
    /// An airfield's aircraft, on the ground or in the air.
    Warplane(shipvivor_encounter::PlaneKind),
    /// A pine on a pine islet, by its place in the islet's plan.
    Pine(u8),
    /// A tier of a sea stack: the stack's place in its islet's plan, and the tier,
    /// counted from zero at its foot.
    StackTier(u8, u8),
    /// A lighthouse tower, its lantern lit while it stands.
    Lighthouse(bool),
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
    hull_paint: Handle<StandardMaterial>,
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
    industry: HashMap<(IndustrialYard, IndustrialPart), Handle<Mesh>>,
    industrial_rock: Handle<Mesh>,
    conveyor: Handle<Mesh>,
    signals: harbour_signals::SignalKit,
    cradles: [Handle<Mesh>; 3],
    lighters: [Handle<Mesh>; 3],
    rigging: Handle<Mesh>,
    plate: Handle<Mesh>,
    cube: Handle<Mesh>,
    shell: Handle<Mesh>,
    /// The tracer astern of a shell: her own gold, the enemy's flame.
    trails: [Handle<Mesh>; 2],
    /// The glow in a shell's base and the lengths of its ghost trail, in the same two
    /// colours.
    glows: [Handle<Mesh>; 2],
    ghosts: [[Handle<Mesh>; crate::ordnance::GHOST_LENGTHS]; 2],
    tracer: Handle<StandardMaterial>,
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
    quay: Handle<StandardMaterial>,
    steel: Handle<StandardMaterial>,
    dark: Handle<StandardMaterial>,
    red: Handle<StandardMaterial>,
    gold: Handle<StandardMaterial>,
    flame: Handle<StandardMaterial>,
    foam: Handle<StandardMaterial>,
    field_ground: Handle<Mesh>,
    shelter: Handle<Mesh>,
    air_hangars: [Handle<Mesh>; 2],
    towers: [Handle<Mesh>; 2],
    depots: [Handle<Mesh>; 2],
    truck: Handle<Mesh>,
    bomber: Handle<Mesh>,
    torpedo_plane: Handle<Mesh>,
    /// A pine islet's trees, and each tier of a stack islet's sea stacks, by their
    /// places in the islet's plan.
    pines: Vec<Handle<Mesh>>,
    stack_tiers: Vec<[Handle<Mesh>; 4]>,
}
struct VesselMeshes {
    deckwork: Handle<Mesh>,
    /// Her bridge and mast sound, battered and wrecked, for a class whose bridge isn't
    /// part of her deckwork.
    upperworks: Option<[Handle<Mesh>; 3]>,
    /// Her turret with one barrel, two, three and four.
    guns: [Handle<Mesh>; 4],
    /// Her side strakes, in the order the encounter counts them. A class that hangs
    /// none has none.
    strakes: Vec<Handle<Mesh>>,
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
    stock_ship: Single<(&Mesh3d, &MeshMaterial3d<StandardMaterial>), With<scene::Ship>>,
    mut cmd: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut paint: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let (stock_hull, hull_paint) = *stock_ship;
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
    // A shell's tracer takes its colour and its fade from the mesh.
    let tracer = paint.add(StandardMaterial {
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
                LandmarkKind::Airfield,
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
        hull: stock_hull.0.clone(),
        hull_paint: hull_paint.0.clone(),
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
        industry: industrial_view::meshes(&mut meshes),
        industrial_rock: meshes.add(foreshore::industrial_rock()),
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
                let bounds = (crate::fleet_models::superstructure(class).get_aabb())
                    .expect("deckwork has vertices");
                let upperworks =
                    [0, 1, 2].map(|stage| crate::fleet_models::upperworks(class, stage));
                (
                    class,
                    VesselMeshes {
                        masthead: bounds.max().y,
                        deckwork: meshes.add(crate::fleet_models::deckwork(class)),
                        upperworks: (upperworks.iter().all(Option::is_some))
                            .then(|| upperworks.map(|mesh| meshes.add(mesh.unwrap()))),
                        guns: [1, 2, 3, 4]
                            .map(|barrels| meshes.add(crate::fleet_models::gun(class, barrels))),
                        strakes: (0..2 * shipvivor_encounter::STRAKES_A_SIDE)
                            .filter(|_| class.plated())
                            .map(|i| meshes.add(crate::fleet_models::strake(class, i)))
                            .collect(),
                        funnels: crate::fleet_models::funnels(class),
                    },
                )
            })
            .collect(),
        cube: meshes.add(Cuboid::default()),
        shell: meshes.add(crate::ordnance::shell()),
        trails: TRACERS.map(|tint| meshes.add(crate::ordnance::trail(tint))),
        glows: TRACERS.map(|tint| meshes.add(crate::ordnance::glow(tint))),
        ghosts: TRACERS.map(|tint| {
            std::array::from_fn(|length| meshes.add(crate::ordnance::ghost(tint, length)))
        }),
        tracer,
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
        quay: material(151, 121, 89, false),
        steel: material(202, 201, 174, false),
        dark: material(49, 55, 62, false),
        red: material(211, 70, 50, true),
        gold: material(249, 189, 70, true),
        flame: material(255, 117, 37, true),
        foam: material(196, 240, 222, true),
        field_ground: meshes.add(airfield_models::ground()),
        shelter: meshes.add(airfield_models::shelter()),
        air_hangars: [false, true].map(|live| meshes.add(airfield_models::hangar(live))),
        towers: [false, true].map(|live| meshes.add(airfield_models::tower(live))),
        depots: [false, true].map(|live| meshes.add(airfield_models::depot(live))),
        truck: meshes.add(airfield_models::truck()),
        bomber: meshes.add(warplane_model::bomber()),
        torpedo_plane: meshes.add(warplane_model::torpedo_plane()),
        pines: (LandmarkKind::Pines.sites().iter().enumerate())
            .map(|(i, site)| meshes.add(foreshore::pine(site, i)))
            .collect(),
        stack_tiers: (LandmarkKind::Stacks.sites().iter().enumerate())
            .map(|(i, site)| [0, 1, 2, 3].map(|t| meshes.add(foreshore::stack_tier(site, i, t))))
            .collect(),
    });
}
/// A tracer's colour: her own rounds' gold, the enemy's flame.
const TRACERS: [Color; 2] = [
    Color::srgb(0.976, 0.741, 0.275),
    Color::srgb(1., 0.459, 0.145),
];
/// The glow in a shell's base, in shell widths across, and the least it is ever drawn:
/// a share of the view's height in radius, and as much again for each of these inches
/// of bore, so a far round is still a spark to follow and a heavy one a bigger spark.
const GLOW: f32 = 1.6;
const GLOW_FLOOR: (f32, f32) = (0.003, 14.);
/// A shell's ghost trail: the seconds of flight it hangs on, its radius at the head in
/// shell widths, and the least that radius is drawn, as a share of the view's height.
const GHOST_SECONDS: f32 = 0.3;
const GHOST: f32 = 0.16;
const GHOST_FLOOR: f32 = 0.0006;

/// The air station's strafer is the attack plane's airframe at this share of its size,
/// 5 m across. A harrier is drawn at the second share, a little over 3 m across.
const STRAFER: f32 = 0.315;
const HARRIER: f32 = 0.2;

/// Turns an airfield model's own frame, x along the runway and z out from the shore,
/// onto its island.
fn field_rotation(field: &shipvivor_encounter::Field) -> Quat {
    let along = Vec3::new(field.along.x, 0., field.along.y);
    let out = Vec3::new(field.seaward.x, 0., field.seaward.y);
    Quat::from_mat3(&Mat3::from_cols(along, Vec3::Y, out))
}

/// How large an airfield's aircraft is drawn.
fn warplane_scale(kind: shipvivor_encounter::PlaneKind) -> f32 {
    match kind {
        shipvivor_encounter::PlaneKind::Bomber => warplane_model::BOMBER,
        shipvivor_encounter::PlaneKind::Torpedo => warplane_model::TORPEDO_PLANE,
    }
}

/// Every airfield: its paving, shelters, hangars, tower and depot as they stand, its
/// trucks, the aircraft in its shelters and on its ground, and its aircraft in the air.
fn draw_airfields(
    wanted: &mut HashMap<Actor, Transform>,
    battle: &shipvivor_encounter::Battle,
    run: &Campaign,
) {
    use airfield_models::LEVEL;
    use shipvivor_encounter::HANGARS;
    use warplane_model::WHEELS;
    let spin = |rate: f32| {
        Transform::from_translation(spotter_model::HUB)
            .with_rotation(Quat::from_rotation_z(battle.at_sea() * rate))
    };
    for port in battle.ports() {
        let Some(field) = port.airfield() else {
            continue;
        };
        let id = port.body.id;
        let turn = field_rotation(&field.field);
        let at = |p: Vec2, y: f32| Transform::from_translation(p.extend(y).xzy()).with_rotation(turn);
        wanted.insert(Actor(id, Shape::FieldGround), at(field.field.centre, 0.));
        for (i, shelter) in field.shelters().iter().enumerate() {
            let p = field.shelter_position(i);
            wanted.insert(Actor(dock_key(id, 10 + i), Shape::Shelter(i as u8)), at(p, LEVEL));
            // An aircraft waits inside, its nose to the doors.
            if let Some(parked) = shelter.plane {
                let k = warplane_scale(parked.kind);
                let mut plane = at(p, LEVEL + WHEELS * k).with_scale(Vec3::splat(k));
                plane.rotate_local_y(std::f32::consts::PI);
                wanted.insert(Actor(dock_key(id, 40 + i), Shape::Warplane(parked.kind)), plane);
            }
        }
        for i in 0..HANGARS.len() {
            let shape = Shape::AirHangar(field.hangar_live(i));
            wanted.insert(Actor(dock_key(id, 20 + i), shape), at(field.hangar_position(i), LEVEL));
        }
        let tower = Shape::Tower(field.tower_live());
        wanted.insert(Actor(dock_key(id, 30), tower), at(field.tower_position(), LEVEL));
        let depot = Shape::Depot(field.depot_live());
        wanted.insert(Actor(dock_key(id, 31), depot), at(field.depot_position(), LEVEL));
        if !field.depot_live() {
            // The burnt-out tanks smoulder on.
            let t = (run.elapsed * 0.4).fract();
            let mut smoke = at(field.depot_position(), LEVEL + 2. + t * 9.);
            smoke.translation.x += t * t * 4.;
            smoke.scale = Vec3::splat(1. + 2.6 * (t * std::f32::consts::PI).sin());
            wanted.insert(Actor(dock_key(id, 32), Shape::Smoke), smoke);
        }
        for truck in field.trucks() {
            wanted.insert(Actor(truck.body.id, Shape::Truck), pose(&truck.body, LEVEL));
        }
        for taxi in field.taxiing() {
            let k = warplane_scale(taxi.kind);
            let airframe = pose(&taxi.body, LEVEL + WHEELS * k).with_scale(Vec3::splat(k));
            wanted.insert(Actor(taxi.body.id, Shape::Warplane(taxi.kind)), airframe);
            if !taxi.sheltered() {
                wanted.insert(Actor(taxi.body.id, Shape::Propeller), airframe.mul_transform(spin(40.)));
            }
        }
    }
    for plane in battle.warplanes() {
        let k = warplane_scale(plane.kind);
        let mut airframe = pose(&plane.body, plane.altitude).with_scale(Vec3::splat(k));
        airframe.rotate_local_z(plane.bank);
        wanted.insert(Actor(plane.body.id, Shape::Warplane(plane.kind)), airframe);
        wanted.insert(Actor(plane.body.id, Shape::Propeller), airframe.mul_transform(spin(44.)));
    }
}

/// The model a downed aircraft falls as, and its scale.
fn downed_shape(frame: Airframe) -> (Shape, f32) {
    match frame {
        Airframe::Spotter => (Shape::Spotter, spotter_model::ALOFT),
        Airframe::Attack => (Shape::AttackPlane, STRAFER),
        Airframe::Harrier => (Shape::AttackPlane, HARRIER),
        Airframe::Bomber | Airframe::TorpedoPlane => (Shape::AttackPlane, STRAFER * 1.15),
    }
}

/// Fuel going up on an airfield: a flash, then a ball of burning fuel that lifts into
/// the mushroom cloud its smoke rolls up into, with fire left burning on the ground.
/// The depot's is more than twice a truck's.
fn fuel_blast(
    wanted: &mut HashMap<Actor, Transform>,
    e: &shipvivor_encounter::Effect,
    age: f32,
    depot: bool,
) {
    let size = if depot { 2.3 } else { 1. };
    let t = (age / e.kind.lifetime()).clamp(0., 1.);
    let origin = pose(&e.body, e.height);
    let id = e.body.id;
    // The cap climbs fast at first, then slows as it cools.
    let rise = size * (1.5 + 13. * t.powf(0.55));
    if age < 0.5 {
        let flash = (age * 14.).min(1.) * (1. - age / 0.5);
        wanted.insert(
            Actor(id, Shape::ImpactFlash),
            origin.with_scale(Vec3::splat((2. + 6. * flash) * size)),
        );
    }
    if t < 0.4 {
        let glow = (age * 12.).min(1.) * (1. - t / 0.4);
        for i in 0..2_u8 {
            let mut ball = origin;
            ball.translation += Vec3::Y * (rise * 0.8 + f32::from(i) * 0.6 * size);
            let swell = size * (1.2 + 3.5 * t.sqrt()) * (0.6 + 0.4 * glow);
            ball.scale = Vec3::splat(swell * if i == 0 { 1. } else { 0.7 });
            wanted.insert(Actor(id, Shape::Blast(i)), ball);
        }
    }
    if t < 0.65 {
        let mut fire = origin;
        fire.scale = Vec3::splat(0.55 * size * (1. - t / 0.65) + 0.05 * (age * 21.).sin());
        wanted.insert(Actor(id, Shape::Fire), fire);
    }
    // Its smoke rolls up into a mushroom cloud in `gunsmoke`.
}
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

/// Seconds a fresh shell hole in a ship smoulders, and the share of that it glows.
const SMOULDER: f32 = 30.;
const GLOWING: f32 = 0.3;
/// Holes that smoulder on one ship at a time, the freshest first.
const SMOULDERING: usize = 8;

/// Metres across the middle of the mark a round leaves on a ship, from the scale the
/// encounter gives it (its calibre over six inches): about a third of a metre from a
/// four-inch round and a metre from an eighteen-inch, growing slower than the bore so
/// that heavy rounds don't paint over the whole of her side.
fn scar_radius(scale: f32) -> f32 {
    0.12 * (6. * scale).max(1.).powf(0.75)
}

/// A ship and all she carries as the fight has left her, posed by `ship`: her upperworks
/// at the stage they are battered to and smoking, every crater and hole in her plating,
/// the fresh holes smouldering, her strakes as far as each is shaken loose, and her
/// guns, the knocked-out ones burning. A ship going down is drawn the same, under her
/// wreck's `id`. `clock` is the battle's time, and `elapsed` the run's, for flicker.
fn draw_vessel(
    r: &shipvivor_encounter::Raider,
    id: u64,
    ship: Transform,
    clock: f32,
    elapsed: f32,
    kit: &Kit,
    wanted: &mut HashMap<Actor, Transform>,
) {
    use std::f32::consts::PI;
    let p = r.class.profile();
    let s = p.gun_scale;
    let local = |at: Vec3| ship.mul_transform(Transform::from_translation(at));
    // Smoke drifts downwind across the world, whichever way she lies.
    let drifting = |mut xf: Transform, drift: f32| {
        xf.translation.x += drift;
        xf
    };
    wanted.insert(Actor(id, Shape::Ship(r.class)), ship);
    let stage = r.works_stage();
    if kit.fleet[&r.class].upperworks.is_some() {
        wanted.insert(Actor(id, Shape::Upperworks(r.class, stage)), ship);
        // Smoke pours from her shot-up bridge, and fire licks round the wreck of it.
        let top = p.deck + s * if stage >= 2 { 0.9 } else { 1.7 };
        for puff in 0..stage {
            let life = (elapsed * 0.5 + f32::from(puff) * 0.5 + (id % 7) as f32 * 0.13).fract();
            let mut cloud = drifting(
                local(Vec3::new(0., top + s * 2.6 * life, 0.)),
                life * life * 2.4,
            );
            let swell = 0.2 + (PI * life).sin() * (0.5 + 0.9 * life);
            cloud.scale = Vec3::splat(s * swell * (0.6 + 0.4 * f32::from(stage)));
            wanted.insert(Actor(id, Shape::Soot(200 + puff)), cloud);
        }
        if stage >= 2 {
            let mut flame = local(Vec3::new(0.2 * s, top - 0.1, 0.));
            flame.scale = Vec3::splat(s * (0.42 + 0.1 * (elapsed * 9. + id as f32).sin()));
            wanted.insert(Actor(id, Shape::GunFire(200)), flame);
        }
    }
    // Craters where her plate held, holes where it gave, and smoke and fire out of the
    // freshest holes.
    let mut fresh = Vec::new();
    for (i, scar) in r.scars().iter().enumerate() {
        let open = armor_fx::opened(scar.impact);
        let radius = scar_radius(scar.scale);
        let surface = Transform::from_translation(scar.position + scar.normal * 0.08)
            .with_rotation(
                Quat::from_rotation_arc(Vec3::Z, scar.normal)
                    * Quat::from_rotation_z(i as f32 * 2.3),
            )
            .with_scale(Vec3::splat(radius));
        wanted.insert(
            Actor(id, Shape::Scar(i as u8, open)),
            ship.mul_transform(surface),
        );
        let age = clock - scar.at;
        if open && (0. ..SMOULDER).contains(&age) {
            fresh.push((i, age));
        }
    }
    fresh.sort_by(|a, b| a.1.total_cmp(&b.1));
    for &(i, age) in fresh.iter().take(SMOULDERING) {
        let scar = &r.scars()[i];
        let fade = 1. - age / SMOULDER;
        // A heavy shell's hole is wider, but no hole smokes like a funnel.
        let size = 1.6 * scar_radius(scar.scale).min(1.);
        let mouth = scar.position + scar.normal * 0.09;
        for wisp in 0..2_u8 {
            let life = (elapsed * 0.55 + f32::from(wisp) * 0.5 + i as f32 * 0.37).fract();
            let rise = scar.normal * size * (0.2 + 0.5 * life) + Vec3::Y * size * 2.2 * life;
            let mut puff = drifting(local(mouth + rise), life * life * 1.4);
            let swell = 0.06 + 0.26 * (PI * life).sin() * (0.4 + life);
            puff.scale = Vec3::splat(size * fade * swell);
            wanted.insert(Actor(id, Shape::Smoulder(i as u8 | wisp << 6)), puff);
        }
        // Fire glows deep in the hole for a while, flush with her side.
        if age < SMOULDER * GLOWING {
            let glow = 1. - age / (SMOULDER * GLOWING);
            let flicker = 0.85 + 0.2 * (elapsed * 13. + i as f32 * 1.9).sin();
            // Just proud of the black of the hole, inside its torn edges.
            let lift = 0.09 + 0.03 * scar_radius(scar.scale);
            let mut ember = ship.mul_transform(
                Transform::from_translation(scar.position + scar.normal * lift)
                    .with_rotation(Quat::from_rotation_arc(Vec3::Z, scar.normal)),
            );
            let across = scar_radius(scar.scale) * 0.3 * (0.4 + 0.6 * glow) * flicker;
            ember.scale = Vec3::new(across, across, 0.02);
            wanted.insert(Actor(id, Shape::Ember(i as u8)), ember);
        }
    }
    if r.class == EnemyClass::Battleship {
        let height = p.deck + 2.48 * s;
        // Her director stands over her bridge, and goes with it.
        if r.director_lost || stage > 0 {
            let life = (elapsed * 0.65).fract();
            let mut soot = drifting(local(Vec3::Y * (height + life * 4.)), life * life * 2.);
            soot.scale = Vec3::splat(0.3 + life * (1. - life) * 4.);
            wanted.insert(Actor(id, Shape::Smoke), soot);
        } else {
            wanted.insert(Actor(id, Shape::Director), local(Vec3::Y * height));
        }
    }
    // Her deck-edge armour, less any strake shot off her, each sagging out from her side
    // as it is shaken loose.
    let (foot, rise) = shipvivor_encounter::STRAKE_RISE;
    for (i, strake) in r.strakes().iter().enumerate().filter(|(_, s)| s.on()) {
        let loose = strake.loose();
        let out = strake.abeam.signum();
        let skew = if i % 2 == 0 { 1. } else { -1. };
        let middle = Vec3::new(
            strake.abeam,
            (foot + 0.5 * rise) * p.deck - 0.2 * loose,
            strake.aft,
        );
        let sag = Transform::from_translation(middle).with_rotation(
            Quat::from_rotation_z(-out * 0.34 * loose) * Quat::from_rotation_y(skew * 0.07 * loose),
        );
        wanted.insert(
            Actor(id, Shape::Strake(r.class, i as u8)),
            ship.mul_transform(sag),
        );
    }
    for (i, gun) in r.secondaries.iter().enumerate() {
        let at = gun.offset();
        let mount = Transform::from_xyz(at.x, p.deck, -at.y)
            .with_rotation(Quat::from_rotation_y(-gun.train()))
            .with_scale(Vec3::splat(0.72));
        wanted.insert(
            Actor(
                id,
                if gun.live() {
                    Shape::SecondaryMount(i as u8)
                } else {
                    Shape::GunFire((32 + i) as u8)
                },
            ),
            ship.mul_transform(mount),
        );
    }
    for (i, gun) in r.guns().iter().enumerate() {
        let i = i as u8;
        let (side, fore, riser) = gun.offset();
        let turn = Quat::from_rotation_y(-gun.train());
        let at =
            |y: f32| ship.mul_transform(Transform::from_xyz(side, y, -fore).with_rotation(turn));
        let base = p.deck + riser;
        let mut socket = at(p.deck + riser * 0.5 + 0.08);
        socket.scale = Vec3::new(2.26 * s, riser + 0.24, 2.26 * s);
        wanted.insert(Actor(id, Shape::Socket(i)), socket);
        if gun.live() {
            wanted.insert(
                Actor(id, Shape::Gun(i, r.class, r.barrels())),
                at(base).with_scale(Vec3::splat(s)),
            );
        } else {
            let flame = s * (0.52 + 0.12 * (elapsed * 8. + f32::from(i)).sin());
            wanted.insert(
                Actor(id, Shape::GunFire(i)),
                at(base + 0.45).with_scale(Vec3::splat(flame)),
            );
            let t = (elapsed * 0.58 + f32::from(i) * 0.43).fract();
            let mut soot = drifting(at(base + s * (0.85 + 2.3 * t)), t * t * s * 1.4);
            soot.scale = Vec3::splat(s * (0.12 + 0.52 * (PI * t).sin()));
            wanted.insert(Actor(id, Shape::Soot(i)), soot);
        }
    }
    if r.is_disabled() {
        let mut xf = local(Vec3::Y * (p.deck + 0.9));
        xf.scale = Vec3::splat(s);
        xf.scale.y *= 1. + 0.16 * (elapsed * 9.).sin();
        wanted.insert(Actor(id, Shape::Fire), xf);
        for puff in 0..3 {
            let life = (elapsed * 0.58 + f32::from(puff) / 3.).fract();
            let mut cloud = local(Vec3::Y * (p.deck + s * (1.6 + 5.8 * life)));
            cloud.translation.x += life * life * 3.;
            cloud.translation.z += 1.4 * life;
            let swell = 0.12 + (PI * life).sin() * (0.7 + 1.7 * life);
            cloud.scale = Vec3::splat(swell * s);
            wanted.insert(Actor(id, Shape::FunnelSmoke(puff)), cloud);
        }
    }
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
                Coast::Landmark(_) | Coast::IndustrialRock => {}
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
        Shape::Industrial(kind, shape, _) => {
            // Furnace throats and instrument lamps carry their own light in fog
            // and on the unlit side of the quay.
            let material = match shape {
                IndustrialPart::Heat(_) | IndustrialPart::GaugeLight => &kit.incandescent,
                _ => &kit.construction,
            };
            part(&kit.industry[&(kind, shape)], material, Vec3::ZERO, Vec3::ONE);
        }
        Shape::IndustrialRock(_) => part(&kit.industrial_rock, &kit.ground, Vec3::ZERO, Vec3::ONE),
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
                &kit.hull_paint,
                Vec3::ZERO,
                v(p.hull.radius / 2.75, p.deck / 1.55, p.hull.bow / 16.),
            );
            part(
                &kit.fleet[&class].deckwork,
                &kit.gunmetal,
                Vec3::ZERO,
                Vec3::ONE,
            );
            // Her colours stay hoisted as she goes down, unless they fly from her
            // upperworks and go with them.
            if !crate::fleet_models::colours_aloft(class) {
                for (hoist, scale) in crate::fleet_models::ensigns(class) {
                    let at = Transform::from_translation(hoist).with_scale(Vec3::splat(scale));
                    kit.signals.spawn_flag(cmd, root, Side::Enemy, at);
                }
            }
        }
        Shape::Upperworks(class, stage) => {
            if let Some(stages) = &kit.fleet[&class].upperworks {
                part(
                    &stages[usize::from(stage.min(2))],
                    &kit.gunmetal,
                    Vec3::ZERO,
                    Vec3::ONE,
                );
            }
            // Battered, her mast is down and her colours with it.
            if stage == 0 && crate::fleet_models::colours_aloft(class) {
                for (hoist, scale) in crate::fleet_models::ensigns(class) {
                    let at = Transform::from_translation(hoist).with_scale(Vec3::splat(scale));
                    kit.signals.spawn_flag(cmd, root, Side::Enemy, at);
                }
            }
        }
        Shape::Smoulder(_) => part(&kit.ball, &kit.smoke, Vec3::ZERO, Vec3::ONE),
        // A flat glowing disc deep in a hole, hotter at its heart.
        Shape::Ember(_) => {
            part(&kit.ball, &kit.flame, Vec3::ZERO, Vec3::ONE);
            part(&kit.ball, &kit.gold, Vec3::Z * 0.2, Vec3::new(0.5, 0.5, 1.));
        }
        Shape::Gun(_, class, barrels) => {
            let gun = &kit.fleet[&class].guns[usize::from(barrels.clamp(1, 4) - 1)];
            part(gun, &kit.gunmetal, Vec3::ZERO, Vec3::ONE);
            if class.is_flagship() {
                part(&kit.drum, &kit.gold, v(0., 1.36, 0.), v(1.18, 0.11, 1.18));
            }
        }
        Shape::Strake(class, i) => part(
            &kit.fleet[&class].strakes[usize::from(i)],
            &kit.gunmetal,
            Vec3::ZERO,
            Vec3::ONE,
        ),
        Shape::Socket(_) => part(&kit.drum, &kit.dark, Vec3::ZERO, Vec3::ONE),
        Shape::Primer(_) => part(&kit.ball, &kit.foam, Vec3::ZERO, v(0.5, 0.35, 0.5)),
        // Lit, so a crater's dish and a hole's torn edges catch the light.
        Shape::Scar(_, open) | Shape::Pock(_, _, open) => part(
            &kit.scars[usize::from(open)],
            &kit.gunmetal,
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
            let hostile = matches!(key.1, Shape::EnemyShot | Shape::ShoreShell);
            part(
                &kit.trails[usize::from(hostile)],
                &kit.tracer,
                Vec3::ZERO,
                Vec3::ONE,
            );
        }
        Shape::ShotGlow(hostile) => part(
            &kit.glows[usize::from(hostile)],
            &kit.tracer,
            Vec3::ZERO,
            Vec3::ONE,
        ),
        Shape::ShotGhost(hostile, length) => part(
            &kit.ghosts[usize::from(hostile)][usize::from(length)],
            &kit.tracer,
            Vec3::ZERO,
            Vec3::ONE,
        ),
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
        Shape::TankLid => {
            part(&kit.drum, &kit.gunmetal, Vec3::ZERO, v(11.8, 0.52, 11.8));
            part(&kit.drum, &kit.steel, v(0., 0.44, 0.), v(2.3, 0.80, 2.3));
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
        Shape::FieldGround => part(&kit.field_ground, &kit.construction, Vec3::ZERO, Vec3::ONE),
        Shape::Shelter(_) => part(&kit.shelter, &kit.construction, Vec3::ZERO, Vec3::ONE),
        Shape::AirHangar(live) => part(
            &kit.air_hangars[usize::from(live)],
            &kit.construction,
            Vec3::ZERO,
            Vec3::ONE,
        ),
        Shape::Tower(live) => part(
            &kit.towers[usize::from(live)],
            &kit.construction,
            Vec3::ZERO,
            Vec3::ONE,
        ),
        Shape::Depot(live) => part(
            &kit.depots[usize::from(live)],
            &kit.construction,
            Vec3::ZERO,
            Vec3::ONE,
        ),
        Shape::Truck => part(&kit.truck, &kit.construction, Vec3::ZERO, Vec3::ONE),
        Shape::Warplane(kind) => {
            let airframe = match kind {
                shipvivor_encounter::PlaneKind::Bomber => &kit.bomber,
                shipvivor_encounter::PlaneKind::Torpedo => &kit.torpedo_plane,
            };
            part(airframe, &kit.gunmetal, Vec3::ZERO, Vec3::ONE);
        }
        Shape::Pine(i) => part(
            &kit.pines[usize::from(i)],
            &kit.ground,
            Vec3::ZERO,
            Vec3::ONE,
        ),
        Shape::StackTier(stack, tier) => part(
            &kit.stack_tiers[usize::from(stack)][usize::from(tier)],
            &kit.ground,
            Vec3::ZERO,
            Vec3::ONE,
        ),
        // Drawn up from its foot. The lantern goes dark when the tower falls.
        Shape::Lighthouse(lit) => {
            part(&kit.drum, &kit.steel, v(0., 6., 0.), v(4.8, 12., 4.8));
            part(&kit.drum, &kit.red, v(0., 8., 0.), v(4.9, 2.6, 4.9));
            part(&kit.drum, &kit.dark, v(0., 12., 0.), v(6., 1., 6.));
            let lantern = if lit { &kit.gold } else { &kit.dark };
            part(&kit.drum, lantern, v(0., 13.6, 0.), v(3.7, 2.4, 3.7));
            part(&kit.plume, &kit.red, v(0., 15.6, 0.), v(5.6, 2., 5.6));
        }
    }
}
/// Seconds a felled pine takes to go over, and a broken lighthouse.
const PINE_FALL: f32 = 1.3;
const TOWER_FALL: f32 = 2.4;
/// Height of a lighthouse's foot above the sea.
const TOWER_FOOT: f32 = 3.;

/// What stands on a small island: its pines, the tiers left of its stacks and its
/// lighthouse. Whatever was brought down goes over the way it fell, slowly at first and
/// then faster, and lies where it came to rest.
fn draw_features(wanted: &mut HashMap<Actor, Transform>, island: &Landmark, now: f32) {
    let turn = Quat::from_rotation_y(-island.body.heading);
    for (i, feature) in island.features.iter().enumerate() {
        let key = |shape| Actor(island.body.id, shape);
        let foot = feature.pos.extend(0.).xzy();
        // The way it fell, and how far over it has gone, to `rest` radians at most.
        let over = |seconds: f32, rest: f32| {
            feature.fell.map(|(when, way)| {
                let t = ((now - when) / seconds).clamp(0., 1.);
                let way = way.extend(0.).xzy();
                let axis = Vec3::Y.cross(way).normalize();
                (way, Quat::from_axis_angle(axis, rest * t * t))
            })
        };
        let i = i as u8;
        match feature.site.kind {
            FeatureKind::Pine => {
                let tilt = over(PINE_FALL, 1.45).map_or(Quat::IDENTITY, |(_, tilt)| tilt);
                let root = Transform::from_translation(foot + Vec3::Y * FOOTING);
                wanted.insert(key(Shape::Pine(i)), root.with_rotation(tilt * turn));
            }
            FeatureKind::Stack => {
                for tier in 0..feature.tiers() {
                    let at = Transform::from_translation(foot).with_rotation(turn);
                    wanted.insert(key(Shape::StackTier(i, tier)), at);
                }
            }
            FeatureKind::Lighthouse => {
                let base = foot + Vec3::Y * TOWER_FOOT;
                let mut tower = Transform::from_translation(base).with_rotation(turn);
                // It goes over on the edge of its foot.
                if let Some((way, tilt)) = over(TOWER_FALL, 1.5) {
                    let pivot = base + way * 2.4;
                    tower.translation = pivot + tilt * (base - pivot);
                    tower.rotation = tilt * turn;
                }
                wanted.insert(key(Shape::Lighthouse(feature.standing())), tower);
            }
        }
    }
}
/// Ordinary yards, including the sea-plane station and factories at an airfield.
fn draw_factories(
    wanted: &mut HashMap<Actor, Transform>,
    port: &shipvivor_encounter::Port,
    harbour: Harbour,
    air_station: bool,
) {
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
        let mut foot = pose(&mill, 3.2);
        if let Some(field) = port.airfield() {
            foot.rotation = field_rotation(&field.field);
            foot.translation.y = airfield_models::LEVEL;
        }
        if index == 0 && air_station {
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
    // The camera, and the height of its view a metre off, to size what must stay seen.
    let eye = lens.as_ref().map(|lens| {
        let spread = match lens.1 {
            Projection::Perspective(lens) => 2. * (lens.fov * 0.5).tan(),
            _ => 1.,
        };
        (*lens.0, spread)
    });
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
            draw_features(&mut wanted, island, battle.at_sea());
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
        draw_airfields(&mut wanted, &battle, &run);
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
            // An airfield's harbour is a cannery's, its factories on the field beside it.
            let harbour = if port.airfield().is_some() {
                Harbour::Cannery
            } else {
                Harbour::for_factories(port.factories().len())
            };
            if let Some(kind) = port.industrial_yard() {
                industrial_view::draw(&mut wanted, port, kind, battle.at_sea());
            } else {
                let air_station = battle
                    .air_station()
                    .is_some_and(|base| base.port == base_index);
                draw_factories(&mut wanted, port, harbour, air_station);
            }
            for (index, dock) in port.docks().iter().enumerate() {
                let key = dock_key(port.body.id, index);
                let yard = Body {
                    pos: port.dock_position(index),
                    ..port.body
                };
                let state = dock.state();
                let yard_foot = pose(&yard, 3.2);
                let dockhouse = port.industrial_yard().filter(|_| index == 1);
                let shape = match dockhouse {
                    Some(kind) => {
                        Shape::Industrial(kind, IndustrialPart::DockHouse(state), index as u8)
                    }
                    None => Shape::Yard(harbour, state),
                };
                wanted.insert(Actor(key, shape), yard_foot);
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
                        if dockhouse.is_none()
                            && state == YardState::Working
                            && port.live_factories() > 0
                        {
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
                    pose(&platform, gun.elevation()).with_scale(Vec3::new(apron, 1., apron)),
                );
                let mount = Body {
                    heading: gun.heading,
                    ..platform
                };
                let tubes = if gun.strength() > 0 { gun.barrels } else { 0 };
                let height = 4. + gun.elevation();
                let mut house = pose(&mount, height).with_scale(Vec3::splat(scale));
                if gun.cliff_battery()
                    && let Some(kind) = port.industrial_yard()
                {
                    let frame = Body { pos, ..port.body };
                    let recess = industrial_view::draw_casemate(
                        &mut wanted,
                        kind,
                        &frame,
                        i,
                        gun.elevation(),
                        gun.deployment(),
                    );
                    house.translation += recess;
                    if gun.deployment() == 0. {
                        continue;
                    }
                }
                wanted.insert(Actor(port.body.id, Shape::ShoreGun(i, tubes)), house);
                // Each round that struck the gunhouse and left it standing shows where
                // it hit, the heavier the wider, so a damaged gun reads as one.
                for (k, pock) in gun.pocks().iter().enumerate().filter(|_| gun.live()) {
                    let toward = Vec3::new(pock.at.x, pock.at.y - height, -pock.at.z) / scale;
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
            let ship = pose(&r.body, 0.);
            draw_vessel(
                r,
                r.body.id,
                ship,
                battle.at_sea(),
                run.elapsed,
                &kit,
                &mut wanted,
            );
            // Smoke streams from every funnel of a ship with steam up, and trails back
            // along her track to show which way she is going. Disabled, she burns
            // instead (see `draw_vessel`).
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
            // A gun about to fire glows at its breech.
            for (i, gun) in r.guns().iter().enumerate() {
                if gun.charging() && !r.is_disabled() {
                    let (side, fore, _) = gun.offset();
                    let pulse = 0.8 + 0.35 * (run.elapsed * 18.).sin();
                    let glow = Transform::from_xyz(side, gun.top(p) + 0.28, -fore)
                        .with_scale(Vec3::splat(pulse * p.gun_scale));
                    wanted.insert(
                        Actor(r.body.id, Shape::Primer(i as u8)),
                        ship.mul_transform(glow),
                    );
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
            if shape == Shape::Tracer {
                continue;
            }
            // A shell is too small to follow far off, so its base glows and it leaves a
            // thin line hanging on its path. Neither is ever drawn under a few pixels.
            let hostile = matches!(shape, Shape::EnemyShot | Shape::ShoreShell);
            let base = xf.translation - s.tangent() * s.diameter;
            let (facing, toward, view) = eye.map_or((Quat::IDENTITY, Vec3::ZERO, 0.), |eye| {
                let to_eye = eye.0.translation - base;
                (
                    eye.0.rotation,
                    to_eye.normalize_or_zero(),
                    to_eye.length() * eye.1,
                )
            });
            let floor = GLOW_FLOOR.0 * (1. + s.calibre() / GLOW_FLOOR.1);
            // The glow stands just clear of the shell on the eye's side, so a round
            // coming straight in doesn't hide its own spark.
            wanted.insert(
                Actor(s.body.id, Shape::ShotGlow(hostile)),
                Transform::from_translation(base + toward * s.diameter * 2.5)
                    .with_rotation(facing)
                    .with_scale(Vec3::splat((s.diameter * GLOW).max(view * floor))),
            );
            let lengths = crate::ordnance::GHOST_LENGTHS;
            let mut head = base;
            for i in 0..lengths {
                let tail = s.wake(GHOST_SECONDS, (i + 1) as f32 / lengths as f32);
                let run = head.distance(tail);
                // Each length keeps its own least width, so the near end of a trail
                // running away from the eye is no fatter than the far end.
                let view = eye.map_or(0., |eye| eye.0.translation.distance(head) * eye.1);
                let width = (s.diameter * GHOST).max(view * GHOST_FLOOR);
                if run > 0.5 {
                    wanted.insert(
                        Actor(s.body.id, Shape::ShotGhost(hostile, i as u8)),
                        Transform::from_translation(head)
                            .looking_at(tail, Vec3::Y)
                            .with_scale(Vec3::new(width, width, run)),
                    );
                }
                head = tail;
            }
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
                EffectKind::StrakeLost(class, i) => {
                    // The plate is thrown up and out from her side, tumbling, and drops
                    // into the sea.
                    let outward = class.strake_station(usize::from(i)).0.signum();
                    let mut xf = pose(&e.body, e.height + 5.5 * age - 6.5 * age * age);
                    let side = e.body.forward().perp() * outward * age * 4.5;
                    xf.translation += Vec3::new(side.x, 0., side.y);
                    xf.rotate_local_z(outward * age * 3.1);
                    xf.rotate_local_x(age * 1.7);
                    wanted.insert(Actor(e.body.id, Shape::Strake(class, i)), xf);
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
                    // She goes down as the fight left her.
                    if let Some(r) = battle.sinking(e.body.id) {
                        draw_vessel(
                            r,
                            e.body.id,
                            xf,
                            battle.at_sea(),
                            run.elapsed,
                            &kit,
                            &mut wanted,
                        );
                    } else {
                        wanted.insert(Actor(e.body.id, Shape::Ship(class)), xf);
                        if kit.fleet[&class].upperworks.is_some() {
                            wanted.insert(Actor(e.body.id, Shape::Upperworks(class, 0)), xf);
                        }
                    }
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
                    let (shape, scale) = downed_shape(frame);
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
                    let (shape, scale) = downed_shape(frame);
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
                EffectKind::FuelBlast { depot } => {
                    fuel_blast(&mut wanted, e, age, depot);
                    if depot {
                        industrial_view::draw_tank_lid(&mut wanted, battle.ports(), e);
                    }
                }
                EffectKind::Demolition { .. } | EffectKind::Felled(_) | EffectKind::Toppled(_) => {
                    // A hangar comes down nearly as hard as a dock, a tower lighter, and a
                    // tier of rock off a sea stack lighter still.
                    let size = match e.kind {
                        EffectKind::Demolition { dock: true } => 1.65,
                        EffectKind::Felled(shipvivor_encounter::FieldWork::Hangar) => 1.4,
                        EffectKind::Felled(_) => 0.8,
                        EffectKind::Toppled(FeatureKind::Lighthouse) => 0.9,
                        EffectKind::Toppled(_) => 0.7,
                        _ => 1.,
                    };
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
