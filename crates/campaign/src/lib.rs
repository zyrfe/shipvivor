//! Run progression: the mission clock, objectives, hull, fittings and routes of one run.
//! Timers advance only while the ship is sailing. Menus, pausing and the helm trial
//! belong to the application, not to the run. A patrol is sailed until every base is
//! down or she sinks: the clock is the time to beat, each base wrecked puts time back on
//! it, and finishing inside it pays a star's worth of yard points.
mod battery;
mod plan;
mod workshop;
pub use battery::{CALIBRES, ShipBeam};
pub use plan::{Order, Plan};
use shipvivor_artillery::{
    AntiAir, MainShell, STOCK_TRAVERSE, Secondaries, TurretLayout, training_rate,
};
use shipvivor_shipyard::{Component, Design, Frame, Module, Protection, Ship, StrikeCraft};
pub use workshop::{
    DeckModule, DockJob, HULL_STEP, MAX_GUN_STATIONS, MAX_LENGTH, MAX_SECTIONS_PER_KIND,
    MOST_GUN_STATIONS, Refusal, deck_ranks, plane_room,
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
    RegentSunk,
}

/// One sea of a run. Each belongs to the pool of one round of [`POOLS`], except Crown
/// Deep, where every route ends.
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
    // Seas past the second round come after the first seven, so their discriminants,
    // and the leg seeds drawn from them, stay as they were.
    SealersRoad,
    KittiwakeSound,
    RimeGate,
    WhalebackReach,
    FogboundBay,
    TheGullet,
    LongnightRoad,
    HollowSound,
    SplinterCut,
    FrostlineReach,
    TernHaven,
    DevilsThroat,
    BorealRoad,
    CalvingSound,
    RegentsGate,
    // The rest of each round's pool, round by round, after the first three of each. A
    // sea keeps its place here when it moves to another round's pool.
    EiderSound,
    SkerryHaven,
    DriftwoodBay,
    PtarmiganInlet,
    CairnRoad,
    SnowmeltNarrows,
    WalrusReach,
    GannetSound,
    ShingleRoad,
    NarwhalCut,
    BrineNarrows,
    AukBay,
    SkuaNarrows,
    FloeRoad,
    BearclawCut,
    IvorySound,
    HoarfrostBay,
    LodestoneReach,
    PetrelSound,
    IcefallCut,
    BeaconRoad,
    CormorantReach,
    ShiverNarrows,
    WreckersBay,
    PolarRoad,
    SleetSound,
    AnvilCut,
    BlackwaterNarrows,
    RavensReach,
    HarrowBay,
    TempestRoad,
    HailstoneSound,
    WolfpackNarrows,
    CoffinCut,
    NorthlightReach,
    ShroudBay,
    MidwinterRoad,
    AdmiraltySound,
    BastionCut,
    GauntletNarrows,
    CitadelBay,
    WhiteoutReach,
}

/// Patrols in a full run before the intercept: an anchorage, then a passage from each yard.
pub const PATROLS: usize = 6;
/// Rounds in a full run: the patrols, then Crown Deep.
pub const ROUNDS: usize = PATROLS + 1;
/// Passages each yard charts, and anchorages the first chart offers.
pub const OFFERED: usize = 3;

/// The seas of every patrol round, all of a round with as many bases and a third of
/// them held by each fleet: nine to a pool in the first three rounds, and twelve in the
/// last three, which took in the seas of a round the run no longer sails, one of each
/// fleet apiece. Each run charts three of each pool from its seed: the first round's as
/// the choice of anchorage, and each later round's as the passages every yard of the
/// round before offers, graded as [`GRADES`] says.
pub const POOLS: [&[Stage]; PATROLS] = {
    use Stage::*;
    [
        &[
            LanternBay,
            IronNarrows,
            BlackcapReach,
            EiderSound,
            SkerryHaven,
            DriftwoodBay,
            PtarmiganInlet,
            CairnRoad,
            SnowmeltNarrows,
        ],
        &[
            OuterRoad,
            GreywaterSound,
            PilotsCut,
            WalrusReach,
            GannetSound,
            ShingleRoad,
            NarwhalCut,
            BrineNarrows,
            AukBay,
        ],
        &[
            SealersRoad,
            KittiwakeSound,
            RimeGate,
            SkuaNarrows,
            FloeRoad,
            BearclawCut,
            IvorySound,
            HoarfrostBay,
            LodestoneReach,
        ],
        &[
            WhalebackReach,
            FogboundBay,
            TheGullet,
            PetrelSound,
            IcefallCut,
            BeaconRoad,
            CormorantReach,
            ShiverNarrows,
            WreckersBay,
            FrostlineReach,
            TernHaven,
            NorthlightReach,
        ],
        &[
            LongnightRoad,
            HollowSound,
            SplinterCut,
            PolarRoad,
            SleetSound,
            AnvilCut,
            BlackwaterNarrows,
            RavensReach,
            HarrowBay,
            CoffinCut,
            HailstoneSound,
            ShroudBay,
        ],
        &[
            BorealRoad,
            CalvingSound,
            RegentsGate,
            MidwinterRoad,
            AdmiraltySound,
            BastionCut,
            GauntletNarrows,
            CitadelBay,
            WhiteoutReach,
            DevilsThroat,
            WolfpackNarrows,
            TempestRoad,
        ],
    ]
};

/// How hard a passage runs. Any sea of a pool may be charted either way, so the same
/// waters come round again as a different fight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum Grade {
    /// Always open. Its stars pay about four fifths of a tough passage's, and its yard
    /// repairs her further.
    #[default]
    Moderate,
    /// Needs three stars. It runs under enemy air cover, its raids come sooner and
    /// quicker, and it fits a card on arrival.
    Tough,
}
impl Grade {
    /// Stars the leg just sailed must have earned to take a passage of this grade.
    pub fn stars(self) -> u8 {
        match self {
            Self::Moderate => 0,
            Self::Tough => MAX_STARS,
        }
    }
    /// What a passage of this grade grants on arrival.
    pub fn perk(self) -> Perk {
        match self {
            Self::Moderate => Perk::Repair(30),
            Self::Tough => Perk::FreeFitting,
        }
    }
    /// Yard points each star of a passage of this grade pays.
    pub fn star_points(self) -> u16 {
        GRADE_STAR_POINTS[self as usize]
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Moderate => "Moderate",
            Self::Tough => "Tough",
        }
    }
}

/// The grades of the passages each yard charts, in the order the chart numbers them:
/// one moderate, always open, then two tough ones that pay the same.
pub const GRADES: [Grade; OFFERED] = [Grade::Moderate, Grade::Tough, Grade::Tough];

/// What holds a sea, as the scouts report it: many light hulls, a few heavy ones, or
/// some of each. Each round's pool holds three seas of each, and every chart offers one
/// of each, so the captain picks the fight her guns suit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Fleet {
    /// Swarms of light hulls: patrol boats, destroyers and torpedo boats, quick off the
    /// slips and lightly gunned. Light plate everywhere, so a volume of quick fire
    /// tells, and a heavy gun's slow round is wasted on a boat it would sink anyway.
    Flotillas,
    /// Light and heavy together, as most waters are held.
    Mixed,
    /// Fewer hulls, most of them cruisers and battleships in line, the battleships
    /// belted with plate only a big gun's armour-piercing round opens.
    BattleLine,
}
impl Fleet {
    pub const ALL: [Fleet; 3] = [Fleet::Flotillas, Fleet::Mixed, Fleet::BattleLine];
    pub fn name(self) -> &'static str {
        match self {
            Self::Flotillas => "Flotillas",
            Self::Mixed => "Mixed squadrons",
            Self::BattleLine => "Battle line",
        }
    }
    /// What the scouts saw, short enough for a narrow card.
    pub fn report(self) -> &'static str {
        match self {
            Self::Flotillas => "Swarms of light hulls",
            Self::Mixed => "Light and heavy",
            Self::BattleLine => "Few hulls, heavy plate",
        }
    }
    /// The guns that tell against it.
    pub fn suits(self) -> &'static str {
        match self {
            Self::Flotillas => "Suits many quick guns",
            Self::Mixed => "Suits a balanced ship",
            Self::BattleLine => "Suits big guns and caps",
        }
    }
}
const _: () = assert!(Fleet::ALL.len() == OFFERED);

/// Keeps the charted seas' stream apart from anything else drawn from the run seed.
const ROUTE_STREAM: u64 = 0x726f_7574_6500_0000;

/// The three seas a run charts for `round`, from 1 for the anchorages to [`PATROLS`],
/// one held by each [`Fleet`], drawn from the round's pool by the run seed and set in an
/// order it also draws, or `None` past the last patrol. The same seed always charts the
/// same seas, whichever way she came.
pub fn charted(seed: u64, round: usize) -> Option<[Stage; OFFERED]> {
    let pool = POOLS.get(round.checked_sub(1)?)?;
    let mut fleets = Fleet::ALL.to_vec();
    let mut state = mix(seed ^ ROUTE_STREAM.wrapping_add(round as u64));
    Some(std::array::from_fn(|_| {
        let fleet = fleets.swap_remove((next(&mut state) % fleets.len() as u64) as usize);
        let held: Vec<Stage> = (pool.iter().copied())
            .filter(|sea| sea.fleet() == fleet)
            .collect();
        held[(next(&mut state) % held.len() as u64) as usize]
    }))
}

/// The anchorages a run's first chart offers.
pub fn anchorages(seed: u64) -> [Stage; OFFERED] {
    charted(seed, 1).expect("the first round has a pool")
}

/// The time limit and star goals of one leg.
#[derive(Clone, Copy)]
pub struct Mission {
    pub name: &'static str,
    pub minutes: u16,
    pub kills: u16,
    pub crates: u16,
}

impl Stage {
    /// The round this sea is sailed in, from 1 for an anchorage to [`ROUNDS`] for Crown Deep.
    pub fn round(self) -> usize {
        POOLS
            .iter()
            .position(|pool| pool.contains(&self))
            .map_or(ROUNDS, |round| round + 1)
    }
    pub fn is_opening(self) -> bool {
        self.round() == 1
    }
    /// The last patrol before the intercept. Its yard sails her on to Crown Deep.
    pub fn is_last_patrol(self) -> bool {
        self.round() == PATROLS
    }
    /// A passage between the anchorage and Crown Deep, which a yard charts and grades.
    fn is_passage(self) -> bool {
        (2..=PATROLS).contains(&self.round())
    }

    /// What holds this sea. Crown Deep is the Regent's battle line.
    pub fn fleet(self) -> Fleet {
        use Fleet::*;
        match self {
            Self::IronNarrows | Self::LanternBay | Self::DriftwoodBay => Mixed,
            Self::EiderSound | Self::SkerryHaven | Self::PtarmiganInlet => Flotillas,
            Self::BlackcapReach | Self::CairnRoad | Self::SnowmeltNarrows => BattleLine,
            Self::GreywaterSound | Self::PilotsCut | Self::BrineNarrows => Mixed,
            Self::GannetSound | Self::AukBay | Self::ShingleRoad => Flotillas,
            Self::OuterRoad | Self::WalrusReach | Self::NarwhalCut => BattleLine,
            Self::SealersRoad | Self::FloeRoad | Self::IvorySound => Mixed,
            Self::KittiwakeSound | Self::SkuaNarrows | Self::HoarfrostBay => Flotillas,
            Self::RimeGate | Self::BearclawCut | Self::LodestoneReach => BattleLine,
            Self::FogboundBay | Self::BeaconRoad | Self::WreckersBay => Mixed,
            Self::PetrelSound | Self::CormorantReach | Self::ShiverNarrows => Flotillas,
            Self::WhalebackReach | Self::TheGullet | Self::IcefallCut => BattleLine,
            Self::LongnightRoad | Self::HollowSound | Self::HarrowBay => Mixed,
            Self::SplinterCut | Self::SleetSound | Self::RavensReach => Flotillas,
            Self::PolarRoad | Self::AnvilCut | Self::BlackwaterNarrows => BattleLine,
            Self::TempestRoad | Self::NorthlightReach | Self::ShroudBay => Mixed,
            Self::TernHaven | Self::HailstoneSound | Self::WolfpackNarrows => Flotillas,
            Self::FrostlineReach | Self::DevilsThroat | Self::CoffinCut => BattleLine,
            Self::BorealRoad | Self::RegentsGate | Self::MidwinterRoad => Mixed,
            Self::CalvingSound | Self::GauntletNarrows | Self::WhiteoutReach => Flotillas,
            Self::AdmiraltySound | Self::BastionCut | Self::CitadelBay => BattleLine,
            Self::CrownDeep => BattleLine,
        }
    }

    /// A tough passage takes the ship into enemy air cover.
    pub fn enters_air_cover(self, grade: Grade) -> bool {
        self.is_passage() && grade == Grade::Tough
    }

    /// Yard points each star earned in this sea pays, sailed at `grade`. A tough passage
    /// pays more than a moderate one, and every anchorage pays the open-water rate.
    /// Crown Deep earns no stars.
    pub fn star_points(self, grade: Grade) -> u16 {
        match self.round() {
            1 => STAR_POINTS,
            round if round <= PATROLS => grade.star_points(),
            _ => 0,
        }
    }

    pub fn mission(self) -> Mission {
        // Every patrol is short. A clean sweep of its bases takes about four minutes,
        // and the clock, the time to beat, leaves a minute or so over, with
        // `BASE_TIME` more for each base wrecked. The goals rise round by round.
        let (name, minutes, kills, crates) = match self {
            Self::CrownDeep => ("Crown Deep", 8, 0, 0),
            Self::LanternBay => ("Lantern Bay", 5, 3, 2),
            Self::IronNarrows => ("The Iron Narrows", 5, 4, 2),
            Self::BlackcapReach => ("Blackcap Reach", 5, 5, 2),
            Self::EiderSound => ("Eider Sound", 5, 3, 2),
            Self::SkerryHaven => ("Skerry Haven", 5, 3, 2),
            Self::DriftwoodBay => ("Driftwood Bay", 5, 4, 2),
            Self::PtarmiganInlet => ("Ptarmigan Inlet", 5, 4, 2),
            Self::CairnRoad => ("Cairn Road", 5, 5, 2),
            Self::SnowmeltNarrows => ("Snowmelt Narrows", 5, 4, 2),
            Self::OuterRoad => ("Outer Road", 5, 5, 2),
            Self::GreywaterSound => ("Greywater Sound", 5, 5, 3),
            Self::PilotsCut => ("Pilot's Cut", 5, 6, 3),
            Self::WalrusReach => ("Walrus Reach", 5, 5, 2),
            Self::GannetSound => ("Gannet Sound", 5, 5, 3),
            Self::ShingleRoad => ("Shingle Road", 5, 6, 2),
            Self::NarwhalCut => ("Narwhal Cut", 5, 6, 3),
            Self::BrineNarrows => ("Brine Narrows", 5, 5, 3),
            Self::AukBay => ("Auk Bay", 5, 5, 3),
            Self::SealersRoad => ("Sealers' Road", 5, 6, 3),
            Self::KittiwakeSound => ("Kittiwake Sound", 5, 6, 3),
            Self::RimeGate => ("Rime Gate", 5, 7, 3),
            Self::SkuaNarrows => ("Skua Narrows", 5, 7, 3),
            Self::FloeRoad => ("Floe Road", 5, 6, 3),
            Self::BearclawCut => ("Bearclaw Cut", 5, 7, 3),
            Self::IvorySound => ("Ivory Sound", 5, 6, 3),
            Self::HoarfrostBay => ("Hoarfrost Bay", 5, 6, 3),
            Self::LodestoneReach => ("Lodestone Reach", 5, 7, 3),
            Self::WhalebackReach => ("Whaleback Reach", 6, 7, 3),
            Self::FogboundBay => ("Fogbound Bay", 6, 7, 4),
            Self::TheGullet => ("The Gullet", 6, 8, 3),
            Self::PetrelSound => ("Petrel Sound", 6, 7, 4),
            Self::IcefallCut => ("Icefall Cut", 6, 8, 3),
            Self::BeaconRoad => ("Beacon Road", 6, 7, 3),
            Self::CormorantReach => ("Cormorant Reach", 6, 8, 3),
            Self::ShiverNarrows => ("Shiver Narrows", 6, 8, 3),
            Self::WreckersBay => ("Wreckers' Bay", 6, 7, 4),
            Self::FrostlineReach => ("Frostline Reach", 6, 8, 3),
            Self::TernHaven => ("Tern Haven", 6, 7, 4),
            Self::NorthlightReach => ("Northlight Reach", 6, 7, 4),
            Self::LongnightRoad => ("Longnight Road", 6, 8, 3),
            Self::HollowSound => ("Hollow Sound", 6, 8, 4),
            Self::SplinterCut => ("Splinter Cut", 6, 9, 4),
            Self::PolarRoad => ("Polar Road", 6, 8, 3),
            Self::SleetSound => ("Sleet Sound", 6, 8, 4),
            Self::AnvilCut => ("Anvil Cut", 6, 9, 4),
            Self::BlackwaterNarrows => ("Blackwater Narrows", 6, 9, 3),
            Self::RavensReach => ("Ravens' Reach", 6, 9, 4),
            Self::HarrowBay => ("Harrow Bay", 6, 8, 4),
            Self::CoffinCut => ("Coffin Cut", 6, 9, 3),
            Self::HailstoneSound => ("Hailstone Sound", 6, 8, 4),
            Self::ShroudBay => ("Shroud Bay", 6, 8, 4),
            Self::BorealRoad => ("Boreal Road", 6, 10, 4),
            Self::CalvingSound => ("Calving Sound", 6, 10, 5),
            Self::RegentsGate => ("Regent's Gate", 6, 11, 4),
            Self::MidwinterRoad => ("Midwinter Road", 6, 10, 4),
            Self::AdmiraltySound => ("Admiralty Sound", 6, 10, 5),
            Self::BastionCut => ("Bastion Cut", 6, 11, 4),
            Self::GauntletNarrows => ("Gauntlet Narrows", 6, 11, 4),
            Self::CitadelBay => ("Citadel Bay", 6, 10, 5),
            Self::WhiteoutReach => ("Whiteout Reach", 6, 11, 4),
            Self::DevilsThroat => ("Devil's Throat", 6, 11, 4),
            Self::WolfpackNarrows => ("Wolfpack Narrows", 6, 11, 4),
            Self::TempestRoad => ("Tempest Road", 6, 10, 4),
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
/// One star per mission goal. The clock takes none of them away: beating it pays a
/// star's worth of yard points on top.
pub const MAX_STARS: u8 = 3;
/// Seconds the leg's clock gains for each base wrecked, so a patrol that keeps working
/// through its bases has a reason to press on and no need to rush.
pub const BASE_TIME: f32 = 45.;
/// Yard points for each star a leg in an anchorage earns.
pub const STAR_POINTS: u16 = 2;
/// Yard points for each star of a moderate passage and of a tough one. With every
/// star, the clock's star's worth and a point a base, a moderate passage pays about
/// four fifths of what a tough one of the same round does.
pub const GRADE_STAR_POINTS: [u16; 2] = [3, 4];
/// Yard points for each enemy yard whose gantry comes down on a leg.
pub const YARD_POINTS: u16 = 1;

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
/// Hull restored by each recovered crate: 8% of the stock hull, so a run of pickups
/// patches her up without swinging her from battered to sound in a few seconds.
pub const CRATE_REPAIR: u16 = 8;
/// Rounds this wide or wider, in inches, strike her with their full damage.
const FULL_BORE: f32 = 10.;
/// Rounds this wide or narrower do this share of their damage, and the share rises
/// evenly from here to [`FULL_BORE`]: a light gun stings, a heavy one hurts.
const LIGHT_BORE: (f32, f32) = (3., 0.5);

/// Share of its damage a round `calibre` inches across does to her, or all of it for a
/// blow no plate can turn, such as a ram, a bomb or a torpedo.
fn bore_share(calibre: Option<f32>) -> f32 {
    let Some(calibre) = calibre else {
        return 1.;
    };
    let (light, share) = LIGHT_BORE;
    let t = ((calibre - light) / (FULL_BORE - light)).clamp(0., 1.);
    share + (1. - share) * t
}
/// Hull points of repair the dock gives after each patrol. A badly damaged ship sails with
/// whatever this does not reach.
pub const DOCK_REPAIR: u16 = 40;

/// What a route grants on arrival.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Perk {
    None,
    Repair(u16),
    FreeFitting,
}

/// A passage from a yard to the next round: the sea, how hard it runs, the stars the
/// leg just sailed must have earned to take it, and what it grants on arrival.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Exit {
    pub stage: Stage,
    pub grade: Grade,
    pub stars: u8,
    pub perk: Perk,
}
impl Exit {
    /// The passage into `stage` at `grade`, priced as the grade says.
    pub fn new(stage: Stage, grade: Grade) -> Self {
        Self {
            stage,
            grade,
            stars: grade.stars(),
            perk: grade.perk(),
        }
    }
    /// Yard points each star earned on this passage pays.
    pub fn star_points(&self) -> u16 {
        self.stage.star_points(self.grade)
    }
    /// The passage runs under enemy air cover.
    pub fn under_air_cover(&self) -> bool {
        self.stage.enters_air_cover(self.grade)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Card {
    Loader,
    Shells,
    Turbines,
    Caps,
    Velocity,
    TripleTurrets,
    QuadTurrets,
    /// One more wing gun on each beam, while her deck has room for it.
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
    /// Another pair of AA guns.
    MoreAntiAir,
    /// More damage from each AA burst.
    HeavyAntiAir,
    /// A shorter AA reload.
    QuickAntiAir,
    /// Bigger pumps, which clear the water from flooding compartments faster.
    Pumps,
    /// Another trainable launcher on every missile rack.
    MoreLaunchers,
    /// Another missile in every launcher's salvo.
    MoreMissiles,
    /// Another catapult and floatplane on the plane section.
    MorePlanes,
    /// Another bomb on every floatplane, dropped in a stick across its mark.
    MoreBombs,
    /// Every wing gun rebuilt as a gunhouse that fires on its own: four-inch on a
    /// battleship's hull, then six-inch on a heavy battleship's.
    SecondaryTurrets,
}
impl Card {
    /// How often a level-up hand deals this card, or `None` for work only the yard does.
    pub fn rarity(self) -> Option<Rarity> {
        DECK.iter()
            .find(|(card, _)| *card == self)
            .map(|&(_, rarity)| rarity)
    }
}

/// How often a level-up hand deals a card, against the other cards the ship can take.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
}
impl Rarity {
    /// The card's share of each draw: a common card comes up six times as often as a rare.
    pub const fn weight(self) -> u32 {
        match self {
            Self::Common => 6,
            Self::Uncommon => 3,
            Self::Rare => 1,
        }
    }
}

/// The level-up deck: every card a hand can deal, and how rare it is. The bore and the
/// barrels a turret carries are yard work, so they are not here, and nor is the gear of
/// the strike craft or the secondary turrets, which the yard fits where they stand. A new card joins the deck
/// with one line.
pub const DECK: &[(Card, Rarity)] = &[
    (Card::Loader, Rarity::Common),
    (Card::Velocity, Rarity::Common),
    (Card::TrainingGear, Rarity::Common),
    (Card::Turbines, Rarity::Common),
    (Card::QuickSecondaries, Rarity::Common),
    (Card::SecondaryRange, Rarity::Common),
    (Card::HeavySecondaries, Rarity::Common),
    (Card::QuickAntiAir, Rarity::Common),
    (Card::MoreAntiAir, Rarity::Common),
    (Card::HeavyAntiAir, Rarity::Common),
    (Card::Pumps, Rarity::Common),
    (Card::Caps, Rarity::Uncommon),
    (Card::Armor, Rarity::Uncommon),
    (Card::Hardening, Rarity::Uncommon),
    (Card::MoreSecondaries, Rarity::Uncommon),
    (Card::TwinSecondaries, Rarity::Rare),
];
/// Cards in a level-up hand, when that many fit.
pub const HAND: usize = 3;

/// The splitmix64 finaliser: every bit of `x` reaches every bit of the result.
fn mix(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}
/// The next number in a splitmix64 stream.
fn next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    mix(*state)
}
/// Keeps the hands' stream apart from anything else drawn from the run seed.
const HAND_STREAM: u64 = 0x6861_6e64_7300_0000;

/// Up to [`HAND`] different cards from `pool`, each draw weighted by rarity among the cards
/// not yet drawn.
fn draw(mut pool: Vec<(Card, Rarity)>, key: u64) -> Vec<Card> {
    let mut state = key;
    let mut hand = Vec::with_capacity(HAND);
    while hand.len() < HAND && !pool.is_empty() {
        let weight = |at: usize| u64::from(pool[at].1.weight());
        let total: u64 = (0..pool.len()).map(weight).sum();
        let mut roll = next(&mut state) % total;
        let mut pick = 0;
        while roll >= weight(pick) {
            roll -= weight(pick);
            pick += 1;
        }
        hand.push(pool.remove(pick).0);
    }
    hand
}

/// Metres each rangefinder card adds to the secondaries' reach.
const WING_RANGE_STEP: f32 = 40.;
/// A size of gunhouse the wing guns are rebuilt as: its bore in inches, what it adds to
/// each hit and to their reach in metres, and the share their reload grows by for the
/// heavier round.
struct WingHouse {
    bore: u16,
    damage: u16,
    reach: f32,
    load: f32,
}
/// The gunhouses the secondaries come in, a rank of [`Card::SecondaryTurrets`] each:
/// four-inch, like the main battery's first turrets, then six-inch.
const WING_HOUSES: [WingHouse; limits::WING_TURRETS as usize] = [
    WingHouse {
        bore: 4,
        damage: 2,
        reach: 100.,
        load: 1.5,
    },
    WingHouse {
        bore: 6,
        damage: 5,
        reach: 180.,
        load: 2.25,
    },
];
/// The share of a barrel's loading time each rank of loaders takes off, for the main
/// battery, the secondaries and the AA alike. It comes off what the ranks before left,
/// so every rank is worth the same to the crew: four ranks take it down to about half.
pub const LOADER_CUT: f32 = 0.15;
/// The share of the stock loading time a crew drilled with `ranks` of loaders takes.
pub fn drilled(ranks: u16) -> f32 {
    (1. - LOADER_CUT).powi(i32::from(ranks))
}
/// The most ranks of each refit the ship can take.
pub mod limits {
    use shipvivor_artillery::{AntiAir, Secondaries};
    pub const TRAINING_GEAR: u16 = shipvivor_artillery::MOST_TRAINING_GEAR;
    /// Ranks of piercing caps: each adds a quarter, so eight treble the stock round's
    /// penetration.
    pub const CAPS: u16 = 8;
    /// On the heaviest hull. A lighter one carries fewer: see [`crate::ShipBeam::most_armor`].
    pub const ARMOR: u16 = crate::ShipBeam::BROADEST.most_armor();
    pub const HARDENING: u16 = 4;
    /// Secondaries past the stock pair a side, as far as any deck has room. Her own deck
    /// usually has room for fewer: see `Run::wing_room`.
    pub const WING_MOUNTS: u16 = (Secondaries::MOST_PER_SIDE - Secondaries::STOCK.per_side) as u16;
    pub const WING_SHELLS: u16 = 5;
    pub const WING_LOADERS: u16 = 6;
    pub const WING_RANGE: u16 = 5;
    /// Barrels past the first on every secondary: twin, triple, then quadruple mounts.
    pub const WING_BARRELS: u16 = 3;
    /// AA pairs past the stock pair, as far as any deck has room. Her own deck usually has
    /// room for fewer: see `Run::aa_room`.
    pub const AA_MOUNTS: u16 = ((AntiAir::MOST_MOUNTS - AntiAir::STOCK.mounts) / 2) as u16;
    pub const AA_SHELLS: u16 = 5;
    pub const AA_LOADERS: u16 = 6;
    pub const PUMPS: u16 = 6;
    /// Launchers past the first on each missile rack, missiles past the stock salvo,
    /// planes past the first and bombs past the first on each plane. Each plane section
    /// launches [`PLANES_A_SECTION`] planes, so more than that want another section.
    pub const LAUNCHERS: u16 = 3;
    pub const MISSILES: u16 = 4;
    pub const PLANES: u16 = 5;
    pub const BOMBS: u16 = 4;
    /// Sizes of gunhouse for the secondaries: four-inch, then six-inch.
    pub const WING_TURRETS: u16 = 2;
    /// Catapults on one plane section.
    pub const PLANES_A_SECTION: u16 = 3;
}
/// Each rank of pumps adds this share of the stock pumping.
const PUMP_STEP: f32 = 0.5;
/// Each rank of armor plate adds this share of stock strength to every panel, and
/// this many inches to its thickness. On sound plate an escort's four ranks turn
/// the enemy's secondaries and aircraft guns, then patrol boats and destroyers. A
/// heavy cruiser's hull adds cruisers, then battleships and shore guns, and a
/// battleship's the Parliament's guns. Nothing turns the Regent's.
const ARMOR_PLATE: f32 = 0.25;
const ARMOR_INCHES: f32 = 1.5;
/// Each rank of hardening adds this share of stock strength to every deck fitting.
const HARDENED_FITTINGS: f32 = 0.5;

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
    pub wing_mounts: u16,
    pub wing_shells: u16,
    pub wing_loaders: u16,
    pub wing_range: u16,
    pub wing_twin: u16,
    pub training_gear: u16,
    pub armor: u16,
    pub hardening: u16,
    pub aa_mounts: u16,
    pub aa_shells: u16,
    pub aa_loaders: u16,
    pub pumps: u16,
    pub launchers: u16,
    pub missiles: u16,
    pub planes: u16,
    pub bombs: u16,
    pub wing_turrets: u16,
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
            Card::MoreSecondaries => &mut self.wing_mounts,
            Card::HeavySecondaries => &mut self.wing_shells,
            Card::QuickSecondaries => &mut self.wing_loaders,
            Card::SecondaryRange => &mut self.wing_range,
            Card::TwinSecondaries => &mut self.wing_twin,
            Card::TrainingGear => &mut self.training_gear,
            Card::Armor => &mut self.armor,
            Card::Hardening => &mut self.hardening,
            Card::MoreAntiAir => &mut self.aa_mounts,
            Card::HeavyAntiAir => &mut self.aa_shells,
            Card::QuickAntiAir => &mut self.aa_loaders,
            Card::Pumps => &mut self.pumps,
            Card::MoreLaunchers => &mut self.launchers,
            Card::MoreMissiles => &mut self.missiles,
            Card::MorePlanes => &mut self.planes,
            Card::MoreBombs => &mut self.bombs,
            Card::SecondaryTurrets => &mut self.wing_turrets,
            // Bore and barrels are fitted turret by turret in the yard.
            Card::TripleTurrets | Card::QuadTurrets => return self,
        };
        *rank = rank.saturating_add(1);
        self
    }
    /// Seconds for one main battery barrel to reload. A twin mount working both barrels
    /// in turn fires every half of this. A light gun loads in moments and a heavy one
    /// slowly, faster than in step with its calibre: see [`MainShell::load_seconds`].
    /// Each rank of quick loaders takes [`LOADER_CUT`] off it: see [`drilled`].
    pub fn reload(&self) -> f32 {
        self.round().load_seconds() * drilled(self.loader)
    }
    /// Seconds for one barrel of a `calibre`-inch gun to reload with this crew's drill.
    /// Turrets of different bores load on the same curve, each at its own pace.
    pub fn reload_for(&self, calibre: u16) -> f32 {
        let round = MainShell {
            bore: calibre.saturating_sub(CALIBRES[0]),
            ..MainShell::STOCK
        };
        round.load_seconds() * drilled(self.loader)
    }
    /// The main battery round as fitted.
    pub fn round(&self) -> MainShell {
        MainShell {
            bore: self.calibre() - CALIBRES[0],
            caps: self.caps.min(limits::CAPS),
            velocity: self.velocity,
        }
    }
    /// Degrees per second the main battery trains, and elevates in proportion. A larger
    /// bore trains more slowly, and training gear wins it back.
    pub fn traverse(&self) -> f32 {
        STOCK_TRAVERSE * training_rate(self.round().calibre_inches(), self.training_gear)
    }
    /// The light AA battery: the pair she is built with and another pair per mounting
    /// card, one more damage a burst per heavier round, and quick loaders working as
    /// they do for the main battery.
    pub fn anti_air(&self) -> AntiAir {
        let stock = AntiAir::STOCK;
        AntiAir {
            mounts: (stock.mounts + 2 * usize::from(self.aa_mounts)).min(AntiAir::MOST_MOUNTS),
            damage: stock
                .damage
                .saturating_add(self.aa_shells.min(limits::AA_SHELLS)),
            reload: stock.reload * drilled(self.aa_loaders.min(limits::AA_LOADERS)),
            range: stock.range,
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
    /// The close-range battery: the stock guns on the bow and the beam and one more gun a
    /// beam per mounting card, one more damage a hit per heavier round, quick loaders
    /// working as they do for the main battery, 40 m more reach per rangefinder and a
    /// second barrel on every gun once twinned. Rebuilt as gunhouses, four-inch and then
    /// six-inch, each gun hits harder and reaches farther, and loads more slowly.
    pub fn secondaries(&self) -> Secondaries {
        let stock = Secondaries::STOCK.per_side;
        let house = usize::from(self.wing_turrets.min(limits::WING_TURRETS))
            .checked_sub(1)
            .map(|rank| &WING_HOUSES[rank]);
        let (damage, reach, load) = house.map_or((0, 0., 1.), |h| (h.damage, h.reach, h.load));
        Secondaries {
            per_side: (stock + usize::from(self.wing_mounts)).min(Secondaries::MOST_PER_SIDE),
            damage: (Secondaries::STOCK.damage)
                .saturating_add(self.wing_shells.min(limits::WING_SHELLS))
                .saturating_add(damage),
            reload: Secondaries::STOCK.reload
                * drilled(self.wing_loaders.min(limits::WING_LOADERS))
                * load,
            range: Secondaries::STOCK.range
                + WING_RANGE_STEP * f32::from(self.wing_range.min(limits::WING_RANGE))
                + reach,
            barrels: 1 + usize::from(self.wing_twin.min(limits::WING_BARRELS)),
            gunhouse: house.map(|h| h.bore),
        }
    }
    /// The bore, in inches, of the next size of gunhouse her secondaries could be
    /// rebuilt as, or `None` once they are the biggest the yard makes.
    pub fn next_secondary_bore(&self) -> Option<u16> {
        WING_HOUSES
            .get(usize::from(self.wing_turrets))
            .map(|h| h.bore)
    }
    /// The strike craft: a launcher on each missile rack and another per launcher card,
    /// the stock salvo and another missile per missile card, a plane and another per
    /// plane card, and a bomb on each plane and another per bomb card.
    pub fn strike_craft(&self) -> StrikeCraft {
        let more = |ranks: u16, most: u16| usize::from(ranks.min(most));
        let stock = StrikeCraft::STOCK;
        StrikeCraft {
            launchers: stock.launchers + more(self.launchers, limits::LAUNCHERS),
            salvo: stock.salvo + more(self.missiles, limits::MISSILES),
            planes: stock.planes + more(self.planes, limits::PLANES),
            bombs: stock.bombs + more(self.bombs, limits::BOMBS),
        }
    }

    /// A hand drawn from `key`: up to [`HAND`] different cards from the [`DECK`] that the
    /// ship can take, each draw weighted by rarity. Fewer are dealt when fewer fit.
    pub fn deal(&self, key: u64) -> Vec<Card> {
        self.deal_where(key, |_| true)
    }
    /// A hand as [`Fittings::deal`] draws it, from only the cards `room` also allows.
    pub fn deal_where(&self, key: u64, room: impl Fn(Card) -> bool) -> Vec<Card> {
        let pool = DECK
            .iter()
            .copied()
            .filter(|&(card, _)| self.can_fit(card) && room(card))
            .collect();
        draw(pool, key)
    }
    /// Ranks of `card` fitted, out of the most the ship can ever take, for a card that
    /// stacks to a limit. `None` for a card that is fitted once and replaced, or that has
    /// no limit.
    pub fn ranks(&self, card: Card) -> Option<(u16, u16)> {
        use Card::*;
        Some(match card {
            MoreSecondaries => (
                self.wing_mounts.min(limits::WING_MOUNTS),
                limits::WING_MOUNTS,
            ),
            HeavySecondaries => (self.wing_shells, limits::WING_SHELLS),
            QuickSecondaries => (self.wing_loaders, limits::WING_LOADERS),
            SecondaryRange => (self.wing_range, limits::WING_RANGE),
            TwinSecondaries => (self.wing_twin, limits::WING_BARRELS),
            TrainingGear => (self.training_gear, limits::TRAINING_GEAR),
            Caps => (self.caps, limits::CAPS),
            Armor => (self.armor, limits::ARMOR),
            Hardening => (self.hardening, limits::HARDENING),
            MoreAntiAir => (self.aa_mounts, limits::AA_MOUNTS),
            HeavyAntiAir => (self.aa_shells, limits::AA_SHELLS),
            QuickAntiAir => (self.aa_loaders, limits::AA_LOADERS),
            Pumps => (self.pumps, limits::PUMPS),
            MoreLaunchers => (self.launchers, limits::LAUNCHERS),
            MoreMissiles => (self.missiles, limits::MISSILES),
            MorePlanes => (self.planes, limits::PLANES),
            MoreBombs => (self.bombs, limits::BOMBS),
            SecondaryTurrets => (self.wing_turrets, limits::WING_TURRETS),
            _ => return None,
        })
    }
    pub fn can_fit(&self, card: Card) -> bool {
        use Card::*;
        match card {
            Caps => self.caps < limits::CAPS,
            Shells => CALIBRES
                .get(usize::from(self.shells) + 1)
                .is_some_and(|&inch| inch <= self.capacity()),
            MoreSecondaries => self.secondaries().per_side < Secondaries::MOST_PER_SIDE,
            HeavySecondaries => self.wing_shells < limits::WING_SHELLS,
            QuickSecondaries => self.wing_loaders < limits::WING_LOADERS,
            SecondaryRange => self.wing_range < limits::WING_RANGE,
            TwinSecondaries => self.wing_twin < limits::WING_BARRELS,
            TrainingGear => self.training_gear < limits::TRAINING_GEAR,
            Armor => self.armor < self.beam.most_armor(),
            Hardening => self.hardening < limits::HARDENING,
            MoreAntiAir => self.aa_mounts < limits::AA_MOUNTS,
            HeavyAntiAir => self.aa_shells < limits::AA_SHELLS,
            QuickAntiAir => self.aa_loaders < limits::AA_LOADERS,
            Pumps => self.pumps < limits::PUMPS,
            MoreLaunchers => self.launchers < limits::LAUNCHERS,
            MoreMissiles => self.missiles < limits::MISSILES,
            MorePlanes => self.planes < limits::PLANES,
            MoreBombs => self.bombs < limits::BOMBS,
            // A gunhouse at every wing station wants the room of a battleship's deck,
            // and a six-inch one a heavy battleship's.
            SecondaryTurrets => self
                .next_secondary_bore()
                .is_some_and(|bore| self.beam.secondary_bore() >= Some(bore)),
            // The yard rebores and rearms each turret on its own: see `DockJob::Calibre`
            // and `DockJob::Barrels`.
            TripleTurrets | QuadTurrets => false,
            _ => true,
        }
    }
    pub fn speed_factor(&self) -> f32 {
        1. + 0.15 * f32::from(self.turbines)
    }
    /// Her pumping, as a multiple of the stock pumps.
    pub fn pumping(&self) -> f32 {
        1. + PUMP_STEP * f32::from(self.pumps.min(limits::PUMPS))
    }
}

#[derive(Default, Clone, Copy)]
pub struct Objectives {
    pub kills: u16,
    pub crates: u16,
    pub destroyed_drydock: bool,
    /// Enemy yards whose gantry has come down. Each pays [`YARD_POINTS`] at the leg's end.
    pub yards: u16,
    /// Coasters sunk. Each pays [`COASTER_XP`] and no stores, and is no raider.
    pub coasters: u16,
    /// Cargo sent to the bottom, in yard points. Each whole point it makes, with what
    /// was carried over from earlier legs, is a yard point.
    pub cargo: f32,
    /// Enemy aircraft shot down. Each pays [`PLANE_XP`].
    pub planes: u16,
    /// Harbour guns silenced. Each pays [`GUN_XP`]. They fight on apart from their yard,
    /// so they are worth taking out or worth steering clear of.
    pub guns: u16,
    /// Fuel trucks blown up on an airfield. Each pays [`VEHICLE_XP`].
    pub vehicles: u16,
}
pub const CRATE_XP: u32 = 10;
/// Experience for each coaster sunk.
pub const COASTER_XP: u32 = 5;
/// Experience for each enemy aircraft shot down.
pub const PLANE_XP: u32 = 3;
/// Experience for each harbour gun silenced.
pub const GUN_XP: u32 = 4;
/// Experience for each fuel truck blown up on an airfield.
pub const VEHICLE_XP: u32 = 2;
/// Experience for each torpedo boat sunk. She is a nuisance and no raider.
pub const TORPEDO_BOAT_XP: u32 = 5;

#[derive(Clone)]
pub struct Run {
    phase: Phase,
    pub stage: Stage,
    /// How hard the passage she took into these waters runs. An anchorage is moderate,
    /// and Crown Deep keeps the grade of the last passage before it.
    pub grade: Grade,
    /// Where the ship began her first patrol, retained on subsequent charts.
    pub origin: Stage,
    /// The passages taken from each yard, in order, retained after the routes meet at
    /// Crown Deep. The anchorage is [`Run::origin`], and Crown Deep is no passage.
    pub course: Vec<Stage>,
    pub elapsed: f32,
    /// Breakwater herself: her modules, what is fitted to them, and their condition.
    pub ship: Ship,
    pub fittings: Fittings,
    pub objectives: Objectives,
    /// Stars earned on every leg before this one.
    pub previous_stars: u8,
    pub experience: u32,
    pub level: u32,
    /// Charts every leg of the run and deals every level-up hand.
    pub seed: u64,
    /// Level-up hands owed. Each is a free card.
    pending_cards: u16,
    /// Hands a card has been taken from, so each hand is dealt afresh from the seed.
    hands: u32,
    /// Yard points earned and not yet spent. They carry from one yard to the next.
    points: u16,
    /// Cargo sunk on earlier legs short of a whole yard point, carried into this one.
    cargo_carried: f32,
    /// The patrol just sailed finished with time still on its clock.
    beat_clock: bool,
}
impl Default for Run {
    fn default() -> Self {
        Self {
            phase: Phase::Sailing,
            stage: Stage::IronNarrows,
            grade: Grade::Moderate,
            origin: Stage::IronNarrows,
            course: Vec::new(),
            elapsed: 0.,
            ship: Ship::new(Design::breakwater()),
            fittings: Fittings::default(),
            objectives: Objectives::default(),
            previous_stars: 0,
            experience: 0,
            level: 1,
            seed: 0,
            pending_cards: 0,
            hands: 0,
            points: 0,
            cargo_carried: 0.,
            beat_clock: false,
        }
    }
}
/// A default run is a fresh ship at the start of the first leg.
impl Run {
    pub fn phase(&self) -> Phase {
        self.phase
    }
    /// The last passage taken from a yard, or `None` while she is on her first patrol.
    pub fn passage(&self) -> Option<Stage> {
        self.course.last().copied()
    }
    /// A tough passage sails under enemy air cover, and so does the intercept after one.
    pub fn under_air_cover(&self) -> bool {
        match self.stage {
            Stage::CrownDeep => self.passage().is_some() && self.grade == Grade::Tough,
            stage => stage.enters_air_cover(self.grade),
        }
    }
    /// Yard points each star of this leg pays.
    pub fn star_points(&self) -> u16 {
        self.stage.star_points(self.grade)
    }
    /// What the passage into these waters granted on arrival. An anchorage and Crown
    /// Deep grant nothing.
    pub fn perk(&self) -> Perk {
        if self.passage() == Some(self.stage) {
            self.grade.perk()
        } else {
            Perk::None
        }
    }
    /// Seconds on the leg's clock: its minutes, and [`BASE_TIME`] for each base wrecked.
    pub fn clock(&self) -> f32 {
        f32::from(self.stage.mission().minutes) * 60. + BASE_TIME * f32::from(self.objectives.yards)
    }
    /// Seconds left on the leg's clock, the time to beat. Past it the leg goes on and
    /// this runs negative, until a base wrecked puts time back on it.
    pub fn remaining(&self) -> f32 {
        self.clock() - self.elapsed
    }
    /// Past the leg's clock. Finishing like this forgoes the clock's points.
    pub fn late(&self) -> bool {
        self.remaining() < 0.
    }
    /// Yard points the clock pays: a star's worth at the lane's rate, for a patrol
    /// finished with time still on it.
    pub fn clock_points(&self) -> u16 {
        if self.beat_clock {
            self.star_points()
        } else {
            0
        }
    }
    /// The patrol just finished with time still on the clock.
    pub fn beat_clock(&self) -> bool {
        self.beat_clock
    }
    pub fn next_level_xp(&self) -> u32 {
        self.level.saturating_add(2).saturating_mul(10)
    }
    /// Torpedo boats sunk on the step just settled, which counted them among the ships
    /// sunk. Each pays [`TORPEDO_BOAT_XP`] and none counts toward the raider goal, so
    /// chasing them earns no star. A level they make is dealt on the next step.
    pub fn sink_torpedo_boats(&mut self, count: u16) {
        if count == 0 || !matches!(self.phase, Phase::Sailing | Phase::Refit) {
            return;
        }
        self.objectives.kills = self.objectives.kills.saturating_sub(count);
        self.experience += u32::from(count) * TORPEDO_BOAT_XP;
    }

    /// Stars for the goals met. The clock never takes one away.
    pub fn stars(&self) -> u8 {
        self.goals_met()
    }
    /// Goals met so far.
    pub fn goals_met(&self) -> u8 {
        if self.stage == Stage::CrownDeep {
            return 0;
        }
        let goals = self.stage.mission();
        u8::from(self.objectives.kills >= goals.kills)
            + u8::from(self.objectives.crates >= goals.crates)
            + u8::from(self.objectives.destroyed_drydock)
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

    /// Yard points this leg has earned so far: its grade's [`GRADE_STAR_POINTS`] a star,
    /// [`YARD_POINTS`] a wrecked yard, one for each whole point of cargo sunk, and once
    /// she has finished inside it, the clock's [`Run::clock_points`].
    pub fn leg_points(&self) -> u16 {
        self.star_points()
            .saturating_mul(u16::from(self.stars()))
            .saturating_add(YARD_POINTS.saturating_mul(self.objectives.yards))
            .saturating_add(self.cargo_points())
            .saturating_add(self.clock_points())
    }
    /// Cargo sunk this leg and carried over from the last, in yard points.
    fn cargo_sunk(&self) -> f32 {
        self.cargo_carried + self.objectives.cargo.max(0.)
    }
    /// Yard points the cargo sunk has made this leg. They rank up on their own, apart
    /// from the level: one each time the cargo sunk crosses a whole point.
    pub fn cargo_points(&self) -> u16 {
        self.cargo_sunk().floor().min(f32::from(u16::MAX)) as u16
    }
    /// How far the cargo sunk has come toward the next yard point, from 0 to 1.
    pub fn cargo_progress(&self) -> f32 {
        self.cargo_sunk().fract()
    }
    /// A new leg starts its goals afresh. Cargo short of a whole point carries over.
    fn new_leg(&mut self) {
        self.cargo_carried = self.cargo_progress();
        self.objectives = Objectives::default();
        self.beat_clock = false;
    }
    /// Yard points to spend, earned on every leg and kept until they are spent.
    pub fn points(&self) -> u16 {
        self.points
    }

    /// Every patrol ends in the yard, which repairs her. Level-ups still owed are dealt as
    /// free hands before it takes her.
    fn finish_leg(&mut self, reason: LegEnd) {
        self.beat_clock = !self.late();
        self.points = self.points.saturating_add(self.leg_points());
        self.ship.dock(f32::from(DOCK_REPAIR));
        self.phase = Phase::Drydock(reason);
    }
    /// Every route rendezvous here, sailing from the last patrol's yard with what it
    /// fitted and repaired. The crossing itself mends nothing.
    pub fn engage_regent(&mut self) -> bool {
        if !matches!(self.phase, Phase::Intercept(_)) {
            return false;
        }
        self.previous_stars += self.stars();
        self.stage = Stage::CrownDeep;
        self.new_leg();
        self.elapsed = 0.;
        self.phase = Phase::Sailing;
        true
    }
    /// Level-up hands owed, each a free card.
    pub fn fittings_remaining(&self) -> u16 {
        self.pending_cards
    }
    /// The hand for the next level-up. It is drawn from the run seed and the count of hands
    /// already taken, so it stands until a card is fitted from it, and the same seed and the
    /// same choices always deal the same hands.
    pub fn offers(&self) -> Vec<Card> {
        let hand = mix(u64::from(self.hands).wrapping_add(HAND_STREAM));
        self.fittings
            .deal_where(mix(self.seed ^ hand), |card| self.has_room(card))
    }
    /// Commit one gameplay tick. A lethal hit ends the run before any rewards.
    ///
    /// `incoming` is damage with no place on the hull, and is spread over every module.
    /// Hits that landed somewhere go through [`Run::strike`] first.
    pub fn tick(&mut self, dt: f32, gains: Objectives, incoming: u16) {
        self.tick_sector(dt, gains, incoming, false);
    }
    /// Resolve one step of a patrol. A surviving ship that destroys the last yard
    /// earns the clearance.
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
    /// loses the run. The clock never ends a leg: she fights on until she wins or sinks.
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
        self.objectives.yards = self.objectives.yards.saturating_add(gains.yards);
        self.objectives.coasters = self.objectives.coasters.saturating_add(gains.coasters);
        self.objectives.cargo += gains.cargo.max(0.);
        self.objectives.destroyed_drydock |=
            gains.destroyed_drydock || gains.yards > 0 || objective == Some(LegEnd::BasesDestroyed);
        self.ship
            .patch(f32::from(gains.crates.saturating_mul(CRATE_REPAIR)));
        self.experience += u32::from(gains.crates) * CRATE_XP;
        self.experience += u32::from(gains.coasters) * COASTER_XP;
        self.objectives.planes = self.objectives.planes.saturating_add(gains.planes);
        self.experience += u32::from(gains.planes) * PLANE_XP;
        self.objectives.guns = self.objectives.guns.saturating_add(gains.guns);
        self.experience += u32::from(gains.guns) * GUN_XP;
        self.objectives.vehicles = self.objectives.vehicles.saturating_add(gains.vehicles);
        self.experience += u32::from(gains.vehicles) * VEHICLE_XP;
        while self.experience >= self.next_level_xp() {
            self.experience -= self.next_level_xp();
            self.level += 1;
            self.pending_cards = self.pending_cards.saturating_add(1);
        }
        if self.stage == Stage::CrownDeep && objective == Some(LegEnd::RegentSunk) {
            self.phase = Phase::Won(LegEnd::RegentSunk);
        } else if self.stage != Stage::CrownDeep && objective == Some(LegEnd::BasesDestroyed) {
            self.finish_leg(LegEnd::BasesDestroyed);
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
    /// says which of its bays the blow got into. A light round does only part of its
    /// damage: see [`bore_share`].
    pub fn strike_side(
        &mut self,
        aft: f32,
        abeam: f32,
        damage: u16,
        calibre: Option<f32>,
        reach: shipvivor_shipyard::Reach,
    ) {
        if self.phase == Phase::Sailing {
            let damage = f32::from(damage) * bore_share(calibre);
            self.ship.strike_into(aft, abeam, damage, calibre, reach);
        }
    }
    /// A torpedo's hole below her waterline, `aft` metres behind the bow tip, which the
    /// sea pours into before the crew can shore it. The repair party drops whatever can
    /// wait and goes to it.
    pub fn breach(&mut self, aft: f32) {
        if self.phase == Phase::Sailing {
            self.ship.breach(aft);
            let urgent = self.ship.triage();
            if urgent.is_some() {
                self.ship.assign(urgent);
            }
        }
    }
    /// Total surviving structure, including any drydock sections.
    pub fn hull(&self) -> u16 {
        self.ship.hull().round() as u16
    }
    /// Plate, harden and pump the hull as her fittings say.
    pub(crate) fn refit_protection(&mut self) {
        self.ship
            .protect(self.fittings.protection(Protection::BREAKWATER));
        self.ship.fit_pumps(self.fittings.pumping());
    }
    pub fn hull_percent(&self) -> u16 {
        (self.ship.integrity() * 100.).round() as u16
    }
    /// Fit a card from the hand on offer, for one of the level-ups owed.
    pub fn install(&mut self, card: Card) {
        if !matches!(self.phase, Phase::Refit | Phase::Drydock(_))
            || self.pending_cards == 0
            || !self.offers().contains(&card)
        {
            return;
        }
        self.fittings = self.fittings.with(card);
        self.refit_protection();
        self.pending_cards -= 1;
        self.hands = self.hands.wrapping_add(1);
        // A ship in dock stays there until she is sailed, with [`Run::leave_dock`].
        if self.pending_cards == 0 && self.phase == Phase::Refit {
            self.phase = Phase::Sailing;
        }
    }
    /// The passages on the chart from the yard she is in: three seas of the next round's
    /// pool, drawn from the run seed and graded as [`GRADES`] says, or `None` once the
    /// next leg is the intercept.
    pub fn exits(&self) -> Option<[Exit; OFFERED]> {
        let seas = charted(self.seed, self.stage.round() + 1)?;
        Some(std::array::from_fn(|i| Exit::new(seas[i], GRADES[i])))
    }
    /// Sail from the chart into the next round, by a passage the leg just sailed has
    /// earned the stars for.
    pub fn depart(&mut self, next: Stage) -> bool {
        let Some(exit) = self
            .exits()
            .into_iter()
            .flatten()
            .find(|exit| exit.stage == next)
        else {
            return false;
        };
        if self.phase != Phase::Chart || self.stars() < exit.stars {
            return false;
        }
        self.previous_stars += self.stars();
        self.course.push(next);
        self.stage = next;
        self.grade = exit.grade;
        self.elapsed = 0.;
        self.new_leg();
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
impl Run {
    /// Bring down the last of her bases and end the leg, with nothing else gained.
    pub(crate) fn clear(&mut self) {
        self.tick_sector(0., Objectives::default(), 0, true);
    }
    /// The moderate passage the chart from her yard offers, always open.
    pub(crate) fn moderate_passage(&self) -> Stage {
        self.exits()
            .expect("a yard before the last charts passages")[0]
            .stage
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stars_and_wrecked_yards_pay_the_yard_once_and_unspent_points_sail_on() {
        for combat_stars in 0..=2 {
            for late in [false, true] {
                let mut run = Run::default();
                let goals = run.stage.mission();
                // Every yard comes down for a clearance.
                let yards = 4;
                let gains = Objectives {
                    kills: if combat_stars >= 1 { goals.kills } else { 0 },
                    crates: if combat_stars >= 2 { goals.crates } else { 0 },
                    yards,
                    ..Default::default()
                };
                // The clock ends nothing and takes no star: she may sail on past it,
                // and only forgoes what it pays. Each yard wrecked puts time back on it.
                let clock = run.clock() + BASE_TIME * f32::from(yards);
                let time = if late { clock + 1. } else { 1. };
                run.tick_sector(time, gains, 0, false);
                assert_eq!(run.clock(), clock);
                assert_eq!((run.phase(), run.late()), (Phase::Sailing, late));
                run.tick_sector(0., Objectives::default(), 0, true);
                let end = LegEnd::BasesDestroyed;
                assert_eq!(run.phase(), Phase::Drydock(end));
                let stars = combat_stars + 1;
                assert_eq!((run.goals_met(), run.stars()), (stars, stars));
                assert_eq!(run.beat_clock(), !late);
                // Every anchorage pays the open-water rate, and the clock a star's worth.
                assert_eq!(run.star_points(), STAR_POINTS);
                let clock_points = if late { 0 } else { STAR_POINTS };
                assert_eq!(run.clock_points(), clock_points);
                let earned = STAR_POINTS * u16::from(stars) + YARD_POINTS * yards + clock_points;
                assert_eq!((run.points(), run.leg_points()), (earned, earned));
                // Nothing is spliced in for her, and no hand is owed: the points buy her refits.
                run.tick_sector(1., gains, 0, true);
                assert_eq!(run.points(), earned);
                assert_eq!(run.dock_modules().count(), 0);
                assert_eq!(run.ship.design().length(), 30.);
                assert_eq!(run.ship.compartments().len(), 6);
                assert_eq!(run.hull_percent(), 100);
                assert_eq!(run.fittings_remaining(), 0);
                // She may sail without spending them, and they sail with her.
                assert!(run.leave_dock());
                assert!(run.depart(run.moderate_passage()));
                assert_eq!(run.points(), earned);
                // The second leg pays too, into a second yard, where the points carried
                // from the first are added to its own.
                run.tick_sector(
                    1.,
                    Objectives {
                        yards: 3,
                        ..Default::default()
                    },
                    0,
                    true,
                );
                assert_eq!(run.phase(), Phase::Drydock(LegEnd::BasesDestroyed));
                // A new leg starts on time: its drydock star and its clock both pay, at
                // the moderate passage's rate.
                assert!(run.beat_clock());
                let both = earned + 2 * GRADE_STAR_POINTS[0] + 3 * YARD_POINTS;
                assert_eq!(run.points(), both);
                assert!(!run.engage_regent());
                assert!(run.refit_in_dock(DockJob::Fitting(Card::Loader)));
                assert_eq!(run.points(), both - 2);
                // Each yard after charts the next round, until the last patrol's yard
                // sails her on to intercept the Regent. Each clearance of a moderate
                // passage pays its star, and its clock, at the moderate rate.
                let mut points = both - 2;
                for round in 2..PATROLS {
                    assert!(run.leave_dock());
                    assert_eq!(run.phase(), Phase::Chart);
                    assert!(!run.engage_regent());
                    let moderate = run.exits().unwrap()[0];
                    assert_eq!(moderate.grade, Grade::Moderate);
                    assert!(run.depart(moderate.stage));
                    assert_eq!(run.stage.round(), round + 1);
                    run.tick_sector(1., Objectives::default(), 0, true);
                    points += 2 * GRADE_STAR_POINTS[0];
                    assert_eq!(run.points(), points);
                }
                assert!(run.leave_dock());
                assert_eq!(run.phase(), Phase::Intercept(LegEnd::BasesDestroyed));
                assert!(run.engage_regent());
                assert_eq!((run.stage, run.points()), (Stage::CrownDeep, points));
                assert_eq!(run.course.len(), PATROLS - 1);
            }
        }
    }

    /// Re-seed the run until its next hand deals `card`.
    fn seed_dealing(run: &mut Run, card: Card) {
        assert!(run.fittings.can_fit(card), "{card:?} cannot be dealt");
        while !run.offers().contains(&card) {
            run.seed += 1;
        }
    }

    #[test]
    fn hands_stand_until_a_card_is_taken_and_follow_the_run_seed() {
        let levelled = |seed| {
            let mut run = Run {
                seed,
                ..Run::default()
            };
            run.tick(
                0.,
                Objectives {
                    crates: 18,
                    ..Default::default()
                },
                0,
            );
            run
        };
        // 180 XP is four levels, each a hand.
        let mut run = levelled(7);
        assert_eq!((run.phase(), run.fittings_remaining()), (Phase::Refit, 4));
        let mut hands = Vec::new();
        while run.fittings_remaining() > 0 {
            let hand = run.offers();
            assert_eq!(run.offers(), hand, "a hand stands until a card is taken");
            assert_eq!(hand.len(), HAND);
            run.install(hand[HAND - 1]);
            hands.push(hand);
        }
        assert_eq!(run.phase(), Phase::Sailing);
        // The same seed and the same choices deal the same hands.
        let mut again = levelled(7);
        for hand in &hands {
            assert_eq!(&again.offers(), hand);
            again.install(hand[HAND - 1]);
        }
        assert_eq!(again.fittings, run.fittings);
        // Another seed deals another hand.
        assert!((8..16).any(|seed| levelled(seed).offers() != hands[0]));
    }

    #[test]
    fn a_hand_deals_different_cards_that_fit_and_rare_ones_least_often() {
        use Rarity::*;
        let stock = Fittings::default();
        let deals = 30_000;
        let mut first = [0_u32; 3];
        for key in 0..deals {
            let hand = stock.deal(key);
            assert_eq!(hand.len(), HAND);
            assert!(hand.iter().all(|&card| stock.can_fit(card)), "{hand:?}");
            assert!(hand[0] != hand[1] && hand[1] != hand[2] && hand[0] != hand[2]);
            first[hand[0].rarity().unwrap() as usize] += 1;
        }
        // The stock ship can take eleven common cards, four uncommon and one rare, so the
        // first card is common 66 times in 79, uncommon 12 and rare once.
        let weights = [Common, Uncommon, Rare].map(|rarity| {
            DECK.iter()
                .filter(|&&(card, of)| of == rarity && stock.can_fit(card))
                .map(|_| rarity.weight())
                .sum::<u32>()
        });
        assert_eq!(weights, [66, 12, 1]);
        for (seen, weight) in first.into_iter().zip(weights) {
            let share = f64::from(seen) / deals as f64;
            let expected = f64::from(weight) / 79.;
            assert!((share - expected).abs() < 0.01, "{first:?}");
        }
        // Fewer are dealt when fewer fit.
        let pair = vec![(Card::Loader, Common), (Card::TwinSecondaries, Rare)];
        assert_eq!(draw(pair, 3).len(), 2);
        assert!(draw(Vec::new(), 3).is_empty());
    }

    #[test]
    fn ammunition_cards_change_distinct_properties_and_never_offer_a_spent_rank() {
        let dealt = |fit: Fittings| (0..400).flat_map(|key| fit.deal(key)).collect::<Vec<_>>();
        let stock = Fittings::default();
        // The bore is yard work. Caps and velocity both come in a hand.
        let hands = dealt(stock);
        assert!(hands.contains(&Card::Caps) && hands.contains(&Card::Velocity));
        assert!(!hands.contains(&Card::Shells));
        let heavy = stock.with(Card::Shells).round();
        assert!(heavy.calibre_inches() > stock.round().calibre_inches());
        assert_eq!(heavy.speed(), stock.round().speed());
        // Caps drive the round through heavier plate and change nothing else.
        let capped = stock.with(Card::Caps);
        assert!(capped.round().penetration() > stock.round().penetration());
        assert_eq!(capped.round().breach_force(), stock.round().breach_force());
        assert_eq!(capped.round().reach(), stock.round().reach());
        // Velocity carries it farther and harder.
        let fast = stock.with(Card::Velocity).round();
        assert!(fast.speed() > stock.round().speed());
        assert!(fast.penetration() > stock.round().penetration());
        let max = Fittings {
            caps: limits::CAPS,
            shells: 4,
            velocity: 4,
            ..stock
        };
        assert_eq!(max.ranks(Card::Caps), Some((limits::CAPS, limits::CAPS)));
        assert_eq!(max.with(Card::Caps).round(), max.round());
        assert!((0..50).all(|key| max.deal(key).len() == HAND));
        assert!(
            dealt(max)
                .iter()
                .all(|card| !matches!(card, Card::Caps | Card::Shells))
        );
    }
    #[test]
    fn reload_grows_with_calibre_and_quick_loaders_cut_it_back() {
        let stock = Fittings::default();
        assert!((stock.reload() - 2.5).abs() < 1e-4);
        // Each inch of bore costs more loading time than the inch before it.
        let mut per_inch = 0.;
        for (rank, pair) in CALIBRES.windows(2).enumerate() {
            let now = Fittings {
                shells: rank as u16,
                beam: ShipBeam::Battleship,
                ..stock
            };
            let step =
                (now.with(Card::Shells).reload() - now.reload()) / f32::from(pair[1] - pair[0]);
            assert!(
                step > per_inch,
                "{step} s an inch for {pair:?}, after {per_inch}"
            );
            per_inch = step;
        }
        let thirty = MainShell {
            bore: 30 - CALIBRES[0],
            ..MainShell::STOCK
        };
        assert!((thirty.load_seconds() - 30.).abs() < 1e-3);
        let eight = Fittings {
            shells: 2,
            beam: ShipBeam::HeavyCruiser,
            ..stock
        };
        assert!((5.5..6.5).contains(&eight.reload()), "{}", eight.reload());
        assert!(eight.with(Card::Loader).reload() < eight.reload());
        // Every rank takes the same share off, and four come to about half.
        for rank in 0..6 {
            let (now, next) = (
                Fittings {
                    loader: rank,
                    ..eight
                },
                Fittings {
                    loader: rank + 1,
                    ..eight
                },
            );
            assert!((next.reload() / now.reload() - (1. - LOADER_CUT)).abs() < 1e-4);
        }
        let refit = Fittings { loader: 4, ..eight };
        assert!((0.5..0.55).contains(&(refit.reload() / eight.reload())));
        // Each bore aboard loads at its own pace under the same drill.
        assert!((refit.reload_for(8) - refit.reload()).abs() < 1e-4);
        assert!(refit.reload_for(4) < refit.reload_for(6));
    }

    #[test]
    fn secondary_cards_mount_arm_and_drill_the_close_range_battery_up_to_their_limits() {
        use Card::*;
        let close = [
            MoreSecondaries,
            HeavySecondaries,
            QuickSecondaries,
            SecondaryRange,
            TwinSecondaries,
        ];
        let stock = Fittings::default();
        assert_eq!(stock.secondaries(), Secondaries::STOCK);
        // Every close-range card is dealt while it can still improve the battery.
        let hands: Vec<_> = (0..400).flat_map(|key| stock.deal(key)).collect();
        assert!(close.iter().all(|card| hands.contains(card)));
        let full = [
            (MoreSecondaries, 2),
            (HeavySecondaries, limits::WING_SHELLS),
            (QuickSecondaries, limits::WING_LOADERS),
            (SecondaryRange, limits::WING_RANGE),
            (TwinSecondaries, limits::WING_BARRELS),
        ]
        .into_iter()
        .flat_map(|(card, ranks)| std::iter::repeat_n(card, usize::from(ranks)))
        .fold(stock, Fittings::with);
        let fit = full.secondaries();
        // The two she was built with a side and two more, five more damage a hit, and
        // four barrels on every mount.
        assert_eq!((fit.per_side, fit.damage, fit.barrels), (4, 6, 4));
        assert!((fit.range - 360.).abs() < 1e-5);
        // Six ranks of loaders, each taking the same share off.
        assert!((fit.reload - Secondaries::STOCK.reload * drilled(6)).abs() < 1e-5);
        assert!((drilled(3) - 0.85_f32.powi(3)).abs() < 1e-6);
        // Only her deck room stops another pair, and a longer hull has more: see
        // `a_full_deck_waits_for_a_longer_hull_before_mounting_more_secondaries`.
        assert!(
            (0..400)
                .flat_map(|key| full.deal(key))
                .all(|card| card == MoreSecondaries || !close.contains(&card))
        );
    }
    #[test]
    fn the_ranks_the_yard_shows_end_where_a_card_stops_fitting() {
        use Card::*;
        // The broadest hull carries the most plate.
        let stock = Fittings {
            beam: ShipBeam::Battleship,
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
            Pumps,
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
    fn machinery_and_protection_cards_stop_at_their_limits_and_reach_the_ship_at_once() {
        use shipvivor_shipyard::Component;
        let stock = Fittings::default();
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
            assert!(
                (0..200).all(|key| !fit.deal(key).contains(&card)),
                "{card:?}"
            );
        }
        // A larger bore trains more slowly, and training gear wins it back.
        let heavy = Fittings {
            shells: 2,
            beam: ShipBeam::HeavyCruiser,
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
        seed_dealing(&mut run, Card::Armor);
        let plate = run.ship.design().plate_strength(1);
        run.install(Card::Armor);
        assert!(run.ship.design().plate_strength(1) > plate);
        for _ in 0..limits::HARDENING {
            run.fittings = run.fittings.with(Card::Hardening);
        }
        run.tick(0.1, Objectives::default(), 0);
        assert_eq!(
            run.ship.design().strength(Component::Engine),
            Some(run.ship.design().module_structure(3))
        );
    }
    #[test]
    fn pumps_stop_at_their_last_rank_and_reach_her_pumping_as_they_are_fitted() {
        assert_eq!(Card::Pumps.rarity(), Some(Rarity::Common));
        let mut run = Run::default();
        assert_eq!(run.ship.design().pumps(), 1.);
        // Crates' experience until a hand is owed.
        let level_up = |run: &mut Run| {
            while run.fittings_remaining() == 0 {
                let crates = Objectives {
                    crates: 3,
                    ..Default::default()
                };
                run.tick(0., crates, 0);
            }
        };
        for rank in 1..=limits::PUMPS {
            level_up(&mut run);
            seed_dealing(&mut run, Card::Pumps);
            run.install(Card::Pumps);
            assert_eq!(run.fittings.ranks(Card::Pumps), Some((rank, limits::PUMPS)));
            assert_eq!(run.ship.design().pumps(), 1. + 0.5 * f32::from(rank));
        }
        // Another rank is neither fitted nor dealt.
        assert!(!run.fittings.can_fit(Card::Pumps));
        assert_eq!(run.fittings.with(Card::Pumps), run.fittings);
        assert!((0..200).all(|key| !run.fittings.deal(key).contains(&Card::Pumps)));
        level_up(&mut run);
        run.install(run.offers()[0]);
        assert_eq!(
            run.ship.design().pumps(),
            1. + 0.5 * f32::from(limits::PUMPS)
        );

        // The yard sells them in the engine bay, and she sails with them.
        let mut run = Run::default();
        run.clear();
        run.points = 2;
        let pumps = DockJob::Fitting(Card::Pumps);
        assert!(run.section_jobs(3).contains(&pumps) && pumps.cost() == 2);
        assert!(run.refit_in_dock(pumps));
        assert_eq!((run.points(), run.ship.design().pumps()), (0, 1.5));
        assert!(run.leave_dock() && run.depart(run.moderate_passage()));
        run.tick(1., Objectives::default(), 0);
        assert_eq!(run.ship.design().pumps(), 1.5);
    }

    #[test]
    fn she_is_built_with_a_pair_of_aa_guns_and_the_cards_add_pairs_and_drill_them() {
        let stock = Fittings::default();
        assert_eq!(stock.anti_air(), AntiAir::STOCK);
        assert_eq!(stock.anti_air().mounts, 2);
        for upgrade in [Card::MoreAntiAir, Card::HeavyAntiAir, Card::QuickAntiAir] {
            assert!(stock.can_fit(upgrade));
            assert!(DECK.iter().any(|&(card, _)| card == upgrade));
        }
        assert_eq!(stock.with(Card::MoreAntiAir).anti_air().mounts, 4);
        // Her deck room, which a longer hull adds to, stops the mounts well short of this:
        // see `a_longer_hull_makes_room_for_more_aa_and_a_plane_section_for_more_planes`.
        let full = [
            (Card::MoreAntiAir, limits::AA_MOUNTS),
            (Card::HeavyAntiAir, limits::AA_SHELLS),
            (Card::QuickAntiAir, limits::AA_LOADERS),
        ]
        .into_iter()
        .flat_map(|(card, ranks)| std::iter::repeat_n(card, usize::from(ranks)))
        .fold(stock, Fittings::with);
        let aa = full.anti_air();
        assert_eq!((aa.mounts, aa.damage), (AntiAir::MOST_MOUNTS, 6));
        assert!((aa.reload - AntiAir::STOCK.reload * drilled(6)).abs() < 1e-5);
        for upgrade in [Card::MoreAntiAir, Card::HeavyAntiAir, Card::QuickAntiAir] {
            assert!(!full.can_fit(upgrade));
        }
    }

    #[test]
    fn each_plane_shot_down_pays_its_experience_toward_the_next_level() {
        let mut run = Run::default();
        let downed = |planes| Objectives {
            planes,
            ..Default::default()
        };
        run.tick(1., downed(3), 0);
        assert_eq!((run.experience, run.objectives.planes), (3 * PLANE_XP, 3));
        assert_eq!(run.objectives.kills, 0, "an aircraft is no raider");
        // Enough of them make a level, and a hand to choose from.
        let short = run.next_level_xp() - run.experience;
        run.tick(1., downed(short.div_ceil(PLANE_XP) as u16), 0);
        assert_eq!((run.level, run.phase()), (2, Phase::Refit));
    }

    #[test]
    fn a_torpedo_boat_sunk_pays_experience_and_is_no_raider() {
        let mut run = Run::default();
        // The step counts every ship sunk, a destroyer and two torpedo boats among them.
        let sunk = |kills| Objectives {
            kills,
            ..Default::default()
        };
        run.tick(1., sunk(3), 0);
        run.sink_torpedo_boats(2);
        assert_eq!(run.objectives.kills, 1);
        assert_eq!(run.experience, 2 * TORPEDO_BOAT_XP);
        // Enough of them make a level, dealt on the next step.
        let short = run.next_level_xp() - run.experience;
        let boats = short.div_ceil(TORPEDO_BOAT_XP) as u16;
        run.tick(1., sunk(boats), 0);
        run.sink_torpedo_boats(boats);
        assert_eq!(run.objectives.kills, 1);
        run.tick(1., Objectives::default(), 0);
        assert_eq!((run.level, run.phase()), (2, Phase::Refit));
        // Once the leg is over, a boat that went down with it counts for nothing.
        let mut run = Run::default();
        run.tick_sector(1., sunk(1), 0, true);
        let (kills, experience) = (run.objectives.kills, run.experience);
        run.sink_torpedo_boats(1);
        assert_eq!((run.objectives.kills, run.experience), (kills, experience));
    }

    #[test]
    fn the_bore_and_the_barrels_are_bought_in_the_yard_and_never_dealt() {
        use Card::*;
        assert_eq!(
            [Shells, TripleTurrets, QuadTurrets].map(Card::rarity),
            [None; 3]
        );
        for fit in [
            Fittings::default(),
            Fittings {
                beam: ShipBeam::Battleship,
                ..Fittings::default()
            },
            Fittings::default().with(TripleTurrets),
        ] {
            assert!(
                (0..300)
                    .flat_map(|key| fit.deal(key))
                    .all(|card| card.rarity().is_some())
            );
        }
        // The yard sells barrels turret by turret, a twin then a triple then a quad, and
        // they sail with her.
        let mut run = Run::default();
        run.clear();
        run.points = 10;
        assert!(!run.refit_in_dock(DockJob::Fitting(TripleTurrets)));
        let at = run.ship.design().gun_stations()[0].berth.module;
        let barrels = |run: &Run| run.ship.design().gun_stations()[0].gunhouse.barrels;
        for (to, left) in [
            (TurretLayout::Twin, 8),
            (TurretLayout::Triple, 5),
            (TurretLayout::Quad, 1),
        ] {
            let from = barrels(&run);
            assert!(run.refit_in_dock(DockJob::Barrels { at, from, to }));
            assert_eq!((barrels(&run), run.points()), (to, left));
        }
        assert_eq!(TurretLayout::Quad.more(), None);
        assert!(run.leave_dock());
        assert!(run.depart(run.moderate_passage()));
        assert_eq!(barrels(&run), TurretLayout::Quad);
        assert_eq!(
            run.ship.design().gun_stations()[1].gunhouse.barrels,
            TurretLayout::Single
        );
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
        for _ in 0..24 {
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
        // Twice the ranks of half the plate each: the same thickness at each beam.
        assert_eq!(most(ShipBeam::Escort), (4, 7.));
        assert_eq!(most(ShipBeam::HeavyCruiser), (8, 13.));
        assert_eq!(most(ShipBeam::Battleship), (limits::ARMOR, 19.));
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
        // A cruiser's 7.6-inch round, still a light one, does about four fifths of its 4.
        run.strike_side(7.5, -2., 4, Some(7.6), bays);
        assert_eq!(run.hull(), hull - 3);
    }

    #[test]
    fn light_rounds_do_part_of_their_damage_and_heavy_ones_all_of_it() {
        assert_eq!(bore_share(Some(2.8)), 0.5);
        assert!((bore_share(Some(4.7)) - 0.62).abs() < 0.01);
        assert_eq!(bore_share(Some(12.6)), 1.);
        assert_eq!(bore_share(None), 1.);
        // A patrol boat's 7 on sound plate does a little over 4.
        let mut run = Run::default();
        let hull = run.hull();
        run.strike_side(7.5, 2., 7, Some(4.7), shipvivor_shipyard::Reach::Bays);
        assert_eq!(run.hull(), hull - 4);
    }

    #[test]
    fn a_torpedo_draws_the_party_to_the_hole_and_slows_her_until_it_is_pumped_out() {
        let mut run = Run::default();
        let gun = shipvivor_shipyard::Berth {
            module: 2,
            index: 1,
        };
        run.ship.strike_fitting(gun, 500.);
        run.tick(1., Objectives::default(), 0);
        assert_eq!(
            run.ship.party(),
            Some(shipvivor_shipyard::Task::Fitting(gun))
        );
        // A torpedo forward, as a torpedo boat's lands: its blow, and the sea let in.
        run.strike_side(7.5, 2., 16, None, shipvivor_shipyard::Reach::Bays);
        run.breach(7.5);
        assert_eq!(run.ship.party(), Some(shipvivor_shipyard::Task::Module(1)));
        let mut slowest = 1_f32;
        for _ in 0..120 {
            run.tick(1., Objectives::default(), 0);
            slowest = slowest.min(run.ship.propulsion());
        }
        let hold = &run.ship.compartments()[1];
        assert!(slowest < 0.85, "{slowest}");
        assert!(!hold.wrecked() && hold.water() < 0.05, "{}", hold.water());
        assert!(run.ship.propulsion() > 0.99);
    }

    #[test]
    fn the_dock_between_legs_leaves_a_battered_ship_short_of_whole() {
        let mut run = Run::default();
        run.strike(7.5, 18);
        run.strike(22.5, 18);
        run.tick(60., Objectives::default(), 45);
        let before = run.hull();
        run.clear();
        assert!(before + DOCK_REPAIR < MAX_HULL, "{before} hull");
        assert_eq!(run.hull(), before + DOCK_REPAIR);
        assert!(run.ship.compartments().iter().all(|hold| !hold.wrecked()));
        // The second yard repairs her too.
        assert!(run.leave_dock());
        assert!(run.depart(run.exits().unwrap()[0].stage));
        run.tick(60., Objectives::default(), 30);
        let battered = run.hull();
        run.clear();
        assert_eq!(run.phase(), Phase::Drydock(LegEnd::BasesDestroyed));
        let repaired = run.hull();
        assert!(repaired > battered && repaired <= battered + DOCK_REPAIR);
    }

    #[test]
    fn patrol_stars_unlock_only_earned_routes_and_fittings_travel() {
        let sunk = |kills, crates| Objectives {
            kills,
            crates,
            ..Default::default()
        };
        // Every patrol ends with her bases down, which is the drydock goal. A leg
        // finished past its clock keeps its stars and forgoes only the clock's points.
        for (goals, late, stars) in [
            (Objectives::default(), true, 1),
            (Objectives::default(), false, 1),
            (sunk(4, 1), false, 2),
            (sunk(4, 1), true, 2),
            (
                Objectives {
                    yards: 1,
                    ..sunk(4, 3)
                },
                false,
                3,
            ),
        ] {
            for exit in Run::default().exits().unwrap() {
                let (dest, required) = (exit.stage, exit.stars);
                let mut r = Run::default();
                r.tick(60., goals, 50);
                while r.phase == Phase::Refit {
                    r.install(r.offers()[0]);
                }
                if late {
                    r.tick(r.remaining() + 1., Objectives::default(), 0);
                }
                let fitted = r.fittings;
                let hull = r.hull();
                r.clear();
                assert!(r.leave_dock());
                assert_eq!(r.stars(), stars);
                assert_eq!(r.depart(dest), stars >= required);
                if stars >= required {
                    assert_eq!((r.stars(), r.grade), (0, exit.grade));
                    // The dock makes good 40 points on any route, and a moderate
                    // passage's yard 30 more.
                    let docked = if exit.grade == Grade::Moderate {
                        70
                    } else {
                        40
                    };
                    assert_eq!(r.hull(), (hull + docked).min(100));
                    assert_eq!(r.previous_stars, stars);
                    if exit.grade == Grade::Tough {
                        assert_eq!((r.phase, r.perk()), (Phase::Refit, Perk::FreeFitting));
                        let hand = r.offers();
                        r.install(hand[0]);
                        assert_eq!(r.fittings, fitted.with(hand[0]));
                    } else {
                        assert_eq!(r.perk(), Perk::Repair(30));
                        assert_eq!(r.fittings, fitted);
                    }
                    // Unspent points sail with her, the clock's among them if she beat it.
                    assert_eq!(
                        r.points(),
                        STAR_POINTS * (u16::from(stars) + u16::from(!late))
                            + YARD_POINTS * goals.yards
                    );
                    assert_eq!(r.experience, u32::from(goals.crates % 3) * CRATE_XP);
                    // The clock runs out and she sails on until her bases are down.
                    let deadline = f32::from(dest.mission().minutes) * 60.;
                    r.tick(deadline + 10., Objectives::default(), 0);
                    assert_eq!((r.phase, r.late()), (Phase::Sailing, true));
                    r.clear();
                    assert_eq!(r.phase, Phase::Drydock(LegEnd::BasesDestroyed));
                    // The second yard charts the third round, and a leg with only its
                    // drydock star opens only its moderate passage.
                    assert_eq!((r.stars(), r.beat_clock()), (1, false));
                    assert!(r.leave_dock());
                    assert_eq!(r.phase, Phase::Chart);
                    let third = r.exits().unwrap();
                    assert_eq!(third.map(|e| e.stage), charted(r.seed, 3).unwrap());
                    assert_eq!(third.map(|e| e.grade), GRADES);
                    assert!(!r.depart(third[1].stage) && !r.depart(third[2].stage));
                    assert!(!r.depart(POOLS[3][0]));
                    assert!(r.depart(third[0].stage));
                    assert_eq!(r.course, [dest, third[0].stage]);
                    assert_eq!(r.previous_stars, stars + 1);
                } else {
                    assert_eq!(r.phase, Phase::Chart);
                }
            }
        }
    }

    #[test]
    fn tough_passages_pay_more_a_star_and_every_anchorage_pays_the_same() {
        for (round, pool) in POOLS.iter().enumerate() {
            for sea in *pool {
                for grade in [Grade::Moderate, Grade::Tough] {
                    let pay = if round == 0 {
                        STAR_POINTS
                    } else {
                        grade.star_points()
                    };
                    assert_eq!(sea.star_points(grade), pay, "{sea:?}");
                }
            }
        }
        assert_eq!(Stage::CrownDeep.star_points(Grade::Tough), 0);
        assert!(GRADE_STAR_POINTS[0] < GRADE_STAR_POINTS[1]);
        assert_eq!(GRADE_STAR_POINTS[1], 2 * STAR_POINTS);
        // Three stars in a tough passage of the second round pay twelve, and the clock
        // four more. The same stars in a moderate one pay nine, and the clock three.
        for (grade, points) in [(Grade::Tough, 16), (Grade::Moderate, 12)] {
            let mut run = Run {
                stage: POOLS[1][0],
                grade,
                ..Run::default()
            };
            let goals = run.stage.mission();
            run.tick_sector(
                1.,
                Objectives {
                    kills: goals.kills,
                    crates: goals.crates,
                    ..Default::default()
                },
                0,
                true,
            );
            assert_eq!((run.stars(), run.points()), (3, points));
        }
    }

    #[test]
    fn a_run_is_six_short_patrols_forking_at_every_yard_and_then_the_regent() {
        let seas: Vec<Stage> = POOLS.concat();
        assert_eq!(seas.len(), 3 * 9 + 3 * 12);
        for (i, sea) in seas.iter().enumerate() {
            assert!(!seas[..i].contains(sea), "{sea:?} is in two pools");
            assert_ne!(*sea, Stage::CrownDeep);
            let names = seas
                .iter()
                .filter(|other| other.mission().name == sea.mission().name);
            assert_eq!(names.count(), 1, "{sea:?}");
        }
        assert_eq!(Stage::CrownDeep.round(), ROUNDS);
        // Each round asks at least as many sinkings as the round before it.
        let mut least = 0;
        for (round, pool) in POOLS.iter().enumerate() {
            for &sea in *pool {
                assert_eq!(sea.round(), round + 1);
                assert_eq!(sea.is_opening(), round == 0);
                // Only a tough passage flies enemy aircraft, and an anchorage never does.
                assert!(!sea.enters_air_cover(Grade::Moderate));
                assert_eq!(sea.enters_air_cover(Grade::Tough), round > 0, "{sea:?}");
                let mission = sea.mission();
                assert!((5..=6).contains(&mission.minutes), "{sea:?}");
                assert!(mission.kills >= least, "{sea:?}");
            }
            least = pool.iter().map(|sea| sea.mission().kills).min().unwrap();
        }
        assert!(!Stage::CrownDeep.enters_air_cover(Grade::Tough));

        // Every run charts three different seas of each round's pool, and its seed
        // decides which, so the best-paid way is not the same from one run to the next.
        let mut seen = vec![Vec::new(); PATROLS];
        for seed in 0..120 {
            assert_eq!(anchorages(seed), charted(seed, 1).unwrap());
            for round in 1..=PATROLS {
                let three = charted(seed, round).unwrap();
                assert!(three.iter().all(|sea| sea.round() == round), "{three:?}");
                assert!(three[0] != three[1] && three[1] != three[2] && three[0] != three[2]);
                // One of each fleet, so the two tough passages are never the same fight.
                let mut fleets = three.map(Stage::fleet).to_vec();
                fleets.sort_by_key(|&fleet| fleet as usize);
                assert_eq!(fleets, Fleet::ALL, "{three:?}");
                assert_eq!(charted(seed, round), Some(three));
                for sea in three {
                    if !seen[round - 1].contains(&sea) {
                        seen[round - 1].push(sea);
                    }
                }
            }
            assert!(charted(seed, 0).is_none() && charted(seed, ROUNDS).is_none());
        }
        for (seas, pool) in seen.iter().zip(POOLS) {
            assert_eq!(seas.len(), pool.len(), "{seen:?}");
        }
        for pool in POOLS {
            for fleet in Fleet::ALL {
                let held = pool.iter().filter(|sea| sea.fleet() == fleet).count();
                assert_eq!(held, pool.len() / OFFERED, "{fleet:?} in {pool:?}");
            }
        }
        // Each yard charts one moderate passage, always open, then two tough ones for
        // three stars that pay alike.
        let exits = Run::default().exits().unwrap();
        assert_eq!(exits.map(|e| e.grade), GRADES);
        assert_eq!(
            exits.map(|e| (e.stars, e.perk)),
            [
                (0, Perk::Repair(30)),
                (3, Perk::FreeFitting),
                (3, Perk::FreeFitting)
            ]
        );
        assert_eq!(exits[1].star_points(), exits[2].star_points());
        assert!(exits[1].under_air_cover() && !exits[0].under_air_cover());

        // The Regent fights under the air cover of the tough passage just sailed.
        let mut run = Run {
            stage: Stage::CrownDeep,
            grade: Grade::Tough,
            ..Run::default()
        };
        assert!(!run.under_air_cover());
        run.course = vec![POOLS[1][0], POOLS[PATROLS - 1][0]];
        assert!(run.under_air_cover());
        run.grade = Grade::Moderate;
        assert!(!run.under_air_cover());
        assert_eq!(run.perk(), Perk::None);
    }

    #[test]
    fn levels_owed_at_the_clearance_are_dealt_as_free_hands_before_the_yard_opens() {
        let mut r = Run::default();
        // Cleared late: the stores and drydock goals are met, and the clock pays nothing.
        r.tick_sector(
            600.,
            Objectives {
                crates: 8,
                ..Default::default()
            },
            0,
            true,
        );
        r.tick(10., Objectives::default(), 100);
        assert_eq!(r.hull(), 100);
        assert_eq!(r.elapsed, 600.);
        assert_eq!((r.goals_met(), r.stars()), (2, 2));
        assert_eq!((r.level, r.experience, r.next_level_xp()), (3, 10, 50));
        assert_eq!(r.phase, Phase::Drydock(LegEnd::BasesDestroyed));
        // Two levels are owed as hands, not points. The stars pay the yard.
        assert_eq!((r.fittings_remaining(), r.points()), (2, 2 * STAR_POINTS));
        // The yard and the chart both wait for the hands.
        assert!(!r.refit_in_dock(DockJob::Fitting(Card::Loader)) && !r.leave_dock());
        // A card the hand does not offer is not fitted.
        let hand = r.offers();
        let other = DECK
            .iter()
            .map(|&(card, _)| card)
            .find(|card| r.fittings.can_fit(*card) && !hand.contains(card))
            .unwrap();
        r.install(other);
        assert_eq!(
            (r.fittings, r.fittings_remaining()),
            (Fittings::default(), 2)
        );
        r.install(hand[0]);
        assert_eq!(r.phase, Phase::Drydock(LegEnd::BasesDestroyed));
        r.install(r.offers()[1]);
        assert_eq!((r.fittings_remaining(), r.points()), (0, 2 * STAR_POINTS));
        assert!(r.refit_in_dock(DockJob::Fitting(Card::Loader)));
        assert_eq!(r.points(), STAR_POINTS);
        assert!(matches!(r.phase, Phase::Drydock(_)) && r.leave_dock());
        assert_eq!(r.phase, Phase::Chart);
        let fitted = r.fittings;
        r.install(r.offers()[0]);
        assert_eq!(r.fittings, fitted);
    }

    #[test]
    fn clearing_banks_the_last_tick_and_pays_the_yard_once() {
        let mut r = Run::default();
        let cleared = Objectives {
            crates: 3,
            yards: 4,
            ..Default::default()
        };
        r.tick_sector(r.remaining(), cleared, 0, true);
        assert!(!r.late());
        assert_eq!(r.phase(), Phase::Drydock(LegEnd::BasesDestroyed));
        // One level owed as a hand. Two stars, four yards and the clock pay ten points.
        assert_eq!((r.stars(), r.level), (2, 2));
        assert_eq!((r.fittings_remaining(), r.points()), (1, 10));
        r.tick_sector(10., cleared, 100, true);
        assert_eq!((r.fittings_remaining(), r.points()), (1, 10));
        r.install(Card::QuadTurrets);
        assert_eq!(r.fittings_remaining(), 1);
        assert!(!r.depart(r.moderate_passage()));
        r.install(r.offers()[0]);
        assert!(r.leave_dock());
        assert_eq!(r.phase(), Phase::Chart);
        assert!(!r.depart(r.exits().unwrap()[1].stage));
        assert!(r.depart(r.moderate_passage()));
        assert_eq!((r.elapsed, r.fittings_remaining(), r.points()), (0., 0, 10));
        r.tick_sector(1., Objectives::default(), 0, true);
        assert_eq!(r.phase(), Phase::Drydock(LegEnd::BasesDestroyed));
        assert_eq!(r.points(), 10 + 2 * GRADE_STAR_POINTS[0]);
        assert!(r.leave_dock());
        assert_eq!(r.phase(), Phase::Chart);
    }

    #[test]
    fn partial_experience_crosses_legs_and_restarts_cleanly() {
        let mut run = Run::default();
        let stores = |crates| Objectives {
            crates,
            ..Default::default()
        };
        // 50 lost, 8 mended by the crew over the minute, and 8 from each crate.
        run.tick(60., stores(2), 50);
        assert_eq!(
            (run.phase, run.hull(), run.experience),
            (Phase::Sailing, 74, 20)
        );
        run.clear();
        assert!(run.leave_dock());
        assert!(run.depart(run.moderate_passage()));
        run.tick(1., stores(1), 0);
        assert_eq!((run.phase, run.level, run.experience), (Phase::Refit, 2, 0));
        run.install(run.offers()[0]);
        run.tick(1., stores(3), 0);
        assert_eq!(
            (run.phase, run.experience, run.next_level_xp()),
            (Phase::Sailing, 30, 40)
        );
        run = Run::default();
        assert_eq!((run.level, run.experience, run.fittings.shells), (1, 0, 0));
        assert_eq!(run.points(), 0);
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
    fn the_regent_must_sink_however_long_it_takes_and_breakwater_must_survive() {
        let finale = || {
            let mut run = Run::default();
            run.stage = POOLS[PATROLS - 1][0];
            run.tick_sector(1., Objectives::default(), 0, true);
            assert!(!run.engage_regent() && run.leave_dock());
            assert!(run.engage_regent());
            assert!(!run.engage_regent());
            run
        };
        // Her bases count for nothing here, and the clock runs out on nobody.
        let mut fight = finale();
        fight.tick_sector(61., Objectives::default(), 0, true);
        fight.tick(fight.remaining() + 120., Objectives::default(), 0);
        assert_eq!((fight.phase(), fight.late()), (Phase::Sailing, true));
        assert!(fight.ship.afloat());

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

    #[test]
    fn sunk_cargo_ranks_up_into_yard_points_apart_from_the_level_and_carries_over() {
        let mut run = Run::default();
        let cargo = |cargo| Objectives {
            cargo,
            ..Default::default()
        };
        // A freighter's worth, hit by hit, and the rest as she goes down.
        run.tick(1., cargo(0.75), 0);
        assert_eq!((run.cargo_points(), run.cargo_progress()), (0, 0.75));
        run.tick(1., cargo(0.25), 0);
        assert_eq!((run.cargo_points(), run.cargo_progress()), (1, 0.));
        assert_eq!((run.experience, run.leg_points()), (0, 1));
        // Two coasters and some of a third: experience for the two, and no raiders.
        run.tick(
            1.,
            Objectives {
                coasters: 2,
                cargo: 0.625,
                ..Default::default()
            },
            0,
        );
        assert_eq!((run.cargo_points(), run.cargo_progress()), (1, 0.625));
        assert_eq!(run.experience, 2 * COASTER_XP);
        assert_eq!((run.objectives.kills, run.objectives.coasters), (0, 2));
        // The whole point is banked at the yard with the stars. The rest sails on.
        run.clear();
        assert_eq!(run.phase(), Phase::Drydock(LegEnd::BasesDestroyed));
        assert_eq!(run.clock_points(), STAR_POINTS);
        assert_eq!(
            run.points(),
            1 + STAR_POINTS * u16::from(run.stars()) + run.clock_points()
        );
        let banked = run.points();
        assert!(run.leave_dock());
        assert!(run.depart(run.moderate_passage()));
        assert_eq!((run.cargo_points(), run.cargo_progress()), (0, 0.625));
        run.tick(1., cargo(0.375 + 2.), 0);
        assert_eq!((run.cargo_points(), run.cargo_progress()), (3, 0.));
        run.tick(1., Objectives::default(), 0);
        assert_eq!(run.leg_points(), 3);
        assert_eq!(run.points(), banked);
    }
}
