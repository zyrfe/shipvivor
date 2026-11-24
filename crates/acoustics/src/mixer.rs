//! Voices, the listener and the master output.
use crate::{
    Air, Cue, RATE, Sound,
    filter::pole,
    flight::{CLOSE, Flight, Passing},
    foundry::{Bank, VARIATIONS},
    hull::Hull,
    siren::Siren,
    tail::Tail,
    voice::{Ears, Heard, Voice},
};
use glam::Vec3;
use std::sync::{
    Arc,
    mpsc::{Receiver, SyncSender, sync_channel},
};

/// Frames rendered between looks at the cue queue and the listener.
const BLOCK: usize = 256;
const VOICES: usize = 64;
const FLIGHTS: usize = 32;
const SHORES: usize = 8;
const SIRENS: usize = 4;
const QUEUE: usize = 1024;
/// Seconds to close or open the output when world time holds or resumes.
const GATE: f32 = 0.008;
const MASTER: f32 = 0.7;
/// How hard a loud sound pushes the ship's own noise down, and the seconds it takes
/// to come back. A gun going off should own the moment.
const DUCK: f32 = 2.5;
const RECOVERY: f32 = 0.45;
/// Level the whole output is turned down to when it stays loud, and the seconds it
/// takes to come back up.
const CEILING: f32 = 0.8;
const RELEASE: f32 = 0.2;
/// A sound fainter than this in every layer is not worth a voice.
const FAINT: f32 = 1e-4;
/// Share of a report that land throws back, the metres of path past which it is not
/// worth a voice, and how close to land a sound must be made to be part of it.
const ECHO: f32 = 1.;
const ECHO_REACH: f32 = 900.;
/// Metres across the middle of land that throws back that whole share. Smaller land
/// throws back less in proportion, and the largest no more than this many times it.
const BLUFF: (f32, f32) = (26., 2.);
const ASHORE: f32 = 20.;
/// Level at which a blast starts to deafen, and the level at which it does so fully.
const DEAFENING: (f32, f32) = (0.8, 1.6);
/// Seconds for the ears to close against a blast and to open again. The crack is past
/// before they close.
const FLINCH: (f32, f32) = (0.03, 0.45);
/// What deafened ears still hear: this share of the mix is cut to below this frequency.
const DEAFENED: (f32, f32) = (0.55, 2500.);

/// The game's end of the cue queue.
pub struct Cues(SyncSender<Cue>);
impl Cues {
    /// Queue a cue for the mixer. A full queue means nobody is listening, so the cue is dropped.
    pub fn send(&self, cue: Cue) {
        let _ = self.0.try_send(cue);
    }
}

/// A cue queue and the mixer that answers it.
pub fn console(bank: Arc<Bank>, air: Air) -> (Cues, Mixer) {
    let (send, cues) = sync_channel(QUEUE);
    let mixer = Mixer {
        cues,
        bank,
        air,
        voices: Vec::with_capacity(VOICES),
        flights: Vec::with_capacity(FLIGHTS),
        shores: [None; SHORES],
        sirens: [0x51E, 0xA1A, 0x3A9, 0x70C].map(Siren::new),
        ears: Ears::default(),
        hull: Hull::new(),
        tail: Tail::new(),
        bed: [[0.; 2]; BLOCK],
        aside: [[0.; 2]; BLOCK],
        send: [[0.; 2]; BLOCK],
        deaf: 0.,
        muffle: [[0.; 2]; 2],
        duck: Rider::OPEN,
        trim: Rider::OPEN,
        flowing: true,
        gate: 1.,
    };
    (Cues(send), mixer)
}

pub struct Mixer {
    cues: Receiver<Cue>,
    bank: Arc<Bank>,
    air: Air,
    voices: Vec<Voice>,
    flights: Vec<Flight>,
    /// Land that answers a report: where it stands and how wide it is.
    shores: [Option<(Vec3, f32)>; SHORES],
    sirens: [Siren; SIRENS],
    ears: Ears,
    hull: Hull,
    tail: Tail,
    bed: [[f32; 2]; BLOCK],
    /// Sirens, which are kept out of the reckoning of how loud a moment is.
    aside: [[f32; 2]; BLOCK],
    send: [[f32; 2]; BLOCK],
    /// How far the ears have closed against a blast, from 0 to 1.
    deaf: f32,
    muffle: [[f32; 2]; 2],
    duck: Rider,
    trim: Rider,
    flowing: bool,
    gate: f32,
}
impl Mixer {
    /// Render the next stereo frames.
    pub fn render(&mut self, out: &mut [[f32; 2]]) {
        for block in out.chunks_mut(BLOCK) {
            self.block(block);
        }
    }

    /// Sounds in the air, counting those yet to reach the listener.
    pub fn sounding(&self) -> usize {
        self.voices.len()
    }

    /// Sounds crossing the air whose front has reached the listener.
    pub fn audible(&self) -> usize {
        let arrived = |v: &&Voice| v.airborne() && v.spent(&self.air) >= 0.;
        self.voices.iter().filter(arrived).count()
    }

    /// Voices making one kind of sound, and how many of those have reached the
    /// listener. Echoes count with the sound they are echoes of.
    pub fn making(&self, sound: Sound) -> (usize, usize) {
        let made = |v: &&Voice| v.cast() / VARIATIONS == sound as usize;
        let arrived = |v: &&Voice| v.spent(&self.air) >= 0.;
        let voices = self.voices.iter().filter(made);
        (voices.clone().count(), voices.filter(arrived).count())
    }

    /// Shells whose flight is being followed, counting those that have landed with
    /// sound still on its way.
    pub fn flying(&self) -> usize {
        self.flights.len()
    }

    /// Sirens that are turning, or have stopped with their last sound still on its way.
    pub fn wailing(&self) -> usize {
        self.sirens.iter().filter(|siren| !siren.spent()).count()
    }

    /// An endless stream of interleaved stereo samples.
    pub fn feed(self) -> Feed {
        Feed {
            mixer: self,
            block: [[0.; 2]; BLOCK],
            at: 2 * BLOCK,
        }
    }

    fn block(&mut self, out: &mut [[f32; 2]]) {
        while let Ok(cue) = self.cues.try_recv() {
            self.answer(cue);
        }
        out.fill([0.; 2]);
        if !self.flowing && self.gate == 0. {
            return;
        }
        self.ears.settle(out.len());
        let (air, ears) = (&self.air, &self.ears);
        self.send[..out.len()].fill([0.; 2]);
        let send = &mut self.send[..out.len()];
        let mut passings = [const { None }; 4];
        self.flights.retain_mut(|flight| {
            let (flying, passing) = flight.sing(ears, air, out, send);
            if let (Some(_), Some(free)) = (&passing, passings.iter_mut().find(|p| p.is_none())) {
                *free = passing;
            }
            flying
        });
        for passing in passings.into_iter().flatten() {
            self.crack(passing);
        }
        let (bank, air, ears) = (&self.bank, &self.air, &self.ears);
        let send = &mut self.send[..out.len()];
        self.voices.retain_mut(|voice| {
            let heard = voice.hearing(ears, air);
            voice.sing(bank.cast(voice.cast()), heard, air, out, send)
        });
        // A siren goes on and on. Were it weighed with the guns it would hold the
        // ship's own noise down for as long as it wailed.
        let aside = &mut self.aside[..out.len()];
        aside.fill([0.; 2]);
        for siren in self.sirens.iter_mut().filter(|siren| !siren.spent()) {
            siren.sing(ears, air, aside, send);
        }
        self.tail.run(send, out);
        // The ship's own noise gives way to anything loud and creeps back afterwards.
        let frames = out.len() as f32;
        let (was, now) = self
            .duck
            .ride(out, RECOVERY, |loud| 1. / (1. + DUCK * loud));
        let bed = &mut self.bed[..out.len()];
        self.hull.sing(bed);
        for (n, (frame, (bed, aside))) in out.iter_mut().zip(bed.iter().zip(aside)).enumerate() {
            let room = was + (now - was) * n as f32 / frames;
            *frame = [0, 1].map(|ear| frame[ear] + room * bed[ear] + aside[ear]);
        }
        self.flinch(out);
        // Loudness that lasts is turned down as a whole, which keeps a salvo clean.
        // Only the first instant of a blast is left for the limiter to bend.
        let (was, now) = self
            .trim
            .ride(out, RELEASE, |loud| (CEILING / (MASTER * loud)).min(1.));
        let (open, step) = (f32::from(self.flowing), 1. / (GATE * RATE));
        for (n, frame) in out.iter_mut().enumerate() {
            self.gate += (open - self.gate).clamp(-step, step);
            let level = self.gate * MASTER * (was + (now - was) * n as f32 / frames);
            *frame = frame.map(|x| limit(x * level));
        }
    }

    fn answer(&mut self, cue: Cue) {
        match cue {
            Cue::Ears { position, right } => self.ears.turn(position, right),
            Cue::Emit {
                sound,
                position,
                serial,
            } => self.emit(sound, position, serial),
            Cue::Shell {
                serial,
                position,
                velocity,
                weight,
            } => {
                if let Some(flight) = self.flights.iter_mut().find(|f| f.serial == serial) {
                    flight.track(position, velocity);
                } else if self.flights.len() < FLIGHTS {
                    self.flights
                        .push(Flight::new(serial, position, velocity, weight));
                }
            }
            Cue::Shore {
                slot,
                position,
                radius,
            } => {
                if let Some(shore) = self.shores.get_mut(slot) {
                    *shore = (radius > 0.).then_some((position, radius));
                }
            }
            Cue::Alarm {
                slot,
                rotor,
                position,
                raised,
            } => {
                if let Some(siren) = self.sirens.get_mut(slot) {
                    siren.order(rotor, position, raised);
                }
            }
            Cue::Way { speed, throttle } => self.hull.way(speed, throttle),
            Cue::Engines { damaged, stopped } => self.hull.plant(damaged, stopped),
            Cue::Moored => {
                self.hull.moor();
                // Nobody is left to work the switch, and each siren runs down.
                self.sirens.iter_mut().for_each(Siren::lower);
            }
            Cue::Flow(flowing) => self.flowing = flowing,
            Cue::Hush => {
                self.voices.clear();
                self.flights.clear();
                self.sirens.iter_mut().for_each(Siren::clear);
                self.tail.clear();
                (self.deaf, self.muffle) = (0., [[0.; 2]; 2]);
                self.ears.placed = false;
            }
        }
    }

    /// After a blast the ears close for a moment: everything goes dull and comes back.
    fn flinch(&mut self, out: &mut [[f32; 2]]) {
        let frames = out.len() as f32;
        let blast = ((self.duck.peak - DEAFENING.0) / (DEAFENING.1 - DEAFENING.0)).clamp(0., 1.);
        let secs = if blast > self.deaf {
            FLINCH.0
        } else {
            FLINCH.1
        };
        let was = self.deaf;
        self.deaf += (blast - self.deaf) * (1. - (-frames / (secs * RATE)).exp());
        let dull = pole(DEAFENED.1);
        for (n, frame) in out.iter_mut().enumerate() {
            let deaf = DEAFENED.0 * (was + (self.deaf - was) * n as f32 / frames);
            for ear in 0..2 {
                let lows = &mut self.muffle[ear];
                lows[0] += dull * (frame[ear] - lows[0]);
                lows[1] += dull * (lows[0] - lows[1]);
                frame[ear] += deaf * (lows[1] - frame[ear]);
            }
        }
    }

    fn emit(&mut self, sound: Sound, position: Vec3, serial: u64) {
        let levels = sound.levels();
        // A small spread of playback speed keeps repeated takes from sounding identical.
        let detune = (serial.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 40) as f64 / (1 << 24) as f64;
        let speed = if sound.tuned() {
            1.
        } else {
            0.96 + 0.08 * detune
        };
        let cast = Bank::index(sound, serial);
        let audible = |gains: [f32; 3]| (0..3).any(|layer| levels[layer] * gains[layer] >= FAINT);
        if sound.aboard() {
            let voice = Voice::new(cast, levels, position, speed, Heard::ABOARD);
            return self.admit(voice.aboard());
        }
        let heard = self.ears.hear(position, 0., &self.air);
        if audible(heard.gains) {
            self.admit(Voice::new(cast, levels, position, speed, heard));
        }
        if !sound.echoes() {
            return;
        }
        // Land throws back the weight of a report and none of its crack. An echo never
        // takes a voice from a sound still playing.
        for (shore, radius) in self.shores.into_iter().flatten() {
            let share = ECHO * (radius / BLUFF.0).min(BLUFF.1);
            let thrown = [0., share * levels[1], share * levels[2]];
            // It comes back off the shore that faces the listener, which on a big
            // island is a long way short of the middle.
            let inland = (self.ears.position - shore).with_y(0.).normalize_or_zero();
            let face = shore + inland * radius;
            let lead = position.distance(face);
            let heard = self.ears.hear(face, lead, &self.air);
            let loud = (1..3).any(|layer| thrown[layer] * heard.gains[layer] >= FAINT);
            let off = position.distance(shore) - radius > ASHORE;
            if off && loud && heard.distance < ECHO_REACH && self.voices.len() < VOICES {
                let cast = Bank::index(sound, serial.wrapping_add(1));
                self.voices
                    .push(Voice::new(cast, thrown, face, speed, heard).echo(lead));
            }
        }
    }

    /// The shock of a shell passing, sounded where the shell was when it made it.
    fn crack(&mut self, passing: Passing) {
        let heard = self.ears.hear(passing.position, 0., &self.air);
        // Like the shell's rush it is for those close under the path, more than for
        // everyone in earshot of the gun.
        let near = (2. * CLOSE / (CLOSE + heard.distance)).min(1.) * passing.weight.min(1.5);
        let levels = Sound::Crack.levels().map(|level| level * near);
        if (0..3).any(|layer| levels[layer] * heard.gains[layer] >= FAINT) {
            let cast = Bank::index(Sound::Crack, passing.serial);
            let voice = Voice::new(cast, levels, passing.position, 1., heard);
            self.admit(voice.arriving(&self.air));
        }
    }

    /// Give a new sound a voice. A full pool drops the voice that has been audible longest.
    fn admit(&mut self, voice: Voice) {
        if self.voices.len() == VOICES {
            let air = &self.air;
            let longest = (0..VOICES)
                .max_by(|&a, &b| {
                    let (a, b) = (self.voices[a].spent(air), self.voices[b].spent(air));
                    a.total_cmp(&b)
                })
                .expect("a full pool has voices");
            self.voices.swap_remove(longest);
        }
        self.voices.push(voice);
    }
}

/// Follows the peaks of the mix, up at once and down slowly, and turns them into a gain.
#[derive(Clone, Copy)]
struct Rider {
    peak: f32,
    gain: f32,
}
impl Rider {
    const OPEN: Self = Self { peak: 0., gain: 1. };

    /// Take in a block and return the gains to glide between across it.
    fn ride(&mut self, block: &[[f32; 2]], recovery: f32, gain: impl Fn(f32) -> f32) -> (f32, f32) {
        let peak = block
            .iter()
            .flatten()
            .fold(0_f32, |peak, x| peak.max(x.abs()));
        self.peak = peak.max(self.peak * (-(block.len() as f32) / (recovery * RATE)).exp());
        let was = self.gain;
        self.gain = gain(self.peak);
        (was, self.gain)
    }
}

/// Bend peaks towards full scale without passing it.
fn limit(x: f32) -> f32 {
    const KNEE: f32 = 0.6;
    if x.abs() <= KNEE {
        x
    } else {
        x.signum() * (KNEE + (1. - KNEE) * ((x.abs() - KNEE) / (1. - KNEE)).tanh())
    }
}

/// The mixer as a stream of samples: left, right, left, right.
pub struct Feed {
    mixer: Mixer,
    block: [[f32; 2]; BLOCK],
    at: usize,
}
impl Iterator for Feed {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.at == 2 * BLOCK {
            self.mixer.render(&mut self.block);
            self.at = 0;
        }
        let sample = self.block[self.at / 2][self.at % 2];
        self.at += 1;
        Some(sample)
    }
}
