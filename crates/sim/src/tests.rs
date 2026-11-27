use crate::bot::Temperament;
use crate::leg::{Outcome, Setup, sail_for};
use shipvivor_campaign::{Grade, Stage};

#[test]
fn a_leg_sails_to_its_limit_and_the_same_seed_sails_it_again_exactly() {
    // A minute of the leg keeps this short. Nothing ends it but winning or sinking.
    let setup = Setup {
        stage: Stage::IronNarrows,
        grade: Grade::Moderate,
        seed: 7,
        temperament: Temperament::Cautious,
    };
    let first = sail_for(setup, 60.);
    assert_eq!(first.outcome, Outcome::Survived, "{}", first.log());
    assert!((59. ..61.).contains(&first.sailed), "{}", first.sailed);
    assert!(first.fired > 0, "she fought on the way");
    let again = sail_for(setup, 60.);
    assert_eq!(first.log(), again.log(), "the simulation is deterministic");
}

/// No boss in an anchorage, the Commodore in the second and third rounds, the
/// Parliament after, and the Regent at Crown Deep. Each sails from her yard except the
/// Regent, who is at sea from the start. A tough passage starts its raids further along
/// and pays the most, and every sea of a round pays alike at each grade: the two tough
/// passages a yard charts pay the same, and the moderate one about four fifths of that.
#[test]
fn each_round_builds_to_its_boss_and_a_tough_passage_comes_sooner_and_pays_more() {
    use crate::{Sector, TOUGH_HEAD_START};
    use shipvivor_campaign::{MAX_STARS, POOLS};
    use shipvivor_encounter::{EnemyClass, Sortie};
    for (round, pool) in POOLS.iter().enumerate() {
        let boss = match round + 1 {
            1 => None,
            2 | 3 => Some(EnemyClass::Commodore),
            _ => Some(EnemyClass::Parliament),
        };
        let mut pay = Vec::new();
        for &sea in *pool {
            let moderate = Sector::of(sea, Grade::Moderate);
            let tough = Sector::of(sea, Grade::Tough);
            let battle = moderate.battle(&moderate.chart(40));
            assert_eq!(battle.boss(), boss, "{sea:?}");
            assert_eq!(battle.flagship().is_some(), boss.is_some(), "{sea:?}");
            assert!(matches!(battle.flagship(), None | Some(Sortie::Due(_))));
            let extra = if round > 0 { TOUGH_HEAD_START } else { 0. };
            assert_eq!(tough.head_start(), moderate.head_start() + extra, "{sea:?}");
            for (grade, sector) in [(Grade::Moderate, &moderate), (Grade::Tough, &tough)] {
                assert_eq!(sector.boss(), boss, "{sea:?}");
                // Every star, the clock's star's worth, and every base.
                assert_eq!(
                    sector.most_points(),
                    sea.star_points(grade) * (u16::from(MAX_STARS) + 1) + sector.bases() as u16,
                    "{sea:?}"
                );
            }
            pay.push((moderate.most_points(), tough.most_points()));
        }
        assert!(
            pay.windows(2).all(|w| w[0] == w[1]),
            "round {}: {pay:?}",
            round + 1
        );
        let (moderate, tough) = pay[0];
        if round == 0 {
            assert_eq!(moderate, tough);
        } else {
            let share = f32::from(moderate) / f32::from(tough);
            assert!(
                (0.78..=0.82).contains(&share),
                "round {}: {pay:?}",
                round + 1
            );
        }
    }
    let deep = Sector::of(Stage::CrownDeep, Grade::Tough);
    assert_eq!(deep.boss(), Some(EnemyClass::Regent));
    assert_eq!(
        deep.head_start(),
        Sector::of(Stage::CrownDeep, Grade::Moderate).head_start()
    );
    let battle = deep.battle(&deep.chart(40));
    assert_eq!(battle.boss(), Some(EnemyClass::Regent));
    // She holds back over the horizon while her screen comes on.
    assert!(battle.boss_ship().is_none() && battle.flagship() == Some(Sortie::Holding));
}

/// Every sea of every pool charts from every run seed, every sea of a round holds as
/// many bases, they grow round by round, and the raids begin further along the deeper
/// the round.
#[test]
fn every_sea_of_every_pool_charts_and_grows_harder_round_by_round() {
    use crate::Sector;
    use shipvivor_campaign::{POOLS, ROUNDS};
    use shipvivor_encounter::Survey;
    let seas = POOLS
        .into_iter()
        .flatten()
        .copied()
        .chain([Stage::CrownDeep]);
    let mut last = (0, 0, -1.);
    for sea in seas {
        let sector = Sector::of(sea, Grade::Moderate);
        // Three seeds a sea keeps this quick with sixty-odd seas. A release sweep of
        // forty seeds a sea found none falling back when the pools were filled.
        for seed in 40..43 {
            let charted = sector.charted(seed);
            assert!(charted.score.is_some(), "{sea:?} run {seed} fell back");
            let survey = Survey::of(&charted.chart);
            assert!(survey.passed(), "{sea:?} run {seed}: {:?}", survey.defects);
        }
        let round = sea.round();
        let bases = sector.bases();
        if sea != Stage::CrownDeep {
            assert!((3..=5).contains(&bases), "{sea:?} has {bases} bases");
        }
        // Crown Deep starts as far along as the last patrol before it.
        if round > last.0 {
            if sea == Stage::CrownDeep {
                assert_eq!(sector.head_start(), last.2);
            } else {
                assert!(sector.head_start() > last.2, "{sea:?}");
                assert!(bases >= last.1, "{sea:?}");
            }
            last = (round, bases, sector.head_start());
        } else {
            assert_eq!((bases, sector.head_start()), (last.1, last.2), "{sea:?}");
        }
        assert_eq!(sector.head_start() == 0., sea.is_opening(), "{sea:?}");
        assert!(round <= ROUNDS);
    }
}

/// A pilot's note names the channel and the sea in it, short enough for a card.
#[test]
fn every_sea_has_a_short_note_and_each_pool_holds_more_than_one_kind_of_channel() {
    use crate::Sector;
    use shipvivor_campaign::POOLS;
    for pool in POOLS {
        let mut kinds = Vec::new();
        for &sea in pool {
            let sector = Sector::of(sea, Grade::Tough);
            let note = sector.note();
            assert!(note.len() <= 26, "{sea:?}: {note}");
            if !kinds.contains(&sector.kind()) {
                kinds.push(sector.kind());
            }
        }
        assert!(kinds.len() >= 3, "{pool:?}: {kinds:?}");
    }
}

/// The shore guns arm up round by round: singles in the first round, twins in the
/// second, triples in the third, and the heavy yard's quad from the fourth on.
#[test]
fn shore_guns_mount_no_more_barrels_than_the_round_allows() {
    use crate::Sector;
    use shipvivor_campaign::POOLS;
    for (round, pool) in POOLS.iter().enumerate() {
        let most = (round + 1).min(4) as u8;
        for &sea in *pool {
            let sector = Sector::of(sea, Grade::Moderate);
            let battle = sector.battle(&sector.chart(40));
            let barrels: Vec<u8> = battle
                .ports()
                .iter()
                .flat_map(|port| port.turrets().iter().map(|gun| gun.barrels))
                .collect();
            assert!(barrels.iter().all(|&b| b <= most), "{sea:?}: {barrels:?}");
            // Every sea ends at a heavy yard, so the cap is met there.
            assert_eq!(barrels.iter().max(), Some(&most), "{sea:?}: {barrels:?}");
            for port in battle.ports() {
                for gun in port.turrets() {
                    assert_eq!(gun.strength(), gun.max_strength(), "{sea:?}");
                }
            }
        }
    }
}

#[test]
fn rounds_five_and_six_end_at_an_industrial_yard_with_open_water() {
    use crate::Sector;
    use shipvivor_campaign::POOLS;
    use shipvivor_encounter::IndustrialYard;
    for stage in POOLS
        .into_iter()
        .flatten()
        .copied()
        .chain([Stage::CrownDeep])
    {
        let expected = match stage.round() {
            5 => Some(IndustrialYard::RedShift),
            6 => Some(IndustrialYard::BellWorks),
            _ => None,
        };
        let sector = Sector::of(stage, Grade::Moderate);
        let fallback = sector.classic();
        let layouts = |chart: &shipvivor_encounter::Chart| {
            chart
                .harbours
                .iter()
                .map(|site| IndustrialYard::from_works(site.works))
                .collect::<Vec<_>>()
        };
        assert_eq!(layouts(&fallback).last(), Some(&expected), "{stage:?}");
        if expected.is_none() {
            assert!(
                sector
                    .plan()
                    .iter()
                    .flat_map(|s| s.iter())
                    .all(|&w| IndustrialYard::from_works(w).is_none())
            );
            continue;
        }
        let chart = sector.charted(40);
        assert!(chart.score.is_some(), "{stage:?} used its fallback");
        let yards = layouts(&chart.chart);
        assert!(yards[..yards.len() - 1].iter().all(Option::is_none));
        let battle = sector.battle(&chart.chart);
        let yard = battle.ports().last().unwrap();
        assert_eq!(yard.industrial_yard(), expected, "{stage:?}");
        let front = yard.body.forward();
        // Cross the mouth on either tack with sea room for a cruiser, clear of the
        // gun skerries, the shore and any random islands added by the generator.
        for across in (-120..=120).step_by(10) {
            let at = yard.body.pos + front * 175. + front.perp() * across as f32;
            assert!(
                battle.navigable_position(at, 10.).distance(at) < 0.1,
                "{stage:?} at {across}"
            );
        }
    }
}
