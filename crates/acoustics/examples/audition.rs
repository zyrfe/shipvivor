//! Renders what the bridge hears to a WAV file, through the game's air. Seven scenes:
//! getting under way, firing the main battery among islands, coming under fire,
//! shelling a base with its siren going until it falls, a base's siren and a
//! flagship's alone, and every sound in the bank alone at three ranges. The times of
//! each are printed.
//!
//! `cargo run -p shipvivor-acoustics --example audition -- tour.wav`
use glam::Vec3;
use shipvivor_acoustics::{Air, Bank, Cue, Cues, Mixer, Rotor, SAMPLE_RATE, Sound, console};
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
    Order(f32),
    /// A siren of this make raised or lowered at a place.
    Alarm(Rotor, Vec3, bool),
}

struct Stage {
    cues: Cues,
    mixer: Mixer,
    frames: Vec<[f32; 2]>,
    /// Rounds in the air: serial, from, to, speed, weight, seconds flown, and the
    /// sound each will make where it lands.
    rounds: Vec<(u64, Vec3, Vec3, f32, f32, f32, Sound)>,
    serial: u64,
    speed: f32,
    throttle: f32,
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
                    Event::Order(throttle) => self.throttle = throttle,
                    Event::Alarm(rotor, position, raised) => self.cues.send(Cue::Alarm {
                        slot: usize::from(rotor == Rotor::Ship),
                        rotor,
                        position,
                        raised,
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
                            .push((self.serial, from, to, speed, weight, 0., lands));
                    }
                }
            }
            if ship {
                self.speed = (self.speed + 1.35 * TICK).min(13.6 * self.throttle);
                self.cues.send(Cue::Way {
                    speed: self.speed,
                    throttle: self.throttle,
                });
            }
            // Each round flies a shallow arc and throws up a column where it lands.
            let mut landed = Vec::new();
            self.rounds
                .retain_mut(|(serial, from, to, speed, weight, flown, lands)| {
                    *flown += TICK;
                    let whole = from.distance(*to) / *speed;
                    let along = *flown / whole;
                    if along >= 1. {
                        landed.push((*to, *lands));
                        return false;
                    }
                    let lift = 0.04 * from.distance(*to);
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
                // its siren has lost its power.
                if sound == Sound::Ruin {
                    self.emit(Sound::Fanfare, BRIDGE);
                    self.cues.send(Cue::Alarm {
                        slot: 0,
                        rotor: Rotor::Shore,
                        position: point + Vec3::Y * 10.,
                        raised: false,
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

fn main() -> io::Result<()> {
    let path = env::args().nth(1).unwrap_or_else(|| "audition.wav".into());
    let mut stage = Stage::new();
    for (slot, position) in ISLANDS.into_iter().enumerate() {
        stage.cues.send(Cue::Shore {
            slot,
            position,
            radius: 26.,
        });
    }

    stage.play(
        "lying stopped, then full ahead is rung and she gathers way",
        14.,
        true,
        &[(3., Event::Order(1.))],
    );

    // The bow mount is 16 m ahead of the bridge and the stern mount 20 m astern.
    let (bow, stern) = (Vec3::new(1., 5., -16.), Vec3::new(-1., 5., 20.));
    let main = |from, to| Event::Round(Sound::MainGun, from, to, 360., 1., Sound::Splash);
    stage.play(
        "the main battery fires four rounds, and the islands answer",
        16.,
        true,
        &[
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
    // The nearest island, 216 m off the port bow, carries a factory and a gantry.
    let works = ISLANDS[0] + Vec3::new(-10., -1.8, 6.);
    let yard = ISLANDS[0] + Vec3::new(5., -1.8, 4.);
    let at_base = |target, lands| Event::Round(Sound::MainGun, bow, target, 360., 1., lands);
    stage.play(
        "shelling a base with its siren going: a factory crumbles at the third hit, then the gantry falls, the base is cheered and the siren runs down",
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

    // A second mixer that has never been told of a ship, so each sound plays alone.
    let Stage { mut frames, .. } = stage;
    let mut stage = Stage::new();
    println!(
        "{:>5.0} s  every sound alone, at {RANGES:?} m, four seconds each",
        { frames.len() as f32 / SAMPLE_RATE as f32 }
    );
    for sound in Sound::ALL {
        for (n, range) in RANGES.into_iter().enumerate() {
            let side = if n % 2 == 0 { 1. } else { -1. };
            let position = Vec3::new(side * 0.6, 0., -0.8).normalize() * range + Vec3::Y * 3.;
            let at = stage.frames.len();
            stage.emit(sound, position);
            stage.frames.resize(at + 4 * SAMPLE_RATE as usize, [0.; 2]);
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
