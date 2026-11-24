//! Scratch: renders the ship's own noise in steady states for measurement.
use glam::Vec3;
use shipvivor_acoustics::{Air, Bank, Cue, Sound, console};
use std::{env, fs};

fn main() {
    let dir = env::args().nth(1).expect("a directory");
    let states: [(&str, Option<(f32, f32)>); 8] = [
        ("rough-idle", Some((0., 0.))),
        ("rough-full", Some((0., 1.))),
        ("moored", None),
        ("idle", Some((0., 0.))),
        ("half", Some((0., 0.5))),
        ("full", Some((0., 1.))),
        ("way-half", Some((6.8, 0.5))),
        ("way-full", Some((13.6, 1.))),
    ];
    for (name, way) in states {
        let (cues, mut mixer) = console(Bank::standard(), Air::SCALED);
        cues.send(Cue::Ears {
            position: Vec3::new(0., 7.5, 0.),
            right: Vec3::X,
        });
        match way {
            Some((speed, throttle)) => cues.send(Cue::Way { speed, throttle }),
            None => cues.send(Cue::Moored),
        }
        if name.starts_with("rough") {
            cues.send(Cue::Engines {
                damaged: 1.,
                stopped: 0.,
            });
        }
        let mut frames = vec![[0.; 2]; 48_000 * 40];
        mixer.render(&mut frames);
        let bytes: Vec<u8> = frames[48_000 * 12..]
            .iter()
            .flatten()
            .flat_map(|x: &f32| x.to_le_bytes())
            .collect();
        fs::write(format!("{dir}/{name}.st.f32"), bytes).unwrap();
    }
    // Each ambient sound alone, in silence, three takes in a row.
    let sounds = [
        (Sound::Breaker, [20., 100., 250.]),
        (Sound::IceCrack, [20., 200., 500.]),
        (Sound::Calving, [20., 400., 1000.]),
        (Sound::Gull, [20., 100., 250.]),
        (Sound::Rivets, [20., 200., 450.]),
        (Sound::Creak, [0., 0., 0.]),
        (Sound::Telegraph, [0., 0., 0.]),
    ];
    for (sound, ranges) in sounds {
        for range in ranges {
            let (cues, mut mixer) = console(Bank::standard(), Air::SCALED);
            cues.send(Cue::Ears {
                position: Vec3::new(0., 7.5, 0.),
                right: Vec3::X,
            });
            let mut frames = vec![[0.; 2]; 48_000 * 60];
            for (take, block) in frames.chunks_mut(48_000 * 20).enumerate() {
                cues.send(Cue::Emit {
                    sound,
                    position: Vec3::new(0.3 * range, 3., -0.95 * range),
                    serial: take as u64,
                });
                mixer.render(block);
            }
            let bytes: Vec<u8> = frames
                .iter()
                .flatten()
                .flat_map(|x: &f32| x.to_le_bytes())
                .collect();
            fs::write(format!("{dir}/{sound:?}-{range}.st.f32"), bytes).unwrap();
        }
    }
}
