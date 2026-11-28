//! How the main battery lays its rounds: how far each falls off its mark, a hull's own
//! steadiness under it, and the light cruiser's long-range gunnery cards.

/// Share of the range a stock crew's round falls off its mark at most, along the line
/// of fire and across it. The errors gather about nothing, as a gunlayer's do, so most
/// rounds fall well inside it: at 800 m a stock round lands within 13 m over or short
/// and 5 m to either side, and half within a third of that.
pub const STOCK_SCATTER: (f32, f32) = (0.016, 0.006);
/// The share of her scatter each rank of long-base rangefinders takes off beyond half
/// her reach.
pub const RANGEFINDER_CUT: f32 = 0.15;
/// Seconds a main-battery hit throws a ship's gun crews off, by rank of suppressing
/// fire, for a hit landing [`SUPPRESSION_AT`] metres out.
pub const SUPPRESSION_SECONDS: [f32; 3] = [2., 3., 4.];
/// Metres out at which a hit holds the crews for [`SUPPRESSION_SECONDS`]. Nearer they
/// shrug it off sooner, to half at half the distance, and farther, where the fall of
/// shot is harder to read, they hold longer, to half again by half as far again.
pub const SUPPRESSION_AT: f32 = 500.;
/// The least and the most of [`SUPPRESSION_SECONDS`] a hit holds the crews for.
const SUPPRESSION_SPAN: (f32, f32) = (0.5, 1.5);
/// A flagship's crews are drilled to stand to their guns: a hit holds them half as long.
pub const FLAGSHIP_SUPPRESSION: f32 = 0.5;
/// Metres out beyond which her rounds come down through the deck, with plunging fire.
pub const PLUNGE_RANGE: f32 = 700.;
/// What each rank of plunging fire adds to a round landing beyond [`PLUNGE_RANGE`]:
/// damage, and a share of its penetration.
pub const PLUNGE_DAMAGE: u16 = 1;
pub const PLUNGE_PENETRATION: f32 = 0.15;
/// Ranks of each of the cruiser's gunnery cards.
pub const MOST_RANGEFINDERS: u16 = 3;
pub const MOST_SUPPRESSION: u16 = 3;
pub const MOST_PLUNGING: u16 = 3;

/// The main battery's fire control as fitted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FireControl {
    /// Her hull's scatter, as a multiple of [`STOCK_SCATTER`]: a cruiser lays her guns
    /// from a steadier, roomier platform than a destroyer.
    pub dispersion: f32,
    /// Ranks of long-base rangefinders.
    pub rangefinders: u16,
    /// Ranks of suppressing fire.
    pub suppression: u16,
    /// Ranks of plunging fire.
    pub plunging: u16,
}
impl Default for FireControl {
    fn default() -> Self {
        Self::STOCK
    }
}
impl FireControl {
    pub const STOCK: Self = Self {
        dispersion: 1.,
        rangefinders: 0,
        suppression: 0,
        plunging: 0,
    };
    /// Metres a round laid `distance` out by a battery reaching `reach` falls off its
    /// mark at most, along the line of fire and across it. Long-base rangefinders take
    /// their cut beyond half her reach, all of it by two thirds.
    pub fn scatter(&self, distance: f32, reach: f32) -> (f32, f32) {
        let distance = distance.max(0.);
        let past_half = ((distance / reach.max(1.) - 0.5) * 6.).clamp(0., 1.);
        let ranks = f32::from(self.rangefinders.min(MOST_RANGEFINDERS));
        let cut = 1. - RANGEFINDER_CUT * ranks * past_half;
        let size = distance * self.dispersion * cut;
        (STOCK_SCATTER.0 * size, STOCK_SCATTER.1 * size)
    }
    /// Seconds a hit landing `distance` out throws a ship's gun crews off, or zero
    /// without suppressing fire.
    pub fn suppression(&self, distance: f32) -> f32 {
        let Some(rank) = self.suppression.min(MOST_SUPPRESSION).checked_sub(1) else {
            return 0.;
        };
        let (least, most) = SUPPRESSION_SPAN;
        SUPPRESSION_SECONDS[usize::from(rank)]
            * (distance.max(0.) / SUPPRESSION_AT).clamp(least, most)
    }
    /// What plunging fire adds to a round landing `distance` out: damage, and the
    /// multiple of its penetration. Nothing short of [`PLUNGE_RANGE`].
    pub fn plunge(&self, distance: f32) -> (u16, f32) {
        let ranks = self.plunging.min(MOST_PLUNGING);
        if distance <= PLUNGE_RANGE || ranks == 0 {
            return (0, 1.);
        }
        (
            PLUNGE_DAMAGE * ranks,
            1. + PLUNGE_PENETRATION * f32::from(ranks),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rangefinders_tighten_only_long_fire() {
        let stock = FireControl::STOCK;
        let ranged = FireControl {
            rangefinders: 3,
            ..stock
        };
        // Inside half her reach they do nothing, and beyond two thirds of it all they do.
        assert_eq!(stock.scatter(400., 1000.), ranged.scatter(400., 1000.));
        let (far, _) = stock.scatter(900., 1000.);
        let (tight, _) = ranged.scatter(900., 1000.);
        assert!((tight / far - 0.55).abs() < 1e-5);
    }

    #[test]
    fn suppression_holds_longer_the_farther_the_hit() {
        let none = FireControl::STOCK;
        assert_eq!(none.suppression(600.), 0.);
        let rank = |suppression| FireControl {
            suppression,
            ..FireControl::STOCK
        };
        assert_eq!(rank(1).suppression(SUPPRESSION_AT), 2.);
        assert_eq!(rank(3).suppression(SUPPRESSION_AT), 4.);
        assert!(rank(2).suppression(100.) < rank(2).suppression(700.));
        assert_eq!(rank(2).suppression(2000.), 4.5);
    }

    #[test]
    fn plunging_fire_starts_beyond_its_range() {
        let plunging = FireControl {
            plunging: 2,
            ..FireControl::STOCK
        };
        assert_eq!(plunging.plunge(PLUNGE_RANGE - 1.), (0, 1.));
        let (damage, deeper) = plunging.plunge(PLUNGE_RANGE + 1.);
        assert_eq!(damage, 2);
        assert!((deeper - 1.3).abs() < 1e-5);
    }
}
