//! Offshore landmarks stand clear of the marked fairways and harbour approaches.
use super::{Body, HarbourSite, Islet, Port, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LandmarkKind {
    Beacon,
    Stacks,
    Pines,
    /// A large island under an ice cap, its glacier running down to the sea.
    Glacier,
}
pub struct Landmark {
    pub body: Body,
    pub radius: f32,
    pub kind: LandmarkKind,
}
impl Landmark {
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
    pub fn volumes(&self) -> [(Vec2, f32, f32); 2] {
        let (width, top) = match self.kind {
            LandmarkKind::Beacon => (0.12, 19.),
            LandmarkKind::Stacks => (0.44, 24.),
            LandmarkKind::Pines => (0.48, 12.),
            LandmarkKind::Glacier => (0.74, 30.),
        };
        [
            (self.body.pos, self.radius, 3.2),
            (self.body.pos, self.radius * width, top),
        ]
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
        .all(|p| super::entry(from, to, p.body.pos, super::ISLAND_RADIUS).is_none())
        && islands
            .iter()
            .all(|i| super::entry(from, to, i.body.pos, i.radius).is_none())
}
