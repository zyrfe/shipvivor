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
//! Five things are made as they play. The ship's own engine, wash and wind follow the
//! speed, the engine order and the state of her engines last given. A shell in flight is heard from where it was when
//! the sound left it, which for a fast shell is a crack and then a tearing fall in pitch.
//! A base's siren is a rotor that winds up and runs down for as long as its alarm is up.
//! A big ship's horn sounds the prolonged blasts of a ship putting out, deeper the
//! longer she is.
//! An aeroplane's engine and propeller are heard sharp as it closes and flat as it goes.
//!
//! The places the ship passes make sounds of their own, which [`Ambience`] decides the
//! moments of: surf, seabirds, ice and the work of a yard.
//!
//! The game thread sends [`Cue`]s. The [`Mixer`] renders on the audio thread, where it
//! never blocks or allocates. Positions are metres with Y up.

mod ambience;
mod engine;
mod filter;
mod flight;
mod foundry;
mod horn;
mod hull;
mod line;
mod mixer;
mod plane;
mod siren;
mod tail;
#[cfg(test)]
mod tests;
mod voice;

pub use ambience::{Ambience, Haunt, Place};
pub use foundry::Bank;
use glam::Vec3;
pub use mixer::{AIRCRAFT, Cues, Feed, Mixer, console};

pub const SAMPLE_RATE: u32 = 48_000;
const RATE: f32 = SAMPLE_RATE as f32;
/// Inside this many metres a sound is at full level.
const NEAR: f32 = 15.;

/// Stroke lengths shared by the chart animation and its paper foley.
pub const CHART_UNROLL_SECONDS: f32 = 0.32;
pub const CHART_ROLL_SECONDS: f32 = 0.22;

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
    /// Two notes for something gained. It is the game speaking and not the world.
    Upgrade,
    UnrollChart,
    RollChart,
    /// The ship's klaxon, which is sounded when she is close to sinking.
    Klaxon,
    /// A small brass handbell struck once, for a yard point won from cargo sunk. It is
    /// the game speaking, as the upgrade is.
    Chime,
    /// A heavy shore battery's long gun: a slow, deep push and a long roll that carries.
    HeavyGun,
    /// A heavy shell falling in the sea: a tall column that goes up deep and comes down
    /// for longer.
    Column,
    /// A torpedo boat's tube firing: the cough of its impulse charge and the hiss of the
    /// air behind it, then the slap of the torpedo striking the water.
    TorpedoLaunch,
    /// The bridge's alarm bell, rung when a torpedo's track is seen closing on her. It
    /// is an electric bell, its clapper hammering a gong a dozen and more times a second.
    TorpedoAlarm,
    /// An enemy torpedo bursting against her side under water: a deep, smothered blow
    /// that shakes her frames, and the column it threw up coming down on her deck.
    Torpedoed,
    /// The scope's magnification ring clicking into a detent, close at hand on the bridge.
    Detent,
    /// An engine hurt and running on damaged: a clank in the engine room and the grind
    /// of something loose in the running gear, felt up through the ship.
    EngineHurt,
    /// An engine giving out: a part letting go, the last of it coasting round under a
    /// falling whine, the clunk as the crank comes to rest, and air sighing out of it.
    EngineStall,
    /// The repair party knocking a fitting true with a hand hammer.
    Hammer,
    /// The repair party's cutting torch, roaring and spitting.
    Torch,
    /// The repair party's spanner: a ratchet worked in short strokes.
    Ratchet,
    /// One beat of the bridge clock, counting out the leg's last minute. Even takes are
    /// the tick and odd ones the tock. It is the game speaking, as the upgrade is.
    Tick,
    /// A light round glancing off plate far thicker than it needs: a sharp tick, a high
    /// ring that is gone at once, and the round whirring away as it tumbles.
    Tap,
    /// A round her plate turns: the plate rings in its low modes under a solid blow.
    Clank,
    /// A round punching through plate too thin to stop it: a crack, the torn edges
    /// crackling, and a dull thump from a plate that has no ring left in it.
    Pierce,
    /// A round bursting inside her a beat after it went in, smothered by the steel
    /// round it: a low boom, the box drumming, frames crunching and splinters rattling.
    Blast,
    /// A section of her giving way: frames folding in a long groan, gear crashing down
    /// inside, and the sea roaring in.
    Gutted,
    /// Her frames taking a heavy blow, carried to the bridge by the ship: a deep thud,
    /// the hull flexing, loose gear rattling and a groan as she settles.
    Shudder,
    /// Her light anti-aircraft guns: a pom-pom's barrels firing in turn, three or four
    /// dull thuds on a steady beat, the action clacking home and spent cases ringing on
    /// the deck between them. Nothing else she carries has a beat.
    PomPom,
    /// One character on the radio room's teleprinter: the typebar whacking ribbon and
    /// paper onto the rubber platen, its rattle home into the basket, the carriage's
    /// escapement stepping on, and the selector clicking off the next character's
    /// pulses. Struck on a steady beat, as the line sets it. It is the game speaking.
    Typebar,
    /// The teleprinter's carriage return and line feed: the platen's ratchet clacking a
    /// line on, the carriage running back on its spring and banging into its dashpot.
    Carriage,
    /// The teleprinter's bell, a small gong struck once by its hammer. A message of
    /// high precedence rings it before it prints.
    PrinterBell,
}
impl Sound {
    pub const ALL: [Self; 50] = [
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
        Self::Upgrade,
        Self::UnrollChart,
        Self::RollChart,
        Self::Klaxon,
        Self::Chime,
        Self::HeavyGun,
        Self::Column,
        Self::TorpedoLaunch,
        Self::TorpedoAlarm,
        Self::Torpedoed,
        Self::Detent,
        Self::EngineHurt,
        Self::EngineStall,
        Self::Hammer,
        Self::Torch,
        Self::Ratchet,
        Self::Tick,
        Self::Tap,
        Self::Clank,
        Self::Pierce,
        Self::Blast,
        Self::Gutted,
        Self::Shudder,
        Self::PomPom,
        Self::Typebar,
        Self::Carriage,
        Self::PrinterBell,
    ];

    /// Level of the detail, body and rumble layers at close range.
    fn levels(self) -> [f32; 3] {
        match self {
            Self::MainGun => [0.9, 1., 0.6],
            Self::SecondaryGun => [0.45, 0.36, 0.14],
            Self::RaiderGun => [0.6, 0.54, 0.2],
            // A shore gun's twelve inches and a heavy battery's fifteen are far heavier
            // than anything she carries, and their reports carry over her own noise.
            Self::ShoreGun => [0.9, 1.8, 2.],
            Self::Splash => [0.55, 0.9, 0.55],
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
            Self::IceCrack => [1., 1.2, 0.24],
            Self::Calving => [0.6, 1.2, 1.],
            Self::Gull => [1.1, 0.68, 0.],
            Self::Rivets => [2., 0.85, 0.1],
            Self::Creak => [0.12, 0.3, 0.12],
            Self::Telegraph => [0.4, 0.08, 0.],
            Self::Upgrade => [0.18, 0.5, 0.25],
            Self::UnrollChart | Self::RollChart => [0.28, 0.39, 0.],
            Self::Klaxon => [0.35, 0.6, 0.],
            Self::Chime => [0.2, 0.42, 0.],
            Self::HeavyGun => [0.7, 2.4, 3.],
            Self::Column => [0.5, 1., 0.9],
            Self::TorpedoLaunch => [0.5, 0.62, 0.3],
            Self::TorpedoAlarm => [0.3, 0.5, 0.],
            Self::Torpedoed => [0.55, 1.1, 1.2],
            Self::Detent => [0.12, 0.18, 0.],
            Self::EngineHurt => [0.5, 0.75, 0.45],
            Self::EngineStall => [0.22, 0.42, 0.3],
            Self::Hammer => [0.33, 0.18, 0.],
            Self::Torch => [0.16, 0.19, 0.],
            Self::Ratchet => [0.5, 0.28, 0.],
            // At full strength, which the clock reaches only in its last seconds.
            Self::Tick => [0.3, 0.4, 0.],
            // A clank at full strength is the burst every hit on her made before plate
            // and calibre were heard. A hit plays each at its own strength: see [`Hit`].
            Self::Tap => [1.3, 0.3, 0.05],
            Self::Clank => [0.5, 0.8, 0.4],
            Self::Pierce => [0.62, 0.7, 0.42],
            Self::Blast => [0.22, 1., 1.1],
            Self::Gutted => [0.2, 0.75, 0.9],
            Self::Shudder => [0.14, 0.4, 0.34],
            Self::PomPom => [0.42, 0.4, 0.1],
            // The radio room is a deck below the bridge: clear over the engines, never
            // over a gun.
            Self::Typebar => [0.3, 0.26, 0.05],
            Self::Carriage => [0.26, 0.3, 0.06],
            Self::PrinterBell => [0.2, 0.32, 0.],
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
                | Self::Upgrade
                | Self::UnrollChart
                | Self::RollChart
                | Self::Klaxon
                | Self::Chime
                | Self::TorpedoAlarm
                | Self::Detent
                | Self::EngineHurt
                | Self::EngineStall
                | Self::Shudder
                | Self::Typebar
                | Self::Carriage
                | Self::PrinterBell
        )
    }

    /// Music and bells, which sound at their own pitch every time.
    fn tuned(self) -> bool {
        matches!(
            self,
            Self::Trumpet
                | Self::Fanfare
                | Self::Telegraph
                | Self::Upgrade
                | Self::Klaxon
                | Self::Chime
                | Self::TorpedoAlarm
                | Self::Tick
                | Self::PrinterBell
        )
    }

    /// The game speaking to the player. It belongs to no moment of the battle, so it
    /// is heard whether the world's time runs or holds, and a hush leaves it be.
    fn notice(self) -> bool {
        matches!(
            self,
            Self::Upgrade
                | Self::Chime
                | Self::Tick
                | Self::Typebar
                | Self::Carriage
                | Self::PrinterBell
        ) || self.chart_paper()
    }

    fn chart_paper(self) -> bool {
        matches!(self, Self::UnrollChart | Self::RollChart)
    }

    /// Loud enough for land to throw back.
    fn echoes(self) -> bool {
        matches!(
            self,
            Self::MainGun
                | Self::RaiderGun
                | Self::ShoreGun
                | Self::HeavyGun
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

/// What is asked of a siren.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alert {
    /// The alarm is up, and it wails.
    Raised,
    /// The alarm is down. The rotor is left to run down by itself, which takes it
    /// half a minute.
    Lowered,
    /// The siren has been brought down with whatever it stood on. Its power is gone
    /// and its rotor is fouled, so its note drops away to nothing in a few seconds.
    /// That is the sound of a base that is finished.
    Wrecked,
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
    /// A siren, and what is being asked of it. Each slot holds one, and one aboard a
    /// ship is given her new position each tick.
    Alarm {
        slot: usize,
        rotor: Rotor,
        position: Vec3,
        alert: Alert,
    },
    /// An aeroplane, heard from where it was when its sound left it: where it is this
    /// tick, in metres and metres per second, and whether its engine is running. Each
    /// slot holds one. Between ticks it is carried on at its speed.
    Aircraft {
        slot: usize,
        position: Vec3,
        velocity: Vec3,
        flying: bool,
    },
    /// The ship is under way: speed through the water in metres per second, and the
    /// engine order as a share of full ahead.
    Way { speed: f32, throttle: f32 },
    /// The state of the ship's engines, as shares of however many she has: those
    /// damaged and still running, which stumble and bang in their exhaust, and those
    /// knocked out, which are silent. It holds until the next is sent.
    Engines { damaged: f32, stopped: f32 },
    /// A ship's horn, aboard a ship this many metres long, which sets it: the longer
    /// the ship, the deeper and louder the horn and the longer each blast. With `blow`
    /// she sounds her signal of prolonged blasts, as a ship putting out does. Without
    /// it the horn is only carried to where she now is. One horn is heard at a time.
    Horn {
        position: Vec3,
        length: f32,
        blow: bool,
    },
    /// The machinery is shut down and only the sea is heard. Any alarm is lowered, and
    /// no aeroplane is heard any longer.
    Moored,
    /// World time runs or holds. Held sound hangs in the air and resumes where it was.
    /// What the game itself has to say is heard all the same.
    Flow(bool),
    /// Drop every sound of the world still in the air. The next ears are taken up
    /// without a glide.
    Hush,
    /// The game speaking at a strength of its own, from 0 for silence to 1 for the
    /// sound's usual level, so that a run of them can swell or fade. The serial picks
    /// its take.
    Notice {
        sound: Sound,
        level: f32,
        serial: u64,
    },
    /// A blow on the listener's own ship, at the point on her it landed, heard as her
    /// plating took it: see [`Hit`]. The serial picks its takes.
    Hit {
        position: Vec3,
        serial: u64,
        hit: Hit,
    },
}

/// A blow on the listener's own ship, as her plating took it. What it sounds like is
/// what tells those aboard how bad it was, without a look at the plating.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// Inches across the round, or `None` for a blow no plate can turn: a ram, a bomb,
    /// ice or a torpedo, each of which makes a sound of its own. Such a blow is heard
    /// here only for what it did inside her.
    pub calibre: Option<f32>,
    /// Inches of plate it met.
    pub plate: f32,
    /// The plate turned it aside.
    pub turned: bool,
    /// Hull points of damage that got through.
    pub dealt: f32,
    /// The worst it cost her.
    pub harm: Harm,
}

/// The worst a blow cost her, from nothing to a magazine going up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Harm {
    #[default]
    None,
    /// Something fitted in her was knocked out.
    Knocked,
    /// A section of her was wrecked.
    Wrecked,
    /// A magazine went up, and the section with it.
    Detonated,
}

/// Inches across the round every hit sound is cast for.
const ROUND: f32 = 6.;
/// Hull points of damage at which a round bursting inside her is as loud as its cast.
const BLAST_DEALT: f32 = 8.;
/// Hull points of damage at which her frames shake as hard as their cast, and the
/// fewest that shake them at all.
const SHAKE: (f32, f32) = (12., 5.);

impl Hit {
    /// The sounds the blow makes, each with its strength and the pace it is played at.
    ///
    /// A heavier round is played slower, so deeper and longer: the time a round takes
    /// to strike steel and the push of its burst both grow with its size. Its strength
    /// grows with its weight, but far less than the hundredfold between the energy of a
    /// two-inch round and a fifteen-inch one.
    ///
    /// A round the plate turns clanks as hard as it came near to getting through and as
    /// it is heavy, and taps for the rest: a light round on heavy plate barely ticks.
    /// One that gets through tears the plate as much as it outmatched it, so a heavy
    /// round through thin plate is nearly all tear, and then it bursts inside her as
    /// loud as the damage it did. Heavy damage shakes her frames, and a section lost
    /// gives way with a roar.
    pub fn voices(self) -> impl Iterator<Item = (Sound, f32, f32)> {
        let mut struck = [0.; 4];
        let mut pace = 1.;
        if let Some(calibre) = self.calibre {
            let heft = calibre.max(0.5) / ROUND;
            let loud = heft.powf(0.8).clamp(0.3, 2.2);
            pace = heft.powf(-0.5).clamp(0.55, 1.6);
            if self.turned {
                let give = (calibre / self.plate.max(0.1)).min(1.);
                let ring = (give.powf(1.5) * heft.sqrt()).min(1.);
                struck[0] = loud * (1. - ring) * (0.7 + 0.3 * give);
                struck[1] = loud * ring;
            } else {
                // What the plate had against it, from nothing to nearly enough.
                let held = (self.plate / calibre).clamp(0., 1.);
                struck[1] = loud * held.max(0.3);
                struck[2] = loud * (1. - 0.5 * held);
                if self.dealt > 0. {
                    struck[3] = (self.dealt / BLAST_DEALT).powf(0.6).min(2.);
                }
            }
        }
        let mut shake = if self.dealt >= SHAKE.1 || self.harm > Harm::None {
            (self.dealt / SHAKE.0).powf(0.7).clamp(0.4, 1.6)
        } else {
            0.
        };
        let (gutted, magazine) = match self.harm {
            Harm::None | Harm::Knocked => (0., 0.),
            Harm::Wrecked => (1., 0.),
            Harm::Detonated => {
                shake = 1.6;
                (1.5, 2.)
            }
        };
        [
            (Sound::Tap, struck[0], pace),
            (Sound::Clank, struck[1], pace),
            (Sound::Pierce, struck[2], pace),
            (Sound::Blast, struck[3], pace),
            (Sound::Shudder, shake, pace.max(0.8)),
            (Sound::Gutted, gutted, 1.),
            (Sound::Blast, magazine, 0.6),
        ]
        .into_iter()
        .filter(|&(_, level, _)| level > 0.)
    }
}
