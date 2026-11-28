//! A captain for simulated legs. She sails for the nearest standing yard, fights what
//! comes within range broadside on, leads her targets as the game's fire control
//! shows, fires torpedoes when a ship lies on a beam, and decides when to run
//! according to her temperament.
//!
//! The bot plays honestly: she gives only the orders a player can give (rudder,
//! engine telegraph, the sight with the trigger held, shell type, a torpedo spread,
//! a card) and reads only what a player can see on the chart.
use glam::{Vec2, Vec3, Vec3Swizzles};
use shipvivor_campaign::{Card, Run};
use shipvivor_compass::{direction, heading};
use shipvivor_encounter::{Aim, Battle, EnemyClass, Raider, TORPEDO_SPEED};
use shipvivor_navigation::Helm;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// How much a captain will stand before she runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Temperament {
    /// Fights and never runs.
    Bold,
    /// Fights, but runs when she is badly hurt.
    Steady,
    /// Runs when she is hurt and from any crowd.
    Cautious,
}
impl Temperament {
    pub const ALL: [Self; 3] = [Self::Bold, Self::Steady, Self::Cautious];
    pub fn name(self) -> &'static str {
        match self {
            Self::Bold => "bold",
            Self::Steady => "steady",
            Self::Cautious => "cautious",
        }
    }
}

/// How well she plays, from 0, a novice, to 1, the bot at her best: every gun laid on
/// the fire-control ghost, a fresh decision every half second, and every torpedo combed
/// as the alarm rings. Below it her laying wanders off the mark, more the farther out it
/// is and more along the line of fire than across it, and she resettles it only every
/// [`AIM_SETTLE`] seconds. She makes up her mind more slowly and turns for a torpedo
/// later.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Skill(pub f32);
impl Skill {
    pub const SHARP: Self = Self(1.);
    /// Metres her laying is off, one standard deviation along the line of fire, for
    /// each 100 metres of range.
    fn aim_error(self) -> f32 {
        AIM_ERROR * (1. - self.0.clamp(0., 1.))
    }
    fn think(self) -> f32 {
        THINK + SLOW_THINK * (1. - self.0.clamp(0., 1.))
    }
    fn dodge_warning(self) -> f32 {
        DODGE_WARNING * (LATE_DODGE + (1. - LATE_DODGE) * self.0.clamp(0., 1.))
    }
}
/// A novice's laying error: metres, one standard deviation along the line of fire, per
/// 100 metres of range. Across the line it is [`AIM_ACROSS`] of that.
const AIM_ERROR: f32 = 14.;
const AIM_ACROSS: f32 = 0.35;
/// Seconds she holds her laying before she corrects it, as a player settles the sight.
pub const AIM_SETTLE: f32 = 1.6;
/// Seconds more a novice takes over each decision about where to go.
const SLOW_THINK: f32 = 1.5;
/// The share of the torpedo warning a novice has left when she turns.
const LATE_DODGE: f32 = 0.35;

/// Ships within this range count as close threats.
pub const THREAT_RANGE: f32 = 800.;
/// Range she prefers to fight at, broadside on.
const FIGHTING_RANGE: f32 = 360.;
/// A warship this close is fought before anything else. Farther off, a yard in reach
/// comes first: it is the leg's work, and its slips keep sending more ships. A squadron's
/// line lies at about four fifths of its guns' range, outside this, so she shells the
/// yard through it, as a player does, and turns on what closes her.
const PRESSING: f32 = 260.;
/// She runs in this close to a yard to wreck it, as a player does: near enough that her
/// guns drop on its works at once and its gantries soon come down.
const BOMBARD_RANGE: f32 = 210.;
/// Metres further off she stands from a yard each time the shore catches her near it, and
/// the farthest she backs off to. At the head of a channel the water runs out round the
/// heavy yard, and a captain who keeps putting her bow on the rocks stands off instead.
const STAND_OFF_STEP: f32 = 70.;
const STAND_OFF_MOST: f32 = 460.;
/// How far ahead she looks for land before committing to a heading.
const LOOKOUT: f32 = 110.;
/// Seconds between her decisions about where to go. The sight and rudder act every step.
const THINK: f32 = 0.5;

/// What she is doing, as the run's profile records it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Intent {
    /// Firing on a ship or a yard in range.
    Fighting,
    /// Under way for something to fight.
    Closing,
    /// Nothing afloat and no yard standing: nothing to do and nowhere to go.
    Idle,
    /// Running from the enemy.
    Fleeing,
}

/// One step's orders.
pub struct Command {
    pub rudder: f32,
    /// The engine order she wants rung: 4 is full ahead, -3 full astern.
    pub notch: i8,
    pub aim: Aim,
    /// A bearing for a torpedo spread.
    pub torpedo: Option<f32>,
    pub intent: Intent,
}

pub struct Bot {
    pub temperament: Temperament,
    pub skill: Skill,
    /// Her own dice, so a seed always plays the same.
    dice: u64,
    /// Her laying error now, along the line of fire and across it, in standard
    /// deviations, the ship it was settled on, and seconds until she settles it again.
    laying: Vec2,
    laid_on: Option<u64>,
    settle: f32,
    /// How far off she stands from the yard she is wrecking, and which yard it is.
    stand_off: f32,
    standing_off: Option<Vec2>,
    /// Where she was at her last check of her progress, and seconds since it.
    progress: (Vec2, f32),
    think: f32,
    torpedo_clock: f32,
    goal: Option<Vec2>,
    desired: f32,
    intent: Intent,
    target: Option<u64>,
    fleeing: bool,
    /// Seconds she has been held against the land with no way on.
    aground: f32,
    /// Seconds left backing off the land astern.
    backing: f32,
    /// Which way the shore last pushed her, and seconds left keeping off it.
    shore: Vec2,
    keep_off: f32,
    /// Seconds left steaming straight out from a shore that caught her going astern.
    escape: f32,
    /// Seconds left combing a torpedo's track, and the heading that does it.
    dodge: f32,
    dodge_heading: f32,
}

/// Seconds pressed against the land before she backs off it.
const AGROUND: f32 = 1.5;
/// Metres a second below which, with way rung on, she is held fast by something.
const STALLED: f32 = 1.;
/// Seconds she backs off for.
const BACK_OFF: f32 = 7.;
/// Seconds she then steams straight out from the shore that held her, before she takes
/// up her course again, so she has the way on to turn and does not drive back onto it.
const STAND_OUT: f32 = 5.;
/// Seconds between her checks that she is getting somewhere, and the metres she must
/// have made good since the last. Short of it she is hemmed in a pocket of the shore, and
/// stands out along the most open water she can see before trying her course again.
const PROGRESS_CHECK: f32 = 20.;
const PROGRESS: f32 = 30.;
const WORK_CLEAR: f32 = 10.;
/// A torpedo track passing this close to her is one to comb.
const DODGE_REACH: f32 = 20.;
/// She turns for a torpedo this many seconds before it would reach her, about when
/// the alarm rings aboard.
const DODGE_WARNING: f32 = 6.;
/// Seconds she holds the combing heading once a torpedo is seen.
const DODGE_HOLD: f32 = 3.;
/// Seconds she keeps away from a shore that has held her.
const KEEP_OFF: f32 = 8.;
/// How far she wants clear water ahead on a fighting course.
const SEA_ROOM: f32 = 250.;

impl Bot {
    pub fn new(temperament: Temperament) -> Self {
        Self::skilled(temperament, Skill::SHARP, 0)
    }

    /// A captain of `skill`, whose laying errors are drawn from `seed`.
    pub fn skilled(temperament: Temperament, skill: Skill, seed: u64) -> Self {
        Self {
            temperament,
            skill,
            dice: seed ^ 0x5A1D_0B07,
            laying: Vec2::ZERO,
            laid_on: None,
            settle: 0.,
            stand_off: BOMBARD_RANGE,
            standing_off: None,
            progress: (Vec2::ZERO, 0.),
            think: 0.,
            torpedo_clock: 0.,
            goal: None,
            desired: 0.,
            intent: Intent::Closing,
            target: None,
            fleeing: false,
            aground: 0.,
            backing: 0.,
            shore: Vec2::ZERO,
            keep_off: 0.,
            escape: 0.,
            dodge: 0.,
            dodge_heading: 0.,
        }
    }

    /// Ring the telegraph to full ahead. She fights at speed.
    pub fn work_up(helm: &mut Helm) {
        helm.ring(8);
    }

    /// The card she takes from a hand: the first of her favourites on offer, else the
    /// first dealt. Her favourites are a player's, in the order the run logs show them
    /// taken: loaders well ahead of all else, then plate, then the secondaries that
    /// clear the small craft round her, and velocity.
    pub fn choose(offers: &[Card]) -> Option<Card> {
        const FAVOURITES: [Card; 12] = [
            Card::Loader,
            Card::Armor,
            Card::SecondaryRange,
            Card::HeavySecondaries,
            Card::TwinSecondaries,
            Card::Velocity,
            Card::MoreSecondaries,
            Card::QuickSecondaries,
            Card::Caps,
            Card::Hardening,
            Card::TrainingGear,
            Card::Turbines,
        ];
        FAVOURITES
            .iter()
            .find(|card| offers.contains(card))
            .or(offers.first())
            .copied()
    }

    /// Her orders for the next step. `held` is how the shore pushed her off on the step
    /// just sailed, if it did.
    pub fn decide(
        &mut self,
        run: &Run,
        battle: &Battle,
        helm: &Helm,
        held: Option<Vec2>,
        dt: f32,
    ) -> Command {
        if let Some(push) = held {
            self.shore = push.normalize_or_zero();
            self.keep_off = KEEP_OFF;
        } else {
            self.keep_off = (self.keep_off - dt).max(0.);
        }
        let at = helm.position;
        let hull = run.hull_percent();
        let crowd = combatants_within(battle, at, 600.);
        self.fleeing = match self.temperament {
            Temperament::Bold => false,
            Temperament::Steady => hull < 35,
            Temperament::Cautious => hull < 60 || crowd >= 3,
        };
        let shell = run.fittings.round();
        let reach = shell.reach() * 0.85;
        // Pick a ship to fire on every step, so the sight never lags the target.
        let quarry = battle
            .raiders()
            .iter()
            .filter(|r| r.afloat() && r.body.pos.distance(at) <= reach)
            .filter(|r| battle.open_water(at, r.body.pos))
            .min_by(|a, b| threat_rank(a, at).total_cmp(&threat_rank(b, at)));
        self.target = quarry.map(|r| r.body.id);
        // She settles her laying afresh now and then, and on each new mark.
        self.settle -= dt;
        if self.settle <= 0. || self.laid_on != self.target {
            self.settle = AIM_SETTLE;
            self.laid_on = self.target;
            self.laying = Vec2::new(self.normal(), self.normal());
        }
        // The nearest of the works still standing: a drydock, or a factory that could
        // raise one again. Its port, where it stands, and its height.
        let yard = battle
            .ports()
            .iter()
            .flat_map(|p| {
                let docks = (0..p.docks().len())
                    .filter(|&i| p.docks()[i].live())
                    .map(move |i| (p, p.dock_position(i), p.dock_top(i) * 0.5));
                let factories = (0..p.production_works().len())
                    .filter(|&i| p.production_works()[i].live())
                    .map(move |i| {
                        let height = if p.industrial_yard().is_some() {
                            (p.factory_bottom(i) + p.factory_top(i)) * 0.5
                        } else {
                            p.factory_top(i) * 0.5
                        };
                        (p, p.factory_position(i), height)
                    });
                // An airfield stands until its hangars are down as well.
                let hangars = (p.airfield().into_iter()).flat_map(move |field| {
                    (0..shipvivor_encounter::HANGARS.len())
                        .filter(|&i| field.hangar_live(i))
                        .map(move |i| (p, field.hangar_position(i), field.hangar_top(i) * 0.5))
                });
                docks.chain(factories).chain(hangars)
            })
            .min_by(|a, b| {
                a.1.distance_squared(at)
                    .total_cmp(&b.1.distance_squared(at))
            });
        // A light shore gun of that yard's base, still firing, comes down first: it is
        // quick to silence and keeps hitting her while she works on the yard.
        let gun = yard.and_then(|(p, _, _)| {
            (p.turrets().iter())
                .filter(|g| g.live() && !g.heavy_battery())
                .map(|g| (p, g.position(&p.body), g.aim_height()))
                .min_by(|a, b| {
                    a.1.distance_squared(at)
                        .total_cmp(&b.1.distance_squared(at))
                })
        });
        let yard_target = gun
            .filter(|(_, spot, _)| spot.distance(at) <= reach)
            .or(yard);
        // A warship close aboard comes first. Otherwise a yard in range is worth more
        // than the light craft and freighters round it.
        let pressing =
            quarry.filter(|r| !r.class.is_merchant() && r.body.pos.distance(at) <= PRESSING);
        // The yard stands on its island, so the line of fire is judged to its slip,
        // which opens onto the water.
        let yard_in_reach = yard_target.filter(|(p, works, _)| {
            works.distance(at) <= reach && battle.open_water(at, p.berth())
        });
        let gun_target = match (pressing, yard_in_reach) {
            (Some(r), _) => Some(r),
            (None, Some(_)) => None,
            (None, None) => quarry,
        };
        let aim = match (gun_target, yard_in_reach) {
            (Some(r), _) => Aim::Sight(self.lay(lead(r, at, shell.speed()), at, 1.)),
            (None, Some((_, works, height))) => {
                // A yard stands still, so she lays on it closer.
                Aim::Sight(self.lay(works.extend(height).xzy(), at, 0.5))
            }
            _ => Aim::Hold,
        };
        // Torpedoes go at a ship lying within either rack's arc and inside their run.
        self.torpedo_clock -= dt;
        let mut torpedo = None;
        if self.torpedo_clock <= 0.
            && let Some(r) =
                quarry.filter(|r| r.body.pos.distance(at) < battle.torpedo_range() * 0.85)
        {
            let bearing = heading(lead(r, at, TORPEDO_SPEED).xz() - at);
            let loaded = battle
                .tubes_for_aim(helm.heading, bearing)
                .is_some_and(|set| battle.tube_sets()[set].loaded());
            if loaded {
                torpedo = Some(bearing);
                self.torpedo_clock = 1.;
            }
        }
        self.think -= dt;
        if self.think <= 0. {
            self.think = self.skill.think();
            // While a yard stands she steers for it, and fights what closes her on the
            // way with her guns alone. A captain who chases every boat never gets there.
            self.plan(
                battle,
                helm,
                pressing.filter(|_| yard.is_none()),
                yard.map(|(p, works, _)| (works, p.berth())),
            );
        }
        let firing = !matches!(aim, Aim::Hold);
        let intent = if self.fleeing {
            Intent::Fleeing
        } else if firing {
            Intent::Fighting
        } else {
            self.intent
        };
        // Held against the land she has no way on and cannot turn, so she backs off it as
        // a captain would, with the rudder reversed to swing her bow towards her heading.
        // Anything else that holds her with full ahead rung, a quay or a hull at anchor,
        // she treats the same way.
        let stalled = self.backing <= 0. && helm.speed.abs() < STALLED;
        self.aground = if (held.is_some() && helm.speed.abs() < 4.) || stalled {
            self.aground + dt
        } else {
            (self.aground - dt).max(0.)
        };
        if self.backing > 0. {
            self.backing -= dt;
            if self.backing <= 0. && self.shore != Vec2::ZERO {
                self.escape = STAND_OUT;
            }
            // Her stern has found the land too: steam out ahead instead.
            if held.is_some() && helm.speed < 0. {
                self.backing = 0.;
                self.escape = BACK_OFF;
            }
        } else if self.aground > AGROUND {
            self.backing = BACK_OFF;
            // Caught by the shore near the yard she is working on: stand further off it.
            if self
                .goal
                .is_some_and(|g| g.distance(at) < self.stand_off + 120.)
                && self.standing_off.is_some()
            {
                self.stand_off = (self.stand_off + STAND_OFF_STEP).min(STAND_OFF_MOST);
            }
            self.aground = 0.;
        }
        self.escape = (self.escape - dt).max(0.);
        self.progress.1 += dt;
        if self.progress.1 >= PROGRESS_CHECK {
            if at.distance(self.progress.0) < PROGRESS && !self.fleeing {
                self.shore = direction(open_heading(battle, at, self.desired));
                self.backing = 0.;
                self.escape = WORK_CLEAR;
            }
            self.progress = (at, 0.);
        }
        // A torpedo whose track will pass close is combed: she turns end-on to it, the
        // nearer way round, as a captain on the bridge would at the alarm.
        let way = helm.forward() * helm.speed;
        let incoming = battle
            .torpedoes()
            .iter()
            .filter(|t| t.hostile())
            .filter_map(|t| t.closing(at, way, DODGE_REACH).map(|s| (s, t)))
            .filter(|(seconds, _)| *seconds < self.skill.dodge_warning())
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, torpedo)) = incoming {
            let along = heading(torpedo.body.forward());
            self.dodge_heading = [along, along + PI]
                .into_iter()
                .min_by(|a, b| {
                    turn(helm.heading, *a)
                        .abs()
                        .total_cmp(&turn(helm.heading, *b).abs())
                })
                .unwrap_or(along)
                .rem_euclid(TAU);
            self.dodge = DODGE_HOLD;
        }
        self.dodge = (self.dodge - dt).max(0.);
        if self.dodge > 0. {
            self.desired = self.dodge_heading;
        }
        if self.escape > 0. && self.shore != Vec2::ZERO {
            self.desired = heading(self.shore);
        }
        let astern = self.backing > 0.;
        let error = turn(helm.heading, self.desired);
        let rudder = (error / 0.45).clamp(-1., 1.);
        Command {
            rudder: if astern { -rudder } else { rudder },
            notch: if astern { -3 } else { 4 },
            aim,
            torpedo,
            intent,
        }
    }

    /// `mark` as she lays on it from `at`: off by her laying error, `share` of it.
    fn lay(&self, mark: Vec3, at: Vec2, share: f32) -> Vec3 {
        let off = mark.xz() - at;
        let along = off.normalize_or_zero();
        let error = self.skill.aim_error() * share * off.length() / 100.;
        let wander =
            along * self.laying.x * error + along.perp() * self.laying.y * error * AIM_ACROSS;
        mark + wander.extend(0.).xzy()
    }

    /// A draw from the standard normal distribution, off her own dice.
    fn normal(&mut self) -> f32 {
        let mut uniform = || {
            self.dice = self.dice.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.dice;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            ((z ^ (z >> 31)) >> 40) as f32 / (1u64 << 24) as f32
        };
        let (u, v) = (uniform().max(1e-7), uniform());
        (-2. * u.ln()).sqrt() * (TAU * v).cos()
    }

    /// Where she is bound and the heading that takes her there clear of the land.
    fn plan(
        &mut self,
        battle: &Battle,
        helm: &Helm,
        quarry: Option<&Raider>,
        yard: Option<(Vec2, Vec2)>,
    ) {
        let at = helm.position;
        let (wanted, intent) = if self.fleeing {
            let threats: Vec<Vec2> = battle
                .raiders()
                .iter()
                .filter(|r| r.afloat() && !r.class.is_merchant())
                .map(|r| r.body.pos)
                .filter(|p| p.distance(at) < THREAT_RANGE * 1.5)
                .collect();
            if threats.is_empty() {
                (helm.heading, Intent::Fleeing)
            } else {
                let centre = threats.iter().copied().sum::<Vec2>() / threats.len() as f32;
                let away = at + (at - centre).normalize_or(helm.forward()) * 400.;
                self.goal = Some(away);
                (heading(battle.pilot(at, away) - at), Intent::Fleeing)
            }
        } else if let Some(r) = quarry {
            self.goal = Some(r.body.pos);
            (
                broadside(battle, helm.heading, at, r.body.pos, FIGHTING_RANGE),
                Intent::Closing,
            )
        } else if let Some((yard, slip)) = yard {
            // She shells it from off its slip, where the water runs out to the fairway.
            // A new yard: she runs in close again.
            if self.standing_off.is_none_or(|w| w.distance(yard) > 1.) {
                self.standing_off = Some(yard);
                self.stand_off = BOMBARD_RANGE;
            }
            let range = self.stand_off;
            let station = yard + (slip - yard).normalize_or(at - yard) * range;
            self.goal = Some(station);
            if station.distance(at) > 120. && yard.distance(at) > range + 60. {
                (heading(battle.pilot(at, station) - at), Intent::Closing)
            } else {
                (
                    broadside(battle, helm.heading, at, yard, range),
                    Intent::Closing,
                )
            }
        } else if let Some(r) = battle
            .raiders()
            .iter()
            .filter(|r| r.afloat())
            .min_by(|a, b| a.body.pos.distance(at).total_cmp(&b.body.pos.distance(at)))
        {
            self.goal = Some(r.body.pos);
            (heading(battle.pilot(at, r.body.pos) - at), Intent::Closing)
        } else {
            self.goal = None;
            (helm.heading, Intent::Idle)
        };
        // A shore that has just held her is kept on the quarter for a while.
        let wanted = if self.keep_off > 0. && self.shore != Vec2::ZERO {
            heading((self.shore * 1.2 + direction(wanted)).normalize_or(self.shore))
        } else {
            wanted
        };
        self.desired = clear_heading(battle, at, wanted);
        self.intent = intent;
    }
}

/// Nearer and heavier ships first, and anything already hit hard last.
fn threat_rank(r: &Raider, at: Vec2) -> f32 {
    let weight = match r.class {
        EnemyClass::Freighter | EnemyClass::Coaster => 1.6,
        EnemyClass::Patrol => 1.,
        // A torpedo boat is quick to sink but no great matter: her torpedo is the threat.
        EnemyClass::TorpedoBoat => 1.3,
        EnemyClass::Destroyer => 0.85,
        EnemyClass::Cruiser => 0.8,
        _ => 0.9,
    };
    r.body.pos.distance(at) * weight
}

/// Where to lay the guns on a ship: her mark, ahead by her motion over the round's
/// flight, as the fire-control ghost shows it.
fn lead(r: &Raider, at: Vec2, speed: f32) -> Vec3 {
    let seconds = at.distance(r.body.pos) / speed;
    let mark = r.aim_point(at);
    mark + (r.motion() * seconds).extend(0.).xzy()
}

/// The heading that keeps `target` on her beam, edging in when she is beyond `range`
/// and out when inside it. Of the two beams she takes the one with sea room ahead, and
/// the nearer turn when both or neither have it.
fn broadside(battle: &Battle, current: f32, at: Vec2, target: Vec2, range: f32) -> f32 {
    let bearing = heading(target - at);
    let tilt = ((at.distance(target) - range) / range).clamp(-1., 1.) * 0.6;
    let course = |base: f32| (base + turn(base, bearing).signum() * tilt).rem_euclid(TAU);
    let room = |h: f32| battle.open_water(at, at + direction(h) * SEA_ROOM);
    [course(bearing + FRAC_PI_2), course(bearing - FRAC_PI_2)]
        .into_iter()
        .min_by(|a, b| {
            (!room(*a), turn(current, *a).abs())
                .partial_cmp(&(!room(*b), turn(current, *b).abs()))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(bearing)
}

/// The nearest heading to `wanted` with open water ahead, looking well ahead first and
/// then only a little way when she is hemmed in.
fn clear_heading(battle: &Battle, at: Vec2, wanted: f32) -> f32 {
    for reach in [LOOKOUT * 2.2, LOOKOUT] {
        for step in 0..11 {
            for side in [1., -1.] {
                let candidate = wanted + side * step as f32 * 0.3;
                if battle.open_water(at, at + direction(candidate) * reach) {
                    return candidate.rem_euclid(TAU);
                }
                if step == 0 {
                    break;
                }
            }
        }
    }
    wanted
}

/// The heading with the most open water ahead of her, out to a few hundred metres, and
/// of those the nearest `wanted`.
fn open_heading(battle: &Battle, at: Vec2, wanted: f32) -> f32 {
    (0..24)
        .map(|i| i as f32 * TAU / 24.)
        .map(|h| {
            let run = [240., 160., 80., 40.]
                .into_iter()
                .find(|&reach| battle.open_water(at, at + direction(h) * reach))
                .unwrap_or(0.);
            (run - 20. * turn(wanted, h).abs(), h)
        })
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map_or(wanted, |(_, h)| h)
}

/// Signed turn from `from` to `to`, the short way round, in radians.
pub fn turn(from: f32, to: f32) -> f32 {
    (to - from + PI).rem_euclid(TAU) - PI
}

/// Enemy warships afloat within `range` of her.
pub fn combatants_within(battle: &Battle, at: Vec2, range: f32) -> usize {
    battle
        .raiders()
        .iter()
        .filter(|r| r.afloat() && !r.class.is_merchant() && r.moored_at().is_none())
        .filter(|r| r.body.pos.distance(at) <= range)
        .count()
}
