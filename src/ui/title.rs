use super::*;

pub(super) fn show(cmd: &mut Commands, root: Entity, face: &Handle<Font>) {
    backdrop(cmd, root);
    // The build number, bottom centre, so a round can be pinned to the build that sailed it.
    let build = cmd
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                bottom: px(14),
                justify_content: JustifyContent::Center,
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    text(cmd, build, build_label(), 13., ink().with_alpha(0.6));

    let panel = cmd
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: percent(5.5),
                bottom: percent(8.),
                width: px(380),
                max_width: percent(89),
                ..column(12.)
            },
            ChildOf(root),
        ))
        .id();
    caption(cmd, panel, "BREAKWATER   /   COASTAL PATROL");
    cmd.spawn((
        Text::new("SHIPVIVOR"),
        TextFont::from_font_size(74.).with_font(face.clone()),
        TextColor(Color::srgb_u8(249, 244, 221)),
        ChildOf(panel),
    ));
    text(
        cmd,
        panel,
        "Clear enemy drydocks to break through the patrols. Earn stars for new routes, then hunt the Iron Regent.",
        16.,
        ink(),
    );

    let choices = cmd
        .spawn((
            Node {
                margin: UiRect::top(px(8)),
                ..column(8.)
            },
            ChildOf(panel),
        ))
        .id();
    for (action, label, detail, shortcut) in [
        (
            Action::Start,
            "New run",
            "Choose your first patrol on the chart",
            "ENTER",
        ),
        (
            Action::Trial,
            "Helm trial",
            "Practice on the five-gate buoy course",
            "T",
        ),
        (
            Action::Customise,
            "Customise",
            "Pick the flag Breakwater flies",
            "C",
        ),
        (
            Action::Options,
            "Options",
            "Choose how the ship earns experience",
            "O",
        ),
    ] {
        let button = button(cmd, choices, action, face, label, shortcut);
        let foreground = if action == Action::Start {
            Color::srgb_u8(24, 43, 47)
        } else {
            ink()
        };
        text(cmd, button.labels, detail, 12., foreground.with_alpha(0.87));
    }
}

/// A brass rule and a line of small capitals over a panel.
pub(super) fn caption(cmd: &mut Commands, panel: Entity, words: &str) {
    let caption = cmd
        .spawn((
            Node {
                align_items: AlignItems::Center,
                column_gap: px(11),
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    cmd.spawn((
        Node {
            width: px(32),
            height: px(2),
            ..default()
        },
        BackgroundColor(brass()),
        ChildOf(caption),
    ));
    text(cmd, caption, words, 14., brass());
}

pub(super) struct Button {
    pub node: Entity,
    /// The column holding the label, for a line of detail under it.
    pub labels: Entity,
}

/// A title button: a label on the left and its key on the right. `New run` is filled
/// with brass; the rest are outlined.
pub(super) fn button(
    cmd: &mut Commands,
    parent: Entity,
    action: Action,
    face: &Handle<Font>,
    label: &str,
    shortcut: &str,
) -> Button {
    let primary = action == Action::Start;
    let node = cmd
        .spawn((
            Node {
                width: percent(100),
                height: px(59),
                padding: UiRect::horizontal(px(15)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                border: UiRect::all(px(1)),
                ..default()
            },
            BorderColor::all(if primary {
                brass()
            } else {
                Color::srgba_u8(192, 213, 202, 100)
            }),
            BackgroundColor(fill(action, false)),
            action,
            RelativeCursorPosition::default(),
            ChildOf(parent),
        ))
        .id();
    let foreground = if primary {
        Color::srgb_u8(24, 43, 47)
    } else {
        ink()
    };
    let labels = cmd.spawn((column(0.), ChildOf(node))).id();
    cmd.spawn((
        Text::new(label),
        TextFont::from_font_size(21.).with_font(face.clone()),
        TextColor(foreground),
        ChildOf(labels),
    ));
    keycap(cmd, node, shortcut, foreground);
    Button { node, labels }
}

/// A key's name in a thin box, at the right of a choice.
pub(super) fn keycap(cmd: &mut Commands, parent: Entity, shortcut: &str, foreground: Color) {
    let keycap = cmd
        .spawn((
            Node {
                min_width: px(26),
                padding: UiRect::axes(px(6), px(2)),
                justify_content: JustifyContent::Center,
                border: UiRect::all(px(1)),
                ..default()
            },
            BorderColor::all(foreground.with_alpha(0.45)),
            ChildOf(parent),
        ))
        .id();
    text(cmd, keycap, shortcut, 12., foreground);
}

/// Leave the starboard side of the frame open for the ship and keep type legible over sunlit water.
pub(super) fn backdrop(cmd: &mut Commands, root: Entity) {
    let shade = |alpha| Color::srgba_u8(7, 23, 33, alpha);
    cmd.entity(root).insert(BackgroundGradient(vec![
        LinearGradient::to_right(vec![
            ColorStop::percent(shade(202), 0.),
            ColorStop::percent(shade(147), 24.),
            ColorStop::percent(shade(0), 57.),
        ])
        .into(),
        LinearGradient::to_top(vec![
            ColorStop::percent(shade(224), 0.),
            ColorStop::percent(shade(118), 22.),
            ColorStop::percent(shade(0), 54.),
        ])
        .into(),
    ]));
}

pub(super) fn fill(action: Action, hovered: bool) -> Color {
    match (action, hovered) {
        (Action::Start, true) => Color::srgb_u8(255, 224, 146),
        (Action::Start, false) => Color::srgb_u8(236, 188, 99),
        (_, true) => button_color(true),
        (_, false) => Color::srgba_u8(16, 39, 49, 213),
    }
}
