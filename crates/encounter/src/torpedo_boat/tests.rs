use super::*;
use crate::{Aim, Chart, Events, HarbourSite, Hull, MainShell, Orders};
use shipvivor_shipyard::Reach;

const DT: f32 = 1. / 30.;

/// One-factory bases at the given places and stages, facing north, in open water, with
/// their torpedo boats moored off them and nothing else afloat. No harriers fly, no
/// slip or convoy sails and Breakwater's crew holds its fire.
fn sector(sites: &[(Vec2, u8)]) -> Battle {
    let mut chart = Chart::classic(
        [
            (0., -152.),
            (168., -424.),
            (496., -300.),
            (416., 76.),
            (72., 164.),
        ]
        .map(Vec2::from),
    );
    chart.islets.clear();
    chart.harbours = sites
        .iter()
        .map(|&(pos, stage)| HarbourSite {
            pos,
            heading: 0.,
            works: 1,
            stage,
        })
        .collect();
    let mut b = Battle::for_chart(1e4, &chart);
    b.raiders.retain(|r| r.class == EnemyClass::TorpedoBoat);
    b.ground_harriers();
    b.wing_guns.clear();
    for port in &mut b.ports {
        port.convoy_clock = 1e6;
        port.coaster_clock = 1e6;
    }
    b
}

fn orders(player: Vec2, heading: f32, speed: f32) -> Orders {
    Orders {
        player,
        heading,
        speed,
        hull: Hull::CRUISER,
        aim: Aim::Hold,
        reload: 6.,
        shell: MainShell::STOCK,
    }
}

/// Step the battle while she holds her course and speed, and every tick's events.
fn sail(b: &mut Battle, at: &mut Orders, seconds: f32) -> Vec<Events> {
    let mut all = Vec::new();
    for _ in 0..(seconds / DT).round() as usize {
        all.push(b.step(DT, at));
        at.player += direction(at.heading) * at.speed * DT;
    }
    all
}

fn boats(b: &Battle) -> impl Iterator<Item = &Raider> {
    b.raiders
        .iter()
        .filter(|r| r.class == EnemyClass::TorpedoBoat)
}

fn hostile(b: &Battle) -> impl Iterator<Item = &crate::Torpedo> {
    b.torpedoes.iter().filter(|t| t.hostile())
}

#[test]
fn each_base_moors_a_pair_clear_of_the_land_and_the_harriers() {
    let b = sector(&[(Vec2::new(0., -1000.), 0), (Vec2::new(1200., -1000.), 1)]);
    for (base, port) in b.ports.iter().enumerate() {
        let pair: Vec<&Raider> = boats(&b).filter(|r| r.moored_at() == Some(base)).collect();
        assert_eq!(pair.len(), MOORED_PAIR, "base {base}");
        for boat in &pair {
            let (home, mooring) = boat.torpedo_mooring().unwrap();
            assert_eq!(home, base);
            assert!(boat.body.pos.distance(mooring) < 0.01);
            assert!(boat.torpedo_aboard() && !boat.running_in());
            assert!(boat.body.pos.distance(port.body.pos) > crate::ISLAND_RADIUS + 10.);
            for slot in 0..crate::LAST_FORMATION {
                assert!(boat.body.pos.distance(port.harrier_mooring(slot)) > 10.);
            }
        }
        assert!(pair[0].body.pos.distance(pair[1].body.pos) > 14.);
    }
    // At anchor they are no part of the fleet at sea.
    assert_eq!(b.combatants(), 0);
    assert_eq!(b.torpedo_boats_out(), 0);
}

#[test]
fn a_boat_makes_two_runs_to_her_beam_twenty_seconds_apart_and_each_torpedo_floods_her() {
    let mut b = sector(&[(Vec2::new(0., -1000.), 0)]);
    let ids: Vec<u64> = boats(&b).map(|r| r.body.id).collect();
    // Northbound at 6 m/s, 550 m east of the base and in sight of it.
    let mut at = orders(Vec2::new(550., -1000.), 0., 6.);
    // Nobody leaves before the base has been roused for its first interval.
    sail(&mut b, &mut at, FIRST_RUN - 1.);
    assert!(boats(&b).all(|r| r.moored_at().is_some()));
    assert!(b.ports[0].has_contact);
    // Each torpedo fired: by whom, when, and where she lay from it.
    let mut shots: Vec<(u64, f32, f32, f32)> = Vec::new();
    let mut torpedoed = Vec::new();
    let mut gunfire = 0_u16;
    let mut fastest = 0_f32;
    // How far off her each boat lay while she harried her between runs.
    let mut between: Vec<f32> = Vec::new();
    let mut t = 0.;
    for _ in 0..(120. / DT) as usize {
        let before: Vec<u64> = hostile(&b).map(|t| t.body.id).collect();
        // These boats judge her course and speed exactly.
        for run in b.raiders.iter_mut().filter_map(|r| r.attack.as_mut()) {
            run.miss = 0.;
        }
        let events = b.step(DT, &at);
        t += DT;
        for blow in &events.blows {
            if blow.calibre.is_none() {
                torpedoed.push(*blow);
            } else {
                gunfire += blow.damage;
            }
        }
        for fish in hostile(&b).filter(|t| !before.contains(&t.body.id)) {
            // Where she lay from the boat when it fired, off her course.
            let offset = fish.body.pos - at.player;
            let off_course = (offset.normalize().dot(direction(at.heading))).acos();
            shots.push((fish.fired_by.unwrap(), t, offset.length(), off_course));
            assert!(
                fish.closing(at.player, direction(at.heading) * at.speed, 6.)
                    .is_some()
            );
        }
        for r in boats(&b) {
            fastest = fastest.max(r.motion().length());
            if r.harrying() && r.torpedo_aboard() {
                between.push(r.body.pos.distance(at.player));
            }
        }
        at.player += direction(at.heading) * at.speed * DT;
    }
    // Each of the pair fired both her torpedoes, one on each run, the second no sooner
    // than she had harried her for the interval between them.
    assert_eq!(
        shots.len(),
        MOORED_PAIR * usize::from(TORPEDOES),
        "{shots:?}"
    );
    for id in &ids {
        let hers: Vec<f32> = (shots.iter().filter(|s| s.0 == *id)).map(|s| s.1).collect();
        assert_eq!(hers.len(), usize::from(TORPEDOES), "{shots:?}");
        assert!(hers[1] - hers[0] >= RERUN, "{shots:?}");
    }
    // Every run is made from off her beam in the firing range, the second as well as
    // the first: she stands out again before turning in.
    for &(_, _, range, off_course) in &shots {
        assert!(
            (FIRING_RANGE.0 - 20.0..=FIRING_RANGE.1).contains(&range),
            "{shots:?}"
        );
        assert!((off_course - PI / 2.).abs() <= ON_BEAM, "{shots:?}");
    }
    // Between runs she closed to circle her about her gun's range.
    let nearest = between.iter().copied().fold(f32::MAX, f32::min);
    assert!(
        (HARRYING_RANGE - 50.0..=HARRYING_RANGE + 30.).contains(&nearest),
        "{nearest}"
    );
    // Her gun found her now and then, a pinprick beside a torpedo.
    assert!((1..=WARHEAD).contains(&gunfire), "{gunfire}");
    // She dashed in well above a destroyer's speed.
    assert!(fastest > 10., "{fastest}");
    // Holding her course, she was hit by all four. Each is a heavy blow below the
    // waterline that no plate turns.
    assert_eq!(torpedoed.len(), shots.len(), "{torpedoed:?}");
    for blow in &torpedoed {
        assert_eq!(blow.damage, WARHEAD);
        assert_eq!(blow.reach, Reach::Bays);
        assert!(blow.aft >= 0. && blow.aft <= Hull::CRUISER.bow + Hull::CRUISER.stern);
    }
    // The boats lay to port of her for their first runs.
    assert!(
        torpedoed[..2].iter().all(|blow| blow.abeam < 0.),
        "{torpedoed:?}"
    );
    // Through her own plating, the blow opens the module to the sea.
    let mut ship = shipvivor_shipyard::Ship::new(shipvivor_shipyard::Design::breakwater());
    let blow = torpedoed[0];
    ship.strike_into(
        blow.aft,
        blow.abeam,
        f32::from(blow.damage),
        blow.calibre,
        blow.reach,
    );
    let module = ship.design().module_at(blow.aft);
    assert!(ship.compartments()[module].inflow() > 0.);
    // Their tubes empty, both stay out to harry her.
    assert!(boats(&b).all(|r| !r.torpedo_aboard() && !r.running_in() && r.harrying()));
}

#[test]
fn spent_she_harries_the_ship_until_she_loses_her_then_reloads_at_the_mooring_and_comes_again() {
    let mut b = sector(&[(Vec2::new(0., -1000.), 0)]);
    let base = b.ports[0].body.pos;
    // Lying stopped to the east of the base, beam on to it.
    let mut at = orders(Vec2::new(520., -1000.), 0., 0.);
    let mut judged = Vec::new();
    let mut fired = 0;
    let mut gunfire = 0_u16;
    // Each boat's range and bearing from her over the last minute of harrying.
    let mut circling: Vec<Vec<(f32, f32)>> = vec![Vec::new(); MOORED_PAIR];
    let mut t = 0.;
    let step = |b: &mut Battle, at: &Orders, t: &mut f32, judged: &mut Vec<f32>| {
        let moored: Vec<bool> = boats(b).map(|r| r.moored_at().is_some()).collect();
        let before: Vec<u64> = hostile(b).map(|t| t.body.id).collect();
        let events = b.step(DT, at);
        *t += DT;
        for (r, was) in boats(b).zip(moored) {
            if was && r.moored_at().is_none() {
                judged.push(r.attack.as_ref().unwrap().miss);
            }
        }
        let new = hostile(b).filter(|t| !before.contains(&t.body.id)).count();
        let shells = (events.blows.iter())
            .filter(|blow| blow.calibre.is_some())
            .map(|blow| blow.damage)
            .sum::<u16>();
        (new, shells)
    };
    // Out and both runs made.
    let all = MOORED_PAIR * usize::from(TORPEDOES);
    while fired < all && t < 200. {
        let (new, shells) = step(&mut b, &at, &mut t, &mut judged);
        fired += new;
        gunfire += shells;
    }
    assert_eq!(fired, all);
    // A minute on, empty, neither has gone home: both circle her at about her gun's
    // range, each sweeping well round her, and their fire finds her now and then.
    let spent = t;
    while t < spent + 60. {
        let (new, shells) = step(&mut b, &at, &mut t, &mut judged);
        fired += new;
        gunfire += shells;
        if t > spent + 15. {
            for (k, r) in boats(&b).enumerate() {
                let off = r.body.pos - at.player;
                circling[k].push((off.length(), bearing(off)));
            }
        }
    }
    assert_eq!(fired, all, "nothing more to fire");
    assert!(boats(&b).all(|r| r.moored_at().is_none() && r.harrying()));
    for track in &circling {
        let (near, far) = (track.iter()).fold((f32::MAX, 0_f32), |(n, f), &(range, _)| {
            (n.min(range), f.max(range))
        });
        let swept = (track.windows(2))
            .map(|w| signed(w[1].1 - w[0].1).abs())
            .sum::<f32>();
        assert!(
            near > HARRYING_RANGE - 60. && far < HARRYING_RANGE + 60.,
            "{near}..{far}"
        );
        assert!(swept > PI / 2., "{swept}");
    }
    // In all that time their fire did her less harm than one torpedo from each.
    assert!(
        (1..MOORED_PAIR as u16 * WARHEAD).contains(&gunfire),
        "{gunfire}"
    );
    // She slips away out of their sight. They search as far as where they last saw
    // her, then turn for home at full speed.
    at.player = Vec2::new(600., 2500.);
    let mut fastest_home = 0_f32;
    // Seconds the boats spent coming in over the last 60 m to their moorings.
    let mut coming_in = 0_f32;
    let mut home_at = None;
    while home_at.is_none() && t < 400. {
        step(&mut b, &at, &mut t, &mut judged);
        for r in boats(&b).filter(|r| r.moored_at().is_none()) {
            if r.body.pos.distance(base) > 120. {
                fastest_home = fastest_home.max(r.motion().length());
            }
            let (_, mooring) = r.torpedo_mooring().unwrap();
            if r.body.pos.distance(mooring) < 60. {
                coming_in += DT;
            }
        }
        if boats(&b).all(|r| r.moored_at().is_some()) {
            home_at = Some(t);
            // Home, but empty until she has reloaded.
            assert!(boats(&b).all(|r| !r.torpedo_aboard()));
        }
    }
    let home = home_at.expect("both boats home");
    assert!(fastest_home > 10., "{fastest_home}");
    // She keeps way on to steer for her berth, rather than creeping about off it.
    assert!(coming_in < 2. * 10., "{coming_in} s between the pair");
    for r in boats(&b) {
        let (_, mooring) = r.torpedo_mooring().unwrap();
        assert!(r.body.pos.distance(mooring) < ALONGSIDE.1);
    }
    // She comes back in sight of the base the moment they are in. They go out again
    // with both tubes loaded, but not before they have reloaded.
    at.player = Vec2::new(520., -1000.);
    let mut away_again = None;
    while away_again.is_none() && t < home + 120. {
        step(&mut b, &at, &mut t, &mut judged);
        if boats(&b).any(|r| r.moored_at().is_none()) {
            away_again = Some(t);
            assert!(boats(&b).all(|r| r.attack.as_ref().unwrap().torpedoes == TORPEDOES));
        }
    }
    let again = away_again.expect("out again");
    assert!(
        again >= home + RELOAD - 0.1,
        "left after {} s",
        again - home
    );
    // Each run, each boat misjudges her by her own few metres along her course.
    assert!(judged.len() >= 4, "{judged:?}");
    assert!(judged.iter().all(|m| m.abs() <= AIM_ERROR), "{judged:?}");
    assert!(judged.windows(2).all(|w| w[0] != w[1]), "{judged:?}");
}

#[test]
fn a_torpedo_runs_straight_misses_a_ship_that_turns_away_and_sinks_at_the_end_of_its_run() {
    let mut b = sector(&[(Vec2::new(4000., 4000.), 0)]);
    b.hold_torpedo_boats();
    // Fired at a ship 300 m off, northbound, from her starboard beam.
    let mut at = orders(Vec2::new(0., -1000.), 0., 7.);
    let tube = at.player + Vec2::new(300., 0.);
    let way = direction(at.heading) * at.speed;
    let aim = intercept(tube, at.player, way, TORPEDO_SPEED).unwrap();
    b.fire_torpedo(tube, bearing(aim - tube), 99);
    let heading = b.torpedoes[0].body.heading;
    // She comes hard round to port, away from it, as soon as she sees it.
    let mut blows = 0;
    let mut steps = 0;
    while !b.torpedoes.is_empty() {
        blows += b.step(DT, &at).blows.len();
        at.heading = (at.heading - 0.3 * DT).max(-PI / 2.);
        at.player += direction(at.heading) * at.speed * DT;
        if let Some(fish) = b.torpedoes.first() {
            assert!(
                (fish.body.heading - heading).abs() < 1e-6,
                "it does not seek her"
            );
        }
        steps += 1;
        assert!(steps < 20 * 30, "the torpedo ran on");
    }
    assert_eq!(blows, 0);
    // It ran its full range before it sank.
    assert!((steps as f32 * DT * TORPEDO_SPEED - TORPEDO_RANGE).abs() < 2.);

    // The same shot at a ship that holds her course strikes her.
    let mut b = sector(&[(Vec2::new(4000., 4000.), 0)]);
    b.hold_torpedo_boats();
    let mut at = orders(Vec2::new(0., -1000.), 0., 7.);
    b.fire_torpedo(tube, bearing(aim - tube), 99);
    let events = sail(&mut b, &mut at, 12.);
    assert_eq!(events.iter().map(|e| e.torpedoed).sum::<u16>(), 1);
    assert!(b.torpedoes.is_empty());
}

#[test]
fn an_enemy_torpedo_passes_under_its_own_side_but_land_stops_it() {
    let mut b = sector(&[(Vec2::new(4000., 4000.), 0)]);
    b.hold_torpedo_boats();
    let mut at = orders(Vec2::new(0., -1000.), 0., 0.);
    let tube = at.player + Vec2::new(300., 0.);
    // A destroyer lies across its track, just off the tube.
    let between = tube - Vec2::new(40., 0.);
    b.spawn(EnemyClass::Destroyer, between, between + Vec2::NEG_Y);
    let destroyer = |b: &Battle| {
        let found = b.raiders.iter().find(|r| r.class == EnemyClass::Destroyer);
        (found.unwrap().hull(), found.unwrap().body.id)
    };
    let (hull, id) = destroyer(&b);
    b.fire_torpedo(tube, bearing(at.player - tube), 99);
    let (mut crossed, mut torpedoed) = (false, 0);
    for _ in 0..(10. / DT) as usize {
        let before = b.torpedoes.first().map(|t| t.body.pos);
        torpedoed += b.step(DT, &at).torpedoed;
        if let (Some(from), Some(to)) = (before, b.torpedoes.first().map(|t| t.body.pos)) {
            crossed |= b.ship(id).unwrap().hull_span(from, to).is_some();
        }
    }
    assert!(crossed, "the torpedo never ran under her");
    assert_eq!(destroyer(&b), (hull, id));
    assert_eq!(torpedoed, 1);
    // A base's island in the way takes it.
    let island = b.ports[0].body.pos;
    b.fire_torpedo(island + Vec2::new(200., 0.), bearing(-Vec2::X), 99);
    let events = sail(&mut b, &mut at, 12.);
    assert!(b.torpedoes.is_empty());
    assert_eq!(events.iter().map(|e| e.torpedoed).sum::<u16>(), 0);
}

#[test]
fn a_sleeping_base_sends_none_and_a_wrecked_one_recalls_its_boats_and_sends_no_more() {
    let mut b = sector(&[(Vec2::new(0., -1000.), 0), (Vec2::new(-900., -1000.), 1)]);
    let mut at = orders(Vec2::new(520., -1000.), 0., 0.);
    sail(&mut b, &mut at, FIRST_RUN + 5.);
    let out = |b: &Battle, base: usize| {
        boats(b)
            .filter(|r| r.moored_at().is_none() && r.torpedo_mooring().unwrap().0 == base)
            .count()
    };
    assert_eq!(out(&b, 0), MOORED_PAIR);
    // The second base sleeps, and its pair rides at anchor whatever it hears.
    assert!(!b.ports[1].roused());
    assert_eq!(out(&b, 1), 0);
    // Wreck the first base with its boats out: they turn for home with their
    // torpedoes still aboard, moor there and stay.
    b.ports[0].wreck();
    // Its guns fight on apart from the yard; silence them to hear the boats alone.
    b.ports[0].silence();
    let recalled: Vec<u64> = (boats(&b))
        .filter(|r| r.torpedo_mooring().unwrap().0 == 0)
        .map(|r| r.body.id)
        .collect();
    let mut shots = 0;
    let mut scrambled = false;
    for _ in 0..(150. / DT) as usize {
        let before: Vec<u64> = hostile(&b).map(|t| t.body.id).collect();
        b.step(DT, &at);
        assert!(
            (boats(&b).filter(|r| recalled.contains(&r.body.id)))
                .all(|r| !r.running_in() && !r.harrying())
        );
        shots += (hostile(&b))
            .filter(|t| !before.contains(&t.body.id))
            .filter(|t| t.fired_by.is_some_and(|id| recalled.contains(&id)))
            .count();
        scrambled |= out(&b, 1) == MOORED_PAIR;
    }
    assert_eq!(shots, 0, "a recalled boat fires no torpedo");
    assert_eq!(out(&b, 0), 0);
    assert!(
        boats(&b)
            .filter(|r| r.torpedo_mooring().unwrap().0 == 0)
            .all(|r| {
                let (_, mooring) = r.torpedo_mooring().unwrap();
                r.body.pos.distance(mooring) < ALONGSIDE.1
            })
    );
    // The second stage is roused now that the first has fallen, and scrambled: it
    // sends its pair out at her.
    assert!(b.ports[1].roused());
    assert!(scrambled);
}

#[test]
fn no_more_than_four_are_out_and_each_base_sends_one_pair_at_a_time() {
    let mut b = sector(&[
        (Vec2::new(-700., -1000.), 0),
        (Vec2::new(0., -1000.), 0),
        (Vec2::new(700., -1000.), 0),
    ]);
    // North of the three bases, lying stopped beam on to them.
    let at = orders(Vec2::new(0., -1500.), PI / 2., 0.);
    let mut most = 0;
    let mut departures: Vec<Vec<f32>> = vec![Vec::new(); 3];
    let mut out_before: Vec<bool> = vec![false; 3];
    let mut sunk_at = None;
    let mut t = 0.;
    // Until just before the base that lost its pair sends their replacements.
    while t < 100. + REPLACEMENT - 1. {
        b.step(DT, &at);
        t += DT;
        most = most.max(b.torpedo_boats_out());
        assert!(b.torpedo_boats_out() <= MOST_OUT);
        for base in 0..3 {
            let out = boats(&b)
                .any(|r| r.moored_at().is_none() && r.torpedo_mooring().unwrap().0 == base);
            if out && !out_before[base] {
                departures[base].push(t);
            }
            out_before[base] = out;
        }
        // The four out stay to harry her, so the third base waits until two of
        // them are sunk.
        if t >= 100. && sunk_at.is_none() {
            assert_eq!(b.torpedo_boats_out(), MOST_OUT);
            let sent = (0..3).find(|&base| !departures[base].is_empty()).unwrap();
            for boat in b.raiders.iter_mut().filter(|r| {
                r.moored_at().is_none() && r.torpedo_mooring().is_some_and(|(home, _)| home == sent)
            }) {
                let at = boat.body.pos;
                boat.hit_hull(at, 99);
            }
            sunk_at = Some(t);
        }
    }
    assert_eq!(most, MOST_OUT);
    let sunk = sunk_at.unwrap();
    // Two bases sent their pairs at once. The third had to wait for room, and sent
    // its pair as soon as there was.
    let first: Vec<f32> = departures
        .iter()
        .filter_map(|d| d.first().copied())
        .collect();
    assert_eq!(first.len(), 3, "{departures:?}");
    assert!(
        first.iter().filter(|&&t| t < FIRST_RUN + 1.).count() == 2,
        "{departures:?}"
    );
    assert!(
        first.iter().any(|&t| t > sunk && t < sunk + 2.),
        "{departures:?} after {sunk}"
    );
    for runs in &departures {
        for pair in runs.windows(2) {
            assert!(pair[1] - pair[0] >= RUN_INTERVAL - 0.1, "{departures:?}");
        }
    }
}

#[test]
fn a_boat_lost_is_replaced_off_her_base_slip_but_not_by_a_wrecked_base() {
    let mut b = sector(&[(Vec2::new(0., -1000.), 0), (Vec2::new(1200., -1000.), 0)]);
    b.hold_torpedo_boats();
    let far = orders(Vec2::new(600., 2000.), 0., 0.);
    let sink = |b: &mut Battle, base: usize| {
        let boat = b
            .raiders
            .iter_mut()
            .find(|r| r.torpedo_mooring().is_some_and(|(home, _)| home == base))
            .unwrap();
        let (_, mooring) = boat.torpedo_mooring().unwrap();
        let at = boat.body.pos;
        boat.hit_hull(at, 99);
        mooring
    };
    let vacant = sink(&mut b, 0);
    sink(&mut b, 1);
    b.ports[1].wreck();
    let mut at = far;
    let events = sail(&mut b, &mut at, 1.);
    assert_eq!(events.iter().map(|e| e.torpedo_boats).sum::<u16>(), 2);
    let count = |b: &Battle, base: usize| {
        boats(b)
            .filter(|r| r.torpedo_mooring().unwrap().0 == base)
            .count()
    };
    assert_eq!((count(&b, 0), count(&b, 1)), (1, 1));
    sail(&mut b, &mut at, REPLACEMENT - 2.);
    assert_eq!(count(&b, 0), 1, "not yet");
    sail(&mut b, &mut at, 3.);
    assert_eq!((count(&b, 0), count(&b, 1)), (2, 1));
    // The new boat came off the slip, and goes to lie where the lost one lay.
    let spare = boats(&b)
        .filter(|r| r.torpedo_mooring().unwrap().0 == 0)
        .max_by_key(|r| r.body.id)
        .unwrap();
    assert!(spare.body.pos.distance(b.ports[0].berth()) < 45.);
    assert_eq!(spare.torpedo_mooring().unwrap().1, vacant);
    sail(&mut b, &mut at, 40.);
    let spare = boats(&b)
        .filter(|r| r.torpedo_mooring().unwrap().0 == 0)
        .max_by_key(|r| r.body.id)
        .unwrap();
    assert_eq!(spare.moored_at(), Some(0));
    assert!(spare.body.pos.distance(vacant) < ALONGSIDE.1);
    assert!(spare.torpedo_aboard());
    sail(&mut b, &mut at, 120.);
    assert_eq!(count(&b, 1), 1);
}

#[test]
fn the_aim_leads_her_and_a_track_is_closing_only_if_it_will_pass_close_aboard() {
    let (ship, way) = (Vec2::new(0., -1000.), Vec2::new(0., -7.));
    let tube = ship + Vec2::new(300., 0.);
    let aim = intercept(tube, ship, way, TORPEDO_SPEED).unwrap();
    // On her track ahead of her, where the torpedo and she arrive together.
    assert!(aim.x.abs() < 1e-3 && aim.y < ship.y);
    let seconds = aim.distance(tube) / TORPEDO_SPEED;
    assert!((ship + way * seconds).distance(aim) < 0.01);
    // A ship faster than the torpedo running straight away from it is never caught.
    assert!(intercept(tube, ship, Vec2::new(-45., 0.), TORPEDO_SPEED).is_none());
    let mut b = sector(&[(Vec2::new(4000., 4000.), 0)]);
    b.fire_torpedo(tube, bearing(aim - tube), 99);
    let fish = &b.torpedoes[0];
    let closing = fish.closing(ship, way, 6.).unwrap();
    assert!(
        (closing - seconds).abs() < 0.05,
        "{closing} against {seconds}"
    );
    // Had she stopped, it would pass 56 m ahead of her.
    assert!(fish.closing(ship, Vec2::ZERO, 20.).is_none());
    assert!(fish.closing(ship, Vec2::ZERO, 60.).is_some());
}
