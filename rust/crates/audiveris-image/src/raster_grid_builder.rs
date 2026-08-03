// SPDX-License-Identifier: AGPL-3.0-or-later

//! Stage-owned raster implementation for the dependency-complete portions of
//! Java `GridBuilder.buildInfo`.
//!
//! `CreateBothLags` and `AddShortSections` are concrete. Line retrieval, bar
//! processing, and line completion remain bounded collaborators, but operate
//! on the same mutable state rather than on a transactional clone.
//! Prepared coordinators used by those collaborators may still implement
//! their own exceptional paths transactionally; they are not yet a claim of
//! Java-compatible failure semantics for the three supplied stages.

use crate::{
    grid_lifecycle::{GridBuildExecutor, GridBuildStage, GridStageFailure},
    line_short_sections::{
        AddShortSectionsError, HorizontalSectionLag, NoopVipSectionHook, VipSectionHook,
    },
    run_table::{GridRunTables, RunTable, RunTableError, create_grid_run_tables},
    section::{InitialGridLags, Section, build_initial_grid_lags},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RasterGridParameters {
    pub max_fore: usize,
    pub ledger_thickness: f64,
    pub minimum_horizontal_run_length: usize,
    pub maximum_vertical_run_shift: usize,
}

/// Intermediates owned by the live Java stage sequence.
#[derive(Clone, Debug)]
pub struct RasterGridBuildState {
    source: RunTable,
    run_tables: Option<GridRunTables>,
    initial_lags: Option<InitialGridLags>,
    horizontal_lag: Option<HorizontalSectionLag>,
    short_sections_added: bool,
}

impl RasterGridBuildState {
    #[must_use]
    pub fn source(&self) -> &RunTable {
        &self.source
    }

    #[must_use]
    pub const fn run_tables(&self) -> Option<&GridRunTables> {
        self.run_tables.as_ref()
    }

    #[must_use]
    pub const fn initial_lags(&self) -> Option<&InitialGridLags> {
        self.initial_lags.as_ref()
    }

    #[must_use]
    pub const fn horizontal_lag(&self) -> Option<&HorizontalSectionLag> {
        self.horizontal_lag.as_ref()
    }

    #[must_use]
    pub const fn short_sections_added(&self) -> bool {
        self.short_sections_added
    }
}

/// Completed lag ownership transferred into the concrete sheet executor.
#[derive(Clone, Debug, PartialEq)]
pub struct RasterGridHandoff {
    pub vertical_run_table: RunTable,
    pub vertical_sections: Vec<Section>,
    /// Absent only if horizontal-lag construction failed after the vertical
    /// prefix had already been installed.
    pub horizontal_lag: Option<HorizontalSectionLag>,
    /// Whether Java's later `addShortSections` stage completed.
    pub short_sections_added: bool,
}

/// Builder capability consumed by the sheet-aware `GRID` executor.
pub trait RasterGridHandoffSource {
    fn take_raster_grid_handoff(&mut self) -> Option<RasterGridHandoff>;
}

/// Supplied implementations for the three GRID stages whose complete raster
/// dependencies have not yet been joined.
pub trait RemainingRasterGridStages {
    type StepError;
    type OtherError;

    fn retrieve_lines(
        &mut self,
        state: &mut RasterGridBuildState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>>;

    fn process_bars(
        &mut self,
        state: &mut RasterGridBuildState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>>;

    fn complete_lines(
        &mut self,
        state: &mut RasterGridBuildState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>>;

    fn log_swallowed_error(&mut self, stage: GridBuildStage, error: &Self::OtherError);

    fn finish(&mut self);
}

#[derive(Clone, Debug, PartialEq)]
pub enum RasterGridOtherError<CollaboratorError> {
    CreateRunTables(RunTableError),
    CreateHorizontalLag(AddShortSectionsError),
    AddShortSections(AddShortSectionsError),
    Collaborator(CollaboratorError),
}

/// Concrete stage-oriented builder used directly by `build_grid_info`.
pub struct HeadlessRasterGridBuilder<Stages, Vip = NoopVipSectionHook> {
    parameters: RasterGridParameters,
    state: RasterGridBuildState,
    stages: Stages,
    vip: Vip,
}

impl<Stages> HeadlessRasterGridBuilder<Stages, NoopVipSectionHook> {
    #[must_use]
    pub fn new(source: RunTable, parameters: RasterGridParameters, stages: Stages) -> Self {
        Self::with_vip(source, parameters, stages, NoopVipSectionHook)
    }
}

impl<Stages, Vip> HeadlessRasterGridBuilder<Stages, Vip> {
    #[must_use]
    pub fn with_vip(
        source: RunTable,
        parameters: RasterGridParameters,
        stages: Stages,
        vip: Vip,
    ) -> Self {
        Self {
            parameters,
            state: RasterGridBuildState {
                source,
                run_tables: None,
                initial_lags: None,
                horizontal_lag: None,
                short_sections_added: false,
            },
            stages,
            vip,
        }
    }

    #[must_use]
    pub const fn state(&self) -> &RasterGridBuildState {
        &self.state
    }

    #[must_use]
    pub const fn stages(&self) -> &Stages {
        &self.stages
    }

    pub fn stages_mut(&mut self) -> &mut Stages {
        &mut self.stages
    }

    /// Move every successfully created lag prefix out after `buildInfo`.
    ///
    /// `None` means `CreateBothLags` failed before vertical ownership existed.
    /// A swallowed RetrieveLines or AddShortSections failure still returns the
    /// initial lags, with [`RasterGridHandoff::short_sections_added`] false.
    pub fn take_handoff(&mut self) -> Option<RasterGridHandoff> {
        let tables = self.state.run_tables.as_ref()?;
        let initial = self.state.initial_lags.as_ref()?;
        Some(RasterGridHandoff {
            vertical_run_table: tables.long_vertical.clone(),
            vertical_sections: initial.vertical.clone(),
            horizontal_lag: self.state.horizontal_lag.take(),
            short_sections_added: self.state.short_sections_added,
        })
    }
}

impl<Stages, Vip> RasterGridHandoffSource for HeadlessRasterGridBuilder<Stages, Vip> {
    fn take_raster_grid_handoff(&mut self) -> Option<RasterGridHandoff> {
        self.take_handoff()
    }
}

impl<Stages, Vip> GridBuildExecutor for HeadlessRasterGridBuilder<Stages, Vip>
where
    Stages: RemainingRasterGridStages,
    Vip: VipSectionHook,
{
    type StepError = Stages::StepError;
    type OtherError = RasterGridOtherError<Stages::OtherError>;

    fn run_stage(
        &mut self,
        stage: GridBuildStage,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        match stage {
            GridBuildStage::CreateBothLags => {
                let tables = create_grid_run_tables(
                    &self.state.source,
                    self.parameters.max_fore,
                    self.parameters.ledger_thickness,
                    self.parameters.minimum_horizontal_run_length,
                )
                .map_err(|error| {
                    GridStageFailure::Other(RasterGridOtherError::CreateRunTables(error))
                })?;
                // Retain each successfully produced Java prefix before the
                // next construction can fail.
                self.state.run_tables = Some(tables);
                let tables = self.state.run_tables.as_ref().expect("just installed");
                self.state.initial_lags = Some(build_initial_grid_lags(
                    tables,
                    self.parameters.maximum_vertical_run_shift,
                ));
                self.state.horizontal_lag = Some(
                    HorizontalSectionLag::from_long_runs(tables.long_horizontal.clone()).map_err(
                        |error| {
                            GridStageFailure::Other(RasterGridOtherError::CreateHorizontalLag(
                                error,
                            ))
                        },
                    )?,
                );
                Ok(())
            }
            GridBuildStage::RetrieveLines => self
                .stages
                .retrieve_lines(&mut self.state)
                .map_err(map_collaborator_failure),
            GridBuildStage::AddShortSections => {
                let tables = self
                    .state
                    .run_tables
                    .as_ref()
                    .expect("CreateBothLags precedes AddShortSections");
                self.state
                    .horizontal_lag
                    .as_mut()
                    .expect("CreateBothLags installed the horizontal lag")
                    .add_short_sections(&tables.short_horizontal, &mut self.vip)
                    .map_err(|error| {
                        GridStageFailure::Other(RasterGridOtherError::AddShortSections(error))
                    })?;
                self.state.short_sections_added = true;
                Ok(())
            }
            GridBuildStage::ProcessBars => self
                .stages
                .process_bars(&mut self.state)
                .map_err(map_collaborator_failure),
            GridBuildStage::CompleteLines => self
                .stages
                .complete_lines(&mut self.state)
                .map_err(map_collaborator_failure),
        }
    }

    fn log_swallowed_error(&mut self, stage: GridBuildStage, error: &Self::OtherError) {
        if let RasterGridOtherError::Collaborator(error) = error {
            self.stages.log_swallowed_error(stage, error);
        }
    }

    fn finish(&mut self) {
        self.stages.finish();
    }
}

fn map_collaborator_failure<StepError, OtherError>(
    failure: GridStageFailure<StepError, OtherError>,
) -> GridStageFailure<StepError, RasterGridOtherError<OtherError>> {
    match failure {
        GridStageFailure::Step(error) => GridStageFailure::Step(error),
        GridStageFailure::Other(error) => {
            GridStageFailure::Other(RasterGridOtherError::Collaborator(error))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        grid_lifecycle::{GridBuildOutcome, build_grid_info},
        run_table::{Orientation, Run},
    };

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Call {
        Retrieve,
        Bars,
        Complete,
    }

    #[derive(Default)]
    struct Stages {
        calls: Vec<Call>,
        fail: Option<(Call, bool)>,
        corrupt_short_table: bool,
        warnings: Vec<(GridBuildStage, &'static str)>,
        finish_count: usize,
    }

    impl RemainingRasterGridStages for Stages {
        type StepError = &'static str;
        type OtherError = &'static str;

        fn retrieve_lines(
            &mut self,
            state: &mut RasterGridBuildState,
        ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
            self.calls.push(Call::Retrieve);
            assert!(state.run_tables().is_some());
            assert!(state.horizontal_lag().is_some());
            assert!(!state.short_sections_added());
            if self.corrupt_short_table {
                state.run_tables.as_mut().expect("created").short_horizontal =
                    RunTable::new(Orientation::Horizontal, 2, 2).unwrap();
            }
            self.result(Call::Retrieve)
        }

        fn process_bars(
            &mut self,
            state: &mut RasterGridBuildState,
        ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
            self.calls.push(Call::Bars);
            assert!(state.short_sections_added());
            self.result(Call::Bars)
        }

        fn complete_lines(
            &mut self,
            state: &mut RasterGridBuildState,
        ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
            self.calls.push(Call::Complete);
            assert!(state.short_sections_added());
            self.result(Call::Complete)
        }

        fn log_swallowed_error(&mut self, stage: GridBuildStage, error: &Self::OtherError) {
            self.warnings.push((stage, *error));
        }

        fn finish(&mut self) {
            self.finish_count += 1;
        }
    }

    impl Stages {
        fn result(&self, call: Call) -> Result<(), GridStageFailure<&'static str, &'static str>> {
            match self.fail {
                Some((failed, true)) if failed == call => {
                    Err(GridStageFailure::Step("step failure"))
                }
                Some((failed, false)) if failed == call => {
                    Err(GridStageFailure::Other("ordinary failure"))
                }
                _ => Ok(()),
            }
        }
    }

    fn source() -> RunTable {
        let mut source = RunTable::new(Orientation::Vertical, 20, 12).unwrap();
        source.add_run(2, Run::new(1, 8)).unwrap();
        source.add_run(8, Run::new(2, 2)).unwrap();
        source.add_run(9, Run::new(2, 2)).unwrap();
        source
    }

    fn parameters() -> RasterGridParameters {
        RasterGridParameters {
            max_fore: 3,
            ledger_thickness: 1.0,
            minimum_horizontal_run_length: 4,
            maximum_vertical_run_shift: 1,
        }
    }

    #[test]
    fn concrete_stages_retain_intermediates_and_export_owned_lags() {
        let mut builder = HeadlessRasterGridBuilder::new(source(), parameters(), Stages::default());

        assert_eq!(
            build_grid_info(&mut builder),
            Ok(GridBuildOutcome::Completed)
        );

        assert_eq!(
            builder.stages().calls,
            [Call::Retrieve, Call::Bars, Call::Complete]
        );
        assert!(builder.state().short_sections_added());
        let expected_vertical = builder
            .state()
            .initial_lags()
            .expect("initial lags")
            .vertical
            .clone();
        let output = builder.take_handoff().expect("completed handoff");
        assert_eq!(output.vertical_sections, expected_vertical);
        assert!(
            !output
                .horizontal_lag
                .as_ref()
                .expect("horizontal lag")
                .sections()
                .is_empty()
        );
        assert!(output.short_sections_added);
        assert!(builder.state().horizontal_lag().is_none());
        assert_eq!(builder.stages().finish_count, 1);
    }

    #[test]
    fn retrieve_step_failure_keeps_create_prefix_and_skips_short_sections() {
        let stages = Stages {
            fail: Some((Call::Retrieve, true)),
            ..Stages::default()
        };
        let mut builder = HeadlessRasterGridBuilder::new(source(), parameters(), stages);

        assert_eq!(build_grid_info(&mut builder), Err("step failure"));

        assert!(builder.state().run_tables().is_some());
        assert!(builder.state().initial_lags().is_some());
        assert!(builder.state().horizontal_lag().is_some());
        assert!(!builder.state().short_sections_added());
        let output = builder.take_handoff().expect("created lag prefix");
        assert!(output.horizontal_lag.is_some());
        assert!(!output.short_sections_added);
        assert_eq!(builder.stages().calls, [Call::Retrieve]);
        assert_eq!(builder.stages().finish_count, 1);
    }

    #[test]
    fn add_short_failure_retains_initial_lag_and_is_swallowed_at_exact_stage() {
        let stages = Stages {
            corrupt_short_table: true,
            ..Stages::default()
        };
        let mut builder = HeadlessRasterGridBuilder::new(source(), parameters(), stages);

        assert_eq!(
            build_grid_info(&mut builder),
            Ok(GridBuildOutcome::Swallowed {
                stage: GridBuildStage::AddShortSections,
                error: RasterGridOtherError::AddShortSections(
                    AddShortSectionsError::IncompatibleRunTable
                ),
            })
        );

        assert!(builder.state().horizontal_lag().is_some());
        assert!(!builder.state().short_sections_added());
        let output = builder.take_handoff().expect("created lag prefix");
        assert!(output.horizontal_lag.is_some());
        assert!(!output.short_sections_added);
        assert_eq!(builder.stages().calls, [Call::Retrieve]);
        assert_eq!(builder.stages().finish_count, 1);
    }

    #[test]
    fn ordinary_bar_failure_is_wrapped_logged_and_retains_added_sections() {
        let stages = Stages {
            fail: Some((Call::Bars, false)),
            ..Stages::default()
        };
        let mut builder = HeadlessRasterGridBuilder::new(source(), parameters(), stages);

        assert_eq!(
            build_grid_info(&mut builder),
            Ok(GridBuildOutcome::Swallowed {
                stage: GridBuildStage::ProcessBars,
                error: RasterGridOtherError::Collaborator("ordinary failure"),
            })
        );

        assert!(builder.state().short_sections_added());
        assert_eq!(builder.stages().calls, [Call::Retrieve, Call::Bars]);
        assert_eq!(
            builder.stages().warnings,
            [(GridBuildStage::ProcessBars, "ordinary failure")]
        );
        assert_eq!(builder.stages().finish_count, 1);
    }

    #[test]
    fn swallowed_retrieve_failure_exports_initial_sheet_lag_prefix() {
        let stages = Stages {
            fail: Some((Call::Retrieve, false)),
            ..Stages::default()
        };
        let mut builder = HeadlessRasterGridBuilder::new(source(), parameters(), stages);

        assert_eq!(
            build_grid_info(&mut builder),
            Ok(GridBuildOutcome::Swallowed {
                stage: GridBuildStage::RetrieveLines,
                error: RasterGridOtherError::Collaborator("ordinary failure"),
            })
        );

        let output = builder.take_handoff().expect("created lag prefix");
        assert!(!output.vertical_sections.is_empty());
        assert!(output.horizontal_lag.is_some());
        assert!(!output.short_sections_added);
        assert_eq!(builder.stages().calls, [Call::Retrieve]);
        assert_eq!(builder.stages().finish_count, 1);
    }

    #[test]
    fn invalid_source_failure_leaves_all_created_state_absent() {
        let source = RunTable::new(Orientation::Horizontal, 10, 10).unwrap();
        let mut builder = HeadlessRasterGridBuilder::new(source, parameters(), Stages::default());

        assert_eq!(
            build_grid_info(&mut builder),
            Ok(GridBuildOutcome::Swallowed {
                stage: GridBuildStage::CreateBothLags,
                error: RasterGridOtherError::CreateRunTables(RunTableError::IncompatibleTable),
            })
        );

        assert!(builder.state().run_tables().is_none());
        assert!(builder.state().initial_lags().is_none());
        assert!(builder.state().horizontal_lag().is_none());
        assert!(builder.stages().calls.is_empty());
        assert_eq!(builder.stages().finish_count, 1);
    }
}
