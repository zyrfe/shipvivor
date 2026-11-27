//! How heavily a warship of a class is gunned. Her class sets her hull and her calibre;
//! her weight sets how many barrels each of her turrets carries: one on a light ship,
//! two on a standard one, three on a heavy one and four on an ultra heavy one. Only
//! destroyers, cruisers and battleships come in weights. Each round of a run musters
//! them from its own roster, light ones first, so the bigger hulls come into the fight
//! lightly gunned before the heavy ones do.
use super::EnemyClass;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Weight {
    Light,
    Standard,
    Heavy,
    UltraHeavy,
}
use Weight::*;

/// What each round of a run musters, by class: destroyers, cruisers, battleships. A
/// yard's launches and a harbour's squadron of a class take the listed weights in turn.
#[rustfmt::skip]
const ROSTER: [[&[Weight]; 3]; 8] = [
    [&[Light], &[Light], &[Light]],
    [&[Light], &[Light], &[Light]],
    [&[Light], &[Light, Standard], &[Light]],
    [&[Light, Standard], &[Standard, Light], &[Light, Standard]],
    [&[Standard, Light], &[Standard], &[Standard]],
    [&[Standard], &[Standard, Heavy], &[Standard, Heavy]],
    [&[Standard, Heavy], &[Heavy, Standard], &[Heavy, UltraHeavy]],
    [&[Heavy, Standard], &[Heavy], &[UltraHeavy, Heavy]],
];

impl Weight {
    pub const ALL: [Weight; 4] = [Light, Standard, Heavy, UltraHeavy];

    /// Barrels in each of her turrets.
    pub fn barrels(self) -> u8 {
        match self {
            Light => 1,
            Standard => 2,
            Heavy => 3,
            UltraHeavy => 4,
        }
    }
    /// The word before her class's name, if any.
    pub fn prefix(self) -> Option<&'static str> {
        match self {
            Light => Some("Light"),
            Standard => None,
            Heavy => Some("Heavy"),
            UltraHeavy => Some("Ultra Heavy"),
        }
    }
    /// Her full name, given her class's: "Light Cruiser", "Heavy Battleship".
    pub fn name(self, class: &str) -> String {
        match self.prefix() {
            Some(prefix) => format!("{prefix} {class}"),
            None => class.to_owned(),
        }
    }
    /// The weight a ship of `class` carries without a roster: the one whose barrels are
    /// her class's own, so a single-gunned destroyer is a light one and a triple-gunned
    /// battleship a heavy one. Classes that come in no weights are standard.
    pub fn stock(class: EnemyClass) -> Self {
        match (Self::row(class), class.profile().barrels) {
            (None, _) | (_, 2) => Standard,
            (_, 0 | 1) => Light,
            (_, 3) => Heavy,
            _ => UltraHeavy,
        }
    }
    /// Barrels a turret of `class` carries on average in round `round`'s roster, counting
    /// from one: her class's own for a class that comes in no weights.
    pub fn mean_barrels(class: EnemyClass, round: usize) -> f32 {
        let Some(row) = Self::row(class) else {
            return f32::from(class.profile().barrels);
        };
        let weights = ROSTER[round.clamp(1, ROSTER.len()) - 1][row];
        let barrels: u8 = weights.iter().map(|w| w.barrels()).sum();
        f32::from(barrels) / weights.len() as f32
    }
    /// Where `class` stands in a roster's row, if she comes in weights.
    pub(super) fn row(class: EnemyClass) -> Option<usize> {
        match class {
            EnemyClass::Destroyer => Some(0),
            EnemyClass::Cruiser => Some(1),
            EnemyClass::Battleship => Some(2),
            _ => None,
        }
    }
}

/// The weights a sector's warships are drawn in, from its round's roster, and how many
/// of each class it has drawn so far.
#[derive(Clone, Debug)]
pub(super) struct Muster {
    round: usize,
    drawn: [usize; 3],
}
impl Muster {
    /// The roster of round `round`, counting from one. Past the last, the last's.
    pub fn of_round(round: usize) -> Self {
        Self {
            round: round.clamp(1, ROSTER.len()),
            drawn: [0; 3],
        }
    }
    /// The weight of the next ship of `class` in these waters.
    pub fn draw(&mut self, class: EnemyClass) -> Weight {
        let Some(row) = Weight::row(class) else {
            return Standard;
        };
        let weights = ROSTER[self.round - 1][row];
        let weight = weights[self.drawn[row] % weights.len()];
        self.drawn[row] += 1;
        weight
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use EnemyClass::*;

    #[test]
    fn stock_weights_keep_each_class_as_it_was_gunned() {
        for class in EnemyClass::ALL {
            let weight = Weight::stock(class);
            if Weight::row(class).is_some() {
                assert_eq!(weight.barrels(), class.profile().barrels, "{class:?}");
            } else {
                assert_eq!(weight, Standard, "{class:?}");
            }
        }
        assert_eq!(Weight::stock(Destroyer), Light);
        assert_eq!(Weight::stock(Battleship), Heavy);
    }

    #[test]
    fn the_first_round_is_all_light_and_the_heaviest_come_last() {
        let mut first = Muster::of_round(1);
        let mut last = Muster::of_round(8);
        for class in [Destroyer, Cruiser, Battleship] {
            let drawn: Vec<Weight> = (0..6).map(|_| last.draw(class)).collect();
            assert!(drawn.contains(&Heavy), "{class:?}: {drawn:?}");
            assert!(drawn.iter().all(|&w| w >= Standard), "{class:?}: {drawn:?}");
            for _ in 0..6 {
                assert_eq!(first.draw(class), Light);
            }
        }
        assert_eq!(first.draw(Patrol), Standard);
        // Only the battleships of the last rounds mount four guns to a turret.
        for round in 1..=ROSTER.len() {
            let mut muster = Muster::of_round(round);
            for class in [Destroyer, Cruiser, Battleship] {
                let quad = (0..6).any(|_| muster.draw(class) == UltraHeavy);
                assert_eq!(quad, class == Battleship && round >= 7, "{class:?} {round}");
            }
        }
        // Rosters never get lighter from one round to the next.
        for row in 0..3 {
            let heaviest = |round: usize| ROSTER[round][row].iter().max().copied();
            let lightest = |round: usize| ROSTER[round][row].iter().min().copied();
            for round in 1..ROSTER.len() {
                assert!(
                    heaviest(round) >= heaviest(round - 1),
                    "row {row} round {round}"
                );
                assert!(
                    lightest(round) >= lightest(round - 1),
                    "row {row} round {round}"
                );
            }
        }
    }

    use crate::{Battle, Body, Chart, HarbourSite, Raider};
    use glam::Vec2;

    /// What a lone ship of `class` lying still fires in `seconds` at a mark `range`
    /// metres abeam: every round, the rounds of the first salvo, and how far wide of the
    /// mark each round's line of flight runs.
    fn rounds_at(
        class: EnemyClass,
        weight: Weight,
        range: f32,
        seconds: f32,
    ) -> (usize, usize, Vec<f32>) {
        let body = Body {
            id: 7,
            pos: Vec2::ZERO,
            heading: 0.,
        };
        let mut ship = Raider::new(body, class);
        ship.reweigh(weight);
        let mark = ship.body.forward().perp() * range;
        let (mut shots, mut effects, mut serial) = (Vec::new(), Vec::new(), 100);
        let mut salvo = 0;
        for _ in 0..(seconds * 60.) as usize {
            let before = shots.len();
            ship.fire_guns(
                1. / 60.,
                mark,
                Vec2::ZERO,
                &mut shots,
                &mut effects,
                &mut serial,
            );
            if salvo == 0 {
                salvo = shots.len() - before;
            }
        }
        let wide = shots
            .iter()
            .map(|shot| {
                let line = shipvivor_compass::direction(shot.body.heading);
                (mark - shot.origin()).perp_dot(line).abs()
            })
            .collect();
        (shots.len(), salvo, wide)
    }

    #[test]
    fn guns_fire_past_their_true_range_out_to_their_reach_and_no_farther() {
        for class in [Patrol, Destroyer, Cruiser, Battleship] {
            let (range, reach) = (class.profile().range(), class.reach());
            assert!(reach > range * 1.2, "{class:?}");
            let stock = Weight::stock(class);
            for at in [range * 0.9, (range + reach) * 0.5, reach - 12.] {
                assert!(rounds_at(class, stock, at, 24.).0 > 0, "{class:?} at {at}");
            }
            assert_eq!(rounds_at(class, stock, reach + 12., 24.).0, 0, "{class:?}");
        }
        // A torpedo boat's little gun harries only from close aboard.
        assert_eq!(TorpedoBoat.reach(), TorpedoBoat.profile().range());
    }

    #[test]
    fn salvos_laid_true_inside_her_range_go_wide_toward_her_reach() {
        let class = Cruiser;
        let (range, reach) = (class.profile().range(), class.reach());
        let mean = |at: f32| {
            let (_, _, wide) = rounds_at(class, Light, at, 90.);
            assert!(wide.len() >= 10, "{} rounds from {at} m", wide.len());
            wide.iter().sum::<f32>() / wide.len() as f32
        };
        let (near, far) = (mean(range * 0.9), mean(reach - 12.));
        assert!(near < 0.5, "{near} m wide inside her range");
        assert!(far > 3., "{far} m wide at her reach");
    }

    #[test]
    fn each_turret_fires_one_round_for_each_barrel_her_weight_gives_it() {
        for weight in Weight::ALL {
            let (_, salvo, _) = rounds_at(Cruiser, weight, 300., 12.);
            assert_eq!(salvo, usize::from(weight.barrels()), "{weight:?}");
        }
        // A class that comes in no weights keeps her own guns.
        let (_, salvo, _) = rounds_at(Patrol, Heavy, 150., 12.);
        assert_eq!(salvo, usize::from(Patrol.profile().barrels));
    }

    fn open_battle() -> Battle {
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
                works: i as u8 + 1,
                stage: 0,
            })
            .collect();
        let mut battle = Battle::for_chart(1000., &chart);
        battle.raiders.clear();
        battle
    }

    #[test]
    fn a_rounds_roster_guns_the_ships_on_station_and_every_one_launched_after() {
        let at = Vec2::new(1800., 0.);
        let mut early = open_battle();
        for class in [Destroyer, Cruiser, Battleship, Patrol, Commodore] {
            early.spawn(class, at, at + Vec2::X);
        }
        let mut early = early.ships_of_round(1);
        early.spawn(Battleship, at, at + Vec2::X);
        let barrels: Vec<u8> = early.raiders.iter().map(Raider::barrels).collect();
        // The patrol boat and the Commodore keep their own guns.
        assert_eq!(barrels, [1, 1, 1, 1, 3, 1]);
        let mut late = open_battle().ships_of_round(8);
        for _ in 0..3 {
            late.spawn(Battleship, at, at + Vec2::X);
        }
        let weights: Vec<Weight> = late.raiders.iter().map(Raider::weight).collect();
        assert_eq!(weights, [UltraHeavy, Heavy, UltraHeavy]);
        // Without a roster, each class is gunned as she always was.
        let mut stock = open_battle();
        stock.spawn(Battleship, at, at + Vec2::X);
        assert_eq!(stock.raiders[0].barrels(), Battleship.profile().barrels);
    }

    #[test]
    fn names_carry_the_weight_before_the_class() {
        assert_eq!(Light.name("Cruiser"), "Light Cruiser");
        assert_eq!(Standard.name("Cruiser"), "Cruiser");
        assert_eq!(UltraHeavy.name("Battleship"), "Ultra Heavy Battleship");
    }
}
