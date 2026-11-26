mod bridge;
mod hull;
mod paint;
pub(crate) mod spray;
use crate::seascape::Sea;
use crate::state::{Campaign, Encounter, PlayerHelm, TrialCourse};
use bevy::{asset::RenderAssetUsages, prelude::*, render::render_resource::PrimitiveTopology};
pub(crate) use hull::deck_height;
use hull::extended_hull_mesh;
use shipvivor_campaign::DockModule;
use shipvivor_encounter::{AaMount, AntiAir, DeckOffsets, Hull, Rack, Station, TurretLayout};
use shipvivor_navigation::Helm;

#[derive(Component)]
pub struct Ship;

#[derive(Component)]
pub(crate) struct Masthead;

#[derive(Clone, Copy)]
enum Anchor {
    Hull,
    Bridge,
    Machinery,
}
#[derive(Component)]
pub struct DeckFitting {
    x: f32,
    z: f32,
    anchor: Anchor,
}
impl DeckFitting {
    /// Fitted along the hull itself, rather than on the bridge or the machinery.
    pub(crate) fn on_hull(&self) -> bool {
        matches!(self.anchor, Anchor::Hull)
    }
}
#[derive(Component)]
pub struct SectionModel(DockModule, usize);
impl SectionModel {
    /// The module of `design` this model stands on, if it is aboard.
    pub(crate) fn place(&self, design: &shipvivor_shipyard::Design) -> Option<usize> {
        let kind = self.0.build();
        design
            .modules()
            .iter()
            .enumerate()
            .filter(|(_, module)| module.same_kind(&kind))
            .nth(self.1)
            .map(|(i, _)| i)
    }
}
/// Gear on a yard section that keeps its own proportions when the beam is widened.
#[derive(Component)]
pub struct Unscaled;

pub fn refit_hull(
    run: Res<Campaign>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut ship: Single<&mut Mesh3d, With<Ship>>,
    mut fittings: Query<(&DeckFitting, &mut Transform), Without<SectionModel>>,
    mut sections: Query<(
        &SectionModel,
        &mut Transform,
        &mut Visibility,
        Has<Unscaled>,
    )>,
    mut previous: Local<(
        Option<std::sync::Arc<shipvivor_shipyard::Design>>,
        Option<u32>,
        Option<Hull>,
    )>,
) {
    let installed = run.ship.design();
    // Orders in the yard refit her in place, so each counts as a new design.
    let preview = Some(run.dock_revision);
    if previous.1 == preview
        && previous
            .0
            .as_ref()
            .is_some_and(|old| std::sync::Arc::ptr_eq(old, installed))
    {
        return;
    }
    let design = run.display_design();
    let hull = Hull::breakwater(&design);
    if previous.2 != Some(hull) {
        **ship = Mesh3d(meshes.add(extended_hull_mesh(hull)));
    }
    *previous = (Some(installed.clone()), preview, Some(hull));
    let offsets = DeckOffsets::for_design(&design);
    for (fitting, mut pose) in &mut fittings {
        pose.translation.z = match fitting.anchor {
            Anchor::Bridge => fitting.z + offsets.bridge_z,
            Anchor::Machinery => fitting.z + offsets.machinery_z,
            Anchor::Hull => Hull::deck_z(fitting.z, hull.extension()),
        };
        if !matches!(fitting.anchor, Anchor::Bridge) {
            pose.translation.x = fitting.x * offsets.beam_scale;
        }
    }
    for (model, mut pose, mut shown, unscaled) in &mut sections {
        *shown = if let Some(i) = model.place(&design) {
            let (fore, aft) = design.span(i);
            pose.translation.z = (fore + aft) * 0.5 - Hull::CRUISER.bow;
            pose.scale.x = if unscaled { 1. } else { offsets.beam_scale };
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

#[derive(Component)]
pub struct Turret(Station);
impl Turret {
    /// Its place in the design's list of gun stations.
    pub(crate) fn index(&self) -> usize {
        self.0.index()
    }
}
/// The barbette a raised turret stands on.
#[derive(Component)]
pub struct Barbette(Station);

#[derive(Component)]
pub struct DeckRack(Rack);
pub fn place_racks(
    run: Res<Campaign>,
    helm: Res<PlayerHelm>,
    view: Res<crate::camera::ShipView>,
    guns: Res<crate::state::Guns>,
    mut racks: Query<(&DeckRack, &mut Transform)>,
) {
    let trained = Rack::for_aim(helm.heading, view.bearing);
    let deck = DeckOffsets::for_design(&run.display_design());
    for (rack, mut transform) in &mut racks {
        let relative = if guns.torpedo_mode && trained == Some(rack.0) {
            view.bearing - helm.heading
        } else {
            rack.0.center(0.)
        };
        transform.translation.z = 1.9 + deck.machinery_z;
        transform.translation.x = if rack.0 == Rack::Port { -1.95 } else { 1.95 } * deck.beam_scale;
        transform.rotation = Quat::from_rotation_y(-relative);
    }
}

#[derive(Component)]
pub struct WingPosition(usize);
impl WingPosition {
    pub(crate) fn index(&self) -> usize {
        self.0
    }
}

/// A wing gun barrel shown when the fitted mounts carry this many barrels.
#[derive(Component)]
pub struct WingBarrel(usize);

#[derive(Component)]
pub struct GunTubes {
    station: Station,
    tube: usize,
}

pub fn place_turrets(
    battle: Res<Encounter>,
    run: Res<Campaign>,
    mut mounts: Query<(&Turret, &mut Transform, &mut Visibility), Without<Barbette>>,
    mut barbettes: Query<(&Barbette, &mut Transform, &mut Visibility), Without<Turret>>,
) {
    let yard = matches!(run.screen(), crate::state::Screen::Drydock(_));
    let preview = yard.then(|| run.display_design().gun_stations());
    let fittings = run.display_fittings();
    for (turret, mut pose, mut shown) in &mut mounts {
        let i = turret.0.index();
        let station = if let Some(positions) = &preview {
            positions.get(i).map(|pos| (*pos, pos.home))
        } else {
            battle
                .main_mounts()
                .get(i)
                .map(|mount| (mount.position, mount.deck_bearing()))
        };
        let Some((position, bearing)) = station else {
            *shown = Visibility::Hidden;
            continue;
        };
        *shown = Visibility::Inherited;
        pose.translation.x = position.abeam;
        pose.translation.y = TURRET_DECK + position.lift();
        pose.translation.z = -position.fore;
        pose.scale = Vec3::splat(fittings.for_station(&position).round().mount_scale());
        pose.rotation = Quat::from_rotation_y(-bearing);
    }
    // A raised turret stands on a drum as high as its lift.
    for (barbette, mut pose, mut shown) in &mut barbettes {
        let i = barbette.0.index();
        let station = match &preview {
            Some(positions) => positions.get(i).copied(),
            None => battle.main_mounts().get(i).map(|mount| mount.position),
        };
        let Some(position) = station.filter(|p| p.lift() > 0.) else {
            shown.set_if_neq(Visibility::Hidden);
            continue;
        };
        shown.set_if_neq(Visibility::Inherited);
        let width = 2.5 * fittings.for_station(&position).round().mount_scale();
        let foot = TURRET_DECK - 0.3;
        let height = position.lift() + 0.3;
        *pose = Transform::from_xyz(position.abeam, foot + 0.5 * height, -position.fore)
            .with_scale(Vec3::new(width, height, width));
    }
}

/// Height of the deck a main turret's ring stands on.
const TURRET_DECK: f32 = 1.72;

/// Every wing gun station has a model. Only the guns fitted are shown.
pub fn place_wing_guns(
    run: Res<Campaign>,
    battle: Res<Encounter>,
    mut guns: Query<(&WingPosition, &mut Transform, &mut Visibility)>,
    mut barrels: Query<(&WingBarrel, &mut Visibility), Without<WingPosition>>,
) {
    let yard = matches!(run.screen(), crate::state::Screen::Drydock(_));
    let secondaries = run.display_fittings().secondaries();
    let deck = yard.then(|| DeckOffsets::for_design(&run.display_design()));
    for (gun, mut pose, mut shown) in &mut guns {
        if let Some(deck) = deck {
            let (x, fore) = deck.wing_position(gun.0);
            *shown = if gun.0 < secondaries.per_side * 2 {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            pose.translation.z = -fore;
            pose.translation.x = x;
            let home = shipvivor_encounter::WingGun::home(gun.0);
            pose.rotation = Quat::from_rotation_y(-home);
            continue;
        }
        let Some(fitted) = battle.wing_guns().get(gun.0) else {
            shown.set_if_neq(Visibility::Hidden);
            continue;
        };
        shown.set_if_neq(Visibility::Inherited);
        pose.translation.z = -fitted.deck_fore();
        pose.translation.x = fitted.deck_x();
        pose.rotation = Quat::from_rotation_y(-fitted.deck_bearing());
    }
    let fitted = if yard {
        secondaries.barrels
    } else {
        battle.secondaries().barrels
    };
    for (barrel, mut shown) in &mut barrels {
        shown.set_if_neq(if barrel.0 == fitted {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
}

/// A light AA mount: the tub it stands in, what trains, and what elevates.
#[derive(Component)]
pub struct AaPosition(usize);
impl AaPosition {
    pub(crate) fn index(&self) -> usize {
        self.0
    }
}
#[derive(Component)]
pub struct AaTrain(usize);
#[derive(Component)]
pub struct AaCradle(usize);
/// Height of an AA tub's floor: the bridge wing deck, with the others raised to match.
const AA_FLOOR: f32 = 3.04;

/// Every AA station has a model. Only the mounts fitted are shown, trained and laid
/// as the crews have them at sea, and turned outboard in the dock.
#[allow(clippy::type_complexity)]
pub fn place_aa_guns(
    run: Res<Campaign>,
    battle: Res<Encounter>,
    mut tubs: Query<(&AaPosition, &mut Transform, &mut Visibility)>,
    mut trains: Query<(&AaTrain, &mut Transform), (Without<AaPosition>, Without<AaCradle>)>,
    mut cradles: Query<(&AaCradle, &mut Transform), (Without<AaPosition>, Without<AaTrain>)>,
) {
    let yard = matches!(run.screen(), crate::state::Screen::Drydock(_));
    let deck = yard.then(|| {
        (
            DeckOffsets::for_design(&run.display_design()),
            run.display_fittings().anti_air().mounts,
        )
    });
    // Where each mount stands, where it points from the bow, and how high it is laid.
    let lay = |i: usize| -> Option<(f32, f32, f32, f32)> {
        if let Some((deck, fitted)) = deck {
            let (x, fore) = deck.aa_position(i);
            (i < fitted).then(|| (x, fore, x.signum() * 1.1, 0.5))
        } else {
            battle
                .aa_mounts()
                .get(i)
                .map(|m| (m.deck_x(), m.deck_fore(), m.deck_bearing(), m.elevation()))
        }
    };
    for (tub, mut pose, mut shown) in &mut tubs {
        let Some((x, fore, _, _)) = lay(tub.0) else {
            shown.set_if_neq(Visibility::Hidden);
            continue;
        };
        shown.set_if_neq(Visibility::Inherited);
        pose.translation.x = x;
        pose.translation.z = -fore;
    }
    for (train, mut pose) in &mut trains {
        if let Some((_, _, bearing, _)) = lay(train.0) {
            pose.rotation = Quat::from_rotation_y(-bearing);
        }
    }
    for (cradle, mut pose) in &mut cradles {
        if let Some((_, _, _, elevation)) = lay(cradle.0) {
            pose.rotation = Quat::from_rotation_x(elevation.clamp(-0.1, 1.45));
        }
    }
}

pub fn animate_gun_tubes(
    run: Res<Campaign>,
    battle: Res<Encounter>,
    mut barrels: Query<(&GunTubes, &mut Transform, &mut Visibility)>,
) {
    let preview = matches!(run.screen(), crate::state::Screen::Drydock(_))
        .then(|| (run.display_design().gun_stations(), run.display_fittings()));
    for (tube, mut pose, mut visibility) in &mut barrels {
        let i = tube.station.index();
        let state = if let Some((stations, fittings)) = &preview {
            stations.get(i).map(|s| {
                let local = fittings.for_station(s);
                (local.turrets, local.round(), 0., 0.)
            })
        } else {
            battle.main_mounts().get(i).map(|m| {
                (
                    m.layout(),
                    m.round(run.fittings.round()),
                    m.recoil(tube.tube),
                    m.pitch(),
                )
            })
        };
        let Some((layout, shell, recoil, pitch)) =
            state.filter(|(l, _, _, _)| tube.tube < l.count())
        else {
            *visibility = Visibility::Hidden;
            continue;
        };
        *visibility = Visibility::Inherited;
        let barrel_width = shell.scale() / shell.mount_scale();
        pose.scale = Vec3::new(barrel_width, barrel_width, 1.);
        pose.translation.x = layout.barrel_x(tube.tube);
        pose.translation.z = 0.64 * recoil;
        pose.rotation = Quat::from_rotation_x(pitch);
    }
}

#[derive(Component)]
pub struct Horizon;

#[derive(Component)]
pub struct Signal(usize);

#[derive(Component)]
pub struct Buoy;

pub fn setup(
    mut commands: Commands,
    course: Res<TrialCourse>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    commands.insert_resource(GlobalAmbientLight {
        brightness: 260.,
        ..default()
    });
    let cube = meshes.add(Cuboid::default());
    let drum = meshes.add(Cylinder::new(0.5, 1.0).mesh().resolution(8));
    let mut paint = |r, g, b| materials.add(Color::srgb_u8(r, g, b));
    let steel = paint(108, 134, 142);
    let pale = paint(179, 196, 190);
    let iron = paint(47, 65, 73);
    let brass = paint(234, 170, 66);
    let ivory = paint(222, 226, 192);
    let red = paint(213, 82, 62);
    let green = paint(61, 177, 151);

    spray::spawn(&mut commands, &mut meshes, &mut materials);
    let gear = crate::strike_craft::Parts::new(&mut meshes, &mut materials);
    let ship = commands
        .spawn((
            Ship,
            Mesh3d(meshes.add(hull_mesh())),
            MeshMaterial3d(materials.add(paint::hull_material(&mut images))),
            Transform::default(),
        ))
        .id();
    let mut fitting =
        |anchor, mesh: &Handle<Mesh>, material: &Handle<StandardMaterial>, x, y, z, w, h, l| {
            commands.spawn((
                DeckFitting { x, z, anchor },
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_xyz(x, y, z).with_scale(Vec3::new(w, h, l)),
                ChildOf(ship),
            ));
        };
    let (house, glass) = bridge::wheelhouse();
    for (mesh, roughness, metallic) in [(house, 0.57, 0.08), (glass, 0.19, 0.22)] {
        let material = materials.add(StandardMaterial {
            perceptual_roughness: roughness,
            metallic,
            ..default()
        });
        fitting(
            Anchor::Bridge,
            &meshes.add(mesh),
            &material,
            0.,
            0.,
            0.,
            1.,
            1.,
            1.,
        );
    }
    let funnel = meshes.add(Cylinder::new(0.5, 1.).mesh().resolution(24));
    for z in [0.5, 2.75] {
        fitting(
            Anchor::Machinery,
            &funnel,
            &steel,
            0.,
            2.45,
            z,
            1.75,
            2.0,
            2.3,
        );
        fitting(
            Anchor::Machinery,
            &funnel,
            &brass,
            0.,
            3.72,
            z,
            1.75,
            0.65,
            2.3,
        );
        fitting(
            Anchor::Machinery,
            &funnel,
            &iron,
            0.,
            4.28,
            z,
            1.85,
            0.55,
            2.4,
        );
    }
    // Paired covered launches alongside the aft stack.
    for x in [-1.9, 1.9] {
        fitting(
            Anchor::Machinery,
            &drum,
            &ivory,
            x,
            1.83,
            2.9,
            0.9,
            0.45,
            3.5,
        );
    }
    // Deck fittings and rail stanchions keep the silhouette readable in the gunsight.
    for side in [-1., 1.] {
        for z in [-7., -4.5, -2., 0.5, 3., 5.5, 8.] {
            fitting(
                Anchor::Hull,
                &cube,
                &pale,
                side * 2.55,
                1.95,
                z,
                0.09,
                0.8,
                0.10,
            );
        }
        for y in [1.98, 2.32] {
            // Two lengths of rail meet at the splice on the stock ship.
            fitting(
                Anchor::Hull,
                &cube,
                &pale,
                side * 2.55,
                y,
                -1.55,
                0.07,
                0.07,
                11.1,
            );
            fitting(
                Anchor::Hull,
                &cube,
                &pale,
                side * 2.55,
                y,
                6.05,
                0.07,
                0.07,
                4.1,
            );
        }
        fitting(
            Anchor::Hull,
            &drum,
            &iron,
            side * 1.2,
            hull::deck_height(-11.6) + 0.30,
            -11.6,
            0.45,
            0.65,
            0.45,
        );
        fitting(
            Anchor::Hull,
            &cube,
            &steel,
            side * 1.38,
            2.06,
            6.8,
            0.72,
            0.9,
            1.5,
        );
    }
    fitting(
        Anchor::Hull,
        &drum,
        &iron,
        0.,
        hull::deck_height(-12.5) + 0.19,
        -12.5,
        1.15,
        0.4,
        1.15,
    );
    fitting(Anchor::Hull, &cube, &pale, 0., 1.8, 12., 1.4, 0.35, 1.1);
    // Compact deck-edge tubes on both sides of the aft funnel.
    for side in [-1., 1.] {
        fitting(
            Anchor::Machinery,
            &cube,
            &iron,
            side * 1.88,
            1.72,
            1.9,
            1.4,
            0.32,
            1.8,
        );
    }
    commands.spawn((
        Masthead,
        DeckFitting {
            anchor: Anchor::Bridge,
            x: 0.,
            z: -0.85,
        },
        // Scaled alike along and across the cloth, so the flag keeps the proportions its
        // colours are laid out in.
        Transform::from_xyz(0., 8.09, -0.85).with_scale(Vec3::new(0.56, 0.56, 0.47)),
        Mesh3d::default(),
        MeshMaterial3d::<StandardMaterial>::default(),
        bevy::light::NotShadowCaster,
        ChildOf(ship),
    ));
    for kind in [
        DockModule::Reserve,
        DockModule::EngineRoom,
        DockModule::Floatplane,
        DockModule::Missiles,
    ] {
        for instance in 0..shipvivor_campaign::MAX_SECTIONS_PER_KIND {
            let section = commands
                .spawn((
                    SectionModel(kind, instance),
                    Transform::default(),
                    Visibility::Hidden,
                    ChildOf(ship),
                ))
                .id();
            let mut detail =
                |mesh: &Handle<Mesh>, paint: &Handle<StandardMaterial>, pos: Vec3, size: Vec3| {
                    commands.spawn((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(paint.clone()),
                        Transform::from_translation(pos).with_scale(size),
                        ChildOf(section),
                    ));
                };
            for side in [-1., 1.] {
                for y in [1.98, 2.32] {
                    detail(
                        &cube,
                        &pale,
                        Vec3::new(side * 2.55, y, 0.),
                        Vec3::new(0.07, 0.07, 5.),
                    );
                }
                for z in [-1.25, 1.25] {
                    detail(
                        &cube,
                        &pale,
                        Vec3::new(side * 2.55, 1.95, z),
                        Vec3::new(0.09, 0.8, 0.10),
                    );
                }
            }
            match kind {
                DockModule::Reserve => {
                    detail(
                        &cube,
                        &iron,
                        Vec3::new(0., 1.70, 0.),
                        Vec3::new(3.4, 0.34, 3.6),
                    );
                    for z in [-0.85, 0.85] {
                        detail(
                            &cube,
                            &ivory,
                            Vec3::new(0., 1.90, z),
                            Vec3::new(3.1, 0.17, 1.5),
                        );
                        detail(
                            &cube,
                            &brass,
                            Vec3::new(0., 2.02, z),
                            Vec3::new(0.85, 0.12, 0.18),
                        );
                    }
                }
                DockModule::EngineRoom => {
                    detail(
                        &cube,
                        &steel,
                        Vec3::new(0., 1.93, 0.),
                        Vec3::new(3.5, 0.82, 3.7),
                    );
                    for x in [-0.82, 0.82] {
                        detail(
                            &drum,
                            &brass,
                            Vec3::new(x, 2.8, 0.4),
                            Vec3::new(1.04, 1.1, 1.15),
                        );
                        detail(
                            &drum,
                            &iron,
                            Vec3::new(x, 3.45, 0.4),
                            Vec3::new(1.17, 0.32, 1.3),
                        );
                    }
                    for z in [-1.45, -1.12, -0.79] {
                        detail(
                            &cube,
                            &iron,
                            Vec3::new(0., 2.37, z),
                            Vec3::new(2.8, 0.09, 0.16),
                        );
                    }
                }
                // The catapult and the missile launcher keep their own proportions
                // on a widened beam, so they stand on a section of their own.
                DockModule::Floatplane | DockModule::Missiles => {
                    let fitted = commands
                        .spawn((
                            SectionModel(kind, instance),
                            Unscaled,
                            Transform::default(),
                            Visibility::Hidden,
                            ChildOf(ship),
                        ))
                        .id();
                    crate::strike_craft::equip(&mut commands, fitted, kind, instance, &gear);
                }
                // Turrets stand on the gun stations.
                DockModule::Turret => {}
            }
        }
    }
    for rack in Rack::BOTH {
        let side = if rack == Rack::Port { -1. } else { 1. };
        let base = commands
            .spawn((
                DeckRack(rack),
                Transform::from_xyz(side * 1.95, 2.03, 1.9)
                    .with_rotation(Quat::from_rotation_y(-rack.center(0.))),
                Visibility::default(),
                ChildOf(ship),
            ))
            .id();
        for lane in [-0.5, 0., 0.5] {
            commands.spawn((
                Mesh3d(drum.clone()),
                MeshMaterial3d(steel.clone()),
                Transform::from_xyz(lane, 0., 0.)
                    .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
                    .with_scale(Vec3::new(0.43, 1.52, 0.43)),
                ChildOf(base),
            ));
        }
    }
    let house = meshes.add(crate::ordnance::cruiser_gunhouse());
    let gunmetal = materials.add(StandardMaterial::default());
    let ring = meshes.add(Cylinder::new(0.5, 1.).mesh().resolution(20));
    for station in Station::SLOTS {
        commands.spawn((
            Barbette(station),
            Mesh3d(ring.clone()),
            MeshMaterial3d(steel.clone()),
            Transform::default(),
            Visibility::Hidden,
            ChildOf(ship),
        ));
        let mount = commands
            .spawn((
                Turret(station),
                Transform::from_xyz(0., 1.72, 0.),
                Visibility::default(),
                ChildOf(ship),
            ))
            .id();
        for (mesh, paint, position, scale) in [
            (&drum, &iron, Vec3::ZERO, Vec3::new(2.7, 0.42, 2.7)),
            (&house, &gunmetal, Vec3::ZERO, Vec3::ONE),
            (
                &cube,
                &steel,
                Vec3::new(0.40, 1.36, 0.48),
                Vec3::new(0.83, 0.11, 0.66),
            ),
            (
                &cube,
                &iron,
                Vec3::new(0., 0.78, -1.10),
                Vec3::new(1.74, 0.52, 0.30),
            ),
            (
                &cube,
                &steel,
                Vec3::new(0., 0.98, 0.48),
                Vec3::new(3.1, 0.22, 0.32),
            ),
        ] {
            commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(paint.clone()),
                Transform::from_translation(position).with_scale(scale),
                ChildOf(mount),
            ));
        }
        for tube in 0..TurretLayout::Quad.count() {
            let tubes = commands
                .spawn((
                    GunTubes { station, tube },
                    Transform::from_xyz(0., 0.63, 0.),
                    Visibility::Hidden,
                    ChildOf(mount),
                ))
                .id();
            for (depth, width, length, mat) in [
                (-1.72, 0.43, 1.58, &steel),
                (-2.95, 0.29, 2.15, &iron),
                (-3.99, 0.34, 0.18, &pale),
            ] {
                commands.spawn((
                    Mesh3d(drum.clone()),
                    MeshMaterial3d(mat.clone()),
                    Transform::from_xyz(0., 0., depth)
                        .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
                        .with_scale(Vec3::new(width, length, width)),
                    ChildOf(tubes),
                ));
            }
        }
    }

    for i in 0..shipvivor_encounter::Secondaries::MOST_PER_SIDE * 2 {
        let (x, fore) = shipvivor_encounter::WingGun::station(i);
        // On the deck, which rises towards the stem under the bow pair.
        let wing = commands
            .spawn((
                WingPosition(i),
                Transform::from_xyz(x, hull::deck_height(-fore) + 0.09, -fore),
                Visibility::Hidden,
                ChildOf(ship),
            ))
            .id();
        for (mesh, mat, at, scale, tip) in [
            (&drum, &iron, Vec3::ZERO, Vec3::new(0.93, 0.36, 0.93), 0.),
            (
                &cube,
                &pale,
                Vec3::new(0., 0.36, -0.28),
                Vec3::new(1.02, 0.75, 0.19),
                0.,
            ),
        ] {
            commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(mat.clone()),
                Transform::from_translation(at)
                    .with_rotation(Quat::from_rotation_x(tip))
                    .with_scale(scale),
                ChildOf(wing),
            ));
        }
        // One barrel on a single mount, or a pair side by side once twinned.
        for (fitted, x) in [(1, 0.), (2, -0.18), (2, 0.18)] {
            commands.spawn((
                WingBarrel(fitted),
                Mesh3d(drum.clone()),
                MeshMaterial3d(steel.clone()),
                Transform::from_xyz(x, 0.36, -0.87)
                    .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
                    .with_scale(Vec3::new(0.17, 1.81, 0.17)),
                if fitted == 1 {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                },
                ChildOf(wing),
            ));
        }
    }
    for i in 0..AntiAir::MOST_MOUNTS {
        let (x, fore) = AaMount::station(i);
        let tub = commands
            .spawn((
                AaPosition(i),
                Transform::from_xyz(x, 0., -fore),
                Visibility::Hidden,
                ChildOf(ship),
            ))
            .id();
        // The bridge wings carry their own pair. The others stand on a column raised
        // from the main deck to the same height.
        if !AaMount::on_bridge_wing(i) {
            let deck = hull::deck_height(-fore);
            commands.spawn((
                Mesh3d(drum.clone()),
                MeshMaterial3d(steel.clone()),
                Transform::from_xyz(0., 0.5 * (deck + AA_FLOOR), 0.).with_scale(Vec3::new(
                    0.42,
                    AA_FLOOR - deck,
                    0.42,
                )),
                ChildOf(tub),
            ));
        }
        for (mesh, mat, at, scale) in [
            (&drum, &iron, AA_FLOOR - 0.04, Vec3::new(1.12, 0.08, 1.12)),
            (&drum, &pale, AA_FLOOR + 0.16, Vec3::new(1.1, 0.34, 1.1)),
        ] {
            commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(mat.clone()),
                Transform::from_xyz(0., at, 0.).with_scale(scale),
                ChildOf(tub),
            ));
        }
        let train = commands
            .spawn((
                AaTrain(i),
                Transform::from_xyz(0., AA_FLOOR, 0.)
                    .with_rotation(Quat::from_rotation_y(-x.signum() * 1.1)),
                Visibility::Inherited,
                ChildOf(tub),
            ))
            .id();
        for (mesh, mat, at, scale) in [
            (
                &drum,
                &iron,
                Vec3::new(0., 0.12, 0.),
                Vec3::new(0.5, 0.24, 0.5),
            ),
            (
                &cube,
                &steel,
                Vec3::new(0., 0.36, 0.08),
                Vec3::new(0.46, 0.3, 0.42),
            ),
            (
                &cube,
                &pale,
                Vec3::new(0., 0.5, -0.24),
                Vec3::new(0.78, 0.42, 0.05),
            ),
        ] {
            commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(mat.clone()),
                Transform::from_translation(at).with_scale(scale),
                ChildOf(train),
            ));
        }
        let cradle = commands
            .spawn((
                AaCradle(i),
                Transform::from_xyz(0., 0.46, 0.).with_rotation(Quat::from_rotation_x(0.5)),
                Visibility::Inherited,
                ChildOf(train),
            ))
            .id();
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(iron.clone()),
            Transform::from_xyz(0., 0., 0.02).with_scale(Vec3::new(0.34, 0.2, 0.5)),
            ChildOf(cradle),
        ));
        // Two barrels side by side, each with a flash hider at the muzzle.
        for side in [-0.11, 0.11] {
            for (z, width, length, mat) in
                [(-0.68, 0.075, 1.05, &steel), (-1.22, 0.11, 0.12, &iron)]
            {
                commands.spawn((
                    Mesh3d(drum.clone()),
                    MeshMaterial3d(mat.clone()),
                    Transform::from_xyz(side, 0.02, z)
                        .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
                        .with_scale(Vec3::new(width, length, width)),
                    ChildOf(cradle),
                ));
            }
        }
    }
    for (index, gate) in course.gates().iter().enumerate() {
        for (p, color) in gate.buoys().into_iter().zip([&red, &green]) {
            let base = commands
                .spawn((
                    Buoy,
                    Visibility::Hidden,
                    Transform::from_xyz(p.x, 0.62, p.y),
                ))
                .id();
            for (size, height, material) in [
                (Vec3::new(2.1, 1.05, 2.1), 0., color),
                (Vec3::new(0.75, 1.65, 0.75), 1., &ivory),
                (Vec3::new(1.0, 0.55, 1.0), 2.1, color),
            ] {
                commands.spawn((
                    Mesh3d(drum.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::from_xyz(0., height, 0.).with_scale(size),
                    ChildOf(base),
                ));
            }
            commands.spawn((
                Signal(index),
                Visibility::Hidden,
                Mesh3d(cube.clone()),
                MeshMaterial3d(brass.clone()),
                Transform::from_xyz(0., 3.85, 0.)
                    .with_scale(Vec3::splat(1.35))
                    .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4)),
                ChildOf(base),
            ));
        }
    }

    let horizon = commands
        .spawn((Horizon, Transform::default(), Visibility::default()))
        .id();
    let rock_style = materials.add(StandardMaterial {
        unlit: true,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(distant_shore())),
        Transform::from_scale(Vec3::splat(5.2)),
        MeshMaterial3d(rock_style),
        ChildOf(horizon),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 9500.0,
            color: Color::srgb(1., 0.87, 0.72),
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(51., 36., -84.).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

// A flat-shaded mesh lets a whole hull or sea tile use a single material.
#[derive(Default)]
pub(crate) struct Facets {
    positions: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
}

impl Facets {
    pub(crate) fn triangle(&mut self, vertices: [Vec3; 3], color: Color) {
        self.triangle_colors(vertices, [color; 3]);
    }

    pub(crate) fn triangle_colors(&mut self, vertices: [Vec3; 3], colors: [Color; 3]) {
        self.positions.extend(vertices.map(|p| p.to_array()));
        self.colors
            .extend(colors.map(|color| color.to_linear().to_f32_array()));
    }

    pub(crate) fn quad(&mut self, [a, b, c, d]: [Vec3; 4], color: Color) {
        self.triangle([a, b, c], color);
        self.triangle([a, c, d], color);
    }

    // A circular section may run along any axis, for rollers, winches and hollow muzzles.
    pub(crate) fn tube(
        &mut self,
        from: Vec3,
        to: Vec3,
        radius: f32,
        end_radius: f32,
        color: Color,
    ) {
        let direction = (to - from).normalize();
        let u = direction.any_orthonormal_vector();
        let v = direction.cross(u);
        let ring = |i: usize| {
            let (s, c) = (i as f32 * std::f32::consts::TAU / 12.).sin_cos();
            u * c + v * s
        };
        for i in 0..12 {
            let (p, q) = (ring(i), ring(i + 1));
            self.quad(
                [
                    from + p * radius,
                    from + q * radius,
                    to + q * end_radius,
                    to + p * end_radius,
                ],
                color,
            );
            self.triangle([to, to + p * end_radius, to + q * end_radius], color);
        }
    }

    pub(crate) fn finish(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
        .with_computed_flat_normals()
    }
}

pub(crate) fn hull_mesh() -> Mesh {
    extended_hull_mesh(Hull::CRUISER)
}

// Two distant headlands give a stable bearing reference when the player pans.
fn distant_shore() -> Mesh {
    let mut mesh = Facets::default();
    for (radius, shift, distant) in [(660., 1.4, true), (427., 0., false)] {
        let ridge = |i: usize| {
            let t = std::f32::consts::TAU * (i % 300) as f32 / 300.;
            let a = t + shift;
            let profile = 0.52 * (3. * a + 0.6).cos() + 0.48 * (5. * a - 1.3).cos() + 0.10;
            let h = (profile * if distant { 103. } else { 71. }
                + 6. * (29. * a).sin()
                + 3. * (53. * a).cos())
            .max(0.);
            let ray = Vec3::new(t.sin(), 0., -t.cos());
            let foot = ray * (radius + 7. * (9. * a).sin()) - Vec3::Y;
            let shoulder =
                ray * (radius + 10. + 0.14 * h) + Vec3::Y * (h * (0.22 + 0.14 * (14. * a).sin()));
            let peak = ray * (radius + 24. + 0.42 * h) + Vec3::Y * h;
            let snow_height =
                if distant { 42. } else { 47. } + 9. * (25. * a).sin() + 4. * (69. * a).cos();
            let t = ((snow_height - shoulder.y) / (peak.y - shoulder.y).max(0.01)).clamp(0., 1.);
            let snowline = shoulder.lerp(peak, t);
            (foot, shoulder, snowline, peak)
        };
        for i in 0..300 {
            let (a, b, c, d) = ridge(i);
            let (e, f, g, h) = ridge(i + 1);
            if d.y == 0. && h.y == 0. {
                continue;
            }
            let light = (0.5 + 0.09 * (d.y - h.y)).clamp(0., 1.);
            let tones = |dark: [u8; 3], bright: [u8; 3]| {
                let color = std::array::from_fn::<_, 3, _>(|j| {
                    f32::from(dark[j]).lerp(f32::from(bright[j]), light) as u8
                });
                Color::srgb_u8(color[0], color[1], color[2])
            };
            let stone = if distant {
                tones([96, 114, 131], [154, 160, 160])
            } else {
                tones([59, 78, 91], [118, 127, 127])
            };
            let base = if distant {
                stone
            } else {
                tones([47, 70, 76], [88, 105, 96])
            };
            let cap = tones([143, 165, 183], [233, 227, 209]);
            for (left, right, up, upper, paint) in
                [(a, e, b, f, base), (b, f, c, g, stone), (c, g, d, h, cap)]
            {
                mesh.triangle([left, right, up], paint);
                mesh.triangle([right, upper, up], paint);
            }
        }
    }
    mesh.finish()
}

pub fn place_ship(
    run: Res<Campaign>,
    helm: Res<PlayerHelm>,
    sea: Res<Sea>,
    mut ship: Single<&mut Transform, With<Ship>>,
) {
    if matches!(run.screen(), crate::state::Screen::Drydock(_)) {
        **ship = Transform::from_translation(crate::dockyard::BERTH);
        return;
    }
    let heel = helm.rudder * (helm.speed / Helm::FULL_AHEAD) * 0.055;
    **ship = sea.float(helm.position, helm.heading, &run.hull_shape());
    ship.rotate_local_z(heel);
}

pub fn place_horizon(helm: Res<PlayerHelm>, mut horizon: Single<&mut Transform, With<Horizon>>) {
    horizon.translation = Vec3::new(helm.position.x, 0., helm.position.y);
}

pub fn signals(
    course: Res<TrialCourse>,
    run: Res<Campaign>,
    mut lamps: Query<(&Signal, &mut Visibility), Without<Buoy>>,
    mut buoys: Query<&mut Visibility, With<Buoy>>,
) {
    let trial = run.mode == crate::state::Mode::Trial;
    for mut visible in &mut buoys {
        visible.set_if_neq(if trial {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    for (signal, mut lamp) in &mut lamps {
        lamp.set_if_neq(if trial && signal.0 == course.active() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
}

/// Buoys ride whatever water passes under them.
pub fn float_buoys(sea: Res<Sea>, mut buoys: Query<&mut Transform, With<Buoy>>) {
    for mut place in &mut buoys {
        place.translation.y = 0.62 + sea.height(place.translation.xz());
    }
}
