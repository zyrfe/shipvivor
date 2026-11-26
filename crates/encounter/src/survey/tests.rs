//! The survey must catch charts that cannot be won, and the generator must never hand
//! one over. Each test takes a chart that passes and spoils it one way.
use super::super::{Chart, Islet, LandmarkKind, Waters};
use super::{Defect, Survey};
use glam::Vec2;
use shipvivor_compass::direction;

fn charted(seed: u64) -> Chart {
    Chart::generate(seed, &Waters::NARROWS)
        .expect("a chart passes")
        .0
}

fn islet(pos: Vec2, radius: f32) -> Islet {
    Islet {
        pos,
        radius,
        kind: LandmarkKind::Stacks,
        heading: 0.,
    }
}

/// A closed ring of overlapping islands, which no ship can pass.
fn wall(around: Vec2, ring: f32) -> Vec<Islet> {
    let count = (std::f32::consts::TAU * ring / 26.).ceil() as usize;
    (0..count)
        .map(|i| {
            let angle = std::f32::consts::TAU * i as f32 / count as f32;
            islet(around + direction(angle) * ring, 18.)
        })
        .collect()
}

fn defects(chart: &Chart) -> Vec<Defect> {
    Survey::of(chart).defects
}

#[test]
fn generated_charts_pass_and_replay_from_their_seed() {
    for waters in [Waters::NARROWS, Waters::OPEN, Waters::SOUND, Waters::CUT] {
        for seed in 1..6 {
            let (chart, survey) = Chart::generate(seed, &waters).expect("a chart passes");
            assert!(survey.passed());
            assert_eq!(
                Survey::of(&chart),
                survey,
                "the screen and the full survey agree"
            );
            assert!(survey.score > 0.5, "{seed}: {}", survey.score);
            assert_eq!(Chart::generate(seed, &waters).unwrap().0, chart);
        }
    }
    assert_ne!(charted(1).gates, charted(2).gates);
}

#[test]
fn an_island_on_a_gate_blocks_it() {
    let mut chart = charted(3);
    let gate = chart.gates[2];
    chart.islets.push(islet(gate + Vec2::X * 10., 24.));
    assert!(defects(&chart).contains(&Defect::GateBlocked { gate: 2 }));
}

#[test]
fn a_wall_round_a_gate_cuts_it_off() {
    let mut chart = charted(4);
    let gate = chart.gates[3];
    chart.islets.extend(wall(gate, 110.));
    let found = defects(&chart);
    assert!(
        found.contains(&Defect::GateUnreachable { gate: 3 }),
        "{found:?}"
    );
    assert!(!found.contains(&Defect::GateBlocked { gate: 3 }));
}

#[test]
fn a_walled_harbour_can_neither_launch_nor_be_shelled() {
    let mut chart = charted(5);
    // The heavy yard, with three factories.
    let base = chart.harbours.len() - 1;
    let harbour = chart.harbours[base].pos;
    // Beyond the slip's launching lane, so the slip is clear but leads nowhere.
    chart.islets.extend(wall(harbour, 175.));
    let found = defects(&chart);
    assert!(
        found.contains(&Defect::BerthUnreachable { base }),
        "{found:?}"
    );
    assert!(found.contains(&Defect::YardHidden { base }), "{found:?}");
    assert!(found.contains(&Defect::FactoryHidden { base, factory: 2 }));
}

#[test]
fn a_wall_round_the_start_strands_the_ship() {
    let mut chart = charted(6);
    chart.islets.extend(wall(Vec2::ZERO, 90.));
    let found = defects(&chart);
    for gate in 0..chart.gates.len() {
        assert!(
            found.contains(&Defect::GateUnreachable { gate }),
            "{found:?}"
        );
    }
    assert!(found.contains(&Defect::BerthUnreachable { base: 0 }));
}

#[test]
fn bunched_bases_and_crowded_islands_fail() {
    let mut chart = charted(7);
    chart.harbours[1].pos = chart.harbours[0].pos + Vec2::new(150., 150.);
    let a = chart.islets[0];
    chart
        .islets
        .push(islet(a.pos + Vec2::X * (a.radius + 20. + 10.), 20.));
    let found = defects(&chart);
    assert!(
        found
            .iter()
            .any(|d| matches!(d, Defect::BasesBunched { a: 0, b: 1, .. })),
        "{found:?}"
    );
    assert!(
        found
            .iter()
            .any(|d| matches!(d, Defect::StraitTooNarrow { a: 0, .. })),
        "{found:?}"
    );
}

#[test]
fn a_hairpin_course_fails() {
    let mut chart = charted(8);
    // Gate 2 folded back almost onto the leg from gate 1.
    let (g1, g2) = (chart.gates[1], chart.gates[2]);
    chart.gates[3] = g2 + (g1 - g2).normalize().rotate(Vec2::from_angle(0.15)) * 260.;
    let found = defects(&chart);
    assert!(
        found
            .iter()
            .any(|d| matches!(d, Defect::Hairpin { gate: 2, .. })),
        "{found:?}"
    );
}

#[test]
fn a_passage_takes_its_bases_in_order_with_the_heavy_yard_last() {
    let mut chart = charted(9);
    // The heavy yard and the first base change places, keeping their stages.
    let last = chart.harbours.len() - 1;
    let (first, heavy) = (chart.harbours[0], chart.harbours[last]);
    (chart.harbours[0].pos, chart.harbours[0].heading) = (heavy.pos, heavy.heading);
    (chart.harbours[last].pos, chart.harbours[last].heading) = (first.pos, first.heading);
    let found = defects(&chart);
    assert!(
        found
            .iter()
            .any(|d| matches!(d, Defect::BasesOutOfOrder { a: 1, b, .. } if *b == last)),
        "{found:?}"
    );
    assert!(
        found
            .iter()
            .any(|d| matches!(d, Defect::BasesOutOfOrder { a: 0, b: 1, .. })),
        "{found:?}"
    );
    assert!(
        found
            .iter()
            .any(|d| matches!(d, Defect::LastBaseShort { .. })),
        "{found:?}"
    );
}

#[test]
fn an_island_hard_under_the_cliffs_fails() {
    let mut chart = charted(10);
    // The piece of shore nearest the start, and an island just off it.
    let shore = *chart
        .coast
        .iter()
        .min_by(|a, b| (a.pos.length() - a.radius).total_cmp(&(b.pos.length() - b.radius)))
        .unwrap();
    let out = -shore.pos.normalize();
    chart
        .islets
        .push(islet(shore.pos + out * (shore.radius + 15. + 20.), 20.));
    let islands = chart.islets.len();
    let found = defects(&chart);
    assert!(
        found
            .iter()
            .any(|d| matches!(d, Defect::StraitToCoast { a, .. } if *a == islands - 1)),
        "{found:?}"
    );
}
