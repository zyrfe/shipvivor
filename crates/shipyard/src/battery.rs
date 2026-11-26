//! Breakwater's authored deck geometry, shared by the dock preview and main battery.
use crate::{Berth, Component, Design, Frame, Gunhouse};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Metres one barbette step lifts a turret above the deck.
pub const BARBETTE: f32 = 1.05;
/// How far either way of her beam a double turret trains: round its own side and a
/// little past the bow and the stern.
const DOUBLE_ARC: f32 = 95.;

/// How a main turret stands on its section.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mounting {
    /// On the centreline, training over either beam.
    #[default]
    Centre,
    /// One of a light pair in a wing gallery, training over its own beam.
    Wing,
    /// One of two full-size turrets abreast on a capital beam, training over its own
    /// side and round to the bow and the stern.
    Double,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GunStation {
    pub berth: Berth,
    /// Metres forward of the player's starting helm datum.
    pub fore: f32,
    /// Metres to starboard, negative on the port side.
    pub abeam: f32,
    /// Deck bearing in radians at the centre of the mount's travel.
    pub home: f32,
    pub half_arc: f32,
    /// A second deckhouse on the other end of an enclosed centre battery.
    pub inner_mask: f32,
    /// How the turret is built: its barrels, its bore and its barbette.
    pub gunhouse: Gunhouse,
    pub mounting: Mounting,
}

impl GunStation {
    /// One of a light wing pair, whose cradles are smaller than a centreline turret's.
    pub fn paired(&self) -> bool {
        self.mounting == Mounting::Wing
    }

    pub fn bears(&self, bearing: f32) -> bool {
        let angle = ((bearing - self.home + PI).rem_euclid(TAU) - PI).abs();
        angle <= self.half_arc + 1e-5 && angle + 1e-5 >= self.inner_mask
    }

    /// Metres the turret's barbette lifts it above the deck.
    pub fn lift(&self) -> f32 {
        BARBETTE * f32::from(self.gunhouse.tier)
    }
}

impl Design {
    /// Main mounts in compartment order, with a port mount before its starboard mate.
    /// A deckhouse or centreline turret masks forty degrees a side from the next
    /// compartment. A wing gallery leaves the centreline clear, and a raised turret fires
    /// over a centreline turret that stands lower than itself.
    pub fn gun_stations(&self) -> Vec<GunStation> {
        let blocks = |i: usize, tier: u8| {
            let module = &self.modules()[i];
            module
                .fit
                .iter()
                .any(|kind| matches!(kind, Component::Director | Component::Engine))
                // A wing gallery leaves the centreline clear. A double pair fills it.
                || (module.frame != Frame::BatteryGallery
                    && module.fit.contains(&Component::MainGun)
                    && module.gunhouse.tier >= tier)
        };
        let shadow =
            |gap: Option<usize>| gap.map_or(0., |n| (40. / (n as f32).sqrt()).to_radians());
        self.fittings()
            .into_iter()
            .filter(|f| f.component == Component::MainGun)
            .map(|gun| {
                let at = gun.berth.module;
                let gunhouse = self.modules()[at].gunhouse;
                let tier = gunhouse.tier;
                let ahead = (0..at).rev().find(|&i| blocks(i, tier)).map(|i| at - i);
                let astern = (at + 1..self.modules().len())
                    .find(|&i| blocks(i, tier))
                    .map(|i| i - at);
                let forward = shadow(astern) >= shadow(ahead);
                let mounting = match self.modules()[at].frame {
                    Frame::BatteryGallery => Mounting::Wing,
                    Frame::DoubleBattery => Mounting::Double,
                    _ => Mounting::Centre,
                };
                // Centreline cradles sit slightly abaft the frame centre, wing cradles abreast.
                let fore = 16.
                    - gun.aft
                    - if mounting == Mounting::Centre {
                        1.2
                    } else {
                        0.
                    };
                let beam = gun.abeam.signum() * FRAC_PI_2;
                let (home, half_arc, inner_mask) = match mounting {
                    Mounting::Wing => (beam, 72_f32.to_radians(), 0.),
                    Mounting::Double => (beam, DOUBLE_ARC.to_radians(), 0.),
                    Mounting::Centre => (
                        if forward { 0. } else { PI },
                        PI - shadow(ahead).max(shadow(astern)),
                        shadow(ahead).min(shadow(astern)),
                    ),
                };
                GunStation {
                    berth: gun.berth,
                    fore,
                    abeam: gun.abeam,
                    home,
                    half_arc,
                    inner_mask,
                    gunhouse,
                    mounting,
                }
            })
            .collect()
    }
}
