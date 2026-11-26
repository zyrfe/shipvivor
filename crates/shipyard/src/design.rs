//! What a ship is built from, and the figures that follow from the build.
use shipvivor_artillery::{ArmorGrade, TurretLayout};
use std::ops::RangeInclusive;

/// Top speed of the reference cruiser in metres per second.
const DESIGN_SPEED: f32 = 13.6;
/// Power of one engine. One engine drives the reference cruiser at the design speed.
const ENGINE_POWER: f32 = 8.;
/// A hull of `TURN_LENGTH` metres turns at `TURN_RATE` radians per second.
const TURN_RATE: f32 = 0.38;
const TURN_LENGTH: f32 = 30.;

/// The shape of one hull module. Figures are for a hull at scale 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Frame {
    /// A complete small hull in one piece.
    Boat,
    Bow,
    Straight,
    /// A straight section with a gallery for side mounts on each beam.
    Sponsons,
    /// Two main mounts abreast, each training on its own beam.
    BatteryGallery,
    /// A broader section with room for two of everything.
    Wide,
    /// Two full-size main turrets abreast, each training over its own side and round to
    /// the bow and the stern. Only a capital beam has the room for them.
    DoubleBattery,
    Stern,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameSpec {
    pub length: f32,
    pub beam: f32,
    /// Deck positions on the centreline.
    pub centre: usize,
    /// Deck positions on the beams, filled port first.
    pub side: usize,
    /// Positions below deck.
    pub bay: usize,
    /// Damage the module absorbs before it is wrecked, at toughness 1.
    pub structure: f32,
    pub mass: f32,
}

impl Frame {
    /// The frames that can stand between a bow and a stern.
    pub const MIDSHIPS: [Self; 5] = [
        Self::Straight,
        Self::Sponsons,
        Self::Wide,
        Self::BatteryGallery,
        Self::DoubleBattery,
    ];

    /// Main turrets on this frame stand abreast, a port turret and a starboard one.
    pub fn abreast(self) -> bool {
        matches!(self, Self::BatteryGallery | Self::DoubleBattery)
    }

    pub fn spec(self) -> FrameSpec {
        let spec = |length, beam, slots: [usize; 3], structure, mass| FrameSpec {
            length,
            beam,
            centre: slots[0],
            side: slots[1],
            bay: slots[2],
            structure,
            mass,
        };
        match self {
            Self::Boat => spec(30., 10., [1, 0, 1], 100., 6.5),
            Self::Bow => spec(5., 4.4, [1, 0, 0], 14., 0.7),
            Self::Straight => spec(5., 5.5, [1, 0, 1], 18., 1.),
            Self::Sponsons => spec(5., 5.5, [1, 2, 1], 18., 1.1),
            Self::BatteryGallery => spec(5., 5.5, [2, 0, 1], 18., 1.2),
            Self::DoubleBattery => spec(5., 5.5, [2, 0, 1], 20., 1.35),
            Self::Wide => spec(5., 7., [2, 0, 2], 24., 1.5),
            Self::Stern => spec(5., 5., [1, 0, 0], 14., 0.7),
        }
    }

    pub fn slots(self, slot: Slot) -> usize {
        let spec = self.spec();
        match slot {
            Slot::Centre => spec.centre,
            Slot::Side => spec.side,
            Slot::Bay => spec.bay,
        }
    }
}

/// Where on a module a component is carried.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Slot {
    Centre,
    Side,
    Bay,
}
impl Slot {
    pub const ALL: [Self; 3] = [Self::Centre, Self::Side, Self::Bay];
}

/// Something fitted to a module. New kinds of equipment are added here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Component {
    MainGun,
    CloseGun,
    TorpedoRack,
    /// Fire control. A ship that carries one reloads slowly without it.
    Director,
    Engine,
    /// Detonates when destroyed, wrecking its module and blasting those beside it.
    Magazine,
    /// A trainable catapult and crane with a floatplane that hunts on its own.
    Catapult,
    /// A trainable box launcher that fires guided missiles at the nearest enemy.
    MissileRack,
}

/// Missiles a rack fires together, the damage each does, how far they reach and the
/// seconds before the rack is loaded again.
pub const MISSILE_SALVO: usize = 2;
pub const MISSILE_DAMAGE: u16 = 8;
pub const MISSILE_RANGE: f32 = 1000.;
pub const MISSILE_RELOAD: f32 = 20.;
/// How far from the ship the floatplane hunts, the damage of each bomb it drops, and
/// the seconds between passes while it rearms.
pub const FLOATPLANE_REACH: f32 = 700.;
pub const BOMB_DAMAGE: u16 = 5;
pub const SORTIE_SECONDS: f32 = 16.;

/// How the ship's strike craft are fitted out: the launchers on each missile rack and
/// the missiles in each salvo, the catapults and planes on the plane section and the
/// bombs each plane carries on a sortie.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StrikeCraft {
    pub launchers: usize,
    pub salvo: usize,
    pub planes: usize,
    pub bombs: usize,
}
impl StrikeCraft {
    /// One launcher of two missiles on a rack, and one plane with one bomb.
    pub const STOCK: Self = Self {
        launchers: 1,
        salvo: MISSILE_SALVO,
        planes: 1,
        bombs: 1,
    };
}
impl Default for StrikeCraft {
    fn default() -> Self {
        Self::STOCK
    }
}

impl Component {
    pub fn slot(self) -> Slot {
        match self {
            Self::MainGun | Self::Director | Self::Catapult | Self::MissileRack => Slot::Centre,
            Self::CloseGun | Self::TorpedoRack => Slot::Side,
            Self::Engine | Self::Magazine => Slot::Bay,
        }
    }

    /// Vital components are what a citadel is drawn around.
    pub fn vital(self) -> bool {
        matches!(self, Self::Engine | Self::Magazine)
    }

    pub fn weapon(self) -> bool {
        matches!(
            self,
            Self::MainGun | Self::CloseGun | Self::TorpedoRack | Self::MissileRack
        )
    }

    pub fn mass(self) -> f32 {
        match self {
            Self::MainGun => 0.4,
            Self::CloseGun => 0.05,
            Self::TorpedoRack => 0.125,
            Self::Director => 0.15,
            Self::Engine => 1.1,
            Self::Magazine => 0.3,
            Self::Catapult => 0.2,
            Self::MissileRack => 0.3,
        }
    }

    /// Damage the component absorbs before it is knocked out, on a module of this
    /// structure. Deck fittings are struck directly and have their own strength. Bay
    /// fittings share every hit on their module, so their strength is a share of its
    /// structure.
    pub fn health(self, structure: f32, design: &Design) -> f32 {
        let strength = design.toughness * design.fittings;
        match self {
            Self::MainGun => 12. * strength,
            Self::CloseGun => 4. * strength,
            Self::TorpedoRack => 5. * strength,
            Self::Director => 8. * strength,
            Self::Engine => design.engine_strength * structure,
            Self::Magazine => 0.6 * structure,
            Self::Catapult => 6. * strength,
            Self::MissileRack => 7. * strength,
        }
    }
}

/// The most a turret can be raised: a third turret firing over a raised second.
pub const MOST_TIER: u8 = 2;

/// How the main turrets on one module are built. A wing pair is built alike.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Gunhouse {
    /// Barrels in each turret.
    pub barrels: TurretLayout,
    /// Inches of bore.
    pub bore: u16,
    /// How many barbette steps the turrets stand above the deck. A turret fires over a
    /// centreline turret that stands lower than itself.
    pub tier: u8,
}
impl Default for Gunhouse {
    /// A single four-inch gun on the deck.
    fn default() -> Self {
        Self {
            barrels: TurretLayout::Single,
            bore: 4,
            tier: 0,
        }
    }
}

/// One hull module and what is fitted to it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Module {
    pub frame: Frame,
    pub fit: Vec<Component>,
    /// How its main turrets are built, if it carries any.
    pub gunhouse: Gunhouse,
}
impl Module {
    pub fn new(frame: Frame, fit: impl IntoIterator<Item = Component>) -> Self {
        Self {
            frame,
            fit: fit.into_iter().collect(),
            gunhouse: Gunhouse::default(),
        }
    }
    /// The same frame carrying the same components, however its turrets are fitted.
    pub fn same_kind(&self, other: &Self) -> bool {
        self.frame == other.frame && self.fit == other.fit
    }
}

/// How well a hull is protected: her side plating, and the strength of what she carries.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Protection {
    /// Strength of the side plating as a multiple of stock.
    pub plating: f32,
    /// Inches of side plate. A round no wider than what is left of it on a panel is
    /// turned aside: a sound panel stops rounds up to its full thickness, and one worn
    /// to half stops rounds up to half of it.
    pub thickness: f32,
    /// Engine durability as a share of the surrounding compartment's structure.
    pub engine: f32,
    /// Strength of deck fittings as a multiple of the hull's toughness.
    pub fittings: f32,
}

/// The address of one fitted component.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Berth {
    pub module: usize,
    /// Position in the module's `fit`.
    pub index: usize,
}

/// A fitted component and where it stands on the hull.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fitting {
    pub berth: Berth,
    pub component: Component,
    /// Metres aft of the bow tip.
    pub aft: f32,
    /// Metres to starboard of the centreline. Negative is to port.
    pub abeam: f32,
}

/// Why a design cannot be built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// A hull is one boat, or a bow and a stern with only midship frames between.
    Ends,
    /// More components for one kind of slot than the module's frame has.
    Crowded { module: usize, slot: Slot },
    /// Nothing drives the ship.
    NoEngine,
    /// No such module or berth.
    Missing,
}

/// The figures a dock shows for a design.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sheet {
    pub length: f32,
    pub beam: f32,
    pub mass: f32,
    pub structure: f32,
    /// Metres per second.
    pub top_speed: f32,
    /// Metres, at top speed with the rudder hard over.
    pub turn_radius: f32,
    /// Deck positions, filled or not.
    pub deck_mounts: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Design {
    pub name: String,
    /// Size of every frame and component against the reference cruiser.
    pub scale: f32,
    /// Multiple of the stock structure and component strength.
    pub toughness: f32,
    /// Armour over the vital modules. `None` leaves the whole hull light.
    pub citadel: Option<ArmorGrade>,
    /// Strength of deck fittings as a multiple of `toughness`.
    pub fittings: f32,
    /// Pace of the crew's repairs. Zero is a ship that cannot mend itself.
    pub repair: f32,
    /// Engine durability as a share of the surrounding compartment’s structure.
    pub(crate) engine_strength: f32,
    /// Auxiliary machinery can drive the ship and train the guns with every engine out.
    pub(crate) auxiliary_drive: bool,
    /// Strength of the side plating as a multiple of stock.
    pub(crate) plating: f32,
    /// Inches of side plate. Zero on a new design, which turns nothing aside.
    pub(crate) thickness: f32,
    /// Seconds the weapons can draw from the battery without a running power supply.
    pub reserve: f32,
    /// She founders once this share of her vital length is wrecked.
    pub sinks_at: f32,
    beam_factor: f32,
    /// Pumping, as a multiple of a stock crew's. Better pumps clear water faster.
    pumps: f32,
    /// Whether her battered modules leak and fill, so that one floods before it is
    /// holed to nothing and comes back once pumped out. Otherwise a module floods only
    /// when holed to nothing, and is afloat again once patched to a quarter.
    pub leaks: bool,
    modules: Vec<Module>,
}

fn check(modules: &[Module]) -> Result<(), Fault> {
    let ends = match modules {
        [] => false,
        [only] => only.frame == Frame::Boat,
        [bow, midships @ .., stern] => {
            bow.frame == Frame::Bow
                && stern.frame == Frame::Stern
                && midships.iter().all(|m| Frame::MIDSHIPS.contains(&m.frame))
        }
    };
    if !ends {
        return Err(Fault::Ends);
    }
    for (module, m) in modules.iter().enumerate() {
        for slot in Slot::ALL {
            if m.fit.iter().filter(|c| c.slot() == slot).count() > m.frame.slots(slot) {
                return Err(Fault::Crowded { module, slot });
            }
        }
    }
    if !modules.iter().any(|m| m.fit.contains(&Component::Engine)) {
        return Err(Fault::NoEngine);
    }
    Ok(())
}

impl Design {
    /// A light, unarmoured hull at the reference scale with a full repair crew.
    pub fn new(name: impl Into<String>, modules: Vec<Module>) -> Result<Self, Fault> {
        check(&modules)?;
        Ok(Self {
            name: name.into(),
            scale: 1.,
            toughness: 1.,
            citadel: None,
            fittings: 1.,
            repair: 1.,
            engine_strength: 0.5,
            plating: 1.,
            thickness: 0.,
            auxiliary_drive: false,
            reserve: 20.,
            sinks_at: 0.5,
            beam_factor: 1.,
            pumps: 1.,
            leaks: false,
            modules,
        })
    }

    pub fn modules(&self) -> &[Module] {
        &self.modules
    }

    /// Apply a change only if the result can still be built.
    fn amend(&mut self, change: impl FnOnce(&mut Vec<Module>)) -> Result<(), Fault> {
        let mut trial = self.modules.clone();
        change(&mut trial);
        check(&trial)?;
        self.modules = trial;
        Ok(())
    }

    /// Put a module in so that it becomes module `at`.
    pub fn insert(&mut self, at: usize, module: Module) -> Result<(), Fault> {
        if at > self.modules.len() {
            return Err(Fault::Missing);
        }
        self.amend(|modules| modules.insert(at, module))
    }

    /// Take a module out with everything fitted to it.
    pub fn remove(&mut self, at: usize) -> Result<Module, Fault> {
        let module = self.modules.get(at).cloned().ok_or(Fault::Missing)?;
        self.amend(|modules| {
            modules.remove(at);
        })?;
        Ok(module)
    }

    /// Move a whole compartment and its fittings. Bow and stern stay at the ends.
    pub fn move_module(&mut self, from: usize, to: usize) -> Result<(), Fault> {
        if from >= self.modules.len() || to >= self.modules.len() {
            return Err(Fault::Missing);
        }
        self.amend(|modules| {
            let section = modules.remove(from);
            modules.insert(to, section);
        })
    }

    pub fn replace_module(&mut self, at: usize, with: Module) -> Result<(), Fault> {
        if at >= self.modules.len() {
            return Err(Fault::Missing);
        }
        self.amend(|modules| modules[at] = with)
    }

    /// Rebuild the main turrets on a module.
    pub fn rearm(&mut self, at: usize, gunhouse: Gunhouse) -> Result<(), Fault> {
        let module = self.modules.get_mut(at).ok_or(Fault::Missing)?;
        module.gunhouse = Gunhouse {
            tier: gunhouse.tier.min(MOST_TIER),
            ..gunhouse
        };
        Ok(())
    }

    pub fn fit(&mut self, module: usize, component: Component) -> Result<Berth, Fault> {
        let index = self.modules.get(module).ok_or(Fault::Missing)?.fit.len();
        self.amend(|modules| modules[module].fit.push(component))?;
        Ok(Berth { module, index })
    }

    /// Remove one component. Later berths on the same module move down by one.
    pub fn strip(&mut self, berth: Berth) -> Result<Component, Fault> {
        let component = *self
            .modules
            .get(berth.module)
            .and_then(|m| m.fit.get(berth.index))
            .ok_or(Fault::Missing)?;
        self.amend(|modules| {
            modules[berth.module].fit.remove(berth.index);
        })?;
        Ok(component)
    }

    pub fn component(&self, berth: Berth) -> Option<Component> {
        self.modules
            .get(berth.module)?
            .fit
            .get(berth.index)
            .copied()
    }

    /// Length of one module in metres.
    pub fn module_length(&self, module: usize) -> f32 {
        self.modules[module].frame.spec().length * self.scale
    }

    /// Beam of one module in metres, widened as the whole hull is.
    pub fn module_beam(&self, module: usize) -> f32 {
        self.modules[module].frame.spec().beam * self.scale * self.beam_factor
    }

    /// Where a module starts and ends, in metres aft of the bow tip.
    pub fn span(&self, module: usize) -> (f32, f32) {
        let fore: f32 = (0..module).map(|i| self.module_length(i)).sum();
        (fore, fore + self.module_length(module))
    }

    /// The module at a point along the hull. Points beyond either end fall on that end.
    pub fn module_at(&self, aft: f32) -> usize {
        let mut end = 0.;
        for module in 0..self.modules.len() {
            end += self.module_length(module);
            if aft < end {
                return module;
            }
        }
        self.modules.len() - 1
    }

    pub fn length(&self) -> f32 {
        (0..self.modules.len()).map(|i| self.module_length(i)).sum()
    }

    pub fn beam(&self) -> f32 {
        self.modules
            .iter()
            .map(|m| m.frame.spec().beam)
            .fold(0., f32::max)
            * self.scale
            * self.beam_factor
    }

    pub fn beam_factor(&self) -> f32 {
        self.beam_factor
    }

    /// Pumping, as a multiple of a stock crew's.
    pub fn pumps(&self) -> f32 {
        self.pumps
    }

    pub fn fit_pumps(&mut self, pumps: f32) {
        self.pumps = pumps.max(0.);
    }

    /// Her side plating and the strength of what she carries.
    pub fn protection(&self) -> Protection {
        Protection {
            plating: self.plating,
            thickness: self.thickness,
            engine: self.engine_strength,
            fittings: self.fittings,
        }
    }
    /// Plate and harden the design. A [`crate::Ship`] built to it is refitted with
    /// [`crate::Ship::protect`].
    pub fn protect(&mut self, p: Protection) {
        self.plating = p.plating.max(0.);
        self.thickness = p.thickness.max(0.);
        self.engine_strength = p.engine.max(0.);
        self.fittings = p.fittings.max(0.);
    }
    /// This design plated and hardened as given.
    #[must_use]
    pub fn protected(&self, p: Protection) -> Self {
        let mut design = self.clone();
        design.protect(p);
        design
    }
    /// Damage the first `component` aboard absorbs before it is knocked out.
    pub fn strength(&self, component: Component) -> Option<f32> {
        let module = self
            .modules
            .iter()
            .position(|m| m.fit.contains(&component))?;
        Some(component.health(self.module_structure(module), self))
    }

    /// Widen all frames across the waterline without lengthening the vessel.
    pub fn widen(&mut self, beam_metres: f32) {
        if beam_metres.is_finite() && beam_metres > self.beam() {
            self.beam_factor *= beam_metres / self.beam();
        }
    }

    /// The modules from the first vital component to the last, on an armoured ship.
    pub fn citadel_span(&self) -> Option<RangeInclusive<usize>> {
        self.citadel?;
        let vital = |m: &Module| m.fit.iter().any(|c| c.vital());
        let first = self.modules.iter().position(vital)?;
        let last = self.modules.iter().rposition(vital)?;
        Some(first..=last)
    }

    /// Armour over one module: the citadel's grade inside it and light plate outside.
    pub fn armor(&self, module: usize) -> ArmorGrade {
        match (self.citadel, self.citadel_span()) {
            (Some(grade), Some(span)) if span.contains(&module) => grade,
            _ => ArmorGrade::Light,
        }
    }

    /// The modules whose loss sinks the ship: the citadel, or the whole of a light hull.
    pub(crate) fn vitals(&self) -> RangeInclusive<usize> {
        self.citadel_span().unwrap_or(0..=self.modules.len() - 1)
    }

    pub fn mass(&self) -> f32 {
        let plate = |module| match self.armor(module) {
            ArmorGrade::Light => 0.,
            ArmorGrade::Reinforced => 0.15,
            ArmorGrade::Capital => 0.3,
        };
        self.modules
            .iter()
            .enumerate()
            .map(|(i, m)| {
                self.beam_factor * m.frame.spec().mass * (1. + plate(i))
                    + m.fit.iter().map(|c| c.mass()).sum::<f32>()
            })
            .sum::<f32>()
            * self.scale.powi(3)
    }

    pub fn count(&self, component: Component) -> usize {
        self.modules
            .iter()
            .flat_map(|m| &m.fit)
            .filter(|c| **c == component)
            .count()
    }

    /// Structure of one module in hull points.
    pub fn module_structure(&self, module: usize) -> f32 {
        self.modules[module].frame.spec().structure * self.toughness
    }

    /// Structure of the whole hull in hull points.
    pub fn structure(&self) -> f32 {
        (0..self.modules.len())
            .map(|i| self.module_structure(i))
            .sum()
    }

    /// Metres per second with every engine running. Power buys speed by its cube root,
    /// so a second engine adds a quarter and an extra module costs little.
    pub fn top_speed(&self) -> f32 {
        let power = ENGINE_POWER * self.count(Component::Engine) as f32 * self.scale.powi(3);
        DESIGN_SPEED * (power / self.mass()).cbrt()
    }

    /// Radians per second with the rudder hard over. A longer hull turns more slowly.
    pub fn turn_rate(&self) -> f32 {
        TURN_RATE * TURN_LENGTH / self.length()
    }

    pub fn sheet(&self) -> Sheet {
        Sheet {
            length: self.length(),
            beam: self.beam(),
            mass: self.mass(),
            structure: self.structure(),
            top_speed: self.top_speed(),
            turn_radius: self.top_speed() / self.turn_rate(),
            deck_mounts: self
                .modules
                .iter()
                .map(|m| m.frame.slots(Slot::Centre) + m.frame.slots(Slot::Side))
                .sum(),
        }
    }

    /// A berth for each of several stations of one kind of component, given as metres
    /// aft of the bow tip. Each station takes an unclaimed berth on the module beneath
    /// it. `None` if a station has no such berth left.
    pub fn berths_for(&self, component: Component, stations: &[f32]) -> Option<Vec<Berth>> {
        let mut claimed: Vec<Berth> = Vec::new();
        for &aft in stations {
            let module = self.module_at(aft);
            let index = (0..self.modules[module].fit.len()).find(|&index| {
                self.modules[module].fit[index] == component
                    && !claimed.contains(&Berth { module, index })
            })?;
            claimed.push(Berth { module, index });
        }
        Some(claimed)
    }

    /// Every fitted component and where it stands. Components sharing a kind of slot
    /// are spaced evenly along their module, and side mounts alternate port and starboard.
    pub fn fittings(&self) -> Vec<Fitting> {
        let mut fittings = Vec::new();
        for (module, m) in self.modules.iter().enumerate() {
            let (fore, _) = self.span(module);
            let length = self.module_length(module);
            let reach = 0.45 * m.frame.spec().beam * self.scale * self.beam_factor;
            for slot in Slot::ALL {
                let sharing = m.fit.iter().filter(|c| c.slot() == slot).count();
                // Side mounts stand in pairs across the hull.
                let stations = if slot == Slot::Side {
                    sharing.div_ceil(2)
                } else {
                    sharing
                };
                let placed = m.fit.iter().enumerate().filter(|(_, c)| c.slot() == slot);
                for (order, (index, &component)) in placed.enumerate() {
                    if m.frame.abreast() && component == Component::MainGun {
                        fittings.push(Fitting {
                            berth: Berth { module, index },
                            component,
                            aft: fore + length * 0.5,
                            abeam: (if order == 0 { -1. } else { 1. }) * reach * 0.56,
                        });
                        continue;
                    }
                    let (station, abeam) = if slot == Slot::Side {
                        (order / 2, if order % 2 == 0 { -reach } else { reach })
                    } else {
                        (order, 0.)
                    };
                    fittings.push(Fitting {
                        berth: Berth { module, index },
                        component,
                        aft: fore + length * (station + 1) as f32 / (stations + 1) as f32,
                        abeam,
                    });
                }
            }
        }
        fittings
    }
}
