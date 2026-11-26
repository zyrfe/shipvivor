//! A channel between two shores. The ship enters at its mouth and fights her way up it
//! reach by reach to the heavy yard at its head. Each reach holds the bases of one stage,
//! each on its own island in a cove off one shore, and narrows part one reach from the
//! next, so high ground at a bend hides what lies beyond. The channel bends as it goes
//! and closes at its head, where a glacier comes down to the water behind the heavy yard.
use super::{Course, HarbourSite, Islet, Keepout, Rng, STRAIT, Waters, legs};
use crate::{Gyre, ISLAND_RADIUS, LandmarkKind, Set, Vec2};
use shipvivor_compass::{direction, heading};
use std::f32::consts::{FRAC_PI_2, TAU};

/// Land drawn beyond the head, so the hills run on out of sight.
const OVERHANG: f32 = 560.;
/// Where the shores end to seaward, in metres short of the start. Beyond lies open sea.
const SEAWARD: f32 = 520.;
/// Where the entrance narrows behind the start, and how much it opens out to seaward.
const MOUTH: f32 = -110.;
const FLARE: f32 = 0.9;
/// Metres either side of a narrows over which the shores close in to it.
const PINCH: f32 = 300.;
/// Metres beyond the heavy yard at which the shores start to close, and least and most
/// at which they meet at the head. The yard keeps open water round it on every hand.
const CLOSING: f32 = 220.;
const HEAD: (f32, f32) = (600., 720.);
/// Water between a base's island and the shore behind it, least and most.
const BACKWATER: (f32, f32) = (STRAIT + 8., STRAIT + 30.);
/// Room the shore leaves round the middle of a base's island.
const SHORE_ROOM: f32 = ISLAND_RADIUS + STRAIT + 4.;
/// How far off its base the mark for it lies, least and most.
const OFF_BASE: (f32, f32) = (200., 280.);
/// How fast the water of each reach runs along its shores, as a share of the set
/// outside, and how far apart the gyres down its middle stand, as a share of half the
/// channel's width.
const GYRE_SHARE: f32 = 0.8;
const GYRE_SPACING: f32 = 1.2;
/// How much longer a reach holding two bases is than one holding one.
const PAIRED: f32 = 1.7;
/// Nearest the first base stands to the start, in metres along the channel.
const FIRST_BASE: f32 = 590.;
/// Smallest and largest radius of the broad hills along each shore, and of the
/// mountains behind them.
const HILLS: (f32, f32) = (70., 190.);
const MOUNTAINS: (f32, f32) = (300., 420.);
/// How far each base's cove runs back into the land, least and most, and its half-width.
const COVES: (f32, f32) = (60., 110.);
const COVE_WIDTH: (f32, f32) = (170., 240.);
/// Smallest and largest crag.
const CRAGS: (f32, f32) = (12., 40.);
/// Stations of the boats on patrol when the ship arrives, as distance along the course
/// and distance across it: two patrol boats short of the first base, and two destroyers
/// about it. Each lies on its own bearing from the start, so none hides another.
const STATIONS: [(f32, f32); 4] = [(300., 50.), (400., -120.), (620., 40.), (520., 150.)];

/// The channel's own axes: `along` it from the mouth to the head, `across` it.
#[derive(Clone, Copy)]
struct Frame {
    along: Vec2,
    across: Vec2,
}
impl Frame {
    fn at(&self, s: f32, t: f32) -> Vec2 {
        self.along * s + self.across * t
    }
    fn s(&self, p: Vec2) -> f32 {
        p.dot(self.along)
    }
    fn t(&self, p: Vec2) -> f32 {
        p.dot(self.across)
    }
}

/// The shape of the channel before crags and glaciers break its shores: its line, its
/// width reach by reach, and each shore's own bays and coves. A side is `1.` or `-1.`,
/// the way across the channel its shore lies from the middle.
struct Channel {
    /// Swings of the channel's line: amplitude, wavelength and phase.
    bends: [(f32, f32, f32); 2],
    /// Where the swings put the line at the start, taken off so it passes through it.
    offset: f32,
    /// Where each reach ends along the channel, and half its width.
    reaches: Vec<(f32, f32)>,
    /// Half the width at the narrows that ends each reach but the last.
    narrows: Vec<f32>,
    /// Each shore's own swell, port and starboard of the middle.
    swells: [(f32, f32, f32); 2],
    /// Where along the channel a cove opens, how deep it runs, its half-width and side.
    coves: Vec<(f32, f32, f32, f32)>,
    /// Where the shores meet at the head.
    head: f32,
}
impl Channel {
    /// Distance across to the middle of the channel. It passes through the start.
    fn middle(&self, s: f32) -> f32 {
        self.bends
            .iter()
            .map(|&(amplitude, wavelength, phase)| amplitude * (TAU * s / wavelength + phase).sin())
            .sum::<f32>()
            - self.offset
    }
    /// Half the width of the open channel, before each shore's swell and coves.
    fn half(&self, s: f32) -> f32 {
        let last = self.reaches.len() - 1;
        let reach = self
            .reaches
            .iter()
            .position(|&(end, _)| s < end)
            .unwrap_or(last);
        let mut half = self.reaches[reach].1;
        // Close in to the narrows either side.
        for (j, &narrow) in self.narrows.iter().enumerate() {
            let at = self.reaches[j].0;
            let u = (s - at).abs() / PINCH;
            if u < 1. {
                let open = if s < at {
                    self.reaches[j].1
                } else {
                    self.reaches[j + 1].1
                };
                half = narrow + (open - narrow) * (u * FRAC_PI_2).sin().powi(2);
            }
        }
        // The entrance narrows a little behind the start and opens out to seaward.
        if s < MOUTH {
            half = self.reaches[0].1 * 0.8 + (MOUTH - s) * FLARE;
        } else if s < 0. {
            let u = (s - MOUTH) / -MOUTH;
            half = self.reaches[0].1 * (0.8 + 0.2 * u);
        }
        // The shores close at the head.
        let closing = self.closing();
        if s > closing {
            let u = ((s - closing) / (self.head - closing)).min(1.);
            half *= (u * FRAC_PI_2).cos().powi(2);
        }
        half
    }
    /// Whether `s` lies where the shores close in to a narrows. Islands keep clear of
    /// it, so a narrows is never narrower than its shores make it.
    fn near_narrows(&self, s: f32) -> bool {
        self.reaches[..self.narrows.len()]
            .iter()
            .any(|&(end, _)| (s - end).abs() < PINCH)
    }
    /// Where the shores begin to close in on the head.
    fn closing(&self) -> f32 {
        self.coves.last().map_or(self.head, |cove| cove.0) + CLOSING
    }
    /// Distance across the channel to one shore.
    fn t(&self, s: f32, side: f32) -> f32 {
        let (amplitude, wavelength, phase) = self.swells[usize::from(side > 0.)];
        let swell = amplitude * (TAU * s / wavelength + phase).sin();
        let coves = self.coves.iter().filter(|cove| cove.3 == side).fold(
            0.,
            |depth, &(at, deep, half, _)| {
                let u = (s - at) / half;
                if u.abs() < 1. {
                    depth + deep * (u * FRAC_PI_2).cos().powi(2)
                } else {
                    depth
                }
            },
        );
        // Past the head the land runs on, its shores meeting at the middle.
        let open = if s > self.head {
            0.
        } else {
            (self.half(s) + swell).max(0.)
        };
        self.middle(s) + side * (open + coves)
    }
    fn point(&self, frame: &Frame, s: f32, side: f32) -> Vec2 {
        frame.at(s, self.t(s, side))
    }
    /// Out from one shore into the channel.
    fn normal(&self, frame: &Frame, s: f32, side: f32) -> Vec2 {
        let slope = (self.t(s + 2., side) - self.t(s - 2., side)) / 4.;
        ((frame.across - frame.along * slope) * -side).normalize()
    }
}

pub(super) struct Passage {
    pub gates: Vec<Vec2>,
    pub harbours: Vec<HarbourSite>,
    pub pickets: [Vec2; 4],
    pub coast: Vec<Islet>,
    pub islets: Vec<Islet>,
    pub set: Set,
    pub gyres: Vec<Gyre>,
}

/// One base's place in the channel: how far along it, which shore, its works and stage.
#[derive(Clone, Copy)]
struct Berth {
    s: f32,
    side: f32,
    works: u8,
    stage: u8,
}

pub(super) fn sketch(rng: &mut Rng, waters: &Waters) -> Passage {
    // The channel runs near a compass point, so the chart stays compact on the screen.
    let quarter = rng.below(4) as f32 * TAU / 4.;
    let along = direction(quarter + rng.range(-0.2, 0.2));
    let frame = Frame {
        along,
        across: along.perp(),
    };
    let (berths, reaches, narrows) = plan(rng, waters);
    let end = berths.last().map_or(0., |b| b.s);
    let mut channel = Channel {
        bends: [
            (
                waters.bends * rng.range(0.6, 0.85),
                rng.range(1500., 2200.),
                rng.range(0., TAU),
            ),
            (
                waters.bends * rng.range(0.15, 0.3),
                rng.range(600., 900.),
                rng.range(0., TAU),
            ),
        ],
        offset: 0.,
        reaches,
        narrows,
        swells: [0, 1].map(|_| {
            (
                waters.bays * rng.range(0.5, 1.),
                rng.range(380., 620.),
                rng.range(0., TAU),
            )
        }),
        coves: vec![],
        head: end + rng.range(HEAD.0, HEAD.1),
    };
    channel.offset = channel.middle(0.);
    channel.coves = berths
        .iter()
        .map(|b| {
            let depth = rng.range(COVES.0, COVES.1);
            (b.s, depth, rng.range(COVE_WIDTH.0, COVE_WIDTH.1), b.side)
        })
        .collect();
    let harbours = bases(rng, &frame, &channel, &berths);
    let gates = marks(rng, &frame, &channel, &berths, &harbours);
    let pickets = stations(&gates, frame.across);
    let keepout = Keepout::new(waters, Course::Passage, &gates, &harbours, &pickets);
    let coast = shores(rng, waters, &frame, &channel, &keepout);
    let islets = islands(rng, waters, &frame, &channel, &gates, &keepout, &coast, end);
    // Outside, the current sets along the coast and across the mouth, one way or the
    // other. It does not run into a channel closed at its head.
    let (slow, fast) = waters.drift;
    let set = Set {
        flow: frame.across * rng.sign() * rng.range(slow, fast),
        meander: rng.range(0.05, 0.15),
        wavelength: rng.range(600., 1100.),
        phase: rng.range(0., TAU),
    };
    // Inside, the water of every reach turns slowly the same way round, in along one
    // shore and out along the other: a chain of gyres down the middle of the channel.
    let turn = rng.sign() * set.flow.length() * GYRE_SHARE;
    let mut gyres = vec![];
    let mut s = 0.;
    while s < channel.closing() {
        let half = channel.half(s);
        let step = half.max(80.) * GYRE_SPACING;
        gyres.push(Gyre {
            centre: frame.at(s, channel.middle(s)),
            width: half,
            circulation: 2. * turn * step,
        });
        s += step;
    }
    Passage {
        gates,
        harbours,
        pickets,
        coast,
        islets,
        set,
        gyres,
    }
}

/// Lay the stages out along the channel: where each base stands and on which shore,
/// and where each reach ends with half its width, and half the width of each narrows.
fn plan(rng: &mut Rng, waters: &Waters) -> (Vec<Berth>, Vec<(f32, f32)>, Vec<f32>) {
    let mut berths = vec![];
    let mut reaches = vec![];
    let mut narrows = vec![];
    let mut start = 0.;
    let mut side = rng.sign();
    let last = waters.stages.len().saturating_sub(1);
    for (stage, works) in waters.stages.iter().enumerate() {
        let pair = works.len() > 1;
        let mut length = rng.range(waters.reach.0, waters.reach.1) * if pair { PAIRED } else { 1. };
        // Each base stands well clear of the narrows either end of its reach, and of the
        // other base of a pair.
        let shares: &[(f32, f32)] = if pair {
            &[(0.23, 0.28), (0.7, 0.76)]
        } else {
            &[(0.47, 0.55)]
        };
        let mut first = true;
        for (&works, &(low, high)) in works.iter().zip(shares) {
            // Mostly the bases alternate across the channel, and a stage's pair always does.
            if !first || rng.chance(0.7) {
                side = -side;
            }
            first = false;
            let mut s = start + length * rng.range(low, high);
            if stage == 0 && s < FIRST_BASE {
                let push = FIRST_BASE - s + rng.range(0., 40.);
                s += push;
                length += push;
            }
            berths.push(Berth {
                s,
                side,
                works,
                stage: stage as u8,
            });
        }
        // The last reach ends at the heavy yard, and the head closes beyond it.
        let end = if stage == last {
            berths.last().map_or(start + length, |b| b.s) + HEAD.1 + OVERHANG
        } else {
            start + length
        };
        reaches.push((end, rng.range(waters.breadth.0, waters.breadth.1) * 0.5));
        if stage != last {
            narrows.push(rng.range(waters.narrows.0, waters.narrows.1) * 0.5);
        }
        start = end;
    }
    (berths, reaches, narrows)
}

/// A base at the head of each cove, on an island off the shore with its slip opening
/// onto the channel.
fn bases(rng: &mut Rng, frame: &Frame, channel: &Channel, berths: &[Berth]) -> Vec<HarbourSite> {
    let last = berths.last().map_or(f32::NAN, |b| b.s);
    berths
        .iter()
        .map(|b| {
            let out = channel.normal(frame, b.s, b.side);
            let pos = channel.point(frame, b.s, b.side)
                + out * (ISLAND_RADIUS + rng.range(BACKWATER.0, BACKWATER.1));
            // The slip turns a little up or down the channel. At the head it opens back
            // down the channel, away from the closing shores.
            let slip = if b.s == last {
                (out - frame.along * rng.range(0.35, 0.75)).normalize()
            } else {
                Vec2::from_angle(rng.range(-0.45, 0.45)).rotate(out)
            };
            HarbourSite {
                pos,
                heading: heading(slip),
                works: b.works,
                stage: b.stage,
            }
        })
        .collect()
}

/// The marks: off each base in turn, and through each narrows on the way to the next
/// reach. The course ends off the heavy yard.
fn marks(
    rng: &mut Rng,
    frame: &Frame,
    channel: &Channel,
    berths: &[Berth],
    harbours: &[HarbourSite],
) -> Vec<Vec2> {
    let mut gates = vec![];
    let mut stage = 0;
    for (berth, harbour) in berths.iter().zip(harbours) {
        if berth.stage != stage {
            stage = berth.stage;
            let s = channel.reaches[usize::from(stage) - 1].0;
            let narrow = channel.narrows[usize::from(stage) - 1];
            let t = channel.middle(s) + narrow * rng.range(-0.15, 0.15);
            gates.push(frame.at(s, t));
        }
        // Straight across the channel from the base, so the marks keep their spacing
        // along it however the channel bends.
        let off = rng.range(OFF_BASE.0, OFF_BASE.1);
        gates.push(harbour.pos - frame.across * berth.side * off);
    }
    gates
}

/// A point on the course, `run` metres from the start.
fn along_course(gates: &[Vec2], mut run: f32) -> Vec2 {
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
fn stations(gates: &[Vec2], across: Vec2) -> [Vec2; 4] {
    STATIONS.map(|(run, off)| along_course(gates, run) + across * off)
}

/// The shore as built so far on one side: the distance across the channel at which the
/// first piece of that side's land is met coming from the middle.
fn edge(coast: &[Islet], frame: &Frame, s: f32, side: f32, fallback: f32) -> f32 {
    coast
        .iter()
        .filter_map(|piece| {
            let ds = s - frame.s(piece.pos);
            (ds.abs() < piece.radius)
                .then(|| frame.t(piece.pos) - side * (piece.radius * piece.radius - ds * ds).sqrt())
        })
        .filter(|&t| (t - fallback) * side > -400.)
        .reduce(|a, b| if (a - b) * side < 0. { a } else { b })
        .unwrap_or(fallback)
}

/// Pieces of the coast that stand out from the shore: crags and glaciers. Two of them
/// either run together or leave a channel wide enough to sail into.
fn apart(piece: &Islet, standing: &[Islet]) -> bool {
    standing.iter().all(|o| {
        let gap = piece.pos.distance(o.pos) - piece.radius - o.radius;
        gap <= -0.35 * piece.radius.min(o.radius) || gap >= STRAIT + 5.
    })
}

/// Both shores: broad hills whose seaward faces make each, with mountains behind, a
/// glacier coming down to the water at some of the narrows and another at the head,
/// and crags standing out from the cliffs.
fn shores(
    rng: &mut Rng,
    waters: &Waters,
    frame: &Frame,
    channel: &Channel,
    keepout: &Keepout,
) -> Vec<Islet> {
    let mut coast: Vec<Islet> = vec![];
    let piece = |rng: &mut Rng, pos: Vec2, radius: f32| Islet {
        pos,
        radius,
        kind: LandmarkKind::Mainland,
        heading: rng.range(0., TAU),
    };
    let shore = |s: f32, side: f32| channel.t(s, side);
    let far = channel.head + OVERHANG;
    for side in [-1., 1.] {
        // Hills whose faces make the shore. A hill that would crowd the fairway or a
        // base stands farther back, which opens a bay round it.
        let mut s = -SEAWARD;
        while s < far {
            let radius = rng.range(HILLS.0, HILLS.1);
            let mut t = shore(s, side) + side * (radius + rng.range(-5., 5.));
            let mut hill = piece(rng, frame.at(s, t), radius);
            for _ in 0..60 {
                if keepout.allows_shore(&hill, SHORE_ROOM) {
                    coast.push(hill);
                    break;
                }
                t += side * 5.;
                hill.pos = frame.at(s, t);
            }
            s += radius * rng.range(0.3, 0.5);
        }
        // Mountains behind, so the land runs on inland. Where the shores close in at the
        // head, they stand back from the heavy yard like the hills.
        let mut s = -SEAWARD;
        while s < far {
            let radius = rng.range(MOUNTAINS.0, MOUNTAINS.1);
            let mut t = shore(s, side) + side * (150. + radius);
            let mut mountain = piece(rng, frame.at(s, t), radius);
            for _ in 0..40 {
                if keepout.allows_shore(&mountain, SHORE_ROOM) {
                    coast.push(mountain);
                    break;
                }
                t += side * 10.;
                mountain.pos = frame.at(s, t);
            }
            s += radius * rng.range(0.6, 0.8);
        }
    }
    // The head is all land, and the mountains close round it.
    let mut t = -300.;
    while t <= 300. {
        let radius = rng.range(MOUNTAINS.0, MOUNTAINS.1);
        let mut s = channel.head + radius + rng.range(40., 120.);
        let mut mountain = piece(rng, frame.at(s, channel.middle(s) + t), radius);
        for _ in 0..40 {
            if keepout.allows_shore(&mountain, SHORE_ROOM) {
                coast.push(mountain);
                break;
            }
            s += 10.;
            mountain.pos = frame.at(s, channel.middle(s) + t);
        }
        t += radius * rng.range(0.6, 0.8);
    }
    let backbone = coast.len();
    let edge = |coast: &[Islet], s: f32, side: f32| {
        edge(&coast[..backbone], frame, s, side, shore(s, side))
    };

    let mut standing: Vec<Islet> = vec![];
    // A glacier comes down to the water at the head, behind the heavy yard, its front
    // facing down the channel.
    let (small, large) = waters.glacier_radius;
    let radius = rng.range(small, large) * 1.15;
    let facing = heading(Vec2::new(-frame.along.y, frame.along.x));
    let mut reach = radius * 0.5;
    while reach >= -radius * 0.3 {
        let s = channel.head - reach + radius;
        let front = Islet {
            pos: frame.at(s, channel.middle(s)),
            radius,
            kind: LandmarkKind::Glacier,
            heading: facing,
        };
        if keepout.allows_shore(&front, 105.) {
            standing.push(front);
            break;
        }
        reach -= 15.;
    }
    // Glaciers at some of the narrows, from one shore or the other. Which narrows get
    // one is shuffled.
    let mut gaps: Vec<f32> = channel.reaches[..channel.narrows.len()]
        .iter()
        .map(|&(end, _)| end)
        .collect();
    for i in (1..gaps.len()).rev() {
        gaps.swap(i, rng.below(i + 1));
    }
    let (fewest, most) = waters.glaciers;
    let glaciers = fewest + rng.below(most.saturating_sub(fewest) + 1);
    for &gap in gaps.iter().take(glaciers) {
        let side = rng.sign();
        let s = gap + rng.range(-40., 40.);
        let out = channel.normal(frame, s, side);
        let radius = rng.range(small, large);
        // The glacier's front lies to starboard of its heading, so it faces the water.
        let facing = heading(Vec2::new(out.y, -out.x));
        let foot = edge(&coast, s, side);
        let mut reach = radius * 0.35;
        while reach >= -radius * 0.2 {
            let jut = Islet {
                pos: frame.at(s, foot + side * (radius - reach)),
                radius,
                kind: LandmarkKind::Glacier,
                heading: facing,
            };
            if keepout.allows_shore(&jut, 105.) && apart(&jut, &standing) {
                standing.push(jut);
                break;
            }
            reach -= 12.;
        }
    }
    // Crags stand out from the cliffs all along both shores.
    let every = 1000. / waters.crags.max(0.1);
    for side in [-1., 1.] {
        let mut s = -SEAWARD * 0.7 + rng.range(0., every);
        while s < channel.head {
            let radius = CRAGS.0 + (CRAGS.1 - CRAGS.0) * rng.unit().powi(2);
            let out = edge(&coast, s, side) - side * radius * rng.range(-0.3, 0.55);
            let crag = piece(rng, frame.at(s, out), radius);
            if keepout.allows_shore(&crag, SHORE_ROOM) && apart(&crag, &standing) {
                standing.push(crag);
            }
            s += every * rng.range(0.6, 1.4);
        }
    }
    coast.extend(standing);
    coast
}

/// Islands in the channel: straits on the course, and groups and lone islands between
/// the course and the shores.
#[allow(clippy::too_many_arguments)]
fn islands(
    rng: &mut Rng,
    waters: &Waters,
    frame: &Frame,
    channel: &Channel,
    gates: &[Vec2],
    keepout: &Keepout,
    coast: &[Islet],
    end: f32,
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
    // Straits: an island either side of a leg, so the course runs between them.
    let legs = legs(Course::Passage, gates);
    for _ in 0..waters.straits {
        for _ in 0..8 {
            let (a, b) = legs[rng.below(legs.len())];
            if a.distance(b) < 280. {
                continue;
            }
            let at = a.lerp(b, rng.range(0.3, 0.7));
            if channel.near_narrows(frame.s(at)) {
                continue;
            }
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
    // Groups and lone islands fill the water between the shores.
    let mut tries = 0;
    while placed.len() < target && tries < 400 {
        tries += 1;
        let s = rng.range(-150., end + 100.);
        let (low, high) = (channel.t(s, -1.), channel.t(s, 1.));
        if high - low < 160. {
            continue;
        }
        let around = frame.at(s, rng.range(low + 50., high - 50.));
        if rng.chance(waters.clustering) {
            for _ in 0..2 + rng.below(3) {
                let at = around + direction(rng.range(0., TAU)) * rng.range(0., 110.);
                let islet = raise(rng, at);
                if placed.len() < target
                    && !channel.near_narrows(frame.s(islet.pos))
                    && keepout.allows(&islet, &placed, false)
                {
                    placed.push(islet);
                }
            }
        } else {
            let stray = rng.chance(waters.strays);
            let islet = raise(rng, around);
            if !channel.near_narrows(frame.s(islet.pos)) && keepout.allows(&islet, &placed, stray) {
                placed.push(islet);
            }
        }
    }
    placed.split_off(first)
}
