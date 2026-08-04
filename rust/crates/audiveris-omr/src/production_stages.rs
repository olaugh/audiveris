// SPDX-License-Identifier: AGPL-3.0-or-later

//! Production implementation of the raster GRID stage protocol.
//!
//! `HeadlessRasterGridBuilder` drives GRID through
//! [`RemainingRasterGridStages`], but until now the only implementation was the
//! `RasterStages` test double inside `grid_executor`'s test module, so the
//! executor path had never run outside tests. That is why
//! [`crate::recognize::recognize_grid_lines`] calls the subsystems directly.
//!
//! This module supplies the production implementation. The builder computes the
//! run-table partition and the initial lags itself, then hands control here for
//! `retrieveLines`, `processBars`, and `completeLines`, mirroring Java
//! `GridBuilder.buildInfo`.
//!
//! # The ported decorator chain
//!
//! The parity-correct shape is the chain that
//! `HeadlessGridExecutor::from_completed_raw_bars_complete_lines` composes:
//!
//! ```text
//! ProductionCompleteLines -> ProductionProcessBars -> RawProductionRetrieveLines -> terminal
//! ```
//!
//! Each decorator owns one stage and delegates the rest, so the innermost
//! element is a terminal that only records what reached it —
//! [`TerminalRasterStages`] here, the `RasterStages` double in
//! `grid_executor`'s tests.
//!
//! [`production_retrieve_lines`] builds the innermost two links today. The
//! remaining blocker for the outer two is `ProductionProcessBars::new`, which
//! takes an already-built `Vec<BarsSystemState>` rather than deriving one.
//! Those states need staff projectors, graded peaks, alignments, and
//! connections, none of which the chain produces.
//! [`crate::recognize::recognize_grid_lines`] does produce them, oracle-matched
//! against Java on every example page, so the migration is: keep that
//! derivation, feed its `BarsSystemState` values into `ProductionProcessBars`,
//! and let `ProductionCompleteLines` follow with the already-ported completion
//! chain.

use audiveris_image::grid_lifecycle::{GridBuildStage, GridStageFailure};
use audiveris_image::prepared_lines::RawProductionRetrieveLines;
use audiveris_image::production_grid_params::ProductionGridParameters;
use audiveris_image::raster_grid_builder::{RasterGridBuildState, RemainingRasterGridStages};

/// Failure raised by a production GRID stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionStageError {
    pub stage: &'static str,
    pub message: String,
}

impl std::fmt::Display for ProductionStageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GRID {} failed: {}", self.stage, self.message)
    }
}

impl std::error::Error for ProductionStageError {}

/// Terminal stage recorder for the production GRID chain.
///
/// The ported decorators (`RawProductionRetrieveLines`,
/// `ProductionProcessBars`, `ProductionCompleteLines`) each implement one
/// stage and delegate the rest, so the innermost element of the chain is a
/// terminal that only records what reached it. This is that terminal; the
/// `RasterStages` test double plays the same role inside `grid_executor`'s
/// tests.
#[derive(Debug, Default)]
pub struct TerminalRasterStages {
    completed_stages: Vec<GridBuildStage>,
    swallowed: Vec<GridBuildStage>,
    finish_count: usize,
}

impl TerminalRasterStages {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Stages that reached the terminal, in order.
    #[must_use]
    pub fn completed_stages(&self) -> &[GridBuildStage] {
        &self.completed_stages
    }

    #[must_use]
    pub fn swallowed_stages(&self) -> &[GridBuildStage] {
        &self.swallowed
    }

    #[must_use]
    pub const fn finish_count(&self) -> usize {
        self.finish_count
    }
}

impl RemainingRasterGridStages for TerminalRasterStages {
    type StepError = ProductionStageError;
    type OtherError = ProductionStageError;

    fn retrieve_lines(
        &mut self,
        _state: &mut RasterGridBuildState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        self.completed_stages.push(GridBuildStage::RetrieveLines);
        Ok(())
    }

    fn process_bars(
        &mut self,
        _state: &mut RasterGridBuildState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        self.completed_stages.push(GridBuildStage::ProcessBars);
        Ok(())
    }

    fn complete_lines(
        &mut self,
        _state: &mut RasterGridBuildState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        self.completed_stages.push(GridBuildStage::CompleteLines);
        Ok(())
    }

    fn log_swallowed_error(&mut self, stage: GridBuildStage, _error: &Self::OtherError) {
        self.swallowed.push(stage);
    }

    fn finish(&mut self) {
        self.finish_count += 1;
    }
}

/// Builds the ported `retrieveLines` stage over a terminal recorder.
///
/// This is the first link of the chain that
/// `HeadlessGridExecutor::from_completed_raw_bars_complete_lines` composes.
/// `ProductionProcessBars` and `ProductionCompleteLines` wrap this once the
/// derived `BarsSystemState` values are available.
#[must_use]
pub fn production_retrieve_lines(
    parameters: &ProductionGridParameters,
) -> RawProductionRetrieveLines<TerminalRasterStages> {
    RawProductionRetrieveLines::new(
        parameters.raw_primary.clone(),
        parameters.lines,
        TerminalRasterStages::new(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recognize::recognize_scale;
    use audiveris_image::prepared_lines::PreparedStaffStage;
    use audiveris_image::production_grid_params::production_grid_parameters;
    use audiveris_image::raster_grid_builder::HeadlessRasterGridBuilder;

    fn repo_path(relative: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join(relative)
    }

    #[test]
    fn ported_retrieve_lines_stage_runs_chula_through_the_builder() {
        let recognition =
            recognize_scale(repo_path("data/examples/chula.png")).expect("chula scale");
        let parameters =
            production_grid_parameters(&recognition.scale, 0.0).expect("derived parameters");
        let raster = parameters.raster;
        let mut builder = HeadlessRasterGridBuilder::new(
            recognition.vertical_runs.clone(),
            raster,
            production_retrieve_lines(&parameters),
        );

        audiveris_image::grid_lifecycle::build_grid_info(&mut builder)
            .expect("grid build should succeed");

        // `RawProductionRetrieveLines` owns retrieveLines and forwards the
        // other two stages to the terminal, so the terminal must see exactly
        // those, in Java order.
        let terminal = builder.stages().downstream();
        assert_eq!(
            terminal.completed_stages(),
            [GridBuildStage::ProcessBars, GridBuildStage::CompleteLines]
        );
        assert!(terminal.swallowed_stages().is_empty());

        // The ported stage published its prepared staffs: six for this page,
        // the same count the direct driver and Java both report.
        let staffs = builder
            .stages_mut()
            .take_prepared_staff_handoff()
            .expect("retrieveLines must publish its prepared staffs");
        assert_eq!(staffs.staffs.len(), 6);
    }
}
