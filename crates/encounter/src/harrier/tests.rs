use super::mooring::REACH;
use super::*;
use crate::projectile::FlightPath;
use crate::{
    Aim, Airframe, AntiAir, Chart, EffectKind, Events, HarbourSite, Hull, MainShell, Orders,
};

/// Three canneries far apart in open water, in the given stages, with nothing else at
/// sea and no AA guns aboard.
fn sector(stages: [u8; 3]) -> Battle {
    let mut chart = Chart::classic([
        Vec2::new(0., -152.),
        Vec2::new(168., -424.),
        Vec2::new(496., -300.),
        Vec2::new(416., 76.),
        Vec2::new(72., 164.),
    ]);
    chart.islets.clear();
    chart.harbours = (0..3)
        .map(|i| HarbourSite {
            pos: Vec2::X * (i as f32 * 3000.),
            heading: 0.,
            works: 1,
            stage: stages[i],
        })
        .collect();
    let mut b = Battle::for_chart(1000., &chart);
    b.raiders.clear();
    b.fit_anti_air(AntiAir::NONE);
    b
}

/// Lying stopped `range` metres out in front of base `index`, beyond its guns.
fn hove_to(b: &Battle, index: usize, range: f32) -> Orders {
    let port = &b.ports[index];
    Orders {
        player: port.body.pos + port.body.forward() * range,
        heading: 0.,
        speed: 0.,
        hull: Hull::CRUISER,
        aim: Aim::Hold,
        reload: 6.4,
        shell: MainShell::STOCK,
    }
}

/// Step the battle for `seconds`, keeping the sea clear of ships.
fn sail(b: &mut Battle, orders: &Orders, seconds: f32, mut each: impl FnMut(&Battle, &Events)) {
    let dt = 1. / 60.;
    for _ in 0..(seconds * 60.).round() as usize {
        b.raiders.clear();
        let events = b.step(dt, orders);
        each(b, &events);
    }
}

#[test]
fn a_roused_base_sends_up_its_formation_from_its_mooring_once_its_interval_is_out() {
    let mut b = sector([0, 1, 1]);
    let orders = hove_to(&b, 0, 700.);
    assert!((0..3).all(|i| b.harriers_moored(i)));
    sail(&mut b, &orders, FIRST_SORTIE - 0.1, |_, _| {});
    assert!(b.harriers().is_empty(), "none before {FIRST_SORTIE} s");
    sail(&mut b, &orders, 0.2, |_, _| {});
    // A pair from the roused base at the mouth, off the water beside its mooring. The
    // sleeping bases keep theirs moored.
    assert_eq!(b.harriers().len(), 2);
    for plane in b.harriers() {
        assert_eq!(plane.base, 0);
        let mooring = b.ports[0].harrier_mooring(plane.slot);
        assert!(
            plane.body.pos.distance(mooring) < 10.,
            "{:?}",
            plane.body.pos
        );
        assert!(plane.altitude < 5.);
    }
    assert!(!b.harriers_moored(0) && b.harriers_moored(1) && b.harriers_moored(2));
    let mooring = |b: &Battle, slot: usize| b.ports[0].harrier_mooring(slot);
    for slot in 0..LAST_FORMATION {
        assert!(mooring(&b, slot).distance(b.ports[0].body.pos) > crate::ISLAND_RADIUS + 8.);
        for other in 0..slot {
            assert!(mooring(&b, slot).distance(mooring(&b, other)) > 8.);
        }
    }
    // The other two stir, and the last stage keeps five. One more formation fills the
    // sky, and the third waits for room for all of its own.
    let now = b.at_sea;
    for port in &mut b.ports[1..] {
        port.rouse(now);
    }
    sail(&mut b, &orders, 0.1, |_, _| {});
    assert_eq!(
        (b.formation(1), b.formation(2)),
        (LAST_FORMATION, LAST_FORMATION)
    );
    assert_eq!(b.harriers().len(), 2 + LAST_FORMATION);
    assert!(b.harriers().len() + LAST_FORMATION > MOST_ALOFT);
    let from = |b: &Battle, base: usize| b.harriers().iter().filter(|p| p.base == base).count();
    assert_eq!((from(&b, 1), from(&b, 2)), (LAST_FORMATION, 0));
    // A wrecked base sends up nothing, though what lay at its mooring lies there still.
    b.ports[2].wreck();
    assert!(!b.harriers_moored(2));
    let mut launched = vec![];
    sail(&mut b, &orders, 240., |b, _| {
        for plane in b.harriers() {
            if !launched.contains(&plane.body.id) {
                launched.push(plane.body.id);
                assert!(!b.ports[plane.base].wrecked());
                assert_ne!(plane.base, 2);
            }
        }
        assert!(b.harriers().len() <= MOST_ALOFT);
    });
    // Each base sent up another formation once its own had come home.
    assert!(
        launched.len() > 4,
        "{} more aloft in four minutes",
        launched.len()
    );
}

#[test]
fn a_pair_strafes_in_short_passes_circles_out_between_and_alights_after_three() {
    let mut b = sector([0, 1, 1]);
    let orders = hove_to(&b, 0, 700.);
    sail(&mut b, &orders, FIRST_SORTIE + 0.1, |_, _| {});
    let pair: Vec<u64> = b.harriers().iter().map(|p| p.body.id).collect();
    assert_eq!(pair.len(), 2);
    let mut rounds = 0;
    let mut hurt = Events::default();
    let mut farthest: f32 = 0.;
    let mut pass_heights = vec![];
    let mut last_seen: Vec<(u64, usize, Vec2)> = vec![];
    let mut home = false;
    sail(&mut b, &orders, 150., |b, events| {
        // The base sends up its next pair only once this one is home.
        if home {
            return;
        }
        rounds += b
            .effects()
            .iter()
            .filter(|fx| fx.age == 0. && fx.kind == EffectKind::Muzzle(Battery::Aircraft))
            .count();
        hurt.damage += events.damage;
        hurt.blows.extend(&events.blows);
        let ours: Vec<&Harrier> = b
            .harriers()
            .iter()
            .filter(|p| pair.contains(&p.body.id))
            .collect();
        home = ours.is_empty();
        for plane in ours {
            if plane.passes() > 0 && !plane.homebound() {
                farthest = farthest.max(plane.body.pos.distance(orders.player));
            }
            if matches!(plane.leg, Leg::Pass { .. }) {
                pass_heights.push(plane.altitude);
            }
            last_seen.retain(|(id, _, _)| *id != plane.body.id);
            last_seen.push((plane.body.id, plane.slot, plane.body.pos));
        }
    });
    assert!(home, "the pair came home within the time");
    assert_eq!(
        rounds,
        2 * usize::from(PASSES) * ROUNDS,
        "three rounds a pass each"
    );
    // They lead her and walk their bursts across her: a nuisance, a point a round.
    assert!(
        hurt.damage > 0 && hurt.damage <= rounds as u16,
        "{}",
        hurt.damage
    );
    assert!(hurt.blows.iter().all(|blow| blow.damage <= ROUND_DAMAGE));
    assert!(pass_heights.iter().all(|&h| (11. ..=14.).contains(&h)));
    assert!(
        (260. ..450.).contains(&farthest),
        "circled out to {farthest:.0} m"
    );
    // Each alights at its own mooring.
    assert_eq!(last_seen.len(), 2);
    for (_, slot, at) in last_seen {
        assert!(at.distance(b.ports[0].harrier_mooring(slot)) < 3., "{at:?}");
    }
}

#[test]
fn formations_grow_from_pairs_at_the_mouth_to_five_at_the_heavy_yard() {
    let sizes = |stages: [u8; 3]| {
        let b = sector(stages);
        [0, 1, 2].map(|i| b.formation(i))
    };
    assert_eq!(sizes([0, 0, 0]), [2, 2, 2]);
    assert_eq!(sizes([0, 1, 2]), [2, 3, 5]);
    assert_eq!(sizes([0, 0, 1]), [2, 2, 5]);
    // A formation flies abreast, centred on its leader's line, a spacing apart.
    for size in FIRST_FORMATION..=LAST_FORMATION {
        let mut sides: Vec<f32> = (0..size).map(|slot| formation_side(slot, size)).collect();
        assert!(sides.iter().sum::<f32>().abs() < 1e-4, "{sides:?}");
        sides.sort_by(f32::total_cmp);
        assert!(
            sides.windows(2).all(|w| (w[1] - w[0] - 2.).abs() < 1e-4),
            "{sides:?}"
        );
    }
    assert_eq!([0, 1].map(|slot| formation_side(slot, 2)), [-1., 1.]);
}

#[test]
fn three_aa_hits_bring_a_harrier_down_and_each_one_downed_is_counted() {
    let mut b = sector([0, 1, 1]);
    let orders = hove_to(&b, 0, 700.);
    sail(&mut b, &orders, FIRST_SORTIE + 3., |_, _| {});
    let plane = &b.harriers()[0];
    let (id, at) = (plane.body.id, plane.position_3d());
    let wingman = b.harriers()[1].position_3d();
    assert!(
        wingman.distance(at) > crate::BURST_RADIUS,
        "the pair fly apart"
    );
    let mut events = Events::default();
    for _ in 0..2 {
        b.burst_flak(at, 1, &mut events);
        assert!(b.harriers().iter().any(|p| p.body.id == id));
        assert_eq!(events.planes_downed, 0);
    }
    b.burst_flak(at, 1, &mut events);
    assert!(b.harriers().iter().all(|p| p.body.id != id));
    assert_eq!(events.planes_downed, 1);
    let falling = b
        .effects()
        .iter()
        .filter(|fx| fx.kind == EffectKind::Downed(Airframe::Harrier))
        .count();
    assert_eq!(falling, 1);
    // The stock guns bring the rest down as they come in, and count each.
    b.fit_anti_air(AntiAir::STOCK);
    let mut downed = 0;
    let mut falls = 0;
    sail(&mut b, &orders, 60., |b, events| {
        downed += events.planes_downed;
        falls += b
            .effects()
            .iter()
            .filter(|fx| fx.age == 0. && fx.kind == EffectKind::Downed(Airframe::Harrier))
            .count();
    });
    assert!(downed > 0, "the stock pair hit nothing");
    assert_eq!(usize::from(downed), falls);
}

#[test]
fn wrecking_their_base_sends_its_pair_home_and_it_sends_no_more() {
    let mut b = sector([0, 1, 1]);
    let orders = hove_to(&b, 0, 700.);
    // The later bases are already down, so wrecking the first stirs nobody.
    for port in &mut b.ports[1..] {
        port.wreck();
    }
    sail(&mut b, &orders, FIRST_SORTIE + 8., |_, _| {});
    assert!(b.harriers().iter().all(|p| p.attacking()));
    b.ports[0].wreck();
    sail(&mut b, &orders, 0.1, |_, _| {});
    assert_eq!(b.harriers().len(), 2);
    assert!(b.harriers().iter().all(Harrier::homebound));
    let mut rounds = 0;
    sail(&mut b, &orders, 90., |b, _| {
        rounds += b
            .effects()
            .iter()
            .filter(|fx| fx.age == 0. && fx.kind == EffectKind::Muzzle(Battery::Aircraft))
            .count();
    });
    assert_eq!(rounds, 0, "they fire nothing on the way home");
    assert!(b.harriers().is_empty());
}

/// Where the harrier in `slot` of base 0's mooring lies, if one does.
fn lying(b: &Battle, slot: usize) -> Option<&MooredHarrier> {
    (b.moored_harriers().iter()).find(|plane| plane.base == 0 && plane.slot == slot)
}

/// Step with her coming on at `speed` along `heading` from `orders.player`, moved by
/// the shoves her hull takes as the game moves her, until `done` or `seconds` are out.
fn steam(
    b: &mut Battle,
    orders: &mut Orders,
    seconds: f32,
    mut done: impl FnMut(&Battle, &Events) -> bool,
) {
    let dt = 1. / 60.;
    for _ in 0..(seconds * 60.).round() as usize {
        b.raiders.clear();
        let events = b.step(dt, orders);
        orders.player +=
            direction(orders.heading) * (orders.speed + events.jolt) * dt + events.shove;
        if done(b, &events) {
            return;
        }
    }
}

#[test]
fn her_stem_shoves_a_moored_harrier_aside_at_a_crawl_and_runs_it_down_at_speed() {
    let mut b = sector([0, 1, 1]);
    b.ground_harriers();
    let everyone: usize = (0..3).map(|i| b.formation(i)).sum();
    assert_eq!(b.moored_harriers().len(), everyone);
    let first = lying(&b, 0).unwrap();
    let (id, berth) = (first.body.id, first.body.pos);
    // She comes at it square across the rows, from outside the berths on its side.
    let across = b.ports[0].body.forward().perp();
    let gap = Hull::CRUISER.bow + 1.8 + 3.;
    let mut orders = hove_to(&b, 0, 0.);
    orders.player = berth - across * gap;
    orders.heading = bearing(across);
    orders.speed = 0.5;
    let mut downed = 0;
    steam(&mut b, &mut orders, 14., |_, events| {
        downed += events.planes_downed;
        false
    });
    let shoved = lying(&b, 0).expect("shoved, not run down");
    assert_eq!(shoved.body.id, id);
    assert!(
        (shoved.body.pos - berth).dot(across) > 1.5,
        "pushed {:.1} m",
        (shoved.body.pos - berth).dot(across)
    );
    assert!(lying(&b, 1).is_some());
    assert_eq!(downed, 0);
    assert!(
        b.effects()
            .iter()
            .all(|fx| !matches!(fx.kind, EffectKind::Downed(_)))
    );
    // Coming on at eight knots, she runs it down.
    orders.speed = 4.;
    steam(&mut b, &mut orders, 4., |b, events| {
        downed += events.planes_downed;
        lying(b, 0).is_none()
    });
    assert!(lying(&b, 0).is_none());
    assert_eq!(downed, 1);
    let wreck = (b.effects().iter())
        .find(|fx| fx.kind == EffectKind::Downed(Airframe::Harrier))
        .expect("its wreck lies on the water");
    assert_eq!(wreck.height, 0.);
    assert!(wreck.body.pos.distance(berth) < 8.);
    assert!(b.effects().iter().any(|fx| fx.kind == EffectKind::Crunch));
    // Its berth stands empty until the yard has another ready, while she lies off.
    orders.player = b.ports[0].body.pos + b.ports[0].body.forward() * 400.;
    orders.speed = 0.;
    steam(&mut b, &mut orders, REFIT - 1., |_, _| false);
    assert!(lying(&b, 0).is_none());
    steam(&mut b, &mut orders, 2., |_, _| false);
    let fresh = lying(&b, 0).expect("refitted");
    assert_ne!(fresh.body.id, id);
    assert!(fresh.body.pos.distance(b.ports[0].harrier_mooring(0)) < 0.1);
}

#[test]
fn a_round_that_strikes_a_moored_harrier_or_bursts_beside_it_wrecks_it() {
    let mut b = sector([0, 1, 1]);
    b.ground_harriers();
    let target = lying(&b, 0).unwrap().body.pos;
    let across = b.ports[0].body.forward().perp();
    // Laid on from her bridge out on its side, the sight passes over it to what lies
    // beyond. The main guns never engage a harrier, so it can't snatch the sight.
    let eye = (target - across * 200.).extend(12.).xzy();
    let ray = (target.extend(0.7).xzy() - eye).normalize();
    let sea = eye.xz() + ray.xz() * (eye.y / -ray.y);
    let sight = b.sight(eye, ray, sea);
    assert!(
        sight.point.xz().distance(eye.xz()) > target.distance(eye.xz()) + REACH,
        "{}",
        sight.point
    );
    let fire_at = |b: &mut Battle, mark: Vec3| {
        let mount = (mark.xz() - across * 200.).extend(5.).xzy();
        let path = FlightPath::to(mount, mark, Battery::Main).unwrap();
        path.launch(
            &mut b.shots,
            &mut b.effects,
            &mut b.serial,
            Battery::Main,
            10,
        );
        let mut events = Events::default();
        while !b.shots.is_empty() {
            b.advance_shots(1. / 60., Vec2::ZERO, &mut events);
        }
        events.planes_downed
    };
    // A round straight at it.
    assert_eq!(fire_at(&mut b, target.extend(0.7).xzy()), 1);
    assert!(lying(&b, 0).is_none());
    // A round into the sea three metres wide of the next, clear of the rest.
    let beside = lying(&b, 1).unwrap().body.pos - b.ports[0].body.forward() * 3.;
    assert_eq!(fire_at(&mut b, beside.extend(0.).xzy()), 1);
    assert!(lying(&b, 1).is_none());
    assert!(b.effects().iter().any(|fx| fx.kind == EffectKind::Splash));
    let everyone: usize = (0..3).map(|i| b.formation(i)).sum();
    assert_eq!(b.moored_harriers().len(), everyone - 2);
}

#[test]
fn a_formation_takes_off_from_where_it_was_shoved_and_comes_home_to_its_berths() {
    let mut b = sector([0, 1, 1]);
    let orders = hove_to(&b, 0, 700.);
    let drift = b.ports[0].body.forward() * 9.;
    let shoved = (b.moorings.planes.iter_mut())
        .find(|plane| plane.base == 0 && plane.slot == 1)
        .unwrap();
    shoved.body.pos += drift;
    let from = shoved.body.pos;
    sail(&mut b, &orders, FIRST_SORTIE + 0.1, |_, _| {});
    let plane = b.harriers().iter().find(|plane| plane.slot == 1).unwrap();
    assert!(plane.body.pos.distance(from) < 5., "{:?}", plane.body.pos);
    assert!(!b.harriers_moored(0) && lying(&b, 0).is_none() && lying(&b, 1).is_none());
    let mut home = false;
    sail(&mut b, &orders, 150., |b, _| {
        if !home && b.harriers().iter().all(|plane| plane.base != 0) {
            home = true;
            for slot in 0..2 {
                let at = lying(b, slot).expect("lying at the mooring").body.pos;
                assert!(at.distance(b.ports[0].harrier_mooring(slot)) < 3., "{at:?}");
            }
        }
    });
    assert!(home);
}
