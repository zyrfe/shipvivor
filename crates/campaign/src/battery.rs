//! Authored fitting limits for the prototype. Beam, cradle and number of barrels
//! together determine which guns fit at an individual station.
use crate::Fittings;
use shipvivor_artillery::TurretLayout;
use shipvivor_shipyard::GunStation;

pub const CALIBRES: [u16; 6] = [4, 6, 8, 12, 15, 18];

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
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
            Self::Cruiser => 9.,
            Self::Capital => 14.,
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
