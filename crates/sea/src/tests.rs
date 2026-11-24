use super::*;
use glam::Vec2;
use shipvivor_compass::direction;

const TICK: f32 = 1. / 60.;

fn cruiser(pos: Vec2, heading: f32) -> Keel {
    Keel {
        id: 7,
        pos,
        heading,
        half_beam: 2.75,
        bow: 16.,
        stern: 14.1,
        draught: 1.1,
    }
}

/// Sail a cruiser due north from the origin, and return the wake and where she ends up.
fn sail(speed: f32, seconds: f32) -> (Wake, Vec2) {
    let mut wake = Wake::default();
    let mut pos = Vec2::ZERO;
    for _ in 0..(seconds / TICK) as usize {
        pos += direction(0.) * speed * TICK;
        wake.step(TICK, pos, &[cruiser(pos, 0.)]);
    }
    (wake, pos)
}

#[test]
fn a_transform_and_its_inverse_return_the_field() {
    let side = 32;
    let grid = fft::Grid::new(side);
    let sample = |seed: f32| -> Vec<f32> {
        (0..side * side)
            .map(|i| (i as f32 * seed).sin() + (i as f32 * 0.37 * seed).cos())
            .collect()
    };
    let (real, imaginary) = (sample(0.71), sample(1.93));
    let (mut re, mut im) = (real.clone(), imaginary.clone());
    grid.transform(&mut re, &mut im, false);
    // A lone wave lands on its own wavenumber. Three crests east and five south
    // sit at row 3, column 5 of the transposed result.
    let mut wave: Vec<f32> = (0..side * side)
        .map(|i| {
            let (x, y) = ((i % side) as f32, (i / side) as f32);
            (std::f32::consts::TAU * (3. * x + 5. * y) / side as f32).cos()
        })
        .collect();
    let mut none = vec![0.; side * side];
    grid.transform(&mut wave, &mut none, false);
    let half = (side * side) as f32 / 2.;
    for (i, level) in wave.iter().enumerate() {
        let (row, column) = (i / side, i % side);
        let peak = (row, column) == (3, 5) || (row, column) == (side - 3, side - 5);
        assert!((level - if peak { half } else { 0. }).abs() < 0.01 * half);
    }
    grid.transform(&mut re, &mut im, true);
    let scale = (side * side) as f32;
    for i in 0..side * side {
        assert!((re[i] / scale - real[i]).abs() < 1e-4);
        assert!((im[i] / scale - imaginary[i]).abs() < 1e-4);
    }
}

#[test]
fn a_storm_raises_a_longer_and_higher_sea_than_a_breeze() {
    let breeze = Swell::new(0.1, 0.3);
    let storm = Swell::new(1., 0.3);
    let height = |sea: &Swell| sea.trains().iter().map(|t| t.amplitude).sum::<f32>();
    let longest = |sea: &Swell| std::f32::consts::TAU / sea.trains()[0].wave.length();
    assert!(height(&breeze) < 0.35);
    assert!((2.0..3.6).contains(&height(&storm)));
    assert!(longest(&storm) > 2. * longest(&breeze));
    // Even the steepest sea stops short of folding over itself.
    let squeeze: f32 = storm
        .trains()
        .iter()
        .map(|t| CHOP * t.amplitude * t.wave.length())
        .sum();
    assert!(squeeze < 0.75);
    // The long swell runs within a point or two of the wind.
    assert!(storm.trains()[0].wave.normalize().dot(direction(0.3)) > 0.95);
}

#[test]
fn the_surface_over_a_point_is_the_water_carried_there() {
    let mut sea = Swell::new(0.8, 2.1);
    sea.advance(13.7);
    for rest in [Vec2::ZERO, Vec2::new(31., -17.), Vec2::new(-240., 95.)] {
        let moved = sea.displacement(rest);
        let over = rest + Vec2::new(moved.x, moved.z);
        assert!((sea.height(over) - moved.y).abs() < 2e-3);
    }
    // Crests are drawn together and sharpened: the water spends longer below the mean than above.
    let above = (0..4000)
        .filter(|&i| sea.height(Vec2::new(i as f32 * 0.37, i as f32 * 0.11)) > 0.)
        .count();
    assert!(above < 1950);
}

#[test]
fn waves_travel_downwind_at_the_speed_of_their_length() {
    let mut sea = Swell::new(0.5, 0.);
    let train = sea.trains()[0];
    let length = std::f32::consts::TAU / train.wave.length();
    let speed = (9.81 * length / std::f32::consts::TAU).sqrt();
    // Find a crest of the first train, run the sea on, and look for it downwind.
    let crest = train.wave.normalize() * (train.phase / train.wave.length());
    sea.advance(2.);
    let after = sea.trains()[0];
    let ahead = crest + train.wave.normalize() * speed * 2.;
    let angle = (after.wave.dot(ahead) - after.phase).rem_euclid(std::f32::consts::TAU);
    assert!(angle.min(std::f32::consts::TAU - angle) < 1e-3);
}

#[test]
fn a_long_hull_rides_out_waves_that_toss_a_boat() {
    let mut sea = Swell::new(1., 0.);
    let (mut boat_roll, mut ship_roll, mut boat_heave, mut ship_heave) =
        (0_f32, 0_f32, 0_f32, 0_f32);
    for _ in 0..400 {
        sea.advance(0.11);
        // Beam on to the wind.
        let boat = sea.ride(Vec2::ZERO, 1.57, 5., 1.5);
        let ship = sea.ride(Vec2::ZERO, 1.57, 40., 10.);
        boat_roll = boat_roll.max(boat.roll.abs());
        ship_roll = ship_roll.max(ship.roll.abs());
        boat_heave = boat_heave.max(boat.heave.abs());
        ship_heave = ship_heave.max(ship.heave.abs());
    }
    assert!(boat_roll > 0.12 && ship_roll < 0.6 * boat_roll);
    assert!(boat_heave > 1.2 && ship_heave < 0.75 * boat_heave);
}

#[test]
fn a_hull_lifts_her_bow_to_the_wave_ahead() {
    let mut sea = Swell::new(0.7, 0.4);
    let mut agree = 0;
    for step in 0..200 {
        sea.advance(0.13);
        let heading = step as f32 * 0.07;
        let ride = sea.ride(Vec2::new(20., -8.), heading, 15., 2.75);
        let ahead = direction(heading);
        let point = Vec2::new(20., -8.);
        // A small hull follows the slope of the water between her ends and across her beam.
        let fore = sea.height(point + ahead * 15.) - sea.height(point - ahead * 15.);
        let side =
            sea.height(point + ahead.perp() * 2.75) - sea.height(point - ahead.perp() * 2.75);
        if fore.abs() > 0.3 {
            agree += i32::from(fore.signum() == ride.pitch.signum()) * 2 - 1;
        }
        if side.abs() > 0.08 {
            agree += i32::from(side.signum() == ride.roll.signum()) * 2 - 1;
        }
    }
    assert!(agree > 120, "pitch and roll follow the water {agree}");
}

#[test]
fn a_ship_at_rest_leaves_the_water_alone() {
    let (wake, pos) = sail(0., 5.);
    for offset in [Vec2::ZERO, Vec2::new(4., 20.), Vec2::new(-9., -3.)] {
        assert_eq!(wake.height(pos + offset), 0.);
        assert_eq!(wake.foam(pos + offset), 0.);
        assert_eq!(wake.slick(pos + offset), 0.);
    }
}

#[test]
fn a_ship_under_way_trails_a_spreading_v_of_waves() {
    let (wake, pos) = sail(10.2, 14.);
    let rough = |along: f32, across: std::ops::RangeInclusive<i32>| {
        across
            .map(|x| wake.height(pos + Vec2::new(x as f32, along)).abs())
            .fold(0_f32, f32::max)
    };
    // Water a ship's length ahead has not heard of her yet.
    assert!(rough(-45., -40..=40) < 0.02);
    // Astern, the waves fill a wedge that widens with distance, mirrored either side.
    assert!(rough(30., 5..=15) > 0.15);
    assert!(rough(60., 14..=26) > 0.08);
    assert!(rough(30., 35..=50) < 0.3 * rough(30., 5..=15));
    for (x, y) in [(8., 30.), (15., 50.), (5., 20.)] {
        let (port, starboard) = (
            wake.height(pos + Vec2::new(-x, y)),
            wake.height(pos + Vec2::new(x, y)),
        );
        assert!((port - starboard).abs() < 0.02);
    }
    // The bow shoulders up a wave of its own.
    assert!(wake.height(pos + Vec2::new(2.5, -9.)) > 0.2);
}

#[test]
fn foam_lines_the_edges_of_a_wake_and_the_slick_outlasts_it() {
    let (wake, pos) = sail(10.2, 14.);
    let astern = |metres: f32| pos + Vec2::new(0., 14.1 + metres);
    let rim = Vec2::X * 3.;
    // The bow wave breaks along her sides. The middle of the lane is left nearly clear.
    for metres in [2., 11., 26.] {
        let (port, middle, starboard) = (
            wake.foam(astern(metres) - rim),
            wake.foam(astern(metres)),
            wake.foam(astern(metres) + rim),
        );
        assert!(starboard > 2.5 * middle);
        assert!((port - starboard).abs() < 0.02);
    }
    assert!(wake.foam(astern(2.) + rim) > 1.);
    assert!(wake.foam(astern(40.) + rim) < 0.6 * wake.foam(astern(2.) + rim));
    assert!(wake.foam(astern(100.) + rim) < 0.3);
    assert!(wake.slick(astern(40.)) > 0.7);
    assert!(wake.slick(astern(100.)) > 0.4);
    assert!(wake.slick(astern(100.)) < wake.slick(astern(40.)));
    // The slick is a little wider than the ship and has clean water either side.
    assert!(wake.slick(astern(40.) + Vec2::X * 3.5) > 0.5);
    assert_eq!(wake.slick(astern(40.) + Vec2::X * 12.), 0.);
    assert_eq!(wake.foam(pos - Vec2::Y * 40.), 0.);
    // A slow ship leaves far less white water than a fast one.
    let (slow, slow_pos) = sail(3.4, 14.);
    assert!(slow.foam(slow_pos + Vec2::new(3., 16.1)) < 0.4 * wake.foam(astern(2.) + rim));
}

#[test]
fn a_splash_spreads_in_rings_and_dies_away() {
    let mut wake = Wake::default();
    let still = cruiser(Vec2::new(0., 60.), 0.);
    wake.step(TICK, Vec2::ZERO, &[still]);
    wake.splash(Vec2::new(20., 0.), 2.2);
    let ring = |wake: &Wake, radius: f32| {
        (0..24)
            .map(|i| {
                let around = direction(i as f32 * 0.2618) * radius;
                wake.height(Vec2::new(20., 0.) + around).abs()
            })
            .fold(0_f32, f32::max)
    };
    assert!(wake.height(Vec2::new(20., 0.)) < -0.8);
    assert!(wake.foam(Vec2::new(20., 0.)) > 1.);
    assert!(ring(&wake, 9.) < 1e-3);
    for _ in 0..150 {
        wake.step(TICK, Vec2::ZERO, &[still]);
    }
    // After two and a half seconds the hollow has filled and the rings are well out.
    let early = ring(&wake, 9.).max(ring(&wake, 6.));
    assert!(early > 0.03);
    assert!(wake.height(Vec2::new(20., 0.)).abs() < 0.15);
    for _ in 0..1200 {
        wake.step(TICK, Vec2::ZERO, &[still]);
    }
    let late = (1..40).map(|r| ring(&wake, r as f32)).fold(0_f32, f32::max);
    assert!(late < 0.1 * early);
}

#[test]
fn the_patch_follows_the_ship_and_never_meets_its_own_wake() {
    // Far enough to cross both patches more than once.
    let (wake, pos) = sail(13.6, 45.);
    assert!(pos.y < -600.);
    for ahead in [30., 60., 85.] {
        for across in [-40., 0., 40.] {
            let point = pos + Vec2::new(across, -ahead);
            assert!(wake.height(point).abs() < 0.03);
            assert_eq!(wake.foam(point), 0.);
        }
    }
    // Nothing is shown beyond the patch, however far the grids wrap.
    assert_eq!(wake.height(pos + Vec2::new(3., 96.5)), 0.);
    assert_eq!(wake.slick(pos + Vec2::new(0., 192.5)), 0.);
    assert!(wake.slick(pos + Vec2::new(0., 60.)) > 0.5);
}

#[test]
fn a_new_voyage_starts_on_still_water() {
    let (mut wake, pos) = sail(10.2, 6.);
    assert!(wake.foam(pos + Vec2::new(3., 20.)) > 0.5);
    wake.clear();
    wake.step(TICK, Vec2::ZERO, &[cruiser(Vec2::ZERO, 1.)]);
    assert_eq!(wake.foam(pos + Vec2::new(3., 20.)), 0.);
    assert_eq!(wake.height(pos + Vec2::new(6., 20.)), 0.);
}

#[test]
fn texels_carry_the_surface_as_half_floats() {
    let mut wake = Wake::default();
    wake.step(TICK, Vec2::ZERO, &[]);
    wake.splash(Vec2::ZERO, 2.);
    let mut surface = vec![0; SIDE * SIDE * 8];
    let mut foam = vec![0; FOAM_SIDE * FOAM_SIDE * 2];
    wake.write_surface(&mut surface);
    wake.write_foam(&mut foam);
    let half = |texel: usize, part: usize| {
        let at = texel * 8 + part * 2;
        let bits = u16::from_le_bytes([surface[at], surface[at + 1]]);
        let (sign, exponent, fraction) = (bits >> 15, (bits >> 10) & 31, bits & 1023);
        let size = (1. + f32::from(fraction) / 1024.) * 2_f32.powi(i32::from(exponent) - 15);
        if exponent == 0 {
            0.
        } else if sign == 1 {
            -size
        } else {
            size
        }
    };
    // The hollow is centred on the first cell. The grid wraps, so the cell to its west is the last in the row.
    assert!((half(0, 0) - wake.height(Vec2::ZERO)).abs() < 1e-3);
    assert!(half(0, 0) < -0.5);
    assert!(half(0, 1).abs() < 1e-3 && half(0, 2).abs() < 1e-3);
    assert!(half(1, 1) > 0.05 && half(SIDE - 1, 1) < -0.05);
    assert!(half(SIDE, 2) > 0.05);
    assert_eq!(half(1, 3), 0.);
    assert_eq!(foam[0], 255);
    assert_eq!(foam[1], 0);
}
