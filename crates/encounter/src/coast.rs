//! Offshore landmarks stand clear of the marked fairways and harbour approaches.
use super::{Body, HarbourSite, Islet, Port, Vec2};
use glam::{Vec3, Vec3Swizzles};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LandmarkKind {
    Beacon,
    Stacks,
    Pines,
    /// A large island under an ice cap, its glacier running down to the sea.
    Glacier,
    /// A piece of the mainland a coastal passage runs along. Pieces overlap into one
    /// shore of cliffs and crags under snow, and the bigger a piece, the higher it rises.
    Mainland,
    /// The flat island an airfield stands on, levelled at the height of a yard's shelf.
    /// Its heading points out from the shore, and its runway runs across that.
    Airfield,
}
/// Height of the sea cliff round a piece of the mainland, in metres.
pub const MAINLAND_CLIFF: f32 = 7.;
/// Share of a mainland piece's radius under its high ground.
pub const MAINLAND_CREST: f32 = 0.8;
/// Height of a mainland piece's high ground: a crag on the shore stands lower than the
/// broad hills behind it, and the mountains inland stand highest.
pub fn mainland_top(radius: f32) -> f32 {
    (14. + 0.12 * radius).min(64.)
}

/// Height of the ground a small island's features stand on, in metres above the sea.
pub const FOOTING: f32 = 3.1;
/// Damage a pine takes before it goes over. Any main-battery round fells one.
const PINE_STRENGTH: u16 = 2;
/// Damage a lighthouse tower takes before it topples: as much as a drydock.
const LIGHTHOUSE_STRENGTH: u16 = 24;
/// Each tier of a sea stack holds this much damage for each metre of the stack's rise,
/// so a heavy round takes the tall stack down a tier a hit and a light one chips at it.
const TIER_PER_METRE: f32 = 0.8;
/// Metres a round's blast reaches among the trees for each inch of its bore.
const BLAST_PER_INCH: f32 = 0.4;

/// Something standing on a small island that rounds can bring down.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FeatureKind {
    /// A pine under snow. A hit fells it, and a heavy round bursting among the trees
    /// fells those near it too.
    Pine,
    /// A sea stack in four tiers of rock, shot away from the top down.
    Stack,
    /// A lighthouse tower, which topples whole once broken.
    Lighthouse,
}

/// Where one of an island's features stands and how big it is. Its place is in the
/// island's own frame at [`super::ISLAND_RADIUS`], before the island's heading turns it
/// and its size spreads it. The view builds its models from the same plan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Site {
    pub kind: FeatureKind,
    pub at: Vec2,
    /// Metres it rises: a pine from its roots, a stack from its foot at 2.2 m, a
    /// lighthouse from the ground to the top of its lantern.
    pub height: f32,
    /// A stack's half-width at its foot. A pine's and a tower's radius.
    pub width: f32,
}
impl Site {
    /// Radius of the solid rounds and sightlines meet.
    fn reach(&self) -> f32 {
        match self.kind {
            FeatureKind::Pine => 0.22 * self.height,
            FeatureKind::Stack => 0.85 * self.width,
            FeatureKind::Lighthouse => 3.,
        }
    }
    fn strength(&self) -> u16 {
        match self.kind {
            FeatureKind::Pine => PINE_STRENGTH,
            FeatureKind::Stack => 4 * self.tier(),
            FeatureKind::Lighthouse => LIGHTHOUSE_STRENGTH,
        }
    }
    fn tier(&self) -> u16 {
        (self.height * TIER_PER_METRE).ceil() as u16
    }
    /// Top of a stack's tier `tier`, counted from zero at its foot. The view stacks its
    /// slabs the same way.
    pub fn tier_top(&self, tier: u8) -> f32 {
        let u = f32::from(tier) / 4.;
        2.2 + u * self.height + self.height * (0.37 - 0.10 * u)
    }
}

/// The same hash the view scatters its rock and scrub with, so that the trees stand
/// where they always have.
fn scatter(x: u32, y: u32, seed: u32) -> f32 {
    let mut n = x.wrapping_mul(0x9e3779b9) ^ y.wrapping_mul(0x85ebca6b) ^ seed;
    n ^= n >> 16;
    n = n.wrapping_mul(0x7feb352d);
    n ^= n >> 15;
    n = n.wrapping_mul(0x846ca68b);
    n ^= n >> 16;
    (n >> 8) as f32 / 16777216.
}

impl LandmarkKind {
    /// The features standing on an island of this kind, in the order a landmark keeps
    /// them.
    pub fn sites(self) -> Vec<Site> {
        match self {
            Self::Pines => (0..19)
                .map(|i| {
                    let seed = 89;
                    let a = scatter(i, 0, seed) * std::f32::consts::TAU;
                    let height = 4.3 + 4.65 * scatter(i, 2, seed);
                    Site {
                        kind: FeatureKind::Pine,
                        at: Vec2::from_angle(a) * (11. * scatter(i, 1, seed).sqrt()),
                        height,
                        width: 0.22 * height,
                    }
                })
                .collect(),
            Self::Stacks => [
                (0., 0., 19.6, 4.9),
                (-6.3, 1.7, 11.8, 3.3),
                (5., -4.8, 6.8, 3.3),
            ]
            .map(|(x, z, height, width)| Site {
                kind: FeatureKind::Stack,
                at: Vec2::new(x, z),
                height,
                width,
            })
            .to_vec(),
            Self::Beacon => vec![Site {
                kind: FeatureKind::Lighthouse,
                at: Vec2::ZERO,
                height: 16.5,
                width: 2.4,
            }],
            Self::Glacier | Self::Mainland | Self::Airfield => Vec::new(),
        }
    }
}

/// One of a landmark's features, where it stands on the chart and what is left of it.
#[derive(Clone, Debug)]
pub struct Feature {
    pub site: Site,
    pub pos: Vec2,
    strength: u16,
    /// When it came down, by the battle's clock, and the way it fell.
    pub fell: Option<(f32, Vec2)>,
}
impl Feature {
    pub fn standing(&self) -> bool {
        self.fell.is_none()
    }
    /// Tiers of a stack still standing, from four down to none.
    pub fn tiers(&self) -> u8 {
        match self.site.kind {
            FeatureKind::Stack => self.strength.div_ceil(self.site.tier()) as u8,
            _ => u8::from(self.standing()) * 4,
        }
    }
    /// What is left of it as a solid for rounds and sightlines: centre, radius, height.
    pub fn volume(&self) -> Option<(Vec2, f32, f32)> {
        let top = match self.site.kind {
            FeatureKind::Stack => self.tiers().checked_sub(1).map(|t| self.site.tier_top(t)),
            _ => self.standing().then_some(FOOTING + self.site.height),
        };
        top.map(|top| (self.pos, self.site.reach(), top))
    }
    fn fall(&mut self, now: f32, way: Vec2) {
        if self.standing() {
            self.strength = 0;
            self.fell = Some((now, way.normalize_or(Vec2::X)));
        }
    }
}

pub struct Landmark {
    pub body: Body,
    pub radius: f32,
    pub kind: LandmarkKind,
    /// What stands on it that rounds can bring down, in the order of its kind's sites.
    pub features: Vec<Feature>,
}
impl Landmark {
    pub fn new(body: Body, radius: f32, kind: LandmarkKind) -> Self {
        let spread = radius / super::ISLAND_RADIUS;
        let turn = Vec2::from_angle(body.heading);
        let features = (kind.sites().into_iter())
            .map(|site| Feature {
                site,
                pos: body.pos + turn.rotate(site.at * spread),
                strength: site.strength(),
                fell: None,
            })
            .collect();
        Self {
            body,
            radius,
            kind,
            features,
        }
    }
    /// A round of `damage` striking feature `index`, travelling along `along`, at `now`
    /// by the battle's clock. Returns where a stack's tier or a lighthouse came down.
    pub(super) fn strike(
        &mut self,
        index: usize,
        damage: u16,
        along: Vec2,
        now: f32,
    ) -> Option<(FeatureKind, Vec3)> {
        let feature = self.features.get_mut(index)?;
        let before = feature.tiers();
        feature.strength = feature.strength.saturating_sub(damage);
        let site = feature.site;
        match site.kind {
            FeatureKind::Stack => {
                let after = feature.tiers();
                if after == 0 {
                    feature.fell = Some((now, along.normalize_or(Vec2::X)));
                }
                (after < before).then(|| {
                    let height = 0.5 * (site.tier_top(before - 1) + site.tier_top(after));
                    (site.kind, feature.pos.extend(height).xzy())
                })
            }
            _ if feature.strength == 0 && feature.standing() => {
                feature.fall(now, along);
                let height = FOOTING + 0.5 * site.height;
                (site.kind == FeatureKind::Lighthouse)
                    .then(|| (site.kind, feature.pos.extend(height).xzy()))
            }
            _ => None,
        }
    }
    /// A round `calibre` inches across bursting at `at` brings down the pines its blast
    /// reaches, each thrown away from the burst. Returns how many fell.
    pub(super) fn blast(&mut self, at: Vec3, calibre: f32, now: f32) -> usize {
        let reach = BLAST_PER_INCH * calibre;
        if at.xz().distance(self.body.pos) > self.radius + reach {
            return 0;
        }
        let mut felled = 0;
        for tree in &mut self.features {
            let away = tree.pos - at.xz();
            if tree.site.kind == FeatureKind::Pine
                && tree.standing()
                && at.y <= FOOTING + tree.site.height
                && away.length() <= reach
            {
                tree.fall(now, away);
                felled += 1;
            }
        }
        felled
    }
    /// The classic layout: one island either side of each leg, where there is room.
    pub(super) fn classic(gates: [Vec2; 5], harbours: &[HarbourSite]) -> Vec<Islet> {
        let mut chart = Vec::<Islet>::new();
        for (i, &end) in gates.iter().enumerate() {
            let start = if i == 0 { Vec2::ZERO } else { gates[i - 1] };
            let normal = (end - start).normalize_or_zero().perp();
            for side in [-1., 1.] {
                let radius = if side < 0. { 28. } else { 42. };
                let center = start.lerp(end, if side < 0. { 0.47 } else { 0.64 })
                    + normal * side * (radius + 60. + 13. * i as f32);
                if gates
                    .iter()
                    .chain([&Vec2::ZERO])
                    .any(|p| p.distance(center) < radius + 65.)
                    || harbours
                        .iter()
                        .any(|h| h.pos.distance(center) < radius + 115.)
                    || harbours
                        .iter()
                        .filter_map(HarbourSite::industrial_roadstead)
                        .flatten()
                        .any(|(hub, room)| hub.distance(center) < radius + room)
                    || chart
                        .iter()
                        .any(|p| p.pos.distance(center) < radius + p.radius + 40.)
                {
                    continue;
                }
                let kind = match (i + usize::from(side > 0.)) % 3 {
                    0 => LandmarkKind::Beacon,
                    1 => LandmarkKind::Stacks,
                    _ => LandmarkKind::Pines,
                };
                chart.push(Islet {
                    pos: center,
                    heading: i as f32,
                    radius,
                    kind,
                });
            }
        }
        chart
    }
    /// The land itself as solids for rounds and sightlines (centre, radius, height): its
    /// low shelf, and the high ground of a glacier, a mainland piece or an airfield. A
    /// small island's features stand on the shelf apart from it.
    pub fn ground(&self) -> impl Iterator<Item = (Vec2, f32, f32)> {
        let (shelf, high) = match self.kind {
            LandmarkKind::Beacon | LandmarkKind::Stacks | LandmarkKind::Pines => (3.2, None),
            LandmarkKind::Glacier => (3.2, Some((0.74, 30.))),
            LandmarkKind::Mainland => (
                MAINLAND_CLIFF,
                Some((MAINLAND_CREST, mainland_top(self.radius))),
            ),
            LandmarkKind::Airfield => (3.2, Some((0.01, 3.2))),
        };
        let (pos, radius) = (self.body.pos, self.radius);
        std::iter::once((pos, radius, shelf))
            .chain(high.map(|(width, top)| (pos, radius * width, top)))
    }
    /// Its ground and whatever still stands on it.
    pub fn volumes(&self) -> impl Iterator<Item = (Vec2, f32, f32)> {
        self.volumes_seen(true)
    }
    /// Its volumes, leaving out a pine islet's trees when `trees` is false. The captain's
    /// sight passes over the trees, which are nothing the main guns engage.
    pub fn volumes_seen(&self, trees: bool) -> impl Iterator<Item = (Vec2, f32, f32)> {
        let standing = (self.features.iter())
            .filter(move |f| trees || f.site.kind != FeatureKind::Pine)
            .filter_map(Feature::volume);
        self.ground().chain(standing)
    }
    pub(super) fn clear(&self, pos: Vec2, margin: f32) -> Vec2 {
        let d = pos - self.body.pos;
        if d.length_squared() < (self.radius + margin).powi(2) {
            self.body.pos + d.try_normalize().unwrap_or(Vec2::X) * (self.radius + margin)
        } else {
            pos
        }
    }
}

/// Unobstructed water between the low wing guns or salvage winch and a target.
pub(super) fn open_water(from: Vec2, to: Vec2, ports: &[Port], islands: &[Landmark]) -> bool {
    ports
        .iter()
        .flat_map(Port::shores)
        .all(|(point, radius)| super::entry(from, to, point, radius).is_none())
        && islands
            .iter()
            .all(|i| super::entry(from, to, i.body.pos, i.radius).is_none())
}

#[cfg(test)]
mod tests;
