//! Playtest feedback: four scores the player can give at the end of each round, in the yard
//! after it, and at the end of the run, won or lost. Each runs from -3 to +3, with 0 for
//! neutral. There is nothing to submit: every click rewrites a small snapshot beside the
//! run's log, `work/runs/<CODE>.feedback.toml`, which also names the screenshots taken in
//! the round it scores. It is asked only where the dev tools keep a run log, and the
//! options screen can turn it off.
use crate::state::{Campaign, Screen};
use bevy::prelude::*;
use shipvivor_campaign::{MAX_STARS, PATROLS, Stage};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// What the player is asked to score.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Metric {
    Challenge,
    Frustration,
    Engagement,
    Motivation,
}
impl Metric {
    pub const ALL: [Self; 4] = [
        Self::Challenge,
        Self::Frustration,
        Self::Engagement,
        Self::Motivation,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Challenge => "Challenge",
            Self::Frustration => "Frustration",
            Self::Engagement => "Engagement",
            Self::Motivation => "Motivation",
        }
    }
    /// The key it is written under in the snapshot.
    fn key(self) -> &'static str {
        match self {
            Self::Challenge => "challenge",
            Self::Frustration => "frustration",
            Self::Engagement => "engagement",
            Self::Motivation => "motivation",
        }
    }
    fn index(self) -> usize {
        self as usize
    }
}

/// The scores on offer: neutral, and one to three either way.
pub const SCORES: [i8; 7] = [-3, -2, -1, 0, 1, 2, 3];

/// What a set of scores is about: a round just sailed, or the whole run at its end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Moment {
    /// In the yard after a round, numbered from 1.
    Round(usize),
    /// On the after-action report, won or lost.
    Run,
}
impl Moment {
    /// The moment the screen asks about, if it asks: the yard after each round, and the
    /// report at the end of the run.
    pub fn of(run: &Campaign) -> Option<Self> {
        // A test sortie is no round of a run.
        if run.is_exercise() {
            return None;
        }
        match run.screen() {
            Screen::Drydock(_) => Some(Self::Round(run.stage.round())),
            Screen::Won(_) | Screen::Lost => Some(Self::Run),
            _ => None,
        }
    }
}

/// The scores given for one moment, with what the run stood at when they were given.
#[derive(Clone, Debug)]
struct Entry {
    moment: Moment,
    /// Lines describing the moment, written as they are: `key = value`.
    context: Vec<String>,
    scores: [Option<i8>; 4],
    /// When the first score for it was given, UTC.
    rated: String,
    screenshots: Vec<String>,
}

/// The scores given in the run under way, or the last one to end.
#[derive(Resource, Default)]
pub struct Feedback {
    /// The code of the run the scores belong to: its log's name.
    code: Option<String>,
    seed: u64,
    entries: Vec<Entry>,
    updated: String,
}
impl Feedback {
    /// The score given to `metric` for `moment` of run `code`, if any.
    pub fn score(&self, code: Option<&str>, moment: Moment, metric: Metric) -> Option<i8> {
        self.entry(code?, moment)?.scores[metric.index()]
    }
    /// Whether any score has been given for `moment` of run `code`.
    pub fn any(&self, code: Option<&str>, moment: Moment) -> bool {
        code.and_then(|code| self.entry(code, moment))
            .is_some_and(|e| e.scores.iter().any(Option::is_some))
    }
    fn entry(&self, code: &str, moment: Moment) -> Option<&Entry> {
        (self.code.as_deref() == Some(code))
            .then(|| self.entries.iter().find(|e| e.moment == moment))
            .flatten()
    }

    /// Give `metric` a score for the moment `run` is at, or take it back if it is the
    /// score already given. A new run code starts a fresh sheet. `screenshots` are the
    /// captures taken in the round the moment is about, and `now` is the time, UTC.
    pub fn rate(
        &mut self,
        code: &str,
        run: &Campaign,
        metric: Metric,
        score: i8,
        screenshots: Vec<String>,
        now: &str,
    ) -> bool {
        let Some(moment) = Moment::of(run) else {
            return false;
        };
        if self.code.as_deref() != Some(code) {
            *self = Self {
                code: Some(code.to_owned()),
                seed: run.seed,
                ..default()
            };
        }
        let at = match self.entries.iter().position(|e| e.moment == moment) {
            Some(at) => at,
            None => {
                self.entries.push(Entry {
                    moment,
                    context: Vec::new(),
                    scores: [None; 4],
                    rated: now.to_owned(),
                    screenshots: Vec::new(),
                });
                self.entries.len() - 1
            }
        };
        let entry = &mut self.entries[at];
        let slot = &mut entry.scores[metric.index()];
        *slot = (*slot != Some(score)).then_some(score);
        entry.context = context(run, moment);
        entry.screenshots = screenshots;
        self.updated = now.to_owned();
        // Rounds in order, then the run.
        self.entries.sort_by_key(|e| match e.moment {
            Moment::Round(n) => n,
            Moment::Run => usize::MAX,
        });
        true
    }

    /// The snapshot as TOML: a header on the run, a `[[round]]` table for each round
    /// scored and a `[match]` table for the run's end. A score not given is left out, and
    /// so is a moment with none.
    pub fn snapshot(&self) -> String {
        let code = self.code.as_deref().unwrap_or("");
        let mut out = String::new();
        let _ = writeln!(
            out,
            "# Shipvivor playtest feedback for run {code}, beside {code}.log.\n\
             # Rewritten at every click. Scores run from -3 to +3 and 0 is neutral;\n\
             # a score left out was not given."
        );
        let _ = writeln!(out, "code = {}", quoted(code));
        let _ = writeln!(out, "seed = {}", quoted(&self.seed.to_string()));
        let _ = writeln!(out, "build = {}", quoted(crate::ui::BUILD.unwrap_or("dev")));
        let _ = writeln!(out, "updated = {}", quoted(&self.updated));
        for entry in &self.entries {
            if entry.scores.iter().all(Option::is_none) {
                continue;
            }
            out.push('\n');
            out.push_str(match entry.moment {
                Moment::Round(_) => "[[round]]\n",
                Moment::Run => "[match]\n",
            });
            for line in &entry.context {
                let _ = writeln!(out, "{line}");
            }
            let _ = writeln!(out, "rated = {}", quoted(&entry.rated));
            for metric in Metric::ALL {
                if let Some(score) = entry.scores[metric.index()] {
                    let _ = writeln!(out, "{} = {score}", metric.key());
                }
            }
            let shots: Vec<String> = entry.screenshots.iter().map(|s| quoted(s)).collect();
            let _ = writeln!(out, "screenshots = [{}]", shots.join(", "));
        }
        out
    }

    /// Where the snapshot goes in `folder`, once a run has been scored.
    pub fn path(&self, folder: &Path) -> Option<PathBuf> {
        let code = self.code.as_deref()?;
        Some(folder.join(format!("{code}.feedback.toml")))
    }

    /// Write the snapshot into `folder`, whole: a reader never sees half of it.
    pub fn save(&self, folder: &Path) -> std::io::Result<PathBuf> {
        let path = self
            .path(folder)
            .ok_or_else(|| std::io::Error::other("no run scored"))?;
        std::fs::create_dir_all(folder)?;
        let partial = path.with_extension("toml.partial");
        std::fs::write(&partial, self.snapshot())?;
        std::fs::rename(&partial, &path)?;
        Ok(path)
    }
}

/// What the run stood at, as `key = value` lines for the moment's table.
fn context(run: &Campaign, moment: Moment) -> Vec<String> {
    let sea = quoted(run.stage.mission().name);
    match moment {
        Moment::Round(number) => vec![
            format!("number = {number}"),
            format!("sea = {sea}"),
            format!("stars = {}", run.stars()),
        ],
        Moment::Run => {
            let outcome = match run.screen() {
                Screen::Won(_) => "won",
                Screen::Lost if run.ship.afloat() && run.stage == Stage::CrownDeep => {
                    "regent escaped"
                }
                _ => "sunk",
            };
            // Stars out of every patrol sailed, the one she ended on included, as the
            // report counts them.
            let sailed = run.stage.round().min(PATROLS) as u8;
            vec![
                format!("outcome = {}", quoted(outcome)),
                format!("round = {}", run.stage.round()),
                format!("sea = {sea}"),
                format!("stars = {}", run.previous_stars + run.stars()),
                format!("possible = {}", sailed * MAX_STARS),
            ]
        }
    }
}

/// A TOML basic string.
fn quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04X}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Mode;
    use shipvivor_campaign::{Objectives, Phase};

    /// A run in the yard after its first round, with every base down.
    fn in_the_yard() -> Campaign {
        let mut run = Campaign::new(Mode::Patrol);
        run.seed = 42;
        run.tick_sector(1., Objectives::default(), 0, true);
        while run.fittings_remaining() > 0 {
            let card = run.offers()[0];
            run.install(card);
        }
        assert!(matches!(run.phase(), Phase::Drydock(_)));
        run
    }

    #[test]
    fn the_yard_asks_about_its_round_and_the_report_about_the_run() {
        let run = in_the_yard();
        assert_eq!(Moment::of(&run), Some(Moment::Round(1)));
        assert_eq!(Moment::of(&Campaign::new(Mode::Patrol)), None);
        assert_eq!(Moment::of(&Campaign::new(Mode::Title)), None);
    }

    #[test]
    fn a_click_scores_and_a_second_click_on_the_same_score_takes_it_back() {
        let run = in_the_yard();
        let mut sheet = Feedback::default();
        let round = Moment::Round(1);
        assert!(sheet.rate("ABCDEFG", &run, Metric::Challenge, 2, vec![], "then"));
        assert_eq!(
            sheet.score(Some("ABCDEFG"), round, Metric::Challenge),
            Some(2)
        );
        assert_eq!(
            sheet.score(Some("ABCDEFG"), round, Metric::Frustration),
            None
        );
        assert!(sheet.any(Some("ABCDEFG"), round));
        // Another score replaces it, and the same one again clears it.
        sheet.rate("ABCDEFG", &run, Metric::Challenge, -3, vec![], "then");
        assert_eq!(
            sheet.score(Some("ABCDEFG"), round, Metric::Challenge),
            Some(-3)
        );
        sheet.rate("ABCDEFG", &run, Metric::Challenge, -3, vec![], "then");
        assert_eq!(sheet.score(Some("ABCDEFG"), round, Metric::Challenge), None);
        assert!(!sheet.any(Some("ABCDEFG"), round));
        // Neutral is a score of its own, not a blank.
        sheet.rate("ABCDEFG", &run, Metric::Motivation, 0, vec![], "then");
        assert_eq!(
            sheet.score(Some("ABCDEFG"), round, Metric::Motivation),
            Some(0)
        );
        // Another run's sheet shows nothing of this one's, and starts afresh.
        assert_eq!(
            sheet.score(Some("ZZZZZZZ"), round, Metric::Motivation),
            None
        );
        sheet.rate("ZZZZZZZ", &run, Metric::Engagement, 1, vec![], "later");
        assert_eq!(
            sheet.score(Some("ABCDEFG"), round, Metric::Motivation),
            None
        );
        assert_eq!(
            sheet.score(Some("ZZZZZZZ"), round, Metric::Engagement),
            Some(1)
        );
        // Nothing is asked away from the yard and the report.
        assert!(!sheet.rate(
            "ZZZZZZZ",
            &Campaign::new(Mode::Patrol),
            Metric::Challenge,
            1,
            vec![],
            "x"
        ));
    }

    #[test]
    fn the_snapshot_is_toml_with_a_table_per_moment_scored() {
        let run = in_the_yard();
        let mut sheet = Feedback::default();
        let shots = vec!["work/screenshots/a.png".to_string()];
        sheet.rate("ABCDEFG", &run, Metric::Challenge, 1, shots.clone(), "t1");
        sheet.rate("ABCDEFG", &run, Metric::Frustration, -2, shots, "t2");
        let text = sheet.snapshot();
        for line in [
            "code = \"ABCDEFG\"",
            "seed = \"42\"",
            "updated = \"t2\"",
            "[[round]]",
            "number = 1",
            "sea = \"The Iron Narrows\"",
            "rated = \"t1\"",
            "challenge = 1",
            "frustration = -2",
            "screenshots = [\"work/screenshots/a.png\"]",
        ] {
            assert!(text.lines().any(|l| l == line), "no `{line}` in\n{text}");
        }
        assert!(!text.contains("engagement"), "{text}");
        assert!(!text.contains("[match]"), "{text}");
        // A moment whose every score was taken back leaves no table.
        sheet.rate("ABCDEFG", &run, Metric::Challenge, 1, vec![], "t3");
        sheet.rate("ABCDEFG", &run, Metric::Frustration, -2, vec![], "t3");
        assert!(!sheet.snapshot().contains("[[round]]"));
    }

    #[test]
    fn the_end_of_a_lost_run_is_scored_under_match() {
        let mut run = in_the_yard();
        run.sail_from_dock();
        // The moderate passage is always open.
        let stage = run.exits().unwrap()[0].stage;
        assert!(run.depart(stage));
        run.tick(1., Objectives::default(), u16::MAX);
        assert_eq!(run.screen(), Screen::Lost);
        let mut sheet = Feedback::default();
        sheet.rate("ABCDEFG", &run, Metric::Engagement, 3, vec![], "t");
        let text = sheet.snapshot();
        assert!(
            text.contains("[match]\noutcome = \"sunk\"\nround = 2\n"),
            "{text}"
        );
        assert!(text.contains("engagement = 3"), "{text}");
    }

    #[test]
    fn the_snapshot_is_written_beside_the_log_and_rewritten_whole() {
        let folder =
            std::env::temp_dir().join(format!("shipvivor-feedback-{}", std::process::id()));
        let run = in_the_yard();
        let mut sheet = Feedback::default();
        assert!(
            sheet.save(&folder).is_err(),
            "nothing scored, nothing to write"
        );
        sheet.rate("ABCDEFG", &run, Metric::Challenge, 1, vec![], "t");
        let path = sheet.save(&folder).unwrap();
        assert_eq!(path, folder.join("ABCDEFG.feedback.toml"));
        sheet.rate("ABCDEFG", &run, Metric::Challenge, 3, vec![], "t");
        sheet.save(&folder).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("challenge = 3") && !text.contains("challenge = 1"));
        assert!(!folder.join("ABCDEFG.feedback.toml.partial").exists());
        std::fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn strings_are_escaped() {
        assert_eq!(quoted("a\"b\\c\n"), "\"a\\\"b\\\\c\\n\"");
    }
}
