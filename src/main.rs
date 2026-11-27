mod alert;
mod battle_view;
mod berth;
mod blueprint;
mod camera;
mod captain;
mod cards;
mod crew_book;
mod damage_control;
mod damage_fx;
mod dockyard;
mod ensign;
mod eyepiece;
mod feedback;
mod fire_control;
mod flagship_hud;
mod flares;
mod gunsmoke;
mod fleet_models;
mod gun_sight;
mod gunnery;
mod icefield;
mod insignia;
#[cfg(feature = "mcp")]
mod mcp;
mod minimap;
mod mission_hud;
mod options;
mod ordnance;
mod passage;
mod rewind;
mod scene;
mod seascape;
mod slip_badges;
mod snapshot;
mod sound;
mod state;
mod strike_craft;
mod surfaces;
mod telemetry;
#[cfg(test)]
mod tests;
mod threat_ring;
mod trajectory;
mod ui;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use camera::ShipView;
use passage::Passage;
#[cfg(test)]
use shipvivor_campaign::Objectives;
use shipvivor_encounter::{Aim, Volley};
#[cfg(test)]
use shipvivor_encounter::{Hull, MainShell, Orders};
use state::{
    Campaign, ChartSeeds, Embarked, Encounter, Guns, Mode, PlayerHelm, Screen, TrialCourse,
};

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(berth::window()),
            ..default()
        }))
        .add_plugins(berth::BerthPlugin)
        .add_plugins(seascape::SeascapePlugin)
        .add_plugins(blueprint::BlueprintPlugin)
        .add_plugins(GamePlugin)
        .add_plugins(crew_book::CrewBookPlugin)
        .add_plugins(sound::SoundPlugin)
        .add_plugins(snapshot::SnapshotPlugin)
        .add_plugins(telemetry::TelemetryPlugin)
        .add_plugins(icefield::IcefieldPlugin)
        .add_plugins(gunsmoke::GunsmokePlugin)
        .add_plugins(battle_view::MainlandPlugin);
    #[cfg(feature = "mcp")]
    app.add_plugins(mcp::McpPlugin);
    app.run();
}

struct GamePlugin;
impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(60.))
            .init_resource::<Assets<seascape::Seascape>>()
            .init_resource::<Assets<blueprint::Drafted>>()
            .init_resource::<Assets<Font>>()
            .init_resource::<PlayerHelm>()
            .init_resource::<TrialCourse>()
            .init_resource::<Campaign>()
            .init_resource::<Encounter>()
            .init_resource::<ChartSeeds>()
            .init_resource::<ensign::Colours>()
            .init_resource::<ui::customise::Locker>()
            .init_resource::<options::GameOptions>()
            .init_resource::<feedback::Feedback>()
            .init_resource::<dockyard::Pointer>()
            .init_resource::<Guns>()
            .init_resource::<ShipView>()
            .init_resource::<seascape::Sea>()
            .init_resource::<sound::Soundboard>()
            .init_resource::<alert::BaseAlerts>()
            .init_resource::<flares::Flares>()
            .add_plugins(rewind::RewindPlugin)
            .add_observer(seascape::embark)
            .add_observer(sound::hush)
            .add_observer(sound::play_notice)
            .add_systems(
                Update,
                ui::passage_chart::roll
                    .before(ui::rebuild_screen)
                    .before(sound::keep_time)
                    .before(camera::cursor_mode),
            )
            .add_systems(Startup, strike_craft::setup)
            .add_systems(Startup, flares::setup)
            .add_systems(Update, flares::draw)
            .add_systems(Startup, trajectory::setup)
            .add_systems(Update, trajectory::draw.after(camera::follow))
            .add_systems(Startup, eyepiece::spawn)
            .add_systems(
                PostUpdate,
                (
                    eyepiece::field_stop.before(bevy::ui::UiSystems::Layout),
                    eyepiece::clip
                        .after(bevy::ui::UiSystems::Layout)
                        .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
                ),
            )
            .add_systems(
                Update,
                (
                    strike_craft::rig,
                    strike_craft::stow,
                    strike_craft::fly.after(camera::follow),
                    strike_craft::trail,
                    strike_craft::fall,
                ),
            )
            .add_systems(
                Startup,
                (
                    scene::setup,
                    captain::setup.after(scene::setup),
                    dockyard::setup,
                    seascape::setup,
                    camera::setup,
                    (|mut helm: ResMut<PlayerHelm>| *helm = PlayerHelm::cruising())
                        .run_if(|run: Res<Campaign>| run.mode == Mode::Title),
                    battle_view::setup.after(scene::setup),
                    damage_fx::setup,
                    fire_control::setup,
                    insignia::setup,
                    minimap::setup,
                    threat_ring::setup,
                ),
            )
            .add_systems(
                PreUpdate,
                (
                    dockyard::point,
                    ui::intent
                        .pipe(ui::customise::choose)
                        .pipe(ui::options::choose)
                        .pipe(ui::feedback::rate)
                        .pipe(apply_action),
                    engine_orders,
                    camera::mouse_look,
                    aim_guns,
                )
                    .chain()
                    .after(bevy::ui::UiSystems::Focus),
            )
            .add_systems(
                FixedUpdate,
                (
                    show_clearance.run_if(|run: Res<Campaign>| run.screen() == Screen::Secured),
                    advance_ship.run_if(under_way),
                    title_cruise.run_if(|run: Res<Campaign>| run.mode == Mode::Title),
                    seascape::churn
                        .run_if(|run: Res<Campaign>| run.mode == Mode::Title || run.under_way()),
                    alert::watch.run_if(under_way),
                    flares::fly.run_if(under_way),
                    sound::report.run_if(under_way),
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    (
                        seascape::roll,
                        scene::refit_hull
                            .run_if(|run: Res<Campaign>| run.screen() != Screen::Secured),
                        dockyard::stage,
                        scene::place_ship,
                        scene::float_buoys.run_if(|run: Res<Campaign>| run.mode == Mode::Trial),
                    )
                        .chain(),
                    seascape::follow,
                    seascape::light,
                    scene::place_turrets
                        .run_if(|run: Res<Campaign>| run.screen() != Screen::Secured),
                    (
                        scene::place_wing_guns,
                        scene::place_aa_guns,
                        scene::place_tower,
                        blueprint::draft.after(scene::refit_hull),
                        dockyard::fans,
                    ),
                    captain::wind,
                    scene::place_racks,
                    scene::animate_gun_tubes,
                    camera::follow.after(seascape::roll),
                    scene::spray::animate.after(camera::follow),
                    camera::cursor_mode,
                    scene::place_horizon,
                    scene::signals,
                    (
                        ui::customise::turn_wheel,
                        ensign::hoist,
                        battle_view::dye_ensigns,
                        battle_view::sync.after(camera::follow),
                        battle_view::animate_harbour_signals.after(camera::follow),
                        battle_view::stream_funnel_smoke.after(battle_view::sync),
                        battle_view::fly_pennant,
                    )
                        .chain(),
                    fire_control::place_sectors,
                    minimap::update,
                    (
                        threat_ring::update,
                        threat_ring::tag_ships,
                        slip_badges::update,
                        damage_fx::mark_repairs.after(scene::place_ship),
                        damage_fx::smoulder.after(scene::place_ship),
                    )
                        .after(camera::follow),
                    sound::keep_time,
                    (
                        dockyard::open,
                        ui::rebuild_screen,
                        ui::customise::refresh,
                        ui::options::refresh,
                        ui::feedback::light,
                        dockyard::dimension,
                        dockyard::place,
                        ui::instruments,
                        captain::react,
                        mission_hud::update,
                        mission_hud::spotter_alerts,
                        flagship_hud::update,
                        (ui::gauges, ui::dock_meter),
                        gun_sight::update,
                        gunnery::update,
                        damage_control::update,
                        fire_control::update_marks,
                        ui::hover,
                        cards::animate,
                        cards::park_pointer,
                    )
                        .chain(),
                    ui::scale
                        .after(ui::passage_chart::roll)
                        .before(ui::rebuild_screen),
                ),
            );
    }
}
fn show_clearance(
    clock: Res<Time<Fixed>>,
    mut run: ResMut<Campaign>,
    mut battle: ResMut<Encounter>,
) {
    battle.advance_effects(clock.delta_secs());
    run.clearance_left = (run.clearance_left - clock.delta_secs()).max(0.);
}

/// Give the title shot a steady head of steam on a broad, continuous turn.
fn title_cruise(clock: Res<Time<Fixed>>, mut helm: ResMut<PlayerHelm>) {
    helm.tick(0.018, clock.delta_secs());
}

fn under_way(run: Res<Campaign>) -> bool {
    run.under_way()
}

#[derive(SystemParam)]
struct Voyage<'w, 's> {
    helm: ResMut<'w, PlayerHelm>,
    course: ResMut<'w, TrialCourse>,
    encounter: ResMut<'w, Encounter>,
    guns: ResMut<'w, Guns>,
    view: ResMut<'w, ShipView>,
    seeds: ResMut<'w, ChartSeeds>,
    options: Res<'w, options::GameOptions>,
    commands: Commands<'w, 's>,
}
impl Voyage<'_, '_> {
    /// Put the ship at the start of the course for the current mode and stage. A patrol
    /// sails waters charted from the run's seed. The trial and the title keep fixed charts.
    fn outfit(&mut self, run: &mut Campaign) {
        run.watch = Default::default();
        let passage = Passage::graded(run.stage, run.grade).with_strength(run.strength());
        let chart = match run.mode {
            Mode::Trial => {
                *self.course = TrialCourse::default();
                shipvivor_encounter::Chart::classic(self.course.gates().map(|g| g.center))
            }
            Mode::Patrol => passage.chart(run.seed),
            Mode::Title | Mode::Paused | Mode::Rewind => passage.classic(),
        };
        // The simulator opens its legs with these same calls.
        let patrol = run.mode == Mode::Patrol;
        *self.encounter = Encounter(shipvivor_sim::open_battle(
            passage.sector(),
            &chart,
            run,
            self.options.rewards,
            patrol,
        ));
        *self.helm = if run.mode == Mode::Title {
            PlayerHelm::cruising()
        } else {
            PlayerHelm(shipvivor_sim::helm_at_start(
                &chart,
                &self.encounter,
                patrol,
            ))
        };
        self.guns.torpedo_mode = false;
        self.guns.launch_pending = false;
        self.guns.volley = None;
        // Crown Deep opens on what is coming for her: the Regent, or the screen she sends
        // on ahead while she holds back over the horizon.
        let raiders = self.encounter.raiders();
        let flagship = (raiders.iter())
            .find(|r| r.class == shipvivor_encounter::EnemyClass::Regent)
            .or_else(|| {
                (run.stage == shipvivor_campaign::Stage::CrownDeep)
                    .then(|| {
                        (raiders.iter())
                            .filter(|r| !r.class.is_merchant())
                            .min_by(|a, b| {
                                let d = |r: &&shipvivor_encounter::Raider| {
                                    r.body.pos.distance_squared(self.helm.position)
                                };
                                d(a).total_cmp(&d(b))
                            })
                    })
                    .flatten()
            });
        if let Some(ship) = flagship {
            self.helm.heading = shipvivor_compass::heading(ship.body.pos - self.helm.position);
        }
        *self.view = ShipView::astern(self.helm.heading);
        self.view.hull_extension = run.added_length();
        if let Some(ship) = flagship {
            self.view.center_on(
                self.helm.position,
                ship.body.pos.extend(ship.class.profile().deck).xzy(),
            );
        }
        self.commands.trigger(Embarked);
    }
}
fn apply_action(In(action): In<Option<ui::Action>>, mut run: ResMut<Campaign>, mut voyage: Voyage) {
    use ui::Action;
    let Some(action) = action else { return };
    // A mouse press used to launch or choose a card is consumed until released.
    voyage.guns.mouse_captured = true;
    match (run.screen(), action) {
        (Screen::Title | Screen::Won(_) | Screen::Lost, Action::Start) => {
            if run.mode != Mode::Title {
                *run = Campaign::default();
                voyage.outfit(&mut run);
            }
            run.title_page = state::TitlePage::Departure;
            run.seed = voyage.seeds.peek();
        }
        (Screen::Departure, Action::Title) => run.title_page = state::TitlePage::Menu,
        // Test mode: its list of seas opens from the title, and from its own yard with
        // the ship built there kept for the next sea.
        (Screen::Title, Action::TestMode) => run.title_page = state::TitlePage::Exercise,
        (Screen::Drydock(_), Action::TestMode) if run.is_exercise() => {
            run.bench = (run.stage.round(), run.grade);
            run.mode = Mode::Title;
            run.title_page = state::TitlePage::Exercise;
            voyage.outfit(&mut run);
        }
        (Screen::Exercise, Action::TestRound(round)) => run.bench.0 = round,
        (Screen::Exercise, Action::TestGrade(grade)) => run.bench.1 = grade,
        (Screen::Exercise, Action::TestSea(stage)) => {
            *run = run.exercise(stage, voyage.seeds.draw());
            voyage.outfit(&mut run);
        }
        // Back at the title a ship built in test mode is given up.
        (Screen::Exercise, Action::Title) if run.is_exercise() => {
            *run = Campaign::default();
            voyage.outfit(&mut run);
        }
        (Screen::Exercise, Action::Title) => run.title_page = state::TitlePage::Menu,
        // A test sortie, however it ended or wherever it was broken off, goes back to
        // its yard with the ship as she sailed.
        (Screen::Paused | Screen::Won(_) | Screen::Lost, Action::Yard) => {
            if run.return_to_yard() {
                voyage.encounter.fit_player(&run);
            }
        }
        (Screen::Departure, Action::Depart(stage)) if stage.is_opening() => {
            *run = Campaign::new(Mode::Patrol);
            run.stage = stage;
            run.origin = stage;
            run.seed = voyage.seeds.draw();
            if let Some(veteran) = dev_start(run.seed) {
                *run = Campaign::with_run(Mode::Patrol, veteran);
            }
            voyage.outfit(&mut run);
        }
        (Screen::Title, Action::Trial) => {
            *run = Campaign::new(Mode::Trial);
            voyage.outfit(&mut run);
        }
        (Screen::Trial, Action::RestartTrial) => voyage.outfit(&mut run),
        (Screen::Trial | Screen::Paused | Screen::Won(_) | Screen::Lost, Action::Title) => {
            *run = Campaign::default();
            voyage.outfit(&mut run);
        }
        (Screen::Secured, Action::Continue) => run.clearance_left = 0.,
        (Screen::Sailing, Action::Pause) => run.mode = Mode::Paused,
        (Screen::Sailing | Screen::PassageMap, Action::ToggleChart) => {
            run.mode = Mode::Paused;
            let roll = run.chart_roll.get_or_insert_default();
            roll.opening = !roll.opening;
            let cue = if roll.opening {
                shipvivor_acoustics::Sound::UnrollChart
            } else {
                shipvivor_acoustics::Sound::RollChart
            };
            voyage.commands.trigger(sound::Notice(cue));
        }
        (Screen::Paused, Action::Resume) => run.mode = Mode::Patrol,
        (Screen::Paused | Screen::Lost, Action::Rewind) => voyage.commands.trigger(rewind::Open),
        (Screen::Refit, Action::Card(card)) => {
            let before = run.fittings;
            run.install(card);
            voyage.encounter.fit_player(&run);
            if run.fittings != before {
                voyage
                    .commands
                    .trigger(sound::Notice(shipvivor_acoustics::Sound::Upgrade));
            }
        }
        (Screen::Drydock(_), Action::Module(index)) => run.select_section(index),
        (Screen::Drydock(_), Action::Preview(index)) => run.focus_option(index),
        // A number key on work the yard cannot make picks it out, so its figures say why.
        // A tile it cannot make takes no click.
        (Screen::Drydock(_), Action::Order(index)) => {
            if run.order_option(index) {
                voyage.encounter.fit_player(&run);
            } else {
                run.focus_option(index);
            }
        }
        (Screen::Drydock(_), Action::Draft(job)) => {
            if run.order(job) {
                voyage.encounter.fit_player(&run);
            }
        }
        (Screen::Drydock(_), Action::Undo(index)) => {
            run.undo_order(index);
            voyage.encounter.fit_player(&run);
        }
        (Screen::Drydock(_), Action::Confirm) => {
            if let Some(job) = run.dock_job() {
                if run.order(job) {
                    voyage.encounter.fit_player(&run);
                }
            } else {
                finalize_refit(&mut run, &mut voyage);
            }
        }
        // Orders still to build are built before she sails.
        (Screen::Drydock(_), Action::Sail) if run.plan().is_some() => {
            finalize_refit(&mut run, &mut voyage);
        }
        (Screen::Drydock(_), Action::Sail) => {
            if run.sail_from_dock() {
                voyage.outfit(&mut run);
            }
        }
        (Screen::Intercept(_), Action::Engage) if run.engage_regent() => voyage.outfit(&mut run),
        (Screen::Chart, Action::Depart(next)) if run.depart(next) => voyage.outfit(&mut run),
        (Screen::Chart, Action::Punish(lane)) => {
            run.toggle_punishing(lane);
        }
        _ => {}
    }
}
/// Build the yard's orders: the blueprints are revealed as the ship herself.
/// With the dev tools, `SHIPVIVOR_START=<round>` opens a new run at that round, for
/// trying the later rounds by hand: the simulator's captain sails every round before it
/// for real, takes her cards, spends her yard points and picks her passages, so the ship
/// is one a good player would bring. `SHIPVIVOR_START_SKILL` sets that captain's skill,
/// from 0 to 1 (1 by default). Rounds 2 to 7; anything else starts as usual. Sailing the
/// rounds before takes a few seconds a round in a release build, and a captain lost on the
/// way is sailed again from the next seed.
fn dev_start(seed: u64) -> Option<shipvivor_campaign::Run> {
    if !cfg!(feature = "dev-tools") {
        return None;
    }
    let round: usize = std::env::var("SHIPVIVOR_START").ok()?.trim().parse().ok()?;
    if !(2..=shipvivor_campaign::ROUNDS).contains(&round) {
        return None;
    }
    let skill = std::env::var("SHIPVIVOR_START_SKILL")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(1.);
    let started = std::time::Instant::now();
    let found = shipvivor_sim::run::veteran(seed, round, shipvivor_sim::bot::Skill(skill), 24);
    match &found {
        Some((_, seed)) => eprintln!(
            "SHIPVIVOR_START: round {round} reached from seed {seed} in {:.0} s",
            started.elapsed().as_secs_f32()
        ),
        None => eprintln!("SHIPVIVOR_START: no captain reached round {round}; starting as usual"),
    }
    found.map(|(run, _)| run)
}

fn finalize_refit(run: &mut Campaign, voyage: &mut Voyage) {
    let Some(plan) = run.finalize() else { return };
    if plan.cost() > 0 {
        voyage
            .commands
            .trigger(sound::Notice(shipvivor_acoustics::Sound::Upgrade));
    }
    voyage
        .commands
        .insert_resource(blueprint::Reveal::of(&plan, run));
    voyage.encounter.fit_player(run);
}
fn engine_orders(
    keys: Res<ButtonInput<KeyCode>>,
    run: Res<Campaign>,
    mut helm: ResMut<PlayerHelm>,
) {
    if !run.under_way() {
        return;
    }
    let step = isize::from(keys.just_pressed(KeyCode::KeyW) as i8)
        - isize::from(keys.just_pressed(KeyCode::KeyS) as i8);
    if step != 0 {
        helm.ring(step);
    }
}
/// A second press of the trigger within this many seconds fires a salvo.
const DOUBLE_CLICK: f32 = 0.35;

fn aim_guns(
    run: Res<Campaign>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<(&Window, &bevy::window::CursorOptions)>,
    time: Res<Time>,
    battle: Res<Encounter>,
    mut guns: ResMut<Guns>,
    mut was_live: Local<bool>,
) {
    let (window, cursor) = *window;
    let left = mouse.pressed(MouseButton::Left);
    if !left {
        guns.mouse_captured = false;
    }
    if left && (!window.focused || cursor.grab_mode != bevy::window::CursorGrabMode::Locked) {
        guns.mouse_captured = true;
    }
    let live = run.screen() == Screen::Sailing && window.focused;
    if live && keys.just_pressed(KeyCode::KeyT) {
        guns.torpedo_mode = !guns.torpedo_mode;
        guns.launch_pending = false;
    }
    // A digit picks out the turret with that number from the bow, and again lets it go.
    // Z steps the picked turret, or with none picked out every turret, through manual,
    // scope and automatic.
    let turrets = battle.main_mounts().len().min(state::TURRETS);
    if guns.selected.is_some_and(|i| i >= turrets) {
        guns.selected = None;
    }
    // The digit that chose a route or a fitting card is not also a turret.
    let steady = live && *was_live;
    *was_live = live;
    if steady {
        let digits = [
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::Digit5,
            KeyCode::Digit6,
            KeyCode::Digit7,
            KeyCode::Digit8,
            KeyCode::Digit9,
        ];
        if let Some(i) = digits[..turrets].iter().position(|&k| keys.just_pressed(k)) {
            guns.selected = (guns.selected != Some(i)).then_some(i);
        }
        if keys.just_pressed(KeyCode::KeyZ) && turrets > 0 {
            match guns.selected {
                Some(i) => guns.controls[i] = guns.controls[i].next(),
                None => {
                    let next = guns.controls[0].next();
                    guns.controls[..turrets].fill(next);
                }
            }
        }
    }
    if !live || !guns.torpedo_mode {
        guns.launch_pending = false;
    } else if mouse.just_pressed(MouseButton::Left) && !guns.mouse_captured {
        guns.launch_pending = true;
    }
    guns.firing = live && left && !guns.mouse_captured && !guns.torpedo_mode;
    guns.nearest = live && keys.pressed(KeyCode::Space);
    // A press fires one barrel and a quick second press the rest. Holding walks the battery.
    let pressed = (mouse.just_pressed(MouseButton::Left) && !guns.mouse_captured)
        || keys.just_pressed(KeyCode::Space);
    if !live || guns.torpedo_mode {
        guns.volley = None;
    } else if pressed {
        let now = time.elapsed_secs();
        let volley = if guns.last_press.is_some_and(|t| now - t <= DOUBLE_CLICK) {
            Volley::Salvo
        } else {
            Volley::Single
        };
        guns.volley = guns.volley.max(Some(volley));
        guns.last_press = Some(now);
    }
}
fn advance_ship(
    clock: Res<Time<Fixed>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut helm: ResMut<PlayerHelm>,
    mut course: ResMut<TrialCourse>,
    mut run: ResMut<Campaign>,
    mut encounter: ResMut<Encounter>,
    sight: (ResMut<Guns>, ResMut<ShipView>),
    mut commands: Commands,
) {
    let (mut guns, mut view) = sight;
    let dt = clock.delta_secs();
    let steering = i32::from(keys.pressed(KeyCode::KeyD)) - i32::from(keys.pressed(KeyCode::KeyA));
    let previous = helm.position;
    let patrol = run.screen() == Screen::Sailing;
    // The simulator sails through these same three steps: steer, engage, settle.
    shipvivor_sim::steer(&run, &encounter, &mut helm, steering as f32, dt, patrol);
    if run.mode == Mode::Trial
        && course
            .bypass_change_detection()
            .advance(previous, helm.position)
    {
        course.set_changed();
    }
    if patrol {
        let aim = if guns.torpedo_mode {
            view.tracked_point(&encounter).map_or(Aim::Hold, Aim::Track)
        } else {
            view.main_aim(helm.position, &encounter, guns.firing, guns.nearest)
        };
        let volley = guns.volley.take();
        encounter.direct(&guns.controls, view.scoped());
        // The trigger offers first the smallest gun that can hurt the ship she aims at.
        let marked = view.target(&encounter).or_else(|| {
            (guns.nearest && view.quarry(&encounter).is_none())
                .then(|| encounter.nearest_ship(helm.position))
                .flatten()
        });
        let marked = marked.map(|r| r.body.id);
        encounter.mark(marked);
        let ev = shipvivor_sim::engage(&run, &mut encounter, &mut helm, dt, aim, volley);
        // The scoped sight rides with her contact through the tick.
        view.ride_zero(helm.position, &encounter);
        view.update_contact(helm.position, &encounter);
        let torpedo =
            (guns.torpedo_mode && std::mem::take(&mut guns.launch_pending)).then_some(view.bearing);
        run.hits += u32::from(ev.hits);
        run.sunk += u32::from(ev.sunk);
        run.watch.observe(dt, &ev);
        let banked = run.cargo_points();
        shipvivor_sim::settle(&mut run, &mut encounter, &helm, dt, &ev, torpedo);
        // Each whole yard point that cargo sunk makes rings the bell. Crown Deep has
        // no yard after it.
        if run.cargo_points() > banked && run.stage != shipvivor_campaign::Stage::CrownDeep {
            commands.trigger(sound::Notice(shipvivor_acoustics::Sound::Chime));
        }
        // The phase, not the screen: a clearance with levels owed deals them first.
        if matches!(
            run.phase(),
            shipvivor_campaign::Phase::Drydock(shipvivor_campaign::LegEnd::BasesDestroyed)
                | shipvivor_campaign::Phase::Intercept(shipvivor_campaign::LegEnd::BasesDestroyed)
                | shipvivor_campaign::Phase::Won(shipvivor_campaign::LegEnd::RegentSunk)
        ) {
            run.clearance_left = if ev.regent_sunk { 5. } else { 2.2 };
        }
    }
}
