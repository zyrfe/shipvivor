//! The intermission yard. The ship itself is the menu: select a section of her and its
//! refits hang beneath it. Work on the hull as a whole has its own panel, and whole
//! sections from the yard's stock are dragged onto the ship where they should go.
//!
//! Every order is made on the ship at once and drawn as a blueprint, listed under its
//! section with its price, and can be taken back until the refit is built.
use crate::{
    cards,
    insignia::Insignia,
    state::{Campaign, Encounter, Screen},
    ui::{Action, leg_result},
};
use bevy::{camera::ScalingMode, prelude::*, ui::RelativeCursorPosition, window::PrimaryWindow};
use shipvivor_campaign::{Card, DeckModule, DockJob, DockModule, LegEnd, Refusal, ShipBeam};
use shipvivor_encounter::Hull;
use shipvivor_shipyard::Design;

mod catalog;
mod proposal;

/// A dedicated basin keeps the drydock view independent of where the patrol ended.
pub const BERTH: Vec3 = Vec3::new(8000., 0., 0.);

#[derive(Component)]
pub struct YardButton {
    selected: bool,
    confirm: bool,
}
#[derive(Component)]
pub struct Basin;
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum Cage {
    Proposal,
    Selection,
    Hover,
    /// The seam a carried section would be spliced into.
    Gap,
}
#[derive(Component)]
pub struct Dimension;
/// A yard panel. A click on one is not a click on the ship behind it.
#[derive(Component)]
pub struct Solid;
#[derive(Component)]
pub struct Callout;
#[derive(Component)]
pub struct Leader;
#[derive(Component)]
pub struct HoverTag;
#[derive(Component)]
pub struct HoverText;
#[derive(Component)]
pub struct HoverIcon;
/// The floor of the yard along the bottom: the hull panel and the stock beside it.
#[derive(Component)]
pub struct Floor;
/// One section in the yard's stock. A refused one cannot be lifted.
#[derive(Component)]
pub struct StockTile {
    module: DockModule,
    refused: bool,
}
/// The carried section, drawn under the cursor.
#[derive(Component)]
pub struct Ghost;
/// The control for work that the work picked out waits on. It strobes to be found.
#[derive(Component)]
pub struct Beckon;

/// Work the yard offers that the work picked out waits on: a job, or a section from the
/// stock to lengthen her.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Need {
    Job(DockJob),
    Stock(DockModule),
}

/// What the work picked out waits on, when the yard offers it: a broader beam for thicker
/// plate, a heavier bore or more barrels; fewer barrels for a bore no beam carries in
/// these; and a longer hull, by a section from the stock, for a broader beam. A wait on
/// work that waits in turn beckons the first step, which beckons the next once picked.
fn need(run: &Campaign) -> Option<Need> {
    let job = run.dock_job()?;
    let widen = Some(Need::Job(DockJob::WidenBeam));
    match run.refusal(job) {
        Some(Refusal::Beam(_)) => return widen,
        // Any section lengthens her, and an empty bay is the cheapest.
        Some(Refusal::Length(_)) => {
            return std::iter::once(DockModule::Reserve)
                .chain(DockModule::ALL)
                .find(|&module| run.stock_refusal(module).is_none())
                .map(Need::Stock);
        }
        Some(_) => return None,
        None if job.fits(run) => return None,
        None => {}
    }
    let house = |at: usize| run.ship.design().modules().get(at).map(|m| m.gunhouse);
    match job {
        DockJob::Calibre { at, to, .. } => {
            let barrels = house(at)?.barrels;
            match run.beam_for(at, barrels, to) {
                Some(beam) if beam > run.beam_class() => widen,
                Some(_) => None,
                // No beam carries it in so many barrels, but fewer would.
                None => {
                    run.beam_for(at, shipvivor_encounter::TurretLayout::Single, to)?;
                    let fewer = barrels.fewer()?;
                    Some(Need::Job(DockJob::Barrels {
                        at,
                        from: barrels,
                        to: fewer,
                    }))
                }
            }
        }
        DockJob::Barrels { at, to, .. } => run
            .beam_for(at, to, house(at)?.bore)
            .filter(|&beam| beam > run.beam_class())
            .and(widen),
        _ => None,
    }
}

/// A steady strobe, one pulse every half second: 0 at rest, 1 at its brightest.
fn strobe(seconds: f32) -> f32 {
    const PERIOD: f32 = 0.5;
    0.5 - 0.5 * (std::f32::consts::TAU * seconds / PERIOD).cos()
}
/// A beckoning control's fill and edge at this point in its strobe.
fn beckon_paint(base: Color, pulse: f32) -> (Color, Color) {
    (
        base.mix(&Color::srgb_u8(150, 116, 44), pulse),
        brass().mix(&Color::srgb_u8(255, 238, 186), pulse),
    )
}

fn brass() -> Color {
    Color::srgb_u8(249, 192, 91)
}
fn paper() -> Color {
    Color::srgb_u8(237, 236, 216)
}
fn muted() -> Color {
    Color::srgb_u8(169, 188, 185)
}
fn cyan() -> Color {
    Color::srgb_u8(124, 229, 218)
}
fn panel() -> Color {
    Color::srgba_u8(17, 31, 36, 247)
}
fn rule() -> Color {
    Color::srgb_u8(87, 100, 96)
}
fn coords(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

pub fn camera(run: &Campaign) -> (Transform, Projection) {
    let length = run.ship.design().length();
    let target = BERTH + Vec3::new(0., 1., (length - Design::BREAKWATER_LENGTH) * 0.5 - 2.0);
    let pose =
        Transform::from_translation(target + Vec3::new(-52., 30., 3.)).looking_at(target, Vec3::Y);
    // Frame the ship high, leaving the water below her for the refit callout.
    let pose = pose.with_translation(pose.translation - pose.up() * 5.5);
    let projection = OrthographicProjection {
        scaling_mode: ScalingMode::AutoMin {
            min_width: (length + 11.) * 1.22,
            min_height: 24.,
        },
        ..OrthographicProjection::default_3d()
    };
    (pose, projection.into())
}

pub fn setup(
    mut cmd: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut paints: ResMut<Assets<StandardMaterial>>,
) {
    let root = cmd
        .spawn((
            Basin,
            Transform::from_translation(BERTH),
            Visibility::Hidden,
        ))
        .id();
    let cube = meshes.add(Cuboid::default());
    let cylinder = meshes.add(Cylinder::new(0.5, 1.).mesh().resolution(12));
    let mut paint = |r, g, b| paints.add(Color::srgb_u8(r, g, b));
    let water = paint(45, 91, 98);
    let wet = paint(73, 79, 72);
    let concrete = paint(132, 128, 110);
    let pale = paint(175, 167, 141);
    let metal = paint(67, 78, 77);
    let black = paint(46, 49, 42);
    let yellow = paint(216, 156, 57);
    let wood = paint(145, 119, 81);
    let crate_edge = paint(170, 149, 113);
    // Stepped sill, a quay rail and the caisson gate at the head of the berth.
    let mut part = |mesh: &Handle<Mesh>,
                    material: &Handle<StandardMaterial>,
                    at: Vec3,
                    size: Vec3,
                    rotation: Quat| {
        cmd.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(at)
                .with_rotation(rotation)
                .with_scale(size),
            ChildOf(root),
        ));
    };
    let still = Quat::IDENTITY;
    part(
        &cube,
        &water,
        coords(0., -0.77, 0.),
        coords(185., 0.5, 190.),
        still,
    );
    for side in [-1., 1.] {
        let x = if side < 0. { -11.6 } else { 8.3 };
        for lane in 0..3 {
            let rise = lane as f32 * 0.62;
            part(
                &cube,
                &wet,
                coords(x + side * lane as f32, -1.05 + rise, 0.),
                coords(1.9, 2.1, 105.),
                still,
            );
        }
        part(
            &cube,
            &concrete,
            coords(x + side * 21., 0.26, 0.),
            coords(38., 3.5, 110.),
            still,
        );
        for z in (-45..=45).step_by(5) {
            let z = z as f32;
            part(
                &cube,
                &pale,
                coords(x + side * 2.5, 1.76, z),
                coords(1.25, 0.40, 4.92),
                still,
            );
            part(
                &cylinder,
                &black,
                coords(x + side * 3.8, 2.40, z),
                coords(0.48, 0.75, 0.48),
                still,
            );
            part(
                &cylinder,
                &metal,
                coords(x + side * 3.8, 2.78, z),
                coords(0.72, 0.16, 0.72),
                still,
            );
            // Alternating amber and tarred chocks mark the lip of the dock.
            part(
                &cube,
                &yellow,
                coords(x + side * 2.5, 2.02, z + 0.7),
                coords(0.45, 0.12, 1.60),
                still,
            );
            part(
                &cube,
                &black,
                coords(x + side * 2.5, 2.02, z + 2.25),
                coords(0.45, 0.12, 1.25),
                still,
            );
        }
    }
    for x in [14.5, 20.5] {
        part(
            &cube,
            &metal,
            coords(x, 2.13, 0.),
            coords(0.20, 0.25, 95.),
            still,
        );
    }
    // Expansion joints in the apron and a grated channel along the working edge.
    for side in [-1., 1.] {
        for across in 0..6 {
            let x = if side < 0. { -16.9 } else { 13.5 } + side * 3.8 * across as f32;
            part(
                &cube,
                &wet,
                coords(x, 2.021, 0.),
                coords(0.048, 0.018, 96.),
                still,
            );
        }
        for z in (-44..48).step_by(4) {
            let x = if side < 0. { -24. } else { 21.6 };
            part(
                &cube,
                &wet,
                coords(x, 2.025, z as f32),
                coords(19., 0.02, 0.052),
                still,
            );
        }
    }
    part(
        &cube,
        &metal,
        coords(-15.7, 2.05, 0.),
        coords(0.42, 0.09, 96.),
        still,
    );
    for z in -46..46 {
        part(
            &cube,
            &wet,
            coords(-15.7, 2.105, z as f32),
            coords(0.52, 0.04, 0.15),
            still,
        );
    }
    // Spare plate is stored in low stacks with timber spacers.
    for (x, z) in [(-17.2, 12.4), (17.2, -6.8)] {
        for lift in 0..4 {
            let y = 2.24 + lift as f32 * 0.23;
            part(
                &cube,
                &metal,
                coords(x, y, z),
                coords(1.7, 0.12, 4.6),
                still,
            );
            for dz in [-1.45, 1.45] {
                part(
                    &cube,
                    &wood,
                    coords(x, y + 0.13, z + dz),
                    coords(1.78, 0.14, 0.16),
                    still,
                );
            }
        }
    }
    for (x, z) in [
        (14.2, -16.),
        (15.7, -15.),
        (14.8, 17.),
        (17.2, 16.),
        (16.4, 20.),
        (-17.3, -8.),
        (-18., -5.),
    ] {
        part(
            &cube,
            &wood,
            coords(x, 2.83, z),
            coords(1.65, 1.65, 2.1),
            still,
        );
        for dz in [-0.8, 0.8] {
            part(
                &cube,
                &crate_edge,
                coords(x, 2.82, z + dz),
                coords(1.75, 1.75, 0.16),
                still,
            );
        }
    }
    for z in [-27., -9., 11., 29.] {
        part(
            &cube,
            &concrete,
            coords(29., 5.0, z),
            coords(13., 6., 16.9),
            still,
        );
        part(
            &cube,
            &metal,
            coords(29., 8.22, z),
            coords(14., 0.45, 17.9),
            still,
        );
        for dz in [-5.6, -2.8, 0., 2.8, 5.6] {
            part(
                &cube,
                &black,
                coords(22.43, 5.7, z + dz),
                coords(0.16, 1.8, 2.25),
                still,
            );
            part(
                &cube,
                &pale,
                coords(22.3, 5.7, z + dz),
                coords(0.10, 1.8, 0.1),
                still,
            );
        }
        // Sheet joints and a roll-up door at the left end of each workshop.
        let door_z = z - 6.8;
        part(
            &cube,
            &metal,
            coords(22.24, 3.72, door_z),
            coords(0.22, 3.3, 2.6),
            still,
        );
        for h in 0..7 {
            part(
                &cube,
                &wet,
                coords(22.10, 2.35 + h as f32 * 0.46, door_z),
                coords(0.08, 0.055, 2.44),
                still,
            );
        }
        for dz in [-8.1, -3.3, 3.3, 8.1] {
            part(
                &cube,
                &pale,
                coords(22.32, 5.15, z + dz),
                coords(0.26, 6.15, 0.25),
                still,
            );
        }
    }
    // Crane bogies, cross-braced uprights and an offset travelling hoist.
    for z in [-21., 24.] {
        for dz in [-2.1, 2.1] {
            part(
                &cube,
                &yellow,
                coords(15., 7.55, z + dz),
                coords(0.64, 10.7, 0.66),
                still,
            );
        }
        part(
            &cube,
            &metal,
            coords(15., 2.63, z),
            coords(2.35, 1.03, 5.9),
            still,
        );
        for tier in 0..4 {
            for sign in [-1., 1.] {
                part(
                    &cube,
                    &yellow,
                    coords(15., 4.0 + tier as f32 * 2.5, z),
                    coords(0.23, 4.8, 0.22),
                    Quat::from_rotation_x(sign * 1.03),
                );
            }
        }
        part(
            &cube,
            &yellow,
            coords(9., 12.65, z),
            coords(15.6, 0.76, 0.9),
            still,
        );
        part(
            &cube,
            &metal,
            coords(15., 12.9, z),
            coords(2.85, 2.2, 3.4),
            still,
        );
        part(
            &cube,
            &yellow,
            coords(3.6, 12.0, z),
            coords(1.5, 0.9, 1.25),
            still,
        );
        for dz in [-0.26, 0.26] {
            part(
                &cube,
                &black,
                coords(3.6, 9.85, z + dz),
                coords(0.065, 3.7, 0.065),
                still,
            );
        }
        part(
            &cube,
            &yellow,
            coords(3.6, 7.85, z),
            coords(0.55, 0.55, 0.8),
            still,
        );
    }

    // Wire cages: cyan for the refit being previewed, brass round the selected section
    // and a pale one round the section under the cursor.
    let mut cage = |cmd: &mut Commands, kind: Cage, color: Color, tint: Option<Color>| {
        let wire = paints.add(StandardMaterial {
            base_color: color,
            unlit: true,
            ..default()
        });
        let frame = cmd
            .spawn((
                kind,
                Transform::default(),
                Visibility::Hidden,
                ChildOf(root),
            ))
            .id();
        if let Some(tint) = tint {
            let tint = paints.add(StandardMaterial {
                base_color: tint,
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..default()
            });
            cmd.spawn((
                Mesh3d(cube.clone()),
                MeshMaterial3d(tint),
                Transform::from_xyz(-0.497, 0., 0.).with_scale(coords(0.005, 0.98, 0.98)),
                ChildOf(frame),
            ));
        }
        for axis in 0..3 {
            for i in [-0.5, 0.5] {
                for j in [-0.5, 0.5] {
                    let mut at = Vec3::ZERO;
                    let mut scale = Vec3::splat(0.016);
                    at[(axis + 1) % 3] = i;
                    at[(axis + 2) % 3] = j;
                    scale[axis] = 1.;
                    cmd.spawn((
                        Mesh3d(cube.clone()),
                        MeshMaterial3d(wire.clone()),
                        Transform::from_translation(at).with_scale(scale),
                        ChildOf(frame),
                    ));
                }
            }
        }
    };
    cage(
        &mut cmd,
        Cage::Proposal,
        cyan(),
        Some(Color::srgba(0.30, 0.93, 0.83, 0.24)),
    );
    cage(&mut cmd, Cage::Selection, brass(), None);
    cage(&mut cmd, Cage::Hover, Color::srgb_u8(214, 226, 222), None);
    cage(
        &mut cmd,
        Cage::Gap,
        cyan(),
        Some(Color::srgba(0.30, 0.93, 0.83, 0.55)),
    );
    // A traverse fan for each main turret the selected section can carry.
    let fan = paints.add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        double_sided: true,
        ..default()
    });
    for slot in 0..2 {
        cmd.spawn((
            Fan(slot),
            Mesh3d::default(),
            MeshMaterial3d(fan.clone()),
            Transform::default(),
            Visibility::Hidden,
            bevy::light::NotShadowCaster,
            ChildOf(root),
        ));
    }
}

/// The block a section of the ship occupies in the basin, as a centre and a size. The
/// same block is outlined when the section is selected and is what a click must hit.
fn section_box(design: &Design, at: usize) -> (Vec3, Vec3) {
    let (fore, aft) = design.span(at);
    (
        coords(0., 1.75, 0.5 * (fore + aft) - Hull::CRUISER.bow),
        coords(design.beam() + 0.6, 5.3, (aft - fore).max(1.)),
    )
}

/// The section a ray from the camera meets first, in basin coordinates.
pub fn section_at(design: &Design, origin: Vec3, direction: Vec3) -> Option<usize> {
    let inverse = direction.recip();
    (0..design.modules().len())
        .filter_map(|at| {
            let (centre, size) = section_box(design, at);
            let a = (centre - 0.5 * size - origin) * inverse;
            let b = (centre + 0.5 * size - origin) * inverse;
            let near = a.min(b).max_element().max(0.);
            let far = a.max(b).min_element();
            (far >= near).then_some((near, at))
        })
        .min_by(|x, y| x.0.total_cmp(&y.0))
        .map(|(_, at)| at)
}

/// The seam a section carried along a ray would be spliced into: the one nearest where
/// the ray crosses the ship's centreline, between the bow and the stern. Seam `at` lies
/// ahead of module `at`, so the new section becomes module `at`.
pub fn gap_at(design: &Design, origin: Vec3, direction: Vec3) -> Option<usize> {
    if direction.x.abs() < 1e-4 {
        return None;
    }
    let crossing = origin + direction * (-origin.x / direction.x);
    let bow = -Hull::CRUISER.bow;
    let stern = bow + design.length();
    if !(-2.5..=8.).contains(&crossing.y) || !(bow - 3.0..=stern + 3.0).contains(&crossing.z) {
        return None;
    }
    (1..design.modules().len()).min_by(|&a, &b| {
        let seam = |at| (design.span(at).0 + bow - crossing.z).abs();
        seam(a).total_cmp(&seam(b))
    })
}

/// The section under the cursor, whether the cursor is over a yard panel instead, and a
/// section from the yard's stock carried to the ship.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
pub struct Pointer {
    pub hovered: Option<usize>,
    pub over_panel: bool,
    pub carrying: Option<DockModule>,
    /// The seam the carried section is over.
    pub gap: Option<usize>,
    /// Set for one frame when the carried section is let go over a seam.
    pub dropped: Option<(usize, DockModule)>,
    /// Lifted with a click rather than a drag, so the next click places it.
    sticky: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn point(
    run: Res<Campaign>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<crate::camera::FollowCamera>>,
    panels: Query<&RelativeCursorPosition, With<Solid>>,
    tiles: Query<(&StockTile, &RelativeCursorPosition)>,
    mut pointer: ResMut<Pointer>,
) {
    if !matches!(run.screen(), Screen::Drydock(_)) {
        pointer.set_if_neq(Pointer::default());
        return;
    }
    let mut next = Pointer {
        carrying: pointer.carrying,
        sticky: pointer.sticky,
        over_panel: panels.iter().any(RelativeCursorPosition::cursor_over),
        ..default()
    };
    let ray = (!next.over_panel)
        .then(|| {
            let (window, (camera, pose)) = (window.single().ok()?, camera.single().ok()?);
            camera
                .viewport_to_world(pose, window.cursor_position()?)
                .ok()
        })
        .flatten();
    let design = run.ship.design();
    if let Some(ray) = ray {
        next.hovered = section_at(design, ray.origin - BERTH, *ray.direction);
        if next.carrying.is_some() {
            next.gap = gap_at(design, ray.origin - BERTH, *ray.direction);
        }
    }
    let tile = tiles
        .iter()
        .find(|(_, place)| place.cursor_over())
        .map(|(tile, _)| tile);
    let (pressed, released) = (
        mouse.just_pressed(MouseButton::Left),
        mouse.just_released(MouseButton::Left),
    );
    if keys.just_pressed(KeyCode::Escape) {
        next.carrying = None;
    } else if let Some(module) = next.carrying
        && (released && !next.sticky || pressed && next.sticky)
    {
        if let Some(at) = next.gap {
            next.dropped = Some((at, module));
            next.carrying = None;
        } else if released && tile.is_some_and(|t| t.module == module) {
            // A click on a tile lifts its section until the next click.
            next.sticky = true;
        } else {
            next.carrying = None;
        }
    }
    if pressed
        && next.carrying.is_none()
        && next.dropped.is_none()
        && let Some(tile) = tile.filter(|t| !t.refused)
    {
        next.carrying = Some(tile.module);
        next.sticky = false;
    }
    if next.carrying.is_none() {
        next.gap = None;
    }
    pointer.set_if_neq(next);
}

/// Arriving in the yard selects the first bay that can take another turret.
pub fn open(mut run: ResMut<Campaign>, mut docked: Local<bool>) {
    let now = matches!(run.screen(), Screen::Drydock(_));
    if now && !*docked {
        run.open_dock();
    }
    *docked = now;
}

/// Where the previewed refit happens, if it has a place to show.
fn proposal_box(run: &Campaign, job: DockJob) -> Option<(Vec3, Vec3)> {
    let design = run.ship.design();
    let fits = job.fits(run);
    Some(match job {
        DockJob::Insert { at, .. } if fits => {
            let proposed = run.projected_design(job)?;
            let (centre, size) = section_box(&proposed, at);
            (centre, size - coords(0.4, 0.4, 0.))
        }
        // Plate goes on the whole hull, so the whole hull is outlined.
        DockJob::Fitting(Card::Armor) if fits => (
            coords(0., 0.9, 0.5 * design.length() - Hull::CRUISER.bow),
            coords(design.beam() + 0.2, 3.8, design.length()),
        ),
        // The whole hull is widened, so the whole hull is outlined at her new beam.
        DockJob::WidenBeam if fits => (
            coords(0., 0.9, 0.5 * design.length() - Hull::CRUISER.bow),
            coords(
                run.next_beam().unwrap_or(run.beam_class()).metres() + 0.2,
                3.8,
                design.length(),
            ),
        ),
        DockJob::Move { from, to } => {
            let at = if fits { to } else { from };
            let (fore, aft) = design.span(at);
            (
                coords(0., 1.85, 0.5 * (fore + aft) - Hull::CRUISER.bow),
                coords(design.beam() + 0.2, 5.2, aft - fore),
            )
        }
        DockJob::Replace { at, .. } => {
            let (fore, aft) = design.span(at);
            (
                coords(0., 1.85, 0.5 * (fore + aft) - Hull::CRUISER.bow),
                coords(design.beam() + 0.2, 5.2, aft - fore),
            )
        }
        // The pair a new mounting adds, or the first pair for work on every secondary.
        DockJob::Fitting(
            Card::MoreSecondaries
            | Card::TwinSecondaries
            | Card::SecondaryRange
            | Card::QuickSecondaries
            | Card::HeavySecondaries,
        ) if fits => {
            let deck = shipvivor_encounter::DeckOffsets::for_design(design);
            let index = if job == DockJob::Fitting(Card::MoreSecondaries) {
                2 * run
                    .display_fittings()
                    .secondaries()
                    .per_side
                    .saturating_sub(1)
            } else {
                0
            };
            let (x, fore) = deck.wing_position(index);
            (
                coords(0., 2.05, -fore),
                coords(x.abs() * 2. + 1.4, 1.45, 3.),
            )
        }
        DockJob::Fitting(Card::MoreAntiAir | Card::HeavyAntiAir | Card::QuickAntiAir) if fits => {
            // The pair the card adds, or the first pair for an upgrade to every gun.
            let deck = shipvivor_encounter::DeckOffsets::for_design(design);
            let index = if job == DockJob::Fitting(Card::MoreAntiAir) {
                run.fittings.anti_air().mounts
            } else {
                0
            };
            let (x, fore) = deck.aa_position(index);
            (
                coords(0., 3.55, -fore),
                coords(x.abs() * 2. + 1.4, 1.3, 2.2),
            )
        }
        DockJob::Fitting(Card::Turbines | Card::Hardening | Card::Pumps) if fits => {
            let deck = shipvivor_encounter::DeckOffsets::for_design(design);
            (
                coords(0., 2.4, 1.1 + deck.machinery_z),
                coords(3.4, 4.8, 4.5),
            )
        }
        DockJob::Fitting(_)
        | DockJob::Calibre { .. }
        | DockJob::Barrels { .. }
        | DockJob::Barbette { .. }
            if fits =>
        {
            // A turret's own refit outlines it, standing as it would. Shells and gear
            // change every turret, so the one on this bay is shown.
            let bay = match job {
                DockJob::Calibre { at, .. }
                | DockJob::Barrels { at, .. }
                | DockJob::Barbette { at, .. } => at,
                _ => run.dock_module,
            };
            let projected = run.projected_design(job);
            let guns = projected.as_ref().unwrap_or(design).gun_stations();
            let mount = guns
                .iter()
                .find(|g| g.berth.module == bay)
                .or_else(|| guns.iter().find(|g| !g.paired()))
                .or(guns.first())?;
            let scale = run
                .display_fittings()
                .for_station(mount)
                .round()
                .mount_scale();
            let local_size = if mount.paired() {
                coords(6., 2.15, 3.35)
            } else {
                coords(3.35, 2.15, 6.)
            };
            let at = coords(
                mount.abeam + mount.home.sin() * 1.4 * scale,
                1.72 + mount.lift() + 0.7 * scale,
                -mount.fore - mount.home.cos() * 1.4 * scale,
            );
            (at, local_size * scale)
        }
        _ => return None,
    })
}

#[allow(clippy::type_complexity)]
pub fn stage(
    run: Res<Campaign>,
    pointer: Res<Pointer>,
    mut basin: Single<&mut Visibility, (With<Basin>, Without<Cage>)>,
    mut cages: Query<(&Cage, &mut Transform, &mut Visibility), Without<Basin>>,
    mut buttons: Query<
        (
            &YardButton,
            &RelativeCursorPosition,
            &mut BackgroundColor,
            &mut BorderColor,
            Has<Beckon>,
        ),
        Without<StockTile>,
    >,
    mut tiles: Query<(
        &StockTile,
        &RelativeCursorPosition,
        &mut BackgroundColor,
        &mut BorderColor,
        Has<Beckon>,
    )>,
    clock: Res<Time>,
) {
    if !matches!(run.screen(), Screen::Drydock(_)) {
        basin.set_if_neq(Visibility::Hidden);
        return;
    }
    basin.set_if_neq(Visibility::Inherited);
    let design = run.ship.design();
    let job = run.dock_job();
    // Work with its own outline puts the selection aside: a bay refit outlines the bay,
    // and hull work or a new section is not about the selected one.
    let own_outline = matches!(
        job,
        Some(
            DockJob::Move { .. }
                | DockJob::Replace { .. }
                | DockJob::Insert { .. }
                | DockJob::WidenBeam
        )
    );
    let last = design.modules().len();
    for (kind, mut pose, mut visible) in &mut cages {
        let placed = match kind {
            Cage::Proposal => job.and_then(|job| proposal_box(&run, job)),
            Cage::Selection if !own_outline && run.dock_module < last => {
                Some(section_box(design, run.dock_module))
            }
            Cage::Selection => None,
            Cage::Hover if pointer.carrying.is_some() => None,
            Cage::Hover => pointer
                .hovered
                .filter(|&at| at != run.dock_module && at < last)
                .map(|at| section_box(design, at)),
            Cage::Gap => pointer.gap.map(|at| {
                let (centre, size) = section_box(design, at);
                (
                    centre - coords(0., 0., 0.5 * size.z),
                    coords(size.x + 0.5, size.y + 0.4, 0.6),
                )
            }),
        };
        if let Some((at, size)) = placed {
            *pose = Transform::from_translation(at).with_scale(size);
            visible.set_if_neq(Visibility::Inherited);
        } else {
            visible.set_if_neq(Visibility::Hidden);
        }
    }
    // Work that the work picked out waits on strobes, so it can be found.
    let pulse = strobe(clock.elapsed_secs());
    for (button, mouse, mut bg, mut edge, beckons) in &mut buttons {
        let base = button_paint(button.selected, button.confirm, mouse.cursor_over());
        if beckons {
            let (fill, rim) = beckon_paint(base, pulse);
            bg.set_if_neq(BackgroundColor(fill));
            edge.set_if_neq(BorderColor::all(rim));
        } else {
            bg.set_if_neq(BackgroundColor(base));
        }
    }
    for (tile, mouse, mut bg, mut edge, beckons) in &mut tiles {
        let lifted = pointer.carrying == Some(tile.module);
        let base = if tile.refused {
            panel()
        } else {
            button_paint(lifted, false, mouse.cursor_over())
        };
        if beckons {
            let (fill, rim) = beckon_paint(base, pulse);
            bg.set_if_neq(BackgroundColor(fill));
            edge.set_if_neq(BorderColor::all(rim));
        } else {
            bg.set_if_neq(BackgroundColor(base));
        }
    }
}

/// Keep the callout under its section, joined to the hull by a leader, and name the
/// section under the cursor above it.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn place(
    run: Res<Campaign>,
    pointer: Res<Pointer>,
    scale: Res<UiScale>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<crate::camera::FollowCamera>>,
    mut callout: Query<
        (&mut Node, &ComputedNode),
        (
            With<Callout>,
            Without<Leader>,
            Without<HoverTag>,
            Without<Ghost>,
        ),
    >,
    mut leader: Query<
        &mut Node,
        (
            With<Leader>,
            Without<Callout>,
            Without<HoverTag>,
            Without<Ghost>,
        ),
    >,
    mut tag: Query<
        (&mut Node, &ComputedNode),
        (
            With<HoverTag>,
            Without<Callout>,
            Without<Leader>,
            Without<Ghost>,
        ),
    >,
    floor: Query<&ComputedNode, With<Floor>>,
    mut ledgers: Query<
        (&Ledger, &mut Node, &ComputedNode),
        (
            Without<Callout>,
            Without<Leader>,
            Without<HoverTag>,
            Without<Ghost>,
        ),
    >,
    mut ghost: Query<(&mut Node, &mut ImageNode), (With<Ghost>, Without<HoverIcon>)>,
    mut label: Query<&mut Text, With<HoverText>>,
    mut glyph: Query<&mut ImageNode, (With<HoverIcon>, Without<Ghost>)>,
    art: Res<Insignia>,
) {
    let (camera, pose) = *camera;
    let design = run.ship.design();
    let project = |local: Vec3| {
        camera
            .world_to_viewport(pose, BERTH + local)
            .ok()
            .map(|p| p / scale.0)
    };
    let Some(screen) = camera.logical_viewport_size().map(|s| s / scale.0) else {
        return;
    };
    let size = |c: &ComputedNode| c.size() * c.inverse_scale_factor();
    // The orders made on each section are listed under it, and the callout hangs under
    // the selected section, below its list.
    let keel = |at: usize| {
        (at < design.modules().len())
            .then(|| {
                project(coords(
                    -0.5 * design.beam(),
                    -0.6,
                    section_box(design, at).0.z,
                ))
            })
            .flatten()
    };
    let mut listed = 0.;
    for (ledger, mut node, computed) in &mut ledgers {
        let Some(anchor) = keel(ledger.0) else {
            node.display = Display::None;
            continue;
        };
        let extent = size(computed);
        node.display = Display::Flex;
        node.left = px(anchor.x - 0.5 * extent.x);
        node.top = px(anchor.y + 8.);
        if ledger.0 == run.dock_module {
            listed = extent.y + 8.;
        }
    }
    let anchor = keel(run.dock_module);
    // The callout stays clear of the panels along the floor of the yard.
    let ceiling = floor
        .iter()
        .map(|c| screen.y - 12. - size(c).y)
        .fold(screen.y, f32::min);
    for (mut node, computed) in &mut callout {
        let Some(anchor) = anchor else {
            node.display = Display::None;
            continue;
        };
        let extent = size(computed);
        // The options column sits under the section; the panel stays on screen. It keeps
        // room for the figures column, so showing them never moves the options.
        let room = OPTIONS_WIDTH + 24. + FIGURES_WIDTH + 15.;
        let left =
            (anchor.x - 0.5 * OPTIONS_WIDTH - 12.).clamp(12., (screen.x - room - 12.).max(12.));
        let top = (anchor.y + 34. + listed)
            .min(ceiling - extent.y - 10.)
            .max(12.);
        node.display = Display::Flex;
        node.left = px(left);
        node.top = px(top);
        for mut line in &mut leader {
            let drop = top - anchor.y - 4.;
            line.display = if drop > 2. {
                Display::Flex
            } else {
                Display::None
            };
            line.left = px(anchor.x - 1.);
            line.top = px(anchor.y + 4.);
            line.height = px(drop.max(0.));
        }
    }
    let hovered = pointer
        .hovered
        .filter(|&at| pointer.carrying.is_none() && at != run.dock_module)
        .filter(|&at| at < design.modules().len());
    let cursor = window
        .single()
        .ok()
        .and_then(Window::cursor_position)
        .map(|p| p / scale.0);
    for (mut node, mut image) in &mut ghost {
        let (Some(module), Some(cursor)) = (pointer.carrying, cursor) else {
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        node.left = px(cursor.x + 10.);
        node.top = px(cursor.y + 10.);
        let picture = catalog::stock_art(module, &art);
        if image.image != picture {
            image.image = picture;
        }
        image.color = if pointer.gap.is_some() {
            cyan()
        } else {
            paper()
        };
    }
    let peak = hovered.and_then(|at| {
        let (centre, extent) = section_box(design, at);
        project(coords(0., centre.y + 0.5 * extent.y, centre.z))
    });
    for (mut node, computed) in &mut tag {
        let Some(peak) = peak else {
            node.display = Display::None;
            continue;
        };
        let extent = size(computed);
        node.display = Display::Flex;
        node.left = px(peak.x - 0.5 * extent.x);
        node.top = px(peak.y - extent.y - 6.);
    }
    if let Some(at) = hovered {
        let name = section_name(&run, at);
        for mut text in &mut label {
            if text.0 != name {
                text.0 = name.to_string();
            }
        }
        let image = section_art(&run, at, &art);
        for mut glyph in &mut glyph {
            if glyph.image != image {
                glyph.image = image.clone();
            }
        }
    }
}

const OPTIONS_WIDTH: f32 = 372.;
/// Width of the column of figures for the refit under the cursor.
const FIGURES_WIDTH: f32 = 250.;

/// What the hover tag and the callout call a section.
fn section_name(run: &Campaign, at: usize) -> &'static str {
    let design = run.ship.design();
    if at == 0 {
        "Bow"
    } else if at + 1 >= design.modules().len() {
        "Stern"
    } else {
        catalog::slot_name(design, at)
    }
}

/// The pictogram for a section of the ship.
fn section_art(run: &Campaign, at: usize, art: &Insignia) -> Handle<Image> {
    use shipvivor_shipyard::Component::*;
    let design = run.ship.design();
    if at == 0 || at + 1 >= design.modules().len() {
        return art.hull.clone();
    }
    let parts = &design.modules()[at].fit;
    if parts.contains(&Engine) {
        art.fitting(Card::Turbines)
    } else if parts.contains(&Director) {
        art.bridge.clone()
    } else if parts.contains(&Catapult) {
        art.floatplane.clone()
    } else if parts.contains(&MissileRack) {
        art.missile.clone()
    } else {
        match parts.iter().filter(|&&p| p == MainGun).count() {
            0 => art.reserve_bay.clone(),
            1 => art.turret.clone(),
            _ => art.wing_turrets.clone(),
        }
    }
}

/// Exchanges are named by what happens to the bay's turrets, so the list says plainly
/// which choice adds one.
fn exchange_name(run: &Campaign, at: usize, with: DeckModule) -> &'static str {
    use shipvivor_shipyard::Component::MainGun;
    let guns = run
        .ship
        .design()
        .modules()
        .get(at)
        .map_or(0, |m| m.fit.iter().filter(|&&c| c == MainGun).count());
    if run.module_locked(at) {
        return with.name();
    }
    match (with, guns) {
        (DeckModule::CentreBattery, 0) => "Add centre turret",
        (DeckModule::CentreBattery, 1) => "Centre turret",
        (DeckModule::CentreBattery, _) => "Merge into centre turret",
        (DeckModule::WingBattery, 0) => "Add wing turrets",
        (DeckModule::WingBattery, 1) => "Split into wing turrets",
        (DeckModule::WingBattery, _) => "Wing turrets",
        (DeckModule::Reserve, 0) => "Reserve bay",
        (DeckModule::Reserve, 1) => "Remove turret",
        (DeckModule::Reserve, _) => "Remove wing turrets",
    }
}
fn job_name(job: DockJob, run: &Campaign) -> String {
    match job {
        DockJob::Insert { module, .. } => format!("Add {}", module.name().to_lowercase()),
        DockJob::WidenBeam => "Widen beam".into(),
        DockJob::Fitting(card) => cards::face(card, run.fittings, run.ship.design())
            .name
            .into(),
        DockJob::Calibre { to, .. } => format!("{to}-inch guns"),
        DockJob::Barrels { from, to, .. } => match (to > from, to.count()) {
            (false, _) => "Fewer barrels".into(),
            (true, 2) => "Twin turret".into(),
            (true, 3) => "Triple turret".into(),
            _ => "Quad turret".into(),
        },
        DockJob::Barbette { from, to, .. } if to > from => "Raise turret".into(),
        DockJob::Barbette { .. } => "Lower turret".into(),
        DockJob::Move { from, to } => if to < from {
            "Move forward"
        } else {
            "Move aft"
        }
        .into(),
        DockJob::Replace { at, with } => exchange_name(run, at, with).into(),
    }
}
fn button_paint(selected: bool, confirm: bool, hover: bool) -> Color {
    match (selected, confirm, hover) {
        (_, true, true) => Color::srgb_u8(255, 204, 111),
        (_, true, false) => brass(),
        (true, _, _) => Color::srgb_u8(66, 60, 39),
        (_, _, true) => Color::srgb_u8(48, 73, 76),
        _ => Color::srgb_u8(32, 48, 51),
    }
}
fn text(cmd: &mut Commands, parent: Entity, words: impl Into<String>, size: f32, color: Color) {
    cmd.spawn((
        Text::new(words.into()),
        TextFont::from_font_size(size),
        TextColor(color),
        ChildOf(parent),
    ));
}
fn node(cmd: &mut Commands, parent: Entity, node: Node, color: Color) -> Entity {
    cmd.spawn((node, BackgroundColor(color), ChildOf(parent)))
        .id()
}
fn button(
    cmd: &mut Commands,
    parent: Entity,
    action: Option<Action>,
    selected: bool,
    confirm: bool,
    node: Node,
) -> Entity {
    let mut entity = cmd.spawn((
        node,
        BackgroundColor(if action.is_some() {
            button_paint(selected, confirm, false)
        } else {
            panel()
        }),
        BorderColor::all(if selected || confirm && action.is_some() {
            brass()
        } else {
            rule()
        }),
        ChildOf(parent),
    ));
    if let Some(action) = action {
        entity.insert((
            action,
            RelativeCursorPosition::default(),
            YardButton { selected, confirm },
        ));
    }
    entity.id()
}
/// A tile's price in yard points, in its top corner by the spanner.
fn price_tag(cmd: &mut Commands, tile: Entity, art: &Insignia, cost: u16, ink: Color) {
    let price = cmd
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(3),
                right: px(4),
                column_gap: px(2),
                align_items: AlignItems::Center,
                ..default()
            },
            ChildOf(tile),
        ))
        .id();
    icon(cmd, price, art.order.clone(), ink, 9.);
    text(cmd, price, cost.to_string(), 10., ink);
}
fn icon(cmd: &mut Commands, parent: Entity, image: Handle<Image>, color: Color, size: f32) {
    cmd.spawn((
        ImageNode {
            image,
            color,
            ..default()
        },
        Node {
            width: px(size),
            height: px(size),
            ..default()
        },
        ChildOf(parent),
    ));
}

/// A pictogram and a figure side by side.
fn badge(
    cmd: &mut Commands,
    parent: Entity,
    image: Handle<Image>,
    color: Color,
    words: impl Into<String>,
    size: f32,
) {
    let row = node(
        cmd,
        parent,
        Node {
            align_items: AlignItems::Center,
            column_gap: px(5),
            ..default()
        },
        Color::NONE,
    );
    icon(cmd, row, image, color, 14.);
    text(cmd, row, words, size, color);
}

fn spacer(cmd: &mut Commands, parent: Entity) {
    node(
        cmd,
        parent,
        Node {
            flex_grow: 1.,
            ..default()
        },
        Color::NONE,
    );
}

pub fn screen(
    cmd: &mut Commands,
    root: Entity,
    run: &Campaign,
    battle: &Encounter,
    art: &Insignia,
    reason: LegEnd,
) {
    // The length a section carried over a seam would add, measured under where it goes.
    let measure = cmd
        .spawn((
            Dimension,
            Node {
                position_type: PositionType::Absolute,
                display: Display::None,
                height: px(26),
                border: UiRect {
                    left: px(2),
                    right: px(2),
                    bottom: px(2),
                    ..default()
                },
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BorderColor::all(cyan()),
            ChildOf(root),
        ))
        .id();
    let caption = node(
        cmd,
        measure,
        Node {
            padding: UiRect::axes(px(5), px(1)),
            ..default()
        },
        panel(),
    );
    text(cmd, caption, "+5 m", 15., cyan());
    header(cmd, root, run, battle, art, reason);

    // The section under the cursor, named above it each frame.
    let tag = cmd
        .spawn((
            HoverTag,
            Node {
                position_type: PositionType::Absolute,
                display: Display::None,
                padding: UiRect::axes(px(7), px(3)),
                border: UiRect::all(px(1)),
                column_gap: px(5),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(panel()),
            BorderColor::all(rule()),
            ChildOf(root),
        ))
        .id();
    cmd.spawn((
        HoverIcon,
        ImageNode {
            image: art.hull.clone(),
            color: paper(),
            ..default()
        },
        Node {
            width: px(14),
            height: px(14),
            ..default()
        },
        ChildOf(tag),
    ));
    cmd.spawn((
        HoverText,
        Text::new(""),
        TextFont::from_font_size(13.),
        TextColor(paper()),
        ChildOf(tag),
    ));
    cmd.spawn((
        Leader,
        Node {
            position_type: PositionType::Absolute,
            display: Display::None,
            width: px(2),
            ..default()
        },
        BackgroundColor(brass()),
        ChildOf(root),
    ));

    ledgers(cmd, root, run, art);
    floor(cmd, root, run, art);
    callout(cmd, root, run, art);
    // Drawn last, so the carried section passes over every panel.
    cmd.spawn((
        Ghost,
        ImageNode {
            image: art.add_bay.clone(),
            color: paper(),
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            display: Display::None,
            width: px(30),
            height: px(30),
            ..default()
        },
        ChildOf(root),
    ));
}

/// The orders made on one section and not yet built, listed on the water under it.
#[derive(Component)]
pub struct Ledger(pub usize);

fn ledgers(cmd: &mut Commands, root: Entity, run: &Campaign, art: &Insignia) {
    let Some(plan) = run.plan() else { return };
    let mut sections: Vec<usize> = plan.orders().iter().filter_map(|o| plan.place(o)).collect();
    sections.sort_unstable();
    sections.dedup();
    for at in sections {
        let list = cmd
            .spawn((
                Ledger(at),
                Solid,
                RelativeCursorPosition::default(),
                Node {
                    position_type: PositionType::Absolute,
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    row_gap: px(3),
                    padding: UiRect::all(px(4)),
                    border: UiRect::top(px(2)),
                    ..default()
                },
                BackgroundColor(panel()),
                BorderColor::all(cyan()),
                ChildOf(root),
            ))
            .id();
        for (index, order) in plan.orders().iter().enumerate() {
            if plan.place(order) == Some(at) {
                order_row(cmd, list, run, art, index, order);
            }
        }
    }
}

/// One order not yet built: what it is and its price. A click takes it back.
fn order_row(
    cmd: &mut Commands,
    parent: Entity,
    run: &Campaign,
    art: &Insignia,
    index: usize,
    order: &shipvivor_campaign::Order,
) {
    let row = button(
        cmd,
        parent,
        Some(Action::Undo(index)),
        false,
        false,
        Node {
            height: px(24),
            padding: UiRect::axes(px(6), px(0)),
            column_gap: px(5),
            align_items: AlignItems::Center,
            border: UiRect::all(px(1)),
            ..default()
        },
    );
    icon(
        cmd,
        row,
        catalog::tile_art(order.job, run, art),
        cyan(),
        14.,
    );
    text(
        cmd,
        row,
        catalog::order_caption(order.job, run),
        12.,
        paper(),
    );
    spacer(cmd, row);
    if order.cost > 0 {
        icon(cmd, row, art.order.clone(), brass(), 11.);
        text(cmd, row, order.cost.to_string(), 12., brass());
    } else {
        text(cmd, row, "Free", 11., muted());
    }
    icon(cmd, row, art.undo.clone(), muted(), 10.);
}

/// The refits of the selected section, hung under it, with the figures for the one under
/// the cursor beside them.
fn callout(cmd: &mut Commands, root: Entity, run: &Campaign, art: &Insignia) {
    let callout = cmd
        .spawn((
            Callout,
            Solid,
            RelativeCursorPosition::default(),
            Node {
                position_type: PositionType::Absolute,
                display: Display::None,
                padding: UiRect::all(px(12)),
                column_gap: px(14),
                border: UiRect::top(px(2)),
                align_items: AlignItems::FlexStart,
                ..default()
            },
            BackgroundColor(panel()),
            BorderColor::all(brass()),
            ChildOf(root),
        ))
        .id();
    let options = node(
        cmd,
        callout,
        Node {
            flex_direction: FlexDirection::Column,
            min_width: px(170),
            max_width: px(OPTIONS_WIDTH),
            row_gap: px(6),
            ..default()
        },
        Color::NONE,
    );
    let head = node(
        cmd,
        options,
        Node {
            align_items: AlignItems::Center,
            column_gap: px(8),
            ..default()
        },
        Color::NONE,
    );
    let at = run.dock_module;
    let jobs = run.dock_jobs();
    icon(cmd, head, section_art(run, at, art), brass(), 22.);
    text(cmd, head, section_name(run, at), 17., brass());
    spacer(cmd, head);
    let design = run.ship.design();
    if at == 0 || at + 1 >= design.modules().len() {
        let row = node(
            cmd,
            options,
            Node {
                align_items: AlignItems::Center,
                column_gap: px(6),
                ..default()
            },
            Color::NONE,
        );
        icon(cmd, row, art.lock.clone(), muted(), 14.);
        text(cmd, row, "Fixed", 13., muted());
        return;
    }
    // Every main turret aboard, with this bay's own in brass.
    let here = design
        .gun_stations()
        .iter()
        .filter(|g| g.berth.module == at)
        .count();
    proposal::pips(
        cmd,
        head,
        &[(here, brass()), (run.main_turrets() - here, paper())],
    );
    catalog::options(cmd, options, run, art, &jobs);
    // The callout keeps one size whatever is picked, so a second click lands on the tile
    // the first one did.
    cmd.entity(callout).entry::<Node>().and_modify(|mut node| {
        node.min_height = px(CALLOUT_HEIGHT);
    });

    // The figures for the refit picked out. Their column keeps its room when nothing is
    // picked.
    match run.dock_job().filter(|job| jobs.contains(job)) {
        Some(job) => order_card(cmd, callout, run, art, job, true),
        None => {
            let spec = figures(cmd, callout, true);
            text(cmd, spec, "Click a refit", 13., muted());
        }
    }
}

/// Least height of a section's callout: room for its options and the tallest figures.
const CALLOUT_HEIGHT: f32 = 290.;

/// The column a refit's figures stand in, beside the options or on top of the hull panel.
fn figures(cmd: &mut Commands, parent: Entity, column: bool) -> Entity {
    cmd.spawn((
        Node {
            flex_direction: FlexDirection::Column,
            width: px(FIGURES_WIDTH),
            row_gap: px(7),
            padding: if column {
                UiRect::left(px(14))
            } else {
                UiRect::bottom(px(4))
            },
            border: if column {
                UiRect::left(px(1))
            } else {
                UiRect::ZERO
            },
            ..default()
        },
        BorderColor::all(rule()),
        ChildOf(parent),
    ))
    .id()
}

/// A refit's figures: its name, what it changes, and what it costs.
fn order_card(
    cmd: &mut Commands,
    parent: Entity,
    run: &Campaign,
    art: &Insignia,
    job: DockJob,
    column: bool,
) {
    let spec = figures(cmd, parent, column);
    text(cmd, spec, job_name(job, run), 16., paper());
    if catalog::ship_wide(job) {
        text(cmd, spec, "ALL TURRETS", 10., cyan());
    }
    proposal::details(cmd, spec, run, art, job);
    let fits = job.fits(run);
    let cost = job.cost();
    let left = run.points_after(job);
    let enabled = fits && left.is_some();
    // An empty bay that leaves too few points for a turret has nothing to put on it.
    if let Some(left) = left.filter(|_| enabled && run.leaves_bay_empty(job)) {
        badge(
            cmd,
            spec,
            art.order.clone(),
            brass(),
            format!(
                "{left} point{} left: this bay sails empty",
                if left == 1 { "" } else { "s" }
            ),
            12.,
        );
    }
    // A fitting at its last rank is done, not locked. Work the yard can make is ordered
    // from its price.
    let done = catalog::fully_fitted(job, run);
    let price = button(
        cmd,
        spec,
        enabled.then_some(Action::Confirm),
        false,
        true,
        Node {
            height: px(34),
            padding: UiRect::axes(px(10), px(0)),
            column_gap: px(6),
            align_items: AlignItems::Center,
            border: UiRect::all(px(1)),
            ..default()
        },
    );
    let ink = if enabled {
        panel()
    } else if done {
        cyan()
    } else if !fits {
        muted()
    } else {
        cards::Change::Cost.color()
    };
    if !enabled {
        cmd.entity(price)
            .insert((BorderColor::all(ink), BackgroundColor(Color::NONE)));
    }
    if done {
        text(cmd, price, "Fitted", 14., ink);
        return;
    }
    if !fits {
        icon(cmd, price, art.lock.clone(), ink, 14.);
        text(cmd, price, "Locked", 14., ink);
        return;
    }
    if cost > 0 {
        icon(cmd, price, art.order.clone(), ink, 14.);
        text(cmd, price, cost.to_string(), 15., ink);
    } else {
        text(cmd, price, "Free", 14., ink);
    }
    if left.is_none() {
        text(cmd, price, " Short of points", 13., ink);
    } else {
        text(cmd, price, "  Order  [Enter]", 14., ink);
    }
}

/// The floor of the yard: work on the hull as a whole, the stock of whole sections to
/// drag onto the ship, and the way out: build the orders, then sail.
fn floor(cmd: &mut Commands, root: Entity, run: &Campaign, art: &Insignia) {
    let floor = cmd
        .spawn((
            Floor,
            Node {
                position_type: PositionType::Absolute,
                left: px(16),
                right: px(16),
                bottom: px(12),
                align_items: AlignItems::FlexEnd,
                column_gap: px(12),
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    let shelf = |cmd: &mut Commands, accent: Color| {
        cmd.spawn((
            Solid,
            RelativeCursorPosition::default(),
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(10)),
                row_gap: px(7),
                border: UiRect::top(px(2)),
                ..default()
            },
            BackgroundColor(panel()),
            BorderColor::all(accent),
            ChildOf(floor),
        ))
        .id()
    };
    let title = |cmd: &mut Commands, parent: Entity, image: Handle<Image>, words: &str| {
        let row = node(
            cmd,
            parent,
            Node {
                align_items: AlignItems::Center,
                column_gap: px(8),
                ..default()
            },
            Color::NONE,
        );
        icon(cmd, row, image, brass(), 18.);
        text(cmd, row, words, 15., brass());
        row
    };

    // The hull as a whole: how long and how wide she is, and the work on both. The
    // figures for the work picked out float above the panel, so nothing moves.
    let hull = shelf(cmd, brass());
    let picked = run.dock_job().filter(|job| run.hull_jobs().contains(job));
    if let Some(job) = picked {
        let float = cmd
            .spawn((
                Solid,
                RelativeCursorPosition::default(),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    bottom: percent(100),
                    margin: UiRect::bottom(px(6)),
                    padding: UiRect::all(px(10)),
                    border: UiRect::top(px(2)),
                    ..default()
                },
                BackgroundColor(panel()),
                BorderColor::all(brass()),
                ChildOf(hull),
            ))
            .id();
        order_card(cmd, float, run, art, job, false);
    }
    let head = title(cmd, hull, art.hull.clone(), "HULL");
    spacer(cmd, head);
    let design = run.ship.design();
    let figures = node(
        cmd,
        hull,
        Node {
            column_gap: px(12),
            ..default()
        },
        Color::NONE,
    );
    badge(
        cmd,
        figures,
        art.length.clone(),
        paper(),
        format!("{:.0} m", design.length()),
        14.,
    );
    badge(
        cmd,
        figures,
        art.beam.clone(),
        paper(),
        format!("{:.1} m", design.beam()),
        14.,
    );
    // Hull work on order, each with its price, taken back with a click.
    if let Some(plan) = run.plan() {
        let list = node(
            cmd,
            hull,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(3),
                ..default()
            },
            Color::NONE,
        );
        for (index, order) in plan.orders().iter().enumerate() {
            if plan.place(order).is_none() {
                order_row(cmd, list, run, art, index, order);
            }
        }
    }
    let works = node(
        cmd,
        hull,
        Node {
            column_gap: px(4),
            ..default()
        },
        Color::NONE,
    );
    let need = need(run);
    for job in run.hull_jobs() {
        let refused = run.refusal(job);
        let chosen = picked == Some(job);
        let tint = if refused.is_some() || !job.fits(run) {
            rule()
        } else {
            paper()
        };
        // A click picks the work out to read its figures, and a second orders it.
        let tile = button(
            cmd,
            works,
            Some(if chosen {
                Action::Draft(job)
            } else {
                Action::Pick(job)
            }),
            chosen,
            false,
            Node {
                width: px(64),
                height: px(50),
                border: UiRect::all(px(1)),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(3),
                ..default()
            },
        );
        if need == Some(Need::Job(job)) {
            cmd.entity(tile).insert(Beckon);
        }
        let (image, words) = match job {
            DockJob::Fitting(card) => (
                art.fitting(card),
                if job.fits(run) {
                    catalog::caption(job, run).to_string()
                } else {
                    "Full".to_string()
                },
            ),
            _ => (
                art.beam.clone(),
                match (refused, run.beam_class().next()) {
                    (Some(Refusal::Widest), _) | (_, None) => "Widest".to_string(),
                    (_, Some(next)) => format!("{:.1} m", next.metres()),
                },
            ),
        };
        icon(cmd, tile, image, tint, 22.);
        if refused.is_none() && job.fits(run) {
            let ink = if run.points_after(job).is_none() {
                cards::Change::Cost.color()
            } else {
                brass()
            };
            price_tag(cmd, tile, art, job.cost(), ink);
        }
        // A step the hull is not ready for shows what it waits on, as the stock tiles do:
        // thicker armor on a broader beam, and a broader beam on a longer hull.
        let waits = match refused {
            Some(Refusal::Beam(beam)) => Some((art.beam.clone(), format!("{:.1}", beam.metres()))),
            Some(Refusal::Length(metres)) => Some((art.length.clone(), metres.to_string())),
            _ => None,
        };
        match waits {
            Some((image, words)) => {
                let caption = node(
                    cmd,
                    tile,
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: px(3),
                        ..default()
                    },
                    Color::NONE,
                );
                icon(cmd, caption, art.lock.clone(), muted(), 9.);
                icon(cmd, caption, image, muted(), 9.);
                text(cmd, caption, words, 10., muted());
            }
            None => text(cmd, tile, words, 10., tint),
        }
    }

    // The yard's stock: whole sections, each dragged to the seam it should go into.
    let tray = shelf(cmd, rule());
    let head = title(cmd, tray, art.add_bay.clone(), "MODULES");
    text(cmd, head, "Drag onto the ship", 12., cyan());
    let row = node(
        cmd,
        tray,
        Node {
            column_gap: px(6),
            ..default()
        },
        Color::NONE,
    );
    for module in DockModule::ALL {
        let refused = run.stock_refusal(module);
        let tint = if refused.is_some() { rule() } else { paper() };
        let tile = cmd
            .spawn((
                StockTile {
                    module,
                    refused: refused.is_some(),
                },
                RelativeCursorPosition::default(),
                Node {
                    width: px(72),
                    height: px(62),
                    border: UiRect::all(px(1)),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    row_gap: px(3),
                    ..default()
                },
                BackgroundColor(panel()),
                BorderColor::all(rule()),
                ChildOf(row),
            ))
            .id();
        if need == Some(Need::Stock(module)) {
            cmd.entity(tile).insert(Beckon);
        }
        // A price the yard cannot pay is red.
        let ink = if refused.is_some() {
            rule()
        } else if module.cost() > run.points() {
            cards::Change::Cost.color()
        } else {
            brass()
        };
        price_tag(cmd, tile, art, module.cost(), ink);
        icon(cmd, tile, catalog::stock_art(module, art), tint, 24.);
        let caption = node(
            cmd,
            tile,
            Node {
                align_items: AlignItems::Center,
                column_gap: px(3),
                ..default()
            },
            Color::NONE,
        );
        match refused {
            None => text(cmd, caption, catalog::stock_caption(module), 10., tint),
            Some(why) => {
                icon(cmd, caption, art.lock.clone(), muted(), 9.);
                let (image, words) = match why {
                    Refusal::Beam(beam) => {
                        (Some(art.beam.clone()), format!("{:.1}", beam.metres()))
                    }
                    Refusal::Longest => (Some(art.length.clone()), "Max".to_string()),
                    Refusal::Full(_) | Refusal::Length(_) | Refusal::Widest => {
                        (None, "Full".to_string())
                    }
                };
                if let Some(image) = image {
                    icon(cmd, caption, image, muted(), 9.);
                }
                text(cmd, caption, words, 10., muted());
            }
        }
    }

    spacer(cmd, floor);
    finish(cmd, floor, run, art);
}

/// The way out of the yard, big enough to find at a glance, with the points left to spend
/// above it: lock in the orders, then sail. It does its own work whatever refit is picked
/// out, and says that points left over are banked for the next yard.
fn finish(cmd: &mut Commands, parent: Entity, run: &Campaign, art: &Insignia) {
    let plan = run.plan();
    let points = run.points();
    let plural = |n: usize| if n == 1 { "" } else { "s" };
    let stack = node(
        cmd,
        parent,
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            ..default()
        },
        Color::NONE,
    );
    // The points left once the orders are built, and what the orders cost.
    let purse = cmd
        .spawn((
            Solid,
            RelativeCursorPosition::default(),
            Node {
                padding: UiRect::axes(px(12), px(7)),
                column_gap: px(10),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::top(px(2)),
                ..default()
            },
            BackgroundColor(panel()),
            BorderColor::all(brass()),
            ChildOf(stack),
        ))
        .id();
    badge(
        cmd,
        purse,
        art.order.clone(),
        brass(),
        format!("{points} point{}", plural(points.into())),
        18.,
    );
    if let Some(plan) = plan {
        text(
            cmd,
            purse,
            format!("({} on order)", plan.cost()),
            13.,
            cyan(),
        );
    }
    let big = button(
        cmd,
        stack,
        Some(Action::Sail),
        false,
        true,
        Node {
            min_width: px(250),
            height: px(84),
            padding: UiRect::axes(px(26), px(8)),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: px(5),
            border: UiRect::all(px(2)),
            ..default()
        },
    );
    cmd.entity(big)
        .insert((Solid, BorderColor::all(Color::srgb_u8(255, 226, 160))));
    let line = |cmd: &mut Commands| {
        node(
            cmd,
            big,
            Node {
                align_items: AlignItems::Center,
                column_gap: px(8),
                ..default()
            },
            Color::NONE,
        )
    };
    let top = line(cmd);
    let foot = line(cmd);
    if let Some(plan) = plan {
        let orders = plan.orders().len();
        icon(cmd, top, art.lock.clone(), panel(), 24.);
        text(cmd, top, "LOCK IN", 26., panel());
        // What the orders leave unspent is banked too, so say so before she locks in,
        // not only after: a lone point that buys nothing here is not lost.
        let banked = if points > 0 {
            format!(", {points} banked")
        } else {
            String::new()
        };
        text(
            cmd,
            foot,
            format!("{orders} order{}{banked}   [Enter]", plural(orders)),
            14.,
            panel(),
        );
    } else {
        icon(cmd, top, art.hull.clone(), panel(), 28.);
        text(cmd, top, "SAIL", 28., panel());
        // Points left over are banked for the next yard, not lost.
        if points > 0 {
            text(
                cmd,
                foot,
                format!("{points} point{} banked", plural(points.into())),
                14.,
                panel(),
            );
        }
        text(cmd, foot, "[Enter]", 14., panel());
    }
}

/// The train of one main turret on the selected section, laid flat on the deck.
#[derive(Component)]
pub struct Fan(usize);

/// Metres from a turret to the rim of its traverse fan.
const FAN_REACH: f32 = 8.5;

/// A flat fan over the bearings a turret can train to: a pale sector with a bright rim
/// and bright edges where its arc stops. Bearings are from the bow, to starboard.
pub fn fan_mesh(station: &shipvivor_shipyard::GunStation, reach: f32) -> Mesh {
    use std::f32::consts::TAU;
    const STEPS: usize = 180;
    let fill = Color::srgba(0.49, 0.90, 0.85, 0.26);
    let edge = Color::srgba(0.55, 0.95, 0.90, 0.85);
    let along = |bearing: f32, r: f32| Vec3::new(bearing.sin() * r, 0., -bearing.cos() * r);
    let bearing = |i: usize| i as f32 * TAU / STEPS as f32;
    let mut mesh = crate::scene::Facets::default();
    let bears = |i: usize| station.bears(bearing(i % STEPS) + 0.5 * TAU / STEPS as f32);
    for i in 0..STEPS {
        if !bears(i) {
            continue;
        }
        let (a, b) = (bearing(i), bearing(i + 1));
        mesh.triangle([Vec3::ZERO, along(b, reach), along(a, reach)], fill);
        mesh.quad(
            [
                along(a, reach * 0.95),
                along(a, reach),
                along(b, reach),
                along(b, reach * 0.95),
            ],
            edge,
        );
        // Where the arc stops, a bright edge runs out to the rim.
        for (side, open) in [(a, !bears(i + STEPS - 1)), (b, !bears(i + 1))] {
            if open {
                let across = Vec3::new(side.cos(), 0., side.sin()) * 0.06;
                let tip = along(side, reach);
                mesh.quad([-across, across, tip + across, tip - across], edge);
            }
        }
    }
    mesh.finish()
}

/// Lay the traverse fans of the selected section's main turrets on the deck, as they
/// stand or as the refit under the cursor would leave them.
#[allow(clippy::type_complexity)]
pub fn fans(
    run: Res<Campaign>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut fans: Query<(&Fan, &mut Mesh3d, &mut Transform, &mut Visibility)>,
    mut last: Local<Option<(u32, usize, Option<DockJob>)>>,
) {
    if !matches!(run.screen(), Screen::Drydock(_)) {
        for (_, _, _, mut shown) in &mut fans {
            shown.set_if_neq(Visibility::Hidden);
        }
        *last = None;
        return;
    }
    let job = run.dock_job();
    let key = (run.dock_revision, run.dock_module, job);
    if *last == Some(key) {
        return;
    }
    *last = Some(key);
    let reshapes = |job: &DockJob| {
        matches!(
            job,
            DockJob::Barbette { .. } | DockJob::Move { .. } | DockJob::Replace { .. }
        )
    };
    let projected = job
        .filter(reshapes)
        .and_then(|job| run.projected_design(job));
    let design = projected.as_ref().unwrap_or(run.ship.design());
    let at = match job.filter(|_| projected.is_some()) {
        Some(DockJob::Move { to, .. }) => to,
        _ => run.dock_module,
    };
    let stations: Vec<_> = design
        .gun_stations()
        .into_iter()
        .filter(|g| g.berth.module == at)
        .collect();
    for (fan, mut mesh, mut pose, mut shown) in &mut fans {
        let Some(station) = stations.get(fan.0) else {
            shown.set_if_neq(Visibility::Hidden);
            continue;
        };
        *mesh = Mesh3d(meshes.add(fan_mesh(station, FAN_REACH)));
        *pose = Transform::from_xyz(station.abeam, 2.0 + station.lift(), -station.fore);
        shown.set_if_neq(Visibility::Inherited);
    }
}

fn header(
    cmd: &mut Commands,
    root: Entity,
    run: &Campaign,
    battle: &Encounter,
    art: &Insignia,
    reason: LegEnd,
) {
    let header = cmd
        .spawn((
            Solid,
            RelativeCursorPosition::default(),
            Node {
                position_type: PositionType::Absolute,
                top: px(0),
                left: px(0),
                right: px(0),
                padding: UiRect::axes(px(24), px(10)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                border: UiRect::bottom(px(2)),
                ..default()
            },
            BackgroundColor(panel()),
            BorderColor::all(brass()),
            ChildOf(root),
        ))
        .id();
    let column = |cmd: &mut Commands, align: AlignItems| {
        node(
            cmd,
            header,
            Node {
                flex_direction: FlexDirection::Column,
                align_items: align,
                row_gap: px(4),
                ..default()
            },
            Color::NONE,
        )
    };
    let line = |cmd: &mut Commands, parent: Entity| {
        node(
            cmd,
            parent,
            Node {
                align_items: AlignItems::Center,
                column_gap: px(12),
                ..default()
            },
            Color::NONE,
        )
    };

    // Her length and beam are on the hull panel, with the work that changes them.
    let name = column(cmd, AlignItems::Start);
    text(cmd, name, "DRYDOCK", 27., brass());
    let ship = line(cmd, name);
    badge(cmd, ship, art.hull.clone(), paper(), "BREAKWATER", 14.);
    text(cmd, name, "Click the ship to refit", 12., cyan());

    let sortie = column(cmd, AlignItems::Center);
    text(cmd, sortie, leg_result(reason), 19., paper());
    let tally = line(cmd, sortie);
    let stars = node(
        cmd,
        tally,
        Node {
            column_gap: px(3),
            ..default()
        },
        Color::NONE,
    );
    for i in 0..shipvivor_campaign::MAX_STARS {
        let earned = i < run.stars();
        icon(
            cmd,
            stars,
            art.star.clone(),
            if earned { brass() } else { rule() },
            16.,
        );
    }
    text(
        cmd,
        tally,
        format!(
            "{} / {} bases",
            if reason == LegEnd::BasesDestroyed {
                battle.ports().len()
            } else {
                battle.ruined_yards()
            },
            battle.ports().len()
        ),
        13.,
        muted(),
    );
    // The clock pays on top of the stars, or says it was missed.
    if run.beat_clock() {
        text(
            cmd,
            tally,
            format!("Clock beaten  +{} pts", run.clock_points()),
            13.,
            brass(),
        );
    } else {
        text(cmd, tally, "Past the clock", 13., muted());
    }

    // Her hull, as the dock has mended it. The points to spend stand over the way out.
    let orders = column(cmd, AlignItems::End);
    let hull = line(cmd, orders);
    let percent = run.hull_percent();
    icon(cmd, hull, art.hull.clone(), muted(), 14.);
    let gauge = node(
        cmd,
        hull,
        Node {
            width: px(90),
            height: px(8),
            ..default()
        },
        rule(),
    );
    node(
        cmd,
        gauge,
        Node {
            width: percent_of(percent),
            height: percent_of(100),
            ..default()
        },
        if percent >= 70 {
            Color::srgb_u8(126, 240, 160)
        } else if percent >= 40 {
            brass()
        } else {
            Color::srgb_u8(255, 122, 102)
        },
    );
    text(cmd, hull, format!("{percent}%"), 13., paper());
}

fn percent_of(value: u16) -> Val {
    Val::Percent(f32::from(value.min(100)))
}

/// Project both ends of the frame a carried section would make, under the seam it is
/// over.
pub fn dimension(
    run: Res<Campaign>,
    pointer: Res<Pointer>,
    scale: Res<UiScale>,
    camera: Single<(&Camera, &GlobalTransform), With<crate::camera::FollowCamera>>,
    mut labels: Query<&mut Node, With<Dimension>>,
) {
    let (camera, pose) = *camera;
    let carried = pointer.carrying.zip(pointer.gap).and_then(|(module, at)| {
        let job = DockJob::Insert { at, module };
        run.projected_design(job).map(|design| (design, at))
    });
    for mut label in &mut labels {
        let Some((design, at)) = &carried else {
            label.display = Display::None;
            continue;
        };
        let point = |aft: f32| {
            camera
                .world_to_viewport(
                    pose,
                    BERTH + Vec3::new(-0.5 * design.beam() - 0.35, -1.05, aft - Hull::CRUISER.bow),
                )
                .ok()
                .map(|p| p / scale.0)
        };
        let (fore, aft) = design.span(*at);
        let (Some(a), Some(b)) = (point(fore), point(aft)) else {
            label.display = Display::None;
            continue;
        };
        label.display = Display::Flex;
        label.left = px(a.x.min(b.x));
        label.top = px(a.y.max(b.y) + 4.);
        label.width = px((a.x - b.x).abs());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_along_the_yard_camera_picks_the_section_under_it() {
        let run = Campaign::default();
        let design = run.ship.design();
        let look = *camera(&run).0.forward();
        for at in 0..design.modules().len() {
            let (centre, _) = section_box(design, at);
            let deck = centre + Vec3::Y;
            assert_eq!(section_at(design, deck - look * 80., look), Some(at));
        }
        // Open water beside the hull is not the ship.
        let water = coords(-design.beam() - 6., 0., 0.);
        assert_eq!(section_at(design, water - look * 80., look), None);
    }

    #[test]
    fn a_turret_fan_covers_the_bearings_it_trains_to_and_none_it_cannot() {
        let run = Campaign::default();
        let guns = run.ship.design().gun_stations();
        // The forward turret trains across the bow, and not back over the bridge.
        let fore = guns[0];
        assert!(fore.bears(0.) && !fore.bears(std::f32::consts::PI));
        let mesh = fan_mesh(&fore, FAN_REACH);
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(points)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("a fan has positions");
        };
        let mut ahead = false;
        for p in points.iter().map(|p| Vec3::from_array(*p)) {
            if p.length() < 0.5 {
                continue;
            }
            // Bearings from the bow, to starboard, as the fan is laid out.
            let bearing = p.x.atan2(-p.z);
            let step = std::f32::consts::TAU / 180.;
            assert!(
                fore.bears(bearing) || fore.bears(bearing + step) || fore.bears(bearing - step),
                "{bearing}"
            );
            ahead |= bearing.abs() < 0.1;
        }
        assert!(ahead, "the fan reaches dead ahead");
    }

    #[test]
    fn a_section_carried_over_the_ship_goes_into_the_nearest_seam_between_bow_and_stern() {
        let run = Campaign::default();
        let design = run.ship.design();
        let look = *camera(&run).0.forward();
        let last = design.modules().len() - 1;
        let over = |aft: f32| {
            let deck = coords(0., 2.5, aft - Hull::CRUISER.bow);
            gap_at(design, deck - look * 80., look)
        };
        for at in 1..=last {
            let seam = design.span(at).0;
            assert_eq!(over(seam + 0.4), Some(at));
            assert_eq!(over(seam - 0.4), Some(at));
        }
        // Ahead of the stem a section still goes in behind the bow, and astern of the
        // transom ahead of the stern. Bow and stern stay at the ends.
        assert_eq!(over(-1.), Some(1));
        assert_eq!(over(design.length() + 1.), Some(last));
        // Well clear of her there is no seam to drop into.
        assert_eq!(over(-20.), None);
        let sky = coords(0., 30., 0.);
        assert_eq!(gap_at(design, sky - look * 80., look), None);
    }
}
