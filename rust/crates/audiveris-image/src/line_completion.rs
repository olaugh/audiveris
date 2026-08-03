// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact stage lifecycle for Java `LinesRetriever.completeLines`.
//!
//! Concrete geometry and ownership remain in the individual helpers. This
//! module freezes their production order and deliberately retains partial
//! mutation when a stage fails.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineCompletionStage {
    DefineEndPoints,
    IncludeDiscardedFilaments,
    FillHolesInitial,
    DispatchHorizontalSections,
    IncludeThickSections,
    IncludeThinSections,
    PolishCurvatures,
    FillHolesAfterPolish,
    IncludeStickers,
    InspectCrossingChunks,
    FillHolesFinal,
}

pub trait LineCompletionExecutor {
    type Error;

    /// Java loads `Picture.SourceKey.BINARY` before entering its `try/finally`.
    fn load_binary_buffer(&mut self) -> Result<(), Self::Error>;

    fn run_stage(&mut self, stage: LineCompletionStage) -> Result<(), Self::Error>;

    /// Java's stopwatch-printing `finally` hook.
    fn finish(&mut self);
}

/// Execute the exact headless `completeLines` order.
pub fn complete_lines<Executor>(
    executor: &mut Executor,
    inspect_crossing_chunks: bool,
) -> Result<(), Executor::Error>
where
    Executor: LineCompletionExecutor,
{
    executor.load_binary_buffer()?;

    let stages = [
        LineCompletionStage::DefineEndPoints,
        LineCompletionStage::IncludeDiscardedFilaments,
        LineCompletionStage::FillHolesInitial,
        LineCompletionStage::DispatchHorizontalSections,
        LineCompletionStage::IncludeThickSections,
        LineCompletionStage::IncludeThinSections,
        LineCompletionStage::PolishCurvatures,
        LineCompletionStage::FillHolesAfterPolish,
        LineCompletionStage::IncludeStickers,
    ];

    let result = (|| {
        for stage in stages {
            executor.run_stage(stage)?;
        }
        if inspect_crossing_chunks {
            executor.run_stage(LineCompletionStage::InspectCrossingChunks)?;
        }
        executor.run_stage(LineCompletionStage::FillHolesFinal)
    })();
    executor.finish();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default)]
    struct RecordingExecutor {
        loaded: bool,
        calls: Vec<LineCompletionStage>,
        fail_load: bool,
        fail_at: Option<LineCompletionStage>,
        finished: usize,
    }

    impl LineCompletionExecutor for RecordingExecutor {
        type Error = &'static str;

        fn load_binary_buffer(&mut self) -> Result<(), Self::Error> {
            if self.fail_load {
                return Err("binary unavailable");
            }
            self.loaded = true;
            Ok(())
        }

        fn run_stage(&mut self, stage: LineCompletionStage) -> Result<(), Self::Error> {
            self.calls.push(stage);
            if self.fail_at == Some(stage) {
                Err("completion failed")
            } else {
                Ok(())
            }
        }

        fn finish(&mut self) {
            self.finished += 1;
        }
    }

    #[test]
    fn successful_completion_preserves_exact_java_order_and_optional_inspector() {
        let mut executor = RecordingExecutor::default();
        assert_eq!(complete_lines(&mut executor, true), Ok(()));
        assert!(executor.loaded);
        assert_eq!(executor.finished, 1);
        assert_eq!(
            executor.calls,
            [
                LineCompletionStage::DefineEndPoints,
                LineCompletionStage::IncludeDiscardedFilaments,
                LineCompletionStage::FillHolesInitial,
                LineCompletionStage::DispatchHorizontalSections,
                LineCompletionStage::IncludeThickSections,
                LineCompletionStage::IncludeThinSections,
                LineCompletionStage::PolishCurvatures,
                LineCompletionStage::FillHolesAfterPolish,
                LineCompletionStage::IncludeStickers,
                LineCompletionStage::InspectCrossingChunks,
                LineCompletionStage::FillHolesFinal,
            ]
        );

        let mut without_inspector = RecordingExecutor::default();
        complete_lines(&mut without_inspector, false).unwrap();
        assert!(
            !without_inspector
                .calls
                .contains(&LineCompletionStage::InspectCrossingChunks)
        );
        assert_eq!(
            without_inspector.calls.last(),
            Some(&LineCompletionStage::FillHolesFinal)
        );
    }

    #[test]
    fn stage_failure_runs_finally_and_retains_partial_mutation() {
        let mut executor = RecordingExecutor {
            fail_at: Some(LineCompletionStage::IncludeThinSections),
            ..RecordingExecutor::default()
        };

        assert_eq!(
            complete_lines(&mut executor, true),
            Err("completion failed")
        );
        assert_eq!(executor.finished, 1);
        assert_eq!(
            executor.calls.last(),
            Some(&LineCompletionStage::IncludeThinSections)
        );
        assert!(
            !executor
                .calls
                .contains(&LineCompletionStage::PolishCurvatures)
        );
    }

    #[test]
    fn binary_buffer_failure_happens_before_java_finally_scope() {
        let mut executor = RecordingExecutor {
            fail_load: true,
            ..RecordingExecutor::default()
        };

        assert_eq!(
            complete_lines(&mut executor, true),
            Err("binary unavailable")
        );
        assert!(!executor.loaded);
        assert!(executor.calls.is_empty());
        assert_eq!(executor.finished, 0);
    }
}
