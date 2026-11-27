//! Light harassing aircraft. Every base whose yard stands keeps a formation moored off
//! its quay, and once it is roused it sends them up against the ship in turn. The
//! bases at the mouth of a channel keep pairs, and those further up keep more, up to
//! five at the heavy yard. A formation comes in low abreast on short strafing passes,
//! climbs away, circles out and comes back. After
//! their passes, or once their base is wrecked, they fly home and alight at the mooring,
//! where they lie in reach of her stem and her guns (see [`mooring`]).
//! They are a nuisance more than a danger: two AA hits bring one down, and two ranks of
//! armor turn their guns.
use super::{
    Battery, Battle, Body, Landmark, Port, air_station::Transit, bearing, fire,
    flak::HARRIER_STRENGTH,
};
use glam::{Vec2, Vec3, Vec3Swizzles};
use shipvivor_compass::direction;
use std::f32::consts::{PI, TAU};

mod mooring;
pub use mooring::{MooredHarrier, REFIT, RUN_DOWN};
pub(crate) use mooring::{Moorings, TOP as MOORED_TOP};

/// Seconds into a leg before the first formation takes off.
pub const FIRST_SORTIE: f32 = 20.;
/// Seconds at least from one formation's take-off at a base to the next's.
pub const SORTIE_INTERVAL: f32 = 35.;
/// The most harriers aloft at once across the sector.
pub const MOST_ALOFT: usize = 8;
/// Harriers in the formation of a base in the first stage of a channel, and of one in
/// its last.
pub const FIRST_FORMATION: usize = 2;
pub const LAST_FORMATION: usize = 5;
const _: () = assert!(LAST_FORMATION <= MOST_ALOFT);
/// Strafing passes a harrier makes before it goes home.
pub const PASSES: u8 = 3;
/// Rounds in one pass, and the damage each does.
pub const ROUNDS: usize = 3;
pub const ROUND_DAMAGE: u16 = 1;
/// Metres per second over the water.
const SPEED: f32 = 40.;
/// Radians per second it turns at most, which brings it round in about 55 metres.
const TURN: f32 = 0.72;
/// How far it banks in its hardest turn.
const BANK: f32 = 0.6;
/// Metres above the water of a strafing pass, of the climb away from one, and of the
/// floats at the mooring.
const PASS_HEIGHT: f32 = 12.;
const CLIMB: f32 = 40.;
const AFLOAT: f32 = 1.2;
/// Metres a second it climbs or comes down at most.
const CLIMB_RATE: f32 = 9.;
/// Metres from the ship that a harrier circles out to between passes.
const STANDOFF: f32 = 300.;
/// Radians either way of its run that each wing of a formation turns away after a
/// pass, and the most of that, in those steps, that its outermost planes turn.
const SPLAY: f32 = 0.35;
const WIDEST_SPLAY: f32 = 1.5;
/// Metres from its mark at which a harrier lined up on the ship commits to a pass, and
/// the radians off that line within which it counts as lined up. One that is nearer
/// still without having lined up goes round again.
const COMMIT: f32 = 160.;
const LINED_UP: f32 = 0.2;
/// Metres short of the mark at which each round of a pass is fired. The burst walks
/// across the mark, a few metres a round.
const BURST: [f32; ROUNDS] = [70., 58., 46.];
const WALK: f32 = 5.;
/// Metres past the mark it holds its run before it pulls up and away.
const OVERSHOOT: f32 = 45.;
/// Metres apart a formation's planes lie abreast at the mooring and fly their passes,
/// and metres between the rows they lie in at the mooring.
const ABREAST: f32 = 10.;
const MOORING_ROW: f32 = 11.;

/// A light strafing floatplane from a base's mooring.
#[derive(Clone)]
pub struct Harrier {
    pub body: Body,
    pub altitude: f32,
    pub bank: f32,
    /// Metres per second over the water, as it last flew.
    pub way: Vec2,
    /// The base it flies from.
    pub base: usize,
    /// Its place at the mooring, from 0.
    pub slot: usize,
    /// Where it flies across the formation, in half-spacings to starboard of its
    /// middle: -1 and 1 for a pair, -2, 0 and 2 for three, and so on.
    pub side: f32,
    /// Hits it can still take before it goes down.
    pub(super) health: u16,
    passes: u8,
    leg: Leg,
}

#[derive(Clone)]
enum Leg {
    /// Coming in at the ship to line up a pass, from the mooring or from circling out.
    Inbound,
    /// On its run at the mark, with the rounds of the pass fired so far.
    Pass { mark: Vec2, fired: usize },
    /// Climbing away from a pass towards this point out from the ship.
    Away(Vec2),
    /// Bound for the mooring, coming down from this height.
    Home { path: Transit, from: f32 },
}

impl Harrier {
    pub fn position_3d(&self) -> Vec3 {
        self.body.pos.extend(self.altitude).xzy()
    }
    /// Coming at the ship, rather than circling out between passes or going home.
    pub fn attacking(&self) -> bool {
        matches!(self.leg, Leg::Inbound | Leg::Pass { .. })
    }
    pub fn homebound(&self) -> bool {
        matches!(self.leg, Leg::Home { .. })
    }
    /// Strafing passes flown so far.
    pub fn passes(&self) -> u8 {
        self.passes
    }

    /// Turn as hard as it may towards `goal`, fly on, and climb or come down towards
    /// `height`, never lower than `floor`.
    fn steer(&mut self, dt: f32, goal: Vec2, height: f32, floor: f32) {
        let off = signed(bearing(goal - self.body.pos) - self.body.heading);
        let turn = off.clamp(-TURN * dt, TURN * dt);
        self.body.heading = (self.body.heading + turn).rem_euclid(TAU);
        let wanted = if dt > 0. {
            BANK * turn / (TURN * dt)
        } else {
            self.bank
        };
        self.bank += (wanted - self.bank) * (1. - (-6. * dt).exp());
        self.body.pos += direction(self.body.heading) * SPEED * dt;
        self.climb(dt, height, floor);
    }

    fn climb(&mut self, dt: f32, height: f32, floor: f32) {
        let step = CLIMB_RATE * dt;
        self.altitude += (height.max(floor) - self.altitude).clamp(-step, step);
        self.altitude = self.altitude.max(floor);
    }

    /// Turn for home, to come down at the mooring heading in towards the base.
    fn recall(&mut self, port: &Port) {
        let path = Transit::new(
            self.body.pos,
            self.body.forward(),
            port.harrier_mooring(self.slot),
            -port.body.forward(),
            SPEED,
        );
        self.leg = Leg::Home {
            path,
            from: self.altitude,
        };
    }
}

impl Port {
    /// Where the harrier in `slot` of the base's formation lies at its mooring: in the
    /// water off the far side of the island from the main quay, two abreast in rows that
    /// run out from the base. A second drydock's slip and berth lie on that side, so
    /// there they moor farther out, beyond its launching water.
    pub fn harrier_mooring(&self, slot: usize) -> Vec2 {
        let f = self.body.forward();
        let wing = if slot.is_multiple_of(2) { -1. } else { 1. };
        let row = (slot / 2) as f32;
        let off = if self.docks().len() > 1 { 32. } else { 22. };
        self.body.pos + f * (34. + row * MOORING_ROW) + f.perp() * (wing * ABREAST * 0.5 - off)
    }
}

/// Where the plane in `slot` of a formation of `size` flies across it: alternately to
/// port and starboard of the middle, the first two innermost, so the formation is
/// centred on its leader's line.
pub fn formation_side(slot: usize, size: usize) -> f32 {
    let spread = |k: usize| {
        let wing = if k.is_multiple_of(2) { -1. } else { 1. };
        wing * (2 * (k / 2) + 1) as f32
    };
    let middle = (0..size).map(spread).sum::<f32>() / size.max(1) as f32;
    spread(slot) - middle
}

/// An angle brought into a half turn either way.
fn signed(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

/// The least height to fly at over `at`: clear of the islands, the shore and the bases,
/// rising ahead of each as the attack plane's does, and over each base's gantry. The
/// gantry is slender, so it is cleared close in, and the mooring beside it stays low.
fn floor(at: Vec2, landmarks: &[Landmark], ports: &[Port]) -> f32 {
    let land = landmarks
        .iter()
        .flat_map(Landmark::volumes)
        .chain(ports.iter().flat_map(Port::land_volumes))
        .map(|(center, radius, top)| top + 4. - 0.48 * (at.distance(center) - radius).max(0.));
    let gantries = ports.iter().flat_map(|port| {
        (0..port.docks().len()).map(move |i| {
            let beyond = (at.distance(port.dock_position(i)) - 8.).max(0.);
            port.dock_top(i) + 3. - 1.2 * beyond
        })
    });
    land.chain(gantries).fold(0., f32::max)
}

impl Battle {
    pub fn harriers(&self) -> &[Harrier] {
        &self.harriers
    }
    /// Whether base `index` has its formation lying at the mooring: its yard stands, none
    /// of them is aloft, and at least one was not wrecked where it lay.
    pub fn harriers_moored(&self, index: usize) -> bool {
        !self.ports[index].wrecked()
            && self.harriers.iter().all(|plane| plane.base != index)
            && self.moorings.planes.iter().any(|plane| plane.base == index)
    }

    /// Harriers in base `index`'s formation. A base in the first stage of the channel
    /// keeps [`FIRST_FORMATION`] and one in the last keeps [`LAST_FORMATION`], with those
    /// between in step, leaning small: in a channel of three stages, pairs, then threes,
    /// then five at the heavy yard; in one of four, pairs, threes, fours and fives.
    pub fn formation(&self, index: usize) -> usize {
        let last = self.ports.iter().map(Port::stage).max().unwrap_or(0);
        if last == 0 {
            return FIRST_FORMATION;
        }
        let steps = (LAST_FORMATION - FIRST_FORMATION) as f32;
        let step = f32::from(self.ports[index].stage()) / f32::from(last);
        FIRST_FORMATION + (steps * step + 0.25).floor() as usize
    }

    /// Each roused base whose yard stands and whose formation is at the mooring sends up
    /// all that lie there once its interval is out, nearest the ship first, while the sky
    /// has room for all of them. Each takes off from where it lies.
    fn launch_harriers(&mut self, player: Vec2) {
        if self.at_sea < FIRST_SORTIE {
            return;
        }
        let mut ready: Vec<usize> = (0..self.ports.len())
            .filter(|&i| {
                self.ports[i].roused
                    && self.at_sea >= self.harrier_due[i]
                    && self.harriers_moored(i)
            })
            .collect();
        ready.sort_by(|&a, &b| {
            let range = |i: usize| self.ports[i].body.pos.distance_squared(player);
            range(a).total_cmp(&range(b))
        });
        for base in ready {
            let lying = |plane: &MooredHarrier| plane.base == base;
            let size = self.moorings.planes.iter().filter(|p| lying(p)).count();
            if self.harriers.len() + size > MOST_ALOFT {
                continue;
            }
            let mut formation: Vec<MooredHarrier> =
                self.moorings.planes.extract_if(.., |p| lying(p)).collect();
            formation.sort_by_key(|plane| plane.slot);
            for (place, plane) in formation.into_iter().enumerate() {
                self.harriers.push(Harrier {
                    body: plane.body,
                    altitude: AFLOAT,
                    bank: 0.,
                    way: Vec2::ZERO,
                    base,
                    slot: plane.slot,
                    side: formation_side(place, size),
                    health: HARRIER_STRENGTH,
                    passes: 0,
                    leg: Leg::Inbound,
                });
            }
            self.harrier_due[base] = self.at_sea + SORTIE_INTERVAL;
        }
    }

    /// Launch, fly and land the harriers for one step, and fire their passes at the ship
    /// at `player`, making `way`.
    pub(super) fn fly_harriers(&mut self, dt: f32, player: Vec2, way: Vec2) {
        self.fill_moorings();
        self.launch_harriers(player);
        let mut landed = Vec::new();
        for (index, plane) in self.harriers.iter_mut().enumerate() {
            let port = &self.ports[plane.base];
            let before = plane.body.pos;
            if port.wrecked() && !plane.homebound() {
                plane.recall(port);
            }
            // It looks a little way ahead, to be climbing before the land is under it.
            let ahead = plane.body.pos + plane.body.forward() * 25.;
            let ground = floor(plane.body.pos, &self.landmarks, &self.ports).max(floor(
                ahead,
                &self.landmarks,
                &self.ports,
            ));
            match &mut plane.leg {
                Leg::Inbound => {
                    // Lead her by her way for the time it takes to close to mid-burst.
                    let range = plane.body.pos.distance(player);
                    let ahead = player + way * (((range - BURST[1]) / SPEED).max(0.) + 0.25);
                    let line = (ahead - plane.body.pos).normalize_or(plane.body.forward());
                    let mark = ahead + line.perp() * plane.side * ABREAST * 0.5;
                    let left = plane.body.pos.distance(mark);
                    let low = ((left - COMMIT) / 150.).clamp(0., 1.);
                    plane.steer(dt, mark, PASS_HEIGHT + (CLIMB - PASS_HEIGHT) * low, ground);
                    let off = signed(bearing(mark - plane.body.pos) - plane.body.heading).abs();
                    if left <= COMMIT && left > BURST[0] + 10. && off <= LINED_UP {
                        plane.body.heading = bearing(mark - plane.body.pos);
                        plane.leg = Leg::Pass { mark, fired: 0 };
                    } else if left <= BURST[0] + 10. {
                        // Too close to line up: go round and come in again.
                        let out = plane.body.forward();
                        plane.leg = Leg::Away(player + out * STANDOFF);
                    }
                }
                &mut Leg::Pass { mark, mut fired } => {
                    let axis = plane.body.forward();
                    plane.bank *= (-6. * dt).exp();
                    plane.body.pos += axis * SPEED * dt;
                    plane.climb(dt, PASS_HEIGHT, ground);
                    let left = (mark - plane.body.pos).dot(axis);
                    while fired < ROUNDS && left <= BURST[fired] {
                        let strike = mark + axis * ((fired as f32 - 1.) * WALK);
                        fire(
                            &mut self.shots,
                            &mut self.effects,
                            &mut self.serial,
                            plane.position_3d(),
                            strike.extend(1.).xzy(),
                            Battery::Aircraft,
                            ROUND_DAMAGE,
                        );
                        fired += 1;
                    }
                    plane.leg = Leg::Pass { mark, fired };
                    if left < -OVERSHOOT {
                        plane.passes += 1;
                        if plane.passes >= PASSES {
                            plane.recall(port);
                        } else {
                            // The formation splits as it pulls up, and comes round from
                            // either side.
                            let splay = plane.side.clamp(-WIDEST_SPLAY, WIDEST_SPLAY) * SPLAY;
                            let out = axis.rotate(Vec2::from_angle(splay));
                            plane.leg = Leg::Away(player + out * STANDOFF);
                        }
                    }
                }
                Leg::Away(turn) => {
                    let turn = *turn;
                    plane.steer(dt, turn, CLIMB, ground);
                    if plane.body.pos.distance(player) >= STANDOFF
                        || plane.body.pos.distance(turn) < 40.
                    {
                        plane.leg = Leg::Inbound;
                    }
                }
                Leg::Home { path, from } => {
                    let (position, tangent, t) = path.step(dt);
                    let heading = bearing(tangent);
                    let turn = signed(heading - plane.body.heading);
                    let wanted = if dt > 0. {
                        (BANK * turn / (TURN * dt)).clamp(-BANK, BANK)
                    } else {
                        plane.bank
                    };
                    plane.bank += (wanted - plane.bank) * (1. - (-6. * dt).exp());
                    plane.body.pos = position;
                    plane.body.heading = heading;
                    // Level off as it comes in, and settle on the water at the end.
                    let height = AFLOAT + (*from - AFLOAT) * (1. - t * t);
                    plane.altitude = height.max(ground * (1. - t));
                    if t >= 1. {
                        landed.push(index);
                    }
                }
            }
            if dt > 0. {
                plane.way = (plane.body.pos - before) / dt;
            }
        }
        // Each lies where it came down, heading in, until the next sortie.
        for index in landed.into_iter().rev() {
            let plane = self.harriers.remove(index);
            (self.moorings.planes).push(MooredHarrier::lying(plane.body, plane.base, plane.slot));
        }
    }

    /// No harriers take off this leg. For tests about other things.
    #[cfg(test)]
    pub(crate) fn ground_harriers(&mut self) {
        self.harrier_due.fill(f32::INFINITY);
    }
}

#[cfg(test)]
mod tests;
