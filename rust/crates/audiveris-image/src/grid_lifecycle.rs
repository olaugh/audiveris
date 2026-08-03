// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact outer lifecycle of Java `GridBuilder.buildInfo`.
//!
//! Unlike the headless recognition core, Java's outer method is intentionally
//! non-transactional: mutations completed before a failure remain visible.
//! A `StepException` is rethrown, any other `Throwable` is logged and swallowed,
//! and the stopwatch `finally` block always runs.

/// Java call order within `GridBuilder.buildInfo`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GridBuildStage {
    CreateBothLags,
    RetrieveLines,
    AddShortSections,
    ProcessBars,
    CompleteLines,
}

impl GridBuildStage {
    pub const ORDER: [Self; 5] = [
        Self::CreateBothLags,
        Self::RetrieveLines,
        Self::AddShortSections,
        Self::ProcessBars,
        Self::CompleteLines,
    ];

    /// Java stopwatch label, or the direct call name where no new watch task
    /// is started for `addShortSections`.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::CreateBothLags => "createBothLags",
            Self::RetrieveLines => "retrieveLines",
            Self::AddShortSections => "addShortSections",
            Self::ProcessBars => "retrieveBarlines",
            Self::CompleteLines => "completeLines",
        }
    }
}

/// Dynamic distinction made by Java's ordered catch clauses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GridStageFailure<StepError, OtherError> {
    Step(StepError),
    Other(OtherError),
}

/// Mutable adapter for the five concrete GridBuilder companions.
///
/// `run_stage` is allowed to mutate before returning an error. The lifecycle
/// runner never clones or rolls that state back.
pub trait GridBuildExecutor {
    type StepError;
    type OtherError;

    fn run_stage(
        &mut self,
        stage: GridBuildStage,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>>;

    /// Java's warning side effect from `catch (Throwable ex)`.
    fn log_swallowed_error(&mut self, stage: GridBuildStage, error: &Self::OtherError);

    /// Java's `finally` body (notably optional stopwatch printing).
    fn finish(&mut self);
}

/// Observable successful return from Java's void `buildInfo` method.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GridBuildOutcome<OtherError> {
    Completed,
    Swallowed {
        stage: GridBuildStage,
        error: OtherError,
    },
}

/// Execute `GridBuilder.buildInfo` with Java exception and mutation semantics.
///
/// A step failure is returned to the caller after `finish`. An ordinary failure
/// is logged, retained in the observable outcome, and otherwise swallowed.
pub fn build_grid_info<Executor>(
    executor: &mut Executor,
) -> Result<GridBuildOutcome<Executor::OtherError>, Executor::StepError>
where
    Executor: GridBuildExecutor,
{
    for stage in GridBuildStage::ORDER {
        match executor.run_stage(stage) {
            Ok(()) => {}
            Err(GridStageFailure::Step(error)) => {
                executor.finish();
                return Err(error);
            }
            Err(GridStageFailure::Other(error)) => {
                executor.log_swallowed_error(stage, &error);
                executor.finish();
                return Ok(GridBuildOutcome::Swallowed { stage, error });
            }
        }
    }
    executor.finish();
    Ok(GridBuildOutcome::Completed)
}

/// Production stages in Java `GridStep.doit`, outside `GridBuilder` itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridStepStage {
    BuildGrid,
    CleanStaffLines,
    UpdateBookScores,
}

impl GridStepStage {
    pub const ORDER: [Self; 3] = [
        Self::BuildGrid,
        Self::CleanStaffLines,
        Self::UpdateBookScores,
    ];
}

/// Adapter for the production `GRID` step driver.
pub trait GridStepExecutor {
    type Error;

    fn run_grid_step_stage(&mut self, stage: GridStepStage) -> Result<(), Self::Error>;

    /// Java prints its optional stopwatch only after all three stages succeed;
    /// there is no `finally` around `GridStep.doit`.
    fn finish_successfully(&mut self);
}

/// Execute the exact non-UI `GridStep.doit` lifecycle.
///
/// Failures propagate immediately and prior mutations are retained. No cleanup,
/// rollback, later stage, or success-finalization callback is run after failure.
pub fn run_grid_step<Executor>(executor: &mut Executor) -> Result<(), Executor::Error>
where
    Executor: GridStepExecutor,
{
    for stage in GridStepStage::ORDER {
        executor.run_grid_step_stage(stage)?;
    }
    executor.finish_successfully();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum FailureKind {
        Step,
        Other,
    }

    #[derive(Debug, Default)]
    struct RecordingExecutor {
        failure: Option<(GridBuildStage, FailureKind)>,
        calls: Vec<GridBuildStage>,
        mutations: Vec<&'static str>,
        warnings: Vec<(GridBuildStage, &'static str)>,
        finish_count: usize,
    }

    impl RecordingExecutor {
        fn failing(stage: GridBuildStage, kind: FailureKind) -> Self {
            Self {
                failure: Some((stage, kind)),
                ..Self::default()
            }
        }
    }

    impl GridBuildExecutor for RecordingExecutor {
        type StepError = &'static str;
        type OtherError = &'static str;

        fn run_stage(
            &mut self,
            stage: GridBuildStage,
        ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
            self.calls.push(stage);
            // Mutation deliberately precedes the failure, proving the outer
            // lifecycle does not provide transactional rollback.
            self.mutations.push(stage.label());
            match self.failure {
                Some((failed, FailureKind::Step)) if failed == stage => {
                    Err(GridStageFailure::Step("stop requested"))
                }
                Some((failed, FailureKind::Other)) if failed == stage => {
                    Err(GridStageFailure::Other("recognizer failure"))
                }
                _ => Ok(()),
            }
        }

        fn log_swallowed_error(&mut self, stage: GridBuildStage, error: &Self::OtherError) {
            self.warnings.push((stage, *error));
        }

        fn finish(&mut self) {
            self.finish_count += 1;
        }
    }

    #[test]
    fn successful_lifecycle_preserves_exact_java_call_order() {
        let mut executor = RecordingExecutor::default();

        assert_eq!(
            build_grid_info(&mut executor),
            Ok(GridBuildOutcome::Completed)
        );
        assert_eq!(executor.calls, GridBuildStage::ORDER);
        assert_eq!(
            executor.mutations,
            [
                "createBothLags",
                "retrieveLines",
                "addShortSections",
                "retrieveBarlines",
                "completeLines",
            ]
        );
        assert!(executor.warnings.is_empty());
        assert_eq!(executor.finish_count, 1);
    }

    #[test]
    fn step_exception_propagates_after_finally_without_rollback() {
        let mut executor =
            RecordingExecutor::failing(GridBuildStage::AddShortSections, FailureKind::Step);

        assert_eq!(build_grid_info(&mut executor), Err("stop requested"));
        assert_eq!(
            executor.calls,
            [
                GridBuildStage::CreateBothLags,
                GridBuildStage::RetrieveLines,
                GridBuildStage::AddShortSections,
            ]
        );
        assert_eq!(executor.mutations.len(), 3);
        assert!(executor.warnings.is_empty());
        assert_eq!(executor.finish_count, 1);
    }

    #[test]
    fn other_throwable_is_logged_swallowed_and_keeps_partial_mutation() {
        let mut executor =
            RecordingExecutor::failing(GridBuildStage::ProcessBars, FailureKind::Other);

        assert_eq!(
            build_grid_info(&mut executor),
            Ok(GridBuildOutcome::Swallowed {
                stage: GridBuildStage::ProcessBars,
                error: "recognizer failure",
            })
        );
        assert_eq!(
            executor.calls,
            [
                GridBuildStage::CreateBothLags,
                GridBuildStage::RetrieveLines,
                GridBuildStage::AddShortSections,
                GridBuildStage::ProcessBars,
            ]
        );
        assert_eq!(executor.mutations.len(), 4);
        assert_eq!(
            executor.warnings,
            [(GridBuildStage::ProcessBars, "recognizer failure")]
        );
        assert_eq!(executor.finish_count, 1);
    }

    #[derive(Debug, Default)]
    struct RecordingGridStep {
        calls: Vec<GridStepStage>,
        fail_at: Option<GridStepStage>,
        finishes: usize,
    }

    impl GridStepExecutor for RecordingGridStep {
        type Error = &'static str;

        fn run_grid_step_stage(&mut self, stage: GridStepStage) -> Result<(), Self::Error> {
            self.calls.push(stage);
            if self.fail_at == Some(stage) {
                Err("grid step failed")
            } else {
                Ok(())
            }
        }

        fn finish_successfully(&mut self) {
            self.finishes += 1;
        }
    }

    #[test]
    fn grid_step_runs_builder_cleaner_then_score_update() {
        let mut executor = RecordingGridStep::default();
        assert_eq!(run_grid_step(&mut executor), Ok(()));
        assert_eq!(executor.calls, GridStepStage::ORDER);
        assert_eq!(executor.finishes, 1);
    }

    #[test]
    fn grid_step_failure_propagates_without_rollback_or_success_finish() {
        let mut executor = RecordingGridStep {
            fail_at: Some(GridStepStage::CleanStaffLines),
            ..RecordingGridStep::default()
        };
        assert_eq!(run_grid_step(&mut executor), Err("grid step failed"));
        assert_eq!(
            executor.calls,
            [GridStepStage::BuildGrid, GridStepStage::CleanStaffLines]
        );
        assert_eq!(executor.finishes, 0);
    }
}
