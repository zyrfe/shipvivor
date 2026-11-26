//! The boss's place on the screen: a herald when smoke goes up over her yard, another
//! when she puts to sea, and a tag that rides over her, or on the edge of the screen
//! towards her, for as long as she floats.
use crate::{
    camera::ShipView,
    mission_hud::{letter, ship_class},
    state::{Encounter, PlayerHelm},
};
use bevy::{camera::primitives::MeshAabb, prelude::*};
use shipvivor_encounter::{EnemyClass, Raider, STEAM_WARNING, Sortie};

/// Seconds the herald stays up after the flagship puts to sea.
pub const HERALD_SECONDS: f32 = 7.;
/// Seconds the herald takes to settle from its first, larger size.
const POP: f32 = 0.35;
/// Seconds the herald's border flashes, and how many times a second.
const FLASH: (f32, f32) = (2.4, 4.);
/// Size of the tag's label, and of the diamond pointing down at her.
const TAG: Vec2 = Vec2::new(196., 36.);
/// The herald hangs below the mission header and its notice line, above the horizon
/// where distant ships are found.
const HERALD_TOP: f32 = 150.;
const POINTER: f32 = 9.;
/// Clearance kept round the gun sight in the middle of the screen.
const SIGHT: Vec2 = Vec2::new(22., 35.);

fn chalk() -> Color {
    Color::srgb_u8(239, 240, 220)
}
fn brass() -> Color {
    Color::srgb_u8(255, 201, 101)
}
fn coral() -> Color {
    Color::srgb_u8(255, 148, 123)
}
fn plate() -> Color {
    Color::srgba_u8(12, 31, 40, 239)
}

#[derive(Component)]
pub struct Herald;
#[derive(Component)]
pub struct HeraldPanel;
#[derive(Component)]
pub struct HeraldLine(usize);
#[derive(Component)]
pub struct Tag;
#[derive(Component)]
pub struct TagLabel;
#[derive(Component)]
pub struct TagPointer;

fn label(cmd: &mut Commands, parent: Entity, size: f32, color: Color) -> Entity {
    cmd.spawn((
        Text::new(""),
        TextFont::from_font_size(size),
        TextColor(color),
        TextLayout::justify(Justify::Center),
        ChildOf(parent),
    ))
    .id()
}

pub fn spawn(cmd: &mut Commands, root: Entity) {
    let herald = cmd
        .spawn((
            Herald,
            Node {
                position_type: PositionType::Absolute,
                top: px(HERALD_TOP),
                left: px(0),
                right: px(0),
                justify_content: JustifyContent::Center,
                display: Display::None,
                ..default()
            },
            GlobalZIndex(5),
            ChildOf(root),
        ))
        .id();
    let panel = cmd
        .spawn((
            HeraldPanel,
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::axes(px(28), px(6)),
                row_gap: px(2),
                border: UiRect::vertical(px(3)),
                ..default()
            },
            BackgroundColor(plate()),
            BorderColor::all(brass()),
            UiTransform::default(),
            ChildOf(herald),
        ))
        .id();
    for (i, (size, color)) in [(12., coral()), (26., brass()), (14., chalk())]
        .into_iter()
        .enumerate()
    {
        let line = label(cmd, panel, size, color);
        cmd.entity(line).insert(HeraldLine(i));
    }

    let tag = cmd
        .spawn((
            Tag,
            Node {
                position_type: PositionType::Absolute,
                width: px(TAG.x),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                display: Display::None,
                ..default()
            },
            UiTransform::default(),
            GlobalZIndex(3),
            ChildOf(root),
        ))
        .id();
    let plate = cmd
        .spawn((
            Node {
                width: px(TAG.x),
                height: px(TAG.y),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(px(1)).with_bottom(px(2)),
                ..default()
            },
            BackgroundColor(plate()),
            BorderColor::all(brass()),
            ChildOf(tag),
        ))
        .id();
    let text = label(cmd, plate, 12., brass());
    cmd.entity(text).insert(TagLabel);
    cmd.spawn((
        TagPointer,
        Node {
            width: px(POINTER),
            height: px(POINTER),
            margin: UiRect::top(px(4)),
            ..default()
        },
        BackgroundColor(brass()),
        UiTransform {
            rotation: Rot2::degrees(45.),
            ..default()
        },
        ChildOf(tag),
    ));
}

/// The boss afloat in the sector, if there is one.
fn flagship(battle: &Encounter) -> Option<&Raider> {
    battle.boss_ship()
}

/// What the herald announces.
#[derive(Clone, Copy)]
enum News {
    /// Smoke over the yards: she will sail in this many seconds.
    Steam(f32),
    /// She has put to sea from this port.
    Sailed(usize),
}

/// What the herald says and how long it has been up, while it is.
fn herald_age(battle: &Encounter) -> Option<(News, f32)> {
    match battle.flagship() {
        Some(Sortie::Sailed { port, at }) => {
            let age = battle.at_sea() - at;
            (age < HERALD_SECONDS).then_some((News::Sailed(port), age))
        }
        Some(Sortie::Due(_)) => {
            let due = battle.raising_steam()?;
            let age = STEAM_WARNING - due;
            (age < HERALD_SECONDS).then_some((News::Steam(due), age))
        }
        None => None,
    }
}

/// Height of the top of her mast above the water, where the tag points.
fn masthead(class: EnemyClass, known: &mut Vec<(EnemyClass, f32)>) -> f32 {
    if let Some(&(_, height)) = known.iter().find(|(c, _)| *c == class) {
        return height;
    }
    let height = crate::fleet_models::superstructure(class)
        .get_aabb()
        .map_or(class.profile().deck + 12., |aabb| aabb.max().y);
    known.push((class, height));
    height
}

fn cardinal(d: Vec2) -> &'static str {
    let bearing = shipvivor_compass::heading(d);
    ["N", "NE", "E", "SE", "S", "SW", "W", "NW"][(bearing.rem_euclid(std::f32::consts::TAU)
        / std::f32::consts::FRAC_PI_4)
        .round() as usize
        % 8]
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn update(
    battle: Res<Encounter>,
    helm: Res<PlayerHelm>,
    view: Res<ShipView>,
    viewport: (Single<&Window>, Res<UiScale>),
    mut masts: Local<Vec<(EnemyClass, f32)>>,
    mut herald: Query<&mut Node, (With<Herald>, Without<Tag>, Without<TagPointer>)>,
    mut panel: Query<(&mut BorderColor, &mut UiTransform), (With<HeraldPanel>, Without<Tag>)>,
    mut lines: Query<(&HeraldLine, &mut Text), Without<TagLabel>>,
    mut tag: Query<(&mut Node, &mut UiTransform), (With<Tag>, Without<HeraldPanel>)>,
    mut tag_label: Query<&mut Text, (With<TagLabel>, Without<HeraldLine>)>,
    mut pointer: Query<&mut Node, (With<TagPointer>, Without<Tag>, Without<Herald>)>,
) {
    let ship = flagship(&battle);
    let heralding = herald_age(&battle);

    // The herald: who has put to sea, from where, and how far off she lies.
    for mut node in &mut herald {
        node.display = if heralding.is_some() {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Some((news, age)) = heralding {
        for (mut border, mut transform) in &mut panel {
            let lit = age < FLASH.0 && ((age * FLASH.1 * 2.).floor() as u32).is_multiple_of(2);
            border.set_if_neq(BorderColor::all(if lit { coral() } else { brass() }));
            transform.scale = Vec2::splat(1. + 0.35 * (1. - age / POP).clamp(0., 1.));
        }
        let class = ship
            .map(|ship| ship.class)
            .or(battle.boss())
            .unwrap_or(EnemyClass::Parliament);
        let name = ship_class(class).to_uppercase();
        let whereabouts = ship.map_or(String::new(), |ship| {
            let d = ship.body.pos - helm.position;
            let range = (d.length() / 10.).round() as u32 * 10;
            format!("{} {range} m", cardinal(d))
        });
        for (line, mut text) in &mut lines {
            let words = match (line.0, news) {
                (0, News::Steam(_)) => "SMOKE OVER THE ENEMY YARDS".to_string(),
                (0, News::Sailed(_)) => "ENEMY FLAGSHIP UNDER WAY".to_string(),
                (1, _) => name.clone(),
                (_, News::Steam(due)) => {
                    let due = due.ceil() as u32;
                    format!(
                        "raising steam    puts to sea in {}:{:02}",
                        due / 60,
                        due % 60
                    )
                }
                (_, News::Sailed(port)) => {
                    format!("puts to sea from Base {}    {whereabouts}", letter(port))
                }
            };
            if text.0 != words {
                text.0 = words;
            }
        }
    }

    // The tag rides over her mast, or waits on the edge of the screen towards her.
    let Ok((mut node, mut transform)) = tag.single_mut() else {
        return;
    };
    let Some(ship) = ship else {
        node.display = Display::None;
        return;
    };
    let (window, scale) = viewport;
    let size = window.size() / scale.0;
    let top = ship.body.pos.extend(masthead(ship.class, &mut masts)).xzy();
    let (mark, edge) = view.project_mark(helm.position, top, size);
    let height = TAG.y + 4. + POINTER;
    let mut corner = if edge.is_empty() {
        // Over the mast, with the diamond's tip just above it.
        let mut corner = Vec2::new(mark.x - TAG.x * 0.5, mark.y - height - 4.);
        let middle = size * 0.5;
        if (mark.x - middle.x).abs() < TAG.x * 0.5 + SIGHT.x
            && corner.y < middle.y + SIGHT.y
            && corner.y + height > middle.y - SIGHT.y
        {
            corner.y = middle.y - SIGHT.y - height;
        }
        corner
    } else {
        mark - Vec2::new(TAG.x, TAG.y) * 0.5
    };
    corner = corner.clamp(
        Vec2::splat(4.),
        (size - Vec2::new(TAG.x, height) - 4.).max(Vec2::ZERO),
    );
    node.left = px(corner.x);
    node.top = px(corner.y);
    node.display = Display::Flex;
    transform.scale = Vec2::splat(match heralding {
        // She is fresh to sea: the tag beats to draw the eye.
        Some((_, age)) => 1. + 0.18 * (age * 5.).sin().abs() * (1. - age / HERALD_SECONDS),
        None => 1.,
    });
    for mut node in &mut pointer {
        node.display = if edge.is_empty() {
            Display::Flex
        } else {
            Display::None
        };
    }
    let d = ship.body.pos - helm.position;
    let range = (d.length() / 10.).round() as u32 * 10;
    let state = if ship.protected() && ship.helm_lost() > 0. {
        // Her bridge is gone: how long she holds her course before someone cons her.
        format!("no helm {:.0} s", ship.helm_lost().ceil())
    } else if ship.protected() {
        format!("{}/{} turrets", ship.live_guns(), ship.guns().len())
    } else if ship.class.is_flagship() {
        "hull exposed".to_string()
    } else {
        // A boss with no turret screen is sunk as any ship is, by flooding her.
        let (flooded, fatal) = ship.vessel().flooding();
        format!("{flooded}/{fatal} flooded")
    };
    let words = format!(
        "{}{edge}\n{range} m   {state}",
        ship_class(ship.class).to_uppercase()
    );
    for mut text in &mut tag_label {
        if text.0 != words {
            text.0 = words.clone();
        }
    }
}
