//! Flags for Breakwater and for whoever she is fighting, drawn from a seed or picked by
//! the player.
//!
//! A flag is a [`Pattern`] that divides the cloth into colour slots, and one tincture for
//! each slot. A drawn flag starts as a colour scheme: two to four tinctures, each drawn
//! to read against the one before it. A pattern with room for every colour comes next,
//! and the scheme is laid into its slots so that no two slots sharing an edge are too
//! close in brightness to tell apart at sea. A flag the player picks may break that rule.
//!
//! On the cloth, `u` runs from the hoist (0) to the fly (1) and `v` from the head (0) to
//! the foot (1). The cloth is [`ASPECT`] times as long as it is deep.
//!
//! A new pattern needs a variant, its slots, the slot under each point of the cloth, a
//! code and a description. Which slots border which is read off the cloth.

use std::{fmt, sync::LazyLock};

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

/// The most colour slots any pattern may have.
pub const MAX_COLOURS: usize = 6;

/// How wide one slash is, and one arm of a saltire, measured across the cloth's diagonal
/// in the units of `u + v`. Either comes to about a quarter of the cloth's depth.
const SLASH: f32 = 0.28;
const SALTIRE: f32 = 0.24;

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
            Sky => "sky blue",
            _ => self.code(),
        }
    }

    /// One word for the tincture in a flag's [code](Flag::code).
    pub const fn code(self) -> &'static str {
        match self {
            White => "white",
            Gold => "gold",
            Scarlet => "scarlet",
            Navy => "navy",
            Green => "green",
            Black => "black",
            Orange => "orange",
            Sky => "sky",
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
pub enum Pattern {
    /// Horizontal bands of equal depth, counted from the head.
    Bands(u8),
    /// Upright bands of equal length, counted from the hoist.
    Pales(u8),
    /// Cut along the diagonal that rises from the lower hoist to the upper fly, counted
    /// from the upper hoist: the corner above, any slashes along the diagonal, and the
    /// corner below.
    Slashes(u8),
    /// Quartered: upper hoist, upper fly, lower hoist and lower fly.
    Quarters,
    /// Cut corner to corner into four triangles: head, fly, foot and hoist.
    Triangles,
    /// An X from corner to corner: head and foot, then hoist and fly, then the X.
    Saltire,
    /// A canton in the upper hoist, slot 0, over this many bands from the head.
    Canton(u8),
}
use Pattern::*;

/// Neighbouring cells of this grid that fall in different slots mark a border between
/// them, if they meet more than [`CONTACT`] times. Slots that touch only at a point, as
/// opposite quarters do, meet a cell or two at a time.
const TRACE: (usize, usize) = (280, 100);
const CONTACT: u32 = 4;

static BORDERS: LazyLock<Vec<Vec<(usize, usize)>>> =
    LazyLock::new(|| Pattern::ALL.iter().map(|p| p.trace_borders()).collect());

impl Pattern {
    /// Every pattern a flag may fly, a family to a row with one more colour to a column,
    /// as the customise screen lays them out.
    pub const ALL: [Self; 15] = [
        Bands(2),
        Bands(3),
        Bands(4),
        Pales(2),
        Pales(3),
        Pales(4),
        Slashes(2),
        Slashes(3),
        Slashes(4),
        Canton(1),
        Canton(2),
        Canton(3),
        Quarters,
        Triangles,
        Saltire,
    ];

    /// How many colours the pattern takes, one to a slot.
    pub fn slots(self) -> usize {
        match self {
            Bands(n) | Pales(n) | Slashes(n) => usize::from(n),
            Quarters | Triangles => 4,
            Saltire => 3,
            Canton(n) => usize::from(n) + 1,
        }
    }

    /// The slot under a point on the cloth.
    pub fn region(self, u: f32, v: f32) -> usize {
        let band = |n: u8, t: f32| ((t * f32::from(n)) as usize).min(usize::from(n) - 1);
        // How far a point lies across each diagonal, -1 to 1. Equal steps are equal
        // widths on the cloth whatever its proportions.
        let rising = u + v - 1.;
        let falling = u - v;
        match self {
            Bands(n) => band(n, v),
            Pales(n) => band(n, u),
            Slashes(n) => {
                let between = usize::from(n.max(2)) - 2;
                let start = -(between as f32) * SLASH / 2.;
                if rising < start {
                    0
                } else {
                    (1 + ((rising - start) / SLASH) as usize).min(usize::from(n) - 1)
                }
            }
            Quarters => 2 * usize::from(v >= 0.5) + usize::from(u >= 0.5),
            Triangles => match (v > u, v > 1. - u) {
                (false, false) => 0,
                (false, true) => 1,
                (true, true) => 2,
                (true, false) => 3,
            },
            Saltire if rising.abs() < SALTIRE / 2. || falling.abs() < SALTIRE / 2. => 2,
            // The head lies above both diagonals and the foot below both.
            Saltire => usize::from((rising < 0.) != (falling > 0.)),
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

    /// Every pair of slots that share an edge, lower index first and in order.
    pub fn borders(self) -> Vec<(usize, usize)> {
        match Self::ALL.iter().position(|&p| p == self) {
            Some(i) => BORDERS[i].clone(),
            None => self.trace_borders(),
        }
    }

    fn trace_borders(self) -> Vec<(usize, usize)> {
        let (columns, rows) = TRACE;
        let at = |c: usize, r: usize| {
            self.region(
                (c as f32 + 0.5) / columns as f32,
                (r as f32 + 0.5) / rows as f32,
            )
        };
        let mut contacts = [[0_u32; MAX_COLOURS]; MAX_COLOURS];
        for r in 0..rows {
            for c in 0..columns {
                let here = at(c, r);
                let right = (c + 1 < columns).then(|| at(c + 1, r));
                let below = (r + 1 < rows).then(|| at(c, r + 1));
                for there in [right, below].into_iter().flatten() {
                    if here != there {
                        contacts[here.min(there)][here.max(there)] += 1;
                    }
                }
            }
        }
        let slots = self.slots();
        (0..slots)
            .flat_map(|a| (a + 1..slots).map(move |b| (a, b)))
            .filter(|&(a, b)| contacts[a][b] > CONTACT)
            .collect()
    }

    /// Every way to lay the scheme into this pattern that shows each of its colours and
    /// keeps neighbouring slots readable, one tincture per slot.
    pub fn fills(self, scheme: &[Tincture]) -> Vec<Vec<Tincture>> {
        let (slots, colours) = (self.slots(), scheme.len());
        if colours == 0 || slots < colours {
            return Vec::new();
        }
        let borders = self.borders();
        (0..colours.pow(slots as u32))
            .map(|code| {
                (0..slots)
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

    /// One word for the pattern in a flag's [code](Flag::code).
    pub fn code(self) -> String {
        match self {
            Bands(n) => format!("bands{n}"),
            Pales(n) => format!("pales{n}"),
            Slashes(n) => format!("slashes{n}"),
            Quarters => "quarters".into(),
            Triangles => "triangles".into(),
            Saltire => "saltire".into(),
            Canton(n) => format!("canton{n}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Flag {
    pattern: Pattern,
    colours: Vec<Tincture>,
}

impl Flag {
    /// A flag of the given colours, one for each of the pattern's slots. Neighbours need
    /// not read against each other; see [`Flag::readable`].
    pub fn new(pattern: Pattern, colours: Vec<Tincture>) -> Option<Self> {
        (colours.len() == pattern.slots()).then_some(Self { pattern, colours })
    }

    /// Whether every pair of neighbouring slots reads at sea. Drawn flags always do.
    pub fn readable(&self) -> bool {
        self.pattern
            .borders()
            .iter()
            .all(|&(a, b)| self.colours[a].reads_against(self.colours[b]))
    }

    /// Draw a colour scheme from the seed, then a pattern and a way to lay it in.
    pub fn generate(seed: u64) -> Self {
        let mut rng = Rng(seed);
        for _ in 0..64 {
            let scheme = scheme(&mut rng);
            let patterns: Vec<Pattern> = Pattern::ALL
                .into_iter()
                .filter(|pattern| pattern.slots() >= scheme.len())
                .collect();
            let pattern = patterns[rng.below(patterns.len())];
            let fills = pattern.fills(&scheme);
            if !fills.is_empty() {
                let colours = fills[rng.below(fills.len())].clone();
                return Self { pattern, colours };
            }
        }
        Self {
            pattern: Bands(2),
            colours: vec![White, Scarlet],
        }
    }

    /// A flag from the seed that cannot be taken for `other` at a distance: at least
    /// [`MIN_DIFFERENCE`] of its cloth is a colour `other` does not show there. Should
    /// none of its draws manage that, it flies two colours `other` lacks, or failing even
    /// that the draw that came closest.
    pub fn rival(seed: u64, other: &Self) -> Self {
        let theirs = other.coverage();
        let mut rng = Rng(seed);
        let mut closest = (f32::MIN, Self::generate(seed));
        for _ in 0..64 {
            let flag = Self::generate(rng.next());
            let apart = flag.difference_from(&theirs);
            if apart >= MIN_DIFFERENCE {
                return flag;
            }
            if apart > closest.0 {
                closest = (apart, flag);
            }
        }
        let free: Vec<Tincture> = Tincture::ALL
            .into_iter()
            .filter(|t| !other.colours.contains(t))
            .collect();
        free.iter()
            .flat_map(|&a| free.iter().map(move |&b| (a, b)))
            .find(|(a, b)| a.reads_against(*b))
            .map_or(closest.1, |(a, b)| Self {
                pattern: Bands(2),
                colours: vec![a, b],
            })
    }

    pub fn pattern(&self) -> Pattern {
        self.pattern
    }

    /// One tincture per slot of the pattern.
    pub fn colours(&self) -> &[Tincture] {
        &self.colours
    }

    /// The colours the flag shows, in the order they first appear.
    pub fn scheme(&self) -> Vec<Tincture> {
        let mut scheme = Vec::new();
        for &t in &self.colours {
            if !scheme.contains(&t) {
                scheme.push(t);
            }
        }
        scheme
    }

    pub fn tincture(&self, u: f32, v: f32) -> Tincture {
        self.colours[self.pattern.region(u, v)]
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
        self.difference_from(&other.coverage())
    }

    fn difference_from(&self, cover: &[f32; 9]) -> f32 {
        let shared: f32 = self
            .coverage()
            .iter()
            .zip(cover)
            .map(|(a, b)| a.min(*b))
            .sum();
        (1. - shared).max(0.)
    }

    /// The cloth as sRGB pixels with opaque alpha, four bytes each, rows from the head
    /// and the hoist on the left. Each pixel averages 16 samples in linear light, so
    /// diagonal edges come out smooth.
    pub fn paint(&self, width: u32, height: u32) -> Vec<u8> {
        let light = self
            .colours
            .iter()
            .map(|t| t.srgb().map(to_linear))
            .collect();
        paint(self.pattern, light, width, height)
    }

    /// The cloth with one slot in its colour and every other slot in `rest`, to show
    /// where on the flag that slot lies.
    pub fn paint_slot(&self, slot: usize, rest: [u8; 3], width: u32, height: u32) -> Vec<u8> {
        let light = (0..self.pattern.slots())
            .map(|i| {
                if i == slot {
                    self.colours[i].srgb()
                } else {
                    rest
                }
            })
            .map(|c| c.map(to_linear))
            .collect();
        paint(self.pattern, light, width, height)
    }

    /// The pattern's code and then each slot's colour, as "canton2 gold scarlet black".
    pub fn code(&self) -> String {
        let mut code = self.pattern.code();
        for t in &self.colours {
            code.push(' ');
            code.push_str(t.code());
        }
        code
    }

    /// The flag a [code](Flag::code) describes, if it names a pattern and a colour for
    /// each of its slots.
    pub fn parse(code: &str) -> Option<Self> {
        let mut words = code.split_whitespace();
        let first = words.next()?;
        let pattern = Pattern::ALL.into_iter().find(|p| p.code() == first)?;
        let colours = words
            .map(|word| Tincture::ALL.into_iter().find(|t| t.code() == word))
            .collect::<Option<Vec<_>>>()?;
        Self::new(pattern, colours)
    }
}

impl fmt::Display for Flag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let list = |colours: &[Tincture], join: &str, last: &str| {
            let names: Vec<&str> = colours.iter().map(|t| t.name()).collect();
            match names.split_last() {
                Some((end, [])) => end.to_string(),
                Some((end, rest)) => format!("{}{last}{end}", rest.join(join)),
                None => String::new(),
            }
        };
        let colours = &self.colours;
        match self.pattern {
            Bands(_) => f.write_str(&list(colours, " over ", " over ")),
            Pales(_) => write!(
                f,
                "upright bands of {} from the hoist",
                list(colours, ", ", " and ")
            ),
            Slashes(2) => write!(f, "{} over {} on the diagonal", colours[0], colours[1]),
            Slashes(n) => {
                let last = usize::from(n) - 1;
                let (above, below) = (colours[0], colours[last]);
                let slashes = list(&colours[1..last], ", ", " and ");
                if last == 2 {
                    write!(f, "{above} and {below} with a {slashes} slash")
                } else {
                    write!(f, "{above} and {below} with {slashes} slashes")
                }
            }
            Quarters => write!(f, "quartered {}", list(colours, ", ", " and ")),
            Triangles => write!(
                f,
                "cut corner to corner: {} at the head, {} at the fly, {} at the foot, {} at the hoist",
                colours[0], colours[1], colours[2], colours[3]
            ),
            Saltire if colours[0] == colours[1] => {
                write!(f, "a {} saltire on {}", colours[2], colours[0])
            }
            Saltire => write!(
                f,
                "a {} saltire, {} at the head and foot, {} at the hoist and fly",
                colours[2], colours[0], colours[1]
            ),
            Canton(1) => write!(f, "{} with a {} canton", colours[1], colours[0]),
            Canton(_) => write!(
                f,
                "{} with a {} canton",
                list(&colours[1..], " over ", " over "),
                colours[0]
            ),
        }
    }
}

/// Paint a pattern with each slot in a linear colour.
fn paint(pattern: Pattern, light: Vec<[f32; 3]>, width: u32, height: u32) -> Vec<u8> {
    const SUB: u32 = 4;
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let mut sum = [0.; 3];
            for sy in 0..SUB {
                for sx in 0..SUB {
                    let u = (x as f32 + (sx as f32 + 0.5) / SUB as f32) / width as f32;
                    let v = (y as f32 + (sy as f32 + 0.5) / SUB as f32) / height as f32;
                    for (total, channel) in sum.iter_mut().zip(light[pattern.region(u, v)]) {
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
