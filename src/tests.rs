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
    assert!(!w.resource::<Campaign>().late());
}

#[test]
fn every_flagship_escort_fits_one_line_of_a_passage_card() {
    for round in 2..=shipvivor_campaign::PATROLS {
        for fleet in shipvivor_campaign::Fleet::ALL {
            let escort = shipvivor_sim::sector::doctrine(fleet).escort(round);
            let line = format!(
                "Escort: {}",
                ui::escorted(escort).expect("a flagship's escort")
            );
            // A card's detail holds about 35 characters of its typeface to a line.
            assert!(line.len() <= 35, "round {round} {fleet:?}: {line}");
        }
    }
}

#[test]
fn every_passage_verdict_fits_one_line_of_a_passage_card() {
    use shipvivor_sim::{Need, Outlook, Rating};
    for rating in Rating::ALL {
        for first in Need::ALL {
            for second in Need::ALL.into_iter().filter(|&n| n != first) {
                let outlook = Outlook {
                    cost: 1.,
                    rating,
                    needs: vec![first, second],
                };
                let line = ui::verdict(&outlook);
                assert!(line.len() <= ui::CARD_LINE, "{line}");
                assert!(line.contains(first.name()), "{line}");
            }
        }
    }
}

#[test]
fn the_passage_chart_and_enemy_air_cover_follow_the_passages_actually_sailed() {
    use shipvivor_campaign::{GRADES, Objectives, PATROLS, Stage, anchorages, charted};
    let keys = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3];
    let offered = anchorages(TEST_SEED);
    // Each run keeps to one place on the chart from its anchorage on: the moderate
    // passages at the top, or the first or the second of the tough passages under enemy
    // air cover, through whichever seas the run seed charts.
    for (lane, (&anchorage, key)) in offered.iter().zip(keys).enumerate() {
        let mut app = headless_game();
        let w = app.world_mut();
        press(w, KeyCode::Enter);
        assert_eq!(w.resource::<Campaign>().screen(), Screen::Departure);
        let mut pins = w.query_filtered::<(&ui::Action, &mut bevy::ui::RelativeCursorPosition), With<ui::passage_chart::ChartName>>();
        assert_eq!(pins.iter(w).count(), offered.len());
        for (action, mut cursor) in pins.iter_mut(w) {
            cursor.cursor_over = *action == ui::Action::Depart(anchorage);
        }
        w.resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        w.run_schedule(PreUpdate);
        w.resource_mut::<ButtonInput<MouseButton>>().reset_all();
        w.run_schedule(Update);
        assert_eq!(w.resource::<Campaign>().screen(), Screen::Sailing);
        assert_eq!(w.resource::<Campaign>().stage, anchorage);
        assert_eq!(w.resource::<Campaign>().seed, TEST_SEED);
        let chart = Passage::of(anchorage).chart(TEST_SEED);
        assert_eq!(
            w.resource::<Encounter>().ports()[0].body.pos,
            chart.harbours[0].pos
        );
        assert!(w.resource::<Encounter>().flagship().is_none());
        assert!(w.resource::<Encounter>().air_station().is_none());
        press(w, KeyCode::Escape);
        let mut labels = w.query::<&Text>();
        for stage in [anchorage, Stage::CrownDeep]
            .into_iter()
            .chain(charted(TEST_SEED, 2).unwrap())
        {
            assert!(labels.iter(w).any(|label| label.0 == stage.mission().name));
        }
        press(w, KeyCode::Escape);
        // Every leg but the last earns its three stars, and each yard charts the next
        // round's passages.
        let air = lane > 0;
        for round in 1..PATROLS {
            {
                let mut run = w.resource_mut::<Campaign>();
                let goals = run.stage.mission();
                run.objectives = Objectives {
                    kills: goals.kills,
                    crates: goals.crates,
                    destroyed_drydock: true,
                    yards: 1,
                    ..default()
                };
                clear_leg(&mut run);
                assert!(run.leave_dock());
            }
            w.run_schedule(Update);
            assert_eq!(w.resource::<Campaign>().screen(), Screen::Chart);
            press(w, key);
            let sea = charted(TEST_SEED, round + 1).unwrap()[lane];
            assert_eq!(w.resource::<Campaign>().passage(), Some(sea));
            assert_eq!(w.resource::<Campaign>().stage, sea);
            assert_eq!(w.resource::<Campaign>().grade, GRADES[lane]);
            assert_eq!(w.resource::<Encounter>().air_station().is_some(), air);
            if w.resource::<Campaign>().screen() == Screen::Refit {
                press(w, KeyCode::Digit1);
            }
        }
        assert_eq!(w.resource::<Campaign>().course.len(), PATROLS - 1);
        // The clock runs out and she sails on, until her bases are down.
        let deadline = f32::from(w.resource::<Campaign>().stage.mission().minutes) * 60.;
        w.resource_mut::<Campaign>()
            .tick(deadline + 10., Objectives::default(), 0);
        w.run_schedule(Update);
        assert_eq!(w.resource::<Campaign>().screen(), Screen::Sailing);
        clear_leg(&mut w.resource_mut::<Campaign>());
        w.run_schedule(Update);
        // The last patrol ends in a yard too, which sails her to the briefing.
        assert_eq!(
            w.resource::<Campaign>().screen(),
            Screen::Drydock(LegEnd::BasesDestroyed)
        );
        sail_from_yard(w);
        assert_eq!(
            w.resource::<Campaign>().screen(),
            Screen::Intercept(LegEnd::BasesDestroyed)
        );
        press(w, KeyCode::Enter);
        assert_eq!(w.resource::<Campaign>().stage, Stage::CrownDeep);
        assert_eq!(w.resource::<Campaign>().origin, anchorage);
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
        for port in offered {
            assert_eq!(
                plotted.iter(w).any(|name| name.0 == port.mission().name),
                port == anchorage
            );
        }
        press(w, KeyCode::KeyM);
        start_in_narrows(w);
        assert_eq!(w.resource::<Campaign>().passage(), None);
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
    let first = shipvivor_campaign::anchorages(TEST_SEED);
    assert_eq!(w.resource::<Campaign>().stage, first[2]);
    w.resource_mut::<Campaign>()
        .tick(1., Objectives::default(), 1000);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Lost);
    press(w, KeyCode::Enter);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Departure);
    // The replay charts its own anchorages from its own seed.
    let next = w.resource::<ChartSeeds>().peek();
    press(w, KeyCode::Digit1);
    let run = w.resource::<Campaign>();
    assert_eq!(run.stage, shipvivor_campaign::anchorages(next)[0]);
    assert_eq!(run.screen(), Screen::Sailing);
    assert_ne!(run.seed, TEST_SEED);
    assert!(run.ship.afloat());
}

/// Begin a run in the Iron Narrows, whose waters many tests know by heart. The test
/// seed's first chart doesn't offer them, so this clicks a pin for them, as the chart
/// would for a seed that did.
fn start_in_narrows(world: &mut World) {
    press(world, KeyCode::Enter);
    assert_eq!(world.resource::<Campaign>().screen(), Screen::Departure);
    let pin = world
        .spawn((
            ui::Action::Depart(shipvivor_campaign::Stage::IronNarrows),
            bevy::ui::RelativeCursorPosition {
                cursor_over: true,
                ..default()
            },
        ))
        .id();
    world
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    world.run_schedule(PreUpdate);
    world.resource_mut::<ButtonInput<MouseButton>>().reset_all();
    world.despawn(pin);
    world.run_schedule(Update);
    assert_eq!(
        world.resource::<Campaign>().stage,
        shipvivor_campaign::Stage::IronNarrows
    );
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
        .find(|job| matches!(job, DockJob::Fitting(Card::Caps | Card::Velocity)))
        .expect("a turret bay is selected")
}

/// From the chart, sail open water through every round left, ending each patrol as she
/// arrives, until the last patrol's yard sails her to the briefing for the intercept.
fn sail_on_to_the_intercept(w: &mut World) {
    while w.resource::<Campaign>().screen() == Screen::Chart {
        press(w, KeyCode::Digit1);
        {
            let mut run = w.resource_mut::<Campaign>();
            assert_eq!(run.screen(), Screen::Sailing);
            clear_leg(&mut run);
            assert!(run.leave_dock());
        }
        w.run_schedule(Update);
    }
}

/// Bring down the last of her bases on the campaign's side and end the leg, with
/// nothing else gained.
/// Seconds into a leg past any clock, however many bases have put time on it.
const PAST_THE_CLOCK: f32 = 10_000.;

/// Clear the leg past its clock, so the yard's budget in these tests is its stars' and
/// yards' alone and the clock pays nothing.
fn clear_leg(run: &mut shipvivor_campaign::Run) {
    run.elapsed = run.elapsed.max(run.clock() + 1.);
    run.tick_sector(0., shipvivor_campaign::Objectives::default(), 0, true);
}

/// Sail from the yard with Enter, with nothing previewed. Points left over are banked,
/// so one press sails.
fn sail_from_yard(w: &mut World) {
    assert!(matches!(
        w.resource::<Campaign>().screen(),
        Screen::Drydock(_)
    ));
    // Nothing picked out, so Enter builds the orders, then sails.
    if w.resource::<Campaign>().dock_job().is_some() {
        press(w, KeyCode::Escape);
    }
    if w.resource::<Campaign>().plan().is_some() {
        press(w, KeyCode::Enter);
        assert!(w.resource::<Campaign>().plan().is_none());
    }
    press(w, KeyCode::Enter);
    assert!(!matches!(
        w.resource::<Campaign>().screen(),
        Screen::Drydock(_)
    ));
}

/// Take `card` from the hand on offer with its number key. Hands are dealt by rarity from
/// the run seed, so the seed is stepped on until a hand deals it.
fn pick(w: &mut World, card: shipvivor_campaign::Card) {
    assert!(w.resource::<Campaign>().fittings.can_fit(card), "{card:?}");
    while !w.resource::<Campaign>().offers().contains(&card) {
        w.resource_mut::<Campaign>().seed += 1;
    }
    let hand = w.resource::<Campaign>().offers();
    let slot = hand.iter().position(|&dealt| dealt == card).unwrap();
    press(w, [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3][slot]);
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
        .init_resource::<bevy::input::mouse::AccumulatedMouseScroll>()
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
    // The hand on offer is the one dealt face up, each card with its rarity's gems.
    let hand = w.resource::<Campaign>().offers();
    let before = w.resource::<Campaign>().fittings;
    let card_labels: Vec<_> = w
        .query::<&Text>()
        .iter(w)
        .map(|text| text.0.clone())
        .collect();
    for &card in &hand {
        let name = cards::face(card, before, w.resource::<Campaign>().ship.design()).name;
        assert!(card_labels.iter().any(|text| text.contains(name)), "{name}");
    }
    let gems: usize = hand
        .iter()
        .map(|card| cards::Gem::count(card.rarity().unwrap()))
        .sum();
    assert_eq!(w.query::<&cards::Gem>().iter(w).count(), gems);
    let pos = w.resource::<PlayerHelm>().position;
    let time = w.resource::<Campaign>().elapsed;
    w.run_schedule(FixedUpdate);
    assert_eq!(w.resource::<PlayerHelm>().position, pos);
    assert_eq!(w.resource::<Campaign>().elapsed, time);
    press(w, KeyCode::Digit2);
    let fitted = before.with(hand[1]);
    assert_eq!(w.resource::<Campaign>().fittings, fitted);
    // A patrol ends only once every base is down. Wreck them all on the first leg and
    // on the second, and sail on to the yard each time.
    let mut docked = fitted;
    let mut carried = 0;
    for leg in 0..2 {
        let bases = w.resource::<Encounter>().ports().len();
        for base in 0..bases {
            destroy_drydock(&mut w.resource_mut::<Encounter>(), base);
        }
        for _ in 0..600 {
            w.run_schedule(FixedUpdate);
            if w.resource::<Campaign>().screen() == Screen::Refit {
                press(w, KeyCode::Digit1);
            }
            if matches!(
                w.resource::<Campaign>().phase(),
                shipvivor_campaign::Phase::Drydock(_)
            ) {
                break;
            }
        }
        // The sector is secured for a moment, then any levels owed are dealt.
        for _ in 0..135 {
            w.run_schedule(FixedUpdate);
        }
        w.run_schedule(Update);
        for _ in 0..4 {
            if w.resource::<Campaign>().screen() == Screen::Refit {
                press(w, KeyCode::Digit1);
            }
        }
        assert!(w.resource::<Campaign>().objectives.destroyed_drydock);
        if leg == 0 {
            assert_eq!(
                w.resource::<Campaign>().screen(),
                Screen::Drydock(LegEnd::BasesDestroyed)
            );
            visit(w, 1);
            let shells = shell_refit(w);
            choose(w, shells);
            // The drydock star and its yard pay for a shell refit. It is ordered at once,
            // the next Enter builds it, and the one after, with nothing picked out, sails
            // with the points left over.
            let points = w.resource::<Campaign>().points();
            assert!(points >= 3, "{points} points");
            press(w, KeyCode::Enter);
            assert_eq!(w.resource::<Campaign>().points(), points - shells.cost());
            press(w, KeyCode::Enter);
            assert!(w.resource::<Campaign>().plan().is_none());
            assert_eq!(
                w.resource::<Campaign>().screen(),
                Screen::Drydock(LegEnd::BasesDestroyed)
            );
            docked = w.resource::<Campaign>().fittings;
            assert_ne!(docked, fitted);
            carried = w.resource::<Campaign>().points();
            // Points left to spend are banked, and one press sails.
            press(w, KeyCode::Enter);
            assert_eq!(w.resource::<Campaign>().screen(), Screen::Chart);
            assert_eq!(w.resource::<Campaign>().points(), carried);
            let tally = (w.resource::<Campaign>().hits, w.resource::<Campaign>().sunk);
            let moderate = w.resource::<Campaign>().exits().unwrap()[0].stage;
            press(w, KeyCode::Digit1);
            let run = w.resource::<Campaign>();
            assert_eq!(run.stage, moderate);
            assert_eq!(run.elapsed, 0.);
            assert_eq!((run.hits, run.sunk), tally);
            assert_eq!(run.fittings, docked);
            assert_eq!(w.resource::<Encounter>().shots().len(), 0);
            assert!(
                w.query::<(&Instrument, &Text)>()
                    .iter(w)
                    .any(|(gauge, text)| matches!(gauge, Instrument::Flagship)
                        && text.contains("fitting out\nPuts to sea in 3:54"))
            );
        } else {
            // The second patrol ends in a yard too, where the points carried from the
            // first are spent with those the second leg paid.
            assert_eq!(
                w.resource::<Campaign>().screen(),
                Screen::Drydock(LegEnd::BasesDestroyed)
            );
            let run = w.resource::<Campaign>();
            let (points, loaders) = (run.points(), run.fittings.loader);
            assert_eq!(points, carried + run.leg_points());
            assert!(points >= 3, "{points} points");
            visit(w, 1);
            choose(
                w,
                shipvivor_campaign::DockJob::Fitting(shipvivor_campaign::Card::Loader),
            );
            press(w, KeyCode::Enter);
            let run = w.resource::<Campaign>();
            assert_eq!(
                (run.points(), run.fittings.loader),
                (points - 2, loaders + 1)
            );
            sail_from_yard(w);
        }
    }
    // The second yard charts the third round. Open water and quick yards carry her on
    // until the last of them sails her to the briefing.
    sail_on_to_the_intercept(w);
    assert_eq!(
        w.resource::<Campaign>().screen(),
        Screen::Intercept(LegEnd::BasesDestroyed)
    );
    press(w, KeyCode::Enter);
    let run = w.resource::<Campaign>();
    assert_eq!(run.stage, Stage::CrownDeep);
    // Field refits on the second leg add to the shells she was given, and take nothing away.
    let shells = |f: shipvivor_campaign::Fittings| (f.caps, f.velocity);
    let (now, then) = (shells(run.fittings), shells(docked));
    assert!(now.0 >= then.0 && now.1 >= then.1);
    assert_eq!(run.screen(), Screen::Sailing);
    // Her screen meets the ship first, and the Regent holds back over the horizon.
    let battle = w.resource::<Encounter>();
    assert!(battle.boss_ship().is_none());
    assert_eq!(
        battle.flagship(),
        Some(shipvivor_encounter::Sortie::Holding)
    );
    assert!(
        (battle.raiders().iter()).any(|r| r.class == shipvivor_encounter::EnemyClass::Cruiser)
    );
    w.resource_mut::<Campaign>()
        .tick_sector(61., Objectives::default(), 0, true);
    // The clock runs out on nobody at Crown Deep: she fights on until one of them sinks.
    let left = w.resource::<Campaign>().remaining();
    w.resource_mut::<Campaign>()
        .tick(left + 30., Objectives::default(), 0);
    w.run_schedule(Update);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Sailing);
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
    assert_eq!(
        w.resource::<Campaign>().fittings,
        shipvivor_campaign::Fittings::default()
    );
    assert_eq!(w.resource::<Campaign>().points(), 0);
    assert_eq!(
        (w.resource::<Campaign>().hits, w.resource::<Campaign>().sunk),
        (0, 0)
    );
}

/// One of a base's works to shell: a drydock or a factory, by its index there.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Works {
    Dock(usize),
    Factory(usize),
}

/// Shell one of base `index`'s works until it falls, from the first bearing 90 m off
/// that has a clear line to it.
pub(crate) fn shell_works(battle: &mut shipvivor_encounter::Battle, index: usize, works: Works) {
    let standing = |battle: &shipvivor_encounter::Battle| {
        let port = &battle.ports()[index];
        match works {
            Works::Dock(j) => port.docks()[j].live(),
            Works::Factory(j) => port.factories()[j].live(),
        }
    };
    let target = |battle: &shipvivor_encounter::Battle| {
        let port = &battle.ports()[index];
        let (at, bottom, top) = match works {
            Works::Dock(j) => (port.dock_position(j), 3.2, port.dock_top(j)),
            Works::Factory(j) => (
                port.factory_position(j),
                port.factory_bottom(j),
                port.factory_top(j),
            ),
        };
        (at, at.extend((bottom + top) * 0.5).xzy())
    };
    // Open water 90 m off with a clear line to it, the bearings nearest the base's front
    // first. Looked for afresh every second, as ships come and go across the line.
    let pick = |battle: &shipvivor_encounter::Battle| {
        let (at, mark) = target(battle);
        let front = battle.ports()[index].body.forward();
        (0..36)
            .map(|k: i32| {
                let off = (k + 1) / 2 * if k % 2 == 0 { 1 } else { -1 };
                let turn = off as f32 * std::f32::consts::TAU / 36.;
                at + 90. * Vec2::from_angle(turn).rotate(front)
            })
            .find(|&from| {
                let eye = from.extend(8.).xzy();
                let sight = battle.sight(eye, (mark - eye).normalize(), at);
                battle.navigable_position(from, 12.).distance(from) < 0.01
                    && match works {
                        Works::Dock(j) => sight.yard == Some((index, j)),
                        Works::Factory(j) => sight.factory == Some((index, j)),
                    }
            })
    };
    let mut from =
        pick(battle).unwrap_or_else(|| panic!("no clear line to {works:?} at base {index}"));
    for step in 0..1800 {
        if !standing(battle) {
            break;
        }
        if step % 30 == 0 {
            from = pick(battle).unwrap_or(from);
        }
        let (at, mark) = target(battle);
        battle.step(
            1. / 30.,
            &Orders {
                player: from,
                heading: shipvivor_compass::heading(at - from),
                hull: Hull::CRUISER,
                speed: 0.,
                aim: Aim::Sight(mark),
                shell: MainShell::STOCK,
                reload: 0.5,
            },
        );
    }
    assert!(!standing(battle), "{works:?} at base {index} still stands");
}

/// Wreck base `index`: shell every drydock, then every factory, until nothing is left to
/// raise a dock again.
fn destroy_drydock(battle: &mut Encounter, index: usize) {
    for _ in 0..12 {
        let port = &battle.ports()[index];
        if port.wrecked() {
            break;
        }
        let dock = (0..port.docks().len()).find(|&j| port.docks()[j].live());
        let factory = (0..port.factories().len()).find(|&j| port.factories()[j].live());
        let works = dock.map(Works::Dock).or(factory.map(Works::Factory));
        if let Some(works) = works {
            shell_works(battle, index, works);
        }
    }
    assert!(battle.ports()[index].wrecked());
}

#[test]
fn destroying_every_yard_brings_the_ship_through_drydock_even_past_the_clock() {
    for late in [false, true] {
        let reason = LegEnd::BasesDestroyed;
        let mut app = headless_game();
        let w = app.world_mut();
        start_in_narrows(w);
        if late {
            // The clock runs out and nothing ends: she sails on, and the clock counts up
            // the time over. She is far enough past it that her bases' 45 s apiece
            // never bring her back inside.
            w.resource_mut::<Campaign>().elapsed = 600.;
            w.run_schedule(FixedUpdate);
            w.run_schedule(Update);
            assert_eq!(w.resource::<Campaign>().screen(), Screen::Sailing);
            assert!(w.resource::<Campaign>().late());
            assert!(
                w.query::<(&mission_hud::Reading, &Text)>()
                    .iter(w)
                    .any(|(r, t)| matches!(r, mission_hud::Reading::Clock) && t.0.starts_with('+'))
            );
        }
        // Fire on each yard from the fairway. Between bases, let the game read the
        // resulting port states and update its mission HUD.
        for i in 0..w.resource::<Encounter>().ports().len() {
            destroy_drydock(&mut w.resource_mut::<Encounter>(), i);
            assert_eq!(w.resource::<Encounter>().ruined_yards(), i + 1);
            // A level comes due on the tick the last yard falls.
            if i + 1 == w.resource::<Encounter>().ports().len() {
                w.resource_mut::<Campaign>().experience = 30;
            }
            w.run_schedule(FixedUpdate);
            w.run_schedule(Update);
            let total = w.resource::<Encounter>().ports().len();
            if i + 1 < total {
                assert_eq!(w.resource::<Campaign>().screen(), Screen::Sailing);
                assert!(w.resource::<Campaign>().objectives.destroyed_drydock);
                let count = format!("{} / {total}", i + 1);
                assert!(
                    w.query::<(&mission_hud::Reading, &Text)>()
                        .iter(w)
                        .any(|(r, t)| matches!(r, mission_hud::Reading::Bases) && t.0 == count)
                );
            }
        }
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
            // The level owed is dealt as a free hand before the yard opens.
            assert_eq!(w.resource::<Campaign>().screen(), Screen::Refit);
            assert_eq!(w.resource::<Campaign>().fittings_remaining(), 1);
            assert_eq!(w.resource::<Campaign>().dock_job(), None);
            press(w, KeyCode::Digit1);
        }
        // Two points a star, one a wrecked yard, and two more for beating the clock.
        // Cleared late, she keeps the drydock star and misses only the clock's points.
        let yards = w.resource::<Encounter>().ports().len() as u16;
        let run = w.resource::<Campaign>();
        assert_eq!(run.screen(), Screen::Drydock(reason));
        assert_eq!((run.fittings_remaining(), run.objectives.yards), (0, yards));
        assert_eq!((run.stars(), run.beat_clock()), (1, !late));
        let points = run.points();
        assert_eq!(
            points,
            shipvivor_campaign::STAR_POINTS * (u16::from(run.stars()) + u16::from(!late)) + yards
        );
        let elapsed = run.elapsed;
        w.run_schedule(FixedUpdate);
        assert_eq!(w.resource::<Campaign>().elapsed, elapsed);
        let purse = if points == 1 {
            "1 point".to_string()
        } else {
            format!("{points} points")
        };
        assert!(w.query::<&Text>().iter(w).any(|t| t.contains("DRYDOCK")));
        assert!(w.query::<&Text>().iter(w).any(|t| t.0 == purse), "{purse}");
        visit(w, 1);
        let shells = shell_refit(w);
        choose(w, shells);
        let paid = points >= shells.cost();
        if !paid {
            // Short of points, the refit's figures say so.
            assert!(
                w.query::<&Text>()
                    .iter(w)
                    .any(|t| t.contains("Short of points"))
            );
        }
        press(w, KeyCode::Enter);
        assert_eq!(w.resource::<Campaign>().fittings.caps > 0, paid);
        let left = if paid { points - shells.cost() } else { points };
        assert_eq!(w.resource::<Campaign>().points(), left);
        if paid {
            // The next Enter locks in the order.
            press(w, KeyCode::Enter);
            assert!(w.resource::<Campaign>().plan().is_none());
        } else {
            // Enter on a refit the yard cannot make leaves it picked out, and she stays in
            // dock rather than sailing by mistake. Esc puts it down.
            assert_eq!(w.resource::<Campaign>().screen(), Screen::Drydock(reason));
            assert_eq!(w.resource::<Campaign>().dock_job(), Some(shells));
            press(w, KeyCode::Escape);
        }
        // She waits in dock until she is sailed, and one Enter sails her with what is left
        // banked for the next yard.
        assert_eq!(w.resource::<Campaign>().screen(), Screen::Drydock(reason));
        press(w, KeyCode::Enter);
        assert_eq!(w.resource::<Campaign>().screen(), Screen::Chart);
        assert_eq!(w.resource::<Campaign>().points(), left);
        // The tough passages want three stars. The moderate one is always open.
        press(w, KeyCode::Digit3);
        assert_eq!(w.resource::<Campaign>().screen(), Screen::Chart);
        let moderate = w.resource::<Campaign>().exits().unwrap()[0].stage;
        press(w, KeyCode::Digit1);
        let run = w.resource::<Campaign>();
        assert_eq!(
            (run.screen(), run.stage, run.elapsed),
            (Screen::Sailing, moderate, 0.)
        );
        assert_eq!(w.resource::<Encounter>().ruined_yards(), 0);
        assert_eq!(run.fittings.caps > 0, paid);
        assert_eq!((run.points(), run.objectives.yards), (left, 0));
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
    use shipvivor_campaign::Grade;
    use shipvivor_encounter::{FLAGSHIP_EARLIEST, FLAGSHIP_SORTIE, SPAWN_STANDOFF};
    // Every sea of every pool, once each, turn about moderate and tough and from three
    // run seeds, which keeps sixty-odd seas quick to check.
    let sailings = [
        (TEST_SEED, Grade::Moderate),
        (2, Grade::Tough),
        (3, Grade::Moderate),
    ];
    for (i, stage) in shipvivor_campaign::POOLS
        .into_iter()
        .flatten()
        .copied()
        .enumerate()
    {
        let (seed, grade) = sailings[i % sailings.len()];
        let passage = Passage::graded(stage, grade);
        let chart = passage.chart(seed);
        let battle = passage.battle(&chart);
        // Past the first leg the flagship is fitting out, not yet at sea, and she puts
        // to sea sooner the deeper the round.
        assert!(
            battle
                .raiders()
                .iter()
                .all(|ship| ship.class != shipvivor_encounter::EnemyClass::Parliament)
        );
        let head_start = shipvivor_sim::Sector::of(stage, grade).head_start();
        assert_eq!(battle.escalation(), head_start);
        assert_eq!(
            battle.flagship(),
            (!stage.is_opening()).then_some(shipvivor_encounter::Sortie::Due(
                (FLAGSHIP_SORTIE - head_start).max(FLAGSHIP_EARLIEST)
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
            for &point in &chart.gates {
                assert_eq!(battle.navigable_position(point, 15.), point);
            }
        }
    }
    let finale = Passage::of(Stage::CrownDeep);
    let afloat_on_open_water = |battle: &shipvivor_encounter::Battle, ship: &shipvivor_encounter::Raider| {
        let hull = ship.class.profile().hull;
        ship.body.pos.length() >= SPAWN_STANDOFF
            && battle
                .navigable_position(ship.body.pos, hull.bow)
                .distance(ship.body.pos)
                < 0.1
    };
    for seed in [1, 2, 3, 83] {
        let chart = finale.chart(seed);
        let mut battle = finale.battle(&chart);
        // Her screen lies on open water clear of the ship, and she holds back.
        assert!(battle.boss_ship().is_none());
        assert_eq!(
            battle.flagship(),
            Some(shipvivor_encounter::Sortie::Holding)
        );
        for ship in battle.raiders().iter().filter(|r| !r.class.is_merchant()) {
            assert!(afloat_on_open_water(&battle, ship), "run {seed}: {:?}", ship.class);
        }
        if seed > 2 {
            continue;
        }
        // Left alone, she comes up the deep in the end, on open water and far off.
        let orders = shipvivor_encounter::Orders {
            player: Vec2::ZERO,
            heading: 0.,
            speed: 0.,
            hull: shipvivor_encounter::Hull::CRUISER,
            aim: shipvivor_encounter::Aim::Hold,
            reload: 2.5,
            shell: shipvivor_encounter::MainShell::STOCK,
        };
        for _ in 0..241 * 10 {
            battle.step(0.1, &orders);
        }
        let boss = battle.boss_ship().expect("she has come up");
        assert!(boss.body.pos.length() > 1300., "run {seed}");
        assert!(afloat_on_open_water(&battle, boss), "run {seed}");
        assert_eq!(boss.live_guns(), 12);
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
    // A bearing with open water out past the guns' reach, clear of the shores: a beam
    // where the channel is broad enough, or else astern, out through its mouth.
    let open = |w: &World, bearing: f32, range: f32| {
        let mark = at + direction(bearing) * range;
        let mut view = ShipView::default();
        view.center_on(at, mark.extend(0.).xzy());
        let sight = view.sight(at, w.resource::<Encounter>());
        sight.point.y < 0.01 && sight.point.xz().distance(mark) < 30.
    };
    // Nor may a ship or a base stand near the line, either way, or the sight marks it.
    let clear = |w: &World, bearing: f32| {
        let battle = w.resource::<Encounter>();
        let line = direction(bearing);
        let ports = battle.ports().iter().map(|port| port.body.pos);
        battle
            .raiders()
            .iter()
            .map(|ship| ship.body.pos)
            .chain(ports)
            .all(|p| (p - at).length() > 1600. || line.angle_to(p - at).abs() > 0.3)
    };
    let bearing = [
        70., -70., 90., -90., 110., -110., 50., -50., 180., 160., -160.,
    ]
    .map(|degrees: f32| heading + degrees.to_radians())
    .into_iter()
    .find(|&bearing| open(w, bearing, 1300.) && open(w, bearing, 350.) && clear(w, bearing))
    .expect("open water on a beam or astern");
    // A bearing well round from it, as clear, to swing the sight across to.
    let across = [180., 135., -135., 90., -90.]
        .map(|degrees: f32| bearing + degrees.to_radians())
        .into_iter()
        .find(|&swung| clear(w, swung))
        .expect("a clear bearing well round from the first");
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
    // Swung well round, every mount is still training round.
    let (_, _, drawn) = lay(w, across, 350., 3);
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
    let (yard, top) = port.lookout().unwrap();
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
fn the_clock_ticks_through_its_last_minute_and_a_base_wrecked_hushes_it() {
    use shipvivor_acoustics::{SAMPLE_RATE, Sound};
    let mut app = headless_game();
    let w = app.world_mut();
    let mut mixer = w.resource::<sound::Soundboard>().claim().unwrap();
    start_in_narrows(w);
    // Far out to sea for so many seconds. Returns how many beats were struck.
    let mut sail = |w: &mut World, secs: usize| {
        let (mut beats, mut sounding) = (0, false);
        for _ in 0..secs * 60 {
            w.resource_mut::<PlayerHelm>().position = Vec2::splat(-6000.);
            w.run_schedule(FixedUpdate);
            mixer.render(&mut vec![[0.; 2]; (SAMPLE_RATE / 60) as usize]);
            let now = mixer.making(Sound::Tick).0 > 0;
            beats += usize::from(now && !sounding);
            sounding = now;
        }
        beats
    };
    let leave = |w: &mut World, left: f32| {
        let run = &mut w.resource_mut::<Campaign>();
        run.elapsed = run.clock() - left;
    };
    let wreck_a_base = |w: &mut World| w.resource_mut::<Campaign>().objectives.yards += 1;
    // Two minutes out it is quiet. Inside the last minute it beats once a second.
    leave(w, 120.);
    assert_eq!(sail(w, 3), 0);
    leave(w, 30.);
    assert!((4..=5).contains(&sail(w, 5)));
    // A base wrecked puts 45 s back. The ticking dies away in a few beats and is gone.
    wreck_a_base(w);
    assert!((1..=3).contains(&sail(w, 4)));
    assert_eq!(sail(w, 4), 0);
    // Time won with seconds to spare leaves the clock inside its last minute, and still
    // the ticking fades and holds off while 15 s of it run down.
    leave(w, 10.);
    assert!(sail(w, 2) >= 1);
    wreck_a_base(w);
    assert!(sail(w, 4) <= 3);
    assert_eq!(sail(w, 8), 0);
    assert!(sail(w, 8) >= 3);
    // Once it has run out the clock says no more.
    leave(w, 2.);
    sail(w, 3);
    assert_eq!(sail(w, 3), 0);
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
            let gates = chart.gates.clone();
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
            // Every yard fills its stores, once the hulls on its slips have their steel.
            // Those further up the channel sleep, building but launching nothing, and send
            // freighters down to the yards awake.
            let mut sailed = false;
            let full = |b: &Battle| b.ports().iter().all(|p| p.stock == YARD_CAPACITY);
            for _ in 0..2400 {
                b.step(0.1, &observer);
                sailed |= b.raiders().iter().any(|r| r.class == EnemyClass::Freighter);
                if full(&b) {
                    break;
                }
            }
            assert!(
                sailed && full(&b),
                "{stage:?} seed {seed} {:?}",
                b.ports().iter().map(|p| p.stock).collect::<Vec<_>>()
            );
        }
    }
}

/// Each run charts its waters afresh, and every chart the game sails has passed the full
/// survey: every gate, slip, yard and factory within reach, bases spread apart. These
/// seas take many runs each, and the simulator's tests survey every other sea of the
/// chain.
#[test]
fn every_leg_of_many_runs_sails_waters_that_pass_their_survey() {
    use shipvivor_encounter::Survey;
    for stage in [
        Stage::IronNarrows,
        Stage::LanternBay,
        Stage::BlackcapReach,
        Stage::OuterRoad,
        Stage::GreywaterSound,
        Stage::PilotsCut,
        Stage::CrownDeep,
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
    // The ship standing most clear of the rest, by bearing, other than the nearest, so a
    // near miss on her lies nearer her than anyone.
    let bearing = |r: &shipvivor_encounter::Raider| shipvivor_compass::heading(r.body.pos - player);
    let nearest = b.nearest_ship(player).unwrap().body.id;
    let clearance = |r: &shipvivor_encounter::Raider| {
        b.raiders()
            .iter()
            .filter(|o| o.body.id != r.body.id)
            .map(|o| {
                use std::f32::consts::{PI, TAU};
                (bearing(o) - bearing(r) + PI).rem_euclid(TAU) - PI
            })
            .map(f32::abs)
            .fold(f32::INFINITY, f32::min)
    };
    let ship = b
        .raiders()
        .iter()
        .filter(|r| r.body.id != nearest && r.body.pos.distance(player) < 900.)
        .max_by(|x, y| clearance(x).total_cmp(&clearance(y)))
        .unwrap();
    assert_ne!(nearest, ship.body.id);
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
    // Off her clear side. On the other, a second patrol boat lies just beyond her, and
    // the scope rides onto it. In other waters it was an islet behind her, and from the
    // shore the free sight looks only for harbour guns.
    view.bearing += 0.021;
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
fn torpedo_mode_click_launches_once_and_a_larger_bore_resizes_the_deck_models() {
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

    // The bore is yard work, turret by turret. A larger one resizes its gunhouse as soon
    // as it is ordered, and the other turret keeps its own.
    w.resource_mut::<ButtonInput<MouseButton>>().reset_all();
    w.resource_mut::<Campaign>().tick(
        0.,
        Objectives {
            kills: 4,
            yards: 1,
            ..default()
        },
        0,
    );
    clear_leg(&mut w.resource_mut::<Campaign>());
    w.run_schedule(Update);
    visit(w, 1);
    choose(
        w,
        shipvivor_campaign::DockJob::Calibre {
            at: 1,
            from: 4,
            to: 6,
        },
    );
    press(w, KeyCode::Enter);
    w.run_schedule(Update);
    let fitted = w.resource::<Campaign>().fittings.round();
    assert_eq!(fitted.calibre_inches(), 6.);
    assert_eq!(
        scales(w),
        vec![fitted.mount_scale(), MainShell::STOCK.mount_scale()]
    );
}

#[test]
fn turrets_bought_in_the_yard_refit_deck_models_sight_pips_and_battery_board() {
    use shipvivor_campaign::DockJob;
    use shipvivor_encounter::Station;
    use shipvivor_encounter::TurretLayout;
    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    // The raider star, the drydock star and three wrecked yards pay seven points: five to
    // make the forward turret a triple, a barrel at a time, and two for a twin aft.
    w.resource_mut::<Campaign>().tick(
        0.,
        Objectives {
            kills: 4,
            yards: 3,
            ..default()
        },
        0,
    );
    clear_leg(&mut w.resource_mut::<Campaign>());
    w.run_schedule(Update);
    assert_eq!(w.resource::<Campaign>().points(), 7);
    let barrels = |w: &World, at: usize| {
        w.resource::<Campaign>().ship.design().modules()[at]
            .gunhouse
            .barrels
    };
    visit(w, 1);
    for (to, tubes) in [(TurretLayout::Twin, 2), (TurretLayout::Triple, 3)] {
        let from = barrels(w, 1);
        choose(w, DockJob::Barrels { at: 1, from, to });
        press(w, KeyCode::Enter);
        assert_eq!(
            w.resource::<Encounter>()
                .main_mount(Station::Bow)
                .layout()
                .count(),
            tubes
        );
    }
    let aft = w.resource::<Campaign>().ship.design().gun_stations()[1]
        .berth
        .module;
    visit(w, aft);
    choose(
        w,
        DockJob::Barrels {
            at: aft,
            from: TurretLayout::Single,
            to: TurretLayout::Twin,
        },
    );
    press(w, KeyCode::Enter);
    assert_eq!(w.resource::<Campaign>().points(), 0);
    // She takes them to sea: Enter builds the refit, and Enter sails.
    press(w, KeyCode::Enter);
    press(w, KeyCode::Enter);
    press(w, KeyCode::Digit1);
    w.run_schedule(Update);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Sailing);
    assert_eq!(
        w.resource::<Encounter>()
            .main_mount(Station::Bow)
            .layout()
            .count(),
        3
    );
    let tubes = 3 + 2;
    assert_eq!(
        w.query_filtered::<&Visibility, With<scene::GunTubes>>()
            .iter(w)
            .filter(|&&v| v == Visibility::Inherited)
            .count(),
        tubes
    );
    assert_eq!(w.query::<&gun_sight::GunPip>().iter(w).count(), tubes);
    assert_eq!(w.query::<&gunnery::PlanBarrel>().iter(w).count(), tubes);
    let summary = format!(" / {tubes} ON TARGET");
    assert!(
        w.query::<&Text>()
            .iter(w)
            .any(|text| text.ends_with(&summary))
    );
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
    // Twin turrets, refitted in the yard, so more than one barrel bears at once.
    w.resource_scope(|w, mut run: Mut<Campaign>| {
        for gun in run.ship.design().gun_stations() {
            let house = shipvivor_shipyard::Gunhouse {
                barrels: shipvivor_encounter::TurretLayout::Twin,
                ..gun.gunhouse
            };
            run.ship.rearm(gun.berth.module, house).unwrap();
        }
        run.dock_revision += 1;
        w.resource_mut::<Encounter>().fit_player(&run);
    });
    w.run_schedule(Update);
    press(w, KeyCode::KeyX);
    // Lay her stopped and beam-on to the pinned ship, so both ends of her battery can
    // bear, and give the gunhouses time to train round.
    let pinned = w
        .resource::<ShipView>()
        .target(w.resource::<Encounter>())
        .map(|ship| ship.body.pos)
        .expect("X pins a ship");
    {
        let mut helm = w.resource_mut::<PlayerHelm>();
        let towards = pinned - helm.position;
        helm.heading = shipvivor_compass::heading(towards.perp());
        let notch = helm.notch();
        helm.ring(-isize::from(notch));
        helm.speed = 0.;
    }
    assert_eq!(
        wait(w, 10.),
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
fn caps_need_no_key_and_the_secondary_hand_mounts_more_wing_guns() {
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
    // She is built with a gun a side amidships and another on the bow.
    assert_eq!(shown_wing_guns(w), 4);
    // There is no filling to choose: every round is the one the guns are fitted for.
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
    // Four levels: piercing caps, more secondaries and quick loaders twice.
    for card in [
        shipvivor_campaign::Card::Caps,
        shipvivor_campaign::Card::MoreSecondaries,
        shipvivor_campaign::Card::Loader,
        shipvivor_campaign::Card::Loader,
    ] {
        pick(w, card);
    }
    let fittings = w.resource::<Campaign>().fittings;
    assert_eq!(fittings.caps, 1);
    assert_eq!(fittings.secondaries().per_side, 3);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Sailing);
    assert_eq!(w.resource::<Encounter>().wing_guns().len(), 6);
    w.run_schedule(Update);
    assert_eq!(shown_wing_guns(w), 6);

    w.run_schedule(Update);
    let shown = texts(w);
    assert!(
        !shown
            .iter()
            .any(|t| t.contains("LOAD") || t.contains("HE / AP"))
    );
}

#[test]
fn digits_pick_out_a_turret_and_z_steps_it_through_manual_scope_and_auto() {
    use shipvivor_encounter::Control;
    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Sailing);
    // The 2 that chose the narrows did not pick out the second turret.
    assert_eq!(w.resource::<Guns>().selected, None);
    let texts = |w: &mut World| -> Vec<String> {
        w.query::<&Text>().iter(w).map(|t| t.0.clone()).collect()
    };
    let controls = |w: &World| w.resource::<Guns>().controls;
    w.run_schedule(Update);
    let board = texts(w);
    assert_eq!(board.iter().filter(|t| *t == "MANUAL").count(), 2, "{board:?}");
    // Each turret's number stands on the plan and over its barrels.
    for number in ["1", "2"] {
        assert!(board.iter().filter(|t| *t == number).count() >= 2, "{board:?}");
    }

    // With no turret picked out, Z steps them all.
    press(w, KeyCode::KeyZ);
    assert_eq!(controls(w)[..2], [Control::Scope; 2]);
    press(w, KeyCode::Digit2);
    assert_eq!(w.resource::<Guns>().selected, Some(1));
    press(w, KeyCode::KeyZ);
    assert_eq!(controls(w)[..2], [Control::Scope, Control::Auto]);
    // She has no third turret, and the same digit again lets the second go.
    press(w, KeyCode::Digit3);
    assert_eq!(w.resource::<Guns>().selected, Some(1));
    press(w, KeyCode::Digit2);
    assert_eq!(w.resource::<Guns>().selected, None);

    // The battle takes them on its next step. With the scope down the crew has both.
    w.run_schedule(FixedUpdate);
    let battle = w.resource::<Encounter>();
    let numbered = battle.numbered();
    assert!(numbered.iter().all(|&s| battle.main_mount(s).crewed()));
    assert_eq!(battle.main_mount(numbered[1]).control(), Control::Auto);
    w.run_schedule(Update);
    let board = texts(w);
    for words in ["SCOPE (AUTO)", "AUTO", "ALL AUTO"] {
        assert!(board.iter().any(|t| t == words), "{words}: {board:?}");
    }
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

    use shipvivor_campaign::{DockJob, DockModule};
    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    let stock = geometry(w);
    let stock_turn = w.resource::<Campaign>().ship.design().turn_rate();
    // Three stars and four wrecked yards pay ten points, cleared past the clock.
    w.resource_mut::<Campaign>().elapsed = PAST_THE_CLOCK;
    w.resource_mut::<Campaign>().tick_sector(
        1.,
        Objectives {
            kills: 4,
            destroyed_drydock: true,
            crates: 2,
            yards: 4,
            ..default()
        },
        0,
        true,
    );
    w.run_schedule(Update);
    assert_eq!(w.resource::<Campaign>().points(), 10);
    assert!(w.query::<&Text>().iter(w).any(|t| t.as_str() == "30 m"));
    // A reserve bay and an auxiliary engine bought ahead of the after turret take seven
    // of them and bring her to 40 m. Each is spliced in as it is dropped, and built with
    // the next Enter.
    for module in [DockModule::Reserve, DockModule::EngineRoom] {
        let at = w.resource::<Campaign>().default_gap();
        draft(w, DockJob::Insert { at, module });
        // The new bay is selected as soon as it is ordered.
        assert_eq!(w.resource::<Campaign>().dock_module, at);
        press(w, KeyCode::Enter);
        assert!(w.resource::<Campaign>().plan().is_none());
    }
    assert_eq!(w.resource::<Campaign>().points(), 3);
    assert!(w.query::<&Text>().iter(w).any(|t| t.as_str() == "40 m"));
    // An escort's beam runs to eight sections, so a third bay waits for a wider hull.
    let reserve = DockJob::Insert {
        at: w.resource::<Campaign>().default_gap(),
        module: DockModule::Reserve,
    };
    draft(w, reserve);
    assert!(w.resource::<Campaign>().plan().is_none());
    assert_eq!(w.resource::<Campaign>().ship.design().length(), 40.);
    assert_eq!(w.resource::<Campaign>().points(), 3);
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
    // A turbine refit on the engine bay spends two more. Both sections and the turbines
    // persist across departure, and so does the point left over.
    visit(w, 3);
    choose(w, DockJob::Fitting(shipvivor_campaign::Card::Turbines));
    press(w, KeyCode::Enter);
    // Enter builds it, and the next sails with the point left over banked.
    press(w, KeyCode::Enter);
    press(w, KeyCode::Enter);
    let moderate = w.resource::<Campaign>().exits().unwrap()[0].stage;
    press(w, KeyCode::Digit1);
    assert_eq!(w.resource::<Campaign>().stage, moderate);
    assert_eq!(w.resource::<ShipView>().hull_extension, 10.);
    assert_eq!(w.query::<&damage_control::Cell>().iter(w).count(), 8);
    assert_eq!(w.resource::<Campaign>().fittings.turbines, 1);
    assert_eq!(w.resource::<Campaign>().points(), 1);
    let design = w.resource::<Campaign>().ship.design();
    assert!(design.top_speed() > shipvivor_navigation::Helm::FULL_AHEAD);
    assert!(design.turn_rate() < stock_turn);
    assert!(w.query::<&Text>().iter(w).any(|t| t.as_str() == "RES"));

    press(w, KeyCode::Escape);
    press(w, KeyCode::KeyM);
    start_in_narrows(w);
    assert_eq!(geometry(w), stock);
    assert_eq!(w.query::<&damage_control::Cell>().iter(w).count(), 6);
}

#[test]
fn the_next_base_keeps_its_flag_on_the_screen_edge_when_it_lies_astern() {
    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    let base = mission_hud::anchor(&w.resource::<Encounter>().ports()[0]);
    let out = w.resource::<Encounter>().ports()[0].body.forward();
    let flags = |w: &mut World| -> Vec<String> {
        w.query::<(&mission_hud::WorldFlag, &Text, &Node)>()
            .iter(w)
            .filter(|(_, _, node)| node.display != Display::None)
            .map(|(_, text, _)| text.0.clone())
            .collect()
    };
    let look = |w: &mut World, bearing: f32| {
        let mut helm = w.resource_mut::<PlayerHelm>();
        helm.position = base + out * 450.;
        helm.heading = bearing;
        w.resource_mut::<crate::camera::ShipView>().bearing = bearing;
        w.run_schedule(Update);
    };
    // Facing her, the flag rides over the base and says it is next.
    look(w, shipvivor_compass::heading(-out));
    let ahead = flags(w);
    assert!(ahead.iter().any(|t| t.starts_with("NEXT  A")), "{ahead:?}");
    // Facing away, it waits on the edge of the screen and points the way to turn.
    look(w, shipvivor_compass::heading(out));
    let astern = flags(w);
    assert!(
        astern
            .iter()
            .any(|t| (t.starts_with("< NEXT A") || t.ends_with(" >")) && t.contains("NEXT A")),
        "{astern:?}"
    );
}

#[test]
fn mission_clock_and_industry_bearings_remain_visible_in_the_last_minute() {
    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    let clock = f32::from(w.resource::<Campaign>().stage.mission().minutes) * 60.;
    w.resource_mut::<Campaign>().elapsed = clock - 59.8;
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
    // The board bears on the bases the chart shows that still stand. At first that is
    // A alone: the bases further up the channel sleep.
    let shown = |w: &mut World| {
        w.query::<(&mission_hud::Reading, &Text, &Node)>()
            .iter(w)
            .filter(|(kind, _, node)| {
                matches!(kind, mission_hud::Reading::Bearing(_)) && node.display != Display::None
            })
            .map(|(_, text, _)| text.0.clone())
            .collect::<Vec<_>>()
    };
    let first = shown(w);
    assert!(
        first.len() == 1 && first[0].starts_with('A') && first[0].ends_with("1/1 factory"),
        "{first:?}"
    );
    // A falls, and B stirs further up the channel.
    destroy_drydock(&mut w.resource_mut::<Encounter>(), 0);
    w.run_schedule(FixedUpdate);
    w.run_schedule(Update);
    let next = shown(w);
    assert!(
        next.len() == 1 && next[0].starts_with('B') && next[0].ends_with("2/2 factories"),
        "{next:?}"
    );
    let paused_at = w.resource::<Campaign>().elapsed;
    press(w, KeyCode::Escape);
    assert_eq!(w.query::<&mission_hud::Reading>().iter(w).count(), 0);
    for _ in 0..60 {
        w.run_schedule(FixedUpdate);
    }
    press(w, KeyCode::Escape);
    assert_eq!(w.resource::<Campaign>().elapsed, paused_at);
}

#[test]
fn the_battle_mounts_every_turret_the_heaviest_hull_carries() {
    assert_eq!(
        shipvivor_encounter::Station::SLOTS.len(),
        shipvivor_campaign::MOST_GUN_STATIONS
    );
    assert!(shipvivor_campaign::MOST_GUN_STATIONS <= crate::state::TURRETS);
}

#[test]
fn yard_controls_take_a_mixed_battery_to_sea() {
    use shipvivor_campaign::{DeckModule, DockJob, DockModule};

    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    // Three stars, and wrecked yards enough to pay for the whole programme: two bays, two
    // classes of hull, the wings and a bore for each turret. Cleared past the clock.
    w.resource_mut::<Campaign>().elapsed = PAST_THE_CLOCK;
    w.resource_mut::<Campaign>().tick_sector(
        1.,
        Objectives {
            kills: 4,
            destroyed_drydock: true,
            crates: 8,
            yards: 51,
            ..default()
        },
        0,
        true,
    );
    w.run_schedule(Update);
    // The two levels the stores earned are dealt as hands before the yard opens.
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Refit);
    assert_eq!(w.resource::<Campaign>().fittings_remaining(), 2);
    press(w, KeyCode::Digit1);
    press(w, KeyCode::Digit1);
    assert_eq!(
        w.resource::<Campaign>().screen(),
        Screen::Drydock(LegEnd::BasesDestroyed)
    );
    assert_eq!(w.resource::<Campaign>().points(), 57);
    // With no bay to stand one on, the yard opens on the turret a split would add to, and
    // shows nothing until a refit is picked out.
    assert_eq!(w.resource::<Campaign>().dock_module, 1);
    assert_eq!(w.resource::<Campaign>().dock_job(), None);
    // Two reserve bays from the stock, each ordered as it is dropped and selected. A
    // centre turret heads its list.
    for _ in 0..2 {
        let at = w.resource::<Campaign>().default_gap();
        draft(
            w,
            DockJob::Insert {
                at,
                module: DockModule::Reserve,
            },
        );
        assert_eq!(w.resource::<Campaign>().dock_module, at);
        assert_eq!(
            w.resource::<Campaign>().dock_jobs()[0],
            DockJob::Replace {
                at,
                with: DeckModule::CentreBattery
            }
        );
    }
    assert_eq!(w.resource::<Campaign>().points(), 51);
    // Both bays are on order, listed under the sections they make.
    assert_eq!(w.query::<&dockyard::Ledger>().iter(w).count(), 2);

    // The bore is chosen on a turret, and too large a bore waits for a wider hull.
    visit(w, 1);
    let huge = DockJob::Calibre {
        at: 1,
        from: 4,
        to: 18,
    };
    choose(w, huge);
    press(w, KeyCode::Enter);
    assert_eq!(w.resource::<Campaign>().points(), 51);
    assert_eq!(
        w.resource::<Campaign>().fittings.round().calibre_inches(),
        4.
    );

    // The hull is rebuilt from the hull panel, a class at a time, at once: a heavy
    // cruiser's, then a battleship's.
    for _ in 0..2 {
        let beam = w.resource::<Campaign>().ship.design().beam();
        draft(w, DockJob::WidenBeam);
        assert!(w.resource::<Campaign>().ship.design().beam() > beam);
        assert_eq!(w.resource::<Campaign>().dock_job(), None);
    }
    assert_eq!(w.resource::<Campaign>().ship.design().beam(), 9.5);

    visit(w, 1);
    choose(w, DockJob::Move { from: 1, to: 2 });
    press(w, KeyCode::Enter);
    assert_eq!(w.resource::<Campaign>().dock_module, 2);
    // Two classes of hull cost twenty-four, and the move nothing.
    assert_eq!(w.resource::<Campaign>().points(), 27);
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
    let turrets = |w: &World| w.resource::<Campaign>().ship.design().gun_stations().len();
    assert_eq!(turrets(w), 2);
    press(w, KeyCode::Enter);
    assert_eq!(turrets(w), 3);
    // The wing pair takes no more than an eight-inch bore, and the centre turret aft all
    // eighteen inches.
    choose(
        w,
        DockJob::Calibre {
            at: 2,
            from: 4,
            to: 8,
        },
    );
    press(w, KeyCode::Enter);
    let aft = w.resource::<Campaign>().ship.design().gun_stations()[2]
        .berth
        .module;
    visit(w, aft);
    choose(
        w,
        DockJob::Calibre {
            at: aft,
            from: 4,
            to: 18,
        },
    );
    // The last points are spent at once. The next press builds the orders, and the one
    // after sails.
    press(w, KeyCode::Enter);
    assert_eq!(w.resource::<Campaign>().points(), 0);
    assert!(w.resource::<Campaign>().plan().is_some());
    press(w, KeyCode::Enter);
    assert!(w.resource::<Campaign>().plan().is_none());
    assert!(matches!(
        w.resource::<Campaign>().screen(),
        Screen::Drydock(_)
    ));
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
fn a_lean_yard_adds_a_turret_that_stays_fitted_until_she_sails() {
    use shipvivor_campaign::{DeckModule, DockJob};

    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    // Cleared late with only the drydock goal met: one star pays two points, and the
    // clock nothing.
    w.resource_mut::<Campaign>()
        .tick_sector(600., Objectives::default(), 0, true);
    w.run_schedule(Update);
    assert_eq!(w.resource::<Campaign>().points(), 2);
    assert!(w.query::<&Text>().iter(w).any(|t| t.0 == "2 points"));

    // With no reserve bay, the yard opens on the forward turret. Splitting it into wing
    // turrets is how two points add a turret.
    assert_eq!(w.resource::<Campaign>().dock_module, 1);
    assert_eq!(w.resource::<Campaign>().dock_job(), None);
    let split = DockJob::Replace {
        at: 1,
        with: DeckModule::WingBattery,
    };
    choose(w, split);
    // The price is on the figures, and the last points are spent on the first press.
    assert!(w.query::<&Text>().iter(w).any(|t| t.0 == "2"));
    press(w, KeyCode::Enter);
    assert_eq!(w.resource::<Campaign>().points(), 0);
    // The order is listed under its section, and a click there takes it back.
    assert_eq!(w.query::<&dockyard::Ledger>().iter(w).count(), 1);
    w.run_system_cached_with(apply_action, Some(ui::Action::Undo(0)))
        .unwrap();
    w.run_schedule(Update);
    assert_eq!(w.resource::<Campaign>().points(), 2);
    assert!(w.resource::<Campaign>().plan().is_none());
    choose(w, split);
    press(w, KeyCode::Enter);
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
    // Enter locks in the order, the purse over the big button is spent and the button
    // turns to Sail, and Enter sails.
    assert!(w.query::<&Text>().iter(w).any(|t| t.0 == "LOCK IN"));
    press(w, KeyCode::Enter);
    assert!(w.query::<&Text>().iter(w).any(|t| t.0 == "0 points"));
    assert!(w.query::<&Text>().iter(w).any(|t| t.0 == "SAIL"));
    press(w, KeyCode::Enter);
    let run = w.resource::<Campaign>();
    assert_eq!(run.screen(), Screen::Chart);
    assert_eq!(run.ship.design().gun_stations().len(), 3);
}

#[test]
fn a_click_orders_a_refit_at_once_and_books_its_points() {
    use shipvivor_campaign::{Card, DeckModule, DockJob};
    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    // The raider star, the drydock star and one wrecked yard pay five points.
    w.resource_mut::<Campaign>().tick(
        1.,
        Objectives {
            kills: 4,
            yards: 1,
            ..default()
        },
        0,
    );
    clear_leg(&mut w.resource_mut::<Campaign>());
    w.run_schedule(Update);
    let act = |w: &mut World, action: ui::Action| {
        w.run_system_cached_with(apply_action, Some(action))
            .unwrap();
        w.run_schedule(Update);
    };
    let split = DockJob::Replace {
        at: 1,
        with: DeckModule::WingBattery,
    };
    visit(w, 1);
    let index = w
        .resource::<Campaign>()
        .dock_jobs()
        .iter()
        .position(|&job| job == split)
        .unwrap();
    let clickable = |w: &mut World| {
        w.query::<&ui::Action>()
            .iter(w)
            .copied()
            .collect::<Vec<_>>()
    };
    assert!(clickable(w).contains(&ui::Action::Order(index)));
    // One click orders the split and books its points, with nothing picked out, and its
    // figures stay up to say what it did.
    act(w, ui::Action::Order(index));
    assert_eq!(w.resource::<Campaign>().points(), 3);
    assert_eq!(w.resource::<Campaign>().dock_job(), None);
    assert_eq!(
        w.resource::<Campaign>().plan().map(|p| p.orders().len()),
        Some(1)
    );
    let texts = |w: &mut World| {
        w.query::<&Text>()
            .iter(w)
            .map(|t| t.0.clone())
            .collect::<Vec<_>>()
    };
    assert!(texts(w).iter().any(|t| t == "Split into wing turrets"));
    assert!(texts(w).iter().any(|t| t == "  On order"));
    // Hull work is ordered at once the same way, from the hull panel.
    let armor = DockJob::Fitting(Card::Armor);
    act(w, ui::Action::Draft(armor));
    assert_eq!(w.resource::<Campaign>().points(), 1);
    assert_eq!(
        w.resource::<Campaign>().plan().map(|p| p.orders().len()),
        Some(2)
    );
    // Fittings the last point cannot cover take no click, here or on the section. Merging
    // the wings back does, for it takes back the split and gives its points back.
    let run = w.resource::<Campaign>();
    let short: Vec<_> = run
        .dock_jobs()
        .into_iter()
        .enumerate()
        .filter(|&(_, job)| {
            matches!(job, DockJob::Fitting(_)) && job.fits(run) && run.points_after(job).is_none()
        })
        .map(|(i, _)| ui::Action::Order(i))
        .collect();
    assert!(!short.is_empty());
    let merge = run
        .dock_jobs()
        .iter()
        .position(|&job| {
            job == DockJob::Replace {
                at: 1,
                with: DeckModule::CentreBattery,
            }
        })
        .map(ui::Action::Order)
        .unwrap();
    let actions = clickable(w);
    assert!(!actions.contains(&ui::Action::Draft(armor)));
    assert!(short.iter().all(|action| !actions.contains(action)));
    assert!(actions.contains(&merge));
    // A click on the order under the section takes it back, and its points with it.
    act(w, ui::Action::Undo(0));
    assert_eq!(w.resource::<Campaign>().points(), 3);
}

#[test]
fn six_inch_guns_then_twin_barrels_then_lock_in() {
    use shipvivor_campaign::DockJob;
    use shipvivor_encounter::TurretLayout;
    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    // Both stars and five wrecked yards pay fourteen points.
    w.resource_mut::<Campaign>().tick(
        1.,
        Objectives {
            kills: 4,
            yards: 5,
            ..default()
        },
        0,
    );
    clear_leg(&mut w.resource_mut::<Campaign>());
    w.run_schedule(Update);
    while w.resource::<Campaign>().screen() == Screen::Refit {
        press(w, KeyCode::Digit1);
    }
    assert!(matches!(
        w.resource::<Campaign>().screen(),
        Screen::Drydock(_)
    ));
    let act = |w: &mut World, action: ui::Action| {
        w.run_system_cached_with(apply_action, Some(action))
            .unwrap();
        w.run_schedule(Update);
    };
    let tile = |w: &World, wanted: fn(&DockJob) -> bool| {
        w.resource::<Campaign>()
            .dock_jobs()
            .iter()
            .position(wanted)
            .map(ui::Action::Order)
            .expect("the forward turret offers it")
    };
    visit(w, 1);
    let points = w.resource::<Campaign>().points();
    let six = tile(w, |job| matches!(job, DockJob::Calibre { to: 6, .. }));
    act(w, six);
    let twin = tile(w, |job| {
        matches!(
            job,
            DockJob::Barrels {
                to: TurretLayout::Twin,
                ..
            }
        )
    });
    act(w, twin);
    // Two clicks, two orders, their points booked, and nothing waiting on a second press.
    let run = w.resource::<Campaign>();
    let booked = run.plan().map_or(0, |plan| plan.cost());
    assert_eq!(run.plan().map(|plan| plan.orders().len()), Some(2));
    assert!(booked > 0);
    assert_eq!(run.points(), points - booked);
    assert_eq!(run.dock_job(), None);
    // Lock in builds them.
    act(w, ui::Action::Sail);
    let run = w.resource::<Campaign>();
    assert!(run.plan().is_none());
    assert!(matches!(run.screen(), Screen::Drydock(_)));
    let house = run.ship.design().modules()[1].gunhouse;
    assert_eq!(house.bore, 6);
    assert_eq!(house.barrels, TurretLayout::Twin);
    assert_eq!(run.points(), points - booked);
}

#[test]
fn the_big_button_locks_in_and_sails_whatever_refit_is_picked_out() {
    use shipvivor_campaign::DockJob;
    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    // The raider star, the drydock star and one wrecked yard pay five points.
    w.resource_mut::<Campaign>().tick(
        1.,
        Objectives {
            kills: 4,
            yards: 1,
            ..default()
        },
        0,
    );
    clear_leg(&mut w.resource_mut::<Campaign>());
    w.run_schedule(Update);
    let act = |w: &mut World, action: ui::Action| {
        w.run_system_cached_with(apply_action, Some(action))
            .unwrap();
        w.run_schedule(Update);
    };
    // A loader is ordered, and an 18-inch bore the hull cannot carry picked out after it.
    visit(w, 1);
    choose(w, DockJob::Fitting(shipvivor_campaign::Card::Loader));
    press(w, KeyCode::Enter);
    let huge = DockJob::Calibre {
        at: 1,
        from: 4,
        to: 18,
    };
    choose(w, huge);
    assert!(!huge.fits(w.resource::<Campaign>()));
    // The points left, with the order's price, stand over the big button.
    assert!(w.query::<&Text>().iter(w).any(|t| t.0 == "3 points"));
    assert!(w.query::<&Text>().iter(w).any(|t| t.0 == "(2 on order)"));
    // The button locks in the order, though the bore is still picked out.
    act(w, ui::Action::Sail);
    assert!(w.resource::<Campaign>().plan().is_none());
    assert_eq!(w.resource::<Campaign>().fittings.loader, 1);
    // It then shows the points left over as banked for the next yard.
    assert!(w.query::<&Text>().iter(w).any(|t| t.0 == "3 points banked"));
    // Enter on the bore she cannot carry leaves her in dock, and the button sails her.
    choose(w, huge);
    press(w, KeyCode::Enter);
    assert!(matches!(
        w.resource::<Campaign>().screen(),
        Screen::Drydock(_)
    ));
    act(w, ui::Action::Sail);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Chart);
    assert_eq!(w.resource::<Campaign>().points(), 3);
}

#[test]
fn points_left_in_the_yard_are_banked_for_the_next_and_one_press_sails() {
    use shipvivor_campaign::DockJob;
    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    // The raider star, the drydock star and one wrecked yard pay five points.
    w.resource_mut::<Campaign>().tick(
        1.,
        Objectives {
            kills: 4,
            yards: 1,
            ..default()
        },
        0,
    );
    clear_leg(&mut w.resource_mut::<Campaign>());
    w.run_schedule(Update);
    assert_eq!(w.resource::<Campaign>().points(), 5);
    // The Sail button says the points left over are banked, and one press sails.
    assert!(w.query::<&Text>().iter(w).any(|t| t.0 == "5 points banked"));
    w.run_system_cached_with(apply_action, Some(ui::Action::Sail))
        .unwrap();
    w.run_schedule(Update);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Chart);
    assert_eq!(w.resource::<Campaign>().points(), 5);
    // They sail with her, and the next yard adds what the next leg pays to them.
    press(w, KeyCode::Digit1);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Sailing);
    assert_eq!(w.resource::<Campaign>().points(), 5);
    clear_leg(&mut w.resource_mut::<Campaign>());
    w.run_schedule(Update);
    let run = w.resource::<Campaign>();
    assert!(matches!(run.screen(), Screen::Drydock(_)));
    assert!(run.leg_points() > 0);
    assert_eq!(run.points(), 5 + run.leg_points());

    // With nothing left to spend, one press sails. Cleared late with only the drydock
    // star, she has two points for the loader.
    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    w.resource_mut::<Campaign>()
        .tick_sector(600., Objectives::default(), 0, true);
    w.run_schedule(Update);
    visit(w, 1);
    choose(w, DockJob::Fitting(shipvivor_campaign::Card::Loader));
    press(w, KeyCode::Enter);
    assert_eq!(w.resource::<Campaign>().points(), 0);
    // One press builds it, and the next sails.
    press(w, KeyCode::Enter);
    press(w, KeyCode::Enter);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Chart);
}

#[test]
fn work_that_waits_on_other_work_beckons_it_step_by_step() {
    use shipvivor_campaign::DockJob;
    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    clear_leg(&mut w.resource_mut::<Campaign>());
    w.run_schedule(Update);
    let beckoned = |w: &mut World| {
        w.query_filtered::<Option<&ui::Action>, With<dockyard::Beckon>>()
            .iter(w)
            .map(|action| action.copied())
            .collect::<Vec<_>>()
    };
    assert!(beckoned(w).is_empty());
    // An 8-inch bore is too heavy for the forward turret on an escort's beam. The broader
    // beam it waits on cannot be ordered yet either, for it waits on a longer hull, so a
    // section from the stock beckons: the step she can take first, and nothing else.
    visit(w, 1);
    let bore = w.resource::<Campaign>().ship.design().modules()[1]
        .gunhouse
        .bore;
    let eight = DockJob::Calibre {
        at: 1,
        from: bore,
        to: 8,
    };
    choose(w, eight);
    assert!(!eight.fits(w.resource::<Campaign>()));
    assert!(w.resource::<Campaign>().widen_refusal().is_some());
    assert_eq!(beckoned(w), [None]);
    assert_eq!(
        w.query_filtered::<(), (With<dockyard::Beckon>, With<dockyard::StockTile>)>()
            .iter(w)
            .count(),
        1
    );
    // The bore she cannot carry takes no click.
    let index = w
        .resource::<Campaign>()
        .dock_jobs()
        .iter()
        .position(|&job| job == eight)
        .unwrap();
    assert!(
        !w.query::<&ui::Action>()
            .iter(w)
            .any(|&action| action == ui::Action::Order(index))
    );
    // Work the ship can take beckons nothing.
    press(w, KeyCode::Escape);
    choose(w, DockJob::Fitting(shipvivor_campaign::Card::Loader));
    assert!(beckoned(w).is_empty());
}

#[test]
fn pumps_come_in_a_hand_or_from_the_engine_bay_and_reach_the_ship() {
    use shipvivor_campaign::{Card, DockJob};
    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    // The card says how far the crew's pumps hold a holed module dry: down to 31% of its
    // structure on the stock pumps, and 21% with half as much again.
    let run = w.resource::<Campaign>();
    let face = cards::face(Card::Pumps, run.fittings, run.ship.design());
    let figures: Vec<_> = face
        .stats
        .iter()
        .map(|s| (&*s.label, s.before.as_str(), s.after.as_str()))
        .collect();
    assert_eq!(
        figures,
        [("Pumping", "100%", "150%"), ("Holds dry to", "31%", "21%")]
    );
    // A level deals them, and the first rank reaches her pumps at once.
    w.resource_mut::<Campaign>().experience = 30;
    w.run_schedule(FixedUpdate);
    w.run_schedule(Update);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Refit);
    pick(w, Card::Pumps);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Sailing);
    assert_eq!(w.resource::<Campaign>().ship.design().pumps(), 1.5);

    // The engine bay sells the next rank, previewed with the same figures.
    w.resource_mut::<Campaign>().tick(
        0.,
        Objectives {
            kills: 4,
            yards: 1,
            ..default()
        },
        0,
    );
    clear_leg(&mut w.resource_mut::<Campaign>());
    w.run_schedule(Update);
    visit(w, 3);
    choose(w, DockJob::Fitting(Card::Pumps));
    for figure in ["150%", "200%", "21%", "13%"] {
        assert!(
            w.query::<&Text>().iter(w).any(|t| t.0 == figure),
            "{figure}"
        );
    }
    press(w, KeyCode::Enter);
    let run = w.resource::<Campaign>();
    assert_eq!((run.fittings.pumps, run.ship.design().pumps()), (2, 2.));
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

#[test]
fn every_enemy_class_has_a_hull_model_and_a_gun_model() {
    use bevy::camera::primitives::MeshAabb;
    for class in shipvivor_encounter::EnemyClass::ALL {
        let deckwork = fleet_models::superstructure(class);
        assert!(deckwork.get_aabb().is_some(), "{class:?} has no deckwork");
        // Every weight's turret, each with more barrels than the last.
        let guns = [1, 2, 3, 4].map(|barrels| fleet_models::gun(class, barrels).count_vertices());
        assert!(guns[0] > 0, "{class:?} has no gun");
        assert!(guns.is_sorted_by(|a, b| a < b), "{class:?}: {guns:?}");
    }
}

#[test]
fn the_spotting_top_trains_round_after_the_sight_and_lies_fore_and_aft_in_the_dock() {
    use std::f32::consts::{FRAC_PI_2, TAU};
    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    let top = |w: &mut World| {
        let (top, pose) = w
            .query::<(&scene::SpottingTop, &Transform)>()
            .single(w)
            .unwrap();
        (top.train(), pose.rotation)
    };
    let tick = |w: &mut World| {
        w.resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(100));
        w.run_schedule(Update);
    };
    let heading = w.resource::<PlayerHelm>().heading;
    w.resource_mut::<ShipView>().bearing = heading;
    for _ in 0..25 {
        tick(w);
    }
    assert!(turned(top(w).0, 0.).abs() < 1e-3);

    // The sight thrown out to the starboard beam: the top swings after it, and arrives.
    w.resource_mut::<ShipView>().bearing = heading + FRAC_PI_2;
    tick(w);
    let (partway, _) = top(w);
    assert!(partway > 0.1 && partway < 1., "trained {partway} in a tick");
    for _ in 0..10 {
        tick(w);
    }
    let (train, rotation) = top(w);
    assert!(turned(train, FRAC_PI_2).abs() < 1e-3, "trained {train}");
    // Its slit, which faces the bow at rest, now looks out along the camera's own line.
    let hull = w
        .query_filtered::<&Transform, With<scene::Ship>>()
        .single(w)
        .unwrap()
        .rotation;
    let lens = w
        .query_filtered::<&Transform, With<camera::FollowCamera>>()
        .single(w)
        .unwrap()
        .forward();
    let slit = (hull * rotation * Vec3::NEG_Z).xz().normalize();
    let sight = lens.xz().normalize();
    assert!(slit.dot(sight) > 0.999, "slit faces {slit}, sight {sight}");

    // Across the bow to the port bow it goes the short way, not round by the stern.
    w.resource_mut::<ShipView>().bearing = heading - 0.5;
    tick(w);
    let (train, _) = top(w);
    assert!(turned(0., train) > 0. && turned(0., train) < FRAC_PI_2);
    for _ in 0..10 {
        tick(w);
    }
    assert!(turned(top(w).0, TAU - 0.5).abs() < 1e-3);

    // The ship turning under a steady sight turns the top on its tower.
    w.resource_mut::<PlayerHelm>().heading = heading - 0.5;
    for _ in 0..5 {
        tick(w);
    }
    assert!(turned(top(w).0, 0.).abs() < 1e-3);

    // In the dock it is found fore and aft at once, wherever the sight was left.
    w.resource_mut::<ShipView>().bearing = heading + 2.;
    for _ in 0..10 {
        tick(w);
    }
    assert!(turned(top(w).0, 2.5).abs() < 1e-3);
    clear_leg(&mut w.resource_mut::<Campaign>());
    w.run_schedule(Update);
    assert!(matches!(
        w.resource::<Campaign>().screen(),
        Screen::Drydock(_)
    ));
    w.run_schedule(Update);
    let (train, rotation) = top(w);
    assert_eq!(train, 0.);
    assert_eq!(rotation, Quat::IDENTITY);
}

#[test]
fn test_mode_sails_any_sea_from_a_free_yard_and_comes_back_to_it() {
    use shipvivor_campaign::{Card, DockJob, Grade};
    let mut app = headless_game();
    let w = app.world_mut();
    let act = |w: &mut World, action: ui::Action| {
        w.run_system_cached_with(apply_action, Some(action))
            .unwrap();
        w.run_schedule(Update);
    };
    let seas = |w: &mut World| {
        (w.query::<&ui::Action>().iter(w))
            .filter(|a| matches!(a, ui::Action::TestSea(_)))
            .count()
    };
    let shows = |w: &mut World, words: &str| w.query::<&Text>().iter(w).any(|t| t.contains(words));

    // X on the title lists the seas of a round: nine in the first, twelve in the
    // sixth, and Crown Deep alone in the seventh. G changes the grade.
    press(w, KeyCode::KeyX);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Exercise);
    assert_eq!(seas(w), 9);
    assert!(shows(w, "Par for round 1: a stock ship"));
    assert!(shows(w, "5 Red Shift") && shows(w, "6 Bell Works"));
    assert!(shows(w, "ship models in any round"));
    assert!(!shows(w, "Every sea ends at"));
    press(w, KeyCode::Digit5);
    assert_eq!(seas(w), 12);
    assert!(shows(w, "Every sea ends at The Red Shift"));
    assert!(shows(w, "ore hopper"));
    press(w, KeyCode::Digit7);
    assert_eq!(seas(w), 1);
    assert!(!shows(w, "Every sea ends at"));
    assert!(shows(w, "Cruisers and destroyers screen the Iron Regent"));
    press(w, KeyCode::Digit6);
    assert_eq!(seas(w), 12);
    assert!(shows(w, "Every sea ends at The Bell Works"));
    press(w, KeyCode::KeyG);
    assert_eq!(w.resource::<Campaign>().bench, (6, Grade::Moderate));
    assert!(shows(w, "steam hammer") && shows(w, "cliff guns"));
    press(w, KeyCode::KeyG);
    assert_eq!(w.resource::<Campaign>().bench, (6, Grade::Tough));
    let par = shipvivor_sim::Par::at(6, Grade::Tough);
    assert!(shows(w, &format!("about {} points of work", par.worth())));

    // A sea opens its yard: a stock ship at the round's usual level, nothing to spend
    // and nothing asked for, with par beside what her work has come to.
    let sea = Stage::BastionCut;
    act(w, ui::Action::TestSea(sea));
    let run = w.resource::<Campaign>();
    assert!(matches!(run.screen(), Screen::Drydock(_)));
    assert!(run.is_exercise());
    assert_eq!(
        (run.stage, run.grade, run.level),
        (sea, Grade::Tough, par.level)
    );
    assert_eq!((run.points(), run.spent()), (0, 0));
    let armor = DockJob::Fitting(Card::Armor);
    assert_eq!(run.quote(armor), Some(i32::from(armor.cost())));
    assert!(shows(w, "TEST YARD"));
    assert!(shows(
        w,
        &format!("Par for round 6, tough: {}", par.worth())
    ));
    assert!(shows(w, &format!("{} under", par.worth())));
    assert!(!shows(w, "RATE ROUND"));
    act(w, ui::Action::Draft(armor));
    let run = w.resource::<Campaign>();
    assert_eq!((run.points(), run.spent()), (0, armor.cost()));
    assert!(shows(w, &format!("{} spent", armor.cost())));
    assert!(shows(w, &format!("{} under", par.worth() - armor.cost())));

    // Lock in, then sail: straight into the sea, at its grade, under its air cover.
    act(w, ui::Action::Sail);
    assert!(matches!(
        w.resource::<Campaign>().screen(),
        Screen::Drydock(_)
    ));
    act(w, ui::Action::Sail);
    let run = w.resource::<Campaign>();
    assert_eq!((run.screen(), run.stage), (Screen::Sailing, sea));
    assert!(run.under_air_cover());
    let bases = shipvivor_sim::Sector::of(sea, Grade::Tough).bases();
    assert_eq!(w.resource::<Encounter>().ports().len(), bases);
    assert!(w.resource::<Encounter>().air_station().is_some());

    // Sunk, Enter goes back to the yard with the ship as she sailed.
    w.resource_mut::<Campaign>()
        .tick(30., Objectives::default(), 10_000);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Lost);
    w.run_schedule(Update);
    assert!(shows(w, "Back to the yard"));
    press(w, KeyCode::Enter);
    let run = w.resource::<Campaign>();
    assert!(matches!(run.screen(), Screen::Drydock(_)));
    assert!(run.ship.afloat());
    assert_eq!((run.hull_percent(), run.elapsed), (100, 0.));
    assert_eq!((run.fittings.armor, run.spent()), (1, armor.cost()));
    assert_eq!(run.debrief.as_deref(), Some("Sunk after 0:30, 0 stars"));
    assert!(shows(w, "Last sortie: Sunk after 0:30"));

    // Cleared, she comes back the same way, with nothing banked and no chart.
    act(w, ui::Action::Sail);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Sailing);
    w.resource_mut::<Campaign>().level += 3;
    clear_leg(&mut w.resource_mut::<Campaign>());
    w.run_schedule(Update);
    w.run_schedule(Update);
    let run = w.resource::<Campaign>();
    assert!(matches!(run.screen(), Screen::Drydock(_)));
    assert_eq!((run.points(), run.level, run.stars()), (0, par.level, 0));
    assert!(run.debrief.as_deref().unwrap().starts_with("Cleared in "));

    // A sortie can be broken off from the pause.
    act(w, ui::Action::Sail);
    press(w, KeyCode::Escape);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Paused);
    press(w, KeyCode::KeyY);
    let run = w.resource::<Campaign>();
    assert!(matches!(run.screen(), Screen::Drydock(_)));
    assert!(run.debrief.as_deref().unwrap().starts_with("Broken off"));

    // M goes back to the list with the ship kept, for another sea at another grade.
    press(w, KeyCode::KeyM);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Exercise);
    assert_eq!(w.resource::<Campaign>().bench, (6, Grade::Tough));
    press(w, KeyCode::Digit2);
    press(w, KeyCode::KeyG);
    act(w, ui::Action::TestSea(Stage::OuterRoad));
    let run = w.resource::<Campaign>();
    assert!(matches!(run.screen(), Screen::Drydock(_)));
    assert_eq!((run.stage, run.grade), (Stage::OuterRoad, Grade::Moderate));
    assert_eq!(run.level, shipvivor_sim::Par::at(2, Grade::Moderate).level);
    assert_eq!((run.fittings.armor, run.spent()), (1, armor.cost()));
    assert!(run.debrief.is_none());
    act(w, ui::Action::Sail);
    let run = w.resource::<Campaign>();
    assert_eq!(
        (run.screen(), run.stage),
        (Screen::Sailing, Stage::OuterRoad)
    );
    assert!(!run.under_air_cover());
    assert!(w.resource::<Encounter>().air_station().is_none());

    // Esc on the list gives the ship up for the title, and a run starts as it always did.
    press(w, KeyCode::Escape);
    press(w, KeyCode::KeyY);
    press(w, KeyCode::KeyM);
    press(w, KeyCode::Escape);
    let run = w.resource::<Campaign>();
    assert_eq!(run.screen(), Screen::Title);
    assert!(!run.is_exercise());
    start_in_narrows(w);
    let run = w.resource::<Campaign>();
    assert!(!run.is_exercise());
    assert_eq!((run.level, run.spent()), (1, 0));
}

/// The battle's timeline keeps the leg as it is sailed and marks each ship sunk. From the
/// pause menu it opens on an earlier moment, Esc takes it back to the present, and Enter
/// sails on from the moment picked, forgetting what came after. From the report of her
/// loss it takes her back afloat.
#[test]
fn the_timeline_takes_the_battle_back_and_sails_on_from_the_moment_picked() {
    use rewind::{MarkKind, STEP, Timeline};
    let mut app = headless_game();
    let w = app.world_mut();
    start_in_narrows(w);
    let sail = |w: &mut World, seconds: f32| {
        for _ in 0..(seconds * 60.).round() as usize {
            w.run_schedule(FixedUpdate);
        }
    };
    let at_sea = |w: &World| w.resource::<Encounter>().at_sea();
    sail(w, 20.);
    let (first, last) = w.resource::<Timeline>().span().expect("moments kept");
    assert!(first < STEP, "{first}");
    assert!(at_sea(w) - last < STEP + 0.02, "{last} vs {}", at_sea(w));
    let moments = w.resource::<Timeline>().moments();
    assert!((38..=42).contains(&moments), "{moments}");

    // A ship sunk by hand goes down on the next step, and the timeline marks it.
    let victim = w.resource::<Encounter>().raiders()[0].body.id;
    w.resource_mut::<Encounter>().dev_damage(victim, 1e6).unwrap();
    sail(w, 5.);
    let marks = w.resource::<Timeline>().marks().to_vec();
    assert_eq!(marks.len(), 1, "{marks:?}");
    assert!(matches!(marks[0].kind, MarkKind::Sunk(_)));
    assert!(w.resource::<Encounter>().ship(victim).is_none());

    // Paused, the timeline opens ten seconds back, before she went down.
    press(w, KeyCode::Escape);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Paused);
    let present = (at_sea(w), w.resource::<PlayerHelm>().position);
    press(w, KeyCode::KeyR);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Rewind);
    let shown = at_sea(w);
    assert!((present.0 - 10. - shown).abs() <= STEP, "{shown} vs {}", present.0);
    assert!(w.resource::<Encounter>().ship(victim).is_some());
    assert_ne!(w.resource::<PlayerHelm>().position, present.1);

    // Esc puts the present back as it was.
    press(w, KeyCode::Escape);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Paused);
    assert_eq!((at_sea(w), w.resource::<PlayerHelm>().position), present);
    assert!(w.resource::<Encounter>().ship(victim).is_none());

    // A step back and Enter: she sails on from there, and what came after is gone.
    press(w, KeyCode::KeyR);
    press(w, KeyCode::ArrowLeft);
    let picked = at_sea(w);
    assert!(picked < shown, "{picked} vs {shown}");
    press(w, KeyCode::Enter);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Sailing);
    assert_eq!(at_sea(w), picked);
    let timeline = w.resource::<Timeline>();
    assert!(timeline.span().unwrap().1 <= picked + 1e-3);
    assert!(timeline.marks().is_empty());
    assert!(w.resource::<Encounter>().ship(victim).is_some());
    sail(w, 1.);
    assert!(at_sea(w) > picked);

    // Lost, the report rewinds her afloat.
    sail(w, 12.);
    let hull = w.resource::<Campaign>().hull_shape();
    for i in 0..12 {
        let aft = (hull.bow + hull.stern) * (i as f32 + 0.5) / 12.;
        w.resource_mut::<Campaign>().strike(aft, 9999);
    }
    sail(w, 1.);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Lost);
    assert!(
        (w.resource::<Timeline>().marks().iter()).any(|m| m.kind == MarkKind::Lost),
        "her loss is marked"
    );
    press(w, KeyCode::KeyR);
    assert_eq!(w.resource::<Campaign>().screen(), Screen::Rewind);
    press(w, KeyCode::Enter);
    let run = w.resource::<Campaign>();
    assert_eq!(run.screen(), Screen::Sailing);
    assert!(run.ship.afloat());
}
