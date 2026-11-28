//! A refit drawn up in the yard before it is built. Every order is made on the ship at
//! once, so every figure and every view of her is the planned ship's, and any order not
//! yet built can be taken back: the ship is rebuilt from where the last refit left her,
//! with her other orders made again in turn.
//!
//! Sections are known to the plan by a number of their own rather than their place
//! along the hull, so an order stays with its section when a section is spliced in
//! ahead of it or it is moved, and an order on a section that is taken back goes too.
use crate::{DockJob, Fittings, Run};
use shipvivor_shipyard::{Design, Ship};

/// A section of the hull as the plan knows it, the same through splices and moves.
type SectionId = u32;

/// One order in the plan, as it was last made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Order {
    /// The job as made on the planned ship, with its places as they stood then.
    pub job: DockJob,
    /// What the order costs in yard points.
    pub cost: u16,
    /// The section the job works on, or the section it splices in.
    target: Option<SectionId>,
    /// The section a spliced section went in ahead of.
    seam: Option<SectionId>,
    /// The section it was ordered on, which it is listed under. Work on the hull as a
    /// whole has none.
    anchor: Option<SectionId>,
}

/// The ship as the last refit left her.
#[derive(Clone)]
struct Built {
    ship: Ship,
    fittings: Fittings,
    points: u16,
    spent: u16,
}

/// Orders made in the yard and not yet built.
#[derive(Clone)]
pub struct Plan {
    built: Built,
    orders: Vec<Order>,
    /// The sections of the planned ship, bow to stern. The ship as built numbers them
    /// from zero, and sections spliced in take the numbers after.
    ids: Vec<SectionId>,
    next_id: SectionId,
}

impl Plan {
    /// A plan with no orders yet, over the ship as she stands.
    pub fn draw(run: &Run) -> Self {
        let sections = run.ship.design().modules().len() as SectionId;
        Self {
            built: Built {
                ship: run.ship.clone(),
                fittings: run.fittings,
                points: run.points,
                spent: run.spent,
            },
            orders: Vec::new(),
            ids: (0..sections).collect(),
            next_id: sections,
        }
    }

    pub fn orders(&self) -> &[Order] {
        &self.orders
    }

    pub fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }

    /// Yard points the orders cost together.
    pub fn cost(&self) -> u16 {
        self.orders.iter().map(|o| o.cost).sum()
    }

    /// The ship as the last refit left her.
    pub fn built_design(&self) -> &Design {
        self.built.ship.design()
    }

    pub fn built_fittings(&self) -> Fittings {
        self.built.fittings
    }

    fn place_of(&self, id: SectionId) -> Option<usize> {
        self.ids.iter().position(|&i| i == id)
    }

    /// The section of the planned ship an order is listed under, or `None` for work on
    /// the hull as a whole.
    pub fn place(&self, order: &Order) -> Option<usize> {
        order.anchor.and_then(|id| self.place_of(id))
    }

    /// Section `at` of the planned ship is spliced in by the plan.
    pub fn is_new(&self, at: usize) -> bool {
        let built = self.built.ship.design().modules().len() as SectionId;
        self.ids.get(at).is_some_and(|&id| id >= built)
    }

    /// Section `at` of the planned ship has its turrets or its place changed by the plan.
    pub fn is_refitted(&self, at: usize) -> bool {
        let Some(&id) = self.ids.get(at) else {
            return false;
        };
        self.orders.iter().any(|o| {
            o.target == Some(id)
                && matches!(
                    o.job,
                    DockJob::Replace { .. }
                        | DockJob::Barrels { .. }
                        | DockJob::Barbette { .. }
                        | DockJob::Calibre { .. }
                        | DockJob::Move { .. }
                )
        })
    }

    /// Order `job` on the planned ship, listed under section `from` when it is work on
    /// her as a whole ordered there. An order that undoes one already made takes it back
    /// instead: a turret refitted twice is refitted once, to the second choice, and a
    /// section moved back is not moved at all. `false`, with the plan as it was, if the
    /// yard cannot make it or her points do not cover it.
    pub fn order(&mut self, run: &mut Run, job: DockJob, from: Option<usize>) -> bool {
        let saved = self.orders.clone();
        let target = self.target_of(job);
        // A section moved back the way it last came cancels that move.
        if let (DockJob::Move { from, to }, Some(id)) = (job, target) {
            let back = self.orders.iter().rposition(|o| {
                o.target == Some(id)
                    && matches!(o.job, DockJob::Move { from: f, to: t }
                        if (t < f) != (to < from))
            });
            if let Some(back) = back {
                self.orders.remove(back);
                self.rebuild(run);
                return true;
            }
        }
        let supersedes = |o: &Order| {
            o.target.is_some()
                && o.target == target
                && matches!(
                    (o.job, job),
                    (
                        DockJob::Replace { .. }
                            | DockJob::Barrels { .. }
                            | DockJob::Barbette { .. }
                            | DockJob::Calibre { .. },
                        DockJob::Replace { .. },
                    ) | (DockJob::Barrels { .. }, DockJob::Barrels { .. })
                        | (DockJob::Barbette { .. }, DockJob::Barbette { .. })
                        | (DockJob::Calibre { .. }, DockJob::Calibre { .. })
                )
        };
        if self.orders.iter().any(supersedes) {
            self.orders.retain(|o| !supersedes(o));
            self.rebuild(run);
        }
        let anchor = match job {
            DockJob::Insert { .. } => None,
            _ => target.or_else(|| from.and_then(|at| self.ids.get(at).copied())),
        };
        let fresh = match job {
            DockJob::Insert { at, .. } => Order {
                job,
                cost: job.cost(),
                target: Some(self.next_id),
                seam: self.ids.get(at).copied(),
                anchor: Some(self.next_id),
            },
            _ => Order {
                job,
                cost: job.cost(),
                target,
                seam: None,
                anchor,
            },
        };
        let Some(job) = self.rebase(run, &fresh) else {
            self.orders = saved;
            self.rebuild(run);
            return false;
        };
        // Refitting a section back to what it is leaves no order.
        if unchanged(run, job) {
            return true;
        }
        if !self.make(run, Order { job, ..fresh }) {
            self.orders = saved;
            self.rebuild(run);
            return false;
        }
        if matches!(job, DockJob::Insert { .. }) {
            self.next_id += 1;
        }
        true
    }

    /// Take back order `index`, with any order that no longer stands without it.
    pub fn undo(&mut self, run: &mut Run, index: usize) {
        if index < self.orders.len() {
            self.orders.remove(index);
            self.rebuild(run);
        }
    }

    /// Take back every order.
    pub fn clear(&mut self, run: &mut Run) {
        self.orders.clear();
        self.rebuild(run);
    }

    /// The section a job works on, on the planned ship as she stands.
    fn target_of(&self, job: DockJob) -> Option<SectionId> {
        match job {
            DockJob::Replace { at, .. }
            | DockJob::Barrels { at, .. }
            | DockJob::Barbette { at, .. }
            | DockJob::Calibre { at, .. }
            | DockJob::Move { from: at, .. } => self.ids.get(at).copied(),
            _ => None,
        }
    }

    /// An order's job with its places, and the turret it starts from, as the planned
    /// ship stands now. `None` if the section it works on is gone.
    fn rebase(&self, run: &Run, order: &Order) -> Option<DockJob> {
        let design = run.ship.design();
        let at = || order.target.and_then(|id| self.place_of(id));
        let house = |at: usize| design.modules().get(at).map(|m| m.gunhouse);
        Some(match order.job {
            DockJob::Insert { at, module } => {
                let last = design.modules().len().saturating_sub(1).max(1);
                let at = order
                    .seam
                    .and_then(|id| self.place_of(id))
                    .unwrap_or(at)
                    .clamp(1, last);
                DockJob::Insert { at, module }
            }
            DockJob::Replace { with, .. } => DockJob::Replace { at: at()?, with },
            DockJob::Move { from, to } => {
                let here = at()?;
                let there = if to < from {
                    here.checked_sub(1)?
                } else {
                    here + 1
                };
                DockJob::Move {
                    from: here,
                    to: there,
                }
            }
            DockJob::Barrels { to, .. } => {
                let at = at()?;
                DockJob::Barrels {
                    at,
                    from: house(at)?.barrels,
                    to,
                }
            }
            DockJob::Barbette { to, .. } => {
                let at = at()?;
                DockJob::Barbette {
                    at,
                    from: house(at)?.tier,
                    to,
                }
            }
            DockJob::Calibre { to, .. } => {
                let at = at()?;
                DockJob::Calibre {
                    at,
                    from: house(at)?.bore,
                    to,
                }
            }
            job => job,
        })
    }

    /// Make an order on the planned ship and keep it, if the yard can.
    fn make(&mut self, run: &mut Run, order: Order) -> bool {
        if !run.refit_in_dock(order.job) {
            return false;
        }
        match order.job {
            DockJob::Insert { at, .. } => {
                self.ids
                    .insert(at, order.target.expect("a spliced section"));
            }
            DockJob::Move { from, to } => {
                let id = self.ids.remove(from);
                self.ids.insert(to, id);
            }
            _ => {}
        }
        self.orders.push(Order {
            cost: order.job.cost(),
            ..order
        });
        true
    }

    /// Put the ship back as the last refit left her and make every order again in turn,
    /// dropping any that no longer stands.
    fn rebuild(&mut self, run: &mut Run) {
        run.ship = self.built.ship.clone();
        run.fittings = self.built.fittings;
        run.points = self.built.points;
        run.spent = self.built.spent;
        self.ids = (0..self.built.ship.design().modules().len() as SectionId).collect();
        for order in std::mem::take(&mut self.orders) {
            if let Some(job) = self.rebase(run, &order)
                && !unchanged(run, job)
            {
                self.make(run, Order { job, ..order });
            }
        }
    }
}

/// A section refitted to what it already is: a turret to its own barrels, barbette or
/// bore, or a bay exchanged for what already stands on it.
fn unchanged(run: &Run, job: DockJob) -> bool {
    match job {
        DockJob::Barrels { from, to, .. } => from == to,
        DockJob::Barbette { from, to, .. } => from == to,
        DockJob::Calibre { from, to, .. } => from == to,
        DockJob::Replace { at, with } => run
            .ship
            .design()
            .modules()
            .get(at)
            .is_some_and(|m| m.same_kind(&with.build())),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Card, DeckModule, DockModule, ShipBeam, TurretLayout};

    /// A run in the yard at her built length with `points` to spend.
    fn docked(points: u16) -> Run {
        let mut run = Run::default();
        run.clear();
        run.points = points;
        run
    }

    fn gun(run: &Run, n: usize) -> usize {
        run.ship.design().gun_stations()[n].berth.module
    }

    #[test]
    fn orders_are_made_at_once_and_each_can_be_taken_back() {
        let mut run = docked(20);
        let mut plan = Plan::draw(&run);
        let fore = gun(&run, 0);
        let loader = DockJob::Fitting(Card::Loader);
        assert!(plan.order(&mut run, loader, Some(fore)));
        let rebore = DockJob::Calibre {
            at: fore,
            from: 4,
            to: 6,
        };
        assert!(plan.order(&mut run, rebore, None));
        assert_eq!((run.points(), plan.cost()), (16, 4));
        assert_eq!(run.fittings.loader, 1);
        assert_eq!(run.ship.design().modules()[fore].gunhouse.bore, 6);
        // Both are listed under the turret they were ordered on.
        assert!(plan.orders().iter().all(|o| plan.place(o) == Some(fore)));
        // Taking the loader back leaves the bore.
        plan.undo(&mut run, 0);
        assert_eq!((run.points(), run.fittings.loader), (18, 0));
        assert_eq!(run.ship.design().modules()[fore].gunhouse.bore, 6);
        // An order her points cannot cover is turned down, and the plan stands.
        let quad = DockJob::Barrels {
            at: fore,
            from: TurretLayout::Single,
            to: TurretLayout::Quad,
        };
        assert_eq!(quad.cost(), 9);
        let mut run = docked(5);
        let mut plan = Plan::draw(&run);
        assert!(plan.order(&mut run, rebore, None));
        assert!(!plan.order(&mut run, quad, None));
        assert_eq!((run.points(), plan.orders().len()), (3, 1));
    }

    #[test]
    fn a_turret_refitted_twice_is_refitted_once_to_the_second_choice() {
        let mut run = docked(30);
        let mut plan = Plan::draw(&run);
        let at = gun(&run, 0);
        let house = |run: &Run| run.ship.design().modules()[at].gunhouse;
        for to in [TurretLayout::Twin, TurretLayout::Triple] {
            let from = house(&run).barrels;
            assert!(plan.order(&mut run, DockJob::Barrels { at, from, to }, None));
        }
        // One order, priced from a single gun to a triple.
        assert_eq!((plan.orders().len(), plan.cost()), (1, 5));
        assert_eq!(house(&run).barrels, TurretLayout::Triple);
        // Back to a single gun leaves nothing to build.
        let single = DockJob::Barrels {
            at,
            from: TurretLayout::Triple,
            to: TurretLayout::Single,
        };
        assert!(plan.order(&mut run, single, None));
        assert!(plan.is_empty() && run.points() == 30);
        assert_eq!(house(&run).barrels, TurretLayout::Single);

        // Split into wings and merged again is no order at all.
        let split = DockJob::Replace {
            at,
            with: DeckModule::WingBattery,
        };
        let merge = DockJob::Replace {
            at,
            with: DeckModule::CentreBattery,
        };
        assert!(plan.order(&mut run, split, None));
        assert!(plan.order(&mut run, merge, None));
        assert!(plan.is_empty() && run.points() == 30);
        // A turret taken off takes its pending refits with it.
        let twin = DockJob::Barrels {
            at,
            from: TurretLayout::Single,
            to: TurretLayout::Twin,
        };
        assert!(plan.order(&mut run, twin, None));
        let remove = DockJob::Replace {
            at,
            with: DeckModule::Reserve,
        };
        assert!(plan.order(&mut run, remove, None));
        assert_eq!(plan.orders().len(), 1);
        assert_eq!(plan.cost(), remove.cost());
    }

    #[test]
    fn orders_stay_with_their_section_through_splices_and_moves() {
        let mut run = docked(40);
        let mut plan = Plan::draw(&run);
        // A bay goes in ahead of the after turret, and takes a turret of its own.
        let at = run.default_gap();
        let bay = DockJob::Insert {
            at,
            module: DockModule::Reserve,
        };
        assert!(plan.order(&mut run, bay, None));
        assert!(plan.is_new(at) && !plan.is_new(at + 1));
        let turret = DockJob::Replace {
            at,
            with: DeckModule::CentreBattery,
        };
        assert!(plan.order(&mut run, turret, None));
        // The after turret, now one further aft, is rebored where it stands.
        let aft = at + 1;
        assert!(plan.order(
            &mut run,
            DockJob::Calibre {
                at: aft,
                from: 4,
                to: 6
            },
            None
        ));
        // A bay spliced in at the bow moves every section aft, and the orders with them.
        let bow = DockJob::Insert {
            at: 1,
            module: DockModule::Reserve,
        };
        assert!(plan.order(&mut run, bow, None));
        let places: Vec<_> = plan.orders().iter().map(|o| plan.place(o)).collect();
        assert_eq!(places, [Some(at + 1), Some(at + 1), Some(aft + 1), Some(1)]);
        // Taking back the first bay takes its turret with it and leaves the rest.
        plan.undo(&mut run, 0);
        assert_eq!(plan.orders().len(), 2);
        let modules = run.ship.design().modules();
        assert_eq!(modules[at + 1].gunhouse.bore, 6);
        assert!(plan.is_new(1));
        assert_eq!(run.points(), 40 - 3 - 2);

        // A section moved and moved back is not moved at all.
        let mut run = docked(10);
        let mut plan = Plan::draw(&run);
        let fore = gun(&run, 0);
        assert!(plan.order(
            &mut run,
            DockJob::Move {
                from: fore,
                to: fore + 1
            },
            None
        ));
        assert!(plan.is_refitted(fore + 1));
        assert!(plan.order(
            &mut run,
            DockJob::Move {
                from: fore + 1,
                to: fore
            },
            None
        ));
        assert!(plan.is_empty());
    }

    #[test]
    fn work_that_waits_on_an_order_taken_back_goes_with_it() {
        let mut run = docked(40);
        let mut plan = Plan::draw(&run);
        for _ in 0..2 {
            let at = run.default_gap();
            let bay = DockJob::Insert {
                at,
                module: DockModule::Reserve,
            };
            assert!(plan.order(&mut run, bay, None));
        }
        assert!(plan.order(&mut run, DockJob::WidenBeam, None));
        let armor = DockJob::Fitting(Card::Armor);
        for _ in 0..9 {
            assert!(plan.order(&mut run, armor, None));
        }
        assert_eq!(run.fittings.armor, 9);
        assert_eq!(plan.place(&plan.orders()[2]), None, "hull work");
        // One bay is length enough for a cruiser's beam.
        plan.undo(&mut run, 1);
        assert_eq!(run.beam_class(), ShipBeam::HeavyCruiser);
        // Without either she is too short to widen, so the beam goes, and the ninth rank
        // of armor that waited on it.
        plan.undo(&mut run, 0);
        assert_eq!(run.beam_class(), ShipBeam::Escort);
        assert_eq!(run.fittings.armor, 8);
        assert_eq!(plan.orders().len(), 8);
        assert_eq!(run.points(), 40 - 8);
        plan.clear(&mut run);
        assert_eq!(run.points(), 40);
        assert_eq!(run.ship.design().modules().len(), 6);
    }
}
