//! What the bridge hears. Each new effect in the battle becomes a cue for the mixer,
//! which plays on the audio thread and makes the listener wait for distant sound.
//! Shells are followed through the air, and the land and ice nearest the ship are
//! charted so they can echo.
//! A base that has word of the ship, or is being hit, sounds its siren, and a boss
//! sounds her horn as she puts to sea: the bigger she is, the deeper it is.
//! The engines are heard as they are: damaged ones stumble and bang, and those knocked
//! out are silent. An engine is heard to take the blow that damages it and to die when
//! it is knocked out, and the repair party is heard at work where it has been sent. Around it all the places make their own sounds, which are surf,
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
    AIRCRAFT, Air, Alert, Ambience, Bank, Cue, Cues, Feed, Haunt, Mixer, Place, Rotor, SAMPLE_RATE,
    Sound, console,
};
use shipvivor_compass::direction;
use shipvivor_encounter::{
    Battery, EffectKind, Hull, ISLAND_RADIUS, Impact, LandmarkKind, YardState,
};
use shipvivor_navigation::Helm;
use shipvivor_shipyard::{Component, State, Task};
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
/// Height above the water of a boss's horn, on her foremast.
const HORN: f32 = 16.;
/// The mixer's voices for base sirens, and for floatplanes. The fourth aircraft voice
/// belongs to a shore-based attacker, and the rest to the harriers aloft. A sector may
/// hold more bases than that, so the voices go to the nearest roused bases, and if
/// more harriers are aloft than there are voices, to the nearest of them.
const SIRENS: usize = 3;
const SPOTTERS: usize = 3;
const HARRIERS: usize = {
    let (aloft, voices) = (shipvivor_encounter::MOST_ALOFT, AIRCRAFT - SPOTTERS - 1);
    if aloft < voices { aloft } else { voices }
};

/// Height above the water from which a yard's work is heard.
const SLIP: f32 = 6.;

/// Seconds before an enemy torpedo would reach her that the alarm bell is rung for it,
/// and the metres beyond her ends within which a track counts as closing on her.
const TORPEDO_WARNING: f32 = 5.5;
const TORPEDO_MARGIN: f32 = 4.;

/// An aeroplane that has moved further than this in one tick has been put somewhere
/// new, and has no speed to be reckoned from the move.
const LEAP: f32 = 5.;

/// Height above the water at which the repair party works, how far either side of the
/// centreline, and the seconds after the party is sent before it is first heard.
const DECK: f32 = 2.2;
const ABEAM: f32 = 1.6;
const SENT: f32 = 0.5;
/// The repair party's work: each tool, how often it is taken up against the others, and
/// the shortest and longest seconds from taking it up to the next.
const TOOLS: [(Sound, f32, (f32, f32)); 3] = [
    (Sound::Hammer, 0.45, (1.8, 3.2)),
    (Sound::Ratchet, 0.3, (2., 3.4)),
    (Sound::Torch, 0.25, (3.2, 4.4)),
];

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

/// Seconds left on the leg's clock when the bridge clock starts to be heard.
const LAST_MINUTE: f32 = 60.;
/// Seconds the ticking takes to swell in, and to fade out once a base puts time back.
const TICK_FADE: f32 = 1.5;
/// How loud a tick is at the minute, against its full strength in the last seconds.
const FIRST_TICK: f32 = 0.45;
/// Seconds of the clock a base's time holds the ticking off for, so even time won in
/// the last seconds, which leaves the clock inside its last minute, is a relief.
const TICK_GRACE: f32 = 15.;
/// The ticking's strength below which a fading beat is no longer struck: about three
/// dying beats once a base puts time back.
const TICK_FLOOR: f32 = 0.1;

/// The bridge clock counting out a leg's last minute: soft at the minute and firmer as
/// it runs down. Time put back on the clock lets the ticking die away, not stop dead.
#[derive(Default)]
struct Countdown {
    /// The whole seconds the clock read at the last look.
    shown: Option<i32>,
    /// How strongly it is ticking, from 0 to 1.
    level: f32,
    /// Seconds on the leg's clock at the last look, to hear time put on it.
    clock: f32,
    /// Seconds left that the clock must come down to before it ticks again, once a base
    /// has put time on it.
    hush: Option<f32>,
}

/// What `report` keeps from one tick to the next about the ship's surroundings: the
/// maker of their sounds, the engine order last rung, when the battle was last seen,
/// and where each aircraft was.
#[derive(Default)]
pub struct Surroundings {
    ambience: Ambience,
    order: Option<f32>,
    klaxon: Klaxon,
    countdown: Countdown,
    seen: f32,
    aircraft: [Option<Vec3>; SPOTTERS + 1 + HARRIERS],
    /// Which base each siren voice and each floatplane voice is following, and which
    /// harrier each harrier voice.
    sirens: [Option<usize>; SIRENS],
    spotters: [Option<usize>; SPOTTERS],
    harriers: [Option<u64>; HARRIERS],
    /// Where each siren voice last sounded from.
    siren_at: [Vec3; SIRENS],
    /// Enemy torpedoes already heard leaving the tube, and those the alarm bell has
    /// already been rung for.
    torpedoes: Vec<u64>,
    alarmed: Vec<u64>,
    /// The boss whose horn has sounded this voyage.
    boss: Option<u64>,
    /// The state each engine was last heard in.
    engines: Vec<State>,
    /// When the repair party next takes up a tool, and how many it has taken up.
    mending: f32,
    tools: u64,
}

/// A share from 0 to 1 that is the same for the same `seed`.
fn chance(seed: u64) -> f32 {
    (seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 40) as f32 / (1 << 24) as f32
}

/// Keep each voice on the base or aircraft it follows while that is still among those
/// `wanted`, and give each free voice to the next one wanted without a voice.
fn allot<T: Copy + PartialEq, const N: usize>(voices: &mut [Option<T>; N], wanted: &[T]) {
    for voice in voices.iter_mut() {
        if voice.is_some_and(|base| !wanted.contains(&base)) {
            *voice = None;
        }
    }
    for &base in wanted {
        if voices.contains(&Some(base)) {
            continue;
        }
        if let Some(free) = voices.iter_mut().find(|voice| voice.is_none()) {
            *free = Some(base);
        }
    }
}

/// The roused bases nearest `at` that `keep` lets through, nearest first, at most `n`.
fn nearest_bases(
    battle: &Encounter,
    at: Vec2,
    n: usize,
    keep: impl Fn(usize) -> bool,
) -> Vec<usize> {
    let mut bases: Vec<(usize, f32)> = (battle.ports().iter().enumerate())
        .filter(|&(i, port)| port.roused() && keep(i))
        .map(|(i, port)| (i, crate::mission_hud::anchor(port).distance_squared(at)))
        .collect();
    bases.sort_by(|a, b| a.1.total_cmp(&b.1));
    bases.into_iter().take(n).map(|(i, _)| i).collect()
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
        around.countdown = Countdown::default();
        around.torpedoes.clear();
        around.alarmed.clear();
        around.boss = None;
        around.engines.clear();
        around.mending = 0.;
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
    // The last minute of a patrol's clock ticks on the bridge, one beat a second. A base
    // wrecked puts time back on it, and the ticking fades away over the next beats and
    // holds off while the clock runs down through the grace.
    let left = run.remaining();
    let countdown = &mut around.countdown;
    if run.clock() > countdown.clock + 0.5 {
        countdown.hush = Some(left - TICK_GRACE);
    } else if run.clock() < countdown.clock - 0.5 {
        // A new leg's fresh clock owes nothing to the last one's bases.
        countdown.hush = None;
    }
    countdown.clock = run.clock();
    let counting = run.screen() == Screen::Sailing
        && run.stage != shipvivor_campaign::Stage::CrownDeep
        && left > 0.
        && left <= LAST_MINUTE.min(countdown.hush.unwrap_or(f32::INFINITY));
    let target = if counting { 1. } else { 0. };
    countdown.level += (target - countdown.level) * (clock.delta_secs() / TICK_FADE).min(1.);
    let second = left.ceil() as i32;
    if countdown.shown.is_some_and(|was| second < was) && second > 0 && countdown.level > TICK_FLOOR
    {
        let urgency = 1. - (left / LAST_MINUTE).clamp(0., 1.);
        board.cues.send(Cue::Notice {
            sound: Sound::Tick,
            level: countdown.level * (FIRST_TICK + (1. - FIRST_TICK) * urgency),
            serial: second.rem_euclid(2) as u64,
        });
    }
    countdown.shown = Some(second);
    // An engine is heard to take the blow that leaves it running rough, and to sputter
    // out when it is knocked out. One mended comes back with no more than its running.
    let states: Vec<State> = engines().map(|(_, state)| state).collect();
    if around.engines.len() == states.len() {
        for (was, now) in around.engines.iter().zip(&states) {
            let sound = match (was, now) {
                (State::Working, State::Damaged) => Sound::EngineHurt,
                (State::Working | State::Damaged, State::Out) => Sound::EngineStall,
                _ => continue,
            };
            board.cues.send(Cue::Emit {
                sound,
                position: bridge,
                serial: voyage.to_bits().into(),
            });
        }
    }
    around.engines = states;
    // The repair party is heard at work on deck over the section it was sent to:
    // hammering, a torch and spanners by turns, with a breath between.
    match run.ship.party().filter(|_| run.ship.afloat()) {
        Some(task) if voyage >= around.mending => {
            let module = match task {
                Task::Module(module) => module,
                Task::Fitting(berth) => berth.module,
            };
            let (fore, aft) = run.ship.design().span(module);
            let seed = around.tools;
            around.tools += 1;
            let pick = chance(seed);
            let mut taken = 0.;
            let (sound, _, (soonest, latest)) = *TOOLS
                .iter()
                .find(|(_, often, _)| {
                    taken += often;
                    pick < taken
                })
                .unwrap_or(&TOOLS[0]);
            let ahead = helm.forward();
            let at = helm.position
                + ahead * (Hull::CRUISER.bow - 0.5 * (fore + aft))
                + ahead.perp() * ABEAM * (2. * chance(seed ^ 0xA5) - 1.);
            board.cues.send(Cue::Emit {
                sound,
                position: Vec3::new(at.x, DECK, at.y),
                serial: seed,
            });
            around.mending = voyage + soonest + (latest - soonest) * chance(seed ^ 0x5A);
        }
        Some(_) => {}
        None => around.mending = voyage + SENT,
    }
    for fx in battle.effects().iter().filter(|fx| fx.age == 0.) {
        let sound = match fx.kind {
            EffectKind::Muzzle(Battery::Main) => Sound::MainGun,
            EffectKind::Muzzle(
                Battery::Secondary | Battery::EnemySecondary | Battery::Aircraft | Battery::AntiAir,
            ) => Sound::SecondaryGun,
            EffectKind::Muzzle(Battery::Raider) => Sound::RaiderGun,
            EffectKind::Muzzle(Battery::Shore) => Sound::ShoreGun,
            EffectKind::Muzzle(Battery::Heavy) => Sound::HeavyGun,
            EffectKind::Splash => Sound::Splash,
            EffectKind::Column => Sound::Column,
            // A ram has no voice of its own yet and borrows the shell burst.
            EffectKind::Burst
            | EffectKind::Crunch
            | EffectKind::MountLost(_)
            | EffectKind::StrakeLost(..)
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
            EffectKind::Torpedoed => Sound::Torpedoed,
            // A slab off an ice shelf comes apart like a glacier's front. Lesser ice cracks.
            EffectKind::Shatter(radius) if radius >= 19 => Sound::Calving,
            EffectKind::Shatter(_) => Sound::IceCrack,
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
    // An enemy torpedo is heard leaving its tube, from where it was fired, and the
    // bridge rings its alarm bell once for each whose track is closing on her.
    let way = direction(helm.heading) * helm.speed;
    let shape = run.hull_shape();
    let reach = shape.bow.max(shape.stern) + TORPEDO_MARGIN;
    let fish = battle.torpedoes().iter().filter(|t| t.hostile());
    for t in fish {
        let id = t.body.id;
        if !around.torpedoes.contains(&id) {
            around.torpedoes.push(id);
            board.cues.send(Cue::Emit {
                sound: Sound::TorpedoLaunch,
                position: Vec3::new(t.body.pos.x, 1.5, t.body.pos.y),
                serial: id,
            });
        }
        let closing = t.closing(helm.position, way, reach);
        if closing.is_some_and(|s| s <= TORPEDO_WARNING) && !around.alarmed.contains(&id) {
            around.alarmed.push(id);
            board.cues.send(Cue::Emit {
                sound: Sound::TorpedoAlarm,
                position: bridge,
                serial: id,
            });
        }
    }
    let running: Vec<u64> = battle.torpedoes().iter().map(|t| t.body.id).collect();
    around.torpedoes.retain(|id| running.contains(id));
    around.alarmed.retain(|id| running.contains(id));
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
    let near = nearest_bases(&battle, helm.position, SIRENS, |_| true);
    allot(&mut around.sirens, &near);
    for slot in 0..SIRENS {
        let alert = match around.sirens[slot] {
            Some(base) => {
                let port = &battle.ports()[base];
                let yard = crate::mission_hud::anchor(port);
                around.siren_at[slot] = Vec3::new(yard.x, MAST, yard.y);
                // A base that is finished is heard to be: its siren comes down with
                // the last of its works and its note drops away to nothing.
                if port.wrecked() {
                    Alert::Wrecked
                } else if alerts.raised(base, &battle) {
                    Alert::Raised
                } else {
                    Alert::Lowered
                }
            }
            None => Alert::Lowered,
        };
        board.cues.send(Cue::Alarm {
            slot,
            rotor: Rotor::Shore,
            position: around.siren_at[slot],
            alert,
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
    let quiet = (0..battle.ports().len()).map(|base| !alerts.raised(base, &battle));
    let yards = battle
        .ports()
        .iter()
        .zip(quiet)
        .filter(|&(_, quiet)| quiet)
        .flat_map(|(port, _)| {
            // Each slip at work is heard on its own.
            (0..port.docks().len())
                .filter(|&i| {
                    matches!(
                        port.docks()[i].state(),
                        YardState::Working | YardState::Damaged
                    )
                })
                .map(|i| {
                    let yard = port.dock_position(i);
                    Place {
                        haunt: Haunt::Yard,
                        position: Vec3::new(yard.x, SLIP, yard.y),
                        radius: 0.,
                    }
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
    let flying = |i: usize| battle.spotters().get(i).is_some_and(|p| p.airborne());
    let near = nearest_bases(&battle, helm.position, SPOTTERS, flying);
    allot(&mut around.spotters, &near);
    let spotter = |voice: Option<usize>| {
        let plane = &battle.spotters()[voice?];
        plane.airborne().then(|| plane.position_3d())
    };
    let mut harriers: Vec<(u64, f32)> = (battle.harriers().iter())
        .map(|p| (p.body.id, p.body.pos.distance_squared(helm.position)))
        .collect();
    harriers.sort_by(|a, b| a.1.total_cmp(&b.1));
    let harriers: Vec<u64> = harriers
        .into_iter()
        .take(HARRIERS)
        .map(|(id, _)| id)
        .collect();
    allot(&mut around.harriers, &harriers);
    let harrier = |voice: Option<u64>| {
        let plane = battle
            .harriers()
            .iter()
            .find(|p| Some(p.body.id) == voice)?;
        Some(plane.position_3d())
    };
    let positions = (around.spotters.map(spotter).into_iter())
        .chain([attacker.map(|p| p.position_3d())])
        .chain(around.harriers.map(harrier));
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
    // A boss sounds her horn as she puts out, or as the leg opens if she is at sea
    // already, as the Regent is. The horn goes with her, and a longer ship's is
    // deeper. Sunk, her last blast is left to cross the water from where she was.
    if let Some(ship) = battle.raiders().iter().find(|r| r.class.is_boss()) {
        let hull = ship.class.profile().hull;
        let blow = around.boss != Some(ship.body.id);
        around.boss = Some(ship.body.id);
        board.cues.send(Cue::Horn {
            position: Vec3::new(ship.body.pos.x, HORN, ship.body.pos.y),
            length: hull.bow + hull.stern,
            blow,
        });
    }
    // A shell's velocity is how far it came since the last tick. One fired this tick
    // is first heard on the next.
    let shells = battle.shots().iter().map(|shot| {
        let position = shot.position();
        let before = aloft.iter().find(|(id, _)| *id == shot.body.id);
        if let Some(&(_, before)) = before.filter(|(_, before)| dt > 0. && *before != position) {
            let weight = match shot.battery {
                Battery::Main => shot.shell.scale(),
                Battery::Shore => 1.2,
                // A heavy shell's rush is deep and carries.
                Battery::Heavy => 2.6,
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
