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

fn press(world: &mut World, key: KeyCode) {
    world.resource_mut::<ButtonInput<KeyCode>>().press(key);
    world.run_schedule(PreUpdate);
    let mut keys = world.resource_mut::<ButtonInput<KeyCode>>();
    keys.release(key);
    keys.clear();
    world.run_schedule(Update);
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

const TEST_SEED: u64 = 1;

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
fn reserve_power_reaches_the_helm_and_damage_control_until_the_engine_restarts() {
    use shipvivor_shipyard::Component;
    let mut app = headless_game();
    let w = app.world_mut();
    press(w, KeyCode::Enter);
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
    press(w, KeyCode::Enter);
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
    // Sail the base exit on both legs using the same early-extraction input as the player.
    for leg in 0..2 {
        for _ in 0..3700 {
            w.run_schedule(FixedUpdate);
            if w.resource::<Campaign>().screen() == Screen::Refit {
                press(w, KeyCode::Digit1);
            }
            if w.resource::<Campaign>().can_extract() {
                break;
            }
        }
        press(w, KeyCode::KeyE);
        if leg == 0 {
            assert_eq!(
                w.resource::<Campaign>().screen(),
                Screen::Drydock(LegEnd::Extracted)
            );
            visit(w, 1);
            let shells = shell_refit(w);
            choose(w, shells);
            // The only order is the last one, so it takes a second press.
            press(w, KeyCode::Enter);
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
    press(w, KeyCode::KeyE);
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
    press(w, KeyCode::Enter);
    assert_eq!(w.resource::<Campaign>().fittings.shells, 0);
    assert_eq!(
        (w.resource::<Campaign>().hits, w.resource::<Campaign>().sunk),
        (0, 0)
    );
}

#[test]
fn destroying_the_yards_or_timing_out_brings_the_ship_through_drydock() {
    use shipvivor_encounter::YardState;
    for reason in [LegEnd::BasesDestroyed, LegEnd::TimeExpired] {
        let mut app = headless_game();
        let w = app.world_mut();
        press(w, KeyCode::Enter);
        if reason == LegEnd::BasesDestroyed {
            // Fire on each yard from the fairway. Between bases, let the game read the
            // resulting port states and update its mission HUD.
            for i in 0..w.resource::<Encounter>().ports().len() {
                let mut battle = w.resource_mut::<Encounter>();
                let yard = battle.ports()[i].yard_position();
                let approach = battle.ports()[i].body.forward();
                for _ in 0..600 {
                    if battle.ports()[i].yard_state() == YardState::Ruined {
                        break;
                    }
                    let mark = yard
                        .extend((3.2 + battle.ports()[i].yard_top()) * 0.5)
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
                assert_eq!(battle.ruined_yards(), i + 1);
                w.run_schedule(FixedUpdate);
                w.run_schedule(Update);
                if i < 2 {
                    assert_eq!(w.resource::<Campaign>().screen(), Screen::Sailing);
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
        // The last order waits for a second press before the dock closes.
        assert!(w.resource::<Campaign>().dock_armed);
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
    press(w, KeyCode::Enter);
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
    for (stage, seed) in std::iter::once(Stage::IronNarrows)
        .chain(EXITS.map(|exit| exit.stage))
        .flat_map(|stage| [TEST_SEED, 2, 3].map(|seed| (stage, seed)))
    {
        let passage = Passage::of(stage);
        let chart = passage.chart(seed);
        let course = Passage::course(&chart);
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
            (stage != Stage::IronNarrows).then_some(shipvivor_encounter::Sortie::Due(
                shipvivor_encounter::FLAGSHIP_SORTIE
            ))
        );
        assert!(
            course
                .gates()
                .iter()
                .map(|g| g.center.length())
                .fold(0., f32::max)
                > 390.
        );
        assert!(
            battle
                .raiders()
                .iter()
                .all(|r| r.body.pos.length() >= SPAWN_STANDOFF)
        );
        for port in battle.ports() {
            assert_eq!(battle.navigable_position(port.berth(), 3.), port.berth());
            for gate in course.gates() {
                assert_eq!(battle.navigable_position(gate.center, 15.), gate.center);
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
    press(w, KeyCode::Enter);
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
    press(w, KeyCode::Enter);
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
    press(w, KeyCode::Enter);
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
fn the_bridge_hears_the_telegraph_answer_and_her_engines_stop() {
    use shipvivor_acoustics::{Mixer, SAMPLE_RATE, Sound};
    use shipvivor_shipyard::Component;
    /// Sail on for some ticks and return the level of what was heard.
    fn sail(w: &mut World, mixer: &mut Mixer, ticks: usize, mut each: impl FnMut(&mut World)) -> f32 {
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
    press(w, KeyCode::Enter);
    // The order the voyage begins with rings no bell. A new one does, at once.
    sail(w, &mut mixer, 240, |_| {});
    assert_eq!(mixer.making(Sound::Telegraph).0, 0);
    w.resource_mut::<PlayerHelm>().ring(1);
    sail(w, &mut mixer, 1, |_| {});
    assert_eq!(mixer.making(Sound::Telegraph).0, 1);
    let steaming = sail(w, &mut mixer, 240, |_| {});
    assert_eq!(mixer.making(Sound::Telegraph).0, 0);
    // With every engine knocked out and kept out, the machinery is heard to stop.
    let wreck = |w: &mut World| {
        let ship = &mut w.resource_mut::<Campaign>().ship;
        let engines: Vec<_> = ship.fitted(Component::Engine).collect();
        for (berth, _) in engines {
            ship.strike_fitting(berth, 1e3);
        }
    };
    sail(w, &mut mixer, 360, wreck);
    assert_eq!(w.resource::<Campaign>().ship.propulsion(), 0.);
    let adrift = sail(w, &mut mixer, 240, wreck);
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
        made.iter().fold((0, 0), |sum, n| (sum.0 + n.0, sum.1 + n.1))
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
    press(w, KeyCode::Enter);
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
    press(w, KeyCode::Enter);
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
    press(w, KeyCode::Enter);
    let player = w.resource::<PlayerHelm>().position;
    let b = w.resource::<Encounter>();
    let ship = &b.raiders()[1];
    assert_ne!(b.nearest_ship(player).unwrap().body.id, ship.body.id);
    let center = ship.body.pos.extend(ship.class.profile().deck * 0.65).xzy();
    for zoom in [0., 1.] {
        let mut view = ShipView::default();
        view.zoom = zoom;
        view.center_on(player, center);
        view.bearing += if zoom == 0. { 0.058 } else { 0.021 };
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
fn torpedo_mode_click_launches_once_and_bore_cards_resize_the_deck_models() {
    let mut app = headless_game();
    let w = app.world_mut();
    press(w, KeyCode::Enter);
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
    press(w, KeyCode::Enter);
    for (stores, key, tubes) in [
        (3, KeyCode::Digit3, 2),
        (4, KeyCode::Digit2, 3),
        (5, KeyCode::Digit2, 4),
    ] {
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
    press(w, KeyCode::Enter);
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
    press(w, KeyCode::Enter);
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
    press(w, KeyCode::Enter);
    let stock = geometry(w);
    w.resource_mut::<Campaign>().tick_sector(
        1.,
        Objectives {
            kills: 4,
            gates: 3,
            crates: 0,
        },
        0,
        true,
    );
    w.run_schedule(Update);
    assert!(w.query::<&Text>().iter(w).any(|t| t.as_str() == "40 m"));
    // Selecting the hull at the bow and choosing a reserve bay previews the slipway,
    // even though the ship still measures 40 m.
    let reserve = shipvivor_campaign::DockJob::Section(shipvivor_campaign::DockModule::Reserve);
    visit(w, 0);
    choose(w, reserve);
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
    // across departure.
    choose(w, reserve);
    press(w, KeyCode::Enter);
    assert_eq!(w.resource::<Campaign>().ship.design().length(), 45.);
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
    press(w, KeyCode::Enter);
    assert_eq!(geometry(w), stock);
    assert_eq!(w.query::<&damage_control::Cell>().iter(w).count(), 6);
}

#[test]
fn mission_clock_and_industry_bearings_remain_visible_in_the_last_minute() {
    let mut app = headless_game();
    let w = app.world_mut();
    press(w, KeyCode::Enter);
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
    press(w, KeyCode::Enter);
    w.resource_mut::<Campaign>().tick_sector(
        1.,
        Objectives {
            kills: 4,
            gates: 3,
            crates: 3,
        },
        0,
        true,
    );
    w.run_schedule(Update);
    assert_eq!(w.resource::<Campaign>().fittings_remaining(), 3);
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
    assert_eq!(w.resource::<Campaign>().fittings_remaining(), 3);
    assert_eq!(
        w.resource::<Campaign>().fittings.round().calibre_inches(),
        4.
    );

    // The hull is widened from its bow.
    visit(w, 0);
    choose(w, DockJob::WidenBeam);
    let beam = w.resource::<Campaign>().ship.design().beam();
    assert!(w.resource::<Campaign>().display_design().beam() > beam);
    press(w, KeyCode::Enter);
    assert!(w.resource::<Campaign>().ship.design().beam() > beam);

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
    // The last order asks twice before the dock closes.
    press(w, KeyCode::Enter);
    assert!(matches!(
        w.resource::<Campaign>().screen(),
        Screen::Drydock(_)
    ));
    assert!(w.resource::<Campaign>().dock_armed);
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
fn a_one_order_yard_adds_a_turret_after_asking_twice() {
    use shipvivor_campaign::{DeckModule, DockJob};

    let mut app = headless_game();
    let w = app.world_mut();
    press(w, KeyCode::Enter);
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
    press(w, KeyCode::Enter);
    assert!(w.resource::<Campaign>().dock_armed);
    press(w, KeyCode::ArrowLeft);
    assert!(!w.resource::<Campaign>().dock_armed);
    choose(w, split);
    press(w, KeyCode::Enter);
    assert!(matches!(
        w.resource::<Campaign>().screen(),
        Screen::Drydock(_)
    ));
    press(w, KeyCode::Enter);
    let run = w.resource::<Campaign>();
    assert_eq!(run.screen(), Screen::Chart);
    assert_eq!(run.ship.design().gun_stations().len(), 3);
}

#[test]
fn a_spotter_contact_posts_a_bridge_alert_then_clears_the_marker() {
    let mut app = headless_game();
    let w = app.world_mut();
    press(w, KeyCode::Enter);
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
