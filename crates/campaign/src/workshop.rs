//! Requisition and fitting rules at the between-leg yard.
use super::{CALIBRES, Card, DockModule, Fittings, Phase, Run, ShipBeam};
use shipvivor_artillery::TurretLayout;
use shipvivor_shipyard::{Component, Design, Frame, Module, Sheet};

pub const MAX_SECTIONS_PER_KIND: usize = 2;
pub const MAX_GUN_STATIONS: usize = 4;
/// The basin takes a hull this long and no longer.
pub const MAX_LENGTH: f32 = 60.;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeckModule {
    CentreBattery,
    WingBattery,
    Reserve,
}
impl DeckModule {
    pub fn name(self) -> &'static str {
        match self {
            Self::CentreBattery => "Centre turret",
            Self::WingBattery => "Wing turrets",
            Self::Reserve => "Reserve bay",
        }
    }
    fn build(self) -> Module {
        match self {
            Self::CentreBattery => DockModule::Turret.build(),
            Self::WingBattery => Module::new(
                Frame::BatteryGallery,
                [Component::MainGun, Component::MainGun],
            ),
            Self::Reserve => DockModule::Reserve.build(),
        }
    }
}

impl DockModule {
    /// Work orders to build and splice in one.
    pub fn cost(self) -> u16 {
        match self {
            Self::Reserve | Self::Floatplane | Self::Missiles => 1,
            Self::EngineRoom | Self::Turret => 2,
        }
    }
    /// The most sections of this kind one hull carries. Turrets are counted by
    /// [`MAX_GUN_STATIONS`] instead, wing turrets included.
    pub fn most(self) -> usize {
        match self {
            Self::Reserve | Self::EngineRoom | Self::Missiles => MAX_SECTIONS_PER_KIND,
            Self::Floatplane => 1,
            Self::Turret => MAX_GUN_STATIONS,
        }
    }
    /// The narrowest beam that has room for it. Missile magazines need a cruiser's.
    pub fn beam(self) -> ShipBeam {
        match self {
            Self::Missiles => ShipBeam::Cruiser,
            _ => ShipBeam::Escort,
        }
    }
}

/// Why the yard turns a job down, whatever work orders are left.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Refusal {
    /// The hull already carries the most of these it can.
    Full(usize),
    /// The hull must first be widened to this beam.
    Beam(ShipBeam),
    /// The hull must first be lengthened to this many metres.
    Length(u16),
    /// The basin takes no longer hull.
    Longest,
    /// The beam is as wide as the yard builds.
    Widest,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DockJob {
    /// Splice a new section in so that it becomes module `at`, between bow and stern.
    Insert {
        at: usize,
        module: DockModule,
    },
    Fitting(Card),
    WidenBeam,
    Calibre(u16),
    TwinTurrets,
    Move {
        from: usize,
        to: usize,
    },
    Replace {
        at: usize,
        with: DeckModule,
    },
}

impl Run {
    /// Work on the hull as a whole rather than on any one section of her.
    pub fn hull_jobs(&self) -> Vec<DockJob> {
        vec![DockJob::WidenBeam, DockJob::Fitting(Card::Armor)]
    }

    /// Where a new section goes when no other place is asked for: ahead of the after
    /// turret, where the yard splices the sections a leg earns.
    pub fn default_gap(&self) -> usize {
        self.ship.design().modules().len() - 2
    }

    /// The yard work offered by one section of the ship, counted from the bow. Bow and
    /// stern are fixed. A bay offers what can stand on it, the fittings of what stands
    /// there now, and its position. Bore, barrels, shells and secondaries apply
    /// ship-wide but are offered where those guns stand.
    pub fn section_jobs(&self, at: usize) -> Vec<DockJob> {
        use Card::*;
        let design = self.ship.design();
        let last = design.modules().len() - 1;
        let Some(module) = design.modules().get(at) else {
            return Vec::new();
        };
        if at == 0 || at == last {
            return Vec::new();
        }
        let mut jobs = Vec::new();
        if !self.module_locked(at) {
            for with in [
                DeckModule::CentreBattery,
                DeckModule::WingBattery,
                DeckModule::Reserve,
            ] {
                if *module != with.build() {
                    jobs.push(DockJob::Replace { at, with });
                }
            }
        }
        let carries = |part| module.fit.contains(&part);
        if carries(Component::MainGun) {
            jobs.extend(CALIBRES.into_iter().map(DockJob::Calibre));
            jobs.push(DockJob::Fitting(
                if self.fittings.turrets == TurretLayout::Twin {
                    TripleTurrets
                } else {
                    QuadTurrets
                },
            ));
            if self.fittings.turrets != TurretLayout::Twin {
                jobs.push(DockJob::TwinTurrets);
            }
            jobs.push(DockJob::Fitting(if !self.fittings.armor_piercing() {
                ArmorPiercing
            } else if self.fittings.can_fit(Caps) {
                Caps
            } else {
                Velocity
            }));
            jobs.push(DockJob::Fitting(Loader));
            jobs.push(DockJob::Fitting(TrainingGear));
        }
        if carries(Component::CloseGun) {
            jobs.extend(
                [
                    MoreSecondaries,
                    TwinSecondaries,
                    SecondaryRange,
                    QuickSecondaries,
                ]
                .map(DockJob::Fitting),
            );
            // The AA guns stand on the bridge wings and abaft the funnel.
            if self.fittings.anti_air == 0 {
                jobs.push(DockJob::Fitting(AntiAir));
            } else {
                jobs.extend([MoreAntiAir, HeavyAntiAir, QuickAntiAir].map(DockJob::Fitting));
            }
        }
        if carries(Component::Engine) {
            jobs.push(DockJob::Fitting(Turbines));
            jobs.push(DockJob::Fitting(Hardening));
        }
        // Moves are free, so they come last.
        if at > 1 {
            jobs.push(DockJob::Move {
                from: at,
                to: at - 1,
            });
        }
        if at + 1 < last {
            jobs.push(DockJob::Move {
                from: at,
                to: at + 1,
            });
        }
        jobs
    }

    /// Why the yard cannot splice in a section of this kind anywhere, if it cannot.
    pub fn stock_refusal(&self, module: DockModule) -> Option<Refusal> {
        let design = self.ship.design();
        let fitted = if module == DockModule::Turret {
            design.count(Component::MainGun)
        } else {
            self.dock_modules()
                .filter(|(_, kind)| *kind == module)
                .count()
        };
        if fitted >= module.most() {
            Some(Refusal::Full(module.most()))
        } else if self.beam_class() < module.beam() {
            Some(Refusal::Beam(module.beam()))
        } else if design.length() + Frame::Straight.spec().length * design.scale > MAX_LENGTH {
            Some(Refusal::Longest)
        } else {
            None
        }
    }

    /// Why the hull cannot be widened another step, if it cannot.
    pub fn widen_refusal(&self) -> Option<Refusal> {
        let next = self.beam_class().next()?;
        (self.ship.design().length() < next.min_length())
            .then_some(Refusal::Length(next.min_length() as u16))
    }

    /// Why the hull cannot take another rank of armor plate until she is broader, if
    /// that is why. At the widest beam her last rank is simply her last.
    pub fn armor_refusal(&self) -> Option<Refusal> {
        let next = self.beam_class().next()?;
        (self.fittings.armor >= self.beam_class().most_armor()).then_some(Refusal::Beam(next))
    }

    /// Why the yard turns this job down, whatever work orders are left. `None` for jobs
    /// that are simply not legal in this place.
    pub fn refusal(&self, job: DockJob) -> Option<Refusal> {
        match job {
            DockJob::Insert { module, .. } => self.stock_refusal(module),
            DockJob::WidenBeam if self.beam_class().next().is_none() => Some(Refusal::Widest),
            DockJob::WidenBeam => self.widen_refusal(),
            DockJob::Fitting(Card::Armor) => self.armor_refusal(),
            _ => None,
        }
    }
}

impl DockJob {
    pub fn cost(self) -> u16 {
        match self {
            Self::Insert { module, .. } => module.cost(),
            Self::Move { .. } | Self::TwinTurrets => 0,
            _ => 1,
        }
    }
    pub fn fits(self, run: &Run) -> bool {
        run.projected_design(self).is_some()
    }
    /// A legal exchange that leaves the ship with more main turrets than she has now.
    pub fn adds_turret(self, run: &Run) -> bool {
        matches!(self, Self::Replace { .. })
            && run
                .projected_design(self)
                .is_some_and(|d| d.gun_stations().len() > run.main_turrets())
    }
}

impl Run {
    pub fn beam_class(&self) -> ShipBeam {
        self.fittings.beam
    }

    /// The next step of beam, once the hull is long enough to carry it.
    pub fn next_beam(&self) -> Option<ShipBeam> {
        self.beam_class()
            .next()
            .filter(|_| self.widen_refusal().is_none())
    }

    pub fn module_name(&self, at: usize) -> &'static str {
        let Some(m) = self.ship.design().modules().get(at) else {
            return "No section";
        };
        match m.frame {
            Frame::Bow => "Bow",
            Frame::Stern => "Stern",
            Frame::BatteryGallery => DeckModule::WingBattery.name(),
            _ if m.fit.contains(&Component::Director) => "Bridge",
            _ if m.fit.contains(&Component::TorpedoRack) => "Machinery space",
            _ if m.fit.contains(&Component::Engine) => DockModule::EngineRoom.name(),
            _ if m.fit.contains(&Component::MainGun) => DeckModule::CentreBattery.name(),
            _ if m.fit.contains(&Component::Catapult) => DockModule::Floatplane.name(),
            _ if m.fit.contains(&Component::MissileRack) => DockModule::Missiles.name(),
            _ => DeckModule::Reserve.name(),
        }
    }

    pub fn main_turrets(&self) -> usize {
        self.ship.design().gun_stations().len()
    }

    /// The first interior bay that can take another main turret. An empty reserve bay
    /// that takes a full-bore centre turret comes before a centre turret split into wings.
    pub fn turret_bay(&self) -> Option<usize> {
        let bays = 1..self.ship.design().modules().len() - 1;
        [DeckModule::CentreBattery, DeckModule::WingBattery]
            .into_iter()
            .find_map(|with| {
                bays.clone()
                    .find(|&at| DockJob::Replace { at, with }.adds_turret(self))
            })
    }

    /// Paying for this job spends the last work order.
    pub fn spends_last_order(&self, job: DockJob) -> bool {
        job.cost() > 0 && job.cost() == self.pending_cards
    }

    /// Sail from the yard for the chart. The yard keeps her until every order is spent;
    /// she stays in dock after the last one, for free work, until she is sailed.
    pub fn leave_dock(&mut self) -> bool {
        if !matches!(self.phase, Phase::Drydock(_)) || self.pending_cards > 0 {
            return false;
        }
        self.phase = Phase::Chart;
        true
    }

    /// Bow, stern, propulsion and the bridge stay aboard. All interior modules may move.
    pub fn module_locked(&self, at: usize) -> bool {
        self.ship.design().modules().get(at).is_none_or(|module| {
            !Frame::MIDSHIPS.contains(&module.frame)
                || module
                    .fit
                    .iter()
                    .any(|c| c.vital() || *c == Component::Director)
        })
    }

    /// A legal trial plan, independent of the player's remaining work orders.
    pub fn projected_design(&self, job: DockJob) -> Option<Design> {
        let mut design = self.ship.design().as_ref().clone();
        match job {
            DockJob::Insert { at, module } => {
                if self.stock_refusal(module).is_some()
                    || !(1..design.modules().len()).contains(&at)
                {
                    return None;
                }
                design.insert(at, module.build()).ok()?;
                if design.count(Component::MainGun) > MAX_GUN_STATIONS {
                    return None;
                }
            }
            DockJob::WidenBeam => design.widen(self.next_beam()?.metres()),
            DockJob::Calibre(inches) => {
                if !CALIBRES.contains(&inches)
                    || inches == self.fittings.calibre()
                    || inches > self.fittings.capacity()
                {
                    return None;
                }
            }
            DockJob::TwinTurrets if self.fittings.turrets == TurretLayout::Twin => return None,
            DockJob::TwinTurrets => {}
            DockJob::Fitting(card) => {
                if !self.fittings.can_fit(card) {
                    return None;
                }
            }
            DockJob::Move { from, to } => {
                let midships = 1..design.modules().len() - 1;
                if from == to || !midships.contains(&from) || !midships.contains(&to) {
                    return None;
                }
                design.move_module(from, to).ok()?;
            }
            DockJob::Replace { at, with } => {
                if self.module_locked(at) {
                    return None;
                }
                if with == DeckModule::Reserve
                    && self
                        .dock_modules()
                        .filter(|(_, kind)| *kind == DockModule::Reserve)
                        .count()
                        >= MAX_SECTIONS_PER_KIND
                {
                    return None;
                }
                let replacement = with.build();
                if design.modules()[at] == replacement {
                    return None;
                }
                design.replace_module(at, replacement).ok()?;
                if !(1..=MAX_GUN_STATIONS).contains(&design.count(Component::MainGun)) {
                    return None;
                }
            }
        }
        Some(design)
    }

    pub fn projected_fittings(&self, job: DockJob) -> Fittings {
        let Some(design) = self.projected_design(job) else {
            return self.fittings;
        };
        let mut fit = self.fittings;
        match job {
            DockJob::Fitting(card) => fit = fit.with(card),
            DockJob::Calibre(inches) => {
                fit.shells = CALIBRES.iter().position(|&inch| inch == inches).unwrap() as u16
            }
            DockJob::TwinTurrets => fit.turrets = TurretLayout::Twin,
            _ => {}
        }
        let beam = if job == DockJob::WidenBeam {
            self.next_beam().unwrap()
        } else {
            self.beam_class()
        };
        fit.on_deck(beam, design.gun_stations().iter().all(|s| s.paired()))
    }

    /// Design figures with a section spliced in where the yard puts earned sections,
    /// before materials are committed.
    pub fn projected_sheet(&self, section: DockModule) -> Sheet {
        let mut design = self.ship.design().as_ref().clone();
        design
            .insert(self.default_gap(), section.build())
            .expect("a straight midship frame");
        design.sheet()
    }

    /// Commit a complete legal yard order, preserving damage to every retained compartment.
    pub fn refit_in_dock(&mut self, job: DockJob) -> bool {
        if !matches!(self.phase, Phase::Drydock(_))
            || self.pending_cards < job.cost()
            || !job.fits(self)
        {
            return false;
        }
        let fit = self.projected_fittings(job);
        let applied = match job {
            DockJob::Insert { at, module } => self.ship.insert_module(at, module.build()),
            DockJob::Move { from, to } => self.ship.move_module(from, to),
            DockJob::Replace { at, with } => self.ship.replace_module(at, with.build()),
            DockJob::WidenBeam => {
                self.ship.widen(fit.beam.metres());
                Ok(())
            }
            _ => Ok(()),
        };
        if applied.is_err() {
            return false;
        }
        self.fittings = fit;
        self.refit_protection();
        self.pending_cards -= job.cost();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Objectives, Stage};

    #[test]
    fn the_yard_previews_without_spending_and_only_installs_affordable_work() {
        let mut run = Run::default();
        let insert = |run: &Run, module| DockJob::Insert {
            at: run.default_gap(),
            module,
        };
        assert!(!run.refit_in_dock(insert(&run, DockModule::Reserve)));
        run.tick_sector(
            0.,
            Objectives {
                kills: 4,
                destroyed_drydock: true,
                crates: 0,
            },
            0,
            true,
        );
        let installed = run.ship.design().length();
        let sheet = run.projected_sheet(DockModule::Reserve);
        assert_eq!(sheet.length, installed + 5.);
        assert_eq!(run.ship.design().length(), installed);
        assert_eq!(run.fittings_remaining(), 2);
        assert!(run.refit_in_dock(insert(&run, DockModule::Reserve)));
        assert_eq!(run.ship.design().sheet(), sheet);
        assert!(!run.refit_in_dock(insert(&run, DockModule::Reserve)));
        assert!(!run.refit_in_dock(insert(&run, DockModule::EngineRoom)));
        assert_eq!(run.fittings_remaining(), 1);
        assert!(run.refit_in_dock(DockJob::Fitting(Card::TripleTurrets)));
        assert!(run.leave_dock());
        assert!(run.depart(Stage::OuterRoad));
        assert_eq!(run.ship.design().length(), 45.);
        assert_eq!(run.fittings.turrets.count(), 3);
    }

    fn clear_sector(crates: u16) -> Run {
        let mut run = Run::default();
        run.tick_sector(
            0.,
            Objectives {
                kills: 4,
                destroyed_drydock: true,
                crates,
            },
            0,
            true,
        );
        run
    }

    #[test]
    fn the_beam_widens_one_step_at_a_time_and_only_on_a_hull_long_enough_for_it() {
        // A short leg leaves her at her built length with one order.
        let mut run = Run::default();
        run.tick(600., Objectives::default(), 0);
        assert_eq!(run.ship.design().length(), 30.);
        assert_eq!(run.widen_refusal(), Some(Refusal::Length(35)));
        assert!(!DockJob::WidenBeam.fits(&run) && !run.refit_in_dock(DockJob::WidenBeam));

        // A clean sweep earns two sections, so she reaches the yard at 40 m.
        let mut run = clear_sector(0);
        assert_eq!(
            (run.ship.design().length(), run.fittings_remaining()),
            (40., 2)
        );
        assert_eq!(
            run.projected_design(DockJob::WidenBeam).unwrap().beam(),
            ShipBeam::Cruiser.metres()
        );
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        assert_eq!(run.beam_class(), ShipBeam::Cruiser);
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        assert_eq!(run.beam_class(), ShipBeam::Capital);
        assert_eq!(run.refusal(DockJob::WidenBeam), Some(Refusal::Widest));
        // Every step keeps her more than four beams long.
        let design = run.ship.design();
        assert!(design.length() / design.beam() >= 4.2);
        assert_eq!(run.fittings.capacity(), 18);
        assert!(DockJob::Calibre(18).fits(&run));
        // Her orders are spent, but she stays in dock until she is sailed.
        assert!(matches!(run.phase(), Phase::Drydock(_)));
        assert!(run.leave_dock());
        assert_eq!(run.phase(), Phase::Chart);
    }

    #[test]
    fn thicker_armor_waits_for_a_broader_beam_which_waits_for_a_longer_hull() {
        // At her built length and beam she carries two ranks of armor and no more.
        let mut run = Run::default();
        run.tick(600., Objectives::default(), 0);
        run.fittings.armor = ShipBeam::Escort.most_armor();
        let armor = DockJob::Fitting(Card::Armor);
        assert_eq!(run.refusal(armor), Some(Refusal::Beam(ShipBeam::Cruiser)));
        assert!(!run.refit_in_dock(armor));
        assert_eq!(run.refusal(DockJob::WidenBeam), Some(Refusal::Length(35)));

        // Lengthened she can be widened, and widened she takes the next rank.
        let mut run = clear_sector(0);
        run.fittings.armor = ShipBeam::Escort.most_armor();
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        assert_eq!(run.refusal(armor), None);
        assert!(run.refit_in_dock(armor));
        assert_eq!(run.ship.design().protection().thickness, 10.);
    }

    #[test]
    fn a_section_bought_in_the_yard_lengthens_her_enough_to_widen() {
        let mut run = Run::default();
        run.tick_sector(0., Objectives::default(), 0, true);
        assert_eq!(run.ship.design().length(), 35.);
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        assert_eq!(run.widen_refusal(), Some(Refusal::Length(40)));
        let bay = DockJob::Insert {
            at: 1,
            module: DockModule::Reserve,
        };
        assert!(run.refit_in_dock(bay));
        assert_eq!(run.ship.design().length(), 40.);
        assert_eq!(run.widen_refusal(), None);
    }

    #[test]
    fn downsizing_a_quad_cradle_frees_room_for_capital_guns_without_spending_an_order() {
        let mut run = clear_sector(8);
        run.fittings = run
            .fittings
            .with(Card::TripleTurrets)
            .with(Card::QuadTurrets);
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        assert_eq!(run.fittings.capacity(), 12);
        assert!(!run.refit_in_dock(DockJob::Calibre(18)));
        let left = run.fittings_remaining();
        assert!(run.refit_in_dock(DockJob::TwinTurrets));
        assert_eq!(run.fittings_remaining(), left);
        assert!(run.refit_in_dock(DockJob::Calibre(18)));
    }

    #[test]
    fn a_wing_gallery_carries_smaller_guns_beside_an_eighteen_inch_centre_battery() {
        let mut run = clear_sector(8);
        let replace = |at, with| DockJob::Replace { at, with };
        assert_eq!(run.fittings_remaining(), 4);
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        assert!(run.refit_in_dock(DockJob::Calibre(18)));
        assert!(run.refit_in_dock(replace(1, DeckModule::WingBattery)));
        let ship = run.ship.design();
        let guns = ship.gun_stations();
        assert_eq!(guns.len(), 3);
        assert_eq!(run.fittings.capacity(), 18);
        let port = run.fittings.for_station(&guns[0]);
        let main = run.fittings.for_station(&guns[2]);
        assert_eq!((port.calibre(), main.calibre()), (8, 18));
        assert!(port.reload() < main.reload());
        assert!(guns[0].abeam < 0. && guns[1].abeam > 0.);
        assert_eq!(guns[0].fore, guns[1].fore);
        let aft = guns[2].berth.module;
        let wings = replace(aft, DeckModule::WingBattery);
        assert_eq!(
            run.projected_fittings(wings).capacity(),
            8,
            "wings fore and aft leave no full-bore station"
        );
        assert_eq!(run.projected_design(wings).unwrap().gun_stations().len(), 4);
    }

    #[test]
    fn a_single_order_dock_can_still_add_a_turret_by_splitting_one_into_wings() {
        let mut run = Run::default();
        run.tick(600., Objectives::default(), 0);
        assert_eq!((run.fittings_remaining(), run.main_turrets()), (1, 2));
        assert_eq!(run.turret_bay(), Some(1));
        let split = DockJob::Replace {
            at: 1,
            with: DeckModule::WingBattery,
        };
        assert!(split.adds_turret(&run) && run.spends_last_order(split));
        // An empty bay bought with the only order would sail without a gun on it.
        assert!(run.spends_last_order(DockJob::Insert {
            at: 1,
            module: DockModule::Reserve
        }));
        assert!(!run.leave_dock(), "an order is still to spend");
        assert!(run.refit_in_dock(split));
        assert_eq!(run.main_turrets(), 3);
        assert!(run.leave_dock());
        assert_eq!(run.phase(), Phase::Chart);
    }

    #[test]
    fn two_orders_buy_a_reserve_bay_and_a_centre_turret_to_stand_on_it() {
        let mut run = Run::default();
        run.tick_sector(0., Objectives::default(), 0, true);
        assert_eq!((run.fittings_remaining(), run.main_turrets()), (2, 2));
        let bay = DockJob::Insert {
            at: run.default_gap(),
            module: DockModule::Reserve,
        };
        assert!(!run.spends_last_order(bay) && run.refit_in_dock(bay));
        let at = run.turret_bay().unwrap();
        assert_eq!(run.module_name(at), DeckModule::Reserve.name());
        let add = DockJob::Replace {
            at,
            with: DeckModule::CentreBattery,
        };
        assert!(add.adds_turret(&run) && run.spends_last_order(add));
        assert!(run.refit_in_dock(add));
        assert!(run.leave_dock());
        assert!(run.depart(Stage::OuterRoad));
        assert_eq!(run.ship.design().gun_stations().len(), 3);
        assert!(run.ship.design().gun_stations().iter().all(|g| !g.paired()));
    }

    #[test]
    fn each_section_offers_its_own_work_and_never_what_it_already_carries() {
        use DeckModule::*;
        let run = clear_sector(0);
        let last = run.ship.design().modules().len() - 1;
        // Bow and stern are fixed. The hull's own work is offered for the whole ship.
        for end in [0, last] {
            assert!(run.section_jobs(end).is_empty());
        }
        assert_eq!(
            run.hull_jobs(),
            [DockJob::WidenBeam, DockJob::Fitting(Card::Armor)]
        );
        let reserve = run
            .dock_modules()
            .find(|(_, k)| *k == DockModule::Reserve)
            .unwrap()
            .0;
        let bay = run.section_jobs(reserve);
        assert_eq!(
            bay[0],
            DockJob::Replace {
                at: reserve,
                with: CentreBattery
            }
        );
        assert!(!bay.contains(&DockJob::Replace {
            at: reserve,
            with: Reserve
        }));
        assert!(!bay.iter().any(|j| matches!(j, DockJob::Calibre(_))));

        let gun = run.section_jobs(1);
        assert!(gun.contains(&DockJob::Calibre(18)));
        assert!(gun.contains(&DockJob::Fitting(Card::Loader)));
        assert!(gun.contains(&DockJob::Replace {
            at: 1,
            with: WingBattery
        }));
        assert!(!gun.contains(&DockJob::Replace {
            at: 1,
            with: CentreBattery
        }));
        assert!(!gun.contains(&DockJob::Move { from: 1, to: 0 }));
        assert_eq!(gun.last(), Some(&DockJob::Move { from: 1, to: 2 }));

        let bridge = run.section_jobs(2);
        assert!(bridge.contains(&DockJob::Fitting(Card::MoreSecondaries)));
        assert!(!bridge.iter().any(|j| matches!(j, DockJob::Replace { .. })));
        assert!(
            run.section_jobs(3)
                .contains(&DockJob::Fitting(Card::Turbines))
        );
        assert!(run.section_jobs(last + 1).is_empty());
    }

    #[test]
    fn an_earned_reserve_bay_is_offered_for_a_turret_before_any_split() {
        let run = clear_sector(0);
        let reserve = run
            .dock_modules()
            .find(|(_, k)| *k == DockModule::Reserve)
            .unwrap()
            .0;
        assert_eq!(run.turret_bay(), Some(reserve));
        assert!(!run.spends_last_order(DockJob::Move {
            from: reserve,
            to: reserve - 1
        }));
    }

    #[test]
    fn invalid_layout_edits_leave_the_real_ship_and_all_work_orders_untouched() {
        let mut run = clear_sector(0);
        let guns = run.ship.design().gun_stations();
        let replace = |at, with| DockJob::Replace { at, with };
        let reserve = run
            .dock_modules()
            .find(|(_, k)| *k == DockModule::Reserve)
            .unwrap()
            .0;
        for at in [0, 2, 3, run.ship.design().modules().len() - 1, usize::MAX] {
            assert!(!run.refit_in_dock(replace(at, DeckModule::WingBattery)));
        }
        for (from, to) in [(2, 0), (1, 1), (usize::MAX, 1)] {
            assert!(!run.refit_in_dock(DockJob::Move { from, to }));
        }
        assert_eq!(run.fittings_remaining(), 2);
        assert_eq!(run.ship.design().gun_stations(), guns);
        assert!(run.refit_in_dock(replace(reserve, DeckModule::WingBattery)));
        assert!(!run.refit_in_dock(replace(1, DeckModule::WingBattery)));
        assert_eq!(run.ship.design().gun_stations().len(), 4);

        let mut run = clear_sector(0);
        let rear = run.ship.design().gun_stations()[1].berth.module;
        assert!(run.refit_in_dock(replace(1, DeckModule::Reserve)));
        assert!(!run.refit_in_dock(replace(rear, DeckModule::Reserve)));
        assert_eq!(run.fittings_remaining(), 1);
    }
}
