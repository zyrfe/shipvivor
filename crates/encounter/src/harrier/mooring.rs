//! A base's harriers lying at their mooring between sorties. Each is a light thing on
//! floats. A hull that comes against one at a crawl shoves it aside and sets it
//! swinging. One that comes on any faster runs it down, and so does a round from her
//! guns that strikes it or bursts close by. A base whose yard stands fills an empty
//! berth [`REFIT`] seconds after it lost the harrier there, once its formation is home.
use super::super::{Airframe, Battle, Body, Effect, EffectKind, Events, Orders, footprint, ram};
use glam::{Vec2, Vec3, Vec3Swizzles};
use shipvivor_compass::direction;

/// Seconds after a harrier is wrecked at its mooring before its base has another lying
/// in the berth.
pub const REFIT: f32 = 45.;
/// Metres per second, about two knots, at which a hull closing on a moored harrier runs
/// it down rather than shoving it aside.
pub const RUN_DOWN: f32 = 1.;
/// Metres from its middle to a wingtip, and from the water to the top of its wing.
pub const REACH: f32 = 1.7;
pub const TOP: f32 = 1.4;
/// Metres per inch of a round's calibre within which its burst wrecks a moored harrier.
const SPLINTERS: f32 = 0.6;
/// What a hull meets: the span across the floats, a little more than its length, and
/// next to nothing for weight.
const FLOATS: ram::Hull = ram::Hull {
    radius: 1.6,
    bow: 1.8,
    stern: 1.8,
    mass: 0.02,
    shock_per_point: 1.,
};
/// Rates per second at which the water takes the way and the swing off a shoved harrier.
const DRAG: f32 = 0.9;
const SWING_DRAG: f32 = 1.6;
/// Radians per second it swings at most.
const MOST_SWING: f32 = 2.5;

/// A harrier lying at its base's mooring.
pub struct MooredHarrier {
    pub body: Body,
    /// The base it flies from, and its berth there, from 0.
    pub base: usize,
    pub slot: usize,
    /// Metres per second it drifts after a shove, and radians per second it swings.
    way: Vec2,
    swing: f32,
}

impl MooredHarrier {
    pub(super) fn lying(body: Body, base: usize, slot: usize) -> Self {
        Self {
            body,
            base,
            slot,
            way: Vec2::ZERO,
            swing: 0.,
        }
    }

    fn floats(&self) -> ram::Ship {
        ram::Ship {
            hull: FLOATS,
            pos: self.body.pos,
            forward: self.body.forward(),
            velocity: self.way,
        }
    }

    /// Take a shove from a hull: part from it, and swing about if it came off centre.
    fn shove(&mut self, contact: &ram::Contact) {
        self.body.pos += contact.give[1];
        let kick = contact.kick[1];
        self.way += kick;
        let lever = contact.point - self.body.pos;
        let turn = lever.perp_dot(kick) / (0.5 * FLOATS.radius * FLOATS.radius);
        self.swing = (self.swing + turn).clamp(-MOST_SWING, MOST_SWING);
    }
}

/// Every base's mooring: the harriers lying there, and the berths left empty.
#[derive(Default)]
pub(crate) struct Moorings {
    pub(crate) planes: Vec<MooredHarrier>,
    /// A base, a berth there wrecked at the mooring, and the seconds into the sector
    /// when another may lie in it.
    refits: Vec<(usize, usize, f32)>,
}

impl Moorings {
    /// Wreck the harrier at `index`. It lies broken on the water and goes down, and its
    /// berth stands empty for [`REFIT`] seconds.
    pub(crate) fn wreck(&mut self, index: usize, now: f32, effects: &mut Vec<Effect>) {
        let plane = self.planes.swap_remove(index);
        self.refits.push((plane.base, plane.slot, now + REFIT));
        effects.push(Effect {
            body: plane.body,
            height: 0.,
            kind: EffectKind::Downed(Airframe::Harrier),
            age: 0.,
        });
    }

    /// A round of `calibre` inches bursts at `at`. It wrecks every harrier lying within
    /// reach of its splinters, and tells how many.
    pub(crate) fn blast(
        &mut self,
        at: Vec3,
        calibre: f32,
        now: f32,
        effects: &mut Vec<Effect>,
    ) -> u16 {
        if at.y > TOP + 1. {
            return 0;
        }
        let reach = SPLINTERS * calibre + REACH;
        let mut wrecked = 0;
        while let Some(index) = (self.planes.iter())
            .position(|plane| plane.body.pos.distance_squared(at.xz()) <= reach * reach)
        {
            self.wreck(index, now, effects);
            wrecked += 1;
        }
        wrecked
    }

    /// Where a round crossing from `a` to `b` first meets each harrier lying at a
    /// mooring, as entry and exit shares of the crossing, with the harrier's index.
    pub(crate) fn crossings(
        &self,
        a: Vec2,
        b: Vec2,
    ) -> impl Iterator<Item = (usize, (f32, f32))> + '_ {
        (self.planes.iter().enumerate())
            .filter_map(move |(i, plane)| Some((i, footprint(a, b, plane.body.pos, REACH)?)))
    }
}

impl Battle {
    /// The harriers lying at the bases' moorings.
    pub fn moored_harriers(&self) -> &[MooredHarrier] {
        &self.moorings.planes
    }

    /// Each base whose yard stands and whose formation is all home lays a harrier in
    /// every empty berth, but for those wrecked there less than [`REFIT`] seconds ago.
    pub(crate) fn fill_moorings(&mut self) {
        let now = self.at_sea;
        self.moorings.refits.retain(|&(.., due)| due > now);
        for (base, port) in self.ports.iter().enumerate() {
            if port.wrecked() || self.harriers.iter().any(|plane| plane.base == base) {
                continue;
            }
            for slot in 0..self.formation(base) {
                let taken = |&(b, s, _): &(usize, usize, f32)| b == base && s == slot;
                if (self.moorings.planes.iter()).any(|p| p.base == base && p.slot == slot)
                    || self.moorings.refits.iter().any(taken)
                {
                    continue;
                }
                let at = port.harrier_mooring(slot);
                let body = super::super::body(&mut self.serial, at, port.body.heading);
                (self.moorings.planes).push(MooredHarrier::lying(body, base, slot));
            }
        }
    }

    /// Let the moored harriers drift and swing, part them from each other, and settle
    /// each hull that comes against one: shoved aside at a crawl, run down at any more.
    pub(crate) fn jostle_moorings(&mut self, dt: f32, orders: &Orders, events: &mut Events) {
        if self.moorings.planes.is_empty() {
            return;
        }
        let way = (-DRAG * dt).exp();
        let swing = (-SWING_DRAG * dt).exp();
        for plane in &mut self.moorings.planes {
            plane.body.pos += plane.way * dt;
            plane.body.heading += plane.swing * dt;
            plane.way *= way;
            plane.swing *= swing;
        }
        let forward = direction(orders.heading);
        let pos = orders.player + events.shove;
        let player = ram::Ship {
            hull: orders.hull,
            pos,
            forward,
            velocity: forward * (orders.speed + events.jolt) + self.floe.currents.at(pos),
        };
        // Her own, then each enemy hull; only her own counts as brought down.
        let hulls = std::iter::once((true, player))
            .chain(self.raiders.iter().map(|r| (false, ram::afloat(r))));
        let mut wrecked = Vec::new();
        for (hers, ship) in hulls {
            for (i, plane) in self.moorings.planes.iter_mut().enumerate() {
                if wrecked.iter().any(|&(j, _, _)| j == i) {
                    continue;
                }
                let Some(contact) = ram::contact(&ship, &plane.floats()) else {
                    continue;
                };
                if contact.closing >= RUN_DOWN {
                    wrecked.push((i, hers, contact.point));
                } else {
                    plane.shove(&contact);
                }
            }
        }
        wrecked.sort_by_key(|&(i, ..)| std::cmp::Reverse(i));
        for (i, hers, point) in wrecked {
            let heading = self.moorings.planes[i].body.heading;
            self.moorings.wreck(i, self.at_sea, &mut self.effects);
            if hers {
                events.planes_downed += 1;
            }
            self.effects.push(Effect {
                body: super::super::body(&mut self.serial, point, heading),
                kind: EffectKind::Crunch,
                height: 0.,
                age: 0.,
            });
        }
        // Those lying close part from each other without harm.
        let planes = &mut self.moorings.planes;
        for i in 1..planes.len() {
            let (before, rest) = planes.split_at_mut(i);
            let b = &mut rest[0];
            for a in before {
                if let Some(contact) = ram::contact(&a.floats(), &b.floats()) {
                    a.body.pos += contact.give[0];
                    b.body.pos += contact.give[1];
                }
            }
        }
    }
}
