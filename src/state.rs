//! Game state shared by every module. `main` advances it and the views read it.
use crate::passage::Passage;
use bevy::prelude::*;
use shipvivor_campaign::{DockJob, Fittings, Grade, LegEnd, Phase, Plan, Run, Stage};
use shipvivor_encounter::{Battle, Control, Volley};
use shipvivor_navigation::{Course, Helm};
use shipvivor_shipyard::Design;
use std::borrow::Cow;

#[derive(Resource, Default, Deref, DerefMut)]
pub struct PlayerHelm(pub Helm);

impl PlayerHelm {
    pub fn cruising() -> Self {
        let mut helm = Helm::default();
        helm.heading = 282_f32.to_radians();
        helm.ring(3);
        helm.speed = helm.ordered_speed();
        Self(helm)
    }
}
#[derive(Resource, Default, Deref, DerefMut)]
pub struct TrialCourse(pub Course);
#[derive(Resource, Deref, DerefMut)]
pub struct Encounter(pub Battle);
impl Default for Encounter {
    fn default() -> Self {
        let passage = Passage::of(Stage::IronNarrows);
        Self(passage.battle(&passage.classic()))
    }
}
/// Where each new run's seed comes from: the clock, or `SHIPVIVOR_SEED` to sail a run's
/// waters again. The seed is logged when each leg is charted.
#[derive(Resource)]
pub struct ChartSeeds(u64);
impl Default for ChartSeeds {
    fn default() -> Self {
        let fixed = std::env::var("SHIPVIVOR_SEED")
            .ok()
            .and_then(|seed| seed.trim().parse().ok());
        Self(fixed.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |t| t.as_nanos() as u64)
        }))
    }
}
impl ChartSeeds {
    #[cfg(test)]
    pub fn fixed(seed: u64) -> Self {
        Self(seed)
    }
    /// The seed the next run will draw, without drawing it.
    pub fn peek(&self) -> u64 {
        self.0
    }
    /// The seed for the next run. The first is the one the source was given.
    pub fn draw(&mut self) -> u64 {
        let seed = self.0;
        self.0 = shipvivor_encounter::leg_seed(seed, u64::MAX);
        seed
    }
}
impl Encounter {
    pub fn fit_player(&mut self, run: &Campaign) {
        shipvivor_sim::fit_player(self, run);
    }
}
#[derive(Resource, Default)]
pub struct Guns {
    /// The left button is held on the sight.
    pub firing: bool,
    /// Space is held on the selected ship.
    pub nearest: bool,
    pub mouse_captured: bool,
    pub torpedo_mode: bool,
    pub launch_pending: bool,
    /// A press of either trigger, waiting for the next fixed step.
    pub volley: Option<Volley>,
    /// When a trigger was last pressed, to tell a double click.
    pub last_press: Option<f32>,
    /// Who lays each main turret, by its number from the bow less one. They stand from
    /// leg to leg, and a turret the ship does not have yet is manual.
    pub controls: [Control; TURRETS],
    /// The turret a digit key picked out, by its number less one, for Z to change. With
    /// none picked out, Z changes them all.
    pub selected: Option<usize>,
}
/// The most main turrets the keys can number, 1 to 9.
pub const TURRETS: usize = 9;

/// What the application is doing around the run. The campaign crate only knows the run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Title,
    Trial,
    Patrol,
    Paused,
    /// The battle's timeline is open, from the pause menu or the report of her loss, and
    /// the waters show the moment picked on it (see `rewind`).
    Rewind,
}

#[derive(Clone, Copy, Default)]
pub enum TitlePage {
    #[default]
    Menu,
    Customise,
    Options,
    Departure,
    Exercise,
}

/// What the player is looking at: the mode, refined by the run's phase while on patrol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Title,
    /// Opened from the title, over the same cruise: Breakwater's flag, for now.
    Customise,
    /// Opened from the title, over the same cruise: how the ship earns experience.
    Options,
    /// Choose the first patrol before the campaign clock starts.
    Departure,
    /// Test mode's list of seas, opened from the title or from its yard: choose a round,
    /// a grade and a sea to try a ship against.
    Exercise,
    Trial,
    Sailing,
    Secured,
    Refit,
    Drydock(LegEnd),
    Chart,
    /// The passage chart consulted from the bridge during a patrol.
    PassageMap,
    Intercept(LegEnd),
    Paused,
    /// Scrubbing back through the battle to sail on from an earlier moment.
    Rewind,
    Won(LegEnd),
    Lost,
}

#[derive(Resource, Deref, DerefMut)]
pub struct Campaign {
    pub chart_roll: Option<ChartRoll>,
    pub watch: crate::captain::Watch,
    pub clearance_left: f32,
    /// The refit on the selected section's list picked out with the arrow keys. Its
    /// figures are shown until it is ordered or another is picked. A click orders at once
    /// and picks nothing out.
    pub dock_option: Option<usize>,
    /// The last order made, kept to show what it did until another section is selected.
    pub dock_receipt: Option<Receipt>,
    /// The section of the ship selected in the drydock, counted from the bow.
    pub dock_module: usize,
    /// Orders made in the yard and not yet built. The ship as she stands carries them
    /// already, so every view of her is the planned ship, and each can be taken back
    /// until the refit is built.
    pub dock_plan: Option<Plan>,
    /// Counts every change to the plan, so views of the ship know to refit.
    pub dock_revision: u32,
    pub title_page: TitlePage,
    /// Test mode's list of seas: the round whose seas it shows, and the grade it sails
    /// them at.
    pub bench: (usize, Grade),
    /// Test mode: the run as she lay in the yard when she last sailed from it, which the
    /// sortie comes back to however it ends.
    berth: Option<Run>,
    /// Test mode: how the last sortie went, for the yard to show.
    pub debrief: Option<String>,
    pub hits: u32,
    pub sunk: u32,
    pub mode: Mode,
    /// The run itself. Its `seed` charts every leg and deals every level-up hand, and
    /// `ChartSeeds` draws a new one for each run.
    #[deref]
    run: Run,
}

/// An order just made in the yard, with the ship as she stood before it, so its figures
/// can be read as the change it made.
pub struct Receipt {
    /// The job as ordered, with its places on the ship as she stood before it.
    pub job: DockJob,
    /// Work on the hull as a whole, shown over the hull panel rather than under a section.
    pub on_hull: bool,
    /// Yard points the order took, less any it gave back by taking back an earlier one.
    pub spent: i32,
    before: Run,
}
impl Receipt {
    /// The yard as it stood before the order, to read the order's figures against.
    pub fn before(&self) -> Campaign {
        Campaign::with_run(Mode::Patrol, self.before.clone())
    }
}

#[derive(Clone, Copy, Default)]
pub struct ChartRoll {
    pub opening: bool,
    /// Fraction of the sheet unwound, retained when a tap reverses the roll.
    pub spread: f32,
}
impl Default for Campaign {
    fn default() -> Self {
        Self::new(Mode::Title)
    }
}
impl Campaign {
    /// A fresh run entered in the given mode.
    pub fn new(mode: Mode) -> Self {
        Self {
            mode,
            chart_roll: None,
            watch: Default::default(),
            clearance_left: 0.,
            dock_option: None,
            dock_receipt: None,
            dock_module: 1,
            dock_plan: None,
            dock_revision: 0,
            title_page: TitlePage::Menu,
            bench: (1, Grade::Tough),
            berth: None,
            debrief: None,
            hits: 0,
            sunk: 0,
            run: Run::default(),
        }
    }
    /// A run made elsewhere, as it stands, entered in the given mode.
    pub fn with_run(mode: Mode, run: Run) -> Self {
        Self {
            run,
            ..Self::new(mode)
        }
    }

    /// The refits offered by the selected section of the ship.
    pub fn dock_jobs(&self) -> Vec<DockJob> {
        self.section_jobs(self.dock_module)
    }
    /// Test mode's yard before `stage`, at the grade its list of seas is set to, with the
    /// ship she has built there already or, coming from the title, a stock one. She
    /// starts at the level a run has usually reached by that round.
    pub fn exercise(&self, stage: Stage, seed: u64) -> Self {
        let grade = self.bench.1;
        let level = shipvivor_sim::Par::of(stage, grade).level;
        let run = if self.run.is_exercise() {
            let mut run = self.run.clone();
            run.restage(stage, grade, seed, level);
            run
        } else {
            Run::exercise(stage, grade, seed, level)
        };
        Self {
            bench: self.bench,
            ..Self::with_run(Mode::Patrol, run)
        }
    }
    /// What a run usually brings to the round test mode's yard stands before.
    pub fn par(&self) -> shipvivor_sim::Par {
        shipvivor_sim::Par::of(self.stage, self.grade)
    }
    /// Test mode: bring her back to the yard as she lay when she sailed, whether the
    /// sortie was won, lost or broken off, and note how it went. `false` with no sortie
    /// to come back from.
    pub fn return_to_yard(&mut self) -> bool {
        let Some(berth) = self.berth.take() else {
            return false;
        };
        let debrief = self.sortie_report();
        *self = Self {
            bench: self.bench,
            debrief: Some(debrief),
            ..Self::with_run(Mode::Patrol, berth)
        };
        true
    }
    /// How the sortie under way stands, in a line: how it ended and when, the stars it
    /// earned and the hull it left her.
    fn sortie_report(&self) -> String {
        let run = &self.run;
        let sailed = run.elapsed.max(0.) as u32;
        let clock = format!("{}:{:02}", sailed / 60, sailed % 60);
        let stars = match run.stars() {
            _ if run.stage == Stage::CrownDeep => String::new(),
            1 => ", 1 star".to_string(),
            n => format!(", {n} stars"),
        };
        match run.phase() {
            Phase::Drydock(_) | Phase::Intercept(_) => format!(
                "Cleared in {clock}{stars}, {}, hull {}%",
                if run.beat_clock() {
                    "inside the clock"
                } else {
                    "past the clock"
                },
                run.hull_percent()
            ),
            Phase::Won(_) => format!("Regent sunk in {clock}, hull {}%", run.hull_percent()),
            Phase::Lost => format!("Sunk after {clock}{stars}"),
            _ => format!(
                "Broken off after {clock}{stars}, hull {}%",
                run.hull_percent()
            ),
        }
    }
    /// The yard opens on the first bay that can take another turret, with nothing
    /// ordered. A test sortie that cleared its sea comes back to its yard as it sailed.
    pub fn open_dock(&mut self) {
        self.return_to_yard();
        self.dock_plan = None;
        self.dock_revision += 1;
        let bay = self.turret_bay().unwrap_or(1);
        self.select_section(bay);
    }
    pub fn select_section(&mut self, at: usize) {
        if at < self.ship.design().modules().len() {
            self.dock_module = at;
            self.dock_option = None;
            self.dock_receipt = None;
        }
    }
    /// Pick out one of the selected section's refits to read its figures.
    pub fn focus_option(&mut self, index: usize) {
        if index < self.dock_jobs().len() {
            self.dock_option = Some(index);
        }
    }
    /// The section an order is listed under: the selected one, or none for work on the
    /// hull as a whole.
    fn listed_under(&self, job: DockJob) -> Option<usize> {
        (!self.hull_jobs().contains(&job)).then_some(self.dock_module)
    }
    /// What ordering `job` would book from the points, or `None` if the yard cannot make
    /// it or it would leave the ship as she is. Work that takes back an order already made
    /// gives that order's points back first, so a bore changed twice is paid for once.
    pub fn quote(&self, job: DockJob) -> Option<i32> {
        let mut run = self.run.clone();
        let mut plan = self.dock_plan.clone().unwrap_or_else(|| Plan::draw(&run));
        let was = plan.orders().to_vec();
        (plan.order(&mut run, job, self.listed_under(job)) && plan.orders() != was.as_slice())
            .then(|| i32::from(run.spent()) - i32::from(self.spent()))
    }
    /// Order one of the selected section's refits.
    pub fn order_option(&mut self, index: usize) -> bool {
        self.dock_jobs()
            .get(index)
            .copied()
            .is_some_and(|job| self.order(job))
    }
    /// Make an order on the planned ship, booking its points at once. Work on the ship as
    /// a whole ordered on a section is listed under that section, and hull work on the
    /// hull. A spliced section is selected, so its own refits are offered next, and a moved
    /// one keeps the selection. Moved from the keyboard's pick, the next step the same way
    /// is picked out, so Enter moves her on. `false` if the yard cannot make it.
    pub fn order(&mut self, job: DockJob) -> bool {
        let picked = self.dock_job() == Some(job);
        let on_hull = self.hull_jobs().contains(&job);
        let from = self.listed_under(job);
        let before = self.run.clone();
        let was = self
            .plan()
            .map_or_else(Vec::new, |plan| plan.orders().to_vec());
        let plan = self.dock_plan.get_or_insert_with(|| Plan::draw(&self.run));
        let made = plan.order(&mut self.run, job, from);
        if plan.is_empty() {
            self.dock_plan = None;
        }
        // An order turned down stays picked out, so its figures say why.
        if !made {
            return false;
        }
        self.dock_option = None;
        // Refitted to what she already is: nothing is ordered.
        if self.plan().map_or(&[][..], Plan::orders) == was.as_slice() {
            return true;
        }
        self.dock_revision += 1;
        match job {
            DockJob::Insert { at, .. } => self.select_section(at),
            DockJob::Move { from, to } => {
                self.select_section(to);
                if picked {
                    let again = DockJob::Move {
                        from: to,
                        to: if to < from {
                            to.wrapping_sub(1)
                        } else {
                            to + 1
                        },
                    };
                    self.dock_option = self.dock_jobs().iter().position(|&j| j == again);
                }
            }
            _ => {}
        }
        self.dock_receipt = Some(Receipt {
            job,
            on_hull,
            spent: i32::from(self.spent()) - i32::from(before.spent()),
            before,
        });
        true
    }
    /// Take back an order not yet built, with any that waited on it.
    pub fn undo_order(&mut self, index: usize) {
        if let Some(plan) = &mut self.dock_plan {
            plan.undo(&mut self.run, index);
            if plan.is_empty() {
                self.dock_plan = None;
            }
            self.dock_revision += 1;
            self.dock_option = None;
            self.dock_receipt = None;
            let last = self.ship.design().modules().len() - 1;
            self.dock_module = self.dock_module.min(last);
        }
    }
    /// Build every order in the plan. They can no longer be taken back. The plan is
    /// returned so the yard can show what was built.
    pub fn finalize(&mut self) -> Option<Plan> {
        let plan = self.dock_plan.take()?;
        self.dock_revision += 1;
        self.dock_option = None;
        self.dock_receipt = None;
        Some(plan)
    }
    /// Orders made in the yard and not yet built.
    pub fn plan(&self) -> Option<&Plan> {
        self.dock_plan.as_ref()
    }
    /// Sail from the yard for the chart. Points left unspent are banked for the next yard,
    /// so nothing is lost and one press sails. Test mode's yard sails her straight into
    /// its sea, keeping her as she lies for the sortie to come back to: `true` then, for
    /// the caller to chart the sea.
    pub fn sail_from_dock(&mut self) -> bool {
        let berth = self.run.is_exercise().then(|| self.run.clone());
        if !self.leave_dock() {
            return false;
        }
        self.debrief = None;
        self.berth = berth;
        self.berth.is_some()
    }
    /// The refit picked out, whose figures the yard shows and which Enter orders.
    pub fn dock_job(&self) -> Option<DockJob> {
        if !matches!(self.screen(), Screen::Drydock(_)) {
            return None;
        }
        self.dock_jobs().get(self.dock_option?).copied()
    }
    /// The ship as the yard shows her: with every order in the plan already made.
    pub fn display_design(&self) -> Cow<'_, Design> {
        Cow::Borrowed(self.ship.design().as_ref())
    }
    pub fn display_fittings(&self) -> Fittings {
        self.fittings
    }
    pub fn hull_shape(&self) -> shipvivor_encounter::Hull {
        shipvivor_encounter::Hull::breakwater(self.ship.design())
    }

    pub fn screen(&self) -> Screen {
        match self.mode {
            Mode::Title => match self.title_page {
                TitlePage::Menu => Screen::Title,
                TitlePage::Customise => Screen::Customise,
                TitlePage::Options => Screen::Options,
                TitlePage::Departure => Screen::Departure,
                TitlePage::Exercise => Screen::Exercise,
            },
            Mode::Trial => Screen::Trial,
            Mode::Paused if self.chart_roll.is_some() => Screen::PassageMap,
            Mode::Paused => Screen::Paused,
            Mode::Rewind => Screen::Rewind,
            Mode::Patrol if self.clearance_left > 0. => Screen::Secured,
            Mode::Patrol => match self.run.phase() {
                Phase::Sailing => Screen::Sailing,
                Phase::Refit => Screen::Refit,
                // Level-ups still owed are dealt as hands before the yard opens.
                Phase::Drydock(_) if self.fittings_remaining() > 0 => Screen::Refit,
                Phase::Drydock(reason) => Screen::Drydock(reason),
                Phase::Chart => Screen::Chart,
                Phase::Intercept(reason) => Screen::Intercept(reason),
                Phase::Won(reason) => Screen::Won(reason),
                Phase::Lost => Screen::Lost,
            },
        }
    }

    /// The ship answers the helm and simulation time advances.
    pub fn under_way(&self) -> bool {
        matches!(self.screen(), Screen::Sailing | Screen::Trial)
    }
}

/// The ship has just been placed at the start of a course.
#[derive(Event)]
pub struct Embarked;
