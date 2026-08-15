// SPDX-License-Identifier: AGPL-3.0-or-later

//! Production-backed `ProcessBars` for prepared system projector state.
//!
//! Each dependency-complete system coordinator is transactional internally.
//! Completed earlier systems are retained if a later system fails, but the
//! failing system itself has no Java-compatible mutation prefix yet.

use std::collections::{HashMap, HashSet};

use crate::{
    bar_alignment::BarAlignment,
    bar_column::StaffId,
    bars_coordinator::{
        BarsConnectionGroupParameters, BarsCoordinatorError, BarsCoordinatorParameters,
        BarsCoordinatorResult, BarsPurgeParameters, BarsRightCClefParameters, BarsRightEvidence,
        BarsRootEvidence, BarsSystemState, BarsWidthInterParameters, CClefParameters, RemovedPeak,
        process_bars_after_braces, process_bars_connections_and_groups,
        process_bars_left_boundary_reassignment, process_bars_peak_purges,
        process_bars_right_ends_and_c_clefs, process_bars_system,
        process_bars_through_too_far_left, process_bars_weak_unconnected_purge,
        process_bars_widths_and_inters,
    },
    bars_logic::{ConnectionInterPlan, VerticalInterPlan},
    grid_lifecycle::{GridBuildStage, GridStageFailure},
    grid_sig::GridSig,
    lines_coordinator::StaffCandidateKind,
    peak_graph::{PeakGraph, PeakGraphError},
    prepared_lines::{
        PreparedStaffHandoff, PreparedStaffStage, RawLineMetadataHandoff, RawLineMetadataStage,
    },
    raster_grid_builder::{
        HeadlessRasterGridBuilder, RasterGridBuildState, RemainingRasterGridStages,
    },
    staff_peak::{PeakBounds, StaffPeak, StaffPeakKey},
};

#[derive(Clone, Debug)]
pub struct PreparedBarsSystem {
    pub system_id: usize,
    /// Exact Java `SystemInfo.getStaves()` traversal, expressed as stable
    /// staff identities. This cannot be reconstructed from `staff_peaks`: a
    /// valid staff may own no surviving bar peak.
    pub staff_ids: Vec<usize>,
    pub staff_peaks: Vec<Vec<StaffPeak>>,
    /// Java `staff.getAbscissa(LEFT/RIGHT)` per staff, after `BarsRetriever`
    /// refined it: the start column sets LEFT, `verifyLinesRoot` may push it
    /// right again, and `refineRightEnds` sets RIGHT. `completeLines` pins every
    /// line ending at these, so they must travel with the peaks.
    pub staff_limits: Vec<(i32, i32)>,
    /// Java `StaffProjector.getBracePeak()` per staff, kept detached from the
    /// ordinary peak list and SIG promotion path.
    pub brace_peaks: Vec<Option<StaffPeak>>,
    pub vertical_plans: Vec<VerticalInterPlan>,
    pub maximum_group_gap: i32,
    pub interline: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreparedConnectionPlan {
    pub system_id: usize,
    pub plan: ConnectionInterPlan,
}

#[derive(Clone, Debug)]
pub struct PreparedBarsHandoff {
    pub systems: Vec<PreparedBarsSystem>,
    pub peak_graph: PeakGraph<BarAlignment>,
    /// Global Java traversal order: prepared system input order, then each
    /// system's local graph edge order. Every local edge ID is remapped to the
    /// corresponding ID in `peak_graph`.
    pub connections: Vec<PreparedConnectionPlan>,
}

pub trait PreparedBarsHandoffSource {
    fn take_prepared_bars_handoff(&mut self) -> Option<PreparedBarsHandoff>;
}

pub trait PreparedBarsStage {
    fn take_prepared_bars_handoff(&mut self) -> Option<PreparedBarsHandoff>;
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProductionProcessBarsError<UpstreamError> {
    InvalidSystemOrder,
    MissingStaffHandoff,
    InvalidStaffOrder(StaffId),
    DuplicateStaff(StaffId),
    UnknownStaff(StaffId),
    StaffKindMismatch(StaffId),
    MissingStaff(usize),
    Bars {
        system_id: usize,
        source: BarsCoordinatorError,
    },
    Graph(PeakGraphError),
    MissingEdgeRemap {
        system_id: usize,
        edge: usize,
    },
    Upstream(UpstreamError),
}

pub struct ProductionProcessBars<Upstream> {
    upstream: Upstream,
    systems: Vec<BarsSystemState>,
    parameters: BarsCoordinatorParameters,
    maximum_group_gap: i32,
    extending: Option<ExtendingPurge>,
    limits: Option<StaffLimitRefinement>,
    completed_brace_prefixes: Option<Vec<CompletedBracePrefix>>,
    handoff: Option<PreparedBarsHandoff>,
    removals: Vec<(usize, RemovedPeak)>,
    weak_unconnected_min_grade: Option<f64>,
    left_boundary_reassignment: bool,
}

/// A system already advanced through `detectBracePortions`, `buildBraces`,
/// `purgeLeftOfBraces`, and `verifyLinesRoot` by a sheet-aware caller.  The
/// state itself remains in `ProductionProcessBars::systems`; this retains the
/// ordered mutation evidence needed to assemble the final coordinator result
/// without replaying the prefix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletedBracePrefix {
    pub system_id: usize,
    pub start_column_index: Option<usize>,
    pub removed_peaks: Vec<RemovedPeak>,
}

/// Projector evidence for the two stages that set a staff's abscissae.
///
/// Both need the staff's projection blanks, which `BarsSystemState` does not
/// carry, so they are supplied here for the same reason the extending purge is.
#[derive(Clone, Debug)]
struct StaffLimitRefinement {
    root_evidence: Vec<BarsRootEvidence>,
    right_evidence: Vec<BarsRightEvidence>,
}

/// Inputs for Java `BarsRetriever.purgeExtendingPeaks`.
///
/// That stage needs each peak's bar-filament bounds, which `BarsSystemState`
/// does not carry, so it lives behind its own entry point
/// ([`process_bars_peak_purges`]) and is supplied here rather than derived.
#[derive(Clone, Debug)]
struct ExtendingPurge {
    filament_bounds: Vec<(StaffPeakKey, PeakBounds)>,
    parameters: BarsPurgeParameters,
}

impl<Upstream> ProductionProcessBars<Upstream> {
    pub fn new(
        upstream: Upstream,
        systems: Vec<BarsSystemState>,
        parameters: BarsCoordinatorParameters,
        maximum_group_gap: i32,
    ) -> Result<Self, ProductionProcessBarsError<Upstream::OtherError>>
    where
        Upstream: RemainingRasterGridStages,
    {
        if systems
            .windows(2)
            .any(|pair| pair[0].system_id() >= pair[1].system_id())
        {
            return Err(ProductionProcessBarsError::InvalidSystemOrder);
        }
        Ok(Self {
            upstream,
            systems,
            parameters,
            maximum_group_gap,
            extending: None,
            limits: None,
            completed_brace_prefixes: None,
            handoff: None,
            removals: Vec::new(),
            weak_unconnected_min_grade: None,
            left_boundary_reassignment: false,
        })
    }

    /// Enables Java's `purgeExtendingPeaks`, which runs after the coordinator
    /// purges and before the peaks are published downstream.
    ///
    /// Callers that own the bar filaments should always enable it: without it
    /// the handoff retains peaks whose stick runs too far past the staff, which
    /// Java drops. It stays opt-in because the bounds are not reconstructible
    /// from `BarsSystemState` alone.
    #[must_use]
    pub fn with_extending_purge(
        mut self,
        filament_bounds: Vec<(StaffPeakKey, PeakBounds)>,
        parameters: BarsPurgeParameters,
    ) -> Self {
        self.extending = Some(ExtendingPurge {
            filament_bounds,
            parameters,
        });
        self
    }

    /// Consume states a sheet-aware brace stage has already advanced through
    /// Java's post-brace boundary. System order must exactly match the owned
    /// state vector.
    pub fn with_completed_brace_prefixes(
        mut self,
        prefixes: Vec<CompletedBracePrefix>,
    ) -> Result<Self, ProductionProcessBarsError<Upstream::OtherError>>
    where
        Upstream: RemainingRasterGridStages,
    {
        if prefixes
            .iter()
            .map(|prefix| prefix.system_id)
            .ne(self.systems.iter().map(BarsSystemState::system_id))
        {
            return Err(ProductionProcessBarsError::InvalidSystemOrder);
        }
        self.completed_brace_prefixes = Some(prefixes);
        Ok(self)
    }

    /// Enables Java's two staff-abscissa refinements: `verifyLinesRoot` and
    /// `refineRightEnds`.
    ///
    /// Without this the handoff publishes the staff limits as the start column
    /// left them, and `completeLines` pins every staff line ending at an
    /// unrefined right abscissa. Evidence is looked up by staff id, so passing
    /// the whole sheet's is fine.
    ///
    /// Ordering caveat: Java runs `verifyLinesRoot` before the left/unaligned/
    /// extending purges and `refineRightEnds` after them, whereas this runs both
    /// after. `verifyLinesRoot` only fires on single-staff systems and only when
    /// the first peak sits far enough past the preceding blank; when it does
    /// fire it raises the staff's left abscissa, which would feed
    /// `purgeLeftPeaks`. That the barline output already matches Java exactly on
    /// every example page is the evidence this does not bite there. Reordering
    /// needs the full staged sequence, which also needs a `GridSig`.
    #[must_use]
    pub fn with_staff_limit_refinement(
        mut self,
        root_evidence: Vec<BarsRootEvidence>,
        right_evidence: Vec<BarsRightEvidence>,
    ) -> Self {
        self.limits = Some(StaffLimitRefinement {
            root_evidence,
            right_evidence,
        });
        self
    }

    /// Enable the fork's weak, unconnected interior-bar rejection. The
    /// default remains off so the ordinary API retains exact Java parity.
    #[must_use]
    pub fn with_weak_unconnected_filter(mut self, minimum_grade: f64) -> Self {
        self.weak_unconnected_min_grade = Some(minimum_grade);
        self
    }

    /// Enable the fork's conservative two-staff left-boundary reassignment.
    /// The default remains off to preserve Java parity.
    #[must_use]
    pub const fn with_left_boundary_reassignment(mut self) -> Self {
        self.left_boundary_reassignment = true;
        self
    }

    #[must_use]
    pub const fn upstream(&self) -> &Upstream {
        &self.upstream
    }

    /// Every peak the last run dropped, as `(system id, record)` in removal
    /// order.
    ///
    /// Kept for diffing against a Java run: each record names both the peak and
    /// the `BarsRetriever` stage that discarded it.
    #[must_use]
    pub fn removals(&self) -> &[(usize, RemovedPeak)] {
        &self.removals
    }

    /// Runs one system through Java `BarsRetriever.process`.
    ///
    /// When the caller supplied the projector evidence -- bar-filament bounds
    /// via [`Self::with_extending_purge`] and projection blanks via
    /// [`Self::with_staff_limit_refinement`] -- this runs the staged sequence in
    /// Java's own order:
    ///
    /// ```text
    /// buildColumns .. purgeTooLeft   (process_bars_through_too_far_left)
    /// purgeLeftOfBraces, verifyLinesRoot (process_bars_after_braces)
    /// purgeLeftPeaks, purgeUnalignedBars, purgeExtendingPeaks
    /// refineRightEnds, purgeCClefs
    /// partitionWidths, createInters
    /// createConnectionInters, groupBarlines
    /// ```
    ///
    /// Order matters here and not only for tidiness: `createInters` must run
    /// after the purges, or peaks that `purgeExtendingPeaks` drops still receive
    /// barline inters. `process_bars_system` bundles the prefix, some purges,
    /// and `createInters` into one call, so it cannot express that.
    ///
    /// Without the evidence it falls back to that bundled call, which is what
    /// callers holding only a `BarsSystemState` can support.
    fn staged_system(
        system: &mut BarsSystemState,
        parameters: BarsCoordinatorParameters,
        extending: Option<&ExtendingPurge>,
        limits: Option<&StaffLimitRefinement>,
        completed_brace_prefix: Option<&CompletedBracePrefix>,
        weak_unconnected_min_grade: Option<f64>,
        left_boundary_reassignment: bool,
    ) -> Result<BarsCoordinatorResult, BarsCoordinatorError> {
        let (Some(extending), Some(limits)) = (extending, limits) else {
            return process_bars_system(system, parameters);
        };

        let (start_column_index, mut removed) = if let Some(completed) = completed_brace_prefix {
            (
                completed.start_column_index,
                completed.removed_peaks.clone(),
            )
        } else {
            let prefix = process_bars_through_too_far_left(system, parameters)?;
            let mut removed = prefix.removed_peaks().to_vec();
            let braces = process_bars_after_braces(system, &limits.root_evidence)?;
            removed.extend_from_slice(braces.removed_peaks());
            (prefix.start_column_index(), removed)
        };

        if left_boundary_reassignment {
            removed.extend(process_bars_left_boundary_reassignment(
                system,
                parameters.interline(),
            )?);
        }

        let purges =
            process_bars_peak_purges(system, &extending.filament_bounds, extending.parameters)?;
        removed.extend_from_slice(purges.removed_peaks());

        let c_clef = parameters.c_clef().unwrap_or(CClefParameters {
            minimum_first_peak_width: 1,
            maximum_second_peak_width: 1,
            minimum_measure_width: 0,
            tail_width: 0,
        });
        let right = process_bars_right_ends_and_c_clefs(
            system,
            &limits.right_evidence,
            BarsRightCClefParameters {
                maximum_double_bar_gap: parameters.maximum_double_bar_gap(),
                c_clef,
            },
        )?;
        removed.extend_from_slice(right.removed_peaks());

        if let Some(minimum_grade) = weak_unconnected_min_grade {
            removed.extend(process_bars_weak_unconnected_purge(
                system,
                minimum_grade,
                parameters.maximum_double_bar_gap(),
                parameters.interline(),
            )?);
        }

        // `partitionWidths` and `createInters` need a SIG to register glyphs and
        // inters into. The sheet builds its own from the published plans, so
        // this one is scratch space for the traversal.
        let mut sig = GridSig::default();
        let widths = process_bars_widths_and_inters(
            system,
            &mut sig,
            BarsWidthInterParameters {
                maximum_double_bar_gap: parameters.maximum_double_bar_gap(),
                interline: parameters.interline(),
                minimum_normalized_width_delta: parameters.minimum_normalized_width_delta(),
                foreground_thickness: parameters.foreground_thickness(),
            },
        )?;
        let connections = process_bars_connections_and_groups(
            system,
            &mut sig,
            BarsConnectionGroupParameters {
                maximum_double_bar_gap: parameters.maximum_double_bar_gap(),
                interline: f64::from(parameters.interline()),
            },
        )?;

        Ok(BarsCoordinatorResult::from_staged(
            start_column_index,
            removed,
            widths.width_assignments().to_vec(),
            widths.vertical_inters().to_vec(),
            connections.connection_inters().to_vec(),
        ))
    }

    /// Runs `processBars` without a `RasterGridBuildState`.
    ///
    /// The stage never reads the build state: its inputs are the prepared
    /// systems it owns and the upstream staff handoff. Callers that already
    /// derived both — `recognize_grid_lines` builds them from the live
    /// projectors — run the stage through here instead of standing up a
    /// builder just to satisfy the signature.
    pub fn run_process_bars(
        &mut self,
    ) -> Result<(), ProductionProcessBarsError<Upstream::OtherError>>
    where
        Upstream: RemainingRasterGridStages + PreparedStaffStage,
    {
        self.handoff = None;
        self.removals.clear();
        let staffs = self
            .upstream
            .prepared_staff_handoff()
            .ok_or(ProductionProcessBarsError::MissingStaffHandoff)?;
        validate_staff_join(staffs, &self.systems)?;

        let mut handoff = PreparedBarsHandoff {
            systems: Vec::with_capacity(self.systems.len()),
            peak_graph: PeakGraph::new(),
            connections: Vec::new(),
        };
        let extending = self.extending.clone();
        let limits = self.limits.clone();
        let completed_brace_prefixes = self.completed_brace_prefixes.clone();
        let parameters = self.parameters;
        let weak_unconnected_min_grade = self.weak_unconnected_min_grade;
        let left_boundary_reassignment = self.left_boundary_reassignment;
        for (system_index, system) in self.systems.iter_mut().enumerate() {
            let system_id = system.system_id();
            let result = match Self::staged_system(
                system,
                parameters,
                extending.as_ref(),
                limits.as_ref(),
                completed_brace_prefixes
                    .as_ref()
                    .and_then(|prefixes| prefixes.get(system_index)),
                weak_unconnected_min_grade,
                left_boundary_reassignment,
            ) {
                Ok(result) => result,
                Err(source) => {
                    self.handoff = Some(handoff);
                    return Err(ProductionProcessBarsError::Bars { system_id, source });
                }
            };
            self.removals.extend(
                result
                    .removed_peaks()
                    .iter()
                    .map(|removed| (system_id, *removed)),
            );
            if let Err(error) = append_system(
                &mut handoff,
                system,
                result.vertical_inters(),
                result.connection_inters(),
                self.maximum_group_gap,
                f64::from(parameters.interline()),
                self.limits.is_some(),
            ) {
                self.handoff = Some(handoff);
                return Err(error);
            }
        }
        self.handoff = Some(handoff);
        Ok(())
    }
}

impl<Stages, Vip> PreparedBarsHandoffSource for HeadlessRasterGridBuilder<Stages, Vip>
where
    Stages: PreparedBarsStage,
{
    fn take_prepared_bars_handoff(&mut self) -> Option<PreparedBarsHandoff> {
        self.stages_mut().take_prepared_bars_handoff()
    }
}

impl<Upstream> PreparedStaffStage for ProductionProcessBars<Upstream>
where
    Upstream: PreparedStaffStage,
{
    fn prepared_staff_handoff(&self) -> Option<&PreparedStaffHandoff> {
        self.upstream.prepared_staff_handoff()
    }

    fn take_prepared_staff_handoff(&mut self) -> Option<PreparedStaffHandoff> {
        self.upstream.take_prepared_staff_handoff()
    }
}

impl<Upstream> RawLineMetadataStage for ProductionProcessBars<Upstream>
where
    Upstream: RawLineMetadataStage,
{
    fn take_raw_line_metadata_handoff(&mut self) -> Option<RawLineMetadataHandoff> {
        self.upstream.take_raw_line_metadata_handoff()
    }
}

impl<Upstream> PreparedBarsStage for ProductionProcessBars<Upstream> {
    fn take_prepared_bars_handoff(&mut self) -> Option<PreparedBarsHandoff> {
        self.handoff.take()
    }
}

impl<Upstream> RemainingRasterGridStages for ProductionProcessBars<Upstream>
where
    Upstream: RemainingRasterGridStages + PreparedStaffStage,
{
    type StepError = Upstream::StepError;
    type OtherError = ProductionProcessBarsError<Upstream::OtherError>;

    fn retrieve_lines(
        &mut self,
        state: &mut RasterGridBuildState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        self.upstream
            .retrieve_lines(state)
            .map_err(map_upstream_failure)
    }

    fn process_bars(
        &mut self,
        _state: &mut RasterGridBuildState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        self.run_process_bars().map_err(GridStageFailure::Other)
    }

    fn complete_lines(
        &mut self,
        state: &mut RasterGridBuildState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        self.upstream
            .complete_lines(state)
            .map_err(map_upstream_failure)
    }

    fn log_swallowed_error(&mut self, stage: GridBuildStage, error: &Self::OtherError) {
        if let ProductionProcessBarsError::Upstream(error) = error {
            self.upstream.log_swallowed_error(stage, error);
        }
    }

    fn finish(&mut self) {
        self.upstream.finish();
    }
}

fn map_upstream_failure<StepError, UpstreamError>(
    failure: GridStageFailure<StepError, UpstreamError>,
) -> GridStageFailure<StepError, ProductionProcessBarsError<UpstreamError>> {
    match failure {
        GridStageFailure::Step(error) => GridStageFailure::Step(error),
        GridStageFailure::Other(error) => {
            GridStageFailure::Other(ProductionProcessBarsError::Upstream(error))
        }
    }
}

fn validate_staff_join<UpstreamError>(
    prepared: &PreparedStaffHandoff,
    systems: &[BarsSystemState],
) -> Result<(), ProductionProcessBarsError<UpstreamError>> {
    let mut previous = None;
    let mut seen = HashSet::new();
    for system in systems {
        for staff in system.staffs() {
            let id = staff.staff_id();
            if !seen.insert(id) {
                return Err(ProductionProcessBarsError::DuplicateStaff(id));
            }
            if previous.is_some_and(|prior: StaffId| prior.value() >= id.value()) {
                return Err(ProductionProcessBarsError::InvalidStaffOrder(id));
            }
            previous = Some(id);
            let candidate = prepared
                .staffs
                .iter()
                .find(|candidate| candidate.id == id.value())
                .ok_or(ProductionProcessBarsError::UnknownStaff(id))?;
            if (candidate.kind == StaffCandidateKind::OneLine) != staff.is_one_line() {
                return Err(ProductionProcessBarsError::StaffKindMismatch(id));
            }
        }
    }
    for candidate in &prepared.staffs {
        if !seen.contains(&StaffId::new(candidate.id))
            && candidate.kind != StaffCandidateKind::OneLine
        {
            return Err(ProductionProcessBarsError::MissingStaff(candidate.id));
        }
    }
    Ok(())
}

fn append_system<UpstreamError>(
    handoff: &mut PreparedBarsHandoff,
    state: &BarsSystemState,
    vertical_plans: &[VerticalInterPlan],
    connections: &[ConnectionInterPlan],
    maximum_group_gap: i32,
    interline: f64,
    refined_limits: bool,
) -> Result<(), ProductionProcessBarsError<UpstreamError>> {
    let mut next = handoff.clone();
    append_system_in_place(
        &mut next,
        state,
        vertical_plans,
        connections,
        maximum_group_gap,
        interline,
        refined_limits,
    )?;
    *handoff = next;
    Ok(())
}

fn append_system_in_place<UpstreamError>(
    handoff: &mut PreparedBarsHandoff,
    state: &BarsSystemState,
    vertical_plans: &[VerticalInterPlan],
    connections: &[ConnectionInterPlan],
    maximum_group_gap: i32,
    interline: f64,
    refined_limits: bool,
) -> Result<(), ProductionProcessBarsError<UpstreamError>> {
    for peak in state.graph().vertices() {
        handoff.peak_graph.add_vertex(peak.clone());
    }
    let mut edge_remap = HashMap::new();
    for edge in state.graph().edges() {
        let global = handoff
            .peak_graph
            .add_edge(edge.source(), edge.target(), *edge.relation())
            .map_err(ProductionProcessBarsError::Graph)?;
        edge_remap.insert(edge.id(), global);
    }
    for connection in connections {
        let global = edge_remap.get(&connection.edge).copied().ok_or(
            ProductionProcessBarsError::MissingEdgeRemap {
                system_id: state.system_id(),
                edge: connection.edge.value(),
            },
        )?;
        let mut plan = *connection;
        plan.edge = global;
        handoff.connections.push(PreparedConnectionPlan {
            system_id: state.system_id(),
            plan,
        });
    }
    handoff.systems.push(PreparedBarsSystem {
        system_id: state.system_id(),
        staff_ids: state
            .staffs()
            .iter()
            .map(|staff| staff.staff_id().value())
            .collect(),
        staff_peaks: state
            .staffs()
            .iter()
            .map(|staff| staff.peaks().to_vec())
            .collect(),
        // Only published when the abscissa refinement actually ran. Without it
        // these are whatever the caller constructed the states with, which is
        // not Java's `getAbscissa` and must not be adopted as such.
        staff_limits: if refined_limits {
            state
                .staffs()
                .iter()
                .map(|staff| (staff.left(), staff.right()))
                .collect()
        } else {
            Vec::new()
        },
        brace_peaks: state
            .staffs()
            .iter()
            .map(|staff| staff.brace_peak().cloned())
            .collect(),
        vertical_plans: vertical_plans.to_vec(),
        maximum_group_gap,
        interline,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::{
        bar_alignment::{AlignmentPeak, BarImpacts},
        bar_column::PeakId,
        bars_coordinator::BarsStaffState,
        prepared_lines::PreparedStaff,
        staff_peak::StaffPeak,
    };

    fn peak(staff: usize, x: i32) -> StaffPeak {
        let mut peak = StaffPeak::new(StaffId::new(staff), 10, 20, x, x + 1).unwrap();
        peak.compute_deskewed_center(|point| point).unwrap();
        peak
    }

    fn connection(top: &StaffPeak, bottom: &StaffPeak) -> BarAlignment {
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

    fn system(system_id: usize, top_id: usize, bottom_id: usize) -> BarsSystemState {
        let top = peak(top_id, 10);
        let bottom = peak(bottom_id, 10);
        let mut graph = PeakGraph::new();
        graph.add_vertex(top.clone());
        graph.add_vertex(bottom.clone());
        graph
            .add_edge(top.key(), bottom.key(), connection(&top, &bottom))
            .unwrap();
        BarsSystemState::new(
            system_id,
            vec![
                BarsStaffState::new(StaffId::new(top_id), 0, true, vec![top], BTreeMap::new())
                    .unwrap(),
                BarsStaffState::new(
                    StaffId::new(bottom_id),
                    0,
                    true,
                    vec![bottom],
                    BTreeMap::new(),
                )
                .unwrap(),
            ],
            graph,
        )
        .unwrap()
    }

    fn parameters() -> BarsCoordinatorParameters {
        BarsCoordinatorParameters::new(2, 12, 6, 20, 2, 10, 0.2, None).unwrap()
    }

    fn empty_handoff() -> PreparedBarsHandoff {
        PreparedBarsHandoff {
            systems: Vec::new(),
            peak_graph: PeakGraph::new(),
            connections: Vec::new(),
        }
    }

    #[test]
    fn remaps_local_edge_ids_in_system_then_graph_order() {
        let mut handoff = empty_handoff();
        for mut state in [system(1, 1, 2), system(2, 3, 4)] {
            let result = process_bars_system(&mut state, parameters()).unwrap();
            append_system::<()>(
                &mut handoff,
                &state,
                result.vertical_inters(),
                result.connection_inters(),
                6,
                10.0,
                true,
            )
            .unwrap();
        }

        assert_eq!(
            handoff
                .peak_graph
                .edges()
                .iter()
                .map(|edge| edge.id().value())
                .collect::<Vec<_>>(),
            [1, 2]
        );
        assert_eq!(
            handoff
                .connections
                .iter()
                .map(|connection| (connection.system_id, connection.plan.edge.value()))
                .collect::<Vec<_>>(),
            [(1, 1), (2, 2)]
        );
        assert_eq!(handoff.systems[0].staff_ids, [1, 2]);
        assert_eq!(handoff.systems[1].staff_ids, [3, 4]);
    }

    #[test]
    fn handoff_preserves_detached_brace_outside_graph_and_peak_list() {
        use crate::staff_peak::StaffPeakAttribute;

        let top = peak(1, 10);
        let bottom = peak(2, 10);
        let mut brace = peak(1, 4);
        brace.set(StaffPeakAttribute::BraceTop);
        let state = BarsSystemState::new(
            1,
            vec![
                BarsStaffState::new(StaffId::new(1), 0, true, vec![top.clone()], BTreeMap::new())
                    .unwrap()
                    .with_brace_peak(brace.clone())
                    .unwrap(),
                BarsStaffState::new(
                    StaffId::new(2),
                    0,
                    true,
                    vec![bottom.clone()],
                    BTreeMap::new(),
                )
                .unwrap(),
            ],
            PeakGraph::new(),
        )
        .unwrap();
        let mut handoff = empty_handoff();

        append_system::<()>(&mut handoff, &state, &[], &[], 6, 10.0, true).unwrap();

        assert_eq!(handoff.systems[0].brace_peaks, [Some(brace), None]);
        assert_eq!(handoff.systems[0].staff_peaks[0], [top]);
        assert!(handoff.peak_graph.vertices().is_empty());
    }

    #[test]
    fn staff_join_rejects_kind_provenance_mismatch() {
        let prepared = PreparedStaffHandoff {
            staffs: vec![PreparedStaff {
                id: 1,
                kind: StaffCandidateKind::Standard,
                left: 0.0,
                right: 40.0,
                interline: 10,
                small: false,
                short: false,
                lines: Vec::new(),
            }],
        };
        let state = system(1, 1, 2);

        assert_eq!(
            validate_staff_join::<()>(&prepared, &[state]),
            Err(ProductionProcessBarsError::StaffKindMismatch(StaffId::new(
                1
            )))
        );
    }

    #[test]
    fn staff_join_reports_duplicate_before_generic_order_failure() {
        let prepared = PreparedStaffHandoff {
            staffs: (1..=3)
                .map(|id| PreparedStaff {
                    id,
                    kind: StaffCandidateKind::OneLine,
                    left: 0.0,
                    right: 40.0,
                    interline: 10,
                    small: false,
                    short: false,
                    lines: Vec::new(),
                })
                .collect(),
        };

        assert_eq!(
            validate_staff_join::<()>(&prepared, &[system(1, 1, 2), system(2, 2, 3)]),
            Err(ProductionProcessBarsError::DuplicateStaff(StaffId::new(2)))
        );
    }

    #[test]
    fn later_duplicate_graph_failure_preserves_completed_system_prefix() {
        let mut first = system(1, 1, 2);
        let first_result = process_bars_system(&mut first, parameters()).unwrap();
        let mut handoff = empty_handoff();
        append_system::<()>(
            &mut handoff,
            &first,
            first_result.vertical_inters(),
            first_result.connection_inters(),
            6,
            10.0,
            true,
        )
        .unwrap();

        let mut duplicate = system(2, 1, 2);
        let duplicate_result = process_bars_system(&mut duplicate, parameters()).unwrap();
        assert!(matches!(
            append_system::<()>(
                &mut handoff,
                &duplicate,
                duplicate_result.vertical_inters(),
                duplicate_result.connection_inters(),
                6,
                10.0,
                true,
            ),
            Err(ProductionProcessBarsError::Graph(_))
        ));
        assert_eq!(handoff.systems.len(), 1);
        assert_eq!(handoff.systems[0].system_id, 1);
        assert_eq!(handoff.peak_graph.edges().len(), 1);
        assert_eq!(handoff.connections.len(), 1);
    }
}
