//! Authored ammunition for the compressed game sea. Plate grades are game tiers.
use crate::ShellClass;

/// Metres a stock round carries.
const STOCK_REACH: f32 = 780.;

/// What a round is filled with. HE bursts on whatever it strikes and opens only light
/// plate. AP is built to go through armour, and caps and a larger bore drive it harder.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Ammo {
    #[default]
    He,
    Ap,
}
impl Ammo {
    pub fn label(self) -> &'static str {
        match self {
            Self::He => "HE",
            Self::Ap => "AP",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MainShell {
    pub bore: u16,
    pub caps: u16,
    pub velocity: u16,
    pub ammo: Ammo,
}

impl MainShell {
    pub const STOCK: Self = Self {
        bore: 0,
        caps: 0,
        velocity: 0,
        ammo: Ammo::He,
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
    /// The larger projectile displaces more material once a plate opens.
    pub fn breach_force(self) -> u16 {
        2_u16.saturating_add(self.bore)
    }
    /// HE never rises above the lightest grade. AP starts one grade up.
    fn grade(self) -> u16 {
        match self.ammo {
            Ammo::He => 0,
            Ammo::Ap => self.caps.saturating_add(self.bore / 2).saturating_add(1),
        }
    }
    pub fn penetration(self) -> ShellClass {
        match self.grade() {
            0 => ShellClass::Standard,
            1 => ShellClass::Breaker,
            _ => ShellClass::Heavy,
        }
    }
    pub fn at_contact(self, flight_seconds: f32) -> ShellClass {
        let class = self.penetration();
        if flight_seconds > 0.8 && self.grade() < 3 {
            match class {
                ShellClass::Heavy => ShellClass::Breaker,
                _ => ShellClass::Standard,
            }
        } else {
            class
        }
    }
}
