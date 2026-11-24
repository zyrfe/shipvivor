//! Sector clock, base bearings and world-space objective flags.
use crate::{
    camera::ShipView,
    state::{Campaign, Encounter, PlayerHelm, Screen},
};
use bevy::prelude::*;
use shipvivor_campaign::{MAX_STARS, Stage};
use shipvivor_encounter::{EffectKind, EnemyClass, Port, YardState};
use std::f32::consts::{FRAC_PI_4, PI, TAU};

const BOARD_WIDTH: f32 = 332.;
const FLAG: Vec2 = Vec2::new(119., 21.);
const LETTERS: [&str; 3] = ["A", "B", "C"];

pub fn letter(index: usize) -> &'static str {
    LETTERS[index]
}
fn chalk() -> Color {
    Color::srgb_u8(239, 240, 220)
}
fn sea_glass() -> Color {
    Color::srgb_u8(145, 219, 195)
}
fn brass() -> Color {
    Color::srgb_u8(255, 201, 101)
}
fn coral() -> Color {
    Color::srgb_u8(255, 148, 123)
}
fn plate() -> Color {
    Color::srgba_u8(12, 31, 40, 239)
}
fn dim() -> Color {
    Color::srgb_u8(151, 179, 179)
}

pub fn ship_class(class: EnemyClass) -> &'static str {
    match class {
        EnemyClass::Patrol => "Patrol boat",
        EnemyClass::Destroyer => "Destroyer",
        EnemyClass::Cruiser => "Cruiser",
        EnemyClass::Battleship => "Battleship",
        EnemyClass::Freighter => "Supply freighter",
        EnemyClass::Parliament => "Brass Parliament",
        EnemyClass::Regent => "Iron Regent",
    }
}

#[derive(Component, Clone, Copy)]
pub enum Reading {
    Clock,
    Bases,
    Stars,
    Turrets,
    Plating,
    Bearing(usize),
    Notice,
}
#[derive(Component)]
pub struct WorldFlag(usize);
#[derive(Component)]
pub struct SpotterAlert(usize);
#[derive(Component)]
pub struct TimeBar;

fn text(cmd: &mut Commands, parent: Entity, value: &str, size: f32, color: Color) -> Entity {
    cmd.spawn((
        Text::new(value),
        TextFont::from_font_size(size),
        TextColor(color),
        TextLayout::justify(Justify::Center),
        ChildOf(parent),
    ))
    .id()
}

pub fn spawn(cmd: &mut Commands, root: Entity, stage: Stage) {
    let header = cmd
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(12),
                left: px(0),
                right: px(0),
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    let board = cmd
        .spawn((
            Node {
                width: px(BOARD_WIDTH),
                flex_direction: FlexDirection::Column,
                border: UiRect::top(px(2)),
                ..default()
            },
            BackgroundColor(plate()),
            BorderColor::all(brass()),
            ChildOf(header),
        ))
        .id();
    let stats = cmd
        .spawn((
            Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceEvenly,
                padding: UiRect::vertical(px(4)),
                ..default()
            },
            ChildOf(board),
        ))
        .id();
    let columns = if stage == Stage::CrownDeep {
        [
            ("GOLD TURRETS", Reading::Turrets, 22., 95.),
            ("TIME LEFT", Reading::Clock, 34., 112.),
            ("BOSS HULL", Reading::Plating, 20., 95.),
        ]
    } else {
        [
            ("BASES CLEARED", Reading::Bases, 22., 95.),
            ("TIME LEFT", Reading::Clock, 34., 112.),
            ("STARS", Reading::Stars, 22., 95.),
        ]
    };
    for (title, reading, font, width) in columns {
        let column = cmd
            .spawn((
                Node {
                    width: px(width),
                    align_items: AlignItems::Center,
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                ChildOf(stats),
            ))
            .id();
        text(cmd, column, title, 11., dim());
        let value = text(cmd, column, "", font, chalk());
        cmd.entity(value).insert(reading);
    }
    let track = cmd
        .spawn((
            Node {
                width: percent(100),
                height: px(2),
                ..default()
            },
            BackgroundColor(Color::srgb_u8(52, 76, 80)),
            ChildOf(board),
        ))
        .id();
    cmd.spawn((
        TimeBar,
        Node {
            height: percent(100),
            ..default()
        },
        BackgroundColor(brass()),
        ChildOf(track),
    ));
    let bearings = cmd
        .spawn((
            Node {
                justify_content: JustifyContent::SpaceEvenly,
                padding: UiRect::vertical(px(4)),
                ..default()
            },
            ChildOf(board),
        ))
        .id();
    for i in 0..LETTERS.len() {
        let alert = text(cmd, root, "!", 25., chalk());
        cmd.entity(alert).insert((
            SpotterAlert(i),
            Node {
                position_type: PositionType::Absolute,
                width: px(32.),
                height: px(32.),
                display: Display::None,
                border: UiRect::all(px(2.)),
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BackgroundColor(Color::srgb_u8(147, 48, 36)),
            BorderColor::all(brass()),
            GlobalZIndex(4),
        ));
        let bearing = text(cmd, bearings, "", 12., coral());
        cmd.entity(bearing).insert((
            Reading::Bearing(i),
            Node {
                width: px(105),
                ..default()
            },
        ));
        if stage == Stage::CrownDeep {
            continue;
        }
        let flag = text(cmd, root, "", 12., chalk());
        cmd.entity(flag).insert((
            WorldFlag(i),
            Node {
                position_type: PositionType::Absolute,
                width: px(FLAG.x),
                height: px(FLAG.y),
                border: UiRect::bottom(px(2)),
                padding: UiRect::top(px(1)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(coral()),
            BackgroundColor(plate()),
        ));
    }
    let notice = text(cmd, header, "", 15., sea_glass());
    cmd.entity(notice).insert((
        Reading::Notice,
        Node {
            margin: UiRect::top(px(5)),
            padding: UiRect::axes(px(12), px(4)),
            display: Display::None,
            ..default()
        },
        BackgroundColor(plate()),
    ));
}

pub fn clearance(cmd: &mut Commands, root: Entity, battle: &Encounter, run: &Campaign) {
    let panel = cmd
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(24),
                padding: UiRect::axes(px(27), px(10)),
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                border: UiRect::top(px(2)),
                ..default()
            },
            BackgroundColor(plate()),
            BorderColor::all(sea_glass()),
            ChildOf(root),
        ))
        .id();
    if run.stage == Stage::CrownDeep {
        text(cmd, panel, "IRON REGENT SUNK", 26., brass());
        text(cmd, panel, "Patrol report   [Enter]", 14., chalk());
        return;
    }
    text(cmd, panel, "SECTOR SECURED", 26., sea_glass());
    text(
        cmd,
        panel,
        &format!(
            "{} / {} drydocks destroyed",
            battle.ruined_yards(),
            battle.ports().len()
        ),
        16.,
        chalk(),
    );
    let destination = if run.stage == shipvivor_campaign::Stage::IronNarrows {
        "Drydock"
    } else {
        "Flagship intercept"
    };
    text(cmd, panel, &format!("{destination}   [Enter]"), 12., dim());
}

fn cardinal(bearing: f32) -> &'static str {
    ["N", "NE", "E", "SE", "S", "SW", "W", "NW"]
        [(bearing.rem_euclid(TAU) / FRAC_PI_4).round() as usize % 8]
}

fn bearing_text(port: &Port, i: usize, player: Vec2, view_bearing: f32) -> String {
    let id = letter(i);
    let total = port.factories().len();
    let works = format!(
        "{}/{} {}",
        port.live_factories(),
        total,
        if total == 1 { "factory" } else { "factories" }
    );
    if port.yard_state() == YardState::Ruined {
        return format!("{id}  CLEARED\n{works}");
    }
    let d = port.yard_position() - player;
    let heading = shipvivor_compass::heading(d);
    let turn = (heading - view_bearing + PI).rem_euclid(TAU) - PI;
    let steer = if turn.abs() < 9_f32.to_radians() {
        ""
    } else if turn < 0. {
        "<"
    } else {
        ">"
    };
    let range = (d.length() / 10.).round() as u32 * 10;
    format!("{id} {steer}{} {range}m\n{works}", cardinal(heading))
}

// Project the top of the crane and leave the roof open for aiming. Stack close flags
// upwards so two harbours on the same bearing have independent labels.
fn flag_positions(
    battle: &Encounter,
    helm: &PlayerHelm,
    view: &ShipView,
    size: Vec2,
) -> [Option<Vec2>; 3] {
    let mut flags = [None; 3];
    for (i, port) in battle.ports().iter().enumerate() {
        let top = port.yard_position().extend(22.).xzy();
        let Some(p) = view.viewport_point(helm.position, top, size) else {
            continue;
        };
        if p.x < 0. || p.x > size.x || p.y > size.y || size.x < FLAG.x {
            continue;
        }
        let mut pos = Vec2::new(
            (p.x - 0.5 * FLAG.x).clamp(0., size.x - FLAG.x),
            p.y - FLAG.y - 5.,
        );
        // Keep the tag away from the gun sight even while the roof falls below the smoke.
        if (p.x - size.x * 0.5).abs() < FLAG.x * 0.5 + 22.
            && pos.y < size.y * 0.5 + 28.
            && pos.y + FLAG.y > size.y * 0.5 - 35.
        {
            pos.y = size.y * 0.5 - 35. - FLAG.y;
        }
        while let Some(other) = flags[..i].iter().flatten().find(|other: &&Vec2| {
            (other.x - pos.x).abs() < FLAG.x + 2. && (other.y - pos.y).abs() < FLAG.y + 2.
        }) {
            pos.y = other.y - FLAG.y - 3.;
        }
        if pos.y >= 0. {
            flags[i] = Some(pos);
        }
    }
    flags
}

fn status_color(port: &Port) -> Color {
    match port.yard_state() {
        YardState::Ruined => sea_glass(),
        YardState::Burning | YardState::Damaged => brass(),
        YardState::Working => coral(),
    }
}

pub fn update(
    run: Res<Campaign>,
    battle: Res<Encounter>,
    helm: Res<PlayerHelm>,
    view: Res<ShipView>,
    viewport: (Single<&Window>, Res<UiScale>),
    mut readings: Query<(&Reading, &mut Text, &mut TextColor, &mut Node), Without<WorldFlag>>,
    mut flags: Query<(&WorldFlag, &mut Text, &mut Node, &mut BorderColor), Without<Reading>>,
    mut bar: Query<
        (&mut Node, &mut BackgroundColor),
        (With<TimeBar>, Without<Reading>, Without<WorldFlag>),
    >,
) {
    if run.screen() != Screen::Sailing {
        return;
    }
    let remaining = run.remaining().ceil().max(0.) as u32;
    let clock_color = match remaining {
        0..=60 => coral(),
        61..=120 => brass(),
        _ => chalk(),
    };
    for (mut node, mut color) in &mut bar {
        node.width = percent(
            100. * run.remaining().max(0.) / (f32::from(run.stage.mission().minutes) * 60.),
        );
        color.set_if_neq(BackgroundColor(clock_color));
    }
    for (reading, mut message, mut ink, mut node) in &mut readings {
        let (words, color) = match *reading {
            Reading::Clock => (
                format!("{:02}:{:02}", remaining / 60, remaining % 60),
                clock_color,
            ),
            Reading::Turrets | Reading::Plating => {
                let Some(boss) = battle
                    .raiders()
                    .iter()
                    .find(|r| r.class == EnemyClass::Regent)
                else {
                    continue;
                };
                if matches!(reading, Reading::Turrets) {
                    (
                        format!("{} / {}", boss.live_guns(), boss.guns().len()),
                        brass(),
                    )
                } else if boss.protected() {
                    ("SEALED".into(), dim())
                } else {
                    ("EXPOSED".into(), coral())
                }
            }
            Reading::Bases => (
                format!("{} / {}", battle.ruined_yards(), battle.ports().len()),
                sea_glass(),
            ),
            Reading::Stars => (format!("{} / {MAX_STARS}", run.stars()), brass()),
            Reading::Bearing(i) => {
                let port = &battle.ports()[i];
                (
                    bearing_text(port, i, helm.position, view.bearing),
                    status_color(port),
                )
            }
            Reading::Notice => {
                // The flagship's sortie has its own herald, in `flagship_hud`.
                if let Some(fx) = battle
                    .effects()
                    .iter()
                    .rev()
                    .find(|fx| matches!(fx.kind, EffectKind::Demolition { .. }))
                {
                    let base = battle.ports().iter().enumerate().min_by(|(_, a), (_, b)| {
                        a.yard_position()
                            .distance_squared(fx.body.pos)
                            .total_cmp(&b.yard_position().distance_squared(fx.body.pos))
                    });
                    let Some((i, _)) = base else {
                        continue;
                    };
                    node.display = Display::Flex;
                    if matches!(fx.kind, EffectKind::Demolition { dock: true }) {
                        (format!("BASE {} DESTROYED", letter(i)), sea_glass())
                    } else {
                        (
                            format!("BASE {}    FACTORY KNOCKED OUT", letter(i)),
                            brass(),
                        )
                    }
                } else if let Some(plane) = battle
                    .spotters()
                    .iter()
                    .filter(|p| p.alert_left > 0.)
                    .max_by(|a, b| a.alert_left.total_cmp(&b.alert_left))
                {
                    node.display = Display::Flex;
                    let bearing = shipvivor_compass::heading(plane.position - helm.position);
                    (
                        format!("SPOTTER {}  /  POSITION REPORTED", cardinal(bearing)),
                        brass(),
                    )
                } else {
                    node.display = Display::None;
                    continue;
                }
            }
        };
        if message.0 != words {
            message.0 = words;
        }
        ink.set_if_neq(TextColor(color));
    }
    let (window, scale) = viewport;
    let positions = flag_positions(&battle, &helm, &view, window.size() / scale.0);
    for (flag, mut message, mut node, mut border) in &mut flags {
        let Some(pos) = positions[flag.0] else {
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        node.left = px(pos.x);
        node.top = px(pos.y);
        let port = &battle.ports()[flag.0];
        let range = port.yard_position().distance(helm.position).round() as u32;
        let id = letter(flag.0);
        let words = if port.yard_state() == YardState::Ruined {
            format!("BASE {id}  CLEARED")
        } else {
            format!("DOCK {id}   {range} m")
        };
        if message.0 != words {
            message.0 = words;
        }
        border.set_if_neq(BorderColor::all(status_color(port)));
    }
}

/// A marker stays with the aircraft under zoom, or moves to the edge on a blind bearing.
pub fn spotter_alerts(
    battle: Res<Encounter>,
    helm: Res<PlayerHelm>,
    view: Res<ShipView>,
    window: Single<&Window>,
    scale: Res<UiScale>,
    mut alerts: Query<(&SpotterAlert, &mut Node, &mut UiTransform)>,
) {
    let size = window.size() / scale.0;
    for (marker, mut node, mut transform) in &mut alerts {
        let plane = &battle.spotters()[marker.0];
        if plane.alert_left <= 0. {
            node.display = Display::None;
            continue;
        }
        let (mut pos, edge) =
            view.project_mark(helm.position, plane.position_3d() + Vec3::Y * 3., size);
        let age = shipvivor_encounter::ALERT_SECONDS - plane.alert_left;
        if edge.is_empty() {
            pos.y -= 20.;
        }
        let margin = (size * 0.5).min(Vec2::splat(24.));
        pos = pos.clamp(margin, size - margin);
        node.left = px(pos.x - 16.);
        node.top = px(pos.y - 16.);
        node.display = Display::Flex;
        transform.scale = Vec2::splat(1. + 0.5 * (1. - age / 0.35).clamp(0., 1.));
    }
}
