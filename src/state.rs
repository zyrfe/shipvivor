//! Game state shared by every module. `main` advances it and the views read it.
use crate::passage::Passage;
use bevy::prelude::*;
use shipvivor_campaign::{DockJob, Fittings, LegEnd, Phase, Plan, Run, Stage};
use shipvivor_encounter::{Ammo, Battle, Control, Volley};
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
    /// The filling the crew loads into each barrel as it empties.
    pub ammo: Ammo,
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
}

#[derive(Clone, Copy, Default)]
pub enum TitlePage {
    #[default]
    Menu,
    Customise,
    Options,
    Departure,
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
    Won(LegEnd),
    Lost,
}

#[derive(Resource, Deref, DerefMut)]
pub struct Campaign {
    pub chart_roll: Option<ChartRoll>,
    pub watch: crate::captain::Watch,
    pub clearance_left: f32,
    /// The refit on the selected section's list picked out with a click or the
    /// keyboard. Its figures are shown until it is ordered or another is picked.
    pub dock_option: Option<usize>,
    /// Work on the hull as a whole picked out on the hull panel, shown the same way.
    pub dock_draft: Option<DockJob>,
    /// The section of the ship selected in the drydock, counted from the bow.
    pub dock_module: usize,
    /// Orders made in the yard and not yet built. The ship as she stands carries them
    /// already, so every view of her is the planned ship, and each can be taken back
    /// until the refit is built.
    pub dock_plan: Option<Plan>,
    /// Counts every change to the plan, so views of the ship know to refit.
    pub dock_revision: u32,
    pub title_page: TitlePage,
    pub hits: u32,
    pub sunk: u32,
    pub mode: Mode,
    /// The run itself. Its `seed` charts every leg and deals every level-up hand, and
    /// `ChartSeeds` draws a new one for each run.
    #[deref]
    run: Run,
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
            dock_draft: None,
            dock_module: 1,
            dock_plan: None,
            dock_revision: 0,
            title_page: TitlePage::Menu,
            hits: 0,
            sunk: 0,
            run: Run::default(),
        }
    }

    /// The refits offered by the selected section of the ship.
    pub fn dock_jobs(&self) -> Vec<DockJob> {
        self.section_jobs(self.dock_module)
    }
    /// The yard opens on the first bay that can take another turret, with nothing
    /// ordered.
    pub fn open_dock(&mut self) {
        self.dock_plan = None;
        self.dock_revision += 1;
        let bay = self.turret_bay().unwrap_or(1);
        self.select_section(bay);
    }
    pub fn select_section(&mut self, at: usize) {
        if at < self.ship.design().modules().len() {
            self.dock_module = at;
            self.dock_option = None;
            self.dock_draft = None;
        }
    }
    /// Pick out work on the hull panel to read its figures.
    pub fn pick_hull(&mut self, job: DockJob) {
        self.dock_draft = Some(job);
        self.dock_option = None;
    }
    /// Pick out one of the selected section's refits to read its figures.
    pub fn focus_option(&mut self, index: usize) {
        if index < self.dock_jobs().len() {
            self.dock_option = Some(index);
            self.dock_draft = None;
        }
    }
    /// Order one of the selected section's refits.
    pub fn order_option(&mut self, index: usize) -> bool {
        self.dock_jobs()
            .get(index)
            .copied()
            .is_some_and(|job| self.order(job))
    }
    /// Make an order on the planned ship. Work on the ship as a whole ordered on a
    /// section is listed under that section, and hull work on the hull. A spliced section
    /// is selected, so its own refits are offered next, and a moved one keeps the
    /// selection with the next step the same way picked out. `false` if the yard cannot
    /// make it.
    pub fn order(&mut self, job: DockJob) -> bool {
        let from = (!self.hull_jobs().contains(&job)).then_some(self.dock_module);
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
        self.dock_draft = None;
        self.dock_revision += 1;
        match job {
            DockJob::Insert { at, .. } => self.select_section(at),
            DockJob::Move { from, to } => {
                self.select_section(to);
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
            _ => {}
        }
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
        Some(plan)
    }
    /// Orders made in the yard and not yet built.
    pub fn plan(&self) -> Option<&Plan> {
        self.dock_plan.as_ref()
    }
    /// Sail from the yard for the chart. Points left unspent are banked for the next yard,
    /// so nothing is lost and one press sails.
    pub fn sail_from_dock(&mut self) {
        self.leave_dock();
    }
    /// The refit picked out, whose figures the yard shows and which Enter orders.
    pub fn dock_job(&self) -> Option<DockJob> {
        if !matches!(self.screen(), Screen::Drydock(_)) {
            return None;
        }
        self.dock_draft
            .or_else(|| self.dock_jobs().get(self.dock_option?).copied())
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
            },
            Mode::Trial => Screen::Trial,
            Mode::Paused if self.chart_roll.is_some() => Screen::PassageMap,
            Mode::Paused => Screen::Paused,
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
