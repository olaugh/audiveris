// SPDX-License-Identifier: AGPL-3.0-or-later

//! Dependency-light decisions from Java `BarsRetriever`.

use std::{error::Error, fmt};

use crate::{
    bar_alignment::{BarAlignment, BarAlignmentKind, VerticalSide},
    bar_column::{BarColumn, BarColumnError, BarPeak, PeakRelation, StaffId},
    part_group::PartGroup,
    peak_graph::{PeakEdgeId, PeakGraph, PeakGraphError},
    run_table::Orientation,
    section::Section,
    staff_peak::{HorizontalSide, PeakBounds, StaffPeak, StaffPeakAttribute, StaffPeakKey},
};

/// Java `BracketKind`; `None` from [`bracket_kind`] remains distinct from
/// `Some(BracketKind::None)` for a middle-only bracket portion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BracketKind {
    None,
    Top,
    Bottom,
    Both,
}

/// Classify the end flags carried by a bracket peak.
#[must_use]
pub fn bracket_kind(peak: &StaffPeak) -> Option<BracketKind> {
    if peak.is_set(StaffPeakAttribute::BracketMiddle) {
        return Some(BracketKind::None);
    }
    match (
        peak.is_bracket_end(VerticalSide::Top),
        peak.is_bracket_end(VerticalSide::Bottom),
    ) {
        (true, true) => Some(BracketKind::Both),
        (true, false) => Some(BracketKind::Top),
        (false, true) => Some(BracketKind::Bottom),
        (false, false) => None,
    }
}

/// Java `BarsRetriever.detectBracketMiddles` over one system's projector peaks.
///
/// Concrete outgoing connections are visited in graph insertion order. A
/// bottom-end destination abandons the current top-end traversal immediately,
/// while any destinations marked earlier in that same edge iteration remain
/// middle portions, matching Java's labeled `continue` behavior.
pub fn detect_bracket_middles(
    graph: &mut PeakGraph<BarAlignment>,
    projector_peaks: &[StaffPeakKey],
) -> Result<Vec<StaffPeakKey>, BarsLogicError> {
    for &key in projector_peaks {
        if graph.vertex(key).is_none() {
            return Err(PeakGraphError::MissingVertex(key).into());
        }
    }

    let mut marked = Vec::new();
    let mut marked_set = std::collections::HashSet::new();
    for &root in projector_peaks {
        if !graph
            .vertex(root)
            .expect("projector peaks were validated above")
            .is_bracket_end(VerticalSide::Top)
        {
            continue;
        }

        let mut current = root;
        let mut traversed = std::collections::HashSet::new();
        loop {
            if !traversed.insert(current) {
                return Err(BarsLogicError::BracketConnectionCycle(current));
            }
            let destinations = graph
                .outgoing_edges(current)?
                .filter(|edge| edge.relation().kind() == BarAlignmentKind::Connection)
                .map(|edge| edge.target())
                .collect::<Vec<_>>();
            let mut next = None;
            let mut reached_bottom = false;
            for destination in destinations {
                next = Some(destination);
                if graph
                    .vertex(destination)
                    .expect("graph edges reference present vertices")
                    .is_bracket_end(VerticalSide::Bottom)
                {
                    reached_bottom = true;
                    break;
                }
                if marked_set.insert(destination) {
                    marked.push(destination);
                }
            }
            if reached_bottom {
                break;
            }
            let Some(destination) = next else {
                break;
            };
            current = destination;
        }
    }

    for &key in &marked {
        graph
            .vertex_mut(key)
            .expect("marked destinations remain graph vertices")
            .set(StaffPeakAttribute::BracketMiddle);
    }
    Ok(marked)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BracketEndDetection {
    pub peak: StaffPeakKey,
    pub side: VerticalSide,
}

/// Java `BarsRetriever.detectBracketEnds` for one staff projector.
///
/// The caller supplies the geometric extension and serif lookup because those
/// depend on staff-line and filament state. Peaks are visited right-to-left,
/// stopping at the first brace, and top is tested before bottom.
pub fn detect_bracket_ends(
    peaks: &mut [StaffPeak],
    start_peak_index: Option<usize>,
    minimum_bracket_width: i32,
    maximum_bracket_extension: f64,
    mut extension_of: impl FnMut(StaffPeakKey, VerticalSide) -> f64,
    mut has_serif: impl FnMut(StaffPeakKey, StaffPeakKey, VerticalSide) -> bool,
) -> Result<Vec<BracketEndDetection>, BarsLogicError> {
    let Some(start_index) = start_peak_index else {
        return Ok(Vec::new());
    };
    if start_index >= peaks.len() {
        return Err(BarsLogicError::InvalidStartIndex(start_index));
    }

    let mut detections = Vec::new();
    for index in (0..start_index).rev() {
        if peaks[index].is_brace() {
            break;
        }
        if peaks[index].width() < minimum_bracket_width {
            continue;
        }
        let peak = peaks[index].key();
        let right = peaks[index + 1].key();
        for side in [VerticalSide::Top, VerticalSide::Bottom] {
            if extension_of(peak, side) <= maximum_bracket_extension && has_serif(peak, right, side)
            {
                peaks[index].set_bracket_end(side);
                detections.push(BracketEndDetection { peak, side });
            }
        }
    }
    Ok(detections)
}

/// Java `getGroups`: collect maximal adjacent peak runs separated by at most
/// `maximum_double_bar_gap`, omitting singleton runs.
///
/// Input order is preserved exactly; Java does not sort or validate it here.
/// Gap arithmetic uses wrapping `int` operations before comparison.
#[must_use]
pub fn peak_groups(peaks: &[StaffPeak], maximum_double_bar_gap: i32) -> Vec<Vec<StaffPeakKey>> {
    let mut groups = Vec::new();
    let mut current = Vec::new();
    let mut previous_stop = None;

    for peak in peaks {
        if let Some(stop) = previous_stop {
            let gap = peak.start().wrapping_sub(stop).wrapping_sub(1);
            if gap > maximum_double_bar_gap {
                if current.len() > 1 {
                    groups.push(std::mem::take(&mut current));
                } else {
                    current.clear();
                }
            }
        }
        current.push(peak.key());
        previous_stop = Some(peak.stop());
    }
    if current.len() > 1 {
        groups.push(current);
    }
    groups
}

/// Width class assigned by Java `BarsRetriever.partitionWidths`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeakWidthClass {
    Thin,
    Thick,
}

/// Stable peak-key result of Java `groupBarPeaks` plus `partitionWidths`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeakWidthAssignment {
    pub peak: StaffPeakKey,
    pub class: PeakWidthClass,
}

/// Neutral form of Java `BarGroupRelation` creation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarGroupLink {
    pub left: StaffPeakKey,
    pub right: StaffPeakKey,
    pub normalized_gap: f64,
}

/// Java `groupBarlines` adjacent-bar relation decisions for one staff.
///
/// Brace and bracket peaks are skipped without clearing the prior ordinary
/// bar, exactly matching the Java loop. Gap subtraction wraps as Java `int`
/// before conversion to the interline fraction.
#[must_use]
pub fn bar_group_links(
    peaks: &[StaffPeak],
    maximum_double_bar_gap: i32,
    interline: i32,
) -> Vec<BarGroupLink> {
    let mut previous: Option<&StaffPeak> = None;
    let mut links = Vec::new();
    for peak in peaks {
        if peak.is_brace() || peak.is_bracket() {
            continue;
        }
        if let Some(left) = previous {
            let gap = peak.start().wrapping_sub(left.stop()).wrapping_sub(1);
            if gap <= maximum_double_bar_gap {
                links.push(BarGroupLink {
                    left: left.key(),
                    right: peak.key(),
                    normalized_gap: f64::from(gap) / f64::from(interline),
                });
            }
        }
        previous = Some(peak);
    }
    links
}

/// Dispatch structural peaks, isolated bars, and adjacent bar groups, then
/// reproduce Java's within-group thin/thick width partition.
///
/// Each inner slice is one staff's already abscissa-ordered peak sequence.
/// Braces and brackets terminate a group and are not classified. Isolated
/// bars are always thin. A heterogeneous group splits at the midpoint, with
/// exact midpoint ties classified thin, as in Java.
#[must_use]
pub fn classify_bar_peak_widths(
    staff_peaks: &[Vec<StaffPeak>],
    maximum_double_bar_gap: i32,
    interline: i32,
    minimum_normalized_width_delta: f64,
) -> Vec<PeakWidthAssignment> {
    let mut isolated = Vec::new();
    let mut groups: Vec<Vec<&StaffPeak>> = Vec::new();

    for peaks in staff_peaks {
        let mut group_index = None;
        let mut previous: Option<&StaffPeak> = None;

        for peak in peaks {
            if peak.is_brace() || peak.is_bracket() {
                if group_index.is_none() {
                    if let Some(previous) = previous {
                        isolated.push(previous);
                    }
                }
                group_index = None;
                previous = None;
                continue;
            }

            if let Some(previous_peak) = previous {
                let gap = peak
                    .start()
                    .wrapping_sub(previous_peak.stop())
                    .wrapping_sub(1);
                if gap <= maximum_double_bar_gap {
                    let index = *group_index.get_or_insert_with(|| {
                        groups.push(vec![previous_peak]);
                        groups.len() - 1
                    });
                    groups[index].push(peak);
                } else if group_index.is_some() {
                    group_index = None;
                } else {
                    isolated.push(previous_peak);
                }
            }
            previous = Some(peak);
        }

        if group_index.is_none() {
            if let Some(previous) = previous {
                isolated.push(previous);
            }
        }
    }

    let mut assignments = isolated
        .into_iter()
        .map(|peak| PeakWidthAssignment {
            peak: peak.key(),
            class: PeakWidthClass::Thin,
        })
        .collect::<Vec<_>>();

    for group in groups {
        let min_width = group
            .iter()
            .map(|peak| peak.width())
            .min()
            .unwrap_or(i32::MAX);
        let max_width = group
            .iter()
            .map(|peak| peak.width())
            .max()
            .unwrap_or(i32::MIN);
        let delta = max_width.wrapping_sub(min_width);
        let heterogeneous =
            f64::from(delta) / f64::from(interline) >= minimum_normalized_width_delta;

        assignments.extend(group.into_iter().map(|peak| {
            let class = if heterogeneous
                && peak.width().wrapping_sub(min_width) > max_width.wrapping_sub(peak.width())
            {
                PeakWidthClass::Thick
            } else {
                PeakWidthClass::Thin
            };
            PeakWidthAssignment {
                peak: peak.key(),
                class,
            }
        }));
    }
    assignments
}

/// Java `purgeLeftPeaks` selection, stopping at the first peak strictly to the
/// right of the staff limit because callers provide abscissa-ordered peaks.
#[must_use]
pub fn peaks_before_staff_start(peaks: &[StaffPeak], staff_left: i32) -> Vec<StaffPeakKey> {
    peaks
        .iter()
        .take_while(|peak| peak.start() <= staff_left)
        .filter(|peak| {
            !peak.is_staff_end(HorizontalSide::Left) && !peak.is_brace() && !peak.is_bracket()
        })
        .map(StaffPeak::key)
        .collect()
}

/// Java `purgeTooLeft` selection in its right-to-left insertion order.
///
/// The start peak anchors the chain. A distant peak is removed without moving
/// the anchor; a nearby peak becomes the next anchor. Java wrapping `int`
/// arithmetic is retained for the gap expression.
pub fn peaks_too_far_left(
    peaks: &[StaffPeak],
    start_index: usize,
    maximum_brace_bar_gap: i32,
) -> Result<Vec<StaffPeakKey>, BarsLogicError> {
    let Some(mut previous) = peaks.get(start_index) else {
        return Err(BarsLogicError::InvalidStartIndex(start_index));
    };
    let mut removed = Vec::new();
    for peak in peaks[..start_index].iter().rev() {
        let gap = previous.start().wrapping_sub(peak.stop()).wrapping_add(1);
        if gap > maximum_brace_bar_gap {
            removed.push(peak.key());
        } else {
            previous = peak;
        }
    }
    Ok(removed)
}

/// Java `purgeLeftOfBraces` selection in right-to-left insertion order.
#[must_use]
pub fn peaks_left_of_brace(
    peaks: &[StaffPeak],
    start_index: Option<usize>,
    brace_start: i32,
) -> Vec<StaffPeakKey> {
    let Some(start_index) = start_index.filter(|&index| index <= peaks.len()) else {
        return Vec::new();
    };
    peaks[..start_index]
        .iter()
        .rev()
        .filter(|peak| peak.stop() < brace_start)
        .map(StaffPeak::key)
        .collect()
}

/// First Java `BarColumn.isStart()` index, or `None` for Java's `-1`.
#[must_use]
pub fn start_column_index(columns: &[BarColumn]) -> Option<usize> {
    columns.iter().position(BarColumn::is_start)
}

/// Original column indices removed by Java `purgePartialColumns`.
///
/// Java represents a missing start column as `-1`, so in that case every
/// partial column is eligible. Columns through the first start column are
/// retained even when partial.
#[must_use]
pub fn partial_column_indices_after_start(columns: &[BarColumn]) -> Vec<usize> {
    let first_eligible = start_column_index(columns).map_or(0, |index| index + 1);
    columns
        .iter()
        .enumerate()
        .skip(first_eligible)
        .filter_map(|(index, column)| (!column.is_full()).then_some(index))
        .collect()
}

/// Java `detectStartColumns` first-phase candidate selection.
///
/// This covers the column scan before staff-line and blank validation. The
/// right-most fully connected column in the initial close group wins. Java's
/// wider brace-to-bar gap applies only when the current absolute column index
/// is one, not to the second full column encountered.
pub fn start_column_candidate(
    columns: &mut [BarColumn],
    relations: &[PeakRelation],
    maximum_brace_bar_gap: i32,
    maximum_double_bar_gap: i32,
) -> Option<usize> {
    let mut candidate: Option<usize> = None;
    for index in 0..columns.len() {
        if !columns[index].is_full() {
            continue;
        }
        if let Some(previous_index) = candidate {
            let current_left = columns[index].deskewed_x() - columns[index].mean_width() / 2.0;
            let previous_right =
                columns[previous_index].deskewed_x() + columns[previous_index].mean_width() / 2.0;
            let maximum_gap = if index == 1 {
                maximum_brace_bar_gap
            } else {
                maximum_double_bar_gap
            };
            if current_left - previous_right > f64::from(maximum_gap) {
                break;
            }
        }
        if columns[index].is_fully_connected(relations) {
            candidate = Some(index);
        }
    }
    candidate
}

#[derive(Clone, Copy, Debug)]
pub struct StartColumnStaffFacts<'a> {
    pub peak: &'a StaffPeak,
    pub staff_left: i32,
    pub is_one_line_staff: bool,
    pub has_standard_blank_to_lines: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StartColumnUpdate {
    pub staff_id: StaffId,
    pub staff_left: i32,
    pub peak: StaffPeakKey,
}

/// Final all-or-nothing validation and mutation intents from Java
/// `detectStartColumns` after a candidate column has been selected.
/// One-line staves skip both line-root checks but still receive the accepted
/// column position and left-end marker.
#[must_use]
pub fn validate_start_column(
    candidate: &[StartColumnStaffFacts<'_>],
    maximum_lines_left_to_start_bar: i32,
) -> Option<Vec<StartColumnUpdate>> {
    for facts in candidate {
        if facts.is_one_line_staff {
            continue;
        }
        if facts.peak.start().wrapping_sub(facts.staff_left) > maximum_lines_left_to_start_bar
            || facts.has_standard_blank_to_lines
        {
            return None;
        }
    }
    Some(
        candidate
            .iter()
            .map(|facts| StartColumnUpdate {
                staff_id: facts.peak.staff_id(),
                staff_left: facts.peak.stop(),
                peak: facts.peak.key(),
            })
            .collect(),
    )
}

/// Java `BarsRetriever.buildColumns` aggregation after graph connectivity has
/// produced peak chains for one system.
///
/// Chains must contain peaks in Java `StaffPeak` order. They are stably sorted
/// by the first peak's deskewed abscissa, then merged only into the latest
/// column when both the abscissa and empty-staff-slot tests pass.
pub fn aggregate_bar_chains(
    staff_ids: &[StaffId],
    chains: &[Vec<BarPeak>],
    maximum_column_dx: i32,
) -> Result<Vec<BarColumn>, BarsLogicError> {
    let mut ordered = chains.iter().collect::<Vec<_>>();
    if ordered.iter().any(|chain| chain.is_empty()) {
        return Err(BarsLogicError::EmptyChain);
    }
    ordered.sort_by(|one, two| one[0].deskewed_x().total_cmp(&two[0].deskewed_x()));

    let mut columns: Vec<BarColumn> = Vec::new();
    for chain in ordered {
        let include_last = if let Some(last) = columns.last_mut() {
            let dx = chain[0].deskewed_x() - last.deskewed_x();
            dx.abs() <= f64::from(maximum_column_dx) && last.can_include(chain)?
        } else {
            false
        };
        if !include_last {
            columns.push(BarColumn::new(staff_ids.to_vec())?);
        }
        columns
            .last_mut()
            .expect("column just created or present")
            .add_chain(chain)?;
    }
    Ok(columns)
}

/// Convert neutral graph components into Java `BarColumn.Chain` scalar facts.
/// Peak IDs are their one-based successful graph insertion positions, which
/// remain stable for this immutable build-columns snapshot.
pub fn graph_bar_chains<R>(graph: &PeakGraph<R>) -> Result<Vec<Vec<BarPeak>>, BarsLogicError> {
    let id_by_key = graph
        .vertices()
        .iter()
        .enumerate()
        .map(|(index, peak)| {
            let value = index
                .checked_add(1)
                .ok_or(BarsLogicError::PeakIdExhausted)?;
            Ok((peak.key(), crate::bar_column::PeakId::new(value)))
        })
        .collect::<Result<Vec<_>, BarsLogicError>>()?;

    graph
        .connected_peak_chains()
        .into_iter()
        .map(|chain| {
            chain
                .into_iter()
                .map(|peak| {
                    let id = id_by_key
                        .iter()
                        .find_map(|(key, id)| (*key == peak.key()).then_some(*id))
                        .expect("graph component peak has an insertion ID");
                    let deskewed = peak
                        .deskewed_center()
                        .ok_or(BarsLogicError::MissingDeskewedCenter(peak.key()))?;
                    BarPeak::new(
                        id,
                        peak.staff_id(),
                        f64::from(peak.width()),
                        deskewed.x,
                        peak.is_brace(),
                        peak.is_staff_end(HorizontalSide::Left),
                    )
                    .map_err(BarsLogicError::from)
                })
                .collect()
        })
        .collect()
}

/// Complete dependency-light `buildColumns` path from neutral peak graph to
/// ordered columns for one system.
pub fn build_bar_columns_from_graph<R>(
    graph: &PeakGraph<R>,
    staff_ids: &[StaffId],
    maximum_column_dx: i32,
) -> Result<Vec<BarColumn>, BarsLogicError> {
    let chains = graph_bar_chains(graph)?;
    aggregate_bar_chains(staff_ids, &chains, maximum_column_dx)
}

/// Java `purgeUnalignedBars` selection for one staff.
///
/// Only multi-staff systems run this purge. Projector peaks absent from the
/// graph are intentionally retained; a present vertex is removed only when it
/// has no incoming or outgoing alignment/connection edge.
#[must_use]
pub fn unaligned_peak_keys<R>(
    peaks: &[StaffPeak],
    graph: &PeakGraph<R>,
    is_multi_staff_system: bool,
) -> Vec<StaffPeakKey> {
    if !is_multi_staff_system {
        return Vec::new();
    }
    peaks
        .iter()
        .filter(|peak| {
            graph.contains_vertex(peak.key())
                && graph
                    .edges_of(peak.key())
                    .expect("contains-vertex check makes edges_of valid")
                    .next()
                    .is_none()
        })
        .map(StaffPeak::key)
        .collect()
}

/// Java `BarsRetriever.isTrueBraceGroup` once directional part-connection
/// queries have been resolved by the caller.
#[must_use]
pub fn is_true_brace_group(
    group: &PartGroup,
    first_connected_above: bool,
    last_connected_below: bool,
    first_connected_below: bool,
    allow_disconnected_braced_parts: bool,
) -> bool {
    group.is_brace()
        && group
            .last_staff_id()
            .wrapping_sub(group.first_staff_id())
            .wrapping_add(1)
            == 2
        && !first_connected_above
        && !last_connected_below
        && (first_connected_below || allow_disconnected_braced_parts)
}

/// Java `BarsRetriever.getConnections` edge selection for one staff side.
/// The first bar is excluded, leaving only within-part connections. Edge order
/// is significant: Java stops once the opposite-side peak belongs to a later
/// staff, relying on `PeakGraph`'s established staff/abscissa order.
#[must_use]
pub fn part_connection_edge_ids(
    graph: &PeakGraph<BarAlignment>,
    staff_id: StaffId,
    side: VerticalSide,
    start_peak: Option<&StaffPeak>,
) -> Vec<PeakEdgeId> {
    let Some(start_peak) = start_peak else {
        return Vec::new();
    };
    let opposite = match side {
        VerticalSide::Top => VerticalSide::Bottom,
        VerticalSide::Bottom => VerticalSide::Top,
    };
    let mut selected = Vec::new();
    for edge in graph.edges() {
        if edge.relation().kind() != BarAlignmentKind::Connection {
            continue;
        }
        let peak = edge.relation().peak(opposite);
        if peak.staff_id() == staff_id {
            if peak.start() > start_peak.start() {
                selected.push(edge.id());
            }
        } else if peak.staff_id().value() > staff_id.value() {
            break;
        }
    }
    selected
}

#[derive(Clone, Copy, Debug)]
pub struct BraceGroupDecision<'a> {
    pub group_index: usize,
    pub group: &'a PartGroup,
    pub is_true_brace: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlannedPart {
    pub first_staff_id: i32,
    pub last_staff_id: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartsPlan {
    pub parts: Vec<PlannedPart>,
    pub removed_group_indices: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroupPeakFacts {
    pub brace: bool,
    pub bracket: bool,
    pub bracket_top: bool,
    pub bracket_bottom: bool,
    pub brace_top: bool,
    pub brace_bottom: bool,
    pub connected_top: bool,
    pub connected_bottom: bool,
}

impl GroupPeakFacts {
    #[must_use]
    pub const fn plain(connected_top: bool, connected_bottom: bool) -> Self {
        Self {
            brace: false,
            bracket: false,
            bracket_top: false,
            bracket_bottom: false,
            brace_top: false,
            brace_bottom: false,
            connected_top,
            connected_bottom,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GroupStaffFacts<'a> {
    pub staff_id: i32,
    pub peaks: &'a [GroupPeakFacts],
    pub start_peak_index: Option<usize>,
    pub brace_peak: Option<GroupPeakFacts>,
    pub part_connected_below: bool,
}

/// Java `BarsRetriever.createGroups` state machine without Sheet/SIG cycles.
/// Active group levels are shared across successive staves; overwritten
/// unfinished groups deliberately remain in the output list.
pub fn create_part_groups(
    staves: &[GroupStaffFacts<'_>],
) -> Result<Vec<PartGroup>, BarsLogicError> {
    use std::collections::BTreeMap;

    let mut groups = Vec::new();
    let mut active: BTreeMap<i32, usize> = BTreeMap::new();
    for staff in staves {
        let Some(start_index) = staff.start_peak_index else {
            continue;
        };
        if start_index >= staff.peaks.len() {
            return Err(BarsLogicError::InvalidGroupStartIndex {
                staff_id: staff.staff_id,
                index: start_index,
            });
        }

        let mut level = 0_i32;
        for peak in staff.peaks[..start_index].iter().rev() {
            if peak.brace {
                break;
            }
            level = level.wrapping_add(1);
            if peak.bracket {
                if peak.bracket_top {
                    let index = groups.len();
                    groups.push(PartGroup::new(
                        level,
                        crate::part_group::PartGroupingSymbol::Bracket,
                        staff.part_connected_below,
                        staff.staff_id,
                    ));
                    active.insert(level, index);
                } else if let Some(&index) = active.get(&level) {
                    groups[index].set_last_staff_id(staff.staff_id);
                    if peak.bracket_bottom {
                        active.remove(&level);
                    }
                }
            } else if !peak.connected_top && peak.connected_bottom {
                let index = groups.len();
                groups.push(PartGroup::new(
                    level,
                    crate::part_group::PartGroupingSymbol::Square,
                    staff.part_connected_below,
                    staff.staff_id,
                ));
                active.insert(level, index);
            } else if peak.connected_top {
                if let Some(&index) = active.get(&level) {
                    groups[index].set_last_staff_id(staff.staff_id);
                    if !peak.connected_bottom {
                        active.remove(&level);
                    }
                }
            }
        }

        if let Some(brace) = staff.brace_peak {
            level = level.wrapping_add(1);
            if brace.brace_top {
                let index = groups.len();
                groups.push(PartGroup::new(
                    level,
                    crate::part_group::PartGroupingSymbol::Brace,
                    staff.part_connected_below,
                    staff.staff_id,
                ));
                active.insert(level, index);
            } else if let Some(&index) = active.get(&level) {
                groups[index].set_last_staff_id(staff.staff_id);
                if brace.brace_bottom {
                    active.remove(&level);
                }
            }
        }
    }
    Ok(groups)
}

/// Dependency-light plan for Java `BarsRetriever.createParts`.
///
/// True brace groups use Java's `TreeSet(PartGroup.byFirstId)`: ordering and
/// uniqueness depend only on first staff ID, so the first input with a shared
/// first ID survives. `createPart`'s overlap guard is preserved by truncating a
/// later range after the latest assigned staff or dropping it entirely.
pub fn plan_parts(
    system_first_staff_id: i32,
    system_last_staff_id: i32,
    groups: &[BraceGroupDecision<'_>],
    force_separate_parts: bool,
    force_single_part: bool,
) -> Result<PartsPlan, BarsLogicError> {
    if system_first_staff_id > system_last_staff_id {
        return Err(BarsLogicError::InvalidStaffRange {
            first: system_first_staff_id,
            last: system_last_staff_id,
        });
    }

    let mut braces = if force_separate_parts {
        Vec::new()
    } else {
        groups
            .iter()
            .copied()
            .filter(|decision| decision.is_true_brace)
            .collect::<Vec<_>>()
    };
    braces.sort_by_key(|decision| decision.group.first_staff_id());
    braces.dedup_by_key(|decision| decision.group.first_staff_id());

    let mut plan = PartsPlan {
        parts: Vec::new(),
        removed_group_indices: braces.iter().map(|decision| decision.group_index).collect(),
    };
    let mut latest_last = None;
    let add_part =
        |first: i32, last: i32, parts: &mut Vec<PlannedPart>, latest_last: &mut Option<i32>| {
            if latest_last.is_some_and(|latest| last <= latest) {
                return;
            }
            let first = latest_last.map_or(first, |latest| latest.saturating_add(1).max(first));
            if first <= last {
                parts.push(PlannedPart {
                    first_staff_id: first,
                    last_staff_id: last,
                });
                *latest_last = Some(last);
            }
        };

    let mut next_id = system_first_staff_id;
    for decision in &braces {
        let first = decision.group.first_staff_id();
        let last = decision.group.last_staff_id();
        while next_id < first {
            add_part(next_id, next_id, &mut plan.parts, &mut latest_last);
            next_id = next_id
                .checked_add(1)
                .ok_or(BarsLogicError::StaffIdExhausted)?;
        }
        add_part(first, last, &mut plan.parts, &mut latest_last);
        next_id = last
            .checked_add(1)
            .ok_or(BarsLogicError::StaffIdExhausted)?;
    }

    let staff_count = system_last_staff_id
        .checked_sub(system_first_staff_id)
        .and_then(|delta| delta.checked_add(1))
        .ok_or(BarsLogicError::StaffIdExhausted)?;
    if force_single_part && staff_count == 2 && braces.is_empty() {
        add_part(
            system_first_staff_id,
            system_last_staff_id,
            &mut plan.parts,
            &mut latest_last,
        );
    } else {
        while next_id <= system_last_staff_id {
            add_part(next_id, next_id, &mut plan.parts, &mut latest_last);
            if next_id == system_last_staff_id {
                break;
            }
            next_id = next_id
                .checked_add(1)
                .ok_or(BarsLogicError::StaffIdExhausted)?;
        }
    }
    Ok(plan)
}

/// Java `extensionOf`: signed filament extension beyond a staff limit.
#[must_use]
pub fn peak_extension(
    peak: &StaffPeak,
    filament_bounds: PeakBounds,
    maximum_foreground_thickness: i32,
    side: VerticalSide,
) -> f64 {
    let half_line = f64::from(maximum_foreground_thickness) / 2.0;
    match side {
        VerticalSide::Top => f64::from(peak.top()) - half_line - f64::from(filament_bounds.y),
        VerticalSide::Bottom => {
            let last_y = filament_bounds
                .y
                .wrapping_add(filament_bounds.height)
                .wrapping_sub(1);
            f64::from(last_y) - half_line - f64::from(peak.bottom())
        }
    }
}

/// Peak keys selected by Java `purgeExtendingPeaks` for one boundary staff.
/// A missing start marker makes every peak eligible, matching `iStart == -1`.
#[must_use]
pub fn extending_peak_keys(
    peaks_and_filaments: &[(StaffPeak, PeakBounds)],
    start_index: Option<usize>,
    maximum_foreground_thickness: i32,
    maximum_bar_extension: f64,
    side: VerticalSide,
) -> Vec<StaffPeakKey> {
    let first_eligible = start_index.map_or(0, |index| index.saturating_add(1));
    peaks_and_filaments
        .iter()
        .skip(first_eligible)
        .filter_map(|(peak, bounds)| {
            (peak_extension(peak, *bounds, maximum_foreground_thickness, side)
                > maximum_bar_extension)
                .then_some(peak.key())
        })
        .collect()
}

/// Java `BarFilamentBuilder.buildFilament` section preselection.
///
/// The caller supplies sections in nondecreasing x order. Intersection uses
/// Java `Rectangle`'s positive-area rule; a section touching the peak's right
/// edge triggers the same early break. Both horizontal and vertical sections
/// are accepted when their horizontal length does not exceed the peak width.
#[must_use]
pub fn bar_filament_section_ids(
    peak_bounds: PeakBounds,
    vertical_extension: i32,
    sections: &[Section],
) -> Vec<usize> {
    if peak_bounds.width <= 0 {
        return Vec::new();
    }
    let left = i128::from(peak_bounds.x);
    let right = left + i128::from(peak_bounds.width);
    let top = i128::from(peak_bounds.y) - i128::from(vertical_extension);
    let bottom =
        i128::from(peak_bounds.y) + i128::from(peak_bounds.height) + i128::from(vertical_extension);
    if right <= left || bottom <= top {
        return Vec::new();
    }

    let mut selected = Vec::new();
    for section in sections {
        let bounds = section.bounds();
        let section_left = bounds.x as i128;
        let section_right = section_left + bounds.width as i128;
        let section_top = bounds.y as i128;
        let section_bottom = section_top + bounds.height as i128;
        let intersects = section_left < right
            && section_right > left
            && section_top < bottom
            && section_bottom > top;
        if intersects {
            if section.length(Orientation::Horizontal) <= peak_bounds.width as usize {
                selected.push(section.id());
            }
        } else if section_left >= right {
            break;
        }
    }
    selected
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionLag {
    Vertical,
    Horizontal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocatedSectionId {
    pub lag: SectionLag,
    pub id: usize,
}

/// Java `BarsRetriever.getSectionsByWidth`: filter VLAG then HLAG and perform
/// a stable x-only sort. Equal-x sections therefore retain lag and entity order.
#[must_use]
pub fn sections_by_width(
    vertical_sections: &[Section],
    horizontal_sections: &[Section],
    maximum_width: i32,
) -> Vec<LocatedSectionId> {
    if maximum_width < 0 {
        return Vec::new();
    }
    let maximum_width = maximum_width as usize;
    let mut selected = vertical_sections
        .iter()
        .filter(|section| section.length(Orientation::Horizontal) <= maximum_width)
        .map(|section| (section.bounds().x, SectionLag::Vertical, section.id()))
        .chain(
            horizontal_sections
                .iter()
                .filter(|section| section.length(Orientation::Horizontal) <= maximum_width)
                .map(|section| (section.bounds().x, SectionLag::Horizontal, section.id())),
        )
        .collect::<Vec<_>>();
    selected.sort_by_key(|(x, _, _)| *x);
    selected
        .into_iter()
        .map(|(_, lag, id)| LocatedSectionId { lag, id })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CClefDetection {
    pub first: StaffPeakKey,
    pub second: StaffPeakKey,
    pub tails: Vec<StaffPeakKey>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CClefPurgeResult {
    pub survivors: Vec<StaffPeakKey>,
    pub detections: Vec<CClefDetection>,
}

/// Java `BarsRetriever.purgeCClefs` over one staff's mutable peak sequence.
/// Connection state is supplied by the caller to keep graph ownership outside.
/// The unusual post-removal jump and selective measure-start updates are exact.
#[allow(clippy::too_many_arguments)]
pub fn purge_c_clef_peaks(
    peaks: &[StaffPeak],
    staff_start: i32,
    minimum_first_peak_width: i32,
    maximum_second_peak_width: i32,
    maximum_double_bar_gap: i32,
    minimum_measure_width: i32,
    c_clef_tail: i32,
    mut is_connected: impl FnMut(StaffPeakKey, VerticalSide) -> bool,
) -> CClefPurgeResult {
    let mut remaining = peaks.to_vec();
    let mut detections = Vec::new();
    let mut measure_start = staff_start;
    let mut index = 0_usize;

    while index < remaining.len() {
        let first = &remaining[index];
        if first.start() <= measure_start {
            index += 1;
            continue;
        }
        let first_shape = !first.is_staff_end(HorizontalSide::Left)
            && !first.is_staff_end(HorizontalSide::Right)
            && !first.is_brace()
            && !first.is_bracket()
            && first.width() >= minimum_first_peak_width;
        if first_shape {
            let gap = first.start().wrapping_sub(measure_start);
            let minimum_gap = if measure_start == staff_start {
                0
            } else {
                maximum_double_bar_gap
            };
            let first_key = first.key();
            let first_location = gap > minimum_gap
                && gap < minimum_measure_width
                && !is_connected(first_key, VerticalSide::Top)
                && !is_connected(first_key, VerticalSide::Bottom);
            if first_location && index + 1 < remaining.len() {
                let second = &remaining[index + 1];
                let second_key = second.key();
                let second_gap = second.start().wrapping_sub(first.stop()).wrapping_sub(1);
                if second.width() <= maximum_second_peak_width
                    && second_gap <= maximum_double_bar_gap
                    && !is_connected(second_key, VerticalSide::Top)
                    && !is_connected(second_key, VerticalSide::Bottom)
                {
                    let second_midpoint = second.start().wrapping_add(second.stop()) / 2;
                    let x_break = second_midpoint.wrapping_add(c_clef_tail);
                    let mut cancelled = false;
                    let mut tail_count = 0_usize;
                    for tail in remaining.iter().skip(index + 2) {
                        let midpoint = tail.start().wrapping_add(tail.stop()) / 2;
                        if midpoint >= x_break {
                            break;
                        }
                        if is_connected(tail.key(), VerticalSide::Top)
                            || is_connected(tail.key(), VerticalSide::Bottom)
                        {
                            cancelled = true;
                            break;
                        }
                        tail_count += 1;
                    }
                    if !cancelled {
                        let tails = remaining[index + 2..index + 2 + tail_count]
                            .iter()
                            .map(StaffPeak::key)
                            .collect::<Vec<_>>();
                        detections.push(CClefDetection {
                            first: first_key,
                            second: second_key,
                            tails,
                        });
                        remaining.drain(index..index + 2 + tail_count);
                        // Java then executes `i += 1 + tails.size()` and the
                        // for-loop increment, on the already shortened list.
                        index = index.saturating_add(2 + tail_count);
                        continue;
                    }
                }
            }
            if !first_location {
                measure_start = first.stop().wrapping_add(1);
            }
        } else {
            measure_start = first.stop().wrapping_add(1);
        }
        index += 1;
    }
    CClefPurgeResult {
        survivors: remaining.iter().map(StaffPeak::key).collect(),
        detections,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarsLogicError {
    InvalidStartIndex(usize),
    EmptyChain,
    Column(BarColumnError),
    MissingDeskewedCenter(StaffPeakKey),
    PeakIdExhausted,
    InvalidStaffRange { first: i32, last: i32 },
    StaffIdExhausted,
    InvalidGroupStartIndex { staff_id: i32, index: usize },
    Graph(PeakGraphError),
    BracketConnectionCycle(StaffPeakKey),
}

impl From<BarColumnError> for BarsLogicError {
    fn from(value: BarColumnError) -> Self {
        Self::Column(value)
    }
}

impl From<PeakGraphError> for BarsLogicError {
    fn from(value: PeakGraphError) -> Self {
        Self::Graph(value)
    }
}

impl fmt::Display for BarsLogicError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidStartIndex(index) => write!(formatter, "invalid start peak index {index}"),
            Self::EmptyChain => formatter.write_str("bar peak chain must not be empty"),
            Self::Column(error) => write!(formatter, "bar column error: {error}"),
            Self::MissingDeskewedCenter(key) => write!(
                formatter,
                "peak on staff {} at {}-{} has no deskewed center",
                key.staff_id().value(),
                key.start(),
                key.stop()
            ),
            Self::PeakIdExhausted => formatter.write_str("bar peak insertion ID exhausted"),
            Self::InvalidStaffRange { first, last } => {
                write!(formatter, "invalid staff range {first}..={last}")
            }
            Self::StaffIdExhausted => formatter.write_str("staff ID arithmetic exhausted"),
            Self::InvalidGroupStartIndex { staff_id, index } => {
                write!(
                    formatter,
                    "invalid group start index {index} for staff {staff_id}"
                )
            }
            Self::Graph(error) => write!(formatter, "peak graph error: {error}"),
            Self::BracketConnectionCycle(key) => {
                write!(formatter, "cyclic bracket connection at peak {key:?}")
            }
        }
    }
}

impl Error for BarsLogicError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Column(error) => Some(error),
            Self::Graph(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bar_column::{BarPeak, PeakId, StaffId};
    use crate::{
        run_table::{Orientation, Run, RunTable},
        section::{JunctionPolicy, build_sections},
    };

    fn peak(staff: usize, start: i32, stop: i32) -> StaffPeak {
        StaffPeak::new(StaffId::new(staff), 0, 40, start, stop).unwrap()
    }

    fn graph_relation(
        id: usize,
        top: StaffPeakKey,
        bottom: StaffPeakKey,
        kind: BarAlignmentKind,
    ) -> BarAlignment {
        use crate::bar_alignment::{AlignmentPeak, BarImpacts};

        let alignment = BarAlignment::new(
            AlignmentPeak::new(PeakId::new(id), top.staff_id(), top.start(), 0.9).unwrap(),
            AlignmentPeak::new(
                PeakId::new(id + 100),
                bottom.staff_id(),
                bottom.start(),
                0.9,
            )
            .unwrap(),
            0.0,
            0.0,
            BarImpacts::alignment(0.9, 0.9).unwrap(),
        )
        .unwrap();
        match kind {
            BarAlignmentKind::Alignment => alignment,
            BarAlignmentKind::Connection => BarAlignment::connection(&alignment, 0.9, 0.9).unwrap(),
        }
    }

    #[test]
    fn bracket_kind_distinguishes_absence_middle_and_end_combinations() {
        let mut value = peak(1, 4, 5);
        assert_eq!(bracket_kind(&value), None);

        value.set(StaffPeakAttribute::BracketMiddle);
        value.set_bracket_end(VerticalSide::Top);
        assert_eq!(bracket_kind(&value), Some(BracketKind::None));

        value.unset(StaffPeakAttribute::BracketMiddle);
        assert_eq!(bracket_kind(&value), Some(BracketKind::Top));
        value.set_bracket_end(VerticalSide::Bottom);
        assert_eq!(bracket_kind(&value), Some(BracketKind::Both));
        value.unset(StaffPeakAttribute::BracketTop);
        assert_eq!(bracket_kind(&value), Some(BracketKind::Bottom));
    }

    #[test]
    fn bracket_middle_detection_marks_connection_chain_until_bottom_end() {
        let mut top = peak(1, 5, 8);
        top.set_bracket_end(VerticalSide::Top);
        let middle = peak(2, 5, 8);
        let mut bottom = peak(3, 5, 8);
        bottom.set_bracket_end(VerticalSide::Bottom);
        let keys = [top.key(), middle.key(), bottom.key()];
        let mut graph = PeakGraph::new();
        for value in [top, middle, bottom] {
            graph.add_vertex(value);
        }
        graph
            .add_edge(
                keys[0],
                keys[1],
                graph_relation(1, keys[0], keys[1], BarAlignmentKind::Connection),
            )
            .unwrap();
        graph
            .add_edge(
                keys[1],
                keys[2],
                graph_relation(2, keys[1], keys[2], BarAlignmentKind::Connection),
            )
            .unwrap();

        assert_eq!(
            detect_bracket_middles(&mut graph, &keys).unwrap(),
            [keys[1]]
        );
        assert!(
            graph
                .vertex(keys[1])
                .unwrap()
                .is_set(StaffPeakAttribute::BracketMiddle)
        );
        assert!(
            !graph
                .vertex(keys[2])
                .unwrap()
                .is_set(StaffPeakAttribute::BracketMiddle)
        );
    }

    #[test]
    fn bracket_middle_detection_preserves_marks_before_bottom_branch_abort() {
        let mut top = peak(1, 5, 8);
        top.set_bracket_end(VerticalSide::Top);
        let first_branch = peak(2, 5, 8);
        let mut bottom_branch = peak(2, 12, 15);
        bottom_branch.set_bracket_end(VerticalSide::Bottom);
        let ignored_alignment = peak(2, 20, 23);
        let keys = [
            top.key(),
            first_branch.key(),
            bottom_branch.key(),
            ignored_alignment.key(),
        ];
        let mut graph = PeakGraph::new();
        for value in [top, first_branch, bottom_branch, ignored_alignment] {
            graph.add_vertex(value);
        }
        for (index, (target, kind)) in [
            (keys[1], BarAlignmentKind::Connection),
            (keys[2], BarAlignmentKind::Connection),
            (keys[3], BarAlignmentKind::Alignment),
        ]
        .into_iter()
        .enumerate()
        {
            graph
                .add_edge(
                    keys[0],
                    target,
                    graph_relation(index + 1, keys[0], target, kind),
                )
                .unwrap();
        }

        assert_eq!(
            detect_bracket_middles(&mut graph, &keys).unwrap(),
            [keys[1]]
        );
        assert!(graph.vertex(keys[1]).unwrap().is_bracket());
        assert!(!graph.vertex(keys[3]).unwrap().is_bracket());
    }

    #[test]
    fn bracket_middle_detection_rejects_a_connection_cycle_atomically() {
        let mut top = peak(1, 5, 8);
        top.set_bracket_end(VerticalSide::Top);
        let middle = peak(2, 5, 8);
        let keys = [top.key(), middle.key()];
        let mut graph = PeakGraph::new();
        for value in [top, middle] {
            graph.add_vertex(value);
        }
        graph
            .add_edge(
                keys[0],
                keys[1],
                graph_relation(1, keys[0], keys[1], BarAlignmentKind::Connection),
            )
            .unwrap();
        graph
            .add_edge(
                keys[1],
                keys[0],
                graph_relation(2, keys[1], keys[0], BarAlignmentKind::Connection),
            )
            .unwrap();

        assert_eq!(
            detect_bracket_middles(&mut graph, &keys),
            Err(BarsLogicError::BracketConnectionCycle(keys[0]))
        );
        assert!(!graph.vertex(keys[1]).unwrap().is_bracket());
    }

    #[test]
    fn bracket_end_detection_walks_backwards_stops_at_brace_and_uses_right_neighbor() {
        use std::cell::RefCell;

        let hidden_left = peak(1, 0, 4);
        let mut brace = peak(1, 6, 9);
        brace.set(StaffPeakAttribute::Brace);
        let candidate = peak(1, 11, 14);
        let thin = peak(1, 16, 16);
        let start = peak(1, 20, 21);
        let candidate_key = candidate.key();
        let thin_key = thin.key();
        let start_key = start.key();
        let calls = RefCell::new(Vec::new());
        let mut peaks = [hidden_left, brace, candidate, thin, start];

        let detections = detect_bracket_ends(
            &mut peaks,
            Some(4),
            4,
            2.0,
            |key, side| {
                calls.borrow_mut().push(("extension", key, key, side));
                match side {
                    VerticalSide::Top => 2.0,
                    VerticalSide::Bottom => 2.5,
                }
            },
            |key, right, side| {
                calls.borrow_mut().push(("serif", key, right, side));
                side == VerticalSide::Top
            },
        )
        .unwrap();

        assert_eq!(
            detections,
            [BracketEndDetection {
                peak: candidate_key,
                side: VerticalSide::Top,
            }]
        );
        assert_eq!(
            calls.into_inner(),
            [
                ("extension", candidate_key, candidate_key, VerticalSide::Top),
                ("serif", candidate_key, thin_key, VerticalSide::Top),
                (
                    "extension",
                    candidate_key,
                    candidate_key,
                    VerticalSide::Bottom,
                ),
            ]
        );
        assert!(peaks[2].is_bracket_end(VerticalSide::Top));
        assert!(!peaks[2].is_bracket_end(VerticalSide::Bottom));
        assert!(!peaks[0].is_bracket());
        assert_eq!(peaks[4].key(), start_key);
    }

    #[test]
    fn bracket_end_detection_skips_missing_start_and_rejects_invalid_index() {
        let mut values = [peak(1, 5, 8)];
        assert!(
            detect_bracket_ends(
                &mut values,
                None,
                1,
                1.0,
                |_, _| { panic!("no start peak must bypass extension checks") },
                |_, _, _| false
            )
            .unwrap()
            .is_empty()
        );
        assert_eq!(
            detect_bracket_ends(&mut values, Some(1), 1, 1.0, |_, _| 0.0, |_, _, _| true),
            Err(BarsLogicError::InvalidStartIndex(1))
        );
        assert!(!values[0].is_bracket());
    }

    #[test]
    fn peak_groups_preserve_order_include_exact_gap_and_omit_singletons() {
        let peaks = [
            peak(1, 0, 1),
            peak(1, 4, 5),   // gap 2: included at the boundary
            peak(1, 10, 10), // gap 4: closes the first group, singleton omitted
            peak(1, 20, 20),
            peak(1, 22, 23),
            peak(1, 25, 25),
        ];

        let groups = peak_groups(&peaks, 2);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0], [peaks[0].key(), peaks[1].key()]);
        assert_eq!(groups[1], [peaks[3].key(), peaks[4].key(), peaks[5].key()]);
    }

    #[test]
    fn peak_groups_reproduce_java_wrapping_gap_arithmetic() {
        let peaks = [peak(1, 0, i32::MIN), peak(1, i32::MAX, i32::MAX)];
        // MAX - MIN - 1 wraps to -2, so Java keeps the pair even at maxGap -1.
        assert_eq!(peak_groups(&peaks, -1).len(), 1);
    }

    #[test]
    fn bar_width_partition_matches_group_boundaries_and_midpoint_ties() {
        let isolated = peak(1, 0, 0);
        let narrow = peak(1, 10, 11); // width 2
        let midpoint = peak(1, 13, 16); // width 4, tied around [2, 6]
        let wide = peak(1, 18, 23); // width 6
        let after_group = peak(1, 40, 42);
        let mut brace = peak(1, 45, 45);
        brace.set(StaffPeakAttribute::Brace);
        let after_brace = peak(1, 47, 51);
        let staves = vec![vec![
            isolated.clone(),
            narrow.clone(),
            midpoint.clone(),
            wide.clone(),
            after_group.clone(),
            brace,
            after_brace.clone(),
        ]];

        let result = classify_bar_peak_widths(&staves, 2, 10, 0.3);
        assert_eq!(
            result,
            [
                PeakWidthAssignment {
                    peak: isolated.key(),
                    class: PeakWidthClass::Thin
                },
                PeakWidthAssignment {
                    peak: after_group.key(),
                    class: PeakWidthClass::Thin
                },
                PeakWidthAssignment {
                    peak: after_brace.key(),
                    class: PeakWidthClass::Thin
                },
                PeakWidthAssignment {
                    peak: narrow.key(),
                    class: PeakWidthClass::Thin
                },
                PeakWidthAssignment {
                    peak: midpoint.key(),
                    class: PeakWidthClass::Thin
                },
                PeakWidthAssignment {
                    peak: wide.key(),
                    class: PeakWidthClass::Thick
                },
            ]
        );
    }

    #[test]
    fn bar_width_partition_keeps_homogeneous_groups_thin() {
        let first = peak(1, 0, 1);
        let second = peak(1, 3, 5);
        let staves = vec![vec![first.clone(), second.clone()]];
        let result = classify_bar_peak_widths(&staves, 1, 10, 0.2);
        assert_eq!(
            result,
            [
                PeakWidthAssignment {
                    peak: first.key(),
                    class: PeakWidthClass::Thin
                },
                PeakWidthAssignment {
                    peak: second.key(),
                    class: PeakWidthClass::Thin
                },
            ]
        );
    }

    #[test]
    fn bar_group_links_skip_structural_peaks_without_resetting_previous_bar() {
        let left = peak(1, 0, 1);
        let mut brace = peak(1, 2, 2);
        brace.set(StaffPeakAttribute::Brace);
        let right = peak(1, 4, 4);
        let far = peak(1, 10, 10);
        let links = bar_group_links(&[left.clone(), brace, right.clone(), far], 2, 4);
        assert_eq!(
            links,
            [BarGroupLink {
                left: left.key(),
                right: right.key(),
                normalized_gap: 0.5,
            }]
        );
    }

    #[test]
    fn bar_group_links_preserve_java_wrapping_and_zero_interline() {
        let left = peak(1, 0, i32::MIN);
        let right = peak(1, i32::MAX, i32::MAX);
        let links = bar_group_links(&[left, right], -1, 0);
        assert_eq!(links.len(), 1);
        assert!(links[0].normalized_gap.is_sign_negative());
        assert!(links[0].normalized_gap.is_infinite());
    }

    #[test]
    fn partial_column_purge_starts_strictly_after_first_start_column() {
        fn column(first: Option<BarPeak>, second: Option<BarPeak>) -> BarColumn {
            let mut column = BarColumn::new(vec![StaffId::new(1), StaffId::new(2)]).unwrap();
            if let Some(peak) = first {
                column.add_peak(peak).unwrap();
            }
            if let Some(peak) = second {
                column.add_peak(peak).unwrap();
            }
            column
        }
        fn bar(id: usize, staff: usize, start: bool) -> BarPeak {
            BarPeak::new(
                PeakId::new(id),
                StaffId::new(staff),
                2.0,
                id as f64,
                false,
                start,
            )
            .unwrap()
        }

        let columns = vec![
            column(Some(bar(1, 1, false)), None),
            column(Some(bar(2, 1, true)), None),
            column(Some(bar(3, 1, false)), Some(bar(4, 2, false))),
            column(None, Some(bar(5, 2, false))),
        ];
        assert_eq!(partial_column_indices_after_start(&columns), [3]);

        let no_start = vec![
            column(Some(bar(6, 1, false)), None),
            column(Some(bar(7, 1, false)), Some(bar(8, 2, false))),
        ];
        assert_eq!(partial_column_indices_after_start(&no_start), [0]);
    }

    #[test]
    fn extending_peak_purge_is_side_specific_and_strictly_after_start() {
        let first = peak(1, 0, 1);
        let start = peak(1, 3, 4);
        let top_long = peak(1, 6, 7);
        let bottom_long = peak(1, 9, 10);
        let facts = vec![
            (
                first,
                PeakBounds {
                    x: 0,
                    y: -20,
                    width: 2,
                    height: 61,
                },
            ),
            (
                start,
                PeakBounds {
                    x: 3,
                    y: -20,
                    width: 2,
                    height: 61,
                },
            ),
            (
                top_long.clone(),
                PeakBounds {
                    x: 6,
                    y: -7,
                    width: 2,
                    height: 48,
                },
            ),
            (
                bottom_long.clone(),
                PeakBounds {
                    x: 9,
                    y: 0,
                    width: 2,
                    height: 48,
                },
            ),
        ];

        assert_eq!(
            extending_peak_keys(&facts, Some(1), 2, 5.0, VerticalSide::Top),
            [top_long.key()]
        );
        assert_eq!(
            extending_peak_keys(&facts, Some(1), 2, 5.0, VerticalSide::Bottom),
            [bottom_long.key()]
        );
        assert_eq!(
            extending_peak_keys(&facts, None, 2, 5.0, VerticalSide::Top),
            [facts[0].0.key(), facts[1].0.key(), top_long.key()]
        );
    }

    #[test]
    fn start_column_candidate_uses_absolute_second_column_brace_gap() {
        fn full_column(base_id: usize, x: f64) -> BarColumn {
            let mut column = BarColumn::new(vec![StaffId::new(1), StaffId::new(2)]).unwrap();
            column
                .add_peak(
                    BarPeak::new(PeakId::new(base_id), StaffId::new(1), 2.0, x, false, false)
                        .unwrap(),
                )
                .unwrap();
            column
                .add_peak(
                    BarPeak::new(
                        PeakId::new(base_id + 1),
                        StaffId::new(2),
                        2.0,
                        x,
                        false,
                        false,
                    )
                    .unwrap(),
                )
                .unwrap();
            column
        }
        let mut columns = vec![
            full_column(1, 10.0),
            full_column(3, 20.0),
            full_column(5, 30.0),
        ];
        let relations = [
            crate::bar_column::PeakRelation::connection(PeakId::new(1), PeakId::new(2)),
            crate::bar_column::PeakRelation::connection(PeakId::new(3), PeakId::new(4)),
            crate::bar_column::PeakRelation::connection(PeakId::new(5), PeakId::new(6)),
        ];
        // Edge gaps are 8. Column index 1 uses the wider brace gap; index 2
        // then exceeds the ordinary double-bar gap and stops the scan.
        assert_eq!(
            start_column_candidate(&mut columns, &relations, 8, 7),
            Some(1)
        );
    }

    #[test]
    fn start_column_candidate_ignores_partial_and_tracks_last_connected_full() {
        fn column(base_id: usize, x: f64, second: bool) -> BarColumn {
            let mut column = BarColumn::new(vec![StaffId::new(1), StaffId::new(2)]).unwrap();
            column
                .add_peak(
                    BarPeak::new(PeakId::new(base_id), StaffId::new(1), 2.0, x, false, false)
                        .unwrap(),
                )
                .unwrap();
            if second {
                column
                    .add_peak(
                        BarPeak::new(
                            PeakId::new(base_id + 1),
                            StaffId::new(2),
                            2.0,
                            x,
                            false,
                            false,
                        )
                        .unwrap(),
                    )
                    .unwrap();
            }
            column
        }
        let mut columns = vec![
            column(1, 2.0, false),
            column(3, 10.0, true),
            column(5, 14.0, true),
        ];
        let relations = [crate::bar_column::PeakRelation::connection(
            PeakId::new(5),
            PeakId::new(6),
        )];
        assert_eq!(
            start_column_candidate(&mut columns, &relations, 10, 10),
            Some(2)
        );
    }

    #[test]
    fn start_column_validation_is_atomic_and_one_line_staves_skip_line_checks() {
        let top = peak(1, 12, 14);
        let bottom = peak(2, 13, 15);
        let facts = [
            StartColumnStaffFacts {
                peak: &top,
                staff_left: 10,
                is_one_line_staff: false,
                has_standard_blank_to_lines: false,
            },
            StartColumnStaffFacts {
                peak: &bottom,
                staff_left: -100,
                is_one_line_staff: true,
                has_standard_blank_to_lines: true,
            },
        ];
        assert_eq!(
            validate_start_column(&facts, 2).unwrap(),
            [
                StartColumnUpdate {
                    staff_id: StaffId::new(1),
                    staff_left: 14,
                    peak: top.key()
                },
                StartColumnUpdate {
                    staff_id: StaffId::new(2),
                    staff_left: 15,
                    peak: bottom.key()
                },
            ]
        );

        let rejected = [
            facts[0],
            StartColumnStaffFacts {
                peak: &bottom,
                staff_left: 12,
                is_one_line_staff: false,
                has_standard_blank_to_lines: true,
            },
        ];
        assert!(validate_start_column(&rejected, 2).is_none());
    }

    #[test]
    fn start_column_validation_uses_java_wrapping_distance() {
        let value = peak(1, i32::MIN, i32::MIN);
        let facts = [StartColumnStaffFacts {
            peak: &value,
            staff_left: i32::MAX,
            is_one_line_staff: false,
            has_standard_blank_to_lines: false,
        }];
        // MIN - MAX wraps to 1.
        assert!(validate_start_column(&facts, 1).is_some());
    }

    #[test]
    fn connected_chains_sort_and_aggregate_only_into_latest_compatible_column() {
        fn bar(id: usize, staff: usize, x: f64) -> BarPeak {
            BarPeak::new(PeakId::new(id), StaffId::new(staff), 2.0, x, false, false).unwrap()
        }
        let chains = vec![
            vec![bar(3, 2, 10.5)],
            vec![bar(4, 1, 20.0)],
            vec![bar(1, 1, 10.0)],
            vec![bar(5, 2, 20.5)],
            vec![bar(6, 2, 21.0)], // occupied staff slot forces a third column
        ];
        let columns =
            aggregate_bar_chains(&[StaffId::new(1), StaffId::new(2)], &chains, 2).unwrap();
        assert_eq!(columns.len(), 3);
        assert_eq!(
            columns[0]
                .peaks()
                .iter()
                .map(|peak| peak.map(BarPeak::id))
                .collect::<Vec<_>>(),
            [Some(PeakId::new(1)), Some(PeakId::new(3))]
        );
        assert_eq!(
            columns[1]
                .peaks()
                .iter()
                .map(|peak| peak.map(BarPeak::id))
                .collect::<Vec<_>>(),
            [Some(PeakId::new(4)), Some(PeakId::new(5))]
        );
        assert_eq!(columns[2].peaks()[1].map(BarPeak::id), Some(PeakId::new(6)));
    }

    #[test]
    fn connected_chain_aggregation_rejects_empty_chain_and_exact_dx_is_inclusive() {
        let empty = vec![Vec::new()];
        assert_eq!(
            aggregate_bar_chains(&[StaffId::new(1)], &empty, 2).unwrap_err(),
            BarsLogicError::EmptyChain
        );
        let chains = vec![
            vec![BarPeak::new(PeakId::new(1), StaffId::new(1), 1.0, 0.0, false, false).unwrap()],
            vec![BarPeak::new(PeakId::new(2), StaffId::new(2), 1.0, 2.0, false, false).unwrap()],
        ];
        assert_eq!(
            aggregate_bar_chains(&[StaffId::new(1), StaffId::new(2)], &chains, 2)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn graph_components_convert_to_stable_bar_chains() {
        let mut top = peak(2, 10, 11);
        let mut bottom = peak(1, 12, 14);
        let mut isolated = peak(3, 20, 20);
        top.compute_deskewed_center(|point| {
            crate::staff_peak::PeakPoint::new(point.x + 0.25, point.y)
        })
        .unwrap();
        bottom
            .compute_deskewed_center(|point| {
                crate::staff_peak::PeakPoint::new(point.x + 0.25, point.y)
            })
            .unwrap();
        isolated
            .compute_deskewed_center(|point| crate::staff_peak::PeakPoint::new(point.x, point.y))
            .unwrap();
        top.set(StaffPeakAttribute::Brace);
        bottom.set_staff_end(HorizontalSide::Left);
        let top_key = top.key();
        let bottom_key = bottom.key();
        let mut graph = PeakGraph::new();
        graph.add_vertex(top);
        graph.add_vertex(bottom);
        graph.add_vertex(isolated);
        graph.add_edge(top_key, bottom_key, ()).unwrap();

        let chains = graph_bar_chains(&graph).unwrap();
        assert_eq!(chains.len(), 2);
        // Chain order is StaffPeak order, while scalar IDs retain graph insertion.
        assert_eq!(chains[0][0].id(), PeakId::new(2));
        assert_eq!(chains[0][1].id(), PeakId::new(1));
        assert!(chains[0][0].is_left_staff_end());
        assert!(chains[0][1].is_brace());
        assert_eq!(chains[1][0].id(), PeakId::new(3));
    }

    #[test]
    fn graph_chain_conversion_requires_deskewed_peak_geometry() {
        let mut graph = PeakGraph::<()>::new();
        let value = peak(1, 0, 1);
        let key = value.key();
        graph.add_vertex(value);
        assert_eq!(
            graph_bar_chains(&graph).unwrap_err(),
            BarsLogicError::MissingDeskewedCenter(key)
        );
    }

    #[test]
    fn graph_to_columns_composes_connectivity_conversion_and_aggregation() {
        let mut top_left = peak(1, 10, 11);
        let mut bottom_left = peak(2, 10, 11);
        let mut top_right = peak(1, 30, 31);
        let mut bottom_right = peak(2, 30, 31);
        for value in [&mut top_left, &mut bottom_left] {
            value
                .compute_deskewed_center(|point| crate::staff_peak::PeakPoint::new(10.0, point.y))
                .unwrap();
        }
        for value in [&mut top_right, &mut bottom_right] {
            value
                .compute_deskewed_center(|point| crate::staff_peak::PeakPoint::new(30.0, point.y))
                .unwrap();
        }
        let left_keys = [top_left.key(), bottom_left.key()];
        let right_keys = [top_right.key(), bottom_right.key()];
        let mut graph = PeakGraph::new();
        for value in [top_right, bottom_left, top_left, bottom_right] {
            graph.add_vertex(value);
        }
        graph.add_edge(left_keys[0], left_keys[1], ()).unwrap();
        graph.add_edge(right_keys[0], right_keys[1], ()).unwrap();
        let columns =
            build_bar_columns_from_graph(&graph, &[StaffId::new(1), StaffId::new(2)], 2).unwrap();
        assert_eq!(columns.len(), 2);
        assert!(columns.iter().all(BarColumn::is_full));
    }

    #[test]
    fn unaligned_purge_requires_multi_staff_present_isolated_vertex() {
        let connected = peak(1, 0, 1);
        let partner = peak(2, 0, 1);
        let isolated = peak(1, 10, 11);
        let absent = peak(1, 20, 21);
        let mut graph = PeakGraph::new();
        for value in [connected.clone(), partner.clone(), isolated.clone()] {
            graph.add_vertex(value);
        }
        graph.add_edge(connected.key(), partner.key(), ()).unwrap();
        let peaks = [connected, isolated.clone(), absent];
        assert!(unaligned_peak_keys(&peaks, &graph, false).is_empty());
        assert_eq!(unaligned_peak_keys(&peaks, &graph, true), [isolated.key()]);
    }

    #[test]
    fn true_brace_group_requires_exact_pair_and_no_external_connections() {
        use crate::part_group::PartGroupingSymbol;

        let mut pair = PartGroup::new(1, PartGroupingSymbol::Brace, true, 3);
        pair.set_last_staff_id(4);
        assert!(is_true_brace_group(&pair, false, false, true, false));
        assert!(is_true_brace_group(&pair, false, false, false, true));
        assert!(!is_true_brace_group(&pair, true, false, true, true));
        assert!(!is_true_brace_group(&pair, false, true, true, true));

        pair.set_last_staff_id(5);
        assert!(!is_true_brace_group(&pair, false, false, true, true));
        let mut bracket = PartGroup::new(1, PartGroupingSymbol::Bracket, true, 3);
        bracket.set_last_staff_id(4);
        assert!(!is_true_brace_group(&bracket, false, false, true, true));
    }

    #[test]
    fn part_connections_exclude_start_and_stop_at_later_opposite_staff() {
        use crate::bar_alignment::{AlignmentPeak, BarImpacts};

        fn connection(
            top_id: usize,
            top_staff: usize,
            bottom_id: usize,
            bottom_staff: usize,
            start: i32,
        ) -> BarAlignment {
            let top = AlignmentPeak::new(PeakId::new(top_id), StaffId::new(top_staff), start, 0.8)
                .unwrap();
            let bottom = AlignmentPeak::new(
                PeakId::new(bottom_id),
                StaffId::new(bottom_staff),
                start,
                0.8,
            )
            .unwrap();
            let alignment = BarAlignment::new(
                top,
                bottom,
                0.0,
                0.0,
                BarImpacts::alignment(0.9, 0.9).unwrap(),
            )
            .unwrap();
            BarAlignment::connection(&alignment, 0.9, 0.9).unwrap()
        }

        let start = peak(2, 10, 11);
        let upper_start = peak(1, 10, 11);
        let part = peak(2, 20, 21);
        let upper_part = peak(1, 20, 21);
        let later = peak(3, 30, 31);
        let later_top = peak(2, 30, 31);
        let ignored = peak(2, 40, 41);
        let ignored_top = peak(1, 40, 41);
        let keys = [
            upper_start.key(),
            start.key(),
            upper_part.key(),
            part.key(),
            later_top.key(),
            later.key(),
            ignored_top.key(),
            ignored.key(),
        ];
        let mut graph = PeakGraph::new();
        for value in [
            upper_start,
            start.clone(),
            upper_part,
            part,
            later_top,
            later,
            ignored_top,
            ignored,
        ] {
            graph.add_vertex(value);
        }
        graph
            .add_edge(keys[0], keys[1], connection(1, 1, 2, 2, 10))
            .unwrap();
        let expected = graph
            .add_edge(keys[2], keys[3], connection(3, 1, 4, 2, 20))
            .unwrap();
        graph
            .add_edge(keys[4], keys[5], connection(5, 2, 6, 3, 30))
            .unwrap();
        graph
            .add_edge(keys[6], keys[7], connection(7, 1, 8, 2, 40))
            .unwrap();

        assert_eq!(
            part_connection_edge_ids(&graph, StaffId::new(2), VerticalSide::Top, Some(&start),),
            [expected]
        );
        assert!(
            part_connection_edge_ids(&graph, StaffId::new(2), VerticalSide::Top, None,).is_empty()
        );
    }

    #[test]
    fn parts_plan_deduplicates_braces_and_truncates_overlapping_ranges() {
        use crate::part_group::PartGroupingSymbol;

        let mut first = PartGroup::new(1, PartGroupingSymbol::Brace, true, 2);
        first.set_last_staff_id(4);
        let mut duplicate_first = PartGroup::new(2, PartGroupingSymbol::Brace, true, 2);
        duplicate_first.set_last_staff_id(3);
        let mut overlap = PartGroup::new(3, PartGroupingSymbol::Brace, true, 4);
        overlap.set_last_staff_id(6);
        let groups = [
            BraceGroupDecision {
                group_index: 7,
                group: &first,
                is_true_brace: true,
            },
            BraceGroupDecision {
                group_index: 8,
                group: &duplicate_first,
                is_true_brace: true,
            },
            BraceGroupDecision {
                group_index: 9,
                group: &overlap,
                is_true_brace: true,
            },
        ];
        let plan = plan_parts(1, 7, &groups, false, false).unwrap();
        assert_eq!(
            plan.parts,
            [
                PlannedPart {
                    first_staff_id: 1,
                    last_staff_id: 1
                },
                PlannedPart {
                    first_staff_id: 2,
                    last_staff_id: 4
                },
                PlannedPart {
                    first_staff_id: 5,
                    last_staff_id: 6
                },
                PlannedPart {
                    first_staff_id: 7,
                    last_staff_id: 7
                },
            ]
        );
        assert_eq!(plan.removed_group_indices, [7, 9]);
    }

    #[test]
    fn parts_plan_preserves_force_switch_interaction() {
        use crate::part_group::PartGroupingSymbol;

        let mut brace = PartGroup::new(1, PartGroupingSymbol::Brace, true, 10);
        brace.set_last_staff_id(11);
        let groups = [BraceGroupDecision {
            group_index: 0,
            group: &brace,
            is_true_brace: true,
        }];
        let forced_separate = plan_parts(10, 11, &groups, true, false).unwrap();
        assert_eq!(
            forced_separate.parts,
            [
                PlannedPart {
                    first_staff_id: 10,
                    last_staff_id: 10
                },
                PlannedPart {
                    first_staff_id: 11,
                    last_staff_id: 11
                },
            ]
        );
        assert!(forced_separate.removed_group_indices.is_empty());

        let forced_single = plan_parts(10, 11, &groups, true, true).unwrap();
        assert_eq!(
            forced_single.parts,
            [PlannedPart {
                first_staff_id: 10,
                last_staff_id: 11
            }]
        );
        let brace_wins = plan_parts(10, 11, &groups, false, true).unwrap();
        assert_eq!(brace_wins.parts, forced_single.parts);
        assert_eq!(brace_wins.removed_group_indices, [0]);
    }

    #[test]
    fn group_state_machine_builds_bracket_square_and_brace_ranges() {
        let start = GroupPeakFacts::plain(false, false);
        let bracket_top = GroupPeakFacts {
            bracket: true,
            bracket_top: true,
            ..GroupPeakFacts::plain(false, false)
        };
        let square_top = GroupPeakFacts::plain(false, true);
        let brace_top = GroupPeakFacts {
            brace: true,
            brace_top: true,
            ..GroupPeakFacts::plain(false, false)
        };
        let bracket_bottom = GroupPeakFacts {
            bracket: true,
            bracket_bottom: true,
            ..GroupPeakFacts::plain(false, false)
        };
        let square_bottom = GroupPeakFacts::plain(true, false);
        let brace_bottom = GroupPeakFacts {
            brace: true,
            brace_bottom: true,
            ..GroupPeakFacts::plain(false, false)
        };
        // Reverse scan sees square at level 1, bracket at level 2.
        let first_peaks = [bracket_top, square_top, start];
        let second_peaks = [bracket_bottom, square_bottom, start];
        let groups = create_part_groups(&[
            GroupStaffFacts {
                staff_id: 1,
                peaks: &first_peaks,
                start_peak_index: Some(2),
                brace_peak: Some(brace_top),
                part_connected_below: true,
            },
            GroupStaffFacts {
                staff_id: 2,
                peaks: &second_peaks,
                start_peak_index: Some(2),
                brace_peak: Some(brace_bottom),
                part_connected_below: false,
            },
        ])
        .unwrap();
        assert_eq!(groups.len(), 3);
        assert_eq!(
            groups
                .iter()
                .map(|group| (
                    group.symbol(),
                    group.number(),
                    group.first_staff_id(),
                    group.last_staff_id(),
                    group.is_barline(),
                ))
                .collect::<Vec<_>>(),
            [
                (crate::part_group::PartGroupingSymbol::Square, 1, 1, 2, true),
                (
                    crate::part_group::PartGroupingSymbol::Bracket,
                    2,
                    1,
                    2,
                    true
                ),
                (crate::part_group::PartGroupingSymbol::Brace, 3, 1, 2, true),
            ]
        );
    }

    #[test]
    fn group_state_machine_skips_staff_without_start_and_keeps_overwritten_group() {
        let start = GroupPeakFacts::plain(false, false);
        let brace_top = GroupPeakFacts {
            brace: true,
            brace_top: true,
            ..GroupPeakFacts::plain(false, false)
        };
        let peaks = [start];
        let groups = create_part_groups(&[
            GroupStaffFacts {
                staff_id: 1,
                peaks: &peaks,
                start_peak_index: Some(0),
                brace_peak: Some(brace_top),
                part_connected_below: false,
            },
            GroupStaffFacts {
                staff_id: 2,
                peaks: &peaks,
                start_peak_index: None,
                brace_peak: Some(brace_top),
                part_connected_below: false,
            },
            GroupStaffFacts {
                staff_id: 3,
                peaks: &peaks,
                start_peak_index: Some(0),
                brace_peak: Some(brace_top),
                part_connected_below: false,
            },
        ])
        .unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(
            (groups[0].first_staff_id(), groups[0].last_staff_id()),
            (1, 1)
        );
        assert_eq!(
            (groups[1].first_staff_id(), groups[1].last_staff_id()),
            (3, 3)
        );
    }

    #[test]
    fn left_staff_purge_stops_in_order_and_preserves_structural_peaks() {
        let mut staff_end = peak(1, 2, 2);
        staff_end.set_staff_end(HorizontalSide::Left);
        let mut brace = peak(1, 3, 3);
        brace.set(StaffPeakAttribute::Brace);
        let mut bracket = peak(1, 4, 4);
        bracket.set(StaffPeakAttribute::BracketMiddle);
        let plain = peak(1, 5, 5);
        let later = peak(1, 10, 10);
        let out_of_order_left = peak(1, 1, 1);
        let peaks = [staff_end, brace, bracket, plain, later, out_of_order_left];

        assert_eq!(peaks_before_staff_start(&peaks, 5), [peaks[3].key()]);
    }

    #[test]
    fn too_left_purge_keeps_near_chain_anchor_and_returns_reverse_order() {
        let peaks = [peak(1, 0, 0), peak(1, 4, 4), peak(1, 8, 8), peak(1, 20, 20)];

        // From start x=20: x=8 is too far and does not replace the anchor;
        // x=4 and x=0 are therefore also too far, in right-to-left order.
        assert_eq!(
            peaks_too_far_left(&peaks, 3, 5).unwrap(),
            [peaks[2].key(), peaks[1].key(), peaks[0].key()]
        );

        let chained = [peak(1, 0, 0), peak(1, 4, 4), peak(1, 8, 8)];
        assert!(peaks_too_far_left(&chained, 2, 5).unwrap().is_empty());
        assert_eq!(
            peaks_too_far_left(&chained, 3, 5),
            Err(BarsLogicError::InvalidStartIndex(3))
        );
    }

    #[test]
    fn brace_purge_uses_start_prefix_and_returns_reverse_order() {
        let peaks = [peak(1, 0, 1), peak(1, 3, 4), peak(1, 7, 8), peak(1, 10, 11)];
        assert_eq!(
            peaks_left_of_brace(&peaks, Some(3), 7),
            [peaks[1].key(), peaks[0].key()]
        );
        assert!(peaks_left_of_brace(&peaks, None, 7).is_empty());
        assert!(peaks_left_of_brace(&peaks, Some(99), 7).is_empty());
    }

    #[test]
    fn start_column_lookup_returns_first_marked_column() {
        let staff_ids = vec![StaffId::new(1), StaffId::new(2)];
        let mut plain = BarColumn::new(staff_ids.clone()).unwrap();
        plain
            .add_peak(
                BarPeak::new(PeakId::new(1), StaffId::new(1), 2.0, 10.0, false, false).unwrap(),
            )
            .unwrap();
        let mut start = BarColumn::new(staff_ids).unwrap();
        start
            .add_peak(
                BarPeak::new(PeakId::new(2), StaffId::new(1), 2.0, 20.0, false, true).unwrap(),
            )
            .unwrap();

        assert_eq!(start_column_index(&[plain.clone(), start]), Some(1));
        assert_eq!(start_column_index(&[plain]), None);
    }

    #[test]
    fn bar_section_selection_grows_vertically_filters_width_and_breaks_at_right_edge() {
        let mut table = RunTable::new(Orientation::Horizontal, 30, 20).unwrap();
        table.add_run(4, Run::new(9, 2)).unwrap(); // intersects after grow
        table.add_run(6, Run::new(10, 4)).unwrap(); // too wide
        table.add_run(10, Run::new(11, 2)).unwrap(); // intersects after grow
        table.add_run(5, Run::new(13, 1)).unwrap(); // touches right edge
        table.add_run(5, Run::new(15, 1)).unwrap(); // never visited after break
        let mut sections = build_sections(&table, JunctionPolicy::All);
        sections.sort_by_key(|section| section.bounds().x);

        assert_eq!(
            bar_filament_section_ids(
                PeakBounds {
                    x: 10,
                    y: 5,
                    width: 3,
                    height: 5,
                },
                2,
                &sections,
            ),
            sections
                .iter()
                .filter(|section| matches!(section.bounds().x, 9 | 11))
                .map(Section::id)
                .collect::<Vec<_>>()
        );
        assert!(
            bar_filament_section_ids(
                PeakBounds {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 1
                },
                0,
                &sections
            )
            .is_empty()
        );
    }

    #[test]
    fn section_width_filter_preserves_vertical_then_horizontal_order_on_x_ties() {
        let mut vertical_table = RunTable::new(Orientation::Vertical, 20, 20).unwrap();
        vertical_table.add_run(5, Run::new(2, 3)).unwrap();
        vertical_table.add_run(9, Run::new(1, 5)).unwrap(); // horizontal length 1
        let vertical = build_sections(&vertical_table, JunctionPolicy::All);

        let mut horizontal_table = RunTable::new(Orientation::Horizontal, 20, 20).unwrap();
        horizontal_table.add_run(3, Run::new(5, 2)).unwrap();
        horizontal_table.add_run(8, Run::new(12, 5)).unwrap(); // filtered at max 3
        let horizontal = build_sections(&horizontal_table, JunctionPolicy::All);

        let selected = sections_by_width(&vertical, &horizontal, 3);
        let vertical_at_five = vertical
            .iter()
            .find(|section| section.bounds().x == 5)
            .unwrap();
        let horizontal_at_five = horizontal
            .iter()
            .find(|section| section.bounds().x == 5)
            .unwrap();
        assert_eq!(
            &selected[..2],
            [
                LocatedSectionId {
                    lag: SectionLag::Vertical,
                    id: vertical_at_five.id(),
                },
                LocatedSectionId {
                    lag: SectionLag::Horizontal,
                    id: horizontal_at_five.id(),
                },
            ]
        );
        assert!(selected.iter().all(|entry| {
            !(entry.lag == SectionLag::Horizontal
                && entry.id
                    == horizontal
                        .iter()
                        .find(|section| section.bounds().x == 12)
                        .unwrap()
                        .id())
        }));
        assert!(sections_by_width(&vertical, &horizontal, -1).is_empty());
    }

    #[test]
    fn c_clef_purge_detects_pair_and_unconnected_tails_with_java_index_jump() {
        let leading = peak(1, 0, 0);
        let first = peak(1, 5, 8);
        let second = peak(1, 10, 10);
        let tail = peak(1, 12, 12);
        let skipped_after_removal = peak(1, 30, 30);
        let final_peak = peak(1, 40, 40);

        let result = purge_c_clef_peaks(
            &[
                leading.clone(),
                first.clone(),
                second.clone(),
                tail.clone(),
                skipped_after_removal.clone(),
                final_peak.clone(),
            ],
            0,
            3,
            2,
            2,
            20,
            5,
            |_, _| false,
        );

        assert_eq!(
            result.detections,
            [CClefDetection {
                first: first.key(),
                second: second.key(),
                tails: vec![tail.key()],
            }]
        );
        assert_eq!(
            result.survivors,
            [leading.key(), skipped_after_removal.key(), final_peak.key(),]
        );
    }

    #[test]
    fn c_clef_purge_keeps_candidate_when_a_tail_is_connected() {
        let first = peak(1, 5, 8);
        let second = peak(1, 10, 10);
        let tail = peak(1, 12, 12);
        let values = [first.clone(), second.clone(), tail.clone()];

        let result = purge_c_clef_peaks(&values, 0, 3, 2, 2, 20, 5, |key, side| {
            key == tail.key() && side == VerticalSide::Bottom
        });

        assert!(result.detections.is_empty());
        assert_eq!(
            result.survivors,
            values.iter().map(StaffPeak::key).collect::<Vec<_>>()
        );
    }

    #[test]
    fn failed_second_peak_does_not_advance_c_clef_measure_start() {
        let candidate = peak(1, 5, 8);
        let too_wide_second = peak(1, 10, 14);
        let next_first = peak(1, 16, 19);
        let next_second = peak(1, 21, 21);

        let result = purge_c_clef_peaks(
            &[
                candidate,
                too_wide_second,
                next_first.clone(),
                next_second.clone(),
            ],
            0,
            3,
            2,
            2,
            20,
            0,
            |_, _| false,
        );

        assert_eq!(
            result.detections,
            [CClefDetection {
                first: next_first.key(),
                second: next_second.key(),
                tails: Vec::new(),
            }]
        );
    }
}
