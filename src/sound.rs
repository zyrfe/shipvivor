//! What the bridge hears. Each new effect in the battle becomes a cue for the mixer,
//! which plays on the audio thread and makes the listener wait for distant sound.
//! Shells are followed through the air, and the land and ice nearest the ship are
//! charted so they can echo.
//! A base that has word of the ship, or is being hit, sounds its siren, and a flagship
//! sounds a deeper one as she puts to sea.
//! The engines are heard as they are: damaged ones stumble and bang, and those knocked
//! out are silent. Around it all the places make their own sounds, which are surf,
//! seabirds, ice and the work of a yard.
use crate::{
    camera::ShipView,
    state::{Campaign, Embarked, Encounter, PlayerHelm, Screen},
};
use bevy::{
    audio::{AddAudioSource, ChannelCount, SampleRate, Source},
    prelude::*,
    reflect::TypePath,
};
use shipvivor_acoustics::{
    Air, Ambience, Bank, Cue, Cues, Feed, Haunt, Mixer, Place, Rotor, SAMPLE_RATE, Sound, console,
};
use shipvivor_compass::direction;
use shipvivor_encounter::{
    Battery, EffectKind, ISLAND_RADIUS, Impact, LandmarkKind, Sortie, YardState,
};
use shipvivor_navigation::Helm;
use shipvivor_shipyard::{Component, State};
use std::{sync::Mutex, time::Duration};

/// Height of the listener above the water, on the bridge.
const BRIDGE: f32 = 7.5;
/// Height of the part of an island that throws sound back.
const BLUFF: f32 = 5.;
/// Surfaces the mixer has room to echo from, and the smallest ice worth one of them.
const SHORES: usize = 8;
const FLOE: f32 = 10.;
/// Height of a base's siren, on a mast over its yard.
const MAST: f32 = 14.;
/// Seconds a base keeps its siren going after it last had word of the ship or was hit.
const ALARM: f32 = 12.;
/// Seconds a flagship sounds her own siren as she puts to sea, the slot the mixer
/// keeps it in after the three bases, and its height above the water.
const HERALD: f32 = 20.;
const FLAGSHIP: usize = 3;
const FUNNEL: f32 = 16.;
/// A burst within this many metres of the middle of a base has landed on it.
const ASHORE: f32 = 1.5 * ISLAND_RADIUS;

/// Height above the water from which a yard's work is heard.
const SLIP: f32 = 6.;

/// What `report` keeps from one tick to the next about the ship's surroundings: the
/// maker of their sounds, the engine order last rung, and when the battle was last seen.
#[derive(Default)]
pub struct Surroundings {
    ambience: Ambience,
    order: Option<f32>,
    seen: f32,
}

/// The game's line to the mixer. The mixer waits here until an output claims it.
#[derive(Resource)]
pub struct Soundboard {
    cues: Cues,
    mixer: Mutex<Option<Mixer>>,
    flowing: bool,
    steaming: bool,
}
impl Default for Soundboard {
    fn default() -> Self {
        let (cues, mixer) = console(Bank::standard(), Air::SCALED);
        Self {
            cues,
            mixer: Mutex::new(Some(mixer)),
            flowing: true,
            steaming: true,
        }
    }
}
impl Soundboard {
    /// Hand the mixer to whatever will play it. There is only one to take.
    pub fn claim(&self) -> Option<Mixer> {
        self.mixer.lock().ok()?.take()
    }
}

/// Send the ears, the ship's way and the state of her engines, every effect born this
/// tick, every shell in the air, the state of each base's alarm and whatever the
/// places round about have done. Runs straight after the battle steps.
#[allow(clippy::too_many_arguments)]
pub fn report(
    battle: Res<Encounter>,
    run: Res<Campaign>,
    helm: Res<PlayerHelm>,
    view: Res<ShipView>,
    board: Res<Soundboard>,
    clock: Res<Time<Fixed>>,
    mut aloft: Local<Vec<(u64, Vec3)>>,
    mut alarms: Local<(f32, [f32; 3])>,
    mut herald: Local<Vec3>,
    mut around: Local<Surroundings>,
) {
    // The listener stands on the bridge and faces wherever the sight is turned.
    let ahead = direction(view.bearing);
    board.cues.send(Cue::Ears {
        position: Vec3::new(helm.position.x, BRIDGE, helm.position.y),
        right: Vec3::new(-ahead.y, 0., ahead.x),
    });
    // The engines answer the telegraph. The wash answers the speed actually made.
    board.cues.send(Cue::Way {
        speed: helm.speed,
        throttle: helm.ordered_speed() / Helm::FULL_AHEAD,
    });
    // A damaged engine runs rough and one knocked out does not run at all.
    let engines = || run.ship.fitted(Component::Engine);
    let share = |state| {
        engines().filter(|(_, is)| *is == state).count() as f32 / engines().count().max(1) as f32
    };
    board.cues.send(Cue::Engines {
        damaged: share(State::Damaged),
        stopped: share(State::Out),
    });
    // The engine room answers a new order on the telegraph's bell. The order the
    // voyage begins with was rung before anyone was listening.
    let bridge = Vec3::new(helm.position.x, BRIDGE, helm.position.y);
    let (ordered, voyage) = (helm.ordered_speed(), battle.at_sea());
    if voyage < around.seen {
        around.order = None;
    }
    if around.order.is_some_and(|last| last != ordered) {
        board.cues.send(Cue::Emit {
            sound: Sound::Telegraph,
            position: bridge,
            serial: voyage.to_bits().into(),
        });
    }
    (around.order, around.seen) = (Some(ordered), voyage);
    for fx in battle.effects().iter().filter(|fx| fx.age == 0.) {
        let sound = match fx.kind {
            EffectKind::Muzzle(Battery::Main) => Sound::MainGun,
            EffectKind::Muzzle(Battery::Secondary | Battery::EnemySecondary) => Sound::SecondaryGun,
            EffectKind::Muzzle(Battery::Raider) => Sound::RaiderGun,
            EffectKind::Muzzle(Battery::Shore) => Sound::ShoreGun,
            EffectKind::Splash => Sound::Splash,
            // A ram has no voice of its own yet and borrows the shell burst.
            EffectKind::Burst | EffectKind::Crunch | EffectKind::MountLost(_) => Sound::Burst,
            EffectKind::Armor {
                outcome: Impact::Bounced,
                ..
            } => Sound::ArmorRing,
            EffectKind::Armor {
                outcome: Impact::Embedded,
                ..
            } => Sound::ArmorKnock,
            EffectKind::Armor { .. } => Sound::Burst,
            EffectKind::Wreck(_) => Sound::Wreck,
            // A factory crumbles. The gantry takes the whole yard down with it.
            EffectKind::Demolition { dock: false } => Sound::Collapse,
            EffectKind::Demolition { dock: true } => Sound::Ruin,
        };
        board.cues.send(Cue::Emit {
            sound,
            position: Vec3::new(fx.body.pos.x, fx.height, fx.body.pos.y),
            serial: fx.body.id,
        });
        // The ship's own guns are felt through the deck before they are heard.
        let felt = match fx.kind {
            EffectKind::Muzzle(Battery::Main) => Sound::Recoil,
            EffectKind::Muzzle(Battery::Secondary) => Sound::Kick,
            EffectKind::Wreck(_) => Sound::Trumpet,
            // A base destroyed is cheered at once. Its fall is heard when it arrives.
            EffectKind::Demolition { dock: true } => Sound::Fanfare,
            _ => continue,
        };
        board.cues.send(Cue::Emit {
            sound: felt,
            position: Vec3::new(helm.position.x, BRIDGE, helm.position.y),
            serial: fx.body.id,
        });
    }
    // Whatever stands out of the sea throws a report back: the bases, the islands
    // and any ice big enough. The mixer has room for a few, so it is told each tick of
    // those that will answer loudest, which are the big and the near.
    let bases = battle
        .ports()
        .iter()
        .map(|port| (port.body.pos, BLUFF, ISLAND_RADIUS));
    let islands = battle
        .landmarks()
        .iter()
        .map(|land| (land.body.pos, BLUFF, land.radius));
    let ice = battle.icebergs().iter().filter(|berg| berg.radius >= FLOE);
    let ice = ice.map(|berg| (berg.body.pos, 0.5 * berg.top(), berg.radius));
    let mut shores: Vec<(Vec2, f32, f32)> = bases.chain(islands).chain(ice).collect();
    let answers = |&(at, _, radius): &(Vec2, f32, f32)| {
        radius.min(2. * ISLAND_RADIUS) / at.distance_squared(helm.position).max(1.)
    };
    shores.sort_by(|a, b| answers(b).total_cmp(&answers(a)));
    for slot in 0..SHORES {
        let (at, height, radius) = shores.get(slot).copied().unwrap_or_default();
        board.cues.send(Cue::Shore {
            slot,
            position: Vec3::new(at.x, height, at.y),
            radius,
        });
    }
    // A base sounds its siren while it has word of the ship or is being hit, and for
    // a while after. A ruined yard has no power, and its siren is left to run down.
    let now = battle.at_sea();
    let (seen, until) = &mut *alarms;
    if now < *seen {
        // The clock has gone back, so this is a new voyage and no alarm is up yet.
        *until = [0.; 3];
    }
    *seen = now;
    for (slot, (port, until)) in battle.ports().iter().zip(until).enumerate() {
        let struck = battle.effects().iter().any(|fx| {
            let burst = matches!(
                fx.kind,
                EffectKind::Burst | EffectKind::Demolition { .. } | EffectKind::MountLost(_)
            );
            burst && fx.age == 0. && fx.body.pos.distance(port.body.pos) < ASHORE
        });
        if port.has_contact || struck {
            *until = now + ALARM;
        }
        let yard = port.yard_position();
        board.cues.send(Cue::Alarm {
            slot,
            rotor: Rotor::Shore,
            position: Vec3::new(yard.x, MAST, yard.y),
            raised: now < *until && port.yard_state() != YardState::Ruined,
        });
    }
    // Every place makes its own sounds. A yard is at work until it is wrecked, and
    // downs tools while its siren is going.
    let land = battle.landmarks().iter().map(|land| Place {
        haunt: match land.kind {
            LandmarkKind::Glacier => Haunt::Glacier,
            _ => Haunt::Island,
        },
        position: Vec3::new(land.body.pos.x, BLUFF, land.body.pos.y),
        radius: land.radius,
    });
    let harbours = battle.ports().iter().map(|port| Place {
        haunt: Haunt::Island,
        position: Vec3::new(port.body.pos.x, BLUFF, port.body.pos.y),
        radius: ISLAND_RADIUS,
    });
    let quiet = alarms.1.iter().map(|until| now >= *until);
    let yards = battle.ports().iter().zip(quiet).filter_map(|(port, quiet)| {
        let working = matches!(port.yard_state(), YardState::Working | YardState::Damaged);
        let yard = port.yard_position();
        (working && quiet).then_some(Place {
            haunt: Haunt::Yard,
            position: Vec3::new(yard.x, SLIP, yard.y),
            radius: 0.,
        })
    });
    let bergs = battle.icebergs().iter().filter(|berg| berg.radius >= FLOE);
    let bergs = bergs.map(|berg| Place {
        haunt: Haunt::Berg,
        position: Vec3::new(berg.body.pos.x, 0.5 * berg.top(), berg.body.pos.y),
        radius: berg.radius,
    });
    around.ambience.pass(
        clock.delta_secs(),
        bridge,
        helm.speed / Helm::FULL_AHEAD,
        land.chain(harbours).chain(yards).chain(bergs),
        &board.cues,
    );
    // A flagship announces her sortie on a siren she carries with her. Sunk, it is
    // left to run down where she was last seen.
    let sailed = matches!(battle.flagship(), Some(Sortie::Sailed { at, .. }) if now - at < HERALD);
    let flagship = battle.raiders().iter().find(|r| r.class.is_flagship());
    if let Some(ship) = flagship {
        *herald = Vec3::new(ship.body.pos.x, FUNNEL, ship.body.pos.y);
    }
    board.cues.send(Cue::Alarm {
        slot: FLAGSHIP,
        rotor: Rotor::Ship,
        position: *herald,
        raised: sailed && flagship.is_some(),
    });
    // A shell's velocity is how far it came since the last tick. One fired this tick
    // is first heard on the next.
    let dt = clock.delta_secs();
    let shells = battle.shots().iter().map(|shot| {
        let position = shot.position();
        let before = aloft.iter().find(|(id, _)| *id == shot.body.id);
        if let Some(&(_, before)) = before.filter(|(_, before)| dt > 0. && *before != position) {
            let weight = match shot.battery {
                Battery::Main => shot.shell.scale(),
                Battery::Shore => 1.2,
                Battery::Raider => 0.7,
                Battery::Secondary | Battery::EnemySecondary => 0.4,
            };
            board.cues.send(Cue::Shell {
                serial: shot.body.id,
                position,
                velocity: (position - before) / dt,
                weight,
            });
        }
        (shot.body.id, position)
    });
    *aloft = shells.collect();
}

/// Sound in the air waits out a pause or a fitting choice with the battle that made it.
/// Off the water the engines are quiet and only the sea is heard.
pub fn keep_time(run: Res<Campaign>, mut board: ResMut<Soundboard>) {
    let flowing = !matches!(run.screen(), Screen::Paused | Screen::Refit);
    if flowing != board.flowing {
        board.flowing = flowing;
        board.cues.send(Cue::Flow(flowing));
    }
    let steaming = run.under_way() || !flowing;
    if steaming != board.steaming {
        board.steaming = steaming;
        if !steaming {
            board.cues.send(Cue::Moored);
        }
    }
}

/// A new voyage starts in still air.
pub fn hush(_: On<Embarked>, board: Res<Soundboard>) {
    board.cues.send(Cue::Hush);
}

/// Plays the mixer through the audio device. The game sends cues with or without it.
pub struct SoundPlugin;
impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<Loudspeaker>()
            .add_systems(Startup, connect);
    }
}

fn connect(
    board: Res<Soundboard>,
    mut speakers: ResMut<Assets<Loudspeaker>>,
    mut commands: Commands,
) {
    if let Some(mixer) = board.claim() {
        let speaker = speakers.add(Loudspeaker(Mutex::new(Some(mixer))));
        commands.spawn(AudioPlayer(speaker));
    }
}

/// An audio source that never ends: the mixer's output, for as long as the game runs.
#[derive(Asset, TypePath)]
struct Loudspeaker(Mutex<Option<Mixer>>);
impl Decodable for Loudspeaker {
    type Decoder = Line;
    fn decoder(&self) -> Line {
        let mixer = self.0.lock().ok().and_then(|mut mixer| mixer.take());
        debug!(
            "audio output opened, mixer on the line: {}",
            mixer.is_some()
        );
        Line(mixer.map(Mixer::feed))
    }
}

/// The mixer as the audio device reads it. A second line from one speaker is silent.
struct Line(Option<Feed>);
impl Iterator for Line {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        self.0.as_mut()?.next()
    }
}
impl Source for Line {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        ChannelCount::new(2).expect("stereo")
    }
    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(SAMPLE_RATE).expect("a positive rate")
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
