//! Where each stage is sailed: what its waters are like, its weather and how often raiders
//! put to sea. The campaign crate decides progression, and navigation and encounter take
//! plain geometry and timing, so this is the one place that ties a stage to both.
use crate::seascape::Weather;
use bevy::log::{info, warn};
use bevy::math::Vec2;
use shipvivor_campaign::Stage;
use shipvivor_encounter::{Battle, Chart, Waters, leg_seed};
use shipvivor_navigation::Course;

pub struct Passage {
    stage: Stage,
    /// What the waters are like. Each run charts them afresh from its seed.
    waters: Waters,
    /// The hand-set buoy circuit, sailed if no generated chart passes its survey.
    waypoints: [Vec2; 5],
    bearing: f32,
    spacing: f32,
    raid_interval: f32,
    pub weather: Weather,
}

impl Passage {
    pub fn of(stage: Stage) -> Self {
        let (waypoints, degrees, spacing, raid_interval): ([(f32, f32); 5], f32, f32, f32) =
            match stage {
                Stage::IronNarrows => (
                    [
                        (0., -38.),
                        (42., -106.),
                        (124., -75.),
                        (104., 19.),
                        (18., 41.),
                    ],
                    0.,
                    1.,
                    7.,
                ),
                Stage::OuterRoad | Stage::CrownDeep => (
                    [
                        (0., -44.),
                        (-62., -110.),
                        (-145., -80.),
                        (-136., 14.),
                        (-50., 58.),
                    ],
                    -30.,
                    1.25,
                    5.5,
                ),
                Stage::GreywaterSound => (
                    [
                        (0., -42.),
                        (57., -85.),
                        (16., -148.),
                        (-47., -112.),
                        (-61., -36.),
                    ],
                    38.,
                    1.,
                    6.,
                ),
                Stage::PilotsCut => (
                    [
                        (0., -38.),
                        (-45., -94.),
                        (10., -134.),
                        (53., -85.),
                        (29., -34.),
                    ],
                    -58.,
                    0.82,
                    4.5,
                ),
            };
        // Sea state, the compass heading the waves run toward, and cloud.
        let (sea, wind, overcast): (f32, f32, f32) = match stage {
            Stage::IronNarrows => (0.34, 125., 0.),
            // Open water, with a long swell running in under a clearing sky.
            Stage::CrownDeep => (0.48, 76., 0.24),
            Stage::OuterRoad => (0.62, 100., 0.3),
            // Sheltered and flat under low cloud.
            Stage::GreywaterSound => (0.18, 160., 0.85),
            // The short way home lies through the weather.
            Stage::PilotsCut => (1., 140., 1.),
        };
        let waters = match stage {
            Stage::IronNarrows => Waters::NARROWS,
            Stage::OuterRoad | Stage::CrownDeep => Waters::OPEN,
            Stage::GreywaterSound => Waters::SOUND,
            Stage::PilotsCut => Waters::CUT,
        };
        Self {
            stage,
            waters,
            weather: Weather {
                sea,
                wind: wind.to_radians(),
                overcast,
            },
            waypoints: waypoints.map(Vec2::from),
            bearing: degrees.to_radians(),
            spacing: spacing * 7.,
            raid_interval: raid_interval * 1.35,
        }
    }

    /// These waters as charted for one run. The same run seed always gives the same chart,
    /// and each leg of a run gets its own. Every candidate is surveyed before it is used,
    /// and if none passes the stage's hand-set chart stands in.
    pub fn chart(&self, run_seed: u64) -> Chart {
        let name = self.stage.mission().name;
        let seed = leg_seed(run_seed, self.stage as u64);
        match Chart::generate(seed, &self.waters) {
            Some((chart, survey)) => {
                info!(
                    "{name}: run seed {run_seed}, chart {seed:#018x}, survey score {:.2}",
                    survey.score
                );
                chart
            }
            None => {
                warn!("{name}: no chart from run seed {run_seed} passed its survey");
                self.classic()
            }
        }
    }

    /// The hand-set chart of this stage.
    pub fn classic(&self) -> Chart {
        let course = Course::chart(self.waypoints, self.bearing, self.spacing);
        Chart::classic(course.gates().map(|gate| gate.center))
    }

    /// The buoy circuit of a chart. The ship arrives heading for its first gate.
    pub fn course(chart: &Chart) -> Course {
        Course::through(chart.gates)
    }

    /// The ports, islands and patrols of a chart.
    pub fn battle(&self, chart: &Chart) -> Battle {
        match self.stage {
            Stage::CrownDeep => Battle::regent_encounter(self.raid_interval, chart),
            Stage::IronNarrows => Battle::for_chart(self.raid_interval, chart),
            _ => Battle::for_chart(self.raid_interval, chart).with_flagship(),
        }
    }
}
