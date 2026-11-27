//! Each yard owns its stock. Freighters sail from one standing yard to another to
//! unload it, so every ship in the sector comes from somewhere the player can see.
//! Coasters lie at anchor off each base until it is roused, then ply between the yards
//! still standing, and each roused yard sends more of them out from its slip.
use super::{Battle, Body, EnemyClass, Vec2, entry};

/// Length of a freighter's approach to a slip that the survey wants clear of land.
const CROSSING: f32 = 300.;
pub const TOW_RANGE: f32 = 96.;
/// Stores a factory pays when it falls, under direct rewards.
pub const FACTORY_STORES: u8 = 1;
/// Stores a drydock pays when it falls, under direct rewards. It is three factories'
/// worth of building. The factories pay their own when they fall.
pub const DRYDOCK_STORES: u8 = 3;
/// Seconds between the groups of coasters a roused yard sends out.
pub const COASTER_INTERVAL: f32 = 35.;
/// No yard sends out coasters while this many are afloat in the sector, at anchor or
/// under way.
pub const COASTER_LIMIT: usize = 8;
/// Coasters at anchor off each base when the sector opens.
pub const MOORED_COASTERS: usize = 2;
/// Where a base's coasters lie at anchor, in metres ahead of its berth and across the
/// line of its slip, best first: out in the roadstead, clear of the slip and its quay.
const ROADSTEAD: [(f32, f32); 6] = [
    (28., 34.),
    (28., -34.),
    (55., 42.),
    (55., -42.),
    (82., 30.),
    (82., -30.),
];

/// How the stores of a sunk ship come aboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rewards {
    /// The moment she sinks, and a wrecked yard building pays its stores too. No
    /// packages float.
    Direct,
    /// She leaves a package that drifts until the winch tows it alongside. Loose
    /// packages also lie ahead and off each berth when the sector opens.
    Packages,
}
impl Rewards {
    /// The stores a fallen ship or building pays at once.
    pub(super) fn at_once(self, stores: u8) -> u16 {
        match self {
            Self::Direct => u16::from(stores),
            Self::Packages => 0,
        }
    }
}

#[derive(Clone)]

pub struct Package {
    pub body: Body,
    pub stores: u8,
    pub hooked: bool,
}
impl Package {
    pub(super) fn new(body: Body, stores: u8) -> Self {
        Self {
            body,
            stores,
            hooked: false,
        }
    }
    /// Drift on `stream` until the winch has her, then come in on the tow. True once
    /// she is alongside.
    pub(super) fn haul(
        &mut self,
        player: Vec2,
        dt: f32,
        ports: &[super::Port],
        islands: &[super::Landmark],
        stream: Vec2,
    ) -> bool {
        if !self.hooked {
            let mut at = self.body.pos + stream * dt;
            for island in islands {
                at = island.clear(at, 1.5);
            }
            for port in ports {
                at = port.clear_land(at, 1.5);
            }
            self.body.pos = at;
        }
        let offset = player - self.body.pos;
        let range = offset.length();
        self.hooked = (self.hooked || range <= TOW_RANGE)
            && super::coast::open_water(self.body.pos, player, ports, islands);
        if self.hooked {
            let travel = (dt * 38.).min(range);
            self.body.pos += offset.normalize_or_zero() * travel;
            return range - travel <= 7.;
        }
        false
    }
}

impl Battle {
    /// The approaches to this yard with clear water all the way in for the last few
    /// hundred metres, best first. The chart survey counts them.
    pub(super) fn convoy_lanes(&self, index: usize) -> impl Iterator<Item = Vec2> + '_ {
        let port = &self.ports[index];
        let berth = port.berth();
        let forward = port.body.forward();
        [0., 0.52, -0.52, 1.05, -1.05, 1.48, -1.48]
            .into_iter()
            .map(move |turn| berth + Vec2::from_angle(turn).rotate(forward) * CROSSING)
            .filter(move |&start| {
                self.ports
                    .iter()
                    .flat_map(super::Port::shores)
                    .all(|(point, radius)| entry(start, berth, point, radius + 9.).is_none())
                    && self
                        .landmarks
                        .iter()
                        .all(|p| entry(start, berth, p.body.pos, p.radius + 9.).is_none())
            })
    }
    /// Where a ship of `class` casts off from yard `index`: its slip, while the yard
    /// stands, the player is at least `SPAWN_STANDOFF` off and no ship under way lies
    /// on it. A coaster at anchor lies out in the roadstead, clear of it.
    pub(super) fn slip(&self, index: usize, class: EnemyClass, player: Vec2) -> Option<Vec2> {
        let port = &self.ports[index];
        let hull = class.profile().hull;
        let slip = port.berth() + port.body.forward() * (hull.stern - super::Hull::RAIDER.stern);
        let clear = !port.wrecked()
            && slip.distance(player) >= super::SPAWN_STANDOFF
            && self.raiders.iter().all(|r| {
                r.moored.is_some()
                    || r.body.pos.distance(slip) > r.class.profile().hull.bow + hull.bow + 2.
            });
        clear.then_some(slip)
    }
    /// Where a freighter for this yard casts off: the slip of the nearest other yard
    /// still standing, clear of shipping and of the player. Nothing comes in from the
    /// open sea, so a sector whose other yards are wrecked gets no more deliveries.
    fn sender(&self, index: usize, player: Vec2) -> Option<Vec2> {
        let target = self.ports[index].berth();
        (0..self.ports.len())
            .filter(|&j| j != index)
            .filter_map(|j| self.slip(j, EnemyClass::Freighter, player))
            .min_by(|a, b| {
                a.distance_squared(target)
                    .total_cmp(&b.distance_squared(target))
            })
    }
    /// The standing yard nearest yard `index`, other than itself: where its coasters
    /// trade.
    fn trading_partner(&self, index: usize) -> Option<usize> {
        let home = self.ports[index].berth();
        (0..self.ports.len())
            .filter(|&j| j != index && !self.ports[j].wrecked())
            .min_by(|&a, &b| {
                let off = |j: usize| self.ports[j].berth().distance_squared(home);
                off(a).total_cmp(&off(b))
            })
    }
    /// Moor [`MOORED_COASTERS`] coasters in the roadstead off each base, on open water
    /// clear of the ice and of each other. They ride there until the base is roused.
    pub(crate) fn with_coasters(mut self) -> Self {
        let hull = EnemyClass::Coaster.profile().hull;
        for base in 0..self.ports.len() {
            let port = &self.ports[base];
            let (berth, forward) = (port.berth(), port.body.forward());
            let mut moored = 0;
            for (fore, across) in ROADSTEAD {
                if moored == MOORED_COASTERS {
                    break;
                }
                let spot = berth + forward * fore + forward.perp() * across;
                // No ship lies within the standoff of where the player enters.
                let clear = spot.length() >= super::SPAWN_STANDOFF
                    && self
                        .navigable_position(spot, hull.radius + 4.)
                        .distance(spot)
                        < 0.01
                    && self
                        .floe
                        .icebergs
                        .iter()
                        .all(|b| b.body.pos.distance(spot) > b.radius + hull.bow + 2.)
                    && self.raiders.iter().all(|r| {
                        r.body.pos.distance(spot) > r.class.profile().hull.bow + hull.bow + 2.
                    });
                if clear {
                    self.spawn(EnemyClass::Coaster, spot, spot + forward);
                    self.raiders.last_mut().unwrap().moor(base);
                    moored += 1;
                }
            }
        }
        self
    }
    /// Coasters at anchor off a roused base weigh for the nearest other standing yard.
    /// Each roused yard still standing sends out two or three more to it every
    /// [`COASTER_INTERVAL`] while fewer than [`COASTER_LIMIT`] are afloat. They leave
    /// its slip one at a time as it clears, and never with the player near it.
    pub(super) fn ply_coasters(&mut self, dt: f32, player: Vec2) {
        for r in 0..self.raiders.len() {
            let Some(home) = self.raiders[r].moored else {
                continue;
            };
            // A garrison's warships weigh for war, not for trade.
            if !self.ports[home].roused || !self.raiders[r].class.is_merchant() {
                continue;
            }
            if let Some(bound) = self.trading_partner(home) {
                let origin = self.ports[home].berth();
                self.raiders[r].weigh(bound, origin);
            }
        }
        let mut afloat = self
            .raiders
            .iter()
            .filter(|r| r.class == EnemyClass::Coaster)
            .count();
        for i in 0..self.ports.len() {
            let port = &mut self.ports[i];
            // A wrecked yard sends nothing, and a sleeping one is sending nothing yet.
            if port.wrecked() {
                port.coasters_due = None;
                continue;
            }
            if !port.roused {
                continue;
            }
            port.coaster_clock = (port.coaster_clock - dt).max(0.);
            if port.coasters_due.is_none() && port.coaster_clock == 0. {
                port.coaster_clock = 5.;
                if afloat < COASTER_LIMIT
                    && let Some(bound) = self.trading_partner(i)
                {
                    let group = 2 + (self.serial % 2) as u8;
                    let port = &mut self.ports[i];
                    port.coasters_due = Some((bound, group));
                    port.coaster_clock = COASTER_INTERVAL;
                }
            }
            let Some((bound, left)) = self.ports[i].coasters_due else {
                continue;
            };
            // The rest of a group bound for a yard that has fallen stays in.
            if self.ports[bound].wrecked() {
                self.ports[i].coasters_due = None;
                continue;
            }
            if let Some(slip) = self.slip(i, EnemyClass::Coaster, player) {
                let out = self.ports[i].body.forward();
                self.spawn(EnemyClass::Coaster, slip, slip + out);
                self.raiders.last_mut().unwrap().weigh(bound, slip);
                afloat += 1;
                self.ports[i].coasters_due = (left > 1).then_some((bound, left - 1));
            }
        }
    }
    pub(super) fn send_convoys(&mut self, dt: f32, player: Vec2) {
        for i in 0..self.ports.len() {
            let port = &mut self.ports[i];
            // A sleeping yard is building nothing, so it calls for no steel. Any yard
            // still standing may send it, so freighters come down from the deep yards.
            if port.wrecked() || !port.roused {
                continue;
            }
            port.convoy_clock = (port.convoy_clock - dt).max(0.);
            if port.convoy_clock > 0.
                || !port.needs_delivery()
                || self.raiders.iter().any(|r| {
                    r.class == EnemyClass::Freighter
                        && !r.is_disabled()
                        && r.destination_port() == Some(i)
                })
            {
                continue;
            }
            let target = port.berth();
            port.convoy_clock = 5.;
            if let Some(start) = self.sender(i, player) {
                self.spawn(EnemyClass::Freighter, start, target);
                self.raiders.last_mut().unwrap().cargo_run = Some((i, start));
                self.ports[i].convoy_clock = 77.;
            }
        }
    }
    pub(super) fn receive_convoys(&mut self) {
        self.raiders.retain(|r| {
            let Some((i, origin)) = r.cargo_run else {
                return true;
            };
            let port = &mut self.ports[i];
            if r.is_disabled() {
                return true;
            }
            if port.wrecked() {
                return r.body.pos.distance(origin) > 12.;
            }
            if r.body.pos.distance(port.berth()) <= 12. {
                // A freighter's steel goes into the yard. A coaster's trade is her own.
                if r.class == EnemyClass::Freighter {
                    port.unload();
                }
                return false;
            }
            true
        });
    }
}
