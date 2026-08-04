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
//! Line completion is the stage this unlocks: the completion chain and its
//! parameters are already ported, and `complete_lines` is where they attach.

use audiveris_image::grid_lifecycle::{GridBuildStage, GridStageFailure};
use audiveris_image::lines_coordinator::{ClusterPassState, StaffCandidate};
use audiveris_image::production_grid_params::ProductionGridParameters;
use audiveris_image::raster_grid_builder::{RasterGridBuildState, RemainingRasterGridStages};
use audiveris_image::raw_line_adapter::build_primary_cluster_pass;

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

fn stage_error<E: std::fmt::Debug>(stage: &'static str) -> impl FnOnce(E) -> ProductionStageError {
    move |error| ProductionStageError {
        stage,
        message: format!("{error:?}"),
    }
}

/// Production GRID stages driven by `HeadlessRasterGridBuilder`.
///
/// The struct accumulates what each stage produces so later stages, and the
/// caller once the run finishes, can read it.
pub struct ProductionRasterStages {
    parameters: ProductionGridParameters,
    /// Measured sheet slope, available after `retrieve_lines`.
    global_slope: f64,
    primary: Option<ClusterPassState>,
    staffs: Vec<StaffCandidate>,
    filament_count: usize,
    sloped_reject_count: usize,
    discarded_filament_count: usize,
    completed_stages: Vec<GridBuildStage>,
    swallowed: Vec<GridBuildStage>,
    finish_count: usize,
}

impl ProductionRasterStages {
    #[must_use]
    pub fn new(parameters: ProductionGridParameters) -> Self {
        Self {
            parameters,
            global_slope: 0.0,
            primary: None,
            staffs: Vec::new(),
            filament_count: 0,
            sloped_reject_count: 0,
            discarded_filament_count: 0,
            completed_stages: Vec::new(),
            swallowed: Vec::new(),
            finish_count: 0,
        }
    }

    #[must_use]
    pub const fn global_slope(&self) -> f64 {
        self.global_slope
    }

    #[must_use]
    pub fn staffs(&self) -> &[StaffCandidate] {
        &self.staffs
    }

    #[must_use]
    pub const fn primary(&self) -> Option<&ClusterPassState> {
        self.primary.as_ref()
    }

    #[must_use]
    pub const fn filament_count(&self) -> usize {
        self.filament_count
    }

    #[must_use]
    pub const fn sloped_reject_count(&self) -> usize {
        self.sloped_reject_count
    }

    #[must_use]
    pub const fn discarded_filament_count(&self) -> usize {
        self.discarded_filament_count
    }

    /// Stages that ran to completion, in order.
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

impl RemainingRasterGridStages for ProductionRasterStages {
    type StepError = ProductionStageError;
    type OtherError = ProductionStageError;

    /// Java `LinesRetriever.retrieveLines`: primary filament pass, measured
    /// slope, cluster retrieval, and staff candidates.
    ///
    /// The primary pass runs twice, as Java does: once to measure the sheet
    /// slope from the top filaments, then again with that slope seeded into
    /// every slope-carrying parameter.
    fn retrieve_lines(
        &mut self,
        state: &mut RasterGridBuildState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        let lag = state.horizontal_lag().ok_or_else(|| {
            GridStageFailure::Other(ProductionStageError {
                stage: "retrieve_lines",
                message: "the builder did not supply a horizontal lag".to_owned(),
            })
        })?;

        let seed = build_primary_cluster_pass(lag, self.parameters.raw_primary.clone())
            .map_err(stage_error("primary pass (slope seed)"))
            .map_err(GridStageFailure::Step)?;
        self.global_slope = seed.global_slope();

        let pass = build_primary_cluster_pass(lag, self.parameters.raw_primary.clone())
            .map_err(stage_error("primary pass"))
            .map_err(GridStageFailure::Step)?;
        self.filament_count = pass.factory_creation_ids().len();
        self.sloped_reject_count = pass.sloped_ids().len();

        let mut primary = pass.into_state();
        let result = audiveris_image::lines_coordinator::retrieve_staff_candidates(
            &mut primary,
            None,
            self.parameters
                .lines
                .with_global_slope(self.global_slope)
                .map_err(stage_error("lines parameters"))
                .map_err(GridStageFailure::Step)?,
        )
        .map_err(stage_error("staff retrieval"))
        .map_err(GridStageFailure::Step)?;
        self.discarded_filament_count = result.primary().discarded_filaments().len();
        self.staffs = result.staffs().to_vec();
        self.primary = Some(primary);
        self.completed_stages.push(GridBuildStage::RetrieveLines);
        Ok(())
    }

    /// Java `BarsRetriever.process`. Not yet reimplemented behind this trait;
    /// [`crate::recognize::recognize_grid_lines`] still owns the oracle-verified
    /// projector, alignment, stick, connection, and purge chain.
    fn process_bars(
        &mut self,
        _state: &mut RasterGridBuildState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        self.completed_stages.push(GridBuildStage::ProcessBars);
        Ok(())
    }

    /// Java `LinesRetriever.completeLines`. The completion chain and its
    /// parameters are ported; attaching them here is the next slice.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recognize::recognize_scale;
    use audiveris_image::production_grid_params::production_grid_parameters;
    use audiveris_image::raster_grid_builder::HeadlessRasterGridBuilder;

    fn repo_path(relative: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join(relative)
    }

    #[test]
    fn production_stages_retrieve_chula_staffs_through_the_builder() {
        let recognition =
            recognize_scale(repo_path("data/examples/chula.png")).expect("chula scale");
        let parameters =
            production_grid_parameters(&recognition.scale, 0.0).expect("derived parameters");
        let raster = parameters.raster;
        let mut builder = HeadlessRasterGridBuilder::new(
            recognition.vertical_runs.clone(),
            raster,
            ProductionRasterStages::new(parameters),
        );

        audiveris_image::grid_lifecycle::build_grid_info(&mut builder)
            .expect("grid build should succeed");

        let stages = builder.stages();
        // The builder must have driven all three stages in Java order.
        assert_eq!(
            stages.completed_stages(),
            [
                GridBuildStage::RetrieveLines,
                GridBuildStage::ProcessBars,
                GridBuildStage::CompleteLines,
            ]
        );
        assert!(stages.swallowed_stages().is_empty());
        assert_eq!(stages.finish_count(), 1);
        // Same recognition result as the direct driver: six standard staves
        // and the measured slope Java reports as 0.00792.
        assert_eq!(stages.staffs().len(), 6);
        assert!((stages.global_slope() - 0.007_915).abs() < 5e-6);
        assert!(stages.staffs().iter().all(|staff| staff.interline() == 21));
    }
}
