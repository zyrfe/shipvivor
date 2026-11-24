use super::*;
use shipvivor_campaign::{LegEnd, Stage};
use ui::{Instrument, instruments};

#[test]
fn fixed_step_navigation_reaches_the_next_gate_in_the_hud() {
    let mut app = App::new();
    let mut clock = Time::<Fixed>::from_hz(60.0);
    clock.advance_by(clock.timestep());
    app.insert_resource(clock)
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<Campaign>()
        .init_resource::<Encounter>()
        .init_resource::<Guns>()
        .init_resource::<ShipView>()
        .init_resource::<PlayerHelm>()
        .init_resource::<TrialCourse>()
        .add_systems(FixedUpdate, advance_ship)
        .add_systems(Update, instruments);

    let world = app.world_mut();
    world.resource_mut::<Campaign>().mode = Mode::Trial;
    let display = world.spawn((Instrument::Navigation, Text::default())).id();
    world.resource_mut::<PlayerHelm>().ring(4);
    for _ in 0..8 * 60 {
        world.run_schedule(FixedUpdate);
    }
    world.run_schedule(Update);
    assert_eq!(world.resource::<TrialCourse>().active(), 1);
    assert!(
        world
            .get::<Text>(display)
            .unwrap()
            .starts_with("Lap 1   Gate 2 / 5")
    );
}

#[test]
fn gates_and_their_bearings_only_appear_in_the_helm_trial() {
    let mut app = headless_game();
    let w = app.world_mut();
    let mut buoys = w.query_filtered::<&Visibility, With<scene::Buoy>>();
    let mut lamps = w.query_filtered::<&Visibility, With<scene::Signal>>();
    let mut text = w.query::<&Text>();
    press(w, KeyCode::KeyT);
    assert_eq!(
        buoys
            .iter(w)
            .filter(|v| **v == Visibility::Inherited)
            .count(),
        10
    );
    assert_eq!(
        lamps
            .iter(w)
            .filter(|v| **v == Visibility::Inherited)
            .count(),
        2
    );
    assert!(text.iter(w).any(|t| t.contains("Gate 1 / 5")));
    press(w, KeyCode::Escape);
    start_in_narrows(w);
    assert!(
        buoys
            .iter(w)
            .chain(lamps.iter(w))
            .all(|v| *v == Visibility::Hidden)
    );
    assert!(text.iter(w).all(|t| !t.to_lowercase().contains("gate")));

    // Sail through the trial's first gate while on patrol.
    let gate = *w.resource::<TrialCourse>().next_gate();
    let mut helm = w.resource_mut::<PlayerHelm>();
    helm.heading = gate.bearing;
    helm.position = gate.center - helm.forward() * 0.05;
    helm.speed = 10.;
    w.run_schedule(FixedUpdate);
    assert_eq!(w.resource::<TrialCourse>().active(), 0);
    assert_eq!(w.resource::<Campaign>().stars(), 0);
    assert!(!w.resource::<Campaign>().can_extract());
}

#[test]
fn the_passage_chart_and_enemy_air_cover_follow_the_exit_actually_sailed() {
    use shipvivor_campaign::{ANCHORAGES, EXITS, Objectives, Stage};
    let keys = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3];
    for ((anchorage, exit), key) in ANCHORAGES.iter().zip(EXITS.iter()).zip(keys) {
        let mut app = headless_game();
        let w = app.world_mut();
        press(w, KeyCode::Enter);
        assert_eq!(w.resource::<Campaign>().screen(), Screen::Departure);
        let mut pins = w.query_filtered::<(&ui::Action, &mut bevy::ui::RelativeCursorPosition), With<ui::passage_chart::ChartName>>();
        assert_eq!(pins.iter(w).count(), ANCHORAGES.len());
        for (action, mut cursor) in pins.iter_mut(w) {
            cursor.cursor_over = *action == ui::Action::Depart(anchorage.stage);
        }
        w.resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        w.run_schedule(PreUpdate);
        w.resource_mut::<ButtonInput<MouseButton>>().reset_all();
        w.run_schedule(Update);
        assert_eq!(w.resource::<Campaign>().screen(), Screen::Sailing);
        assert_eq!(w.resource::<Campaign>().stage, anchorage.stage);
        assert_eq!(w.resource::<Campaign>().seed, TEST_SEED);
        let chart = Passage::of(anchorage.stage).chart(TEST_SEED);
        assert_eq!(
            w.resource::<Encounter>().ports()[0].body.pos,
            chart.harbours[0].pos
        );
        assert!(w.resource::<Encounter>().flagship().is_none());
        assert!(w.resource::<Encounter>().air_station().is_none());
        press(w, KeyCode::Escape);
        let mut labels = w.query::<&Text>();
        for stage in [anchorage.stage, Stage::CrownDeep]
            .into_iter()
            .chain(EXITS.map(|route| route.stage))
        {
            assert!(labels.iter(w).any(|label| label.0 == stage.mission().name));
        }
        press(w, KeyCode::Escape);
        {
            let mut run = w.resource_mut::<Campaign>();
            run.objectives = Objectives {
                kills: anchorage.stage.mission().kills,
                crates: anchorage.stage.mission().crates,
                destroyed_drydock: true,
            };
            run.extract();
            let order = run.fittings.offers()[0];
            run.install(order);
            assert!(run.leave_dock());
        }
        w.run_schedule(Update);
        assert_eq!(w.resource::<Campaign>().screen(), Screen::Chart);
        press(w, key);
        let air = exit.stage == Stage::PilotsCut;
        assert_eq!(w.resource::<Campaign>().passage, Some(exit.stage));
        assert_eq!(w.resource::<Encounter>().air_station().is_some(), air);
        if w.resource::<Campaign>().screen() == Screen::Refit {
            press(w, KeyCode::Digit1);
        }
        w.resource_mut::<Campaign>()
            .tick(1000., Objectives::default(), 0);
        w.run_schedule(Update);
        press(w, KeyCode::Enter);
        assert_eq!(w.resource::<Campaign>().stage, Stage::CrownDeep);
        assert_eq!(w.resource::<Campaign>().origin, anchorage.stage);
        assert_eq!(w.resource::<Encounter>().air_station().is_some(), air);
        if air {
            // Check the finale's first real launch, including the shore's contact report.
            let mut sea = w.resource_mut::<Encounter>();
            let port = &sea.ports()[0];
            let orders = shipvivor_encounter::Orders {
                player: port.berth() + port.body.forward() * 120.,
                heading: 0.,
                speed: 0.,
                hull: shipvivor_encounter::Hull::CRUISER,
                aim: shipvivor_encounter::Aim::Hold,
                reload: 3.,
                shell: shipvivor_encounter::MainShell::STOCK,
            };
            let raid = (0..60 * 45).find_map(|_| {
                sea.step(1. / 60., &orders);
                sea.air_station()?.aircraft.as_ref().map(|plane| plane.raid)
            });
            assert_eq!(raid, Some(shipvivor_encounter::AirRaid::Bomb));
        }
        press(w, KeyCode::Escape);
        let mut plotted = w.query_filtered::<&Text, With<ui::passage_chart::ChartName>>();
        for port in &ANCHORAGES {
            assert_eq!(
                plotted
                    .iter(w)
                    .any(|name| name.0 == port.stage.mission().name),
                port.stage == anchorage.stage
            );
        }
        press(w, KeyCode::KeyM);
        start_in_narrows(w);
        assert_eq!(w.resource::<Campaign>().passage, None);
        assert!(w.resource::<Encounter>().air_station().is_none());
    }
}

#[test]
fn the_departure_chart_waits_for_a_choice_on_new_and_replayed_runs() {
    let mut app = headless_game();
    let w = app.world_mut();
    press(w, KeyCode::Enter);
    press(w, KeyCode::Enter);
    for _ in 0..60 {
        w.run_schedule(FixedUpdate);
    }
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Departure);
    assert_eq!(w.resource::<Campaign>().elapsed, 0.);
    assert_eq!(w.resource::<ChartSeeds>().peek(), TEST_SEED);
    press(w, KeyCode::Escape);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Title);
    press(w, KeyCode::Enter);
    press(w, KeyCode::Digit3);
    assert_eq!(w.resource::<Campaign>().stage, Stage::BlackcapReach);
    w.resource_mut::<Campaign>()
        .tick(1., Objectives::default(), 1000);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Lost);
    press(w, KeyCode::Enter);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Departure);
    press(w, KeyCode::Digit1);
    let run = w.resource::<Campaign>();
    assert_eq!(run.stage, Stage::LanternBay);
    assert_eq!(run.screen(), Screen::Sailing);
    assert_ne!(run.seed, TEST_SEED);
    assert!(run.ship.afloat());
}

fn start_in_narrows(world: &mut World) {
    press(world, KeyCode::Enter);
    assert_eq!(world.resource::<Campaign>().screen(), Screen::Departure);
    press(world, KeyCode::Digit2);
}

fn press(world: &mut World, key: KeyCode) {
    world.resource_mut::<ButtonInput<KeyCode>>().press(key);
    world.run_schedule(PreUpdate);
    let mut keys = world.resource_mut::<ButtonInput<KeyCode>>();
    keys.release(key);
    keys.clear();
    world.run_schedule(Update);
}

#[test]
fn the_options_screen_sets_whether_the_next_sector_floats_packages() {
    use shipvivor_encounter::Rewards;
    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    // A new player gets experience straight from what she sinks.
    assert_eq!(w.resource::<Encounter>().rewards(), Rewards::Direct);
    assert!(w.resource::<Encounter>().salvage().is_empty());

    press(w, KeyCode::Escape);
    press(w, KeyCode::KeyM);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Title);
    press(w, KeyCode::KeyO);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Options);
    assert!(
        w.query::<&ui::Action>()
            .iter(w)
            .any(|a| *a == ui::Action::Rewards(Rewards::Packages))
    );
    press(w, KeyCode::Digit2);
    assert_eq!(
        w.resource::<options::GameOptions>().rewards,
        Rewards::Packages
    );
    press(w, KeyCode::Escape);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Title);
    start_in_narrows(w);
    assert_eq!(w.resource::<Encounter>().rewards(), Rewards::Packages);
    assert!(!w.resource::<Encounter>().salvage().is_empty());
}

#[test]
fn open_charts_resize_with_the_window_and_stowing_restores_the_bridge_scale() {
    let mut app = headless_game();
    let w = app.world_mut();
    let mut windows = w.query::<&mut Window>();
    windows.single_mut(w).unwrap().resolution.set(1000., 650.);
    press(w, KeyCode::Enter);
    let compact = w.resource::<UiScale>().0;
    windows.single_mut(w).unwrap().resolution.set(1920., 1080.);
    w.run_schedule(Update);
    let full_hd = w.resource::<UiScale>().0;
    assert!(full_hd > compact * 1.5);
    windows.single_mut(w).unwrap().resolution.set(3840., 2160.);
    w.run_schedule(Update);
    assert_eq!(w.resource::<UiScale>().0, full_hd * 2.);

    press(w, KeyCode::Digit2);
    assert_eq!(w.resource::<UiScale>().0, 1.);
    press(w, KeyCode::Tab);
    let at_sea = w.resource::<UiScale>().0;
    assert!(at_sea > full_hd * 2.);
    windows.single_mut(w).unwrap().resolution.set(1920., 1080.);
    w.run_schedule(Update);
    assert_eq!(w.resource::<UiScale>().0, at_sea / 2.);
    windows.single_mut(w).unwrap().resolution.set(3440., 1080.);
    w.run_schedule(Update);
    assert_eq!(w.resource::<UiScale>().0, at_sea / 2.);
    w.resource_mut::<Time>()
        .advance_by(std::time::Duration::from_secs(1));
    press(w, KeyCode::Tab);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Sailing);
    assert_eq!(w.resource::<UiScale>().0, 1.);
}

#[test]
fn tab_unrolls_and_stows_the_chart_with_the_patrol_held_and_paper_audible() {
    use shipvivor_acoustics::Sound;
    use std::time::Duration;
    use ui::passage_chart::RollPart;

    let mut app = headless_game();
    let w = app.world_mut();
    let mut mixer = w.resource::<sound::Soundboard>().claim().unwrap();
    press(w, KeyCode::Tab);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Title);
    start_in_narrows(w);
    let elapsed = w.resource::<Campaign>().elapsed;
    w.resource_mut::<Time>()
        .advance_by(Duration::from_millis(40));
    press(w, KeyCode::Tab);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::PassageMap);
    assert!(
        w.query::<&ui::Action>()
            .iter(w)
            .all(|a| !matches!(a, ui::Action::Depart(_)))
    );
    let mut frames = [[0.; 2]; 1024];
    mixer.render(&mut frames);
    assert_eq!(mixer.making(Sound::UnrollChart).0, 1);
    let width = |w: &mut World| {
        let node = w
            .query::<(&RollPart, &Node)>()
            .iter(w)
            .find(|(part, _)| matches!(part, RollPart::Sheet))
            .unwrap()
            .1;
        let Val::Px(width) = node.width else {
            panic!("chart clip should be in pixels")
        };
        width
    };
    let first = width(w);
    w.run_schedule(Update);
    let opening = width(w);
    assert!(opening > first);
    press(w, KeyCode::Tab);
    let closing = width(w);
    assert!(closing < opening && closing > 0.);
    mixer.render(&mut frames);
    assert_eq!(mixer.making(Sound::UnrollChart).0, 0);
    assert_eq!(mixer.making(Sound::RollChart).0, 1);
    assert!(frames.iter().flatten().any(|s| s.abs() > 0.01));
    press(w, KeyCode::Tab);
    assert!(width(w) > closing);
    for _ in 0..10 {
        w.run_schedule(FixedUpdate);
        w.run_schedule(Update);
    }
    assert_eq!(w.resource::<Campaign>().chart_roll.unwrap().spread, 1.);
    assert_eq!(w.resource::<Campaign>().elapsed, elapsed);
    press(w, KeyCode::Escape);
    for _ in 0..6 {
        w.run_schedule(Update);
    }
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Sailing);
    assert_eq!(w.query::<&RollPart>().iter(w).count(), 0);
    w.run_schedule(FixedUpdate);
    assert!(w.resource::<Campaign>().elapsed > elapsed);
}

/// Step the drydock selection to a section of the ship with the bracket keys.
fn visit(w: &mut World, at: usize) {
    for _ in 0..16 {
        let now = w.resource::<Campaign>().dock_module;
        if now == at {
            return;
        }
        press(
            w,
            if now < at {
                KeyCode::BracketRight
            } else {
                KeyCode::BracketLeft
            },
        );
    }
    panic!("section {at} cannot be selected");
}

/// Step through the selected section's refits with the arrow keys until `job` is previewed.
fn choose(w: &mut World, job: shipvivor_campaign::DockJob) {
    for _ in 0..24 {
        if w.resource::<Campaign>().dock_job() == Some(job) {
            return;
        }
        press(w, KeyCode::ArrowRight);
    }
    panic!("{job:?} is not offered by the selected section");
}

/// Preview work from the hull panel or a section dropped from the yard's stock, as a
/// click on the panel or a drop on the ship would.
fn draft(w: &mut World, job: shipvivor_campaign::DockJob) {
    w.run_system_cached_with(apply_action, Some(ui::Action::Draft(job)))
        .unwrap();
    w.run_schedule(Update);
}

/// The next shell fitting offered on the selected turret bay.
fn shell_refit(w: &World) -> shipvivor_campaign::DockJob {
    use shipvivor_campaign::{Card, DockJob};
    w.resource::<Campaign>()
        .dock_jobs()
        .into_iter()
        .find(|job| {
            matches!(
                job,
                DockJob::Fitting(Card::ArmorPiercing | Card::Caps | Card::Velocity)
            )
        })
        .expect("a turret bay is selected")
}

/// A run seed whose charts the scripted runs below can sail. Charts depend on where the
/// opening patrol screen lies, so moving `PICKETS` can call for another sweep.
const TEST_SEED: u64 = 4;

/// The turn from one bearing to another, the short way, in radians.
fn turned(from: f32, to: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    (to - from + PI).rem_euclid(TAU) - PI
}

fn headless_game() -> App {
    let mut app = App::new();
    app.init_resource::<Assets<Image>>()
        .add_plugins(GamePlugin)
        // Every run in a test sails the same waters.
        .insert_resource(state::ChartSeeds::fixed(TEST_SEED))
        .init_resource::<Time>()
        .init_resource::<bevy::input::mouse::AccumulatedMouseMotion>()
        .init_resource::<UiScale>()
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>();
    let w = app.world_mut();
    w.spawn(Window::default());
    let mut clock = w.resource_mut::<Time<Fixed>>();
    let dt = clock.timestep();
    clock.advance_by(dt);
    w.run_schedule(Startup);
    w.run_schedule(Update);
    app
}

#[test]
fn captain_leaves_the_prow_for_the_bridge_and_reacts_to_the_run() {
    use captain::{Face, ProwCaptain};
    use shipvivor_encounter::Events;
    let mut app = headless_game();
    let w = app.world_mut();
    let mut figure = w.query_filtered::<(&ChildOf, &Visibility), With<ProwCaptain>>();
    let (parent, visible) = figure.single(w).unwrap();
    assert!(w.get::<scene::Ship>(parent.parent()).is_some());
    assert_eq!(*visible, Visibility::Inherited);
    let mut face = w.query_filtered::<&ImageNode, With<Face>>();
    assert_eq!(face.iter(w).count(), 0);
    start_in_narrows(w);
    assert_eq!(*figure.single(w).unwrap().1, Visibility::Hidden);
    let calm = face.single(w).unwrap().rect.unwrap();
    let size = calm.width();
    assert_eq!(calm.min, Vec2::ZERO);
    let sink = Events {
        sunk: 1,
        ..default()
    };
    w.resource_mut::<Campaign>().watch.observe(0., &sink);
    w.run_schedule(Update);
    assert_eq!(face.single(w).unwrap().rect.unwrap().min, Vec2::splat(size));
    w.resource_mut::<Campaign>().watch.observe(
        0.,
        &Events {
            damage: 1,
            ..default()
        },
    );
    w.run_schedule(Update);
    assert_eq!(face.single(w).unwrap().rect.unwrap().min, Vec2::X * size);
    let mut run = w.resource_mut::<Campaign>();
    run.ship.strike_hull(17., 12.);
    run.watch.observe(2., &Events::default());
    w.run_schedule(Update);
    assert_eq!(face.single(w).unwrap().rect.unwrap().min, Vec2::Y * size);
    press(w, KeyCode::Escape);
    assert_eq!(face.iter(w).count(), 0);
    press(w, KeyCode::KeyM);
    assert_eq!(*figure.single(w).unwrap().1, Visibility::Inherited);
    press(w, KeyCode::KeyT);
    assert_eq!(face.single(w).unwrap().rect, Some(calm));
    w.resource_mut::<Time>()
        .advance_by(std::time::Duration::from_millis(4640));
    w.run_schedule(Update);
    assert_eq!(face.single(w).unwrap().rect, None);
}

#[test]
fn reserve_power_reaches_the_helm_and_damage_control_until_the_engine_restarts() {
    use shipvivor_shipyard::Component;
    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    let mut run = w.resource_mut::<Campaign>();
    let engine = run.ship.fitted(Component::Engine).next().unwrap().0;
    let top_speed = run.ship.design().top_speed() * run.fittings.speed_factor();
    run.ship.strike_fitting(engine, 12.);
    let mut helm = w.resource_mut::<PlayerHelm>();
    helm.position = Vec2::splat(2000.);
    helm.ring(4);
    for _ in 0..3 * 60 {
        w.run_schedule(FixedUpdate);
    }
    assert!((w.resource::<PlayerHelm>().speed - 0.15 * top_speed).abs() < 0.001);
    w.run_schedule(Update);
    let mut reports = w.query_filtered::<&Text, With<damage_control::Summary>>();
    let report = reports.single(w).unwrap();
    assert!(report.contains("RESERVE POWER") && report.contains("15%") && report.contains("30%"));
    w.resource_mut::<PlayerHelm>().ring(-7);
    let astern = w.resource::<PlayerHelm>().ordered_speed() * top_speed
        / shipvivor_navigation::Helm::FULL_AHEAD;
    for _ in 0..3 * 60 {
        w.run_schedule(FixedUpdate);
    }
    assert!((w.resource::<PlayerHelm>().speed - 0.15 * astern).abs() < 0.001);
    w.resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyD);
    let initial_heading = w.resource::<PlayerHelm>().heading;
    for _ in 0..60 {
        w.run_schedule(FixedUpdate);
    }
    assert!(turned(initial_heading, w.resource::<PlayerHelm>().heading) < -0.001);
    for _ in 0..5 * 60 {
        w.run_schedule(FixedUpdate);
    }
    w.run_schedule(Update);
    assert!(!w.resource::<Campaign>().ship.on_reserve_power());
    assert!(reports.single(w).unwrap().contains("Engines 50%"));
}

#[test]
fn a_run_reaches_the_regent_through_the_helm_and_keeps_its_refits() {
    let mut app = headless_game();
    let w = app.world_mut();
    assert!(
        w.query::<&ui::Action>()
            .iter(w)
            .any(|a| *a == ui::Action::Start)
    );
    start_in_narrows(w);
    w.resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Space);
    w.run_schedule(PreUpdate);
    let mut sailed_with_partial_xp = false;
    // The packages drift and hostile crews can now reach the opening lane from farther out.
    // Steam through it at full ahead to collect the first field refit under fire.
    press(w, KeyCode::KeyW);
    press(w, KeyCode::KeyW);
    for _ in 0..3600 {
        w.run_schedule(FixedUpdate);
        let run = w.resource::<Campaign>();
        if run.objectives.crates == 1 {
            assert_eq!((run.screen(), run.experience), (Screen::Sailing, 10));
            sailed_with_partial_xp = true;
        }
        if run.screen() == Screen::Refit {
            break;
        }
    }
    w.run_schedule(Update);
    assert!(sailed_with_partial_xp);
    assert_eq!(
        w.resource::<Campaign>().screen(),
        Screen::Refit,
        "first refit at {:?}, {} stores, {} hits, {} sunk",
        w.resource::<PlayerHelm>().position,
        w.resource::<Campaign>().objectives.crates,
        w.resource::<Campaign>().hits,
        w.resource::<Campaign>().sunk
    );
    assert_eq!(w.resource::<Campaign>().level, 2);
    assert!(w.resource::<Campaign>().objectives.kills >= 1);
    assert!(w.resource::<Campaign>().hits > 0);
    assert_eq!(
        w.resource::<Campaign>().sunk,
        u32::from(w.resource::<Campaign>().objectives.kills)
    );
    assert_eq!(
        w.query::<&ui::Action>()
            .iter(w)
            .filter(|a| matches!(a, ui::Action::Card(_)))
            .count(),
        3
    );
    let card_labels: Vec<_> = w
        .query::<&Text>()
        .iter(w)
        .map(|text| text.0.as_str())
        .collect();
    for name in ["Armor-Piercing", "Larger Bore", "Shell Velocity"] {
        assert!(card_labels.iter().any(|text| text.contains(name)));
    }
    let pos = w.resource::<PlayerHelm>().position;
    let time = w.resource::<Campaign>().elapsed;
    w.run_schedule(FixedUpdate);
    assert_eq!(w.resource::<PlayerHelm>().position, pos);
    assert_eq!(w.resource::<Campaign>().elapsed, time);
    press(w, KeyCode::Digit2);
    assert_eq!(w.resource::<Campaign>().fittings.shells, 1);
    // Take the drydock exit on the first leg and the timed exit on the second.
    for leg in 0..2 {
        if leg == 0 {
            destroy_drydock(&mut w.resource_mut::<Encounter>(), 0);
        }
        for _ in 0..3700 {
            w.run_schedule(FixedUpdate);
            if w.resource::<Campaign>().screen() == Screen::Refit {
                press(w, KeyCode::Digit1);
            }
            if w.resource::<Campaign>().can_extract() {
                break;
            }
        }
        if leg == 0 {
            assert!(w.resource::<Campaign>().objectives.destroyed_drydock);
            assert!(w.resource::<Campaign>().elapsed < shipvivor_campaign::EXTRACT_SECONDS);
        }
        // E at the helm does nothing. Extraction waits in the pause menu.
        press(w, KeyCode::KeyE);
        assert_eq!(w.resource::<Campaign>().screen(), Screen::Sailing);
        press(w, KeyCode::Escape);
        assert_eq!(w.resource::<Campaign>().screen(), Screen::Paused);
        press(w, KeyCode::KeyE);
        if leg == 0 {
            assert_eq!(
                w.resource::<Campaign>().screen(),
                Screen::Drydock(LegEnd::Extracted)
            );
            visit(w, 1);
            let shells = shell_refit(w);
            choose(w, shells);
            // The only order is fitted at once, and the next Enter, with nothing
            // previewed, sails.
            press(w, KeyCode::Enter);
            assert_eq!(w.resource::<Campaign>().fittings_remaining(), 0);
            assert_eq!(
                w.resource::<Campaign>().screen(),
                Screen::Drydock(LegEnd::Extracted)
            );
            press(w, KeyCode::Enter);
            assert_eq!(w.resource::<Campaign>().screen(), Screen::Chart);
            let tally = (w.resource::<Campaign>().hits, w.resource::<Campaign>().sunk);
            press(w, KeyCode::Digit1);
            let run = w.resource::<Campaign>();
            assert_eq!(run.stage, shipvivor_campaign::Stage::OuterRoad);
            assert_eq!(run.elapsed, 0.);
            assert_eq!((run.hits, run.sunk), tally);
            assert_eq!(run.fittings.shells, 1);
            assert_eq!(w.resource::<Encounter>().shots().len(), 0);
            assert!(
                w.query::<(&Instrument, &Text)>()
                    .iter(w)
                    .any(|(gauge, text)| matches!(gauge, Instrument::Flagship)
                        && text.contains("fitting out\nPuts to sea in 4:30"))
            );
        }
    }
    assert_eq!(
        w.resource::<Campaign>().screen(),
        Screen::Intercept(LegEnd::Extracted)
    );
    press(w, KeyCode::Enter);
    let run = w.resource::<Campaign>();
    assert_eq!(run.stage, Stage::CrownDeep);
    assert_eq!(run.fittings.shells, 1);
    assert_eq!(run.screen(), Screen::Sailing);
    assert_eq!(
        w.resource::<Encounter>()
            .raiders()
            .iter()
            .filter(|r| r.class == shipvivor_encounter::EnemyClass::Regent)
            .count(),
        1
    );
    w.resource_mut::<Campaign>()
        .tick_sector(61., Objectives::default(), 0, true);
    // There is no extracting from Crown Deep, even from the pause menu.
    press(w, KeyCode::Escape);
    press(w, KeyCode::KeyE);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Paused);
    press(w, KeyCode::Escape);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Sailing);
    w.resource_mut::<Campaign>().tick_engagement(
        0.1,
        Objectives::default(),
        0,
        Some(LegEnd::RegentSunk),
    );
    assert_eq!(
        w.resource::<Campaign>().screen(),
        Screen::Won(LegEnd::RegentSunk)
    );
    press(w, KeyCode::Escape);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Title);
    start_in_narrows(w);
    assert_eq!(w.resource::<Campaign>().fittings.shells, 0);
    assert_eq!(
        (w.resource::<Campaign>().hits, w.resource::<Campaign>().sunk),
        (0, 0)
    );
}

/// Fire on the selected yard from its open-water approach.
fn destroy_drydock(battle: &mut Encounter, index: usize) {
    use shipvivor_encounter::YardState;
    let yard = battle.ports()[index].yard_position();
    let approach = battle.ports()[index].body.forward();
    for _ in 0..600 {
        if battle.ports()[index].yard_state() == YardState::Ruined {
            break;
        }
        let mark = yard
            .extend((3.2 + battle.ports()[index].yard_top()) * 0.5)
            .xzy();
        battle.step(
            1. / 30.,
            &Orders {
                player: yard + 90. * approach,
                heading: shipvivor_compass::heading(-approach),
                hull: Hull::CRUISER,
                speed: 0.,
                aim: Aim::Sight(mark),
                shell: MainShell::STOCK,
                reload: 0.5,
            },
        );
    }
    assert_eq!(battle.ports()[index].yard_state(), YardState::Ruined);
}

#[test]
fn destroying_the_yards_or_timing_out_brings_the_ship_through_drydock() {
    for reason in [LegEnd::BasesDestroyed, LegEnd::TimeExpired] {
        let mut app = headless_game();
        let w = app.world_mut();
        start_in_narrows(w);
        if reason == LegEnd::BasesDestroyed {
            // Fire on each yard from the fairway. Between bases, let the game read the
            // resulting port states and update its mission HUD.
            for i in 0..w.resource::<Encounter>().ports().len() {
                destroy_drydock(&mut w.resource_mut::<Encounter>(), i);
                assert_eq!(w.resource::<Encounter>().ruined_yards(), i + 1);
                w.run_schedule(FixedUpdate);
                w.run_schedule(Update);
                if i < 2 {
                    assert_eq!(w.resource::<Campaign>().screen(), Screen::Sailing);
                    assert!(w.resource::<Campaign>().objectives.destroyed_drydock);
                    assert!(w.resource::<Campaign>().can_extract());
                    let count = format!("{} / 3", i + 1);
                    assert!(
                        w.query::<(&mission_hud::Reading, &Text)>()
                            .iter(w)
                            .any(|(r, t)| matches!(r, mission_hud::Reading::Bases) && t.0 == count)
                    );
                }
            }
        } else {
            w.resource_mut::<Campaign>().elapsed = 599.999;
            w.run_schedule(FixedUpdate);
            w.run_schedule(Update);
        }
        let picks = if reason == LegEnd::BasesDestroyed {
            2
        } else {
            1
        };
        if reason == LegEnd::BasesDestroyed {
            assert_eq!(w.resource::<Campaign>().screen(), Screen::Secured);
            assert!(
                w.query::<&Text>()
                    .iter(w)
                    .any(|t| t.contains("SECTOR SECURED"))
            );
            let clock = w.resource::<Campaign>().elapsed;
            for _ in 0..135 {
                w.run_schedule(FixedUpdate);
            }
            w.run_schedule(Update);
            assert_eq!(clock, w.resource::<Campaign>().elapsed);
        }
        let run = w.resource::<Campaign>();
        assert_eq!(run.screen(), Screen::Drydock(reason));
        assert_eq!(run.fittings_remaining(), picks);
        let elapsed = run.elapsed;
        w.run_schedule(FixedUpdate);
        assert_eq!(w.resource::<Campaign>().elapsed, elapsed);
        assert!(w.query::<&Text>().iter(w).any(|t| t.contains("DRYDOCK")));
        visit(w, 1);
        for _ in 0..picks {
            let shells = shell_refit(w);
            choose(w, shells);
            press(w, KeyCode::Enter);
        }
        // Every order is fitted, and she waits in dock until she is sailed.
        assert_eq!(w.resource::<Campaign>().fittings_remaining(), 0);
        assert_eq!(w.resource::<Campaign>().screen(), Screen::Drydock(reason));
        press(w, KeyCode::Enter);
        assert_eq!(w.resource::<Campaign>().screen(), Screen::Chart);
        press(w, KeyCode::Digit3);
        assert_eq!(w.resource::<Campaign>().screen(), Screen::Chart);
        press(w, KeyCode::Digit1);
        let run = w.resource::<Campaign>();
        assert_eq!(
            (run.screen(), run.stage, run.elapsed),
            (Screen::Sailing, Stage::OuterRoad, 0.)
        );
        assert_eq!(w.resource::<Encounter>().ruined_yards(), 0);
        assert!(run.fittings.armor_piercing());
    }
}

#[test]
fn helm_mouse_look_drives_render_and_fire_then_releases_to_menus() {
    use bevy::{
        input::mouse::AccumulatedMouseMotion,
        window::{CursorGrabMode, CursorOptions},
    };
    let mut app = headless_game();
    let w = app.world_mut();
    let camera = w
        .query_filtered::<Entity, With<camera::FollowCamera>>()
        .single(w)
        .unwrap();
    let window = w
        .query_filtered::<Entity, With<Window>>()
        .single(w)
        .unwrap();
    assert!(matches!(
        w.get::<Projection>(camera),
        Some(Projection::Perspective(_))
    ));
    for _ in 0..12 {
        w.run_schedule(FixedUpdate);
    }
    w.run_schedule(Update);
    let helm = w.resource::<PlayerHelm>();
    let eye = w.get::<Transform>(camera).unwrap().translation;
    assert!(helm.position.length() > 1.);
    assert!((eye.xz() - helm.position).dot(helm.forward()) > Hull::CRUISER.bow);
    let clearance = eye.y - w.resource::<seascape::Sea>().height(eye.xz());
    assert!((0.4..1.8).contains(&clearance));
    assert_eq!(w.resource::<Campaign>().elapsed, 0.);
    start_in_narrows(w);
    assert_eq!(w.resource::<PlayerHelm>().position, Vec2::ZERO);
    assert_eq!(
        w.get::<CursorOptions>(window).unwrap().grab_mode,
        CursorGrabMode::Locked
    );
    // Discard the backend warp that can arrive immediately after locking the mouse.
    w.resource_mut::<AccumulatedMouseMotion>().delta = Vec2::splat(640.);
    w.run_schedule(PreUpdate);
    // A patrol sets out on whatever heading the run's chart gives the first leg.
    let start = w.resource::<PlayerHelm>().heading;
    assert!(turned(start, w.resource::<ShipView>().bearing).abs() < 1e-5);
    w.resource_mut::<Time>()
        .advance_by(std::time::Duration::from_millis(200));
    w.resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ShiftRight);
    w.resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    w.resource_mut::<AccumulatedMouseMotion>().delta = Vec2::new(130., 20.);
    w.run_schedule(PreUpdate);
    w.run_schedule(FixedUpdate);
    w.run_schedule(Update);
    let ray = w.get::<Transform>(camera).unwrap().forward();
    let heading = ray.x.atan2(-ray.z);
    assert!(turned(start, heading) > 0.);
    use shipvivor_encounter::{Battery, Station};
    assert!(
        w.resource::<Encounter>()
            .shots()
            .iter()
            .all(|s| s.battery != Battery::Main)
    );
    for _ in 0..120 {
        w.run_schedule(FixedUpdate);
        if w.resource::<Encounter>()
            .shots()
            .iter()
            .any(|s| s.battery == Battery::Main)
        {
            break;
        }
    }
    let battle = w.resource::<Encounter>();
    let helm = w.resource::<PlayerHelm>();
    let mount = battle.main_mount(Station::Bow);
    let target = w.resource::<ShipView>().sight(helm.position, battle).point;
    let round = battle
        .shots()
        .iter()
        .find(|s| s.battery == Battery::Main)
        .unwrap();
    assert!(mount.aim_mark(helm.position, helm.heading).distance(target) < 0.02);
    let trained = shipvivor_compass::direction(helm.heading + mount.deck_bearing());
    assert!(round.tangent().xz().normalize().abs_diff_eq(trained, 1e-4));
    let Some(Projection::Perspective(lens)) = w.get::<Projection>(camera) else {
        panic!("expected perspective")
    };
    assert!(lens.fov < 25_f32.to_radians());

    w.resource_mut::<AccumulatedMouseMotion>().delta = Vec2::ZERO;
    w.resource_mut::<ButtonInput<KeyCode>>()
        .release(KeyCode::ShiftRight);
    w.resource_mut::<PlayerHelm>().heading = 1.7;
    w.run_schedule(PreUpdate);
    w.run_schedule(Update);
    assert!(turned(heading, w.resource::<ShipView>().bearing).abs() < 1e-5);
    let Some(Projection::Perspective(lens)) = w.get::<Projection>(camera) else {
        panic!("expected perspective")
    };
    assert!(lens.fov > 50_f32.to_radians());
    press(w, KeyCode::KeyC);
    assert!((w.resource::<ShipView>().bearing - 1.7).abs() < 1e-5);

    press(w, KeyCode::Escape);
    assert!(w.get::<CursorOptions>(window).unwrap().visible);
    assert_eq!(
        w.get::<CursorOptions>(window).unwrap().grab_mode,
        CursorGrabMode::None
    );
    press(w, KeyCode::Enter);
    assert!(!w.resource::<Guns>().firing);
    w.get_mut::<Window>(window).unwrap().focused = false;
    w.resource_mut::<AccumulatedMouseMotion>().delta = Vec2::splat(50.);
    w.run_schedule(PreUpdate);
    w.run_schedule(Update);
    assert!((w.resource::<ShipView>().bearing - 1.7).abs() < 1e-5);
    assert_eq!(
        w.get::<CursorOptions>(window).unwrap().grab_mode,
        CursorGrabMode::None
    );
}

#[test]
fn chart_and_telegraph_follow_the_playable_sector() {
    use shipvivor_campaign::EXITS;
    use shipvivor_encounter::SPAWN_STANDOFF;
    for (stage, seed) in shipvivor_campaign::ANCHORAGES
        .map(|port| port.stage)
        .into_iter()
        .chain(EXITS.map(|exit| exit.stage))
        .flat_map(|stage| [TEST_SEED, 2, 3].map(|seed| (stage, seed)))
    {
        let passage = Passage::of(stage);
        let chart = passage.chart(seed);
        let battle = passage.battle(&chart);
        // Past the first leg the flagship is fitting out, not yet at sea.
        assert!(
            battle
                .raiders()
                .iter()
                .all(|ship| ship.class != shipvivor_encounter::EnemyClass::Parliament)
        );
        assert_eq!(
            battle.flagship(),
            (!stage.is_opening()).then_some(shipvivor_encounter::Sortie::Due(
                shipvivor_encounter::FLAGSHIP_SORTIE
            ))
        );
        assert!(chart.gates.iter().map(|p| p.length()).fold(0., f32::max) > 390.);
        assert!(
            battle
                .raiders()
                .iter()
                .all(|r| r.body.pos.length() >= SPAWN_STANDOFF)
        );
        for port in battle.ports() {
            assert_eq!(battle.navigable_position(port.berth(), 3.), port.berth());
            for point in chart.gates {
                assert_eq!(battle.navigable_position(point, 15.), point);
            }
        }
    }
    let finale = Passage::of(Stage::CrownDeep);
    for seed in [1, 2, 3, 83] {
        let chart = finale.chart(seed);
        let battle = finale.battle(&chart);
        let boss = battle
            .raiders()
            .iter()
            .find(|r| r.class == shipvivor_encounter::EnemyClass::Regent)
            .unwrap();
        let hull = boss.class.profile().hull;
        assert!(boss.body.pos.length() >= SPAWN_STANDOFF);
        assert!(
            battle
                .navigable_position(boss.body.pos, hull.bow)
                .distance(boss.body.pos)
                < 0.1
        );
        assert_eq!(boss.live_guns(), 12);
        assert!(battle.flagship().is_none());
    }
    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    assert_eq!(w.resource::<PlayerHelm>().order_label(), "HALF AHEAD");
    for _ in 0..5 {
        press(w, KeyCode::KeyS);
    }
    assert_eq!(w.resource::<PlayerHelm>().order_label(), "FULL ASTERN");
    w.resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyW);
    w.run_schedule(PreUpdate);
    w.resource_mut::<ButtonInput<KeyCode>>().clear();
    w.run_schedule(PreUpdate);
    assert_eq!(w.resource::<PlayerHelm>().order_label(), "HALF ASTERN");
    let chart = w.resource::<minimap::ChartImage>().0.clone();
    assert!(
        w.query::<&ImageNode>()
            .iter(w)
            .any(|node| node.image == chart)
    );
    let pixels = |w: &World| {
        w.resource::<Assets<Image>>()
            .get(&chart)
            .unwrap()
            .data
            .clone()
            .unwrap()
    };
    let before = pixels(w);
    w.resource_mut::<PlayerHelm>().position.x += 100.;
    w.resource_mut::<Campaign>().elapsed += 1.;
    w.run_schedule(Update);
    assert!(pixels(w) != before);
    press(w, KeyCode::Escape);
    assert!(
        !w.query::<&ImageNode>()
            .iter(w)
            .any(|node| node.image == chart)
    );
}

#[test]
fn gun_rings_go_grey_and_arcs_drop_while_the_mark_is_beyond_reach_or_the_guns_train() {
    use shipvivor_compass::direction;
    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    let (at, heading) = {
        let helm = w.resource::<PlayerHelm>();
        (helm.position, helm.heading)
    };
    // A beam with open water out past the guns' reach, clear of the coast.
    let open = |w: &World, bearing: f32, range: f32| {
        let mark = at + direction(bearing) * range;
        let mut view = ShipView::default();
        view.center_on(at, mark.extend(0.).xzy());
        let sight = view.sight(at, w.resource::<Encounter>());
        sight.point.y < 0.01 && sight.point.xz().distance(mark) < 30.
    };
    let bearing = [70., -70., 90., -90., 110., -110., 50., -50.]
        .map(|degrees: f32| heading + degrees.to_radians())
        .into_iter()
        .find(|&bearing| open(w, bearing, 1300.) && open(w, bearing, 350.))
        .expect("open water on a beam");
    // The mounts that would draw a shell arc onto the aim point.
    let arcs = |w: &World| {
        let (view, battle, guns) = (
            w.resource::<ShipView>(),
            w.resource::<Encounter>(),
            w.resource::<Guns>(),
        );
        let aim = match view.main_aim(at, battle, guns.firing, guns.nearest) {
            Aim::Sight(p) | Aim::Track(p) => p,
            _ => panic!("the sight lays the guns"),
        };
        let round = w.resource::<Campaign>().fittings.round();
        trajectory::laid(battle, w.resource::<PlayerHelm>(), aim, round).len()
    };
    let lay = |w: &mut World, bearing: f32, range: f32, ticks: usize| {
        let mark = at + direction(bearing) * range;
        w.resource_mut::<ShipView>()
            .center_on(at, mark.extend(0.).xzy());
        for _ in 0..ticks {
            w.run_schedule(FixedUpdate);
        }
        w.run_schedule(Update);
        assert!(
            w.resource::<ShipView>()
                .quarry(w.resource::<Encounter>())
                .is_none()
        );
        let note = w
            .query_filtered::<&Node, With<gun_sight::RangeNote>>()
            .single(w)
            .unwrap()
            .display;
        let grey = w
            .query_filtered::<&TextColor, With<gun_sight::GunLabel>>()
            .iter(w)
            .filter(|tint| tint.0 == gun_sight::out_of_range_ink())
            .count();
        (note, grey, arcs(w))
    };
    // Long enough for the mounts to train round onto the bearing.
    let (note, grey, drawn) = lay(w, bearing, 1300., 240);
    assert_eq!(note, Display::Flex);
    assert!(grey > 0);
    assert_eq!(drawn, 0);
    let (note, grey, drawn) = lay(w, bearing, 350., 240);
    assert_eq!(note, Display::None);
    assert_eq!(grey, 0);
    assert!(drawn > 0);
    // Swung across to the other beam, every mount is still training round.
    let (_, _, drawn) = lay(w, bearing + std::f32::consts::PI, 350., 3);
    assert_eq!(drawn, 0);
}

#[test]
fn gun_circles_track_distinct_mounts_and_leave_with_the_helm() {
    use shipvivor_encounter::Station;
    let mut app = headless_game();
    let w = app.world_mut();
    let circles = |w: &mut World| {
        w.query_filtered::<Entity, With<gun_sight::GunCircle>>()
            .iter(w)
            .count()
    };
    assert_eq!(circles(w), 0);
    start_in_narrows(w);
    w.resource_mut::<ShipView>().bearing = 70_f32.to_radians();
    for _ in 0..155 {
        w.run_schedule(FixedUpdate);
    }
    w.run_schedule(Update);
    assert_eq!(circles(w), 2);
    let positions: Vec<_> = w
        .query_filtered::<&Node, With<gun_sight::GunCircle>>()
        .iter(w)
        .map(|n| (n.left, n.top))
        .collect();
    assert_ne!(positions[0], positions[1]);
    let view = w.resource::<ShipView>();
    let player = w.resource::<PlayerHelm>();
    let lens_size = Vec2::new(1280., 760.);
    let marks = Station::ALL.map(|s| {
        let pt = w
            .resource::<Encounter>()
            .main_mount(s)
            .aim_mark(player.position, player.heading);
        view.project_mark(player.position, pt, lens_size)
    });
    assert_ne!(marks[0].0, marks[1].0);
    press(w, KeyCode::Escape);
    assert_eq!(circles(w), 0);
}

#[test]
fn a_base_with_word_of_the_ship_sounds_its_siren_until_the_voyage_ends() {
    use shipvivor_acoustics::{Mixer, SAMPLE_RATE};
    fn listen(mixer: &mut Mixer, secs: f32) {
        mixer.render(&mut vec![[0.; 2]; (secs * SAMPLE_RATE as f32) as usize]);
    }
    let mut app = headless_game();
    let w = app.world_mut();
    let mut mixer = w.resource::<sound::Soundboard>().claim().unwrap();
    start_in_narrows(w);
    // Lying 150 m off the quay of the first base, the ship is in plain sight of it.
    let port = &w.resource::<Encounter>().ports()[0];
    let berth = port.body.pos + port.body.forward() * 150.;
    let mut alerted = false;
    for _ in 0..300 {
        w.resource_mut::<PlayerHelm>().position = berth;
        w.run_schedule(FixedUpdate);
        listen(&mut mixer, 1. / 60.);
        alerted = w.resource::<Encounter>().ports()[0].has_contact;
        if alerted {
            break;
        }
    }
    assert!(alerted);
    let bases = w.resource::<Encounter>().ports().iter();
    let alarms = bases.filter(|port| port.has_contact).count();
    listen(&mut mixer, 1. / 60.);
    assert!(alarms >= 1);
    assert_eq!(mixer.wailing(), alarms);
    // At the title nobody works the switch. Each rotor runs down and stops.
    press(w, KeyCode::Escape);
    press(w, KeyCode::KeyM);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Title);
    listen(&mut mixer, 50.);
    assert_eq!(mixer.wailing(), 0);
}

#[test]
fn a_spotter_is_heard_by_a_ship_under_its_circuit_and_not_from_far_out_to_sea() {
    use shipvivor_acoustics::{Mixer, SAMPLE_RATE};
    /// Sail on for some ticks with the ship held where she is put.
    fn sail(w: &mut World, mixer: &mut Mixer, ticks: usize, at: impl Fn(&World) -> Vec2) {
        for _ in 0..ticks {
            let at = at(w);
            w.resource_mut::<PlayerHelm>().position = at;
            w.run_schedule(FixedUpdate);
            mixer.render(&mut vec![[0.; 2]; (SAMPLE_RATE / 60) as usize]);
        }
    }
    let mut app = headless_game();
    let w = app.world_mut();
    let mut mixer = w.resource::<sound::Soundboard>().claim().unwrap();
    start_in_narrows(w);
    // Six kilometres out no aeroplane is in earshot.
    sail(w, &mut mixer, 120, |_| Vec2::splat(-6000.));
    assert_eq!(mixer.droning(), 0);
    // Under the first base's spotter its engine is heard, and perhaps the others'.
    let under = |w: &World| w.resource::<Encounter>().spotters()[0].position;
    sail(w, &mut mixer, 120, under);
    assert!(mixer.droning() >= 1);
    // At the title nothing flies.
    press(w, KeyCode::Escape);
    press(w, KeyCode::KeyM);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Title);
    mixer.render(&mut vec![[0.; 2]; 10 * SAMPLE_RATE as usize]);
    assert_eq!(mixer.droning(), 0);
}

#[test]
fn a_mainland_answers_from_its_open_shore_and_no_stretch_of_it_twice() {
    use sound::{Disc, open_shore, spaced};
    let disc = |x: f32, y: f32, radius: f32| Disc {
        at: Vec2::new(x, y),
        radius,
        height: 5.,
    };
    // Hills run together along a shore, with a mountain behind them and an islet off it.
    let hills = (0..=10).map(|n| disc(60. * n as f32, 0., 100.));
    let (mountain, islet) = (disc(300., 300., 350.), disc(300., -160., 20.));
    let discs: Vec<Disc> = hills.chain([mountain, islet]).collect();
    let ship = Vec2::new(300., -260.);
    let shore = open_shore(ship, &discs);
    // The mountain's nearest point is under the hills, and so are those of the hills
    // away to either side, which lean in towards the ship.
    assert!(!shore.iter().any(|(disc, _)| *disc == mountain));
    assert!(shore.contains(&(islet, Vec2::new(300., -180.))));
    assert!(shore.contains(&(discs[5], Vec2::new(300., -100.))));
    assert!(shore.len() < discs.len() - 3, "{}", shore.len());
    for (_, face) in &shore {
        let inside = |other: &Disc| face.distance(other.at) < other.radius - 1.5;
        assert!(!discs.iter().any(inside), "{face}");
    }
    // The nearest answers first, and its neighbours along the same shore do not.
    let nearest = |(_, face): &(Disc, Vec2)| -face.distance(ship);
    let kept = spaced(shore.clone(), nearest, 60., 8);
    assert_eq!(kept[0].0, islet);
    assert!(
        kept.len() >= 2 && kept.len() < shore.len(),
        "{}",
        kept.len()
    );
    for (n, (_, face)) in kept.iter().enumerate() {
        assert!(kept[..n].iter().all(|taken| taken.1.distance(*face) >= 60.));
    }
    // Taken further apart, this little bay is one stretch, and never more than asked.
    assert_eq!(spaced(shore.clone(), nearest, 120., 8).len(), 1);
    assert_eq!(spaced(shore, nearest, 0., 3).len(), 3);
    // Open sea has no shore at all, and one island is its own.
    assert!(open_shore(ship, &[]).is_empty());
    assert_eq!(open_shore(ship, &[islet]).len(), 1);
}

#[test]
fn an_alerted_base_sends_up_flares_that_come_down_slowly_under_canopies() {
    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    let models = |w: &mut World| w.query::<&flares::FlareModel>().iter(w).count();
    // In plain sight of the first base, 150 m off its quay, the ship is soon reported
    // and three flares go up from its yard. Word goes round by radio, so the other
    // bases may send up their own.
    let port = &w.resource::<Encounter>().ports()[0];
    let (yard, top) = (port.yard_position(), port.yard_top());
    let berth = port.body.pos + port.body.forward() * 150.;
    let all = |w: &World| w.resource::<flares::Flares>().live().len();
    let here = |w: &World| -> Vec<Vec3> {
        let live = w.resource::<flares::Flares>().live().iter();
        let near = live.filter(|flare| flare.position.xz().distance(yard) < 200.);
        near.map(|flare| flare.position).collect()
    };
    let sail = |w: &mut World, ticks: usize, at: Vec2| {
        for _ in 0..ticks {
            w.resource_mut::<PlayerHelm>().position = at;
            w.run_schedule(FixedUpdate);
        }
    };
    assert_eq!(all(w), 0);
    for _ in 0..300 {
        sail(w, 1, berth);
        if here(w).len() == 3 {
            break;
        }
    }
    assert_eq!(here(w).len(), 3);
    // Three seconds on they have burst high over the water between base and ship.
    sail(w, 180, berth);
    let live = w.resource::<flares::Flares>().live();
    assert!(live.iter().all(|flare| flare.glow() > 0.99));
    let burst = here(w);
    assert!(burst.iter().all(|at| at.y > top + 45.), "{burst:?}");
    let out: Vec<f32> = burst.iter().map(|at| at.xz().distance(yard)).collect();
    assert!(out.iter().all(|out| (40. ..170.).contains(out)), "{out:?}");
    assert!(burst[0].distance(burst[2]) > 20.);
    // They sink under their canopies at a few metres a second, and each is drawn.
    sail(w, 120, berth);
    let fell: Vec<f32> = burst.iter().zip(here(w)).map(|(a, b)| a.y - b.y).collect();
    assert!(fell.iter().all(|fall| (5. ..9.).contains(fall)), "{fell:?}");
    w.run_schedule(Update);
    assert_eq!(models(w), all(w));
    assert_eq!(w.query::<&PointLight>().iter(w).count(), all(w));
    // Far out to sea the alarm comes down and no more go up. Those in the air burn out.
    sail(w, 60 * 34, Vec2::splat(-6000.));
    assert_eq!(all(w), 0);
    w.run_schedule(Update);
    assert_eq!(models(w), 0);
    assert_eq!(w.query::<&PointLight>().iter(w).count(), 0);
}

#[test]
fn the_klaxon_sounds_three_times_when_one_more_module_would_sink_her() {
    use shipvivor_acoustics::{SAMPLE_RATE, Sound};
    let mut app = headless_game();
    let w = app.world_mut();
    let mut mixer = w.resource::<sound::Soundboard>().claim().unwrap();
    start_in_narrows(w);
    // Far out to sea, so that the only damage is what is done here. Returns how many
    // blasts began.
    let mut sail = |w: &mut World, ticks: usize| {
        let (mut blasts, mut sounding) = (0, false);
        for _ in 0..ticks {
            w.resource_mut::<PlayerHelm>().position = Vec2::splat(-6000.);
            w.run_schedule(FixedUpdate);
            mixer.render(&mut vec![[0.; 2]; (SAMPLE_RATE / 60) as usize]);
            let now = mixer.making(Sound::Klaxon).0 > 0;
            blasts += usize::from(now && !sounding);
            sounding = now;
        }
        blasts
    };
    assert_eq!(sail(w, 120), 0);
    // Shells into the forward half of her, a little at a time, until she is critical.
    for hit in 0..400 {
        let ship = &mut w.resource_mut::<Campaign>().ship;
        if ship.in_peril() {
            break;
        }
        ship.strike_hull(2.5 + 5. * (hit % 3) as f32, 4.);
    }
    let ship = &w.resource::<Campaign>().ship;
    assert!(ship.in_peril() && ship.afloat());
    // Three blasts, a couple of seconds apart, and then it holds its peace a while.
    assert_eq!(sail(w, 60 * 8), 3);
    assert!(w.resource::<Campaign>().ship.in_peril());
    assert_eq!(sail(w, 60 * 8), 0);
}

#[test]
fn choosing_a_fitting_sounds_two_notes_while_the_battle_is_still_held() {
    use shipvivor_acoustics::{SAMPLE_RATE, Sound};
    let mut app = headless_game();
    let w = app.world_mut();
    let mut mixer = w.resource::<sound::Soundboard>().claim().unwrap();
    start_in_narrows(w);
    // Two levels earned at once: the first choice leaves the second still to make.
    w.resource_mut::<Campaign>().experience = 70;
    for _ in 0..3 {
        w.run_schedule(FixedUpdate);
    }
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Refit);
    // The battle and all its sound are held for the choice.
    w.run_schedule(Update);
    let mut heard = vec![[0_f32; 2]; (SAMPLE_RATE / 2) as usize];
    mixer.render(&mut heard);
    mixer.render(&mut heard);
    assert_eq!(mixer.making(Sound::Upgrade).0, 0);
    assert!(heard.iter().flatten().all(|x| *x == 0.));
    press(w, KeyCode::Digit1);
    mixer.render(&mut heard);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Refit);
    assert_eq!(mixer.making(Sound::Upgrade).0, 1);
    assert!(heard.iter().flatten().any(|x| x.abs() > 0.05));
}

#[test]
fn the_bridge_hears_the_telegraph_answer_and_her_engines_stop() {
    use shipvivor_acoustics::{Mixer, SAMPLE_RATE, Sound};
    use shipvivor_shipyard::Component;
    /// Sail on for some ticks and return the level of what was heard.
    fn sail(
        w: &mut World,
        mixer: &mut Mixer,
        ticks: usize,
        mut each: impl FnMut(&mut World),
    ) -> f32 {
        let mut heard = vec![[0_f32; 2]; ticks * (SAMPLE_RATE / 60) as usize];
        for tick in heard.chunks_mut((SAMPLE_RATE / 60) as usize) {
            each(w);
            w.run_schedule(FixedUpdate);
            mixer.render(tick);
        }
        let power: f32 = heard.iter().flatten().map(|x| x * x).sum();
        (power / (heard.len() * 2) as f32).sqrt()
    }
    let mut app = headless_game();
    let w = app.world_mut();
    let mut mixer = w.resource::<sound::Soundboard>().claim().unwrap();
    start_in_narrows(w);
    // She is kept far out to sea, where nobody will find her and start shooting.
    let alone = |w: &mut World| w.resource_mut::<PlayerHelm>().position = Vec2::splat(-6000.);
    // The order the voyage begins with rings no bell. A new one does, at once.
    sail(w, &mut mixer, 60, alone);
    assert_eq!(mixer.making(Sound::Telegraph).0, 0);
    w.resource_mut::<PlayerHelm>().ring(1);
    sail(w, &mut mixer, 1, alone);
    assert_eq!(mixer.making(Sound::Telegraph).0, 1);
    let steaming = sail(w, &mut mixer, 180, alone);
    // With every engine knocked out and kept out, the machinery is heard to stop.
    let wreck = |w: &mut World| {
        alone(w);
        let ship = &mut w.resource_mut::<Campaign>().ship;
        let engines: Vec<_> = ship.fitted(Component::Engine).collect();
        for (berth, _) in engines {
            ship.strike_fitting(berth, 1e3);
        }
    };
    sail(w, &mut mixer, 240, wreck);
    let out = |(_, state)| state == shipvivor_shipyard::State::Out;
    assert!(
        w.resource::<Campaign>()
            .ship
            .fitted(Component::Engine)
            .all(out)
    );
    let adrift = sail(w, &mut mixer, 120, wreck);
    let guns = [Sound::MainGun, Sound::RaiderGun, Sound::ShoreGun];
    assert!(guns.iter().all(|gun| mixer.making(*gun).0 == 0));
    assert!(adrift < 0.4 * steaming, "{adrift} against {steaming}");
}

#[test]
fn the_bridge_sees_a_gun_flash_before_it_hears_the_report() {
    use shipvivor_acoustics::{Mixer, SAMPLE_RATE, Sound};
    /// Gun reports in the air, and how many of them have reached the bridge. The
    /// surf and the birds are in the air all the while and are not counted.
    fn reports(mixer: &Mixer) -> (usize, usize) {
        let guns = [
            Sound::MainGun,
            Sound::SecondaryGun,
            Sound::RaiderGun,
            Sound::ShoreGun,
        ];
        let made = guns.map(|gun| mixer.making(gun));
        made.iter()
            .fold((0, 0), |sum, n| (sum.0 + n.0, sum.1 + n.1))
    }
    /// Let sound run on and return its level.
    fn listen(mixer: &mut Mixer, secs: f32) -> f32 {
        let mut heard = vec![[0_f32; 2]; (secs * SAMPLE_RATE as f32) as usize];
        mixer.render(&mut heard);
        let power: f32 = heard.iter().flatten().map(|x| x * x).sum();
        (power / (heard.len() * 2) as f32).sqrt()
    }
    let mut app = headless_game();
    let w = app.world_mut();
    let mut mixer = w.resource::<sound::Soundboard>().claim().unwrap();
    assert!(w.resource::<sound::Soundboard>().claim().is_none());
    // At the title only the sea is heard.
    let moored = listen(&mut mixer, 3.);
    assert!(moored > 0.005);
    start_in_narrows(w);
    w.resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Space);
    w.run_schedule(PreUpdate);
    // Sound keeps step with the battle. On the tick of the first flash its report is
    // in the air and has yet to reach the bridge.
    let (mut flashed, mut sailing) = (false, 0_f32);
    for _ in 0..1200 {
        w.run_schedule(FixedUpdate);
        sailing = sailing.max(listen(&mut mixer, 1. / 60.));
        let fresh = |fx: &shipvivor_encounter::Effect| fx.age == 0.;
        if w.resource::<Encounter>().effects().iter().any(fresh) {
            flashed = true;
            break;
        }
        assert_eq!(reports(&mixer), (0, 0));
    }
    assert!(flashed);
    assert!(reports(&mixer).0 >= 1);
    assert_eq!(reports(&mixer).1, 0);
    // The round itself is followed through the air from its second tick.
    assert_eq!(mixer.flying(), 0);
    w.run_schedule(FixedUpdate);
    listen(&mut mixer, 1. / 60.);
    assert!(mixer.flying() >= 1);
    // At the longer engagement ranges the sound front can take several seconds.
    for _ in 0..240 {
        listen(&mut mixer, 1. / 60.);
        if reports(&mixer).1 > 0 {
            break;
        }
    }
    assert!(reports(&mixer).1 >= 1);
    // Engines at half ahead and the wash along the hull are well above the idle sea.
    assert!(sailing > moored * 2., "{sailing} against {moored}");

    // A pause holds the roll of the gun in the air, and resuming lets it finish.
    press(w, KeyCode::Escape);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Paused);
    listen(&mut mixer, 0.05);
    let held = mixer.sounding();
    assert!(held >= 1);
    assert_eq!(listen(&mut mixer, 5.), 0.);
    assert_eq!(mixer.sounding(), held);
    press(w, KeyCode::Escape);
    assert!(listen(&mut mixer, 0.2) > moored);

    // Leaving for the title clears whatever was on its way and stops the engines.
    press(w, KeyCode::Escape);
    press(w, KeyCode::KeyM);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Title);
    // The last echo off an island can take ten seconds to come back and die away.
    listen(&mut mixer, 12.);
    assert_eq!((mixer.sounding(), mixer.flying()), (0, 0));
    let ashore = listen(&mut mixer, 3.);
    assert!(ashore > 0.005 && ashore < sailing * 0.6);
}

#[test]
fn each_chart_has_clear_gates_and_sailable_supply_routes_to_all_three_yards() {
    use shipvivor_encounter::{Battle, EnemyClass, YARD_CAPACITY};
    for stage in [
        Stage::IronNarrows,
        Stage::OuterRoad,
        Stage::GreywaterSound,
        Stage::PilotsCut,
    ] {
        for seed in [TEST_SEED, 2] {
            let chart = Passage::of(stage).chart(seed);
            let gates = chart.gates;
            let mut b = Battle::for_chart(9999., &chart);
            assert!(b.landmarks().len() >= 4, "{stage:?}");
            for gate in gates {
                assert_eq!(b.navigable_position(gate, 15.), gate);
            }
            let observer = Orders {
                player: Vec2::splat(-3000.),
                heading: 0.,
                speed: 0.,
                hull: Hull::CRUISER,
                aim: Aim::Hold,
                reload: 6.4,
                shell: shipvivor_encounter::MainShell::STOCK,
            };
            let mut sailed = false;
            for _ in 0..1700 {
                b.step(0.1, &observer);
                sailed |= b.raiders().iter().any(|r| r.class == EnemyClass::Freighter);
                if b.ports().iter().all(|p| p.stock == YARD_CAPACITY) {
                    break;
                }
            }
            assert!(
                sailed && b.ports().iter().all(|p| p.stock == YARD_CAPACITY),
                "{stage:?} seed {seed} {:?}",
                b.ports().each_ref().map(|p| p.stock)
            );
        }
    }
}

/// Each run charts its waters afresh, and every chart the game sails has passed the full
/// survey: every gate, slip, yard and factory within reach, bases spread apart.
#[test]
fn every_leg_of_many_runs_sails_waters_that_pass_their_survey() {
    use shipvivor_encounter::Survey;
    for stage in [
        Stage::IronNarrows,
        Stage::OuterRoad,
        Stage::GreywaterSound,
        Stage::PilotsCut,
    ] {
        let passage = Passage::of(stage);
        let mut circuits = vec![];
        let mut turns = [0; 2];
        for seed in 100..124 {
            let chart = passage.chart(seed);
            assert!(
                chart.seed.is_some(),
                "{stage:?} run {seed} fell back to the hand-set chart"
            );
            let survey = Survey::of(&chart);
            assert!(
                survey.passed(),
                "{stage:?} run {seed}: {}",
                survey
                    .defects
                    .iter()
                    .map(|d| d.to_string())
                    .collect::<Vec<_>>()
                    .join("; ")
            );
            assert_eq!(passage.chart(seed), chart, "a run seed replays its waters");
            let g = chart.gates;
            turns[usize::from((g[1] - g[0]).perp_dot(g[2] - g[1]) > 0.)] += 1;
            circuits.push(g);
        }
        // No two runs share a circuit, and circuits turn both ways.
        for (i, a) in circuits.iter().enumerate() {
            for b in &circuits[i + 1..] {
                assert!(
                    a.iter().zip(b).any(|(p, q)| p.distance(*q) > 50.),
                    "{stage:?}"
                );
            }
        }
        assert!(turns.iter().all(|&n| n >= 4), "{stage:?} turns {turns:?}");
    }
    // The two legs of one run are charted separately.
    let first = Passage::of(Stage::IronNarrows).chart(TEST_SEED);
    let second = Passage::of(Stage::OuterRoad).chart(TEST_SEED);
    assert_ne!(first.gates, second.gates);
    let mut seeds = state::ChartSeeds::fixed(TEST_SEED);
    assert_eq!(seeds.draw(), TEST_SEED);
    assert_ne!(seeds.draw(), TEST_SEED);
}

/// The survey is no harder on the generator than on the charts the game shipped with:
/// the hand-set Iron Narrows and Outer Road pass it.
#[test]
fn hand_set_charts_pass_the_survey_that_generated_ones_must() {
    use shipvivor_encounter::Survey;
    for stage in [Stage::IronNarrows, Stage::OuterRoad] {
        let survey = Survey::of(&Passage::of(stage).classic());
        assert!(survey.passed(), "{stage:?}: {:?}", survey.defects);
        assert!(survey.score > 0.6, "{stage:?}: {}", survey.score);
    }
    assert_eq!(
        shipvivor_encounter::GATE_HALF_WIDTH,
        shipvivor_navigation::GATE_HALF_WIDTH
    );
}

#[test]
fn x_pins_the_guns_while_the_camera_can_look_astern() {
    use bevy::input::mouse::AccumulatedMouseMotion;
    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    let player = w.resource::<PlayerHelm>().position;
    let cruiser = &w.resource::<Encounter>().raiders()[1];
    let id = cruiser.body.id;
    let center = cruiser
        .body
        .pos
        .extend(cruiser.class.profile().deck * 0.65)
        .xzy();
    w.resource_mut::<ShipView>().center_on(player, center);
    w.run_schedule(PreUpdate);
    let view = w.resource::<ShipView>();
    assert!(!view.locked());
    assert_eq!(view.target(w.resource::<Encounter>()).unwrap().body.id, id);

    press(w, KeyCode::KeyX);
    let local = |mark: Vec3, r: &shipvivor_encounter::Raider| {
        let d = mark.xz() - r.body.pos;
        let f = r.body.forward();
        Vec3::new(d.dot(f.perp()), mark.y, d.dot(f))
    };
    let point = |w: &World| {
        let b = w.resource::<Encounter>();
        let v = w.resource::<ShipView>();
        let r = v.target(b).unwrap();
        assert_eq!(r.body.id, id);
        local(v.tracked_point(b).unwrap(), r)
    };
    let original = point(w);
    w.resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Right);
    w.resource_mut::<AccumulatedMouseMotion>().delta = Vec2::new(12., -2.);
    w.run_schedule(PreUpdate);
    let trimmed = point(w);
    assert!(trimmed.distance(original) > 1.);
    w.resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Right);
    w.resource_mut::<AccumulatedMouseMotion>().delta = Vec2::X * (std::f32::consts::PI / 0.003);
    w.run_schedule(PreUpdate);
    w.resource_mut::<AccumulatedMouseMotion>().delta = Vec2::ZERO;
    let astern = w.resource::<ShipView>().bearing;
    for _ in 0..300 {
        w.run_schedule(FixedUpdate);
    }
    w.run_schedule(Update);
    let b = w.resource::<Encounter>();
    let view = w.resource::<ShipView>();
    let helm = w.resource::<PlayerHelm>();
    assert!(point(w).abs_diff_eq(trimmed, 1e-4));
    assert_eq!(view.bearing, astern);
    let mark = view.tracked_point(b).unwrap();
    assert!(
        view.viewport_point(helm.position, mark, Vec2::splat(600.))
            .is_none()
    );
    let bow = b.main_mount(shipvivor_encounter::Station::Bow);
    assert!(bow.aim_mark(helm.position, helm.heading).distance(mark) < 0.4);
    press(w, KeyCode::KeyX);
    let view = w.resource::<ShipView>();
    assert!(!view.locked());
    assert!(view.target(w.resource::<Encounter>()).is_none());
}

#[test]
fn soft_lock_acquires_a_near_miss_at_both_magnifications_and_releases_on_open_water() {
    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    let player = w.resource::<PlayerHelm>().position;
    let b = w.resource::<Encounter>();
    let ship = &b.raiders()[1];
    assert_ne!(b.nearest_ship(player).unwrap().body.id, ship.body.id);
    let center = ship.body.pos.extend(ship.class.profile().deck * 0.65).xzy();
    for zoom in [0., 1.] {
        let mut view = ShipView::default();
        view.zoom = zoom;
        view.center_on(player, center);
        // Miss her to whichever side has open water behind her. Pickets start at one of
        // several stations, and a miss onto land or ice behind her looks for a battery.
        let near_miss = if zoom == 0. { 0.058 } else { 0.021 };
        let on = view.bearing;
        view.bearing = on + near_miss;
        if view.sight(player, b).point.y > 0.01 {
            view.bearing = on - near_miss;
        }
        assert!(
            view.sight(player, b).point.y <= 0.01,
            "open water behind her"
        );
        assert!(view.sight(player, b).ship_id.is_none());
        view.update_contact(player, b);
        assert_eq!(view.target(b).map(|r| r.body.id), Some(ship.body.id));
        assert!(
            matches!(view.main_aim(player, b, false, false), Aim::Track(p) if p == ship.aim_point(player))
        );
        view.bearing += std::f32::consts::PI;
        view.update_contact(player, b);
        assert!(view.target(b).is_none());
    }
}

#[test]
fn the_scope_drops_the_snap_and_rides_with_the_contact_off_the_reticle() {
    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    let size = Vec2::splat(600.);
    let mark =
        |r: &shipvivor_encounter::Raider| r.body.pos.extend(r.class.profile().deck * 0.65).xzy();
    let player = w.resource::<PlayerHelm>().position;
    let ship = &w.resource::<Encounter>().raiders()[1];
    let (id, center) = (ship.body.id, mark(ship));
    let mut view = w.resource_mut::<ShipView>();
    view.zoom = 1.;
    view.center_on(player, center);
    // Off her seaward side: on the other, the reticle comes to rest on the islet behind
    // her, and from the shore the free sight looks only for harbour guns.
    view.bearing -= 0.021;
    w.resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ShiftLeft);
    w.run_schedule(PreUpdate);
    let offset = |w: &World| {
        let b = w.resource::<Encounter>();
        let view = w.resource::<ShipView>();
        let r = view.target(b).expect("the scope keeps her");
        assert_eq!(r.body.id, id);
        let at = w.resource::<PlayerHelm>().position;
        view.viewport_point(at, mark(r), size).unwrap() - size * 0.5
    };
    let before = offset(w);
    assert!(before.length() > 20.);

    let b = w.resource::<Encounter>();
    let view = w.resource::<ShipView>();
    let ship = b.ship(id).unwrap();
    assert!(view.sight(player, b).ship_id.is_none());
    let Aim::Track(p) = view.main_aim(player, b, false, false) else {
        panic!("the scope tracks the reticle");
    };
    assert_ne!(p, ship.aim_point(player));
    // On the reticle, out at her range.
    assert!(
        view.viewport_point(player, p, size)
            .unwrap()
            .distance(size * 0.5)
            < 0.5
    );
    assert!((p.xz().distance(player) - ship.body.pos.distance(player)).abs() < 3.);
    // Space still lays the guns on her.
    assert!(
        matches!(view.main_aim(player, b, false, true), Aim::Sight(q) if q == ship.aim_point(player))
    );

    let (start, bearing) = (ship.body.pos, view.bearing);
    for _ in 0..200 {
        w.run_schedule(FixedUpdate);
    }
    let after = offset(w);
    assert!(after.distance(before) < 1.5, "{before} drifted to {after}");
    let ship = w.resource::<Encounter>().ship(id).unwrap();
    assert!(ship.body.pos.distance(start) > 5.);
    assert_ne!(w.resource::<ShipView>().bearing, bearing);

    // Out of the scope the free sight snaps onto her again.
    w.resource_mut::<ButtonInput<KeyCode>>()
        .release(KeyCode::ShiftLeft);
    w.run_schedule(PreUpdate);
    let b = w.resource::<Encounter>();
    let view = w.resource::<ShipView>();
    let at = w.resource::<PlayerHelm>().position;
    let ship = view.target(b).unwrap();
    assert!(matches!(view.main_aim(at, b, false, false), Aim::Track(p) if p == ship.aim_point(at)));
}

#[test]
fn torpedo_mode_click_launches_once_and_bore_cards_resize_the_deck_models() {
    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    let scales = |w: &mut World| {
        w.query_filtered::<(&Transform, &Visibility), With<scene::Turret>>()
            .iter(w)
            .filter(|(_, shown)| **shown != Visibility::Hidden)
            .map(|(t, _)| t.scale.x)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        scales(w),
        vec![shipvivor_encounter::MainShell::STOCK.mount_scale(); 2]
    );
    w.resource_mut::<Campaign>().tick(
        0.,
        Objectives {
            crates: 3,
            ..default()
        },
        0,
    );
    press(w, KeyCode::Digit2);
    let fitted = w.resource::<Campaign>().fittings.round();
    assert_eq!(fitted.calibre_inches(), 6.);
    assert_eq!(scales(w), vec![fitted.mount_scale(); 2]);
    press(w, KeyCode::KeyT);
    let abeam = w.resource::<PlayerHelm>().heading + std::f32::consts::FRAC_PI_2;
    w.resource_mut::<ShipView>().bearing = abeam;
    w.resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    w.run_schedule(PreUpdate);
    for _ in 0..12 {
        w.run_schedule(FixedUpdate);
    }
    w.run_schedule(Update);
    assert_eq!(w.resource::<Encounter>().torpedoes().len(), 3);
    assert!(
        w.query::<&Text>()
            .iter(w)
            .any(|t| t.contains("Starboard tubes"))
    );
    assert!(
        w.query::<&Text>()
            .iter(w)
            .any(|t| t.contains("Ship hits") && t.contains("Sunk"))
    );
    press(w, KeyCode::KeyT);
    assert!(!w.resource::<Guns>().torpedo_mode);
}

#[test]
fn choosing_turret_cards_refits_deck_models_sight_pips_and_battery_board() {
    use shipvivor_encounter::Station;
    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    for (stores, between, key, tubes) in [
        (3, 0, KeyCode::Digit3, 2),
        (4, 0, KeyCode::Digit2, 3),
        // Three more fittings bring the main battery's hand round again.
        (5, 3, KeyCode::Digit2, 4),
    ] {
        for _ in 0..between {
            let fit = &mut w.resource_mut::<Campaign>().fittings;
            *fit = fit.with(shipvivor_campaign::Card::Turbines);
        }
        w.resource_mut::<Campaign>().tick(
            0.,
            Objectives {
                crates: stores,
                ..default()
            },
            0,
        );
        w.run_schedule(Update);
        press(w, key);
        assert_eq!(w.resource::<Campaign>().screen(), Screen::Sailing);
        assert_eq!(
            w.resource::<Encounter>()
                .main_mount(Station::Bow)
                .layout()
                .count(),
            tubes
        );
        assert_eq!(
            w.query_filtered::<&Visibility, With<scene::GunTubes>>()
                .iter(w)
                .filter(|&&v| v == Visibility::Inherited)
                .count(),
            2 * tubes
        );
        assert_eq!(w.query::<&gun_sight::GunPip>().iter(w).count(), 2 * tubes);
        assert_eq!(w.query::<&gunnery::PlanBarrel>().iter(w).count(), 2 * tubes);
        let summary = format!(" / {} ON TARGET", 2 * tubes);
        assert!(
            w.query::<&Text>()
                .iter(w)
                .any(|text| text.ends_with(&summary))
        );
    }
}

#[test]
fn click_fires_one_barrel_double_click_the_rest_and_holding_ripples() {
    use shipvivor_encounter::{Battery, EffectKind, MountStatus, RIPPLE, Station};
    use std::time::Duration;
    /// Main battery barrels that fired on the last fixed step.
    fn fired(w: &World) -> usize {
        w.resource::<Encounter>()
            .effects()
            .iter()
            .filter(|fx| fx.kind == EffectKind::Muzzle(Battery::Main) && fx.age == 0.)
            .count()
    }
    fn on_target(w: &World) -> usize {
        let battle = w.resource::<Encounter>();
        Station::ALL
            .iter()
            .map(|&s| battle.main_mount(s))
            .filter(|m| m.status() == MountStatus::Ready)
            .map(|m| (0..m.layout().count()).filter(|&t| m.loaded(t)).count())
            .sum()
    }
    fn trigger(w: &mut World, down: bool) {
        let mut mouse = w.resource_mut::<ButtonInput<MouseButton>>();
        if down {
            mouse.press(MouseButton::Left);
        } else {
            mouse.release(MouseButton::Left);
        }
        w.run_schedule(PreUpdate);
        w.resource_mut::<ButtonInput<MouseButton>>().clear();
    }
    fn wait(w: &mut World, seconds: f32) -> usize {
        w.resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(seconds));
        let mut shots = 0;
        for _ in 0..(seconds * 60.).round() as usize {
            w.run_schedule(FixedUpdate);
            shots += fired(w);
        }
        shots
    }
    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    press(w, KeyCode::KeyX);
    assert_eq!(
        wait(w, 4.),
        0,
        "the guns lay on the locked ship without firing"
    );
    let ready = on_target(w);
    assert!(ready >= 2, "{ready}");

    trigger(w, true);
    w.run_schedule(FixedUpdate);
    assert_eq!(fired(w), 1, "a press fires one barrel at once");
    trigger(w, false);
    assert_eq!(wait(w, 0.2), 0);
    trigger(w, true);
    w.run_schedule(FixedUpdate);
    assert_eq!(
        fired(w),
        ready - 1,
        "a second press fires every other barrel that bears"
    );
    trigger(w, false);
    w.run_schedule(Update);
    let texts: Vec<String> = w.query::<&Text>().iter(w).map(|t| t.0.clone()).collect();
    assert!(texts.iter().any(|t| t == "MAIN BATTERY"));
    // Each emptied barrel counts down in tenths of a second.
    let timers = texts
        .iter()
        .filter(|t| {
            t.strip_suffix(" s").is_some_and(|n| {
                n.parse::<f32>().is_ok() && n.split('.').nth(1).map(str::len) == Some(1)
            })
        })
        .count();
    assert!(timers >= ready, "{texts:?}");

    // Let the battery reload, then hold the trigger down.
    assert_eq!(wait(w, 7.), 0);
    let ready = on_target(w);
    trigger(w, true);
    let mut steps = vec![];
    for step in 0..(60. * RIPPLE * ready as f32) as usize + 10 {
        w.resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(1. / 60.));
        w.run_schedule(PreUpdate);
        w.run_schedule(FixedUpdate);
        steps.extend(std::iter::repeat_n(step, fired(w)));
    }
    assert_eq!(steps.len(), ready, "{steps:?}");
    assert_eq!(steps[0], 0);
    assert!(
        steps
            .windows(2)
            .all(|p| (p[1] - p[0]) as f32 / 60. >= RIPPLE - 1e-4),
        "{steps:?}"
    );
}

#[test]
fn ap_card_unlocks_q_and_the_secondary_hand_mounts_more_wing_guns() {
    use shipvivor_encounter::Ammo;
    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    let texts = |w: &mut World| -> Vec<String> {
        w.query::<&Text>().iter(w).map(|t| t.0.clone()).collect()
    };
    let shown_wing_guns = |w: &mut World| {
        w.query::<(&scene::WingPosition, &Visibility)>()
            .iter(w)
            .filter(|(_, v)| **v != Visibility::Hidden)
            .count()
    };
    w.run_schedule(Update);
    assert_eq!(shown_wing_guns(w), 2);
    // Q does nothing until AP is fitted.
    press(w, KeyCode::KeyQ);
    assert_eq!(w.resource::<Guns>().ammo, Ammo::He);
    assert!(!texts(w).iter().any(|t| t.contains("LOAD")));

    w.resource_mut::<Campaign>().tick(
        0.,
        Objectives {
            crates: 18,
            ..default()
        },
        0,
    );
    w.run_schedule(Update);
    // Armor-piercing shells, quick loaders, shell velocity, then the secondary hand.
    for _ in 0..4 {
        press(w, KeyCode::Digit1);
    }
    let fittings = w.resource::<Campaign>().fittings;
    assert!(fittings.armor_piercing());
    assert_eq!(fittings.secondaries().per_side, 2);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Sailing);
    assert_eq!(w.resource::<Encounter>().wing_guns().len(), 4);
    w.run_schedule(Update);
    assert_eq!(shown_wing_guns(w), 4);

    press(w, KeyCode::KeyQ);
    assert_eq!(w.resource::<Guns>().ammo, Ammo::Ap);
    w.run_schedule(Update);
    let shown = texts(w);
    assert!(shown.iter().any(|t| t == "LOAD AP"), "{shown:?}");
    assert!(shown.iter().any(|t| t.contains("Q  HE / AP")));
    // Barrels loaded before the switch still hold HE.
    assert!(shown.iter().any(|t| t.starts_with("HE ")), "{shown:?}");
    press(w, KeyCode::KeyQ);
    assert_eq!(w.resource::<Guns>().ammo, Ammo::He);
}

#[test]
fn both_dock_sections_move_the_after_gun_and_sail_on_the_longer_hull() {
    fn geometry(w: &mut World) -> (f32, f32, Vec<f32>) {
        let hull = w
            .query_filtered::<&Mesh3d, With<scene::Ship>>()
            .single(w)
            .unwrap();
        let mesh = w.resource::<Assets<Mesh>>().get(&hull.0).unwrap();
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .unwrap()
            .as_float3()
            .unwrap();
        let bow = positions.iter().map(|v| v[2]).fold(f32::INFINITY, f32::min);
        let stern = positions
            .iter()
            .map(|v| v[2])
            .fold(f32::NEG_INFINITY, f32::max);
        let mut guns: Vec<_> = w
            .query_filtered::<(&Transform, &Visibility), With<scene::Turret>>()
            .iter(w)
            .filter(|(_, visible)| **visible != Visibility::Hidden)
            .map(|(t, _)| t.translation.z)
            .collect();
        guns.sort_by(f32::total_cmp);
        (bow, stern, guns)
    }

    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    let stock = geometry(w);
    w.resource_mut::<Campaign>().tick_sector(
        1.,
        Objectives {
            kills: 4,
            destroyed_drydock: true,
            crates: 0,
        },
        0,
        true,
    );
    w.run_schedule(Update);
    assert!(w.query::<&Text>().iter(w).any(|t| t.as_str() == "40 m"));
    // A reserve bay dropped ahead of the after turret previews the slipway, even
    // though the ship still measures 40 m.
    let reserve = shipvivor_campaign::DockJob::Insert {
        at: w.resource::<Campaign>().default_gap(),
        module: shipvivor_campaign::DockModule::Reserve,
    };
    draft(w, reserve);
    assert_eq!(w.resource::<Campaign>().dock_job(), Some(reserve));
    assert_eq!(w.resource::<Campaign>().ship.design().length(), 40.);
    assert!((geometry(w).1 - stock.1 - 15.).abs() < 0.001);
    press(w, KeyCode::Escape);
    assert_eq!(w.resource::<Campaign>().dock_job(), None);
    let (bow, stern, guns) = geometry(w);
    assert_eq!(bow, stock.0);
    assert!((stern - stock.1 - 10.).abs() < 0.001);
    assert_eq!(guns, [stock.2[0], stock.2[1] + 10.]);
    assert_eq!(
        w.query_filtered::<&Visibility, With<scene::SectionModel>>()
            .iter(w)
            .filter(|shown| **shown == Visibility::Inherited)
            .count(),
        2
    );
    // Commit the third section and a turbine refit on the engine bay. Both persist
    // across departure. The new bay is selected once it is fitted.
    draft(w, reserve);
    press(w, KeyCode::Enter);
    assert_eq!(w.resource::<Campaign>().ship.design().length(), 45.);
    let shipvivor_campaign::DockJob::Insert { at, .. } = reserve else {
        unreachable!()
    };
    assert_eq!(w.resource::<Campaign>().dock_module, at);
    visit(w, 3);
    choose(
        w,
        shipvivor_campaign::DockJob::Fitting(shipvivor_campaign::Card::Turbines),
    );
    press(w, KeyCode::Enter);
    press(w, KeyCode::Enter);
    press(w, KeyCode::Digit1);
    assert_eq!(w.resource::<Campaign>().stage, Stage::OuterRoad);
    assert_eq!(w.resource::<ShipView>().hull_extension, 15.);
    assert_eq!(w.query::<&damage_control::Cell>().iter(w).count(), 9);
    assert_eq!(w.resource::<Campaign>().fittings.turbines, 1);
    let design = w.resource::<Campaign>().ship.design();
    assert!(design.top_speed() > shipvivor_navigation::Helm::FULL_AHEAD);
    assert!(design.turn_rate() < 0.38);
    assert!(w.query::<&Text>().iter(w).any(|t| t.as_str() == "RES"));

    press(w, KeyCode::Escape);
    press(w, KeyCode::KeyM);
    start_in_narrows(w);
    assert_eq!(geometry(w), stock);
    assert_eq!(w.query::<&damage_control::Cell>().iter(w).count(), 6);
}

#[test]
fn mission_clock_and_industry_bearings_remain_visible_in_the_last_minute() {
    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    w.resource_mut::<Campaign>().elapsed = 540.2;
    w.run_schedule(Update);
    let readouts: Vec<_> = w
        .query::<(&mission_hud::Reading, &Text, &TextColor)>()
        .iter(w)
        .map(|(kind, t, c)| (*kind, t.0.clone(), c.0))
        .collect();
    assert!(readouts.iter().any(
        |(kind, text, color)| matches!(kind, mission_hud::Reading::Clock)
            && text == "01:00"
            && *color == Color::srgb_u8(255, 148, 123)
    ));
    for (index, count) in ["1/1 factory", "2/2 factories", "3/3 factories"]
        .iter()
        .enumerate()
    {
        assert!(readouts.iter().any(
            |(kind, text, _)| matches!(kind, mission_hud::Reading::Bearing(i) if *i == index)
                && text.ends_with(count)
        ));
    }
    press(w, KeyCode::Escape);
    assert_eq!(w.query::<&mission_hud::Reading>().iter(w).count(), 0);
    for _ in 0..60 {
        w.run_schedule(FixedUpdate);
    }
    press(w, KeyCode::Escape);
    assert_eq!(w.resource::<Campaign>().elapsed, 540.2);
}

#[test]
fn yard_controls_take_a_mixed_battery_to_sea() {
    use shipvivor_campaign::{DeckModule, DockJob};

    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    w.resource_mut::<Campaign>().tick_sector(
        1.,
        Objectives {
            kills: 4,
            destroyed_drydock: true,
            crates: 8,
        },
        0,
        true,
    );
    w.run_schedule(Update);
    assert_eq!(w.resource::<Campaign>().fittings_remaining(), 4);
    // The yard opens on the earned reserve bay and previews nothing until a refit is
    // chosen. A centre turret heads its list.
    let reserve = w.resource::<Campaign>().turret_bay().unwrap();
    assert_eq!(w.resource::<Campaign>().dock_module, reserve);
    assert_eq!(w.resource::<Campaign>().dock_job(), None);
    assert_eq!(
        w.resource::<Campaign>().dock_jobs()[0],
        DockJob::Replace {
            at: reserve,
            with: DeckModule::CentreBattery
        }
    );

    // The bore is chosen on a turret, and too large a bore waits for a wider hull.
    visit(w, 1);
    choose(w, DockJob::Calibre(18));
    press(w, KeyCode::Enter);
    assert_eq!(w.resource::<Campaign>().fittings_remaining(), 4);
    assert_eq!(
        w.resource::<Campaign>().fittings.round().calibre_inches(),
        4.
    );

    // The hull is widened from the hull panel, one step at a time.
    for _ in 0..2 {
        draft(w, DockJob::WidenBeam);
        let beam = w.resource::<Campaign>().ship.design().beam();
        assert!(w.resource::<Campaign>().display_design().beam() > beam);
        press(w, KeyCode::Enter);
        assert!(w.resource::<Campaign>().ship.design().beam() > beam);
        assert_eq!(w.resource::<Campaign>().dock_job(), None);
    }
    assert_eq!(w.resource::<Campaign>().ship.design().beam(), 9.5);

    visit(w, 1);
    choose(w, DockJob::Move { from: 1, to: 2 });
    press(w, KeyCode::Enter);
    assert_eq!(w.resource::<Campaign>().dock_module, 2);
    assert_eq!(w.resource::<Campaign>().fittings_remaining(), 2);
    // The selection travels with the bay, ready to move it again.
    assert_eq!(
        w.resource::<Campaign>().dock_job(),
        Some(DockJob::Move { from: 2, to: 3 })
    );
    press(w, KeyCode::BracketRight);
    assert_eq!(w.resource::<Campaign>().dock_module, 3);
    assert!(
        !w.resource::<Campaign>()
            .dock_jobs()
            .iter()
            .any(|job| matches!(job, DockJob::Replace { .. })),
        "machinery is protected"
    );

    press(w, KeyCode::BracketLeft);
    choose(
        w,
        DockJob::Replace {
            at: 2,
            with: DeckModule::WingBattery,
        },
    );
    assert_eq!(
        w.resource::<Campaign>()
            .display_design()
            .gun_stations()
            .len(),
        3
    );
    assert_eq!(
        w.resource::<Campaign>().ship.design().gun_stations().len(),
        2
    );
    press(w, KeyCode::Enter);
    // The wing pair offers the bore for every turret too.
    choose(w, DockJob::Calibre(18));
    // The last order is fitted at once, and she sails on the next press.
    press(w, KeyCode::Enter);
    assert!(matches!(
        w.resource::<Campaign>().screen(),
        Screen::Drydock(_)
    ));
    assert_eq!(w.resource::<Campaign>().fittings_remaining(), 0);
    press(w, KeyCode::Enter);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Chart);
    press(w, KeyCode::Digit1);

    let run = w.resource::<Campaign>();
    assert_eq!(run.screen(), Screen::Sailing);
    let calibres: Vec<_> = run
        .ship
        .design()
        .gun_stations()
        .iter()
        .map(|station| run.fittings.for_station(station).round().calibre_inches())
        .collect();
    assert_eq!(calibres, [8., 8., 18.]);
    assert_eq!(w.query::<&gun_sight::GunCircle>().iter(w).count(), 3);
    let mut visible: Vec<_> = w
        .query_filtered::<(&Transform, &Visibility), With<scene::Turret>>()
        .iter(w)
        .filter(|(_, v)| **v != Visibility::Hidden)
        .map(|(t, _)| t.translation)
        .collect();
    visible.sort_by(|a, b| a.x.total_cmp(&b.x));
    assert_eq!(visible.len(), 3);
    assert!(visible[0].x < 0. && visible[2].x > 0.);
    assert!((visible[0].z - visible[2].z).abs() < 0.001);
}

#[test]
fn a_one_order_yard_adds_a_turret_that_stays_fitted_until_she_sails() {
    use shipvivor_campaign::{DeckModule, DockJob};

    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    w.resource_mut::<Campaign>()
        .tick(600., Objectives::default(), 0);
    w.run_schedule(Update);
    assert_eq!(w.resource::<Campaign>().fittings_remaining(), 1);

    // With no reserve bay, the yard opens on the forward turret. Splitting it into wing
    // turrets is how a single order adds a turret.
    assert_eq!(w.resource::<Campaign>().dock_module, 1);
    assert_eq!(w.resource::<Campaign>().dock_job(), None);
    let split = DockJob::Replace {
        at: 1,
        with: DeckModule::WingBattery,
    };
    choose(w, split);
    // The last order is fitted on the first press, like any other.
    press(w, KeyCode::Enter);
    assert_eq!(w.resource::<Campaign>().fittings_remaining(), 0);
    let turrets = |w: &World| w.resource::<Campaign>().ship.design().gun_stations().len();
    assert_eq!(turrets(w), 3);
    // Looking over another section leaves the work done, and she stays in dock.
    press(w, KeyCode::BracketRight);
    assert_eq!(w.resource::<Campaign>().dock_module, 2);
    assert_eq!(turrets(w), 3);
    assert!(matches!(
        w.resource::<Campaign>().screen(),
        Screen::Drydock(_)
    ));
    // The spent orders give way to the Sail button, and Enter sails.
    assert!(
        w.query::<&Text>()
            .iter(w)
            .any(|t| t.contains("Sail  [Enter]"))
    );
    press(w, KeyCode::Enter);
    let run = w.resource::<Campaign>();
    assert_eq!(run.screen(), Screen::Chart);
    assert_eq!(run.ship.design().gun_stations().len(), 3);
}

#[test]
fn a_spotter_contact_posts_a_bridge_alert_then_clears_the_marker() {
    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    let plane = w.resource::<Encounter>().spotters()[0].position;
    let at = w
        .resource::<Encounter>()
        .navigable_position(plane + Vec2::X * 70., 12.);
    w.resource_mut::<PlayerHelm>().position = at;
    w.run_schedule(FixedUpdate);
    w.run_schedule(Update);
    assert!(
        w.query_filtered::<(&Text, &Node), With<mission_hud::SpotterAlert>>()
            .iter(w)
            .any(|(mark, node)| mark.0 == "!" && node.display == Display::Flex)
    );
    assert!(
        w.query::<&Text>()
            .iter(w)
            .any(|t| t.0.contains("POSITION REPORTED"))
    );

    w.resource_mut::<PlayerHelm>().position = Vec2::splat(5000.);
    for _ in 0..210 {
        w.run_schedule(FixedUpdate);
    }
    w.run_schedule(Update);
    assert!(
        w.query_filtered::<&Node, With<mission_hud::SpotterAlert>>()
            .iter(w)
            .all(|node| node.display == Display::None)
    );
}
