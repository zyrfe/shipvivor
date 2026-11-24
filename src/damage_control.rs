//! The damage-control strip in the Breakwater panel: one cell for each hull module from
//! stern to bow, what is fitted to it, where the repair party is working, and how close
//! she is to sinking.
use crate::state::Campaign;
use bevy::prelude::*;
use shipvivor_shipyard::{Berth, Component as Gear, Ship, State, Task};

/// The hull module a cell stands for. Its border marks flooding and the repair party.
#[derive(Component)]
pub struct Cell(usize);
/// The bar inside a cell: structure left in the module.
#[derive(Component)]
pub struct Fill(usize);
/// The tick inside a cell: as far as the crew can mend the module at sea.
#[derive(Component)]
pub struct Limit(usize);
/// The label of one fitted component.
#[derive(Component)]
pub struct Pip(Berth);
#[derive(Component)]
pub struct Summary;

const CELL_WIDTH: f32 = 44.;
/// The summary's text, kept apart from the component labels.
type SummaryOnly = (With<Summary>, Without<Pip>);

fn ink() -> Color {
    Color::srgb_u8(234, 239, 222)
}
fn pale() -> Color {
    Color::srgb_u8(164, 194, 192)
}
fn brass() -> Color {
    Color::srgb_u8(255, 199, 91)
}
fn trim() -> Color {
    Color::srgb_u8(74, 106, 108)
}
fn sound() -> Color {
    Color::srgb_u8(158, 178, 125)
}
fn alarm() -> Color {
    Color::srgb_u8(255, 124, 102)
}
fn flooded() -> Color {
    Color::srgb_u8(46, 78, 112)
}

fn label(gear: Gear) -> &'static str {
    match gear {
        Gear::MainGun => "GUN",
        Gear::CloseGun => "CG",
        Gear::TorpedoRack => "TT",
        Gear::Director => "DIR",
        Gear::Engine => "ENG",
        Gear::Magazine => "MAG",
    }
}

/// Add the strip to the ship's panel. Cells run stern to bow, left to right.
pub fn spawn(cmd: &mut Commands, panel: Entity, run: &Campaign) {
    let design = run.ship.design();
    let caption = cmd
        .spawn((
            Node {
                justify_content: JustifyContent::SpaceBetween,
                margin: UiRect::top(px(3)),
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    for (words, color) in [
        ("STERN", pale()),
        ("DAMAGE CONTROL", brass()),
        ("BOW", pale()),
    ] {
        cmd.spawn((
            Text::new(words),
            TextFont::from_font_size(11.),
            TextColor(color),
            ChildOf(caption),
        ));
    }
    let strip = cmd
        .spawn((
            Node {
                column_gap: px(3),
                align_items: AlignItems::FlexStart,
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    for module in (0..design.modules().len()).rev() {
        let column = cmd
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    width: px((CELL_WIDTH + 3.) * 6. / design.modules().len() as f32 - 3.),
                    row_gap: px(1),
                    ..default()
                },
                ChildOf(strip),
            ))
            .id();
        let cell = cmd
            .spawn((
                Cell(module),
                Node {
                    width: percent(100),
                    height: px(14),
                    border: UiRect::all(px(1)),
                    margin: UiRect::bottom(px(2)),
                    ..default()
                },
                BorderColor::all(trim()),
                BackgroundColor(Color::srgb_u8(6, 18, 24)),
                ChildOf(column),
            ))
            .id();
        cmd.spawn((
            Fill(module),
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
            BackgroundColor(sound()),
            ChildOf(cell),
        ));
        cmd.spawn((
            Limit(module),
            Node {
                position_type: PositionType::Absolute,
                display: Display::None,
                top: px(0),
                width: px(1),
                height: percent(100),
                ..default()
            },
            BackgroundColor(ink()),
            ChildOf(cell),
        ));
        if run
            .dock_modules()
            .any(|(i, kind)| i == module && kind == shipvivor_campaign::DockModule::Reserve)
        {
            cmd.spawn((
                Text::new("RES"),
                TextFont::from_font_size(11.),
                TextColor(pale()),
                ChildOf(column),
            ));
        }
        for (index, &gear) in design.modules()[module].fit.iter().enumerate() {
            cmd.spawn((
                Pip(Berth { module, index }),
                Text::new(label(gear)),
                TextFont::from_font_size(11.),
                TextColor(ink()),
                ChildOf(column),
            ));
        }
    }
    cmd.spawn((
        Summary,
        Text::new(""),
        TextFont::from_font_size(12.),
        TextColor(pale()),
        ChildOf(panel),
    ));
}

/// The two lines under the strip: how near she is to sinking, what the engines give,
/// what the repair party is doing, and the state of her power.
fn report(ship: &Ship) -> String {
    let (lost, fatal) = ship.flooding();
    let party = match ship.party() {
        None => "Repair party standing by".to_string(),
        Some(Task::Module(_)) => "Repair party: hull".to_string(),
        Some(Task::Fitting(berth)) => format!(
            "Repair party: {}",
            ship.design().component(berth).map_or("", label)
        ),
    };
    if ship.on_reserve_power() {
        return format!(
            "RESERVE POWER   Speed {:.0}%   Gun train {:.0}%\nFlooded {lost}/{fatal}   {party}",
            Ship::RESERVE_SPEED * 100.,
            Ship::RESERVE_GUN_RATE * 100.
        );
    }
    format!(
        "Flooded {lost}, sinks at {fatal}    Engines {:.0}%\n{party}",
        ship.propulsion() * 100.
    )
}

pub fn update(
    run: Res<Campaign>,
    mut cells: Query<(&Cell, &mut BorderColor)>,
    mut fills: Query<(&Fill, &mut Node, &mut BackgroundColor), Without<Limit>>,
    mut limits: Query<(&Limit, &mut Node), Without<Fill>>,
    mut pips: Query<(&Pip, &mut Text, &mut TextColor)>,
    mut summary: Query<(&mut Text, &mut TextColor), SummaryOnly>,
) {
    let ship = &run.ship;
    let holds = ship.compartments();
    let party = ship.party();
    for (cell, mut border) in &mut cells {
        // The water inside already marks a flooded module, so the party's mark wins.
        let color = if party == Some(Task::Module(cell.0)) {
            brass()
        } else if holds[cell.0].wrecked() {
            alarm()
        } else {
            trim()
        };
        border.set_if_neq(BorderColor::all(color));
    }
    for (fill, mut node, mut paint) in &mut fills {
        let hold = &holds[fill.0];
        // A flooded module shows full of water, not empty.
        let (share, color) = if hold.wrecked() {
            (1., flooded())
        } else {
            let share = hold.soundness();
            let color = if share >= 0.7 {
                sound()
            } else if share >= 0.35 {
                brass()
            } else {
                alarm()
            };
            (share, color)
        };
        let width = percent((share * 100.).round());
        if node.width != width {
            node.width = width;
        }
        paint.set_if_neq(BackgroundColor(color));
    }
    for (limit, mut node) in &mut limits {
        let hold = &holds[limit.0];
        let share = hold.ceiling() / hold.capacity();
        // Shown only once damage has left something the crew cannot make good.
        let display = if share < 0.99 && !hold.wrecked() {
            Display::Flex
        } else {
            Display::None
        };
        let left = percent((share * 100.).round().min(97.));
        if node.display != display {
            node.display = display;
        }
        if node.left != left {
            node.left = left;
        }
    }
    for (pip, mut text, mut paint) in &mut pips {
        let Some(gear) = ship.design().component(pip.0) else {
            continue;
        };
        let color = match ship.state(pip.0) {
            State::Out => alarm(),
            // Sound, but dead until an engine is turning again.
            _ if !ship.powered() => trim(),
            State::Damaged => brass(),
            State::Working => ink(),
        };
        let words = if party == Some(Task::Fitting(pip.0)) {
            format!("[{}]", label(gear))
        } else {
            label(gear).to_string()
        };
        if text.0 != words {
            text.0 = words;
        }
        paint.set_if_neq(TextColor(color));
    }
    for (mut text, mut paint) in &mut summary {
        let words = report(ship);
        if text.0 != words {
            text.0 = words;
        }
        let color = if ship.in_peril() || !ship.powered() {
            alarm()
        } else if ship.on_reserve_power() {
            brass()
        } else {
            pale()
        };
        paint.set_if_neq(TextColor(color));
    }
}
