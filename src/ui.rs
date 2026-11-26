pub(crate) mod customise;
pub(crate) mod options;
pub(crate) mod passage_chart;
mod title;
use crate::state::{Campaign, Encounter, PlayerHelm, Screen, TrialCourse};
use bevy::{prelude::*, ui::RelativeCursorPosition};
use shipvivor_campaign::{
    ANCHORAGES, Card, Fittings, LegEnd, MAX_STARS, PATROLS, Perk, ROUNDS, Stage,
};

const KNOTS_PER_MPS: f32 = 1.94384;
const CHART_TABLE_WIDTH: f32 = 964.;

/// Whole compass degrees, 0 to 359.
fn degrees(heading: f32) -> i32 {
    (heading.to_degrees().round() as i32).rem_euclid(360)
}
fn knots(metres_per_second: f32) -> f32 {
    metres_per_second * KNOTS_PER_MPS
}
/// The leg's clock as the instruments read it: the time left to beat, or once it has
/// run out, how far over she is.
pub(crate) fn clock_reading(run: &Campaign) -> String {
    let left = run.remaining();
    let whole = left.abs().ceil() as u32;
    let sign = if run.late() { "+" } else { "" };
    format!("{sign}{:02}:{:02}", whole / 60, whole % 60)
}

/// This release build's number, counted up by `build.rs`. Dev builds have none.
pub(crate) const BUILD: Option<&str> = option_env!("SHIPVIVOR_BUILD");

/// "Build 42", or "Dev build", as the title screen shows it.
pub(crate) fn build_label() -> String {
    BUILD.map_or_else(|| "Dev build".into(), |number| format!("Build {number}"))
}

pub(crate) fn leg_result(reason: LegEnd) -> &'static str {
    match reason {
        LegEnd::RegentSunk => "Iron Regent sunk",
        LegEnd::BasesDestroyed => "Sector cleared",
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
/// The fill of the yard-point meter, set apart from the brass of the level bar.
fn cyan() -> Color {
    Color::srgb_u8(124, 229, 218)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Component)]
pub(crate) enum Action {
    Start,
    Trial,
    RestartTrial,
    Title,
    Resume,
    Pause,
    ToggleChart,
    Card(Card),
    /// Select a section of the ship in the drydock, counted from the bow.
    Module(usize),
    /// Pick out a refit on the selected section's list, to read its figures.
    Preview(usize),
    /// Order a refit on the selected section's list.
    Order(usize),
    /// Pick out work on the hull panel, to read its figures.
    Pick(shipvivor_campaign::DockJob),
    /// Order work from the hull panel, or a section from the yard's stock dropped onto
    /// the ship.
    Draft(shipvivor_campaign::DockJob),
    /// Take back an order the yard has not yet built.
    Undo(usize),
    /// Order the refit picked out, or with none picked out, build every order.
    Confirm,
    /// Leave the drydock for the chart once every work order is spent.
    Sail,
    Continue,
    Depart(Stage),
    Engage,
    /// Open the customise screen from the title.
    Customise,
    /// On the customise screen: fly a pattern, choose which of its slots to colour,
    /// colour it, or spin the wheel for a pattern and its colours together.
    Pattern(shipvivor_ensign::Pattern),
    Slot(usize),
    Tincture(shipvivor_ensign::Tincture),
    Spin,
    /// Open the options screen from the title.
    Options,
    /// On the options screen: how the ship earns experience.
    Rewards(shipvivor_encounter::Rewards),
}
#[derive(Component)]
pub struct ScreenRoot;
#[derive(Component)]
pub enum Instrument {
    Navigation,
    Flagship,
    Target,
    Spotting,
    AirRaid,
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
    buttons: Query<(
        &Action,
        &RelativeCursorPosition,
        Option<&crate::cards::Dealt>,
    )>,
) -> Option<Action> {
    // A fitting card still dealing in can sweep under the pointer. It is not a choice yet.
    if mouse.just_pressed(MouseButton::Left)
        && let Some((action, ..)) = buttons
            .iter()
            .find(|(_, p, card)| p.cursor_over() && card.is_none_or(|c| c.landed()))
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
        Screen::Title if key(KeyCode::KeyC) => Some(Action::Customise),
        Screen::Title if key(KeyCode::KeyO) => Some(Action::Options),
        Screen::Options if key(KeyCode::Escape) || key(KeyCode::Enter) => Some(Action::Title),
        Screen::Options => choice
            .and_then(|i| options::REWARDS.get(i))
            .map(|&(rewards, ..)| Action::Rewards(rewards)),
        Screen::Departure if key(KeyCode::Escape) => Some(Action::Title),
        Screen::Departure => choice.map(|i| Action::Depart(ANCHORAGES[i].stage)),
        Screen::Customise if key(KeyCode::Escape) || key(KeyCode::Enter) => Some(Action::Title),
        Screen::Customise if key(KeyCode::KeyR) => Some(Action::Spin),
        Screen::Customise => [
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::Digit5,
            KeyCode::Digit6,
        ]
        .into_iter()
        .position(key)
        .map(Action::Slot),
        Screen::Trial if key(KeyCode::KeyR) => Some(Action::RestartTrial),
        Screen::Trial | Screen::Won(_) | Screen::Lost if key(KeyCode::Escape) => {
            Some(Action::Title)
        }
        Screen::Secured if key(KeyCode::Enter) => Some(Action::Continue),
        Screen::Sailing if key(KeyCode::Escape) => Some(Action::Pause),
        Screen::Sailing | Screen::PassageMap if key(KeyCode::Tab) => Some(Action::ToggleChart),
        Screen::PassageMap
            if key(KeyCode::Escape) && run.chart_roll.is_some_and(|roll| roll.opening) =>
        {
            Some(Action::ToggleChart)
        }
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
            if let Some((at, module)) = pointer.dropped {
                // A section from the yard's stock let go over a seam is tried there.
                Some(Action::Draft(shipvivor_campaign::DockJob::Insert {
                    at,
                    module,
                }))
            } else if pointer.carrying.is_some() {
                None
            } else if mouse.just_pressed(MouseButton::Left) && !pointer.over_panel {
                pointer.hovered.map(Action::Module)
            } else if key(KeyCode::Enter) {
                // Enter orders the refit picked out, and one the yard cannot make stays
                // picked out, its figures saying why: it never sails her by mistake. With
                // nothing picked out it does what the big button does: locks in the
                // orders, then sails.
                Some(if run.dock_job().is_some() {
                    Action::Confirm
                } else {
                    Action::Sail
                })
            } else if key(KeyCode::Backspace) {
                // The last order made is taken back.
                run.plan()
                    .and_then(|plan| plan.orders().len().checked_sub(1))
                    .map(Action::Undo)
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
        Screen::Refit => choice.and_then(|i| run.offers().get(i).copied().map(Action::Card)),
        Screen::Intercept(_) if key(KeyCode::Enter) => Some(Action::Engage),
        Screen::Chart => choice.and_then(|i| Some(Action::Depart(run.exits()?[i].stage))),
        Screen::Won(_) | Screen::Lost if key(KeyCode::Enter) => Some(Action::Start),
        _ => None,
    }
}
fn shows_chart_table(screen: Screen) -> bool {
    matches!(
        screen,
        Screen::Departure | Screen::Chart | Screen::Intercept(_) | Screen::Paused
    )
}

pub fn scale(window: Single<&Window>, run: Res<Campaign>, mut scale: ResMut<UiScale>) {
    let screen = run.screen();
    let bridge_scale = (window.width() / 1000.)
        .min(window.height() / 650.)
        .clamp(0.45, 1.);
    let size = if screen == Screen::PassageMap {
        passage_chart::reading_scale(window.size())
    } else if matches!(screen, Screen::Drydock(_)) {
        // The yard's panels grow on a large screen, so its controls stay easy to hit.
        (window.width() / 2000.)
            .min(window.height() / 1150.)
            .max(bridge_scale)
            .min(1.6)
    } else if shows_chart_table(screen) {
        // Leave the table its usual side margins and room for the longest set of orders.
        (window.width() * 0.92 / CHART_TABLE_WIDTH)
            .min(window.height() / 600.)
            .max(bridge_scale)
    } else {
        bridge_scale
    };
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
    mut buttons: Query<
        (
            &Action,
            &RelativeCursorPosition,
            &mut BackgroundColor,
            Has<passage_chart::ChartName>,
        ),
        Buttons,
    >,
) {
    for (action, cursor, mut bg, chart_name) in &mut buttons {
        let desired = if matches!(
            run.screen(),
            Screen::Title | Screen::Customise | Screen::Options
        ) {
            title::fill(*action, cursor.cursor_over())
        } else if chart_name {
            if cursor.cursor_over() {
                brass()
            } else {
                passage_chart::PAPER.with_alpha(0.92)
            }
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
type ScreenKey = (
    Screen,
    Stage,
    Fittings,
    u16,
    Option<usize>,
    usize,
    (u32, Option<shipvivor_campaign::DockJob>),
);

pub fn rebuild_screen(
    art: Res<crate::insignia::Insignia>,
    captain: Res<crate::captain::CaptainArt>,
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
        (run.dock_revision, run.dock_draft),
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
    if let Some(roll) = run
        .chart_roll
        .filter(|_| run.screen() == Screen::PassageMap)
    {
        passage_chart::unfurl(&mut cmd, root, &run, &art, roll);
        return;
    }
    if sailing {
        let (mut plan, mut images) = plans;
        let deck = (run.screen() == Screen::Sailing).then(|| plan.image(&run, &mut images));
        hud(
            &mut cmd,
            root,
            &run,
            chart.0.clone(),
            (art.contact.clone(), art.order.clone()),
            deck,
            &captain,
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
    if run.screen() == Screen::Customise {
        customise::show(&mut cmd, root, &art);
        return;
    }
    if run.screen() == Screen::Options {
        options::show(&mut cmd, root, &art.display_face);
        return;
    }
    cmd.entity(root)
        .insert(BackgroundColor(Color::srgba_u8(6, 18, 26, 145)));
    let nautical = shows_chart_table(run.screen());
    let width = if nautical {
        CHART_TABLE_WIDTH
    } else if run.screen() == Screen::Refit {
        900.
    } else {
        560.
    };
    let panel = menu_panel(&mut cmd, root, width);
    let panel = if nautical {
        passage_chart::table(&mut cmd, panel, &run, &art)
    } else {
        panel
    };
    match run.screen() {
        Screen::Departure => {
            text(&mut cmd, panel, "Choose your anchorage", 25., ink());
            for (i, port) in ANCHORAGES.iter().enumerate() {
                let mission = port.stage.mission();
                action(
                    &mut cmd,
                    panel,
                    Some(Action::Depart(port.stage)),
                    &format!("[{}]  {}", i + 1, mission.name),
                    &format!("{} min. {}", mission.minutes, port.pilot_note),
                    false,
                );
            }
            action(
                &mut cmd,
                panel,
                Some(Action::Title),
                "Back to title     [Esc]",
                "",
                false,
            );
        }
        Screen::Refit => {
            text(
                &mut cmd,
                panel,
                if run.stage.exit().perk == Perk::FreeFitting && run.elapsed == 0. {
                    format!(
                        "{}   /   DOCKYARD FITTING",
                        run.stage.mission().name.to_uppercase()
                    )
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
            crate::cards::hand(&mut cmd, panel, &run, &art);
        }
        Screen::Chart => {
            text(&mut cmd, panel, "Choose your passage", 26., ink());
            // Every passage of a round leads to the same flagship, so the chart leaves
            // it to this heading.
            let flagship = run
                .exits()
                .and_then(|exits| shipvivor_sim::Sector::of(exits[0].stage).boss())
                .map_or(String::new(), |class| {
                    format!("\nFlagship: {}", crate::mission_hud::ship_class(class))
                });
            text(
                &mut cmd,
                panel,
                format!(
                    "{}   /   {} / {MAX_STARS} stars\nNext: round {} of {ROUNDS}{flagship}",
                    run.stage.mission().name,
                    run.stars(),
                    run.stage.round() + 1
                ),
                16.,
                brass(),
            );
            for (i, exit) in run.exits().into_iter().flatten().enumerate() {
                let (stage, required) = (exit.stage, exit.stars);
                let open = run.stars() >= required;
                let description = match exit.perk {
                    Perk::None => "Frequent patrols".to_string(),
                    Perk::Repair(hull) => format!("Repair up to {hull} hull"),
                    Perk::FreeFitting => "Free fitting on arrival".to_string(),
                };
                // The spanner count on the chart, spelled out: every star at the
                // lane's rate, the same again for beating the clock, and every base,
                // before any cargo. Two lines, so the narrow card never breaks a phrase.
                let pay = format!(
                    "\nUp to {} points:\n{} a star and the clock, {} a base",
                    shipvivor_sim::Sector::of(stage).most_points(),
                    stage.star_points(),
                    shipvivor_campaign::YARD_POINTS
                );
                let air = match (stage.enters_air_cover(), stage.is_last_patrol()) {
                    (true, true) => "\nStrafers here, bombers at Crown Deep",
                    (true, false) => "\nStrafers fly from an air station here",
                    _ => "",
                };
                // One fact a line, so a narrow card never wraps the lock or the perk
                // into the line beside it. The lit card already says it is open.
                let lock = if open {
                    String::new()
                } else {
                    format!("Locked - needs {required} stars\n")
                };
                let detail = format!("{lock}{description}{pay}{air}");
                action(
                    &mut cmd,
                    panel,
                    open.then_some(Action::Depart(stage)),
                    &format!("[{}]  {}", i + 1, stage.mission().name),
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
                "A 168-metre dreadnought is crossing the deep with her escorts. Break her twelve gold turrets to expose the hull. Change course when a turret flashes. She escapes after eight minutes.",
                18.,
                ink(),
            );
            action(
                &mut cmd,
                panel,
                Some(Action::Engage),
                "Intercept     [Enter]",
                "She sails as the yard left her. The crossing mends nothing more.",
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
            // Stars out of every patrol sailed, the one she ended on included.
            let sailed = run.stage.round().min(PATROLS) as u8;
            let tally = if run.stage.is_opening() {
                format!("{} / {MAX_STARS} stars", run.stars())
            } else {
                format!(
                    "{} / {} run stars   /   round {} of {ROUNDS}",
                    run.previous_stars + run.stars(),
                    sailed * MAX_STARS,
                    run.stage.round()
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
            if run.under_air_cover() {
                text(
                    &mut cmd,
                    panel,
                    "Enemy bombers and strafers fly from hangar A.",
                    15.,
                    brass(),
                );
            }
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
            text(&mut cmd, panel, run.stage.mission().name, 26., ink());
            text(&mut cmd, panel, pause_note(&run), 16., pale());
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
/// Where this leg stands in the run, and what the yard after it will offer.
fn pause_note(run: &Campaign) -> String {
    let round = run.stage.round();
    if run.stage == Stage::CrownDeep {
        return if run.under_air_cover() {
            format!(
                "Round {round} of {ROUNDS}. The air station here launches bombers as well as strafers."
            )
        } else {
            format!("Round {round} of {ROUNDS}. Sink the Iron Regent: there is no going home.")
        };
    }
    let air = if run.under_air_cover() {
        " Strafers fly from the air station here."
    } else {
        ""
    };
    let next = if run.stage.is_last_patrol() {
        "This yard sails her on to the Iron Regent at Crown Deep."
    } else {
        "Each yard charts three passages: open water, a sheltered sound for two stars and a hard passage for three, under enemy air cover. The harder the passage, the more each star pays."
    };
    let clock = "Sink every base to finish the patrol. Each base wrecked puts 45 seconds on the clock, and finishing inside it pays a star's worth of yard points.";
    format!("Round {round} of {ROUNDS}.{air} {clock} {next}")
}

/// The patrol's instruments. `icons` are the contact mark and the yard's spanner.
fn hud(
    cmd: &mut Commands,
    root: Entity,
    run: &Campaign,
    chart: Handle<Image>,
    (contact_icon, spanner): (Handle<Image>, Handle<Image>),
    deck: Option<Handle<Image>>,
    captain: &crate::captain::CaptainArt,
) {
    if run.screen() == Screen::Sailing {
        crate::mission_hud::spawn(cmd, root, run.stage);
        crate::slip_badges::spawn(cmd, root, spanner.clone());
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
                width: px(crate::minimap::SHOWN),
                height: px(crate::minimap::SHOWN),
                ..default()
            },
            ChildOf(panel),
        ));
        text(
            cmd,
            panel,
            "200 m grid     A B C   Enemy drydocks",
            12.,
            pale(),
        );
        let target = text(cmd, panel, "", 13., brass());
        cmd.entity(target).insert(Instrument::Target);
        let spotted = text(cmd, panel, "", 12., ink());
        cmd.entity(spotted).insert(Instrument::Spotting);
        if run.under_air_cover() {
            let raid = text(cmd, panel, "", 13., brass());
            cmd.entity(raid).insert(Instrument::AirRaid);
        }
        if !run.stage.is_opening() {
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
        let heading = if matches!(inst, Instrument::Engine) {
            let row = cmd
                .spawn((
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: px(10),
                        ..default()
                    },
                    ChildOf(panel),
                ))
                .id();
            crate::captain::portrait(cmd, row, captain);
            cmd.spawn((column(3.), ChildOf(row))).id()
        } else {
            panel
        };
        text(cmd, heading, title, 16., brass());
        let e = text(cmd, heading, "", 14., ink());
        if matches!(inst, Instrument::Navigation)
            && run.screen() == Screen::Sailing
            && run.stage != Stage::CrownDeep
        {
            text(
                cmd,
                panel,
                format!(
                    "Each star   {} yard points\nEach base   {} yard point, +{:.0} s\nInside the clock   {} yard points",
                    run.stage.star_points(),
                    shipvivor_campaign::YARD_POINTS,
                    shipvivor_campaign::BASE_TIME,
                    run.stage.star_points()
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
                if run.stage != Stage::CrownDeep {
                    dock_meter_gauge(cmd, panel, run, spanner.clone());
                }
            }
        }
        cmd.entity(e).insert(inst);
    }
}
#[derive(Component)]
pub struct Notch(i8);
#[derive(Component)]
pub struct XpBar;

/// Seconds of the flourish when cargo sunk makes a whole yard point.
const FLOURISH: f32 = 0.9;
/// The yard-point meter under the level bar. Cargo sunk ranks up into yard points on
/// its own: it shows the points banked this leg and how far it is to the next, and
/// flourishes as each one comes.
#[derive(Component)]
pub struct DockMeter {
    shown: u16,
    /// Seconds since the last point came, while its flourish plays.
    since: Option<f32>,
}
#[derive(Component)]
pub struct DockMeterBadge;
#[derive(Component)]
pub struct DockMeterCount;
#[derive(Component)]
pub struct DockMeterBar;

/// The spanner the yard prices its work with, the yard points cargo has made this leg,
/// and a bar filling toward the next.
fn dock_meter_gauge(cmd: &mut Commands, panel: Entity, run: &Campaign, spanner: Handle<Image>) {
    let points = run.cargo_points();
    let meter = cmd
        .spawn((
            DockMeter {
                shown: points,
                since: None,
            },
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                column_gap: px(7),
                margin: UiRect::top(px(2)),
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    let badge = cmd
        .spawn((
            DockMeterBadge,
            UiTransform::default(),
            Node {
                align_items: AlignItems::Center,
                column_gap: px(3),
                ..default()
            },
            ChildOf(meter),
        ))
        .id();
    cmd.spawn((
        ImageNode {
            image: spanner,
            color: brass(),
            ..default()
        },
        Node {
            width: px(13),
            height: px(13),
            ..default()
        },
        ChildOf(badge),
    ));
    let count = text(cmd, badge, points.to_string(), 13., brass());
    cmd.entity(count).insert(DockMeterCount);
    let track = cmd
        .spawn((
            Node {
                flex_grow: 1.,
                height: px(4),
                ..default()
            },
            BackgroundColor(trim()),
            ChildOf(meter),
        ))
        .id();
    cmd.spawn((
        DockMeterBar,
        Node {
            width: percent(100. * run.cargo_progress()),
            height: percent(100),
            ..default()
        },
        BackgroundColor(cyan()),
        ChildOf(track),
    ));
}

/// Fill the yard-point meter, and flourish when a whole point comes: the spanner and
/// its count spring up and flash, and the bar brims white before it drains to what is
/// left over toward the next.
pub fn dock_meter(
    time: Res<Time>,
    run: Res<Campaign>,
    mut meters: Query<&mut DockMeter>,
    mut badges: Query<&mut UiTransform, With<DockMeterBadge>>,
    mut counts: Query<(&mut Text, &mut TextColor), With<DockMeterCount>>,
    mut bars: Query<(&mut Node, &mut BackgroundColor), With<DockMeterBar>>,
) {
    let points = run.cargo_points();
    let mut since = None;
    for mut meter in &mut meters {
        if points > meter.shown {
            meter.since = Some(0.);
        }
        meter.shown = points;
        meter.since = meter
            .since
            .map(|t| t + time.delta_secs().min(0.05))
            .filter(|&t| t < FLOURISH);
        since = meter.since;
    }
    let flash = |base: Color, t: f32| base.mix(&Color::WHITE, (1. - t / 0.45).max(0.));
    for mut pose in &mut badges {
        let spring = since.map_or(0., |t| 0.6 * (-6. * t).exp() * (14. * t).sin());
        pose.scale = Vec2::splat(1. + spring);
    }
    for (mut label, mut color) in &mut counts {
        if label.0 != points.to_string() {
            label.0 = points.to_string();
        }
        color.0 = since.map_or(brass(), |t| flash(brass(), t));
    }
    let progress = run.cargo_progress();
    for (mut node, mut fill) in &mut bars {
        let (share, ink) = match since {
            Some(t) => {
                let drain = ((t - 0.45) / 0.3).clamp(0., 1.);
                let drain = drain * drain * (3. - 2. * drain);
                (1. + (progress - 1.) * drain, flash(cyan(), t))
            }
            None => (progress, cyan()),
        };
        node.width = percent(100. * share);
        fill.0 = ink;
    }
}
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
                if run.screen() == Screen::Trial {
                    let p = course.next_gate().center - helm.position;
                    let bearing = degrees(shipvivor_compass::heading(p));
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
                        "Raiders {} / {}     Stores  {} / {}\nDrydock destroyed  {} / 1\nShip hits {}     Sunk {}\n{}",
                        g.kills,
                        goal.kills,
                        g.crates,
                        goal.crates,
                        u8::from(g.destroyed_drydock),
                        run.hits,
                        run.sunk,
                        if run.late() {
                            format!(
                                "Past the clock  {}  : a base wrecked wins back {:.0} s",
                                clock_reading(&run),
                                shipvivor_campaign::BASE_TIME
                            )
                        } else {
                            format!(
                                "Clear it inside  {}  for {} more points",
                                clock_reading(&run),
                                run.stage.star_points()
                            )
                        }
                    )
                }
            }
            Instrument::Target => (view.quarry(&battle))
                .and_then(|quarry| quarry.frame(&battle))
                .map_or_else(String::new, |(at, _)| {
                    let selection = if view.locked() { "LOCK" } else { "AUTO" };
                    format!("{selection}   {:.0} m", helm.position.distance(at))
                }),
            Instrument::AirRaid => battle.air_station().map_or_else(String::new, |base| {
                let port = &battle.ports()[base.port];
                if let Some(plane) = base.aircraft.as_ref().filter(|plane| plane.inbound()) {
                    let offset = plane.body.pos - helm.position;
                    let raid = match plane.raid {
                        shipvivor_encounter::AirRaid::Strafe => "Strafing run",
                        shipvivor_encounter::AirRaid::Bomb => "Bomber inbound",
                    };
                    format!(
                        "{raid}   {:03} deg   {:.0} m",
                        degrees(shipvivor_compass::heading(offset)),
                        offset.length()
                    )
                } else if !base.operational(port) {
                    "Air station A / Silenced".into()
                } else {
                    "Air station A / Hangar active".into()
                }
            }),
            Instrument::Spotting => {
                let contact = view.sight(helm.position, &battle);
                if let Some(class) = contact.ship {
                    let class_name = crate::mission_hud::ship_class(class);
                    // Her weight before her class: a light cruiser, a heavy battleship.
                    let name = (contact.ship_id)
                        .and_then(|id| battle.ship(id))
                        .map_or_else(|| class_name.to_owned(), |r| r.weight().name(class_name));
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
                    if factory == 0
                        && battle
                            .air_station()
                            .is_some_and(|station| station.port == base)
                    {
                        format!(
                            "Seaplane hangar {}  /  {condition}",
                            crate::mission_hud::letter(base)
                        )
                    } else {
                        format!(
                            "Factory {}{}  /  {condition}\nBase output   {:.0} steel/min",
                            crate::mission_hud::letter(base),
                            factory + 1,
                            port.production_per_minute()
                        )
                    }
                } else if let Some((base, index)) = contact.shore_turret {
                    let gun = &battle.ports()[base].turrets()[index];
                    let id = crate::mission_hud::letter(base);
                    let kind = if gun.heavy_battery() {
                        "Heavy battery"
                    } else {
                        "Shore turret"
                    };
                    format!(
                        "{kind} {id}{}  /  {} {}\nMount strength   {} / {}",
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
                } else if let Some((index, slip)) = contact.yard {
                    let port = &battle.ports()[index];
                    let dock = &port.docks()[slip];
                    let base = crate::mission_hud::letter(index);
                    // A base with two slips numbers them.
                    let id = if port.docks().len() > 1 {
                        format!("{base}{}", slip + 1)
                    } else {
                        base.to_owned()
                    };
                    let factories = format!(
                        "Factories online   {} / {}",
                        port.live_factories(),
                        port.factories().len()
                    );
                    if port.wrecked() {
                        format!("Base {base} destroyed\nLaunch slip closed")
                    } else if dock.rebuilding().is_some() {
                        match port.rebuild_eta() {
                            Some(eta) => format!(
                                "Drydock {id}  /  Wrecked\nFactories rebuilding   {:.0} s\n{factories}",
                                eta.ceil()
                            ),
                            None => format!("Drydock {id}  /  Wrecked\nLaunch slip closed"),
                        }
                    } else {
                        let (class, cost) = dock.order();
                        let order = if dock.laid() >= cost {
                            "Funded"
                        } else {
                            "Building"
                        };
                        format!(
                            "Drydock {id}  /  {:?}\n{order}  {}\nSteel   {} laid / {cost} needed\n{factories}",
                            dock.state(),
                            crate::mission_hud::ship_class(class),
                            dock.laid(),
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
                use shipvivor_encounter::{Ammo, Impact};
                let report = match battle.salvo_report() {
                    Some(Impact::Bounced) => "RICOCHET",
                    Some(Impact::Embedded) => "ARMOR HELD",
                    Some(Impact::Penetrated) => "PENETRATION",
                    Some(Impact::PassedThrough) => "OPEN BREACH",
                    Some(Impact::Overpenetrated) => "OVERPENETRATION",
                    None => "",
                };
                // Her plate is holding HE that the AP aboard would open. The call sits on
                // a line of its own and stands until Q switches the crew to AP.
                if battle.ap_called_for() && run.fittings.armor_piercing() && guns.ammo == Ammo::He
                {
                    format!("{report}\nPRESS Q TO USE AP")
                } else {
                    report.into()
                }
            }
            Instrument::Flagship => {
                let boss = crate::mission_hud::ship_class(
                    battle
                        .boss()
                        .unwrap_or(shipvivor_encounter::EnemyClass::Parliament),
                )
                .to_uppercase();
                if let Some(ship) = battle.boss_ship() {
                    let n = ship.live_guns();
                    let range = helm.position.distance(ship.body.pos).round() as u32;
                    let name = crate::mission_hud::ship_class(ship.class).to_uppercase();
                    if !ship.class.is_flagship() {
                        let (flooded, fatal) = ship.vessel().flooding();
                        format!(
                            "{name}    {range} m\n{n} / {} turrets firing  /  {flooded} / {fatal} flooded\nSink her to clear the patrol",
                            ship.guns().len()
                        )
                    } else if n > 0 {
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
                            "{boss}    fitting out\nPuts to sea in {}:{:02}",
                            left / 60,
                            left % 60
                        )
                    } else {
                        format!("{boss}    fitting out\nPutting to sea")
                    }
                } else {
                    format!("{boss} SUNK")
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
                        "W/S  Engine   A/D  Rudder   C  Astern\nX  Pin / release   T  Gun / torpedo\nMouse  Look   RMB + drag  Trim lock\n{weapon}\n1-9  Turret   Z  Manual / Scope / Auto\nShift  Scope{ammo}   Tab  Chart   Esc  Pause"
                    )
                }
            }
        };
        if label.0 != value {
            label.0 = value;
        }
    }
}
