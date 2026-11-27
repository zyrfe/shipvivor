//! Par for a round: what a captain who cleared every round before it brings to it, in
//! yard points, in fittings dealt her free, and in level. Test mode's yard charges
//! nothing, so it shows this beside what her work would have cost, to say whether the
//! ship being tried is one a run could have built by then.
//!
//! The yard points are the game's own: each earlier leg cleared with every star, inside
//! its clock and with every base down, as [`Sector::most_points`] counts it. The cargo
//! and the levels are not rules but habits, read on 2026-11-27 from the run logs of the
//! runs that reached Crown Deep, with experience paid direct: legs made 3 to 6 yard
//! points of cargo, and rounds began within a level or two of [`LEVELS`].
use crate::Sector;
use shipvivor_campaign::{Card, DockJob, Grade, PATROLS, POOLS, ROUNDS, Stage};

/// The level she has usually reached at the start of each round, from the first.
pub const LEVELS: [u32; ROUNDS] = [1, 11, 18, 24, 30, 36, 42];
/// Yard points the cargo she sinks usually makes on a leg.
pub const CARGO: u16 = 4;

/// What a run usually brings to a round. See the module's notes for where it comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Par {
    /// The round, from 1 for an anchorage to [`ROUNDS`] for Crown Deep.
    pub round: usize,
    /// Yard points every leg before it has paid.
    pub points: u16,
    /// Fittings dealt her free by then: one for each level gained, and one on arrival by
    /// each tough passage, this round's included.
    pub fittings: u16,
    /// Her level as the round begins.
    pub level: u32,
}
impl Par {
    /// Par for `stage` reached by passages of `grade`.
    pub fn of(stage: Stage, grade: Grade) -> Self {
        Self::at(stage.round(), grade)
    }
    /// Par for `round`, from 1 to [`ROUNDS`], reached by passages of `lanes`. Every sea
    /// of a round holds as many bases, so one stands for them all.
    pub fn at(round: usize, lanes: Grade) -> Self {
        let round = round.clamp(1, ROUNDS);
        let points = (1..round)
            .map(|earlier| Sector::of(POOLS[earlier - 1][0], lanes).most_points() + CARGO)
            .sum();
        let level = LEVELS[round - 1];
        let arrivals = match lanes {
            Grade::Tough => round.min(PATROLS) - 1,
            Grade::Moderate => 0,
        };
        Self {
            round,
            points,
            fittings: (level - 1) as u16 + arrivals as u16,
            level,
        }
    }
    /// The whole of it in yard points, the fittings at the yard's own price for one.
    pub fn worth(&self) -> u16 {
        self.points + self.fittings * DockJob::Fitting(Card::Loader).cost()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn par_grows_round_by_round_and_tough_lanes_bring_more() {
        // An anchorage is sailed by a stock ship.
        let first = Par::of(Stage::LanternBay, Grade::Tough);
        assert_eq!((first.points, first.fittings, first.worth()), (0, 0, 0));
        for lanes in [Grade::Moderate, Grade::Tough] {
            let pars: Vec<Par> = (1..=ROUNDS).map(|round| Par::at(round, lanes)).collect();
            assert!(pars.windows(2).all(|p| p[0].worth() < p[1].worth()));
            assert!(pars.windows(2).all(|p| p[0].level < p[1].level));
        }
        // The first leg pays the same whichever way she goes on: two points a star and
        // the clock, three bases and the cargo. A tough passage deals a fitting as well.
        let (easy, hard) = (Par::at(2, Grade::Moderate), Par::at(2, Grade::Tough));
        assert_eq!((easy.points, hard.points), (15, 15));
        assert_eq!(hard.fittings, easy.fittings + 1);
        // From there a tough passage pays four points a star to a moderate one's three.
        let (easy, hard) = (Par::at(3, Grade::Moderate), Par::at(3, Grade::Tough));
        assert_eq!(hard.points - easy.points, 4);
        // Crown Deep is no passage: it deals nothing on arrival.
        let last = Par::of(Stage::CrownDeep, Grade::Tough);
        assert_eq!(last.round, ROUNDS);
        assert_eq!(last.fittings, (LEVELS[ROUNDS - 1] - 1) as u16 + 5);
        assert_eq!(last.worth(), last.points + 2 * last.fittings);
    }
}
