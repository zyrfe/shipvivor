use crate::bot::Temperament;
use crate::leg::{Outcome, Setup, sail_for};
use shipvivor_campaign::Stage;

#[test]
fn a_leg_sails_to_its_limit_and_the_same_seed_sails_it_again_exactly() {
    // A minute of the leg keeps this short. Nothing ends it but winning or sinking.
    let setup = Setup {
        stage: Stage::IronNarrows,
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
/// Regent, who is at sea from the start. The hard passage of a round starts its raids
/// further along, and pays the most.
#[test]
fn each_round_builds_to_its_boss_and_the_hard_passage_comes_sooner_and_pays_more() {
    use crate::{HARD_PASSAGE_HEAD_START, Sector};
    use shipvivor_campaign::{MAX_STARS, SEAS};
    use shipvivor_encounter::{EnemyClass, Sortie};
    for (round, seas) in SEAS.iter().enumerate() {
        let boss = match round + 1 {
            1 => None,
            2 | 3 => Some(EnemyClass::Commodore),
            _ => Some(EnemyClass::Parliament),
        };
        for (lane, &sea) in seas.iter().enumerate() {
            let sector = Sector::of(sea);
            assert_eq!(sector.boss(), boss, "{sea:?}");
            let battle = sector.battle(&sector.chart(40));
            assert_eq!(battle.boss(), boss, "{sea:?}");
            assert_eq!(battle.flagship().is_some(), boss.is_some(), "{sea:?}");
            assert!(matches!(battle.flagship(), None | Some(Sortie::Due(_))));
            let open = Sector::of(seas[0]);
            let extra = if round > 0 && lane == 2 {
                HARD_PASSAGE_HEAD_START
            } else {
                0.
            };
            assert_eq!(sector.head_start(), open.head_start() + extra, "{sea:?}");
            // Every star, the clock's star's worth, and every base.
            assert_eq!(
                sector.most_points(),
                sea.star_points() * (u16::from(MAX_STARS) + 1) + sector.bases() as u16,
                "{sea:?}"
            );
        }
        if round > 0 {
            let points = seas.map(|sea| Sector::of(sea).most_points());
            assert!(points[0] < points[1] && points[1] < points[2], "{points:?}");
        }
    }
    let deep = Sector::of(Stage::CrownDeep);
    assert_eq!(deep.boss(), Some(EnemyClass::Regent));
    let battle = deep.battle(&deep.chart(40));
    assert_eq!(battle.boss(), Some(EnemyClass::Regent));
    assert!(battle.boss_ship().is_some() && battle.flagship().is_none());
}

/// Every sea of a run charts from every run seed, its bases grow round by round, and
/// its raids begin further along the deeper it lies.
#[test]
fn every_sea_of_the_chain_charts_and_grows_harder_round_by_round() {
    use crate::Sector;
    use shipvivor_campaign::{ROUNDS, SEAS};
    use shipvivor_encounter::Survey;
    let seas = SEAS.iter().flatten().copied().chain([Stage::CrownDeep]);
    let mut last = (0, 0, -1.);
    for sea in seas {
        let sector = Sector::of(sea);
        for seed in 40..46 {
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
        }
        assert_eq!(sector.head_start() == 0., sea.is_opening(), "{sea:?}");
        assert!(round <= ROUNDS);
    }
}

/// The shore guns arm up round by round: singles in the first round, twins in the
/// second, triples in the third, and the heavy yard's quad from the fourth on.
#[test]
fn shore_guns_mount_no_more_barrels_than_the_round_allows() {
    use crate::Sector;
    use shipvivor_campaign::SEAS;
    for (round, seas) in SEAS.iter().enumerate() {
        let most = (round + 1).min(4) as u8;
        for &sea in seas {
            let sector = Sector::of(sea);
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
