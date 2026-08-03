// SPDX-License-Identifier: AGPL-3.0-or-later

//! Production-backed `RetrieveLines` for prepared cluster-pass inputs.
//!
//! The cluster coordinators are prepared upstream from the live horizontal
//! lag. This adapter verifies that provenance, executes
//! [`retrieve_staff_candidates`], and retains an owned staff/filament prefix
//! for the sheet-aware driver. The coordinator itself is transactional on
//! exceptional paths; this is deliberately not a claim of Java-compatible
//! failure-prefix semantics yet.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    filament::StaffFilament,
    grid_lifecycle::{GridBuildStage, GridStageFailure},
    line_cluster::FilamentId,
    lines_coordinator::{
        ClusterPass, ClusterPassState, LinesCoordinatorError, LinesCoordinatorParameters,
        StaffCandidateKind, preview_cluster_pass, retrieve_staff_candidates,
        retrieve_staff_candidates_with_secondary_order,
    },
    raster_grid_builder::{
        HeadlessRasterGridBuilder, RasterGridBuildState, RemainingRasterGridStages,
    },
    raw_line_adapter::{
        RawLineAdapterError, RawPrimaryPassParameters, RawSecondaryPassParameters,
        build_primary_cluster_pass, build_secondary_cluster_pass,
    },
    section::Section,
};

#[derive(Clone, Debug)]
pub struct PreparedStaffLine {
    pub id: usize,
    pub filament: StaffFilament,
}

#[derive(Clone, Debug)]
pub struct PreparedStaff {
    pub id: usize,
    pub kind: StaffCandidateKind,
    pub left: f64,
    pub right: f64,
    pub interline: usize,
    pub small: bool,
    pub short: bool,
    pub lines: Vec<PreparedStaffLine>,
}

#[derive(Clone, Debug)]
pub struct PreparedStaffHandoff {
    pub staffs: Vec<PreparedStaff>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RawDiscardedFilamentProvenance {
    PrimaryDiscarded,
    Sloped,
    SecondaryDiscarded,
}

/// One exact factory filament still eligible for Java's later
/// `includeDiscardedFilaments` fallback.
#[derive(Clone, Debug)]
pub struct RawDiscardedFilament {
    pub provenance: RawDiscardedFilamentProvenance,
    pub line: PreparedStaffLine,
}

/// Raw-line state retained after rejection for sheet skew and later fallback
/// inclusion. Curvature rejects are deliberately not represented.
#[derive(Clone, Debug)]
pub struct RawLineMetadataHandoff {
    pub global_slope: f64,
    /// Authoritative, ordered final fallback set.
    pub final_discarded_filaments: Vec<RawDiscardedFilament>,
    /// Compatibility projection consumed by the current sheet installer.
    /// Java keeps every original slope reject outside both cluster passes, so
    /// this is their exact original order.
    pub sloped_filaments: Vec<PreparedStaffLine>,
}

pub trait PreparedStaffHandoffSource {
    fn take_prepared_staff_handoff(&mut self) -> Option<PreparedStaffHandoff>;
}

pub trait RawLineMetadataHandoffSource {
    fn take_raw_line_metadata_handoff(&mut self) -> Option<RawLineMetadataHandoff>;
}

pub trait PreparedStaffStage {
    fn prepared_staff_handoff(&self) -> Option<&PreparedStaffHandoff>;
    fn take_prepared_staff_handoff(&mut self) -> Option<PreparedStaffHandoff>;
}

pub trait RawLineMetadataStage {
    fn take_raw_line_metadata_handoff(&mut self) -> Option<RawLineMetadataHandoff>;
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProductionRetrieveLinesError<DownstreamError> {
    MissingHorizontalLag,
    Raw(RawLineAdapterError),
    DuplicateLagSection(usize),
    Lines(LinesCoordinatorError),
    CandidateIdMismatch {
        expected: usize,
        actual: usize,
    },
    MissingCandidateCluster {
        candidate: usize,
    },
    CandidateLineIdsMismatch {
        candidate: usize,
    },
    DuplicateFilamentId(FilamentId),
    FilamentIdOverflow(FilamentId),
    UnknownLagSection {
        filament: FilamentId,
        section: usize,
    },
    LagSectionMismatch {
        filament: FilamentId,
        section: usize,
    },
    MissingDiscardedFilament(FilamentId),
    DuplicateDiscardedFilament(FilamentId),
    InterlineMismatch {
        candidate: usize,
        filament: FilamentId,
    },
    Downstream(DownstreamError),
}

/// `RetrieveLines` adapter which constructs its primary pass from the live lag.
///
/// This is additive to [`ProductionRetrieveLines`], whose prepared-state API is
/// retained for callers that own a prebuilt primary or secondary pass.
pub struct RawProductionRetrieveLines<Downstream> {
    raw_parameters: RawPrimaryPassParameters,
    lines_parameters: LinesCoordinatorParameters,
    downstream: Downstream,
    primary: Option<ClusterPassState>,
    secondary_parameters: Option<RawSecondaryPassParameters>,
    secondary: Option<ClusterPassState>,
    handoff: Option<PreparedStaffHandoff>,
    raw_metadata_handoff: Option<RawLineMetadataHandoff>,
}

impl<Downstream> RawProductionRetrieveLines<Downstream> {
    #[must_use]
    pub fn new(
        raw_parameters: RawPrimaryPassParameters,
        lines_parameters: LinesCoordinatorParameters,
        downstream: Downstream,
    ) -> Self {
        Self {
            raw_parameters,
            lines_parameters,
            downstream,
            primary: None,
            secondary_parameters: None,
            secondary: None,
            handoff: None,
            raw_metadata_handoff: None,
        }
    }

    /// Enable Java's lazy small-interline pass while retaining the existing
    /// primary-only constructor and behavior by default.
    #[must_use]
    pub fn with_small_interline(mut self, parameters: RawSecondaryPassParameters) -> Self {
        self.secondary_parameters = Some(parameters);
        self
    }

    #[must_use]
    pub const fn primary(&self) -> Option<&ClusterPassState> {
        self.primary.as_ref()
    }

    #[must_use]
    pub const fn secondary(&self) -> Option<&ClusterPassState> {
        self.secondary.as_ref()
    }

    #[must_use]
    pub const fn downstream(&self) -> &Downstream {
        &self.downstream
    }

    #[must_use]
    pub const fn raw_metadata_handoff(&self) -> Option<&RawLineMetadataHandoff> {
        self.raw_metadata_handoff.as_ref()
    }
}

/// Replace only the supplied `RetrieveLines` stage. Bars and completion remain
/// delegated until their production adapters are joined.
pub struct ProductionRetrieveLines<Downstream> {
    primary: ClusterPassState,
    secondary: Option<ClusterPassState>,
    parameters: LinesCoordinatorParameters,
    downstream: Downstream,
    handoff: Option<PreparedStaffHandoff>,
}

impl<Downstream> ProductionRetrieveLines<Downstream> {
    #[must_use]
    pub fn new(
        primary: ClusterPassState,
        secondary: Option<ClusterPassState>,
        parameters: LinesCoordinatorParameters,
        downstream: Downstream,
    ) -> Self {
        Self {
            primary,
            secondary,
            parameters,
            downstream,
            handoff: None,
        }
    }

    pub fn take_handoff(&mut self) -> Option<PreparedStaffHandoff> {
        self.handoff.take()
    }

    #[must_use]
    pub const fn downstream(&self) -> &Downstream {
        &self.downstream
    }
}

impl<Stages, Vip> PreparedStaffHandoffSource for HeadlessRasterGridBuilder<Stages, Vip>
where
    Stages: PreparedStaffStage,
{
    fn take_prepared_staff_handoff(&mut self) -> Option<PreparedStaffHandoff> {
        self.stages_mut().take_prepared_staff_handoff()
    }
}

impl<Stages, Vip> RawLineMetadataHandoffSource for HeadlessRasterGridBuilder<Stages, Vip>
where
    Stages: RawLineMetadataStage,
{
    fn take_raw_line_metadata_handoff(&mut self) -> Option<RawLineMetadataHandoff> {
        self.stages_mut().take_raw_line_metadata_handoff()
    }
}

impl<Downstream> PreparedStaffStage for ProductionRetrieveLines<Downstream> {
    fn prepared_staff_handoff(&self) -> Option<&PreparedStaffHandoff> {
        self.handoff.as_ref()
    }

    fn take_prepared_staff_handoff(&mut self) -> Option<PreparedStaffHandoff> {
        self.take_handoff()
    }
}

impl<Downstream> PreparedStaffStage for RawProductionRetrieveLines<Downstream> {
    fn prepared_staff_handoff(&self) -> Option<&PreparedStaffHandoff> {
        self.handoff.as_ref()
    }

    fn take_prepared_staff_handoff(&mut self) -> Option<PreparedStaffHandoff> {
        self.handoff.take()
    }
}

impl<Downstream> RawLineMetadataStage for RawProductionRetrieveLines<Downstream> {
    fn take_raw_line_metadata_handoff(&mut self) -> Option<RawLineMetadataHandoff> {
        self.raw_metadata_handoff.take()
    }
}

impl<Downstream> RemainingRasterGridStages for ProductionRetrieveLines<Downstream>
where
    Downstream: RemainingRasterGridStages,
{
    type StepError = Downstream::StepError;
    type OtherError = ProductionRetrieveLinesError<Downstream::OtherError>;

    fn retrieve_lines(
        &mut self,
        state: &mut RasterGridBuildState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        self.handoff = None;
        let lag = state.horizontal_lag().ok_or_else(|| {
            GridStageFailure::Other(ProductionRetrieveLinesError::MissingHorizontalLag)
        })?;
        let lag_sections = index_lag_sections(lag.sections()).map_err(GridStageFailure::Other)?;
        let result = if let Some(secondary) = self.secondary.as_mut() {
            retrieve_staff_candidates(&mut self.primary, Some(secondary), self.parameters)
        } else {
            retrieve_staff_candidates(&mut self.primary, None, self.parameters)
        }
        .map_err(|error| GridStageFailure::Other(ProductionRetrieveLinesError::Lines(error)))?;
        self.handoff = Some(
            materialize_staffs(
                result.staffs(),
                &self.primary,
                self.secondary.as_ref(),
                &lag_sections,
            )
            .map_err(GridStageFailure::Other)?,
        );
        Ok(())
    }

    fn process_bars(
        &mut self,
        state: &mut RasterGridBuildState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        self.downstream
            .process_bars(state)
            .map_err(map_downstream_failure)
    }

    fn complete_lines(
        &mut self,
        state: &mut RasterGridBuildState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        self.downstream
            .complete_lines(state)
            .map_err(map_downstream_failure)
    }

    fn log_swallowed_error(&mut self, stage: GridBuildStage, error: &Self::OtherError) {
        if let ProductionRetrieveLinesError::Downstream(error) = error {
            self.downstream.log_swallowed_error(stage, error);
        }
    }

    fn finish(&mut self) {
        self.downstream.finish();
    }
}

impl<Downstream> RemainingRasterGridStages for RawProductionRetrieveLines<Downstream>
where
    Downstream: RemainingRasterGridStages,
{
    type StepError = Downstream::StepError;
    type OtherError = ProductionRetrieveLinesError<Downstream::OtherError>;

    fn retrieve_lines(
        &mut self,
        state: &mut RasterGridBuildState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        self.handoff = None;
        self.raw_metadata_handoff = None;
        self.primary = None;
        self.secondary = None;
        let lag = state.horizontal_lag().ok_or_else(|| {
            GridStageFailure::Other(ProductionRetrieveLinesError::MissingHorizontalLag)
        })?;
        let lag_sections = index_lag_sections(lag.sections()).map_err(GridStageFailure::Other)?;
        let built = build_primary_cluster_pass(lag, self.raw_parameters.clone())
            .map_err(|error| GridStageFailure::Other(ProductionRetrieveLinesError::Raw(error)))?;
        let global_slope = built.global_slope();
        let retained_sloped = built.sloped_filaments().clone();
        let sloped_ids = built.sloped_ids().to_vec();
        let lines_parameters = self
            .lines_parameters
            .with_global_slope(global_slope)
            .map_err(|error| GridStageFailure::Other(ProductionRetrieveLinesError::Lines(error)))?;
        self.primary = Some(built.into_state());
        let primary = self.primary.as_mut().expect("raw pass was just installed");
        let result = if let Some(parameters) = self.secondary_parameters.clone() {
            let preview = preview_cluster_pass(primary).map_err(|error| {
                GridStageFailure::Other(ProductionRetrieveLinesError::Lines(error))
            })?;
            let secondary = build_secondary_cluster_pass(
                lag.run_table().width(),
                primary,
                preview.discarded_filaments(),
                parameters,
            )
            .map_err(|error| GridStageFailure::Other(ProductionRetrieveLinesError::Raw(error)))?;
            let secondary_order = secondary.filament_order().to_vec();
            self.secondary = Some(secondary);
            retrieve_staff_candidates_with_secondary_order(
                primary,
                self.secondary
                    .as_mut()
                    .expect("raw secondary pass was just installed"),
                &secondary_order,
                lines_parameters,
            )
        } else {
            retrieve_staff_candidates(primary, None, lines_parameters)
        }
        .map_err(|error| GridStageFailure::Other(ProductionRetrieveLinesError::Lines(error)))?;
        self.raw_metadata_handoff = Some(
            build_raw_metadata_handoff(
                global_slope,
                result.primary().discarded_filaments(),
                result
                    .secondary()
                    .map(crate::cluster_pipeline::ClusterRetrievalResult::discarded_filaments),
                primary,
                self.secondary.as_ref(),
                &sloped_ids,
                &retained_sloped,
            )
            .map_err(GridStageFailure::Other)?,
        );
        self.handoff = Some(
            materialize_staffs(
                result.staffs(),
                primary,
                self.secondary.as_ref(),
                &lag_sections,
            )
            .map_err(GridStageFailure::Other)?,
        );
        Ok(())
    }

    fn process_bars(
        &mut self,
        state: &mut RasterGridBuildState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        self.downstream
            .process_bars(state)
            .map_err(map_downstream_failure)
    }

    fn complete_lines(
        &mut self,
        state: &mut RasterGridBuildState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        self.downstream
            .complete_lines(state)
            .map_err(map_downstream_failure)
    }

    fn log_swallowed_error(&mut self, stage: GridBuildStage, error: &Self::OtherError) {
        if let ProductionRetrieveLinesError::Downstream(error) = error {
            self.downstream.log_swallowed_error(stage, error);
        }
    }

    fn finish(&mut self) {
        self.downstream.finish();
    }
}

fn build_raw_metadata_handoff<DownstreamError>(
    global_slope: f64,
    primary_discarded: &[FilamentId],
    secondary_discarded: Option<&[FilamentId]>,
    primary: &ClusterPassState,
    secondary: Option<&ClusterPassState>,
    sloped_ids: &[FilamentId],
    sloped: &BTreeMap<FilamentId, StaffFilament>,
) -> Result<RawLineMetadataHandoff, ProductionRetrieveLinesError<DownstreamError>> {
    let final_discarded_filaments = materialize_final_discarded_filaments(
        primary_discarded,
        secondary_discarded,
        primary,
        secondary,
        sloped_ids,
        sloped,
    )?;
    let sloped_filaments = final_discarded_filaments
        .iter()
        .filter(|discarded| discarded.provenance == RawDiscardedFilamentProvenance::Sloped)
        .map(|discarded| discarded.line.clone())
        .collect();
    Ok(RawLineMetadataHandoff {
        global_slope,
        final_discarded_filaments,
        sloped_filaments,
    })
}

fn materialize_final_discarded_filaments<DownstreamError>(
    primary_discarded: &[FilamentId],
    secondary_discarded: Option<&[FilamentId]>,
    primary: &ClusterPassState,
    secondary: Option<&ClusterPassState>,
    sloped_ids: &[FilamentId],
    sloped: &BTreeMap<FilamentId, StaffFilament>,
) -> Result<Vec<RawDiscardedFilament>, ProductionRetrieveLinesError<DownstreamError>> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    let mut append = |id: FilamentId,
                      provenance: RawDiscardedFilamentProvenance,
                      filaments: &BTreeMap<FilamentId, StaffFilament>|
     -> Result<(), ProductionRetrieveLinesError<DownstreamError>> {
        if !seen.insert(id) {
            return Err(ProductionRetrieveLinesError::DuplicateDiscardedFilament(id));
        }
        let filament = filaments
            .get(&id)
            .ok_or(ProductionRetrieveLinesError::MissingDiscardedFilament(id))?
            .clone();
        let raw_id = usize::try_from(id.value())
            .map_err(|_| ProductionRetrieveLinesError::FilamentIdOverflow(id))?;
        result.push(RawDiscardedFilament {
            provenance,
            line: PreparedStaffLine {
                id: raw_id,
                filament,
            },
        });
        Ok(())
    };

    if let Some(ids) = secondary_discarded {
        if !ids.is_empty() {
            let pass = secondary.ok_or(ProductionRetrieveLinesError::MissingDiscardedFilament(
                ids[0],
            ))?;
            for &id in ids {
                append(
                    id,
                    RawDiscardedFilamentProvenance::SecondaryDiscarded,
                    pass.filaments(),
                )?;
            }
        }
    } else {
        for &id in primary_discarded {
            append(
                id,
                RawDiscardedFilamentProvenance::PrimaryDiscarded,
                primary.filaments(),
            )?;
        }
    }
    // Java retains slope rejects outside both cluster passes and appends them
    // after the final discarded set before the stable top-ordinate sort.
    for &id in sloped_ids {
        append(id, RawDiscardedFilamentProvenance::Sloped, sloped)?;
    }
    Ok(result)
}

fn map_downstream_failure<StepError, DownstreamError>(
    failure: GridStageFailure<StepError, DownstreamError>,
) -> GridStageFailure<StepError, ProductionRetrieveLinesError<DownstreamError>> {
    match failure {
        GridStageFailure::Step(error) => GridStageFailure::Step(error),
        GridStageFailure::Other(error) => {
            GridStageFailure::Other(ProductionRetrieveLinesError::Downstream(error))
        }
    }
}

fn index_lag_sections<DownstreamError>(
    sections: &[Section],
) -> Result<BTreeMap<usize, &Section>, ProductionRetrieveLinesError<DownstreamError>> {
    let mut indexed = BTreeMap::new();
    for section in sections {
        if indexed.insert(section.id(), section).is_some() {
            return Err(ProductionRetrieveLinesError::DuplicateLagSection(
                section.id(),
            ));
        }
    }
    Ok(indexed)
}

fn materialize_staffs<DownstreamError>(
    candidates: &[crate::lines_coordinator::StaffCandidate],
    primary: &ClusterPassState,
    secondary: Option<&ClusterPassState>,
    lag_sections: &BTreeMap<usize, &Section>,
) -> Result<PreparedStaffHandoff, ProductionRetrieveLinesError<DownstreamError>> {
    let mut seen_filaments = BTreeSet::new();
    let mut staffs = Vec::with_capacity(candidates.len());
    for (index, candidate) in candidates.iter().enumerate() {
        let expected = index + 1;
        if candidate.id() != expected {
            return Err(ProductionRetrieveLinesError::CandidateIdMismatch {
                expected,
                actual: candidate.id(),
            });
        }
        let pass = match candidate.source().pass() {
            ClusterPass::Main => primary,
            ClusterPass::Small => {
                secondary.ok_or(ProductionRetrieveLinesError::MissingCandidateCluster {
                    candidate: candidate.id(),
                })?
            }
        };
        let cluster = pass.clusters().get(&candidate.source().cluster()).ok_or(
            ProductionRetrieveLinesError::MissingCandidateCluster {
                candidate: candidate.id(),
            },
        )?;
        let cluster_ids = cluster
            .lines()
            .map(|(_, line)| line.primary_id())
            .collect::<Vec<_>>();
        if cluster_ids != candidate.line_ids() {
            return Err(ProductionRetrieveLinesError::CandidateLineIdsMismatch {
                candidate: candidate.id(),
            });
        }
        let mut lines = Vec::with_capacity(cluster_ids.len());
        for (_, line) in cluster.lines() {
            let id = line.primary_id();
            if !seen_filaments.insert(id) {
                return Err(ProductionRetrieveLinesError::DuplicateFilamentId(id));
            }
            if candidate.source().pass() == ClusterPass::Main
                && line.filament().interline() != candidate.interline()
            {
                return Err(ProductionRetrieveLinesError::InterlineMismatch {
                    candidate: candidate.id(),
                    filament: id,
                });
            }
            for section in line.filament().sections() {
                let registered = lag_sections.get(&section.id()).ok_or(
                    ProductionRetrieveLinesError::UnknownLagSection {
                        filament: id,
                        section: section.id(),
                    },
                )?;
                if *registered != section {
                    return Err(ProductionRetrieveLinesError::LagSectionMismatch {
                        filament: id,
                        section: section.id(),
                    });
                }
            }
            lines.push(PreparedStaffLine {
                id: usize::try_from(id.value())
                    .map_err(|_| ProductionRetrieveLinesError::FilamentIdOverflow(id))?,
                filament: line.filament().clone(),
            });
        }
        staffs.push(PreparedStaff {
            id: candidate.id(),
            kind: candidate.kind(),
            left: candidate.left(),
            right: candidate.right(),
            interline: candidate.interline(),
            small: candidate.is_small(),
            short: candidate.is_short(),
            lines,
        });
    }
    Ok(PreparedStaffHandoff { staffs })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::{
        cluster_expand::ClusterExpansionParameters,
        cluster_merge::{ClusterMergeParameters, ClusterMergePassParameters},
        cluster_ownership::ClusterOwnership,
        cluster_pipeline::ClusterRetrievalParameters,
        filament_factory::{FilamentFactoryParams, OverlapParams},
        grid_lifecycle::{GridBuildOutcome, build_grid_info},
        raster_grid_builder::{RasterGridOtherError, RasterGridParameters},
        run_table::{Orientation, Run, RunTable},
    };

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Call {
        Bars,
        Complete,
    }

    #[derive(Default)]
    struct Downstream {
        calls: Vec<Call>,
        finish_count: usize,
    }

    impl RemainingRasterGridStages for Downstream {
        type StepError = &'static str;
        type OtherError = &'static str;

        fn retrieve_lines(
            &mut self,
            _state: &mut RasterGridBuildState,
        ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
            panic!("raw adapter owns RetrieveLines")
        }

        fn process_bars(
            &mut self,
            state: &mut RasterGridBuildState,
        ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
            assert!(state.short_sections_added());
            self.calls.push(Call::Bars);
            Ok(())
        }

        fn complete_lines(
            &mut self,
            state: &mut RasterGridBuildState,
        ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
            assert!(state.short_sections_added());
            self.calls.push(Call::Complete);
            Ok(())
        }

        fn log_swallowed_error(&mut self, _stage: GridBuildStage, _error: &Self::OtherError) {}

        fn finish(&mut self) {
            self.finish_count += 1;
        }
    }

    fn source() -> RunTable {
        let mut source = RunTable::new(Orientation::Vertical, 100, 61).unwrap();
        for x in 0..100 {
            source.add_run(x, Run::new(10, 1)).unwrap();
            source.add_run(x, Run::new(20, 1)).unwrap();
            if x <= 20 || x >= 40 {
                source.add_run(x, Run::new(30, 1)).unwrap();
            }
            source.add_run(x, Run::new(40, 1)).unwrap();
            source.add_run(x, Run::new(50, 1)).unwrap();
        }
        source
    }

    fn mixed_main_and_small_source() -> RunTable {
        let mut source = RunTable::new(Orientation::Vertical, 100, 111).unwrap();
        for x in 0..100 {
            for y in [10, 20, 30, 40, 50, 70, 75, 80, 85, 90] {
                source.add_run(x, Run::new(y, 1)).unwrap();
            }
        }
        source
    }

    fn raw_parameters() -> RawPrimaryPassParameters {
        let expansion = ClusterExpansionParameters::new(0.0, 0, 1, 0, 0.1, 1, 10).unwrap();
        let compatibility = ClusterMergeParameters::new(0.0, 0, 0.1, 0, 1, 10).unwrap();
        let merge = ClusterMergePassParameters::new(compatibility, 0, 1).unwrap();
        let retrieval = ClusterRetrievalParameters::new(
            10,
            BTreeSet::from([5]),
            expansion,
            merge,
            0.0,
            0.0,
            0.0,
            1,
            0,
            None,
            1,
        )
        .unwrap();
        RawPrimaryPassParameters {
            factory: FilamentFactoryParams {
                interline: 10,
                min_core_section_length: 1,
                min_section_aspect: 1.0,
                max_coord_gap: 5.0,
                max_pos_gap: 1.0,
                max_pos_gap_for_slope: 1.0,
                max_gap_slope: 0.1,
                min_length_for_delta_slope: 10.0,
                max_delta_slope: 0.1,
            },
            overlap: OverlapParams {
                probe_width: 2,
                max_overlap_delta_pos: 1.0,
                max_thickness: 2.0,
                max_overlap_space: 0.0,
                max_expansion_space: 0.0,
                max_involving_length: 10.0,
                max_consistent_ratio: 2.0,
            },
            sampling_dx: 20,
            minimum_delta_y: 9,
            maximum_delta_y: 11,
            retrieval,
        }
    }

    fn raster_parameters() -> RasterGridParameters {
        RasterGridParameters {
            max_fore: 2,
            ledger_thickness: 1.0,
            minimum_horizontal_run_length: 10,
            maximum_vertical_run_shift: 1,
        }
    }

    fn small_parameters() -> RawSecondaryPassParameters {
        let expansion = ClusterExpansionParameters::new(0.0, 0, 1, 0, 0.1, 1, 10).unwrap();
        let compatibility = ClusterMergeParameters::new(0.0, 0, 0.1, 0, 1, 10).unwrap();
        let merge = ClusterMergePassParameters::new(compatibility, 0, 1).unwrap();
        RawSecondaryPassParameters {
            sampling_dx: 20,
            minimum_delta_y: 4,
            maximum_delta_y: 6,
            retrieval: ClusterRetrievalParameters::new(
                5,
                BTreeSet::from([5]),
                expansion,
                merge,
                0.0,
                0.0,
                0.0,
                1,
                0,
                None,
                1,
            )
            .unwrap(),
        }
    }

    fn lines_parameters() -> LinesCoordinatorParameters {
        LinesCoordinatorParameters::new(0.0, 1, None, 1.0, 0.0, 100.0).unwrap()
    }

    fn synthetic_pass(
        values: impl IntoIterator<Item = (FilamentId, StaffFilament)>,
        interline: usize,
    ) -> ClusterPassState {
        let mut parameters = raw_parameters().retrieval;
        if interline != parameters.interline() {
            parameters = small_parameters().retrieval;
        }
        ClusterPassState::new(
            ClusterOwnership::new(),
            values.into_iter().collect(),
            BTreeMap::new(),
            Vec::new(),
            parameters,
        )
    }

    #[test]
    fn raster_lag_retrieve_lines_materializes_split_middle_staff_in_stage_order() {
        let stages = RawProductionRetrieveLines::new(
            raw_parameters(),
            lines_parameters(),
            Downstream::default(),
        );
        let mut builder = HeadlessRasterGridBuilder::new(source(), raster_parameters(), stages);

        assert_eq!(
            build_grid_info(&mut builder),
            Ok(GridBuildOutcome::Completed)
        );
        let handoff = builder
            .stages()
            .prepared_staff_handoff()
            .expect("prepared staff handoff");
        assert_eq!(handoff.staffs.len(), 1);
        assert_eq!(handoff.staffs[0].lines.len(), 5);
        let metadata = builder
            .stages()
            .raw_metadata_handoff()
            .expect("raw metadata handoff");
        assert_eq!(metadata.global_slope, 0.0);
        assert!(metadata.sloped_filaments.is_empty());
        assert_eq!(
            handoff.staffs[0]
                .lines
                .iter()
                .filter(|line| line.filament.sections().len() == 2)
                .count(),
            1
        );
        assert_eq!(
            builder.stages().downstream().calls,
            [Call::Bars, Call::Complete]
        );
        assert_eq!(builder.stages().downstream().finish_count, 1);
    }

    #[test]
    fn malformed_raw_parameters_keep_create_lag_prefix_and_stop_at_retrieve_lines() {
        let mut invalid = raw_parameters();
        invalid.sampling_dx = 0;
        let stages =
            RawProductionRetrieveLines::new(invalid, lines_parameters(), Downstream::default());
        let mut builder = HeadlessRasterGridBuilder::new(source(), raster_parameters(), stages);

        let outcome = build_grid_info(&mut builder).unwrap();
        assert!(matches!(
            outcome,
            GridBuildOutcome::Swallowed {
                stage: GridBuildStage::RetrieveLines,
                error: RasterGridOtherError::Collaborator(ProductionRetrieveLinesError::Raw(_)),
            }
        ));
        assert!(builder.state().run_tables().is_some());
        assert!(builder.state().initial_lags().is_some());
        assert!(builder.state().horizontal_lag().is_some());
        assert!(!builder.state().short_sections_added());
        assert!(builder.stages().primary().is_none());
        assert!(builder.stages().prepared_staff_handoff().is_none());
        assert!(builder.stages().raw_metadata_handoff().is_none());
        assert!(builder.stages().downstream().calls.is_empty());
        assert_eq!(builder.stages().downstream().finish_count, 1);
    }

    #[test]
    fn raw_small_pass_materializes_candidate_interline_with_original_filament_geometry() {
        let stages = RawProductionRetrieveLines::new(
            raw_parameters(),
            lines_parameters(),
            Downstream::default(),
        )
        .with_small_interline(small_parameters());
        let mut builder = HeadlessRasterGridBuilder::new(
            mixed_main_and_small_source(),
            raster_parameters(),
            stages,
        );

        assert_eq!(
            build_grid_info(&mut builder),
            Ok(GridBuildOutcome::Completed)
        );
        let handoff = builder
            .stages()
            .prepared_staff_handoff()
            .expect("mixed prepared staff handoff");
        assert_eq!(handoff.staffs.len(), 2);
        let main = handoff.staffs.iter().find(|staff| !staff.small).unwrap();
        let small = handoff.staffs.iter().find(|staff| staff.small).unwrap();
        assert_eq!(main.interline, 10);
        assert_eq!(main.lines.len(), 5);
        assert_eq!(small.interline, 5);
        assert_eq!(small.lines.len(), 5);
        assert!(
            small
                .lines
                .iter()
                .all(|line| line.filament.interline() == 10)
        );
        assert!(small.lines.iter().all(|line| {
            line.filament.sections().iter().all(|section| {
                builder
                    .state()
                    .horizontal_lag()
                    .unwrap()
                    .sections()
                    .iter()
                    .any(|source| source == section)
            })
        }));
        assert_eq!(
            builder
                .stages()
                .secondary()
                .unwrap()
                .parameters()
                .interline(),
            5
        );
        let metadata = builder.stages().raw_metadata_handoff().unwrap();
        assert!(metadata.final_discarded_filaments.is_empty());
        assert!(metadata.sloped_filaments.is_empty());
    }

    #[test]
    fn secondary_acceptance_does_not_consume_separate_slope_rejects() {
        let slope_id = FilamentId::new(8);
        let slope = StaffFilament::new(10).unwrap();
        let primary = synthetic_pass(Vec::new(), 10);
        let secondary = synthetic_pass(Vec::new(), 5);
        let metadata = build_raw_metadata_handoff::<&'static str>(
            0.25,
            &[],
            Some(&[]),
            &primary,
            Some(&secondary),
            &[slope_id],
            &BTreeMap::from([(slope_id, slope)]),
        )
        .unwrap();

        assert_eq!(metadata.global_slope, 0.25);
        assert_eq!(metadata.final_discarded_filaments.len(), 1);
        assert_eq!(metadata.final_discarded_filaments[0].line.id, 8);
        assert_eq!(
            metadata.final_discarded_filaments[0].provenance,
            RawDiscardedFilamentProvenance::Sloped
        );
        assert_eq!(metadata.sloped_filaments.len(), 1);
        assert_eq!(metadata.sloped_filaments[0].id, 8);
    }

    #[test]
    fn secondary_rejects_replace_original_classes_without_duplicates() {
        let primary_id = FilamentId::new(3);
        let slope_id = FilamentId::new(8);
        let primary_value = StaffFilament::new(10).unwrap();
        let slope = StaffFilament::new(10).unwrap();
        let primary = synthetic_pass([(primary_id, primary_value.clone())], 10);
        let secondary = synthetic_pass([(primary_id, primary_value)], 5);
        let metadata = build_raw_metadata_handoff::<&'static str>(
            0.0,
            &[primary_id],
            Some(&[primary_id]),
            &primary,
            Some(&secondary),
            &[slope_id],
            &BTreeMap::from([(slope_id, slope)]),
        )
        .unwrap();

        assert_eq!(metadata.final_discarded_filaments.len(), 2);
        assert_eq!(metadata.final_discarded_filaments[0].line.id, 3);
        assert_eq!(
            metadata.final_discarded_filaments[0].provenance,
            RawDiscardedFilamentProvenance::SecondaryDiscarded
        );
        assert_eq!(metadata.final_discarded_filaments[1].line.id, 8);
        assert_eq!(
            metadata.final_discarded_filaments[1].provenance,
            RawDiscardedFilamentProvenance::Sloped
        );
        assert_eq!(metadata.sloped_filaments.len(), 1);
        assert_eq!(metadata.sloped_filaments[0].id, 8);
    }

    #[test]
    fn no_secondary_preserves_java_primary_then_sloped_order_and_rejects_duplicates() {
        let primary_id = FilamentId::new(9);
        let slope_id = FilamentId::new(2);
        let primary_value = StaffFilament::new(10).unwrap();
        let slope = StaffFilament::new(10).unwrap();
        let primary = synthetic_pass([(primary_id, primary_value)], 10);
        let sloped = BTreeMap::from([(slope_id, slope.clone())]);
        let metadata = build_raw_metadata_handoff::<&'static str>(
            0.0,
            &[primary_id],
            None,
            &primary,
            None,
            &[slope_id],
            &sloped,
        )
        .unwrap();

        assert_eq!(
            metadata
                .final_discarded_filaments
                .iter()
                .map(|discarded| discarded.line.id)
                .collect::<Vec<_>>(),
            [9, 2]
        );
        assert_eq!(metadata.sloped_filaments.len(), 1);
        assert_eq!(metadata.sloped_filaments[0].id, 2);

        let duplicate = build_raw_metadata_handoff::<&'static str>(
            0.0,
            &[slope_id],
            None,
            &synthetic_pass([(slope_id, slope.clone())], 10),
            None,
            &[slope_id],
            &sloped,
        )
        .unwrap_err();
        assert_eq!(
            duplicate,
            ProductionRetrieveLinesError::DuplicateDiscardedFilament(slope_id)
        );
    }

    #[test]
    fn malformed_small_parameters_fail_after_primary_without_publishing_handoff() {
        let mut invalid = small_parameters();
        invalid.sampling_dx = 0;
        let stages = RawProductionRetrieveLines::new(
            raw_parameters(),
            lines_parameters(),
            Downstream::default(),
        )
        .with_small_interline(invalid);
        let mut builder = HeadlessRasterGridBuilder::new(
            mixed_main_and_small_source(),
            raster_parameters(),
            stages,
        );

        let outcome = build_grid_info(&mut builder).unwrap();
        assert!(matches!(
            outcome,
            GridBuildOutcome::Swallowed {
                stage: GridBuildStage::RetrieveLines,
                error: RasterGridOtherError::Collaborator(ProductionRetrieveLinesError::Raw(_)),
            }
        ));
        assert!(builder.stages().primary().is_some());
        assert!(builder.stages().secondary().is_none());
        assert!(builder.stages().prepared_staff_handoff().is_none());
        assert!(builder.stages().raw_metadata_handoff().is_none());
    }
}
