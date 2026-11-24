//! Flags drawn from a seed, for Breakwater and for whoever she is fighting.
//!
//! A flag starts as a colour scheme: two to four tinctures, each drawn to read against
//! the one before it. A layout of bands or quarters with room for every colour comes
//! next, and the scheme is laid into its regions so that no two regions sharing an edge
//! are too close in brightness to tell apart at sea.
//!
//! On the cloth, `u` runs from the hoist (0) to the fly (1) and `v` from the head (0) to
//! the foot (1). The cloth is [`ASPECT`] times as long as it is deep.

use std::fmt;

#[cfg(test)]
mod tests;

/// The cloth's length over its depth at the hoist. Cantons are proportioned in it.
pub const ASPECT: f32 = 2.8;

/// The least contrast two tinctures need to share an edge: the ratio of their relative
/// luminances, each plus 0.05, as WCAG measures text. Gold on white fails, and so does
/// red on green, which also fails for the colour-blind.
pub const MIN_CONTRAST: f32 = 2.2;

/// The least share of the cloth that must differ in colour between two sides' flags.
pub const MIN_DIFFERENCE: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Tincture {
    White,
    Gold,
    Scarlet,
    Navy,
    Green,
    Black,
    Orange,
    Sky,
    Purple,
}
use Tincture::*;

impl Tincture {
    pub const ALL: [Self; 9] = [
        White, Gold, Scarlet, Navy, Green, Black, Orange, Sky, Purple,
    ];

    /// Bright poster colours that sit with the game's paint.
    pub const fn srgb(self) -> [u8; 3] {
        match self {
            White => [244, 240, 226],
            Gold => [250, 190, 50],
            Scarlet => [214, 48, 36],
            Navy => [28, 52, 120],
            Green => [24, 120, 72],
            Black => [28, 28, 32],
            Orange => [240, 128, 30],
            Sky => [74, 148, 212],
            Purple => [110, 44, 120],
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            White => "white",
            Gold => "gold",
            Scarlet => "scarlet",
            Navy => "navy",
            Green => "green",
            Black => "black",
            Orange => "orange",
            Sky => "sky blue",
            Purple => "purple",
        }
    }

    /// Relative luminance, 0 for black to 1 for white.
    pub fn luminance(self) -> f32 {
        let [r, g, b] = self.srgb().map(to_linear);
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    pub fn contrast(self, other: Self) -> f32 {
        let (a, b) = (self.luminance(), other.luminance());
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// Whether the two may share an edge. A tincture never reads against itself.
    pub fn reads_against(self, other: Self) -> bool {
        self.contrast(other) >= MIN_CONTRAST
    }
}

impl fmt::Display for Tincture {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Layout {
    /// Horizontal bands of equal depth, counted from the head.
    Bands(u8),
    /// Vertical bands of equal length, counted from the hoist.
    Pales(u8),
    /// Quartered: upper hoist, upper fly, lower hoist and lower fly.
    Quarters,
    /// Cut corner to corner into four triangles: head, fly, foot and hoist.
    Saltire,
    /// A canton in the upper hoist, region 0, over this many bands from the head.
    Canton(u8),
}
use Layout::*;

impl Layout {
    pub const ALL: [Self; 12] = [
        Bands(2),
        Bands(3),
        Bands(4),
        Pales(2),
        Pales(3),
        Pales(4),
        Quarters,
        Saltire,
        Canton(1),
        Canton(2),
        Canton(3),
        Canton(4),
    ];

    pub fn regions(self) -> usize {
        match self {
            Bands(n) | Pales(n) => usize::from(n),
            Quarters | Saltire => 4,
            Canton(n) => usize::from(n) + 1,
        }
    }

    /// The region under a point on the cloth.
    pub fn region(self, u: f32, v: f32) -> usize {
        let band = |n: u8, t: f32| ((t * f32::from(n)) as usize).min(usize::from(n) - 1);
        match self {
            Bands(n) => band(n, v),
            Pales(n) => band(n, u),
            Quarters => 2 * usize::from(v >= 0.5) + usize::from(u >= 0.5),
            Saltire => match (v > u, v > 1. - u) {
                (false, false) => 0,
                (false, true) => 1,
                (true, true) => 2,
                (true, false) => 3,
            },
            Canton(n) => {
                let (depth, length) = self.canton();
                if u < length && v < depth {
                    0
                } else {
                    1 + band(n, v)
                }
            }
        }
    }

    /// The canton's depth down the hoist and its length along the head, as shares of the
    /// cloth. It reaches down to the foot of a band and is half as long again as it is deep.
    fn canton(self) -> (f32, f32) {
        let Canton(n) = self else {
            return (0., 0.);
        };
        let depth = if n < 2 {
            0.5
        } else {
            f32::from(n.div_ceil(2)) / f32::from(n)
        };
        (depth, 1.5 * depth / ASPECT)
    }

    /// Every pair of regions that share an edge, lower index first. Regions that meet
    /// only at a point, like opposite quarters, are not neighbours.
    pub fn borders(self) -> Vec<(usize, usize)> {
        match self {
            Bands(n) | Pales(n) => (1..usize::from(n)).map(|i| (i - 1, i)).collect(),
            Quarters => vec![(0, 1), (0, 2), (1, 3), (2, 3)],
            Saltire => vec![(0, 1), (1, 2), (2, 3), (0, 3)],
            Canton(n) => {
                let n = usize::from(n);
                // The canton runs beside the bands it covers and sits on the next.
                let covered = (self.canton().0 * n as f32).round() as usize;
                (1..=n.min(covered + 1))
                    .map(|band| (0, band))
                    .chain((2..=n).map(|band| (band - 1, band)))
                    .collect()
            }
        }
    }

    /// Every way to lay the scheme into this layout that shows each of its colours and
    /// keeps neighbouring regions readable, one tincture per region.
    pub fn fills(self, scheme: &[Tincture]) -> Vec<Vec<Tincture>> {
        let (regions, colours) = (self.regions(), scheme.len());
        if colours == 0 || regions < colours {
            return Vec::new();
        }
        let borders = self.borders();
        (0..colours.pow(regions as u32))
            .map(|code| {
                (0..regions)
                    .map(|i| code / colours.pow(i as u32) % colours)
                    .collect::<Vec<_>>()
            })
            .filter(|picks| (0..colours).all(|c| picks.contains(&c)))
            .filter(|picks| {
                borders
                    .iter()
                    .all(|&(a, b)| scheme[picks[a]].reads_against(scheme[picks[b]]))
            })
            .map(|picks| picks.into_iter().map(|c| scheme[c]).collect())
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Flag {
    layout: Layout,
    fills: Vec<Tincture>,
}

impl Flag {
    /// A flag of the given regions, if every neighbouring pair reads.
    pub fn new(layout: Layout, fills: Vec<Tincture>) -> Option<Self> {
        let readable = fills.len() == layout.regions()
            && layout
                .borders()
                .iter()
                .all(|&(a, b)| fills[a].reads_against(fills[b]));
        readable.then_some(Self { layout, fills })
    }

    /// Draw a colour scheme from the seed, then a layout and a way to lay it in.
    pub fn generate(seed: u64) -> Self {
        let mut rng = Rng(seed);
        for _ in 0..64 {
            let scheme = scheme(&mut rng);
            let layouts: Vec<Layout> = Layout::ALL
                .into_iter()
                .filter(|layout| layout.regions() >= scheme.len())
                .collect();
            let layout = layouts[rng.below(layouts.len())];
            let fills = layout.fills(&scheme);
            if !fills.is_empty() {
                let fills = fills[rng.below(fills.len())].clone();
                return Self { layout, fills };
            }
        }
        Self {
            layout: Bands(2),
            fills: vec![White, Scarlet],
        }
    }

    /// A flag from the seed that cannot be taken for `other` at a distance: at least
    /// [`MIN_DIFFERENCE`] of its cloth is a colour `other` does not show there.
    pub fn rival(seed: u64, other: &Self) -> Self {
        let mut rng = Rng(seed);
        for _ in 0..64 {
            let flag = Self::generate(rng.next());
            if flag.difference(other) >= MIN_DIFFERENCE {
                return flag;
            }
        }
        // Two colours the other flag does not use at all.
        let free: Vec<Tincture> = Tincture::ALL
            .into_iter()
            .filter(|t| !other.fills.contains(t))
            .collect();
        let pair = free
            .iter()
            .flat_map(|&a| free.iter().map(move |&b| (a, b)))
            .find(|(a, b)| a.reads_against(*b))
            .expect("four of nine tinctures leave a readable pair");
        Self {
            layout: Bands(2),
            fills: vec![pair.0, pair.1],
        }
    }

    pub fn layout(&self) -> Layout {
        self.layout
    }

    /// One tincture per region of the layout.
    pub fn fills(&self) -> &[Tincture] {
        &self.fills
    }

    /// The colours the flag shows, in the order they first appear.
    pub fn scheme(&self) -> Vec<Tincture> {
        let mut scheme = Vec::new();
        for &t in &self.fills {
            if !scheme.contains(&t) {
                scheme.push(t);
            }
        }
        scheme
    }

    pub fn tincture(&self, u: f32, v: f32) -> Tincture {
        self.fills[self.layout.region(u, v)]
    }

    /// The share of the cloth in each tincture, in the order of [`Tincture::ALL`].
    pub fn coverage(&self) -> [f32; 9] {
        const COLUMNS: usize = 140;
        const ROWS: usize = 50;
        let mut cover = [0.; 9];
        for row in 0..ROWS {
            for column in 0..COLUMNS {
                let u = (column as f32 + 0.5) / COLUMNS as f32;
                let v = (row as f32 + 0.5) / ROWS as f32;
                cover[self.tincture(u, v) as usize] += 1. / (ROWS * COLUMNS) as f32;
            }
        }
        cover
    }

    /// The share of the cloth that differs in colour between the two flags, regardless of
    /// where it lies: 0 when they show the same colours in the same amounts.
    pub fn difference(&self, other: &Self) -> f32 {
        let shared: f32 = self
            .coverage()
            .iter()
            .zip(other.coverage())
            .map(|(a, b)| a.min(b))
            .sum();
        (1. - shared).max(0.)
    }

    /// The cloth as sRGB pixels with opaque alpha, four bytes each, rows from the head
    /// and the hoist on the left. Each pixel averages 16 samples in linear light, so
    /// diagonal edges come out smooth.
    pub fn paint(&self, width: u32, height: u32) -> Vec<u8> {
        const SUB: u32 = 4;
        let light = Tincture::ALL.map(|t| t.srgb().map(to_linear));
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                let mut sum = [0.; 3];
                for sy in 0..SUB {
                    for sx in 0..SUB {
                        let u = (x as f32 + (sx as f32 + 0.5) / SUB as f32) / width as f32;
                        let v = (y as f32 + (sy as f32 + 0.5) / SUB as f32) / height as f32;
                        let colour = light[self.tincture(u, v) as usize];
                        for (total, channel) in sum.iter_mut().zip(colour) {
                            *total += channel;
                        }
                    }
                }
                rgba.extend(sum.map(|total| to_srgb(total / (SUB * SUB) as f32)));
                rgba.push(255);
            }
        }
        rgba
    }
}

impl fmt::Display for Flag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let list = |fills: &[Tincture], join: &str, last: &str| {
            let names: Vec<&str> = fills.iter().map(|t| t.name()).collect();
            match names.split_last() {
                Some((end, [])) => end.to_string(),
                Some((end, rest)) => format!("{}{last}{end}", rest.join(join)),
                None => String::new(),
            }
        };
        let fills = &self.fills;
        match self.layout {
            Bands(_) => f.write_str(&list(fills, " over ", " over ")),
            Pales(_) => write!(
                f,
                "upright bands of {} from the hoist",
                list(fills, ", ", " and ")
            ),
            Quarters => write!(f, "quartered {}", list(fills, ", ", " and ")),
            Saltire => write!(
                f,
                "cut corner to corner: {} at the head, {} at the fly, {} at the foot, {} at the hoist",
                fills[0], fills[1], fills[2], fills[3]
            ),
            Canton(1) => write!(f, "{} with a {} canton", fills[1], fills[0]),
            Canton(_) => write!(
                f,
                "{} with a {} canton",
                list(&fills[1..], " over ", " over "),
                fills[0]
            ),
        }
    }
}

/// Two to four tinctures, each reading against the one before, so that they can always
/// be laid out in bands in this order.
fn scheme(rng: &mut Rng) -> Vec<Tincture> {
    let count = [2, 2, 2, 3, 3, 3, 3, 4, 4][rng.below(9)];
    let mut scheme = vec![Tincture::ALL[rng.below(Tincture::ALL.len())]];
    while scheme.len() < count {
        let last = scheme[scheme.len() - 1];
        let next: Vec<Tincture> = Tincture::ALL
            .into_iter()
            .filter(|t| !scheme.contains(t) && t.reads_against(last))
            .collect();
        if next.is_empty() {
            break;
        }
        scheme.push(next[rng.below(next.len())]);
    }
    scheme
}

/// SplitMix64.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn to_linear(c: u8) -> f32 {
    let c = f32::from(c) / 255.;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn to_srgb(l: f32) -> u8 {
    let c = if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1. / 2.4) - 0.055
    };
    (c * 255.).round().clamp(0., 255.) as u8
}
