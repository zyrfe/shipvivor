use crate::{
    Aim, Battery, Battle, Chart, Course, EffectKind, Events, HarbourSite, Hull, IndustrialYard,
    MainShell, Orders, fire,
};
use glam::{Vec2, Vec3, Vec3Swizzles};

fn works(kind: IndustrialYard) -> Battle {
    let mut chart = Chart::classic([Vec2::NEG_Y * 200.; 5]);
    chart.course = Course::Passage;
    chart.harbours = vec![HarbourSite {
        pos: Vec2::ZERO,
        heading: 0.,
        works: kind.works(),
        stage: 0,
    }];
    chart.islets.clear();
    Battle::chart_sector(9999., &chart).with_heavy_battery(&chart)
}

fn shell(b: &mut Battle, eye: Vec3, target: Vec3) -> Events {
    fire(
        &mut b.shots,
        &mut b.effects,
        &mut b.serial,
        eye,
        target,
        Battery::Main,
        12,
    );
    let mut events = Events::default();
    b.advance_shots(1., eye.xz(), &mut events);
    events
}

fn strike_factory(b: &mut Battle, index: usize) -> Events {
    let port = &b.ports[0];
    let at = port.factory_position(index);
    let height = (port.factory_bottom(index) + port.factory_top(index)) * 0.5;
    let mark = at.extend(height).xzy();
    // A captain can sail down either flank and bring the nearest governor to bear.
    let across = if index == 0 { -46. } else { 46. };
    let eye = mark + Vec3::new(across, 0., -100.);
    assert_eq!(
        b.sight(eye, (mark - eye).normalize(), mark.xz()).factory,
        Some((0, index))
    );
    shell(b, eye, mark)
}

#[test]
fn governors_bring_down_the_machine_and_the_cliff_guns_run_out_before_firing() {
    for kind in [IndustrialYard::RedShift, IndustrialYard::BellWorks] {
        let mut b = works(kind);
        let cliff = b.ports[0]
            .turrets()
            .iter()
            .position(|g| g.cliff_battery())
            .unwrap();
        let wall = b.ports[0].industrial_center().extend(5.5).xzy();
        let behind = wall + Vec3::Z * 90.;
        let masonry = b.sight(behind, -Vec3::Z, wall.xz());
        assert!(masonry.factory.is_none() && masonry.point.z > wall.z + 5.);
        shell(&mut b, behind, wall);
        assert_eq!(b.ports[0].live_factories(), 2);
        let mark = b.ports[0].turrets()[cliff]
            .position(&b.ports[0].body)
            .extend(b.ports[0].turrets()[cliff].aim_height())
            .xzy();
        let eye = mark - Vec3::Z * 90.;
        let before = b.ports[0].turrets()[cliff].strength();
        assert_eq!(b.sight(eye, Vec3::Z, mark.xz()).shore_turret, None);
        shell(&mut b, eye, mark);
        assert_eq!(b.ports[0].turrets()[cliff].strength(), before);
        strike_factory(&mut b, 0);
        assert_eq!(b.ports[0].industrial_collapse(), None);
        strike_factory(&mut b, 1);
        assert_eq!(b.ports[0].industrial_collapse(), Some(0.));
        assert!(
            b.effects
                .iter()
                .any(|e| e.kind == EffectKind::Demolition { dock: false })
        );
        let orders = Orders {
            player: eye.xz(),
            heading: 0.,
            speed: 0.,
            hull: Hull::CRUISER,
            aim: Aim::Hold,
            reload: 3.,
            shell: MainShell::STOCK,
        };
        for tick in 0..120 {
            b.advance_effects(0.05);
            b.update_lookouts(0.05, orders.player);
            b.range_heavy_guns(0.05, &orders, Vec2::ZERO);
            if tick == 59 {
                let gun = &b.ports[0].turrets()[cliff];
                assert!((0.1..0.9).contains(&gun.deployment()) && !gun.live());
            }
            if tick < 115 {
                assert!(b.shots.iter().all(|s| s.battery != Battery::Heavy));
            }
        }
        assert!(b.ports[0].turrets()[cliff].live());
        // The exposed mount is hittable at the same altitude as its rendered barrels.
        assert_eq!(
            b.sight(eye, Vec3::Z, mark.xz()).shore_turret,
            Some((0, cliff))
        );
        for _ in 0..80 {
            b.update_lookouts(0.05, orders.player);
            b.range_heavy_guns(0.05, &orders, Vec2::ZERO);
        }
        assert!(
            b.shots
                .iter()
                .any(|s| s.battery == Battery::Heavy && s.position().y > 22.)
        );
        for index in 0..b.ports[0].docks().len() {
            b.ports[0].damage_dock(index, u16::MAX);
        }
        assert!(b.ports[0].factories()[IndustrialYard::FUEL_TANK].live());
        assert!(b.ports[0].wrecked());
    }
}

#[test]
fn an_exploding_tank_clears_the_nearby_quay_gun_once_and_leaves_the_mill_running() {
    let mut b = works(IndustrialYard::RedShift);
    let tank = IndustrialYard::FUEL_TANK;
    let events = strike_factory(&mut b, tank);
    assert_eq!(events.guns_silenced, 1);
    assert_eq!(b.ports[0].live_factories(), 2);
    let wreck = b.ports[0].factory_position(tank).extend(4.).xzy();
    shell(&mut b, wreck + Vec3::new(100., 0., -70.), wreck);
    assert_eq!(
        b.effects
            .iter()
            .filter(|e| matches!(e.kind, EffectKind::FuelBlast { depot: true }))
            .count(),
        1
    );
}

#[test]
fn skerries_allow_a_single_gun_duel_then_open_to_several_on_a_rush() {
    let b = works(IndustrialYard::RedShift);
    let port = &b.ports[0];
    let approach = |x, fore| port.local(Vec2::new(x, fore));
    let guns = |player| {
        port.turrets()
            .iter()
            .filter(|gun| gun.covers(&port.body, player))
            .count()
    };
    assert_eq!(guns(approach(-50., 390.)), 1);
    assert!(guns(approach(0., 140.)) >= 4);
    for across in [-1., 1.] {
        let route = approach(across * 170., 85.);
        assert_eq!(b.navigable_position(route, 10.), route);
    }
    for (at, _, _) in port.industrial_rocks() {
        assert_ne!(b.navigable_position(at, 3.), at);
    }
}
