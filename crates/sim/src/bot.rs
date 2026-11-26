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
use shipvivor_encounter::{
    Aim, Ammo, Battle, EnemyClass, RACK_HALF_ARC, Raider, TORPEDO_RANGE, TORPEDO_SPEED,
};
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

/// Ships within this range count as close threats.
pub const THREAT_RANGE: f32 = 800.;
/// Range she prefers to fight at, broadside on.
const FIGHTING_RANGE: f32 = 360.;
/// A warship this close is fought before anything else.
const PRESSING: f32 = 400.;
/// She stands this far off a yard while she shells it.
const BOMBARD_RANGE: f32 = 420.;
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
    pub ammo: Ammo,
    /// A bearing for a torpedo spread.
    pub torpedo: Option<f32>,
    pub intent: Intent,
}

pub struct Bot {
    pub temperament: Temperament,
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
/// Seconds she backs off for.
const BACK_OFF: f32 = 4.;
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
        Self {
            temperament,
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
    /// first dealt.
    pub fn choose(offers: &[Card]) -> Option<Card> {
        const FAVOURITES: [Card; 8] = [
            Card::Loader,
            Card::Armor,
            Card::Hardening,
            Card::Turbines,
            Card::TrainingGear,
            Card::Velocity,
            Card::QuickSecondaries,
            Card::MoreSecondaries,
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
        // The nearest of the works still standing: a drydock, or a factory that could
        // raise one again. Its port, where it stands, and its height.
        let yard = battle
            .ports()
            .iter()
            .flat_map(|p| {
                let docks = (0..p.docks().len())
                    .filter(|&i| p.docks()[i].live())
                    .map(move |i| (p, p.dock_position(i), p.dock_top(i)));
                let factories = (0..p.factories().len())
                    .filter(|&i| p.factories()[i].live())
                    .map(move |i| (p, p.factory_position(i), p.factory_top(i)));
                docks.chain(factories)
            })
            .min_by(|a, b| {
                a.1.distance_squared(at)
                    .total_cmp(&b.1.distance_squared(at))
            });
        // A warship close aboard comes first. Otherwise a yard in range is worth more
        // than the light craft and freighters round it.
        let pressing =
            quarry.filter(|r| !r.class.is_merchant() && r.body.pos.distance(at) <= PRESSING);
        // The yard stands on its island, so the line of fire is judged to its slip,
        // which opens onto the water.
        let yard_in_reach = yard.filter(|(p, works, _)| {
            works.distance(at) <= reach && battle.open_water(at, p.berth())
        });
        let gun_target = match (pressing, yard_in_reach) {
            (Some(r), _) => Some(r),
            (None, Some(_)) => None,
            (None, None) => quarry,
        };
        let (aim, ammo) = match (gun_target, yard_in_reach) {
            (Some(r), _) => {
                let lead = lead(r, at, shell.speed());
                let heavy = matches!(
                    r.class,
                    EnemyClass::Cruiser
                        | EnemyClass::Battleship
                        | EnemyClass::Parliament
                        | EnemyClass::Regent
                );
                (
                    Aim::Sight(lead),
                    if heavy && run.fittings.armor_piercing() {
                        Ammo::Ap
                    } else {
                        Ammo::He
                    },
                )
            }
            (None, Some((_, works, top))) => {
                let mark = works.extend(top * 0.5).xzy();
                (Aim::Sight(mark), Ammo::He)
            }
            _ => (Aim::Hold, Ammo::He),
        };
        // Torpedoes go at a ship lying within either rack's arc and inside their run.
        self.torpedo_clock -= dt;
        let mut torpedo = None;
        if self.torpedo_clock <= 0.
            && let Some(r) = quarry.filter(|r| r.body.pos.distance(at) < TORPEDO_RANGE * 0.85)
        {
            let bearing = heading(lead(r, at, TORPEDO_SPEED).xz() - at);
            let off_bow = turn(helm.heading, bearing).abs();
            if (off_bow - FRAC_PI_2).abs() < RACK_HALF_ARC * 0.85 {
                torpedo = Some(bearing);
                self.torpedo_clock = 1.;
            }
        }
        self.think -= dt;
        if self.think <= 0. {
            self.think = THINK;
            self.plan(
                battle,
                helm,
                pressing,
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
        self.aground = if held.is_some() && helm.speed.abs() < 4. {
            self.aground + dt
        } else {
            (self.aground - dt).max(0.)
        };
        if self.backing > 0. {
            self.backing -= dt;
            // Her stern has found the land too: steam out ahead instead.
            if held.is_some() && helm.speed < 0. {
                self.backing = 0.;
                self.escape = BACK_OFF;
            }
        } else if self.aground > AGROUND {
            self.backing = BACK_OFF;
            self.aground = 0.;
        }
        self.escape = (self.escape - dt).max(0.);
        // A torpedo whose track will pass close is combed: she turns end-on to it, the
        // nearer way round, as a captain on the bridge would at the alarm.
        let way = helm.forward() * helm.speed;
        let incoming = battle
            .torpedoes()
            .iter()
            .filter(|t| t.hostile())
            .filter_map(|t| t.closing(at, way, DODGE_REACH).map(|s| (s, t)))
            .filter(|(seconds, _)| *seconds < DODGE_WARNING)
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
            ammo,
            torpedo,
            intent,
        }
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
            let station = yard + (slip - yard).normalize_or(at - yard) * BOMBARD_RANGE;
            self.goal = Some(station);
            if station.distance(at) > 120. && yard.distance(at) > BOMBARD_RANGE + 60. {
                (heading(battle.pilot(at, station) - at), Intent::Closing)
            } else {
                (
                    broadside(battle, helm.heading, at, yard, BOMBARD_RANGE),
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
