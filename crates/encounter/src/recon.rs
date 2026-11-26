//! Lookouts and aircraft share sightings across the arcade sector.
use super::{
    Battle, Iceberg, Landmark, Port, SCRAMBLE_ALERT, bearing, footprint, ranging::RANGEFINDER,
};
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
    /// Metres per second over the water, as it last flew.
    pub way: Vec2,
    /// Hits it can still take before it goes down.
    pub(super) health: u16,
    /// Shot down, and gone from the base's lookout for the rest of the leg.
    downed: bool,
    /// Moored at its base, which has not yet been roused. It takes off when it is.
    moored: bool,
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
            phase: [2.76, 0.9, 4.48][index % 3] + 0.7 * (index / 3) as f32,
            rate: [0.15, -0.14, 0.13][index % 3],
            position: Vec2::ZERO,
            heading: 0.,
            altitude: 0.,
            bank: 0.,
            alert_left: 0.,
            reporting: false,
            way: Vec2::ZERO,
            health: super::flak::SPOTTER_STRENGTH,
            downed: false,
            moored: !port.roused,
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
        self.way = tangent * self.rate;
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
    /// Still flying. A floatplane shot down stays down for the rest of the leg, and one
    /// whose base sleeps has not taken off.
    pub fn airborne(&self) -> bool {
        !self.downed && !self.moored
    }
    pub(super) fn shoot_down(&mut self) {
        self.downed = true;
        self.health = 0;
        self.reporting = false;
        self.alert_left = 0.;
        self.way = Vec2::ZERO;
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
    pub fn spotters(&self) -> &[Spotter] {
        &self.spotters
    }

    pub(super) fn update_lookouts(&mut self, dt: f32, player: Vec2) {
        // Every base keeps a lookout from its tallest standing works, roused or not, and
        // its guns keep one while they stand. A sleeping yard that sights the ship is
        // roused, and stirs the stages before it.
        let shore_reports: Vec<bool> = self
            .ports
            .iter()
            .map(|port| {
                port.lookout().is_some_and(|(at, top)| {
                    sees(
                        at,
                        top,
                        player,
                        BASE_SIGHT,
                        &self.ports,
                        &self.landmarks,
                        &self.floe.icebergs,
                    )
                })
            })
            .collect();
        for (port, &seen) in self.ports.iter_mut().zip(&shore_reports) {
            port.alarm |= seen;
        }
        self.rouse();
        for (plane, port) in self.spotters.iter_mut().zip(&self.ports) {
            if plane.downed {
                continue;
            }
            if plane.moored {
                if !port.roused || port.wrecked() {
                    continue;
                }
                plane.moored = false;
            }
            if port.wrecked() {
                // The patrol leaves on its present heading when its home signal goes dark.
                plane.way = shipvivor_compass::direction(plane.heading) * 35.;
                plane.position += plane.way * dt;
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
        // A base at alert passes word of her too, whether or not it can see her.
        let alerted: Vec<Vec2> = (0..self.ports.len())
            .filter(|&i| self.at_alert(i) && !self.ports[i].wrecked())
            .map(|i| self.ports[i].body.pos)
            .collect();
        let transmitters = self
            .spotters
            .iter()
            .filter(|plane| plane.reporting)
            .map(|plane| plane.position)
            .chain(
                self.ports
                    .iter()
                    .zip(&shore_reports)
                    .filter(|(port, seen)| **seen && port.roused)
                    .filter_map(|(port, _)| port.lookout().map(|(at, _)| at)),
            )
            .chain(alerted.iter().copied());
        let hears_report = |at: Vec2| {
            transmitters
                .clone()
                .any(|source| at.distance_squared(source) <= RADIO_RANGE * RADIO_RANGE)
        };
        let shore_contacts: Vec<bool> = self
            .ports
            .iter()
            .map(|p| p.roused && p.lookout().is_some() && hears_report(p.body.pos))
            .collect();
        for ship in &mut self.raiders {
            ship.has_contact = !ship.class.is_merchant()
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
        // A heavy battery has her through its own rangefinder, which sees as far as its
        // guns reach, or by its base's word. A floatplane reporting her, or another base's
        // lookout, spots the fall of its shot for it.
        let in_radio =
            |at: Vec2, source: Vec2| at.distance_squared(source) <= RADIO_RANGE * RADIO_RANGE;
        let batteries: Vec<Vec<(bool, bool)>> = (self.ports.iter().enumerate())
            .map(|(i, port)| {
                // A battery fights on whatever becomes of its yard.
                let standing = port.roused;
                let guns = port.turrets.iter();
                guns.map(|gun| {
                    if !gun.heavy_battery() || !gun.live() || !standing {
                        return (false, false);
                    }
                    let at = gun.position(&port.body);
                    let (ports, land, ice) = (&self.ports, &self.landmarks, &self.floe.icebergs);
                    let sighted = sees(at, RANGEFINDER, player, gun.reach(), ports, land, ice);
                    let aloft = (self.spotters.iter())
                        .any(|plane| plane.reporting && in_radio(port.body.pos, plane.position));
                    let ashore = (self.ports.iter().zip(&shore_reports).enumerate()).any(
                        |(j, (other, &seen))| {
                            j != i
                                && seen
                                && other.roused
                                && in_radio(port.body.pos, other.body.pos)
                        },
                    );
                    (sighted, aloft || ashore)
                })
                .collect()
            })
            .collect();
        let alert: Vec<bool> = (0..self.ports.len()).map(|i| self.at_alert(i)).collect();
        for (((port, contact), guns), alert) in (self.ports.iter_mut())
            .zip(shore_contacts)
            .zip(batteries)
            .zip(alert)
        {
            // A base at alert keeps her in its plot, whether or not it can see her.
            port.has_contact = contact || (alert && port.roused);
            for (gun, (sighted, spotted)) in port.turrets.iter_mut().zip(guns) {
                if let Some(plot) = &mut gun.ranging {
                    plot.in_sight = sighted || contact;
                    plot.spotted = spotted;
                }
            }
        }
    }

    /// Rouse each stage whose turn has come: every base of the stage before it is
    /// wrecked, or one of its bases, or a base of a later stage, has sighted the ship or
    /// been shelled. A stage once roused stays so. A stage roused because the one before
    /// it collapsed is scrambled: it goes to alert for [`SCRAMBLE_ALERT`] seconds, its
    /// squadron sails at once for where she is, and its harriers take off as soon as they
    /// may.
    pub(super) fn rouse(&mut self) {
        let stages = self
            .ports
            .iter()
            .map(|p| usize::from(p.stage()) + 1)
            .max()
            .unwrap_or(0);
        // The deepest stage that has had word of the ship at first hand.
        let alarmed = self
            .ports
            .iter()
            .filter(|p| p.alarm)
            .map(|p| usize::from(p.stage()) + 1)
            .max()
            .unwrap_or(0);
        for port in &mut self.ports {
            port.alarm = false;
        }
        for stage in 0..stages {
            let cleared = stage == 0
                || self
                    .ports
                    .iter()
                    .filter(|p| usize::from(p.stage()) + 1 == stage)
                    .all(|p| p.wrecked());
            if cleared || stage < alarmed {
                let now = self.at_sea;
                for (i, port) in self.ports.iter_mut().enumerate() {
                    if usize::from(port.stage()) != stage {
                        continue;
                    }
                    // The base before it has just collapsed: sound the alarm here.
                    if stage > 0 && cleared && !port.roused() && !port.wrecked() {
                        self.alert_until[i] = now + SCRAMBLE_ALERT;
                        self.harrier_due[i] = now;
                    }
                    port.rouse(now);
                }
            }
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
        chart.harbours = (0..3)
            .map(|i| HarbourSite {
                pos: Vec2::X * (i as f32 * 5000.),
                heading: 0.,
                works: i as u8 + 1,
                stage: 0,
            })
            .collect();
        let mut battle = Battle::for_chart(1000., &chart);
        battle.raiders.clear();
        battle
    }

    /// Three bases far apart, one to a stage, the first nearest the origin.
    fn staged_sector() -> Battle {
        let mut chart = Chart::classic([
            Vec2::new(0., -152.),
            Vec2::new(168., -424.),
            Vec2::new(496., -300.),
            Vec2::new(416., 76.),
            Vec2::new(72., 164.),
        ]);
        chart.islets.clear();
        chart.harbours = (0..3)
            .map(|i| HarbourSite {
                pos: Vec2::X * (i as f32 * 5000.),
                heading: 0.,
                works: 1,
                stage: i as u8,
            })
            .collect();
        let mut battle = Battle::for_chart(1000., &chart);
        battle.raiders.clear();
        battle
    }

    #[test]
    fn later_stages_sleep_until_the_stage_before_falls() {
        use crate::{Aim, Hull, MainShell, Orders};
        let mut b = staged_sector();
        let away = Vec2::new(2500., 3000.);
        let orders = Orders {
            player: away,
            heading: 0.,
            speed: 0.,
            hull: Hull::CRUISER,
            aim: Aim::Hold,
            reload: 3.2,
            shell: MainShell::STOCK,
        };
        for _ in 0..600 {
            b.step(0.1, &orders);
        }
        let roused: Vec<bool> = b.ports.iter().map(Port::roused).collect();
        assert_eq!(roused, [true, false, false]);
        // A sleeping yard builds, but holds what it builds on its slip and keeps its
        // floatplane on the water.
        assert!(b.ports[0].stock > 2);
        assert!(b.ports[1..].iter().all(|p| p.stock > 2));
        assert!(b.spotters[0].airborne());
        assert!(b.spotters[1..].iter().all(|p| !p.airborne()));
        // The first stage falls, and the second stirs. The third sleeps on.
        b.ports[0].wreck();
        b.step(0.1, &orders);
        let roused: Vec<bool> = b.ports.iter().map(Port::roused).collect();
        assert_eq!(roused, [true, true, false]);
        assert!(b.spotters[1].airborne() && !b.spotters[2].airborne());
        for _ in 0..600 {
            b.step(0.1, &orders);
        }
        assert!(b.ports[2].stock > 2 && !b.ports[2].roused());
    }

    #[test]
    fn squadrons_grow_up_the_channel_and_a_collapse_scrambles_the_next_at_her() {
        use crate::{Aim, Hull, MainShell, Orders, SQUADRON_TIERS};
        use EnemyClass::*;
        let squadron = |b: &Battle, base: usize| -> Vec<EnemyClass> {
            (b.raiders.iter())
                .filter(|r| r.moored_at() == Some(base))
                .map(|r| r.class)
                .collect()
        };
        // Heavier up the channel, and heavier again deeper into the run. Each lies in
        // its harbour from the start: none is added afterwards.
        let light = staged_sector().with_squadrons(0);
        assert_eq!(squadron(&light, 1), [Patrol, Patrol, Destroyer]);
        assert_eq!(squadron(&light, 2), [Destroyer, Destroyer, Cruiser]);
        let heavy = staged_sector().with_squadrons(SQUADRON_TIERS + 4);
        assert_eq!(squadron(&heavy, 0), [Destroyer]);
        assert_eq!(squadron(&heavy, 1), [Destroyer, Cruiser, Cruiser]);
        assert_eq!(squadron(&heavy, 2), [Cruiser, Battleship, Battleship]);

        let mut b = staged_sector().with_squadrons(0);
        let away = Vec2::new(2500., 3000.);
        let orders = Orders {
            player: away,
            heading: 0.,
            speed: 0.,
            hull: Hull::CRUISER,
            aim: Aim::Hold,
            reload: 3.2,
            shell: MainShell::STOCK,
        };
        for _ in 0..50 {
            b.step(0.1, &orders);
        }
        assert!((0..3).all(|i| !b.at_alert(i)));
        // The first base collapses: the second goes to alert and its whole squadron
        // weighs for where she is, though none of it can see her. The third sleeps on.
        b.ports[0].wreck();
        b.step(0.1, &orders);
        assert!(b.at_alert(1) && !b.at_alert(2));
        assert!(b.ports[1].roused() && b.ports[1].has_contact);
        assert!(!b.ports[2].roused());
        let scrambled: Vec<u64> = (b.raiders.iter())
            .filter(|r| !r.class.is_merchant() && r.attack.is_none())
            .filter(|r| r.body.pos.distance(b.ports[1].body.pos) < 400.)
            .map(|r| r.body.id)
            .collect();
        assert_eq!(scrambled.len(), 3);
        let range = |b: &Battle, id: u64| b.ship(id).unwrap().body.pos.distance(away);
        let start: Vec<f32> = scrambled.iter().map(|&id| range(&b, id)).collect();
        // Once they have weighed, swung round and worked up to speed, they close her.
        for _ in 0..400 {
            b.step(0.1, &orders);
        }
        for (&id, start) in scrambled.iter().zip(start) {
            let ship = b.ship(id).unwrap();
            assert!(ship.moored_at().is_none() && ship.last_sighting == Some(away));
            assert!(range(&b, id) < start - 60., "{:?} held back", ship.class);
        }
        // The alarm stands down after a minute, and the squadron hunts on from its fix.
        for _ in 0..(SCRAMBLE_ALERT * 10.) as usize {
            b.step(0.1, &orders);
        }
        assert!(!b.at_alert(1));
    }

    #[test]
    fn a_sleeping_yard_that_sights_the_ship_rouses_its_stage_and_those_before() {
        let mut b = staged_sector();
        // In plain sight of the last yard, and far from the others.
        let player = b.ports[2].dock_position(0) + Vec2::NEG_Y * 600.;
        b.update_lookouts(0.1, player);
        assert!(b.ports.iter().all(Port::roused));
        assert!(b.ports[2].has_contact);
        // Once roused, a yard stays so after it loses the ship.
        b.update_lookouts(0.1, Vec2::splat(-9000.));
        assert!(b.ports.iter().all(Port::roused));
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
        b.ports[0].wreck();
        b.ports[0].silence();
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
            // A shore gun's layers scatter at the edge of its reach, so a round that
            // falls close aboard shows it reaches her as well as a hit does.
            let mut close = 0;
            for _ in 0..60 * 9 {
                damage += b.step(1. / 60., &orders).damage;
                let fresh = b.effects().iter().filter(|fx| fx.age == 0.);
                for fx in fresh {
                    rounds += usize::from(fx.kind == EffectKind::Muzzle(battery));
                    let fell = matches!(fx.kind, EffectKind::Splash | EffectKind::Burst);
                    close += usize::from(fell && fx.body.pos.distance(player) < 40.);
                }
            }
            assert!(
                rounds > 0 && (damage > 0 || (shore && close > 0)),
                "{battery:?} from {range} m: {rounds} rounds, {damage} damage, {close} close"
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
            port.wreck();
            port.silence();
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
