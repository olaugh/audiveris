// SPDX-License-Identifier: AGPL-3.0-or-later

//! The gaussian blur `SpotsBuilder` applies before closing.
//!
//! Ported from `image/GaussianGrayFilter.java`, which Audiveris derives from
//! Jerry Huxtable's filter. Two things about it are load-bearing and neither is
//! implied by "gaussian blur".
//!
//! The kernel is not the one its radius suggests. `sigma` is pinned at 1
//! regardless of radius -- there is a comment in the Java saying it was once
//! `radius / 3` -- and the radius only decides where the kernel is truncated to
//! zero. At Audiveris's radius of 1 that gives three taps.
//!
//! And every arithmetic step is `f32`, including the accumulator. A `f64`
//! accumulator would be more accurate and would not match: the taps are
//! normalised in `f32`, so they do not sum to exactly one, and the error that
//! leaves is what decides which side of a rounding boundary a pixel lands on.

/// Audiveris's `Picture.gaussianRadius` default.
pub const DEFAULT_RADIUS: f32 = 1.0;

/// `GaussianGrayFilter.makeKernel`: the separable taps, normalised.
#[must_use]
pub fn make_kernel(radius: f32) -> Vec<f32> {
    let r = radius.ceil() as i32;
    let sigma = 1.0_f32;
    let sigma_sq2 = 2.0 * sigma * sigma;
    let radius_sq = radius * radius;

    let mut matrix = Vec::with_capacity((r * 2 + 1) as usize);
    let mut total = 0.0_f32;
    for row in -r..=r {
        let distance_sq = (row * row) as f32;
        let tap = if distance_sq > radius_sq {
            0.0
        } else {
            // Java computes the exponent in `float` and only then widens it for
            // `Math.exp`, which returns a `double` narrowed straight back.
            f64::from(-distance_sq / sigma_sq2).exp() as f32
        };
        total += tap;
        matrix.push(tap);
    }

    for tap in &mut matrix {
        *tap /= total;
    }
    matrix
}

/// `GaussianGrayFilter.convolveAndTranspose`: one separable pass, transposing.
///
/// The output is `height` by `width`, so two passes return to the original
/// orientation. Samples off the end of a row repeat the edge pixel.
fn convolve_and_transpose(
    input: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    matrix: &[f32],
) {
    let cols2 = (matrix.len() / 2) as i32;

    for y in 0..height {
        let mut index = y;
        let row_start = y * width;

        for x in 0..width {
            let mut p = 0.0_f32;

            for col in -cols2..=cols2 {
                let f = matrix[(cols2 + col) as usize];
                // Java skips a zero tap rather than adding zero. With an f32
                // accumulator that is not the same statement, so it is kept.
                if f != 0.0 {
                    let ix = (x as i32 + col).clamp(0, width as i32 - 1) as usize;
                    p += f * f32::from(input[row_start + ix]);
                }
            }

            // `(int) (p + 0.5)` in Java: the float widens, the half is a
            // double, and the narrowing truncates toward zero.
            output[index] = (f64::from(p) + 0.5).clamp(0.0, 255.0) as u8;
            index += height;
        }
    }
}

/// Blurs a gray buffer, as `Picture.gaussianFiltered`.
///
/// # Panics
///
/// Panics if `input` is not exactly `width * height` bytes.
#[must_use]
pub fn gaussian_gray(width: usize, height: usize, input: &[u8], radius: f32) -> Vec<u8> {
    assert_eq!(input.len(), width * height, "buffer is not width * height");
    let matrix = make_kernel(radius);

    let mut transposed = vec![0_u8; input.len()];
    convolve_and_transpose(input, &mut transposed, width, height, &matrix);

    let mut output = vec![0_u8; input.len()];
    convolve_and_transpose(&transposed, &mut output, height, width, &matrix);

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_the_three_tap_kernel_java_builds() {
        let matrix = make_kernel(DEFAULT_RADIUS);
        assert_eq!(matrix.len(), 3);
        assert_eq!(matrix[0], matrix[2]);
        // Normalised in f32, so the sum is near one rather than one -- which is
        // the whole reason the accumulator below has to be f32 too.
        assert!((matrix.iter().sum::<f32>() - 1.0).abs() < 1e-6);

        // The shape, from Java's own arithmetic: exp(-0.5) either side of 1.
        let raw = (-0.5_f64).exp() as f32;
        let total = 2.0 * raw + 1.0;
        assert_eq!(matrix, [raw / total, 1.0 / total, raw / total]);
    }

    #[test]
    fn leaves_a_flat_buffer_alone() {
        for level in [0_u8, 1, 128, 254, 255] {
            let buffer = vec![level; 32 * 20];
            assert_eq!(
                gaussian_gray(32, 20, &buffer, DEFAULT_RADIUS),
                vec![level; 32 * 20],
                "level {level}"
            );
        }
    }

    #[test]
    fn edges_repeat_rather_than_darken() {
        // A white buffer with one dark column at the left edge. If the edge
        // clamp were wrong -- padding with zero, or wrapping -- the corner
        // pixels would move differently from the middle of the column.
        let (width, height) = (8, 6);
        let mut buffer = vec![255_u8; width * height];
        for y in 0..height {
            buffer[y * width] = 0;
        }
        let blurred = gaussian_gray(width, height, &buffer, DEFAULT_RADIUS);
        let column: Vec<u8> = (0..height).map(|y| blurred[y * width]).collect();
        assert!(column.iter().all(|&pixel| pixel == column[0]), "{column:?}");
    }
}
