//! Fixed-step ship handling and navigation through the prototype buoy course.

mod course;
mod helm;

pub use course::{Course, GATE_HALF_WIDTH, GATES, Gate};
pub use helm::Helm;
