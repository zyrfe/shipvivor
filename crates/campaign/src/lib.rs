//! Run progression: the mission clock, objectives, hull, fittings and routes of one run.
//! Timers advance only while the ship is sailing. Menus, pausing and the helm trial
//! belong to the application, not to the run.
mod battery;
mod workshop;
pub use battery::{CALIBRES, ShipBeam};
use shipvivor_artillery::{
    AntiAir, MainShell, STOCK_TRAVERSE, Secondaries, TurretLayout, training_rate,
};
use shipvivor_shipyard::{Component, Design, Frame, Module, Protection, Ship};
pub use workshop::{
    DeckModule, DockJob, MAX_GUN_STATIONS, MAX_LENGTH, MAX_SECTIONS_PER_KIND, Refusal,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Sailing,
    Refit,
    Drydock(LegEnd),
    Chart,
    Intercept(LegEnd),
    Won(LegEnd),
    Lost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegEnd {
    BasesDestroyed,
    TimeExpired,
    Extracted,
    RegentSunk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Stage {
    #[default]
    IronNarrows,
    OuterRoad,
    GreywaterSound,
    PilotsCut,
    CrownDeep,
    LanternBay,
    BlackcapReach,
}

pub struct Anchorage {
    pub stage: Stage,
    pub pilot_note: &'static str,
}

pub const ANCHORAGES: [Anchorage; 3] = [
    Anchorage {
        stage: Stage::LanternBay,
        pilot_note: "Calm water behind wooded islands. Patrols are spaced furthest apart.",
    },
    Anchorage {
        stage: Stage::IronNarrows,
        pilot_note: "Steady patrols in a winding channel between the islands.",
    },
    Anchorage {
        stage: Stage::BlackcapReach,
        pilot_note: "An exposed coast with heavy swell and the quickest reinforcements.",
    },
];

/// The time limit and star goals of one leg.
#[derive(Clone, Copy)]
pub struct Mission {
    pub name: &'static str,
    pub minutes: u16,
    pub kills: u16,
    pub crates: u16,
}

impl Stage {
    pub fn is_opening(self) -> bool {
        ANCHORAGES.iter().any(|port| port.stage == self)
    }

    /// The three-star passage takes the ship into enemy air cover.
    pub fn enters_air_cover(self) -> bool {
        self == Self::PilotsCut
    }

    pub fn mission(self) -> Mission {
        let (name, minutes, kills, crates) = match self {
            Self::CrownDeep => ("Crown Deep", 15, 0, 0),
            Self::IronNarrows => ("The Iron Narrows", 10, 4, 2),
            Self::LanternBay => ("Lantern Bay", 12, 3, 2),
            Self::BlackcapReach => ("Blackcap Reach", 9, 5, 2),
            Self::OuterRoad => ("Outer Road", 12, 7, 3),
            Self::GreywaterSound => ("Greywater Sound", 12, 6, 4),
            Self::PilotsCut => ("Pilot's Cut", 15, 8, 3),
        };
        Mission {
            name,
            minutes,
            kills,
            crates,
        }
    }
}

pub const MAX_HULL: u16 = 100;
/// One star per mission goal.
pub const MAX_STARS: u8 = 3;
pub const RESERVE_STARS: u8 = 2;

/// A whole section the yard can splice into the hull: a straight frame and what stands on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockModule {
    Reserve,
    EngineRoom,
    Turret,
    Floatplane,
    Missiles,
}
impl DockModule {
    /// The yard's stock, in the order it is laid out.
    pub const ALL: [Self; 5] = [
        Self::Turret,
        Self::EngineRoom,
        Self::Floatplane,
        Self::Missiles,
        Self::Reserve,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Reserve => "Reserve bay",
            Self::EngineRoom => "Auxiliary engine",
            Self::Turret => "Centre turret",
            Self::Floatplane => "Floatplane catapult",
            Self::Missiles => "Missile rack",
        }
    }

    pub fn build(self) -> Module {
        let fit = match self {
            Self::Reserve => None,
            Self::EngineRoom => Some(Component::Engine),
            Self::Turret => Some(Component::MainGun),
            Self::Floatplane => Some(Component::Catapult),
            Self::Missiles => Some(Component::MissileRack),
        };
        Module::new(Frame::Straight, fit)
    }
}
/// Hull restored by each recovered crate.
pub const CRATE_REPAIR: u16 = 12;
/// Hull points of repair the dock gives between legs. A badly damaged ship sails with
/// whatever this does not reach.
pub const DOCK_REPAIR: u16 = 40;
/// Time until extraction opens if no drydock has been destroyed.
pub const EXTRACT_SECONDS: f32 = 60.;

/// What a route grants on arrival.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Perk {
    None,
    Repair(u16),
    FreeFitting,
}

/// A route out of the first leg and the stars needed to take it.
#[derive(Clone, Copy, Debug)]
pub struct Exit {
    pub stage: Stage,
    pub stars: u8,
    pub perk: Perk,
}

pub const EXITS: [Exit; 3] = [
    Exit {
        stage: Stage::OuterRoad,
        stars: 0,
        perk: Perk::None,
    },
    Exit {
        stage: Stage::GreywaterSound,
        stars: 2,
        perk: Perk::Repair(30),
    },
    Exit {
        stage: Stage::PilotsCut,
        stars: 3,
        perk: Perk::FreeFitting,
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Card {
    Loader,
    Shells,
    Turbines,
    Caps,
    Velocity,
    TripleTurrets,
    QuadTurrets,
    /// Unlocks AP rounds for the main battery. Stock rounds are HE.
    ArmorPiercing,
    /// One more wing gun on each beam.
    MoreSecondaries,
    /// More damage from each secondary hit.
    HeavySecondaries,
    /// A shorter secondary reload.
    QuickSecondaries,
    /// The crew engages farther out.
    SecondaryRange,
    /// A second barrel on every wing gun.
    TwinSecondaries,
    /// Faster training and elevation for every main gunhouse.
    TrainingGear,
    /// Heavier side plating, which wears more slowly and turns aside part of a hit.
    Armor,
    /// A stronger engine and stronger deck fittings.
    Hardening,
    /// A pair of light anti-aircraft guns.
    AntiAir,
    /// Another pair of AA guns.
    MoreAntiAir,
    /// More damage from each AA burst.
    HeavyAntiAir,
    /// A shorter AA reload.
    QuickAntiAir,
}

/// Metres each rangefinder card adds to the secondaries' reach.
const WING_RANGE_STEP: f32 = 40.;
/// The most ranks of each refit the ship can take.
pub mod limits {
    pub const TRAINING_GEAR: u16 = shipvivor_artillery::MOST_TRAINING_GEAR;
    /// At the widest beam. A narrower hull carries fewer: see [`crate::ShipBeam::most_armor`].
    pub const ARMOR: u16 = crate::ShipBeam::Capital.most_armor();
    pub const HARDENING: u16 = 2;
    pub const AA_MOUNTS: u16 = 2;
    pub const AA_SHELLS: u16 = 2;
    pub const AA_LOADERS: u16 = 3;
}
/// Each rank of armor plate adds this share of stock strength to every panel, and
/// this many inches to its thickness. On sound plate the stock beam's two ranks turn
/// the enemy's secondaries and aircraft guns, then patrol boats and destroyers. A
/// cruiser's beam adds cruisers, then battleships and shore guns, and a capital beam
/// the Parliament's guns. Nothing turns the Regent's.
const ARMOR_PLATE: f32 = 0.5;
const ARMOR_INCHES: f32 = 3.;
/// Each rank of hardening adds this share of stock strength to every deck fitting.
const HARDENED_FITTINGS: f32 = 0.5;

/// Seconds per inch of calibre for one barrel to reload with stock gear: 6.4 s at four inches.
const RELOAD_PER_INCH: f32 = 1.6;

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fittings {
    pub beam: ShipBeam,
    pub wings_only: bool,
    pub turrets: TurretLayout,
    pub loader: u16,
    pub shells: u16,
    pub caps: u16,
    pub velocity: u16,
    pub turbines: u16,
    pub armor_piercing: u16,
    pub wing_mounts: u16,
    pub wing_shells: u16,
    pub wing_loaders: u16,
    pub wing_range: u16,
    pub wing_twin: u16,
    pub training_gear: u16,
    pub armor: u16,
    pub hardening: u16,
    pub anti_air: u16,
    pub aa_mounts: u16,
    pub aa_shells: u16,
    pub aa_loaders: u16,
}
impl Fittings {
    /// These fittings with one more rank of `card`.
    #[must_use]
    pub fn with(mut self, card: Card) -> Self {
        if !self.can_fit(card) {
            return self;
        }
        let rank = match card {
            Card::Loader => &mut self.loader,
            Card::Shells => &mut self.shells,
            Card::Caps => &mut self.caps,
            Card::Velocity => &mut self.velocity,
            Card::Turbines => &mut self.turbines,
            Card::ArmorPiercing => &mut self.armor_piercing,
            Card::MoreSecondaries => &mut self.wing_mounts,
            Card::HeavySecondaries => &mut self.wing_shells,
            Card::QuickSecondaries => &mut self.wing_loaders,
            Card::SecondaryRange => &mut self.wing_range,
            Card::TwinSecondaries => &mut self.wing_twin,
            Card::TrainingGear => &mut self.training_gear,
            Card::Armor => &mut self.armor,
            Card::Hardening => &mut self.hardening,
            Card::AntiAir => &mut self.anti_air,
            Card::MoreAntiAir => &mut self.aa_mounts,
            Card::HeavyAntiAir => &mut self.aa_shells,
            Card::QuickAntiAir => &mut self.aa_loaders,
            Card::TripleTurrets | Card::QuadTurrets => {
                self.turrets = match (card, self.turrets) {
                    (Card::TripleTurrets, TurretLayout::Twin) => TurretLayout::Triple,
                    (Card::QuadTurrets, TurretLayout::Triple) => TurretLayout::Quad,
                    _ => self.turrets,
                };
                return self;
            }
        };
        *rank = rank.saturating_add(1);
        self
    }
    /// Seconds for one main battery barrel to reload. A twin mount working both barrels
    /// in turn fires every half of this. A heavier shell takes longer to load, in proportion
    /// to its calibre. Each rank of quick loaders divides that down, so four ranks bring an
    /// eight-inch battery back to the four-inch stock time.
    pub fn reload(&self) -> f32 {
        RELOAD_PER_INCH * self.round().calibre_inches() / (1. + 0.25 * f32::from(self.loader))
    }
    /// The main battery round as fitted, filled with HE. The crew loads whichever
    /// filling is selected, and AP only once it is fitted.
    pub fn round(&self) -> MainShell {
        MainShell {
            bore: self.calibre() - CALIBRES[0],
            caps: self.caps,
            velocity: self.velocity,
            ..MainShell::STOCK
        }
    }
    pub fn armor_piercing(&self) -> bool {
        self.armor_piercing > 0
    }
    /// Degrees per second the main battery trains, and elevates in proportion. A larger
    /// bore trains more slowly, and training gear wins it back.
    pub fn traverse(&self) -> f32 {
        STOCK_TRAVERSE * training_rate(self.round().calibre_inches(), self.training_gear)
    }
    /// The light AA battery: a pair of guns once fitted and another pair per mounting
    /// card, one more damage a burst per heavier round, and quick loaders working as
    /// they do for the main battery.
    pub fn anti_air(&self) -> AntiAir {
        if self.anti_air == 0 {
            return AntiAir::NONE;
        }
        AntiAir {
            mounts: (2 + 2 * usize::from(self.aa_mounts)).min(AntiAir::MOST_MOUNTS),
            damage: AntiAir::NONE.damage.saturating_add(self.aa_shells),
            reload: AntiAir::NONE.reload / (1. + 0.25 * f32::from(self.aa_loaders)),
            range: AntiAir::NONE.range,
        }
    }
    /// The hull's protection with these fittings, on a hull built with `stock`. Armor
    /// plate strengthens and thickens the side plating, so it wears more slowly and
    /// turns aside heavier rounds. Hardening brings the engine up to the strength of
    /// its compartment and toughens every deck fitting.
    pub fn protection(&self, stock: Protection) -> Protection {
        let armor = f32::from(self.armor.min(limits::ARMOR));
        let hardening = f32::from(self.hardening.min(limits::HARDENING));
        Protection {
            plating: stock.plating * (1. + ARMOR_PLATE * armor),
            thickness: stock.thickness + ARMOR_INCHES * armor,
            engine: stock.engine
                + (1. - stock.engine).max(0.) * hardening / f32::from(limits::HARDENING),
            fittings: stock.fittings * (1. + HARDENED_FITTINGS * hardening),
        }
    }
    /// The close-range battery: one more gun a beam per mounting card, one more damage a
    /// hit per heavier round, quick loaders working as they do for the main battery, 40 m
    /// more reach per rangefinder and a second barrel on every gun once twinned.
    pub fn secondaries(&self) -> Secondaries {
        Secondaries {
            per_side: (1 + usize::from(self.wing_mounts)).min(Secondaries::MOST_PER_SIDE),
            damage: Secondaries::STOCK.damage.saturating_add(self.wing_shells),
            reload: Secondaries::STOCK.reload / (1. + 0.25 * f32::from(self.wing_loaders)),
            range: Secondaries::STOCK.range + WING_RANGE_STEP * f32::from(self.wing_range),
            barrels: if self.wing_twin > 0 { 2 } else { 1 },
        }
    }

    /// Hands come round in four themes: ammunition first, then the main battery, then
    /// machinery and protection, then the close-range and anti-aircraft guns. A hand
    /// that runs out of its own cards is filled from the rest.
    pub fn offers(&self) -> Vec<Card> {
        use Card::*;
        let installed = [
            self.shells,
            self.caps,
            self.velocity,
            self.loader,
            self.turbines,
            self.armor_piercing,
            self.wing_mounts,
            self.wing_shells,
            self.wing_loaders,
            self.wing_range,
            self.wing_twin,
            self.training_gear,
            self.armor,
            self.hardening,
            self.anti_air,
            self.aa_mounts,
            self.aa_shells,
            self.aa_loaders,
        ]
        .into_iter()
        .map(u32::from)
        .sum::<u32>()
            + (self.turrets.count() - 2) as u32;
        // Each close-range hand leads with a different card.
        let mut close = [
            MoreSecondaries,
            AntiAir,
            TwinSecondaries,
            HeavySecondaries,
            MoreAntiAir,
            SecondaryRange,
            HeavyAntiAir,
            QuickSecondaries,
            QuickAntiAir,
        ];
        let turn = (installed / 4) as usize % close.len();
        close.rotate_left(turn);
        let (lead, rest): (&[Card], &[Card]) = match installed % 4 {
            0 => (
                &[ArmorPiercing, Caps, Shells, Velocity],
                &[
                    Loader,
                    TrainingGear,
                    Turbines,
                    Armor,
                    Hardening,
                    TripleTurrets,
                    QuadTurrets,
                ],
            ),
            1 => (
                &[Loader, TripleTurrets, QuadTurrets, TrainingGear],
                &[
                    Turbines,
                    Armor,
                    Caps,
                    Velocity,
                    Shells,
                    ArmorPiercing,
                    Hardening,
                ],
            ),
            2 => (
                &[Armor, Turbines, Hardening],
                &[
                    Velocity,
                    Shells,
                    TrainingGear,
                    TripleTurrets,
                    QuadTurrets,
                    Caps,
                    Loader,
                    ArmorPiercing,
                ],
            ),
            _ => (
                &close,
                &[
                    ArmorPiercing,
                    Caps,
                    Turbines,
                    Hardening,
                    Loader,
                    Velocity,
                    Shells,
                    Armor,
                    TrainingGear,
                    TripleTurrets,
                    QuadTurrets,
                ],
            ),
        };
        let close_after = if installed % 4 == 3 {
            &[][..]
        } else {
            &close[..]
        };
        lead.iter()
            .chain(rest)
            .chain(close_after)
            .copied()
            .filter(|card| self.can_fit(*card))
            .take(3)
            .collect()
    }
    /// Ranks of `card` fitted, out of the most the ship can ever take, for a card that
    /// stacks to a limit. `None` for a card that is fitted once and replaced, or that has
    /// no limit.
    pub fn ranks(&self, card: Card) -> Option<(u16, u16)> {
        use Card::*;
        let most_per_side = Secondaries::MOST_PER_SIDE as u16 - 1;
        Some(match card {
            MoreSecondaries => (self.wing_mounts.min(most_per_side), most_per_side),
            HeavySecondaries => (self.wing_shells, 2),
            QuickSecondaries => (self.wing_loaders, 3),
            SecondaryRange => (self.wing_range, 2),
            TwinSecondaries => (self.wing_twin, 1),
            TrainingGear => (self.training_gear, limits::TRAINING_GEAR),
            Armor => (self.armor, limits::ARMOR),
            Hardening => (self.hardening, limits::HARDENING),
            MoreAntiAir => (self.aa_mounts, limits::AA_MOUNTS),
            HeavyAntiAir => (self.aa_shells, limits::AA_SHELLS),
            QuickAntiAir => (self.aa_loaders, limits::AA_LOADERS),
            _ => return None,
        })
    }
    pub fn can_fit(&self, card: Card) -> bool {
        use Card::*;
        match card {
            // Caps drive AP only, and stop helping once it reaches the capital grade.
            Caps => self.armor_piercing() && self.caps.saturating_add(self.round().bore / 2) < 2,
            ArmorPiercing => !self.armor_piercing(),
            Shells => CALIBRES
                .get(usize::from(self.shells) + 1)
                .is_some_and(|&inch| inch <= self.capacity()),
            MoreSecondaries => self.secondaries().per_side < Secondaries::MOST_PER_SIDE,
            HeavySecondaries => self.wing_shells < 2,
            QuickSecondaries => self.wing_loaders < 3,
            SecondaryRange => self.wing_range < 2,
            TwinSecondaries => self.wing_twin == 0,
            TrainingGear => self.training_gear < limits::TRAINING_GEAR,
            Armor => self.armor < self.beam.most_armor(),
            Hardening => self.hardening < limits::HARDENING,
            AntiAir => self.anti_air == 0,
            MoreAntiAir => self.anti_air > 0 && self.aa_mounts < limits::AA_MOUNTS,
            HeavyAntiAir => self.anti_air > 0 && self.aa_shells < limits::AA_SHELLS,
            QuickAntiAir => self.anti_air > 0 && self.aa_loaders < limits::AA_LOADERS,
            TripleTurrets => {
                self.turrets == TurretLayout::Twin && self.can_carry(TurretLayout::Triple)
            }
            QuadTurrets => {
                self.turrets == TurretLayout::Triple && self.can_carry(TurretLayout::Quad)
            }
            _ => true,
        }
    }
    pub fn speed_factor(&self) -> f32 {
        1. + 0.15 * f32::from(self.turbines)
    }
}

#[derive(Default, Clone, Copy)]
pub struct Objectives {
    pub kills: u16,
    pub crates: u16,
    pub destroyed_drydock: bool,
}
pub const CRATE_XP: u32 = 10;

pub struct Run {
    phase: Phase,
    pub stage: Stage,
    /// Where the ship began her first patrol, retained on subsequent charts.
    pub origin: Stage,
    /// The exit actually sailed, retained after the routes meet at Crown Deep.
    pub passage: Option<Stage>,
    pub elapsed: f32,
    /// Breakwater herself: her modules, what is fitted to them, and their condition.
    pub ship: Ship,
    pub fittings: Fittings,
    pub objectives: Objectives,
    pub previous_stars: u8,
    pub experience: u32,
    pub level: u32,
    pending_cards: u16,
}
impl Default for Run {
    fn default() -> Self {
        Self {
            phase: Phase::Sailing,
            stage: Stage::IronNarrows,
            origin: Stage::IronNarrows,
            passage: None,
            elapsed: 0.,
            ship: Ship::new(Design::breakwater()),
            fittings: Fittings::default(),
            objectives: Objectives::default(),
            previous_stars: 0,
            experience: 0,
            level: 1,
            pending_cards: 0,
        }
    }
}
/// A default run is a fresh ship at the start of the first leg.
impl Run {
    pub fn phase(&self) -> Phase {
        self.phase
    }
    /// Seconds until the leg ends on its own.
    pub fn remaining(&self) -> f32 {
        f32::from(self.stage.mission().minutes) * 60. - self.elapsed
    }
    pub fn next_level_xp(&self) -> u32 {
        self.level.saturating_add(2).saturating_mul(10)
    }

    pub fn stars(&self) -> u8 {
        if self.stage == Stage::CrownDeep {
            return 0;
        }
        let goals = self.stage.mission();
        u8::from(self.objectives.kills >= goals.kills)
            + u8::from(self.objectives.crates >= goals.crates)
            + u8::from(self.objectives.destroyed_drydock)
    }
    pub fn can_extract(&self) -> bool {
        self.stage != Stage::CrownDeep
            && (self.objectives.destroyed_drydock || self.elapsed >= EXTRACT_SECONDS)
    }
    pub fn extract(&mut self) {
        if self.phase == Phase::Sailing && self.can_extract() {
            self.finish_leg(LegEnd::Extracted);
        }
    }
    /// The installed drydock sections and their compartment indices, from bow to stern.
    /// A centre turret is not listed: she was built with two of her own.
    pub fn dock_modules(&self) -> impl Iterator<Item = (usize, DockModule)> + '_ {
        let modules = self.ship.design().modules();
        modules
            .iter()
            .enumerate()
            .filter_map(|(i, m)| match (m.frame, m.fit.as_slice()) {
                (Frame::Straight, []) => Some((i, DockModule::Reserve)),
                (Frame::Straight, [Component::Engine]) => Some((i, DockModule::EngineRoom)),
                (Frame::Straight, [Component::Catapult]) => Some((i, DockModule::Floatplane)),
                (Frame::Straight, [Component::MissileRack]) => Some((i, DockModule::Missiles)),
                _ => None,
            })
    }

    pub fn added_length(&self) -> f32 {
        self.ship.design().length() - Design::BREAKWATER_LENGTH
    }

    fn finish_leg(&mut self, reason: LegEnd) {
        if self.stage.is_opening() {
            self.ship.dock(f32::from(DOCK_REPAIR));
            for (earned, module) in [
                (self.stars() >= RESERVE_STARS, DockModule::Reserve),
                (reason == LegEnd::BasesDestroyed, DockModule::EngineRoom),
            ] {
                if earned {
                    let aft_gun = self.ship.design().modules().len() - 2;
                    self.ship
                        .insert_module(aft_gun, module.build())
                        .expect("a midship frame fits before the aft gun");
                }
            }
            let dock_fittings = 1 + u16::from(reason == LegEnd::BasesDestroyed);
            self.pending_cards = self.pending_cards.saturating_add(dock_fittings);
            self.phase = Phase::Drydock(reason);
        } else {
            self.phase = Phase::Intercept(reason);
        }
    }
    /// All second-leg routes rendezvous here. Keep earned upgrades and honour any
    /// fitting earned on the last tick before putting the player back under way.
    pub fn engage_regent(&mut self) -> bool {
        if !matches!(self.phase, Phase::Intercept(_)) {
            return false;
        }
        self.previous_stars += self.stars();
        self.stage = Stage::CrownDeep;
        self.objectives = Objectives::default();
        self.elapsed = 0.;
        self.ship.dock(f32::from(DOCK_REPAIR));
        self.phase = if self.pending_cards > 0 {
            Phase::Refit
        } else {
            Phase::Sailing
        };
        true
    }
    pub fn fittings_remaining(&self) -> u16 {
        self.pending_cards
    }
    /// Commit one gameplay tick. A lethal hit ends the run before any rewards.
    ///
    /// `incoming` is damage with no place on the hull, and is spread over every module.
    /// Hits that landed somewhere go through [`Run::strike`] first.
    pub fn tick(&mut self, dt: f32, gains: Objectives, incoming: u16) {
        self.tick_sector(dt, gains, incoming, false);
    }
    /// Resolve the encounter and deadline together. A surviving ship that destroys
    /// the last yard on the closing tick earns the clearance.
    pub fn tick_sector(
        &mut self,
        dt: f32,
        gains: Objectives,
        incoming: u16,
        all_bases_destroyed: bool,
    ) {
        self.tick_engagement(
            dt,
            gains,
            incoming,
            all_bases_destroyed.then_some(LegEnd::BasesDestroyed),
        );
    }
    /// Only the final flagship counts in Crown Deep. A lethal blow on the same tick
    /// loses the run, while sinking the Regent at the deadline wins it.
    pub fn tick_engagement(
        &mut self,
        dt: f32,
        gains: Objectives,
        incoming: u16,
        objective: Option<LegEnd>,
    ) {
        if self.phase != Phase::Sailing {
            return;
        }
        self.refit_protection();
        let dt = dt.min(self.remaining().max(0.));
        self.elapsed += dt;
        if incoming > 0 {
            let design = self.ship.design().clone();
            for module in 0..design.modules().len() {
                let (fore, aft) = design.span(module);
                let share = design.module_structure(module) / design.structure();
                self.ship
                    .strike_hull(0.5 * (fore + aft), f32::from(incoming) * share);
            }
        }
        if !self.ship.afloat() {
            self.phase = Phase::Lost;
            return;
        }
        // The crew mends what it can, and the repair party goes where it is needed most.
        self.ship.tend(dt);
        if self.ship.party().is_none() {
            let urgent = self.ship.triage();
            self.ship.assign(urgent);
        }
        self.objectives.kills = self.objectives.kills.saturating_add(gains.kills);
        self.objectives.crates = self.objectives.crates.saturating_add(gains.crates);
        self.objectives.destroyed_drydock |=
            gains.destroyed_drydock || objective == Some(LegEnd::BasesDestroyed);
        self.ship
            .patch(f32::from(gains.crates.saturating_mul(CRATE_REPAIR)));
        self.experience += u32::from(gains.crates) * CRATE_XP;
        while self.experience >= self.next_level_xp() {
            self.experience -= self.next_level_xp();
            self.level += 1;
            self.pending_cards = self.pending_cards.saturating_add(1);
        }
        if self.stage == Stage::CrownDeep && objective == Some(LegEnd::RegentSunk) {
            self.phase = Phase::Won(LegEnd::RegentSunk);
        } else if self.stage != Stage::CrownDeep && objective == Some(LegEnd::BasesDestroyed) {
            self.finish_leg(LegEnd::BasesDestroyed);
        } else if self.remaining() <= 0. {
            if self.stage == Stage::CrownDeep {
                self.phase = Phase::Lost;
            } else {
                self.finish_leg(LegEnd::TimeExpired);
            }
        } else if self.pending_cards > 0 {
            self.phase = Phase::Refit;
        }
    }
    /// A hit on the hull, `aft` metres behind the bow tip, that meets no plating. It
    /// lands on the module there.
    pub fn strike(&mut self, aft: f32, damage: u16) {
        if self.phase == Phase::Sailing {
            self.ship.strike_hull(aft, f32::from(damage));
        }
    }
    /// A hit on her side, `aft` metres behind the bow tip and `abeam` metres to
    /// starboard, from a round `calibre` inches across or, with `None`, a blow no plate
    /// can turn. The plating there takes it first, then the module behind, and `reach`
    /// says which of its bays the blow got into.
    pub fn strike_side(
        &mut self,
        aft: f32,
        abeam: f32,
        damage: u16,
        calibre: Option<f32>,
        reach: shipvivor_shipyard::Reach,
    ) {
        if self.phase == Phase::Sailing {
            self.ship
                .strike_into(aft, abeam, f32::from(damage), calibre, reach);
        }
    }
    /// Total surviving structure, including any drydock sections.
    pub fn hull(&self) -> u16 {
        self.ship.hull().round() as u16
    }
    /// Plate and harden the hull as her fittings say.
    pub(crate) fn refit_protection(&mut self) {
        self.ship
            .protect(self.fittings.protection(Protection::BREAKWATER));
    }
    pub fn hull_percent(&self) -> u16 {
        (self.ship.integrity() * 100.).round() as u16
    }
    pub fn install(&mut self, card: Card) {
        if !matches!(self.phase, Phase::Refit | Phase::Drydock(_))
            || self.pending_cards == 0
            || !self.fittings.offers().contains(&card)
        {
            return;
        }
        self.fittings = self.fittings.with(card);
        self.refit_protection();
        self.pending_cards -= 1;
        // A ship in dock stays there until she is sailed, with [`Run::leave_dock`].
        if self.pending_cards == 0 && self.phase == Phase::Refit {
            self.phase = Phase::Sailing;
        }
    }
    pub fn depart(&mut self, next: Stage) -> bool {
        let Some(exit) = EXITS.iter().find(|exit| exit.stage == next) else {
            return false;
        };
        if self.phase != Phase::Chart || self.stars() < exit.stars {
            return false;
        }
        self.previous_stars = self.stars();
        self.passage = Some(next);
        self.stage = next;
        self.elapsed = 0.;
        self.objectives = Objectives::default();
        self.phase = Phase::Sailing;
        match exit.perk {
            Perk::None => {}
            Perk::Repair(extra) => {
                self.ship.dock(f32::from(extra));
            }
            Perk::FreeFitting => {
                self.pending_cards = 1;
                self.phase = Phase::Refit;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shipvivor_artillery::Ammo;
    #[test]
    fn the_yard_installs_each_earned_section_once_and_it_sails_the_next_leg() {
        for combat_stars in 0..=2 {
            for end in [
                LegEnd::TimeExpired,
                LegEnd::Extracted,
                LegEnd::BasesDestroyed,
            ] {
                let mut run = Run::default();
                let goals = run.stage.mission();
                let gains = Objectives {
                    kills: if combat_stars >= 1 { goals.kills } else { 0 },
                    crates: if combat_stars >= 2 { goals.crates } else { 0 },
                    ..Default::default()
                };
                let time = if end == LegEnd::TimeExpired {
                    run.remaining()
                } else {
                    EXTRACT_SECONDS
                };
                run.tick_sector(time, gains, 0, end == LegEnd::BasesDestroyed);
                if end == LegEnd::Extracted {
                    run.extract();
                }
                assert_eq!(run.phase(), Phase::Drydock(end));
                let stars = combat_stars + u8::from(end == LegEnd::BasesDestroyed);
                assert_eq!(run.stars(), stars);
                let mut expected = Vec::new();
                if stars >= RESERVE_STARS {
                    expected.push(DockModule::Reserve);
                }
                if end == LegEnd::BasesDestroyed {
                    expected.push(DockModule::EngineRoom);
                }
                let count = expected.len();
                run.extract();
                run.tick_sector(1., gains, 0, true);
                assert_eq!(
                    run.dock_modules()
                        .map(|(_, module)| module)
                        .collect::<Vec<_>>(),
                    expected
                );
                assert_eq!(run.ship.design().length(), 30. + 5. * count as f32);
                assert_eq!(run.ship.compartments().len(), 6 + count);
                assert_eq!(run.hull_percent(), 100);
                assert_eq!(
                    run.fittings_remaining(),
                    1 + u16::from(end == LegEnd::BasesDestroyed)
                );
                while run.fittings_remaining() > 0 {
                    run.install(run.fittings.offers()[0]);
                }
                assert_eq!(run.phase(), Phase::Drydock(end));
                assert!(run.leave_dock());
                assert!(run.depart(Stage::OuterRoad));
                assert_eq!(
                    run.ship.design().count(Component::Engine),
                    1 + usize::from(end == LegEnd::BasesDestroyed)
                );
                run.tick_sector(1., gains, 0, true);
                assert_eq!(run.phase(), Phase::Intercept(LegEnd::BasesDestroyed));
                assert_eq!(run.ship.compartments().len(), 6 + count);
            }
        }
    }

    #[test]
    fn ammunition_cards_change_distinct_properties_and_never_offer_a_spent_grade() {
        let stock = Fittings::default();
        assert_eq!(
            stock.offers(),
            [Card::ArmorPiercing, Card::Shells, Card::Velocity]
        );
        assert_eq!(stock.round().ammo, Ammo::He);
        let heavy = stock.with(Card::Shells).round();
        assert!(heavy.calibre_inches() > stock.round().calibre_inches());
        assert_eq!(heavy.speed(), stock.round().speed());
        let ap = |f: Fittings| MainShell {
            ammo: Ammo::Ap,
            ..f.round()
        };
        let armed = stock.with(Card::ArmorPiercing);
        assert!(armed.armor_piercing() && !armed.offers().contains(&Card::ArmorPiercing));
        let capped = armed.with(Card::Caps);
        assert_ne!(ap(capped).penetration(), ap(armed).penetration());
        assert_eq!(capped.round().penetration(), stock.round().penetration());
        assert_eq!(capped.round().breach_force(), stock.round().breach_force());
        let fast = stock.with(Card::Velocity).round();
        assert!(fast.speed() > stock.round().speed());
        assert_eq!(fast.penetration(), stock.round().penetration());
        let max = Fittings {
            armor_piercing: 1,
            caps: 1,
            shells: 4,
            velocity: 4,
            ..stock
        };
        let offers = max.offers();
        assert_eq!(offers.len(), 3);
        assert!(
            offers
                .iter()
                .all(|card| !matches!(card, Card::ArmorPiercing | Card::Caps | Card::Shells))
        );
    }
    #[test]
    fn reload_grows_with_calibre_and_quick_loaders_cut_it_back() {
        let stock = Fittings::default();
        assert!((stock.reload() - 6.4).abs() < 1e-5);
        for (rank, pair) in CALIBRES.windows(2).enumerate() {
            let now = Fittings {
                shells: rank as u16,
                beam: ShipBeam::Capital,
                ..stock
            };
            let step = now.with(Card::Shells).reload() - now.reload();
            assert!(
                (step - 1.6 * f32::from(pair[1] - pair[0])).abs() < 1e-5,
                "each extra inch adds 1.6 s, got {step} for {pair:?}"
            );
        }
        let eight = Fittings {
            shells: 2,
            beam: ShipBeam::Cruiser,
            ..stock
        };
        assert!((eight.reload() - 12.8).abs() < 1e-5);
        assert!(eight.with(Card::Loader).reload() < eight.reload());
        let refit = Fittings { loader: 4, ..eight };
        assert!((refit.reload() - stock.reload()).abs() < 1e-5);
    }
    #[test]
    fn secondary_cards_mount_arm_and_drill_the_close_range_battery_up_to_their_limits() {
        let stock = Fittings::default();
        assert_eq!(stock.secondaries(), Secondaries::STOCK);
        // Every fourth hand is for the close-range and anti-aircraft guns.
        let fourth = Fittings {
            velocity: 3,
            ..stock
        };
        assert_eq!(
            fourth.offers(),
            [Card::MoreSecondaries, Card::AntiAir, Card::TwinSecondaries]
        );
        // The next close-range hand leads with a different card.
        let eighth = Fittings {
            velocity: 7,
            ..stock
        };
        assert_eq!(eighth.offers()[0], Card::AntiAir);
        let mut full = stock;
        for card in [
            Card::MoreSecondaries,
            Card::MoreSecondaries,
            Card::HeavySecondaries,
            Card::HeavySecondaries,
            Card::QuickSecondaries,
            Card::QuickSecondaries,
            Card::QuickSecondaries,
            Card::SecondaryRange,
            Card::SecondaryRange,
            Card::TwinSecondaries,
        ] {
            full = full.with(card);
        }
        let fit = full.secondaries();
        assert_eq!((fit.per_side, fit.damage, fit.barrels), (3, 3, 2));
        assert!((fit.range - 240.).abs() < 1e-5);
        assert!((fit.reload - Secondaries::STOCK.reload / 1.75).abs() < 1e-5);
        for velocity in 0..4 {
            let offers = Fittings { velocity, ..full }.offers();
            assert!(offers.iter().all(|card| !matches!(
                card,
                Card::MoreSecondaries
                    | Card::HeavySecondaries
                    | Card::QuickSecondaries
                    | Card::SecondaryRange
                    | Card::TwinSecondaries
            )));
        }
    }
    #[test]
    fn the_ranks_the_yard_shows_end_where_a_card_stops_fitting() {
        use Card::*;
        // The broadest hull carries the most plate, and AA must be aboard to improve.
        let stock = Fittings {
            beam: ShipBeam::Capital,
            anti_air: 1,
            ..Fittings::default()
        };
        for card in [
            MoreSecondaries,
            HeavySecondaries,
            QuickSecondaries,
            SecondaryRange,
            TwinSecondaries,
            TrainingGear,
            Armor,
            Hardening,
            MoreAntiAir,
            HeavyAntiAir,
            QuickAntiAir,
        ] {
            let (none, most) = stock.ranks(card).unwrap();
            assert_eq!(none, 0, "{card:?}");
            let mut fit = stock;
            for rank in 1..=most {
                assert!(fit.can_fit(card), "{card:?}");
                fit = fit.with(card);
                assert_eq!(fit.ranks(card), Some((rank, most)), "{card:?}");
            }
            assert!(!fit.can_fit(card), "{card:?} fits past its last rank");
        }
        assert_eq!(stock.ranks(Loader), None);
    }
    #[test]
    fn every_third_hand_is_for_machinery_and_protection_and_each_card_stops_at_its_limit() {
        use shipvivor_shipyard::Component;
        let stock = Fittings::default();
        let third = Fittings {
            velocity: 2,
            ..stock
        };
        assert_eq!(
            third.offers(),
            [Card::Armor, Card::Turbines, Card::Hardening]
        );
        // The main battery's hand offers training gear beside the loaders and turrets.
        let second = Fittings {
            velocity: 1,
            ..stock
        };
        assert!(second.offers().contains(&Card::TrainingGear));
        for (card, most) in [
            (Card::TrainingGear, limits::TRAINING_GEAR),
            (Card::Armor, stock.beam.most_armor()),
            (Card::Hardening, limits::HARDENING),
        ] {
            let mut fit = stock;
            for _ in 0..most {
                assert!(fit.can_fit(card), "{card:?}");
                fit = fit.with(card);
            }
            assert!(!fit.can_fit(card) && fit.with(card) == fit, "{card:?}");
        }
        // A larger bore trains more slowly, and training gear wins it back.
        let heavy = Fittings {
            shells: 2,
            beam: ShipBeam::Cruiser,
            ..stock
        };
        assert!((stock.traverse() - 18.).abs() < 1e-4);
        assert!(heavy.traverse() < stock.traverse());
        assert!(heavy.with(Card::TrainingGear).traverse() > heavy.traverse());
        // Armor and hardening reach the ship as soon as they are fitted.
        let mut run = Run::default();
        run.tick(
            0.,
            Objectives {
                crates: 3,
                ..Default::default()
            },
            0,
        );
        run.fittings.velocity = 2;
        let plate = run.ship.design().plate_strength(1);
        run.install(Card::Armor);
        assert!(run.ship.design().plate_strength(1) > plate);
        run.fittings = run.fittings.with(Card::Hardening).with(Card::Hardening);
        run.tick(0.1, Objectives::default(), 0);
        assert_eq!(
            run.ship.design().strength(Component::Engine),
            Some(run.ship.design().module_structure(3))
        );
    }
    #[test]
    fn aa_guns_come_aboard_in_pairs_and_their_upgrades_wait_for_them() {
        let stock = Fittings::default();
        assert_eq!(stock.anti_air().mounts, 0);
        for upgrade in [Card::MoreAntiAir, Card::HeavyAntiAir, Card::QuickAntiAir] {
            assert!(!stock.can_fit(upgrade));
        }
        let fitted = stock.with(Card::AntiAir);
        assert!(!fitted.can_fit(Card::AntiAir));
        assert_eq!(fitted.anti_air().mounts, 2);
        let full = [
            Card::MoreAntiAir,
            Card::MoreAntiAir,
            Card::HeavyAntiAir,
            Card::HeavyAntiAir,
            Card::QuickAntiAir,
            Card::QuickAntiAir,
            Card::QuickAntiAir,
        ]
        .into_iter()
        .fold(fitted, Fittings::with);
        let aa = full.anti_air();
        assert_eq!((aa.mounts, aa.damage), (AntiAir::MOST_MOUNTS, 3));
        assert!((aa.reload - AntiAir::NONE.reload / 1.75).abs() < 1e-5);
        for upgrade in [Card::MoreAntiAir, Card::HeavyAntiAir, Card::QuickAntiAir] {
            assert!(!full.can_fit(upgrade));
        }
    }
    #[test]
    fn turret_cards_upgrade_both_stations_in_order_and_leave_the_deck_when_spent() {
        let mut run = Run::default();
        for (stores, between, card, count) in [
            (3, 0, Card::Velocity, 2),
            (4, 0, Card::TripleTurrets, 3),
            // Three more fittings bring the main battery's hand round again.
            (5, 3, Card::QuadTurrets, 4),
        ] {
            for _ in 0..between {
                run.fittings = run.fittings.with(Card::Turbines);
            }
            run.tick(
                0.,
                Objectives {
                    crates: stores,
                    ..Default::default()
                },
                0,
            );
            if card != Card::QuadTurrets {
                assert!(!run.fittings.offers().contains(&Card::QuadTurrets));
            }
            assert!(run.fittings.offers().contains(&card));
            run.install(card);
            assert_eq!(run.fittings.turrets.count(), count);
        }
        for _ in 0..3 {
            let offers = run.fittings.offers();
            assert_eq!(offers.len(), 3);
            assert!(!offers.contains(&Card::TripleTurrets) && !offers.contains(&Card::QuadTurrets));
            run.fittings = run.fittings.with(Card::Loader);
        }
        run.tick(60., Objectives::default(), 0);
        run.extract();
        run.install(run.fittings.offers()[0]);
        assert!(run.leave_dock());
        assert!(run.depart(Stage::OuterRoad));
        assert_eq!(run.fittings.turrets, TurretLayout::Quad);
    }
    #[test]
    fn hits_land_on_modules_and_the_ship_is_lost_when_she_founders() {
        use shipvivor_shipyard::{Berth, State};
        let mut run = Run::default();
        let engine = Berth {
            module: 3,
            index: 2,
        };
        // Two hits on the engine module put her on reserve power.
        run.strike(17., 6);
        run.strike(17., 6);
        run.tick(1., Objectives::default(), 0);
        assert_eq!(run.phase(), Phase::Sailing);
        assert!(run.ship.on_reserve_power());
        assert_eq!(run.ship.propulsion(), 0.15);
        assert_eq!(run.hull(), 88);
        // The repair party goes to the engine unasked and has it turning again.
        for _ in 0..15 {
            run.tick(1., Objectives::default(), 0);
        }
        assert_ne!(run.ship.state(engine), State::Out);
        assert!(!run.ship.on_reserve_power() && run.ship.propulsion() > 0.15);
        // Four wrecked modules sink her with hull points still unspent.
        for aft in [2., 7.5, 12.5, 27.] {
            assert_eq!(run.phase(), Phase::Sailing);
            run.strike(aft, 18);
            run.tick(1., Objectives::default(), 0);
        }
        assert_eq!(run.phase(), Phase::Lost);
        assert!(run.hull() > 20);
    }
    #[test]
    fn armor_plate_thickens_only_as_far_as_her_beam_allows() {
        let most = |beam| {
            let mut fit = Fittings {
                beam,
                ..Fittings::default()
            };
            while fit.can_fit(Card::Armor) {
                fit = fit.with(Card::Armor);
            }
            (fit.armor, fit.protection(Protection::BREAKWATER).thickness)
        };
        assert_eq!(most(ShipBeam::Escort), (2, 7.));
        assert_eq!(most(ShipBeam::Cruiser), (4, 13.));
        assert_eq!(most(ShipBeam::Capital), (limits::ARMOR, 19.));
        // On her stock beam she can turn a destroyer's round, but not a cruiser's.
        let mut run = Run::default();
        while run.fittings.can_fit(Card::Armor) {
            run.fittings = run.fittings.with(Card::Armor);
        }
        run.refit_protection();
        let hull = run.hull();
        let bays = shipvivor_shipyard::Reach::Bays;
        run.strike_side(7.5, 2., 4, Some(5.9), bays);
        assert_eq!(run.hull(), hull);
        run.strike_side(7.5, -2., 4, Some(7.6), bays);
        assert_eq!(run.hull(), hull - 4);
    }
    #[test]
    fn the_dock_between_legs_leaves_a_battered_ship_short_of_whole() {
        let mut run = Run::default();
        run.strike(7.5, 18);
        run.strike(22.5, 18);
        run.tick(60., Objectives::default(), 45);
        let before = run.hull();
        run.extract();
        assert!(before + DOCK_REPAIR < MAX_HULL, "{before} hull");
        assert_eq!(run.hull(), before + DOCK_REPAIR);
        run.install(run.fittings.offers()[0]);
        assert!(run.leave_dock());
        assert!(run.depart(Stage::OuterRoad));
        assert_eq!(run.hull(), before + DOCK_REPAIR);
        assert!(run.ship.compartments().iter().all(|hold| !hold.wrecked()));
    }
    #[test]
    fn patrol_stars_unlock_only_earned_routes_and_fittings_travel() {
        for (goals, stars) in [
            (Objectives::default(), 0),
            (
                Objectives {
                    kills: 4,
                    crates: 1,
                    destroyed_drydock: false,
                },
                1,
            ),
            (
                Objectives {
                    kills: 4,
                    crates: 2,
                    destroyed_drydock: false,
                },
                2,
            ),
            (
                Objectives {
                    kills: 4,
                    crates: 3,
                    destroyed_drydock: true,
                },
                3,
            ),
        ] {
            for Exit {
                stage: dest,
                stars: required,
                ..
            } in EXITS
            {
                let mut r = Run::default();
                r.tick(60., goals, 50);
                while r.phase == Phase::Refit {
                    r.install(Card::Shells);
                }
                let hull = r.hull();
                r.extract();
                r.install(r.fittings.offers()[0]);
                assert!(r.leave_dock());
                assert_eq!(r.stars(), stars);
                assert_eq!(r.depart(dest), stars >= required);
                if stars >= required {
                    assert_eq!(r.stars(), 0);
                    // The dock makes good 40 points on any route, and 30 more on this one.
                    let docked = if dest == Stage::GreywaterSound {
                        70
                    } else {
                        40
                    };
                    let extension = if stars >= RESERVE_STARS { 18 } else { 0 };
                    assert_eq!(r.hull(), (hull + docked).min(100) + extension);
                    assert_eq!(r.previous_stars, stars);
                    if dest == Stage::PilotsCut {
                        assert_eq!(r.phase, Phase::Refit);
                        r.install(r.fittings.offers()[0]);
                    }
                    assert_eq!(
                        r.fittings.round().breach_force(),
                        2 + 2 * (goals.crates / 3)
                    );
                    assert_eq!(r.experience, u32::from(goals.crates % 3) * CRATE_XP);
                    r.tick(1000., Objectives::default(), 0);
                    assert_eq!(r.phase, Phase::Intercept(LegEnd::TimeExpired));
                } else {
                    assert_eq!(r.phase, Phase::Chart);
                }
            }
        }
    }
    #[test]
    fn multiple_levels_queue_one_choice_each_and_pause_even_at_deadline() {
        let mut r = Run::default();
        r.extract();
        assert_eq!(r.phase, Phase::Sailing);
        r.tick(
            600.,
            Objectives {
                crates: 8,
                ..Default::default()
            },
            0,
        );
        r.tick(10., Objectives::default(), 100);
        assert_eq!(r.hull(), 100);
        assert_eq!(r.elapsed, 600.);
        assert_eq!((r.level, r.experience, r.next_level_xp()), (3, 10, 50));
        assert_eq!(r.phase, Phase::Drydock(LegEnd::TimeExpired));
        assert_eq!(r.fittings_remaining(), 3);
        r.install(Card::Velocity);
        assert_eq!(r.phase, Phase::Drydock(LegEnd::TimeExpired));
        // A card the hand does not offer is not fitted.
        r.install(Card::Turbines);
        assert_eq!((r.fittings.turbines, r.fittings_remaining()), (0, 2));
        r.install(Card::Loader);
        r.install(Card::Turbines);
        assert!(matches!(r.phase, Phase::Drydock(_)) && r.leave_dock());
        assert_eq!(r.phase, Phase::Chart);
        assert!(r.fittings.round().speed() > Fittings::default().round().speed());
        assert!(r.fittings.speed_factor() > 1.);
        r.install(Card::Shells);
        assert_eq!(r.fittings.shells, 0);
    }
    #[test]
    fn clearing_on_the_deadline_banks_the_last_tick_and_awards_the_dock_once() {
        let mut r = Run::default();
        r.tick_sector(
            600.,
            Objectives {
                crates: 3,
                ..Default::default()
            },
            0,
            true,
        );
        assert_eq!(r.phase(), Phase::Drydock(LegEnd::BasesDestroyed));
        assert_eq!(r.fittings_remaining(), 3); // One level, one dock fitting, one clearance bonus.
        assert_eq!((r.stars(), r.level), (2, 2));
        r.tick_sector(10., Objectives::default(), 100, true);
        r.extract();
        r.install(Card::QuadTurrets);
        assert_eq!(r.fittings_remaining(), 3);
        assert!(!r.depart(Stage::OuterRoad));
        for _ in 0..3 {
            r.install(r.fittings.offers()[0]);
        }
        assert!(r.leave_dock());
        assert_eq!(r.phase(), Phase::Chart);
        assert!(!r.depart(Stage::PilotsCut));
        assert!(r.depart(Stage::OuterRoad));
        assert_eq!((r.elapsed, r.fittings_remaining()), (0., 0));
        r.tick_sector(1., Objectives::default(), 0, true);
        assert_eq!(r.phase(), Phase::Intercept(LegEnd::BasesDestroyed));
    }
    #[test]
    fn partial_experience_crosses_legs_and_restarts_cleanly() {
        let mut run = Run::default();
        let stores = |crates| Objectives {
            crates,
            ..Default::default()
        };
        // 50 lost, 12 mended by the crew over the minute, and 12 from each crate.
        run.tick(60., stores(2), 50);
        assert_eq!(
            (run.phase, run.hull(), run.experience),
            (Phase::Sailing, 86, 20)
        );
        run.extract();
        run.install(run.fittings.offers()[0]);
        assert!(run.leave_dock());
        assert!(run.depart(Stage::OuterRoad));
        run.tick(1., stores(1), 0);
        assert_eq!((run.phase, run.level, run.experience), (Phase::Refit, 2, 0));
        run.install(run.fittings.offers()[0]);
        run.tick(1., stores(3), 0);
        assert_eq!(
            (run.phase, run.experience, run.next_level_xp()),
            (Phase::Sailing, 30, 40)
        );
        run = Run::default();
        assert_eq!((run.level, run.experience, run.fittings.shells), (1, 0, 0));
    }

    #[test]
    fn sinking_overrides_salvage_and_restart_restores_the_starter_ship() {
        let mut r = Run::default();
        r.tick_sector(
            600.,
            Objectives {
                crates: 1,
                ..Default::default()
            },
            100,
            true,
        );
        assert_eq!(r.phase, Phase::Lost);
        r = Run::default();
        assert_eq!(r.hull(), 100);
        assert_eq!(r.elapsed, 0.);
        assert_eq!(r.stars(), 0);
    }
    #[test]
    fn the_regent_must_sink_before_she_escapes_and_breakwater_must_survive() {
        let finale = || {
            let mut run = Run::default();
            run.stage = Stage::OuterRoad;
            run.tick_sector(1., Objectives::default(), 0, true);
            assert!(run.engage_regent());
            assert!(!run.engage_regent());
            run
        };
        let mut escape = finale();
        escape.tick_sector(61., Objectives::default(), 0, true);
        escape.extract();
        assert_eq!(escape.phase(), Phase::Sailing);
        escape.tick(escape.remaining(), Objectives::default(), 0);
        assert_eq!(escape.phase(), Phase::Lost);
        assert!(escape.ship.afloat());

        let mut clutch = finale();
        clutch.tick_engagement(
            clutch.remaining(),
            Objectives::default(),
            0,
            Some(LegEnd::RegentSunk),
        );
        assert_eq!(clutch.phase(), Phase::Won(LegEnd::RegentSunk));
        let mut lost = finale();
        lost.tick_engagement(1., Objectives::default(), 1000, Some(LegEnd::RegentSunk));
        assert_eq!(lost.phase(), Phase::Lost);
        assert!(!lost.ship.afloat());
    }
}
