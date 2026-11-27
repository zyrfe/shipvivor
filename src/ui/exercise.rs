//! Test mode's list of seas, opened from the title over the same cruise, or from test
//! mode's yard with the ship built there kept. Any sea of any round can be sailed on its
//! own at either grade, from a yard that charges nothing: a round and a grade are set
//! here, and a click on a sea opens the yard before it.
use super::*;
use shipvivor_campaign::POOLS;
use shipvivor_sim::{Par, Sector};

/// The seas of a round, from 1 for the anchorages to [`ROUNDS`] for Crown Deep alone.
pub(crate) fn seas(round: usize) -> &'static [Stage] {
    match round.checked_sub(1).and_then(|pool| POOLS.get(pool)) {
        Some(pool) => pool,
        None => &[Stage::CrownDeep],
    }
}

/// What the grade changes in a round, in a line under its buttons.
fn grade_note(round: usize, grade: Grade) -> &'static str {
    match (round, grade) {
        (1, _) => "An anchorage is sailed the same at either grade.",
        (ROUNDS, Grade::Tough) => "Tough: the Regent's bombers fly from her air station.",
        (ROUNDS, Grade::Moderate) => "Moderate: the Regent fights without her air station.",
        (_, Grade::Tough) => "Tough: air cover, raids sooner and quicker, heavier ships earlier.",
        (_, Grade::Moderate) => "Moderate: no air cover, and the raids build up at their own pace.",
    }
}

/// Par as the list of seas and the yard both word it: what a run brings to the round,
/// and what that comes to at the yard's prices.
pub(crate) fn par_line(par: Par, grade: Grade) -> String {
    if par.worth() == 0 {
        return "Par for round 1: a stock ship, at level 1.".to_string();
    }
    format!(
        "Par for round {} by {} passages: {} yard points and {} fittings, about {} points of work. Level {}.",
        par.round,
        grade.name().to_lowercase(),
        par.points,
        par.fittings,
        par.worth(),
        par.level
    )
}

pub(super) fn show(cmd: &mut Commands, root: Entity, run: &Campaign, face: &Handle<Font>) {
    title::backdrop(cmd, root);
    let (round, grade) = run.bench;
    let panel = cmd
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: percent(5.5),
                bottom: percent(8.),
                width: px(640),
                max_width: percent(89),
                ..column(10.)
            },
            ChildOf(root),
        ))
        .id();
    title::caption(cmd, panel, "TEST MODE   /   ANY SEA, ANY GRADE");
    cmd.spawn((
        Text::new("Choose a sea"),
        TextFont::from_font_size(44.).with_font(face.clone()),
        TextColor(Color::srgb_u8(249, 244, 221)),
        ChildOf(panel),
    ));
    text(
        cmd,
        panel,
        if run.is_exercise() {
            "She keeps the ship you built. Its yard charges nothing, and shows what a run would have brought here."
        } else {
            "Fit her out in a yard that charges nothing, and shows what a run would have brought here. Then sail the sea alone, as often as you like."
        },
        15.,
        ink(),
    );

    let row = |cmd: &mut Commands, label: &str| {
        let row = cmd
            .spawn((
                Node {
                    align_items: AlignItems::Center,
                    column_gap: px(6),
                    margin: UiRect::top(px(4)),
                    ..default()
                },
                ChildOf(panel),
            ))
            .id();
        cmd.spawn((
            Text::new(label),
            TextFont::from_font_size(12.),
            TextColor(brass()),
            Node {
                width: px(58),
                ..default()
            },
            ChildOf(row),
        ));
        row
    };
    let rounds = row(cmd, "ROUND");
    for number in 1..=ROUNDS {
        let label = if number == ROUNDS {
            format!("{number}  Crown Deep")
        } else {
            number.to_string()
        };
        tab(
            cmd,
            rounds,
            Action::TestRound(number),
            &label,
            number == round,
        );
    }
    let grades = row(cmd, "GRADE");
    for choice in [Grade::Moderate, Grade::Tough] {
        tab(
            cmd,
            grades,
            Action::TestGrade(choice),
            choice.name(),
            choice == grade,
        );
    }
    title::keycap(cmd, grades, "G", ink());
    text(cmd, panel, grade_note(round, grade), 13., pale());
    text(
        cmd,
        panel,
        par_line(Par::at(round, grade), grade),
        13.,
        cyan(),
    );

    // The round's seas, three across, as its chart would deal them: one of each fleet.
    let grid = cmd
        .spawn((
            Node {
                flex_wrap: FlexWrap::Wrap,
                column_gap: px(8),
                row_gap: px(8),
                margin: UiRect::top(px(6)),
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    for &stage in seas(round) {
        let sector = Sector::of(stage, grade);
        let card = cmd
            .spawn((
                Node {
                    width: px(208),
                    padding: UiRect::axes(px(11), px(8)),
                    border: UiRect::all(px(1)),
                    ..column(3.)
                },
                BorderColor::all(Color::srgba_u8(192, 213, 202, 100)),
                BackgroundColor(title::fill(Action::TestSea(stage), false)),
                Action::TestSea(stage),
                RelativeCursorPosition::default(),
                ChildOf(grid),
            ))
            .id();
        cmd.spawn((
            Text::new(stage.mission().name),
            TextFont::from_font_size(17.).with_font(face.clone()),
            TextColor(ink()),
            ChildOf(card),
        ));
        text(
            cmd,
            card,
            format!("{}, {} bases", sector.fleet().name(), sector.bases()),
            12.,
            brass(),
        );
        text(cmd, card, sector.note(), 12., pale());
    }

    let back = title::button(
        cmd,
        panel,
        Action::Title,
        face,
        if run.is_exercise() {
            "Give up the ship: back to title"
        } else {
            "Back to title"
        },
        "ESC",
    );
    cmd.entity(back.node).insert(Node {
        width: px(380),
        height: px(48),
        margin: UiRect::top(px(8)),
        padding: UiRect::horizontal(px(15)),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::SpaceBetween,
        border: UiRect::all(px(1)),
        ..default()
    });
}

/// One of a row of choices of which one is made: a round or a grade. The one made is
/// lit in brass.
fn tab(cmd: &mut Commands, row: Entity, action: Action, label: &str, chosen: bool) {
    let tab = cmd
        .spawn((
            Node {
                min_width: px(38),
                height: px(34),
                padding: UiRect::horizontal(px(11)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(if chosen { px(2) } else { px(1) }),
                ..default()
            },
            BorderColor::all(if chosen {
                brass()
            } else {
                Color::srgba_u8(192, 213, 202, 100)
            }),
            BackgroundColor(title::fill(action, false)),
            action,
            RelativeCursorPosition::default(),
            ChildOf(row),
        ))
        .id();
    text(cmd, tab, label, 15., if chosen { brass() } else { ink() });
}
