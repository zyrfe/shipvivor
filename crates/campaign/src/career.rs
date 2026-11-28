//! A career: one captain carried from run to run with the tiers their skills have
//! reached, and the points runs have paid to raise them. The captain's skills work
//! ship-wide on every run they command, through the [`CaptainPerks`] the run carries.
use crate::crew::{Captain, CareerRules, Skill, Wardroom};
use crate::{Phase, Run, StartingHull};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What a captain's skills do on a run, read from the crew data when the run starts.
/// All zero for a run with no captain.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CaptainPerks {
    /// Yard points more every patrol.
    pub yard_points: u16,
    /// Seconds more on every patrol's clock.
    pub clock_seconds: f32,
    /// Chance a hit on her glances off and does nothing.
    pub glance: f32,
    /// Share more experience for everything that pays it.
    pub experience: f32,
    /// How far leader hands lean rarer: see [`crate::crew::Hand::weight_leaning`].
    pub hand_lean: f32,
    /// Share more repair from salvaged stores.
    pub stores_repair: f32,
}

impl CaptainPerks {
    /// The perks of a captain holding `skills`, each at its tier, as `wardroom` has them.
    pub fn of(wardroom: &Wardroom, skills: &[(Skill, u8)]) -> Self {
        let mut perks = Self::default();
        for &(skill, tier) in skills {
            let book = wardroom.skill(skill);
            let n = |name| book.number(tier, name);
            match skill {
                Skill::Quartermaster => perks.yard_points += n("points").max(0.).round() as u16,
                Skill::Renown => perks.clock_seconds += n("seconds"),
                Skill::Lucky => perks.glance += n("glance"),
                Skill::OldHand => perks.experience += n("experience"),
                Skill::Reputation => perks.hand_lean += n("lean"),
                Skill::Provisioner => perks.stores_repair += n("repair"),
                _ => {}
            }
        }
        perks.glance = perks.glance.clamp(0., 1.);
        perks
    }
}

/// The captain in command of a run.
#[derive(Clone, Debug, PartialEq)]
pub struct CaptainAboard {
    /// The captain's id in the crew data.
    pub id: String,
    pub name: String,
    pub skills: Vec<(Skill, u8)>,
    pub perks: CaptainPerks,
}

/// How a career run begins: in which hull, under which captain.
#[derive(Clone, Debug, PartialEq)]
pub struct CareerStart {
    pub hull: StartingHull,
    pub captain: CaptainAboard,
}

impl CareerStart {
    /// `run` as this start sets her out.
    #[must_use]
    pub fn board(self, run: Run) -> Run {
        let mut run = run.with_hull(self.hull);
        run.captain = Some(self.captain);
        run
    }
}

/// One captain's career, kept between runs.
#[derive(Clone, Debug, PartialEq)]
pub struct Career {
    /// The captain's id in the crew data.
    pub captain: String,
    pub name: String,
    /// The captain's skills at the tiers they have reached, in [`Skill::ALL`] order.
    pub skills: Vec<(Skill, u8)>,
    /// Career points earned and not yet spent.
    pub points: u32,
    /// Runs commanded, won or lost.
    pub runs: u32,
    /// Runs that sank the Iron Regent.
    pub wins: u32,
    /// Stars earned over every run.
    pub stars: u32,
    /// What the last run commanded has paid so far, so a run rewound and ended again
    /// pays only what it adds.
    pub last_run: Option<Paid>,
}

/// What one run has paid a career.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Paid {
    /// The run's seed, which tells one run from another.
    pub seed: u64,
    pub points: u32,
    pub stars: u32,
    pub won: bool,
}

impl Career {
    /// A new career for `captain`, at the tiers they start with.
    pub fn new(captain: &Captain) -> Self {
        Self {
            captain: captain.id.clone(),
            name: captain.name.clone(),
            skills: captain.skills.clone(),
            points: 0,
            runs: 0,
            wins: 0,
            stars: 0,
            last_run: None,
        }
    }

    /// The tier `skill` stands at, or `None` if the captain lacks it.
    pub fn tier(&self, skill: Skill) -> Option<u8> {
        self.skills
            .iter()
            .find(|&&(s, _)| s == skill)
            .map(|&(_, tier)| tier)
    }

    /// Career points to raise `skill` a tier, or `None` if the captain lacks it or it is
    /// at its last tier.
    pub fn next_cost(&self, skill: Skill, wardroom: &Wardroom) -> Option<u32> {
        let tier = self.tier(skill)?;
        if usize::from(tier) >= wardroom.skill(skill).tiers.len() {
            return None;
        }
        wardroom.career.cost(tier + 1)
    }

    /// Raise `skill` a tier with career points. False, and nothing spent, if it can't be
    /// raised or the points don't cover it.
    pub fn raise(&mut self, skill: Skill, wardroom: &Wardroom) -> bool {
        let Some(cost) = self.next_cost(skill, wardroom) else {
            return false;
        };
        if cost > self.points {
            return false;
        }
        self.points -= cost;
        for entry in &mut self.skills {
            if entry.0 == skill {
                entry.1 += 1;
            }
        }
        true
    }

    /// The captain as they come aboard a run, with what their skills do.
    pub fn aboard(&self, wardroom: &Wardroom) -> CaptainAboard {
        CaptainAboard {
            id: self.captain.clone(),
            name: self.name.clone(),
            skills: self.skills.clone(),
            perks: CaptainPerks::of(wardroom, &self.skills),
        }
    }

    /// Career points `run` pays as it stands: its stars, and more for a win.
    pub fn earned(run: &Run, rules: &CareerRules) -> u32 {
        let stars = u32::from(run.previous_stars) + u32::from(run.stars());
        let win = matches!(run.phase(), Phase::Won(_));
        stars * rules.points_per_star + if win { rules.points_for_a_win } else { 0 }
    }

    /// Bank what `run` has paid, once it is over and if this captain commanded it.
    /// A run rewound after it ended and ended again banks only what it adds; it is never
    /// counted twice. Returns the points banked, or `None` if there was nothing new.
    pub fn settle(&mut self, run: &Run, rules: &CareerRules) -> Option<u32> {
        let over = matches!(run.phase(), Phase::Won(_) | Phase::Lost);
        let ours = run.captain.as_ref().is_some_and(|c| c.id == self.captain);
        if !over || !ours {
            return None;
        }
        let now = Paid {
            seed: run.seed,
            points: Self::earned(run, rules),
            stars: u32::from(run.previous_stars) + u32::from(run.stars()),
            won: matches!(run.phase(), Phase::Won(_)),
        };
        let before = match self.last_run {
            Some(paid) if paid.seed == run.seed => paid,
            _ => {
                self.runs += 1;
                Paid {
                    seed: run.seed,
                    points: 0,
                    stars: 0,
                    won: false,
                }
            }
        };
        let points = now.points.saturating_sub(before.points);
        self.points += points;
        self.stars += now.stars.saturating_sub(before.stars);
        self.wins += u32::from(now.won && !before.won);
        self.last_run = Some(Paid {
            points: now.points.max(before.points),
            stars: now.stars.max(before.stars),
            won: now.won || before.won,
            ..now
        });
        (points > 0).then_some(points)
    }

    /// The career as the save file keeps it.
    pub fn code(&self) -> String {
        let kept = Kept {
            captain: self.captain.clone(),
            name: self.name.clone(),
            points: self.points,
            runs: self.runs,
            wins: self.wins,
            stars: self.stars,
            last_run: self.last_run.map(|paid| LastRun {
                seed: format!("{:016x}", paid.seed),
                points: paid.points,
                stars: paid.stars,
                won: paid.won,
            }),
            skills: self
                .skills
                .iter()
                .map(|&(skill, tier)| (skill.id().to_string(), tier))
                .collect(),
        };
        toml::to_string(&kept).unwrap_or_default()
    }

    /// A career from its save file, checked against `wardroom`: a captain or a skill the
    /// crew data no longer has is refused, and a tier past a skill's last is brought
    /// down to it.
    ///
    /// # Errors
    /// Why the file can't be read as a career.
    pub fn parse(text: &str, wardroom: &Wardroom) -> Result<Self, String> {
        let kept: Kept = toml::from_str(text).map_err(|err| err.to_string())?;
        if wardroom.captain(&kept.captain).is_none() {
            return Err(format!(
                "no captain called {} in the crew data",
                kept.captain
            ));
        }
        let mut skills = Vec::new();
        for (id, tier) in kept.skills {
            let skill = Skill::from_id(&id).ok_or_else(|| format!("no skill called {id}"))?;
            if !skill.captain() {
                return Err(format!("{id} is not a captain's skill"));
            }
            let most = wardroom.skill(skill).tiers.len() as u8;
            skills.push((skill, tier.clamp(1, most)));
        }
        skills.sort();
        let last_run = match kept.last_run {
            Some(last) => Some(Paid {
                seed: u64::from_str_radix(&last.seed, 16)
                    .map_err(|_| format!("last_run.seed {} is not a run's seed", last.seed))?,
                points: last.points,
                stars: last.stars,
                won: last.won,
            }),
            None => None,
        };
        Ok(Self {
            last_run,
            captain: kept.captain,
            name: kept.name,
            skills,
            points: kept.points,
            runs: kept.runs,
            wins: kept.wins,
            stars: kept.stars,
        })
    }
}

/// A career as it is written to its file.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Kept {
    captain: String,
    name: String,
    points: u32,
    runs: u32,
    wins: u32,
    stars: u32,
    skills: BTreeMap<String, u8>,
    last_run: Option<LastRun>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LastRun {
    /// In hex: a seed runs past what a TOML integer holds.
    seed: String,
    points: u32,
    stars: u32,
    won: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LegEnd;

    fn ashby(wardroom: &Wardroom) -> Career {
        Career::new(wardroom.captain("ashby").unwrap())
    }

    #[test]
    fn a_career_raises_its_captains_skills_with_points_and_survives_its_file() {
        let wardroom = Wardroom::stock();
        let mut career = ashby(&wardroom);
        assert_eq!(career.tier(Skill::Lucky), Some(1));
        assert_eq!(career.next_cost(Skill::Lucky, &wardroom), Some(6));
        // A skill the captain lacks can't be raised, nor one the points don't cover.
        assert!(!career.raise(Skill::OldHand, &wardroom));
        assert!(!career.raise(Skill::Lucky, &wardroom));
        career.points = 20;
        assert!(career.raise(Skill::Lucky, &wardroom));
        assert!(career.raise(Skill::Lucky, &wardroom));
        assert_eq!((career.tier(Skill::Lucky), career.points), (Some(3), 2));
        assert_eq!(career.next_cost(Skill::Lucky, &wardroom), None);
        assert!(!career.raise(Skill::Lucky, &wardroom));

        let kept = Career::parse(&career.code(), &wardroom).unwrap();
        assert_eq!(kept, career);
        // A file naming a captain the data has dropped is refused, not guessed at.
        let gone = career.code().replace("ashby", "nobody");
        assert!(Career::parse(&gone, &wardroom).is_err());
        assert!(Career::parse("captain = 3", &wardroom).is_err());
    }

    #[test]
    fn perks_follow_the_tiers_and_a_run_pays_its_captain_once() {
        let wardroom = Wardroom::stock();
        let mut career = ashby(&wardroom);
        let perks = career.aboard(&wardroom).perks;
        assert_eq!((perks.glance, perks.clock_seconds), (0.04, 20.));
        assert_eq!(perks.yard_points, 0);

        let start = CareerStart {
            hull: StartingHull::HeavyDestroyer,
            captain: career.aboard(&wardroom),
        };
        let mut run = start.board(Run::default());
        assert_eq!(run.hull, StartingHull::HeavyDestroyer);
        let clock = Run::default().clock();
        assert_eq!(run.clock(), clock + 20.);
        run.seed = u64::MAX - 1;
        // Nothing is banked while the run goes on.
        assert_eq!(career.settle(&run, &wardroom.career), None);
        // Lost, rewound and sailed on to a win: the loss pays, then only what the win adds.
        run.previous_stars = 5;
        run.phase = Phase::Lost;
        assert_eq!(career.settle(&run, &wardroom.career), Some(5));
        assert_eq!(career.settle(&run, &wardroom.career), None);
        run.previous_stars = 7;
        run.phase = Phase::Won(LegEnd::RegentSunk);
        assert_eq!(career.settle(&run, &wardroom.career), Some(2 + 5));
        assert_eq!(
            (career.points, career.runs, career.wins, career.stars),
            (12, 1, 1, 7)
        );
        // The file keeps what the last run paid, seed and all.
        assert_eq!(Career::parse(&career.code(), &wardroom).unwrap(), career);
        // The next run is a new one.
        run.seed = 3;
        assert_eq!(career.settle(&run, &wardroom.career), Some(12));
        assert_eq!(career.runs, 2);
        // Another captain's run pays this one nothing.
        let mut other = Run::default().with_captain_named("thorne");
        other.phase = Phase::Lost;
        assert_eq!(career.settle(&other, &wardroom.career), None);
    }

    #[test]
    fn luck_turns_hits_and_an_old_hand_earns_more_experience() {
        let lucky = CaptainPerks {
            glance: 1.,
            experience: 0.5,
            ..CaptainPerks::default()
        };
        let mut run = Run::default().with_captain_named("ashby");
        run.captain.as_mut().unwrap().perks = lucky;
        let hull = run.hull();
        run.strike(17., 12);
        assert_eq!(run.hull(), hull, "every hit glances");
        // Five experience a torpedo boat, half again: 7, then the half carried makes 8.
        run.sink_torpedo_boats(1);
        assert_eq!(run.experience, 7);
        run.sink_torpedo_boats(1);
        assert_eq!(run.experience, 15);

        // Without a captain, a hit lands and experience is as stock.
        let mut plain = Run::default();
        plain.strike(17., 12);
        assert!(plain.hull() < hull);
        plain.sink_torpedo_boats(1);
        assert_eq!(plain.experience, 5);
    }

    impl Run {
        fn with_captain_named(mut self, id: &str) -> Self {
            self.captain = Some(CaptainAboard {
                id: id.into(),
                name: id.into(),
                skills: vec![],
                perks: CaptainPerks::default(),
            });
            self
        }
    }
}
