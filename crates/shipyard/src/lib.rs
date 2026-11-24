//! Ships built from a row of hull modules, and what damage does to them.
//!
//! A [`Design`] lists modules from bow to stern. Each module is one watertight
//! compartment with components fitted on its deck and in its bays, so the dock edits
//! the same thing a shell lands on. A [`Ship`] is one hull built to a design: it takes
//! each hit where it lands, loses what stood behind it, floods, founders, and is
//! mended by its crew.
//!
//! Distances along a hull are metres aft of the bow tip.
mod battery;
mod design;
mod repair;
mod ship;
mod stock;

pub use battery::GunStation;
pub use design::{Berth, Component, Design, Fault, Fitting, Frame, FrameSpec, Module, Sheet, Slot};
pub use repair::{Mend, Task};
pub use ship::{Compartment, Loss, Ship, State};
pub use shipvivor_artillery::ArmorGrade;

#[cfg(test)]
mod tests;
