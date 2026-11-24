use super::*;

/// Leave the starboard side of the frame open for the ship and keep type legible over sunlit water.
pub(super) fn show(cmd: &mut Commands, root: Entity, face: &Handle<Font>) {
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
    text(
        cmd,
        caption,
        "BREAKWATER   /   COASTAL PATROL",
        14.,
        brass(),
    );
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
            "Twin 4-inch mounts fore and aft",
            "ENTER",
        ),
        (
            Action::Trial,
            "Helm trial",
            "Practice on the five-gate buoy course",
            "T",
        ),
    ] {
        let primary = action == Action::Start;
        let button = cmd
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
                ChildOf(choices),
            ))
            .id();
        let foreground = if primary {
            Color::srgb_u8(24, 43, 47)
        } else {
            ink()
        };
        let labels = cmd.spawn((column(0.), ChildOf(button))).id();
        cmd.spawn((
            Text::new(label),
            TextFont::from_font_size(21.).with_font(face.clone()),
            TextColor(foreground),
            ChildOf(labels),
        ));
        text(cmd, labels, detail, 12., foreground.with_alpha(0.87));
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
                ChildOf(button),
            ))
            .id();
        text(cmd, keycap, shortcut, 12., foreground);
    }
}

pub(super) fn fill(action: Action, hovered: bool) -> Color {
    match (action, hovered) {
        (Action::Start, true) => Color::srgb_u8(255, 224, 146),
        (Action::Start, false) => Color::srgb_u8(236, 188, 99),
        (_, true) => button_color(true),
        (_, false) => Color::srgba_u8(16, 39, 49, 213),
    }
}
