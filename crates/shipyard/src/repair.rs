//! Mending a ship: the crew's steady repairs, the repair party, stores and the dock.
//!
//! Damage lingers. The crew restores each module only up to its ceiling, which every
//! hit lowers, and cannot restart a component that was knocked out or mend a wrecked
//! module. Those wait for the repair party. Stores and the dock raise ceilings. Side
//! plating mends at the same pace as the module beneath it, and the dock replates.
//!
//! A battered module leaks, the more the more it is battered, and the pumps clear
//! water from every module at a steady rate, more where the party is sent. A torpedo's
//! hole lets the sea pour in faster than that for a few seconds, until it is shored. A
//! module that fills floods, and comes back into service once pumped down to half full.
//!
//! Ships running from a battery draw on its charge while the party works.
use crate::design::{Berth, Component};
use crate::ship::{Compartment, Ship, State};

/// Share of a module or working component the crew restores each second.
const AUTO: f32 = 0.002;
/// What the repair party adds on the module it is sent to.
const PARTY_HULL: f32 = 0.012;
/// What the repair party adds on the component it is sent to.
const PARTY_GEAR: f32 = 0.03;
/// The dock pumps out a wrecked module it brings back to this share of its structure.
const REFLOAT: f32 = 0.25;
/// Share of a module the crew's pumps clear each second, at the stock rate.
pub const PUMP: f32 = 0.05;
/// What the repair party's pumps add on the module it is sent to.
pub const PARTY_PUMP: f32 = 0.1;
/// A flooded module is back in service once pumped down to this share of water.
pub const DRAINED: f32 = 0.5;
/// The party goes to a module filling this far before it floods, and to one this full
/// whether it is still filling or not.
const FILLING: f32 = 0.25;
const BAILING: f32 = 0.5;
/// A knocked-out component is back in service at this share of its strength.
const RESTORE: f32 = 0.3;

/// Where the repair party is working.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    Module(usize),
    Fitting(Berth),
}

/// Something repairs brought back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mend {
    Restored(Berth),
    Refloated(usize),
}

/// A wrecked module is back in service once pumped down to half full, or on a ship
/// that does not leak, once patched to a quarter of its structure. A badly scarred one
/// refloats at its ceiling, which is all it can ever regain at sea.
fn refloat(hold: &mut Compartment, leaks: bool) -> bool {
    let afloat = hold.wrecked
        && if leaks {
            hold.structure > 0. && hold.water <= DRAINED
        } else {
            hold.ceiling > 0. && hold.structure >= (REFLOAT * hold.max).min(hold.ceiling)
        };
    if afloat {
        hold.wrecked = false;
        // A ship that does not leak is dry the moment she is afloat again.
        if !leaks {
            hold.water = 0.;
        }
    }
    afloat
}

impl Ship {
    pub fn party(&self) -> Option<Task> {
        self.party
    }

    /// Whether the repair party has anything it can do there.
    fn workable(&self, task: Task) -> bool {
        if !self.afloat() || self.design().repair <= 0. {
            return false;
        }
        match task {
            Task::Module(module) => self
                .holds
                .get(module)
                .is_some_and(|hold| hold.structure < hold.ceiling || hold.water > 0.),
            Task::Fitting(berth) => self.holds.get(berth.module).is_some_and(|hold| {
                !hold.wrecked
                    && hold
                        .gear
                        .get(berth.index)
                        .is_some_and(|gear| gear.health < gear.max)
            }),
        }
    }

    /// Send the repair party to a task, or stand it down with `None`. An order it
    /// cannot carry out is refused and the party stays where it was.
    pub fn assign(&mut self, task: Option<Task>) -> bool {
        let accepted = task.is_none_or(|task| self.workable(task));
        if accepted {
            self.party = task;
        }
        accepted
    }

    /// The most urgent job for the repair party: keep the ship afloat, then get her
    /// moving, then get her fighting, then everything else.
    pub fn triage(&self) -> Option<Task> {
        let vitals = self.design().vitals();
        let berths: Vec<(Berth, Component)> = self
            .design()
            .fittings()
            .into_iter()
            .map(|fitting| (fitting.berth, fitting.component))
            .collect();
        let flooded = |inside: bool| {
            let vitals = vitals.clone();
            (0..self.holds.len())
                .filter(move |i| self.holds[*i].wrecked && vitals.contains(i) == inside)
                .map(Task::Module)
        };
        // A module filling faster than the crew can pump it is the next to go, and so is
        // one still deep in the water a torpedo let in.
        let filling = (0..self.holds.len())
            .filter(|&i| {
                let hold = &self.holds[i];
                let leaking = hold.inflow() > PUMP * self.design().pumps() * self.design().repair;
                !hold.wrecked
                    && (hold.breached()
                        || hold.water >= FILLING && (hold.water >= BAILING || leaking))
            })
            .max_by(|a, b| self.holds[*a].water.total_cmp(&self.holds[*b].water))
            .map(Task::Module);
        let knocked = |engines: bool| {
            berths
                .iter()
                .filter(move |(berth, component)| {
                    (*component == Component::Engine) == engines && self.state(*berth) == State::Out
                })
                .map(|(berth, _)| Task::Fitting(*berth))
        };
        let weakest_fitting = berths
            .iter()
            .map(|(berth, _)| *berth)
            .filter(|berth| self.workable(Task::Fitting(*berth)))
            .min_by(|a, b| self.health(*a).total_cmp(&self.health(*b)))
            .map(Task::Fitting);
        let weakest_module = (0..self.holds.len())
            .filter(|module| self.workable(Task::Module(*module)))
            .min_by(|a, b| {
                let filled = |i: &usize| self.holds[*i].structure / self.holds[*i].ceiling;
                filled(a).total_cmp(&filled(b))
            })
            .map(Task::Module);
        flooded(true)
            .chain(knocked(true))
            .chain(filling)
            .chain(knocked(false))
            .chain(flooded(false))
            .chain(weakest_fitting)
            .chain(weakest_module)
            .find(|task| self.workable(*task))
    }

    /// Advance the ship's power and repairs by `dt` seconds. The party stands down
    /// when its task is done or can no longer be worked.
    pub fn tend(&mut self, dt: f32) -> Vec<Mend> {
        let mut mends = Vec::new();
        if !self.afloat() {
            return mends;
        }
        self.draw_power(dt);
        let pace = self.design().repair * dt;
        if pace <= 0. {
            return mends;
        }
        let party = self.party;
        let leaks = self.design().leaks;
        let pumps = self.design().pumps() * self.design().repair;
        let mut filled = false;
        for (module, hold) in self.holds.iter_mut().enumerate() {
            let crew = if hold.wrecked { 0. } else { AUTO };
            let here = party == Some(Task::Module(module));
            let sent = if here { PARTY_HULL } else { 0. };
            hold.structure = (hold.structure + (crew + sent) * pace * hold.max).min(hold.ceiling);
            hold.mend_plates((crew + sent) * pace);
            // The sea comes in through her damage and the pumps send it back out.
            if leaks {
                let pumping = (PUMP + if here { PARTY_PUMP } else { 0. }) * pumps;
                let poured = hold.pour(dt);
                hold.water = (hold.water + poured + (hold.inflow() - pumping) * dt).clamp(0., 1.);
                if !hold.wrecked && hold.water >= 1. {
                    hold.wrecked = true;
                    filled = true;
                }
            }
            if refloat(hold, leaks) {
                mends.push(Mend::Refloated(module));
            }
            if hold.wrecked {
                hold.inrush = 0.;
                continue;
            }
            for (index, gear) in hold.gear.iter_mut().enumerate() {
                let berth = Berth { module, index };
                let crew = if gear.out { 0. } else { AUTO };
                let sent = if party == Some(Task::Fitting(berth)) {
                    PARTY_GEAR
                } else {
                    0.
                };
                gear.health = (gear.health + (crew + sent) * pace * gear.max).min(gear.max);
                if gear.out && gear.health >= RESTORE * gear.max {
                    gear.out = false;
                    mends.push(Mend::Restored(berth));
                }
            }
        }
        if filled {
            self.settle(&mut Vec::new());
        }
        if party.is_some_and(|task| !self.workable(task)) {
            self.party = None;
        }
        mends
    }

    /// Spend stores worth `points` of hull on the weakest modules. Unlike the crew's
    /// own repairs this is new material, so it lifts the ceiling with the structure.
    pub fn patch(&mut self, points: f32) -> Vec<Mend> {
        let mut mends = Vec::new();
        let leaks = self.design().leaks;
        let mut left = points;
        while self.afloat() && left > 1e-4 {
            let Some((module, hold)) = self
                .holds
                .iter_mut()
                .enumerate()
                .filter(|(_, hold)| hold.structure < hold.max)
                .min_by(|(_, a), (_, b)| a.soundness().total_cmp(&b.soundness()))
            else {
                break;
            };
            let added = left.min(hold.max - hold.structure);
            hold.structure += added;
            hold.ceiling = hold.ceiling.max(hold.structure);
            left -= added;
            if refloat(hold, leaks) {
                mends.push(Mend::Refloated(module));
            }
        }
        mends
    }

    /// Dock work between legs, worth `points` of hull. New plate goes to the weakest
    /// modules first, as stores do, and every module left afloat is replated and has
    /// its components returned to full strength. A badly damaged ship sails with what
    /// the dock could not reach.
    pub fn dock(&mut self, points: f32) -> Vec<Mend> {
        let mut mends = self.patch(points);
        let leaks = self.design().leaks;
        if !self.afloat() {
            return mends;
        }
        for (module, hold) in self.holds.iter_mut().enumerate() {
            // In the dock she is pumped dry wherever the yard has made her good enough.
            hold.inrush = 0.;
            if hold.structure >= (REFLOAT * hold.max).min(hold.ceiling) && hold.ceiling > 0. {
                hold.water = 0.;
                if refloat(hold, leaks) {
                    mends.push(Mend::Refloated(module));
                }
            }
            if hold.wrecked {
                continue;
            }
            hold.mend_plates(1.);
            for (index, gear) in hold.gear.iter_mut().enumerate() {
                gear.health = gear.max;
                if std::mem::take(&mut gear.out) {
                    mends.push(Mend::Restored(Berth { module, index }));
                }
            }
        }
        self.recharge();
        if self.party.is_some_and(|task| !self.workable(task)) {
            self.party = None;
        }
        mends
    }
}
