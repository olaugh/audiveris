// SPDX-License-Identifier: AGPL-3.0-or-later

//! Signed-short projection storage used by Java `grid.StaffProjector`.
//!
//! Staff projection counts are stored in Java `short` cells. Increments narrow
//! with two's-complement wrapping, while reads and derivatives widen to `int`.

use std::{error::Error, fmt};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectionError {
    InvalidDomain { start: i32, stop: i32 },
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDomain { start, stop } => {
                write!(formatter, "invalid projection domain {start}..={stop}")
            }
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
    #[should_panic(expected = "projection position 14 outside 10..=13")]
    fn value_outside_domain_matches_java_bounds_failure() {
        let projection = ShortProjection::new(10, 13).unwrap();
        let _ = projection.value(14);
    }
}
