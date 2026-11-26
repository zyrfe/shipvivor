//! Ships built from a row of hull modules, and what damage does to them.
//!
//! A [`Design`] lists modules from bow to stern. Each module is one watertight
//! compartment with components fitted on its deck and in its bays, so the dock edits
//! the same thing a shell lands on. A [`Ship`] is one hull built to a design: it takes
//! each hit where it lands, loses what stood behind it, floods, founders, and is
//! mended by its crew. Each module's sides are plated in four [`Panel`]s, port and
//! starboard, fore and aft, and each wears on its own, so fire kept on one spot gets
//! through sooner than the same fire spread along her.
//!
//! Distances along a hull are metres aft of the bow tip.
mod battery;
mod design;
mod repair;
mod ship;
mod stock;

pub use battery::{BARBETTE, GunStation, Mounting};
pub use design::{
    BOMB_DAMAGE, Berth, Component, Design, FLOATPLANE_REACH, Fault, Fitting, Frame, FrameSpec,
    Gunhouse, MISSILE_DAMAGE, MISSILE_RANGE, MISSILE_RELOAD, MISSILE_SALVO, MOST_TIER, Module,
    Protection, SORTIE_SECONDS, Sheet, Slot, StrikeCraft,
};
pub use repair::{DRAINED, Mend, PARTY_PUMP, PUMP, Task};
pub use ship::{BREACH, Compartment, INFLOW, Loss, Panel, Reach, Ship, Side, State};
pub use shipvivor_artillery::ArmorGrade;
pub use shipvivor_artillery::TurretLayout;

#[cfg(test)]
mod tests;
