//! The colours each side flies. Breakwater's are the player's own, picked on the
//! customise screen and kept between launches. The enemy hoists fresh colours for every
//! leg, drawn never to be mistaken for hers.
use crate::state::{Campaign, ChartSeeds, Mode};
use bevy::prelude::*;
use shipvivor_encounter::leg_seed;
use shipvivor_ensign::{Flag, Pattern, Tincture};
use std::path::{Path, PathBuf};

/// A leg no stage uses, so the enemy's colours never share a draw with a chart.
const RAIDERS: u64 = 0xF1A7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Breakwater,
    Enemy,
}

#[derive(Resource)]
pub struct Colours {
    pub breakwater: Flag,
    pub enemy: Flag,
    /// The seed and the flag of Breakwater's that the enemy's colours were drawn against.
    drawn: (u64, Flag),
}

impl Default for Colours {
    fn default() -> Self {
        // Tests fly the stock flag rather than whatever the player last picked.
        let kept = if cfg!(test) {
            None
        } else {
            locker().and_then(|path| stowed(&path))
        };
        Self::against(kept.unwrap_or_else(stock), 0)
    }
}

impl Colours {
    fn against(breakwater: Flag, seed: u64) -> Self {
        Self {
            enemy: Flag::rival(seed, &breakwater),
            drawn: (seed, breakwater.clone()),
            breakwater,
        }
    }

    pub fn flag(&self, side: Side) -> &Flag {
        match side {
            Side::Breakwater => &self.breakwater,
            Side::Enemy => &self.enemy,
        }
    }
}

/// What Breakwater flies until the player picks her colours.
fn stock() -> Flag {
    use Tincture::*;
    Flag::new(Pattern::Canton(2), vec![Gold, Navy, Scarlet]).expect("three slots")
}

/// The seed of the enemy's colours for the leg in hand. At the title and in the helm
/// trial no run has been drawn yet, so they come from the next one.
fn draw(run: &Campaign, next_run: u64) -> u64 {
    let base = match run.mode {
        Mode::Title | Mode::Trial => next_run,
        Mode::Patrol | Mode::Paused | Mode::Rewind => run.seed,
    };
    leg_seed(leg_seed(base, run.stage as u64), RAIDERS)
}

/// Run up new enemy colours when a run or a leg begins, or when Breakwater changes hers.
pub fn hoist(run: Res<Campaign>, seeds: Res<ChartSeeds>, mut colours: ResMut<Colours>) {
    let seed = draw(&run, seeds.peek());
    let (drawn, against) = &colours.drawn;
    if *drawn != seed || *against != colours.breakwater {
        *colours = Colours::against(colours.breakwater.clone(), seed);
    }
}

/// Where the player's flag is kept between launches: the file `SHIPVIVOR_FLAG_FILE`
/// names, or `flag.txt` in the platform's settings folder for Shipvivor.
fn locker() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SHIPVIVOR_FLAG_FILE") {
        return Some(path.into());
    }
    settings_folder().map(|folder| folder.join("flag.txt"))
}

/// The platform's settings folder for Shipvivor, where what the player chose is kept
/// between launches.
pub fn settings_folder() -> Option<PathBuf> {
    let var = |name| std::env::var_os(name).map(PathBuf::from);
    if cfg!(target_os = "macos") {
        var("HOME").map(|home| home.join("Library/Application Support/Shipvivor"))
    } else if cfg!(windows) {
        var("APPDATA").map(|data| data.join("Shipvivor"))
    } else {
        var("XDG_CONFIG_HOME")
            .or_else(|| var("HOME").map(|home| home.join(".config")))
            .map(|config| config.join("shipvivor"))
    }
}

fn stowed(path: &Path) -> Option<Flag> {
    Flag::parse(&std::fs::read_to_string(path).ok()?)
}

fn stow_at(path: &Path, flag: &Flag) -> std::io::Result<()> {
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    std::fs::write(path, format!("{}\n", flag.code()))
}

/// Keep the player's flag for the next launch. Tests keep nothing.
pub fn stow(flag: &Flag) {
    if cfg!(test) {
        return;
    }
    let Some(path) = locker() else {
        warn!("Nowhere to keep the flag: no settings folder");
        return;
    };
    if let Err(err) = stow_at(&path, flag) {
        warn!("Could not keep the flag in {}: {err}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shipvivor_campaign::Stage;

    #[test]
    fn the_title_draws_the_enemy_the_next_run_meets_first() {
        let mut seeds = ChartSeeds::fixed(42);
        let title = draw(&Campaign::default(), seeds.peek());
        let trial = draw(&Campaign::new(Mode::Trial), seeds.peek());
        let mut run = Campaign::new(Mode::Patrol);
        run.seed = seeds.draw();
        let sailing = draw(&run, seeds.peek());
        assert_eq!(title, sailing);
        assert_eq!(trial, sailing);
        run.mode = Mode::Paused;
        assert_eq!(draw(&run, seeds.peek()), sailing);
    }

    #[test]
    fn the_enemy_hoists_new_colours_every_leg_and_whenever_ours_change() {
        let mut run = Campaign::new(Mode::Patrol);
        run.seed = 7;
        let mut seen = Vec::new();
        for stage in [
            Stage::IronNarrows,
            Stage::OuterRoad,
            Stage::GreywaterSound,
            Stage::PilotsCut,
            Stage::CrownDeep,
        ] {
            run.stage = stage;
            let seed = draw(&run, 0);
            assert!(!seen.contains(&seed));
            seen.push(seed);
            let colours = Colours::against(stock(), seed);
            assert!(
                colours.enemy.difference(&colours.breakwater) >= shipvivor_ensign::MIN_DIFFERENCE
            );
        }

        let mut app = App::new();
        app.insert_resource(Campaign::new(Mode::Patrol))
            .insert_resource(ChartSeeds::fixed(1))
            .init_resource::<Colours>()
            .add_systems(Update, hoist);
        app.update();
        let ours = Flag::new(Pattern::Bands(2), vec![Tincture::White, Tincture::Scarlet]).unwrap();
        app.world_mut().resource_mut::<Colours>().breakwater = ours.clone();
        app.update();
        let colours = app.world().resource::<Colours>();
        assert_eq!(colours.breakwater, ours);
        assert_eq!(colours.enemy, Flag::rival(colours.drawn.0, &ours));
        assert!(colours.enemy.difference(&ours) >= shipvivor_ensign::MIN_DIFFERENCE);
    }

    #[test]
    fn the_flag_is_kept_between_launches() {
        let folder = std::env::temp_dir().join(format!("shipvivor-flag-{}", std::process::id()));
        let path = folder.join("deeper").join("flag.txt");
        assert_eq!(stowed(&path), None);
        let flag = Flag::new(
            Pattern::Saltire,
            vec![Tincture::Green, Tincture::Black, Tincture::Gold],
        )
        .unwrap();
        stow_at(&path, &flag).unwrap();
        assert_eq!(stowed(&path), Some(flag));
        std::fs::write(&path, "bands9 mauve").unwrap();
        assert_eq!(stowed(&path), None);
        std::fs::remove_dir_all(&folder).unwrap();
        assert!(stock().readable());
    }
}
