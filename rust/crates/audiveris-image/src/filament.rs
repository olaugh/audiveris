// SPDX-License-Identifier: AGPL-3.0-or-later

//! Neutral, headless staff-filament geometry for the `GRID` stage.
//!
//! This is the dependency-light numerical surface of Java `SectionCompound`,
//! `Filament`, `CurvedFilament`, and `StaffFilament`. It deliberately omits
//! `FilamentFactory` grouping, combs/clusters, hole filling, glyph ownership,
//! curvature polishing, vertical splines, persistence, and all UI behavior.

use std::error::Error;
use std::fmt;

use audiveris_core::natural_spline::{NaturalSpline, SplineError};

use crate::run_table::Orientation;
use crate::section::{Bounds, Section};

/// Cached horizontal guide geometry through a compound of sections.
#[derive(Clone, Debug, PartialEq)]
pub struct FilamentGeometry {
    start: (f64, f64),
    stop: (f64, f64),
    points: Vec<(f64, f64)>,
    spline: NaturalSpline,
}

impl FilamentGeometry {
    #[must_use]
    pub const fn start(&self) -> (f64, f64) {
        self.start
    }

    #[must_use]
    pub const fn stop(&self) -> (f64, f64) {
        self.stop
    }

    #[must_use]
    pub fn points(&self) -> &[(f64, f64)] {
        &self.points
    }

    /// Java-compatible ordinate, with endpoint-line extrapolation.
    pub fn position_at(&self, x: f64) -> Result<f64, FilamentError> {
        if x < self.start.0 || x > self.stop.0 {
            let slope = (self.stop.1 - self.start.1) / (self.stop.0 - self.start.0);
            Ok(self.start.1 + (slope * (x - self.start.0)))
        } else {
            Ok(self.spline.y_at_x(x)?)
        }
    }

    pub fn slope_at(&self, x: f64) -> Result<f64, FilamentError> {
        Ok(self.spline.y_derivative_at_x(x)?)
    }

    #[must_use]
    pub fn is_within_range(&self, x: f64) -> bool {
        x >= self.start.0 && x <= self.stop.0
    }
}

/// Horizontal staff-line filament assembled from already constructed sections.
#[derive(Clone, Debug)]
pub struct StaffFilament {
    interline: usize,
    sections: Vec<Section>,
}

impl StaffFilament {
    pub fn new(interline: usize) -> Result<Self, FilamentError> {
        if interline == 0 {
            return Err(FilamentError::InvalidInterline);
        }
        Ok(Self {
            interline,
            sections: Vec::new(),
        })
    }

    pub fn add_section(&mut self, section: Section) -> Result<(), FilamentError> {
        if section.orientation() != Orientation::Horizontal {
            return Err(FilamentError::UnsupportedOrientation);
        }
        self.sections.push(section);
        // Java's TreeSet uses Section.byFullAbscissa. The stable ID is the
        // final tie-breaker when geometry is otherwise identical.
        self.sections
            .sort_by_key(|section| (section.bounds().x, section.bounds().y, section.id()));
        Ok(())
    }

    #[must_use]
    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    pub fn bounds(&self) -> Result<Bounds, FilamentError> {
        let first = self.sections.first().ok_or(FilamentError::Empty)?;
        let mut bounds = first.bounds();
        let mut max_x = bounds.x + bounds.width - 1;
        let mut max_y = bounds.y + bounds.height - 1;
        for section in &self.sections[1..] {
            let current = section.bounds();
            let min_x = bounds.x.min(current.x);
            let min_y = bounds.y.min(current.y);
            max_x = max_x.max(current.x + current.width - 1);
            max_y = max_y.max(current.y + current.height - 1);
            bounds.x = min_x;
            bounds.y = min_y;
        }
        bounds.width = max_x - bounds.x + 1;
        bounds.height = max_y - bounds.y + 1;
        Ok(bounds)
    }

    #[must_use]
    pub fn weight(&self) -> usize {
        self.sections.iter().map(Section::weight).sum()
    }

    /// Java `Filament.getTrueLength`, including its current hole arithmetic.
    pub fn true_length(&self) -> Result<usize, FilamentError> {
        let first = self.sections.first().ok_or(FilamentError::Empty)?;
        let coordinate_min = first.start_coord();
        let mut coordinate_max = None;
        let mut holes = 0;
        for section in &self.sections {
            let start = section.start_coord();
            let stop = section.stop_coord();
            if let Some(previous_max) = coordinate_max
                && start > previous_max
            {
                holes += start - previous_max;
            }
            coordinate_max = Some(coordinate_max.map_or(stop, |value: usize| value.max(stop)));
        }
        Ok((coordinate_max.expect("non-empty filament") - coordinate_min + 1) - holes)
    }

    pub fn thickness(&self) -> Result<f64, FilamentError> {
        Ok(self.weight() as f64 / self.true_length()? as f64)
    }

    /// Compute the source-compatible probe centroids and natural spline.
    pub fn geometry(&self) -> Result<FilamentGeometry, FilamentError> {
        let bounds = self.bounds()?;
        let probe_width = ((self.interline as f64) * 0.5).round_ties_even() as usize;
        let minimum_weight = (((self.interline as f64) * 0.2).round_ties_even() as usize).max(1);
        let segment_length = ((self.interline as f64) * 4.0).round_ties_even();
        if probe_width == 0 || segment_length == 0.0 {
            return Err(FilamentError::DegenerateGeometry);
        }

        let start_x = bounds.x as f64;
        let stop_x = (bounds.x + bounds.width - 1) as f64;
        let length = stop_x - start_x + 1.0;
        let segment_count = (length / segment_length).round_ties_even() as usize;
        if segment_count == 0 {
            return Err(FilamentError::DegenerateGeometry);
        }
        let precise_segment_length = length / segment_count as f64;

        let first_roi = Bounds {
            x: start_x.ceil() as usize,
            y: bounds.y,
            width: probe_width,
            height: bounds.height,
        };
        let first_centroid = self
            .centroid(first_roi, 1)
            .ok_or(FilamentError::EmptyProbe)?;
        let start = (start_x, first_centroid.1);
        let mut points = vec![start];

        for index in 1..segment_count {
            let roi = Bounds {
                x: (start_x + (index as f64 * precise_segment_length)).round_ties_even() as usize,
                y: bounds.y,
                width: probe_width,
                height: bounds.height,
            };
            if let Some(point) = self.centroid(roi, minimum_weight) {
                points.push(point);
            }
        }

        let final_x = (stop_x - probe_width as f64 + 1.0).floor() as usize;
        let final_roi = Bounds {
            x: final_x,
            y: bounds.y,
            width: probe_width,
            height: bounds.height,
        };
        let final_centroid = self
            .centroid(final_roi, 1)
            .ok_or(FilamentError::EmptyProbe)?;
        let stop = (stop_x, final_centroid.1);
        points.push(stop);
        let spline = NaturalSpline::interpolate(&points)?;

        Ok(FilamentGeometry {
            start,
            stop,
            points,
            spline,
        })
    }

    fn centroid(&self, roi: Bounds, minimum_weight: usize) -> Option<(f64, f64)> {
        let roi_stop_x = roi.x + roi.width - 1;
        let roi_stop_y = roi.y + roi.height - 1;
        let mut weight = 0_usize;
        let mut sum_x = 0_f64;
        let mut sum_y = 0_f64;

        for section in &self.sections {
            for (offset, run) in section.runs().iter().enumerate() {
                let y = section.first_pos() + offset;
                if y < roi.y || y > roi_stop_y {
                    continue;
                }
                let start = run.start.max(roi.x);
                let stop = run.stop().min(roi_stop_x);
                if start > stop {
                    continue;
                }
                for x in start..=stop {
                    weight += 1;
                    sum_x += x as f64;
                    sum_y += y as f64;
                }
            }
        }

        (weight >= minimum_weight).then(|| (sum_x / weight as f64, sum_y / weight as f64))
    }
}

/// Failure in the supported neutral staff-filament surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FilamentError {
    InvalidInterline,
    Empty,
    UnsupportedOrientation,
    EmptyProbe,
    DegenerateGeometry,
    Spline(SplineError),
}

impl From<SplineError> for FilamentError {
    fn from(value: SplineError) -> Self {
        Self::Spline(value)
    }
}

impl fmt::Display for FilamentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInterline => formatter.write_str("filament interline must be positive"),
            Self::Empty => formatter.write_str("filament has no sections"),
            Self::UnsupportedOrientation => {
                formatter.write_str("staff filament requires horizontal sections")
            }
            Self::EmptyProbe => formatter.write_str("filament endpoint probe has no pixels"),
            Self::DegenerateGeometry => formatter.write_str("filament geometry is degenerate"),
            Self::Spline(error) => write!(formatter, "filament spline error: {error}"),
        }
    }
}

impl Error for FilamentError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_table::{Run, RunTable};
    use crate::section::{JunctionPolicy, build_sections};

    fn fixture() -> StaffFilament {
        let mut table = RunTable::new(Orientation::Horizontal, 165, 15).unwrap();
        for (row, start) in [(2, 0), (5, 40), (8, 80), (11, 120)] {
            table.add_run(row, Run::new(start, 45)).unwrap();
            table.add_run(row + 1, Run::new(start, 45)).unwrap();
        }
        let mut filament = StaffFilament::new(10).unwrap();
        for section in build_sections(&table, JunctionPolicy::DEFAULT_RATIO) {
            filament.add_section(section).unwrap();
        }
        filament
    }

    fn near(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= 1.0e-12,
            "{actual} != {expected}"
        );
    }

    #[test]
    fn matches_java_synthetic_metrics_and_geometry() {
        let filament = fixture();
        assert_eq!(filament.sections().len(), 4);
        assert_eq!(
            filament.bounds().unwrap(),
            Bounds {
                x: 0,
                y: 2,
                width: 165,
                height: 11
            }
        );
        assert_eq!(filament.weight(), 360);
        assert_eq!(filament.true_length().unwrap(), 165);
        near(filament.thickness().unwrap(), 360.0 / 165.0);

        let geometry = filament.geometry().unwrap();
        assert_eq!(geometry.start(), (0.0, 2.5));
        assert_eq!(geometry.stop(), (164.0, 11.5));
        for (x, position, slope) in [
            (0.0, 2.5, 0.031_307_977_737),
            (40.0, 4.019_976_180_724, 0.051_382_258_081),
            (80.0, 7.025_570_151_553, 0.094_844_643_539),
            (120.0, 10.663_237_522_531, 0.066_575_007_119),
            (164.0, 11.5, -0.008_850_931_677),
        ] {
            near(geometry.position_at(x).unwrap(), position);
            near(geometry.slope_at(x).unwrap(), slope);
        }
        assert_eq!(
            [-1.0, 0.0, 164.0, 165.0].map(|x| geometry.is_within_range(x)),
            [false, true, true, false]
        );
    }

    #[test]
    fn rejects_invalid_and_vertical_inputs() {
        assert!(matches!(
            StaffFilament::new(0),
            Err(FilamentError::InvalidInterline)
        ));
        let mut filament = StaffFilament::new(10).unwrap();
        assert_eq!(filament.bounds(), Err(FilamentError::Empty));
        let mut table = RunTable::new(Orientation::Vertical, 2, 4).unwrap();
        table.add_run(0, Run::new(0, 2)).unwrap();
        let section = build_sections(&table, JunctionPolicy::All).remove(0);
        assert_eq!(
            filament.add_section(section),
            Err(FilamentError::UnsupportedOrientation)
        );
    }

    #[test]
    fn true_length_preserves_java_hole_arithmetic() {
        let mut table = RunTable::new(Orientation::Horizontal, 10, 3).unwrap();
        table.add_run(0, Run::new(0, 3)).unwrap();
        table.add_run(2, Run::new(5, 3)).unwrap();
        let mut filament = StaffFilament::new(2).unwrap();
        for section in build_sections(&table, JunctionPolicy::All) {
            filament.add_section(section).unwrap();
        }
        // Java counts start-cMax (3), not the two uncovered pixels 3 and 4.
        assert_eq!(filament.true_length().unwrap(), 5);
    }
}
