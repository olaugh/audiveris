// SPDX-License-Identifier: AGPL-3.0-or-later

//! Raw-raster construction boundary for Java's per-staff `StaffProjector`s.
//!
//! This adapter consumes the prepared staff prefix and the live binary raster,
//! then runs the already ported neutral projection logic. Every retained peak
//! receives the sheet's exact deskew transform before it enters
//! [`BarsProjectorRegistry`]. Detached brace candidates receive the same
//! treatment. The adapter can also materialize the exact ordered peak-graph
//! vertices. A single retained staff has an unambiguous one-system grouping
//! and can therefore reach bar-column construction without any invented
//! alignment edges. Multi-staff grouping stops explicitly at the still-
//! missing Java alignment and connection-discovery collaborators.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use audiveris_image::{
    bar_alignment::{BarAlignment, BarAlignmentKind},
    bar_alignments::{
        AlignmentBuildError, AlignmentBuildReport, AlignmentParameters, AlignmentStaff,
        alignment_for_pair, alignment_pairs_for_peak, find_all_alignments,
    },
    bar_column::{BarColumn, StaffId},
    bar_connections::{
        ConnectionBuildError, ConnectionBuildReport, ConnectionParameters, ConnectionRaster,
        check_connection_edge, find_connections,
    },
    bar_splits::{
        SplitBuildError, SplitBuildReport, SplitParameters, split_merged_groups_and_purge,
    },
    bar_sticks::{
        BarStickBuildState, BarStickError, BarStickParameters, build_bar_sticks, build_sub_stick,
    },
    bars_coordinator::{
        BarsConnectionGroupParameters, BarsConnectionGroupResult, BarsCoordinatorError,
        BarsCoordinatorParameters, BarsPartResult, BarsPostBraceResult, BarsPrefixResult,
        BarsPurgeParameters, BarsPurgeResult, BarsRecordGroupResult, BarsRightCClefParameters,
        BarsRightCClefResult, BarsRightEvidence, BarsRootEvidence, BarsStaffState, BarsSystemState,
        BarsWidthInterParameters, BarsWidthInterResult, contextualize_bars,
        process_bars_after_braces, process_bars_connections_and_groups, process_bars_create_parts,
        process_bars_peak_purges, process_bars_record_and_groups,
        process_bars_right_ends_and_c_clefs, process_bars_through_too_far_left,
        process_bars_widths_and_inters,
    },
    bars_logic::{
        BarsLogicError, BracketEndDetection, build_bar_columns_from_graph, detect_bracket_middles,
    },
    filament::{FilamentError, FilamentGeometry},
    grid_lifecycle::{GridBuildStage, GridStageFailure},
    grid_sig::{BarTailParameters, BarTailResult, GridSig},
    lines_coordinator::StaffCandidateKind,
    peak_graph::PeakGraph,
    prepared_completion::PreparedCompletionOwnershipStage,
    prepared_lines::{
        PreparedStaff, PreparedStaffHandoff, PreparedStaffStage, RawLineMetadataHandoff,
        RawLineMetadataStage,
    },
    projection::{
        BarlineHeightSpec, BarsProjectorRegistry, BraceSearchRequest, PeakCoreGeometry,
        ProjectionError, ProjectorRegistration, StaffProjectorProcessRequest,
        StaffProjectorProcessTuning, StaffProjectorScaleRatios, StaffProjectorScaleRequest,
        barline_height, has_blank_between, process_staff_projection,
    },
    section::Section,
    staff_peak::{PeakBounds, PeakPoint, StaffPeak, StaffPeakError, StaffPeakKey},
};

use audiveris_image::discarded_completion::PreparedCompletionSystem;
use audiveris_image::raster_grid_builder::{RasterGridBuildState, RemainingRasterGridStages};

use crate::grid_executor::HeadlessSkew;
use crate::system_grouping::{
    SystemGroupingError, SystemGroupingParameters, SystemGroupingReport, SystemGroupingStaff,
    group_systems_and_build_columns,
};
use crate::{
    brace_filament::{
        BraceFilamentError, BraceFilamentParameters, BraceFilamentPortion, build_brace_filament,
    },
    brace_portions::{
        BraceColumnSet, BracePortionError, BracePortionParameters, BracePortionReport,
        BracePortionStaff, BracePortionSystem, BraceProbe, apply_brace_replacements_with_peak_ids,
        detect_brace_portions,
    },
    brace_sig::{BracePromotion, BraceSigError, BraceSigStore, promote_brace_filament},
    bracket_serif::{
        BracketSerifError, BracketSerifParameters, BracketSerifStore,
        detect_bracket_ends_from_sections,
    },
};

/// Borrowed live sheet raster. Zero is foreground, as in Audiveris run tables.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawProjectorRaster<'a> {
    pub width: usize,
    pub height: usize,
    pub pixels: &'a [u8],
}

/// Scale-independent settings that differ by prepared staff.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawStaffProjectorSettings {
    pub staff_id: usize,
    pub barline_height: BarlineHeightSpec,
    /// Optional source-resolved brace window. Brace discovery is deliberately
    /// not inferred by this construction adapter.
    pub brace_search: Option<BraceSearchRequest>,
}

/// Explicit sheet-scale and Java-constant inputs for projector construction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawProjectorParameters<'a> {
    pub large_interline: i32,
    pub foreground_thickness: i32,
    pub ratios: StaffProjectorScaleRatios,
    pub tuning: StaffProjectorProcessTuning,
    pub staffs: &'a [RawStaffProjectorSettings],
}

/// One detached brace peak, kept outside normal projector/graph registration.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedBracePeak {
    pub staff_id: StaffId,
    pub peak: StaffPeak,
}

/// Construction result for one prepared staff handoff (normally one sheet).
#[derive(Clone, Debug, PartialEq)]
pub struct RawProjectorPreparation {
    pub registry: BarsProjectorRegistry,
    /// One decision per prepared staff, in prepared-staff order.
    pub registrations: Vec<ProjectorRegistration>,
    /// Detached candidates in prepared-staff order, omitting absent braces.
    pub brace_peaks: Vec<PreparedBracePeak>,
}

/// Deepest honest downstream boundary available from raw projectors alone.
///
/// Java discovers cross-staff alignments and concrete connections before it
/// decides system membership. Those collaborators are not ported yet, so the
/// multi-staff case is represented as pending rather than as fabricated
/// singleton systems.
#[derive(Clone, Debug)]
pub enum RawSystemGroupingBoundary {
    /// With one retained staff, Java's fallback grouping is unambiguous and
    /// isolated graph vertices are valid one-peak bar chains.
    CompleteSingleStaff {
        system_id: usize,
        staff_id: StaffId,
        columns: Vec<BarColumn>,
    },
    /// The graph vertices are complete, but the edges and resulting system
    /// partition require Java's missing alignment/connection discovery.
    NeedsAlignmentAndConnectionDiscovery { staff_ids: Vec<StaffId> },
    /// Raw alignments exist, but Java cannot group systems until pixel-backed
    /// concrete connection promotion has run.
    NeedsConnectionDiscovery {
        staff_ids: Vec<StaffId>,
        alignment_count: usize,
    },
    /// Concrete connections have been promoted, but Java still performs
    /// merged-peak splitting and conflict purge before system inference.
    NeedsSplitAndAlignmentPurge {
        staff_ids: Vec<StaffId>,
        alignment_count: usize,
        connection_count: usize,
    },
    /// Split and alignment conflict resolution are complete; Java next infers
    /// system tops from the surviving concrete connections.
    NeedsSystemGrouping {
        staff_ids: Vec<StaffId>,
        retained_edge_count: usize,
        connection_count: usize,
    },
    /// Java system ownership, cross-system purge, and initial bar columns are
    /// complete for every retained staff.
    CompleteSystems {
        staff_ids: Vec<StaffId>,
        system_count: usize,
    },
    /// Start-column selection and the two following column/left purges are
    /// complete. Java next requires brace-portion section/glyph evidence.
    NeedsBracePortionDetection {
        staff_ids: Vec<StaffId>,
        system_count: usize,
    },
    /// Brace detection, construction, left purge, and one-staff lines-root
    /// verification are complete. Java next detects bracket ends.
    NeedsBracketEndDetection {
        staff_ids: Vec<StaffId>,
        system_count: usize,
    },
    /// Bracket ends and connection-backed middle portions are complete. Java
    /// next purges ordinary peaks left of each staff root.
    NeedsLeftPeakPurge {
        staff_ids: Vec<StaffId>,
        system_count: usize,
    },
    /// Left, isolated, and vertically extending peak purges are complete.
    /// Java next refines every staff's right endpoint.
    NeedsRightEndRefinement {
        staff_ids: Vec<StaffId>,
        system_count: usize,
    },
    /// Staff right endpoints and C-clef false positives are resolved. Java
    /// next partitions surviving peaks into thin and thick classes.
    NeedsWidthPartition {
        staff_ids: Vec<StaffId>,
        system_count: usize,
    },
    /// Width classes and vertical bar/bracket interpretations now exist.
    /// Java next promotes cross-staff connection interpretations.
    NeedsConnectionInters {
        staff_ids: Vec<StaffId>,
        system_count: usize,
    },
    /// Cross-staff connectors and within-staff bar groups are materialized.
    /// Java next records barline inters on their owning staffs.
    NeedsBarRecording {
        staff_ids: Vec<StaffId>,
        system_count: usize,
    },
    /// Staff barline ownership and structural groups are recorded. Java next
    /// creates logical parts from these groups.
    NeedsPartCreation {
        staff_ids: Vec<StaffId>,
        system_count: usize,
    },
    /// Bar retrieval completed through part creation and contextual grading.
    CompleteBars {
        staff_ids: Vec<StaffId>,
        system_count: usize,
    },
}

/// Ordered peak graph plus the system-grouping boundary reached from it.
#[derive(Clone, Debug)]
pub struct RawProjectorGraphBridge {
    pub graph: PeakGraph<BarAlignment>,
    /// Retained projector staff IDs in Java projector order.
    pub retained_staff_ids: Vec<StaffId>,
    /// One-line staves discarded by `findBarPeaks`, in encounter order.
    pub discarded_one_line_staff_ids: Vec<StaffId>,
    /// Brace candidates remain detached from ordinary graph registration.
    pub brace_peaks: Vec<PreparedBracePeak>,
    pub grouping: RawSystemGroupingBoundary,
}

/// Raw projector graph after real section-backed sticks have been built and
/// the weak Java curvature pass has marked brace-like peaks.
#[derive(Clone, Debug)]
pub struct RawBarStickGraphBridge {
    pub projectors: RawProjectorGraphBridge,
    pub sticks: BarStickBuildState,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawAlignmentBridgeParameters {
    pub sticks: BarStickParameters,
    pub maximum_alignment_slope: f64,
    pub maximum_alignment_delta_width: i32,
    pub maximum_column_dx: i32,
}

/// Stick-backed graph after `findAllAlignments`, stopped immediately before
/// Java's raster-backed `findConnections` pass.
#[derive(Clone, Debug)]
pub struct RawAlignmentGraphBridge {
    pub bars: RawBarStickGraphBridge,
    pub alignments: AlignmentBuildReport,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawConnectionBridgeParameters {
    pub alignments: RawAlignmentBridgeParameters,
    pub maximum_connection_gap: i32,
    pub maximum_connection_white_ratio: f64,
}

/// Pixel-backed connection graph, stopped before split/purge semantics.
#[derive(Clone, Debug)]
pub struct RawConnectionGraphBridge {
    pub alignments: RawAlignmentGraphBridge,
    pub connections: ConnectionBuildReport,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawSplitBridgeParameters {
    pub connections: RawConnectionBridgeParameters,
    pub maximum_close_gap: i32,
    pub maximum_width_ratio: f64,
}

/// Raw graph after Java's merged-peak fixed point and alignment purge.
#[derive(Clone, Debug)]
pub struct RawSplitGraphBridge {
    pub connections: RawConnectionGraphBridge,
    /// Exact projector order after any merged-peak replacements.
    pub staffs: Vec<AlignmentStaff>,
    pub splits: SplitBuildReport,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawSystemGroupingParameters {
    pub splits: RawSplitBridgeParameters,
    pub maximum_first_connection_x_offset: i32,
}

#[derive(Clone, Debug)]
pub struct RawSystemGraphBridge {
    pub split: RawSplitGraphBridge,
    pub systems: SystemGroupingReport,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawBarsPrefixParameters {
    pub systems: RawSystemGroupingParameters,
    pub bars: BarsCoordinatorParameters,
}

#[derive(Clone, Debug)]
pub struct RawBarsPrefixSystem {
    pub state: BarsSystemState,
    pub result: BarsPrefixResult,
    pub sig: GridSig,
    pub bar_tail: BarTailResult,
}

#[derive(Clone, Debug)]
pub struct RawBarsPrefixBridge {
    pub systems: RawSystemGraphBridge,
    pub bars: Vec<RawBarsPrefixSystem>,
}

#[derive(Clone, Debug)]
pub struct RawCompletedBarsSystem {
    pub system_id: usize,
    pub staff_ids: Vec<usize>,
    pub sig: GridSig,
    pub bar_tail: BarTailResult,
}

#[derive(Clone, Debug)]
pub struct RawBarsCompletionHandoff {
    systems: Vec<RawCompletedBarsSystem>,
}

impl RawBarsCompletionHandoff {
    pub fn from_completed_systems(
        systems: Vec<RawCompletedBarsSystem>,
    ) -> Result<Self, RawProjectorAdapterError> {
        for system in &systems {
            if !system.bar_tail.contextualized {
                return Err(RawProjectorAdapterError::UncontextualizedBars(
                    system.system_id,
                ));
            }
            if system
                .bar_tail
                .staff_barlines
                .keys()
                .copied()
                .ne(system.staff_ids.iter().copied())
            {
                return Err(RawProjectorAdapterError::CompletedBarsStaffOrder);
            }
        }
        Ok(Self { systems })
    }

    #[must_use]
    pub fn systems(&self) -> &[RawCompletedBarsSystem] {
        &self.systems
    }

    #[must_use]
    pub fn ownership(&self) -> Vec<PreparedCompletionSystem> {
        self.systems
            .iter()
            .map(|system| PreparedCompletionSystem {
                system_id: system.system_id,
                staff_ids: system.staff_ids.clone(),
            })
            .collect()
    }
}

/// Production lifecycle adapter for a completed raw BarsRetriever result.
/// It forwards the final SIG/tail intact while exposing only exact ownership
/// to `ProductionCompleteLines`; no bar plans or tail collaborators replay.
pub struct ProductionRawBarsCompletion<Upstream> {
    upstream: Upstream,
    completed: RawBarsCompletionHandoff,
    ownership_handoff: Option<Vec<PreparedCompletionSystem>>,
}

impl<Upstream> ProductionRawBarsCompletion<Upstream> {
    pub fn new(
        upstream: Upstream,
        bridge: &RawBarsPrefixBridge,
    ) -> Result<Self, RawProjectorAdapterError> {
        Self::from_handoff(upstream, raw_bars_completion_handoff(bridge)?)
    }

    pub fn from_handoff(
        upstream: Upstream,
        completed: RawBarsCompletionHandoff,
    ) -> Result<Self, RawProjectorAdapterError> {
        for system in completed.systems() {
            if !system.bar_tail.contextualized {
                return Err(RawProjectorAdapterError::UncontextualizedBars(
                    system.system_id,
                ));
            }
        }
        Ok(Self {
            upstream,
            completed,
            ownership_handoff: None,
        })
    }

    #[must_use]
    pub const fn upstream(&self) -> &Upstream {
        &self.upstream
    }

    #[must_use]
    pub const fn completed(&self) -> &RawBarsCompletionHandoff {
        &self.completed
    }
}

impl<Upstream> PreparedStaffStage for ProductionRawBarsCompletion<Upstream>
where
    Upstream: PreparedStaffStage,
{
    fn prepared_staff_handoff(&self) -> Option<&PreparedStaffHandoff> {
        self.upstream.prepared_staff_handoff()
    }

    fn take_prepared_staff_handoff(&mut self) -> Option<PreparedStaffHandoff> {
        self.upstream.take_prepared_staff_handoff()
    }
}

impl<Upstream> RawLineMetadataStage for ProductionRawBarsCompletion<Upstream>
where
    Upstream: RawLineMetadataStage,
{
    fn take_raw_line_metadata_handoff(&mut self) -> Option<RawLineMetadataHandoff> {
        self.upstream.take_raw_line_metadata_handoff()
    }
}

impl<Upstream> PreparedCompletionOwnershipStage for ProductionRawBarsCompletion<Upstream> {
    fn take_prepared_completion_ownership(&mut self) -> Option<Vec<PreparedCompletionSystem>> {
        self.ownership_handoff.take()
    }
}

impl<Upstream> RemainingRasterGridStages for ProductionRawBarsCompletion<Upstream>
where
    Upstream: RemainingRasterGridStages,
{
    type StepError = Upstream::StepError;
    type OtherError = Upstream::OtherError;

    fn retrieve_lines(
        &mut self,
        state: &mut RasterGridBuildState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        self.ownership_handoff = None;
        self.upstream.retrieve_lines(state)
    }

    fn process_bars(
        &mut self,
        state: &mut RasterGridBuildState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        // The completed raw result is the ProcessBars mutation prefix and is
        // made available even when a later delegated callback fails.
        self.ownership_handoff = Some(self.completed.ownership());
        self.upstream.process_bars(state)
    }

    fn complete_lines(
        &mut self,
        state: &mut RasterGridBuildState,
    ) -> Result<(), GridStageFailure<Self::StepError, Self::OtherError>> {
        self.upstream.complete_lines(state)
    }

    fn log_swallowed_error(&mut self, stage: GridBuildStage, error: &Self::OtherError) {
        self.upstream.log_swallowed_error(stage, error);
    }

    fn finish(&mut self) {
        self.upstream.finish();
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawBraceStageParameters {
    pub portions: BracePortionParameters,
    pub filament: BraceFilamentParameters,
    pub maximum_alignment_dx: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RawBraceSystemReport {
    pub system_id: usize,
    pub portions: BracePortionReport,
    pub promotions: Vec<BracePromotion>,
    pub post_brace: BarsPostBraceResult,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RawBraceStageReport {
    pub systems: Vec<RawBraceSystemReport>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawBracketStageParameters {
    pub serif: BracketSerifParameters,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawBracketSystemReport {
    pub system_id: usize,
    pub ends: Vec<BracketEndDetection>,
    pub middles: Vec<StaffPeakKey>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RawBracketStageReport {
    pub systems: Vec<RawBracketSystemReport>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawPeakPurgeStageParameters {
    pub purges: BarsPurgeParameters,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawPeakPurgeSystemReport {
    pub system_id: usize,
    pub purges: BarsPurgeResult,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RawPeakPurgeStageReport {
    pub systems: Vec<RawPeakPurgeSystemReport>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawRightCClefStageParameters {
    pub bars: BarsRightCClefParameters,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawRightCClefSystemReport {
    pub system_id: usize,
    pub result: BarsRightCClefResult,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RawRightCClefStageReport {
    pub systems: Vec<RawRightCClefSystemReport>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawWidthInterStageParameters {
    pub bars: BarsWidthInterParameters,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RawWidthInterSystemReport {
    pub system_id: usize,
    pub result: BarsWidthInterResult,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RawWidthInterStageReport {
    pub systems: Vec<RawWidthInterSystemReport>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawConnectionGroupStageParameters {
    pub bars: BarsConnectionGroupParameters,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RawConnectionGroupSystemReport {
    pub system_id: usize,
    pub result: BarsConnectionGroupResult,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RawConnectionGroupStageReport {
    pub systems: Vec<RawConnectionGroupSystemReport>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawRecordGroupSystemReport {
    pub system_id: usize,
    pub result: BarsRecordGroupResult,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RawRecordGroupStageReport {
    pub systems: Vec<RawRecordGroupSystemReport>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RawPartContextStageParameters {
    pub bars: BarTailParameters,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawPartContextSystemReport {
    pub system_id: usize,
    pub result: BarsPartResult,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RawPartContextStageReport {
    pub systems: Vec<RawPartContextSystemReport>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RawProjectorAdapterError {
    InvalidRasterShape,
    BarsNotComplete,
    CompletedBarsSystemCount {
        expected: usize,
        actual: usize,
    },
    CompletedBarsStaffOrder,
    UncontextualizedBars(usize),
    DuplicateStaffSettings(usize),
    UnknownStaffSettings(usize),
    MissingStaffSettings(usize),
    MissingPreparedStaff(usize),
    EmptyStaffLines(usize),
    StaffLineCountOverflow(usize),
    StaffInterlineOverflow {
        staff_id: usize,
        interline: usize,
    },
    Filament {
        staff_id: usize,
        source: FilamentError,
    },
    Projection {
        staff_id: usize,
        source: ProjectionError,
    },
    Deskew {
        staff_id: usize,
        source: StaffPeakError,
    },
    MissingRegisteredPeak(StaffPeakKey),
    DuplicateRegisteredPeak(StaffPeakKey),
    BarSticks(BarStickError),
    Alignments(AlignmentBuildError),
    Connections(ConnectionBuildError),
    Splits(SplitBuildError),
    Systems(SystemGroupingError),
    Bars(BarsCoordinatorError),
    BarColumns(BarsLogicError),
    BracePortions(BracePortionError),
    BraceFilament(BraceFilamentError),
    BraceSig(BraceSigError),
    BraceMembers(String),
    BracketSerif(BracketSerifError),
    BracketMiddles(BarsLogicError),
    FilamentBoundsOverflow(StaffPeakKey),
}

impl fmt::Display for RawProjectorAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRasterShape => formatter.write_str("invalid raw raster shape"),
            Self::BarsNotComplete => formatter.write_str("raw bars are not complete"),
            Self::CompletedBarsSystemCount { expected, actual } => write!(
                formatter,
                "completed raw bars contain {actual} systems, expected {expected}"
            ),
            Self::CompletedBarsStaffOrder => {
                formatter.write_str("completed raw bar staff ownership order does not match")
            }
            Self::UncontextualizedBars(system) => {
                write!(formatter, "raw bar system {system} is not contextualized")
            }
            Self::DuplicateStaffSettings(id) => {
                write!(formatter, "staff {id} has duplicate projector settings")
            }
            Self::UnknownStaffSettings(id) => {
                write!(formatter, "projector settings refer to unknown staff {id}")
            }
            Self::MissingStaffSettings(id) => {
                write!(formatter, "staff {id} has no projector settings")
            }
            Self::MissingPreparedStaff(id) => {
                write!(
                    formatter,
                    "retained projector staff {id} has no prepared staff"
                )
            }
            Self::EmptyStaffLines(id) => write!(formatter, "staff {id} has no prepared lines"),
            Self::StaffLineCountOverflow(id) => {
                write!(formatter, "staff {id} line count does not fit Java int")
            }
            Self::StaffInterlineOverflow {
                staff_id,
                interline,
            } => write!(
                formatter,
                "staff {staff_id} interline {interline} does not fit Java int"
            ),
            Self::Filament { staff_id, source } => {
                write!(
                    formatter,
                    "staff {staff_id} filament geometry failed: {source}"
                )
            }
            Self::Projection { staff_id, source } => {
                write!(formatter, "staff {staff_id} projection failed: {source}")
            }
            Self::Deskew { staff_id, source } => {
                write!(formatter, "staff {staff_id} peak deskew failed: {source}")
            }
            Self::MissingRegisteredPeak(key) => write!(
                formatter,
                "registered peak on staff {} at {}-{} is absent from retained projectors",
                key.staff_id().value(),
                key.start(),
                key.stop()
            ),
            Self::DuplicateRegisteredPeak(key) => write!(
                formatter,
                "registered peak on staff {} at {}-{} occurs twice in graph order",
                key.staff_id().value(),
                key.start(),
                key.stop()
            ),
            Self::BarSticks(source) => write!(formatter, "bar-stick construction failed: {source}"),
            Self::Alignments(source) => {
                write!(formatter, "bar-alignment discovery failed: {source}")
            }
            Self::Connections(source) => {
                write!(formatter, "bar-connection discovery failed: {source}")
            }
            Self::Splits(source) => write!(formatter, "merged bar-peak splitting failed: {source}"),
            Self::Systems(source) => write!(formatter, "system grouping failed: {source}"),
            Self::Bars(source) => write!(formatter, "bars prefix failed: {source}"),
            Self::BarColumns(source) => {
                write!(formatter, "bar-column construction failed: {source}")
            }
            Self::BracePortions(source) => write!(formatter, "brace portions failed: {source}"),
            Self::BraceFilament(source) => write!(formatter, "brace filament failed: {source}"),
            Self::BraceSig(source) => write!(formatter, "brace SIG promotion failed: {source}"),
            Self::BraceMembers(source) => write!(formatter, "brace members failed: {source}"),
            Self::BracketSerif(source) => write!(formatter, "bracket serif failed: {source}"),
            Self::BracketMiddles(source) => {
                write!(formatter, "bracket middle detection failed: {source}")
            }
            Self::FilamentBoundsOverflow(peak) => {
                write!(formatter, "bar filament bounds overflow for {peak:?}")
            }
        }
    }
}

impl Error for RawProjectorAdapterError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Filament { source, .. } => Some(source),
            Self::Projection { source, .. } => Some(source),
            Self::Deskew { source, .. } => Some(source),
            Self::BarSticks(source) => Some(source),
            Self::Alignments(source) => Some(source),
            Self::Connections(source) => Some(source),
            Self::Splits(source) => Some(source),
            Self::Systems(source) => Some(source),
            Self::Bars(source) => Some(source),
            Self::BarColumns(source) => Some(source),
            Self::BracePortions(source) => Some(source),
            Self::BraceFilament(source) => Some(source),
            Self::BraceSig(source) => Some(source),
            Self::BracketSerif(source) => Some(source),
            Self::BracketMiddles(source) => Some(source),
            _ => None,
        }
    }
}

/// Materialize Java's ordered `PeakGraph` vertices and proceed only as far as
/// the available evidence permits.
///
/// No alignment edge is synthesized here. A multi-staff result deliberately
/// stops before system grouping because `findAllAlignments`, `findConnections`,
/// and their pixel-evidence checks are not ported yet.
pub fn bridge_raw_projectors_to_graph(
    preparation: &RawProjectorPreparation,
    maximum_column_dx: i32,
) -> Result<RawProjectorGraphBridge, RawProjectorAdapterError> {
    let mut graph = PeakGraph::new();
    for &key in preparation.registry.graph_vertex_order() {
        let peak = preparation
            .registry
            .projectors()
            .iter()
            .flat_map(|projector| &projector.result.peaks)
            .find(|peak| peak.key() == key)
            .cloned()
            .ok_or(RawProjectorAdapterError::MissingRegisteredPeak(key))?;
        if !graph.add_vertex(peak) {
            return Err(RawProjectorAdapterError::DuplicateRegisteredPeak(key));
        }
    }

    let retained_staff_ids = preparation
        .registry
        .projectors()
        .iter()
        .map(|projector| projector.staff_id)
        .collect::<Vec<_>>();
    let grouping = group_raw_projector_graph(&graph, &retained_staff_ids, maximum_column_dx)?;

    Ok(RawProjectorGraphBridge {
        graph,
        retained_staff_ids,
        discarded_one_line_staff_ids: preparation.registry.discarded_one_line_staves().to_vec(),
        brace_peaks: preparation.brace_peaks.clone(),
        grouping,
    })
}

/// Continue the raw graph through Java's section-backed `buildBarSticks` and
/// weak curvature pass, without creating any alignment or connection edge.
pub fn bridge_raw_projectors_through_bar_sticks(
    preparation: &RawProjectorPreparation,
    vertical_sections: &[Section],
    horizontal_sections: &[Section],
    stick_parameters: BarStickParameters,
    maximum_column_dx: i32,
) -> Result<RawBarStickGraphBridge, RawProjectorAdapterError> {
    let peak_order = preparation.registry.graph_vertex_order().to_vec();
    let mut projectors = bridge_raw_projectors_to_graph(preparation, maximum_column_dx)?;
    let mut sticks = BarStickBuildState::new(stick_parameters.first_filament_id)
        .map_err(RawProjectorAdapterError::BarSticks)?;
    build_bar_sticks(
        &mut projectors.graph,
        &peak_order,
        vertical_sections,
        horizontal_sections,
        stick_parameters,
        &mut sticks,
    )
    .map_err(RawProjectorAdapterError::BarSticks)?;

    projectors.grouping = group_raw_projector_graph(
        &projectors.graph,
        &projectors.retained_staff_ids,
        maximum_column_dx,
    )?;
    Ok(RawBarStickGraphBridge { projectors, sticks })
}

/// Continue through Java `findAllAlignments` and stop before any raster-backed
/// connection promotion or system inference.
pub fn bridge_raw_projectors_through_alignments(
    handoff: &PreparedStaffHandoff,
    preparation: &RawProjectorPreparation,
    vertical_sections: &[Section],
    horizontal_sections: &[Section],
    skew: &HeadlessSkew,
    parameters: RawAlignmentBridgeParameters,
) -> Result<RawAlignmentGraphBridge, RawProjectorAdapterError> {
    let mut bars = bridge_raw_projectors_through_bar_sticks(
        preparation,
        vertical_sections,
        horizontal_sections,
        parameters.sticks,
        parameters.maximum_column_dx,
    )?;
    let staffs = alignment_staffs(handoff, &bars.projectors)?;
    let mut alignments = AlignmentBuildReport::default();
    find_all_alignments(
        &mut bars.projectors.graph,
        &staffs,
        AlignmentParameters {
            sheet_slope: skew.slope,
            maximum_alignment_slope: parameters.maximum_alignment_slope,
            maximum_alignment_delta_width: parameters.maximum_alignment_delta_width,
        },
        &mut alignments,
    )
    .map_err(RawProjectorAdapterError::Alignments)?;

    if bars.projectors.retained_staff_ids.len() > 1 {
        bars.projectors.grouping = RawSystemGroupingBoundary::NeedsConnectionDiscovery {
            staff_ids: bars.projectors.retained_staff_ids.clone(),
            alignment_count: alignments.edge_ids().len(),
        };
    }
    Ok(RawAlignmentGraphBridge { bars, alignments })
}

/// Continue through Java `findConnections`, preserving stick/section
/// ownership evidence, and stop before merged-peak splitting or edge purge.
pub fn bridge_raw_projectors_through_connections(
    handoff: &PreparedStaffHandoff,
    preparation: &RawProjectorPreparation,
    raster: RawProjectorRaster<'_>,
    vertical_sections: &[Section],
    horizontal_sections: &[Section],
    skew: &HeadlessSkew,
    parameters: RawConnectionBridgeParameters,
) -> Result<RawConnectionGraphBridge, RawProjectorAdapterError> {
    let mut alignments = bridge_raw_projectors_through_alignments(
        handoff,
        preparation,
        vertical_sections,
        horizontal_sections,
        skew,
        parameters.alignments,
    )?;
    let mut connections = ConnectionBuildReport::default();
    find_connections(
        &mut alignments.bars.projectors.graph,
        ConnectionRaster {
            width: raster.width,
            height: raster.height,
            pixels: raster.pixels,
        },
        alignments.bars.sticks.sticks(),
        ConnectionParameters {
            maximum_gap: parameters.maximum_connection_gap,
            maximum_white_ratio: parameters.maximum_connection_white_ratio,
        },
        &mut connections,
    )
    .map_err(RawProjectorAdapterError::Connections)?;

    if alignments.bars.projectors.retained_staff_ids.len() > 1 {
        alignments.bars.projectors.grouping =
            RawSystemGroupingBoundary::NeedsSplitAndAlignmentPurge {
                staff_ids: alignments.bars.projectors.retained_staff_ids.clone(),
                alignment_count: alignments.alignments.edge_ids().len(),
                connection_count: connections.promoted_count(),
            };
    }
    Ok(RawConnectionGraphBridge {
        alignments,
        connections,
    })
}

/// Continue through Java `splitMergedGroups` and `purgeAlignments` with the
/// original section/raster evidence. System creation remains the next explicit
/// boundary.
#[allow(clippy::too_many_arguments)]
pub fn bridge_raw_projectors_through_splits(
    handoff: &PreparedStaffHandoff,
    preparation: &RawProjectorPreparation,
    raster: RawProjectorRaster<'_>,
    vertical_sections: &[Section],
    horizontal_sections: &[Section],
    skew: &HeadlessSkew,
    parameters: RawSplitBridgeParameters,
) -> Result<RawSplitGraphBridge, RawProjectorAdapterError> {
    let mut connections = bridge_raw_projectors_through_connections(
        handoff,
        preparation,
        raster,
        vertical_sections,
        horizontal_sections,
        skew,
        parameters.connections,
    )?;
    let mut staffs = alignment_staffs(handoff, &connections.alignments.bars.projectors)?;
    let raster = ConnectionRaster {
        width: raster.width,
        height: raster.height,
        pixels: raster.pixels,
    };
    let alignment_parameters = AlignmentParameters {
        sheet_slope: skew.slope,
        maximum_alignment_slope: parameters.connections.alignments.maximum_alignment_slope,
        maximum_alignment_delta_width: parameters
            .connections
            .alignments
            .maximum_alignment_delta_width,
    };
    let connection_parameters = ConnectionParameters {
        maximum_gap: parameters.connections.maximum_connection_gap,
        maximum_white_ratio: parameters.connections.maximum_connection_white_ratio,
    };
    // Both callbacks visit this registry sequentially. Interior mutability
    // makes that source ordering explicit without cloning registered sticks.
    let mut splits = SplitBuildReport::default();
    {
        let sticks = RefCell::new(&mut connections.alignments.bars.sticks);
        let connection_report = RefCell::new(&mut connections.connections);
        let graph = &mut connections.alignments.bars.projectors.graph;
        split_merged_groups_and_purge(
            graph,
            &mut staffs,
            SplitParameters {
                maximum_close_gap: parameters.maximum_close_gap,
                maximum_width_ratio: parameters.maximum_width_ratio,
            },
            |old, replacement| {
                let mut state = sticks.borrow_mut();
                let members = state
                    .sticks()
                    .iter()
                    .find(|stick| stick.peak == old)
                    .map(|stick| stick.members.clone())
                    .ok_or_else(|| format!("old peak has no registered filament: {old:?}"))?;
                let stick_parameters = BarStickParameters {
                    first_filament_id: state.next_filament_id(),
                    ..parameters.connections.alignments.sticks
                };
                build_sub_stick(
                    replacement,
                    &members,
                    vertical_sections,
                    horizontal_sections,
                    stick_parameters,
                    &mut state,
                )
                .map(|stick| stick.is_some())
                .map_err(|error| error.to_string())
            },
            |graph, staffs, replacement| {
                for (top, bottom) in alignment_pairs_for_peak(staffs, replacement)
                    .map_err(|error| error.to_string())?
                {
                    let Some(alignment) =
                        alignment_for_pair(graph, top, bottom, alignment_parameters)
                            .map_err(|error| error.to_string())?
                    else {
                        continue;
                    };
                    let edge = graph
                        .add_edge(top, bottom, alignment)
                        .map_err(|error| error.to_string())?;
                    let state = sticks.borrow();
                    let decision = check_connection_edge(
                        graph,
                        edge,
                        raster,
                        state.sticks(),
                        connection_parameters,
                    )
                    .map_err(|error| error.to_string())?;
                    connection_report.borrow_mut().record(decision);
                }
                Ok(())
            },
            |point| skew.deskewed(point),
            &mut splits,
        )
        .map_err(RawProjectorAdapterError::Splits)?;
    }

    if connections
        .alignments
        .bars
        .projectors
        .retained_staff_ids
        .len()
        > 1
    {
        let graph = &connections.alignments.bars.projectors.graph;
        connections.alignments.bars.projectors.grouping =
            RawSystemGroupingBoundary::NeedsSystemGrouping {
                staff_ids: connections
                    .alignments
                    .bars
                    .projectors
                    .retained_staff_ids
                    .clone(),
                retained_edge_count: graph.edges().len(),
                connection_count: graph
                    .edges()
                    .iter()
                    .filter(|edge| edge.relation().kind() == BarAlignmentKind::Connection)
                    .count(),
            };
    }
    Ok(RawSplitGraphBridge {
        connections,
        staffs,
        splits,
    })
}

/// Continue through Java system-top inference, `createSystems`, cross-system
/// alignment purge, and the immediately following initial column build.
#[allow(clippy::too_many_arguments)]
pub fn bridge_raw_projectors_through_systems(
    handoff: &PreparedStaffHandoff,
    preparation: &RawProjectorPreparation,
    raster: RawProjectorRaster<'_>,
    vertical_sections: &[Section],
    horizontal_sections: &[Section],
    skew: &HeadlessSkew,
    parameters: RawSystemGroupingParameters,
) -> Result<RawSystemGraphBridge, RawProjectorAdapterError> {
    let mut split = bridge_raw_projectors_through_splits(
        handoff,
        preparation,
        raster,
        vertical_sections,
        horizontal_sections,
        skew,
        parameters.splits,
    )?;
    let staffs = split
        .staffs
        .iter()
        .map(|geometry| {
            let source = handoff
                .staffs
                .iter()
                .find(|staff| staff.id == geometry.staff_id.value())
                .ok_or(RawProjectorAdapterError::MissingPreparedStaff(
                    geometry.staff_id.value(),
                ))?;
            Ok(SystemGroupingStaff {
                geometry: geometry.clone(),
                kind: source.kind,
            })
        })
        .collect::<Result<Vec<_>, RawProjectorAdapterError>>()?;
    let mut systems = SystemGroupingReport::default();
    group_systems_and_build_columns(
        &mut split.connections.alignments.bars.projectors.graph,
        &staffs,
        SystemGroupingParameters {
            maximum_first_connection_x_offset: parameters.maximum_first_connection_x_offset,
            maximum_column_dx: parameters.splits.connections.alignments.maximum_column_dx,
        },
        &mut systems,
    )
    .map_err(RawProjectorAdapterError::Systems)?;
    split.connections.alignments.bars.projectors.grouping =
        RawSystemGroupingBoundary::CompleteSystems {
            staff_ids: split
                .connections
                .alignments
                .bars
                .projectors
                .retained_staff_ids
                .clone(),
            system_count: systems.systems.len(),
        };
    Ok(RawSystemGraphBridge { split, systems })
}

/// Continue through `detectStartColumns`, `purgePartialColumns`, and
/// `purgeTooLeft`, then stop before Java's section/glyph-backed brace pass.
#[allow(clippy::too_many_arguments)]
pub fn bridge_raw_projectors_through_bars_prefix(
    handoff: &PreparedStaffHandoff,
    preparation: &RawProjectorPreparation,
    raster: RawProjectorRaster<'_>,
    vertical_sections: &[Section],
    horizontal_sections: &[Section],
    skew: &HeadlessSkew,
    parameters: RawBarsPrefixParameters,
) -> Result<RawBarsPrefixBridge, RawProjectorAdapterError> {
    let mut systems = bridge_raw_projectors_through_systems(
        handoff,
        preparation,
        raster,
        vertical_sections,
        horizontal_sections,
        skew,
        parameters.systems,
    )?;
    let source_graph = &systems.split.connections.alignments.bars.projectors.graph;
    let mut bars = Vec::with_capacity(systems.systems.systems.len());
    for system in &systems.systems.systems {
        let graph = induced_system_graph(source_graph, &system.staff_ids)?;
        let mut states = Vec::with_capacity(system.staff_ids.len());
        for &staff_id in &system.staff_ids {
            let geometry = systems
                .split
                .staffs
                .iter()
                .find(|staff| staff.staff_id == staff_id)
                .expect("system staff originates in retained split staff order");
            let prepared = handoff
                .staffs
                .iter()
                .find(|staff| staff.id == staff_id.value())
                .ok_or(RawProjectorAdapterError::MissingPreparedStaff(
                    staff_id.value(),
                ))?;
            let projector = preparation
                .registry
                .projectors()
                .iter()
                .find(|projector| projector.staff_id == staff_id)
                .ok_or(RawProjectorAdapterError::MissingPreparedStaff(
                    staff_id.value(),
                ))?;
            let peaks = geometry
                .peaks
                .iter()
                .filter_map(|&key| graph.vertex(key).cloned())
                .collect::<Vec<_>>();
            let staff_left = prepared.left.round_ties_even() as i32;
            let blanks = peaks
                .iter()
                .map(|peak| {
                    (
                        peak.key(),
                        has_blank_between(
                            &projector.result.all_blanks,
                            peak.stop(),
                            staff_left,
                            projector.scale_parameters.minimum_standard_blank_width,
                        ),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            let mut state = BarsStaffState::new(
                staff_id,
                staff_left,
                prepared.kind == StaffCandidateKind::OneLine,
                peaks,
                blanks,
            )
            .map_err(RawProjectorAdapterError::Bars)?
            .with_right(prepared.right.round_ties_even() as i32);
            if let Some(brace) = systems
                .split
                .connections
                .alignments
                .bars
                .projectors
                .brace_peaks
                .iter()
                .find(|brace| brace.staff_id == staff_id)
            {
                state = state
                    .with_brace_peak(brace.peak.clone())
                    .map_err(RawProjectorAdapterError::Bars)?;
            }
            states.push(state);
        }
        let mut state = BarsSystemState::new(system.id, states, graph)
            .map_err(RawProjectorAdapterError::Bars)?;
        let result = process_bars_through_too_far_left(&mut state, parameters.bars)
            .map_err(RawProjectorAdapterError::Bars)?;
        bars.push(RawBarsPrefixSystem {
            state,
            result,
            sig: GridSig::default(),
            bar_tail: BarTailResult::default(),
        });
    }
    systems
        .split
        .connections
        .alignments
        .bars
        .projectors
        .grouping = RawSystemGroupingBoundary::NeedsBracePortionDetection {
        staff_ids: systems
            .split
            .connections
            .alignments
            .bars
            .projectors
            .retained_staff_ids
            .clone(),
        system_count: bars.len(),
    };
    Ok(RawBarsPrefixBridge { systems, bars })
}

/// Continue an existing raw prefix through `detectBracePortions`, replacement,
/// `buildBraces`, `purgeLeftOfBraces`, and `verifyLinesRoot`.
///
/// Probe construction and peak-to-filament member lookup remain explicit
/// production seams because the former allocates a newly registered filament.
/// The bridge and SIG store are caller-owned so any successful Java mutation
/// prefix remains observable when a later system or callback fails.
#[allow(clippy::too_many_arguments)]
pub fn continue_raw_bars_through_lines_root(
    bridge: &mut RawBarsPrefixBridge,
    preparation: &RawProjectorPreparation,
    all_brace_sections: &[Section],
    skew: &HeadlessSkew,
    parameters: RawBraceStageParameters,
    sig_store: &mut BraceSigStore,
    mut probe: impl FnMut(usize, StaffId, i32, i32) -> Result<Option<BraceProbe>, String>,
    mut members_of: impl FnMut(StaffPeakKey) -> Result<Vec<Section>, String>,
) -> Result<RawBraceStageReport, RawProjectorAdapterError> {
    if !parameters.maximum_alignment_dx.is_finite() || parameters.maximum_alignment_dx < 0.0 {
        return Err(RawProjectorAdapterError::BraceMembers(
            "invalid brace-alignment distance".to_owned(),
        ));
    }
    let mut report = RawBraceStageReport::default();
    for raw_system in &mut bridge.bars {
        let system_id = raw_system.state.system_id();
        let mut systems = [brace_portion_snapshot(&raw_system.state)];
        let mut portion_report = BracePortionReport::default();
        let detection = detect_brace_portions(
            &mut systems,
            parameters.portions,
            |staff_id, minimum, maximum| probe(system_id, staff_id, minimum, maximum),
            &mut portion_report,
        );
        reconcile_brace_snapshot(&mut raw_system.state, &systems[0])?;
        detection.map_err(RawProjectorAdapterError::BracePortions)?;

        let mut columns = [BraceColumnSet {
            system_id,
            columns: raw_system.state.columns().to_vec(),
        }];
        let (graph, peak_ids) = raw_system.state.graph_and_peak_ids_mut();
        let replacement = apply_brace_replacements_with_peak_ids(
            graph,
            peak_ids,
            &mut systems,
            &mut columns,
            &portion_report.replacements,
        );
        raw_system
            .state
            .replace_columns(std::mem::take(&mut columns[0].columns));
        reconcile_brace_snapshot(&mut raw_system.state, &systems[0])?;
        replacement.map_err(RawProjectorAdapterError::BracePortions)?;

        let promotions = build_and_promote_system_braces(
            &mut raw_system.state,
            all_brace_sections,
            skew,
            parameters,
            sig_store,
            &mut members_of,
        )?;
        let root_evidence = raw_system
            .state
            .staffs()
            .iter()
            .map(|staff| {
                let projector = preparation
                    .registry
                    .projectors()
                    .iter()
                    .find(|projector| projector.staff_id == staff.staff_id())
                    .ok_or(RawProjectorAdapterError::MissingPreparedStaff(
                        staff.staff_id().value(),
                    ))?;
                Ok(BarsRootEvidence {
                    staff_id: staff.staff_id(),
                    blanks: projector.result.all_blanks.clone(),
                    minimum_small_blank_width: projector.scale_parameters.minimum_small_blank_width,
                    maximum_left_extremum: projector.scale_parameters.maximum_left_extremum,
                })
            })
            .collect::<Result<Vec<_>, RawProjectorAdapterError>>()?;
        let post_brace = process_bars_after_braces(&mut raw_system.state, &root_evidence)
            .map_err(RawProjectorAdapterError::Bars)?;
        report.systems.push(RawBraceSystemReport {
            system_id,
            portions: portion_report,
            promotions,
            post_brace,
        });
    }

    bridge
        .systems
        .split
        .connections
        .alignments
        .bars
        .projectors
        .grouping = RawSystemGroupingBoundary::NeedsBracketEndDetection {
        staff_ids: bridge
            .systems
            .split
            .connections
            .alignments
            .bars
            .projectors
            .retained_staff_ids
            .clone(),
        system_count: bridge.bars.len(),
    };
    Ok(report)
}

/// Continue Java `BarsRetriever.process` through `detectBracketEnds` and then
/// `detectBracketMiddles` using the registered bar-stick members and both live
/// section lags.
///
/// The caller owns the serif store, so component registrations and accepted
/// attachments survive a later staff/system failure. Projector copies are
/// reconciled after every graph traversal, including error returns.
pub fn continue_raw_bars_through_bracket_middles(
    bridge: &mut RawBarsPrefixBridge,
    vertical_sections: &[Section],
    horizontal_sections: &[Section],
    parameters: RawBracketStageParameters,
    serif_store: &mut BracketSerifStore,
) -> Result<RawBracketStageReport, RawProjectorAdapterError> {
    let sticks = bridge
        .systems
        .split
        .connections
        .alignments
        .bars
        .sticks
        .clone();
    let mut report = RawBracketStageReport::default();
    for raw_system in &mut bridge.bars {
        let system_id = raw_system.state.system_id();
        let mut ends = Vec::new();
        let mut middles = Vec::new();
        if raw_system.state.staffs().len() > 1 {
            // detectBracketEnds: systems, staves, then candidate peaks from
            // start-1 right-to-left; top precedes bottom inside the evidence
            // adapter.
            for staff_index in 0..raw_system.state.staffs().len() {
                let staff_id = raw_system.state.staffs()[staff_index].staff_id();
                let brace_peak = raw_system.state.staffs()[staff_index].brace_peak().cloned();
                let mut peaks = raw_system.state.staffs()[staff_index].peaks().to_vec();
                let start_index = peaks.iter().position(|peak| {
                    peak.is_staff_end(audiveris_image::staff_peak::HorizontalSide::Left)
                });
                let detection = detect_bracket_ends_from_sections(
                    &mut peaks,
                    raw_system.state.graph_mut(),
                    start_index,
                    &sticks,
                    vertical_sections,
                    horizontal_sections,
                    parameters.serif,
                    serif_store,
                );
                // Java projector and graph share StaffPeak instances. Mirror
                // every mutation prefix before propagating a neutral error.
                raw_system
                    .state
                    .staff_mut(staff_id)
                    .expect("staff index came from this system")
                    .replace_brace_stage(peaks, brace_peak)
                    .map_err(RawProjectorAdapterError::Bars)?;
                ends.extend(detection.map_err(RawProjectorAdapterError::BracketSerif)?);
            }

            // detectBracketMiddles repeats the staff/peak traversal over the
            // graph after every bracket-end mutation is complete.
            for staff_index in 0..raw_system.state.staffs().len() {
                let keys = raw_system.state.staffs()[staff_index]
                    .peaks()
                    .iter()
                    .map(StaffPeak::key)
                    .collect::<Vec<_>>();
                let detection = detect_bracket_middles(raw_system.state.graph_mut(), &keys);
                reconcile_graph_peak_attributes(&mut raw_system.state)?;
                middles.extend(detection.map_err(RawProjectorAdapterError::BracketMiddles)?);
            }
        }
        report.systems.push(RawBracketSystemReport {
            system_id,
            ends,
            middles,
        });
    }

    bridge
        .systems
        .split
        .connections
        .alignments
        .bars
        .projectors
        .grouping = RawSystemGroupingBoundary::NeedsLeftPeakPurge {
        staff_ids: bridge
            .systems
            .split
            .connections
            .alignments
            .bars
            .projectors
            .retained_staff_ids
            .clone(),
        system_count: bridge.bars.len(),
    };
    Ok(report)
}

/// Continue through Java `purgeLeftPeaks`, `purgeUnalignedBars`, and
/// `purgeExtendingPeaks` using the registered bar-stick bounds.
pub fn continue_raw_bars_through_peak_purges(
    bridge: &mut RawBarsPrefixBridge,
    parameters: RawPeakPurgeStageParameters,
) -> Result<RawPeakPurgeStageReport, RawProjectorAdapterError> {
    let filament_bounds = bridge
        .systems
        .split
        .connections
        .alignments
        .bars
        .sticks
        .sticks()
        .iter()
        .map(|stick| {
            Ok((
                stick.peak,
                PeakBounds {
                    x: i32::try_from(stick.bounds.x).map_err(|_| {
                        RawProjectorAdapterError::FilamentBoundsOverflow(stick.peak)
                    })?,
                    y: i32::try_from(stick.bounds.y).map_err(|_| {
                        RawProjectorAdapterError::FilamentBoundsOverflow(stick.peak)
                    })?,
                    width: i32::try_from(stick.bounds.width).map_err(|_| {
                        RawProjectorAdapterError::FilamentBoundsOverflow(stick.peak)
                    })?,
                    height: i32::try_from(stick.bounds.height).map_err(|_| {
                        RawProjectorAdapterError::FilamentBoundsOverflow(stick.peak)
                    })?,
                },
            ))
        })
        .collect::<Result<Vec<_>, RawProjectorAdapterError>>()?;
    let mut report = RawPeakPurgeStageReport::default();
    for raw_system in &mut bridge.bars {
        let system_id = raw_system.state.system_id();
        let purges =
            process_bars_peak_purges(&mut raw_system.state, &filament_bounds, parameters.purges)
                .map_err(RawProjectorAdapterError::Bars)?;
        report
            .systems
            .push(RawPeakPurgeSystemReport { system_id, purges });
    }
    bridge
        .systems
        .split
        .connections
        .alignments
        .bars
        .projectors
        .grouping = RawSystemGroupingBoundary::NeedsRightEndRefinement {
        staff_ids: bridge
            .systems
            .split
            .connections
            .alignments
            .bars
            .projectors
            .retained_staff_ids
            .clone(),
        system_count: bridge.bars.len(),
    };
    Ok(report)
}

/// Continue through `StaffProjector.refineRightEnd` for every retained staff,
/// then Java `BarsRetriever.purgeCClefs`.
pub fn continue_raw_bars_through_right_ends_and_c_clefs(
    bridge: &mut RawBarsPrefixBridge,
    preparation: &RawProjectorPreparation,
    sheet_width: usize,
    parameters: RawRightCClefStageParameters,
) -> Result<RawRightCClefStageReport, RawProjectorAdapterError> {
    let sheet_right = sheet_width
        .checked_sub(1)
        .and_then(|value| i32::try_from(value).ok())
        .ok_or(RawProjectorAdapterError::InvalidRasterShape)?;
    let mut report = RawRightCClefStageReport::default();
    for raw_system in &mut bridge.bars {
        let system_id = raw_system.state.system_id();
        let evidence = raw_system
            .state
            .staffs()
            .iter()
            .map(|staff| {
                let projector = preparation
                    .registry
                    .projectors()
                    .iter()
                    .find(|projector| projector.staff_id == staff.staff_id())
                    .ok_or(RawProjectorAdapterError::MissingPreparedStaff(
                        staff.staff_id().value(),
                    ))?;
                Ok(BarsRightEvidence {
                    staff_id: staff.staff_id(),
                    blanks: projector.result.all_blanks.clone(),
                    sheet_right,
                    minimum_small_blank_width: projector.scale_parameters.minimum_small_blank_width,
                    maximum_right_extremum: projector.scale_parameters.maximum_right_extremum,
                })
            })
            .collect::<Result<Vec<_>, RawProjectorAdapterError>>()?;
        let result =
            process_bars_right_ends_and_c_clefs(&mut raw_system.state, &evidence, parameters.bars)
                .map_err(RawProjectorAdapterError::Bars)?;
        report
            .systems
            .push(RawRightCClefSystemReport { system_id, result });
    }
    bridge
        .systems
        .split
        .connections
        .alignments
        .bars
        .projectors
        .grouping = RawSystemGroupingBoundary::NeedsWidthPartition {
        staff_ids: bridge
            .systems
            .split
            .connections
            .alignments
            .bars
            .projectors
            .retained_staff_ids
            .clone(),
        system_count: bridge.bars.len(),
    };
    Ok(report)
}

/// Continue through Java `partitionWidths` and `createInters` for every
/// system, retaining glyph/inter identities and peak backlinks in each
/// system-owned SIG.
pub fn continue_raw_bars_through_widths_and_inters(
    bridge: &mut RawBarsPrefixBridge,
    parameters: RawWidthInterStageParameters,
) -> Result<RawWidthInterStageReport, RawProjectorAdapterError> {
    let mut report = RawWidthInterStageReport::default();
    for raw_system in &mut bridge.bars {
        let system_id = raw_system.state.system_id();
        let result = process_bars_widths_and_inters(
            &mut raw_system.state,
            &mut raw_system.sig,
            parameters.bars,
        )
        .map_err(RawProjectorAdapterError::Bars)?;
        report
            .systems
            .push(RawWidthInterSystemReport { system_id, result });
    }
    bridge
        .systems
        .split
        .connections
        .alignments
        .bars
        .projectors
        .grouping = RawSystemGroupingBoundary::NeedsConnectionInters {
        staff_ids: bridge
            .systems
            .split
            .connections
            .alignments
            .bars
            .projectors
            .retained_staff_ids
            .clone(),
        system_count: bridge.bars.len(),
    };
    Ok(report)
}

/// Continue through Java `createConnectionInters` and `groupBarlines` for
/// every system. Each system SIG retains connector identities, support/freeze
/// effects, warnings, and any mutation prefix from a later grouping failure.
pub fn continue_raw_bars_through_connections_and_groups(
    bridge: &mut RawBarsPrefixBridge,
    parameters: RawConnectionGroupStageParameters,
) -> Result<RawConnectionGroupStageReport, RawProjectorAdapterError> {
    let mut report = RawConnectionGroupStageReport::default();
    for raw_system in &mut bridge.bars {
        let system_id = raw_system.state.system_id();
        let result = process_bars_connections_and_groups(
            &raw_system.state,
            &mut raw_system.sig,
            parameters.bars,
        )
        .map_err(RawProjectorAdapterError::Bars)?;
        report
            .systems
            .push(RawConnectionGroupSystemReport { system_id, result });
    }
    bridge
        .systems
        .split
        .connections
        .alignments
        .bars
        .projectors
        .grouping = RawSystemGroupingBoundary::NeedsBarRecording {
        staff_ids: bridge
            .systems
            .split
            .connections
            .alignments
            .bars
            .projectors
            .retained_staff_ids
            .clone(),
        system_count: bridge.bars.len(),
    };
    Ok(report)
}

/// Continue through Java `recordBars` and `createGroups` for every system,
/// retaining exact empty-staff ownership and any completed record prefix when
/// later structural evidence fails.
pub fn continue_raw_bars_through_recording_and_groups(
    bridge: &mut RawBarsPrefixBridge,
) -> Result<RawRecordGroupStageReport, RawProjectorAdapterError> {
    let mut report = RawRecordGroupStageReport::default();
    for raw_system in &mut bridge.bars {
        let system_id = raw_system.state.system_id();
        let result = process_bars_record_and_groups(
            &raw_system.state,
            &raw_system.sig,
            &mut raw_system.bar_tail,
        )
        .map_err(RawProjectorAdapterError::Bars)?;
        report
            .systems
            .push(RawRecordGroupSystemReport { system_id, result });
    }
    bridge
        .systems
        .split
        .connections
        .alignments
        .bars
        .projectors
        .grouping = RawSystemGroupingBoundary::NeedsPartCreation {
        staff_ids: bridge
            .systems
            .split
            .connections
            .alignments
            .bars
            .projectors
            .retained_staff_ids
            .clone(),
        system_count: bridge.bars.len(),
    };
    Ok(report)
}

/// Finish Java `BarsRetriever.process`: create parts for every system first,
/// then contextualize every system SIG. A later part failure therefore keeps
/// earlier completed parts but contextualizes none, matching the two source
/// traversals.
pub fn finish_raw_bars_through_parts_and_context(
    bridge: &mut RawBarsPrefixBridge,
    parameters: RawPartContextStageParameters,
) -> Result<RawPartContextStageReport, RawProjectorAdapterError> {
    let mut report = RawPartContextStageReport::default();
    for raw_system in &mut bridge.bars {
        let system_id = raw_system.state.system_id();
        let result = process_bars_create_parts(
            &raw_system.state,
            &raw_system.sig,
            &mut raw_system.bar_tail,
            parameters.bars,
        )
        .map_err(RawProjectorAdapterError::Bars)?;
        report
            .systems
            .push(RawPartContextSystemReport { system_id, result });
    }
    for raw_system in &mut bridge.bars {
        contextualize_bars(&mut raw_system.sig, &mut raw_system.bar_tail);
    }
    bridge
        .systems
        .split
        .connections
        .alignments
        .bars
        .projectors
        .grouping = RawSystemGroupingBoundary::CompleteBars {
        staff_ids: bridge
            .systems
            .split
            .connections
            .alignments
            .bars
            .projectors
            .retained_staff_ids
            .clone(),
        system_count: bridge.bars.len(),
    };
    Ok(report)
}

/// Freeze the completed raw bar state for production completion handoff.
pub fn raw_bars_completion_handoff(
    bridge: &RawBarsPrefixBridge,
) -> Result<RawBarsCompletionHandoff, RawProjectorAdapterError> {
    let RawSystemGroupingBoundary::CompleteBars {
        staff_ids,
        system_count,
    } = &bridge
        .systems
        .split
        .connections
        .alignments
        .bars
        .projectors
        .grouping
    else {
        return Err(RawProjectorAdapterError::BarsNotComplete);
    };
    if *system_count != bridge.bars.len() {
        return Err(RawProjectorAdapterError::CompletedBarsSystemCount {
            expected: *system_count,
            actual: bridge.bars.len(),
        });
    }
    let actual_staffs = bridge
        .bars
        .iter()
        .flat_map(|system| system.state.staffs())
        .map(|staff| staff.staff_id())
        .collect::<Vec<_>>();
    if actual_staffs != *staff_ids {
        return Err(RawProjectorAdapterError::CompletedBarsStaffOrder);
    }

    let mut systems = Vec::with_capacity(bridge.bars.len());
    for system in &bridge.bars {
        if !system.bar_tail.contextualized {
            return Err(RawProjectorAdapterError::UncontextualizedBars(
                system.state.system_id(),
            ));
        }
        let state_staff_ids = system
            .state
            .staffs()
            .iter()
            .map(|staff| staff.staff_id().value())
            .collect::<Vec<_>>();
        let tail_staff_ids = system
            .bar_tail
            .staff_barlines
            .keys()
            .copied()
            .collect::<Vec<_>>();
        if tail_staff_ids != state_staff_ids {
            return Err(RawProjectorAdapterError::CompletedBarsStaffOrder);
        }
        systems.push(RawCompletedBarsSystem {
            system_id: system.state.system_id(),
            staff_ids: tail_staff_ids,
            sig: system.sig.clone(),
            bar_tail: system.bar_tail.clone(),
        });
    }
    Ok(RawBarsCompletionHandoff { systems })
}

fn reconcile_graph_peak_attributes(
    state: &mut BarsSystemState,
) -> Result<(), RawProjectorAdapterError> {
    let snapshots = state
        .staffs()
        .iter()
        .map(|staff| {
            let peaks = staff
                .peaks()
                .iter()
                .map(|peak| {
                    state
                        .graph()
                        .vertex(peak.key())
                        .cloned()
                        .ok_or(RawProjectorAdapterError::MissingRegisteredPeak(peak.key()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok((staff.staff_id(), peaks, staff.brace_peak().cloned()))
        })
        .collect::<Result<Vec<_>, RawProjectorAdapterError>>()?;
    for (staff_id, peaks, brace_peak) in snapshots {
        state
            .staff_mut(staff_id)
            .expect("snapshot staff remains in the system")
            .replace_brace_stage(peaks, brace_peak)
            .map_err(RawProjectorAdapterError::Bars)?;
    }
    Ok(())
}

fn brace_portion_snapshot(state: &BarsSystemState) -> BracePortionSystem {
    BracePortionSystem {
        system_id: state.system_id(),
        staffs: state
            .staffs()
            .iter()
            .map(|staff| BracePortionStaff {
                staff_id: staff.staff_id(),
                peaks: staff.peaks().iter().map(StaffPeak::key).collect(),
                start_peak_index: staff.peaks().iter().position(|peak| {
                    peak.is_staff_end(audiveris_image::staff_peak::HorizontalSide::Left)
                }),
                brace_peak: staff.brace_peak().cloned(),
            })
            .collect(),
    }
}

fn reconcile_brace_snapshot(
    state: &mut BarsSystemState,
    snapshot: &BracePortionSystem,
) -> Result<(), RawProjectorAdapterError> {
    for staff in &snapshot.staffs {
        let peaks = staff
            .peaks
            .iter()
            .map(|&key| {
                state
                    .graph()
                    .vertex(key)
                    .cloned()
                    .ok_or(RawProjectorAdapterError::MissingRegisteredPeak(key))
            })
            .collect::<Result<Vec<_>, _>>()?;
        state
            .staff_mut(staff.staff_id)
            .expect("brace snapshot comes from this system")
            .replace_brace_stage(peaks, staff.brace_peak.clone())
            .map_err(RawProjectorAdapterError::Bars)?;
    }
    Ok(())
}

fn build_and_promote_system_braces(
    state: &mut BarsSystemState,
    all_brace_sections: &[Section],
    skew: &HeadlessSkew,
    parameters: RawBraceStageParameters,
    sig_store: &mut BraceSigStore,
    members_of: &mut impl FnMut(StaffPeakKey) -> Result<Vec<Section>, String>,
) -> Result<Vec<BracePromotion>, RawProjectorAdapterError> {
    let mut promotions = Vec::new();
    if state.staffs().len() <= 1 {
        return Ok(promotions);
    }
    let mut staff_index = 0;
    while staff_index < state.staffs().len() {
        let Some(top) = state.staffs()[staff_index].brace_peak().cloned() else {
            staff_index += 1;
            continue;
        };
        if !top.is_set(audiveris_image::staff_peak::StaffPeakAttribute::BraceTop) {
            staff_index += 1;
            continue;
        }

        let mut portions = vec![(state.staffs()[staff_index].staff_id(), top.clone())];
        let mut current_top = top;
        for lower_index in (staff_index + 1)..state.staffs().len() {
            let mut lower = state.staffs()[lower_index].brace_peak().cloned();
            if lower.is_none() {
                let candidate = state.staffs()[lower_index]
                    .peaks()
                    .first()
                    .cloned()
                    .ok_or_else(|| {
                        RawProjectorAdapterError::BraceMembers(format!(
                            "staff {} has no first peak for brace alignment",
                            state.staffs()[lower_index].staff_id().value()
                        ))
                    })?;
                if brace_peaks_align(
                    &current_top,
                    &candidate,
                    parameters.maximum_alignment_dx,
                    skew,
                ) {
                    let mut middle = candidate;
                    middle.set(audiveris_image::staff_peak::StaffPeakAttribute::BraceMiddle);
                    let key = middle.key();
                    *state
                        .graph_mut()
                        .vertex_mut(key)
                        .expect("fallback brace candidate remains in graph") = middle.clone();
                    let staff = state
                        .staff_mut(key.staff_id())
                        .expect("fallback brace candidate belongs to lower staff");
                    *staff
                        .peaks_mut()
                        .first_mut()
                        .expect("fallback candidate is the first peak") = middle.clone();
                    staff
                        .replace_brace_stage(staff.peaks().to_vec(), Some(middle.clone()))
                        .map_err(RawProjectorAdapterError::Bars)?;
                    lower = Some(middle);
                }
            }
            let Some(lower) = lower else {
                break;
            };
            if !lower.is_set(audiveris_image::staff_peak::StaffPeakAttribute::BraceMiddle)
                && !lower.is_set(audiveris_image::staff_peak::StaffPeakAttribute::BraceBottom)
            {
                break;
            }
            portions.push((state.staffs()[lower_index].staff_id(), lower.clone()));
            if lower.is_set(audiveris_image::staff_peak::StaffPeakAttribute::BraceBottom) {
                break;
            }
            current_top = lower;
        }

        if portions.len() > 1 {
            let filament_portions = portions
                .iter()
                .map(|(_, peak)| {
                    Ok(BraceFilamentPortion {
                        peak: peak.clone(),
                        members: members_of(peak.key())
                            .map_err(RawProjectorAdapterError::BraceMembers)?,
                    })
                })
                .collect::<Result<Vec<_>, RawProjectorAdapterError>>()?;
            let filament =
                build_brace_filament(&filament_portions, all_brace_sections, parameters.filament)
                    .map_err(RawProjectorAdapterError::BraceFilament)?;
            promotions.push(
                promote_brace_filament(&filament, state.system_id(), portions[0].0, sig_store)
                    .map_err(RawProjectorAdapterError::BraceSig)?,
            );
        }
        staff_index = state
            .staffs()
            .iter()
            .position(|staff| staff.staff_id() == portions.last().expect("top exists").0)
            .expect("portion staff remains in system")
            + 1;
    }
    Ok(promotions)
}

fn brace_peaks_align(
    top: &StaffPeak,
    bottom: &StaffPeak,
    maximum_dx: f64,
    skew: &HeadlessSkew,
) -> bool {
    let top_mid = top.start().wrapping_add(top.stop()) / 2;
    let bottom_mid = bottom.start().wrapping_add(bottom.stop()) / 2;
    let top = skew.deskewed(PeakPoint::new(f64::from(top_mid), f64::from(top.bottom())));
    let bottom = skew.deskewed(PeakPoint::new(
        f64::from(bottom_mid),
        f64::from(bottom.top()),
    ));
    (bottom.x - top.x).abs() <= maximum_dx
}

fn induced_system_graph(
    source: &PeakGraph<BarAlignment>,
    staff_ids: &[StaffId],
) -> Result<PeakGraph<BarAlignment>, RawProjectorAdapterError> {
    let mut graph = PeakGraph::new();
    for peak in source
        .vertices()
        .iter()
        .filter(|peak| staff_ids.contains(&peak.staff_id()))
    {
        graph.add_vertex(peak.clone());
    }
    for edge in source.edges().iter().filter(|edge| {
        staff_ids.contains(&edge.source().staff_id())
            && staff_ids.contains(&edge.target().staff_id())
    }) {
        graph
            .add_edge(edge.source(), edge.target(), *edge.relation())
            .map_err(BarsCoordinatorError::from)
            .map_err(RawProjectorAdapterError::Bars)?;
    }
    Ok(graph)
}

fn alignment_staffs(
    handoff: &PreparedStaffHandoff,
    bridge: &RawProjectorGraphBridge,
) -> Result<Vec<AlignmentStaff>, RawProjectorAdapterError> {
    bridge
        .retained_staff_ids
        .iter()
        .map(|&staff_id| {
            let staff = handoff
                .staffs
                .iter()
                .find(|staff| staff.id == staff_id.value())
                .ok_or(RawProjectorAdapterError::MissingPreparedStaff(
                    staff_id.value(),
                ))?;
            let first = staff
                .lines
                .first()
                .ok_or(RawProjectorAdapterError::EmptyStaffLines(staff.id))?
                .filament
                .geometry()
                .map_err(|source| RawProjectorAdapterError::Filament {
                    staff_id: staff.id,
                    source,
                })?;
            let last = staff
                .lines
                .last()
                .expect("nonempty staff was checked above")
                .filament
                .geometry()
                .map_err(|source| RawProjectorAdapterError::Filament {
                    staff_id: staff.id,
                    source,
                })?;
            Ok(AlignmentStaff {
                staff_id,
                left: staff.left,
                right: staff.right,
                top: first.start().1,
                bottom: last.start().1,
                short: staff.short,
                peaks: bridge
                    .graph
                    .vertices()
                    .iter()
                    .filter(|peak| peak.staff_id() == staff_id)
                    .map(StaffPeak::key)
                    .collect(),
            })
        })
        .collect()
}

fn group_raw_projector_graph(
    graph: &PeakGraph<BarAlignment>,
    retained_staff_ids: &[StaffId],
    maximum_column_dx: i32,
) -> Result<RawSystemGroupingBoundary, RawProjectorAdapterError> {
    if let [staff_id] = retained_staff_ids {
        let columns = build_bar_columns_from_graph(graph, &[*staff_id], maximum_column_dx)
            .map_err(RawProjectorAdapterError::BarColumns)?;
        Ok(RawSystemGroupingBoundary::CompleteSingleStaff {
            system_id: 1,
            staff_id: *staff_id,
            columns,
        })
    } else {
        Ok(
            RawSystemGroupingBoundary::NeedsAlignmentAndConnectionDiscovery {
                staff_ids: retained_staff_ids.to_vec(),
            },
        )
    }
}

/// Build and register raw per-staff projectors with construction-time deskew.
pub fn prepare_raw_projectors(
    handoff: &PreparedStaffHandoff,
    raster: RawProjectorRaster<'_>,
    skew: &HeadlessSkew,
    parameters: RawProjectorParameters<'_>,
) -> Result<RawProjectorPreparation, RawProjectorAdapterError> {
    let staff_ids = handoff
        .staffs
        .iter()
        .map(|staff| staff.id)
        .collect::<BTreeSet<_>>();
    let mut settings = BTreeMap::new();
    for entry in parameters.staffs {
        if !staff_ids.contains(&entry.staff_id) {
            return Err(RawProjectorAdapterError::UnknownStaffSettings(
                entry.staff_id,
            ));
        }
        if settings.insert(entry.staff_id, *entry).is_some() {
            return Err(RawProjectorAdapterError::DuplicateStaffSettings(
                entry.staff_id,
            ));
        }
    }

    let mut registry = BarsProjectorRegistry::new();
    let mut registrations = Vec::with_capacity(handoff.staffs.len());
    let mut brace_peaks = Vec::new();

    for staff in &handoff.staffs {
        let setting = settings
            .get(&staff.id)
            .copied()
            .ok_or(RawProjectorAdapterError::MissingStaffSettings(staff.id))?;
        let prepared = prepare_staff_projector(staff, raster, skew, parameters, setting)?;
        if let Some(peak) = prepared.brace_peak {
            brace_peaks.push(PreparedBracePeak {
                staff_id: StaffId::new(staff.id),
                peak,
            });
        }
        registrations.push(registry.register(prepared.output, prepared.is_one_line));
    }

    Ok(RawProjectorPreparation {
        registry,
        registrations,
        brace_peaks,
    })
}

struct PreparedProjector {
    output: audiveris_image::projection::StaffProjectorProcessOutput,
    brace_peak: Option<StaffPeak>,
    is_one_line: bool,
}

fn prepare_staff_projector(
    staff: &PreparedStaff,
    raster: RawProjectorRaster<'_>,
    skew: &HeadlessSkew,
    parameters: RawProjectorParameters<'_>,
    setting: RawStaffProjectorSettings,
) -> Result<PreparedProjector, RawProjectorAdapterError> {
    if staff.lines.is_empty() {
        return Err(RawProjectorAdapterError::EmptyStaffLines(staff.id));
    }
    let line_count = i32::try_from(staff.lines.len())
        .map_err(|_| RawProjectorAdapterError::StaffLineCountOverflow(staff.id))?;
    let interline = i32::try_from(staff.interline).map_err(|_| {
        RawProjectorAdapterError::StaffInterlineOverflow {
            staff_id: staff.id,
            interline: staff.interline,
        }
    })?;
    let geometries = staff
        .lines
        .iter()
        .map(|line| line.filament.geometry())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| RawProjectorAdapterError::Filament {
            staff_id: staff.id,
            source,
        })?;
    let thicknesses = staff
        .lines
        .iter()
        .map(|line| line.filament.thickness())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| RawProjectorAdapterError::Filament {
            staff_id: staff.id,
            source,
        })?;
    let ordinates = precompute_ordinates(staff.id, raster.width, &geometries)?;
    let middle = geometries.len() / 2;
    let is_one_line = staff.kind == StaffCandidateKind::OneLine;
    let half_bar_height = barline_height(setting.barline_height, interline) / 2;
    let first = if is_one_line {
        ordinates[middle]
            .iter()
            .map(|y| y.wrapping_sub(half_bar_height))
            .collect::<Vec<_>>()
    } else {
        ordinates[0].clone()
    };
    let last = if is_one_line {
        ordinates[middle]
            .iter()
            .map(|y| y.wrapping_add(half_bar_height))
            .collect::<Vec<_>>()
    } else {
        ordinates[ordinates.len() - 1].clone()
    };
    let staff_id = StaffId::new(staff.id);
    let mut output = process_staff_projection(
        raster.width,
        raster.height,
        raster.pixels,
        StaffProjectorProcessRequest {
            staff_id,
            staff_left: staff.left.round_ties_even() as i32,
            staff_right: staff.right.round_ties_even() as i32,
            line_thicknesses: &thicknesses,
            staff_line_count: line_count,
            foreground_thickness: parameters.foreground_thickness,
            scale: StaffProjectorScaleRequest {
                large_interline: parameters.large_interline,
                staff_specific_interline: interline,
                is_one_line_staff: is_one_line,
                barline_height: setting.barline_height,
                ratios: parameters.ratios,
            },
            tuning: parameters.tuning,
        },
        |x| first[index_x(x, raster.width)],
        |x| last[index_x(x, raster.width)],
        |x| {
            let x = index_x(x, raster.width);
            PeakCoreGeometry::new(
                ordinates[0][x],
                ordinates[middle][x],
                ordinates[line_count as usize - 1][x],
            )
        },
    )
    .map_err(|source| RawProjectorAdapterError::Projection {
        staff_id: staff.id,
        source,
    })?;

    for peak in &mut output.result.peaks {
        peak.compute_deskewed_center(|point| skew.deskewed(point))
            .map_err(|source| RawProjectorAdapterError::Deskew {
                staff_id: staff.id,
                source,
            })?;
    }

    let brace_candidate = setting
        .brace_search
        .map(|request| {
            output
                .result
                .projection
                .find_brace_candidate(&output.result.all_blanks, request)
        })
        .transpose()
        .map_err(|source| RawProjectorAdapterError::Projection {
            staff_id: staff.id,
            source,
        })?
        .flatten();
    output.result.set_brace_candidate(brace_candidate);
    let brace_peak = brace_candidate
        .map(|candidate| {
            candidate.into_staff_peak(
                staff_id,
                |x| {
                    let x = index_x(x, raster.width);
                    (ordinates[0][x], ordinates[ordinates.len() - 1][x])
                },
                |point| skew.deskewed(point),
            )
        })
        .transpose()
        .map_err(|source| RawProjectorAdapterError::Projection {
            staff_id: staff.id,
            source,
        })?;

    Ok(PreparedProjector {
        output,
        brace_peak,
        is_one_line,
    })
}

fn precompute_ordinates(
    staff_id: usize,
    width: usize,
    geometries: &[FilamentGeometry],
) -> Result<Vec<Vec<i32>>, RawProjectorAdapterError> {
    geometries
        .iter()
        .map(|geometry| {
            (0..width)
                .map(|x| {
                    geometry
                        .position_at(x as f64)
                        .map(|y| y.round_ties_even() as i32)
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| RawProjectorAdapterError::Filament { staff_id, source })
}

fn index_x(x: i32, width: usize) -> usize {
    usize::try_from(x)
        .expect("projection only requests nonnegative in-domain abscissae")
        .min(width.saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use audiveris_image::{
        bar_alignment::BarAlignmentKind,
        prepared_lines::PreparedStaffLine,
        run_table::{Orientation, Run, RunTable},
        section::{JunctionPolicy, build_sections, build_sections_from_id},
        staff_peak::{PeakPoint, StaffPeakAttribute},
    };

    fn one_system_fixture() -> (PreparedStaffHandoff, Vec<u8>) {
        let width = 20;
        let height = 6;
        let mut pixels = vec![255; width * height];
        let mut table = RunTable::new(Orientation::Horizontal, width, height).unwrap();
        for y in [0, 5] {
            table.add_run(y, Run::new(0, width)).unwrap();
            for x in 0..width {
                pixels[(y * width) + x] = 0;
            }
        }
        for x in 5..=6 {
            for y in 0..height {
                pixels[(y * width) + x] = 0;
            }
        }

        let lines = build_sections(&table, JunctionPolicy::All)
            .into_iter()
            .enumerate()
            .map(|(index, section)| {
                let mut filament = audiveris_image::filament::StaffFilament::new(5).unwrap();
                filament.add_section(section).unwrap();
                PreparedStaffLine {
                    id: index + 1,
                    cluster_position: index as i32,
                    filament,
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);

        (
            PreparedStaffHandoff {
                staffs: vec![PreparedStaff {
                    id: 1,
                    kind: StaffCandidateKind::Standard,
                    left: 5.0,
                    right: 6.0,
                    interline: 5,
                    small: false,
                    short: false,
                    lines,
                }],
            },
            pixels,
        )
    }

    fn two_staff_connection_fixture() -> (PreparedStaffHandoff, Vec<u8>) {
        let width = 20;
        let height = 12;
        let mut pixels = vec![255; width * height];
        let line_pair = |first_y: usize, last_y: usize| {
            let mut table = RunTable::new(Orientation::Horizontal, width, height).unwrap();
            for y in [first_y, last_y] {
                table.add_run(y, Run::new(0, width)).unwrap();
            }
            build_sections(&table, JunctionPolicy::All)
                .into_iter()
                .enumerate()
                .map(|(index, section)| {
                    let mut filament = audiveris_image::filament::StaffFilament::new(5).unwrap();
                    filament.add_section(section).unwrap();
                    PreparedStaffLine {
                        id: index + 1,
                        cluster_position: index as i32,
                        filament,
                    }
                })
                .collect::<Vec<_>>()
        };
        for y in [0, 5, 6, 11] {
            for x in 0..width {
                pixels[(y * width) + x] = 0;
            }
        }
        for x in 5..=6 {
            for y in 0..height {
                pixels[(y * width) + x] = 0;
            }
        }
        (
            PreparedStaffHandoff {
                staffs: vec![
                    PreparedStaff {
                        id: 1,
                        kind: StaffCandidateKind::Standard,
                        left: 5.0,
                        right: 6.0,
                        interline: 5,
                        small: false,
                        short: false,
                        lines: line_pair(0, 5),
                    },
                    PreparedStaff {
                        id: 2,
                        kind: StaffCandidateKind::Standard,
                        left: 5.0,
                        right: 6.0,
                        interline: 5,
                        small: false,
                        short: false,
                        lines: line_pair(6, 11),
                    },
                ],
            },
            pixels,
        )
    }

    #[test]
    fn synthetic_system_registers_deskewed_bar_and_detached_brace() {
        let (handoff, pixels) = one_system_fixture();
        let ratios = StaffProjectorScaleRatios {
            staff_abscissa_margin: 20.0,
            bar_refine_dx: 2.0,
            bar_threshold: 0.8,
            brace_threshold: 0.8,
            gap_threshold: 0.2,
            minimum_wide_blank_width: 2.0,
            maximum_bar_width: 4.0,
            chunk_width: 1.0,
            ..StaffProjectorScaleRatios::java_defaults()
        };
        let tuning = StaffProjectorProcessTuning {
            top_derivative_count: 2,
            minimum_derivative_ratio: 1.0,
            blank_threshold_ratio: 2.1,
            chunk_threshold_ratio: 0.4,
            minimum_white_ratio_beyond_serif: 0.3,
        };
        let settings = [RawStaffProjectorSettings {
            staff_id: 1,
            barline_height: BarlineHeightSpec::Four,
            brace_search: Some(BraceSearchRequest::new(5, 0, 7, 2, 4)),
        }];
        let skew = HeadlessSkew::new(0.25, 20, 6);

        let prepared = prepare_raw_projectors(
            &handoff,
            RawProjectorRaster {
                width: 20,
                height: 6,
                pixels: &pixels,
            },
            &skew,
            RawProjectorParameters {
                large_interline: 1,
                foreground_thickness: 2,
                ratios,
                tuning,
                staffs: &settings,
            },
        )
        .unwrap();

        assert_eq!(prepared.registrations.len(), 1);
        assert_eq!(prepared.registry.projectors()[0].result.peaks.len(), 1);
        assert!(
            matches!(
                &prepared.registrations[0],
                ProjectorRegistration::Retained {
                    projector_index: 0,
                    added_graph_vertices,
                } if added_graph_vertices.len() == 1
            ),
            "{:?}",
            prepared.registrations
        );
        assert_eq!(prepared.registry.graph_vertex_order().len(), 1);
        let peak = &prepared.registry.projectors()[0].result.peaks[0];
        assert_eq!(
            (peak.start(), peak.stop(), peak.top(), peak.bottom()),
            (5, 6, 0, 5)
        );
        assert_eq!(
            peak.deskewed_center(),
            Some(skew.deskewed(PeakPoint::new(5.5, 2.5)))
        );

        assert_eq!(prepared.brace_peaks.len(), 1);
        let brace = &prepared.brace_peaks[0];
        assert_eq!(brace.staff_id, StaffId::new(1));
        assert!(brace.peak.is_set(StaffPeakAttribute::Brace));
        assert_eq!((brace.peak.start(), brace.peak.stop()), (4, 7));
        assert_eq!(
            brace.peak.deskewed_center(),
            Some(skew.deskewed(PeakPoint::new(5.5, 2.5)))
        );
        assert_eq!(
            prepared.registry.projectors()[0].result.brace_candidate,
            Some(audiveris_image::projection::ProjectionBraceCandidate {
                raw_start: 5,
                raw_stop: 6,
                start: 4,
                stop: 7,
                search_right: 7,
            })
        );

        let expected_key = peak.key();
        let expected_center = peak.deskewed_center().unwrap();
        let mut bridge = bridge_raw_projectors_to_graph(&prepared, 2).unwrap();
        assert_eq!(
            bridge
                .graph
                .vertices()
                .iter()
                .map(StaffPeak::key)
                .collect::<Vec<_>>(),
            prepared.registry.graph_vertex_order()
        );
        assert_eq!(
            bridge.graph.vertex(expected_key).unwrap().deskewed_center(),
            Some(expected_center)
        );
        assert_eq!(bridge.retained_staff_ids, vec![StaffId::new(1)]);
        assert!(bridge.discarded_one_line_staff_ids.is_empty());
        assert_eq!(bridge.brace_peaks, prepared.brace_peaks);
        match &mut bridge.grouping {
            RawSystemGroupingBoundary::CompleteSingleStaff {
                system_id,
                staff_id,
                columns,
            } => {
                assert_eq!(*system_id, 1);
                assert_eq!(*staff_id, StaffId::new(1));
                assert_eq!(columns.len(), 1);
                let peak = columns[0].peaks()[0].unwrap();
                assert_eq!(peak.id().value(), 1);
                assert_eq!(peak.staff_id(), StaffId::new(1));
                assert_eq!(peak.deskewed_x(), expected_center.x);
                assert_eq!(columns[0].deskewed_x(), expected_center.x);
            }
            RawSystemGroupingBoundary::NeedsAlignmentAndConnectionDiscovery { .. } => {
                panic!("one retained staff does not require alignment discovery")
            }
            RawSystemGroupingBoundary::NeedsConnectionDiscovery { .. } => {
                panic!("one retained staff does not require connection discovery")
            }
            RawSystemGroupingBoundary::NeedsSplitAndAlignmentPurge { .. } => {
                panic!("one retained staff does not require split or alignment purge")
            }
            RawSystemGroupingBoundary::NeedsSystemGrouping { .. } => {
                panic!("one retained staff does not require system grouping")
            }
            RawSystemGroupingBoundary::CompleteSystems { .. } => {
                panic!("one retained staff uses its earlier direct completion")
            }
            RawSystemGroupingBoundary::NeedsBracePortionDetection { .. } => {
                panic!("one retained staff does not require brace-portion grouping")
            }
            RawSystemGroupingBoundary::NeedsBracketEndDetection { .. } => {
                panic!("one retained staff does not require bracket-end grouping")
            }
            RawSystemGroupingBoundary::NeedsLeftPeakPurge { .. } => {
                panic!("one retained staff does not require left-peak purge grouping")
            }
            RawSystemGroupingBoundary::NeedsRightEndRefinement { .. } => {
                panic!("one retained staff does not require right-end refinement grouping")
            }
            RawSystemGroupingBoundary::NeedsWidthPartition { .. } => {
                panic!("one retained staff does not require width partition grouping")
            }
            RawSystemGroupingBoundary::NeedsConnectionInters { .. } => {
                panic!("one retained staff does not require connection-inter grouping")
            }
            RawSystemGroupingBoundary::NeedsBarRecording { .. } => {
                panic!("one retained staff does not require bar-recording grouping")
            }
            RawSystemGroupingBoundary::NeedsPartCreation { .. } => {
                panic!("one retained staff does not require part-creation grouping")
            }
            RawSystemGroupingBoundary::CompleteBars { .. } => {
                panic!("one retained staff does not use grouped bar completion")
            }
        }

        let mut vertical_table = RunTable::new(Orientation::Vertical, 20, 6).unwrap();
        vertical_table.add_run(5, Run::new(0, 6)).unwrap();
        vertical_table.add_run(6, Run::new(0, 6)).unwrap();
        let vertical_sections = build_sections(&vertical_table, JunctionPolicy::All);
        let with_sticks = bridge_raw_projectors_through_bar_sticks(
            &prepared,
            &vertical_sections,
            &[],
            BarStickParameters {
                vertical_extension: 0,
                minimum_core_section_length: 3,
                probe_width: 1,
                minimum_probe_weight: 1,
                segment_length: 3,
                minimum_mean_curvature: 0.0,
                first_filament_id: 1,
            },
            2,
        )
        .unwrap();
        assert_eq!(with_sticks.sticks.sticks().len(), 1);
        assert_eq!(with_sticks.sticks.sticks()[0].peak, expected_key);
        assert_eq!(with_sticks.sticks.sticks()[0].id, 1);
        assert_eq!(with_sticks.sticks.sticks()[0].members.len(), 1);
        assert_eq!(
            with_sticks
                .projectors
                .graph
                .vertex(expected_key)
                .unwrap()
                .deskewed_center(),
            Some(expected_center)
        );
        assert!(matches!(
            with_sticks.projectors.grouping,
            RawSystemGroupingBoundary::CompleteSingleStaff { ref columns, .. }
                if columns.len() == 1
        ));
    }

    #[test]
    fn multiple_staffs_stop_before_missing_alignment_discovery() {
        let (mut handoff, pixels) = one_system_fixture();
        let mut second = handoff.staffs[0].clone();
        second.id = 2;
        handoff.staffs.push(second);
        let settings = [1, 2].map(|staff_id| RawStaffProjectorSettings {
            staff_id,
            barline_height: BarlineHeightSpec::Four,
            brace_search: None,
        });
        let prepared = prepare_raw_projectors(
            &handoff,
            RawProjectorRaster {
                width: 20,
                height: 6,
                pixels: &pixels,
            },
            &HeadlessSkew::new(0.0, 20, 6),
            RawProjectorParameters {
                large_interline: 1,
                foreground_thickness: 2,
                ratios: StaffProjectorScaleRatios {
                    staff_abscissa_margin: 20.0,
                    bar_refine_dx: 2.0,
                    bar_threshold: 0.8,
                    gap_threshold: 0.2,
                    minimum_wide_blank_width: 2.0,
                    maximum_bar_width: 4.0,
                    chunk_width: 1.0,
                    ..StaffProjectorScaleRatios::java_defaults()
                },
                tuning: StaffProjectorProcessTuning {
                    top_derivative_count: 2,
                    minimum_derivative_ratio: 1.0,
                    blank_threshold_ratio: 2.1,
                    chunk_threshold_ratio: 0.4,
                    minimum_white_ratio_beyond_serif: 0.3,
                },
                staffs: &settings,
            },
        )
        .unwrap();

        let bridge = bridge_raw_projectors_to_graph(&prepared, 2).unwrap();
        assert_eq!(bridge.graph.vertices().len(), 2);
        assert!(bridge.graph.edges().is_empty());
        assert!(matches!(
            bridge.grouping,
            RawSystemGroupingBoundary::NeedsAlignmentAndConnectionDiscovery { ref staff_ids }
                if staff_ids == &[StaffId::new(1), StaffId::new(2)]
        ));

        let mut vertical_table = RunTable::new(Orientation::Vertical, 20, 6).unwrap();
        vertical_table.add_run(5, Run::new(0, 6)).unwrap();
        vertical_table.add_run(6, Run::new(0, 6)).unwrap();
        let vertical_sections = build_sections(&vertical_table, JunctionPolicy::All);
        let aligned = bridge_raw_projectors_through_alignments(
            &handoff,
            &prepared,
            &vertical_sections,
            &[],
            &HeadlessSkew::new(0.0, 20, 6),
            RawAlignmentBridgeParameters {
                sticks: BarStickParameters {
                    vertical_extension: 0,
                    minimum_core_section_length: 3,
                    probe_width: 1,
                    minimum_probe_weight: 1,
                    segment_length: 3,
                    minimum_mean_curvature: 0.0,
                    first_filament_id: 1,
                },
                maximum_alignment_slope: 0.06,
                maximum_alignment_delta_width: 1,
                maximum_column_dx: 2,
            },
        )
        .unwrap();
        assert_eq!(aligned.alignments.edge_ids().len(), 1);
        assert_eq!(aligned.bars.projectors.graph.edges().len(), 1);
        let edge = &aligned.bars.projectors.graph.edges()[0];
        assert_eq!(edge.source().staff_id(), StaffId::new(1));
        assert_eq!(edge.target().staff_id(), StaffId::new(2));
        assert!(matches!(
            aligned.bars.projectors.grouping,
            RawSystemGroupingBoundary::NeedsConnectionDiscovery {
                ref staff_ids,
                alignment_count: 1,
            } if staff_ids == &[StaffId::new(1), StaffId::new(2)]
        ));
    }

    #[test]
    fn settings_are_exact_and_fail_closed() {
        let (handoff, pixels) = one_system_fixture();
        let skew = HeadlessSkew::new(0.0, 20, 6);
        let error = prepare_raw_projectors(
            &handoff,
            RawProjectorRaster {
                width: 20,
                height: 6,
                pixels: &pixels,
            },
            &skew,
            RawProjectorParameters {
                large_interline: 5,
                foreground_thickness: 1,
                ratios: StaffProjectorScaleRatios::java_defaults(),
                tuning: StaffProjectorProcessTuning::java_defaults(),
                staffs: &[],
            },
        )
        .unwrap_err();
        assert_eq!(error, RawProjectorAdapterError::MissingStaffSettings(1));
    }

    #[test]
    fn two_staff_raw_raster_promotes_connection_and_stops_before_split_purge() {
        let (handoff, pixels) = two_staff_connection_fixture();
        let settings = [1, 2].map(|staff_id| RawStaffProjectorSettings {
            staff_id,
            barline_height: BarlineHeightSpec::Four,
            brace_search: None,
        });
        let skew = HeadlessSkew::new(0.0, 20, 12);
        let raster = RawProjectorRaster {
            width: 20,
            height: 12,
            pixels: &pixels,
        };
        let prepared = prepare_raw_projectors(
            &handoff,
            raster,
            &skew,
            RawProjectorParameters {
                large_interline: 1,
                foreground_thickness: 2,
                ratios: StaffProjectorScaleRatios {
                    staff_abscissa_margin: 20.0,
                    bar_refine_dx: 2.0,
                    bar_threshold: 0.8,
                    gap_threshold: 0.2,
                    minimum_wide_blank_width: 2.0,
                    maximum_bar_width: 4.0,
                    chunk_width: 1.0,
                    ..StaffProjectorScaleRatios::java_defaults()
                },
                tuning: StaffProjectorProcessTuning {
                    top_derivative_count: 2,
                    minimum_derivative_ratio: 1.0,
                    blank_threshold_ratio: 2.1,
                    chunk_threshold_ratio: 0.4,
                    minimum_white_ratio_beyond_serif: 0.3,
                },
                staffs: &settings,
            },
        )
        .unwrap();
        let mut vertical_table = RunTable::new(Orientation::Vertical, 20, 12).unwrap();
        vertical_table.add_run(5, Run::new(0, 12)).unwrap();
        vertical_table.add_run(6, Run::new(0, 12)).unwrap();
        let vertical_sections = build_sections(&vertical_table, JunctionPolicy::All);

        let connected = bridge_raw_projectors_through_connections(
            &handoff,
            &prepared,
            raster,
            &vertical_sections,
            &[],
            &skew,
            RawConnectionBridgeParameters {
                alignments: RawAlignmentBridgeParameters {
                    sticks: BarStickParameters {
                        vertical_extension: 0,
                        minimum_core_section_length: 3,
                        probe_width: 1,
                        minimum_probe_weight: 1,
                        segment_length: 3,
                        minimum_mean_curvature: 0.0,
                        first_filament_id: 1,
                    },
                    maximum_alignment_slope: 0.06,
                    maximum_alignment_delta_width: 1,
                    maximum_column_dx: 2,
                },
                maximum_connection_gap: 1,
                maximum_connection_white_ratio: 0.25,
            },
        )
        .unwrap();

        assert_eq!(connected.connections.decisions().len(), 1);
        assert_eq!(connected.connections.promoted_count(), 1);
        let graph = &connected.alignments.bars.projectors.graph;
        assert_eq!(graph.edges().len(), 1);
        assert_eq!(
            graph.edges()[0].relation().kind(),
            BarAlignmentKind::Connection
        );
        assert!(matches!(
            connected.alignments.bars.projectors.grouping,
            RawSystemGroupingBoundary::NeedsSplitAndAlignmentPurge {
                ref staff_ids,
                alignment_count: 1,
                connection_count: 1,
            } if staff_ids == &[StaffId::new(1), StaffId::new(2)]
        ));

        let split = bridge_raw_projectors_through_splits(
            &handoff,
            &prepared,
            raster,
            &vertical_sections,
            &[],
            &skew,
            RawSplitBridgeParameters {
                connections: RawConnectionBridgeParameters {
                    alignments: RawAlignmentBridgeParameters {
                        sticks: BarStickParameters {
                            vertical_extension: 0,
                            minimum_core_section_length: 3,
                            probe_width: 1,
                            minimum_probe_weight: 1,
                            segment_length: 3,
                            minimum_mean_curvature: 0.0,
                            first_filament_id: 1,
                        },
                        maximum_alignment_slope: 0.06,
                        maximum_alignment_delta_width: 1,
                        maximum_column_dx: 2,
                    },
                    maximum_connection_gap: 1,
                    maximum_connection_white_ratio: 0.25,
                },
                maximum_close_gap: 2,
                maximum_width_ratio: 0.5,
            },
        )
        .unwrap();
        assert!(split.splits.splits.is_empty());
        assert!(split.splits.purged_alignments.is_empty());
        assert!(matches!(
            split.connections.alignments.bars.projectors.grouping,
            RawSystemGroupingBoundary::NeedsSystemGrouping {
                ref staff_ids,
                retained_edge_count: 1,
                connection_count: 1,
            } if staff_ids == &[StaffId::new(1), StaffId::new(2)]
        ));

        let systems = bridge_raw_projectors_through_systems(
            &handoff,
            &prepared,
            raster,
            &vertical_sections,
            &[],
            &skew,
            RawSystemGroupingParameters {
                splits: RawSplitBridgeParameters {
                    connections: RawConnectionBridgeParameters {
                        alignments: RawAlignmentBridgeParameters {
                            sticks: BarStickParameters {
                                vertical_extension: 0,
                                minimum_core_section_length: 3,
                                probe_width: 1,
                                minimum_probe_weight: 1,
                                segment_length: 3,
                                minimum_mean_curvature: 0.0,
                                first_filament_id: 1,
                            },
                            maximum_alignment_slope: 0.06,
                            maximum_alignment_delta_width: 1,
                            maximum_column_dx: 2,
                        },
                        maximum_connection_gap: 1,
                        maximum_connection_white_ratio: 0.25,
                    },
                    maximum_close_gap: 2,
                    maximum_width_ratio: 0.5,
                },
                maximum_first_connection_x_offset: 20,
            },
        )
        .unwrap();
        assert_eq!(systems.systems.systems.len(), 1);
        assert_eq!(systems.systems.systems[0].staff_ids.len(), 2);
        assert_eq!(systems.systems.systems[0].columns.len(), 1);
        assert!(matches!(
            systems.split.connections.alignments.bars.projectors.grouping,
            RawSystemGroupingBoundary::CompleteSystems {
                ref staff_ids,
                system_count: 1,
            } if staff_ids == &[StaffId::new(1), StaffId::new(2)]
        ));

        let mut prefix = bridge_raw_projectors_through_bars_prefix(
            &handoff,
            &prepared,
            raster,
            &vertical_sections,
            &[],
            &skew,
            RawBarsPrefixParameters {
                systems: RawSystemGroupingParameters {
                    splits: RawSplitBridgeParameters {
                        connections: RawConnectionBridgeParameters {
                            alignments: RawAlignmentBridgeParameters {
                                sticks: BarStickParameters {
                                    vertical_extension: 0,
                                    minimum_core_section_length: 3,
                                    probe_width: 1,
                                    minimum_probe_weight: 1,
                                    segment_length: 3,
                                    minimum_mean_curvature: 0.0,
                                    first_filament_id: 1,
                                },
                                maximum_alignment_slope: 0.06,
                                maximum_alignment_delta_width: 1,
                                maximum_column_dx: 2,
                            },
                            maximum_connection_gap: 1,
                            maximum_connection_white_ratio: 0.25,
                        },
                        maximum_close_gap: 2,
                        maximum_width_ratio: 0.5,
                    },
                    maximum_first_connection_x_offset: 20,
                },
                bars: BarsCoordinatorParameters::new(2, 10, 2, 20, 2, 1, 0.2, None).unwrap(),
            },
        )
        .unwrap();
        assert_eq!(prefix.bars.len(), 1);
        assert_eq!(prefix.bars[0].state.staffs().len(), 2);
        assert_eq!(prefix.bars[0].state.columns().len(), 1);
        assert!(matches!(
            prefix
                .systems
                .split
                .connections
                .alignments
                .bars
                .projectors
                .grouping,
            RawSystemGroupingBoundary::NeedsBracePortionDetection {
                ref staff_ids,
                system_count: 1,
            } if staff_ids == &[StaffId::new(1), StaffId::new(2)]
        ));

        let make_brace_section = |id, y| {
            let mut table = RunTable::new(Orientation::Vertical, 3, 12).unwrap();
            table.add_run(1, Run::new(y, 6)).unwrap();
            build_sections_from_id(&table, JunctionPolicy::All, id)
                .into_iter()
                .find(|section| section.id() == id)
                .unwrap()
        };
        let top_member = make_brace_section(100, 0);
        let bottom_member = make_brace_section(101, 6);
        let all_brace_sections = vec![top_member.clone(), bottom_member.clone()];
        let mut sig_store = BraceSigStore::new(0, 0, [1]);
        let brace_report = continue_raw_bars_through_lines_root(
            &mut prefix,
            &prepared,
            &all_brace_sections,
            &skew,
            RawBraceStageParameters {
                portions: BracePortionParameters {
                    neutral_gap: 1,
                    maximum_peak_width: 2,
                    maximum_bar_gap: 4,
                    minimum_portion_height: 5.0,
                    maximum_curvature: 10.0,
                    lookup_extension: 2.0,
                },
                filament: BraceFilamentParameters {
                    interline: 5,
                    segment_length: 5,
                    left_margin: 1,
                },
                maximum_alignment_dx: 4.0,
            },
            &mut sig_store,
            |_, staff_id, _, _| {
                let (top, bottom, extension_top, extension_bottom) = if staff_id == StaffId::new(1)
                {
                    (0, 5, 0.0, 3.0)
                } else {
                    (6, 11, 3.0, 0.0)
                };
                let mut peak = StaffPeak::new(staff_id, top, bottom, 1, 1).unwrap();
                peak.set(StaffPeakAttribute::Brace);
                Ok(Some(BraceProbe {
                    peak,
                    filament: Some(crate::brace_portions::BraceFilamentEvidence {
                        vertical_length: 6.0,
                        mean_curvature: 1.0,
                        extension_top,
                        extension_bottom,
                    }),
                }))
            },
            |key| {
                Ok(if key.staff_id() == StaffId::new(1) {
                    vec![top_member.clone()]
                } else {
                    vec![bottom_member.clone()]
                })
            },
        )
        .unwrap();
        assert_eq!(brace_report.systems.len(), 1);
        assert_eq!(brace_report.systems[0].promotions.len(), 1);
        assert!(
            prefix.bars[0].state.staffs()[0]
                .brace_peak()
                .unwrap()
                .is_set(StaffPeakAttribute::BraceTop)
        );
        assert!(
            prefix.bars[0].state.staffs()[1]
                .brace_peak()
                .unwrap()
                .is_set(StaffPeakAttribute::BraceBottom)
        );
        assert_eq!(sig_store.originals().len(), 1);
        assert_eq!(sig_store.system_nodes(1).unwrap().len(), 1);
        assert!(matches!(
            prefix
                .systems
                .split
                .connections
                .alignments
                .bars
                .projectors
                .grouping,
            RawSystemGroupingBoundary::NeedsBracketEndDetection {
                ref staff_ids,
                system_count: 1,
            } if staff_ids == &[StaffId::new(1), StaffId::new(2)]
        ));

        let next_serif_id = prefix
            .systems
            .split
            .connections
            .alignments
            .bars
            .sticks
            .next_filament_id();
        let mut serif_store = BracketSerifStore::new(next_serif_id).unwrap();
        let bracket_report = continue_raw_bars_through_bracket_middles(
            &mut prefix,
            &vertical_sections,
            &[],
            RawBracketStageParameters {
                serif: BracketSerifParameters {
                    interline: 5,
                    maximum_foreground_thickness: 1,
                    minimum_bracket_width: 1,
                    maximum_bracket_extension: 6.0,
                    serif_roi_width: 10,
                    serif_roi_height: 10,
                    serif_minimum_weight: 1,
                },
            },
            &mut serif_store,
        )
        .unwrap();
        assert_eq!(bracket_report.systems.len(), 1);
        assert!(bracket_report.systems[0].ends.is_empty());
        assert!(bracket_report.systems[0].middles.is_empty());
        assert!(serif_store.filaments().is_empty());
        assert!(matches!(
            prefix
                .systems
                .split
                .connections
                .alignments
                .bars
                .projectors
                .grouping,
            RawSystemGroupingBoundary::NeedsLeftPeakPurge {
                ref staff_ids,
                system_count: 1,
            } if staff_ids == &[StaffId::new(1), StaffId::new(2)]
        ));

        let purge_report = continue_raw_bars_through_peak_purges(
            &mut prefix,
            RawPeakPurgeStageParameters {
                purges: BarsPurgeParameters {
                    large_system_staff_count: 10,
                    maximum_foreground_thickness: 1,
                    maximum_bar_extension: 5.0,
                },
            },
        )
        .unwrap();
        assert_eq!(purge_report.systems.len(), 1);
        assert!(purge_report.systems[0].purges.removed_peaks().is_empty());
        assert!(matches!(
            prefix
                .systems
                .split
                .connections
                .alignments
                .bars
                .projectors
                .grouping,
            RawSystemGroupingBoundary::NeedsRightEndRefinement {
                ref staff_ids,
                system_count: 1,
            } if staff_ids == &[StaffId::new(1), StaffId::new(2)]
        ));

        let right_report = continue_raw_bars_through_right_ends_and_c_clefs(
            &mut prefix,
            &prepared,
            raster.width,
            RawRightCClefStageParameters {
                bars: BarsRightCClefParameters {
                    maximum_double_bar_gap: 2,
                    c_clef: audiveris_image::bars_coordinator::CClefParameters {
                        minimum_first_peak_width: 2,
                        maximum_second_peak_width: 1,
                        minimum_measure_width: 10,
                        tail_width: 4,
                    },
                },
            },
        )
        .unwrap();
        assert_eq!(right_report.systems.len(), 1);
        assert_eq!(right_report.systems[0].result.right_updates().len(), 2);
        assert!(
            right_report.systems[0]
                .result
                .c_clef_detections()
                .is_empty()
        );
        assert!(right_report.systems[0].result.removed_peaks().is_empty());
        assert!(matches!(
            prefix
                .systems
                .split
                .connections
                .alignments
                .bars
                .projectors
                .grouping,
            RawSystemGroupingBoundary::NeedsWidthPartition {
                ref staff_ids,
                system_count: 1,
            } if staff_ids == &[StaffId::new(1), StaffId::new(2)]
        ));

        let expected_inter_peaks = prefix.bars[0]
            .state
            .staffs()
            .iter()
            .flat_map(|staff| staff.peaks())
            .filter(|peak| !peak.is_brace())
            .map(StaffPeak::key)
            .collect::<Vec<_>>();
        let width_report = continue_raw_bars_through_widths_and_inters(
            &mut prefix,
            RawWidthInterStageParameters {
                bars: BarsWidthInterParameters {
                    maximum_double_bar_gap: 2,
                    interline: 10,
                    minimum_normalized_width_delta: 0.2,
                    foreground_thickness: 2,
                },
            },
        )
        .unwrap();
        assert_eq!(width_report.systems.len(), 1);
        assert_eq!(
            width_report.systems[0]
                .result
                .vertical_inters()
                .iter()
                .map(|plan| plan.peak)
                .collect::<Vec<_>>(),
            expected_inter_peaks
        );
        assert_eq!(
            width_report.systems[0].result.promoted_inters().len(),
            expected_inter_peaks.len()
        );
        for peak in expected_inter_peaks {
            assert!(prefix.bars[0].sig.inter_of(peak).is_some());
            let projector = prefix.bars[0]
                .state
                .staffs()
                .iter()
                .flat_map(|staff| staff.peaks())
                .find(|candidate| candidate.key() == peak)
                .unwrap();
            let graph = prefix.bars[0].state.graph().vertex(peak).unwrap();
            assert_eq!(
                projector.attributes().collect::<Vec<_>>(),
                graph.attributes().collect::<Vec<_>>()
            );
        }
        assert!(matches!(
            prefix
                .systems
                .split
                .connections
                .alignments
                .bars
                .projectors
                .grouping,
            RawSystemGroupingBoundary::NeedsConnectionInters {
                ref staff_ids,
                system_count: 1,
            } if staff_ids == &[StaffId::new(1), StaffId::new(2)]
        ));

        let connection_report = continue_raw_bars_through_connections_and_groups(
            &mut prefix,
            RawConnectionGroupStageParameters {
                bars: BarsConnectionGroupParameters {
                    maximum_double_bar_gap: 2,
                    interline: 10.0,
                },
            },
        )
        .unwrap();
        assert_eq!(connection_report.systems.len(), 1);
        assert!(
            connection_report.systems[0]
                .result
                .connection_warnings()
                .is_empty()
        );
        assert!(matches!(
            prefix
                .systems
                .split
                .connections
                .alignments
                .bars
                .projectors
                .grouping,
            RawSystemGroupingBoundary::NeedsBarRecording {
                ref staff_ids,
                system_count: 1,
            } if staff_ids == &[StaffId::new(1), StaffId::new(2)]
        ));

        let recording_report = continue_raw_bars_through_recording_and_groups(&mut prefix).unwrap();
        assert_eq!(recording_report.systems.len(), 1);
        assert_eq!(
            recording_report.systems[0]
                .result
                .staff_barlines()
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert!(matches!(
            prefix
                .systems
                .split
                .connections
                .alignments
                .bars
                .projectors
                .grouping,
            RawSystemGroupingBoundary::NeedsPartCreation {
                ref staff_ids,
                system_count: 1,
            } if staff_ids == &[StaffId::new(1), StaffId::new(2)]
        ));

        let part_report = finish_raw_bars_through_parts_and_context(
            &mut prefix,
            RawPartContextStageParameters {
                bars: BarTailParameters::default(),
            },
        )
        .unwrap();
        assert_eq!(part_report.systems.len(), 1);
        assert_eq!(part_report.systems[0].result.parts().len(), 2);
        assert!(prefix.bars[0].bar_tail.contextualized);
        assert!(matches!(
            prefix
                .systems
                .split
                .connections
                .alignments
                .bars
                .projectors
                .grouping,
            RawSystemGroupingBoundary::CompleteBars {
                ref staff_ids,
                system_count: 1,
            } if staff_ids == &[StaffId::new(1), StaffId::new(2)]
        ));

        let completion = raw_bars_completion_handoff(&prefix).unwrap();
        assert_eq!(
            completion.ownership(),
            [PreparedCompletionSystem {
                system_id: 1,
                staff_ids: vec![1, 2],
            }]
        );
        assert!(completion.systems()[0].bar_tail.contextualized);
        assert_eq!(completion.systems()[0].sig.nodes_in_order().count(), 3);

        let mut incomplete = prefix.clone();
        incomplete
            .systems
            .split
            .connections
            .alignments
            .bars
            .projectors
            .grouping = RawSystemGroupingBoundary::NeedsPartCreation {
            staff_ids: vec![StaffId::new(1), StaffId::new(2)],
            system_count: 1,
        };
        assert!(matches!(
            raw_bars_completion_handoff(&incomplete),
            Err(RawProjectorAdapterError::BarsNotComplete)
        ));

        let mut uncontextualized = prefix.clone();
        uncontextualized.bars[0].bar_tail.contextualized = false;
        assert!(matches!(
            raw_bars_completion_handoff(&uncontextualized),
            Err(RawProjectorAdapterError::UncontextualizedBars(1))
        ));
    }
}
