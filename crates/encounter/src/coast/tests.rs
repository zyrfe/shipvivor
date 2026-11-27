use crate::projectile::FlightPath;
use crate::*;

/// Where the player is kept, well clear of every round.
const FAR: Vec2 = Vec2::new(5000., 5000.);

/// An empty sea with one small island of `kind` at (300, 0), unturned and of the size
/// its models are built at.
fn islet(kind: LandmarkKind) -> Battle {
    let gates = [
        (0., -152.),
        (168., -424.),
        (496., -300.),
        (416., 76.),
        (72., 164.),
    ];
    let mut b = Battle::for_route(14., gates.map(Vec2::from));
    b.raiders.clear();
    b.landmarks.clear();
    b.ports.clear();
    b.floe.icebergs.clear();
    let at = body(&mut b.serial, Vec2::new(300., 0.), 0.);
    b.landmarks.push(Landmark::new(at, ISLAND_RADIUS, kind));
    b
}

/// One main-battery round `inches` across from `eye` at `mark`, flown until it lands.
/// Returns where it struck.
fn salvo(b: &mut Battle, eye: Vec3, mark: Vec3, inches: u16) -> Vec2 {
    let shell = MainShell {
        bore: inches - 4,
        ..MainShell::STOCK
    };
    let path = FlightPath::main(eye, mark, shell).expect("the mark is in reach");
    b.effects.clear();
    let (shots, effects, serial) = (&mut b.shots, &mut b.effects, &mut b.serial);
    path.launch(shots, effects, serial, Battery::Main, shell.breach_force());
    for _ in 0..40 {
        b.advance_shots(0.1, FAR, &mut Events::default());
    }
    assert!(b.shots.is_empty());
    let fall = b
        .effects
        .iter()
        .find(|e| !matches!(e.kind, EffectKind::Muzzle(_)));
    fall.expect("the round came down").body.pos
}

fn toppled(b: &Battle) -> Vec<FeatureKind> {
    (b.effects.iter())
        .filter_map(|e| match e.kind {
            EffectKind::Toppled(kind) => Some(kind),
            _ => None,
        })
        .collect()
}

#[test]
fn a_lighthouse_takes_four_eight_inch_hits_to_topple_and_then_stops_nothing() {
    let mut b = islet(LandmarkKind::Beacon);
    let (eye, mark) = (Vec3::new(100., 12., 0.), Vec3::new(300., 12., 0.));
    for _ in 0..3 {
        let struck = salvo(&mut b, eye, mark, 8);
        assert!((struck.x - 297.).abs() < 0.5, "{struck}");
        assert!(b.landmarks[0].features[0].standing());
        assert!(toppled(&b).is_empty());
    }
    salvo(&mut b, eye, mark, 8);
    let tower = &b.landmarks[0].features[0];
    let (_, way) = tower.fell.expect("the fourth hit brings it down");
    assert!(way.x > 0.99, "it falls the way the round was going: {way}");
    assert_eq!(tower.volume(), None);
    assert_eq!(toppled(&b), [FeatureKind::Lighthouse]);
    // The next round flies on through where the tower stood.
    let struck = salvo(&mut b, eye, mark, 8);
    assert!(struck.x > 303., "{struck}");
}

#[test]
fn a_sea_stack_is_shot_away_a_tier_at_a_time() {
    let mut b = islet(LandmarkKind::Stacks);
    // From the east, clear of the two lesser stacks, at the foot of the tall one.
    let (eye, mark) = (Vec3::new(500., 14., 0.), Vec3::new(300., 6., 0.));
    let stack = |b: &Battle| b.landmarks[0].features[0].clone();
    let site = stack(&b).site;
    assert_eq!((site.kind, stack(&b).tiers()), (FeatureKind::Stack, 4));
    for left in (0..4).rev() {
        salvo(&mut b, eye, mark, 18);
        assert_eq!(stack(&b).tiers(), left);
        assert_eq!(toppled(&b), [FeatureKind::Stack]);
        let top = stack(&b).volume().map(|(_, _, top)| top);
        assert_eq!(top, left.checked_sub(1).map(|t| site.tier_top(t)));
    }
    assert!(!stack(&b).standing());
    // An eight-inch round only chips at a fresh stack.
    let mut b = islet(LandmarkKind::Stacks);
    salvo(&mut b, eye, mark, 8);
    assert_eq!((stack(&b).tiers(), toppled(&b).len()), (4, 0));
}

#[test]
fn a_heavy_round_fells_the_pines_round_where_it_bursts_and_a_light_one_fewer() {
    let felled = |inches: u16| {
        let mut b = islet(LandmarkKind::Pines);
        let trees = &b.landmarks[0].features;
        // The tree with the most others close round it.
        let crowd = |i: usize| {
            let at = trees[i].pos;
            trees.iter().filter(|t| t.pos.distance(at) < 7.).count()
        };
        let target = (0..trees.len()).max_by_key(|&i| crowd(i)).unwrap();
        let mark = trees[target].pos.extend(FOOTING + 1.).xzy();
        let struck = salvo(&mut b, Vec3::new(100., 10., 0.), mark, inches);
        let island = &b.landmarks[0];
        let down: Vec<&Feature> = island.features.iter().filter(|t| !t.standing()).collect();
        let reach = 0.4 * f32::from(inches);
        for tree in &down {
            let (_, way) = tree.fell.unwrap();
            let away = tree.pos - struck;
            // The struck tree falls along the round; the rest are thrown clear of it.
            assert!(away.length() < 2.5 || way.dot(away) > 0., "{way} {away}");
            assert!(away.length() <= reach.max(2.5), "{away}");
        }
        assert_eq!(
            island.volumes().count(),
            1 + island.features.len() - down.len()
        );
        down.len()
    };
    let (heavy, light) = (felled(18), felled(4));
    assert!(light >= 1, "a hit fells the tree it strikes");
    assert!(heavy >= 3 && heavy > light, "{heavy} {light}");
}

#[test]
fn features_stand_on_the_shelf_and_spread_and_turn_with_the_island() {
    for kind in [
        LandmarkKind::Pines,
        LandmarkKind::Stacks,
        LandmarkKind::Beacon,
    ] {
        for site in kind.sites() {
            assert!(
                site.at.length() + site.width < 0.8 * ISLAND_RADIUS,
                "{kind:?}"
            );
        }
    }
    let mut serial = 0;
    let at = body(
        &mut serial,
        Vec2::new(50., 50.),
        std::f32::consts::FRAC_PI_2,
    );
    let island = Landmark::new(at, 2. * ISLAND_RADIUS, LandmarkKind::Stacks);
    let site = island.features[1].site;
    let spread = island.features[1].pos - island.body.pos;
    assert!(spread.distance(Vec2::from_angle(at.heading).rotate(2. * site.at)) < 1e-4);
    assert!(LandmarkKind::Glacier.sites().is_empty());
}
