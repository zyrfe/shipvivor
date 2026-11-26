use super::*;

const TICK: f32 = 1. / 60.;
/// A cruiser-sized raider: 58 metres long.
const CRUISER: Hull = Hull {
    radius: 5.8,
    bow: 27.,
    stern: 24.,
    mass: 30.,
    shock_per_point: 0.5,
};

fn steaming(id: u64, pos: Vec2, pace: f32) -> Steaming {
    Steaming {
        id,
        pos,
        heading: 0.,
        hull: CRUISER,
        way: 6.6 * pace,
        pace,
    }
}

/// Sail one raider due north from the origin, and return her tracks and where she ends up.
fn sail(pace: f32, seconds: f32) -> (Tracks, Vec2) {
    let mut tracks = Tracks::default();
    let mut pos = Vec2::ZERO;
    for _ in 0..(seconds / TICK) as usize {
        pos += shipvivor_compass::direction(0.) * 6.6 * pace * TICK;
        tracks.step(TICK, [steaming(1, pos, pace)]);
    }
    (tracks, pos)
}

fn points(wakes: &Wakes, slot: usize) -> &[Vec4] {
    let kept = wakes.bounds[slot].w as usize;
    &wakes.track[slot * TRACK..slot * TRACK + kept]
}

#[test]
fn a_hull_at_speed_trails_fading_widening_white_water_astern() {
    let (tracks, pos) = sail(1., 30.);
    let wakes = tracks.draw(pos);
    assert_eq!(wakes.count, 1);
    let [head, hull] = [wakes.hulls[0], wakes.hulls[1]];
    assert_eq!(head.truncate().truncate(), pos);
    assert!(hull.w > 0.99, "a full bow wave at full speed: {hull}");
    let track = points(&wakes, 0);
    assert_eq!(track.len(), TRACK);
    // From her stern back, the white thins and the band spreads.
    let stern = pos - shipvivor_compass::direction(0.) * CRUISER.stern;
    assert!(track[0].truncate().truncate().distance(stern) < 1e-3);
    assert!(track[0].z > 1.2);
    for pair in track.windows(2) {
        assert!(pair[1].z <= pair[0].z && pair[1].w >= pair[0].w, "{pair:?}");
    }
    assert!(track[TRACK - 1].z < 0.1 * track[0].z);
    // It runs back along her course, most of her white water's life astern.
    let tail = track[TRACK - 1].truncate().truncate();
    let astern = stern.distance(tail);
    assert!(astern > 0.8 * 6.6 * life(&CRUISER), "{astern}");
    assert!(tail.x.abs() < 1e-3);
    // Everything she leaves lies inside the circle the shader tests first.
    let bound = wakes.bounds[0];
    let inside = |at: Vec2, reach: f32| at.distance(bound.truncate().truncate()) + reach <= bound.z;
    assert!(track.iter().all(|p| inside(p.truncate().truncate(), p.w)));
    assert!(inside(pos + shipvivor_compass::direction(0.) * CRUISER.bow, hull.z));
}

#[test]
fn a_hull_lying_stopped_leaves_nothing() {
    let (tracks, pos) = sail(0., 30.);
    let wakes = tracks.draw(pos);
    assert_eq!(wakes.count, 1);
    assert_eq!(wakes.hulls[1].w, 0.);
    assert!(points(&wakes, 0).iter().all(|p| p.z == 0.));
}

#[test]
fn a_slow_hull_still_shows_she_has_way_on() {
    let (tracks, pos) = sail(0.2, 30.);
    let wakes = tracks.draw(pos);
    let (bow, stern) = (wakes.hulls[1].w, points(&wakes, 0)[0].z);
    assert!(bow > 0.3 && bow < 0.6, "{bow}");
    assert!(stern > 0.4 && stern < 0.8, "{stern}");
}

#[test]
fn a_sunk_hulls_white_water_lingers_then_goes() {
    let (mut tracks, pos) = sail(1., 20.);
    tracks.step(TICK, []);
    let wakes = tracks.draw(pos);
    assert_eq!(wakes.count, 1);
    assert_eq!(wakes.hulls[0], Vec4::ZERO);
    assert_eq!(wakes.hulls[1], Vec4::ZERO);
    assert!(points(&wakes, 0)[0].z > 1.);
    for _ in 0..(life(&CRUISER) / TICK) as usize + 2 {
        tracks.step(TICK, []);
    }
    assert_eq!(tracks.draw(pos).count, 0);
}

#[test]
fn only_the_nearest_hulls_are_drawn() {
    let mut tracks = Tracks::default();
    let fleet = (0..WAKES as u64 + 8).map(|id| steaming(id, Vec2::new(100. * id as f32, 0.), 1.));
    tracks.step(TICK, fleet);
    let wakes = tracks.draw(Vec2::ZERO);
    assert_eq!(wakes.count, WAKES as u32);
    let farthest = (0..WAKES).map(|slot| wakes.hulls[2 * slot].x).fold(0., f32::max);
    assert_eq!(farthest, 100. * (WAKES - 1) as f32);
}
