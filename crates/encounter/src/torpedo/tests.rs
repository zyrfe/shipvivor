use super::*;
use crate::{EnemyClass, Events};
use shipvivor_compass::direction;
use std::f32::consts::FRAC_PI_2;

const DT: f32 = 1. / 60.;
/// Where Breakwater lies, far out in open water.
const AT: Vec2 = Vec2::splat(3000.);

/// Open water with nothing afloat but what a test puts there, and Breakwater fitted out
/// as the heavy destroyer with her torpedoes fitted so.
fn sea(fit: Torpedoes) -> Battle {
    let mut b = Battle::for_route(
        14.,
        [
            (0., -152.),
            (168., -424.),
            (496., -300.),
            (416., 76.),
            (72., 164.),
        ]
        .map(Vec2::from),
    );
    b.raiders.clear();
    b.landmarks.clear();
    b.floe.icebergs.clear();
    b.fit_tubes(&Design::heavy_destroyer());
    b.fit_torpedoes(fit);
    b
}
fn guided(rank: u16) -> Torpedoes {
    Torpedoes {
        guidance: rank,
        ..Torpedoes::STOCK
    }
}
/// The bearing of her starboard beam, steering north: the tubes amidships take it.
fn beam() -> f32 {
    Rack::Starboard.center(0.)
}
/// A ship of `class` at `pos`, steering `course` at her full speed.
fn under_way(b: &mut Battle, class: EnemyClass, pos: Vec2, course: f32) -> u64 {
    b.spawn(class, pos, pos + direction(course));
    let r = b.raiders.last_mut().unwrap();
    r.way = r.body.forward() * class.profile().speed();
    r.body.id
}
/// Sail for `seconds`, every ship holding her speed and swinging at the rate `swing`
/// gives her by her place in the list, radians a second to starboard, and every torpedo
/// running on. The torpedoes' hits, counted.
fn sail(b: &mut Battle, seconds: f32, swing: impl Fn(usize) -> f32) -> Events {
    let mut ev = Events::default();
    for _ in 0..(seconds / DT).round() as usize {
        for (i, r) in b.raiders.iter_mut().enumerate() {
            let speed = r.way.length();
            r.body.heading += swing(i) * DT;
            r.way = r.body.forward() * speed;
            r.body.pos += r.way * DT;
        }
        b.advance_torpedoes(DT, AT, &mut ev);
    }
    ev
}

#[test]
fn guidance_turns_a_torpedo_onto_a_crossing_ship_that_a_straight_one_misses() {
    // A destroyer off the beam, crossing it at full speed, and a spread laid on where
    // she is rather than where she will be. The first rank of guidance makes up a small
    // error, the second a larger one.
    for (range, rank, hits) in [
        (220., 0, false),
        (220., 1, true),
        (300., 1, false),
        (300., 2, true),
    ] {
        let mut b = sea(guided(rank));
        let ship = AT + direction(beam()) * range;
        under_way(&mut b, EnemyClass::Destroyer, ship, 0.);
        assert!(b.launch_spread(AT, 0., beam()));
        let ev = sail(&mut b, 12., |_| 0.);
        assert_eq!(ev.hits > 0, hits, "rank {rank} at {range} m");
    }
}

#[test]
fn a_guided_torpedo_turns_no_faster_than_its_guidance() {
    // A cruiser 15 degrees off the aim, inside the seeker's cone and its reach, lying
    // still.
    for rank in [1, 2] {
        let mut b = sea(guided(rank));
        let aim = beam();
        b.spawn(
            EnemyClass::Cruiser,
            AT + direction(aim + 15_f32.to_radians()) * 110.,
            AT,
        );
        assert!(b.launch_spread(AT, 0., aim));
        let most = GUIDANCE_TURN[usize::from(rank) - 1].to_radians() * DT;
        let mut was: Vec<f32> = b.torpedoes.iter().map(|t| t.body.heading).collect();
        for _ in 0..30 {
            sail(&mut b, DT, |_| 0.);
            for (fish, before) in b.torpedoes.iter().zip(&mut was) {
                let turned = off_bearing(fish.body.heading, *before).abs();
                assert!(turned <= most * 1.001, "rank {rank}: {turned} > {most}");
                assert!(turned > most * 0.99, "rank {rank} turns as hard as it may");
                *before = fish.body.heading;
            }
        }
    }
}

#[test]
fn a_ship_that_turns_hard_away_near_the_end_of_the_run_gets_away() {
    // A destroyer crossing 400 m off the beam, and a guided spread laid on her lead.
    for (swing, escapes) in [(0., false), (1., true)] {
        let mut b = sea(guided(2));
        let ship = AT + direction(beam()) * 400.;
        under_way(&mut b, EnemyClass::Destroyer, ship, 0.);
        let r = &b.raiders[0];
        let lead = intercept(AT, r.body.pos, r.motion(), TORPEDO_SPEED).unwrap();
        assert!(b.launch_spread(AT, 0., heading(lead - AT)));
        // Hard over to starboard, away from the spread, as fast as she swings: in a
        // circle of 20 m at her full speed.
        let rate = swing * EnemyClass::Destroyer.profile().speed() / 20.;
        let ev = sail(&mut b, 14., |_| rate);
        assert_eq!(ev.hits == 0, escapes, "swing {swing}");
    }
}

#[test]
fn a_fanned_spread_gives_each_torpedo_a_ship_of_its_own() {
    // Three cruisers lying still 260 m off, the spread aimed at the middle one.
    let mut b = sea(guided(3));
    let aim = beam();
    let ids: Vec<u64> = [-12_f32, 0., 12.]
        .map(|off| {
            b.spawn(
                EnemyClass::Cruiser,
                AT + direction(aim + off.to_radians()) * 260.,
                AT,
            );
            b.raiders.last().unwrap().body.id
        })
        .to_vec();
    let sound: Vec<f32> = b.raiders.iter().map(Raider::hull).collect();
    assert!(b.launch_spread(AT, 0., aim));
    let targets: Vec<Option<u64>> = b.torpedoes.iter().map(Torpedo::target).collect();
    // From port to starboard, each to the ship on its own side.
    assert_eq!(targets, ids.iter().copied().map(Some).collect::<Vec<_>>());
    let ev = sail(&mut b, 10., |_| 0.);
    assert_eq!(ev.hits, 3);
    for (r, sound) in b.raiders.iter().zip(sound) {
        assert!(r.hull() < sound, "each ship is struck once");
    }

    // With fewer ships than torpedoes, they double up, the ship aimed at first.
    let mut b = sea(guided(3));
    let near = AT + direction(aim) * 250.;
    b.spawn(EnemyClass::Cruiser, near, AT);
    let first = b.raiders[0].body.id;
    b.spawn(EnemyClass::Cruiser, AT + direction(aim + 0.15) * 300., AT);
    let second = b.raiders[1].body.id;
    assert!(b.launch_spread(AT, 0., aim));
    let targets: Vec<Option<u64>> = b.torpedoes.iter().map(Torpedo::target).collect();
    assert_eq!(targets, [Some(first), Some(first), Some(second)]);

    // Below the third rank, the whole spread runs for the ship aimed at.
    let mut b = sea(guided(2));
    b.spawn(EnemyClass::Cruiser, near, AT);
    b.spawn(EnemyClass::Cruiser, AT + direction(aim + 0.15) * 300., AT);
    let first = b.raiders[0].body.id;
    assert!(b.launch_spread(AT, 0., aim));
    assert!(b.torpedoes.iter().all(|t| t.target() == Some(first)));
}

#[test]
fn full_guidance_fires_five_and_sees_wider() {
    let mut b = sea(guided(GUIDANCE_RANKS));
    let aim = beam();
    assert!(b.launch_spread(AT, 0., aim));
    assert_eq!(b.torpedoes.len(), FULL_SPREAD);
    // Still across the same spread either side of the aim, from port to starboard.
    let offs: Vec<f32> = b
        .torpedoes
        .iter()
        .map(|t| off_bearing(t.body.heading, aim))
        .collect();
    assert!((offs[0] + SPREAD_HALF_ARC).abs() < 1e-5);
    assert!((offs[4] - SPREAD_HALF_ARC).abs() < 1e-5);
    assert!(offs.windows(2).all(|w| w[1] > w[0]));
    assert!(b.guidance().cone > Guidance::for_rank(3).cone);
    for rank in 0..GUIDANCE_RANKS {
        assert_eq!(Guidance::for_rank(rank).spread, SPREAD);
    }
}

#[test]
fn quick_tubes_load_sooner_and_long_running_torpedoes_run_farther_as_fast() {
    let fit = Torpedoes {
        reload: 0.5,
        run: 1.4,
        ..Torpedoes::STOCK
    };
    let mut b = sea(fit);
    assert!(b.launch_spread(AT, 0., beam()));
    let set = b.tubes_for_aim(0., beam()).unwrap();
    assert!((b.tube_sets()[set].reload() - TUBE_RELOAD * 0.5).abs() < 1e-4);
    // At the stock speed, for 1.4 times the stock run.
    let from = b.torpedoes[0].body.pos;
    sail(&mut b, 1., |_| 0.);
    assert!((b.torpedoes[0].body.pos.distance(from) - TORPEDO_SPEED).abs() < 0.1);
    let stock = TORPEDO_RANGE / TORPEDO_SPEED;
    sail(&mut b, stock, |_| 0.);
    assert_eq!(b.torpedoes.len(), 3, "still running past the stock run");
    sail(&mut b, stock * 0.5, |_| 0.);
    assert!(b.torpedoes.is_empty());
}

#[test]
fn a_guided_torpedo_steers_in_short_steps_through_a_long_one() {
    // One long step turns it no more than its turn rate allows over the time.
    let mut b = sea(guided(2));
    let aim = beam();
    b.spawn(
        EnemyClass::Cruiser,
        AT + direction(aim + FRAC_PI_2 * 0.25) * 110.,
        AT,
    );
    assert!(b.launch_spread(AT, 0., aim));
    let before = b.torpedoes[1].body.heading;
    b.advance_torpedoes(0.5, AT, &mut Events::default());
    let turned = off_bearing(b.torpedoes[1].body.heading, before).abs();
    let most = GUIDANCE_TURN[1].to_radians() * 0.5;
    assert!(turned <= most * 1.001 && turned > most * 0.9);
}
