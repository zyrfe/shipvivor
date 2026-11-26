//! A ring follows each deck mount, sized to its bore, with one load pip per fitted
//! barrel. A mount laid on a mark beyond her reach goes grey, and a note under the rings
//! says so.
//! Beside the sight a rangefinder reads the range to whatever lies under it, and the time
//! a main round takes to get there.
use crate::{
    camera::ShipView,
    state::{Campaign, Encounter, PlayerHelm},
};
use bevy::prelude::*;
use shipvivor_encounter::{Aim, Battle, MainMount, MainShell, MountStatus, Station};

#[derive(Component)]
pub struct GunCircle(Station);
/// The turret's number from the bow.
#[derive(Component)]
pub struct GunLabel(usize);
#[derive(Component)]
pub struct GunPip(usize);
/// Says "Out of Range" under the rings of the mounts that cannot reach the mark.
#[derive(Component)]
pub struct RangeNote;
/// The rangefinder's reading beside the sight in the middle of the screen.
#[derive(Component)]
pub struct RangeReadout;

/// The reading sits this far right of the widest ring, clear of its load pips, and this
/// far above the middle of the screen, so its line of text centres on the sightline.
const READOUT_GAP: f32 = 14.;
const READOUT_RISE: f32 = 8.;

/// The box the note is centred in, and how far it hangs under the lowest grey ring,
/// clear of the ring labels.
const NOTE_WIDTH: f32 = 140.;
const NOTE_DROP: f32 = 18.;

pub(crate) fn out_of_range_ink() -> Color {
    Color::srgb_u8(152, 158, 160)
}

/// Whether the flight the mount would take onto `aim` falls short of it at her reach.
pub(crate) fn out_of_reach(
    mount: &MainMount,
    helm: &PlayerHelm,
    aim: Vec3,
    round: MainShell,
) -> bool {
    mount
        .trajectory(helm.position, helm.heading, aim, round)
        .is_some_and(|path| path.point(0.).xz().distance(aim.xz()) > mount.round(round).reach())
}

/// Pixels of ring radius to an inch of bore, so a ring is as big as its gun.
const RING_PER_INCH: f32 = 3.;
/// The least step between one ring and the next one out. Turrets of one bore still nest
/// apart, so each keeps its own load pips, and numbers on the same side of the sight stay
/// clear of each other.
const RING_GAP: f32 = 7.;
/// Height of a ring's number tag.
const LABEL_HEIGHT: f32 = 14.;

/// Each turret's ring radius, by its slot: in proportion to its bore, nested smallest
/// out, and stepped out by [`RING_GAP`] from a ring of the same bore, the lower turret
/// number innermost.
fn radii(battle: &Battle, round: MainShell) -> Vec<f32> {
    let mounts = battle.main_mounts();
    let positions: Vec<_> = mounts.iter().map(|m| m.position).collect();
    let bore = |k: usize| mounts[k].round(round).calibre_inches();
    let number = |k: usize| Station::SLOTS[k].number(&positions);
    let mut order: Vec<usize> = (0..mounts.len()).collect();
    order.sort_by(|&a, &b| bore(a).total_cmp(&bore(b)).then(number(a).cmp(&number(b))));
    let mut radii = vec![0.; mounts.len()];
    let mut inner = f32::NEG_INFINITY;
    for k in order {
        inner = (RING_PER_INCH * bore(k)).max(inner + RING_GAP);
        radii[k] = inner;
    }
    radii
}
/// How far down its ring a barrel's load pip sits: the pips stack down the ring's right
/// side, centred on its middle.
fn pip_top(radius: f32, tube: usize, barrels: usize) -> f32 {
    radius - 2.5 + 7. * (tube as f32 - barrels.saturating_sub(1) as f32 * 0.5)
}
/// Where a ring's number sits: above the ring for the innermost, the third and so on,
/// below it for the others, so tags on one side are two gaps apart.
fn label_top(radii: &[f32], radius: f32) -> f32 {
    let inside = radii.iter().filter(|&&r| r < radius).count();
    if inside.is_multiple_of(2) {
        -LABEL_HEIGHT - 4.
    } else {
        2. * radius + 1.
    }
}
pub(crate) fn ink(status: MountStatus) -> Color {
    match status {
        MountStatus::Ready => Color::srgb_u8(105, 251, 180),
        MountStatus::Training => Color::srgb_u8(255, 195, 71),
        MountStatus::Loading => Color::srgb_u8(128, 209, 255),
        MountStatus::Disabled => Color::srgb_u8(234, 125, 98),
        MountStatus::Masked => Color::srgb_u8(183, 181, 174),
    }
}
pub fn spawn(cmd: &mut Commands, root: Entity, run: &Campaign) {
    let positions = run.ship.design().gun_stations();
    for (&station, position) in Station::SLOTS.iter().zip(&positions) {
        let fitted = run.fittings.for_station(position);
        let layout = fitted.turrets;
        // `update` nests the rings by bore every frame; this is only the first guess.
        let radius = RING_PER_INCH * f32::from(fitted.calibre());
        let ring = cmd
            .spawn((
                GunCircle(station),
                Node {
                    position_type: PositionType::Absolute,
                    width: px(2. * radius),
                    height: px(2. * radius),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BorderColor::all(ink(MountStatus::Training)),
                ChildOf(root),
            ))
            .id();
        let number = station.number(&positions);
        cmd.spawn((
            GunLabel(number),
            Text::new(number.to_string()),
            TextFont::from_font_size(11.),
            TextColor(Color::WHITE),
            BackgroundColor(Color::srgba_u8(14, 34, 40, 218)),
            Node {
                position_type: PositionType::Absolute,
                left: px(radius - 7.),
                top: px(2. * radius + 1.),
                height: px(LABEL_HEIGHT),
                padding: UiRect::horizontal(px(3)),
                ..default()
            },
            ChildOf(ring),
        ));
        for tube in 0..layout.count() {
            cmd.spawn((
                GunPip(tube),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(2. * radius + 3.),
                    top: px(pip_top(radius, tube, layout.count())),
                    width: px(5),
                    height: px(5),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                BorderColor::all(ink(MountStatus::Loading)),
                ChildOf(ring),
            ));
        }
    }
    cmd.spawn((
        RangeNote,
        Node {
            position_type: PositionType::Absolute,
            width: px(NOTE_WIDTH),
            justify_content: JustifyContent::Center,
            display: Display::None,
            ..default()
        },
        ChildOf(root),
    ))
    .with_child((
        Text::new("Out of Range"),
        TextFont::from_font_size(12.),
        TextColor(out_of_range_ink()),
        BackgroundColor(Color::srgba_u8(14, 34, 40, 218)),
        Node {
            padding: UiRect::horizontal(px(4)),
            ..default()
        },
    ));
    cmd.spawn((
        RangeReadout,
        Text::new(""),
        TextFont::from_font_size(12.),
        TextColor(Color::WHITE),
        BackgroundColor(Color::srgba_u8(14, 34, 40, 218)),
        Node {
            position_type: PositionType::Absolute,
            padding: UiRect::horizontal(px(4)),
            display: Display::None,
            ..default()
        },
        ChildOf(root),
    ));
}

type NoteOnly = (With<RangeNote>, Without<GunCircle>, Without<GunPip>);
type ReadoutOnly = (
    With<RangeReadout>,
    Without<RangeNote>,
    Without<GunCircle>,
    Without<GunPip>,
    Without<GunLabel>,
);

pub fn update(
    sight: (Res<ShipView>, Res<PlayerHelm>, Res<Encounter>),
    window: Single<&Window>,
    controls: (Res<UiScale>, Res<crate::state::Guns>, Res<Campaign>),
    mut circles: Query<(&GunCircle, &Children, &mut Node, &mut BorderColor)>,
    mut labels: Query<
        (&GunLabel, &mut Node, &mut Text, &mut TextColor),
        (Without<GunCircle>, Without<GunPip>, Without<RangeNote>),
    >,
    mut pips: Query<
        (&GunPip, &mut Node, &mut BackgroundColor, &mut BorderColor),
        Without<GunCircle>,
    >,
    mut notes: Query<&mut Node, NoteOnly>,
    mut readouts: Query<(&mut Node, &mut Text, &mut TextColor), ReadoutOnly>,
) {
    let (view, helm, encounter) = sight;
    let (scale, guns, run) = controls;
    let size = window.size() / scale.0;
    if size.min_element() < 1. {
        return;
    }
    let aim = match view.main_aim(helm.position, &encounter, guns.firing, guns.nearest) {
        Aim::Sight(p) | Aim::Track(p) => Some(p),
        _ => None,
    };
    let round = run.fittings.round();
    let radii = radii(&encounter, round);
    let radius = |station: Station| radii.get(station.index()).copied().unwrap_or(12.);
    // The rings that went grey, by centre and radius.
    let mut short = Vec::new();
    for (circle, children, mut node, mut border) in &mut circles {
        node.display = if guns.torpedo_mode {
            Display::None
        } else {
            Display::Flex
        };
        let station = circle.0;
        let mount = encounter.main_mount(station);
        let point = mount.aim_mark(helm.position, helm.heading);
        let (center, edge) = view.project_mark(helm.position, point, size);
        let radius = radius(station);
        node.left = px(center.x - radius);
        node.top = px(center.y - radius);
        node.width = px(2. * radius);
        node.height = px(2. * radius);
        let laying = matches!(
            mount.status(),
            MountStatus::Ready | MountStatus::Training | MountStatus::Loading
        );
        // A crew picks its own targets inside the round's reach.
        let beyond = laying
            && !mount.engaged()
            && aim.is_some_and(|aim| out_of_reach(mount, &helm, aim, round));
        if beyond && edge.is_empty() && !guns.torpedo_mode {
            short.push((center, radius));
        }
        let color = if beyond {
            out_of_range_ink()
        } else {
            ink(mount.status())
        };
        let empty = if beyond {
            out_of_range_ink()
        } else {
            ink(MountStatus::Loading)
        };
        *border = BorderColor::all(if edge.is_empty() { color } else { Color::NONE });
        let barrels = children.iter().filter(|&c| pips.contains(c)).count();
        for child in children.iter() {
            if let Ok((pip, mut node, mut fill, mut rim)) = pips.get_mut(child) {
                node.display = if edge.is_empty() {
                    Display::Flex
                } else {
                    Display::None
                };
                node.left = px(2. * radius + 3.);
                node.top = px(pip_top(radius, pip.0, barrels));
                let (inside, outline) = if mount.loaded(pip.0) {
                    (color, color)
                } else {
                    (Color::NONE, empty)
                };
                fill.0 = inside;
                *rim = BorderColor::all(outline);
            }
            if let Ok((label, mut node, mut text, mut tint)) = labels.get_mut(child) {
                node.left = px(radius - 7.);
                node.top = px(label_top(&radii, radius));
                let caption = format!("{}{edge}", label.0);
                if text.0 != caption {
                    text.0 = caption;
                }
                if tint.0 != color {
                    tint.0 = color;
                }
            }
        }
    }
    for mut note in &mut notes {
        if short.is_empty() {
            note.display = Display::None;
            continue;
        }
        let x = short.iter().map(|(c, _)| c.x).sum::<f32>() / short.len() as f32;
        let low = (short.iter().map(|(c, r)| c.y + r)).fold(f32::MIN, f32::max);
        note.left = px((x - NOTE_WIDTH * 0.5).clamp(0., (size.x - NOTE_WIDTH).max(0.)));
        note.top = px((low + NOTE_DROP).min(size.y - 20.));
        note.display = Display::Flex;
    }
    let reading = view
        .ranged(helm.position, &encounter)
        .filter(|_| !guns.torpedo_mode);
    for (mut node, mut text, mut tint) in &mut readouts {
        let Some(mark) = reading else {
            node.display = Display::None;
            continue;
        };
        // The flight of the slowest round in reach, which is when the salvo is all down.
        let mut widest = 0_f32;
        let mut flight = None::<f32>;
        for (circle, ..) in &circles {
            widest = widest.max(radius(circle.0));
            let mount = encounter.main_mount(circle.0);
            let laying = matches!(
                mount.status(),
                MountStatus::Ready | MountStatus::Training | MountStatus::Loading
            );
            if laying
                && !out_of_reach(mount, &helm, mark, round)
                && let Some(path) = mount.trajectory(helm.position, helm.heading, mark, round)
            {
                flight = Some(flight.map_or(path.duration, |t| t.max(path.duration)));
            }
        }
        let metres = helm.position.distance(mark.xz());
        // Past every gun's reach it reads the range alone, grey like the rings.
        let (caption, color) = match flight {
            Some(seconds) => (format!("{metres:.0} m  {seconds:.1} s"), Color::WHITE),
            None => (format!("{metres:.0} m"), out_of_range_ink()),
        };
        if text.0 != caption {
            text.0 = caption;
        }
        if tint.0 != color {
            tint.0 = color;
        }
        let middle = size * 0.5;
        node.left = px(middle.x + widest + READOUT_GAP);
        node.top = px(middle.y - READOUT_RISE);
        node.display = Display::Flex;
    }
}
