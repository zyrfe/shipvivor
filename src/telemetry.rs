//! With the `dev-tools` feature (on by default, release builds too): a logbook of every
//! patrol run, written as it happens to `work/runs/<CODE>.log`, so a session can be
//! talked over afterwards with `/recap`.
//! Each run gets a short code of capital letters, shown in the window title.
//!
//! The logbook only watches. After each fixed step it compares the run, the battle and
//! the helm with what it saw the step before and writes down what changed: ships sunk,
//! hits landed and taken, damage by section, stores and bases wrecked, refits,
//! routes and pauses. Every 15 seconds of sailing a pulse line sums up the stretch.
use crate::snapshot::{show_toast, timestamp};
use crate::state::{Campaign, Encounter, Mode, PlayerHelm};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use shipvivor_campaign::{Fittings, LegEnd, Phase, Stage};
use shipvivor_encounter::{Battery, EffectKind, Impact, Sortie, YardState};
use shipvivor_shipyard::{Berth, State};
use std::collections::HashSet;
use std::f32::consts::{PI, TAU};
use std::fs::File;
use std::io::Write as _;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// The run logs folder inside the project, wherever the game is launched from.
const FOLDER: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/work/runs");
/// Seconds of sailing each pulse line sums up.
const PULSE: f32 = 15.;
/// Letters in a run code.
const CODE_LETTERS: usize = 7;
/// A step that costs this share of the whole hull or more gets a line of its own.
const HEAVY: f32 = 0.06;
/// Hull percentages worth a line as she falls through them.
const HULL_MARKS: [u16; 4] = [75, 50, 25, 10];
/// What became of the main battery's hits, in the order the tally reads them.
const IMPACTS: [(Impact, &str); 5] = [
    (Impact::Penetrated, "penetrated"),
    (Impact::PassedThrough, "passed through"),
    (Impact::Overpenetrated, "overpenetrated"),
    (Impact::Embedded, "embedded"),
    (Impact::Bounced, "bounced"),
];

pub struct TelemetryPlugin;
impl Plugin for TelemetryPlugin {
    fn build(&self, app: &mut App) {
        if cfg!(feature = "dev-tools") {
            app.init_resource::<Logbook>()
                .add_systems(FixedUpdate, record.after(crate::advance_ship))
                .add_systems(Update, title_bar)
                .add_systems(Last, sign_off);
        }
    }
}

/// The logs of this sitting's runs: the one under way, or the last one to end.
#[derive(Resource)]
pub struct Logbook {
    /// Where logs are written. `None` keeps their lines in memory, for tests.
    folder: Option<PathBuf>,
    /// When the game was launched. Runs with the same launch were one sitting.
    launch: String,
    run: Option<RunLog>,
    /// Messages for the screen, shown by the next fixed step.
    toasts: Vec<String>,
}
impl Default for Logbook {
    fn default() -> Self {
        Self::writing_to(Some(PathBuf::from(FOLDER)))
    }
}
impl Drop for Logbook {
    fn drop(&mut self) {
        if let Some(run) = self.run.as_mut().filter(|r| !r.over) {
            run.out
                .line("# The log stops here: the game shut down mid-run.");
        }
    }
}
impl Logbook {
    fn writing_to(folder: Option<PathBuf>) -> Self {
        Self {
            folder,
            launch: timestamp(SystemTime::now()).1,
            run: None,
            toasts: Vec::new(),
        }
    }
    /// The code of the run under way, or of the last one to end.
    pub fn code(&self) -> Option<&str> {
        self.run.as_ref().map(|r| r.code.as_str())
    }
    /// Note a dev capture in the log of the run under way.
    pub fn note_capture(&mut self, png: &str) {
        if let Some(run) = self.run.as_mut().filter(|r| !r.over) {
            run.write("capture", png);
        }
    }

    /// Compare the game with the last look and write down what changed.
    fn observe(&mut self, run: &Campaign, battle: &Encounter, helm: &PlayerHelm) {
        let live = matches!(run.mode, Mode::Patrol | Mode::Paused);
        if let Some(log) = self.run.as_mut().filter(|r| !r.over) {
            // Going back to the title gives the run up. A new seed is a new run.
            if !live || log.seed != run.seed {
                log.close("given up: back to the title");
                self.toasts
                    .push(format!("Run {} logged   /recap {}", log.code, log.code));
            }
        }
        if live && self.run.as_ref().is_none_or(|r| r.seed != run.seed) {
            self.open(run, battle);
        }
        let Some(log) = self.run.as_mut().filter(|r| !r.over) else {
            return;
        };
        log.clock = run.elapsed;
        log.follow_stage(run, battle);
        log.step(run, battle, helm);
        log.follow(run);
        if log.over {
            self.toasts
                .push(format!("Run {} logged   /recap {}", log.code, log.code));
        }
    }

    fn open(&mut self, run: &Campaign, battle: &Encounter) {
        let (code, out) = match &self.folder {
            None => (fresh_code(), Out::Memory(Vec::new())),
            Some(folder) => create_log(folder),
        };
        let mut log = RunLog::new(code, run, battle, out);
        let (_, started) = timestamp(SystemTime::now());
        for header in [
            format!("# Shipvivor run {}", log.code),
            format!("# started: {started}"),
            format!(
                "# launched: {} (runs with the same launch were one sitting)",
                self.launch
            ),
            format!(
                "# seed: {} (SHIPVIVOR_SEED={} cargo run sails these waters again)",
                run.seed, run.seed
            ),
            "# columns: wall clock (UTC), leg clock (m:ss sailed this leg), event, details".into(),
            format!(
                "# Pulses sum up each {PULSE:.0} s of sailing. Hits count when shells land, so a \
                 pulse can show more hits than rounds fired. Damage is hull points lost. Hits taken \
                 are counted from shell bursts on the hull, so two in one step count as one."
            ),
            String::new(),
        ] {
            log.out.line(&header);
        }
        log.begin_leg(run, battle);
        self.toasts.push(format!(
            "Run {}   logging to work/runs/{}.log",
            log.code, log.code
        ));
        self.run = Some(log);
    }

    /// Close the run under way, if any, saying why.
    fn give_up(&mut self, why: &str) {
        if let Some(log) = self.run.as_mut().filter(|r| !r.over) {
            log.close(why);
        }
    }

    #[cfg(test)]
    fn in_memory() -> Self {
        Self::writing_to(None)
    }
    #[cfg(test)]
    fn lines(&self) -> &[String] {
        match self.run.as_ref().map(|r| &r.out) {
            Some(Out::Memory(lines)) => lines,
            _ => &[],
        }
    }
}

fn record(
    run: Res<Campaign>,
    battle: Res<Encounter>,
    helm: Res<PlayerHelm>,
    mut log: ResMut<Logbook>,
    mut commands: Commands,
) {
    log.observe(&run, &battle, &helm);
    for message in std::mem::take(&mut log.toasts) {
        show_toast(&mut commands, message);
    }
}

/// The run's code stands in the window title, so it can be read off to `/recap` later.
fn title_bar(log: Res<Logbook>, mut window: Single<&mut Window, With<PrimaryWindow>>) {
    let title = log.code().map_or_else(
        || "Shipvivor".to_string(),
        |code| format!("Shipvivor — {code}"),
    );
    if window.title != title {
        window.title = title;
    }
}

fn sign_off(mut exits: MessageReader<AppExit>, mut log: ResMut<Logbook>) {
    if exits.read().next().is_some() {
        log.give_up("the game was closed");
    }
}

/// Seven capital letters, short enough to type back in.
fn fresh_code() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    );
    let mut bits = hasher.finish();
    (0..CODE_LETTERS)
        .map(|_| {
            let letter = char::from(b'A' + (bits % 26) as u8);
            bits /= 26;
            letter
        })
        .collect()
}

/// A code no earlier run has used, and its log file opened for writing.
fn create_log(folder: &std::path::Path) -> (String, Out) {
    if let Err(err) = std::fs::create_dir_all(folder) {
        warn!("Couldn't create {}: {err}", folder.display());
        return (fresh_code(), Out::Nowhere);
    }
    for _ in 0..8 {
        let code = fresh_code();
        match File::create_new(folder.join(format!("{code}.log"))) {
            Ok(file) => return (code, Out::File(file)),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => {
                warn!("Couldn't start the run log: {err}");
                return (code, Out::Nowhere);
            }
        }
    }
    (fresh_code(), Out::Nowhere)
}

enum Out {
    File(File),
    Memory(Vec<String>),
    Nowhere,
}
impl Out {
    fn line(&mut self, text: &str) {
        match self {
            Self::File(file) => {
                if let Err(err) = writeln!(file, "{text}") {
                    warn!("Couldn't write the run log: {err}");
                    *self = Self::Nowhere;
                }
            }
            Self::Memory(lines) => lines.push(text.to_owned()),
            Self::Nowhere => {}
        }
    }
}

/// What happened over a stretch of sailing.
#[derive(Default)]
struct Tally {
    main_fired: u32,
    /// Main battery hits on ships, by outcome in the order of `IMPACTS`.
    main_hits: [u32; IMPACTS.len()],
    secondary_fired: u32,
    secondary_hits: u32,
    torpedoes: u32,
    torpedo_hits: u32,
    aa_fired: u32,
    /// Enemy aircraft her AA guns brought down.
    planes: u32,
    /// Rounds fired at her by ships, shore guns and aircraft.
    ship_fire: u32,
    shore_fire: u32,
    air_fire: u32,
    hits_taken: u32,
    /// Hull points lost, in all and by section.
    damage: f32,
    by_section: Vec<(String, f32)>,
    collisions: u32,
    sunk: u32,
    enemy_guns: u32,
    shore_guns: u32,
    factories: u32,
    yards: u32,
    crates: u32,
}
impl Tally {
    fn main_hit(&self) -> u32 {
        self.main_hits.iter().sum()
    }
    fn hurt(&mut self, section: &str, damage: f32) {
        self.damage += damage;
        match self.by_section.iter_mut().find(|(name, _)| name == section) {
            Some((_, sum)) => *sum += damage,
            None => self.by_section.push((section.to_owned(), damage)),
        }
    }
    fn is_quiet(&self) -> bool {
        self.describe() == "quiet"
    }
    /// One line, leaving out whatever didn't happen.
    fn describe(&self) -> String {
        let mut parts = Vec::new();
        let mut fired = Vec::new();
        if self.main_fired > 0 || self.main_hit() > 0 {
            let outcomes: Vec<String> = IMPACTS
                .iter()
                .zip(self.main_hits)
                .filter(|(_, n)| *n > 0)
                .map(|((_, name), n)| format!("{n} {name}"))
                .collect();
            fired.push(match self.main_hit() {
                0 => format!("{} main (no hits)", self.main_fired),
                hit => format!(
                    "{} main ({hit} hit: {})",
                    self.main_fired,
                    outcomes.join(", ")
                ),
            });
        }
        if self.secondary_fired > 0 || self.secondary_hits > 0 {
            fired.push(format!(
                "{} secondary ({} hit)",
                self.secondary_fired, self.secondary_hits
            ));
        }
        if self.torpedoes > 0 || self.torpedo_hits > 0 {
            fired.push(format!(
                "{} ({} hit)",
                count(self.torpedoes, "torpedo", "torpedoes"),
                self.torpedo_hits
            ));
        }
        if self.aa_fired > 0 {
            fired.push(format!("{} AA", self.aa_fired));
        }
        if !fired.is_empty() {
            parts.push(format!("fired {}", fired.join(", ")));
        }
        if self.hits_taken > 0 || self.damage >= 0.5 {
            let mut sections: Vec<&(String, f32)> = self.by_section.iter().collect();
            sections.sort_by(|a, b| b.1.total_cmp(&a.1));
            let sections: Vec<String> = sections
                .iter()
                .filter(|(_, d)| *d >= 0.5)
                .map(|(name, d)| format!("{d:.0} on {name}"))
                .collect();
            parts.push(format!(
                "took {} for {:.0} ({})",
                count(self.hits_taken, "hit", "hits"),
                self.damage,
                sections.join(", ")
            ));
        }
        if self.collisions > 0 {
            parts.push(count(self.collisions, "collision", "collisions"));
        }
        let gains: Vec<String> = [
            (self.sunk, "ship sunk", "ships sunk"),
            (self.planes, "aircraft shot down", "aircraft shot down"),
            (
                self.enemy_guns,
                "enemy gun knocked out",
                "enemy guns knocked out",
            ),
            (self.shore_guns, "shore gun silenced", "shore guns silenced"),
            (self.factories, "factory wrecked", "factories wrecked"),
            (self.yards, "yard wrecked", "yards wrecked"),
            (self.crates, "store recovered", "stores recovered"),
        ]
        .into_iter()
        .filter(|(n, ..)| *n > 0)
        .map(|(n, one, many)| count(n, one, many))
        .collect();
        if !gains.is_empty() {
            parts.push(gains.join(", "));
        }
        let incoming: Vec<String> = [
            (self.ship_fire, "ships"),
            (self.shore_fire, "shore"),
            (self.air_fire, "aircraft"),
        ]
        .into_iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, from)| format!("{n} from {from}"))
        .collect();
        if !incoming.is_empty() {
            parts.push(format!("enemy rounds: {}", incoming.join(", ")));
        }
        if parts.is_empty() {
            "quiet".into()
        } else {
            parts.join("; ")
        }
    }
}

fn count(n: u32, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The ship as last seen, to tell what a step did to her.
#[derive(Default)]
struct Hull {
    names: Vec<String>,
    structure: Vec<f32>,
    wrecked: Vec<bool>,
    gear: Vec<Vec<State>>,
    percent: u16,
    peril: bool,
}
impl Hull {
    fn of(run: &Campaign) -> Self {
        let ship = &run.ship;
        let modules = ship.design().modules();
        Self {
            names: section_names(run),
            structure: ship.compartments().iter().map(|h| h.structure()).collect(),
            wrecked: ship.compartments().iter().map(|h| h.wrecked()).collect(),
            gear: (0..modules.len())
                .map(|module| {
                    (0..modules[module].fit.len())
                        .map(|index| ship.state(Berth { module, index }))
                        .collect()
                })
                .collect(),
            percent: run.hull_percent(),
            peril: ship.in_peril(),
        }
    }
}

/// The battle as last seen.
struct Waters {
    at_sea: f32,
    raiders: HashSet<u64>,
    torpedoes: HashSet<u64>,
    /// Each base's yard, working factories and live shore guns.
    ports: Vec<(YardState, usize, usize)>,
    spotted: Vec<bool>,
    flagship: Option<Sortie>,
    /// Ships with their bridge shot away and nobody yet at the helm.
    helmless: HashSet<u64>,
}
impl Waters {
    fn of(battle: &Encounter) -> Self {
        Self {
            at_sea: battle.at_sea(),
            raiders: battle.raiders().iter().map(|r| r.body.id).collect(),
            torpedoes: battle.torpedoes().iter().map(|t| t.body.id).collect(),
            ports: battle
                .ports()
                .iter()
                .map(|p| {
                    let guns = p.turrets().iter().filter(|g| g.live()).count();
                    (p.yard_state(), p.live_factories(), guns)
                })
                .collect(),
            spotted: battle.spotters().iter().map(|s| s.reporting).collect(),
            flagship: battle.flagship(),
            helmless: battle
                .raiders()
                .iter()
                .filter(|r| r.helm_lost() > 0.)
                .map(|r| r.body.id)
                .collect(),
        }
    }
}

/// One run's log, from its start until it is won, lost or given up.
struct RunLog {
    code: String,
    seed: u64,
    out: Out,
    over: bool,
    /// Seconds sailed this leg, for stamping lines.
    clock: f32,
    leg: u8,
    stage: Stage,
    phase: Phase,
    mode: Mode,
    paused: Option<Instant>,
    next_pulse: f32,
    /// The lowest the hull has been this leg, in percent.
    low: u16,
    pulse: Tally,
    leg_tally: Tally,
    total: Tally,
    /// One line per finished leg, for the run's summary.
    legs: Vec<String>,
    /// A leg is being sailed and has not been summed up.
    leg_open: bool,
    /// The ship's sections from bow to stern, to tell a refit in the dock.
    sections: Vec<String>,
    hits: u32,
    level: u32,
    objectives: (u16, u16, bool),
    goals: [bool; 3],
    can_extract: bool,
    fittings: Fittings,
    hull: Hull,
    waters: Waters,
}
impl RunLog {
    fn new(code: String, run: &Campaign, battle: &Encounter, out: Out) -> Self {
        Self {
            code,
            seed: run.seed,
            out,
            over: false,
            clock: run.elapsed,
            leg: 0,
            stage: run.stage,
            phase: run.phase(),
            mode: run.mode,
            paused: None,
            next_pulse: PULSE,
            low: run.hull_percent(),
            pulse: Tally::default(),
            leg_tally: Tally::default(),
            total: Tally::default(),
            legs: Vec::new(),
            leg_open: false,
            sections: section_names(run),
            hits: run.hits,
            level: run.level,
            objectives: objectives(run),
            goals: goals(run),
            can_extract: run.can_extract(),
            fittings: run.fittings,
            hull: Hull::of(run),
            waters: Waters::of(battle),
        }
    }

    fn write(&mut self, tag: &str, text: impl std::fmt::Display) {
        let secs = self.clock.max(0.) as u32;
        let line = format!(
            "{}  {:>5}  {tag:<9}{text}",
            wall_clock(),
            format!("{}:{:02}", secs / 60, secs % 60)
        );
        self.out.line(&line);
    }

    fn bump(&mut self, change: impl Fn(&mut Tally)) {
        change(&mut self.pulse);
        change(&mut self.leg_tally);
        change(&mut self.total);
    }

    fn begin_leg(&mut self, run: &Campaign, battle: &Encounter) {
        self.leg += 1;
        self.leg_open = true;
        self.stage = run.stage;
        self.next_pulse = PULSE;
        self.low = run.hull_percent();
        self.pulse = Tally::default();
        self.leg_tally = Tally::default();
        let mission = run.stage.mission();
        let goals = if run.stage == Stage::CrownDeep {
            "sink the Regent".to_string()
        } else {
            format!(
                "goals {} sunk, {} stores, a drydock",
                mission.kills, mission.crates
            )
        };
        self.write(
            "leg",
            format!(
                "{} {}: {} min, {goals}; hull {}%, level {}",
                self.leg,
                mission.name,
                mission.minutes,
                run.hull_percent(),
                run.level
            ),
        );
        self.write("ship", describe_ship(run));
        self.sections = section_names(run);
        let mut fleet: Vec<(String, u32)> = Vec::new();
        for r in battle.raiders().iter().filter(|r| r.afloat()) {
            let class = format!("{:?}", r.class);
            match fleet.iter_mut().find(|(c, _)| *c == class) {
                Some((_, n)) => *n += 1,
                None => fleet.push((class, 1)),
            }
        }
        let fleet: Vec<String> = fleet.iter().map(|(c, n)| format!("{n} {c}")).collect();
        let fleet = if fleet.is_empty() {
            "none".to_string()
        } else {
            fleet.join(", ")
        };
        self.write("fleet", format!("afloat at the start: {fleet}"));
    }

    /// A new leg brings new waters.
    fn follow_stage(&mut self, run: &Campaign, battle: &Encounter) {
        if run.stage == self.stage {
            return;
        }
        self.begin_leg(run, battle);
        self.objectives = objectives(run);
        self.goals = goals(run);
        self.can_extract = run.can_extract();
        self.hull = Hull::of(run);
        self.waters = Waters::of(battle);
    }

    /// What one fixed step of the battle did.
    fn step(&mut self, run: &Campaign, battle: &Encounter, helm: &PlayerHelm) {
        if battle.at_sea() <= self.waters.at_sea {
            // No step since the last look: effects that are new still read as new.
            self.waters.at_sea = battle.at_sea();
            self.hull = Hull::of(run);
            return;
        }
        let shape = run.hull_shape();
        let reach = shape.bow.max(shape.stern) + shape.radius + 2.;
        let near = |at: Vec2| at.distance(helm.position) <= reach;
        let (mut bursts, mut armour_hits) = (0, 0);
        let mut crunches = Vec::new();
        for fx in battle.effects().iter().filter(|fx| fx.age == 0.) {
            match fx.kind {
                EffectKind::Muzzle(Battery::Main) => self.bump(|t| t.main_fired += 1),
                EffectKind::Muzzle(Battery::Secondary) => self.bump(|t| t.secondary_fired += 1),
                EffectKind::Muzzle(Battery::Raider | Battery::EnemySecondary) => {
                    self.bump(|t| t.ship_fire += 1)
                }
                EffectKind::Muzzle(Battery::Shore) => self.bump(|t| t.shore_fire += 1),
                EffectKind::Muzzle(Battery::Aircraft) => self.bump(|t| t.air_fire += 1),
                EffectKind::Muzzle(Battery::AntiAir) => self.bump(|t| t.aa_fired += 1),
                EffectKind::Downed(frame) => {
                    self.bump(|t| t.planes += 1);
                    let place = whereabouts(helm, fx.body.pos);
                    self.write("shot down", format!("{frame:?} {place}"));
                }
                EffectKind::Armor {
                    outcome, battery, ..
                } => {
                    armour_hits += 1;
                    if battery == Battery::Main {
                        let slot = IMPACTS.iter().position(|(i, _)| *i == outcome);
                        self.bump(|t| t.main_hits[slot.unwrap_or(0)] += 1);
                    } else {
                        self.bump(|t| t.secondary_hits += 1);
                    }
                }
                EffectKind::Wreck(class) => {
                    self.bump(|t| t.sunk += 1);
                    let flagship = if class.is_flagship() {
                        ", the flagship"
                    } else {
                        ""
                    };
                    let place = whereabouts(helm, fx.body.pos);
                    self.write("sunk", format!("{class:?}{flagship} {place}"));
                }
                EffectKind::MountLost(_) => self.bump(|t| t.enemy_guns += 1),
                EffectKind::Burst if near(fx.body.pos) => bursts += 1,
                EffectKind::Crunch if near(fx.body.pos) => crunches.push(fx.body.pos),
                _ => {}
            }
        }
        // Torpedoes strike without an armour effect, so they are the hits left over.
        let landed = run.hits.saturating_sub(self.hits);
        let torpedo_hits = landed.saturating_sub(armour_hits);
        self.bump(|t| t.torpedo_hits += torpedo_hits);
        self.hits = run.hits;
        let fish: Vec<u64> = battle
            .torpedoes()
            .iter()
            .map(|t| t.body.id)
            .filter(|id| !self.waters.torpedoes.contains(id))
            .collect();
        if !fish.is_empty() {
            let n = fish.len() as u32;
            self.bump(|t| t.torpedoes += n);
            self.write("torpedo", format!("a spread of {n} away"));
        }
        self.sightings(battle, helm);
        self.damage(run, battle, bursts, &crunches);
        self.bases(battle, helm);
        self.objectives(run);
        self.waters = Waters::of(battle);
        if run.elapsed >= self.next_pulse {
            self.write_pulse(run, battle, helm);
            self.next_pulse = (run.elapsed / PULSE).floor() * PULSE + PULSE;
        }
    }

    fn sightings(&mut self, battle: &Encounter, helm: &PlayerHelm) {
        for r in battle.raiders() {
            if self.waters.raiders.contains(&r.body.id) {
                continue;
            }
            use shipvivor_encounter::EnemyClass::*;
            if matches!(r.class, Cruiser | Battleship | Parliament | Regent) {
                let place = whereabouts(helm, r.body.pos);
                self.write("contact", format!("{:?} put to sea {place}", r.class));
            }
        }
        for (i, spotter) in battle.spotters().iter().enumerate() {
            if spotter.reporting && !self.waters.spotted.get(i).copied().unwrap_or(true) {
                self.write("spotted", "a spotter plane is reporting her position");
            }
        }
        let flagship = battle.flagship();
        if let Some(Sortie::Sailed { port, .. }) = flagship
            && flagship != self.waters.flagship
        {
            self.write(
                "flagship",
                format!("the flagship sailed from base {}", port + 1),
            );
        }
        for r in battle.raiders() {
            let was = self.waters.helmless.contains(&r.body.id);
            let lost = r.helm_lost();
            if lost > 0. && !was {
                let place = whereabouts(helm, r.body.pos);
                self.write(
                    "flagship",
                    format!(
                        "bridge shot away {place}: she holds her course at reduced way for {:.0} s",
                        lost.ceil()
                    ),
                );
            } else if lost == 0. && was && r.afloat() {
                self.write("flagship", "under command again");
            }
        }
    }

    fn damage(&mut self, run: &Campaign, battle: &Encounter, bursts: u32, crunches: &[Vec2]) {
        let now = Hull::of(run);
        let old = std::mem::take(&mut self.hull);
        if now.names != old.names {
            // The leg's last step can refit her in the dock. That is no damage.
            self.hull = now;
            return;
        }
        let mut lost = 0.;
        for (i, (&before, &after)) in old.structure.iter().zip(&now.structure).enumerate() {
            let drop = before - after;
            if drop > 0.01 {
                lost += drop;
                let name = now.names[i].clone();
                self.bump(|t| t.hurt(&name, drop));
            }
        }
        let worst = now
            .names
            .iter()
            .zip(old.structure.iter().zip(&now.structure))
            .max_by(|a, b| (a.1.0 - a.1.1).total_cmp(&(b.1.0 - b.1.1)))
            .map(|(name, _)| name.clone());
        if lost > 0.05 {
            let blows = bursts.max(1).max(crunches.len() as u32);
            self.bump(|t| t.hits_taken += blows);
            if lost >= HEAVY * run.ship.design().structure()
                && let Some(worst) = &worst
            {
                self.write(
                    "heavy",
                    format!("{lost:.0} hull lost in one blow, most on the {worst}"),
                );
            }
        }
        for &at in crunches {
            self.bump(|t| t.collisions += 1);
            let struck = battle
                .raiders()
                .iter()
                .map(|r| {
                    (
                        r.body.pos.distance(at),
                        format!("{:?} #{}", r.class, r.body.id),
                    )
                })
                .chain(battle.icebergs().iter().map(|ice| {
                    let size = format!("{:?} {:.0} m across", ice.shape, ice.radius * 2.);
                    (
                        ice.body.pos.distance(at) - ice.radius,
                        format!("ice, a {size}"),
                    )
                }))
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map_or_else(|| "something".into(), |(_, what)| what);
            let cost = if lost > 0.05 {
                format!("{lost:.0} hull lost")
            } else {
                "no damage".into()
            };
            self.write("collide", format!("struck {struck}: {cost}"));
        }
        // The leg's last step can repair her in the dock, and that is no news.
        if run.phase() == Phase::Sailing {
            for (module, (before, after)) in old.gear.iter().zip(&now.gear).enumerate() {
                for (index, (&was, &is)) in before.iter().zip(after).enumerate() {
                    if was == is {
                        continue;
                    }
                    let what = run.ship.design().modules()[module].fit[index];
                    let change = match (was, is) {
                        (_, State::Out) => "knocked out",
                        (State::Working, State::Damaged) => "damaged",
                        (State::Out, State::Damaged) => "half repaired",
                        (_, State::Working) => "back in action",
                        _ => continue,
                    };
                    let section = &now.names[module];
                    let what = words(&format!("{what:?}"));
                    self.write("gear", format!("{section} {what} {change}"));
                }
            }
            for (module, (&was, &is)) in old.wrecked.iter().zip(&now.wrecked).enumerate() {
                if was != is {
                    let section = &now.names[module];
                    let change = if is {
                        "wrecked and flooded"
                    } else {
                        "pumped out"
                    };
                    self.write("section", format!("{section} {change}"));
                }
            }
            let crossed = HULL_MARKS
                .iter()
                .any(|&mark| old.percent > mark && now.percent <= mark);
            if crossed {
                self.write("hull", format!("down to {}%", now.percent));
            }
            if now.peril && !old.peril {
                self.write("peril", "one more wrecked section would sink her");
            }
            self.low = self.low.min(now.percent);
        }
        self.hull = now;
    }

    fn bases(&mut self, battle: &Encounter, helm: &PlayerHelm) {
        for (i, port) in battle.ports().iter().enumerate() {
            let Some(&(yard, factories, guns)) = self.waters.ports.get(i) else {
                continue;
            };
            let place = whereabouts(helm, port.yard_position());
            let base = i + 1;
            let state = port.yard_state();
            if state != yard {
                let ruined = battle
                    .ports()
                    .iter()
                    .filter(|p| p.yard_state() == YardState::Ruined)
                    .count();
                let news = match state {
                    YardState::Ruined => {
                        self.bump(|t| t.yards += 1);
                        format!("yard wrecked ({ruined} of 3 down)")
                    }
                    YardState::Burning => "yard burning".into(),
                    YardState::Damaged => "yard damaged".into(),
                    YardState::Working => "yard back at work".into(),
                };
                self.write("base", format!("base {base} {place}: {news}"));
            }
            let left = port.live_factories();
            if left < factories {
                let n = (factories - left) as u32;
                self.bump(|t| t.factories += n);
                self.write(
                    "base",
                    format!("base {base}: factory wrecked, {left} left there"),
                );
            }
            let live = port.turrets().iter().filter(|g| g.live()).count();
            if live < guns {
                let n = (guns - live) as u32;
                self.bump(|t| t.shore_guns += n);
                self.write(
                    "base",
                    format!("base {base}: shore gun silenced, {live} left there"),
                );
            }
        }
    }

    /// The points she scores toward the leg's stars.
    fn objectives(&mut self, run: &Campaign) {
        let (_, crates, _) = objectives(run);
        let (_, had_crates, _) = self.objectives;
        let mission = run.stage.mission();
        if crates > had_crates {
            let n = u32::from(crates - had_crates);
            self.bump(|t| t.crates += n);
            self.write(
                "stores",
                format!(
                    "recovered {n}, {crates} this leg (the star needs {})",
                    mission.crates
                ),
            );
        }
        self.objectives = objectives(run);
        let now = goals(run);
        if run.stage != Stage::CrownDeep {
            for (i, name) in ["ships sunk", "stores recovered", "drydock destroyed"]
                .iter()
                .enumerate()
            {
                if now[i] && !self.goals[i] {
                    let stars = run.stars();
                    self.write("star", format!("{name} goal met, {stars} of 3 stars"));
                }
            }
        }
        self.goals = now;
        if run.can_extract() && !self.can_extract {
            self.write("extract", "extraction is open");
        }
        self.can_extract = run.can_extract();
        if run.level > self.level {
            self.write(
                "level",
                format!("reached level {}, a fitting to choose", run.level),
            );
        }
        self.level = run.level;
    }

    fn write_pulse(&mut self, run: &Campaign, battle: &Encounter, helm: &PlayerHelm) {
        let afloat: Vec<_> = battle.raiders().iter().filter(|r| r.afloat()).collect();
        let holding = afloat.iter().filter(|r| r.has_contact).count();
        let nearest = afloat
            .iter()
            .min_by(|a, b| {
                let range = |r: &&&shipvivor_encounter::Raider| r.body.pos.distance(helm.position);
                range(a).total_cmp(&range(b))
            })
            .map_or(String::new(), |r| {
                format!(", nearest {:?} {}", r.class, whereabouts(helm, r.body.pos))
            });
        let line = format!(
            "{} {:.0} kn heading {:03}, hull {}%; {}; {} afloat, {holding} holding contact{nearest}",
            helm.order_label(),
            helm.speed * 1.943_84,
            degrees(helm.heading),
            run.hull_percent(),
            self.pulse.describe(),
            afloat.len(),
        );
        self.write("pulse", line);
        self.pulse = Tally::default();
    }

    /// Refits, pauses and the end of each leg, which happen between steps.
    fn follow(&mut self, run: &Campaign) {
        if run.fittings != self.fittings {
            let card = self
                .fittings
                .offers()
                .into_iter()
                .find(|&card| self.fittings.with(card) == run.fittings);
            let news = match card {
                Some(card) => format!("fitted {}", words(&format!("{card:?}"))),
                None => format!("fittings now {}", describe_fittings(run)),
            };
            self.write("refit", news);
            self.fittings = run.fittings;
        }
        let names = section_names(run);
        if names != self.sections {
            self.sections = names;
            self.write("ship", describe_ship(run));
            self.hull = Hull::of(run);
        }
        match (self.mode, run.mode) {
            (Mode::Patrol, Mode::Paused) => {
                self.paused = Some(Instant::now());
                self.write("pause", "paused");
            }
            (Mode::Paused, Mode::Patrol) => {
                let secs = self.paused.take().map_or(0, |t| t.elapsed().as_secs());
                self.write("resume", format!("after {}:{:02}", secs / 60, secs % 60));
            }
            _ => {}
        }
        self.mode = run.mode;
        let phase = run.phase();
        if phase == self.phase {
            return;
        }
        let was = std::mem::replace(&mut self.phase, phase);
        match phase {
            Phase::Drydock(end) | Phase::Intercept(end) if was == Phase::Sailing => {
                self.end_leg(run, leg_end(end));
                let next = if matches!(phase, Phase::Drydock(_)) {
                    "into the drydock"
                } else {
                    "on to intercept the Regent"
                };
                self.write("next", next);
            }
            Phase::Chart => self.write("next", "choosing a route"),
            Phase::Won(end) => {
                self.end_leg(run, leg_end(end));
                self.finish("won");
            }
            Phase::Lost => {
                let why = if run.ship.afloat() {
                    "time ran out"
                } else {
                    "she sank"
                };
                self.end_leg(run, why);
                self.finish("lost");
            }
            _ => {}
        }
    }

    fn end_leg(&mut self, run: &Campaign, why: &str) {
        self.leg_open = false;
        if !self.pulse.is_quiet() {
            let tally = self.pulse.describe();
            self.write(
                "pulse",
                format!("(last {:.0} s) {tally}", run.elapsed % PULSE),
            );
        }
        self.pulse = Tally::default();
        let mission = run.stage.mission();
        let (kills, crates, drydock) = objectives(run);
        let left = run.remaining().max(0.) as u32;
        let score = if run.stage == Stage::CrownDeep {
            String::new()
        } else {
            format!(
                ": {} (sunk {kills} of {}, stores {crates} of {}, {})",
                count(u32::from(run.stars()), "star", "stars"),
                mission.kills,
                mission.crates,
                if drydock {
                    "a drydock destroyed"
                } else {
                    "no drydock destroyed"
                }
            )
        };
        let summary = format!(
            "{why} with {}:{:02} left{score}; hull {}%, lowest {}%; level {}",
            left / 60,
            left % 60,
            run.hull_percent(),
            self.low.min(run.hull_percent()),
            run.level
        );
        self.legs.push(format!("{}: {summary}", mission.name));
        self.write("leg-end", summary);
        let tally = self.leg_tally.describe();
        self.write("tally", tally);
    }

    fn finish(&mut self, outcome: &str) {
        let legs = count(self.legs.len() as u32, "leg", "legs");
        self.write("run-end", format!("{outcome} after {legs}"));
        for leg in std::mem::take(&mut self.legs) {
            self.write("legs", leg);
        }
        let tally = self.total.describe();
        self.write("tally", tally);
        self.seal();
    }

    /// Stop writing. Lines kept in memory stay readable.
    fn seal(&mut self) {
        self.over = true;
        if !matches!(self.out, Out::Memory(_)) {
            self.out = Out::Nowhere;
        }
    }

    /// End the log of a run that stopped before it was won or lost.
    fn close(&mut self, why: &str) {
        if !self.pulse.is_quiet() {
            let tally = self.pulse.describe();
            self.write("pulse", format!("(last stretch) {tally}"));
        }
        self.write("end", why);
        if self.leg_open {
            let tally = self.leg_tally.describe();
            self.write("tally", format!("this leg: {tally}"));
        }
        let legs = std::mem::take(&mut self.legs);
        for leg in legs {
            self.write("legs", leg);
        }
        let tally = self.total.describe();
        self.write("tally", format!("this run: {tally}"));
        self.seal();
    }
}

fn objectives(run: &Campaign) -> (u16, u16, bool) {
    let o = run.objectives;
    (o.kills, o.crates, o.destroyed_drydock)
}

fn goals(run: &Campaign) -> [bool; 3] {
    let (kills, crates, drydock) = objectives(run);
    let m = run.stage.mission();
    [kills >= m.kills, crates >= m.crates, drydock]
}

fn leg_end(end: LegEnd) -> &'static str {
    match end {
        LegEnd::BasesDestroyed => "all three bases destroyed",
        LegEnd::TimeExpired => "time ran out",
        LegEnd::Extracted => "extracted",
        LegEnd::RegentSunk => "the Regent sunk",
    }
}

/// The ship's sections from bow to stern, numbered where two share a name.
fn section_names(run: &Campaign) -> Vec<String> {
    let count = run.ship.design().modules().len();
    let names: Vec<&str> = (0..count).map(|at| run.module_name(at)).collect();
    names
        .iter()
        .enumerate()
        .map(|(at, name)| {
            if names.iter().filter(|n| *n == name).count() > 1 {
                let nth = names[..=at].iter().filter(|n| *n == name).count();
                format!("{name} {nth}")
            } else {
                (*name).to_string()
            }
        })
        .collect()
}

fn describe_ship(run: &Campaign) -> String {
    format!(
        "{} ({:.0} m); {}",
        section_names(run).join(", "),
        run.ship.design().length(),
        describe_fittings(run)
    )
}

fn describe_fittings(run: &Campaign) -> String {
    let f = &run.fittings;
    let mut parts = vec![format!(
        "{} in main guns, {:?} turrets{}",
        f.calibre(),
        f.turrets,
        if f.armor_piercing() { ", AP" } else { "" }
    )];
    for (rank, name) in [
        (f.loader, "loader"),
        (f.caps, "caps"),
        (f.velocity, "velocity"),
        (f.turbines, "turbines"),
        (f.wing_mounts, "more secondaries"),
        (f.wing_shells, "heavy secondaries"),
        (f.wing_loaders, "quick secondaries"),
        (f.wing_range, "secondary range"),
        (f.wing_twin, "twin secondaries"),
        (f.training_gear, "training gear"),
        (f.armor, "armor plate"),
        (f.hardening, "hardening"),
        (f.anti_air, "AA guns"),
        (f.aa_mounts, "more AA"),
        (f.aa_shells, "heavy flak"),
        (f.aa_loaders, "AA loaders"),
    ] {
        if rank > 0 {
            parts.push(format!("{name} {rank}"));
        }
    }
    parts.push(format!("{:?} beam", f.beam));
    parts.join(", ")
}

/// Range and where it bears from her bow, as a lookout would call it.
fn whereabouts(helm: &PlayerHelm, at: Vec2) -> String {
    let range = at.distance(helm.position);
    let bearing = shipvivor_compass::heading(at - helm.position);
    let off = (bearing - helm.heading + PI).rem_euclid(TAU) - PI;
    let deg = off.to_degrees().abs().round();
    let side = if off > 0. { "starboard" } else { "port" };
    let bearing = match deg {
        d if d < 10. => "dead ahead".to_string(),
        d if d > 170. => "astern".to_string(),
        d => format!("{d:.0} deg to {side}"),
    };
    if range >= 1000. {
        format!("{:.1} km {bearing}", range / 1000.)
    } else {
        format!("{range:.0} m {bearing}")
    }
}

fn degrees(radians: f32) -> i32 {
    (radians.to_degrees().round() as i32).rem_euclid(360)
}

/// `ArmorPiercing` as "armor piercing".
fn words(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push(' ');
        }
        out.extend(c.to_lowercase());
    }
    out
}

/// The time of day in UTC, like the capture stamps.
fn wall_clock() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!(
        "{:02}:{:02}:{:02}",
        secs / 3600 % 24,
        secs / 60 % 60,
        secs % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use shipvivor_campaign::Objectives;

    #[test]
    fn codes_are_seven_capitals_and_rarely_repeat() {
        let codes: HashSet<String> = (0..200).map(|_| fresh_code()).collect();
        assert!(codes.len() > 195);
        for code in &codes {
            assert_eq!(code.len(), CODE_LETTERS);
            assert!(code.chars().all(|c| c.is_ascii_uppercase()));
        }
    }

    #[test]
    fn a_tally_leaves_out_what_did_not_happen() {
        let mut tally = Tally::default();
        assert_eq!(tally.describe(), "quiet");
        tally.main_fired = 6;
        tally.main_hits = [2, 0, 0, 0, 1];
        tally.hurt("Bow", 9.);
        tally.hurt("Bridge", 14.);
        tally.hurt("Bow", 3.);
        tally.hits_taken = 3;
        tally.sunk = 1;
        tally.crates = 2;
        tally.shore_fire = 4;
        assert_eq!(
            tally.describe(),
            "fired 6 main (3 hit: 2 penetrated, 1 bounced); took 3 hits for 26 (14 on \
             Bridge, 12 on Bow); 1 ship sunk, 2 stores recovered; enemy rounds: 4 from shore"
        );
        assert_eq!(words("ArmorPiercing"), "armor piercing");
    }

    fn game() -> App {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>()
            .add_plugins(crate::GamePlugin)
            .insert_resource(crate::state::ChartSeeds::fixed(4))
            .insert_resource(Logbook::in_memory())
            .add_plugins(TelemetryPlugin)
            .init_resource::<Time>()
            .init_resource::<bevy::input::mouse::AccumulatedMouseMotion>()
            .init_resource::<UiScale>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_message::<AppExit>();
        let w = app.world_mut();
        w.spawn(Window::default());
        let mut clock = w.resource_mut::<Time<Fixed>>();
        let dt = clock.timestep();
        clock.advance_by(dt);
        w.run_schedule(Startup);
        w.run_schedule(Update);
        app
    }

    fn sail(w: &mut World, seconds: f32) {
        for _ in 0..(seconds * 60.) as usize {
            w.run_schedule(FixedUpdate);
        }
    }

    fn logged(w: &World, tag: &str) -> Vec<String> {
        w.resource::<Logbook>()
            .lines()
            .iter()
            .filter(|line| line.get(17..).is_some_and(|rest| rest.starts_with(tag)))
            .cloned()
            .collect()
    }

    #[test]
    fn a_patrol_is_logged_from_its_first_leg_to_giving_up() {
        let mut app = game();
        let w = app.world_mut();
        sail(w, 1.);
        assert!(
            w.resource::<Logbook>().code().is_none(),
            "the title is not a run"
        );
        let mut keys = w.resource_mut::<ButtonInput<KeyCode>>();
        keys.press(KeyCode::Enter);
        w.run_schedule(PreUpdate);
        w.resource_mut::<ButtonInput<KeyCode>>().reset_all();
        sail(w, 1.);
        assert!(w.resource::<Logbook>().code().is_none());
        w.resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit2);
        w.run_schedule(PreUpdate);
        w.resource_mut::<ButtonInput<KeyCode>>().reset_all();
        // Well clear of the pickets, so the only damage is what the test does.
        w.resource_mut::<PlayerHelm>().position = Vec2::splat(2000.);
        sail(w, 1.);
        let code = w.resource::<Logbook>().code().unwrap().to_owned();
        let lines = w.resource::<Logbook>().lines();
        assert_eq!(lines[0], format!("# Shipvivor run {code}"));
        assert!(lines[3].contains("SHIPVIVOR_SEED=4 cargo run"));
        let leg = logged(w, "leg");
        assert_eq!(leg.len(), 1);
        assert!(logged(w, "fleet")[0].contains("afloat at the start: "));
        assert!(leg[0].contains("1 The Iron Narrows: 10 min, goals 4 sunk"));

        // A shell on the bridge between steps reads as a hit on the next one.
        let at = w.resource::<Campaign>().ship.design().length() * 0.3;
        w.resource_mut::<Campaign>().ship.strike_hull(at, 9.);
        sail(w, 15.);
        let pulses = logged(w, "pulse");
        assert_eq!(pulses.len(), 1, "{pulses:?}");
        assert!(pulses[0].contains("took 1 hit for 9 ("), "{}", pulses[0]);

        w.resource_mut::<Campaign>().mode = Mode::Paused;
        sail(w, 0.1);
        w.resource_mut::<Campaign>().mode = Mode::Patrol;
        sail(w, 0.1);
        assert_eq!(logged(w, "pause").len(), 1);
        assert!(logged(w, "resume")[0].contains("after 0:00"));

        // The clock runs out on the first leg: into the drydock.
        w.resource_mut::<Campaign>()
            .tick(600., Objectives::default(), 0);
        sail(w, 0.1);
        let end = logged(w, "leg-end");
        assert_eq!(end.len(), 1);
        assert!(
            end[0].contains("time ran out with 0:00 left: 0 stars"),
            "{}",
            end[0]
        );
        assert!(logged(w, "next")[0].contains("drydock"));

        // Any work the yard will take is written down once the step after it runs.
        let mut run = w.resource_mut::<Campaign>();
        let jobs: Vec<_> = (0..run.ship.design().modules().len())
            .flat_map(|at| run.section_jobs(at))
            .collect();
        assert!(jobs.into_iter().any(|job| run.refit_in_dock(job)));
        sail(w, 0.1);
        let refits = logged(w, "ship").len() - 1 + logged(w, "refit").len();
        assert_eq!(refits, 1, "{:?}", w.resource::<Logbook>().lines());

        w.resource_mut::<Campaign>().mode = Mode::Title;
        sail(w, 0.1);
        let end = logged(w, "end");
        assert!(end[0].contains("10:00  end      given up"), "{end:?}");
        assert!(logged(w, "tally").iter().all(|t| !t.contains("this leg")));
        let tallies = logged(w, "tally");
        assert!(
            tallies
                .last()
                .unwrap()
                .contains("this run: took 1 hit for 9")
        );
        assert_eq!(w.resource::<Logbook>().code(), Some(code.as_str()));
    }
}
