//! Breakwater's arcade torpedo spreads, from whatever tubes her sections carry: fixed
//! in the bow and firing ahead, fixed in the stern and firing astern, and trained to
//! either beam amidships. The sight and the tubes share each set's sector. The enemy's
//! torpedo boats fire the same torpedo, one at a time, at her.
use super::torpedo_boat::WARHEAD;
use super::{Battle, Blow, Body, Effect, EffectKind, Events, body, entry};
use glam::Vec2;
use shipvivor_compass::direction;
use shipvivor_shipyard::{Design, Ship, TubeAim, TubeMount};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

pub const TORPEDO_RANGE: f32 = 420.;
pub const TORPEDO_SPEED: f32 = 38.;
/// Half the sector a set of tubes amidships covers on either beam.
pub const RACK_HALF_ARC: f32 = 50_f32.to_radians();
/// Half the sector fixed bow or stern tubes cover: a torpedo's gyro turns it onto any
/// course this far off dead ahead, or dead astern, once it is in the water.
pub const FIXED_HALF_ARC: f32 = 30_f32.to_radians();
pub const SPREAD_HALF_ARC: f32 = 4_f32.to_radians();
/// Seconds a set of tubes takes to load again after a spread.
pub const TUBE_RELOAD: f32 = 14.;
/// Metres out from the centreline that tubes amidships fire from, at the stock beam.
const BEAM_TUBES_ABEAM: (f32, f32) = (1.95, 0.75);
/// Integrity a torpedo's warhead takes off the ice it strikes: enough to split the
/// biggest slab at a blow.
const WARHEAD_ON_ICE: f32 = 12.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rack {
    Port,
    Starboard,
}
impl Rack {
    pub const BOTH: [Self; 2] = [Self::Port, Self::Starboard];
    pub fn center(self, heading: f32) -> f32 {
        heading
            + if self == Self::Port {
                -FRAC_PI_2
            } else {
                FRAC_PI_2
            }
    }
    pub fn for_aim(heading: f32, aim: f32) -> Option<Self> {
        Self::BOTH.into_iter().find(|side| {
            let delta = (aim - side.center(heading) + PI).rem_euclid(TAU) - PI;
            delta.abs() <= RACK_HALF_ARC - SPREAD_HALF_ARC
        })
    }
}
/// A set of torpedo tubes aboard Breakwater, and how near it is to firing again.
#[derive(Clone, Debug, PartialEq)]
pub struct TubeSet {
    pub mount: TubeMount,
    /// Seconds until it can fire again.
    reload: f32,
    /// Its tubes are sound enough to fire.
    pub in_service: bool,
}
impl TubeSet {
    fn new(mount: TubeMount) -> Self {
        Self {
            mount,
            reload: 0.,
            in_service: true,
        }
    }
    pub fn reload(&self) -> f32 {
        self.reload
    }
    pub fn loaded(&self) -> bool {
        self.in_service && self.reload == 0.
    }
    /// Half the sector it covers about each of its [`TubeSet::centres`].
    pub fn half_arc(&self) -> f32 {
        match self.mount.aim {
            TubeAim::Beam => RACK_HALF_ARC,
            TubeAim::Ahead | TubeAim::Astern => FIXED_HALF_ARC,
        }
    }
    /// The middle of each sector it covers on a ship steering `heading`: dead ahead,
    /// dead astern, or both beams.
    pub fn centres(&self, heading: f32) -> Vec<f32> {
        match self.mount.aim {
            TubeAim::Ahead => vec![heading],
            TubeAim::Astern => vec![heading + PI],
            TubeAim::Beam => Rack::BOTH.map(|side| side.center(heading)).to_vec(),
        }
    }
    /// The middle of the sector it would fire a spread at `aim` from, if any: a whole
    /// spread must fit inside the sector.
    pub fn centre_for(&self, heading: f32, aim: f32) -> Option<f32> {
        let reach = self.half_arc() - SPREAD_HALF_ARC;
        self.centres(heading)
            .into_iter()
            .find(|centre| ((aim - centre + PI).rem_euclid(TAU) - PI).abs() <= reach)
    }
}

#[derive(Clone)]
pub struct Torpedo {
    pub body: Body,
    remaining: f32,
    /// The torpedo boat that fired it, or `None` for one of Breakwater's own.
    pub fired_by: Option<u64>,
}
impl Torpedo {
    /// Fired at Breakwater by a torpedo boat. It runs for her and for nothing else.
    pub fn hostile(&self) -> bool {
        self.fired_by.is_some()
    }
    /// Metres it has run since it left the tube.
    pub fn run(&self) -> f32 {
        TORPEDO_RANGE - self.remaining
    }
    /// Seconds until it passes closest to a ship at `ship` making `way`, if it will pass
    /// within `reach` metres of her before it runs out. Neither turns in the meantime.
    pub fn closing(&self, ship: Vec2, way: Vec2, reach: f32) -> Option<f32> {
        let relative = self.body.forward() * TORPEDO_SPEED - way;
        let offset = self.body.pos - ship;
        let seconds = -offset.dot(relative) / relative.length_squared().max(1e-6);
        let miss = (offset + relative * seconds).length();
        (seconds > 0. && seconds * TORPEDO_SPEED <= self.remaining && miss <= reach)
            .then_some(seconds)
    }
}

/// Where a torpedo fired from `from` at `speed` meets a ship at `ship` holding her
/// course and making `way`, or `None` if it can never catch her.
pub fn intercept(from: Vec2, ship: Vec2, way: Vec2, speed: f32) -> Option<Vec2> {
    // |ship + way t - from| = speed t, solved for the first t after the launch.
    let offset = ship - from;
    let a = way.length_squared() - speed * speed;
    let b = 2. * offset.dot(way);
    let c = offset.length_squared();
    let seconds = if a.abs() < 1e-6 {
        (b < 0.).then(|| -c / b)?
    } else {
        let disc = b * b - 4. * a * c;
        if disc < 0. {
            return None;
        }
        let root = disc.sqrt();
        [(-b - root) / (2. * a), (-b + root) / (2. * a)]
            .into_iter()
            .filter(|t| *t > 0.)
            .min_by(f32::total_cmp)?
    };
    Some(ship + way * seconds)
}

impl Battle {
    /// Fit the tubes `design` carries. A set that stands where one stood before keeps
    /// its reload.
    pub(super) fn fit_tubes(&mut self, design: &Design) {
        let old = std::mem::take(&mut self.tubes);
        self.tubes = design
            .tube_mounts()
            .into_iter()
            .map(|mount| {
                old.iter()
                    .find(|set| set.mount == mount)
                    .cloned()
                    .unwrap_or_else(|| TubeSet::new(mount))
            })
            .collect();
    }
    /// Tubes knocked out by a hit cannot fire until they are mended.
    pub(super) fn tube_service(&mut self, ship: &Ship) {
        for set in &mut self.tubes {
            set.in_service = ship.ready(set.mount.berth);
        }
    }
    /// Every set of tubes aboard, from the bow aft.
    pub fn tube_sets(&self) -> &[TubeSet] {
        &self.tubes
    }
    /// The set of tubes that would fire a spread at `aim`: a loaded one whose sector
    /// takes it, or failing that the one in service that will be loaded soonest.
    pub fn tubes_for_aim(&self, heading: f32, aim: f32) -> Option<usize> {
        self.tubes
            .iter()
            .enumerate()
            .filter(|(_, set)| set.in_service && set.centre_for(heading, aim).is_some())
            .min_by(|a, b| a.1.reload.total_cmp(&b.1.reload))
            .map(|(i, _)| i)
    }
    /// Where a spread from tube set `set`, aimed at `aim`, leaves the ship at `player`
    /// steering `heading`: tubes amidships fire from the beam the spread goes out on.
    pub fn tube_origin(&self, set: usize, player: Vec2, heading: f32, aim: f32) -> Vec2 {
        let set = &self.tubes[set];
        let along = player - direction(heading) * (set.mount.aft - self.keel.1.bow);
        match (set.mount.aim, set.centre_for(heading, aim)) {
            (TubeAim::Beam, Some(beam)) => {
                let (out, rail) = BEAM_TUBES_ABEAM;
                along + direction(beam) * (out * self.deck.beam_scale + rail)
            }
            _ => along,
        }
    }
    pub fn torpedoes(&self) -> &[Torpedo] {
        &self.torpedoes
    }
    /// Fire a spread of three at `aim` from a loaded set of tubes whose sector takes it.
    /// Returns whether one fired.
    pub fn launch_spread(&mut self, player: Vec2, heading: f32, aim: f32) -> bool {
        let Some(set) = self.tubes_for_aim(heading, aim) else {
            return false;
        };
        if !self.tubes[set].loaded() {
            return false;
        }
        let origin = self.tube_origin(set, player, heading, aim);
        for offset in [-SPREAD_HALF_ARC, 0., SPREAD_HALF_ARC] {
            self.torpedoes.push(Torpedo {
                body: body(&mut self.serial, origin, aim + offset),
                remaining: TORPEDO_RANGE,
                fired_by: None,
            });
        }
        self.effects.push(Effect {
            body: body(&mut self.serial, origin, aim),
            kind: EffectKind::Splash,
            age: 0.,
            height: 0.2,
        });
        self.tubes[set].reload = TUBE_RELOAD;
        true
    }
    /// A torpedo boat at `from`, with the id `boat`, puts her one torpedo in the water on
    /// `heading`.
    pub(super) fn fire_torpedo(&mut self, from: Vec2, heading: f32, boat: u64) {
        self.torpedoes.push(Torpedo {
            body: body(&mut self.serial, from, heading),
            remaining: TORPEDO_RANGE,
            fired_by: Some(boat),
        });
    }
    /// Run every torpedo on for a step. Breakwater's strike the enemy's ships, and the
    /// enemy's strike her, at `player`, and pass under their own side. Land and ice
    /// stop both.
    pub(super) fn advance_torpedoes(&mut self, dt: f32, player: Vec2, events: &mut Events) {
        for set in &mut self.tubes {
            set.reload = (set.reload - dt).max(0.);
        }
        let (forward, keel) = self.keel;
        let outline = keel.outline().map(Vec2::from);
        let local =
            |p: Vec2| Vec2::new((p - player).dot(forward.perp()), -(p - player).dot(forward));
        self.torpedoes.retain_mut(|fish| {
            let travel = (dt * TORPEDO_SPEED).min(fish.remaining);
            let from = fish.body.pos;
            let to = from + fish.body.forward() * travel;
            let land = self
                .ports
                .iter()
                .flat_map(|p| p.shores())
                .chain(self.landmarks.iter().map(|l| (l.body.pos, l.radius)))
                .filter_map(|(at, r)| entry(from, to, at, r))
                .min_by(f32::total_cmp);
            // `None` for the ship it is running at: Breakwater, for an enemy's torpedo.
            let hull = if fish.hostile() {
                let start = local(from);
                super::plating::clip_hull(&outline, start, local(to) - start)
                    .map(|(enter, _)| (enter, None))
            } else {
                self.raiders
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| r.afloat())
                    .filter_map(|(i, r)| Some((r.hull_span(from, to)?.0, Some(i))))
                    .min_by(|a, b| a.0.total_cmp(&b.0))
            };
            // Most of a berg lies under water, so a running torpedo meets its whole width.
            let ice = self
                .floe
                .icebergs
                .iter()
                .enumerate()
                .filter_map(|(i, b)| Some((entry(from, to, b.body.pos, b.radius)?, i)))
                .min_by(|a, b| a.0.total_cmp(&b.0));
            let struck = match (hull, ice) {
                (Some((t, ship)), _)
                    if land.is_none_or(|l| t < l) && ice.is_none_or(|(c, _)| t < c) =>
                {
                    let at = from.lerp(to, t);
                    if let Some(i) = ship {
                        self.raiders[i].hit_hull(at, 6.);
                        events.hits += 1;
                        Some((t, EffectKind::Burst))
                    } else {
                        // A blow under her waterline that no plate turns. It opens the
                        // bays of the module it lands on to the sea, and the sea pours
                        // into the module until the hole is shored.
                        let side = local(at);
                        events.blows.push(Blow {
                            aft: (keel.bow + side.y).clamp(0., keel.bow + keel.stern),
                            abeam: side.x,
                            damage: WARHEAD,
                            calibre: None,
                            reach: shipvivor_shipyard::Reach::Bays,
                            breach: true,
                        });
                        events.damage = events.damage.saturating_add(WARHEAD);
                        events.torpedoed += 1;
                        Some((t, EffectKind::Torpedoed))
                    }
                }
                (_, Some((t, i))) if land.is_none_or(|l| t < l) => {
                    self.floe.icebergs[i].strike(WARHEAD_ON_ICE, to - from);
                    Some((t, EffectKind::Burst))
                }
                _ => land.map(|t| (t, EffectKind::Splash)),
            };
            if let Some((t, kind)) = struck {
                if kind == EffectKind::Burst {
                    self.effects.push(Effect {
                        body: body(&mut self.serial, from.lerp(to, t), fish.body.heading),
                        age: 0.,
                        height: 0.2,
                        kind: EffectKind::Splash,
                    });
                }
                self.effects.push(Effect {
                    body: body(&mut self.serial, from.lerp(to, t), fish.body.heading),
                    age: 0.,
                    height: if kind == EffectKind::Torpedoed {
                        0.
                    } else {
                        0.7
                    },
                    kind,
                });
                return false;
            }
            fish.body.pos = to;
            fish.remaining -= travel;
            fish.remaining > 0.
        });
    }
}
