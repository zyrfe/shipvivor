use super::*;
use crate::{Aim, AntiAir, Chart, Hull, ISLAND_RADIUS, LandmarkKind, MainShell, Orders, Waters};

/// An open channel with a cannery at the mouth, an airfield between and the heavy yard at
/// the head, with no ships at sea and no AA guns aboard. The airfield base's index.
fn airfield_sector() -> (Battle, usize) {
    let waters = Waters::OPEN.staged(&[&[1], &[AIRFIELD_WORKS], &[3]]);
    let (chart, _) = (1..60)
        .find_map(|seed| Chart::generate(seed, &waters))
        .expect("a channel with an airfield charts");
    let mut b = Battle::for_chart(1000., &chart);
    b.raiders.clear();
    b.fit_anti_air(AntiAir::NONE);
    let i = (b.ports.iter().position(|p| p.airfield().is_some())).expect("its airfield is sited");
    (b, i)
}

/// Lying stopped `range` metres out from the airfield's seaward side.
fn off_the_field(b: &Battle, i: usize, range: f32) -> Orders {
    let field = b.ports[i].airfield().unwrap().field;
    Orders {
        player: field.centre + field.seaward * range,
        heading: 0.,
        speed: 0.,
        hull: Hull::CRUISER,
        aim: Aim::Hold,
        reload: 6.4,
        shell: MainShell::STOCK,
    }
}

fn sail(b: &mut Battle, orders: &Orders, seconds: f32, mut each: impl FnMut(&Battle, &Events)) {
    let dt = 1. / 60.;
    for _ in 0..(seconds * 60.).round() as usize {
        b.raiders.clear();
        let events = b.step(dt, orders);
        each(b, &events);
    }
}

fn field(b: &Battle, i: usize) -> &Airfield {
    b.ports[i].airfield().unwrap()
}
fn field_mut(b: &mut Battle, i: usize) -> &mut Airfield {
    b.ports[i].airfield.as_deref_mut().unwrap()
}

#[test]
fn an_airfield_stands_on_its_own_island_beside_its_harbour_with_its_works_guns_and_aircraft() {
    let (b, i) = airfield_sector();
    let port = &b.ports[i];
    let field = port.airfield().unwrap();
    // Its island is level ground of its own, clear of the harbour's.
    let island = (b.landmarks.iter())
        .find(|l| l.kind == LandmarkKind::Airfield)
        .unwrap();
    assert_eq!(island.body.pos, field.field.centre);
    assert!(island.body.pos.distance(port.body.pos) >= AIRFIELD_RADIUS + ISLAND_RADIUS);
    // One dock on the harbour island; two factories, every part and gun on the field.
    assert_eq!((port.docks().len(), port.factories().len()), (1, 2));
    for j in 0..2 {
        assert!(port.factory_position(j).distance(field.field.centre) < AIRFIELD_RADIUS);
    }
    let heavy = port.turrets().iter().filter(|g| g.heavy_battery()).count();
    assert_eq!(
        (port.turrets().len(), heavy),
        (1 + LIGHT_GUNS.len() + HEAVY_GUNS.len(), 2)
    );
    for gun in &port.turrets()[1..] {
        let at = gun.position(&port.body);
        assert!(
            at.distance(field.field.centre) + gun.radius() < AIRFIELD_RADIUS,
            "{at}"
        );
    }
    // Every shelter holds a fuelled aircraft, and the trucks wait at the depot.
    assert_eq!(field.ready(), SHELTERS.len());
    assert_eq!(field.trucks().len(), TRUCKS);
    assert_eq!(
        (field.live_hangars(), field.tower_live(), field.depot_live()),
        (2, true, true)
    );
}

#[test]
fn a_stirred_field_sends_a_wing_rolling_out_that_takes_off_and_comes_for_her() {
    let (mut b, i) = airfield_sector();
    let orders = off_the_field(&b, i, 700.);
    b.ports[i].rouse(0.);
    let mut rolled = 0;
    let mut most_aloft = 0;
    let mut first_up = None;
    sail(&mut b, &orders, 70., |b, _| {
        rolled = rolled.max(
            field(b, i)
                .taxiing()
                .iter()
                .filter(|t| !t.sheltered())
                .count(),
        );
        most_aloft = most_aloft.max(b.warplanes.len());
        if first_up.is_none() && !b.warplanes.is_empty() {
            first_up = Some(b.at_sea);
        }
    });
    // Nothing goes up until the field stirs and its first wing is due, and then the
    // wing goes up together: three in the early rounds.
    let up = first_up.expect("a wing took off");
    assert!(up > FIRST_WING, "{up}");
    assert!(rolled >= 1);
    assert_eq!(most_aloft, 3);
    // It comes for her: bombs fall about her, or a torpedo runs at her.
    let mut struck = false;
    sail(&mut b, &orders, 60., |b, _| {
        struck |= !b.bombs.is_empty() || b.torpedoes.iter().any(|t| t.hostile());
    });
    assert!(struck, "the wing attacked");
}

#[test]
fn aircraft_rolling_out_can_be_hit_and_their_fuel_takes_whatever_is_close_by() {
    let (mut b, i) = airfield_sector();
    let orders = off_the_field(&b, i, 700.);
    b.ports[i].rouse(0.);
    sail(&mut b, &orders, FIRST_WING + 0.5, |_, _| {});
    let taxi = field(&b, i)
        .taxiing()
        .iter()
        .position(|t| !t.sheltered())
        .unwrap();
    let (at, shelter) = {
        let t = &field(&b, i).taxiing()[taxi];
        (t.position_3d(), t.shelter)
    };
    let mut events = Events::default();
    let (kind, _) = b.strike_airfield(i, AirfieldPart::Plane(taxi), 4, at, &mut events);
    assert_eq!(kind, EffectKind::FuelBlast { depot: false });
    assert!(events.planes_downed >= 1);
    // Its shelter is free for the hangars to fill again.
    assert!(field(&b, i).shelters()[shelter].away.is_none());
    // A truck blown up by the depot takes the trucks waiting beside it.
    let (mut b, i) = airfield_sector();
    let truck = field(&b, i).trucks()[0]
        .body
        .pos
        .extend(FIELD_LEVEL + 1.)
        .xzy();
    let mut events = Events::default();
    let (kind, _) = b.strike_airfield(i, AirfieldPart::Truck(0), 1, truck, &mut events);
    assert_eq!(kind, EffectKind::FuelBlast { depot: false });
    assert_eq!(events.vehicles as usize, TRUCKS);
    assert!(field(&b, i).trucks().is_empty());
    // The shelters themselves turn every round.
    let door = field(&b, i)
        .shelter_position(0)
        .extend(FIELD_LEVEL + 2.)
        .xzy();
    let (kind, _) = b.strike_airfield(i, AirfieldPart::Shelter(0), 99, door, &mut events);
    assert_eq!(kind, EffectKind::Burst);
}

#[test]
fn with_its_tower_down_the_field_sends_wings_up_far_less_often_until_it_is_raised_again() {
    let (mut b, i) = airfield_sector();
    let mut events = Events::default();
    let tower = field(&b, i).tower_position().extend(FIELD_LEVEL + 5.).xzy();
    let (kind, _) = b.strike_airfield(i, AirfieldPart::Tower, 99, tower, &mut events);
    assert_eq!(kind, EffectKind::Felled(FieldWork::Tower));
    // The count toward the next wing runs at a fraction of its pace while it is down.
    let f = field_mut(&mut b, i);
    f.stir();
    f.wing_due = WING_INTERVAL;
    let mut serial = 1_000_000;
    for _ in 0..100 {
        f.send_wing(0.1, 3, &mut serial);
    }
    assert!(
        (f.wing_due - (WING_INTERVAL - 10. * TOWERLESS)).abs() < 0.01,
        "{}",
        f.wing_due
    );
    // Its two factories raise it, twice as fast while its drydock keeps it supplied.
    let supplied = TOWER_REBUILD / (2. * SUPPLIED);
    let mut f = field(&b, i).clone();
    let mut g = f.clone();
    let mut t = 0.;
    while !f.tower_live() {
        f.work(0.1, (2, true, false), &mut serial);
        t += 0.1;
    }
    assert!((t - supplied).abs() < 0.3, "{t}");
    t = 0.;
    while !g.tower_live() {
        g.work(0.1, (2, false, false), &mut serial);
        t += 0.1;
    }
    assert!((t - 2. * supplied).abs() < 0.3, "{t}");
}

#[test]
fn without_its_trucks_a_plane_still_fuels_from_its_shelter_only_more_slowly() {
    let (b, i) = airfield_sector();
    let mut serial = 1_000_000;
    let fuel = |f: &Airfield| f.shelters[0].plane.unwrap().fuel;
    let mut f = field(&b, i).clone();
    f.shelters[0].plane = Some(Parked {
        kind: PlaneKind::Bomber,
        fuel: 0.,
    });
    let mut slow = f.clone();
    slow.trucks.clear();
    slow.depot = 0;
    let (mut fast_time, mut slow_time) = (0., 0.);
    while fuel(&f) < 1. {
        f.work(0.1, (2, true, false), &mut serial);
        fast_time += 0.1;
    }
    while fuel(&slow) < 1. {
        slow.work(0.1, (2, true, false), &mut serial);
        slow_time += 0.1;
    }
    assert!(fast_time < 12., "{fast_time}");
    assert!((slow_time - HYDRANT_FUELLING).abs() < 0.3, "{slow_time}");
    assert!(slow.trucks.is_empty(), "no depot, no new trucks");
}

#[test]
fn the_base_falls_only_once_its_hangars_are_down_with_its_dock_and_factories() {
    let (mut b, i) = airfield_sector();
    let mut events = Events::default();
    for j in 0..b.ports[i].docks().len() {
        b.ports[i].damage_dock(j, 999);
    }
    for j in 0..b.ports[i].factories().len() {
        b.ports[i].damage_factory(j, 999);
    }
    assert!(!b.ports[i].wrecked());
    for h in 0..HANGARS.len() {
        let at = field(&b, i)
            .hangar_position(h)
            .extend(FIELD_LEVEL + 3.)
            .xzy();
        let (kind, _) = b.strike_airfield(i, AirfieldPart::Hangar(h), 999, at, &mut events);
        assert_eq!(kind, EffectKind::Felled(FieldWork::Hangar));
    }
    assert!(b.ports[i].wrecked());
}
