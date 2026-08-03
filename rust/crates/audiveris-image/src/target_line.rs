// SPDX-License-Identifier: AGPL-3.0-or-later

//! Headless destination-to-source mapping from Java `grid.TargetLine`.
//!
//! Page, system, and staff ownership remain with the caller. This slice keeps
//! only the numerical mapping needed to deskew one physical staff line.

use std::{error::Error, fmt};

use crate::filament::{FilamentError, FilamentGeometry};

/// Immutable mapping between one ideal target line and its physical source.
#[derive(Clone, Debug)]
pub struct TargetLine {
    source: FilamentGeometry,
    target_y: f64,
    target_left: f64,
    target_right: f64,
    sin: f64,
    cos: f64,
}

impl TargetLine {
    pub fn new(
        source: FilamentGeometry,
        target_y: f64,
        target_left: f64,
        target_right: f64,
    ) -> Result<Self, TargetLineError> {
        if !target_y.is_finite()
            || !target_left.is_finite()
            || !target_right.is_finite()
            || target_left == target_right
        {
            return Err(TargetLineError::InvalidTarget);
        }
        let start = source.start();
        let stop = source.stop();
        let delta_x = stop.0 - start.0;
        let delta_y = stop.1 - start.1;
        let hypot = delta_x.hypot(delta_y);
        if !hypot.is_finite() || hypot == 0.0 {
            return Err(TargetLineError::DegenerateSource);
        }
        Ok(Self {
            source,
            target_y,
            target_left,
            target_right,
            sin: delta_y / hypot,
            cos: delta_x / hypot,
        })
    }

    #[must_use]
    pub const fn target_y(&self) -> f64 {
        self.target_y
    }

    /// Source point corresponding to an abscissa on the ideal target line.
    pub fn source_of_x(&self, target_x: f64) -> Result<(f64, f64), TargetLineError> {
        let ratio = (target_x - self.target_left) / (self.target_right - self.target_left);
        let start = self.source.start();
        let stop = self.source.stop();
        let source_x = ((1.0 - ratio) * start.0) + (ratio * stop.0);
        Ok((source_x, self.source.position_at(source_x)?))
    }

    /// Source point corresponding to a target point above or below the line.
    pub fn source_of_point(&self, target: (f64, f64)) -> Result<(f64, f64), TargetLineError> {
        let distance = target.1 - self.target_y;
        let projected = self.source_of_x(target.0)?;
        Ok((
            projected.0 - (distance * self.sin),
            projected.1 + (distance * self.cos),
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TargetLineError {
    Filament(FilamentError),
    InvalidTarget,
    DegenerateSource,
}

impl From<FilamentError> for TargetLineError {
    fn from(value: FilamentError) -> Self {
        Self::Filament(value)
    }
}

impl fmt::Display for TargetLineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Filament(error) => write!(formatter, "target-line source error: {error}"),
            Self::InvalidTarget => formatter.write_str("target-line destination span is invalid"),
            Self::DegenerateSource => formatter.write_str("target-line source is degenerate"),
        }
    }
}

impl Error for TargetLineError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        filament::StaffFilament,
        run_table::{Orientation, Run, RunTable},
        section::{JunctionPolicy, build_sections},
    };

    fn horizontal_geometry() -> FilamentGeometry {
        let mut table = RunTable::new(Orientation::Horizontal, 40, 5).unwrap();
        table.add_run(2, Run::new(0, 40)).unwrap();
        let mut filament = StaffFilament::new(10).unwrap();
        filament
            .add_section(build_sections(&table, JunctionPolicy::All).remove(0))
            .unwrap();
        filament.geometry().unwrap()
    }

    #[test]
    fn maps_target_abscissa_across_source_endpoints() {
        let target = TargetLine::new(horizontal_geometry(), 50.0, 100.0, 200.0).unwrap();

        assert_eq!(target.target_y(), 50.0);
        assert_eq!(target.source_of_x(100.0).unwrap(), (0.0, 2.0));
        assert_eq!(target.source_of_x(200.0).unwrap(), (39.0, 2.0));
        assert_eq!(target.source_of_x(150.0).unwrap(), (19.5, 2.0));
    }

    #[test]
    fn offsets_orthogonally_from_the_target_line() {
        let target = TargetLine::new(horizontal_geometry(), 50.0, 100.0, 200.0).unwrap();

        assert_eq!(target.source_of_point((150.0, 60.0)).unwrap(), (19.5, 12.0));
        assert_eq!(target.source_of_point((150.0, 40.0)).unwrap(), (19.5, -8.0));
    }

    #[test]
    fn rejects_non_finite_or_zero_width_target_spans() {
        for (y, left, right) in [
            (f64::NAN, 0.0, 1.0),
            (0.0, f64::INFINITY, 1.0),
            (0.0, 1.0, 1.0),
        ] {
            assert_eq!(
                TargetLine::new(horizontal_geometry(), y, left, right).unwrap_err(),
                TargetLineError::InvalidTarget
            );
        }
    }
}
