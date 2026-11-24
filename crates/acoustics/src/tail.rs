//! What the open sea gives back after a loud sound: no walls, only a long dark roll
//! scattered off the swell and the air, arriving from everywhere at once.
//!
//! Sixteen delay lines feed one another through a matrix that shares each line equally
//! among all of them. Sound sent in on the left starts on the left and spreads.
use crate::{RATE, filter::pole};

const LINES: usize = 16;
/// Samples in each line. Primes, so that no two repeat together, at uneven spacings,
/// so that their first returns have no rhythm.
const LENGTHS: [usize; LINES] = [
    1447, 1753, 2131, 2389, 2903, 3251, 3697, 4159, 4513, 5087, 5477, 6053, 6451, 6983, 7481, 7919,
];
/// Samples in the short loops that smear a sound before it enters, for each side.
const SMEARS: [[usize; 4]; 2] = [[241, 557, 883, 1307], [263, 601, 919, 1361]];
const SMEAR: f32 = 0.6;
/// Seconds for the lows of the tail to fall by 60 dB.
const DECAY: f32 = 2.6;
/// The highs are lost a little on every pass, so they die several times sooner.
const DAMP: f32 = 2400.;
/// Nothing above or below these enters the tail at all.
const BAND: (f32, f32) = (90., 3000.);
const WET: f32 = 0.4;
/// Below this the tail is silent. After this many silent samples, longer than any
/// sound can stay unheard inside it, it is emptied and left alone.
const SILENT: f32 = 1e-7;
const PATIENCE: usize = RATE as usize;

pub(crate) struct Tail {
    /// Every line and smear loop end to end.
    store: Vec<f32>,
    lines: [Loop; LINES],
    smears: [[Loop; 4]; 2],
    keep: [f32; LINES],
    damped: [f32; LINES],
    /// Low-pass and high-pass state of the way in, for each side.
    band: [[f32; 2]; 2],
    silent: usize,
    empty: bool,
}

/// A stretch of the store read and written in a circle.
#[derive(Clone, Copy)]
struct Loop {
    start: usize,
    len: usize,
    at: usize,
}
impl Loop {
    /// The oldest sample, which the next write replaces.
    fn slot<'a>(&self, store: &'a mut [f32]) -> &'a mut f32 {
        &mut store[self.start + self.at]
    }
    fn step(&mut self) {
        self.at = (self.at + 1) % self.len;
    }
}

impl Tail {
    /// Allocates. Call it away from the audio thread.
    pub fn new() -> Self {
        let mut start = 0;
        let mut span = |len: usize| {
            let lap = Loop { start, len, at: 0 };
            start += len;
            lap
        };
        let lines = LENGTHS.map(&mut span);
        let smears = SMEARS.map(|side| side.map(&mut span));
        Self {
            store: vec![0.; start],
            lines,
            smears,
            keep: LENGTHS.map(|len| 10_f32.powf(-3. * len as f32 / (DECAY * RATE))),
            damped: [0.; LINES],
            band: [[0.; 2]; 2],
            silent: 0,
            empty: true,
        }
    }

    pub fn clear(&mut self) {
        if !self.empty {
            self.store.fill(0.);
            (self.damped, self.band, self.empty) = ([0.; LINES], [[0.; 2]; 2], true);
        }
    }

    /// Take in what the voices sent and add what comes back to the block.
    pub fn run(&mut self, send: &[[f32; 2]], out: &mut [[f32; 2]]) {
        let loud = |block: &[[f32; 2]]| block.iter().flatten().any(|x| x.abs() > SILENT);
        if self.empty && !loud(send) {
            return;
        }
        self.empty = false;
        let (damp, top, bottom) = (pole(DAMP), pole(BAND.1), pole(BAND.0));
        let mut heard = false;
        for (frame, send) in out.iter_mut().zip(send) {
            let mut taps = [0.; LINES];
            for (tap, line) in taps.iter_mut().zip(&self.lines) {
                *tap = *line.slot(&mut self.store);
            }
            // Lines alternate between the ears. The signs keep their sum from piling up.
            let mut wet = [0.; 2];
            for (i, tap) in taps.iter().enumerate() {
                wet[i % 2] += if i % 4 < 2 { *tap } else { -tap };
            }
            heard |= wet[0].abs() > SILENT || wet[1].abs() > SILENT;
            *frame = [frame[0] + WET * wet[0], frame[1] + WET * wet[1]];

            let mut entering = [0.; 2];
            for side in 0..2 {
                let band = &mut self.band[side];
                band[0] += top * (send[side] - band[0]);
                band[1] += bottom * (band[0] - band[1]);
                let mut x = band[0] - band[1];
                for smear in &mut self.smears[side] {
                    let slot = smear.slot(&mut self.store);
                    let y = *slot - SMEAR * x;
                    *slot = x + SMEAR * y;
                    smear.step();
                    x = y;
                }
                entering[side] = x;
            }
            share(&mut taps);
            for (i, line) in self.lines.iter_mut().enumerate() {
                self.damped[i] += damp * (taps[i] - self.damped[i]);
                *line.slot(&mut self.store) = entering[i % 2] + self.keep[i] * self.damped[i];
                line.step();
            }
        }
        self.silent = if heard || loud(send) {
            0
        } else {
            self.silent + out.len()
        };
        if self.silent > PATIENCE {
            self.clear();
        }
    }
}

/// Share every line equally among all of them, with signs that lose no energy.
fn share(taps: &mut [f32; LINES]) {
    let mut span = 1;
    while span < LINES {
        for pair in (0..LINES).step_by(span * 2) {
            for i in pair..pair + span {
                let (a, b) = (taps[i], taps[i + span]);
                (taps[i], taps[i + span]) = (a + b, a - b);
            }
        }
        span *= 2;
    }
    for tap in taps {
        *tap *= 0.25;
    }
}
