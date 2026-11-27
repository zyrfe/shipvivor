//! Game options, set from the title's options screen and kept between launches.
use bevy::prelude::*;
use shipvivor_encounter::Rewards;
use std::path::{Path, PathBuf};

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameOptions {
    /// Whether experience comes straight from sinking ships and wrecking yards, or from
    /// the packages their wrecks leave to tow in.
    pub rewards: Rewards,
    /// Whether the yard after each round and the report at the end of a run ask for
    /// four quick playtest scores. See [`crate::feedback`].
    pub feedback: bool,
}

impl GameOptions {
    /// What a player who has never opened the options screen gets.
    pub const STOCK: Self = Self {
        rewards: Rewards::Direct,
        feedback: true,
    };

    /// One `key = value` line per option.
    fn code(&self) -> String {
        let rewards = match self.rewards {
            Rewards::Direct => "direct",
            Rewards::Packages => "packages",
        };
        let feedback = if self.feedback { "on" } else { "off" };
        format!("rewards = {rewards}\nfeedback = {feedback}\n")
    }

    /// Options from their lines. A line that is not understood keeps the stock choice.
    fn parse(text: &str) -> Self {
        let mut options = Self::STOCK;
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match (key.trim(), value.trim()) {
                ("rewards", "direct") => options.rewards = Rewards::Direct,
                ("rewards", "packages") => options.rewards = Rewards::Packages,
                ("feedback", "on") => options.feedback = true,
                ("feedback", "off") => options.feedback = false,
                _ => {}
            }
        }
        options
    }
}

impl Default for GameOptions {
    fn default() -> Self {
        // Tests play with the stock options rather than whatever the player last picked.
        if cfg!(test) {
            return Self::STOCK;
        }
        shelf()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .map_or(Self::STOCK, |text| Self::parse(&text))
    }
}

/// Where the options are kept: the file `SHIPVIVOR_OPTIONS_FILE` names, or
/// `options.txt` in the platform's settings folder for Shipvivor.
fn shelf() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SHIPVIVOR_OPTIONS_FILE") {
        return Some(path.into());
    }
    crate::ensign::settings_folder().map(|folder| folder.join("options.txt"))
}

fn keep_at(path: &Path, options: &GameOptions) -> std::io::Result<()> {
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    std::fs::write(path, options.code())
}

/// Keep the options for the next launch. Tests keep nothing.
pub fn keep(options: &GameOptions) {
    if cfg!(test) {
        return;
    }
    let Some(path) = shelf() else {
        warn!("Nowhere to keep the options: no settings folder");
        return;
    };
    if let Err(err) = keep_at(&path, options) {
        warn!("Could not keep the options in {}: {err}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewards_default_to_direct_and_survive_a_round_trip() {
        assert_eq!(GameOptions::default().rewards, Rewards::Direct);
        for rewards in [Rewards::Direct, Rewards::Packages] {
            for feedback in [true, false] {
                let options = GameOptions { rewards, feedback };
                assert_eq!(GameOptions::parse(&options.code()), options);
            }
        }
        // Feedback is asked for unless it was turned off, so options kept before it
        // existed ask for it.
        assert!(GameOptions::STOCK.feedback);
        assert!(GameOptions::parse("rewards = packages").feedback);
        assert!(!GameOptions::parse("feedback = off").feedback);
        assert_eq!(GameOptions::parse(""), GameOptions::STOCK);
        assert_eq!(
            GameOptions::parse("rewards = gold\nnoise"),
            GameOptions::STOCK
        );
        assert_eq!(
            GameOptions::parse("  rewards=packages  ").rewards,
            Rewards::Packages
        );
    }

    #[test]
    fn options_are_kept_in_their_file() {
        let folder = std::env::temp_dir().join(format!("shipvivor-options-{}", std::process::id()));
        let path = folder.join("deeper").join("options.txt");
        let packages = GameOptions {
            rewards: Rewards::Packages,
            feedback: false,
        };
        keep_at(&path, &packages).unwrap();
        let kept = GameOptions::parse(&std::fs::read_to_string(&path).unwrap());
        assert_eq!(kept, packages);
        std::fs::remove_dir_all(&folder).unwrap();
    }
}
