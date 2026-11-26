//! Every generated chart is a channel between two shores: the bases come stage by stage
//! up it, the heavy yard last at its closed head, and the ships that sail it find their
//! way along it.
use super::super::{
    Aim, Battle, Chart, Course, EnemyClass, Hull, LandmarkKind, MainShell, Orders, Survey, Waters,
};
use glam::Vec2;
use std::collections::HashMap;

const SEAS: [Waters; 4] = [Waters::NARROWS, Waters::OPEN, Waters::SOUND, Waters::CUT];

fn charted(seed: u64, waters: &Waters) -> Chart {
    Chart::generate(seed, waters).expect("a chart passes").0
}

/// Metres along a ray to the first land it meets, if any within `reach`.
fn landfall(chart: &Chart, from: Vec2, dir: Vec2, reach: f32) -> Option<f32> {
    chart
        .coast
        .iter()
        .filter_map(|piece| {
            let to = piece.pos - from;
            let along = to.dot(dir);
            let off = to.perp_dot(dir).abs();
            (off < piece.radius).then(|| along - (piece.radius * piece.radius - off * off).sqrt())
        })
        .filter(|&t| t > -1e-3 && t < reach)
        .reduce(f32::min)
}

#[test]
fn every_passage_runs_up_a_channel_stage_by_stage_to_the_heavy_yard_at_its_head() {
    for waters in &SEAS {
        for seed in 1..5 {
            let chart = charted(seed, waters);
            assert_eq!(chart.course, Course::Passage);
            let survey = Survey::of(&chart);
            assert!(
                survey.passed() && survey.metrics.stage_balance > 0.3,
                "{seed}"
            );
            // The bases are the waters' stages in order, the heavy yard last.
            let planned: Vec<(u8, u8)> = waters
                .stages
                .iter()
                .enumerate()
                .flat_map(|(stage, works)| works.iter().map(move |&w| (stage as u8, w)))
                .collect();
            let built: Vec<(u8, u8)> = chart.harbours.iter().map(|h| (h.stage, h.works)).collect();
            assert_eq!(built, planned, "{seed}");
            assert_eq!(chart.harbours.last().map(|h| h.works), Some(3));
            assert_eq!(chart.stages(), waters.stages.len());
            // Land lies on both hands all the way up, within a reach's breadth, so the
            // ship has room to manoeuvre and nowhere else to go. A leg may cross the
            // channel on a long slant, so the width is taken across the channel's
            // narrowest line through each point, not square to the leg.
            let course: Vec<Vec2> = std::iter::once(Vec2::ZERO)
                .chain(chart.gates.iter().copied())
                .collect();
            let across = |at: Vec2| {
                (0..36)
                    .filter_map(|k| {
                        let dir = Vec2::from_angle(k as f32 * std::f32::consts::PI / 36.);
                        Some(landfall(&chart, at, dir, 1e4)? + landfall(&chart, at, -dir, 1e4)?)
                    })
                    .reduce(f32::min)
            };
            for leg in course.windows(2) {
                for k in 1..4 {
                    let at = leg[0].lerp(leg[1], k as f32 / 4.);
                    let width = across(at);
                    assert!(
                        width.is_some_and(|w| w < waters.breadth.1 + 150.),
                        "{seed}: open water beside {at}, {width:?} m across"
                    );
                }
            }
            // The head is closed beyond the last mark.
            let end = course[course.len() - 1];
            let up = (end - course[course.len() - 2]).normalize();
            assert!(
                landfall(&chart, end, up, 600.).is_some(),
                "{seed}: the head is open beyond {end}"
            );
            // Each stage lies beyond the last along the course, and the bases of a pair
            // stand on opposite shores.
            let side = |p: Vec2| {
                let (i, _) = course
                    .windows(2)
                    .enumerate()
                    .min_by(|a, b| {
                        let d = |w: &[Vec2]| super::segment_distance(p, w[0], w[1]);
                        d(a.1).total_cmp(&d(b.1))
                    })
                    .unwrap();
                (course[i + 1] - course[i]).perp_dot(p - course[i]).signum()
            };
            for pair in chart.harbours.windows(2) {
                if pair[0].stage == pair[1].stage {
                    assert_ne!(side(pair[0].pos), side(pair[1].pos), "{seed}");
                }
            }
            // A glacier comes down to the water somewhere along the shores or at the head.
            assert!(
                chart.coast.iter().any(|i| i.kind == LandmarkKind::Glacier),
                "{seed}: no glacier"
            );
            // The set runs across the mouth, and the water of each reach turns.
            let along = (end - Vec2::ZERO).normalize();
            assert!(chart.set.flow.normalize().dot(along).abs() < 0.5, "{seed}");
            assert!(chart.gyres.len() >= chart.stages(), "{seed}");
        }
    }
}

/// Ships launched from the bays, chasing the cruiser up the channel or heading for the
/// patrol ground, never sit against a cliff or a quay.
#[test]
fn raiders_sail_a_passage_without_running_aground_in_its_coves() {
    for (seed, waters) in [(1, Waters::NARROWS), (4, Waters::CUT)] {
        let chart = charted(seed, &waters);
        let mut b = Battle::for_chart(9.45, &chart).with_flagship();
        let route: Vec<Vec2> = std::iter::once(Vec2::ZERO).chain(chart.gates).collect();
        let (mut player, mut leg) = (Vec2::ZERO, 0);
        let mut last: HashMap<u64, (Vec2, f32, f32)> = HashMap::new();
        for tick in 0..6000 {
            let t = tick as f32 * 0.1;
            if let Some(&to) = route.get(leg + 1) {
                if player.distance(to) < 3. {
                    leg += 1;
                } else {
                    player += (to - player).normalize() * 0.6;
                }
            }
            let heading =
                shipvivor_compass::heading(route[(leg + 1).min(route.len() - 1)] - player);
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
