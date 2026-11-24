//! Requisition and fitting rules at the between-leg yard.
use super::{CALIBRES, Card, DockModule, Fittings, LegEnd, Phase, RESERVE_STARS, Run, ShipBeam};
use shipvivor_artillery::TurretLayout;
use shipvivor_shipyard::{Component, Design, Frame, Module, Sheet};

pub const MAX_SECTIONS_PER_KIND: usize = 2;
pub const MAX_GUN_STATIONS: usize = 4;

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
            Self::CentreBattery => Module::new(Frame::Straight, [Component::MainGun]),
            Self::WingBattery => Module::new(
                Frame::BatteryGallery,
                [Component::MainGun, Component::MainGun],
            ),
            Self::Reserve => DockModule::Reserve.build(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DockJob {
    Section(DockModule),
    Fitting(Card),
    WidenBeam,
    Calibre(u16),
    TwinTurrets,
    Move { from: usize, to: usize },
    Replace { at: usize, with: DeckModule },
}

impl Run {
    /// The yard work offered by one section of the ship, counted from the bow. The bow
    /// and stern carry the hull's own work: lengthening it and widening the beam. A bay
    /// offers what can stand on it, the fittings of what stands there now, and its
    /// position. Bore, barrels, shells and secondaries apply ship-wide but are offered
    /// where those guns stand.
    pub fn section_jobs(&self, at: usize) -> Vec<DockJob> {
        use Card::*;
        let design = self.ship.design();
        let last = design.modules().len() - 1;
        let Some(module) = design.modules().get(at) else {
            return Vec::new();
        };
        if at == 0 || at == last {
            return vec![
                DockJob::Section(DockModule::Reserve),
                DockJob::Section(DockModule::EngineRoom),
                DockJob::WidenBeam,
            ];
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
        }
        if carries(Component::Engine) {
            jobs.push(DockJob::Fitting(Turbines));
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
}

impl DockJob {
    pub fn cost(self) -> u16 {
        match self {
            Self::Section(DockModule::EngineRoom) => 2,
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

    pub fn next_beam(&self) -> Option<ShipBeam> {
        // A successful first leg has already lengthened the hull by two frames.
        // This capital conversion leaves a clearance order to commission its battery.
        if self.beam_class() == ShipBeam::Escort
            && self.phase == Phase::Drydock(LegEnd::BasesDestroyed)
            && self.stars() >= RESERVE_STARS
            && self.ship.design().length() >= 40.
        {
            Some(ShipBeam::Capital)
        } else {
            self.beam_class().next()
        }
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
            _ if m.fit.contains(&Component::Engine) => "Auxiliary engine",
            _ if m.fit.contains(&Component::MainGun) => DeckModule::CentreBattery.name(),
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

    /// Paying for this job spends the last work order and sends the ship to the chart.
    pub fn closes_dock(&self, job: DockJob) -> bool {
        job.cost() > 0 && job.cost() == self.pending_cards
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
            DockJob::Section(kind) => {
                if self
                    .dock_modules()
                    .filter(|(_, fitted)| *fitted == kind)
                    .count()
                    >= MAX_SECTIONS_PER_KIND
                {
                    return None;
                }
                design
                    .insert(design.modules().len() - 2, kind.build())
                    .ok()?;
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

    /// Design figures with the selected frame spliced in, before materials are committed.
    pub fn projected_sheet(&self, section: DockModule) -> Sheet {
        let mut design = self.ship.design().as_ref().clone();
        design
            .insert(design.modules().len() - 2, section.build())
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
            DockJob::Section(kind) => self
                .ship
                .insert_module(self.ship.design().modules().len() - 2, kind.build()),
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
        self.pending_cards -= job.cost();
        if self.pending_cards == 0 {
            self.phase = Phase::Chart;
        }
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
        let reserve = DockJob::Section(DockModule::Reserve);
        let engine = DockJob::Section(DockModule::EngineRoom);
        assert!(!run.refit_in_dock(reserve));
        run.tick_sector(
            0.,
            Objectives {
                kills: 4,
                gates: 3,
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
        assert!(run.refit_in_dock(reserve));
        assert_eq!(run.ship.design().sheet(), sheet);
        assert!(!run.refit_in_dock(reserve));
        assert!(!run.refit_in_dock(engine));
        assert_eq!(run.fittings_remaining(), 1);
        assert!(run.refit_in_dock(DockJob::Fitting(Card::TripleTurrets)));
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
                gates: 3,
                crates,
            },
            0,
            true,
        );
        run
    }

    #[test]
    fn earning_the_long_hull_opens_an_eighteen_inch_battery_in_one_dock_visit() {
        let mut run = clear_sector(0);
        assert_eq!(run.fittings_remaining(), 2);
        assert!(!run.refit_in_dock(DockJob::Calibre(18)));
        assert_eq!(run.beam_class(), ShipBeam::Escort);
        assert_eq!(
            run.projected_design(DockJob::WidenBeam).unwrap().beam(),
            14.
        );
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        assert_eq!(run.beam_class(), ShipBeam::Capital);
        assert!(run.refit_in_dock(DockJob::Calibre(18)));
        assert!(run.depart(Stage::OuterRoad));
        assert_eq!(
            (
                run.ship.design().beam(),
                run.fittings.round().calibre_inches()
            ),
            (14., 18.)
        );
        assert!(!run.fittings.can_fit(Card::Shells) && !run.fittings.can_fit(Card::QuadTurrets));
        assert!(!run.refit_in_dock(DockJob::Calibre(8)));
    }

    #[test]
    fn broadening_an_ordinary_hull_opens_the_cruiser_branch_and_field_cards_respect_it() {
        let mut run = Run::default();
        run.tick(600., Objectives::default(), 0);
        assert_eq!(run.next_beam(), Some(ShipBeam::Cruiser));
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        assert!(run.depart(Stage::OuterRoad));
        let mut fit = run.fittings;
        for inches in [6, 8, 12] {
            assert!(fit.can_fit(Card::Shells));
            fit = fit.with(Card::Shells);
            assert_eq!(fit.calibre(), inches);
        }
        assert!(!fit.offers().contains(&Card::Shells));
        assert_eq!(fit.with(Card::Shells), fit);
        assert!(!fit.can_fit(Card::TripleTurrets));
    }

    #[test]
    fn downsizing_a_quad_cradle_frees_room_for_capital_guns_without_spending_an_order() {
        let mut run = clear_sector(0);
        run.fittings = run
            .fittings
            .with(Card::TripleTurrets)
            .with(Card::QuadTurrets);
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        assert_eq!(run.fittings.capacity(), 12);
        assert!(!run.refit_in_dock(DockJob::Calibre(18)));
        assert!(run.refit_in_dock(DockJob::TwinTurrets));
        assert_eq!(run.fittings_remaining(), 1);
        assert!(run.refit_in_dock(DockJob::Calibre(18)));
    }

    #[test]
    fn a_wing_gallery_carries_smaller_guns_beside_an_eighteen_inch_centre_battery() {
        let mut run = clear_sector(8);
        let replace = |at, with| DockJob::Replace { at, with };
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
        assert!(run.refit_in_dock(replace(aft, DeckModule::WingBattery)));
        assert_eq!((run.fittings.capacity(), run.fittings.calibre()), (8, 8));
        assert!(!run.fittings.can_fit(Card::Shells));
        assert_eq!(run.ship.design().gun_stations().len(), 4);
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
        assert!(split.adds_turret(&run) && run.closes_dock(split));
        // An empty bay bought with the only order would sail without a gun on it.
        assert!(run.closes_dock(DockJob::Section(DockModule::Reserve)));
        assert!(run.refit_in_dock(split));
        assert_eq!((run.main_turrets(), run.phase()), (3, Phase::Chart));
    }

    #[test]
    fn two_orders_buy_a_reserve_bay_and_a_centre_turret_to_stand_on_it() {
        let mut run = Run::default();
        run.tick_sector(0., Objectives::default(), 0, true);
        assert_eq!((run.fittings_remaining(), run.main_turrets()), (2, 2));
        let bay = DockJob::Section(DockModule::Reserve);
        assert!(!run.closes_dock(bay) && run.refit_in_dock(bay));
        let at = run.turret_bay().unwrap();
        assert_eq!(run.module_name(at), DeckModule::Reserve.name());
        let add = DockJob::Replace {
            at,
            with: DeckModule::CentreBattery,
        };
        assert!(add.adds_turret(&run) && run.closes_dock(add));
        assert!(run.refit_in_dock(add));
        assert!(run.depart(Stage::OuterRoad));
        assert_eq!(run.ship.design().gun_stations().len(), 3);
        assert!(run.ship.design().gun_stations().iter().all(|g| !g.paired()));
    }

    #[test]
    fn each_section_offers_its_own_work_and_never_what_it_already_carries() {
        use DeckModule::*;
        let run = clear_sector(0);
        let last = run.ship.design().modules().len() - 1;
        for end in [0, last] {
            assert_eq!(
                run.section_jobs(end),
                [
                    DockJob::Section(DockModule::Reserve),
                    DockJob::Section(DockModule::EngineRoom),
                    DockJob::WidenBeam
                ]
            );
        }
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
        assert!(!run.closes_dock(DockJob::Move {
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
