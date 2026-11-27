//! A chart plate supplies the coastline. Ink the passages, fixes and soundings for this run.
use super::*;
use crate::state::{ChartRoll, Mode};
use shipvivor_campaign::{Fleet, OFFERED, charted};
use shipvivor_encounter::EnemyClass;
use shipvivor_sim::Rating;

const WIDTH: f32 = 538.;
const HEIGHT: f32 = WIDTH * 2. / 3.;
/// The plate with its title above and its legend and key below.
const UNROLLED_HEIGHT: f32 = HEIGHT + 104. + GAUGE_KEY + OUTLOOK_ROWS;
/// Room under the key for the gauge's row, and at sea for a line on each passage on
/// offer, which has no card beside it there.
const GAUGE_KEY: f32 = 20.;
const OUTLOOK_ROWS: f32 = 3. * 19. + 6.;
const SEA_INK: Color = Color::srgb_u8(22, 75, 80);
const FADED: Color = Color::srgb_u8(87, 114, 111);
const PENCIL: Color = Color::srgb_u8(147, 55, 41);
pub(super) const PAPER: Color = Color::srgb_u8(238, 237, 211);
/// The gauge's ink on the paper, from an easy passage to a deadly one.
const DANGER: [Color; 4] = [
    Color::srgb_u8(46, 118, 70),
    Color::srgb_u8(170, 124, 22),
    Color::srgb_u8(196, 86, 28),
    Color::srgb_u8(158, 26, 34),
];
fn danger(rating: Rating) -> Color {
    DANGER[rating.level() - 1]
}
/// How `exit` looks for her as she stands: at sea, as the yard she is bound for will
/// leave her, its repairs done.
pub(super) fn outlook(run: &Campaign, exit: &shipvivor_campaign::Exit) -> shipvivor_sim::Outlook {
    if run.phase() == shipvivor_campaign::Phase::Sailing {
        let mut docked = shipvivor_campaign::Run::clone(run);
        docked.ship.dock(f32::from(shipvivor_campaign::DOCK_REPAIR));
        shipvivor_sim::outlook(&docked, exit)
    } else {
        shipvivor_sim::outlook(run, exit)
    }
}

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

/// How a fix is drawn, and where its name sits so that neighbours stay clear.
#[derive(Clone, Copy, PartialEq)]
enum Mark {
    /// Where she is now, named small in pencil off to the left, on the side her course
    /// did not come in by, clear of the passages fanning out to the right.
    Present { above: bool },
    /// A passage the yard offers. Its tab carries the name, so the fix is bare.
    Offered { open: bool },
    /// The anchorage she sailed from, named small and plain on the side her course did
    /// not leave it by.
    Origin { above: bool },
    /// A passage sailed since, a bare pencilled fix on her course.
    Sailed,
    /// Crown Deep ahead, where every route meets.
    Rendezvous,
    /// Not reached, or passed by: a bare fix.
    Bare,
}

/// Her course astern in thin, faint pencil, so the passages on offer stand out.
const WAKE: Color = Color::srgba_u8(147, 55, 41, 130);

fn fix(cmd: &mut Commands, sheet: Entity, p: Vec2, stage: Stage, mark: Mark, face: &Handle<Font>) {
    let radius = match mark {
        Mark::Present { .. } => 6.,
        Mark::Bare => 3.5,
        Mark::Origin { .. } | Mark::Sailed => 4.,
        _ => 5.,
    };
    let (border, fill) = match mark {
        Mark::Present { .. } => (PENCIL, PENCIL),
        Mark::Origin { .. } | Mark::Sailed => (WAKE, PAPER),
        Mark::Bare | Mark::Offered { open: false } => (FADED, PAPER),
        _ => (SEA_INK, PAPER),
    };
    cmd.spawn((
        Node {
            border_radius: BorderRadius::MAX,
            border: UiRect::all(px(2.)),
            ..absolute(p.x - radius, p.y - radius, 2. * radius, 2. * radius)
        },
        BorderColor::all(border),
        BackgroundColor(fill),
        ChildOf(sheet),
    ));
    // The anchorage she sailed from is named plainly. The present fix has a light paper
    // backing, so that the bare fixes and courses behind it never show through its name.
    let (size, at, shift, color, backing) = match mark {
        Mark::Bare | Mark::Offered { .. } | Mark::Sailed => return,
        Mark::Present { above } => (
            11.,
            Vec2::new(-9., if above { -17. } else { 3. }),
            -100.,
            PENCIL,
            0.8,
        ),
        Mark::Origin { above } => (
            10.,
            Vec2::new(0., if above { -18. } else { 5. }),
            -50.,
            FADED,
            0.,
        ),
        Mark::Rendezvous => (14., Vec2::new(0., -27.), -50., SEA_INK, 0.92),
    };
    let label = text(cmd, sheet, stage.mission().name, size, color);
    cmd.entity(label).insert((
        ChartName,
        TextFont::from_font_size(size).with_font(face.clone()),
        TextLayout::no_wrap(),
        Node {
            position_type: PositionType::Absolute,
            left: px(p.x + at.x),
            top: px(p.y + at.y),
            padding: UiRect::horizontal(px(4.)),
            ..default()
        },
        UiTransform::from_translation(Val2::percent(shift, 0.)),
        BackgroundColor(PAPER.with_alpha(backing)),
    ));
}

/// The number and name share one hit area, with the course drawn beneath the paper.
fn anchorage_pin(
    cmd: &mut Commands,
    sheet: Entity,
    p: Vec2,
    stage: Stage,
    number: usize,
    rating: Rating,
    art: &crate::insignia::Insignia,
) {
    let face = &art.display_face;
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
    stamp(cmd, pin, &art.fleet(stage.fleet()), SEA_INK);
    stamp(cmd, pin, &art.gauge(rating.level()), danger(rating));
}

/// Where the fixes of a run stand on the plate: the anchorages down the left, a column
/// for each later round with the seas the run charts there, its moderate passage at the
/// top and its two tough ones below, and Crown Deep at the right, where every route
/// meets.
struct Plate {
    jitter: Box<dyn Fn(u32, u32) -> f32>,
    seed: u64,
}

impl Plate {
    fn of(run: &Campaign) -> Self {
        let seed = (run.seed ^ (run.seed >> 32)) as u32;
        Self {
            jitter: Box::new(move |i, j| crate::surfaces::sample(i, j, seed) - 0.5),
            seed: run.seed,
        }
    }
    /// The seas the run charts for `round`, from 1 for the anchorages to [`PATROLS`].
    fn charted(&self, round: usize) -> [Stage; OFFERED] {
        charted(self.seed, round).expect("every patrol round has a pool")
    }
    /// Where `stage` stands across its round, numbered from the top as the chart
    /// numbers its passages. A sea the run never charted stands in the middle.
    fn slot(&self, stage: Stage) -> usize {
        charted(self.seed, stage.round())
            .and_then(|seas| seas.iter().position(|&sea| sea == stage))
            .unwrap_or(1)
    }
    fn column(round: usize) -> f32 {
        match round {
            1 => 64.,
            round if round < ROUNDS => 190. + 52. * (round - 2) as f32,
            _ => 492.,
        }
    }
    fn lane(lane: usize) -> f32 {
        80. + 90. * lane as f32
    }
    fn at(&self, stage: Stage) -> Vec2 {
        if stage == Stage::CrownDeep {
            return Vec2::new(Self::column(ROUNDS), 151. + 18. * (self.jitter)(9, 0));
        }
        let slot = self.slot(stage);
        let (round, lane) = (stage.round() as u32, slot as u32);
        Vec2::new(
            Self::column(stage.round()) + 6. * (self.jitter)(round, lane),
            Self::lane(slot) + 12. * (self.jitter)(lane + 7, round),
        )
    }
    /// An anchorage pin on the departure chart, which needs more room than a fix.
    fn pin(&self, lane: usize) -> Vec2 {
        Vec2::new(94., self.at(self.charted(1)[lane]).y)
    }
    /// The flagship that sails in `round`, from 1 for the anchorages to [`ROUNDS`]. Every
    /// passage of a round has the same one.
    fn boss(&self, round: usize) -> Option<EnemyClass> {
        match round {
            ROUNDS => Some(EnemyClass::Regent),
            _ => {
                let sea = charted(self.seed, round)?[0];
                shipvivor_sim::Sector::of(sea, shipvivor_campaign::Grade::Moderate).boss()
            }
        }
    }
}

const ROMAN: [&str; ROUNDS] = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII"];

/// The run's course in pencil, the passages the yard offers next, and the rounds beyond
/// in faint dashed lanes. The same seas drive the plotted courses and departure orders.
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
    let plate = Plate::of(run);
    let jitter = &plate.jitter;
    let departing = run.screen() == Screen::Departure;
    let face = &art.display_face;
    // Occasional depth soundings sit in the margins, clear of the names.
    for (i, (x, y)) in [
        (150., 40.),
        (120., 318.),
        (224., 330.),
        (318., 318.),
        (412., 332.),
        (128., 214.),
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
    cmd.entity(north).insert(absolute(20., 300., 15., 14.));
    // The rounds are numbered along the head of the plate, each with the stamp of the
    // flagship that sails there, so the run's bosses read at a glance.
    let present = (!departing).then(|| run.stage.round());
    for (i, numeral) in ROMAN.iter().enumerate() {
        let colour = if present == Some(i + 1) {
            PENCIL
        } else {
            FADED
        };
        let mark = text(cmd, sheet, *numeral, 11., colour);
        let x = Plate::column(i + 1) + if i == 0 && departing { 30. } else { 0. };
        cmd.entity(mark).insert((
            TextFont::from_font_size(11.).with_font(face.clone()),
            TextLayout::justify(Justify::Center),
            absolute(x - 20., 14., 40., 14.),
        ));
        if let Some(image) = plate.boss(i + 1).and_then(|boss| art.boss(boss)) {
            let ink = if colour == PENCIL { PENCIL } else { SEA_INK };
            let emblem = cmd
                .spawn((absolute(x + 12., 15., 12., 12.), ChildOf(sheet)))
                .id();
            stamp(cmd, emblem, &image, ink);
        }
    }

    // Her course so far, from the anchorage through each passage taken.
    let mut sailed: Vec<Stage> = Vec::new();
    if !departing {
        sailed.push(run.origin);
        sailed.extend(run.course.iter().copied());
        if run.stage == Stage::CrownDeep {
            sailed.push(Stage::CrownDeep);
        }
    }
    for pair in sailed.windows(2) {
        course(cmd, sheet, plate.at(pair[0]), plate.at(pair[1]), true, WAKE);
    }
    // The present fix is named on the side her course did not come in by.
    let present = |p: Vec2| Mark::Present {
        above: sailed
            .iter()
            .rev()
            .nth(1)
            .is_none_or(|&astern| plate.at(astern).y > p.y - 20.),
    };
    // The passages on offer from the present fix, open or locked by stars.
    let offered = if departing { None } else { run.exits() };
    if let Some(exits) = offered {
        for exit in exits {
            let open = run.stars() >= exit.stars;
            let colour = if open { SEA_INK } else { FADED };
            course(
                cmd,
                sheet,
                plate.at(run.stage),
                plate.at(exit.stage),
                open,
                colour,
            );
        }
    } else if !departing && run.stage.is_last_patrol() {
        let (here, crown) = (plate.at(run.stage), plate.at(Stage::CrownDeep));
        course(cmd, sheet, here, crown, true, SEA_INK);
    }
    // Every anchorage fans out to the second round's three passages.
    if departing {
        for lane in 0..OFFERED {
            for next in plate.charted(2) {
                course(cmd, sheet, plate.pin(lane), plate.at(next), true, SEA_INK);
            }
        }
    }
    // The rounds beyond run on as three dashed lanes to Crown Deep.
    let ahead = if departing { 2 } else { run.stage.round() + 1 };
    for round in ahead..PATROLS {
        for lane in 0..OFFERED {
            let (from, to) = (plate.charted(round)[lane], plate.charted(round + 1)[lane]);
            course(cmd, sheet, plate.at(from), plate.at(to), false, FADED);
        }
    }
    if ahead <= PATROLS {
        for sea in plate.charted(PATROLS) {
            let (from, to) = (plate.at(sea), plate.at(Stage::CrownDeep));
            course(cmd, sheet, from, to, false, FADED);
        }
    }

    // The fixes go over the lines.
    for round in 1..=PATROLS {
        for (lane, &sea) in plate.charted(round).iter().enumerate() {
            let p = plate.at(sea);
            if departing && round == 1 {
                let rating = shipvivor_sim::outlook::anchorage(run, sea).rating;
                anchorage_pin(cmd, sheet, plate.pin(lane), sea, lane + 1, rating, art);
                continue;
            }
            let mark = if sea == run.stage && !departing {
                present(p)
            } else if sailed.first() == Some(&sea) {
                Mark::Origin {
                    above: sailed
                        .get(1)
                        .is_none_or(|&next| plate.at(next).y > p.y - 20.),
                }
            } else if sailed.contains(&sea) {
                Mark::Sailed
            } else if let Some(exit) = offered.iter().flatten().find(|exit| exit.stage == sea) {
                Mark::Offered {
                    open: run.stars() >= exit.stars,
                }
            } else {
                Mark::Bare
            };
            fix(cmd, sheet, p, sea, mark, face);
        }
    }
    let crown = plate.at(Stage::CrownDeep);
    let crown_mark = if run.stage == Stage::CrownDeep {
        present(crown)
    } else {
        Mark::Rendezvous
    };
    fix(cmd, sheet, crown, Stage::CrownDeep, crown_mark, face);
    // The tabs go over everything, numbered as the cards beside the chart and the keys.
    for (lane, exit) in offered.into_iter().flatten().enumerate() {
        offer_tab(cmd, sheet, run, (lane, exit), plate.at(exit.stage), art);
    }
}

/// A passage on offer, on a paper tab hung off its fix: the key that sails it, its
/// name, the stars it still needs if it is locked, the most it can pay the yard in
/// spanners, and a plane if it runs under air cover. The top lane's tab hangs above
/// its fix and the others' below, clear of the courses fanning out from the present
/// fix, and a tab that would run off the plate stops at its edge instead.
fn offer_tab(
    cmd: &mut Commands,
    sheet: Entity,
    run: &Campaign,
    (lane, exit): (usize, shipvivor_campaign::Exit),
    p: Vec2,
    art: &crate::insignia::Insignia,
) {
    let sea = exit.stage;
    let open = run.stars() >= exit.stars;
    // A locked passage is greyed like its card beside the chart, air station and all.
    let ink = if open { SEA_INK } else { FADED };
    // A strip from the fix to the plate's edge holds the tab. Its auto margin keeps the
    // tab at the fix while there is room, and gives way when there is none, so that
    // the tab ends at the edge.
    let strip = cmd
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(p.x - 12.),
                right: px(6.),
                top: if lane == 0 { auto() } else { px(p.y + 13.) },
                bottom: if lane == 0 {
                    px(HEIGHT - p.y + 13.)
                } else {
                    auto()
                },
                justify_content: JustifyContent::FlexEnd,
                ..default()
            },
            ChildOf(sheet),
        ))
        .id();
    let tab = cmd
        .spawn((
            Node {
                margin: UiRect::right(auto()),
                flex_shrink: 0.,
                align_items: AlignItems::Center,
                column_gap: px(5.),
                padding: UiRect::new(px(3.), px(7.), px(2.), px(2.)),
                border: UiRect::all(px(1.)),
                border_radius: BorderRadius::all(px(12.)),
                ..default()
            },
            BackgroundColor(PAPER.with_alpha(0.92)),
            BorderColor::all(ink),
            ChildOf(strip),
        ))
        .id();
    if run.screen() == Screen::Chart && open {
        cmd.entity(tab).insert((
            ChartName,
            Action::Depart(sea),
            RelativeCursorPosition::default(),
        ));
    }
    let badge = cmd
        .spawn((
            Node {
                width: px(19.),
                height: px(19.),
                border_radius: BorderRadius::MAX,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(ink),
            ChildOf(tab),
        ))
        .id();
    text(cmd, badge, (lane + 1).to_string(), 13., PAPER);
    let name = text(cmd, tab, sea.mission().name, 13., ink);
    cmd.entity(name).insert((
        TextFont::from_font_size(13.).with_font(art.display_face.clone()),
        TextLayout::no_wrap(),
    ));
    // What holds it: flotillas, mixed squadrons or a battle line.
    stamp(cmd, tab, &art.fleet(sea.fleet()), ink);
    let face = &art.display_face;
    if !open {
        stamp(cmd, tab, &art.lock, ink);
        figure(cmd, tab, exit.stars.into(), &art.star, ink, face);
    }
    // Every star at the grade's rate and every yard, before any cargo.
    let most = shipvivor_sim::Sector::of(sea, exit.grade).most_points();
    figure(cmd, tab, most, &art.order, ink, face);
    if exit.under_air_cover() {
        stamp(cmd, tab, &art.floatplane, if open { PENCIL } else { FADED });
    }
    // How hard it looks for her as she stands.
    let rating = outlook(run, &exit).rating;
    let gauge = if open { danger(rating) } else { FADED };
    stamp(cmd, tab, &art.gauge(rating.level()), gauge);
}

/// One of the 12-pixel stamps, inked onto a tab.
fn stamp(cmd: &mut Commands, parent: Entity, image: &Handle<Image>, ink: Color) {
    cmd.spawn((
        ImageNode {
            image: image.clone(),
            color: ink,
            ..default()
        },
        Node {
            width: px(12.),
            height: px(12.),
            ..default()
        },
        ChildOf(parent),
    ));
}

/// A count and the stamp it counts, set close together.
fn figure(
    cmd: &mut Commands,
    parent: Entity,
    count: u16,
    image: &Handle<Image>,
    ink: Color,
    face: &Handle<Font>,
) {
    let row = cmd
        .spawn((
            Node {
                align_items: AlignItems::Center,
                column_gap: px(2.),
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    let number = text(cmd, row, count.to_string(), 12., ink);
    cmd.entity(number)
        .insert(TextFont::from_font_size(12.).with_font(face.clone()));
    stamp(cmd, row, image, ink);
}

/// The plate's key to its stamps: what holds each passage and the guns that tell
/// against it, the flagships over the round numbers, and the gauge of how hard each
/// passage looks for her.
fn key(
    cmd: &mut Commands,
    parent: Entity,
    art: &crate::insignia::Insignia,
    ink: Color,
    inset: f32,
) {
    let fleets = [
        (Fleet::Flotillas, "Flotillas, quick guns"),
        (Fleet::Mixed, "Mixed"),
        (Fleet::BattleLine, "Battle line, big guns"),
    ]
    .map(|(fleet, label)| (art.fleet(fleet), label));
    let flagships = [
        EnemyClass::Commodore,
        EnemyClass::Parliament,
        EnemyClass::Regent,
    ]
    .map(|boss| (art.boss(boss), crate::mission_hud::ship_class(boss)));
    let flagships = flagships
        .into_iter()
        .filter_map(|(image, label)| Some((image?, label)));
    for entries in [fleets.to_vec(), flagships.collect()] {
        let row = cmd
            .spawn((
                Node {
                    column_gap: px(14.),
                    align_items: AlignItems::Center,
                    margin: UiRect::left(px(inset)),
                    ..default()
                },
                ChildOf(parent),
            ))
            .id();
        for (image, label) in entries {
            let entry = cmd
                .spawn((
                    Node {
                        column_gap: px(4.),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    ChildOf(row),
                ))
                .id();
            stamp(cmd, entry, &image, ink);
            text(cmd, entry, label, 13., ink);
        }
    }
    // The gauge in its own inks, then what it measures.
    let row = cmd
        .spawn((
            Node {
                column_gap: px(14.),
                align_items: AlignItems::Center,
                margin: UiRect::left(px(inset)),
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    for rating in Rating::ALL {
        let entry = cmd
            .spawn((
                Node {
                    column_gap: px(4.),
                    align_items: AlignItems::Center,
                    ..default()
                },
                ChildOf(row),
            ))
            .id();
        stamp(cmd, entry, &art.gauge(rating.level()), danger(rating));
        text(cmd, entry, rating.name(), 13., ink);
    }
    text(cmd, row, "for your ship as she stands", 13., ink);
}

/// At sea the chart has no cards beside it, so each passage on offer gets a line under
/// the key: how hard it looks for her as she stands, and what would help her most.
fn outlooks(cmd: &mut Commands, parent: Entity, run: &Campaign) {
    let Some(exits) = run.exits() else {
        return;
    };
    let column = cmd
        .spawn((
            Node {
                margin: UiRect::new(px(12.), px(12.), px(6.), px(0.)),
                ..column(2.)
            },
            ChildOf(parent),
        ))
        .id();
    for (lane, exit) in exits.iter().enumerate() {
        let outlook = outlook(run, exit);
        let row = cmd
            .spawn((
                Node {
                    column_gap: px(8.),
                    align_items: AlignItems::Center,
                    ..default()
                },
                ChildOf(column),
            ))
            .id();
        text(
            cmd,
            row,
            format!("{}  {}", lane + 1, exit.stage.mission().name),
            13.,
            SEA_INK,
        );
        text(
            cmd,
            row,
            super::verdict(&outlook),
            13.,
            danger(outlook.rating),
        );
    }
}

fn legend(run: &Campaign) -> &'static str {
    if run.screen() == Screen::Departure {
        "Click an anchorage to sail.   Each yard charts a moderate and two tough passages."
    } else {
        "Red  Your course      Solid  Open      Dashed  Locked or ahead"
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
    let legend = text(cmd, paper, legend(run), 13., SEA_INK);
    cmd.entity(legend).insert(Node {
        margin: UiRect::axes(px(12), px(4)),
        ..default()
    });
    key(cmd, paper, art, SEA_INK, 12.);
    outlooks(cmd, paper, run);
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
    key(cmd, plate, art, pale(), 0.);
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
