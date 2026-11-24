//! Each yard owns its stock. Freighters sail from one standing yard to another to
//! unload it, so every ship in the sector comes from somewhere the player can see.
use super::{Battle, Body, EnemyClass, Vec2, YardState, entry};

const CROSSING: f32 = 490.;
pub const TOW_RANGE: f32 = 96.;
/// Stores a factory pays when it falls, under direct rewards.
pub const FACTORY_STORES: u8 = 1;
/// Stores a drydock pays when it falls, under direct rewards. It is three factories'
/// worth of building, and any factories still standing go down with it.
pub const DRYDOCK_STORES: u8 = 3;

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
    /// The approaches to this yard from the open sea with clear water all the way in,
    /// best first. The chart survey counts them.
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
                    .all(|p| entry(start, berth, p.body.pos, super::ISLAND_RADIUS + 9.).is_none())
                    && self
                        .landmarks
                        .iter()
                        .all(|p| entry(start, berth, p.body.pos, p.radius + 9.).is_none())
            })
    }
    /// Where a freighter for this yard casts off: the slip of the nearest other yard
    /// still standing, clear of shipping and of the player. Nothing comes in from the
    /// open sea, so a sector whose other yards are wrecked gets no more deliveries.
    fn sender(&self, index: usize, player: Vec2) -> Option<Vec2> {
        let class = EnemyClass::Freighter.profile().hull;
        let target = self.ports[index].berth();
        self.ports
            .iter()
            .enumerate()
            .filter(|&(j, port)| j != index && port.yard_state() != YardState::Ruined)
            .map(|(_, port)| {
                port.berth() + port.body.forward() * (class.stern - super::Hull::RAIDER.stern)
            })
            .filter(|&slip| {
                slip.distance(player) >= super::SPAWN_STANDOFF
                    && self.raiders.iter().all(|r| {
                        r.body.pos.distance(slip) > r.class.profile().hull.bow + class.bow + 2.
                    })
            })
            .min_by(|a, b| {
                a.distance_squared(target)
                    .total_cmp(&b.distance_squared(target))
            })
    }
    pub(super) fn send_convoys(&mut self, dt: f32, player: Vec2) {
        for i in 0..self.ports.len() {
            let port = &mut self.ports[i];
            if port.yard_state() == YardState::Ruined {
                continue;
            }
            port.convoy_clock = (port.convoy_clock - dt).max(0.);
            if port.convoy_clock > 0.
                || !port.needs_delivery()
                || self
                    .raiders
                    .iter()
                    .any(|r| !r.is_disabled() && r.destination_port() == Some(i))
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
            if port.yard_state() == YardState::Ruined {
                return r.body.pos.distance(origin) > 12.;
            }
            if r.body.pos.distance(port.berth()) <= 12. {
                port.unload();
                return false;
            }
            true
        });
    }
}
