mod title;
use crate::state::{Campaign, Encounter, PlayerHelm, Screen, TrialCourse};
use bevy::{prelude::*, ui::RelativeCursorPosition};
use shipvivor_campaign::{
    Card, EXITS, EXTRACT_GATES, EXTRACT_SECONDS, Fittings, LegEnd, MAX_STARS, Perk, Stage,
};

const KNOTS_PER_MPS: f32 = 1.94384;

/// Whole compass degrees, 0 to 359.
fn degrees(heading: f32) -> i32 {
    (heading.to_degrees().round() as i32).rem_euclid(360)
}
fn knots(metres_per_second: f32) -> f32 {
    metres_per_second * KNOTS_PER_MPS
}
fn extraction_rule() -> String {
    let seconds = EXTRACT_SECONDS as u32;
    format!(
        "Exit at {EXTRACT_GATES} gates or {:02}:{:02} elapsed",
        seconds / 60,
        seconds % 60
    )
}

pub(crate) fn leg_result(reason: LegEnd) -> &'static str {
    match reason {
        LegEnd::RegentSunk => "Iron Regent sunk",
        LegEnd::BasesDestroyed => "Sector cleared",
        LegEnd::TimeExpired => "Time expired",
        LegEnd::Extracted => "Early extraction",
    }
}

fn ink() -> Color {
    Color::srgb_u8(234, 239, 222)
}
fn pale() -> Color {
    Color::srgb_u8(164, 194, 192)
}
fn brass() -> Color {
    Color::srgb_u8(255, 199, 91)
}
fn navy() -> Color {
    Color::srgba_u8(13, 32, 42, 246)
}
fn trim() -> Color {
    Color::srgb_u8(74, 106, 108)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Component)]
pub(crate) enum Action {
    Start,
    Trial,
    RestartTrial,
    Title,
    Resume,
    Pause,
    Extract,
    Card(Card),
    /// Select a section of the ship in the drydock, counted from the bow.
    Module(usize),
    Preview(usize),
    Confirm,
    Continue,
    Depart(Stage),
    Engage,
}
#[derive(Component)]
pub struct ScreenRoot;
#[derive(Component)]
pub enum Instrument {
    Navigation,
    Flagship,
    Target,
    Spotting,
    Impact,
    Engine,
    Orders,
    Experience,
}

pub fn intent(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    run: Res<Campaign>,
    pointer: Res<crate::dockyard::Pointer>,
    buttons: Query<(&Action, &RelativeCursorPosition)>,
) -> Option<Action> {
    if mouse.just_pressed(MouseButton::Left)
        && let Some((action, _)) = buttons.iter().find(|(_, p)| p.cursor_over())
    {
        return Some(*action);
    }
    let key = |k| keys.just_pressed(k);
    let choice = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3]
        .into_iter()
        .position(key);
    match run.screen() {
        Screen::Title if key(KeyCode::Enter) => Some(Action::Start),
        Screen::Title if key(KeyCode::KeyT) => Some(Action::Trial),
        Screen::Trial if key(KeyCode::KeyR) => Some(Action::RestartTrial),
        Screen::Trial | Screen::Won(_) | Screen::Lost if key(KeyCode::Escape) => {
            Some(Action::Title)
        }
        Screen::Secured if key(KeyCode::Enter) => Some(Action::Continue),
        Screen::Sailing if key(KeyCode::Escape) => Some(Action::Pause),
        Screen::Sailing if key(KeyCode::KeyE) => Some(Action::Extract),
        Screen::Paused if key(KeyCode::Escape) || key(KeyCode::Enter) => Some(Action::Resume),
        Screen::Paused if key(KeyCode::KeyM) => Some(Action::Title),
        Screen::Drydock(_) => {
            // The ship is the menu: a click on her selects the section under the cursor.
            let count = run.dock_jobs().len();
            let step = |forward: bool| {
                (count > 0).then(|| {
                    Action::Preview(match (run.dock_option, forward) {
                        (None, true) => 0,
                        (None, false) => count - 1,
                        (Some(i), true) => (i + 1) % count,
                        (Some(i), false) => (i + count - 1) % count,
                    })
                })
            };
            if mouse.just_pressed(MouseButton::Left) && !pointer.over_panel {
                pointer.hovered.map(Action::Module)
            } else if key(KeyCode::Enter) {
                Some(Action::Confirm)
            } else if key(KeyCode::Escape) {
                Some(Action::Module(run.dock_module))
            } else if key(KeyCode::BracketLeft) {
                Some(Action::Module(run.dock_module.saturating_sub(1)))
            } else if key(KeyCode::BracketRight) {
                Some(Action::Module(run.dock_module + 1))
            } else if key(KeyCode::ArrowLeft) || key(KeyCode::ArrowUp) {
                step(false)
            } else if key(KeyCode::ArrowRight) || key(KeyCode::ArrowDown) {
                step(true)
            } else {
                [
                    KeyCode::Digit1,
                    KeyCode::Digit2,
                    KeyCode::Digit3,
                    KeyCode::Digit4,
                    KeyCode::Digit5,
                    KeyCode::Digit6,
                    KeyCode::Digit7,
                    KeyCode::Digit8,
                    KeyCode::Digit9,
                ]
                .into_iter()
                .position(key)
                .map(Action::Preview)
            }
        }
        Screen::Refit => {
            choice.and_then(|i| run.fittings.offers().get(i).copied().map(Action::Card))
        }
        Screen::Intercept(_) if key(KeyCode::Enter) => Some(Action::Engage),
        Screen::Chart => choice.map(|i| Action::Depart(EXITS[i].stage)),
        Screen::Won(_) | Screen::Lost if key(KeyCode::Enter) => Some(Action::Start),
        _ => None,
    }
}
pub fn scale(window: Single<&Window>, mut scale: ResMut<UiScale>) {
    let size = (window.width() / 1000.)
        .min(window.height() / 650.)
        .clamp(0.45, 1.);
    if scale.0 != size {
        scale.0 = size;
    }
}
fn column(gap: f32) -> Node {
    Node {
        flex_direction: FlexDirection::Column,
        row_gap: px(gap),
        ..default()
    }
}
fn text(
    cmd: &mut Commands,
    parent: Entity,
    s: impl Into<String>,
    size: f32,
    color: Color,
) -> Entity {
    cmd.spawn((
        Text::new(s.into()),
        TextFont::from_font_size(size),
        TextColor(color),
        ChildOf(parent),
    ))
    .id()
}
fn action(
    cmd: &mut Commands,
    parent: Entity,
    opt: Option<Action>,
    title: &str,
    detail: &str,
    card: bool,
) -> Entity {
    let node = cmd
        .spawn((
            Node {
                padding: UiRect::axes(px(16), px(12)),
                border: UiRect::left(px(3)),
                flex_basis: if card { px(0) } else { auto() },
                flex_grow: if card { 1. } else { 0. },
                min_width: px(0),
                min_height: if card { px(146) } else { auto() },
                ..column(7.)
            },
            BackgroundColor(if opt.is_some() {
                button_color(false)
            } else {
                Color::srgb_u8(29, 45, 51)
            }),
            BorderColor::all(if opt.is_some() { brass() } else { trim() }),
            ChildOf(parent),
        ))
        .id();
    if let Some(a) = opt {
        cmd.entity(node)
            .insert((a, RelativeCursorPosition::default()));
    }
    text(
        cmd,
        node,
        title,
        if card { 20. } else { 18. },
        if opt.is_some() { ink() } else { pale() },
    );
    if !detail.is_empty() {
        text(cmd, node, detail, 14., pale());
    }
    node
}
fn button_color(hover: bool) -> Color {
    if hover {
        Color::srgb_u8(52, 86, 89)
    } else {
        Color::srgb_u8(30, 61, 68)
    }
}
/// Menu buttons. Fitting cards light themselves.
type Buttons = (
    With<Action>,
    Without<crate::cards::Dealt>,
    Without<crate::dockyard::YardButton>,
);

pub fn hover(
    run: Res<Campaign>,
    mut buttons: Query<(&Action, &RelativeCursorPosition, &mut BackgroundColor), Buttons>,
) {
    for (action, cursor, mut bg) in &mut buttons {
        let desired = if run.screen() == Screen::Title {
            title::fill(*action, cursor.cursor_over())
        } else {
            button_color(cursor.cursor_over())
        };
        if bg.0 != desired {
            bg.0 = desired;
        }
    }
}
fn menu_panel(cmd: &mut Commands, root: Entity, width: f32) -> Entity {
    cmd.spawn((
        Node {
            width: px(width),
            max_width: percent(92),
            padding: UiRect::all(px(26)),
            border: UiRect::top(px(3)),
            ..column(12.)
        },
        BorderColor::all(brass()),
        BackgroundColor(navy()),
        ChildOf(root),
    ))
    .id()
}
// Everything a menu screen is built from. The HUD updates its own text every frame.
type ScreenKey = (Screen, Stage, Fittings, u16, Option<usize>, usize, bool);

pub fn rebuild_screen(
    art: Res<crate::insignia::Insignia>,
    chart: Res<crate::minimap::ChartImage>,
    plans: (Local<crate::gunnery::DeckPlan>, ResMut<Assets<Image>>),
    mut cmd: Commands,
    patrol: (Res<Campaign>, Res<Encounter>),
    old: Query<Entity, With<ScreenRoot>>,
    mut last: Local<Option<ScreenKey>>,
) {
    let (run, battle) = patrol;
    let key = (
        run.screen(),
        run.stage,
        run.fittings,
        run.fittings_remaining(),
        run.dock_option,
        run.dock_module,
        run.dock_armed,
    );
    if *last == Some(key) {
        return;
    }
    *last = Some(key);
    for e in &old {
        cmd.entity(e).despawn();
    }
    let sailing = matches!(run.screen(), Screen::Sailing | Screen::Trial);
    let root = cmd
        .spawn((
            ScreenRoot,
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..column(0.)
            },
        ))
        .id();
    if sailing {
        let (mut plan, mut images) = plans;
        let deck = (run.screen() == Screen::Sailing).then(|| plan.image(&run, &mut images));
        hud(
            &mut cmd,
            root,
            &run,
            chart.0.clone(),
            art.contact.clone(),
            deck,
        );
        return;
    }
    if run.screen() == Screen::Secured {
        crate::mission_hud::clearance(&mut cmd, root, &battle, &run);
        return;
    }
    if let Screen::Drydock(reason) = run.screen() {
        crate::dockyard::screen(&mut cmd, root, &run, &battle, &art, reason);
        return;
    }
    if run.screen() == Screen::Title {
        title::show(&mut cmd, root, &art.display_face);
        return;
    }
    cmd.entity(root)
        .insert(BackgroundColor(Color::srgba_u8(6, 18, 26, 145)));
    let width = if run.screen() == Screen::Refit {
        900.
    } else {
        560.
    };
    let panel = menu_panel(&mut cmd, root, width);
    match run.screen() {
        Screen::Refit => {
            text(
                &mut cmd,
                panel,
                if run.stage == Stage::PilotsCut && run.elapsed == 0. {
                    "PILOT'S CUT   /   DOCKYARD FITTING".into()
                } else {
                    format!("LEVEL {}   /   FIELD REFIT", run.level)
                },
                14.,
                brass(),
            );
            text(&mut cmd, panel, "Choose a fitting", 36., ink());
            text(
                &mut cmd,
                panel,
                "Select one card. Your fittings stay aboard for the run.",
                16.,
                pale(),
            );
            crate::cards::hand(&mut cmd, panel, run.fittings, &art);
        }
        Screen::Chart => {
            text(&mut cmd, panel, "PASSAGE CHART   /   LEG 02", 14., brass());
            text(&mut cmd, panel, "Choose your exit", 34., ink());
            text(
                &mut cmd,
                panel,
                format!(
                    "{} / {MAX_STARS} stars earned in the Iron Narrows",
                    run.stars()
                ),
                18.,
                brass(),
            );
            let stars = cmd
                .spawn((
                    Node {
                        column_gap: px(5),
                        ..default()
                    },
                    ChildOf(panel),
                ))
                .id();
            for n in 0..MAX_STARS {
                cmd.spawn((
                    ImageNode {
                        image: art.star.clone(),
                        color: if n < run.stars() { brass() } else { trim() },
                        ..default()
                    },
                    Node {
                        width: px(24),
                        height: px(24),
                        ..default()
                    },
                    ChildOf(stars),
                ));
            }
            for (i, exit) in EXITS.into_iter().enumerate() {
                let (stage, required) = (exit.stage, exit.stars);
                let open = run.stars() >= required;
                let description = match exit.perk {
                    Perk::None => "Wide channels, frequent patrols".to_string(),
                    Perk::Repair(_) => "Hull repairs on arrival".to_string(),
                    Perk::FreeFitting => "Tight turns, free fitting on arrival".to_string(),
                };
                let detail = format!(
                    "{}   /   {}",
                    if open {
                        "Open".into()
                    } else {
                        format!("Locked - needs {required} stars")
                    },
                    description
                );
                action(
                    &mut cmd,
                    panel,
                    open.then_some(Action::Depart(stage)),
                    &format!(
                        "{}   {}     {} stars",
                        i + 1,
                        stage.mission().name,
                        required
                    ),
                    &detail,
                    false,
                );
            }
        }
        Screen::Intercept(reason) => {
            text(
                &mut cmd,
                panel,
                "FINAL INTERCEPT   /   CROWN DEEP",
                14.,
                brass(),
            );
            text(&mut cmd, panel, "Sink the Iron Regent", 34., ink());
            text(
                &mut cmd,
                panel,
                format!("{}   /   {}", run.stage.mission().name, leg_result(reason)),
                17.,
                pale(),
            );
            text(
                &mut cmd,
                panel,
                "A 168-metre dreadnought is crossing the deep with her escorts. Break her twelve gold turrets to expose the hull. Change course when a turret flashes. She escapes in fifteen minutes.",
                18.,
                ink(),
            );
            action(
                &mut cmd,
                panel,
                Some(Action::Engage),
                "Intercept     [Enter]",
                "Repair crews restore up to 40 hull before departure.",
                false,
            );
        }
        Screen::Won(_) | Screen::Lost => {
            text(
                &mut cmd,
                panel,
                "BREAKWATER   /   AFTER-ACTION REPORT",
                14.,
                brass(),
            );
            text(
                &mut cmd,
                panel,
                match run.screen() {
                    Screen::Won(_) => "Iron Regent sunk",
                    Screen::Lost if run.ship.afloat() && run.stage == Stage::CrownDeep => {
                        "The Regent escaped"
                    }
                    _ => "Ship lost",
                },
                38.,
                ink(),
            );
            let tally = if run.stage == Stage::IronNarrows {
                format!("{} / {MAX_STARS} stars", run.stars())
            } else {
                format!(
                    "{} / {} run stars",
                    run.previous_stars + run.stars(),
                    2 * MAX_STARS
                )
            };
            text(
                &mut cmd,
                panel,
                format!(
                    "{}   /   {}\n{} raiders sunk   /   {} stores recovered this leg\nRun    Ship hits {}    Sunk {}",
                    run.stage.mission().name,
                    tally,
                    run.objectives.kills,
                    run.objectives.crates,
                    run.hits,
                    run.sunk
                ),
                17.,
                pale(),
            );
            action(
                &mut cmd,
                panel,
                Some(Action::Start),
                "New run     [Enter]",
                "",
                false,
            );
            action(
                &mut cmd,
                panel,
                Some(Action::Title),
                "Title menu     [Esc]",
                "",
                false,
            );
        }
        Screen::Paused => {
            text(&mut cmd, panel, "At the helm", 34., ink());
            action(
                &mut cmd,
                panel,
                Some(Action::Resume),
                "Resume patrol     [Esc]",
                "",
                false,
            );
            action(
                &mut cmd,
                panel,
                Some(Action::Title),
                "End run and return to title     [M]",
                "",
                false,
            );
        }
        _ => {}
    }
}
fn hud(
    cmd: &mut Commands,
    root: Entity,
    run: &Campaign,
    chart: Handle<Image>,
    contact_icon: Handle<Image>,
    deck: Option<Handle<Image>>,
) {
    if run.screen() == Screen::Sailing {
        crate::mission_hud::spawn(cmd, root, run.stage);
        crate::flagship_hud::spawn(cmd, root);
        crate::gun_sight::spawn(cmd, root, run);
        crate::fire_control::spawn(cmd, root, contact_icon);
        let panel = cmd
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: px(18),
                    top: px(18),
                    padding: UiRect::all(px(8)),
                    border: UiRect::all(px(1)),
                    ..column(4.)
                },
                BorderColor::all(trim()),
                BackgroundColor(navy()),
                ChildOf(root),
            ))
            .id();
        text(cmd, panel, "CHART    NORTH UP", 14., ink());
        cmd.spawn((
            ImageNode::new(chart),
            Node {
                width: px(192),
                height: px(192),
                ..default()
            },
            ChildOf(panel),
        ));
        text(
            cmd,
            panel,
            "200 m grid     [ ] Next gate\nA B C   Enemy drydocks",
            12.,
            pale(),
        );
        let target = text(cmd, panel, "", 13., brass());
        cmd.entity(target).insert(Instrument::Target);
        let spotted = text(cmd, panel, "", 12., ink());
        cmd.entity(spotted).insert(Instrument::Spotting);
        if run.stage != shipvivor_campaign::Stage::IronNarrows {
            let report = text(cmd, panel, "", 13., brass());
            cmd.entity(report).insert(Instrument::Flagship);
        }
        let key = cmd
            .spawn((
                Node {
                    column_gap: px(9),
                    ..default()
                },
                ChildOf(panel),
            ))
            .id();
        for (s, c) in [
            ("Cleared", Color::srgb_u8(145, 219, 195)),
            ("Raiders", Color::srgb_u8(255, 124, 102)),
            ("Stores", brass()),
        ] {
            text(cmd, key, s, 12., c);
        }
    }
    // Centred by the full-viewport HUD root. The corner panels are absolutely positioned.
    let sight = cmd
        .spawn((
            Node {
                width: px(32),
                height: px(20),
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    if run.screen() == Screen::Sailing {
        let report = text(cmd, sight, "", 14., brass());
        cmd.entity(report).insert((
            Instrument::Impact,
            TextLayout::justify(Justify::Center),
            TextShadow::default(),
            Node {
                position_type: PositionType::Absolute,
                top: px(67),
                left: px(-94),
                width: px(220),
                ..default()
            },
        ));
    }
    for (x, y, w, h) in [
        (0., 9., 12., 3.),
        (20., 9., 12., 3.),
        (15., 13., 3., 7.),
        (15., 8., 3., 3.),
    ] {
        cmd.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(x),
                top: px(y),
                width: px(w),
                height: px(h),
                border: UiRect::all(px(0.75)),
                ..default()
            },
            BackgroundColor(ink()),
            BorderColor::all(Color::srgb_u8(23, 42, 46)),
            ChildOf(sight),
        ));
    }
    for (inst, title, top, right) in [
        (
            Instrument::Navigation,
            if run.screen() == Screen::Trial {
                "SOUNDING RUN"
            } else {
                run.stage.mission().name
            },
            true,
            false,
        ),
        (Instrument::Engine, "BREAKWATER", false, false),
        (Instrument::Orders, "HELM ORDERS", false, true),
    ] {
        let panel = cmd
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: if right { auto() } else { px(18) },
                    right: if right { px(18) } else { auto() },
                    top: if top { px(18) } else { auto() },
                    bottom: if top { auto() } else { px(18) },
                    padding: UiRect::axes(px(14), px(9)),
                    border: UiRect::left(px(3)),
                    ..column(4.)
                },
                BackgroundColor(navy()),
                BorderColor::all(brass()),
                ChildOf(root),
            ))
            .id();
        let title = if matches!(inst, Instrument::Engine) && run.screen() == Screen::Sailing {
            format!("{title}   /   {:.0} m", run.ship.design().length())
        } else {
            title.to_uppercase()
        };
        text(cmd, panel, title, 16., brass());
        let e = text(cmd, panel, "", 14., ink());
        if matches!(inst, Instrument::Navigation)
            && run.screen() == Screen::Sailing
            && run.stage == Stage::IronNarrows
        {
            text(
                cmd,
                panel,
                format!(
                    "{} stars   Reserve hold\n3 docks   Engine room",
                    shipvivor_campaign::RESERVE_STARS
                ),
                12.,
                pale(),
            );
        }
        if matches!(inst, Instrument::Engine) {
            if let Some(deck) = &deck {
                crate::gunnery::spawn(cmd, panel, deck.clone(), run);
                crate::damage_control::spawn(cmd, panel, run);
            }
            let row = cmd
                .spawn((
                    Node {
                        column_gap: px(3),
                        ..default()
                    },
                    ChildOf(panel),
                ))
                .id();
            for notch in -3_i8..=4 {
                let cell = cmd
                    .spawn((
                        Notch(notch),
                        Node {
                            width: px(31),
                            height: px(20),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(trim()),
                        ChildOf(row),
                    ))
                    .id();
                let label = if notch > 0 {
                    format!("+{notch}")
                } else {
                    notch.to_string()
                };
                text(cmd, cell, label, 12., ink());
            }
            if run.screen() == Screen::Sailing {
                let xp = text(cmd, panel, "", 13., brass());
                cmd.entity(xp).insert(Instrument::Experience);
                let track = cmd
                    .spawn((
                        Node {
                            width: percent(100),
                            height: px(4),
                            ..default()
                        },
                        BackgroundColor(trim()),
                        ChildOf(panel),
                    ))
                    .id();
                cmd.spawn((
                    XpBar,
                    Node {
                        height: percent(100),
                        ..default()
                    },
                    BackgroundColor(brass()),
                    ChildOf(track),
                ));
            }
        }
        cmd.entity(e).insert(inst);
    }
}
#[derive(Component)]
pub struct Notch(i8);
#[derive(Component)]
pub struct XpBar;
pub fn gauges(
    helm: Res<PlayerHelm>,
    run: Res<Campaign>,
    mut cells: Query<(&Notch, &mut BackgroundColor)>,
    mut bars: Query<&mut Node, With<XpBar>>,
) {
    for (notch, mut color) in &mut cells {
        color.0 = if notch.0 == helm.notch() {
            Color::srgb_u8(153, 92, 31)
        } else {
            Color::srgb_u8(47, 70, 77)
        };
    }
    for mut node in &mut bars {
        node.width = percent(100. * run.experience as f32 / run.next_level_xp() as f32);
    }
}

pub fn instruments(
    run: Res<Campaign>,
    battle: Res<Encounter>,
    helm: Res<PlayerHelm>,
    view: Res<crate::camera::ShipView>,
    guns: Res<crate::state::Guns>,
    course: Res<TrialCourse>,
    mut labels: Query<(&Instrument, &mut Text)>,
) {
    for (inst, mut label) in &mut labels {
        let value = match inst {
            Instrument::Navigation => {
                let goal = run.stage.mission();
                let p = course.next_gate().center - helm.position;
                let bearing = degrees(shipvivor_compass::heading(p));
                if run.screen() == Screen::Trial {
                    format!(
                        "Lap {}   Gate {} / {}\nGold buoys   {bearing:03} deg   {:.0} m",
                        course.lap(),
                        course.active() + 1,
                        course.gates().len(),
                        p.length()
                    )
                } else if run.stage == Stage::CrownDeep {
                    format!(
                        "Sink the Iron Regent\nEscorts sunk   {}   /   Stores recovered   {}",
                        run.objectives.kills, run.objectives.crates
                    )
                } else {
                    let g = run.objectives;
                    format!(
                        "Raiders {} / {}     Stores  {} / {}\nGates  {} / {}     {:03} deg / {:.0} m\nShip hits {}     Sunk {}\n{}",
                        g.kills,
                        goal.kills,
                        g.crates,
                        goal.crates,
                        g.gates,
                        goal.gates,
                        bearing,
                        p.length(),
                        run.hits,
                        run.sunk,
                        if run.can_extract() {
                            "[E]  Extract".to_string()
                        } else {
                            extraction_rule()
                        }
                    )
                }
            }
            Instrument::Target => view.target(&battle).map_or_else(String::new, |r| {
                let selection = if view.locked() { "LOCK" } else { "AUTO" };
                format!("{selection}   {:.0} m", helm.position.distance(r.body.pos))
            }),
            Instrument::Spotting => {
                let contact = view.sight(helm.position, &battle);
                if let Some(class) = contact.ship {
                    let name = crate::mission_hud::ship_class(class);
                    if let Some(live) = contact.director {
                        format!(
                            "{name}  /  {}",
                            if live { "Director" } else { "Director out" }
                        )
                    } else {
                        match contact.turret {
                            Some(n) if n > class.main_count() => {
                                format!("{name}  /  Secondary {}", n - class.main_count())
                            }
                            Some(n) => format!("{name}  /  Turret {n}"),
                            None => name.to_owned(),
                        }
                    }
                } else if let Some((base, factory)) = contact.factory {
                    let port = &battle.ports()[base];
                    let condition = if port.factories()[factory].live() {
                        "Online"
                    } else {
                        "Offline"
                    };
                    format!(
                        "Factory {}{}  /  {condition}\nBase output   {:.0} steel/min",
                        crate::mission_hud::letter(base),
                        factory + 1,
                        port.production_per_minute()
                    )
                } else if let Some((base, index)) = contact.shore_turret {
                    let gun = &battle.ports()[base].turrets()[index];
                    let id = crate::mission_hud::letter(base);
                    format!(
                        "Shore turret {id}{}  /  {} {}\nMount strength   {} / {}",
                        index + 1,
                        gun.barrels,
                        if gun.barrels == 1 {
                            "barrel"
                        } else {
                            "barrels"
                        },
                        gun.strength(),
                        gun.max_strength()
                    )
                } else if let Some(index) = contact.yard {
                    let port = &battle.ports()[index];
                    let id = crate::mission_hud::letter(index);
                    if port.yard_state() == shipvivor_encounter::YardState::Ruined {
                        format!("Base {id} destroyed\nLaunch slip closed")
                    } else {
                        let (class, cost) = port.build_order();
                        let order = if port.stock >= cost {
                            "Funded"
                        } else {
                            "Building"
                        };
                        format!(
                            "Drydock {id}  /  {:?}\n{order}  {}\nSteel   {} stored / {cost} needed\nFactories online   {} / {}",
                            port.yard_state(),
                            crate::mission_hud::ship_class(class),
                            port.stock,
                            port.live_factories(),
                            port.factories().len()
                        )
                    }
                } else {
                    String::new()
                }
            }
            Instrument::Impact if guns.torpedo_mode => {
                use shipvivor_encounter::Rack;
                match Rack::for_aim(helm.heading, view.bearing) {
                    None => "Turn to a broadside".to_string(),
                    Some(r) if battle.rack_reload(r) > 0. => {
                        format!("{r:?} tubes   /   {:.0} s", battle.rack_reload(r).ceil())
                    }
                    Some(r) => format!("{r:?} tubes ready"),
                }
            }
            Instrument::Impact => {
                use shipvivor_encounter::Impact;
                match battle.salvo_report() {
                    Some(Impact::Bounced) => "RICOCHET",
                    Some(Impact::Embedded) => "ARMOR HELD",
                    Some(Impact::Penetrated) => "PENETRATION",
                    Some(Impact::PassedThrough) => "OPEN BREACH",
                    Some(Impact::Overpenetrated) => "OVERPENETRATION",
                    None => "",
                }
                .into()
            }
            Instrument::Flagship => {
                if let Some(ship) = battle.raiders().iter().find(|s| s.class.is_flagship()) {
                    let n = ship.live_guns();
                    let range = helm.position.distance(ship.body.pos).round() as u32;
                    let name = crate::mission_hud::ship_class(ship.class).to_uppercase();
                    if n > 0 {
                        format!(
                            "{name}    {range} m\n{n} / {} gun mounts intact\nBreak the gold turrets",
                            ship.guns().len()
                        )
                    } else {
                        let (flooded, fatal) = ship.vessel().flooding();
                        format!(
                            "{name}    {range} m\nHull exposed  •  {flooded} / {fatal} compartments flooded"
                        )
                    }
                } else if let Some(shipvivor_encounter::Sortie::Due(due)) = battle.flagship() {
                    let left = (due - battle.at_sea()).max(0.).ceil() as u32;
                    if left > 0 {
                        format!(
                            "BRASS PARLIAMENT    fitting out\nPuts to sea in {}:{:02}",
                            left / 60,
                            left % 60
                        )
                    } else {
                        "BRASS PARLIAMENT    fitting out\nPutting to sea".into()
                    }
                } else {
                    "BRASS PARLIAMENT SUNK".into()
                }
            }
            Instrument::Experience => format!(
                "Level {}    XP   {} / {}",
                run.level,
                run.experience,
                run.next_level_xp()
            ),
            Instrument::Engine => {
                let helm_info = format!(
                    "{}   {:+.1} kn    HDG {:03}",
                    helm.order_label(),
                    knots(helm.speed),
                    degrees(helm.heading)
                );
                if run.screen() == Screen::Trial {
                    format!("{helm_info}\nRudder {:+.0} deg", helm.rudder * 35.)
                } else {
                    // The main battery has its own board in the centre.
                    let batteries = if guns.torpedo_mode {
                        let [port, starboard] = shipvivor_encounter::Rack::BOTH.map(|r| {
                            let left = battle.rack_reload(r);
                            if left > 0. {
                                format!("{left:.0}s")
                            } else {
                                "READY".into()
                            }
                        });
                        format!("\nTubes   P {port}   S {starboard}")
                    } else {
                        String::new()
                    };
                    format!(
                        "{helm_info}\nHull {}{batteries}",
                        match run.hull_percent() {
                            // One more wrecked module would sink her.
                            _ if run.ship.in_peril() => "critical",
                            100 => "sound",
                            70.. => "scarred",
                            35.. => "battered",
                            _ => "critical",
                        }
                    )
                }
            }
            Instrument::Orders => {
                if run.screen() == Screen::Trial {
                    "Tap W / S   One engine notch     A / D   Rudder\nMouse   Look     Shift   Zoom     C   Astern\nR   Reset course         Esc   Title"
                        .into()
                } else {
                    let weapon = if guns.torpedo_mode {
                        "Click  Launch spread (420 m)"
                    } else {
                        "Click  One gun   Double-click  Salvo\nHold  Ripple     Space  At target"
                    };
                    let ammo = if run.fittings.armor_piercing() {
                        "   Q  HE / AP"
                    } else {
                        ""
                    };
                    format!(
                        "W/S  Engine   A/D  Rudder   C  Astern\nX  Pin / release   T  Gun / torpedo\nMouse  Look   RMB + drag  Trim lock\n{weapon}\nShift  Scope{ammo}   Esc  Pause"
                    )
                }
            }
        };
        if label.0 != value {
            label.0 = value;
        }
    }
}
