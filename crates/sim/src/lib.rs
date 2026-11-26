//! Simulated legs of Shipvivor, sailed by a bot without the game's window, camera or
//! sound, to see whether a leg can be won and what the captain spends it doing.
//!
//! [`sector`] and [`voyage`] are shared with the game, which sails exactly the same
//! sectors through exactly the same steps.
pub mod bot;
pub mod leg;
pub mod sector;
pub mod voyage;

#[cfg(test)]
mod tests;

pub use sector::{Charted, Kind, Sector, TOUGH_HEAD_START, TOUGH_RAID_PACE, Weather};
pub use voyage::{engage, fit_player, helm_at_start, hull_shape, open_battle, settle, steer};
