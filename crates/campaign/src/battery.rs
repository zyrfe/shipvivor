//! Authored fitting limits for the prototype. Beam, cradle and number of barrels
//! together determine which guns fit at an individual station.
use crate::Fittings;
use shipvivor_artillery::TurretLayout;
use shipvivor_shipyard::GunStation;

pub const CALIBRES: [u16; 6] = [4, 6, 8, 12, 15, 18];

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
        let limits = match (beam, paired) {
            (ShipBeam::Escort, false) => [6, 4, 4],
            (ShipBeam::Escort, true) => [4, 0, 0],
            (ShipBeam::Cruiser, false) => [12, 8, 6],
            (ShipBeam::Cruiser, true) => [6, 4, 4],
            (ShipBeam::Capital, false) => [18, 15, 12],
            (ShipBeam::Capital, true) => [8, 8, 6],
        };
        limits[self.turrets.count() - 2]
    }

    /// The biggest gun the current ship can carry at any of its stations.
    pub fn capacity(&self) -> u16 {
        self.capacity_for(self.beam, self.wings_only)
    }

    fn sized_for(mut self, wings_only: bool) -> Self {
        self.wings_only = wings_only;
        if self.capacity() == 0 {
            self.turrets = TurretLayout::Twin;
        }
        if self.calibre() > self.capacity() {
            self.shells = CALIBRES
                .iter()
                .position(|&inch| inch == self.capacity())
                .unwrap() as u16;
        }
        self
    }

    pub fn for_station(self, station: &GunStation) -> Self {
        self.sized_for(station.paired())
    }

    pub(crate) fn on_deck(mut self, beam: ShipBeam, wings_only: bool) -> Self {
        self.beam = beam;
        self.sized_for(wings_only)
    }

    pub(crate) fn can_carry(&self, layout: TurretLayout) -> bool {
        let trial = Self {
            turrets: layout,
            ..*self
        };
        trial.calibre() <= trial.capacity()
    }
}
