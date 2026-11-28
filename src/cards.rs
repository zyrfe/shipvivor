//! Fitting cards for the refit: each card is a face of data (category, rank, before and
//! after stats) drawn as a framed card that deals in, lifts and glows under the cursor.
use crate::{insignia::Insignia, ui::Action};
use bevy::{
    prelude::*,
    ui::{RelativeCursorPosition, UiGlobalTransform},
    window::{CursorGrabMode, CursorOptions},
};
use shipvivor_campaign::{CALIBRES, Card, Fittings, LOADER_GAIN, Rarity, Run, limits};
use shipvivor_encounter::{ArmorGrade, FireControl, Guidance, TORPEDO_RANGE, TUBE_RELOAD, gunnery};
use shipvivor_navigation::Helm;
use shipvivor_shipyard::{
    BOMB_DAMAGE, Component, Design, INFLOW, MISSILE_DAMAGE, PUMP, Protection,
};
use std::borrow::Cow;

const KNOTS_PER_MPS: f32 = 1.94384;
const WIDTH: f32 = 268.;
/// The most ranks a card shows as diamonds.
const MOST_DIAMONDS: u16 = 12;
const HEIGHT: f32 = 380.;
/// Seconds between one card landing and the next.
const STAGGER: f32 = 0.09;
const DEAL: f32 = 0.38;
/// How far under the cards a fresh hand leaves the pointer, in UI pixels.
const PARK_GAP: f32 = 24.;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Ammunition,
    Gunnery,
    Machinery,
    Protection,
    DeckGuns,
    Secondaries,
    AntiAir,
    StrikeCraft,
    Torpedoes,
}
impl Kind {
    fn label(self) -> &'static str {
        match self {
            Self::Ammunition => "AMMUNITION",
            Self::Gunnery => "MAIN BATTERY",
            Self::Machinery => "MACHINERY",
            Self::Protection => "PROTECTION",
            Self::DeckGuns => "DECK GUNS",
            Self::Secondaries => "SECONDARIES",
            Self::AntiAir => "ANTI-AIRCRAFT",
            Self::StrikeCraft => "STRIKE CRAFT",
            Self::Torpedoes => "TORPEDOES",
        }
    }
    fn color(self) -> Color {
        match self {
            Self::Ammunition => Color::srgb_u8(255, 176, 74),
            Self::Gunnery => Color::srgb_u8(240, 104, 88),
            Self::Machinery => Color::srgb_u8(88, 204, 214),
            Self::Protection => Color::srgb_u8(200, 208, 218),
            Self::DeckGuns => Color::srgb_u8(132, 224, 120),
            Self::Secondaries => Color::srgb_u8(92, 200, 170),
            Self::AntiAir => Color::srgb_u8(150, 158, 255),
            Self::StrikeCraft => Color::srgb_u8(236, 150, 226),
            Self::Torpedoes => Color::srgb_u8(234, 222, 96),
        }
    }
}

/// Whether the change helps, costs, or adds something new.
#[derive(Clone, Copy)]
pub(crate) enum Change {
    Gain,
    Cost,
    New,
}
impl Change {
    pub(crate) fn color(self) -> Color {
        match self {
            Self::Gain => Color::srgb_u8(126, 240, 160),
            Self::Cost => Color::srgb_u8(255, 122, 102),
            Self::New => Color::srgb_u8(255, 212, 112),
        }
    }
}

pub(crate) struct Stat {
    pub(crate) label: Cow<'static, str>,
    /// Empty when the card adds something the ship did not have, or when the figure is
    /// the change itself, such as the share a loader card takes off every reload.
    pub(crate) before: String,
    pub(crate) after: String,
    pub(crate) change: Change,
}
fn stat(
    label: impl Into<Cow<'static, str>>,
    before: impl ToString,
    after: impl ToString,
    change: Change,
) -> Stat {
    Stat {
        label: label.into(),
        before: before.to_string(),
        after: after.to_string(),
        change,
    }
}
/// A loader card's figures: what it adds to the rate of fire, then each gun it drills as
/// its time now and after, labelled.
fn quicker(guns: impl IntoIterator<Item = (Cow<'static, str>, f32, f32)>) -> Vec<Stat> {
    let cut = stat(
        "Fire rate",
        "",
        format!("+{:.0}%", 100. * LOADER_GAIN),
        Change::Gain,
    );
    let each = (guns.into_iter())
        .map(|(label, now, next)| stat(label, seconds(now), seconds(next), Change::Gain));
    std::iter::once(cut).chain(each).collect()
}
/// Every bore among the main turrets aboard, smallest first, each once.
fn bores(design: &Design, f: Fittings) -> Vec<u16> {
    let mut bores: Vec<u16> = (design.gun_stations().iter())
        .map(|station| f.for_station(station).calibre())
        .collect();
    bores.sort_unstable();
    bores.dedup();
    bores
}

pub(crate) struct Face {
    kind: Kind,
    pub(crate) name: &'static str,
    /// Ranks already fitted, and the most there can be when the card has a limit.
    rank: (u16, Option<u16>),
    pub(crate) stats: Vec<Stat>,
    flavour: &'static str,
}
impl Face {
    /// The card adds something the ship cannot do yet, rather than improving it.
    fn unlocks(&self, card: Card) -> bool {
        self.rank.0 == 0
            && matches!(
                card,
                Card::TwinSecondaries
                    | Card::SecondaryTurrets
                    | Card::GuidedTorpedoes
                    | Card::TripleTurrets
                    | Card::QuadTurrets
            )
    }
}

/// The most secondary gunhouses a side her deck has room for while her deck guns keep
/// theirs.
fn room_for_houses(card: Card, design: &Design, now: &Fittings) -> Option<u16> {
    Some(
        shipvivor_campaign::deck_ranks(card, design, now)
            .map_or(limits::WING_TURRETS, |room| room.min(limits::WING_TURRETS)),
    )
}

/// A deck gun's bore: in whole inches, or to the half inch.
fn bore_inches(bore: f32) -> String {
    if bore.fract() == 0. {
        format!("{bore:.0} in")
    } else {
        format!("{bore:.1} in")
    }
}

fn seconds(s: f32) -> String {
    format!("{s:.2} s")
}
fn metres(m: f32) -> String {
    format!("{m:.0} m")
}
/// Degrees per second, spelled out: the default font has no degree sign.
fn traverse(f: Fittings) -> String {
    format!("{:.0} deg/s", f.traverse())
}
fn share(part: f32) -> String {
    format!("{:.0}%", 100. * part)
}
/// The widest round a sound plate turns aside.
fn inches(calibre: f32) -> String {
    format!("{calibre:.0} in")
}
/// The least structure a module can keep and still be held dry by the crew's pumps alone,
/// with these fittings: its leak grows with the cube of the structure lost.
fn holds_dry(f: Fittings) -> String {
    let pumping = PUMP * Design::BREAKWATER_REPAIR * f.pumping();
    share(1. - (pumping / INFLOW).min(1.).cbrt())
}
/// Damage the first such fitting aboard takes before it fails, with these fittings.
fn strength(design: &Design, f: Fittings, part: Component) -> String {
    design
        .protected(f.protection(Protection::BREAKWATER))
        .strength(part)
        .map_or_else(String::new, |points| format!("{points:.0}"))
}
fn knots(f: Fittings) -> String {
    format!(
        "{:.1} kn",
        Helm::FULL_AHEAD * f.speed_factor() * KNOTS_PER_MPS
    )
}
/// How her torpedoes steer with these fittings.
fn guidance(f: Fittings) -> Guidance {
    Guidance::for_rank(f.torpedoes().guidance)
}
/// How hard a torpedo turns toward a ship, or that it runs straight.
fn steering(g: Guidance) -> String {
    if g.guided() {
        format!("{:.0} deg/s", g.turn.to_degrees())
    } else {
        "Straight".into()
    }
}
/// How far out a torpedo's seeker sees a ship, and how wide.
fn seeker(g: Guidance) -> String {
    if g.guided() {
        format!("{:.0} m, {:.0} deg", g.reach, 2. * g.cone.to_degrees())
    } else {
        "None".into()
    }
}
/// How many ships a spread is laid across.
fn targets(g: Guidance) -> String {
    match (g.fan, g.spread) {
        (false, _) => "1 ship".into(),
        (true, n) => format!("{n} ships"),
    }
}
/// The torpedo card's figures: the turn and the seeker always, then how many ships a
/// spread fans out across and how many torpedoes go in it, once those come into it.
fn guidance_stats(now: Fittings, next: Fittings) -> Vec<Stat> {
    let (g, after) = (guidance(now), guidance(next));
    let mut stats = vec![
        stat("Turn", steering(g), steering(after), Change::Gain),
        stat("Seeker", seeker(g), seeker(after), Change::Gain),
    ];
    if after.fan {
        stats.push(stat(
            "Fans out across",
            targets(g),
            targets(after),
            Change::Gain,
        ));
    }
    if after.spread != g.spread {
        stats.push(stat("Spread", g.spread, after.spread, Change::Gain));
    }
    stats
}

/// Metres out at which a card reckons what the main battery's round pierces.
const PIERCING_RANGE: f32 = 400.;
/// Inches of plate the main battery's round goes through, [`PIERCING_RANGE`] out.
fn piercing(f: Fittings) -> f32 {
    let round = f.round();
    round.penetration_at(PIERCING_RANGE / round.speed())
}
fn pierces(f: Fittings) -> String {
    format!("{:.1} in", piercing(f))
}
/// The heaviest plate the main battery's round opens [`PIERCING_RANGE`] out.
fn opens(f: Fittings) -> &'static str {
    [
        ArmorGrade::Capital,
        ArmorGrade::Reinforced,
        ArmorGrade::Light,
    ]
    .into_iter()
    .find(|grade| piercing(f) >= grade.inches())
    .map_or("None", ArmorGrade::label)
}
/// The piercing rows for a card that changes the main battery's round: inches of plate
/// pierced, and the heaviest grade opened once that changes.
fn piercing_stats(now: Fittings, next: Fittings) -> Vec<Stat> {
    let mut stats = vec![stat("Pierces", pierces(now), pierces(next), Change::Gain)];
    if opens(now) != opens(next) {
        stats.push(stat("Opens", opens(now), opens(next), Change::Gain));
    }
    stats
}

pub(crate) fn face(card: Card, now: Fittings, design: &Design) -> Face {
    // A legendary card shows its ordinary card's figures for all the ranks it fits at
    // once, under its own name.
    if let Some((base, ranks)) = card.legend() {
        let (name, flavour) = match card {
            Card::Overdrive => (
                "Overdrive",
                "Every valve screwed down, every governor wired open. The chief engineer has stopped asking questions.",
            ),
            Card::BattleshipPlate => (
                "Battleship Plate",
                "Somebody's battleship is missing her belt. Nobody here knows anything about it.",
            ),
            Card::SuperchargedShells => (
                "Supercharged Shells",
                "Double charges, polished bores, a prayer for the breech blocks. Reach out and touch the horizon.",
            ),
            _ => (
                "Auto-Loaders",
                "A contraption of chains and rams that loads faster than the crew can say \"is that safe\".",
            ),
        };
        let mut face = face_to(base, now, now.with(card), design);
        face.name = name;
        face.flavour = flavour;
        let at_once = stat("Ranks at once", "", format!("x{ranks}"), Change::Gain);
        face.stats.insert(0, at_once);
        return face;
    }
    face_to(card, now, now.with(card), design)
}

/// The face of an ordinary card, going from `now` to `next`.
fn face_to(card: Card, now: Fittings, next: Fittings, design: &Design) -> Face {
    use Change::*;
    let (wing, wing_next) = (now.secondaries(), next.secondaries());
    let (flak, flak_next) = (now.anti_air(), next.anti_air());
    let (craft, craft_next) = (now.strike_craft(), next.strike_craft());
    // The most ranks of a card that wants room which her deck or her plane sections
    // have room for.
    let room = |most: u16| {
        Some(
            shipvivor_campaign::deck_ranks(card, design, &now)
                .map_or(most, |room| most.min(room)),
        )
    };
    match card {
        // [`face`] dresses a legendary card as its ordinary one.
        Card::Overdrive | Card::BattleshipPlate | Card::SuperchargedShells | Card::AutoLoaders => {
            let base = card.legend().map_or(Card::Turbines, |(base, _)| base);
            face_to(base, now, next, design)
        }
        // Turrets of different bores load at their own pace, so each bore aboard has
        // its own line.
        Card::Loader => Face {
            kind: Kind::Gunnery,
            name: "Quick Loaders",
            rank: (now.loader, None),
            stats: quicker(bores(design, now).into_iter().map(|calibre| {
                (
                    Cow::Owned(format!("{calibre} in")),
                    now.reload_for(calibre),
                    next.reload_for(calibre),
                )
            })),
            flavour: "Drilled crews ram the next round home sooner.",
        },
        Card::Shells => Face {
            kind: Kind::Ammunition,
            name: "Larger Bore",
            rank: (
                now.shells,
                Some(
                    CALIBRES
                        .iter()
                        .rposition(|&inches| inches <= now.capacity())
                        .unwrap_or(0) as u16,
                ),
            ),
            stats: vec![
                stat(
                    "Calibre",
                    format!("{:.0} in", now.round().calibre_inches()),
                    format!("{:.0} in", next.round().calibre_inches()),
                    Gain,
                ),
                stat(
                    "Breach",
                    now.round().breach_force(),
                    next.round().breach_force(),
                    Gain,
                ),
                stat(
                    "Reload",
                    seconds(now.reload()),
                    seconds(next.reload()),
                    Cost,
                ),
                stat("Traverse", traverse(now), traverse(next), Cost),
            ]
            .into_iter()
            .chain(piercing_stats(now, next))
            .collect(),
            flavour: "A heavier shell loads and trains more slowly.",
        },
        Card::Caps => Face {
            kind: Kind::Ammunition,
            name: "Piercing Caps",
            rank: (now.caps, Some(limits::CAPS)),
            stats: piercing_stats(now, next),
            flavour: "Hardened caps drive every round through heavier plate.",
        },
        Card::Velocity => Face {
            kind: Kind::Ammunition,
            name: "Shell Velocity",
            rank: (now.velocity, None),
            stats: vec![
                stat(
                    "Speed",
                    format!("{:.0} m/s", now.round().speed()),
                    format!("{:.0} m/s", next.round().speed()),
                    Gain,
                ),
                stat(
                    "Range",
                    metres(now.round().reach()),
                    metres(next.round().reach()),
                    Gain,
                ),
            ]
            .into_iter()
            .chain(piercing_stats(now, next))
            .collect(),
            flavour: "A flatter arc, a shorter lead, a longer reach and a harder blow.",
        },
        Card::Turbines => Face {
            kind: Kind::Machinery,
            name: "Turbine Refit",
            rank: (now.turbines, None),
            stats: vec![stat("Top speed", knots(now), knots(next), Gain)],
            flavour: "More steam to the shafts. Outrun what you cannot outgun.",
        },
        Card::TripleTurrets | Card::QuadTurrets => Face {
            kind: Kind::Gunnery,
            name: if card == Card::TripleTurrets {
                "Triple Turrets"
            } else {
                "Quad Turrets"
            },
            rank: (now.turrets.count().saturating_sub(2) as u16, Some(2)),
            stats: vec![stat(
                "Barrels",
                now.turrets.count(),
                next.turrets.count(),
                Gain,
            )],
            flavour: "Wider gunhouses, more barrels in every salvo.",
        },
        Card::MoreSecondaries => Face {
            kind: Kind::DeckGuns,
            name: "More Deck Guns",
            rank: (now.wing_mounts, room(limits::WING_MOUNTS)),
            stats: vec![stat("Guns a side", wing.per_side, wing_next.per_side, Gain)],
            flavour: "Another gun each beam, out along the deck edge from the bridge, or in a casemate in a big hull's side. A longer hull has room for more.",
        },
        Card::HeavySecondaries => Face {
            kind: Kind::DeckGuns,
            name: "Heavier Deck Guns",
            rank: (now.wing_shells, Some(limits::WING_SHELLS)),
            stats: {
                let mut stats = vec![stat("Damage", wing.damage, wing_next.damage, Gain)];
                if wing_next.bore > wing.bore {
                    stats.push(stat("Bore", bore_inches(wing.bore), bore_inches(wing_next.bore), Gain));
                }
                stats
            },
            flavour: "A heavier gun and round on every deck gun, up to three inches. They whittle a ship down; they never wreck her.",
        },
        Card::QuickSecondaries => Face {
            kind: Kind::DeckGuns,
            name: "Deck Gun Loaders",
            rank: (now.wing_loaders, Some(limits::WING_LOADERS)),
            stats: quicker([("Each gun".into(), wing.reload, wing_next.reload)]),
            flavour: "Ready-use lockers beside every gun.",
        },
        Card::SecondaryRange => Face {
            kind: Kind::DeckGuns,
            name: "Rangefinders",
            rank: (now.wing_range, Some(limits::WING_RANGE)),
            stats: vec![stat(
                "Reach",
                format!("{:.0} m", wing.range),
                format!("{:.0} m", wing_next.range),
                Gain,
            )],
            flavour: "The crew opens fire on boats farther out.",
        },
        Card::TwinSecondaries => Face {
            kind: Kind::DeckGuns,
            name: match wing_next.barrels {
                ..=2 => "Twin Deck Guns",
                3 => "Triple Deck Guns",
                _ => "Quad Deck Guns",
            },
            rank: (now.wing_twin, Some(limits::WING_BARRELS)),
            stats: vec![stat("Barrels", wing.barrels, wing_next.barrels, Gain)],
            flavour: "Another barrel on every deck gun. They all fire together.",
        },
        Card::TrainingGear => Face {
            kind: Kind::Gunnery,
            name: "Training Gear",
            rank: (now.training_gear, Some(limits::TRAINING_GEAR)),
            stats: vec![stat("Traverse", traverse(now), traverse(next), Gain)],
            flavour: "Stronger motors swing every gunhouse onto a new bearing, and lay its guns, sooner.",
        },
        Card::Armor => {
            let (a, b) = (
                now.protection(Protection::BREAKWATER),
                next.protection(Protection::BREAKWATER),
            );
            Face {
                kind: Kind::Protection,
                name: "Armor Plate",
                rank: (now.armor, Some(limits::ARMOR)),
                stats: vec![
                    stat("Plate", share(a.plating), share(b.plating), Gain),
                    stat("Stops", inches(a.thickness), inches(b.thickness), Gain),
                ],
                flavour: "Thicker side plate wears more slowly, and turns aside smaller guns until it is battered thin. A broader beam carries more of it.",
            }
        }
        Card::Hardening => Face {
            kind: Kind::Protection,
            name: "Hardened Systems",
            rank: (now.hardening, Some(limits::HARDENING)),
            stats: vec![
                stat(
                    "Engine",
                    strength(design, now, Component::Engine),
                    strength(design, next, Component::Engine),
                    Gain,
                ),
                stat(
                    "Turret",
                    strength(design, now, Component::MainGun),
                    strength(design, next, Component::MainGun),
                    Gain,
                ),
            ],
            flavour: "Splinter shields and stiffer seatings. Engine, guns and gear take more before they fail.",
        },
        Card::Pumps => Face {
            kind: Kind::Machinery,
            name: "Bilge Pumps",
            rank: (now.pumps, Some(limits::PUMPS)),
            stats: vec![
                stat("Pumping", share(now.pumping()), share(next.pumping()), Gain),
                stat("Holds dry to", holds_dry(now), holds_dry(next), Gain),
            ],
            flavour: "Heavier pumps send the sea back over the side before a holed compartment floods.",
        },
        Card::MoreAntiAir => Face {
            kind: Kind::AntiAir,
            name: "More AA Guns",
            rank: (now.aa_mounts, room(limits::AA_MOUNTS)),
            stats: vec![stat("Guns", flak.mounts, flak_next.mounts, Gain)],
            flavour: "Another pair of guns in tubs raised over the deck. A longer hull has room for more.",
        },
        Card::HeavyAntiAir => Face {
            kind: Kind::AntiAir,
            name: "Heavier Flak",
            rank: (now.aa_shells, Some(limits::AA_SHELLS)),
            stats: vec![stat("Damage", flak.damage, flak_next.damage, Gain)],
            flavour: "A heavier burst tears up whatever it catches.",
        },
        Card::QuickAntiAir => Face {
            kind: Kind::AntiAir,
            name: "AA Loaders",
            rank: (now.aa_loaders, Some(limits::AA_LOADERS)),
            stats: quicker([("Each mount".into(), flak.reload, flak_next.reload)]),
            flavour: "Clips passed up hand to hand keep the guns firing.",
        },
        Card::SecondaryTurrets => {
            let (houses, houses_next) = (now.gunhouses(), next.gunhouses());
            let room = |h: &shipvivor_encounter::Gunhouses| design.deck_gun_room(h.per_side);
            Face {
                kind: Kind::Secondaries,
                // Named for the bore her class of hull carries.
                name: match now.gunhouse_bore() {
                    Some(..=4) | None => "4-inch Secondary Gunhouses",
                    Some(5..=6) => "6-inch Secondary Gunhouses",
                    Some(_) => "8-inch Secondary Gunhouses",
                },
                rank: (now.wing_turrets, room_for_houses(card, design, &now)),
                stats: vec![
                    stat(
                        "Gunhouses a side",
                        houses.per_side,
                        houses_next.per_side,
                        Gain,
                    ),
                    stat("Damage", "", houses_next.damage, Gain),
                    stat("Reach", "", metres(houses_next.range), Gain),
                    stat(
                        "Deck gun room",
                        room(&houses),
                        room(&houses_next),
                        Cost,
                    ),
                ],
                flavour: "A twin gunhouse each beam, laid and fired by its own crew: four-inch on a battleship's hull, six-inch on a heavy battleship's, eight-inch on an ultra heavy battleship's. Each takes the deck edge two deck guns would.",
            }
        }
        Card::MoreLaunchers => Face {
            kind: Kind::StrikeCraft,
            name: "More Launchers",
            rank: (now.launchers, Some(limits::LAUNCHERS)),
            stats: vec![
                stat("Each rack", craft.launchers, craft_next.launchers, Gain),
                stat(
                    "Missiles",
                    craft.launchers * craft.salvo,
                    craft_next.launchers * craft_next.salvo,
                    Gain,
                ),
            ],
            flavour: "Another trainable box on every rack, each ripple-firing at the nearest raider.",
        },
        Card::MoreMissiles => Face {
            kind: Kind::StrikeCraft,
            name: "More Missiles",
            rank: (now.missiles, Some(limits::MISSILES)),
            stats: vec![
                stat("Salvo", craft.salvo, craft_next.salvo, Gain),
                stat(
                    "Damage",
                    craft.salvo as u16 * MISSILE_DAMAGE,
                    craft_next.salvo as u16 * MISSILE_DAMAGE,
                    Gain,
                ),
            ],
            flavour: "A bigger box of cells on every launcher, so each salvo carries one more missile.",
        },
        Card::MorePlanes => Face {
            kind: Kind::StrikeCraft,
            name: "More Planes",
            rank: (now.planes, room(limits::PLANES)),
            stats: vec![stat("Planes", craft.planes, craft_next.planes, Gain)],
            flavour: "Another catapult and floatplane. Each hunts the nearest raider the others are not after.",
        },
        Card::MoreBombs => Face {
            kind: Kind::StrikeCraft,
            name: "More Bombs",
            rank: (now.bombs, Some(limits::BOMBS)),
            stats: vec![
                stat("Bombs", craft.bombs, craft_next.bombs, Gain),
                stat(
                    "Stick",
                    craft.bombs as u16 * BOMB_DAMAGE,
                    craft_next.bombs as u16 * BOMB_DAMAGE,
                    Gain,
                ),
            ],
            flavour: "Every plane drops a stick across its mark instead of a single bomb.",
        },
        Card::GuidedTorpedoes => Face {
            kind: Kind::Torpedoes,
            name: "Guided Torpedoes",
            rank: (now.guidance, Some(limits::GUIDANCE)),
            stats: guidance_stats(now, next),
            flavour: match next.guidance {
                ..=1 => {
                    "A seeker in every warhead turns it onto the ship ahead late in its run. A ship that turns hard away from a long shot can still slip it."
                }
                2 => {
                    "A quicker rudder on every torpedo, so it comes round after a ship that turns."
                }
                3 => {
                    "Fire control lays each torpedo of a spread on a ship of its own, the one aimed at first. Spread the love."
                }
                _ => "Five to a spread, and a seeker that sees wider and farther.",
            },
        },
        Card::QuickTubes => Face {
            kind: Kind::Torpedoes,
            name: "Tube Drill",
            rank: (now.quick_tubes, Some(limits::QUICK_TUBES)),
            stats: quicker([(
                "Each set".into(),
                TUBE_RELOAD * now.torpedoes().reload,
                TUBE_RELOAD * next.torpedoes().reload,
            )]),
            flavour: "Spare torpedoes on trolleys beside the tubes, and a crew drilled to reload them.",
        },
        Card::LongRunningTorpedoes => Face {
            kind: Kind::Torpedoes,
            name: "Long-Running Torpedoes",
            rank: (now.long_run, Some(limits::LONG_RUN)),
            stats: vec![stat(
                "Run",
                metres(TORPEDO_RANGE * now.torpedoes().run),
                metres(TORPEDO_RANGE * next.torpedoes().run),
                Gain,
            )],
            flavour: "A bigger air flask carries every torpedo farther, as fast as ever.",
        },
        Card::SuppressingFire => {
            let held = |f: Fittings, at: f32| gunnery_of(f).suppression(at);
            let line =
                |label, at| stat(label, seconds(held(now, at)), seconds(held(next, at)), Gain);
            let at = gunnery::SUPPRESSION_AT;
            Face {
                kind: Kind::Gunnery,
                name: "Suppressing Fire",
                rank: (now.suppression, Some(limits::SUPPRESSION)),
                stats: vec![
                    line("Crews held", at),
                    line("Close in", 0.5 * at),
                    line("Far out", 1.5 * at),
                ],
                flavour: "Every hit sends her gun crews diving for cover. They hold fire, and load nothing, until they pick themselves up. Farther out, where the fall of shot is hard to read, they keep their heads down longer.",
            }
        }
        Card::LongBaseRangefinders => {
            // What is left of her scatter well out, as a share of what it was.
            let left = |f: Fittings| 1. - gunnery::RANGEFINDER_CUT * f32::from(f.rangefinders);
            Face {
                kind: Kind::Gunnery,
                name: "Long-Base Rangefinders",
                rank: (now.rangefinders, Some(limits::RANGEFINDERS)),
                stats: vec![
                    stat("Scatter far out", share(left(now)), share(left(next)), Gain),
                    stat("Beyond", "", metres(0.5 * next.round().reach()), New),
                ],
                flavour: "A rangefinder as wide as her beam reads the far fall of shot to the yard, and her salvos close up at long range.",
            }
        }
        Card::PlungingFire => {
            let far = gunnery::PLUNGE_RANGE + 1.;
            let (harder, deeper) = gunnery_of(now).plunge(far);
            let (harder_next, deeper_next) = gunnery_of(next).plunge(far);
            let pierce = |deeper: f32| format!("+{:.0}%", 100. * (deeper - 1.));
            Face {
                kind: Kind::Gunnery,
                name: "Plunging Fire",
                rank: (now.plunging, Some(limits::PLUNGING)),
                stats: vec![
                    stat("Plunges beyond", "", metres(gunnery::PLUNGE_RANGE), New),
                    stat(
                        "Extra damage",
                        format!("+{harder}"),
                        format!("+{harder_next}"),
                        Gain,
                    ),
                    stat("Extra pierce", pierce(deeper), pierce(deeper_next), Gain),
                ],
                flavour: "Laid high, her far rounds come down steep through the deck, where the plate is thinnest.",
            }
        }
    }
}

/// The cruiser's gunnery cards as `f` fits them, on a stock crew's scatter.
fn gunnery_of(f: Fittings) -> FireControl {
    FireControl {
        rangefinders: f.rangefinders,
        suppression: f.suppression,
        plunging: f.plunging,
        ..FireControl::STOCK
    }
}

/// The card currently dealt into slot `slot`, easing in and lifting under the cursor.
#[derive(Component)]
pub struct Dealt {
    slot: usize,
    color: Color,
    age: f32,
    lift: f32,
}
impl Dealt {
    /// How far the card has come in, from 0 still in the deck to 1 landed.
    fn progress(&self) -> f32 {
        ((self.age - self.slot as f32 * STAGGER) / DEAL).clamp(0., 1.)
    }
    /// A card takes a click only once it has landed, not while it sweeps in under the pointer.
    pub(crate) fn landed(&self) -> bool {
        self.progress() >= 1.
    }
}

/// The row a hand is dealt into. Once its layout holds still, the pointer is moved to
/// just under the cards, so a click still meant for the guns lands on none of them.
#[derive(Component, Default)]
pub struct Hand {
    /// Where the pointer goes, as measured last frame, in physical pixels.
    settling: Option<Vec2>,
    parked: bool,
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    a.mix(&b, t)
}
fn navy() -> Color {
    Color::srgb_u8(12, 28, 38)
}
fn ink() -> Color {
    Color::srgb_u8(236, 240, 226)
}
fn pale() -> Color {
    Color::srgb_u8(152, 182, 182)
}

fn text(
    cmd: &mut Commands,
    parent: Entity,
    s: impl Into<String>,
    size: f32,
    color: Color,
) -> Entity {
    cmd.spawn((
        Text::new(s.into()),
        TextFont::from_font_size(size),
        TextColor(color),
        ChildOf(parent),
    ))
    .id()
}

/// The gems in a card's corner: one for a common card up to five for a legendary one, in
/// the colours games have long used for them: grey-white, green, blue, purple, orange.
#[derive(Component)]
pub struct Gem;
impl Gem {
    pub(crate) fn count(rarity: Rarity) -> usize {
        1 + Rarity::ALL.iter().position(|&r| r == rarity).unwrap_or(0)
    }
    pub(crate) fn color(rarity: Rarity) -> Color {
        match rarity {
            Rarity::Common => Color::srgb_u8(196, 204, 208),
            Rarity::Uncommon => Color::srgb_u8(64, 214, 92),
            Rarity::Rare => Color::srgb_u8(64, 148, 255),
            Rarity::Epic => Color::srgb_u8(184, 96, 255),
            Rarity::Legendary => Color::srgb_u8(255, 140, 26),
        }
    }
}

/// Lay the hand on offer in a row under `parent`.
pub fn hand(cmd: &mut Commands, parent: Entity, run: &Run, art: &Insignia) {
    let (now, design) = (run.fittings, run.ship.design());
    let row = cmd
        .spawn((
            Hand::default(),
            Node {
                width: percent(100),
                justify_content: JustifyContent::Center,
                column_gap: px(16),
                padding: UiRect::vertical(px(10)),
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    for (slot, card) in run.offers().into_iter().enumerate() {
        let rarity = now.rarity(card);
        spawn_card(cmd, row, slot, face(card, now, design), card, rarity, art);
    }
}

fn spawn_card(
    cmd: &mut Commands,
    row: Entity,
    slot: usize,
    face: Face,
    card: Card,
    rarity: Option<Rarity>,
    art: &Insignia,
) {
    let tint = face.kind.color();
    // An epic or legendary card wears its rarity's colour round its edge and in its glow.
    let edge = rarity
        .filter(|&r| r >= Rarity::Epic)
        .map_or(tint, Gem::color);
    let root = cmd
        .spawn((
            Action::Card(card),
            RelativeCursorPosition::default(),
            Dealt {
                slot,
                color: edge,
                age: 0.,
                lift: 0.,
            },
            Node {
                width: px(WIDTH),
                height: px(HEIGHT),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(12)),
                ..default()
            },
            BackgroundGradient::from(LinearGradient::to_bottom(vec![
                ColorStop::auto(mix(navy(), tint, 0.22)),
                ColorStop::percent(navy(), 46),
                ColorStop::auto(Color::srgb_u8(8, 18, 26)),
            ])),
            BorderColor::all(edge),
            BoxShadow::new(edge.with_alpha(0.25), px(0), px(6), px(0), px(16)),
            UiTransform::from_translation(Val2::px(0., 90.)),
            Visibility::Hidden,
            ChildOf(row),
        ))
        .id();

    // Header: the key that picks this card, and its category.
    let header = cmd
        .spawn((
            Node {
                width: percent(100),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::new(px(10), px(12), px(10), px(6)),
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    let key = cmd
        .spawn((
            Node {
                width: px(26),
                height: px(26),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BackgroundColor(Color::srgb_u8(8, 18, 26)),
            BorderColor::all(tint),
            ChildOf(header),
        ))
        .id();
    text(cmd, key, (slot + 1).to_string(), 14., ink());
    text(cmd, header, face.kind.label(), 12., tint);

    // The art window: the stencil on a glow of the category colour.
    let window = cmd
        .spawn((
            Node {
                width: px(WIDTH - 28.),
                height: px(128),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(8)),
                ..default()
            },
            BackgroundGradient::from(RadialGradient::new(
                UiPosition::CENTER,
                RadialGradientShape::FarthestCorner,
                vec![
                    ColorStop::auto(tint.with_alpha(0.55)),
                    ColorStop::percent(tint.with_alpha(0.12), 60),
                    ColorStop::auto(Color::srgba_u8(4, 12, 18, 0)),
                ],
            )),
            BackgroundColor(Color::srgb_u8(10, 22, 30)),
            BorderColor::all(tint.with_alpha(0.45)),
            ChildOf(root),
        ))
        .id();
    cmd.spawn((
        ImageNode {
            image: art.fitting(card),
            color: ink(),
            ..default()
        },
        Node {
            width: px(84),
            height: px(84),
            ..default()
        },
        ChildOf(window),
    ));
    if face.unlocks(card) {
        let ribbon = cmd
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: px(8),
                    right: px(8),
                    padding: UiRect::axes(px(6), px(2)),
                    border_radius: BorderRadius::all(px(3)),
                    ..default()
                },
                BackgroundColor(Change::New.color()),
                ChildOf(window),
            ))
            .id();
        text(cmd, ribbon, "NEW", 11., Color::srgb_u8(30, 24, 8));
    }
    if let Some(rarity) = rarity {
        rarity_gems(cmd, window, rarity);
    }

    // Name and rank.
    let name = text(cmd, root, face.name, 21., ink());
    cmd.entity(name).insert(Node {
        margin: UiRect::top(px(10)),
        ..default()
    });
    let ranks = cmd
        .spawn((
            Node {
                column_gap: px(6),
                align_items: AlignItems::Center,
                margin: UiRect::vertical(px(6)),
                height: px(16),
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    match face.rank {
        // More ranks than diamonds fit across the card are counted in words.
        (owned, Some(most)) if most > MOST_DIAMONDS => {
            let words = format!("RANK {} OF {most}", (owned + 1).min(most));
            text(cmd, ranks, words, 12., tint);
        }
        (owned, Some(most)) => {
            for i in 0..most {
                let (fill, glow) = if i < owned {
                    (tint, None)
                } else if i == owned {
                    (ink(), Some(tint))
                } else {
                    (Color::srgb_u8(40, 60, 66), None)
                };
                let mut pip = cmd.spawn((
                    Node {
                        width: px(9),
                        height: px(9),
                        ..default()
                    },
                    BackgroundColor(fill),
                    UiTransform::from_rotation(Rot2::degrees(45.)),
                    ChildOf(ranks),
                ));
                if let Some(glow) = glow {
                    pip.insert(BoxShadow::new(glow, px(0), px(0), px(1), px(6)));
                }
            }
        }
        (owned, None) => {
            text(cmd, ranks, format!("RANK {}", owned + 1), 12., tint);
        }
    }
    cmd.spawn((
        Node {
            width: px(WIDTH - 40.),
            height: px(1),
            margin: UiRect::bottom(px(8)),
            ..default()
        },
        BackgroundColor(tint.with_alpha(0.35)),
        ChildOf(root),
    ));

    // What changes, before and after, centred in the room left above the flavour line.
    let stats = cmd
        .spawn((
            Node {
                width: percent(100),
                flex_grow: 1.,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                // Four figures close up so the art window keeps its height.
                row_gap: px(if face.stats.len() >= 4 { 2 } else { 7 }),
                padding: UiRect::horizontal(px(16)),
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    // A card that changes one thing shows it large.
    let hero = face.stats.len() == 1;
    for s in &face.stats {
        let line = cmd
            .spawn((
                Node {
                    width: percent(100),
                    flex_direction: if hero {
                        FlexDirection::Column
                    } else {
                        FlexDirection::Row
                    },
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: if hero {
                        AlignItems::Center
                    } else {
                        AlignItems::Baseline
                    },
                    row_gap: px(4),
                    ..default()
                },
                ChildOf(stats),
            ))
            .id();
        let label = text(cmd, line, s.label.to_uppercase(), 12., pale());
        cmd.entity(label).insert(TextLayout::no_wrap());
        let (small, large) = if hero { (17., 26.) } else { (13., 15.) };
        let value = text(cmd, line, "", large, s.change.color());
        cmd.entity(value).insert(TextLayout::no_wrap());
        if !s.before.is_empty() {
            cmd.spawn((
                TextSpan::new(format!("{}  ", s.before)),
                TextFont::from_font_size(small),
                TextColor(pale()),
                ChildOf(value),
            ));
            cmd.spawn((
                TextSpan::new("> "),
                TextFont::from_font_size(small),
                TextColor(tint),
                ChildOf(value),
            ));
        }
        cmd.spawn((
            TextSpan::new(s.after.clone()),
            TextFont::from_font_size(large),
            TextColor(s.change.color()),
            ChildOf(value),
        ));
    }
    let flavour = text(cmd, root, face.flavour, 12., pale().with_alpha(0.8));
    cmd.entity(flavour).insert((
        TextLayout::justify(Justify::Center),
        Node {
            max_width: px(WIDTH - 36.),
            margin: UiRect::bottom(px(14)),
            ..default()
        },
    ));
}

/// How rare the card is, as gems in the art window's top left corner with the rarity
/// named beside them.
fn rarity_gems(cmd: &mut Commands, window: Entity, rarity: Rarity) {
    let row = cmd
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(9),
                left: px(9),
                column_gap: px(4),
                align_items: AlignItems::Center,
                ..default()
            },
            ChildOf(window),
        ))
        .id();
    let color = Gem::color(rarity);
    for _ in 0..Gem::count(rarity) {
        let mut gem = cmd.spawn((
            Gem,
            Node {
                width: px(10),
                height: px(10),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BackgroundColor(color),
            BorderColor::all(mix(color, navy(), 0.55)),
            ChildOf(row),
        ));
        if rarity >= Rarity::Rare {
            gem.insert(BoxShadow::new(color, px(0), px(0), px(1), px(5)));
        }
    }
    let label = text(cmd, row, rarity.name().to_uppercase(), 10., color);
    cmd.entity(label).insert(Node {
        margin: UiRect::left(px(3)),
        ..default()
    });
}

/// Deal the hand in one card after another, then lift and brighten whichever is pointed at.
pub fn animate(
    time: Res<Time>,
    mut cards: Query<(
        &mut Dealt,
        &RelativeCursorPosition,
        &mut UiTransform,
        &mut BoxShadow,
        &mut BorderColor,
        &mut Visibility,
    )>,
) {
    let dt = time.delta_secs().min(0.05);
    for (mut card, cursor, mut pose, mut shadow, mut border, mut shown) in &mut cards {
        card.age += dt;
        let t = card.progress();
        if t > 0. {
            shown.set_if_neq(Visibility::Inherited);
        }
        let landed = 1. - (1. - t).powi(3);
        let wanted = if cursor.cursor_over() && t >= 1. {
            1.
        } else {
            0.
        };
        card.lift += (wanted - card.lift) * (dt * 14.).min(1.);
        let lift = card.lift;
        pose.translation = Val2::px(0., (1. - landed) * 90. - 12. * lift);
        pose.scale = Vec2::splat((0.9 + 0.1 * landed) * (1. + 0.04 * lift));
        pose.rotation = Rot2::degrees((1. - landed) * (card.slot as f32 - 1.) * 6.);
        if let Some(glow) = shadow.0.first_mut() {
            glow.color = card.color.with_alpha(0.22 + 0.5 * lift);
            glow.blur_radius = px(16. + 18. * lift);
            glow.y_offset = px(6. + 8. * lift);
        }
        *border = BorderColor::all(mix(card.color, Color::WHITE, 0.4 * lift));
    }
}

/// Move the pointer to just under a freshly dealt hand, clear of every card, so the captain
/// has to move up onto the one they want. It waits until the row has held still for a
/// frame, the pointer is free and the window has focus, so it never takes a pointer that
/// is working somewhere else.
pub fn park_pointer(
    mut window: Single<(&mut Window, &CursorOptions)>,
    mut hands: Query<(&mut Hand, &ComputedNode, &UiGlobalTransform)>,
) {
    let (window, cursor) = &mut *window;
    if !window.focused || cursor.grab_mode != CursorGrabMode::None {
        return;
    }
    for (mut hand, row, place) in &mut hands {
        if hand.parked || row.size().y <= 0. {
            continue;
        }
        // Layout works in physical pixels. The cards end at the row's bottom padding.
        let below = 0.5 * row.size().y - row.padding.max_inset.y;
        let spot = place.translation + Vec2::Y * (below + PARK_GAP / row.inverse_scale_factor());
        if hand.settling != Some(spot) {
            hand.settling = Some(spot);
            continue;
        }
        hand.parked = true;
        let edge = window.physical_size().as_vec2() - 1.;
        window.set_physical_cursor_position(Some(spot.clamp(Vec2::ZERO, edge).as_dvec2()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shipvivor_shipyard::Gunhouse;

    #[test]
    fn a_loader_card_shows_its_share_and_the_time_for_each_bore_aboard() {
        let mut design = Design::breakwater();
        let at = design.gun_stations()[0].berth.module;
        let house = design.modules()[at].gunhouse;
        design.rearm(at, Gunhouse { bore: 6, ..house }).unwrap();
        let now = Fittings {
            loader: 1,
            ..Fittings::default()
        };
        let figures = |card| {
            face(card, now, &design)
                .stats
                .iter()
                .map(|s| (s.label.to_string(), s.before.clone(), s.after.clone()))
                .collect::<Vec<_>>()
        };
        let next = now.with(Card::Loader);
        let time = |f: Fittings, inches| seconds(f.reload_for(inches));
        let row = |inches| {
            (
                format!("{inches} in"),
                time(now, inches),
                time(next, inches),
            )
        };
        assert_eq!(
            figures(Card::Loader),
            [
                ("Fire rate".into(), String::new(), "+32%".into()),
                row(4),
                row(6),
            ]
        );
        // Each bore loads at its own pace, and every one gets the same share off.
        assert!(now.reload_for(6) > now.reload_for(4));
        for inches in [4, 6] {
            let share = next.reload_for(inches) / now.reload_for(inches);
            let expected = shipvivor_campaign::drilled(2) / shipvivor_campaign::drilled(1);
            assert!((share - expected).abs() < 1e-4);
        }
        // The close-range loaders read the same way, for their one gun.
        let wing = figures(Card::QuickSecondaries);
        assert_eq!(wing[0].2, "+32%");
        assert_eq!(wing[1].0, "Each gun");
    }

    #[test]
    fn torpedo_cards_show_what_each_rank_brings() {
        let design = Design::heavy_destroyer();
        let figures = |card, now: Fittings| {
            face(card, now, &design)
                .stats
                .iter()
                .map(|s| (s.label.to_string(), s.before.clone(), s.after.clone()))
                .collect::<Vec<_>>()
        };
        let row = |label: &str, before: &str, after: &str| {
            (label.to_string(), before.to_string(), after.to_string())
        };
        let mut now = Fittings::default();
        let guided = |now| figures(Card::GuidedTorpedoes, now);
        assert_eq!(
            guided(now),
            [
                row("Turn", "Straight", "6 deg/s"),
                row("Seeker", "None", "120 m, 50 deg"),
            ]
        );
        now.guidance = 1;
        assert_eq!(guided(now)[0], row("Turn", "6 deg/s", "14 deg/s"));
        now.guidance = 2;
        assert_eq!(guided(now)[2], row("Fans out across", "1 ship", "3 ships"));
        now.guidance = 3;
        assert_eq!(
            guided(now)[1..],
            [
                row("Seeker", "120 m, 50 deg", "170 m, 80 deg"),
                row("Fans out across", "3 ships", "5 ships"),
                row("Spread", "3", "5"),
            ]
        );
        let now = Fittings::default();
        assert_eq!(
            figures(Card::QuickTubes, now),
            [
                row("Fire rate", "", "+32%"),
                row("Each set", "14.00 s", "10.61 s")
            ]
        );
        assert_eq!(
            figures(Card::LongRunningTorpedoes, now),
            [row("Run", "420 m", "504 m")]
        );
    }
}
