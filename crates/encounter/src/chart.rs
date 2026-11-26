//! Seeded sector charts: a coastal passage, three harbours along it and the islands off it.
//!
//! `Chart::generate` sketches candidates from a seed and the character of a stage's
//! waters, surveys each one (see `survey`), and keeps the best of the first few that
//! pass. Every run therefore sails different waters, and none of them is unwinnable.
//! Each passage runs along a craggy mainland: the ship enters at one end and fights her
//! way past the lightest base in the first bay and the middle base further on, to the
//! heavy yard at the far end (see `passage`).
use super::{
    Gyre, ISLAND_RADIUS, IceShape, Landmark, LandmarkKind, PICKETS, Port, Set, Survey, Vec2,
};
use std::f32::consts::TAU;

mod passage;
#[cfg(test)]
mod tests;

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
    /// The bases, stage by stage up the channel, each given as its number of factories:
    /// one for a cannery, two for the twin mills, three for the heavy yard. The last base
    /// of the last stage stands at the head of the channel. A stage stirs only once
    /// every base of the one before it is wrecked, or once it sights the ship.
    pub stages: &'static [&'static [u8]],
    /// Length of a reach holding one base, least and most, in metres along the channel.
    /// A reach holding two is seven tenths as long again.
    pub reach: (f32, f32),
    /// Width of the channel in its reaches, shore to shore, least and most.
    pub breadth: (f32, f32),
    /// Width of the channel at the narrows between one reach and the next.
    pub narrows: (f32, f32),
    /// How far the channel's line swings from side to side, in metres.
    pub bends: f32,
    /// How far each shore swings in and out over its own bays and points, in metres.
    pub bays: f32,
    /// Crags standing out from the cliffs, per kilometre of shore.
    pub crags: f32,
    /// How many islands to raise in the channel, at least and at most. Crowded water
    /// can leave fewer.
    pub islands: (usize, usize),
    /// Smallest and largest island radius in metres.
    pub island_radius: (f32, f32),
    /// Share of islands raised in groups rather than alone.
    pub clustering: f32,
    /// Pairs of islands to try either side of a leg, so the course runs through a strait.
    pub straits: usize,
    /// Open water kept either side of each leg, in metres, except at straits.
    pub fairway: f32,
    /// Share of lone islands allowed to stand in a fairway, so the leg is sailed round it.
    pub strays: f32,
    /// Relative odds of beacon, stack and pine islands.
    pub kinds: [f32; 3],
    /// Glaciers to bring down to the channel from either shore, at least and at most,
    /// and their smallest and largest radius in metres. One more always fills the head.
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
    /// A winding channel with islands in pairs either side of the course.
    pub const NARROWS: Self = Self {
        stages: &[&[1], &[2], &[2, 3]],
        reach: (800., 950.),
        breadth: (960., 1200.),
        narrows: (520., 620.),
        bends: 195.,
        bays: 35.,
        crags: 8.,
        islands: (10, 16),
        island_radius: (20., 40.),
        clustering: 0.3,
        straits: 2,
        fairway: 55.,
        strays: 0.1,
        kinds: [1., 1., 1.],
        glaciers: (1, 1),
        glacier_radius: (90., 130.),
        drift: (0.35, 0.6),
        ice: (32, 48),
        bergs: 0.35,
    };
    /// A broad, straight reach under the cliffs, with a few rocks and beacons in it.
    pub const OPEN: Self = Self {
        stages: &[&[1], &[1, 2], &[2], &[3]],
        reach: (850., 1000.),
        breadth: (1240., 1520.),
        narrows: (620., 740.),
        bends: 120.,
        bays: 45.,
        crags: 5.,
        islands: (8, 14),
        island_radius: (18., 40.),
        clustering: 0.2,
        straits: 0,
        fairway: 80.,
        strays: 0.25,
        kinds: [1.5, 0.8, 0.7],
        glaciers: (1, 2),
        glacier_radius: (110., 160.),
        drift: (0.25, 0.45),
        ice: (32, 48),
        bergs: 0.5,
    };
    /// Sheltered water among wooded islands.
    pub const SOUND: Self = Self {
        stages: &[&[1], &[1, 2], &[3]],
        reach: (825., 975.),
        breadth: (1150., 1380.),
        narrows: (570., 670.),
        bends: 150.,
        bays: 30.,
        crags: 6.,
        islands: (14, 22),
        island_radius: (22., 44.),
        clustering: 0.35,
        straits: 1,
        fairway: 50.,
        strays: 0.1,
        kinds: [0.6, 0.8, 1.6],
        glaciers: (1, 2),
        glacier_radius: (90., 130.),
        drift: (0.2, 0.35),
        ice: (44, 60),
        bergs: 0.2,
    };
    /// A tight, crooked course hard under the crags, threaded between crowded rocks.
    pub const CUT: Self = Self {
        stages: &[&[1], &[1, 2], &[2], &[2], &[2, 3]],
        reach: (800., 900.),
        breadth: (920., 1100.),
        narrows: (490., 570.),
        bends: 225.,
        bays: 40.,
        crags: 11.,
        islands: (12, 20),
        island_radius: (18., 36.),
        clustering: 0.5,
        straits: 2,
        fairway: 40.,
        strays: 0.18,
        kinds: [0.8, 1.6, 0.6],
        glaciers: (1, 2),
        glacier_radius: (80., 110.),
        drift: (0.5, 0.8),
        ice: (32, 48),
        bergs: 0.3,
    };

    /// The same waters with the bases staged differently.
    pub const fn staged(self, stages: &'static [&'static [u8]]) -> Self {
        Self { stages, ..self }
    }
    /// How many bases the waters hold.
    pub fn bases(&self) -> usize {
        self.stages.iter().map(|stage| stage.len()).sum()
    }
}

/// Where a harbour stands, the way its slip faces, what it works and when it stirs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HarbourSite {
    pub pos: Vec2,
    pub heading: f32,
    /// Factories: one at a cannery, two at the twin mills, three at the heavy yard.
    pub works: u8,
    /// The stage of the passage the base belongs to. The bases of stage 0 are roused
    /// when the ship arrives, and each later stage once every base of the one before
    /// it is wrecked, or as soon as one of its own lookouts sights her.
    pub stage: u8,
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

/// How the gates of a chart are sailed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Course {
    /// A closed circuit of buoys, the hand-set charts and the helm trial: the last gate
    /// leads back to the first.
    Circuit,
    /// A run up a channel: from the start through each mark in turn, past the bases
    /// stage by stage, to the heavy yard at its head.
    Passage,
}

/// The fixed geography of one sector. The ship enters at the origin.
#[derive(Clone, Debug, PartialEq)]
pub struct Chart {
    /// The seed the chart was drawn from, or `None` for a classic hand-set chart.
    pub seed: Option<u64>,
    pub course: Course,
    /// The five buoys of a circuit, or the marks a passage runs through. Patrols sail
    /// them unmarked: they set out the fairways the survey keeps open.
    pub gates: Vec<Vec2>,
    /// The bases, lettered from A in this order. On a passage they come stage by stage
    /// up the channel, the heavy yard last at its head.
    pub harbours: Vec<HarbourSite>,
    /// Islands standing alone in the water.
    pub islets: Vec<Islet>,
    /// The mainland a passage runs through, as overlapping pieces of both shores, with
    /// the glaciers that come down to the water from them. A circuit has none.
    pub coast: Vec<Islet>,
    /// Where the two patrol boats, then the two destroyers, are on patrol when the ship
    /// arrives.
    pub pickets: [Vec2; 4],
    /// The current far from land. The islands shape it into the stream sailed.
    pub set: Set,
    /// The water turning slowly round in each reach of a channel closed at its head,
    /// where the set itself reaches only a little way in.
    pub gyres: Vec<Gyre>,
    /// Ice adrift on the current when the ship arrives.
    pub ice: Vec<Ice>,
}
impl Chart {
    /// The hand-set layout around a buoy circuit: a harbour outside gates 1, 2 and 4,
    /// and an island either side of each leg where there is room.
    pub fn classic(gates: [Vec2; 5]) -> Self {
        let harbours = Port::classic_sites(gates).to_vec();
        let islets = Landmark::classic(gates, &harbours);
        let ahead = gates[0].normalize_or(Vec2::NEG_Y);
        Self {
            seed: None,
            course: Course::Circuit,
            gates: gates.to_vec(),
            harbours,
            islets,
            coast: vec![],
            pickets: PICKETS.map(|(range, lateral)| ahead * range + ahead.perp() * lateral),
            set: Set::SLACK,
            gyres: vec![],
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
        let passage = passage::sketch(rng, waters);
        let ice = ice(
            rng,
            waters,
            &passage.gates,
            &passage.harbours,
            &passage.islets,
            &passage.coast,
            &passage.pickets,
        );
        Self {
            seed: Some(seed),
            course: Course::Passage,
            gates: passage.gates,
            harbours: passage.harbours,
            islets: passage.islets,
            coast: passage.coast,
            pickets: passage.pickets,
            set: passage.set,
            gyres: passage.gyres,
            ice,
        }
    }

    /// The middle of the gates.
    pub fn middle(&self) -> Vec2 {
        self.gates.iter().sum::<Vec2>() / self.gates.len() as f32
    }
    /// Each leg of the course in turn. A circuit's run from gate `k` to the next, the
    /// last closing the lap; a passage's from the start to the first mark and on to
    /// the last.
    pub fn legs(&self) -> Vec<(Vec2, Vec2)> {
        legs(self.course, &self.gates)
    }
    /// How many stages the bases come in.
    pub fn stages(&self) -> usize {
        self.harbours
            .iter()
            .map(|h| usize::from(h.stage) + 1)
            .max()
            .unwrap_or(0)
    }
    /// Corners of the water the chart sets out: the start, gates, harbours and islands,
    /// and the shore of any coast within a few hundred metres of them.
    pub fn extent(&self) -> (Vec2, Vec2) {
        let points = std::iter::once((Vec2::ZERO, 0.))
            .chain(self.gates.iter().map(|&g| (g, 0.)))
            .chain(self.harbours.iter().map(|h| (h.pos, ISLAND_RADIUS)))
            .chain(self.islets.iter().map(|i| (i.pos, i.radius)));
        let (lo, hi) = points.fold((Vec2::MAX, Vec2::MIN), |(lo, hi), (p, r)| {
            (lo.min(p - r), hi.max(p + r))
        });
        let (near, far) = (lo - COAST_IN_VIEW, hi + COAST_IN_VIEW);
        self.coast.iter().fold((lo, hi), |(lo, hi), piece| {
            let low = (piece.pos - piece.radius).max(near);
            let high = (piece.pos + piece.radius).min(far);
            if low.cmplt(high).all() {
                (lo.min(low), hi.max(high))
            } else {
                (lo, hi)
            }
        })
    }
    /// The land the current runs round: harbours, islands and the coast, as centre and
    /// radius.
    pub fn land(&self) -> Vec<(Vec2, f32)> {
        self.harbours
            .iter()
            .map(|h| (h.pos, ISLAND_RADIUS))
            .chain(self.islets.iter().map(|i| (i.pos, i.radius)))
            .chain(self.coast.iter().map(|i| (i.pos, i.radius)))
            .collect()
    }
}

/// How far beyond the open water the chart's extent takes in the coast.
const COAST_IN_VIEW: f32 = 300.;

fn legs(course: Course, gates: &[Vec2]) -> Vec<(Vec2, Vec2)> {
    let n = gates.len();
    match course {
        Course::Circuit => (0..n).map(|k| (gates[k], gates[(k + 1) % n])).collect(),
        Course::Passage => (0..n)
            .map(|k| (if k == 0 { Vec2::ZERO } else { gates[k - 1] }, gates[k]))
            .collect(),
    }
}

/// Water the islands must leave open: the start, the gates and their approaches, the
/// stations of the ships already at sea, the harbours and their slips, where the flagship
/// also sails from, and the fairways.
struct Keepout {
    gate_lanes: Vec<(Vec2, Vec2)>,
    gates: Vec<Vec2>,
    fairways: Vec<(Vec2, Vec2)>,
    stations: Vec<(Vec2, f32)>,
    harbours: Vec<(Vec2, [Vec2; 3])>,
    fairway: f32,
}
impl Keepout {
    fn new(
        waters: &Waters,
        course: Course,
        gates: &[Vec2],
        harbours: &[HarbourSite],
        pickets: &[Vec2; 4],
    ) -> Self {
        let mut previous = Vec2::ZERO;
        let gate_lanes = gates
            .iter()
            .map(|&g| {
                let dir = (g - previous).normalize_or_zero();
                previous = g;
                (g - dir * 70., g + dir * 70.)
            })
            .collect();
        let mut fairways = legs(course, gates);
        fairways.push((Vec2::ZERO, gates[0]));
        let mut stations: Vec<(Vec2, f32)> = pickets.iter().map(|&p| (p, 45.)).collect();
        stations.push((Vec2::ZERO, 70.));
        let ports = Port::chart(harbours, 1., &mut 0);
        Self {
            gate_lanes,
            gates: gates.to_vec(),
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
    /// Clear of the gates and their approaches, the stations and the slips, and at
    /// least `harbour_room` from the middle of any harbour.
    fn open(&self, islet: &Islet, harbour_room: f32) -> bool {
        let (p, r) = (islet.pos, islet.radius);
        self.gates.iter().all(|g| g.distance(p) >= r + 60.)
            && self
                .gate_lanes
                .iter()
                .all(|&(a, b)| segment_distance(p, a, b) >= r + 26.)
            && self
                .stations
                .iter()
                .all(|&(s, room)| s.distance(p) >= r + room)
            && self.harbours.iter().all(|(centre, apron)| {
                centre.distance(p) >= r + harbour_room
                    && apron.iter().all(|a| a.distance(p) >= r + 40.)
            })
    }
    fn clear_of_fairways(&self, islet: &Islet) -> bool {
        self.fairways
            .iter()
            .all(|&(a, b)| segment_distance(islet.pos, a, b) >= islet.radius + self.fairway)
    }
    /// Room for an island among those `placed`, the coast included.
    fn allows(&self, islet: &Islet, placed: &[Islet], in_fairway: bool) -> bool {
        placed
            .iter()
            .all(|o| o.pos.distance(islet.pos) >= islet.radius + o.radius + STRAIT)
            && self.open(islet, 105.)
            && (in_fairway || self.clear_of_fairways(islet))
    }
    /// Room for a piece of the shore, which may overlap the rest of the coast.
    fn allows_shore(&self, piece: &Islet, harbour_room: f32) -> bool {
        self.open(piece, harbour_room) && self.clear_of_fairways(piece)
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
    gates: &[Vec2],
    harbours: &[HarbourSite],
    islets: &[Islet],
    coast: &[Islet],
    pickets: &[Vec2; 4],
) -> Vec<Ice> {
    let (fewest, most) = waters.ice;
    let count = fewest + rng.below(most.saturating_sub(fewest) + 1);
    let keepout = IceKeepout::new(gates, harbours, islets, coast, pickets);
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
    gates: Vec<Vec2>,
    lanes: Vec<(Vec2, Vec2)>,
    aprons: Vec<Vec2>,
    stations: Vec<Vec2>,
    /// Where ice may be scattered: the chart's water and a margin beyond.
    pub extent: (Vec2, Vec2),
}
impl IceKeepout {
    pub(super) fn new(
        gates: &[Vec2],
        harbours: &[HarbourSite],
        islets: &[Islet],
        coast: &[Islet],
        pickets: &[Vec2; 4],
    ) -> Self {
        let ports = Port::chart(harbours, 1., &mut 0);
        let mut land: Vec<(Vec2, f32)> = ports
            .iter()
            .map(|p| (p.body.pos, ISLAND_RADIUS + 18.))
            .collect();
        land.extend(islets.iter().chain(coast).map(|i| (i.pos, i.radius + 8.)));
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
        let mut stations = pickets.to_vec();
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
            gates: gates.to_vec(),
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
