//! Game state shared by every module. `main` advances it and the views read it.
use crate::passage::Passage;
use bevy::prelude::*;
use shipvivor_campaign::{DockJob, DockModule, Fittings, LegEnd, Phase, Run, Stage};
use shipvivor_encounter::{Ammo, Battle, Volley};
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
        self.fit_design(run.ship.design(), |station| {
            let local = run.fittings.for_station(station);
            (local.round(), local.turrets)
        });
        self.fit_secondaries(run.fittings.secondaries());
        self.fit_anti_air(run.fittings.anti_air());
        self.fit_training(run.fittings.training_gear);
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
}

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
    /// The refit previewed from the selected section's list. Selecting a section
    /// previews nothing until one of its refits is chosen.
    pub dock_option: Option<usize>,
    /// The section of the ship selected in the drydock, counted from the bow.
    pub dock_module: usize,
    /// Work previewed from outside the selected section's list: widening the hull, or a
    /// section from the yard's stock dropped onto the ship. It stands in for the option.
    pub dock_draft: Option<DockJob>,
    pub title_page: TitlePage,
    pub hits: u32,
    pub sunk: u32,
    pub mode: Mode,
    /// Charts every leg of the run. `ChartSeeds` draws a new one for each run.
    pub seed: u64,
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
            dock_module: 1,
            dock_draft: None,
            title_page: TitlePage::Menu,
            hits: 0,
            sunk: 0,
            seed: 0,
            run: Run::default(),
        }
    }

    /// The refits offered by the selected section of the ship.
    pub fn dock_jobs(&self) -> Vec<DockJob> {
        self.section_jobs(self.dock_module)
    }
    /// The yard opens on the first bay that can take another turret, previewing nothing.
    pub fn open_dock(&mut self) {
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
    /// Preview one of the selected section's refits.
    pub fn preview_option(&mut self, index: usize) {
        if index < self.dock_jobs().len() {
            self.dock_option = Some(index);
            self.dock_draft = None;
        }
    }
    /// Preview work that is not on the selected section's list.
    pub fn preview_draft(&mut self, job: DockJob) {
        self.dock_draft = Some(job);
        self.dock_option = None;
    }
    pub fn dock_job(&self) -> Option<DockJob> {
        if !matches!(self.screen(), Screen::Drydock(_)) {
            return None;
        }
        self.dock_draft
            .or_else(|| self.dock_jobs().get(self.dock_option?).copied())
    }
    pub fn display_design(&self) -> Cow<'_, Design> {
        self.dock_job()
            .and_then(|job| self.projected_design(job))
            .map_or_else(|| Cow::Borrowed(self.ship.design().as_ref()), Cow::Owned)
    }
    /// A section from the yard's stock being tried in place, and where it would go.
    pub fn preview_insert(&self) -> Option<(usize, DockModule)> {
        match self.dock_job() {
            Some(job @ DockJob::Insert { at, module }) if job.fits(self) => Some((at, module)),
            _ => None,
        }
    }
    pub fn display_fittings(&self) -> Fittings {
        self.dock_job()
            .map_or(self.fittings, |job| self.projected_fittings(job))
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
