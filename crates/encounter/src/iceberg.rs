//! Ice adrift on the current: growlers and bergs. They ride the stream round the islands,
//! ground on the shores and break up there, stop rounds and torpedoes, and stove in any
//! hull that meets them too fast. Fire breaks them down a size at a time: a hit splits a
//! berg into pieces that fly on along the line of the shot, and a hit on a growler leaves
//! nothing but brash.
use super::{Battle, Blow, Body, Chart, Effect, EffectKind, Events, Orders, Vec2, bearing, body};
use super::{Currents, SPAWN_STANDOFF, chart::Ice, chart::Rng, ram};
use shipvivor_compass::direction;
use std::f32::consts::{PI, TAU};

/// Seconds the stream may pin ice against a shore before it breaks up there.
const GROUNDED_SECS: f32 = 45.;
/// Ice smaller than this melts away when it breaks rather than splitting again.
const MELT_RADIUS: f32 = 4.5;
/// Seconds between pieces drifting in from upstream while the chart is short of ice.
const REFILL_SECS: f32 = 25.;
/// Open water beyond the chart's features where ice still drifts. Past it, ice is gone.
const ICE_MARGIN: f32 = 260.;
/// Ice at least this big is worth steering round. Raiders shoulder smaller pieces aside.
pub const AVOIDED_RADIUS: f32 = 5.;
/// Damage ice takes per metre of its radius before it splits. A stock main-battery round
/// splits anything up to 10 m in radius and a light round a growler. The biggest bergs
/// take a few hits.
const TOUGHNESS: f32 = 0.2;
/// Share of a hull's closing speed that ice takes as damage to its integrity. A cruiser
/// at full speed breaks a berg up to about 8 m in radius in one ram.
const CRUSHING: f32 = 0.12;
/// Speed of the pieces a blow breaks off, in metres per second for a piece 1 m in radius.
/// Smaller pieces fly faster, as the square root of their size.
const SCATTER: f32 = 12.;
/// Half the angle, either side of the line of the blow, that the pieces fan out across.
const FAN: f32 = 0.8;
/// Speed at which the pieces of ice crushed on a shore come apart.
const PARTING: f32 = 0.4;
/// Fraction of the closing speed returned when two pieces of ice meet.
const ICE_REBOUND: f32 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IceShape {
    /// A small, low lump of ice, hard to see and easy to hit.
    Growler,
    Dome,
    Pinnacle,
    /// A flat-topped slab with sheer sides.
    Tabular,
}
impl IceShape {
    /// Height of the ice above the water, as a share of its waterline radius.
    pub fn freeboard(self) -> f32 {
        match self {
            Self::Growler => 0.3,
            Self::Dome => 0.55,
            Self::Pinnacle => 1.05,
            Self::Tabular => 0.36,
        }
    }
    /// Radius of the solid core above water, as a share of the waterline radius.
    fn waist(self) -> f32 {
        match self {
            Self::Growler => 0.85,
            Self::Dome => 0.75,
            Self::Pinnacle => 0.55,
            Self::Tabular => 0.95,
        }
    }
    /// Displacement per cubic metre of radius, in the units hulls use. Most of a berg
    /// is under water, so even a modest one outweighs a cruiser many times over.
    fn heft(self) -> f32 {
        match self {
            Self::Growler | Self::Dome => 0.05,
            Self::Pinnacle => 0.04,
            Self::Tabular => 0.065,
        }
    }
    fn for_radius(radius: f32, draw: u64) -> Self {
        if radius < MELT_RADIUS + 1. {
            Self::Growler
        } else {
            [Self::Dome, Self::Pinnacle, Self::Tabular][(draw % 3) as usize]
        }
    }
}

pub struct Iceberg {
    pub body: Body,
    /// Radius at the waterline in metres.
    pub radius: f32,
    pub shape: IceShape,
    /// Velocity over the ground, in metres per second.
    pub way: Vec2,
    /// Rate of turn in radians per second, positive clockwise.
    pub spin: f32,
    /// Damage the ice can take before it splits.
    pub integrity: f32,
    /// The blows it has taken, each along its line and weighted by its damage. The pieces
    /// fly on along their sum when the ice splits.
    blow: Vec2,
    /// Seconds the stream has held it against a shore.
    aground: f32,
    /// Seconds before a hull grinding against it can be hurt again.
    rest: f32,
}
impl Iceberg {
    fn new(body: Body, radius: f32, shape: IceShape, way: Vec2) -> Self {
        Self {
            body,
            radius,
            shape,
            way,
            spin: 0.,
            integrity: TOUGHNESS * radius,
            blow: Vec2::ZERO,
            aground: 0.,
            rest: 0.,
        }
    }
    /// A blow of `damage` travelling along `along`. If it splits the ice, the pieces fly
    /// on that way.
    pub(super) fn strike(&mut self, damage: f32, along: Vec2) {
        self.integrity -= damage;
        self.blow += along.normalize_or_zero() * damage;
    }
    /// Height of the highest ice above the water.
    pub fn top(&self) -> f32 {
        self.radius * self.shape.freeboard()
    }
    pub fn mass(&self) -> f32 {
        self.shape.heft() * self.radius.powi(3)
    }
    /// The ice above water as a solid for rounds and sightlines: centre, radius, height.
    pub fn volume(&self) -> (Vec2, f32, f32) {
        (self.body.pos, self.radius * self.shape.waist(), self.top())
    }
    /// Seconds the stream has pinned it against a shore.
    pub fn aground(&self) -> f32 {
        self.aground
    }
    fn hull(&self) -> ram::Ship {
        ram::Ship {
            hull: super::Hull {
                radius: self.radius,
                bow: self.radius + 0.05,
                stern: self.radius + 0.05,
                mass: self.mass(),
                shock_per_point: 1.,
            },
            pos: self.body.pos,
            forward: self.body.forward(),
            velocity: self.way,
        }
    }
}

/// The current and the ice on it, kept by the battle.
pub(super) struct Floe {
    pub currents: Currents,
    pub icebergs: Vec<Iceberg>,
    /// How much ice the chart began with. The current keeps bringing more while short.
    stock: usize,
    /// Share of the chart's ice that was bergs rather than growlers.
    bergs: f32,
    clock: f32,
    rng: Rng,
    /// Ice that drifts out of this box is gone.
    reach: (Vec2, Vec2),
}
impl Floe {
    pub(super) fn slack() -> Self {
        Self {
            currents: Currents::slack(),
            icebergs: vec![],
            stock: 0,
            bergs: 0.,
            clock: REFILL_SECS,
            rng: Rng(0),
            reach: (Vec2::ZERO, Vec2::ZERO),
        }
    }
}

impl Battle {
    /// Solve the chart's current and set its ice adrift on it.
    pub fn with_sea(mut self, chart: &Chart) -> Self {
        let (lo, hi) = chart.extent();
        let currents = Currents::solve(chart.set, &chart.land(), &chart.gyres, lo, hi);
        let icebergs = chart
            .ice
            .iter()
            .map(|ice: &Ice| {
                let at = body(&mut self.serial, ice.pos, ice.heading);
                Iceberg::new(at, ice.radius, ice.shape, currents.at(ice.pos))
            })
            .collect::<Vec<_>>();
        let bergs = icebergs
            .iter()
            .filter(|b| b.shape != IceShape::Growler)
            .count() as f32
            / icebergs.len().max(1) as f32;
        self.floe = Floe {
            currents,
            stock: icebergs.len(),
            icebergs,
            bergs,
            clock: REFILL_SECS,
            rng: Rng(chart.seed.unwrap_or(0) ^ 0x1CE_B3A6),
            reach: (lo - ICE_MARGIN, hi + ICE_MARGIN),
        };
        self
    }

    pub fn currents(&self) -> &Currents {
        &self.floe.currents
    }
    /// Velocity of the water at a point, in metres per second.
    pub fn current(&self, at: Vec2) -> Vec2 {
        self.floe.currents.at(at)
    }
    pub fn icebergs(&self) -> &[Iceberg] {
        &self.floe.icebergs
    }

    /// Carry the ice on the current, ground it on the shores, part pieces that meet,
    /// break what the shore or the guns have beaten, and bring in more from upstream.
    pub(super) fn drift_ice(&mut self, dt: f32, player: Vec2) {
        if self.floe.icebergs.is_empty() && self.floe.stock == 0 {
            return;
        }
        let floe = &mut self.floe;
        let mut broken = vec![];
        for (index, berg) in floe.icebergs.iter_mut().enumerate() {
            let at = berg.body.pos;
            let stream = floe.currents.at(at);
            // Bigger ice answers the stream more slowly.
            let lag = 4. + 0.35 * berg.radius;
            berg.way += (stream - berg.way) * (1. - (-dt / lag).exp());
            berg.spin += (floe.currents.spin(at) - berg.spin) * (1. - (-dt / 25.).exp());
            berg.body.heading = (berg.body.heading + berg.spin * dt).rem_euclid(TAU);
            berg.body.pos += berg.way * dt;
            berg.rest = (berg.rest - dt).max(0.);
            let before = berg.body.pos;
            for island in &self.landmarks {
                berg.body.pos = island.clear(berg.body.pos, berg.radius);
            }
            for port in &self.ports {
                berg.body.pos = port.clear_land(berg.body.pos, berg.radius);
            }
            let pushed = berg.body.pos - before;
            let pressed = match pushed.try_normalize() {
                Some(normal) => {
                    // The shore takes the way off ice driven onto it.
                    let onto = berg.way.dot(normal);
                    if onto < 0. {
                        berg.way -= normal * onto;
                    }
                    stream.dot(normal) < -0.02
                }
                None => false,
            };
            berg.aground = if pressed {
                berg.aground + dt
            } else {
                (berg.aground - 2. * dt).max(0.)
            };
            if berg.aground > GROUNDED_SECS || berg.integrity <= 0. {
                broken.push(index);
            }
        }
        // Pieces that meet part without harm and share their momentum.
        let ice = &mut floe.icebergs;
        for i in 1..ice.len() {
            let (ahead, rest) = ice.split_at_mut(i);
            let b = &mut rest[0];
            for a in ahead {
                let gap = b.body.pos - a.body.pos;
                let reach = a.radius + b.radius;
                if gap.length_squared() >= reach * reach {
                    continue;
                }
                let normal = gap.try_normalize().unwrap_or(Vec2::X);
                let (ma, mb) = (a.mass(), b.mass());
                let depth = reach - gap.length();
                a.body.pos -= normal * depth * mb / (ma + mb);
                b.body.pos += normal * depth * ma / (ma + mb);
                let closing = (a.way - b.way).dot(normal);
                if closing > 0. {
                    let impulse = (1. + ICE_REBOUND) * closing / (1. / ma + 1. / mb);
                    a.way -= normal * impulse / ma;
                    b.way += normal * impulse / mb;
                }
            }
        }
        for &index in broken.iter().rev() {
            self.break_up(index);
        }
        let (lo, hi) = self.floe.reach;
        self.floe.icebergs.retain(|b| {
            let p = b.body.pos;
            p.x >= lo.x && p.y >= lo.y && p.x <= hi.x && p.y <= hi.y
        });
        self.floe.clock -= dt;
        if self.floe.clock <= 0. {
            self.floe.clock = REFILL_SECS;
            if self.floe.icebergs.len() < self.floe.stock {
                self.float_in(player);
            }
        }
    }

    /// Ice the shore or the guns have beaten splits into two or three smaller pieces, or
    /// shatters into brash if it was small already. Pieces knocked off by a blow fly on
    /// along its line, fanned out either side of it, the smaller the faster. Ice crushed
    /// on a shore just comes apart.
    fn break_up(&mut self, index: usize) {
        let berg = self.floe.icebergs.swap_remove(index);
        self.effects.push(Effect {
            body: body(&mut self.serial, berg.body.pos, berg.body.heading),
            kind: EffectKind::Shatter(berg.radius.round().min(255.) as u8),
            height: 0.,
            age: 0.,
        });
        if berg.radius < MELT_RADIUS {
            return;
        }
        let draw = self.floe.rng.next();
        let pieces = 2 + (draw % 2) as usize;
        let radius = berg.radius * if pieces == 2 { 0.66 } else { 0.55 };
        let (line, arc, speed) = match berg.blow.try_normalize() {
            Some(along) => (bearing(along), FAN, SCATTER / radius.sqrt()),
            // Evenly round the compass: opposite ways for two pieces, a third apart for three.
            None => (berg.body.heading, PI * (1. - 1. / pieces as f32), PARTING),
        };
        for k in 0..pieces {
            // From -1 on one edge of the fan to +1 on the other.
            let across = 2. * k as f32 / (pieces - 1) as f32 - 1.;
            let jitter = self.floe.rng.range(-0.15, 0.15);
            let out = direction(line + arc * across + jitter);
            let at = body(
                &mut self.serial,
                berg.body.pos + out * (berg.radius - radius * 0.8),
                berg.body.heading + k as f32 * 1.9,
            );
            let shape = IceShape::for_radius(radius, draw >> (8 * k + 4));
            let mut piece = Iceberg::new(at, radius, shape, berg.way + out * speed);
            // Each piece is struck off-centre and tumbles away from the line of the blow.
            piece.spin = berg.spin + across * (0.02 + 0.03 * speed);
            self.floe.icebergs.push(piece);
        }
    }

    /// One new piece of ice drifts in across the upstream edge of the water, if there is
    /// open water there out of the player's sight.
    fn float_in(&mut self, player: Vec2) {
        let (lo, hi) = self.floe.reach;
        for _ in 0..8 {
            let rng = &mut self.floe.rng;
            let inside = Vec2::new(rng.range(lo.x, hi.x), rng.range(lo.y, hi.y));
            let (radius, shape) = if rng.chance(self.floe.bergs) {
                let radius = rng.range(6., 15.);
                (radius, IceShape::for_radius(radius, rng.next()))
            } else {
                (rng.range(2.2, 4.6), IceShape::Growler)
            };
            let heading = rng.range(0., TAU);
            // Walk the point back up the stream to the edge it came in over.
            let up = -self.floe.currents.at(inside).normalize_or_zero();
            if up == Vec2::ZERO {
                return;
            }
            let exit = [(lo.x - inside.x) / up.x, (hi.x - inside.x) / up.x]
                .into_iter()
                .chain([(lo.y - inside.y) / up.y, (hi.y - inside.y) / up.y])
                .filter(|t| t.is_finite() && *t > 0.)
                .fold(f32::MAX, f32::min);
            let at = inside + up * (exit - radius - 20.).max(0.);
            let afloat = self
                .navigable_position(at, radius + 6.)
                .distance_squared(at)
                < 0.01
                && self
                    .floe
                    .icebergs
                    .iter()
                    .all(|b| b.body.pos.distance(at) > b.radius + radius + 6.);
            if afloat && at.distance(player) > SPAWN_STANDOFF {
                let stream = self.floe.currents.at(at);
                let at = body(&mut self.serial, at, heading);
                self.floe
                    .icebergs
                    .push(Iceberg::new(at, radius, shape, stream));
                return;
            }
        }
    }

    /// Hulls that meet ice: the ice gives way by its share of the mass, which for a berg
    /// is almost none, and a hard blow stoves in the ship and chips the ice.
    pub(super) fn ice_contacts(&mut self, orders: &Orders, events: &mut Events) {
        if self.floe.icebergs.is_empty() {
            return;
        }
        let forward = direction(orders.heading);
        let mut pos = orders.player + events.shove;
        let mut speed = orders.speed + events.jolt;
        let drift = self.floe.currents.at(pos);
        for berg in &mut self.floe.icebergs {
            let player = ram::Ship {
                hull: orders.hull,
                pos,
                forward,
                velocity: forward * speed + drift,
            };
            let Some(hit) = ram::contact(&player, &berg.hull()) else {
                continue;
            };
            pos += hit.give[0];
            // The helm only carries way along the keel.
            speed += hit.kick[0].dot(forward);
            berg.body.pos += hit.give[1];
            berg.way += hit.kick[1];
            if hit.closing >= ram::BUMP && berg.rest == 0. {
                berg.rest = ram::RAM_REST;
                // The pieces fly on ahead of her stem, along the line from her to the ice.
                berg.strike(CRUSHING * hit.closing, hit.normal);
                events.damage = events.damage.saturating_add(hit.damage[0]);
                if hit.damage[0] > 0 {
                    let reach = orders.hull.bow + orders.hull.stern;
                    let ahead = (hit.point - pos).dot(forward);
                    events.blows.push(Blow {
                        aft: (orders.hull.bow - ahead).clamp(0., reach),
                        abeam: (hit.point - pos).dot(forward.perp()),
                        damage: hit.damage[0],
                        calibre: None,
                        reach: shipvivor_shipyard::Reach::Bays,
                        breach: false,
                    });
                }
                self.effects.push(Effect {
                    body: body(&mut self.serial, hit.point, bearing(hit.normal)),
                    kind: EffectKind::Crunch,
                    height: 0.,
                    age: 0.,
                });
            }
        }
        events.shove = pos - orders.player;
        events.jolt = speed - orders.speed;
        for raider in &mut self.raiders {
            for berg in &mut self.floe.icebergs {
                let Some(hit) = ram::contact(&ram::afloat(raider), &berg.hull()) else {
                    continue;
                };
                raider.body.pos += hit.give[0];
                raider.knock += hit.kick[0];
                berg.body.pos += hit.give[1];
                berg.way += hit.kick[1];
                if hit.closing >= ram::BUMP && raider.ram_rest == 0. {
                    raider.ram_rest = ram::RAM_REST;
                    raider.hit_hull(hit.point, f32::from(hit.damage[0]));
                    berg.strike(CRUSHING * hit.closing, hit.normal);
                    self.effects.push(Effect {
                        body: body(&mut self.serial, hit.point, bearing(hit.normal)),
                        kind: EffectKind::Crunch,
                        height: 0.,
                        age: 0.,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
