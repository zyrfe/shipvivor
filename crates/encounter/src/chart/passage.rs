//! A coastal passage. The course runs along a craggy mainland from the start: past the
//! lightest base in the first bay, round a headland or a glacier, past the middle base,
//! and on to the heavy yard at the far end, where a last cape turns the coast out to sea.
//! Each base stands on its own island in its bay with a channel behind it, and each bay
//! is hidden from the one before by the headland between them.
use super::{Course, HarbourSite, Islet, Keepout, Rng, STRAIT, Waters, legs};
use crate::{ISLAND_RADIUS, LandmarkKind, Set, Vec2};
use shipvivor_compass::{direction, heading};
use std::f32::consts::TAU;

/// Coast drawn beyond each end of the passage, so the shore runs on out of sight.
const OVERHANG: f32 = 560.;
/// Water between a base's island and the shore behind it, least and most.
const BACKWATER: (f32, f32) = (STRAIT + 8., STRAIT + 30.);
/// Room the shore leaves round the middle of a base's island.
const SHORE_ROOM: f32 = ISLAND_RADIUS + STRAIT + 4.;
/// How far off its base the mark for each bay lies, least and most.
const OFF_BASE: (f32, f32) = (165., 235.);
/// Where along the passage the first two bases stand, as shares of its length. The heavy
/// yard is always at the far end.
const BAYS: [(f32, f32); 2] = [(0.24, 0.31), (0.55, 0.63)];
/// Smallest and largest radius of the broad hills along the shore, and of the
/// mountains behind them.
const HILLS: (f32, f32) = (70., 190.);
const MOUNTAINS: (f32, f32) = (300., 420.);
/// How far each base's cove runs back into the land, least and most.
const COVES: (f32, f32) = (60., 115.);
/// Smallest and largest crag, and of a headland.
const CRAGS: (f32, f32) = (12., 42.);
const HEADLANDS: (f32, f32) = (55., 100.);
/// Stations of the boats on patrol when the ship arrives, as distance along the course
/// and distance to seaward of it: two patrol boats short of the first bay, and two
/// destroyers beyond them, one well out to sea.
const STATIONS: [(f32, f32); 4] = [(330., 40.), (430., 150.), (720., 110.), (560., 330.)];

/// The passage's own axes: `along` the coast from the start, `seaward` off it.
#[derive(Clone, Copy)]
struct Frame {
    along: Vec2,
    seaward: Vec2,
}
impl Frame {
    fn at(&self, s: f32, t: f32) -> Vec2 {
        self.along * s + self.seaward * t
    }
    fn s(&self, p: Vec2) -> f32 {
        p.dot(self.along)
    }
    fn t(&self, p: Vec2) -> f32 {
        p.dot(self.seaward)
    }
}

/// The line the shore follows before crags and headlands break it, as distance seaward
/// of the passage's axis at each distance along it.
struct Shore {
    depth: f32,
    swells: [(f32, f32, f32); 2],
    /// Where along the passage a cove opens, how deep it runs and its half-width.
    coves: Vec<(f32, f32, f32)>,
}
impl Shore {
    fn t(&self, s: f32) -> f32 {
        let swell = self
            .swells
            .iter()
            .fold(-self.depth, |t, &(amplitude, wavelength, phase)| {
                t + amplitude * (TAU * s / wavelength + phase).sin()
            });
        self.coves.iter().fold(swell, |t, &(at, depth, half)| {
            let u = (s - at) / half;
            if u.abs() < 1. {
                t - depth * (u * TAU / 4.).cos().powi(2)
            } else {
                t
            }
        })
    }
    fn point(&self, frame: &Frame, s: f32) -> Vec2 {
        frame.at(s, self.t(s))
    }
    fn normal(&self, frame: &Frame, s: f32) -> Vec2 {
        let slope = (self.t(s + 2.) - self.t(s - 2.)) / 4.;
        (frame.seaward - frame.along * slope).normalize()
    }
}

pub(super) struct Passage {
    pub gates: [Vec2; 5],
    pub harbours: [HarbourSite; 3],
    pub pickets: [Vec2; 4],
    pub coast: Vec<Islet>,
    pub islets: Vec<Islet>,
    pub set: Set,
}

pub(super) fn sketch(rng: &mut Rng, waters: &Waters) -> Passage {
    // The coast runs near a compass point, so the chart's square of water stays compact.
    let quarter = rng.below(4) as f32 * TAU / 4.;
    let along = direction(quarter + rng.range(-0.2, 0.2));
    let frame = Frame {
        along,
        seaward: along.perp() * rng.sign(),
    };
    let length = waters.length * rng.range(0.92, 1.08);
    let mut shore = Shore {
        depth: rng.range(waters.offing.0, waters.offing.1),
        swells: [
            (
                waters.bays * rng.range(0.5, 0.8),
                rng.range(700., 1100.),
                rng.range(0., TAU),
            ),
            (
                waters.bays * rng.range(0.2, 0.4),
                rng.range(300., 480.),
                rng.range(0., TAU),
            ),
        ],
        coves: vec![],
    };
    // A cove for each base, the first two a quarter and past halfway along, the heavy
    // yard's at the far end.
    let bays = [
        rng.range(BAYS[0].0, BAYS[0].1) * length,
        rng.range(BAYS[1].0, BAYS[1].1) * length,
        length,
    ];
    shore.coves = bays
        .iter()
        .map(|&s| (s, rng.range(COVES.0, COVES.1), rng.range(190., 270.)))
        .collect();
    let harbours = bases(rng, &frame, &shore, bays);
    let gates = marks(rng, waters, &frame, &shore, &harbours);
    let pickets = stations(&gates, frame.seaward);
    let keepout = Keepout::new(waters, Course::Passage, &gates, &harbours, &pickets);
    let (coast, unplaced) = mainland(rng, waters, &frame, &shore, &keepout, &harbours, length);
    let islets = offshore(
        rng, waters, &frame, &gates, &keepout, &coast, length, unplaced,
    );
    // The current sets along the coast, one way or the other. The land is the edge of
    // the sea, not an island in it, so the stream runs parallel to the shore's line and
    // the shore is one streamline from end to end.
    let (slow, fast) = waters.drift;
    let set = Set {
        flow: frame.along * rng.sign() * rng.range(slow, fast),
        meander: rng.range(0.05, 0.15),
        wavelength: rng.range(600., 1100.),
        phase: rng.range(0., TAU),
    };
    Passage {
        gates,
        harbours,
        pickets,
        coast,
        islets,
        set,
    }
}

/// A base at the head of each cove, on an island off the shore with its slip opening
/// onto the fairway.
fn bases(rng: &mut Rng, frame: &Frame, shore: &Shore, bays: [f32; 3]) -> [HarbourSite; 3] {
    bays.map(|s| {
        let out = shore.normal(frame, s);
        let pos =
            shore.point(frame, s) + out * (ISLAND_RADIUS + rng.range(BACKWATER.0, BACKWATER.1));
        // The slip turns a little up or down the coast.
        let slip = Vec2::from_angle(rng.range(-0.45, 0.45)).rotate(out);
        HarbourSite {
            pos,
            heading: heading(slip),
        }
    })
}

/// The marks: off the point before the first bay, off base A, between the first two
/// bays, off base B, and off the heavy yard at the end.
fn marks(
    rng: &mut Rng,
    waters: &Waters,
    frame: &Frame,
    shore: &Shore,
    harbours: &[HarbourSite; 3],
) -> [Vec2; 5] {
    let (near, far) = waters.offing;
    let (a, b) = (frame.s(harbours[0].pos), frame.s(harbours[1].pos));
    let point = a * rng.range(0.42, 0.56);
    let between = a + (b - a) * rng.range(0.42, 0.58);
    let [point, between] = [point, between].map(|s| frame.at(s, shore.t(s) + rng.range(near, far)));
    let [first, second, last] = harbours
        .map(|h| h.pos + shore.normal(frame, frame.s(h.pos)) * rng.range(OFF_BASE.0, OFF_BASE.1));
    [point, first, between, second, last]
}

/// A point on the course, `run` metres from the start.
fn along_course(gates: &[Vec2; 5], mut run: f32) -> Vec2 {
    let mut from = Vec2::ZERO;
    for &to in gates {
        let leg = from.distance(to);
        if run <= leg {
            return from.lerp(to, run / leg.max(1e-3));
        }
        run -= leg;
        from = to;
    }
    from
}

/// Where the boats on patrol lie when the ship arrives.
fn stations(gates: &[Vec2; 5], seaward: Vec2) -> [Vec2; 4] {
    STATIONS.map(|(run, off)| along_course(gates, run) + seaward * off)
}

/// The shore as built so far, distance to seaward at a distance along: the farthest
/// any piece of the coast reaches there.
fn coastline(coast: &[Islet], frame: &Frame, s: f32, fallback: f32) -> f32 {
    coast
        .iter()
        .filter_map(|piece| {
            let ds = s - frame.s(piece.pos);
            (ds.abs() < piece.radius)
                .then(|| frame.t(piece.pos) + (piece.radius * piece.radius - ds * ds).sqrt())
        })
        .reduce(f32::max)
        .unwrap_or(fallback)
}

/// Pieces of the coast that stand out from the shore: crags, headlands and glaciers.
/// Two of them either run together or leave a channel wide enough to sail into.
fn apart(piece: &Islet, standing: &[Islet]) -> bool {
    standing.iter().all(|o| {
        let gap = piece.pos.distance(o.pos) - piece.radius - o.radius;
        gap <= -0.35 * piece.radius.min(o.radius) || gap >= STRAIT + 5.
    })
}

/// The mainland: broad hills along the shore and mountains behind, a headland or a
/// glacier between each pair of bays, crags standing out from the cliffs, and a cape
/// beyond the last base. Returns the coast and how many glaciers found no room on it.
fn mainland(
    rng: &mut Rng,
    waters: &Waters,
    frame: &Frame,
    shore: &Shore,
    keepout: &Keepout,
    harbours: &[HarbourSite; 3],
    length: f32,
) -> (Vec<Islet>, usize) {
    let mut coast: Vec<Islet> = vec![];
    let piece = |rng: &mut Rng, pos: Vec2, radius: f32| Islet {
        pos,
        radius,
        kind: LandmarkKind::Mainland,
        heading: rng.range(0., TAU),
    };
    // Hills whose seaward faces make the shore. A hill that would crowd the fairway or a
    // base stands farther back, which opens a bay round each base.
    let mut s = -OVERHANG;
    while s < length + OVERHANG {
        let radius = rng.range(HILLS.0, HILLS.1);
        let mut t = shore.t(s) - radius + rng.range(-5., 5.);
        let mut hill = piece(rng, frame.at(s, t), radius);
        for _ in 0..60 {
            if keepout.allows_shore(&hill, SHORE_ROOM) {
                coast.push(hill);
                break;
            }
            t -= 5.;
            hill.pos = frame.at(s, t);
        }
        s += radius * rng.range(0.3, 0.5);
    }
    // Mountains behind, so the land runs on inland.
    let mut s = -OVERHANG;
    while s < length + OVERHANG {
        let radius = rng.range(MOUNTAINS.0, MOUNTAINS.1);
        let mountain = piece(rng, frame.at(s, shore.t(s) - 150. - radius), radius);
        coast.push(mountain);
        s += radius * rng.range(0.6, 0.8);
    }
    let backbone = coast.len();
    let edge = |coast: &[Islet], s: f32| coastline(&coast[..backbone], frame, s, shore.t(s));

    // Between the bays: a headland, or a glacier coming down to the sea in its place.
    // Which gaps get one is shuffled.
    let [a, b] = [0, 1].map(|i| frame.s(harbours[i].pos));
    let mut gaps = [a * 0.5, (a + b) * 0.5, (b + length) * 0.5];
    for i in (1..gaps.len()).rev() {
        gaps.swap(i, rng.below(i + 1));
    }
    let (fewest, most) = waters.glaciers;
    let mut glaciers = if most == 0 {
        0
    } else {
        fewest + rng.below(most.saturating_sub(fewest) + 1)
    };
    let (fewest, most) = waters.headlands;
    let headlands = fewest + rng.below(most.saturating_sub(fewest) + 1);
    let mut standing: Vec<Islet> = vec![];
    for &gap in gaps.iter().take(headlands) {
        let glacier = glaciers > 0;
        let s = gap + rng.range(-40., 40.);
        let out = shore.normal(frame, s);
        let (radius, kind) = if glacier {
            let (small, large) = waters.glacier_radius;
            (rng.range(small, large), LandmarkKind::Glacier)
        } else {
            (rng.range(HEADLANDS.0, HEADLANDS.1), LandmarkKind::Mainland)
        };
        // The glacier's tongue lies to starboard of its heading, so it faces the sea.
        let facing = heading(Vec2::new(out.y, -out.x));
        let foot = edge(&coast, s);
        let mut reach = radius * if glacier { 0.6 } else { 1.3 };
        while reach >= radius * 0.2 {
            let jut = Islet {
                pos: frame.at(s, foot + reach - radius),
                radius,
                kind,
                heading: if glacier { facing } else { rng.range(0., TAU) },
            };
            if keepout.allows_shore(&jut, 105.) && apart(&jut, &standing) {
                standing.push(jut);
                glaciers -= usize::from(glacier);
                break;
            }
            reach -= 12.;
        }
    }
    // The cape beyond the heavy yard, as far out as it will go, so the passage ends in
    // the yard's bight.
    let s = length + rng.range(230., 300.);
    let radius = rng.range(80., 130.);
    let foot = edge(&coast, s);
    let mut reach = 320.;
    while reach >= 60. {
        let cape = piece(rng, frame.at(s, foot + reach - radius), radius);
        if keepout.allows_shore(&cape, 105.) && apart(&cape, &standing) {
            standing.push(cape);
            break;
        }
        reach -= 15.;
    }
    // Crags stand out from the cliffs all along.
    let every = 1000. / waters.crags.max(0.1);
    let mut s = -OVERHANG * 0.7 + rng.range(0., every);
    while s < length + OVERHANG * 0.7 {
        let radius = CRAGS.0 + (CRAGS.1 - CRAGS.0) * rng.unit().powi(2);
        let out = edge(&coast, s) + radius * rng.range(-0.3, 0.55);
        let crag = piece(rng, frame.at(s, out), radius);
        if keepout.allows_shore(&crag, SHORE_ROOM) && apart(&crag, &standing) {
            standing.push(crag);
        }
        s += every * rng.range(0.6, 1.4);
    }
    coast.extend(standing);
    (coast, glaciers)
}

/// The fairway's distance to seaward at a distance along the passage.
fn fairway_line(frame: &Frame, gates: &[Vec2; 5], s: f32) -> f32 {
    let mut from = Vec2::ZERO;
    for &to in gates {
        let (a, b) = (frame.s(from), frame.s(to));
        if s <= b || to == gates[4] {
            let u = ((s - a) / (b - a).abs().max(1.)).clamp(0., 1.);
            return frame.t(from) + (frame.t(to) - frame.t(from)) * u;
        }
        from = to;
    }
    frame.t(gates[4])
}

/// Islands off the coast: any glacier with no room ashore, straits on the fairway, a
/// chain of skerries outside it, and groups and lone islands between the shore and the
/// open sea.
#[allow(clippy::too_many_arguments)]
fn offshore(
    rng: &mut Rng,
    waters: &Waters,
    frame: &Frame,
    gates: &[Vec2; 5],
    keepout: &Keepout,
    coast: &[Islet],
    length: f32,
    glaciers: usize,
) -> Vec<Islet> {
    let mut placed: Vec<Islet> = coast.to_vec();
    let first = placed.len();
    let (fewest, most) = waters.islands;
    let target = first + fewest + rng.below(most.saturating_sub(fewest) + 1);
    let raise = |rng: &mut Rng, pos: Vec2| {
        let kind = rng.kind(waters.kinds);
        let (small, large) = waters.island_radius;
        let size = match kind {
            LandmarkKind::Beacon => rng.range(0., 0.45),
            LandmarkKind::Stacks => rng.range(0.15, 0.85),
            _ => rng.range(0.3, 1.),
        };
        Islet {
            pos,
            radius: small + (large - small) * size,
            kind,
            heading: rng.range(0., TAU),
        }
    };
    let line = |s: f32| fairway_line(frame, gates, s);
    let shore = |s: f32| coastline(coast, frame, s, line(s) - 200.);
    for _ in 0..glaciers {
        let (smallest, largest) = waters.glacier_radius;
        let wanted = rng.range(smallest, largest);
        let heading = rng.range(0., TAU);
        let mut best: Option<Islet> = None;
        for _ in 0..24 {
            let s = rng.range(0.1, 0.95) * length;
            let mut radius = wanted;
            while radius >= smallest * 0.8 && best.is_none_or(|b| radius > b.radius) {
                let islet = Islet {
                    pos: frame.at(s, line(s) + rng.range(130., 360.) + radius),
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
    let legs = legs(Course::Passage, gates);
    for _ in 0..waters.narrows {
        for _ in 0..8 {
            let (a, b) = legs[1 + rng.below(4)];
            if a.distance(b) < 280. {
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
    // Skerries: a broken chain outside the fairway, sheltering the water inside it.
    for j in 0..waters.skerries {
        let s = (j as f32 + rng.range(0.2, 0.8)) / waters.skerries as f32 * (length + 200.) - 100.;
        let mut islet = raise(rng, Vec2::ZERO);
        islet.pos = frame.at(s, line(s) + rng.range(105., 185.) + islet.radius);
        if keepout.allows(&islet, &placed, false) {
            placed.push(islet);
        }
    }
    // Groups and lone islands fill the water between the shore and the open sea.
    let mut tries = 0;
    while placed.len() < target && tries < 400 {
        tries += 1;
        let s = rng.range(-150., length + 150.);
        let (inner, outer) = (shore(s) + 50., line(s) + 430.);
        let around = frame.at(s, rng.range(inner, outer.max(inner + 1.)));
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
    placed.split_off(first)
}
