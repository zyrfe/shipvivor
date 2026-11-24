//! A survey of a sector chart before anyone sails it.
//!
//! Hard defects reject the chart: a gate the cruiser cannot reach or sail through, a yard
//! she cannot get a clear shot at, a slip the raiders cannot leave, a freighter with no
//! way in, bases bunched together or under each other's guns. Reachability is worked
//! out on a grid of the actual shoals and piers, using the same land tests that keep the
//! ships afloat. A passage must also take its bases in order, the heaviest last, and
//! run past each under its guns. What passes is scored, so the generator can prefer
//! bases spread along the course, easy turns and islands that give cover without
//! walling off the course.
use super::chart::{IceKeepout, segment_distance};
use super::{Battle, Chart, Course, ISLAND_RADIUS, Port, SHORE_RANGE, SPAWN_STANDOFF, Vec2, entry};
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::fmt;

/// Half the spacing of a gate's buoys. It matches `shipvivor_navigation::GATE_HALF_WIDTH`.
pub const GATE_HALF_WIDTH: f32 = 14.;
/// Grid spacing of the water survey in metres.
const CELL: f32 = 8.;
/// Water the cruiser's centre keeps from land on the survey grid: her widest beam, with
/// sea room to turn.
const SEA_ROOM: f32 = 12.;
/// Clear water either side of a gate, along the course, needed to line up and pass it.
const GATE_LANE: f32 = 60.;
/// Narrowest strait between two islands, or an island and a harbour. In anything tighter
/// the larger raiders, which steer round one island at a time, can wedge.
const MIN_STRAIT: f32 = 34.;
const MIN_GATE_SPACING: f32 = 190.;
const MIN_START_TO_GATE: f32 = 140.;
const MIN_LEG: f32 = 200.;
const MIN_LEG_BALANCE: f32 = 0.3;
/// Turns sharper than this at a gate double the ship back on herself.
const HAIRPIN_DEGREES: f32 = 150.;
/// On the second lap the first gate is met from the last; this far off its line at most.
const LAP_APPROACH_DEGREES: f32 = 50.;
const MIN_BASE_SPACING: f32 = 360.;
/// The narrowest angle between two bases, seen from the middle of the circuit.
const MIN_BASE_SECTOR_DEGREES: f32 = 55.;
/// Reinforcements launch only beyond `SPAWN_STANDOFF` of the cruiser, so a base nearer
/// the start than this, less the length of its quay, would sit idle at the opening.
const MIN_BASE_TO_START: f32 = 380.;
/// The classic charts put each base 105 m outside a gate, which plays well.
const MIN_BASE_TO_GATE: f32 = 100.;
/// Room between a base's centre and a fairway, for the quay and the water off it.
const MIN_BASE_TO_LEG: f32 = 95.;
/// Farthest the cruiser should have to stand off to get a clear shot at a building.
const FIRING_RANGE: f32 = 400.;
/// Water path over straight line, at most, on any leg.
const MAX_DETOUR: f32 = 1.7;
const MIN_ISLANDS: usize = 4;
/// Least distance along a passage from one base to the next, in metres.
const MIN_STAGE: f32 = 380.;
/// The heavy yard stands within this distance of the end of a passage.
const MAX_SHORT_OF_END: f32 = 260.;
/// Farthest a passage may run from a base: within reach of its shore guns, and a little more.
const MAX_BASE_TO_COURSE: f32 = SHORE_RANGE + 40.;

const MIN_CONVOY_LANES: usize = 2;
/// Islands within this distance of a leg shape the fighting along it.
const COVER_RANGE: f32 = 150.;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Defect {
    /// Two gates, or the start (index 5) and a gate, too close together.
    GatesCrowded {
        a: usize,
        b: usize,
        metres: f32,
    },
    ShortLeg {
        leg: usize,
        metres: f32,
    },
    UnevenLegs {
        ratio: f32,
    },
    Hairpin {
        gate: usize,
        degrees: f32,
    },
    /// Gate 0 lies too far across the line from the last gate for the next lap.
    LapApproach {
        degrees: f32,
    },
    BasesBunched {
        a: usize,
        b: usize,
        metres: f32,
    },
    BasesOneSided {
        degrees: f32,
    },
    /// On a passage, base `b` comes before base `a` along the course, or too soon after it.
    BasesOutOfOrder {
        a: usize,
        b: usize,
        metres: f32,
    },
    /// The heavy yard stands this far short of the end of a passage.
    LastBaseShort {
        metres: f32,
    },
    /// A passage runs this far from a base at its nearest.
    BaseOffCourse {
        base: usize,
        metres: f32,
    },
    BaseNearStart {
        base: usize,
        metres: f32,
    },
    BaseNearGate {
        base: usize,
        gate: usize,
        metres: f32,
    },
    /// Leg 5 is the run in from the start to gate 0.
    BaseOnFairway {
        base: usize,
        leg: usize,
        metres: f32,
    },
    /// A gate under the guns of two shore batteries at once.
    Crossfire {
        gate: usize,
    },
    Barren {
        islands: usize,
    },
    /// An island too near another island (`b` < islands) or a harbour (islands + base).
    StraitTooNarrow {
        a: usize,
        b: usize,
        metres: f32,
    },
    /// An island (`a` < islands) or a harbour (islands + base) too near the coast.
    StraitToCoast {
        a: usize,
        metres: f32,
    },
    /// A piece of ice adrift too near land, the start, a gate or its approach, a slip or
    /// a boat on patrol when the ship arrives.
    IceInTheWay {
        piece: usize,
        near: &'static str,
    },
    StartAground,
    /// A buoy, or the water either side of the gate, is on land.
    GateBlocked {
        gate: usize,
    },
    GateUnreachable {
        gate: usize,
    },
    /// Ships launched from the slip would come out onto land.
    BerthBlocked {
        base: usize,
    },
    /// The flagship would put to sea from this base onto land or into cut-off water.
    SortieStranded {
        base: usize,
    },
    BerthUnreachable {
        base: usize,
    },
    /// No water the cruiser can reach has a clear shot at the yard.
    YardHidden {
        base: usize,
    },
    FactoryHidden {
        base: usize,
        factory: usize,
    },
    NoConvoyLane {
        base: usize,
        lanes: usize,
    },
    RaiderStranded {
        raider: usize,
    },
    RaiderTooClose {
        raider: usize,
        metres: f32,
    },
    SalvageStranded {
        package: usize,
    },
    Detour {
        leg: usize,
        ratio: f32,
    },
}
impl fmt::Display for Defect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let base = |i: usize| ["A", "B", "C"][i.min(2)];
        match *self {
            Self::GatesCrowded { a, b, metres } => {
                write!(f, "gates {a} and {b} only {metres:.0} m apart")
            }
            Self::ShortLeg { leg, metres } => write!(f, "leg {leg} only {metres:.0} m long"),
            Self::UnevenLegs { ratio } => write!(f, "shortest leg {ratio:.2} of the longest"),
            Self::Hairpin { gate, degrees } => write!(f, "{degrees:.0}° turn at gate {gate}"),
            Self::LapApproach { degrees } => {
                write!(f, "next lap meets gate 0 {degrees:.0}° off its line")
            }
            Self::BasesBunched { a, b, metres } => {
                write!(
                    f,
                    "bases {} and {} only {metres:.0} m apart",
                    base(a),
                    base(b)
                )
            }
            Self::BasesOneSided { degrees } => {
                write!(f, "two bases only {degrees:.0}° apart round the circuit")
            }
            Self::BasesOutOfOrder { a, b, metres } => write!(
                f,
                "base {} only {metres:.0} m along the passage past {}",
                base(b),
                base(a)
            ),
            Self::LastBaseShort { metres } => {
                write!(f, "base C stands {metres:.0} m short of the end")
            }
            Self::BaseOffCourse { base: i, metres } => {
                write!(f, "the passage keeps {metres:.0} m from base {}", base(i))
            }
            Self::BaseNearStart { base: i, metres } => {
                write!(f, "base {} only {metres:.0} m from the start", base(i))
            }
            Self::BaseNearGate {
                base: i,
                gate,
                metres,
            } => write!(f, "base {} {metres:.0} m from gate {gate}", base(i)),
            Self::BaseOnFairway {
                base: i,
                leg,
                metres,
            } => write!(f, "base {} {metres:.0} m from leg {leg}", base(i)),
            Self::Crossfire { gate } => write!(f, "gate {gate} under two shore guns"),
            Self::Barren { islands } => write!(f, "only {islands} islands"),
            Self::StraitTooNarrow { a, b, metres } => {
                write!(f, "{metres:.0} m strait between island {a} and {b}")
            }
            Self::StraitToCoast { a, metres } => {
                write!(f, "{metres:.0} m strait between island {a} and the coast")
            }
            Self::IceInTheWay { piece, near } => write!(f, "ice {piece} adrift near {near}"),
            Self::StartAground => write!(f, "the start is on land"),
            Self::GateBlocked { gate } => write!(f, "gate {gate} is blocked"),
            Self::GateUnreachable { gate } => write!(f, "gate {gate} cannot be reached"),
            Self::BerthBlocked { base: i } => write!(f, "slip at {} runs onto land", base(i)),
            Self::BerthUnreachable { base: i } => {
                write!(f, "slip at {} is cut off", base(i))
            }
            Self::SortieStranded { base: i } => {
                write!(f, "the flagship cannot sail from {}", base(i))
            }
            Self::YardHidden { base: i } => write!(f, "no clear shot at yard {}", base(i)),
            Self::FactoryHidden { base: i, factory } => {
                write!(f, "no clear shot at factory {factory} of {}", base(i))
            }
            Self::NoConvoyLane { base: i, lanes } => {
                write!(f, "{lanes} freighter lanes into {}", base(i))
            }
            Self::RaiderStranded { raider } => write!(f, "raider {raider} is cut off"),
            Self::RaiderTooClose { raider, metres } => {
                write!(f, "raider {raider} only {metres:.0} m from the start")
            }
            Self::SalvageStranded { package } => write!(f, "package {package} is cut off"),
            Self::Detour { leg, ratio } => write!(f, "leg {leg} sails {ratio:.2}× its length"),
        }
    }
}

/// Measures of a chart the defects leave open, for choosing between charts that pass.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Metrics {
    /// Closest pair of bases, in metres.
    pub base_spacing: f32,
    /// Narrowest angle between two bases, seen from the middle of the circuit, in degrees.
    pub base_sector: f32,
    /// On a passage, the shortest run from the start to base A, A to B or B to C, over
    /// the longest.
    pub stage_balance: f32,
    /// Closest pair of factories at different bases, in metres.
    pub factory_spacing: f32,
    /// Shortest leg of the circuit over the longest.
    pub leg_balance: f32,
    /// Sharpest turn at a gate, in degrees.
    pub sharpest_turn: f32,
    /// Sailing distance over straight distance, on the leg with the most land in the way.
    pub worst_detour: f32,
    /// Share of the circuit within range of a shore gun.
    pub shore_exposure: f32,
    /// Islands within `COVER_RANGE` of a leg.
    pub cover: usize,
    /// Fewest open freighter approaches at any base.
    pub convoy_lanes: usize,
    pub islands: usize,
}
impl Metrics {
    /// From 0 to 1: how well a chart should play, judged on what the defects leave open.
    pub fn score(&self) -> f32 {
        let rise = |x: f32, low: f32, high: f32| ((x - low) / (high - low)).clamp(0., 1.);
        // A circuit spreads its bases round the middle and evens its legs. A passage
        // spaces its bases evenly along the coast, the start to A included.
        let (spread, balance) = if self.stage_balance > 0. {
            (rise(self.stage_balance, 0.35, 0.8), 1.)
        } else {
            (
                rise(self.base_sector, MIN_BASE_SECTOR_DEGREES, 110.),
                rise(self.leg_balance, MIN_LEG_BALANCE, 0.7),
            )
        };
        let terms = [
            // Factories are what the cruiser shoots at, so spread them rather than the bases.
            (3., rise(self.factory_spacing, MIN_BASE_SPACING - 45., 650.)),
            (2., spread),
            (2., balance),
            (1., 1. - rise(self.sharpest_turn, 110., HAIRPIN_DEGREES)),
            (2., 1. - rise(self.worst_detour, 1.2, MAX_DETOUR)),
            // Some of the circuit under the shore guns, but not most of it.
            (1., 1. - ((self.shore_exposure - 0.3).abs() / 0.3).min(1.)),
            (2., rise(self.cover as f32, 1., 5.)),
            (1., rise(self.convoy_lanes as f32, 1., 5.)),
        ];
        let weight: f32 = terms.iter().map(|(w, _)| w).sum();
        terms.iter().map(|(w, s)| w * s).sum::<f32>() / weight
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Survey {
    pub defects: Vec<Defect>,
    pub metrics: Metrics,
    pub score: f32,
}
impl Survey {
    pub fn passed(&self) -> bool {
        self.defects.is_empty()
    }
    /// Every check and measure, whatever fails.
    pub fn of(chart: &Chart) -> Self {
        Self::run(chart, false)
    }
    /// Stop after the layout checks if they fail. The water survey is the costly part,
    /// and the generator only needs to know that a chart is out.
    pub(crate) fn screen(chart: &Chart) -> Self {
        Self::run(chart, true)
    }
    fn run(chart: &Chart, early_out: bool) -> Self {
        let mut survey = Self {
            defects: vec![],
            metrics: Metrics::default(),
            score: 0.,
        };
        survey.layout(chart);
        if survey.passed() || !early_out {
            let battle = Battle::patrolled(9999., chart).with_flagship();
            survey.water(chart, &battle);
        }
        survey.score = if survey.passed() {
            survey.metrics.score()
        } else {
            0.
        };
        survey
    }

    fn layout(&mut self, chart: &Chart) {
        let gates = chart.gates;
        let circuit = chart.course == Course::Circuit;
        let defects = &mut self.defects;
        let m = &mut self.metrics;
        for a in 0..5 {
            for b in a + 1..5 {
                let metres = gates[a].distance(gates[b]);
                if metres < MIN_GATE_SPACING {
                    defects.push(Defect::GatesCrowded { a, b, metres });
                }
            }
            let metres = gates[a].length();
            if metres < MIN_START_TO_GATE {
                defects.push(Defect::GatesCrowded { a: 5, b: a, metres });
            }
        }
        let legs = chart.legs();
        let lengths = legs.map(|(a, b)| a.distance(b));
        for (leg, &metres) in lengths.iter().enumerate() {
            if metres < MIN_LEG {
                defects.push(Defect::ShortLeg { leg, metres });
            }
        }
        m.leg_balance = lengths.iter().copied().fold(f32::MAX, f32::min)
            / lengths.iter().copied().fold(0., f32::max).max(1.);
        if circuit && m.leg_balance < MIN_LEG_BALANCE {
            defects.push(Defect::UnevenLegs {
                ratio: m.leg_balance,
            });
        }
        let turn =
            |from: Vec2, at: Vec2, to: Vec2| (at - from).angle_to(to - at).abs().to_degrees();
        m.sharpest_turn = 0.;
        // A passage turns at each mark but the last. A circuit turns at every gate, and
        // at the first both from the start and from the last gate.
        for gate in 0..if circuit { 5 } else { 4 } {
            let next = gates[(gate + 1) % 5];
            let from = if gate == 0 {
                Vec2::ZERO
            } else {
                gates[gate - 1]
            };
            let mut sharpest = turn(from, gates[gate], next);
            if circuit && gate == 0 {
                sharpest = sharpest.max(turn(gates[4], gates[0], next));
            }
            m.sharpest_turn = m.sharpest_turn.max(sharpest);
            if sharpest > HAIRPIN_DEGREES {
                defects.push(Defect::Hairpin {
                    gate,
                    degrees: sharpest,
                });
            }
        }
        let lap = gates[0].angle_to(gates[0] - gates[4]).abs().to_degrees();
        if circuit && lap > LAP_APPROACH_DEGREES {
            defects.push(Defect::LapApproach { degrees: lap });
        }

        let bases = chart.harbours.map(|h| h.pos);
        m.base_spacing = f32::MAX;
        for a in 0..3 {
            for b in a + 1..3 {
                let metres = bases[a].distance(bases[b]);
                m.base_spacing = m.base_spacing.min(metres);
                if metres < MIN_BASE_SPACING {
                    defects.push(Defect::BasesBunched { a, b, metres });
                }
            }
        }
        if circuit {
            let middle = chart.middle();
            let mut bearings = bases.map(|p| shipvivor_compass::heading(p - middle).to_degrees());
            bearings.sort_by(f32::total_cmp);
            m.base_sector = (0..3)
                .map(|i| (bearings[(i + 1) % 3] - bearings[i]).rem_euclid(360.))
                .fold(f32::MAX, f32::min);
            if m.base_sector < MIN_BASE_SECTOR_DEGREES {
                defects.push(Defect::BasesOneSided {
                    degrees: m.base_sector,
                });
            }
        } else {
            // The bases come in order along the passage, the heavy yard at its end, and
            // the course runs past each within reach of its guns.
            let along = bases.map(|p| course_run(&legs, p));
            let end: f32 = lengths.iter().sum();
            let stages = [along[0].0, along[1].0 - along[0].0, along[2].0 - along[1].0];
            for (a, b) in [(0, 1), (1, 2)] {
                let metres = along[b].0 - along[a].0;
                if metres < MIN_STAGE {
                    defects.push(Defect::BasesOutOfOrder { a, b, metres });
                }
            }
            if end - along[2].0 > MAX_SHORT_OF_END {
                defects.push(Defect::LastBaseShort {
                    metres: end - along[2].0,
                });
            }
            for (base, &(_, metres)) in along.iter().enumerate() {
                if metres > MAX_BASE_TO_COURSE {
                    defects.push(Defect::BaseOffCourse { base, metres });
                }
            }
            m.stage_balance = stages.iter().copied().fold(f32::MAX, f32::min).max(0.)
                / stages.iter().copied().fold(0., f32::max).max(1.);
        }
        let mut fairways = legs.to_vec();
        fairways.push((Vec2::ZERO, gates[0]));
        for (base, &pos) in bases.iter().enumerate() {
            let metres = pos.length();
            if metres < MIN_BASE_TO_START {
                defects.push(Defect::BaseNearStart { base, metres });
            }
            for (gate, g) in gates.iter().enumerate() {
                let metres = pos.distance(*g);
                if metres < MIN_BASE_TO_GATE {
                    defects.push(Defect::BaseNearGate { base, gate, metres });
                }
            }
            for (leg, &(a, b)) in fairways.iter().enumerate() {
                let metres = segment_distance(pos, a, b);
                if metres < MIN_BASE_TO_LEG {
                    defects.push(Defect::BaseOnFairway { base, leg, metres });
                }
            }
        }
        let ports = Port::chart(&chart.harbours, 9999., &mut 0);
        m.factory_spacing = f32::MAX;
        for a in 0..3 {
            for b in a + 1..3 {
                for i in 0..ports[a].factories().len() {
                    for j in 0..ports[b].factories().len() {
                        let metres = ports[a]
                            .factory_position(i)
                            .distance(ports[b].factory_position(j));
                        m.factory_spacing = m.factory_spacing.min(metres);
                    }
                }
            }
        }
        // How many bases have a shore gun bearing on a point.
        let covering = |point: Vec2| {
            ports
                .iter()
                .filter(|port| port.turrets().iter().any(|g| g.covers(&port.body, point)))
                .count()
        };
        for (gate, &g) in gates.iter().enumerate() {
            if covering(g) >= 2 {
                defects.push(Defect::Crossfire { gate });
            }
        }
        let samples: Vec<Vec2> = legs
            .iter()
            .flat_map(|&(a, b)| {
                let steps = (a.distance(b) / 20.).ceil().max(1.) as usize;
                (0..steps).map(move |i| a.lerp(b, i as f32 / steps as f32))
            })
            .collect();
        m.shore_exposure = samples.iter().filter(|&&p| covering(p) > 0).count() as f32
            / samples.len().max(1) as f32;

        let islets = &chart.islets;
        m.islands = islets.len();
        if islets.len() < MIN_ISLANDS {
            defects.push(Defect::Barren {
                islands: islets.len(),
            });
        }
        // Water left between an island and the coast, or the coast's overlap as negative.
        let to_coast = |pos: Vec2, radius: f32| {
            chart
                .coast
                .iter()
                .map(|piece| pos.distance(piece.pos) - piece.radius - radius)
                .fold(f32::MAX, f32::min)
        };
        for (a, i) in islets.iter().enumerate() {
            for (b, j) in islets.iter().enumerate().skip(a + 1) {
                let metres = i.pos.distance(j.pos) - i.radius - j.radius;
                if metres < MIN_STRAIT {
                    defects.push(Defect::StraitTooNarrow { a, b, metres });
                }
            }
            for (base, port) in ports.iter().enumerate() {
                let pier_end = port.berth() - port.body.forward().perp() * 8.;
                let metres = (i.pos.distance(port.body.pos) - ISLAND_RADIUS)
                    .min(i.pos.distance(pier_end) - 3.)
                    - i.radius;
                if metres < MIN_STRAIT {
                    defects.push(Defect::StraitTooNarrow {
                        a,
                        b: islets.len() + base,
                        metres,
                    });
                }
            }
            let metres = to_coast(i.pos, i.radius);
            if metres < MIN_STRAIT {
                defects.push(Defect::StraitToCoast { a, metres });
            }
        }
        for (base, port) in ports.iter().enumerate() {
            let metres = to_coast(port.body.pos, ISLAND_RADIUS);
            if metres < MIN_STRAIT {
                defects.push(Defect::StraitToCoast {
                    a: islets.len() + base,
                    metres,
                });
            }
        }
        m.cover = islets
            .iter()
            .filter(|i| {
                legs.iter()
                    .any(|&(a, b)| segment_distance(i.pos, a, b) - i.radius <= COVER_RANGE)
            })
            .count();
        let keepout = IceKeepout::new(
            &gates,
            &chart.harbours,
            islets,
            &chart.coast,
            &chart.pickets,
        );
        for (piece, ice) in chart.ice.iter().enumerate() {
            if let Some(near) = keepout.clear(ice) {
                defects.push(Defect::IceInTheWay { piece, near });
            }
        }
    }

    fn water(&mut self, chart: &Chart, battle: &Battle) {
        let gates = chart.gates;
        // The grid covers the water: the coast's hinterland is all land.
        let mut extent: Vec<Vec2> = vec![Vec2::ZERO];
        extent.extend(gates);
        extent.extend(chart.harbours.iter().map(|h| h.pos));
        extent.extend(chart.islets.iter().map(|i| i.pos));
        extent.extend(battle.raiders().iter().map(|r| r.body.pos));
        let water = Water::chart(battle, &extent, SEA_ROOM);
        let Some(start) = water.cell(Vec2::ZERO).filter(|&c| water.open[c]) else {
            self.defects.push(Defect::StartAground);
            return;
        };
        let afloat_from_start = water.flood(start);
        let reached = |p: Vec2, within: f32| water.reached(&afloat_from_start, p, within);
        let afloat = |p: Vec2, room: f32| battle.navigable_position(p, room).distance(p) < 1e-3;
        let defects = &mut self.defects;

        let mut previous = Vec2::ZERO;
        for (gate, &g) in gates.iter().enumerate() {
            let dir = (g - previous).normalize_or_zero();
            previous = g;
            let across = dir.perp() * GATE_HALF_WIDTH;
            let lane = (-GATE_LANE as i32..=GATE_LANE as i32)
                .step_by(4)
                .all(|t| afloat(g + dir * t as f32, SEA_ROOM));
            if !(lane && afloat(g + across, 4.) && afloat(g - across, 4.)) {
                defects.push(Defect::GateBlocked { gate });
            } else if !reached(g - dir * GATE_LANE, CELL * 1.5) {
                defects.push(Defect::GateUnreachable { gate });
            }
        }

        let ports = battle.ports();
        for (base, port) in ports.iter().enumerate() {
            let (berth, f) = (port.berth(), port.body.forward());
            if !(12..=90)
                .step_by(4)
                .all(|t| afloat(berth + f * t as f32, 7.))
            {
                defects.push(Defect::BerthBlocked { base });
            } else if !reached(berth + f * 90., CELL * 1.5) {
                defects.push(Defect::BerthUnreachable { base });
            }
            // On the second leg the flagship may sail from any working yard.
            let hull = super::EnemyClass::Parliament.profile().hull;
            let sortie = battle.sortie_station(port);
            if !(afloat(sortie, hull.radius) && reached(sortie, hull.bow + CELL * 2.)) {
                defects.push(Defect::SortieStranded { base });
            }
            let sighted = |target: Vec2| {
                water.any_cell_near(target, FIRING_RANGE, 2, |cell, at| {
                    at.distance(target) >= 60.
                        && afloat_from_start[cell]
                        && clear_shot(at, target, base, battle)
                })
            };
            if !sighted(port.yard_position()) {
                defects.push(Defect::YardHidden { base });
            }
            for factory in 0..port.factories().len() {
                if !sighted(port.factory_position(factory)) {
                    defects.push(Defect::FactoryHidden { base, factory });
                }
            }
            let lanes = battle.convoy_lanes(base).count();
            self.metrics.convoy_lanes = if base == 0 {
                lanes
            } else {
                self.metrics.convoy_lanes.min(lanes)
            };
            if lanes < MIN_CONVOY_LANES {
                defects.push(Defect::NoConvoyLane { base, lanes });
            }
        }

        for (raider, ship) in battle.raiders().iter().enumerate() {
            let pos = ship.body.pos;
            if pos.length() < SPAWN_STANDOFF {
                defects.push(Defect::RaiderTooClose {
                    raider,
                    metres: pos.length(),
                });
            }
            if !reached(pos, ship.class.profile().hull.bow + CELL * 2.) {
                defects.push(Defect::RaiderStranded { raider });
            }
        }
        for (package, stores) in battle.salvage().iter().enumerate() {
            if !reached(stores.body.pos, 40.) {
                defects.push(Defect::SalvageStranded { package });
            }
        }

        // Each leg, sailed on the grid, against the same leg across open water. A
        // circuit's run in from the start comes first, as leg 5.
        let mut worst: f32 = 1.;
        let mut legs = vec![];
        if chart.course == Course::Circuit {
            legs.push((Vec2::ZERO, gates[0]));
        }
        legs.extend(chart.legs());
        let first = if chart.course == Course::Circuit {
            5
        } else {
            0
        };
        for (index, &(a, b)) in legs.iter().enumerate() {
            let (Some(from), Some(to)) = (water.cell(a), water.cell(b)) else {
                continue;
            };
            if !water.open[from] || !water.open[to] {
                continue;
            }
            let Some(sailed) = water.path(from, to) else {
                continue;
            };
            let ratio = sailed / water.octile(from, to).max(CELL);
            worst = worst.max(ratio);
            if ratio > MAX_DETOUR {
                defects.push(Defect::Detour {
                    leg: (index + first) % 6,
                    ratio,
                });
            }
        }
        self.metrics.worst_detour = worst;
    }
}

#[cfg(test)]
mod tests;

/// How far along a course of legs the point nearest `p` lies, and how far off it `p` is.
fn course_run(legs: &[(Vec2, Vec2); 5], p: Vec2) -> (f32, f32) {
    let mut run = 0.;
    let mut best = (0., f32::MAX);
    for &(a, b) in legs {
        let ab = b - a;
        let u = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0., 1.);
        let off = p.distance(a + ab * u);
        if off < best.1 {
            best = (run + ab.length() * u, off);
        }
        run += ab.length();
    }
    best
}

/// A flat shot from `from` to `to` that clears every island and every other harbour.
/// The target's own island is under the shell's fall, so it does not count.
fn clear_shot(from: Vec2, to: Vec2, own: usize, battle: &Battle) -> bool {
    battle
        .landmarks()
        .iter()
        .all(|i| entry(from, to, i.body.pos, i.radius).is_none())
        && battle
            .ports()
            .iter()
            .enumerate()
            .all(|(i, p)| i == own || entry(from, to, p.body.pos, ISLAND_RADIUS).is_none())
}

/// Open water on a square grid, with `room` kept from every shoal and pier.
struct Water {
    origin: Vec2,
    cols: usize,
    rows: usize,
    open: Vec<bool>,
}
impl Water {
    fn chart(battle: &Battle, extent: &[Vec2], room: f32) -> Self {
        let margin = Vec2::splat(180.);
        let low = extent.iter().copied().fold(Vec2::MAX, Vec2::min) - margin;
        let high = extent.iter().copied().fold(Vec2::MIN, Vec2::max) + margin;
        let cols = ((high.x - low.x) / CELL).ceil() as usize + 1;
        let rows = ((high.y - low.y) / CELL).ceil() as usize + 1;
        let mut water = Self {
            origin: low,
            cols,
            rows,
            open: vec![true; cols * rows],
        };
        // A ship keeps `room` from every island's round shore. Only the harbours, with
        // their piers, need the exact test the ships use.
        for island in battle.landmarks() {
            water.each_cell_near(
                island.body.pos,
                island.radius + room,
                1,
                |water, cell, _| {
                    water.open[cell] = false;
                },
            );
        }
        let shoals = battle
            .ports()
            .iter()
            .map(|p| (p.body.pos, ISLAND_RADIUS + 50. + room));
        for (centre, reach) in shoals {
            water.each_cell_near(centre, reach, 1, |water, cell, at| {
                if battle.navigable_position(at, room).distance(at) > 1e-3 {
                    water.open[cell] = false;
                }
            });
        }
        water
    }
    fn cell(&self, p: Vec2) -> Option<usize> {
        let q = ((p - self.origin) / CELL).round();
        (q.x >= 0. && q.y >= 0. && (q.x as usize) < self.cols && (q.y as usize) < self.rows)
            .then(|| q.y as usize * self.cols + q.x as usize)
    }
    fn centre(&self, cell: usize) -> Vec2 {
        self.origin + Vec2::new((cell % self.cols) as f32, (cell / self.cols) as f32) * CELL
    }
    fn bounds(&self, centre: Vec2, reach: f32) -> (usize, usize, usize, usize) {
        let low = ((centre - reach - self.origin) / CELL)
            .floor()
            .max(Vec2::ZERO);
        let high = ((centre + reach - self.origin) / CELL).ceil();
        (
            low.x as usize,
            (high.x as usize).min(self.cols - 1),
            low.y as usize,
            (high.y as usize).min(self.rows - 1),
        )
    }
    fn each_cell_near(
        &mut self,
        centre: Vec2,
        reach: f32,
        stride: usize,
        mut visit: impl FnMut(&mut Self, usize, Vec2),
    ) {
        let (x0, x1, y0, y1) = self.bounds(centre, reach);
        for y in (y0..=y1).step_by(stride) {
            for x in (x0..=x1).step_by(stride) {
                let cell = y * self.cols + x;
                let at = self.centre(cell);
                if at.distance(centre) <= reach {
                    visit(self, cell, at);
                }
            }
        }
    }
    fn any_cell_near(
        &self,
        centre: Vec2,
        reach: f32,
        stride: usize,
        test: impl Fn(usize, Vec2) -> bool,
    ) -> bool {
        let (x0, x1, y0, y1) = self.bounds(centre, reach);
        (y0..=y1).step_by(stride).any(|y| {
            (x0..=x1).step_by(stride).any(|x| {
                let cell = y * self.cols + x;
                let at = self.centre(cell);
                self.open[cell] && at.distance(centre) <= reach && test(cell, at)
            })
        })
    }
    /// Some open cell within `within` of `p` that the start can reach.
    fn reached(&self, from_start: &[bool], p: Vec2, within: f32) -> bool {
        self.any_cell_near(p, within, 1, |cell, _| from_start[cell])
    }
    /// Every open cell connected to `from`, by the same moves as `path`.
    fn flood(&self, from: usize) -> Vec<bool> {
        let mut seen = vec![false; self.open.len()];
        let mut stack = vec![from];
        seen[from] = true;
        while let Some(cell) = stack.pop() {
            for next in self.moves(cell).map(|(next, _)| next) {
                if !seen[next] {
                    seen[next] = true;
                    stack.push(next);
                }
            }
        }
        seen
    }
    /// The open cells next to `cell` and the distance to each. A diagonal step needs both
    /// of the cells beside it open, so no path cuts the corner of a shoal.
    fn moves(&self, cell: usize) -> impl Iterator<Item = (usize, f32)> + '_ {
        let cols = self.cols as isize;
        let (x, y) = ((cell % self.cols) as isize, (cell / self.cols) as isize);
        let open = move |x: isize, y: isize| {
            (x >= 0 && y >= 0 && x < cols && y < self.rows as isize)
                .then(|| (y * cols + x) as usize)
                .filter(|&c| self.open[c])
        };
        [
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
            (1, 1),
            (1, -1),
            (-1, 1),
            (-1, -1),
        ]
        .into_iter()
        .filter_map(move |(dx, dy)| {
            let next = open(x + dx, y + dy)?;
            if dx != 0 && dy != 0 {
                open(x + dx, y)?;
                open(x, y + dy)?;
                Some((next, CELL * std::f32::consts::SQRT_2))
            } else {
                Some((next, CELL))
            }
        })
    }
    /// Straight-line cost between two cells on an empty grid, for comparing paths.
    fn octile(&self, a: usize, b: usize) -> f32 {
        let dx = (a % self.cols).abs_diff(b % self.cols) as f32;
        let dy = (a / self.cols).abs_diff(b / self.cols) as f32;
        (dx.max(dy) + (std::f32::consts::SQRT_2 - 1.) * dx.min(dy)) * CELL
    }
    /// Sailing distance from one open cell to another, by A* over open cells.
    fn path(&self, from: usize, to: usize) -> Option<f32> {
        let mut dist = vec![f32::INFINITY; self.open.len()];
        let mut heap = BinaryHeap::new();
        let key = |cell: usize, d: f32| ((d + self.octile(cell, to)) * 16.) as u32;
        dist[from] = 0.;
        heap.push(Reverse((key(from, 0.), from)));
        while let Some(Reverse((queued, cell))) = heap.pop() {
            let here = dist[cell];
            if cell == to {
                return Some(here);
            }
            // A cell is queued again each time a shorter way to it turns up.
            if queued > key(cell, here) {
                continue;
            }
            for (next, step) in self.moves(cell) {
                if here + step < dist[next] {
                    dist[next] = here + step;
                    heap.push(Reverse((key(next, here + step), next)));
                }
            }
        }
        None
    }
}
