//! The ship a run starts in, picked before her first patrol.
use crate::{Fittings, Run};
use shipvivor_shipyard::{Design, Protection, Ship};

/// The hulls a run can start in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StartingHull {
    /// Long-barrelled guns that outreach a destroyer's, under thin plate, and no tubes.
    #[default]
    LightCruiser,
    /// Tubes in the bow, amidships and in the stern, a second engine for speed, and
    /// heavier plate, with shorter-reaching guns.
    HeavyDestroyer,
}

impl StartingHull {
    pub const ALL: [Self; 2] = [Self::LightCruiser, Self::HeavyDestroyer];

    /// The word the settings files keep it under.
    pub fn id(self) -> &'static str {
        match self {
            Self::LightCruiser => "light_cruiser",
            Self::HeavyDestroyer => "heavy_destroyer",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|hull| hull.id() == id)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::LightCruiser => "Light cruiser",
            Self::HeavyDestroyer => "Heavy destroyer",
        }
    }

    /// One line for the card she is picked from.
    pub fn summary(self) -> &'static str {
        match self {
            Self::LightCruiser => "Long-reaching guns, thin plate, no tubes",
            Self::HeavyDestroyer => "Tubes fore, aft and amidships; fast and well plated",
        }
    }

    pub fn design(self) -> Design {
        match self {
            Self::LightCruiser => Design::breakwater(),
            Self::HeavyDestroyer => Design::heavy_destroyer(),
        }
    }

    /// Her plate before any armour is fitted.
    pub fn protection(self) -> Protection {
        match self {
            Self::LightCruiser => Protection::LIGHT_CRUISER,
            Self::HeavyDestroyer => Protection::BREAKWATER,
        }
    }

    /// What she sails with before any card: the light cruiser's long barrels are a rank
    /// of muzzle velocity.
    pub fn fittings(self) -> Fittings {
        match self {
            Self::LightCruiser => Fittings {
                velocity: 1,
                ..Fittings::default()
            },
            Self::HeavyDestroyer => Fittings::default(),
        }
    }
}

impl Run {
    /// This run, before her first patrol, in a new ship of `hull`.
    #[must_use]
    pub fn with_hull(mut self, hull: StartingHull) -> Self {
        self.hull = hull;
        self.ship = Ship::new(hull.design());
        self.fittings = hull.fittings();
        self.refit_protection();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shipvivor_shipyard::{Component, TubeAim};

    #[test]
    fn each_hull_starts_as_described() {
        let cruiser = Run::default();
        assert_eq!(cruiser.hull, StartingHull::LightCruiser);
        assert_eq!(cruiser.fittings.velocity, 1);
        assert!(cruiser.ship.design().tube_mounts().is_empty());

        let destroyer = Run::default().with_hull(StartingHull::HeavyDestroyer);
        let design = destroyer.ship.design();
        let aims: Vec<TubeAim> = design.tube_mounts().iter().map(|t| t.aim).collect();
        assert_eq!(aims, [TubeAim::Ahead, TubeAim::Beam, TubeAim::Astern]);
        assert_eq!(design.count(Component::Engine), 2);
        assert_eq!(design.machinery(), Some(3));
        assert!(design.top_speed() > cruiser.ship.design().top_speed());
        assert!(design.protection().thickness > cruiser.ship.design().protection().thickness);
        assert_eq!(destroyer.fittings.velocity, 0);
    }
}
