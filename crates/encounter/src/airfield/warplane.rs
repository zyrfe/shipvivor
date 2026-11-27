//! The airfields' aircraft in the air. A wing climbs away from the runway and comes for
//! her together, spread across her track. A light bomber lines up, glides down and drops
//! a stick of two bombs across where she will be. A torpedo plane swings out onto her
//! beam, comes in low and drops its torpedo ahead of her, then breaks away. Each flies
//! home empty, lands and taxis back to its shelter to fuel again.
use super::{FIELD_LEVEL, LIFT_OFF, PlaneKind, RUNWAY, TOUCH_DOWN, Taxi};
use crate::air_station::Transit;
use crate::aviation::Bomb;
use crate::{Battle, Body, Effect, EffectKind, Events, LandmarkKind, TORPEDO_SPEED, intercept};
use glam::{Vec2, Vec3, Vec3Swizzles};
use shipvivor_compass::heading;

/// Speed in the air: a light bomber, then a torpedo plane.
const SPEED: [f32; 2] = [44., 40.];
/// Radians a second it turns, and how far it banks into a turn.
const TURN: f32 = 0.75;
const BANK: f32 = 0.9;
/// Metres a second it climbs or comes down.
const CLIMB_RATE: f32 = 8.;
/// A bomber's height on the way in and on its run.
const BOMBER_HEIGHT: f32 = 34.;
const BOMB_RUN: f32 = 22.;
/// A torpedo plane's height on the way in and when it drops.
const TORPEDO_HEIGHT: f32 = 20.;
const TORPEDO_RUN: f32 = 6.;
/// Range at which a bomber commits to its run, and how nearly lined up it must be.
const COMMIT: f32 = 260.;
const LINED_UP: f32 = 0.3;
/// Seconds a bomb takes to fall from its run, which the bomber leads her by.
const FALL: f32 = 1.8;
/// Metres short of and past the mark the two bombs of a stick fall.
const STICK: [f32; 2] = [4., -4.];
/// Metres out on her beam a torpedo plane swings to before it comes in, the ranges it
/// drops between, how nearly it must point at where she will be, and how far off its
/// aim it lays the torpedo, either side by turns.
const BEAM: f32 = 300.;
const DROP: (f32, f32) = (150., 280.);
const DROP_LINE: f32 = 0.12;
const TORPEDO_ERROR: f32 = 14.;
/// Metres out a plane breaks away after its attack.
const BREAK: f32 = 190.;
/// Metres apart across her track the aircraft of a wing come in.
const SPREAD: f32 = 16.;
/// Height it comes home at, and the glide down to the runway from the far end.
const FINALS: f32 = 14.;
const GLIDE: f32 = 0.11;
/// What each kind takes from the AA before it goes down.
pub(super) const STRENGTH: [u16; 2] = [6, 5];
/// The most aircraft all the airfields keep in the air and on their way to it.
pub(super) const MOST_ALOFT: usize = 8;

enum Leg {
    /// Away from the runway and up to its height.
    Climb,
    /// Coming for her: a bomber at its height, a torpedo plane out to her beam.
    Inbound,
    /// A bomber's run to the mark, or a torpedo plane's run in low.
    Run { mark: Vec2, dropped: u8 },
    /// Clear of her after a run, round to the given point.
    Away(Vec2),
    /// Home to the end of the runway.
    Home(Transit),
    /// Down onto the runway.
    Finals,
}

/// One of an airfield's aircraft in the air.
pub struct Warplane {
    pub body: Body,
    pub altitude: f32,
    pub bank: f32,
    pub way: Vec2,
    pub kind: PlaneKind,
    /// The base it flies from, and its shelter there.
    pub base: usize,
    pub shelter: usize,
    pub(crate) health: u16,
    armed: bool,
    /// Its place across her track in the wing, from one side to the other.
    side: f32,
    leg: Leg,
}

impl Warplane {
    pub(super) fn lift_off(base: usize, taxi: &Taxi) -> Self {
        let kind = taxi.kind;
        Self {
            body: taxi.body,
            altitude: FIELD_LEVEL + 1.,
            bank: 0.,
            way: taxi.body.forward() * LIFT_OFF,
            kind,
            base,
            shelter: taxi.shelter,
            health: STRENGTH[kind as usize],
            armed: true,
            side: (taxi.shelter as f32 - 2.) * 0.5,
            leg: Leg::Climb,
        }
    }
    pub fn position_3d(&self) -> Vec3 {
        self.body.pos.extend(self.altitude).xzy()
    }
    /// Still carrying its bomb or torpedo, and coming for her.
    pub fn attacking(&self) -> bool {
        self.armed && matches!(self.leg, Leg::Inbound | Leg::Run { .. } | Leg::Climb)
    }
    pub fn armed(&self) -> bool {
        self.armed
    }
    /// On its way home or coming down onto the runway.
    pub fn homebound(&self) -> bool {
        matches!(self.leg, Leg::Home(_) | Leg::Finals)
    }
    fn speed(&self) -> f32 {
        SPEED[self.kind as usize]
    }
    /// Turn toward `goal` at its turning rate, banking into the turn.
    fn turn_toward(&mut self, dt: f32, goal: Vec2) {
        let want = heading(goal - self.body.pos);
        let error = wrap(want - self.body.heading);
        let turn = error.clamp(-TURN * dt, TURN * dt);
        self.body.heading = wrap(self.body.heading + turn);
        let rate = if dt > 0. { turn / dt } else { 0. };
        self.bank += ((-rate / TURN * BANK) - self.bank) * (dt * 4.).min(1.);
    }
    fn climb_toward(&mut self, dt: f32, height: f32) {
        let step = CLIMB_RATE * dt;
        self.altitude += (height - self.altitude).clamp(-step, step);
    }
    fn fly(&mut self, dt: f32, speed: f32) {
        self.way = self.body.forward() * speed;
        self.body.pos += self.way * dt;
    }
}

/// An angle wrapped to between minus and plus a half turn.
fn wrap(a: f32) -> f32 {
    (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

impl Battle {
    /// The height an aircraft over `at` keeps above the islands and the shore, with room
    /// to spare. An airfield's own level island is no obstacle.
    fn air_clearance(&self, at: Vec2) -> f32 {
        self.landmarks
            .iter()
            .filter(|island| island.kind != LandmarkKind::Airfield)
            .flat_map(|island| island.volumes())
            .map(|(centre, radius, top)| top + 6. - 0.45 * (at.distance(centre) - radius).max(0.))
            .fold(0., f32::max)
    }

    /// Fly every airfield aircraft on for a step: at her, at `player` making `way`,
    /// then home, down onto the runway and back to its shelter.
    pub(crate) fn fly_warplanes(&mut self, dt: f32, player: Vec2, way: Vec2, _events: &mut Events) {
        let mut landed = vec![];
        for n in 0..self.warplanes.len() {
            let clearance = self.air_clearance(self.warplanes[n].body.pos);
            let Some(field) = self.ports[self.warplanes[n].base].airfield.as_deref() else {
                landed.push(n);
                continue;
            };
            let (approach, inbound) = field.approach();
            let field_frame = field.field;
            let plane = &mut self.warplanes[n];
            let speed = plane.speed();
            let range = plane.body.pos.distance(player);
            let ahead = way.try_normalize().unwrap_or(plane.body.forward());
            let across = ahead.perp();
            let spread = across * plane.side * SPREAD;
            let mut drop_bomb = None;
            let mut drop_torpedo = None;
            // The leg comes out while the plane flies it, and goes back after.
            let mut leg = std::mem::replace(&mut plane.leg, Leg::Finals);
            let next = match &mut leg {
                Leg::Climb => {
                    let cruise = match plane.kind {
                        PlaneKind::Bomber => BOMBER_HEIGHT,
                        PlaneKind::Torpedo => TORPEDO_HEIGHT,
                    };
                    // Straight out along the runway's line until clear, then round for her.
                    if plane.altitude > FIELD_LEVEL + 10. {
                        plane.turn_toward(dt, player + spread);
                    }
                    plane.climb_toward(dt, cruise);
                    plane.fly(dt, speed);
                    (plane.altitude >= cruise * 0.7).then_some(Leg::Inbound)
                }
                Leg::Inbound => match plane.kind {
                    PlaneKind::Bomber => {
                        let lead = player + way * (range / speed) + spread;
                        plane.turn_toward(dt, lead);
                        plane.climb_toward(dt, BOMBER_HEIGHT.max(clearance));
                        plane.fly(dt, speed);
                        let to = lead - plane.body.pos;
                        let lined = plane.body.forward().angle_to(to).abs() < LINED_UP;
                        if range < COMMIT && range > 110. && lined {
                            let mark = player + way * (range / speed + FALL) + spread * 0.5;
                            Some(Leg::Run { mark, dropped: 0 })
                        } else if range <= 110. {
                            let f = plane.body.forward();
                            Some(Leg::Away(plane.body.pos + f * BREAK + f.perp() * BREAK))
                        } else {
                            None
                        }
                    }
                    PlaneKind::Torpedo => {
                        let side = if plane.side < 0. { -1. } else { 1. };
                        let beam = across * side;
                        let setup = player + beam * BEAM + ahead * (BEAM * 0.25);
                        plane.turn_toward(dt, setup);
                        plane.climb_toward(dt, TORPEDO_HEIGHT.max(clearance));
                        plane.fly(dt, speed);
                        let bearing = (plane.body.pos - player).normalize_or_zero();
                        let on_beam = bearing.dot(beam) > 0.7 && range < BEAM + 60.;
                        (plane.body.pos.distance(setup) < 80. || on_beam).then_some(Leg::Run {
                            mark: Vec2::ZERO,
                            dropped: 0,
                        })
                    }
                },
                Leg::Run { mark, dropped } => match plane.kind {
                    PlaneKind::Bomber => {
                        plane.climb_toward(dt, BOMB_RUN.max(clearance));
                        plane.fly(dt, speed);
                        plane.bank *= 1. - (dt * 3.).min(1.);
                        let left = plane.body.forward().dot(*mark - plane.body.pos);
                        if (*dropped as usize) < STICK.len() && left <= STICK[*dropped as usize] {
                            *dropped += 1;
                            drop_bomb = Some(plane.position_3d());
                            if *dropped as usize == STICK.len() {
                                plane.armed = false;
                            }
                        }
                        (left < -60.).then(|| {
                            Leg::Home(Transit::new(
                                plane.body.pos,
                                plane.body.forward(),
                                approach,
                                inbound,
                                speed,
                            ))
                        })
                    }
                    PlaneKind::Torpedo => {
                        let side = if plane.side < 0. { -1. } else { 1. };
                        let error = across
                            * side
                            * TORPEDO_ERROR
                            * if plane.body.id % 2 == 0 { 1. } else { -1. };
                        let aim = intercept(plane.body.pos, player, way, TORPEDO_SPEED)
                            .unwrap_or(player)
                            + error;
                        plane.turn_toward(dt, aim);
                        plane.climb_toward(dt, TORPEDO_RUN.max(clearance));
                        plane.fly(dt, speed);
                        let f = plane.body.forward();
                        let lined = f.angle_to(aim - plane.body.pos).abs() < DROP_LINE;
                        if (DROP.0..=DROP.1).contains(&range) && lined && plane.altitude < 10. {
                            plane.armed = false;
                            drop_torpedo = Some((plane.body.pos, heading(aim - plane.body.pos)));
                            Some(Leg::Away(
                                plane.body.pos + f * BREAK - f.perp() * side * BREAK,
                            ))
                        } else if range < DROP.0 {
                            Some(Leg::Away(
                                plane.body.pos + f * BREAK - f.perp() * side * BREAK,
                            ))
                        } else {
                            None
                        }
                    }
                },
                Leg::Away(to) => {
                    let to = *to;
                    plane.turn_toward(dt, to);
                    plane.climb_toward(dt, BOMBER_HEIGHT.max(clearance));
                    plane.fly(dt, speed);
                    (plane.body.pos.distance(to) < 45.).then(|| {
                        if plane.armed {
                            Leg::Inbound
                        } else {
                            Leg::Home(Transit::new(
                                plane.body.pos,
                                plane.body.forward(),
                                approach,
                                inbound,
                                speed,
                            ))
                        }
                    })
                }
                Leg::Home(transit) => {
                    let before = plane.body.pos;
                    let (pos, tangent, t) = transit.step(dt);
                    plane.body.pos = pos;
                    if tangent.length_squared() > 1e-6 {
                        let turned = heading(tangent);
                        let rate = wrap(turned - plane.body.heading) / dt.max(1e-3);
                        plane.bank += ((-rate / TURN * BANK).clamp(-BANK, BANK) - plane.bank)
                            * (dt * 4.).min(1.);
                        plane.body.heading = turned;
                    }
                    plane.way = (pos - before) / dt.max(1e-3);
                    plane.climb_toward(dt, FINALS.max(clearance));
                    (t >= 1.).then_some(Leg::Finals)
                }
                Leg::Finals => {
                    plane.body.heading = heading(inbound);
                    plane.bank *= 1. - (dt * 3.).min(1.);
                    let slow = (speed - (speed - TOUCH_DOWN) * 0.8).max(TOUCH_DOWN);
                    plane.fly(dt, slow);
                    let along = field_frame.local(plane.body.pos).x;
                    let height = FIELD_LEVEL + GLIDE * (along - RUNWAY.1 + 6.).max(0.);
                    plane.altitude = plane.altitude.min(height.max(FIELD_LEVEL));
                    if along <= RUNWAY.1 - 4. {
                        landed.push(n);
                    }
                    None
                }
            };
            plane.leg = next.unwrap_or(leg);
            if let Some(at) = drop_bomb {
                self.bombs.push(Bomb::new(at, true));
            }
            if let Some((from, toward)) = drop_torpedo {
                let id = self.warplanes[n].body.id;
                self.fire_torpedo(from, toward, id);
            }
        }
        // Those down on the runway taxi home, and those whose field is gone are lost.
        for &n in landed.iter().rev() {
            let plane = self.warplanes.remove(n);
            let serial = &mut self.serial;
            if let Some(field) = self.ports[plane.base].airfield.as_deref_mut() {
                field.land(&plane, serial);
            }
        }
    }

    /// An airfield aircraft goes down: its shelter is free for another, and it falls
    /// from where it was hit.
    pub(crate) fn down_warplane(&mut self, n: usize, events: &mut Events) {
        let plane = self.warplanes.remove(n);
        if let Some(field) = self.ports[plane.base].airfield.as_deref_mut() {
            field.lost(plane.shelter);
        }
        events.planes_downed += 1;
        let frame = match plane.kind {
            PlaneKind::Bomber => crate::Airframe::Bomber,
            PlaneKind::Torpedo => crate::Airframe::TorpedoPlane,
        };
        self.effects.push(Effect {
            body: crate::body(&mut self.serial, plane.body.pos, plane.body.heading),
            height: plane.altitude,
            kind: EffectKind::Downed(frame),
            age: 0.,
        });
    }
}
