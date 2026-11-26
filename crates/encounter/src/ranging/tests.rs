use super::*;
use crate::{Aim, EffectKind, HarbourSite, Hull, MainShell, Port};
use shipvivor_compass::direction;

const COURSE: f32 = 0.3;
const SPEED: f32 = 9.;

/// The errors of `salvos` salvos laid on her holding one course and speed.
fn steady(spotted: bool, salvos: usize) -> Vec<f32> {
    let mut plot = Ranging {
        spotted,
        in_sight: true,
        ..Ranging::default()
    };
    (0..salvos).map(|_| plot.lay(COURSE, SPEED, true)).collect()
}

/// A plot five salvos into a steady course, and what it lays as she then sails `on`.
fn after(on: &[(f32, f32)]) -> (f32, Vec<f32>) {
    let mut plot = Ranging {
        in_sight: true,
        ..Ranging::default()
    };
    for _ in 0..5 {
        plot.lay(COURSE, SPEED, true);
    }
    let close = plot.error().unwrap().abs();
    let laid = on
        .iter()
        .map(|&(h, s)| plot.lay(h, s, true).abs())
        .collect();
    (close, laid)
}

#[test]
fn a_steady_course_walks_the_salvos_in_over_then_short() {
    let errors = steady(false, 6);
    assert_eq!(errors[0], FIRST_ERROR.0, "{errors:?}");
    assert!(errors[1] < -50., "{errors:?}");
    for pair in errors.windows(2) {
        assert!(pair[0].signum() != pair[1].signum(), "{errors:?}");
        assert!(pair[1].abs() <= 0.55 * pair[0].abs(), "{errors:?}");
    }
    // By the fifth salvo the nearer shell of the pair falls on her, and by the sixth the
    // pair falls about her: one over and one short.
    assert!(errors[4].abs() < SPREAD, "{errors:?}");
    assert!(errors[5].abs() < 0.5 * SPREAD, "{errors:?}");
    // The other way about, the first falls short.
    let mut plot = Ranging::default();
    assert_eq!(plot.lay(COURSE, SPEED, false), -FIRST_ERROR.0);
}

#[test]
fn a_real_turn_or_change_of_speed_throws_the_plot_out_and_it_closes_again() {
    let degrees = |d: f32| d.to_radians();
    // Small changes are only her helm and telegraph at work.
    let (close, laid) = after(&[(COURSE + degrees(10.), SPEED - 1.)]);
    assert!(laid[0] < close, "{close} then {laid:?}");
    // A turn of thirty degrees grows the error back.
    let (close, laid) = after(&[(COURSE - degrees(30.), SPEED)]);
    assert!(laid[0] >= REGROWN * FIRST_ERROR.0 && laid[0] > 3. * close);
    // So does losing a third of her way, or getting under way from stopped.
    let (_, laid) = after(&[(COURSE, SPEED * 0.6)]);
    assert!(laid[0] >= REGROWN * FIRST_ERROR.0, "{laid:?}");
    let (_, laid) = after(&[(COURSE, 0.), (COURSE, 4.)]);
    assert!(
        laid.iter().all(|&e| e >= REGROWN * FIRST_ERROR.0),
        "{laid:?}"
    );
    // Holding the new course, the plot closes once more.
    let turned = COURSE + degrees(40.);
    let (_, laid) = after(&[(turned, SPEED); 4]);
    assert!(laid.windows(2).all(|w| w[1] < w[0]), "{laid:?}");
    // Weaving across her course every salvo keeps it from ever closing.
    let weave = [degrees(30.), degrees(-30.)].map(|d| (COURSE + d, SPEED));
    let (_, laid) = after(&[weave[0], weave[1], weave[0], weave[1], weave[0]]);
    assert!(
        laid.iter().all(|&e| e >= REGROWN * FIRST_ERROR.0),
        "{laid:?}"
    );
}

#[test]
fn a_spotter_reporting_her_starts_the_plot_closer_and_closes_it_faster() {
    let alone = steady(false, 8);
    let spotted = steady(true, 8);
    assert!(spotted[0].abs() < 0.7 * alone[0].abs());
    let fine = |errors: &[f32]| errors.iter().position(|e| e.abs() < 6.).unwrap();
    assert!(
        fine(&spotted) + 2 <= fine(&alone),
        "spotted {spotted:?}, alone {alone:?}"
    );
    // Thrown out, it grows back less far with a spotter too.
    let mut plot = Ranging {
        spotted: true,
        ..Ranging::default()
    };
    for _ in 0..5 {
        plot.lay(COURSE, SPEED, true);
    }
    assert!(plot.lay(COURSE + 1., SPEED, true).abs() <= FIRST_ERROR.1);
}

#[test]
fn a_plot_goes_stale_once_she_has_been_lost_a_while() {
    let mut plot = Ranging {
        in_sight: true,
        ..Ranging::default()
    };
    for _ in 0..4 {
        plot.lay(COURSE, SPEED, true);
    }
    plot.in_sight = false;
    plot.watch(STALE * 0.5);
    assert!(plot.ranging() && plot.salvos() == 4);
    plot.watch(STALE);
    assert!(!plot.ranging() && plot.salvos() == 0 && plot.error().is_none());
    plot.in_sight = true;
    plot.watch(0.1);
    assert_eq!(plot.lay(COURSE, SPEED, true), FIRST_ERROR.0);
}

/// A heavy yard alone at the head of a passage on open water, with nothing afloat and
/// no harriers to send up. A cannery may stand `cannery`, for a lookout or to hold the
/// stage before the yard's.
fn heavy_yard(cannery: Option<Vec2>) -> Battle {
    staged_yard(cannery, 0)
}
fn staged_yard(cannery: Option<Vec2>, stage: u8) -> Battle {
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
    chart.course = Course::Passage;
    chart.islets.clear();
    let site = |pos, works, stage| HarbourSite {
        pos,
        heading: 0.,
        works,
        stage,
    };
    chart.harbours = std::iter::once(site(Vec2::ZERO, 3, stage))
        .chain(cannery.map(|at| site(at, 1, 0)))
        .collect();
    let mut b = Battle::for_chart(1000., &chart);
    b.raiders.clear();
    b.next_wave = f32::INFINITY;
    b.harrier_due.fill(f32::INFINITY);
    b
}

/// Where the heavy battery stands, and its index among its base's mounts.
fn heavy_gun(b: &Battle) -> (usize, Vec2) {
    let port = &b.ports[0];
    let index = port.turrets().iter().position(ShoreTurret::heavy_battery);
    let index = index.expect("the heavy yard mounts the heavy battery");
    (index, port.turrets()[index].position(&port.body))
}

/// What a heavy battery did to her as she sailed: how far from her each salvo's
/// columns fell, in order, and the damage its shells did.
struct Passage {
    falls: Vec<f32>,
    damage: u32,
    rounds: usize,
}

/// Sail her from `start` at `speed` for `secs`, steering the heading `helm` gives at
/// each moment.
fn sail(b: &mut Battle, start: Vec2, speed: f32, secs: f32, helm: impl Fn(f32) -> f32) -> Passage {
    let dt = 1. / 30.;
    let mut at = start;
    let mut passage = Passage {
        falls: vec![],
        damage: 0,
        rounds: 0,
    };
    for tick in 0..(secs / dt) as usize {
        let heading = helm(tick as f32 * dt);
        let orders = Orders {
            player: at,
            heading,
            speed,
            hull: Hull::CRUISER,
            aim: Aim::Hold,
            reload: 3.2,
            shell: MainShell::STOCK,
        };
        let events = b.step(dt, &orders);
        let heavy = |blow: &&crate::Blow| blow.calibre.is_some_and(|c| c > 14.);
        passage.damage += events
            .blows
            .iter()
            .filter(heavy)
            .map(|b| u32::from(b.damage))
            .sum::<u32>();
        for fx in b.effects.iter().filter(|fx| fx.age == 0.) {
            match fx.kind {
                EffectKind::Column => passage.falls.push(fx.body.pos.distance(at)),
                EffectKind::Muzzle(Battery::Heavy) => passage.rounds += 1,
                _ => {}
            }
        }
        at += direction(heading) * speed * dt;
    }
    passage
}

#[test]
fn a_steady_course_under_the_heavy_guns_is_walked_in_on_and_hit() {
    let mut b = heavy_yard(None);
    let (_, gun) = heavy_gun(&b);
    // Nine hundred metres east of the battery, steaming north past it.
    let start = gun + Vec2::new(900., 300.);
    let steady = sail(&mut b, start, SPEED, 90., |_| 0.);
    assert!(steady.rounds >= 8, "{} rounds", steady.rounds);
    let falls = &steady.falls;
    // The first salvo falls well wide of her, and the later ones close in on her.
    assert!(falls[0] > 80., "{falls:?}");
    let late = falls[falls.len() - 4..].iter().sum::<f32>() / 4.;
    assert!(late < 0.3 * falls[0], "{falls:?}");
    assert!(
        steady.damage >= 2 * u32::from(HEAVY_DAMAGE),
        "{} damage",
        steady.damage
    );
    // A captain who weaves thirty degrees either side of her course every nine seconds
    // keeps the plot from closing, and is not hit.
    let mut b = heavy_yard(None);
    let weave = |t: f32| if (t / 9.) as u32 % 2 == 0 { 0.5 } else { -0.5 };
    let weaving = sail(&mut b, start, SPEED, 90., weave);
    assert!(weaving.rounds >= 8);
    assert!(
        weaving.damage <= u32::from(HEAVY_DAMAGE),
        "{}",
        weaving.damage
    );
    assert!(
        weaving.damage < steady.damage / 2,
        "weaving took {}, steady {}",
        weaving.damage,
        steady.damage
    );
}

#[test]
fn the_heavy_guns_reach_far_past_the_light_ones_and_no_further() {
    let mut b = heavy_yard(None);
    let (_, gun) = heavy_gun(&b);
    let out = (gun - b.ports[0].body.pos).normalize();
    // Lying stopped well inside its reach, she is fired on.
    let inside = sail(&mut b, gun + out * (HEAVY_RANGE - 60.), 0., 30., |_| 0.);
    assert!(inside.rounds > 0 && HEAVY_RANGE - 60. > 4. * crate::SHORE_RANGE);
    // Beyond it the battery holds its fire.
    let mut b = heavy_yard(None);
    let outside = sail(&mut b, gun + out * (HEAVY_RANGE + 60.), 0., 30., |_| 0.);
    assert_eq!(outside.rounds, 0);
}

#[test]
fn a_heavy_battery_holds_fire_while_its_base_sleeps_and_once_it_is_silenced() {
    // The heavy yard sleeps at the head of the passage while a cannery stands.
    let mut b = staged_yard(Some(Vec2::new(-3000., 0.)), 1);
    assert!(!b.ports[0].roused());
    let (index, gun) = heavy_gun(&b);
    let start = gun + Vec2::new(900., 300.);
    let asleep = sail(&mut b, start, SPEED, 30., |_| 0.);
    assert_eq!(asleep.rounds, 0);
    assert!(!b.ports[0].roused());
    // Woken, it opens fire.
    b.ports[0].rouse(b.at_sea);
    let awake = sail(&mut b, start, SPEED, 30., |_| 0.);
    assert!(awake.rounds > 0);
    // Silenced like any shore gun, it falls quiet for good.
    b.ports[0].turrets[index].strength = 0;
    b.shots.clear();
    let silenced = sail(&mut b, start, SPEED, 30., |_| 0.);
    assert_eq!(silenced.rounds, 0);
    assert_eq!(silenced.damage, 0);
}

#[test]
fn a_floatplane_or_another_base_spots_for_the_battery_but_its_own_lookout_does_not() {
    let plot = |b: &Battle| *b.ports[0].turrets()[heavy_gun(b).0].ranging().unwrap();
    // Under her own base's floatplane, she is spotted.
    let mut b = heavy_yard(None);
    let player = b.spotters[0].position;
    b.update_lookouts(0., player);
    assert!(b.spotters[0].reporting);
    assert!(plot(&b).in_sight() && plot(&b).spotted());
    // With the floatplane down, the base's own lookout sees her, but that is no spotting.
    b.spotters[0].shoot_down();
    b.update_lookouts(0., player);
    assert!(b.ports[0].has_contact);
    assert!(plot(&b).in_sight() && !plot(&b).spotted());
    // Far out, only the battery's own rangefinder has her.
    let (_, gun) = heavy_gun(&b);
    let far = gun + Vec2::new(1100., 0.);
    b.update_lookouts(0., far);
    assert!(!b.ports[0].has_contact);
    assert!(plot(&b).in_sight() && !plot(&b).spotted());
    // Another base's lookout watching her within call of the battery spots for it.
    let mut b = heavy_yard(Some(far + Vec2::new(-400., 350.)));
    b.spotters[1].shoot_down();
    b.update_lookouts(0., far);
    assert!(b.ports[1].has_contact);
    assert!(plot(&b).spotted());
}

#[test]
fn a_heavy_shell_is_heard_coming_after_its_report_and_matters_against_plate() {
    let mut b = heavy_yard(None);
    let (_, gun) = heavy_gun(&b);
    let start = gun + Vec2::new(1000., 0.);
    sail(&mut b, start, 0., 12., |_| 0.);
    let shell = b
        .shots
        .iter()
        .find(|s| s.battery == Battery::Heavy)
        .expect("in the air");
    // Slower over the water than sound in the scaled sea (170 m/s), so its report
    // arrives first.
    assert!(HEAVY_SPEED < 170.);
    assert!((shell.calibre() - HEAVY_CALIBRE).abs() < 0.01);
    // Four cards of plate are 13 inches, and five are 16.
    assert!(shell.calibre() > 13. && shell.calibre() < 16.);
    // It climbs no higher than a fifth of the range it flies.
    let mut b = heavy_yard(None);
    let mut top = 0_f32;
    for _ in 0..30 * 20 {
        let orders = Orders {
            player: start,
            heading: 0.,
            speed: 0.,
            hull: Hull::CRUISER,
            aim: Aim::Hold,
            reload: 3.2,
            shell: MainShell::STOCK,
        };
        b.step(1. / 30., &orders);
        for s in b.shots.iter().filter(|s| s.battery == Battery::Heavy) {
            top = top.max(s.position().y);
        }
    }
    assert!(top > 60. && top < 0.2 * 1000., "{top}");
}

#[test]
fn only_a_passage_mounts_a_heavy_battery_and_it_stands_on_its_heaviest_base() {
    let classic = Battle::for_route(
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
    let heavy = |b: &Battle| {
        b.ports()
            .iter()
            .map(|p| p.turrets().iter().filter(|g| g.heavy_battery()).count())
            .collect::<Vec<_>>()
    };
    assert_eq!(heavy(&classic), [0, 0, 0]);
    let b = heavy_yard(Some(Vec2::new(-3000., 0.)));
    assert_eq!(heavy(&b), [1, 0]);
    let gun = &b.ports[0].turrets()[heavy_gun(&b).0];
    assert_eq!((gun.barrels, gun.strength()), (2, 18));
    assert!(gun.reach() > 4. * gun_reach(&b.ports[1]));
}

fn gun_reach(port: &Port) -> f32 {
    port.turrets()[0].reach()
}
