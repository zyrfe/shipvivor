//! The customise screen, opened from the title over the same cruise. For now it dresses
//! Breakwater's flag: a pattern, a colour for each of its slots, or a spin of the wheel
//! for both. The flag at her masthead is the preview, so the camera closes on it.
use super::*;
use crate::ensign::{self, Colours};
use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use shipvivor_ensign::{Flag, MAX_COLOURS, Pattern, Tincture};

/// Texels in a pattern tile and in a slot's icon.
const TILE: (u32, u32) = (112, 40);
const SLOT: (u32, u32) = (84, 30);
/// The slots a slot's icon is not about.
const REST: [u8; 3] = [52, 74, 80];
/// Turns of the wheel in one spin, the first wait between them, and how much longer
/// each wait is than the last as the wheel runs down.
const TURNS: u32 = 10;
const FIRST_WAIT: f32 = 0.04;
const SLOWING: f32 = 1.22;

#[derive(Resource)]
pub struct Locker {
    /// Every slot's colour, including slots the pattern does not use, so that a pattern
    /// with fewer slots and back again loses nothing.
    palette: [Tincture; MAX_COLOURS],
    pattern: Pattern,
    /// The slot the swatches colour.
    slot: usize,
    spin: Option<Spin>,
}

struct Spin {
    seed: u64,
    turns: u32,
    /// Until the next turn.
    wait: f32,
}

impl Default for Locker {
    fn default() -> Self {
        let mut locker = Self {
            palette: [Tincture::White; MAX_COLOURS],
            pattern: Pattern::Bands(2),
            slot: 0,
            spin: None,
        };
        locker.dress(&Flag::generate(0));
        locker
    }
}

impl Locker {
    /// Take up a flag's pattern and colours. Slots it leaves out get colours it does
    /// not use.
    fn dress(&mut self, flag: &Flag) {
        let colours = flag.colours();
        let mut spare = Tincture::ALL.into_iter().filter(|t| !colours.contains(t));
        for (i, slot) in self.palette.iter_mut().enumerate() {
            *slot = colours
                .get(i)
                .copied()
                .or_else(|| spare.next())
                .unwrap_or(Tincture::White);
        }
        self.pattern = flag.pattern();
        self.slot = self.slot.min(self.pattern.slots() - 1);
    }

    /// The palette laid into a pattern.
    fn flag_in(&self, pattern: Pattern) -> Flag {
        Flag::new(pattern, self.palette[..pattern.slots()].to_vec())
            .expect("every pattern has at most MAX_COLOURS slots")
    }

    fn flag(&self) -> Flag {
        self.flag_in(self.pattern)
    }

    fn selected(&self, action: Action) -> bool {
        match action {
            Action::Pattern(pattern) => pattern == self.pattern,
            Action::Slot(slot) => slot == self.slot,
            Action::Tincture(t) => self.palette[self.slot] == t,
            _ => false,
        }
    }
}

/// A pattern tile, slot or swatch. It lights its own border rather than its fill.
#[derive(Component)]
pub(crate) struct Picker;

/// A picture of the flag: a pattern in the palette's colours, or where a slot lies.
#[derive(Component, Clone, Copy)]
pub(crate) enum Cloth {
    Pattern(Pattern),
    Slot(usize),
}

/// Choices made on the customise screen, and opening and closing it. Anything else
/// goes on to the run.
pub fn choose(
    In(action): In<Option<Action>>,
    mut run: ResMut<Campaign>,
    mut locker: ResMut<Locker>,
    mut colours: ResMut<Colours>,
) -> Option<Action> {
    let action = action?;
    let before = colours.breakwater.clone();
    match (run.screen(), action) {
        (Screen::Title, Action::Customise) => {
            run.title_page = crate::state::TitlePage::Customise;
            locker.slot = 0;
            locker.dress(&before);
            return None;
        }
        (Screen::Customise, Action::Title) => {
            // A spin cut short keeps the flag it stopped on.
            if locker.spin.take().is_some() {
                ensign::stow(&before);
            }
            run.title_page = crate::state::TitlePage::Menu;
            return None;
        }
        (Screen::Customise, Action::Spin) => {
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |t| t.as_nanos() as u64);
            locker.spin = Some(Spin {
                seed,
                turns: 0,
                wait: 0.,
            });
            return None;
        }
        (Screen::Customise, Action::Pattern(pattern)) => {
            locker.pattern = pattern;
            locker.slot = locker.slot.min(pattern.slots() - 1);
        }
        (Screen::Customise, Action::Slot(slot)) => {
            if slot < locker.pattern.slots() {
                locker.slot = slot;
            }
            return None;
        }
        (Screen::Customise, Action::Tincture(t)) => {
            let slot = locker.slot;
            locker.palette[slot] = t;
        }
        _ => return Some(action),
    }
    locker.spin = None;
    let flag = locker.flag();
    if flag != before {
        ensign::stow(&flag);
        colours.breakwater = flag;
    }
    None
}

/// Run the wheel down: a new flag at each turn, each turn slower than the last, and the
/// last one kept.
pub fn turn_wheel(clock: Res<Time>, mut locker: ResMut<Locker>, mut colours: ResMut<Colours>) {
    let ticking = locker.bypass_change_detection();
    let Some(spin) = ticking.spin.as_mut() else {
        return;
    };
    spin.wait -= clock.delta_secs();
    if spin.wait > 0. {
        return;
    }
    spin.turns += 1;
    spin.wait = FIRST_WAIT * SLOWING.powi(spin.turns as i32);
    let flag = Flag::generate(spin.seed.wrapping_add(u64::from(spin.turns)));
    let last = spin.turns >= TURNS;
    if last {
        locker.spin = None;
        ensign::stow(&flag);
    }
    locker.dress(&flag);
    colours.breakwater = flag;
}

pub(super) fn show(cmd: &mut Commands, root: Entity, art: &crate::insignia::Insignia) {
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
    title::caption(cmd, panel, "BREAKWATER   /   COLOURS");
    cmd.spawn((
        Text::new("Customise"),
        TextFont::from_font_size(40.).with_font(art.display_face.clone()),
        TextColor(Color::srgb_u8(249, 244, 221)),
        ChildOf(panel),
    ));

    text(cmd, panel, "PATTERN", 12., pale());
    let patterns = cmd
        .spawn((
            Node {
                flex_wrap: FlexWrap::Wrap,
                column_gap: px(6),
                row_gap: px(4),
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    for pattern in Pattern::ALL {
        let frame = picker(cmd, patterns, Action::Pattern(pattern), Node::default());
        cmd.spawn((
            Node {
                width: px(TILE.0 as f32),
                height: px(TILE.1 as f32),
                ..default()
            },
            ImageNode::default(),
            Cloth::Pattern(pattern),
            ChildOf(frame),
        ));
    }

    text(cmd, panel, "COLOURS", 12., pale());
    let slots = cmd
        .spawn((
            Node {
                column_gap: px(6),
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    for slot in 0..MAX_COLOURS {
        let frame = picker(
            cmd,
            slots,
            Action::Slot(slot),
            Node {
                flex_grow: 1.,
                flex_basis: px(0),
                max_width: px(SLOT.0 as f32 + 4.),
                ..default()
            },
        );
        cmd.spawn((
            Node {
                width: percent(100),
                height: px(SLOT.1 as f32),
                ..default()
            },
            ImageNode::default(),
            Cloth::Slot(slot),
            ChildOf(frame),
        ));
    }
    let swatches = cmd
        .spawn((
            Node {
                column_gap: px(3),
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    for t in Tincture::ALL {
        let frame = picker(
            cmd,
            swatches,
            Action::Tincture(t),
            Node {
                padding: UiRect::all(px(2)),
                ..default()
            },
        );
        let [r, g, b] = t.srgb();
        cmd.spawn((
            Node {
                width: px(30),
                height: px(30),
                ..default()
            },
            BackgroundColor(Color::srgb_u8(r, g, b)),
            ChildOf(frame),
        ));
    }

    let orders = cmd
        .spawn((
            Node {
                margin: UiRect::top(px(6)),
                column_gap: px(8),
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    for (action, label, shortcut) in [
        (Action::Spin, "Random", "R"),
        (Action::Title, "Done", "ESC"),
    ] {
        let button = title::button(cmd, orders, action, &art.display_face, label, shortcut);
        cmd.entity(button.node).insert(Node {
            flex_grow: 1.,
            flex_basis: px(0),
            height: px(52),
            padding: UiRect::horizontal(px(14)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            border: UiRect::all(px(1)),
            ..default()
        });
        if action == Action::Spin {
            // The die rolls beside its word.
            cmd.entity(button.labels).insert(Node {
                flex_direction: FlexDirection::RowReverse,
                align_items: AlignItems::Center,
                column_gap: px(9),
                ..default()
            });
            cmd.spawn((
                ImageNode {
                    image: art.dice.clone(),
                    color: brass(),
                    ..default()
                },
                Node {
                    width: px(24),
                    height: px(24),
                    ..default()
                },
                ChildOf(button.labels),
            ));
        }
    }
}

/// A frame that lights when its choice is hovered or made.
fn picker(cmd: &mut Commands, parent: Entity, action: Action, node: Node) -> Entity {
    cmd.spawn((
        Node {
            border: UiRect::all(px(2)),
            ..node
        },
        BorderColor::all(Color::NONE),
        action,
        Picker,
        RelativeCursorPosition::default(),
        ChildOf(parent),
    ))
    .id()
}

/// Light the choices made and hovered, show as many slots as the pattern has, and repaint
/// the pictures when the flag changes or the screen is built.
pub fn refresh(
    locker: Res<Locker>,
    mut images: ResMut<Assets<Image>>,
    mut pickers: Query<
        (
            &Action,
            &RelativeCursorPosition,
            &mut BorderColor,
            &mut Node,
        ),
        With<Picker>,
    >,
    mut cloths: Query<(&Cloth, &mut ImageNode)>,
    built: Query<(), Added<Cloth>>,
) {
    let slots = locker.pattern.slots();
    for (&action, cursor, mut border, mut node) in &mut pickers {
        let colour = if locker.selected(action) {
            brass()
        } else if cursor.cursor_over() {
            ink().with_alpha(0.8)
        } else {
            Color::srgba_u8(192, 213, 202, 38)
        };
        let lit = BorderColor::all(colour);
        if *border != lit {
            *border = lit;
        }
        let display = match action {
            Action::Slot(slot) if slot >= slots => Display::None,
            _ => Display::Flex,
        };
        if node.display != display {
            node.display = display;
        }
    }
    if !locker.is_changed() && built.is_empty() {
        return;
    }
    let flag = locker.flag();
    for (&cloth, mut picture) in &mut cloths {
        let (pixels, size) = match cloth {
            Cloth::Pattern(pattern) => (locker.flag_in(pattern).paint(TILE.0, TILE.1), TILE),
            Cloth::Slot(slot) if slot < slots => {
                (flag.paint_slot(slot, REST, SLOT.0, SLOT.1), SLOT)
            }
            Cloth::Slot(_) => continue,
        };
        picture.image = images.add(image(pixels, size));
    }
}

fn image(rgba: Vec<u8>, (width, height): (u32, u32)) -> Image {
    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::linear();
    image
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Mode;

    fn app() -> App {
        let mut app = App::new();
        app.insert_resource(Campaign::new(Mode::Title))
            .init_resource::<Locker>()
            .init_resource::<Colours>()
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

    #[test]
    fn the_title_opens_the_locker_on_the_flag_she_flies_and_esc_closes_it() {
        let mut app = app();
        assert_eq!(press(&mut app, Action::Customise), None);
        assert_eq!(
            app.world().resource::<Campaign>().screen(),
            Screen::Customise
        );
        let flying = app.world().resource::<Colours>().breakwater.clone();
        assert_eq!(app.world().resource::<Locker>().flag(), flying);
        // A run cannot start from behind the locker.
        assert_eq!(press(&mut app, Action::Start), Some(Action::Start));
        assert_eq!(press(&mut app, Action::Title), None);
        assert_eq!(app.world().resource::<Campaign>().screen(), Screen::Title);
        assert_eq!(app.world().resource::<Colours>().breakwater, flying);
        // The title passes its own choices on.
        assert_eq!(press(&mut app, Action::Trial), Some(Action::Trial));
        assert_eq!(
            press(&mut app, Action::Pattern(Pattern::Quarters)),
            Some(Action::Pattern(Pattern::Quarters))
        );
    }

    #[test]
    fn a_pattern_keeps_the_colours_of_slots_it_hides_and_a_swatch_colours_the_chosen_slot() {
        let mut app = app();
        press(&mut app, Action::Customise);
        press(&mut app, Action::Pattern(Pattern::Bands(4)));
        press(&mut app, Action::Slot(3));
        press(&mut app, Action::Tincture(Tincture::Purple));
        let four = app.world().resource::<Colours>().breakwater.clone();
        assert_eq!(four.pattern(), Pattern::Bands(4));
        assert_eq!(four.colours()[3], Tincture::Purple);
        press(&mut app, Action::Pattern(Pattern::Bands(2)));
        let locker = app.world().resource::<Locker>();
        assert_eq!(locker.slot, 1, "the chosen slot stays on the cloth");
        assert_eq!(
            app.world().resource::<Colours>().breakwater.colours(),
            &four.colours()[..2]
        );
        // A slot past the pattern's last cannot be chosen.
        press(&mut app, Action::Slot(2));
        assert_eq!(app.world().resource::<Locker>().slot, 1);
        press(&mut app, Action::Pattern(Pattern::Bands(4)));
        assert_eq!(app.world().resource::<Colours>().breakwater, four);
        press(&mut app, Action::Pattern(Pattern::Saltire));
        press(&mut app, Action::Slot(2));
        press(&mut app, Action::Tincture(Tincture::Gold));
        let saltire = &app.world().resource::<Colours>().breakwater;
        assert_eq!(saltire.pattern(), Pattern::Saltire);
        assert_eq!(saltire.colours()[2], Tincture::Gold);
    }

    #[test]
    fn the_wheel_runs_down_through_flags_and_stops_on_one() {
        let mut app = app();
        app.init_resource::<Time>().add_systems(Update, turn_wheel);
        press(&mut app, Action::Customise);
        press(&mut app, Action::Spin);
        let mut seen = Vec::new();
        for _ in 0..TURNS * 2 {
            if let Some(spin) = app.world_mut().resource_mut::<Locker>().spin.as_mut() {
                spin.wait = 0.;
            }
            app.update();
            let flag = app.world().resource::<Colours>().breakwater.clone();
            if seen.last() != Some(&flag) {
                seen.push(flag);
            }
        }
        assert!(app.world().resource::<Locker>().spin.is_none());
        assert!(seen.len() >= TURNS as usize - 1, "{} flags", seen.len());
        let kept = seen.last().unwrap();
        assert!(kept.readable());
        assert_eq!(&app.world().resource::<Locker>().flag(), kept);
    }
}
