//! The main battery board: a deck plan of Breakwater whose mounts train with the real ones,
//! each barrel filling as it reloads, and a readout per barrel to a tenth of a second.
use crate::{
    camera::ShipView,
    gun_sight::ink,
    state::{Campaign, Encounter, Guns, PlayerHelm},
};
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use shipvivor_encounter::{Ammo, MainMount, MainShell, MountStatus, Station, TurretLayout, Volley};
use shipvivor_shipyard::{Component as Equipment, Design, Frame};

/// Plan pixels per metre across and along the hull. The beam is drawn a little wide to read.
const SCALE: Vec2 = Vec2::new(5., 4.);
const PLAN: UVec2 = UVec2::new(40, 128);
/// The frame leaves room for barrels trained abeam to reach past the hull.
const FRAME: Vec2 = Vec2::new(64., 128.);
/// The frame pixel over the ship's origin.
const ORIGIN: Vec2 = Vec2::new(FRAME.x / 2., 67.);
/// A turret icon is a square this wide, pivoting about its centre.
const TURRET: f32 = 30.;
const HOUSE: f32 = 12.;
const BARREL: Vec2 = Vec2::new(3., TURRET / 2. - HOUSE / 2. + 2.);
/// The sight pointer reaches to this ellipse, just clear of the hull.
const RIM: Vec2 = Vec2::new(26., 66.);

#[derive(Default)]
pub struct DeckPlan {
    cached: Option<(std::sync::Arc<Design>, Handle<Image>)>,
}
impl DeckPlan {
    pub fn image(&mut self, run: &Campaign, images: &mut Assets<Image>) -> Handle<Image> {
        let design = run.ship.design();
        if let Some((installed, image)) = &self.cached
            && std::sync::Arc::ptr_eq(installed, design)
        {
            return image.clone();
        }
        let image = images.add(plan_image(design));
        self.cached = Some((design.clone(), image.clone()));
        image
    }
}

#[derive(Component)]
pub struct PlanTurret(Station);
#[derive(Component)]
pub struct PlanHouse(Station);
#[derive(Component)]
pub struct PlanBarrel(Station, usize);
/// Where the sight looks relative to the bow: a line from amidships and a diamond at its tip.
#[derive(Component)]
pub enum SightMark {
    Line,
    Tip,
}
#[derive(Component)]
pub enum Readout {
    Tag(Station, usize, &'static str),
    Bar(Station, usize),
    State(Station, usize),
    Summary,
    /// What the crew loads next, once AP is fitted.
    Selection,
}

fn navy() -> Color {
    Color::srgba_u8(13, 32, 42, 246)
}
fn brass() -> Color {
    Color::srgb_u8(255, 199, 91)
}
fn trim() -> Color {
    Color::srgb_u8(47, 70, 77)
}

/// Even-odd test against the hull outline, in hull metres with the bow at -Z.
fn inside(outline: &[(f32, f32)], p: Vec2) -> bool {
    let mut odd = false;
    for i in 0..outline.len() {
        let a = Vec2::from(outline[i]);
        let b = Vec2::from(outline[(i + 1) % outline.len()]);
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            odd = !odd;
        }
    }
    odd
}
fn edge_distance(outline: &[(f32, f32)], p: Vec2) -> f32 {
    (0..outline.len())
        .map(|i| {
            let a = Vec2::from(outline[i]);
            let b = Vec2::from(outline[(i + 1) % outline.len()]);
            let t = ((p - a).dot(b - a) / (b - a).length_squared()).clamp(0., 1.);
            p.distance(a + (b - a) * t)
        })
        .fold(f32::MAX, f32::min)
}
/// The paint at one point of the plan: rim, deck, bridge and funnels as the 3D ship has them.
fn paint(
    outline: &[(f32, f32)],
    design: &Design,
    offsets: shipvivor_encounter::DeckOffsets,
    p: Vec2,
) -> Option<[f32; 3]> {
    if !inside(outline, p) {
        return None;
    }
    let funnel = [0.5_f32, 2.75]
        .iter()
        .any(|z| Vec2::new(p.x / 0.875, (p.y - offsets.machinery_z - z) / 1.15).length() <= 1.);
    let aft = p.y + shipvivor_encounter::Hull::CRUISER.bow;
    let i = design.module_at(aft);
    let module = &design.modules()[i];
    let (a, b) = design.span(i);
    let z = aft - (a + b) * 0.5;
    if module.frame == Frame::Straight {
        if module.fit.is_empty()
            && p.x.abs() < 1.55 * offsets.beam_scale
            && (z.abs() - 0.85).abs() < 0.75
        {
            return Some([198., 196., 159.]);
        }
        if module.fit.as_slice() == [Equipment::Engine]
            && Vec2::new(p.x.abs() / offsets.beam_scale - 0.82, z - 0.4).length() < 0.62
        {
            return Some([202., 157., 73.]);
        }
    }
    Some(if funnel {
        [46., 60., 66.]
    } else if p.x.abs() <= 1.85 && (-5.1..=0.9).contains(&(p.y - offsets.bridge_z)) {
        [139., 160., 164.]
    } else if edge_distance(outline, p) < 0.35 {
        [164., 194., 192.]
    } else {
        [88., 114., 120.]
    })
}

fn plan_scale(design: &Design) -> Vec2 {
    Vec2::new(
        SCALE.x * design.beam_factor().min(1.35) / design.beam_factor(),
        SCALE.y,
    )
}
fn plan_image(design: &Design) -> Image {
    let hull = shipvivor_encounter::Hull::breakwater(design);
    let extension = hull.extension();
    let outline = hull.outline();
    let offsets = shipvivor_encounter::DeckOffsets::for_design(design);
    let scale = plan_scale(design);
    let height = PLAN.y + (extension * SCALE.y) as u32;
    let mut image = Image::new_fill(
        Extent3d {
            width: PLAN.x,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0; 4],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    let origin = ORIGIN - Vec2::new((FRAME.x - PLAN.x as f32) / 2., 0.);
    if let Some(pixels) = image.data.as_mut() {
        // Four by four samples a pixel keep the narrow bow smooth.
        for y in 0..height {
            for x in 0..PLAN.x {
                let mut sum = [0.; 3];
                let mut hits = 0.;
                for sy in 0..4 {
                    for sx in 0..4 {
                        let at = Vec2::new(x as f32, y as f32)
                            + (Vec2::new(sx as f32, sy as f32) + 0.5) / 4.;
                        if let Some(rgb) = paint(&outline, design, offsets, (at - origin) / scale) {
                            for (s, c) in sum.iter_mut().zip(rgb) {
                                *s += c;
                            }
                            hits += 1.;
                        }
                    }
                }
                if hits > 0. {
                    let i = ((y * PLAN.x + x) * 4) as usize;
                    let rgb = sum.map(|s| (s / hits).round() as u8);
                    let alpha = (hits / 16. * 255.).round() as u8;
                    pixels[i..i + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], alpha]);
                }
            }
        }
    }
    image
}

pub fn spawn(cmd: &mut Commands, root: Entity, plan: Handle<Image>, run: &Campaign) {
    let design = run.ship.design();
    let positions = design.gun_stations();
    let scale = plan_scale(design);
    let extension = run.added_length();
    let extra_pixels = extension * SCALE.y;
    let panel = cmd
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(-3),
                bottom: percent(100),
                margin: UiRect::bottom(px(8)),
                padding: UiRect::axes(px(10), px(8)),
                border: UiRect::left(px(3)),
                column_gap: px(10),
                ..default()
            },
            BackgroundColor(navy()),
            BorderColor::all(brass()),
            ChildOf(root),
        ))
        .id();
    let frame = cmd
        .spawn((
            Node {
                width: px(FRAME.x),
                height: px(FRAME.y + extra_pixels),
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    cmd.spawn((
        ImageNode::new(plan),
        Node {
            position_type: PositionType::Absolute,
            left: px((FRAME.x - PLAN.x as f32) / 2.),
            top: px(0),
            width: px(PLAN.x as f32),
            height: px(PLAN.y as f32 + extra_pixels),
            ..default()
        },
        ChildOf(frame),
    ));
    let line = cmd
        .spawn((
            SightMark::Line,
            Node {
                position_type: PositionType::Absolute,
                width: px(1.5),
                ..default()
            },
            UiTransform::default(),
            ChildOf(frame),
        ))
        .id();
    // The line pivots about its centre, so only its upper half is drawn.
    cmd.spawn((
        Node {
            width: percent(100),
            height: percent(50),
            ..default()
        },
        BackgroundColor(brass().with_alpha(0.7)),
        ChildOf(line),
    ));
    cmd.spawn((
        SightMark::Tip,
        Node {
            position_type: PositionType::Absolute,
            width: px(6),
            height: px(6),
            ..default()
        },
        BackgroundColor(brass()),
        UiTransform::from_rotation(Rot2::degrees(45.)),
        ChildOf(frame),
    ));
    for (&station, position) in Station::SLOTS.iter().zip(&positions) {
        let layout = run.fittings.for_station(position).turrets;
        let at = ORIGIN + Vec2::new(position.abeam, -position.fore) * scale - TURRET / 2.;
        let turret = cmd
            .spawn((
                PlanTurret(station),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(at.x),
                    top: px(at.y),
                    width: px(TURRET),
                    height: px(TURRET),
                    ..default()
                },
                UiTransform::default(),
                ChildOf(frame),
            ))
            .id();
        // Tube 0 is the left barrel looking out along the mount's bearing.
        for tube in 0..layout.count() {
            let barrel = cmd
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(TURRET / 2. + layout.barrel_x(tube) * 6.8 - BARREL.x / 2.),
                        top: px(0),
                        width: px(BARREL.x),
                        height: px(BARREL.y),
                        ..default()
                    },
                    BackgroundColor(Color::srgb_u8(22, 38, 46)),
                    ChildOf(turret),
                ))
                .id();
            // The round goes in at the breech, so the barrel fills from the gunhouse out.
            cmd.spawn((
                PlanBarrel(station, tube),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    bottom: px(0),
                    width: percent(100),
                    ..default()
                },
                BackgroundColor(ink(MountStatus::Ready)),
                ChildOf(barrel),
            ));
        }
        cmd.spawn((
            PlanHouse(station),
            Node {
                position_type: PositionType::Absolute,
                left: px(TURRET / 2. - HOUSE / 2.),
                top: px(TURRET / 2. - HOUSE / 2.),
                width: px(HOUSE),
                height: px(HOUSE),
                border: UiRect::all(px(1.5)),
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BackgroundColor(ink(MountStatus::Training)),
            BorderColor::all(Color::srgb_u8(16, 30, 36)),
            ChildOf(turret),
        ));
    }

    let readouts = cmd
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                height: px(FRAME.y + extra_pixels),
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    let label = |cmd: &mut Commands, parent, text: &str, size, color, extra: Option<Readout>| {
        let mut e = cmd.spawn((
            Text::new(text),
            TextFont::from_font_size(size),
            TextColor(color),
            TextLayout::no_wrap(),
            ChildOf(parent),
        ));
        if let Some(extra) = extra {
            e.insert(extra);
        }
        e.id()
    };
    let heading = cmd
        .spawn((
            Node {
                column_gap: px(10),
                align_items: AlignItems::Baseline,
                ..default()
            },
            ChildOf(readouts),
        ))
        .id();
    label(cmd, heading, "MAIN BATTERY", 13., brass(), None);
    label(
        cmd,
        heading,
        "",
        12.,
        Color::WHITE,
        Some(Readout::Selection),
    );
    let groups = cmd
        .spawn((
            Node {
                flex_direction: if positions.len() <= 2
                    && run.fittings.turrets == TurretLayout::Twin
                {
                    FlexDirection::Column
                } else {
                    FlexDirection::Row
                },
                justify_content: JustifyContent::SpaceEvenly,
                align_items: AlignItems::Center,
                flex_grow: 1.,
                column_gap: px(9),
                ..default()
            },
            ChildOf(readouts),
        ))
        .id();
    for (&station, position) in Station::SLOTS.iter().zip(&positions) {
        let fit = run.fittings.for_station(position);
        let layout = fit.turrets;
        let block = cmd
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(3),
                    ..default()
                },
                ChildOf(groups),
            ))
            .id();
        if positions.len() > 2 {
            label(
                cmd,
                block,
                &format!(
                    "{}  {:.0}\"",
                    station.label(&positions),
                    fit.round().calibre_inches()
                ),
                11.,
                brass(),
                None,
            );
        }
        for tube in 0..layout.count() {
            let row = cmd
                .spawn((
                    Node {
                        column_gap: px(6),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    ChildOf(block),
                ))
                .id();
            label(
                cmd,
                row,
                "",
                12.,
                Color::WHITE,
                Some(Readout::Tag(station, tube, station.label(&positions))),
            );
            let gauge = cmd
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        min_width: px(60),
                        ..default()
                    },
                    ChildOf(row),
                ))
                .id();
            label(
                cmd,
                gauge,
                "",
                12.,
                Color::WHITE,
                Some(Readout::State(station, tube)),
            );
            let track = cmd
                .spawn((
                    Node {
                        width: percent(100),
                        height: px(3),
                        ..default()
                    },
                    BackgroundColor(trim()),
                    ChildOf(gauge),
                ))
                .id();
            cmd.spawn((
                Readout::Bar(station, tube),
                Node {
                    height: percent(100),
                    ..default()
                },
                BackgroundColor(ink(MountStatus::Ready)),
                ChildOf(track),
            ));
        }
    }
    label(cmd, readouts, "", 12., Color::WHITE, Some(Readout::Summary));
}

/// The colour and words for one barrel: its countdown while loading, else its mount's state.
/// With AP fitted, the words start with the round in the barrel or going into it.
fn barrel_state(mount: &MainMount, tube: usize, with_ammo: bool) -> (Color, String) {
    let round = if with_ammo {
        format!("{} ", mount.ammo(tube).label())
    } else {
        String::new()
    };
    if mount.status() != MountStatus::Disabled && !mount.loaded(tube) {
        // Round up so a barrel never reads 0.0 s while it is still loading.
        let tenths = (mount.reload_left(tube) * 10.).ceil() / 10.;
        return (ink(MountStatus::Loading), format!("{round}{tenths:.1} s"));
    }
    let status = mount.status();
    let words = match status {
        MountStatus::Ready | MountStatus::Loading => "READY",
        MountStatus::Training => "TRAINING",
        MountStatus::Masked => "MASKED",
        MountStatus::Disabled => "OUT",
    };
    (ink(status), format!("{round}{words}"))
}
fn ammo_ink(ammo: Ammo) -> Color {
    match ammo {
        Ammo::He => Color::srgb_u8(255, 154, 84),
        Ammo::Ap => Color::srgb_u8(214, 224, 232),
    }
}

type SightParts<'a> = (&'a SightMark, &'a mut Node, &'a mut UiTransform);
type SightFilter = (Without<PlanBarrel>, Without<Readout>, Without<PlanTurret>);
type ReadoutFilter = (Without<PlanBarrel>, Without<SightMark>, Without<PlanHouse>);
type ReadoutParts<'a> = (
    &'a Readout,
    Option<&'a mut Node>,
    Option<&'a mut BackgroundColor>,
    Option<&'a mut Text>,
    Option<&'a mut TextColor>,
);

/// Everything the board reads but does not draw into.
type BoardState<'w> = (
    Res<'w, Encounter>,
    Res<'w, Campaign>,
    Res<'w, PlayerHelm>,
    Res<'w, ShipView>,
    Res<'w, Guns>,
);

pub fn update(
    state: BoardState,
    mut turrets: Query<(&PlanTurret, &mut UiTransform), Without<SightMark>>,
    mut houses: Query<(&PlanHouse, &mut BackgroundColor), Without<PlanBarrel>>,
    mut barrels: Query<(&PlanBarrel, &mut Node, &mut BackgroundColor), Without<Readout>>,
    mut sight: Query<SightParts, SightFilter>,
    mut readouts: Query<ReadoutParts, ReadoutFilter>,
) {
    let (battle, run, helm, view, guns) = state;
    let with_ammo = run.fittings.armor_piercing();
    // The icons grow with the gunhouses, by half as much.
    for (turret, mut pose) in &mut turrets {
        let mount = battle.main_mount(turret.0);
        let bore = mount.round(run.fittings.round()).mount_scale() / MainShell::STOCK.mount_scale();
        pose.rotation = Rot2::radians(mount.deck_bearing());
        pose.scale =
            Vec2::splat((0.5 + 0.5 * bore) * if mount.position.paired() { 0.65 } else { 1. });
    }
    for (house, mut fill) in &mut houses {
        let color = ink(battle.main_mount(house.0).status());
        if fill.0 != color {
            fill.0 = color;
        }
    }
    for (barrel, mut node, mut fill) in &mut barrels {
        let mount = battle.main_mount(barrel.0);
        node.height = percent(100. * mount.load_fraction(barrel.1));
        fill.0 = barrel_state(mount, barrel.1, false).0;
    }
    let relative = view.bearing - helm.heading;
    let direction = Vec2::new(relative.sin(), -relative.cos());
    let mut rim = RIM;
    if direction.y > 0. {
        rim.y += run.added_length() * SCALE.y;
    }
    let reach = direction / (direction / rim).length();
    for (mark, mut node, mut pose) in &mut sight {
        match mark {
            SightMark::Line => {
                let length = reach.length();
                node.left = px(ORIGIN.x - 0.75);
                node.top = px(ORIGIN.y - length);
                node.height = px(2. * length);
                pose.rotation = Rot2::radians(relative);
            }
            SightMark::Tip => {
                node.left = px(ORIGIN.x + reach.x - 3.);
                node.top = px(ORIGIN.y + reach.y - 3.);
            }
        }
    }
    let next = battle.next_barrel();
    let mut ready = 0;
    for mount in battle.main_mounts() {
        ready += (0..mount.layout().count())
            .filter(|&t| mount.loaded(t) && mount.status() == MountStatus::Ready)
            .count();
    }
    let total: usize = battle
        .main_mounts()
        .iter()
        .map(|m| m.layout().count())
        .sum();
    for (readout, node, fill, text, tint) in &mut readouts {
        let (words, color) = match *readout {
            Readout::Tag(station, tube, letter) => {
                let up = next == Some((station, tube));
                let words = format!("{}{}{}", if up { ">" } else { " " }, letter, tube + 1);
                (words, if up { brass() } else { Color::WHITE })
            }
            Readout::Bar(station, tube) => {
                let mount = battle.main_mount(station);
                if let Some(mut node) = node {
                    node.width = percent(100. * mount.load_fraction(tube));
                }
                if let Some(mut fill) = fill {
                    fill.0 = barrel_state(mount, tube, false).0;
                }
                continue;
            }
            Readout::State(station, tube) => {
                let (color, words) = barrel_state(battle.main_mount(station), tube, with_ammo);
                (words, color)
            }
            Readout::Selection if with_ammo => {
                (format!("LOAD {}", guns.ammo.label()), ammo_ink(guns.ammo))
            }
            Readout::Selection => (String::new(), Color::WHITE),
            Readout::Summary => match battle.standing_volley() {
                Some(Volley::Salvo) => ("SALVO".into(), brass()),
                Some(Volley::Single) => ("STANDING BY".into(), brass()),
                None => (
                    format!("{ready} / {total} ON TARGET"),
                    if ready > 0 {
                        ink(MountStatus::Ready)
                    } else {
                        Color::srgb_u8(164, 194, 192)
                    },
                ),
            },
        };
        if let Some(mut text) = text
            && text.0 != words
        {
            text.0 = words;
        }
        if let Some(mut tint) = tint
            && tint.0 != color
        {
            tint.0 = color;
        }
    }
}
