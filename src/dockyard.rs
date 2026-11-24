//! The intermission yard. The ship itself is the menu: select a section of her and its
//! refits hang beneath it. Only confirmation alters the ship that will sail.
use crate::{
    cards,
    insignia::Insignia,
    state::{Campaign, Encounter, Screen},
    ui::{Action, leg_result},
};
use bevy::{camera::ScalingMode, prelude::*, ui::RelativeCursorPosition, window::PrimaryWindow};
use shipvivor_campaign::{Card, DeckModule, DockJob, DockModule, LegEnd, ShipBeam};
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

/// The section under the cursor, and whether the cursor is over a yard panel instead.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
pub struct Pointer {
    pub hovered: Option<usize>,
    pub over_panel: bool,
}

pub fn point(
    run: Res<Campaign>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<crate::camera::FollowCamera>>,
    panels: Query<&RelativeCursorPosition, With<Solid>>,
    mut pointer: ResMut<Pointer>,
) {
    let mut next = Pointer::default();
    if matches!(run.screen(), Screen::Drydock(_)) {
        next.over_panel = panels.iter().any(RelativeCursorPosition::cursor_over);
        if !next.over_panel
            && let (Ok(window), Ok((camera, pose))) = (window.single(), camera.single())
            && let Some(cursor) = window.cursor_position()
            && let Ok(ray) = camera.viewport_to_world(pose, cursor)
        {
            next.hovered = section_at(run.ship.design(), ray.origin - BERTH, *ray.direction);
        }
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
        DockJob::Section(_) if fits => (
            coords(0., 1.25, Hull::SPLICE_Z + run.added_length() + 2.5),
            coords(design.beam() + 0.2, 4.5, 5.),
        ),
        DockJob::WidenBeam if fits => (
            coords(0., 0.9, Hull::SPLICE_Z - 2.5),
            coords(
                run.next_beam().unwrap_or(run.beam_class()).metres() + 0.2,
                3.8,
                5.,
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
        DockJob::Fitting(Card::MoreSecondaries | Card::TwinSecondaries | Card::SecondaryRange)
            if fits =>
        {
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
        DockJob::Fitting(Card::Turbines) if fits => {
            let deck = shipvivor_encounter::DeckOffsets::for_design(design);
            (
                coords(0., 2.4, 1.1 + deck.machinery_z),
                coords(3.4, 4.8, 4.5),
            )
        }
        DockJob::Fitting(_) | DockJob::Calibre(_) | DockJob::TwinTurrets if fits => {
            // Bore, barrels and shells change every turret. Show the one on this bay.
            let guns = design.gun_stations();
            let mount = guns
                .iter()
                .find(|g| g.berth.module == run.dock_module)
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
                1.72 + 0.7 * scale,
                -mount.fore - mount.home.cos() * 1.4 * scale,
            );
            (at, local_size * scale)
        }
        _ => return None,
    })
}

pub fn stage(
    run: Res<Campaign>,
    pointer: Res<Pointer>,
    mut basin: Single<&mut Visibility, (With<Basin>, Without<Cage>)>,
    mut cages: Query<(&Cage, &mut Transform, &mut Visibility), Without<Basin>>,
    mut buttons: Query<(&YardButton, &RelativeCursorPosition, &mut BackgroundColor)>,
) {
    if !matches!(run.screen(), Screen::Drydock(_)) {
        basin.set_if_neq(Visibility::Hidden);
        return;
    }
    basin.set_if_neq(Visibility::Inherited);
    let design = run.ship.design();
    let job = run.dock_job();
    // A bay refit already outlines the bay, so the selection steps aside for it.
    let bay_work = matches!(job, Some(DockJob::Move { .. } | DockJob::Replace { .. }));
    let last = design.modules().len();
    for (kind, mut pose, mut visible) in &mut cages {
        let placed = match kind {
            Cage::Proposal => job.and_then(|job| proposal_box(&run, job)),
            Cage::Selection if !bay_work && run.dock_module < last => {
                Some(section_box(design, run.dock_module))
            }
            Cage::Selection => None,
            Cage::Hover => pointer
                .hovered
                .filter(|&at| at != run.dock_module && at < last)
                .map(|at| section_box(design, at)),
        };
        if let Some((at, size)) = placed {
            *pose = Transform::from_translation(at).with_scale(size);
            visible.set_if_neq(Visibility::Inherited);
        } else {
            visible.set_if_neq(Visibility::Hidden);
        }
    }
    for (button, mouse, mut bg) in &mut buttons {
        bg.set_if_neq(BackgroundColor(button_paint(
            button.selected,
            button.confirm,
            mouse.cursor_over(),
        )));
    }
}

/// Keep the callout under its section, joined to the hull by a leader, and name the
/// section under the cursor above it.
#[allow(clippy::type_complexity)]
pub fn place(
    run: Res<Campaign>,
    pointer: Res<Pointer>,
    scale: Res<UiScale>,
    camera: Single<(&Camera, &GlobalTransform), With<crate::camera::FollowCamera>>,
    mut callout: Query<
        (&mut Node, &ComputedNode),
        (With<Callout>, Without<Leader>, Without<HoverTag>),
    >,
    mut leader: Query<&mut Node, (With<Leader>, Without<Callout>, Without<HoverTag>)>,
    mut tag: Query<(&mut Node, &ComputedNode), (With<HoverTag>, Without<Callout>, Without<Leader>)>,
    mut label: Query<&mut Text, With<HoverText>>,
    mut glyph: Query<&mut ImageNode, With<HoverIcon>>,
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
    let anchor = (run.dock_module < design.modules().len())
        .then(|| section_box(design, run.dock_module).0)
        .and_then(|centre| project(coords(-0.5 * design.beam(), -0.6, centre.z)));
    for (mut node, computed) in &mut callout {
        let Some(anchor) = anchor else {
            node.display = Display::None;
            continue;
        };
        let extent = size(computed);
        // The options column sits under the section; the panel stays on screen.
        let left =
            (anchor.x - 0.5 * OPTIONS_WIDTH - 12.).clamp(12., (screen.x - extent.x - 12.).max(12.));
        let top = (anchor.y + 34.).min(screen.y - extent.y - 12.);
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
        .filter(|&at| at != run.dock_module && at < design.modules().len());
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

/// What the hover tag and the callout call a section.
fn section_name(run: &Campaign, at: usize) -> &'static str {
    let design = run.ship.design();
    if at == 0 || at + 1 >= design.modules().len() {
        "Hull"
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
        DockJob::Section(DockModule::Reserve) => "Add reserve bay".into(),
        DockJob::Section(DockModule::EngineRoom) => "Add auxiliary engine".into(),
        DockJob::WidenBeam => "Widen beam".into(),
        DockJob::TwinTurrets => "Twin turrets".into(),
        DockJob::Fitting(card) => cards::face(card, run.fittings).name.into(),
        DockJob::Calibre(inches) => format!("{inches}-inch main battery"),
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
    if run.preview_section().is_some() {
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
    }
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

    // The selected section's refits, hung under the section itself.
    let at = run.dock_module;
    let jobs = run.dock_jobs();
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
    icon(cmd, head, section_art(run, at, art), brass(), 22.);
    text(cmd, head, section_name(run, at), 17., brass());
    spacer(cmd, head);
    let design = run.ship.design();
    // The hull's length and beam are in the header already.
    if at != 0 && at + 1 < design.modules().len() {
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
    }
    catalog::options(cmd, options, run, art, &jobs);

    // The preview column opens only once a refit is chosen.
    let Some(job) = run.dock_job() else {
        return;
    };
    let spec = cmd
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                width: px(250),
                row_gap: px(7),
                padding: UiRect::left(px(14)),
                border: UiRect::left(px(1)),
                ..default()
            },
            BorderColor::all(rule()),
            ChildOf(callout),
        ))
        .id();
    text(cmd, spec, job_name(job, run), 16., paper());
    if catalog::ship_wide(job) {
        text(cmd, spec, "ALL TURRETS", 10., cyan());
    }
    proposal::details(cmd, spec, run, art, job);
    let fits = job.fits(run);
    let cost = job.cost();
    let affordable = cost <= run.fittings_remaining();
    let enabled = fits && affordable;
    // Spending the last order sails the ship, so that press is asked for twice.
    let last = enabled && run.closes_dock(job);
    if last {
        let (warning, color) = if run.dock_armed {
            (
                "Press again to close the dock",
                Color::srgb_u8(255, 148, 123),
            )
        } else if job == DockJob::Section(DockModule::Reserve) {
            ("Last order: this bay sails empty", brass())
        } else {
            ("Last order: the dock closes after", brass())
        };
        badge(cmd, spec, art.order.clone(), color, warning, 12.);
    }
    let submit = button(
        cmd,
        spec,
        enabled.then_some(Action::Confirm),
        false,
        true,
        Node {
            height: px(36),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            column_gap: px(4),
            border: UiRect::all(px(1)),
            ..default()
        },
    );
    let ink = if enabled { panel() } else { muted() };
    if !fits {
        icon(cmd, submit, art.lock.clone(), ink, 16.);
    } else {
        // The price in work orders, one spanner each.
        for _ in 0..cost {
            icon(cmd, submit, art.order.clone(), ink, 16.);
        }
    }
    text(
        cmd,
        submit,
        match (fits, affordable) {
            (false, _) => " Locked",
            (_, false) => " Short of orders",
            _ if last && run.dock_armed => " Close dock  [Enter]",
            _ if cost == 0 => "Free  [Enter]",
            _ => " Fit  [Enter]",
        },
        15.,
        ink,
    );
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
    let design = run.ship.design();

    let name = column(cmd, AlignItems::Start);
    text(cmd, name, "DRYDOCK", 27., brass());
    let ship = line(cmd, name);
    badge(cmd, ship, art.hull.clone(), paper(), "BREAKWATER", 14.);
    badge(
        cmd,
        ship,
        art.length.clone(),
        paper(),
        format!("{:.0} m", design.length()),
        14.,
    );
    badge(
        cmd,
        ship,
        art.beam.clone(),
        paper(),
        format!("{:.1} m", design.beam()),
        14.,
    );
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

    let orders = column(cmd, AlignItems::End);
    let tickets = line(cmd, orders);
    badge(
        cmd,
        tickets,
        art.order.clone(),
        brass(),
        format!(
            "{} work order{}",
            run.fittings_remaining(),
            if run.fittings_remaining() == 1 {
                ""
            } else {
                "s"
            }
        ),
        18.,
    );
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

/// Project both ends of the same frame that the dock crew will splice in.
pub fn dimension(
    run: Res<Campaign>,
    scale: Res<UiScale>,
    camera: Single<(&Camera, &GlobalTransform), With<crate::camera::FollowCamera>>,
    mut labels: Query<&mut Node, With<Dimension>>,
) {
    let (camera, pose) = *camera;
    for mut label in &mut labels {
        let fore = Hull::SPLICE_Z + run.added_length();
        let point = |z| {
            camera
                .world_to_viewport(
                    pose,
                    BERTH + Vec3::new(-0.5 * run.ship.design().beam() - 0.35, -1.05, z),
                )
                .ok()
                .map(|p| p / scale.0)
        };
        let (Some(a), Some(b)) = (
            point(fore),
            point(fore + shipvivor_shipyard::Frame::Straight.spec().length),
        ) else {
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
}
