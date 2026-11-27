//! Authored ammunition for the compressed game sea. Plate thicknesses are game tiers.
use crate::Round;

/// Metres a stock round carries.
const STOCK_REACH: f32 = 780.;
/// The stock loading curve runs through these two guns, each given as its bore in
/// inches and the seconds one barrel takes to load.
const LOADING: ((f32, f32), (f32, f32)) = ((4., 2.5), (30., 30.));
/// Share of a round's penetration each rank of piercing caps adds.
const CAP_GAIN: f32 = 0.25;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MainShell {
    pub bore: u16,
    pub caps: u16,
    pub velocity: u16,
}

impl MainShell {
    pub const STOCK: Self = Self {
        bore: 0,
        caps: 0,
        velocity: 0,
    };
    pub fn calibre_inches(self) -> f32 {
        4. + f32::from(self.bore)
    }
    pub fn speed(self) -> f32 {
        let rank = f32::from(self.velocity);
        360. + 220. * rank / (3. + rank)
    }
    /// Metres the round carries before it comes down. A faster round carries farther,
    /// in step with its muzzle speed.
    pub fn reach(self) -> f32 {
        STOCK_REACH * self.speed() / Self::STOCK.speed()
    }
    /// Bore relative to the six-inch barrel model.
    pub fn scale(self) -> f32 {
        self.calibre_inches() / 6.
    }
    /// How much deck space the game model occupies. Large gunhouses are
    /// compact enough to train on one modular deck section.
    pub fn mount_scale(self) -> f32 {
        let inches = self.calibre_inches();
        (inches.min(6.) + (inches - 6.).max(0.) * 0.25) / 6.
    }
    pub fn diameter_metres(self) -> f32 {
        self.calibre_inches() * 0.0254
    }
    /// Seconds a stock crew takes to load one barrel with this round. A light gun loads
    /// in moments, and the time grows faster than the bore: 2.5 s at four inches, about
    /// 6 s at eight, 16 s at eighteen and 30 s at thirty.
    pub fn load_seconds(self) -> f32 {
        let ((light, quick), (heavy, slow)) = LOADING;
        let steepness = (slow / quick).ln() / (heavy / light).ln();
        quick * (self.calibre_inches() / light).powf(steepness)
    }
    /// The larger projectile displaces more material once a plate opens.
    pub fn breach_force(self) -> u16 {
        2_u16.saturating_add(self.bore)
    }
    /// Inches of sound plate the round goes through striking square at the muzzle. It
    /// starts at its bore, each rank of caps adds a quarter, and a faster round drives in
    /// harder in step with its muzzle speed.
    pub fn penetration(self) -> f32 {
        self.calibre_inches()
            * (1. + CAP_GAIN * f32::from(self.caps))
            * (self.speed() / Self::STOCK.speed())
    }
    /// Inches of sound plate it goes through `flight_seconds` out. It loses way as it
    /// flies, so a faster round, which gets there sooner, keeps more of it.
    pub fn penetration_at(self, flight_seconds: f32) -> f32 {
        self.penetration() * Round::retained(flight_seconds)
    }
    /// The round as it meets plate `flight_seconds` out.
    pub fn round_at(self, flight_seconds: f32) -> Round {
        Round {
            calibre: self.calibre_inches(),
            penetration: self.penetration_at(flight_seconds),
        }
    }
}
