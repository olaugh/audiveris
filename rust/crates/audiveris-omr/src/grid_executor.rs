// SPDX-License-Identifier: AGPL-3.0-or-later

//! Concrete headless state adapter for the production `GRID` lifecycle.
//!
//! The low-level line/bar builder remains a supplied implementation of
//! `GridBuildExecutor`, but all state after it is concrete: staff lines and
//! horizontal-lag pixels are mutated, systems receive section ownership and
//! pages, soft references share stable identity, and book score topology is
//! updated from the resulting `PageRef` objects.

use std::convert::Infallible;

use audiveris_image::{
    bar_alignment::BarAlignment,
    bars_logic::{BarsLogicError, ConnectionInterPlan, VerticalInterPlan},
    discarded_completion::PreparedCompletionSystem,
    filament::StaffFilament,
    grid_lifecycle::{
        GridBuildExecutor, GridBuildOutcome, GridStepExecutor, GridStepStage, build_grid_info,
        run_grid_step,
    },
    grid_sig::{
        BarGroupPromotionError, BarTailError, BarTailParameters, BarTailResult,
        ConnectionPromotionWarning, GridInterId, GridSig,
    },
    lag_rebuild::{RebuildHorizontalLagError, RegisteredHorizontalLag, rebuild_horizontal_lag},
    line_short_sections::NoopVipSectionHook,
    lines_coordinator::{LinesCoordinatorParameters, StaffCandidateKind},
    peak_graph::{PeakEdgeId, PeakGraph},
    prepared_bars::{PreparedBarsHandoff, PreparedBarsHandoffSource},
    prepared_completion::{
        ProductionCompleteLines, ProductionLineCompletion, ProductionLineCompletionParameters,
        production_line_completion,
    },
    prepared_lines::{
        PreparedStaffHandoff, PreparedStaffHandoffSource, PreparedStaffLine,
        RawLineMetadataHandoff, RawLineMetadataHandoffSource, RawProductionRetrieveLines,
    },
    raster_grid_builder::{
        HeadlessRasterGridBuilder, RasterGridHandoff, RasterGridHandoffSource,
        RasterGridParameters, RemainingRasterGridStages,
    },
    raw_line_adapter::RawPrimaryPassParameters,
    run_table::{RunTable, RunTableError},
    section::Section,
    staff_line_cleaner::{OriginalStaffLine, StaffLineCleanerExecutor, clean_staff_lines},
    staff_line_conversion::{
        OriginalGlyphRegistrar, PersistentStaffLine, StaffGlyph, StaffLineConversionError,
        to_registered_staff_line,
    },
    staff_peak::{PeakPoint, StaffPeak},
    system_population::{
        PopulationLag, PopulationPage, PopulationPageReport, PopulationReferencePage,
        PopulationReferenceRegistry, PopulationSection, PopulationSystem, PopulationSystemArea,
        PopulationSystemCounts, PopulationSystemGeometry, SystemPopulationExecutor,
        SystemSectionOwnership, SystemStaffBoundaries, allocate_population_pages,
        build_population_system_areas, check_population_indentation, dispatch_sections,
        populate_systems, population_page_report,
    },
};

use crate::score_update::{
    PageInput, PageKey, ScoreTopology, ScoreUpdateError, StubPages, update_scores,
};

#[derive(Clone, Debug)]
pub enum HeadlessStaffLine {
    Filament {
        line_id: usize,
        filament: StaffFilament,
    },
    Persistent {
        source_line_id: usize,
        line: PersistentStaffLine,
    },
}

#[derive(Clone, Debug)]
pub struct HeadlessStaff {
    pub id: usize,
    pub kind: StaffCandidateKind,
    pub left: f64,
    pub right: f64,
    pub interline: usize,
    pub small: bool,
    pub short: bool,
    pub barlines: Vec<GridInterId>,
    pub lines: Vec<HeadlessStaffLine>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HeadlessGlyphRegistry {
    pub originals: Vec<StaffGlyph>,
}

impl OriginalGlyphRegistrar for HeadlessGlyphRegistry {
    type Error = Infallible;

    fn register_original(&mut self, glyph: StaffGlyph) -> Result<StaffGlyph, Self::Error> {
        self.originals.push(glyph.clone());
        Ok(glyph)
    }
}

#[derive(Clone, Debug)]
pub struct HeadlessSystemSigState {
    pub system_id: usize,
    pub sig: GridSig,
    pub vertical_plans: Vec<VerticalInterPlan>,
    pub staff_peaks: Vec<Vec<StaffPeak>>,
    pub brace_peaks: Vec<Option<StaffPeak>>,
    pub maximum_group_gap: i32,
    pub interline: f64,
    pub bar_tail: BarTailResult,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeadlessConnectionPlan {
    pub system_id: usize,
    pub plan: ConnectionInterPlan,
}

#[derive(Clone, Debug)]
pub struct HeadlessGridSigState {
    pub systems: Vec<HeadlessSystemSigState>,
    /// Global `peakGraph` used by Java `createConnectionInters`.
    pub peak_graph: PeakGraph<BarAlignment>,
    /// Plans in the exact global `peakGraph.edgeSet()` iteration order.
    pub connections: Vec<HeadlessConnectionPlan>,
    pub connection_warnings: Vec<HeadlessConnectionWarning>,
}

impl HeadlessGridSigState {
    pub fn install_prepared_bars_handoff(&mut self, handoff: PreparedBarsHandoff) {
        self.install_prepared_bars_handoff_with_skew(handoff, None);
    }

    fn install_prepared_bars_handoff_with_skew(
        &mut self,
        mut handoff: PreparedBarsHandoff,
        skew: Option<&HeadlessSkew>,
    ) {
        if let Some(skew) = skew {
            for system in &mut handoff.systems {
                for peak in system.staff_peaks.iter_mut().flatten() {
                    peak.compute_deskewed_center(|point| skew.deskewed(point))
                        .expect("finite sheet skew produces finite peak coordinates");
                }
                for peak in system.brace_peaks.iter_mut().flatten() {
                    peak.compute_deskewed_center(|point| skew.deskewed(point))
                        .expect("finite sheet skew produces finite peak coordinates");
                }
            }
            let keys = handoff
                .peak_graph
                .vertices()
                .iter()
                .map(StaffPeak::key)
                .collect::<Vec<_>>();
            for key in keys {
                handoff
                    .peak_graph
                    .vertex_mut(key)
                    .expect("key was collected from this peak graph")
                    .compute_deskewed_center(|point| skew.deskewed(point))
                    .expect("finite sheet skew produces finite peak coordinates");
            }
        }
        self.systems = handoff
            .systems
            .into_iter()
            .map(|system| HeadlessSystemSigState {
                system_id: system.system_id,
                sig: GridSig::default(),
                vertical_plans: system.vertical_plans,
                staff_peaks: system.staff_peaks,
                brace_peaks: system.brace_peaks,
                maximum_group_gap: system.maximum_group_gap,
                interline: system.interline,
                bar_tail: BarTailResult::default(),
            })
            .collect();
        self.peak_graph = handoff.peak_graph;
        self.connections = handoff
            .connections
            .into_iter()
            .map(|connection| HeadlessConnectionPlan {
                system_id: connection.system_id,
                plan: connection.plan,
            })
            .collect();
        self.connection_warnings.clear();
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HeadlessConnectionWarning {
    Endpoint {
        system_id: usize,
        warning: ConnectionPromotionWarning,
    },
    Logic {
        system_id: usize,
        edge: PeakEdgeId,
        error: BarsLogicError,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeadlessGridPromotionError {
    MissingSystem(usize),
    BarGroup {
        system_id: usize,
        source: BarGroupPromotionError,
    },
    BarTail {
        system_id: usize,
        source: BarTailError,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HeadlessBuildOtherError<BuilderError> {
    Builder(BuilderError),
    Promotion(HeadlessGridPromotionError),
}

#[derive(Clone, Debug)]
pub struct HeadlessPopulationState {
    pub sheet_width: i32,
    pub sheet_height: i32,
    pub vertical_margin: i32,
    pub minimum_indentation: f64,
    pub geometries: Vec<PopulationSystemGeometry>,
    pub staff_boundaries: Vec<SystemStaffBoundaries>,
    pub vertical_sections: Vec<PopulationSection>,
    pub section_ownership: Vec<SystemSectionOwnership>,
    pub systems: Vec<PopulationSystem>,
    pub areas: Vec<PopulationSystemArea>,
    pub staff_areas_computed: Vec<usize>,
    pub pages: Vec<PopulationPage>,
    pub page_refs: Vec<PopulationReferencePage>,
    pub references: PopulationReferenceRegistry,
    pub reports: Vec<PopulationPageReport>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeadlessVerticalLag {
    pub run_table: RunTable,
    pub sections: Vec<Section>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InstalledRasterGridPrefix {
    pub short_sections_added: bool,
    pub horizontal_lag_present: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeadlessSkew {
    pub slope: f64,
    cosine: f64,
    sine: f64,
    translate_x: f64,
    translate_y: f64,
    deskewed_width: f64,
    deskewed_height: f64,
}

impl HeadlessSkew {
    /// Reproduce Java `Skew.initTransients`, including `AffineTransform`'s
    /// post-concatenated origin translation.
    #[must_use]
    pub fn new(slope: f64, sheet_width: i32, sheet_height: i32) -> Self {
        let deskew_angle = -slope.atan();
        let cosine = deskew_angle.cos();
        let sine = deskew_angle.sin();
        let width = f64::from(sheet_width);
        let height = f64::from(sheet_height);
        let rotate = |x: f64, y: f64| ((cosine * x) - (sine * y), (sine * x) + (cosine * y));
        let top_right = rotate(width, 0.0);
        let bottom_left = rotate(0.0, height);
        let bottom_right = rotate(width, height);
        let (dx, dy, deskewed_width, deskewed_height) = if deskew_angle <= 0.0 {
            let dy = -top_right.1;
            (0.0, dy, bottom_right.0, bottom_left.1 + dy)
        } else {
            let dx = -bottom_left.0;
            (dx, 0.0, top_right.0 + dx, bottom_right.1)
        };
        let (translate_x, translate_y) = rotate(dx, dy);
        Self {
            slope,
            cosine,
            sine,
            translate_x,
            translate_y,
            deskewed_width,
            deskewed_height,
        }
    }

    #[must_use]
    pub fn deskewed(&self, point: PeakPoint) -> PeakPoint {
        PeakPoint::new(
            (self.cosine * point.x) - (self.sine * point.y) + self.translate_x,
            (self.sine * point.x) + (self.cosine * point.y) + self.translate_y,
        )
    }

    /// Apply Java `Skew.skewed`, the inverse of the stored deskew transform.
    #[must_use]
    pub fn skewed(&self, point: PeakPoint) -> PeakPoint {
        let x = point.x - self.translate_x;
        let y = point.y - self.translate_y;
        PeakPoint::new(
            (self.cosine * x) + (self.sine * y),
            (-self.sine * x) + (self.cosine * y),
        )
    }

    #[must_use]
    pub fn deskewed_x(&self, x: f64, y: f64) -> f64 {
        self.deskewed(PeakPoint::new(x, y)).x
    }

    #[must_use]
    pub const fn deskewed_width(&self) -> f64 {
        self.deskewed_width
    }

    #[must_use]
    pub const fn deskewed_height(&self) -> f64 {
        self.deskewed_height
    }
}

#[derive(Clone, Debug)]
pub struct HeadlessGridSheet {
    pub sheet_number: u32,
    pub staffs: Vec<HeadlessStaff>,
    pub glyphs: HeadlessGlyphRegistry,
    pub sig: HeadlessGridSigState,
    pub promotion_failure: Option<HeadlessGridPromotionError>,
    pub no_staff_table: Option<RunTable>,
    pub max_fore: Option<usize>,
    pub ledger_thickness: f64,
    pub vertical_lag: Option<HeadlessVerticalLag>,
    pub horizontal_lag: Option<RegisteredHorizontalLag>,
    pub installed_raster_prefix: Option<InstalledRasterGridPrefix>,
    pub skew: Option<HeadlessSkew>,
    pub sloped_line_fallbacks: Vec<PreparedStaffLine>,
    pub population: HeadlessPopulationState,
}

impl HeadlessGridSheet {
    pub fn install_raw_line_metadata_handoff(&mut self, handoff: RawLineMetadataHandoff) {
        install_raw_line_metadata(
            handoff,
            &mut self.skew,
            &mut self.sloped_line_fallbacks,
            &mut self.population.geometries,
            self.population.sheet_width,
            self.population.sheet_height,
        );
    }

    pub fn install_prepared_staff_handoff(&mut self, handoff: PreparedStaffHandoff) {
        self.staffs = handoff
            .staffs
            .into_iter()
            .map(|staff| HeadlessStaff {
                id: staff.id,
                kind: staff.kind,
                left: staff.left,
                right: staff.right,
                interline: staff.interline,
                small: staff.small,
                short: staff.short,
                barlines: Vec::new(),
                lines: staff
                    .lines
                    .into_iter()
                    .map(|line| HeadlessStaffLine::Filament {
                        line_id: line.id,
                        filament: line.filament,
                    })
                    .collect(),
            })
            .collect();
    }

    /// Transfer the builder-owned lag prefix into sheet ownership before
    /// `StaffLineCleaner`, including prefixes from swallowed build failures.
    pub fn install_raster_grid_handoff(&mut self, handoff: RasterGridHandoff) {
        let horizontal_lag_present = handoff.horizontal_lag.is_some();
        self.vertical_lag = Some(HeadlessVerticalLag {
            run_table: handoff.vertical_run_table,
            sections: handoff.vertical_sections.clone(),
        });
        self.population.vertical_sections = handoff
            .vertical_sections
            .iter()
            .map(|section| {
                let (centroid_x, centroid_y) = section
                    .pixel_centroid_in(section.bounds())
                    .expect("a registered vertical section contains pixels");
                PopulationSection {
                    id: section.id(),
                    centroid_x,
                    centroid_y,
                }
            })
            .collect();
        self.horizontal_lag = handoff
            .horizontal_lag
            .map(RegisteredHorizontalLag::Populated);
        self.installed_raster_prefix = Some(InstalledRasterGridPrefix {
            short_sections_added: handoff.short_sections_added,
            horizontal_lag_present,
        });
    }
}

fn install_raw_line_metadata(
    handoff: RawLineMetadataHandoff,
    skew_state: &mut Option<HeadlessSkew>,
    sloped_line_fallbacks: &mut Vec<PreparedStaffLine>,
    geometries: &mut [PopulationSystemGeometry],
    sheet_width: i32,
    sheet_height: i32,
) {
    let skew = HeadlessSkew::new(handoff.global_slope, sheet_width, sheet_height);
    apply_skew_to_population_geometries(&skew, geometries);
    *skew_state = Some(skew);
    *sloped_line_fallbacks = handoff.sloped_filaments;
}

fn apply_skew_to_population_geometries(
    skew: &HeadlessSkew,
    geometries: &mut [PopulationSystemGeometry],
) {
    for geometry in geometries {
        geometry.deskewed_upper_left_x =
            skew.deskewed_x(f64::from(geometry.left), f64::from(geometry.top));
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeadlessGridBook {
    pub stubs: Vec<StubPages>,
    pub scores: Vec<ScoreTopology>,
}

#[derive(Debug, PartialEq)]
pub enum HeadlessGridError<StepError> {
    Build(StepError),
    MissingStaff(usize),
    PersistentStaffLine { staff_id: usize, line_id: usize },
    StaffLineConversion(StaffLineConversionError<Infallible>),
    MissingHorizontalLag,
    RemoveStaffSections(RunTableError),
    RebuildHorizontalLag(RebuildHorizontalLagError),
    ScoreUpdate(ScoreUpdateError),
}

/// Stateful executor for one Java `GridStep.doit` invocation.
pub struct HeadlessGridExecutor<Builder>
where
    Builder: GridBuildExecutor,
{
    pub builder: Builder,
    pub sheet: HeadlessGridSheet,
    pub book: HeadlessGridBook,
    pub build_outcome: Option<GridBuildOutcome<HeadlessBuildOtherError<Builder::OtherError>>>,
    raster_handoff: Option<fn(&mut Builder) -> Option<RasterGridHandoff>>,
    staff_handoff: Option<fn(&mut Builder) -> Option<PreparedStaffHandoff>>,
    raw_line_metadata_handoff: Option<fn(&mut Builder) -> Option<RawLineMetadataHandoff>>,
    bars_handoff: Option<fn(&mut Builder) -> Option<PreparedBarsHandoff>>,
    bar_tail_parameters: BarTailParameters,
    pub cleaner_finished: bool,
    pub step_finished: bool,
}

impl<Builder> HeadlessGridExecutor<Builder>
where
    Builder: GridBuildExecutor,
{
    #[must_use]
    pub fn new(builder: Builder, sheet: HeadlessGridSheet, book: HeadlessGridBook) -> Self {
        Self {
            builder,
            sheet,
            book,
            build_outcome: None,
            raster_handoff: None,
            staff_handoff: None,
            raw_line_metadata_handoff: None,
            bars_handoff: None,
            bar_tail_parameters: BarTailParameters::default(),
            cleaner_finished: false,
            step_finished: false,
        }
    }

    /// Enable automatic transfer from a concrete raster builder into the
    /// sheet immediately after `buildInfo` and before cleanup.
    #[must_use]
    pub fn with_raster_grid_handoff(mut self) -> Self
    where
        Builder: RasterGridHandoffSource,
    {
        self.raster_handoff = Some(Builder::take_raster_grid_handoff);
        self
    }

    /// Install production-retrieved staff candidates into concrete sheet
    /// ownership before staff-line cleanup.
    #[must_use]
    pub fn with_prepared_staff_handoff(mut self) -> Self
    where
        Builder: PreparedStaffHandoffSource,
    {
        self.staff_handoff = Some(Builder::take_prepared_staff_handoff);
        self
    }

    /// Install raw slope and retained slope rejects into sheet ownership after
    /// the build, provided raw line construction reached that handoff.
    #[must_use]
    pub fn with_raw_line_metadata_handoff(mut self) -> Self
    where
        Builder: RawLineMetadataHandoffSource,
    {
        self.raw_line_metadata_handoff = Some(Builder::take_raw_line_metadata_handoff);
        self
    }

    /// Install production bar-system results before the existing SIG
    /// promotion pass runs.
    #[must_use]
    pub fn with_prepared_bars_handoff(mut self) -> Self
    where
        Builder: PreparedBarsHandoffSource,
    {
        self.bars_handoff = Some(Builder::take_prepared_bars_handoff);
        self
    }

    #[must_use]
    pub fn with_bar_tail_parameters(mut self, parameters: BarTailParameters) -> Self {
        self.bar_tail_parameters = parameters;
        self
    }

    pub fn run(&mut self) -> Result<(), HeadlessGridError<Builder::StepError>> {
        run_grid_step(self)
    }

    fn update_book_scores(&mut self) -> Result<(), HeadlessGridError<Builder::StepError>> {
        let current = self
            .book
            .stubs
            .iter_mut()
            .find(|stub| stub.number == self.sheet.sheet_number)
            .ok_or(HeadlessGridError::ScoreUpdate(
                ScoreUpdateError::MissingCurrentStub(self.sheet.sheet_number),
            ))?;
        current.pages = self
            .sheet
            .population
            .page_refs
            .iter()
            .map(|page| PageInput {
                key: PageKey {
                    sheet_number: self.sheet.sheet_number,
                    page_id: page.id as u32,
                },
                movement_start: page.movement_start,
            })
            .collect();
        update_scores(
            &self.book.stubs,
            self.sheet.sheet_number,
            &mut self.book.scores,
        )
        .map_err(HeadlessGridError::ScoreUpdate)
    }
}

impl<Downstream>
    HeadlessGridExecutor<HeadlessRasterGridBuilder<RawProductionRetrieveLines<Downstream>>>
where
    Downstream: RemainingRasterGridStages,
{
    /// Construct the sheet-aware executor directly from a raster and the
    /// production raw/line parameters.
    ///
    /// `RetrieveLines` is owned by the raw production adapter. Bars and line
    /// completion remain explicit downstream collaborators until their raw
    /// production adapters are joined. Both raster-prefix and prepared-staff
    /// handoffs are enabled so successful and swallowed build prefixes retain
    /// the same installation semantics as the generic executor.
    #[must_use]
    pub fn from_raw_raster_lines(
        source: RunTable,
        raster_parameters: RasterGridParameters,
        raw_parameters: RawPrimaryPassParameters,
        lines_parameters: LinesCoordinatorParameters,
        downstream: Downstream,
        sheet: HeadlessGridSheet,
        book: HeadlessGridBook,
    ) -> Self {
        Self::new(
            HeadlessRasterGridBuilder::new(
                source,
                raster_parameters,
                RawProductionRetrieveLines::new(raw_parameters, lines_parameters, downstream),
            ),
            sheet,
            book,
        )
        .with_raster_grid_handoff()
        .with_raw_line_metadata_handoff()
        .with_prepared_staff_handoff()
    }
}

impl<Downstream>
    HeadlessGridExecutor<
        HeadlessRasterGridBuilder<
            ProductionCompleteLines<
                RawProductionRetrieveLines<Downstream>,
                ProductionLineCompletion<Downstream::StepError>,
            >,
        >,
    >
where
    Downstream: RemainingRasterGridStages,
{
    /// Construct the raw raster path with every concrete Java
    /// `LinesRetriever.completeLines` collaborator installed.
    ///
    /// System/staff ownership is explicit because flat retrieved-staff order
    /// cannot safely reconstruct side-by-side Java systems. The source clone
    /// is retained solely for the exact binary-provenance check performed
    /// before the completion `try/finally` boundary.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn from_raw_raster_complete_lines(
        source: RunTable,
        raster_parameters: RasterGridParameters,
        raw_parameters: RawPrimaryPassParameters,
        lines_parameters: LinesCoordinatorParameters,
        downstream: Downstream,
        completion_systems: Vec<PreparedCompletionSystem>,
        completion_parameters: ProductionLineCompletionParameters,
        maximum_thin_weight: usize,
        inspect_crossing_chunks: bool,
        sheet: HeadlessGridSheet,
        book: HeadlessGridBook,
    ) -> Self {
        let prepared_binary = source.clone();
        let raw = RawProductionRetrieveLines::new(raw_parameters, lines_parameters, downstream);
        let completion = production_line_completion(completion_parameters);
        let stages = ProductionCompleteLines::new(
            raw,
            completion,
            Some(prepared_binary),
            maximum_thin_weight,
            inspect_crossing_chunks,
        )
        .with_completion_systems(completion_systems);
        Self::new(
            HeadlessRasterGridBuilder::new(source, raster_parameters, stages),
            sheet,
            book,
        )
        .with_raster_grid_handoff()
        .with_raw_line_metadata_handoff()
        .with_prepared_staff_handoff()
    }
}

impl<Builder> GridStepExecutor for HeadlessGridExecutor<Builder>
where
    Builder: GridBuildExecutor,
{
    type Error = HeadlessGridError<Builder::StepError>;

    fn run_grid_step_stage(&mut self, stage: GridStepStage) -> Result<(), Self::Error> {
        match stage {
            GridStepStage::BuildGrid => {
                let result = {
                    let mut builder = SigPromotingBuilder {
                        builder: &mut self.builder,
                        sig: &mut self.sheet.sig,
                        staffs: &mut self.sheet.staffs,
                        promotion_failure: &mut self.sheet.promotion_failure,
                        bars_handoff: self.bars_handoff,
                        raw_line_metadata_handoff: self.raw_line_metadata_handoff,
                        skew: &mut self.sheet.skew,
                        sloped_line_fallbacks: &mut self.sheet.sloped_line_fallbacks,
                        population_geometries: &mut self.sheet.population.geometries,
                        sheet_width: self.sheet.population.sheet_width,
                        sheet_height: self.sheet.population.sheet_height,
                        bar_tail_parameters: self.bar_tail_parameters,
                    };
                    build_grid_info(&mut builder)
                };
                if let Some(extract) = self.raster_handoff
                    && let Some(handoff) = extract(&mut self.builder)
                {
                    self.sheet.install_raster_grid_handoff(handoff);
                }
                if let Some(extract) = self.raw_line_metadata_handoff
                    && let Some(handoff) = extract(&mut self.builder)
                {
                    self.sheet.install_raw_line_metadata_handoff(handoff);
                }
                if let Some(extract) = self.staff_handoff
                    && let Some(handoff) = extract(&mut self.builder)
                {
                    self.sheet.install_prepared_staff_handoff(handoff);
                    // Prepared staffs arrive after buildInfo, so restore the
                    // recordBars ownership already produced during ProcessBars.
                    install_staff_barlines(&self.sheet.sig, &mut self.sheet.staffs);
                }
                match result {
                    Ok(outcome) => {
                        self.build_outcome = Some(outcome);
                        Ok(())
                    }
                    Err(error) => Err(HeadlessGridError::Build(error)),
                }
            }
            GridStepStage::CleanStaffLines => clean_staff_lines(self),
            GridStepStage::UpdateBookScores => self.update_book_scores(),
        }
    }

    fn finish_successfully(&mut self) {
        self.step_finished = true;
    }
}

struct SigPromotingBuilder<'a, Builder> {
    builder: &'a mut Builder,
    sig: &'a mut HeadlessGridSigState,
    staffs: &'a mut [HeadlessStaff],
    promotion_failure: &'a mut Option<HeadlessGridPromotionError>,
    bars_handoff: Option<fn(&mut Builder) -> Option<PreparedBarsHandoff>>,
    raw_line_metadata_handoff: Option<fn(&mut Builder) -> Option<RawLineMetadataHandoff>>,
    skew: &'a mut Option<HeadlessSkew>,
    sloped_line_fallbacks: &'a mut Vec<PreparedStaffLine>,
    population_geometries: &'a mut [PopulationSystemGeometry],
    sheet_width: i32,
    sheet_height: i32,
    bar_tail_parameters: BarTailParameters,
}

impl<Builder> GridBuildExecutor for SigPromotingBuilder<'_, Builder>
where
    Builder: GridBuildExecutor,
{
    type StepError = Builder::StepError;
    type OtherError = HeadlessBuildOtherError<Builder::OtherError>;

    fn run_stage(
        &mut self,
        stage: audiveris_image::grid_lifecycle::GridBuildStage,
    ) -> Result<
        (),
        audiveris_image::grid_lifecycle::GridStageFailure<Self::StepError, Self::OtherError>,
    > {
        let result = self
            .builder
            .run_stage(stage)
            .map_err(|failure| match failure {
                audiveris_image::grid_lifecycle::GridStageFailure::Step(error) => {
                    audiveris_image::grid_lifecycle::GridStageFailure::Step(error)
                }
                audiveris_image::grid_lifecycle::GridStageFailure::Other(error) => {
                    audiveris_image::grid_lifecycle::GridStageFailure::Other(
                        HeadlessBuildOtherError::Builder(error),
                    )
                }
            });
        if stage == audiveris_image::grid_lifecycle::GridBuildStage::ProcessBars {
            if let Some(extract) = self.raw_line_metadata_handoff
                && let Some(handoff) = extract(self.builder)
            {
                install_raw_line_metadata(
                    handoff,
                    self.skew,
                    self.sloped_line_fallbacks,
                    self.population_geometries,
                    self.sheet_width,
                    self.sheet_height,
                );
            }
            if let Some(extract) = self.bars_handoff
                && let Some(handoff) = extract(self.builder)
            {
                self.sig
                    .install_prepared_bars_handoff_with_skew(handoff, self.skew.as_ref());
            }
            result?;
            promote_grid_sigs(self.sig, self.bar_tail_parameters).map_err(|error| {
                audiveris_image::grid_lifecycle::GridStageFailure::Other(
                    HeadlessBuildOtherError::Promotion(error),
                )
            })?;
            install_staff_barlines(self.sig, self.staffs);
        } else {
            result?;
        }
        Ok(())
    }

    fn log_swallowed_error(
        &mut self,
        stage: audiveris_image::grid_lifecycle::GridBuildStage,
        error: &Self::OtherError,
    ) {
        match error {
            HeadlessBuildOtherError::Builder(error) => {
                self.builder.log_swallowed_error(stage, error);
            }
            HeadlessBuildOtherError::Promotion(error) => {
                *self.promotion_failure = Some(*error);
            }
        }
    }

    fn finish(&mut self) {
        self.builder.finish();
    }
}

fn promote_grid_sigs(
    state: &mut HeadlessGridSigState,
    bar_tail_parameters: BarTailParameters,
) -> Result<(), HeadlessGridPromotionError> {
    // Java createInters traverses every system before connection promotion.
    for system in &mut state.systems {
        system.sig.promote_vertical_inters(&system.vertical_plans);
    }

    // createConnectionInters catches each individual connection exception.
    state.connection_warnings.clear();
    for connection in &state.connections {
        let system = state
            .systems
            .iter_mut()
            .find(|system| system.system_id == connection.system_id)
            .ok_or(HeadlessGridPromotionError::MissingSystem(
                connection.system_id,
            ))?;
        match system
            .sig
            .promote_connection_inters(&state.peak_graph, &[connection.plan])
        {
            Ok(warnings) => {
                state
                    .connection_warnings
                    .extend(warnings.into_iter().map(|warning| {
                        HeadlessConnectionWarning::Endpoint {
                            system_id: connection.system_id,
                            warning,
                        }
                    }))
            }
            Err(error) => state
                .connection_warnings
                .push(HeadlessConnectionWarning::Logic {
                    system_id: connection.system_id,
                    edge: connection.plan.edge,
                    error,
                }),
        }
    }

    // groupBarlines has no local catch: prior relations remain and the failure
    // reaches GridBuilder.buildInfo's ordinary-Throwable catch.
    for system in &mut state.systems {
        let interline = system.interline;
        system
            .sig
            .group_barlines(&system.staff_peaks, system.maximum_group_gap, |gap| {
                f64::from(gap) / interline
            })
            .map_err(|source| HeadlessGridPromotionError::BarGroup {
                system_id: system.system_id,
                source,
            })?;
    }

    // Java continues in exact order with recordBars, createGroups, then
    // createParts for every system.
    for system in &mut state.systems {
        system
            .sig
            .build_bar_tail(
                &mut system.bar_tail,
                &system.staff_peaks,
                &system.brace_peaks,
                &state.peak_graph,
                bar_tail_parameters,
            )
            .map_err(|source| HeadlessGridPromotionError::BarTail {
                system_id: system.system_id,
                source,
            })?;
    }

    // BarsRetriever.contextualize is a separate final system traversal after
    // all systems have completed their ownership/group/part tail.
    for system in &mut state.systems {
        system.sig.contextualize();
        system.bar_tail.contextualized = true;
    }
    Ok(())
}

fn install_staff_barlines(state: &HeadlessGridSigState, staffs: &mut [HeadlessStaff]) {
    for staff in staffs {
        staff.barlines.clear();
        for system in &state.systems {
            if let Some(barlines) = system.bar_tail.staff_barlines.get(&staff.id) {
                staff.barlines.extend_from_slice(barlines);
            }
        }
    }
}

impl<Builder> StaffLineCleanerExecutor for HeadlessGridExecutor<Builder>
where
    Builder: GridBuildExecutor,
{
    type Error = HeadlessGridError<Builder::StepError>;

    fn staff_ids(&self) -> Vec<usize> {
        self.sheet.staffs.iter().map(|staff| staff.id).collect()
    }

    fn simplify_staff_lines(
        &mut self,
        staff_id: usize,
    ) -> Result<Vec<OriginalStaffLine>, Self::Error> {
        let sheet = &mut self.sheet;
        let staff = sheet
            .staffs
            .iter_mut()
            .find(|staff| staff.id == staff_id)
            .ok_or(HeadlessGridError::MissingStaff(staff_id))?;
        let originals = std::mem::take(&mut staff.lines);
        let mut detailed = Vec::with_capacity(originals.len());
        for line in originals {
            match line {
                HeadlessStaffLine::Filament { line_id, filament } => {
                    let original = OriginalStaffLine {
                        line_id,
                        section_ids: filament
                            .sections()
                            .iter()
                            .map(audiveris_image::section::Section::id)
                            .collect(),
                    };
                    let persistent = to_registered_staff_line(&filament, &mut sheet.glyphs)
                        .map_err(HeadlessGridError::StaffLineConversion)?;
                    staff.lines.push(HeadlessStaffLine::Persistent {
                        source_line_id: line_id,
                        line: persistent,
                    });
                    detailed.push(original);
                }
                HeadlessStaffLine::Persistent { source_line_id, .. } => {
                    return Err(HeadlessGridError::PersistentStaffLine {
                        staff_id,
                        line_id: source_line_id,
                    });
                }
            }
        }
        Ok(detailed)
    }

    fn remove_horizontal_sections(&mut self, section_ids: &[usize]) -> Result<(), Self::Error> {
        let Some(RegisteredHorizontalLag::Populated(lag)) = &mut self.sheet.horizontal_lag else {
            return Err(HeadlessGridError::MissingHorizontalLag);
        };
        lag.remove_sections(section_ids)
            .map_err(HeadlessGridError::RemoveStaffSections)
    }

    fn rebuild_horizontal_lag(&mut self) -> Result<(), Self::Error> {
        rebuild_horizontal_lag(
            self.sheet.no_staff_table.as_ref(),
            self.sheet.max_fore,
            self.sheet.ledger_thickness,
            &mut self.sheet.horizontal_lag,
            &mut NoopVipSectionHook,
        )
        .map(|_| ())
        .map_err(HeadlessGridError::RebuildHorizontalLag)
    }

    fn populate_systems(&mut self) -> Result<(), Self::Error> {
        if let Some(skew) = &self.sheet.skew {
            apply_skew_to_population_geometries(skew, &mut self.sheet.population.geometries);
        }
        let horizontal_sections = match &self.sheet.horizontal_lag {
            Some(RegisteredHorizontalLag::Populated(lag)) => lag
                .sections()
                .iter()
                .map(|section| {
                    let centroid = section
                        .pixel_centroid_in(section.bounds())
                        .expect("a section contains at least one pixel");
                    PopulationSection {
                        id: section.id(),
                        centroid_x: centroid.0,
                        centroid_y: centroid.1,
                    }
                })
                .collect(),
            Some(RegisteredHorizontalLag::Empty) | None => Vec::new(),
        };
        let mut executor = TypedPopulationExecutor {
            state: &mut self.sheet.population,
            horizontal_sections,
        };
        match populate_systems(&mut executor) {
            Ok(()) => Ok(()),
            Err(never) => match never {},
        }
    }

    fn finish_successfully(&mut self) {
        self.cleaner_finished = true;
    }
}

struct TypedPopulationExecutor<'a> {
    state: &'a mut HeadlessPopulationState,
    horizontal_sections: Vec<PopulationSection>,
}

impl SystemPopulationExecutor for TypedPopulationExecutor<'_> {
    type Error = Infallible;

    fn system_ids(&self) -> Vec<usize> {
        self.state
            .geometries
            .iter()
            .map(|system| system.system_id)
            .collect()
    }

    fn staff_ids(&self, system_id: usize) -> Vec<usize> {
        self.state
            .systems
            .iter()
            .find(|system| system.id == system_id)
            .into_iter()
            .flat_map(|system| &system.parts)
            .flat_map(|part| &part.staves)
            .map(|staff| staff.staff_id)
            .collect()
    }

    fn update_system_coordinates(&mut self, _system_id: usize) -> Result<(), Self::Error> {
        Ok(())
    }

    fn compute_system_area(&mut self, system_id: usize) -> Result<(), Self::Error> {
        let all = build_population_system_areas(
            &self.state.geometries,
            &self.state.staff_boundaries,
            self.state.sheet_width,
            self.state.sheet_height,
            self.state.vertical_margin,
        );
        let area = all
            .into_iter()
            .find(|area| area.system_id == system_id)
            .expect("system geometry and area IDs match");
        if let Some(existing) = self
            .state
            .areas
            .iter_mut()
            .find(|existing| existing.system_id == system_id)
        {
            *existing = area;
        } else {
            self.state.areas.push(area);
        }
        Ok(())
    }

    fn compute_staff_area(&mut self, staff_id: usize) -> Result<(), Self::Error> {
        if !self.state.staff_areas_computed.contains(&staff_id) {
            self.state.staff_areas_computed.push(staff_id);
        }
        Ok(())
    }

    fn dispatch_horizontal_sections(&mut self) -> Result<(), Self::Error> {
        dispatch_to_areas(
            PopulationLag::Horizontal,
            &mut self.state.section_ownership,
            &self.horizontal_sections,
            &self.state.areas,
        );
        Ok(())
    }

    fn dispatch_vertical_sections(&mut self) -> Result<(), Self::Error> {
        dispatch_to_areas(
            PopulationLag::Vertical,
            &mut self.state.section_ownership,
            &self.state.vertical_sections,
            &self.state.areas,
        );
        Ok(())
    }

    fn check_indentations(&mut self) -> Result<(), Self::Error> {
        for index in 0..self.state.geometries.len() {
            let id = self.state.geometries[index].system_id;
            let indented = check_population_indentation(
                &self.state.geometries,
                index,
                self.state.minimum_indentation,
            );
            self.state
                .systems
                .iter_mut()
                .find(|system| system.id == id)
                .expect("population systems match geometry IDs")
                .indented = indented;
        }
        Ok(())
    }

    fn allocate_pages(&mut self) -> Result<(), Self::Error> {
        allocate_population_pages(
            &mut self.state.systems,
            &mut self.state.pages,
            &mut self.state.page_refs,
            &mut self.state.references,
        );
        Ok(())
    }

    fn report_results(&mut self) -> Result<(), Self::Error> {
        let counts = self
            .state
            .systems
            .iter()
            .map(|system| PopulationSystemCounts {
                system_id: system.id,
                part_count: system.parts.len(),
                tablature_count: 0,
            })
            .collect::<Vec<_>>();
        self.state.reports = self
            .state
            .pages
            .iter()
            .map(|page| population_page_report(page, &counts))
            .collect();
        Ok(())
    }
}

fn dispatch_to_areas(
    lag: PopulationLag,
    ownership: &mut [SystemSectionOwnership],
    sections: &[PopulationSection],
    areas: &[PopulationSystemArea],
) {
    dispatch_sections(lag, ownership, sections, |system_id, x, y| {
        areas
            .iter()
            .find(|area| area.system_id == system_id)
            .is_some_and(|area| area.contains(x, y))
    });
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;
    use audiveris_image::{
        bar_alignment::{AlignmentPeak, BarImpacts},
        bar_column::{PeakId, StaffId},
        bars_coordinator::{BarsCoordinatorParameters, BarsStaffState, BarsSystemState},
        bars_logic::{PeakWidthClass, VerticalInterKind, VerticalMedian, plan_connection_inters},
        cluster_expand::ClusterExpansionParameters,
        cluster_merge::{ClusterMergeParameters, ClusterMergePassParameters},
        cluster_ownership::ClusterOwnership,
        cluster_pipeline::ClusterRetrievalParameters,
        crossing_completion::InspectCrossingChunksParameters,
        curvature_completion::{PolishCurvaturesCompletionError, PolishCurvaturesError},
        discarded_completion::{
            IncludeDiscardedCompletionError, IncludeDiscardedFilamentsParameters,
        },
        filament::{FilamentError, StaffFilament},
        filament_factory::{FilamentFactoryParams, OverlapParams},
        grid_lifecycle::{GridBuildStage, GridStageFailure},
        grid_sig::{GridSigNode, GridSigRelation},
        line_cluster::FilamentId,
        line_completion::LineCompletionStage,
        line_endpoints::{DefineEndPointsCompletionError, DefineEndPointsParameters},
        line_holes::{FillHolesCompletionError, HoleFillInvocation},
        line_short_sections::HorizontalSectionLag,
        lines_coordinator::{ClusterPassState, LinesCoordinatorParameters},
        prepared_bars::{PreparedBarsSystem, ProductionProcessBars},
        prepared_completion::ProductionCompleteLinesError,
        prepared_lines::ProductionRetrieveLines,
        raster_grid_builder::{
            HeadlessRasterGridBuilder, RasterGridBuildState, RasterGridOtherError,
            RasterGridParameters, RemainingRasterGridStages,
        },
        run_table::{Orientation, Run, create_grid_run_tables},
        section::{JunctionPolicy, build_sections},
        section_completion::{IncludeSectionsCompletionError, IncludeSectionsParameters},
        staff_peak::StaffPeak,
        sticker_completion::IncludeStickersParameters,
        system_population::{
            BoundarySegment, PopulationReferencePart, PopulationReferenceStaff,
            PopulationStaffConfig, PopulationSystemRefState, StaffBoundary,
        },
    };

    fn near(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= 1.0e-12,
            "{actual} != {expected}"
        );
    }

    #[test]
    fn headless_skew_matches_java_positive_negative_and_zero_coordinates() {
        let point = PeakPoint::new(10.0, 20.0);

        let positive = HeadlessSkew::new(0.5, 100, 50);
        let transformed = positive.deskewed(point);
        near(transformed.x, 37.888_543_819_998_32);
        near(transformed.y, 53.416_407_864_998_74);
        near(positive.deskewed_width(), 111.803_398_874_989_48);
        near(positive.deskewed_height(), 89.442_719_099_991_59);
        let roundtrip = positive.skewed(transformed);
        near(roundtrip.x, point.x);
        near(roundtrip.y, point.y);

        let negative = HeadlessSkew::new(-0.5, 100, 50);
        let transformed = negative.deskewed(point);
        near(transformed.x, 20.0);
        near(transformed.y, 32.360_679_774_997_9);
        near(negative.deskewed_width(), 111.803_398_874_989_48);
        near(negative.deskewed_height(), 89.442_719_099_991_59);
        let roundtrip = negative.skewed(transformed);
        near(roundtrip.x, point.x);
        near(roundtrip.y, point.y);

        let zero = HeadlessSkew::new(0.0, 100, 50);
        assert_eq!(zero.deskewed(point), point);
        assert_eq!(zero.skewed(point), point);
        assert_eq!(
            (zero.deskewed_width(), zero.deskewed_height()),
            (100.0, 50.0)
        );
    }

    #[derive(Default)]
    struct SuccessfulBuilder {
        stages: Vec<GridBuildStage>,
        finish_count: usize,
        failure: Option<(GridBuildStage, bool)>,
        warnings: Vec<GridBuildStage>,
    }

    #[derive(Default)]
    struct RasterStages {
        retrieve_failure: Option<bool>,
        finish_count: usize,
    }

    impl RemainingRasterGridStages for RasterStages {
        type StepError = &'static str;
        type OtherError = &'static str;

        fn retrieve_lines(
            &mut self,
            _state: &mut RasterGridBuildState,
        ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
            match self.retrieve_failure {
                Some(true) => Err(GridStageFailure::Step("retrieve step failure")),
                Some(false) => Err(GridStageFailure::Other("retrieve ordinary failure")),
                None => Ok(()),
            }
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
            Ok(())
        }

        fn log_swallowed_error(&mut self, _stage: GridBuildStage, _error: &Self::OtherError) {}

        fn finish(&mut self) {
            self.finish_count += 1;
        }
    }

    impl GridBuildExecutor for SuccessfulBuilder {
        type StepError = &'static str;
        type OtherError = &'static str;

        fn run_stage(
            &mut self,
            stage: GridBuildStage,
        ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
            self.stages.push(stage);
            if self.failure == Some((stage, true)) {
                return Err(GridStageFailure::Step("grid step failure"));
            }
            if self.failure == Some((stage, false)) {
                return Err(GridStageFailure::Other("ordinary grid failure"));
            }
            Ok(())
        }

        fn log_swallowed_error(&mut self, stage: GridBuildStage, _error: &Self::OtherError) {
            self.warnings.push(stage);
        }

        fn finish(&mut self) {
            self.finish_count += 1;
        }
    }

    fn horizontal_table() -> RunTable {
        let mut table = RunTable::new(Orientation::Horizontal, 100, 20).unwrap();
        table.add_run(5, Run::new(10, 81)).unwrap();
        table.add_run(12, Run::new(10, 4)).unwrap();
        table
    }

    fn vertical_no_staff() -> RunTable {
        let mut table = RunTable::new(Orientation::Vertical, 100, 20).unwrap();
        table.add_run(3, Run::new(4, 2)).unwrap();
        table.add_run(12, Run::new(11, 2)).unwrap();
        table
    }

    fn raster_source() -> RunTable {
        let mut table = RunTable::new(Orientation::Vertical, 100, 20).unwrap();
        table.add_run(3, Run::new(1, 8)).unwrap();
        table.add_run(12, Run::new(11, 2)).unwrap();
        table
    }

    fn raster_parameters() -> RasterGridParameters {
        RasterGridParameters {
            max_fore: 3,
            ledger_thickness: 1.0,
            minimum_horizontal_run_length: 2,
            maximum_vertical_run_shift: 1,
        }
    }

    fn production_source() -> RunTable {
        let mut table = RunTable::new(Orientation::Vertical, 42, 20).unwrap();
        for x in 0..=40 {
            table.add_run(x, Run::new(10, 1)).unwrap();
        }
        table
    }

    fn production_raster_parameters() -> RasterGridParameters {
        RasterGridParameters {
            max_fore: 3,
            ledger_thickness: 1.0,
            minimum_horizontal_run_length: 4,
            maximum_vertical_run_shift: 1,
        }
    }

    fn split_middle_source() -> RunTable {
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

    fn split_middle_with_discarded_source() -> RunTable {
        let mut source = RunTable::new(Orientation::Vertical, 100, 61).unwrap();
        for x in 0..100 {
            if (10..=20).contains(&x) {
                source.add_run(x, Run::new(5, 1)).unwrap();
            }
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

    fn positive_slope_source() -> RunTable {
        let mut source = RunTable::new(Orientation::Vertical, 100, 90).unwrap();
        for x in 0..100 {
            for base in [10, 20, 30, 40, 50] {
                source.add_run(x, Run::new(base + (x / 20), 1)).unwrap();
            }
            if x < 60 {
                source.add_run(x, Run::new(65 + (x / 10), 1)).unwrap();
            }
        }
        source
    }

    fn raw_primary_parameters() -> RawPrimaryPassParameters {
        let expansion = ClusterExpansionParameters::new(0.0, 0, 1, 0, 0.1, 1, 10).unwrap();
        let compatibility = ClusterMergeParameters::new(0.0, 0, 0.1, 0, 1, 10).unwrap();
        let retrieval = ClusterRetrievalParameters::new(
            10,
            BTreeSet::from([5]),
            expansion,
            ClusterMergePassParameters::new(compatibility, 0, 1).unwrap(),
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

    fn raw_raster_parameters() -> RasterGridParameters {
        RasterGridParameters {
            max_fore: 2,
            ledger_thickness: 1.0,
            minimum_horizontal_run_length: 10,
            maximum_vertical_run_shift: 1,
        }
    }

    fn raw_lines_parameters() -> LinesCoordinatorParameters {
        LinesCoordinatorParameters::new(0.0, 1, None, 1.0, 0.0, 100.0).unwrap()
    }

    fn placeholder_slope_lines_parameters() -> LinesCoordinatorParameters {
        LinesCoordinatorParameters::new(-0.75, 1, None, 1.0, 0.0, 100.0).unwrap()
    }

    fn raw_completion_parameters(
        minimum_curvature_radius: i32,
    ) -> ProductionLineCompletionParameters {
        ProductionLineCompletionParameters {
            define_end_points: DefineEndPointsParameters {
                scale_interline: 10,
                foreground_thickness: 1,
                maximum_ending_dx: 10,
                pattern_width: 10,
                pattern_jitter: 3,
            },
            include_discarded_filaments: IncludeDiscardedFilamentsParameters {
                scale_interline: 10,
                foreground_thickness: 1,
                maximum_sticker_thickness: 2.0,
                maximum_sticker_gap: 1.0,
                maximum_sticker_extension: 2,
            },
            include_sections: IncludeSectionsParameters {
                scale_interline: 10,
                foreground_thickness: 1,
                maximum_sticker_thickness: 2.0,
                maximum_sticker_gap: 1.0,
                maximum_sticker_extension: 2,
            },
            minimum_curvature_radius,
            include_stickers: IncludeStickersParameters {
                maximum_connection_length: 1,
            },
            // Deliberately not the measured raw slope. The integration test
            // proves completion consumes RawLineMetadataHandoff instead.
            inspect_crossing_chunks: InspectCrossingChunksParameters {
                minimum_offset: 1_000,
                global_slope: -0.75,
                minimum_chunk_slope: 0.04,
            },
        }
    }

    fn raw_completion_systems() -> Vec<PreparedCompletionSystem> {
        vec![PreparedCompletionSystem {
            system_id: 1,
            staff_ids: vec![1],
        }]
    }

    fn cluster_parameters() -> ClusterRetrievalParameters {
        let expansion = ClusterExpansionParameters::new(0.0, 0, 1, 0, 0.1, 1, 10).unwrap();
        let compatibility = ClusterMergeParameters::new(0.0, 0, 0.1, 0, 1, 10).unwrap();
        ClusterRetrievalParameters::new(
            10,
            BTreeSet::from([1]),
            expansion,
            ClusterMergePassParameters::new(compatibility, 0, 1).unwrap(),
            0.0,
            0.0,
            0.0,
            1,
            0,
            None,
            1,
        )
        .unwrap()
    }

    fn prepared_primary(section: Section) -> ClusterPassState {
        let id = FilamentId::new(7);
        let mut filament = StaffFilament::new(10).unwrap();
        filament.add_section(section).unwrap();
        let mut ownership = ClusterOwnership::new();
        ownership.register_filament(id, &filament).unwrap();
        ClusterPassState::new(
            ownership,
            BTreeMap::from([(id, filament)]),
            BTreeMap::new(),
            vec![id],
            cluster_parameters(),
        )
    }

    fn production_section() -> Section {
        let tables = create_grid_run_tables(&production_source(), 3, 1.0, 4).unwrap();
        HorizontalSectionLag::from_long_runs(tables.long_horizontal)
            .unwrap()
            .sections()[0]
            .clone()
    }

    fn production_executor(
        primary: ClusterPassState,
    ) -> HeadlessGridExecutor<HeadlessRasterGridBuilder<ProductionRetrieveLines<RasterStages>>>
    {
        let base = executor();
        let parameters = LinesCoordinatorParameters::new(0.0, 1, None, 1.0, 0.0, 100.0).unwrap();
        let stages =
            ProductionRetrieveLines::new(primary, None, parameters, RasterStages::default());
        HeadlessGridExecutor::new(
            HeadlessRasterGridBuilder::new(
                production_source(),
                production_raster_parameters(),
                stages,
            ),
            base.sheet,
            base.book,
        )
        .with_raster_grid_handoff()
        .with_prepared_staff_handoff()
    }

    fn production_bars_executor() -> HeadlessGridExecutor<
        HeadlessRasterGridBuilder<ProductionProcessBars<ProductionRetrieveLines<RasterStages>>>,
    > {
        let base = executor();
        let line_parameters =
            LinesCoordinatorParameters::new(0.0, 1, None, 1.0, 0.0, 100.0).unwrap();
        let lines = ProductionRetrieveLines::new(
            prepared_primary(production_section()),
            None,
            line_parameters,
            RasterStages::default(),
        );
        let mut bar = peak(1, 10);
        bar.compute_deskewed_center(|point| point).unwrap();
        let mut graph = PeakGraph::new();
        graph.add_vertex(bar.clone());
        let system = BarsSystemState::new(
            1,
            vec![
                BarsStaffState::new(StaffId::new(1), 0, true, vec![bar], BTreeMap::new()).unwrap(),
            ],
            graph,
        )
        .unwrap();
        let bars = ProductionProcessBars::new(
            lines,
            vec![system],
            BarsCoordinatorParameters::new(2, 12, 6, 20, 2, 10, 0.2, None).unwrap(),
            3,
        )
        .unwrap();
        HeadlessGridExecutor::new(
            HeadlessRasterGridBuilder::new(
                production_source(),
                production_raster_parameters(),
                bars,
            ),
            base.sheet,
            base.book,
        )
        .with_raster_grid_handoff()
        .with_prepared_staff_handoff()
        .with_prepared_bars_handoff()
    }

    fn boundary(y: f64) -> StaffBoundary {
        StaffBoundary {
            segments: vec![BoundarySegment::Line {
                start: (0.0, y),
                end: (100.0, y),
            }],
        }
    }

    fn peak(staff: usize, x: i32) -> StaffPeak {
        StaffPeak::new(StaffId::new(staff), 4, 15, x, x).unwrap()
    }

    fn vertical_plan(peak: &StaffPeak) -> VerticalInterPlan {
        VerticalInterPlan {
            peak: peak.key(),
            median: VerticalMedian {
                x: f64::from(peak.start()) + 0.5,
                top: 3.5,
                bottom: 16.5,
            },
            width: 1.0,
            impacts: None,
            kind: VerticalInterKind::Barline {
                width_class: PeakWidthClass::Thin,
                left_staff_end: false,
                right_staff_end: false,
            },
        }
    }

    fn connection_graph(top: StaffPeak, bottom: StaffPeak) -> PeakGraph<BarAlignment> {
        let mut graph = PeakGraph::new();
        add_connection(&mut graph, top, bottom, 1);
        graph
    }

    fn add_connection(
        graph: &mut PeakGraph<BarAlignment>,
        top: StaffPeak,
        bottom: StaffPeak,
        id: usize,
    ) {
        let top_key = top.key();
        let bottom_key = bottom.key();
        graph.add_vertex(top);
        graph.add_vertex(bottom);
        let alignment = BarAlignment::new(
            AlignmentPeak::new(PeakId::new(id), top_key.staff_id(), 10, 1.0).unwrap(),
            AlignmentPeak::new(PeakId::new(id + 100), bottom_key.staff_id(), 10, 1.0).unwrap(),
            0.0,
            0.0,
            BarImpacts::alignment(1.0, 1.0).unwrap(),
        )
        .unwrap();
        graph
            .add_edge(
                top_key,
                bottom_key,
                BarAlignment::connection(&alignment, 1.0, 1.0).unwrap(),
            )
            .unwrap();
    }

    fn sig_state(
        peaks: Vec<Vec<StaffPeak>>,
        vertical_plans: Vec<VerticalInterPlan>,
    ) -> HeadlessSystemSigState {
        let brace_peaks = vec![None; peaks.len()];
        HeadlessSystemSigState {
            system_id: 1,
            sig: GridSig::default(),
            vertical_plans,
            staff_peaks: peaks,
            brace_peaks,
            maximum_group_gap: 3,
            interline: 10.0,
            bar_tail: BarTailResult::default(),
        }
    }

    fn grid_sig_state(
        system: HeadlessSystemSigState,
        peak_graph: PeakGraph<BarAlignment>,
        plans: Vec<ConnectionInterPlan>,
    ) -> HeadlessGridSigState {
        HeadlessGridSigState {
            systems: vec![system],
            peak_graph,
            connections: plans
                .into_iter()
                .map(|plan| HeadlessConnectionPlan { system_id: 1, plan })
                .collect(),
            connection_warnings: Vec::new(),
        }
    }

    fn executor() -> HeadlessGridExecutor<SuccessfulBuilder> {
        let lag = HorizontalSectionLag::from_long_runs(horizontal_table()).unwrap();
        let mut filament = StaffFilament::new(10).unwrap();
        filament.add_section(lag.sections()[0].clone()).unwrap();
        let staff_config = PopulationStaffConfig {
            line_count: 5,
            is_small: false,
        };
        let population = HeadlessPopulationState {
            sheet_width: 100,
            sheet_height: 20,
            vertical_margin: 1,
            minimum_indentation: 4.0,
            geometries: vec![PopulationSystemGeometry {
                system_id: 1,
                left: 0,
                width: 100,
                top: 2,
                bottom: 18,
                area_left: 0,
                deskewed_upper_left_x: 0.0,
            }],
            staff_boundaries: vec![SystemStaffBoundaries {
                first_line: boundary(3.0),
                last_line: boundary(17.0),
            }],
            vertical_sections: vec![PopulationSection {
                id: 90,
                centroid_x: 12.0,
                centroid_y: 12.0,
            }],
            section_ownership: vec![SystemSectionOwnership {
                system_id: 1,
                horizontal_sections: Vec::new(),
                vertical_sections: Vec::new(),
            }],
            systems: vec![PopulationSystem {
                id: 1,
                indented: false,
                parts: vec![PopulationReferencePart {
                    part_id: 1,
                    staves: vec![PopulationReferenceStaff {
                        staff_id: 1,
                        config: staff_config,
                    }],
                }],
                system_ref: PopulationSystemRefState::default(),
                page_id: None,
            }],
            areas: Vec::new(),
            staff_areas_computed: Vec::new(),
            pages: Vec::new(),
            page_refs: Vec::new(),
            references: PopulationReferenceRegistry::default(),
            reports: Vec::new(),
        };
        HeadlessGridExecutor::new(
            SuccessfulBuilder::default(),
            HeadlessGridSheet {
                sheet_number: 1,
                staffs: vec![HeadlessStaff {
                    id: 1,
                    kind: StaffCandidateKind::OneLine,
                    left: 0.0,
                    right: 7.0,
                    interline: 10,
                    small: false,
                    short: false,
                    barlines: Vec::new(),
                    lines: vec![HeadlessStaffLine::Filament {
                        line_id: 10,
                        filament,
                    }],
                }],
                glyphs: HeadlessGlyphRegistry::default(),
                sig: HeadlessGridSigState {
                    systems: Vec::new(),
                    peak_graph: PeakGraph::new(),
                    connections: Vec::new(),
                    connection_warnings: Vec::new(),
                },
                promotion_failure: None,
                no_staff_table: Some(vertical_no_staff()),
                max_fore: Some(3),
                ledger_thickness: 1.0,
                vertical_lag: None,
                horizontal_lag: Some(RegisteredHorizontalLag::Populated(lag)),
                installed_raster_prefix: None,
                skew: None,
                sloped_line_fallbacks: Vec::new(),
                population,
            },
            HeadlessGridBook {
                stubs: vec![StubPages {
                    number: 1,
                    valid_selected: true,
                    pages: Vec::new(),
                }],
                scores: Vec::new(),
            },
        )
    }

    fn raster_executor(
        stages: RasterStages,
    ) -> HeadlessGridExecutor<HeadlessRasterGridBuilder<RasterStages>> {
        let base = executor();
        HeadlessGridExecutor::new(
            HeadlessRasterGridBuilder::new(raster_source(), raster_parameters(), stages),
            base.sheet,
            base.book,
        )
        .with_raster_grid_handoff()
    }

    #[test]
    fn concrete_executor_mutates_sheet_references_and_scores_in_java_order() {
        let mut executor = executor();

        executor.run().unwrap();

        assert_eq!(executor.builder.stages, GridBuildStage::ORDER);
        assert_eq!(executor.builder.finish_count, 1);
        assert!(matches!(
            executor.sheet.staffs[0].lines.as_slice(),
            [HeadlessStaffLine::Persistent {
                source_line_id: 10,
                ..
            }]
        ));
        assert_eq!(executor.sheet.glyphs.originals.len(), 1);
        assert_eq!(executor.sheet.population.pages[0].system_ids, [1]);
        assert_eq!(executor.sheet.population.page_refs[0].systems.len(), 1);
        let system_ref = executor.sheet.population.page_refs[0].systems[0];
        assert_eq!(
            executor
                .sheet
                .population
                .references
                .get(system_ref)
                .expect("registered system ref")
                .page_ref_id,
            executor.sheet.population.page_refs[0].object_id
        );
        assert_eq!(
            executor.sheet.population.section_ownership[0].vertical_sections,
            [90]
        );
        assert_eq!(executor.sheet.population.staff_areas_computed, [1]);
        assert_eq!(executor.book.stubs[0].pages.len(), 1);
        assert_eq!(executor.book.scores.len(), 1);
        assert_eq!(
            executor.book.scores[0].pages,
            [PageKey {
                sheet_number: 1,
                page_id: 1,
            }]
        );
        assert!(executor.cleaner_finished);
        assert!(executor.step_finished);
    }

    #[test]
    fn cleaner_failure_keeps_simplification_and_skips_population_and_scores() {
        let mut executor = executor();
        executor.sheet.horizontal_lag = None;

        assert_eq!(executor.run(), Err(HeadlessGridError::MissingHorizontalLag));

        assert!(matches!(
            executor.sheet.staffs[0].lines.as_slice(),
            [HeadlessStaffLine::Persistent {
                source_line_id: 10,
                ..
            }]
        ));
        assert!(executor.sheet.population.pages.is_empty());
        assert!(executor.book.scores.is_empty());
        assert!(!executor.cleaner_finished);
        assert!(!executor.step_finished);
    }

    #[test]
    fn build_step_failure_finishes_builder_then_skips_all_downstream_mutation() {
        let mut executor = executor();
        executor.builder.failure = Some((GridBuildStage::AddShortSections, true));

        assert_eq!(
            executor.run(),
            Err(HeadlessGridError::Build("grid step failure"))
        );

        assert_eq!(
            executor.builder.stages,
            [
                GridBuildStage::CreateBothLags,
                GridBuildStage::RetrieveLines,
                GridBuildStage::AddShortSections,
            ]
        );
        assert_eq!(executor.builder.finish_count, 1);
        assert!(matches!(
            executor.sheet.staffs[0].lines[0],
            HeadlessStaffLine::Filament { .. }
        ));
        assert!(executor.sheet.population.pages.is_empty());
        assert!(executor.book.scores.is_empty());
    }

    #[test]
    fn ordinary_builder_failure_is_swallowed_before_concrete_downstream_stages() {
        let mut executor = executor();
        executor.builder.failure = Some((GridBuildStage::ProcessBars, false));

        executor.run().unwrap();

        assert_eq!(executor.builder.warnings, [GridBuildStage::ProcessBars]);
        assert!(matches!(
            executor.build_outcome,
            Some(GridBuildOutcome::Swallowed {
                stage: GridBuildStage::ProcessBars,
                error: HeadlessBuildOtherError::Builder("ordinary grid failure"),
            })
        ));
        assert_eq!(executor.sheet.population.pages.len(), 1);
        assert_eq!(executor.book.scores.len(), 1);
        assert!(executor.step_finished);
    }

    #[test]
    fn score_failure_retains_completed_cleaner_and_page_ownership() {
        let mut executor = executor();
        executor.book.stubs.clear();

        assert_eq!(
            executor.run(),
            Err(HeadlessGridError::ScoreUpdate(
                ScoreUpdateError::MissingCurrentStub(1)
            ))
        );

        assert!(executor.cleaner_finished);
        assert_eq!(executor.sheet.population.pages.len(), 1);
        assert_eq!(executor.sheet.population.page_refs[0].systems.len(), 1);
        assert!(executor.book.scores.is_empty());
        assert!(!executor.step_finished);
    }

    #[test]
    fn process_bars_promotes_backlinks_connections_and_freezing_before_cleanup() {
        let top = peak(1, 10);
        let bottom = peak(2, 10);
        let graph = connection_graph(top.clone(), bottom.clone());
        let plans = plan_connection_inters(&graph, |_| true);
        let mut executor = executor();
        executor.sheet.sig = grid_sig_state(
            sig_state(
                vec![vec![top.clone()], vec![bottom.clone()]],
                vec![vertical_plan(&top), vertical_plan(&bottom)],
            ),
            graph,
            plans,
        );

        executor.run().unwrap();

        let state = &executor.sheet.sig.systems[0];
        let top_inter = state.sig.inter_of(top.key()).expect("top backlink");
        let bottom_inter = state.sig.inter_of(bottom.key()).expect("bottom backlink");
        assert!(matches!(
            state.sig.node(top_inter),
            Some(GridSigNode::Vertical { frozen: true, .. })
        ));
        assert!(matches!(
            state.sig.node(bottom_inter),
            Some(GridSigNode::Vertical { frozen: true, .. })
        ));
        assert_eq!(state.sig.edges().len(), 3);
        assert_eq!(state.bar_tail.staff_barlines[&1], [top_inter]);
        assert_eq!(state.bar_tail.staff_barlines[&2], [bottom_inter]);
        assert_eq!(state.bar_tail.parts.len(), 2);
        assert!(state.bar_tail.contextualized);
        assert_eq!(
            state.sig.node(top_inter).unwrap().contextual_grade(),
            Some(0.0)
        );
        assert_eq!(executor.sheet.staffs[0].barlines, [top_inter]);
        assert!(executor.sheet.sig.connection_warnings.is_empty());
        assert!(executor.cleaner_finished);
    }

    #[test]
    fn connection_extension_failure_is_caught_per_edge_with_partial_sig_retained() {
        let top = peak(1, 10);
        let bottom = peak(2, 10);
        let source_graph = connection_graph(top.clone(), bottom.clone());
        let plans = plan_connection_inters(&source_graph, |_| true);
        let missing_edge = plans[0].edge;
        let mut state = grid_sig_state(
            sig_state(
                vec![vec![top.clone()], vec![bottom.clone()]],
                vec![vertical_plan(&top), vertical_plan(&bottom)],
            ),
            PeakGraph::new(),
            plans,
        );

        promote_grid_sigs(&mut state, BarTailParameters::default()).unwrap();

        assert!(matches!(
            state.connection_warnings.as_slice(),
            [HeadlessConnectionWarning::Logic { edge, .. }] if *edge == missing_edge
        ));
        assert_eq!(state.systems[0].sig.edges().len(), 3);
        assert!(matches!(
            state.systems[0]
                .sig
                .nodes_in_order()
                .last()
                .map(|(_, node)| node),
            Some(GridSigNode::Connector { frozen: true, .. })
        ));
    }

    #[test]
    fn group_failure_is_swallowed_at_process_bars_after_retaining_group_prefix() {
        let first = peak(1, 10);
        let second = peak(1, 12);
        let missing = peak(1, 14);
        let expected = HeadlessGridPromotionError::BarGroup {
            system_id: 1,
            source: BarGroupPromotionError::MissingInter(missing.key()),
        };
        let mut executor = executor();
        executor.sheet.sig = grid_sig_state(
            sig_state(
                vec![vec![first.clone(), second.clone(), missing]],
                vec![vertical_plan(&first), vertical_plan(&second)],
            ),
            PeakGraph::new(),
            Vec::new(),
        );

        executor.run().unwrap();

        assert_eq!(
            executor.builder.stages.last(),
            Some(&GridBuildStage::ProcessBars)
        );
        assert_eq!(executor.sheet.promotion_failure, Some(expected));
        assert!(matches!(
            executor.build_outcome,
            Some(GridBuildOutcome::Swallowed {
                stage: GridBuildStage::ProcessBars,
                error: HeadlessBuildOtherError::Promotion(error),
            }) if error == expected
        ));
        assert_eq!(
            executor.sheet.sig.systems[0]
                .sig
                .edges()
                .iter()
                .filter(|edge| matches!(edge.relation, GridSigRelation::BarGroup { .. }))
                .count(),
            1
        );
        assert_eq!(executor.sheet.population.pages.len(), 1);
        assert!(executor.step_finished);
    }

    #[test]
    fn second_filament_failure_retains_converted_prefix_and_detaches_remainder() {
        let mut executor = executor();
        let valid = match &executor.sheet.staffs[0].lines[0] {
            HeadlessStaffLine::Filament { filament, .. } => filament.clone(),
            HeadlessStaffLine::Persistent { .. } => unreachable!(),
        };
        executor.sheet.staffs[0].lines = vec![
            HeadlessStaffLine::Filament {
                line_id: 10,
                filament: valid.clone(),
            },
            HeadlessStaffLine::Filament {
                line_id: 20,
                filament: StaffFilament::new(10).unwrap(),
            },
            HeadlessStaffLine::Filament {
                line_id: 30,
                filament: valid,
            },
        ];

        assert_eq!(
            executor.run(),
            Err(HeadlessGridError::StaffLineConversion(
                StaffLineConversionError::Filament(FilamentError::Empty)
            ))
        );

        assert!(matches!(
            executor.sheet.staffs[0].lines.as_slice(),
            [HeadlessStaffLine::Persistent {
                source_line_id: 10,
                ..
            }]
        ));
        assert_eq!(executor.sheet.glyphs.originals.len(), 1);
        assert!(executor.sheet.population.pages.is_empty());
        assert!(!executor.cleaner_finished);
    }

    #[test]
    fn connections_follow_global_peak_edge_order_across_target_systems() {
        let one_top = peak(1, 10);
        let one_bottom = peak(2, 10);
        let two_top = peak(3, 20);
        let two_bottom = peak(4, 20);
        let mut source_graph = PeakGraph::new();
        add_connection(&mut source_graph, one_top.clone(), one_bottom.clone(), 1);
        add_connection(&mut source_graph, two_top.clone(), two_bottom.clone(), 2);
        let plans = plan_connection_inters(&source_graph, |_| true);
        let mut second_system = sig_state(
            vec![vec![two_top.clone()], vec![two_bottom.clone()]],
            vec![vertical_plan(&two_top), vertical_plan(&two_bottom)],
        );
        second_system.system_id = 2;
        let mut state = HeadlessGridSigState {
            systems: vec![
                sig_state(
                    vec![vec![one_top.clone()], vec![one_bottom.clone()]],
                    vec![vertical_plan(&one_top), vertical_plan(&one_bottom)],
                ),
                second_system,
            ],
            // Force one caught extension error per globally ordered plan.
            peak_graph: PeakGraph::new(),
            connections: vec![
                HeadlessConnectionPlan {
                    system_id: 2,
                    plan: plans[1],
                },
                HeadlessConnectionPlan {
                    system_id: 1,
                    plan: plans[0],
                },
            ],
            connection_warnings: Vec::new(),
        };

        promote_grid_sigs(&mut state, BarTailParameters::default()).unwrap();

        assert_eq!(
            state
                .connection_warnings
                .iter()
                .map(|warning| match warning {
                    HeadlessConnectionWarning::Endpoint { system_id, .. }
                    | HeadlessConnectionWarning::Logic { system_id, .. } => *system_id,
                })
                .collect::<Vec<_>>(),
            [2, 1]
        );
        assert_eq!(state.systems[0].sig.edges().len(), 3);
        assert_eq!(state.systems[1].sig.edges().len(), 3);
    }

    #[test]
    fn prepared_bar_peaks_are_reprojected_through_sheet_skew() {
        let bar = peak(1, 10);
        let key = bar.key();
        let mut graph = PeakGraph::new();
        graph.add_vertex(bar.clone());
        let handoff = PreparedBarsHandoff {
            systems: vec![PreparedBarsSystem {
                system_id: 1,
                staff_peaks: vec![vec![bar]],
                brace_peaks: vec![None],
                vertical_plans: Vec::new(),
                maximum_group_gap: 3,
                interline: 10.0,
            }],
            peak_graph: graph,
            connections: Vec::new(),
        };
        let skew = HeadlessSkew::new(0.5, 100, 50);
        let mut state = HeadlessGridSigState {
            systems: Vec::new(),
            peak_graph: PeakGraph::new(),
            connections: Vec::new(),
            connection_warnings: Vec::new(),
        };

        state.install_prepared_bars_handoff_with_skew(handoff, Some(&skew));

        let expected = skew.deskewed(PeakPoint::new(10.0, 9.5));
        assert_eq!(
            state.systems[0].staff_peaks[0][0].deskewed_center(),
            Some(expected)
        );
        assert_eq!(
            state.peak_graph.vertex(key).unwrap().deskewed_center(),
            Some(expected)
        );
    }

    #[test]
    fn raster_builder_installs_completed_lags_before_cleaner() {
        let mut executor = raster_executor(RasterStages::default());
        executor.sheet.horizontal_lag = None;
        executor.sheet.population.vertical_sections.clear();

        executor.run().unwrap();

        assert_eq!(
            executor.sheet.installed_raster_prefix,
            Some(InstalledRasterGridPrefix {
                short_sections_added: true,
                horizontal_lag_present: true,
            })
        );
        assert!(
            !executor
                .sheet
                .vertical_lag
                .as_ref()
                .expect("installed vertical lag")
                .sections
                .is_empty()
        );
        assert!(!executor.sheet.population.vertical_sections.is_empty());
        assert!(executor.cleaner_finished);
        assert!(executor.step_finished);
    }

    #[test]
    fn swallowed_retrieve_installs_initial_prefix_then_continues_cleanup() {
        let mut executor = raster_executor(RasterStages {
            retrieve_failure: Some(false),
            ..RasterStages::default()
        });
        executor.sheet.horizontal_lag = None;

        executor.run().unwrap();

        assert_eq!(
            executor.sheet.installed_raster_prefix,
            Some(InstalledRasterGridPrefix {
                short_sections_added: false,
                horizontal_lag_present: true,
            })
        );
        assert!(executor.sheet.vertical_lag.is_some());
        assert!(executor.cleaner_finished);
        assert!(executor.step_finished);
        assert_eq!(executor.builder.stages().finish_count, 1);
    }

    #[test]
    fn retrieve_step_failure_installs_created_prefix_but_aborts_cleaner_and_scores() {
        let mut executor = raster_executor(RasterStages {
            retrieve_failure: Some(true),
            ..RasterStages::default()
        });
        executor.sheet.horizontal_lag = None;

        assert_eq!(
            executor.run(),
            Err(HeadlessGridError::Build("retrieve step failure"))
        );

        assert_eq!(
            executor.sheet.installed_raster_prefix,
            Some(InstalledRasterGridPrefix {
                short_sections_added: false,
                horizontal_lag_present: true,
            })
        );
        assert!(executor.sheet.vertical_lag.is_some());
        assert!(matches!(
            executor.sheet.staffs[0].lines[0],
            HeadlessStaffLine::Filament { .. }
        ));
        assert!(!executor.cleaner_finished);
        assert!(!executor.step_finished);
        assert!(executor.book.scores.is_empty());
        assert_eq!(executor.builder.stages().finish_count, 1);
    }

    #[test]
    fn production_retrieve_materializes_candidate_metadata_and_filament() {
        let mut executor = production_executor(prepared_primary(production_section()));

        executor.run().unwrap();

        assert!(
            matches!(executor.build_outcome, Some(GridBuildOutcome::Completed)),
            "unexpected build outcome: {:?}",
            executor.build_outcome
        );

        let [staff] = executor.sheet.staffs.as_slice() else {
            panic!("one prepared staff expected");
        };
        assert_eq!(staff.id, 1);
        assert_eq!(staff.kind, StaffCandidateKind::OneLine);
        assert_eq!((staff.left, staff.right), (0.0, 40.0));
        assert_eq!(staff.interline, 10);
        assert!(!staff.small);
        assert!(!staff.short);
        assert!(matches!(
            staff.lines.as_slice(),
            [HeadlessStaffLine::Persistent {
                source_line_id: 7,
                ..
            }]
        ));
        assert!(executor.cleaner_finished);
        assert!(executor.step_finished);
    }

    #[test]
    fn raw_raster_constructor_installs_split_middle_staff_and_raster_prefix() {
        let base = executor();
        let mut executor = HeadlessGridExecutor::from_raw_raster_lines(
            split_middle_source(),
            raw_raster_parameters(),
            raw_primary_parameters(),
            raw_lines_parameters(),
            RasterStages::default(),
            base.sheet,
            base.book,
        );

        executor
            .run_grid_step_stage(GridStepStage::BuildGrid)
            .unwrap();

        assert!(
            matches!(executor.build_outcome, Some(GridBuildOutcome::Completed)),
            "unexpected build outcome: {:?}",
            executor.build_outcome
        );
        assert_eq!(
            executor.sheet.installed_raster_prefix,
            Some(InstalledRasterGridPrefix {
                short_sections_added: true,
                horizontal_lag_present: true,
            })
        );
        let [staff] = executor.sheet.staffs.as_slice() else {
            panic!("one raw-retrieved staff expected");
        };
        assert_eq!(staff.kind, StaffCandidateKind::Standard);
        assert_eq!(staff.lines.len(), 5);
        assert_eq!(
            staff
                .lines
                .iter()
                .filter(|line| matches!(
                    line,
                    HeadlessStaffLine::Filament { filament, .. }
                        if filament.sections().len() == 2
                ))
                .count(),
            1
        );
        assert_eq!(executor.builder.stages().downstream().finish_count, 1);
    }

    #[test]
    fn raw_completion_constructor_runs_every_concrete_stage_in_java_order() {
        let base = executor();
        let mut executor = HeadlessGridExecutor::from_raw_raster_complete_lines(
            split_middle_with_discarded_source(),
            raw_raster_parameters(),
            raw_primary_parameters(),
            placeholder_slope_lines_parameters(),
            RasterStages::default(),
            raw_completion_systems(),
            raw_completion_parameters(120),
            1_000,
            true,
            base.sheet,
            base.book,
        );

        executor
            .run_grid_step_stage(GridStepStage::BuildGrid)
            .unwrap();

        assert!(matches!(
            executor.build_outcome,
            Some(GridBuildOutcome::Completed)
        ));
        let stages = executor.builder.stages();
        let state = stages.state().expect("retained concrete completion state");
        assert_eq!(
            state.completed_stages,
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
        assert_eq!(state.global_slope, Some(0.0));
        assert_eq!(state.completion_systems, Some(raw_completion_systems()));
        assert_eq!(state.defined_endpoints.len(), 1);
        assert_eq!(
            state
                .fill_hole_invocations
                .iter()
                .map(|audit| (audit.invocation, audit.completed))
                .collect::<Vec<_>>(),
            [
                (HoleFillInvocation::Initial, true),
                (HoleFillInvocation::AfterPolish, true),
                (HoleFillInvocation::Final, true),
            ]
        );
        assert!(state.discarded_filament_steals.is_empty());
        assert!(state.discarded_filament_recomputations.is_empty());
        assert_eq!(state.discarded_filaments.len(), 1);
        assert_eq!(
            state.discarded_filaments[0]
                .line
                .filament
                .sections()
                .iter()
                .map(Section::id)
                .collect::<Vec<_>>(),
            [1]
        );
        assert_eq!(state.section_inclusion_batches.len(), 10);
        assert!(
            state
                .section_inclusion_batches
                .iter()
                .all(|batch| batch.endpoints_recomputed)
        );
        assert_eq!(
            state
                .section_inclusion_batches
                .iter()
                .map(|batch| batch.stage)
                .collect::<Vec<_>>(),
            [
                LineCompletionStage::IncludeThickSections,
                LineCompletionStage::IncludeThickSections,
                LineCompletionStage::IncludeThickSections,
                LineCompletionStage::IncludeThickSections,
                LineCompletionStage::IncludeThickSections,
                LineCompletionStage::IncludeThinSections,
                LineCompletionStage::IncludeThinSections,
                LineCompletionStage::IncludeThinSections,
                LineCompletionStage::IncludeThinSections,
                LineCompletionStage::IncludeThinSections,
            ]
        );
        assert!(state.curvature_removals.is_empty());
        assert!(state.curvature_recomputations.is_empty());
        assert_eq!(state.sticker_section_ids, [1]);
        assert!(state.sticker_inclusion_batches.is_empty());
        assert!(state.crossing_line_inspections.is_empty());
        assert_eq!(stages.completion().finish_count(), 1);
        assert_eq!(stages.upstream().downstream().finish_count, 1);
        // SigPromotingBuilder consumed the one-shot sheet handoff after
        // ProcessBars, yet completion still retained the same measured slope.
        assert_eq!(executor.sheet.skew.expect("forwarded raw slope").slope, 0.0);
        assert_eq!(executor.sheet.staffs.len(), 1);
        assert_eq!(executor.sheet.staffs[0].lines.len(), 5);
    }

    #[test]
    fn raw_completion_late_failure_retains_exact_audit_and_staff_prefix() {
        let base = executor();
        let mut executor = HeadlessGridExecutor::from_raw_raster_complete_lines(
            split_middle_with_discarded_source(),
            raw_raster_parameters(),
            raw_primary_parameters(),
            placeholder_slope_lines_parameters(),
            RasterStages::default(),
            raw_completion_systems(),
            raw_completion_parameters(-1),
            1_000,
            true,
            base.sheet,
            base.book,
        );

        executor
            .run_grid_step_stage(GridStepStage::BuildGrid)
            .unwrap();

        assert!(matches!(
            executor.build_outcome.as_ref(),
            Some(GridBuildOutcome::Swallowed {
                stage: GridBuildStage::CompleteLines,
                error: HeadlessBuildOtherError::Builder(RasterGridOtherError::Collaborator(
                    ProductionCompleteLinesError::Completion(DefineEndPointsCompletionError::Next(
                        IncludeDiscardedCompletionError::Next(FillHolesCompletionError::Next(
                            IncludeSectionsCompletionError::Next(
                                PolishCurvaturesCompletionError::PolishCurvatures(
                                    PolishCurvaturesError::InvalidMinimumRadius(-1)
                                )
                            )
                        ))
                    ))
                )),
            })
        ));
        let stages = executor.builder.stages();
        let state = stages.state().expect("retained late failure prefix");
        assert_eq!(
            state.completed_stages,
            [
                LineCompletionStage::DefineEndPoints,
                LineCompletionStage::IncludeDiscardedFilaments,
                LineCompletionStage::FillHolesInitial,
                LineCompletionStage::DispatchHorizontalSections,
                LineCompletionStage::IncludeThickSections,
                LineCompletionStage::IncludeThinSections,
            ]
        );
        assert_eq!(state.global_slope, Some(0.0));
        assert_eq!(state.defined_endpoints.len(), 1);
        assert_eq!(
            state
                .fill_hole_invocations
                .iter()
                .map(|audit| (audit.invocation, audit.completed))
                .collect::<Vec<_>>(),
            [(HoleFillInvocation::Initial, true)]
        );
        assert_eq!(state.section_inclusion_batches.len(), 10);
        assert!(
            state
                .section_inclusion_batches
                .iter()
                .all(|batch| batch.endpoints_recomputed)
        );
        assert_eq!(state.discarded_filaments.len(), 1);
        assert!(state.discarded_filament_steals.is_empty());
        assert!(state.discarded_filament_recomputations.is_empty());
        assert!(state.curvature_removals.is_empty());
        assert!(state.curvature_recomputations.is_empty());
        assert!(state.sticker_section_ids.is_empty());
        assert!(state.sticker_inclusion_batches.is_empty());
        assert!(state.crossing_line_inspections.is_empty());
        assert_eq!(stages.completion().finish_count(), 1);
        assert_eq!(stages.upstream().downstream().finish_count, 1);
        // The headless handoff occurs after the swallowed build and must
        // expose the exact mutation prefix rather than the pre-completion
        // staff snapshot.
        assert_eq!(executor.sheet.staffs.len(), 1);
        assert_eq!(executor.sheet.staffs[0].lines.len(), 5);
        assert!(executor.sheet.skew.is_some());
    }

    #[test]
    fn raw_raster_constructor_uses_measured_slope_and_installs_sloped_fallback() {
        let base = executor();
        let mut executor = HeadlessGridExecutor::from_raw_raster_lines(
            positive_slope_source(),
            raw_raster_parameters(),
            raw_primary_parameters(),
            placeholder_slope_lines_parameters(),
            RasterStages::default(),
            base.sheet,
            base.book,
        );

        executor
            .run_grid_step_stage(GridStepStage::BuildGrid)
            .unwrap();

        assert!(
            matches!(executor.build_outcome, Some(GridBuildOutcome::Completed)),
            "unexpected build outcome: {:?}",
            executor.build_outcome
        );
        let slope = executor.sheet.skew.expect("installed raw skew").slope;
        assert!(slope > 0.03 && slope < 0.06, "measured slope: {slope}");
        assert_ne!(slope, placeholder_slope_lines_parameters().global_slope());
        assert_eq!(executor.sheet.staffs.len(), 1);
        assert_eq!(executor.sheet.staffs[0].lines.len(), 5);
        assert_eq!(executor.sheet.sloped_line_fallbacks.len(), 1);
        let fallback_geometry = executor.sheet.sloped_line_fallbacks[0]
            .filament
            .geometry()
            .unwrap();
        let (start_x, start_y) = fallback_geometry.start();
        let (stop_x, stop_y) = fallback_geometry.stop();
        assert!((stop_y - start_y) / (stop_x - start_x) > 0.08);

        // A system geometry can be attached or refreshed after RetrieveLines.
        // The population entry point must derive its deskewed abscissa from
        // the stored sheet transform at point of use.
        executor.sheet.population.geometries[0].deskewed_upper_left_x = -999.0;
        StaffLineCleanerExecutor::populate_systems(&mut executor).unwrap();
        let skew = executor.sheet.skew.expect("stored sheet skew");
        near(
            executor.sheet.population.geometries[0].deskewed_upper_left_x,
            skew.deskewed_x(
                f64::from(executor.sheet.population.geometries[0].left),
                f64::from(executor.sheet.population.geometries[0].top),
            ),
        );
    }

    #[test]
    fn raw_raster_constructor_installs_failure_prefix_without_staff_handoff() {
        let base = executor();
        let mut sheet = base.sheet;
        sheet.staffs.clear();
        let mut invalid = raw_primary_parameters();
        invalid.sampling_dx = 0;
        let mut executor = HeadlessGridExecutor::from_raw_raster_lines(
            split_middle_source(),
            raw_raster_parameters(),
            invalid,
            raw_lines_parameters(),
            RasterStages::default(),
            sheet,
            base.book,
        );

        executor
            .run_grid_step_stage(GridStepStage::BuildGrid)
            .unwrap();

        assert!(matches!(
            executor.build_outcome,
            Some(GridBuildOutcome::Swallowed {
                stage: GridBuildStage::RetrieveLines,
                ..
            })
        ));
        assert_eq!(
            executor.sheet.installed_raster_prefix,
            Some(InstalledRasterGridPrefix {
                short_sections_added: false,
                horizontal_lag_present: true,
            })
        );
        assert!(executor.sheet.staffs.is_empty());
        assert!(executor.sheet.skew.is_none());
        assert!(executor.sheet.sloped_line_fallbacks.is_empty());
        assert_eq!(executor.builder.stages().downstream().finish_count, 1);
    }

    #[test]
    fn production_retrieve_rejects_filament_from_different_lag_provenance() {
        let mut alien_table = RunTable::new(Orientation::Horizontal, 42, 20).unwrap();
        alien_table.add_run(11, Run::new(0, 40)).unwrap();
        let alien = build_sections(&alien_table, JunctionPolicy::All).remove(0);
        assert_eq!(alien.id(), production_section().id());
        let mut executor = production_executor(prepared_primary(alien));
        executor.sheet.staffs.clear();

        executor.run().unwrap();

        assert!(executor.sheet.staffs.is_empty());
        assert!(matches!(
            executor.build_outcome,
            Some(GridBuildOutcome::Swallowed {
                stage: GridBuildStage::RetrieveLines,
                ..
            })
        ));
        assert!(executor.cleaner_finished);
        assert!(executor.step_finished);
    }

    #[test]
    fn prepared_staff_replacement_restores_process_bars_recorded_ownership() {
        let mut executor = production_bars_executor();

        executor.run().unwrap();

        assert!(
            matches!(executor.build_outcome, Some(GridBuildOutcome::Completed)),
            "unexpected build outcome: {:?}",
            executor.build_outcome
        );

        let [staff] = executor.sheet.staffs.as_slice() else {
            panic!("one retrieved staff expected");
        };
        assert_eq!(staff.id, 1);
        let system = &executor.sheet.sig.systems[0];
        assert_eq!(
            (
                staff.barlines.len(),
                system.bar_tail.staff_barlines[&1].len()
            ),
            (1, 1)
        );
        assert_eq!(system.bar_tail.staff_barlines[&1], staff.barlines);
        assert_eq!(system.bar_tail.parts.len(), 1);
        assert!(system.bar_tail.contextualized);
    }
}
