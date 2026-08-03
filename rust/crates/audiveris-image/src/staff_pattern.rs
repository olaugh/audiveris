// SPDX-License-Identifier: AGPL-3.0-or-later

//! Pixel-domain fit score for an idealized set of parallel staff lines.

/// Source-compatible neutral form of Java `StaffPattern`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaffPattern {
    count: usize,
    width: usize,
    line: usize,
    interline: f64,
}

impl StaffPattern {
    #[must_use]
    pub const fn new(count: usize, width: usize, line: usize, interline: f64) -> Self {
        Self {
            count,
            width,
            line,
            interline,
        }
    }

    /// Fraction of pattern trials landing on zero-valued foreground pixels.
    ///
    /// Trials outside the image count in the denominator but cannot match,
    /// exactly as in Java. The slice must contain `buffer_width * buffer_height`
    /// row-major samples.
    #[must_use]
    pub fn evaluate(
        self,
        location: (f64, f64),
        buffer_width: usize,
        buffer_height: usize,
        pixels: &[u8],
    ) -> f64 {
        assert_eq!(
            pixels.len(),
            buffer_width.saturating_mul(buffer_height),
            "staff-pattern buffer dimensions must match its pixels"
        );
        let x_min = location.0.round_ties_even() as isize;
        let mut trials = 0_usize;
        let mut matches = 0_usize;

        for index in 0..self.count {
            let y_mid = location.1 + (index as f64 * self.interline);
            let y_min = (y_mid - (self.line as f64 / 2.0)).ceil() as isize;
            let y_max = (y_mid + (self.line as f64 / 2.0)).floor() as isize;

            for y in y_min..=y_max {
                for x_offset in 0..self.width {
                    let x = x_min + x_offset as isize;
                    trials += 1;
                    if x >= 0
                        && x < buffer_width as isize
                        && y >= 0
                        && y < buffer_height as isize
                        && pixels[y as usize * buffer_width + x as usize] == 0
                    {
                        matches += 1;
                    }
                }
            }
        }

        matches as f64 / trials as f64
    }
}

/// Vertical probe offsets used by Java `LinesRetriever.retrieveEndPoints`.
/// Even jitter values cover `[-jitter/2, +jitter/2]`; odd values cover the
/// symmetric integer range through `(jitter + 1) / 2`, always in
/// `0,+1,-1,+2,-2,...` order.
#[must_use]
pub fn ending_pattern_offsets(pattern_jitter: i32) -> Vec<i32> {
    let radius = (pattern_jitter + 1) / 2;
    let iter_max = 1 + (2 * radius);
    let mut offsets = Vec::with_capacity(iter_max.max(0) as usize);
    let mut dy: i32 = 0;
    for iter in 1..=iter_max {
        offsets.push(dy);
        if dy == 0 {
            dy = 1;
        } else {
            dy += (-dy).signum() * iter;
        }
    }
    offsets
}

/// Select the first strictly best endpoint-pattern offset, matching Java's
/// `ratio > bestRatio` update and initial `(bestDy, bestRatio) = (0, 0)`.
#[must_use]
pub fn best_ending_pattern_offset(
    pattern_jitter: i32,
    mut evaluate: impl FnMut(i32) -> f64,
) -> (i32, f64) {
    let mut best_offset = 0;
    let mut best_ratio = 0.0;
    for offset in ending_pattern_offsets(pattern_jitter) {
        let ratio = evaluate(offset);
        if ratio > best_ratio {
            best_ratio = ratio;
            best_offset = offset;
        }
    }
    (best_offset, best_ratio)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_fractional_interlines_and_zero_foreground() {
        let mut pixels = vec![255; 8 * 10];
        for (x, y) in [(2, 1), (3, 1), (4, 1), (2, 4), (3, 4), (2, 8)] {
            pixels[y * 8 + x] = 0;
        }
        let pattern = StaffPattern::new(3, 3, 1, 3.5);

        // Rows 1, 4.5 -> 4..5, and 8 are sampled: 12 trials, 6 matches.
        assert_eq!(pattern.evaluate((2.0, 1.0), 8, 10, &pixels), 0.5);
    }

    #[test]
    fn preserves_ties_even_x_and_out_of_bounds_penalty() {
        let pixels = [0, 255, 255, 255];
        let pattern = StaffPattern::new(1, 2, 1, 4.0);

        // 0.5 rounds to x=0; one of two trials is foreground.
        assert_eq!(pattern.evaluate((0.5, 0.0), 4, 1, &pixels), 0.5);
        // x=-1 keeps two trials but only x=0 can match.
        assert_eq!(pattern.evaluate((-1.0, 0.0), 4, 1, &pixels), 0.5);
    }

    #[test]
    fn endpoint_pattern_offsets_match_java_alternating_jitter_order() {
        assert_eq!(ending_pattern_offsets(4), [0, 1, -1, 2, -2]);
        assert_eq!(ending_pattern_offsets(5), [0, 1, -1, 2, -2, 3, -3]);
        assert_eq!(ending_pattern_offsets(0), [0]);
    }

    #[test]
    fn endpoint_pattern_selection_keeps_first_strict_best_and_zero_floor() {
        let (offset, ratio) = best_ending_pattern_offset(4, |dy| match dy {
            1 | -1 => 0.75,
            _ => 0.5,
        });
        assert_eq!((offset, ratio), (1, 0.75));

        assert_eq!(best_ending_pattern_offset(2, |_| f64::NAN), (0, 0.0));
    }

    #[test]
    fn even_line_thickness_samples_java_inclusive_span() {
        let pixels = [0; 3 * 3];
        let pattern = StaffPattern::new(1, 1, 2, 4.0);

        // Java samples yMid-line/2 through yMid+line/2 inclusively: 3 rows.
        assert_eq!(pattern.evaluate((1.0, 1.0), 3, 3, &pixels), 1.0);
    }
}
