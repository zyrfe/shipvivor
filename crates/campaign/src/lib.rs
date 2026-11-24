//! Run progression: the mission clock, objectives, hull, fittings and routes of one run.
//! Timers advance only while the ship is sailing. Menus, pausing and the helm trial
//! belong to the application, not to the run.
mod battery;
mod workshop;
pub use battery::{CALIBRES, ShipBeam};
use shipvivor_artillery::{MainShell, Secondaries, TurretLayout};
use shipvivor_shipyard::{Component, Design, Frame, Module, Ship};
pub use workshop::{DeckModule, DockJob, MAX_GUN_STATIONS, MAX_SECTIONS_PER_KIND};

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
}

/// The time limit and star goals of one leg.
#[derive(Clone, Copy)]
pub struct Mission {
    pub name: &'static str,
    pub minutes: u16,
    pub kills: u16,
    pub crates: u16,
    pub gates: u16,
}

impl Stage {
    pub fn mission(self) -> Mission {
        let (name, minutes, kills, crates, gates) = match self {
            Self::CrownDeep => ("Crown Deep", 15, 0, 0, 0),
            Self::IronNarrows => ("The Iron Narrows", 10, 4, 2, 3),
            Self::OuterRoad => ("Outer Road", 12, 7, 3, 3),
            Self::GreywaterSound => ("Greywater Sound", 12, 6, 4, 4),
            Self::PilotsCut => ("Pilot's Cut", 15, 8, 3, 5),
        };
        Mission {
            name,
            minutes,
            kills,
            crates,
            gates,
        }
    }
}

pub const MAX_HULL: u16 = 100;
/// One star per mission goal.
pub const MAX_STARS: u8 = 3;
pub const RESERVE_STARS: u8 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockModule {
    Reserve,
    EngineRoom,
}
impl DockModule {
    pub fn name(self) -> &'static str {
        match self {
            Self::Reserve => "Reserve bay",
            Self::EngineRoom => "Auxiliary engine",
        }
    }

    fn build(self) -> Module {
        match self {
            Self::Reserve => Module::new(Frame::Straight, []),
            Self::EngineRoom => Module::new(Frame::Straight, [Component::Engine]),
        }
    }
}
/// Hull restored by each recovered crate.
pub const CRATE_REPAIR: u16 = 12;
/// Hull points of repair the dock gives between legs. A badly damaged ship sails with
/// whatever this does not reach.
pub const DOCK_REPAIR: u16 = 40;
/// Extraction opens after this many gate crossings or this long at sea.
pub const EXTRACT_GATES: u16 = 2;
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
}

/// Metres each rangefinder card adds to the secondaries' reach.
const WING_RANGE_STEP: f32 = 40.;

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

    /// Keep the first draft about ammunition. Later drafts also offer machinery, and every
    /// fourth draft is for the close-range battery.
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
        ]
        .into_iter()
        .map(u32::from)
        .sum::<u32>()
            + (self.turrets.count() - 2) as u32;
        // Each close-range hand leads with a different secondary card.
        let mut wing = [
            MoreSecondaries,
            TwinSecondaries,
            HeavySecondaries,
            SecondaryRange,
            QuickSecondaries,
        ];
        let turn = (installed / 4) as usize % 5;
        wing.rotate_left(turn);
        let [w0, w1, w2, w3, w4] = wing;
        let deck = match installed % 4 {
            0 => [
                ArmorPiercing,
                Caps,
                Shells,
                Velocity,
                Loader,
                Turbines,
                TripleTurrets,
                QuadTurrets,
                w0,
                w1,
                w2,
                w3,
                w4,
            ],
            1 => [
                Loader,
                TripleTurrets,
                QuadTurrets,
                Turbines,
                Caps,
                Velocity,
                Shells,
                ArmorPiercing,
                w0,
                w1,
                w2,
                w3,
                w4,
            ],
            2 => [
                Velocity,
                Shells,
                TripleTurrets,
                QuadTurrets,
                Turbines,
                Caps,
                Loader,
                ArmorPiercing,
                w0,
                w1,
                w2,
                w3,
                w4,
            ],
            _ => [
                w0,
                w1,
                w2,
                w3,
                w4,
                ArmorPiercing,
                Caps,
                Turbines,
                Loader,
                Velocity,
                Shells,
                TripleTurrets,
                QuadTurrets,
            ],
        };
        deck.into_iter()
            .filter(|card| self.can_fit(*card))
            .take(3)
            .collect()
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
    pub gates: u16,
}
pub const CRATE_XP: u32 = 10;

pub struct Run {
    phase: Phase,
    pub stage: Stage,
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
            + u8::from(self.objectives.gates >= goals.gates)
    }
    pub fn can_extract(&self) -> bool {
        self.stage != Stage::CrownDeep
            && (self.objectives.gates >= EXTRACT_GATES || self.elapsed >= EXTRACT_SECONDS)
    }
    pub fn extract(&mut self) {
        if self.phase == Phase::Sailing && self.can_extract() {
            self.finish_leg(LegEnd::Extracted);
        }
    }
    /// The installed drydock sections and their compartment indices, from bow to stern.
    pub fn dock_modules(&self) -> impl Iterator<Item = (usize, DockModule)> + '_ {
        let modules = self.ship.design().modules();
        modules
            .iter()
            .enumerate()
            .filter_map(|(i, m)| match (m.frame, m.fit.as_slice()) {
                (Frame::Straight, []) => Some((i, DockModule::Reserve)),
                (Frame::Straight, [Component::Engine]) => Some((i, DockModule::EngineRoom)),
                _ => None,
            })
    }

    pub fn added_length(&self) -> f32 {
        self.ship.design().length() - Design::BREAKWATER_LENGTH
    }

    fn finish_leg(&mut self, reason: LegEnd) {
        if self.stage == Stage::IronNarrows {
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
        self.objectives.gates = self.objectives.gates.saturating_add(gains.gates);
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
    /// A hit on the hull, `aft` metres behind the bow tip. It lands on the module there.
    pub fn strike(&mut self, aft: f32, damage: u16) {
        if self.phase == Phase::Sailing {
            self.ship.strike_hull(aft, f32::from(damage));
        }
    }
    /// Total surviving structure, including any drydock sections.
    pub fn hull(&self) -> u16 {
        self.ship.hull().round() as u16
    }
    pub fn hull_percent(&self) -> u16 {
        (self.ship.integrity() * 100.).round() as u16
    }
    pub fn install(&mut self, card: Card) {
        if !matches!(self.phase, Phase::Refit | Phase::Drydock(_))
            || !self.fittings.offers().contains(&card)
        {
            return;
        }
        self.fittings = self.fittings.with(card);
        self.pending_cards -= 1;
        if self.pending_cards == 0 {
            self.phase = if matches!(self.phase, Phase::Drydock(_)) {
                Phase::Chart
            } else {
                Phase::Sailing
            };
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
        for stars in 0..=MAX_STARS {
            for end in [
                LegEnd::TimeExpired,
                LegEnd::Extracted,
                LegEnd::BasesDestroyed,
            ] {
                let mut run = Run::default();
                let goals = run.stage.mission();
                let gains = Objectives {
                    kills: if stars >= 1 { goals.kills } else { 0 },
                    gates: if stars >= 2 { goals.gates } else { 0 },
                    crates: if stars >= 3 { goals.crates } else { 0 },
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
                while run.phase() == Phase::Drydock(end) {
                    run.install(run.fittings.offers()[0]);
                }
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
        // Every fourth hand is for the close-range battery.
        let fourth = Fittings {
            velocity: 3,
            ..stock
        };
        assert_eq!(
            fourth.offers(),
            [
                Card::MoreSecondaries,
                Card::TwinSecondaries,
                Card::HeavySecondaries
            ]
        );
        // The next close-range hand leads with a different card.
        let eighth = Fittings {
            velocity: 7,
            ..stock
        };
        assert_eq!(eighth.offers()[0], Card::TwinSecondaries);
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
    fn turret_cards_upgrade_both_stations_in_order_and_leave_the_deck_when_spent() {
        let mut run = Run::default();
        for (stores, card, count) in [
            (3, Card::Velocity, 2),
            (4, Card::TripleTurrets, 3),
            (5, Card::QuadTurrets, 4),
        ] {
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
        assert!(run.depart(Stage::OuterRoad));
        assert_eq!(run.hull(), before + DOCK_REPAIR);
        assert!(run.ship.compartments().iter().all(|hold| !hold.wrecked()));
    }
    #[test]
    fn gates_unlock_only_earned_routes_and_fittings_travel() {
        for (goals, stars) in [
            (Objectives::default(), 0),
            (
                Objectives {
                    kills: 4,
                    crates: 1,
                    gates: 1,
                },
                1,
            ),
            (
                Objectives {
                    kills: 4,
                    crates: 2,
                    gates: 2,
                },
                2,
            ),
            (
                Objectives {
                    kills: 4,
                    crates: 3,
                    gates: 3,
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
        r.install(Card::Turbines);
        r.install(Card::Velocity);
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
        assert_eq!((r.stars(), r.level), (1, 2));
        r.tick_sector(10., Objectives::default(), 100, true);
        r.extract();
        r.install(Card::QuadTurrets);
        assert_eq!(r.fittings_remaining(), 3);
        assert!(!r.depart(Stage::OuterRoad));
        for _ in 0..3 {
            r.install(r.fittings.offers()[0]);
        }
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
