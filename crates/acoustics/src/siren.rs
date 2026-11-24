//! A base's siren, heard across the water.
//!
//! A siren is a fan that chops its own air. A motor spins a drum with ports cut in it
//! inside a casing with ports to match, and each time they line up a puff gets out.
//! The note is the number of ports times the turns a second, so the pitch is the speed
//! of the rotor and nothing else. This one has two rows, of ten ports and twelve, which
//! sound a minor third apart.
//!
//! The wail is the motor being switched on and off. Under power the rotor gains speed
//! steadily until the motor has no more to give. With the power off, the air it throws
//! slows it: quickly at first and ever more slowly. Those two rates, the strength of the
//! overtones and how the level follows the speed were all measured from recordings of
//! sirens winding up and running down.
//!
//! It is heard down a [`Line`]. So a siren is heard to start late, its pitch leans
//! with the ship's own way towards it or from it, and it wails on for some seconds
//! after the power to it has gone.
use crate::{Air, Alert, RATE, Rotor, filter::Rng, line::Line, voice::Ears};
use glam::Vec3;
use std::f32::consts::TAU;

/// One make of siren.
#[derive(Clone, Copy)]
struct Make {
    /// Ports in the two rows of the rotor.
    ports: [f32; 2],
    /// Turns a second at full speed.
    full: f32,
    /// Share of full speed gained each second under power.
    spin: f32,
    /// Share of its speed the rotor loses each second at full speed with the power
    /// off. That is the load of the fan, and it falls with the square of the speed.
    drag: f32,
    /// Level at full speed, close to.
    level: f32,
}
impl Rotor {
    fn make(self) -> Make {
        match self {
            // 2850 turns a minute, which sounds 475 and 570 Hz. Its rates are the
            // measured ones.
            Self::Shore => Make {
                ports: [10., 12.],
                full: 47.5,
                spin: 0.27,
                drag: 0.115,
                level: 0.6,
            },
            // 1800 turns a minute, which sounds 240 and 300 Hz. No recording of one
            // was measured: it is the shore siren with a heavier drum, which is slower
            // both to gather speed and to lose it.
            Self::Ship => Make {
                ports: [8., 10.],
                full: 30.,
                spin: 0.19,
                drag: 0.08,
                level: 0.75,
            },
        }
    }
}
/// No two sirens run at quite the same speed. This is how far one may differ.
const TRIM: f32 = 0.02;
/// Seconds the motor takes to ease onto full speed at the last.
const EASE: f32 = 0.2;
/// Share of full speed the bearings take each second whatever the speed, which is what
/// stops a rotor in the end.
const BEARINGS: f32 = 0.012;
/// Share of full speed a wrecked siren loses each second besides, with its drum
/// dragging in what is left of its casing. It stops from full speed in four seconds.
const FOULED: f32 = 0.2;
/// The wail: seconds the motor is left on once the rotor is up to speed, and seconds
/// it is then left off.
const WAIL: (f32, f32) = (1., 4.);
const UP_TO_SPEED: f32 = 0.97;
/// Strength of each tone's overtones against the tone itself.
const OVERTONES: [f32; 6] = [1., 0.63, 0.4, 0.2, 0.14, 0.16];
/// The power of the speed by which the level falls as the rotor slows.
const SWELL: f32 = 1.6;

pub(crate) struct Siren {
    position: Vec3,
    /// The alarm is up, and the motor has power.
    raised: bool,
    fouled: bool,
    powered: bool,
    /// Seconds of the present spell: at speed under power, or with the power off.
    spell: f32,
    make: Make,
    /// Speed as a share of full, how far round the rotor is in turns, and how far this
    /// siren's full speed is from its make's.
    speed: f32,
    angle: f32,
    trim: f32,
    level: f32,
    line: Line,
}
impl Siren {
    /// Sirens with different seeds are different sirens of the same make.
    pub fn new(seed: u32) -> Self {
        let mut rng = Rng::new(seed);
        Self {
            position: Vec3::ZERO,
            raised: false,
            fouled: false,
            powered: false,
            spell: 0.,
            make: Rotor::Shore.make(),
            speed: 0.,
            angle: rng.unit(),
            trim: 1. + TRIM * rng.noise(),
            level: 0.,
            line: Line::new(seed ^ 0xFADE),
        }
    }

    /// Raise or lower the alarm of a siren of this make, or wreck it, where it now
    /// stands. One aboard a ship moves with her.
    pub fn order(&mut self, rotor: Rotor, position: Vec3, alert: Alert) {
        let raised = alert == Alert::Raised;
        // One that has been silent is heard afresh, wherever it and the ears now are.
        // A rotor that is turning cannot be swapped for another.
        if self.spent() {
            self.line.renew();
            self.make = rotor.make();
        }
        if raised && !self.raised {
            (self.powered, self.spell) = (true, 0.);
        }
        (self.position, self.raised) = (position, raised);
        self.fouled = alert == Alert::Wrecked;
    }

    /// Lower the alarm where it stands.
    pub fn lower(&mut self) {
        self.raised = false;
    }

    /// Nothing is turning and nothing it made is still on its way.
    pub fn spent(&self) -> bool {
        !self.raised && self.speed == 0. && self.line.spent()
    }

    /// Stop dead, with nothing left in the air.
    pub fn clear(&mut self) {
        self.line.clear();
        (self.raised, self.fouled, self.powered, self.spell) = (false, false, false, 0.);
        (self.speed, self.level) = (0., 0.);
    }

    /// Turn the rotor through one block and add what the listener hears of it.
    pub fn sing(&mut self, ears: &Ears, air: &Air, out: &mut [[f32; 2]], send: &mut [[f32; 2]]) {
        self.turn(out.len());
        self.line.cross(self.position, ears, air, out, send);
    }

    /// Work the switch, spin the rotor and keep what it puts out.
    fn turn(&mut self, frames: usize) {
        let secs = frames as f32 / RATE;
        if !self.raised {
            self.powered = false;
        } else if self.powered {
            // The spell of power is counted from when the rotor is up to speed, so
            // the first one, from rest, is as long as it needs to be.
            if self.speed >= UP_TO_SPEED {
                self.spell += secs;
            }
            if self.spell >= WAIL.0 {
                (self.powered, self.spell) = (false, 0.);
            }
        } else {
            self.spell += secs;
            if self.spell >= WAIL.1 {
                (self.powered, self.spell) = (true, 0.);
            }
        }
        if self.speed == 0. && !self.powered {
            // At rest it makes nothing, but what it made is still crossing.
            return self.line.rest(frames);
        }
        // Both rows together at full speed come to one, as near as matters.
        let whole = OVERTONES.iter().map(|w| w * w).sum::<f32>().sqrt();
        let was = self.level;
        let (make, powered) = (self.make, self.powered);
        let friction = BEARINGS + if self.fouled { FOULED } else { 0. };
        let (speed, angle, level, trim) =
            (&mut self.speed, &mut self.angle, &mut self.level, self.trim);
        self.line.keep(frames, |n| {
            let change = if powered {
                make.spin.min((1. - *speed) / EASE)
            } else {
                -(make.drag * *speed * *speed + friction)
            };
            *speed = (*speed + change / RATE).clamp(0., 1.);
            *angle = (*angle + make.full * trim * *speed / RATE).fract();
            if n == 0 {
                *level = make.level / whole * speed.powf(SWELL);
            }
            // Each overtone is found from the two below it, which costs one sine and
            // one cosine a row.
            let mut puff = 0.;
            for ports in make.ports {
                let (sin, cos) = (TAU * ports * *angle).sin_cos();
                let (mut under, mut tone) = (0., sin);
                for weight in OVERTONES {
                    puff += weight * tone;
                    (under, tone) = (tone, 2. * cos * tone - under);
                }
            }
            (was + (*level - was) * n as f32 / frames as f32) * puff
        });
    }
}
