use super::*;
fn battle() -> Battle {
    Battle::for_route(
        14.,
        [
            (0., -152.),
            (168., -424.),
            (496., -300.),
            (416., 76.),
            (72., 164.),
        ]
        .map(Vec2::from),
    )
}
#[test]
fn reserve_power_slows_main_and_secondary_aim_and_both_can_still_fire() {
    use shipvivor_shipyard::{Component, Design, Ship};
    let player = Vec2::new(2000., 2000.);
    let target = player + Vec2::new(90., -80.);
    let orders = Orders {
        player,
        heading: 0.,
        speed: 0.,
        hull: Hull::CRUISER,
        aim: Aim::Sight(target.extend(18.).xzy()),
        reload: 3.2,
        shell: MainShell::STOCK,
    };
    let exercise = |reserve: bool| {
        let mut b = battle();
        b.raiders.clear();
        b.spawn(EnemyClass::Patrol, target, player);
        let mut ship = Ship::new(Design::breakwater());
        if reserve {
            let engine = ship.fitted(Component::Engine).next().unwrap().0;
            ship.strike_fitting(engine, 12.);
        }
        b.update_mount_service(&ship);
        let starboard = b.wing_guns[1].deck_bearing();
        let dt = 1. / 60.;
        b.step(dt, &orders);
        let movement = [
            b.main_mounts[0].deck_bearing().abs(),
            b.main_mounts[0].pitch().abs(),
            (b.wing_guns[1].deck_bearing() - starboard).abs(),
        ];
        let mut fired = [false; 2];
        for _ in 0..20 * 60 {
            b.step(dt, &orders);
            for (i, battery) in [Battery::Main, Battery::Secondary].into_iter().enumerate() {
                fired[i] |= b
                    .effects
                    .iter()
                    .any(|fx| fx.kind == EffectKind::Muzzle(battery));
            }
            if fired == [true, true] {
                break;
            }
        }
        assert_eq!(fired, [true, true], "reserve power {reserve}");
        movement
    };
    let full = exercise(false);
    let reserve = exercise(true);
    for (normal, reduced) in full.into_iter().zip(reserve) {
        assert!(normal > 0.);
        assert!((reduced / normal - 0.30).abs() < 0.001);
    }
}

#[test]
fn crippled_raider_drifts_and_falls_silent_when_its_reserve_power_runs_out() {
    let mut b = battle();
    b.raiders.truncate(1);
    b.next_wave = 1000.;
    for port in &mut b.ports {
        port.convoy_clock = 1000.; // Isolate the disabled boat from convoy escorts.
    }
    for gun in &mut b.wing_guns {
        gun.reload = 100.; // And from the player's crew.
    }
    // Far enough off that she carries her way off and drifts without fouling the player.
    b.raiders[0].body.pos = Vec2::new(0., -70.);
    cripple(&mut b.raiders[0]);
    let start = b.raiders[0].body.pos;
    let orders = Orders {
        heading: 0.,
        speed: 0.,
        hull: Hull::CRUISER,
        player: Vec2::ZERO,
        aim: Aim::Hold,
        reload: 3.2,
        shell: super::MainShell::STOCK,
    };
    // Its gun has 20 seconds of power after the engine stops, and nobody to restart it.
    let mut fired = [0, 0];
    for tick in 0..60 * 45 {
        assert_eq!(b.step(1. / 60., &orders).sunk, 0);
        fired[usize::from(tick > 60 * 21)] += b
            .effects
            .iter()
            .filter(|fx| fx.kind == EffectKind::Muzzle(Battery::Raider) && fx.age == 0.)
            .count();
    }
    assert!(fired[0] > 0, "it fights on while the reserve lasts");
    assert_eq!(fired[1], 0);
    assert!(b.raiders[0].is_disabled());
    assert_ne!(b.raiders[0].body.pos, start);
}
#[test]
fn shell_sweeps_and_stops_at_first_contact() {
    assert_eq!(
        entry(Vec2::ZERO, Vec2::X * 10., Vec2::X * 6., 1.),
        Some(0.5)
    );
    assert_eq!(entry(Vec2::ZERO, Vec2::X, Vec2::Y * 5., 1.), None);
    let mut b = battle();
    b.raiders.clear();
    for gun in &mut b.wing_guns {
        gun.reload = 100.;
    }
    // Both boats lie across the line of fire.
    b.spawn(
        EnemyClass::Patrol,
        Vec2::new(0., -40.),
        Vec2::new(10., -40.),
    );
    b.spawn(
        EnemyClass::Patrol,
        Vec2::new(0., -15.),
        Vec2::new(10., -15.),
    );
    let e = b.step(
        0.5,
        &Orders {
            heading: 0.,
            speed: 0.,
            hull: Hull::CRUISER,
            player: Vec2::ZERO,
            aim: Aim::At(Vec2::NEG_Y * 100.),
            shell: super::MainShell {
                bore: 3,
                ..super::MainShell::STOCK
            },
            reload: 2.,
        },
    );
    assert_eq!(e.sunk, 1);
    assert!(b.raiders[0].body.pos.y < -30.);
    assert!(
        b.effects
            .iter()
            .any(|e| e.kind == EffectKind::Wreck(EnemyClass::Patrol))
    );
}
#[test]
fn pickup_counts_once_and_nearby_crew_fires_without_main_order() {
    let mut b = battle();
    let o = Orders {
        heading: 0.,
        speed: 0.,
        hull: Hull::CRUISER,
        player: Vec2::new(0., -28.),
        aim: Aim::Hold,
        shell: super::MainShell::STOCK,
        reload: 3.2,
    };
    assert_eq!(b.step(0.01, &o).recovered, 1);
    assert_eq!(b.step(0.01, &o).recovered, 0);
    b.spawn(EnemyClass::Patrol, o.player + Vec2::X * 22., o.player);
    for gun in &mut b.wing_guns {
        gun.reload = 0.;
    }
    for _ in 0..15 {
        b.step(0.01, &o);
        if b.effects
            .iter()
            .any(|e| e.kind == EffectKind::Muzzle(Battery::Secondary))
        {
            break;
        }
    }
    assert!(
        b.effects
            .iter()
            .any(|e| e.kind == EffectKind::Muzzle(Battery::Secondary))
    );
    assert!(!b.shots.iter().any(|s| s.battery == Battery::Main));
}

#[test]
fn new_waves_leave_distant_berths_and_never_teleport_near_the_player() {
    let mut b = battle();
    let berths: Vec<Vec2> = b.ports.iter().map(Port::berth).collect();
    assert!(
        b.raiders
            .iter()
            .all(|r| r.body.pos.length() >= SPAWN_STANDOFF)
    );
    let remote_store = b.salvage.last().unwrap().body.id;
    let player = berths[0];
    let o = Orders {
        player,
        heading: 0.,
        speed: 0.,
        hull: Hull::CRUISER,
        aim: Aim::Hold,
        shell: super::MainShell::STOCK,
        reload: 3.2,
    };
    b.next_wave = 0.;
    for p in &mut b.ports {
        p.clear_slips();
    }
    // Each roused base's garrison weighs first. Then exactly one yard launches.
    b.rouse_garrisons();
    let at_sea = b.combatants();
    b.step(0.01, &o);
    let raider = b.raiders.last().unwrap();
    assert_eq!(b.combatants(), at_sea + 1);
    assert!(raider.body.pos.distance(player) >= SPAWN_STANDOFF - 0.1);
    assert!(
        berths[1..]
            .iter()
            .any(|&berth| raider.body.pos.distance(berth) < 0.1)
    );
    assert!(b.salvage.iter().any(|c| c.body.id == remote_store));
    // If every dock is too close there is no replacement spawn around the player.
    for i in 1..3 {
        b.ports[i].body = b.ports[0].body;
    }
    b.reinforce(player);
    assert_eq!(b.combatants(), at_sea + 1);
}

#[test]
fn shore_gun_can_hit_an_approaching_ship_and_be_silenced() {
    let mut b = battle();
    b.raiders.clear();
    b.next_wave = 100.;
    let gun = b.ports[0].turrets()[0].position(&b.ports[0].body);
    let o = Orders {
        heading: 0.,
        speed: 0.,
        hull: Hull::CRUISER,
        player: gun + 80. * b.ports[0].body.forward(),
        aim: Aim::Hold,
        shell: super::MainShell::STOCK,
        reload: 3.2,
    };
    let dt = 1. / 60.;
    let mut damage = 0;
    for _ in 0..420 {
        damage += b.step(dt, &o).damage;
    }
    // One round from the single gun, close aboard.
    assert_eq!(damage, 11);
    let attack = Orders {
        aim: Aim::At(gun),
        ..o
    };
    for _ in 0..280 {
        b.step(dt, &attack);
    }
    assert_eq!(b.ports[0].turrets[0].strength, 0);
    b.shots.clear();
    for _ in 0..450 {
        assert_eq!(b.step(dt, &o).damage, 0);
        assert!(b.shots.iter().all(|shot| shot.battery != Battery::Shore));
    }
}

#[test]
fn harbour_salvos_leave_every_fitted_barrel_at_its_own_muzzle() {
    let batteries: [&[u8]; 3] = [&[1], &[2, 2], &[4, 3, 3]];
    for (base, expected) in batteries.into_iter().enumerate() {
        for (slot, &barrels) in expected.iter().enumerate() {
            let mut b = battle();
            let port = &mut b.ports[base];
            assert_eq!(port.turrets().len(), expected.len());
            let gun = &port.turrets()[slot];
            assert_eq!(gun.barrels, barrels);
            let at = gun.position(&port.body);
            let out = (at - port.body.pos).normalize();
            // Each outward-facing station has a clear approach from the sea.
            for _ in 0..350 {
                port.fire_turrets(
                    1. / 60.,
                    Some(at + out * 95.),
                    Vec2::ZERO,
                    &mut b.shots,
                    &mut b.effects,
                    &mut b.serial,
                );
            }
            let salvo: Vec<_> = b
                .shots
                .iter()
                .filter(|s| s.body.pos.distance(at) < 10.)
                .collect();
            assert_eq!(
                salvo.len(),
                usize::from(barrels),
                "base {base}, mount {slot}"
            );
            for (i, shot) in salvo.iter().enumerate() {
                let muzzle = at + out * 8.25 + out.perp() * ShoreTurret::barrel_x(barrels, i as u8);
                assert!(shot.body.pos.distance(muzzle) < 0.02);
                assert_eq!(shot.battery, Battery::Shore);
            }
        }
    }
}

#[test]
fn heavier_batteries_punish_a_cruiser_lingering_broadside() {
    let damage = [0, 1, 2].map(|base| {
        let mut b = battle();
        b.raiders.clear();
        b.landmarks.clear();
        b.wing_guns.clear();
        b.next_wave = 1000.;
        for (i, p) in b.ports.iter_mut().enumerate() {
            if i != base {
                p.wreck();
                p.silence();
            }
        }
        let p = &b.ports[base];
        let mount = p.turrets()[0].position(&p.body);
        let outward = (mount - p.body.pos).normalize();
        let orders = Orders {
            player: mount + outward * 90.,
            heading: bearing(outward.perp()),
            ..distant_observer()
        };
        // Long enough for several salvos, so the layers' errors even out.
        (0..60 * 30)
            .map(|_| u32::from(b.step(1. / 60., &orders).damage))
            .sum::<u32>()
    });
    assert!(
        damage[0] > 0 && damage[0] < damage[1] && damage[1] < damage[2],
        "damage over the same half minute: {damage:?}"
    );
}

#[test]
fn each_foundry_mount_can_be_sighted_and_silenced_under_fire() {
    let mut b = battle();
    b.raiders.clear();
    b.landmarks.clear();
    let damage = MainShell::STOCK.breach_force();
    for (slot, hits) in [12, 9, 9].into_iter().enumerate() {
        let p = &b.ports[2];
        let at = p.turrets()[slot].position(&p.body);
        let out = (at - p.body.pos).normalize();
        let target = at.extend(5.7).xzy();
        let eye = (at + out * 60.).extend(5.7).xzy();
        for remaining in (0..hits).rev() {
            let sight = b.sight(eye, (target - eye).normalize(), at - out * 20.);
            assert_eq!(sight.shore_turret, Some((2, slot)));
            fire(
                &mut b.shots,
                &mut b.effects,
                &mut b.serial,
                eye,
                target,
                Battery::Main,
                damage,
            );
            b.advance_shots(0.5, eye.xz(), &mut Events::default());
            assert_eq!(b.ports[2].turrets()[slot].strength(), damage * remaining);
        }
        assert_eq!(
            b.sight(eye, (target - eye).normalize(), at - out * 8.)
                .shore_turret,
            None
        );
        let port = &mut b.ports[2];
        assert_eq!(port.docks()[0].state(), YardState::Working);
        for (index, gun) in port.turrets().iter().enumerate() {
            assert_eq!(
                gun.strength(),
                if index <= slot { 0 } else { gun.max_strength() }
            );
        }
        // Bring the next surviving mount to bear while the previous ones stay silent.
        let next = (slot + 1) % port.turrets().len();
        let station = port.turrets()[next].position(&port.body);
        let player = station + (station - port.body.pos).normalize() * 90.;
        port.fire_turrets(
            20.,
            Some(player),
            Vec2::ZERO,
            &mut b.shots,
            &mut b.effects,
            &mut b.serial,
        );
        assert_eq!(
            b.shots
                .iter()
                .filter(|s| s.body.pos.distance(station) < 10.)
                .count(),
            if next == 0 { 0 } else { 3 }
        );
        assert!(b.shots.iter().all(|s| s.body.pos.distance(at) > 10.));
        b.shots.clear();
    }
}

#[test]
fn one_delivery_shares_its_spares_across_the_heavy_battery() {
    let mut b = battle();
    let port = &mut b.ports[2];
    port.stock = YARD_CAPACITY;
    assert!(!port.needs_delivery());
    port.turrets[0].strength -= 1;
    port.turrets[1].strength = 0;
    assert!(port.needs_delivery());
    port.unload();
    assert_eq!(
        port.turrets()
            .iter()
            .map(ShoreTurret::strength)
            .collect::<Vec<_>>(),
        [24, 2, 18]
    );
    // A wrecked base takes no delivery, though its guns still stand.
    port.wreck();
    let standing: Vec<u16> = port.turrets().iter().map(ShoreTurret::strength).collect();
    port.unload();
    assert_eq!(port.stock, 0);
    assert_eq!(
        port.turrets()
            .iter()
            .map(ShoreTurret::strength)
            .collect::<Vec<_>>(),
        standing
    );
}

#[test]
fn land_stops_ship_and_shells_on_the_far_side_of_a_gun() {
    let mut b = battle();
    b.raiders.clear();
    let port = &b.ports[0];
    let center = port.body.pos;
    let gun = port.turrets()[0].position(&port.body);
    let pier = center + port.body.forward() * 34. + port.body.forward().perp() * 6.;
    assert!(b.navigable_position(pier, 6.).distance(pier) > 8.9);
    assert!(b.navigable_position(center, 6.).distance(center) >= ISLAND_RADIUS + 5.99);
    let player = center - 75. * (gun - center).normalize();
    let o = Orders {
        player,
        heading: 0.,
        speed: 0.,
        hull: Hull::CRUISER,
        aim: Aim::At(gun),
        shell: super::MainShell {
            bore: 8,
            ..super::MainShell::STOCK
        },
        reload: 3.2,
    };
    for _ in 0..200 {
        b.step(1. / 60., &o);
    }
    assert_eq!(b.ports[0].turrets[0].strength, 6);
    assert!(!b.shots.iter().any(|s| s.battery == Battery::Shore));
}

#[test]
fn long_round_clears_a_nearby_boat_then_splashes_at_the_sight() {
    let mut b = battle();
    b.raiders.clear();
    // The bow pair would finish the boat off before the round arrives.
    b.wing_guns.clear();
    b.spawn(EnemyClass::Patrol, Vec2::new(0., -110.), Vec2::ZERO);
    cripple(&mut b.raiders[0]);
    let goal = Vec3::new(0., 0., -260.);
    let mut orders = Orders {
        player: Vec2::ZERO,
        heading: 0.,
        speed: 0.,
        hull: Hull::CRUISER,
        aim: Aim::Sight(goal),
        shell: super::MainShell {
            bore: 7,
            ..super::MainShell::STOCK
        },
        reload: 3.2,
    };
    orders.aim = Aim::Track(goal);
    for _ in 0..30 {
        b.step(1. / 60., &orders);
    }
    orders.aim = Aim::Sight(goal);
    b.step(1. / 60., &orders);
    orders.aim = Aim::Hold;
    let mut apex = 0_f32;
    let mut elapsed = 1. / 60.;
    while !b.shots.is_empty() && elapsed < 1. {
        apex = apex.max(b.shots[0].position().y);
        b.step(1. / 60., &orders);
        elapsed += 1. / 60.;
    }
    assert!((0.66..0.75).contains(&elapsed));
    assert!(apex > 5. && b.raiders[0].hull() == 2.);
    let splash: Vec<_> = b
        .effects
        .iter()
        .filter(|fx| fx.kind == EffectKind::Splash)
        .collect();
    assert_eq!(splash.len(), 1);
    assert!(
        (splash[0].body.pos.distance(goal.xz())
            - TurretLayout::Twin.barrel_x(1) * orders.shell.mount_scale())
        .abs()
            < 0.01
            && splash[0].height < 0.001
    );
}

#[test]
fn sights_distinguish_a_ship_from_open_water_above_its_roof() {
    let mut b = battle();
    b.raiders.clear();
    let p = Vec2::NEG_Y * 130.;
    b.spawn(EnemyClass::Patrol, p, Vec2::ZERO);
    let eye = Vec3::new(0., 18., 12.);
    let ray = (p.extend(2.2).xzy() - eye).normalize();
    assert!(b.sight_point(eye, ray, p * 1.2).y > 1.);
    let miss = b.sight_point(eye, Vec3::NEG_Z, p * 2.);
    assert_eq!(miss, (p * 2.).extend(0.).xzy());
}

/// Muzzles of the main battery barrels that fired on the last step.
fn fired(b: &Battle) -> Vec<Vec2> {
    b.effects
        .iter()
        .filter(|fx| fx.kind == EffectKind::Muzzle(Battery::Main) && fx.age == 0.)
        .map(|fx| fx.body.pos)
        .collect()
}

const BEAM_TARGET: Vec3 = Vec3::new(175., 0., 0.);

/// Breakwater heading north with both mounts loaded and trained on the starboard beam.
fn broadside() -> (Battle, Orders) {
    let mut battle = battle();
    battle.raiders.clear();
    battle.next_wave = 1000.;
    let orders = Orders {
        player: Vec2::ZERO,
        heading: 0.,
        speed: 0.,
        hull: Hull::CRUISER,
        aim: Aim::Track(BEAM_TARGET),
        reload: 6.4,
        shell: super::MainShell::STOCK,
    };
    for _ in 0..360 {
        battle.step(1. / 60., &orders);
    }
    for station in Station::ALL {
        assert_eq!(battle.main_mount(station).status(), MountStatus::Ready);
    }
    (battle, orders)
}

#[test]
fn broadside_tracks_without_firing_then_salvos_from_both_stations() {
    let mut battle = battle();
    let target = BEAM_TARGET;
    let mut orders = Orders {
        player: Vec2::ZERO,
        heading: 0.,
        speed: 0.,
        hull: Hull::CRUISER,
        aim: Aim::Track(target),
        reload: 6.4,
        shell: super::MainShell::STOCK,
    };
    let dt = 1. / 60.;
    // Hold fire while both pairs cross the beam from their separate home positions.
    for _ in 0..60 {
        battle.step(dt, &orders);
    }
    let a = battle.main_mount(Station::Bow);
    assert_eq!(a.status(), MountStatus::Training);
    assert!((a.deck_bearing().to_degrees() - 18.).abs() < 0.01);
    assert!(a.aim_mark(Vec2::ZERO, 0.).distance(target) > 100.);
    for _ in 0..300 {
        battle.step(dt, &orders);
    }
    for station in Station::ALL {
        let gun = battle.main_mount(station);
        assert_eq!(gun.status(), MountStatus::Ready);
        assert!(gun.aim_mark(Vec2::ZERO, 0.).distance(target) < 0.01);
    }
    assert!(battle.shots().iter().all(|s| s.battery != Battery::Main));
    battle.order_volley(Volley::Salvo);
    battle.step(dt, &orders);
    let muzzles = fired(&battle);
    assert_eq!(muzzles.len(), 4);
    assert_eq!(muzzles.iter().filter(|p| p.y < -5.).count(), 2);
    assert_eq!(muzzles.iter().filter(|p| p.y > 5.).count(), 2);
    for station in Station::ALL {
        let gun = battle.main_mount(station);
        assert_eq!(gun.status(), MountStatus::Loading);
        assert!((0..gun.layout().count()).all(|t| gun.recoil(t) > 0.99));
    }
    assert_eq!(battle.next_barrel(), None);
    orders.aim = Aim::Track(target);
    for _ in 0..390 {
        battle.step(dt, &orders);
        assert!(fired(&battle).is_empty());
    }
    assert_eq!(
        battle.next_barrel(),
        Some((Station::Bow, 0)),
        "the battery reloads together and starts the order again"
    );
}

#[test]
fn triple_and_quad_salvos_use_each_fitted_muzzle_and_keep_existing_reload_times() {
    for layout in [TurretLayout::Triple, TurretLayout::Quad] {
        let (mut battle, orders) = broadside();
        let dt = 1. / 60.;
        battle.order_volley(Volley::Single);
        battle.step(dt, &orders);
        let left = battle.main_mount(Station::Bow).reload_left(0);
        battle.fit_turrets(layout);
        assert_eq!(battle.main_mount(Station::Bow).reload_left(0), left);
        battle.order_volley(Volley::Salvo);
        battle.step(dt, &orders);
        assert_eq!(fired(&battle).len(), layout.count() * 2 - 1);
        assert!(battle.next_barrel().is_none());
        for _ in 0..390 {
            battle.step(dt, &orders);
        }
        battle.order_volley(Volley::Salvo);
        battle.step(dt, &orders);
        let muzzles = fired(&battle);
        assert_eq!(muzzles.len(), layout.count() * 2);
        for station in Station::ALL {
            let mount = battle.main_mount(station);
            let mut fired_here: Vec<_> = muzzles
                .iter()
                .filter(|p| (p.y < 0.) == (station == Station::Bow))
                .map(|p| p.dot(shipvivor_compass::direction(mount.deck_bearing()).perp()))
                .collect();
            fired_here.sort_by(f32::total_cmp);
            assert_eq!(fired_here.len(), layout.count());
            for (tube, pos) in fired_here.iter().enumerate() {
                let expected =
                    (layout.barrel_x(tube) - layout.barrel_x(0)) * orders.shell.mount_scale();
                assert!((pos - fired_here[0] - expected).abs() < 1e-4);
                assert!(mount.recoil(tube) > 0.99 && !mount.loaded(tube));
            }
        }
    }
}

#[test]
fn a_held_trigger_walks_the_battery_one_barrel_at_a_time() {
    let (mut b, mut orders) = broadside();
    let dt = 1. / 60.;
    orders.aim = Aim::Sight(BEAM_TARGET);
    let mut volley = vec![];
    for step in 0..395 {
        b.step(dt, &orders);
        volley.extend(fired(&b).into_iter().map(|muzzle| (step, muzzle)));
    }
    assert_eq!(volley.len(), 5);
    let (first, again) = volley.split_at(4);
    assert_eq!(first[0].0, 0, "the press fires at once");
    // Bow and stern take turns. Each pair fires its left barrel, nearer the bow, first.
    let bow: Vec<_> = first.iter().map(|(_, p)| p.y < 0.).collect();
    assert_eq!(bow, [true, false, true, false]);
    assert!(first[0].1.y < first[2].1.y && first[1].1.y < first[3].1.y);
    for pair in first.windows(2) {
        let gap = (pair[1].0 - pair[0].0) as f32 * dt;
        assert!((RIPPLE..RIPPLE + 1.5 * dt).contains(&gap), "{gap}");
    }
    // The battery then waits for the first barrel to be loaded again.
    assert!((384..=386).contains(&again[0].0), "{}", again[0].0);
    assert!((again[0].1 - first[0].1).length() < 1e-3);
}

/// Hold the trigger for `steps`, and give the steps on which each barrel fired, by its
/// muzzle.
fn hold_fire(b: &mut Battle, orders: &Orders, steps: usize) -> Vec<(Vec2, Vec<usize>)> {
    let mut barrels: Vec<(Vec2, Vec<usize>)> = vec![];
    for step in 0..steps {
        b.step(1. / 60., orders);
        for muzzle in fired(b) {
            match barrels.iter_mut().find(|(m, _)| m.distance(muzzle) < 1e-3) {
                Some((_, shots)) => shots.push(step),
                None => barrels.push((muzzle, vec![step])),
            }
        }
    }
    barrels
}

/// The longest wait between two rounds from one barrel, in steps.
fn longest_wait(shots: &[usize]) -> usize {
    shots
        .windows(2)
        .map(|w| w[1] - w[0])
        .max()
        .unwrap_or(usize::MAX)
}

#[test]
fn a_held_trigger_quickens_so_a_fast_battery_fires_every_barrel_each_time_it_loads() {
    let (mut b, mut orders) = broadside();
    // Four barrels loading in 0.6 s: a ripple apart, the trigger would come back to each
    // only every 1.2 s.
    orders.reload = 0.6;
    orders.aim = Aim::Sight(BEAM_TARGET);
    let barrels = hold_fire(&mut b, &orders, 300);
    assert_eq!(barrels.len(), 4);
    for (_, shots) in &barrels {
        // 36 steps to load, and a step for the clock.
        assert!(longest_wait(shots) <= 37, "{shots:?}");
    }
    // The battery still ripples rather than firing in salvoes.
    let mut all: Vec<_> = barrels
        .iter()
        .flat_map(|(_, s)| s.iter().copied())
        .collect();
    all.sort();
    assert!(all.windows(2).all(|w| w[1] - w[0] >= 8), "{all:?}");
}

#[test]
fn a_held_trigger_fast_enough_for_its_small_guns_still_reaches_the_big_ones() {
    let mut b = battle();
    b.raiders.clear();
    b.next_wave = 1000.;
    let design = shipvivor_shipyard::Design::breakwater();
    let bow = design.gun_stations()[0];
    let he = |inches: u16| MainShell {
        bore: inches - 4,
        ..MainShell::STOCK
    };
    // A twelve-inch turret forward and a four-inch aft. The four-inch loads in 0.3 s, a
    // ripple, so it always had a barrel ready and the twelve-inch never got the trigger.
    b.fit_design(&design, |station| {
        (
            if *station == bow { he(12) } else { he(4) },
            TurretLayout::Twin,
        )
    });
    let mut orders = Orders {
        player: Vec2::ZERO,
        heading: 0.,
        speed: 0.,
        hull: Hull::CRUISER,
        aim: Aim::Track(BEAM_TARGET),
        reload: 0.3,
        shell: he(4),
    };
    for _ in 0..900 {
        b.step(1. / 60., &orders);
    }
    for station in Station::ALL {
        assert_eq!(b.main_mount(station).status(), MountStatus::Ready);
    }
    orders.aim = Aim::Sight(BEAM_TARGET);
    let barrels = hold_fire(&mut b, &orders, 600);
    assert_eq!(barrels.len(), 4);
    let big = b.main_mount(Station::Bow).reload(&orders);
    for (_, shots) in &barrels {
        // A barrel waits no more than its reload and a ripple for the trigger.
        assert!(
            (longest_wait(shots) as f32) < (big + RIPPLE) * 60.,
            "{shots:?}"
        );
    }
}

#[test]
fn a_click_fires_one_barrel_and_a_second_click_fires_the_rest() {
    let (mut b, orders) = broadside();
    let dt = 1. / 60.;
    b.order_volley(Volley::Single);
    b.step(dt, &orders);
    assert_eq!(fired(&b).len(), 1);
    assert_eq!(b.standing_volley(), None);
    assert_eq!(b.next_barrel(), Some((Station::Stern, 0)));
    for _ in 0..10 {
        b.step(dt, &orders);
        assert!(
            fired(&b).is_empty(),
            "a released trigger fires nothing more"
        );
    }
    b.order_volley(Volley::Salvo);
    b.step(dt, &orders);
    assert_eq!(fired(&b).len(), 3);
    assert_eq!(b.next_barrel(), None);
    for _ in 0..30 {
        b.step(dt, &orders);
    }
    assert_eq!(b.standing_volley(), None);
}

#[test]
fn a_click_waits_briefly_for_guns_coming_to_bear_and_lapses_on_a_long_swing() {
    let (mut b, mut orders) = broadside();
    let dt = 1. / 60.;
    // A two-degree correction leaves both mounts training for a few steps.
    orders.aim = Aim::Track(Vec3::new(175., 0., 6.));
    b.order_volley(Volley::Single);
    b.step(dt, &orders);
    assert!(fired(&b).is_empty());
    assert_eq!(b.standing_volley(), Some(Volley::Single));
    let mut waited = dt;
    let mut shots = 0;
    while b.standing_volley().is_some() {
        b.step(dt, &orders);
        shots += fired(&b).len();
        waited += dt;
    }
    assert_eq!(shots, 1);
    assert!(waited < VOLLEY_WINDOW, "{waited}");
    // Swinging 45 degrees takes far longer than the order stands.
    orders.aim = Aim::Track(Vec3::new(120., 0., 120.));
    b.order_volley(Volley::Salvo);
    for _ in 0..(VOLLEY_WINDOW / dt) as usize + 2 {
        b.step(dt, &orders);
        assert!(fired(&b).is_empty());
    }
    assert_eq!(b.standing_volley(), None);
}

#[test]
fn blind_sector_prevents_cross_deck_fire_and_travel_through_the_bridge() {
    let mut battle = battle();
    let mut orders = Orders {
        player: Vec2::ZERO,
        heading: 0.,
        speed: 0.,
        hull: Hull::CRUISER,
        aim: Aim::Sight(Vec3::NEG_Z * 180.),
        reload: 6.4,
        shell: super::MainShell::STOCK,
    };
    for _ in 0..540 {
        battle.step(1. / 60., &orders);
    }
    assert_eq!(
        battle.main_mount(Station::Stern).status(),
        MountStatus::Masked
    );
    let stern = battle.main_mount(Station::Stern);
    assert!((0..stern.layout().count()).all(|t| stern.loaded(t)));
    let bow = battle.main_mount(Station::Bow);
    assert!(!(0..bow.layout().count()).all(|t| bow.loaded(t)));
    // With the same world sight astern after a hull turn, the bow loses its arc.
    orders.heading = std::f32::consts::PI;
    battle.step(1. / 60., &orders);
    assert_eq!(
        battle.main_mount(Station::Bow).status(),
        MountStatus::Masked
    );
    assert_eq!(
        battle.main_mount(Station::Stern).status(),
        MountStatus::Training
    );
    let before = battle.main_mount(Station::Stern).deck_bearing();
    battle.step(1. / 60., &orders);
    assert!(
        (battle.main_mount(Station::Stern).deck_bearing() - before).abs() <= 0.301_f32.to_radians()
    );
}

/// What a ship met while steaming due north from the origin.
#[derive(Default)]
struct Passage {
    damage: u16,
    sunk: u16,
    crunches: usize,
    recovered: u16,
}
/// Steam north for `ticks`, answering each contact the way the helm does.
fn steam(b: &mut Battle, hull: Hull, pos: &mut Vec2, speed: &mut f32, ticks: usize) -> Passage {
    let dt = 1. / 60.;
    let mut seen = Passage::default();
    for _ in 0..ticks {
        *pos += Vec2::NEG_Y * *speed * dt;
        let ev = b.step(
            dt,
            &Orders {
                player: *pos,
                heading: 0.,
                speed: *speed,
                hull,
                aim: Aim::Hold,
                reload: 3.2,
                shell: super::MainShell::STOCK,
            },
        );
        *pos += ev.shove;
        *speed += ev.jolt;
        seen.recovered += ev.recovered;
        seen.damage += ev.damage;
        seen.sunk += ev.sunk;
        seen.crunches += b
            .effects
            .iter()
            .filter(|fx| fx.kind == EffectKind::Crunch && fx.age == 0.)
            .count();
    }
    seen
}
/// A fresh raider lying across the cruiser's track, a few metres off the bow.
fn raider_ahead() -> Battle {
    let mut b = battle();
    b.raiders.clear();
    for gun in &mut b.wing_guns {
        gun.reload = 100.;
    }
    b.spawn(
        EnemyClass::Patrol,
        Vec2::new(0., -22.),
        Vec2::new(10., -22.),
    );
    b
}
#[test]
fn cruiser_at_full_ahead_ploughs_through_a_raider() {
    let mut b = raider_ahead();
    let (mut pos, mut speed) = (Vec2::ZERO, 13.6);
    let seen = steam(&mut b, Hull::CRUISER, &mut pos, &mut speed, 40);
    assert_eq!(seen.sunk, 1);
    assert_eq!(seen.crunches, 1);
    assert!((1..=4).contains(&seen.damage), "hull lost: {}", seen.damage);
    assert!(speed > 11.5, "way kept: {speed}");
    assert!(
        b.effects
            .iter()
            .any(|fx| fx.kind == EffectKind::Wreck(EnemyClass::Patrol))
    );
    assert_eq!(b.salvage.len() + usize::from(seen.recovered), 10);
}
#[test]
fn cruiser_at_half_ahead_cripples_a_raider_and_shoves_it_clear() {
    let mut b = raider_ahead();
    let (mut pos, mut speed) = (Vec2::ZERO, 6.8);
    let seen = steam(&mut b, Hull::CRUISER, &mut pos, &mut speed, 60);
    let raider = &b.raiders[0];
    assert_eq!((seen.sunk, seen.crunches), (0, 1));
    assert_eq!(raider.hull(), 2.);
    assert!(raider.is_disabled());
    assert!(seen.damage <= 2);
    assert!(speed > 5.5);
    // The blow carried the boat ahead of the bow, and it is slowing in the water.
    assert!(raider.body.pos.y < pos.y - Hull::CRUISER.bow);
    assert!(raider.knock.length() < 6.8);
}
#[test]
fn ramming_is_cheap_for_a_heavy_ship_and_ruinous_for_a_light_one() {
    // Bow-on into the beam of a ship lying across the track.
    let strike = |hull: Hull, target: Hull, speed: f32| {
        let a = ram::Ship {
            hull,
            pos: Vec2::ZERO,
            forward: Vec2::NEG_Y,
            velocity: Vec2::NEG_Y * speed,
        };
        let b = ram::Ship {
            hull: target,
            pos: Vec2::NEG_Y * (hull.bow + target.radius - 0.1),
            forward: Vec2::X,
            velocity: Vec2::ZERO,
        };
        ram::contact(&a, &b).expect("hulls overlap")
    };
    // The cruiser's hull at a raider's weight, so only mass differs.
    let light = Hull {
        mass: Hull::RAIDER.mass,
        ..Hull::CRUISER
    };
    let heavy_hit = strike(Hull::CRUISER, Hull::RAIDER, 13.6);
    assert_eq!(heavy_hit.damage, [3, 4]);
    assert!(heavy_hit.kick[0].length() < 0.15 * 13.6);

    let light_hit = strike(light, Hull::RAIDER, 13.6);
    assert!(light_hit.damage[0] >= 4 * heavy_hit.damage[0]);
    assert!(
        light_hit.damage[1] < 4,
        "an even match does not sink outright"
    );
    assert!(light_hit.kick[0].length() > 0.5 * 13.6);

    let into_a_giant = strike(light, Hull::CRUISER, 13.6);
    assert!(into_a_giant.damage[0] >= 20);
    assert!(into_a_giant.damage[0] > 3 * into_a_giant.damage[1]);
    assert!(
        into_a_giant.kick[0].dot(Vec2::NEG_Y) < -13.6,
        "it bounces off"
    );

    // The stem takes a blow better than the beam it strikes.
    let even = strike(Hull::CRUISER, Hull::CRUISER, 13.6);
    assert!(even.damage[0] * 2 <= even.damage[1]);

    let bump = strike(Hull::CRUISER, Hull::RAIDER, 1.5);
    assert_eq!(bump.damage, [0, 0]);
    assert!(((bump.give[1] - bump.give[0]).dot(bump.normal) - 0.1).abs() < 1e-3);
    assert!(bump.give[1].length() > 7. * bump.give[0].length());
}
#[test]
fn hulls_grinding_together_are_hurt_once_per_blow() {
    let mut b = battle();
    b.raiders.clear();
    b.spawn(EnemyClass::Patrol, Vec2::ZERO, Vec2::ZERO);
    let orders = Orders {
        player: Vec2::ZERO,
        heading: 0.,
        speed: 0.,
        hull: Hull::CRUISER,
        aim: Aim::Hold,
        reload: 3.2,
        shell: super::MainShell::STOCK,
    };
    // A raider driving its bow into the cruiser's starboard beam.
    let drive = |b: &mut Battle| {
        let raider = &mut b.raiders[0];
        raider.body.pos = Vec2::X * (Hull::CRUISER.radius + Hull::RAIDER.bow - 0.2);
        raider.body.heading = bearing(Vec2::NEG_X);
        raider.way = Vec2::NEG_X * 5.4;
        let mut events = Events::default();
        b.resolve_contacts(&orders, &mut events);
        events
    };
    let first = drive(&mut b);
    assert_eq!(first.damage, 2);
    assert!(first.shove.x < 0. && first.jolt.abs() < 1e-3);
    assert!(b.raiders[0].knock.x > 5.);
    let grind = drive(&mut b);
    assert_eq!(grind.damage, 0);
    assert!(grind.shove.x < 0., "the hulls still part");
    b.raiders[0].ram_rest = 0.;
    assert_eq!(drive(&mut b).damage, 2);
    let crunches = |b: &Battle| {
        b.effects
            .iter()
            .filter(|fx| fx.kind == EffectKind::Crunch)
            .count()
    };
    assert_eq!(crunches(&b), 2);
    assert_eq!(b.raiders[0].hull(), 4., "its own stem shrugs off the blow");
}
#[test]
fn raiders_shoulder_each_other_apart_without_harm() {
    let mut b = battle();
    b.raiders.clear();
    b.spawn(EnemyClass::Patrol, Vec2::new(100., -100.), Vec2::ZERO);
    b.spawn(EnemyClass::Patrol, Vec2::new(101., -100.), Vec2::ZERO);
    let mut pos = Vec2::ZERO;
    let seen = steam(&mut b, Hull::CRUISER, &mut pos, &mut 0., 1);
    let [a, c] = [&b.raiders[0], &b.raiders[1]];
    assert!(a.body.pos.distance(c.body.pos) > 4.9);
    assert_eq!((a.hull(), c.hull(), seen.crunches), (4., 4., 0));
}

#[test]
fn raids_grow_from_patrol_boats_through_destroyers_and_cruisers_to_battleships() {
    use EnemyClass::*;
    let mut b = battle().with_flagship();
    // The flagship is still in a yard when the sector opens, and coasters and each
    // base's garrison lie at anchor.
    assert_eq!(
        b.raiders
            .iter()
            .filter(|r| r.moored_at().is_none())
            .map(|r| r.class)
            .filter(|class| *class != Coaster)
            .collect::<Vec<_>>(),
        [Patrol, Patrol, Destroyer, Destroyer]
    );
    let observer = Vec2::splat(-500.);
    // Four launches with every yard fully stocked, a given time into the sector.
    let mut raid = |at: f32| {
        let tier = RAID_TIERS.iter().filter(|&&t| at >= t).count();
        for p in &mut b.ports {
            p.work(0., tier);
        }
        // Finish each yard's already committed hull first.
        for _ in 0..3 {
            for p in &mut b.ports {
                p.clear_slips();
                p.stock = YARD_CAPACITY;
            }
            b.reinforce(observer);
            b.raiders.pop().unwrap();
        }
        (0..4)
            .map(|_| {
                for p in &mut b.ports {
                    p.clear_slips();
                    p.stock = YARD_CAPACITY;
                }
                b.reinforce(observer);
                let r = b.raiders.pop().unwrap();
                assert!(r.body.pos.distance(observer) >= SPAWN_STANDOFF);
                r.class
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(raid(0.), [Patrol; 4]);
    let early = raid(RAID_TIERS[0]);
    assert!(early.contains(&Destroyer), "{early:?}");
    assert!(
        early.iter().all(|c| matches!(c, Patrol | Destroyer)),
        "{early:?}"
    );
    let middle = raid(RAID_TIERS[1]);
    assert!(middle.contains(&Cruiser), "{middle:?}");
    assert!(!middle.contains(&Battleship), "{middle:?}");
    let late = raid(RAID_TIERS[2]);
    assert!(late.contains(&Battleship), "{late:?}");
    assert!(
        !late.contains(&Patrol) && !late.contains(&Parliament),
        "{late:?}"
    );
    // Each class is heavier than the last.
    let [p, d, c, bb] = [Patrol, Destroyer, Cruiser, Battleship].map(|k| k.profile());
    assert!(p.hull.mass < d.hull.mass && d.hull.mass < c.hull.mass && c.hull.mass < bb.hull.mass);
}

fn hove_to() -> Orders {
    Orders {
        player: Vec2::ZERO,
        heading: 0.,
        speed: 0.,
        hull: Hull::CRUISER,
        aim: Aim::Hold,
        reload: 3.2,
        shell: super::MainShell::STOCK,
    }
}
fn flagships(b: &Battle) -> usize {
    b.raiders
        .iter()
        .filter(|r| r.class == EnemyClass::Parliament)
        .count()
}

#[test]
fn deeper_waters_start_their_raids_further_along_and_sail_the_flagship_sooner() {
    use EnemyClass::*;
    let b = battle().with_flagship().escalated(120.);
    assert_eq!(b.flagship(), Some(Sortie::Due(FLAGSHIP_SORTIE - 120.)));
    // Either order of building comes to the same, and she never sails straight away.
    let far = battle().escalated(400.).with_flagship();
    assert_eq!(
        (far.flagship(), far.escalation()),
        (Some(Sortie::Due(FLAGSHIP_EARLIEST)), 400.)
    );
    assert_eq!(battle().escalated(-5.).escalation(), 0.);
    // Waters a battleship's head start along lay battleships down on their first step.
    let mut b = battle().escalated(RAID_TIERS[2]);
    b.next_wave = f32::MAX;
    b.step(1. / 60., &hove_to());
    let observer = Vec2::splat(-500.);
    let launches: Vec<EnemyClass> = (0..7)
        .map(|_| {
            for p in &mut b.ports {
                p.clear_slips();
                p.stock = YARD_CAPACITY;
            }
            b.reinforce(observer);
            b.raiders.pop().unwrap().class
        })
        .collect();
    assert!(launches[3..].contains(&Battleship), "{launches:?}");
    assert!(!launches[3..].contains(&Patrol), "{launches:?}");
}

#[test]
fn flagship_puts_to_sea_partway_through_from_the_farthest_yard() {
    let mut b = battle().with_flagship();
    b.raiders.clear();
    b.next_wave = f32::MAX;
    assert_eq!(b.flagship(), Some(Sortie::Due(FLAGSHIP_SORTIE)));
    b.at_sea = FLAGSHIP_SORTIE - 1.;
    b.step(0.5, &hove_to());
    assert_eq!(flagships(&b), 0);
    b.step(0.5, &hove_to());
    assert_eq!(flagships(&b), 1);
    let farthest = (0..b.ports.len())
        .max_by(|&i, &j| {
            let range = |k: usize| b.sortie_station(&b.ports[k]).length();
            range(i).total_cmp(&range(j))
        })
        .unwrap();
    assert_eq!(
        b.flagship(),
        Some(Sortie::Sailed {
            port: farthest,
            at: FLAGSHIP_SORTIE
        })
    );
    let flagship = b
        .raiders
        .iter()
        .find(|r| r.class == EnemyClass::Parliament)
        .unwrap();
    assert!(flagship.body.pos.length() >= SPAWN_STANDOFF);
    // She heads out of the harbour, and only one of her sails.
    let out = b.ports[farthest].body.forward();
    assert!(flagship.body.forward().dot(out) > 0.99);
    b.step(0.5, &hove_to());
    assert_eq!(flagships(&b), 1);
}

#[test]
fn a_commodore_raises_steam_before_she_sails_and_sinks_like_any_battleship() {
    let mut b = battle().with_boss(EnemyClass::Commodore);
    b.raiders.clear();
    b.next_wave = f32::MAX;
    assert_eq!(b.boss(), Some(EnemyClass::Commodore));
    assert!(b.boss_ship().is_none());
    // Smoke goes up over her yard half a minute before she is due, and not before.
    b.at_sea = FLAGSHIP_SORTIE - STEAM_WARNING - 1.;
    b.step(0.5, &hove_to());
    assert_eq!(b.raising_steam(), None);
    b.step(1., &hove_to());
    let due = b.raising_steam().unwrap();
    assert!((STEAM_WARNING - 1. ..STEAM_WARNING).contains(&due), "{due}");
    b.at_sea = FLAGSHIP_SORTIE - 0.25;
    b.step(0.5, &hove_to());
    assert_eq!(b.raising_steam(), None);
    let ship = b.boss_ship().expect("she sailed");
    assert_eq!(ship.class, EnemyClass::Commodore);
    assert!(matches!(b.flagship(), Some(Sortie::Sailed { .. })));
    // A boss, but no flagship proper: no turret screen keeps hits off her hull.
    assert!(ship.class.is_boss() && !ship.class.is_flagship());
    assert!(!ship.protected());
    assert_eq!((ship.guns().len(), ship.secondaries.len()), (4, 16));
    // A boss outlasts a battleship: the same capital plate, and twice the hull.
    let battleship = EnemyClass::Battleship;
    let structure = |class: EnemyClass| class.design().structure();
    assert!(structure(EnemyClass::Commodore) > 1.8 * structure(battleship));
    assert!(structure(EnemyClass::Commodore) < 2.2 * structure(battleship));
    let plate = |class: EnemyClass| (class.belt(), class.strake_plate(), class.turret_plate());
    assert_eq!(plate(EnemyClass::Commodore), plate(battleship));
}

#[test]
fn flagship_waits_for_a_working_yard_clear_of_traffic_and_out_of_sight() {
    let mut b = battle().with_flagship();
    b.raiders.clear();
    b.next_wave = f32::MAX;
    b.at_sea = FLAGSHIP_SORTIE;
    let mut by_range: Vec<usize> = (0..b.ports.len()).collect();
    by_range.sort_by(|&i, &j| {
        let range = |k: usize| b.sortie_station(&b.ports[k]).length();
        range(j).total_cmp(&range(i))
    });
    let [far, middle, near] = by_range[..] else {
        unreachable!()
    };
    // The nearest yard is too close to put her to sea in view of the player.
    assert!(b.sortie_station(&b.ports[near]).length() < SPAWN_STANDOFF);
    b.ports[far].wreck();
    let station = b.sortie_station(&b.ports[middle]);
    b.spawn(EnemyClass::Cruiser, station, station + Vec2::X);
    b.step(0.25, &hove_to());
    assert_eq!(b.flagship(), Some(Sortie::Due(FLAGSHIP_SORTIE)));
    assert_eq!(flagships(&b), 0);
    b.raiders.clear();
    b.step(0.25, &hove_to());
    assert!(
        matches!(b.flagship(), Some(Sortie::Sailed { port, .. }) if port == middle),
        "{:?}",
        b.flagship()
    );
}

#[test]
fn each_gold_mount_takes_independent_swept_hits_before_the_hull_opens() {
    let mut b = battle();
    b.raiders.clear();
    b.salvage.clear();
    let class = EnemyClass::Parliament;
    let pos = Vec2::new(-230., 75.);
    let facing = shipvivor_compass::direction(0.73);
    b.spawn(class, pos, pos + facing);
    let p = class.profile();
    let side = facing.perp();
    let stock = b.raiders[0].hull();
    let hit = |b: &mut Battle, eye: Vec3, point: Vec3, damage| {
        let ray = (point - eye).normalize();
        let sight = b.sight_point(eye, ray, eye.xz() + ray.xz() * 170.);
        assert!(
            sight.distance(point) < 13.,
            "sight found {sight} instead of {point}"
        );
        fire(
            &mut b.shots,
            &mut b.effects,
            &mut b.serial,
            eye,
            sight,
            Battery::Main,
            damage,
        );
        b.advance_shots(1., Vec2::ZERO, &mut Events::default());
        assert!(b.shots.is_empty());
    };
    let stern = (pos - facing * 105.).extend(3.).xzy();
    let keel = (pos - facing * 32.).extend(1.4).xzy();
    // Her capital belt holds a stock round, and her turrets screen her hull from the
    // little that gets through: a heavy blow does her a fraction of a quarter of it.
    hit(&mut b, stern, keel, 8);
    let dented = b.raiders[0].hull();
    let lost = stock - dented;
    let quarter = 8. * super::fleet::SCREENED;
    assert!(lost > 0. && lost < 0.25 * quarter, "{lost}");
    assert_eq!(b.salvo_report(), Some(Impact::Embedded));
    assert_eq!(b.raiders[0].live_guns(), 10);

    for (killed, index) in [4, 1, 8, 0, 2, 9, 5, 7, 3, 6].into_iter().enumerate() {
        let ship = &b.raiders[0];
        let base = ship.guns()[index].base(&ship.body, p);
        let aim = base + Vec3::Y * p.gun_scale;
        let beam = (base.xz() - pos).dot(side);
        let approach = if beam.abs() > 1. {
            side * beam.signum()
        } else {
            (base.xz() - pos).normalize()
        };
        let eye = (base.xz() + approach * 85.).extend(aim.y + 4.).xzy();
        let ray = (aim - eye).normalize();
        let spotted = b.sight(eye, ray, eye.xz() + ray.xz() * 170.);
        assert_eq!(
            (spotted.ship, spotted.turret),
            (Some(class), Some(index + 1))
        );
        let before = ship.live_guns();
        // A gold turret's reinforced plate stands five stock rounds, and it falls to
        // the sixth.
        for _ in 0..5 {
            hit(&mut b, eye, aim, 2);
            assert_eq!(b.raiders[0].live_guns(), before, "station {index}");
        }
        hit(&mut b, eye, aim, 2);
        assert_eq!(b.raiders[0].live_guns(), before - 1, "station {index}");
        assert!(!b.raiders[0].guns[index].live());
        assert_eq!(b.raiders[0].hull(), dented);
        // Eight turrets in ten still screen her hull. With the third down it's open.
        assert_eq!(
            b.raiders[0].screened(),
            killed < 2,
            "{} turrets",
            before - 1
        );
    }
    assert_eq!(
        b.effects
            .iter()
            .filter(|e| e.kind == EffectKind::MountLost(class))
            .count(),
        10
    );
    assert!(b.raiders[0].is_disabled());
    // Stripped, she is still a battleship's hull behind a capital belt: it takes six
    // heavy blows to wreck half her length and send her down.
    for hits in 1..=6 {
        hit(&mut b, stern, keel, 100);
        assert_eq!(b.raiders[0].afloat(), hits < 6, "hit {hits}");
    }
    let orders = Orders {
        aim: Aim::Hold,
        player: Vec2::ZERO,
        heading: 0.,
        speed: 0.,
        hull: Hull::CRUISER,
        shell: super::MainShell::STOCK,
        reload: 6.4,
    };
    assert_eq!(b.step(0.02, &orders).sunk, 1);
    assert_eq!(b.step(0.02, &orders).sunk, 0);
    assert_eq!(b.salvage.len(), 1);
    assert_eq!(b.salvage[0].stores, 12);
    assert!(b.effects.iter().any(|e| e.kind == EffectKind::Wreck(class)));
}

#[test]
fn dismantling_the_near_broadside_removes_its_fire_and_preserves_the_opposite_battery() {
    let mut b = battle();
    b.raiders.clear();
    let class = EnemyClass::Parliament;
    b.spawn(class, Vec2::ZERO, Vec2::NEG_Y);
    // Rounds from her turrets and from her secondaries over forty seconds.
    let fire_for = |b: &mut Battle, side: f32| {
        b.shots.clear();
        b.effects.clear();
        let r = &mut b.raiders[0];
        for _ in 0..60 * 40 {
            r.fire_guns(
                1. / 60.,
                Vec2::X * side * 125.,
                Vec2::ZERO,
                &mut b.shots,
                &mut b.effects,
                &mut b.serial,
            );
        }
        let count = |battery| b.shots.iter().filter(|s| s.battery == battery).count();
        (count(Battery::Raider), count(Battery::EnemySecondary))
    };
    let (main, secondary) = fire_for(&mut b, 1.);
    assert!(main >= 30 && secondary >= 30, "{main} {secondary}");
    let ship = &mut b.raiders[0];
    for index in 0..ship.guns.len() {
        if ship.guns[index].base(&ship.body, class.profile()).x >= 0. {
            ship.hit_gun(index, 7.);
        }
    }
    // The near broadside's turrets are silent. Her secondaries there have crews of
    // their own and fight on.
    let (main, secondary) = fire_for(&mut b, 1.);
    assert_eq!(main, 0);
    assert!(secondary >= 30, "{secondary}");
    assert!(fire_for(&mut b, -1.).0 >= 12);
}

#[test]
fn shooting_away_the_flagships_bridge_slows_her_on_her_course_while_her_turrets_stand() {
    use super::fleet::{CONN_LOST, CONN_PACE};
    let mut b = battle();
    b.raiders.clear();
    b.salvage.clear();
    let class = EnemyClass::Parliament;
    let pos = Vec2::new(-230., 75.);
    let facing = shipvivor_compass::direction(0.73);
    b.spawn(class, pos, pos + facing);
    let p = class.profile();
    let stock = b.raiders[0].hull();
    // From her beam, over the deck and the secondaries, into the tower amidships.
    let bridge = pos.extend(p.deck + 2. * p.gun_scale).xzy();
    let eye = (pos + facing.perp() * 85.).extend(bridge.y + 4.).xzy();
    let shoot = |b: &mut Battle| {
        let (shots, effects, serial) = (&mut b.shots, &mut b.effects, &mut b.serial);
        fire(shots, effects, serial, eye, bridge, Battery::Main, 2);
        b.advance_shots(1., Vec2::ZERO, &mut Events::default());
        assert!(b.shots.is_empty());
    };
    for _ in 0..2 {
        shoot(&mut b);
        assert!(b.raiders[0].bridge_standing());
        assert_eq!(b.raiders[0].helm_lost(), 0.);
    }
    shoot(&mut b);
    let ship = &b.raiders[0];
    assert!(!ship.bridge_standing());
    assert_eq!(ship.helm_lost(), CONN_LOST);
    // The bridge is a target of its own: her hull and turrets are untouched.
    assert_eq!((ship.hull(), ship.live_guns()), (stock, 10));
    assert!(ship.protected() && !ship.is_disabled());

    // The player lies off her beam, so with a helm she would turn towards the player.
    let player = pos + facing.perp() * 400.;
    let sail = |b: &mut Battle, seconds: f32| {
        for _ in 0..(seconds * 60.).round() as usize {
            b.raiders[0].sail(
                1. / 60.,
                player,
                false,
                &b.ports,
                &b.landmarks,
                &b.floe.icebergs,
                Vec2::ZERO,
            );
        }
    };
    let heading = b.raiders[0].body.heading;
    sail(&mut b, 10.);
    assert_eq!(b.raiders[0].body.heading, heading, "she holds her course");
    let slowed = b.raiders[0].motion().length();
    assert!(slowed > 0.);
    sail(&mut b, CONN_LOST - 10. + 0.5);
    assert_eq!(b.raiders[0].helm_lost(), 0.);
    // Conned from aft, she gathers way again.
    sail(&mut b, 15.);
    let full = b.raiders[0].motion().length();
    assert!(
        (slowed / full - CONN_PACE).abs() < 1e-3,
        "{slowed} of {full}"
    );
    sail(&mut b, 5.);
    assert_ne!(
        b.raiders[0].body.heading, heading,
        "she answers the helm again"
    );
    // A bridge is shot away only once, and no other class has one to lose.
    assert!(!b.raiders[0].hit_bridge(100.));
    assert_eq!(b.raiders[0].helm_lost(), 0.);
    b.spawn(
        EnemyClass::Battleship,
        Vec2::new(200., 0.),
        Vec2::new(200., 10.),
    );
    assert!(!b.raiders[1].bridge_standing());
}

#[test]
fn stock_main_battery_can_finish_a_moving_flagship_in_open_water() {
    let mut b = battle();
    b.raiders.clear();
    b.next_wave = 1000.;
    b.spawn(
        EnemyClass::Parliament,
        Vec2::new(220., -30.),
        Vec2::new(220., -90.),
    );
    let orders = Orders {
        aim: Aim::Nearest,
        player: Vec2::ZERO,
        heading: 0.,
        speed: 0.,
        hull: Hull::CRUISER,
        shell: super::MainShell::STOCK,
        reload: 6.4,
    };
    for _ in 0..60 * 150 {
        if b.step(1. / 60., &orders).sunk == 1 {
            return;
        }
    }
    panic!(
        "flagship still has {} mounts and {} hull",
        b.raiders[0].live_guns(),
        b.raiders[0].hull()
    );
}

#[test]
fn cruisers_and_battleships_can_lose_a_turret_or_sink_with_other_guns_intact() {
    for (class, count, barrels, reward) in [
        (EnemyClass::Cruiser, 2, 2, 3),
        (EnemyClass::Battleship, 3, 3, 7),
    ] {
        let mut b = battle();
        b.raiders.clear();
        b.salvage.clear();
        let center = Vec2::new(-75., -125.);
        b.spawn(class, center, center + Vec2::NEG_Y);
        let ship = &b.raiders[0];
        let profile = class.profile();
        assert_eq!((ship.guns().len(), profile.barrels), (count, barrels));
        let spot = ship.guns()[0].base(&ship.body, profile) + Vec3::Y * profile.gun_scale;
        let stock = ship.hull();
        // A round with nothing to spare on her plate, so it bursts behind it.
        let shell = if class == EnemyClass::Battleship {
            MainShell {
                bore: 14,
                ..MainShell::STOCK
            }
        } else {
            MainShell::STOCK
        };
        let shoot = |b: &mut Battle, spot: Vec3| {
            projectile::FlightPath::main(spot + Vec3::X * 75., spot, shell)
                .unwrap()
                .launch(
                    &mut b.shots,
                    &mut b.effects,
                    &mut b.serial,
                    Battery::Main,
                    50,
                );
            b.advance_shots(0.5, Vec2::ZERO, &mut Events::default());
        };
        shoot(&mut b, spot);
        assert_eq!(b.raiders[0].live_guns(), count - 1);
        assert_eq!(b.raiders[0].hull(), stock);
        // One wrecked module does not sink her. Half her vital length does.
        let forward = b.raiders[0].body.forward();
        for (ahead, afloat) in [(0., true), (8., true), (-8., false)] {
            shoot(&mut b, (center + forward * ahead).extend(1.2).xzy());
            assert_eq!(
                b.raiders[0].afloat(),
                afloat,
                "{class:?} hit {ahead} m ahead"
            );
        }
        let orders = Orders {
            player: Vec2::ZERO,
            heading: 0.,
            speed: 0.,
            hull: Hull::CRUISER,
            aim: Aim::Hold,
            reload: 6.4,
            shell: super::MainShell::STOCK,
        };
        assert_eq!(b.step(0.01, &orders).sunk, 1);
        assert_eq!(b.salvage.len(), 1);
        assert_eq!(b.salvage[0].stores, reward);
    }
}

#[test]
fn plate_that_holds_a_round_is_cratered_and_still_lets_a_little_through() {
    let mut b = battle();
    b.raiders.clear();
    let center = Vec2::new(-75., -125.);
    b.spawn(EnemyClass::Battleship, center, center + Vec2::NEG_Y);
    let shoot = |b: &mut Battle, shell: MainShell, target: Vec3, side: Vec2| {
        b.effects.clear();
        projectile::FlightPath::main(target + side.extend(0.).xzy() * 82., target, shell)
            .unwrap()
            .launch(
                &mut b.shots,
                &mut b.effects,
                &mut b.serial,
                Battery::Main,
                shell.breach_force(),
            );
        b.advance_shots(0.5, Vec2::ZERO, &mut Events::default());
        b.effects
            .iter()
            .find_map(|e| match e.kind {
                EffectKind::Armor { outcome, .. } => Some(outcome),
                _ => None,
            })
            .unwrap()
    };
    let belt = center.extend(1.4).xzy();
    // A stock round only craters the capital strake on her side, but every one gets a
    // little of its blast through, a bit more as the plate there thins.
    let mut before = b.raiders[0].hull();
    let mut losses = vec![];
    for _ in 0..3 {
        assert_eq!(
            shoot(&mut b, MainShell::STOCK, belt, Vec2::X),
            Impact::Embedded
        );
        losses.push(before - b.raiders[0].hull());
        before = b.raiders[0].hull();
    }
    assert!(losses[0] > 0.1 && losses[0] < 0.5, "{losses:?}");
    assert!(losses.windows(2).all(|w| w[1] >= w[0]), "{losses:?}");
    let scars = b.raiders[0].scars();
    assert_eq!(scars.len(), 1);
    assert_eq!(scars[0].impact, Impact::Embedded);
    // Her turret's plate holds it too, and the gun fights on.
    let turret =
        b.raiders[0].guns[0].base(&b.raiders[0].body, EnemyClass::Battleship.profile()) + Vec3::Y;
    assert_eq!(
        shoot(&mut b, MainShell::STOCK, turret, Vec2::X),
        Impact::Embedded
    );
    assert!(b.raiders[0].guns()[0].live());
    // Her upperworks are light, and a round there silences her director and starts to
    // wreck them, though it does her hull no harm.
    let director = center.extend(7.).xzy();
    let hull = b.raiders[0].hull();
    assert_eq!(
        shoot(&mut b, MainShell::STOCK, director, Vec2::X),
        Impact::Penetrated
    );
    assert!(b.raiders[0].director_lost);
    assert!(b.raiders[0].mangled() > 0.);
    assert_eq!(b.raiders[0].hull(), hull);
    // An eighteen-inch round opens the capital strake on her other side and shakes it
    // loose. The thin plate behind it gives to a stock round.
    let eighteen = MainShell {
        bore: 14,
        ..MainShell::STOCK
    };
    assert_eq!(
        shoot(&mut b, eighteen, belt, Vec2::NEG_X),
        Impact::Penetrated
    );
    assert_eq!(b.raiders[0].strakes().iter().filter(|s| !s.on()).count(), 1);
    assert_eq!(
        shoot(&mut b, MainShell::STOCK, belt + Vec3::Y * 0.3, Vec2::NEG_X),
        Impact::Penetrated
    );
}

#[test]
fn stock_rounds_her_belt_holds_shake_a_strake_loose_and_bare_the_hull() {
    let center = Vec2::new(-75., -125.);
    let target = center.extend(1.4).xzy();
    let fresh = || {
        let mut b = battle();
        b.raiders.clear();
        b.spawn(EnemyClass::Battleship, center, center + Vec2::NEG_Y);
        b
    };
    // What became of one stock round from 82 m off on the side `from`.
    let shoot = |b: &mut Battle, from: Vec3| {
        b.effects.clear();
        let shell = MainShell::STOCK;
        projectile::FlightPath::main(target + from * 82., target, shell)
            .unwrap()
            .launch(
                &mut b.shots,
                &mut b.effects,
                &mut b.serial,
                Battery::Main,
                shell.breach_force(),
            );
        b.advance_shots(0.5, Vec2::ZERO, &mut Events::default());
        b.effects
            .iter()
            .find_map(|e| match e.kind {
                EffectKind::Armor { outcome, .. } => Some(outcome),
                _ => None,
            })
            .unwrap()
    };
    let loose = |b: &Battle| b.raiders[0].strakes().iter().filter(|s| !s.on()).count();
    let mut b = fresh();
    let intact = b.raiders[0].hull();
    assert_eq!(b.raiders[0].strakes().len(), 2 * STRAKES_A_SIDE);
    // Her capital strake holds a stock round, which only does her a little harm, but
    // every round wears the strake it strikes, and the sixth shakes it loose.
    let mut before = intact;
    for round in 1..=6 {
        assert_eq!(shoot(&mut b, Vec3::X), Impact::Embedded, "round {round}");
        let lost = before - b.raiders[0].hull();
        assert!(lost > 0. && lost < 1., "round {round}: {lost}");
        before = b.raiders[0].hull();
        assert_eq!(loose(&b), usize::from(round == 6), "round {round}");
    }
    let held = b.raiders[0].hull();
    assert!(
        b.effects
            .iter()
            .any(|e| matches!(e.kind, EffectKind::StrakeLost(EnemyClass::Battleship, _)))
    );
    // Only thin plate lies behind it, and the same guns open that.
    assert_eq!(shoot(&mut b, Vec3::X), Impact::Penetrated);
    assert!(held - b.raiders[0].hull() > 1.5);
    // The strake on her other side still holds.
    assert_eq!(shoot(&mut b, Vec3::NEG_X), Impact::Embedded);
    assert_eq!(loose(&b), 1);

    // Light guns' rounds chip a strake loose too, given enough of them.
    let mut b = fresh();
    for _ in 0..30 {
        let (shots, effects, serial) = (&mut b.shots, &mut b.effects, &mut b.serial);
        fire(
            shots,
            effects,
            serial,
            target + Vec3::X * 82.,
            target,
            Battery::Secondary,
            3,
        );
        b.advance_shots(0.5, Vec2::ZERO, &mut Events::default());
    }
    let struck = b.effects.iter().filter(|e| {
        matches!(
            e.kind,
            EffectKind::Armor {
                battery: Battery::Secondary,
                ..
            }
        )
    });
    assert!(struck.count() > 8);
    assert_eq!(loose(&b), 1);
}

#[test]
fn a_flagships_turrets_screen_her_hull_until_a_fifth_of_them_fall() {
    use super::fleet::SCREENED;
    let fresh = || {
        let mut b = battle();
        b.raiders.clear();
        b.spawn(
            EnemyClass::Parliament,
            Vec2::new(-230., 75.),
            Vec2::new(-230., 0.),
        );
        b
    };
    // Hull lost to a blow of 4 on her side amidships.
    let lost = |b: &mut Battle, bare: bool| {
        let r = &mut b.raiders[0];
        let before = r.hull();
        let at = r.body.pos + r.body.forward().perp() * 5.;
        if bare {
            r.hit_bare_hull(at, 4.);
        } else {
            r.hit_hull(at, 4.);
        }
        before - r.hull()
    };
    let mut b = fresh();
    assert!(b.raiders[0].screened());
    let screened = lost(&mut b, false);
    assert!((screened - 4. * SCREENED).abs() < 1e-3, "{screened}");
    let bare = lost(&mut fresh(), true);
    assert!((bare - 4.).abs() < 1e-3, "{bare}");
    // With three of her ten turrets down the rest no longer screen her, though she
    // fights on.
    let mut b = fresh();
    for gun in 0..3 {
        b.raiders[0].hit_gun(gun, 7.);
        assert_eq!(b.raiders[0].screened(), gun < 2, "{gun}");
    }
    assert!(b.raiders[0].protected());
    let open = lost(&mut b, false);
    assert!((open - 4.).abs() < 1e-3, "{open}");
}

#[test]
fn a_strake_shot_off_a_flagships_side_lets_rounds_in_past_her_turret_screen() {
    use super::fleet::SCREENED;
    let center = Vec2::new(-75., -125.);
    let target = center.extend(1.4).xzy();
    // A six-inch round, 4 damage, which her capital strakes hold.
    let shell = MainShell {
        bore: 2,
        ..MainShell::STOCK
    };
    // Hull lost to one round from abeam, with the strake there shot off first or not.
    let lost = |strip: bool| {
        let mut b = battle();
        b.raiders.clear();
        b.spawn(EnemyClass::Parliament, center, center + Vec2::NEG_Y);
        let r = &mut b.raiders[0];
        let side = r.body.forward().perp().extend(0.).xzy();
        if strip {
            let skin = target + side * r.class.profile().hull.radius;
            let round = MainShell::STOCK.round_at(0.);
            let struck = r.strike_hull(skin, -side, round, false, 18., 0.9, 0.);
            assert!(struck.loosed.is_some());
        }
        assert!(r.screened());
        let intact = r.hull();
        projectile::FlightPath::main(target + side * 82., target, shell)
            .unwrap()
            .launch(
                &mut b.shots,
                &mut b.effects,
                &mut b.serial,
                Battery::Main,
                shell.breach_force(),
            );
        b.advance_shots(0.5, Vec2::ZERO, &mut Events::default());
        intact - b.raiders[0].hull()
    };
    // The strake holds it, and her turrets screen what gets through.
    let screened = lost(false);
    assert!(screened > 0.2 && screened < 4. * SCREENED, "{screened}");
    // The thin plate behind a strake shot off lets the whole round in, past the screen.
    let bare = lost(true);
    assert!((bare - 4.).abs() < 1e-3, "{bare}");
}

#[test]
fn a_round_with_penetration_to_spare_can_go_through_one_ship_into_another() {
    let center = Vec2::new(-75., -125.);
    let target = center.extend(1.2).xzy();
    // What one round from 82 m abeam did to the near destroyer and to the one beyond
    // her, fired with serial `serial`.
    let fire = |shell: MainShell, serial: u64| {
        let mut b = battle();
        b.raiders.clear();
        b.landmarks.clear();
        b.spawn(EnemyClass::Destroyer, center, center + Vec2::NEG_Y);
        let beyond = center + Vec2::NEG_X * 11.;
        b.spawn(EnemyClass::Destroyer, beyond, beyond + Vec2::NEG_Y);
        let intact: Vec<f32> = b.raiders.iter().map(Raider::hull).collect();
        b.serial += serial;
        projectile::FlightPath::main(target + Vec3::X * 82., target, shell)
            .unwrap()
            .launch(
                &mut b.shots,
                &mut b.effects,
                &mut b.serial,
                Battery::Main,
                shell.breach_force(),
            );
        for _ in 0..10 {
            b.advance_shots(0.05, Vec2::ZERO, &mut Events::default());
        }
        let lost: Vec<f32> = (b.raiders.iter().zip(&intact))
            .map(|(r, was)| was - r.hull())
            .collect();
        (lost[0], lost[1])
    };
    // A four-inch round has nothing to spare on a destroyer's light plate: it bursts
    // inside her every time.
    for serial in 0..12 {
        let (near, far) = fire(MainShell::STOCK, serial * 7);
        assert!((near - 2.).abs() < 1e-3 && far == 0., "{near} {far}");
    }
    // An eighteen-inch round sometimes goes clean through her, doing her less, and on
    // into the ship beyond.
    let eighteen = MainShell {
        bore: 14,
        ..MainShell::STOCK
    };
    let rounds: Vec<(f32, f32)> = (0..12).map(|serial| fire(eighteen, serial * 7)).collect();
    let through: Vec<_> = rounds.iter().filter(|(_, far)| *far > 0.).collect();
    assert!(
        !through.is_empty() && through.len() < rounds.len(),
        "{rounds:?}"
    );
    for (near, far) in through {
        assert!(*near > 0. && *far > 0., "{near} {far}");
    }
}

#[test]
fn a_shallow_round_skips_off_heavy_plate_and_flies_on() {
    let center = Vec2::new(-75., -125.);
    let mut b = battle();
    b.raiders.clear();
    b.landmarks.clear();
    b.spawn(EnemyClass::Battleship, center, center + Vec2::NEG_Y);
    let r = &mut b.raiders[0];
    let side = r.body.forward().perp().extend(0.).xzy();
    let point = r.body.pos + r.body.forward().perp() * r.class.profile().hull.radius;
    let hit = point.extend(1.4).xzy();
    let tangent = (r.body.forward().extend(0.).xzy() - side * 0.06).normalize();
    let round = MainShell::STOCK.round_at(0.2);
    assert_eq!(
        r.strike_hull(hit, tangent, round, false, 0., 0.9, 0.)
            .impact,
        Impact::Bounced
    );
    // Square on, the same plate holds the same round.
    assert_eq!(
        r.strike_hull(hit, -side, round, false, 0., 0.9, 0.).impact,
        Impact::Embedded
    );
    // Every main-battery round is wide enough to bite into light plate at any angle.
    let mut light = battle();
    let cruiser = &mut light.raiders[1];
    let edge =
        cruiser.body.pos + cruiser.body.forward().perp() * cruiser.class.profile().hull.radius;
    let across = cruiser.body.forward().perp().extend(0.).xzy();
    let graze = (cruiser.body.forward().extend(0.).xzy() - across * 0.06).normalize();
    assert!(
        (cruiser
            .strike_hull(edge.extend(1.).xzy(), graze, round, false, 0., 0.9, 0.)
            .impact)
            .opened()
    );
    // A round fired along her side skips off and tumbles on as a round of its own.
    let start = hit + tangent * -80. + side * 0.6;
    let shell = MainShell::STOCK;
    projectile::FlightPath::main(start, hit - side * 0.02, shell)
        .unwrap()
        .launch(
            &mut b.shots,
            &mut b.effects,
            &mut b.serial,
            Battery::Main,
            shell.breach_force(),
        );
    let fired = b.shots.last().unwrap().body.id;
    for _ in 0..10 {
        b.advance_shots(0.03, Vec2::ZERO, &mut Events::default());
        if !b.shots.iter().any(|s| s.body.id == fired) {
            break;
        }
    }
    assert!(b.effects.iter().any(|e| matches!(
        e.kind,
        EffectKind::Armor {
            outcome: Impact::Bounced,
            ..
        }
    )));
    assert!(
        b.shots
            .iter()
            .any(|s| s.battery == Battery::Main && s.body.id != fired)
    );
}

#[test]
fn velocity_fitting_changes_flight_and_plate_outcome_at_the_same_distance() {
    let target = Vec3::new(0., 1., -315.);
    let mount = Vec3::new(0., 2., 0.);
    let mut heights = Vec::new();
    for velocity in [0, 1] {
        // A capped twelve-inch round falls just short of opening her capital strake
        // this far out, and gets through it a little faster.
        let shell = MainShell {
            bore: 8,
            caps: 1,
            velocity,
        };
        let path = projectile::FlightPath::main(mount, target, shell).unwrap();
        heights.push(path.pitch);
        let mut b = battle();
        b.raiders.clear();
        b.landmarks.clear(); // Compare the same unobstructed shot for both fittings.
        let center = target.xz();
        b.spawn(EnemyClass::Battleship, center, center + Vec2::X);
        path.launch(
            &mut b.shots,
            &mut b.effects,
            &mut b.serial,
            Battery::Main,
            shell.breach_force(),
        );
        b.advance_shots(0.83, Vec2::ZERO, &mut Events::default());
        if velocity == 0 {
            assert_eq!(b.shots.len(), 1);
            b.advance_shots(0.1, Vec2::ZERO, &mut Events::default());
        }
        let desired = if velocity == 0 {
            Impact::Embedded
        } else {
            Impact::Penetrated
        };
        assert!(
            b.effects
                .iter()
                .any(|e| matches!(e.kind,EffectKind::Armor{outcome,..} if outcome==desired))
        );
    }
    assert!(heights[1] < heights[0]);
}

#[test]
fn losing_the_director_reduces_the_battleships_sustained_fire() {
    fn volleys(director_lost: bool) -> usize {
        let mut b = battle();
        b.raiders.clear();
        b.spawn(EnemyClass::Battleship, Vec2::ZERO, Vec2::NEG_Y);
        let r = &mut b.raiders[0];
        if director_lost {
            r.disable_director();
        }
        for _ in 0..60 * 75 {
            r.fire_guns(
                1. / 60.,
                Vec2::X * 95.,
                Vec2::ZERO,
                &mut b.shots,
                &mut b.effects,
                &mut b.serial,
            );
        }
        b.shots
            .iter()
            .filter(|s| s.battery == Battery::Raider)
            .count()
    }
    let intact = volleys(false);
    let impaired = volleys(true);
    assert!(impaired > 0 && impaired * 4 < intact * 3);
}

fn distant_observer() -> Orders {
    Orders {
        player: Vec2::splat(-1800.),
        heading: 0.,
        speed: 0.,
        hull: Hull::CRUISER,
        aim: Aim::Hold,
        reload: 6.,
        shell: MainShell::STOCK,
    }
}

#[test]
fn the_crew_works_both_beams_at_range_while_the_main_battery_holds() {
    let mut b = battle();
    b.raiders.clear();
    b.landmarks.clear();
    let orders = Orders {
        player: Vec2::ZERO,
        ..distant_observer()
    };
    for side in [-1., 1.] {
        b.spawn(EnemyClass::Cruiser, Vec2::X * side * 140., Vec2::ZERO);
    }
    let mut shots = [0; 2];
    for _ in 0..180 {
        b.step(1. / 60., &orders);
        for fx in b.effects().iter().filter(|e| e.age == 0.) {
            assert_ne!(fx.kind, EffectKind::Muzzle(Battery::Main));
            if fx.kind == EffectKind::Muzzle(Battery::Secondary) {
                shots[usize::from(fx.body.pos.x > 0.)] += 1;
            }
        }
    }
    assert!(shots.iter().all(|&count| count >= 3), "{shots:?}");
}

#[test]
fn secondary_mounts_increase_by_ship_class_and_fire_from_separate_stations() {
    for (class, count) in [
        (EnemyClass::Patrol, 0),
        (EnemyClass::Cruiser, 4),
        (EnemyClass::Battleship, 12),
        (EnemyClass::Parliament, 20),
    ] {
        let mut b = battle();
        b.raiders.clear();
        b.spawn(class, Vec2::ZERO, Vec2::NEG_Y);
        let r = &mut b.raiders[0];
        assert_eq!(r.secondaries.len(), count);
        for _ in 0..300 {
            r.fire_guns(
                1. / 60.,
                Vec2::X * 130.,
                Vec2::ZERO,
                &mut b.shots,
                &mut b.effects,
                &mut b.serial,
            );
        }
        let fired: Vec<_> = b
            .effects
            .iter()
            .filter(|e| e.kind == EffectKind::Muzzle(Battery::EnemySecondary))
            .collect();
        for gun in &r.secondaries {
            let station = gun.station(&r.body, class.profile()).xz();
            let fired_here = fired.iter().any(|fx| fx.body.pos.distance(station) <= 1.81);
            assert_eq!(fired_here, station.x > 0., "{class:?} {station:?}");
        }
    }
}

#[test]
fn one_tow_remembers_its_catch_and_pays_the_entire_package_once() {
    let mut b = battle();
    b.salvage.clear();
    b.raiders.clear();
    let mut orders = distant_observer();
    b.salvage.push(Package::new(
        body(&mut b.serial, orders.player + Vec2::X * 100., 0.),
        7,
    ));
    assert_eq!(b.step(0.1, &orders).recovered, 0);
    assert!(!b.salvage[0].hooked);
    orders.player += Vec2::X * 6.;
    assert_eq!(b.step(0.1, &orders).recovered, 0);
    assert!(b.salvage[0].hooked);
    orders.player -= Vec2::X * 25.;
    let reward: u16 = (0..70).map(|_| b.step(0.1, &orders).recovered).sum();
    assert_eq!(reward, 7);
    assert!(b.salvage.is_empty());
}

#[test]
fn direct_rewards_float_no_packages_and_pay_for_each_wreck_and_fallen_building() {
    let mut b = battle().with_rewards(Rewards::Direct);
    assert_eq!(b.rewards(), Rewards::Direct);
    assert!(b.salvage.is_empty(), "the loose packages are cleared");
    b.raiders.clear();
    let orders = distant_observer();
    b.spawn(EnemyClass::Freighter, Vec2::new(-75., -125.), Vec2::ZERO);
    scuttle(&mut b.raiders[0]);
    let e = b.step(1. / 60., &orders);
    assert_eq!((e.sunk, e.recovered), (1, 4));
    assert!(b.salvage.is_empty(), "the wreck leaves nothing to tow");

    // Shell a building until it falls, and total what it paid.
    let shell = |b: &mut Battle, eye: Vec3, target: Vec3| {
        fire(
            &mut b.shots,
            &mut b.effects,
            &mut b.serial,
            eye,
            target,
            Battery::Main,
            2,
        );
        let mut events = Events::default();
        b.advance_shots(0.4, Vec2::ZERO, &mut events);
        events.recovered
    };
    let pos = b.ports[2].factory_position(0);
    let approach = (pos - b.ports[2].body.pos).normalize();
    let target = pos.extend(8.5).xzy();
    let eye = target + (60. * approach).extend(0.).xzy();
    let paid: u16 = (0..4).map(|_| shell(&mut b, eye, target)).sum();
    assert!(!b.ports[2].factories()[0].live());
    assert_eq!(paid, u16::from(FACTORY_STORES));

    // A drydock pays for itself. Its factory stands, and pays when it falls in turn.
    assert_eq!(b.ports[0].live_factories(), 1);
    let mut paid = 0;
    for _ in 0..12 {
        let p = &b.ports[0];
        let yard = p.dock_position(0);
        let target = yard.extend((p.dock_top(0) + 3.2) * 0.5).xzy();
        let eye = target + (p.body.forward() * 60.).extend(0.).xzy();
        paid += shell(&mut b, eye, target);
    }
    assert_eq!(b.ports[0].docks()[0].state(), YardState::Ruined);
    assert_eq!(b.ports[0].live_factories(), 1);
    assert_eq!(paid, u16::from(DRYDOCK_STORES));
}

#[test]
fn packages_pay_nothing_until_towed_and_buildings_pay_nothing() {
    let mut b = battle();
    assert_eq!(b.rewards(), Rewards::Packages);
    let loose = b.salvage.len();
    assert!(loose > 0);
    b.raiders.clear();
    b.spawn(EnemyClass::Freighter, Vec2::new(-75., -125.), Vec2::ZERO);
    scuttle(&mut b.raiders[0]);
    let e = b.step(1. / 60., &distant_observer());
    assert_eq!((e.sunk, e.recovered), (1, 0));
    assert_eq!(b.salvage.len(), loose + 1);
    for _ in 0..12 {
        let p = &b.ports[0];
        let yard = p.dock_position(0);
        let target = yard.extend((p.dock_top(0) + 3.2) * 0.5).xzy();
        let eye = target + (p.body.forward() * 60.).extend(0.).xzy();
        fire(
            &mut b.shots,
            &mut b.effects,
            &mut b.serial,
            eye,
            target,
            Battery::Main,
            2,
        );
        let mut events = Events::default();
        b.advance_shots(0.4, Vec2::ZERO, &mut events);
        assert_eq!(events.recovered, 0);
    }
    assert_eq!(b.ports[0].docks()[0].state(), YardState::Ruined);
}

#[test]
fn inbound_cargo_unloads_material_and_repairs_the_surviving_dock() {
    let mut b = battle();
    b.raiders.clear();
    b.next_wave = 1000.;
    b.ports[2].wreck();
    // The second yard stands to send the freighter, and wants nothing itself. No yard
    // sends coasters.
    b.ports[1].convoy_clock = 1000.;
    for p in &mut b.ports {
        p.coaster_clock = 1e4;
    }
    let observer = distant_observer();
    let port = &mut b.ports[0];
    port.stock = 2;
    port.clear_slips();
    b.reinforce(observer.player);
    assert_eq!(b.ports[0].stock, 0);
    assert_eq!(b.raiders.len(), 1);
    b.raiders.clear();
    let port = &mut b.ports[0];
    port.damage_dock(0, 17);
    port.damage_factory(0, 8);
    port.turrets[0].strength = 0;
    port.convoy_clock = 0.;
    b.step(0.1, &observer);
    assert_eq!(b.raiders.len(), 1, "no escort comes in from the open sea");
    let freight = &b.raiders[0];
    assert_eq!(freight.class, EnemyClass::Freighter);
    assert!(
        freight.body.pos.distance(b.ports[1].berth()) < 30.,
        "she casts off from the other yard's slip"
    );
    // Steel in the storehouse and in the hull on the slip.
    let steel = |b: &Battle| b.ports[0].stock + b.ports[0].docks()[0].laid();
    let mut before = 0;
    for _ in 0..4000 {
        before = steel(&b);
        let e = b.step(0.1, &observer);
        assert_eq!((e.sunk, e.recovered), (0, 0));
        if b.raiders.is_empty() {
            break;
        }
        assert!(
            steel(&b) <= before + 1,
            "only residual income before delivery"
        );
    }
    assert!(b.raiders.is_empty(), "freighter failed to dock");
    assert_eq!(steel(&b), before + 6);
    assert_eq!(b.ports[0].live_factories(), 0);
    assert!(b.ports[0].turrets()[0].live());
    assert_eq!(b.ports[0].docks()[0].state(), YardState::Damaged);
}

#[test]
fn sighted_yard_loses_bays_to_shells_and_a_wrecked_yard_stays_closed() {
    let mut b = battle();
    b.raiders.clear();
    let mut felled = 0;
    for salvo in 0..12 {
        let p = &b.ports[0];
        let yard = p.dock_position(0);
        let f = p.body.forward();
        let target = yard.extend((p.dock_top(0) + 3.2) * 0.5).xzy();
        let eye = target + (f * 60.).extend(0.).xzy();
        let sight = b.sight(eye, (target - eye).normalize(), yard - f * 28.);
        assert_eq!(sight.yard, Some((0, 0)));
        fire(
            &mut b.shots,
            &mut b.effects,
            &mut b.serial,
            eye,
            target,
            Battery::Main,
            2,
        );
        let mut events = Events::default();
        b.advance_shots(0.4, Vec2::ZERO, &mut events);
        felled += events.docks_felled;
        if salvo == 3 {
            assert_eq!(b.ports[0].docks()[0].state(), YardState::Damaged);
        }
        if salvo == 7 {
            assert_eq!(b.ports[0].docks()[0].state(), YardState::Burning);
        }
    }
    assert_eq!(felled, 1);
    // The dock is down, but the cannery's factory stands to raise it, and its gun fights on.
    let p = &mut b.ports[0];
    assert_eq!(p.docks()[0].state(), YardState::Ruined);
    assert!(!p.wrecked());
    assert!(p.turrets().iter().all(ShoreTurret::live));
    // With the factory down too the base is wrecked: no delivery lands there.
    assert!(p.damage_factory(0, u16::MAX));
    assert!(p.wrecked());
    p.unload();
    assert_eq!(p.stock, 0);
    let collapsed = |e: &&Effect| e.kind == EffectKind::Demolition { dock: true };
    assert_eq!(b.effects.iter().filter(collapsed).count(), 1);
    let elsewhere = b.ports[1].stock;
    b.advance_effects(2.2);
    assert_eq!(b.effects.iter().filter(collapsed).count(), 1);
    b.advance_effects(1.5);
    assert_eq!(b.effects.iter().filter(collapsed).count(), 0);
    assert_eq!(b.ports[1].stock, elsewhere);
}

#[test]
fn sinking_a_supply_ship_at_the_berth_denies_the_delivery() {
    let mut b = battle();
    b.raiders.clear();
    let orders = distant_observer();
    let port = &mut b.ports[0];
    port.stock = 0;
    let berth = port.berth();
    let start = berth + port.body.forward() * 400.;
    b.spawn(EnemyClass::Freighter, berth, start);
    let r = &mut b.raiders[0];
    r.cargo_run = Some((0, start));
    scuttle(r);
    let e = b.step(1. / 60., &orders);
    assert_eq!(e.sunk, 1);
    assert_eq!(b.ports[0].stock, 0);
    assert_eq!(b.salvage.last().unwrap().stores, 4);
}

#[test]
fn each_exposed_secondary_can_be_silenced_without_opening_the_capital_belt() {
    let mut b = battle();
    b.raiders.clear();
    b.landmarks.clear();
    let center = Vec2::ZERO;
    b.spawn(EnemyClass::Battleship, center, Vec2::NEG_Y);
    let ship = &b.raiders[0];
    let stock = ship.hull();
    let gun = ship.secondaries.len() / 2 + 1;
    let mount = ship.secondaries[gun].station(&ship.body, ship.class.profile()) + Vec3::Y * 0.65;
    let eye = mount + Vec3::X * 32.;
    assert_eq!(
        b.sight(eye, Vec3::NEG_X, mount.xz() - Vec2::X * 30.).turret,
        Some(ship.guns.len() + gun + 1)
    );
    fire(
        &mut b.shots,
        &mut b.effects,
        &mut b.serial,
        eye,
        mount,
        Battery::Main,
        2,
    );
    b.advance_shots(0.15, Vec2::splat(-600.), &mut Events::default());
    let ship = &b.raiders[0];
    assert!(!ship.secondaries[gun].live());
    assert_eq!(ship.hull(), stock);
    assert_eq!(ship.secondaries.iter().filter(|g| g.live()).count(), 11);
}

#[test]
fn inbound_cargo_withdraws_when_its_yard_falls() {
    let mut b = battle();
    b.raiders.clear();
    b.next_wave = 1000.;
    let observer = distant_observer();
    b.ports[2].wreck();
    b.ports[1].convoy_clock = 1000.;
    b.ports[0].convoy_clock = 0.;
    for _ in 0..160 {
        b.step(0.1, &observer);
    }
    assert_eq!(
        b.raiders.len(),
        1,
        "a freighter, and no escort from the open sea"
    );
    let (index, origin) = b.raiders[0].cargo_run.unwrap();
    assert_eq!(index, 0);
    assert!(origin.distance(b.ports[1].berth()) < 30.);
    assert!(origin.distance(b.raiders[0].body.pos) > 90.);
    b.ports[index].wreck();
    for _ in 0..1500 {
        assert_eq!(b.step(0.1, &observer).sunk, 0);
        if b.raiders.iter().all(|r| r.cargo_run.is_none()) {
            break;
        }
    }
    assert!(b.raiders.iter().all(|r| r.cargo_run.is_none()));
    assert_eq!(b.ports[index].stock, 0);
    // Its guns are their own matter, and stand.
    assert!(b.ports[index].turrets().iter().all(ShoreTurret::live));
}

#[test]
fn no_ship_comes_in_from_the_sea_once_the_other_yards_are_wrecked() {
    let mut b = battle();
    b.raiders.clear();
    b.next_wave = 1000.;
    let observer = distant_observer();
    for p in &mut b.ports[1..] {
        p.wreck();
    }
    // The last yard is battered and short of stock, and nothing can bring it any.
    b.ports[0].damage_dock(0, 17);
    b.ports[0].convoy_clock = 0.;
    for _ in 0..1200 {
        b.step(0.1, &observer);
    }
    assert!(b.raiders.is_empty(), "{} ships appeared", b.raiders.len());
}

#[test]
fn each_picket_lies_at_one_of_three_stations_the_seed_picks() {
    let station = Vec2::new(120., -380.);
    assert_eq!(
        picket_station(None, 0, station),
        station,
        "a hand-set chart"
    );
    let mut seen = std::collections::HashSet::new();
    for seed in 0..64_u64 {
        for index in 0..4 {
            let at = picket_station(Some(seed), index, station);
            assert_eq!(at, picket_station(Some(seed), index, station));
            assert!(at.length() >= station.length(), "never nearer the entry");
            seen.insert((at.x.round() as i32, at.y.round() as i32));
        }
    }
    assert_eq!(seen.len(), PICKET_SHIFTS.len());
}

#[test]
fn torpedo_racks_share_the_displayed_sectors_and_load_independently() {
    let mut b = battle();
    let player = Vec2::splat(3000.);
    let heading = std::f32::consts::TAU - 0.2;
    assert!(!b.launch_spread(player, heading, heading));
    for side in Rack::BOTH {
        let beam = side.center(heading);
        assert_eq!(
            Rack::for_aim(heading, beam - std::f32::consts::TAU),
            Some(side)
        );
        assert!(!b.launch_spread(player, heading, beam + RACK_HALF_ARC));
        assert!(b.launch_spread(player, heading, beam));
        assert!(!b.launch_spread(player, heading, beam));
        let fan = &b.torpedoes()[b.torpedoes.len() - 3..];
        for (fish, offset) in fan.iter().zip([-SPREAD_HALF_ARC, 0., SPREAD_HALF_ARC]) {
            assert_eq!(fish.body.pos, side.origin(player, heading));
            assert!((fish.body.heading - beam - offset).abs() < 1e-5);
        }
    }
    b.advance_torpedoes(20., player, &mut Events::default());
    assert!(b.torpedoes.is_empty());
    assert!(b.launch_spread(player, heading, Rack::Port.center(heading)));
}

#[test]
fn a_spread_hits_the_first_ship_once_and_land_stops_it() {
    let mut b = battle();
    b.raiders.clear();
    b.landmarks.clear();
    let p = Vec2::splat(2000.);
    let bearing = Rack::Starboard.center(0.);
    for range in [130., 200.] {
        b.spawn(
            EnemyClass::Patrol,
            p + Vec2::X * range,
            p + Vec2::X * (range + 10.),
        );
    }
    assert!(b.launch_spread(p, 0., bearing));
    let mut ev = Events::default();
    b.advance_torpedoes(8., p, &mut ev);
    assert_eq!(ev.hits, 1);
    assert_eq!(b.raiders[0].hull(), 0.);
    assert!(b.raiders[1].hull() > 0.);
    let report = b.step(0.01, &distant_observer());
    assert_eq!((report.hits, report.sunk), (0, 1));
    b.advance_torpedoes(15., p, &mut ev);
    let shore = b.ports[0].body.pos;
    b.raiders.clear();
    b.spawn(EnemyClass::Patrol, shore + Vec2::X * 60., shore);
    b.launch_spread(shore - Vec2::X * 90., 0., bearing);
    let mut ev = Events::default();
    b.advance_torpedoes(8., p, &mut ev);
    assert!(b.torpedoes.is_empty());
    assert_eq!(ev.hits, 0);
    assert!(b.raiders[0].hull() > 0.);
}

#[test]
fn fitted_secondaries_add_guns_fire_faster_and_hit_harder() {
    // She is built with a pair amidships and a pair on the bow, and her stock hull has
    // room for six a side. More than that wait for a longer hull.
    let stock = battle();
    assert_eq!(stock.wing_guns().len(), 4);
    let mut b = battle();
    let full = Secondaries {
        per_side: Secondaries::MOST_PER_SIDE,
        ..Secondaries::STOCK
    };
    b.fit_secondaries(full);
    assert_eq!(shipvivor_shipyard::Design::breakwater().wing_room(), 6);
    assert_eq!(b.wing_guns().len(), 12);
    let at = |i: usize| (b.wing_guns()[i].deck_x(), b.wing_guns()[i].deck_fore());
    assert_eq!(at(0), (-2.18, 0.));
    assert_eq!(at(3), (1.35, 10.6));
    assert_eq!(at(5), (2.18, 3.8));
    // Then beside the forward turret, and abaft the machinery.
    assert!(at(6).1 > 5. && at(7).0 > 0. && at(9).1 < 0.);
    b.fit_secondaries(Secondaries::STOCK);
    assert_eq!(b.wing_guns().len(), 4);

    /// Secondary rounds fired, and hull taken off a cruiser lying off the starboard beam.
    fn engage(fit: Secondaries) -> (usize, f32) {
        let mut b = battle();
        b.raiders.clear();
        b.next_wave = 1000.;
        b.spawn(
            EnemyClass::Cruiser,
            Vec2::new(120., 0.),
            Vec2::new(120., -10.),
        );
        toughen(&mut b.raiders[0], 100.);
        let intact = b.raiders[0].hull();
        b.fit_secondaries(fit);
        let orders = Orders {
            player: Vec2::ZERO,
            heading: 0.,
            speed: 0.,
            hull: Hull::CRUISER,
            aim: Aim::Hold,
            reload: 6.4,
            shell: MainShell::STOCK,
        };
        let mut rounds = 0;
        // The bow pair stands by. As she gets under way she draws forward into its arc.
        for gun in b.wing_guns.iter_mut().filter(|gun| gun.deck_fore() > 9.) {
            gun.reload = 100.;
        }
        for _ in 0..180 {
            b.step(1. / 60., &orders);
            rounds += b
                .effects
                .iter()
                .filter(|fx| fx.kind == EffectKind::Muzzle(Battery::Secondary) && fx.age == 0.)
                .count();
        }
        (rounds, intact - b.raiders[0].hull())
    }
    let (one, hurt) = engage(Secondaries::STOCK);
    assert!(one >= 3 && hurt > 0., "{one} rounds, {hurt} hull");
    // Five of her six a side stand on the beam, all but the bow gun.
    let (five, _) = engage(full);
    assert_eq!(five, 5 * one, "every station on the beam joins in");
    let (quick, _) = engage(Secondaries {
        reload: Secondaries::STOCK.reload / 2.,
        ..Secondaries::STOCK
    });
    assert!(quick >= 2 * one - 1, "{quick} vs {one}");
    let (_, heavy) = engage(Secondaries {
        damage: 3,
        ..Secondaries::STOCK
    });
    // Within a few percent: a plate worn by the first rounds lets a little more of the
    // heavier ones through when they land close together.
    assert!((heavy / hurt - 3.).abs() < 0.03, "{heavy} against {hurt}");
    let (twin, _) = engage(Secondaries {
        barrels: 2,
        ..Secondaries::STOCK
    });
    assert_eq!(twin, 2 * one, "both barrels of a twin mount fire together");
}

#[test]
fn a_secondary_gunhouse_fires_its_own_bore_and_reaches_as_far_as_it_engages() {
    /// The widest secondary round in flight, in inches, and how far out the farthest
    /// flies, in five seconds' fire at a cruiser lying `range` metres off her beam.
    fn engage(fit: Secondaries, range: f32) -> (f32, f32) {
        let mut b = battle();
        b.raiders.clear();
        b.landmarks.clear();
        b.next_wave = 1000.;
        let at = Vec2::new(range, 0.);
        b.spawn(EnemyClass::Cruiser, at, at + Vec2::NEG_Y);
        toughen(&mut b.raiders[0], 100.);
        b.fit_secondaries(fit);
        let orders = Orders {
            player: Vec2::ZERO,
            ..distant_observer()
        };
        let (mut widest, mut farthest) = (0_f32, 0_f32);
        for _ in 0..300 {
            // She lies where she is put, so the range holds.
            b.raiders[0].body.pos = at;
            b.step(1. / 60., &orders);
            for shot in b.shots().iter().filter(|s| s.battery == Battery::Secondary) {
                widest = widest.max(shot.calibre());
                farthest = farthest.max(shot.position().xz().length());
            }
        }
        (widest, farthest)
    }
    let house = |bore, range| Secondaries {
        gunhouse: Some(bore),
        range,
        ..Secondaries::STOCK
    };
    // A shielded mount fires its light round, and a gunhouse the round of its bore.
    let (light, _) = engage(Secondaries::STOCK, 120.);
    assert!((light - Secondaries::MOUNT_BORE).abs() < 0.01, "{light}");
    let (four, _) = engage(house(4, 260.), 120.);
    let (six, _) = engage(house(6, 340.), 120.);
    assert!(
        (four - 4.).abs() < 0.01 && (six - 6.).abs() < 0.01,
        "{four}, {six}"
    );
    // With every rangefinder a six-inch gunhouse engages out past 500 metres, and its
    // rounds carry that far rather than falling short.
    let (_, far) = engage(house(6, 540.), 500.);
    assert!(far > 480., "rounds fly {far} m at a cruiser 500 m off");
}

#[test]
fn the_bow_pair_covers_dead_ahead_together_and_each_its_own_bow_to_the_beam() {
    /// Secondary rounds from each side's bow gun and beam guns at a cruiser lying
    /// `bearing` degrees from the bow, to starboard, 130 metres off.
    fn rounds(bearing: f32) -> [[usize; 2]; 2] {
        let mut b = battle();
        b.raiders.clear();
        b.landmarks.clear();
        b.next_wave = 1000.;
        let at = shipvivor_compass::direction(bearing.to_radians()) * 130.;
        b.spawn(EnemyClass::Cruiser, at, at + Vec2::X);
        toughen(&mut b.raiders[0], 100.);
        let orders = Orders {
            player: Vec2::ZERO,
            ..distant_observer()
        };
        let mut fired = [[0; 2]; 2];
        for _ in 0..180 {
            // She lies where she is put, so the bearing holds.
            b.raiders[0].body.pos = at;
            b.raiders[0].way = Vec2::ZERO;
            b.step(1. / 60., &orders);
            for fx in b.effects.iter().filter(|fx| fx.age == 0.) {
                if fx.kind == EffectKind::Muzzle(Battery::Secondary) {
                    // Forward of the forward turret is the bow pair.
                    let bow = -fx.body.pos.y > 9.;
                    fired[usize::from(bow)][usize::from(fx.body.pos.x > 0.)] += 1;
                }
            }
        }
        fired
    }
    let [beam, bow] = rounds(0.);
    assert_eq!(beam, [0, 0], "dead ahead is masked from the beam guns");
    assert!(
        bow.iter().all(|&n| n >= 3),
        "both bow guns bear dead ahead: {bow:?}"
    );
    let [beam, bow] = rounds(-60.);
    assert!(bow[0] >= 3 && beam[0] >= 3 && bow[1] == 0 && beam[1] == 0);
    // Abaft the beam the bow gun cannot follow, and the beam gun has it alone.
    let [beam, bow] = rounds(110.);
    assert!(
        bow == [0, 0] && beam[1] >= 3 && beam[0] == 0,
        "{bow:?} {beam:?}"
    );
}

// One working cannery, with the ship lying broadside off its fairway.
fn coastal_battery_drill() -> (Battle, Orders) {
    let mut b = battle();
    b.raiders.clear();
    b.landmarks.clear();
    b.next_wave = 1000.;
    for port in &mut b.ports[1..] {
        port.wreck();
        port.silence();
    }
    b.ports[0].body.pos = Vec2::ZERO;
    b.ports[0].body.heading = 0.;
    b.ports[0].convoy_clock = 1000.;
    let orders = Orders {
        player: Vec2::new(-90., -54.),
        heading: 0.,
        ..distant_observer()
    };
    (b, orders)
}

#[test]
fn secondaries_engage_each_live_structure_and_fall_silent_at_a_closed_slip() {
    let (mut b, orders) = coastal_battery_drill();
    let mut gun_out = false;
    let mut mill_out = false;
    let mut dock_out = false;
    for _ in 0..60 * 45 {
        b.step(1. / 60., &orders);
        let port = &b.ports[0];
        gun_out |= !port.turrets()[0].live();
        mill_out |= !port.factories()[0].live();
        dock_out |= !port.docks()[0].live();
        // Nothing left standing: the base is wrecked and its gun silenced.
        if port.wrecked() && port.turrets().iter().all(|g| !g.live()) {
            break;
        }
    }
    assert!(
        gun_out && mill_out && dock_out,
        "the crew must hit all three with the main battery held"
    );
    for _ in 0..180 {
        b.step(1. / 60., &orders);
        assert!(
            !b.effects
                .iter()
                .any(|e| e.kind == EffectKind::Muzzle(Battery::Secondary) && e.age == 0.)
        );
    }
}

#[test]
fn a_wing_gun_requires_a_visible_structure_in_its_arc_and_fitted_range() {
    let (mut b, mut orders) = coastal_battery_drill();
    b.ports[0].damage_factory(0, 8);
    b.ports[0].turrets[0].strength = 0;
    let yard = b.ports[0].dock_position(0);
    orders.player = yard - Vec2::Y * 206.;
    orders.heading = std::f32::consts::FRAC_PI_2;
    let fires = |b: &mut Battle, orders: &Orders| {
        (0..120).any(|_| {
            b.step(1. / 60., orders);
            b.effects
                .iter()
                .any(|e| e.kind == EffectKind::Muzzle(Battery::Secondary) && e.age == 0.)
        })
    };
    assert!(!fires(&mut b, &orders));
    b.fit_secondaries(Secondaries {
        range: 240.,
        ..Secondaries::STOCK
    });
    b.landmarks.push(Landmark::new(
        body(&mut b.serial, orders.player.midpoint(yard), 0.),
        25.,
        LandmarkKind::Stacks,
    ));
    assert!(!fires(&mut b, &orders), "rock masks the dock");
    b.landmarks.clear();
    orders.heading = 0.;
    assert!(!fires(&mut b, &orders), "the dock is dead astern");
    orders.heading = std::f32::consts::FRAC_PI_2;
    assert!(fires(&mut b, &orders));
}

#[test]
fn secondaries_defend_against_a_ship_before_shelling_a_nearer_harbour() {
    let (mut b, orders) = coastal_battery_drill();
    let raider = orders.player + Vec2::new(118., -40.);
    b.spawn(EnemyClass::Cruiser, raider, raider - Vec2::Y);
    let mut hits = 0;
    for _ in 0..180 {
        hits += b.step(1. / 60., &orders).hits;
    }
    assert!(
        hits > 0,
        "the ship on the same beam draws the secondary fire"
    );
    assert_eq!(
        b.ports[0].turrets[0].strength, 6,
        "the nearer shore gun is still intact"
    );
}

#[test]
fn rangefinders_let_the_secondaries_reach_a_farther_boat() {
    fn rounds(range: f32) -> usize {
        let mut b = battle();
        b.raiders.clear();
        b.next_wave = 1000.;
        b.spawn(
            EnemyClass::Cruiser,
            Vec2::new(195., 0.),
            Vec2::new(195., -10.),
        );
        b.fit_secondaries(Secondaries {
            range,
            ..Secondaries::STOCK
        });
        let orders = Orders {
            player: Vec2::ZERO,
            heading: 0.,
            speed: 0.,
            hull: Hull::CRUISER,
            aim: Aim::Hold,
            reload: 6.4,
            shell: MainShell::STOCK,
        };
        (0..120)
            .map(|_| {
                b.step(1. / 60., &orders);
                b.effects
                    .iter()
                    .filter(|fx| fx.kind == EffectKind::Muzzle(Battery::Secondary) && fx.age == 0.)
                    .count()
            })
            .sum()
    }
    assert_eq!(rounds(Secondaries::STOCK.range), 0);
    assert!(rounds(240.) > 0);
}

#[test]
fn every_round_opens_a_cruisers_light_plate_and_does_its_full_damage() {
    // Not so heavy that it might go clean through her.
    for bore in [0, 2] {
        let mut b = battle();
        b.raiders.clear();
        let center = Vec2::new(-75., -125.);
        b.spawn(EnemyClass::Cruiser, center, center + Vec2::NEG_Y);
        let intact = b.raiders[0].hull();
        let shell = MainShell {
            bore,
            ..MainShell::STOCK
        };
        let side = center.extend(1.2).xzy();
        projectile::FlightPath::main(side + Vec3::X * 75., side, shell)
            .unwrap()
            .launch(
                &mut b.shots,
                &mut b.effects,
                &mut b.serial,
                Battery::Main,
                shell.breach_force(),
            );
        b.advance_shots(0.5, Vec2::ZERO, &mut Events::default());
        assert!(
            b.effects.iter().any(|e| matches!(
                e.kind,
                EffectKind::Armor {
                    outcome: Impact::Penetrated,
                    ..
                }
            )),
            "{bore}"
        );
        let lost = intact - b.raiders[0].hull();
        assert_eq!(lost, f32::from(shell.breach_force()), "{bore}");
    }
}

/// One hit amidships that stops a patrol boat's engine and leaves it afloat.
fn cripple(raider: &mut Raider) {
    let at = raider.body.pos;
    raider.hit_hull(at, 2.);
    assert!(raider.is_disabled() && raider.afloat());
}
/// The same ship built `factor` times as strong, for counting hits without sinking her.
fn toughen(raider: &mut Raider, factor: f32) {
    let mut design = (*raider.class.design()).clone();
    design.toughness *= factor;
    raider.ship = shipvivor_shipyard::Ship::new(design);
}
/// Wreck every module.
fn scuttle(raider: &mut Raider) {
    let design = raider.ship.design().clone();
    for module in 0..design.modules().len() {
        let (fore, aft) = design.span(module);
        raider.ship.strike_hull(0.5 * (fore + aft), 1e6);
    }
    assert!(!raider.afloat());
}

#[test]
fn hammering_a_cruisers_stern_stops_her_until_her_crew_restarts_the_engine() {
    let mut b = battle();
    b.raiders.clear();
    b.landmarks.clear();
    b.next_wave = 1000.;
    let orders = distant_observer();
    let start = Vec2::new(-75., -125.);
    b.spawn(EnemyClass::Cruiser, start, start + Vec2::NEG_Y);
    // A point on her bottom, in metres ahead of her centre and just to `side` of the
    // keel, so each blow lands on the plate meant for it. Her engine is under the
    // after gun, 7 metres back.
    let keel = |b: &Battle, ahead: f32, side: f32| {
        let forward = b.raiders[0].body.forward();
        b.raiders[0].body.pos + forward * ahead + forward.perp() * side * 0.5
    };
    let run = |b: &mut Battle, seconds: usize| {
        let from = b.raiders[0].body.pos;
        for _ in 0..seconds * 60 {
            b.step(1. / 60., &orders);
        }
        b.raiders[0].body.pos.distance(from) / seconds as f32
    };
    let cruising = run(&mut b, 2);
    assert!((cruising - 6.6).abs() < 0.2, "{cruising} m/s");

    let stern = keel(&b, -7., -1.);
    b.raiders[0].hit_hull(stern, 2.);
    assert!(!b.raiders[0].is_disabled());
    // She carries her way for a few seconds before the damage tells.
    run(&mut b, 4);
    let limping = run(&mut b, 2);
    assert!(
        (limping - 3.3).abs() < 0.2,
        "a damaged engine gives half: {limping} m/s"
    );

    let stern = keel(&b, -7., 1.);
    b.raiders[0].hit_hull(stern, 2.);
    let r = &b.raiders[0];
    assert!(r.is_disabled() && r.afloat());
    assert_eq!(r.live_guns(), 2, "the guns were not touched");
    run(&mut b, 4);
    let adrift = run(&mut b, 5);
    assert!(adrift < 1., "{adrift} m/s");

    // Left alone, her repair party has the engine turning again.
    run(&mut b, 20);
    assert!(!b.raiders[0].is_disabled());
    assert!(run(&mut b, 2) > 3.);

    // A third hit wrecks the module, and the gun standing on it goes with it.
    let stern = keel(&b, -7., -1.);
    b.raiders[0].hit_hull(stern, 2.);
    let r = &b.raiders[0];
    assert!(r.vessel().compartments()[4].wrecked());
    assert!(r.is_disabled() && r.afloat());
    assert_eq!(r.live_guns(), 1);
    assert!(r.guns()[0].live() && !r.guns()[1].live());
}

#[test]
fn locked_fire_walks_along_the_hull_as_modules_are_wrecked() {
    let mut b = battle();
    b.raiders.clear();
    let pos = Vec2::new(-75., -125.);
    b.spawn(EnemyClass::Cruiser, pos, pos + Vec2::NEG_Y);
    let r = &mut b.raiders[0];
    let forward = r.body.forward();
    assert_eq!(r.aim_point(Vec2::ZERO).xz(), pos);
    // Hits on a wrecked module do little, so the mark moves to the nearest sound one.
    r.hit_hull(pos, 5.);
    let mark = r.aim_point(Vec2::ZERO).xz();
    let ahead = (mark - pos).dot(forward);
    assert!((ahead - 3.46).abs() < 0.05, "{ahead} m ahead");
    assert_eq!(r.vessel().design().module_at(16. - ahead), 2);
    r.hit_hull(mark, 5.);
    assert!(r.afloat());
    let mark = r.aim_point(Vec2::ZERO).xz();
    assert_eq!(
        r.vessel()
            .design()
            .module_at(16. - (mark - pos).dot(forward)),
        4
    );
    r.hit_hull(mark, 5.);
    assert!(!r.afloat(), "three of her six modules are gone");
}

#[test]
fn hits_on_the_player_report_where_along_her_hull_they_landed() {
    // A shell from abeam, landing ahead of or behind her centre. She heads north.
    let shell = |north: f32, hull: Hull| {
        let mut b = battle();
        b.keel = (Vec2::NEG_Y, hull);
        b.raiders.clear();
        fire(
            &mut b.shots,
            &mut b.effects,
            &mut b.serial,
            Vec3::new(60., 2., -north),
            Vec3::new(0., 1., -north),
            Battery::Raider,
            7,
        );
        let mut events = Events::default();
        b.advance_shots(2., Vec2::ZERO, &mut events);
        events.blows
    };
    let hull = Hull::CRUISER;
    for north in [12., 3., -3., -10.] {
        let blows = shell(north, hull);
        assert_eq!(blows.len(), 1);
        assert_eq!(blows[0].damage, 7);
        assert!((blows[0].aft - (hull.bow - north)).abs() < 0.01);
        // It came in from the east, on her starboard beam, five inches across.
        assert!(blows[0].abeam > 0., "{:?}", blows[0]);
        assert_eq!(blows[0].calibre.map(f32::round), Some(5.));
    }
    assert!(shell(-21., hull).is_empty());
    let long = Hull {
        stern: hull.stern + 10.,
        ..hull
    };
    let blows = shell(-21., long);
    assert_eq!(blows.len(), 1);
    assert!((blows[0].aft - 37.).abs() < 0.01);
    assert!(shell(-26., long).is_empty());

    // A ram lands on her stem.
    let mut b = raider_ahead();
    let (mut pos, speed) = (Vec2::ZERO, 13.6);
    let mut blows = Vec::new();
    for _ in 0..40 {
        pos += Vec2::NEG_Y * speed / 60.;
        let orders = Orders {
            player: pos,
            speed,
            ..distant_observer()
        };
        blows.extend(b.step(1. / 60., &orders).blows);
    }
    assert_eq!(blows.len(), 1);
    assert!(blows[0].aft < 3. && blows[0].damage > 0, "{:?}", blows[0]);
    assert_eq!(blows[0].calibre, None, "no plate turns a ram");
    assert_eq!(blows[0].reach, shipvivor_shipyard::Reach::Bays);
}

#[test]
fn only_a_round_that_gets_below_her_deck_amidships_reaches_her_engine() {
    use shipvivor_shipyard::{Design, Reach, Ship, State};
    // A round from her starboard beam at the given height, half a metre abaft her
    // centre, where the forward funnel stands over the machinery space.
    let blow = |height: f32| {
        let mut b = battle();
        b.keel = (Vec2::NEG_Y, Hull::CRUISER);
        b.raiders.clear();
        fire(
            &mut b.shots,
            &mut b.effects,
            &mut b.serial,
            Vec3::new(60., height + 0.5, 0.5),
            Vec3::new(0., height, 0.5),
            Battery::Raider,
            7,
        );
        let mut events = Events::default();
        b.advance_shots(2., Vec2::ZERO, &mut events);
        assert_eq!(events.blows.len(), 1, "{height} m up");
        events.blows[0]
    };
    let engine = |blow: Blow| {
        let mut ship = Ship::new(Design::breakwater());
        ship.strike_into(blow.aft, blow.abeam, 7., None, blow.reach);
        let berth = shipvivor_shipyard::Berth {
            module: 3,
            index: 2,
        };
        ship.state(berth)
    };
    let side = blow(0.9);
    assert!((side.aft - 16.5).abs() < 0.05, "{side:?}");
    let Reach::Track(Some([(_, from), (_, to)])) = side.reach else {
        panic!("a round holing her side runs on inside: {side:?}");
    };
    assert!(from > 2.5 && to < 0.5, "{side:?}");
    assert_eq!(engine(side), State::Damaged);
    let funnel = blow(3.);
    assert!((funnel.aft - 16.5).abs() < 0.05, "{funnel:?}");
    assert_eq!(funnel.reach, Reach::Track(None));
    assert_eq!(engine(funnel), State::Working);
}

#[test]
fn each_enemy_gun_fires_rounds_as_wide_as_its_calibre() {
    let calibre = |battery: Battery, barrel: f32| {
        let mut b = battle();
        b.shots.clear();
        let path = projectile::FlightPath::from_barrel(
            Vec3::Y * 2.,
            Vec3::new(100., 1., 0.),
            battery,
            barrel,
        );
        path.unwrap()
            .launch(&mut b.shots, &mut b.effects, &mut b.serial, battery, 1);
        b.shots[0].calibre().round()
    };
    // The armor plate cards are pitched against these: 4, 7 and 10 inches.
    let main = |class: EnemyClass| calibre(Battery::Raider, 3.2 * class.profile().gun_scale);
    assert_eq!(
        [
            EnemyClass::Patrol,
            EnemyClass::Destroyer,
            EnemyClass::Cruiser,
            EnemyClass::Battleship
        ]
        .map(main),
        [5., 6., 8., 12.]
    );
    assert_eq!(calibre(Battery::EnemySecondary, 1.8), 3.);
    assert_eq!(calibre(Battery::Aircraft, 1.8), 3.);
    assert_eq!(calibre(Battery::Shore, 0.), 13.);
}

/// Rounds fired and shell hits taken in a minute steaming past one enemy lying abeam.
fn gunnery_against(class: EnemyClass, speed: f32, range: f32) -> (usize, usize) {
    let mut b = battle();
    b.raiders.clear();
    b.landmarks.clear();
    b.next_wave = 1000.;
    b.spawn(class, Vec2::new(range, 0.), Vec2::new(range, -10.));
    toughen(&mut b.raiders[0], 1000.);
    for gun in &mut b.wing_guns {
        gun.reload = 1e6;
    }
    let mut orders = Orders {
        player: Vec2::ZERO,
        heading: 0.,
        speed,
        hull: Hull::CRUISER,
        aim: Aim::Hold,
        reload: 6.4,
        shell: MainShell::STOCK,
    };
    let (mut rounds, mut hits) = (0, 0);
    for _ in 0..60 * 60 {
        orders.player += shipvivor_compass::direction(orders.heading) * speed / 60.;
        // Turn back to keep the enemy abeam at about the same range.
        if orders.player.distance(b.raiders[0].body.pos) > range * 1.6 {
            orders.heading += std::f32::consts::PI;
        }
        hits += b.step(1. / 60., &orders).blows.len();
        rounds += b
            .effects
            .iter()
            .filter(|fx| {
                matches!(
                    fx.kind,
                    EffectKind::Muzzle(Battery::Raider | Battery::EnemySecondary)
                ) && fx.age == 0.
            })
            .count();
    }
    (rounds, hits)
}

#[test]
fn enemy_gunners_lead_a_ship_holding_her_course() {
    let (rounds, hits) = gunnery_against(EnemyClass::Cruiser, 11., 170.);
    assert!(
        rounds > 20 && hits * 10 >= rounds * 6,
        "a cruiser hit {hits} of {rounds} at full ahead"
    );
    let (rounds, hits) = gunnery_against(EnemyClass::Battleship, 7., 240.);
    assert!(
        rounds > 40 && hits * 100 >= rounds * 45,
        "a battleship hit {hits} of {rounds} at half ahead"
    );
}

/// Steel at a base: in its storehouse and laid into the hulls on its slips.
fn steel(port: &Port) -> u8 {
    port.stock + port.docks().iter().map(Dock::laid).sum::<u8>()
}

#[test]
fn every_live_factory_adds_output_and_closing_them_preserves_residual_production() {
    let mut battle = battle();
    for (i, port) in battle.ports.iter_mut().enumerate() {
        let works = i + 1;
        let docks = dock_count(works);
        assert_eq!((port.factories().len(), port.docks().len()), (works, docks));
        port.stock = 0;
        port.work(60., 0);
        assert_eq!(steel(port), (docks + 3 * works) as u8);
        for j in 0..works {
            assert!(port.damage_factory(j, 8));
            assert!(!port.damage_factory(j, 8));
            assert_eq!(port.live_factories(), works - 1 - j);
            assert_eq!(
                port.production_per_minute(),
                (docks + 3 * (works - 1 - j)) as f32
            );
        }
        // Each dock still makes a little steel of its own.
        let before = steel(port);
        for _ in 0..3600 {
            port.work(1. / 60., 0);
        }
        assert_eq!(steel(port) - before, docks as u8);
        for k in 0..docks {
            assert_eq!(port.docks()[k].state(), YardState::Working);
            assert!(port.damage_dock(k, 24));
            assert!(!port.damage_dock(k, 24));
        }
        // With no factory to raise them, the docks stay down and the base is wrecked.
        assert!(port.wrecked());
        port.work(600., 3);
        port.unload();
        assert_eq!((port.stock, port.production_per_minute()), (0, 0.));
        assert!(port.docks().iter().all(|d| !d.live()));
    }
    assert_eq!(battle.ruined_yards(), 3);
}

#[test]
fn standing_factories_raise_a_fallen_dock_and_spend_their_steel_on_it() {
    let mut b = battle();
    let mills = &mut b.ports[1];
    assert_eq!(mills.docks().len(), 2);
    assert!(mills.damage_dock(0, u16::MAX));
    assert!(!mills.wrecked() && mills.raising());
    // Only the standing dock makes steel while the factories rebuild.
    assert_eq!(mills.production_per_minute(), 1.);
    let eta = mills.rebuild_eta().unwrap();
    assert!((eta - REBUILD_SECONDS / 2.).abs() < 1e-3, "{eta}");
    mills.work(eta * 0.5, 0);
    assert!((mills.docks()[0].rebuilding().unwrap() - 0.5).abs() < 1e-3);
    // Losing a factory halves the work; the other finishes the job.
    assert!(mills.damage_factory(0, u16::MAX));
    let eta = mills.rebuild_eta().unwrap();
    assert!((eta - REBUILD_SECONDS * 0.5).abs() < 1e-2, "{eta}");
    mills.work(eta + 0.1, 0);
    assert_eq!(mills.docks()[0].state(), YardState::Working);
    assert_eq!(mills.rebuild_eta(), None);
    assert_eq!(mills.production_per_minute(), 2. + 3.);
    // Both docks down and the factories gone: nothing is left to raise them.
    assert!(mills.damage_factory(1, u16::MAX));
    assert!(mills.damage_dock(0, u16::MAX));
    assert!(!mills.wrecked());
    assert!(mills.damage_dock(1, u16::MAX));
    assert!(mills.wrecked());
    mills.work(600., 0);
    assert!(mills.docks().iter().all(|d| !d.live()));
    assert_eq!(b.ruined_yards(), 1);
}

#[test]
fn only_the_heavy_slip_reserves_material_for_a_battleship_and_waits_for_an_open_berth() {
    let mut b = battle();
    let observer = distant_observer().player;
    b.raiders.clear();
    for p in &mut b.ports {
        p.work(90., 3);
    }
    // Every slip had a funded patrol hull before the late-tier orders opened. The bases
    // take turns, so the second slips go once each base's first is fitting out.
    let slips: usize = b.ports.iter().map(|p| p.docks().len()).sum();
    assert_eq!(slips, 5);
    for _ in 0..slips {
        b.reinforce(observer);
        assert_eq!(b.raiders.pop().unwrap().class, EnemyClass::Patrol);
    }
    for p in &mut b.ports {
        p.clear_slips();
    }
    let orders = |b: &Battle| -> Vec<Vec<(EnemyClass, u8)>> {
        (b.ports.iter())
            .map(|p| p.docks().iter().map(Dock::order).collect())
            .collect()
    };
    let (destroyer, cruiser, battleship) = (
        (EnemyClass::Destroyer, 4),
        (EnemyClass::Cruiser, 7),
        (EnemyClass::Battleship, 12),
    );
    assert_eq!(
        orders(&b),
        [
            vec![destroyer],
            vec![cruiser, cruiser],
            vec![battleship, battleship]
        ]
    );
    for p in &mut b.ports[..2] {
        p.wreck();
    }
    let heavy = &mut b.ports[2];
    heavy.empty_hulls();
    heavy.stock = 11;
    b.reinforce(observer);
    assert!(
        b.raiders.is_empty(),
        "the drydock keeps material for its scheduled hull"
    );
    // The plates went to one hull, so it is nearly done rather than both half done.
    assert_eq!(b.ports[2].docks()[0].laid(), 11);
    let heavy = &mut b.ports[2];
    heavy.work(6., 3);
    assert!(heavy.docks()[0].ready() && !heavy.docks()[1].ready());
    let berth = heavy.dock_berth(0);
    b.reinforce(berth);
    assert!(b.raiders.is_empty());
    assert!(b.ports[2].docks()[0].ready());
    b.reinforce(observer);
    assert_eq!(b.raiders.len(), 1);
    assert_eq!(b.raiders[0].class, EnemyClass::Battleship);
    assert!(b.raiders[0].body.pos.distance(berth) < 18.);
    // The first slip fits out; the second waits for a full bill of its own.
    b.ports[2].stock = YARD_CAPACITY;
    b.reinforce(observer);
    assert_eq!(b.raiders.len(), 1);
    assert!(!b.ports[2].docks()[1].ready());
}

#[test]
fn each_slip_launches_from_its_own_berth() {
    let mut b = battle();
    let observer = distant_observer().player;
    b.raiders.clear();
    for p in &mut b.ports {
        p.work(90., 0);
        p.clear_slips();
    }
    let mills = &b.ports[1];
    let berths = [mills.dock_berth(0), mills.dock_berth(1)];
    assert!(berths[0].distance(berths[1]) > 25.);
    for p in [0, 2] {
        b.ports[p].wreck();
    }
    b.reinforce(observer);
    b.reinforce(observer);
    let launched: Vec<Vec2> = b.raiders.iter().map(|r| r.body.pos).collect();
    assert_eq!(launched.len(), 2);
    for (at, berth) in launched.iter().zip(berths) {
        assert!(at.distance(berth) < 18., "{at:?} {berth:?}");
    }
}

#[test]
fn aiming_at_each_factory_hits_only_that_building_and_its_ruins_stay_visible() {
    let mut b = battle();
    b.raiders.clear();
    let i = 2;
    for j in 0..b.ports[i].factories().len() {
        let pos = b.ports[i].factory_position(j);
        let approach = (pos - b.ports[i].body.pos).normalize();
        for salvo in 0..4 {
            let target = pos.extend(8.5).xzy();
            let eye = target + (60. * approach).extend(0.).xzy();
            let sight = b.sight(eye, (target - eye).normalize(), pos - 20. * approach);
            assert_eq!(sight.factory, Some((i, j)));
            assert_eq!(sight.yard, None);
            fire(
                &mut b.shots,
                &mut b.effects,
                &mut b.serial,
                eye,
                target,
                Battery::Main,
                2,
            );
            b.advance_shots(0.4, Vec2::ZERO, &mut Events::default());
            assert_eq!(b.ports[i].factories()[j].live(), salvo < 3);
        }
        let wreck = pos.extend(4.1).xzy();
        let eye = wreck + (60. * approach).extend(0.).xzy();
        let sight = b.sight(eye, (wreck - eye).normalize(), pos - 20. * approach);
        assert_eq!(sight.factory, Some((i, j)));
        assert_eq!(b.ports[i].live_factories(), 2 - j);
        assert_eq!(b.ports[i].docks()[0].state(), YardState::Working);
        let collapses = b
            .effects
            .iter()
            .filter(|e| e.kind == EffectKind::Demolition { dock: false } && e.body.pos == pos)
            .count();
        assert_eq!(collapses, 1);
    }
    assert_eq!(b.ruined_yards(), 0);
}

#[test]
fn with_its_docks_down_the_heavy_yards_back_hall_falls_to_fire_from_the_sea() {
    let mut b = battle();
    b.raiders.clear();
    b.floe.icebergs.clear();
    let i = 2;
    for j in 0..b.ports[i].docks().len() {
        assert!(b.ports[i].damage_dock(j, u16::MAX));
    }
    // The hall stands behind the first dock's ruins and beside the bluff. Over both the
    // eye sees it and the guns reach it.
    let port = &b.ports[i];
    let (ahead, side) = (port.body.forward(), port.body.forward().perp());
    let hall = port.factory_position(2);
    let target = hall.extend(7.7).xzy();
    for abeam in [-20., 0., 20.] {
        let from = port.body.pos + ahead * 80. + side * abeam;
        let eye = from.extend(9.).xzy();
        let sight = b.sight(eye, (target - eye).normalize(), hall);
        assert_eq!(sight.factory, Some((i, 2)), "{abeam}");
    }
    let mount = (port.body.pos + ahead * 80.).extend(2.6).xzy();
    for salvo in 0..4 {
        fire(
            &mut b.shots,
            &mut b.effects,
            &mut b.serial,
            mount,
            target,
            Battery::Main,
            2,
        );
        b.advance_shots(0.6, Vec2::ZERO, &mut Events::default());
        assert_eq!(b.ports[i].factories()[2].live(), salvo < 3);
    }
}

#[test]
fn gallery_guns_share_a_compartment_but_train_and_load_independently() {
    use shipvivor_shipyard::{Component, Design, Frame, Module};
    let mut design = Design::breakwater();
    design.widen(14.);
    design
        .replace_module(
            1,
            Module::new(Frame::BatteryGallery, [Component::MainGun; 2]),
        )
        .unwrap();
    let mut b = battle();
    b.raiders.clear();
    let main = MainShell {
        bore: 14,
        ..MainShell::STOCK
    };
    let wing = MainShell {
        bore: 4,
        ..MainShell::STOCK
    };
    b.fit_design(&design, |station| {
        (
            if station.paired() { wing } else { main },
            TurretLayout::Twin,
        )
    });
    let mut orders = Orders {
        player: Vec2::ZERO,
        heading: 0.,
        speed: 0.,
        hull: Hull::breakwater(&design),
        aim: Aim::Track(Vec3::X * 190.),
        shell: main,
        reload: 18.,
    };
    // The eighteen-inch gunhouse trains at under half the pace of the wing guns.
    for _ in 0..800 {
        b.step(1. / 60., &orders);
    }
    assert_eq!(b.main_mounts()[0].status(), MountStatus::Masked);
    assert_eq!(b.main_mounts()[1].status(), MountStatus::Ready);
    assert_eq!(b.main_mounts()[2].status(), MountStatus::Ready);
    b.order_volley(Volley::Salvo);
    b.step(1. / 60., &orders);
    let shells: Vec<_> = b
        .shots()
        .iter()
        .filter(|s| s.battery == Battery::Main)
        .collect();
    assert_eq!(shells.len(), 4);
    for (shell, fore) in [(wing, true), (main, false)] {
        let fired: Vec<_> = shells.iter().filter(|s| s.shell == shell).collect();
        assert_eq!(fired.len(), 2);
        assert!(
            fired
                .iter()
                .all(|s| (s.diameter - shell.diameter_metres()).abs() < 1e-6
                    && (s.body.pos.y < 0.) == fore)
        );
    }
    // The wing guns load on the same curve as the main battery, with the same drill.
    let wing_reload = 18. * wing.load_seconds() / main.load_seconds();
    assert!((b.main_mounts()[1].reload_left(0) - wing_reload).abs() < 0.01);
    assert!((b.main_mounts()[2].reload_left(0) - 18.).abs() < 0.01);
    orders.aim = Aim::Track(Vec3::NEG_X * 190.);
    for _ in 0..660 {
        b.step(1. / 60., &orders);
    }
    assert_eq!(b.main_mounts()[0].status(), MountStatus::Ready);
    assert_eq!(b.main_mounts()[1].status(), MountStatus::Masked);
}

#[test]
fn shells_fit_their_firing_mount_and_scale_with_its_size() {
    use super::projectile::FlightPath;
    let mut b = battle();
    let muzzle = Vec3::Y * 2.;
    let target = Vec3::NEG_Z * 100.;
    for (battery, barrel) in [
        (Battery::Secondary, 1.8),
        (Battery::Raider, 2.56),
        (Battery::Raider, 6.56),
        (Battery::Shore, 8.),
    ] {
        FlightPath::from_barrel(muzzle, target, battery, barrel)
            .unwrap()
            .launch(&mut b.shots, &mut b.effects, &mut b.serial, battery, 1);
    }
    let widths: Vec<_> = b.shots.iter().map(|s| s.diameter).collect();
    assert!(widths[0] < widths[1] && widths[1] < widths[2]);
    assert!((widths[2] / widths[1] - 6.56 / 2.56).abs() < 1e-5);
    assert!(widths[0] < 0.17 && widths[3] < 0.75);
}

#[test]
fn opening_screen_fires_from_separated_bearings_in_the_first_minute() {
    let mut b = battle();
    let orders = Orders {
        player: Vec2::ZERO,
        ..distant_observer()
    };
    let mut bearings: Vec<Vec2> = vec![];
    for _ in 0..60 * 60 {
        b.step(1. / 60., &orders);
        for fx in &b.effects {
            if fx.age == 0. && fx.kind == EffectKind::Muzzle(Battery::Raider) {
                bearings.push(fx.body.pos.normalize());
            }
        }
    }
    assert!(
        bearings
            .iter()
            .any(|a| bearings.iter().any(|b| a.dot(*b) < -0.35)),
        "at least two ships should engage more than 110 degrees apart"
    );
}

#[test]
fn the_regent_requires_sustained_fire_and_can_be_finished_with_stock_guns() {
    let chart = Chart::classic([
        Vec2::new(0., -152.),
        Vec2::new(168., -424.),
        Vec2::new(496., -300.),
        Vec2::new(416., 76.),
        Vec2::new(72., 164.),
    ]);
    let mut b = Battle::regent_encounter(14., &chart);
    assert!(b.raiders.iter().any(|r| r.class == EnemyClass::Cruiser));
    assert!(b.boss_ship().is_none());
    // Her screen broken, she comes up the deep with her battle line.
    b.raiders.clear();
    b.step(1. / 30., &distant_observer());
    assert!(b.boss_ship().is_some());
    // Isolate the duel so escort kills cannot satisfy the boss objective.
    b.raiders.retain(|r| r.class == EnemyClass::Regent);
    b.landmarks.clear();
    for port in &mut b.ports {
        port.wreck();
        port.silence();
    }
    b.raiders[0].body.pos = Vec2::new(1800., 1800.);
    let mut exposed_at = None;
    let dt = 1. / 30.;
    for tick in 0..30 * 900 {
        let seconds = tick as f32 * dt;
        let ship = &b.raiders[0];
        let beam = Vec2::from_angle(seconds * 0.054);
        let orders = Orders {
            player: ship.body.pos + beam * 174.,
            heading: bearing(beam.perp()),
            speed: 9.4,
            aim: Aim::Nearest,
            reload: 6.4,
            ..distant_observer()
        };
        if !ship.protected() {
            exposed_at.get_or_insert(seconds);
        }
        let ev = b.step(dt, &orders);
        if ev.regent_sunk {
            let stripped = exposed_at.expect("all twelve turrets must be knocked out first");
            assert!(
                stripped > 60. && seconds - stripped > 10.,
                "stripped at {stripped}, sunk at {seconds}"
            );
            assert!(!b.step(dt, &orders).regent_sunk);
            eprintln!(
                "stock-gun duel: turrets cleared at {stripped:.1}s, Regent sunk at {seconds:.1}s"
            );
            return;
        }
    }
    panic!(
        "Regent survived 15 minutes: {} turrets, {} hull",
        b.raiders[0].live_guns(),
        b.raiders[0].hull()
    );
}

#[test]
fn a_regent_turret_keeps_its_warned_aim_when_the_player_changes_course() {
    let mut ship = Raider::new(
        Body {
            id: 1,
            pos: Vec2::ZERO,
            heading: 0.,
        },
        EnemyClass::Regent,
    );
    for i in 1..ship.guns().len() {
        ship.hit_gun(i, 100.);
    }
    let first = Vec2::new(270., -100.);
    let mut warned = 0;
    let mut serial = 1;
    let mut shots = Vec::new();
    let mut effects = Vec::new();
    for _ in 0..60 * 20 {
        let player = if warned > 0 {
            first + Vec2::Y * 55.
        } else {
            first
        };
        ship.fire_guns(
            1. / 60.,
            player,
            Vec2::ZERO,
            &mut shots,
            &mut effects,
            &mut serial,
        );
        if ship.guns()[0].charging() {
            warned += 1;
        }
        if shots.len() == 3 {
            let middle = &shots[1];
            assert!(warned >= 180);
            let line = middle.body.forward();
            assert!(line.dot((first - middle.body.pos).normalize()) > 0.999);
            assert!(line.dot((player - middle.body.pos).normalize()) < 0.99);
            return;
        }
    }
    panic!("the warned salvo never fired");
}

/// Seconds until the AA guns bring down the spotter over port 0, with the ship lying
/// `offset` metres from where it starts its circuit. `None` if it is still flying.
fn spotter_shot_down(fit: AntiAir, offset: Vec2, seconds: f32) -> (Option<f32>, Battle) {
    let mut b = battle();
    b.raiders.clear();
    b.fit_anti_air(fit);
    let player = b.spotters[0].position + offset;
    let orders = Orders {
        player,
        heading: 0.,
        speed: 0.,
        hull: Hull::CRUISER,
        aim: Aim::Hold,
        reload: 6.4,
        shell: MainShell::STOCK,
    };
    let dt = 1. / 60.;
    let mut t = 0.;
    while t < seconds {
        b.raiders.clear();
        b.step(dt, &orders);
        t += dt;
        if !b.spotters[0].airborne() {
            return (Some(t), b);
        }
    }
    (None, b)
}

#[test]
fn aa_guns_bring_down_a_spotter_in_reach_and_it_stops_reporting() {
    let fitted = AntiAir {
        mounts: 2,
        ..AntiAir::NONE
    };
    // Lying under the far end of its circuit, the guns have it in reach for a while.
    let under = Vec2::new(200., -180.);
    let (unarmed, _) = spotter_shot_down(AntiAir::NONE, under, 30.);
    assert_eq!(unarmed, None);
    let (downed, mut b) = spotter_shot_down(fitted, under, 30.);
    let downed = downed.expect("two AA guns bring the spotter down within half a minute");
    assert!(downed > 1., "it takes more than one burst: {downed:.1} s");
    assert!(!b.spotters[0].reporting && b.spotters[0].alert_left == 0.);
    assert!(
        b.effects()
            .iter()
            .any(|fx| fx.kind == EffectKind::Downed(Airframe::Spotter))
    );
    assert!(b.effects().iter().any(|fx| fx.kind == EffectKind::Flak));
    // It falls into the sea and stays down for the rest of the leg.
    let before = b.spotters[0].position;
    for _ in 0..60 * 5 {
        b.advance_effects(1. / 60.);
    }
    assert!(
        b.effects()
            .iter()
            .all(|fx| !matches!(fx.kind, EffectKind::Downed(_)))
    );
    b.update_lookouts(10., before);
    assert!(!b.spotters[0].airborne() && !b.spotters[0].reporting);
    // More guns, heavier bursts and quicker loaders bring it down sooner.
    let heavy = AntiAir {
        mounts: 6,
        damage: 3,
        reload: 0.6,
        ..AntiAir::NONE
    };
    let (sooner, _) = spotter_shot_down(heavy, under, 30.);
    assert!(sooner.unwrap() < downed, "{sooner:?} against {downed:.1} s");
    // Out of reach, the guns hold their fire.
    let (far, b) = spotter_shot_down(fitted, Vec2::new(900., 0.), 10.);
    assert_eq!(far, None);
    assert!(b.shots().iter().all(|s| s.battery != Battery::AntiAir));
}

#[test]
fn aa_guns_shoot_down_a_strafer_and_the_station_takes_longer_to_send_another() {
    let mut b = battle();
    b.raiders.clear();
    b.landmarks.clear();
    b.open_air_station(AirRaid::Strafe);
    b.fit_anti_air(AntiAir {
        mounts: 4,
        damage: 2,
        ..AntiAir::NONE
    });
    let port = &b.ports[0];
    let player =
        port.factory_position(0) + port.body.forward() * 185. - port.body.forward().perp() * 55.;
    let orders = Orders {
        player,
        heading: 0.,
        speed: 0.,
        hull: Hull::CRUISER,
        aim: Aim::Hold,
        reload: 6.4,
        shell: MainShell::STOCK,
    };
    let dt = 1. / 60.;
    let mut downed = false;
    for _ in 0..60 * 70 {
        b.raiders.clear();
        b.ports[0].has_contact = true;
        b.step(dt, &orders);
        if b.effects()
            .iter()
            .any(|fx| fx.kind == EffectKind::Downed(Airframe::Attack))
        {
            downed = true;
            break;
        }
    }
    assert!(downed, "four AA guns bring a strafer down on its run");
    let station = b.air_station().unwrap();
    assert!(station.aircraft.is_none());
    assert!(station.rearm > 40.);
}

#[test]
fn a_faster_round_flies_farther_and_training_gear_trains_a_heavy_mount_sooner() {
    let mut b = battle();
    b.raiders.clear();
    let far = Vec3::new(0., 0., -1300.);
    let mount = b.main_mount(Station::Bow);
    let stock = mount
        .trajectory(Vec2::ZERO, 0., far, MainShell::STOCK)
        .unwrap();
    let fast = MainShell {
        velocity: 3,
        ..MainShell::STOCK
    };
    let quick = mount.trajectory(Vec2::ZERO, 0., far, fast).unwrap();
    let carried = |t: &Trajectory| t.end().xz().distance(t.point(0.).xz());
    assert!((carried(&stock) - MainShell::STOCK.reach()).abs() < 6.);
    assert!(carried(&quick) > carried(&stock) + 150.);
    assert!(stock.end().y.abs() < 1e-3 && stock.descends_to(0.).is_some());

    // An eight-inch gunhouse swinging onto the beam, with and without training gear.
    let heavy = MainShell {
        bore: 4,
        ..MainShell::STOCK
    };
    let swing = |gear: u16| {
        let mut b = battle();
        b.raiders.clear();
        b.fit_training(gear);
        let orders = Orders {
            player: Vec2::ZERO,
            heading: 0.,
            speed: 0.,
            hull: Hull::CRUISER,
            aim: Aim::Track(Vec3::new(200., 1.7, -20.)),
            reload: 12.8,
            shell: heavy,
        };
        let dt = 1. / 60.;
        (1..=60 * 30)
            .find(|_| {
                b.step(dt, &orders);
                b.raiders.clear();
                b.main_mount(Station::Bow).status() == MountStatus::Ready
            })
            .map(|steps| steps as f32 * dt)
            .unwrap()
    };
    let (slow, geared) = (swing(0), swing(4));
    assert!(geared < 0.6 * slow, "{geared:.2} s against {slow:.2} s");
}

/// Three bases 1200 m apart in open water, at the given stages, with the coasters
/// moored off them and nothing else afloat.
fn coastwise(stages: [u8; 3]) -> Battle {
    let mut b = Battle::for_chart(1000., &coastwise_chart(stages));
    b.raiders.retain(|r| r.class == EnemyClass::Coaster);
    for port in &mut b.ports {
        port.convoy_clock = 1e4;
    }
    b
}
/// Three one-factory bases in a row along open water, roused in the given stages.
fn coastwise_chart(stages: [u8; 3]) -> Chart {
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
    chart.harbours = (0..3)
        .map(|i| HarbourSite {
            pos: Vec2::new(800. + 1200. * i as f32, 0.),
            heading: 0.,
            works: 1,
            stage: stages[i],
        })
        .collect();
    chart
}
fn coasters(b: &Battle) -> impl Iterator<Item = &Raider> {
    b.raiders.iter().filter(|r| r.class == EnemyClass::Coaster)
}

#[test]
fn each_base_keeps_a_squadron_at_anchor_that_sails_when_it_is_roused() {
    let mut b = Battle::for_chart(1000., &coastwise_chart([0, 1, 1]));
    // Its torpedo boats wait at their own mooring to be sent out (see `torpedo_boat`).
    let garrison = |b: &Battle, base: usize| {
        b.raiders
            .iter()
            .filter(|r| r.moored_at() == Some(base) && !r.class.is_merchant())
            .filter(|r| r.class != EnemyClass::TorpedoBoat)
            .count()
    };
    // The base at the mouth keeps a light garrison and the last stage a squadron.
    for base in 0..3 {
        assert_eq!(garrison(&b, base), b.squadron(base).len(), "base {base}");
    }
    assert_eq!(b.squadron(0), GARRISON);
    assert!(b.squadron(1).len() > GARRISON.len());
    let orders = Orders {
        player: Vec2::splat(-3000.),
        ..distant_observer()
    };
    // The first base is awake from the start, so its garrison sails at once. The rest
    // ride at anchor, and count for nothing against the yards' launches.
    b.step(0.1, &orders);
    assert_eq!(garrison(&b, 0), 0);
    let (one, two) = (b.squadron(1).len(), b.squadron(2).len());
    assert_eq!((garrison(&b, 1), garrison(&b, 2)), (one, two));
    let at_sea = b.combatants();
    b.ports[1].rouse(b.at_sea);
    b.step(0.1, &orders);
    assert_eq!(
        garrison(&b, 1),
        0,
        "the second base's squadron sails with it"
    );
    assert!(b.combatants() >= at_sea + one);
}

#[test]
fn a_punishing_passage_moors_twice_the_squadrons_and_its_yards_work_twice_over() {
    let chart = coastwise_chart([0, 1, 1]);
    let charted = Battle::for_chart(60., &chart).with_squadrons(3);
    let doubled = Battle::for_chart(60., &chart)
        .with_squadrons(3)
        .with_strength(2);
    assert_eq!((charted.strength(), doubled.strength()), (1, 2));
    let garrison = |b: &Battle, base: usize| {
        (b.raiders.iter())
            .filter(|r| r.moored_at() == Some(base) && !r.class.is_merchant())
            .filter(|r| r.class != EnemyClass::TorpedoBoat)
            .count()
    };
    for base in 0..3 {
        assert_eq!(garrison(&doubled, base), 2 * garrison(&charted, base), "base {base}");
    }
    // Steel comes in twice as quick, and the slips fit out in half the time.
    for (a, b) in charted.ports.iter().zip(&doubled.ports) {
        assert_eq!(b.production_per_minute(), 2. * a.production_per_minute());
        for (a, b) in a.docks().iter().zip(b.docks()) {
            assert_eq!(b.slipway_clock(), a.slipway_clock() / 2.);
        }
    }
    assert_eq!(doubled.wave_interval, charted.wave_interval / 2.);
    // Setting it again changes nothing.
    let again = doubled.with_strength(2);
    assert_eq!(
        again.ports[0].production_per_minute(),
        2. * charted.ports[0].production_per_minute()
    );
}

#[test]
fn coasters_ride_at_anchor_until_their_base_is_roused_then_ply_to_the_next_yard() {
    let mut b = coastwise([0, 1, 2]);
    let orders = Orders {
        player: Vec2::splat(-3000.),
        ..distant_observer()
    };
    // Two lie in the roadstead off each base, clear of its slip, with no way on.
    assert_eq!(coasters(&b).count(), 3 * MOORED_COASTERS);
    for r in coasters(&b) {
        let base = r.moored_at().unwrap();
        let berth = b.ports[base].berth();
        assert!((30. ..120.).contains(&r.body.pos.distance(berth)));
        assert_eq!(r.cargo(), COASTER_CARGO);
    }
    let anchored: Vec<(u64, Vec2)> = coasters(&b)
        .filter(|r| r.moored_at() != Some(0))
        .map(|r| (r.body.id, r.body.pos))
        .collect();
    for _ in 0..200 {
        b.step(0.1, &orders);
    }
    // The first base is roused from the start, and its coasters are under way for the
    // next yard. The sleeping bases' coasters have not stirred, current or no.
    let roused = coasters(&b).filter(|r| r.destination_port() == Some(1));
    assert_eq!(roused.count(), MOORED_COASTERS);
    for (id, at) in &anchored {
        let r = b.ship(*id).unwrap();
        assert!(r.moored_at().is_some() && r.body.pos == *at);
        assert!(!r.has_contact);
    }
    // Then the roused yard sends out a group from its slip, one at a time.
    let slip = b.ports[0].berth();
    let mut launched = Vec::new();
    let mut most = 0;
    let mut steel = b.ports[1].stock;
    for _ in 0..2600 {
        b.step(0.1, &orders);
        // A coaster's trade is her own: the next yard's steel grows only by its own
        // works, a unit at a time, never by a delivery.
        assert!(b.ports[1].stock <= steel + 1);
        steel = b.ports[1].stock;
        for r in coasters(&b) {
            if r.body.pos.distance(slip) < 8. && !launched.contains(&r.body.id) {
                launched.push(r.body.id);
                assert_eq!(r.destination_port(), Some(1));
            }
        }
        most = most.max(coasters(&b).count());
    }
    assert!(launched.len() >= 2, "{launched:?}");
    assert!(most <= COASTER_LIMIT + 2, "{most}");
    // The first have come in to the next yard and gone alongside.
    assert!(coasters(&b).count() < most);
    assert!(b.raiders.iter().all(|r| r.class == EnemyClass::Coaster));
}

#[test]
fn a_wrecked_yard_sends_no_coasters_and_those_bound_for_it_turn_back() {
    let mut b = coastwise([0, 0, 0]);
    let orders = Orders {
        player: Vec2::splat(-3000.),
        ..distant_observer()
    };
    for _ in 0..100 {
        b.step(0.1, &orders);
    }
    // All three bases are roused, and every coaster has weighed for its nearest yard.
    assert!(coasters(&b).all(|r| r.moored_at().is_none()));
    let inbound: Vec<u64> = coasters(&b)
        .filter(|r| r.destination_port() == Some(1))
        .map(|r| r.body.id)
        .collect();
    assert_eq!(inbound.len(), 2 * MOORED_COASTERS);
    b.ports[1].wreck();
    b.ports[1].coaster_clock = 0.;
    let slip = b.ports[1].berth();
    for _ in 0..1500 {
        b.step(0.1, &orders);
        assert!(coasters(&b).all(|r| r.body.pos.distance(slip) > 20.));
        assert!(b.ports[1].coasters_due.is_none());
    }
    // Those bound for the wreck went back where they came from and in.
    assert!(inbound.iter().all(|&id| b.ship(id).is_none()));
    assert!(coasters(&b).all(|r| r.destination_port() != Some(1)));
    // The yards still standing trade with each other.
    assert!(coasters(&b).any(|r| r.destination_port() == Some(2)));
}

#[test]
fn hits_put_cargo_over_the_side_in_proportion_and_the_rest_goes_down_with_her() {
    let mut b = battle().with_rewards(Rewards::Direct);
    b.raiders.clear();
    let orders = distant_observer();
    let at = Vec2::new(-75., -125.);
    b.spawn(EnemyClass::Freighter, at, Vec2::ZERO);
    let structure = b.raiders[0].class.design().structure();
    let mut sunk = 0.;
    for damage in [4_u16, 2] {
        b.raiders[0].hit_hull(at, f32::from(damage));
        let e = b.step(1. / 60., &orders);
        let share = f32::from(damage) / structure * FREIGHTER_CARGO;
        assert!((e.cargo - share).abs() <= 1. / 1024., "{} {share}", e.cargo);
        sunk += e.cargo;
    }
    assert_eq!(b.raiders[0].cargo() + sunk, FREIGHTER_CARGO);
    // A crew that patches her hull cannot bring back what has gone over the side.
    b.step(1., &orders);
    scuttle(&mut b.raiders[0]);
    let e = b.step(1. / 60., &orders);
    assert_eq!((e.sunk, e.coasters, e.recovered), (1, 0, 4));
    assert_eq!(sunk + e.cargo, FREIGHTER_CARGO);

    // A coaster is easy work: one stock main round leaves her adrift and one round
    // from the secondaries sinks her. She pays her cargo and no stores.
    let at = Vec2::new(-60., -110.);
    b.spawn(EnemyClass::Coaster, at, at + Vec2::X);
    let mut cargo = 0.;
    for (battery, damage) in [(Battery::Main, 2), (Battery::Secondary, 1)] {
        let target = b.raiders[0].aim_point(at + Vec2::new(0., 90.));
        let eye = target + Vec3::new(0., 3., 90.);
        fire(
            &mut b.shots,
            &mut b.effects,
            &mut b.serial,
            eye,
            target,
            battery,
            damage,
        );
        let mut events = Events::default();
        b.advance_shots(0.5, Vec2::splat(-1800.), &mut events);
        assert_eq!(events.hits, 1, "{battery:?}");
        if battery == Battery::Main {
            assert!(b.raiders[0].is_disabled() && b.raiders[0].afloat());
        }
        let e = b.step(1. / 60., &orders);
        cargo += e.cargo;
        if battery == Battery::Secondary {
            assert_eq!((e.sunk, e.coasters, e.recovered), (1, 1, 0));
        }
    }
    assert_eq!(cargo, COASTER_CARGO);
    assert!(b.raiders.is_empty());

    // Under packages she leaves nothing to tow.
    let mut b = battle();
    b.raiders.clear();
    let loose = b.salvage.len();
    b.spawn(EnemyClass::Coaster, at, at + Vec2::X);
    scuttle(&mut b.raiders[0]);
    let e = b.step(1. / 60., &distant_observer());
    assert_eq!((e.sunk, e.coasters, e.cargo), (1, 1, COASTER_CARGO));
    assert_eq!(b.salvage.len(), loose);
}

#[test]
fn the_trigger_offers_first_the_smallest_gun_that_can_hurt_the_marked_ship() {
    let mut b = battle();
    b.raiders.clear();
    b.next_wave = 1000.;
    let design = shipvivor_shipyard::Design::breakwater();
    let bow = design.gun_stations()[0];
    let ap = |inches: u16| MainShell {
        bore: inches - 4,
        ..MainShell::STOCK
    };
    // A twelve-inch turret forward and a four-inch aft.
    b.fit_design(&design, |station| {
        (
            if *station == bow { ap(12) } else { ap(4) },
            TurretLayout::Twin,
        )
    });
    let orders = Orders {
        player: Vec2::ZERO,
        heading: 0.,
        speed: 0.,
        hull: Hull::CRUISER,
        aim: Aim::Track(BEAM_TARGET),
        reload: 1.,
        shell: ap(4),
    };
    let dt = 1. / 60.;
    for _ in 0..900 {
        b.step(dt, &orders);
    }
    for station in Station::ALL {
        assert_eq!(b.main_mount(station).status(), MountStatus::Ready);
    }
    let station = |b: &Battle| b.next_barrel().map(|(station, _)| station);
    // With nothing marked, the smallest gun goes first.
    assert_eq!(station(&b), Some(Station::Stern));
    b.spawn(
        EnemyClass::Patrol,
        Vec2::new(0., 3000.),
        Vec2::new(0., 3100.),
    );
    let patrol = b.raiders.last().unwrap().body.id;
    b.spawn(
        EnemyClass::Parliament,
        Vec2::new(0., -3000.),
        Vec2::new(0., -3100.),
    );
    let flagship = b.raiders.last().unwrap().body.id;
    // A light hull: the four-inch opens it, so the big gun is kept back.
    b.mark(Some(patrol));
    assert_eq!(station(&b), Some(Station::Stern));
    // A flagship screened by her turrets' reinforced plate: the four-inch can't open
    // it out at its reach, so the twelve-inch goes first, both its barrels, and the
    // four-inch only once nothing that can open her is ready.
    b.mark(Some(flagship));
    assert_eq!(station(&b), Some(Station::Bow));
    let mut order = vec![];
    for _ in 0..3 {
        order.push(station(&b));
        b.order_volley(Volley::Single);
        b.step(dt, &orders);
        assert_eq!(fired(&b).len(), 1);
    }
    assert_eq!(
        order,
        [Some(Station::Bow), Some(Station::Bow), Some(Station::Stern)]
    );
}

#[test]
fn the_sight_passes_over_trees_but_not_rock() {
    let mut b = battle();
    b.raiders.clear();
    b.landmarks.clear();
    b.ports.clear();
    b.floe.icebergs.clear();
    b.landmarks.push(Landmark::new(
        body(&mut b.serial, Vec2::new(100., 0.), 0.),
        30.,
        LandmarkKind::Pines,
    ));
    // Low over the islet's rock and into its trees.
    let eye = Vec3::new(0., 10., 0.);
    let ray = (Vec3::new(200., 6., 0.) - eye).normalize();
    let sight = b.sight(eye, ray, Vec2::new(400., 0.));
    assert!(sight.point.x > 130., "{}", sight.point);
    // Rock still stops the sight, and the trees still hide what is behind them from a
    // turret's crew.
    let island = &b.landmarks[0];
    assert_eq!(island.volumes_seen(false).count(), 1);
    assert_eq!(island.volumes_seen(true).count(), 1 + island.features.len());
    let low = (Vec3::new(100., 0., 0.) - eye).normalize();
    let rock = b.sight(eye, low, Vec2::new(400., 0.)).point;
    assert!((69. ..71.).contains(&rock.x), "{rock}");
}

#[test]
fn a_row_of_secondaries_fires_ragged_and_never_through_the_gun_beside_it() {
    let full = Secondaries {
        per_side: Secondaries::MOST_PER_SIDE,
        ..Secondaries::STOCK
    };
    let orders = Orders {
        player: Vec2::ZERO,
        heading: 0.,
        speed: 0.,
        hull: Hull::CRUISER,
        aim: Aim::Hold,
        reload: 6.4,
        shell: MainShell::STOCK,
    };
    // A mark fine on the starboard bow, and another on her quarter. Alone on the beam,
    // the gun abaft the bridge bears on both. In a row it would fire through the guns
    // ahead of it and astern, so only the bow gun bears on the first, and only the gun
    // at the after end of the row on the second.
    let fine = shipvivor_compass::direction(15_f32.to_radians()) * 120.;
    let quarter = shipvivor_compass::direction(170_f32.to_radians()) * 120.;
    let bears = |b: &Battle, mark: Vec2| -> Vec<usize> {
        (0..b.wing_guns.len())
            .filter(|&i| b.wing_guns[i].covers(Vec2::ZERO, 0., mark))
            .collect()
    };
    let mut b = battle();
    assert_eq!(bears(&b, fine), [1, 3]);
    assert_eq!(bears(&b, quarter), [1]);
    b.fit_secondaries(full);
    assert_eq!(bears(&b, fine), [3]);
    let aftermost = (0..b.wing_guns.len())
        .filter(|&i| b.wing_guns[i].deck_x() > 0.)
        .min_by(|&x, &y| {
            let fore = |i: usize| b.wing_guns[i].deck_fore();
            fore(x).total_cmp(&fore(y))
        })
        .unwrap();
    assert_eq!(bears(&b, quarter), [aftermost]);
    // On the beam, every gun of that side but the bow gun.
    assert_eq!(bears(&b, Vec2::X * 120.), [1, 5, 7, 9, 11]);

    // A cruiser off the starboard beam: five guns bear, and each fires in its own time
    // rather than all five together, at the rate they fired before.
    b.raiders.clear();
    b.next_wave = 1000.;
    b.spawn(
        EnemyClass::Cruiser,
        Vec2::new(120., 0.),
        Vec2::new(120., -10.),
    );
    toughen(&mut b.raiders[0], 1000.);
    for gun in b.wing_guns.iter_mut().filter(|gun| gun.deck_fore() > 9.) {
        gun.reload = 100.;
    }
    let mut volleys = Vec::new();
    for _ in 0..300 {
        b.step(1. / 60., &orders);
        let fired = (b.effects.iter())
            .filter(|fx| fx.kind == EffectKind::Muzzle(Battery::Secondary) && fx.age == 0.)
            .count();
        if fired > 0 {
            volleys.push(fired);
        }
    }
    let rounds: usize = volleys.iter().sum();
    assert!((26..=36).contains(&rounds), "{rounds} rounds");
    assert!(volleys.iter().all(|&guns| guns <= 2), "{volleys:?}");
    assert!(volleys.len() * 5 >= rounds * 4, "{volleys:?}");
}

#[test]
fn her_secondaries_open_fire_each_in_its_own_time_and_not_through_the_next_gun() {
    let secondary = |fx: &&Effect| fx.kind == EffectKind::Muzzle(Battery::EnemySecondary);
    let mut b = battle();
    b.raiders.clear();
    b.spawn(EnemyClass::Battleship, Vec2::ZERO, Vec2::NEG_Y);
    let r = &mut b.raiders[0];
    // Out of range off her starboard beam, her six guns a side follow and stand loaded.
    for _ in 0..600 {
        r.fire_guns(
            1. / 60.,
            Vec2::X * 2000.,
            Vec2::ZERO,
            &mut b.shots,
            &mut b.effects,
            &mut b.serial,
        );
    }
    assert_eq!(b.effects.iter().filter(secondary).count(), 0);
    // Within it, they open one after another, not in a single volley, and keep apart.
    let mut volleys = Vec::new();
    for _ in 0..900 {
        let before = b.effects.iter().filter(secondary).count();
        r.fire_guns(
            1. / 60.,
            Vec2::X * 130.,
            Vec2::ZERO,
            &mut b.shots,
            &mut b.effects,
            &mut b.serial,
        );
        let fired = b.effects.iter().filter(secondary).count() - before;
        if fired > 0 {
            volleys.push(fired);
        }
    }
    let rounds: usize = volleys.iter().sum();
    assert!((24..=34).contains(&rounds), "{rounds} rounds");
    assert!(volleys.iter().all(|&guns| guns <= 2), "{volleys:?}");
    assert!(volleys.len() * 5 >= rounds * 4, "{volleys:?}");

    // Fine on her starboard bow, only the gun at the head of the row bears: the others
    // would fire through the guns ahead of them.
    b.effects.clear();
    let fine = shipvivor_compass::direction(15_f32.to_radians()) * 130.;
    for _ in 0..600 {
        r.fire_guns(
            1. / 60.,
            fine,
            Vec2::ZERO,
            &mut b.shots,
            &mut b.effects,
            &mut b.serial,
        );
    }
    let foremost = (r.secondaries.iter())
        .map(|gun| gun.offset().y)
        .fold(f32::MIN, f32::max);
    for gun in &r.secondaries {
        let station = gun.station(&r.body, EnemyClass::Battleship.profile()).xz();
        let fired_here =
            (b.effects.iter().filter(secondary)).any(|fx| fx.body.pos.distance(station) <= 1.81);
        let head = gun.offset().x > 0. && gun.offset().y == foremost;
        assert_eq!(fired_here, head, "{:?}", gun.offset());
    }
}

#[test]
fn a_secondarys_rounds_fall_about_the_mark_and_not_all_on_it() {
    let from = Vec3::new(0., 2., 0.);
    let mark = Vec3::new(160., 1.7, 0.);
    let falls: Vec<Vec3> = (0..400)
        .map(|serial| projectile::strewn(from, mark, serial))
        .collect();
    // At 160 m none falls more than four metres to either side, nor ten short or over.
    for fall in &falls {
        assert!(
            fall.z.abs() <= 4.001 && (fall.x - 160.).abs() <= 9.601,
            "{fall}"
        );
        assert_eq!(fall.y, mark.y);
    }
    // They fall to both sides, short and over, most of them near the middle and about
    // the mark on the whole.
    assert!(falls.iter().any(|f| f.z > 2.) && falls.iter().any(|f| f.z < -2.));
    assert!(falls.iter().any(|f| f.x > 165.) && falls.iter().any(|f| f.x < 155.));
    let wide = falls.iter().filter(|f| f.z.abs() >= 2.).count();
    assert!(wide * 3 < falls.len(), "{wide} wide");
    let middle = falls.iter().sum::<Vec3>() / falls.len() as f32;
    assert!(middle.distance(mark) < 0.8, "{middle}");
    // Twice as far off, twice as wide.
    let far = (0..400).map(|serial| projectile::strewn(from, mark * Vec3::new(2., 1., 1.), serial));
    assert!(far.map(|f| f.z.abs()).fold(0., f32::max) > 6.);
}
