//! Breakwater's authored deck geometry, shared by the dock preview and main battery.
use crate::{Berth, Component, Design, Frame};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

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
}

impl GunStation {
    pub fn paired(&self) -> bool {
        self.abeam != 0.
    }

    pub fn bears(&self, bearing: f32) -> bool {
        let angle = ((bearing - self.home + PI).rem_euclid(TAU) - PI).abs();
        angle <= self.half_arc + 1e-5 && angle + 1e-5 >= self.inner_mask
    }
}

impl Design {
    /// Main mounts in compartment order, with a port mount before its starboard mate.
    /// A deckhouse or centreline turret masks forty degrees a side from the next
    /// compartment. A wing gallery leaves the centreline clear.
    pub fn gun_stations(&self) -> Vec<GunStation> {
        let blocks = |i: usize| {
            let module = &self.modules()[i];
            module
                .fit
                .iter()
                .any(|kind| matches!(kind, Component::Director | Component::Engine))
                || (module.frame != Frame::BatteryGallery
                    && module.fit.contains(&Component::MainGun))
        };
        let shadow =
            |gap: Option<usize>| gap.map_or(0., |n| (40. / (n as f32).sqrt()).to_radians());
        self.fittings()
            .into_iter()
            .filter(|f| f.component == Component::MainGun)
            .map(|gun| {
                let at = gun.berth.module;
                let ahead = (0..at).rev().find(|&i| blocks(i)).map(|i| at - i);
                let astern = (at + 1..self.modules().len())
                    .find(|&i| blocks(i))
                    .map(|i| i - at);
                let forward = shadow(astern) >= shadow(ahead);
                let paired = self.modules()[at].frame == Frame::BatteryGallery;
                // Centreline cradles sit slightly abaft the frame centre, wing cradles abreast.
                let fore = 16. - gun.aft - if paired { 0. } else { 1.2 };
                let (home, half_arc, inner_mask) = if paired {
                    (gun.abeam.signum() * FRAC_PI_2, 72_f32.to_radians(), 0.)
                } else {
                    (
                        if forward { 0. } else { PI },
                        PI - shadow(ahead).max(shadow(astern)),
                        shadow(ahead).min(shadow(astern)),
                    )
                };
                GunStation {
                    berth: gun.berth,
                    fore,
                    abeam: gun.abeam,
                    home,
                    half_arc,
                    inner_mask,
                }
            })
            .collect()
    }
}
