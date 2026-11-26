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
/// The most bases a sector holds, lettered in order up the channel.
pub const MAX_BASES: usize = 8;
const LETTERS: [&str; MAX_BASES] = ["A", "B", "C", "D", "E", "F", "G", "H"];
/// Bearings on the board: one for each base the chart shows that still stands.
const BEARINGS: usize = 3;
/// Seconds a newly roused stage is announced.
const STIRRING: f32 = 7.;
/// Seconds the clock's flourish lasts when a base puts time on it.
const PULSE: f32 = 0.9;

/// The clock as last seen, to catch the moment a base puts time on it.
#[derive(Default)]
pub struct ClockPulse {
    /// The leg and the seconds on its clock at the last look.
    seen: Option<(Stage, f32)>,
    /// Seconds since time was last put on it, while the flourish lasts.
    since: Option<f32>,
}

pub fn letter(index: usize) -> &'static str {
    LETTERS[index.min(MAX_BASES - 1)]
}

/// The bases the chart shows: those roused to the fight. A base further up the channel
/// is not marked until the stage before it falls or it sights the ship.
pub fn charted(port: &Port) -> bool {
    port.roused()
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
        EnemyClass::TorpedoBoat => "Torpedo boat",
        EnemyClass::Destroyer => "Destroyer",
        EnemyClass::Cruiser => "Cruiser",
        EnemyClass::Battleship => "Battleship",
        EnemyClass::Freighter => "Supply freighter",
        EnemyClass::Coaster => "Coaster",
        EnemyClass::Commodore => "Steel Commodore",
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
        cmd.entity(value).insert((reading, UiTransform::default()));
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
    for i in 0..BEARINGS {
        let bearing = text(cmd, bearings, "", 12., coral());
        cmd.entity(bearing).insert((
            Reading::Bearing(i),
            Node {
                width: px(105),
                ..default()
            },
        ));
    }
    for i in 0..MAX_BASES {
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
            "{} / {} bases destroyed",
            battle.ruined_yards(),
            battle.ports().len()
        ),
        16.,
        chalk(),
    );
    // Every patrol, the last included, ends in a yard.
    text(cmd, panel, "Drydock   [Enter]", 12., dim());
}

/// The bases of a stage that has just been roused, to announce: "BASE D STIRRING", or
/// "BASES B + C STIRRING" for a pair.
fn stirring(battle: &Encounter) -> Option<String> {
    let now = battle.at_sea();
    let fresh: Vec<&str> = battle
        .ports()
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            p.stage() > 0 && !p.wrecked() && p.roused_at().is_some_and(|at| now - at < STIRRING)
        })
        .map(|(i, _)| letter(i))
        .collect();
    match fresh.as_slice() {
        [] => None,
        [one] => Some(format!("BASE {one} STIRRING")),
        many => Some(format!("BASES {} STIRRING", many.join(" + "))),
    }
}

fn cardinal(bearing: f32) -> &'static str {
    ["N", "NE", "E", "SE", "S", "SW", "W", "NW"]
        [(bearing.rem_euclid(TAU) / FRAC_PI_4).round() as usize % 8]
}

/// Where a base is marked and ranged from: between its drydocks.
pub fn anchor(port: &Port) -> Vec2 {
    let docks = port.docks().len().max(1);
    (0..docks).map(|i| port.dock_position(i)).sum::<Vec2>() / docks as f32
}

fn bearing_text(port: &Port, i: usize, player: Vec2, view_bearing: f32) -> String {
    let id = letter(i);
    let count = |live: usize, total: usize, one: &str, many: &str| {
        format!("{live}/{total} {}", if total == 1 { one } else { many })
    };
    let works = format!(
        "{}\n{}",
        count(port.live_docks(), port.docks().len(), "dock", "docks"),
        count(
            port.live_factories(),
            port.factories().len(),
            "factory",
            "factories"
        )
    );
    if port.wrecked() {
        return format!("{id}  CLEARED\n{works}");
    }
    let d = anchor(port) - player;
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
/// The next base to make for: the nearest the chart shows that still stands.
pub fn next_base(battle: &Encounter, player: Vec2) -> Option<usize> {
    (0..battle.ports().len())
        .filter(|&i| charted(&battle.ports()[i]) && !battle.ports()[i].wrecked())
        .min_by(|&a, &b| {
            let range = |i: usize| anchor(&battle.ports()[i]).distance_squared(player);
            range(a).total_cmp(&range(b))
        })
}

/// Where each charted base's flag goes on the screen, and for the next base, when it
/// lies off the screen or astern, the edge it is pinned to: " <", " >", " ^" or " v".
fn flag_positions(
    battle: &Encounter,
    helm: &PlayerHelm,
    view: &ShipView,
    size: Vec2,
) -> Vec<Option<(Vec2, &'static str)>> {
    let mut flags: Vec<Option<(Vec2, &'static str)>> = vec![None; battle.ports().len()];
    let next = next_base(battle, helm.position);
    for (i, port) in battle.ports().iter().enumerate() {
        if !charted(port) {
            continue;
        }
        // The tag rides over the base, above the badges on its slips.
        let top = anchor(port).extend(22.).xzy();
        let shown = view
            .viewport_point(helm.position, top, size)
            .filter(|p| p.x >= 0. && p.x <= size.x && p.y <= size.y && size.x >= FLAG.x);
        let Some(p) = shown else {
            // The next base is never lost from view: its flag waits on the edge of the
            // screen nearest the way to turn for it.
            if next == Some(i) && size.cmpge(FLAG * 2.).all() {
                let (mark, edge) = view.project_mark(helm.position, top, size);
                let at = (mark - FLAG * 0.5).clamp(Vec2::splat(4.), size - FLAG - 4.);
                flags[i] = Some((at, if edge.is_empty() { " v" } else { edge }));
            }
            continue;
        };
        let mut pos = Vec2::new(
            (p.x - 0.5 * FLAG.x).clamp(0., size.x - FLAG.x),
            p.y - FLAG.y - 5. - crate::slip_badges::lift(port),
        );
        // Keep the tag away from the gun sight even while the roof falls below the smoke.
        if (p.x - size.x * 0.5).abs() < FLAG.x * 0.5 + 22.
            && pos.y < size.y * 0.5 + 28.
            && pos.y + FLAG.y > size.y * 0.5 - 35.
        {
            pos.y = size.y * 0.5 - 35. - FLAG.y;
        }
        while let Some((other, _)) = flags[..i].iter().flatten().find(|(other, _)| {
            (other.x - pos.x).abs() < FLAG.x + 2. && (other.y - pos.y).abs() < FLAG.y + 2.
        }) {
            pos.y = other.y - FLAG.y - 3.;
        }
        if pos.y >= 0. {
            flags[i] = Some((pos, ""));
        }
    }
    flags
}

/// Sea glass once the base is wrecked, brass while a dock is hurt or down, and coral
/// while every dock is at work.
fn status_color(port: &Port) -> Color {
    if port.wrecked() {
        sea_glass()
    } else if port.docks().iter().any(|d| d.state() != YardState::Working) {
        brass()
    } else {
        coral()
    }
}

pub fn update(
    (time, mut pulse): (Res<Time>, Local<ClockPulse>),
    run: Res<Campaign>,
    battle: Res<Encounter>,
    helm: Res<PlayerHelm>,
    view: Res<ShipView>,
    viewport: (Single<&Window>, Res<UiScale>),
    mut readings: Query<(&Reading, &mut Text, &mut TextColor, &mut Node), Without<WorldFlag>>,
    mut faces: Query<(&Reading, &mut UiTransform), Without<WorldFlag>>,
    mut flags: Query<(&WorldFlag, &mut Text, &mut Node, &mut BorderColor), Without<Reading>>,
    mut bar: Query<
        (&mut Node, &mut BackgroundColor),
        (With<TimeBar>, Without<Reading>, Without<WorldFlag>),
    >,
) {
    if run.screen() != Screen::Sailing {
        return;
    }
    // The clock is the time to beat. Past it she sails on, and it counts up the time
    // over in coral, until a base wrecked puts time back on it.
    let remaining = run.remaining().ceil().max(0.) as u32;
    let steady = match remaining {
        _ if run.late() => coral(),
        0..=60 => coral(),
        61..=120 => brass(),
        _ => chalk(),
    };
    // Time put on the clock springs its figures up and flashes them sea glass, and the
    // bar leaps with them. A new leg's fresh clock is no flourish.
    let clock = run.clock();
    if (pulse.seen).is_some_and(|(stage, seen)| stage == run.stage && clock > seen + 0.5) {
        pulse.since = Some(0.);
    }
    pulse.seen = Some((run.stage, clock));
    pulse.since = (pulse.since)
        .map(|t| t + time.delta_secs().min(0.05))
        .filter(|&t| t < PULSE);
    let (swell, clock_color) = pulse.since.map_or((0., steady), |t| {
        let spring = 0.3 * (-5. * t).exp() * (12. * t).sin();
        (spring, steady.mix(&sea_glass(), (1. - t / 0.6).max(0.)))
    });
    for (reading, mut pose) in &mut faces {
        let scale = Vec2::splat(1. + swell);
        if matches!(reading, Reading::Clock) && pose.scale != scale {
            pose.scale = scale;
        }
    }
    for (mut node, mut color) in &mut bar {
        let full = f32::from(run.stage.mission().minutes) * 60.;
        node.width = percent((100. * run.remaining().max(0.) / full).min(100.));
        color.set_if_neq(BackgroundColor(clock_color));
    }
    for (reading, mut message, mut ink, mut node) in &mut readings {
        let (words, color) = match *reading {
            Reading::Clock => (crate::ui::clock_reading(&run), clock_color),
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
            Reading::Stars => (
                format!("{} / {MAX_STARS}", run.stars()),
                if run.late() { coral() } else { brass() },
            ),
            Reading::Bearing(slot) => {
                // The standing bases the chart shows, in order up the channel.
                let shown = battle
                    .ports()
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| charted(p) && !p.wrecked())
                    .nth(slot);
                let Some((i, port)) = shown else {
                    node.display = Display::None;
                    continue;
                };
                node.display = Display::Flex;
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
                        anchor(a)
                            .distance_squared(fx.body.pos)
                            .total_cmp(&anchor(b).distance_squared(fx.body.pos))
                    });
                    let Some((i, port)) = base else {
                        continue;
                    };
                    node.display = Display::Flex;
                    let id = letter(i);
                    if port.wrecked() && run.stage != Stage::CrownDeep {
                        // The time it puts on the clock, said where the eye already is.
                        let gained = shipvivor_campaign::BASE_TIME;
                        (
                            format!("BASE {id} DESTROYED    +{gained:.0} S"),
                            sea_glass(),
                        )
                    } else if port.wrecked() {
                        (format!("BASE {id} DESTROYED"), sea_glass())
                    } else if matches!(fx.kind, EffectKind::Demolition { dock: true }) {
                        // Its factories will raise the dock again unless they fall too.
                        let after = if port.live_factories() > 0 {
                            "    FACTORIES REBUILDING"
                        } else {
                            ""
                        };
                        (format!("BASE {id}    DRYDOCK DOWN{after}"), brass())
                    } else {
                        (format!("BASE {id}    FACTORY KNOCKED OUT"), brass())
                    }
                } else if let Some(stirring) = stirring(&battle) {
                    node.display = Display::Flex;
                    (stirring, coral())
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
    let next = next_base(&battle, helm.position);
    for (flag, mut message, mut node, mut border) in &mut flags {
        let Some((pos, edge)) = positions.get(flag.0).copied().flatten() else {
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        node.left = px(pos.x);
        node.top = px(pos.y);
        let port = &battle.ports()[flag.0];
        let range = anchor(port).distance(helm.position).round() as u32;
        let id = letter(flag.0);
        let reach = if range >= 1000 {
            format!("{:.1} km", range as f32 / 1000.)
        } else {
            format!("{range} m")
        };
        let words = if port.wrecked() {
            format!("BASE {id}  CLEARED")
        } else if next == Some(flag.0) {
            // The next base's flag says so, and points the way when it is off the screen.
            match edge {
                " <" => format!("< NEXT {id}  {reach}"),
                " >" => format!("NEXT {id}  {reach} >"),
                " ^" => format!("^ NEXT {id}  {reach}"),
                " v" => format!("v NEXT {id}  {reach}"),
                _ => format!("NEXT  {id}   {reach}"),
            }
        } else {
            format!("BASE {id}   {reach}")
        };
        if message.0 != words {
            message.0 = words;
        }
        let edge_width = if next == Some(flag.0) { 3 } else { 2 };
        node.border = UiRect::bottom(px(edge_width));
        let color = if next == Some(flag.0) && !port.wrecked() {
            brass()
        } else {
            status_color(port)
        };
        border.set_if_neq(BorderColor::all(color));
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
        let Some(plane) = battle.spotters().get(marker.0) else {
            node.display = Display::None;
            continue;
        };
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
