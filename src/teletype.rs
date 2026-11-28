//! The radio room's teleprinter, under the chart. The fleet sends the ship her orders for
//! each sea and word of what is happening in it: a flagship raising steam or sailing, a
//! battle line forming, a base destroyed. Each message prints as a 1930s fleet broadcast
//! does: a precedence and date-time group, the text as it came down the wire in groups
//! of five letters, and under it the decoding officer's plain English. Paper rolls up
//! out of the machine, over the top of its roll and out of sight. The machine clatters
//! on a steady beat as it prints, and rings its bell first for anything urgent.
//!
//! Anything that wants to tell the captain something, such as a side task offered later
//! on, sends it with [`Teletype::send`].
use crate::{
    mission_hud::{letter, ship_class},
    sound::Soundboard,
    state::{Campaign, Embarked, Encounter, Mode, PlayerHelm, Screen},
};
use bevy::prelude::*;
use shipvivor_acoustics::Sound;
use shipvivor_campaign::{Fleet, Stage};
use shipvivor_encounter::{Doctrine, EnemyClass, Sortie};
use std::collections::VecDeque;

/// Characters to a line of the paper, and letters to a group of the coded text.
const COLUMNS: usize = 38;
const GROUP: usize = 5;
/// Seconds to print a character: a fast teleprinter, at 130 words a minute. A carriage
/// return and line feed takes this many characters' time, and the bell this many
/// seconds a stroke.
const BEAT: f32 = 0.075;
const RETURN: f32 = 2.5;
const BELL: f32 = 0.45;
/// How loud a space is against a character: the typebar stays in its basket and only
/// the selector and escapement are heard.
const SPACE: f32 = 0.35;
/// A blank line is only a line feed, with no carriage to run back.
const FEED_ONLY: f32 = 0.45;
/// Lines kept on the roll, more than the paper ever shows.
const KEPT: usize = 40;

/// The type: FiraMono, the default font, whose characters are 0.6 of its size across.
const FONT: f32 = 12.;
const ADVANCE: f32 = 0.6 * FONT;
const LINE: f32 = 14.;
/// The paper: its width, its left margin, the tallest it shows, the radius of the roll
/// it curls over at the top, and the space between the line being printed and the
/// platen.
const PAPER_WIDTH: f32 = COLUMNS as f32 * ADVANCE + 2. * MARGIN;
const MARGIN: f32 = 10.;
const PAPER_HEIGHT: f32 = 252.;
const CURL: f32 = 26.;
const PLATEN_GAP: f32 = 8.;
/// Lines of paper laid out: enough to reach over the roll.
const SHOWN: usize = 22;
/// Seconds the paper takes to rise a line.
const FEED: f32 = 0.09;
/// Where the machine stands: under the chart on the right, and clear of the helm orders.
const TOP: f32 = 404.;
const BOTTOM: f32 = 196.;
const RIGHT: f32 = 18.;
const MACHINE_HEIGHT: f32 = 28.;
/// The window the machine stands at its own size in, and grows with past it: two and a
/// half times on a 4K screen.
const READ_WIDTH: f32 = 1536.;
const READ_HEIGHT: f32 = 840.;

/// How urgently a message goes out, which sets its place in the queue and its bells.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Precedence {
    Flash,
    Immediate,
    Priority,
    Routine,
}
impl Precedence {
    fn word(self) -> &'static str {
        match self {
            Self::Flash => "FLASH",
            Self::Immediate => "IMMEDIATE",
            Self::Priority => "PRIORITY",
            Self::Routine => "ROUTINE",
        }
    }
    fn bells(self) -> u8 {
        match self {
            Self::Flash => 4,
            Self::Immediate => 2,
            Self::Priority => 1,
            Self::Routine => 0,
        }
    }
}

/// A message to print: the text as sent, in the navy's short forms, which prints in
/// groups of five, and the same in plain English, which prints under it.
#[derive(Clone, Debug)]
pub struct Message {
    pub precedence: Precedence,
    pub code: String,
    pub plain: String,
}
impl Message {
    pub fn new(precedence: Precedence, code: impl Into<String>, plain: impl Into<String>) -> Self {
        Self {
            precedence,
            code: code.into(),
            plain: plain.into(),
        }
    }

    /// The lines it prints as, sent at `dtg`: the heading, the groups, the plain text
    /// and a blank line after it.
    fn lines(&self, dtg: &str) -> Vec<(String, Ink)> {
        let mut lines = vec![(
            format!("{} {dtg} CTF 88", self.precedence.word()),
            Ink::Heading,
        )];
        lines.extend(groups(&self.code).into_iter().map(|l| (l, Ink::Code)));
        lines.extend(
            wrap(&self.plain.to_uppercase())
                .into_iter()
                .map(|l| (l, Ink::Plain)),
        );
        lines.push((String::new(), Ink::Code));
        lines
    }
}

/// The letters and figures of `code` run together and cut into groups of five, the last
/// filled out with X, as many groups to a line as fit.
fn groups(code: &str) -> Vec<String> {
    let mut letters: Vec<char> = code
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect();
    while !letters.len().is_multiple_of(GROUP) {
        letters.push('X');
    }
    let groups: Vec<String> = letters.chunks(GROUP).map(|g| g.iter().collect()).collect();
    let per_line = (COLUMNS + 1) / (GROUP + 1);
    groups.chunks(per_line).map(|line| line.join(" ")).collect()
}

/// Words laid into lines no wider than the paper.
fn wrap(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(line) if line.len() + 1 + word.len() <= COLUMNS => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.chars().take(COLUMNS).collect()),
        }
    }
    lines
}

/// What a line is printed as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ink {
    Heading,
    Code,
    Plain,
}
impl Ink {
    fn color(self) -> Color {
        match self {
            Self::Heading => Color::srgb_u8(112, 104, 96),
            Self::Code => Color::srgb_u8(30, 28, 32),
            Self::Plain => Color::srgb_u8(28, 54, 122),
        }
    }
}

/// The message being printed: its lines, and the bells still to ring before it.
struct Job {
    lines: Vec<(String, Ink)>,
    line: usize,
    column: usize,
    bells: u8,
}

/// What the battle looked like at the last look, to send word when it changes.
#[derive(Default)]
struct Watch {
    /// A new voyage is under way and its orders have not gone out.
    fresh: bool,
    seen: f32,
    wrecked: Vec<bool>,
    steam: bool,
    sortie: Option<Sortie>,
    boss: Option<EnemyClass>,
    /// Squadrons already reported in line of battle, by their first ship.
    lines: Vec<u64>,
    surprise: bool,
}

#[derive(Resource)]
pub struct Teletype {
    queue: VecDeque<Message>,
    job: Option<Job>,
    /// Seconds before the machine does its next thing.
    wait: f32,
    /// The roll, oldest first. The last line is the one at the platen.
    paper: VecDeque<(String, Ink)>,
    /// Seconds since the paper last fed a line.
    fed: f32,
    serial: u64,
    watch: Watch,
}
impl Default for Teletype {
    fn default() -> Self {
        Self {
            queue: VecDeque::new(),
            job: None,
            wait: 0.,
            paper: VecDeque::from([(String::new(), Ink::Code)]),
            fed: 1.,
            serial: 0,
            watch: Watch::default(),
        }
    }
}
impl Teletype {
    /// Queue a message to print, behind any of the same or higher precedence.
    pub fn send(&mut self, message: Message) {
        let at = (self.queue.iter())
            .position(|queued| queued.precedence > message.precedence)
            .unwrap_or(self.queue.len());
        self.queue.insert(at, message);
    }

    /// Whether the machine has anything to print.
    pub fn busy(&self) -> bool {
        self.job.is_some() || !self.queue.is_empty()
    }

    fn feed(&mut self) {
        self.paper.push_back((String::new(), Ink::Code));
        while self.paper.len() > KEPT {
            self.paper.pop_front();
        }
        self.fed = 0.;
    }

    /// Run the machine for `dt` seconds, sending each sound it makes to `board`.
    fn run(&mut self, mut dt: f32, dtg: impl Fn() -> String, board: &Soundboard) {
        self.fed += dt;
        loop {
            if self.wait > dt {
                self.wait -= dt;
                return;
            }
            dt -= self.wait;
            self.wait = 0.;
            let Some(job) = self.job.as_mut() else {
                let Some(message) = self.queue.pop_front() else {
                    return;
                };
                self.job = Some(Job {
                    lines: message.lines(&dtg()),
                    line: 0,
                    column: 0,
                    bells: message.precedence.bells(),
                });
                continue;
            };
            self.serial += 1;
            if job.bells > 0 {
                job.bells -= 1;
                board.notice(Sound::PrinterBell, 1., self.serial);
                self.wait = if job.bells > 0 { BELL } else { 2. * BELL };
                continue;
            }
            let (text, ink) = &job.lines[job.line];
            if let Some(c) = text.chars().nth(job.column) {
                let at_platen = self.paper.back_mut().expect("the roll always has a line");
                at_platen.0.push(c);
                at_platen.1 = *ink;
                job.column += 1;
                let level = if c == ' ' { SPACE } else { 1. };
                board.notice(Sound::Typebar, level, self.serial);
                self.wait = BEAT;
                continue;
            }
            let level = if job.column == 0 { FEED_ONLY } else { 1. };
            board.notice(Sound::Carriage, level, self.serial);
            job.line += 1;
            job.column = 0;
            if job.line == job.lines.len() {
                self.job = None;
            }
            self.feed();
            self.wait = RETURN * BEAT;
        }
    }
}

/// A new voyage: whatever was coming over is cut off, and the next look sends her
/// orders. The roll carries on from the last sea.
pub fn embark(_: On<Embarked>, mut tty: ResMut<Teletype>) {
    tty.queue.clear();
    if tty.job.take().is_some() || tty.paper.back().is_some_and(|line| !line.0.is_empty()) {
        tty.feed();
    }
    tty.wait = 0.;
    tty.watch = Watch {
        fresh: true,
        ..default()
    };
}

/// The machine is on the bulkhead while she is at sea on patrol.
fn shown(run: &Campaign) -> bool {
    run.mode == Mode::Patrol && matches!(run.screen(), Screen::Sailing | Screen::Secured)
}

/// The day of the month, as the date-time group gives it, of the sea she is in.
fn dtg(run: &Campaign, battle: &Encounter) -> String {
    let minutes = 6 * 60 + 12 + (battle.at_sea() / 60.) as usize;
    format!(
        "{:02}{:02}{:02}Z FEB",
        9 + run.stage.round(),
        minutes / 60 % 24,
        minutes % 60
    )
}

/// Whole compass degrees and yards from her to `at`, as a signal gives them.
fn fix(helm: &PlayerHelm, at: Vec2) -> (String, String) {
    let bearing = shipvivor_compass::heading(at - helm.position)
        .to_degrees()
        .round() as i32;
    let yards = (at.distance(helm.position) * 1.0936 / 50.).round() as i32 * 50;
    (format!("{:03}", bearing.rem_euclid(360)), yards.to_string())
}

/// A boss's name as it is sent.
fn call_sign(class: EnemyClass) -> String {
    ship_class(class).to_uppercase()
}

fn orders(run: &Campaign, battle: &Encounter) -> Message {
    if run.stage == Stage::CrownDeep {
        return Message::new(
            Precedence::Priority,
            "INTERCEPT IRON REGENT CROWN DEEP SINK HER SCREEN AHEAD OF HER",
            "Intercept the Iron Regent at Crown Deep and sink her. Her screen sails ahead of her.",
        );
    }
    let sea = run.stage.mission().name;
    let bases = battle.ports().len();
    let (fleet, seen) = match run.stage.fleet() {
        Fleet::Flotillas => ("LIGHT FLOTILLAS", "swarms of light hulls"),
        Fleet::Mixed => ("MIXED SQDNS", "light and heavy ships"),
        Fleet::BattleLine => ("BATTLE LINE", "a battle line in heavy plate"),
    };
    let mut code = format!(
        "PROCEED {} DESTROY {bases} BASES {fleet} REPORTED",
        sea.trim_start_matches("The ")
    );
    let mut plain = format!("Proceed to {sea}. Destroy its {bases} bases. Scouts report {seen}.");
    if let Some(boss) = battle.boss() {
        code += &format!(" {} FITTING OUT", call_sign(boss));
        plain += &format!(" The {} is fitting out there.", ship_class(boss));
    }
    if run.under_air_cover() {
        code += " ENY AIR COVER";
        plain += " Expect enemy aircraft.";
    }
    Message::new(Precedence::Priority, code, plain)
}

fn baseline(watch: &mut Watch, battle: &Encounter) {
    watch.seen = battle.at_sea();
    watch.wrecked = battle.ports().iter().map(|p| p.wrecked()).collect();
    watch.steam = battle.raising_steam().is_some();
    watch.sortie = battle.flagship();
    watch.boss = battle.boss_ship().map(|b| b.class);
    watch.lines = (battle.squadrons())
        .filter(|(_, deployed)| *deployed)
        .filter_map(|(ships, _)| ships.first().copied())
        .collect();
    watch.surprise = surprise_roused(battle).is_some();
}

/// The base that moors the fleet the scouts missed, once it is roused.
fn surprise_roused(battle: &Encounter) -> Option<(usize, Doctrine)> {
    let surprise = battle.surprise()?;
    let base =
        (0..battle.ports().len()).find(|&i| battle.surprised(i) && battle.ports()[i].roused())?;
    Some((base, surprise.doctrine))
}

/// Watch the battle and send word of what changes in it.
pub fn listen(
    run: Res<Campaign>,
    battle: Res<Encounter>,
    helm: Res<PlayerHelm>,
    mut tty: ResMut<Teletype>,
) {
    if !shown(&run) {
        return;
    }
    let mut news = Vec::new();
    let watch = &mut tty.watch;
    if watch.fresh || battle.at_sea() < watch.seen {
        // A new voyage gets its orders. A battle wound back is taken as it now stands.
        if std::mem::take(&mut watch.fresh) {
            news.push(orders(&run, &battle));
        }
        baseline(watch, &battle);
        for message in news {
            tty.send(message);
        }
        return;
    }
    watch.seen = battle.at_sea();

    // The flagship: smoke over her yard, then at sea, and at last sunk.
    let steam = battle.raising_steam().is_some();
    if steam
        && !watch.steam
        && let Some(boss) = battle.boss()
    {
        news.push(Message::new(
            Precedence::Immediate,
            format!("SMOKE OVER ENY YARD {} RAISING STEAM", call_sign(boss)),
            format!(
                "Smoke over the enemy yard. The {} is raising steam and will sail within the minute.",
                ship_class(boss)
            ),
        ));
    }
    watch.steam = steam;
    let sortie = battle.flagship();
    if sortie != watch.sortie
        && let Some(boss) = battle.boss_ship()
    {
        let (bearing, yards) = fix(&helm, boss.body.pos);
        let name = ship_class(boss.class);
        match sortie {
            Some(Sortie::Sailed { port, .. }) => news.push(Message::new(
                Precedence::Flash,
                format!(
                    "{} SORTIED BASE {} BRG {bearing} RNG {yards} YDS",
                    call_sign(boss.class),
                    letter(port)
                ),
                format!(
                    "The {name} has put to sea from base {}. Bearing {bearing}, range {yards} yards.",
                    letter(port)
                ),
            )),
            Some(Sortie::Sighted { .. }) => news.push(Message::new(
                Precedence::Flash,
                format!(
                    "{} IN SIGHT BRG {bearing} RNG {yards} YDS",
                    call_sign(boss.class)
                ),
                format!(
                    "The {name} is coming over the horizon. Bearing {bearing}, range {yards} yards."
                ),
            )),
            _ => {}
        }
    }
    watch.sortie = sortie;
    let boss = battle.boss_ship().map(|b| b.class);
    if let (Some(sunk), None) = (watch.boss, boss) {
        news.push(Message::new(
            Precedence::Immediate,
            format!("{} SUNK BZ", call_sign(sunk)),
            format!("The {} is sunk. Well done.", ship_class(sunk)),
        ));
    }
    watch.boss = boss;

    // A battle line forming: the big targets of the sea.
    for (ships, deployed) in battle.squadrons() {
        let Some(&first) = ships.first() else {
            continue;
        };
        if !deployed || watch.lines.contains(&first) {
            continue;
        }
        watch.lines.push(first);
        let classes: Vec<&shipvivor_encounter::Raider> = (battle.raiders().iter())
            .filter(|r| ships.contains(&r.body.id) && !r.class.is_boss())
            .collect();
        let count = |class| classes.iter().filter(|r| r.class == class).count();
        let (battleships, cruisers) = (count(EnemyClass::Battleship), count(EnemyClass::Cruiser));
        if battleships == 0 && cruisers < 2 {
            continue;
        }
        let middle = classes.iter().map(|r| r.body.pos).sum::<Vec2>() / classes.len() as f32;
        let (bearing, yards) = fix(&helm, middle);
        let mut heavy = Vec::new();
        let plural =
            |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        if battleships > 0 {
            heavy.push(plural(battleships, "battleship", "battleships"));
        }
        if cruisers > 0 {
            heavy.push(plural(cruisers, "cruiser", "cruisers"));
        }
        news.push(Message::new(
            Precedence::Immediate,
            format!(
                "ENY BATTLE LINE {battleships} BB {cruisers} CA FORMING BRG {bearing} RNG {yards} YDS"
            ),
            format!(
                "Enemy battle line forming: {}. Bearing {bearing}, range {yards} yards.",
                heavy.join(" and ")
            ),
        ));
    }

    // The fleet the scouts missed, once its base is roused.
    if !watch.surprise
        && let Some((base, doctrine)) = surprise_roused(&battle)
    {
        watch.surprise = true;
        let (code, plain) = match doctrine {
            Doctrine::BattleLine => ("BATTLE LINE", "a battle line"),
            Doctrine::Flotillas => ("FLOTILLAS", "flotillas"),
            Doctrine::Mixed => ("SQDN", "a squadron"),
        };
        news.push(Message::new(
            Precedence::Immediate,
            format!("CORRECTION SCOUTS MISSED {code} BASE {}", letter(base)),
            format!(
                "Correction to the scouts' report: {plain} lies at base {}.",
                letter(base)
            ),
        ));
    }

    // Bases destroyed.
    let wrecked: Vec<bool> = battle.ports().iter().map(|p| p.wrecked()).collect();
    let standing = wrecked.iter().filter(|w| !**w).count();
    for (base, (&now, &was)) in wrecked.iter().zip(&watch.wrecked).enumerate() {
        if !now || was {
            continue;
        }
        news.push(if standing == 0 {
            Message::new(
                Precedence::Routine,
                format!("BASE {} DESTROYED ALL BASES DESTROYED BZ", letter(base)),
                format!(
                    "Base {} destroyed. Every base in the sector is destroyed. Well done.",
                    letter(base)
                ),
            )
        } else {
            Message::new(
                Precedence::Routine,
                format!("BASE {} DESTROYED {standing} REMAIN", letter(base)),
                format!("Base {} destroyed. {standing} remain.", letter(base)),
            )
        });
    }
    watch.wrecked = wrecked;

    for message in news {
        tty.send(message);
    }
}

/// Print, while the machine is on the bulkhead and the battle runs.
pub fn print(
    time: Res<Time>,
    run: Res<Campaign>,
    battle: Res<Encounter>,
    board: Res<Soundboard>,
    mut tty: ResMut<Teletype>,
) {
    if !shown(&run) {
        return;
    }
    tty.run(time.delta_secs(), || dtg(&run, &battle), &board);
}

/// The machine, built at this many times its size.
#[derive(Component)]
pub struct Machine(f32);
#[derive(Component)]
pub struct Paper;
#[derive(Component)]
pub struct PaperLine(usize);
#[derive(Component)]
pub struct TypeHead;
#[derive(Component)]
pub struct Lamp;

fn paper() -> Color {
    Color::srgb_u8(241, 234, 212)
}

/// Build the machine at `k` times its size.
fn build(cmd: &mut Commands, k: f32) {
    let s = |v: f32| px(v * k);
    let root = cmd
        .spawn((
            Machine(k),
            Node {
                position_type: PositionType::Absolute,
                right: px(RIGHT),
                top: px(TOP),
                bottom: px(BOTTOM),
                width: s(PAPER_WIDTH + 20.),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                ..default()
            },
            Visibility::Hidden,
            GlobalZIndex(1),
        ))
        .id();
    let sheet = cmd
        .spawn((
            Paper,
            Node {
                width: s(PAPER_WIDTH),
                height: s(PAPER_HEIGHT),
                min_height: s(0.),
                flex_shrink: 1.,
                overflow: Overflow::clip(),
                border_radius: BorderRadius::top(s(CURL * 0.5)),
                ..default()
            },
            BackgroundColor(paper()),
            BoxShadow::new(Color::BLACK.with_alpha(0.35), s(0.), s(2.), s(0.), s(6.)),
            ChildOf(root),
        ))
        .id();
    for n in 0..SHOWN {
        cmd.spawn((
            PaperLine(n),
            Text::new(""),
            TextFont::from_font_size(FONT * k),
            TextColor(Ink::Code.color()),
            TextLayout::no_wrap(),
            Node {
                position_type: PositionType::Absolute,
                left: s(MARGIN),
                bottom: s(0.),
                height: s(LINE),
                ..default()
            },
            UiTransform::default(),
            Visibility::Hidden,
            ChildOf(sheet),
        ));
    }
    // The ribbon guide riding along the line being printed.
    cmd.spawn((
        TypeHead,
        Node {
            position_type: PositionType::Absolute,
            left: s(MARGIN),
            bottom: s(PLATEN_GAP - 1.),
            width: s(ADVANCE + 2.),
            height: s(LINE + 1.),
            border: UiRect::top(s(2.)),
            border_radius: BorderRadius::top(s(2.)),
            ..default()
        },
        BackgroundColor(Color::srgba_u8(52, 58, 60, 224)),
        BorderColor::all(Color::srgb_u8(150, 40, 36)),
        ChildOf(sheet),
    ));
    // The paper going over the roll: shaded as it turns away, with the light along it.
    cmd.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: s(0.),
            right: s(0.),
            top: s(0.),
            height: s(CURL),
            ..default()
        },
        BackgroundGradient::from(LinearGradient::to_bottom(vec![
            ColorStop::percent(Color::srgba_u8(28, 22, 16, 200), 0),
            ColorStop::percent(Color::srgba_u8(28, 22, 16, 96), 16),
            ColorStop::percent(Color::srgba_u8(255, 253, 244, 130), 38),
            ColorStop::percent(Color::srgba_u8(255, 253, 244, 40), 60),
            ColorStop::percent(Color::srgba_u8(120, 104, 80, 26), 82),
            ColorStop::percent(Color::srgba_u8(120, 104, 80, 0), 100),
        ])),
        ChildOf(sheet),
    ));
    // The machine: its cover, the platen the paper comes up from behind, and a lamp
    // that is lit while a message comes over.
    let cover = cmd
        .spawn((
            Node {
                width: s(PAPER_WIDTH + 20.),
                height: s(MACHINE_HEIGHT),
                flex_shrink: 0.,
                padding: UiRect::new(s(12.), s(12.), s(11.), s(0.)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                border: UiRect::top(s(1.)),
                border_radius: BorderRadius::new(s(5.), s(5.), s(3.), s(3.)),
                ..default()
            },
            BackgroundGradient::from(LinearGradient::to_bottom(vec![
                ColorStop::auto(Color::srgb_u8(104, 114, 116)),
                ColorStop::auto(Color::srgb_u8(62, 70, 73)),
            ])),
            BorderColor::all(Color::srgb_u8(150, 160, 160)),
            BoxShadow::new(Color::BLACK.with_alpha(0.4), s(0.), s(3.), s(0.), s(6.)),
            ChildOf(root),
        ))
        .id();
    cmd.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: s(6.),
            right: s(6.),
            top: s(-3.),
            height: s(8.),
            border_radius: BorderRadius::all(s(4.)),
            ..default()
        },
        BackgroundGradient::from(LinearGradient::to_bottom(vec![
            ColorStop::auto(Color::srgb_u8(58, 56, 54)),
            ColorStop::auto(Color::srgb_u8(14, 14, 15)),
        ])),
        ChildOf(cover),
    ));
    cmd.spawn((
        Text::new("FLEET BROADCAST"),
        TextFont::from_font_size(9. * k),
        TextColor(Color::srgb_u8(214, 190, 128)),
        ChildOf(cover),
    ));
    cmd.spawn((
        Lamp,
        Node {
            width: s(8.),
            height: s(8.),
            border: UiRect::all(s(1.)),
            border_radius: BorderRadius::all(s(4.)),
            ..default()
        },
        BackgroundColor(Color::srgb_u8(70, 48, 20)),
        BorderColor::all(Color::srgb_u8(30, 30, 30)),
        ChildOf(cover),
    ));
}

/// Lay the roll out on the paper: lines rising off the platen as it feeds, and over
/// the top of the roll foreshortened, darkening as they turn away, until they go out of
/// sight behind it.
#[allow(clippy::type_complexity)]
pub fn draw(
    mut cmd: Commands,
    (run, tty, window, ui): (Res<Campaign>, Res<Teletype>, Single<&Window>, Res<UiScale>),
    mut machine: Query<(Entity, &Machine, &mut Visibility)>,
    mut sheet: Query<
        (&ComputedNode, &mut Node),
        (With<Paper>, Without<PaperLine>, Without<TypeHead>),
    >,
    mut lines: Query<
        (
            &PaperLine,
            &mut Text,
            &mut TextColor,
            &mut Node,
            &mut UiTransform,
            &mut Visibility,
        ),
        Without<Machine>,
    >,
    mut head: Query<&mut Node, (With<TypeHead>, Without<PaperLine>, Without<Paper>)>,
    mut lamp: Query<&mut BackgroundColor, With<Lamp>>,
) {
    // The bridge's lettering stops growing at full size; the machine keeps on growing
    // with the window, so its print can be read on a large screen.
    let k = (window.width() / READ_WIDTH)
        .min(window.height() / READ_HEIGHT)
        .max(ui.0)
        / ui.0;
    let show = shown(&run);
    let mut built = false;
    for (machine, &Machine(size), mut vis) in &mut machine {
        if (size - k).abs() > 0.01 {
            cmd.entity(machine).despawn();
            continue;
        }
        built = true;
        vis.set_if_neq(if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    if !built {
        build(&mut cmd, k);
        return;
    }
    if !show {
        return;
    }
    let Ok((sheet, mut sheet_node)) = sheet.single_mut() else {
        return;
    };
    // The paper rises a line over FEED seconds after each line feed, quick off the mark.
    let feed = (tty.fed / FEED).min(1.);
    let (line_h, curl, gap) = (LINE * k, CURL * k, PLATEN_GAP * k);
    let lift = (1. - (1. - feed) * (1. - feed) - 1.) * line_h;
    // It rolls out of the machine as it prints, until it reaches over the roll, and
    // shows no more than there is room for.
    let rolled = gap + tty.paper.len() as f32 * line_h + lift + curl;
    sheet_node.height = px(rolled.clamp(gap + 2. * line_h + curl, PAPER_HEIGHT * k));
    let height = sheet.size().y * sheet.inverse_scale_factor();
    let tangent = height - curl;
    for (PaperLine(n), mut text, mut color, mut node, mut transform, mut vis) in &mut lines {
        let Some((line, ink)) = tty
            .paper
            .len()
            .checked_sub(1 + n)
            .and_then(|i| tty.paper.get(i))
        else {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        };
        let flat = gap + *n as f32 * line_h + lift + 0.5 * line_h;
        // Over the roll a line a distance `past` round it stands at the roll's sine of
        // the angle it has turned, and is seen edge-on as it turns.
        let (centre, turned) = if flat > tangent {
            let angle = (flat - tangent) / curl;
            (tangent + curl * angle.min(PI_2).sin(), angle)
        } else {
            (flat, 0.)
        };
        let facing = turned.min(PI_2).cos();
        // A line turned past the top of the roll is behind it. The paper's clip does not
        // reach a squashed line, so one that would stand over its edge is not shown.
        if centre + 0.5 * line_h * facing > height - 1. {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        }
        vis.set_if_neq(Visibility::Inherited);
        if text.0 != *line {
            text.0.clone_from(line);
        }
        color.0 = ink.color().with_alpha(facing * facing);
        node.bottom = px(centre - 0.5 * line_h);
        transform.scale = Vec2::new(1., facing.max(0.05));
    }
    let column = tty.paper.back().map_or(0, |line| line.0.chars().count());
    for mut node in &mut head {
        node.left = px((MARGIN + column as f32 * ADVANCE - 1.) * k);
        node.bottom = px((PLATEN_GAP - 1.) * k + lift);
    }
    for mut bg in &mut lamp {
        bg.0 = if tty.busy() {
            Color::srgb_u8(255, 176, 64)
        } else {
            Color::srgb_u8(70, 48, 20)
        };
    }
}
const PI_2: f32 = std::f32::consts::FRAC_PI_2;

pub struct TeletypePlugin;
impl Plugin for TeletypePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Teletype>()
            .add_observer(embark)
            .add_systems(Update, (listen, print, draw).chain());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_runs_together_in_groups_of_five_filled_out_with_x() {
        assert_eq!(groups("ADVISE SHIP AHEAD"), ["ADVIS ESHIP AHEAD"]);
        assert_eq!(groups("BRG 040, RNG 1200"), ["BRG04 0RNG1 200XX"]);
        let long = groups(&"ENEMY BATTLE LINE ".repeat(4));
        assert!(long.len() > 1);
        assert!(long.iter().all(|line| line.len() <= COLUMNS));
        assert!(
            long.iter()
                .flat_map(|l| l.split(' '))
                .all(|g| g.len() == GROUP)
        );
    }

    #[test]
    fn plain_text_wraps_to_the_paper() {
        let plain = "Proceed to Snowmelt Narrows. Destroy its 3 bases. Scouts report a battle line in heavy plate.";
        let lines = wrap(&plain.to_uppercase());
        assert!(lines.len() >= 3);
        assert!(lines.iter().all(|line| line.len() <= COLUMNS));
        assert_eq!(lines.join(" "), plain.to_uppercase());
    }

    #[test]
    fn urgent_messages_go_ahead_of_routine_ones() {
        let mut tty = Teletype::default();
        tty.send(Message::new(Precedence::Routine, "A", "a"));
        tty.send(Message::new(Precedence::Routine, "B", "b"));
        tty.send(Message::new(Precedence::Flash, "C", "c"));
        tty.send(Message::new(Precedence::Immediate, "D", "d"));
        let order: String = tty.queue.iter().map(|m| m.code.as_str()).collect();
        assert_eq!(order, "CDAB");
    }

    #[test]
    fn a_message_prints_on_the_beat_and_ends_on_a_fresh_line() {
        let board = Soundboard::default();
        let mut tty = Teletype::default();
        let message = Message::new(Precedence::Flash, "ADVISE SHIP AHEAD", "Advise ship ahead.");
        let lines = message.lines("140612Z FEB");
        tty.send(message);
        // Bells, every character, and each line's return, with a little to spare.
        let characters: usize = lines.iter().map(|(l, _)| l.chars().count()).sum();
        let secs = 4. * BELL + BEAT * characters as f32 + RETURN * BEAT * lines.len() as f32;
        let mut t = 0.;
        while t < 0.95 * secs {
            tty.run(1. / 60., || "140612Z FEB".into(), &board);
            t += 1. / 60.;
        }
        assert!(tty.busy(), "it is not done before its time");
        for _ in 0..120 {
            tty.run(1. / 60., || "140612Z FEB".into(), &board);
        }
        assert!(!tty.busy());
        let printed: Vec<&str> = tty.paper.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(
            printed,
            [
                "FLASH 140612Z FEB CTF 88",
                "ADVIS ESHIP AHEAD",
                "ADVISE SHIP AHEAD.",
                "",
                ""
            ]
        );
    }
}
