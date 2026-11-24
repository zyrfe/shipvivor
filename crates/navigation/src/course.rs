use glam::Vec2;
use shipvivor_compass::{direction, heading};

#[derive(Clone, Copy)]
pub struct Gate {
    pub center: Vec2,
    /// Heading of the inbound course through the gate.
    pub bearing: f32,
}

impl Gate {
    // The segment joining the buoys runs across the inbound course.
    pub fn across(&self) -> Vec2 {
        direction(self.bearing).perp()
    }

    /// Port and starboard buoy positions.
    pub fn buoys(&self) -> [Vec2; 2] {
        [-1., 1.].map(|side| self.center + side * GATE_HALF_WIDTH * self.across())
    }

    pub fn crossed(&self, from: Vec2, to: Vec2) -> bool {
        let across = self.across();
        let forward = direction(self.bearing);
        let a = (from - self.center).dot(forward);
        let b = (to - self.center).dot(forward);
        if !((a < 0.0 && b >= 0.0) || (a > 0.0 && b <= 0.0)) {
            return false;
        }
        let crossing = from.lerp(to, a / (a - b));
        (crossing - self.center).dot(across).abs() <= GATE_HALF_WIDTH
    }
}

pub const GATE_HALF_WIDTH: f32 = 14.0;
pub const GATES: [Gate; 5] = [
    Gate {
        center: Vec2::new(0.0, -38.0),
        bearing: 0.0,
    },
    Gate {
        center: Vec2::new(42.0, -106.0),
        bearing: 40_f32.to_radians(),
    },
    Gate {
        center: Vec2::new(124.0, -75.0),
        bearing: 115_f32.to_radians(),
    },
    Gate {
        center: Vec2::new(104.0, 19.0),
        bearing: 195_f32.to_radians(),
    },
    Gate {
        center: Vec2::new(18.0, 41.0),
        bearing: 265_f32.to_radians(),
    },
];

pub struct Course {
    cleared: usize,
    gates: [Gate; 5],
}
impl Default for Course {
    fn default() -> Self {
        Self {
            cleared: 0,
            gates: GATES,
        }
    }
}

impl Course {
    /// Place each buoy pair across the inbound leg from the previous waypoint.
    pub fn chart(waypoints: [Vec2; 5], bearing: f32, spacing: f32) -> Self {
        let rotation = glam::Mat2::from_angle(bearing);
        Self::through(waypoints.map(|waypoint| rotation * waypoint * spacing))
    }
    /// Buoy pairs centred on these points, each across the leg in from the one before.
    /// The first leg starts at the origin, where the ship enters the sector.
    pub fn through(centers: [Vec2; 5]) -> Self {
        let mut previous = Vec2::ZERO;
        let gates = centers.map(|center| {
            let inbound = center - previous;
            previous = center;
            Gate {
                center,
                bearing: heading(inbound),
            }
        });
        Self { cleared: 0, gates }
    }
    pub fn gates(&self) -> &[Gate; 5] {
        &self.gates
    }
    pub fn next_gate(&self) -> &Gate {
        &self.gates[self.active()]
    }
    pub fn active(&self) -> usize {
        self.cleared % self.gates.len()
    }

    pub fn lap(&self) -> usize {
        self.cleared / self.gates.len() + 1
    }

    /// Only the active gate counts. Return whether the navigation display changed.
    pub fn advance(&mut self, from: Vec2, to: Vec2) -> bool {
        if self.next_gate().crossed(from, to) {
            self.cleared += 1;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_requires_a_crossing_between_the_two_buoys() {
        let gate = &GATES[1];
        let fwd = -gate.across().perp();
        let from = gate.center - fwd;
        let to = gate.center + fwd;
        assert!(gate.crossed(from, to));
        let outside = gate.across() * (GATE_HALF_WIDTH + 1.0);
        assert!(!gate.crossed(from + outside, to + outside));
        assert!(!gate.crossed(from, from));
    }

    #[test]
    fn only_the_active_gate_counts_and_the_last_starts_a_new_lap() {
        let mut course = Course::default();
        let cross = |course: &mut Course, gate: &Gate| {
            let normal = gate.across().perp();
            course.advance(gate.center - normal, gate.center + normal)
        };
        assert!(!cross(&mut course, &GATES[1]));
        for (index, gate) in GATES.iter().enumerate() {
            assert_eq!(course.active(), index);
            assert_eq!(course.lap(), 1);
            assert!(cross(&mut course, gate));
        }
        assert_eq!(course.active(), 0);
        assert_eq!(course.lap(), 2);
    }
}
