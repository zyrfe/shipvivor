//! The battle's timeline. Every half second of a patrol leg the whole fight is kept: the
//! battle, her helm and the run, with its hull, clock, points and cards. From the pause
//! menu, or from the report of her loss, the timeline opens over the waters. Scrubbing
//! along it shows each moment as it stood, and she sails on from the one picked. Every
//! ship sunk is marked on it, and so is her own loss.
//!
//! The last minute is kept to the half second. Older moments thin out to one every two
//! seconds, so a long leg costs a few tens of megabytes at most (a moment is 40 to
//! 140 KB, nearly all of it the enemy ships).
//!
//! Every captain may rewind for now. It is meant to become a crew perk: [`permitted`] is
//! where that will be checked.
use crate::state::{Campaign, Embarked, Encounter, Guns, Mode, PlayerHelm, Screen};
use bevy::{ecs::system::SystemParam, prelude::*, ui::RelativeCursorPosition};
use shipvivor_campaign::{Phase, Run};
use shipvivor_encounter::{Battle, EffectKind, EnemyClass};
use shipvivor_navigation::Helm;

/// Seconds of battle between the moments kept over the last minute.
pub const STEP: f32 = 0.5;
/// Seconds of the latest battle kept at full grain.
const RECENT: f32 = 60.;
/// Seconds between the moments kept from before the last minute.
const SPARSE: f32 = 2.;
/// The timeline opens this many seconds of battle before the moment it was opened at.
const LOOK_BACK: f32 = 10.;
/// A jump to a sinking lands this many seconds before it, to watch it come.
const JUMP_LEAD: f32 = 3.;
/// Seconds of battle a held arrow key scrubs in a second, and with Shift.
const SCRUB_RATE: f32 = 12.;
const SCRUB_FAST: f32 = 48.;
/// Seconds an arrow key is held before it scrubs on its own.
const HOLD_DELAY: f32 = 0.25;

pub struct RewindPlugin;
impl Plugin for RewindPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Timeline>()
            .add_observer(forget)
            .add_observer(open)
            .add_systems(FixedUpdate, record.after(crate::advance_ship))
            // After the game's own input, so the key that closes the timeline isn't
            // also read by the screen it closes onto.
            .add_systems(
                PreUpdate,
                steer
                    .after(crate::aim_guns)
                    .run_if(|run: Res<Campaign>| run.screen() == Screen::Rewind),
            )
            .add_systems(Update, draw);
    }
}

/// Whether this captain may take the battle back. Always, for now: a crew perk will
/// decide it, with its officer aboard and any charges left.
pub fn permitted(run: &Campaign) -> bool {
    let _ = run;
    true
}

/// Open the timeline from the pause menu or the report of her loss.
#[derive(Event)]
pub struct Open;

/// One moment of the battle, whole.
#[derive(Clone)]
struct Frame {
    /// Seconds into the sector.
    at: f32,
    battle: Battle,
    helm: Helm,
    run: Run,
    hits: u32,
    sunk: u32,
    watch: crate::captain::Watch,
    /// The newest wreck the timeline had marked by then.
    wrecks: u64,
}

/// Something worth finding again on the timeline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MarkKind {
    /// An enemy ship of this class went down.
    Sunk(EnemyClass),
    /// Breakwater was lost.
    Lost,
}
#[derive(Clone, Copy, Debug)]
pub struct Mark {
    /// Seconds into the sector.
    pub at: f32,
    pub kind: MarkKind,
    /// Where it happened on the chart.
    pub pos: Vec2,
}

/// The timeline open on screen.
struct Scrub {
    /// The battle as it stood when the timeline was opened, to go back to.
    present: Frame,
    /// The mode it was opened from: paused, or on patrol with her lost.
    back: Mode,
    /// The moment picked, in seconds into the sector.
    cursor: f32,
    /// The frame the waters show, or `None` for the present.
    shown: Option<usize>,
    /// Seconds an arrow key has been held.
    held: f32,
    /// The pointer is dragging the playhead.
    dragging: bool,
}

/// The kept moments of the leg under way, and what happened in it.
#[derive(Resource, Default)]
pub struct Timeline {
    frames: Vec<Frame>,
    /// Frames before this index are already thinned to [`SPARSE`].
    sparse: usize,
    marks: Vec<Mark>,
    /// The newest wreck marked, by body id.
    wrecks: u64,
    lost_marked: bool,
    scrub: Option<Scrub>,
}
impl Timeline {
    /// Seconds into the sector of the first and last moments kept.
    pub fn span(&self) -> Option<(f32, f32)> {
        Some((self.frames.first()?.at, self.frames.last()?.at))
    }
    pub fn moments(&self) -> usize {
        self.frames.len()
    }
    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }
    /// The timeline is open, and the moment it shows.
    pub fn scrubbing(&self) -> Option<f32> {
        self.scrub.as_ref().map(|s| s.cursor)
    }
    /// Whether the timeline can open on this screen: a patrol leg with a moment kept.
    pub fn can_open(&self, run: &Campaign) -> bool {
        permitted(run)
            && !self.frames.is_empty()
            && self.scrub.is_none()
            && matches!(run.screen(), Screen::Paused | Screen::Lost)
    }
    /// Take the battle back to the latest moment kept at or before `at` seconds into
    /// the sector, and leave it paused there. The timeline after it is forgotten. The
    /// seconds it was taken from and to.
    pub fn rewind_to(&mut self, at: f32, live: &mut Live) -> Result<(f32, f32), String> {
        if !permitted(&live.run) {
            return Err("this captain can't rewind".into());
        }
        if self.scrub.is_some() {
            self.cancel(live);
        }
        if !matches!(live.run.mode, Mode::Patrol | Mode::Paused) {
            return Err("only a patrol leg has a timeline".into());
        }
        let index = self.frame_at(at).ok_or_else(|| match self.span() {
            Some((first, last)) => {
                format!("the timeline runs from {first:.1} s to {last:.1} s")
            }
            None => "nothing of this leg is kept yet".into(),
        })?;
        let from = live.battle.at_sea();
        live.apply(&self.frames[index]);
        let to = self.settle(index);
        live.run.mode = Mode::Paused;
        live.note_rewind(from, to);
        Ok((from, to))
    }

    /// The latest frame at or before `at`.
    fn frame_at(&self, at: f32) -> Option<usize> {
        let after = self.frames.partition_point(|f| f.at <= at + 1e-3);
        after.checked_sub(1)
    }
    /// Sail on from frame `index`: what came after it is forgotten. Its time.
    fn settle(&mut self, index: usize) -> f32 {
        self.frames.truncate(index + 1);
        self.sparse = self.sparse.min(self.frames.len());
        let frame = &self.frames[index];
        let to = frame.at;
        self.wrecks = frame.wrecks;
        self.lost_marked = false;
        self.marks.retain(|m| m.at <= to + 1e-3);
        to
    }
    /// Show the moment the playhead is on, if it isn't shown already.
    fn show(&mut self, live: &mut Live) {
        let Some(scrub) = &mut self.scrub else { return };
        let wanted = if scrub.cursor >= scrub.present.at - 1e-3 {
            None
        } else {
            let after = self.frames.partition_point(|f| f.at <= scrub.cursor + 1e-3);
            Some(after.saturating_sub(1))
        };
        if wanted == scrub.shown {
            return;
        }
        scrub.shown = wanted;
        match wanted {
            Some(index) => live.apply(&self.frames[index]),
            None => live.apply(&scrub.present),
        }
    }
    /// Close the timeline on the battle as it stood when it was opened.
    fn cancel(&mut self, live: &mut Live) {
        let Some(scrub) = self.scrub.take() else {
            return;
        };
        if scrub.shown.is_some() {
            live.apply(&scrub.present);
        }
        self.wrecks = scrub.present.wrecks;
        live.run.mode = scrub.back;
    }
    /// Sail on from the moment shown. From her loss, the present is no moment to sail
    /// on from, and the timeline stays open.
    fn confirm(&mut self, live: &mut Live) {
        let Some(scrub) = self.scrub.take() else {
            return;
        };
        match scrub.shown {
            None if scrub.back == Mode::Paused => live.run.mode = Mode::Patrol,
            None => self.scrub = Some(scrub),
            Some(index) => {
                let to = self.settle(index);
                live.run.mode = Mode::Patrol;
                live.note_rewind(scrub.present.at, to);
            }
        }
    }
    /// Note what this fixed step did, and keep the moment if one is due.
    fn observe(&mut self, run: &Campaign, battle: &Encounter, helm: &PlayerHelm) {
        let at = battle.at_sea();
        // A clock behind the last moment is a new voyage the timeline missed.
        if self.frames.last().is_some_and(|f| f.at > at + 1e-3) {
            *self = Self::default();
        }
        let mut newest = self.wrecks;
        for effect in battle.effects() {
            if let EffectKind::Wreck(class) = effect.kind
                && effect.body.id > self.wrecks
            {
                self.marks.push(Mark {
                    at,
                    kind: MarkKind::Sunk(class),
                    pos: effect.body.pos,
                });
                newest = newest.max(effect.body.id);
            }
        }
        self.wrecks = newest;
        if run.phase() == Phase::Lost && !self.lost_marked {
            self.lost_marked = true;
            self.marks.push(Mark {
                at,
                kind: MarkKind::Lost,
                pos: helm.position,
            });
        }
        if run.phase() != Phase::Sailing {
            return;
        }
        if self.frames.last().is_some_and(|f| at < f.at + STEP - 1e-4) {
            return;
        }
        self.frames.push(Frame {
            at,
            battle: battle.0.clone(),
            helm: helm.0.clone(),
            run: Run::clone(run),
            hits: run.hits,
            sunk: run.sunk,
            watch: run.watch.clone(),
            wrecks: self.wrecks,
        });
        self.thin(at);
    }
    /// Thin the moments that have fallen out of the last minute to one every
    /// [`SPARSE`] seconds.
    fn thin(&mut self, now: f32) {
        let cutoff = now - RECENT;
        let mut i = self.sparse.max(1);
        while i < self.frames.len() && self.frames[i].at < cutoff {
            if self.frames[i].at - self.frames[i - 1].at < SPARSE - 1e-3 {
                self.frames.remove(i);
            } else {
                i += 1;
            }
        }
        self.sparse = i.min(self.frames.len());
    }
}

/// Everything a moment of the battle is restored into.
#[derive(SystemParam)]
pub struct Live<'w> {
    pub run: ResMut<'w, Campaign>,
    pub battle: ResMut<'w, Encounter>,
    pub helm: ResMut<'w, PlayerHelm>,
    guns: ResMut<'w, Guns>,
    sea: ResMut<'w, crate::seascape::Sea>,
    board: Res<'w, crate::sound::Soundboard>,
    log: Option<ResMut<'w, crate::telemetry::Logbook>>,
}
impl Live<'_> {
    fn capture(&self, wrecks: u64) -> Frame {
        Frame {
            at: self.battle.at_sea(),
            battle: self.battle.0.clone(),
            helm: self.helm.0.clone(),
            run: Run::clone(&self.run),
            hits: self.run.hits,
            sunk: self.run.sunk,
            watch: self.run.watch.clone(),
            wrecks,
        }
    }
    fn apply(&mut self, frame: &Frame) {
        self.battle.0 = frame.battle.clone();
        self.helm.0 = frame.helm.clone();
        let run: &mut Run = &mut self.run;
        *run = frame.run.clone();
        self.run.hits = frame.hits;
        self.run.sunk = frame.sunk;
        self.run.watch = frame.watch.clone();
        self.run.clearance_left = 0.;
        self.run.chart_roll = None;
        self.guns.volley = None;
        self.guns.launch_pending = false;
        self.guns.firing = false;
        self.sea.calm();
        self.board.silence();
    }
    fn note_rewind(&mut self, from: f32, to: f32) {
        if let Some(log) = &mut self.log {
            log.rewound(&self.run, &self.battle, from, to);
        }
    }
}

/// A new voyage has a timeline of its own.
fn forget(_: On<Embarked>, mut timeline: ResMut<Timeline>) {
    *timeline = Timeline::default();
}

fn open(_: On<Open>, mut timeline: ResMut<Timeline>, mut live: Live) {
    if !timeline.can_open(&live.run) {
        return;
    }
    let present = live.capture(timeline.wrecks);
    let first = timeline.frames[0].at;
    let cursor = (present.at - LOOK_BACK).max(first);
    timeline.scrub = Some(Scrub {
        present,
        back: live.run.mode,
        cursor,
        shown: None,
        held: 0.,
        dragging: false,
    });
    live.run.mode = Mode::Rewind;
    live.run.chart_roll = None;
    timeline.show(&mut live);
}

fn record(
    run: Res<Campaign>,
    battle: Res<Encounter>,
    helm: Res<PlayerHelm>,
    mut timeline: ResMut<Timeline>,
) {
    if run.mode == Mode::Patrol && timeline.scrub.is_none() {
        timeline.observe(&run, &battle, &helm);
    }
}

/// The parts of the timeline bar the pointer works.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Control {
    Track,
    Sail,
    Back,
}

fn steer(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    controls: Query<(&Control, &RelativeCursorPosition)>,
    mut timeline: ResMut<Timeline>,
    mut live: Live,
) {
    let timeline = &mut *timeline;
    if timeline.scrub.is_none() {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        timeline.cancel(&mut live);
        return;
    }
    let pressed = |control| {
        mouse.just_pressed(MouseButton::Left)
            && controls
                .iter()
                .any(|(c, p)| *c == control && p.cursor_over())
    };
    if keys.just_pressed(KeyCode::Enter) || pressed(Control::Sail) {
        timeline.confirm(&mut live);
        return;
    }
    if pressed(Control::Back) {
        timeline.cancel(&mut live);
        return;
    }
    let first = timeline.frames[0].at;
    let Some(scrub) = timeline.scrub.as_mut() else {
        return;
    };
    let last = scrub.present.at;
    // Drag along the track.
    if pressed(Control::Track) {
        scrub.dragging = true;
    }
    if !mouse.pressed(MouseButton::Left) {
        scrub.dragging = false;
    }
    if scrub.dragging
        && let Some(at) = (controls.iter())
            .find(|(c, _)| **c == Control::Track)
            .and_then(|(_, p)| p.normalized)
    {
        scrub.cursor = first + (at.x + 0.5).clamp(0., 1.) * (last - first);
    }
    // Arrows step a moment, and held, scrub.
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let dir = f32::from(i8::from(keys.pressed(KeyCode::ArrowRight)))
        - f32::from(i8::from(keys.pressed(KeyCode::ArrowLeft)));
    if keys.any_just_pressed([KeyCode::ArrowLeft, KeyCode::ArrowRight]) {
        scrub.held = 0.;
        scrub.cursor += dir * if shift { 10. } else { STEP };
    } else if dir != 0. {
        scrub.held += time.delta_secs();
        if scrub.held > HOLD_DELAY {
            let rate = if shift { SCRUB_FAST } else { SCRUB_RATE };
            scrub.cursor += dir * rate * time.delta_secs();
        }
    }
    // [ and ] jump to the sinking before or after the playhead, a little ahead of it.
    let back = keys.just_pressed(KeyCode::BracketLeft);
    if back || keys.just_pressed(KeyCode::BracketRight) {
        let cursor = scrub.cursor;
        let targets = (timeline.marks.iter()).map(|m| (m.at - JUMP_LEAD).max(first));
        let target = if back {
            targets.filter(|&t| t < cursor - 0.25).reduce(f32::max)
        } else {
            targets.filter(|&t| t > cursor + 0.25).reduce(f32::min)
        };
        if let Some(t) = target {
            scrub.cursor = t;
        }
    }
    if keys.just_pressed(KeyCode::Home) {
        scrub.cursor = first;
    }
    if keys.just_pressed(KeyCode::End) {
        scrub.cursor = last;
    }
    scrub.cursor = scrub.cursor.clamp(first, last);
    timeline.show(&mut live);
}

#[derive(Component)]
struct Bar;
#[derive(Component)]
struct Playhead;
#[derive(Component)]
struct Ahead;
#[derive(Component)]
struct Reading;
#[derive(Component)]
struct Caption;
#[derive(Component)]
struct SailLabel;

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
fn danger() -> Color {
    Color::srgb_u8(236, 92, 72)
}
/// The colour a mark is struck in: brass for a warship, gold and taller for a boss,
/// pale for a merchant, red for her own loss.
fn mark_color(kind: MarkKind) -> Color {
    match kind {
        MarkKind::Lost => danger(),
        MarkKind::Sunk(class) if class.is_boss() => Color::srgb_u8(255, 228, 140),
        MarkKind::Sunk(class) if class.is_merchant() => pale(),
        MarkKind::Sunk(_) => brass(),
    }
}
fn clock(secs: f32) -> String {
    let secs = secs.max(0.).round() as u32;
    format!("{}:{:02}", secs / 60, secs % 60)
}
fn mark_name(kind: MarkKind) -> String {
    match kind {
        MarkKind::Sunk(class) => format!("{} sunk", crate::mission_hud::ship_class(class)),
        MarkKind::Lost => "Breakwater lost".into(),
    }
}

/// The timeline bar along the foot of the screen, while it is open.
fn draw(
    mut cmd: Commands,
    timeline: Res<Timeline>,
    bars: Query<Entity, With<Bar>>,
    mut nodes: ParamSet<(
        Query<&mut Node, With<Playhead>>,
        Query<&mut Node, With<Ahead>>,
    )>,
    mut texts: ParamSet<(
        Query<&mut Text, With<Reading>>,
        Query<&mut Text, With<Caption>>,
        Query<&mut Text, With<SailLabel>>,
    )>,
) {
    let Some(scrub) = &timeline.scrub else {
        for bar in &bars {
            cmd.entity(bar).despawn();
        }
        return;
    };
    let first = timeline.frames[0].at;
    let length = (scrub.present.at - first).max(1e-3);
    let share = |at: f32| ((at - first) / length).clamp(0., 1.) * 100.;
    if bars.is_empty() {
        build(&mut cmd, &timeline, scrub, share);
    }
    let head = share(scrub.cursor);
    for mut node in &mut nodes.p0() {
        node.left = percent(head);
    }
    for mut node in &mut nodes.p1() {
        node.left = percent(head);
        node.width = percent(100. - head);
    }
    let shown = scrub
        .shown
        .map_or(scrub.present.at, |i| timeline.frames[i].at);
    let back = scrub.present.at - shown;
    for mut text in &mut texts.p0() {
        text.0 = if back > 0.05 {
            format!("{}   /   {:.0} s back", clock(shown), back)
        } else {
            format!("{}   /   now", clock(shown))
        };
    }
    let near = (timeline.marks.iter())
        .filter(|m| (m.at - scrub.cursor).abs() < 2.5)
        .min_by(|a, b| {
            (a.at - scrub.cursor)
                .abs()
                .total_cmp(&(b.at - scrub.cursor).abs())
        });
    for mut text in &mut texts.p1() {
        text.0 = near.map_or(String::new(), |m| {
            format!("{} at {}", mark_name(m.kind), clock(m.at))
        });
    }
    let lost_now = scrub.shown.is_none() && scrub.back != Mode::Paused;
    for mut text in &mut texts.p2() {
        text.0 = if lost_now {
            "Pick a moment before her loss".into()
        } else if scrub.shown.is_none() {
            "Sail on     [Enter]".into()
        } else {
            "Sail from here     [Enter]".into()
        };
    }
}

fn build(cmd: &mut Commands, timeline: &Timeline, scrub: &Scrub, share: impl Fn(f32) -> f32) {
    let bar = cmd
        .spawn((
            Bar,
            Node {
                position_type: PositionType::Absolute,
                left: percent(6),
                right: percent(6),
                bottom: px(28),
                padding: UiRect::axes(px(20), px(14)),
                border: UiRect::top(px(3)),
                flex_direction: FlexDirection::Column,
                row_gap: px(12),
                ..default()
            },
            BorderColor::all(brass()),
            BackgroundColor(Color::srgba_u8(13, 32, 42, 236)),
            GlobalZIndex(20),
        ))
        .id();
    // The heading: what this is, the moment shown, and the mark beside the playhead.
    let head = cmd
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Baseline,
                column_gap: px(18),
                ..default()
            },
            ChildOf(bar),
        ))
        .id();
    let sinkings = (timeline.marks.iter())
        .filter(|m| matches!(m.kind, MarkKind::Sunk(_)))
        .count();
    cmd.spawn((
        Text::new(format!("REWIND   /   {sinkings} SUNK THIS LEG")),
        TextFont::from_font_size(14.),
        TextColor(brass()),
        ChildOf(head),
    ));
    cmd.spawn((
        Reading,
        Text::new(""),
        TextFont::from_font_size(22.),
        TextColor(ink()),
        ChildOf(head),
    ));
    cmd.spawn((
        Caption,
        Text::new(""),
        TextFont::from_font_size(17.),
        TextColor(brass()),
        ChildOf(head),
    ));
    // The track: the leg from its first moment kept to the present, a tick for every
    // sinking, the stretch ahead of the playhead shaded.
    let track = cmd
        .spawn((
            Control::Track,
            RelativeCursorPosition::default(),
            Node {
                height: px(34),
                width: percent(100),
                ..default()
            },
            BackgroundColor(Color::srgb_u8(30, 61, 68)),
            ChildOf(bar),
        ))
        .id();
    cmd.spawn((
        Ahead,
        Node {
            position_type: PositionType::Absolute,
            top: px(0),
            bottom: px(0),
            ..default()
        },
        BackgroundColor(Color::srgba_u8(6, 18, 26, 150)),
        ChildOf(track),
    ));
    for mark in &timeline.marks {
        let tall = matches!(mark.kind, MarkKind::Lost)
            || matches!(mark.kind, MarkKind::Sunk(class) if class.is_boss());
        cmd.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: percent(share(mark.at)),
                margin: UiRect::left(px(if tall { -3. } else { -2. })),
                width: px(if tall { 6. } else { 4. }),
                top: px(if tall { -6. } else { 6. }),
                bottom: px(if tall { -6. } else { 6. }),
                ..default()
            },
            BackgroundColor(mark_color(mark.kind)),
            ChildOf(track),
        ));
    }
    cmd.spawn((
        Playhead,
        Node {
            position_type: PositionType::Absolute,
            margin: UiRect::left(px(-2)),
            width: px(4),
            top: px(-10),
            bottom: px(-10),
            ..default()
        },
        BackgroundColor(ink()),
        ChildOf(track),
    ));
    // The ends of the track, and the keys.
    let foot = cmd
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(14),
                ..default()
            },
            ChildOf(bar),
        ))
        .id();
    cmd.spawn((
        Text::new(format!(
            "{}  to  {}        ← →  scrub   (Shift: faster)        [ ]  sinkings        Home End",
            clock(timeline.frames[0].at),
            clock(scrub.present.at)
        )),
        TextFont::from_font_size(14.),
        TextColor(pale()),
        Node {
            flex_grow: 1.,
            ..default()
        },
        ChildOf(foot),
    ));
    for (control, label) in [(Control::Back, "Back     [Esc]"), (Control::Sail, "")] {
        let button = cmd
            .spawn((
                control,
                RelativeCursorPosition::default(),
                Node {
                    padding: UiRect::axes(px(16), px(9)),
                    border: UiRect::left(px(3)),
                    ..default()
                },
                BackgroundColor(Color::srgb_u8(30, 61, 68)),
                BorderColor::all(if control == Control::Sail {
                    brass()
                } else {
                    trim()
                }),
                ChildOf(foot),
            ))
            .id();
        let mut text = cmd.spawn((
            Text::new(label),
            TextFont::from_font_size(16.),
            TextColor(ink()),
            ChildOf(button),
        ));
        if control == Control::Sail {
            text.insert(SailLabel);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(at: f32) -> Frame {
        let run = Campaign::new(Mode::Patrol);
        Frame {
            at,
            battle: Encounter::default().0,
            helm: Helm::default(),
            run: Run::clone(&run),
            hits: 0,
            sunk: 0,
            watch: Default::default(),
            wrecks: 0,
        }
    }

    #[test]
    fn the_last_minute_is_kept_to_the_half_second_and_older_moments_thinned() {
        let mut timeline = Timeline::default();
        let mut at = 0.;
        while at <= 180. {
            timeline.frames.push(frame(at));
            timeline.thin(at);
            at += STEP;
        }
        let recent = timeline.frames.iter().filter(|f| f.at >= 120.).count();
        assert_eq!(recent, 121);
        let old: Vec<f32> = (timeline.frames.iter())
            .map(|f| f.at)
            .filter(|&t| t < 120.)
            .collect();
        assert_eq!(old.first(), Some(&0.));
        for pair in old.windows(2) {
            assert!((pair[1] - pair[0] - SPARSE).abs() < 1e-3, "{pair:?}");
        }
    }

    #[test]
    fn a_moment_is_found_at_or_before_the_time_asked_and_sailing_on_forgets_the_rest() {
        let mut timeline = Timeline::default();
        for i in 0..10 {
            timeline.frames.push(frame(i as f32 * STEP));
        }
        timeline.marks.push(Mark {
            at: 1.0,
            kind: MarkKind::Sunk(EnemyClass::Cruiser),
            pos: Vec2::ZERO,
        });
        timeline.marks.push(Mark {
            at: 3.5,
            kind: MarkKind::Lost,
            pos: Vec2::ZERO,
        });
        assert_eq!(timeline.frame_at(-1.), None);
        assert_eq!(timeline.frame_at(1.2), Some(2));
        assert_eq!(timeline.frame_at(99.), Some(9));
        assert_eq!(timeline.settle(2), 1.0);
        assert_eq!(timeline.frames.len(), 3);
        assert_eq!(timeline.marks.len(), 1, "the sinking at the moment stays");
    }
}
