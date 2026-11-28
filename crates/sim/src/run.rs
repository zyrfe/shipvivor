//! Whole runs, sailed by the bot from her anchorage to Crown Deep as a player sails them:
//! she takes every card she is dealt, spends her points at every yard, picks her passage
//! on the chart and carries her ship, her level and her points from leg to leg. Many
//! seeds sailed by captains of many skills show how hard a run is, and for whom.
use crate::bot::{Bot, Skill, Temperament};
use crate::leg::{self, Outcome, Report, Setup};
use shipvivor_campaign::{
    CALIBRES, Card, DockJob, DockModule, Exit, Grade, Phase, Plan, Run, anchorages,
};

/// Which passages she takes from the chart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lanes {
    /// A tough passage whenever her stars open one, for its points and its free card.
    Tough,
    /// Always the moderate passage, for its repairs and its easier seas.
    Moderate,
}
impl Lanes {
    pub fn name(self) -> &'static str {
        match self {
            Self::Tough => "tough",
            Self::Moderate => "moderate",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RunSetup {
    pub seed: u64,
    pub temperament: Temperament,
    pub skill: Skill,
    pub lanes: Lanes,
    /// How hard the enemy's blows land, as a share of their weight (see
    /// `Battle::with_pressure`), over and above the game's own `sector::PRESSURE`. 1 is
    /// the game as it stands.
    pub pressure: f32,
    /// A further share for each round, from the first, to try a new `sector::PRESSURE`
    /// table without building the game.
    pub shape: [f32; shipvivor_campaign::ROUNDS],
}

/// How a run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ending {
    /// The Regent sunk at Crown Deep.
    Won,
    /// She sank in this round.
    Lost(usize),
    /// She was still afloat in this round with the leg unwon long past its clock: a
    /// captain who could neither win nor lose it.
    Stuck(usize),
}
impl Ending {
    /// The round the run ended in, or one past the last for a win.
    pub fn round(self) -> usize {
        match self {
            Self::Won => shipvivor_campaign::ROUNDS + 1,
            Self::Lost(round) | Self::Stuck(round) => round,
        }
    }
}

/// What she bought at one yard.
#[derive(Clone, Debug, Default)]
pub struct Refit {
    pub jobs: Vec<DockJob>,
    pub spent: u16,
    pub left: u16,
}

#[derive(Clone, Debug)]
pub struct RunReport {
    pub setup: RunSetup,
    pub ending: Ending,
    /// Every leg sailed, in order, with the round and grade it was sailed at.
    pub legs: Vec<(usize, Grade, Report)>,
    /// What she bought at each yard.
    pub refits: Vec<Refit>,
    /// Her length, her main guns as (bore, barrels) and her level at the end.
    pub length: f32,
    pub guns: Vec<(u16, u8)>,
    pub level: u32,
    pub wall: f32,
}

/// Seconds a leg may run past its clock before the simulation gives it up as stuck. The
/// clock ends no leg, so a player who can't clear one sails on; this is long enough that
/// a captain who could would have.
const PAST_CLOCK: f32 = 240.;

/// Sail a whole run.
pub fn sail_run(setup: RunSetup) -> RunReport {
    sail(setup, None).0
}

/// A run brought to the start of round `round`, as a captain of `skill` sails it there:
/// every leg before it sailed for real, every yard spent and every passage taken, so the
/// ship, her level and her points are what such a captain would bring. For trying the
/// later rounds by hand without sailing the early ones. A captain lost on the way is
/// sailed again from the next seed, up to `tries` times. Returns the run and the seed
/// it was sailed from.
pub fn veteran(seed: u64, round: usize, skill: Skill, tries: u64) -> Option<(Run, u64)> {
    (seed..seed.saturating_add(tries)).find_map(|seed| {
        let setup = RunSetup {
            seed,
            temperament: Temperament::Steady,
            skill,
            lanes: Lanes::Tough,
            pressure: 1.,
            shape: [1.; shipvivor_campaign::ROUNDS],
        };
        let (report, run) = sail(setup, Some(round));
        (report.ending == Ending::Stuck(round)
            && run.stage.round() == round
            && run.phase() != Phase::Lost)
            .then_some((run, seed))
    })
}

/// Sail a run until it ends, or until it stands at the start of round `stop`, and
/// return it with the report of what was sailed.
fn sail(setup: RunSetup, stop: Option<usize>) -> (RunReport, Run) {
    let started = std::time::Instant::now();
    let mut run = Run::default();
    run.seed = setup.seed;
    // She picks her anchorage as the seed falls.
    let first = anchorages(setup.seed)[(setup.seed % 3) as usize];
    run.stage = first;
    run.origin = first;
    let mut legs = Vec::new();
    let mut refits = Vec::new();
    let ending = loop {
        let round = run.stage.round();
        if stop == Some(round) {
            break Ending::Stuck(round);
        }
        let leg = Setup {
            stage: run.stage,
            grade: run.grade,
            seed: setup.seed,
            temperament: setup.temperament,
        };
        let limit = f32::from(run.stage.mission().minutes) * 60. + PAST_CLOCK;
        let mut bot = Bot::skilled(
            setup.temperament,
            setup.skill,
            setup.seed.wrapping_mul(31).wrapping_add(round as u64),
        );
        let pressure = setup.pressure * setup.shape[round.clamp(1, setup.shape.len()) - 1];
        let report = leg::sail_leg(&mut run, leg, &mut bot, limit, pressure);
        let outcome = report.outcome;
        legs.push((round, run.grade, report));
        match run.phase() {
            Phase::Won(_) => break Ending::Won,
            Phase::Lost => break Ending::Lost(round),
            Phase::Drydock(_) => {}
            _ if outcome == Outcome::Sunk => break Ending::Lost(round),
            _ => break Ending::Stuck(round),
        }
        // Level-ups still owed are free hands in the yard.
        while run.fittings_remaining() > 0 {
            let Some(card) = Bot::choose(&run.offers()) else {
                break;
            };
            run.install(card);
        }
        refits.push(refit(&mut run));
        if !run.leave_dock() {
            break Ending::Stuck(round);
        }
        match run.phase() {
            Phase::Intercept(_) => {
                run.engage_regent();
            }
            Phase::Chart => {
                let Some(exit) = passage(&run, setup.lanes) else {
                    break Ending::Stuck(round);
                };
                run.depart(exit.stage);
            }
            _ => break Ending::Stuck(round),
        }
    };
    let design = run.ship.design();
    let report = RunReport {
        setup,
        ending,
        legs,
        refits,
        length: design.length(),
        guns: design
            .modules()
            .iter()
            .filter(|m| m.fit.contains(&shipvivor_shipyard::Component::MainGun))
            .map(|m| (m.gunhouse.bore, m.gunhouse.barrels.count() as u8))
            .collect(),
        level: run.level,
        wall: started.elapsed().as_secs_f32(),
    };
    (report, run)
}

/// The passage she takes from the chart: a tough one when she wants it and her stars
/// open it, the moderate one otherwise. Of two tough passages the seed picks.
fn passage(run: &Run, lanes: Lanes) -> Option<Exit> {
    let exits = run.exits()?;
    let open = |exit: &&Exit| run.stars() >= exit.stars;
    let tough: Vec<&Exit> = (exits.iter())
        .filter(|e| e.grade == Grade::Tough)
        .filter(open)
        .collect();
    match lanes {
        Lanes::Tough if !tough.is_empty() => {
            Some(*tough[(run.seed as usize + run.course.len()) % tough.len()])
        }
        _ => exits.iter().find(|e| e.grade == Grade::Moderate).copied(),
    }
}

/// Spend her points at the yard as a fair player does: the next step of bore, another
/// turret, a heavier class of hull to carry more, more barrels, then plate and the
/// gunnery cards, and the strike craft once the guns are seen to. She buys the most
/// wanted job she can pay for, again and again, until nothing she wants is left in her
/// purse. A new hull costs more than most legs pay, so once she is long enough for one
/// she banks what does not go on guns towards it, rather than spend it on lesser work.
pub fn refit(run: &mut Run) -> Refit {
    let before = run.points();
    let mut plan = Plan::draw(run);
    let mut jobs = Vec::new();
    let hull = DockJob::WidenBeam;
    // Every purchase costs a point or more, so this bound is never the one that stops her.
    for _ in 0..64 {
        let saving = hull.fits(run) && run.points() < hull.cost();
        let least = want(run, hull).filter(|_| saving).unwrap_or(0);
        let mut wants: Vec<(u32, DockJob)> = offered(run)
            .into_iter()
            .filter_map(|job| want(run, job).map(|w| (w, job)))
            .filter(|&(w, _)| w >= least)
            .collect();
        wants.sort_by_key(|&(w, _)| std::cmp::Reverse(w));
        let Some(job) = wants
            .into_iter()
            .map(|(_, job)| job)
            .find(|&job| plan.order(run, job, None))
        else {
            break;
        };
        jobs.push(job);
    }
    Refit {
        jobs,
        spent: before.saturating_sub(run.points()),
        left: run.points(),
    }
}

/// Every job the yard offers her: on the hull as a whole, on each section, and a new
/// section of each kind spliced in where the yard puts them.
fn offered(run: &Run) -> Vec<DockJob> {
    let sections = run.ship.design().modules().len();
    let gap = run.default_gap();
    run.hull_jobs()
        .into_iter()
        .chain((0..sections).flat_map(|at| run.section_jobs(at)))
        .chain(DockModule::ALL.map(|module| DockJob::Insert { at: gap, module }))
        .collect()
}

/// How much she wants a job, or `None` for one she would never buy: a step down, a move,
/// a barbette or a bare bay.
fn want(run: &Run, job: DockJob) -> Option<u32> {
    use Card::*;
    Some(match job {
        DockJob::Calibre { from, to, .. } => {
            (CALIBRES.into_iter().find(|&c| c > from) == Some(to)).then_some(100)?
        }
        DockJob::Insert {
            module: DockModule::Turret,
            ..
        } => 95,
        DockJob::Replace { .. } if job.adds_turret(run) => 94,
        DockJob::WidenBeam => 90,
        DockJob::Barrels { from, to, .. } => (to > from).then_some(85)?,
        DockJob::Fitting(card) => match card {
            Armor => 75,
            Loader => 70,
            Hardening => 60,
            TrainingGear => 55,
            Velocity | Caps => 50,
            Turbines => 45,
            Pumps => 40,
            SecondaryTurrets => 35,
            MoreLaunchers | MoreMissiles | MorePlanes | MoreBombs => 30,
            MoreSecondaries | TwinSecondaries | HeavySecondaries | QuickSecondaries
            | SecondaryRange => 30,
            MoreAntiAir | HeavyAntiAir | QuickAntiAir => 25,
            // Her tubes are a destroyer's weapon, and guidance makes them hers.
            GuidedTorpedoes => 50,
            QuickTubes | LongRunningTorpedoes => 30,
            // The cruiser's long-range gunnery, for a captain who keeps her guns' reach.
            SuppressingFire | PlungingFire => 45,
            LongBaseRangefinders => 40,
            _ => return None,
        },
        DockJob::Insert {
            module: DockModule::Missiles,
            ..
        } => 50,
        DockJob::Insert {
            module: DockModule::Floatplane,
            ..
        } => 45,
        DockJob::Insert {
            module: DockModule::EngineRoom,
            ..
        } => 35,
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
