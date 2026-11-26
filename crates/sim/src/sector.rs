//! Where each stage is sailed: what its waters are like, its weather and how often raiders
//! put to sea. The campaign crate decides progression, and navigation and encounter take
//! plain geometry and timing, so this is the one place that ties a stage to both. The
//! game and the simulator both sail these sectors.
use glam::{Mat2, Vec2};
use shipvivor_campaign::{MAX_STARS, PATROLS, Stage, YARD_POINTS};
use shipvivor_encounter::{Battle, Chart, EnemyClass, Waters, leg_seed};

/// The weather of a stage as plain numbers. The game turns them into its seascape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weather {
    /// Sea state, from flat calm at 0 to heavy weather at 1.
    pub sea: f32,
    /// Heading the waves run towards, in radians.
    pub wind: f32,
    /// Cloud cover from 0 to 1.
    pub overcast: f32,
}

pub struct Sector {
    stage: Stage,
    /// What the waters are like. Each run charts them afresh from its seed.
    waters: Waters,
    /// The fairway used to lay out the stage's fallback chart.
    waypoints: [Vec2; 5],
    bearing: f32,
    spacing: f32,
    raid_interval: f32,
    /// Seconds along their escalation the raids begin, deeper into a run.
    head_start: f32,
    pub weather: Weather,
}

/// A fallback chart's fairway, its bearing in degrees and its scale.
type Lay = ([(f32, f32); 5], f32, f32);
const NARROWS_LAY: Lay = (
    [
        (0., -38.),
        (42., -106.),
        (124., -75.),
        (104., 19.),
        (18., 41.),
    ],
    0.,
    1.,
);
const ROAD_LAY: Lay = (
    [
        (0., -44.),
        (-62., -110.),
        (-145., -80.),
        (-136., 14.),
        (-50., 58.),
    ],
    -30.,
    1.25,
);
const SOUND_LAY: Lay = (
    [
        (0., -42.),
        (57., -85.),
        (16., -148.),
        (-47., -112.),
        (-61., -36.),
    ],
    38.,
    1.,
);
const CUT_LAY: Lay = (
    [
        (0., -38.),
        (-45., -94.),
        (10., -134.),
        (53., -85.),
        (29., -34.),
    ],
    -58.,
    0.82,
);

/// How far along their escalation each round's raids begin, a round deeper than the last.
/// The first round's start from patrol boats, as they always have.
pub const HEAD_START_PER_ROUND: f32 = 30.;
/// The furthest along any waters start, which the last patrol and Crown Deep share.
const MOST_HEAD_START: f32 = 180.;
/// Seconds further along their escalation a hard passage's raids begin than the open
/// water and the sound of the same round, so its heavier ships and its boss come sooner.
pub const HARD_PASSAGE_HEAD_START: f32 = 45.;

/// The most barrels a shore gun mounts in a stage's waters: one in the first round, two
/// in the second, three in the third and the full four from the fourth on.
pub fn most_barrels(stage: Stage) -> u8 {
    stage.round().clamp(1, 4) as u8
}

impl Sector {
    pub fn of(stage: Stage) -> Self {
        // Each lane past the first two rounds lays its fallback like the seas it follows.
        let (waypoints, degrees, spacing) = match stage {
            Stage::IronNarrows => NARROWS_LAY,
            Stage::OuterRoad | Stage::CrownDeep | Stage::BlackcapReach => ROAD_LAY,
            Stage::GreywaterSound | Stage::LanternBay => SOUND_LAY,
            Stage::PilotsCut => CUT_LAY,
            other => [ROAD_LAY, SOUND_LAY, CUT_LAY][other.lane()],
        };
        // Sea state, wave heading, cloud cover and time between raider launches.
        let (sea, wind, overcast, raid_interval): (f32, f32, f32, f32) = match stage {
            Stage::LanternBay => (0.14, 85., 0.12, 9.5),
            Stage::IronNarrows => (0.34, 125., 0., 7.),
            Stage::BlackcapReach => (0.82, 42., 0.72, 5.),
            // Open water, with a long swell running in under a clearing sky.
            Stage::CrownDeep => (0.48, 76., 0.24, 5.5),
            Stage::OuterRoad => (0.62, 100., 0.3, 5.5),
            // Sheltered and flat under low cloud.
            Stage::GreywaterSound => (0.18, 160., 0.85, 6.),
            Stage::PilotsCut => (1., 140., 1., 4.5),
            Stage::SealersRoad => (0.55, 110., 0.35, 5.2),
            Stage::KittiwakeSound => (0.2, 150., 0.6, 5.6),
            Stage::RimeGate => (0.7, 60., 0.5, 4.4),
            Stage::WhalebackReach => (0.66, 95., 0.4, 5.),
            // Still water lost in a fog bank.
            Stage::FogboundBay => (0.16, 175., 0.95, 5.4),
            Stage::TheGullet => (0.78, 130., 0.65, 4.2),
            Stage::LongnightRoad => (0.6, 20., 0.8, 4.8),
            Stage::HollowSound => (0.24, 200., 0.45, 5.2),
            Stage::SplinterCut => (0.85, 145., 0.75, 4.),
            Stage::FrostlineReach => (0.7, 70., 0.2, 4.6),
            // A clear, calm haven, and the yards there work fast.
            Stage::TernHaven => (0.12, 120., 0.1, 5.),
            Stage::DevilsThroat => (0.9, 30., 0.9, 3.8),
            Stage::BorealRoad => (0.75, 85., 0.55, 4.4),
            Stage::CalvingSound => (0.3, 165., 0.7, 4.8),
            // The last way through to the Regent lies through the worst of the weather.
            Stage::RegentsGate => (0.95, 135., 1., 3.6),
        };
        // The bases come in stages up each channel. A stage of two leaves the order to
        // the captain, and the next stage stirs once both are down. Each patrol is short:
        // three bases in the first two rounds, four in the next two and five after, so a
        // sweep stays near four minutes as she grows.
        let waters = match stage {
            // A; B; C.
            Stage::LanternBay => Waters::SOUND.staged(&[&[1], &[1], &[3]]),
            Stage::IronNarrows => Waters::NARROWS.staged(&[&[1], &[2], &[3]]),
            // A and B together from the start; C.
            Stage::BlackcapReach => Waters::OPEN.staged(&[&[1, 1], &[3]]),
            Stage::OuterRoad => Waters::OPEN.staged(&[&[1], &[2], &[3]]),
            Stage::GreywaterSound => Waters::SOUND.staged(&[&[1, 1], &[3]]),
            Stage::PilotsCut => Waters::CUT.staged(&[&[1], &[2], &[3]]),
            // A; B and C together; D.
            Stage::SealersRoad => Waters::OPEN.staged(&[&[1], &[1, 2], &[3]]),
            Stage::KittiwakeSound => Waters::SOUND,
            // A; B; C and D together.
            Stage::RimeGate => Waters::NARROWS,
            Stage::WhalebackReach => Waters::OPEN.staged(&[&[1], &[2], &[2, 3]]),
            Stage::FogboundBay => Waters::SOUND.staged(&[&[1, 1], &[2], &[3]]),
            Stage::TheGullet => Waters::CUT.staged(&[&[1], &[1, 2], &[3]]),
            // Five bases, with a pair to choose between on the way up.
            Stage::LongnightRoad => Waters::OPEN.staged(&[&[1], &[1, 2], &[2, 3]]),
            Stage::HollowSound => Waters::SOUND.staged(&[&[1], &[2], &[1, 2], &[3]]),
            Stage::SplinterCut => Waters::CUT.staged(&[&[1], &[1, 2], &[2], &[3]]),
            Stage::FrostlineReach => Waters::OPEN,
            Stage::TernHaven => Waters::SOUND.staged(&[&[1, 1], &[2], &[2, 3]]),
            Stage::DevilsThroat => Waters::NARROWS.staged(&[&[1], &[1, 2], &[2], &[3]]),
            Stage::BorealRoad => Waters::OPEN.staged(&[&[1], &[2], &[1, 2], &[3]]),
            Stage::CalvingSound => Waters::SOUND.staged(&[&[1], &[1, 2], &[2], &[3]]),
            Stage::RegentsGate => Waters::CUT.staged(&[&[1], &[2], &[2], &[2, 3]]),
            // A broad deep for the Regent: A; B and C together.
            Stage::CrownDeep => Waters {
                breadth: (1560., 1800.),
                narrows: (800., 900.),
                ..Waters::OPEN.staged(&[&[1], &[2, 3]])
            },
        };
        // The hard passage of a round is the one under air cover.
        let hard = stage.enters_air_cover();
        let head_start = (HEAD_START_PER_ROUND * (stage.round() - 1) as f32).min(MOST_HEAD_START)
            + if hard { HARD_PASSAGE_HEAD_START } else { 0. };
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
            head_start,
        }
    }

    /// Seconds along their escalation these waters' raids begin: heavier ships sooner,
    /// and a flagship out earlier, the deeper the round.
    pub fn head_start(&self) -> f32 {
        self.head_start
    }

    /// How many bases these waters hold.
    pub fn bases(&self) -> usize {
        self.waters.bases()
    }

    /// The boss these waters build up to: none in an anchorage, the Commodore's
    /// battleship in the second and third rounds, the Brass Parliament from the fourth,
    /// and the Iron Regent at Crown Deep.
    pub fn boss(&self) -> Option<EnemyClass> {
        match self.stage.round() {
            1 => None,
            2 | 3 => Some(EnemyClass::Commodore),
            round if round <= PATROLS => Some(EnemyClass::Parliament),
            _ => Some(EnemyClass::Regent),
        }
    }

    /// The most yard points a leg here can pay, short of cargo: every star at its
    /// lane's rate, a star's worth more for beating the clock, and every yard.
    pub fn most_points(&self) -> u16 {
        self.stage.star_points() * (u16::from(MAX_STARS) + 1)
            + YARD_POINTS * u16::try_from(self.bases()).unwrap_or(u16::MAX)
    }

    /// These waters as charted for one run. The same run seed always gives the same chart,
    /// and each leg of a run gets its own. Every candidate is surveyed before it is used,
    /// and if none passes the stage's hand-set chart stands in.
    pub fn chart(&self, run_seed: u64) -> Chart {
        self.charted(run_seed).chart
    }

    /// As [`Sector::chart`], with what the survey made of it for the log.
    pub fn charted(&self, run_seed: u64) -> Charted {
        let seed = leg_seed(run_seed, self.stage as u64);
        match Chart::generate(seed, &self.waters) {
            Some((chart, survey)) => Charted {
                chart,
                seed,
                score: Some(survey.score),
            },
            None => Charted {
                chart: self.classic(),
                seed,
                score: None,
            },
        }
    }

    /// The hand-set chart of this stage.
    pub fn classic(&self) -> Chart {
        let rotation = Mat2::from_angle(self.bearing);
        Chart::classic(self.waypoints.map(|p| rotation * p * self.spacing))
    }

    /// The ports, islands and patrols of a chart.
    pub fn battle(&self, chart: &Chart) -> Battle {
        let patrol =
            || Battle::for_chart(self.raid_interval, chart).with_squadrons(self.squadron_tier());
        match (self.stage, self.boss()) {
            (Stage::CrownDeep, _) => Battle::regent_encounter(self.raid_interval, chart),
            (_, Some(boss)) => patrol().with_boss(boss),
            (_, None) => patrol(),
        }
        .escalated(self.head_start)
        .turrets_capped(most_barrels(self.stage))
        .ships_of_round(self.stage.round())
    }

    /// How heavy the squadrons in a patrol's harbours are: the first round's light, then
    /// a step every two rounds. Crown Deep's bases keep none: the fight there is with
    /// the Regent and her screen.
    pub fn squadron_tier(&self) -> usize {
        self.stage.round().min(PATROLS) / 2
    }
}

/// A leg's chart and how it was drawn.
pub struct Charted {
    pub chart: Chart,
    /// The leg's own seed, drawn from the run seed and the stage.
    pub seed: u64,
    /// The survey's score, or `None` when no candidate passed and the hand-set chart
    /// stands in.
    pub score: Option<f32>,
}
