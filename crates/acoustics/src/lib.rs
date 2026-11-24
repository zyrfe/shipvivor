//! Sound as it crosses the water.
//!
//! Every sound is a baked cast of three layers: fine detail, body and rumble. A voice
//! reads its cast as the listener would hear it, late by the time the sound takes to
//! arrive, with the air taking the detail first and the rumble last.
//!
//! On the way it also comes off the sea just short of the listener, off any land that
//! stands in the open, and back from the whole sea as a long dark tail. Each ear hears
//! it at its own moment.
//!
//! Three things are made as they play. The ship's own engine, wash and wind follow the
//! speed and engine order last given. A shell in flight is heard from where it was when
//! the sound left it, which for a fast shell is a crack and then a tearing fall in pitch.
//! A base's siren is a rotor that winds up and runs down for as long as its alarm is up.
//!
//! The game thread sends [`Cue`]s. The [`Mixer`] renders on the audio thread, where it
//! never blocks or allocates. Positions are metres with Y up.

mod ambience;
mod engine;
mod filter;
mod flight;
mod foundry;
mod hull;
mod mixer;
mod siren;
mod tail;
#[cfg(test)]
mod tests;
mod voice;

pub use ambience::{Ambience, Haunt, Place};
pub use foundry::Bank;
use glam::Vec3;
pub use mixer::{Cues, Feed, Mixer, console};

pub const SAMPLE_RATE: u32 = 48_000;
const RATE: f32 = SAMPLE_RATE as f32;
/// Inside this many metres a sound is at full level.
const NEAR: f32 = 15.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    MainGun,
    SecondaryGun,
    RaiderGun,
    ShoreGun,
    Splash,
    Burst,
    Wreck,
    ArmorRing,
    ArmorKnock,
    /// The shock of a shell passing faster than sound.
    Crack,
    /// The ship's own main battery, felt through the hull before it is heard.
    Recoil,
    /// The same for one of her smaller guns.
    Kick,
    Trumpet,
    /// A building coming down in a heap of masonry.
    Collapse,
    /// A dockyard's gantry and sheds coming down, steel and all.
    Ruin,
    /// Brass and drums for a base destroyed, played aboard.
    Fanfare,
    /// The sea breaking on a shore and running back.
    Breaker,
    /// Ice splitting under its own weight.
    IceCrack,
    /// A glacier's front falling into the sea.
    Calving,
    /// A kittiwake calling.
    Gull,
    /// A riveting gang at work in a yard.
    Rivets,
    /// The ship's own frames working in a seaway, heard aboard.
    Creak,
    /// The engine-room telegraph ringing a new order, heard aboard.
    Telegraph,
}
impl Sound {
    pub const ALL: [Self; 23] = [
        Self::MainGun,
        Self::SecondaryGun,
        Self::RaiderGun,
        Self::ShoreGun,
        Self::Splash,
        Self::Burst,
        Self::Wreck,
        Self::ArmorRing,
        Self::ArmorKnock,
        Self::Crack,
        Self::Recoil,
        Self::Kick,
        Self::Trumpet,
        Self::Collapse,
        Self::Ruin,
        Self::Fanfare,
        Self::Breaker,
        Self::IceCrack,
        Self::Calving,
        Self::Gull,
        Self::Rivets,
        Self::Creak,
        Self::Telegraph,
    ];

    /// Level of the detail, body and rumble layers at close range.
    fn levels(self) -> [f32; 3] {
        match self {
            Self::MainGun => [0.9, 1., 0.6],
            Self::SecondaryGun => [0.45, 0.36, 0.14],
            Self::RaiderGun => [0.6, 0.54, 0.2],
            Self::ShoreGun => [0.8, 1., 0.6],
            Self::Splash => [0.25, 0.4, 0.25],
            Self::Burst => [0.5, 0.8, 0.4],
            Self::Wreck => [0.5, 1., 0.8],
            Self::ArmorRing => [0.84, 0.52, 0.10],
            Self::ArmorKnock => [0.40, 0.84, 0.48],
            Self::Crack => [0.6, 0.3, 0.1],
            Self::Recoil => [0.22, 0.5, 0.36],
            Self::Trumpet => [0.22, 0.45, 0.12],
            Self::Kick => [0.1, 0.22, 0.1],
            Self::Collapse => [0.5, 0.9, 0.7],
            Self::Ruin => [0.55, 1., 0.9],
            Self::Fanfare => [0.26, 0.66, 0.48],
            Self::Breaker => [0.5, 0.64, 0.08],
            Self::IceCrack => [0.5, 0.6, 0.12],
            Self::Calving => [0.4, 0.8, 0.7],
            Self::Gull => [0.4, 0.24, 0.],
            Self::Rivets => [0.9, 0.4, 0.05],
            Self::Creak => [0.12, 0.3, 0.12],
            Self::Telegraph => [0.4, 0.08, 0.],
        }
    }

    /// Carried to the listener by the ship itself: heard at once, from no direction,
    /// with no sea or land to answer it.
    fn aboard(self) -> bool {
        matches!(
            self,
            Self::Recoil
                | Self::Kick
                | Self::Trumpet
                | Self::Fanfare
                | Self::Creak
                | Self::Telegraph
        )
    }

    /// Music and bells, which sound at their own pitch every time.
    fn tuned(self) -> bool {
        matches!(self, Self::Trumpet | Self::Fanfare | Self::Telegraph)
    }

    /// Loud enough for land to throw back.
    fn echoes(self) -> bool {
        matches!(
            self,
            Self::MainGun
                | Self::RaiderGun
                | Self::ShoreGun
                | Self::Wreck
                | Self::Ruin
                | Self::Calving
        )
    }
}

/// A make of siren.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rotor {
    /// The air-raid siren of a base: two tones a minor third apart, near 475 and 570 Hz.
    Shore,
    /// A heavier one carried by a capital ship: slower to wind up, and a major third
    /// near 240 and 300 Hz.
    Ship,
}

/// What the air does to a sound on its way to the listener.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Air {
    /// Metres per second.
    pub speed: f32,
    /// Exponent of the loss with distance. Open air is 1: half the level at twice the range.
    pub rolloff: f32,
    /// Multiple of the real absorption of high frequencies.
    pub haze: f32,
}
impl Default for Air {
    /// Air at sea level.
    fn default() -> Self {
        Self {
            speed: 343.,
            rolloff: 1.,
            haze: 1.,
        }
    }
}
impl Air {
    /// Air for the game's scaled sea. Shells there fly at about half their real speed,
    /// so sound does too and a round still lands ahead of its report. Ranges are short,
    /// so distance is drawn more boldly than open air would draw it.
    pub const SCALED: Self = Self {
        speed: 170.,
        rolloff: 0.75,
        haze: 3.,
    };

    /// Samples a sound takes to cross this range.
    fn delay(&self, metres: f32) -> f32 {
        metres / self.speed * RATE
    }

    /// Gains of the detail, body and rumble layers. Detail is the first to go and
    /// rumble carries furthest.
    fn gains(&self, metres: f32) -> [f32; 3] {
        let body = (NEAR / metres.max(NEAR)).powf(self.rolloff);
        let crisp = 1. / (1. + (self.haze * metres / 400.).powi(2));
        [body * crisp, body, body.powf(0.7)]
    }

    /// Frequency above which the air has absorbed more than 3 dB. Absorption grows with
    /// the square of frequency, about 23 dB per kilometre at 4 kHz.
    fn ceiling(&self, metres: f32) -> f32 {
        (45_800. / (self.haze * metres).max(1.).sqrt()).min(20_000.)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cue {
    /// Where the listener stands, and the direction of their right ear.
    Ears { position: Vec3, right: Vec3 },
    /// A sound made once at a fixed point. The serial picks its variation.
    Emit {
        sound: Sound,
        position: Vec3,
        serial: u64,
    },
    /// A shell in the air this tick, in metres and metres per second. Its weight is 1
    /// for a six-inch round. A shell that stops being reported has landed.
    Shell {
        serial: u64,
        position: Vec3,
        velocity: Vec3,
        weight: f32,
    },
    /// Land or ice standing out of the sea, which throws loud sounds back: the more of
    /// it there is, the more it throws. Each slot holds one, and a radius of nothing
    /// empties the slot.
    Shore {
        slot: usize,
        position: Vec3,
        radius: f32,
    },
    /// A siren. While its alarm is raised it wails. Lowered, it is left to run down, as
    /// it also is when it loses its power. Each slot holds one, and one aboard a ship
    /// is given her new position each tick.
    Alarm {
        slot: usize,
        rotor: Rotor,
        position: Vec3,
        raised: bool,
    },
    /// The ship is under way: speed through the water in metres per second, and the
    /// engine order as a share of full ahead.
    Way { speed: f32, throttle: f32 },
    /// The state of the ship's engines, as shares of however many she has: those
    /// damaged and still running, which stumble and bang in their exhaust, and those
    /// knocked out, which are silent. It holds until the next is sent.
    Engines { damaged: f32, stopped: f32 },
    /// The machinery is shut down and only the sea is heard. Any alarm is lowered.
    Moored,
    /// World time runs or holds. Held sound hangs in the air and resumes where it was.
    Flow(bool),
    /// Drop every sound still in the air. The next ears are taken up without a glide.
    Hush,
}
