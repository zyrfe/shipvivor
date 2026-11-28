//! The crew data the game plays with: `data/crew.toml`, read at launch and read again
//! whenever it is saved, so skills and officers can be tuned while the game runs. A
//! file that has a problem is ignored, with every problem logged, and the game keeps
//! what it had.
use bevy::prelude::*;
use shipvivor_campaign::crew::Wardroom;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// How often the file is checked for a save, in seconds.
const CHECK_EVERY: f32 = 1.;

pub struct CrewBookPlugin;
impl Plugin for CrewBookPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CrewBook::open())
            .add_systems(Update, reread);
    }
}

/// The wardroom in play, and the file it came from.
#[derive(Resource)]
pub struct CrewBook {
    pub wardroom: Wardroom,
    /// The file being watched, or `None` when playing the copy built into the game.
    file: Option<PathBuf>,
    /// When the file was last saved, as of the last read.
    saved: Option<SystemTime>,
    since_check: f32,
}

impl CrewBook {
    fn open() -> Self {
        let mut book = Self {
            wardroom: Wardroom::stock(),
            file: if cfg!(test) { None } else { find() },
            saved: None,
            since_check: 0.,
        };
        match book.file.clone() {
            Some(file) => book.read(&file),
            None => info!(
                "Crew data built into the game ({}): no data/crew.toml found",
                book.tally()
            ),
        }
        book
    }

    /// What the book holds, for the log.
    fn tally(&self) -> String {
        format!(
            "{} officers, {} skills",
            self.wardroom.officers.len(),
            self.wardroom.skills().len()
        )
    }

    /// Reads the file into the book, and logs what came of it.
    fn read(&mut self, file: &Path) {
        self.saved = saved(file);
        let text = match std::fs::read_to_string(file) {
            Ok(text) => text,
            Err(err) => {
                warn!("Could not read {}: {err}", file.display());
                return;
            }
        };
        match Wardroom::parse(&text) {
            Ok(wardroom) => {
                self.wardroom = wardroom;
                info!("Crew data from {} ({})", file.display(), self.tally());
            }
            Err(problems) => {
                warn!(
                    "{} not taken, the game keeps what it had:\n{problems}",
                    file.display()
                );
            }
        }
    }
}

/// Where the crew file is: the file `SHIPVIVOR_CREW_FILE` names, then `data/crew.toml`
/// beside the executable, then in the folder the game was started from.
fn find() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SHIPVIVOR_CREW_FILE") {
        return Some(path.into());
    }
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.join("data/crew.toml")));
    let here = Some(PathBuf::from("data/crew.toml"));
    [beside, here]
        .into_iter()
        .flatten()
        .find(|path| path.is_file())
}

fn saved(file: &Path) -> Option<SystemTime> {
    std::fs::metadata(file)
        .and_then(|meta| meta.modified())
        .ok()
}

fn reread(time: Res<Time<Real>>, mut book: ResMut<CrewBook>) {
    book.check(time.delta_secs());
}

impl CrewBook {
    /// Counts `seconds` toward the next look at the file, and rereads it if it has been
    /// saved since the last read. A broken save is read once, not every second.
    fn check(&mut self, seconds: f32) {
        self.since_check += seconds;
        if self.since_check < CHECK_EVERY {
            return;
        }
        self.since_check = 0.;
        let Some(file) = self.file.clone() else {
            return;
        };
        let now = saved(&file);
        if now.is_some() && now != self.saved {
            self.read(&file);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shipvivor_campaign::crew::{STOCK, Skill};
    use std::time::Duration;

    /// Writes `text` to `file`, stamped as saved `at` seconds into the epoch.
    fn save(file: &Path, text: &str, at: u64) {
        std::fs::write(file, text).unwrap();
        std::fs::File::options()
            .write(true)
            .open(file)
            .unwrap()
            .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(at))
            .unwrap();
    }

    fn fury_seconds(book: &CrewBook) -> f32 {
        book.wardroom.skill(Skill::GunFury).number(1, "seconds")
    }

    #[test]
    fn a_saved_file_is_reread_and_a_broken_one_is_ignored() {
        let folder = std::env::temp_dir().join(format!("shipvivor-crew-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let file = folder.join("crew.toml");
        let fury = "{ seconds = 10, reload = 0.5, cooldown = 60 }";
        assert!(STOCK.contains(fury));
        save(
            &file,
            &STOCK.replacen(fury, &fury.replace("10", "11"), 1),
            1_000,
        );
        let mut book = CrewBook {
            wardroom: Wardroom::stock(),
            file: Some(file.clone()),
            saved: None,
            since_check: 0.,
        };
        book.check(CHECK_EVERY);
        assert_eq!(fury_seconds(&book), 11.);

        // A save is looked for once a second, not every frame.
        save(
            &file,
            &STOCK.replacen(fury, &fury.replace("10", "12"), 1),
            2_000,
        );
        book.check(CHECK_EVERY * 0.5);
        assert_eq!(fury_seconds(&book), 11.);
        book.check(CHECK_EVERY * 0.5);
        assert_eq!(fury_seconds(&book), 12.);

        // A broken save keeps what the game had.
        save(&file, "[growth\nexperience = [100]", 3_000);
        book.check(CHECK_EVERY);
        assert_eq!(fury_seconds(&book), 12.);
        assert_eq!(book.saved, saved(&file));

        std::fs::remove_dir_all(&folder).unwrap();
    }
}
