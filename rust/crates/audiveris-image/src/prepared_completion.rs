// SPDX-License-Identifier: AGPL-3.0-or-later

//! Production-backed `CompleteLines` over prepared staff and raster state.
//!
//! Binary-buffer acquisition and horizontal-section dispatch are concrete.
//! The remaining geometry-heavy stages are explicit collaborators operating
//! on the same retained state. Java's inner `finally` is preserved: buffer
//! acquisition precedes it, so acquisition failure does not finish the inner
//! completion timer, while every later success or failure does.

use std::collections::{HashMap, HashSet};

use crate::{
    crossing_completion::{
        InspectCrossingChunksCompletion, InspectCrossingChunksParameters,
        PreparedCrossingLineInspection,
    },
    curvature_completion::{
        PolishCurvaturesCompletion, PreparedCurvatureRecomputation, PreparedCurvatureRemoval,
    },
    discarded_completion::{
        IncludeDiscardedFilamentsCompletion, IncludeDiscardedFilamentsParameters,
        PreparedCompletionSystem, PreparedDiscardedFilamentSteal, PreparedFilamentRecomputation,
    },
    filament::regularize_five_line_staff,
    grid_lifecycle::{GridBuildStage, GridStageFailure},
    line_completion::{
        LineCompletionExecutor, LineCompletionStage, complete_lines as run_line_completion,
    },
    line_endpoints::{
        DefineEndPointsCompletion, DefineEndPointsParameters, PreparedStaffEndPoints,
    },
    line_holes::{FillHolesInitialCompletion, PreparedHoleFillInvocation},
    line_section_dispatch::dispatch_horizontal_sections,
    prepared_bars::{PreparedBarsHandoff, PreparedBarsStage},
    prepared_lines::{
        PreparedStaff, PreparedStaffHandoff, PreparedStaffStage, RawDiscardedFilament,
        RawLineMetadataHandoff, RawLineMetadataStage,
    },
    raster_grid_builder::{RasterGridBuildState, RemainingRasterGridStages},
    run_table::RunTable,
    section::Section,
    section_completion::{
        IncludeSectionsCompletion, IncludeSectionsParameters, PreparedSectionInclusionBatch,
    },
    sticker_completion::{
        IncludeStickersCompletion, IncludeStickersParameters, PreparedStickerInclusionBatch,
    },
};

#[derive(Clone, Debug)]
pub struct PreparedCompletionState {
    pub staffs: Vec<PreparedStaff>,
    /// Measured raw sheet slope retained after `RetrieveLines`. Concrete
    /// crossing inspection consumes this late-bound value instead of the
    /// placeholder slope supplied before the raster was measured.
    pub global_slope: Option<f64>,
    /// Exact system/staff traversal. `None` is a bounded missing seam; flat
    /// staff order must not be silently treated as one system.
    pub completion_systems: Option<Vec<PreparedCompletionSystem>>,
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
    /// Separate audit boundaries for Java's three repeated hole-fill calls.
    pub fill_hole_invocations: Vec<PreparedHoleFillInvocation>,
    pub discarded_filament_steals: Vec<PreparedDiscardedFilamentSteal>,
    pub discarded_filament_recomputations: Vec<PreparedFilamentRecomputation>,
    pub section_inclusion_batches: Vec<PreparedSectionInclusionBatch>,
    /// Ordered fixed candidate tally produced by Java `getAllStickers`.
    pub sticker_section_ids: Vec<usize>,
    pub sticker_inclusion_batches: Vec<PreparedStickerInclusionBatch>,
    pub crossing_line_inspections: Vec<PreparedCrossingLineInspection>,
    pub curvature_removals: Vec<PreparedCurvatureRemoval>,
    pub curvature_recomputations: Vec<PreparedCurvatureRecomputation>,
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

/// All scale-resolved inputs consumed by Java's concrete `completeLines`
/// collaborators. Stage order is not configurable; it is fixed by
/// [`crate::line_completion::complete_lines`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProductionLineCompletionParameters {
    pub define_end_points: DefineEndPointsParameters,
    pub include_discarded_filaments: IncludeDiscardedFilamentsParameters,
    pub include_sections: IncludeSectionsParameters,
    pub minimum_curvature_radius: i32,
    pub include_stickers: IncludeStickersParameters,
    pub inspect_crossing_chunks: InspectCrossingChunksParameters,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnclaimedLineCompletionStage(pub LineCompletionStage);

impl std::fmt::Display for UnclaimedLineCompletionStage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "unclaimed concrete completion stage {:?}",
            self.0
        )
    }
}

impl std::error::Error for UnclaimedLineCompletionStage {}

#[doc(hidden)]
pub struct ProductionCompletionTail<StepError> {
    marker: std::marker::PhantomData<fn() -> StepError>,
}

impl<StepError> Default for ProductionCompletionTail<StepError> {
    fn default() -> Self {
        Self {
            marker: std::marker::PhantomData,
        }
    }
}

impl<StepError> RemainingLineCompletionStages for ProductionCompletionTail<StepError> {
    type StepError = StepError;
    type OtherError = UnclaimedLineCompletionStage;

    fn run_stage(
        &mut self,
        stage: LineCompletionStage,
        _state: &mut PreparedCompletionState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        Err(GridStageFailure::Other(UnclaimedLineCompletionStage(stage)))
    }

    fn log_swallowed_error(&mut self, _error: &Self::OtherError) {}

    fn finish(&mut self) {}
}

type ProductionLineCompletionChain<StepError> = DefineEndPointsCompletion<
    IncludeDiscardedFilamentsCompletion<
        FillHolesInitialCompletion<
            IncludeSectionsCompletion<
                PolishCurvaturesCompletion<
                    IncludeStickersCompletion<
                        InspectCrossingChunksCompletion<ProductionCompletionTail<StepError>>,
                    >,
                >,
            >,
        >,
    >,
>;

/// Strict concrete owner for every non-dispatch `completeLines` stage.
///
/// The private terminal rejects any stage not claimed by the wrappers. The
/// facade also exposes the Java inner `finally` count without requiring
/// callers to know the nested collaborator shape.
pub struct ProductionLineCompletion<StepError> {
    chain: ProductionLineCompletionChain<StepError>,
    finish_count: usize,
}

impl<StepError> ProductionLineCompletion<StepError> {
    #[must_use]
    pub const fn finish_count(&self) -> usize {
        self.finish_count
    }
}

impl<StepError> RemainingLineCompletionStages for ProductionLineCompletion<StepError> {
    type StepError = StepError;
    type OtherError =
        <ProductionLineCompletionChain<StepError> as RemainingLineCompletionStages>::OtherError;

    fn run_stage(
        &mut self,
        stage: LineCompletionStage,
        state: &mut PreparedCompletionState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        self.chain.run_stage(stage, state)
    }

    fn log_swallowed_error(&mut self, error: &Self::OtherError) {
        self.chain.log_swallowed_error(error);
    }

    fn finish(&mut self) {
        self.finish_count += 1;
        self.chain.finish();
    }
}

#[must_use]
pub fn production_line_completion<StepError>(
    parameters: ProductionLineCompletionParameters,
) -> ProductionLineCompletion<StepError> {
    let tail = ProductionCompletionTail::default();
    let crossing = InspectCrossingChunksCompletion::new(parameters.inspect_crossing_chunks, tail);
    let stickers = IncludeStickersCompletion::new(parameters.include_stickers, crossing);
    let curvature = PolishCurvaturesCompletion::new(parameters.minimum_curvature_radius, stickers);
    let sections = IncludeSectionsCompletion::new(parameters.include_sections, curvature);
    let holes = FillHolesInitialCompletion::new(sections);
    let discarded =
        IncludeDiscardedFilamentsCompletion::new(parameters.include_discarded_filaments, holes);
    let chain = DefineEndPointsCompletion::new(parameters.define_end_points, discarded);
    ProductionLineCompletion {
        chain,
        finish_count: 0,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProductionCompleteLinesError<UpstreamError, CompletionError> {
    MissingPreparedStaffs,
    MissingHorizontalLag,
    BinaryBufferUnavailable,
    BinaryBufferProvenanceMismatch,
    CompletionOwnership(PreparedCompletionOwnershipError),
    TentativeGeometry(crate::filament::FilamentError),
    Upstream(UpstreamError),
    Completion(CompletionError),
}

/// Invalid ownership at the exact `ProcessBars` -> `CompleteLines` join.
///
/// These are intentionally distinct from the later completion collaborators:
/// no endpoint or section mutation may begin until exact Java system/staff
/// traversal has been proven.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreparedCompletionOwnershipError {
    MissingPreparedBarsHandoff,
    MissingRawBarsOwnership,
    DuplicatePreparedStaff(usize),
    InvalidSystemId(usize),
    DuplicateSystem(usize),
    EmptySystem(usize),
    UnknownSystemStaff {
        system_id: usize,
        staff_id: usize,
    },
    StaffInMultipleSystems(usize),
    UnownedStaff(usize),
    StaffOrderMismatch {
        position: usize,
        expected: usize,
        actual: usize,
    },
    ExplicitOwnershipMismatch {
        explicit: Vec<PreparedCompletionSystem>,
        automatic: Vec<PreparedCompletionSystem>,
    },
}

/// One-shot exact system/staff ownership emitted by a completed raw bars
/// producer. Unlike [`PreparedBarsStage`], this seam does not ask downstream
/// completion to reconstruct or replay bar/SIG collaborators.
pub trait PreparedCompletionOwnershipStage {
    fn take_prepared_completion_ownership(&mut self) -> Option<Vec<PreparedCompletionSystem>>;
}

type DirectOwnershipSource<Upstream> = fn(&mut Upstream) -> Option<Vec<PreparedCompletionSystem>>;

pub struct ProductionCompleteLines<Upstream, Completion> {
    upstream: Upstream,
    completion: Completion,
    prepared_binary: Option<RunTable>,
    maximum_thin_weight: usize,
    inspect_crossing_chunks: bool,
    /// Caller-supplied topology retained for prepared/non-raw entry points.
    completion_systems: Option<Vec<PreparedCompletionSystem>>,
    /// Runtime topology derived from the successful ProcessBars handoff.
    automatic_completion_systems: Option<Vec<PreparedCompletionSystem>>,
    prepared_bars_source: Option<fn(&mut Upstream) -> Option<PreparedBarsHandoff>>,
    direct_ownership_source: Option<DirectOwnershipSource<Upstream>>,
    /// One-shot direct ownership retained even when a later ProcessBars join
    /// fails, matching the prepared-bars failure-prefix contract.
    direct_ownership_handoff: Option<Vec<PreparedCompletionSystem>>,
    /// One-shot handoff captured before ownership validation and forwarded to
    /// the sheet even when a later join check rejects the attempt.
    prepared_bars_handoff: Option<PreparedBarsHandoff>,
    /// Retained copy of the refined staff abscissae, read at `processBars`
    /// time. `prepared_bars_handoff` above is one-shot and the sheet takes it
    /// during that same stage, so `completeLines` cannot read the limits back
    /// out of it; this mirrors the `raw_metadata` / `raw_metadata_handoff`
    /// split for the same reason.
    refined_staff_limits: Vec<(usize, i32, i32)>,
    state: Option<PreparedCompletionState>,
    /// Retained copy consumed by concrete completion after ProcessBars.
    raw_metadata: Option<RawLineMetadataHandoff>,
    /// One-shot clone forwarded to the sheet immediately after ProcessBars.
    raw_metadata_handoff: Option<RawLineMetadataHandoff>,
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
            completion_systems: None,
            automatic_completion_systems: None,
            prepared_bars_source: None,
            direct_ownership_source: None,
            direct_ownership_handoff: None,
            prepared_bars_handoff: None,
            refined_staff_limits: Vec::new(),
            state: None,
            raw_metadata: None,
            raw_metadata_handoff: None,
        }
    }

    /// Supply exact Java system/staff traversal for discarded-filament and
    /// section inclusion. Flat prepared-staff order is deliberately not
    /// inferred as one system.
    #[must_use]
    pub fn with_completion_systems(
        mut self,
        completion_systems: Vec<PreparedCompletionSystem>,
    ) -> Self {
        self.completion_systems = Some(completion_systems);
        self
    }

    /// Derive exact completion system/staff traversal from the prepared bars
    /// producer. This is the production raw path: the producer's stable IDs
    /// and order are authoritative, while an optional explicit topology is
    /// accepted only when it is byte-for-byte identical.
    #[must_use]
    pub fn with_completion_systems_from_prepared_bars(mut self) -> Self
    where
        Upstream: PreparedBarsStage,
    {
        self.prepared_bars_source = Some(Upstream::take_prepared_bars_handoff);
        self
    }

    /// Consume exact ownership directly from a completed raw-bars adapter.
    /// No prepared bar plans or tail collaborators are injected or replayed.
    #[must_use]
    pub fn with_completion_systems_from_raw_bars(mut self) -> Self
    where
        Upstream: PreparedCompletionOwnershipStage,
    {
        self.direct_ownership_source = Some(Upstream::take_prepared_completion_ownership);
        self
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
        self.raw_metadata_handoff
            .take()
            .or_else(|| self.upstream.take_raw_line_metadata_handoff())
    }
}

impl<Upstream, Completion> PreparedBarsStage for ProductionCompleteLines<Upstream, Completion>
where
    Upstream: PreparedBarsStage,
{
    fn take_prepared_bars_handoff(&mut self) -> Option<PreparedBarsHandoff> {
        self.prepared_bars_handoff
            .take()
            .or_else(|| self.upstream.take_prepared_bars_handoff())
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
        self.raw_metadata = None;
        self.raw_metadata_handoff = None;
        self.automatic_completion_systems = None;
        self.prepared_bars_handoff = None;
        self.direct_ownership_handoff = None;
        self.state = None;
        let result = self
            .upstream
            .retrieve_lines(state)
            .map_err(map_upstream_failure);
        // Raw retrieval installs metadata before its final materialization
        // join, so retain that legitimate prefix even when the delegated
        // RetrieveLines attempt fails later.
        let metadata = self.upstream.take_raw_line_metadata_handoff();
        self.raw_metadata = metadata.clone();
        self.raw_metadata_handoff = metadata;
        result
    }

    fn process_bars(
        &mut self,
        state: &mut RasterGridBuildState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        self.automatic_completion_systems = None;
        self.prepared_bars_handoff = None;
        self.direct_ownership_handoff = None;
        let result = self
            .upstream
            .process_bars(state)
            .map_err(map_upstream_failure);
        if let Some(extract) = self.prepared_bars_source {
            self.prepared_bars_handoff = extract(&mut self.upstream);
            self.refined_staff_limits = self
                .prepared_bars_handoff
                .as_ref()
                .map(collect_refined_staff_limits)
                .unwrap_or_default();
        }
        if let Some(extract) = self.direct_ownership_source {
            self.direct_ownership_handoff = extract(&mut self.upstream);
        }
        // Preserve an upstream failure and its exact bar prefix. Ownership
        // validation must never mask the producer's earlier error.
        result?;
        if self.prepared_bars_source.is_none() && self.direct_ownership_source.is_none() {
            return Ok(());
        }
        let staffs = self.upstream.prepared_staff_handoff().ok_or_else(|| {
            GridStageFailure::Other(ProductionCompleteLinesError::MissingPreparedStaffs)
        })?;
        let automatic = if self.direct_ownership_source.is_some() {
            let direct = self.direct_ownership_handoff.as_ref().ok_or_else(|| {
                GridStageFailure::Other(ProductionCompleteLinesError::CompletionOwnership(
                    PreparedCompletionOwnershipError::MissingRawBarsOwnership,
                ))
            })?;
            validate_direct_completion_ownership(staffs, direct).map_err(|error| {
                GridStageFailure::Other(ProductionCompleteLinesError::CompletionOwnership(error))
            })?
        } else {
            let handoff = self.prepared_bars_handoff.as_ref().ok_or_else(|| {
                GridStageFailure::Other(ProductionCompleteLinesError::CompletionOwnership(
                    PreparedCompletionOwnershipError::MissingPreparedBarsHandoff,
                ))
            })?;
            validate_prepared_completion_ownership(staffs, handoff).map_err(|error| {
                GridStageFailure::Other(ProductionCompleteLinesError::CompletionOwnership(error))
            })?
        };
        if let Some(explicit) = &self.completion_systems
            && explicit != &automatic
        {
            return Err(GridStageFailure::Other(
                ProductionCompleteLinesError::CompletionOwnership(
                    PreparedCompletionOwnershipError::ExplicitOwnershipMismatch {
                        explicit: explicit.clone(),
                        automatic,
                    },
                ),
            ));
        }
        self.automatic_completion_systems = Some(automatic);
        Ok(())
    }

    fn complete_lines(
        &mut self,
        raster: &mut RasterGridBuildState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        let mut staffs = self
            .upstream
            .prepared_staff_handoff()
            .ok_or_else(|| {
                GridStageFailure::Other(ProductionCompleteLinesError::MissingPreparedStaffs)
            })?
            .staffs
            .clone();
        // Java `defineEndPoints` pins every line ending at
        // `staff.getAbscissa(side)`, which `BarsRetriever` refined during
        // `processBars`: the start column sets LEFT, `verifyLinesRoot` may push
        // it right, and `refineRightEnds` sets RIGHT. The prepared staffs still
        // carry the cluster candidate limits from `retrieveLines`, so adopt the
        // refined ones before the endpoints are computed from them.
        adopt_refined_staff_limits(&mut staffs, &self.refined_staff_limits);
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
            global_slope: self
                .raw_metadata
                .as_ref()
                .map(|metadata| metadata.global_slope),
            completion_systems: self
                .automatic_completion_systems
                .clone()
                .or_else(|| self.completion_systems.clone()),
            discarded_filaments,
            horizontal_sections,
            binary_buffer: None,
            thick_section_ids: Vec::new(),
            thin_section_ids: Vec::new(),
            defined_endpoints: Vec::new(),
            fill_hole_invocations: Vec::new(),
            discarded_filament_steals: Vec::new(),
            discarded_filament_recomputations: Vec::new(),
            section_inclusion_batches: Vec::new(),
            sticker_section_ids: Vec::new(),
            sticker_inclusion_batches: Vec::new(),
            crossing_line_inspections: Vec::new(),
            curvature_removals: Vec::new(),
            curvature_recomputations: Vec::new(),
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
            .map_err(map_completion_failure)?;
        // Completion intentionally mutates individual filaments (endpoints,
        // holes, sections, stickers and curvature). For tentative ridge
        // staffs that invalidates the shared spline installed at retrieval and
        // can reintroduce beam/stem contamination. Reapply the five-line
        // constraint once, after every evidence-gathering stage and before the
        // staff-line cleaner persists the geometry.
        for staff in &mut self.state.as_mut().expect("completion state").staffs {
            if staff.tentative && staff.lines.len() == 5 {
                regularize_five_line_staff(
                    staff.lines.iter_mut().map(|line| &mut line.filament),
                    staff.interline,
                )
                .map_err(|error| {
                    GridStageFailure::Other(ProductionCompleteLinesError::TentativeGeometry(error))
                })?;
            }
        }
        Ok(())
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

/// Flattens the bars handoff's per-system staff abscissae into
/// `(staff id, left, right)`.
fn collect_refined_staff_limits(handoff: &PreparedBarsHandoff) -> Vec<(usize, i32, i32)> {
    handoff
        .systems
        .iter()
        .flat_map(|system| {
            system
                .staff_ids
                .iter()
                .zip(system.staff_limits.iter())
                .map(|(id, &(left, right))| (*id, left, right))
        })
        .collect()
}

/// Replaces each prepared staff's abscissae with the ones `BarsRetriever`
/// refined, keyed by staff id.
///
/// A staff the bars handoff does not mention keeps its candidate limits: that
/// is a staff `processBars` never saw, and inventing a limit for it would be
/// worse than leaving the retrieval's own answer in place.
fn adopt_refined_staff_limits(staffs: &mut [PreparedStaff], refined: &[(usize, i32, i32)]) {
    let refined: HashMap<usize, (i32, i32)> = refined
        .iter()
        .map(|&(id, left, right)| (id, (left, right)))
        .collect();
    for staff in staffs {
        if let Some(&(left, right)) = refined.get(&staff.id) {
            staff.left = f64::from(left);
            staff.right = f64::from(right);
        }
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
            ProductionCompleteLinesError::CompletionOwnership(error) => {
                ProductionCompleteLinesError::CompletionOwnership(error)
            }
            ProductionCompleteLinesError::TentativeGeometry(error) => {
                ProductionCompleteLinesError::TentativeGeometry(error)
            }
            ProductionCompleteLinesError::Completion(error) => {
                ProductionCompleteLinesError::Completion(error)
            }
            ProductionCompleteLinesError::Upstream(never) => match never {},
        }),
    }
}

/// Validate and convert exact prepared-bars ownership for concrete line
/// completion. The returned order is the producer order; no sorting,
/// deduplication, renumbering, or singleton-system inference is performed.
pub fn validate_prepared_completion_ownership(
    prepared: &PreparedStaffHandoff,
    bars: &PreparedBarsHandoff,
) -> Result<Vec<PreparedCompletionSystem>, PreparedCompletionOwnershipError> {
    let systems = bars
        .systems
        .iter()
        .map(|system| PreparedCompletionSystem {
            system_id: system.system_id,
            staff_ids: system.staff_ids.clone(),
        })
        .collect::<Vec<_>>();
    validate_direct_completion_ownership(prepared, &systems)
}

/// Validate exact ownership emitted directly by a completed raw-bars stage.
/// Input order and identities are returned unchanged.
pub fn validate_direct_completion_ownership(
    prepared: &PreparedStaffHandoff,
    direct: &[PreparedCompletionSystem],
) -> Result<Vec<PreparedCompletionSystem>, PreparedCompletionOwnershipError> {
    let prepared_ids = prepared
        .staffs
        .iter()
        .map(|staff| staff.id)
        .collect::<Vec<_>>();
    let mut known = HashSet::with_capacity(prepared_ids.len());
    for &staff_id in &prepared_ids {
        if !known.insert(staff_id) {
            return Err(PreparedCompletionOwnershipError::DuplicatePreparedStaff(
                staff_id,
            ));
        }
    }

    let mut system_ids = HashSet::with_capacity(direct.len());
    let mut assigned = HashSet::with_capacity(prepared_ids.len());
    let mut flattened = Vec::with_capacity(prepared_ids.len());
    let mut systems = Vec::with_capacity(direct.len());
    for system in direct {
        if system.system_id == 0 {
            return Err(PreparedCompletionOwnershipError::InvalidSystemId(0));
        }
        if !system_ids.insert(system.system_id) {
            return Err(PreparedCompletionOwnershipError::DuplicateSystem(
                system.system_id,
            ));
        }
        if system.staff_ids.is_empty() {
            return Err(PreparedCompletionOwnershipError::EmptySystem(
                system.system_id,
            ));
        }
        for &staff_id in &system.staff_ids {
            if !assigned.insert(staff_id) {
                return Err(PreparedCompletionOwnershipError::StaffInMultipleSystems(
                    staff_id,
                ));
            }
            if !known.contains(&staff_id) {
                return Err(PreparedCompletionOwnershipError::UnknownSystemStaff {
                    system_id: system.system_id,
                    staff_id,
                });
            }
            flattened.push(staff_id);
        }
        systems.push(PreparedCompletionSystem {
            system_id: system.system_id,
            staff_ids: system.staff_ids.clone(),
        });
    }

    if let Some(&staff_id) = prepared_ids
        .iter()
        .find(|staff_id| !assigned.contains(staff_id))
    {
        return Err(PreparedCompletionOwnershipError::UnownedStaff(staff_id));
    }
    if let Some((position, (&expected, &actual))) = prepared_ids
        .iter()
        .zip(&flattened)
        .enumerate()
        .find(|(_, (expected, actual))| expected != actual)
    {
        return Err(PreparedCompletionOwnershipError::StaffOrderMismatch {
            position,
            expected,
            actual,
        });
    }
    Ok(systems)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        filament::StaffFilament,
        grid_lifecycle::{GridBuildOutcome, build_grid_info},
        line_short_sections::HorizontalSectionLag,
        lines_coordinator::StaffCandidateKind,
        peak_graph::PeakGraph,
        prepared_bars::PreparedBarsSystem,
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
        bars_handoff: Option<PreparedBarsHandoff>,
        direct_ownership: Option<Vec<PreparedCompletionSystem>>,
        process_bars_error: Option<&'static str>,
        finish_count: usize,
    }

    impl PreparedBarsStage for Upstream {
        fn take_prepared_bars_handoff(&mut self) -> Option<PreparedBarsHandoff> {
            self.bars_handoff.take()
        }
    }

    impl PreparedCompletionOwnershipStage for Upstream {
        fn take_prepared_completion_ownership(&mut self) -> Option<Vec<PreparedCompletionSystem>> {
            self.direct_ownership.take()
        }
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
            match self.process_bars_error {
                Some(error) => Err(GridStageFailure::Other(error)),
                None => Ok(()),
            }
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
                tentative: false,
                lines: vec![PreparedStaffLine {
                    id: 7,
                    cluster_position: 0,
                    filament,
                }],
            }],
        }
    }

    fn prepared_staffs_with_ids(ids: &[usize]) -> PreparedStaffHandoff {
        let template = prepared_staffs().staffs.remove(0);
        PreparedStaffHandoff {
            staffs: ids
                .iter()
                .map(|&id| {
                    let mut staff = template.clone();
                    staff.id = id;
                    staff
                })
                .collect(),
        }
    }

    fn prepared_bars(systems: &[(usize, &[usize])]) -> PreparedBarsHandoff {
        PreparedBarsHandoff {
            systems: systems
                .iter()
                .map(|&(system_id, staff_ids)| PreparedBarsSystem {
                    system_id,
                    staff_ids: staff_ids.to_vec(),
                    staff_peaks: vec![Vec::new(); staff_ids.len()],
                    // These fixtures exercise ownership validation, not staff
                    // geometry; leaving the limits empty keeps the prepared
                    // staffs' own abscissae in place.
                    staff_limits: Vec::new(),
                    brace_peaks: vec![None; staff_ids.len()],
                    vertical_plans: Vec::new(),
                    maximum_group_gap: 3,
                    interline: 10.0,
                })
                .collect(),
            peak_graph: PeakGraph::new(),
            connections: Vec::new(),
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

    fn automatic_builder(
        staff_ids: &[usize],
        bars_handoff: PreparedBarsHandoff,
        explicit: Option<Vec<PreparedCompletionSystem>>,
        process_bars_error: Option<&'static str>,
    ) -> HeadlessRasterGridBuilder<ProductionCompleteLines<Upstream, Completion>> {
        let upstream = Upstream {
            handoff: Some(prepared_staffs_with_ids(staff_ids)),
            bars_handoff: Some(bars_handoff),
            process_bars_error,
            ..Upstream::default()
        };
        let mut stages =
            ProductionCompleteLines::new(upstream, Completion::default(), Some(source()), 20, true);
        if let Some(explicit) = explicit {
            stages = stages.with_completion_systems(explicit);
        }
        HeadlessRasterGridBuilder::new(
            source(),
            raster_parameters(),
            stages.with_completion_systems_from_prepared_bars(),
        )
    }

    fn direct_builder(
        staff_ids: &[usize],
        ownership: Option<Vec<PreparedCompletionSystem>>,
        process_bars_error: Option<&'static str>,
    ) -> HeadlessRasterGridBuilder<ProductionCompleteLines<Upstream, Completion>> {
        let upstream = Upstream {
            handoff: Some(prepared_staffs_with_ids(staff_ids)),
            direct_ownership: ownership,
            process_bars_error,
            ..Upstream::default()
        };
        HeadlessRasterGridBuilder::new(
            source(),
            raster_parameters(),
            ProductionCompleteLines::new(upstream, Completion::default(), Some(source()), 20, true)
                .with_completion_systems_from_raw_bars(),
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
    fn automatic_ownership_preserves_stable_system_and_staff_order() {
        let expected = vec![
            PreparedCompletionSystem {
                system_id: 4,
                staff_ids: vec![7, 9],
            },
            PreparedCompletionSystem {
                system_id: 8,
                staff_ids: vec![12],
            },
        ];
        let mut builder = automatic_builder(
            &[7, 9, 12],
            prepared_bars(&[(4, &[7, 9]), (8, &[12])]),
            None,
            None,
        );

        assert_eq!(
            build_grid_info(&mut builder),
            Ok(GridBuildOutcome::Completed)
        );

        let stages = builder.stages();
        assert_eq!(
            stages.state().unwrap().completion_systems,
            Some(expected.clone())
        );
        assert_eq!(stages.completion().finish_count, 1);
        assert_eq!(stages.upstream().finish_count, 1);
        let forwarded = builder
            .stages_mut()
            .take_prepared_bars_handoff()
            .expect("automatic join must preserve the sheet handoff");
        assert_eq!(
            forwarded
                .systems
                .iter()
                .map(|system| (system.system_id, system.staff_ids.clone()))
                .collect::<Vec<_>>(),
            expected
                .iter()
                .map(|system| (system.system_id, system.staff_ids.clone()))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn direct_raw_ownership_runs_all_completion_stages_without_bar_handoff() {
        let expected = vec![PreparedCompletionSystem {
            system_id: 3,
            staff_ids: vec![1],
        }];
        let mut builder = direct_builder(&[1], Some(expected.clone()), None);

        assert_eq!(
            build_grid_info(&mut builder),
            Ok(GridBuildOutcome::Completed)
        );
        let stages = builder.stages();
        assert_eq!(stages.state().unwrap().completion_systems, Some(expected));
        assert_eq!(stages.state().unwrap().completed_stages.len(), 11);
        assert_eq!(stages.completion().finish_count, 1);
        assert!(stages.prepared_bars_handoff.is_none());
    }

    #[test]
    fn direct_raw_ownership_validates_before_completion_and_retains_upstream_failure() {
        let mut missing = direct_builder(&[1], None, None);
        assert!(matches!(
            build_grid_info(&mut missing).unwrap(),
            GridBuildOutcome::Swallowed {
                stage: GridBuildStage::ProcessBars,
                error: RasterGridOtherError::Collaborator(
                    ProductionCompleteLinesError::CompletionOwnership(
                        PreparedCompletionOwnershipError::MissingRawBarsOwnership
                    )
                ),
            }
        ));
        assert!(missing.stages().state().is_none());

        let ownership = vec![PreparedCompletionSystem {
            system_id: 1,
            staff_ids: vec![1],
        }];
        let mut upstream_failure = direct_builder(&[1], Some(ownership.clone()), Some("late bars"));
        assert!(matches!(
            build_grid_info(&mut upstream_failure).unwrap(),
            GridBuildOutcome::Swallowed {
                stage: GridBuildStage::ProcessBars,
                error: RasterGridOtherError::Collaborator(ProductionCompleteLinesError::Upstream(
                    "late bars"
                )),
            }
        ));
        assert_eq!(
            upstream_failure.stages().direct_ownership_handoff.as_ref(),
            Some(&ownership)
        );
        assert!(upstream_failure.stages().state().is_none());
    }

    #[test]
    fn malformed_automatic_ownership_fails_at_process_bars_without_completion_prefix() {
        let cases = [
            (
                prepared_bars(&[]),
                PreparedCompletionOwnershipError::UnownedStaff(1),
            ),
            (
                prepared_bars(&[(1, &[1]), (2, &[1])]),
                PreparedCompletionOwnershipError::StaffInMultipleSystems(1),
            ),
            (
                prepared_bars(&[(1, &[2])]),
                PreparedCompletionOwnershipError::UnknownSystemStaff {
                    system_id: 1,
                    staff_id: 2,
                },
            ),
            (
                prepared_bars(&[(1, &[])]),
                PreparedCompletionOwnershipError::EmptySystem(1),
            ),
        ];

        for (bars, expected) in cases {
            let mut builder = automatic_builder(&[1], bars, None, None);
            let outcome = build_grid_info(&mut builder).unwrap();

            assert_eq!(
                outcome,
                GridBuildOutcome::Swallowed {
                    stage: GridBuildStage::ProcessBars,
                    error: RasterGridOtherError::Collaborator(
                        ProductionCompleteLinesError::CompletionOwnership(expected)
                    ),
                }
            );
            let stages = builder.stages();
            assert!(stages.state().is_none());
            assert_eq!(stages.completion().finish_count, 0);
            assert_eq!(stages.upstream().finish_count, 1);
        }
    }

    #[test]
    fn automatic_ownership_rejects_order_and_explicit_mismatch() {
        let prepared = prepared_staffs_with_ids(&[1, 2]);
        assert_eq!(
            validate_prepared_completion_ownership(&prepared, &prepared_bars(&[(1, &[2, 1])])),
            Err(PreparedCompletionOwnershipError::StaffOrderMismatch {
                position: 0,
                expected: 1,
                actual: 2,
            })
        );

        let explicit = vec![PreparedCompletionSystem {
            system_id: 9,
            staff_ids: vec![1],
        }];
        let automatic = vec![PreparedCompletionSystem {
            system_id: 1,
            staff_ids: vec![1],
        }];
        let mut builder = automatic_builder(
            &[1],
            prepared_bars(&[(1, &[1])]),
            Some(explicit.clone()),
            None,
        );

        assert_eq!(
            build_grid_info(&mut builder).unwrap(),
            GridBuildOutcome::Swallowed {
                stage: GridBuildStage::ProcessBars,
                error: RasterGridOtherError::Collaborator(
                    ProductionCompleteLinesError::CompletionOwnership(
                        PreparedCompletionOwnershipError::ExplicitOwnershipMismatch {
                            explicit,
                            automatic,
                        }
                    )
                ),
            }
        );
        assert!(builder.stages().state().is_none());
        assert_eq!(builder.stages().completion().finish_count, 0);
    }

    #[test]
    fn upstream_process_bars_failure_wins_over_malformed_ownership() {
        let mut builder =
            automatic_builder(&[1], prepared_bars(&[]), None, Some("upstream bars failed"));

        assert_eq!(
            build_grid_info(&mut builder).unwrap(),
            GridBuildOutcome::Swallowed {
                stage: GridBuildStage::ProcessBars,
                error: RasterGridOtherError::Collaborator(ProductionCompleteLinesError::Upstream(
                    "upstream bars failed"
                )),
            }
        );
        let stages = builder.stages();
        assert!(stages.state().is_none());
        assert_eq!(stages.completion().finish_count, 0);
        assert_eq!(stages.upstream().finish_count, 1);
        assert!(builder.stages_mut().take_prepared_bars_handoff().is_some());
    }

    #[test]
    fn production_completion_terminal_rejects_any_unclaimed_stage() {
        let mut tail = ProductionCompletionTail::<()>::default();
        let mut state = PreparedCompletionState {
            staffs: Vec::new(),
            global_slope: None,
            completion_systems: None,
            discarded_filaments: Vec::new(),
            horizontal_sections: Vec::new(),
            binary_buffer: None,
            thick_section_ids: Vec::new(),
            thin_section_ids: Vec::new(),
            defined_endpoints: Vec::new(),
            fill_hole_invocations: Vec::new(),
            discarded_filament_steals: Vec::new(),
            discarded_filament_recomputations: Vec::new(),
            section_inclusion_batches: Vec::new(),
            sticker_section_ids: Vec::new(),
            sticker_inclusion_batches: Vec::new(),
            crossing_line_inspections: Vec::new(),
            curvature_removals: Vec::new(),
            curvature_recomputations: Vec::new(),
            completed_stages: Vec::new(),
        };

        assert_eq!(
            tail.run_stage(LineCompletionStage::FillHolesFinal, &mut state),
            Err(GridStageFailure::Other(UnclaimedLineCompletionStage(
                LineCompletionStage::FillHolesFinal
            )))
        );
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
        assert_eq!(builder.stages().state().unwrap().global_slope, Some(0.0));
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
