//! The options screen, opened from the title over the same cruise. For now it holds one
//! choice: whether experience comes straight from what the ship sinks and wrecks, or from
//! the packages the wrecks leave to tow in.
use super::*;
use crate::options::GameOptions;
use shipvivor_encounter::Rewards;

/// Each way experience can come aboard, as the options screen offers it, with its key.
pub(super) const REWARDS: [(Rewards, &str, &str); 2] = [
    (
        Rewards::Direct,
        "Direct XP",
        "Sinking ships and wrecking yards pays XP and repairs at once",
    ),
    (
        Rewards::Packages,
        "Packages",
        "Wrecks leave packages to tow in for XP and repairs",
    ),
];

/// A choice on the options screen. It lights its border when it is the one made.
#[derive(Component)]
pub(crate) struct Choice;

/// Choices made on the options screen, and opening and closing it. Anything else goes
/// on to the run.
pub fn choose(
    In(action): In<Option<Action>>,
    mut run: ResMut<Campaign>,
    mut options: ResMut<GameOptions>,
) -> Option<Action> {
    let action = action?;
    match (run.screen(), action) {
        (Screen::Title, Action::Options) => run.title_page = crate::state::TitlePage::Options,
        (Screen::Options, Action::Title) => run.title_page = crate::state::TitlePage::Menu,
        (Screen::Options, Action::Rewards(rewards)) => {
            if options.rewards != rewards {
                options.rewards = rewards;
                crate::options::keep(&options);
            }
        }
        _ => return Some(action),
    }
    None
}

pub(super) fn show(cmd: &mut Commands, root: Entity, face: &Handle<Font>) {
    title::backdrop(cmd, root);
    let panel = cmd
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: percent(5.5),
                bottom: percent(8.),
                width: px(380),
                max_width: percent(89),
                ..column(10.)
            },
            ChildOf(root),
        ))
        .id();
    title::caption(cmd, panel, "BREAKWATER   /   OPTIONS");
    cmd.spawn((
        Text::new("Options"),
        TextFont::from_font_size(40.).with_font(face.clone()),
        TextColor(Color::srgb_u8(249, 244, 221)),
        ChildOf(panel),
    ));

    text(cmd, panel, "REWARDS", 12., pale());
    let choices = cmd.spawn((column(6.), ChildOf(panel))).id();
    for (i, (rewards, label, detail)) in REWARDS.into_iter().enumerate() {
        let action = Action::Rewards(rewards);
        let row = cmd
            .spawn((
                Node {
                    padding: UiRect::axes(px(14), px(10)),
                    border: UiRect::all(px(2)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    column_gap: px(12),
                    ..default()
                },
                BorderColor::all(Color::NONE),
                BackgroundColor(title::fill(action, false)),
                action,
                Choice,
                RelativeCursorPosition::default(),
                ChildOf(choices),
            ))
            .id();
        let labels = cmd
            .spawn((
                Node {
                    flex_shrink: 1.,
                    ..column(3.)
                },
                ChildOf(row),
            ))
            .id();
        cmd.spawn((
            Text::new(label),
            TextFont::from_font_size(19.).with_font(face.clone()),
            TextColor(ink()),
            ChildOf(labels),
        ));
        text(cmd, labels, detail, 12., ink().with_alpha(0.87));
        title::keycap(cmd, row, &(i + 1).to_string(), ink());
    }

    let done = title::button(cmd, panel, Action::Title, face, "Done", "ESC");
    cmd.entity(done.node).insert(Node {
        width: percent(100),
        height: px(52),
        margin: UiRect::top(px(6)),
        padding: UiRect::horizontal(px(14)),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::SpaceBetween,
        border: UiRect::all(px(1)),
        ..default()
    });
}

/// Light the choice made, and the one under the pointer.
pub fn refresh(
    options: Res<GameOptions>,
    mut choices: Query<(&Action, &RelativeCursorPosition, &mut BorderColor), With<Choice>>,
) {
    for (&action, cursor, mut border) in &mut choices {
        let colour = match action {
            Action::Rewards(rewards) if rewards == options.rewards => brass(),
            _ if cursor.cursor_over() => ink().with_alpha(0.8),
            _ => Color::srgba_u8(192, 213, 202, 38),
        };
        let lit = BorderColor::all(colour);
        if *border != lit {
            *border = lit;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Mode;

    fn app() -> App {
        let mut app = App::new();
        app.insert_resource(Campaign::new(Mode::Title))
            .init_resource::<GameOptions>()
            .init_resource::<Events>()
            .add_systems(
                Update,
                (|mut events: ResMut<Events>| events.0.pop())
                    .pipe(choose)
                    .pipe(|In(rest): In<Option<Action>>, mut events: ResMut<Events>| {
                        events.1 = rest;
                    }),
            );
        app
    }

    #[derive(Resource, Default)]
    struct Events(Vec<Action>, Option<Action>);

    fn press(app: &mut App, action: Action) -> Option<Action> {
        app.world_mut().resource_mut::<Events>().0.push(action);
        app.update();
        app.world_mut().resource_mut::<Events>().1.take()
    }

    fn rewards(app: &App) -> Rewards {
        app.world().resource::<GameOptions>().rewards
    }

    #[test]
    fn the_title_opens_the_options_on_direct_xp_and_a_choice_sticks_after_esc() {
        let mut app = app();
        assert_eq!(rewards(&app), Rewards::Direct);
        // Rewards are only chosen on the options screen.
        assert_eq!(
            press(&mut app, Action::Rewards(Rewards::Packages)),
            Some(Action::Rewards(Rewards::Packages))
        );
        assert_eq!(rewards(&app), Rewards::Direct);
        assert_eq!(press(&mut app, Action::Options), None);
        assert_eq!(app.world().resource::<Campaign>().screen(), Screen::Options);
        // A run cannot start from behind the options.
        assert_eq!(press(&mut app, Action::Start), Some(Action::Start));
        assert_eq!(press(&mut app, Action::Rewards(Rewards::Packages)), None);
        assert_eq!(rewards(&app), Rewards::Packages);
        assert_eq!(press(&mut app, Action::Title), None);
        assert_eq!(app.world().resource::<Campaign>().screen(), Screen::Title);
        assert_eq!(rewards(&app), Rewards::Packages);
        // The title passes its own choices on.
        assert_eq!(press(&mut app, Action::Trial), Some(Action::Trial));
    }
}
