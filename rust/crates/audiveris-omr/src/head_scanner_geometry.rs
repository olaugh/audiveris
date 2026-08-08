// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact geometry adapters for Java `NoteHeadsBuilder.Scanner`.
//!
//! A persistent staff line and a ledger glyph expose superficially similar
//! axes, but Java evaluates them differently. Staff ordinates use
//! `NaturalSpline.yAtX` within the endpoint range and the global endpoint
//! chord outside it. Ledger ordinates use `LineUtil.yAtX`, including its
//! determinant operation order. Integer ordinate rounding remains owned by
//! [`crate::head_scanner`].

use std::{error::Error, fmt};

use audiveris_image::{
    beam_structure::Segment,
    staff_line_conversion::PersistentStaffLine,
    system_population::{BoundarySegment, StaffBoundary, boundary_from_points},
};

use crate::head_scanner::{HeadScannerLedger, HeadScannerLine};

type ScannerPoint = (f64, f64);
type ScannerEndpoints = (ScannerPoint, ScannerPoint);

/// Invalid persistent geometry that cannot safely back an infallible scanner
/// line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeadScannerGeometryError {
    /// A persistent staff line has fewer than the two required defining points.
    TooFewPersistentStaffPoints { count: usize },
    /// A staff boundary has no spline segment.
    EmptyStaffBoundary,
    /// A staff spline segment contains a non-finite coordinate.
    NonFiniteStaffSegment { index: usize },
    /// A staff spline segment does not run strictly from left to right.
    NonIncreasingStaffSegment { index: usize },
    /// A staff spline segment does not start at the preceding segment's end.
    DisconnectedStaffSegments { index: usize },
    /// A ledger center-line contains a non-finite coordinate.
    NonFiniteLedgerAxis,
    /// A ledger center-line does not run strictly from left to right.
    NonIncreasingLedgerAxis,
}

impl fmt::Display for HeadScannerGeometryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooFewPersistentStaffPoints { count } => write!(
                formatter,
                "persistent staff line has {count} defining points; at least 2 are required"
            ),
            Self::EmptyStaffBoundary => write!(formatter, "staff boundary has no spline segment"),
            Self::NonFiniteStaffSegment { index } => {
                write!(formatter, "staff spline segment {index} is not finite")
            }
            Self::NonIncreasingStaffSegment { index } => write!(
                formatter,
                "staff spline segment {index} does not run strictly left to right"
            ),
            Self::DisconnectedStaffSegments { index } => write!(
                formatter,
                "staff spline segment {index} is disconnected from its predecessor"
            ),
            Self::NonFiniteLedgerAxis => write!(formatter, "ledger center-line is not finite"),
            Self::NonIncreasingLedgerAxis => {
                write!(
                    formatter,
                    "ledger center-line does not run strictly left to right"
                )
            }
        }
    }
}

impl Error for HeadScannerGeometryError {}

/// Exact endpoint axis shared by the concrete staff and ledger adapters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeadScannerAxis {
    /// Left endpoint abscissa.
    pub x1: f64,
    /// Left endpoint ordinate.
    pub y1: f64,
    /// Right endpoint abscissa.
    pub x2: f64,
    /// Right endpoint ordinate.
    pub y2: f64,
}

impl HeadScannerAxis {
    const fn from_endpoints(left: (f64, f64), right: (f64, f64)) -> Self {
        Self {
            x1: left.0,
            y1: left.1,
            x2: right.0,
            y2: right.1,
        }
    }

    const fn as_segment(&self) -> Segment {
        Segment {
            x1: self.x1,
            y1: self.y1,
            x2: self.x2,
            y2: self.y2,
        }
    }
}

impl From<Segment> for HeadScannerAxis {
    fn from(axis: Segment) -> Self {
        Self {
            x1: axis.x1,
            y1: axis.y1,
            x2: axis.x2,
            y2: axis.y2,
        }
    }
}

/// Owned adapter for a persistent staff-line spline.
#[derive(Clone, Debug, PartialEq)]
pub struct StaffLineAdapter {
    boundary: StaffBoundary,
    axis: HeadScannerAxis,
    left_abscissa: i32,
    right_abscissa: i32,
}

/// Scanner-oriented name for [`StaffLineAdapter`].
pub type StaffLineScannerAdapter = StaffLineAdapter;

impl StaffLineAdapter {
    /// Validate and own a persistent staff-line spline.
    pub fn try_new(boundary: StaffBoundary) -> Result<Self, HeadScannerGeometryError> {
        let (left_endpoint, right_endpoint) = validate_staff_axis(&boundary)?;
        Ok(Self {
            left_abscissa: left_endpoint.0.floor() as i32,
            right_abscissa: right_endpoint.0.floor() as i32,
            boundary,
            axis: HeadScannerAxis::from_endpoints(left_endpoint, right_endpoint),
        })
    }

    /// Build and own the exact spline represented by a persistent staff line.
    pub fn try_from_persistent(
        line: &PersistentStaffLine,
    ) -> Result<Self, HeadScannerGeometryError> {
        if line.points.len() < 2 {
            return Err(HeadScannerGeometryError::TooFewPersistentStaffPoints {
                count: line.points.len(),
            });
        }
        let boundary = boundary_from_points(&line.points).map_err(|_| {
            HeadScannerGeometryError::TooFewPersistentStaffPoints {
                count: line.points.len(),
            }
        })?;
        Self::try_new(boundary)
    }

    /// Build and own the exact spline represented by a persistent staff line.
    pub fn from_persistent(line: &PersistentStaffLine) -> Result<Self, HeadScannerGeometryError> {
        Self::try_from_persistent(line)
    }

    /// The global endpoint chord of the persistent staff line.
    #[must_use]
    pub const fn axis(&self) -> HeadScannerAxis {
        self.axis
    }

    /// The exact persistent spline used for in-range ordinate evaluation.
    #[must_use]
    pub fn boundary(&self) -> &StaffBoundary {
        &self.boundary
    }

    /// Java `LineInfo.getEndPoint(LEFT)`.
    #[must_use]
    pub const fn left_endpoint(&self) -> (f64, f64) {
        (self.axis.x1, self.axis.y1)
    }

    /// Java `LineInfo.getEndPoint(RIGHT)`.
    #[must_use]
    pub const fn right_endpoint(&self) -> (f64, f64) {
        (self.axis.x2, self.axis.y2)
    }

    /// Java staff adapter's `floor(left endpoint x)` rule.
    #[must_use]
    pub const fn left_abscissa(&self) -> i32 {
        self.left_abscissa
    }

    /// Java staff adapter's `floor(right endpoint x)` rule.
    #[must_use]
    pub const fn right_abscissa(&self) -> i32 {
        self.right_abscissa
    }

    /// Recover the owned persistent spline.
    #[must_use]
    pub fn into_boundary(self) -> StaffBoundary {
        self.boundary
    }
}

impl TryFrom<StaffBoundary> for StaffLineAdapter {
    type Error = HeadScannerGeometryError;

    fn try_from(axis: StaffBoundary) -> Result<Self, Self::Error> {
        Self::try_new(axis)
    }
}

impl TryFrom<&PersistentStaffLine> for StaffLineAdapter {
    type Error = HeadScannerGeometryError;

    fn try_from(line: &PersistentStaffLine) -> Result<Self, Self::Error> {
        Self::try_from_persistent(line)
    }
}

impl TryFrom<PersistentStaffLine> for StaffLineAdapter {
    type Error = HeadScannerGeometryError;

    fn try_from(line: PersistentStaffLine) -> Result<Self, Self::Error> {
        Self::try_from_persistent(&line)
    }
}

impl HeadScannerLine for StaffLineAdapter {
    fn y_at(&self, x: f64) -> f64 {
        if x < self.axis.x1 || x > self.axis.x2 {
            // Preserve StaffLine.yAt's operation order: compute the global
            // slope first, then multiply it by the query delta.
            let slope = (self.axis.y2 - self.axis.y1) / (self.axis.x2 - self.axis.x1);
            self.axis.y1 + (slope * (x - self.axis.x1))
        } else if x.is_nan() {
            x
        } else {
            self.boundary
                .geopath_y_at_x(x)
                .expect("validated staff spline covers its endpoint range")
        }
    }
}

/// Owned adapter for a ledger glyph's horizontal center-line.
#[derive(Clone, Debug, PartialEq)]
pub struct LedgerAdapter {
    axis: HeadScannerAxis,
    left_abscissa: i32,
    right_abscissa: i32,
}

/// Scanner-oriented name for [`LedgerAdapter`].
pub type LedgerScannerAdapter = LedgerAdapter;

impl LedgerAdapter {
    /// Validate and own a ledger center-line.
    pub fn try_new(axis: Segment) -> Result<Self, HeadScannerGeometryError> {
        if ![axis.x1, axis.y1, axis.x2, axis.y2]
            .into_iter()
            .all(f64::is_finite)
        {
            return Err(HeadScannerGeometryError::NonFiniteLedgerAxis);
        }
        if axis.x1 >= axis.x2 {
            return Err(HeadScannerGeometryError::NonIncreasingLedgerAxis);
        }

        Ok(Self {
            left_abscissa: axis.x1.ceil() as i32,
            right_abscissa: axis.x2.floor() as i32,
            axis: axis.into(),
        })
    }

    /// Validate and own a ledger center-line.
    pub fn from_segment(axis: Segment) -> Result<Self, HeadScannerGeometryError> {
        Self::try_new(axis)
    }

    /// The exact glyph center-line used as the ledger axis.
    #[must_use]
    pub const fn axis(&self) -> HeadScannerAxis {
        self.axis
    }

    /// Java `Glyph.getStartPoint(HORIZONTAL)`.
    #[must_use]
    pub const fn left_endpoint(&self) -> (f64, f64) {
        (self.axis.x1, self.axis.y1)
    }

    /// Java `Glyph.getStopPoint(HORIZONTAL)`.
    #[must_use]
    pub const fn right_endpoint(&self) -> (f64, f64) {
        (self.axis.x2, self.axis.y2)
    }

    /// Java ledger adapter's `ceil(left endpoint x)` rule.
    #[must_use]
    pub const fn left_abscissa(&self) -> i32 {
        self.left_abscissa
    }

    /// Java ledger adapter's `floor(right endpoint x)` rule.
    #[must_use]
    pub const fn right_abscissa(&self) -> i32 {
        self.right_abscissa
    }
}

impl TryFrom<Segment> for LedgerAdapter {
    type Error = HeadScannerGeometryError;

    fn try_from(axis: Segment) -> Result<Self, Self::Error> {
        Self::try_new(axis)
    }
}

impl HeadScannerLine for LedgerAdapter {
    fn y_at(&self, x: f64) -> f64 {
        self.axis.as_segment().y_at_x(x)
    }
}

impl HeadScannerLedger for LedgerAdapter {
    fn left_abscissa(&self) -> i32 {
        self.left_abscissa
    }

    fn right_abscissa(&self) -> i32 {
        self.right_abscissa
    }
}

fn validate_staff_axis(axis: &StaffBoundary) -> Result<ScannerEndpoints, HeadScannerGeometryError> {
    let first = axis
        .segments
        .first()
        .copied()
        .ok_or(HeadScannerGeometryError::EmptyStaffBoundary)?;
    let left_endpoint = segment_start(first);
    let mut previous_end = None;

    for (index, segment) in axis.segments.iter().copied().enumerate() {
        let start = segment_start(segment);
        let end = segment_end(segment);
        if !segment_is_finite(segment) {
            return Err(HeadScannerGeometryError::NonFiniteStaffSegment { index });
        }
        if start.0 >= end.0 {
            return Err(HeadScannerGeometryError::NonIncreasingStaffSegment { index });
        }
        if previous_end.is_some_and(|point| point != start) {
            return Err(HeadScannerGeometryError::DisconnectedStaffSegments { index });
        }
        previous_end = Some(end);
    }

    Ok((
        left_endpoint,
        previous_end.expect("non-empty staff boundary has a final endpoint"),
    ))
}

const fn segment_start(segment: BoundarySegment) -> (f64, f64) {
    match segment {
        BoundarySegment::Line { start, .. }
        | BoundarySegment::Quadratic { start, .. }
        | BoundarySegment::Cubic { start, .. } => start,
    }
}

const fn segment_end(segment: BoundarySegment) -> (f64, f64) {
    match segment {
        BoundarySegment::Line { end, .. }
        | BoundarySegment::Quadratic { end, .. }
        | BoundarySegment::Cubic { end, .. } => end,
    }
}

fn segment_is_finite(segment: BoundarySegment) -> bool {
    let point_is_finite = |point: (f64, f64)| point.0.is_finite() && point.1.is_finite();
    match segment {
        BoundarySegment::Line { start, end } => point_is_finite(start) && point_is_finite(end),
        BoundarySegment::Quadratic {
            start,
            control,
            end,
        } => point_is_finite(start) && point_is_finite(control) && point_is_finite(end),
        BoundarySegment::Cubic {
            start,
            control1,
            control2,
            end,
        } => {
            point_is_finite(start)
                && point_is_finite(control1)
                && point_is_finite(control2)
                && point_is_finite(end)
        }
    }
}

#[cfg(test)]
mod tests {
    use audiveris_image::{
        run_table::{FOREGROUND, Orientation, RunTable},
        staff_line_conversion::StaffGlyph,
    };

    use super::*;

    fn persistent_staff_line(points: Vec<(f64, f64)>) -> PersistentStaffLine {
        PersistentStaffLine {
            points,
            thickness: 1.0,
            glyph: StaffGlyph {
                x: 0,
                y: 0,
                runs: RunTable::from_pixels(Orientation::Horizontal, 2, 1, &[FOREGROUND; 2])
                    .unwrap(),
            },
        }
    }

    #[test]
    fn persistent_staff_line_constructor_builds_an_owned_exact_spline() {
        let persistent =
            persistent_staff_line(vec![(0.75, 20.0), (4.0, 19.5), (8.0, 21.0), (10.875, 20.5)]);
        let adapter = StaffLineScannerAdapter::from_persistent(&persistent).unwrap();

        assert_eq!(adapter.left_endpoint(), persistent.points[0]);
        assert_eq!(adapter.right_endpoint(), persistent.points[3]);
        assert_eq!(adapter.left_abscissa(), 0);
        assert_eq!(adapter.right_abscissa(), 10);
        assert_eq!(adapter.boundary().segments.len(), 3);
        assert_eq!(adapter.clone(), adapter);

        let too_short = persistent_staff_line(vec![(0.0, 10.0)]);
        assert_eq!(
            StaffLineAdapter::try_from(&too_short),
            Err(HeadScannerGeometryError::TooFewPersistentStaffPoints { count: 1 })
        );
    }

    #[test]
    fn staff_adapter_uses_geopath_inside_and_global_chord_outside() {
        let axis = StaffBoundary {
            segments: vec![BoundarySegment::Cubic {
                start: (10.75, 20.0),
                control1: (10.75, 40.0),
                control2: (20.875, 40.0),
                end: (20.875, 24.0),
            }],
        };
        let adapter = StaffLineAdapter::try_new(axis.clone()).unwrap();

        assert_eq!(adapter.left_endpoint(), (10.75, 20.0));
        assert_eq!(adapter.right_endpoint(), (20.875, 24.0));
        assert_eq!(
            adapter.axis(),
            HeadScannerAxis {
                x1: 10.75,
                y1: 20.0,
                x2: 20.875,
                y2: 24.0,
            }
        );
        assert_eq!(adapter.left_abscissa(), 10);
        assert_eq!(adapter.right_abscissa(), 20);

        let inside_x = 13.0;
        let geopath = axis.geopath_y_at_x(inside_x).unwrap();
        assert_eq!(adapter.y_at(inside_x).to_bits(), geopath.to_bits());
        assert_ne!(
            adapter.y_at(inside_x).to_bits(),
            axis.y_at_x(inside_x).unwrap().to_bits()
        );

        let outside_x = 7.25;
        let slope = (24.0 - 20.0) / (20.875 - 10.75);
        let extrapolated = 20.0 + (slope * (outside_x - 10.75));
        assert_eq!(adapter.y_at(outside_x).to_bits(), extrapolated.to_bits());
        assert_eq!(adapter.boundary(), &axis);
    }

    #[test]
    fn ledger_adapter_preserves_axis_arithmetic_and_asymmetric_bounds() {
        let axis = Segment {
            x1: 10.125,
            y1: 200.75,
            x2: 21.875,
            y2: 203.5,
        };
        let adapter = LedgerScannerAdapter::from_segment(axis).unwrap();

        assert_eq!(adapter.axis(), HeadScannerAxis::from(axis));
        assert_eq!(adapter.left_endpoint(), (10.125, 200.75));
        assert_eq!(adapter.right_endpoint(), (21.875, 203.5));
        assert_eq!(adapter.left_abscissa(), 11);
        assert_eq!(adapter.right_abscissa(), 21);
        assert_eq!(adapter.y_at(17.0).to_bits(), axis.y_at_x(17.0).to_bits());

        let as_ledger: &dyn HeadScannerLedger = &adapter;
        assert_eq!(as_ledger.left_abscissa(), 11);
        assert_eq!(as_ledger.right_abscissa(), 21);
    }

    #[test]
    fn staff_validation_rejects_empty_disconnected_and_non_function_axes() {
        assert_eq!(
            StaffLineAdapter::try_new(StaffBoundary { segments: vec![] }),
            Err(HeadScannerGeometryError::EmptyStaffBoundary)
        );

        let disconnected = StaffBoundary {
            segments: vec![
                BoundarySegment::Line {
                    start: (0.0, 10.0),
                    end: (5.0, 11.0),
                },
                BoundarySegment::Line {
                    start: (5.0, 12.0),
                    end: (10.0, 13.0),
                },
            ],
        };
        assert_eq!(
            StaffLineAdapter::try_new(disconnected),
            Err(HeadScannerGeometryError::DisconnectedStaffSegments { index: 1 })
        );

        let reversed = StaffBoundary {
            segments: vec![BoundarySegment::Line {
                start: (5.0, 10.0),
                end: (4.0, 10.0),
            }],
        };
        assert_eq!(
            StaffLineAdapter::try_new(reversed),
            Err(HeadScannerGeometryError::NonIncreasingStaffSegment { index: 0 })
        );
    }

    #[test]
    fn construction_rejects_non_finite_and_vertical_axes() {
        let non_finite_staff = StaffBoundary {
            segments: vec![BoundarySegment::Quadratic {
                start: (0.0, 10.0),
                control: (2.0, f64::NAN),
                end: (4.0, 10.0),
            }],
        };
        assert_eq!(
            StaffLineAdapter::try_new(non_finite_staff),
            Err(HeadScannerGeometryError::NonFiniteStaffSegment { index: 0 })
        );

        assert_eq!(
            LedgerAdapter::try_new(Segment {
                x1: 4.0,
                y1: 10.0,
                x2: 4.0,
                y2: 11.0,
            }),
            Err(HeadScannerGeometryError::NonIncreasingLedgerAxis)
        );
        assert_eq!(
            LedgerAdapter::try_new(Segment {
                x1: 4.0,
                y1: 10.0,
                x2: f64::INFINITY,
                y2: 11.0,
            }),
            Err(HeadScannerGeometryError::NonFiniteLedgerAxis)
        );
    }
}
