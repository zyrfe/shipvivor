//! Breakwater's wardroom as data: the posts officers hold, the skills they bring and each
//! skill's tiers, the officers who can be dealt as leader cards, how a skill ranks up,
//! and the odds of a leader hand. It all comes from `data/crew.toml`, so it can be tuned
//! without a rebuild. The game knows only each [`Skill`]'s id, the numbers its effect
//! reads, and what its experience counts.
use crate::Rarity;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt;

/// The wardroom the game ships with, built into it.
pub const STOCK: &str = include_str!("../../../data/crew.toml");

/// A skill an officer can bring. Its numbers, tiers and card text are data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Skill {
    GunFury,
    SpottingCorrections,
    RatePlot,
    RadarOperator,
    TargetIdentification,
    RangeBoost,
    FallOfShot,
    ForcedDraught,
    SteamHeld,
    SmokeScreen,
    HardOver,
    SecondRepairCrew,
    DrilledParty,
    Shoring,
    Counterflooding,
    BomberStrike,
    ReconFlight,
    FleetGunfire,
    FighterCover,
}

impl Skill {
    pub const ALL: [Self; 19] = [
        Self::GunFury,
        Self::SpottingCorrections,
        Self::RatePlot,
        Self::RadarOperator,
        Self::TargetIdentification,
        Self::RangeBoost,
        Self::FallOfShot,
        Self::ForcedDraught,
        Self::SteamHeld,
        Self::SmokeScreen,
        Self::HardOver,
        Self::SecondRepairCrew,
        Self::DrilledParty,
        Self::Shoring,
        Self::Counterflooding,
        Self::BomberStrike,
        Self::ReconFlight,
        Self::FleetGunfire,
        Self::FighterCover,
    ];

    /// Its name in the data file.
    pub fn id(self) -> &'static str {
        match self {
            Self::GunFury => "gun_fury",
            Self::SpottingCorrections => "spotting_corrections",
            Self::RatePlot => "rate_plot",
            Self::RadarOperator => "radar_operator",
            Self::TargetIdentification => "target_identification",
            Self::RangeBoost => "range_boost",
            Self::FallOfShot => "fall_of_shot",
            Self::ForcedDraught => "forced_draught",
            Self::SteamHeld => "steam_held",
            Self::SmokeScreen => "smoke_screen",
            Self::HardOver => "hard_over",
            Self::SecondRepairCrew => "second_repair_crew",
            Self::DrilledParty => "drilled_party",
            Self::Shoring => "shoring",
            Self::Counterflooding => "counterflooding",
            Self::BomberStrike => "bomber_strike",
            Self::ReconFlight => "recon_flight",
            Self::FleetGunfire => "fleet_gunfire",
            Self::FighterCover => "fighter_cover",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|skill| skill.id() == id)
    }

    /// The numbers every tier must give: what the skill's effect reads. A `true` or
    /// `false` in the file reads as 1 or 0.
    pub fn numbers(self) -> &'static [&'static str] {
        match self {
            Self::GunFury => &["seconds", "reload", "cooldown"],
            Self::SpottingCorrections => &["per_salvo", "most"],
            Self::RatePlot => &["scatter"],
            Self::RadarOperator => &["range", "lay_guns"],
            Self::TargetIdentification => &["show_aim", "damage"],
            Self::RangeBoost => &["reach"],
            Self::FallOfShot => &["every_shell", "torpedoes"],
            Self::ForcedDraught => &["speed"],
            Self::SteamHeld => &["power"],
            Self::SmokeScreen => &["seconds", "cooldown"],
            Self::HardOver => &["rudder"],
            Self::SecondRepairCrew => &["power"],
            Self::DrilledParty => &["repairs"],
            Self::Shoring => &["restore"],
            Self::Counterflooding => &["drag"],
            Self::BomberStrike => &["bombers", "per_round", "pick_target"],
            Self::ReconFlight => &["hidden_fleet", "spotter"],
            Self::FleetGunfire => &["per_round", "barrels", "bore"],
            Self::FighterCover => &["fighters", "every_raid"],
        }
    }

    /// What one unit of the skill's `experience` rate is earned for, or `None` where
    /// nothing it does can be credited to it, so it ranks up from officer points alone.
    pub fn earns(self) -> Option<&'static str> {
        match self {
            Self::GunFury => Some("point of damage dealt while a fury is on"),
            Self::SpottingCorrections => Some("hit from a corrected salvo"),
            Self::RatePlot => Some("hit on a ship making over half her speed"),
            Self::RadarOperator => Some("point of damage to a ship first seen on radar"),
            Self::TargetIdentification => Some("point of damage to the marked threat"),
            Self::RangeBoost => Some("hit beyond the weapon's stock reach"),
            Self::SteamHeld => Some("second steamed on a cooked engine"),
            Self::SmokeScreen => Some("enemy round that misses her through smoke"),
            Self::SecondRepairCrew => Some("hull point the second crew mends"),
            Self::DrilledParty => Some("hull point mended by steady repairs"),
            Self::Shoring => Some("knocked-out part brought back"),
            Self::Counterflooding => Some("tonne of water pumped out"),
            Self::BomberStrike => Some("point of damage the bombers do to port facilities"),
            Self::FleetGunfire => Some("point of damage the salvos do"),
            Self::FighterCover => Some("plane the fighters bring down"),
            Self::FallOfShot | Self::ForcedDraught | Self::HardOver | Self::ReconFlight => None,
        }
    }
}

/// Everything wrong with a crew file, one problem a line.
#[derive(Debug, Clone, PartialEq)]
pub struct Problems(pub Vec<String>);

impl fmt::Display for Problems {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.join("\n"))
    }
}

impl std::error::Error for Problems {}

/// How skills rank up.
#[derive(Debug, Clone, PartialEq)]
pub struct Growth {
    /// Experience to reach each tier past the first.
    pub experience: Vec<u32>,
    /// Officer points each star of a leg pays.
    pub points_per_star: u32,
    /// Experience one officer point buys.
    pub experience_per_point: u32,
}

impl Growth {
    /// The most tiers any skill can have.
    pub fn most_tiers(&self) -> usize {
        self.experience.len() + 1
    }

    /// The tier, from one, that `experience` reaches, before any skill's own cap.
    pub fn tier(&self, experience: u32) -> u8 {
        1 + self
            .experience
            .iter()
            .filter(|&&need| experience >= need)
            .count() as u8
    }

    /// The experience a skill holds on reaching `tier`, so one that starts there has it.
    pub fn floor(&self, tier: u8) -> u32 {
        match tier {
            0 | 1 => 0,
            _ => self.experience[usize::from(tier) - 2],
        }
    }
}

/// The odds of a leader hand after a leg.
#[derive(Debug, Clone, PartialEq)]
pub struct Hand {
    pub cards: usize,
    chance: Vec<f32>,
    weights: Vec<(Rarity, Vec<f32>)>,
}

impl Hand {
    /// The chance of a leader hand after a leg of `stars`. Stars past the table use its
    /// last entry.
    pub fn chance(&self, stars: u8) -> f32 {
        by_stars(&self.chance, stars)
    }

    /// How often a card of `rarity` is drawn after a leg of `stars`, against the others.
    pub fn weight(&self, rarity: Rarity, stars: u8) -> f32 {
        self.weights
            .iter()
            .find(|(r, _)| *r == rarity)
            .map_or(0., |(_, table)| by_stars(table, stars))
    }
}

fn by_stars(table: &[f32], stars: u8) -> f32 {
    table[usize::from(stars).min(table.len() - 1)]
}

/// A post one officer holds.
#[derive(Debug, Clone, PartialEq)]
pub struct Post {
    pub id: String,
    pub name: String,
    pub colour: [u8; 3],
}

/// One stripe of gold lace on a shoulder board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lace {
    /// The broad band a commodore wears.
    Broad,
    /// The ordinary stripe.
    Wide,
    /// The thin stripe set between two wide ones.
    Thin,
}

/// An officer's rank. It follows the skill tiers the officer holds, all skills together,
/// so it climbs as their skills rank up.
#[derive(Debug, Clone, PartialEq)]
pub struct Rank {
    pub name: String,
    /// Written before the name: `Lt. Cdr.`.
    pub short: String,
    /// The stripes on the shoulder board, from the outer end in. The post's colour shows
    /// between them.
    pub lace: Vec<Lace>,
    /// Skill tiers held, all skills together, to wear it.
    pub tiers: u32,
}

/// A skill as the data has it.
#[derive(Debug, Clone, PartialEq)]
pub struct SkillBook {
    pub skill: Skill,
    pub name: String,
    /// Set off with a key, rather than always working.
    pub key: bool,
    /// Experience for each unit of [`Skill::earns`].
    pub experience: f32,
    pub tiers: Vec<Tier>,
}

impl SkillBook {
    /// The tier, from one, that `experience` reaches, capped at this skill's last.
    pub fn tier_at(&self, growth: &Growth, experience: u32) -> u8 {
        growth.tier(experience).min(self.tiers.len() as u8)
    }

    /// Tier `tier`, counting from one. A tier past the last gives the last.
    pub fn tier(&self, tier: u8) -> &Tier {
        &self.tiers[usize::from(tier.max(1) - 1).min(self.tiers.len() - 1)]
    }

    /// One of the skill's [`Skill::numbers`] at `tier`.
    ///
    /// # Panics
    /// If `name` is not one of them: that is a mistake in the game, not the data.
    pub fn number(&self, tier: u8, name: &str) -> f32 {
        let at = self
            .skill
            .numbers()
            .iter()
            .position(|&n| n == name)
            .unwrap_or_else(|| panic!("{} has no number `{name}`", self.skill.id()));
        self.tier(tier).values[at]
    }

    /// Whether one of its numbers is set at `tier`: true, or anything but zero.
    pub fn flag(&self, tier: u8, name: &str) -> bool {
        self.number(tier, name) != 0.
    }
}

/// One tier of a skill.
#[derive(Debug, Clone, PartialEq)]
pub struct Tier {
    /// The card's line for this tier, numbers filled in.
    pub text: String,
    /// In the order of [`Skill::numbers`].
    values: Vec<f32>,
}

/// An officer who can be dealt as a leader card.
#[derive(Debug, Clone, PartialEq)]
pub struct Officer {
    pub id: String,
    pub name: String,
    /// Into [`Wardroom::posts`].
    pub post: usize,
    pub rarity: Rarity,
    pub portrait: Option<String>,
    /// The skills the officer brings and the tier each starts at, in [`Skill::ALL`] order.
    pub skills: Vec<(Skill, u8)>,
}

impl Officer {
    /// The skill tiers the officer joins with, all skills together.
    pub fn tiers(&self) -> u32 {
        self.skills.iter().map(|&(_, tier)| u32::from(tier)).sum()
    }
}

/// The whole crew file, read and checked.
#[derive(Debug, Clone, PartialEq)]
pub struct Wardroom {
    pub growth: Growth,
    pub hand: Hand,
    pub posts: Vec<Post>,
    /// Ranks from the lowest up.
    pub ladder: Vec<Rank>,
    /// One for every skill, in [`Skill::ALL`] order.
    skills: Vec<SkillBook>,
    pub officers: Vec<Officer>,
}

impl Wardroom {
    /// The wardroom the game ships with.
    ///
    /// # Panics
    /// If the built-in file has a problem, which its test rules out.
    pub fn stock() -> Self {
        Self::parse(STOCK).unwrap_or_else(|problems| panic!("data/crew.toml:\n{problems}"))
    }

    pub fn skill(&self, skill: Skill) -> &SkillBook {
        &self.skills[Skill::ALL.iter().position(|&s| s == skill).unwrap_or(0)]
    }

    pub fn skills(&self) -> &[SkillBook] {
        &self.skills
    }

    pub fn officer(&self, id: &str) -> Option<&Officer> {
        self.officers.iter().find(|officer| officer.id == id)
    }

    pub fn post(&self, officer: &Officer) -> &Post {
        &self.posts[officer.post]
    }

    /// The rank of an officer holding `tiers` skill tiers, all skills together: the
    /// highest they have the tiers for, or the lowest.
    pub fn rank(&self, tiers: u32) -> &Rank {
        self.ladder
            .iter()
            .rev()
            .find(|rank| tiers >= rank.tiers)
            .unwrap_or(&self.ladder[0])
    }

    /// Reads a crew file, and checks everything the game would trip on.
    ///
    /// # Errors
    /// Every problem found, each naming where it is.
    pub fn parse(text: &str) -> Result<Self, Problems> {
        let raw: RawWardroom =
            toml::from_str(text).map_err(|err| Problems(vec![err.to_string()]))?;
        let mut problems = Vec::new();
        let growth = growth(raw.growth, &mut problems);
        let hand = hand(raw.hand, &mut problems);
        let posts = posts(raw.post, &mut problems);
        let ladder = ladder(raw.rank, &mut problems);
        let skills = skills(raw.skill, &growth, &mut problems);
        let officers = officers(raw.officer, &posts, &ladder, &skills, &mut problems);
        if problems.is_empty() {
            Ok(Self {
                growth,
                hand,
                posts,
                ladder,
                skills,
                officers,
            })
        } else {
            Err(Problems(problems))
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWardroom {
    growth: RawGrowth,
    hand: RawHand,
    #[serde(default)]
    post: Vec<RawPost>,
    #[serde(default)]
    rank: Vec<RawRank>,
    #[serde(default)]
    skill: Vec<RawSkill>,
    #[serde(default)]
    officer: Vec<RawOfficer>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGrowth {
    experience: Vec<u32>,
    points_per_star: u32,
    experience_per_point: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHand {
    cards: usize,
    chance: Vec<f32>,
    weight: BTreeMap<String, Vec<f32>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPost {
    id: String,
    name: String,
    colour: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRank {
    name: String,
    short: String,
    lace: String,
    tiers: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSkill {
    id: String,
    name: String,
    #[serde(default)]
    key: bool,
    card: Option<String>,
    #[serde(default)]
    experience: f32,
    tiers: Vec<toml::Table>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOfficer {
    id: String,
    name: String,
    post: String,
    rarity: String,
    portrait: Option<String>,
    skills: BTreeMap<String, u8>,
}

const RARITIES: [(&str, Rarity); 3] = [
    ("common", Rarity::Common),
    ("uncommon", Rarity::Uncommon),
    ("rare", Rarity::Rare),
];

fn rarity(name: &str) -> Option<Rarity> {
    RARITIES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|&(_, rarity)| rarity)
}

fn growth(raw: RawGrowth, problems: &mut Vec<String>) -> Growth {
    if raw.experience.windows(2).any(|pair| pair[1] <= pair[0]) {
        problems.push(format!(
            "growth.experience: each tier must need more than the one before, not {:?}",
            raw.experience
        ));
    }
    if raw.experience.first() == Some(&0) {
        problems.push("growth.experience: tier II must need more than 0".into());
    }
    if raw.experience_per_point == 0 {
        problems.push("growth.experience_per_point: must be more than 0".into());
    }
    Growth {
        experience: raw.experience,
        points_per_star: raw.points_per_star,
        experience_per_point: raw.experience_per_point,
    }
}

fn hand(raw: RawHand, problems: &mut Vec<String>) -> Hand {
    if raw.cards == 0 {
        problems.push("hand.cards: a hand needs at least 1 card".into());
    }
    if raw.chance.is_empty() {
        problems.push("hand.chance: give a chance for 0 stars at least".into());
    }
    if let Some(bad) = raw.chance.iter().find(|c| !(0. ..=1.).contains(*c)) {
        problems.push(format!("hand.chance: {bad} is not between 0 and 1"));
    }
    let mut weights = Vec::new();
    for (name, table) in raw.weight {
        let Some(rarity) = rarity(&name) else {
            problems.push(format!(
                "hand.weight.{name}: no such rarity (common, uncommon or rare)"
            ));
            continue;
        };
        if table.len() != raw.chance.len() {
            problems.push(format!(
                "hand.weight.{name}: give {} weights, one for each entry of hand.chance",
                raw.chance.len()
            ));
        } else if table.iter().any(|&w| w < 0.) {
            problems.push(format!("hand.weight.{name}: weights can't be negative"));
        }
        weights.push((rarity, table));
    }
    for (name, rarity) in RARITIES {
        if !weights.iter().any(|(r, _)| *r == rarity) {
            problems.push(format!("hand.weight.{name}: missing"));
        }
    }
    Hand {
        cards: raw.cards,
        chance: raw.chance,
        weights,
    }
}

fn colour(text: &str) -> Option<[u8; 3]> {
    let hex = text.strip_prefix('#')?;
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let byte = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

fn posts(raw: Vec<RawPost>, problems: &mut Vec<String>) -> Vec<Post> {
    let mut posts: Vec<Post> = Vec::new();
    for post in raw {
        if posts.iter().any(|p| p.id == post.id) {
            problems.push(format!("post {}: listed twice", post.id));
            continue;
        }
        let colour = colour(&post.colour).unwrap_or_else(|| {
            problems.push(format!(
                "post {}: colour {:?} is not like \"#f06858\"",
                post.id, post.colour
            ));
            [255; 3]
        });
        posts.push(Post {
            id: post.id,
            name: post.name,
            colour,
        });
    }
    posts
}

fn skills(raw: Vec<RawSkill>, growth: &Growth, problems: &mut Vec<String>) -> Vec<SkillBook> {
    let mut found: Vec<SkillBook> = Vec::new();
    for book in raw {
        let Some(skill) = Skill::from_id(&book.id) else {
            problems.push(format!(
                "skill {}: the game has no such skill (it knows {})",
                book.id,
                Skill::ALL.map(Skill::id).join(", ")
            ));
            continue;
        };
        if found.iter().any(|b| b.skill == skill) {
            problems.push(format!("skill {}: listed twice", book.id));
            continue;
        }
        let at = format!("skill {}", book.id);
        if book.experience < 0. {
            problems.push(format!("{at}: experience can't be negative"));
        }
        if skill.earns().is_none() && book.experience != 0. {
            problems.push(format!(
                "{at}: nothing it does is counted, so experience must be 0"
            ));
        }
        if book.tiers.is_empty() || book.tiers.len() > growth.most_tiers() {
            problems.push(format!(
                "{at}: give 1 to {} tiers (growth.experience allows that many), not {}",
                growth.most_tiers(),
                book.tiers.len()
            ));
        }
        let tiers = book
            .tiers
            .into_iter()
            .enumerate()
            .map(|(i, table)| tier(skill, &at, i + 1, table, book.card.as_deref(), problems))
            .collect();
        found.push(SkillBook {
            skill,
            name: book.name,
            key: book.key,
            experience: book.experience,
            tiers,
        });
    }
    let mut skills = Vec::new();
    for skill in Skill::ALL {
        match found.iter().position(|b| b.skill == skill) {
            Some(at) => skills.push(found.swap_remove(at)),
            None => problems.push(format!("skill {}: missing", skill.id())),
        }
    }
    skills
}

fn tier(
    skill: Skill,
    at: &str,
    number: usize,
    mut table: toml::Table,
    card: Option<&str>,
    problems: &mut Vec<String>,
) -> Tier {
    let at = format!("{at}, tier {number}");
    let own_text = match table.remove("text") {
        Some(toml::Value::String(text)) => Some(text),
        Some(other) => {
            problems.push(format!("{at}: text must be a string, not {other}"));
            None
        }
        None => None,
    };
    let mut values = Vec::new();
    for &name in skill.numbers() {
        let value = match table.remove(name) {
            Some(toml::Value::Integer(n)) => n as f32,
            Some(toml::Value::Float(n)) => n as f32,
            Some(toml::Value::Boolean(b)) => f32::from(u8::from(b)),
            Some(other) => {
                problems.push(format!(
                    "{at}: {name} must be a number or true/false, not {other}"
                ));
                0.
            }
            None => {
                problems.push(format!("{at}: missing {name}"));
                0.
            }
        };
        values.push(value);
    }
    for name in table.keys() {
        problems.push(format!(
            "{at}: no number called {name} ({} takes {}, and text)",
            skill.id(),
            skill.numbers().join(", ")
        ));
    }
    let text = match (own_text, card) {
        (Some(text), _) => text,
        (None, Some(card)) => fill(card, skill, &values).unwrap_or_else(|problem| {
            problems.push(format!("{at}: card {problem}"));
            card.to_string()
        }),
        (None, None) => {
            problems.push(format!("{at}: needs a text, as the skill has no card line"));
            String::new()
        }
    };
    Tier { text, values }
}

/// A card line with each `{name}` replaced by that number, and each `{name%}` by it as a
/// percentage.
fn fill(card: &str, skill: Skill, values: &[f32]) -> Result<String, String> {
    let mut out = String::new();
    let mut rest = card;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let close = rest[open..]
            .find('}')
            .ok_or_else(|| format!("has an unclosed {{ in {card:?}"))?;
        let field = &rest[open + 1..open + close];
        let (name, percent) = match field.strip_suffix('%') {
            Some(name) => (name, true),
            None => (field, false),
        };
        let at = skill
            .numbers()
            .iter()
            .position(|&n| n == name)
            .ok_or_else(|| format!("names {{{field}}}, which isn't one of the skill's numbers"))?;
        let value = values[at];
        out.push_str(&if percent {
            format!("{}%", plain(value * 100.))
        } else {
            plain(value)
        });
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// A number as a card writes it: whole numbers bare, others to at most two places.
fn plain(value: f32) -> String {
    let rounded = (value * 100.).round() / 100.;
    if rounded.fract() == 0. {
        format!("{rounded:.0}")
    } else {
        format!("{rounded:.2}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

fn ladder(raw: Vec<RawRank>, problems: &mut Vec<String>) -> Vec<Rank> {
    if raw.is_empty() {
        problems.push("rank: give at least one rank".into());
    }
    let mut ladder: Vec<Rank> = Vec::new();
    for rank in raw {
        let at = format!("rank {}", rank.name);
        if let Some(below) = ladder.last()
            && rank.tiers <= below.tiers
        {
            problems.push(format!(
                "{at}: needs {} tiers, but it must need more than {} below it ({})",
                rank.tiers, below.name, below.tiers
            ));
        }
        if rank.short.trim().is_empty() {
            problems.push(format!("{at}: short can't be blank"));
        }
        let lace: Option<Vec<Lace>> = rank
            .lace
            .chars()
            .map(|c| match c {
                'b' => Some(Lace::Broad),
                'w' => Some(Lace::Wide),
                'n' => Some(Lace::Thin),
                _ => None,
            })
            .collect();
        let lace = match lace {
            Some(lace) if (1..=6).contains(&lace.len()) => lace,
            _ => {
                problems.push(format!(
                    "{at}: lace \"{}\" should be 1 to 6 of b (broad), w (wide), n (thin)",
                    rank.lace
                ));
                vec![Lace::Wide]
            }
        };
        ladder.push(Rank {
            name: rank.name,
            short: rank.short,
            lace,
            tiers: rank.tiers,
        });
    }
    ladder
}

fn officers(
    raw: Vec<RawOfficer>,
    posts: &[Post],
    ladder: &[Rank],
    skills: &[SkillBook],
    problems: &mut Vec<String>,
) -> Vec<Officer> {
    let mut officers: Vec<Officer> = Vec::new();
    for officer in raw {
        let at = format!("officer {}", officer.id);
        if officers.iter().any(|o| o.id == officer.id) {
            problems.push(format!("{at}: listed twice"));
            continue;
        }
        if let Some(rank) = ladder
            .iter()
            .find(|rank| officer.name.starts_with(&format!("{} ", rank.short)))
        {
            problems.push(format!(
                "{at}: leave {} off the name; the card puts the officer's rank there",
                rank.short
            ));
        }
        let post = posts.iter().position(|p| p.id == officer.post);
        if post.is_none() {
            problems.push(format!("{at}: no post called {}", officer.post));
        }
        let rarity = rarity(&officer.rarity);
        if rarity.is_none() {
            problems.push(format!(
                "{at}: rarity {} is not common, uncommon or rare",
                officer.rarity
            ));
        }
        if officer.skills.is_empty() {
            problems.push(format!("{at}: brings no skills"));
        }
        let mut brings = Vec::new();
        for (id, start) in &officer.skills {
            let Some(skill) = Skill::from_id(id) else {
                problems.push(format!("{at}: no skill called {id}"));
                continue;
            };
            let tiers = skills
                .iter()
                .find(|b| b.skill == skill)
                .map_or(0, |b| b.tiers.len());
            if *start == 0 || usize::from(*start) > tiers {
                problems.push(format!(
                    "{at}: {id} starts at tier {start}, but it has tiers 1 to {tiers}"
                ));
            }
            brings.push((skill, *start));
        }
        brings.sort();
        officers.push(Officer {
            id: officer.id,
            name: officer.name,
            post: post.unwrap_or(0),
            rarity: rarity.unwrap_or(Rarity::Common),
            portrait: officer.portrait,
            skills: brings,
        });
    }
    officers
}

#[cfg(test)]
mod tests {
    use super::*;

    fn problems(text: &str) -> Vec<String> {
        Wardroom::parse(text).expect_err("should be refused").0
    }

    /// The stock file with `from` replaced by `to`, which must be there.
    fn stock_with(from: &str, to: &str) -> String {
        assert!(STOCK.contains(from), "the stock file has no {from:?}");
        STOCK.replacen(from, to, 1)
    }

    #[test]
    fn the_shipped_wardroom_is_sound() {
        let wardroom = Wardroom::parse(STOCK).unwrap_or_else(|p| panic!("{p}"));
        assert_eq!(wardroom.posts.len(), 5);
        assert_eq!(wardroom.officers.len(), 15);
        for skill in Skill::ALL {
            assert_eq!(wardroom.skill(skill).skill, skill);
        }
        // Every post has an officer of each rarity.
        for (at, post) in wardroom.posts.iter().enumerate() {
            for (_, rarity) in RARITIES {
                assert!(
                    wardroom
                        .officers
                        .iter()
                        .any(|o| o.post == at && o.rarity == rarity),
                    "{} has no {rarity:?} officer",
                    post.id
                );
            }
        }
    }

    #[test]
    fn rank_follows_the_tiers_held() {
        let wardroom = Wardroom::stock();
        let rank = |tiers| wardroom.rank(tiers).short.as_str();
        assert_eq!(rank(0), "Sub-Lt.");
        assert_eq!(rank(2), "Sub-Lt.");
        assert_eq!(rank(3), "Lt.");
        assert_eq!(rank(5), "Lt. Cdr.");
        assert_eq!(rank(9), "Capt.");
        assert_eq!(wardroom.rank(4).lace, [Lace::Wide, Lace::Thin, Lace::Wide]);
        // Commons join as sub-lieutenants and the rest as lieutenants; each has
        // promotions ahead of them.
        for officer in &wardroom.officers {
            let joins = wardroom.rank(officer.tiers());
            let expected = if officer.rarity == Rarity::Common {
                "Sub-Lt."
            } else {
                "Lt."
            };
            assert_eq!(joins.short, expected, "{}", officer.id);
            let most = officer.skills.len() as u32 * wardroom.growth.most_tiers() as u32;
            assert_ne!(wardroom.rank(most), joins, "{}", officer.id);
        }
    }

    #[test]
    fn a_bad_ladder_or_a_rank_in_a_name_is_refused() {
        let text = stock_with("lace = \"wnw\"\ntiers = 4", "lace = \"wxw\"\ntiers = 2").replacen(
            "name = \"Augustin Brask\"",
            "name = \"Lt. Augustin Brask\"",
            1,
        );
        assert_eq!(
            problems(&text),
            vec![
                "rank Lieutenant Commander: needs 2 tiers, but it must need more than \
                 Lieutenant below it (3)",
                "rank Lieutenant Commander: lace \"wxw\" should be 1 to 6 of b (broad), \
                 w (wide), n (thin)",
                "officer brask: leave Lt. off the name; the card puts the officer's rank there",
            ]
        );
    }

    #[test]
    fn numbers_and_card_lines_come_from_the_tiers() {
        let wardroom = Wardroom::stock();
        let fury = wardroom.skill(Skill::GunFury);
        assert!(fury.key);
        assert_eq!(fury.number(1, "seconds"), 10.);
        assert_eq!(fury.number(1, "reload"), 0.5);
        assert_eq!(fury.number(3, "cooldown"), 40.);
        assert_eq!(fury.tier(1).text, "10 s at 50% reload time, 60 s cooldown");
        let range = wardroom.skill(Skill::RangeBoost);
        assert_eq!(range.tier(3).text, "All weapons +40% reach");
        // A tier past the last reads as the last.
        assert_eq!(range.number(9, "reach"), range.number(3, "reach"));
        let radar = wardroom.skill(Skill::RadarOperator);
        assert!(!radar.flag(2, "lay_guns"));
        assert!(radar.flag(3, "lay_guns"));
        assert_eq!(
            radar.tier(1).text,
            "Contacts beyond line of sight, out to 1500 m"
        );
    }

    #[test]
    fn experience_ranks_a_skill_up_to_its_last_tier() {
        let wardroom = Wardroom::stock();
        let growth = &wardroom.growth;
        assert_eq!(growth.tier(0), 1);
        assert_eq!(growth.tier(99), 1);
        assert_eq!(growth.tier(100), 2);
        assert_eq!(growth.tier(10_000), 3);
        assert_eq!(growth.floor(2), 100);
        let fury = wardroom.skill(Skill::GunFury);
        assert_eq!(fury.tier_at(growth, 10_000), 3);
        let brask = wardroom.officer("brask").unwrap();
        assert_eq!(wardroom.post(brask).name, "Gunnery");
        assert_eq!(brask.rarity, Rarity::Uncommon);
        assert!(brask.skills.contains(&(Skill::GunFury, 2)));
    }

    #[test]
    fn the_hand_leans_rarer_with_stars() {
        let hand = Wardroom::stock().hand;
        assert_eq!(hand.cards, 3);
        assert_eq!(hand.chance(0), 0.);
        assert_eq!(hand.chance(3), 1.);
        assert_eq!(hand.chance(7), 1.);
        assert!(hand.weight(Rarity::Rare, 3) > hand.weight(Rarity::Rare, 0));
    }

    #[test]
    fn a_misspelt_number_is_named_with_what_the_skill_takes() {
        let found = problems(&stock_with(
            "{ seconds = 10, reload = 0.5, cooldown = 60 }",
            "{ secnds = 10, reload = 0.5, cooldown = 60 }",
        ));
        assert!(found.contains(&"skill gun_fury, tier 1: missing seconds".to_string()));
        assert!(found.iter().any(|p| {
            p.starts_with("skill gun_fury, tier 1: no number called secnds")
                && p.contains("seconds, reload, cooldown")
        }));
    }

    #[test]
    fn a_bad_file_lists_every_problem_at_once() {
        let text = stock_with("rarity = \"rare\"", "rarity = \"legendary\"")
            .replacen("post = \"signals\"", "post = \"sonar\"", 1)
            .replacen("experience = [100, 250]", "experience = [250, 100]", 1);
        let found = problems(&text);
        assert_eq!(found.len(), 3, "{found:#?}");
        assert!(found[0].starts_with("growth.experience"));
        assert!(
            found
                .iter()
                .any(|p| p.contains("officer sollum: rarity legendary"))
        );
        assert!(
            found
                .iter()
                .any(|p| p.contains("officer callender: no post called sonar"))
        );
    }

    #[test]
    fn unknown_fields_and_broken_toml_say_where() {
        let unknown = problems(&stock_with("points_per_star = 1", "points_per_starr = 1"));
        assert!(unknown[0].contains("points_per_starr"), "{unknown:?}");
        assert!(unknown[0].contains("line"), "{unknown:?}");
        let broken = problems("[growth\nexperience = [100]");
        assert!(broken[0].contains("line 1"), "{broken:?}");
    }

    #[test]
    fn skills_must_all_be_there_and_known() {
        let renamed = problems(&stock_with("id = \"hard_over\"", "id = \"hard_overr\""));
        assert!(
            renamed
                .iter()
                .any(|p| p.starts_with("skill hard_overr: the game has no such skill"))
        );
        assert!(renamed.contains(&"skill hard_over: missing".to_string()));
    }

    #[test]
    fn a_tier_beyond_the_ranks_or_the_skill_is_refused() {
        let found = problems(&stock_with(
            "skills = { gun_fury = 2, spotting_corrections = 1 }",
            "skills = { gun_fury = 4, spotting_corrections = 1 }",
        ));
        assert_eq!(
            found,
            vec!["officer brask: gun_fury starts at tier 4, but it has tiers 1 to 3"]
        );
        let four = problems(&stock_with(
            "    { scatter = 0.30 },\n]",
            "    { scatter = 0.30 },\n    { scatter = 0.40 },\n]",
        ));
        assert_eq!(
            four,
            vec!["skill rate_plot: give 1 to 3 tiers (growth.experience allows that many), not 4"]
        );
    }

    #[test]
    fn card_lines_fill_numbers_and_percentages() {
        let fill = |card| fill(card, Skill::GunFury, &[12., 0.35, 7.5]);
        assert_eq!(
            fill("{seconds} s, {reload%}, {cooldown}").unwrap(),
            "12 s, 35%, 7.5"
        );
        assert!(fill("{speed}").unwrap_err().contains("{speed}"));
        assert!(fill("{seconds").is_err());
        assert_eq!(plain(0.125), "0.13");
        assert_eq!(plain(2500.), "2500");
    }

    #[test]
    fn counted_experience_only_where_something_is_counted() {
        let found = problems(&stock_with(
            "name = \"Hard Over\"\ncard = \"Rudder +{rudder%}\"\nexperience = 0.0",
            "name = \"Hard Over\"\ncard = \"Rudder +{rudder%}\"\nexperience = 1.0",
        ));
        assert_eq!(
            found,
            vec!["skill hard_over: nothing it does is counted, so experience must be 0"]
        );
    }
}
