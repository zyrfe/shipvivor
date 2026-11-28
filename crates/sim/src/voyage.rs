//! The ship at sea, one fixed step at a time, shared by the game and the simulator so
//! that the two cannot sail differently. The game wraps its camera, sound and screens
//! round these calls. The simulator's bot gives the orders instead of the keyboard.
//!
//! A step is [`steer`], then [`engage`], then [`settle`], in that order. The game moves
//! its sight between `engage` and `settle`, because a torpedo spread goes where the
//! sight looks once the battle has moved.
use crate::sector::Sector;
use shipvivor_campaign::{LegEnd, Objectives, Run, Stage};
use shipvivor_encounter::{
    Aim, AirRaid, Battle, Chart, EnemyClass, Events, Hull, Orders, Rewards, Volley,
};
use shipvivor_navigation::Helm;

/// Breakwater's hull as the battle sees it.
pub fn hull_shape(run: &Run) -> Hull {
    Hull::breakwater(run.ship.design())
}

/// Fit the battle's player mounts to her design and fittings.
pub fn fit_player(battle: &mut Battle, run: &Run) {
    // The strike craft are shipped with the design, so they are fitted out first.
    battle.fit_strike(run.fittings.strike_craft());
    battle.fit_design(run.ship.design(), |station| {
        let local = run.fittings.for_station(station);
        (local.round(), local.turrets)
    });
    battle.fit_secondaries(run.fittings.secondaries());
    battle.fit_gunhouses(run.fittings.gunhouses());
    battle.fit_anti_air(run.fittings.anti_air());
    battle.fit_training(run.fittings.training_gear);
    battle.fit_fire_control(run.fire_control());
    battle.fit_torpedoes(run.fittings.torpedoes());
}

/// The battle for a leg sailed on `chart`, with her mounts fitted. A patrol through
/// enemy air cover opens its air station: bombers at Crown Deep, strafers elsewhere.
pub fn open_battle(
    sector: &Sector,
    chart: &Chart,
    run: &Run,
    rewards: Rewards,
    patrol: bool,
) -> Battle {
    let mut battle = sector.battle(chart).with_rewards(rewards);
    if patrol && run.under_air_cover() {
        battle.open_air_station(if run.stage == Stage::CrownDeep {
            AirRaid::Bomb
        } else {
            AirRaid::Strafe
        });
    }
    fit_player(&mut battle, run);
    battle
}

/// Her helm at the start of a leg: bound for the first mark, or for the Regent when she
/// is at sea, and at half ahead on patrol.
pub fn helm_at_start(chart: &Chart, battle: &Battle, patrol: bool) -> Helm {
    let mut helm = Helm::default();
    helm.heading = shipvivor_compass::heading(chart.gates[0]);
    if patrol {
        helm.ring(2);
    }
    if let Some(regent) = battle
        .raiders()
        .iter()
        .find(|r| r.class == EnemyClass::Regent)
    {
        helm.heading = shipvivor_compass::heading(regent.body.pos - helm.position);
    }
    helm
}

/// Answer the helm for one step, with `rudder` from -1, hard to port, to 1, hard to
/// starboard. At sea the current sets her along and the shore holds her off, taking
/// way off her when it does. Returns how far the shore pushed her off this step, if
/// it did.
pub fn steer(
    run: &Run,
    battle: &Battle,
    helm: &mut Helm,
    rudder: f32,
    dt: f32,
    at_sea: bool,
) -> Option<glam::Vec2> {
    // Stopped engines and flooded modules take way off her.
    let pace = run.fittings.speed_factor() * run.ship.propulsion();
    let design = run.ship.design();
    let speed = design.top_speed() / Helm::FULL_AHEAD;
    helm.tick_with_handling(rudder, dt, speed * pace, design.turn_rate());
    if at_sea {
        // The current sets her along with the water, whatever her engines are doing.
        let set = battle.current(helm.position) * dt;
        helm.position += set;
        let afloat = battle.navigable_hull(helm.position, helm.forward(), hull_shape(run));
        if afloat != helm.position {
            let push = afloat - helm.position;
            helm.position = afloat;
            helm.speed *= 0.9;
            return Some(push);
        }
    }
    None
}

/// Lay the guns on `aim`, fire any `volley` ordered, and advance the battle one step.
/// Hulls that met this step push apart, and the blow takes way off her.
pub fn engage(
    run: &Run,
    battle: &mut Battle,
    helm: &mut Helm,
    dt: f32,
    aim: Aim,
    volley: Option<Volley>,
) -> Events {
    if let Some(volley) = volley {
        battle.order_volley(volley);
    }
    battle.update_mount_service(&run.ship);
    let ev = battle.step(
        dt,
        &Orders {
            player: helm.position,
            heading: helm.heading,
            speed: helm.speed,
            hull: hull_shape(run),
            aim,
            shell: run.fittings.round(),
            reload: run.fittings.reload(),
        },
    );
    helm.position += ev.shove;
    helm.speed += ev.jolt;
    ev
}

/// Launch a torpedo spread along `torpedo`, a bearing, if one was ordered, then land the
/// step's blows on her and run the leg's clock and goals. True if a spread went.
pub fn settle(
    run: &mut Run,
    battle: &mut Battle,
    helm: &Helm,
    dt: f32,
    ev: &Events,
    torpedo: Option<f32>,
) -> bool {
    let launched =
        torpedo.is_some_and(|bearing| battle.launch_spread(helm.position, helm.heading, bearing));
    // Each hit lands on the plating it struck and the module behind it, and on the
    // engines and magazines in its bays only if it got that far in. A torpedo lets the
    // sea in as well.
    for blow in &ev.blows {
        run.strike_side(blow.aft, blow.abeam, blow.damage, blow.calibre, blow.reach);
        if blow.breach {
            run.breach(blow.aft);
        }
    }
    let ruined_yards = battle.ruined_yards();
    // Gains are what this step added: the yards that came down since the last count.
    let fallen = u16::try_from(ruined_yards)
        .unwrap_or(u16::MAX)
        .saturating_sub(run.objectives.yards);
    run.tick_engagement(
        dt,
        Objectives {
            // A coaster pays experience of her own and is not counted among the raiders.
            kills: ev.sunk.saturating_sub(ev.coasters),
            crates: ev.recovered,
            // A drydock brought down earns the goal, even if its factories raise it again.
            destroyed_drydock: ruined_yards > 0 || ev.docks_felled > 0,
            yards: fallen,
            coasters: ev.coasters,
            cargo: ev.cargo,
            planes: ev.planes_downed,
            guns: ev.guns_silenced,
            vehicles: ev.vehicles,
        },
        0,
        if ev.regent_sunk {
            Some(LegEnd::RegentSunk)
        } else if ruined_yards == battle.ports().len() && battle.boss_ship().is_none() {
            // A boss once at sea must be sunk as well before the patrol is won.
            Some(LegEnd::BasesDestroyed)
        } else {
            None
        },
    );
    // A torpedo boat sunk was counted among the raiders. She pays her own experience
    // instead, and earns no star.
    run.sink_torpedo_boats(ev.torpedo_boats);
    launched
}
