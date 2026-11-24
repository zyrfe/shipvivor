//! Seeded sector charts: the buoy circuit, three harbours and the islands among them.
//!
//! `Chart::generate` sketches candidates from a seed and the character of a stage's
//! waters, surveys each one (see `survey`), and keeps the best of the first few that
//! pass. Every run therefore sails different waters, and none of them is unwinnable.
use super::{ISLAND_RADIUS, IceShape, Landmark, LandmarkKind, PICKETS, Port, Set, Survey, Vec2};
use shipvivor_compass::{direction, heading};
use std::f32::consts::TAU;

/// Narrowest water the generator leaves between two islands. The flagship passes with
/// room to spare, and each island stays a lone obstacle the raiders can steer round.
pub const STRAIT: f32 = 40.;
/// Candidates sketched before the generator gives up on a seed.
const ATTEMPTS: usize = 240;
/// Charts that pass the survey before the best of them is taken.
const SHORTLIST: usize = 3;

/// What a stage's waters are like. The seed decides the particulars.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Waters {
    /// Typical distance in metres from the middle of the buoy circuit to a gate.
    pub reach: f32,
    /// How many islands to raise, at least and at most. Crowded water can leave fewer.
    pub islands: (usize, usize),
    /// Smallest and largest island radius in metres.
    pub island_radius: (f32, f32),
    /// Share of islands raised in groups rather than alone.
    pub clustering: f32,
    /// Pairs of islands to try either side of a leg, so the course runs through a strait.
    pub narrows: usize,
    /// Islands to try on a broken ring round the circuit, so the water feels enclosed.
    pub rim: usize,
    /// Open water kept either side of each leg, in metres, except at narrows.
    pub fairway: f32,
    /// Share of lone islands allowed to stand in a fairway, so the leg is sailed round it.
    pub strays: f32,
    /// Relative odds of beacon, stack and pine islands.
    pub kinds: [f32; 3],
    /// Large islands under ice to try, at least and at most, and their smallest and
    /// largest radius in metres.
    pub glaciers: (usize, usize),
    pub glacier_radius: (f32, f32),
    /// Speed of the open-water current, least and most, in metres per second.
    pub drift: (f32, f32),
    /// Pieces of ice adrift when the ship arrives, at least and at most.
    pub ice: (usize, usize),
    /// Share of the ice that is bergs rather than growlers.
    pub bergs: f32,
}
impl Waters {
    /// Islands in pairs either side of the course, and a few groups between the legs.
    pub const NARROWS: Self = Self {
        reach: 480.,
        islands: (7, 10),
        island_radius: (20., 44.),
        clustering: 0.35,
        narrows: 2,
        rim: 0,
        fairway: 60.,
        strays: 0.12,
        kinds: [1., 1., 1.],
        glaciers: (1, 1),
        glacier_radius: (100., 160.),
        drift: (0.35, 0.6),
        ice: (18, 26),
        bergs: 0.35,
    };
    /// A wide roadstead with a few rocks and beacons scattered across it.
    pub const OPEN: Self = Self {
        reach: 600.,
        islands: (5, 8),
        island_radius: (18., 40.),
        clustering: 0.2,
        narrows: 0,
        rim: 0,
        fairway: 90.,
        strays: 0.25,
        kinds: [1.5, 0.8, 0.7],
        glaciers: (0, 1),
        glacier_radius: (120., 190.),
        drift: (0.25, 0.45),
        ice: (16, 24),
        bergs: 0.5,
    };
    /// Sheltered water inside a broken ring of wooded islands.
    pub const SOUND: Self = Self {
        reach: 470.,
        islands: (10, 14),
        island_radius: (22., 48.),
        clustering: 0.45,
        narrows: 1,
        rim: 7,
        fairway: 55.,
        strays: 0.1,
        kinds: [0.6, 0.8, 1.6],
        glaciers: (1, 2),
        glacier_radius: (90., 150.),
        drift: (0.2, 0.35),
        ice: (26, 36),
        bergs: 0.2,
    };
    /// A tight course threaded between crowded rocks.
    pub const CUT: Self = Self {
        reach: 400.,
        islands: (9, 13),
        island_radius: (18., 40.),
        clustering: 0.5,
        narrows: 3,
        rim: 0,
        fairway: 40.,
        strays: 0.18,
        kinds: [0.8, 1.6, 0.6],
        glaciers: (1, 2),
        glacier_radius: (80., 130.),
        drift: (0.5, 0.8),
        ice: (16, 24),
        bergs: 0.3,
    };
}

/// Where a harbour stands and the way its slip faces.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HarbourSite {
    pub pos: Vec2,
    pub heading: f32,
}

/// A piece of ice adrift when the ship enters the sector.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ice {
    pub pos: Vec2,
    pub radius: f32,
    pub shape: IceShape,
    pub heading: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Islet {
    pub pos: Vec2,
    pub radius: f32,
    pub kind: LandmarkKind,
    /// Turns the island's features. It does not change its outline.
    pub heading: f32,
}

/// The fixed geography of one sector. The ship enters at the origin.
#[derive(Clone, Debug, PartialEq)]
pub struct Chart {
    /// The seed the chart was drawn from, or `None` for a classic hand-set chart.
    pub seed: Option<u64>,
    pub gates: [Vec2; 5],
    /// Bases A, B and C, with one, two and three factories.
    pub harbours: [HarbourSite; 3],
    pub islets: Vec<Islet>,
    /// The current far from land. The islands shape it into the stream sailed.
    pub set: Set,
    /// Ice adrift on the current when the ship arrives.
    pub ice: Vec<Ice>,
}
impl Chart {
    /// The hand-set layout around a buoy circuit: a harbour outside gates 1, 2 and 4,
    /// and an island either side of each leg where there is room.
    pub fn classic(gates: [Vec2; 5]) -> Self {
        let harbours = Port::classic_sites(gates);
        let islets = Landmark::classic(gates, &harbours);
        Self {
            seed: None,
            gates,
            harbours,
            islets,
            set: Set::SLACK,
            ice: vec![],
        }
    }

    /// Sketch candidates from the seed, survey each, and keep the best of the first few
    /// that pass. The same seed and waters always give the same chart. `None` means no
    /// candidate passed, which the tests rule out for the waters the game sails.
    pub fn generate(seed: u64, waters: &Waters) -> Option<(Self, Survey)> {
        let mut rng = Rng(seed);
        let mut best: Option<(Self, Survey)> = None;
        let mut passed = 0;
        for _ in 0..ATTEMPTS {
            let chart = Self::sketch(&mut rng, seed, waters);
            let survey = Survey::screen(&chart);
            if !survey.passed() {
                continue;
            }
            passed += 1;
            if best.as_ref().is_none_or(|(_, b)| survey.score > b.score) {
                best = Some((chart, survey));
            }
            if passed == SHORTLIST {
                break;
            }
        }
        best
    }

    /// One unsurveyed candidate.
    pub fn sketch_one(seed: u64, waters: &Waters) -> Self {
        Self::sketch(&mut Rng(seed), seed, waters)
    }

    fn sketch(rng: &mut Rng, seed: u64, waters: &Waters) -> Self {
        let reach = waters.reach * rng.range(0.88, 1.12);
        let gates = circuit(rng, reach);
        let harbours = harbours(rng, &gates);
        let islets = islands(rng, waters, &gates, &harbours);
        let set = stream(rng, waters);
        let ice = ice(rng, waters, &gates, &harbours, &islets);
        Self {
            seed: Some(seed),
            gates,
            harbours,
            islets,
            set,
            ice,
        }
    }

    /// The middle of the buoy circuit.
    pub fn middle(&self) -> Vec2 {
        self.gates.iter().sum::<Vec2>() / self.gates.len() as f32
    }
    /// Each leg of the circuit, from gate `k` to the next. The last closes the lap.
    pub fn legs(&self) -> [(Vec2, Vec2); 5] {
        legs(&self.gates)
    }
    /// Corners of the water the chart sets out: the start, gates, harbours and land.
    pub fn extent(&self) -> (Vec2, Vec2) {
        let points = std::iter::once((Vec2::ZERO, 0.))
            .chain(self.gates.iter().map(|&g| (g, 0.)))
            .chain(self.harbours.iter().map(|h| (h.pos, ISLAND_RADIUS)))
            .chain(self.islets.iter().map(|i| (i.pos, i.radius)));
        points.fold((Vec2::MAX, Vec2::MIN), |(lo, hi), (p, r)| {
            (lo.min(p - r), hi.max(p + r))
        })
    }
    /// The land the current runs round: harbours and islands, as centre and radius.
    pub fn land(&self) -> Vec<(Vec2, f32)> {
        self.harbours
            .iter()
            .map(|h| (h.pos, ISLAND_RADIUS))
            .chain(self.islets.iter().map(|i| (i.pos, i.radius)))
            .collect()
    }
}

fn legs(gates: &[Vec2; 5]) -> [(Vec2, Vec2); 5] {
    std::array::from_fn(|k| (gates[k], gates[(k + 1) % 5]))
}

/// Five gates. The ship enters on the closing leg, so she meets the first gate head on
/// in every lap, and the other three sweep round to one side.
fn circuit(rng: &mut Rng, reach: f32) -> [Vec2; 5] {
    let ahead = direction(rng.range(0., TAU));
    // Positive turns to starboard: clockwise on the chart, as headings run.
    let turn = rng.sign();
    let inside = ahead.perp() * turn;
    let first = ahead * rng.range(230., 320.);
    let last = -ahead * rng.range(150., 330.) + inside * rng.range(-60., 160.);
    let middle = first.midpoint(last)
        + inside * reach * rng.range(0.6, 1.)
        + ahead * reach * rng.range(-0.3, 0.3);
    let from = heading(first - middle);
    // The long way round from the first gate to the last, in the direction of the turn.
    let span = ((heading(last - middle) - from) * turn).rem_euclid(TAU);
    let weights: [f32; 4] = std::array::from_fn(|_| rng.range(0.6, 1.4));
    let total: f32 = weights.iter().sum();
    let mut gates = [first, Vec2::ZERO, Vec2::ZERO, Vec2::ZERO, last];
    let mut swept = 0.;
    for k in 1..4 {
        swept += weights[k - 1] / total * span;
        gates[k] = middle + direction(from + turn * swept) * reach * rng.range(0.72, 1.28);
    }
    if rng.chance(0.3) {
        // A dent: one gate drawn in toward the middle, so the circuit doubles back.
        let k = 1 + rng.below(3);
        gates[k] = middle.lerp(gates[k], rng.range(0.45, 0.62));
    }
    if rng.chance(0.35) {
        // A stretch along some axis, from a round sound to a long reach.
        let axis = direction(rng.range(0., TAU));
        let stretch = rng.range(0.2, 0.55);
        for gate in &mut gates[1..4] {
            let offset = *gate - middle;
            *gate += axis * offset.dot(axis) * stretch;
        }
    }
    if rng.chance(0.15) {
        // A crossover: the middle gates swap, so the course crosses its own track.
        gates.swap(2, 3);
    }
    gates
}

/// Three harbours beside different legs, mostly outside the circuit, each facing its
/// fairway. Which base stands where is shuffled too. A site plainly too near the start,
/// a gate, a fairway or another base is drawn again; the survey judges the rest.
fn harbours(rng: &mut Rng, gates: &[Vec2; 5]) -> [HarbourSite; 3] {
    let middle = gates.iter().sum::<Vec2>() / 5.;
    let legs = legs(gates);
    let mut fairways = legs.to_vec();
    fairways.push((Vec2::ZERO, gates[0]));
    let mut used = [false; 5];
    let mut sites: Vec<HarbourSite> = Vec::with_capacity(3);
    while sites.len() < 3 {
        let mut site = None;
        for _ in 0..16 {
            let leg = rng.below(5);
            if used[leg] {
                continue;
            }
            let (a, b) = legs[leg];
            let beside = a.lerp(b, rng.range(0.2, 0.8));
            let normal = (b - a).normalize_or_zero().perp();
            let outward = if normal.dot(beside - middle) >= 0. {
                normal
            } else {
                -normal
            };
            let side = if rng.chance(0.75) { outward } else { -outward };
            let pos = beside + side * rng.range(115., 190.);
            site = Some((
                leg,
                HarbourSite {
                    pos,
                    heading: heading(beside - pos) + rng.range(-0.35, 0.35),
                },
            ));
            let bearing = heading(pos - middle);
            if pos.length() >= 430.
                && gates.iter().all(|g| g.distance(pos) >= 120.)
                && fairways
                    .iter()
                    .all(|&(a, b)| segment_distance(pos, a, b) >= 100.)
                && sites.iter().all(|s| {
                    s.pos.distance(pos) >= 400.
                        && ((heading(s.pos - middle) - bearing + TAU / 2.).rem_euclid(TAU)
                            - TAU / 2.)
                            .abs()
                            >= 60_f32.to_radians()
                })
            {
                break;
            }
        }
        // Sixteen draws can all land on used legs. Take the first unused one then.
        let (leg, site) = site.unwrap_or_else(|| {
            let leg = used.iter().position(|u| !u).unwrap_or(0);
            let (a, b) = legs[leg];
            let beside = a.midpoint(b);
            let pos = beside + (beside - middle).normalize_or_zero() * 150.;
            (
                leg,
                HarbourSite {
                    pos,
                    heading: heading(beside - pos),
                },
            )
        });
        used[leg] = true;
        sites.push(site);
    }
    [sites[0], sites[1], sites[2]]
}

/// Water the islands must leave open: the start, the gates and their approaches, the
/// stations of the ships already at sea, the harbours and their slips, where the flagship
/// also sails from, and the fairways.
struct Keepout {
    gate_lanes: [(Vec2, Vec2); 5],
    gates: [Vec2; 5],
    fairways: Vec<(Vec2, Vec2)>,
    stations: Vec<(Vec2, f32)>,
    harbours: Vec<(Vec2, [Vec2; 3])>,
    fairway: f32,
}
impl Keepout {
    fn new(waters: &Waters, gates: &[Vec2; 5], harbours: &[HarbourSite; 3]) -> Self {
        let mut previous = Vec2::ZERO;
        let gate_lanes = gates.map(|g| {
            let dir = (g - previous).normalize_or_zero();
            previous = g;
            (g - dir * 70., g + dir * 70.)
        });
        let mut fairways = legs(gates).to_vec();
        fairways.push((Vec2::ZERO, gates[0]));
        let ahead = gates[0].normalize_or_zero();
        let mut stations: Vec<(Vec2, f32)> = PICKETS
            .iter()
            .map(|&(range, lateral)| (ahead * range + ahead.perp() * lateral, 45.))
            .collect();
        stations.push((Vec2::ZERO, 70.));
        let ports = Port::chart(harbours, 1., &mut 0);
        Self {
            gate_lanes,
            gates: *gates,
            fairways,
            stations,
            harbours: ports
                .iter()
                .map(|p| {
                    let (berth, f) = (p.berth(), p.body.forward());
                    (p.body.pos, [berth, berth + f * 45., berth + f * 90.])
                })
                .collect(),
            fairway: waters.fairway,
        }
    }
    fn allows(&self, islet: &Islet, placed: &[Islet], in_fairway: bool) -> bool {
        let (p, r) = (islet.pos, islet.radius);
        placed
            .iter()
            .all(|o| o.pos.distance(p) >= r + o.radius + STRAIT)
            && self.gates.iter().all(|g| g.distance(p) >= r + 60.)
            && self
                .gate_lanes
                .iter()
                .all(|&(a, b)| segment_distance(p, a, b) >= r + 26.)
            && self
                .stations
                .iter()
                .all(|&(s, room)| s.distance(p) >= r + room)
            && self.harbours.iter().all(|(centre, apron)| {
                centre.distance(p) >= r + 105. && apron.iter().all(|a| a.distance(p) >= r + 40.)
            })
            && (in_fairway
                || self
                    .fairways
                    .iter()
                    .all(|&(a, b)| segment_distance(p, a, b) >= r + self.fairway))
    }
}

fn islands(
    rng: &mut Rng,
    waters: &Waters,
    gates: &[Vec2; 5],
    harbours: &[HarbourSite; 3],
) -> Vec<Islet> {
    let keepout = Keepout::new(waters, gates, harbours);
    let (fewest, most) = waters.islands;
    let target = fewest + rng.below(most.saturating_sub(fewest) + 1);
    let mut placed: Vec<Islet> = vec![];
    let raise = |rng: &mut Rng, pos: Vec2| {
        let kind = rng.kind(waters.kinds);
        let (small, large) = waters.island_radius;
        let size = match kind {
            LandmarkKind::Beacon => rng.range(0., 0.45),
            LandmarkKind::Stacks => rng.range(0.15, 0.85),
            LandmarkKind::Pines | LandmarkKind::Glacier => rng.range(0.3, 1.),
        };
        Islet {
            pos,
            radius: small + (large - small) * size,
            kind,
            heading: rng.range(0., TAU),
        }
    };
    let middle = gates.iter().sum::<Vec2>() / 5.;
    let span = gates
        .iter()
        .chain(harbours.iter().map(|h| &h.pos))
        .map(|p| p.distance(middle))
        .fold(0., f32::max);
    // Glaciers first, while there is room: big islands under an ice cap. The circuit
    // often rounds one, and the current runs hard past their headlands.
    let (fewest_glaciers, most_glaciers) = waters.glaciers;
    let glaciers = if most_glaciers == 0 {
        0
    } else {
        fewest_glaciers + rng.below(most_glaciers.saturating_sub(fewest_glaciers) + 1)
    };
    let (smallest, largest) = waters.glacier_radius;
    for _ in 0..glaciers {
        // Try sites in the middle of the circuit and round it, and raise the glacier where
        // the most of its intended size fits.
        let wanted = rng.range(smallest, largest);
        let heading = rng.range(0., TAU);
        let mut best: Option<Islet> = None;
        for attempt in 0..24 {
            let at = if attempt < 4 {
                middle + direction(rng.range(0., TAU)) * rng.range(0., 80.)
            } else {
                middle + direction(rng.range(0., TAU)) * span * rng.range(0.35, 1.35)
            };
            let mut radius = wanted;
            while radius >= smallest * 0.8 && best.is_none_or(|b| radius > b.radius) {
                let islet = Islet {
                    pos: at,
                    radius,
                    kind: LandmarkKind::Glacier,
                    heading,
                };
                if keepout.allows(&islet, &placed, false) {
                    best = Some(islet);
                    break;
                }
                radius *= 0.93;
            }
        }
        placed.extend(best);
    }
    // Narrows: a strait the course runs through, an island either side of a leg.
    for _ in 0..waters.narrows {
        for _ in 0..6 {
            let (a, b) = legs(gates)[rng.below(5)];
            if a.distance(b) < 300. {
                continue;
            }
            let at = a.lerp(b, rng.range(0.3, 0.7));
            let normal = (b - a).normalize_or_zero().perp();
            let half = rng.range(32., 48.);
            let pair = [-1., 1.].map(|side| {
                let mut islet = raise(rng, at);
                islet.pos += normal * side * (half + islet.radius);
                islet
            });
            if pair.iter().all(|i| keepout.allows(i, &placed, true)) {
                placed.extend(pair);
                break;
            }
        }
    }
    // The rim stands beyond the farthest gate or harbour, with gaps to sail out through.
    if waters.rim > 0 {
        let ring = span + rng.range(110., 190.);
        let phase = rng.range(0., TAU);
        for j in 0..waters.rim {
            let angle = phase + TAU * (j as f32 + rng.range(-0.25, 0.25)) / waters.rim as f32;
            let at = middle + direction(angle) * ring * rng.range(0.92, 1.08);
            let islet = raise(rng, at);
            if keepout.allows(&islet, &placed, false) {
                placed.push(islet);
            }
        }
    }
    // Groups and lone islands fill the rest of the water round the circuit.
    let mut tries = 0;
    while placed.len() < target && tries < 400 {
        tries += 1;
        let around = middle + direction(rng.range(0., TAU)) * (span + 120.) * rng.unit().sqrt();
        if rng.chance(waters.clustering) {
            for _ in 0..2 + rng.below(3) {
                let at = around + direction(rng.range(0., TAU)) * rng.range(0., 130.);
                let islet = raise(rng, at);
                if placed.len() < target && keepout.allows(&islet, &placed, false) {
                    placed.push(islet);
                }
            }
        } else {
            let stray = rng.chance(waters.strays);
            let islet = raise(rng, around);
            if keepout.allows(&islet, &placed, stray) {
                placed.push(islet);
            }
        }
    }
    placed
}

/// The set of the current: any direction, at the speed the waters run, swinging gently
/// from side to side along its length.
fn stream(rng: &mut Rng, waters: &Waters) -> Set {
    let (slow, fast) = waters.drift;
    Set {
        flow: direction(rng.range(0., TAU)) * rng.range(slow, fast),
        meander: rng.range(0.12, 0.35),
        wavelength: rng.range(450., 900.),
        phase: rng.range(0., TAU),
    }
}

/// Smallest and largest growler, and smallest and largest berg, by radius in metres.
const GROWLERS: (f32, f32) = (2.2, 4.6);
const BERGS: (f32, f32) = (6., 17.);
/// The odd berg is far larger: a tabular slab broken from an ice shelf.
const SLABS: (f32, f32) = (19., 30.);

/// Ice scattered over the chart, clear of the land, the start, the gates and their
/// approaches, the harbour slips and the boats on patrol, and of each other.
fn ice(
    rng: &mut Rng,
    waters: &Waters,
    gates: &[Vec2; 5],
    harbours: &[HarbourSite; 3],
    islets: &[Islet],
) -> Vec<Ice> {
    let (fewest, most) = waters.ice;
    let count = fewest + rng.below(most.saturating_sub(fewest) + 1);
    let keepout = IceKeepout::new(gates, harbours, islets);
    let (lo, hi) = keepout.extent;
    let mut ice: Vec<Ice> = Vec::with_capacity(count);
    for _ in 0..count * 16 {
        if ice.len() == count {
            break;
        }
        let pos = Vec2::new(rng.range(lo.x, hi.x), rng.range(lo.y, hi.y));
        let (radius, shape) = if !rng.chance(waters.bergs) {
            (rng.range(GROWLERS.0, GROWLERS.1), IceShape::Growler)
        } else if rng.chance(0.12) {
            (rng.range(SLABS.0, SLABS.1), IceShape::Tabular)
        } else {
            let radius = BERGS.0 + (BERGS.1 - BERGS.0) * rng.unit().powf(1.5);
            let shape = match rng.below(3) {
                0 => IceShape::Dome,
                1 => IceShape::Pinnacle,
                _ => IceShape::Tabular,
            };
            (radius, shape)
        };
        let piece = Ice {
            pos,
            radius,
            shape,
            heading: rng.range(0., TAU),
        };
        if keepout.clear(&piece).is_none()
            && ice
                .iter()
                .all(|o| o.pos.distance(pos) >= o.radius + radius + 6.)
        {
            ice.push(piece);
        }
    }
    ice
}

/// Water the ice must leave open when the ship arrives. The survey holds a chart to the
/// same rules, so a hand-set chart with ice in the way fails too.
pub(super) struct IceKeepout {
    land: Vec<(Vec2, f32)>,
    gates: [Vec2; 5],
    lanes: Vec<(Vec2, Vec2)>,
    aprons: Vec<Vec2>,
    stations: Vec<Vec2>,
    /// Where ice may be scattered: the chart's water and a margin beyond.
    pub extent: (Vec2, Vec2),
}
impl IceKeepout {
    pub(super) fn new(gates: &[Vec2; 5], harbours: &[HarbourSite; 3], islets: &[Islet]) -> Self {
        let ports = Port::chart(harbours, 1., &mut 0);
        let mut land: Vec<(Vec2, f32)> = ports
            .iter()
            .map(|p| (p.body.pos, ISLAND_RADIUS + 18.))
            .collect();
        land.extend(islets.iter().map(|i| (i.pos, i.radius + 8.)));
        let mut previous = Vec2::ZERO;
        let mut lanes: Vec<(Vec2, Vec2)> = gates
            .iter()
            .map(|&g| {
                let dir = (g - previous).normalize_or_zero();
                previous = g;
                (g - dir * 60., g + dir * 60.)
            })
            .collect();
        lanes.push((Vec2::ZERO, gates[0]));
        let ahead = gates[0].normalize_or_zero();
        let mut stations: Vec<Vec2> = PICKETS
            .iter()
            .map(|&(range, lateral)| ahead * range + ahead.perp() * lateral)
            .collect();
        stations.push(Vec2::ZERO);
        let aprons = ports
            .iter()
            .flat_map(|p| {
                let (berth, f) = (p.berth(), p.body.forward());
                [berth, berth + f * 40., berth + f * 80.]
            })
            .collect();
        let (lo, hi) = std::iter::once(Vec2::ZERO)
            .chain(gates.iter().copied())
            .chain(harbours.iter().map(|h| h.pos))
            .chain(islets.iter().map(|i| i.pos))
            .fold((Vec2::MAX, Vec2::MIN), |(lo, hi), p| (lo.min(p), hi.max(p)));
        Self {
            land,
            gates: *gates,
            lanes,
            aprons,
            stations,
            extent: (lo - 140., hi + 140.),
        }
    }
    /// What a piece of ice is too close to, if anything.
    pub(super) fn clear(&self, piece: &Ice) -> Option<&'static str> {
        let (p, r) = (piece.pos, piece.radius);
        if self.land.iter().any(|&(c, room)| c.distance(p) < room + r) {
            Some("land")
        } else if p.length() < 110. + r {
            Some("the start")
        } else if self.gates.iter().any(|g| g.distance(p) < r + 50.) {
            Some("a gate")
        } else if self
            .lanes
            .iter()
            .any(|&(a, b)| segment_distance(p, a, b) < r + 22.)
        {
            Some("a gate approach")
        } else if self.aprons.iter().any(|a| a.distance(p) < r + 35.) {
            Some("a harbour slip")
        } else if self.stations.iter().any(|s| s.distance(p) < r + 30.) {
            Some("a patrol station")
        } else {
            None
        }
    }
}

pub(super) fn segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0., 1.);
    p.distance(a + ab * t)
}

/// SplitMix64: small, fast and the same on every platform, so a seed always replays.
pub(super) struct Rng(pub(super) u64);
impl Rng {
    pub(super) fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in [0, 1).
    pub(super) fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1_u64 << 24) as f32
    }
    pub(super) fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.unit()
    }
    pub(super) fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    pub(super) fn chance(&mut self, p: f32) -> bool {
        self.unit() < p
    }
    fn sign(&mut self) -> f32 {
        if self.chance(0.5) { 1. } else { -1. }
    }
    fn kind(&mut self, odds: [f32; 3]) -> LandmarkKind {
        let mut pick = self.unit() * odds.iter().sum::<f32>();
        for (kind, odd) in [
            LandmarkKind::Beacon,
            LandmarkKind::Stacks,
            LandmarkKind::Pines,
        ]
        .into_iter()
        .zip(odds)
        {
            if pick < odd {
                return kind;
            }
            pick -= odd;
        }
        LandmarkKind::Pines
    }
}

/// Mix a run's seed with a stage number, so each leg of a run gets its own waters.
pub fn leg_seed(run: u64, leg: u64) -> u64 {
    Rng(run ^ leg.wrapping_mul(0xA24B_AED4_963E_E407)).next()
}
