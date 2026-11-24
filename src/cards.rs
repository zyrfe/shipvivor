//! Fitting cards for the refit: each card is a face of data (category, rank, before and
//! after stats) drawn as a framed card that deals in, lifts and glows under the cursor.
use crate::{insignia::Insignia, ui::Action};
use bevy::{prelude::*, ui::RelativeCursorPosition};
use shipvivor_campaign::{CALIBRES, Card, Fittings};
use shipvivor_encounter::{Ammo, MainShell};
use shipvivor_navigation::Helm;

const KNOTS_PER_MPS: f32 = 1.94384;
const WIDTH: f32 = 268.;
const HEIGHT: f32 = 380.;
/// Seconds between one card landing and the next.
const STAGGER: f32 = 0.09;
const DEAL: f32 = 0.38;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Ammunition,
    Gunnery,
    Machinery,
    Secondaries,
}
impl Kind {
    fn label(self) -> &'static str {
        match self {
            Self::Ammunition => "AMMUNITION",
            Self::Gunnery => "MAIN BATTERY",
            Self::Machinery => "MACHINERY",
            Self::Secondaries => "SECONDARIES",
        }
    }
    fn color(self) -> Color {
        match self {
            Self::Ammunition => Color::srgb_u8(255, 176, 74),
            Self::Gunnery => Color::srgb_u8(240, 104, 88),
            Self::Machinery => Color::srgb_u8(88, 204, 214),
            Self::Secondaries => Color::srgb_u8(132, 224, 120),
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
    pub(crate) label: &'static str,
    /// Empty when the card adds something the ship did not have.
    pub(crate) before: String,
    pub(crate) after: String,
    pub(crate) change: Change,
}
fn stat(label: &'static str, before: impl ToString, after: impl ToString, change: Change) -> Stat {
    Stat {
        label,
        before: before.to_string(),
        after: after.to_string(),
        change,
    }
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
                Card::ArmorPiercing
                    | Card::TwinSecondaries
                    | Card::TripleTurrets
                    | Card::QuadTurrets
            )
    }
}

fn seconds(s: f32) -> String {
    format!("{s:.2} s")
}
fn knots(f: Fittings) -> String {
    format!(
        "{:.1} kn",
        Helm::FULL_AHEAD * f.speed_factor() * KNOTS_PER_MPS
    )
}
fn ap(f: Fittings) -> MainShell {
    MainShell {
        ammo: Ammo::Ap,
        ..f.round()
    }
}

pub(crate) fn face(card: Card, now: Fittings) -> Face {
    use Change::*;
    let next = now.with(card);
    let (wing, wing_next) = (now.secondaries(), next.secondaries());
    match card {
        Card::Loader => Face {
            kind: Kind::Gunnery,
            name: "Quick Loaders",
            rank: (now.loader, None),
            stats: vec![stat(
                "Reload",
                seconds(now.reload()),
                seconds(next.reload()),
                Gain,
            )],
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
            ],
            flavour: "A larger shell takes longer to load. Each mount uses the largest bore its deck can carry.",
        },
        Card::Caps => {
            let (before, after) = (ap(now).penetration(), ap(next).penetration());
            let (a, b) = if before != after {
                (before, after)
            } else {
                (ap(now).at_contact(1.), ap(next).at_contact(1.))
            };
            Face {
                kind: Kind::Ammunition,
                name: "Piercing Caps",
                rank: (now.caps, None),
                stats: vec![stat(
                    if before != after {
                        "AP opens"
                    } else {
                        "AP at range"
                    },
                    a.plate_label(),
                    b.plate_label(),
                    Gain,
                )],
                flavour: "Hardened caps carry the AP round through heavier plate.",
            }
        }
        Card::Velocity => Face {
            kind: Kind::Ammunition,
            name: "Shell Velocity",
            rank: (now.velocity, None),
            stats: vec![stat(
                "Speed",
                format!("{:.0} m/s", now.round().speed()),
                format!("{:.0} m/s", next.round().speed()),
                Gain,
            )],
            flavour: "A flatter arc and a shorter lead.",
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
            rank: (now.turrets.count() as u16 - 2, Some(2)),
            stats: vec![stat(
                "Barrels",
                now.turrets.count(),
                next.turrets.count(),
                Gain,
            )],
            flavour: "Wider gunhouses, more barrels in every salvo.",
        },
        Card::ArmorPiercing => Face {
            kind: Kind::Ammunition,
            name: "Armor-Piercing",
            rank: (now.armor_piercing, Some(1)),
            stats: vec![
                stat(
                    "AP opens",
                    "",
                    format!("{} plate", ap(next).penetration().plate_label()),
                    New,
                ),
                stat("Light hulls", "", "Half damage", Cost),
                stat("Switch", "", "Q key", New),
            ],
            flavour: "Load AP for armour, HE for everything else.",
        },
        Card::MoreSecondaries => Face {
            kind: Kind::Secondaries,
            name: "More Secondaries",
            rank: (now.wing_mounts, Some(2)),
            stats: vec![stat("Guns a side", wing.per_side, wing_next.per_side, Gain)],
            flavour: "Another shielded gun each side, forward and then aft.",
        },
        Card::HeavySecondaries => Face {
            kind: Kind::Secondaries,
            name: "Heavier Secondaries",
            rank: (now.wing_shells, Some(2)),
            stats: vec![stat("Damage", wing.damage, wing_next.damage, Gain)],
            flavour: "A heavier round for the close-range guns.",
        },
        Card::QuickSecondaries => Face {
            kind: Kind::Secondaries,
            name: "Secondary Loaders",
            rank: (now.wing_loaders, Some(3)),
            stats: vec![stat(
                "Reload",
                seconds(wing.reload),
                seconds(wing_next.reload),
                Gain,
            )],
            flavour: "Ready-use lockers beside every gun.",
        },
        Card::SecondaryRange => Face {
            kind: Kind::Secondaries,
            name: "Rangefinders",
            rank: (now.wing_range, Some(2)),
            stats: vec![stat(
                "Reach",
                format!("{:.0} m", wing.range),
                format!("{:.0} m", wing_next.range),
                Gain,
            )],
            flavour: "The crew opens fire on boats farther out.",
        },
        Card::TwinSecondaries => Face {
            kind: Kind::Secondaries,
            name: "Twin Secondaries",
            rank: (now.wing_twin, Some(1)),
            stats: vec![stat("Barrels", wing.barrels, wing_next.barrels, Gain)],
            flavour: "Every wing gun becomes a twin mount. Both barrels fire.",
        },
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

/// Lay the offered cards in a row under `parent`.
pub fn hand(cmd: &mut Commands, parent: Entity, now: Fittings, art: &Insignia) {
    let row = cmd
        .spawn((
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
    for (slot, card) in now.offers().into_iter().enumerate() {
        spawn_card(cmd, row, slot, card, now, art);
    }
}

fn spawn_card(
    cmd: &mut Commands,
    row: Entity,
    slot: usize,
    card: Card,
    now: Fittings,
    art: &Insignia,
) {
    let face = face(card, now);
    let tint = face.kind.color();
    let root = cmd
        .spawn((
            Action::Card(card),
            RelativeCursorPosition::default(),
            Dealt {
                slot,
                color: tint,
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
            BorderColor::all(tint),
            BoxShadow::new(tint.with_alpha(0.25), px(0), px(6), px(0), px(16)),
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
                row_gap: px(7),
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
        let t = ((card.age - card.slot as f32 * STAGGER) / DEAL).clamp(0., 1.);
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
