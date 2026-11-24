use super::*;

const EARS: Cue = Cue::Ears {
    position: Vec3::ZERO,
    right: Vec3::X,
};

fn desk(air: Air) -> (Cues, Mixer) {
    let (cues, mixer) = console(Bank::standard(), air);
    cues.send(EARS);
    (cues, mixer)
}
fn emit(cues: &Cues, sound: Sound, position: Vec3) {
    cues.send(Cue::Emit {
        sound,
        position,
        serial: 0,
    });
}
fn listen(mixer: &mut Mixer, secs: f32) -> Vec<[f32; 2]> {
    let mut out = vec![[0.; 2]; (secs * RATE) as usize];
    mixer.render(&mut out);
    out
}
fn peak(heard: &[[f32; 2]]) -> f32 {
    heard.iter().flatten().fold(0., |peak, x| peak.max(x.abs()))
}
/// Seconds until the sound first rises to a tenth of its peak.
fn onset(heard: &[[f32; 2]]) -> f32 {
    let floor = peak(heard) * 0.1;
    let first = heard
        .iter()
        .position(|f| f[0].abs().max(f[1].abs()) > floor);
    first.expect("something was heard") as f32 / RATE
}
/// The first moments of a sound, before the sea and the ears have answered it.
fn opening(heard: &[[f32; 2]], secs: f32) -> &[[f32; 2]] {
    let start = (onset(heard) * RATE) as usize;
    &heard[start..start + (secs * RATE) as usize]
}
/// Share of the energy that lies in the sample-to-sample movement: higher is brighter.
fn brightness(heard: &[[f32; 2]]) -> f32 {
    let energy: f32 = heard.iter().map(|f| f[0] * f[0]).sum();
    let movement: f32 = heard.windows(2).map(|w| (w[1][0] - w[0][0]).powi(2)).sum();
    movement / energy
}

#[test]
fn the_bank_is_full_scale_and_every_layer_ends_in_silence() {
    let bank = Bank::standard();
    for sound in Sound::ALL {
        for take in 0..foundry::VARIATIONS as u64 {
            for layer in &bank.cast(Bank::index(sound, take)).layers {
                let peak = layer.iter().fold(0_f32, |peak, x| peak.max(x.abs()));
                // A bird has no rumble and a bell no weight. Such a layer is silent.
                let full = (peak - 1.).abs() < 1e-3;
                assert!(full || peak == 0., "{sound:?} peaks at {peak}");
                assert!(layer.last().unwrap().abs() < 1e-3);
                assert!(layer.iter().all(|x| x.is_finite()));
            }
        }
    }
}

#[test]
fn a_report_arrives_when_the_sound_has_crossed_the_water() {
    let (cues, mut mixer) = desk(Air::default());
    emit(&cues, Sound::MainGun, Vec3::new(0., 0., -343.));
    let heard = listen(&mut mixer, 2.);
    assert!((onset(&heard) - 1.).abs() < 0.004, "{}", onset(&heard));
    assert_eq!(peak(&heard[..(0.99 * RATE) as usize]), 0.);

    // The game's slower air doubles the wait between flash and report.
    let (cues, mut mixer) = desk(Air::SCALED);
    emit(&cues, Sound::ShoreGun, Vec3::new(145., 6.8, 0.));
    let heard = listen(&mut mixer, 2.);
    assert!((onset(&heard) - 145.2 / 170.).abs() < 0.004);
}

#[test]
fn distance_quiets_a_gun_and_takes_its_crack_first() {
    let hear = |range: f32| {
        let (cues, mut mixer) = desk(Air::default());
        emit(&cues, Sound::MainGun, Vec3::new(0., 0., -range));
        listen(&mut mixer, 6.)
    };
    let (near, far) = (hear(15.), hear(450.));
    assert!(peak(&far) < peak(&near) * 0.1);
    assert!(peak(&far) > 0.005);
    let (crack, dulled) = (opening(&near, 0.025), opening(&far, 0.025));
    assert!(brightness(dulled) < brightness(crack) * 0.5);
    // Open air halves the body with each doubling of range. Rumble falls more slowly.
    let air = Air::default();
    assert!((air.gains(60.)[1] / air.gains(30.)[1] - 0.5).abs() < 1e-6);
    assert!(air.gains(480.)[2] > air.gains(480.)[1] * 2.);
    assert!(air.gains(480.)[0] < air.gains(480.)[1] * 0.5);
    assert!(air.ceiling(100.) > 4000. && air.ceiling(480.) < 2500.);
}

#[test]
fn a_gun_to_starboard_is_louder_in_the_right_ear() {
    let side = |position: Vec3| {
        let (cues, mut mixer) = desk(Air::default());
        emit(&cues, Sound::RaiderGun, position);
        let heard = listen(&mut mixer, 1.5);
        let ear = |i: usize| heard.iter().fold(0_f32, |peak, f| peak.max(f[i].abs()));
        (ear(0), ear(1))
    };
    let (left, right) = side(Vec3::new(80., 0., 0.));
    assert!(right > left * 3. && left > 0.);
    let (left, right) = side(Vec3::new(-80., 0., 0.));
    assert!(left > right * 3.);
    // Dead ahead the gun is the same in both ears. Only the sea's tail differs.
    let (left, right) = side(Vec3::new(0., 0., -80.));
    assert!((left - right).abs() < left * 0.05);
}

#[test]
fn held_time_keeps_a_sound_hanging_in_the_air() {
    let (cues, mut mixer) = desk(Air::default());
    emit(&cues, Sound::MainGun, Vec3::new(0., 0., -343.));
    assert_eq!(peak(&listen(&mut mixer, 0.5)), 0.);
    cues.send(Cue::Flow(false));
    assert_eq!(peak(&listen(&mut mixer, 3.)), 0.);
    cues.send(Cue::Flow(true));
    // Half a second of the crossing remained. The gate cost a few milliseconds of it.
    let heard = listen(&mut mixer, 1.5);
    assert!((onset(&heard) - 0.5).abs() < 0.02, "{}", onset(&heard));
}

#[test]
fn a_hush_clears_the_air_and_moves_the_ears_at_once() {
    let (cues, mut mixer) = desk(Air::default());
    emit(&cues, Sound::Wreck, Vec3::new(30., 0., 0.));
    assert!(peak(&listen(&mut mixer, 0.5)) > 0.1);
    cues.send(Cue::Hush);
    assert_eq!(peak(&listen(&mut mixer, 1.)), 0.);
    // New ears a kilometre off hear a gun beside them without any glide from the old place.
    let there = Vec3::new(1000., 0., 0.);
    cues.send(Cue::Ears {
        position: there,
        right: Vec3::X,
    });
    emit(&cues, Sound::SecondaryGun, there + Vec3::Z * 17.);
    assert!((onset(&listen(&mut mixer, 1.)) - 17. / 343.).abs() < 0.004);
}

#[test]
fn a_crowded_fight_stays_inside_full_scale() {
    let (cues, mut mixer) = desk(Air::SCALED);
    let mut loudest = 0_f32;
    for wave in 0..40_u64 {
        for i in 0..5 {
            let serial = wave * 5 + i;
            let bearing = serial as f32 * 0.7;
            cues.send(Cue::Emit {
                sound: Sound::ALL[serial as usize % 7],
                position: Vec3::new(bearing.cos(), 0.1, bearing.sin()) * (10. + 3. * i as f32),
                serial,
            });
        }
        let heard = listen(&mut mixer, 0.05);
        assert!(heard.iter().flatten().all(|x| x.is_finite()));
        loudest = loudest.max(peak(&heard));
    }
    assert!(loudest > 0.75 && loudest <= 1., "{loudest}");
    // Once the first shock has passed, the pile-up is turned down and not flattened.
    let sustained = listen(&mut mixer, 0.1);
    assert!(
        peak(&sustained[2400..]) < 0.9,
        "{}",
        peak(&sustained[2400..])
    );
    assert_eq!(mixer.sounding(), 64);
    // Left alone, the air empties.
    listen(&mut mixer, 8.);
    assert_eq!(mixer.sounding(), 0);
}

#[test]
fn a_ship_closing_on_a_sound_hears_it_sooner() {
    let hear = |speed: f32| {
        let (cues, mut mixer) = desk(Air::default());
        emit(&cues, Sound::Burst, Vec3::new(0., 0., -343.));
        let mut heard = Vec::new();
        for tick in 0..90 {
            cues.send(Cue::Ears {
                position: Vec3::NEG_Z * speed * tick as f32 / 60.,
                right: Vec3::X,
            });
            heard.extend(listen(&mut mixer, 1. / 60.));
        }
        onset(&heard)
    };
    // Closing at 34.3 m/s shortens a one-second crossing by a tenth, less the ear lag.
    let (still, closing) = (hear(0.), hear(34.3));
    assert!((still - 1.).abs() < 0.004);
    assert!((closing - 1. / 1.1).abs() < 0.012, "{closing}");
}

fn level(heard: &[[f32; 2]]) -> f32 {
    let power: f32 = heard.iter().flatten().map(|x| x * x).sum();
    (power / (heard.len() * 2) as f32).sqrt()
}

#[test]
fn the_ship_is_never_silent_and_speed_makes_it_louder() {
    let (cues, mut mixer) = desk(Air::SCALED);
    // Until the game says where the ship stands there is nothing to hear of it.
    assert_eq!(peak(&listen(&mut mixer, 0.5)), 0.);
    let mut steady = |cue: Cue| {
        cues.send(cue);
        listen(&mut mixer, 6.);
        let heard = listen(&mut mixer, 3.);
        assert!(peak(&heard) < 0.9);
        level(&heard)
    };
    let stopped = steady(Cue::Way {
        speed: 0.,
        throttle: 0.,
    });
    let half = steady(Cue::Way {
        speed: 6.8,
        throttle: 0.5,
    });
    let full = steady(Cue::Way {
        speed: -13.6,
        throttle: -1.,
    });
    let moored = steady(Cue::Moored);
    assert!(stopped > 0.02);
    assert!(half > stopped * 1.5);
    assert!(full > half * 1.5);
    assert!(moored > 0.008 && moored < stopped * 0.6);
}

#[test]
fn a_gun_alongside_pushes_the_ships_own_noise_down() {
    let hear = |under_way: bool, gun: bool| {
        let (cues, mut mixer) = desk(Air::SCALED);
        if under_way {
            cues.send(Cue::Way {
                speed: 6.8,
                throttle: 0.5,
            });
        }
        listen(&mut mixer, 6.);
        if gun {
            emit(&cues, Sound::SecondaryGun, Vec3::new(0., 0., -15.));
        }
        listen(&mut mixer, 1.5)
    };
    let (bed, both, gun) = (hear(true, false), hear(true, true), hear(false, true));
    // Below the limiter the mix is a plain sum, so taking the gun away leaves the bed.
    assert!(peak(&both) < 0.6);
    let span = (0.2 * RATE) as usize..(0.5 * RATE) as usize;
    let under: Vec<[f32; 2]> = (span.clone())
        .map(|n| [both[n][0] - gun[n][0], both[n][1] - gun[n][1]])
        .collect();
    let (ducked, open) = (level(&under), level(&bed[span]));
    assert!(ducked < open * 0.75 && ducked > open * 0.2);
    // A second and a half on, the bed is most of the way back.
    let late = (1.3 * RATE) as usize..(1.5 * RATE) as usize;
    let back: Vec<[f32; 2]> = (late.clone())
        .map(|n| [both[n][0] - gun[n][0], both[n][1] - gun[n][1]])
        .collect();
    assert!(level(&back) > level(&bed[late]) * 0.8);
}

/// How much the first 150 ms repeat at any period from 4 to 25 ms. A struck drum or a
/// sung note scores high. A blast scores low.
fn pitch(layer: &[f32]) -> f32 {
    let window = &layer[..(0.15 * RATE) as usize];
    let energy: f32 = window.iter().map(|x| x * x).sum();
    let at = |lag: usize| -> f32 {
        let product: f32 = window[lag..].iter().zip(window).map(|(a, b)| a * b).sum();
        product / energy
    };
    ((0.004 * RATE) as usize..(0.025 * RATE) as usize)
        .filter(|&lag| at(lag) > at(lag - 8) && at(lag) > at(lag + 8))
        .map(at)
        .fold(0., f32::max)
}

#[test]
fn a_gun_is_a_blast_and_not_a_note() {
    let bank = Bank::standard();
    for sound in [
        Sound::MainGun,
        Sound::SecondaryGun,
        Sound::RaiderGun,
        Sound::ShoreGun,
    ] {
        for take in 0..foundry::VARIATIONS as u64 {
            let body = &bank.cast(Bank::index(sound, take)).layers[1];
            assert!(pitch(body) < 0.3, "{sound:?} take {take}: {}", pitch(body));
        }
    }
    // A falling tone of the kind a drum makes, for comparison.
    let mut phase = 0_f32;
    let drum: Vec<f32> = (0..(0.2 * RATE) as usize)
        .map(|n| {
            let t = n as f32 / RATE;
            phase += std::f32::consts::TAU * 62. * (1. + 0.9 * (-t / 0.035).exp()) / RATE;
            phase.sin() * (-t / 0.16).exp()
        })
        .collect();
    assert!(pitch(&drum) > 0.4, "{}", pitch(&drum));
}

/// Ears on the bridge, where the sea below can answer.
fn bridge(air: Air) -> (Cues, Mixer) {
    let (cues, mixer) = console(Bank::standard(), air);
    cues.send(Cue::Ears {
        position: Vec3::Y * 7.5,
        right: Vec3::X,
    });
    (cues, mixer)
}
fn apart(a: &[[f32; 2]], b: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let frames = a.iter().zip(b);
    frames.map(|(a, b)| [a[0] - b[0], a[1] - b[1]]).collect()
}
/// Seconds until a sound first rises above a fixed floor.
fn first(heard: &[[f32; 2]], ear: usize) -> f32 {
    let first = heard.iter().position(|f| f[ear].abs() > 1e-5);
    first.expect("something was heard") as f32 / RATE
}

#[test]
fn the_sea_sends_a_second_copy_by_the_longer_way_round() {
    // Two shocks the same distance off, one higher above the water than the other.
    // They reach the bridge alike until the sea's copy of the lower one comes in.
    let hear = |height: f32| {
        let (cues, mut mixer) = bridge(Air::SCALED);
        emit(&cues, Sound::Crack, Vec3::new(0., height, -16.));
        listen(&mut mixer, 0.3)
    };
    let (low, high) = (hear(5.), hear(10.));
    let direct = (16_f32 * 16. + 2.5 * 2.5).sqrt();
    let off_sea = (16_f32 * 16. + 12.5 * 12.5).sqrt();
    assert!((first(&low, 0) - direct / 170.).abs() < 0.001);
    let parted = first(&apart(&low, &high), 0);
    assert!((parted - off_sea / 170.).abs() < 0.001, "{parted}");
    // A sound made on the water itself has no longer way to come by.
    let (cues, mut mixer) = bridge(Air::SCALED);
    emit(&cues, Sound::Crack, Vec3::new(0., 0., -16.));
    let skimmed = listen(&mut mixer, 0.3);
    assert!(peak(&skimmed) > peak(&low));
}

#[test]
fn the_far_ear_hears_late_and_dull() {
    let (cues, mut mixer) = desk(Air::default());
    emit(&cues, Sound::Crack, Vec3::new(80., 0., 0.));
    let heard = listen(&mut mixer, 0.5);
    // Sound takes about two thirds of a millisecond to come round the head.
    let late = first(&heard, 0) - first(&heard, 1);
    assert!((0.0004..0.0007).contains(&late), "{late}");
    let ear = |i: usize| -> Vec<[f32; 2]> { heard.iter().map(|f| [f[i]; 2]).collect() };
    let (far, near) = (ear(0), ear(1));
    assert!(brightness(opening(&far, 0.02)) < brightness(opening(&near, 0.02)) * 0.7);
}

#[test]
fn a_sound_astern_is_duller_than_the_same_sound_ahead() {
    let hear = |z: f32| {
        let (cues, mut mixer) = desk(Air::default());
        emit(&cues, Sound::Crack, Vec3::new(0., 0., z));
        brightness(opening(&listen(&mut mixer, 0.5), 0.02))
    };
    assert!(hear(60.) < hear(-60.) * 0.8);
}

#[test]
fn the_open_sea_answers_with_a_tail_that_differs_in_each_ear() {
    let (cues, mut mixer) = bridge(Air::SCALED);
    emit(&cues, Sound::Crack, Vec3::new(0., 5., -60.));
    let heard = listen(&mut mixer, 9.);
    // The crack itself is over in a third of a second. What follows is the sea's.
    let start = (onset(&heard) * RATE) as usize;
    let span =
        |from: f32, to: f32| &heard[start + (from * RATE) as usize..][..(to * RATE) as usize];
    let (early, late) = (level(span(0.6, 0.4)), level(span(1.6, 0.4)));
    assert!(
        early > 1e-4 && late > 1e-5 && late < early * 0.5,
        "{early} {late}"
    );
    // Straight ahead the crack is the same in both ears, and its tail is not.
    let alike = |heard: &[[f32; 2]]| {
        let sum = |pick: fn(&[f32; 2]) -> f32| heard.iter().map(pick).sum::<f32>();
        sum(|f| f[0] * f[1]) / (sum(|f| f[0] * f[0]) * sum(|f| f[1] * f[1])).sqrt()
    };
    assert!(alike(span(0., 0.02)) > 0.99);
    assert!(alike(span(0.6, 1.)).abs() < 0.5, "{}", alike(span(0.6, 1.)));
    // It dies away entirely, and the mixer goes back to making nothing at all.
    assert_eq!(peak(&listen(&mut mixer, 1.)), 0.);
}

#[test]
fn land_throws_a_report_back_from_its_own_side() {
    let hear = |shore: bool| {
        let (cues, mut mixer) = bridge(Air::SCALED);
        if shore {
            cues.send(Cue::Shore {
                slot: 0,
                position: Vec3::new(-300., 5., 0.),
                radius: 26.,
            });
        }
        emit(&cues, Sound::MainGun, Vec3::new(0., 5., -16.));
        // A gun on the land itself is not answered by it.
        emit(&cues, Sound::ShoreGun, Vec3::new(-290., 6.8, 0.));
        let heard = listen(&mut mixer, 0.01);
        assert_eq!(mixer.sounding(), if shore { 3 } else { 2 });
        [heard, listen(&mut mixer, 8.)].concat()
    };
    let echo = apart(&hear(true), &hear(false));
    // Out to the shore that faces the bridge, and back from there.
    let face = Vec3::new(-274., 5., 0.);
    let out = face.distance(Vec3::new(0., 5., -16.));
    let back = face.distance(Vec3::Y * 7.5);
    let due = (out + back) / 170.;
    assert!((onset(&echo) - due).abs() < 0.02, "{} {due}", onset(&echo));
    let ear = |i: usize| echo.iter().fold(0_f32, |peak, f| peak.max(f[i].abs()));
    assert!(
        ear(0) > ear(1) * 2. && ear(0) > 0.005,
        "{} {}",
        ear(0),
        ear(1)
    );
}

#[test]
fn more_land_throws_back_more_and_an_emptied_slot_nothing() {
    let answer = |radius: f32| {
        let hear = |radius: f32| {
            let (cues, mut mixer) = bridge(Air::SCALED);
            // The near face is 274 m off whatever the size, so only the size differs.
            cues.send(Cue::Shore {
                slot: 0,
                position: Vec3::new(-274. - radius, 5., 0.),
                radius,
            });
            emit(&cues, Sound::MainGun, Vec3::new(0., 5., -16.));
            listen(&mut mixer, 8.)
        };
        level(&apart(&hear(radius), &hear(0.)))
    };
    let (berg, island, glacier) = (answer(13.), answer(26.), answer(120.));
    assert!(berg > 0. && island > 1.5 * berg && glacier > 1.2 * island);
    assert!(glacier < 2.5 * island);
}

#[test]
fn the_ship_carries_her_own_recoil_to_the_bridge_at_once() {
    let (cues, mut mixer) = bridge(Air::SCALED);
    emit(&cues, Sound::Recoil, Vec3::new(500., 0., 0.));
    let heard = listen(&mut mixer, 1.5);
    assert!(onset(&heard) < 0.003);
    assert!(peak(&heard) > 0.2);
    assert!(heard.iter().all(|f| f[0] == f[1]));
    // It is felt, not heard across the air.
    assert_eq!((mixer.sounding(), mixer.audible()), (0, 0));
    emit(&cues, Sound::Kick, Vec3::ZERO);
    listen(&mut mixer, 0.01);
    assert_eq!((mixer.sounding(), mixer.audible()), (1, 0));
}

/// Fly a shell along +X over the bridge at `height`, from 300 m short to 300 m past,
/// and return what was heard with the count of voices at each tick.
fn fly(air: Air, speed: f32, height: f32) -> (Vec<[f32; 2]>, Vec<usize>, Mixer) {
    let (cues, mut mixer) = bridge(air);
    let (mut heard, mut voices) = (Vec::new(), Vec::new());
    for tick in 0..(600. / speed * 60.) as usize {
        cues.send(Cue::Shell {
            serial: 9,
            position: Vec3::new(-300. + speed * tick as f32 / 60., height, 0.),
            velocity: Vec3::X * speed,
            weight: 1.,
        });
        heard.extend(listen(&mut mixer, 1. / 60.));
        voices.push(mixer.sounding());
    }
    assert_eq!(mixer.flying(), 1);
    heard.extend(listen(&mut mixer, 8.));
    (heard, voices, mixer)
}

#[test]
fn a_shell_faster_than_sound_is_silent_until_its_shock_arrives() {
    let (heard, voices, mixer) = fly(Air::SCALED, 360., 30.);
    // Overhead after 300 m, and the cone trails it by an angle set by its speed.
    let (mach, miss) = (360_f32 / 170., 22.5_f32);
    let due = 300. / 360. + miss / 360. * (mach * mach - 1.).sqrt();
    assert_eq!(peak(&heard[..((due - 0.02) * RATE) as usize]), 0.);
    assert!(
        (onset(&heard) - due).abs() < 0.02,
        "{} {due}",
        onset(&heard)
    );
    // The shock is one sound, made once.
    assert_eq!(voices.iter().max(), Some(&1));
    assert!(voices[..(due * 60.) as usize - 2].iter().all(|&n| n == 0));
    // Then the flight is heard falling away, in pitch as well as level.
    let from = |secs: f32| &heard[((due + secs) * RATE) as usize..][..(0.15 * RATE) as usize];
    assert!(level(from(0.35)) > 0.002, "{}", level(from(0.35)));
    assert!(level(from(1.2)) < level(from(0.35)) * 0.5);
    assert!(brightness(from(1.2)) < brightness(from(0.35)) * 0.7);
    // Its last sound has arrived well before the wait is over.
    assert_eq!(mixer.flying(), 0);
}

#[test]
fn a_shell_slower_than_sound_is_heard_coming_and_drops_in_pitch_as_it_passes() {
    let (heard, voices, mixer) = fly(Air::default(), 200., 30.);
    // No shock: only the rush of it, from soon after it is first seen.
    assert!(voices.iter().all(|&n| n == 0));
    assert!(onset(&heard) < 1.4);
    // It passes overhead at 1.5 s. Closing at this speed raises the pitch 2.4 times
    // and going away lowers it to 0.63.
    let at = |secs: f32| &heard[(secs * RATE) as usize..][..(0.2 * RATE) as usize];
    assert!(brightness(at(2.3)) < brightness(at(1.1)) * 0.3);
    assert!(level(at(1.45)) > level(at(1.1)) && level(at(1.45)) > level(at(2.3)));
    assert_eq!(mixer.flying(), 0);
}

#[test]
fn a_blast_alongside_closes_the_ears_for_a_moment() {
    let hear = |under_way: bool, gun: bool| {
        let (cues, mut mixer) = bridge(Air::SCALED);
        if under_way {
            cues.send(Cue::Way {
                speed: 13.6,
                throttle: 1.,
            });
        }
        listen(&mut mixer, 6.);
        if gun {
            emit(&cues, Sound::MainGun, Vec3::new(4., 5., -16.));
        }
        listen(&mut mixer, 5.)
    };
    let (bed, both, gun) = (hear(true, false), hear(true, true), hear(false, true));
    // Once the blast is under the limiter the mix is a plain sum again, so taking the
    // gun away leaves the ship's own noise as the ears had it.
    let under = apart(&both, &gun);
    let span = |heard: &[[f32; 2]], from: f32| -> Vec<[f32; 2]> {
        heard[(from * RATE) as usize..][..(0.2 * RATE) as usize].to_vec()
    };
    let dulled = brightness(&span(&under, 0.2)) / brightness(&span(&bed, 0.2));
    let restored = brightness(&span(&under, 4.5)) / brightness(&span(&bed, 4.5));
    assert!(dulled < 0.92, "{dulled}");
    assert!((0.98..1.02).contains(&restored), "{restored}");
}

/// How nearly a steady sound repeats itself at any period from 4 to 40 ms: close to 1
/// for a held note, and low for a rumble however regular its rhythm.
fn hum(sound: &[f32]) -> f32 {
    let energy: f32 = sound.iter().map(|x| x * x).sum();
    let at = |lag: usize| -> f32 {
        let product: f32 = sound[lag..].iter().zip(sound).map(|(a, b)| a * b).sum();
        product / energy
    };
    let lags = (0.004 * RATE) as usize..(0.04 * RATE) as usize;
    lags.step_by(4).map(at).fold(0., f32::max)
}

#[test]
fn the_engines_are_low_tones_a_crank_turn_apart_and_not_one_note() {
    let run = |seed: u32, revs: f32| -> Vec<f32> {
        let mut diesel = engine::Diesel::new(seed);
        (0..3 * SAMPLE_RATE)
            .map(|_| diesel.run(revs, 1., 0., 0.).0)
            .collect()
    };
    let (idle, full) = (run(1, 0.), run(1, 1.));
    // Recordings aboard ships put nine tenths of an engine's sound below 350 Hz at
    // the most, whatever its revolutions. The way to the bridge sees to that here.
    let lows = |sound: &[f32]| {
        let mut low = [filter::Biquad::low_pass(320., 0.7); 2];
        let kept = sound.iter().map(|&x| {
            let once = low[0].run(x);
            low[1].run(once).powi(2)
        });
        kept.sum::<f32>() / sound.iter().map(|x| x * x).sum::<f32>()
    };
    assert!(lows(&idle) > 0.9 && lows(&full) > 0.9);
    let pitch = |sound: &[f32]| {
        let pairs: Vec<[f32; 2]> = sound.iter().map(|&x| [x; 2]).collect();
        brightness(&pairs)
    };
    let climbed = pitch(&full) / pitch(&idle);
    assert!((0.5..2.5).contains(&climbed), "{climbed}");
    // Eighteen unequal firings make a pattern that comes round once a turn of the
    // crank: 7.5 times a second at full revolutions and 3 at idle. From one firing to
    // the next it hardly repeats at all, which is why there is no single note at the
    // firing rate, only tones a turn apart.
    let repeats = |sound: &[f32], period: f32| -> f32 {
        let energy: f32 = sound.iter().map(|x| x * x).sum();
        let at = |lag: usize| -> f32 {
            let product: f32 = sound[lag..].iter().zip(sound).map(|(a, b)| a * b).sum();
            product / energy
        };
        let lag = period * RATE;
        ((lag * 0.98) as usize..(lag * 1.02) as usize)
            .map(at)
            .fold(0., f32::max)
    };
    assert!(repeats(&full, 1. / 7.5) > 0.5 && repeats(&idle, 1. / 3.) > 0.5);
    assert!(repeats(&full, 1. / 135.) < 0.3 && repeats(&idle, 1. / 54.) < 0.3);
    // A motor of the old kind, for comparison, is one note and nothing else.
    assert!(hum(&idle) < 0.5 && hum(&full) < 0.5);
    let motor: Vec<f32> = (0..SAMPLE_RATE)
        .map(|n| {
            let turn = std::f32::consts::TAU * 64. * n as f32 / RATE;
            (1..=8).map(|k| (k as f32 * turn).sin() / k as f32).sum()
        })
        .collect();
    assert!(hum(&motor) > 0.9);
    // Harder work is louder, and two engines of the same make are never in step.
    let loud = |sound: &[f32]| sound.iter().map(|x| x * x).sum::<f32>();
    let mut eased = engine::Diesel::new(1);
    let eased: Vec<f32> = (0..3 * SAMPLE_RATE)
        .map(|_| eased.run(1., 0.5, 0., 0.).0)
        .collect();
    assert!(loud(&full) > loud(&eased) * 3.);
    let other = run(2, 1.);
    let together: f32 = full.iter().zip(&other).map(|(a, b)| a * b).sum();
    assert!(together.abs() < 0.5 * (loud(&full) * loud(&other)).sqrt());
    // An engine run right down fires no more and falls silent.
    let mut stopped = engine::Diesel::new(1);
    let dying: Vec<(f32, f32)> = (0..4 * SAMPLE_RATE)
        .map(|_| stopped.run(-1., 1., 0., 0.))
        .collect();
    let dying: Vec<f32> = dying.iter().map(|(hull, funnel)| hull + funnel).collect();
    assert!(loud(&dying[3 * SAMPLE_RATE as usize..]) < 1e-12);
}

/// Share of a sound's power that lies above a frequency, near enough.
fn above(sound: &[f32], hz: f32) -> f32 {
    let mut high = [filter::Biquad::high_pass(hz, 0.7); 2];
    let kept = sound.iter().map(|&x| {
        let once = high[0].run(x);
        high[1].run(once).powi(2)
    });
    kept.sum::<f32>() / sound.iter().map(|x| x * x).sum::<f32>()
}

#[test]
fn the_funnel_carries_the_middle_of_the_engine_that_the_hull_loses() {
    let run = |load: f32| -> (Vec<f32>, Vec<f32>) {
        let mut diesel = engine::Diesel::new(1);
        (0..3 * SAMPLE_RATE)
            .map(|_| diesel.run(1., load, 0., 0.))
            .unzip()
    };
    let (hull, funnel) = run(1.);
    // What comes through the ship is all low. What comes down from the funnel is the
    // part of the same engine that the ear picks out.
    assert!(above(&hull, 250.) < 0.1, "{}", above(&hull, 250.));
    assert!(above(&funnel, 250.) > 0.7, "{}", above(&funnel, 250.));
    assert!(above(&funnel, 3000.) < 0.1, "{}", above(&funnel, 3000.));
    // Worked harder, the exhaust grows faster than the blows that make it.
    let loud = |sound: &[f32]| sound.iter().map(|x| x * x).sum::<f32>();
    let (eased_hull, eased_funnel) = run(0.5);
    let (blows, exhaust) = (
        loud(&hull) / loud(&eased_hull),
        loud(&funnel) / loud(&eased_funnel),
    );
    assert!(exhaust > 1.5 * blows, "{exhaust} against {blows}");
    // It is still the engine: a puff for each firing, 135 of them a second.
    let pulse: Vec<f32> = funnel.iter().map(|x| x * x).collect();
    let mean = pulse.iter().sum::<f32>() / pulse.len() as f32;
    let swing: Vec<f32> = pulse.iter().map(|x| x - mean).collect();
    let firing = RATE / 135.;
    let at = |lag: usize| -> f32 {
        let both = swing[lag..].iter().zip(&swing).map(|(a, b)| a * b);
        both.sum::<f32>()
    };
    let near = |lag: f32| (lag as usize - 12..lag as usize + 12).map(at).fold(0., f32::max);
    let (on, off) = (near(firing), near(1.5 * firing));
    assert!(on > 2. * off.max(0.), "{on} against {off}");
}

#[test]
fn a_damaged_engine_misses_stumbles_and_bangs_in_its_exhaust() {
    let run = |fault: f32, bangs: f32| -> (Vec<f32>, Vec<f32>) {
        let mut diesel = engine::Diesel::new(3);
        (0..20 * SAMPLE_RATE)
            .map(|_| diesel.run(1., 1., fault, bangs))
            .unzip()
    };
    let loud = |sound: &[f32]| sound.iter().map(|x| x * x).sum::<f32>();
    let rms = |sound: &[f32]| (loud(sound) / sound.len() as f32).sqrt();
    let crest = |sound: &[f32]| sound.iter().fold(0_f32, |p, x| p.max(x.abs())) / rms(sound);
    let (hull, funnel) = run(0., 0.);
    let (lame, missing) = run(1., 0.);
    let (_, banging) = run(1., 1.5);
    // With cylinders lost there is less of it, through the hull and from the funnel.
    assert!(loud(&lame) < 0.8 * loud(&hull));
    assert!(loud(&missing) < 0.8 * loud(&funnel));
    // A sound engine never bangs, and neither does a lame one with nothing to light.
    assert!(crest(&funnel) < 8. && crest(&missing) < 8.);
    // Recordings put a backfire 10 to 20 dB over the engine it comes from. Its peak
    // stands that far over the engine's own peaks.
    let over = crest(&banging) / crest(&funnel);
    assert!((2.5..20.).contains(&over), "{over}");
    // They come in fits: counted apart when 50 ms of quiet lies between them.
    let floor = 5. * rms(&missing);
    let mut bangs = 0;
    let mut since = usize::MAX;
    for x in &banging {
        if x.abs() > floor {
            bangs += usize::from(since > (0.05 * RATE) as usize);
            since = 0;
        }
        since = since.saturating_add(1);
    }
    assert!((10..80).contains(&bangs), "{bangs} bangs in 20 s");
    // The same engine always fails the same way.
    assert_eq!(run(1., 1.5).1, banging);
}

#[test]
fn engines_knocked_out_fall_silent_and_damaged_ones_are_heard_to_be() {
    let (cues, mut mixer) = desk(Air::SCALED);
    let mut steady = |cue: Cue| {
        cues.send(cue);
        listen(&mut mixer, 6.);
        listen(&mut mixer, 8.)
    };
    let ahead = Cue::Way {
        speed: 0.,
        throttle: 1.,
    };
    let state = |damaged: f32, stopped: f32| Cue::Engines { damaged, stopped };
    let sound = steady(ahead);
    // One engine of two knocked out leaves half of them turning.
    let half = steady(state(0., 0.5));
    assert!(level(&half) < 0.85 * level(&sound) && level(&half) > 0.4 * level(&sound));
    // With every engine out there is the sea and nothing else, whatever is rung.
    let adrift = steady(state(0., 1.));
    let moored = steady(Cue::Moored);
    cues.send(ahead);
    let adrift_again = steady(state(0., 1.));
    assert!(level(&adrift) < 0.2 * level(&sound));
    assert!((level(&adrift_again) / level(&moored) - 1.).abs() < 0.25);
    // Repaired, they start again. Damaged, they bang.
    let mended = steady(state(0., 0.));
    assert!(level(&mended) > 0.8 * level(&sound));
    let rough = steady(state(1., 0.));
    assert!(peak(&rough) > 1.2 * peak(&mended), "{}", peak(&rough));
    assert!(level(&rough) < level(&mended));
    assert!(peak(&rough) < 0.9);
}

/// Let the places make their sounds for a while, and return the most voices each of
/// these sounds had at once and what was heard.
fn haunted<const N: usize>(
    places: &[Place],
    secs: f32,
    sounds: [Sound; N],
) -> ([usize; N], Vec<[f32; 2]>) {
    let (cues, mut mixer) = desk(Air::SCALED);
    let mut ambience = Ambience::new(7);
    let (mut most, mut heard) = ([0; N], Vec::new());
    for _ in 0..(secs * 60.) as usize {
        ambience.pass(1. / 60., Vec3::ZERO, 0., places.iter().copied(), &cues);
        heard.extend(listen(&mut mixer, 1. / 60.));
        for (most, sound) in most.iter_mut().zip(sounds) {
            *most = (*most).max(mixer.making(sound).0);
        }
    }
    (most, heard)
}

#[test]
fn a_place_makes_its_own_sounds_and_is_heard_from_where_it_stands() {
    let place = |haunt: Haunt, x: f32, radius: f32| Place {
        haunt,
        position: Vec3::new(x, 5., 0.),
        radius,
    };
    let kinds = [
        Sound::Breaker,
        Sound::Gull,
        Sound::IceCrack,
        Sound::Calving,
        Sound::Rivets,
    ];
    // An island 150 m to starboard has surf and birds, and no ice or industry.
    let (made, heard) = haunted(&[place(Haunt::Island, 176., 26.)], 60., kinds);
    assert!(made[0] >= 1 && made[1] >= 1, "{made:?}");
    assert_eq!(made[2..], [0, 0, 0]);
    let ear = |i: usize| heard.iter().map(|f| f[i] * f[i]).sum::<f32>();
    assert!(ear(1) > 2. * ear(0) && ear(0) > 0.);
    // It is the background, well under any gun.
    assert!(peak(&heard) < 0.3 && peak(&heard) > 0.002, "{}", peak(&heard));
    // A glacier also splits, ice adrift does without the birds, and a yard rivets.
    let (made, _) = haunted(&[place(Haunt::Glacier, 300., 120.)], 120., kinds);
    assert!(made[0] >= 1 && made[2] >= 1, "{made:?}");
    let (made, _) = haunted(&[place(Haunt::Berg, 60., 14.)], 240., kinds);
    assert!(made[0] >= 1 && made[2] >= 1 && made[1] == 0, "{made:?}");
    let (made, _) = haunted(&[place(Haunt::Yard, 200., 0.)], 30., kinds);
    assert!(made[4] >= 1 && made[0] == 0, "{made:?}");
    // Out of earshot a place is left alone. Only the ship's own frames are heard.
    let far = [place(Haunt::Island, 900., 26.), place(Haunt::Yard, 900., 0.)];
    let (made, heard) = haunted(&far, 60., kinds);
    assert_eq!(made, [0; 5]);
    let (creaks, _) = haunted(&[], 60., [Sound::Creak]);
    assert!(creaks[0] >= 1 && peak(&heard) > 0.);
    // The same seed makes the same minute.
    assert_eq!(haunted(&far, 20., kinds).1, haunted(&far, 20., kinds).1);
}

#[test]
fn the_telegraph_rings_aboard_and_a_calving_glacier_is_heard_late_and_low() {
    let (cues, mut mixer) = desk(Air::SCALED);
    emit(&cues, Sound::Telegraph, Vec3::new(500., 0., 0.));
    let bell = listen(&mut mixer, 1.);
    // Wherever the game says it is, the bell is aboard: at once and in both ears.
    assert!(onset(&bell) < 0.02);
    let ear = |i: usize| bell.iter().map(|f| f[i] * f[i]).sum::<f32>();
    assert!((ear(0) / ear(1) - 1.).abs() < 0.01);
    assert!(brightness(&bell) > 0.02, "{}", brightness(&bell));

    let (cues, mut mixer) = desk(Air::SCALED);
    emit(&cues, Sound::Calving, Vec3::new(0., 6., -680.));
    let fall = listen(&mut mixer, 20.);
    // 680 m is four seconds away. The crack comes first and the roar builds after it.
    assert_eq!(peak(&fall[..(3.95 * RATE) as usize]), 0.);
    assert!(peak(&fall[..(4.1 * RATE) as usize]) > 0.);
    let second = |from: f32| level(&fall[(from * RATE) as usize..((from + 1.) * RATE) as usize]);
    let worst = (4..10).map(|s| second(s as f32)).fold(0., f32::max);
    assert!(worst > 3. * second(4.1), "{worst}");
    assert!(worst > 4. * second(15.) && second(15.) > 0.);
    let roar = &fall[(5.5 * RATE) as usize..(8.5 * RATE) as usize];
    assert!(brightness(roar) < 0.01, "{}", brightness(roar));
}

#[test]
fn a_building_comes_down_in_crashes_and_a_yard_takes_longer() {
    let hear = |sound: Sound| {
        let (cues, mut mixer) = bridge(Air::default());
        emit(&cues, sound, Vec3::new(0., 3.2, -20.));
        let heard = listen(&mut mixer, 8.);
        heard[(onset(&heard) * RATE) as usize..].to_vec()
    };
    // Level of each tenth of a second, and which tenths stand above both neighbours.
    let tenths = |heard: &[[f32; 2]]| -> Vec<f32> {
        let tenth = (0.1 * RATE) as usize;
        heard.chunks(tenth).map(level).collect()
    };
    let shape = |sound: Sound| {
        let levels = tenths(&hear(sound));
        let top = levels.iter().copied().fold(0., f32::max);
        let loudest = levels.iter().position(|&l| l == top).unwrap();
        let lasts = levels.iter().rposition(|&l| l > top * 0.1).unwrap();
        let crashes = (1..levels.len() - 1)
            .filter(|&i| levels[i] > top * 0.3)
            .filter(|&i| levels[i] > levels[i - 1] * 1.25 && levels[i] >= levels[i + 1])
            .count();
        (loudest as f32 * 0.1, lasts as f32 * 0.1, crashes)
    };
    // Recorded collapses are loudest well after they begin, stay within 20 dB of that
    // for two to five seconds and come as several crashes.
    let (worst, lasts, crashes) = shape(Sound::Collapse);
    assert!(worst >= 0.4 && (1.5..3.2).contains(&lasts) && crashes >= 2);
    let (worst, longer, crashes) = shape(Sound::Ruin);
    assert!(worst >= 1. && longer > lasts + 0.8 && crashes >= 3);
    // A shell burst or a ship blowing up is the other way about: loudest at once.
    assert!(shape(Sound::Burst).0 < 0.2 && shape(Sound::Wreck).0 < 0.2);
    // Among the crashes are separate knocks of brick on brick, where a steady hiss of
    // the same loudness would have none. Noise scores 3 on this measure.
    let mut high = filter::Biquad::high_pass(1500., 0.7);
    let knocks: Vec<f32> = (hear(Sound::Collapse)[..(2. * RATE) as usize].iter())
        .map(|f| high.run(f[0]))
        .collect();
    let power = |n: i32| knocks.iter().map(|x| x.powi(n)).sum::<f32>() / knocks.len() as f32;
    assert!(power(4) / power(2).powi(2) > 4.5);
}

#[test]
fn a_base_destroyed_is_cheered_aboard_and_in_tune_every_time() {
    let play = |sound: Sound, serial: u64| {
        let (cues, mut mixer) = bridge(Air::SCALED);
        cues.send(Cue::Emit {
            sound,
            position: Vec3::new(400., 3., 0.),
            serial,
        });
        let heard = listen(&mut mixer, 4.);
        assert_eq!((mixer.sounding(), mixer.audible()), (0, 0));
        heard
    };
    // How nearly the held chord repeats every turn of its lowest note, the G at 98 Hz.
    let in_g = |heard: &[[f32; 2]]| -> f32 {
        let chord: Vec<f32> = (heard[(1.7 * RATE) as usize..(2.6 * RATE) as usize].iter())
            .map(|f| f[0])
            .collect();
        let lag = (RATE / 98.).round() as usize;
        let product: f32 = chord[lag..].iter().zip(&chord).map(|(a, b)| a * b).sum();
        product / chord.iter().map(|x| x * x).sum::<f32>()
    };
    for serial in [0, 3, 11] {
        let heard = play(Sound::Fanfare, serial);
        // It starts at once however far off the base is, and is the same in both ears.
        assert!(onset(&heard) < 0.2);
        assert!(heard.iter().all(|f| f[0] == f[1]));
        assert!(in_g(&heard) > 0.6, "{}", in_g(&heard));
        assert!(peak(&heard) > 0.25 && peak(&heard) < 0.75);
    }
    // It is longer and fuller than the three notes for a ship sunk.
    let (cheer, call) = (play(Sound::Fanfare, 0), play(Sound::Trumpet, 0));
    let sounding = |heard: &[[f32; 2]]| heard.iter().rposition(|f| f[0].abs() > 0.01).unwrap();
    assert!(sounding(&cheer) > 2 * sounding(&call));
}

/// Turns a second of a siren's rotor: the speed at which ten and twelve ports to the
/// turn, and twice those, would best account for a stretch of sound.
fn turns(heard: &[[f32; 2]]) -> f32 {
    let held = |hz: f32| -> f32 {
        let (mut re, mut im) = (0., 0.);
        for (n, frame) in heard.iter().enumerate().step_by(4) {
            let (sin, cos) = (std::f32::consts::TAU * hz * n as f32 / RATE).sin_cos();
            (re, im) = (re + frame[0] * cos, im + frame[0] * sin);
        }
        (re * re + im * im).sqrt()
    };
    let fits = |turns: f32| {
        [10., 12., 20., 24.]
            .map(|ports| held(ports * turns))
            .iter()
            .sum()
    };
    let tried = (20..260).map(|quarters| quarters as f32 / 4.);
    let by_fit = |a: &f32, b: &f32| f32::total_cmp(&fits(*a), &fits(*b));
    tried.max_by(by_fit).unwrap()
}
/// The same, every quarter second from `start`, each over a tenth of a second.
fn spinning(heard: &[[f32; 2]], start: f32) -> Vec<f32> {
    let from = (start * RATE) as usize;
    let stretches = heard[from..].chunks((0.25 * RATE) as usize);
    stretches
        .filter(|stretch| stretch.len() >= (0.1 * RATE) as usize)
        .map(|stretch| turns(&stretch[..(0.1 * RATE) as usize]))
        .collect()
}
fn alarm(cues: &Cues, position: Vec3, raised: bool) {
    cues.send(Cue::Alarm {
        slot: 0,
        rotor: Rotor::Shore,
        position,
        raised,
    });
}

#[test]
fn a_siren_is_heard_late_and_wails_as_its_rotor_gains_and_loses_speed() {
    let (cues, mut mixer) = desk(Air::default());
    alarm(&cues, Vec3::new(0., 12., -343.), true);
    let heard = listen(&mut mixer, 18.);
    assert_eq!(mixer.wailing(), 1);
    // It is switched on at once, and for a second nothing has crossed the water.
    assert_eq!(peak(&heard[..(0.99 * RATE) as usize]), 0.);
    // Full speed is about 2850 turns a minute, reached from rest in about four seconds.
    let rotor = spinning(&heard, 2.);
    let top = rotor.iter().copied().fold(0., f32::max);
    assert!((46. ..49.).contains(&top), "{top}");
    let reached = rotor.iter().position(|&turns| turns > 0.99 * top).unwrap();
    assert!((6..=14).contains(&reached), "{reached}");
    assert!(rotor[..reached].windows(2).all(|w| w[1] > w[0]));
    // Then the power goes off and on. It falls about a third and comes back up, and
    // rising is quicker than falling.
    let wail = &rotor[reached..];
    let low = wail.iter().copied().fold(top, f32::min);
    assert!((0.62..0.74).contains(&(low / top)), "{}", low / top);
    let (falling, rising) = wail.windows(2).fold((0, 0), |(falling, rising), w| {
        let step = w[1] - w[0];
        (
            falling + usize::from(step < -0.3),
            rising + usize::from(step > 0.3),
        )
    });
    assert!(falling > 2 * rising && rising >= 6, "{falling} {rising}");
    let again = wail.iter().rposition(|&turns| turns > 0.99 * top).unwrap();
    assert!(again > wail.iter().position(|&turns| turns < 0.75 * top).unwrap());
    // A third of a kilometre off it is well under a gun and well over nothing.
    let loud = level(&heard[(6. * RATE) as usize..]);
    assert!((0.005..0.08).contains(&loud), "{loud}");
}

#[test]
fn a_siren_runs_down_and_stops_when_its_alarm_is_lowered() {
    let (cues, mut mixer) = desk(Air::default());
    let there = Vec3::new(20., 12., -30.);
    alarm(&cues, there, true);
    let top = turns(&listen(&mut mixer, 5.)[(4.7 * RATE) as usize..]);
    alarm(&cues, there, false);
    let heard = listen(&mut mixer, 20.);
    // The air it throws slows it: quickly at first, and less each second after.
    let falling = spinning(&heard, 0.5);
    assert!(falling.windows(2).all(|w| w[1] <= w[0] + 0.3));
    let lost = |secs: usize| 1. - falling[4 * secs] / falling[4 * (secs - 1)];
    assert!(falling[2] < 0.94 * top && lost(2) > 1.2 * lost(6));
    assert!(falling[4 * 15] > 0.2 * top && falling[4 * 15] < 0.45 * top);
    // It is quieter as it slows, and in the end it stops and leaves nothing in the air.
    assert!(level(&heard[(15. * RATE) as usize..]) < 0.25 * level(&heard[..RATE as usize]));
    assert_eq!(mixer.wailing(), 1);
    listen(&mut mixer, 25.);
    assert_eq!(mixer.wailing(), 0);
    assert_eq!(peak(&listen(&mut mixer, 0.5)), 0.);
    // A hush stops one dead.
    alarm(&cues, there, true);
    assert!(peak(&listen(&mut mixer, 3.)) > 0.01);
    cues.send(Cue::Hush);
    assert_eq!(peak(&listen(&mut mixer, 1.)), 0.);
    assert_eq!(mixer.wailing(), 0);
}

#[test]
fn a_ship_closing_on_a_siren_hears_it_sharp() {
    // A tenth of the speed of sound towards it raises every note a tenth.
    let top = |closing: f32| {
        let (cues, mut mixer) = desk(Air::default());
        alarm(&cues, Vec3::new(0., 12., -700.), true);
        let mut heard = Vec::new();
        for step in 0..12 * 60 {
            cues.send(Cue::Ears {
                position: Vec3::NEG_Z * closing * step as f32 / 60.,
                right: Vec3::X,
            });
            heard.extend(listen(&mut mixer, 1. / 60.));
        }
        spinning(&heard, 4.).into_iter().fold(0., f32::max)
    };
    let (lying, closing) = (top(0.), top(34.3));
    assert!((closing / lying - 1.1).abs() < 0.012, "{lying} {closing}");
}
