//! Her upperworks amidships, built to her class and to the line she started in: the
//! raised deck from the bridge aft over the machinery, the bridge standing on it, the
//! funnels and the after director. The yard can insert and move sections, so all of it
//! is laid out on the design: it runs from the bridge's section aft over the clear
//! sections, the machinery among them, and stops short of any section carrying a turret,
//! a catapult or a missile rack, and of the turrets either side. None of it changes where
//! anything stands in the battle.
//!
//! The light cruiser's line grows the tall US look: a raised deck, a stepped tower bridge,
//! tall funnels and an after director. The heavy destroyer's line stays squat and armoured
//! at every class: a sloped casemate, a low armoured pilot house behind a broad conning
//! tower, one short trunked uptake with grilles and a heavy belt along her waterline.
use super::{Facets, fittings, hull::deck_height};
use bevy::prelude::*;
use shipvivor_campaign::{Fittings, ShipBeam, StartingHull};
use shipvivor_encounter::{DeckOffsets, Hull};
use shipvivor_shipyard::{Component, Design, HULL_OUTLINE, HULL_SPLICE};

/// Height of her main deck amidships, where the sheer has not begun.
const DECK: f32 = 1.55;
/// Where the bridge model's lowest tier stands, its fore and after ends, the height of its
/// wing deck and its roof, and where its mast stands, all as it is built.
const BRIDGE_FOOT: f32 = 1.53;
const BRIDGE_FORE: f32 = -5.07;
const BRIDGE_AFT: f32 = 0.87;
const BRIDGE_HALF: f32 = 1.83;
const WING_DECK: f32 = 3.04;
const BRIDGE_ROOF: f32 = 4.57;
const MAST_Z: f32 = -0.85;
/// The bridge model's masthead and yard, as it is built.
const MAST_HEAD: f32 = 8.2;
const MAST_YARD: (f32, f32) = (6.71, 1.68);
/// Metres from the centreline to her waist rail at the stock beam.
const RAIL: f32 = 2.55;
/// How far in an armoured casemate's sides lean for each metre they rise.
const CASEMATE_SLOPE: f32 = 0.42;

/// How she looks: her hull class, and whether she is of the heavy destroyer's armoured
/// line or the light cruiser's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Style {
    pub class: ShipBeam,
    pub armoured: bool,
}

/// The parts of a style: the raised deck's height above the main deck and its half
/// width at the stock beam, the deckhouse under the bridge and funnels, the step of the
/// tower under the bridge, and the bridge's size against the light cruiser's.
struct Proportions {
    raised: f32,
    half: f32,
    pedestal: f32,
    tier: f32,
    bridge: f32,
    /// Each funnel's lip, metres aft of the machinery's stock place, and its size.
    funnels: &'static [(f32, [f32; 3])],
    rake: f32,
}

impl Style {
    pub(crate) fn of(design: &Design, hull: StartingHull) -> Self {
        let beam = design.beam();
        let class = ShipBeam::ALL
            .into_iter()
            .rev()
            .find(|class| beam >= class.metres() - 0.5)
            .unwrap_or_default();
        Self {
            class,
            armoured: hull == StartingHull::HeavyDestroyer,
        }
    }

    fn proportions(self) -> Proportions {
        let all = f32::INFINITY;
        match (self.armoured, self.class) {
            // A long low casemate, one short trunked uptake.
            (true, ShipBeam::Escort) => Proportions {
                raised: 0.7,
                half: all,
                pedestal: 0.,
                tier: 0.,
                bridge: 0.95,
                funnels: &[(1.4, [1.8, 1.9, 2.5])],
                rake: 0.04,
            },
            (true, ShipBeam::HeavyCruiser) => Proportions {
                raised: 0.9,
                half: all,
                pedestal: 0.6,
                tier: 0.,
                bridge: 1.05,
                funnels: &[(1.6, [2.2, 2.2, 3.0])],
                rake: 0.04,
            },
            (true, ShipBeam::Battleship) => Proportions {
                raised: 1.05,
                half: all,
                pedestal: 0.75,
                tier: 0.,
                bridge: 1.15,
                funnels: &[(1.8, [2.6, 2.4, 3.5])],
                rake: 0.03,
            },
            (true, ShipBeam::HeavyBattleship) => Proportions {
                raised: 1.2,
                half: all,
                pedestal: 0.85,
                tier: 0.,
                bridge: 1.25,
                funnels: &[(2.0, [2.9, 2.6, 3.9])],
                rake: 0.03,
            },
            // Chunkier still: a taller casemate and a fat uptake.
            (true, ShipBeam::UltraHeavyBattleship) => Proportions {
                raised: 1.35,
                half: all,
                pedestal: 0.95,
                tier: 0.,
                bridge: 1.35,
                funnels: &[(2.2, [3.4, 2.8, 4.5])],
                rake: 0.03,
            },
            (false, ShipBeam::Escort) => Proportions {
                raised: 0.8,
                half: all,
                pedestal: 0.45,
                tier: 0.,
                bridge: 1.,
                funnels: &[(0.5, [1.73, 3.3, 1.98]), (2.75, [1.73, 3.3, 1.98])],
                rake: 0.1,
            },
            (false, ShipBeam::HeavyCruiser) => Proportions {
                raised: 0.85,
                half: all,
                pedestal: 0.7,
                tier: 0.6,
                bridge: 1.1,
                funnels: &[(0.6, [1.9, 3.6, 2.2]), (3.3, [1.9, 3.6, 2.2])],
                rake: 0.12,
            },
            (false, ShipBeam::Battleship) => Proportions {
                raised: 1.,
                half: all,
                pedestal: 0.85,
                tier: 0.85,
                bridge: 1.2,
                funnels: &[(1.6, [2.6, 4.5, 3.4])],
                rake: 0.08,
            },
            (false, ShipBeam::HeavyBattleship) => Proportions {
                raised: 1.1,
                half: all,
                pedestal: 0.95,
                tier: 1.,
                bridge: 1.3,
                funnels: &[(0.9, [2.3, 4.6, 2.6]), (4.1, [2.3, 4.6, 2.6])],
                rake: 0.06,
            },
            // The heavy battleship's lines built broader and higher everywhere: a tall
            // bridge tower on a deep superstructure, and two fat funnels.
            (false, ShipBeam::UltraHeavyBattleship) => Proportions {
                raised: 1.25,
                half: all,
                pedestal: 1.05,
                tier: 1.15,
                bridge: 1.45,
                funnels: &[(0.9, [2.9, 5.1, 3.2]), (4.5, [2.9, 5.1, 3.2])],
                rake: 0.05,
            },
        }
    }

    /// A cruiser-line hull of a heavy cruiser's class or broader, with a conning tower
    /// and an after director.
    fn capital(self) -> bool {
        !self.armoured && self.class != ShipBeam::Escort
    }
}

/// A stretch of raised deck or deckhouse: its fore and after ends, its half width at
/// its foot, how far its roof stands above the main deck, and how far in its sides lean
/// for each metre they rise.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Block {
    fore: f32,
    aft: f32,
    half: f32,
    rise: f32,
    slope: f32,
}

/// One funnel: its lip, its size and its rake.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Stack {
    pub lip: Vec3,
    pub size: Vec3,
    pub rake: f32,
}
impl Stack {
    /// Its fore and after ends along the deck.
    fn reach(&self) -> (f32, f32) {
        let half = 0.52 * self.size.z;
        (
            self.lip.z - self.rake * self.size.z - half,
            self.lip.z + half,
        )
    }
}

/// Her upperworks as her design and class lay them out, in her own frame.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Look {
    pub style: Style,
    beam: f32,
    extension: f32,
    blocks: Vec<Block>,
    /// The bridge model's place: its offset and its scale.
    bridge_offset: Vec3,
    bridge_scale: Vec3,
    pub funnels: Vec<Stack>,
    /// Where she carries her amidships tubes on the centreline, trained out to fire: an
    /// armoured hull's abaft her uptake.
    pub centre_tubes: Option<f32>,
    /// The conning tower before the bridge, and the after director: where each stands.
    conning: Option<Vec3>,
    after: Option<(Vec3, f32)>,
}

impl Look {
    pub(crate) fn of(design: &Design, hull: StartingHull, fittings: &Fittings) -> Self {
        let style = Style::of(design, hull);
        let p = style.proportions();
        let beam = design.beam_factor();
        let extension = design.length() - Design::BREAKWATER_LENGTH;
        let offsets = DeckOffsets::for_design(design);
        let modules = design.modules();
        let last = modules.len().saturating_sub(1);
        let clear = |i: usize| {
            i > 0
                && i < last
                && !modules[i].fit.iter().any(|c| {
                    matches!(
                        c,
                        Component::MainGun | Component::Catapult | Component::MissileRack
                    )
                })
        };
        // The clear sections round `at`, and the stretch of deck they cover.
        let run_of = |at: usize| -> Option<(f32, f32)> {
            if !clear(at) {
                return None;
            }
            let mut first = at;
            while first > 1 && clear(first - 1) {
                first -= 1;
            }
            let mut end = at;
            while clear(end + 1) {
                end += 1;
            }
            Some((
                design.span(first).0 - Hull::CRUISER.bow,
                design.span(end).1 - Hull::CRUISER.bow,
            ))
        };
        let director = modules
            .iter()
            .position(|m| m.fit.contains(&Component::Director));
        // Each stretch stops short of the turrets either side, as they train.
        let turrets: Vec<(f32, f32)> = design
            .gun_stations()
            .iter()
            .map(|s| {
                let scale = fittings.for_station(s).round().mount_scale();
                (-s.fore, 1.5 * scale + 0.2)
            })
            .collect();
        let trim = |(mut fore, mut aft): (f32, f32)| {
            let middle = 0.5 * (fore + aft);
            for &(z, room) in &turrets {
                if z < middle {
                    fore = fore.max(z + room);
                } else {
                    aft = aft.min(z - room);
                }
            }
            (fore, aft)
        };
        // The stretch with the bridge, and the one with the machinery; often the same.
        let forward = director.and_then(run_of).map(trim);
        // A machinery space between turrets on both sides has no raised deck.
        let machinery = design.machinery().and_then(run_of).map(trim);
        // The raised deck runs from the bridge's section aft, not forward of it.
        let start = director.map(|at| design.span(at).0 - Hull::CRUISER.bow);
        let forward = forward.map(|(fore, aft)| (fore.max(start.unwrap_or(fore)), aft));

        // The bridge, on the raised deck and its deckhouse, grown aft from its front as
        // the hull class grows. An armoured hull's is sunk into its casemate, so that its
        // wing deck stands low.
        let s = p.bridge;
        let depth = if s > 1. { 1. + 0.45 * (s - 1.) } else { s };
        let pivot = if s > 1. { BRIDGE_FORE } else { -2.6 };
        let stands = if forward.is_none() {
            0.
        } else {
            p.raised + p.pedestal + p.tier
        };
        let bridge_scale = Vec3::new(s, s, depth);
        let foot = if style.armoured {
            DECK + stands + 0.5 * s - WING_DECK * s
        } else {
            DECK + stands - BRIDGE_FOOT * s
        };
        let bridge_offset = Vec3::new(0., foot, offsets.bridge_z + pivot * (1. - depth));
        let local_z = |z: f32| bridge_offset.z + depth * z;
        let (bridge_fore, bridge_aft) = (local_z(BRIDGE_FORE), local_z(BRIDGE_AFT));
        let mast = local_z(MAST_Z);

        // The funnels stand on the machinery, clear of the bridge's mast.
        let mut funnels: Vec<Stack> = p
            .funnels
            .iter()
            .map(|&(z, size)| Stack {
                lip: Vec3::new(0., 0., z + offsets.machinery_z),
                size: Vec3::from(size),
                rake: p.rake,
            })
            .collect();
        if design.machinery().is_some()
            && let Some(first) = funnels.first()
        {
            let room = mast + 0.4 - first.reach().0;
            let near = (first.lip.z - mast).abs() < 6.;
            if room > 0. && near {
                for stack in &mut funnels {
                    stack.lip.z += room;
                }
            }
        } else {
            funnels.clear();
        }

        let slope = if style.armoured { CASEMATE_SLOPE } else { 0. };
        let mut blocks = Vec::new();
        let raised = |(fore, aft): (f32, f32), blocks: &mut Vec<Block>| {
            if aft - fore > 0.5 {
                blocks.push(Block {
                    fore,
                    aft,
                    half: p.half * beam,
                    rise: p.raised,
                    slope,
                });
            }
        };
        if let Some(run) = forward {
            raised(run, &mut blocks);
        }
        if let Some(run) = machinery.filter(|&m| Some(m) != forward) {
            raised(run, &mut blocks);
        }
        let funnels_aft = funnels.iter().map(|f| f.reach().1).fold(f32::MIN, f32::max);
        let mut conning = None;
        let mut after = None;
        if p.pedestal > 0.
            && let Some((fore, aft)) = forward
        {
            // The deckhouse under the bridge and the funnels, broader than the bridge and
            // clear of the secondaries beside it.
            let end = if machinery == forward && !funnels.is_empty() {
                funnels_aft + 0.5
            } else {
                bridge_aft + 0.3
            };
            let house = BRIDGE_HALF * s + 0.06;
            let half = house.max((house + 0.35 * beam).min(2.18 * beam - 0.9));
            blocks.push(Block {
                fore: (bridge_fore - 0.1).max(fore),
                aft: end.min(aft),
                // A sloped casemate's foot stands out as far as its sides lean in.
                half: half + slope * (p.raised + p.pedestal),
                rise: p.raised + p.pedestal,
                slope,
            });
            // The tower the bridge stands on, a step narrower again.
            if p.tier > 0. {
                blocks.push(Block {
                    fore: (bridge_fore - 0.3).max(fore),
                    aft: (bridge_aft + 0.3).min(aft),
                    half: BRIDGE_HALF * s + 0.22,
                    rise: p.raised + p.pedestal + p.tier,
                    slope: 0.,
                });
            }
        }
        if let Some((fore, aft)) = forward
            && (style.capital() || style.armoured)
        {
            // The conning tower stands before the bridge, on the raised deck: an armoured
            // hull's broad and low.
            let radius = if style.armoured { 0.9 } else { 0.62 } * s;
            let at = if style.armoured { 0.1 } else { -0.25 };
            conning = Some(Vec3::new(
                0.,
                DECK + p.raised,
                (bridge_fore + at).max(fore + radius + 0.05),
            ));
            // An after deckhouse with its director and mainmast, where there is room.
            let end = if machinery == forward && !funnels.is_empty() {
                funnels_aft + 0.5
            } else {
                bridge_aft + 0.3
            };
            let from = end.max(bridge_aft + 0.3) + 0.3;
            let to = (from + 3.2).min(aft - 0.15);
            if style.capital() && to - from >= 2. {
                let rise = p.raised + 0.8 * p.pedestal;
                blocks.push(Block {
                    fore: from,
                    aft: to,
                    half: 1.25 * beam,
                    rise,
                    slope: 0.,
                });
                after = Some((Vec3::new(0., DECK + rise, 0.5 * (from + to)), to - from));
            }
        }
        let mut look = Self {
            style,
            beam,
            extension,
            blocks,
            bridge_offset,
            bridge_scale,
            funnels,
            centre_tubes: None,
            conning,
            after,
        };
        for i in 0..look.funnels.len() {
            let lip = look.funnels[i].lip;
            let foot = look.floor(0., lip.z);
            look.funnels[i].lip.y = foot;
        }
        if style.armoured
            && let Some(uptake) = look.funnels.last()
        {
            look.centre_tubes = Some(uptake.reach().1 + 0.95);
        }
        look
    }

    /// Whether the bridge model, with its open wheelhouse, is shown: an armoured hull
    /// has her own low pilot house in its place.
    pub(crate) fn open_bridge(&self) -> bool {
        !self.style.armoured
    }

    /// Metres from the centreline to her deck edge at `z`, at her beam.
    fn deck_edge(&self, z: f32) -> f32 {
        let edge = HULL_OUTLINE[..6].iter().map(|&(x, at)| {
            (
                x,
                if at >= HULL_SPLICE {
                    at + self.extension
                } else {
                    at
                },
            )
        });
        let edge: Vec<(f32, f32)> = edge.collect();
        let x = edge.windows(2).find(|w| z <= w[1].1).map_or(0., |w| {
            let t = ((z - w[0].1) / (w[1].1 - w[0].1)).clamp(0., 1.);
            w[0].0 + (w[1].0 - w[0].0) * t
        });
        x * self.beam
    }

    /// Metres from the centreline to her side plating `y` above the waterline at `z`,
    /// as her hull's strakes are drawn in towards the keel.
    fn side(&self, y: f32, z: f32) -> f32 {
        let rows = [
            (-1.9, 0.62),
            (0.18, 0.885),
            (0.48, 0.91),
            (DECK - 0.105, 0.997),
            (DECK, 1.),
        ];
        let k = rows.windows(2).find(|w| y <= w[1].0).map_or(1., |w| {
            let t = ((y - w[0].0) / (w[1].0 - w[0].0)).clamp(0., 1.);
            w[0].1 + (w[1].1 - w[0].1) * t
        });
        k * self.deck_edge(z / k)
    }

    /// Half the width of `block` at its foot at `z`. The raised deck reaches out to just
    /// inside her deck rail, so the secondaries kept inside the rail stand on it; a
    /// deckhouse is narrower, and both narrow with the hull.
    fn half_at(&self, block: &Block, z: f32) -> f32 {
        let edge = self.deck_edge(z);
        if block.half.is_infinite() {
            (0.97 * edge).min(RAIL * self.beam) - 0.08
        } else {
            block.half.min(edge - 0.32)
        }
    }

    /// Half the width of `block`'s roof at `z`, inside its sloped sides.
    fn roof_at(&self, block: &Block, z: f32) -> f32 {
        self.half_at(block, z) - block.slope * block.rise
    }

    /// The height of whatever she has to stand on at `x` to starboard and `z` aft of her
    /// datum: the roof of the raised deck or a deckhouse, or her main deck.
    pub(crate) fn floor(&self, x: f32, z: f32) -> f32 {
        let deck = deck_height(z);
        self.blocks
            .iter()
            .filter(|b| (b.fore..=b.aft).contains(&z) && x.abs() <= self.roof_at(b, z))
            .map(|b| deck + b.rise)
            .fold(deck, f32::max)
    }

    /// The height of the bridge wing deck, where its AA tubs stand.
    pub(crate) fn wing_deck(&self) -> f32 {
        self.bridge_offset.y + self.bridge_scale.y * WING_DECK
    }

    /// A part of the bridge built at `at` with `size`, moved where the bridge stands.
    pub(crate) fn on_bridge(&self, at: Vec3, size: Vec3) -> Transform {
        Transform::from_translation(self.bridge_offset + self.bridge_scale * at)
            .with_scale(self.bridge_scale * size)
    }

    /// A point of the bridge model as it is built, where the bridge stands.
    fn bridge_point(&self, at: Vec3) -> Vec3 {
        self.bridge_offset + self.bridge_scale * at
    }

    /// The raised deck, the deckhouses, the funnels and the after director, as one mesh
    /// in her own frame, and an armoured hull's belt, casemate and pilot house.
    pub(crate) fn mesh(&self) -> Mesh {
        let mut mesh = Facets::default();
        let rgb = Color::srgb_u8;
        let armoured = self.style.armoured;
        let wall = if armoured {
            rgb(92, 110, 117)
        } else {
            rgb(110, 134, 140)
        };
        let shade = rgb(68, 87, 96);
        let roof = if armoured {
            rgb(118, 116, 104)
        } else {
            rgb(150, 142, 121)
        };
        let enamel = rgb(181, 196, 189);
        let iron = rgb(54, 65, 69);
        let scuttle = rgb(33, 49, 57);
        let armour = rgb(63, 75, 81);
        for (n, block) in self.blocks.iter().enumerate() {
            let steps = ((block.aft - block.fore) / 0.5).ceil().max(1.) as usize;
            let at = |k: usize| {
                let z = block.fore + (block.aft - block.fore) * k as f32 / steps as f32;
                let deck = deck_height(z);
                let foot = self.half_at(block, z);
                // The foot of a sloped side stands below the deck, so it is drawn out
                // that far again.
                (
                    z,
                    foot + 0.06 * block.slope,
                    self.roof_at(block, z),
                    deck - 0.06,
                    deck + block.rise,
                )
            };
            for k in 0..steps {
                let (za, fa, ha, ba, ta) = at(k);
                let (zb, fb, hb, bb, tb) = at(k + 1);
                for side in [-1., 1.] {
                    let mut q = [
                        Vec3::new(side * fa, ba, za),
                        Vec3::new(side * ha, ta, za),
                        Vec3::new(side * hb, tb, zb),
                        Vec3::new(side * fb, bb, zb),
                    ];
                    if side < 0. {
                        q.reverse();
                    }
                    mesh.quad(q, wall);
                    // A dark band under the roof's edge.
                    let lean = 0.09 * block.slope;
                    let mut band = [
                        Vec3::new(side * (ha + lean + 0.012), ta - 0.09, za),
                        Vec3::new(side * (ha + 0.012), ta, za),
                        Vec3::new(side * (hb + 0.012), tb, zb),
                        Vec3::new(side * (hb + lean + 0.012), tb - 0.09, zb),
                    ];
                    if side < 0. {
                        band.reverse();
                    }
                    mesh.quad(band, shade);
                }
                mesh.quad(
                    [
                        Vec3::new(-ha, ta, za),
                        Vec3::new(-hb, tb, zb),
                        Vec3::new(hb, tb, zb),
                        Vec3::new(ha, ta, za),
                    ],
                    roof,
                );
            }
            for (k, fore) in [(0, true), (steps, false)] {
                let (z, f, h, b, t) = at(k);
                let mut q = [
                    Vec3::new(-f, b, z),
                    Vec3::new(-h, t, z),
                    Vec3::new(h, t, z),
                    Vec3::new(f, b, z),
                ];
                if !fore {
                    q.reverse();
                }
                mesh.quad(q, wall);
            }
            // Scuttles along each side, a row for each deck it stands; armour has none.
            if block.rise > 0.4 && block.slope == 0. {
                let count = ((block.aft - block.fore) / 1.15).floor() as usize;
                for j in 0..count {
                    let z = block.fore + (block.aft - block.fore) * (j as f32 + 0.5) / count as f32;
                    let h = self.half_at(block, z) + 0.01;
                    let y = deck_height(z) + block.rise - 0.42;
                    for side in [-1., 1.] {
                        let mut q = [
                            Vec3::new(side * h, y - 0.07, z - 0.07),
                            Vec3::new(side * h, y + 0.07, z - 0.07),
                            Vec3::new(side * h, y + 0.07, z + 0.07),
                            Vec3::new(side * h, y - 0.07, z + 0.07),
                        ];
                        if side < 0. {
                            q.reverse();
                        }
                        mesh.quad(q, scuttle);
                    }
                }
            }
            // The first stretch, the raised deck, has a guardrail round its edge.
            if n == 0 || block.half.is_infinite() {
                let rail = |side: f32| -> Vec<Vec3> {
                    (0..=steps)
                        .map(|k| {
                            let (z, _, h, _, t) = at(k);
                            Vec3::new(side * (h - 0.06), t, z)
                        })
                        .collect()
                };
                fittings::guardwires(&mut mesh, &rail(1.), 0.62, enamel);
                fittings::guardwires(&mut mesh, &rail(-1.), 0.62, enamel);
            }
        }
        for stack in &self.funnels {
            fittings::raked_funnel(
                &mut mesh,
                stack.lip,
                stack.size,
                rgb(195, 139, 67),
                false,
                stack.rake,
            );
            if armoured {
                // An armoured casing round the uptake's foot, and grilles over its mouth.
                let (fore, aft) = stack.reach();
                let foot = stack.lip.y - 0.05;
                sloped_box(
                    &mut mesh,
                    (fore - 0.2, aft + 0.2),
                    0.5 * stack.size.x + 0.3,
                    (foot, foot + 0.38 * stack.size.y),
                    (0.35, 0.5),
                    armour,
                    shade,
                );
                let top = stack.lip.y + stack.size.y + 0.03;
                for k in 0..5 {
                    let z = stack.lip.z + stack.size.z * (k as f32 / 4. - 0.5) * 0.72;
                    let x = 0.5 * stack.size.x * 0.82;
                    mesh.tube(
                        Vec3::new(-x, top, z),
                        Vec3::new(x, top, z),
                        0.045,
                        0.045,
                        iron,
                    );
                }
            }
        }
        let s = self.bridge_scale.x;
        if let Some(foot) = self.conning {
            if armoured {
                // A broad, low armoured conning tower, its sides sloped in, with a slit.
                let top = self.wing_deck() + 0.5 * s;
                let r = 0.9 * s;
                let slit = top - 0.3 * s;
                mesh.tube(foot - Vec3::Y * 0.1, foot.with_y(slit), r, r * 0.9, armour);
                mesh.tube(
                    foot.with_y(slit),
                    foot.with_y(slit + 0.12 * s),
                    r * 0.9,
                    r * 0.89,
                    scuttle,
                );
                mesh.tube(
                    foot.with_y(slit + 0.12 * s),
                    foot.with_y(top),
                    r * 0.89,
                    r * 0.72,
                    armour,
                );
            } else {
                // An armoured conning tower with its vision slits, before the bridge.
                let top = self.wing_deck() + 0.35 * s;
                let r = 0.62 * s;
                mesh.tube(foot - Vec3::Y * 0.1, foot.with_y(top - 0.22), r, r, wall);
                mesh.tube(
                    foot.with_y(top - 0.22),
                    foot.with_y(top - 0.1),
                    r * 1.01,
                    r * 1.01,
                    scuttle,
                );
                mesh.tube(foot.with_y(top - 0.1), foot.with_y(top), r, r * 0.9, shade);
            }
        }
        if armoured {
            self.pilot_house(&mut mesh, armour, wall, shade, scuttle, iron);
            self.belt(&mut mesh, armour);
        }
        if let Some((foot, length)) = self.after {
            // The after director on its tub, and the mainmast before it.
            let z = foot.z + 0.25 * length;
            let base = foot.with_z(z);
            mesh.tube(base, base + Vec3::Y * 0.9 * s, 0.5 * s, 0.45 * s, wall);
            let house = base + Vec3::Y * 0.9 * s;
            cuboid(
                &mut mesh,
                house + Vec3::Y * 0.32 * s,
                Vec3::new(0.48, 0.32, 0.62) * s,
                shade,
            );
            mesh.tube(
                house + Vec3::new(-1.1 * s, 0.42 * s, 0.1 * s),
                house + Vec3::new(1.1 * s, 0.42 * s, 0.1 * s),
                0.1 * s,
                0.1 * s,
                iron,
            );
            let mast = foot.with_z(foot.z - 0.3 * length);
            let head = self.bridge_offset.y + self.bridge_scale.y * BRIDGE_ROOF + 1.6 * s;
            mesh.tube(mast, mast.with_y(head), 0.13 * s, 0.07 * s, wall);
            mesh.tube(
                mast.with_y(head - 0.9 * s) - Vec3::X * 1.1 * s,
                mast.with_y(head - 0.9 * s) + Vec3::X * 1.1 * s,
                0.06 * s,
                0.06 * s,
                iron,
            );
            for side in [-1., 1.] {
                mesh.tube(
                    mast.with_y(head - 0.8 * s),
                    mast + Vec3::new(side * 1.05 * self.beam, 0., 0.9),
                    0.018,
                    0.018,
                    iron,
                );
            }
        }
        mesh.finish()
    }

    /// An armoured hull's bridge: a sloped casemate up to the wing deck where the bridge
    /// model's base would stand, a low pilot house on it with a vision slit, where the
    /// rangefinder tower stands, and the mast with its yard, where the masthead flies.
    fn pilot_house(
        &self,
        mesh: &mut Facets,
        armour: Color,
        wall: Color,
        shade: Color,
        slit: Color,
        iron: Color,
    ) {
        let s = self.bridge_scale.x;
        let deck = self.wing_deck();
        let fore = self.bridge_point(Vec3::Z * BRIDGE_FORE).z;
        let aft = self.bridge_point(Vec3::Z * BRIDGE_AFT).z;
        let foot = (self.floor(0., fore) - 0.05).min(deck - 0.3);
        // Broad enough for the AA tubs on its wings.
        let half = (BRIDGE_HALF * s).max(1.3 * self.beam + 0.6);
        sloped_box(
            mesh,
            (fore, aft),
            half,
            (foot, deck),
            (CASEMATE_SLOPE, 0.6),
            wall,
            shade,
        );
        let roof = self.bridge_point(Vec3::Y * BRIDGE_ROOF).y;
        let (front, back) = (
            self.bridge_point(Vec3::Z * -4.6).z,
            self.bridge_point(Vec3::Z * -2.0).z,
        );
        sloped_box(
            mesh,
            (front, back),
            1.25 * s,
            (deck - 0.02, roof),
            (0.3, 0.55),
            armour,
            shade,
        );
        // The vision slit across its face.
        let h = roof - deck;
        let y = deck + 0.72 * h;
        let inset = 0.55 * (y - deck);
        cuboid(
            mesh,
            Vec3::new(0., y, front + inset - 0.01),
            Vec3::new(0.8 * s, 0.06 * s, 0.05),
            slit,
        );
        // The mast abaft it, and its yard.
        let mast = self.bridge_point(Vec3::Z * MAST_Z).z;
        let head = self.bridge_point(Vec3::Y * MAST_HEAD).y;
        let at = |y: f32| Vec3::new(0., y, mast);
        mesh.tube(at(deck - 0.1), at(head), 0.15 * s, 0.06 * s, wall);
        let yard = self.bridge_point(Vec3::Y * MAST_YARD.0).y;
        mesh.tube(
            at(yard) - Vec3::X * MAST_YARD.1 * s,
            at(yard) + Vec3::X * MAST_YARD.1 * s,
            0.06 * s,
            0.04 * s,
            iron,
        );
    }

    /// A heavy armour belt along her waterline amidships, standing proud of her side,
    /// its top bevelled into the plating and its ends faired into it.
    fn belt(&self, mesh: &mut Facets, armour: Color) {
        let (low, high, top) = (-0.25, 0.95, 1.15);
        let fore = -9.;
        let aft = 9. + self.extension;
        let steps = ((aft - fore) / 0.5).ceil() as usize;
        let at = |k: usize| {
            let z = fore + (aft - fore) * k as f32 / steps as f32;
            let fair = ((z - fore).min(aft - z) / 1.5).clamp(0., 1.);
            let proud = 0.02 + 0.13 * fair;
            // The outer face leans with her side, so the belt stands out evenly.
            let outer = |y: f32| self.side(y, z) + proud;
            (
                z,
                outer(low),
                outer(high),
                self.side(top, z),
                self.side(low, z),
            )
        };
        for side in [-1., 1.] {
            let face = |mut q: [Vec3; 4]| {
                if side < 0. {
                    q.reverse();
                }
                q
            };
            let p = |x: f32, y: f32, z: f32| Vec3::new(side * x, y, z);
            for k in 0..steps {
                let (za, la, oa, ta, ia) = at(k);
                let (zb, lb, ob, tb, ib) = at(k + 1);
                mesh.quad(
                    face([
                        p(la, low, za),
                        p(oa, high, za),
                        p(ob, high, zb),
                        p(lb, low, zb),
                    ]),
                    armour,
                );
                mesh.quad(
                    face([
                        p(oa, high, za),
                        p(ta, top, za),
                        p(tb, top, zb),
                        p(ob, high, zb),
                    ]),
                    armour,
                );
                mesh.quad(
                    face([
                        p(ia, low, za),
                        p(la, low, za),
                        p(lb, low, zb),
                        p(ib, low, zb),
                    ]),
                    armour,
                );
            }
            for (k, end) in [(0, true), (steps, false)] {
                let (z, l, o, t, i) = at(k);
                let mut q = [p(i, low, z), p(t, top, z), p(o, high, z), p(l, low, z)];
                if end != (side > 0.) {
                    q.reverse();
                }
                mesh.quad(q, armour);
            }
        }
    }
}

/// A box with sloped sides on the centreline: fore and after ends, half its width at its
/// foot, its foot and roof, and how far its sides and its front lean in for each metre
/// they rise.
fn sloped_box(
    mesh: &mut Facets,
    (fore, aft): (f32, f32),
    half: f32,
    (foot, top): (f32, f32),
    (side, front): (f32, f32),
    wall: Color,
    roof: Color,
) {
    let h = top - foot;
    let inner = (half - side * h).max(0.05);
    let front_top = (fore + front * h).min(aft - 0.05);
    let low = [
        Vec3::new(-half, foot, fore),
        Vec3::new(half, foot, fore),
        Vec3::new(half, foot, aft),
        Vec3::new(-half, foot, aft),
    ];
    let high = [
        Vec3::new(-inner, top, front_top),
        Vec3::new(inner, top, front_top),
        Vec3::new(inner, top, aft),
        Vec3::new(-inner, top, aft),
    ];
    let inside = Vec3::new(0., 0.5 * (foot + top), 0.5 * (fore + aft));
    let mut face = |q: [Vec3; 4], paint: Color| {
        let normal = (q[2] - q[0]).cross(q[3] - q[1]);
        let middle = q.iter().copied().sum::<Vec3>() / 4.;
        let mut q = q;
        if normal.dot(middle - inside) < 0. {
            q.reverse();
        }
        mesh.quad(q, paint);
    };
    face(high, roof);
    for i in 0..4 {
        let j = (i + 1) % 4;
        face([low[i], low[j], high[j], high[i]], wall);
    }
}

/// A box about `centre`, `half` its size each way.
fn cuboid(mesh: &mut Facets, centre: Vec3, half: Vec3, color: Color) {
    let c = |x: f32, y: f32, z: f32| centre + half * Vec3::new(x, y, z);
    for [a, b, d, e] in [
        [
            c(1., -1., -1.),
            c(1., 1., -1.),
            c(1., 1., 1.),
            c(1., -1., 1.),
        ],
        [
            c(-1., -1., 1.),
            c(-1., 1., 1.),
            c(-1., 1., -1.),
            c(-1., -1., -1.),
        ],
        [
            c(-1., 1., -1.),
            c(-1., 1., 1.),
            c(1., 1., 1.),
            c(1., 1., -1.),
        ],
        [
            c(-1., -1., -1.),
            c(-1., 1., -1.),
            c(1., 1., -1.),
            c(1., -1., -1.),
        ],
        [
            c(1., -1., 1.),
            c(1., 1., 1.),
            c(-1., 1., 1.),
            c(-1., -1., 1.),
        ],
    ] {
        mesh.quad([a, b, d, e], color);
    }
}
