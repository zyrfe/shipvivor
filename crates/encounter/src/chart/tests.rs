//! Every generated chart is a passage along a coast: the bases come in order along it,
//! the heaviest at the far end, and the ships that sail it find their way along the shore.
use super::super::{
    Aim, Battle, Chart, Course, EnemyClass, Hull, LandmarkKind, MainShell, Orders, Survey, Waters,
};
use glam::Vec2;
use std::collections::HashMap;

const SEAS: [Waters; 4] = [Waters::NARROWS, Waters::OPEN, Waters::SOUND, Waters::CUT];

fn charted(seed: u64, waters: &Waters) -> Chart {
    Chart::generate(seed, waters).expect("a chart passes").0
}

#[test]
fn every_passage_runs_along_one_coast_past_a_then_b_to_c_at_the_far_end() {
    let mut sides = [0; 2];
    for waters in &SEAS {
        for seed in 1..6 {
            let chart = charted(seed, waters);
            assert_eq!(chart.course, Course::Passage);
            let survey = Survey::of(&chart);
            assert!(
                survey.passed() && survey.metrics.stage_balance > 0.3,
                "{seed}"
            );
            // Alongside the course the coast lies to one side of it, the bases between it
            // and the course. Beyond the end the last cape juts out across its line.
            let end = chart.gates[4];
            let side = |p: Vec2| end.perp_dot(p).signum();
            let land = side(chart.harbours[0].pos);
            assert!(chart.coast.len() > 20);
            let alongside = |p: Vec2| (0. ..end.length()).contains(&p.dot(end.normalize()));
            for piece in chart.coast.iter().filter(|piece| alongside(piece.pos)) {
                assert_eq!(side(piece.pos), land, "{seed}: {piece:?}");
            }
            sides[usize::from(land > 0.)] += 1;
            // A, B and C are met in that order, and C is the last thing on the course.
            let run = chart.harbours.map(|h| h.pos.dot(end.normalize()));
            assert!(run[0] < run[1] && run[1] < run[2], "{seed}: {run:?}");
            assert!(chart.harbours[2].pos.distance(end) < 260., "{seed}");
            // The glaciers come down to the sea from the coast, or stand offshore.
            let glaciers = chart
                .coast
                .iter()
                .chain(&chart.islets)
                .filter(|i| i.kind == LandmarkKind::Glacier)
                .count();
            assert!(glaciers >= waters.glaciers.0, "{seed}: {glaciers} glaciers");
            // The set runs along the coast.
            let along = end.normalize();
            assert!(chart.set.flow.normalize().dot(along).abs() > 0.9);
        }
    }
    // The coast lies to port on some charts and to starboard on others.
    assert!(sides.iter().all(|&n| n >= 4), "{sides:?}");
}

/// Ships launched from the bays, chasing the cruiser along the coast or heading for the
/// patrol ground, never sit against a cliff or a quay.
#[test]
fn raiders_sail_a_passage_without_running_aground_in_its_coves() {
    for (seed, waters) in [(1, Waters::NARROWS), (4, Waters::CUT)] {
        let chart = charted(seed, &waters);
        let mut b = Battle::for_chart(9.45, &chart).with_flagship();
        let route: Vec<Vec2> = std::iter::once(Vec2::ZERO).chain(chart.gates).collect();
        let (mut player, mut leg) = (Vec2::ZERO, 0);
        let mut last: HashMap<u64, (Vec2, f32, f32)> = HashMap::new();
        for tick in 0..3000 {
            let t = tick as f32 * 0.1;
            if let Some(&to) = route.get(leg + 1) {
                if player.distance(to) < 3. {
                    leg += 1;
                } else {
                    player += (to - player).normalize() * 0.6;
                }
            }
            let heading = shipvivor_compass::heading(route[(leg + 1).min(5)] - player);
            b.step(
                0.1,
                &Orders {
                    player,
                    heading,
                    speed: 6.,
                    hull: Hull::CRUISER,
                    aim: Aim::Hold,
                    reload: 6.4,
                    shell: MainShell::STOCK,
                },
            );
            if tick % 50 != 0 {
                continue;
            }
            for r in b.raiders() {
                let room = r.class.profile().hull.radius + 4.;
                let aground = b.navigable_position(r.body.pos, room).distance(r.body.pos) > 0.01;
                let (from, since, stuck) = last.entry(r.body.id).or_insert((r.body.pos, t, 0.));
                if t - *since < 5. {
                    continue;
                }
                let idle = r.body.pos.distance(*from) < 4.
                    && aground
                    && !r.is_disabled()
                    && r.class != EnemyClass::Freighter;
                *stuck = if idle { *stuck + t - *since } else { 0. };
                (*from, *since) = (r.body.pos, t);
                assert!(
                    *stuck < 20.,
                    "seed {seed}: {:?} held against the land at {} at {t} s",
                    r.class,
                    r.body.pos
                );
            }
        }
    }
}
