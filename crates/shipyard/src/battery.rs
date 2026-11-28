//! Breakwater's authored deck geometry, shared by the dock preview and main battery.
use crate::{Berth, Component, Design, Frame, Gunhouse};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Breakwater's deck edge as built, in metres to starboard and aft of her helm datum:
/// from the stem down the starboard side to the stern, and back up the port side.
pub const HULL_OUTLINE: [(f32, f32); 11] = [
    (0., -16.),
    (1.65, -11.7),
    (2.7, -5.),
    (2.75, 6.),
    (2.1, 12.7),
    (1.4, 14.1),
    (-1.4, 14.1),
    (-2.1, 12.7),
    (-2.75, 6.),
    (-2.7, -5.),
    (-1.65, -11.7),
];
/// Metres aft of her helm datum where the yard splices in new frames. Her deck edge
/// abaft this moves aft by whatever she has been lengthened.
pub const HULL_SPLICE: f32 = 4.;
/// Metres from her bow tip back to her helm datum, which deck stations are measured from.
const DATUM: f32 = 16.;

/// Metres one barbette step lifts a turret above the deck.
pub const BARBETTE: f32 = 1.05;
/// The machinery space's funnels stand as high as a turret raised this many steps, so a
/// turret raised one step more fires over them. The bridge masks a turret at any height.
pub const FUNNEL_TIER: u8 = 1;
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
    /// The nearest sections ahead of and astern of section `at` that mask a centreline
    /// turret standing `tier` barbette steps up. The bridge masks it at any height, the
    /// funnels until it stands above [`FUNNEL_TIER`], and a centreline or double turret
    /// only when it stands as high. A wing gallery leaves the centreline clear.
    pub fn masking(&self, at: usize, tier: u8) -> [Option<usize>; 2] {
        let blocks = |&i: &usize| {
            let module = &self.modules()[i];
            let has = |kind: Component| module.fit.contains(&kind);
            has(Component::Director)
                || (has(Component::Engine) && FUNNEL_TIER >= tier)
                // A wing gallery leaves the centreline clear. A double pair fills it.
                || (module.frame != Frame::BatteryGallery
                    && has(Component::MainGun)
                    && module.gunhouse.tier >= tier)
        };
        [
            (0..at).rev().find(blocks),
            (at + 1..self.modules().len()).find(blocks),
        ]
    }

    /// Main mounts in compartment order, with a port mount before its starboard mate.
    /// A deckhouse or centreline turret masks forty degrees a side from the next
    /// compartment. A wing gallery leaves the centreline clear, and a raised turret fires
    /// over a centreline turret, or funnels, that stand lower than itself.
    pub fn gun_stations(&self) -> Vec<GunStation> {
        let shadow =
            |gap: Option<usize>| gap.map_or(0., |n| (40. / (n as f32).sqrt()).to_radians());
        self.fittings()
            .into_iter()
            .filter(|f| f.component == Component::MainGun)
            .map(|gun| {
                let at = gun.berth.module;
                let gunhouse = self.modules()[at].gunhouse;
                let [ahead, astern] = self.masking(at, gunhouse.tier);
                let ahead = ahead.map(|i| at - i);
                let astern = astern.map(|i| i - at);
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

/// What a gun along her side stands in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Emplacement {
    /// A deck gun on a shielded mount at the deck edge.
    #[default]
    Deck,
    /// A deck gun in a casemate low in her side, on a battleship's hull and up.
    Casemate,
    /// A gunhouse of the secondary battery, where two deck guns would stand.
    Gunhouse,
}

/// Her guns along the deck edge, as [`Design::wing_layout`] stands them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WingLayout {
    /// The deck guns, on deck and in casemates, in the order they are fitted.
    pub deck: Vec<WingStation>,
    /// The secondary gunhouses, in the order they are fitted.
    pub gunhouses: Vec<WingStation>,
}

/// Where one of her guns along her side stands, and the arc it covers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WingStation {
    /// Metres to starboard, negative on the port side.
    pub abeam: f32,
    /// Metres forward of her helm datum.
    pub fore: f32,
    /// Bearing from the bow the gun rests on: the middle of the arc it was built with.
    pub home: f32,
    /// How far from `home` the gun trains, anticlockwise and clockwise seen from above:
    /// the first is never above zero, nor the second below it.
    pub arc: (f32, f32),
    pub emplacement: Emplacement,
}

/// Something standing on deck that a secondary must not fire through: metres to
/// starboard and forward of her helm datum, and the metres round it to keep clear.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Obstruction {
    pub abeam: f32,
    pub fore: f32,
    pub radius: f32,
}

/// The `stations` she has guns at, each arc cut short of every other gun's `berth`, the
/// metres round a mount its neighbours' fire keeps clear of, and of whatever else is
/// `standing` on her deck. A gun in the middle of a row trains only so far towards the
/// guns either side of it, while the one at the end of the row keeps the open end.
pub fn clear_arcs(
    stations: &[WingStation],
    berth: f32,
    standing: &[Obstruction],
) -> Vec<WingStation> {
    let guns = stations.iter().map(|s| Obstruction {
        abeam: s.abeam,
        fore: s.fore,
        radius: berth,
    });
    stations
        .iter()
        .enumerate()
        .map(|(own, &station)| {
            let mut arc = station.arc;
            let others = guns.clone().enumerate().filter(|&(i, _)| i != own);
            for block in others.map(|(_, gun)| gun).chain(standing.iter().copied()) {
                let (abeam, fore) = (block.abeam - station.abeam, block.fore - station.fore);
                let distance = abeam.hypot(fore);
                // Standing inside it, no arc would clear it.
                if distance <= block.radius {
                    continue;
                }
                let off = (abeam.atan2(fore) - station.home + PI).rem_euclid(TAU) - PI;
                let width = (block.radius / distance).asin();
                if off > 0. {
                    arc.1 = arc.1.min((off - width).max(0.));
                } else {
                    arc.0 = arc.0.max((off + width).min(0.));
                }
            }
            WingStation { arc, ..station }
        })
        .collect()
}

/// A beam gun covers its own side, short of four degrees from the bow and the stern.
const BEAM_ARC: (f32, f32) = (90_f32.to_radians(), 86_f32.to_radians());
/// A bow gun covers from four degrees across the bow round to its own beam, so the pair
/// overlap dead ahead.
const BOW_ARC: (f32, f32) = (43_f32.to_radians(), 47_f32.to_radians());
/// Secondaries along the deck edge stand this far apart: two a side on each open section.
const WING_PITCH: f32 = 2.5;
/// No two guns on one side stand closer than this, fore and aft.
const WING_CLEARANCE: f32 = 2.;
/// How far inboard of the deck edge a gun stands where the hull narrows.
const WING_MARGIN: f32 = 0.5;
/// Metres out from the centreline of a gun where the hull is broad, at the stock beam.
const WING_ABEAM: f32 = 2.18;
/// Nowhere narrower than this either side of the centreline takes a gun.
const WING_NARROWEST: f32 = 1.2;
/// Hulls this many times her stock beam carry deck guns in casemates in their side: a
/// battleship's hull and broader.
pub const CASEMATE_BEAM: f32 = 1.6;
/// A casemate covers its own beam this far either way: the port in her side allows no
/// more.
const CASEMATE_ARC: (f32, f32) = (90_f32.to_radians(), 58_f32.to_radians());
/// Metres more a secondary gunhouse keeps from a main turret than a deck gun's mount.
const GUNHOUSE_ROOM: f32 = 0.7;

/// Where one of her light AA mounts stands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AaStation {
    /// Metres to starboard, negative on the port side.
    pub abeam: f32,
    /// Metres forward of her helm datum.
    pub fore: f32,
    /// It stands on a bridge wing, which carries it at the height of the AA tubs. The
    /// others stand on a column raised from the deck.
    pub bridge_wing: bool,
}
/// Metres out from the centreline of an AA tub raised over the deck, at the stock beam:
/// inboard of the secondaries, clear of a turret's ring.
const AA_ABEAM: f32 = 1.2;
/// How far inboard of the deck edge an AA tub's column stands where the hull narrows.
const AA_MARGIN: f32 = 0.75;
/// No two AA tubs on one side stand closer than this, fore and aft.
const AA_CLEARANCE: f32 = 1.5;

/// Metres of clear deck a secondary wants from a main turret's centre: this much for
/// each six-inch of gunhouse (a six-inch gunhouse reaches about two metres to its
/// corners), this much more for each barrel past the first, and this much for the
/// secondary's own mount. A stock four-inch single leaves the forecastle pair where she
/// was built with it; a big gunhouse pushes it toward the stem.
const TURRET_ROOM: (f32, f32, f32) = (1.8, 0.18, 0.85);
/// Metres short of her stem that a secondary pushed forward may stand, and the step it
/// is moved by while it looks for room.
const STEM_ROOM: (f32, f32) = (2., 0.25);

/// The clear deck a secondary wants round a main turret of this build.
fn turret_room(house: crate::Gunhouse) -> f32 {
    let inches = f32::from(house.bore);
    let scale = (inches.min(6.) + (inches - 6.).max(0.) * 0.25) / 6.;
    let (per_scale, per_barrel, mount) = TURRET_ROOM;
    per_scale * scale + per_barrel * (house.barrels.count() as f32 - 1.) + mount
}

/// A pair of secondaries she was built with: metres out from the centreline and forward
/// of her datum on the stock hull, the middle of the starboard gun's arc and how far
/// either way it trains, and whether it goes with the bridge when the yard moves it.
struct Builder {
    abeam: f32,
    fore: f32,
    arc: (f32, f32),
    bridge: bool,
}
/// Abeam just abaft the bridge, on the forecastle between the forward turret and the
/// capstans, and abeam the bridge.
const BUILT: [Builder; 3] = [
    Builder {
        abeam: WING_ABEAM,
        fore: 0.,
        arc: BEAM_ARC,
        bridge: true,
    },
    Builder {
        abeam: 1.35,
        fore: 10.6,
        arc: BOW_ARC,
        bridge: false,
    },
    Builder {
        abeam: WING_ABEAM,
        fore: 3.8,
        arc: BEAM_ARC,
        bridge: true,
    },
];

impl Design {
    /// Where her deck guns stand, in the order they are fitted, a port gun before its
    /// starboard mate, with no secondary gunhouses aboard: see [`Design::wing_layout`].
    pub fn wing_stations(&self) -> Vec<WingStation> {
        self.wing_layout(0).deck
    }

    /// Where her guns along the deck edge stand with `houses` secondary gunhouses a side.
    ///
    /// The gunhouses stand one a side on each open section of deck, nearest the bridge
    /// first: a turret's bay, an engine room or an empty bay. The bridge and the
    /// machinery are crowded already, and a gallery of turrets abreast, a catapult or a
    /// missile rack wants its deck edge.
    ///
    /// The deck guns follow, in the order they are fitted. The pairs she was built with
    /// come first: abaft the bridge, on the forecastle and beside the bridge, the
    /// bridge's going where the yard moves it. On a hull broad enough, a casemate in her
    /// side under each open section comes next, which takes no deck. Then two a side
    /// along the deck edge of each open section no gunhouse stands on, nearest the bridge
    /// first. A longer hull has room for more. Only the player's hull carries them.
    pub fn wing_layout(&self, houses: usize) -> WingLayout {
        let moved = self
            .modules()
            .iter()
            .position(|m| m.fit.contains(&Component::Director))
            .map_or(0., |at| {
                self.span(at).0 - 2. * Frame::Straight.spec().length
            });
        let beam = self.beam_factor();
        // Each main turret, and the deck it wants clear round it, measured at the stock
        // beam so a broader hull carries the same layout further out. A secondary stands
        // on both beams, so it is measured to the turret on its own side.
        let turrets: Vec<(f32, f32, f32)> = self
            .gun_stations()
            .iter()
            .map(|g| (g.abeam.abs() / beam, g.fore, turret_room(g.gunhouse)))
            .collect();
        let clear = |abeam: f32, fore: f32| {
            turrets
                .iter()
                .all(|&(at, tf, room)| (abeam - at).hypot(fore - tf) >= room)
        };
        // Starboard guns: metres out at the stock beam, forward of the datum, and arc.
        // A pair she was built with that a turret crowds is pushed toward the stem if it
        // stands forward of the bridge, and otherwise given up for open deck.
        let bridge = BUILT[2].fore - moved;
        let mut guns: Vec<(f32, f32, (f32, f32))> = Vec::new();
        for b in &BUILT {
            let fore = b.fore - if b.bridge { moved } else { 0. };
            if clear(b.abeam, fore) {
                guns.push((b.abeam, fore, b.arc));
                continue;
            }
            if b.bridge {
                continue;
            }
            let (short, step) = STEM_ROOM;
            let room = (1..)
                .map(|k| fore + step * k as f32)
                .take_while(|&f| f <= DATUM - short)
                .map(|f| (b.abeam.min(self.deck_edge(f) - WING_MARGIN), f))
                .take_while(|&(abeam, _)| abeam >= WING_NARROWEST)
                .find(|&(abeam, f)| clear(abeam, f));
            if let Some((abeam, f)) = room {
                guns.push((abeam, f, b.arc));
            }
        }
        let last = self.modules().len().saturating_sub(1);
        let mut sections: Vec<usize> = (1..last).filter(|&at| self.open_deck(at)).collect();
        let middle = |at: usize| {
            let (start, end) = self.span(at);
            DATUM - 0.5 * (start + end)
        };
        sections.sort_by(|&a, &b| {
            (middle(a) - bridge)
                .abs()
                .total_cmp(&(middle(b) - bridge).abs())
        });
        // A gunhouse wants more room from a turret than a deck gun's mount, measured
        // in metres on her own beam: a broad hull leaves room beside a turret's bay.
        let house_clear = |abeam: f32, fore: f32| {
            turrets.iter().all(|&(at, tf, room)| {
                (beam * (abeam - at)).hypot(fore - tf) >= room + GUNHOUSE_ROOM
            })
        };
        let gunhouses: Vec<(usize, f32, f32)> = sections
            .iter()
            .map(|&at| {
                let fore = middle(at);
                (at, WING_ABEAM.min(self.deck_edge(fore) - WING_MARGIN), fore)
            })
            .filter(|&(_, abeam, fore)| abeam >= WING_NARROWEST && house_clear(abeam, fore))
            .take(houses.min(shipvivor_artillery::Gunhouses::MOST_PER_SIDE))
            .collect();
        let housed = |at: usize| gunhouses.iter().any(|&(h, ..)| h == at);
        let mut deck: Vec<(f32, f32, (f32, f32), Emplacement)> = guns
            .into_iter()
            .map(|(abeam, fore, arc)| (abeam, fore, arc, Emplacement::Deck))
            .collect();
        // Casemates low in her side, which neither the deck guns nor the gunhouses above
        // stand in the way of.
        if beam >= CASEMATE_BEAM {
            deck.extend(sections.iter().map(|&at| {
                let fore = middle(at);
                (self.deck_edge(fore), fore, CASEMATE_ARC, Emplacement::Casemate)
            }));
        }
        let mut open: Vec<(f32, f32)> = sections
            .iter()
            .filter(|&&at| !housed(at))
            .flat_map(|&at| {
                let (start, end) = self.span(at);
                let count = ((end - start) / WING_PITCH).round().max(1.) as usize;
                (0..count)
                    .map(move |k| DATUM - start - (end - start) * (k as f32 + 0.5) / count as f32)
            })
            .filter_map(|fore| {
                let abeam = WING_ABEAM.min(self.deck_edge(fore) - WING_MARGIN);
                (abeam >= WING_NARROWEST && clear(abeam, fore)).then_some((abeam, fore))
            })
            .collect();
        open.sort_by(|a, b| (a.1 - bridge).abs().total_cmp(&(b.1 - bridge).abs()));
        for (abeam, fore) in open {
            let room = deck
                .iter()
                .filter(|g| g.3 == Emplacement::Deck)
                .all(|g| (g.1 - fore).abs() >= WING_CLEARANCE);
            if room {
                deck.push((abeam, fore, BEAM_ARC, Emplacement::Deck));
            }
        }
        deck.truncate(shipvivor_artillery::Secondaries::MOST_PER_SIDE);
        let pairs = |(abeam, fore, (home, half_arc), emplacement): (f32, f32, (f32, f32), Emplacement)| {
            [-1., 1.].map(|side| WingStation {
                abeam: side * abeam * beam,
                fore,
                home: side * home,
                arc: (-half_arc, half_arc),
                emplacement,
            })
        };
        WingLayout {
            deck: deck.into_iter().flat_map(pairs).collect(),
            gunhouses: gunhouses
                .into_iter()
                .map(|(_, abeam, fore)| (abeam, fore, BEAM_ARC, Emplacement::Gunhouse))
                .flat_map(pairs)
                .collect(),
        }
    }

    /// Deck guns a side her deck has room for, with no secondary gunhouses aboard.
    pub fn wing_room(&self) -> usize {
        self.wing_stations().len() / 2
    }

    /// Deck guns a side her deck has room for with `houses` secondary gunhouses a side.
    pub fn deck_gun_room(&self, houses: usize) -> usize {
        self.wing_layout(houses).deck.len() / 2
    }

    /// Secondary gunhouses a side her deck has room for: one on each open section.
    pub fn gunhouse_room(&self) -> usize {
        self.wing_layout(usize::MAX).gunhouses.len() / 2
    }

    /// Where her light AA mounts stand, in the order they are fitted, a port mount
    /// before its starboard mate. The pairs she was built with come first: on the bridge
    /// wings, then abaft the after funnel and between the funnels, each going where the
    /// yard moves the bridge or the machinery. Then a pair raised on a column at the
    /// forward end of each open section, nearest the bridge first. A longer hull has
    /// room for more. Only the player's hull carries them.
    pub fn aa_stations(&self) -> Vec<AaStation> {
        let moved = |at: Option<usize>, stock_module: f32| {
            at.map_or(0., |at| {
                self.span(at).0 - stock_module * Frame::Straight.spec().length
            })
        };
        let bridge = self
            .modules()
            .iter()
            .position(|m| m.fit.contains(&Component::Director));
        let (bridge, machinery) = (moved(bridge, 2.), moved(self.machinery(), 3.));
        // Starboard mounts: metres out at the stock beam, forward of the datum, and
        // whether it stands on a bridge wing.
        let mut mounts: Vec<(f32, f32, bool)> = vec![
            (1.3, 1.3 - bridge, true),
            (1.0, -4.85 - machinery, false),
            (1.45, -1.6 - machinery, false),
        ];
        let wings = mounts[0].1;
        let last = self.modules().len().saturating_sub(1);
        let mut open: Vec<(f32, f32)> = (1..last)
            .filter(|&at| self.open_deck(at))
            .map(|at| {
                let (start, end) = self.span(at);
                DATUM - start - 0.25 * (end - start)
            })
            .filter_map(|fore| {
                let abeam = AA_ABEAM.min(self.deck_edge(fore) - AA_MARGIN);
                (abeam >= WING_NARROWEST).then_some((abeam, fore))
            })
            .collect();
        open.sort_by(|a, b| (a.1 - wings).abs().total_cmp(&(b.1 - wings).abs()));
        for (abeam, fore) in open {
            if mounts.iter().all(|m| (m.1 - fore).abs() >= AA_CLEARANCE) {
                mounts.push((abeam, fore, false));
            }
        }
        mounts.truncate(shipvivor_artillery::AntiAir::MOST_MOUNTS / 2);
        let beam = self.beam_factor();
        mounts
            .into_iter()
            .flat_map(|(abeam, fore, bridge_wing)| {
                [-1., 1.].map(|side| AaStation {
                    abeam: side * abeam * beam,
                    fore,
                    bridge_wing,
                })
            })
            .collect()
    }

    /// AA mounts her deck has room for, in port and starboard pairs.
    pub fn aa_room(&self) -> usize {
        self.aa_stations().len()
    }

    /// Module `at` leaves its deck edge clear for secondaries.
    fn open_deck(&self, at: usize) -> bool {
        let module = &self.modules()[at];
        !module.frame.abreast()
            && module.frame != Frame::Sponsons
            && !module.fit.iter().any(|c| {
                matches!(
                    c,
                    Component::Director
                        | Component::TorpedoRack
                        | Component::Catapult
                        | Component::MissileRack
                )
            })
    }

    /// Metres from the centreline to her deck edge `fore` metres forward of her datum,
    /// at the stock beam: multiply by [`Design::beam_factor`] for hers.
    pub fn deck_edge(&self, fore: f32) -> f32 {
        let extension = self.length() - Self::BREAKWATER_LENGTH;
        let edge: Vec<(f32, f32)> = HULL_OUTLINE[..6]
            .iter()
            .map(|&(x, z)| (x, if z >= HULL_SPLICE { z + extension } else { z }))
            .collect();
        let aft = -fore;
        edge.windows(2).find(|w| aft <= w[1].1).map_or(0., |w| {
            let t = ((aft - w[0].1) / (w[1].1 - w[0].1)).clamp(0., 1.);
            w[0].0 + (w[1].0 - w[0].0) * t
        })
    }
}

#[cfg(test)]
mod wing_tests {
    use crate::{Component::*, Design, Emplacement, Frame, Module};

    /// Her secondaries a side, each as metres out and forward of her datum.
    fn starboard(design: &Design) -> Vec<(f32, f32)> {
        let stations = design.wing_stations();
        for pair in stations.chunks(2) {
            assert_eq!(pair[0].abeam, -pair[1].abeam);
            assert_eq!(pair[0].fore, pair[1].fore);
            assert_eq!(pair[0].home, -pair[1].home);
        }
        stations
            .iter()
            .skip(1)
            .step_by(2)
            .map(|s| (s.abeam, s.fore))
            .collect()
    }

    #[test]
    fn a_longer_hull_has_deck_room_for_more_secondaries() {
        let stock = Design::breakwater();
        // Five a side: the forward turret keeps its beams clear.
        assert_eq!(stock.wing_room(), 5);
        // The pairs she was built with come first, where they always stood.
        assert_eq!(
            starboard(&stock)[..3],
            [(2.18, 0.), (1.35, 10.6), (2.18, 3.8)]
        );
        let mut long = stock.clone();
        for _ in 0..6 {
            let at = long.modules().len() - 2;
            long.insert(at, Module::new(Frame::Straight, [])).unwrap();
        }
        // Two a side on each empty bay.
        assert_eq!(long.wing_room(), 5 + 2 * 6);
        let guns = starboard(&long);
        for (i, &(abeam, fore)) in guns.iter().enumerate() {
            assert!(
                abeam > 1. && abeam < long.deck_edge(fore),
                "{i}: {abeam} at {fore}"
            );
            for &(_, other) in &guns[..i] {
                assert!((other - fore).abs() >= 2., "{fore} beside {other}");
            }
        }
    }

    #[test]
    fn a_longer_hull_has_room_for_more_aa_tubs() {
        let stock = Design::breakwater();
        let tubs = stock.aa_stations();
        // The bridge wings first, where they always stood, then the funnels.
        assert_eq!(
            (tubs[1].abeam, tubs[1].fore, tubs[1].bridge_wing),
            (1.3, 1.3, true)
        );
        assert_eq!((tubs[3].abeam, tubs[3].fore), (1.0, -4.85));
        assert_eq!(stock.aa_room(), 8);
        let mut long = stock.clone();
        for _ in 0..6 {
            let at = long.modules().len() - 2;
            long.insert(at, Module::new(Frame::Straight, [])).unwrap();
        }
        // A pair on each empty bay but the first, where the after funnel's pair stands,
        // and on the after turret's bay, which the bays move clear of that pair.
        assert_eq!(long.aa_room(), 8 + 2 * 6);
        let tubs: Vec<_> = long.aa_stations().into_iter().skip(1).step_by(2).collect();
        for (i, tub) in tubs.iter().enumerate() {
            assert!(tub.abeam < long.deck_edge(tub.fore), "{tub:?}");
            for other in &tubs[..i] {
                assert!((other.fore - tub.fore).abs() >= 1.5, "{tub:?} by {other:?}");
            }
        }
    }

    #[test]
    fn galleries_catapults_and_racks_keep_their_deck_edge() {
        let stock = Design::breakwater();
        for module in [
            Module::new(Frame::BatteryGallery, [MainGun, MainGun]),
            Module::new(Frame::DoubleBattery, [MainGun, MainGun]),
            Module::new(Frame::Straight, [Catapult]),
            Module::new(Frame::Straight, [MissileRack]),
        ] {
            let mut design = stock.clone();
            design
                .insert(Design::BREAKWATER_SPLICE, module.clone())
                .unwrap();
            assert_eq!(design.wing_room(), stock.wing_room(), "{module:?}");
        }
        let mut engines = stock.clone();
        engines
            .insert(
                Design::BREAKWATER_SPLICE,
                Module::new(Frame::Straight, [Engine]),
            )
            .unwrap();
        assert_eq!(engines.wing_room(), stock.wing_room() + 2);
    }

    #[test]
    fn a_gun_in_a_row_trains_short_of_the_guns_beside_it() {
        use super::{Obstruction, clear_arcs};
        use std::f32::consts::FRAC_PI_2;
        let built = Design::breakwater().wing_stations();
        let berth = 1.2;
        // The two pairs she is built with stand well apart and keep their whole arcs.
        assert_eq!(clear_arcs(&built[..4], berth, &[]), built[..4]);
        // With every station manned, nowhere in a gun's arc does its fire pass within a
        // berth of another gun.
        let cleared = clear_arcs(&built, berth, &[]);
        for (own, gun) in cleared.iter().enumerate() {
            assert!(gun.arc.0 < 0. && gun.arc.1 > 0., "{gun:?}");
            for step in 0..=40 {
                let bearing = gun.home + gun.arc.0 + (gun.arc.1 - gun.arc.0) * step as f32 / 40.;
                let (east, north) = bearing.sin_cos();
                for (_, other) in cleared.iter().enumerate().filter(|&(i, _)| i != own) {
                    let (abeam, fore) = (other.abeam - gun.abeam, other.fore - gun.fore);
                    let along = abeam * east + fore * north;
                    let wide = (abeam * north - fore * east).abs();
                    assert!(along <= 0. || wide >= berth - 1e-3, "{gun:?} {other:?}");
                }
            }
        }
        // The aftermost gun on the starboard beam is cut short towards the bow, where the
        // row stands, and keeps the open end towards the stern.
        let aftermost = (0..built.len())
            .filter(|&i| built[i].home == FRAC_PI_2)
            .min_by(|&a, &b| built[a].fore.total_cmp(&built[b].fore))
            .unwrap();
        assert!(cleared[aftermost].arc.0 > built[aftermost].arc.0 + 0.3);
        assert_eq!(cleared[aftermost].arc.1, built[aftermost].arc.1);
        // A turret at the deck edge astern of it closes that end too.
        let turret = Obstruction {
            abeam: built[aftermost].abeam,
            fore: built[aftermost].fore - 4.,
            radius: 2.,
        };
        let masked = clear_arcs(&built, berth, &[turret])[aftermost];
        assert!(
            (masked.arc.1 - 60_f32.to_radians()).abs() < 1e-4,
            "{masked:?}"
        );
        assert_eq!(masked.arc.0, cleared[aftermost].arc.0);
    }

    #[test]
    fn a_bigger_forward_turret_pushes_the_forecastle_pair_toward_the_stem() {
        let stock = Design::breakwater();
        let forecastle = |design: &Design| starboard(design)[1];
        assert_eq!(forecastle(&stock), (1.35, 10.6));
        let mut big = stock.clone();
        let at = big.gun_stations()[0].berth.module;
        let house = big.modules()[at].gunhouse;
        big.rearm(
            at,
            crate::Gunhouse {
                bore: 14,
                barrels: shipvivor_artillery::TurretLayout::Triple,
                ..house
            },
        )
        .unwrap();
        let turret = big.gun_stations()[0];
        let (abeam, fore) = forecastle(&big);
        assert!(fore > 10.6, "pushed forward: {fore}");
        // Nothing stands beside the turret on its beams.
        for (abeam, fore) in starboard(&big) {
            assert!(
                (abeam - turret.abeam).hypot(fore - turret.fore)
                    >= super::turret_room(turret.gunhouse),
                "{abeam} at {fore}"
            );
        }
    }

    #[test]
    fn a_broader_beam_carries_its_secondaries_out_to_the_deck_edge() {
        let stock = Design::breakwater();
        let mut broad = stock.clone();
        broad.widen(9.5);
        let factor = broad.beam_factor();
        assert!(factor > 1.5);
        // Her casemates aside, which only a broad hull carries.
        let on_deck = |d: &Design| {
            (d.wing_stations().into_iter())
                .filter(|s| s.emplacement == Emplacement::Deck)
                .collect::<Vec<_>>()
        };
        for (narrow, wide) in on_deck(&stock).iter().zip(on_deck(&broad)) {
            assert!((narrow.abeam * factor - wide.abeam).abs() < 1e-4);
            assert_eq!(narrow.fore, wide.fore);
        }
    }

    #[test]
    fn a_battleship_has_casemates_under_her_open_deck_and_gunhouses_take_its_edge() {
        let mut long = Design::breakwater();
        for _ in 0..2 {
            long.insert(4, Module::new(Frame::Straight, [])).unwrap();
        }
        let casemates = |d: &Design, houses: usize| {
            (d.wing_layout(houses).deck.iter())
                .filter(|s| s.emplacement == Emplacement::Casemate)
                .count()
        };
        // A narrow hull has no casemates.
        assert_eq!(casemates(&long, 0), 0);
        let mut broad = long.clone();
        broad.widen(9.5);
        let last = broad.modules().len() - 1;
        let open = (1..last).filter(|&at| broad.open_deck(at)).count();
        // One a side under each open section, gunhouses or none.
        assert_eq!(casemates(&broad, 0), 2 * open);
        assert!(broad.gunhouse_room() >= 2);
        assert_eq!(casemates(&broad, 2), 2 * open);
        // Each gunhouse takes the deck edge from the deck guns, where a turret has not
        // already kept them off it.
        let most = broad.gunhouse_room();
        for houses in 0..most {
            assert!(broad.deck_gun_room(houses + 1) <= broad.deck_gun_room(houses));
        }
        assert!(broad.deck_gun_room(most) < broad.deck_gun_room(0));
        let layout = broad.wing_layout(2);
        assert_eq!(layout.gunhouses.len(), 4);
        assert!(
            (layout.gunhouses.iter()).all(|s| s.emplacement == Emplacement::Gunhouse)
        );
        // A casemate is cut in her side, at her deck edge, and covers its own beam.
        let casemate = (layout.deck.iter())
            .find(|s| s.emplacement == Emplacement::Casemate)
            .unwrap();
        let edge = broad.deck_edge(casemate.fore) * broad.beam_factor();
        assert!((casemate.abeam.abs() - edge).abs() < 1e-4);
        assert!(casemate.arc.1 < 1.1);
    }
}
