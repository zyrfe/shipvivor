use crate::projectile::FlightPath;
use crate::*;

const EAST: Set = Set {
    flow: Vec2::new(0.5, 0.),
    meander: 0.,
    wavelength: 600.,
    phase: 0.,
};
/// Well clear of everything, so no gun or patrol takes notice.
const FAR: Vec2 = Vec2::new(5000., 5000.);

/// Harbours and gates well clear of the water under test round the origin.
fn sea(set: Set, islets: Vec<Islet>, ice: Vec<Ice>) -> Battle {
    let site = |x: f32, y: f32| HarbourSite {
        pos: Vec2::new(x, y),
        heading: 0.,
    };
    let chart = Chart {
        seed: Some(7),
        course: Course::Circuit,
        coast: vec![],
        pickets: PICKETS.map(|(ahead, side)| Vec2::new(side, -ahead)),
        gates: [
            (0., -300.),
            (300., -150.),
            (300., 200.),
            (-300., 200.),
            (-300., -150.),
        ]
        .map(Vec2::from),
        harbours: [site(0., -480.), site(500., 420.), site(-500., 420.)],
        islets,
        set,
        ice,
    };
    let mut b = Battle::for_chart(14., &chart);
    b.raiders.clear();
    for gun in &mut b.wing_guns {
        gun.reload = 1000.;
    }
    b
}
fn islet(x: f32, y: f32, radius: f32) -> Islet {
    Islet {
        pos: Vec2::new(x, y),
        radius,
        kind: LandmarkKind::Stacks,
        heading: 0.,
    }
}
fn piece(x: f32, y: f32, radius: f32, shape: IceShape) -> Ice {
    Ice {
        pos: Vec2::new(x, y),
        radius,
        shape,
        heading: 0.,
    }
}
fn orders(player: Vec2, speed: f32) -> Orders {
    Orders {
        player,
        heading: 0.,
        speed,
        hull: Hull::CRUISER,
        aim: Aim::Hold,
        reload: 3.2,
        shell: MainShell::STOCK,
    }
}

#[test]
fn ice_rides_the_stream_round_an_island_without_grounding() {
    let mut b = sea(
        EAST,
        vec![islet(0., 0., 50.)],
        vec![piece(-220., 30., 4., IceShape::Growler)],
    );
    assert!(
        b.icebergs()[0]
            .way
            .distance(b.current(Vec2::new(-220., 30.)))
            < 1e-4
    );
    let mut closest = f32::MAX;
    for _ in 0..4 * 1200 {
        b.drift_ice(0.25, FAR);
        let berg = &b.icebergs()[0];
        closest = closest.min(berg.body.pos.length() - 50. - berg.radius);
        if berg.body.pos.x > 200. {
            break;
        }
    }
    let berg = &b.icebergs()[0];
    assert!(berg.body.pos.x > 200., "still at {}", berg.body.pos);
    assert!(closest > 0., "touched the shore by {closest} m");
    assert_eq!(berg.aground(), 0.);
    let stream = b.current(berg.body.pos);
    assert!(berg.way.distance(stream) < 0.1 * stream.length() + 0.02);
}

#[test]
fn a_berg_jammed_in_a_strait_breaks_up_and_the_pieces_drift_through() {
    // A 30 m gap, and a berg 40 m across driven into it.
    let mut b = sea(
        EAST,
        vec![islet(0., -55., 40.), islet(0., 55., 40.)],
        vec![piece(-110., 0., 20., IceShape::Tabular)],
    );
    let mut broke_at = None;
    for tick in 0..4 * 1800 {
        b.drift_ice(0.25, FAR);
        if broke_at.is_none() && b.icebergs().len() > 1 {
            broke_at = Some(tick as f32 * 0.25);
        }
        if b.icebergs().iter().all(|i| i.body.pos.x > 60.) {
            break;
        }
    }
    let broke_at = broke_at.expect("the berg never broke");
    assert!(broke_at > 45., "broke after only {broke_at} s");
    assert!(
        b.effects().iter().any(|fx| fx.kind == EffectKind::Splash) || broke_at < 1700.,
        "no sign of the break"
    );
    for i in b.icebergs() {
        assert!(
            i.body.pos.x > 60.,
            "a piece is still jammed at {}",
            i.body.pos
        );
        assert!(i.radius < 20.);
    }
}

#[test]
fn a_cruiser_that_rams_a_berg_is_stove_in_and_the_berg_hardly_moves() {
    let start = Vec2::new(0., -40.);
    let mut b = sea(
        Set::SLACK,
        vec![],
        vec![piece(0., -40., 10., IceShape::Dome)],
    );
    let (mut pos, mut speed) = (Vec2::ZERO, 13.6);
    let (mut damage, mut blows, mut crunches) = (0, 0, 0);
    let dt = 1. / 60.;
    for _ in 0..120 {
        pos += Vec2::NEG_Y * speed * dt;
        let ev = b.step(dt, &orders(pos, speed));
        pos += ev.shove;
        speed += ev.jolt;
        damage += ev.damage;
        blows += ev.blows.len();
        crunches += b
            .effects()
            .iter()
            .filter(|fx| fx.kind == EffectKind::Crunch && fx.age == 0.)
            .count();
    }
    assert_eq!(crunches, 1);
    assert_eq!(blows, 1);
    assert!((15..=40).contains(&damage), "hull lost: {damage}");
    assert!(speed < 1., "kept way: {speed}");
    let berg = &b.icebergs()[0];
    // A berg of this size outweighs the cruiser six times over.
    assert!(
        berg.body.pos.distance(start) < 4.,
        "berg shoved to {}",
        berg.body.pos
    );

    // Nosing into the same berg dead slow does no harm: she pushes it slowly ahead.
    let mut b = sea(
        Set::SLACK,
        vec![],
        vec![piece(0., -40., 10., IceShape::Dome)],
    );
    let (mut pos, speed) = (Vec2::ZERO, 1.5);
    for _ in 0..60 * 12 {
        pos += Vec2::NEG_Y * speed * dt;
        let ev = b.step(dt, &orders(pos, speed));
        pos += ev.shove;
        assert_eq!(ev.damage, 0);
        let berg = &b.icebergs()[0];
        let stem = pos.y - Hull::CRUISER.bow;
        assert!(
            stem > berg.body.pos.y + berg.radius - 0.5,
            "the bow is in the ice"
        );
    }
    assert!(
        b.icebergs()[0].body.pos.y < start.y - 0.5,
        "the berg never moved"
    );
}

#[test]
fn ice_shields_what_lies_behind_it_and_breaks_under_fire() {
    // A pinnacle stands tall enough to catch a round on its arc.
    let mut b = sea(
        Set::SLACK,
        vec![],
        vec![piece(0., -80., 12., IceShape::Pinnacle)],
    );
    // The sight stops on the near face.
    let eye = Vec3::new(0., 5., 0.);
    let sight = b.sight(
        eye,
        Vec3::new(0., -3., -100.).normalize(),
        Vec2::new(0., -100.),
    );
    assert!(
        (sight.point.z + 80. - 12. * 0.55).abs() < 0.5,
        "sight landed at {}",
        sight.point
    );

    // A round aimed past the berg bursts on it and chips it.
    let full = b.icebergs()[0].integrity;
    let path = FlightPath::to(
        Vec3::new(0., 3., 0.),
        Vec3::new(0., 1.7, -160.),
        Battery::Secondary,
    )
    .expect("a firing solution");
    path.launch(
        &mut b.shots,
        &mut b.effects,
        &mut b.serial,
        Battery::Secondary,
        9,
    );
    let mut events = Events::default();
    for _ in 0..240 {
        b.advance_shots(1. / 60., FAR, &mut events);
    }
    assert!(b.shots.is_empty());
    assert!(
        b.effects()
            .iter()
            .any(|fx| fx.kind == EffectKind::Burst && fx.body.pos.y > -80.),
        "no burst on the ice"
    );
    assert_eq!(b.icebergs()[0].integrity, full - 9.);

    // Enough fire splits it into smaller pieces.
    b.floe.icebergs[0].integrity = -1.;
    let before = b.icebergs()[0].radius;
    b.drift_ice(0.1, FAR);
    assert!(b.icebergs().len() >= 2, "the berg did not split");
    assert!(b.icebergs().iter().all(|i| i.radius < before));
}

#[test]
fn a_torpedo_spends_itself_on_ice() {
    let mut b = sea(
        Set::SLACK,
        vec![],
        vec![piece(150., 0., 15., IceShape::Dome)],
    );
    let p = Vec2::ZERO;
    let beam = Rack::Starboard.center(0.);
    assert!(b.launch_spread(p, 0., beam));
    let mut events = Events::default();
    for _ in 0..600 {
        b.advance_torpedoes(1. / 60., &mut events);
    }
    assert!(b.torpedoes.is_empty());
    assert_eq!(events.hits, 0);
    assert!(
        b.icebergs()[0].integrity < 0.,
        "integrity {}",
        b.icebergs()[0].integrity
    );
    b.drift_ice(0.1, FAR);
    assert!(b.icebergs().len() >= 2, "the berg did not split");
}

#[test]
fn raiders_drift_with_the_stream_and_steer_round_bergs() {
    let mut b = sea(EAST, vec![], vec![piece(0., 0., 14., IceShape::Tabular)]);
    b.spawn(
        EnemyClass::Destroyer,
        Vec2::new(-160., 0.),
        Vec2::new(200., 0.),
    );
    let hull = EnemyClass::Destroyer.profile().hull;
    let target = Vec2::new(260., 0.);
    let mut closest = f32::MAX;
    for _ in 0..60 * 50 {
        let Battle {
            raiders,
            ports,
            landmarks,
            floe,
            ..
        } = &mut b;
        let raider = &mut raiders[0];
        let stream = floe.currents.at(raider.body.pos);
        raider.sail(1. / 60., target, ports, landmarks, &floe.icebergs, stream);
        closest = closest.min(raider.body.pos.distance(floe.icebergs[0].body.pos));
    }
    assert!(
        closest > 14. + hull.radius,
        "came within {closest} m of the berg's centre"
    );
    let raider = &b.raiders[0];
    assert!(raider.body.pos.x > 100., "made no way: {}", raider.body.pos);
    // Her way over the ground includes the set, which the guns must lead.
    let through_water = raider.motion() - b.current(raider.body.pos);
    assert!(through_water.length() < raider.motion().length());
}

#[test]
fn ice_lost_downstream_is_replaced_from_upstream_out_of_sight() {
    let mut b = sea(
        EAST,
        vec![],
        vec![
            piece(-150., -200., 3., IceShape::Growler),
            piece(150., 220., 8., IceShape::Dome),
        ],
    );
    let player = Vec2::new(-200., 0.);
    b.floe.icebergs.clear();
    for _ in 0..4 * 60 {
        b.drift_ice(0.25, player);
    }
    let ice = b.icebergs();
    assert_eq!(ice.len(), 2, "ice drifted in: {}", ice.len());
    let (lo, _) = b.floe.reach;
    for berg in ice {
        assert!(berg.body.pos.distance(player) > SPAWN_STANDOFF);
        // It came in over the western edge, the way the stream runs from.
        assert!(
            berg.body.pos.x < lo.x + 120.,
            "appeared at {}",
            berg.body.pos
        );
    }
}
