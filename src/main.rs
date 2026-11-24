mod alert;
mod battle_view;
mod berth;
mod camera;
mod captain;
mod cards;
mod damage_control;
mod dockyard;
mod ensign;
mod fire_control;
mod flagship_hud;
mod flares;
mod fleet_models;
mod gun_sight;
mod gunnery;
mod icefield;
mod insignia;
mod minimap;
mod mission_hud;
mod options;
mod ordnance;
mod passage;
mod scene;
mod seascape;
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
use shipvivor_campaign::Objectives;
#[cfg(test)]
use shipvivor_encounter::Hull;
use shipvivor_encounter::{Aim, Ammo, MainShell, Orders, Volley};
use state::{
    Campaign, ChartSeeds, Embarked, Encounter, Guns, Mode, PlayerHelm, Screen, TrialCourse,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(berth::window()),
            ..default()
        }))
        .add_plugins(berth::BerthPlugin)
        .add_plugins(seascape::SeascapePlugin)
        .add_plugins(GamePlugin)
        .add_plugins(sound::SoundPlugin)
        .add_plugins(snapshot::SnapshotPlugin)
        .add_plugins(telemetry::TelemetryPlugin)
        .add_plugins(icefield::IcefieldPlugin)
        .add_plugins(battle_view::MainlandPlugin)
        .run();
}

struct GamePlugin;
impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(60.))
            .init_resource::<Assets<seascape::Seascape>>()
            .init_resource::<Assets<Font>>()
            .init_resource::<PlayerHelm>()
            .init_resource::<TrialCourse>()
            .init_resource::<Campaign>()
            .init_resource::<Encounter>()
            .init_resource::<ChartSeeds>()
            .init_resource::<ensign::Colours>()
            .init_resource::<ui::customise::Locker>()
            .init_resource::<options::GameOptions>()
            .init_resource::<dockyard::Pointer>()
            .init_resource::<Guns>()
            .init_resource::<ShipView>()
            .init_resource::<seascape::Sea>()
            .init_resource::<sound::Soundboard>()
            .init_resource::<alert::BaseAlerts>()
            .init_resource::<flares::Flares>()
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
            .add_systems(
                Update,
                (
                    strike_craft::stow,
                    strike_craft::fly,
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
                    battle_view::setup,
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
                    (scene::place_wing_guns, scene::place_aa_guns),
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
                        battle_view::sync,
                        battle_view::animate_harbour_signals.after(camera::follow),
                        battle_view::fly_pennant,
                    )
                        .chain(),
                    fire_control::place_sectors,
                    minimap::update,
                    (threat_ring::update, threat_ring::tag_ships).after(camera::follow),
                    sound::keep_time,
                    (
                        dockyard::open,
                        ui::rebuild_screen,
                        ui::customise::refresh,
                        ui::options::refresh,
                        dockyard::dimension,
                        dockyard::place,
                        ui::instruments,
                        captain::react,
                        mission_hud::update,
                        mission_hud::spotter_alerts,
                        flagship_hud::update,
                        ui::gauges,
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
        let passage = Passage::of(run.stage);
        let chart = match run.mode {
            Mode::Trial => {
                *self.course = TrialCourse::default();
                shipvivor_encounter::Chart::classic(self.course.gates().map(|g| g.center))
            }
            Mode::Patrol => passage.chart(run.seed),
            Mode::Title | Mode::Paused => passage.classic(),
        };
        *self.encounter = Encounter(passage.battle(&chart).with_rewards(self.options.rewards));
        if run.mode == Mode::Patrol
            && run
                .passage
                .is_some_and(shipvivor_campaign::Stage::enters_air_cover)
        {
            use shipvivor_encounter::AirRaid;
            self.encounter
                .open_air_station(if run.stage == shipvivor_campaign::Stage::CrownDeep {
                    AirRaid::Bomb
                } else {
                    AirRaid::Strafe
                });
        }
        self.encounter.fit_player(run);
        if run.mode == Mode::Title {
            *self.helm = PlayerHelm::cruising();
        } else {
            *self.helm = PlayerHelm::default();
            self.helm.heading = shipvivor_compass::heading(chart.gates[0]);
            if run.mode == Mode::Patrol {
                self.helm.ring(2);
            }
        }
        self.guns.torpedo_mode = false;
        self.guns.launch_pending = false;
        self.guns.volley = None;
        // A new run starts without AP. A fitted ship keeps her selection between legs.
        if !run.fittings.armor_piercing() {
            self.guns.ammo = Ammo::He;
        }
        let flagship = self
            .encounter
            .raiders()
            .iter()
            .find(|r| r.class == shipvivor_encounter::EnemyClass::Regent);
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
        (Screen::Departure, Action::Depart(stage)) if stage.is_opening() => {
            *run = Campaign::new(Mode::Patrol);
            run.stage = stage;
            run.origin = stage;
            run.seed = voyage.seeds.draw();
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
        (Screen::Paused, Action::Extract) if run.can_extract() => {
            run.mode = Mode::Patrol;
            run.extract();
        }
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
        (Screen::Drydock(_), Action::Preview(index)) => run.preview_option(index),
        (Screen::Drydock(_), Action::Draft(job)) => run.preview_draft(job),
        (Screen::Drydock(_), Action::Confirm) => {
            let Some(job) = run.dock_job() else { return };
            if run.refit_in_dock(job) {
                use shipvivor_campaign::DockJob;
                // An order spent is something gained. Shifting a bay about is not.
                if job.cost() > 0 {
                    voyage
                        .commands
                        .trigger(sound::Notice(shipvivor_acoustics::Sound::Upgrade));
                }
                if let DockJob::Move { from, to } = job {
                    // The selection travels with the bay, ready to move it again.
                    run.select_section(to);
                    let again = DockJob::Move {
                        from: to,
                        to: if to < from {
                            to.wrapping_sub(1)
                        } else {
                            to + 1
                        },
                    };
                    run.dock_option = run.dock_jobs().iter().position(|&j| j == again);
                } else if let DockJob::Insert { at, .. } = job {
                    // The new section is selected, so its own refits are offered next.
                    run.select_section(at);
                } else {
                    run.dock_option = None;
                    run.dock_draft = None;
                }
                voyage.encounter.fit_player(&run);
            }
        }
        (Screen::Drydock(_), Action::Sail) => {
            run.leave_dock();
        }
        (Screen::Intercept(_), Action::Engage) if run.engage_regent() => voyage.outfit(&mut run),
        (Screen::Chart, Action::Depart(next)) if run.depart(next) => voyage.outfit(&mut run),
        _ => {}
    }
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
    mut guns: ResMut<Guns>,
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
    // Q changes what the crew loads next. Rounds already in the barrels stay.
    if live && keys.just_pressed(KeyCode::KeyQ) && run.fittings.armor_piercing() {
        guns.ammo = match guns.ammo {
            Ammo::He => Ammo::Ap,
            Ammo::Ap => Ammo::He,
        };
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
) {
    let (mut guns, mut view) = sight;
    let dt = clock.delta_secs();
    let steering = i32::from(keys.pressed(KeyCode::KeyD)) - i32::from(keys.pressed(KeyCode::KeyA));
    let previous = helm.position;
    // Stopped engines and flooded modules take way off her.
    let pace = run.fittings.speed_factor() * run.ship.propulsion();
    let design = run.ship.design();
    let speed = design.top_speed() / shipvivor_navigation::Helm::FULL_AHEAD;
    helm.tick_with_handling(steering as f32, dt, speed * pace, design.turn_rate());
    let hull = run.hull_shape();
    let patrol = run.screen() == Screen::Sailing;
    if patrol {
        // The current sets her along with the water, whatever her engines are doing.
        let set = encounter.current(helm.position) * dt;
        helm.position += set;
        let afloat = encounter.navigable_hull(helm.position, helm.forward(), hull);
        if afloat != helm.position {
            helm.position = afloat;
            helm.speed *= 0.9;
        }
    }
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
        if let Some(volley) = guns.volley.take() {
            encounter.order_volley(volley);
        }
        encounter.update_mount_service(&run.ship);
        let ev = encounter.step(
            dt,
            &Orders {
                player: helm.position,
                heading: helm.heading,
                speed: helm.speed,
                hull,
                aim,
                shell: MainShell {
                    ammo: if run.fittings.armor_piercing() {
                        guns.ammo
                    } else {
                        Ammo::He
                    },
                    ..run.fittings.round()
                },
                reload: run.fittings.reload(),
            },
        );
        // Hulls that met this tick push apart, and the blow takes way off the ship.
        helm.position += ev.shove;
        helm.speed += ev.jolt;
        // The scoped sight rides with her contact through the tick.
        view.ride_zero(helm.position, &encounter);
        view.update_contact(helm.position, &encounter);
        if guns.torpedo_mode && std::mem::take(&mut guns.launch_pending) {
            encounter.launch_spread(helm.position, helm.heading, view.bearing);
        }
        run.hits += u32::from(ev.hits);
        run.sunk += u32::from(ev.sunk);
        run.watch.observe(dt, &ev);
        // Each hit lands on the plating it struck and the module behind it, and on the
        // engines and magazines in its bays only if it got that far in.
        for blow in &ev.blows {
            run.strike_side(blow.aft, blow.abeam, blow.damage, blow.calibre, blow.reach);
        }
        let ruined_yards = encounter.ruined_yards();
        run.tick_engagement(
            dt,
            Objectives {
                kills: ev.sunk,
                crates: ev.recovered,
                destroyed_drydock: ruined_yards > 0,
            },
            0,
            if ev.regent_sunk {
                Some(shipvivor_campaign::LegEnd::RegentSunk)
            } else if ruined_yards == encounter.ports().len() {
                Some(shipvivor_campaign::LegEnd::BasesDestroyed)
            } else {
                None
            },
        );
        if matches!(
            run.screen(),
            Screen::Drydock(shipvivor_campaign::LegEnd::BasesDestroyed)
                | Screen::Intercept(shipvivor_campaign::LegEnd::BasesDestroyed)
                | Screen::Won(shipvivor_campaign::LegEnd::RegentSunk)
        ) {
            run.clearance_left = if ev.regent_sunk { 5. } else { 2.2 };
        }
    }
}
