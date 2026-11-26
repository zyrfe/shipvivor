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

/// The biggest bore a turret with these barrels can carry on this beam, in a wing
/// gallery or on the centreline. Zero is a cradle too heavy for the beam even at four
/// inches. A single gun carries no more than a twin.
pub fn cradle(beam: ShipBeam, paired: bool, barrels: TurretLayout) -> u16 {
    let limits = match (beam, paired) {
        (ShipBeam::Escort, false) => [6, 6, 4, 4],
        (ShipBeam::Escort, true) => [4, 4, 0, 0],
        (ShipBeam::Cruiser, false) => [12, 12, 8, 6],
        (ShipBeam::Cruiser, true) => [6, 6, 4, 4],
        (ShipBeam::Capital, false) => [18, 18, 15, 12],
        (ShipBeam::Capital, true) => [8, 8, 8, 6],
    };
    limits[barrels.count() - 1]
}

/// Classes of beam, narrowest first.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ShipBeam {
    #[default]
    Escort,
    Cruiser,
    Capital,
}

impl ShipBeam {
    pub const ALL: [Self; 3] = [Self::Escort, Self::Cruiser, Self::Capital];
    pub fn metres(self) -> f32 {
        match self {
            Self::Escort => 5.5,
            Self::Cruiser => 7.5,
            Self::Capital => 9.5,
        }
    }
    /// The shortest hull that may carry this beam. Each keeps her at least 4.2 beams
    /// long, so her bow stays fine enough to part the water at speed.
    pub fn min_length(self) -> f32 {
        match self {
            Self::Escort => 0.,
            Self::Cruiser => 35.,
            Self::Capital => 40.,
        }
    }
    /// The most modules, bow and stern included, a hull this broad may run to. She
    /// is widened before she is lengthened further, so she never grows long and thin.
    pub const fn most_modules(self) -> usize {
        match self {
            Self::Escort => 8,
            Self::Cruiser => 11,
            Self::Capital => 14,
        }
    }
    /// The most main turrets a hull this broad carries. A capital beam has room for
    /// double turrets abreast, and the stability for two more turrets in all.
    pub const fn most_turrets(self) -> usize {
        match self {
            Self::Escort | Self::Cruiser => crate::MAX_GUN_STATIONS,
            Self::Capital => 6,
        }
    }
    /// The most ranks of armor plate a hull this broad can carry. Each step of beam
    /// gives her the stability for two more ranks of heavier side plate.
    pub const fn most_armor(self) -> u16 {
        match self {
            Self::Escort => 2,
            Self::Cruiser => 4,
            Self::Capital => 6,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Escort => "Escort",
            Self::Cruiser => "Cruiser",
            Self::Capital => "Capital",
        }
    }
    pub fn next(self) -> Option<Self> {
        match self {
            Self::Escort => Some(Self::Cruiser),
            Self::Cruiser => Some(Self::Capital),
            Self::Capital => None,
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
