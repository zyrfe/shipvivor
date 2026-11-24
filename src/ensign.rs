//! The colours each side flies. Breakwater's are drawn from the run's seed, so the title
//! already shows the flag the next run sails under. The enemy hoists fresh colours for
//! every leg, drawn never to be mistaken for hers.
use crate::state::{Campaign, ChartSeeds, Mode};
use bevy::prelude::*;
use shipvivor_encounter::leg_seed;
use shipvivor_ensign::Flag;

/// Legs no stage uses, so neither side's colours share a draw with a chart.
const BREAKWATER: u64 = 0xF1A6;
const RAIDERS: u64 = 0xF1A7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Breakwater,
    Enemy,
}

#[derive(Resource)]
pub struct Colours {
    pub breakwater: Flag,
    pub enemy: Flag,
    drawn: (u64, u64),
}

impl Default for Colours {
    fn default() -> Self {
        Self::drawn((0, 0))
    }
}

impl Colours {
    fn drawn(seeds: (u64, u64)) -> Self {
        let breakwater = Flag::generate(seeds.0);
        let enemy = Flag::rival(seeds.1, &breakwater);
        Self {
            breakwater,
            enemy,
            drawn: seeds,
        }
    }

    pub fn flag(&self, side: Side) -> &Flag {
        match side {
            Side::Breakwater => &self.breakwater,
            Side::Enemy => &self.enemy,
        }
    }
}

/// The seeds of Breakwater's colours and the enemy's for the leg in hand. At the title
/// and in the helm trial no run has been drawn yet, so hers come from the next one.
fn draws(run: &Campaign, next_run: u64) -> (u64, u64) {
    let base = match run.mode {
        Mode::Title | Mode::Trial => next_run,
        Mode::Patrol | Mode::Paused => run.seed,
    };
    let leg = leg_seed(base, run.stage as u64);
    (leg_seed(base, BREAKWATER), leg_seed(leg, RAIDERS))
}

/// Run up new colours when a run or a leg begins.
pub fn hoist(run: Res<Campaign>, seeds: Res<ChartSeeds>, mut colours: ResMut<Colours>) {
    let wanted = draws(&run, seeds.peek());
    if colours.drawn != wanted {
        *colours = Colours::drawn(wanted);
        info!(
            "Breakwater flies {}. The enemy flies {}.",
            colours.breakwater, colours.enemy
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shipvivor_campaign::Stage;

    #[test]
    fn the_title_flies_the_colours_the_next_run_sails_under() {
        let mut seeds = ChartSeeds::fixed(42);
        let title = draws(&Campaign::default(), seeds.peek());
        let trial = draws(&Campaign::new(Mode::Trial), seeds.peek());
        let mut run = Campaign::new(Mode::Patrol);
        run.seed = seeds.draw();
        let sailing = draws(&run, seeds.peek());
        assert_eq!(title.0, sailing.0);
        assert_eq!(trial.0, sailing.0);
        run.mode = Mode::Paused;
        assert_eq!(draws(&run, seeds.peek()), sailing);
        let mut next = Campaign::new(Mode::Patrol);
        next.seed = seeds.draw();
        assert_ne!(draws(&next, seeds.peek()).0, sailing.0);
    }

    #[test]
    fn the_enemy_hoists_new_colours_every_leg() {
        let mut run = Campaign::new(Mode::Patrol);
        run.seed = 7;
        let mut seen = Vec::new();
        for stage in [
            Stage::IronNarrows,
            Stage::OuterRoad,
            Stage::GreywaterSound,
            Stage::PilotsCut,
            Stage::CrownDeep,
        ] {
            run.stage = stage;
            let (ours, theirs) = draws(&run, 0);
            assert_eq!(ours, draws(&run_at(7), 0).0);
            assert!(!seen.contains(&theirs));
            seen.push(theirs);
            let colours = Colours::drawn((ours, theirs));
            assert!(
                colours.enemy.difference(&colours.breakwater) >= shipvivor_ensign::MIN_DIFFERENCE
            );
        }
    }

    fn run_at(seed: u64) -> Campaign {
        let mut run = Campaign::new(Mode::Patrol);
        run.seed = seed;
        run
    }
}
