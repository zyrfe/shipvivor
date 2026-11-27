//! Late industrial yards stand along one side of an open roadstead. The seaward-facing
//! machine, its controls and the outlying gun rocks share a plan with the renderer.
use crate::{Port, ShoreTurret};
use glam::Vec2;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum IndustrialYard {
    RedShift,
    BellWorks,
}

impl IndustrialYard {
    pub const FUEL_TANK: usize = 2;
    /// Barrels from the outermost picket back to the high shore battery.
    pub const LIGHT_GUNS: [u8; 5] = [1, 2, 3, 4, 2];
    /// Seconds per forge stroke and the fraction at which the head strikes the bed.
    pub const HAMMER_PERIOD: f32 = 3.4;
    pub const HAMMER_STRIKE: f32 = 0.90;

    pub const fn name(self) -> &'static str {
        match self {
            Self::RedShift => "The Red Shift",
            Self::BellWorks => "The Bell Works",
        }
    }

    pub const fn from_works(works: u8) -> Option<Self> {
        match works {
            5 => Some(Self::RedShift),
            6 => Some(Self::BellWorks),
            _ => None,
        }
    }

    pub const fn works(self) -> u8 {
        match self {
            Self::RedShift => 5,
            Self::BellWorks => 6,
        }
    }

    pub(super) fn factory_station(self, index: usize) -> Vec2 {
        if index == Self::FUEL_TANK {
            return Vec2::new(36., -14.);
        }
        let side = if index == 0 { -1. } else { 1. };
        match self {
            Self::RedShift => Vec2::new(18. * side, -22.),
            Self::BellWorks => Vec2::new(10. * side, -21.),
        }
    }
}

// Side and fore offsets, radius and summit. Detached skerries leave room to turn
// outwards; the shelf and the higher cliffs behind it carry the hall and batteries.
const ROCKS: &[(f32, f32, f32, f32)] = &[
    (0., -33., 34., 3.2),
    (36., -14., 13., 3.2),
    (-50., 92., 11., 2.1),
    (63., 46., 12., 2.4),
    (-31., 8., 12., 2.8),
    (28., -1., 11., 2.9),
    (-55., -26., 16., 9.),
    (44., -45., 20., 22.),
    (43., -64., 15., 32.),
    // High shoulders flank the gallery mouth and tie it into the main headland.
    (32., -53., 8., 37.),
    (56., -54., 8., 35.),
    (29., -56., 14., 13.),
];

pub(super) fn batteries(heading: f32) -> Vec<ShoreTurret> {
    let mut guns = Vec::new();
    for (i, barrels) in IndustrialYard::LIGHT_GUNS
        .into_iter()
        .chain([2])
        .enumerate()
    {
        let (x, y, _, top) = ROCKS[i + 2];
        let mut gun = if i == 5 {
            ShoreTurret::heavy(x, y, heading, i)
        } else {
            ShoreTurret::new(x, y, barrels, heading, i)
        };
        if i >= 4 {
            gun.elevation = top;
        }
        if i == 5 {
            gun.deployment = Some(0.);
        }
        guns.push(gun);
    }
    guns
}

impl Port {
    pub fn industrial_yard(&self) -> Option<IndustrialYard> {
        self.industry
    }

    pub fn industrial_center(&self) -> Vec2 {
        self.local(Vec2::new(0., -35.))
    }

    pub fn industrial_rocks(&self) -> impl Iterator<Item = (Vec2, f32, f32)> + '_ {
        let rocks = if self.industry.is_some() { ROCKS } else { &[] };
        rocks
            .iter()
            .map(|&(x, y, radius, top)| (self.local(Vec2::new(x, y)), radius, top))
    }

    /// Seconds since the second governor fell and the machinery let go.
    pub fn industrial_collapse(&self) -> Option<f32> {
        self.collapse
    }

    pub(super) fn advance_collapse(&mut self, dt: f32) {
        let Some(age) = &mut self.collapse else {
            return;
        };
        *age += dt;
        // The structure falls before the shutters part. The heavy barrels take another
        // three seconds to roll out and the layers have to train on the ship.
        for gun in &mut self.turrets {
            if let Some(deployment) = &mut gun.deployment {
                *deployment = ((*age - 1.5) / 3.).clamp(0., 1.);
            }
        }
    }

    /// Large, untargetable masses of the works stop shells and the gunsight. Leave
    /// daylight through the press frame and under the raised hopper, even while
    /// their silhouettes are still standing.
    pub(super) fn industrial_contacts(
        &self,
        a: Vec2,
        b: Vec2,
        mut visit: impl FnMut((f32, f32), f32, f32),
    ) {
        let Some(kind) = self.industry else { return };
        let centre = self.industrial_center();
        if crate::footprint(a, b, centre, 32.).is_none() {
            return;
        }
        let progress = (self.collapse.unwrap_or(0.) / 2.8).clamp(0., 1.);
        let stage = (3.9 * progress) as usize;
        let mut solid = |side: f32, depth, width, low: f32, high: f32| {
            let at = centre + self.body.forward().perp() * side;
            if let Some(span) = self.building_span(at, (depth, width), a, b) {
                visit(span, low + 3.2, high + 3.2);
            }
        };
        match kind {
            IndustrialYard::RedShift => {
                for (bay, falls) in [1, 1, 2, 2, 3, 4].into_iter().enumerate() {
                    let height = if stage >= falls { 2.45 } else { 17.8 };
                    solid((bay as f32 - 2.5) * 9., 8.5, 4.5, 0., height);
                }
                let bottom = 28.7 - 21.3 * progress * progress;
                // Three sections lower at different rates as the hopper tips.
                for (x, drop) in [(-14., 10.), (-2., 6.), (10., 2.)] {
                    let low = bottom - drop * progress * progress;
                    solid(x, 4.4, 6., low.max(0.), low + 10.4);
                }
            }
            IndustrialYard::BellWorks => {
                solid(0., 6.5, 8., 0., 3.4);
                solid(8.3, 4.2, 4.1, 0., if stage == 3 { 6.5 } else { 10.8 });
                // The headstock stays high in the empty frame after the ram falls.
                let crown = [32., 32., 29., 24.8][stage];
                solid(0., 4., 6., crown - 1.5, crown + 1.5);
                if stage < 3 {
                    solid(0., 2.5, 2.5, crown - 13., crown + 4.6);
                }
            }
        }
        let stacks = match kind {
            IndustrialYard::RedShift => [(-21.8, 7., 41.), (-14.8, 7., 32.5)],
            IndustrialYard::BellWorks => [(-8.8, 7.5, 28.), (8.7, 7.7, 23.8)],
        };
        let forward = self.body.forward();
        for (x, aft, height) in stacks {
            let at = centre + forward.perp() * x - forward * aft;
            if let Some(span) = crate::footprint(a, b, at, 1.5) {
                visit(span, 3.2, height + 3.2);
            }
        }
    }
}

#[cfg(test)]
mod tests;
