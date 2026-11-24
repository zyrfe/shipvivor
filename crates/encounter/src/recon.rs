//! Lookouts and aircraft share sightings across the arcade sector.
use super::{Battle, Iceberg, Landmark, Port, YardState, bearing, footprint};
use glam::Vec2;

pub const AIR_SIGHT: f32 = 320.;
pub const BASE_SIGHT: f32 = 660.;
const RADIO_RANGE: f32 = 1100.;
pub const ALERT_SECONDS: f32 = 3.2;
const LOST_SECONDS: f32 = 3.;
const PLAYER_MAST: f32 = 6.;

pub struct Spotter {
    pub position: Vec2,
    pub heading: f32,
    pub altitude: f32,
    pub bank: f32,
    /// Time remaining on the exclamation mark for this acquisition.
    pub alert_left: f32,
    pub reporting: bool,
    // Once a contact drops, a brief gap in cover cannot repeatedly raise the alarm.
    lost_for: f32,
    center: Vec2,
    axis: Vec2,
    span: Vec2,
    phase: f32,
    rate: f32,
}

impl Spotter {
    pub(super) fn patrol(port: &Port, index: usize) -> Self {
        let axis = port.body.forward();
        let mut flight = Self {
            center: port.body.pos + axis * (145. + 18. * index as f32),
            axis,
            span: Vec2::new(208. + 14. * index as f32, 115.),
            phase: [2.76, 0.9, 4.48][index],
            rate: [0.15, -0.14, 0.13][index],
            position: Vec2::ZERO,
            heading: 0.,
            altitude: 0.,
            bank: 0.,
            alert_left: 0.,
            reporting: false,
            lost_for: LOST_SECONDS,
        };
        flight.fly_circuit(0.);
        flight
    }
    fn fly_circuit(&mut self, dt: f32) {
        self.phase = (self.phase + self.rate * dt).rem_euclid(std::f32::consts::TAU);
        let (s, c) = self.phase.sin_cos();
        let side = self.axis.perp();
        self.position = self.center + self.axis * (self.span.x * c) + side * (self.span.y * s);
        let tangent = side * (self.span.y * c) - self.axis * (self.span.x * s);
        self.heading = bearing(tangent * self.rate);
        self.altitude = 43. + 6. * (2. * self.phase).sin();
        self.bank = -self.rate.signum() * (0.12 + 0.29 * c * c);
    }
    fn observe(&mut self, dt: f32, in_sight: bool) {
        self.alert_left = (self.alert_left - dt).max(0.);
        if in_sight {
            if self.lost_for >= LOST_SECONDS {
                self.alert_left = ALERT_SECONDS;
            }
            self.lost_for = 0.;
        } else {
            self.lost_for += dt;
        }
        self.reporting = in_sight;
    }
    pub fn position_3d(&self) -> glam::Vec3 {
        glam::Vec3::new(self.position.x, self.altitude, self.position.y)
    }
}

/// The line from a lookout to the player's mast can pass over a low shoal or
/// behind a tall stack. Only the part of the ray inside each footprint counts.
fn sees(
    at: Vec2,
    height: f32,
    player: Vec2,
    range: f32,
    ports: &[Port],
    islands: &[Landmark],
    ice: &[Iceberg],
) -> bool {
    if at.distance_squared(player) > range * range {
        return false;
    }
    let blocks = |(center, radius, top)| {
        footprint(at, player, center, radius).is_some_and(|(enter, leave)| {
            let t = if height >= PLAYER_MAST { leave } else { enter };
            height + (PLAYER_MAST - height) * t <= top
        })
    };
    !islands.iter().flat_map(Landmark::volumes).any(blocks)
        && !ports.iter().flat_map(Port::land_volumes).any(blocks)
        && !ice.iter().map(Iceberg::volume).any(blocks)
}

impl Battle {
    pub fn spotters(&self) -> &[Spotter; 3] {
        &self.spotters
    }

    pub(super) fn update_lookouts(&mut self, dt: f32, player: Vec2) {
        for (plane, port) in self.spotters.iter_mut().zip(&self.ports) {
            if port.yard_state() == YardState::Ruined {
                // The patrol leaves on its present heading when its home signal goes dark.
                plane.position += shipvivor_compass::direction(plane.heading) * 35. * dt;
                plane.bank = 0.;
                plane.altitude = (plane.altitude + 2. * dt).min(94.);
                plane.alert_left = 0.;
                plane.observe(dt, false);
                continue;
            }
            plane.fly_circuit(dt);
            let range = if plane.lost_for < LOST_SECONDS {
                AIR_SIGHT + 35.
            } else {
                AIR_SIGHT
            };
            let seen = sees(
                plane.position,
                plane.altitude,
                player,
                range,
                &self.ports,
                &self.landmarks,
                &self.floe.icebergs,
            );
            plane.observe(dt, seen);
        }
        let shore_reports = self.ports.each_ref().map(|port| {
            port.yard_state() != YardState::Ruined
                && sees(
                    port.yard_position(),
                    port.yard_top(),
                    player,
                    BASE_SIGHT,
                    &self.ports,
                    &self.landmarks,
                    &self.floe.icebergs,
                )
        });
        let transmitters = self
            .spotters
            .iter()
            .filter(|plane| plane.reporting)
            .map(|plane| plane.position)
            .chain(
                self.ports
                    .iter()
                    .zip(shore_reports)
                    .filter(|(_, seen)| *seen)
                    .map(|(port, _)| port.yard_position()),
            );
        let hears_report = |at: Vec2| {
            transmitters
                .clone()
                .any(|source| at.distance_squared(source) <= RADIO_RANGE * RADIO_RANGE)
        };
        let shore_contacts = self
            .ports
            .each_ref()
            .map(|p| p.yard_state() != YardState::Ruined && hears_report(p.body.pos));
        for ship in &mut self.raiders {
            ship.has_contact = ship.class != super::EnemyClass::Freighter
                && (hears_report(ship.body.pos)
                    || sees(
                        ship.body.pos,
                        ship.class.profile().deck + 7.,
                        player,
                        ship.class.lookout_range(),
                        &self.ports,
                        &self.landmarks,
                        &self.floe.icebergs,
                    ));
            if ship.has_contact {
                ship.last_sighting = Some(player);
            }
        }
        for (port, contact) in self.ports.iter_mut().zip(shore_contacts) {
            port.has_contact = contact;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Chart, EnemyClass, HarbourSite, LandmarkKind};

    fn open_sector() -> Battle {
        let mut chart = Chart::classic([
            Vec2::new(0., -152.),
            Vec2::new(168., -424.),
            Vec2::new(496., -300.),
            Vec2::new(416., 76.),
            Vec2::new(72., 164.),
        ]);
        chart.islets.clear();
        chart.harbours = std::array::from_fn(|i| HarbourSite {
            pos: Vec2::X * (i as f32 * 5000.),
            heading: 0.,
        });
        let mut battle = Battle::for_chart(1000., &chart);
        battle.raiders.clear();
        battle
    }

    #[test]
    fn aircraft_report_beyond_surface_sight_then_the_ships_search_the_last_fix() {
        let mut b = open_sector();
        b.spotters[0].phase = 0.;
        let player = Vec2::new(0., -670.);
        b.spawn(EnemyClass::Patrol, Vec2::new(900., -230.), player);
        b.spawn(EnemyClass::Patrol, Vec2::new(2000., -670.), player);
        b.update_lookouts(0., player);
        assert!(b.spotters[0].reporting && b.spotters[0].alert_left == ALERT_SECONDS);
        assert!(b.ports[0].has_contact && b.raiders[0].has_contact);
        assert!(!b.raiders[1].has_contact);
        b.update_lookouts(0.1, Vec2::splat(-3000.));
        assert!(!b.ports[0].has_contact && !b.raiders[0].has_contact);
        assert_eq!(b.raiders[0].last_sighting, Some(player));
        b.ports[0].damage_yard(u16::MAX);
        let before = b.spotters[0].position;
        b.update_lookouts(4., player);
        assert!(b.spotters[0].position.distance(before) > 100.);
        assert_eq!(b.spotters[0].alert_left, 0.);
        assert!(!b.spotters[0].reporting && !b.ports[0].has_contact);
    }

    #[test]
    fn the_alert_pops_once_until_the_spotter_really_loses_the_ship() {
        let mut b = open_sector();
        let player = b.spotters[0].position;
        b.update_lookouts(0., player);
        for _ in 0..40 {
            b.update_lookouts(0.1, player);
        }
        assert!(b.spotters[0].reporting);
        assert_eq!(b.spotters[0].alert_left, 0.);
        b.update_lookouts(0.25, Vec2::splat(-3000.));
        b.update_lookouts(0., player);
        assert_eq!(b.spotters[0].alert_left, 0.);
        b.update_lookouts(LOST_SECONDS + 0.1, Vec2::splat(-3000.));
        b.update_lookouts(0., b.spotters[0].position);
        assert_eq!(b.spotters[0].alert_left, ALERT_SECONDS);
    }

    #[test]
    fn shore_and_ship_main_guns_can_reach_their_extended_firing_ranges() {
        use crate::{Aim, Battery, EffectKind, Hull, MainShell, Orders};
        for (shore, range, battery) in
            [(true, 305., Battery::Shore), (false, 202., Battery::Raider)]
        {
            let mut b = open_sector();
            let port = &b.ports[0];
            let gun = &port.turrets()[0];
            let at = gun.position(&port.body);
            let outward = (at - port.body.pos).normalize();
            let player = if shore {
                at + outward * range
            } else {
                let station = Vec2::new(1800., 0.);
                b.spawn(EnemyClass::Patrol, station, station + Vec2::X);
                station + Vec2::X * range
            };
            let orders = Orders {
                player,
                heading: 0.,
                speed: 0.,
                hull: Hull::CRUISER,
                aim: Aim::Hold,
                reload: 3.2,
                shell: MainShell::STOCK,
            };
            b.wing_guns.clear();
            let mut rounds = 0;
            let mut damage = 0;
            for _ in 0..60 * 9 {
                damage += b.step(1. / 60., &orders).damage;
                rounds += b
                    .effects()
                    .iter()
                    .filter(|fx| fx.kind == EffectKind::Muzzle(battery) && fx.age == 0.)
                    .count();
            }
            assert!(
                rounds > 0 && damage > 0,
                "{battery:?} from {range} m: {rounds} rounds, {damage} damage"
            );
        }
    }

    #[test]
    fn direct_lookouts_see_farther_than_gun_range_and_high_ground_masks_them() {
        let mut b = open_sector();
        let player = Vec2::NEG_Y * 625.;
        b.update_lookouts(0., player);
        assert!(!b.spotters[0].reporting && b.ports[0].has_contact);
        let gun = &b.ports[0].turrets()[0];
        assert!(!gun.covers(&b.ports[0].body, player));

        for port in &mut b.ports {
            port.damage_yard(u16::MAX);
        }
        let observer = Vec2::new(100., -625.);
        b.spawn(EnemyClass::Destroyer, observer, observer + Vec2::X);
        let mark = observer + Vec2::X * 625.;
        b.update_lookouts(0., mark);
        assert!(b.raiders[0].has_contact);
        b.update_lookouts(0., observer + Vec2::X * 680.);
        assert!(!b.raiders[0].has_contact);
        b.landmarks.push(Landmark {
            body: crate::body(&mut b.serial, observer + Vec2::X * 130., 0.),
            radius: 35.,
            kind: LandmarkKind::Stacks,
        });
        b.update_lookouts(0., mark);
        assert!(!b.raiders[0].has_contact);
        assert!(sees(observer, 47., mark, 650., &b.ports, &b.landmarks, &[]));
    }
}
