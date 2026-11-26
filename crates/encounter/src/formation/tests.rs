//! Squadrons in close order and in line of battle.
use super::*;
use crate::{Aim, Chart, HarbourSite, MainShell, Orders};
use EnemyClass::*;

/// Where the ship lies: far from every base, so no lookout ashore sees her.
const HER: Vec2 = Vec2::new(2500., 3000.);

/// Open water with three bases far apart, and nothing afloat.
fn open_sea() -> Battle {
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
            pos: Vec2::X * (i as f32 * 5000.),
            heading: 0.,
            works: 1,
            stage: 0,
        })
        .collect();
    let mut b = Battle::for_chart(1000., &chart);
    b.raiders.clear();
    b.squadrons.clear();
    b
}

fn lying_still(at: Vec2) -> Orders {
    Orders {
        player: at,
        heading: 0.,
        speed: 0.,
        hull: Hull::CRUISER,
        aim: Aim::Hold,
        reload: 3.2,
        shell: MainShell::STOCK,
    }
}

/// A squadron of `classes`, heaviest first, in close order on its guide at `at`,
/// heading along `course`. Returns their ids in that order.
fn squadron(b: &mut Battle, classes: &[EnemyClass], at: Vec2, course: Vec2) -> Vec<u64> {
    let hulls: Vec<Hull> = classes.iter().map(|c| c.profile().hull).collect();
    let places = std::iter::once((0., 0.)).chain(close_order(&hulls));
    let ids: Vec<u64> = classes
        .iter()
        .zip(places)
        .map(|(&class, (ahead, starboard))| {
            let spot = at + course * ahead + course.perp() * starboard;
            b.spawn(class, spot, spot + course);
            b.serial
        })
        .collect();
    b.form_squadron(ids.clone());
    ids
}

fn ship(b: &Battle, id: u64) -> &Raider {
    b.ship(id).unwrap()
}

#[test]
fn escorts_take_the_guides_quarters_outward_in_turn_and_never_overlap() {
    let hulls: Vec<Hull> = [Battleship, Cruiser, Cruiser, Destroyer, Destroyer]
        .map(|c| c.profile().hull)
        .to_vec();
    let order = close_order(&hulls);
    assert_eq!(order.len(), 4);
    // Starboard, then port, then each a rank further out and further astern.
    assert!(order[0].1 > 0. && order[1].1 < 0.);
    assert!(order[2].1 > order[0].1 && order[3].1 < order[1].1);
    assert!(order[2].0 < order[0].0 && order[3].0 < order[1].0);
    assert!(order.iter().all(|&(ahead, _)| ahead < 0.));
    let places: Vec<Vec2> = std::iter::once(Vec2::ZERO)
        .chain(
            order
                .iter()
                .map(|&(ahead, starboard)| Vec2::new(starboard, ahead)),
        )
        .collect();
    for i in 0..places.len() {
        for j in i + 1..places.len() {
            let room = hulls[i].bow.max(hulls[i].stern) + hulls[j].bow.max(hulls[j].stern);
            assert!(places[i].distance(places[j]) > room, "{i} and {j} overlap");
        }
    }
}

#[test]
fn a_squadron_on_passage_keeps_close_order_at_the_pace_of_its_slowest() {
    let mut b = open_sea();
    let classes = [Cruiser, Destroyer, Destroyer];
    let ids = squadron(&mut b, &classes, HER + Vec2::new(2600., 0.), Vec2::NEG_X);
    // Nobody has word of her, so the squadron makes for its patrol ground together.
    let orders = lying_still(HER);
    let mut farthest = 0_f32;
    for step in 0..1500 {
        b.step(0.1, &orders);
        if step < 600 {
            continue;
        }
        let guide = ship(&b, ids[0]);
        let (at, forward) = (guide.body.pos, guide.body.forward());
        let hulls: Vec<Hull> = classes.iter().map(|c| c.profile().hull).collect();
        for (&id, (ahead, starboard)) in ids[1..].iter().zip(close_order(&hulls)) {
            let station = at + forward * ahead + forward.perp() * starboard;
            farthest = farthest.max(ship(&b, id).body.pos.distance(station));
        }
    }
    assert!(
        farthest < 45.,
        "an escort fell {farthest:.0} m off her station"
    );
    assert!(ids.iter().all(|&id| !ship(&b, id).has_contact));
    let Some(Order::Guide { pace, ring: None }) = ship(&b, ids[0]).order else {
        panic!("the cruiser leads, in close order");
    };
    assert!((pace - SQUADRON_PACE).abs() < 1e-4);
    // A patrol boat holds a destroyer back to her own pace.
    let mut slow = open_sea();
    let pair = squadron(
        &mut slow,
        &[Destroyer, Patrol],
        HER + Vec2::X * 3000.,
        Vec2::X,
    );
    slow.step(0.1, &orders);
    let Some(Order::Guide { pace, .. }) = ship(&slow, pair[0]).order else {
        panic!("the destroyer leads");
    };
    let expected = SQUADRON_PACE * Patrol.profile().speed() / Destroyer.profile().speed();
    assert!((pace - expected).abs() < 1e-4, "{pace}");
}

#[test]
fn in_reach_of_her_a_squadron_fans_out_into_a_line_of_battle_broadside_on() {
    let mut b = open_sea();
    let classes = [Battleship, Cruiser, Cruiser, Destroyer, Destroyer];
    let ids = squadron(&mut b, &classes, HER + Vec2::X * 760., Vec2::NEG_X);
    let orders = lying_still(HER);
    for _ in 0..2400 {
        b.step(0.1, &orders);
    }
    assert!(b.squadrons().all(|(_, deployed)| deployed));
    let bearing = |id: u64| {
        let off = ship(&b, id).body.pos - HER;
        off.y.atan2(off.x)
    };
    // The guide wheels about her at her fighting range, and the rest at theirs.
    for &id in &ids {
        let r = ship(&b, id);
        let range = r.body.pos.distance(HER);
        let own = line_range(r.class);
        assert!(
            (own * 0.75..own * 1.3).contains(&range),
            "{:?} at {range:.0} m, her line lies at {own:.0} m",
            r.class
        );
    }
    // The line spreads across her bearing, the destroyers out on the wings.
    let guide = bearing(ids[0]);
    let turn = |angle: f32| (angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU);
    let spread: Vec<f32> = ids.iter().map(|&id| turn(bearing(id) - guide)).collect();
    let (least, most) = spread
        .iter()
        .fold((f32::MAX, f32::MIN), |(lo, hi), &a| (lo.min(a), hi.max(a)));
    assert!(
        most - least > 0.6,
        "the line spans only {:.2} rad",
        most - least
    );
    let wide = |id: u64| (bearing(id) - guide).sin().abs();
    assert!(wide(ids[3]) > wide(ids[1]) && wide(ids[4]) > wide(ids[2]));
    // Each lies broadside on to her, not bows on.
    for &id in &ids {
        let r = ship(&b, id);
        let towards = (HER - r.body.pos).normalize();
        assert!(
            r.body.forward().dot(towards).abs() < 0.6,
            "{:?} is not broadside on",
            r.class
        );
    }
}

#[test]
fn a_squadron_weighs_as_one_and_shares_what_any_ship_of_it_sees() {
    let mut b = open_sea();
    // Spread far enough that only the leading destroyer has her in sight.
    let ids = squadron(
        &mut b,
        &[Cruiser, Destroyer],
        HER + Vec2::X * 1500.,
        Vec2::X,
    );
    let near = HER + Vec2::X * 600.;
    let destroyer = b.raiders.iter_mut().find(|r| r.body.id == ids[1]).unwrap();
    destroyer.body.pos = near;
    for r in &mut b.raiders {
        r.moor(0);
    }
    b.update_lookouts(0.1, HER);
    assert!(ids.iter().all(|&id| ship(&b, id).has_contact));
    assert!(
        ids.iter()
            .all(|&id| ship(&b, id).last_sighting == Some(HER))
    );
    // One of them has weighed, so the rest weigh with her.
    b.raiders[0].cast_off();
    b.keep_formation(0.1, HER);
    assert!(ids.iter().all(|&id| ship(&b, id).moored_at().is_none()));
}

#[test]
fn the_next_heaviest_takes_the_lead_and_a_squadron_of_one_sails_alone() {
    let mut b = open_sea();
    let ids = squadron(
        &mut b,
        &[Battleship, Cruiser, Destroyer],
        HER + Vec2::X * 3000.,
        Vec2::X,
    );
    let orders = lying_still(HER);
    b.step(0.1, &orders);
    assert!(matches!(ship(&b, ids[0]).order, Some(Order::Guide { .. })));
    assert!(matches!(ship(&b, ids[1]).order, Some(Order::Keep(_))));
    b.raiders.retain(|r| r.body.id != ids[0]);
    b.step(0.1, &orders);
    assert!(matches!(ship(&b, ids[1]).order, Some(Order::Guide { .. })));
    assert!(matches!(ship(&b, ids[2]).order, Some(Order::Keep(_))));
    b.raiders.retain(|r| r.body.id != ids[1]);
    b.step(0.1, &orders);
    assert!(ship(&b, ids[2]).order.is_none());
    assert_eq!(b.squadrons().count(), 0);
}

#[test]
fn the_regent_sails_at_the_head_of_her_battle_fleet() {
    let chart = Chart::classic([
        Vec2::new(0., -152.),
        Vec2::new(168., -424.),
        Vec2::new(496., -300.),
        Vec2::new(416., 76.),
        Vec2::new(72., 164.),
    ]);
    let b = Battle::regent_encounter(14., &chart);
    let (fleet, _) = b.squadrons().next().expect("the Regent's squadron");
    let regent = b.raiders.iter().find(|r| r.class == Regent).unwrap();
    assert!(fleet.contains(&regent.body.id));
    assert_eq!(fleet.len(), 1 + super::super::REGENT_FLEET.len());
    let battleships = (b.raiders.iter())
        .filter(|r| fleet.contains(&r.body.id) && r.class == Battleship)
        .count();
    assert_eq!(battleships, 3);
    // She is in sight of it from the start, and none of it can reach her yet.
    assert!(regent.body.pos.length() < Regent.lookout_range());
    for r in &b.raiders {
        assert!(r.body.pos.length() > r.class.reach(), "{:?} in reach", r.class);
    }
}
