//! Squadrons sail as one. A base's squadron weighs together and keeps close order on
//! her guide, the heaviest ship: her escorts on the guide's quarters, the heavier
//! nearer, or in column astern where the water is too narrow for that. Once the guide
//! has the ship in reach across open water, the squadron fans out into a line of battle
//! across her bearing. Each ship lies broadside on at her own fighting range, the
//! lighter ones on the wings and nearer in, and the line wheels about her as the guide
//! does. The squadron signals what it sees, so one ship's sighting is every ship's.
use super::{Battle, EnemyClass, Hull, Raider, Weight, pilotage};
use glam::Vec2;
use std::collections::VecDeque;

/// Share of her true range at which a ship lies in the line of battle. Her guns are
/// laid true there, and she is far enough out that the line has room to fan.
pub const LINE_RANGE: f32 = 0.8;
/// Metres beyond her fighting range at which a guide deploys her squadron into line,
/// and the more it takes to draw the line back into close order.
pub const DEPLOY: f32 = 380.;
const RECALL: f32 = 160.;
/// Share of its slowest ship's speed a squadron makes, so every ship has way in hand
/// to take up and hold her station.
pub const SQUADRON_PACE: f32 = 0.85;
/// Metres between hulls in the line of battle, and in column.
const LINE_SPACE: f32 = 60.;
const COLUMN_SPACE: f32 = 30.;
/// Metres between beams in close order, and the share of their length each rank of
/// escorts drops astern of the one before.
const ABREAST_SPACE: f32 = 48.;
const ECHELON: f32 = 0.45;
/// Seconds the other close order must look right before the squadron changes to it.
const DOUBT: f32 = 3.;
/// Metres apart the guide's track is marked for the ships in column astern of her.
const TRAIL_STEP: f32 = 6.;
/// Metres off her station past which a ship drops her station-keeping and makes for it.
const CATCH_UP: f32 = 120.;
/// Metres ahead of her station along its course that a ship keeping it steers for. The
/// error she corrects at once is no more than this, so she eases into place.
const KEEP_LEAD: f32 = 50.;
/// Metres a second more than her station's pace a ship makes for each metre she lies
/// astern of it, and the least share of her full speed she slows to when ahead.
const TAKE_UP: f32 = 0.05;
const SLOWEST: f32 = 0.12;

/// A ship's part in her squadron for one tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Order {
    /// Lead the squadron at `pace`, a share of her full speed. In line of battle she
    /// holds the ship at `ring` metres, wheeling about her.
    Guide { pace: f32, ring: Option<f32> },
    /// Take up and hold this station on the guide.
    Keep(Station),
}

/// Where a ship keeping station should lie, and the course and speed it is making.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Station {
    pub at: Vec2,
    /// Unit vector of the station's course.
    pub heading: Vec2,
    pub speed: f32,
}
impl Station {
    /// The course she steers from `from` to take up the station and hold it.
    pub(super) fn course(&self, from: Vec2) -> Vec2 {
        let off = self.at - from;
        let along = off.dot(self.heading);
        if off.length() > CATCH_UP && along > 0. {
            return off.normalize_or(self.heading);
        }
        // Close to it, she steers ahead of it along its course, and comes across onto it
        // no more sharply than the lead allows. Ahead of it, she eases and lets it come up.
        let across = off.dot(self.heading.perp());
        (self.heading * (KEEP_LEAD + along.clamp(-0.5 * KEEP_LEAD, KEEP_LEAD))
            + self.heading.perp() * across.clamp(-KEEP_LEAD, KEEP_LEAD))
        .normalize_or(self.heading)
    }
    /// The share of her full speed `full` she makes from `from`: more while she lies
    /// astern of her station, less while she is ahead of it.
    pub(super) fn throttle(&self, from: Vec2, full: f32) -> f32 {
        let off = self.at - from;
        let along = off.dot(self.heading);
        let wanted = if off.length() > CATCH_UP && along > 0. {
            full
        } else {
            self.speed + along * TAKE_UP
        };
        (wanted / full.max(0.1)).clamp(SLOWEST, 1.)
    }
}

/// The ships of one squadron and how it is sailing.
#[derive(Clone, Debug)]
pub(super) struct Squadron {
    /// Body ids of the ships still in it.
    ships: Vec<u64>,
    /// The guide of the last tick, whose track `trail` follows.
    guide: u64,
    /// In line of battle rather than in close order.
    deployed: bool,
    /// In close order, in column astern of the guide rather than spread on her quarters.
    column: bool,
    /// Seconds the other close order has looked right.
    doubt: f32,
    /// Where the guide has been, newest first, a [`TRAIL_STEP`] apart.
    trail: VecDeque<Vec2>,
}
impl Squadron {
    pub(super) fn of(ships: Vec<u64>) -> Self {
        Self {
            guide: ships.first().copied().unwrap_or_default(),
            ships,
            deployed: false,
            column: false,
            doubt: 0.,
            trail: VecDeque::new(),
        }
    }
    pub(super) fn ships(&self) -> &[u64] {
        &self.ships
    }
    pub(super) fn deployed(&self) -> bool {
        self.deployed
    }
}

/// How heavy a ship of `class` and `weight` is, the heaviest first: the order of a
/// squadron's line, whose guide is its heaviest ship.
fn precedence(class: EnemyClass, weight: Weight) -> (u8, Weight) {
    let rank = match class {
        EnemyClass::Regent => 9,
        EnemyClass::Parliament => 8,
        EnemyClass::Commodore => 7,
        EnemyClass::Battleship => 6,
        EnemyClass::Cruiser => 5,
        EnemyClass::Destroyer => 4,
        EnemyClass::Patrol => 3,
        EnemyClass::TorpedoBoat => 2,
        EnemyClass::Freighter | EnemyClass::Coaster => 1,
    };
    (rank, weight)
}

/// Metres off the ship at which a ship of `class` lies in the line of battle. The
/// Regent keeps her own stand-off, and her line forms on her.
pub fn line_range(class: EnemyClass) -> f32 {
    let p = class.profile();
    if class == EnemyClass::Regent {
        p.stand_off()
    } else {
        LINE_RANGE * p.range()
    }
}

/// Where each ship after the guide lies in close order, in a squadron of `hulls` in
/// line order, the guide first: metres ahead of the guide (astern is negative) and to
/// her starboard (port is negative). The second ship takes the guide's starboard
/// quarter, the third her port quarter, and so on outward, each rank a little further
/// astern than the one inside it.
pub fn close_order(hulls: &[Hull]) -> Vec<(f32, f32)> {
    let mut wings = [(0., 0., hulls[0]); 2];
    hulls[1..]
        .iter()
        .enumerate()
        .map(|(i, hull)| {
            let (astern, abeam, inner) = &mut wings[i % 2];
            *abeam += ABREAST_SPACE + 2. * inner.radius.max(hull.radius);
            *astern += ECHELON * (inner.stern + hull.bow) + 8.;
            *inner = *hull;
            let side = if i % 2 == 0 { 1. } else { -1. };
            (-*astern, side * *abeam)
        })
        .collect()
}

/// Each ship's place along a line, in a squadron of `hulls` in line order, the guide
/// first: metres from the guide along it, to the starboard wing's end (the port wing's
/// is negative). The second ship lies next to the guide on the starboard wing, the
/// third next on the port wing, and so on outward.
fn line_places(hulls: &[Hull], space: f32) -> Vec<f32> {
    let mut ends = [(0_f32, hulls[0]); 2];
    std::iter::once(0.)
        .chain(hulls[1..].iter().enumerate().map(|(i, hull)| {
            let (run, inner) = &mut ends[i % 2];
            *run += inner.bow.max(inner.stern) + hull.bow.max(hull.stern) + space;
            *inner = *hull;
            if i % 2 == 0 { *run } else { -*run }
        }))
        .collect()
}

/// The point `run` metres astern of the newest point along `trail`, and the course
/// there, or `None` when the trail is shorter.
fn along_trail(trail: &VecDeque<Vec2>, mut run: f32) -> Option<(Vec2, Vec2)> {
    for (newer, older) in trail.iter().zip(trail.iter().skip(1)) {
        let leg = newer.distance(*older);
        if run <= leg && leg > 0. {
            let course = (*newer - *older) / leg;
            return Some((newer.lerp(*older, run / leg), course));
        }
        run -= leg;
    }
    None
}

impl Battle {
    /// Form a squadron of the ships with these ids, if there are two or more.
    pub(super) fn form_squadron(&mut self, ships: Vec<u64>) {
        if ships.len() >= 2 {
            self.squadrons.push(Squadron::of(ships));
        }
    }

    /// A squadron signals what it sees: once any of its ships has the ship in sight or
    /// by radio, every one of them has, and they all know where she is.
    pub(super) fn signal_squadrons(&mut self, player: Vec2) {
        for squadron in &self.squadrons {
            let member = |r: &Raider| squadron.ships.contains(&r.body.id);
            if self.raiders.iter().any(|r| member(r) && r.has_contact) {
                for r in self.raiders.iter_mut().filter(|r| member(r)) {
                    r.has_contact = true;
                    r.last_sighting = Some(player);
                }
            }
        }
    }

    /// Give every ship in a squadron her order for this tick: the guide her pace and
    /// her range, the rest their stations on her. A squadron at anchor weighs together
    /// once any of it has weighed. Ships sunk or disabled fall out, and a squadron of
    /// one is a ship on her own.
    pub(super) fn keep_formation(&mut self, dt: f32, player: Vec2) {
        for raider in &mut self.raiders {
            raider.order = None;
        }
        let mut squadrons = std::mem::take(&mut self.squadrons);
        let ground = self.patrol_ground();
        let mut orders: Vec<(usize, Order)> = Vec::new();
        for squadron in &mut squadrons {
            let find = |id: u64| self.raiders.iter().position(|r| r.body.id == id);
            squadron.ships.retain(|&id| {
                find(id).is_some_and(|i| self.raiders[i].afloat() && !self.raiders[i].is_disabled())
            });
            if squadron.ships.len() < 2 {
                continue;
            }
            let mut line: Vec<usize> = squadron.ships.iter().filter_map(|&id| find(id)).collect();
            if line.iter().any(|&i| self.raiders[i].moored.is_none()) {
                for &i in &line {
                    self.raiders[i].moored.take();
                }
            }
            if self.raiders[line[0]].moored.is_some() {
                continue;
            }
            line.sort_by_key(|&i| {
                let r = &self.raiders[i];
                (
                    std::cmp::Reverse(precedence(r.class, r.weight())),
                    r.body.id,
                )
            });
            let guide = &self.raiders[line[0]];
            let (at, forward) = (guide.body.pos, guide.body.forward());
            if squadron.guide != guide.body.id {
                squadron.guide = guide.body.id;
                squadron.trail.clear();
            }
            if squadron
                .trail
                .front()
                .is_none_or(|p| p.distance(at) >= TRAIL_STEP)
            {
                squadron.trail.push_front(at);
            }
            let hulls: Vec<Hull> = line
                .iter()
                .map(|&i| self.raiders[i].class.profile().hull)
                .collect();
            let reach: f32 = line_places(&hulls, COLUMN_SPACE)
                .iter()
                .map(|run| run.abs())
                .sum::<f32>()
                + 60.;
            squadron
                .trail
                .truncate((reach / TRAIL_STEP).ceil() as usize + 2);

            // She deploys only with the ship in reach across open water.
            let range = line_range(guide.class);
            let off = at.distance(player);
            let bound = guide.last_sighting.unwrap_or(ground);
            let piloting = pilotage(&self.fairway, &self.landmarks, at, bound) != bound;
            let clear = guide.has_contact && !piloting && self.open_water(at, player);
            let out = if squadron.deployed { RECALL } else { 0. };
            squadron.deployed = clear && off < range + DEPLOY + out;

            let pace = SQUADRON_PACE
                * line
                    .iter()
                    .map(|&i| self.raiders[i].class.profile().speed())
                    .fold(f32::INFINITY, f32::min);
            let speed = guide.way_on().max(0.);
            let ring = (squadron.deployed && guide.class != EnemyClass::Regent).then_some(range);
            orders.push((
                line[0],
                Order::Guide {
                    pace: (pace / guide.class.profile().speed()).min(1.),
                    ring,
                },
            ));
            let stations: Vec<Station> = if squadron.deployed {
                // The line lies across her bearing from the guide, and curves about her.
                // It unfolds from close order: the starboard wing, which lay clockwise of
                // the guide as she bore down on her, keeps that end of the line, so no
                // ship crosses another to take up her station.
                let u = (at - player).normalize_or(-forward);
                let wheel = forward.dot(u.perp());
                let span = off.max(range);
                let places = line_places(&hulls, LINE_SPACE);
                line.iter()
                    .zip(places)
                    .skip(1)
                    .map(|(&i, place)| {
                        let r = &self.raiders[i];
                        let turn = Vec2::from_angle(-place / span);
                        let own = line_range(r.class);
                        let out = (off + own - range).max(0.5 * own);
                        let station = player + turn.rotate(u) * out;
                        let hull = r.class.profile().hull;
                        Station {
                            at: self.navigable_position(station, hull.radius + 8.),
                            heading: turn.rotate(forward),
                            speed: speed * (1. - wheel.abs() + wheel.abs() * out / off.max(1.)),
                        }
                    })
                    .collect()
            } else {
                let abreast: Vec<Station> = close_order(&hulls)
                    .into_iter()
                    .map(|(ahead, starboard)| Station {
                        at: at + forward * ahead + forward.perp() * starboard,
                        heading: forward,
                        speed,
                    })
                    .collect();
                let fits = abreast.iter().zip(&hulls[1..]).all(|(station, hull)| {
                    self.navigable_position(station.at, hull.radius + 12.)
                        .distance_squared(station.at)
                        < 0.01
                        && self.open_water(at, station.at)
                });
                // Change only once the other order has looked right for a while.
                if fits == squadron.column {
                    squadron.doubt += dt;
                    if squadron.doubt >= DOUBT {
                        squadron.column = !fits;
                        squadron.doubt = 0.;
                    }
                } else {
                    squadron.doubt = 0.;
                }
                if squadron.column {
                    let mut run = 0.;
                    hulls
                        .windows(2)
                        .map(|pair| {
                            run += pair[0].stern + pair[1].bow + COLUMN_SPACE;
                            let (at, heading) = along_trail(&squadron.trail, run)
                                .unwrap_or((at - forward * run, forward));
                            Station { at, heading, speed }
                        })
                        .collect()
                } else {
                    abreast
                }
            };
            orders.extend(
                line[1..]
                    .iter()
                    .zip(stations)
                    .map(|(&i, station)| (i, Order::Keep(station))),
            );
        }
        squadrons.retain(|s| s.ships.len() >= 2);
        self.squadrons = squadrons;
        for (i, order) in orders {
            self.raiders[i].order = Some(order);
        }
    }
}

#[cfg(test)]
mod tests;
