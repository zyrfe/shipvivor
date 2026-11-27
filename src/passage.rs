//! Where each stage is sailed. Its waters, weather and raid pace live in
//! `shipvivor_sim::Sector`, so the game and the simulator sail the same sectors. This
//! adds the seascape's weather and logs how each chart was drawn.
use crate::seascape::Weather;
use bevy::log::{info, warn};
use shipvivor_campaign::{Grade, Stage};
use shipvivor_encounter::{Battle, Chart};
use shipvivor_sim::Sector;

pub struct Passage {
    stage: Stage,
    sector: Sector,
    pub weather: Weather,
}

impl Passage {
    /// The sea `stage` as a moderate passage sails it, which is all the weather and the
    /// hand-set chart need.
    pub fn of(stage: Stage) -> Self {
        Self::graded(stage, Grade::Moderate)
    }

    /// The sea `stage` sailed at `grade`, as the battle there needs it.
    pub fn graded(stage: Stage, grade: Grade) -> Self {
        let sector = Sector::of(stage, grade);
        let shipvivor_sim::Weather {
            sea,
            wind,
            overcast,
        } = sector.weather;
        Self {
            stage,
            sector,
            weather: Weather {
                sea,
                wind,
                overcast,
            },
        }
    }

    /// The same waters with bases that moor and launch `times` the ships, as a
    /// punishing passage sails them.
    pub fn with_strength(mut self, times: usize) -> Self {
        self.sector = self.sector.with_strength(times);
        self
    }

    /// These waters as charted for one run. The same run seed always gives the same chart,
    /// and each leg of a run gets its own. Every candidate is surveyed before it is used,
    /// and if none passes the stage's hand-set chart stands in.
    pub fn chart(&self, run_seed: u64) -> Chart {
        let name = self.stage.mission().name;
        let charted = self.sector.charted(run_seed);
        match charted.score {
            Some(score) => info!(
                "{name}: run seed {run_seed}, chart {:#018x}, survey score {score:.2}",
                charted.seed
            ),
            None => warn!("{name}: no chart from run seed {run_seed} passed its survey"),
        }
        charted.chart
    }

    /// The hand-set chart of this stage.
    pub fn classic(&self) -> Chart {
        self.sector.classic()
    }

    /// The ports, islands and patrols of a chart.
    pub fn battle(&self, chart: &Chart) -> Battle {
        self.sector.battle(chart)
    }

    pub fn sector(&self) -> &Sector {
        &self.sector
    }
}
