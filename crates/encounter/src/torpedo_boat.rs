//! Torpedo boats. Each base keeps a pair moored off the flank of its island, in plain
//! sight. Once the base is roused and has word of the ship, it sends them out together,
//! never sooner than [`RUN_INTERVAL`] after its last pair left and only once both are
//! home and loaded, and never while more than [`MOST_OUT`] would be out across the
//! sector. Each carries [`TORPEDOES`] torpedoes and fires one on each run: she runs
//! in fast to a position off the ship's beam, a little ahead of her, and puts it in the
//! water across her course. Then she sheers off and circles the ship at
//! [`HARRYING_RANGE`], her little forecastle gun firing at her, for [`RERUN`] seconds.
//! After that she stands out to her firing station and makes her next run. With her
//! tubes empty she stays out and harries the ship with her gun. She turns for home only
//! when she loses the ship, gives up a run or is recalled, and reloads at the mooring
//! for [`RELOAD`] seconds. The torpedo runs straight and does not seek her. A wrecked
//! base calls its boats home and sends none, and nothing comes from the open sea: a
//! boat lost is replaced off her own base's slip.
use super::torpedo::{TORPEDO_RANGE, TORPEDO_SPEED, intercept};
use super::{Battle, EnemyClass, Raider, SPAWN_STANDOFF, Vec2, bearing};
use shipvivor_compass::direction;
use std::f32::consts::{PI, TAU};

/// Seconds a base has been roused before it first sends its boats out.
pub const FIRST_RUN: f32 = 40.;
/// Seconds at least from one pair leaving a base to the next.
pub const RUN_INTERVAL: f32 = 60.;
/// The most torpedo boats out across the sector at once.
pub const MOST_OUT: usize = 4;
/// Boats each base keeps at its mooring.
pub const MOORED_PAIR: usize = 2;
/// Seconds a boat lies at her mooring reloading after she has fired.
pub const RELOAD: f32 = 30.;
/// Torpedoes a boat carries out from her mooring. She fires one on each run.
pub const TORPEDOES: u8 = 2;
/// Seconds a boat harries the ship after one run's torpedo before she stands out for
/// the next.
pub const RERUN: f32 = 20.;
/// Metres from the ship at which a harrying boat circles her, inside her gun's reach.
pub const HARRYING_RANGE: f32 = 230.;
/// Metres along the line of fire, and across it, that each step of a harrying boat's
/// fall of shot may stray from its mark: four steps over and short, two either side.
pub const HARRYING_SCATTER: (f32, f32) = (4., 3.);
/// Radians round from where a harrying boat lies off the ship to the point she steers
/// for, so that she circles her.
const HARRYING_LEAD: f32 = 0.5;
/// Metres short of her firing station's range from the ship at which a boat standing
/// out for her next run turns in to make it.
const STAND_OUT: f32 = 40.;
/// Seconds after a boat is lost before her base sends another off its slip.
pub const REPLACEMENT: f32 = 45.;
/// Metres from her, nearest and farthest, at which a boat in open water fires. Where
/// the water off her beam is narrower, the boat comes in as far as it lets her.
pub const FIRING_RANGE: (f32, f32) = (300., 380.);
/// Damage an enemy torpedo does where it strikes her, below the waterline.
pub const WARHEAD: u16 = 16;
/// Metres along her course, ahead or astern, that a boat's aim may be off. A ship that
/// holds her course is struck by most torpedoes but not by all.
pub const AIM_ERROR: f32 = 24.;
/// The nearest a boat closes before she fires, however narrow the water.
const CLOSEST: f32 = 200.;
/// Radians either way of the boat's bow her torpedo can be sent: its gyro turns it onto
/// its course as it leaves the tube.
const GYRO: f32 = 0.6;
/// Radians off the ship's beam, ahead or abaft, within which a boat counts as on it.
const ON_BEAM: f32 = 0.7;
/// Seconds a boat keeps up a run before she gives it up and goes home.
const RUN_LIMIT: f32 = 90.;
/// Metres from her mooring at which she lets go her anchor, and the most she lets it go
/// at once she has run past it.
const ALONGSIDE: (f32, f32) = (12., 25.);
/// Metres out from her mooring at which she begins to ease her way, and the least share
/// of her speed she keeps coming in. Her rudder needs way on to bite, so she keeps
/// enough to steer for her berth and lets the anchor take the rest off.
const EASING: (f32, f32) = (30., 0.45);
/// Where a base's boats lie, in metres to starboard and ahead of its island's centre,
/// best first: off the flank away from the harriers, or else astern of them.
const MOORINGS: [(f32, f32); 6] = [
    (40., 16.),
    (56., 16.),
    (40., -6.),
    (56., -6.),
    (-44., -6.),
    (-60., -6.),
];

/// A torpedo boat's part in her base's runs.
#[derive(Clone)]
pub(super) struct Attack {
    /// Her base, and where she lies at its mooring.
    pub base: usize,
    pub mooring: Vec2,
    /// Torpedoes still aboard her.
    pub torpedoes: u8,
    /// Seconds still to reload at the mooring.
    pub reload: f32,
    /// Bound home: she has lost the ship, given up her run or been recalled.
    pub homeward: bool,
    /// Seconds into her present run, counted from her last torpedo once she has fired.
    pub run: f32,
    /// Seconds still to harry the ship before she makes her next run.
    pub rerun: f32,
    /// Done harrying, she stands out to her firing station before turning in.
    pub standing_out: bool,
    /// Metres ahead of the ship, along her course, that the boat's torpedo is aimed on
    /// this run: her error in judging the ship's course and speed.
    pub miss: f32,
    /// Where she is making for, and the share of her full speed she makes for it.
    pub goal: Vec2,
    pub throttle: f32,
}
impl Attack {
    fn moored(base: usize, mooring: Vec2) -> Self {
        Self {
            base,
            mooring,
            torpedoes: TORPEDOES,
            reload: 0.,
            homeward: false,
            run: 0.,
            rerun: 0.,
            standing_out: false,
            miss: 0.,
            goal: mooring,
            throttle: 1.,
        }
    }
    /// Her tubes are full.
    fn full(&self) -> bool {
        self.torpedoes == TORPEDOES
    }
}

/// A base's torpedo boats.
#[derive(Clone)]
pub(super) struct Flotilla {
    /// Where they lie at its mooring.
    pub moorings: Vec<Vec2>,
    /// Seconds into the sector when the base may next send them out.
    pub due: f32,
    /// Boats lost and not yet replaced, and when the next of them leaves the slip.
    pub lost: u8,
    pub spare_due: Option<f32>,
}

/// A share from 0 to 1 that `seed` always gives, by a splitmix step.
fn unit(seed: u64) -> f32 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((z ^ (z >> 31)) >> 40) as f32 / (1_u64 << 24) as f32
}

/// An angle brought into a half turn either way.
fn signed(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

impl Raider {
    /// A torpedo boat out on a run at the ship with a torpedo aboard for it.
    pub fn running_in(&self) -> bool {
        self.moored.is_none()
            && self
                .attack
                .as_ref()
                .is_some_and(|run| run.torpedoes > 0 && !run.homeward && run.rerun == 0.)
    }
    /// A torpedo boat out circling the ship with her gun, between runs or with her
    /// tubes empty.
    pub fn harrying(&self) -> bool {
        self.moored.is_none()
            && self
                .attack
                .as_ref()
                .is_some_and(|run| !run.homeward && (run.torpedoes == 0 || run.rerun > 0.))
    }
    /// A torpedo boat with a torpedo aboard, at her mooring or at sea.
    pub fn torpedo_aboard(&self) -> bool {
        self.attack.as_ref().is_some_and(|run| run.torpedoes > 0)
    }
    /// A torpedo boat's base, and where she lies at its mooring.
    pub fn torpedo_mooring(&self) -> Option<(usize, Vec2)> {
        self.attack.as_ref().map(|run| (run.base, run.mooring))
    }
}

impl Battle {
    /// Moor the waters' torpedo boats off each base ([`MOORED_PAIR`] in mixed waters; see
    /// `Doctrine::torpedo_boats`), on open water clear of the harriers' moorings, the ice
    /// and every other ship. They lie there until it sends them out.
    pub(crate) fn with_torpedo_boats(mut self) -> Self {
        let hull = EnemyClass::TorpedoBoat.profile().hull;
        let boats = self.doctrine.torpedo_boats();
        for base in 0..self.ports.len() {
            let port = &self.ports[base];
            let (centre, forward) = (port.body.pos, port.body.forward());
            let harriers: Vec<Vec2> = (0..crate::LAST_FORMATION)
                .map(|slot| port.harrier_mooring(slot))
                .collect();
            let mut moorings = Vec::new();
            for (across, ahead) in MOORINGS {
                if moorings.len() == boats {
                    break;
                }
                let spot = centre + forward.perp() * across + forward * ahead;
                // No ship lies within the standoff of where the player enters.
                let clear = spot.length() >= SPAWN_STANDOFF
                    && self
                        .navigable_position(spot, hull.radius + 4.)
                        .distance(spot)
                        < 0.01
                    && harriers.iter().all(|h| h.distance(spot) > hull.bow + 6.)
                    && self
                        .floe
                        .icebergs
                        .iter()
                        .all(|b| b.body.pos.distance(spot) > b.radius + hull.bow + 2.)
                    && self.raiders.iter().all(|r| {
                        r.body.pos.distance(spot) > r.class.profile().hull.bow + hull.bow + 2.
                    });
                if clear {
                    self.spawn(EnemyClass::TorpedoBoat, spot, spot + forward);
                    let boat = self.raiders.last_mut().unwrap();
                    boat.moor(base);
                    boat.attack = Some(Attack::moored(base, spot));
                    moorings.push(spot);
                }
            }
            self.flotillas.push(Flotilla {
                moorings,
                due: 0.,
                lost: 0,
                spare_due: None,
            });
        }
        self
    }

    /// Torpedo boats out from their moorings, and under command.
    pub fn torpedo_boats_out(&self) -> usize {
        self.raiders
            .iter()
            .filter(|r| r.attack.is_some() && r.moored.is_none() && !r.is_disabled())
            .count()
    }

    /// Reload the boats at their moorings, send out each base's whose turn has come,
    /// replace those lost, and con those at sea for a step against the ship at `player`
    /// on `heading`, making `way`.
    pub(super) fn work_torpedo_boats(&mut self, dt: f32, player: Vec2, heading: f32, way: Vec2) {
        if self.flotillas.is_empty() {
            return;
        }
        for r in &mut self.raiders {
            let anchored = r.moored.is_some();
            let Some(run) = &mut r.attack else {
                continue;
            };
            if anchored {
                run.reload = (run.reload - dt).max(0.);
                if run.reload == 0. {
                    run.torpedoes = TORPEDOES;
                }
                run.goal = r.body.pos;
            }
        }
        self.send_torpedo_boats(player);
        self.replace_torpedo_boats(player);
        let course = way.try_normalize().filter(|_| way.length() > 0.5);
        let course = course.unwrap_or_else(|| direction(heading));
        for i in 0..self.raiders.len() {
            let r = &self.raiders[i];
            if r.attack.is_some() && r.moored.is_none() && !r.is_disabled() {
                self.con_torpedo_boat(i, dt, player, course, way);
            }
        }
    }

    /// Each roused base still standing that has word of the ship sends out its boats
    /// once its interval is out and every one of them under command is home and loaded,
    /// nearest the ship first, while the sector has room for them all.
    fn send_torpedo_boats(&mut self, player: Vec2) {
        let mut out = self.torpedo_boats_out();
        let mut bases: Vec<usize> = (0..self.ports.len()).collect();
        bases.sort_by(|&a, &b| {
            let range = |i: usize| self.ports[i].body.pos.distance_squared(player);
            range(a).total_cmp(&range(b))
        });
        for base in bases {
            let port = &self.ports[base];
            let Some(roused_at) = port.roused_at() else {
                continue;
            };
            let due = self.flotillas[base].due.max(roused_at + FIRST_RUN);
            if port.wrecked() || !port.has_contact || self.at_sea < due {
                continue;
            }
            // A boat adrift at her mooring stays there.
            let crew: Vec<usize> = (0..self.raiders.len())
                .filter(|&i| {
                    let r = &self.raiders[i];
                    !r.is_disabled() && r.attack.as_ref().is_some_and(|run| run.base == base)
                })
                .collect();
            let ready = crew.iter().all(|&i| {
                let r = &self.raiders[i];
                r.moored.is_some() && r.attack.as_ref().is_some_and(Attack::full)
            });
            if crew.is_empty() || !ready || out + crew.len() > MOST_OUT {
                continue;
            }
            for &i in &crew {
                let boat = &mut self.raiders[i];
                boat.cast_off();
                let judged = unit(boat.body.id ^ self.at_sea.to_bits().rotate_left(17) as u64);
                let run = boat.attack.as_mut().unwrap();
                run.homeward = false;
                run.run = 0.;
                run.rerun = 0.;
                run.standing_out = false;
                run.miss = AIM_ERROR * (2. * judged - 1.);
                run.throttle = 1.;
            }
            out += crew.len();
            self.flotillas[base].due = self.at_sea + RUN_INTERVAL;
        }
    }

    /// A roused base still standing sends a new boat off its slip [`REPLACEMENT`]
    /// seconds after it loses one, to lie at the mooring she left empty.
    fn replace_torpedo_boats(&mut self, player: Vec2) {
        for base in 0..self.flotillas.len() {
            let port = &self.ports[base];
            if self.flotillas[base].lost == 0
                || !port.roused()
                || port.wrecked()
            {
                continue;
            }
            let at_sea = self.at_sea;
            let due = *self.flotillas[base]
                .spare_due
                .get_or_insert(at_sea + REPLACEMENT);
            if at_sea < due {
                continue;
            }
            let taken: Vec<Vec2> = (self.raiders.iter())
                .filter_map(Raider::torpedo_mooring)
                .filter(|&(home, _)| home == base)
                .map(|(_, mooring)| mooring)
                .collect();
            let free = (self.flotillas[base].moorings.iter())
                .copied()
                .find(|m| !taken.contains(m));
            let Some(mooring) = free else {
                // Every berth has a boat in it already.
                self.flotillas[base].lost = 0;
                self.flotillas[base].spare_due = None;
                continue;
            };
            let Some(slip) = self.slip(base, EnemyClass::TorpedoBoat, player) else {
                continue;
            };
            let out = self.ports[base].body.forward();
            self.spawn(EnemyClass::TorpedoBoat, slip, slip + out);
            self.raiders.last_mut().unwrap().attack = Some(Attack {
                homeward: true,
                ..Attack::moored(base, mooring)
            });
            let flotilla = &mut self.flotillas[base];
            flotilla.lost -= 1;
            flotilla.spare_due = None;
        }
    }

    /// Where a boat at `at` would fire from off the ship's beam on `side`, to starboard
    /// (1) or port (-1) of her `course`: as far out in the firing range as the water
    /// there allows, a little ahead of her so the torpedo meets her as she comes on.
    fn firing_station(&self, player: Vec2, course: Vec2, way: Vec2, side: f32) -> Option<Vec2> {
        let mid = 0.5 * (FIRING_RANGE.0 + FIRING_RANGE.1);
        [mid, FIRING_RANGE.0, 260., 230., CLOSEST + 10.]
            .into_iter()
            .map(|range| player + way * (range / TORPEDO_SPEED) + course.perp() * side * range)
            .find(|&station| {
                self.navigable_position(station, 12.).distance(station) < 0.01
                    && self.open_water(player, station)
            })
    }

    /// Where a boat at `at` steers to harry the ship at `player`: a point on the circle
    /// [`HARRYING_RANGE`] from her, a little further round than the boat lies now, so
    /// that she circles her anticlockwise (`sense` 1) or clockwise (-1) with her gun
    /// bearing.
    fn harrying_station(&self, player: Vec2, at: Vec2, sense: f32) -> Vec2 {
        let from = (at - player).try_normalize().unwrap_or(Vec2::X);
        let round = Vec2::from_angle(sense * HARRYING_LEAD).rotate(from);
        self.navigable_position(player + round * HARRYING_RANGE, 12.)
    }

    /// One step of a torpedo boat at sea: in to her firing position and the shot,
    /// circling the ship with her gun between runs and once her tubes are empty, or
    /// home to her mooring, where she lets go her anchor and reloads.
    fn con_torpedo_boat(&mut self, i: usize, dt: f32, player: Vec2, course: Vec2, way: Vec2) {
        let r = &self.raiders[i];
        let run = r.attack.as_ref().unwrap();
        let (at, base, mooring) = (r.body.pos, run.base, run.mooring);
        let recalled = self.ports[base].wrecked();
        let fix = if r.has_contact {
            Some(player)
        } else {
            r.last_sighting
        };
        // Without word of the ship, she searches as far as her last fix and no further.
        let lost = fix.is_none_or(|fix| !r.has_contact && fix.distance(at) < 60.);
        // A run kept up too long is given up. With her tubes empty she has no run to
        // give up, and stays out to harry the ship for as long as she has her.
        let gave_up = run.torpedoes > 0 && run.run > RUN_LIMIT;
        let homeward = run.homeward || recalled || gave_up || lost;
        let harrying = run.torpedoes == 0 || run.rerun > 0.;
        let mut standing_out = run.standing_out;
        let mut fired = None;
        let goal = if homeward {
            mooring
        } else if r.has_contact && harrying {
            // Her two boats circle the ship opposite ways.
            let sense = if r.body.id % 2 == 0 { 1. } else { -1. };
            self.harrying_station(player, at, sense)
        } else if r.has_contact {
            let offset = at - player;
            let range = offset.length();
            let side = if offset.dot(course.perp()) >= 0. {
                1.
            } else {
                -1.
            };
            let station = self
                .firing_station(player, course, way, side)
                .or_else(|| self.firing_station(player, course, way, -side));
            // From close aboard, where she harried the ship, she first stands out to her
            // station, so that her next torpedo comes from as far off as her first.
            standing_out &=
                station.is_some_and(|station| range < station.distance(player) - STAND_OUT);
            let abeam = (offset.dot(course) / range.max(1.)).abs() <= ON_BEAM.sin();
            let tube = at + r.body.forward() * (r.class.profile().hull.bow - 1.);
            let aim = intercept(tube, player + course * run.miss, way, TORPEDO_SPEED);
            if let Some(aim) = aim
                && !standing_out
                && abeam
                && (CLOSEST..=FIRING_RANGE.1).contains(&range)
                && signed(bearing(aim - tube) - r.body.heading).abs() <= GYRO
                && aim.distance(tube) <= TORPEDO_RANGE * 0.9
                && self.open_water(tube, aim)
            {
                fired = Some((tube, bearing(aim - tube)));
            }
            // Close in and on her beam, she lines up on where her torpedo will meet her.
            match (aim, station) {
                (_, Some(station)) if standing_out => station,
                (Some(aim), _) if abeam && range <= FIRING_RANGE.1 + 80. => aim,
                (_, Some(station)) => station,
                _ => player,
            }
        } else {
            fix.unwrap()
        };
        let id = self.raiders[i].body.id;
        let judged = unit(id ^ self.at_sea.to_bits().rotate_left(17) as u64);
        if let Some((tube, line)) = fired {
            self.fire_torpedo(tube, line, id);
        }
        let boat = &mut self.raiders[i];
        let forward = boat.body.forward();
        let run = boat.attack.as_mut().unwrap();
        run.run += dt;
        // Her time harrying the ship is up: she stands out for her next run.
        let harried = run.rerun > 0.;
        run.rerun = (run.rerun - dt).max(0.);
        run.standing_out = standing_out || (harried && run.rerun == 0.);
        if fired.is_some() {
            // She sheers off to harry the ship, and judges her afresh for the next run.
            run.torpedoes -= 1;
            run.run = 0.;
            run.miss = AIM_ERROR * (2. * judged - 1.);
            if run.torpedoes > 0 {
                run.rerun = RERUN;
            }
        }
        run.homeward = homeward;
        run.goal = goal;
        run.throttle = 1.;
        if !run.homeward {
            return;
        }
        run.goal = mooring;
        let off = mooring - at;
        let left = off.length();
        if left < ALONGSIDE.0 || (left < ALONGSIDE.1 && off.dot(forward) < 0.) {
            // Home: she lets go her anchor and reloads if she fired.
            run.reload = if run.full() { 0. } else { RELOAD };
            run.homeward = false;
            run.rerun = 0.;
            run.standing_out = false;
            run.goal = at;
            boat.anchor(base);
        } else {
            run.throttle = ((left - ALONGSIDE.0) / EASING.0).clamp(EASING.1, 1.);
        }
    }

    /// No torpedo boat leaves her mooring this leg. For tests about other things.
    #[cfg(test)]
    pub(crate) fn hold_torpedo_boats(&mut self) {
        for flotilla in &mut self.flotillas {
            flotilla.due = f32::INFINITY;
        }
    }
}

#[cfg(test)]
mod tests;
