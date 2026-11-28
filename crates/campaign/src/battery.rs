//! Authored fitting limits for the prototype. Beam, cradle and number of barrels
//! together determine which guns fit at an individual station.
use crate::Fittings;
use shipvivor_artillery::TurretLayout;
use shipvivor_shipyard::GunStation;

pub const CALIBRES: [u16; 6] = [4, 6, 8, 12, 15, 18];

/// The rank of the biggest calibre no wider than `inches`.
fn shells_for(inches: u16) -> u16 {
    CALIBRES.iter().rposition(|&c| c <= inches).unwrap_or(0) as u16
}

/// The biggest bore a turret with these barrels can carry on this hull, in a wing
/// gallery or on the centreline. Zero is a cradle too heavy for the hull even at four
/// inches. A single gun carries no more than a twin.
pub fn cradle(beam: ShipBeam, paired: bool, barrels: TurretLayout) -> u16 {
    let limits = match (beam, paired) {
        (ShipBeam::Escort, false) => [6, 6, 4, 4],
        (ShipBeam::Escort, true) => [4, 4, 0, 0],
        (ShipBeam::HeavyCruiser, false) => [12, 12, 8, 6],
        (ShipBeam::HeavyCruiser, true) => [6, 6, 4, 4],
        (ShipBeam::Battleship, false) => [18, 18, 15, 12],
        (ShipBeam::Battleship, true) => [8, 8, 8, 6],
        (ShipBeam::HeavyBattleship, false) => [18, 18, 18, 15],
        (ShipBeam::HeavyBattleship, true) => [12, 12, 8, 8],
        (ShipBeam::UltraHeavyBattleship, false) => [18, 18, 18, 18],
        (ShipBeam::UltraHeavyBattleship, true) => [15, 15, 12, 12],
    };
    limits[barrels.count() - 1]
}

/// The classes of hull the yard builds, narrowest first. She is built an escort, and the
/// yard rebuilds her a class at a time: each is broader in the beam than the last, and
/// carries heavier guns, thicker plate and more of everything.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ShipBeam {
    #[default]
    Escort,
    HeavyCruiser,
    Battleship,
    HeavyBattleship,
    /// The broadest of all: as long as a battleship and far broader, built to carry
    /// eight-inch secondaries beside the heaviest main battery.
    UltraHeavyBattleship,
}

impl ShipBeam {
    pub const ALL: [Self; 5] = [
        Self::Escort,
        Self::HeavyCruiser,
        Self::Battleship,
        Self::HeavyBattleship,
        Self::UltraHeavyBattleship,
    ];
    /// The broadest hull the yard builds.
    pub const BROADEST: Self = Self::UltraHeavyBattleship;
    pub fn metres(self) -> f32 {
        match self {
            Self::Escort => 5.5,
            Self::HeavyCruiser => 7.5,
            Self::Battleship => 9.5,
            Self::HeavyBattleship => 11.5,
            Self::UltraHeavyBattleship => 14.,
        }
    }
    /// The shortest hull that may be rebuilt to this class. Each keeps her at least 4.2
    /// beams long, so her bow stays fine enough to part the water at speed.
    pub fn min_length(self) -> f32 {
        match self {
            Self::Escort => 0.,
            Self::HeavyCruiser => 35.,
            Self::Battleship => 40.,
            Self::HeavyBattleship => 50.,
            Self::UltraHeavyBattleship => 60.,
        }
    }
    /// The most modules, bow and stern included, a hull of this class may run to. She
    /// is rebuilt broader before she is lengthened further, so she never grows long and
    /// thin. A battleship already fills the basin, so a heavy battleship is no longer.
    pub const fn most_modules(self) -> usize {
        match self {
            Self::Escort => 8,
            Self::HeavyCruiser => 11,
            Self::Battleship | Self::HeavyBattleship | Self::UltraHeavyBattleship => 14,
        }
    }
    /// The most main turrets a hull of this class carries. A battleship has room for
    /// double turrets abreast and the stability for two more turrets in all, and a heavy
    /// battleship for two more again.
    pub const fn most_turrets(self) -> usize {
        match self {
            Self::Escort | Self::HeavyCruiser => crate::MAX_GUN_STATIONS,
            Self::Battleship => 6,
            Self::HeavyBattleship | Self::UltraHeavyBattleship => 8,
        }
    }
    /// The most ranks of armor plate a hull of this class can carry. Each step up to a
    /// battleship gives her the stability for eight more half-strength ranks of side
    /// plate. A heavy battleship carries a battleship's plate: nothing may turn the
    /// Regent's guns.
    pub const fn most_armor(self) -> u16 {
        match self {
            Self::Escort => 8,
            Self::HeavyCruiser => 16,
            Self::Battleship | Self::HeavyBattleship | Self::UltraHeavyBattleship => 24,
        }
    }
    /// The bore, in inches, of the secondary gunhouses a hull of this class carries, or
    /// `None` on one too narrow to carry gunhouses along her deck edge.
    pub const fn secondary_bore(self) -> Option<u16> {
        match self {
            Self::Escort | Self::HeavyCruiser => None,
            Self::Battleship => Some(4),
            Self::HeavyBattleship => Some(6),
            Self::UltraHeavyBattleship => Some(8),
        }
    }
    /// The class as the yard names her hull.
    pub fn name(self) -> &'static str {
        match self {
            Self::Escort => "Escort",
            Self::HeavyCruiser => "Heavy cruiser",
            Self::Battleship => "Battleship",
            Self::HeavyBattleship => "Heavy battleship",
            Self::UltraHeavyBattleship => "Ultra heavy battleship",
        }
    }
    /// The class in a word or two, for a tile's caption.
    pub fn short_name(self) -> &'static str {
        match self {
            Self::Escort => "Escort",
            Self::HeavyCruiser => "Hvy cruiser",
            Self::Battleship => "Battleship",
            Self::HeavyBattleship => "Hvy b'ship",
            Self::UltraHeavyBattleship => "Ultra b'ship",
        }
    }
    pub fn next(self) -> Option<Self> {
        match self {
            Self::Escort => Some(Self::HeavyCruiser),
            Self::HeavyCruiser => Some(Self::Battleship),
            Self::Battleship => Some(Self::HeavyBattleship),
            Self::HeavyBattleship => Some(Self::UltraHeavyBattleship),
            Self::UltraHeavyBattleship => None,
        }
    }
}

impl Fittings {
    pub fn calibre(&self) -> u16 {
        CALIBRES[usize::from(self.shells).min(CALIBRES.len() - 1)]
    }

    /// Maximum available calibre in inches. Zero denotes an oversize cradle even
    /// at the starting four-inch bore.
    pub fn capacity_for(&self, beam: ShipBeam, paired: bool) -> u16 {
        cradle(beam, paired, self.turrets)
    }

    /// The biggest gun the current ship can carry at any of its stations.
    pub fn capacity(&self) -> u16 {
        self.capacity_for(self.beam, self.wings_only)
    }

    fn sized_for(mut self, wings_only: bool) -> Self {
        self.wings_only = wings_only;
        if self.capacity() == 0 {
            self.turrets = TurretLayout::Single;
        }
        if self.calibre() > self.capacity() {
            self.shells = CALIBRES
                .iter()
                .position(|&inch| inch == self.capacity())
                .unwrap() as u16;
        }
        self
    }

    /// These fittings as they stand at one turret: its own barrels and bore, as far
    /// as its cradle carries them.
    pub fn for_station(self, station: &GunStation) -> Self {
        Self {
            turrets: station.gunhouse.barrels,
            shells: shells_for(station.gunhouse.bore),
            ..self
        }
        .sized_for(station.paired())
    }

    /// The ship's own figures for her turrets as fitted: the biggest bore aboard, and
    /// the lightest full-bore turret, which sets the biggest bore any turret could take.
    pub(crate) fn on_deck(mut self, beam: ShipBeam, stations: &[GunStation]) -> Self {
        self.beam = beam;
        let wings_only = stations.iter().all(GunStation::paired);
        self.turrets = stations
            .iter()
            .filter(|s| wings_only || !s.paired())
            .map(|s| s.gunhouse.barrels)
            .min()
            .unwrap_or_default();
        let fitted = self;
        self.shells = stations
            .iter()
            .map(|s| fitted.for_station(s).shells)
            .max()
            .unwrap_or(0);
        self.sized_for(wings_only)
    }
}
