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
    filament::StaffFilament,
    grid_lifecycle::{
        GridBuildExecutor, GridBuildOutcome, GridStepExecutor, GridStepStage, build_grid_info,
        run_grid_step,
    },
    grid_sig::{BarGroupPromotionError, ConnectionPromotionWarning, GridSig},
    lag_rebuild::{RebuildHorizontalLagError, RegisteredHorizontalLag, rebuild_horizontal_lag},
    line_short_sections::NoopVipSectionHook,
    peak_graph::{PeakEdgeId, PeakGraph},
    run_table::{RunTable, RunTableError},
    staff_line_cleaner::{OriginalStaffLine, StaffLineCleanerExecutor, clean_staff_lines},
    staff_line_conversion::{
        OriginalGlyphRegistrar, PersistentStaffLine, StaffGlyph, StaffLineConversionError,
        to_registered_staff_line,
    },
    staff_peak::StaffPeak,
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
    pub maximum_group_gap: i32,
    pub interline: f64,
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
    pub horizontal_lag: Option<RegisteredHorizontalLag>,
    pub population: HeadlessPopulationState,
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
            cleaner_finished: false,
            step_finished: false,
        }
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

impl<Builder> GridStepExecutor for HeadlessGridExecutor<Builder>
where
    Builder: GridBuildExecutor,
{
    type Error = HeadlessGridError<Builder::StepError>;

    fn run_grid_step_stage(&mut self, stage: GridStepStage) -> Result<(), Self::Error> {
        match stage {
            GridStepStage::BuildGrid => {
                let mut builder = SigPromotingBuilder {
                    builder: &mut self.builder,
                    sig: &mut self.sheet.sig,
                    promotion_failure: &mut self.sheet.promotion_failure,
                };
                self.build_outcome =
                    Some(build_grid_info(&mut builder).map_err(HeadlessGridError::Build)?);
                Ok(())
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
    promotion_failure: &'a mut Option<HeadlessGridPromotionError>,
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
        self.builder
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
            })?;
        if stage == audiveris_image::grid_lifecycle::GridBuildStage::ProcessBars {
            promote_grid_sigs(self.sig).map_err(|error| {
                audiveris_image::grid_lifecycle::GridStageFailure::Other(
                    HeadlessBuildOtherError::Promotion(error),
                )
            })?;
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

fn promote_grid_sigs(state: &mut HeadlessGridSigState) -> Result<(), HeadlessGridPromotionError> {
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
    Ok(())
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
    use super::*;
    use audiveris_image::{
        bar_alignment::{AlignmentPeak, BarImpacts},
        bar_column::{PeakId, StaffId},
        bars_logic::{PeakWidthClass, VerticalInterKind, VerticalMedian, plan_connection_inters},
        filament::FilamentError,
        grid_lifecycle::{GridBuildStage, GridStageFailure},
        grid_sig::{GridSigNode, GridSigRelation},
        line_short_sections::HorizontalSectionLag,
        run_table::{Orientation, Run},
        staff_peak::StaffPeak,
        system_population::{
            BoundarySegment, PopulationReferencePart, PopulationReferenceStaff,
            PopulationStaffConfig, PopulationSystemRefState, StaffBoundary,
        },
    };

    #[derive(Default)]
    struct SuccessfulBuilder {
        stages: Vec<GridBuildStage>,
        finish_count: usize,
        failure: Option<(GridBuildStage, bool)>,
        warnings: Vec<GridBuildStage>,
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
        HeadlessSystemSigState {
            system_id: 1,
            sig: GridSig::default(),
            vertical_plans,
            staff_peaks: peaks,
            maximum_group_gap: 3,
            interline: 10.0,
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
                horizontal_lag: Some(RegisteredHorizontalLag::Populated(lag)),
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

        promote_grid_sigs(&mut state).unwrap();

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

        promote_grid_sigs(&mut state).unwrap();

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
}
