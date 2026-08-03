// SPDX-License-Identifier: AGPL-3.0-or-later

//! Graph/projector mutation kernel for Java `PeakGraph.splitMergedGroups`.
//!
//! Filament construction and immediate alignment/connection discovery are
//! injected because they own raster and section evidence. This kernel owns the
//! source-sensitive fixed-point traversal, split tests, insertion/pruning
//! order, impacted-prefix behavior, old-vertex removal, and the subsequent
//! `purgeAlignments` call.

use std::{error::Error, fmt};

use crate::{
    bar_alignment::{BarAlignment, VerticalSide},
    bar_alignments::AlignmentStaff,
    peak_graph::{PeakGraph, PeakGraphEdge, PeakGraphError},
    staff_peak::{PeakPoint, StaffPeak, StaffPeakError, StaffPeakKey},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplitParameters {
    pub maximum_close_gap: i32,
    pub maximum_width_ratio: f64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplitEvent {
    pub original: StaffPeakKey,
    pub left: StaffPeakKey,
    pub right: StaffPeakKey,
}

#[derive(Clone, Debug, Default)]
pub struct SplitBuildReport {
    pub splits: Vec<SplitEvent>,
    /// A successful left registration remains observable when right-hand
    /// filament construction fails, as it does in Java's filament index.
    pub failed_after_left_registration: Vec<StaffPeakKey>,
    pub purged_alignments: Vec<PeakGraphEdge<BarAlignment>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SplitBuildError {
    InvalidParameters,
    MissingProjector(StaffPeakKey),
    MissingImpacts(StaffPeakKey),
    DuplicateReplacement(StaffPeakKey),
    Peak(StaffPeakError),
    Graph(PeakGraphError),
    Filament(String),
    Rediscovery(String),
}

impl fmt::Display for SplitBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidParameters => formatter.write_str("invalid merged-peak split parameters"),
            Self::MissingProjector(peak) => write!(formatter, "missing projector peak {peak:?}"),
            Self::MissingImpacts(peak) => write!(formatter, "split peak has no impacts: {peak:?}"),
            Self::DuplicateReplacement(peak) => {
                write!(formatter, "split replacement already exists: {peak:?}")
            }
            Self::Peak(source) => write!(formatter, "split peak construction failed: {source}"),
            Self::Graph(source) => write!(formatter, "split graph mutation failed: {source}"),
            Self::Filament(message) => {
                write!(formatter, "split filament construction failed: {message}")
            }
            Self::Rediscovery(message) => {
                write!(formatter, "split edge rediscovery failed: {message}")
            }
        }
    }
}

impl Error for SplitBuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Peak(source) => Some(source),
            Self::Graph(source) => Some(source),
            _ => None,
        }
    }
}

/// Run Java's split fixed point and, only after it completes, purge competing
/// alignments. Callback side effects are intentionally non-transactional.
pub fn split_merged_groups_and_purge(
    graph: &mut PeakGraph<BarAlignment>,
    staffs: &mut [AlignmentStaff],
    parameters: SplitParameters,
    mut build_filament: impl FnMut(StaffPeakKey, &StaffPeak) -> Result<bool, String>,
    mut rediscover: impl FnMut(
        &mut PeakGraph<BarAlignment>,
        &[AlignmentStaff],
        StaffPeakKey,
    ) -> Result<(), String>,
    mut deskew: impl FnMut(PeakPoint) -> PeakPoint,
    report: &mut SplitBuildReport,
) -> Result<(), SplitBuildError> {
    if parameters.maximum_close_gap < 0
        || !parameters.maximum_width_ratio.is_finite()
        || parameters.maximum_width_ratio < 0.0
    {
        return Err(SplitBuildError::InvalidParameters);
    }

    let mut to_split = peaks_to_split(
        graph,
        staffs,
        graph.vertices().iter().map(StaffPeak::key),
        parameters,
    )?;
    while !to_split.is_empty() {
        let mut impacted = Vec::new();
        for peak in to_split {
            if graph.contains_vertex(peak) {
                split_peak(
                    graph,
                    staffs,
                    peak,
                    parameters,
                    &mut build_filament,
                    &mut rediscover,
                    &mut deskew,
                    &mut impacted,
                    report,
                )?;
            }
        }
        to_split = peaks_to_split(graph, staffs, impacted, parameters)?;
    }
    report.purged_alignments = graph.purge_alignments();
    Ok(())
}

fn peaks_to_split(
    graph: &PeakGraph<BarAlignment>,
    staffs: &[AlignmentStaff],
    peaks: impl IntoIterator<Item = StaffPeakKey>,
    parameters: SplitParameters,
) -> Result<Vec<StaffPeakKey>, SplitBuildError> {
    let mut selected = Vec::new();
    for peak in peaks {
        if selected.contains(&peak) || !graph.contains_vertex(peak) {
            continue;
        }
        if !split_partners(graph, staffs, peak, parameters)?.is_empty() {
            selected.push(peak);
        }
    }
    Ok(selected)
}

fn split_partners(
    graph: &PeakGraph<BarAlignment>,
    staffs: &[AlignmentStaff],
    key: StaffPeakKey,
    parameters: SplitParameters,
) -> Result<Vec<(VerticalSide, Vec<StaffPeakKey>)>, SplitBuildError> {
    let peak = graph
        .vertex(key)
        .ok_or(SplitBuildError::Graph(PeakGraphError::MissingVertex(key)))?;
    if group_of(staffs, &[key], parameters.maximum_close_gap)?.len() > 1 {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    for side in [VerticalSide::Top, VerticalSide::Bottom] {
        let connected = graph
            .connected_peaks(key, side)
            .map_err(SplitBuildError::Graph)?
            .into_iter()
            .map(StaffPeak::key)
            .collect::<Vec<_>>();
        let partners = group_of(staffs, &connected, parameters.maximum_close_gap)?;
        if partners.len() != 2 {
            continue;
        }
        let first = graph
            .vertex(partners[0])
            .expect("projector peak is in graph");
        let last = graph
            .vertex(partners[1])
            .expect("projector peak is in graph");
        let maximum_partner_width = first.width().max(last.width());
        if peak.width() <= maximum_partner_width.wrapping_add(2) {
            continue;
        }
        let gap = last.start().wrapping_sub(first.stop()).wrapping_add(1);
        if gap > parameters.maximum_close_gap {
            continue;
        }
        let total = first.width().wrapping_add(last.width());
        let span = last.stop().wrapping_sub(first.start()).wrapping_add(1);
        let total_ratio = f64::from(total.wrapping_sub(peak.width()).wrapping_abs())
            / f64::from(total.max(peak.width()));
        let span_ratio = f64::from(span.wrapping_sub(peak.width()).wrapping_abs())
            / f64::from(span.max(peak.width()));
        if total_ratio.min(span_ratio) > parameters.maximum_width_ratio {
            continue;
        }
        let ratio = f64::from(first.width()) / f64::from(first.width().wrapping_add(last.width()));
        let mid = peak
            .start()
            .wrapping_add(java_rint_to_i32(f64::from(peak.width()) * ratio));
        if mid <= peak.start().wrapping_add(1) || mid >= peak.stop().wrapping_sub(1) {
            continue;
        }
        result.push((side, partners));
    }
    Ok(result)
}

fn group_of(
    staffs: &[AlignmentStaff],
    peaks: &[StaffPeakKey],
    maximum_close_gap: i32,
) -> Result<Vec<StaffPeakKey>, SplitBuildError> {
    let Some(&first) = peaks.first() else {
        return Ok(Vec::new());
    };
    let staff = staffs
        .iter()
        .find(|staff| staff.staff_id == first.staff_id())
        .ok_or(SplitBuildError::MissingProjector(first))?;
    let Some(first_index) = staff.peaks.iter().position(|&peak| peak == first) else {
        return Ok(Vec::new());
    };
    let mut minimum = first_index;
    let mut previous = first;
    for index in (0..first_index).rev() {
        let peak = staff.peaks[index];
        let gap = previous.start().wrapping_sub(peak.stop()).wrapping_add(1);
        if gap > maximum_close_gap {
            break;
        }
        minimum = index;
        previous = peak;
    }
    let last = *peaks.last().expect("nonempty peak group");
    let Some(last_index) = staff.peaks.iter().position(|&peak| peak == last) else {
        return Ok(Vec::new());
    };
    let mut maximum = last_index;
    // Preserve Java's current source behavior: `previous` is not reset to
    // `last` after the left scan before checking the right side.
    for index in (last_index + 1)..staff.peaks.len() {
        let peak = staff.peaks[index];
        let gap = peak.start().wrapping_sub(previous.stop()).wrapping_add(1);
        if gap > maximum_close_gap {
            break;
        }
        maximum = index;
        previous = peak;
    }
    Ok(staff.peaks[minimum..=maximum].to_vec())
}

#[allow(clippy::too_many_arguments)]
fn split_peak(
    graph: &mut PeakGraph<BarAlignment>,
    staffs: &mut [AlignmentStaff],
    key: StaffPeakKey,
    parameters: SplitParameters,
    build_filament: &mut impl FnMut(StaffPeakKey, &StaffPeak) -> Result<bool, String>,
    rediscover: &mut impl FnMut(
        &mut PeakGraph<BarAlignment>,
        &[AlignmentStaff],
        StaffPeakKey,
    ) -> Result<(), String>,
    deskew: &mut impl FnMut(PeakPoint) -> PeakPoint,
    impacted: &mut Vec<StaffPeakKey>,
    report: &mut SplitBuildReport,
) -> Result<bool, SplitBuildError> {
    let partners = split_partners(graph, staffs, key, parameters)?;
    if partners.is_empty() {
        return Ok(false);
    }
    let peak = graph
        .vertex(key)
        .expect("split candidate remains in graph")
        .clone();
    let mut split_ratio = None;
    for (_, group) in &partners {
        for &partner in group {
            insert_unique(impacted, partner);
        }
        let first = graph.vertex(group[0]).expect("partner remains in graph");
        let last = graph.vertex(group[1]).expect("partner remains in graph");
        let ratio = f64::from(first.width()) / f64::from(first.width().wrapping_add(last.width()));
        split_ratio = Some(match split_ratio {
            None => ratio,
            Some(previous) => (previous + ratio) / 2.0,
        });
    }
    impacted.retain(|&candidate| candidate != key);
    let mid = peak.start().wrapping_add(java_rint_to_i32(
        f64::from(peak.width()) * split_ratio.expect("candidate has partner ratio"),
    ));
    let impacts = peak.impacts().ok_or(SplitBuildError::MissingImpacts(key))?;
    let left = StaffPeak::with_impacts(
        peak.staff_id(),
        peak.top(),
        peak.bottom(),
        peak.start(),
        mid.wrapping_sub(1),
        impacts,
    )
    .map_err(SplitBuildError::Peak)?;
    let right = StaffPeak::with_impacts(
        peak.staff_id(),
        peak.top(),
        peak.bottom(),
        mid.wrapping_add(1),
        peak.stop(),
        impacts,
    )
    .map_err(SplitBuildError::Peak)?;
    if !build_filament(key, &left).map_err(SplitBuildError::Filament)? {
        return Ok(false);
    }
    if !build_filament(key, &right).map_err(SplitBuildError::Filament)? {
        report.failed_after_left_registration.push(key);
        return Ok(false);
    }
    let mut replacements = [left, right];
    for replacement in &mut replacements {
        replacement
            .compute_deskewed_center(&mut *deskew)
            .map_err(SplitBuildError::Peak)?;
        let replacement_key = replacement.key();
        if !graph.add_vertex(replacement.clone()) {
            return Err(SplitBuildError::DuplicateReplacement(replacement_key));
        }
        insert_before(staffs, replacement_key, key)?;
        rediscover(graph, staffs, replacement_key).map_err(SplitBuildError::Rediscovery)?;
        insert_unique(impacted, replacement_key);
        let incident = graph
            .edges_of(replacement_key)
            .map_err(SplitBuildError::Graph)?
            .map(|edge| {
                if edge.target() == replacement_key {
                    edge.source()
                } else {
                    edge.target()
                }
            })
            .collect::<Vec<_>>();
        for endpoint in incident {
            insert_unique(impacted, endpoint);
        }
    }
    let new_keys = [replacements[0].key(), replacements[1].key()];
    for (side, group) in partners {
        match side {
            VerticalSide::Top => prune_group_pair(graph, &group, &new_keys)?,
            VerticalSide::Bottom => prune_group_pair(graph, &new_keys, &group)?,
        }
    }
    remove_projector_peak(staffs, key)?;
    graph.remove_vertex(key);
    report.splits.push(SplitEvent {
        original: key,
        left: new_keys[0],
        right: new_keys[1],
    });
    Ok(true)
}

fn insert_before(
    staffs: &mut [AlignmentStaff],
    replacement: StaffPeakKey,
    old: StaffPeakKey,
) -> Result<(), SplitBuildError> {
    let staff = staffs
        .iter_mut()
        .find(|staff| staff.staff_id == old.staff_id())
        .ok_or(SplitBuildError::MissingProjector(old))?;
    let index = staff
        .peaks
        .iter()
        .position(|&peak| peak == old)
        .ok_or(SplitBuildError::MissingProjector(old))?;
    staff.peaks.insert(index, replacement);
    Ok(())
}

fn remove_projector_peak(
    staffs: &mut [AlignmentStaff],
    old: StaffPeakKey,
) -> Result<(), SplitBuildError> {
    let staff = staffs
        .iter_mut()
        .find(|staff| staff.staff_id == old.staff_id())
        .ok_or(SplitBuildError::MissingProjector(old))?;
    let index = staff
        .peaks
        .iter()
        .position(|&peak| peak == old)
        .ok_or(SplitBuildError::MissingProjector(old))?;
    staff.peaks.remove(index);
    Ok(())
}

fn prune_group_pair(
    graph: &mut PeakGraph<BarAlignment>,
    top: &[StaffPeakKey],
    bottom: &[StaffPeakKey],
) -> Result<(), SplitBuildError> {
    for index in 0..top.len() {
        let outgoing = graph
            .outgoing_edges(top[index])
            .map_err(SplitBuildError::Graph)?
            .map(|edge| (edge.id(), edge.target()))
            .collect::<Vec<_>>();
        for (edge, target) in outgoing {
            if target != bottom[index] {
                graph.remove_edge(edge);
            }
        }
        let incoming = graph
            .incoming_edges(bottom[index])
            .map_err(SplitBuildError::Graph)?
            .map(|edge| (edge.id(), edge.source()))
            .collect::<Vec<_>>();
        for (edge, source) in incoming {
            if source != top[index] {
                graph.remove_edge(edge);
            }
        }
    }
    Ok(())
}

fn insert_unique(values: &mut Vec<StaffPeakKey>, value: StaffPeakKey) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn java_rint_to_i32(value: f64) -> i32 {
    value.round_ties_even() as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        bar_alignment::{AlignmentPeak, BarImpacts},
        bar_column::{PeakId, StaffId},
        staff_peak::StaffVerticalImpacts,
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

    fn connection(
        top: &StaffPeak,
        bottom: &StaffPeak,
        top_id: usize,
        bottom_id: usize,
    ) -> BarAlignment {
        let relation_peak = |peak: &StaffPeak, id| {
            AlignmentPeak::with_geometry(
                PeakId::new(id),
                peak.staff_id(),
                peak.start(),
                peak.width() as usize,
                peak.top(),
                peak.bottom(),
                peak.impacts().unwrap().grade(),
            )
            .unwrap()
        };
        let alignment = BarAlignment::new(
            relation_peak(top, top_id),
            relation_peak(bottom, bottom_id),
            0.0,
            0.0,
            BarImpacts::alignment(1.0, 1.0).unwrap(),
        )
        .unwrap();
        BarAlignment::connection(&alignment, 1.0, 1.0).unwrap()
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

    #[test]
    fn split_inserts_replacements_at_graph_tail_and_projector_slot_then_purges() {
        let first = peak(1, 0, 4, 10, 11);
        let second = peak(1, 0, 4, 14, 15);
        let merged = peak(2, 10, 14, 10, 15);
        let mut graph = PeakGraph::new();
        graph.add_vertex(first.clone());
        graph.add_vertex(second.clone());
        graph.add_vertex(merged.clone());
        graph
            .add_edge(first.key(), merged.key(), connection(&first, &merged, 1, 3))
            .unwrap();
        graph
            .add_edge(
                second.key(),
                merged.key(),
                connection(&second, &merged, 2, 3),
            )
            .unwrap();
        let mut staffs = [
            staff(1, 0.0, 4.0, &[first.key(), second.key()]),
            staff(2, 10.0, 14.0, &[merged.key()]),
        ];
        let mut built = Vec::new();
        let mut report = SplitBuildReport::default();
        split_merged_groups_and_purge(
            &mut graph,
            &mut staffs,
            SplitParameters {
                maximum_close_gap: 4,
                maximum_width_ratio: 0.0,
            },
            |_, replacement| {
                built.push(replacement.key());
                Ok(true)
            },
            |graph, _, replacement| {
                let top = if replacement.start() == 10 {
                    first.key()
                } else {
                    second.key()
                };
                let top_peak = graph.vertex(top).unwrap().clone();
                let replacement_peak = graph.vertex(replacement).unwrap().clone();
                graph
                    .add_edge(
                        top,
                        replacement,
                        connection(&top_peak, &replacement_peak, 1, 4),
                    )
                    .unwrap();
                Ok(())
            },
            |point| point,
            &mut report,
        )
        .unwrap();

        let left = peak(2, 10, 14, 10, 12).key();
        let right = peak(2, 10, 14, 14, 15).key();
        assert_eq!(built, vec![left, right]);
        assert_eq!(staffs[1].peaks, vec![left, right]);
        assert_eq!(
            graph
                .vertices()
                .iter()
                .map(StaffPeak::key)
                .collect::<Vec<_>>(),
            vec![first.key(), second.key(), left, right]
        );
        assert_eq!(
            report.splits,
            vec![SplitEvent {
                original: merged.key(),
                left,
                right
            }]
        );
        assert!(report.purged_alignments.is_empty());
        assert_eq!(graph.edges().len(), 2);
    }

    #[test]
    fn right_failure_keeps_old_peak_and_observes_left_registration_prefix() {
        let first = peak(1, 0, 4, 10, 11);
        let second = peak(1, 0, 4, 14, 15);
        let merged = peak(2, 10, 14, 10, 15);
        let mut graph = PeakGraph::new();
        graph.add_vertex(first.clone());
        graph.add_vertex(second.clone());
        graph.add_vertex(merged.clone());
        graph
            .add_edge(first.key(), merged.key(), connection(&first, &merged, 1, 3))
            .unwrap();
        graph
            .add_edge(
                second.key(),
                merged.key(),
                connection(&second, &merged, 2, 3),
            )
            .unwrap();
        let mut staffs = [
            staff(1, 0.0, 4.0, &[first.key(), second.key()]),
            staff(2, 10.0, 14.0, &[merged.key()]),
        ];
        let mut calls = 0;
        let mut report = SplitBuildReport::default();
        split_merged_groups_and_purge(
            &mut graph,
            &mut staffs,
            SplitParameters {
                maximum_close_gap: 4,
                maximum_width_ratio: 0.0,
            },
            |_, _| {
                calls += 1;
                Ok(calls == 1)
            },
            |_, _, _| Ok(()),
            |point| point,
            &mut report,
        )
        .unwrap();
        assert_eq!(calls, 2);
        assert!(graph.contains_vertex(merged.key()));
        assert_eq!(staffs[1].peaks, vec![merged.key()]);
        assert!(report.splits.is_empty());
        assert_eq!(report.failed_after_left_registration, vec![merged.key()]);
        assert_eq!(report.purged_alignments.len(), 1);
    }
}
