//! One leg sailed by the bot, and the profile of how she spent it.
use crate::bot::{Bot, Intent, THREAT_RANGE, Temperament, combatants_within};
use crate::sector::Sector;
use crate::voyage::{engage, fit_player, helm_at_start, open_battle, settle, steer};
use shipvivor_campaign::{Grade, LegEnd, Phase, Run, Stage};
use shipvivor_encounter::{Battery, EffectKind, Rewards};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// The game's fixed step.
pub const STEP: f32 = 1. / 60.;
/// Seconds between pulses in a leg's log, as in the game's run logs.
const PULSE: f32 = 15.;
/// Hull points a second, averaged over about ten seconds, that count as being swamped:
/// at this rate a stock hull is gone inside a minute and a half.
const SWAMP_RATE: f32 = 1.2;
/// Warships this close at once also count as being swamped.
const SWAMP_CROWD: usize = 4;
const SWAMP_RANGE: f32 = 600.;
/// Below this speed with the telegraph at full ahead she is crawling.
const CRAWL: f32 = 3.1;
const KNOTS_PER_MPS: f32 = 1.943_844;

/// What a second of the leg was, for the profile. Fleeing and being swamped outrank
/// fighting, which outranks closing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Activity {
    Fighting,
    Closing,
    Idle,
    Swamped,
    Fleeing,
}
impl Activity {
    pub const ALL: [Self; 5] = [
        Self::Fighting,
        Self::Closing,
        Self::Idle,
        Self::Swamped,
        Self::Fleeing,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Fighting => "fighting",
            Self::Closing => "closing",
            Self::Idle => "idle",
            Self::Swamped => "swamped",
            Self::Fleeing => "fleeing",
        }
    }
}

/// How a leg ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Outcome {
    /// Every yard wrecked, and any boss that sailed sunk.
    Cleared,
    /// Afloat, and the leg not won, when the simulation stopped a minute past the clock.
    Survived,
    RegentSunk,
    Sunk,
    /// The bot ran out of steps without the leg ending, which is a bug.
    Stalled,
}
impl Outcome {
    pub fn name(self) -> &'static str {
        match self {
            Self::Cleared => "cleared",
            Self::Survived => "survived",
            Self::RegentSunk => "regent sunk",
            Self::Sunk => "sunk",
            Self::Stalled => "stalled",
        }
    }
    fn of(end: LegEnd) -> Self {
        match end {
            LegEnd::BasesDestroyed => Self::Cleared,
            LegEnd::RegentSunk => Self::RegentSunk,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Setup {
    pub stage: Stage,
    /// How hard the passage into the sea runs. An anchorage ignores it.
    pub grade: Grade,
    pub seed: u64,
    pub temperament: Temperament,
}

/// Fifteen seconds of the leg.
#[derive(Clone, Debug, Default)]
pub struct Pulse {
    pub at: f32,
    pub hull: u16,
    pub knots: f32,
    pub fired: u32,
    pub hits: u32,
    pub damage: u32,
    pub sunk: u32,
    pub near: usize,
    pub seconds: BTreeMap<Activity, f32>,
}

#[derive(Clone, Debug)]
pub struct Report {
    pub setup: Setup,
    pub outcome: Outcome,
    /// Seconds sailed.
    pub sailed: f32,
    pub minutes: u16,
    pub stars: u8,
    pub kills: u32,
    pub yards: usize,
    pub bases: usize,
    pub lowest_hull: u16,
    pub damage: u32,
    pub fired: u32,
    pub hits: u32,
    pub spreads: u32,
    pub level: u32,
    pub cards: Vec<String>,
    pub seconds: BTreeMap<Activity, f32>,
    pub crawling: f32,
    /// Seconds with her engines out or half out.
    pub engine_down: f32,
    /// Seconds pressed against the land.
    pub aground: f32,
    /// Seconds with a module filling, short of flooded.
    pub filling: f32,
    /// The most of her length that was ever full of water at once.
    pub peak_water: f32,
    pub longest_quiet: f32,
    pub first_contact: Option<f32>,
    pub first_damage: Option<f32>,
    pub peak_crowd: usize,
    /// Enemy warships afloat in the sector, and within threat range of her, averaged
    /// over the leg: how busy the sea is.
    pub mean_afloat: f32,
    pub mean_near: f32,
    /// Enemy aircraft shot down.
    pub planes: u32,
    /// Enemy torpedoes that struck her.
    pub torpedoed: u32,
    pub pulses: Vec<Pulse>,
    /// Wall-clock seconds the simulation took.
    pub wall: f32,
}

impl Report {
    pub fn share(&self, activity: Activity) -> f32 {
        self.seconds.get(&activity).copied().unwrap_or(0.) / self.sailed.max(1e-3)
    }

    /// The leg as a log in the spirit of the game's run logs.
    pub fn log(&self) -> String {
        let s = &self.setup;
        let mut out = String::new();
        let _ = writeln!(
            out,
            "# simulated {} ({}) seed {} {}: {} after {}, {} stars, {} sunk, {}/{} yards, lowest hull {}%",
            s.stage.mission().name,
            s.grade.name().to_lowercase(),
            s.seed,
            s.temperament.name(),
            self.outcome.name(),
            clock(self.sailed),
            self.stars,
            self.kills,
            self.yards,
            self.bases,
            self.lowest_hull
        );
        let _ = writeln!(
            out,
            "# fired {} main ({} hits), {} spreads; took {} damage; level {}; cards {}",
            self.fired,
            self.hits,
            self.spreads,
            self.damage,
            self.level,
            self.cards.join(", ")
        );
        let _ = writeln!(out, "# {}", self.profile_line());
        for p in &self.pulses {
            let doing = p
                .seconds
                .iter()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .map_or("-", |(a, _)| a.name());
            let _ = writeln!(
                out,
                "{:>5}  {:<8} hull {:>3}%  {:>4.1} kn  fired {:>2} hit {:>2}  took {:>3}  sunk {}  near {}",
                clock(p.at),
                doing,
                p.hull,
                p.knots,
                p.fired,
                p.hits,
                p.damage,
                p.sunk,
                p.near
            );
        }
        out
    }

    pub fn profile_line(&self) -> String {
        let mut line = Activity::ALL
            .iter()
            .map(|a| format!("{} {:.0}%", a.name(), self.share(*a) * 100.))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = write!(
            line,
            "; afloat {:.1}, near {:.1}, {:.1} sunk/min, {} planes; crawling {:.0}% (engine down {:.0}%, aground {:.0}%); filling {:.0}%, peak water {:.0}%; longest quiet {}; first contact {}; peak crowd {}",
            self.mean_afloat,
            self.mean_near,
            self.kills as f32 / (self.sailed / 60.).max(1e-3),
            self.planes,
            self.crawling / self.sailed.max(1e-3) * 100.,
            self.engine_down / self.sailed.max(1e-3) * 100.,
            self.aground / self.sailed.max(1e-3) * 100.,
            self.filling / self.sailed.max(1e-3) * 100.,
            self.peak_water * 100.,
            clock(self.longest_quiet),
            self.first_contact.map_or("never".into(), clock),
            self.peak_crowd
        );
        line
    }
}

pub fn clock(seconds: f32) -> String {
    let whole = seconds.max(0.).round() as u32;
    format!("{}:{:02}", whole / 60, whole % 60)
}

/// Seconds past a leg's clock the simulation sails on before it stops. The clock ends
/// no leg, so a captain who can't win it would sail for ever.
pub const OVERTIME: f32 = 60.;

/// Sail one leg from a fresh, stock ship, until it ends or [`OVERTIME`] past its clock.
pub fn sail(setup: Setup) -> Report {
    let limit = f32::from(setup.stage.mission().minutes) * 60. + OVERTIME;
    sail_for(setup, limit)
}

/// Sail one leg from a fresh, stock ship for at most `limit` seconds.
pub fn sail_for(setup: Setup, limit: f32) -> Report {
    let mut run = Run::default();
    run.stage = setup.stage;
    run.grade = setup.grade;
    if setup.stage.is_opening() {
        run.origin = setup.stage;
    } else {
        run.course.push(setup.stage);
    }
    run.seed = setup.seed;
    sail_leg(&mut run, setup, &mut Bot::new(setup.temperament), limit, 1.)
}

/// Sail the leg `run` stands at the start of, with `bot` at the helm, for at most `limit`
/// seconds, until it ends: in her yard, at the intercept, won or lost. The run is left
/// as the leg left it. `pressure` scales the harm the enemy does her beyond the round's
/// own [`crate::sector::PRESSURE`] (see `Battle::with_pressure`).
pub fn sail_leg(run: &mut Run, setup: Setup, bot: &mut Bot, limit: f32, pressure: f32) -> Report {
    let started = std::time::Instant::now();
    let sector = Sector::of(setup.stage, setup.grade).with_strength(run.strength());
    let chart = sector.chart(setup.seed);
    let battle = open_battle(&sector, &chart, run, Rewards::Direct, true);
    // The round's own pressure, turned further by the simulation's.
    let own = battle.pressure();
    let mut battle = battle.with_pressure(own * pressure);
    let mut helm = helm_at_start(&chart, &battle, true);
    Bot::work_up(&mut helm);
    let minutes = setup.stage.mission().minutes;
    let max_steps = (limit / STEP).round() as usize;

    let mut report = Report {
        setup,
        outcome: Outcome::Stalled,
        sailed: 0.,
        minutes,
        stars: 0,
        kills: 0,
        yards: 0,
        bases: battle.ports().len(),
        lowest_hull: 100,
        damage: 0,
        fired: 0,
        hits: 0,
        spreads: 0,
        level: run.level,
        cards: vec![],
        seconds: BTreeMap::new(),
        crawling: 0.,
        engine_down: 0.,
        aground: 0.,
        filling: 0.,
        peak_water: 0.,
        longest_quiet: 0.,
        first_contact: None,
        first_damage: None,
        peak_crowd: 0,
        mean_afloat: 0.,
        mean_near: 0.,
        planes: 0,
        torpedoed: 0,
        pulses: vec![],
        wall: 0.,
    };
    let mut pulse = Pulse::default();
    let mut last_shot = battle.shots().iter().map(|s| s.body.id).max().unwrap_or(0);
    let mut damage_rate = 0_f32;
    let mut quiet = 0_f32;
    let mut held = None;
    let mut t = 0_f32;
    for _ in 0..max_steps {
        match run.phase() {
            Phase::Sailing => {}
            Phase::Refit => {
                let Some(card) = Bot::choose(&run.offers()) else {
                    break;
                };
                run.install(card);
                fit_player(&mut battle, run);
                report.cards.push(format!("{card:?}"));
                continue;
            }
            Phase::Lost => {
                report.outcome = Outcome::Sunk;
                break;
            }
            Phase::Drydock(end) | Phase::Intercept(end) | Phase::Won(end) => {
                report.outcome = Outcome::of(end);
                break;
            }
            Phase::Chart => {
                report.outcome = Outcome::Survived;
                break;
            }
        }
        report.stars = run.stars();
        report.kills = u32::from(run.objectives.kills);
        report.yards = battle.ruined_yards();
        report.level = run.level;
        let command = bot.decide(run, &battle, &helm, held, STEP);
        let ring = isize::from(command.notch) - isize::from(helm.notch());
        if ring != 0 {
            helm.ring(ring);
        }
        held = steer(run, &battle, &mut helm, command.rudder, STEP, true);
        let ev = engage(
            run,
            &mut battle,
            &mut helm,
            STEP,
            command.aim,
            None,
        );
        // Main-battery hits, counted as the game's run logs count them: the armour
        // effects her main guns raised this step.
        let main_hits = battle
            .effects()
            .iter()
            .filter(|fx| {
                fx.age == 0.
                    && matches!(
                        fx.kind,
                        EffectKind::Armor {
                            battery: Battery::Main,
                            ..
                        }
                    )
            })
            .count() as u32;
        let launched = settle(run, &mut battle, &helm, STEP, &ev, command.torpedo);
        t += STEP;
        report.yards = battle.ruined_yards();
        if run.phase() == Phase::Sailing {
            report.stars = run.stars();
            report.kills = u32::from(run.objectives.kills);
        }

        // What she did this step.
        let newest = battle
            .shots()
            .iter()
            .filter(|s| s.battery == Battery::Main && s.body.id > last_shot)
            .count() as u32;
        last_shot = battle
            .shots()
            .iter()
            .map(|s| s.body.id)
            .max()
            .unwrap_or(last_shot)
            .max(last_shot);
        let damage = u32::from(ev.damage);
        report.fired += newest;
        report.hits += main_hits;
        report.damage += damage;
        report.spreads += u32::from(launched);
        pulse.fired += newest;
        pulse.hits += main_hits;
        pulse.damage += damage;
        pulse.sunk += u32::from(ev.sunk);
        damage_rate += (damage as f32 - damage_rate * STEP) / 10.;
        let crowd = combatants_within(&battle, helm.position, SWAMP_RANGE);
        let near = combatants_within(&battle, helm.position, THREAT_RANGE);
        report.peak_crowd = report.peak_crowd.max(crowd);
        report.mean_afloat +=
            combatants_within(&battle, helm.position, f32::INFINITY) as f32 * STEP;
        report.mean_near += near as f32 * STEP;
        report.planes += u32::from(ev.planes_downed);
        report.torpedoed += u32::from(ev.torpedoed);
        if near > 0 && report.first_contact.is_none() {
            report.first_contact = Some(t);
        }
        if damage > 0 && report.first_damage.is_none() {
            report.first_damage = Some(t);
        }
        let activity = match command.intent {
            Intent::Fleeing => Activity::Fleeing,
            _ if damage_rate > SWAMP_RATE || crowd >= SWAMP_CROWD => Activity::Swamped,
            Intent::Fighting => Activity::Fighting,
            Intent::Closing => Activity::Closing,
            Intent::Idle => Activity::Idle,
        };
        *report.seconds.entry(activity).or_default() += STEP;
        *pulse.seconds.entry(activity).or_default() += STEP;
        if helm.speed < CRAWL {
            report.crawling += STEP;
        }
        if run.ship.propulsion() < 0.5 {
            report.engine_down += STEP;
        }
        let holds = run.ship.compartments();
        if holds.iter().any(|h| !h.wrecked() && h.water() > 0.05) {
            report.filling += STEP;
        }
        report.peak_water = report.peak_water.max(run.ship.water_aboard());
        if held.is_some() {
            report.aground += STEP;
        }
        let peaceful = newest == 0 && damage == 0 && near == 0;
        quiet = if peaceful { quiet + STEP } else { 0. };
        report.longest_quiet = report.longest_quiet.max(quiet);
        report.lowest_hull = report.lowest_hull.min(run.hull_percent());
        if t >= (report.pulses.len() + 1) as f32 * PULSE {
            pulse.at = t;
            pulse.hull = run.hull_percent();
            pulse.knots = helm.speed * KNOTS_PER_MPS;
            pulse.near = near;
            report.pulses.push(std::mem::take(&mut pulse));
        }
    }
    // The clock ends no leg: one still being sailed when the simulation stops survived.
    if report.outcome == Outcome::Stalled && run.phase() == Phase::Sailing {
        report.outcome = Outcome::Survived;
    }
    report.sailed = t;
    report.mean_afloat /= t.max(STEP);
    report.mean_near /= t.max(STEP);
    report.wall = started.elapsed().as_secs_f32();
    report
}
