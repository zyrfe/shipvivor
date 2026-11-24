//! A radix-2 transform over a square grid, sized once and reused every step.
//!
//! Real and imaginary parts are kept apart, and each pass combines whole rows.
//! The inner loop then runs along memory with a single twiddle, which the
//! compiler vectorises. Transposing between two such passes covers both axes.
use std::f32::consts::TAU;

pub(crate) struct Grid {
    side: usize,
    /// Where each row goes before the butterflies run.
    reversed: Vec<usize>,
    /// `e^(-i·tau·n/side)` for the first half turn: real part, then imaginary.
    twiddles: Vec<[f32; 2]>,
}

impl Grid {
    pub(crate) fn new(side: usize) -> Self {
        assert!(side.is_power_of_two());
        let bits = side.trailing_zeros();
        Self {
            side,
            reversed: (0..side)
                .map(|i| i.reverse_bits() >> (usize::BITS - bits))
                .collect(),
            twiddles: (0..side / 2)
                .map(|n| {
                    let (sin, cos) = (TAU * n as f32 / side as f32).sin_cos();
                    [cos, -sin]
                })
                .collect(),
        }
    }

    /// Transform every column.
    fn columns(&self, real: &mut [f32], imaginary: &mut [f32], inverse: bool) {
        let side = self.side;
        for (i, &j) in self.reversed.iter().enumerate() {
            if i < j {
                for part in [&mut *real, &mut *imaginary] {
                    let (head, tail) = part.split_at_mut(j * side);
                    head[i * side..][..side].swap_with_slice(&mut tail[..side]);
                }
            }
        }
        let mut half = 1;
        while half < side {
            let stride = side / (2 * half);
            let blocks = real
                .chunks_exact_mut(2 * half * side)
                .zip(imaginary.chunks_exact_mut(2 * half * side));
            for (real, imaginary) in blocks {
                let (real, real_across) = real.split_at_mut(half * side);
                let (imaginary, imaginary_across) = imaginary.split_at_mut(half * side);
                let rows = real
                    .chunks_exact_mut(side)
                    .zip(imaginary.chunks_exact_mut(side))
                    .zip(real_across.chunks_exact_mut(side))
                    .zip(imaginary_across.chunks_exact_mut(side));
                for (n, (((ar, ai), br), bi)) in rows.enumerate() {
                    let [cos, sin] = self.twiddles[n * stride];
                    let sin = if inverse { -sin } else { sin };
                    let near = ar.iter_mut().zip(ai.iter_mut());
                    let far = br.iter_mut().zip(bi.iter_mut());
                    for ((ar, ai), (br, bi)) in near.zip(far) {
                        let (tr, ti) = (*br * cos - *bi * sin, *br * sin + *bi * cos);
                        (*br, *bi) = (*ar - tr, *ai - ti);
                        (*ar, *ai) = (*ar + tr, *ai + ti);
                    }
                }
            }
            half *= 2;
        }
    }

    /// Transform both axes in place, leaving the result transposed. A second
    /// call puts it back, so a forward and an inverse transform pair up as long
    /// as whatever is done between them treats both axes alike. Neither
    /// direction is normalised.
    pub(crate) fn transform(&self, real: &mut [f32], imaginary: &mut [f32], inverse: bool) {
        let side = self.side;
        self.columns(real, imaginary, inverse);
        for part in [&mut *real, &mut *imaginary] {
            for y in 0..side {
                for x in y + 1..side {
                    part.swap(y * side + x, x * side + y);
                }
            }
        }
        self.columns(real, imaginary, inverse);
    }
}
