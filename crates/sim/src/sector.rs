//! Where each stage is sailed: what its waters are like, its weather and how often raiders
//! put to sea. The campaign crate decides progression, and navigation and encounter take
//! plain geometry and timing, so this is the one place that ties a stage to both. The
//! game and the simulator both sail these sectors.
use glam::{Mat2, Vec2};
use shipvivor_campaign::{
    Exit, Fleet, Grade, MAX_STARS, PATROLS, POOLS, ROUNDS, SURVIVAL_POINTS, Stage, YARD_POINTS,
};
use shipvivor_encounter::{
    Battle, Chart, Doctrine, EnemyClass, IndustrialYard, Surprise, Waters, leg_seed,
};

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
    /// How hard the passage into these waters runs.
    grade: Grade,
    /// How many times over its bases moor and launch their ships: 2 on a punishing
    /// passage (see [`Sector::with_strength`]).
    strength: usize,
    /// The channel the waters are charted as.
    kind: Kind,
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

/// The kind of channel a sea is charted as, from the broadest to the tightest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Broad reaches with few islands.
    Open,
    /// A sound crowded with islands.
    Sound,
    /// Island narrows between broad reaches.
    Narrows,
    /// A tight cut, winding hard between crags.
    Cut,
}
impl Kind {
    /// What the waters of this kind are like, before their bases are staged.
    pub fn waters(self) -> Waters {
        match self {
            Self::Open => Waters::OPEN,
            Self::Sound => Waters::SOUND,
            Self::Narrows => Waters::NARROWS,
            Self::Cut => Waters::CUT,
        }
    }
    fn lay(self) -> Lay {
        match self {
            Self::Open => ROAD_LAY,
            Self::Sound => SOUND_LAY,
            Self::Narrows => NARROWS_LAY,
            Self::Cut => CUT_LAY,
        }
    }
    /// The channel as a pilot would note it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Open => "Open reaches",
            Self::Sound => "Island sound",
            Self::Narrows => "Narrows",
            Self::Cut => "Winding cut",
        }
    }
}

/// How far along their escalation each round's raids begin, a round deeper than the last,
/// so the last patrol starts as far along as any. The first round's start from patrol
/// boats, as they always have.
pub const HEAD_START_PER_ROUND: f32 = MOST_HEAD_START / (PATROLS - 1) as f32;
/// The furthest along any waters start, which the last patrol and Crown Deep share.
const MOST_HEAD_START: f32 = 180.;
/// Seconds further along their escalation a tough passage's raids begin than a moderate
/// one of the same round, so its heavier ships and its boss come sooner.
pub const TOUGH_HEAD_START: f32 = 45.;
/// The share of a sea's time between raider launches left on a tough passage, so its
/// raiders come out quicker than they do when the same waters are charted moderate.
pub const TOUGH_RAID_PACE: f32 = 0.85;
/// How hard the enemy's blows land in each round, as a share of their weight: the dial
/// that turns a round harder or easier without changing what she meets in it (see
/// `Battle::with_pressure`). `cargo run --release -p shipvivor-sim --example runs`
/// shows what a setting does to the chance of a field of players getting through.
pub const PRESSURE: [f32; ROUNDS] = [1.; ROUNDS];
const _: () = assert!(shipvivor_encounter::RUN_ROUNDS == ROUNDS);

/// One passage in this many, past the anchorages, hides a fleet the scouts missed.
pub const SURPRISE_ODDS: u64 = 3;
/// Keeps the surprises' draw apart from anything else drawn from a leg's seed.
const SURPRISE_STREAM: u64 = 0x7375_7270_7269_7365;

/// The encounter's doctrine for what holds a sea.
pub fn doctrine(fleet: Fleet) -> Doctrine {
    match fleet {
        Fleet::Flotillas => Doctrine::Flotillas,
        Fleet::Mixed => Doctrine::Mixed,
        Fleet::BattleLine => Doctrine::BattleLine,
    }
}

/// The most barrels a shore gun mounts in a stage's waters: one in the first round, two
/// in the second, three in the third and the full four from the fourth on.
pub fn most_barrels(stage: Stage) -> u8 {
    stage.round().clamp(1, 4) as u8
}

/// Bases by stage up the channel, as factory counts. Every sea of a round holds as many
/// bases: three in the first two rounds, four in the next two and five in the last two,
/// so a sweep stays near four minutes as she grows, and the passages of a round pay alike.
type Plan = &'static [&'static [u8]];
/// Sea state, wave heading in degrees, cloud cover and seconds between raider launches.
type Climate = (f32, f32, f32, f32);

/// Each sea's channel, the bases staged up it, and its weather. A `4` is an airfield
/// base: two or three seas of each pool from the third round on keep one up the channel,
/// short of the heavy yard. A stage of two leaves
/// the order to the captain, and the next stage stirs once both are down.
fn sea(stage: Stage) -> (Kind, Plan, Climate) {
    use Kind::*;
    use Stage::*;
    const MILL: u8 = IndustrialYard::RedShift.works();
    const FORGE: u8 = IndustrialYard::BellWorks.works();
    match stage {
        // The anchorages. A; B; C, or A and B together; C.
        LanternBay => (Sound, &[&[1], &[1], &[3]], (0.14, 85., 0.12, 9.5)),
        IronNarrows => (Narrows, &[&[1], &[2], &[3]], (0.34, 125., 0., 7.)),
        BlackcapReach => (Open, &[&[1, 1], &[3]], (0.82, 42., 0.72, 5.)),
        EiderSound => (Sound, &[&[1, 1], &[3]], (0.2, 140., 0.35, 8.5)),
        SkerryHaven => (Sound, &[&[1], &[2], &[3]], (0.1, 60., 0.6, 9.)),
        DriftwoodBay => (Open, &[&[1], &[1], &[3]], (0.45, 100., 0.25, 7.5)),
        PtarmiganInlet => (Cut, &[&[1], &[2], &[3]], (0.25, 170., 0.5, 8.)),
        CairnRoad => (Open, &[&[1], &[2], &[3]], (0.6, 20., 0.8, 6.)),
        SnowmeltNarrows => (Narrows, &[&[1, 1], &[3]], (0.3, 75., 0.9, 7.)),
        // The second round: three bases.
        OuterRoad => (Open, &[&[1], &[2], &[3]], (0.62, 100., 0.3, 5.5)),
        // Sheltered and flat under low cloud.
        GreywaterSound => (Sound, &[&[1, 1], &[3]], (0.18, 160., 0.85, 6.)),
        PilotsCut => (Cut, &[&[1], &[2], &[3]], (1., 140., 1., 5.3)),
        WalrusReach => (Open, &[&[1, 1], &[3]], (0.5, 55., 0.4, 5.6)),
        GannetSound => (Sound, &[&[1], &[2], &[3]], (0.3, 120., 0.2, 5.8)),
        ShingleRoad => (Open, &[&[1], &[1], &[3]], (0.7, 15., 0.65, 5.2)),
        NarwhalCut => (Cut, &[&[1], &[1], &[3]], (0.4, 190., 0.7, 5.5)),
        BrineNarrows => (Narrows, &[&[1], &[2], &[3]], (0.55, 130., 0.55, 5.4)),
        AukBay => (Sound, &[&[1], &[1], &[3]], (0.22, 80., 0.9, 6.)),
        // The third round: four bases. A; B and C together; D, and the like.
        SealersRoad => (Open, &[&[1], &[1, 2], &[3]], (0.55, 110., 0.35, 5.2)),
        KittiwakeSound => (Sound, &[&[1], &[1, 2], &[3]], (0.2, 150., 0.6, 5.6)),
        RimeGate => (Narrows, &[&[1], &[2], &[2, 3]], (0.7, 60., 0.5, 5.2)),
        SkuaNarrows => (Narrows, &[&[1], &[1, 2], &[3]], (0.45, 35., 0.75, 5.)),
        FloeRoad => (Open, &[&[1], &[4], &[2, 3]], (0.35, 95., 0.15, 5.4)),
        BearclawCut => (Cut, &[&[1], &[1, 2], &[3]], (0.65, 155., 0.8, 5.)),
        IvorySound => (Sound, &[&[1, 1], &[2], &[3]], (0.15, 70., 0.3, 5.6)),
        HoarfrostBay => (Sound, &[&[1], &[2], &[2, 3]], (0.28, 185., 0.95, 5.3)),
        LodestoneReach => (Open, &[&[1, 1], &[4], &[3]], (0.6, 125., 0.5, 5.1)),
        // The fourth round: four bases.
        WhalebackReach => (Open, &[&[1], &[2], &[2, 3]], (0.66, 95., 0.4, 5.)),
        // Still water lost in a fog bank.
        FogboundBay => (Sound, &[&[1, 1], &[2], &[3]], (0.16, 175., 0.95, 5.4)),
        TheGullet => (Cut, &[&[1], &[1, 2], &[3]], (0.78, 130., 0.65, 4.9)),
        PetrelSound => (Sound, &[&[1], &[1, 2], &[3]], (0.25, 105., 0.4, 5.2)),
        IcefallCut => (Cut, &[&[1], &[2], &[2, 3]], (0.5, 165., 0.6, 5.)),
        BeaconRoad => (Open, &[&[1], &[1, 4], &[3]], (0.72, 45., 0.85, 4.9)),
        CormorantReach => (Open, &[&[1, 1], &[2], &[3]], (0.58, 80., 0.3, 5.1)),
        ShiverNarrows => (Narrows, &[&[1], &[2], &[2, 3]], (0.85, 25., 0.9, 4.8)),
        WreckersBay => (Sound, &[&[1], &[4], &[2, 3]], (0.4, 200., 0.7, 5.2)),
        FrostlineReach => (Open, &[&[1], &[1, 2], &[3]], (0.7, 70., 0.2, 5.)),
        // A clear, calm haven, and the yards there work fast.
        TernHaven => (Sound, &[&[1, 1], &[2], &[3]], (0.12, 120., 0.1, 4.8)),
        NorthlightReach => (Open, &[&[1], &[2], &[2, 3]], (0.3, 10., 0.05, 5.1)),
        // The fifth patrol ends at the Red Shift rolling mill.
        LongnightRoad => (Open, &[&[1], &[1, 2], &[2, MILL]], (0.6, 20., 0.8, 4.8)),
        HollowSound => (
            Sound,
            &[&[1], &[2], &[1, 2], &[MILL]],
            (0.24, 200., 0.45, 5.2),
        ),
        SplinterCut => (
            Cut,
            &[&[1], &[1, 2], &[2], &[MILL]],
            (0.85, 145., 0.75, 4.7),
        ),
        PolarRoad => (Open, &[&[1], &[2], &[1, 4], &[MILL]], (0.5, 65., 0.1, 5.)),
        SleetSound => (Sound, &[&[1, 1], &[2], &[2, MILL]], (0.35, 135., 0.9, 5.)),
        AnvilCut => (Cut, &[&[1], &[2], &[2], &[2, MILL]], (0.6, 175., 0.55, 4.8)),
        BlackwaterNarrows => (
            Narrows,
            &[&[1], &[1, 2], &[2], &[MILL]],
            (0.7, 110., 1., 4.7),
        ),
        RavensReach => (Open, &[&[1], &[1, 2], &[2], &[MILL]], (0.8, 40., 0.6, 4.7)),
        HarrowBay => (
            Sound,
            &[&[1], &[1, 2], &[4], &[MILL]],
            (0.2, 90., 0.35, 5.2),
        ),
        CoffinCut => (
            Cut,
            &[&[1], &[1, 2], &[2], &[MILL]],
            (0.75, 185., 0.85, 4.8),
        ),
        HailstoneSound => (
            Sound,
            &[&[1], &[2], &[1, 2], &[MILL]],
            (0.45, 160., 0.8, 5.),
        ),
        ShroudBay => (
            Sound,
            &[&[1], &[1, 2], &[4], &[MILL]],
            (0.18, 140., 1., 4.9),
        ),
        // The last patrol ends at the Bell Works steam forge.
        BorealRoad => (
            Open,
            &[&[1], &[2], &[1, 2], &[FORGE]],
            (0.75, 85., 0.55, 4.4),
        ),
        CalvingSound => (
            Sound,
            &[&[1], &[1, 2], &[2], &[FORGE]],
            (0.3, 165., 0.7, 4.8),
        ),
        // The last way through to the Regent lies through the worst of the weather.
        RegentsGate => (Cut, &[&[1], &[2], &[2], &[2, FORGE]], (0.95, 135., 1., 4.2)),
        MidwinterRoad => (Open, &[&[1], &[1, 2], &[2, FORGE]], (0.65, 30., 0.9, 4.4)),
        AdmiraltySound => (Sound, &[&[1, 1], &[4], &[2, FORGE]], (0.4, 95., 0.5, 4.6)),
        BastionCut => (Cut, &[&[1], &[1, 2], &[2], &[FORGE]], (0.7, 150., 0.7, 4.3)),
        GauntletNarrows => (
            Narrows,
            &[&[1], &[1, 2], &[2], &[FORGE]],
            (0.85, 60., 0.8, 4.2),
        ),
        CitadelBay => (
            Sound,
            &[&[1], &[2], &[1, 4], &[FORGE]],
            (0.22, 190., 0.4, 4.7),
        ),
        WhiteoutReach => (Open, &[&[1], &[1, 2], &[2], &[FORGE]], (0.9, 120., 1., 4.3)),
        DevilsThroat => (
            Narrows,
            &[&[1], &[1, 2], &[2], &[FORGE]],
            (0.9, 30., 0.9, 4.3),
        ),
        WolfpackNarrows => (
            Narrows,
            &[&[1], &[2], &[1, 2], &[FORGE]],
            (0.6, 50., 0.65, 4.3),
        ),
        TempestRoad => (Open, &[&[1], &[1, 4], &[2, FORGE]], (0.95, 115., 0.95, 4.4)),
        // A broad deep for the Regent, under a long swell and a clearing sky: A; B and
        // C together. Its breadth is set apart in `Sector::of`.
        CrownDeep => (Open, &[&[1], &[2, 3]], (0.48, 76., 0.24, 5.5)),
    }
}

impl Sector {
    /// The sea `stage` as sailed at `grade`. An anchorage and Crown Deep are sailed the
    /// same whatever the grade.
    pub fn of(stage: Stage, grade: Grade) -> Self {
        let (kind, plan, (sea, wind, overcast, raid_interval)) = sea(stage);
        let (waypoints, degrees, spacing) = kind.lay();
        let waters = match stage {
            Stage::CrownDeep => Waters {
                breadth: (1560., 1800.),
                narrows: (800., 900.),
                ..Waters::OPEN.staged(plan)
            },
            _ => kind.waters().staged(plan),
        };
        // A tough passage's raids begin further along and come out quicker.
        let tough = stage.enters_air_cover(grade);
        let head_start = (HEAD_START_PER_ROUND * (stage.round() - 1) as f32).min(MOST_HEAD_START)
            + if tough { TOUGH_HEAD_START } else { 0. };
        let pace = if tough { TOUGH_RAID_PACE } else { 1. } * doctrine(stage.fleet()).raid_pace();
        Self {
            stage,
            grade,
            strength: 1,
            kind,
            waters,
            weather: Weather {
                sea,
                wind: wind.to_radians(),
                overcast,
            },
            waypoints: waypoints.map(Vec2::from),
            bearing: degrees.to_radians(),
            spacing: spacing * 7.,
            raid_interval: raid_interval * 1.35 * pace,
            head_start,
        }
    }

    /// These waters as a passage `exit` sails them: at its grade, and punishing if the
    /// captain set it so.
    pub fn sailed(exit: &Exit) -> Self {
        Self::of(exit.stage, exit.grade).with_strength(exit.strength())
    }
    /// Bases that moor and launch `times` the ships: see [`Battle::with_strength`]. An
    /// anchorage and Crown Deep are never punishing.
    pub fn with_strength(mut self, times: usize) -> Self {
        if (2..=PATROLS).contains(&self.stage.round()) {
            self.strength = times.max(1);
        }
        self
    }
    /// How many times over its bases moor and launch their ships.
    pub fn strength(&self) -> usize {
        self.strength
    }
    /// The bases moor and launch twice the ships, and coming through pays a bonus.
    pub fn punishing(&self) -> bool {
        self.strength > 1
    }

    /// The channel these waters are charted as.
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// The waters as a pilot would note them, short enough for a narrow card: the
    /// channel and the sea running in it.
    pub fn note(&self) -> String {
        let sea = match self.weather.sea {
            s if s < 0.25 => "calm",
            s if s < 0.5 => "light swell",
            s if s < 0.8 => "heavy swell",
            _ => "rough seas",
        };
        // An airfield up the channel is the first thing a captain wants to know.
        let channel = if self.airfield() {
            "Airfield"
        } else {
            self.kind.name()
        };
        format!("{channel}, {sea}")
    }

    /// An airfield base stands somewhere up the channel.
    pub fn airfield(&self) -> bool {
        (self.waters.stages.iter())
            .any(|stage| stage.contains(&shipvivor_encounter::AIRFIELD_WORKS))
    }

    /// How often raiders put out here beside the other seas of the round's pool, whatever
    /// the grade.
    pub fn patrols(&self) -> &'static str {
        let launches = |stage: Stage| sea(stage).2.3 * doctrine(stage.fleet()).raid_pace();
        let Some(pool) = POOLS.get(self.stage.round() - 1) else {
            return "The Regent's screen";
        };
        let mut pace: Vec<f32> = pool.iter().map(|&sea| launches(sea)).collect();
        pace.sort_by(f32::total_cmp);
        let ratio = launches(self.stage) / pace[pace.len() / 2];
        if ratio > 1.1 {
            "Patrols spaced apart"
        } else if ratio < 0.92 {
            "Quick reinforcements"
        } else {
            "Steady patrols"
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

    /// The bases staged up the channel, as factory counts by stage: see `sea`.
    pub fn plan(&self) -> &'static [&'static [u8]] {
        self.waters.stages
    }

    /// The named factory complex at the last base of this passage, if it has one.
    pub fn industrial_yard(&self) -> Option<IndustrialYard> {
        let &works = self.plan().last()?.last()?;
        IndustrialYard::from_works(works)
    }

    /// Seconds between the raiders' launches here: quicker as many times over as the
    /// bases' strength.
    pub fn raid_interval(&self) -> f32 {
        self.raid_interval / self.strength as f32
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

    /// What holds these waters, as the scouts report it.
    pub fn fleet(&self) -> Fleet {
        self.stage.fleet()
    }

    /// The ships that put to sea with the boss here, in the order they sail.
    pub fn escort(&self) -> &'static [EnemyClass] {
        match (self.stage, self.boss()) {
            (Stage::CrownDeep, _) | (_, None) => &[],
            _ => doctrine(self.fleet()).escort(self.stage.round()),
        }
    }

    /// The fleet the scouts missed in these waters as charted, if one lies here: one
    /// passage in [`SURPRISE_ODDS`] past the anchorages, drawn from the chart's own seed
    /// so a run always meets the same. Never in a hand-set chart or at Crown Deep.
    pub fn surprise(&self, chart: &Chart) -> Option<Surprise> {
        let seed = chart.seed?;
        if !(2..=PATROLS).contains(&self.stage.round()) {
            return None;
        }
        let mut z = (seed ^ SURPRISE_STREAM).wrapping_add(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        (z.is_multiple_of(SURPRISE_ODDS))
            .then(|| Surprise::against(doctrine(self.fleet()), z >> 32))
    }

    /// The most yard points a leg here can pay, short of cargo: every star at its
    /// grade's rate, a star's worth more for beating the clock, and every yard.
    pub fn most_points(&self) -> u16 {
        self.stage.star_points(self.grade) * (u16::from(MAX_STARS) + 1)
            + YARD_POINTS * u16::try_from(self.bases()).unwrap_or(u16::MAX)
            + if self.punishing() { SURVIVAL_POINTS } else { 0 }
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
        Chart::classic_with_yard(
            self.waypoints.map(|p| rotation * p * self.spacing),
            self.industrial_yard(),
        )
    }

    /// The ports, islands and patrols of a chart.
    pub fn battle(&self, chart: &Chart) -> Battle {
        let patrol = || {
            let held = (Battle::for_chart(self.raid_interval, chart))
                .with_doctrine(doctrine(self.fleet()));
            match self.surprise(chart) {
                Some(surprise) => held.with_surprise(surprise),
                None => held,
            }
            .with_squadrons(self.stage.round())
            .with_strength(self.strength)
        };
        match (self.stage, self.boss()) {
            (Stage::CrownDeep, _) => Battle::regent_encounter(self.raid_interval, chart),
            (_, Some(boss)) => patrol().with_boss(boss),
            (_, None) => patrol(),
        }
        .escalated(self.head_start)
        .turrets_capped(most_barrels(self.stage))
        .ships_of_round(self.stage.round())
        .with_pressure(PRESSURE[self.stage.round().clamp(1, ROUNDS) - 1])
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
