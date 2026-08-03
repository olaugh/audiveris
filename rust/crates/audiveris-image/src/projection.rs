// SPDX-License-Identifier: AGPL-3.0-or-later

//! Signed-short projection storage used by Java `grid.StaffProjector`.
//!
//! Staff projection counts are stored in Java `short` cells. Increments narrow
//! with two's-complement wrapping, while reads and derivatives widen to `int`.

use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::{error::Error, fmt};

use crate::staff_peak::HorizontalSide;

/// Java `StaffProjector.Blank`: an inclusive region without staff lines.
#[derive(Clone, Copy, Debug)]
pub struct ProjectionBlank {
    start: i32,
    stop: i32,
}

impl ProjectionBlank {
    #[must_use]
    pub const fn start(self) -> i32 {
        self.start
    }

    #[must_use]
    pub const fn stop(self) -> i32 {
        self.stop
    }

    /// Java's inclusive `stop - start + 1` arithmetic.
    #[must_use]
    pub const fn width(self) -> i32 {
        self.stop.wrapping_sub(self.start).wrapping_add(1)
    }

    const fn midpoint(self) -> i32 {
        self.start.wrapping_add(self.stop) / 2
    }
}

impl PartialEq for ProjectionBlank {
    fn eq(&self, other: &Self) -> bool {
        // Java Blank equality and ordering intentionally use start only.
        self.start == other.start
    }
}

impl Eq for ProjectionBlank {}

impl PartialOrd for ProjectionBlank {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ProjectionBlank {
    fn cmp(&self, other: &Self) -> Ordering {
        self.start.cmp(&other.start)
    }
}

impl Hash for ProjectionBlank {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.start.hash(state);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShortProjection {
    start: i32,
    stop: i32,
    values: Vec<i16>,
}

impl ShortProjection {
    pub fn new(start: i32, stop: i32) -> Result<Self, ProjectionError> {
        if stop < start {
            return Err(ProjectionError::InvalidDomain { start, stop });
        }
        let length = i64::from(stop) - i64::from(start) + 1;
        let length =
            usize::try_from(length).map_err(|_| ProjectionError::InvalidDomain { start, stop })?;
        Ok(Self {
            start,
            stop,
            values: vec![0; length],
        })
    }

    #[must_use]
    pub const fn start(&self) -> i32 {
        self.start
    }

    #[must_use]
    pub const fn stop(&self) -> i32 {
        self.stop
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    #[must_use]
    pub fn value(&self, position: i32) -> i32 {
        i32::from(self.values[self.index(position)])
    }

    /// Java `Projection.Short.increment(pos)`.
    pub fn increment_one(&mut self, position: i32) {
        self.increment(position, 1);
    }

    /// Java narrows the wrapped `int` sum back to a signed `short`.
    pub fn increment(&mut self, position: i32, increment: i32) {
        let index = self.index(position);
        let sum = i32::from(self.values[index]).wrapping_add(increment);
        self.values[index] = sum as i16;
    }

    /// Java returns zero at (and even below) the projection start.
    #[must_use]
    pub fn derivative(&self, position: i32) -> i32 {
        if position <= self.start {
            0
        } else {
            self.value(position) - self.value(position - 1)
        }
    }

    /// Adaptive threshold from Java `StaffProjector.computeProjection`.
    ///
    /// Derivatives are observed after `x_min`, the largest `top_count` absolute
    /// values are averaged, then scaled and rounded with Java `Math.rint`.
    pub fn staff_derivative_threshold(
        &self,
        x_min: i32,
        x_max: i32,
        top_count: usize,
        minimum_ratio: f64,
    ) -> Result<i32, ProjectionError> {
        if x_min < self.start || x_max > self.stop || x_min > x_max {
            return Err(ProjectionError::InvalidDerivativeRange { x_min, x_max });
        }
        if top_count == 0 {
            // Java computes rint(0.0 / 0 * ratio), then narrows NaN to zero.
            return Ok(0);
        }

        let mut derivatives = Vec::new();
        let mut position = x_min;
        while position < x_max {
            position += 1;
            derivatives.push(self.derivative(position).abs());
        }
        if derivatives.len() < top_count {
            return Err(ProjectionError::InsufficientDerivativeSamples {
                available: derivatives.len(),
                required: top_count,
            });
        }

        derivatives.sort_unstable();
        let cumulative = derivatives
            .iter()
            .rev()
            .take(top_count)
            .fold(0_i32, |sum, derivative| sum.wrapping_add(*derivative));
        let elite = f64::from(cumulative) / top_count as f64;
        Ok((elite * minimum_ratio).round_ties_even() as i32)
    }

    /// Java `StaffProjector.findAllBlanks` over the projection domain.
    #[must_use]
    pub fn blank_regions(&self, maximum_value: i32) -> Vec<ProjectionBlank> {
        let mut blanks = Vec::new();
        let mut active_start = None;

        for position in self.start..=self.stop {
            if self.value(position) <= maximum_value {
                active_start.get_or_insert(position);
            } else if let Some(start) = active_start.take() {
                blanks.push(ProjectionBlank {
                    start,
                    stop: position - 1,
                });
            }
        }
        if let Some(start) = active_start {
            blanks.push(ProjectionBlank {
                start,
                stop: self.stop,
            });
        }
        blanks
    }

    fn index(&self, position: i32) -> usize {
        assert!(
            (self.start..=self.stop).contains(&position),
            "projection position {position} outside {}..={}",
            self.start,
            self.stop
        );
        usize::try_from(i64::from(position) - i64::from(self.start))
            .expect("validated projection offset is nonnegative")
    }
}

/// Java `StaffProjector.selectBlank` over its start-ordered blank list.
#[must_use]
pub fn select_blank(
    blanks: &[ProjectionBlank],
    side: HorizontalSide,
    start: i32,
    minimum_width: i32,
) -> Option<ProjectionBlank> {
    let qualifies = |blank: &&ProjectionBlank| {
        let direction = match side {
            HorizontalSide::Left => -1_i32,
            HorizontalSide::Right => 1_i32,
        };
        direction.wrapping_mul(blank.midpoint().wrapping_sub(start)) > 0
            && blank.width() >= minimum_width
    };

    match side {
        HorizontalSide::Left => blanks.iter().rev().find(qualifies).copied(),
        HorizontalSide::Right => blanks.iter().find(qualifies).copied(),
    }
}

/// Java `StaffProjector.hasStandardBlank`.
#[must_use]
pub fn has_blank_between(
    blanks: &[ProjectionBlank],
    start: i32,
    stop: i32,
    minimum_width: i32,
) -> bool {
    if stop <= start {
        return false;
    }
    select_blank(blanks, HorizontalSide::Right, start, minimum_width)
        .is_some_and(|blank| blank.start <= stop)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectionError {
    InvalidDomain { start: i32, stop: i32 },
    InvalidDerivativeRange { x_min: i32, x_max: i32 },
    InsufficientDerivativeSamples { available: usize, required: usize },
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDomain { start, stop } => {
                write!(formatter, "invalid projection domain {start}..={stop}")
            }
            Self::InvalidDerivativeRange { x_min, x_max } => {
                write!(formatter, "invalid derivative range {x_min}..={x_max}")
            }
            Self::InsufficientDerivativeSamples {
                available,
                required,
            } => write!(
                formatter,
                "only {available} derivative samples available, need {required}"
            ),
        }
    }
}

impl Error for ProjectionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonzero_domain_values_and_derivatives_match_java() {
        let mut projection = ShortProjection::new(10, 13).unwrap();
        assert_eq!(projection.start(), 10);
        assert_eq!(projection.stop(), 13);
        assert_eq!(projection.len(), 4);
        assert!(!projection.is_empty());
        assert_eq!(projection.value(10), 0);

        projection.increment(10, 3);
        projection.increment_one(11);
        projection.increment(11, 4);
        projection.increment(12, -2);
        assert_eq!(projection.value(10), 3);
        assert_eq!(projection.value(11), 5);
        assert_eq!(projection.value(12), -2);
        assert_eq!(projection.derivative(10), 0);
        assert_eq!(projection.derivative(11), 2);
        assert_eq!(projection.derivative(12), -7);
        assert_eq!(projection.derivative(9), 0);
    }

    #[test]
    fn increments_narrow_with_java_signed_short_wrapping() {
        let mut projection = ShortProjection::new(0, 1).unwrap();
        projection.increment(0, i32::from(i16::MAX));
        projection.increment_one(0);
        assert_eq!(projection.value(0), i32::from(i16::MIN));

        projection.increment(1, 65_537);
        assert_eq!(projection.value(1), 1);
        projection.increment(1, i32::MAX);
        assert_eq!(projection.value(1), 0);
    }

    #[test]
    fn rejects_reversed_domain() {
        assert_eq!(
            ShortProjection::new(4, 3),
            Err(ProjectionError::InvalidDomain { start: 4, stop: 3 })
        );
    }

    #[test]
    fn staff_threshold_averages_the_largest_absolute_derivatives() {
        let mut projection = ShortProjection::new(0, 6).unwrap();
        for (position, value) in [(1, 2), (2, 7), (3, 6), (4, 14), (5, 14), (6, 11)] {
            projection.increment(position, value);
        }
        // Absolute derivatives are [2, 5, 1, 8, 0, 3]. The top five sum to
        // 19, average to 3.8, and scale to 1.9, which rint rounds to 2.
        assert_eq!(
            projection.staff_derivative_threshold(0, 6, 5, 0.5).unwrap(),
            2
        );
        // A bounded StaffProjector scan uses only derivatives inside its x
        // interval: [1, 8, 0] here, whose top two average is 4.5.
        assert_eq!(
            projection.staff_derivative_threshold(2, 5, 2, 1.0).unwrap(),
            4
        );
    }

    #[test]
    fn staff_threshold_preserves_java_ties_even_rounding() {
        let mut projection = ShortProjection::new(10, 12).unwrap();
        projection.increment(11, 5);
        // Derivatives are +5 and -5, so elite is exactly 5.
        assert_eq!(
            projection
                .staff_derivative_threshold(10, 12, 2, 0.5)
                .unwrap(),
            2
        );
        assert_eq!(
            projection
                .staff_derivative_threshold(10, 12, 2, 0.7)
                .unwrap(),
            4
        );
        assert_eq!(
            projection
                .staff_derivative_threshold(10, 12, 0, f64::NAN)
                .unwrap(),
            0
        );
    }

    #[test]
    fn staff_threshold_rejects_invalid_or_undersized_windows() {
        let projection = ShortProjection::new(10, 15).unwrap();
        assert_eq!(
            projection.staff_derivative_threshold(9, 15, 5, 0.3),
            Err(ProjectionError::InvalidDerivativeRange {
                x_min: 9,
                x_max: 15,
            })
        );
        assert_eq!(
            projection.staff_derivative_threshold(10, 16, 5, 0.3),
            Err(ProjectionError::InvalidDerivativeRange {
                x_min: 10,
                x_max: 16,
            })
        );
        assert_eq!(
            projection.staff_derivative_threshold(12, 11, 1, 0.3),
            Err(ProjectionError::InvalidDerivativeRange {
                x_min: 12,
                x_max: 11,
            })
        );
        assert_eq!(
            projection.staff_derivative_threshold(10, 12, 3, 0.3),
            Err(ProjectionError::InsufficientDerivativeSamples {
                available: 2,
                required: 3,
            })
        );
    }

    #[test]
    fn blank_regions_use_inclusive_thresholds_and_finish_at_domain_end() {
        let mut projection = ShortProjection::new(0, 9).unwrap();
        for (position, value) in [
            (0, 0),
            (1, 0),
            (2, 2),
            (3, 1),
            (4, 0),
            (5, 0),
            (6, 0),
            (7, 3),
            (8, 0),
            (9, 0),
        ] {
            projection.increment(position, value);
        }
        let blanks = projection.blank_regions(1);
        assert_eq!(
            blanks
                .iter()
                .map(|blank| (blank.start(), blank.stop(), blank.width()))
                .collect::<Vec<_>>(),
            [(0, 1, 2), (3, 6, 4), (8, 9, 2)]
        );

        let mut none = ShortProjection::new(20, 21).unwrap();
        none.increment(20, 2);
        none.increment(21, 2);
        assert!(none.blank_regions(1).is_empty());
        assert_eq!(
            ShortProjection::new(20, 21)
                .unwrap()
                .blank_regions(0)
                .iter()
                .map(|blank| (blank.start(), blank.stop()))
                .collect::<Vec<_>>(),
            [(20, 21)]
        );
    }

    #[test]
    fn blank_selection_respects_side_width_and_strict_midpoint_position() {
        let mut projection = ShortProjection::new(0, 9).unwrap();
        projection.increment(2, 2);
        projection.increment(7, 2);
        let blanks = projection.blank_regions(1);

        assert_eq!(
            select_blank(&blanks, HorizontalSide::Right, 2, 2)
                .map(|blank| (blank.start(), blank.stop())),
            Some((3, 6))
        );
        // The 3..6 midpoint is 4 and is not strictly right of start=4.
        assert_eq!(
            select_blank(&blanks, HorizontalSide::Right, 4, 2)
                .map(|blank| (blank.start(), blank.stop())),
            Some((8, 9))
        );
        assert_eq!(
            select_blank(&blanks, HorizontalSide::Left, 7, 2)
                .map(|blank| (blank.start(), blank.stop())),
            Some((3, 6))
        );
        assert_eq!(select_blank(&blanks, HorizontalSide::Right, 2, 5), None);
    }

    #[test]
    fn blank_selection_preserves_java_negative_midpoint_truncation() {
        let projection = ShortProjection::new(-13, -10).unwrap();
        let blanks = projection.blank_regions(0);
        // Java (-13 + -10) / 2 truncates -11.5 toward zero to -11.
        assert_eq!(
            select_blank(&blanks, HorizontalSide::Right, -12, 4)
                .map(|blank| (blank.start(), blank.stop())),
            Some((-13, -10))
        );
        assert_eq!(select_blank(&blanks, HorizontalSide::Left, -11, 4), None);
    }

    #[test]
    fn standard_blank_range_uses_blank_start_and_rejects_empty_ranges() {
        let mut projection = ShortProjection::new(0, 9).unwrap();
        projection.increment(2, 2);
        projection.increment(7, 2);
        let blanks = projection.blank_regions(1);

        assert!(has_blank_between(&blanks, 2, 3, 2));
        assert!(!has_blank_between(&blanks, 2, 2, 2));
        assert!(!has_blank_between(&blanks, 2, 9, 5));
        assert!(!has_blank_between(&blanks, 9, 3, 1));
    }

    #[test]
    #[should_panic(expected = "projection position 14 outside 10..=13")]
    fn value_outside_domain_matches_java_bounds_failure() {
        let projection = ShortProjection::new(10, 13).unwrap();
        let _ = projection.value(14);
    }
}
