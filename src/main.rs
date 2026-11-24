mod battle_view;
mod camera;
mod cards;
mod damage_control;
mod dockyard;
mod ensign;
mod fire_control;
mod flagship_hud;
mod fleet_models;
mod gun_sight;
mod gunnery;
mod icefield;
mod insignia;
mod minimap;
mod mission_hud;
mod ordnance;
mod passage;
mod scene;
mod seascape;
mod snapshot;
mod sound;
mod state;
mod surfaces;
#[cfg(test)]
mod tests;
mod ui;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use camera::ShipView;
use passage::Passage;
use shipvivor_campaign::Objectives;
#[cfg(test)]
use shipvivor_encounter::Hull;
use shipvivor_encounter::{Aim, Ammo, MainShell, Orders, Volley};
use shipvivor_navigation::Course;
use state::{
    Campaign, ChartSeeds, Embarked, Encounter, Guns, Mode, PlayerHelm, Screen, TrialCourse,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Shipvivor".into(),
                resolution: (1280, 760).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(seascape::SeascapePlugin)
        .add_plugins(GamePlugin)
        .add_plugins(sound::SoundPlugin)
        .add_plugins(snapshot::SnapshotPlugin)
        .add_plugins(icefield::IcefieldPlugin)
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
            .init_resource::<dockyard::Pointer>()
            .init_resource::<Guns>()
            .init_resource::<ShipView>()
            .init_resource::<seascape::Sea>()
            .init_resource::<sound::Soundboard>()
            .add_observer(seascape::embark)
            .add_observer(sound::hush)
            .add_systems(
                Startup,
                (
                    scene::setup,
                    dockyard::setup,
                    seascape::setup,
                    camera::setup,
                    (|mut helm: ResMut<PlayerHelm>| *helm = PlayerHelm::cruising())
                        .run_if(|run: Res<Campaign>| run.mode == Mode::Title),
                    battle_view::setup,
                    fire_control::setup,
                    insignia::setup,
                    minimap::setup,
                ),
            )
            .add_systems(
                PreUpdate,
                (
                    dockyard::point,
                    ui::intent.pipe(apply_action),
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
                        scene::float_buoys,
                    )
                        .chain(),
                    seascape::follow,
                    seascape::light,
                    scene::place_turrets
                        .run_if(|run: Res<Campaign>| run.screen() != Screen::Secured),
                    scene::place_wing_guns,
                    scene::place_racks,
                    scene::animate_gun_tubes,
                    camera::follow.after(seascape::roll),
                    scene::spray::animate.after(camera::follow),
                    camera::cursor_mode,
                    scene::place_horizon,
                    scene::signals,
                    (
                        ensign::hoist,
                        battle_view::dye_ensigns,
                        battle_view::sync,
                        battle_view::animate_harbour_signals.after(camera::follow),
                        battle_view::fly_pennant,
                    )
                        .chain(),
                    fire_control::place_sectors,
                    minimap::update,
                    sound::keep_time,
                    (
                        dockyard::open,
                        ui::rebuild_screen,
                        dockyard::dimension,
                        dockyard::place,
                        ui::instruments,
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
                    )
                        .chain(),
                    ui::scale,
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
    commands: Commands<'w, 's>,
}
impl Voyage<'_, '_> {
    /// Put the ship at the start of the course for the current mode and stage. A patrol
    /// sails waters charted from the run's seed. The trial and the title keep fixed charts.
    fn outfit(&mut self, run: &Campaign) {
        let passage = Passage::of(run.stage);
        let (course, chart) = match run.mode {
            Mode::Trial => {
                let course = Course::default();
                let chart = shipvivor_encounter::Chart::classic(course.gates().map(|g| g.center));
                (course, chart)
            }
            Mode::Patrol => {
                let chart = passage.chart(run.seed);
                (Passage::course(&chart), chart)
            }
            Mode::Title | Mode::Paused => {
                let chart = passage.classic();
                (Passage::course(&chart), chart)
            }
        };
        *self.course = TrialCourse(course);
        *self.encounter = Encounter(passage.battle(&chart));
        self.encounter.fit_player(run);
        if run.mode == Mode::Title {
            *self.helm = PlayerHelm::cruising();
        } else {
            *self.helm = PlayerHelm::default();
            self.helm.heading = self.course.gates()[0].bearing;
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
    // Any other choice withdraws a last order that is waiting for its second press.
    if action != Action::Confirm && run.dock_armed {
        run.dock_armed = false;
    }
    match (run.screen(), action) {
        (Screen::Title | Screen::Won(_) | Screen::Lost, Action::Start) => {
            *run = Campaign::new(Mode::Patrol);
            run.seed = voyage.seeds.draw();
            voyage.outfit(&run);
        }
        (Screen::Title, Action::Trial) => {
            *run = Campaign::new(Mode::Trial);
            voyage.outfit(&run);
        }
        (Screen::Trial, Action::RestartTrial) => voyage.outfit(&run),
        (Screen::Trial | Screen::Paused | Screen::Won(_) | Screen::Lost, Action::Title) => {
            *run = Campaign::default();
            voyage.outfit(&run);
        }
        (Screen::Secured, Action::Continue) => run.clearance_left = 0.,
        (Screen::Sailing, Action::Pause) => run.mode = Mode::Paused,
        (Screen::Paused, Action::Resume) => run.mode = Mode::Patrol,
        (Screen::Sailing, Action::Extract) => run.extract(),
        (Screen::Refit, Action::Card(card)) => {
            run.install(card);
            voyage.encounter.fit_player(&run);
        }
        (Screen::Drydock(_), Action::Module(index)) => run.select_section(index),
        (Screen::Drydock(_), Action::Preview(index)) if index < run.dock_jobs().len() => {
            run.dock_option = Some(index)
        }
        (Screen::Drydock(_), Action::Confirm) => {
            let Some(job) = run.dock_job() else { return };
            // The last order sails the ship, so its first press only asks for a second.
            if run.closes_dock(job) && job.fits(&run) && !run.dock_armed {
                run.dock_armed = true;
                return;
            }
            run.dock_armed = false;
            if run.refit_in_dock(job) {
                use shipvivor_campaign::DockJob;
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
                } else {
                    run.dock_option = None;
                }
                voyage.encounter.fit_player(&run);
            }
        }
        (Screen::Intercept(_), Action::Engage) if run.engage_regent() => voyage.outfit(&run),
        (Screen::Chart, Action::Depart(next)) if run.depart(next) => voyage.outfit(&run),
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
    let crossed = course
        .bypass_change_detection()
        .advance(previous, helm.position);
    if crossed {
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
        view.update_contact(helm.position, &encounter);
        if guns.torpedo_mode && std::mem::take(&mut guns.launch_pending) {
            encounter.launch_spread(helm.position, helm.heading, view.bearing);
        }
        run.hits += u32::from(ev.hits);
        run.sunk += u32::from(ev.sunk);
        // Each hit lands on the module it struck.
        for blow in &ev.blows {
            run.strike(blow.aft, blow.damage);
        }
        run.tick_engagement(
            dt,
            Objectives {
                kills: ev.sunk,
                crates: ev.recovered,
                gates: u16::from(crossed),
            },
            0,
            if ev.regent_sunk {
                Some(shipvivor_campaign::LegEnd::RegentSunk)
            } else if encounter.ruined_yards() == encounter.ports().len() {
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
