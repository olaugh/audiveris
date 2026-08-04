// SPDX-License-Identifier: AGPL-3.0-or-later

//! Executable neutral seam for the typed portions of `BarsRetriever.process`.
//!
//! System discovery and brace/bracket recognition require Sheet, glyph, and
//! SIG ownership and remain outside. The caller supplies a system graph after
//! those structural attributes have been resolved; this coordinator performs
//! the source-ordered column/start/purge/classification/planning transitions.

use std::{collections::BTreeMap, error::Error, fmt};

use crate::{
    bar_alignment::{BarAlignment, BarAlignmentKind, VerticalSide},
    bar_column::{BarColumn, PeakId, PeakRelation, StaffId},
    bars_logic::{
        BarsLogicError, CClefDetection, ConnectionInterPlan, PeakWidthAssignment, PeakWidthClass,
        PlannedPart, StartColumnStaffFacts, VerticalInterPlan, build_bar_columns_from_graph,
        classify_bar_peak_widths, extending_peak_keys, peaks_before_staff_start,
        peaks_too_far_left, plan_connection_inters, plan_vertical_inters, purge_c_clef_peaks,
        start_column_candidate, unaligned_peak_keys, validate_start_column,
    },
    grid_sig::{
        BarGroupPromotionError, BarTailError, BarTailResult, ConnectionPromotionWarning,
        GridInterId, GridSig, GridSigEdge,
    },
    part_group::PartGroup,
    peak_graph::{PeakGraph, PeakGraphError},
    projection::{ProjectionBlank, check_lines_root_transition, refine_right_end_transition},
    staff_peak::{HorizontalSide, PeakBounds, StaffPeak, StaffPeakAttribute, StaffPeakKey},
};

#[derive(Clone, Debug)]
pub struct BarsStaffState {
    staff_id: StaffId,
    left: i32,
    right: i32,
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
            right: left,
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
    pub const fn right(&self) -> i32 {
        self.right
    }

    #[must_use]
    pub fn with_right(mut self, right: i32) -> Self {
        self.right = right;
        self
    }

    #[must_use]
    pub const fn is_one_line(&self) -> bool {
        self.one_line
    }

    #[must_use]
    pub fn peaks(&self) -> &[StaffPeak] {
        &self.peaks
    }

    pub fn peaks_mut(&mut self) -> &mut [StaffPeak] {
        &mut self.peaks
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

    /// Reconcile an externally executed brace stage with this projector-owned
    /// staff copy. Validation happens before either field is replaced.
    pub fn replace_brace_stage(
        &mut self,
        peaks: Vec<StaffPeak>,
        brace_peak: Option<StaffPeak>,
    ) -> Result<(), BarsCoordinatorError> {
        // Java `insertPeak(new, old)` owns replacement order explicitly; do
        // not re-sort or reject that observable transient order here.
        if peaks.iter().any(|peak| peak.staff_id() != self.staff_id) {
            return Err(BarsCoordinatorError::InvalidStaffState(self.staff_id));
        }
        if let Some(brace) = &brace_peak {
            if brace.staff_id() != self.staff_id {
                return Err(BarsCoordinatorError::BracePeakOutsideStaff {
                    staff: self.staff_id,
                    peak: brace.key(),
                });
            }
            if !brace.is_brace() {
                return Err(BarsCoordinatorError::InvalidBracePeak(brace.key()));
            }
        }
        self.peaks = peaks;
        self.brace_peak = brace_peak;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct BarsSystemState {
    system_id: usize,
    staffs: Vec<BarsStaffState>,
    graph: PeakGraph<BarAlignment>,
    /// Stable Java entity identities assigned before any projector purge.
    peak_ids: Vec<(PeakId, StaffPeakKey)>,
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
        let peak_ids = graph
            .vertices()
            .iter()
            .enumerate()
            .map(|(index, peak)| (PeakId::new(index + 1), peak.key()))
            .collect();
        Ok(Self {
            system_id,
            staffs,
            graph,
            peak_ids,
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

    pub fn graph_mut(&mut self) -> &mut PeakGraph<BarAlignment> {
        &mut self.graph
    }

    #[must_use]
    pub fn peak_ids(&self) -> &[(PeakId, StaffPeakKey)] {
        &self.peak_ids
    }

    pub fn peak_ids_mut(&mut self) -> &mut Vec<(PeakId, StaffPeakKey)> {
        &mut self.peak_ids
    }

    pub fn graph_and_peak_ids_mut(
        &mut self,
    ) -> (
        &mut PeakGraph<BarAlignment>,
        &mut Vec<(PeakId, StaffPeakKey)>,
    ) {
        (&mut self.graph, &mut self.peak_ids)
    }

    #[must_use]
    pub fn columns(&self) -> &[BarColumn] {
        &self.columns
    }

    pub fn replace_columns(&mut self, columns: Vec<BarColumn>) {
        self.columns = columns;
    }

    pub fn staff_mut(&mut self, staff_id: StaffId) -> Option<&mut BarsStaffState> {
        self.staffs
            .iter_mut()
            .find(|staff| staff.staff_id == staff_id)
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

    #[must_use]
    pub const fn maximum_double_bar_gap(self) -> i32 {
        self.maximum_double_bar_gap
    }

    #[must_use]
    pub const fn foreground_thickness(self) -> i32 {
        self.foreground_thickness
    }

    #[must_use]
    pub const fn minimum_normalized_width_delta(self) -> f64 {
        self.minimum_normalized_width_delta
    }

    #[must_use]
    pub const fn c_clef(self) -> Option<CClefParameters> {
        self.c_clef
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeakRemovalStage {
    PartialColumn,
    TooFarLeft,
    LeftOfBrace,
    LeftOfStaff,
    Unaligned,
    ExtendingTop,
    ExtendingBottom,
    CClef,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarsPurgeParameters {
    pub large_system_staff_count: usize,
    pub maximum_foreground_thickness: i32,
    pub maximum_bar_extension: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BarsPurgeResult {
    removed_peaks: Vec<RemovedPeak>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BarsRightEvidence {
    pub staff_id: StaffId,
    pub blanks: Vec<ProjectionBlank>,
    pub sheet_right: i32,
    pub minimum_small_blank_width: i32,
    pub maximum_right_extremum: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RightEndUpdate {
    pub staff_id: StaffId,
    pub lines_right: i32,
    pub staff_right: i32,
    pub marked_peak: Option<StaffPeakKey>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BarsRightCClefParameters {
    pub maximum_double_bar_gap: i32,
    pub c_clef: CClefParameters,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BarsRightCClefResult {
    right_updates: Vec<RightEndUpdate>,
    c_clef_detections: Vec<(StaffId, CClefDetection)>,
    removed_peaks: Vec<RemovedPeak>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarsWidthInterParameters {
    pub maximum_double_bar_gap: i32,
    pub interline: i32,
    pub minimum_normalized_width_delta: f64,
    pub foreground_thickness: i32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BarsWidthInterResult {
    width_assignments: Vec<PeakWidthAssignment>,
    vertical_inters: Vec<VerticalInterPlan>,
    promoted_inters: Vec<GridInterId>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarsConnectionGroupParameters {
    pub maximum_double_bar_gap: i32,
    pub interline: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BarsConnectionGroupResult {
    connection_inters: Vec<ConnectionInterPlan>,
    connection_warnings: Vec<ConnectionPromotionWarning>,
    group_relations: Vec<GridSigEdge>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BarsRecordGroupResult {
    staff_barlines: BTreeMap<usize, Vec<GridInterId>>,
    part_groups: Vec<PartGroup>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BarsPartResult {
    retained_groups: Vec<PartGroup>,
    parts: Vec<PlannedPart>,
}

impl BarsPartResult {
    #[must_use]
    pub fn retained_groups(&self) -> &[PartGroup] {
        &self.retained_groups
    }

    #[must_use]
    pub fn parts(&self) -> &[PlannedPart] {
        &self.parts
    }
}

impl BarsRecordGroupResult {
    #[must_use]
    pub fn staff_barlines(&self) -> &BTreeMap<usize, Vec<GridInterId>> {
        &self.staff_barlines
    }

    #[must_use]
    pub fn part_groups(&self) -> &[PartGroup] {
        &self.part_groups
    }
}

impl BarsConnectionGroupResult {
    #[must_use]
    pub fn connection_inters(&self) -> &[ConnectionInterPlan] {
        &self.connection_inters
    }

    #[must_use]
    pub fn connection_warnings(&self) -> &[ConnectionPromotionWarning] {
        &self.connection_warnings
    }

    #[must_use]
    pub fn group_relations(&self) -> &[GridSigEdge] {
        &self.group_relations
    }
}

impl BarsWidthInterResult {
    #[must_use]
    pub fn width_assignments(&self) -> &[PeakWidthAssignment] {
        &self.width_assignments
    }

    #[must_use]
    pub fn vertical_inters(&self) -> &[VerticalInterPlan] {
        &self.vertical_inters
    }

    #[must_use]
    pub fn promoted_inters(&self) -> &[GridInterId] {
        &self.promoted_inters
    }
}

impl BarsRightCClefResult {
    #[must_use]
    pub fn right_updates(&self) -> &[RightEndUpdate] {
        &self.right_updates
    }

    #[must_use]
    pub fn c_clef_detections(&self) -> &[(StaffId, CClefDetection)] {
        &self.c_clef_detections
    }

    #[must_use]
    pub fn removed_peaks(&self) -> &[RemovedPeak] {
        &self.removed_peaks
    }
}

impl BarsPurgeResult {
    #[must_use]
    pub fn removed_peaks(&self) -> &[RemovedPeak] {
        &self.removed_peaks
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BarsRootEvidence {
    pub staff_id: StaffId,
    pub blanks: Vec<ProjectionBlank>,
    pub minimum_small_blank_width: i32,
    pub maximum_left_extremum: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinesRootUpdate {
    pub staff_id: StaffId,
    pub previous_left: i32,
    pub staff_left: i32,
    pub cleared_peak: Option<StaffPeakKey>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BarsPostBraceResult {
    removed_peaks: Vec<RemovedPeak>,
    lines_root_updates: Vec<LinesRootUpdate>,
}

impl BarsPostBraceResult {
    #[must_use]
    pub fn removed_peaks(&self) -> &[RemovedPeak] {
        &self.removed_peaks
    }

    #[must_use]
    pub fn lines_root_updates(&self) -> &[LinesRootUpdate] {
        &self.lines_root_updates
    }
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
    /// Assembles a result from the individually staged entry points.
    ///
    /// [`process_bars_system`] bundles the same stages into one call, which
    /// forces `createInters` to run before the later purges; callers that need
    /// Java's real order run the stages themselves and rejoin here.
    #[must_use]
    pub fn from_staged(
        start_column_index: Option<usize>,
        removed_peaks: Vec<RemovedPeak>,
        width_assignments: Vec<PeakWidthAssignment>,
        vertical_inters: Vec<VerticalInterPlan>,
        connection_inters: Vec<ConnectionInterPlan>,
    ) -> Self {
        Self {
            start_column_index,
            removed_peaks,
            width_assignments,
            vertical_inters,
            connection_inters,
        }
    }

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

    let id_to_key = next.peak_ids.clone();
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

/// Continue Java `BarsRetriever.process` immediately after `buildBraces`:
/// `purgeLeftOfBraces`, then `verifyLinesRoot`.
///
/// Mutations are deliberately not transactional. The source removes projector
/// and graph peaks before deleting their related columns, and a later adapter
/// evidence failure retains that completed prefix.
pub fn process_bars_after_braces(
    state: &mut BarsSystemState,
    root_evidence: &[BarsRootEvidence],
) -> Result<BarsPostBraceResult, BarsCoordinatorError> {
    let id_to_key = state.peak_ids.clone();
    let mut result = BarsPostBraceResult::default();

    // purgeLeftOfBraces applies only to multi-staff systems.
    if state.staffs.len() > 1 {
        for staff_index in 0..state.staffs.len() {
            let Some(brace_start) = state.staffs[staff_index]
                .brace_peak
                .as_ref()
                .map(StaffPeak::start)
            else {
                continue;
            };
            let Some(start_index) = state.staffs[staff_index]
                .peaks
                .iter()
                .position(|peak| peak.is_staff_end(HorizontalSide::Left))
            else {
                continue;
            };
            let keys = state.staffs[staff_index].peaks[..start_index]
                .iter()
                .rev()
                .filter(|peak| peak.stop() < brace_start)
                .map(StaffPeak::key)
                .collect::<Vec<_>>();
            remove_keys_and_related_columns_java_order(
                state,
                &keys,
                &id_to_key,
                PeakRemovalStage::LeftOfBrace,
                &mut result.removed_peaks,
            );
        }
    }

    // verifyLinesRoot applies only to one-staff systems.
    if state.staffs.len() == 1 {
        let staff = &mut state.staffs[0];
        let evidence = root_evidence
            .iter()
            .find(|evidence| evidence.staff_id == staff.staff_id)
            .ok_or(BarsCoordinatorError::MissingRootEvidence(staff.staff_id))?;
        if evidence.minimum_small_blank_width < 0 || evidence.maximum_left_extremum < 0 {
            return Err(BarsCoordinatorError::InvalidRootEvidence(staff.staff_id));
        }
        let start_index = staff
            .peaks
            .iter()
            .position(|peak| peak.is_staff_end(HorizontalSide::Left));
        let previous_left = staff.left;
        let transition = check_lines_root_transition(
            &staff.peaks,
            &evidence.blanks,
            staff.brace_peak.is_some(),
            start_index,
            staff.left,
            evidence.minimum_small_blank_width,
            evidence.maximum_left_extremum,
        );
        let cleared_peak = transition
            .clear_staff_left_end_at
            .map(|index| staff.peaks[index].key());
        if let Some(index) = transition.clear_staff_left_end_at {
            staff.peaks[index].unset(StaffPeakAttribute::StaffLeftEnd);
            state
                .graph
                .vertex_mut(staff.peaks[index].key())
                .expect("projector start peak remains in its system graph")
                .unset(StaffPeakAttribute::StaffLeftEnd);
        }
        staff.left = transition.staff_left;
        result.lines_root_updates.push(LinesRootUpdate {
            staff_id: staff.staff_id,
            previous_left,
            staff_left: transition.staff_left,
            cleared_peak,
        });
    }

    Ok(result)
}

/// Continue Java `BarsRetriever.process` through `purgeLeftPeaks`,
/// `purgeUnalignedBars`, and `purgeExtendingPeaks`.
///
/// Registered bar-filament bounds are supplied by stable peak key. Mutations
/// are non-transactional: a later missing bound preserves every earlier
/// source-ordered projector, graph, and column removal.
pub fn process_bars_peak_purges(
    state: &mut BarsSystemState,
    filament_bounds: &[(StaffPeakKey, PeakBounds)],
    parameters: BarsPurgeParameters,
) -> Result<BarsPurgeResult, BarsCoordinatorError> {
    if parameters.large_system_staff_count == 0
        || parameters.maximum_foreground_thickness < 0
        || !parameters.maximum_bar_extension.is_finite()
        || parameters.maximum_bar_extension < 0.0
    {
        return Err(BarsCoordinatorError::InvalidPurgeParameters);
    }
    let id_to_key = state.peak_ids.clone();
    let mut result = BarsPurgeResult::default();

    for staff_index in 0..state.staffs.len() {
        let keys = peaks_before_staff_start(
            &state.staffs[staff_index].peaks,
            state.staffs[staff_index].left,
        );
        remove_keys_and_related_columns_java_order(
            state,
            &keys,
            &id_to_key,
            PeakRemovalStage::LeftOfStaff,
            &mut result.removed_peaks,
        );
    }

    // The source removes each isolated projector/graph peak directly and does
    // not call deleteRelatedColumns. Valid post-prefix isolated peaks cannot
    // belong to a surviving full column.
    if state.staffs.len() > 1 {
        for staff_index in 0..state.staffs.len() {
            let keys = unaligned_peak_keys(&state.staffs[staff_index].peaks, &state.graph, true);
            remove_keys_without_columns(
                state,
                &keys,
                PeakRemovalStage::Unaligned,
                &mut result.removed_peaks,
            );
        }
    }

    if state.staffs.len() < parameters.large_system_staff_count && !state.staffs.is_empty() {
        for (side, staff_index, stage) in [
            (VerticalSide::Top, 0, PeakRemovalStage::ExtendingTop),
            (
                VerticalSide::Bottom,
                state.staffs.len() - 1,
                PeakRemovalStage::ExtendingBottom,
            ),
        ] {
            let start_index = state.staffs[staff_index]
                .peaks
                .iter()
                .position(|peak| peak.is_staff_end(HorizontalSide::Left));
            let first_eligible = start_index.map_or(0, |index| index.saturating_add(1));
            let mut facts = Vec::with_capacity(state.staffs[staff_index].peaks.len());
            for (index, peak) in state.staffs[staff_index].peaks.iter().enumerate() {
                let bounds = if index < first_eligible {
                    peak.bounds()
                } else {
                    filament_bounds
                        .iter()
                        .find_map(|(key, bounds)| (*key == peak.key()).then_some(*bounds))
                        .ok_or(BarsCoordinatorError::MissingFilamentBounds(peak.key()))?
                };
                facts.push((peak.clone(), bounds));
            }
            let keys = extending_peak_keys(
                &facts,
                start_index,
                parameters.maximum_foreground_thickness,
                parameters.maximum_bar_extension,
                side,
            );
            remove_keys_and_related_columns_java_order(
                state,
                &keys,
                &id_to_key,
                stage,
                &mut result.removed_peaks,
            );
        }
    }
    Ok(result)
}

/// Java `BarsRetriever.refineRightEnds` on its own.
///
/// Split out of [`process_bars_right_ends_and_c_clefs`] because it is the only
/// stage that sets a staff's right abscissa, and callers that already ran the
/// C-clef purge need this without repeating it. Java runs the two back to back;
/// the composed entry point below preserves that.
///
/// Every staff commits before the caller sees a failure, so a missing later
/// staff's evidence retains the exact earlier projector/graph mutation prefix.
pub fn process_bars_right_ends(
    state: &mut BarsSystemState,
    right_evidence: &[BarsRightEvidence],
) -> Result<Vec<RightEndUpdate>, BarsCoordinatorError> {
    let mut updates = Vec::with_capacity(state.staffs.len());
    for staff_index in 0..state.staffs.len() {
        let staff_id = state.staffs[staff_index].staff_id;
        let evidence = right_evidence
            .iter()
            .find(|evidence| evidence.staff_id == staff_id)
            .ok_or(BarsCoordinatorError::MissingRightEvidence(staff_id))?;
        if evidence.sheet_right < 0
            || evidence.minimum_small_blank_width < 0
            || evidence.maximum_right_extremum < 0
        {
            return Err(BarsCoordinatorError::InvalidRightEvidence(staff_id));
        }
        let lines_right = state.staffs[staff_index].right;
        let transition = refine_right_end_transition(
            &state.staffs[staff_index].peaks,
            &evidence.blanks,
            lines_right,
            evidence.sheet_right,
            evidence.minimum_small_blank_width,
            evidence.maximum_right_extremum,
        );
        let marked_peak = transition
            .set_staff_right_end_at
            .map(|index| state.staffs[staff_index].peaks[index].key());
        state.staffs[staff_index].right = transition.staff_right;
        if let Some(index) = transition.set_staff_right_end_at {
            state.staffs[staff_index].peaks[index].set_staff_end(HorizontalSide::Right);
            state
                .graph
                .vertex_mut(state.staffs[staff_index].peaks[index].key())
                .expect("projector right peak remains in its system graph")
                .set_staff_end(HorizontalSide::Right);
        }
        updates.push(RightEndUpdate {
            staff_id,
            lines_right,
            staff_right: transition.staff_right,
            marked_peak,
        });
    }
    Ok(updates)
}

/// Continue Java `BarsRetriever.process` through `refineRightEnds` and
/// `purgeCClefs`.
///
/// Right endpoints are committed for every staff before C-clef traversal
/// begins. A missing later staff's evidence therefore retains the exact
/// earlier projector/graph mutation prefix.
pub fn process_bars_right_ends_and_c_clefs(
    state: &mut BarsSystemState,
    right_evidence: &[BarsRightEvidence],
    parameters: BarsRightCClefParameters,
) -> Result<BarsRightCClefResult, BarsCoordinatorError> {
    if parameters.maximum_double_bar_gap < 0
        || parameters.c_clef.minimum_first_peak_width < 0
        || parameters.c_clef.maximum_second_peak_width < 0
        || parameters.c_clef.minimum_measure_width < 0
        || parameters.c_clef.tail_width < 0
    {
        return Err(BarsCoordinatorError::InvalidRightCClefParameters);
    }
    let mut result = BarsRightCClefResult {
        right_updates: process_bars_right_ends(state, right_evidence)?,
        ..BarsRightCClefResult::default()
    };

    let id_to_key = state.peak_ids.clone();
    for staff_index in 0..state.staffs.len() {
        let staff_id = state.staffs[staff_index].staff_id;
        let peaks = state.staffs[staff_index].peaks.clone();
        let graph = &state.graph;
        let purge = purge_c_clef_peaks(
            &peaks,
            state.staffs[staff_index].left,
            parameters.c_clef.minimum_first_peak_width,
            parameters.c_clef.maximum_second_peak_width,
            parameters.maximum_double_bar_gap,
            parameters.c_clef.minimum_measure_width,
            parameters.c_clef.tail_width,
            |key, side| graph.is_connected(key, side).unwrap_or(false),
        );
        let survivors = purge
            .survivors
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        let keys = peaks
            .iter()
            .map(StaffPeak::key)
            .filter(|key| !survivors.contains(key))
            .collect::<Vec<_>>();
        result.c_clef_detections.extend(
            purge
                .detections
                .into_iter()
                .map(|detection| (staff_id, detection)),
        );
        remove_keys_and_related_columns_java_order(
            state,
            &keys,
            &id_to_key,
            PeakRemovalStage::CClef,
            &mut result.removed_peaks,
        );
    }
    Ok(result)
}

/// Continue Java `BarsRetriever.process` through `partitionWidths` and
/// `createInters`.
///
/// Width attributes are mirrored into projector and graph peaks before the
/// exact system/staff/peak traversal registers one glyph and one SIG inter per
/// non-brace peak. The peak-to-inter backlink is retained by `GridSig`.
pub fn process_bars_widths_and_inters(
    state: &mut BarsSystemState,
    sig: &mut GridSig,
    parameters: BarsWidthInterParameters,
) -> Result<BarsWidthInterResult, BarsCoordinatorError> {
    if parameters.maximum_double_bar_gap < 0
        || parameters.interline <= 0
        || !parameters.minimum_normalized_width_delta.is_finite()
        || parameters.minimum_normalized_width_delta < 0.0
        || parameters.foreground_thickness < 0
    {
        return Err(BarsCoordinatorError::InvalidWidthInterParameters);
    }

    let staff_peaks = state
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
    apply_width_assignments(state, &width_assignments);

    let staff_peaks = state
        .staffs
        .iter()
        .map(|staff| staff.peaks.clone())
        .collect::<Vec<_>>();
    let vertical_inters = plan_vertical_inters(&staff_peaks, parameters.foreground_thickness);
    let promoted_inters = sig.promote_vertical_inters(&vertical_inters);

    Ok(BarsWidthInterResult {
        width_assignments,
        vertical_inters,
        promoted_inters,
    })
}

/// Continue Java `BarsRetriever.process` through `createConnectionInters` and
/// `groupBarlines`.
///
/// Connection promotion retains Java's per-edge partial failures: an orphan
/// connector and any already-added endpoint relation remain in the SIG. A
/// later missing bar inter likewise leaves all earlier group relations intact.
pub fn process_bars_connections_and_groups(
    state: &BarsSystemState,
    sig: &mut GridSig,
    parameters: BarsConnectionGroupParameters,
) -> Result<BarsConnectionGroupResult, BarsCoordinatorError> {
    if parameters.maximum_double_bar_gap < 0
        || !parameters.interline.is_finite()
        || parameters.interline <= 0.0
    {
        return Err(BarsCoordinatorError::InvalidConnectionGroupParameters);
    }

    let connection_inters = plan_connection_inters(&state.graph, |key| sig.inter_of(key).is_some());
    let connection_warnings = sig.promote_connection_inters(&state.graph, &connection_inters)?;
    let staff_peaks = state
        .staffs
        .iter()
        .map(|staff| staff.peaks.clone())
        .collect::<Vec<_>>();
    let first_group_edge = sig.edges().len();
    sig.group_barlines(&staff_peaks, parameters.maximum_double_bar_gap, |gap| {
        f64::from(gap) / parameters.interline
    })?;
    let group_relations = sig.edges()[first_group_edge..].to_vec();

    Ok(BarsConnectionGroupResult {
        connection_inters,
        connection_warnings,
        group_relations,
    })
}

/// Continue Java `BarsRetriever.process` through `recordBars` and
/// `createGroups`.
///
/// Explicit staff identities preserve empty-staff ownership. Barline lists
/// are committed staff by staff before group construction begins, so a later
/// invalid brace/group fact retains the exact `recordBars` prefix.
pub fn process_bars_record_and_groups(
    state: &BarsSystemState,
    sig: &GridSig,
    tail: &mut BarTailResult,
) -> Result<BarsRecordGroupResult, BarsCoordinatorError> {
    let staff_ids = state
        .staffs
        .iter()
        .map(|staff| staff.staff_id)
        .collect::<Vec<_>>();
    let staff_peaks = state
        .staffs
        .iter()
        .map(|staff| staff.peaks.clone())
        .collect::<Vec<_>>();
    let brace_peaks = state
        .staffs
        .iter()
        .map(|staff| staff.brace_peak.clone())
        .collect::<Vec<_>>();

    sig.record_bars_for_staffs(tail, &staff_ids, &staff_peaks)?;
    sig.create_groups_for_staffs(tail, &staff_ids, &staff_peaks, &brace_peaks, &state.graph)?;
    Ok(BarsRecordGroupResult {
        staff_barlines: tail.staff_barlines.clone(),
        part_groups: tail.part_groups.clone(),
    })
}

/// Java `createParts` for one system, preserving explicit empty-staff
/// ownership and true-brace removal semantics.
pub fn process_bars_create_parts(
    state: &BarsSystemState,
    sig: &GridSig,
    tail: &mut BarTailResult,
    parameters: crate::grid_sig::BarTailParameters,
) -> Result<BarsPartResult, BarsCoordinatorError> {
    let staff_ids = state
        .staffs
        .iter()
        .map(|staff| staff.staff_id)
        .collect::<Vec<_>>();
    let staff_peaks = state
        .staffs
        .iter()
        .map(|staff| staff.peaks.clone())
        .collect::<Vec<_>>();
    sig.create_parts_for_staffs(tail, &staff_ids, &staff_peaks, &state.graph, parameters)?;
    Ok(BarsPartResult {
        retained_groups: tail.part_groups.clone(),
        parts: tail.parts.clone(),
    })
}

/// Java's final per-system `SIGraph.contextualize()` mutation.
pub fn contextualize_bars(sig: &mut GridSig, tail: &mut BarTailResult) {
    sig.contextualize();
    tail.contextualized = true;
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
    let id_to_key = next.peak_ids.clone();
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

fn remove_keys_and_related_columns_java_order(
    state: &mut BarsSystemState,
    keys: &[StaffPeakKey],
    id_to_key: &[(PeakId, StaffPeakKey)],
    stage: PeakRemovalStage,
    removed: &mut Vec<RemovedPeak>,
) {
    if keys.is_empty() {
        return;
    }
    let mut column_indices = Vec::new();
    for key in keys {
        if let Some(index) = state.columns.iter().position(|column| {
            column.peaks().iter().flatten().any(|peak| {
                id_to_key
                    .iter()
                    .find_map(|(id, mapped)| (*id == peak.id()).then_some(mapped))
                    == Some(key)
            })
        }) && !column_indices.contains(&index)
        {
            column_indices.push(index);
        }
    }

    // projector.removePeaks(toRemove) precedes deleteRelatedColumns.
    remove_keys_without_columns(state, keys, stage, removed);
    let mut removed_column_indices = Vec::new();
    for original_index in column_indices {
        let shift = removed_column_indices
            .iter()
            .filter(|&&removed_index| removed_index < original_index)
            .count();
        let current_index = original_index - shift;
        let related = state.columns[current_index]
            .peaks()
            .iter()
            .flatten()
            .filter_map(|peak| {
                id_to_key
                    .iter()
                    .find_map(|(id, key)| (*id == peak.id()).then_some(*key))
            })
            .collect::<Vec<_>>();
        remove_keys_without_columns(state, &related, stage, removed);
        state.columns.remove(current_index);
        removed_column_indices.push(original_index);
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
    MissingRootEvidence(StaffId),
    InvalidRootEvidence(StaffId),
    InvalidPurgeParameters,
    MissingFilamentBounds(StaffPeakKey),
    InvalidRightCClefParameters,
    MissingRightEvidence(StaffId),
    InvalidRightEvidence(StaffId),
    InvalidWidthInterParameters,
    InvalidConnectionGroupParameters,
    InvalidColumnIndex(usize),
    Logic(BarsLogicError),
    BarGroup(BarGroupPromotionError),
    BarTail(BarTailError),
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

impl From<BarGroupPromotionError> for BarsCoordinatorError {
    fn from(value: BarGroupPromotionError) -> Self {
        Self::BarGroup(value)
    }
}

impl From<BarTailError> for BarsCoordinatorError {
    fn from(value: BarTailError) -> Self {
        Self::BarTail(value)
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
            Self::MissingRootEvidence(staff) => {
                write!(
                    formatter,
                    "missing lines-root evidence for staff {}",
                    staff.value()
                )
            }
            Self::InvalidRootEvidence(staff) => {
                write!(
                    formatter,
                    "invalid lines-root evidence for staff {}",
                    staff.value()
                )
            }
            Self::InvalidPurgeParameters => formatter.write_str("invalid bar purge parameters"),
            Self::MissingFilamentBounds(peak) => {
                write!(formatter, "missing bar filament bounds for {peak:?}")
            }
            Self::InvalidRightCClefParameters => {
                formatter.write_str("invalid right-end/C-clef parameters")
            }
            Self::MissingRightEvidence(staff) => {
                write!(
                    formatter,
                    "missing right-end evidence for staff {}",
                    staff.value()
                )
            }
            Self::InvalidRightEvidence(staff) => {
                write!(
                    formatter,
                    "invalid right-end evidence for staff {}",
                    staff.value()
                )
            }
            Self::InvalidWidthInterParameters => {
                formatter.write_str("invalid width-partition/inter parameters")
            }
            Self::InvalidConnectionGroupParameters => {
                formatter.write_str("invalid connection/group parameters")
            }
            Self::InvalidColumnIndex(index) => {
                write!(formatter, "invalid bar column index {index}")
            }
            Self::Logic(error) => write!(formatter, "bars logic failed: {error}"),
            Self::BarGroup(error) => write!(formatter, "bar grouping failed: {error:?}"),
            Self::BarTail(error) => write!(formatter, "bar tail failed: {error:?}"),
            Self::Graph(error) => write!(formatter, "peak graph failed: {error}"),
        }
    }
}

impl Error for BarsCoordinatorError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bar_alignment::{AlignmentPeak, BarImpacts};
    use crate::{bar_column::BarPeak, projection::ShortProjection};

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

    #[test]
    fn post_brace_purge_removes_explicit_peak_then_its_column_peers() {
        let top_left = peak(1, 0, 1);
        let mut top_start = peak(1, 10, 11);
        top_start.set_staff_end(HorizontalSide::Left);
        let bottom_left = peak(2, 0, 1);
        let mut bottom_start = peak(2, 10, 11);
        bottom_start.set_staff_end(HorizontalSide::Left);
        let mut graph = PeakGraph::new();
        for value in [&top_left, &bottom_left, &top_start, &bottom_start] {
            graph.add_vertex(value.clone());
        }
        let mut brace = peak(1, 5, 7);
        brace.set(StaffPeakAttribute::BraceTop);
        let top = BarsStaffState::new(
            StaffId::new(1),
            0,
            false,
            vec![top_left.clone(), top_start],
            BTreeMap::new(),
        )
        .unwrap()
        .with_brace_peak(brace)
        .unwrap();
        let bottom = BarsStaffState::new(
            StaffId::new(2),
            0,
            false,
            vec![bottom_left.clone(), bottom_start],
            BTreeMap::new(),
        )
        .unwrap();
        let mut state = BarsSystemState::new(1, vec![top, bottom], graph).unwrap();
        let mut column = BarColumn::new(vec![StaffId::new(1), StaffId::new(2)]).unwrap();
        column
            .add_peak(
                BarPeak::new(PeakId::new(1), StaffId::new(1), 2.0, 0.5, false, false).unwrap(),
            )
            .unwrap();
        column
            .add_peak(
                BarPeak::new(PeakId::new(2), StaffId::new(2), 2.0, 0.5, false, false).unwrap(),
            )
            .unwrap();
        state.columns.push(column);

        let result = process_bars_after_braces(&mut state, &[]).unwrap();

        assert_eq!(
            result.removed_peaks(),
            [
                RemovedPeak {
                    peak: top_left.key(),
                    stage: PeakRemovalStage::LeftOfBrace,
                },
                RemovedPeak {
                    peak: bottom_left.key(),
                    stage: PeakRemovalStage::LeftOfBrace,
                },
            ]
        );
        assert!(state.columns().is_empty());
        assert_eq!(state.graph().vertices().len(), 2);
        assert_eq!(state.staffs()[0].peaks().len(), 1);
        assert_eq!(state.staffs()[1].peaks().len(), 1);
    }

    #[test]
    fn one_staff_lines_root_clears_projector_and_graph_start_peak() {
        let first = peak(1, 20, 21);
        let mut start = peak(1, 25, 26);
        start.set_staff_end(HorizontalSide::Left);
        let mut graph = PeakGraph::new();
        graph.add_vertex(first.clone());
        graph.add_vertex(start.clone());
        let staff = BarsStaffState::new(
            StaffId::new(1),
            3,
            false,
            vec![first, start.clone()],
            BTreeMap::new(),
        )
        .unwrap();
        let mut state = BarsSystemState::new(1, vec![staff], graph).unwrap();
        let mut projection = ShortProjection::new(0, 10).unwrap();
        for position in 2..=4 {
            projection.increment_one(position);
        }
        let evidence = [BarsRootEvidence {
            staff_id: StaffId::new(1),
            blanks: projection.blank_regions(0),
            minimum_small_blank_width: 4,
            maximum_left_extremum: 8,
        }];

        let result = process_bars_after_braces(&mut state, &evidence).unwrap();

        assert_eq!(
            result.lines_root_updates(),
            [LinesRootUpdate {
                staff_id: StaffId::new(1),
                previous_left: 3,
                staff_left: 11,
                cleared_peak: Some(start.key()),
            }]
        );
        assert_eq!(state.staffs()[0].left(), 11);
        assert!(!state.staffs()[0].peaks()[1].is_staff_end(HorizontalSide::Left));
        assert!(
            !state
                .graph()
                .vertex(start.key())
                .unwrap()
                .is_staff_end(HorizontalSide::Left)
        );
    }

    #[test]
    fn peak_purge_slice_preserves_source_stage_and_side_order() {
        let left = peak(1, 0, 1);
        let mut top_start = peak(1, 5, 6);
        top_start.set_staff_end(HorizontalSide::Left);
        let top_bar = peak(1, 10, 11);
        let top_unaligned = peak(1, 20, 21);
        let top_long = peak(1, 30, 31);
        let mut bottom_start = peak(2, 5, 6);
        bottom_start.set_staff_end(HorizontalSide::Left);
        let bottom_bar = peak(2, 10, 11);
        let bottom_long = peak(2, 30, 31);
        let mut graph = PeakGraph::new();
        for value in [
            &left,
            &top_start,
            &top_bar,
            &top_unaligned,
            &top_long,
            &bottom_start,
            &bottom_bar,
            &bottom_long,
        ] {
            graph.add_vertex(value.clone());
        }
        for (top, bottom) in [
            (top_start.key(), bottom_start.key()),
            (top_bar.key(), bottom_bar.key()),
            (top_long.key(), bottom_long.key()),
        ] {
            graph
                .add_edge(top, bottom, connection(top, bottom))
                .unwrap();
        }
        let mut state = BarsSystemState::new(
            1,
            vec![
                BarsStaffState::new(
                    StaffId::new(1),
                    5,
                    false,
                    vec![
                        left.clone(),
                        top_start,
                        top_bar.clone(),
                        top_unaligned.clone(),
                        top_long.clone(),
                    ],
                    BTreeMap::new(),
                )
                .unwrap(),
                BarsStaffState::new(
                    StaffId::new(2),
                    5,
                    false,
                    vec![bottom_start, bottom_bar.clone(), bottom_long.clone()],
                    BTreeMap::new(),
                )
                .unwrap(),
            ],
            graph,
        )
        .unwrap();
        let bounds = [
            (
                top_bar.key(),
                PeakBounds {
                    x: 10,
                    y: 10,
                    width: 2,
                    height: 11,
                },
            ),
            (
                top_long.key(),
                PeakBounds {
                    x: 30,
                    y: 0,
                    width: 2,
                    height: 21,
                },
            ),
            (
                bottom_bar.key(),
                PeakBounds {
                    x: 10,
                    y: 10,
                    width: 2,
                    height: 11,
                },
            ),
            (
                bottom_long.key(),
                PeakBounds {
                    x: 30,
                    y: 10,
                    width: 2,
                    height: 31,
                },
            ),
        ];

        let result = process_bars_peak_purges(
            &mut state,
            &bounds,
            BarsPurgeParameters {
                large_system_staff_count: 10,
                maximum_foreground_thickness: 2,
                maximum_bar_extension: 5.0,
            },
        )
        .unwrap();

        assert_eq!(
            result.removed_peaks(),
            [
                RemovedPeak {
                    peak: left.key(),
                    stage: PeakRemovalStage::LeftOfStaff
                },
                RemovedPeak {
                    peak: top_unaligned.key(),
                    stage: PeakRemovalStage::Unaligned,
                },
                RemovedPeak {
                    peak: top_long.key(),
                    stage: PeakRemovalStage::ExtendingTop,
                },
                RemovedPeak {
                    peak: bottom_long.key(),
                    stage: PeakRemovalStage::ExtendingBottom,
                },
            ]
        );
        assert_eq!(state.graph().vertices().len(), 4);
        assert_eq!(state.staffs()[0].peaks().len(), 2);
        assert_eq!(state.staffs()[1].peaks().len(), 2);
    }

    #[test]
    fn missing_extension_evidence_retains_earlier_left_purge() {
        let left = peak(1, 0, 1);
        let mut start = peak(1, 5, 6);
        start.set_staff_end(HorizontalSide::Left);
        let bar = peak(1, 10, 11);
        let mut graph = PeakGraph::new();
        for value in [&left, &start, &bar] {
            graph.add_vertex(value.clone());
        }
        let mut state = BarsSystemState::new(
            1,
            vec![
                BarsStaffState::new(
                    StaffId::new(1),
                    5,
                    false,
                    vec![left.clone(), start, bar.clone()],
                    BTreeMap::new(),
                )
                .unwrap(),
            ],
            graph,
        )
        .unwrap();

        assert_eq!(
            process_bars_peak_purges(
                &mut state,
                &[],
                BarsPurgeParameters {
                    large_system_staff_count: 10,
                    maximum_foreground_thickness: 2,
                    maximum_bar_extension: 5.0,
                },
            ),
            Err(BarsCoordinatorError::MissingFilamentBounds(bar.key()))
        );
        assert!(!state.graph().contains_vertex(left.key()));
        assert_eq!(state.staffs()[0].peaks().len(), 2);
        assert!(state.graph().contains_vertex(bar.key()));
    }

    #[test]
    fn right_end_is_committed_before_c_clef_pair_is_removed() {
        let mut start = peak(1, 0, 1);
        start.set_staff_end(HorizontalSide::Left);
        let first = peak(1, 10, 13);
        let second = peak(1, 15, 15);
        let end = peak(1, 30, 30);
        let mut graph = PeakGraph::new();
        for value in [&start, &first, &second, &end] {
            graph.add_vertex(value.clone());
        }
        let staff = BarsStaffState::new(
            StaffId::new(1),
            0,
            false,
            vec![start, first.clone(), second.clone(), end.clone()],
            BTreeMap::new(),
        )
        .unwrap()
        .with_right(25);
        let mut state = BarsSystemState::new(1, vec![staff], graph).unwrap();
        let mut projection = ShortProjection::new(0, 40).unwrap();
        for position in (0..=31).chain(36..=40) {
            projection.increment_one(position);
        }
        let evidence = [BarsRightEvidence {
            staff_id: StaffId::new(1),
            blanks: projection.blank_regions(0),
            sheet_right: 40,
            minimum_small_blank_width: 3,
            maximum_right_extremum: 3,
        }];

        let result = process_bars_right_ends_and_c_clefs(
            &mut state,
            &evidence,
            BarsRightCClefParameters {
                maximum_double_bar_gap: 2,
                c_clef: CClefParameters {
                    minimum_first_peak_width: 4,
                    maximum_second_peak_width: 1,
                    minimum_measure_width: 20,
                    tail_width: 5,
                },
            },
        )
        .unwrap();

        assert_eq!(
            result.right_updates(),
            [RightEndUpdate {
                staff_id: StaffId::new(1),
                lines_right: 25,
                staff_right: 30,
                marked_peak: Some(end.key()),
            }]
        );
        assert_eq!(result.c_clef_detections().len(), 1);
        assert_eq!(
            result.removed_peaks(),
            [
                RemovedPeak {
                    peak: first.key(),
                    stage: PeakRemovalStage::CClef
                },
                RemovedPeak {
                    peak: second.key(),
                    stage: PeakRemovalStage::CClef
                },
            ]
        );
        assert_eq!(state.staffs()[0].right(), 30);
        assert!(
            state.staffs()[0]
                .peaks()
                .last()
                .unwrap()
                .is_staff_end(HorizontalSide::Right)
        );
        assert!(
            state
                .graph()
                .vertex(end.key())
                .unwrap()
                .is_staff_end(HorizontalSide::Right)
        );
    }

    #[test]
    fn missing_later_right_evidence_retains_earlier_right_end_without_c_clef_purge() {
        let mut top_start = peak(1, 0, 1);
        top_start.set_staff_end(HorizontalSide::Left);
        let top_first = peak(1, 10, 13);
        let top_second = peak(1, 15, 15);
        let top_end = peak(1, 30, 30);
        let mut bottom_start = peak(2, 0, 1);
        bottom_start.set_staff_end(HorizontalSide::Left);
        let bottom_end = peak(2, 30, 30);
        let mut graph = PeakGraph::new();
        for value in [
            &top_start,
            &top_first,
            &top_second,
            &top_end,
            &bottom_start,
            &bottom_end,
        ] {
            graph.add_vertex(value.clone());
        }
        let top = BarsStaffState::new(
            StaffId::new(1),
            0,
            false,
            vec![
                top_start,
                top_first.clone(),
                top_second.clone(),
                top_end.clone(),
            ],
            BTreeMap::new(),
        )
        .unwrap()
        .with_right(25);
        let bottom = BarsStaffState::new(
            StaffId::new(2),
            0,
            false,
            vec![bottom_start, bottom_end],
            BTreeMap::new(),
        )
        .unwrap()
        .with_right(25);
        let mut state = BarsSystemState::new(1, vec![top, bottom], graph).unwrap();
        let mut projection = ShortProjection::new(0, 40).unwrap();
        for position in (0..=31).chain(36..=40) {
            projection.increment_one(position);
        }
        let evidence = [BarsRightEvidence {
            staff_id: StaffId::new(1),
            blanks: projection.blank_regions(0),
            sheet_right: 40,
            minimum_small_blank_width: 3,
            maximum_right_extremum: 3,
        }];

        assert_eq!(
            process_bars_right_ends_and_c_clefs(
                &mut state,
                &evidence,
                BarsRightCClefParameters {
                    maximum_double_bar_gap: 2,
                    c_clef: CClefParameters {
                        minimum_first_peak_width: 4,
                        maximum_second_peak_width: 1,
                        minimum_measure_width: 20,
                        tail_width: 5,
                    },
                },
            ),
            Err(BarsCoordinatorError::MissingRightEvidence(StaffId::new(2)))
        );
        assert_eq!(state.staffs()[0].right(), 30);
        assert_eq!(state.staffs()[1].right(), 25);
        assert!(
            state.staffs()[0]
                .peaks()
                .last()
                .unwrap()
                .is_staff_end(HorizontalSide::Right)
        );
        assert!(
            state
                .graph()
                .vertex(top_end.key())
                .unwrap()
                .is_staff_end(HorizontalSide::Right)
        );
        assert!(state.graph().contains_vertex(top_first.key()));
        assert!(state.graph().contains_vertex(top_second.key()));
    }

    #[test]
    fn width_partition_precedes_stable_glyph_and_inter_registration() {
        let impacts = crate::staff_peak::StaffVerticalImpacts::new(0.9, 0.8, 1.0, 1.0, 0.7, 0.6);
        let mut thin = StaffPeak::with_impacts(StaffId::new(1), 10, 20, 10, 10, impacts).unwrap();
        thin.compute_deskewed_center(|point| point).unwrap();
        let thick = peak(1, 12, 14);
        let isolated = peak(1, 30, 31);
        let mut graph = PeakGraph::new();
        for value in [&thin, &thick, &isolated] {
            graph.add_vertex(value.clone());
        }
        let staff = BarsStaffState::new(
            StaffId::new(1),
            0,
            false,
            vec![thin.clone(), thick.clone(), isolated.clone()],
            BTreeMap::new(),
        )
        .unwrap();
        let mut state = BarsSystemState::new(1, vec![staff], graph).unwrap();
        let mut sig = GridSig::default();

        let result = process_bars_widths_and_inters(
            &mut state,
            &mut sig,
            BarsWidthInterParameters {
                maximum_double_bar_gap: 2,
                interline: 10,
                minimum_normalized_width_delta: 0.2,
                foreground_thickness: 2,
            },
        )
        .unwrap();

        assert_eq!(result.width_assignments().len(), 3);
        assert!(state.staffs()[0].peaks()[0].is_set(StaffPeakAttribute::Thin));
        assert!(state.staffs()[0].peaks()[1].is_set(StaffPeakAttribute::Thick));
        assert!(state.staffs()[0].peaks()[2].is_set(StaffPeakAttribute::Thin));
        for peak in [&thin, &thick, &isolated] {
            assert_eq!(
                state
                    .graph()
                    .vertex(peak.key())
                    .unwrap()
                    .attributes()
                    .collect::<Vec<_>>(),
                state.staffs()[0]
                    .peaks()
                    .iter()
                    .find(|candidate| candidate.key() == peak.key())
                    .unwrap()
                    .attributes()
                    .collect::<Vec<_>>()
            );
        }
        assert_eq!(
            result
                .vertical_inters()
                .iter()
                .map(|plan| plan.peak)
                .collect::<Vec<_>>(),
            vec![thin.key(), thick.key(), isolated.key()]
        );
        assert_eq!(
            result
                .promoted_inters()
                .iter()
                .map(|id| id.value())
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(sig.inter_of(thin.key()), Some(result.promoted_inters()[0]));
        assert_eq!(
            sig.node(result.promoted_inters()[0])
                .unwrap()
                .intrinsic_grade(),
            impacts.grade()
        );
    }

    #[test]
    fn invalid_width_parameters_do_not_mutate_peaks_or_sig() {
        let bar = peak(1, 10, 11);
        let mut graph = PeakGraph::new();
        graph.add_vertex(bar.clone());
        let staff = BarsStaffState::new(
            StaffId::new(1),
            0,
            false,
            vec![bar.clone()],
            BTreeMap::new(),
        )
        .unwrap();
        let mut state = BarsSystemState::new(1, vec![staff], graph).unwrap();
        let mut sig = GridSig::default();

        assert_eq!(
            process_bars_widths_and_inters(
                &mut state,
                &mut sig,
                BarsWidthInterParameters {
                    maximum_double_bar_gap: 2,
                    interline: 0,
                    minimum_normalized_width_delta: 0.2,
                    foreground_thickness: 2,
                },
            ),
            Err(BarsCoordinatorError::InvalidWidthInterParameters)
        );
        assert!(!state.staffs()[0].peaks()[0].is_set(StaffPeakAttribute::Thin));
        assert!(!state.staffs()[0].peaks()[0].is_set(StaffPeakAttribute::Thick));
        assert!(sig.nodes_in_order().next().is_none());
        assert_eq!(sig.inter_of(bar.key()), None);
    }

    #[test]
    fn connection_promotion_precedes_staff_ordered_bar_groups() {
        let top_left = peak(1, 10, 10);
        let top_right = peak(1, 12, 12);
        let bottom_left = peak(2, 10, 10);
        let bottom_right = peak(2, 12, 12);
        let mut graph = PeakGraph::new();
        for value in [&top_left, &top_right, &bottom_left, &bottom_right] {
            graph.add_vertex(value.clone());
        }
        graph
            .add_edge(
                top_left.key(),
                bottom_left.key(),
                connection(top_left.key(), bottom_left.key()),
            )
            .unwrap();
        graph
            .add_edge(
                top_right.key(),
                bottom_right.key(),
                connection(top_right.key(), bottom_right.key()),
            )
            .unwrap();
        let top = BarsStaffState::new(
            StaffId::new(1),
            0,
            false,
            vec![top_left.clone(), top_right.clone()],
            BTreeMap::new(),
        )
        .unwrap();
        let bottom = BarsStaffState::new(
            StaffId::new(2),
            0,
            false,
            vec![bottom_left.clone(), bottom_right.clone()],
            BTreeMap::new(),
        )
        .unwrap();
        let state = BarsSystemState::new(1, vec![top, bottom], graph).unwrap();
        let staff_peaks = state
            .staffs()
            .iter()
            .map(|staff| staff.peaks().to_vec())
            .collect::<Vec<_>>();
        let plans = plan_vertical_inters(&staff_peaks, 2);
        let mut sig = GridSig::default();
        sig.promote_vertical_inters(&plans);

        let result = process_bars_connections_and_groups(
            &state,
            &mut sig,
            BarsConnectionGroupParameters {
                maximum_double_bar_gap: 2,
                interline: 10.0,
            },
        )
        .unwrap();

        assert_eq!(result.connection_inters().len(), 2);
        assert!(result.connection_warnings().is_empty());
        assert_eq!(result.group_relations().len(), 2);
        assert_eq!(sig.edges().len(), 8);
        assert_eq!(
            result
                .group_relations()
                .iter()
                .map(|edge| (edge.source, edge.target))
                .collect::<Vec<_>>(),
            vec![
                (
                    sig.inter_of(top_left.key()).unwrap(),
                    sig.inter_of(top_right.key()).unwrap(),
                ),
                (
                    sig.inter_of(bottom_left.key()).unwrap(),
                    sig.inter_of(bottom_right.key()).unwrap(),
                ),
            ]
        );
        for peak in [
            top_left.key(),
            top_right.key(),
            bottom_left.key(),
            bottom_right.key(),
        ] {
            assert!(matches!(
                sig.node(sig.inter_of(peak).unwrap()),
                Some(crate::grid_sig::GridSigNode::Vertical { frozen: true, .. })
            ));
        }
    }

    #[test]
    fn missing_later_bar_inter_retains_earlier_group_relation() {
        let first = peak(1, 10, 10);
        let second = peak(1, 12, 12);
        let third = peak(1, 14, 14);
        let mut graph = PeakGraph::new();
        for value in [&first, &second, &third] {
            graph.add_vertex(value.clone());
        }
        let staff = BarsStaffState::new(
            StaffId::new(1),
            0,
            false,
            vec![first.clone(), second.clone(), third.clone()],
            BTreeMap::new(),
        )
        .unwrap();
        let state = BarsSystemState::new(1, vec![staff], graph).unwrap();
        let plans = plan_vertical_inters(&[vec![first.clone(), second.clone()]], 2);
        let mut sig = GridSig::default();
        sig.promote_vertical_inters(&plans);

        assert_eq!(
            process_bars_connections_and_groups(
                &state,
                &mut sig,
                BarsConnectionGroupParameters {
                    maximum_double_bar_gap: 2,
                    interline: 10.0,
                },
            ),
            Err(BarsCoordinatorError::BarGroup(
                BarGroupPromotionError::MissingInter(third.key())
            ))
        );
        assert_eq!(sig.edges().len(), 1);
        assert!(matches!(
            sig.edges()[0].relation,
            crate::grid_sig::GridSigRelation::BarGroup { gap_fraction }
                if (gap_fraction - 0.1).abs() < f64::EPSILON
        ));
    }

    #[test]
    fn record_bars_then_create_detached_brace_group_in_staff_order() {
        let mut top_bar = peak(1, 10, 10);
        top_bar.set_staff_end(HorizontalSide::Left);
        let mut bottom_bar = peak(2, 10, 10);
        bottom_bar.set_staff_end(HorizontalSide::Left);
        let mut top_brace = peak(1, 4, 5);
        top_brace.set(StaffPeakAttribute::BraceTop);
        let mut bottom_brace = peak(2, 4, 5);
        bottom_brace.set(StaffPeakAttribute::BraceBottom);
        let mut graph = PeakGraph::new();
        graph.add_vertex(top_bar.clone());
        graph.add_vertex(bottom_bar.clone());
        graph
            .add_edge(
                top_bar.key(),
                bottom_bar.key(),
                connection(top_bar.key(), bottom_bar.key()),
            )
            .unwrap();
        let top = BarsStaffState::new(
            StaffId::new(1),
            0,
            false,
            vec![top_bar.clone()],
            BTreeMap::new(),
        )
        .unwrap()
        .with_brace_peak(top_brace)
        .unwrap();
        let bottom = BarsStaffState::new(
            StaffId::new(2),
            0,
            false,
            vec![bottom_bar.clone()],
            BTreeMap::new(),
        )
        .unwrap()
        .with_brace_peak(bottom_brace)
        .unwrap();
        let state = BarsSystemState::new(1, vec![top, bottom], graph).unwrap();
        let staff_peaks = state
            .staffs()
            .iter()
            .map(|staff| staff.peaks().to_vec())
            .collect::<Vec<_>>();
        let mut sig = GridSig::default();
        let inters = sig.promote_vertical_inters(&plan_vertical_inters(&staff_peaks, 2));
        let mut tail = BarTailResult::default();

        let result = process_bars_record_and_groups(&state, &sig, &mut tail).unwrap();

        assert_eq!(result.staff_barlines()[&1], [inters[0]]);
        assert_eq!(result.staff_barlines()[&2], [inters[1]]);
        assert_eq!(result.part_groups().len(), 1);
        assert_eq!(
            result.part_groups()[0].symbol(),
            crate::part_group::PartGroupingSymbol::Brace
        );
        assert_eq!(result.part_groups()[0].first_staff_id(), 1);
        assert_eq!(result.part_groups()[0].last_staff_id(), 2);
    }

    #[test]
    fn invalid_later_brace_retains_all_recorded_staff_barlines() {
        let mut top_bar = peak(1, 10, 10);
        top_bar.set_staff_end(HorizontalSide::Left);
        let mut bottom_bar = peak(2, 10, 10);
        bottom_bar.set_staff_end(HorizontalSide::Left);
        let mut graph = PeakGraph::new();
        graph.add_vertex(top_bar.clone());
        graph.add_vertex(bottom_bar.clone());
        let top = BarsStaffState::new(
            StaffId::new(1),
            0,
            false,
            vec![top_bar.clone()],
            BTreeMap::new(),
        )
        .unwrap();
        let bottom = BarsStaffState::new(
            StaffId::new(2),
            0,
            false,
            vec![bottom_bar.clone()],
            BTreeMap::new(),
        )
        .unwrap();
        let mut state = BarsSystemState::new(1, vec![top, bottom], graph).unwrap();
        let mut foreign_brace = peak(1, 4, 5);
        foreign_brace.set(StaffPeakAttribute::BraceBottom);
        state.staffs[1].brace_peak = Some(foreign_brace);
        let staff_peaks = state
            .staffs()
            .iter()
            .map(|staff| staff.peaks().to_vec())
            .collect::<Vec<_>>();
        let mut sig = GridSig::default();
        let inters = sig.promote_vertical_inters(&plan_vertical_inters(&staff_peaks, 2));
        let mut tail = BarTailResult::default();

        assert_eq!(
            process_bars_record_and_groups(&state, &sig, &mut tail),
            Err(BarsCoordinatorError::BarTail(
                BarTailError::BracePeakStaffMismatch {
                    expected: 2,
                    actual: 1,
                }
            ))
        );
        assert_eq!(tail.staff_barlines[&1], [inters[0]]);
        assert_eq!(tail.staff_barlines[&2], [inters[1]]);
        assert!(tail.part_groups.is_empty());
    }

    #[test]
    fn create_parts_keeps_empty_staff_and_contextualizes_only_afterward() {
        let top_bar = peak(1, 10, 10);
        let mut graph = PeakGraph::new();
        graph.add_vertex(top_bar.clone());
        let top = BarsStaffState::new(
            StaffId::new(1),
            0,
            false,
            vec![top_bar.clone()],
            BTreeMap::new(),
        )
        .unwrap();
        let bottom =
            BarsStaffState::new(StaffId::new(2), 0, false, Vec::new(), BTreeMap::new()).unwrap();
        let state = BarsSystemState::new(1, vec![top, bottom], graph).unwrap();
        let mut sig = GridSig::default();
        let inter =
            sig.promote_vertical_inters(&plan_vertical_inters(&[vec![top_bar], Vec::new()], 2))[0];
        let mut tail = BarTailResult::default();

        let result = process_bars_create_parts(
            &state,
            &sig,
            &mut tail,
            crate::grid_sig::BarTailParameters {
                allow_disconnected_braced_parts: false,
                force_separate_parts: false,
                force_single_part: true,
            },
        )
        .unwrap();

        assert_eq!(
            result.parts(),
            [PlannedPart {
                first_staff_id: 1,
                last_staff_id: 2,
            }]
        );
        assert!(!tail.contextualized);
        assert_eq!(sig.node(inter).unwrap().contextual_grade(), None);

        contextualize_bars(&mut sig, &mut tail);
        assert!(tail.contextualized);
        assert!(sig.node(inter).unwrap().contextual_grade().is_some());
    }
}
