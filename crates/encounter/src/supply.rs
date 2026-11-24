//! Each yard owns its stock. Freighters physically cross the roadstead to unload it.
use super::{Battle, Body, EnemyClass, Vec2, YardState, entry};

const CROSSING: f32 = 490.;
pub const TOW_RANGE: f32 = 96.;

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
    /// Where a freighter for this yard can start, best first: clear water all the way in.
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
    fn crossing(&self, index: usize, player: Vec2) -> Option<Vec2> {
        self.convoy_lanes(index).find(|&start| {
            start.distance_squared(player) >= super::SPAWN_STANDOFF.powi(2)
                && self
                    .raiders
                    .iter()
                    .all(|r| r.body.pos.distance_squared(start) > 48_f32.powi(2))
        })
    }
    /// Escorts sweep in via a second charted approach. Prefer the bearing least
    /// covered by ships already closing so successive convoys threaten different flanks.
    fn convoy_escort(&mut self, port: usize, player: Vec2) {
        if self.ports[port].live_factories() == 0 || self.combatants() >= super::COMBATANT_LIMIT {
            return;
        }
        let class = if self.at_sea >= super::RAID_TIERS[1] {
            EnemyClass::Cruiser
        } else {
            EnemyClass::Destroyer
        };
        let coverage = |at: Vec2| {
            let bearing = (at - player).normalize_or_zero();
            self.raiders
                .iter()
                .filter(|r| r.class != EnemyClass::Freighter)
                .map(|r| bearing.dot((r.body.pos - player).normalize_or_zero()))
                .fold(-1., f32::max)
        };
        let station = self
            .convoy_lanes(port)
            .filter(|&p| {
                p.distance(player) >= super::SPAWN_STANDOFF
                    && self.raiders.iter().all(|r| {
                        r.body.pos.distance(p)
                            > r.class.profile().hull.bow + class.profile().hull.bow + 5.
                    })
            })
            .min_by(|a, b| coverage(*a).total_cmp(&coverage(*b)));
        if let Some(station) = station {
            self.spawn(class, station, player);
        }
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
            if let Some(start) = self.crossing(i, player) {
                self.spawn(EnemyClass::Freighter, start, target);
                self.raiders.last_mut().unwrap().cargo_run = Some((i, start));
                self.convoy_escort(i, player);
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
