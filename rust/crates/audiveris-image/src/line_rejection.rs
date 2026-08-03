// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light port of the early Java `LinesRetriever` rejection pass.
//!
//! Java first removes candidates whose spline midpoint bends too far away from
//! the start-to-stop chord. It then stably sorts the survivors by descending
//! horizontal length, estimates sheet slope from the longest prefix, and
//! removes slope outliers. Short candidates receive Java's asymmetric
//! tolerance for horizontal sections on a sloped page.

use std::{cmp::Reverse, error::Error, fmt};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinePoint {
    pub x: f64,
    pub y: f64,
}

impl LinePoint {
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// Geometry consumed by Java `purgeCurvedFilaments`,
/// `retrieveGlobalSlope`, and `purgeSlopedFilaments`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineCandidate {
    id: usize,
    start: LinePoint,
    stop: LinePoint,
    spline_midpoint: LinePoint,
    horizontal_length: usize,
}

impl LineCandidate {
    pub fn new(
        id: usize,
        start: LinePoint,
        stop: LinePoint,
        spline_midpoint_y: f64,
        horizontal_length: usize,
    ) -> Result<Self, LineRejectionError> {
        let x_mid = (start.x + stop.x) / 2.0;
        if id == 0
            || horizontal_length == 0
            || !start.x.is_finite()
            || !start.y.is_finite()
            || !stop.x.is_finite()
            || !stop.y.is_finite()
            || !spline_midpoint_y.is_finite()
            || stop.x <= start.x
        {
            return Err(LineRejectionError::InvalidCandidate(id));
        }
        Ok(Self {
            id,
            start,
            stop,
            spline_midpoint: LinePoint::new(x_mid, spline_midpoint_y),
            horizontal_length,
        })
    }

    #[must_use]
    pub const fn id(self) -> usize {
        self.id
    }

    #[must_use]
    pub const fn horizontal_length(self) -> usize {
        self.horizontal_length
    }

    #[must_use]
    pub fn slope(self) -> f64 {
        (self.stop.y - self.start.y) / (self.stop.x - self.start.x)
    }

    #[must_use]
    pub fn central_rotation(self) -> f64 {
        central_rotation(self.start, self.stop, self.spline_midpoint)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineRejectionParameters {
    pub top_ratio_for_slope: f64,
    pub maximum_filament_rotation: f64,
    pub maximum_slope_difference: f64,
    pub minimum_significant_slope: f64,
    pub minimum_length_for_strict_slope: usize,
}

impl LineRejectionParameters {
    /// Java defaults, with the scale-dependent length converted from four
    /// interlines using `Math.rint` (ties to even).
    #[must_use]
    pub fn java_defaults(interline: f64) -> Self {
        Self {
            top_ratio_for_slope: 0.1,
            maximum_filament_rotation: 0.1,
            maximum_slope_difference: 0.025,
            minimum_significant_slope: 0.0002,
            minimum_length_for_strict_slope: (4.0 * interline).round_ties_even() as usize,
        }
    }

    fn validate(self) -> Result<Self, LineRejectionError> {
        if !self.top_ratio_for_slope.is_finite()
            || self.top_ratio_for_slope <= 0.0
            || self.top_ratio_for_slope > 1.0
            || !self.maximum_filament_rotation.is_finite()
            || self.maximum_filament_rotation < 0.0
            || !self.maximum_slope_difference.is_finite()
            || self.maximum_slope_difference < 0.0
            || !self.minimum_significant_slope.is_finite()
            || self.minimum_significant_slope < 0.0
            || self.minimum_length_for_strict_slope == 0
        {
            return Err(LineRejectionError::InvalidParameters);
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineRejectionReason {
    Curvature,
    Slope,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RejectedLine {
    pub candidate: LineCandidate,
    pub reason: LineRejectionReason,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LineRejectionReport {
    /// Survivors in Java's post-`retrieveGlobalSlope` stable length order.
    pub survivors: Vec<LineCandidate>,
    /// Curvature failures in original filament order.
    pub curved: Vec<RejectedLine>,
    /// Slope failures in post-sort order.
    pub sloped: Vec<RejectedLine>,
    pub global_slope: f64,
    pub slope_sample_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineRejectionError {
    InvalidCandidate(usize),
    InvalidParameters,
    NoStraightCandidates,
}

impl fmt::Display for LineRejectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCandidate(id) => write!(formatter, "invalid line candidate {id}"),
            Self::InvalidParameters => formatter.write_str("invalid line-rejection parameters"),
            Self::NoStraightCandidates => {
                formatter.write_str("no line candidates remain after curvature rejection")
            }
        }
    }
}

impl Error for LineRejectionError {}

/// Java `LineUtil.rotation`: central angle implied by chord and sagitta.
#[must_use]
pub fn central_rotation(start: LinePoint, stop: LinePoint, middle: LinePoint) -> f64 {
    let dx = stop.x - start.x;
    let dy = stop.y - start.y;
    let half_chord_length_sq = ((dx * dx) + (dy * dy)) / 4.0;
    let cross = (dx * (start.y - middle.y)) - ((start.x - middle.x) * dy);
    let sagitta_sq = (cross * cross) / ((dx * dx) + (dy * dy));
    4.0 * (sagitta_sq / half_chord_length_sq).sqrt().atan()
}

/// Stable descending horizontal-length ordering used by
/// `Compounds.byReverseLength(HORIZONTAL)`.
pub fn sort_by_reverse_horizontal_length(candidates: &mut [LineCandidate]) {
    candidates.sort_by_key(|candidate| Reverse(candidate.horizontal_length));
}

/// Java `retrieveGlobalSlope` on an already curvature-filtered population.
pub fn estimate_global_slope(
    candidates: &mut [LineCandidate],
    top_ratio: f64,
    minimum_significant_slope: f64,
) -> Result<(f64, usize), LineRejectionError> {
    if candidates.is_empty() {
        return Err(LineRejectionError::NoStraightCandidates);
    }
    if !top_ratio.is_finite()
        || top_ratio <= 0.0
        || top_ratio > 1.0
        || !minimum_significant_slope.is_finite()
        || minimum_significant_slope < 0.0
    {
        return Err(LineRejectionError::InvalidParameters);
    }
    sort_by_reverse_horizontal_length(candidates);
    let top_count = ((candidates.len() as f64 * top_ratio).round_ties_even() as usize).max(1);
    let mean = candidates[..top_count]
        .iter()
        .map(|candidate| candidate.slope())
        .sum::<f64>()
        / top_count as f64;
    let slope = if mean.abs() >= minimum_significant_slope {
        mean
    } else {
        0.0
    };
    Ok((slope, top_count))
}

/// Full Java ordering: curvature purge, stable reverse-length sort and slope
/// estimate, then slope purge.
pub fn reject_line_candidates(
    candidates: Vec<LineCandidate>,
    parameters: LineRejectionParameters,
) -> Result<LineRejectionReport, LineRejectionError> {
    let parameters = parameters.validate()?;
    let mut straight = Vec::with_capacity(candidates.len());
    let mut curved = Vec::new();
    for candidate in candidates {
        if candidate.central_rotation() > parameters.maximum_filament_rotation {
            curved.push(RejectedLine {
                candidate,
                reason: LineRejectionReason::Curvature,
            });
        } else {
            straight.push(candidate);
        }
    }
    let (global_slope, slope_sample_count) = estimate_global_slope(
        &mut straight,
        parameters.top_ratio_for_slope,
        parameters.minimum_significant_slope,
    )?;

    let minimum_short_slope = if global_slope > 0.0 {
        -parameters.maximum_slope_difference / 2.0
    } else {
        global_slope
    };
    let maximum_short_slope = if global_slope > 0.0 {
        global_slope
    } else {
        parameters.maximum_slope_difference / 2.0
    };
    let mut survivors = Vec::with_capacity(straight.len());
    let mut sloped = Vec::new();
    for candidate in straight {
        let filament_slope = candidate.slope();
        let slope_difference = (global_slope - filament_slope).abs();
        let short_tolerance = candidate.horizontal_length
            < parameters.minimum_length_for_strict_slope
            && filament_slope >= minimum_short_slope
            && filament_slope <= maximum_short_slope;
        if slope_difference > parameters.maximum_slope_difference && !short_tolerance {
            sloped.push(RejectedLine {
                candidate,
                reason: LineRejectionReason::Slope,
            });
        } else {
            survivors.push(candidate);
        }
    }

    Ok(LineRejectionReport {
        survivors,
        curved,
        sloped,
        global_slope,
        slope_sample_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(id: usize, length: usize, slope: f64) -> LineCandidate {
        LineCandidate::new(
            id,
            LinePoint::new(0.0, 0.0),
            LinePoint::new(length as f64, length as f64 * slope),
            length as f64 * slope / 2.0,
            length,
        )
        .unwrap()
    }

    fn bowed(id: usize, length: usize, slope: f64, rotation: f64) -> LineCandidate {
        let start = LinePoint::new(0.0, 0.0);
        let stop = LinePoint::new(length as f64, length as f64 * slope);
        let chord = ((stop.x - start.x).powi(2) + (stop.y - start.y).powi(2)).sqrt();
        let sagitta = chord * (rotation / 4.0).tan() / 2.0;
        let normal_y = (stop.x - start.x) / chord;
        let chord_mid_y = (start.y + stop.y) / 2.0;
        LineCandidate::new(id, start, stop, chord_mid_y + sagitta * normal_y, length).unwrap()
    }

    fn ids(candidates: &[LineCandidate]) -> Vec<usize> {
        candidates.iter().map(|candidate| candidate.id()).collect()
    }

    fn rejected_ids(candidates: &[RejectedLine]) -> Vec<usize> {
        candidates
            .iter()
            .map(|rejected| rejected.candidate.id())
            .collect()
    }

    #[test]
    fn curvature_threshold_is_strict_and_preserves_input_rejection_order() {
        let parameters = LineRejectionParameters {
            top_ratio_for_slope: 1.0,
            maximum_filament_rotation: 0.1,
            maximum_slope_difference: 1.0,
            minimum_significant_slope: 0.0,
            minimum_length_for_strict_slope: 40,
        };
        let report = reject_line_candidates(
            vec![
                bowed(1, 100, 0.0, 0.100_001),
                bowed(2, 80, 0.0, 0.1),
                bowed(3, 120, 0.0, 0.2),
                line(4, 60, 0.0),
            ],
            parameters,
        )
        .unwrap();

        assert_eq!(rejected_ids(&report.curved), [1, 3]);
        // Curvature filtering precedes the stable descending-length sort.
        assert_eq!(ids(&report.survivors), [2, 4]);
    }

    #[test]
    fn global_slope_uses_java_rint_and_stable_reverse_length_order() {
        let mut candidates = vec![
            line(1, 10, 0.9),
            line(2, 20, 0.01),
            line(3, 20, 0.5),
            line(4, 30, 0.03),
            line(5, 5, -0.9),
        ];
        // 5 * 0.5 = 2.5, and Java Math.rint selects the even integer 2.
        let (slope, count) = estimate_global_slope(&mut candidates, 0.5, 0.0002).unwrap();

        assert_eq!(count, 2);
        assert!((slope - 0.02).abs() < 1e-12);
        assert_eq!(ids(&candidates), [4, 2, 3, 1, 5]);
    }

    #[test]
    fn insignificant_global_slope_is_zero_but_boundary_is_retained() {
        let mut below = vec![line(1, 100, 0.000_199)];
        let mut boundary = vec![line(2, 100, -0.000_2)];
        assert_eq!(
            estimate_global_slope(&mut below, 0.1, 0.0002).unwrap().0,
            0.0
        );
        assert_eq!(
            estimate_global_slope(&mut boundary, 0.1, 0.0002).unwrap().0,
            -0.0002
        );
    }

    #[test]
    fn positive_sheet_short_tolerance_is_inclusive_and_length_is_strict() {
        let parameters = LineRejectionParameters {
            top_ratio_for_slope: 0.2,
            maximum_filament_rotation: 1.0,
            maximum_slope_difference: 0.025,
            minimum_significant_slope: 0.0002,
            minimum_length_for_strict_slope: 40,
        };
        let report = reject_line_candidates(
            vec![
                line(1, 100, 0.05),
                line(2, 39, 0.0),
                line(3, 38, -0.0125),
                line(4, 37, -0.012_501),
                line(5, 40, 0.0),
                line(6, 36, 0.075_001),
            ],
            parameters,
        )
        .unwrap();

        assert_eq!(report.global_slope, 0.05);
        assert_eq!(ids(&report.survivors), [1, 2, 3]);
        assert_eq!(rejected_ids(&report.sloped), [5, 4, 6]);
    }

    #[test]
    fn negative_sheet_short_tolerance_runs_from_sheet_slope_through_zero() {
        let parameters = LineRejectionParameters {
            top_ratio_for_slope: 0.2,
            maximum_filament_rotation: 1.0,
            maximum_slope_difference: 0.025,
            minimum_significant_slope: 0.0002,
            minimum_length_for_strict_slope: 40,
        };
        let report = reject_line_candidates(
            vec![
                line(1, 100, -0.05),
                line(2, 39, 0.0),
                line(3, 38, 0.0125),
                line(4, 37, 0.012_501),
                line(5, 36, -0.075_001),
            ],
            parameters,
        )
        .unwrap();

        assert_eq!(report.global_slope, -0.05);
        assert_eq!(ids(&report.survivors), [1, 2, 3]);
        assert_eq!(rejected_ids(&report.sloped), [4, 5]);
    }

    #[test]
    fn composed_pass_reports_exact_java_stage_orders() {
        let parameters = LineRejectionParameters {
            top_ratio_for_slope: 0.25,
            maximum_filament_rotation: 0.1,
            maximum_slope_difference: 0.025,
            minimum_significant_slope: 0.0002,
            minimum_length_for_strict_slope: 40,
        };
        let report = reject_line_candidates(
            vec![
                line(1, 20, 0.0),
                bowed(2, 90, 0.04, 0.2),
                line(3, 100, 0.04),
                line(4, 70, 0.09),
                line(5, 30, 0.0),
            ],
            parameters,
        )
        .unwrap();

        assert_eq!(report.slope_sample_count, 1);
        assert_eq!(report.global_slope, 0.04);
        assert_eq!(rejected_ids(&report.curved), [2]);
        assert_eq!(rejected_ids(&report.sloped), [4]);
        assert_eq!(ids(&report.survivors), [3, 5, 1]);
    }

    #[test]
    fn java_default_length_uses_ties_to_even() {
        assert_eq!(
            LineRejectionParameters::java_defaults(10.125).minimum_length_for_strict_slope,
            40
        );
        assert_eq!(
            LineRejectionParameters::java_defaults(10.375).minimum_length_for_strict_slope,
            42
        );
    }
}
