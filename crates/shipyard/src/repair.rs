//! Mending a ship: the crew's steady repairs, the repair party, stores and the dock.
//!
//! Damage lingers. The crew restores each module only up to its ceiling, which every
//! hit lowers, and cannot restart a component that was knocked out or pump out a
//! wrecked module. Those wait for the repair party. Stores and the dock raise ceilings.
//! Side plating mends at the same pace as the module beneath it, and the dock replates.
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
/// A wrecked module is afloat again at this share of its structure.
const REFLOAT: f32 = 0.25;
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

/// A wrecked module with enough structure back is afloat. A badly scarred one
/// refloats at its ceiling, which is all it can ever regain at sea.
fn refloat(hold: &mut Compartment) -> bool {
    let afloat = hold.wrecked
        && hold.ceiling > 0.
        && hold.structure >= (REFLOAT * hold.max).min(hold.ceiling);
    if afloat {
        hold.wrecked = false;
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
                .is_some_and(|hold| hold.structure < hold.ceiling),
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
        for (module, hold) in self.holds.iter_mut().enumerate() {
            let crew = if hold.wrecked { 0. } else { AUTO };
            let sent = if party == Some(Task::Module(module)) {
                PARTY_HULL
            } else {
                0.
            };
            hold.structure = (hold.structure + (crew + sent) * pace * hold.max).min(hold.ceiling);
            hold.mend_plates((crew + sent) * pace);
            if refloat(hold) {
                mends.push(Mend::Refloated(module));
            }
            if hold.wrecked {
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
        if party.is_some_and(|task| !self.workable(task)) {
            self.party = None;
        }
        mends
    }

    /// Spend stores worth `points` of hull on the weakest modules. Unlike the crew's
    /// own repairs this is new material, so it lifts the ceiling with the structure.
    pub fn patch(&mut self, points: f32) -> Vec<Mend> {
        let mut mends = Vec::new();
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
            if refloat(hold) {
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
        if !self.afloat() {
            return mends;
        }
        for (module, hold) in self.holds.iter_mut().enumerate() {
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
