//! What the bridge hears. Each new effect in the battle becomes a cue for the mixer,
//! which plays on the audio thread and makes the listener wait for distant sound.
//! Shells are followed through the air, and the land and ice nearest the ship are
//! charted so they can echo.
//! A base that has word of the ship, or is being hit, sounds its siren, and a flagship
//! sounds a deeper one as she puts to sea.
//! The engines are heard as they are: damaged ones stumble and bang, and those knocked
//! out are silent. Around it all the places make their own sounds, which are surf,
//! seabirds, ice and the work of a yard. Each base's spotter is heard overhead.
use crate::{
    alert::BaseAlerts,
    camera::ShipView,
    state::{Campaign, Embarked, Encounter, PlayerHelm, Screen},
};
use bevy::{
    audio::{AddAudioSource, ChannelCount, SampleRate, Source},
    prelude::*,
    reflect::TypePath,
};
use shipvivor_acoustics::{
    Air, Alert, Ambience, Bank, Cue, Cues, Feed, Haunt, Mixer, Place, Rotor, SAMPLE_RATE, Sound,
    console,
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
/// Land that runs together is one shore. A point of it this far inside another piece
/// is buried, and no two answers are taken from shore closer together than `APART`.
const LAP: f32 = 1.;
const APART: f32 = 120.;
/// A mainland's surf and birds are heard from the stretches of its shore within this
/// many metres, each of them about what one small island would give.
const SURF: f32 = 420.;
/// Height of a base's siren, on a mast over its yard.
const MAST: f32 = 14.;
/// Seconds a flagship sounds her own siren as she puts to sea, the slot the mixer
/// keeps it in after the three bases, and its height above the water.
const HERALD: f32 = 20.;
const FLAGSHIP: usize = 3;
const FUNNEL: f32 = 16.;

/// Height above the water from which a yard's work is heard.
const SLIP: f32 = 6.;

/// An aeroplane that has moved further than this in one tick has been put somewhere
/// new, and has no speed to be reckoned from the move.
const LEAP: f32 = 5.;

/// Blasts of the klaxon when the ship first comes close to sinking, the seconds from
/// one to the next, and the seconds before they are sounded again while she stays so.
const KLAXON: (usize, f32, f32) = (3, 2.2, 20.);

/// The klaxon's button: whether the ship was in peril at the last tick, the blasts
/// still to sound, when the next is due and when the whole call is due again.
#[derive(Default)]
struct Klaxon {
    peril: bool,
    left: usize,
    next: f32,
    again: f32,
}

/// What `report` keeps from one tick to the next about the ship's surroundings: the
/// maker of their sounds, the engine order last rung, when the battle was last seen,
/// and where each aircraft was.
#[derive(Default)]
pub struct Surroundings {
    ambience: Ambience,
    order: Option<f32>,
    klaxon: Klaxon,
    seen: f32,
    aircraft: [Option<Vec3>; 4],
}

/// A piece of land or ice as sound meets it: a disc at the waterline, and the height
/// of the part of it that throws sound back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Disc {
    pub at: Vec2,
    pub radius: f32,
    pub height: f32,
}

/// The shore that a ship at `from` has before her. Each disc is given with the point
/// of it nearest her, wherever that point is the water's edge and not buried in a
/// neighbour. Land runs together, a mainland most of all, and what answers a gun or
/// carries surf is the edge of the whole.
pub fn open_shore(from: Vec2, discs: &[Disc]) -> Vec<(Disc, Vec2)> {
    let faces = discs.iter().filter_map(|disc| {
        let face = disc.at + (from - disc.at).normalize_or_zero() * disc.radius;
        let buries = |other: &Disc| other != disc && face.distance(other.at) < other.radius - LAP;
        (!discs.iter().any(buries)).then_some((*disc, face))
    });
    faces.collect()
}

/// The best of these stretches of shore, by `worth`, taking none within `apart` metres
/// of one already taken and no more than `most`.
pub fn spaced(
    mut shore: Vec<(Disc, Vec2)>,
    worth: impl Fn(&(Disc, Vec2)) -> f32,
    apart: f32,
    most: usize,
) -> Vec<(Disc, Vec2)> {
    shore.sort_by(|a, b| worth(b).total_cmp(&worth(a)));
    let mut kept: Vec<(Disc, Vec2)> = Vec::with_capacity(most);
    for stretch in shore {
        if kept.len() < most
            && kept
                .iter()
                .all(|taken| taken.1.distance(stretch.1) >= apart)
        {
            kept.push(stretch);
        }
    }
    kept
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
    alerts: Res<BaseAlerts>,
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
        around.klaxon = Klaxon::default();
    }
    if around.order.is_some_and(|last| last != ordered) {
        board.cues.send(Cue::Emit {
            sound: Sound::Telegraph,
            position: bridge,
            serial: voyage.to_bits().into(),
        });
    }
    (around.order, around.seen) = (Some(ordered), voyage);
    // One more module lost would sink her. The klaxon says so when it first becomes
    // true, and again every so often for as long as it stays true.
    let (peril, klaxon) = (run.ship.in_peril(), &mut around.klaxon);
    if peril && (!klaxon.peril || voyage >= klaxon.again) {
        (klaxon.left, klaxon.next, klaxon.again) = (KLAXON.0, voyage, voyage + KLAXON.2);
    }
    if !peril {
        klaxon.left = 0;
    }
    klaxon.peril = peril;
    if klaxon.left > 0 && voyage >= klaxon.next {
        board.cues.send(Cue::Emit {
            sound: Sound::Klaxon,
            position: bridge,
            serial: voyage.to_bits().into(),
        });
        klaxon.left -= 1;
        klaxon.next += KLAXON.1;
    }
    for fx in battle.effects().iter().filter(|fx| fx.age == 0.) {
        let sound = match fx.kind {
            EffectKind::Muzzle(Battery::Main) => Sound::MainGun,
            EffectKind::Muzzle(
                Battery::Secondary | Battery::EnemySecondary | Battery::Aircraft | Battery::AntiAir,
            ) => Sound::SecondaryGun,
            EffectKind::Muzzle(Battery::Raider) => Sound::RaiderGun,
            EffectKind::Muzzle(Battery::Shore) => Sound::ShoreGun,
            EffectKind::Splash => Sound::Splash,
            // A ram has no voice of its own yet and borrows the shell burst.
            EffectKind::Burst
            | EffectKind::Crunch
            | EffectKind::MountLost(_)
            | EffectKind::Flak => Sound::Burst,
            // A stricken aircraft is heard when it meets the water.
            EffectKind::Downed(_) => continue,
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
    // Whatever stands out of the sea throws a report back: the bases, the islands, the
    // mainland and any ice big enough. The mixer has room for a few, so it is told each
    // tick of those that will answer loudest, which are the big and the near. Where
    // land runs together only its open shore answers, and no stretch of it twice.
    let disc = |at: Vec2, radius: f32, height: f32| Disc { at, radius, height };
    let bases = battle.ports().iter();
    let bases = bases.map(|port| disc(port.body.pos, ISLAND_RADIUS, BLUFF));
    let islands = battle.landmarks().iter();
    let islands = islands.map(|land| disc(land.body.pos, land.radius, BLUFF));
    let ice = battle.icebergs().iter().filter(|berg| berg.radius >= FLOE);
    let ice = ice.map(|berg| disc(berg.body.pos, berg.radius, 0.5 * berg.top()));
    let discs: Vec<Disc> = bases.chain(islands).chain(ice).collect();
    let shore = open_shore(helm.position, &discs);
    let answers = |(disc, face): &(Disc, Vec2)| {
        disc.radius.min(2. * ISLAND_RADIUS) / face.distance_squared(helm.position).max(1.)
    };
    let echoing = spaced(shore.clone(), answers, APART, SHORES);
    for slot in 0..SHORES {
        let (at, radius, height) = echoing.get(slot).map_or((Vec2::ZERO, 0., 0.), |(disc, _)| {
            (disc.at, disc.radius, disc.height)
        });
        board.cues.send(Cue::Shore {
            slot,
            position: Vec3::new(at.x, height, at.y),
            radius,
        });
    }
    // A base sounds its siren while it has word of the ship or is being hit, and for
    // a while after. A ruined yard has no power, and its siren is left to run down.
    let now = battle.at_sea();
    for (slot, port) in battle.ports().iter().enumerate() {
        let yard = port.yard_position();
        board.cues.send(Cue::Alarm {
            slot,
            rotor: Rotor::Shore,
            position: Vec3::new(yard.x, MAST, yard.y),
            alert: match port.yard_state() {
                // A base that is finished is heard to be: its siren comes down with
                // the gantry and its note drops away to nothing.
                YardState::Ruined => Alert::Wrecked,
                _ if alerts.raised(slot, &battle) => Alert::Raised,
                _ => Alert::Lowered,
            },
        });
    }
    // Every place makes its own sounds. A yard is at work until it is wrecked, and
    // downs tools while its siren is going. A mainland is a hundred pieces run together,
    // far too many to have each its own surf, so it is heard as the stretches of its
    // open shore that lie near the ship.
    let mainland: Vec<Disc> = (battle.landmarks().iter())
        .filter(|land| land.kind == LandmarkKind::Mainland)
        .map(|land| disc(land.body.pos, land.radius, BLUFF))
        .collect();
    let near = |(_, face): &(Disc, Vec2)| face.distance(helm.position) < SURF;
    let coast = shore
        .into_iter()
        .filter(|stretch| mainland.contains(&stretch.0) && near(stretch));
    let nearest = |(_, face): &(Disc, Vec2)| -face.distance(helm.position);
    let coast = spaced(coast.collect(), nearest, APART, SHORES);
    let coast = coast.into_iter().map(|(_, face)| {
        // Each stretch is given as a small island whose shore lies along it.
        let inland = (face - helm.position).normalize_or_zero();
        let middle = face + inland * ISLAND_RADIUS;
        Place {
            haunt: Haunt::Coast,
            position: Vec3::new(middle.x, BLUFF, middle.y),
            radius: ISLAND_RADIUS,
        }
    });
    let islands = battle.landmarks().iter();
    let islands = islands.filter(|land| land.kind != LandmarkKind::Mainland);
    let land = islands.map(|land| Place {
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
    let quiet = (0..3).map(|base| !alerts.raised(base, &battle));
    let yards = battle
        .ports()
        .iter()
        .zip(quiet)
        .filter_map(|(port, quiet)| {
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
        land.chain(coast).chain(harbours).chain(yards).chain(bergs),
        &board.cues,
    );
    // The fourth engine belongs to a shore-based attacker while it is aloft.
    let dt = clock.delta_secs();
    let attacker = battle.air_station().and_then(|base| base.aircraft.as_ref());
    let positions = battle
        .spotters()
        .iter()
        .map(|p| p.airborne().then(|| p.position_3d()))
        .chain([attacker.map(|p| p.position_3d())]);
    for (slot, (now, before)) in positions.zip(&mut around.aircraft).enumerate() {
        let Some(position) = now.or(*before) else {
            continue;
        };
        let moved = position - before.unwrap_or(position);
        let steady = dt > 0. && moved.length() < LEAP;
        board.cues.send(Cue::Aircraft {
            slot,
            position,
            velocity: if steady { moved / dt } else { Vec3::ZERO },
            flying: now.is_some(),
        });
        *before = now;
    }
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
        alert: if sailed && flagship.is_some() {
            Alert::Raised
        } else {
            Alert::Lowered
        },
    });
    // A shell's velocity is how far it came since the last tick. One fired this tick
    // is first heard on the next.
    let shells = battle.shots().iter().map(|shot| {
        let position = shot.position();
        let before = aloft.iter().find(|(id, _)| *id == shot.body.id);
        if let Some(&(_, before)) = before.filter(|(_, before)| dt > 0. && *before != position) {
            let weight = match shot.battery {
                Battery::Main => shot.shell.scale(),
                Battery::Shore => 1.2,
                Battery::Raider => 0.7,
                Battery::Secondary | Battery::EnemySecondary | Battery::Aircraft => 0.4,
                Battery::AntiAir => 0.25,
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
    let flowing = !matches!(
        run.screen(),
        Screen::Paused | Screen::PassageMap | Screen::Refit
    );
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

/// A local interface sound that plays while the encounter is paused.
#[derive(Event)]
pub struct Notice(pub Sound);

pub fn play_notice(notice: On<Notice>, board: Res<Soundboard>) {
    board.cues.send(Cue::Emit {
        sound: notice.event().0,
        position: Vec3::ZERO,
        serial: 0,
    });
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
