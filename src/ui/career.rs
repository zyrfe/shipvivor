//! The title's career page: one screen with a captain and a hull, each picked and
//! previewed independently and in any order, the captain in command's skills raised in
//! place with career points, and a button that locks the pair in and launches the run.
//! Nothing is locked until that button. Every captain keeps a career of their own, so
//! taking another to sea loses nothing.
use super::*;
use crate::career::{CareerBook, Launch};
use crate::crew_book::CrewBook;
use crate::state::PlayerHelm;
use shipvivor_campaign::crew::{Skill, Wardroom};
use shipvivor_campaign::{CareerStart, StartingHull};
use shipvivor_encounter::MainShell;
use shipvivor_shipyard::TubeAim;

/// The career page's own root, drawn over the screen root as the career changes.
#[derive(Component)]
pub(crate) struct CareerPage;

/// Choices on the career page, opening it, and the launch's hand-over to the chart.
/// Anything else goes on to the run.
pub fn choose(
    In(action): In<Option<Action>>,
    mut run: ResMut<Campaign>,
    mut book: ResMut<CareerBook>,
    crew: Res<CrewBook>,
    helm: Res<PlayerHelm>,
) -> Option<Action> {
    // The ship has sailed off: the chart of anchorages takes the screen, as for any run.
    if let Some(launch) = book.launch.as_mut()
        && launch.sailed()
        && !launch.arrived
    {
        launch.arrived = true;
        run.title_page = crate::state::TitlePage::Menu;
        return Some(Action::Start);
    }
    let action = action?;
    if book.launch.is_some() && run.screen() == Screen::Career {
        // The sail is skipped by its own click or ESC; nothing else is taken meanwhile.
        return None;
    }
    let wardroom = &crew.wardroom;
    match (run.screen(), action) {
        // A plain new run is no career's.
        (Screen::Title, Action::Start) => {
            run.career = None;
            return Some(action);
        }
        (Screen::Title, Action::Career) => {
            run.title_page = crate::state::TitlePage::Career;
            book.open_page(&crew);
        }
        (Screen::Career, Action::Title) => run.title_page = crate::state::TitlePage::Menu,
        (Screen::Career, Action::PickCaptain(at)) => {
            if at < wardroom.captains.len() && book.captain != at {
                book.captain = at;
                book.revision = book.revision.wrapping_add(1);
            }
        }
        (Screen::Career, Action::Vessel(hull)) => {
            if book.hull != hull {
                book.hull = hull;
                book.revision = book.revision.wrapping_add(1);
            }
        }
        (Screen::Career, Action::Raise(at)) => {
            // The previewed captain spends their own career's points.
            let Some(picked) = wardroom.captains.get(book.captain).map(|c| c.id.clone()) else {
                return None;
            };
            let Some(career) = book.roster.get_mut(&picked) else {
                return None;
            };
            let Some(&(skill, _)) = career.skills.get(at) else {
                return None;
            };
            if career.raise(skill, wardroom) {
                book.changed();
            }
        }
        (Screen::Career, Action::Launch) => {
            let Some(captain) = wardroom.captains.get(book.captain) else {
                return None;
            };
            // A captain who has not sailed yet begins their career; one who has takes up
            // their own where they left it.
            let aboard = book.roster.enlist(captain).aboard(wardroom);
            book.roster.last = Some(captain.id.clone());
            book.changed();
            run.career = Some(CareerStart {
                hull: book.hull,
                captain: aboard,
            });
            // The game opens on this ship next time, under this captain.
            crate::career::keep_ship(book.hull);
            book.launch = Some(Launch {
                elapsed: 0.,
                anchor: PlayerHelm(helm.0.clone()),
                arrived: false,
            });
            book.revision = book.revision.wrapping_add(1);
        }
        (Screen::Career, _) => {}
        _ => return Some(action),
    }
    None
}

/// Draw the career page on show, again whenever the career or a pick changes, and take
/// it down on leaving or while the ship sails off.
pub fn draw(
    mut cmd: Commands,
    run: Res<Campaign>,
    book: Res<CareerBook>,
    crew: Res<CrewBook>,
    art: Option<Res<crate::insignia::Insignia>>,
    captain_art: Option<Res<crate::captain::CaptainArt>>,
    pages: Query<Entity, With<CareerPage>>,
    mut last: Local<Option<u32>>,
) {
    let showing = run.screen() == Screen::Career && book.launch.is_none();
    let key = showing.then_some(book.revision);
    if *last == key && !crew.is_changed() {
        return;
    }
    *last = key;
    for page in &pages {
        cmd.entity(page).despawn();
    }
    let Some(art) = art.filter(|_| showing) else {
        return;
    };
    let root = cmd
        .spawn((
            CareerPage,
            GlobalZIndex(1),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: percent(100),
                height: percent(100),
                ..default()
            },
        ))
        .id();
    let face = &art.display_face;
    let wardroom = &crew.wardroom;
    let columns = cmd
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: percent(4.),
                top: percent(6.),
                column_gap: px(22),
                align_items: AlignItems::FlexStart,
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    let captains = cmd
        .spawn((
            Node {
                width: px(430),
                ..column(9.)
            },
            ChildOf(columns),
        ))
        .id();
    captain_column(
        &mut cmd,
        captains,
        face,
        &book,
        wardroom,
        captain_art.as_deref(),
    );
    let hulls = cmd
        .spawn((
            Node {
                width: px(330),
                ..column(9.)
            },
            ChildOf(columns),
        ))
        .id();
    hull_column(&mut cmd, hulls, face, &book);
    launch_bar(&mut cmd, root, face);
}

fn heading(cmd: &mut Commands, panel: Entity, face: &Handle<Font>, words: &str) {
    cmd.spawn((
        Text::new(words),
        TextFont::from_font_size(36.).with_font(face.clone()),
        TextColor(Color::srgb_u8(249, 244, 221)),
        ChildOf(panel),
    ));
}

/// A pickable tile: a label and its key, lit with a brass border when picked.
fn tile(
    cmd: &mut Commands,
    parent: Entity,
    face: &Handle<Font>,
    action: Action,
    label: &str,
    (shortcut, picked): (&str, bool),
) {
    let tile = cmd
        .spawn((
            Node {
                padding: UiRect::axes(px(10), px(7)),
                border: UiRect::all(px(2)),
                align_items: AlignItems::Center,
                column_gap: px(8),
                ..default()
            },
            BorderColor::all(if picked {
                brass()
            } else {
                Color::srgba_u8(192, 213, 202, 50)
            }),
            BackgroundColor(title::fill(action, false)),
            action,
            RelativeCursorPosition::default(),
            ChildOf(parent),
        ))
        .id();
    cmd.spawn((
        Text::new(label),
        TextFont::from_font_size(16.).with_font(face.clone()),
        TextColor(if picked { brass() } else { ink() }),
        ChildOf(tile),
    ));
    title::keycap(cmd, tile, shortcut, ink());
}

/// A preview card: a dark panel with a thin brass edge.
fn card(cmd: &mut Commands, parent: Entity) -> Entity {
    cmd.spawn((
        Node {
            padding: UiRect::all(px(12)),
            border: UiRect::all(px(1)),
            ..column(8.)
        },
        BorderColor::all(brass().with_alpha(0.55)),
        BackgroundColor(Color::srgba_u8(10, 28, 37, 226)),
        ChildOf(parent),
    ))
    .id()
}

/// Tier pips: a filled bar for each tier reached, an empty one for each to come.
fn pips(cmd: &mut Commands, parent: Entity, tier: u8, most: usize) {
    let strip = cmd
        .spawn((
            Node {
                column_gap: px(4),
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    for at in 0..most {
        cmd.spawn((
            Node {
                width: px(16),
                height: px(6),
                border: UiRect::all(px(1)),
                ..default()
            },
            BorderColor::all(brass()),
            BackgroundColor(if at < usize::from(tier) {
                brass()
            } else {
                Color::NONE
            }),
            ChildOf(strip),
        ));
    }
}

fn captain_column(
    cmd: &mut Commands,
    panel: Entity,
    face: &Handle<Font>,
    book: &CareerBook,
    wardroom: &Wardroom,
    art: Option<&crate::captain::CaptainArt>,
) {
    title::caption(cmd, panel, "CAREER   /   CAPTAIN");
    heading(cmd, panel, face, "Captain");
    let rank = wardroom
        .ladder
        .last()
        .map_or("Capt.", |rank| rank.short.as_str());
    let tiles = cmd
        .spawn((
            Node {
                flex_wrap: FlexWrap::Wrap,
                column_gap: px(6),
                row_gap: px(6),
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    for (at, captain) in wardroom.captains.iter().enumerate() {
        let surname = captain.name.rsplit(' ').next().unwrap_or(&captain.name);
        tile(
            cmd,
            tiles,
            face,
            Action::PickCaptain(at),
            surname,
            (&(at + 1).to_string(), at == book.captain),
        );
    }
    let Some(picked) = wardroom.captains.get(book.captain) else {
        return;
    };
    let in_command = book.roster.get(&picked.id);
    let preview = card(cmd, panel);

    // Portrait and name.
    let head = cmd
        .spawn((
            Node {
                column_gap: px(12),
                align_items: AlignItems::Center,
                ..default()
            },
            ChildOf(preview),
        ))
        .id();
    if let Some(art) = art {
        let (image, rect) = art.portrait(&picked.id);
        cmd.spawn((
            Node {
                width: px(84),
                height: px(84),
                border: UiRect::all(px(1)),
                ..default()
            },
            BorderColor::all(brass().with_alpha(0.7)),
            ImageNode {
                image,
                rect: Some(rect),
                ..default()
            },
            ChildOf(head),
        ));
    }
    let names = cmd.spawn((column(3.), ChildOf(head))).id();
    cmd.spawn((
        Text::new(format!("{rank} {}", picked.name)),
        TextFont::from_font_size(21.).with_font(face.clone()),
        TextColor(ink()),
        ChildOf(names),
    ));
    let standing = match in_command {
        Some(career) => format!(
            "In command: {} run{}, {} won, {} star{}",
            career.runs,
            if career.runs == 1 { "" } else { "s" },
            career.wins,
            career.stars,
            if career.stars == 1 { "" } else { "s" },
        ),
        None => "A new career".to_string(),
    };
    text(cmd, names, standing, 12., pale());
    if let Some(career) = in_command {
        let banked = (book.last_banked.as_ref())
            .filter(|(id, _)| *id == picked.id)
            .map_or(String::new(), |(_, points)| {
                format!("  (+{points} last run)")
            });
        text(
            cmd,
            names,
            format!("{} career points{banked}", career.points),
            13.,
            brass(),
        );
    }

    // Skills, with their tiers; the captain in command's raised here.
    let skills: Vec<(Skill, u8)> = in_command.map_or(picked.skills.clone(), |c| c.skills.clone());
    let keys = ["Q", "W", "E"];
    for (at, &(skill, tier)) in skills.iter().enumerate() {
        let skill_book = wardroom.skill(skill);
        let line = cmd.spawn((column(4.), ChildOf(preview))).id();
        let top = cmd
            .spawn((
                Node {
                    column_gap: px(10),
                    align_items: AlignItems::Center,
                    ..default()
                },
                ChildOf(line),
            ))
            .id();
        cmd.spawn((
            Text::new(skill_book.name.clone()),
            TextFont::from_font_size(16.).with_font(face.clone()),
            TextColor(ink()),
            ChildOf(top),
        ));
        pips(cmd, top, tier, skill_book.tiers.len());
        text(cmd, line, skill_book.tier(tier).text.clone(), 13., ink());
        let next = in_command.and_then(|career| Some((career, career.next_cost(skill, wardroom)?)));
        match next {
            Some((career, cost)) => {
                let affordable = cost <= career.points;
                let raise = cmd
                    .spawn((
                        Node {
                            padding: UiRect::axes(px(8), px(4)),
                            border: UiRect::all(px(1)),
                            align_items: AlignItems::Center,
                            column_gap: px(8),
                            align_self: AlignSelf::FlexStart,
                            ..default()
                        },
                        BorderColor::all(if affordable {
                            brass()
                        } else {
                            ink().with_alpha(0.25)
                        }),
                        BackgroundColor(title::fill(Action::Career, false)),
                        ChildOf(line),
                    ))
                    .id();
                if affordable {
                    cmd.entity(raise)
                        .insert((Action::Raise(at), RelativeCursorPosition::default()));
                }
                let colour = if affordable {
                    ink()
                } else {
                    ink().with_alpha(0.45)
                };
                text(
                    cmd,
                    raise,
                    format!("Raise for {cost} pts: {}", skill_book.tier(tier + 1).text),
                    12.,
                    colour,
                );
                title::keycap(cmd, raise, keys.get(at).copied().unwrap_or(""), colour);
            }
            None if in_command.is_some() => {
                text(cmd, line, "At its last tier", 11., pale());
            }
            None => {}
        }
    }
    if in_command.is_none() {
        text(
            cmd,
            preview,
            "Skills work ship-wide. Every run in command pays career points to raise them.",
            12.,
            pale(),
        );
    }
}

fn hull_column(cmd: &mut Commands, panel: Entity, face: &Handle<Font>, book: &CareerBook) {
    title::caption(cmd, panel, "SHIP");
    heading(cmd, panel, face, "Ship");
    let tiles = cmd
        .spawn((
            Node {
                column_gap: px(6),
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    for (hull, key) in StartingHull::ALL.into_iter().zip(["A", "S"]) {
        tile(
            cmd,
            tiles,
            face,
            Action::Vessel(hull),
            hull.name(),
            (key, hull == book.hull),
        );
    }
    let hull = book.hull;
    let design = hull.design();
    let sheet = design.sheet();
    let reach = MainShell {
        velocity: hull.fittings().velocity,
        ..MainShell::STOCK
    }
    .reach();
    let tubes: Vec<&str> = design
        .tube_mounts()
        .iter()
        .map(|t| match t.aim {
            TubeAim::Ahead => "bow",
            TubeAim::Beam => "amidships",
            TubeAim::Astern => "stern",
        })
        .collect();
    let preview = card(cmd, panel);
    cmd.spawn((
        Text::new(hull.name()),
        TextFont::from_font_size(21.).with_font(face.clone()),
        TextColor(ink()),
        ChildOf(preview),
    ));
    text(cmd, preview, hull.summary(), 13., pale());
    let figures = [
        ("Length", format!("{:.0} m", sheet.length)),
        ("Top speed", format!("{:.0} kn", knots(sheet.top_speed))),
        ("Gun reach", format!("{:.0} m", reach)),
        // The UI face has no double prime, so the inches are spelled out.
        ("Plate", format!("{} in", hull.protection().thickness)),
        (
            "Torpedo tubes",
            if tubes.is_empty() {
                "None".to_string()
            } else {
                tubes.join(", ")
            },
        ),
    ];
    for (label, value) in figures {
        let row = cmd
            .spawn((
                Node {
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                },
                ChildOf(preview),
            ))
            .id();
        text(cmd, row, label, 13., pale());
        text(cmd, row, value, 13., ink());
    }
}

/// The lock-in button at the foot of the page, centred, with ESC beside it.
fn launch_bar(cmd: &mut Commands, root: Entity, face: &Handle<Font>) {
    let bar = cmd
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                bottom: percent(5.),
                align_items: AlignItems::Center,
                ..column(8.)
            },
            ChildOf(root),
        ))
        .id();
    let label = "Lock in and launch".to_string();
    let row = cmd
        .spawn((
            Node {
                column_gap: px(10),
                ..default()
            },
            ChildOf(bar),
        ))
        .id();
    for (action, label, key, width) in [
        (Action::Title, "Back".to_string(), "ESC", 150.),
        (Action::Launch, label, "ENTER", 360.),
    ] {
        let b = title::button(cmd, row, action, face, &label, key);
        cmd.entity(b.node).insert(Node {
            width: px(width),
            height: px(56),
            padding: UiRect::horizontal(px(15)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            border: UiRect::all(px(1)),
            ..default()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Mode;

    fn app() -> App {
        let mut app = App::new();
        app.insert_resource(Campaign::new(Mode::Title))
            .insert_resource(PlayerHelm::cruising())
            .add_plugins(crate::crew_book::CrewBookPlugin)
            .init_resource::<CareerBook>()
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

    fn screen(app: &App) -> Screen {
        app.world().resource::<Campaign>().screen()
    }

    fn book(app: &mut App) -> Mut<'_, CareerBook> {
        app.world_mut().resource_mut::<CareerBook>()
    }

    #[test]
    fn captain_and_hull_are_picked_in_any_order_and_locked_in_only_at_the_launch() {
        let wardroom = Wardroom::stock();
        let mut app = app();
        assert_eq!(press(&mut app, Action::Career), None);
        assert_eq!(screen(&app), Screen::Career);
        // The first captain and the light cruiser are picked, so the launch works at once.
        assert_eq!(book(&mut app).captain, 0);
        assert_eq!(book(&mut app).hull, StartingHull::LightCruiser);
        // Picks preview, in any order, and lock nothing in.
        press(&mut app, Action::Vessel(StartingHull::HeavyDestroyer));
        press(&mut app, Action::PickCaptain(2));
        press(&mut app, Action::PickCaptain(1));
        assert!(book(&mut app).roster.careers().is_empty());
        assert!(app.world().resource::<Campaign>().career.is_none());

        // The launch locks both in, sails, and hands over to the chart of anchorages.
        assert_eq!(press(&mut app, Action::Launch), None);
        let roster = book(&mut app).roster.clone();
        assert!(roster.get(&wardroom.captains[1].id).is_some());
        assert_eq!(
            roster.last.as_deref(),
            Some(wardroom.captains[1].id.as_str())
        );
        let start = app.world().resource::<Campaign>().career.clone().unwrap();
        assert_eq!(start.hull, StartingHull::HeavyDestroyer);
        assert_eq!(start.captain.id, wardroom.captains[1].id);
        // Nothing else is taken while she sails.
        assert_eq!(press(&mut app, Action::Title), None);
        assert_eq!(screen(&app), Screen::Career);
        book(&mut app).launch.as_mut().unwrap().skip();
        assert_eq!(press(&mut app, Action::Title), Some(Action::Start));
        assert!(book(&mut app).launch.as_ref().unwrap().arrived);
        // A plain new run from the title is no career's.
        assert_eq!(press(&mut app, Action::Start), Some(Action::Start));
        assert!(app.world().resource::<Campaign>().career.is_none());
    }

    #[test]
    fn each_captain_raises_their_own_skills_and_keeps_their_career_when_another_sails() {
        let wardroom = Wardroom::stock();
        let mut app = app();
        let (thorne, marrack) = (&wardroom.captains[2], &wardroom.captains[0]);
        book(&mut app).roster.enlist(thorne).points = 6;
        book(&mut app).roster.enlist(marrack).points = 6;
        book(&mut app).roster.last = Some(thorne.id.clone());
        press(&mut app, Action::Career);
        // The page opens on the captain who sailed last.
        assert_eq!(book(&mut app).captain, 2);
        press(&mut app, Action::Raise(0));
        let raised = book(&mut app).roster.get(&thorne.id).cloned().unwrap();
        assert_eq!((raised.skills[0].1, raised.points), (2, 0));
        // Another captain spends their own points, and launching with them takes
        // nothing from the first.
        press(&mut app, Action::PickCaptain(0));
        press(&mut app, Action::Raise(0));
        assert_eq!(book(&mut app).roster.get(&marrack.id).unwrap().points, 0);
        press(&mut app, Action::Launch);
        let roster = book(&mut app).roster.clone();
        assert_eq!(roster.get(&thorne.id), Some(&raised));
        assert_eq!(roster.last.as_deref(), Some(marrack.id.as_str()));
        let start = app.world().resource::<Campaign>().career.clone().unwrap();
        assert_eq!(
            start.captain.skills,
            roster.get(&marrack.id).unwrap().skills
        );
    }
}
