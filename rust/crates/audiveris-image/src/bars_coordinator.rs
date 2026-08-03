// SPDX-License-Identifier: AGPL-3.0-or-later

//! Executable neutral seam for the typed portions of `BarsRetriever.process`.
//!
//! System discovery and brace/bracket recognition require Sheet, glyph, and
//! SIG ownership and remain outside. The caller supplies a system graph after
//! those structural attributes have been resolved; this coordinator performs
//! the source-ordered column/start/purge/classification/planning transitions.

use std::{collections::BTreeMap, error::Error, fmt};

use crate::{
    bar_alignment::{BarAlignment, BarAlignmentKind},
    bar_column::{BarColumn, PeakId, PeakRelation, StaffId},
    bars_logic::{
        BarsLogicError, ConnectionInterPlan, PeakWidthAssignment, PeakWidthClass,
        StartColumnStaffFacts, VerticalInterPlan, build_bar_columns_from_graph,
        classify_bar_peak_widths, peaks_before_staff_start, peaks_too_far_left,
        plan_connection_inters, plan_vertical_inters, purge_c_clef_peaks, start_column_candidate,
        unaligned_peak_keys, validate_start_column,
    },
    peak_graph::{PeakGraph, PeakGraphError},
    staff_peak::{HorizontalSide, StaffPeak, StaffPeakAttribute, StaffPeakKey},
};

#[derive(Clone, Debug)]
pub struct BarsStaffState {
    staff_id: StaffId,
    left: i32,
    one_line: bool,
    peaks: Vec<StaffPeak>,
    brace_peak: Option<StaffPeak>,
    standard_blank_to_lines: BTreeMap<StaffPeakKey, bool>,
}

impl BarsStaffState {
    pub fn new(
        staff_id: StaffId,
        left: i32,
        one_line: bool,
        peaks: Vec<StaffPeak>,
        standard_blank_to_lines: BTreeMap<StaffPeakKey, bool>,
    ) -> Result<Self, BarsCoordinatorError> {
        if staff_id.value() == 0
            || peaks.iter().any(|peak| peak.staff_id() != staff_id)
            || peaks.windows(2).any(|pair| pair[0].key() > pair[1].key())
        {
            return Err(BarsCoordinatorError::InvalidStaffState(staff_id));
        }
        Ok(Self {
            staff_id,
            left,
            one_line,
            peaks,
            brace_peak: None,
            standard_blank_to_lines,
        })
    }

    #[must_use]
    pub const fn staff_id(&self) -> StaffId {
        self.staff_id
    }

    #[must_use]
    pub const fn left(&self) -> i32 {
        self.left
    }

    #[must_use]
    pub const fn is_one_line(&self) -> bool {
        self.one_line
    }

    #[must_use]
    pub fn peaks(&self) -> &[StaffPeak] {
        &self.peaks
    }

    /// Preserve Java `StaffProjector.getBracePeak()`, which is held outside
    /// the ordinary peak list and is not necessarily promoted into the SIG.
    pub fn with_brace_peak(mut self, brace_peak: StaffPeak) -> Result<Self, BarsCoordinatorError> {
        if brace_peak.staff_id() != self.staff_id {
            return Err(BarsCoordinatorError::BracePeakOutsideStaff {
                staff: self.staff_id,
                peak: brace_peak.key(),
            });
        }
        if !brace_peak.is_brace() {
            return Err(BarsCoordinatorError::InvalidBracePeak(brace_peak.key()));
        }
        self.brace_peak = Some(brace_peak);
        Ok(self)
    }

    #[must_use]
    pub const fn brace_peak(&self) -> Option<&StaffPeak> {
        self.brace_peak.as_ref()
    }
}

#[derive(Clone, Debug)]
pub struct BarsSystemState {
    system_id: usize,
    staffs: Vec<BarsStaffState>,
    graph: PeakGraph<BarAlignment>,
    columns: Vec<BarColumn>,
}

impl BarsSystemState {
    pub fn new(
        system_id: usize,
        staffs: Vec<BarsStaffState>,
        graph: PeakGraph<BarAlignment>,
    ) -> Result<Self, BarsCoordinatorError> {
        if system_id == 0 || staffs.is_empty() {
            return Err(BarsCoordinatorError::InvalidSystemState(system_id));
        }
        let staff_ids = staffs
            .iter()
            .map(BarsStaffState::staff_id)
            .collect::<Vec<_>>();
        if staff_ids
            .windows(2)
            .any(|pair| pair[0].value() >= pair[1].value())
        {
            return Err(BarsCoordinatorError::InvalidSystemState(system_id));
        }
        for peak in graph.vertices() {
            let Some(staff) = staffs
                .iter()
                .find(|staff| staff.staff_id == peak.staff_id())
            else {
                return Err(BarsCoordinatorError::GraphPeakOutsideSystem(peak.key()));
            };
            if !staff
                .peaks
                .iter()
                .any(|candidate| candidate.key() == peak.key())
            {
                return Err(BarsCoordinatorError::GraphPeakMissingFromStaff(peak.key()));
            }
        }
        Ok(Self {
            system_id,
            staffs,
            graph,
            columns: Vec::new(),
        })
    }

    #[must_use]
    pub const fn system_id(&self) -> usize {
        self.system_id
    }

    #[must_use]
    pub fn staffs(&self) -> &[BarsStaffState] {
        &self.staffs
    }

    #[must_use]
    pub const fn graph(&self) -> &PeakGraph<BarAlignment> {
        &self.graph
    }

    #[must_use]
    pub fn columns(&self) -> &[BarColumn] {
        &self.columns
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CClefParameters {
    pub minimum_first_peak_width: i32,
    pub maximum_second_peak_width: i32,
    pub minimum_measure_width: i32,
    pub tail_width: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarsCoordinatorParameters {
    maximum_column_dx: i32,
    maximum_brace_bar_gap: i32,
    maximum_double_bar_gap: i32,
    maximum_lines_left_to_start_bar: i32,
    foreground_thickness: i32,
    interline: i32,
    minimum_normalized_width_delta: f64,
    c_clef: Option<CClefParameters>,
}

impl BarsCoordinatorParameters {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        maximum_column_dx: i32,
        maximum_brace_bar_gap: i32,
        maximum_double_bar_gap: i32,
        maximum_lines_left_to_start_bar: i32,
        foreground_thickness: i32,
        interline: i32,
        minimum_normalized_width_delta: f64,
        c_clef: Option<CClefParameters>,
    ) -> Result<Self, BarsCoordinatorError> {
        if maximum_column_dx < 0
            || maximum_brace_bar_gap < 0
            || maximum_double_bar_gap < 0
            || maximum_lines_left_to_start_bar < 0
            || foreground_thickness <= 0
            || interline <= 0
            || !minimum_normalized_width_delta.is_finite()
            || minimum_normalized_width_delta < 0.0
            || c_clef.is_some_and(|parameters| {
                parameters.minimum_first_peak_width <= 0
                    || parameters.maximum_second_peak_width <= 0
                    || parameters.minimum_measure_width < 0
                    || parameters.tail_width < 0
            })
        {
            return Err(BarsCoordinatorError::InvalidParameters);
        }
        Ok(Self {
            maximum_column_dx,
            maximum_brace_bar_gap,
            maximum_double_bar_gap,
            maximum_lines_left_to_start_bar,
            foreground_thickness,
            interline,
            minimum_normalized_width_delta,
            c_clef,
        })
    }

    #[must_use]
    pub const fn interline(self) -> i32 {
        self.interline
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeakRemovalStage {
    PartialColumn,
    TooFarLeft,
    LeftOfStaff,
    Unaligned,
    CClef,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RemovedPeak {
    pub peak: StaffPeakKey,
    pub stage: PeakRemovalStage,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BarsCoordinatorResult {
    start_column_index: Option<usize>,
    removed_peaks: Vec<RemovedPeak>,
    width_assignments: Vec<PeakWidthAssignment>,
    vertical_inters: Vec<VerticalInterPlan>,
    connection_inters: Vec<ConnectionInterPlan>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BarsPrefixResult {
    start_column_index: Option<usize>,
    removed_peaks: Vec<RemovedPeak>,
}

impl BarsPrefixResult {
    #[must_use]
    pub const fn start_column_index(&self) -> Option<usize> {
        self.start_column_index
    }

    #[must_use]
    pub fn removed_peaks(&self) -> &[RemovedPeak] {
        &self.removed_peaks
    }
}

impl BarsCoordinatorResult {
    #[must_use]
    pub const fn start_column_index(&self) -> Option<usize> {
        self.start_column_index
    }

    #[must_use]
    pub fn removed_peaks(&self) -> &[RemovedPeak] {
        &self.removed_peaks
    }

    #[must_use]
    pub fn width_assignments(&self) -> &[PeakWidthAssignment] {
        &self.width_assignments
    }

    #[must_use]
    pub fn vertical_inters(&self) -> &[VerticalInterPlan] {
        &self.vertical_inters
    }

    #[must_use]
    pub fn connection_inters(&self) -> &[ConnectionInterPlan] {
        &self.connection_inters
    }
}

/// Run the dependency-complete portions of `BarsRetriever.process` in source
/// order. All mutations commit together only after every stage succeeds.
pub fn process_bars_system(
    state: &mut BarsSystemState,
    parameters: BarsCoordinatorParameters,
) -> Result<BarsCoordinatorResult, BarsCoordinatorError> {
    let mut next = state.clone();
    let prefix = process_prefix(&mut next, parameters)?;

    let id_to_key = next
        .graph
        .vertices()
        .iter()
        .enumerate()
        .map(|(index, peak)| (PeakId::new(index + 1), peak.key()))
        .collect::<Vec<_>>();
    let mut removed_peaks = prefix.removed_peaks;
    let start_column_index = prefix.start_column_index;

    for staff_index in 0..next.staffs.len() {
        let keys = peaks_before_staff_start(
            &next.staffs[staff_index].peaks,
            next.staffs[staff_index].left,
        );
        remove_keys(
            &mut next,
            &keys,
            &id_to_key,
            PeakRemovalStage::LeftOfStaff,
            &mut removed_peaks,
        );
    }

    let multi_staff = next.staffs.len() > 1;
    for staff_index in 0..next.staffs.len() {
        let keys = unaligned_peak_keys(&next.staffs[staff_index].peaks, &next.graph, multi_staff);
        remove_keys(
            &mut next,
            &keys,
            &id_to_key,
            PeakRemovalStage::Unaligned,
            &mut removed_peaks,
        );
    }

    if let Some(c_clef) = parameters.c_clef {
        for staff_index in 0..next.staffs.len() {
            let peaks = next.staffs[staff_index].peaks.clone();
            let graph = &next.graph;
            let result = purge_c_clef_peaks(
                &peaks,
                next.staffs[staff_index].left,
                c_clef.minimum_first_peak_width,
                c_clef.maximum_second_peak_width,
                parameters.maximum_double_bar_gap,
                c_clef.minimum_measure_width,
                c_clef.tail_width,
                |key, side| graph.is_connected(key, side).unwrap_or(false),
            );
            let survivors = result
                .survivors
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>();
            let keys = peaks
                .iter()
                .map(StaffPeak::key)
                .filter(|key| !survivors.contains(key))
                .collect::<Vec<_>>();
            remove_keys(
                &mut next,
                &keys,
                &id_to_key,
                PeakRemovalStage::CClef,
                &mut removed_peaks,
            );
        }
    }

    let mut staff_peaks = next
        .staffs
        .iter()
        .map(|staff| staff.peaks.clone())
        .collect::<Vec<_>>();
    let width_assignments = classify_bar_peak_widths(
        &staff_peaks,
        parameters.maximum_double_bar_gap,
        parameters.interline,
        parameters.minimum_normalized_width_delta,
    );
    apply_width_assignments(&mut next, &width_assignments);
    staff_peaks = next
        .staffs
        .iter()
        .map(|staff| staff.peaks.clone())
        .collect();
    let vertical_inters = plan_vertical_inters(&staff_peaks, parameters.foreground_thickness);
    let inter_keys = vertical_inters
        .iter()
        .map(|plan| plan.peak)
        .collect::<std::collections::BTreeSet<_>>();
    let connection_inters = plan_connection_inters(&next.graph, |key| inter_keys.contains(&key));

    *state = next;
    Ok(BarsCoordinatorResult {
        start_column_index,
        removed_peaks,
        width_assignments,
        vertical_inters,
        connection_inters,
    })
}

/// Run the exact dependency-complete prefix through Java `purgeTooLeft` and
/// stop before `detectBracePortions`, which requires brace section/glyph state.
pub fn process_bars_through_too_far_left(
    state: &mut BarsSystemState,
    parameters: BarsCoordinatorParameters,
) -> Result<BarsPrefixResult, BarsCoordinatorError> {
    let mut next = state.clone();
    let result = process_prefix(&mut next, parameters)?;
    *state = next;
    Ok(result)
}

fn process_prefix(
    next: &mut BarsSystemState,
    parameters: BarsCoordinatorParameters,
) -> Result<BarsPrefixResult, BarsCoordinatorError> {
    let staff_ids = next
        .staffs
        .iter()
        .map(|staff| staff.staff_id)
        .collect::<Vec<_>>();
    next.columns =
        build_bar_columns_from_graph(&next.graph, &staff_ids, parameters.maximum_column_dx)?;
    let id_to_key = next
        .graph
        .vertices()
        .iter()
        .enumerate()
        .map(|(index, peak)| (PeakId::new(index + 1), peak.key()))
        .collect::<Vec<_>>();
    let relations = graph_relations(&next.graph, &id_to_key)?;

    let candidate = start_column_candidate(
        &mut next.columns,
        &relations,
        parameters.maximum_brace_bar_gap,
        parameters.maximum_double_bar_gap,
    );
    let start_column_index = if let Some(index) = candidate {
        validate_and_apply_start(
            &mut *next,
            index,
            &id_to_key,
            parameters.maximum_lines_left_to_start_bar,
        )?
    } else {
        None
    };

    let mut removed_peaks = Vec::new();
    purge_partial_columns(
        &mut *next,
        start_column_index,
        &id_to_key,
        &mut removed_peaks,
    )?;
    for staff_index in 0..next.staffs.len() {
        let start = next.staffs[staff_index]
            .peaks
            .iter()
            .position(|peak| peak.is_staff_end(HorizontalSide::Left));
        if let Some(start) = start {
            let keys = peaks_too_far_left(
                &next.staffs[staff_index].peaks,
                start,
                parameters.maximum_brace_bar_gap,
            )?;
            remove_keys(
                &mut *next,
                &keys,
                &id_to_key,
                PeakRemovalStage::TooFarLeft,
                &mut removed_peaks,
            );
        }
    }

    Ok(BarsPrefixResult {
        start_column_index,
        removed_peaks,
    })
}

fn graph_relations(
    graph: &PeakGraph<BarAlignment>,
    id_to_key: &[(PeakId, StaffPeakKey)],
) -> Result<Vec<PeakRelation>, BarsCoordinatorError> {
    let key_to_id = id_to_key
        .iter()
        .map(|(id, key)| (*key, *id))
        .collect::<BTreeMap<_, _>>();
    graph
        .edges()
        .iter()
        .map(|edge| {
            let first = *key_to_id
                .get(&edge.source())
                .ok_or(BarsCoordinatorError::MissingPeakId(edge.source()))?;
            let second = *key_to_id
                .get(&edge.target())
                .ok_or(BarsCoordinatorError::MissingPeakId(edge.target()))?;
            Ok(match edge.relation().kind() {
                BarAlignmentKind::Alignment => PeakRelation::alignment(first, second),
                BarAlignmentKind::Connection => PeakRelation::connection(first, second),
            })
        })
        .collect()
}

fn validate_and_apply_start(
    state: &mut BarsSystemState,
    index: usize,
    id_to_key: &[(PeakId, StaffPeakKey)],
    maximum_lines_left_to_start_bar: i32,
) -> Result<Option<usize>, BarsCoordinatorError> {
    let column = state
        .columns
        .get(index)
        .ok_or(BarsCoordinatorError::InvalidColumnIndex(index))?;
    let mut values = Vec::new();
    for peak in column.peaks().iter().flatten() {
        let key = *id_to_key
            .iter()
            .find_map(|(id, key)| (*id == peak.id()).then_some(key))
            .ok_or(BarsCoordinatorError::MissingColumnPeak(peak.id()))?;
        let staff = state
            .staffs
            .iter()
            .find(|staff| staff.staff_id == key.staff_id())
            .ok_or(BarsCoordinatorError::GraphPeakOutsideSystem(key))?;
        let value = state
            .graph
            .vertex(key)
            .cloned()
            .ok_or(BarsCoordinatorError::MissingGraphPeak(key))?;
        let blank = if staff.one_line {
            false
        } else {
            *staff
                .standard_blank_to_lines
                .get(&key)
                .ok_or(BarsCoordinatorError::MissingBlankEvidence(key))?
        };
        values.push((value, staff.left, staff.one_line, blank));
    }
    let facts = values
        .iter()
        .map(
            |(peak, staff_left, one_line, blank)| StartColumnStaffFacts {
                peak,
                staff_left: *staff_left,
                is_one_line_staff: *one_line,
                has_standard_blank_to_lines: *blank,
            },
        )
        .collect::<Vec<_>>();
    let Some(updates) = validate_start_column(&facts, maximum_lines_left_to_start_bar) else {
        return Ok(None);
    };
    for update in updates {
        let staff = state
            .staffs
            .iter_mut()
            .find(|staff| staff.staff_id == update.staff_id)
            .expect("validated column uses a system staff");
        staff.left = update.staff_left;
        staff
            .peaks
            .iter_mut()
            .find(|peak| peak.key() == update.peak)
            .expect("validated column peak remains in projector order")
            .set_staff_end(HorizontalSide::Left);
        state
            .graph
            .vertex_mut(update.peak)
            .expect("validated column peak remains in graph")
            .set_staff_end(HorizontalSide::Left);
    }
    Ok(Some(index))
}

fn purge_partial_columns(
    state: &mut BarsSystemState,
    start: Option<usize>,
    id_to_key: &[(PeakId, StaffPeakKey)],
    removed: &mut Vec<RemovedPeak>,
) -> Result<(), BarsCoordinatorError> {
    let first = start.map_or(0, |index| index + 1);
    let mut index = first;
    while index < state.columns.len() {
        if state.columns[index].is_full() {
            index += 1;
            continue;
        }
        let keys = state.columns[index]
            .peaks()
            .iter()
            .flatten()
            .map(|peak| {
                id_to_key
                    .iter()
                    .find_map(|(id, key)| (*id == peak.id()).then_some(*key))
                    .ok_or(BarsCoordinatorError::MissingColumnPeak(peak.id()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        state.columns.remove(index);
        remove_keys_without_columns(state, &keys, PeakRemovalStage::PartialColumn, removed);
    }
    Ok(())
}

fn remove_keys(
    state: &mut BarsSystemState,
    keys: &[StaffPeakKey],
    id_to_key: &[(PeakId, StaffPeakKey)],
    stage: PeakRemovalStage,
    removed: &mut Vec<RemovedPeak>,
) {
    if keys.is_empty() {
        return;
    }
    let mut expanded = keys.to_vec();
    state.columns.retain(|column| {
        let related = column.peaks().iter().flatten().any(|peak| {
            id_to_key
                .iter()
                .find_map(|(id, key)| (*id == peak.id()).then_some(key))
                .is_some_and(|key| keys.contains(key))
        });
        if related {
            for peak in column.peaks().iter().flatten() {
                if let Some(key) = id_to_key
                    .iter()
                    .find_map(|(id, key)| (*id == peak.id()).then_some(*key))
                    && !expanded.contains(&key)
                {
                    expanded.push(key);
                }
            }
        }
        !related
    });
    remove_keys_without_columns(state, &expanded, stage, removed);
}

fn remove_keys_without_columns(
    state: &mut BarsSystemState,
    keys: &[StaffPeakKey],
    stage: PeakRemovalStage,
    removed: &mut Vec<RemovedPeak>,
) {
    for &key in keys {
        let mut present = false;
        for staff in &mut state.staffs {
            present |= staff.peaks.iter().any(|peak| peak.key() == key);
            staff.peaks.retain(|peak| peak.key() != key);
        }
        present |= state.graph.remove_vertex(key).is_some();
        if present {
            removed.push(RemovedPeak { peak: key, stage });
        }
    }
}

fn apply_width_assignments(state: &mut BarsSystemState, assignments: &[PeakWidthAssignment]) {
    for assignment in assignments {
        for staff in &mut state.staffs {
            if let Some(peak) = staff
                .peaks
                .iter_mut()
                .find(|peak| peak.key() == assignment.peak)
            {
                set_width(peak, assignment.class);
            }
        }
        if let Some(peak) = state.graph.vertex_mut(assignment.peak) {
            set_width(peak, assignment.class);
        }
    }
}

fn set_width(peak: &mut StaffPeak, class: PeakWidthClass) {
    peak.unset(StaffPeakAttribute::Thin);
    peak.unset(StaffPeakAttribute::Thick);
    peak.set(match class {
        PeakWidthClass::Thin => StaffPeakAttribute::Thin,
        PeakWidthClass::Thick => StaffPeakAttribute::Thick,
    });
}

#[derive(Clone, Debug, PartialEq)]
pub enum BarsCoordinatorError {
    InvalidParameters,
    InvalidStaffState(StaffId),
    BracePeakOutsideStaff { staff: StaffId, peak: StaffPeakKey },
    InvalidBracePeak(StaffPeakKey),
    InvalidSystemState(usize),
    GraphPeakOutsideSystem(StaffPeakKey),
    GraphPeakMissingFromStaff(StaffPeakKey),
    MissingPeakId(StaffPeakKey),
    MissingColumnPeak(PeakId),
    MissingGraphPeak(StaffPeakKey),
    MissingBlankEvidence(StaffPeakKey),
    InvalidColumnIndex(usize),
    Logic(BarsLogicError),
    Graph(PeakGraphError),
}

impl From<BarsLogicError> for BarsCoordinatorError {
    fn from(value: BarsLogicError) -> Self {
        Self::Logic(value)
    }
}

impl From<PeakGraphError> for BarsCoordinatorError {
    fn from(value: PeakGraphError) -> Self {
        Self::Graph(value)
    }
}

impl fmt::Display for BarsCoordinatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidParameters => {
                formatter.write_str("bars coordinator parameters are invalid")
            }
            Self::InvalidStaffState(id) => {
                write!(formatter, "invalid bars state for staff {}", id.value())
            }
            Self::BracePeakOutsideStaff { staff, peak } => write!(
                formatter,
                "brace peak {:?} does not belong to staff {}",
                peak,
                staff.value()
            ),
            Self::InvalidBracePeak(peak) => {
                write!(formatter, "detached peak {:?} is not a brace", peak)
            }
            Self::InvalidSystemState(id) => write!(formatter, "invalid bars state for system {id}"),
            Self::GraphPeakOutsideSystem(key) => {
                write!(formatter, "graph peak {:?} is outside the system", key)
            }
            Self::GraphPeakMissingFromStaff(key) => {
                write!(formatter, "graph peak {:?} is absent from staff peaks", key)
            }
            Self::MissingPeakId(key) => write!(formatter, "peak {:?} has no stable column ID", key),
            Self::MissingColumnPeak(id) => {
                write!(formatter, "column peak {} has no graph key", id.value())
            }
            Self::MissingGraphPeak(key) => write!(formatter, "missing graph peak {:?}", key),
            Self::MissingBlankEvidence(key) => write!(
                formatter,
                "missing start-column blank evidence for {:?}",
                key
            ),
            Self::InvalidColumnIndex(index) => {
                write!(formatter, "invalid bar column index {index}")
            }
            Self::Logic(error) => write!(formatter, "bars logic failed: {error}"),
            Self::Graph(error) => write!(formatter, "peak graph failed: {error}"),
        }
    }
}

impl Error for BarsCoordinatorError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bar_alignment::{AlignmentPeak, BarImpacts};

    fn peak(staff: usize, start: i32, stop: i32) -> StaffPeak {
        let mut peak = StaffPeak::new(StaffId::new(staff), 10, 20, start, stop).unwrap();
        peak.compute_deskewed_center(|point| point).unwrap();
        peak
    }

    fn connection(top: StaffPeakKey, bottom: StaffPeakKey) -> BarAlignment {
        let alignment = BarAlignment::new(
            AlignmentPeak::new(PeakId::new(1), top.staff_id(), top.start(), 1.0).unwrap(),
            AlignmentPeak::new(PeakId::new(2), bottom.staff_id(), bottom.start(), 1.0).unwrap(),
            0.0,
            0.0,
            BarImpacts::alignment(1.0, 1.0).unwrap(),
        )
        .unwrap();
        BarAlignment::connection(&alignment, 1.0, 1.0).unwrap()
    }

    fn parameters() -> BarsCoordinatorParameters {
        BarsCoordinatorParameters::new(2, 12, 6, 20, 2, 10, 0.2, None).unwrap()
    }

    fn system(blank_evidence: bool) -> BarsSystemState {
        let top = peak(1, 10, 11);
        let bottom = peak(2, 10, 11);
        let partial = peak(1, 40, 41);
        let mut graph = PeakGraph::new();
        graph.add_vertex(top.clone());
        graph.add_vertex(bottom.clone());
        graph.add_vertex(partial.clone());
        graph
            .add_edge(top.key(), bottom.key(), connection(top.key(), bottom.key()))
            .unwrap();
        let top_blanks = if blank_evidence {
            BTreeMap::from([(top.key(), false)])
        } else {
            BTreeMap::new()
        };
        let bottom_blanks = if blank_evidence {
            BTreeMap::from([(bottom.key(), false)])
        } else {
            BTreeMap::new()
        };
        BarsSystemState::new(
            1,
            vec![
                BarsStaffState::new(StaffId::new(1), 0, false, vec![top, partial], top_blanks)
                    .unwrap(),
                BarsStaffState::new(StaffId::new(2), 0, false, vec![bottom], bottom_blanks)
                    .unwrap(),
            ],
            graph,
        )
        .unwrap()
    }

    #[test]
    fn process_applies_start_then_purges_partial_and_plans_inters() {
        let mut state = system(true);
        let result = process_bars_system(&mut state, parameters()).unwrap();

        assert_eq!(result.start_column_index(), Some(0));
        assert_eq!(state.columns().len(), 1);
        assert_eq!(state.staffs()[0].left(), 11);
        assert_eq!(state.staffs()[1].left(), 11);
        assert_eq!(state.graph().vertices().len(), 2);
        assert_eq!(
            result.removed_peaks(),
            [RemovedPeak {
                peak: StaffPeak::new(StaffId::new(1), 10, 20, 40, 41)
                    .unwrap()
                    .key(),
                stage: PeakRemovalStage::PartialColumn,
            }]
        );
        assert_eq!(result.width_assignments().len(), 2);
        assert_eq!(result.vertical_inters().len(), 2);
        assert_eq!(result.connection_inters().len(), 1);
        assert!(result.connection_inters()[0].endpoints_complete);
    }

    #[test]
    fn detached_brace_requires_matching_staff_provenance() {
        let staff = BarsStaffState::new(
            StaffId::new(1),
            0,
            false,
            vec![peak(1, 10, 11)],
            BTreeMap::new(),
        )
        .unwrap();
        let mut foreign = peak(2, 4, 6);
        foreign.set(StaffPeakAttribute::BraceTop);
        assert!(matches!(
            staff.clone().with_brace_peak(foreign),
            Err(BarsCoordinatorError::BracePeakOutsideStaff {
                staff: id,
                ..
            }) if id == StaffId::new(1)
        ));

        let plain = peak(1, 4, 6);
        assert!(matches!(
            staff.with_brace_peak(plain),
            Err(BarsCoordinatorError::InvalidBracePeak(_))
        ));
    }

    #[test]
    fn missing_start_blank_evidence_rolls_back_system_state() {
        let mut state = system(false);
        let before_vertices = state.graph().vertices().len();
        let before_lefts = state
            .staffs()
            .iter()
            .map(BarsStaffState::left)
            .collect::<Vec<_>>();

        let error = process_bars_system(&mut state, parameters()).unwrap_err();

        assert!(matches!(
            error,
            BarsCoordinatorError::MissingBlankEvidence(_)
        ));
        assert!(state.columns().is_empty());
        assert_eq!(state.graph().vertices().len(), before_vertices);
        assert_eq!(
            state
                .staffs()
                .iter()
                .map(BarsStaffState::left)
                .collect::<Vec<_>>(),
            before_lefts
        );
    }
}
