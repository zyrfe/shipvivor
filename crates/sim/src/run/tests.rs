//! Whole runs: what she buys at the yard and which passage she takes.
use super::*;
use shipvivor_campaign::Objectives;

/// A run whose first leg has just been won with every goal met and three yards down,
/// so she is in the yard with its points to spend.
fn in_the_yard(seed: u64) -> Run {
    let mut run = Run::default();
    run.seed = seed;
    let goals = run.stage.mission();
    let gains = Objectives {
        kills: goals.kills,
        crates: goals.crates,
        destroyed_drydock: true,
        yards: 3,
        ..Default::default()
    };
    run.tick_sector(1., gains, 0, true);
    while run.fittings_remaining() > 0 {
        let card = Bot::choose(&run.offers()).unwrap();
        run.install(card);
    }
    assert!(matches!(run.phase(), Phase::Drydock(_)));
    run
}

#[test]
fn the_yard_rebores_her_guns_first_and_spends_her_points_down() {
    let mut run = in_the_yard(7);
    let points = run.points();
    assert!(points > 0);
    let bore = |run: &Run| {
        (run.ship.design().modules().iter())
            .filter(|m| m.fit.contains(&shipvivor_shipyard::Component::MainGun))
            .map(|m| m.gunhouse.bore)
            .max()
    };
    let before = bore(&run);
    let refit = refit(&mut run);
    assert!(!refit.jobs.is_empty());
    assert_eq!(refit.spent + refit.left, points);
    assert_eq!(refit.left, run.points());
    // The next step of bore is the first thing she wants, and a stock ship's points
    // run to it.
    assert!(matches!(
        refit.jobs[0],
        DockJob::Calibre { from: 4, to: 6, .. }
    ));
    assert!(bore(&run) > before);
    // Nothing she wants and could still pay for is left on the yard's list.
    let wanted: Vec<DockJob> = offered(&run)
        .into_iter()
        .filter(|&job| want(&run, job).is_some())
        .collect();
    let mut plan = Plan::draw(&run);
    for job in wanted {
        assert!(!plan.order(&mut run, job, None), "{job:?} left unbought");
    }
}

#[test]
fn she_takes_a_tough_passage_when_her_stars_open_one_and_she_wants_it() {
    let mut run = in_the_yard(11);
    assert_eq!(run.stars(), 3);
    assert!(run.leave_dock());
    assert_eq!(run.phase(), Phase::Chart);
    let tough = passage(&run, Lanes::Tough).unwrap();
    assert_eq!(tough.grade, Grade::Tough);
    let moderate = passage(&run, Lanes::Moderate).unwrap();
    assert_eq!(moderate.grade, Grade::Moderate);
    assert!(run.depart(tough.stage));
    assert_eq!(run.grade, Grade::Tough);
    assert_eq!(run.stage.round(), 2);
}

#[test]
fn want_never_buys_a_step_down_a_move_or_a_bare_bay() {
    let run = in_the_yard(3);
    for job in offered(&run) {
        let downgrade = match job {
            DockJob::Calibre { from, to, .. } => to <= from,
            DockJob::Barrels { from, to, .. } => to <= from,
            DockJob::Move { .. } | DockJob::Barbette { .. } => true,
            DockJob::Insert {
                module: DockModule::Reserve,
                ..
            } => true,
            _ => false,
        };
        if downgrade {
            assert_eq!(want(&run, job), None, "{job:?}");
        }
    }
}

#[test]
fn a_veteran_run_stands_at_the_start_of_its_round_with_what_she_earned_before_it() {
    let (run, seed) = veteran(1, 2, Skill::SHARP, 8).expect("a captain reaches round 2");
    assert_eq!(run.seed, seed);
    assert_eq!(run.stage.round(), 2);
    assert!(matches!(run.phase(), Phase::Sailing | Phase::Refit));
    // She sailed the first round for real: her level and her ship show it.
    assert!(run.level > 1);
    assert_eq!(run.course.len(), 1);
    assert!(run.ship.design().gun_stations().len() >= 2);
}
