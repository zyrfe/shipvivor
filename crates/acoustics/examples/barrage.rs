//! Drives the mixer as hard as a heavily gunned ship would, and says whether it keeps
//! up. A capital ship's main battery of eighteen quick-firing guns, eight secondaries
//! and four anti-aircraft mounts fire as fast as they load among islands that answer,
//! while a raider's shells come in over her bridge and rounds strike her.
//!
//! It prints how long each second of sound took to render, how full the voices and
//! shell flights ran, how many of the raider's shells were followed in flight, and how
//! much of each blow on her was still heard. It writes what the bridge heard to a WAV.
//!
//! `cargo run --release -p shipvivor-acoustics --example barrage -- barrage.wav`
use glam::Vec3;
use shipvivor_acoustics::{Air, Bank, Cue, Harm, Hit, Mixer, SAMPLE_RATE, Sound, console};
use std::{env, fs, io, time::Instant};

const TICK: f32 = 1. / 64.;
const SECS: f32 = 24.;
const BRIDGE: Vec3 = Vec3::new(0., 7.5, 0.);

/// A gun aboard her: where it stands, the seconds it takes to load, the sound of its
/// report and what she feels of it, and its round's speed, weight and range.
struct Gun {
    at: Vec3,
    reload: f32,
    report: Sound,
    felt: Option<Sound>,
    speed: f32,
    weight: f32,
    range: f32,
    /// The bearing it fires on, and seconds until it is loaded.
    bearing: f32,
    loaded: f32,
}

/// A round in the air: serial, where it left, where it lands, seconds flown and to fly,
/// weight, the sound it makes where it lands, and whether it is the raider's.
struct Round {
    serial: u64,
    from: Vec3,
    to: Vec3,
    flown: f32,
    whole: f32,
    weight: f32,
    lands: Sound,
    hostile: bool,
    /// The raider's round was followed in flight as it came over her.
    heard: bool,
}

fn battery() -> Vec<Gun> {
    let mut guns = Vec::new();
    // Six triple turrets along 60 m of her, firing a little either side of the beam.
    for turret in 0..6 {
        for barrel in 0..3 {
            let along = -28. + 11. * turret as f32;
            guns.push(Gun {
                at: Vec3::new(0.6 * (barrel as f32 - 1.), 5., along),
                reload: 1.5,
                report: Sound::MainGun,
                felt: Some(Sound::Recoil),
                speed: 360.,
                weight: 1.,
                range: 650.,
                bearing: 1.2 + 0.05 * (turret * 3 + barrel) as f32,
                loaded: 0.083 * (turret * 3 + barrel) as f32,
            });
        }
    }
    for side in [-1., 1.] {
        for n in 0..4 {
            guns.push(Gun {
                at: Vec3::new(4. * side, 4., -12. + 8. * n as f32),
                reload: 1.,
                report: Sound::SecondaryGun,
                felt: Some(Sound::Kick),
                speed: 240.,
                weight: 0.4,
                range: 400.,
                bearing: if side > 0. { 1.5 } else { -1.5 } + 0.1 * n as f32,
                loaded: 0.13 * n as f32 + if side > 0. { 0. } else { 0.5 },
            });
        }
    }
    // Light anti-aircraft guns firing up at a raid four rounds a second.
    for n in 0..4 {
        guns.push(Gun {
            at: Vec3::new(if n % 2 == 0 { 3. } else { -3. }, 6., -4. + 4. * n as f32),
            reload: 0.25,
            report: Sound::SecondaryGun,
            felt: None,
            speed: 300.,
            weight: 0.25,
            range: 300.,
            bearing: 0.4 * n as f32,
            loaded: 0.06 * n as f32,
        });
    }
    guns
}

/// The blows on her, each a second into the scene, and where on her it lands.
fn blows() -> Vec<(f32, &'static str, Hit, Vec3)> {
    let round = |calibre: f32, plate: f32, dealt: f32, harm| Hit {
        calibre: Some(calibre),
        plate,
        turned: calibre <= plate,
        dealt: if calibre <= plate { 0. } else { dealt },
        harm,
    };
    vec![
        (
            6.,
            "a clank, 8-inch on 10",
            round(8., 10., 0., Harm::None),
            Vec3::new(2.4, 2.2, 4.),
        ),
        (
            11.,
            "an 8-inch through, 6 dealt",
            round(8., 1., 6., Harm::None),
            Vec3::new(-2.4, 2.2, -8.),
        ),
        (
            16.,
            "a section wrecked",
            round(12., 1., 16., Harm::Wrecked),
            Vec3::new(2.4, 2.2, 10.),
        ),
    ]
}

/// Render the scene, with her own guns firing or not and the blows on her or not.
fn scene(firing: bool, struck: bool, report: bool) -> Vec<[f32; 2]> {
    let (cues, mut mixer) = console(Bank::standard(), Air::SCALED);
    cues.send(Cue::Ears {
        position: BRIDGE,
        right: Vec3::X,
    });
    // Islands all round, each throwing her reports back.
    for slot in 0..8 {
        let bearing = slot as f32 * 0.785 + 0.3;
        let range = 220. + 60. * slot as f32;
        cues.send(Cue::Shore {
            slot,
            position: Vec3::new(bearing.sin(), 0., -bearing.cos()) * range + Vec3::Y * 5.,
            radius: 30.,
        });
    }
    let mut guns = if firing { battery() } else { Vec::new() };
    let mut aloft: Vec<Round> = Vec::new();
    let mut serial = 0_u64;
    let mut frames = Vec::new();
    let blows = if struck { blows() } else { Vec::new() };
    let (mut raider, mut followed, mut sent) = (0.5_f32, 0_usize, 0_usize);
    let (mut busy, mut slow, mut times) = (0_f32, 0_usize, Vec::new());
    let (mut full, mut flying_full, mut ticks) = (0_usize, 0_usize, 0_usize);
    let mut second = Instant::now();
    for step in 0..(SECS / TICK) as usize {
        cues.send(Cue::Way {
            speed: 9.,
            throttle: 0.7,
        });
        for gun in &mut guns {
            gun.loaded -= TICK;
            if gun.loaded > 0. {
                continue;
            }
            gun.loaded += gun.reload;
            serial += 1;
            cues.send(Cue::Emit {
                sound: gun.report,
                position: gun.at,
                serial,
            });
            if let Some(felt) = gun.felt {
                cues.send(Cue::Emit {
                    sound: felt,
                    position: BRIDGE,
                    serial,
                });
            }
            let (sin, cos) = gun.bearing.sin_cos();
            let to = gun.at.with_y(0.) + Vec3::new(sin, 0., -cos) * gun.range;
            aloft.push(Round {
                serial,
                from: gun.at,
                to,
                flown: 0.,
                whole: gun.range / gun.speed,
                weight: gun.weight,
                lands: if gun.weight < 0.3 {
                    Sound::Burst
                } else {
                    Sound::Splash
                },
                hostile: false,
                heard: false,
            });
        }
        // The raider, 700 m on the port bow, fires every two seconds, high over her.
        raider -= TICK;
        if raider <= 0. {
            raider += 2.;
            serial += 1;
            let from = Vec3::new(-500., 4., -500.);
            cues.send(Cue::Emit {
                sound: Sound::RaiderGun,
                position: from,
                serial,
            });
            aloft.push(Round {
                serial,
                from,
                to: Vec3::new(60., 0., 50.),
                flown: 0.,
                whole: 3.,
                weight: 0.7,
                lands: Sound::Splash,
                hostile: true,
                heard: false,
            });
        }
        // The game reports every shell in the air each tick, oldest first.
        let mut landed = Vec::new();
        aloft.retain_mut(|round| {
            round.flown += TICK;
            let along = round.flown / round.whole;
            if round.hostile && along > 0.6 && mixer.following(round.serial) {
                round.heard = true;
            }
            if along >= 1. {
                if round.hostile {
                    (sent, followed) = (sent + 1, followed + usize::from(round.heard));
                }
                landed.push((round.to, round.lands, round.serial));
                return false;
            }
            let lift = 0.05 * round.from.distance(round.to);
            let level = (round.to - round.from) / round.whole;
            cues.send(Cue::Shell {
                serial: round.serial,
                position: round.from.lerp(round.to, along)
                    + Vec3::Y * lift * 4. * along * (1. - along),
                velocity: level + Vec3::Y * lift * 4. * (1. - 2. * along) / round.whole,
                weight: round.weight,
            });
            true
        });
        for (at, sound, serial) in landed {
            cues.send(Cue::Emit {
                sound,
                position: at,
                serial,
            });
        }
        for &(at, name, hit, position) in &blows {
            if step == (at / TICK) as usize {
                if report {
                    println!("{at:>6.1} s  {name}");
                }
                cues.send(Cue::Hit {
                    position,
                    serial: step as u64,
                    hit,
                });
            }
        }
        let start = frames.len();
        frames.resize(start + (SAMPLE_RATE as f32 * TICK) as usize, [0.; 2]);
        let took = Instant::now();
        mixer.render(&mut frames[start..]);
        let took = took.elapsed().as_secs_f32();
        busy += took;
        times.push(took / TICK);
        ticks += 1;
        full += usize::from(mixer.sounding() == 64);
        flying_full += usize::from(mixer.flying() == 32);
        if (step + 1) % 64 == 0 {
            slow += usize::from(second.elapsed().as_secs_f32() > 1.);
            second = Instant::now();
        }
    }
    if report {
        times.sort_by(f32::total_cmp);
        let late = times[times.len() * 99 / 100];
        println!(
            "rendering took {:.1}% of real time on average, {:.0}% in one tick of a hundred; {} seconds over 1 s",
            100. * busy / SECS,
            100. * late,
            slow
        );
        println!(
            "voices full {:.0}% of the time, shell flights full {:.0}%",
            100. * full as f32 / ticks as f32,
            100. * flying_full as f32 / ticks as f32
        );
        println!("the raider's shells followed as they came over her: {followed} of {sent}");
    }
    let _: &Mixer = &mixer;
    frames
}

/// Power between two seconds of a recording, in decibels.
fn level(frames: &[[f32; 2]], from: f32, to: f32) -> f32 {
    let span = &frames[(from * SAMPLE_RATE as f32) as usize..(to * SAMPLE_RATE as f32) as usize];
    let power: f32 = span.iter().flatten().map(|x| x * x).sum();
    10. * (power / (span.len() * 2) as f32).max(1e-12).log10()
}

fn main() -> io::Result<()> {
    let path = env::args().nth(1).unwrap_or_else(|| "barrage.wav".into());
    let heard = scene(true, true, true);
    let unstruck = scene(true, false, false);
    let quiet = scene(false, true, false);
    // A blow is heard through the barrage by how much louder the scene is with it.
    println!(
        "{:>28}  {:>10}  {:>10}  {:>10}",
        "", "in quiet", "in barrage", "barrage"
    );
    for (at, name, ..) in blows() {
        for (span, (from, to)) in [("first 0.3 s", (0.05, 0.35)), ("0.5 to 2.5 s", (0.5, 2.5))] {
            let (a, b) = (at + from, at + to);
            println!(
                "{:>28}  {:>8.1} dB  {:>8.1} dB  {:>8.1} dB",
                format!("{name}, {span}"),
                level(&quiet, a, b),
                level(&heard, a, b),
                level(&unstruck, a, b),
            );
        }
    }
    fs::write(&path, wav(&heard))?;
    let without = path.replace(".wav", "-unstruck.wav");
    fs::write(&without, wav(&unstruck))?;
    println!("wrote {path} and, without the blows, {without}");
    Ok(())
}

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
    for frame in frames {
        for x in frame {
            out.extend(((x.clamp(-1., 1.) * 32767.) as i16).to_le_bytes());
        }
    }
    out
}
