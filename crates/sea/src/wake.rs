//! Water the ships have disturbed, on a patch that follows the player.
//!
//! Both grids are anchored to the chart and wrap: a cell stands for every point a
//! whole number of spans away, so nothing is copied as the patch moves. Only the
//! window around the focus is live, and anything leaving it is damped out before
//! it can come round again.
//!
//! Waves cross the patch at the speed deep water gives each wavelength, long
//! waves outrunning short ones. That is what turns a moving hull's disturbance
//! into a V and a splash into spreading rings. Each step transforms the patch
//! to wavenumbers, turns every one through its own angle, and transforms back.
use crate::fft::Grid;
use glam::Vec2;
use shipvivor_compass::direction;
use std::f32::consts::TAU;

/// Metres across one cell of either grid.
pub const CELL: f32 = 0.75;
/// Cells across the wave patch: 192 metres.
pub const SIDE: usize = 256;
/// Cells across the patch that holds foam and slick: 384 metres. Both outlast the waves.
pub const FOAM_SIDE: usize = 512;
/// Foam above this covers the water completely. The foam texels store up to this value.
pub const FOAM_WHITE: f32 = 1.5;

/// The ships are small for their speed. Stronger gravity shortens the waves a
/// hull makes to suit its length, as it does the fall of shot.
const GRAVITY: f32 = 9.81 * 2.5;
/// Loss per second shared by every wave, and the extra per squared radian per metre.
/// Ripples die within a few seconds and a long wave lasts to the edge of the patch.
const CALM: f32 = 0.04;
const VISCOSITY: f32 = 0.07;
/// Seconds for foam to fade to about a third, and for the slick a hull leaves.
const FOAM_LIFE: f32 = 4.5;
const SLICK_LIFE: f32 = 14.;
/// The slick is this many times the width of the hull, and is complete at this speed.
const SLICK_SPREAD: f32 = 1.7;
const SLICK_SPEED: f32 = 4.;
/// Fraction of the distance to the patch edge where waves start to be damped, and where none remain.
const SHORE: (f32, f32) = (0.72, 0.98);
/// The same for where the surface is drawn and read. Wrapped copies begin just beyond the patch edge.
const SHOWN: (f32, f32) = (0.74, 0.95);
/// Fraction of the way forward from the stern over which the keel rises to the
/// waterline. Water let go all at once would spring back as a spike astern.
const RUN: f32 = 0.45;
/// Metres over which a hull's edge is blurred. Sharper than a cell or two would ring.
const SOFT: f32 = 1.5;
/// The bow wave breaks along a hull's sides, in a band out to this many times
/// her beam, and leaves this much foam there per second at a metre per second.
/// It grows with the square of her speed.
const RIM_SPREAD: f32 = 1.5;
const RIM: f32 = 0.011;
/// Foam the screws leave per metre of water let fall astern, at ten metres per
/// second. It is thin beside the rims, so the middle of a wake soon shows the slick.
const WASH: f32 = 0.3;
/// Height a wave must reach to break, and the foam it then makes per metre per second.
const CREST: f32 = 0.55;
const SPILL: f32 = 1.6;

/// A hull in the water.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keel {
    /// Hulls are matched from one step to the next by this.
    pub id: u64,
    pub pos: Vec2,
    pub heading: f32,
    pub half_beam: f32,
    pub bow: f32,
    pub stern: f32,
    /// Metres of water the hull pushes down amidships.
    pub draught: f32,
}
impl Keel {
    /// Depth the hull presses the surface down at a point, given the way she is pointing.
    fn sink(&self, point: Vec2, ahead: Vec2) -> f32 {
        let offset = point - self.pos;
        let (along, across) = (offset.dot(ahead), offset.dot(ahead.perp()).abs());
        // The waterline tapers to a point at the stem and narrows a little to the transom.
        let width = if along > 0. {
            (1. - (along / self.bow).min(1.).powi(2)).powf(0.6)
        } else {
            1. - 0.5 * (along / self.stern).min(1.).powi(4)
        };
        self.draught
            * ramp(-0.5, 0.5, (width * self.half_beam - across) / SOFT)
            * ramp(0., 1., (self.bow - along) / SOFT)
            * ramp(0., 1., (along + self.stern) / (RUN * self.stern))
    }
    fn reach(&self) -> f32 {
        self.bow.max(self.stern) + SOFT
    }
}

pub struct Wake {
    grid: Grid,
    /// Height of the water in each cell.
    height: Vec<f32>,
    /// The motion of the water, as the height each wave will have a quarter of
    /// a period on. With `height` it is the two parts of one complex field.
    motion: Vec<f32>,
    /// The turn and loss for each wavenumber over one step of `stepped`: cosine, then sine.
    turn: Vec<[f32; 2]>,
    /// What survives a step at each distance in cells from the focus, along one axis.
    shore: Vec<f32>,
    stepped: f32,
    foam: Vec<f32>,
    /// Water a hull has churned smooth: 1 where no ripple has yet returned.
    slick: Vec<f32>,
    focus: Vec2,
    keels: Vec<Keel>,
}
impl Default for Wake {
    fn default() -> Self {
        Self {
            grid: Grid::new(SIDE),
            height: vec![0.; SIDE * SIDE],
            motion: vec![0.; SIDE * SIDE],
            turn: Vec::new(),
            shore: Vec::new(),
            stepped: 0.,
            foam: vec![0.; FOAM_SIDE * FOAM_SIDE],
            slick: vec![0.; FOAM_SIDE * FOAM_SIDE],
            focus: Vec2::ZERO,
            keels: Vec::new(),
        }
    }
}

fn ramp(from: f32, to: f32, x: f32) -> f32 {
    let t = ((x - from) / (to - from)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}
/// Cell containing a chart coordinate. Cell centres sit on whole multiples of `CELL`.
fn cell(x: f32) -> i32 {
    (x / CELL).round() as i32
}
fn wrap(cell: i32, side: usize) -> usize {
    cell.rem_euclid(side as i32) as usize
}
/// Distance from the focus cell along one axis, as a fraction of the way to the patch edge.
fn span(from: i32, to: i32, side: usize) -> f32 {
    (to - from).abs() as f32 / (side / 2) as f32
}

impl Wake {
    /// Still water, as at the start of a voyage.
    pub fn clear(&mut self) {
        self.height.fill(0.);
        self.motion.fill(0.);
        self.foam.fill(0.);
        self.slick.fill(0.);
        self.keels.clear();
    }

    fn prepare(&mut self, dt: f32) {
        if dt == self.stepped {
            return;
        }
        self.stepped = dt;
        let number = |i: usize| {
            let turns = if i < SIDE / 2 {
                i as f32
            } else {
                i as f32 - SIDE as f32
            };
            TAU * turns / (SIDE as f32 * CELL)
        };
        self.turn = (0..SIDE * SIDE)
            .map(|at| {
                let k = number(at % SIDE).hypot(number(at / SIDE));
                let (sin, cos) = ((GRAVITY * k).sqrt() * dt).sin_cos();
                // Both transforms are unnormalised, so the scale is folded in here.
                // The patch holds no standing level: the mean is dropped.
                let keep = if at == 0 {
                    0.
                } else {
                    (-dt * (CALM + VISCOSITY * k * k)).exp()
                };
                [cos, sin].map(|part| part * keep / (SIDE * SIDE) as f32)
            })
            .collect();
        self.shore = (0..=SIDE / 2)
            .map(|cells| {
                let out = ramp(SHORE.0, SHORE.1, cells as f32 / (SIDE / 2) as f32);
                (-dt * 8. * out).exp()
            })
            .collect();
    }

    /// Run the water on by `dt` seconds with the patch centred on `focus`.
    pub fn step(&mut self, dt: f32, focus: Vec2, keels: &[Keel]) {
        self.prepare(dt);
        self.focus = focus;
        let centre = (cell(focus.x), cell(focus.y));
        for keel in keels {
            if let Some(&before) = self.keels.iter().find(|old| old.id == keel.id) {
                self.shift(&before, keel, dt, centre);
            }
        }
        self.keels.clear();
        self.keels.extend_from_slice(keels);

        let (height, motion) = (&mut self.height[..], &mut self.motion[..]);
        self.grid.transform(height, motion, false);
        for ((height, motion), [cos, sin]) in
            height.iter_mut().zip(motion.iter_mut()).zip(&self.turn)
        {
            (*height, *motion) = (*height * cos + *motion * sin, *motion * cos - *height * sin);
        }
        self.grid.transform(height, motion, true);

        // For each wave cell along an axis: what survives, and its foam cell.
        let axis = |centre: i32| -> Vec<(f32, usize)> {
            (0..SIDE as i32)
                .map(|at| {
                    let away =
                        (at - centre + SIDE as i32 / 2).rem_euclid(SIDE as i32) - SIDE as i32 / 2;
                    (
                        self.shore[away.unsigned_abs() as usize],
                        wrap(centre + away, FOAM_SIDE),
                    )
                })
                .collect()
        };
        let (east, south) = (axis(centre.0), axis(centre.1));
        let rows = height
            .as_chunks_mut::<SIDE>()
            .0
            .iter_mut()
            .zip(motion.as_chunks_mut::<SIDE>().0);
        for ((height, motion), &(keep_row, foam_row)) in rows.zip(&south) {
            for ((height, motion), &(keep, foam)) in height.iter_mut().zip(motion).zip(&east) {
                *height *= keep * keep_row;
                *motion *= keep * keep_row;
                if *height > CREST {
                    // Water a hull has just churned flat does not break again.
                    let at = foam_row * FOAM_SIDE + foam;
                    let spilt = SPILL * (*height - CREST) * (1. - self.slick[at]) * dt;
                    self.foam[at] = (self.foam[at] + spilt).min(FOAM_WHITE);
                }
            }
        }

        let fade = (-dt / FOAM_LIFE).exp();
        let settle = (-dt / SLICK_LIFE).exp();
        let edge = |centre: i32| -> Vec<f32> {
            (0..FOAM_SIDE as i32)
                .map(|at| {
                    let away = (at - centre + FOAM_SIDE as i32 / 2).rem_euclid(FOAM_SIDE as i32)
                        - FOAM_SIDE as i32 / 2;
                    1. - ramp(SHOWN.0, SHOWN.1, away.abs() as f32 / (FOAM_SIDE / 2) as f32)
                })
                .collect()
        };
        let (east, south) = (edge(centre.0), edge(centre.1));
        let rows = self
            .foam
            .as_chunks_mut::<FOAM_SIDE>()
            .0
            .iter_mut()
            .zip(self.slick.as_chunks_mut::<FOAM_SIDE>().0);
        for ((foam, slick), keep_row) in rows.zip(south) {
            for ((foam, slick), keep) in foam.iter_mut().zip(slick).zip(&east) {
                *foam *= keep * keep_row * fade;
                *slick *= keep * keep_row * settle;
            }
        }
    }

    /// Move one hull. Water rises where the hull arrives and falls where it
    /// leaves. Under way she lays foam along both sides and a little astern, and
    /// leaves a smooth lane between.
    fn shift(&mut self, before: &Keel, after: &Keel, dt: f32, centre: (i32, i32)) {
        let way = after.pos.distance(before.pos) / dt;
        let wider = |spread: f32| Keel {
            half_beam: after.half_beam * spread,
            ..*after
        };
        let (rim, swept) = (wider(RIM_SPREAD), wider(SLICK_SPREAD));
        let draught = after.draught.max(1e-3);
        let calmed = (way / SLICK_SPEED).min(1.) / draught;
        let (was_ahead, ahead) = (direction(before.heading), direction(after.heading));
        let reach = Vec2::splat(before.reach().max(after.reach()));
        let low = (before.pos.min(after.pos) - reach) / CELL;
        let high = (before.pos.max(after.pos) + reach) / CELL;
        for south in low.y.floor() as i32..=high.y.ceil() as i32 {
            for east in low.x.floor() as i32..=high.x.ceil() as i32 {
                let point = Vec2::new(east as f32, south as f32) * CELL;
                let depth = after.sink(point, ahead);
                let moved = depth - before.sink(point, was_ahead);
                let out = span(centre.0, east, SIDE).max(span(centre.1, south, SIDE));
                if out < SHORE.0 {
                    let gate = 1. - ramp(SHORE.0 - 0.14, SHORE.0, out);
                    self.height[wrap(south, SIDE) * SIDE + wrap(east, SIDE)] += moved * gate;
                }
                if span(centre.0, east, FOAM_SIDE).max(span(centre.1, south, FOAM_SIDE)) < SHOWN.1 {
                    let at = wrap(south, FOAM_SIDE) * FOAM_SIDE + wrap(east, FOAM_SIDE);
                    let beside = (rim.sink(point, ahead) - depth).max(0.) / draught;
                    let churn = RIM * beside * way * way * dt + WASH * (-moved).max(0.) * way * 0.1;
                    self.foam[at] = (self.foam[at] + churn).min(FOAM_WHITE);
                    self.slick[at] = self.slick[at].max(swept.sink(point, ahead) * calmed);
                }
            }
        }
    }

    /// A shell or a wreck striking the water: a hollow with a raised rim, which
    /// springs back and spreads as rings, and a patch of white water over it.
    pub fn splash(&mut self, at: Vec2, radius: f32) {
        let centre = (cell(self.focus.x), cell(self.focus.y));
        let reach = (3. * radius / CELL).ceil() as i32;
        for south in cell(at.y) - reach..=cell(at.y) + reach {
            for east in cell(at.x) - reach..=cell(at.x) + reach {
                let point = Vec2::new(east as f32, south as f32) * CELL;
                let r2 = point.distance_squared(at) / (radius * radius);
                if span(centre.0, east, SIDE).max(span(centre.1, south, SIDE)) < SHORE.0 {
                    // As much water is thrown onto the rim as leaves the hollow.
                    self.height[wrap(south, SIDE) * SIDE + wrap(east, SIDE)] -=
                        0.5 * radius * (1. - r2) * (-r2).exp();
                }
                if span(centre.0, east, FOAM_SIDE).max(span(centre.1, south, FOAM_SIDE)) < SHOWN.1 {
                    let foam =
                        &mut self.foam[wrap(south, FOAM_SIDE) * FOAM_SIDE + wrap(east, FOAM_SIDE)];
                    *foam = (*foam + FOAM_WHITE * (-0.6 * r2).exp()).min(FOAM_WHITE);
                }
            }
        }
    }

    /// How much of the patch is shown at a point: 1 near the focus, 0 past the edge.
    fn shown(&self, point: Vec2, side: usize) -> f32 {
        let out = (point - self.focus).abs().max_element() / (side as f32 * CELL * 0.5);
        1. - ramp(SHOWN.0, SHOWN.1, out)
    }

    fn sample<T>(point: Vec2, side: usize, at: impl Fn(usize) -> T) -> f32
    where
        T: Into<f32>,
    {
        let grid = point / CELL;
        let base = grid.floor();
        let mix = grid - base;
        let (east, south) = (base.x as i32, base.y as i32);
        let read = |e: i32, s: i32| at(wrap(south + s, side) * side + wrap(east + e, side)).into();
        let north_row = read(0, 0) + (read(1, 0) - read(0, 0)) * mix.x;
        let south_row = read(0, 1) + (read(1, 1) - read(0, 1)) * mix.x;
        north_row + (south_row - north_row) * mix.y
    }

    /// Height the disturbed water adds to the swell at a point.
    pub fn height(&self, point: Vec2) -> f32 {
        Self::sample(point, SIDE, |at| self.height[at]) * self.shown(point, SIDE)
    }

    /// Foam lying at a point, from 0 to [`FOAM_WHITE`].
    pub fn foam(&self, point: Vec2) -> f32 {
        Self::sample(point, FOAM_SIDE, |at| self.foam[at]) * self.shown(point, FOAM_SIDE)
    }

    /// How far the ripples are smoothed away at a point, from 0 to 1.
    pub fn slick(&self, point: Vec2) -> f32 {
        Self::sample(point, FOAM_SIDE, |at| self.slick[at]) * self.shown(point, FOAM_SIDE)
    }

    /// Where the patch is centred, and the fractions of each half-span over which it fades out.
    pub fn window(&self) -> (Vec2, (f32, f32)) {
        (self.focus, SHOWN)
    }

    /// One texel per wave cell: height, then its slope to the east and to the
    /// south, then zero, as little-endian half floats.
    pub fn write_surface(&self, texels: &mut [u8]) {
        for (at, texel) in texels.as_chunks_mut::<8>().0.iter_mut().enumerate() {
            let (east, south) = (at % SIDE, at / SIDE);
            let height = |e: usize, s: usize| self.height[(s % SIDE) * SIDE + e % SIDE];
            let slope = |ahead: f32, behind: f32| (ahead - behind) / (2. * CELL);
            let values = [
                height(east, south),
                slope(height(east + 1, south), height(east + SIDE - 1, south)),
                slope(height(east, south + 1), height(east, south + SIDE - 1)),
                0.,
            ];
            for (bytes, value) in texel.as_chunks_mut::<2>().0.iter_mut().zip(values) {
                *bytes = half(value).to_le_bytes();
            }
        }
    }

    /// Two bytes per cell: foam, with 255 standing for [`FOAM_WHITE`], then slick.
    pub fn write_foam(&self, texels: &mut [u8]) {
        let cells = self.foam.iter().zip(&self.slick);
        for (texel, (foam, slick)) in texels.as_chunks_mut::<2>().0.iter_mut().zip(cells) {
            *texel = [(foam * (255. / FOAM_WHITE)) as u8, (slick * 255.) as u8];
        }
    }
}

/// The nearest half float toward zero. Heights here are far inside its range.
fn half(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = (bits >> 16) as u16 & 0x8000;
    let exponent = (bits >> 23 & 0xff) as i32 - 112;
    match exponent {
        ..=0 => sign,
        31.. => sign | 0x7bff,
        _ => sign | (exponent as u16) << 10 | (bits >> 13) as u16 & 0x3ff,
    }
}
