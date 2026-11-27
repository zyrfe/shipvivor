//! Renders what the bridge hears to a WAV file, through the game's air. Seventeen scenes:
//! the chime for an upgrade, the handbell for a yard point won from cargo, the scope's
//! ring clicked through its detents, the klaxon of a ship close to sinking, a torpedo fired at her, getting under way, an engine damaged and then knocked out, the repair party at work, lying off a glacier and
//! a working yard with a floatplane circling, firing the main battery among islands,
//! coming under fire, a heavy battery ranging on her, shelling a base with its siren going until it falls, a base's
//! siren and a flagship's alone, the horns of the three bosses putting out, a floatplane
//! flying past, and every sound in the bank alone at three ranges. The times of each are
//! printed.
//!
//! `cargo run -p shipvivor-acoustics --example audition -- tour.wav`
//!
//! With `horns` after the file name only the horns are rendered:
//! `cargo run -p shipvivor-acoustics --example audition -- horns.wav horns`
//!
//! With `ring` only the scope's ring is rendered:
//! `cargo run -p shipvivor-acoustics --example audition -- ring.wav ring`
//!
//! With `damage` only an engine hurt, given out and mended, and the repair party at
//! work, are rendered:
//! `cargo run -p shipvivor-acoustics --example audition -- damage.wav damage`
//!
//! With `shore` only a shore gun heard at three ranges, a triple's salvo and a heavy
//! battery are rendered:
//! `cargo run -p shipvivor-acoustics --example audition -- shore.wav shore`
//!
//! With `hits` only rounds striking her are rendered, under way at half ahead: rounds
//! her plate turns, from a tap to a deep clang, rounds that get through, from a light
//! punch to a heavy round through bare plate, and sections lost:
//! `cargo run -p shipvivor-acoustics --example audition -- hits.wav hits`
use glam::Vec3;
use shipvivor_acoustics::{
    Air, Alert, Ambience, Bank, Cue, Cues, Harm, Haunt, Hit, Mixer, Place, Rotor, SAMPLE_RATE,
    Sound, console,
};
use std::{env, fs, io};

const TICK: f32 = 1. / 60.;
const BRIDGE: Vec3 = Vec3::new(0., 7.5, 0.);
const RANGES: [f32; 3] = [18., 145., 450.];
const ISLANDS: [Vec3; 3] = [
    Vec3::new(-180., 5., -120.),
    Vec3::new(260., 5., 90.),
    Vec3::new(40., 5., -420.),
];

/// Something that happens at a moment in a scene.
enum Event {
    Sound(Sound, Vec3),
    /// A round fired from one point to land at another, at this speed and weight,
    /// with the sound of the gun that fired it and the sound it makes where it lands.
    Round(Sound, Vec3, Vec3, f32, f32, Sound),
    /// The same on a high lob that falls by this many metres per second squared, as a
    /// heavy battery's shell does.
    Lob(Sound, Vec3, Vec3, f32, f32, Sound, f32),
    Order(f32),
    /// Shares of the engines damaged and knocked out.
    Engines(f32, f32),
    /// A siren of this make raised or lowered at a place.
    Alarm(Rotor, Vec3, bool),
    /// A ship this many metres long sounds her horn at a place.
    Horn(Vec3, f32),
    /// An aeroplane takes up this course, or with none its engine is cut.
    Plane(Option<Course>),
    /// The game speaking at a strength of its own, in this take.
    Notice(Sound, f32, u64),
    /// A blow on her, at a point on her, as her plating took it.
    Hit(Hit, Vec3),
}

/// The way an aeroplane flies.
#[derive(Clone, Copy)]
enum Course {
    /// From a place, at a velocity.
    Straight(Vec3, Vec3),
    /// Round a place: the radius of the circle and the radians flown each second.
    Round(Vec3, f32, f32),
}
impl Course {
    /// Where it is and how it is moving, this many seconds along.
    fn at(self, secs: f32) -> (Vec3, Vec3) {
        match self {
            Self::Straight(from, velocity) => (from + velocity * secs, velocity),
            Self::Round(middle, radius, rate) => {
                let (sin, cos) = (rate * secs).sin_cos();
                let out = Vec3::new(cos, 0., sin);
                let along = Vec3::new(-sin, 0., cos);
                (middle + radius * out, radius * rate * along)
            }
        }
    }
}

struct Stage {
    cues: Cues,
    mixer: Mixer,
    frames: Vec<[f32; 2]>,
    /// Rounds in the air: serial, from, to, speed, weight, seconds flown, the sound
    /// each will make where it lands, and the pull its arc falls by, if it is a lob.
    rounds: Vec<(u64, Vec3, Vec3, f32, f32, f32, Sound, Option<f32>)>,
    serial: u64,
    speed: f32,
    throttle: f32,
    /// What stands round about and makes sounds of its own.
    ambience: Ambience,
    places: Vec<Place>,
    /// The aeroplane's course and the seconds it has flown it.
    plane: Option<(Course, f32)>,
}
impl Stage {
    fn new() -> Self {
        let (cues, mixer) = console(Bank::standard(), Air::SCALED);
        cues.send(Cue::Ears {
            position: BRIDGE,
            right: Vec3::X,
        });
        Self {
            cues,
            mixer,
            frames: Vec::new(),
            rounds: Vec::new(),
            serial: 0,
            speed: 0.,
            throttle: 0.,
            ambience: Ambience::new(11),
            places: Vec::new(),
            plane: None,
        }
    }

    fn now(&self) -> f32 {
        self.frames.len() as f32 / SAMPLE_RATE as f32
    }

    fn emit(&mut self, sound: Sound, position: Vec3) {
        self.serial += 1;
        self.cues.send(Cue::Emit {
            sound,
            position,
            serial: self.serial,
        });
    }

    /// Play a scene of the given length. With `ship` the engines and wash are heard.
    fn play(&mut self, name: &str, secs: f32, ship: bool, script: &[(f32, Event)]) {
        let start = self.now();
        println!("{start:>5.0} s  {name}");
        for step in 0..(secs / TICK) as usize {
            for (at, event) in script {
                if step != (at / TICK) as usize {
                    continue;
                }
                match *event {
                    Event::Sound(sound, position) => self.emit(sound, position),
                    Event::Notice(sound, level, serial) => self.cues.send(Cue::Notice {
                        sound,
                        level,
                        serial,
                    }),
                    Event::Hit(hit, position) => {
                        self.serial += 1;
                        self.cues.send(Cue::Hit {
                            position,
                            serial: self.serial,
                            hit,
                        });
                    }
                    Event::Order(throttle) => {
                        self.throttle = throttle;
                        self.emit(Sound::Telegraph, BRIDGE);
                    }
                    Event::Plane(course) => {
                        self.plane = course.map(|course| (course, 0.));
                        if course.is_none() {
                            self.cues.send(Cue::Aircraft {
                                slot: 0,
                                position: Vec3::ZERO,
                                velocity: Vec3::ZERO,
                                flying: false,
                            });
                        }
                    }
                    Event::Engines(damaged, stopped) => {
                        self.cues.send(Cue::Engines { damaged, stopped });
                    }
                    Event::Alarm(rotor, position, raised) => self.cues.send(Cue::Alarm {
                        slot: usize::from(rotor == Rotor::Ship),
                        rotor,
                        position,
                        alert: if raised {
                            Alert::Raised
                        } else {
                            Alert::Lowered
                        },
                    }),
                    Event::Horn(position, length) => self.cues.send(Cue::Horn {
                        position,
                        length,
                        blow: true,
                    }),
                    Event::Round(gun, from, to, speed, weight, lands) => {
                        self.emit(gun, from);
                        match gun {
                            Sound::MainGun => self.emit(Sound::Recoil, BRIDGE),
                            Sound::SecondaryGun => self.emit(Sound::Kick, BRIDGE),
                            _ => {}
                        }
                        self.serial += 1;
                        self.rounds
                            .push((self.serial, from, to, speed, weight, 0., lands, None));
                    }
                    Event::Lob(gun, from, to, speed, weight, lands, drop) => {
                        self.emit(gun, from);
                        self.serial += 1;
                        let round = (self.serial, from, to, speed, weight, 0., lands, Some(drop));
                        self.rounds.push(round);
                    }
                }
            }
            if ship {
                self.speed = (self.speed + 1.35 * TICK).min(13.6 * self.throttle);
                self.cues.send(Cue::Way {
                    speed: self.speed,
                    throttle: self.throttle,
                });
                let places = self.places.iter().copied();
                self.ambience
                    .pass(TICK, BRIDGE, self.speed / 13.6, places, &self.cues);
            }
            if let Some((course, flown)) = &mut self.plane {
                let (position, velocity) = course.at(*flown);
                self.cues.send(Cue::Aircraft {
                    slot: 0,
                    position,
                    velocity,
                    flying: true,
                });
                *flown += TICK;
            }
            // Each round flies a shallow arc, or a lob a high one, and throws up a column
            // where it lands.
            let mut landed = Vec::new();
            self.rounds
                .retain_mut(|(serial, from, to, speed, weight, flown, lands, lob)| {
                    *flown += TICK;
                    let whole = from.distance(*to) / *speed;
                    let along = *flown / whole;
                    if along >= 1. {
                        landed.push((*to, *lands));
                        return false;
                    }
                    let lift =
                        lob.map_or(0.04 * from.distance(*to), |drop| drop * whole * whole / 8.);
                    let level = (*to - *from) / whole;
                    self.cues.send(Cue::Shell {
                        serial: *serial,
                        position: from.lerp(*to, along)
                            + Vec3::Y * lift * 4. * along * (1. - along),
                        velocity: level + Vec3::Y * lift * 4. * (1. - 2. * along) / whole,
                        weight: *weight,
                    });
                    true
                });
            for (point, sound) in landed {
                self.emit(sound, point);
                // A base destroyed is cheered aboard the moment it is seen to fall, and
                // its siren comes down with the rest.
                if sound == Sound::Ruin {
                    self.emit(Sound::Fanfare, BRIDGE);
                    self.cues.send(Cue::Alarm {
                        slot: 0,
                        rotor: Rotor::Shore,
                        position: point + Vec3::Y * 10.,
                        alert: Alert::Wrecked,
                    });
                }
            }
            let at = self.frames.len();
            self.frames
                .resize(at + (SAMPLE_RATE as f32 * TICK) as usize, [0.; 2]);
            self.mixer.render(&mut self.frames[at..]);
        }
        let heard = &self.frames[(start * SAMPLE_RATE as f32) as usize..];
        let power: f32 = heard.iter().flatten().map(|x| x * x).sum();
        let peak = heard.iter().flatten().fold(0_f32, |p, x| p.max(x.abs()));
        println!(
            "         level {:.1} dB, peak {:.1} dB",
            10. * (power / (heard.len() * 2) as f32).log10(),
            20. * peak.log10(),
        );
    }
}

/// The three bosses, by the metres from stem to stern that set their horns.
const BOSSES: [(&str, f32); 3] = [("Commodore", 58.), ("Parliament", 81.), ("Regent", 168.)];

/// Each boss sounding her horn as she puts out, 400 m off the starboard bow, and the
/// Regent's again from 120 m.
fn horns(stage: &mut Stage) {
    let off = Vec3::new(260., 16., -300.);
    for (name, length) in BOSSES {
        stage.play(
            &format!("the {name}'s horn ({length} m) putting out, 400 m off the starboard bow"),
            if length >= 120. { 27. } else { 16. },
            false,
            &[(0.5, Event::Horn(off, length))],
        );
    }
    stage.play(
        "the Regent's horn again, 120 m off the port bow",
        25.,
        false,
        &[(0.5, Event::Horn(Vec3::new(-70., 16., -95.), 168.))],
    );
}

/// A shore gun on the starboard bow heard from the bridge at half ahead, from close
/// aboard out past its reach, then a triple's salvo, and a heavy battery for scale.
/// Each scene is nine seconds and the gun fires one second in.
fn shore_guns(stage: &mut Stage) {
    stage.play(
        "half ahead rung, and she settles",
        6.,
        true,
        &[(0., Event::Order(0.5))],
    );
    let bearing = Vec3::new(0.6, 0., -0.8);
    let at = |range: f32| bearing * range + Vec3::Y * 6.65;
    for range in [150., 320., 600.] {
        stage.play(
            &format!("a light shore gun fires at {range} m"),
            9.,
            true,
            &[(1., Event::Sound(Sound::ShoreGun, at(range)))],
        );
    }
    let side = Vec3::new(0.8, 0., 0.6) * 1.3;
    stage.play(
        "a triple's salvo at 320 m",
        9.,
        true,
        &[
            (1., Event::Sound(Sound::ShoreGun, at(320.) - side)),
            (1., Event::Sound(Sound::ShoreGun, at(320.))),
            (1., Event::Sound(Sound::ShoreGun, at(320.) + side)),
        ],
    );
    stage.play(
        "a heavy battery fires at 900 m",
        9.,
        true,
        &[(1., Event::Sound(Sound::HeavyGun, at(900.)))],
    );
}

/// The scope's magnification ring, turned the way the mouse wheel turns it.
fn ring(stage: &mut Stage) {
    stage.play(
        "the scope's ring spun three detents in, then two out",
        2.,
        false,
        &[
            (0.3, Event::Sound(Sound::Detent, BRIDGE)),
            (0.35, Event::Sound(Sound::Detent, BRIDGE)),
            (0.4, Event::Sound(Sound::Detent, BRIDGE)),
            (1.2, Event::Sound(Sound::Detent, BRIDGE)),
            (1.25, Event::Sound(Sound::Detent, BRIDGE)),
        ],
    );
}

/// An engine hurt, given out and mended at half ahead, then the repair party at work
/// on a section forward of the bridge and then on one abaft it.
fn damage(stage: &mut Stage) {
    stage.play(
        "half ahead is rung; an engine is hurt and knocks, stumbles and bangs in its exhaust; it gives out and sputters to a stop; then it is mended",
        34.,
        true,
        &[
            (0., Event::Order(0.5)),
            (7., Event::Engines(1., 0.)),
            (7., Event::Sound(Sound::EngineHurt, BRIDGE)),
            (19., Event::Engines(0., 1.)),
            (19., Event::Sound(Sound::EngineStall, BRIDGE)),
            (28., Event::Engines(0., 0.)),
        ],
    );
    // The game sends the party's work from the section it is on, every second or two.
    let (forward, aft) = (Vec3::new(1., 2.5, -11.), Vec3::new(-1.5, 2.5, 15.));
    let mut seed = 0x2545_F491_u32;
    let mut unit = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed >> 8) as f32 / (1 << 24) as f32
    };
    let mut script = vec![(0., Event::Order(0.5))];
    let mut at = 1.5;
    while at < 26. {
        let tools = [Sound::Hammer, Sound::Hammer, Sound::Torch, Sound::Ratchet];
        let tool = tools[(unit() * 4.) as usize % 4];
        let place = if at < 13.5 { forward } else { aft };
        script.push((at, Event::Sound(tool, place)));
        at += 0.7 + 1.8 * unit();
    }
    stage.play(
        "the repair party at work at half ahead, 12 m forward of the bridge and then 17 m abaft it: hammer, torch and spanner",
        28.,
        true,
        &script,
    );
}

/// The bridge clock counting out a leg's last minute, at half ahead. Each scene gives
/// the seconds the clock reads at each beat, and the strengths follow `src/sound.rs`:
/// soft at the minute, full in the last seconds, gliding in and out over 1.5 s.
fn countdown(stage: &mut Stage) {
    stage.play(
        "half ahead rung, and she settles",
        6.,
        true,
        &[(0., Event::Order(0.5))],
    );
    let scenes: [(&str, Vec<f32>); 3] = [
        (
            "the clock comes into its last minute, soft",
            (54..=61).rev().map(|s| s as f32).collect(),
        ),
        (
            "its last twelve seconds, firming, and it runs out",
            (-3..=12).rev().map(|s| s as f32).collect(),
        ),
        (
            "a base wrecked with eight seconds left puts 45 s back: the ticking fades, holds off 15 s and comes back softer",
            (8..=13)
                .rev()
                .chain((33..=53).rev())
                .map(|s| s as f32)
                .collect(),
        ),
    ];
    for (name, reads) in scenes {
        let mut level: f32 = 0.;
        // Time put on the clock holds the ticking off through 15 s of grace.
        let mut hush = f32::INFINITY;
        let mut last = f32::INFINITY;
        let script: Vec<(f32, Event)> = reads
            .iter()
            .enumerate()
            .filter_map(|(beat, &left)| {
                if left > last + 1. {
                    hush = left - 15.;
                }
                last = left;
                let counting = left > 0. && left <= hush.min(60.);
                // A second of the game's glide toward its target.
                level += (f32::from(u8::from(counting)) - level) * (1. - (-1. / 1.5_f32).exp());
                let urgency = 1. - (left / 60.).clamp(0., 1.);
                (beat > 0 && left > 0. && level > 0.1).then(|| {
                    let strength = level * (0.45 + 0.55 * urgency);
                    let take = (left as i64).rem_euclid(2) as u64;
                    (beat as f32, Event::Notice(Sound::Tick, strength, take))
                })
            })
            .collect();
        stage.play(name, reads.len() as f32 + 1., true, &script);
    }
}

/// A run of blows on her under one title: each named, as her plating took it, and where.
type Volley = (&'static str, Vec<(&'static str, Hit, Vec3)>);

/// Rounds striking her under way at half ahead, a few seconds apart, each where it
/// struck: on her side amidships to starboard, forward to port, or aft. Every hit is
/// printed with the second it is heard at.
fn hits(stage: &mut Stage) {
    // A round of `calibre` inches into `plate` inches of plate, doing `dealt` damage
    // if it gets through, and costing her `harm`.
    let round = |calibre: f32, plate: f32, dealt: f32, harm| Hit {
        calibre: Some(calibre),
        plate,
        turned: calibre <= plate,
        dealt: if calibre <= plate { 0. } else { dealt },
        harm,
    };
    let none = Harm::None;
    let (amidships, forward, aft) = (
        Vec3::new(2.4, 2.2, 4.),
        Vec3::new(-2.2, 2.2, -10.),
        Vec3::new(2., 2.2, 14.),
    );
    let groups: [Volley; 3] = [
        (
            "rounds her plate turns",
            vec![
                (
                    "a secondary's 2.8-inch round on 19 inches: a tap",
                    round(2.8, 19., 0., none),
                    amidships,
                ),
                (
                    "the same round on 4 inches",
                    round(2.8, 4., 0., none),
                    forward,
                ),
                (
                    "a destroyer's 5-inch on 7 inches",
                    round(5., 7., 0., none),
                    amidships,
                ),
                (
                    "a cruiser's 8-inch on 10 inches: the old clank",
                    round(8., 10., 0., none),
                    aft,
                ),
                (
                    "a battleship's 12-inch on 13 inches: a deep clang",
                    round(12., 13., 0., none),
                    amidships,
                ),
                (
                    "a heavy battery's 15-inch on 19 inches",
                    round(15., 19., 0., none),
                    forward,
                ),
            ],
        ),
        (
            "rounds that get through",
            vec![
                (
                    "a 2.8-inch through stock plate, a scratch",
                    round(2.8, 1., 0.5, none),
                    amidships,
                ),
                (
                    "a 5-inch that only just gets through 4.5 inches",
                    round(5., 4.5, 3., none),
                    forward,
                ),
                (
                    "an 8-inch through stock plate",
                    round(8., 1., 6., none),
                    amidships,
                ),
                (
                    "a 12-inch through stock plate",
                    round(12., 1., 11., none),
                    aft,
                ),
                (
                    "a 15-inch through bare plate, a gun knocked out",
                    round(15., 0., 24., Harm::Knocked),
                    amidships,
                ),
            ],
        ),
        (
            "sections lost",
            vec![
                (
                    "an 8-inch that wrecks a section",
                    round(8., 1., 14., Harm::Wrecked),
                    amidships,
                ),
                (
                    "a 12-inch into a magazine",
                    round(12., 1., 20., Harm::Detonated),
                    aft,
                ),
            ],
        ),
    ];
    stage.play(
        "under way at half ahead",
        3.,
        true,
        &[(0., Event::Order(0.5))],
    );
    for (title, group) in &groups {
        let gap = if *title == "sections lost" { 6. } else { 3. };
        let start = stage.frames.len() as f32 / SAMPLE_RATE as f32;
        let script: Vec<(f32, Event)> = (group.iter().enumerate())
            .map(|(n, &(name, hit, at))| {
                println!("{:>8.1} s  {name}", start + 0.5 + gap * n as f32);
                (0.5 + gap * n as f32, Event::Hit(hit, at))
            })
            .collect();
        stage.play(title, 0.5 + gap * group.len() as f32, true, &script);
    }
    // Then each alone, with her machinery stopped, four seconds apart.
    let Stage { frames, .. } = std::mem::replace(stage, Stage::new());
    stage.frames = frames;
    let start = stage.frames.len() as f32 / SAMPLE_RATE as f32;
    let alone: Vec<_> = groups.iter().flat_map(|(_, group)| group).collect();
    let script: Vec<(f32, Event)> = (alone.iter().enumerate())
        .map(|(n, &&(name, hit, at))| {
            println!("{:>8.1} s  alone: {name}", start + 0.5 + 4. * n as f32);
            (0.5 + 4. * n as f32, Event::Hit(hit, at))
        })
        .collect();
    stage.play(
        "each alone, machinery stopped",
        0.5 + 4. * alone.len() as f32,
        false,
        &script,
    );
}

fn main() -> io::Result<()> {
    let path = env::args().nth(1).unwrap_or_else(|| "audition.wav".into());
    let mut stage = Stage::new();
    if env::args().nth(2).as_deref() == Some("damage") {
        damage(&mut stage);
        // Then each of them alone, twice over, from 15 m off and with no ship to hear.
        let Stage { mut frames, .. } = stage;
        let mut alone = Stage::new();
        let sounds = [
            Sound::EngineHurt,
            Sound::EngineStall,
            Sound::Hammer,
            Sound::Torch,
            Sound::Ratchet,
        ];
        println!(
            "{:>5.0} s  each alone, twice, 15 m off, five seconds each",
            frames.len() as f32 / SAMPLE_RATE as f32
        );
        for sound in sounds.into_iter().flat_map(|sound| [sound, sound]) {
            let at = alone.frames.len();
            alone.emit(sound, Vec3::new(9., 2.5, -12.));
            alone.frames.resize(at + 5 * SAMPLE_RATE as usize, [0.; 2]);
            alone.mixer.render(&mut alone.frames[at..]);
        }
        frames.append(&mut alone.frames);
        fs::write(&path, wav(&frames))?;
        println!("wrote {path}");
        return Ok(());
    }
    if env::args().nth(2).as_deref() == Some("horns") {
        horns(&mut stage);
        fs::write(&path, wav(&stage.frames))?;
        println!("wrote {path}");
        return Ok(());
    }
    if env::args().nth(2).as_deref() == Some("ring") {
        ring(&mut stage);
        fs::write(&path, wav(&stage.frames))?;
        println!("wrote {path}");
        return Ok(());
    }
    if env::args().nth(2).as_deref() == Some("shore") {
        shore_guns(&mut stage);
        fs::write(&path, wav(&stage.frames))?;
        println!("wrote {path}");
        return Ok(());
    }
    if env::args().nth(2).as_deref() == Some("hits") {
        hits(&mut stage);
        fs::write(&path, wav(&stage.frames))?;
        println!("wrote {path}");
        return Ok(());
    }
    if env::args().nth(2).as_deref() == Some("clock") {
        countdown(&mut stage);
        fs::write(&path, wav(&stage.frames))?;
        println!("wrote {path}");
        return Ok(());
    }
    for (slot, position) in ISLANDS.into_iter().enumerate() {
        stage.cues.send(Cue::Shore {
            slot,
            position,
            radius: 26.,
        });
    }

    // The islands have their surf and their birds, and the nearest has a yard at work.
    let island = |position| Place {
        haunt: Haunt::Island,
        position,
        radius: 26.,
    };
    let at_work = Place {
        haunt: Haunt::Yard,
        position: ISLANDS[0] + Vec3::new(5., 1., 4.),
        radius: 0.,
    };
    let islands = ISLANDS.map(island).to_vec();
    stage.places = [islands.clone(), vec![at_work]].concat();

    stage.play(
        "an upgrade chosen, twice: two rising notes",
        3.,
        false,
        &[
            (0.3, Event::Sound(Sound::Upgrade, BRIDGE)),
            (1.6, Event::Sound(Sound::Upgrade, BRIDGE)),
        ],
    );
    stage.play(
        "a yard point won from cargo sunk, twice: a handbell struck once",
        5.,
        false,
        &[
            (0.3, Event::Sound(Sound::Chime, BRIDGE)),
            (2.6, Event::Sound(Sound::Chime, BRIDGE)),
        ],
    );
    ring(&mut stage);
    stage.play(
        "the klaxon, three times: one more module lost would sink her",
        8.,
        false,
        &[
            (0.3, Event::Sound(Sound::Klaxon, BRIDGE)),
            (2.5, Event::Sound(Sound::Klaxon, BRIDGE)),
            (4.7, Event::Sound(Sound::Klaxon, BRIDGE)),
        ],
    );
    // The torpedo runs 340 m at 38 m/s. The bell rings when it is 115 m off.
    stage.play(
        "a torpedo boat fires from 340 m on the port bow, the alarm bell rings as its track closes, and it strikes her amidships",
        13.,
        false,
        &[
            (0.3, Event::Sound(Sound::TorpedoLaunch, Vec3::new(-330., 1.5, -80.))),
            (6.2, Event::Sound(Sound::TorpedoAlarm, BRIDGE)),
            (9.3, Event::Sound(Sound::Torpedoed, Vec3::new(-2.8, 0., 1.))),
        ],
    );
    stage.play(
        "lying stopped, then full ahead is rung and she gathers way",
        14.,
        true,
        &[(3., Event::Order(1.))],
    );
    damage(&mut stage);
    // A glacier stands 250 m off the port beam. Left to itself it calves every two
    // minutes or so, and here it is made to once.
    let glacier = Place {
        haunt: Haunt::Glacier,
        position: Vec3::new(-370., 5., -40.),
        radius: 120.,
    };
    let berg = Place {
        haunt: Haunt::Berg,
        position: Vec3::new(70., 4., 60.),
        radius: 14.,
    };
    // An islet close on the starboard bow, its shore 110 m off.
    let islet = island(Vec3::new(90., 5., -100.));
    stage.places.extend([glacier, berg, islet]);
    // The yard's spotter circles 45 m up, passing 60 m from the ship every 42 s.
    let circuit = Course::Round(Vec3::new(-150., 45., -150.), 150., 0.15);
    stage.play(
        "stop is rung and she lies off an islet, a glacier, an iceberg and a yard at work, with a floatplane circling: surf, kittiwakes, riveting, ice splitting, and at 12 s the glacier calves",
        40.,
        true,
        &[
            (0., Event::Order(0.)),
            (0., Event::Plane(Some(circuit))),
            (12., Event::Sound(Sound::Calving, Vec3::new(-252., 6., -30.))),
            (38., Event::Plane(None)),
        ],
    );
    stage.places = [islands.clone(), vec![at_work]].concat();

    // The bow mount is 16 m ahead of the bridge and the stern mount 20 m astern.
    let (bow, stern) = (Vec3::new(1., 5., -16.), Vec3::new(-1., 5., 20.));
    let main = |from, to| Event::Round(Sound::MainGun, from, to, 360., 1., Sound::Splash);
    stage.play(
        "full ahead again: the main battery fires four rounds, and the islands answer",
        16.,
        true,
        &[
            (0., Event::Order(1.)),
            (1., main(bow, Vec3::new(60., 0., -300.))),
            (2.6, main(stern, Vec3::new(80., 0., -290.))),
            (4.2, main(bow, Vec3::new(40., 0., -310.))),
            (8., main(stern, Vec3::new(-200., 0., 80.))),
        ],
    );

    let raider = Vec3::new(-150., 4., -120.);
    let shore = Vec3::new(120., 6.8, -85.);
    let splash = Sound::Splash;
    let from_raider = |to| Event::Round(Sound::RaiderGun, raider, to, 190., 0.7, splash);
    let from_shore = |to| Event::Round(Sound::ShoreGun, shore, to, 270., 1.2, splash);
    let close = |to| {
        let mount = Vec3::new(-4., 5., 2.);
        Event::Round(Sound::SecondaryGun, mount, to, 240., 0.4, splash)
    };
    stage.play(
        "under fire: a raider to port and a shore gun to starboard, rounds passing close",
        20.,
        true,
        &[
            (1., from_raider(Vec3::new(25., 0., 12.))),
            (3.5, from_raider(Vec3::new(-20., 0., -14.))),
            (5., from_shore(Vec3::new(-60., 0., 30.))),
            (7.5, from_raider(Vec3::new(8., 0., 40.))),
            (9., close(Vec3::new(-50., 0., -20.))),
            (9.4, close(Vec3::new(-52., 0., -24.))),
            (9.8, close(Vec3::new(-55., 0., -22.))),
            (10.5, from_shore(Vec3::new(-30., 0., -45.))),
            (12., Event::Sound(Sound::Burst, Vec3::new(-150., 4., -120.))),
            (
                12.6,
                Event::Sound(Sound::Wreck, Vec3::new(-150., 3., -120.)),
            ),
        ],
    );
    // A heavy battery 1,100 m off the starboard bow walks four salvos of two in on her:
    // well over, short, over, then a straddle. Each lob is slower than sound, so its
    // report reaches the bridge first and then its rush comes down out of the sky.
    let battery = Vec3::new(780., 8.2, -780.);
    let salvo = |at: f32, beyond: f32| {
        let line = (BRIDGE - battery).with_y(0.).normalize();
        let across = Vec3::new(line.z, 0., -line.x);
        [-8., 8.].map(|pair| {
            let land = (line * (beyond + pair) + across * pair * 0.3).with_y(0.);
            let lob = Event::Lob(
                Sound::HeavyGun,
                battery,
                land,
                140.,
                2.6,
                Sound::Column,
                16.,
            );
            (at, lob)
        })
    };
    let ranging: Vec<(f32, Event)> = [(0.5, 150.), (9.5, -75.), (18.5, 37.), (27.5, 4.)]
        .into_iter()
        .flat_map(|(at, beyond)| salvo(at, beyond))
        .collect();
    stage.play(
        "a heavy battery 1.1 km off ranging on her: its report, then each pair's rush and columns, well over, short, over, then a straddle",
        42.,
        true,
        &ranging,
    );

    // The nearest island, 216 m off the port bow, carries a factory and a gantry.
    let works = ISLANDS[0] + Vec3::new(-10., -1.8, 6.);
    let yard = ISLANDS[0] + Vec3::new(5., -1.8, 4.);
    let at_base = |target, lands| Event::Round(Sound::MainGun, bow, target, 360., 1., lands);
    // A yard downs tools while its siren is going.
    stage.places = islands;
    stage.play(
        "shelling a base with its siren going: a factory crumbles at the third hit, then the gantry falls, the base is cheered and the siren dies away in four seconds",
        30.,
        true,
        &[
            (0., Event::Alarm(Rotor::Shore, yard + Vec3::Y * 10., true)),
            (1., at_base(works, Sound::Burst)),
            (2.6, at_base(works, Sound::Burst)),
            (4.2, at_base(works, Sound::Collapse)),
            (9., at_base(yard, Sound::Burst)),
            (10.6, at_base(yard, Sound::Burst)),
            (12.2, at_base(yard, Sound::Ruin)),
        ],
    );
    stage.cues.send(Cue::Moored);
    stage.play("engines stopped: the sea alone", 4., false, &[]);
    let siren = Vec3::new(-90., 14., -120.);
    stage.play(
        "a base's siren alone, 150 m off the port bow: raised, three wails, then lowered",
        34.,
        false,
        &[
            (0., Event::Alarm(Rotor::Shore, siren, true)),
            (18., Event::Alarm(Rotor::Shore, siren, false)),
        ],
    );
    let funnel = Vec3::new(200., 16., -220.);
    stage.play(
        "a flagship's siren alone, 300 m off the starboard bow: raised, three wails, then lowered",
        36.,
        false,
        &[
            (0., Event::Alarm(Rotor::Ship, funnel, true)),
            (20., Event::Alarm(Rotor::Ship, funnel, false)),
        ],
    );
    horns(&mut stage);

    let pass = Course::Straight(Vec3::new(-600., 45., -30.), Vec3::new(30., 0., 0.));
    stage.play(
        "a floatplane alone: in from 600 m on the port beam at 30 m/s and 45 m up, past the bridge at 20 s, and away",
        40.,
        false,
        &[(0., Event::Plane(Some(pass)))],
    );

    // A second mixer that has never been told of a ship, so each sound plays alone.
    let Stage { mut frames, .. } = stage;
    let mut stage = Stage::new();
    println!(
        "{:>5.0} s  every sound alone, at {RANGES:?} m, four seconds each and sixteen for the glacier",
        { frames.len() as f32 / SAMPLE_RATE as f32 }
    );
    for sound in Sound::ALL {
        for (n, range) in RANGES.into_iter().enumerate() {
            let side = if n % 2 == 0 { 1. } else { -1. };
            let position = Vec3::new(side * 0.6, 0., -0.8).normalize() * range + Vec3::Y * 3.;
            let at = stage.frames.len();
            stage.emit(sound, position);
            let secs = if sound == Sound::Calving { 16 } else { 4 };
            stage
                .frames
                .resize(at + secs * SAMPLE_RATE as usize, [0.; 2]);
            stage.mixer.render(&mut stage.frames[at..]);
            let heard = &stage.frames[at..];
            let peak = heard.iter().flatten().fold(0_f32, |p, x| p.max(x.abs()));
            let wait = heard
                .iter()
                .position(|f| f[0].abs().max(f[1].abs()) > peak * 0.1);
            println!(
                "         {sound:?} at {range} m: heard after {:.2} s, peak {:.1} dB",
                wait.unwrap_or(0) as f32 / SAMPLE_RATE as f32,
                20. * peak.log10(),
            );
        }
    }
    frames.append(&mut stage.frames);
    fs::write(&path, wav(&frames))?;
    println!("wrote {path}");
    Ok(())
}

/// 16-bit stereo PCM.
fn wav(frames: &[[f32; 2]]) -> Vec<u8> {
    let data = frames.len() as u32 * 4;
    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend(b"RIFF");
    out.extend((36 + data).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16_u32.to_le_bytes());
    out.extend(1_u16.to_le_bytes());
    out.extend(2_u16.to_le_bytes());
    out.extend(SAMPLE_RATE.to_le_bytes());
    out.extend((SAMPLE_RATE * 4).to_le_bytes());
    out.extend(4_u16.to_le_bytes());
    out.extend(16_u16.to_le_bytes());
    out.extend(b"data");
    out.extend(data.to_le_bytes());
    for sample in frames.iter().flatten() {
        out.extend(((sample.clamp(-1., 1.) * 32767.) as i16).to_le_bytes());
    }
    out
}
