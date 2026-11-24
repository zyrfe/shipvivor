//! The chart convention shared by every crate.
//!
//! Positions are metres on the water plane, stored as `Vec2 { x: east, y: south }`,
//! so north is `-Y`. Headings and bearings are radians, clockwise from north.

use glam::Vec2;

/// Unit vector pointing along a heading.
pub fn direction(heading: f32) -> Vec2 {
    Vec2::new(heading.sin(), -heading.cos())
}

/// Heading of a direction. The vector does not need to be normalised.
pub fn heading(direction: Vec2) -> f32 {
    direction.x.atan2(-direction.y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, PI};

    #[test]
    fn north_is_negative_y_and_headings_turn_clockwise() {
        assert!(direction(0.).abs_diff_eq(Vec2::NEG_Y, 1e-6));
        assert!(direction(FRAC_PI_2).abs_diff_eq(Vec2::X, 1e-6));
        assert!(direction(PI).abs_diff_eq(Vec2::Y, 1e-6));
        for angle in [-2.5, -0.4, 0., 0.9, 3.0_f32] {
            assert!((heading(direction(angle) * 7.) - angle).abs() < 1e-5);
        }
    }
}
