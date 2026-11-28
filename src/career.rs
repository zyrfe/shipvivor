//! The career kept between launches: one captain carried from run to run, with the tiers
//! their skills have reached and the career points runs have paid. It lives in
//! `career.toml` in the platform's settings folder (or the file `SHIPVIVOR_CAREER_FILE`
//! names), written whenever it changes. A run that ends under the career captain banks
//! its points at once.
//!
//! The career page launches a run with a short jaunt: the title camera holds where it
//! stands while the ship rings up and surges past it and away, the screen fades to black,
//! and the chart of anchorages fades in. A click or ESC skips straight to the fade-in.
use crate::crew_book::CrewBook;
use crate::state::{Campaign, PlayerHelm, Screen};
use bevy::prelude::*;
use shipvivor_campaign::{Roster, StartingHull};
use std::path::{Path, PathBuf};

/// Seconds the ship sails before the chart of anchorages takes the screen.
pub const LAUNCH_SAIL: f32 = 3.;
/// Seconds into the sail the fade to black begins.
const FADE_OUT_FROM: f32 = 2.2;
/// Seconds the chart takes to fade in from black.
const FADE_IN: f32 = 0.6;
/// Metres a second the ship surges ahead of her cruising speed once she has rung up.
const SURGE: f32 = 16.;
/// Seconds she takes to ring up to the full surge.
const RING_UP: f32 = 1.;

pub struct CareerPlugin;
impl Plugin for CareerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CareerBook>().add_systems(
            Update,
            (
                settle,
                dress_title,
                crate::ui::career::draw.after(crate::ui::rebuild_screen),
                sail_launch,
            ),
        );
    }
}

/// The career, if there is one, and the career page's picks.
#[derive(Resource)]
pub struct CareerBook {
    /// Every captain's career, and who sailed last.
    pub roster: Roster,
    /// The captain previewed on the career page, by place in the crew data. Nothing is
    /// locked in until the launch.
    pub captain: usize,
    /// The hull previewed on the career page.
    pub hull: StartingHull,
    /// Career points the last run banked, to show on the career page.
    pub last_banked: Option<(String, u32)>,
    /// Counts every change, so the career page knows to redraw.
    pub revision: u32,
    /// The launch under way, if one is.
    pub launch: Option<Launch>,
}

/// A run being launched from the career page: the ship sailing off, and the fades.
pub struct Launch {
    /// Seconds since the launch.
    pub elapsed: f32,
    /// Where the ship lay at the launch, which the title camera holds to while she sails
    /// out of its view.
    pub anchor: PlayerHelm,
    /// The chart of anchorages has the screen, and is fading in.
    pub arrived: bool,
}

impl Launch {
    /// How dark the screen is, from 0 to 1.
    pub fn shade(&self) -> f32 {
        if self.arrived {
            1. - ((self.elapsed - LAUNCH_SAIL) / FADE_IN).clamp(0., 1.)
        } else {
            let t = ((self.elapsed - FADE_OUT_FROM) / (LAUNCH_SAIL - FADE_OUT_FROM)).clamp(0., 1.);
            t * t * (3. - 2. * t)
        }
    }
    /// The sail is done, or was skipped: time for the chart.
    pub fn sailed(&self) -> bool {
        self.elapsed >= LAUNCH_SAIL
    }
    /// Go straight to the chart's fade-in.
    pub fn skip(&mut self) {
        self.elapsed = self.elapsed.max(LAUNCH_SAIL);
    }
}

impl CareerBook {
    /// No career.
    fn empty() -> Self {
        Self {
            roster: Roster::default(),
            captain: 0,
            hull: StartingHull::LightCruiser,
            last_banked: None,
            revision: 0,
            launch: None,
        }
    }

    /// The career kept in its file, if there is one the crew data still knows.
    pub fn open(crew: &CrewBook) -> Self {
        let mut book = Self::empty();
        if cfg!(test) {
            return book;
        }
        book.hull = last_ship().unwrap_or_default();
        let Some(path) = shelf() else {
            return book;
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return book;
        };
        match Roster::parse(&text, &crew.wardroom) {
            Ok((roster, problems)) => {
                for problem in problems {
                    warn!("{}: {problem}", path.display());
                }
                book.roster = roster;
            }
            Err(problem) => warn!(
                "{} not taken, starting without careers: {problem}",
                path.display()
            ),
        }
        book
    }

    /// Open the career page on the captain who sailed last, or the first, and whatever
    /// hull was last previewed.
    pub fn open_page(&mut self, crew: &CrewBook) {
        let wardroom = &crew.wardroom;
        self.captain = self
            .roster
            .last
            .as_ref()
            .and_then(|last| wardroom.captains.iter().position(|c| &c.id == last))
            .unwrap_or(0)
            .min(wardroom.captains.len().saturating_sub(1));
        self.launch = None;
        self.revision = self.revision.wrapping_add(1);
    }

    /// Where the title camera holds while the ship sails off, during a launch.
    pub fn camera_anchor(&self) -> Option<&PlayerHelm> {
        self.launch
            .as_ref()
            .filter(|launch| !launch.arrived)
            .map(|launch| &launch.anchor)
    }

    /// Note a change, and keep the career for the next launch.
    pub fn changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        keep(&self.roster);
    }
}

impl FromWorld for CareerBook {
    fn from_world(world: &mut World) -> Self {
        match world.get_resource::<CrewBook>() {
            Some(crew) => Self::open(crew),
            None => Self::empty(),
        }
    }
}

/// Where the career is kept: the file `SHIPVIVOR_CAREER_FILE` names, or `career.toml`
/// in the platform's settings folder for Shipvivor.
fn shelf() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SHIPVIVOR_CAREER_FILE") {
        return Some(path.into());
    }
    crate::ensign::settings_folder().map(|folder| folder.join("career.toml"))
}

fn keep_at(path: &Path, roster: &Roster) -> std::io::Result<()> {
    if roster.careers().is_empty() {
        return if path.exists() {
            std::fs::remove_file(path)
        } else {
            Ok(())
        };
    }
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    std::fs::write(path, roster.code())
}

/// Where the hull last launched is kept: the file `SHIPVIVOR_LAST_SHIP_FILE` names, or
/// `last-ship.txt` beside the career.
fn ship_shelf() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SHIPVIVOR_LAST_SHIP_FILE") {
        return Some(path.into());
    }
    crate::ensign::settings_folder().map(|folder| folder.join("last-ship.txt"))
}

/// The hull last launched, if one was and the file still names a hull.
fn last_ship() -> Option<StartingHull> {
    let text = std::fs::read_to_string(ship_shelf()?).ok()?;
    StartingHull::from_id(text.trim())
}

/// Keep the hull just launched, so the game opens on her next time. Tests keep nothing.
pub fn keep_ship(hull: StartingHull) {
    if cfg!(test) {
        return;
    }
    let Some(path) = ship_shelf() else {
        return;
    };
    let kept = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&path, format!("{}\n", hull.id())));
    if let Err(err) = kept {
        warn!("Could not keep the last ship in {}: {err}", path.display());
    }
}

/// The title sails the hull last launched, or the one previewed on the career page. A
/// ship built in test mode is left as she is.
fn dress_title(mut run: ResMut<Campaign>, book: Res<CareerBook>) {
    if run.mode != crate::state::Mode::Title || run.is_exercise() || run.hull == book.hull {
        return;
    }
    let dressed = std::mem::take(&mut **run).with_hull(book.hull);
    **run = dressed;
}

/// Keep the career, or with none, clear the file. Tests keep nothing.
fn keep(roster: &Roster) {
    if cfg!(test) {
        return;
    }
    let Some(path) = shelf() else {
        warn!("Nowhere to keep the career: no settings folder");
        return;
    };
    if let Err(err) = keep_at(&path, roster) {
        warn!("Could not keep the career in {}: {err}", path.display());
    }
}

/// Bank a run that has ended under the career captain.
fn settle(run: Res<Campaign>, mut book: ResMut<CareerBook>, crew: Res<CrewBook>) {
    if !matches!(run.screen(), Screen::Won(_) | Screen::Lost) || run.captain.is_none() {
        return;
    }
    let Some(id) = run.captain.as_ref().map(|c| c.id.clone()) else {
        return;
    };
    let Some(career) = book.roster.get_mut(&id) else {
        return;
    };
    if let Some(points) = career.settle(&run, &crew.wardroom.career) {
        info!("{} banks {points} career points", career.name);
        book.last_banked = Some((id, points));
        book.changed();
    }
}

/// A full-screen shade over everything while a launch fades out and in.
#[derive(Component)]
struct LaunchShade;

/// Sail a launch: the ship surges off while the camera holds, the screen fades, and once
/// the chart has the screen it fades back in. A click or ESC skips to the fade-in.
fn sail_launch(
    mut cmd: Commands,
    time: Res<Time>,
    mut book: ResMut<CareerBook>,
    mut helm: ResMut<PlayerHelm>,
    run: Res<Campaign>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut shades: Query<(Entity, &mut BackgroundColor), With<LaunchShade>>,
) {
    let Some(launch) = book.launch.as_mut() else {
        for (shade, _) in &shades {
            cmd.entity(shade).despawn();
        }
        return;
    };
    // Left the page some other way: nothing to sail.
    if !launch.arrived && run.screen() != Screen::Career {
        book.launch = None;
        return;
    }
    let dt = time.delta_secs();
    if !launch.arrived {
        if keys.just_pressed(KeyCode::Escape) || mouse.just_pressed(MouseButton::Left) {
            launch.skip();
        }
        let rung = (launch.elapsed / RING_UP).clamp(0., 1.);
        let ahead = helm.forward();
        helm.position += ahead * SURGE * rung * rung * dt;
    }
    launch.elapsed += dt;
    let shade = launch.shade();
    if launch.arrived && launch.elapsed >= LAUNCH_SAIL + FADE_IN {
        book.launch = None;
    }
    let colour = Color::srgba(0.01, 0.03, 0.05, shade);
    match shades.single_mut() {
        Ok((_, mut fill)) => fill.0 = colour,
        Err(_) => {
            cmd.spawn((
                LaunchShade,
                GlobalZIndex(50),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    top: px(0),
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                BackgroundColor(colour),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shipvivor_campaign::crew::Wardroom;

    #[test]
    fn the_title_sails_the_ship_last_launched_but_leaves_a_test_ship_alone() {
        use crate::state::Mode;
        let mut app = App::new();
        app.insert_resource(Campaign::new(Mode::Title))
            .insert_resource(CareerBook {
                hull: StartingHull::HeavyDestroyer,
                ..CareerBook::empty()
            })
            .add_systems(Update, dress_title);
        app.update();
        let run = app.world().resource::<Campaign>();
        assert_eq!(run.hull, StartingHull::HeavyDestroyer);
        assert_eq!(run.ship.design().tube_mounts().len(), 3);

        // A ship built in test mode keeps her own hull at the title.
        let exercise = shipvivor_campaign::Run::exercise(
            shipvivor_campaign::Stage::IronNarrows,
            shipvivor_campaign::Grade::Moderate,
            1,
            1,
        );
        app.insert_resource(Campaign::with_run(Mode::Title, exercise));
        app.update();
        assert_eq!(
            app.world().resource::<Campaign>().hull,
            StartingHull::LightCruiser
        );
        for hull in StartingHull::ALL {
            assert_eq!(StartingHull::from_id(hull.id()), Some(hull));
        }
    }

    #[test]
    fn every_career_is_kept_in_its_file_and_an_empty_roster_clears_it() {
        let folder = std::env::temp_dir().join(format!("shipvivor-career-{}", std::process::id()));
        let path = folder.join("deeper").join("career.toml");
        let wardroom = Wardroom::stock();
        let mut roster = Roster::default();
        roster.enlist(&wardroom.captains[0]).points = 9;
        roster.enlist(&wardroom.captains[1]).points = 2;
        roster.last = Some(wardroom.captains[1].id.clone());
        keep_at(&path, &roster).unwrap();
        let (kept, problems) =
            Roster::parse(&std::fs::read_to_string(&path).unwrap(), &wardroom).unwrap();
        assert_eq!((kept, problems), (roster, vec![]));
        keep_at(&path, &Roster::default()).unwrap();
        assert!(!path.exists());
        keep_at(&path, &Roster::default()).unwrap();
        std::fs::remove_dir_all(&folder).unwrap();
    }
}
