//! The current: a steady stream across the sector that bends round every island, runs
//! fast through straits and past headlands, and turns back on itself in eddies behind
//! the islands.
//!
//! The flow comes from a stream function solved on a grid, so it has no divergence:
//! drifting ice neither piles up nor thins out in open water, and no water crosses a
//! coast. Each island holds one streamline as its shore and carries no net circulation,
//! so the stream parts round it and closes again behind it, as it would in nature. A
//! coast that runs off the chart holds the streamline it meets the open sea on there, so
//! whatever turns in the water it encloses turns along its shore. A
//! pair of opposed vortices in each island's lee makes the slack water and back eddy
//! seen downstream of real islands. In a channel closed at its head the set reaches only
//! a little way in, so the chart can turn the water of each reach slowly round on
//! itself with gyres, in along one shore and out along the other.
use super::Vec2;
use std::f32::consts::TAU;

/// Grid spacing of the finest solve, in metres. Shores fall between nodes at their true
/// distance, so even the narrowest strait the charts allow spans four cells.
const CELL: f32 = 10.;
/// Open water solved beyond the outermost feature, so the frame sees an undisturbed set.
pub const MARGIN: f32 = 220.;
/// Fastest water anywhere on a chart, in metres per second. The slowest flagship makes
/// 2.7 m/s through the water, so she stems the strongest stream with way to spare.
pub const MAX_CURRENT: f32 = 1.25;
/// Most over-relaxation used. Each level starts from the one coarser, so what is left to
/// settle is local to the shores, which heavier over-relaxation only sets ringing.
const OMEGA_CAP: f32 = 1.7;
/// Islands smaller than this shed no eddies worth solving for.
const EDDY_RADIUS: f32 = 15.;
/// Circulation of each lee vortex per unit of stream speed and island radius.
const EDDY_STRENGTH: f32 = 1.7;

/// The current far from land.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Set {
    /// Velocity of the open-water stream, in metres per second.
    pub flow: Vec2,
    /// How far the stream swings from side to side, as a share of its speed.
    pub meander: f32,
    /// Metres along the stream from one swing to the next.
    pub wavelength: f32,
    /// Where along the stream the swing starts, in radians.
    pub phase: f32,
}
impl Set {
    /// Still water.
    pub const SLACK: Self = Self {
        flow: Vec2::ZERO,
        meander: 0.,
        wavelength: 1.,
        phase: 0.,
    };

    fn wave(&self, p: Vec2) -> (f32, Vec2) {
        let along = self.flow.normalize_or_zero();
        let k = TAU / self.wavelength.max(1.);
        (k * along.dot(p) + self.phase, along)
    }
    /// Stream function: velocity is (∂ψ/∂y, −∂ψ/∂x).
    fn stream(&self, p: Vec2) -> f32 {
        let (angle, _) = self.wave(p);
        let swing = self.meander * self.flow.length() * self.wavelength.max(1.) / TAU;
        self.flow.x * p.y - self.flow.y * p.x + swing * angle.sin()
    }
    /// Velocity of the open-water stream at a point.
    pub fn velocity(&self, p: Vec2) -> Vec2 {
        let (angle, along) = self.wave(p);
        // The swing runs across the stream, so it carries no water along it.
        self.flow + Vec2::new(along.y, -along.x) * self.meander * self.flow.length() * angle.cos()
    }
}

/// A slow turning of the water over a stretch of enclosed channel, as the tide and the
/// melt off the ice keep it going: in along one shore and out along the other. A chain
/// of them down the middle of a channel turns it as one, and the water along each shore
/// runs at about half their circulation over the spacing between them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gyre {
    pub centre: Vec2,
    /// Metres across the turning water, about half the channel's width.
    pub width: f32,
    /// Square metres a second. Its sign gives the way it turns.
    pub circulation: f32,
}
impl Gyre {
    fn eddy(&self) -> Eddy {
        Eddy {
            centre: self.centre,
            width: self.width * 0.5,
            circulation: self.circulation,
        }
    }
}

/// A vortex spread over a small patch of water, for the eddies behind an island.
#[derive(Clone, Copy)]
struct Eddy {
    centre: Vec2,
    width: f32,
    circulation: f32,
}
impl Eddy {
    /// The water it turns at a point, in open water with no shore to bound it.
    fn velocity(&self, p: Vec2) -> Vec2 {
        let out = p - self.centre;
        let d2 = out.length_squared().max(1e-6);
        let held = 1. - (-d2 / (2. * self.width * self.width)).exp();
        // Velocity is (∂ψ/∂y, −∂ψ/∂x) and the vorticity is −∇²ψ, so a positive
        // circulation turns the water from +x toward +y.
        Vec2::new(out.y, -out.x) * (-self.circulation * held / (TAU * d2))
    }
    fn vorticity(&self, p: Vec2) -> f32 {
        let s2 = self.width * self.width;
        let d2 = p.distance_squared(self.centre);
        // Past four widths the vortex has spent all but a trace of itself.
        if d2 > 16. * s2 {
            return 0.;
        }
        self.circulation / (TAU * s2) * (-d2 / (2. * s2)).exp()
    }
}

/// The solved current over one chart.
pub struct Currents {
    set: Set,
    origin: Vec2,
    cell: f32,
    cols: usize,
    rows: usize,
    flow: Vec<Vec2>,
    peak: f32,
}
impl Currents {
    /// No current at all.
    pub fn slack() -> Self {
        Self {
            set: Set::SLACK,
            origin: Vec2::ZERO,
            cell: 1.,
            cols: 0,
            rows: 0,
            flow: vec![],
            peak: 0.,
        }
    }

    /// Solve the stream round circular islands of land, given as centre and radius, over
    /// the water from `lo` to `hi` and a margin beyond, with any gyres turning in it.
    pub fn solve(set: Set, land: &[(Vec2, f32)], gyres: &[Gyre], lo: Vec2, hi: Vec2) -> Self {
        if set.flow.length_squared() < 1e-8 {
            return Self::slack();
        }
        let charted = (lo, hi);
        let (lo, hi) = (lo - MARGIN, hi + MARGIN);
        // An island sheds its eddies into the stream that passes it: the set, or in a
        // channel the turning of its gyres, which the set does not reach.
        let turning: Vec<Eddy> = gyres.iter().map(Gyre::eddy).collect();
        let stream = |p: Vec2| {
            if turning.is_empty() {
                set.velocity(p)
            } else {
                turning.iter().map(|eddy| eddy.velocity(p)).sum()
            }
        };
        let mut eddies = lee_eddies(stream, land);
        eddies.extend(turning);
        let mut solved: Option<Level> = None;
        // Coarse levels settle the broad stream cheaply. The finest only sharpens it.
        for (cell, sweeps) in [(CELL * 4., 240), (CELL * 2., 90), (CELL, 36)] {
            let mut level = Level::new(&set, land, &eddies, lo, hi, cell);
            if let Some(coarse) = &solved {
                level.start_from(coarse);
            }
            level.relax(sweeps, set.flow.length());
            solved = Some(level);
        }
        let level = solved.expect("three levels were solved");
        let mut flow = level.velocities(&set);
        // Narrow straits can run hard. Ease the fastest water toward the limit smoothly,
        // leaving the gentler stream as solved. The peak is taken over the charted water,
        // not the margin, where a coast runs out of the grid.
        let knee = MAX_CURRENT * 0.7;
        let mut peak: f32 = 0.;
        for (n, v) in flow.iter_mut().enumerate() {
            let speed = v.length();
            if speed > knee {
                let room = MAX_CURRENT - knee;
                *v *= (knee + room * ((speed - knee) / room).tanh()) / speed;
            }
            if level.node(n).cmpge(charted.0).all() && level.node(n).cmple(charted.1).all() {
                peak = peak.max(v.length());
            }
        }
        Self {
            set,
            origin: level.origin,
            cell: level.cell,
            cols: level.cols,
            rows: level.rows,
            flow,
            peak,
        }
    }

    /// The open-water set this field was solved for.
    pub fn set(&self) -> Set {
        self.set
    }
    /// Fastest water on the chart, in metres per second.
    pub fn peak(&self) -> f32 {
        self.peak
    }
    /// Velocity of the water at a point, in metres per second.
    pub fn at(&self, p: Vec2) -> Vec2 {
        if self.flow.is_empty() {
            return Vec2::ZERO;
        }
        let g = (p - self.origin) / self.cell;
        let (i, j) = (g.x.floor(), g.y.floor());
        if i < 0. || j < 0. || i >= (self.cols - 1) as f32 || j >= (self.rows - 1) as f32 {
            return self.set.velocity(p);
        }
        let (i, j) = (i as usize, j as usize);
        let (fx, fy) = (g.x - i as f32, g.y - j as f32);
        let n = j * self.cols + i;
        let top = self.flow[n].lerp(self.flow[n + 1], fx);
        let bottom = self.flow[n + self.cols].lerp(self.flow[n + self.cols + 1], fx);
        top.lerp(bottom, fy)
    }
    /// How fast the water turns at a point, in radians per second: half its vorticity,
    /// positive clockwise on the chart, as headings run.
    pub fn spin(&self, p: Vec2) -> f32 {
        let h = self.cell;
        let (dx, dy) = (Vec2::X * h, Vec2::Y * h);
        let curl =
            (self.at(p + dx).y - self.at(p - dx).y) - (self.at(p + dy).x - self.at(p - dy).x);
        0.5 * curl / (2. * h)
    }
}

/// A vortex pair behind each island of any size that stands alone, turning so the water
/// between them runs back toward the shore. Land that runs into other land, such as the
/// pieces of a coast, sheds no eddies of its own.
fn lee_eddies(stream: impl Fn(Vec2) -> Vec2, land: &[(Vec2, f32)]) -> Vec<Eddy> {
    let mut eddies = vec![];
    let alone = |k: usize| {
        let (centre, radius) = land[k];
        land.iter()
            .enumerate()
            .all(|(j, &(c, r))| j == k || centre.distance(c) >= radius + r)
    };
    for &(centre, radius) in (0..land.len())
        .filter(|&k| land[k].1 >= EDDY_RADIUS && alone(k))
        .map(|k| &land[k])
    {
        let stream = stream(centre);
        let speed = stream.length();
        if speed < 1e-4 {
            continue;
        }
        let down = stream / speed.max(1e-6);
        let lee = centre + down * radius * 1.45;
        let circulation = EDDY_STRENGTH * speed * radius;
        for side in [-1., 1.] {
            eddies.push(Eddy {
                centre: lee + down.perp() * side * radius * 0.6,
                width: radius * 0.42,
                circulation: -side * circulation,
            });
        }
    }
    eddies
}

/// The frame of the grid holds the open-water set.
const FRAME: u32 = u32::MAX;
const WATER: u32 = u32::MAX - 1;
const INLAND: u32 = u32::MAX;
/// Shortest arm kept between a node and the shore, as a share of a cell, so a node
/// almost on the beach cannot swamp its stencil.
const SHORTEST_ARM: f32 = 0.05;
/// The four arms of a node's stencil: east, west, south and north.
const ARMS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// An arm of a node's stencil that meets the shore before the next node.
#[derive(Clone, Copy)]
struct Landfall {
    /// Share of a cell from the node to the shore.
    arm: f32,
    mass: u32,
    /// The open-water stream function where the arm meets the shore.
    open: f32,
}

/// Land joined into one piece. Its whole shore is one streamline.
struct Mass {
    /// The stream function along the shore.
    level: f32,
    /// The stream function where land running off the grid meets the open sea at its
    /// edge, which holds the shore there. Land wholly on the grid takes the level that
    /// leaves no net circulation round it.
    anchor: Option<f32>,
    nodes: Vec<usize>,
    /// Water nodes beside the shore, each weighted by how close it stands.
    rim: Vec<(usize, f32)>,
}

/// One resolution of the stream function solve. `phi` is the departure from the
/// open-water stream function, so it is zero on the frame. Nodes beside a shore use the
/// true distance to the coast along each arm (the Shortley–Weller stencil), so the
/// islands keep their real size however coarse the grid.
struct Level {
    origin: Vec2,
    cell: f32,
    cols: usize,
    rows: usize,
    phi: Vec<f32>,
    /// The open-water stream function at each node.
    open: Vec<f32>,
    /// Cell area times vorticity at each node of water.
    source: Vec<f32>,
    /// `WATER`, `FRAME`, or the landmass a node lies on.
    kind: Vec<u32>,
    /// For each node beside a shore, where its arms reach land, or `None` inland.
    landfalls: Vec<[Option<Landfall>; 4]>,
    /// Index into `landfalls` of each node, or `INLAND` for a node with all four arms in water.
    shore_of: Vec<u32>,
    masses: Vec<Mass>,
}
impl Level {
    fn new(
        set: &Set,
        land: &[(Vec2, f32)],
        eddies: &[Eddy],
        lo: Vec2,
        hi: Vec2,
        cell: f32,
    ) -> Self {
        let cols = ((hi.x - lo.x) / cell).ceil() as usize + 1;
        let rows = ((hi.y - lo.y) / cell).ceil() as usize + 1;
        let count = cols * rows;
        let node = |n: usize| lo + Vec2::new((n % cols) as f32, (n / cols) as f32) * cell;
        let mut kind = vec![WATER; count];
        let mut open = vec![0.; count];
        let mut source = vec![0.; count];
        let mut dry = vec![false; count];
        for n in 0..count {
            open[n] = set.stream(node(n));
            let (i, j) = (n % cols, n / cols);
            if i == 0 || j == 0 || i == cols - 1 || j == rows - 1 {
                kind[n] = FRAME;
            }
        }
        // The nodes within `reach` of a point, a square of them at a time. A coast of
        // many pieces is marked piece by piece, not node by node against every piece.
        let near = |centre: Vec2, reach: f32| {
            let low = ((centre - reach - lo) / cell).floor().max(Vec2::ZERO);
            let high = ((centre + reach - lo) / cell).ceil();
            let (i0, j0) = (low.x as usize, low.y as usize);
            let (i1, j1) = (
                (high.x.max(0.) as usize).min(cols - 1),
                (high.y.max(0.) as usize).min(rows - 1),
            );
            (j0..=j1).flat_map(move |j| (i0..=i1).map(move |i| j * cols + i))
        };
        for &(c, r) in land {
            for n in near(c, r) {
                if kind[n] != FRAME && node(n).distance_squared(c) < r * r {
                    dry[n] = true;
                }
            }
        }
        for eddy in eddies {
            for n in near(eddy.centre, eddy.width * 4.) {
                if kind[n] != FRAME && !dry[n] {
                    source[n] += cell * cell * eddy.vorticity(node(n));
                }
            }
        }
        let step = |n: usize, (di, dj): (i32, i32)| {
            (n as isize + di as isize + dj as isize * cols as isize) as usize
        };
        // The frame holds the open-water set, so its nodes are never dry, but land that
        // runs off the grid is found where it crosses the frame.
        let on_frame = |n: usize| {
            let (i, j) = (n % cols, n / cols);
            i == 0 || j == 0 || i == cols - 1 || j == rows - 1
        };
        let frame_land: Vec<bool> = (0..count)
            .map(|n| {
                on_frame(n)
                    && land
                        .iter()
                        .any(|&(c, r)| node(n).distance_squared(c) < r * r)
            })
            .collect();
        let along_frame = |n: usize| {
            let (i, j) = ((n % cols) as isize, (n / cols) as isize);
            ARMS.into_iter().filter_map(move |(di, dj)| {
                let (a, b) = (i + di as isize, j + dj as isize);
                (a >= 0 && b >= 0 && a < cols as isize && b < rows as isize)
                    .then(|| b as usize * cols + a as usize)
            })
        };
        // Water cut off from where the ship enters, behind a coast that runs off the
        // grid, is none of the sea she sails. It is taken for land, so it neither holds
        // a stream of its own nor gives a coast a false level where it meets the frame.
        let mut frame_land = frame_land;
        let entry = ((-lo) / cell).round();
        let inside =
            entry.cmpge(Vec2::ZERO).all() && (entry.x as usize) < cols && (entry.y as usize) < rows;
        let start = entry.y as usize * cols + entry.x as usize;
        if inside && !dry[start] && !frame_land[start] {
            let mut sea = vec![false; count];
            let mut stack = vec![start];
            sea[start] = true;
            while let Some(n) = stack.pop() {
                for m in along_frame(n) {
                    if !sea[m] && !dry[m] && !frame_land[m] {
                        sea[m] = true;
                        stack.push(m);
                    }
                }
            }
            for n in 0..count {
                if !sea[n] {
                    if on_frame(n) {
                        frame_land[n] = true;
                    } else {
                        dry[n] = true;
                    }
                }
            }
        }
        let mut walked = vec![false; count];
        // Touching islands share one shore streamline, so label land by connection.
        let mut masses = vec![];
        for start in 0..count {
            if !dry[start] || kind[start] != WATER {
                continue;
            }
            let label = masses.len() as u32;
            let mut nodes = vec![];
            let mut junctions = vec![];
            let mut stack = vec![start];
            kind[start] = label;
            while let Some(n) = stack.pop() {
                nodes.push(n);
                for arm in ARMS {
                    let m = step(n, arm);
                    if dry[m] && kind[m] == WATER {
                        kind[m] = label;
                        stack.push(m);
                    } else if frame_land[m] && !walked[m] {
                        // Walk the land along the frame to where it meets open water.
                        walked[m] = true;
                        let mut ahead = vec![m];
                        while let Some(f) = ahead.pop() {
                            for g in along_frame(f).filter(|&g| on_frame(g)) {
                                if !frame_land[g] {
                                    junctions.push(f);
                                } else if !walked[g] {
                                    walked[g] = true;
                                    ahead.push(g);
                                }
                            }
                        }
                    }
                }
            }
            let anchor = (!junctions.is_empty())
                .then(|| junctions.iter().map(|&f| open[f]).sum::<f32>() / junctions.len() as f32);
            masses.push(Mass {
                level: anchor.unwrap_or(0.),
                anchor,
                nodes,
                rim: vec![],
            });
        }
        let mut landfalls = vec![];
        let mut shore_of = vec![INLAND; count];
        for n in 0..count {
            if kind[n] != WATER {
                continue;
            }
            let p = node(n);
            let mut reach = [None; 4];
            for (k, arm) in ARMS.into_iter().enumerate() {
                let m = step(n, arm);
                if !dry[m] {
                    continue;
                }
                let dir = Vec2::new(arm.0 as f32, arm.1 as f32);
                let t = land
                    .iter()
                    .filter_map(|&(c, r)| shore_crossing(p, dir, c, r))
                    .fold(cell, f32::min);
                let landfall = Landfall {
                    arm: (t / cell).clamp(SHORTEST_ARM, 1.),
                    mass: kind[m],
                    open: set.stream(p + dir * t),
                };
                masses[landfall.mass as usize]
                    .rim
                    .push((n, 1. / landfall.arm));
                reach[k] = Some(landfall);
            }
            if reach.iter().any(Option::is_some) {
                shore_of[n] = landfalls.len() as u32;
                landfalls.push(reach);
            }
        }
        Self {
            origin: lo,
            cell,
            cols,
            rows,
            phi: vec![0.; count],
            open,
            source,
            kind,
            landfalls,
            shore_of,
            masses,
        }
    }

    fn node(&self, n: usize) -> Vec2 {
        self.origin + Vec2::new((n % self.cols) as f32, (n / self.cols) as f32) * self.cell
    }

    fn sample(&self, p: Vec2) -> f32 {
        let g = ((p - self.origin) / self.cell).clamp(
            Vec2::ZERO,
            Vec2::new((self.cols - 1) as f32, (self.rows - 1) as f32) - 1e-3,
        );
        let (i, j) = (g.x as usize, g.y as usize);
        let (fx, fy) = (g.x - i as f32, g.y - j as f32);
        let n = j * self.cols + i;
        let top = self.phi[n] + (self.phi[n + 1] - self.phi[n]) * fx;
        let bottom =
            self.phi[n + self.cols] + (self.phi[n + self.cols + 1] - self.phi[n + self.cols]) * fx;
        top + (bottom - top) * fy
    }

    /// Begin from a coarser solution over the same water.
    fn start_from(&mut self, coarse: &Level) {
        for n in 0..self.phi.len() {
            if self.kind[n] != FRAME {
                self.phi[n] = coarse.sample(self.node(n));
            }
        }
    }

    /// The departure at each end of a node's arms, and the arms' lengths in cells.
    fn arms(&self, n: usize) -> ([f32; 4], [f32; 4]) {
        let cols = self.cols as isize;
        let reach = (self.shore_of[n] != INLAND).then(|| self.landfalls[self.shore_of[n] as usize]);
        let mut value = [0.; 4];
        let mut length = [1.; 4];
        for (k, (di, dj)) in ARMS.into_iter().enumerate() {
            match reach.and_then(|r| r[k]) {
                Some(l) => {
                    value[k] = self.masses[l.mass as usize].level - l.open;
                    length[k] = l.arm;
                }
                None => {
                    value[k] = self.phi[(n as isize + di as isize + dj as isize * cols) as usize]
                }
            }
        }
        (value, length)
    }

    /// Successive over-relaxation. Before each sweep every landmass on the grid takes the
    /// shore value that leaves no net circulation round it.
    fn relax(&mut self, sweeps: usize, speed: f32) {
        let side = self.cols.max(self.rows) as f32;
        let omega = (2. / (1. + std::f32::consts::PI / side)).min(OMEGA_CAP);
        let settled = 2e-5 * speed * self.cell;
        let cols = self.cols;
        for _ in 0..sweeps {
            for mass in self.masses.iter_mut().filter(|mass| mass.anchor.is_none()) {
                let (sum, weight) = mass.rim.iter().fold((0., 0.), |(s, w), &(m, k)| {
                    (s + k * (self.phi[m] + self.open[m]), w + k)
                });
                if weight > 0. {
                    mass.level = sum / weight;
                }
            }
            let mut change: f32 = 0.;
            for j in 1..self.rows - 1 {
                for i in 1..cols - 1 {
                    let n = j * cols + i;
                    if self.kind[n] != WATER {
                        continue;
                    }
                    let target = if self.shore_of[n] == INLAND {
                        let phi = &self.phi;
                        (phi[n - 1] + phi[n + 1] + phi[n - cols] + phi[n + cols] + self.source[n])
                            * 0.25
                    } else {
                        let (value, length) = self.arms(n);
                        let mut total = self.source[n];
                        let mut weight = 0.;
                        for (a, b) in [(0, 1), (2, 3)] {
                            let span = length[a] + length[b];
                            for k in [a, b] {
                                let w = 2. / (length[k] * span);
                                total += w * value[k];
                                weight += w;
                            }
                        }
                        total / weight
                    };
                    let step = target - self.phi[n];
                    self.phi[n] += omega * step;
                    change = change.max(step.abs());
                }
            }
            if change < settled {
                break;
            }
        }
        for mass in &self.masses {
            for &m in &mass.nodes {
                self.phi[m] = mass.level - self.open[m];
            }
        }
    }

    /// Velocity at every node: the open-water set plus the solved departure. Land is still.
    fn velocities(&self, set: &Set) -> Vec<Vec2> {
        (0..self.phi.len())
            .map(|n| {
                let p = self.node(n);
                match self.kind[n] {
                    FRAME => set.velocity(p),
                    WATER => {
                        let (value, length) = self.arms(n);
                        let here = self.phi[n];
                        // Centred differences on uneven arms.
                        let slope = |ahead: usize, behind: usize| {
                            let (a, b) = (length[ahead], length[behind]);
                            (b * b * (value[ahead] - here) + a * a * (here - value[behind]))
                                / (a * b * (a + b) * self.cell)
                        };
                        set.velocity(p) + Vec2::new(slope(2, 3), -slope(0, 1))
                    }
                    _ => Vec2::ZERO,
                }
            })
            .collect()
    }
}

/// How far along a ray from water the shore of a circular island lies, if it does.
fn shore_crossing(from: Vec2, dir: Vec2, centre: Vec2, radius: f32) -> Option<f32> {
    let offset = from - centre;
    let b = offset.dot(dir);
    let c = offset.length_squared() - radius * radius;
    let disc = b * b - c;
    if c < 0. || disc < 0. {
        return None;
    }
    let t = -b - disc.sqrt();
    (t >= 0.).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EAST: Set = Set {
        flow: Vec2::new(0.5, 0.),
        meander: 0.,
        wavelength: 600.,
        phase: 0.,
    };

    fn field(land: &[(Vec2, f32)]) -> Currents {
        Currents::solve(EAST, land, &[], Vec2::splat(-300.), Vec2::splat(300.))
    }

    #[test]
    fn slack_water_and_open_water_hold_the_set() {
        let still = Currents::slack();
        assert_eq!(still.at(Vec2::new(40., -90.)), Vec2::ZERO);
        let open = field(&[]);
        for p in [Vec2::ZERO, Vec2::new(250., -120.), Vec2::new(-2000., 50.)] {
            assert!(open.at(p).distance(EAST.flow) < 0.01, "{p}: {}", open.at(p));
        }
        // A meander swings the stream across itself and back.
        let swinging = Set {
            meander: 0.4,
            ..EAST
        };
        let lateral: Vec<f32> = (0..6)
            .map(|k| swinging.velocity(Vec2::X * k as f32 * 100.).y)
            .collect();
        assert!(lateral.iter().any(|&v| v > 0.15) && lateral.iter().any(|&v| v < -0.15));
        assert!(lateral.iter().all(|&v| v.abs() <= 0.2 + 1e-4));
    }

    #[test]
    fn the_stream_parts_round_an_island_and_slackens_in_its_lee() {
        let (centre, radius) = (Vec2::ZERO, 60.);
        let f = field(&[(centre, radius)]);
        assert_eq!(f.at(centre), Vec2::ZERO);
        // Along the shore the water runs past, not onto the land, even where it meets
        // the island head on.
        for k in 0..16 {
            let out = Vec2::from_angle(k as f32 * TAU / 16.);
            let v = f.at(centre + out * (radius + 4.));
            // The grid steps round the shore, which bends the stream by a few degrees.
            assert!(
                v.dot(out).abs() < 0.08 + 0.2 * v.length(),
                "bearing {k}: {v} crosses the shore"
            );
        }
        // Faster round the flanks than in open water, slack astern of the island.
        let flank = f.at(centre + Vec2::Y * (radius + 12.)).length();
        let lee = f.at(centre + Vec2::X * radius * 1.5);
        let meeting = f.at(centre - Vec2::X * radius * 1.5);
        assert!(flank > 0.75, "flank {flank}");
        assert!(lee.length() < 0.1, "lee {lee}");
        // The eddies widen the slack astern beyond the still water where the stream
        // meets the island. They stop short of a closed back eddy, which would trap ice.
        assert!(
            meeting.length() > 2. * lee.length(),
            "meeting {meeting}, lee {lee}"
        );
        assert!(
            (0..40).all(|k| f.at(centre + Vec2::X * (radius + 4. * k as f32)).x >= -0.01),
            "a back eddy"
        );
    }

    #[test]
    fn a_strait_runs_faster_than_the_open_sea() {
        let half = 25.;
        let f = field(&[
            (Vec2::new(0., -(half + 45.)), 45.),
            (Vec2::new(0., half + 45.), 45.),
        ]);
        let gap = f.at(Vec2::ZERO);
        assert!(gap.x > 0.75, "strait {gap}");
        assert!(f.peak() <= MAX_CURRENT);
    }

    #[test]
    fn the_flow_neither_gathers_nor_spreads() {
        let f = field(&[(Vec2::new(-60., 30.), 50.), (Vec2::new(90., -40.), 30.)]);
        let h = 4.;
        let mut worst: f32 = 0.;
        for k in 0..400 {
            let p = Vec2::new((k % 20) as f32 * 30. - 280., (k / 20) as f32 * 30. - 280.);
            if [(Vec2::new(-60., 30.), 50.), (Vec2::new(90., -40.), 30.)]
                .iter()
                .any(|&(c, r)| p.distance(c) < r + 2. * CELL)
            {
                continue;
            }
            let div = (f.at(p + Vec2::X * h).x - f.at(p - Vec2::X * h).x + f.at(p + Vec2::Y * h).y
                - f.at(p - Vec2::Y * h).y)
                / (2. * h);
            worst = worst.max(div.abs());
        }
        // Against a shear of order speed over radius, about 0.01 per second.
        assert!(worst < 2e-3, "divergence {worst}");
    }
}
