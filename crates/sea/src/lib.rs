//! The water the patrol is sailed on. It runs headlessly; the application draws it.
//!
//! [`Swell`] is the open sea: wave trains that depend only on the sea state and
//! the time, so they cover any distance at no cost. [`Wake`] is the water the
//! ships disturb, simulated on a patch that follows the player. Positions use the
//! chart convention of `shipvivor_compass`, and heights are metres above calm water.
mod fft;
mod swell;
#[cfg(test)]
mod tests;
mod wake;

pub use swell::{CHOP, Ride, Swell, TRAINS, Train};
pub use wake::{CELL, FOAM_SIDE, FOAM_WHITE, Keel, SIDE, Wake};
