// SPDX-License-Identifier: AGPL-3.0-or-later

//! Production-backed `CompleteLines` over prepared staff and raster state.
//!
//! Binary-buffer acquisition and horizontal-section dispatch are concrete.
//! The remaining geometry-heavy stages are explicit collaborators operating
//! on the same retained state. Java's inner `finally` is preserved: buffer
//! acquisition precedes it, so acquisition failure does not finish the inner
//! completion timer, while every later success or failure does.

use crate::{
    grid_lifecycle::{GridBuildStage, GridStageFailure},
    line_completion::{
        LineCompletionExecutor, LineCompletionStage, complete_lines as run_line_completion,
    },
    line_endpoints::PreparedStaffEndPoints,
    line_section_dispatch::dispatch_horizontal_sections,
    prepared_bars::{PreparedBarsHandoff, PreparedBarsStage},
    prepared_lines::{
        PreparedStaff, PreparedStaffHandoff, PreparedStaffStage, RawDiscardedFilament,
        RawLineMetadataHandoff, RawLineMetadataStage,
    },
    raster_grid_builder::{RasterGridBuildState, RemainingRasterGridStages},
    run_table::RunTable,
    section::Section,
};

#[derive(Clone, Debug)]
pub struct PreparedCompletionState {
    pub staffs: Vec<PreparedStaff>,
    /// Exact final candidates consumed by Java's
    /// `includeDiscardedFilaments` stage, in their pre-top-sort source order.
    pub discarded_filaments: Vec<RawDiscardedFilament>,
    pub horizontal_sections: Vec<Section>,
    pub binary_buffer: Option<RunTable>,
    pub thick_section_ids: Vec<usize>,
    pub thin_section_ids: Vec<usize>,
    /// Java `defineEndPoints` results in completed-staff order. A failure
    /// retains only the fully applied staff prefix.
    pub defined_endpoints: Vec<PreparedStaffEndPoints>,
    pub completed_stages: Vec<LineCompletionStage>,
}

pub trait RemainingLineCompletionStages {
    type StepError;
    type OtherError;

    fn run_stage(
        &mut self,
        stage: LineCompletionStage,
        state: &mut PreparedCompletionState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>>;

    fn log_swallowed_error(&mut self, error: &Self::OtherError);
    fn finish(&mut self);
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProductionCompleteLinesError<UpstreamError, CompletionError> {
    MissingPreparedStaffs,
    MissingHorizontalLag,
    BinaryBufferUnavailable,
    BinaryBufferProvenanceMismatch,
    Upstream(UpstreamError),
    Completion(CompletionError),
}

pub struct ProductionCompleteLines<Upstream, Completion> {
    upstream: Upstream,
    completion: Completion,
    prepared_binary: Option<RunTable>,
    maximum_thin_weight: usize,
    inspect_crossing_chunks: bool,
    state: Option<PreparedCompletionState>,
    raw_metadata: Option<RawLineMetadataHandoff>,
}

impl<Upstream, Completion> ProductionCompleteLines<Upstream, Completion> {
    #[must_use]
    pub fn new(
        upstream: Upstream,
        completion: Completion,
        prepared_binary: Option<RunTable>,
        maximum_thin_weight: usize,
        inspect_crossing_chunks: bool,
    ) -> Self {
        Self {
            upstream,
            completion,
            prepared_binary,
            maximum_thin_weight,
            inspect_crossing_chunks,
            state: None,
            raw_metadata: None,
        }
    }

    #[must_use]
    pub const fn state(&self) -> Option<&PreparedCompletionState> {
        self.state.as_ref()
    }

    #[must_use]
    pub const fn upstream(&self) -> &Upstream {
        &self.upstream
    }

    #[must_use]
    pub const fn completion(&self) -> &Completion {
        &self.completion
    }
}

impl<Upstream, Completion> PreparedStaffStage for ProductionCompleteLines<Upstream, Completion>
where
    Upstream: PreparedStaffStage,
{
    fn prepared_staff_handoff(&self) -> Option<&PreparedStaffHandoff> {
        self.upstream.prepared_staff_handoff()
    }

    fn take_prepared_staff_handoff(&mut self) -> Option<PreparedStaffHandoff> {
        if let Some(state) = self.state.as_ref() {
            return Some(PreparedStaffHandoff {
                staffs: state.staffs.clone(),
            });
        }
        self.upstream.take_prepared_staff_handoff()
    }
}

impl<Upstream, Completion> RawLineMetadataStage for ProductionCompleteLines<Upstream, Completion>
where
    Upstream: RawLineMetadataStage,
{
    fn take_raw_line_metadata_handoff(&mut self) -> Option<RawLineMetadataHandoff> {
        self.raw_metadata
            .take()
            .or_else(|| self.upstream.take_raw_line_metadata_handoff())
    }
}

impl<Upstream, Completion> PreparedBarsStage for ProductionCompleteLines<Upstream, Completion>
where
    Upstream: PreparedBarsStage,
{
    fn take_prepared_bars_handoff(&mut self) -> Option<PreparedBarsHandoff> {
        self.upstream.take_prepared_bars_handoff()
    }
}

impl<Upstream, Completion> RemainingRasterGridStages
    for ProductionCompleteLines<Upstream, Completion>
where
    Upstream: RemainingRasterGridStages + PreparedStaffStage + RawLineMetadataStage,
    Completion: RemainingLineCompletionStages<StepError = Upstream::StepError>,
{
    type StepError = Upstream::StepError;
    type OtherError = ProductionCompleteLinesError<Upstream::OtherError, Completion::OtherError>;

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
        state: &mut RasterGridBuildState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        self.upstream
            .process_bars(state)
            .map_err(map_upstream_failure)
    }

    fn complete_lines(
        &mut self,
        raster: &mut RasterGridBuildState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        let staffs = self
            .upstream
            .prepared_staff_handoff()
            .ok_or_else(|| {
                GridStageFailure::Other(ProductionCompleteLinesError::MissingPreparedStaffs)
            })?
            .staffs
            .clone();
        let horizontal_sections = raster
            .horizontal_lag()
            .ok_or_else(|| {
                GridStageFailure::Other(ProductionCompleteLinesError::MissingHorizontalLag)
            })?
            .sections()
            .to_vec();
        if self.raw_metadata.is_none() {
            self.raw_metadata = self.upstream.take_raw_line_metadata_handoff();
        }
        let discarded_filaments = self
            .raw_metadata
            .as_ref()
            .map_or_else(Vec::new, |metadata| {
                metadata.final_discarded_filaments.clone()
            });
        self.state = Some(PreparedCompletionState {
            staffs,
            discarded_filaments,
            horizontal_sections,
            binary_buffer: None,
            thick_section_ids: Vec::new(),
            thin_section_ids: Vec::new(),
            defined_endpoints: Vec::new(),
            completed_stages: Vec::new(),
        });
        let mut executor = CompletionAdapter {
            completion: &mut self.completion,
            state: self.state.as_mut().expect("just installed"),
            prepared_binary: self.prepared_binary.as_ref(),
            live_binary: raster.source(),
            maximum_thin_weight: self.maximum_thin_weight,
        };
        run_line_completion(&mut executor, self.inspect_crossing_chunks)
            .map_err(map_completion_failure)
    }

    fn log_swallowed_error(&mut self, stage: GridBuildStage, error: &Self::OtherError) {
        match error {
            ProductionCompleteLinesError::Upstream(error) => {
                self.upstream.log_swallowed_error(stage, error);
            }
            ProductionCompleteLinesError::Completion(error) => {
                self.completion.log_swallowed_error(error);
            }
            _ => {}
        }
    }

    fn finish(&mut self) {
        self.upstream.finish();
    }
}

struct CompletionAdapter<'a, Completion> {
    completion: &'a mut Completion,
    state: &'a mut PreparedCompletionState,
    prepared_binary: Option<&'a RunTable>,
    live_binary: &'a RunTable,
    maximum_thin_weight: usize,
}

impl<Completion> LineCompletionExecutor for CompletionAdapter<'_, Completion>
where
    Completion: RemainingLineCompletionStages,
{
    type Error = GridStageFailure<
        Completion::StepError,
        ProductionCompleteLinesError<std::convert::Infallible, Completion::OtherError>,
    >;

    fn load_binary_buffer(&mut self) -> Result<(), Self::Error> {
        let prepared = self.prepared_binary.ok_or_else(|| {
            GridStageFailure::Other(ProductionCompleteLinesError::BinaryBufferUnavailable)
        })?;
        if prepared != self.live_binary {
            return Err(GridStageFailure::Other(
                ProductionCompleteLinesError::BinaryBufferProvenanceMismatch,
            ));
        }
        self.state.binary_buffer = Some(prepared.clone());
        Ok(())
    }

    fn run_stage(&mut self, stage: LineCompletionStage) -> Result<(), Self::Error> {
        if stage == LineCompletionStage::DispatchHorizontalSections {
            let dispatch = dispatch_horizontal_sections(
                &self.state.horizontal_sections,
                self.maximum_thin_weight,
            );
            self.state.thick_section_ids =
                dispatch.thick.iter().map(|section| section.id()).collect();
            self.state.thin_section_ids =
                dispatch.thin.iter().map(|section| section.id()).collect();
        } else {
            self.completion
                .run_stage(stage, self.state)
                .map_err(|failure| match failure {
                    GridStageFailure::Step(error) => GridStageFailure::Step(error),
                    GridStageFailure::Other(error) => {
                        GridStageFailure::Other(ProductionCompleteLinesError::Completion(error))
                    }
                })?;
        }
        self.state.completed_stages.push(stage);
        Ok(())
    }

    fn finish(&mut self) {
        self.completion.finish();
    }
}

fn map_upstream_failure<StepError, UpstreamError, CompletionError>(
    failure: GridStageFailure<StepError, UpstreamError>,
) -> GridStageFailure<StepError, ProductionCompleteLinesError<UpstreamError, CompletionError>> {
    match failure {
        GridStageFailure::Step(error) => GridStageFailure::Step(error),
        GridStageFailure::Other(error) => {
            GridStageFailure::Other(ProductionCompleteLinesError::Upstream(error))
        }
    }
}

fn map_completion_failure<StepError, UpstreamError, CompletionError>(
    failure: GridStageFailure<
        StepError,
        ProductionCompleteLinesError<std::convert::Infallible, CompletionError>,
    >,
) -> GridStageFailure<StepError, ProductionCompleteLinesError<UpstreamError, CompletionError>> {
    match failure {
        GridStageFailure::Step(error) => GridStageFailure::Step(error),
        GridStageFailure::Other(error) => GridStageFailure::Other(match error {
            ProductionCompleteLinesError::MissingPreparedStaffs => {
                ProductionCompleteLinesError::MissingPreparedStaffs
            }
            ProductionCompleteLinesError::MissingHorizontalLag => {
                ProductionCompleteLinesError::MissingHorizontalLag
            }
            ProductionCompleteLinesError::BinaryBufferUnavailable => {
                ProductionCompleteLinesError::BinaryBufferUnavailable
            }
            ProductionCompleteLinesError::BinaryBufferProvenanceMismatch => {
                ProductionCompleteLinesError::BinaryBufferProvenanceMismatch
            }
            ProductionCompleteLinesError::Completion(error) => {
                ProductionCompleteLinesError::Completion(error)
            }
            ProductionCompleteLinesError::Upstream(never) => match never {},
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        filament::StaffFilament,
        grid_lifecycle::{GridBuildOutcome, build_grid_info},
        line_short_sections::HorizontalSectionLag,
        lines_coordinator::StaffCandidateKind,
        prepared_lines::PreparedStaffLine,
        raster_grid_builder::{
            HeadlessRasterGridBuilder, RasterGridOtherError, RasterGridParameters,
        },
        run_table::{Orientation, Run, create_grid_run_tables},
    };

    #[derive(Default)]
    struct Upstream {
        handoff: Option<PreparedStaffHandoff>,
        raw_metadata: Option<RawLineMetadataHandoff>,
        finish_count: usize,
    }

    impl RawLineMetadataStage for Upstream {
        fn take_raw_line_metadata_handoff(&mut self) -> Option<RawLineMetadataHandoff> {
            self.raw_metadata.take()
        }
    }

    impl PreparedStaffStage for Upstream {
        fn prepared_staff_handoff(&self) -> Option<&PreparedStaffHandoff> {
            self.handoff.as_ref()
        }

        fn take_prepared_staff_handoff(&mut self) -> Option<PreparedStaffHandoff> {
            self.handoff.take()
        }
    }

    impl RemainingRasterGridStages for Upstream {
        type StepError = &'static str;
        type OtherError = &'static str;

        fn retrieve_lines(
            &mut self,
            _state: &mut RasterGridBuildState,
        ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
            Ok(())
        }

        fn process_bars(
            &mut self,
            _state: &mut RasterGridBuildState,
        ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
            Ok(())
        }

        fn complete_lines(
            &mut self,
            _state: &mut RasterGridBuildState,
        ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
            panic!("production completion must replace the supplied stage")
        }

        fn log_swallowed_error(&mut self, _stage: GridBuildStage, _error: &Self::OtherError) {}

        fn finish(&mut self) {
            self.finish_count += 1;
        }
    }

    #[derive(Default)]
    struct Completion {
        fail_at: Option<(LineCompletionStage, bool)>,
        calls: Vec<LineCompletionStage>,
        warnings: Vec<&'static str>,
        observed_discarded_ids: Vec<usize>,
        finish_count: usize,
    }

    impl RemainingLineCompletionStages for Completion {
        type StepError = &'static str;
        type OtherError = &'static str;

        fn run_stage(
            &mut self,
            stage: LineCompletionStage,
            state: &mut PreparedCompletionState,
        ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
            self.calls.push(stage);
            if stage == LineCompletionStage::IncludeDiscardedFilaments {
                self.observed_discarded_ids = state
                    .discarded_filaments
                    .iter()
                    .map(|discarded| discarded.line.id)
                    .collect();
            }
            match self.fail_at {
                Some((failed, true)) if failed == stage => {
                    Err(GridStageFailure::Step("completion step failure"))
                }
                Some((failed, false)) if failed == stage => {
                    Err(GridStageFailure::Other("completion ordinary failure"))
                }
                _ => Ok(()),
            }
        }

        fn log_swallowed_error(&mut self, error: &Self::OtherError) {
            self.warnings.push(error);
        }

        fn finish(&mut self) {
            self.finish_count += 1;
        }
    }

    fn source() -> RunTable {
        let mut table = RunTable::new(Orientation::Vertical, 42, 20).unwrap();
        for x in 0..=40 {
            table.add_run(x, Run::new(10, 1)).unwrap();
        }
        table
    }

    fn raster_parameters() -> RasterGridParameters {
        RasterGridParameters {
            max_fore: 3,
            ledger_thickness: 1.0,
            minimum_horizontal_run_length: 4,
            maximum_vertical_run_shift: 1,
        }
    }

    fn prepared_staffs() -> PreparedStaffHandoff {
        let tables = create_grid_run_tables(&source(), 3, 1.0, 4).unwrap();
        let section = HorizontalSectionLag::from_long_runs(tables.long_horizontal)
            .unwrap()
            .sections()[0]
            .clone();
        let mut filament = StaffFilament::new(10).unwrap();
        filament.add_section(section).unwrap();
        PreparedStaffHandoff {
            staffs: vec![PreparedStaff {
                id: 1,
                kind: StaffCandidateKind::OneLine,
                left: 0.0,
                right: 40.0,
                interline: 10,
                small: false,
                short: false,
                lines: vec![PreparedStaffLine { id: 7, filament }],
            }],
        }
    }

    fn builder(
        completion: Completion,
        binary: Option<RunTable>,
    ) -> HeadlessRasterGridBuilder<ProductionCompleteLines<Upstream, Completion>> {
        let upstream = Upstream {
            handoff: Some(prepared_staffs()),
            ..Upstream::default()
        };
        HeadlessRasterGridBuilder::new(
            source(),
            raster_parameters(),
            ProductionCompleteLines::new(upstream, completion, binary, 20, true),
        )
    }

    fn builder_with_metadata(
        completion: Completion,
        binary: Option<RunTable>,
        metadata: RawLineMetadataHandoff,
    ) -> HeadlessRasterGridBuilder<ProductionCompleteLines<Upstream, Completion>> {
        let upstream = Upstream {
            handoff: Some(prepared_staffs()),
            raw_metadata: Some(metadata),
            ..Upstream::default()
        };
        HeadlessRasterGridBuilder::new(
            source(),
            raster_parameters(),
            ProductionCompleteLines::new(upstream, completion, binary, 20, true),
        )
    }

    #[test]
    fn success_loads_binary_dispatches_sections_and_finishes_inner_boundary() {
        let mut builder = builder(Completion::default(), Some(source()));

        assert_eq!(
            build_grid_info(&mut builder),
            Ok(GridBuildOutcome::Completed)
        );

        let stages = builder.stages();
        let state = stages.state().expect("retained completion state");
        assert_eq!(state.binary_buffer, Some(source()));
        assert_eq!(state.thick_section_ids, [1]);
        assert!(state.thin_section_ids.is_empty());
        assert_eq!(state.completed_stages.len(), 11);
        assert_eq!(stages.completion().finish_count, 1);
        assert_eq!(stages.upstream().finish_count, 1);
    }

    #[test]
    fn final_discarded_set_is_visible_only_at_completion_and_remains_handoff_safe() {
        let line = prepared_staffs().staffs[0].lines[0].clone();
        let metadata = RawLineMetadataHandoff {
            global_slope: 0.0,
            final_discarded_filaments: vec![RawDiscardedFilament {
                provenance: crate::prepared_lines::RawDiscardedFilamentProvenance::PrimaryDiscarded,
                line: line.clone(),
            }],
            sloped_filaments: Vec::new(),
        };
        let mut builder = builder_with_metadata(Completion::default(), Some(source()), metadata);

        assert_eq!(
            build_grid_info(&mut builder),
            Ok(GridBuildOutcome::Completed)
        );
        assert_eq!(
            builder.stages().state().unwrap().discarded_filaments[0]
                .line
                .id,
            line.id
        );
        assert_eq!(
            builder.stages().completion().observed_discarded_ids,
            [line.id]
        );
        let forwarded = builder
            .stages_mut()
            .take_raw_line_metadata_handoff()
            .expect("completion must retain metadata for sheet installation");
        assert_eq!(forwarded.final_discarded_filaments.len(), 1);
        assert!(forwarded.sloped_filaments.is_empty());
    }

    #[test]
    fn buffer_load_failure_precedes_inner_finally() {
        let mut builder = builder(Completion::default(), None);

        let outcome = build_grid_info(&mut builder).unwrap();

        assert!(matches!(
            outcome,
            GridBuildOutcome::Swallowed {
                stage: GridBuildStage::CompleteLines,
                error: RasterGridOtherError::Collaborator(
                    ProductionCompleteLinesError::BinaryBufferUnavailable
                ),
            }
        ));
        let stages = builder.stages();
        let state = stages.state().expect("state precedes buffer acquisition");
        assert!(state.binary_buffer.is_none());
        assert!(state.completed_stages.is_empty());
        assert_eq!(stages.completion().finish_count, 0);
        assert_eq!(stages.upstream().finish_count, 1);
    }

    #[test]
    fn inner_step_failure_runs_finally_and_aborts_outer_sequence() {
        let mut builder = builder(
            Completion {
                fail_at: Some((LineCompletionStage::IncludeThinSections, true)),
                ..Completion::default()
            },
            Some(source()),
        );

        assert_eq!(
            build_grid_info(&mut builder),
            Err("completion step failure")
        );

        let stages = builder.stages();
        assert_eq!(stages.completion().finish_count, 1);
        assert_eq!(stages.upstream().finish_count, 1);
        assert_eq!(
            stages.state().unwrap().completed_stages.last(),
            Some(&LineCompletionStage::IncludeThickSections)
        );
    }

    #[test]
    fn completion_failure_retains_final_discarded_prefix_before_first_stage() {
        let line = prepared_staffs().staffs[0].lines[0].clone();
        let metadata = RawLineMetadataHandoff {
            global_slope: 0.0,
            final_discarded_filaments: vec![RawDiscardedFilament {
                provenance: crate::prepared_lines::RawDiscardedFilamentProvenance::Sloped,
                line: line.clone(),
            }],
            sloped_filaments: vec![line],
        };
        let mut builder = builder_with_metadata(
            Completion {
                fail_at: Some((LineCompletionStage::DefineEndPoints, true)),
                ..Completion::default()
            },
            Some(source()),
            metadata,
        );

        assert_eq!(
            build_grid_info(&mut builder),
            Err("completion step failure")
        );
        let state = builder.stages().state().expect("completion state prefix");
        assert_eq!(state.discarded_filaments.len(), 1);
        assert!(state.completed_stages.is_empty());
        assert_eq!(builder.stages().completion().finish_count, 1);
    }

    #[test]
    fn inner_ordinary_failure_runs_finally_and_is_logged_by_outer_catch() {
        let mut builder = builder(
            Completion {
                fail_at: Some((LineCompletionStage::PolishCurvatures, false)),
                ..Completion::default()
            },
            Some(source()),
        );

        let outcome = build_grid_info(&mut builder).unwrap();

        assert!(matches!(
            outcome,
            GridBuildOutcome::Swallowed {
                stage: GridBuildStage::CompleteLines,
                ..
            }
        ));
        let stages = builder.stages();
        assert_eq!(
            stages.completion().warnings,
            ["completion ordinary failure"]
        );
        assert_eq!(stages.completion().finish_count, 1);
        assert_eq!(stages.upstream().finish_count, 1);
        assert_eq!(
            stages.state().unwrap().completed_stages.last(),
            Some(&LineCompletionStage::IncludeThinSections)
        );
    }
}
