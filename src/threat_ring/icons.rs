//! Plan-view emblems for each kind of enemy, bow up, and the disc and arrow they sit
//! with. Drawn smooth rather than as pixel stamps so they can turn to an enemy's heading.
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use shipvivor_encounter::{EnemyClass, ShoreTurret};

/// Shapes are laid out in units on a 48-unit square and drawn at [`SCALE`] pixels a
/// unit, so they stay sharp on a Retina display. Emblems stay within [`REACH`] units of
/// the centre, inside the disc's rim, so turning them never crosses it.
const UNITS: f32 = 48.;
const SCALE: f32 = 2.;
const SIDE: u32 = (UNITS * SCALE) as u32;
const REACH: f32 = 19.5;
const RIM: (f32, f32) = (20., 23.5);
const ARROW_SIDE: u32 = (32. * SCALE) as u32;

const WHITE: [u8; 3] = [255, 255, 255];
const CORAL: [u8; 3] = [255, 111, 93];
const GOLD: [u8; 3] = [255, 200, 94];
const INK: [u8; 3] = [21, 38, 44];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Emblem {
    Patrol,
    /// A torpedo boat: long, lean and fine in the bow, with her tube along the
    /// centreline aft.
    TorpedoBoat,
    Destroyer,
    Cruiser,
    Battleship,
    Freighter,
    /// A coaster: short and beamy, her hold forward and her wheelhouse aft.
    Coaster,
    /// A flagship: the crown she carries on the chart.
    Crown,
    Plane,
    /// A shore battery, as a star fort.
    Fort,
    /// A heavy battery: the star fort with its long gun, which points the way it is laid.
    Heavy,
    /// Fire from a gun that cannot be named.
    Unknown,
    /// An enemy torpedo running at her: its head, body, fins and screw, with the bubbles
    /// of its track behind.
    Torpedo,
}
impl Emblem {
    pub const ALL: [Self; 13] = [
        Self::Patrol,
        Self::TorpedoBoat,
        Self::Destroyer,
        Self::Cruiser,
        Self::Battleship,
        Self::Freighter,
        Self::Coaster,
        Self::Crown,
        Self::Plane,
        Self::Fort,
        Self::Heavy,
        Self::Unknown,
        Self::Torpedo,
    ];
    pub fn of(class: EnemyClass) -> Self {
        match class {
            EnemyClass::Freighter => Self::Freighter,
            EnemyClass::Coaster => Self::Coaster,
            EnemyClass::Patrol => Self::Patrol,
            EnemyClass::TorpedoBoat => Self::TorpedoBoat,
            EnemyClass::Destroyer => Self::Destroyer,
            EnemyClass::Cruiser => Self::Cruiser,
            EnemyClass::Battleship => Self::Battleship,
            EnemyClass::Commodore | EnemyClass::Parliament | EnemyClass::Regent => Self::Crown,
        }
    }
    /// A harbour gun: a heavy battery, or a light one.
    pub fn battery(gun: &ShoreTurret) -> Self {
        if gun.heavy_battery() {
            Self::Heavy
        } else {
            Self::Fort
        }
    }
    /// Hulls and aircraft turn with their heading, and a heavy battery's gun with its
    /// training. A crown, a fort and an unknown stay upright.
    pub fn turns(self) -> bool {
        !matches!(self, Self::Crown | Self::Fort | Self::Unknown)
    }
    /// Cargo ships and flagships are gold, as on the chart.
    pub fn gold(self) -> bool {
        matches!(self, Self::Freighter | Self::Coaster | Self::Crown)
    }
    /// How serious a threat of this kind is. The more serious pip draws on top where
    /// pips overlap. A torpedo in the water is the thing to dodge, then a flagship,
    /// then the heaviest guns down to an armed merchantman.
    pub fn menace(self) -> u8 {
        match self {
            Self::Freighter | Self::Coaster => 0,
            Self::Unknown => 1,
            Self::Patrol => 2,
            Self::Plane | Self::Fort => 3,
            Self::Destroyer | Self::TorpedoBoat => 4,
            Self::Cruiser => 5,
            Self::Battleship | Self::Heavy => 6,
            Self::Crown => 7,
            Self::Torpedo => 8,
        }
    }

    /// Whether a point, in units from the centre with y down, is inked. Classes differ
    /// in length, beam and turrets, drawn large enough to tell apart at a glance.
    fn covers(self, p: Vec2) -> bool {
        let wells =
            |wells: &[(f32, f32)]| wells.iter().any(|&(y, r)| p.distance(Vec2::new(0., y)) < r);
        match self {
            Self::Patrol => hull(p, 11.5, 4., 6.) && !wells(&[(-3., 2.4)]),
            Self::TorpedoBoat => {
                let tube = p.x.abs() < 1. && (1.5..=9.5).contains(&p.y);
                hull(p, 13.5, 3., 9.) && !tube
            }
            Self::Destroyer => hull(p, 17.5, 4.2, 8.) && !wells(&[(-8., 2.5), (10., 2.5)]),
            Self::Cruiser => hull(p, 18., 6.2, 8.) && !wells(&[(-8., 3.4), (10., 3.4)]),
            Self::Battleship => {
                hull(p, 18.5, 8.5, 8.) && !wells(&[(-10., 4.), (-1.5, 4.), (10.5, 4.)])
            }
            Self::Freighter => {
                let hatch = [-8., -1., 6.]
                    .iter()
                    .any(|&y| p.x.abs() < 3.2 && (p.y - y).abs() < 2.2);
                hull(p, 16., 6., 3.5) && !hatch
            }
            Self::Coaster => {
                let hatch = p.x.abs() < 2.4 && (p.y + 1.5).abs() < 3.;
                hull(p, 10., 5., 4.) && !hatch
            }
            Self::Crown => {
                // Three points over a band, with a line between them.
                let x = p.x.abs();
                let top = if x < 6.8 {
                    -13. + x / 6.8 * 12.
                } else {
                    -1. - (x - 6.8) / 6.7 * 8.5
                };
                x <= 13.5 && p.y <= 9.5 && p.y >= top && !(2.5..4.2).contains(&p.y)
            }
            Self::Torpedo => {
                let head = p.y < -9. && p.distance(Vec2::new(0., -9.)) <= 2.3;
                let body = p.x.abs() <= 2.3 && (-9. ..=7.5).contains(&p.y);
                let fins = (7.5..=10.5).contains(&p.y) && p.x.abs() <= 5. - (p.y - 7.5) * 0.4;
                let screw = (10.5..=12.).contains(&p.y) && p.x.abs() <= 1.3;
                let track = [(15., 1.5), (18., 1.1)]
                    .iter()
                    .any(|&(y, r)| p.distance(Vec2::new(0., y)) <= r);
                head || body || fins || screw || track
            }
            Self::Plane => {
                let fuselage = p.x.abs() <= 2.2 && (-15.5..=14.).contains(&p.y);
                let wing = (-6. ..=-0.5).contains(&p.y) && p.x.abs() <= 17.5 - (p.y + 6.) * 0.8;
                let tail = (9.5..=13.5).contains(&p.y) && p.x.abs() <= 7.;
                fuselage || wing || tail
            }
            Self::Fort => {
                let walls = p.x.abs() <= 10. && p.y.abs() <= 10.;
                let bastion = (p.abs() - Vec2::splat(10.)).abs().element_sum() <= 6.5;
                let yard = p.x.abs() < 4.5 && p.y.abs() < 4.5;
                (walls || bastion) && !yard
            }
            Self::Heavy => {
                let gunhouse = p.length() <= 3.;
                let barrel = p.x.abs() <= 1.6 && (-19. ..=0.).contains(&p.y);
                Self::Fort.covers(p) || gunhouse || barrel
            }
            Self::Unknown => p.length() <= 6.5,
        }
    }
}

/// A hull in plan, bow up: a fine entry over `bow` pixels, then parallel sides to a
/// slightly narrower transom.
fn hull(p: Vec2, half_length: f32, half_beam: f32, bow: f32) -> bool {
    let from_bow = p.y + half_length;
    if !(0. ..=2. * half_length).contains(&from_bow) {
        return false;
    }
    let aft = 2. * half_length - from_bow;
    let width = if from_bow < bow {
        half_beam * (1. - (1. - from_bow / bow).powi(2)).sqrt()
    } else if aft < 3. {
        half_beam * (0.75 + 0.25 * aft / 3.)
    } else {
        half_beam
    };
    p.x.abs() <= width
}

fn rgba(c: [u8; 3], alpha: f32) -> Vec4 {
    Vec3::new(c[0] as f32, c[1] as f32, c[2] as f32).extend(255. * alpha) / 255.
}

/// Paint a square canvas with 4×4 samples a pixel, so edges stay smooth when turned.
fn raster(side: u32, paint: impl Fn(Vec2) -> Vec4) -> Image {
    const SUB: u32 = 4;
    let mut data = Vec::with_capacity((side * side * 4) as usize);
    for y in 0..side {
        for x in 0..side {
            let mut sum = Vec4::ZERO;
            for sy in 0..SUB {
                for sx in 0..SUB {
                    let offset = (Vec2::new(sx as f32, sy as f32) + 0.5) / SUB as f32;
                    let ink = paint(Vec2::new(x as f32, y as f32) + offset - side as f32 / 2.);
                    sum += (ink.truncate() * ink.w).extend(ink.w);
                }
            }
            let alpha = sum.w / (SUB * SUB) as f32;
            let colour = if sum.w > 0. {
                sum.truncate() / sum.w
            } else {
                Vec3::ZERO
            };
            data.extend(
                colour
                    .extend(alpha)
                    .to_array()
                    .map(|c| (c * 255.).round() as u8),
            );
        }
    }
    Image::new(
        Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// White, so the HUD can tint it.
pub(super) fn emblem(kind: Emblem) -> Image {
    raster(SIDE, |p| {
        let p = p / SCALE;
        if p.length() <= REACH && kind.covers(p) {
            rgba(WHITE, 1.)
        } else {
            Vec4::ZERO
        }
    })
}

/// A dark disc an emblem reads against over sky, sea and land alike, with a coloured rim.
pub(super) fn disc(gold: bool) -> Image {
    raster(SIDE, |p| match p.length() / SCALE {
        r if r > RIM.1 => Vec4::ZERO,
        r if r > RIM.0 => rgba(if gold { GOLD } else { CORAL }, 1.),
        _ => rgba(INK, 0.9),
    })
}

/// A coral arrowhead, tip up, with a dark edge.
pub(super) fn arrow() -> Image {
    // Two triangles meeting along the notch, so the head is a chevron.
    let head = |p: Vec2| {
        let (tip, notch) = (Vec2::new(0., -12.), Vec2::new(0., 3.));
        let corners = [Vec2::new(-10., 9.), Vec2::new(10., 9.)];
        corners.iter().any(|&c| triangle(p, tip, c, notch))
    };
    raster(ARROW_SIDE, |p| {
        let p = p / SCALE;
        if head(p) {
            rgba(CORAL, 1.)
        } else if head((p - Vec2::Y * 0.5) / 1.25) {
            rgba(INK, 1.)
        } else {
            Vec4::ZERO
        }
    })
}

fn triangle(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let side = |u: Vec2, v: Vec2| (v - u).perp_dot(p - u);
    let (x, y, z) = (side(a, b), side(b, c), side(c, a));
    (x >= 0. && y >= 0. && z >= 0.) || (x <= 0. && y <= 0. && z <= 0.)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inked(kind: Emblem) -> Vec<Vec2> {
        let image = emblem(kind);
        let data = image.data.unwrap();
        (0..SIDE * SIDE)
            .filter(|i| data[*i as usize * 4 + 3] > 0)
            .map(|i| {
                (Vec2::new((i % SIDE) as f32, (i / SIDE) as f32) + 0.5 - SIDE as f32 / 2.) / SCALE
            })
            .collect()
    }

    #[test]
    fn every_emblem_turns_inside_the_rim_and_is_its_own_shape() {
        let mut seen = Vec::new();
        for kind in Emblem::ALL {
            let ink = inked(kind);
            assert!(ink.len() > 20, "{kind:?} is nearly empty");
            let reach = ink.iter().map(|p| p.length()).fold(0., f32::max);
            assert!(reach <= REACH + 0.5, "{kind:?} reaches {reach}");
            assert!(REACH < RIM.0);
            assert!(!seen.contains(&ink), "{kind:?} repeats another emblem");
            seen.push(ink);
        }
    }

    #[test]
    fn bigger_classes_carry_more_hull_and_point_their_bows_up() {
        let area = |kind| inked(kind).len();
        assert!(area(Emblem::Patrol) < area(Emblem::Destroyer));
        assert!(area(Emblem::Destroyer) < area(Emblem::Cruiser));
        assert!(area(Emblem::Cruiser) < area(Emblem::Battleship));
        for kind in [
            Emblem::Patrol,
            Emblem::Destroyer,
            Emblem::Cruiser,
            Emblem::Battleship,
        ] {
            let ink = inked(kind);
            let (top, bottom) = ink
                .iter()
                .fold((f32::MAX, f32::MIN), |(t, b), p| (t.min(p.y), b.max(p.y)));
            // The fine entry leaves the forward third leaner than the after third.
            let third = (bottom - top) / 3.;
            let fore = ink.iter().filter(|p| p.y < top + third).count();
            let aft = ink.iter().filter(|p| p.y > bottom - third).count();
            assert!(fore < aft, "{kind:?}: bow not up ({fore} fore, {aft} aft)");
        }
    }

    #[test]
    fn the_arrow_is_coral_with_a_dark_edge_and_its_tip_up() {
        let image = arrow();
        let data = image.data.unwrap();
        let pixel = |x: u32, y: u32| {
            let i = ((y * ARROW_SIDE + x) * 4) as usize;
            [data[i], data[i + 1], data[i + 2], data[i + 3]]
        };
        let middle = ARROW_SIDE / 2;
        assert_eq!(pixel(middle, middle - 6)[..3], CORAL);
        assert_eq!(pixel(0, 0)[3], 0);
        let all = || (0..ARROW_SIDE).flat_map(|y| (0..ARROW_SIDE).map(move |x| pixel(x, y)));
        assert!(all().any(|p| p[3] == 255 && p[..3] == INK), "no dark edge");
        // The tip is the top of the ink.
        let top = (0..ARROW_SIDE)
            .find(|&y| (0..ARROW_SIDE).any(|x| pixel(x, y)[3] > 0))
            .unwrap();
        assert!((0..ARROW_SIDE).filter(|&x| pixel(x, top)[3] > 0).count() <= 3);
    }
}
