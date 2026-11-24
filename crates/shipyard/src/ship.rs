//! One hull built to a design, and where each hit goes.
use crate::design::{Berth, Component, Design, Fault, Module, Slot};
use crate::repair::Task;
use std::sync::Arc;

/// Share of each point of hull damage that the crew cannot make good at sea.
const SCAR: f32 = 0.35;
/// Share of a hit on a wrecked module that carries into the modules beside it.
const SPILL: f32 = 0.5;
/// A detonating magazine strikes each neighbour with this share of its module's structure.
const BLAST: f32 = 0.5;
/// Speed lost when the whole hull is flooded. A part-flooded hull loses its share.
const FLOOD_DRAG: f32 = 0.5;
/// Seconds of reserve power regained for each second an engine runs.
const RECHARGE: f32 = 0.5;

/// How well a fitted component is working.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Working,
    /// At or below half strength: it gives half its output.
    Damaged,
    /// Knocked out, or standing on a wrecked module.
    Out,
}
impl State {
    pub fn output(self) -> f32 {
        match self {
            Self::Working => 1.,
            Self::Damaged => 0.5,
            Self::Out => 0.,
        }
    }
}

/// Something a hit cost the ship, in the order it happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Loss {
    Knocked(Berth),
    Wrecked(usize),
    Detonated(usize),
    Foundered,
}

pub(crate) struct Gear {
    pub health: f32,
    pub max: f32,
    /// Stays set until repairs bring the component back into service.
    pub out: bool,
}

/// The condition of one module.
pub struct Compartment {
    pub(crate) structure: f32,
    pub(crate) ceiling: f32,
    pub(crate) max: f32,
    pub(crate) wrecked: bool,
    pub(crate) gear: Vec<Gear>,
}
impl Compartment {
    fn new(design: &Design, index: usize) -> Self {
        let max = design.module_structure(index);
        let gear = design.modules()[index]
            .fit
            .iter()
            .map(|component| {
                let max = component.health(max, design);
                Gear {
                    health: max,
                    max,
                    out: false,
                }
            })
            .collect();
        Self {
            structure: max,
            ceiling: max,
            max,
            wrecked: false,
            gear,
        }
    }

    /// Hull points left.
    pub fn structure(&self) -> f32 {
        self.structure
    }
    /// The most the crew can restore at sea. Stores and the dock raise it.
    pub fn ceiling(&self) -> f32 {
        self.ceiling
    }
    pub fn capacity(&self) -> f32 {
        self.max
    }
    /// Flooded and out of action, with everything fitted to it.
    pub fn wrecked(&self) -> bool {
        self.wrecked
    }
    /// Structure left, from 0 to 1.
    pub fn soundness(&self) -> f32 {
        self.structure / self.max
    }
}

pub struct Ship {
    design: Arc<Design>,
    pub(crate) holds: Vec<Compartment>,
    pub(crate) party: Option<Task>,
    pub(crate) foundered: bool,
    /// Seconds of reserve power left.
    charge: f32,
}

impl Ship {
    /// Fractions of normal propulsion and gun movement on auxiliary machinery.
    pub const RESERVE_SPEED: f32 = 0.15;
    pub const RESERVE_GUN_RATE: f32 = 0.30;

    /// A sound ship fresh from the dock.
    pub fn new(design: impl Into<Arc<Design>>) -> Self {
        let design = design.into();
        let holds = (0..design.modules().len())
            .map(|i| Compartment::new(&design, i))
            .collect();
        Self {
            charge: design.reserve,
            design,
            holds,
            party: None,
            foundered: false,
        }
    }

    /// Splice in a sound compartment, preserving damage, equipment and repair assignments.
    pub fn insert_module(&mut self, at: usize, module: Module) -> Result<(), Fault> {
        Arc::make_mut(&mut self.design).insert(at, module)?;
        self.holds.insert(at, Compartment::new(&self.design, at));
        if let Some(task) = &mut self.party {
            let index = match task {
                Task::Module(index) => index,
                Task::Fitting(berth) => &mut berth.module,
            };
            if *index >= at {
                *index += 1;
            }
        }
        Ok(())
    }

    pub fn design(&self) -> &Arc<Design> {
        &self.design
    }

    pub fn widen(&mut self, beam_metres: f32) {
        Arc::make_mut(&mut self.design).widen(beam_metres);
    }

    pub fn move_module(&mut self, from: usize, to: usize) -> Result<(), Fault> {
        Arc::make_mut(&mut self.design).move_module(from, to)?;
        let hold = self.holds.remove(from);
        self.holds.insert(to, hold);
        if let Some(task) = &mut self.party {
            let index = match task {
                Task::Module(i) => i,
                Task::Fitting(b) => &mut b.module,
            };
            *index = if *index == from {
                to
            } else if from < *index && *index <= to {
                *index - 1
            } else if to <= *index && *index < from {
                *index + 1
            } else {
                *index
            };
        }
        Ok(())
    }

    pub fn replace_module(&mut self, at: usize, module: Module) -> Result<(), Fault> {
        Arc::make_mut(&mut self.design).replace_module(at, module)?;
        self.holds[at] = Compartment::new(&self.design, at);
        if matches!(self.party, Some(Task::Module(i)) if i == at)
            || matches!(self.party, Some(Task::Fitting(b)) if b.module == at)
        {
            self.party = None;
        }
        Ok(())
    }

    /// One compartment per module, bow to stern.
    pub fn compartments(&self) -> &[Compartment] {
        &self.holds
    }

    pub fn afloat(&self) -> bool {
        !self.foundered
    }

    /// Hull points left across the ship.
    pub fn hull(&self) -> f32 {
        self.holds.iter().map(|h| h.structure).sum()
    }

    /// Hull points left as a share of the design's, from 0 to 1.
    pub fn integrity(&self) -> f32 {
        self.hull() / self.design.structure()
    }

    pub fn state(&self, berth: Berth) -> State {
        let Some(hold) = self.holds.get(berth.module) else {
            return State::Out;
        };
        match hold.gear.get(berth.index) {
            Some(gear) if !hold.wrecked && !gear.out => {
                if gear.health > 0.5 * gear.max {
                    State::Working
                } else {
                    State::Damaged
                }
            }
            _ => State::Out,
        }
    }

    /// Strength left in one component, from 0 to 1.
    pub fn health(&self, berth: Berth) -> f32 {
        self.holds
            .get(berth.module)
            .and_then(|hold| hold.gear.get(berth.index))
            .map_or(0., |gear| gear.health / gear.max)
    }

    /// Every berth carrying one kind of component, with its state.
    pub fn fitted(&self, component: Component) -> impl Iterator<Item = (Berth, State)> + '_ {
        self.design
            .modules()
            .iter()
            .enumerate()
            .flat_map(move |(module, m)| {
                m.fit
                    .iter()
                    .enumerate()
                    .filter(move |(_, c)| **c == component)
                    .map(move |(index, _)| Berth { module, index })
            })
            .map(|berth| (berth, self.state(berth)))
    }

    fn wrecked_share(&self) -> f32 {
        let wrecked: f32 = (0..self.holds.len())
            .filter(|&i| self.holds[i].wrecked)
            .map(|i| self.design.module_length(i))
            .sum();
        wrecked / self.design.length()
    }

    /// Share of normal speed available at the telegraph. Flooding drags on the main drive.
    pub fn propulsion(&self) -> f32 {
        if self.foundered {
            return 0.;
        }
        let running = self.running();
        if running == 0. && self.design.auxiliary_drive {
            return Self::RESERVE_SPEED;
        }
        running / self.design.count(Component::Engine) as f32
            * (1. - FLOOD_DRAG * self.wrecked_share())
    }

    /// Output of the engines still turning, counted in engines.
    fn running(&self) -> f32 {
        self.fitted(Component::Engine)
            .map(|(_, state)| state.output())
            .sum()
    }

    /// Auxiliary drive takes over immediately when the last main engine goes out.
    pub fn on_reserve_power(&self) -> bool {
        self.afloat() && self.design.auxiliary_drive && self.running() == 0.
    }

    /// Speed of powered traverse and elevation on a serviceable mount.
    pub fn gun_train_rate(&self) -> f32 {
        if self.on_reserve_power() {
            Self::RESERVE_GUN_RATE
        } else if self.powered() {
            1.
        } else {
            0.
        }
    }

    /// Whether a running engine, auxiliary machinery or battery can power the weapons.
    pub fn powered(&self) -> bool {
        self.afloat() && (self.design.auxiliary_drive || self.charge > 0. || self.running() > 0.)
    }

    /// Battery charge, from 0 to 1. Auxiliary machinery can supply power continuously.
    pub fn reserve(&self) -> f32 {
        if self.design.reserve > 0. {
            self.charge / self.design.reserve
        } else {
            0.
        }
    }

    /// Whether a component can be used right now: in service, and powered.
    pub fn ready(&self, berth: Berth) -> bool {
        self.powered() && self.state(berth) != State::Out
    }

    /// Drain the reserve while the engines are stopped and refill it while they run.
    pub(crate) fn draw_power(&mut self, dt: f32) {
        self.charge = if self.design.auxiliary_drive || self.running() > 0. {
            (self.charge + RECHARGE * dt).min(self.design.reserve)
        } else {
            (self.charge - dt).max(0.)
        };
    }

    pub(crate) fn recharge(&mut self) {
        self.charge = self.design.reserve;
    }

    /// Multiple of the normal reload time. A ship built without a director has
    /// nothing to lose. One that carries them loads slowly once they are gone.
    pub fn reload_factor(&self) -> f32 {
        let best = self
            .fitted(Component::Director)
            .map(|(_, state)| state.output())
            .reduce(f32::max);
        match best {
            None => 1.,
            Some(output) if output >= 1. => 1.,
            Some(output) if output > 0. => 1.3,
            Some(_) => 1.65,
        }
    }

    /// How many vital modules are wrecked, and how few of them would sink her.
    pub fn flooding(&self) -> (usize, usize) {
        let vitals = self.design.vitals();
        let wrecked = vitals.clone().filter(|&m| self.holds[m].wrecked).count();
        let mut lengths: Vec<f32> = vitals.map(|m| self.design.module_length(m)).collect();
        lengths.sort_by(|a, b| b.total_cmp(a));
        let whole: f32 = lengths.iter().sum();
        let mut gone = 0.;
        let fatal = lengths
            .iter()
            .position(|length| {
                gone += length;
                gone >= self.design.sinks_at * whole - 1e-3
            })
            .map_or(lengths.len(), |last| last + 1);
        (wrecked, fatal)
    }

    /// Whether losing one more vital module could sink her.
    pub fn in_peril(&self) -> bool {
        let (mut wrecked, mut whole, mut largest) = (0., 0., 0_f32);
        for module in self.design.vitals() {
            let length = self.design.module_length(module);
            whole += length;
            if self.holds[module].wrecked {
                wrecked += length;
            } else {
                largest = largest.max(length);
            }
        }
        !self.foundered && wrecked + largest >= self.design.sinks_at * whole - 1e-3
    }

    /// A penetrating hit on the hull, `aft` metres behind the bow tip.
    ///
    /// The module there takes the damage, and so does everything in its bays. A
    /// module that is already wrecked passes half of it to the modules on each side.
    pub fn strike_hull(&mut self, aft: f32, damage: f32) -> Vec<Loss> {
        let mut losses = Vec::new();
        if self.foundered || damage <= 0. {
            return losses;
        }
        let module = self.design.module_at(aft);
        if self.holds[module].wrecked {
            let sound = |i: &usize| !self.holds[*i].wrecked;
            let beside: Vec<usize> = (0..module)
                .rev()
                .find(sound)
                .into_iter()
                .chain((module + 1..self.holds.len()).find(sound))
                .collect();
            for &next in &beside {
                self.hurt(next, damage * SPILL / beside.len() as f32, &mut losses);
            }
        } else {
            self.hurt(module, damage, &mut losses);
        }
        self.settle(&mut losses);
        losses
    }

    /// A hit on one fitted component. The hull behind it is not harmed.
    pub fn strike_fitting(&mut self, berth: Berth, damage: f32) -> Vec<Loss> {
        let mut losses = Vec::new();
        if self.foundered || damage <= 0. || self.state(berth) == State::Out {
            return losses;
        }
        let gear = &mut self.holds[berth.module].gear[berth.index];
        gear.health = (gear.health - damage).max(0.);
        if gear.health == 0. {
            gear.out = true;
            losses.push(Loss::Knocked(berth));
            if self.design.component(berth) == Some(Component::Magazine) {
                self.detonate(berth.module, &mut losses);
            }
        }
        self.settle(&mut losses);
        losses
    }

    fn hurt(&mut self, module: usize, damage: f32, losses: &mut Vec<Loss>) {
        let hold = &mut self.holds[module];
        if hold.wrecked {
            return;
        }
        let dealt = damage.min(hold.structure);
        hold.structure -= dealt;
        hold.ceiling -= dealt * SCAR;
        let mut blown = false;
        let fit = &self.design.modules()[module].fit;
        for (index, (gear, component)) in hold.gear.iter_mut().zip(fit).enumerate() {
            if component.slot() != Slot::Bay || gear.out {
                continue;
            }
            gear.health = (gear.health - damage).max(0.);
            if gear.health == 0. {
                gear.out = true;
                losses.push(Loss::Knocked(Berth { module, index }));
                blown |= *component == Component::Magazine;
            }
        }
        if blown {
            self.detonate(module, losses);
        } else if hold.structure <= 0. {
            hold.wrecked = true;
            losses.push(Loss::Wrecked(module));
        }
    }

    fn detonate(&mut self, module: usize, losses: &mut Vec<Loss>) {
        let hold = &mut self.holds[module];
        hold.ceiling -= hold.structure * SCAR;
        hold.structure = 0.;
        hold.wrecked = true;
        losses.push(Loss::Detonated(module));
        losses.push(Loss::Wrecked(module));
        let blast = hold.max * BLAST;
        for next in [module.checked_sub(1), Some(module + 1)]
            .into_iter()
            .flatten()
        {
            if next < self.holds.len() {
                self.hurt(next, blast, losses);
            }
        }
    }

    /// The ship goes down once enough of its vital length is wrecked. On an armoured
    /// ship only the citadel counts, so wrecked ends cannot sink it.
    fn settle(&mut self, losses: &mut Vec<Loss>) {
        if self.foundered {
            return;
        }
        let (mut wrecked, mut whole) = (0., 0.);
        for module in self.design.vitals() {
            let length = self.design.module_length(module);
            whole += length;
            if self.holds[module].wrecked {
                wrecked += length;
            }
        }
        // Module lengths are scaled, so allow for rounding in the comparison.
        if wrecked >= self.design.sinks_at * whole - 1e-3 {
            self.foundered = true;
            self.party = None;
            losses.push(Loss::Foundered);
        }
    }
}
