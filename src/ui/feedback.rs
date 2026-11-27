//! The playtest feedback sheet: four rows of scores from -3 to +3, clicked straight on, in
//! the yard after each round and on the report at the end of a run. A click saves at once;
//! there is no submit. See [`crate::feedback`] for what is saved.
use super::*;
use crate::feedback::{Feedback, Metric, Moment, SCORES};
use crate::options::GameOptions;
use crate::snapshot::{show_toast, timestamp};
use crate::telemetry::Logbook;
use std::time::SystemTime;

/// One score on the sheet. It lights when it is the score given.
#[derive(Component)]
pub(crate) struct Tile {
    metric: Metric,
    score: i8,
}
/// The word that shows once a score has been saved for the moment on screen.
#[derive(Component)]
pub(crate) struct Saved;

/// Width of a score tile, and of the column of names beside them.
const TILE: f32 = 27.;
const NAMES: f32 = 94.;

fn lit() -> Color {
    brass()
}
fn tile_fill(hover: bool) -> Color {
    if hover {
        Color::srgb_u8(48, 73, 76)
    } else {
        Color::srgb_u8(30, 46, 50)
    }
}
fn depth() -> Color {
    Color::srgb_u8(17, 31, 36)
}

/// Whether the yard and the report ask for scores: the option is on and there is a run
/// log for them to be saved beside.
pub(crate) fn asking(options: &GameOptions, log: Option<&Logbook>) -> bool {
    options.feedback && log.is_some_and(|log| log.code().is_some())
}

/// The sheet, built into `parent` under `heading`: a row a metric, a tile a score.
pub(crate) fn sheet(cmd: &mut Commands, parent: Entity, heading: &str) {
    // As wide as its rows, so "Saved" stands over the last tile in a wider panel too.
    let column = cmd
        .spawn((
            Node {
                align_self: AlignSelf::FlexStart,
                ..column(5.)
            },
            ChildOf(parent),
        ))
        .id();
    let top = cmd
        .spawn((
            Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                margin: UiRect::bottom(px(2)),
                ..default()
            },
            ChildOf(column),
        ))
        .id();
    text(cmd, top, heading, 13., brass());
    // Hidden rather than absent, so it takes its room from the start and nothing moves.
    let saved = text(cmd, top, "Saved", 12., cyan());
    cmd.entity(saved).insert((Saved, Visibility::Hidden));
    for metric in Metric::ALL {
        let row = cmd
            .spawn((
                Node {
                    align_items: AlignItems::Center,
                    column_gap: px(3),
                    ..default()
                },
                ChildOf(column),
            ))
            .id();
        let name = cmd
            .spawn((
                Node {
                    width: px(NAMES),
                    flex_shrink: 0.,
                    ..default()
                },
                ChildOf(row),
            ))
            .id();
        text(cmd, name, metric.name(), 14., ink());
        for score in SCORES {
            let tile = cmd
                .spawn((
                    Node {
                        width: px(TILE),
                        height: px(23),
                        flex_shrink: 0.,
                        // Neutral stands a little apart, so the middle reads at a glance.
                        margin: UiRect::horizontal(px(if score == 0 { 4. } else { 0. })),
                        border: UiRect::all(px(1)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(tile_fill(false)),
                    BorderColor::all(trim()),
                    Action::Rate(metric, score),
                    Tile { metric, score },
                    RelativeCursorPosition::default(),
                    ChildOf(row),
                ))
                .id();
            let words = if score > 0 {
                format!("+{score}")
            } else {
                score.to_string()
            };
            text(cmd, tile, words, 13., pale());
        }
    }
}

/// Light the score given for each metric, the tile under the pointer, and "Saved" once
/// anything has been.
pub fn light(
    run: Res<Campaign>,
    sheet: Res<Feedback>,
    log: Option<Res<Logbook>>,
    mut tiles: Query<(
        &Tile,
        &RelativeCursorPosition,
        &mut BackgroundColor,
        &mut BorderColor,
        &Children,
    )>,
    mut words: Query<&mut TextColor>,
    mut saved: Query<&mut Visibility, With<Saved>>,
) {
    let Some(moment) = Moment::of(&run) else {
        return;
    };
    let code = log.as_deref().and_then(Logbook::code);
    for (tile, cursor, mut fill, mut border, children) in &mut tiles {
        let given = sheet.score(code, moment, tile.metric) == Some(tile.score);
        let (paint, edge, ink) = if given {
            (lit(), lit(), depth())
        } else {
            (tile_fill(cursor.cursor_over()), trim(), pale())
        };
        fill.set_if_neq(BackgroundColor(paint));
        border.set_if_neq(BorderColor::all(edge));
        for &child in children {
            if let Ok(mut color) = words.get_mut(child) {
                color.set_if_neq(TextColor(ink));
            }
        }
    }
    let shown = if sheet.any(code, moment) {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut visibility in &mut saved {
        visibility.set_if_neq(shown);
    }
}

/// Give a score, or take it back, and save the run's sheet beside its log at once. The
/// first save of a run says where it went. Anything else goes on to the run.
pub fn rate(
    In(action): In<Option<Action>>,
    run: Res<Campaign>,
    log: Option<Res<Logbook>>,
    mut sheet: ResMut<Feedback>,
    mut commands: Commands,
    mut told: Local<Option<String>>,
) -> Option<Action> {
    let Some(Action::Rate(metric, score)) = action else {
        return action;
    };
    let log = log?;
    let code = log.code()?;
    let round = match Moment::of(&run)? {
        Moment::Round(round) => round,
        // The run's end is about the round it ended in, too.
        Moment::Run => run.stage.round(),
    };
    let (_, now) = timestamp(SystemTime::now());
    if !sheet.rate(code, &run, metric, score, log.captures_in(round), &now) {
        return None;
    }
    let Some(folder) = log.folder() else {
        return None;
    };
    match sheet.save(folder) {
        Ok(path) if told.as_deref() != Some(code) => {
            *told = Some(code.to_owned());
            let name = path
                .file_name()
                .map_or(String::new(), |n| n.to_string_lossy().into());
            show_toast(&mut commands, format!("Feedback saved   work/runs/{name}"));
        }
        Ok(_) => {}
        Err(err) => warn!("Couldn't save the feedback: {err}"),
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feedback::Moment;
    use bevy::ui::RelativeCursorPosition;

    /// The game with a run logbook kept in memory, at the title.
    fn game() -> App {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>()
            .add_plugins(crate::GamePlugin)
            .insert_resource(crate::state::ChartSeeds::fixed(4))
            .insert_resource(Logbook::in_memory())
            .add_plugins(crate::telemetry::TelemetryPlugin)
            .init_resource::<Time>()
            .init_resource::<bevy::input::mouse::AccumulatedMouseMotion>()
            .init_resource::<bevy::input::mouse::AccumulatedMouseScroll>()
            .init_resource::<UiScale>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_message::<AppExit>();
        let w = app.world_mut();
        w.spawn(Window::default());
        let mut clock = w.resource_mut::<Time<Fixed>>();
        let dt = clock.timestep();
        clock.advance_by(dt);
        w.run_schedule(Startup);
        w.run_schedule(Update);
        app
    }

    /// A left click on whatever carries `action`, as if the pointer were over it.
    fn click(w: &mut World, action: Action) {
        let mut buttons = w.query::<(&Action, &mut RelativeCursorPosition)>();
        for (a, mut cursor) in buttons.iter_mut(w) {
            cursor.cursor_over = *a == action;
        }
        w.resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        w.run_schedule(PreUpdate);
        w.resource_mut::<ButtonInput<MouseButton>>().reset_all();
        let mut buttons = w.query::<&mut RelativeCursorPosition>();
        for mut cursor in buttons.iter_mut(w) {
            cursor.cursor_over = false;
        }
        w.run_schedule(Update);
    }

    fn tile_fill(w: &mut World, metric: Metric, score: i8) -> Color {
        w.query::<(&Tile, &BackgroundColor)>()
            .iter(w)
            .find(|(t, _)| t.metric == metric && t.score == score)
            .map(|(_, fill)| fill.0)
            .unwrap()
    }

    #[test]
    fn the_yard_after_a_round_takes_scores_with_a_click_and_nothing_to_submit() {
        let mut app = game();
        let w = app.world_mut();
        w.resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        w.run_schedule(PreUpdate);
        w.resource_mut::<ButtonInput<KeyCode>>().reset_all();
        w.run_schedule(Update);
        // Sailing asks nothing.
        w.spawn((
            Action::Depart(shipvivor_campaign::Stage::IronNarrows),
            RelativeCursorPosition {
                cursor_over: true,
                ..default()
            },
        ));
        click(w, Action::Depart(shipvivor_campaign::Stage::IronNarrows));
        for _ in 0..60 {
            w.run_schedule(FixedUpdate);
        }
        w.run_schedule(Update);
        let code = w.resource::<Logbook>().code().unwrap().to_owned();
        assert_eq!(w.query::<&Tile>().iter(w).count(), 0);

        // Every base down: the yard opens on the round just sailed.
        {
            let mut run = w.resource_mut::<Campaign>();
            run.tick_sector(1., shipvivor_campaign::Objectives::default(), 0, true);
            while run.fittings_remaining() > 0 {
                let card = run.offers()[0];
                run.install(card);
            }
        }
        w.run_schedule(Update);
        assert!(matches!(
            w.resource::<Campaign>().screen(),
            Screen::Drydock(_)
        ));
        assert_eq!(w.query::<&Tile>().iter(w).count(), 4 * SCORES.len());
        assert!(w.query::<&Text>().iter(w).any(|t| t.0 == "RATE ROUND 1"));
        let saved = |w: &mut World| {
            *w.query_filtered::<&Visibility, With<Saved>>()
                .single(w)
                .unwrap()
                == Visibility::Inherited
        };
        assert!(!saved(w));

        // One click scores, lights the tile, and leaves her in the yard.
        click(w, Action::Rate(Metric::Challenge, 2));
        let score = |w: &World, metric| {
            w.resource::<Feedback>()
                .score(Some(&code), Moment::Round(1), metric)
        };
        assert_eq!(score(w, Metric::Challenge), Some(2));
        assert!(matches!(
            w.resource::<Campaign>().screen(),
            Screen::Drydock(_)
        ));
        assert_eq!(tile_fill(w, Metric::Challenge, 2), lit());
        assert_ne!(tile_fill(w, Metric::Challenge, 1), lit());
        assert!(saved(w));
        // Neutral is a score; another changes it; the same again takes it back.
        click(w, Action::Rate(Metric::Frustration, 0));
        assert_eq!(score(w, Metric::Frustration), Some(0));
        click(w, Action::Rate(Metric::Challenge, -3));
        assert_eq!(score(w, Metric::Challenge), Some(-3));
        assert_ne!(tile_fill(w, Metric::Challenge, 2), lit());
        click(w, Action::Rate(Metric::Challenge, -3));
        assert_eq!(score(w, Metric::Challenge), None);
        let snapshot = w.resource::<Feedback>().snapshot();
        assert!(snapshot.contains("frustration = 0"), "{snapshot}");
        assert!(!snapshot.contains("challenge ="), "{snapshot}");
    }

    #[test]
    fn scores_are_asked_only_with_the_option_on_and_a_run_log_to_keep_them_beside() {
        let on = GameOptions::STOCK;
        let off = GameOptions {
            feedback: false,
            ..on
        };
        assert!(!asking(&on, None));
        // A logbook before any run has no log to keep them beside.
        assert!(!asking(&on, Some(&Logbook::in_memory())));
        assert!(!asking(&off, None));
    }
}
