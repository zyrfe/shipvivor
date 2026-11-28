//! The ship a run starts in, picked before her first patrol.
use crate::{Card, Fittings, Run};
use shipvivor_artillery::FireControl;
use shipvivor_shipyard::{Design, Protection, Ship};

/// The light cruiser's long-range gunnery: cards dealt and fitted only on her line.
pub const CRUISER_GUNNERY: [Card; 3] = [
    Card::SuppressingFire,
    Card::LongBaseRangefinders,
    Card::PlungingFire,
];

/// The hulls a run can start in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StartingHull {
    /// Long-barrelled guns that outreach a destroyer's, under thin plate, and no tubes.
    #[default]
    LightCruiser,
    /// Built like a tank: a third again the hull, half again the plate and a second
    /// engine, with one main gun to the cruiser's two and tubes in the bow, amidships and
    /// the stern.
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
            Self::HeavyDestroyer => "Heavy plate, a tough hull, one gun and three sets of tubes",
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
            Self::HeavyDestroyer => Protection::HEAVY_DESTROYER,
        }
    }

    /// What she sails with before any card: the light cruiser's long barrels are a rank
    /// of muzzle velocity.
    pub fn fittings(self) -> Fittings {
        match self {
            // The cruiser's deck guns are one-inch: she fights with her main battery,
            // and the destroyer's two-inch guns make up for her single turret.
            Self::LightCruiser => Fittings {
                velocity: 1,
                one_inch_deck_guns: true,
                ..Fittings::default()
            },
            Self::HeavyDestroyer => Fittings::default(),
        }
    }

    /// Her main battery's scatter, as a multiple of the stock crew's: the cruiser lays
    /// her guns from a steadier platform, and the destroyer's are sloppy off her lively
    /// deck.
    pub fn dispersion(self) -> f32 {
        match self {
            Self::LightCruiser => 0.8,
            Self::HeavyDestroyer => 1.25,
        }
    }

    /// A ship of her line can be fitted with `card`. The cruiser's long-range gunnery
    /// is hers alone.
    pub fn takes(self, card: Card) -> bool {
        self == Self::LightCruiser || !CRUISER_GUNNERY.contains(&card)
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

    /// Her main battery's fire control: her hull's scatter, and the cruiser's gunnery
    /// cards.
    pub fn fire_control(&self) -> FireControl {
        let fit = &self.fittings;
        FireControl {
            dispersion: self.hull.dispersion(),
            rangefinders: fit.rangefinders,
            suppression: fit.suppression,
            plunging: fit.plunging,
        }
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
        // Fewer guns, more hull: one main turret to the cruiser's two, and a third again
        // the structure.
        assert_eq!(design.gun_stations().len(), 1);
        assert_eq!(cruiser.ship.design().gun_stations().len(), 2);
        assert!(design.structure() > 1.25 * cruiser.ship.design().structure());
    }
}
