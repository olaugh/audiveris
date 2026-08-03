// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact headless seam for Java `PeakGraph.findAllAlignments`.
//!
//! The pass retains every acceptable alignment in nested projector, peak,
//! neighboring-staff, and candidate-peak order. It neither selects a best
//! alignment nor promotes one to a concrete connection; both happen later in
//! Java's pipeline.

use std::{error::Error, fmt};

use crate::{
    bar_alignment::{AlignmentPeak, BarAlignment, BarAlignmentError, BarImpacts},
    bar_column::{PeakId, StaffId},
    peak_graph::{PeakEdgeId, PeakGraph, PeakGraphError},
    staff_peak::{StaffPeak, StaffPeakKey},
};

/// Staff geometry and peak traversal order required by `vertNeighbors` and
/// `findAllAlignments`.
#[derive(Clone, Debug, PartialEq)]
pub struct AlignmentStaff {
    pub staff_id: StaffId,
    pub left: f64,
    pub right: f64,
    /// First-line left endpoint ordinate.
    pub top: f64,
    /// Last-line left endpoint ordinate.
    pub bottom: f64,
    pub short: bool,
    pub peaks: Vec<StaffPeakKey>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AlignmentParameters {
    /// Java sheet skew slope; its vertical reference is the negation.
    pub sheet_slope: f64,
    pub maximum_alignment_slope: f64,
    pub maximum_alignment_delta_width: i32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AlignmentBuildReport {
    edge_ids: Vec<PeakEdgeId>,
}

impl AlignmentBuildReport {
    #[must_use]
    pub fn edge_ids(&self) -> &[PeakEdgeId] {
        &self.edge_ids
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AlignmentBuildError {
    InvalidParameters,
    DuplicateStaffId(StaffId),
    InvalidStaffGeometry(StaffId),
    MissingPeak(StaffPeakKey),
    WrongPeakStaff { peak: StaffPeakKey, staff: StaffId },
    MissingPeakImpacts(StaffPeakKey),
    PeakIdExhausted,
    Relation(BarAlignmentError),
    Graph(PeakGraphError),
}

impl fmt::Display for AlignmentBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidParameters => formatter.write_str("invalid bar-alignment parameters"),
            Self::DuplicateStaffId(staff) => {
                write!(formatter, "duplicate alignment staff {}", staff.value())
            }
            Self::InvalidStaffGeometry(staff) => {
                write!(
                    formatter,
                    "invalid alignment geometry for staff {}",
                    staff.value()
                )
            }
            Self::MissingPeak(peak) => write!(formatter, "alignment peak is absent: {peak:?}"),
            Self::WrongPeakStaff { peak, staff } => write!(
                formatter,
                "alignment peak {peak:?} is not owned by staff {}",
                staff.value()
            ),
            Self::MissingPeakImpacts(peak) => {
                write!(
                    formatter,
                    "alignment peak has no projection impacts: {peak:?}"
                )
            }
            Self::PeakIdExhausted => formatter.write_str("alignment peak ID exhausted"),
            Self::Relation(source) => {
                write!(formatter, "bar-alignment construction failed: {source}")
            }
            Self::Graph(source) => write!(formatter, "peak-graph insertion failed: {source}"),
        }
    }
}

impl Error for AlignmentBuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Relation(source) => Some(source),
            Self::Graph(source) => Some(source),
            _ => None,
        }
    }
}

/// Insert every Java-acceptable alignment. Any insertion failure leaves the
/// successful edge prefix in the graph and report.
pub fn find_all_alignments(
    graph: &mut PeakGraph<BarAlignment>,
    staffs: &[AlignmentStaff],
    parameters: AlignmentParameters,
    report: &mut AlignmentBuildReport,
) -> Result<(), AlignmentBuildError> {
    validate_inputs(graph, staffs, parameters)?;
    let peak_ids = graph
        .vertices()
        .iter()
        .enumerate()
        .map(|(index, peak)| {
            let value = index
                .checked_add(1)
                .ok_or(AlignmentBuildError::PeakIdExhausted)?;
            Ok((peak.key(), PeakId::new(value)))
        })
        .collect::<Result<Vec<_>, AlignmentBuildError>>()?;

    for (staff_index, staff) in staffs.iter().enumerate() {
        let below = vertical_neighbors_below(staffs, staff_index);
        if below.is_empty() || below[0].short != staff.short {
            continue;
        }
        for &top_key in &staff.peaks {
            let top = graph
                .vertex(top_key)
                .expect("alignment inputs were prevalidated")
                .clone();
            for bottom_staff in below.iter().copied() {
                for &bottom_key in &bottom_staff.peaks {
                    let bottom = graph
                        .vertex(bottom_key)
                        .expect("alignment inputs were prevalidated")
                        .clone();
                    let Some(relation) = check_alignment(&top, &bottom, &peak_ids, parameters)?
                    else {
                        continue;
                    };
                    let edge = graph
                        .add_edge(top_key, bottom_key, relation)
                        .map_err(AlignmentBuildError::Graph)?;
                    report.edge_ids.push(edge);
                }
            }
        }
    }
    Ok(())
}

fn validate_inputs(
    graph: &PeakGraph<BarAlignment>,
    staffs: &[AlignmentStaff],
    parameters: AlignmentParameters,
) -> Result<(), AlignmentBuildError> {
    if !parameters.sheet_slope.is_finite()
        || !parameters.maximum_alignment_slope.is_finite()
        || parameters.maximum_alignment_slope <= 0.0
        || parameters.maximum_alignment_delta_width <= 0
    {
        return Err(AlignmentBuildError::InvalidParameters);
    }
    let mut seen = Vec::with_capacity(staffs.len());
    for staff in staffs {
        if seen.contains(&staff.staff_id) {
            return Err(AlignmentBuildError::DuplicateStaffId(staff.staff_id));
        }
        seen.push(staff.staff_id);
        if !staff.left.is_finite()
            || !staff.right.is_finite()
            || !staff.top.is_finite()
            || !staff.bottom.is_finite()
            || staff.right < staff.left
            || staff.bottom < staff.top
        {
            return Err(AlignmentBuildError::InvalidStaffGeometry(staff.staff_id));
        }
        for &key in &staff.peaks {
            let peak = graph
                .vertex(key)
                .ok_or(AlignmentBuildError::MissingPeak(key))?;
            if peak.staff_id() != staff.staff_id {
                return Err(AlignmentBuildError::WrongPeakStaff {
                    peak: key,
                    staff: staff.staff_id,
                });
            }
            if peak.impacts().is_none() {
                return Err(AlignmentBuildError::MissingPeakImpacts(key));
            }
        }
    }
    Ok(())
}

fn vertical_neighbors_below(
    staffs: &[AlignmentStaff],
    current_index: usize,
) -> Vec<&AlignmentStaff> {
    let current = &staffs[current_index];
    let Some(first_index) =
        ((current_index + 1)..staffs.len()).find(|&index| x_overlaps(current, &staffs[index]))
    else {
        return Vec::new();
    };
    let mut indices = vec![first_index];
    for direction in [-1_isize, 1] {
        let mut cursor = first_index;
        while let Some(next) = horizontal_neighbor(staffs, cursor, direction) {
            if !indices.contains(&next) {
                indices.push(next);
            }
            cursor = next;
        }
    }
    indices.retain(|&index| index != current_index);
    indices.sort_unstable_by_key(|&index| staffs[index].staff_id.value());
    indices.into_iter().map(|index| &staffs[index]).collect()
}

fn horizontal_neighbor(
    staffs: &[AlignmentStaff],
    current_index: usize,
    direction: isize,
) -> Option<usize> {
    let mut index = current_index as isize + direction;
    while (0..staffs.len() as isize).contains(&index) {
        let candidate = index as usize;
        if y_overlaps(&staffs[current_index], &staffs[candidate]) {
            return Some(candidate);
        }
        index += direction;
    }
    None
}

fn x_overlaps(one: &AlignmentStaff, two: &AlignmentStaff) -> bool {
    one.left.max(two.left) < one.right.min(two.right)
}

fn y_overlaps(one: &AlignmentStaff, two: &AlignmentStaff) -> bool {
    one.top.max(two.top) < one.bottom.min(two.bottom)
}

fn check_alignment(
    top: &StaffPeak,
    bottom: &StaffPeak,
    peak_ids: &[(StaffPeakKey, PeakId)],
    parameters: AlignmentParameters,
) -> Result<Option<BarAlignment>, AlignmentBuildError> {
    let sheet_vertical_slope = -parameters.sheet_slope;
    let top_y = f64::from(top.bottom());
    let bottom_y = f64::from(bottom.top());
    let left_slope = (f64::from(bottom.start()) - f64::from(top.start())) / (bottom_y - top_y);
    let left_difference = (left_slope - sheet_vertical_slope).abs();
    let right_slope = (f64::from(bottom.stop()) - f64::from(top.stop())) / (bottom_y - top_y);
    let right_difference = (right_slope - sheet_vertical_slope).abs();
    let minimum_slope = java_min(left_difference, right_difference);
    if minimum_slope > parameters.maximum_alignment_slope {
        return Ok(None);
    }

    // Java performs the integer subtraction before promotion to double.
    let delta_width = f64::from(bottom.width().wrapping_sub(top.width()));
    if delta_width.abs() > f64::from(parameters.maximum_alignment_delta_width) {
        return Ok(None);
    }
    let align_impact = 1.0 - (minimum_slope.abs() / parameters.maximum_alignment_slope);
    let width_impact =
        1.0 - (delta_width.abs() / f64::from(parameters.maximum_alignment_delta_width));
    let top_peak = alignment_peak(top, peak_ids)?;
    let bottom_peak = alignment_peak(bottom, peak_ids)?;
    let impacts =
        BarImpacts::alignment(align_impact, width_impact).map_err(AlignmentBuildError::Relation)?;
    BarAlignment::new(top_peak, bottom_peak, minimum_slope, delta_width, impacts)
        .map(Some)
        .map_err(AlignmentBuildError::Relation)
}

fn alignment_peak(
    peak: &StaffPeak,
    peak_ids: &[(StaffPeakKey, PeakId)],
) -> Result<AlignmentPeak, AlignmentBuildError> {
    let id = peak_ids
        .iter()
        .find_map(|(key, id)| (*key == peak.key()).then_some(*id))
        .expect("graph vertex has a precomputed insertion ID");
    let width = usize::try_from(peak.width())
        .map_err(|_| AlignmentBuildError::Relation(BarAlignmentError::InvalidPeakGeometry(id)))?;
    AlignmentPeak::with_geometry(
        id,
        peak.staff_id(),
        peak.start(),
        width,
        peak.top(),
        peak.bottom(),
        peak.impacts()
            .ok_or(AlignmentBuildError::MissingPeakImpacts(peak.key()))?
            .grade(),
    )
    .map_err(AlignmentBuildError::Relation)
}

fn java_min(one: f64, two: f64) -> f64 {
    if one.is_nan() || two.is_nan() {
        f64::NAN
    } else {
        one.min(two)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        bar_alignment::BarAlignmentKind,
        staff_peak::{StaffPeak, StaffVerticalImpacts},
    };

    fn peak(staff: usize, top: i32, bottom: i32, start: i32, stop: i32) -> StaffPeak {
        let mut peak = StaffPeak::with_impacts(
            StaffId::new(staff),
            top,
            bottom,
            start,
            stop,
            StaffVerticalImpacts::new(1.0, 1.0, 1.0, 1.0, 1.0, 1.0),
        )
        .unwrap();
        peak.compute_deskewed_center(|point| point).unwrap();
        peak
    }

    fn staff(id: usize, top: f64, bottom: f64, peaks: &[StaffPeakKey]) -> AlignmentStaff {
        AlignmentStaff {
            staff_id: StaffId::new(id),
            left: 0.0,
            right: 100.0,
            top,
            bottom,
            short: false,
            peaks: peaks.to_vec(),
        }
    }

    fn parameters() -> AlignmentParameters {
        AlignmentParameters {
            sheet_slope: 0.02,
            maximum_alignment_slope: 0.06,
            maximum_alignment_delta_width: 2,
        }
    }

    #[test]
    fn full_width_staves_insert_relations_in_java_nested_order() {
        let top_left = peak(1, 2, 9, 10, 11);
        let top_right = peak(1, 2, 9, 30, 31);
        let middle_left = peak(2, 20, 27, 10, 11);
        let middle_right = peak(2, 20, 27, 30, 31);
        let bottom_left = peak(3, 38, 45, 10, 11);
        let bottom_right = peak(3, 38, 45, 30, 31);
        let staffs = [
            staff(1, 2.0, 9.0, &[top_left.key(), top_right.key()]),
            staff(2, 20.0, 27.0, &[middle_left.key(), middle_right.key()]),
            staff(3, 38.0, 45.0, &[bottom_left.key(), bottom_right.key()]),
        ];
        let mut graph = PeakGraph::new();
        for peak in [
            top_left.clone(),
            top_right.clone(),
            middle_left.clone(),
            middle_right.clone(),
            bottom_left.clone(),
            bottom_right.clone(),
        ] {
            assert!(graph.add_vertex(peak));
        }
        let mut report = AlignmentBuildReport::default();

        find_all_alignments(&mut graph, &staffs, parameters(), &mut report).unwrap();

        assert_eq!(report.edge_ids().len(), 4);
        assert_eq!(
            graph
                .edges()
                .iter()
                .map(|edge| (edge.source(), edge.target()))
                .collect::<Vec<_>>(),
            [
                (top_left.key(), middle_left.key()),
                (top_right.key(), middle_right.key()),
                (middle_left.key(), bottom_left.key()),
                (middle_right.key(), bottom_right.key()),
            ]
        );
        let first = graph.edges()[0].relation();
        assert_eq!(first.kind(), BarAlignmentKind::Alignment);
        assert!((first.slope() - 0.02).abs() < 1.0e-14);
        assert_eq!(first.delta_width(), 0.0);
        assert!((first.impacts().align() - (2.0 / 3.0)).abs() < 1.0e-14);
        assert_eq!(first.impacts().width(), 1.0);
        let expected_grade = 0.8 * (4.0_f64 / 9.0).powf(1.0 / 3.0);
        assert!((first.grade() - expected_grade).abs() < 1.0e-14);
    }

    #[test]
    fn side_by_side_lower_neighbors_are_sorted_and_short_gate_uses_first() {
        let left = peak(1, 2, 9, 10, 11);
        let right = peak(1, 2, 9, 80, 81);
        let below_left = peak(2, 20, 27, 10, 11);
        let below_right = peak(3, 20, 27, 80, 81);
        let mut top = staff(1, 2.0, 9.0, &[left.key(), right.key()]);
        let mut lower_left = staff(2, 20.0, 27.0, &[below_left.key()]);
        let mut lower_right = staff(3, 20.0, 27.0, &[below_right.key()]);
        top.left = 0.0;
        top.right = 100.0;
        lower_left.left = 0.0;
        lower_left.right = 45.0;
        lower_right.left = 55.0;
        lower_right.right = 100.0;
        let mut graph = PeakGraph::new();
        for peak in [
            left.clone(),
            right.clone(),
            below_left.clone(),
            below_right.clone(),
        ] {
            assert!(graph.add_vertex(peak));
        }
        let mut report = AlignmentBuildReport::default();
        find_all_alignments(
            &mut graph,
            &[top.clone(), lower_left.clone(), lower_right.clone()],
            parameters(),
            &mut report,
        )
        .unwrap();
        assert_eq!(
            graph
                .edges()
                .iter()
                .map(|edge| (edge.source(), edge.target()))
                .collect::<Vec<_>>(),
            [
                (left.key(), below_left.key()),
                (right.key(), below_right.key())
            ]
        );

        let mut gated = PeakGraph::new();
        for peak in [left, right, below_left, below_right] {
            assert!(gated.add_vertex(peak));
        }
        lower_left.short = true;
        let mut gated_report = AlignmentBuildReport::default();
        find_all_alignments(
            &mut gated,
            &[top, lower_left, lower_right],
            parameters(),
            &mut gated_report,
        )
        .unwrap();
        assert!(gated.edges().is_empty());
    }

    #[test]
    fn duplicate_insertion_keeps_prior_edge_prefix_and_does_not_purge_competitors() {
        let top = peak(1, 2, 9, 10, 11);
        let first = peak(2, 20, 27, 10, 11);
        let second = peak(2, 20, 27, 10, 12);
        let staffs = [
            staff(1, 2.0, 9.0, &[top.key()]),
            staff(2, 20.0, 27.0, &[first.key(), second.key()]),
        ];
        let mut graph = PeakGraph::new();
        for peak in [top.clone(), first.clone(), second.clone()] {
            assert!(graph.add_vertex(peak));
        }
        let ids = [
            (top.key(), PeakId::new(1)),
            (first.key(), PeakId::new(2)),
            (second.key(), PeakId::new(3)),
        ];
        let existing = check_alignment(&top, &second, &ids, parameters())
            .unwrap()
            .unwrap();
        graph.add_edge(top.key(), second.key(), existing).unwrap();
        let mut report = AlignmentBuildReport::default();

        assert_eq!(
            find_all_alignments(&mut graph, &staffs, parameters(), &mut report),
            Err(AlignmentBuildError::Graph(PeakGraphError::DuplicateEdge {
                source: top.key(),
                target: second.key(),
            }))
        );
        assert_eq!(report.edge_ids().len(), 1);
        assert!(graph.edge_between(top.key(), first.key()).is_some());
        assert!(graph.edge_between(top.key(), second.key()).is_some());
        assert_eq!(graph.edges().len(), 2);
    }
}
