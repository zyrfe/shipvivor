//! A chart plate supplies the coastline. Ink the passages, fixes and soundings for this run.
use super::*;
use crate::state::{ChartRoll, Mode};

const WIDTH: f32 = 538.;
const HEIGHT: f32 = WIDTH * 2. / 3.;
const UNROLLED_HEIGHT: f32 = HEIGHT + 62.;
const SEA_INK: Color = Color::srgb_u8(22, 75, 80);
const FADED: Color = Color::srgb_u8(87, 114, 111);
const PENCIL: Color = Color::srgb_u8(147, 55, 41);
pub(super) const PAPER: Color = Color::srgb_u8(238, 237, 211);

fn absolute(x: f32, y: f32, width: f32, height: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: px(x),
        top: px(y),
        width: px(width),
        height: px(height),
        ..default()
    }
}

fn stroke(cmd: &mut Commands, sheet: Entity, a: Vec2, b: Vec2, weight: f32, color: Color) {
    let mid = a.midpoint(b);
    let length = a.distance(b);
    cmd.spawn((
        absolute(mid.x - length / 2., mid.y - weight / 2., length, weight),
        BackgroundColor(color),
        UiTransform::from_rotation(Rot2::radians((b - a).to_angle())),
        ChildOf(sheet),
    ));
}

fn course(cmd: &mut Commands, sheet: Entity, a: Vec2, b: Vec2, open: bool, color: Color) {
    if open {
        stroke(cmd, sheet, a, b, 2.4, color);
    } else {
        let count = (a.distance(b) / 12.).round() as usize;
        for n in 0..count {
            stroke(
                cmd,
                sheet,
                a.lerp(b, n as f32 / count as f32),
                a.lerp(b, (n as f32 + 0.52) / count as f32),
                1.5,
                color,
            );
        }
    }
    let axis = (b - a).normalize();
    let tip = a.lerp(b, 0.59);
    for side in [-1., 1.] {
        stroke(
            cmd,
            sheet,
            tip - axis * 7. + axis.perp() * side * 3.3,
            tip,
            1.8,
            color,
        );
    }
}

#[derive(Component)]
pub(crate) struct ChartName;

fn fix(
    cmd: &mut Commands,
    sheet: Entity,
    p: Vec2,
    stage: Stage,
    current: bool,
    face: &Handle<Font>,
) -> Entity {
    let radius = if current { 7. } else { 5. };
    cmd.spawn((
        Node {
            border_radius: BorderRadius::MAX,
            border: UiRect::all(px(2.)),
            ..absolute(p.x - radius, p.y - radius, 2. * radius, 2. * radius)
        },
        BorderColor::all(if current { PENCIL } else { SEA_INK }),
        BackgroundColor(if current { PENCIL } else { PAPER }),
        ChildOf(sheet),
    ));
    let name_y = if stage == Stage::OuterRoad { -43. } else { 9. };
    let label = text(cmd, sheet, stage.mission().name, 16., SEA_INK);
    cmd.entity(label).insert((
        ChartName,
        TextFont::from_font_size(16.).with_font(face.clone()),
        TextLayout::no_wrap(),
        Node {
            position_type: PositionType::Absolute,
            left: px(p.x),
            top: px(p.y + name_y),
            padding: UiRect::horizontal(px(4.)),
            ..default()
        },
        UiTransform::from_translation(Val2::percent(-50., 0.)),
        BackgroundColor(PAPER.with_alpha(0.92)),
    ));
    if current {
        let tag_y = if stage == Stage::OuterRoad { 9. } else { -26. };
        let tag = text(cmd, sheet, "PRESENT FIX", 10., PENCIL);
        cmd.entity(tag).insert((
            TextFont::from_font_size(10.).with_font(face.clone()),
            TextLayout::justify(Justify::Center),
            absolute(p.x - 40., p.y + tag_y, 80., 14.),
        ));
    }
    label
}

/// The number and name share one hit area, with the course drawn beneath the paper.
fn anchorage_pin(
    cmd: &mut Commands,
    sheet: Entity,
    p: Vec2,
    stage: Stage,
    number: usize,
    face: &Handle<Font>,
) {
    let pin = cmd
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(p.x),
                top: px(p.y),
                min_height: px(32.),
                padding: UiRect::axes(px(6.), px(3.)),
                column_gap: px(5.),
                align_items: AlignItems::Center,
                border: UiRect::all(px(1.)),
                ..default()
            },
            UiTransform::from_translation(Val2::percent(-50., -50.)),
            ChartName,
            Action::Depart(stage),
            RelativeCursorPosition::default(),
            BackgroundColor(PAPER.with_alpha(0.92)),
            BorderColor::all(SEA_INK),
            ChildOf(sheet),
        ))
        .id();
    let badge = cmd
        .spawn((
            Node {
                width: px(21.),
                height: px(21.),
                border_radius: BorderRadius::MAX,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(SEA_INK),
            ChildOf(pin),
        ))
        .id();
    text(cmd, badge, number.to_string(), 14., PAPER);
    let name = text(cmd, pin, stage.mission().name, 16., SEA_INK);
    cmd.entity(name).insert((
        TextFont::from_font_size(16.).with_font(face.clone()),
        TextLayout::no_wrap(),
    ));
}

/// The same anchorages and exits drive the plotted courses and departure orders.
fn plot(cmd: &mut Commands, parent: Entity, run: &Campaign, art: &crate::insignia::Insignia) {
    let sheet = cmd
        .spawn((
            ImageNode::new(art.chart_paper.clone()),
            Node {
                width: px(WIDTH),
                height: px(HEIGHT),
                flex_shrink: 0.,
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    let seed = (run.seed ^ (run.seed >> 32)) as u32;
    let jitter = |i, j| crate::surfaces::sample(i, j, seed) - 0.5;
    let departing = run.screen() == Screen::Departure;
    let origins = std::array::from_fn::<_, { ANCHORAGES.len() }, _>(|i| {
        Vec2::new(
            94. + 8. * jitter(8, i as u32),
            83. + 84. * i as f32 + 12. * jitter(7, i as u32),
        )
    });
    let junction = Vec2::new(205., 167. + 14. * jitter(8, 0));
    let crown = Vec2::new(461., 151. + 18. * jitter(9, 0));
    let ports = std::array::from_fn::<_, { EXITS.len() }, _>(|i| {
        Vec2::new(
            262. + 27. * jitter(i as u32, 0),
            68. + i as f32 * 89. + 14. * jitter(i as u32, 1),
        )
    });
    // Occasional depth soundings sit off the passage lines.
    for (i, (x, y)) in [
        (171., 36.),
        (355., 39.),
        (471., 75.),
        (73., 210.),
        (169., 294.),
        (346., 275.),
    ]
    .into_iter()
    .enumerate()
    {
        let sounding = text(
            cmd,
            sheet,
            format!("{}", (27. + jitter(i as u32, 3) * 34.) as i32),
            10.,
            FADED,
        );
        cmd.entity(sounding)
            .insert(absolute(x + jitter(i as u32, 2) * 14., y, 24., 14.));
    }
    let north = text(cmd, sheet, "N", 11., SEA_INK);
    cmd.entity(north).insert(absolute(32., 280., 15., 14.));
    for (port, p) in ANCHORAGES.iter().zip(origins) {
        if departing || port.stage == run.origin {
            course(
                cmd,
                sheet,
                p,
                junction,
                true,
                if run.passage.is_some() {
                    PENCIL
                } else {
                    SEA_INK
                },
            );
        }
    }
    for (exit, p) in EXITS.iter().zip(ports) {
        let chosen = run.passage == Some(exit.stage);
        let open = !departing && (chosen || (run.passage.is_none() && run.stars() >= exit.stars));
        let color = if chosen {
            PENCIL
        } else if open {
            SEA_INK
        } else {
            FADED
        };
        course(cmd, sheet, junction, p, open, color);
        course(cmd, sheet, p, crown, open, color);
    }
    for (exit, p) in EXITS.iter().zip(ports) {
        let label = fix(
            cmd,
            sheet,
            p,
            exit.stage,
            run.stage == exit.stage,
            &art.display_face,
        );
        if run.screen() == Screen::Chart && run.stars() >= exit.stars {
            cmd.entity(label).insert((
                Action::Depart(exit.stage),
                RelativeCursorPosition::default(),
            ));
        }
        let detail = if exit.stage.enters_air_cover() {
            format!("{} stars  /  Air station", exit.stars)
        } else if exit.stars == 0 {
            "0 stars".into()
        } else {
            format!("{} stars", exit.stars)
        };
        let note = text(
            cmd,
            sheet,
            detail,
            11.,
            if exit.stage.enters_air_cover() {
                PENCIL
            } else {
                SEA_INK
            },
        );
        cmd.entity(note).insert((
            TextFont::from_font_size(12.).with_font(art.display_face.clone()),
            absolute(
                p.x - 66.,
                p.y + if exit.stage == Stage::OuterRoad {
                    -23.
                } else {
                    29.
                },
                132.,
                16.,
            ),
            TextLayout::justify(Justify::Center),
        ));
        if exit.stage.enters_air_cover() {
            cmd.spawn((
                ImageNode {
                    image: art.floatplane.clone(),
                    color: PENCIL,
                    ..default()
                },
                absolute(p.x - 8., p.y + 46., 16., 16.),
                ChildOf(sheet),
            ));
        }
    }
    for (i, (port, p)) in ANCHORAGES.iter().zip(origins).enumerate() {
        if departing {
            anchorage_pin(cmd, sheet, p, port.stage, i + 1, &art.display_face);
        } else if port.stage == run.origin {
            fix(
                cmd,
                sheet,
                p,
                port.stage,
                run.stage == port.stage,
                &art.display_face,
            );
        }
    }
    fix(
        cmd,
        sheet,
        crown,
        Stage::CrownDeep,
        run.stage == Stage::CrownDeep,
        &art.display_face,
    );
}

fn legend(run: &Campaign) -> &'static str {
    if run.screen() == Screen::Departure {
        "Click an anchorage to sail.   Dashed lines show later passages."
    } else if run.passage.is_some() {
        "Red course   Selected passage       Dashed   Other passages"
    } else {
        "Solid   Open passage       Dashed   Star-locked passage"
    }
}

#[derive(Component)]
pub(crate) enum RollPart {
    Sheet,
    Curl,
}

impl RollPart {
    fn width(&self, spread: f32) -> Val {
        let eased = spread * spread * (3. - 2. * spread);
        px(match self {
            Self::Sheet => 18. + (WIDTH - 18.) * eased,
            Self::Curl => 18. * (1. - eased).sqrt(),
        })
    }
}

/// Fit the whole sheet, including its title and key, with sea visible around the edges.
pub(super) fn reading_scale(viewport: Vec2) -> f32 {
    (viewport.x * 0.92 / WIDTH)
        .min(viewport.y * 0.88 / UNROLLED_HEIGHT)
        .max(0.45)
}

/// Keep the ink at its printed scale. A clipping window follows the moving curl.
pub(super) fn unfurl(
    cmd: &mut Commands,
    root: Entity,
    run: &Campaign,
    art: &crate::insignia::Insignia,
    roll: ChartRoll,
) {
    let frame = cmd
        .spawn((
            Node {
                width: px(WIDTH),
                height: px(UNROLLED_HEIGHT),
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    let sheet = cmd
        .spawn((
            RollPart::Sheet,
            Node {
                width: RollPart::Sheet.width(roll.spread),
                height: percent(100),
                overflow: Overflow::clip(),
                ..default()
            },
            BoxShadow::new(Color::srgba(0., 0., 0., 0.56), px(2), px(8), px(1), px(16)),
            ChildOf(frame),
        ))
        .id();
    let paper = cmd
        .spawn((
            Node {
                width: px(WIDTH),
                height: percent(100),
                flex_shrink: 0.,
                ..column(0.)
            },
            BackgroundColor(PAPER),
            ChildOf(sheet),
        ))
        .id();
    let head = cmd
        .spawn((
            Node {
                height: px(36),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::horizontal(px(12)),
                ..default()
            },
            ChildOf(paper),
        ))
        .id();
    let title = text(cmd, head, "Passage chart", 21., SEA_INK);
    cmd.entity(title)
        .insert(TextFont::from_font_size(21.).with_font(art.display_face.clone()));
    text(cmd, head, "Tab / Esc   Stow chart", 13., SEA_INK);
    plot(cmd, paper, run, art);
    let key = text(cmd, paper, legend(run), 13., SEA_INK);
    cmd.entity(key).insert(Node {
        margin: UiRect::axes(px(12), px(4)),
        ..default()
    });
    cmd.spawn((
        RollPart::Curl,
        Node {
            position_type: PositionType::Absolute,
            right: px(0),
            top: px(0),
            height: percent(100),
            width: RollPart::Curl.width(roll.spread),
            ..default()
        },
        BackgroundGradient::from(LinearGradient::to_right(vec![
            ColorStop::percent(Color::srgb_u8(114, 124, 102), 0.),
            ColorStop::percent(PAPER, 26.),
            ColorStop::percent(Color::srgb_u8(250, 246, 220), 45.),
            ColorStop::percent(Color::srgb_u8(164, 160, 127), 84.),
            ColorStop::percent(Color::srgb_u8(81, 87, 77), 100.),
        ])),
        ChildOf(sheet),
    ));
}

pub(crate) fn roll(
    time: Res<Time>,
    mut run: ResMut<Campaign>,
    mut parts: Query<(&RollPart, &mut Node)>,
) {
    let Some(current) = run.chart_roll else {
        return;
    };
    if current.opening && current.spread == 1. {
        return;
    }
    let speed = if current.opening {
        1. / shipvivor_acoustics::CHART_UNROLL_SECONDS
    } else {
        -1. / shipvivor_acoustics::CHART_ROLL_SECONDS
    };
    let spread = (current.spread + time.delta_secs() * speed).clamp(0., 1.);
    if !current.opening && spread == 0. {
        run.chart_roll = None;
        run.mode = Mode::Patrol;
        return;
    }
    run.chart_roll = Some(ChartRoll { spread, ..current });
    for (part, mut node) in &mut parts {
        node.width = part.width(spread);
    }
}

/// Place the chart next to the existing orders, both legible at the game's minimum UI scale.
pub(super) fn table(
    cmd: &mut Commands,
    panel: Entity,
    run: &Campaign,
    art: &crate::insignia::Insignia,
) -> Entity {
    let title = if run.screen() == Screen::Departure {
        "First patrol"
    } else {
        "Passage chart"
    };
    let heading = text(cmd, panel, title, 32., ink());
    cmd.entity(heading)
        .insert(TextFont::from_font_size(32.).with_font(art.display_face.clone()));
    let row = cmd
        .spawn((
            Node {
                column_gap: px(20.),
                align_items: AlignItems::Center,
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    let plate = cmd.spawn((column(7.), ChildOf(row))).id();
    plot(cmd, plate, run, art);
    text(cmd, plate, legend(run), 13., pale());
    cmd.spawn((
        Node {
            flex_grow: 1.,
            flex_basis: px(0.),
            min_width: px(0.),
            ..column(9.)
        },
        ChildOf(row),
    ))
    .id()
}
