//! What holds a sector's waters, and so what its harbours moor, what its yards build,
//! how quickly they launch and who sails with its boss. Flotillas are many light hulls
//! in light plate, quick off the slips; a battle line is fewer hulls, more of them
//! cruisers and battleships, whose belts only a big gun's armour-piercing round opens;
//! mixed waters are held as they always were. The same waters may hide a surprise: one
//! stage of its harbours held by a fleet the scouts did not report.
use super::EnemyClass::{self, *};

/// What holds a sector's waters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub enum Doctrine {
    Flotillas,
    #[default]
    Mixed,
    BattleLine,
}
use Doctrine::*;

const P: EnemyClass = Patrol;
const D: EnemyClass = Destroyer;
const C: EnemyClass = Cruiser;
const B: EnemyClass = Battleship;

/// A round's squadrons: lying at the mouth of the channel, between, and at the heavy
/// yard at its head.
type Squadrons = [&'static [EnemyClass]; 3];

/// The squadron lying in each base's harbour when a sector of mixed waters opens, by
/// round and by where the base stands up the channel. The first round's are small, a
/// cruiser and her two destroyers at the head, and the last rounds' are battle fleets.
#[rustfmt::skip]
const MIXED: [Squadrons; 8] = [
    [&super::GARRISON, &[P, P, D], &[D, D, C]],
    [&[P, P], &[D, D, D, C], &[D, D, D, C, C]],
    [&[P, P, D], &[D, D, D, C], &[D, D, D, C, C, B]],
    [&[D, D, C], &[D, D, D, C, C], &[D, D, D, C, C, C, B]],
    [&[D, D, D, C], &[D, D, D, C, C, B], &[D, D, D, D, C, C, C, B, B]],
    [&[D, D, D, C, B], &[D, D, D, C, C, B, B], &[D, D, D, D, C, C, C, B, B, B]],
    [&[D, D, D, C, C, B], &[D, D, D, D, C, C, B, B], &[D, D, D, D, D, C, C, C, C, B, B, B]],
    [&[D, D, D, C, C, B], &[D, D, D, D, C, C, B, B], &[D, D, D, D, D, C, C, C, C, B, B, B]],
];
/// Flotillas: swarms of patrol boats and destroyers, led late on by a cruiser or three.
#[rustfmt::skip]
const FLOTILLAS: [Squadrons; 8] = [
    [&[P, P], &[P, P, P, D], &[D, D, P, P, P]],
    [&[P, P, P], &[D, D, P, P, P], &[D, D, D, D, P, P]],
    [&[D, P, P, P], &[D, D, D, P, P, P], &[D, D, D, D, D, C, P, P]],
    [&[D, D, P, P, P], &[D, D, D, D, P, P], &[D, D, D, D, D, D, C, P, P]],
    [&[D, D, D, P, P], &[D, D, D, D, D, P, P], &[D, D, D, D, D, D, D, C, C, P, P]],
    [&[D, D, D, D, P, P], &[D, D, D, D, D, D, C, P], &[D, D, D, D, D, D, D, D, C, C, P, P]],
    [&[D, D, D, D, D, P, P], &[D, D, D, D, D, D, D, C, P, P],
     &[D, D, D, D, D, D, D, D, D, D, C, C, C, P]],
    [&[D, D, D, D, D, P, P], &[D, D, D, D, D, D, D, C, P, P],
     &[D, D, D, D, D, D, D, D, D, D, C, C, C, P]],
];
/// A battle line: cruisers and battleships with a destroyer or two to screen them. It
/// moors about as many battleships as mixed waters do, but cruisers in place of most of
/// the destroyers, so fewer hulls and more of them heavy.
#[rustfmt::skip]
const BATTLE_LINE: [Squadrons; 8] = [
    [&[D], &[C, D], &[C, C, D]],
    [&[C], &[C, C, D], &[C, C, C, D]],
    [&[C, D], &[C, C, D], &[B, C, C, D]],
    [&[C, C, D], &[B, C, D], &[B, B, C, D]],
    [&[C, C, D], &[B, C, C, D], &[B, B, C, C, D]],
    [&[B, C, C, D], &[B, B, C, D], &[B, B, B, C, C, D]],
    [&[B, C, C, D, D], &[B, B, C, C, D], &[B, B, B, C, C, C, D, D]],
    [&[B, C, C, D, D], &[B, B, C, C, D], &[B, B, B, C, C, C, D, D]],
];

/// The ships that put to sea with a boss, by round, in close order on her. Each is
/// short enough to name on one line of the chart's card ("Escort: a battleship and 2
/// cruisers").
#[rustfmt::skip]
const ESCORTS: [[&[EnemyClass]; 3]; 8] = [
    // Flotillas, mixed, battle line.
    [&[], &[], &[]],
    [&[D, D, D], &[C, D], &[C, C]],
    [&[D, D, D, D], &[C, D, D], &[C, C, D]],
    [&[D, D, D, D], &[C, C, D], &[B, C]],
    [&[D, D, D, D, D], &[C, C, D, D], &[B, C]],
    [&[D, D, D, D, D, D], &[C, C, D, D], &[B, B, C]],
    [&[D, D, D, D, D, D, D], &[B, C, C], &[B, B, C]],
    [&[], &[], &[]],
];

/// A slip's bill of steel for each class the yards build.
const PATROL: (EnemyClass, u8) = (Patrol, 2);
const DESTROYER: (EnemyClass, u8) = (Destroyer, 4);
const CRUISER: (EnemyClass, u8) = (Cruiser, 7);
const BATTLESHIP: (EnemyClass, u8) = (Battleship, super::port::YARD_CAPACITY);

impl Doctrine {
    pub const ALL: [Doctrine; 3] = [Flotillas, Mixed, BattleLine];

    fn table(self) -> &'static [Squadrons; 8] {
        match self {
            Flotillas => &FLOTILLAS,
            Mixed => &MIXED,
            BattleLine => &BATTLE_LINE,
        }
    }
    /// The squadron a base moors in round `round` of a run, counting from one, lying at
    /// `place`: 0 at the mouth, 1 between, 2 the heavy yard.
    pub fn squadron(self, round: usize, place: usize) -> &'static [EnemyClass] {
        self.table()[round.clamp(1, 8) - 1][place.min(2)]
    }
    /// The ships that put to sea with a boss in round `round`.
    pub fn escort(self, round: usize) -> &'static [EnemyClass] {
        ESCORTS[round.clamp(1, 8) - 1][self as usize]
    }
    /// The share of the waters' time between launches its yards keep: flotillas' light
    /// hulls come off the slips quicker.
    pub fn raid_pace(self) -> f32 {
        match self {
            Flotillas => 0.75,
            Mixed | BattleLine => 1.,
        }
    }
    /// Torpedo boats each base moors.
    pub fn torpedo_boats(self) -> usize {
        match self {
            Flotillas => 4,
            Mixed => super::torpedo_boat::MOORED_PAIR,
            BattleLine => 1,
        }
    }
    /// What a yard lays down in turn at escalation `tier`, with `factories` works: patrol
    /// boats first, then destroyers, then the heavy ships. Flotillas never lay down a
    /// battleship and keep laying down patrol boats; a battle line's yards lay down no
    /// more patrol boats once the raids are under way, and cruisers sooner.
    pub fn rotation(self, tier: usize, factories: usize) -> &'static [(EnemyClass, u8)] {
        match (self, tier, factories) {
            (Flotillas, 0, _) => &[PATROL],
            (Flotillas, 1, _) => &[DESTROYER, PATROL, PATROL],
            (Flotillas, _, 1) => &[DESTROYER, PATROL],
            (Flotillas, 2, _) | (Flotillas, _, 2) => &[DESTROYER, DESTROYER, PATROL],
            (Flotillas, ..) => &[DESTROYER, DESTROYER, CRUISER],
            (Mixed, 0, _) => &[PATROL],
            (Mixed, 1, 1) => &[DESTROYER, PATROL],
            (Mixed, 1, _) => &[DESTROYER, DESTROYER, PATROL],
            (Mixed, _, 1) => &[DESTROYER],
            (Mixed, 2, _) | (Mixed, _, 2) => &[CRUISER, CRUISER, DESTROYER],
            (Mixed, ..) => &[BATTLESHIP, BATTLESHIP, CRUISER],
            (BattleLine, 0, _) => &[PATROL],
            (BattleLine, 1, 1) | (BattleLine, _, 1) => &[DESTROYER],
            (BattleLine, 1, _) => &[CRUISER, DESTROYER],
            (BattleLine, 2, _) | (BattleLine, _, 2) => &[CRUISER, CRUISER, DESTROYER],
            (BattleLine, ..) => &[BATTLESHIP, BATTLESHIP, CRUISER],
        }
    }
}

/// A fleet the scouts missed: the squadrons at `place` up the channel (1 between, 2
/// the heavy yard) are `doctrine`'s, not the waters' own. Only a stage that wakes later
/// hides one, so it shows itself when it stirs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Surprise {
    pub place: usize,
    pub doctrine: Doctrine,
}
impl Surprise {
    /// The surprise waters held by `doctrine` may hide, picked by `draw`: a battle line
    /// at the head of flotilla waters, flotillas swarming between a battle line's
    /// harbours, and either in mixed waters.
    pub fn against(doctrine: Doctrine, draw: u64) -> Self {
        match (doctrine, draw % 2) {
            (Flotillas, _) | (Mixed, 0) => Self {
                place: 2,
                doctrine: BattleLine,
            },
            (BattleLine, _) | (Mixed, _) => Self {
                place: 1,
                doctrine: Flotillas,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heft(ships: &[EnemyClass]) -> f32 {
        ships
            .iter()
            .map(|class| match class {
                Patrol => 0.5,
                Destroyer => 1.,
                Cruiser => 2.,
                _ => 4.,
            })
            .sum()
    }

    #[test]
    fn squadrons_grow_up_the_channel_and_round_by_round() {
        for doctrine in Doctrine::ALL {
            for round in 1..=7 {
                let [mouth, between, head] = doctrine.table()[round - 1];
                assert!(heft(mouth) <= heft(between), "{doctrine:?} {round}");
                assert!(heft(between) <= heft(head), "{doctrine:?} {round}");
                for place in 0..3 {
                    let next = doctrine.squadron(round + 1, place);
                    let here = doctrine.squadron(round, place);
                    assert!(heft(next) >= heft(here), "{doctrine:?} {round} {place}");
                    assert!(here.len() <= 25, "more ships than a harbour has berths");
                }
            }
        }
    }

    #[test]
    fn flotillas_send_more_hulls_and_a_battle_line_heavier_ones() {
        for round in 1..=7 {
            for place in 0..3 {
                let [swarm, mixed, line] =
                    Doctrine::ALL.map(|doctrine| doctrine.squadron(round, place));
                assert!(swarm.len() >= mixed.len(), "{round} {place}");
                assert!(line.len() <= mixed.len(), "{round} {place}");
                assert!(!swarm.contains(&Battleship), "{round} {place}");
                // A greater share of a battle line is cruisers and battleships.
                let heavy = |ships: &[EnemyClass]| {
                    (ships.iter())
                        .filter(|class| matches!(class, Cruiser | Battleship))
                        .count()
                };
                assert!(
                    heavy(line) * mixed.len() >= heavy(mixed) * line.len(),
                    "{round} {place}: {line:?}"
                );
            }
        }
        // No flotilla yard ever lays down a battleship, and a battle line's lays down no
        // patrol boats once the raids are under way.
        for tier in 0..4 {
            for works in 1..=3 {
                assert!(!Flotillas.rotation(tier, works).contains(&BATTLESHIP));
                if tier > 0 {
                    assert!(!BattleLine.rotation(tier, works).contains(&PATROL));
                }
            }
        }
        assert!(Mixed.rotation(1, 3).contains(&PATROL));
    }

    #[test]
    fn a_surprise_is_the_fleet_the_waters_did_not_report() {
        for draw in 0..4 {
            for doctrine in Doctrine::ALL {
                let surprise = Surprise::against(doctrine, draw);
                assert_ne!(surprise.doctrine, doctrine);
                assert!(surprise.place >= 1);
            }
        }
    }

    use crate::{Aim, Battle, Chart, HarbourSite, Hull, MainShell, Orders, Weight};
    use glam::Vec2;

    /// Three bases of one works each, a stage apart and far apart, the first where she
    /// enters. `stages` sets each base's stage.
    fn sector(stages: [u8; 3]) -> Battle {
        let mut chart = Chart::classic([
            Vec2::new(0., -152.),
            Vec2::new(168., -424.),
            Vec2::new(496., -300.),
            Vec2::new(416., 76.),
            Vec2::new(72., 164.),
        ]);
        chart.islets.clear();
        chart.harbours = (0..3)
            .map(|i| HarbourSite {
                pos: Vec2::X * (i as f32 * 5000.),
                heading: 0.,
                works: 1,
                stage: stages[i],
            })
            .collect();
        Battle::for_chart(1000., &chart)
    }
    fn moored(b: &Battle, base: usize) -> Vec<EnemyClass> {
        (b.raiders.iter())
            .filter(|r| r.moored_at() == Some(base) && r.attack.is_none())
            .filter(|r| !r.class.is_merchant())
            .map(|r| r.class)
            .collect()
    }
    fn boats(b: &Battle, base: usize) -> usize {
        (b.raiders.iter())
            .filter(|r| r.class == TorpedoBoat && r.moored_at() == Some(base))
            .count()
    }

    #[test]
    fn the_waters_doctrine_moors_its_squadrons_and_torpedo_boats_and_guns_its_ships() {
        for doctrine in Doctrine::ALL {
            let b = sector([0, 1, 2]).with_doctrine(doctrine).with_squadrons(5);
            assert_eq!(b.doctrine(), doctrine);
            // The first base lies where she enters, too close for a mooring.
            for base in 1..3 {
                assert_eq!(moored(&b, base), doctrine.squadron(5, base), "{doctrine:?}");
                assert_eq!(boats(&b, base), doctrine.torpedo_boats(), "{doctrine:?}");
            }
        }
        // Mixed waters are charted as they always were.
        let stock = sector([0, 1, 2]);
        assert_eq!(
            (1..3).map(|base| boats(&stock, base)).collect::<Vec<_>>(),
            (1..3)
                .map(|base| boats(&sector([0, 1, 2]).with_doctrine(Mixed), base))
                .collect::<Vec<_>>()
        );
        // Every doctrine guns its ships from the round's own roster.
        let first_destroyer = |doctrine: Doctrine| {
            let b = (sector([0, 1, 2]).with_doctrine(doctrine))
                .with_squadrons(5)
                .ships_of_round(5);
            b.raiders
                .iter()
                .find(|r| r.class == Destroyer)
                .map(|r| r.weight())
        };
        for doctrine in Doctrine::ALL {
            assert_eq!(first_destroyer(doctrine), Some(Weight::Standard));
        }
    }

    #[test]
    fn a_surprise_squadron_lies_where_the_report_said_otherwise() {
        let surprise = Surprise::against(Flotillas, 0);
        let b = (sector([0, 1, 2]).with_doctrine(Flotillas))
            .with_surprise(surprise)
            .with_squadrons(5);
        assert_eq!(b.surprise(), Some(surprise));
        assert!(b.surprised(2) && !b.surprised(1) && !b.surprised(0));
        assert_eq!(moored(&b, 2), BattleLine.squadron(5, 2));
        assert_eq!(moored(&b, 1), Flotillas.squadron(5, 1));
        // With no base between the mouth and the heavy yard, a surprise meant for
        // between lies at the heavy yard.
        let b = (sector([0, 0, 1]).with_doctrine(BattleLine))
            .with_surprise(Surprise::against(BattleLine, 0))
            .with_squadrons(5);
        assert!(b.surprised(2));
        assert_eq!(moored(&b, 2), Flotillas.squadron(5, 2));
    }

    #[test]
    fn the_boss_puts_to_sea_with_her_escort_in_one_squadron() {
        for doctrine in Doctrine::ALL {
            let mut b = (sector([0, 1, 2]).with_doctrine(doctrine))
                .with_squadrons(4)
                .with_boss(Parliament)
                .escalated(crate::FLAGSHIP_SORTIE);
            assert_eq!(b.escort(), doctrine.escort(4));
            let orders = Orders {
                player: Vec2::new(2500., 3000.),
                heading: 0.,
                speed: 0.,
                hull: Hull::CRUISER,
                aim: Aim::Hold,
                reload: 3.2,
                shell: MainShell::STOCK,
            };
            for _ in 0..((crate::FLAGSHIP_EARLIEST + 2.) * 10.) as usize {
                b.step(0.1, &orders);
            }
            let boss = b.boss_ship().expect("she has sailed").body.id;
            let squadron = (b.squadrons())
                .map(|(ships, _)| ships.to_vec())
                .find(|ships| ships.contains(&boss))
                .expect("she leads a squadron");
            let mut sailed: Vec<EnemyClass> = (squadron.iter())
                .filter(|&&id| id != boss)
                .filter_map(|&id| b.ship(id))
                .map(|r| r.class)
                .collect();
            let mut escort = doctrine.escort(4).to_vec();
            sailed.sort_by_key(|&c| c as usize);
            escort.sort_by_key(|&c| c as usize);
            assert_eq!(sailed, escort, "{doctrine:?}");
        }
    }
}
