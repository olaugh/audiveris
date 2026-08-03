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
        BarsCoordinatorError, BarsCoordinatorParameters, BarsSystemState, process_bars_system,
    },
    bars_logic::{ConnectionInterPlan, VerticalInterPlan},
    grid_lifecycle::{GridBuildStage, GridStageFailure},
    lines_coordinator::StaffCandidateKind,
    peak_graph::{PeakGraph, PeakGraphError},
    prepared_lines::{
        PreparedStaffHandoff, PreparedStaffStage, RawLineMetadataHandoff, RawLineMetadataStage,
    },
    raster_grid_builder::{
        HeadlessRasterGridBuilder, RasterGridBuildState, RemainingRasterGridStages,
    },
    staff_peak::StaffPeak,
};

#[derive(Clone, Debug)]
pub struct PreparedBarsSystem {
    pub system_id: usize,
    /// Exact Java `SystemInfo.getStaves()` traversal, expressed as stable
    /// staff identities. This cannot be reconstructed from `staff_peaks`: a
    /// valid staff may own no surviving bar peak.
    pub staff_ids: Vec<usize>,
    pub staff_peaks: Vec<Vec<StaffPeak>>,
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
    handoff: Option<PreparedBarsHandoff>,
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
            handoff: None,
        })
    }

    #[must_use]
    pub const fn upstream(&self) -> &Upstream {
        &self.upstream
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
        self.handoff = None;
        let staffs = self.upstream.prepared_staff_handoff().ok_or_else(|| {
            GridStageFailure::Other(ProductionProcessBarsError::MissingStaffHandoff)
        })?;
        validate_staff_join(staffs, &self.systems).map_err(GridStageFailure::Other)?;

        let mut handoff = PreparedBarsHandoff {
            systems: Vec::with_capacity(self.systems.len()),
            peak_graph: PeakGraph::new(),
            connections: Vec::new(),
        };
        for system in &mut self.systems {
            let system_id = system.system_id();
            let result = match process_bars_system(system, self.parameters) {
                Ok(result) => result,
                Err(source) => {
                    self.handoff = Some(handoff);
                    return Err(GridStageFailure::Other(ProductionProcessBarsError::Bars {
                        system_id,
                        source,
                    }));
                }
            };
            if let Err(error) = append_system(
                &mut handoff,
                system,
                result.vertical_inters(),
                result.connection_inters(),
                self.maximum_group_gap,
                f64::from(self.parameters.interline()),
            ) {
                self.handoff = Some(handoff);
                return Err(GridStageFailure::Other(error));
            }
        }
        self.handoff = Some(handoff);
        Ok(())
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
) -> Result<(), ProductionProcessBarsError<UpstreamError>> {
    let mut next = handoff.clone();
    append_system_in_place(
        &mut next,
        state,
        vertical_plans,
        connections,
        maximum_group_gap,
        interline,
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

        append_system::<()>(&mut handoff, &state, &[], &[], 6, 10.0).unwrap();

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
            ),
            Err(ProductionProcessBarsError::Graph(_))
        ));
        assert_eq!(handoff.systems.len(), 1);
        assert_eq!(handoff.systems[0].system_id, 1);
        assert_eq!(handoff.peak_graph.edges().len(), 1);
        assert_eq!(handoff.connections.len(), 1);
    }
}
