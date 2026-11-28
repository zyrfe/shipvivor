//! Requisition and fitting rules at the between-leg yard.
use super::{CALIBRES, Card, DockModule, Fittings, Phase, Run, ShipBeam, battery::cradle, limits};
use shipvivor_artillery::TurretLayout;
use shipvivor_shipyard::{
    Component, Design, Frame, Gunhouse, MOST_TIER, Module, Sheet, StrikeCraft,
};

pub const MAX_SECTIONS_PER_KIND: usize = 2;

/// Floatplanes the plane sections of `design` have catapults for.
pub fn plane_room(design: &Design) -> usize {
    design.count(Component::Catapult) * usize::from(limits::PLANES_A_SECTION)
}

/// The most ranks of `card` a ship built to `design` with `fittings` has room for, for a
/// card that wants room: deck guns, gunhouses and AA by her deck, which a longer hull
/// adds to, and planes by her plane sections. Each gunhouse takes the deck edge two deck
/// guns would. `None` for every other card.
pub fn deck_ranks(card: Card, design: &Design, fittings: &Fittings) -> Option<u16> {
    let room = match card {
        Card::MoreSecondaries => design
            .deck_gun_room(fittings.gunhouses().per_side)
            .saturating_sub(shipvivor_artillery::Secondaries::STOCK.per_side),
        Card::SecondaryTurrets => {
            let deck = fittings.secondaries().per_side;
            // As many as leave the deck guns aboard room enough.
            (0..=design.gunhouse_room())
                .take_while(|&houses| design.deck_gun_room(houses) >= deck)
                .last()
                .unwrap_or(0)
                .max(fittings.gunhouses().per_side)
        }
        Card::MoreAntiAir => {
            design
                .aa_room()
                .saturating_sub(shipvivor_artillery::AntiAir::STOCK.mounts)
                / 2
        }
        Card::MorePlanes => plane_room(design).saturating_sub(StrikeCraft::STOCK.planes),
        _ => return None,
    };
    Some(room as u16)
}
/// The most main turrets an escort's hull or a heavy cruiser's carries. A battleship
/// carries more: see [`ShipBeam::most_turrets`].
pub const MAX_GUN_STATIONS: usize = 4;
/// The most main turrets any hull carries: a heavy battleship's.
pub const MOST_GUN_STATIONS: usize = ShipBeam::BROADEST.most_turrets();
/// The basin takes a hull this long and no longer: the most modules the broadest hull
/// may run to.
pub const MAX_LENGTH: f32 = 5. * ShipBeam::BROADEST.most_modules() as f32;
/// Yard points to rebuild her hull as the next class up. It is the biggest job the yard
/// does, and usually more than one leg pays for.
pub const HULL_STEP: u16 = 12;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeckModule {
    CentreBattery,
    WingBattery,
    /// Two full-size turrets abreast, on a battleship's hull.
    DoubleBattery,
    Reserve,
}
impl DeckModule {
    pub fn name(self) -> &'static str {
        match self {
            Self::CentreBattery => "Centre turret",
            Self::WingBattery => "Wing turrets",
            Self::DoubleBattery => "Double turrets",
            Self::Reserve => "Reserve bay",
        }
    }
    pub(crate) fn build(self) -> Module {
        match self {
            Self::CentreBattery => DockModule::Turret.build(),
            Self::WingBattery => Module::new(
                Frame::BatteryGallery,
                [Component::MainGun, Component::MainGun],
            ),
            Self::DoubleBattery => Module::new(
                Frame::DoubleBattery,
                [Component::MainGun, Component::MainGun],
            ),
            Self::Reserve => DockModule::Reserve.build(),
        }
    }
    /// The lightest class of hull with room for it.
    pub fn beam(self) -> ShipBeam {
        match self {
            Self::DoubleBattery => ShipBeam::Battleship,
            _ => ShipBeam::Escort,
        }
    }
}

/// Yard points to put a turret on a bay, split or merge one, or take one off. Double
/// turrets are two full-size gunhouses, and cost twice as much.
const EXCHANGE: u16 = 2;
/// Yard points for each barbette step a turret is raised.
const BARBETTE: u16 = 2;

/// Yard points to rebore a turret's guns up to `inches` from the calibre below it.
/// Each step up costs more than the last.
fn bore_price(inches: u16) -> u16 {
    CALIBRES
        .iter()
        .position(|&c| c == inches)
        .map_or(0, |rank| 1 + rank as u16)
}

/// Yard points to give a turret one more barrel, to reach `barrels`.
fn barrel_price(barrels: TurretLayout) -> u16 {
    match barrels {
        TurretLayout::Single => 0,
        TurretLayout::Twin => 2,
        TurretLayout::Triple => 3,
        TurretLayout::Quad => 4,
    }
}

impl DockModule {
    /// Yard points to build and splice in one.
    pub fn cost(self) -> u16 {
        match self {
            Self::Reserve | Self::Floatplane => 3,
            Self::Missiles | Self::EngineRoom => 4,
            Self::Turret => 5,
        }
    }
    /// The most sections of this kind one hull carries. Turrets are counted by
    /// the beam's [`ShipBeam::most_turrets`] instead, wing and double turrets included.
    pub fn most(self) -> usize {
        match self {
            Self::Reserve | Self::EngineRoom | Self::Missiles | Self::Floatplane => {
                MAX_SECTIONS_PER_KIND
            }
            Self::Turret => MAX_GUN_STATIONS,
        }
    }
    /// The lightest class of hull that has room for it. Missile magazines need a heavy
    /// cruiser's.
    pub fn beam(self) -> ShipBeam {
        match self {
            Self::Missiles => ShipBeam::HeavyCruiser,
            _ => ShipBeam::Escort,
        }
    }
}

/// Why the yard turns a job down, whatever points are left.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Refusal {
    /// The hull already carries the most of these it can.
    Full(usize),
    /// The hull must first be rebuilt to this class.
    Beam(ShipBeam),
    /// The hull must first be lengthened to this many metres.
    Length(u16),
    /// The ship must first carry another section of this kind.
    Section(DockModule),

    /// The basin takes no longer hull.
    Longest,
    /// She is the heaviest class of hull the yard builds.
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
    /// Rebuild her hull as the next class up: a heavy cruiser's, then a battleship's,
    /// then a heavy battleship's.
    WidenBeam,
    /// Rebore the main turrets on module `at` from `from` inches to `to`. A wing pair is
    /// rebored alike.
    Calibre {
        at: usize,
        from: u16,
        to: u16,
    },
    /// Refit the main turrets on module `at` with `to` barrels in place of `from`. A wing
    /// pair is refitted alike.
    Barrels {
        at: usize,
        from: TurretLayout,
        to: TurretLayout,
    },
    /// Stand the main turret on module `at` on a barbette `to` steps high in place of
    /// `from`, so it fires over a lower turret beside it.
    Barbette {
        at: usize,
        from: u8,
        to: u8,
    },
    Move {
        from: usize,
        to: usize,
    },
    Replace {
        at: usize,
        with: DeckModule,
    },
    /// Fit a set of torpedo tubes into module `at`, beside what it carries: in the bow
    /// they fire ahead, in the stern astern, and amidships they train to either beam.
    Tubes {
        at: usize,
    },
}

/// Yard points for a set of torpedo tubes.
pub const TUBES_PRICE: u16 = 4;
/// The refits of her torpedoes, offered where her tubes stand and dealt only while she
/// carries some.
pub const TORPEDO_FITTINGS: [Card; 3] = [
    Card::GuidedTorpedoes,
    Card::QuickTubes,
    Card::LongRunningTorpedoes,
];

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
        // Tubes go into any section of a built-up hull that has none yet, and the
        // torpedoes are refitted where tubes stand, in every set alike.
        let tubes = (module.frame != Frame::Boat && !module.fit.contains(&Component::TorpedoRack))
            .then_some(DockJob::Tubes { at });
        let torpedoes = module
            .fit
            .contains(&Component::TorpedoRack)
            .then_some(TORPEDO_FITTINGS.map(DockJob::Fitting))
            .into_iter()
            .flatten();
        if at == 0 || at == last {
            return torpedoes.chain(tubes).collect();
        }
        let mut jobs = Vec::new();
        if !self.module_locked(at) {
            // Double turrets are offered from a heavy cruiser's hull, to show what a
            // battleship's brings.
            let doubles =
                (self.beam_class() >= ShipBeam::HeavyCruiser).then_some(DeckModule::DoubleBattery);
            for with in [
                Some(DeckModule::CentreBattery),
                Some(DeckModule::WingBattery),
                doubles,
                Some(DeckModule::Reserve),
            ]
            .into_iter()
            .flatten()
            {
                if !module.same_kind(&self.rebuilt(at, with)) {
                    jobs.push(DockJob::Replace { at, with });
                }
            }
        }
        let carries = |part| module.fit.contains(&part);
        if carries(Component::MainGun) {
            // This bay's own turrets: their bore, their barrels and how high they stand.
            let bore = module.gunhouse.bore;
            jobs.extend(CALIBRES.map(|to| DockJob::Calibre { at, from: bore, to }));
            let from = module.gunhouse.barrels;
            jobs.extend(
                [from.more(), from.fewer()]
                    .into_iter()
                    .flatten()
                    .map(|to| DockJob::Barrels { at, from, to }),
            );
            let tier = module.gunhouse.tier;
            if self.overlooked(at) {
                jobs.push(DockJob::Barbette {
                    at,
                    from: tier,
                    to: tier + 1,
                });
            }
            if tier > 0 {
                jobs.push(DockJob::Barbette {
                    at,
                    from: tier,
                    to: tier - 1,
                });
            }
            jobs.push(DockJob::Fitting(if self.fittings.can_fit(Caps) {
                Caps
            } else {
                Velocity
            }));
            jobs.push(DockJob::Fitting(Loader));
            jobs.push(DockJob::Fitting(TrainingGear));
            // The light cruiser's long-range gunnery is worked from her turrets.
            jobs.extend(
                (crate::CRUISER_GUNNERY.into_iter())
                    .filter(|&card| self.hull.takes(card))
                    .map(DockJob::Fitting),
            );
        }
        if carries(Component::CloseGun) {
            jobs.extend(
                [
                    MoreSecondaries,
                    TwinSecondaries,
                    SecondaryRange,
                    QuickSecondaries,
                    SecondaryTurrets,
                ]
                .map(DockJob::Fitting),
            );
            // The AA guns stand on the bridge wings and abaft the funnel.
            jobs.extend([MoreAntiAir, HeavyAntiAir, QuickAntiAir].map(DockJob::Fitting));
        }
        if carries(Component::Engine) {
            jobs.push(DockJob::Fitting(Turbines));
            jobs.push(DockJob::Fitting(Hardening));
            jobs.push(DockJob::Fitting(Pumps));
        }
        // The strike craft are refitted on every rack and every plane alike.
        if carries(Component::MissileRack) {
            jobs.extend([MoreLaunchers, MoreMissiles].map(DockJob::Fitting));
        }
        if carries(Component::Catapult) {
            jobs.extend([MorePlanes, MoreBombs].map(DockJob::Fitting));
        }
        jobs.extend(torpedoes);
        jobs.extend(tubes);
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
        let turrets = module == DockModule::Turret;
        let fitted = if turrets {
            design.count(Component::MainGun)
        } else {
            self.dock_modules()
                .filter(|(_, kind)| *kind == module)
                .count()
        };
        let most = if turrets {
            self.beam_class().most_turrets()
        } else {
            module.most()
        };
        if fitted >= most {
            // A heavier hull carries more turrets.
            let broader = ShipBeam::ALL
                .into_iter()
                .find(|&beam| turrets && beam > self.beam_class() && beam.most_turrets() > fitted);
            Some(broader.map_or(Refusal::Full(most), Refusal::Beam))
        } else if self.beam_class() < module.beam() {
            Some(Refusal::Beam(module.beam()))
        } else if design.modules().len() >= self.beam_class().most_modules()
            || design.length() + Frame::Straight.spec().length * design.scale > MAX_LENGTH
        {
            // A longer hull waits for a heavier class, up to the longest the basin takes.
            let roomier = ShipBeam::ALL.into_iter().find(|&beam| {
                beam > self.beam_class() && beam.most_modules() > design.modules().len()
            });
            Some(roomier.map_or(Refusal::Longest, Refusal::Beam))
        } else {
            None
        }
    }

    /// Why the hull cannot be rebuilt as the next class up, if it cannot.
    pub fn widen_refusal(&self) -> Option<Refusal> {
        let next = self.beam_class().next()?;
        (self.ship.design().length() < next.min_length())
            .then_some(Refusal::Length(next.min_length() as u16))
    }

    /// Why the hull cannot take another rank of armor plate until she is a heavier
    /// class, if that is why. On a hull no heavier class out-plates, her last rank is
    /// simply her last.
    pub fn armor_refusal(&self) -> Option<Refusal> {
        let class = self.beam_class();
        if self.fittings.armor < class.most_armor() {
            return None;
        }
        ShipBeam::ALL
            .into_iter()
            .find(|&beam| beam > class && beam.most_armor() > class.most_armor())
            .map(Refusal::Beam)
    }

    /// Deck guns a side her deck has room for, beside the gunhouses she carries: two on
    /// each open section no gunhouse stands on, and on a battleship's hull and up a
    /// casemate under each, so a longer hull carries more.
    pub fn wing_room(&self) -> usize {
        (self.ship.design()).deck_gun_room(self.fittings.gunhouses().per_side)
    }

    /// Secondary gunhouses a side her deck has room for while her deck guns keep theirs.
    pub fn gunhouse_room(&self) -> usize {
        deck_ranks(Card::SecondaryTurrets, self.ship.design(), &self.fittings)
            .map_or(0, usize::from)
    }

    /// AA mounts her deck has room for: the three pairs she was built with and a pair on
    /// each open section, so a longer hull carries more.
    pub fn aa_room(&self) -> usize {
        self.ship.design().aa_room()
    }

    /// Floatplanes her plane sections have catapults for.
    pub fn plane_room(&self) -> usize {
        plane_room(self.ship.design())
    }

    /// Her deck has room for another rank of `card`. Only more guns or planes want room:
    /// secondaries and AA a longer hull makes, and planes another plane section. Her
    /// torpedoes are refitted only while she carries tubes.
    pub fn has_room(&self, card: Card) -> bool {
        if let Some((base, _)) = card.legend() {
            return self.has_room(base);
        }
        match card {
            _ if !self.hull.takes(card) => false,
            _ if TORPEDO_FITTINGS.contains(&card) => {
                self.ship.design().count(Component::TorpedoRack) > 0
            }
            Card::MoreSecondaries => self.fittings.secondaries().per_side < self.wing_room(),
            // Where her class carries them at all: see `beam_wanted`.
            Card::SecondaryTurrets if self.fittings.gunhouse_bore().is_some() => {
                self.fittings.gunhouses().per_side < self.gunhouse_room()
            }
            Card::MoreAntiAir => self.fittings.anti_air().mounts + 2 <= self.aa_room(),
            Card::MorePlanes => self.fittings.strike_craft().planes < self.plane_room(),
            _ => true,
        }
    }

    /// Ranks of `card` fitted, out of the most she can take, as [`Fittings::ranks`]
    /// counts them, but out of the most her deck has room for where that is fewer.
    pub fn ranks(&self, card: Card) -> Option<(u16, u16)> {
        let (fitted, most) = self.fittings.ranks(card)?;
        let most = deck_ranks(card, self.ship.design(), &self.fittings)
            .map_or(most, |room| most.min(room));
        Some((fitted.min(most), most))
    }

    /// Why she has no room for another rank of `card`, while the yard could still make
    /// some: another plane section for a plane, or a longer hull for guns. `None` once
    /// nothing she could be given would make room.
    fn room_refusal(&self, card: Card) -> Option<Refusal> {
        // Tubes are fitted section by section, where the torpedo work is then offered,
        // and no yard gives another hull the cruiser's gunnery.
        if TORPEDO_FITTINGS.contains(&card) || crate::CRUISER_GUNNERY.contains(&card) {
            return None;
        }
        if card == Card::MorePlanes {
            return match self.stock_refusal(DockModule::Floatplane) {
                None => Some(Refusal::Section(DockModule::Floatplane)),
                Some(Refusal::Beam(beam)) => Some(Refusal::Beam(beam)),
                Some(_) => None,
            };
        }
        self.wing_refusal()
    }

    /// Why her deck has no room for more guns, while a section that gives her some could
    /// still be spliced in, or could on a broader beam. `None` once she is as long as the
    /// basin takes her.
    fn wing_refusal(&self) -> Option<Refusal> {
        let roomy = [
            DockModule::Reserve,
            DockModule::EngineRoom,
            DockModule::Turret,
        ];
        let refusals = roomy.map(|module| self.stock_refusal(module));
        if refusals.iter().any(Option::is_none) {
            let design = self.ship.design();
            let longer = design.length() + Frame::Straight.spec().length * design.scale;
            return Some(Refusal::Length(longer.round() as u16));
        }
        refusals
            .into_iter()
            .flatten()
            .find(|why| matches!(why, Refusal::Beam(_)))
    }

    /// Why the yard turns this job down, whatever points are left. `None` for jobs
    /// that are simply not legal in this place.
    pub fn refusal(&self, job: DockJob) -> Option<Refusal> {
        match job {
            DockJob::Insert { module, .. } => self.stock_refusal(module),
            DockJob::WidenBeam if self.beam_class().next().is_none() => Some(Refusal::Widest),
            DockJob::WidenBeam => self.widen_refusal(),
            DockJob::Fitting(Card::Armor) => self.armor_refusal(),
            DockJob::Fitting(card) if !self.has_room(card) => self.room_refusal(card),
            DockJob::Fitting(Card::SecondaryTurrets) | DockJob::Replace { .. }
                if self.beam_class() < self.beam_wanted(job) =>
            {
                Some(Refusal::Beam(self.beam_wanted(job)))
            }
            _ => None,
        }
    }

    /// The lightest class of hull this job can be done on.
    fn beam_wanted(&self, job: DockJob) -> ShipBeam {
        match job {
            // Secondary gunhouses, on the first hull that carries them.
            DockJob::Fitting(Card::SecondaryTurrets) => ShipBeam::ALL
                .into_iter()
                .find(|beam| beam.secondary_bore().is_some())
                .unwrap_or(ShipBeam::BROADEST),
            DockJob::Replace { with, .. } => with.beam(),
            DockJob::Insert { module, .. } => module.beam(),
            _ => ShipBeam::Escort,
        }
    }

    /// The centreline turret on module `at` is masked by something that raising it as
    /// far as it goes would let it fire over: a turret ahead or astern standing as high
    /// as itself, or the funnels. The turret need not be beside it: a wing gallery
    /// between them leaves the centreline clear, so a turret beyond it masks just the
    /// same. A bridge masks it at any height. Double turrets stand across the
    /// centreline, so a centreline turret beside them is overlooked too, but they are
    /// never raised themselves.
    pub fn overlooked(&self, at: usize) -> bool {
        let design = self.ship.design();
        let Some(own) = design
            .modules()
            .get(at)
            .filter(|m| !m.frame.abreast() && m.fit.contains(&Component::MainGun))
        else {
            return false;
        };
        let tier = own.gunhouse.tier;
        tier < MOST_TIER && design.masking(at, tier) != design.masking(at, MOST_TIER)
    }

    /// The biggest bore the turrets on module `at` could carry with these barrels, on
    /// her hull as it is now.
    pub fn cradle_at(&self, at: usize, barrels: TurretLayout) -> u16 {
        cradle(self.beam_class(), self.gallery(at), barrels)
    }

    /// The lightest class of hull on which the turrets on module `at` could carry `bore`
    /// with these barrels, if any could.
    pub fn beam_for(&self, at: usize, barrels: TurretLayout, bore: u16) -> Option<ShipBeam> {
        let paired = self.gallery(at);
        ShipBeam::ALL
            .into_iter()
            .find(|&beam| cradle(beam, paired, barrels) >= bore)
    }

    /// Module `at` is a wing gallery, whose turrets stand in pairs.
    fn gallery(&self, at: usize) -> bool {
        self.ship
            .design()
            .modules()
            .get(at)
            .is_some_and(|m| m.frame == Frame::BatteryGallery)
    }
}

impl DockJob {
    /// The price in yard points. The bigger the change to her, the more it costs.
    pub fn cost(self) -> u16 {
        match self {
            Self::Insert { module, .. } => module.cost(),
            Self::Move { .. } => 0,
            Self::WidenBeam => HULL_STEP,
            Self::Fitting(Card::TripleTurrets) => 3,
            Self::Fitting(Card::QuadTurrets) => 4,
            // Another light gun bolted to the deck is the smallest job the yard does, so a
            // lone point left over always has somewhere to go while there is deck to take one.
            Self::Fitting(Card::MoreSecondaries | Card::MoreAntiAir) => 1,
            // A rank of armor is half a step of plate, so it costs half a fitting.
            Self::Fitting(Card::Armor) => 1,
            // Each barrel added is paid for. Taking barrels off is free.
            Self::Barrels { from, to, .. } => TurretLayout::ALL
                .into_iter()
                .filter(|&b| b > from && b <= to)
                .map(barrel_price)
                .sum(),
            Self::Barbette { from, to, .. } => BARBETTE * u16::from(to.saturating_sub(from)),
            // Each step of bore is paid for. A smaller bore is free.
            Self::Calibre { from, to, .. } => CALIBRES
                .into_iter()
                .filter(|&c| c > from && c <= to)
                .map(bore_price)
                .sum(),
            Self::Replace {
                with: DeckModule::DoubleBattery,
                ..
            } => 2 * EXCHANGE,
            Self::Replace { .. } => EXCHANGE,
            Self::Tubes { .. } => TUBES_PRICE,
            Self::Fitting(_) => 2,
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

    /// The next class of hull up, once she is long enough to be rebuilt as one.
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
            Frame::DoubleBattery => DeckModule::DoubleBattery.name(),
            _ if m.fit.contains(&Component::Director) => "Bridge",
            _ if self.ship.design().machinery() == Some(at) => "Machinery space",
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

    /// The yard points this job leaves, if they cover it. Test mode's yard charges
    /// nothing, so there every job is covered and leaves what there was.
    pub fn points_after(&self, job: DockJob) -> Option<u16> {
        if self.exercise {
            return Some(self.points);
        }
        self.points.checked_sub(job.cost())
    }

    /// This job buys an empty bay with so few points to spare that no turret can follow it
    /// onto the bay.
    pub fn leaves_bay_empty(&self, job: DockJob) -> bool {
        matches!(
            job,
            DockJob::Insert {
                module: DockModule::Reserve,
                ..
            }
        ) && !self.exercise
            && self.points_after(job).is_some_and(|left| left < EXCHANGE)
    }

    /// Sail from the yard once every level-up hand owed is taken: for the chart after
    /// every patrol but the last, and to intercept the Regent after that. Points left over
    /// carry to the next yard. She stays in dock until she is sailed. Test mode's yard
    /// sails her straight into the sea it stands before, on a leg begun afresh.
    pub fn leave_dock(&mut self) -> bool {
        let Phase::Drydock(reason) = self.phase else {
            return false;
        };
        if self.pending_cards > 0 {
            return false;
        }
        if self.exercise {
            self.new_leg();
            self.cargo_carried = 0.;
            self.elapsed = 0.;
            self.phase = Phase::Sailing;
            return true;
        }
        self.phase = if self.stage.is_last_patrol() {
            Phase::Intercept(reason)
        } else {
            Phase::Chart
        };
        true
    }

    /// Bow, stern, propulsion and the bridge stay aboard. All interior modules may move.
    /// Module `at` rebuilt as `with`. Any tubes fitted into it stay aboard.
    fn rebuilt(&self, at: usize, with: DeckModule) -> Module {
        let mut module = with.build();
        let tubes = self
            .ship
            .design()
            .modules()
            .get(at)
            .is_some_and(|old| old.fit.contains(&Component::TorpedoRack));
        if tubes {
            module.fit.push(Component::TorpedoRack);
        }
        module
    }

    pub fn module_locked(&self, at: usize) -> bool {
        self.ship.design().modules().get(at).is_none_or(|module| {
            !Frame::MIDSHIPS.contains(&module.frame)
                || module
                    .fit
                    .iter()
                    .any(|c| c.vital() || *c == Component::Director)
        })
    }

    /// A legal trial plan, independent of the points left to pay for it.
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
                if design.count(Component::MainGun) > self.beam_class().most_turrets() {
                    return None;
                }
            }
            DockJob::WidenBeam => design.widen(self.next_beam()?.metres()),
            DockJob::Calibre { at, from, to } => {
                let module = design.modules().get(at)?;
                let house = module.gunhouse;
                if !module.fit.contains(&Component::MainGun)
                    || house.bore != from
                    || to == from
                    || !CALIBRES.contains(&to)
                    || to > self.cradle_at(at, house.barrels)
                {
                    return None;
                }
                design.rearm(at, Gunhouse { bore: to, ..house }).ok()?;
            }
            DockJob::Barrels { at, from, to } => {
                let module = design.modules().get(at)?;
                let house = module.gunhouse;
                // A heavier cradle must still carry the bore the turret has.
                if !module.fit.contains(&Component::MainGun)
                    || house.barrels != from
                    || to == from
                    || (to > from && self.cradle_at(at, to) < house.bore)
                {
                    return None;
                }
                design
                    .rearm(
                        at,
                        Gunhouse {
                            barrels: to,
                            ..house
                        },
                    )
                    .ok()?;
            }
            DockJob::Barbette { at, from, to } => {
                let module = design.modules().get(at)?;
                let centreline =
                    !module.frame.abreast() && module.fit.contains(&Component::MainGun);
                let house = module.gunhouse;
                if !centreline
                    || house.tier != from
                    || to == from
                    || to > MOST_TIER
                    || (to > from && !self.overlooked(at))
                {
                    return None;
                }
                design.rearm(at, Gunhouse { tier: to, ..house }).ok()?;
            }
            DockJob::Fitting(card) => {
                if !self.fittings.can_fit(card) || !self.has_room(card) {
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
                if self.module_locked(at) || self.beam_class() < with.beam() {
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
                let replacement = self.rebuilt(at, with);
                if design.modules()[at].same_kind(&replacement) {
                    return None;
                }
                design.replace_module(at, replacement).ok()?;
                let most = self.beam_class().most_turrets();
                if !(1..=most).contains(&design.count(Component::MainGun)) {
                    return None;
                }
            }
            DockJob::Tubes { at } => {
                let module = design.modules().get(at)?;
                if module.fit.contains(&Component::TorpedoRack) {
                    return None;
                }
                design.fit(at, Component::TorpedoRack).ok()?;
            }
        }
        Some(design)
    }

    pub fn projected_fittings(&self, job: DockJob) -> Fittings {
        let Some(design) = self.projected_design(job) else {
            return self.fittings;
        };
        let mut fit = self.fittings;
        if let DockJob::Fitting(card) = job {
            fit = fit.with(card);
        }
        let beam = if job == DockJob::WidenBeam {
            self.next_beam().unwrap()
        } else {
            self.beam_class()
        };
        fit.on_deck(beam, &design.gun_stations())
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
    /// The yard opens once the level-up hands owed are taken, and its work is paid for in
    /// points, except in test mode, where it is only counted: see [`Run::spent`].
    pub fn refit_in_dock(&mut self, job: DockJob) -> bool {
        if !matches!(self.phase, Phase::Drydock(_))
            || self.pending_cards > 0
            || self.points_after(job).is_none()
            || !job.fits(self)
        {
            return false;
        }
        let fit = self.projected_fittings(job);
        let applied = match job {
            DockJob::Insert { at, module } => self.ship.insert_module(at, module.build()),
            DockJob::Move { from, to } => self.ship.move_module(from, to),
            DockJob::Replace { at, with } => self.ship.replace_module(at, self.rebuilt(at, with)),
            DockJob::Tubes { at } => self.ship.fit_tubes(at).map(|_| ()),
            DockJob::Barrels { at, .. }
            | DockJob::Barbette { at, .. }
            | DockJob::Calibre { at, .. } => {
                let planned = self.projected_design(job).expect("a legal job");
                self.ship.rearm(at, planned.modules()[at].gunhouse)
            }
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
        if !self.exercise {
            self.points -= job.cost();
        }
        self.spent = self.spent.saturating_add(job.cost());
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Objectives;

    #[test]
    fn the_yard_previews_without_spending_and_only_installs_affordable_work() {
        let mut run = Run::default();
        let insert = |run: &Run, module| DockJob::Insert {
            at: run.default_gap(),
            module,
        };
        assert!(!run.refit_in_dock(insert(&run, DockModule::Reserve)));
        run.tick_sector(
            PAST_THE_CLOCK,
            Objectives {
                kills: 4,
                destroyed_drydock: true,
                crates: 0,
                yards: 4,
                ..Default::default()
            },
            0,
            true,
        );
        // Two stars and four wrecked yards pay eight points, and the clock nothing.
        assert_eq!(run.points(), 8);
        let installed = run.ship.design().length();
        let sheet = run.projected_sheet(DockModule::Reserve);
        assert_eq!(sheet.length, installed + 5.);
        assert_eq!(run.ship.design().length(), installed);
        assert_eq!(run.points(), 8);
        assert!(run.refit_in_dock(insert(&run, DockModule::Reserve)));
        assert_eq!(run.ship.design().sheet(), sheet);
        assert!(run.refit_in_dock(insert(&run, DockModule::Reserve)));
        assert_eq!(run.points(), 2);
        // Two bays are all she carries, and two points will not build an engine room.
        assert!(!run.refit_in_dock(insert(&run, DockModule::Reserve)));
        assert!(!run.refit_in_dock(insert(&run, DockModule::EngineRoom)));
        assert!(!run.refit_in_dock(DockJob::Fitting(Card::TripleTurrets)));
        assert!(run.refit_in_dock(DockJob::Fitting(Card::Loader)));
        assert_eq!(run.points(), 0);
        assert!(run.leave_dock());
        assert!(run.depart(run.moderate_passage()));
        assert_eq!(run.ship.design().length(), 40.);
        assert_eq!(run.fittings.loader, 1);
    }

    /// Seconds into a leg that leave any patrol's clock behind, bases and all, so the
    /// yard's budget is its stars' and yards' alone.
    const PAST_THE_CLOCK: f32 = 900.;

    /// A cleared sector: the raider star, the drydock star and four wrecked yards, with the
    /// stores star too for two crates or more. It is cleared past the clock.
    fn clear_sector(crates: u16) -> Run {
        let mut run = Run::default();
        run.tick_sector(
            PAST_THE_CLOCK,
            Objectives {
                kills: 4,
                destroyed_drydock: true,
                crates,
                yards: 4,
                ..Default::default()
            },
            0,
            true,
        );
        run
    }

    /// A run in the yard at her built length with `points` to spend.
    fn docked(points: u16) -> Run {
        let mut run = Run::default();
        run.clear();
        run.points = points;
        run
    }

    #[test]
    fn torpedo_cards_stack_to_their_limits_and_come_only_with_tubes() {
        use crate::StartingHull;
        // The light cruiser carries no tubes, so no hand deals her torpedo cards and no
        // section of hers offers the work.
        let mut cruiser = docked(40);
        for card in TORPEDO_FITTINGS {
            assert!(!cruiser.has_room(card));
            assert!(!DockJob::Fitting(card).fits(&cruiser));
            assert!(!cruiser.refit_in_dock(DockJob::Fitting(card)));
        }
        let torpedo_hand = |run: &Run| {
            (0..400).any(|key| {
                let hand = run.fittings.deal_where(key, |card| run.has_room(card));
                hand.iter().any(|card| TORPEDO_FITTINGS.contains(card))
            })
        };
        let destroyer = Run::default().with_hull(StartingHull::HeavyDestroyer);
        assert!(!torpedo_hand(&cruiser));
        assert!(torpedo_hand(&destroyer));
        let sections = cruiser.ship.design().modules().len();
        assert!((0..sections).all(|at| {
            (cruiser.section_jobs(at).iter())
                .all(|job| !matches!(job, DockJob::Fitting(c) if TORPEDO_FITTINGS.contains(c)))
        }));
        // Once tubes are fitted, they are offered where the tubes stand.
        assert!(cruiser.refit_in_dock(DockJob::Tubes { at: 0 }));
        assert!(TORPEDO_FITTINGS.iter().all(|&card| cruiser.has_room(card)));

        // The heavy destroyer carries tubes in the bow, amidships and the stern, and each
        // set offers its torpedoes' work.
        let mut run = docked(80).with_hull(StartingHull::HeavyDestroyer);
        run.points = 80;
        for mount in run.ship.design().tube_mounts() {
            let jobs = run.section_jobs(mount.berth.module);
            for card in TORPEDO_FITTINGS {
                assert!(jobs.contains(&DockJob::Fitting(card)), "{card:?}");
            }
        }
        // Each stacks to its limit, and the battle's torpedoes follow it.
        for (card, most) in [
            (Card::GuidedTorpedoes, limits::GUIDANCE),
            (Card::QuickTubes, limits::QUICK_TUBES),
            (Card::LongRunningTorpedoes, limits::LONG_RUN),
        ] {
            for rank in 0..most {
                assert_eq!(run.ranks(card), Some((rank, most)));
                assert!(run.refit_in_dock(DockJob::Fitting(card)), "{card:?} {rank}");
            }
            assert_eq!(run.ranks(card), Some((most, most)));
            assert!(!run.refit_in_dock(DockJob::Fitting(card)));
            assert_eq!(run.fittings.with(card), run.fittings);
        }
        let torpedoes = run.fittings.torpedoes();
        assert_eq!(torpedoes.guidance, 4);
        assert!((torpedoes.reload - crate::drilled(3)).abs() < 1e-5);
        assert!((torpedoes.run - 1.4).abs() < 1e-5);
    }

    #[test]
    fn the_cruisers_gunnery_cards_come_only_to_her_line_and_stack_to_their_limits() {
        use crate::{CRUISER_GUNNERY, Rarity, StartingHull};
        let gunnery_hand = |run: &Run| {
            (0..600).any(|key| {
                let hand = run.fittings.deal_where(key, |card| run.has_room(card));
                hand.iter().any(|card| CRUISER_GUNNERY.contains(card))
            })
        };
        let offered = |run: &Run, card: Card| {
            let sections = run.ship.design().modules().len();
            (0..sections).any(|at| run.section_jobs(at).contains(&DockJob::Fitting(card)))
        };
        // The heavy destroyer is never dealt them, nor offered them in the yard.
        let mut destroyer = docked(40).with_hull(StartingHull::HeavyDestroyer);
        destroyer.points = 40;
        assert!(!gunnery_hand(&destroyer));
        for card in CRUISER_GUNNERY {
            assert!(!destroyer.has_room(card));
            assert!(!offered(&destroyer, card));
            assert!(!destroyer.refit_in_dock(DockJob::Fitting(card)));
            assert_eq!(destroyer.refusal(DockJob::Fitting(card)), None);
        }
        // The light cruiser is dealt them, and her turrets offer them.
        let mut cruiser = docked(80);
        assert!(gunnery_hand(&cruiser));
        let turret = (cruiser.ship.design().modules().iter())
            .position(|m| m.fit.contains(&Component::MainGun))
            .unwrap();
        for card in CRUISER_GUNNERY {
            assert!(
                cruiser
                    .section_jobs(turret)
                    .contains(&DockJob::Fitting(card))
            );
        }
        for (card, most) in [
            (Card::SuppressingFire, limits::SUPPRESSION),
            (Card::LongBaseRangefinders, limits::RANGEFINDERS),
            (Card::PlungingFire, limits::PLUNGING),
        ] {
            for rank in 0..most {
                assert_eq!(cruiser.ranks(card), Some((rank, most)));
                assert!(
                    cruiser.refit_in_dock(DockJob::Fitting(card)),
                    "{card:?} {rank}"
                );
            }
            assert_eq!(cruiser.ranks(card), Some((most, most)));
            assert!(!cruiser.refit_in_dock(DockJob::Fitting(card)));
            assert_eq!(cruiser.fittings.with(card), cruiser.fittings);
        }
        // They reach her fire control, on the cruiser's tighter scatter; the destroyer
        // lays wider than stock.
        let fire = cruiser.fire_control();
        assert_eq!(
            (fire.suppression, fire.rangefinders, fire.plunging),
            (3, 3, 3)
        );
        assert!(fire.dispersion < 1. && destroyer.fire_control().dispersion > 1.);
        // Their rarity climbs with the ranks fitted, from where the deck sets them.
        let fresh = Run::default().fittings;
        assert_eq!(
            fresh.rarity(Card::LongBaseRangefinders),
            Some(Rarity::Common)
        );
        assert_eq!(fresh.rarity(Card::SuppressingFire), Some(Rarity::Uncommon));
        assert_eq!(fresh.rarity(Card::PlungingFire), Some(Rarity::Uncommon));
        let ranged = fresh.with(Card::LongBaseRangefinders);
        assert_eq!(
            ranged.rarity(Card::LongBaseRangefinders),
            Some(Rarity::Uncommon)
        );
    }

    #[test]
    fn tubes_go_into_any_section_and_stay_through_a_rebuild() {
        use shipvivor_shipyard::TubeAim;
        let mut run = docked(20);
        let last = run.ship.design().modules().len() - 1;
        // The bow and the stern offer tubes and nothing else.
        assert_eq!(run.section_jobs(0), [DockJob::Tubes { at: 0 }]);
        assert_eq!(run.section_jobs(last), [DockJob::Tubes { at: last }]);
        assert!(run.refit_in_dock(DockJob::Tubes { at: 0 }));
        assert_eq!(run.points(), 20 - TUBES_PRICE);
        // Then the work on her torpedoes, and no more tubes.
        assert_eq!(run.section_jobs(0), TORPEDO_FITTINGS.map(DockJob::Fitting));
        assert!(!run.refit_in_dock(DockJob::Tubes { at: 0 }));
        // Amidships, over the after turret, and kept when the turret makes way for a bay.
        let at = last - 1;
        assert!(run.section_jobs(at).contains(&DockJob::Tubes { at }));
        assert!(run.refit_in_dock(DockJob::Tubes { at }));
        let bay = DockJob::Replace {
            at,
            with: DeckModule::Reserve,
        };
        assert!(run.refit_in_dock(bay));
        let mounts: Vec<_> = run
            .ship
            .design()
            .tube_mounts()
            .iter()
            .map(|t| (t.berth.module, t.aim))
            .collect();
        assert_eq!(mounts, [(0, TubeAim::Ahead), (at, TubeAim::Beam)]);
        // The new tubes are sound, and moving the section takes them along.
        assert!(run.ship.ready(run.ship.design().tube_mounts()[1].berth));
    }

    /// Splice a reserve bay in ahead of the after turret.
    fn lengthen(run: &mut Run) {
        let bay = DockJob::Insert {
            at: run.default_gap(),
            module: DockModule::Reserve,
        };
        assert!(run.refit_in_dock(bay));
    }

    /// The first reserve bay aboard.
    fn reserve_bay(run: &Run) -> usize {
        run.dock_modules()
            .find(|(_, k)| *k == DockModule::Reserve)
            .unwrap()
            .0
    }

    #[test]
    fn the_yard_prices_its_work_by_how_much_it_changes_her() {
        use DockModule::*;
        assert_eq!(
            [Reserve, Floatplane, Missiles, EngineRoom, Turret].map(DockModule::cost),
            [3, 3, 4, 4, 5]
        );
        for (job, price) in [
            // A new class of hull is the dearest thing the yard builds.
            (DockJob::WidenBeam, 12),
            (
                DockJob::Calibre {
                    at: 1,
                    from: 4,
                    to: 6,
                },
                2,
            ),
            // Each step of bore costs more than the last: three more to eight inches.
            (
                DockJob::Calibre {
                    at: 1,
                    from: 4,
                    to: 8,
                },
                5,
            ),
            (
                DockJob::Calibre {
                    at: 1,
                    from: 18,
                    to: 4,
                },
                0,
            ),
            (
                DockJob::Barrels {
                    at: 1,
                    from: TurretLayout::Single,
                    to: TurretLayout::Twin,
                },
                2,
            ),
            (
                DockJob::Barrels {
                    at: 1,
                    from: TurretLayout::Twin,
                    to: TurretLayout::Quad,
                },
                7,
            ),
            (
                DockJob::Barrels {
                    at: 1,
                    from: TurretLayout::Quad,
                    to: TurretLayout::Single,
                },
                0,
            ),
            (
                DockJob::Barbette {
                    at: 1,
                    from: 0,
                    to: 1,
                },
                2,
            ),
            (
                DockJob::Barbette {
                    at: 1,
                    from: 1,
                    to: 0,
                },
                0,
            ),
            (DockJob::Move { from: 1, to: 2 }, 0),
            (
                DockJob::Replace {
                    at: 1,
                    with: DeckModule::WingBattery,
                },
                2,
            ),
            (DockJob::Fitting(Card::Loader), 2),
            (DockJob::Fitting(Card::Armor), 1),
        ] {
            assert_eq!(job.cost(), price, "{job:?}");
        }
        // The price comes off her points, and work they do not cover is turned down.
        let mut run = docked(3);
        assert!(!run.refit_in_dock(DockJob::Insert {
            at: 1,
            module: EngineRoom
        }));
        assert!(rebore(&mut run, 1, 6));
        assert_eq!(run.points(), 1);
        assert!(run.refit_in_dock(DockJob::Move { from: 1, to: 2 }));
        assert_eq!(run.points(), 1);
        assert!(!run.refit_in_dock(DockJob::Fitting(Card::Loader)));
        assert_eq!(run.points_after(DockJob::Fitting(Card::Loader)), None);
    }

    #[test]
    fn the_hull_is_rebuilt_a_class_at_a_time_and_only_once_she_is_long_enough_for_it() {
        // At her built length she cannot be rebuilt, whatever she can pay.
        let mut run = docked(60);
        assert_eq!(run.ship.design().length(), 30.);
        assert_eq!(run.widen_refusal(), Some(Refusal::Length(35)));
        assert!(!DockJob::WidenBeam.fits(&run) && !run.refit_in_dock(DockJob::WidenBeam));

        // Two bays bring her to 40 m, long enough for a heavy cruiser's hull and then a
        // battleship's.
        lengthen(&mut run);
        lengthen(&mut run);
        assert_eq!((run.ship.design().length(), run.points()), (40., 54));
        assert_eq!(
            run.projected_design(DockJob::WidenBeam).unwrap().beam(),
            ShipBeam::HeavyCruiser.metres()
        );
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        assert_eq!(run.beam_class(), ShipBeam::HeavyCruiser);
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        assert_eq!(run.beam_class(), ShipBeam::Battleship);
        // Each class costs twelve points.
        assert_eq!(run.points(), 30);
        // A heavy battleship's hull wants 50 m under it: two more sections.
        assert_eq!(run.refusal(DockJob::WidenBeam), Some(Refusal::Length(50)));
        for _ in 0..2 {
            assert!(run.refit_in_dock(DockJob::Insert {
                at: run.default_gap(),
                module: DockModule::Floatplane,
            }));
        }
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        assert_eq!(run.beam_class(), ShipBeam::HeavyBattleship);
        assert!((run.ship.design().beam() - 11.5).abs() < 1e-3);
        assert_eq!(run.points(), 12);
        // An ultra heavy battleship's wants 60 m.
        assert_eq!(run.refusal(DockJob::WidenBeam), Some(Refusal::Length(60)));
        // Every step keeps her more than four beams long.
        let design = run.ship.design();
        assert!(design.length() / design.beam() >= 4.2);
        assert_eq!(run.fittings.capacity(), 18);
        let gun = run.ship.design().gun_stations()[0].berth.module;
        assert!(
            DockJob::Calibre {
                at: gun,
                from: 4,
                to: 18
            }
            .fits(&run)
        );
        // She stays in dock until she is sailed, points to spare or not.
        assert!(matches!(run.phase(), Phase::Drydock(_)));
        assert!(run.leave_dock());
        assert_eq!(run.phase(), Phase::Chart);
    }

    #[test]
    fn thicker_armor_waits_for_a_broader_beam_which_waits_for_a_longer_hull() {
        // At her built length and beam she carries eight ranks of armor and no more.
        let mut run = docked(20);
        run.fittings.armor = ShipBeam::Escort.most_armor();
        let armor = DockJob::Fitting(Card::Armor);
        assert_eq!(
            run.refusal(armor),
            Some(Refusal::Beam(ShipBeam::HeavyCruiser))
        );
        assert!(!run.refit_in_dock(armor));
        assert_eq!(run.refusal(DockJob::WidenBeam), Some(Refusal::Length(35)));

        // Lengthened she can be widened, and widened she takes the next rank.
        lengthen(&mut run);
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        assert_eq!(run.refusal(armor), None);
        assert!(run.refit_in_dock(armor));
        // On the light cruiser's three quarters of an inch of stock plate.
        assert_eq!(run.ship.design().protection().thickness, 7.5);
    }

    #[test]
    fn each_beam_allows_so_many_modules_and_a_longer_hull_waits_for_a_broader_one() {
        let mut run = docked(60);
        // An escort's beam runs to eight modules: two bays past her built six.
        lengthen(&mut run);
        lengthen(&mut run);
        assert_eq!(run.ship.design().modules().len(), 8);
        let bay = DockJob::Insert {
            at: run.default_gap(),
            module: DockModule::EngineRoom,
        };
        assert_eq!(
            run.stock_refusal(DockModule::EngineRoom),
            Some(Refusal::Beam(ShipBeam::HeavyCruiser))
        );
        assert!(!run.refit_in_dock(bay));
        // A heavy cruiser's hull may run to eleven, then a battleship's to fourteen.
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        for module in [
            DockModule::EngineRoom,
            DockModule::Floatplane,
            DockModule::Turret,
        ] {
            assert_eq!(run.stock_refusal(module), None, "{module:?}");
        }
        for module in [
            DockModule::EngineRoom,
            DockModule::Floatplane,
            DockModule::Missiles,
        ] {
            assert!(run.refit_in_dock(DockJob::Insert {
                at: run.default_gap(),
                module
            }));
        }
        assert_eq!(run.ship.design().modules().len(), 11);
        assert_eq!(
            run.stock_refusal(DockModule::Missiles),
            Some(Refusal::Beam(ShipBeam::Battleship))
        );
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        assert_eq!(run.stock_refusal(DockModule::Missiles), None);
        assert_eq!(MAX_LENGTH, 70.);
        // Fourteen fill the basin, so no heavier hull runs longer.
        for module in [
            DockModule::Turret,
            DockModule::Turret,
            DockModule::Floatplane,
        ] {
            assert!(run.refit_in_dock(DockJob::Insert {
                at: run.default_gap(),
                module
            }));
        }
        assert_eq!(run.ship.design().modules().len(), 14);
        assert_eq!(
            run.stock_refusal(DockModule::Missiles),
            Some(Refusal::Longest)
        );
    }

    #[test]
    fn a_section_bought_in_the_yard_lengthens_her_enough_for_a_hull_she_must_save_for() {
        // Three stars and four yards pay ten points.
        let mut run = clear_sector(2);
        assert_eq!((run.ship.design().length(), run.points()), (30., 10));
        assert_eq!(run.widen_refusal(), Some(Refusal::Length(35)));
        let bay = DockJob::Insert {
            at: 1,
            module: DockModule::Reserve,
        };
        assert!(run.refit_in_dock(bay));
        // She is long enough now, but one leg's points do not pay for a new hull.
        assert_eq!((run.widen_refusal(), run.points()), (None, 7));
        assert!(DockJob::WidenBeam.fits(&run) && !run.refit_in_dock(DockJob::WidenBeam));
        // Banked, with the next leg's, they do.
        run.points += 8;
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        assert_eq!(run.beam_class(), ShipBeam::HeavyCruiser);
        assert_eq!(run.widen_refusal(), Some(Refusal::Length(40)));
        assert!(run.refit_in_dock(bay));
        assert_eq!(run.ship.design().length(), 40.);
        assert_eq!((run.widen_refusal(), run.points()), (None, 0));
        assert!(!run.refit_in_dock(DockJob::WidenBeam));
    }

    /// Rebore the turrets on module `at` to `to` inches.
    fn rebore(run: &mut Run, at: usize, to: u16) -> bool {
        let from = run.ship.design().modules()[at].gunhouse.bore;
        run.refit_in_dock(DockJob::Calibre { at, from, to })
    }

    /// Refit the turrets on module `at` with `to` barrels.
    fn rearm(run: &mut Run, at: usize, to: TurretLayout) -> bool {
        let from = run.ship.design().modules()[at].gunhouse.barrels;
        run.refit_in_dock(DockJob::Barrels { at, from, to })
    }

    #[test]
    fn each_turret_takes_its_barrels_one_at_a_time_and_carries_what_its_cradle_allows() {
        let mut run = docked(30);
        let guns = run.ship.design().gun_stations();
        let (fore, aft) = (guns[0].berth.module, guns[1].berth.module);
        // She is built with single guns, and each turret is refitted on its own.
        assert!(
            guns.iter()
                .all(|g| g.gunhouse.barrels == TurretLayout::Single)
        );
        let jobs = run.section_jobs(fore);
        assert!(jobs.contains(&DockJob::Barrels {
            at: fore,
            from: TurretLayout::Single,
            to: TurretLayout::Twin
        }));
        assert!(!jobs.iter().any(|j| matches!(
            j,
            DockJob::Barrels {
                to: TurretLayout::Triple | TurretLayout::Quad,
                ..
            }
        )));
        // Straight to a quad is refused: one barrel at a time.
        let leap = DockJob::Barrels {
            at: fore,
            from: TurretLayout::Single,
            to: TurretLayout::Quad,
        };
        assert!(!leap.fits(&run) || leap.cost() == 9);
        for to in [TurretLayout::Twin, TurretLayout::Triple, TurretLayout::Quad] {
            assert!(rearm(&mut run, fore, to));
        }
        assert_eq!(run.points(), 30 - 9);
        let guns = run.ship.design().gun_stations();
        assert_eq!(
            (guns[0].gunhouse.barrels, guns[1].gunhouse.barrels),
            (TurretLayout::Quad, TurretLayout::Single)
        );
        // On an escort's beam the quad carries four inches, and the single turret
        // still sets the bore she can be given.
        assert_eq!(run.fittings.capacity(), 6);
        assert!(
            !rebore(&mut run, fore, 6),
            "a quad cradle carries four inches"
        );
        assert!(rebore(&mut run, aft, 6));
        let guns = run.ship.design().gun_stations();
        assert_eq!(run.fittings.for_station(&guns[0]).calibre(), 4);
        assert_eq!(run.fittings.for_station(&guns[1]).calibre(), 6);
        // A six-inch turret takes no more barrels than its cradle carries at that bore.
        assert!(rearm(&mut run, aft, TurretLayout::Twin));
        assert!(!rearm(&mut run, aft, TurretLayout::Triple));
        assert!(rebore(&mut run, aft, 4));
        // Taking barrels off is free.
        let left = run.points();
        assert!(rearm(&mut run, fore, TurretLayout::Twin));
        assert_eq!(run.points(), left);
        // The refit sails with her, turret by turret.
        assert!(run.leave_dock());
        let guns = run.ship.design().gun_stations();
        assert!(
            guns.iter()
                .all(|g| g.gunhouse.barrels == TurretLayout::Twin)
        );
    }

    #[test]
    fn a_turret_behind_another_is_raised_to_fire_over_it() {
        let mut run = docked(30);
        // Two centre turrets side by side.
        lengthen(&mut run);
        let reserve = reserve_bay(&run);
        assert!(run.refit_in_dock(DockJob::Replace {
            at: reserve,
            with: DeckModule::CentreBattery
        }));
        let guns = run.ship.design().gun_stations();
        let pair = guns
            .windows(2)
            .find(|w| w[1].berth.module == w[0].berth.module + 1)
            .map(|w| (w[0], w[1]));
        let Some((low, high)) = pair else {
            panic!("two turrets side by side");
        };
        // Each masks the other across the forty degrees beside its end.
        let blind = |g: &shipvivor_shipyard::GunStation| {
            (0..360)
                .filter(|d| !g.bears((*d as f32).to_radians()))
                .count()
        };
        let before = blind(&high);
        let at = high.berth.module;
        assert!(run.overlooked(at) && run.overlooked(low.berth.module));
        let raise = DockJob::Barbette { at, from: 0, to: 1 };
        assert!(run.section_jobs(at).contains(&raise));
        assert!(run.refit_in_dock(raise));
        let raised = run
            .ship
            .design()
            .gun_stations()
            .into_iter()
            .find(|g| g.berth.module == at)
            .unwrap();
        assert_eq!(
            (raised.gunhouse.tier, raised.lift()),
            (1, shipvivor_shipyard::BARBETTE)
        );
        assert!(blind(&raised) < before, "it fires over the lower turret");
        // A second step stands it over the funnels beyond that turret too, and none
        // goes higher.
        assert!(run.overlooked(at));
        assert!(run.refit_in_dock(DockJob::Barbette { at, from: 1, to: 2 }));
        assert!(!run.overlooked(at));
        // Lowering it again is free.
        let left = run.points();
        assert!(run.refit_in_dock(DockJob::Barbette { at, from: 2, to: 0 }));
        assert_eq!(run.points(), left);
    }

    #[test]
    fn the_after_turret_is_raised_two_steps_to_fire_over_the_funnels() {
        let mut run = docked(30);
        let at = run.ship.design().modules().len() - 2;
        let modules = run.ship.design().modules();
        assert!(modules[at].fit.contains(&Component::MainGun));
        assert!(modules[at - 1].fit.contains(&Component::Engine));
        // Thirty-five degrees off the bow, across the funnels.
        let over = |run: &Run| {
            run.ship
                .design()
                .gun_stations()
                .into_iter()
                .find(|g| g.berth.module == at)
                .unwrap()
                .bears(35_f32.to_radians())
        };
        let raise = |from| DockJob::Barbette {
            at,
            from,
            to: from + 1,
        };
        assert!(!over(&run), "the funnels mask it");
        assert!(run.section_jobs(at).contains(&raise(0)));
        assert!(run.refit_in_dock(raise(0)));
        assert!(!over(&run), "one step stands it level with the funnel tops");
        assert!(run.section_jobs(at).contains(&raise(1)));
        assert!(run.refit_in_dock(raise(1)));
        assert!(over(&run), "two steps fire over the funnels");
        // Only the bridge masks it now, and no barbette clears a bridge.
        assert!(!run.overlooked(at));
    }

    #[test]
    fn a_turret_is_raised_over_the_turret_beyond_a_wing_gallery() {
        let mut run = docked(80);
        // Forward turret, wing gallery, centre turret, bridge: nothing beside the centre
        // turret stands on the centreline, but the forward turret masks it all the same.
        for _ in 0..2 {
            assert!(run.refit_in_dock(DockJob::Insert {
                at: 2,
                module: DockModule::Reserve,
            }));
        }
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        assert!(run.refit_in_dock(DockJob::Replace {
            at: 2,
            with: DeckModule::WingBattery,
        }));
        // A fifth turret wants a capital beam.
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        assert!(run.refit_in_dock(DockJob::Replace {
            at: 3,
            with: DeckModule::CentreBattery,
        }));
        let modules = run.ship.design().modules();
        assert!(modules[1].fit.contains(&Component::MainGun));
        assert_eq!(modules[2].frame, Frame::BatteryGallery);
        assert!(modules[4].fit.contains(&Component::Director));
        let station = |run: &Run| {
            run.ship
                .design()
                .gun_stations()
                .into_iter()
                .find(|g| g.berth.module == 3)
                .unwrap()
        };
        let ahead = |g: &shipvivor_shipyard::GunStation| g.bears(0.);
        assert!(
            !ahead(&station(&run)),
            "the forward turret masks dead ahead"
        );
        let raise = DockJob::Barbette {
            at: 3,
            from: 0,
            to: 1,
        };
        assert!(run.overlooked(3) && run.section_jobs(3).contains(&raise));
        assert!(run.refit_in_dock(raise));
        assert!(ahead(&station(&run)), "it fires over the forward turret");
        // Only the bridge masks it now, and no barbette clears a bridge.
        assert!(!run.overlooked(3));
    }

    #[test]
    fn a_wing_gallery_carries_smaller_guns_beside_an_eighteen_inch_centre_battery() {
        let mut run = docked(60);
        let replace = |at, with| DockJob::Replace { at, with };
        lengthen(&mut run);
        lengthen(&mut run);
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        let aft = run.ship.design().gun_stations()[1].berth.module;
        assert!(rebore(&mut run, aft, 18));
        assert!(run.refit_in_dock(replace(1, DeckModule::WingBattery)));
        // A wing gallery's cradles stop at eight inches.
        assert!(!rebore(&mut run, 1, 12));
        assert!(rebore(&mut run, 1, 8));
        // Six for the bays, twenty-four for the two classes of hull, twenty for the
        // eighteen-inch bore, two for the wings and five for their eight-inch bore.
        assert_eq!(run.points(), 60 - 57);
        let ship = run.ship.design();
        let guns = ship.gun_stations();
        assert_eq!(guns.len(), 3);
        assert_eq!(run.fittings.capacity(), 18);
        assert_eq!(run.fittings.calibre(), 18);
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
    fn a_lean_dock_can_still_add_a_turret_by_splitting_one_into_wings() {
        let mut run = Run::default();
        // Late, with only the drydock goal met: the clock pays nothing.
        run.tick_sector(PAST_THE_CLOCK, Objectives::default(), 0, true);
        // One star pays two points: enough to split a turret, not to buy a bay.
        assert_eq!((run.points(), run.main_turrets()), (2, 2));
        assert_eq!(run.turret_bay(), Some(1));
        let split = DockJob::Replace {
            at: 1,
            with: DeckModule::WingBattery,
        };
        assert!(split.adds_turret(&run) && run.points_after(split) == Some(0));
        let bay = DockJob::Insert {
            at: 1,
            module: DockModule::Reserve,
        };
        assert_eq!(run.points_after(bay), None);
        assert!(!run.refit_in_dock(bay));
        assert!(run.refit_in_dock(split));
        assert_eq!((run.main_turrets(), run.points()), (3, 0));
        assert!(run.leave_dock());
        assert_eq!(run.phase(), Phase::Chart);
    }

    #[test]
    fn five_points_buy_a_reserve_bay_and_a_centre_turret_to_stand_on_it() {
        let mut run = Run::default();
        // The raider star, the drydock star and the one yard that earned it, past the
        // clock.
        run.tick(
            PAST_THE_CLOCK,
            Objectives {
                kills: 4,
                yards: 1,
                ..Default::default()
            },
            0,
        );
        run.clear();
        assert_eq!((run.points(), run.main_turrets()), (5, 2));
        let bay = DockJob::Insert {
            at: run.default_gap(),
            module: DockModule::Reserve,
        };
        // A point fewer and the bay would sail empty.
        run.points = 4;
        assert!(run.leaves_bay_empty(bay));
        run.points = 5;
        assert!(!run.leaves_bay_empty(bay) && run.refit_in_dock(bay));
        let at = run.turret_bay().unwrap();
        assert_eq!(run.module_name(at), DeckModule::Reserve.name());
        let add = DockJob::Replace {
            at,
            with: DeckModule::CentreBattery,
        };
        assert!(add.adds_turret(&run) && run.points_after(add) == Some(0));
        assert!(run.refit_in_dock(add));
        assert!(run.leave_dock());
        assert!(run.depart(run.moderate_passage()));
        assert_eq!(run.ship.design().gun_stations().len(), 3);
        assert!(run.ship.design().gun_stations().iter().all(|g| !g.paired()));
    }

    #[test]
    fn each_section_offers_its_own_work_and_never_what_it_already_carries() {
        use DeckModule::*;
        let mut run = clear_sector(0);
        lengthen(&mut run);
        let last = run.ship.design().modules().len() - 1;
        // Bow and stern are fixed, and take only tubes. The hull's own work is offered for
        // the whole ship.
        for end in [0, last] {
            assert_eq!(run.section_jobs(end), [DockJob::Tubes { at: end }]);
        }
        assert_eq!(
            run.hull_jobs(),
            [DockJob::WidenBeam, DockJob::Fitting(Card::Armor)]
        );
        let reserve = reserve_bay(&run);
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
        assert!(!bay.iter().any(|j| matches!(j, DockJob::Calibre { .. })));

        let gun = run.section_jobs(1);
        assert!(gun.contains(&DockJob::Calibre {
            at: 1,
            from: 4,
            to: 18
        }));
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
    fn a_bought_reserve_bay_is_offered_for_a_turret_before_any_split() {
        let mut run = clear_sector(0);
        lengthen(&mut run);
        let reserve = reserve_bay(&run);
        assert_eq!(run.turret_bay(), Some(reserve));
        // Moving it costs nothing.
        let shift = DockJob::Move {
            from: reserve,
            to: reserve - 1,
        };
        assert_eq!(run.points_after(shift), Some(run.points()));
    }

    #[test]
    fn invalid_layout_edits_leave_the_real_ship_and_all_its_points_untouched() {
        let mut run = clear_sector(0);
        lengthen(&mut run);
        let guns = run.ship.design().gun_stations();
        let replace = |at, with| DockJob::Replace { at, with };
        let reserve = reserve_bay(&run);
        for at in [0, 2, 3, run.ship.design().modules().len() - 1, usize::MAX] {
            assert!(!run.refit_in_dock(replace(at, DeckModule::WingBattery)));
        }
        for (from, to) in [(2, 0), (1, 1), (usize::MAX, 1)] {
            assert!(!run.refit_in_dock(DockJob::Move { from, to }));
        }
        assert_eq!(run.points(), 5);
        assert_eq!(run.ship.design().gun_stations(), guns);
        assert!(run.refit_in_dock(replace(reserve, DeckModule::WingBattery)));
        assert!(!run.refit_in_dock(replace(1, DeckModule::WingBattery)));
        assert_eq!(run.ship.design().gun_stations().len(), 4);

        let mut run = clear_sector(0);
        lengthen(&mut run);
        let rear = run.ship.design().gun_stations()[1].berth.module;
        assert!(run.refit_in_dock(replace(1, DeckModule::Reserve)));
        assert!(!run.refit_in_dock(replace(rear, DeckModule::Reserve)));
        assert_eq!(run.points(), 3);
    }

    #[test]
    fn a_battleship_takes_double_turrets_abreast_and_six_turrets_in_all() {
        use shipvivor_shipyard::Mounting;
        let mut run = docked(80);
        lengthen(&mut run);
        lengthen(&mut run);
        let double = |at| DockJob::Replace {
            at,
            with: DeckModule::DoubleBattery,
        };
        let bay = reserve_bay(&run);
        // On an escort's hull the yard does not offer them.
        assert!(!run.section_jobs(bay).contains(&double(bay)));
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        // A heavy cruiser's hull shows them, waiting on a battleship's.
        assert!(run.section_jobs(bay).contains(&double(bay)));
        assert!(!double(bay).fits(&run) && !run.refit_in_dock(double(bay)));
        assert_eq!(
            run.refusal(double(bay)),
            Some(Refusal::Beam(ShipBeam::Battleship))
        );
        // Four turrets fill a heavy cruiser, and a battleship takes more.
        assert!(run.refit_in_dock(DockJob::Replace {
            at: bay,
            with: DeckModule::WingBattery,
        }));
        assert_eq!(run.main_turrets(), 4);
        assert_eq!(
            run.stock_refusal(DockModule::Turret),
            Some(Refusal::Beam(ShipBeam::Battleship))
        );
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        // Doubling the wing pair costs twice an exchange.
        let left = run.points();
        assert!(run.refit_in_dock(double(bay)));
        assert_eq!(run.points(), left - 4);
        let guns = run.ship.design().gun_stations();
        let pair: Vec<_> = guns.iter().filter(|g| g.berth.module == bay).collect();
        assert_eq!(pair.len(), 2);
        assert!(
            pair.iter()
                .all(|g| g.mounting == Mounting::Double && !g.paired())
        );
        assert!(pair[0].abeam < 0. && pair[1].abeam > 0.);
        // They take a centreline turret's bore, which a wing gallery's cradles cannot.
        assert!(rebore(&mut run, bay, 18));
        assert_eq!(run.fittings.for_station(pair[0]).calibre(), 4);
        let guns = run.ship.design().gun_stations();
        let big = guns.iter().find(|g| g.berth.module == bay).unwrap();
        assert_eq!(run.fittings.for_station(big).calibre(), 18);
        // A second pair on the other bay brings her to six, the most a battleship carries.
        let other = reserve_bay(&run);
        assert!(run.refit_in_dock(double(other)));
        assert_eq!(run.main_turrets(), 6);
        // Double turrets are never raised, but the centreline turret abaft them can be.
        let aft = run.ship.design().modules().len() - 2;
        assert_eq!(aft, other + 1);
        assert!(!run.overlooked(other) && run.overlooked(aft));
        // A heavy battleship carries two more.
        assert_eq!(
            run.stock_refusal(DockModule::Turret),
            Some(Refusal::Beam(ShipBeam::HeavyBattleship))
        );
        assert!(!run.refit_in_dock(DockJob::Replace {
            at: 1,
            with: DeckModule::DoubleBattery,
        }));
    }

    #[test]
    fn the_racks_and_the_plane_section_take_more_launchers_missiles_planes_and_bombs() {
        use Card::*;
        use shipvivor_shipyard::StrikeCraft;
        let mut run = docked(60);
        lengthen(&mut run);
        // A missile magazine wants a cruiser's beam.
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        for module in [DockModule::Missiles, DockModule::Floatplane] {
            let at = run.default_gap();
            assert!(run.refit_in_dock(DockJob::Insert { at, module }));
        }
        let find = |run: &Run, kind| run.dock_modules().find(|(_, k)| *k == kind).unwrap().0;
        let (rack, catapult) = (
            find(&run, DockModule::Missiles),
            find(&run, DockModule::Floatplane),
        );
        let offers = |at, card| run.section_jobs(at).contains(&DockJob::Fitting(card));
        assert!(offers(rack, MoreLaunchers) && offers(rack, MoreMissiles));
        assert!(offers(catapult, MorePlanes) && offers(catapult, MoreBombs));
        assert!(!offers(rack, MorePlanes) && !offers(catapult, MoreMissiles));
        // They are yard work, never dealt in a level-up hand.
        let cards = [MoreLaunchers, MoreMissiles, MorePlanes, MoreBombs];
        assert!(cards.iter().all(|card| card.rarity().is_none()));
        assert_eq!(run.fittings.strike_craft(), StrikeCraft::STOCK);
        for card in cards {
            let (_, most) = run.ranks(card).unwrap();
            for _ in 0..most {
                assert!(run.refit_in_dock(DockJob::Fitting(card)), "{card:?}");
            }
            assert!(!DockJob::Fitting(card).fits(&run), "{card:?}");
        }
        // One plane section launches three planes. More want a second section.
        let planes = DockJob::Fitting(MorePlanes);
        assert_eq!(
            run.refusal(planes),
            Some(Refusal::Section(DockModule::Floatplane))
        );
        assert_eq!(
            run.fittings.strike_craft(),
            StrikeCraft {
                launchers: 4,
                salvo: StrikeCraft::STOCK.salvo + 4,
                planes: 3,
                bombs: 5,
            }
        );
        let at = run.default_gap();
        let module = DockModule::Floatplane;
        assert!(run.refit_in_dock(DockJob::Insert { at, module }));
        assert_eq!(run.refusal(planes), None);
        assert_eq!(run.ranks(MorePlanes), Some((2, 5)));
    }

    #[test]
    fn secondary_gunhouses_come_a_pair_at_a_time_on_a_battleship_and_grow_with_her_class() {
        let mut run = docked(250);
        let bridge = 2;
        assert_eq!(run.module_name(bridge), "Bridge");
        let turrets = DockJob::Fitting(Card::SecondaryTurrets);
        assert!(run.section_jobs(bridge).contains(&turrets));
        assert!(!turrets.fits(&run));
        assert_eq!(
            run.refusal(turrets),
            Some(Refusal::Beam(ShipBeam::Battleship))
        );
        lengthen(&mut run);
        lengthen(&mut run);
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        assert_eq!(
            run.refusal(turrets),
            Some(Refusal::Beam(ShipBeam::Battleship))
        );
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        assert_eq!(run.refusal(turrets), None);
        let (deck, room) = (run.fittings.secondaries(), run.wing_room());
        assert!(run.refit_in_dock(turrets));
        let four = run.fittings.gunhouses();
        assert_eq!((four.per_side, four.bore, four.barrels), (1, 4, 2));
        // A gunhouse takes the deck edge two deck guns would, and the deck guns stay.
        assert_eq!(run.fittings.secondaries(), deck);
        assert!(run.wing_room() < room, "{} of {room}", run.wing_room());
        // It hits harder and reaches farther than any deck gun, and loads more slowly.
        assert!(four.damage > deck.damage && four.range > deck.range);
        assert!(four.reload > deck.reload);
        assert!(run.refit_in_dock(turrets));
        assert_eq!(run.fittings.gunhouses().per_side, 2);
        // Six-inch on a heavy battleship's hull, which wants 50 m.
        let insert = |run: &mut Run, module| {
            let at = run.default_gap();
            assert!(run.refit_in_dock(DockJob::Insert { at, module }));
        };
        for _ in 0..2 {
            insert(&mut run, DockModule::Floatplane);
        }
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        let six = run.fittings.gunhouses();
        assert_eq!((six.per_side, six.bore), (2, 6));
        assert!(six.damage > four.damage && six.range > four.range);
        // Eight-inch on an ultra heavy battleship's, which wants 60 m.
        for _ in 0..2 {
            insert(&mut run, DockModule::EngineRoom);
        }
        assert!(run.refit_in_dock(DockJob::WidenBeam));
        assert_eq!(run.refusal(DockJob::WidenBeam), Some(Refusal::Widest));
        let design = run.ship.design();
        assert!(design.length() / design.beam() >= 4.2);
        assert_eq!(run.beam_class(), ShipBeam::UltraHeavyBattleship);
        let eight = run.fittings.gunhouses();
        assert_eq!((eight.per_side, eight.bore), (2, 8));
        assert!(eight.damage > six.damage && eight.range > six.range);
        assert!(eight.reload > six.reload);
    }

    #[test]
    fn deck_guns_start_light_and_grow_no_heavier_than_three_inches() {
        use crate::StartingHull;
        let cruiser = StartingHull::LightCruiser.fittings();
        let destroyer = StartingHull::HeavyDestroyer.fittings();
        assert_eq!(cruiser.secondaries().bore, 1.);
        assert_eq!(destroyer.secondaries().bore, 2.);
        let heavier = |mut f: Fittings, ranks: u16| {
            f.wing_shells = ranks;
            f.secondaries().bore
        };
        assert_eq!(heavier(cruiser, 2), 2.);
        assert_eq!(heavier(cruiser, limits::WING_SHELLS), 3.);
        assert_eq!(heavier(destroyer, 1), 2.5);
        assert_eq!(heavier(destroyer, limits::WING_SHELLS), 3.);
    }

    #[test]
    fn a_heavy_battleship_carries_eight_turrets_and_heavier_cradles() {
        use ShipBeam::*;
        assert_eq!(
            ShipBeam::ALL.map(ShipBeam::most_turrets),
            [4, 4, 6, 8, MOST_GUN_STATIONS]
        );
        assert_eq!(MOST_GUN_STATIONS, 8);
        // Three eighteen-inch barrels a turret, where a battleship carries two, and
        // twelve-inch guns in her wing galleries.
        assert_eq!(cradle(Battleship, false, TurretLayout::Triple), 15);
        assert_eq!(cradle(HeavyBattleship, false, TurretLayout::Triple), 18);
        assert_eq!(cradle(HeavyBattleship, false, TurretLayout::Quad), 15);
        assert_eq!(cradle(HeavyBattleship, true, TurretLayout::Twin), 12);
        // Every class carries at least what the class below it does.
        for pair in ShipBeam::ALL.windows(2) {
            for paired in [false, true] {
                for barrels in TurretLayout::ALL {
                    assert!(cradle(pair[1], paired, barrels) >= cradle(pair[0], paired, barrels));
                }
            }
            assert!(pair[1].metres() > pair[0].metres());
            assert!(pair[1].min_length() / pair[1].metres() >= 4.2);
            assert!(pair[1].most_armor() >= pair[0].most_armor());
        }
        // She plates no thicker than a battleship, so once that is on, armor is done.
        let mut run = docked(0);
        run.fittings.beam = Battleship;
        run.fittings.armor = Battleship.most_armor();
        assert_eq!(run.refusal(DockJob::Fitting(Card::Armor)), None);
        assert!(!DockJob::Fitting(Card::Armor).fits(&run));
    }

    #[test]
    fn a_longer_hull_makes_room_for_more_aa_tubs() {
        let mut run = docked(40);
        let more = DockJob::Fitting(Card::MoreAntiAir);
        // Her stock hull has room for four pairs, two more than she is built with.
        assert_eq!(run.aa_room(), 8);
        assert_eq!(run.ranks(Card::MoreAntiAir), Some((0, 3)));
        for _ in 0..3 {
            assert!(run.refit_in_dock(more));
        }
        assert_eq!(run.fittings.anti_air().mounts, 8);
        assert_eq!(run.refusal(more), Some(Refusal::Length(35)));
        for hand in 0..200 {
            run.hands = hand;
            assert!(!run.offers().contains(&Card::MoreAntiAir));
        }
        lengthen(&mut run);
        lengthen(&mut run);
        // The first bay gives its place to the after funnel's pair. The second has one,
        // and moves the after turret's clear of it.
        assert_eq!(run.aa_room(), 12);
        assert!(run.refit_in_dock(more) && run.refit_in_dock(more));
        assert_eq!(run.fittings.anti_air().mounts, 12);
    }

    #[test]
    fn a_full_deck_waits_for_a_longer_hull_before_mounting_more_secondaries() {
        let mut run = docked(40);
        let more = DockJob::Fitting(Card::MoreSecondaries);
        // Her stock hull has room for five a side, three more than she is built with: the
        // forward turret keeps its beams clear.
        assert_eq!(run.wing_room(), 5);
        assert_eq!(run.ranks(Card::MoreSecondaries), Some((0, 3)));
        for _ in 0..3 {
            assert!(run.refit_in_dock(more));
        }
        assert_eq!(run.fittings.secondaries().per_side, 5);
        assert_eq!(run.ranks(Card::MoreSecondaries), Some((3, 3)));
        // Her deck is full: what she wants is a longer hull, and no hand deals the card.
        assert!(!more.fits(&run));
        assert_eq!(run.refusal(more), Some(Refusal::Length(35)));
        for hand in 0..200 {
            run.hands = hand;
            assert!(!run.offers().contains(&Card::MoreSecondaries));
        }
        // An empty bay has room for two more a side.
        lengthen(&mut run);
        assert_eq!(run.wing_room(), 7);
        assert_eq!(run.ranks(Card::MoreSecondaries), Some((3, 5)));
        assert_eq!(run.refusal(more), None);
        assert!(run.refit_in_dock(more) && run.refit_in_dock(more));
        assert_eq!(run.fittings.secondaries().per_side, 7);
    }
}
