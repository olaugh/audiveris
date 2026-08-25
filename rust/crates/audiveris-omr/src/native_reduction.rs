// SPDX-License-Identifier: AGPL-3.0-or-later

//! Native semantic primitives for Java's `ReductionStep`.
//!
//! The dependency-light lifecycle in [`crate::reduction_step`] owns sheet and
//! system ordering.  This module starts the production bridge from terminal
//! native STEMS SIGs with Java's deterministic `SIGraph.reduceExclusions()`
//! algorithm, lossless overlap discovery, chord prolog, and the complete
//! foundations outer fixed point.  It also owns the enabled
//! `StemInter.refineHeadEnd()` pass that immediately follows foundations and
//! the sheet-epilog beam-group consistency and free-stem measurement passes.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use audiveris_image::{
    beam_structure::Segment,
    run_table::{Orientation, RunTable},
};

use crate::grid_executor::HeadlessSkew;
use crate::head_scanner_slices::VerticalRibbonArea;
use crate::native_sig::{
    NativeSigBounds, NativeSigContextualization, NativeSigEdge, NativeSigEdgeId, NativeSigError,
    NativeSigInterKind, NativeSigRelationKind, NativeSigRelationOrigin, NativeSigSystem,
    NativeSigSystemBindings, NativeSigVertexId,
};
use crate::native_stems::NativeStemsRecognition;
use crate::native_stems_beam_vlinkers::generic_intersection;
use crate::stems_step::{
    NativeBeamPortion, NativeStemHeadSide, NativeStemLine, NativeStemPoint, NativeStemVerticalSide,
};

/// One selected exclusion and the branch Java removed from it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeReductionExclusionDecision {
    pub exclusion: NativeSigEdgeId,
    pub source: NativeSigVertexId,
    pub source_best_grade: f64,
    pub target: NativeSigVertexId,
    pub target_best_grade: f64,
    pub removed: NativeSigVertexId,
}

/// Exact observable result of one `SIGraph.reduceExclusions()` call.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeReductionExclusionTransaction {
    pub system_id: usize,
    /// Decision order. Java uses exclusion insertion order as the tie-breaker.
    pub decisions: Vec<NativeReductionExclusionDecision>,
    /// Java `LinkedHashSet` order: weaker inter, then any dying ensembles.
    pub removed_vertices: Vec<NativeSigVertexId>,
    /// Fresh whole-SIG contextualizations after each removal. Recomputing the
    /// whole native SIG is value-equivalent to Java's impacted-neighbor update.
    pub contextualizations: Vec<NativeSigContextualization>,
}

/// Owned native pipeline state after reducing exclusions already present in
/// each terminal STEMS SIG.
///
/// This is deliberately not named a completed REDUCTION recognition: Java's
/// preceding overlap discovery and its foundation consistency checks are not
/// part of this boundary yet.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeReductionExistingExclusionsRecognition {
    pub stems: NativeStemsRecognition,
    pub initial_contextualizations: Vec<NativeSigContextualization>,
    pub transactions: Vec<NativeReductionExclusionTransaction>,
}

/// Ordered removals from one foundation consistency check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeReductionBeamPruneTransaction {
    pub system_id: usize,
    /// Beam/hook iteration order; this is Java's modification count authority.
    pub removed_beams: Vec<NativeSigVertexId>,
    /// Dying sole-member groups removed extensively with those beams.
    pub removed_groups: Vec<NativeSigVertexId>,
}

/// Ordered orphan removal from the first branch of Java `checkHeads()` or
/// `checkStems()`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeReductionOrphanPruneTransaction {
    pub system_id: usize,
    pub kind: NativeSigInterKind,
    pub removed_vertices: Vec<NativeSigVertexId>,
    pub removed_ensembles: Vec<NativeSigVertexId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeReductionStemEndingPrune {
    pub stem: NativeSigVertexId,
    pub removed_head_stem_edges: Vec<NativeSigEdgeId>,
    pub added_exclusions: Vec<NativeSigEdgeId>,
}

/// Result of Java `checkStemEndingHeads()` in stem insertion order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeReductionStemEndingTransaction {
    pub system_id: usize,
    /// Only stems for which at least one wrong-side ending link was removed.
    pub modified_stems: Vec<NativeReductionStemEndingPrune>,
}

/// Staff/pitch identity consumed by Java `SigReducer.lookupHead`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeReductionHeadIdentity {
    pub staff_id: usize,
    pub integer_pitch: i32,
}

type NativeReductionHeadLookup = (
    BTreeMap<NativeSigVertexId, NativeReductionHeadIdentity>,
    Vec<(usize, usize)>,
);

/// One Java-counted mutation from `checkHeads()`, in its exact head/relation
/// traversal order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeReductionHeadMutation {
    OrphanRemoved {
        head: NativeSigVertexId,
    },
    UnknownDirectionStemRemoved {
        head: NativeSigVertexId,
        stem: NativeSigVertexId,
        relation: NativeSigEdgeId,
    },
    WrongSideRelationRemoved {
        head: NativeSigVertexId,
        stem: NativeSigVertexId,
        relation: NativeSigEdgeId,
        exclusion: Option<NativeSigEdgeId>,
    },
}

/// Full Java `checkHeads()` graph transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeReductionHeadCheckTransaction {
    pub system_id: usize,
    pub head_order: Vec<NativeSigVertexId>,
    pub mutations: Vec<NativeReductionHeadMutation>,
    /// Ensemble cascades are not included in Java's modification count but
    /// remain observable graph mutations.
    pub removed_ensembles: Vec<NativeSigVertexId>,
    /// `StemInter.remove()` can invalidate same-stem `HeadHeadRelation`s.
    pub removed_head_head_supports: Vec<NativeSigEdgeId>,
}

/// One pathological ledger identity repaired between two adjacent staff maps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeReductionSharedLedgerFix {
    pub ledger: NativeSigVertexId,
    pub upper_staff_id: usize,
    pub lower_staff_id: usize,
    pub column_ledgers: Vec<NativeSigVertexId>,
    pub column_heads: Vec<NativeSigVertexId>,
    pub owner_staff_id: Option<usize>,
}

/// One unsupported ledger removal, including the iterative Java pass that
/// exposed it after an outer ledger disappeared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeReductionLedgerRemoval {
    pub pass: usize,
    pub staff_id: usize,
    pub ledger_index: i32,
    pub ledger: NativeSigVertexId,
}

/// Full Java `checkLedgers()` transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeReductionLedgerCheckTransaction {
    pub system_id: usize,
    pub shared_fixes: Vec<NativeReductionSharedLedgerFix>,
    pub removals: Vec<NativeReductionLedgerRemoval>,
    /// Java's return value: shared identities fixed plus SIG ledgers removed.
    pub modification_count: usize,
}

/// One directed stem whose head links at the tail end were cut by Java
/// `stemHasSingleHeadEnd()`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeReductionStemTailPrune {
    pub stem: NativeSigVertexId,
    /// Java `StemInter.computeDirection()`: -1 upward, +1 downward.
    pub direction: i8,
    pub removed_head_stem_edges: Vec<NativeSigEdgeId>,
    pub added_exclusions: Vec<NativeSigEdgeId>,
}

/// Full Java `checkStems()` transaction in the snapshotted stem order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeReductionStemCheckTransaction {
    pub system_id: usize,
    pub stem_order: Vec<NativeSigVertexId>,
    pub removed_orphan_stems: Vec<NativeSigVertexId>,
    pub tail_prunes: Vec<NativeReductionStemTailPrune>,
    pub removed_ensembles: Vec<NativeSigVertexId>,
    /// Java's return value: one per orphan stem or modified directed stem.
    pub modification_count: usize,
}

/// Exact relation mutations made by foundations `analyzeChords()` in good
/// stem order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeReductionChordAnalysisTransaction {
    pub system_id: usize,
    pub scanned_stems: Vec<NativeSigVertexId>,
    pub intersected_head_exclusions: Vec<NativeSigEdgeId>,
    pub incompatible_exclusions: Vec<NativeSigEdgeId>,
    pub head_head_supports: Vec<NativeSigEdgeId>,
}

/// One invocation of Java foundations `checkConsistencies()`.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeReductionFoundationConsistencyPassTransaction {
    pub system_id: usize,
    pub stem_ending: NativeReductionStemEndingTransaction,
    pub post_stem_ending_weak_purge: NativeReductionWeakPurgeTransaction,
    pub heads: NativeReductionHeadCheckTransaction,
    pub post_heads_weak_purge: NativeReductionWeakPurgeTransaction,
    pub hooks: NativeReductionBeamPruneTransaction,
    pub post_hooks_weak_purge: NativeReductionWeakPurgeTransaction,
    pub beams: NativeReductionBeamPruneTransaction,
    pub post_beams_weak_purge: NativeReductionWeakPurgeTransaction,
    pub ledgers: NativeReductionLedgerCheckTransaction,
    pub post_ledgers_weak_purge: NativeReductionWeakPurgeTransaction,
    pub stems: NativeReductionStemCheckTransaction,
    pub post_stems_weak_purge: NativeReductionWeakPurgeTransaction,
    /// Exact sum returned by Java's six checks. Purge removals do not count.
    pub modification_count: usize,
}

/// Java foundations setup plus the complete inner consistency fixed point.
///
/// `consistency_passes` includes the final zero-modification invocation which
/// makes Java's `while ((modifs = checkConsistencies()) > 0)` terminate.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeReductionFoundationFixedPointTransaction {
    pub system_id: usize,
    pub overlap: NativeReductionOverlapTransaction,
    pub pre_prolog_contextualization: NativeSigContextualization,
    pub chord_analysis: NativeReductionChordAnalysisTransaction,
    pub initial_weak_purge: NativeReductionWeakPurgeTransaction,
    pub consistency_passes: Vec<NativeReductionFoundationConsistencyPassTransaction>,
}

/// The single foundations `checkLateConsistencies()` invocation.
///
/// Java currently performs chord analysis, reduces the exclusions it creates,
/// contextualizes/purges, and returns zero because stem-length validation is
/// commented out.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeReductionFoundationLateConsistencyTransaction {
    pub system_id: usize,
    pub chord_analysis: NativeReductionChordAnalysisTransaction,
    pub exclusions: NativeReductionExclusionTransaction,
    pub weak_purge: NativeReductionWeakPurgeTransaction,
    pub modification_count: usize,
}

/// Java's first foundations reduction epoch through late consistency.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeReductionFoundationEpochTransaction {
    pub system_id: usize,
    pub fixed_point: NativeReductionFoundationFixedPointTransaction,
    /// The outer reducer's local `reduced` set before late consistency.
    pub remaining_exclusions: NativeReductionExclusionTransaction,
    pub late_consistency: NativeReductionFoundationLateConsistencyTransaction,
    /// Java's local `deleted` set contains only the epoch-opening purge here;
    /// adapter-owned purge sets are deliberately distinct.
    pub outer_deleted_vertices: Vec<NativeSigVertexId>,
    /// Java's local `reduced` set contains this pre-late exclusion result.
    pub outer_reduced_vertices: Vec<NativeSigVertexId>,
    pub requires_outer_repeat: bool,
}

/// One foundations outer epoch after the one-time overlap/context/prolog.
///
/// Java's local `deleted` and `reduced` variables shadow the adapter fields
/// with the same names.  Consequently only `opening_weak_purge` and
/// `remaining_exclusions` drive the outer `do/while`; mutations performed by
/// consistency and late-consistency calls remain deliberately excluded.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeReductionFoundationContinuationEpochTransaction {
    pub system_id: usize,
    pub opening_weak_purge: NativeReductionWeakPurgeTransaction,
    pub consistency_passes: Vec<NativeReductionFoundationConsistencyPassTransaction>,
    pub remaining_exclusions: NativeReductionExclusionTransaction,
    pub late_consistency: NativeReductionFoundationLateConsistencyTransaction,
    pub outer_deleted_vertices: Vec<NativeSigVertexId>,
    pub outer_reduced_vertices: Vec<NativeSigVertexId>,
    pub requires_outer_repeat: bool,
}

/// Complete Java foundations reduction through its terminal outer epoch.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeReductionFoundationsTransaction {
    pub system_id: usize,
    pub first_epoch: NativeReductionFoundationEpochTransaction,
    pub continuation_epochs: Vec<NativeReductionFoundationContinuationEpochTransaction>,
    /// Java `allRemoved`, in insertion order, from only the outer-local sets.
    pub all_removed_vertices: Vec<NativeSigVertexId>,
}

/// Why Java selected the line used to project one refined stem endpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeReductionReliableStemLineSource {
    Median,
    SkewedVertical,
}

/// One Java `StemInter.refineHeadEnd()` mutation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeReductionStemHeadEndRefinement {
    pub stem: NativeSigVertexId,
    pub direction: i8,
    pub leading_head: NativeSigVertexId,
    pub head_stem_relation: NativeSigEdgeId,
    pub head_side: NativeStemHeadSide,
    pub vertical_side: NativeStemVerticalSide,
    pub reference_point: NativeStemPoint,
    pub reliable_line_source: NativeReductionReliableStemLineSource,
    pub reliable_line: NativeStemLine,
    pub median_before: NativeStemLine,
    pub median_after: NativeStemLine,
    pub bounds_before: NativeSigBounds,
    pub bounds_after: NativeSigBounds,
}

/// Complete enabled stem-head-end refinement for one system.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeReductionStemHeadEndTransaction {
    pub system_id: usize,
    pub stem_order: Vec<NativeSigVertexId>,
    pub no_head_stems: Vec<NativeSigVertexId>,
    pub refinements: Vec<NativeReductionStemHeadEndRefinement>,
}

/// One call to Java `BeamGroupInter.sortedBeamsAround()` during epilog checks.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeReductionBeamGroupCheck {
    pub group: NativeSigVertexId,
    pub beam: NativeSigVertexId,
    pub siblings: Vec<NativeSigVertexId>,
    pub beam_index: Option<usize>,
    pub previous_beam: Option<NativeSigVertexId>,
    pub common_concrete_stems: Vec<NativeSigVertexId>,
    pub split_group: Option<NativeSigVertexId>,
}

/// Exact graph mutation performed by Java `SplitterOnSpace.process()`.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeReductionBeamGroupSplit {
    pub original_group: NativeSigVertexId,
    pub alien_group: NativeSigVertexId,
    pub upper_beam: NativeSigVertexId,
    pub lower_beam: NativeSigVertexId,
    pub moved_beams: Vec<NativeSigVertexId>,
    pub removed_containments: Vec<NativeSigEdgeId>,
    pub added_containments: Vec<NativeSigEdgeId>,
    pub added_beam_supports: Vec<NativeSigEdgeId>,
    pub removed_cross_stem_relations: Vec<NativeSigEdgeId>,
    pub removed_cross_beam_relations: Vec<NativeSigEdgeId>,
}

/// Complete Java beam-group consistency epilog for one system.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeReductionBeamGroupTransaction {
    pub system_id: usize,
    pub initial_groups: Vec<NativeSigVertexId>,
    pub checks: Vec<NativeReductionBeamGroupCheck>,
    pub splits: Vec<NativeReductionBeamGroupSplit>,
}

/// Why Java `StemInter.getFreeLength()` returned `null` for one live stem.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeReductionStemFreeLengthSkip {
    BeamAttached,
    NoHeads,
}

/// One non-null Java `StemInter.getFreeLength()` result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeReductionStemFreeLength {
    pub system_id: usize,
    pub stem: NativeSigVertexId,
    pub direction: i8,
    pub last_head: NativeSigVertexId,
    pub head_stem_relation: NativeSigEdgeId,
    pub head_side: NativeStemHeadSide,
    pub reference_vertical_side: NativeStemVerticalSide,
    pub reference_point: NativeStemPoint,
    pub stem_end: NativeStemPoint,
    pub pixels: i32,
}

/// Per-system stem scan in Java SIG insertion order.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeReductionSystemStemFreeLengths {
    pub system_id: usize,
    pub stem_order: Vec<NativeSigVertexId>,
    pub skips: Vec<(NativeSigVertexId, NativeReductionStemFreeLengthSkip)>,
    pub lengths: Vec<NativeReductionStemFreeLength>,
}

/// Sheet-wide collection and Java upper-middle median.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeReductionStemFreeLengthTransaction {
    pub systems: Vec<NativeReductionSystemStemFreeLengths>,
    pub sorted_lengths: Vec<i32>,
    pub median: Option<crate::reduction_step::StemFreeLengthMedian>,
}

/// Exact head anchor lookup keyed by SIG head, horizontal side, vertical side.
pub type NativeReductionHeadAnchorMap = BTreeMap<
    (
        NativeSigVertexId,
        NativeStemHeadSide,
        NativeStemVerticalSide,
    ),
    NativeStemPoint,
>;

/// Compatibility name retained while callers migrate from Boundary 288.
pub type NativeReductionFoundationPrefixTransaction =
    NativeReductionFoundationFixedPointTransaction;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeReductionFoundationPrefixError {
    Graph(NativeSigError),
    Overlap(NativeReductionOverlapError),
    StemEnding(NativeReductionStemEndingError),
    MissingSystem(usize),
    MissingStemMedian {
        system_id: usize,
        stem: NativeSigVertexId,
    },
    MissingHeadIdentity {
        system_id: usize,
        head: NativeSigVertexId,
    },
    MissingLedgerStaff {
        system_id: usize,
        staff_id: usize,
    },
    UnsupportedHeadShape {
        system_id: usize,
        head: NativeSigVertexId,
    },
    InvalidStemRefinementContext {
        system_id: usize,
    },
    MissingStemThickness {
        system_id: usize,
        stem: NativeSigVertexId,
    },
    MissingStemHeadAnchor {
        system_id: usize,
        head: NativeSigVertexId,
        horizontal: NativeStemHeadSide,
        vertical: NativeStemVerticalSide,
    },
    InvalidRefinedStemGeometry {
        system_id: usize,
        stem: NativeSigVertexId,
    },
    MissingBeamGeometry {
        system_id: usize,
        beam: NativeSigVertexId,
    },
    InvalidBeamGroupGeometry {
        system_id: usize,
        beam: NativeSigVertexId,
    },
    InvalidStemFreeLength {
        system_id: usize,
        stem: NativeSigVertexId,
    },
}

impl fmt::Display for NativeReductionFoundationPrefixError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Graph(source) => write!(formatter, "REDUCTION foundation-prefix graph: {source}"),
            Self::Overlap(source) => {
                write!(formatter, "REDUCTION foundation-prefix overlap: {source}")
            }
            Self::StemEnding(source) => {
                write!(
                    formatter,
                    "REDUCTION foundation-prefix stem ending: {source}"
                )
            }
            Self::MissingSystem(system_id) => {
                write!(formatter, "missing terminal STEMS system {system_id}")
            }
            Self::MissingStemMedian { system_id, stem } => write!(
                formatter,
                "REDUCTION system {system_id} chord analysis has no median for stem {}",
                stem.0
            ),
            Self::MissingHeadIdentity { system_id, head } => write!(
                formatter,
                "REDUCTION system {system_id} head check has no staff/pitch identity for head {}",
                head.0
            ),
            Self::MissingLedgerStaff {
                system_id,
                staff_id,
            } => write!(
                formatter,
                "REDUCTION system {system_id} ledger check has no staff geometry {staff_id}"
            ),
            Self::UnsupportedHeadShape { system_id, head } => write!(
                formatter,
                "REDUCTION system {system_id} chord analysis has unsupported head shape at {}",
                head.0
            ),
            Self::InvalidStemRefinementContext { system_id } => write!(
                formatter,
                "REDUCTION system {system_id} has invalid stem-refinement scale or skew"
            ),
            Self::MissingStemThickness { system_id, stem } => write!(
                formatter,
                "REDUCTION system {system_id} has no thickness for refined stem {}",
                stem.0
            ),
            Self::MissingStemHeadAnchor {
                system_id,
                head,
                horizontal,
                vertical,
            } => write!(
                formatter,
                "REDUCTION system {system_id} has no {horizontal:?}/{vertical:?} anchor for head {}",
                head.0
            ),
            Self::InvalidRefinedStemGeometry { system_id, stem } => write!(
                formatter,
                "REDUCTION system {system_id} produced invalid refined geometry for stem {}",
                stem.0
            ),
            Self::MissingBeamGeometry { system_id, beam } => write!(
                formatter,
                "REDUCTION system {system_id} has no geometry for beam {}",
                beam.0
            ),
            Self::InvalidBeamGroupGeometry { system_id, beam } => write!(
                formatter,
                "REDUCTION system {system_id} produced invalid beam-group geometry around beam {}",
                beam.0
            ),
            Self::InvalidStemFreeLength { system_id, stem } => write!(
                formatter,
                "REDUCTION system {system_id} produced invalid free length for stem {}",
                stem.0
            ),
        }
    }
}

impl Error for NativeReductionFoundationPrefixError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Graph(source) => Some(source),
            Self::Overlap(source) => Some(source),
            Self::StemEnding(source) => Some(source),
            Self::MissingSystem(_)
            | Self::MissingStemMedian { .. }
            | Self::MissingHeadIdentity { .. }
            | Self::MissingLedgerStaff { .. }
            | Self::UnsupportedHeadShape { .. }
            | Self::InvalidStemRefinementContext { .. }
            | Self::MissingStemThickness { .. }
            | Self::MissingStemHeadAnchor { .. }
            | Self::InvalidRefinedStemGeometry { .. }
            | Self::MissingBeamGeometry { .. }
            | Self::InvalidBeamGroupGeometry { .. }
            | Self::InvalidStemFreeLength { .. } => None,
        }
    }
}

impl From<NativeSigError> for NativeReductionFoundationPrefixError {
    fn from(source: NativeSigError) -> Self {
        Self::Graph(source)
    }
}

impl From<NativeReductionOverlapError> for NativeReductionFoundationPrefixError {
    fn from(source: NativeReductionOverlapError) -> Self {
        Self::Overlap(source)
    }
}

impl From<NativeReductionStemEndingError> for NativeReductionFoundationPrefixError {
    fn from(source: NativeReductionStemEndingError) -> Self {
        Self::StemEnding(source)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeReductionStemEndingError {
    Graph(NativeSigError),
    MissingStemMedian {
        system_id: usize,
        stem: NativeSigVertexId,
    },
}

impl fmt::Display for NativeReductionStemEndingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Graph(source) => write!(formatter, "REDUCTION stem-ending graph: {source}"),
            Self::MissingStemMedian { system_id, stem } => write!(
                formatter,
                "REDUCTION system {system_id} stem {} has no terminal median",
                stem.0
            ),
        }
    }
}

impl Error for NativeReductionStemEndingError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Graph(source) => Some(source),
            Self::MissingStemMedian { .. } => None,
        }
    }
}

impl From<NativeSigError> for NativeReductionStemEndingError {
    fn from(source: NativeSigError) -> Self {
        Self::Graph(source)
    }
}

/// Java `Grades.minContextualGrade`.
pub const MIN_REDUCTION_CONTEXTUAL_GRADE: f64 = 0.5;

/// Java `SigReducer.Constants.minIou` broad-phase threshold.
pub const MIN_REDUCTION_OVERLAP_IOU: f64 = 0.05;

/// Precise geometry which the recognition-owned SIG deliberately does not
/// flatten into rectangles.
///
/// Java's `AbstractInter.overlaps()` dispatches across glyph run tables,
/// areas, ensembles, staff/pitch-aware heads, and bounds.  The REDUCTION
/// scheduler owns when these questions are asked; the production adapter
/// resolves them from retained stage products without weakening them to box
/// tests.
pub trait NativeReductionOverlapGeometry {
    /// Whether `right` belongs to the mirror entity set built for `left`.
    fn is_mirror_entity(
        &mut self,
        left: NativeSigVertexId,
        right: NativeSigVertexId,
    ) -> Result<bool, NativeReductionOverlapGeometryError>;

    /// Java's mutual `left.overlaps(right) && right.overlaps(left)` test.
    fn mutually_overlaps(
        &mut self,
        left: NativeSigVertexId,
        right: NativeSigVertexId,
    ) -> Result<bool, NativeReductionOverlapGeometryError>;
}

/// One absolute point in a retained Java `Area` boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeReductionAreaPoint {
    pub x: f64,
    pub y: f64,
}

/// Lossless foreground ownership of one Java `Glyph`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeReductionGlyphGeometry {
    pub left: i32,
    pub top: i32,
    pub run_table: RunTable,
}

/// A Java `Area` represented as a union of convex path components.
///
/// Every currently scanned native area is a straight horizontal/vertical
/// ribbon, or a bracket ribbon plus rectangular serifs, so convex components
/// preserve the exact non-empty-intersection question without rasterization.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeReductionAreaGeometry {
    pub components: Vec<Vec<NativeReductionAreaPoint>>,
}

/// Head-only state used by Java `HeadInter.overlaps(HeadInter)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeReductionHeadGeometry {
    pub staff_id: Option<usize>,
    pub integer_pitch: i32,
}

/// Complete overlap evidence for one live interpretation.
///
/// An explicit record with neither glyph nor area is meaningful: it is the
/// Java bounds-only branch. A missing record is an error, never a rectangle
/// fallback.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeReductionInterGeometry {
    pub bounds: crate::native_sig::NativeSigBounds,
    pub core_bounds: crate::native_sig::NativeSigBounds,
    pub implicit: bool,
    pub glyph: Option<NativeReductionGlyphGeometry>,
    pub area: Option<NativeReductionAreaGeometry>,
    pub head: Option<NativeReductionHeadGeometry>,
    pub ensemble_members: Vec<NativeSigVertexId>,
}

impl NativeReductionInterGeometry {
    #[must_use]
    pub const fn bounds_only(bounds: crate::native_sig::NativeSigBounds) -> Self {
        Self {
            bounds,
            core_bounds: bounds,
            implicit: false,
            glyph: None,
            area: None,
            head: None,
            ensemble_members: Vec::new(),
        }
    }

    #[must_use]
    pub fn head(
        bounds: crate::native_sig::NativeSigBounds,
        glyph: NativeReductionGlyphGeometry,
        staff_id: Option<usize>,
        integer_pitch: i32,
    ) -> Self {
        Self {
            bounds,
            core_bounds: shrunk_head_core_bounds(bounds),
            implicit: false,
            glyph: Some(glyph),
            area: None,
            head: Some(NativeReductionHeadGeometry {
                staff_id,
                integer_pitch,
            }),
            ensemble_members: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeReductionOverlapGeometryError {
    MissingSystem(usize),
    MissingGeometry(NativeSigVertexId),
    MissingStemGeometry {
        system_id: usize,
        stem: NativeSigVertexId,
    },
    InvalidArea {
        vertex: NativeSigVertexId,
        component: usize,
    },
    MissingEnsembleMember {
        ensemble: NativeSigVertexId,
        member: NativeSigVertexId,
    },
}

impl fmt::Display for NativeReductionOverlapGeometryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSystem(system_id) => {
                write!(formatter, "missing terminal STEMS system {system_id}")
            }
            Self::MissingGeometry(vertex) => {
                write!(
                    formatter,
                    "missing exact overlap geometry for vertex {}",
                    vertex.0
                )
            }
            Self::MissingStemGeometry { system_id, stem } => write!(
                formatter,
                "terminal STEMS system {system_id} has no geometry for stem {}",
                stem.0
            ),
            Self::InvalidArea { vertex, component } => write!(
                formatter,
                "vertex {} has invalid overlap area component {component}",
                vertex.0
            ),
            Self::MissingEnsembleMember { ensemble, member } => write!(
                formatter,
                "overlap ensemble {} has missing member {}",
                ensemble.0, member.0
            ),
        }
    }
}

impl Error for NativeReductionOverlapGeometryError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeReductionOverlapError {
    Graph(NativeSigError),
    Geometry(NativeReductionOverlapGeometryError),
}

impl fmt::Display for NativeReductionOverlapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Graph(source) => write!(formatter, "REDUCTION overlap graph: {source}"),
            Self::Geometry(source) => write!(formatter, "REDUCTION overlap geometry: {source}"),
        }
    }
}

impl Error for NativeReductionOverlapError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Graph(source) => Some(source),
            Self::Geometry(source) => Some(source),
        }
    }
}

impl From<NativeSigError> for NativeReductionOverlapError {
    fn from(source: NativeSigError) -> Self {
        Self::Graph(source)
    }
}

impl From<NativeReductionOverlapGeometryError> for NativeReductionOverlapError {
    fn from(source: NativeReductionOverlapGeometryError) -> Self {
        Self::Geometry(source)
    }
}

/// Exact, fail-closed implementation of Java `AbstractInter.overlaps` and
/// `HeadInter.overlaps` over retained glyph/area evidence.
#[derive(Clone, Debug, Default)]
pub struct NativeReductionLosslessOverlapResolver {
    geometry: BTreeMap<NativeSigVertexId, NativeReductionInterGeometry>,
    mirror_pairs: BTreeSet<(NativeSigVertexId, NativeSigVertexId)>,
    support_pairs: BTreeSet<(NativeSigVertexId, NativeSigVertexId)>,
}

impl NativeReductionLosslessOverlapResolver {
    #[must_use]
    pub fn new(
        geometry: impl IntoIterator<Item = (NativeSigVertexId, NativeReductionInterGeometry)>,
    ) -> Self {
        Self {
            geometry: geometry.into_iter().collect(),
            mirror_pairs: BTreeSet::new(),
            support_pairs: BTreeSet::new(),
        }
    }

    pub fn add_mirror_pair(&mut self, one: NativeSigVertexId, two: NativeSigVertexId) {
        self.mirror_pairs.insert(normalized_vertex_pair(one, two));
    }

    pub fn add_support_pair(&mut self, one: NativeSigVertexId, two: NativeSigVertexId) {
        self.support_pairs.insert(normalized_vertex_pair(one, two));
    }

    fn evidence(
        &self,
        vertex: NativeSigVertexId,
    ) -> Result<&NativeReductionInterGeometry, NativeReductionOverlapGeometryError> {
        self.geometry
            .get(&vertex)
            .ok_or(NativeReductionOverlapGeometryError::MissingGeometry(vertex))
    }

    fn directional_overlaps(
        &self,
        this_id: NativeSigVertexId,
        that_id: NativeSigVertexId,
    ) -> Result<bool, NativeReductionOverlapGeometryError> {
        let this = self.evidence(this_id)?;
        let that = self.evidence(that_id)?;
        if this.implicit
            || that.implicit
            || !rectangles_intersect(this.core_bounds, that.core_bounds)
        {
            return Ok(false);
        }
        if let (Some(this_head), Some(that_head)) = (this.head, that.head) {
            return Ok(heads_overlap(this, this_head, that, that_head));
        }
        if !this.ensemble_members.is_empty() {
            if this.ensemble_members.contains(&that_id) {
                return Ok(false);
            }
            for &member in &this.ensemble_members {
                self.evidence(member).map_err(|error| match error {
                    NativeReductionOverlapGeometryError::MissingGeometry(_) => {
                        NativeReductionOverlapGeometryError::MissingEnsembleMember {
                            ensemble: this_id,
                            member,
                        }
                    }
                    other => other,
                })?;
                if self.directional_overlaps(member, that_id)?
                    && self.directional_overlaps(that_id, member)?
                    && !self
                        .support_pairs
                        .contains(&normalized_vertex_pair(member, that_id))
                {
                    return Ok(true);
                }
            }
            return Ok(false);
        }
        if let (Some(one), Some(two)) = (&this.glyph, &that.glyph) {
            return Ok(glyphs_intersect(one, two));
        }
        if let Some(area) = &this.area {
            validate_area(this_id, area)?;
            if let Some(that_area) = &that.area {
                validate_area(that_id, that_area)?;
                return Ok(areas_intersect(area, that_area));
            }
            if let Some(glyph) = &that.glyph {
                return Ok(glyph_intersects_area(glyph, area));
            }
            return Ok(area_intersects_rectangle(area, that.bounds));
        }
        if let Some(glyph) = &this.glyph {
            return Ok(glyph_intersects_rectangle(glyph, that.bounds));
        }
        if let Some(glyph) = &that.glyph {
            return Ok(glyph_intersects_rectangle(glyph, this.bounds));
        }
        Ok(true)
    }
}

impl NativeReductionOverlapGeometry for NativeReductionLosslessOverlapResolver {
    fn is_mirror_entity(
        &mut self,
        left: NativeSigVertexId,
        right: NativeSigVertexId,
    ) -> Result<bool, NativeReductionOverlapGeometryError> {
        self.evidence(left)?;
        self.evidence(right)?;
        Ok(self
            .mirror_pairs
            .contains(&normalized_vertex_pair(left, right)))
    }

    fn mutually_overlaps(
        &mut self,
        left: NativeSigVertexId,
        right: NativeSigVertexId,
    ) -> Result<bool, NativeReductionOverlapGeometryError> {
        Ok(self.directional_overlaps(left, right)? && self.directional_overlaps(right, left)?)
    }
}

/// Join terminal STEMS identities to every exact overlap artifact retained by
/// its immutable predecessor products and mutable stem registry.
pub fn native_stems_lossless_overlap_resolver(
    stems: &NativeStemsRecognition,
    system_id: usize,
) -> Result<NativeReductionLosslessOverlapResolver, NativeReductionOverlapGeometryError> {
    let system = stems
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .ok_or(NativeReductionOverlapGeometryError::MissingSystem(
            system_id,
        ))?;
    let carrier = &system.transaction.state_after;
    let sig = &carrier.beam_state.sig;
    let bindings = &carrier.beam_state.bindings;
    let known_stems = &carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems;
    let mut geometry = bindings.overlap_geometry.clone();
    for (&stem_identity, &vertex) in &bindings.stem_vertices {
        let stem = known_stems
            .iter()
            .find(|stem| stem.stem_identity == stem_identity && stem.sig_attached)
            .ok_or(NativeReductionOverlapGeometryError::MissingStemGeometry {
                system_id,
                stem: vertex,
            })?;
        let bounds = sig
            .vertex(vertex.0)
            .ok_or(NativeReductionOverlapGeometryError::MissingStemGeometry {
                system_id,
                stem: vertex,
            })?
            .bounds;
        geometry.insert(
            vertex,
            NativeReductionInterGeometry {
                bounds,
                core_bounds: bounds,
                implicit: false,
                glyph: Some(NativeReductionGlyphGeometry {
                    left: i32::try_from(stem.glyph_content.bounds.x).map_err(|_| {
                        NativeReductionOverlapGeometryError::MissingStemGeometry {
                            system_id,
                            stem: vertex,
                        }
                    })?,
                    top: i32::try_from(stem.glyph_content.bounds.y).map_err(|_| {
                        NativeReductionOverlapGeometryError::MissingStemGeometry {
                            system_id,
                            stem: vertex,
                        }
                    })?,
                    run_table: stem.glyph_content.run_table.clone(),
                }),
                area: Some(vertical_line_area(
                    stem.geometry.median.start.x,
                    stem.geometry.median.start.y,
                    stem.geometry.median.stop.x,
                    stem.geometry.median.stop.y,
                    stem.geometry.mean_thickness,
                )),
                head: None,
                ensemble_members: Vec::new(),
            },
        );
    }

    for vertex in sig.vertices.iter().filter(|vertex| {
        vertex.active && !is_overlap_disabled(vertex.kind) && !is_header_inter(vertex.kind)
    }) {
        let id = NativeSigVertexId(vertex.ordinal);
        if !geometry.contains_key(&id) {
            return Err(NativeReductionOverlapGeometryError::MissingGeometry(id));
        }
    }

    let mut resolver = NativeReductionLosslessOverlapResolver::new(geometry);
    for edge in sig
        .edges
        .iter()
        .filter(|edge| edge.active && edge.support.is_some())
    {
        resolver.add_support_pair(
            NativeSigVertexId(edge.source),
            NativeSigVertexId(edge.target),
        );
    }
    Ok(resolver)
}

/// Execute Java's overlap-discovery epoch directly against one owned terminal
/// STEMS SIG using only recognition-retained geometry.
pub fn detect_native_stems_reduction_overlaps(
    stems: &mut NativeStemsRecognition,
    system_id: usize,
) -> Result<NativeReductionOverlapTransaction, NativeReductionOverlapError> {
    let mut resolver = native_stems_lossless_overlap_resolver(stems, system_id)?;
    let system = stems
        .systems
        .iter_mut()
        .find(|system| system.system_id == system_id)
        .ok_or(NativeReductionOverlapGeometryError::MissingSystem(
            system_id,
        ))?;
    detect_native_reduction_overlaps(
        &mut system.transaction.state_after.beam_state.sig,
        &mut resolver,
    )
}

/// Execute Java foundations setup and the complete consistency fixed point
/// against terminal native STEMS state.
pub fn reduce_native_stems_foundation_fixed_point(
    stems: &mut NativeStemsRecognition,
    system_id: usize,
) -> Result<NativeReductionFoundationFixedPointTransaction, NativeReductionFoundationPrefixError> {
    let mut resolver = native_stems_lossless_overlap_resolver(stems, system_id)
        .map_err(NativeReductionOverlapError::from)?;
    let medians = native_stems_terminal_medians(stems, system_id)?;
    let (head_identities, merged_staff_pairs) = native_stems_head_identities(stems, system_id)?;
    let system = stems
        .systems
        .iter_mut()
        .find(|system| system.system_id == system_id)
        .ok_or(NativeReductionFoundationPrefixError::MissingSystem(
            system_id,
        ))?;
    let beam_state = &mut system.transaction.state_after.beam_state;
    reduce_native_foundation_fixed_point(
        &mut beam_state.sig,
        &mut beam_state.bindings,
        &mut resolver,
        &medians,
        &head_identities,
        &merged_staff_pairs,
    )
}

/// Execute the first Java foundations epoch through remaining exclusions and
/// its single zero-returning late-consistency invocation.
pub fn reduce_native_stems_foundation_epoch(
    stems: &mut NativeStemsRecognition,
    system_id: usize,
) -> Result<NativeReductionFoundationEpochTransaction, NativeReductionFoundationPrefixError> {
    let mut resolver = native_stems_lossless_overlap_resolver(stems, system_id)
        .map_err(NativeReductionOverlapError::from)?;
    let medians = native_stems_terminal_medians(stems, system_id)?;
    let (head_identities, merged_staff_pairs) = native_stems_head_identities(stems, system_id)?;
    let system = stems
        .systems
        .iter_mut()
        .find(|system| system.system_id == system_id)
        .ok_or(NativeReductionFoundationPrefixError::MissingSystem(
            system_id,
        ))?;
    let beam_state = &mut system.transaction.state_after.beam_state;
    reduce_native_foundation_epoch(
        &mut beam_state.sig,
        &mut beam_state.bindings,
        &mut resolver,
        &medians,
        &head_identities,
        &merged_staff_pairs,
    )
}

/// Execute the complete Java foundations reducer, including every repeated
/// outer epoch and the final epoch whose local sets are both empty.
pub fn reduce_native_stems_foundations(
    stems: &mut NativeStemsRecognition,
    system_id: usize,
) -> Result<NativeReductionFoundationsTransaction, NativeReductionFoundationPrefixError> {
    let mut resolver = native_stems_lossless_overlap_resolver(stems, system_id)
        .map_err(NativeReductionOverlapError::from)?;
    let medians = native_stems_terminal_medians(stems, system_id)?;
    let (head_identities, merged_staff_pairs) = native_stems_head_identities(stems, system_id)?;
    let system = stems
        .systems
        .iter_mut()
        .find(|system| system.system_id == system_id)
        .ok_or(NativeReductionFoundationPrefixError::MissingSystem(
            system_id,
        ))?;
    let beam_state = &mut system.transaction.state_after.beam_state;
    reduce_native_foundations(
        &mut beam_state.sig,
        &mut beam_state.bindings,
        &mut resolver,
        &medians,
        &head_identities,
        &merged_staff_pairs,
    )
}

/// Boundary-288 compatibility entry point. It now returns the fixed-point
/// transaction rather than stopping after one consistency invocation.
pub fn reduce_native_stems_foundation_prefix(
    stems: &mut NativeStemsRecognition,
    system_id: usize,
) -> Result<NativeReductionFoundationPrefixTransaction, NativeReductionFoundationPrefixError> {
    reduce_native_stems_foundation_fixed_point(stems, system_id)
}

/// Dependency-light foundations setup and fixed point used by production and
/// synthetic order/cascade tests.
pub fn reduce_native_foundation_fixed_point(
    sig: &mut NativeSigSystem,
    bindings: &mut NativeSigSystemBindings,
    geometry: &mut impl NativeReductionOverlapGeometry,
    stem_medians: &BTreeMap<NativeSigVertexId, NativeStemLine>,
    head_identities: &BTreeMap<NativeSigVertexId, NativeReductionHeadIdentity>,
    merged_staff_pairs: &[(usize, usize)],
) -> Result<NativeReductionFoundationFixedPointTransaction, NativeReductionFoundationPrefixError> {
    let overlap = detect_native_reduction_overlaps(sig, geometry)?;
    // AdapterForFoundations.checkFrozens() is the inherited no-op.
    let pre_prolog_contextualization = sig.contextualize();
    let chord_analysis = analyze_native_foundation_chords(sig, stem_medians)?;
    let initial_weak_purge = contextualize_and_purge_native_weaks(sig)?;
    // AdapterForFoundations.checkSlurs() is the inherited empty set.
    let consistency_passes = run_native_foundation_consistency_fixed_point(
        sig,
        bindings,
        stem_medians,
        head_identities,
        merged_staff_pairs,
    )?;
    Ok(NativeReductionFoundationFixedPointTransaction {
        system_id: sig.system_id,
        overlap,
        pre_prolog_contextualization,
        chord_analysis,
        initial_weak_purge,
        consistency_passes,
    })
}

/// Boundary-288 compatibility entry point for dependency-light callers.
pub fn reduce_native_foundation_prefix(
    sig: &mut NativeSigSystem,
    bindings: &mut NativeSigSystemBindings,
    geometry: &mut impl NativeReductionOverlapGeometry,
    stem_medians: &BTreeMap<NativeSigVertexId, NativeStemLine>,
    head_identities: &BTreeMap<NativeSigVertexId, NativeReductionHeadIdentity>,
    merged_staff_pairs: &[(usize, usize)],
) -> Result<NativeReductionFoundationPrefixTransaction, NativeReductionFoundationPrefixError> {
    reduce_native_foundation_fixed_point(
        sig,
        bindings,
        geometry,
        stem_medians,
        head_identities,
        merged_staff_pairs,
    )
}

/// Dependency-light first Java foundations epoch.
pub fn reduce_native_foundation_epoch(
    sig: &mut NativeSigSystem,
    bindings: &mut NativeSigSystemBindings,
    geometry: &mut impl NativeReductionOverlapGeometry,
    stem_medians: &BTreeMap<NativeSigVertexId, NativeStemLine>,
    head_identities: &BTreeMap<NativeSigVertexId, NativeReductionHeadIdentity>,
    merged_staff_pairs: &[(usize, usize)],
) -> Result<NativeReductionFoundationEpochTransaction, NativeReductionFoundationPrefixError> {
    let fixed_point = reduce_native_foundation_fixed_point(
        sig,
        bindings,
        geometry,
        stem_medians,
        head_identities,
        merged_staff_pairs,
    )?;
    let remaining_exclusions = reduce_native_sig_exclusions(sig)?;
    let late_consistency = reduce_native_foundation_late_consistency(sig, stem_medians)?;
    let outer_deleted_vertices = fixed_point.initial_weak_purge.removed_vertices.clone();
    let outer_reduced_vertices = remaining_exclusions.removed_vertices.clone();
    let requires_outer_repeat =
        !outer_deleted_vertices.is_empty() || !outer_reduced_vertices.is_empty();
    Ok(NativeReductionFoundationEpochTransaction {
        system_id: sig.system_id,
        fixed_point,
        remaining_exclusions,
        late_consistency,
        outer_deleted_vertices,
        outer_reduced_vertices,
        requires_outer_repeat,
    })
}

/// Dependency-light complete foundations reducer.
pub fn reduce_native_foundations(
    sig: &mut NativeSigSystem,
    bindings: &mut NativeSigSystemBindings,
    geometry: &mut impl NativeReductionOverlapGeometry,
    stem_medians: &BTreeMap<NativeSigVertexId, NativeStemLine>,
    head_identities: &BTreeMap<NativeSigVertexId, NativeReductionHeadIdentity>,
    merged_staff_pairs: &[(usize, usize)],
) -> Result<NativeReductionFoundationsTransaction, NativeReductionFoundationPrefixError> {
    let first_epoch = reduce_native_foundation_epoch(
        sig,
        bindings,
        geometry,
        stem_medians,
        head_identities,
        merged_staff_pairs,
    )?;
    let mut all_removed_vertices = Vec::new();
    for &vertex in &first_epoch.outer_deleted_vertices {
        push_unique(&mut all_removed_vertices, vertex);
    }
    for &vertex in &first_epoch.outer_reduced_vertices {
        push_unique(&mut all_removed_vertices, vertex);
    }
    let mut requires_outer_repeat = first_epoch.requires_outer_repeat;
    let mut continuation_epochs = Vec::new();
    while requires_outer_repeat {
        let epoch = reduce_native_foundation_continuation_epoch(
            sig,
            bindings,
            stem_medians,
            head_identities,
            merged_staff_pairs,
        )?;
        for &vertex in &epoch.outer_deleted_vertices {
            push_unique(&mut all_removed_vertices, vertex);
        }
        for &vertex in &epoch.outer_reduced_vertices {
            push_unique(&mut all_removed_vertices, vertex);
        }
        requires_outer_repeat = epoch.requires_outer_repeat;
        continuation_epochs.push(epoch);
    }
    Ok(NativeReductionFoundationsTransaction {
        system_id: sig.system_id,
        first_epoch,
        continuation_epochs,
        all_removed_vertices,
    })
}

/// Execute Java's enabled `StemInter.refineHeadEnd()` loop against terminal
/// native STEMS ownership after foundations reduction.
pub fn refine_native_stems_head_ends(
    stems: &mut NativeStemsRecognition,
    system_id: usize,
) -> Result<NativeReductionStemHeadEndTransaction, NativeReductionFoundationPrefixError> {
    let system_index = stems
        .systems
        .iter()
        .position(|system| system.system_id == system_id)
        .ok_or(NativeReductionFoundationPrefixError::MissingSystem(
            system_id,
        ))?;
    let head_system = stems
        .components
        .head_corners
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .ok_or(NativeReductionFoundationPrefixError::MissingSystem(
            system_id,
        ))?;
    let beam_state = &stems.systems[system_index]
        .transaction
        .state_after
        .beam_state;
    let bindings = &beam_state.bindings;
    let known_stems = &beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems;
    let mut medians = BTreeMap::new();
    let mut thicknesses = BTreeMap::new();
    for (&identity, &vertex) in &bindings.stem_vertices {
        let stem = known_stems
            .iter()
            .find(|stem| stem.stem_identity == identity && stem.sig_attached)
            .ok_or(NativeReductionFoundationPrefixError::MissingStemMedian {
                system_id,
                stem: vertex,
            })?;
        medians.insert(vertex, stem.geometry.median);
        thicknesses.insert(vertex, stem.geometry.mean_thickness);
    }
    let mut anchors = NativeReductionHeadAnchorMap::new();
    for head in &head_system.heads_in_sig_order {
        let Some(&vertex) = bindings.head_vertices.get(&head.reference) else {
            continue;
        };
        for corner in &head.corners_in_constructor_order {
            anchors.insert(
                (vertex, corner.horizontal, corner.vertical),
                corner.reference,
            );
        }
    }

    let beam_state = &mut stems.systems[system_index]
        .transaction
        .state_after
        .beam_state;
    let transaction = refine_native_reduction_stem_head_ends(
        &mut beam_state.sig,
        &mut medians,
        &thicknesses,
        &anchors,
        stems.reduction_interline,
        stems.sheet_skew_slope,
    )?;
    for refinement in &transaction.refinements {
        let (&stem_identity, _) = beam_state
            .bindings
            .stem_vertices
            .iter()
            .find(|(_, vertex)| **vertex == refinement.stem)
            .ok_or(NativeReductionFoundationPrefixError::MissingStemMedian {
                system_id,
                stem: refinement.stem,
            })?;
        let stem = beam_state
            .latest_base_apply
            .transaction_state
            .system_stems
            .known_stems
            .iter_mut()
            .find(|stem| stem.stem_identity == stem_identity && stem.sig_attached)
            .ok_or(NativeReductionFoundationPrefixError::MissingStemMedian {
                system_id,
                stem: refinement.stem,
            })?;
        stem.geometry.median = refinement.median_after;
        stem.geometry.ribbon_bounds = crate::head_scanner_slices::JavaRectangle::new(
            refinement.bounds_after.x,
            refinement.bounds_after.y,
            refinement.bounds_after.width,
            refinement.bounds_after.height,
        );
    }
    Ok(transaction)
}

/// Dependency-light port of the enabled stem head-end refinement loop.
pub fn refine_native_reduction_stem_head_ends(
    sig: &mut NativeSigSystem,
    stem_medians: &mut BTreeMap<NativeSigVertexId, NativeStemLine>,
    stem_thicknesses: &BTreeMap<NativeSigVertexId, f64>,
    head_anchors: &NativeReductionHeadAnchorMap,
    interline: i32,
    sheet_skew_slope: f64,
) -> Result<NativeReductionStemHeadEndTransaction, NativeReductionFoundationPrefixError> {
    let mut shadow_sig = sig.clone();
    let mut shadow_medians = stem_medians.clone();
    let transaction = apply_native_reduction_stem_head_ends(
        &mut shadow_sig,
        &mut shadow_medians,
        stem_thicknesses,
        head_anchors,
        interline,
        sheet_skew_slope,
    )?;
    *sig = shadow_sig;
    *stem_medians = shadow_medians;
    Ok(transaction)
}

fn apply_native_reduction_stem_head_ends(
    sig: &mut NativeSigSystem,
    stem_medians: &mut BTreeMap<NativeSigVertexId, NativeStemLine>,
    stem_thicknesses: &BTreeMap<NativeSigVertexId, f64>,
    head_anchors: &NativeReductionHeadAnchorMap,
    interline: i32,
    sheet_skew_slope: f64,
) -> Result<NativeReductionStemHeadEndTransaction, NativeReductionFoundationPrefixError> {
    if interline <= 0 || !sheet_skew_slope.is_finite() {
        return Err(
            NativeReductionFoundationPrefixError::InvalidStemRefinementContext {
                system_id: sig.system_id,
            },
        );
    }
    sig.validate_integrity()?;
    let stem_order = sig
        .vertices
        .iter()
        .filter(|vertex| vertex.active && vertex.kind == NativeSigInterKind::Stem)
        .map(|vertex| NativeSigVertexId(vertex.ordinal))
        .collect::<Vec<_>>();
    let mut no_head_stems = Vec::new();
    let mut refinements = Vec::new();
    for &stem in &stem_order {
        let mut relations = active_head_stem_relations_to(sig, stem);
        if relations.is_empty() {
            no_head_stems.push(stem);
            continue;
        }
        relations.sort_by_key(|relation| {
            let head = sig.edges[relation.0].source;
            let bounds = sig.vertices[head].bounds;
            bounds.y.saturating_add(bounds.height / 2)
        });
        let direction = native_stem_direction(sig, stem, stem_medians)?;
        let relation = if direction > 0 {
            relations[0]
        } else {
            *relations.last().expect("nonempty head relations")
        };
        let edge = sig.edges[relation.0];
        let leading_head = NativeSigVertexId(edge.source);
        let payload = edge.head_stem.expect("validated HeadStem payload");
        let vertical_side = if direction > 0 {
            NativeStemVerticalSide::Bottom
        } else {
            NativeStemVerticalSide::Top
        };
        let reference_point = *head_anchors
            .get(&(leading_head, payload.head_side, vertical_side))
            .ok_or(
                NativeReductionFoundationPrefixError::MissingStemHeadAnchor {
                    system_id: sig.system_id,
                    head: leading_head,
                    horizontal: payload.head_side,
                    vertical: vertical_side,
                },
            )?;
        let median_before = *stem_medians.get(&stem).ok_or(
            NativeReductionFoundationPrefixError::MissingStemMedian {
                system_id: sig.system_id,
                stem,
            },
        )?;
        let bounds_before = sig.vertices[stem.0].bounds;
        let (reliable_line_source, reliable_line) = if bounds_before.height >= interline {
            (NativeReductionReliableStemLineSource::Median, median_before)
        } else {
            let center = NativeStemPoint {
                x: f64::from(bounds_before.x.saturating_add(bounds_before.width / 2)),
                y: f64::from(bounds_before.y.saturating_add(bounds_before.height / 2)),
            };
            (
                NativeReductionReliableStemLineSource::SkewedVertical,
                NativeStemLine {
                    start: center,
                    stop: NativeStemPoint {
                        x: center.x - (1000.0 * sheet_skew_slope),
                        y: center.y + 1000.0,
                    },
                },
            )
        };
        let cross = generic_intersection(
            stem_segment(reliable_line),
            Segment {
                x1: 0.0,
                y1: reference_point.y,
                x2: 1000.0,
                y2: reference_point.y,
            },
        );
        let median_after = if direction > 0 {
            NativeStemLine {
                start: cross,
                stop: median_before.stop,
            }
        } else {
            NativeStemLine {
                start: median_before.start,
                stop: cross,
            }
        };
        let thickness = *stem_thicknesses.get(&stem).ok_or(
            NativeReductionFoundationPrefixError::MissingStemThickness {
                system_id: sig.system_id,
                stem,
            },
        )?;
        let bounds =
            VerticalRibbonArea::new(stem_segment(median_after), thickness).integer_bounds();
        let bounds_after = NativeSigBounds {
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: bounds.height,
        };
        if ![
            median_after.start.x,
            median_after.start.y,
            median_after.stop.x,
            median_after.stop.y,
            thickness,
        ]
        .into_iter()
        .all(f64::is_finite)
            || median_after.start.y >= median_after.stop.y
            || thickness <= 0.0
            || bounds_after.width <= 0
            || bounds_after.height <= 0
        {
            return Err(
                NativeReductionFoundationPrefixError::InvalidRefinedStemGeometry {
                    system_id: sig.system_id,
                    stem,
                },
            );
        }
        stem_medians.insert(stem, median_after);
        sig.vertices[stem.0].bounds = bounds_after;
        refinements.push(NativeReductionStemHeadEndRefinement {
            stem,
            direction,
            leading_head,
            head_stem_relation: relation,
            head_side: payload.head_side,
            vertical_side,
            reference_point,
            reliable_line_source,
            reliable_line,
            median_before,
            median_after,
            bounds_before,
            bounds_after,
        });
    }
    sig.validate_integrity()?;
    Ok(NativeReductionStemHeadEndTransaction {
        system_id: sig.system_id,
        stem_order,
        no_head_stems,
        refinements,
    })
}

/// Execute Java's sheet-wide `StemInter.getFreeLength()` collection and
/// upper-middle median after beam-group checking.
pub fn measure_native_reduction_stem_free_lengths(
    stems: &NativeStemsRecognition,
) -> Result<NativeReductionStemFreeLengthTransaction, NativeReductionFoundationPrefixError> {
    if stems.reduction_interline <= 0 {
        return Err(
            NativeReductionFoundationPrefixError::InvalidStemRefinementContext { system_id: 0 },
        );
    }
    let mut systems = Vec::new();
    for system in &stems.systems {
        let system_id = system.system_id;
        let beam_state = &system.transaction.state_after.beam_state;
        let known_stems = &beam_state
            .latest_base_apply
            .transaction_state
            .system_stems
            .known_stems;
        let mut medians = BTreeMap::new();
        for (&identity, &vertex) in &beam_state.bindings.stem_vertices {
            let stem = known_stems
                .iter()
                .find(|stem| stem.stem_identity == identity && stem.sig_attached)
                .ok_or(NativeReductionFoundationPrefixError::MissingStemMedian {
                    system_id,
                    stem: vertex,
                })?;
            medians.insert(vertex, stem.geometry.median);
        }
        let head_system = stems
            .components
            .head_corners
            .systems
            .iter()
            .find(|heads| heads.system_id == system_id)
            .ok_or(NativeReductionFoundationPrefixError::MissingSystem(
                system_id,
            ))?;
        let mut anchors = NativeReductionHeadAnchorMap::new();
        for head in &head_system.heads_in_sig_order {
            let Some(&vertex) = beam_state.bindings.head_vertices.get(&head.reference) else {
                continue;
            };
            for corner in &head.corners_in_constructor_order {
                anchors.insert(
                    (vertex, corner.horizontal, corner.vertical),
                    corner.reference,
                );
            }
        }
        systems.push(measure_native_reduction_system_stem_free_lengths(
            &beam_state.sig,
            &medians,
            &anchors,
        )?);
    }
    let mut sorted_lengths = systems
        .iter()
        .flat_map(|system| system.lengths.iter().map(|length| length.pixels))
        .collect::<Vec<_>>();
    sorted_lengths.sort_unstable();
    let median = (!sorted_lengths.is_empty()).then(|| {
        let pixels = sorted_lengths[sorted_lengths.len() / 2];
        crate::reduction_step::StemFreeLengthMedian {
            pixels,
            interlines: f64::from(pixels) / f64::from(stems.reduction_interline),
        }
    });
    Ok(NativeReductionStemFreeLengthTransaction {
        systems,
        sorted_lengths,
        median,
    })
}

/// Dependency-light system kernel for Java `StemInter.getFreeLength()`.
pub fn measure_native_reduction_system_stem_free_lengths(
    sig: &NativeSigSystem,
    stem_medians: &BTreeMap<NativeSigVertexId, NativeStemLine>,
    head_anchors: &NativeReductionHeadAnchorMap,
) -> Result<NativeReductionSystemStemFreeLengths, NativeReductionFoundationPrefixError> {
    sig.validate_integrity()?;
    let stem_order = sig
        .vertices
        .iter()
        .filter(|vertex| vertex.active && vertex.kind == NativeSigInterKind::Stem)
        .map(|vertex| NativeSigVertexId(vertex.ordinal))
        .collect::<Vec<_>>();
    let mut skips = Vec::new();
    let mut lengths = Vec::new();
    for &stem in &stem_order {
        if sig.edges.iter().any(|edge| {
            edge.active
                && edge.kind == NativeSigRelationKind::BeamStem
                && (edge.source == stem.0 || edge.target == stem.0)
        }) {
            skips.push((stem, NativeReductionStemFreeLengthSkip::BeamAttached));
            continue;
        }
        let mut heads = active_head_stem_relations_to(sig, stem)
            .into_iter()
            .map(|relation| NativeSigVertexId(sig.edges[relation.0].source))
            .fold(Vec::new(), |mut heads, head| {
                if !heads.contains(&head) {
                    heads.push(head);
                }
                heads
            });
        if heads.is_empty() {
            skips.push((stem, NativeReductionStemFreeLengthSkip::NoHeads));
            continue;
        }
        heads.sort_by_key(|head| {
            let bounds = sig.vertices[head.0].bounds;
            bounds.y.saturating_add(bounds.height / 2)
        });
        let direction = native_stem_direction(sig, stem, stem_medians)?;
        let last_head = if direction < 0 {
            heads[0]
        } else {
            *heads.last().expect("nonempty heads")
        };
        let head_stem_relation = active_head_stem_relations_to(sig, stem)
            .into_iter()
            .find(|relation| sig.edges[relation.0].source == last_head.0)
            .expect("head came from a live HeadStem relation");
        let payload = sig.edges[head_stem_relation.0]
            .head_stem
            .expect("validated HeadStem payload");
        let reference_vertical_side = if direction < 0 {
            NativeStemVerticalSide::Bottom
        } else {
            NativeStemVerticalSide::Top
        };
        let reference_point = *head_anchors
            .get(&(last_head, payload.head_side, reference_vertical_side))
            .ok_or(
                NativeReductionFoundationPrefixError::MissingStemHeadAnchor {
                    system_id: sig.system_id,
                    head: last_head,
                    horizontal: payload.head_side,
                    vertical: reference_vertical_side,
                },
            )?;
        let median = *stem_medians.get(&stem).ok_or(
            NativeReductionFoundationPrefixError::MissingStemMedian {
                system_id: sig.system_id,
                stem,
            },
        )?;
        let stem_end = if direction < 0 {
            median.start
        } else {
            median.stop
        };
        let raw_length = (stem_end.y - reference_point.y).abs();
        if !raw_length.is_finite() || raw_length > f64::from(i32::MAX) {
            return Err(
                NativeReductionFoundationPrefixError::InvalidStemFreeLength {
                    system_id: sig.system_id,
                    stem,
                },
            );
        }
        lengths.push(NativeReductionStemFreeLength {
            system_id: sig.system_id,
            stem,
            direction,
            last_head,
            head_stem_relation,
            head_side: payload.head_side,
            reference_vertical_side,
            reference_point,
            stem_end,
            pixels: raw_length.round_ties_even() as i32,
        });
    }
    Ok(NativeReductionSystemStemFreeLengths {
        system_id: sig.system_id,
        stem_order,
        skips,
        lengths,
    })
}

/// Execute Java `BeamGroupInter.checkBeamGroups(system)` against one terminal
/// production STEMS SIG after stem head-end refinement.
pub fn check_native_reduction_beam_groups(
    stems: &mut NativeStemsRecognition,
    system_id: usize,
) -> Result<NativeReductionBeamGroupTransaction, NativeReductionFoundationPrefixError> {
    let system_index = stems
        .systems
        .iter()
        .position(|system| system.system_id == system_id)
        .ok_or(NativeReductionFoundationPrefixError::MissingSystem(
            system_id,
        ))?;
    let beam_state = &stems.systems[system_index]
        .transaction
        .state_after
        .beam_state;
    let mut stem_medians = BTreeMap::new();
    for (&identity, &vertex) in &beam_state.bindings.stem_vertices {
        let stem = beam_state
            .latest_base_apply
            .transaction_state
            .system_stems
            .known_stems
            .iter()
            .find(|stem| stem.stem_identity == identity && stem.sig_attached)
            .ok_or(NativeReductionFoundationPrefixError::MissingStemMedian {
                system_id,
                stem: vertex,
            })?;
        stem_medians.insert(vertex, stem.geometry.median);
    }
    check_native_reduction_beam_groups_in_sig(
        &mut stems.systems[system_index]
            .transaction
            .state_after
            .beam_state
            .sig,
        &stem_medians,
        stems.reduction_interline,
        stems.reduction_skew,
    )
}

/// Dependency-light, atomic port of Java's beam-group consistency epilog.
pub fn check_native_reduction_beam_groups_in_sig(
    sig: &mut NativeSigSystem,
    stem_medians: &BTreeMap<NativeSigVertexId, NativeStemLine>,
    interline: i32,
    skew: HeadlessSkew,
) -> Result<NativeReductionBeamGroupTransaction, NativeReductionFoundationPrefixError> {
    let mut shadow = sig.clone();
    let transaction =
        apply_native_reduction_beam_groups(&mut shadow, stem_medians, interline, &skew)?;
    *sig = shadow;
    Ok(transaction)
}

fn apply_native_reduction_beam_groups(
    sig: &mut NativeSigSystem,
    stem_medians: &BTreeMap<NativeSigVertexId, NativeStemLine>,
    interline: i32,
    skew: &HeadlessSkew,
) -> Result<NativeReductionBeamGroupTransaction, NativeReductionFoundationPrefixError> {
    if interline <= 0 || !skew.slope.is_finite() {
        return Err(
            NativeReductionFoundationPrefixError::InvalidStemRefinementContext {
                system_id: sig.system_id,
            },
        );
    }
    sig.validate_integrity()?;
    // Java allocates the outer group list, so new split groups are not revisited.
    let initial_groups = sig
        .vertices
        .iter()
        .filter(|vertex| vertex.active && vertex.kind == NativeSigInterKind::BeamGroup)
        .map(|vertex| NativeSigVertexId(vertex.ordinal))
        .collect::<Vec<_>>();
    let mut checks = Vec::new();
    let mut splits = Vec::new();
    for &group in &initial_groups {
        // The enhanced-for loop also owns a fresh `getBeams()` snapshot.
        let beams = native_beam_group_members(sig, group);
        for beam in beams {
            let siblings = native_sorted_beams_around(sig, group, beam, skew)?;
            let beam_index = siblings.iter().position(|candidate| *candidate == beam);
            let mut previous_beam = None;
            let mut common_concrete_stems = Vec::new();
            let mut split_group = None;
            if let Some(index) = beam_index.filter(|index| *index > 0) {
                let previous = siblings[index - 1];
                previous_beam = Some(previous);
                common_concrete_stems =
                    native_common_concrete_stems(sig, previous, beam, stem_medians, interline)?;
                if common_concrete_stems.is_empty() {
                    let split = split_native_reduction_beam_group(
                        sig,
                        group,
                        &siblings,
                        index,
                        stem_medians,
                        interline,
                    )?;
                    split_group = Some(split.alien_group);
                    splits.push(split);
                }
            }
            checks.push(NativeReductionBeamGroupCheck {
                group,
                beam,
                siblings,
                beam_index,
                previous_beam,
                common_concrete_stems,
                split_group,
            });
        }
    }
    sig.validate_integrity()?;
    Ok(NativeReductionBeamGroupTransaction {
        system_id: sig.system_id,
        initial_groups,
        checks,
        splits,
    })
}

fn split_native_reduction_beam_group(
    sig: &mut NativeSigSystem,
    original_group: NativeSigVertexId,
    siblings: &[NativeSigVertexId],
    alien_index: usize,
    stem_medians: &BTreeMap<NativeSigVertexId, NativeStemLine>,
    interline: i32,
) -> Result<NativeReductionBeamGroupSplit, NativeReductionFoundationPrefixError> {
    let upper_beam = siblings[alien_index - 1];
    let lower_beam = siblings[alien_index];
    let moved_beams = siblings[alien_index..].to_vec();
    let alien_group = NativeSigVertexId(sig.vertices.len());
    sig.append_vertex(crate::native_sig::NativeSigVertex {
        ordinal: alien_group.0,
        active: true,
        removed: false,
        frozen: false,
        kind: NativeSigInterKind::BeamGroup,
        shape: None,
        grade: 1.0,
        contextual_grade: None,
        bounds: NativeSigBounds {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        },
        abnormal: false,
        beam_geometry: None,
    })?;

    let mut removed_containments = Vec::new();
    let mut added_containments = Vec::new();
    let mut added_beam_supports = Vec::new();
    for &beam in &moved_beams {
        if let Some(edge) = first_active_directed_relation(
            sig,
            original_group,
            beam,
            Some(NativeSigRelationKind::Containment),
        ) {
            sig.remove_edge(edge)?;
            removed_containments.push(edge);
        }
        let containment = append_native_reduction_relation(
            sig,
            alien_group,
            beam,
            NativeSigRelationKind::Containment,
            None,
        )?;
        added_containments.push(containment);
        for member in native_beam_group_members(sig, alien_group) {
            if member != beam {
                if let Some(edge) = insert_native_reduction_beam_support(sig, beam, member)? {
                    added_beam_supports.push(edge);
                }
            }
        }
    }

    let mut removed_cross_stem_relations = Vec::new();
    for stem in native_concrete_stems(sig, lower_beam, stem_medians, interline)? {
        if let Some(edge) = first_active_directed_relation(sig, upper_beam, stem, None) {
            sig.remove_edge(edge)?;
            removed_cross_stem_relations.push(edge);
        }
    }
    for stem in native_concrete_stems(sig, upper_beam, stem_medians, interline)? {
        if let Some(edge) = first_active_directed_relation(sig, lower_beam, stem, None) {
            sig.remove_edge(edge)?;
            removed_cross_stem_relations.push(edge);
        }
    }

    // Java's non-sibling dispatch stream ends in `peek` without a terminal
    // operation, so it intentionally performs no mutation.
    let mut removed_cross_beam_relations = Vec::new();
    let old_beams = native_beam_group_members(sig, original_group);
    let alien_beams = native_beam_group_members(sig, alien_group);
    for one in old_beams {
        for &two in &alien_beams {
            for (source, target) in [(one, two), (two, one)] {
                if let Some(edge) = first_active_directed_relation(sig, source, target, None) {
                    sig.remove_edge(edge)?;
                    removed_cross_beam_relations.push(edge);
                }
            }
        }
    }
    recompute_native_beam_group_bounds(sig, original_group);
    recompute_native_beam_group_bounds(sig, alien_group);
    Ok(NativeReductionBeamGroupSplit {
        original_group,
        alien_group,
        upper_beam,
        lower_beam,
        moved_beams,
        removed_containments,
        added_containments,
        added_beam_supports,
        removed_cross_stem_relations,
        removed_cross_beam_relations,
    })
}

fn native_sorted_beams_around(
    sig: &NativeSigSystem,
    group: NativeSigVertexId,
    beam: NativeSigVertexId,
    skew: &HeadlessSkew,
) -> Result<Vec<NativeSigVertexId>, NativeReductionFoundationPrefixError> {
    let geometry = native_beam_geometry(sig, beam)?;
    let x1 = skew.deskewed_x(geometry.x1, geometry.y1);
    let x2 = skew.deskewed_x(geometry.x2, geometry.y2);
    let x = (x1 + x2) / 2.0;
    let mut siblings = Vec::new();
    for candidate in native_beam_group_members(sig, group) {
        if candidate == beam {
            siblings.push(candidate);
            continue;
        }
        let other = native_beam_geometry(sig, candidate)?;
        let left = x1.max(skew.deskewed_x(other.x1, other.y1));
        let right = x2.min(skew.deskewed_x(other.x2, other.y2));
        if right > left {
            siblings.push(candidate);
        }
    }
    let mut ordinates = BTreeMap::new();
    for &candidate in &siblings {
        let ordinate = native_beam_y_at_x(native_beam_geometry(sig, candidate)?, x);
        if !ordinate.is_finite() {
            return Err(
                NativeReductionFoundationPrefixError::InvalidBeamGroupGeometry {
                    system_id: sig.system_id,
                    beam: candidate,
                },
            );
        }
        ordinates.insert(candidate, ordinate);
    }
    siblings.sort_by(|one, two| ordinates[one].total_cmp(&ordinates[two]));
    Ok(siblings)
}

fn native_common_concrete_stems(
    sig: &NativeSigSystem,
    one: NativeSigVertexId,
    two: NativeSigVertexId,
    stem_medians: &BTreeMap<NativeSigVertexId, NativeStemLine>,
    interline: i32,
) -> Result<Vec<NativeSigVertexId>, NativeReductionFoundationPrefixError> {
    let one_stems = native_concrete_stems(sig, one, stem_medians, interline)?;
    let two_stems = native_concrete_stems(sig, two, stem_medians, interline)?;
    Ok(one_stems
        .into_iter()
        .filter(|stem| two_stems.contains(stem))
        .collect())
}

fn native_concrete_stems(
    sig: &NativeSigSystem,
    beam: NativeSigVertexId,
    stem_medians: &BTreeMap<NativeSigVertexId, NativeStemLine>,
    interline: i32,
) -> Result<Vec<NativeSigVertexId>, NativeReductionFoundationPrefixError> {
    let max_gap = (0.25 * f64::from(interline)).round_ties_even() as i32;
    let beam_geometry = native_beam_geometry(sig, beam)?;
    let mut stems = Vec::new();
    for edge in sig.incident_edges(beam.0)? {
        if edge.kind != NativeSigRelationKind::BeamStem {
            continue;
        }
        let other = if edge.source == beam.0 {
            edge.target
        } else {
            edge.source
        };
        if sig.vertices[other].kind != NativeSigInterKind::Stem {
            continue;
        }
        let stem = NativeSigVertexId(other);
        let median = *stem_medians.get(&stem).ok_or(
            NativeReductionFoundationPrefixError::MissingStemMedian {
                system_id: sig.system_id,
                stem,
            },
        )?;
        let beam_middle = generic_intersection(stem_segment(median), beam_segment(beam_geometry)).y;
        let gap = if median.start.y <= beam_middle {
            let top = shifted_beam_segment(beam_geometry, -beam_geometry.height / 2.0);
            let beam_top = generic_intersection(stem_segment(median), top).y;
            0.0_f64.max(beam_top - median.stop.y)
        } else {
            let bottom = shifted_beam_segment(beam_geometry, beam_geometry.height / 2.0);
            let beam_bottom = generic_intersection(stem_segment(median), bottom).y;
            0.0_f64.max(median.start.y - beam_bottom)
        };
        if !gap.is_finite() {
            return Err(
                NativeReductionFoundationPrefixError::InvalidBeamGroupGeometry {
                    system_id: sig.system_id,
                    beam,
                },
            );
        }
        if gap <= f64::from(max_gap) {
            stems.push(stem);
        }
    }
    Ok(stems)
}

fn native_beam_group_members(
    sig: &NativeSigSystem,
    group: NativeSigVertexId,
) -> Vec<NativeSigVertexId> {
    sig.edges
        .iter()
        .filter(|edge| {
            edge.active
                && edge.source == group.0
                && edge.kind == NativeSigRelationKind::Containment
                && sig.vertices[edge.target].active
                && is_native_beam(sig.vertices[edge.target].kind)
        })
        .map(|edge| NativeSigVertexId(edge.target))
        .collect()
}

fn is_native_beam(kind: NativeSigInterKind) -> bool {
    matches!(
        kind,
        NativeSigInterKind::Beam | NativeSigInterKind::BeamHook | NativeSigInterKind::SmallBeam
    )
}

fn native_beam_geometry(
    sig: &NativeSigSystem,
    beam: NativeSigVertexId,
) -> Result<crate::native_sig::NativeSigBeamGeometry, NativeReductionFoundationPrefixError> {
    sig.vertices[beam.0].beam_geometry.ok_or(
        NativeReductionFoundationPrefixError::MissingBeamGeometry {
            system_id: sig.system_id,
            beam,
        },
    )
}

fn beam_segment(geometry: crate::native_sig::NativeSigBeamGeometry) -> Segment {
    Segment {
        x1: geometry.x1,
        y1: geometry.y1,
        x2: geometry.x2,
        y2: geometry.y2,
    }
}

fn shifted_beam_segment(geometry: crate::native_sig::NativeSigBeamGeometry, dy: f64) -> Segment {
    Segment {
        x1: geometry.x1,
        y1: geometry.y1 + dy,
        x2: geometry.x2,
        y2: geometry.y2 + dy,
    }
}

fn native_beam_y_at_x(geometry: crate::native_sig::NativeSigBeamGeometry, x: f64) -> f64 {
    generic_intersection(
        beam_segment(geometry),
        Segment {
            x1: x,
            y1: 0.0,
            x2: x,
            y2: 1_000.0,
        },
    )
    .y
}

fn first_active_directed_relation(
    sig: &NativeSigSystem,
    source: NativeSigVertexId,
    target: NativeSigVertexId,
    kind: Option<NativeSigRelationKind>,
) -> Option<NativeSigEdgeId> {
    sig.edges
        .iter()
        .find(|edge| {
            edge.active
                && edge.source == source.0
                && edge.target == target.0
                && kind.is_none_or(|kind| edge.kind == kind)
        })
        .map(|edge| NativeSigEdgeId(edge.ordinal))
}

fn append_native_reduction_relation(
    sig: &mut NativeSigSystem,
    source: NativeSigVertexId,
    target: NativeSigVertexId,
    kind: NativeSigRelationKind,
    support: Option<crate::native_sig::NativeSigSupport>,
) -> Result<NativeSigEdgeId, NativeSigError> {
    let edge = NativeSigEdgeId(sig.edges.len());
    sig.append_edge(NativeSigEdge {
        ordinal: edge.0,
        active: true,
        source: source.0,
        target: target.0,
        kind,
        origin: NativeSigRelationOrigin::BaselineGraph,
        support,
        beam_portion: None,
        stem_extension: None,
        head_stem: None,
    })?;
    Ok(edge)
}

fn insert_native_reduction_beam_support(
    sig: &mut NativeSigSystem,
    one: NativeSigVertexId,
    two: NativeSigVertexId,
) -> Result<Option<NativeSigEdgeId>, NativeSigError> {
    let (source, target) = normalized_vertex_pair(one, two);
    let excluded = sig.edges.iter().any(|edge| {
        edge.active
            && edge.kind == NativeSigRelationKind::Exclusion
            && ((edge.source == source.0 && edge.target == target.0)
                || (edge.source == target.0 && edge.target == source.0))
    });
    let exists = sig.edges.iter().any(|edge| {
        edge.active
            && edge.kind == NativeSigRelationKind::BeamBeam
            && ((edge.source == source.0 && edge.target == target.0)
                || (edge.source == target.0 && edge.target == source.0))
    });
    if excluded || exists {
        return Ok(None);
    }
    append_native_reduction_relation(
        sig,
        source,
        target,
        NativeSigRelationKind::BeamBeam,
        Some(crate::native_sig::NativeSigSupport {
            grade: 1.0,
            bar_connection_impacts: None,
        }),
    )
    .map(Some)
}

fn recompute_native_beam_group_bounds(sig: &mut NativeSigSystem, group: NativeSigVertexId) {
    let bounds = native_beam_group_members(sig, group)
        .into_iter()
        .map(|beam| sig.vertices[beam.0].bounds)
        .reduce(native_bounds_union)
        .unwrap_or(NativeSigBounds {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        });
    sig.vertices[group.0].bounds = bounds;
}

fn native_bounds_union(one: NativeSigBounds, two: NativeSigBounds) -> NativeSigBounds {
    let right = one
        .x
        .saturating_add(one.width)
        .max(two.x.saturating_add(two.width));
    let bottom = one
        .y
        .saturating_add(one.height)
        .max(two.y.saturating_add(two.height));
    let x = one.x.min(two.x);
    let y = one.y.min(two.y);
    NativeSigBounds {
        x,
        y,
        width: right.saturating_sub(x),
        height: bottom.saturating_sub(y),
    }
}

const fn stem_segment(line: NativeStemLine) -> Segment {
    Segment {
        x1: line.start.x,
        y1: line.start.y,
        x2: line.stop.x,
        y2: line.stop.y,
    }
}

fn reduce_native_foundation_continuation_epoch(
    sig: &mut NativeSigSystem,
    bindings: &mut NativeSigSystemBindings,
    stem_medians: &BTreeMap<NativeSigVertexId, NativeStemLine>,
    head_identities: &BTreeMap<NativeSigVertexId, NativeReductionHeadIdentity>,
    merged_staff_pairs: &[(usize, usize)],
) -> Result<
    NativeReductionFoundationContinuationEpochTransaction,
    NativeReductionFoundationPrefixError,
> {
    let opening_weak_purge = contextualize_and_purge_native_weaks(sig)?;
    // AdapterForFoundations.checkSlurs() is the inherited empty set.
    let consistency_passes = run_native_foundation_consistency_fixed_point(
        sig,
        bindings,
        stem_medians,
        head_identities,
        merged_staff_pairs,
    )?;
    let remaining_exclusions = reduce_native_sig_exclusions(sig)?;
    let late_consistency = reduce_native_foundation_late_consistency(sig, stem_medians)?;
    let outer_deleted_vertices = opening_weak_purge.removed_vertices.clone();
    let outer_reduced_vertices = remaining_exclusions.removed_vertices.clone();
    let requires_outer_repeat =
        !outer_deleted_vertices.is_empty() || !outer_reduced_vertices.is_empty();
    Ok(NativeReductionFoundationContinuationEpochTransaction {
        system_id: sig.system_id,
        opening_weak_purge,
        consistency_passes,
        remaining_exclusions,
        late_consistency,
        outer_deleted_vertices,
        outer_reduced_vertices,
        requires_outer_repeat,
    })
}

/// Port foundations `AdapterForFoundations.checkLateConsistencies()`.
pub fn reduce_native_foundation_late_consistency(
    sig: &mut NativeSigSystem,
    stem_medians: &BTreeMap<NativeSigVertexId, NativeStemLine>,
) -> Result<NativeReductionFoundationLateConsistencyTransaction, NativeReductionFoundationPrefixError>
{
    let chord_analysis = analyze_native_foundation_chords(sig, stem_medians)?;
    let exclusions = reduce_native_sig_exclusions(sig)?;
    let weak_purge = contextualize_and_purge_native_weaks(sig)?;
    Ok(NativeReductionFoundationLateConsistencyTransaction {
        system_id: sig.system_id,
        chord_analysis,
        exclusions,
        weak_purge,
        modification_count: 0,
    })
}

fn run_native_foundation_consistency_pass(
    sig: &mut NativeSigSystem,
    bindings: &mut NativeSigSystemBindings,
    stem_medians: &BTreeMap<NativeSigVertexId, NativeStemLine>,
    head_identities: &BTreeMap<NativeSigVertexId, NativeReductionHeadIdentity>,
    merged_staff_pairs: &[(usize, usize)],
) -> Result<NativeReductionFoundationConsistencyPassTransaction, NativeReductionFoundationPrefixError>
{
    let stem_ending = prune_native_foundation_stem_ending_heads(sig, stem_medians)?;
    let post_stem_ending_weak_purge = contextualize_and_purge_native_weaks(sig)?;
    let heads =
        prune_native_foundation_heads(sig, stem_medians, head_identities, merged_staff_pairs)?;
    let post_heads_weak_purge = contextualize_and_purge_native_weaks(sig)?;
    let hooks = prune_native_foundation_hooks(sig)?;
    let post_hooks_weak_purge = contextualize_and_purge_native_weaks(sig)?;
    let beams = prune_native_foundation_beams(sig)?;
    let post_beams_weak_purge = contextualize_and_purge_native_weaks(sig)?;
    let ledgers = prune_native_foundation_ledgers(sig, bindings)?;
    let post_ledgers_weak_purge = contextualize_and_purge_native_weaks(sig)?;
    let stems = prune_native_foundation_stems(sig, stem_medians)?;
    let post_stems_weak_purge = contextualize_and_purge_native_weaks(sig)?;
    let modification_count = stem_ending.modified_stems.len()
        + heads.mutations.len()
        + hooks.removed_beams.len()
        + beams.removed_beams.len()
        + ledgers.modification_count
        + stems.modification_count;
    Ok(NativeReductionFoundationConsistencyPassTransaction {
        system_id: sig.system_id,
        stem_ending,
        post_stem_ending_weak_purge,
        heads,
        post_heads_weak_purge,
        hooks,
        post_hooks_weak_purge,
        beams,
        post_beams_weak_purge,
        ledgers,
        post_ledgers_weak_purge,
        stems,
        post_stems_weak_purge,
        modification_count,
    })
}

fn run_native_foundation_consistency_fixed_point(
    sig: &mut NativeSigSystem,
    bindings: &mut NativeSigSystemBindings,
    stem_medians: &BTreeMap<NativeSigVertexId, NativeStemLine>,
    head_identities: &BTreeMap<NativeSigVertexId, NativeReductionHeadIdentity>,
    merged_staff_pairs: &[(usize, usize)],
) -> Result<
    Vec<NativeReductionFoundationConsistencyPassTransaction>,
    NativeReductionFoundationPrefixError,
> {
    let mut consistency_passes = Vec::new();
    loop {
        let pass = run_native_foundation_consistency_pass(
            sig,
            bindings,
            stem_medians,
            head_identities,
            merged_staff_pairs,
        )?;
        let converged = pass.modification_count == 0;
        consistency_passes.push(pass);
        if converged {
            return Ok(consistency_passes);
        }
    }
}

/// Port Java `SigReducer.checkLedgers()` including the pathological shared
/// identity repair and the iterative inside-to-outside support cascade.
pub fn prune_native_foundation_ledgers(
    sig: &mut NativeSigSystem,
    bindings: &mut NativeSigSystemBindings,
) -> Result<NativeReductionLedgerCheckTransaction, NativeReductionFoundationPrefixError> {
    sig.validate_integrity()?;
    let shared_fixes = fix_native_shared_ledgers(sig, bindings)?;
    let all_heads = {
        let mut heads = sig
            .vertices
            .iter()
            .filter(|vertex| vertex.active && vertex.kind == NativeSigInterKind::Head)
            .map(|vertex| NativeSigVertexId(vertex.ordinal))
            .collect::<Vec<_>>();
        heads.sort_by_key(|head| sig.vertices[head.0].bounds.x);
        heads
    };
    let mut removals = Vec::new();
    let mut pass = 0;
    loop {
        pass += 1;
        let snapshot = bindings
            .reduction_staffs
            .iter()
            .enumerate()
            .filter(|(_, staff)| !staff.tablature)
            .flat_map(|(staff_index, staff)| {
                staff
                    .ledger_map
                    .iter()
                    .flat_map(move |(&ledger_index, ledgers)| {
                        ledgers
                            .iter()
                            .copied()
                            .map(move |ledger| (staff_index, ledger_index, ledger))
                    })
            })
            .collect::<Vec<_>>();
        let mut modified = false;
        for (staff_index, ledger_index, ledger) in snapshot {
            if sig.vertex(ledger.0).is_none() {
                continue;
            }
            if native_ledger_has_support(
                sig,
                bindings,
                staff_index,
                ledger_index,
                ledger,
                &all_heads,
            )? {
                continue;
            }
            let staff_id = bindings.reduction_staffs[staff_index].staff_id;
            remove_first_staff_ledger(&mut bindings.reduction_staffs[staff_index], ledger);
            sig.remove_vertex(ledger)?;
            removals.push(NativeReductionLedgerRemoval {
                pass,
                staff_id,
                ledger_index,
                ledger,
            });
            modified = true;
        }
        if !modified {
            break;
        }
    }
    sig.validate_integrity()?;
    Ok(NativeReductionLedgerCheckTransaction {
        system_id: sig.system_id,
        modification_count: shared_fixes.len() + removals.len(),
        shared_fixes,
        removals,
    })
}

fn fix_native_shared_ledgers(
    sig: &NativeSigSystem,
    bindings: &mut NativeSigSystemBindings,
) -> Result<Vec<NativeReductionSharedLedgerFix>, NativeReductionFoundationPrefixError> {
    let mut shared = Vec::new();
    for lower_index in 1..bindings.reduction_staffs.len() {
        let upper_index = lower_index - 1;
        let upper_first = bindings.reduction_staffs[upper_index]
            .ledger_map
            .get(&1)
            .cloned()
            .unwrap_or_default();
        if upper_first.is_empty()
            || !bindings.reduction_staffs[lower_index]
                .ledger_map
                .contains_key(&-1)
        {
            continue;
        }
        let lower_outer = bindings.reduction_staffs[lower_index]
            .ledger_map
            .first_key_value()
            .map(|(_, ledgers)| ledgers.clone())
            .unwrap_or_default();
        for ledger in upper_first
            .into_iter()
            .filter(|ledger| lower_outer.contains(ledger))
        {
            shared.push(fix_native_shared_ledger(
                sig,
                bindings,
                ledger,
                upper_index,
                lower_index,
            )?);
        }
    }
    Ok(shared)
}

fn fix_native_shared_ledger(
    sig: &NativeSigSystem,
    bindings: &mut NativeSigSystemBindings,
    ledger: NativeSigVertexId,
    upper_index: usize,
    lower_index: usize,
) -> Result<NativeReductionSharedLedgerFix, NativeReductionFoundationPrefixError> {
    let ledger_bounds = sig
        .vertex(ledger.0)
        .ok_or(NativeSigError::MissingVertex {
            system_id: sig.system_id,
            ordinal: ledger.0,
        })?
        .bounds;
    let (center_x, _) = java_bounds_center(ledger_bounds);
    let upper_y = bindings.reduction_staffs[upper_index]
        .last_line
        .y_at_x_ext(f64::from(center_x))
        .round_ties_even() as i32;
    let lower_y = bindings.reduction_staffs[lower_index]
        .first_line
        .y_at_x_ext(f64::from(center_x))
        .round_ties_even() as i32;
    let upper_margin = (f64::from(bindings.reduction_staffs[upper_index].specific_interline) * 0.33)
        .round_ties_even() as i32;
    let lower_margin = (f64::from(bindings.reduction_staffs[lower_index].specific_interline) * 0.33)
        .round_ties_even() as i32;
    let mut column_box = ledger_bounds;
    java_bounds_add_point(
        &mut column_box,
        center_x,
        upper_y.saturating_add(upper_margin),
    );
    java_bounds_add_point(
        &mut column_box,
        center_x,
        lower_y.saturating_sub(lower_margin),
    );

    let column_ledgers = sig
        .vertices
        .iter()
        .filter(|vertex| {
            vertex.active
                && vertex.kind == NativeSigInterKind::Ledger
                && java_bounds_intersect(vertex.bounds, column_box)
        })
        .map(|vertex| NativeSigVertexId(vertex.ordinal))
        .collect::<Vec<_>>();
    let mut column_heads = sig
        .vertices
        .iter()
        .filter(|vertex| {
            vertex.active
                && vertex.kind == NativeSigInterKind::Head
                && java_bounds_contains(column_box, java_bounds_center(vertex.bounds))
        })
        .map(|vertex| NativeSigVertexId(vertex.ordinal))
        .collect::<Vec<_>>();
    column_heads
        .sort_by(|left, right| best_grade_of(sig, right.0).total_cmp(&best_grade_of(sig, left.0)));

    let upper_staff_id = bindings.reduction_staffs[upper_index].staff_id;
    let lower_staff_id = bindings.reduction_staffs[lower_index].staff_id;
    let owner_staff_id = column_heads.first().map(|head| {
        let (_, head_y) = java_bounds_center(sig.vertices[head.0].bounds);
        let upper_dp = (f64::from(head_y - upper_y)
            / (0.5 * f64::from(bindings.reduction_staffs[upper_index].specific_interline)))
        .round_ties_even() as i32;
        let lower_dp = (f64::from(lower_y - head_y)
            / (0.5 * f64::from(bindings.reduction_staffs[lower_index].specific_interline)))
        .round_ties_even() as i32;
        if lower_dp > upper_dp {
            lower_staff_id
        } else {
            upper_staff_id
        }
    });

    if let Some(owner_id) = owner_staff_id {
        let owner_index = if owner_id == upper_staff_id {
            upper_index
        } else {
            lower_index
        };
        let other_index = if owner_index == upper_index {
            lower_index
        } else {
            upper_index
        };
        purge_staff_ledgers(&mut bindings.reduction_staffs[other_index], &column_ledgers);
        for &head in &column_heads {
            let head_bounds = sig.vertices[head.0].bounds;
            let geometry = bindings
                .overlap_geometry
                .get_mut(&head)
                .and_then(|geometry| geometry.head.as_mut())
                .ok_or(NativeReductionFoundationPrefixError::MissingHeadIdentity {
                    system_id: sig.system_id,
                    head,
                })?;
            if geometry.staff_id != Some(owner_id) {
                geometry.staff_id = Some(owner_id);
                geometry.integer_pitch = native_staff_integer_pitch(
                    &bindings.reduction_staffs[owner_index],
                    java_bounds_center(head_bounds),
                );
            }
        }
    } else {
        purge_staff_ledgers(&mut bindings.reduction_staffs[upper_index], &column_ledgers);
        purge_staff_ledgers(&mut bindings.reduction_staffs[lower_index], &column_ledgers);
    }

    Ok(NativeReductionSharedLedgerFix {
        ledger,
        upper_staff_id,
        lower_staff_id,
        column_ledgers,
        column_heads,
        owner_staff_id,
    })
}

fn native_ledger_has_support(
    sig: &NativeSigSystem,
    bindings: &NativeSigSystemBindings,
    staff_index: usize,
    ledger_index: i32,
    ledger: NativeSigVertexId,
    all_heads: &[NativeSigVertexId],
) -> Result<bool, NativeReductionFoundationPrefixError> {
    let staff = bindings.reduction_staffs.get(staff_index).ok_or(
        NativeReductionFoundationPrefixError::MissingLedgerStaff {
            system_id: sig.system_id,
            staff_id: staff_index,
        },
    )?;
    let bounds = sig.vertices[ledger.0].bounds;
    let ledger_box = NativeSigBounds {
        x: bounds.x,
        y: bounds.y.saturating_sub(bindings.reduction_interline),
        width: bounds.width,
        height: bounds
            .height
            .saturating_add(bindings.reduction_interline.saturating_mul(2)),
    };
    let next_index = ledger_index + ledger_index.signum();
    if staff.ledger_map.get(&next_index).is_some_and(|next| {
        next.iter().any(|next_ledger| {
            sig.vertex(next_ledger.0)
                .is_some_and(|vertex| java_x_overlap(ledger_box, vertex.bounds) > 0)
        })
    }) {
        return Ok(true);
    }

    let ledger_pitch = ledger_index.signum() * 4 + (2 * ledger_index);
    let next_pitch = ledger_pitch + ledger_index.signum();
    for &head in all_heads {
        if sig.vertex(head.0).is_none()
            || !java_bounds_intersect(ledger_box, sig.vertices[head.0].bounds)
        {
            continue;
        }
        let pitch = bindings
            .overlap_geometry
            .get(&head)
            .and_then(|geometry| geometry.head)
            .ok_or(NativeReductionFoundationPrefixError::MissingHeadIdentity {
                system_id: sig.system_id,
                head,
            })?
            .integer_pitch;
        if pitch == ledger_pitch || pitch == next_pitch {
            return Ok(true);
        }
    }
    Ok(false)
}

fn native_staff_integer_pitch(
    staff: &crate::native_sig::NativeSigReductionStaff,
    (x, y): (i32, i32),
) -> i32 {
    let x = f64::from(x);
    let y = f64::from(y);
    let top = staff.first_line.y_at_x_ext(x);
    let bottom = staff.last_line.y_at_x_ext(x);
    let pitch = if y >= top && y <= bottom {
        (staff.line_count.saturating_sub(1) as f64) * ((2.0 * y) - bottom - top) / (bottom - top)
    } else {
        let direction = if y < top { -1 } else { 1 };
        let mut previous_y = if direction == -1 { top } else { bottom };
        let mut previous_pitch = direction * staff.line_count.saturating_sub(1) as i32;
        let mut index = direction;
        loop {
            let Some(line) = staff.ledger_lines.get(&index) else {
                break previous_pitch as f64
                    + (2.0 * (y - previous_y) / f64::from(staff.specific_interline));
            };
            let ledger_y = line.y_at_x_ext(x);
            if (direction == -1 && ledger_y <= y) || (direction == 1 && ledger_y >= y) {
                break previous_pitch as f64
                    + (2.0 * f64::from(direction) * (y - previous_y) / (ledger_y - previous_y));
            }
            previous_y = ledger_y;
            previous_pitch += 2 * direction;
            index += direction;
        }
    };
    pitch.round_ties_even() as i32
}

fn remove_first_staff_ledger(
    staff: &mut crate::native_sig::NativeSigReductionStaff,
    ledger: NativeSigVertexId,
) {
    let found = staff
        .ledger_map
        .iter()
        .find_map(|(&index, ledgers)| ledgers.contains(&ledger).then_some(index));
    if let Some(index) = found {
        if let Some(ledgers) = staff.ledger_map.get_mut(&index) {
            ledgers.retain(|candidate| *candidate != ledger);
            if ledgers.is_empty() {
                staff.ledger_map.remove(&index);
            }
        }
    }
}

fn purge_staff_ledgers(
    staff: &mut crate::native_sig::NativeSigReductionStaff,
    ledgers: &[NativeSigVertexId],
) {
    staff.ledger_map.retain(|_, owned| {
        owned.retain(|candidate| !ledgers.contains(candidate));
        !owned.is_empty()
    });
}

fn java_bounds_center(bounds: crate::native_sig::NativeSigBounds) -> (i32, i32) {
    (
        bounds.x.saturating_add(bounds.width / 2),
        bounds.y.saturating_add(bounds.height / 2),
    )
}

fn java_bounds_add_point(bounds: &mut crate::native_sig::NativeSigBounds, x: i32, y: i32) {
    let right = bounds.x.saturating_add(bounds.width).max(x);
    let bottom = bounds.y.saturating_add(bounds.height).max(y);
    bounds.x = bounds.x.min(x);
    bounds.y = bounds.y.min(y);
    bounds.width = right.saturating_sub(bounds.x);
    bounds.height = bottom.saturating_sub(bounds.y);
}

fn java_bounds_intersect(
    left: crate::native_sig::NativeSigBounds,
    right: crate::native_sig::NativeSigBounds,
) -> bool {
    left.width > 0
        && left.height > 0
        && right.width > 0
        && right.height > 0
        && left.x < right.x.saturating_add(right.width)
        && right.x < left.x.saturating_add(left.width)
        && left.y < right.y.saturating_add(right.height)
        && right.y < left.y.saturating_add(left.height)
}

fn java_bounds_contains(bounds: crate::native_sig::NativeSigBounds, (x, y): (i32, i32)) -> bool {
    bounds.width > 0
        && bounds.height > 0
        && x >= bounds.x
        && y >= bounds.y
        && x < bounds.x.saturating_add(bounds.width)
        && y < bounds.y.saturating_add(bounds.height)
}

fn java_x_overlap(
    left: crate::native_sig::NativeSigBounds,
    right: crate::native_sig::NativeSigBounds,
) -> i32 {
    left.x
        .saturating_add(left.width)
        .min(right.x.saturating_add(right.width))
        .saturating_sub(left.x.max(right.x))
}

fn native_stems_head_identities(
    stems: &NativeStemsRecognition,
    system_id: usize,
) -> Result<NativeReductionHeadLookup, NativeReductionFoundationPrefixError> {
    let system = stems
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .ok_or(NativeReductionFoundationPrefixError::MissingSystem(
            system_id,
        ))?;
    let bindings = &system.transaction.state_after.beam_state.bindings;
    let identities = bindings
        .overlap_geometry
        .iter()
        .filter_map(|(&vertex, geometry)| {
            geometry.head.and_then(|head| {
                head.staff_id.map(|staff_id| {
                    (
                        vertex,
                        NativeReductionHeadIdentity {
                            staff_id,
                            integer_pitch: head.integer_pitch,
                        },
                    )
                })
            })
        })
        .collect();
    Ok((identities, bindings.merged_staff_pairs.clone()))
}

fn native_stems_terminal_medians(
    stems: &NativeStemsRecognition,
    system_id: usize,
) -> Result<BTreeMap<NativeSigVertexId, NativeStemLine>, NativeReductionFoundationPrefixError> {
    let system = stems
        .systems
        .iter()
        .find(|system| system.system_id == system_id)
        .ok_or(NativeReductionFoundationPrefixError::MissingSystem(
            system_id,
        ))?;
    let carrier = &system.transaction.state_after;
    let bindings = &carrier.beam_state.bindings;
    let known_stems = &carrier
        .beam_state
        .latest_base_apply
        .transaction_state
        .system_stems
        .known_stems;
    let mut medians = BTreeMap::new();
    for (&identity, &vertex) in &bindings.stem_vertices {
        let stem = known_stems
            .iter()
            .find(|stem| stem.stem_identity == identity && stem.sig_attached)
            .ok_or(NativeReductionFoundationPrefixError::MissingStemMedian {
                system_id,
                stem: vertex,
            })?;
        medians.insert(vertex, stem.geometry.median);
    }
    Ok(medians)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativeHeadDuration {
    Quarter,
    Half,
    Whole,
    Breve,
}

/// Port foundations `analyzeChords()` before any weak purge.
pub fn analyze_native_foundation_chords(
    sig: &mut NativeSigSystem,
    stem_medians: &BTreeMap<NativeSigVertexId, NativeStemLine>,
) -> Result<NativeReductionChordAnalysisTransaction, NativeReductionFoundationPrefixError> {
    const GOOD_INTER_GRADE: f64 = 0.4;
    const MIN_STEM_HEAD_IOU: f64 = 0.02;

    sig.validate_integrity()?;
    let mut stems = sig
        .vertices
        .iter()
        .filter(|vertex| vertex.active && vertex.kind == NativeSigInterKind::Stem)
        .map(|vertex| NativeSigVertexId(vertex.ordinal))
        .collect::<Vec<_>>();
    stems.sort_by(|left, right| {
        sig.vertices[right.0]
            .grade
            .total_cmp(&sig.vertices[left.0].grade)
    });
    let mut all_heads = sig
        .vertices
        .iter()
        .filter(|vertex| {
            vertex.active
                && vertex.kind == NativeSigInterKind::Head
                && vertex.shape.as_deref().is_some_and(is_stem_head_shape)
        })
        .map(|vertex| NativeSigVertexId(vertex.ordinal))
        .collect::<Vec<_>>();
    all_heads.sort_by_key(|head| sig.vertices[head.0].bounds.x);

    let mut scanned_stems = Vec::new();
    let mut intersected_head_exclusions = Vec::new();
    let mut incompatible_exclusions = Vec::new();
    let mut head_head_supports = Vec::new();

    for stem in stems {
        if sig.vertices[stem.0].grade < GOOD_INTER_GRADE {
            break;
        }
        scanned_stems.push(stem);
        let &median = stem_medians.get(&stem).ok_or(
            NativeReductionFoundationPrefixError::MissingStemMedian {
                system_id: sig.system_id,
                stem,
            },
        )?;
        let mut intersected_heads = all_heads
            .iter()
            .copied()
            .filter(|head| {
                line_intersects_rectangle(median, sig.vertices[head.0].bounds)
                    && java_rectangle_iou(sig.vertices[stem.0].bounds, sig.vertices[head.0].bounds)
                        >= MIN_STEM_HEAD_IOU
            })
            .collect::<Vec<_>>();
        let mut duration_sets = Vec::<(NativeHeadDuration, Vec<NativeSigVertexId>)>::new();
        let mut standard_heads = Vec::new();
        let mut small_heads = Vec::new();
        let mut standard_beams = Vec::new();
        let mut small_beams = Vec::new();

        let incident = sig
            .incident_edges(stem.0)?
            .into_iter()
            .map(|edge| (edge.kind, edge.source))
            .collect::<Vec<_>>();
        for (kind, source) in incident {
            match kind {
                NativeSigRelationKind::HeadStem => {
                    let head = NativeSigVertexId(source);
                    intersected_heads.retain(|candidate| *candidate != head);
                    let shape = sig.vertices[head.0].shape.as_deref().ok_or(
                        NativeReductionFoundationPrefixError::UnsupportedHeadShape {
                            system_id: sig.system_id,
                            head,
                        },
                    )?;
                    let duration = native_head_duration(shape).ok_or(
                        NativeReductionFoundationPrefixError::UnsupportedHeadShape {
                            system_id: sig.system_id,
                            head,
                        },
                    )?;
                    let duration_index = duration_sets
                        .iter()
                        .position(|(candidate, _)| *candidate == duration)
                        .unwrap_or_else(|| {
                            duration_sets.push((duration, Vec::new()));
                            duration_sets.len() - 1
                        });
                    push_unique(&mut duration_sets[duration_index].1, head);
                    if is_small_head_shape(shape) {
                        push_unique(&mut small_heads, head);
                    } else {
                        push_unique(&mut standard_heads, head);
                    }
                }
                NativeSigRelationKind::BeamStem => {
                    let beam = NativeSigVertexId(source);
                    if sig.vertices[beam.0].kind == NativeSigInterKind::SmallBeam {
                        push_unique(&mut small_beams, beam);
                    } else {
                        push_unique(&mut standard_beams, beam);
                    }
                }
                _ => {}
            }
        }

        for head in intersected_heads {
            if let Some(edge) = insert_native_overlap_exclusion(sig, stem, head)? {
                push_unique_edge(&mut intersected_head_exclusions, edge);
            }
        }
        exclude_native_sets(
            sig,
            &small_heads,
            &standard_heads,
            &mut incompatible_exclusions,
        )?;
        for index in 0..duration_sets.len() {
            for other in index + 1..duration_sets.len() {
                exclude_native_sets(
                    sig,
                    &duration_sets[index].1,
                    &duration_sets[other].1,
                    &mut incompatible_exclusions,
                )?;
            }
            for left in 0..duration_sets[index].1.len() {
                for right in left + 1..duration_sets[index].1.len() {
                    if let Some(edge) = insert_native_head_head_support(
                        sig,
                        duration_sets[index].1[left],
                        duration_sets[index].1[right],
                    )? {
                        push_unique_edge(&mut head_head_supports, edge);
                    }
                }
            }
        }
        exclude_native_sets(
            sig,
            &standard_beams,
            &small_beams,
            &mut incompatible_exclusions,
        )?;
        // Java's default disallows standard heads on small beams and small
        // heads on standard beams.
        exclude_native_sets(
            sig,
            &small_beams,
            &standard_heads,
            &mut incompatible_exclusions,
        )?;
        exclude_native_sets(
            sig,
            &standard_beams,
            &small_heads,
            &mut incompatible_exclusions,
        )?;
    }
    sig.validate_integrity()?;
    Ok(NativeReductionChordAnalysisTransaction {
        system_id: sig.system_id,
        scanned_stems,
        intersected_head_exclusions,
        incompatible_exclusions,
        head_head_supports,
    })
}

fn native_head_duration(shape: &str) -> Option<NativeHeadDuration> {
    match shape {
        "NOTEHEAD_BLACK" | "NOTEHEAD_BLACK_SMALL" => Some(NativeHeadDuration::Quarter),
        "NOTEHEAD_VOID" | "NOTEHEAD_VOID_SMALL" => Some(NativeHeadDuration::Half),
        "WHOLE_NOTE" | "WHOLE_NOTE_SMALL" => Some(NativeHeadDuration::Whole),
        "BREVE" | "BREVE_SMALL" => Some(NativeHeadDuration::Breve),
        _ => None,
    }
}

fn is_small_head_shape(shape: &str) -> bool {
    matches!(
        shape,
        "NOTEHEAD_BLACK_SMALL" | "NOTEHEAD_VOID_SMALL" | "WHOLE_NOTE_SMALL" | "BREVE_SMALL"
    )
}

fn exclude_native_sets(
    sig: &mut NativeSigSystem,
    one: &[NativeSigVertexId],
    two: &[NativeSigVertexId],
    inserted: &mut Vec<NativeSigEdgeId>,
) -> Result<(), NativeSigError> {
    for &left in one {
        for &right in two {
            if let Some(edge) = insert_native_overlap_exclusion(sig, left, right)? {
                push_unique_edge(inserted, edge);
            }
        }
    }
    Ok(())
}

fn insert_native_head_head_support(
    sig: &mut NativeSigSystem,
    one: NativeSigVertexId,
    two: NativeSigVertexId,
) -> Result<Option<NativeSigEdgeId>, NativeSigError> {
    let (source, target) = normalized_vertex_pair(one, two);
    if active_exclusion_between(sig, source, target).is_some() {
        return Ok(None);
    }
    if sig.edges.iter().any(|edge| {
        edge.active
            && edge.source == source.0
            && edge.target == target.0
            && edge.kind == NativeSigRelationKind::HeadHead
    }) {
        return Ok(None);
    }
    let edge = NativeSigEdgeId(sig.edges.len());
    sig.append_edge(NativeSigEdge {
        ordinal: edge.0,
        active: true,
        source: source.0,
        target: target.0,
        kind: NativeSigRelationKind::HeadHead,
        origin: NativeSigRelationOrigin::BaselineGraph,
        support: Some(crate::native_sig::NativeSigSupport {
            grade: 1.0,
            bar_connection_impacts: None,
        }),
        beam_portion: None,
        stem_extension: None,
        head_stem: None,
    })?;
    Ok(Some(edge))
}

fn line_intersects_rectangle(
    line: NativeStemLine,
    bounds: crate::native_sig::NativeSigBounds,
) -> bool {
    let left = f64::from(bounds.x);
    let top = f64::from(bounds.y);
    let right = left + f64::from(bounds.width);
    let bottom = top + f64::from(bounds.height);
    let mut x1 = line.start.x;
    let mut y1 = line.start.y;
    let x2 = line.stop.x;
    let y2 = line.stop.y;
    let out2 = rectangle_outcode(x2, y2, left, top, right, bottom);
    if out2 == 0 {
        return true;
    }
    loop {
        let out1 = rectangle_outcode(x1, y1, left, top, right, bottom);
        if out1 == 0 {
            return true;
        }
        if out1 & out2 != 0 {
            return false;
        }
        if out1 & 3 != 0 {
            let x = if out1 & 2 != 0 { right } else { left };
            y1 = y1 + ((x - x1) * (y2 - y1) / (x2 - x1));
            x1 = x;
        } else {
            let y = if out1 & 8 != 0 { bottom } else { top };
            x1 = x1 + ((y - y1) * (x2 - x1) / (y2 - y1));
            y1 = y;
        }
    }
}

fn rectangle_outcode(x: f64, y: f64, left: f64, top: f64, right: f64, bottom: f64) -> u8 {
    let horizontal = if right <= left {
        3
    } else if x < left {
        1
    } else if x > right {
        2
    } else {
        0
    };
    let vertical = if bottom <= top {
        12
    } else if y < top {
        4
    } else if y > bottom {
        8
    } else {
        0
    };
    horizontal | vertical
}

fn vertical_line_area(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    width: f64,
) -> NativeReductionAreaGeometry {
    let dx = width / 2.0;
    NativeReductionAreaGeometry {
        components: vec![vec![
            NativeReductionAreaPoint { x: x1 - dx, y: y1 },
            NativeReductionAreaPoint { x: x2 - dx, y: y2 },
            NativeReductionAreaPoint { x: x2 + dx, y: y2 },
            NativeReductionAreaPoint { x: x1 + dx, y: y1 },
        ]],
    }
}

fn normalized_vertex_pair(
    one: NativeSigVertexId,
    two: NativeSigVertexId,
) -> (NativeSigVertexId, NativeSigVertexId) {
    if one <= two { (one, two) } else { (two, one) }
}

fn shrunk_head_core_bounds(
    bounds: crate::native_sig::NativeSigBounds,
) -> crate::native_sig::NativeSigBounds {
    let center_x = f64::from(bounds.x) + (f64::from(bounds.width) / 2.0);
    let center_y = f64::from(bounds.y) + (f64::from(bounds.height) / 2.0);
    let half_width = f64::from(bounds.width) * 0.25;
    let half_height = f64::from(bounds.height) * 0.25;
    let left = (center_x - half_width).floor() as i32;
    let top = (center_y - half_height).floor() as i32;
    let right = (center_x + half_width).ceil() as i32;
    let bottom = (center_y + half_height).ceil() as i32;
    crate::native_sig::NativeSigBounds {
        x: left,
        y: top,
        width: right.saturating_sub(left),
        height: bottom.saturating_sub(top),
    }
}

fn rectangles_intersect(
    one: crate::native_sig::NativeSigBounds,
    two: crate::native_sig::NativeSigBounds,
) -> bool {
    one.width > 0
        && one.height > 0
        && two.width > 0
        && two.height > 0
        && one.x < two.x.saturating_add(two.width)
        && two.x < one.x.saturating_add(one.width)
        && one.y < two.y.saturating_add(two.height)
        && two.y < one.y.saturating_add(one.height)
}

fn rectangle_intersection(
    one: crate::native_sig::NativeSigBounds,
    two: crate::native_sig::NativeSigBounds,
) -> crate::native_sig::NativeSigBounds {
    let x = one.x.max(two.x);
    let y = one.y.max(two.y);
    let right = one
        .x
        .saturating_add(one.width)
        .min(two.x.saturating_add(two.width));
    let bottom = one
        .y
        .saturating_add(one.height)
        .min(two.y.saturating_add(two.height));
    crate::native_sig::NativeSigBounds {
        x,
        y,
        width: right.saturating_sub(x),
        height: bottom.saturating_sub(y),
    }
}

fn heads_overlap(
    one: &NativeReductionInterGeometry,
    one_head: NativeReductionHeadGeometry,
    two: &NativeReductionInterGeometry,
    two_head: NativeReductionHeadGeometry,
) -> bool {
    let pitch_distance = (one_head.staff_id == two_head.staff_id)
        .then_some(one_head.integer_pitch.abs_diff(two_head.integer_pitch));
    if pitch_distance.is_some_and(|distance| distance > 1) {
        return false;
    }
    let common = rectangle_intersection(one.bounds, two.bounds);
    let minimum_width = one.bounds.width.min(two.bounds.width);
    let width_ratio = f64::from(common.width) / f64::from(minimum_width);
    if width_ratio <= 0.2 {
        return false;
    }
    if width_ratio >= 0.8 && pitch_distance.is_some_and(|distance| distance <= 1) {
        return true;
    }
    let one_area = one.bounds.width.wrapping_mul(one.bounds.height);
    let two_area = two.bounds.width.wrapping_mul(two.bounds.height);
    let minimum_area = one_area.min(two_area);
    let common_area = common.width.wrapping_mul(common.height);
    f64::from(common_area) / f64::from(minimum_area) > 0.25
}

fn validate_area(
    vertex: NativeSigVertexId,
    area: &NativeReductionAreaGeometry,
) -> Result<(), NativeReductionOverlapGeometryError> {
    for (component, polygon) in area.components.iter().enumerate() {
        if polygon.len() < 3
            || polygon
                .iter()
                .any(|point| !point.x.is_finite() || !point.y.is_finite())
        {
            return Err(NativeReductionOverlapGeometryError::InvalidArea { vertex, component });
        }
    }
    Ok(())
}

fn glyphs_intersect(
    one: &NativeReductionGlyphGeometry,
    two: &NativeReductionGlyphGeometry,
) -> bool {
    glyph_run_rectangles(one).any(|rectangle| glyph_intersects_rectangle(two, rectangle))
}

fn glyph_intersects_rectangle(
    glyph: &NativeReductionGlyphGeometry,
    rectangle: crate::native_sig::NativeSigBounds,
) -> bool {
    glyph_run_rectangles(glyph).any(|run| rectangles_intersect(run, rectangle))
}

fn glyph_intersects_area(
    glyph: &NativeReductionGlyphGeometry,
    area: &NativeReductionAreaGeometry,
) -> bool {
    glyph_run_rectangles(glyph).any(|run| area_intersects_rectangle(area, run))
}

fn glyph_run_rectangles(
    glyph: &NativeReductionGlyphGeometry,
) -> impl Iterator<Item = crate::native_sig::NativeSigBounds> + '_ {
    (0..glyph.run_table.sequence_count()).flat_map(move |sequence| {
        glyph
            .run_table
            .sequence(sequence)
            .unwrap_or_default()
            .iter()
            .map(move |run| match glyph.run_table.orientation() {
                Orientation::Horizontal => crate::native_sig::NativeSigBounds {
                    x: glyph
                        .left
                        .saturating_add(i32::try_from(run.start).unwrap_or(i32::MAX)),
                    y: glyph
                        .top
                        .saturating_add(i32::try_from(sequence).unwrap_or(i32::MAX)),
                    width: i32::try_from(run.length).unwrap_or(i32::MAX),
                    height: 1,
                },
                Orientation::Vertical => crate::native_sig::NativeSigBounds {
                    x: glyph
                        .left
                        .saturating_add(i32::try_from(sequence).unwrap_or(i32::MAX)),
                    y: glyph
                        .top
                        .saturating_add(i32::try_from(run.start).unwrap_or(i32::MAX)),
                    width: 1,
                    height: i32::try_from(run.length).unwrap_or(i32::MAX),
                },
            })
    })
}

fn areas_intersect(one: &NativeReductionAreaGeometry, two: &NativeReductionAreaGeometry) -> bool {
    one.components.iter().any(|left| {
        two.components
            .iter()
            .any(|right| convex_polygons_intersect(left, right))
    })
}

fn area_intersects_rectangle(
    area: &NativeReductionAreaGeometry,
    rectangle: crate::native_sig::NativeSigBounds,
) -> bool {
    if rectangle.width <= 0 || rectangle.height <= 0 {
        return false;
    }
    let left = f64::from(rectangle.x);
    let top = f64::from(rectangle.y);
    let right = left + f64::from(rectangle.width);
    let bottom = top + f64::from(rectangle.height);
    let polygon = [
        NativeReductionAreaPoint { x: left, y: top },
        NativeReductionAreaPoint { x: right, y: top },
        NativeReductionAreaPoint {
            x: right,
            y: bottom,
        },
        NativeReductionAreaPoint { x: left, y: bottom },
    ];
    area.components
        .iter()
        .any(|component| convex_polygons_intersect(component, &polygon))
}

fn convex_polygons_intersect(
    one: &[NativeReductionAreaPoint],
    two: &[NativeReductionAreaPoint],
) -> bool {
    !has_separating_axis(one, two) && !has_separating_axis(two, one)
}

fn has_separating_axis(
    source: &[NativeReductionAreaPoint],
    other: &[NativeReductionAreaPoint],
) -> bool {
    source.iter().enumerate().any(|(index, start)| {
        let stop = source[(index + 1) % source.len()];
        let axis = (-(stop.y - start.y), stop.x - start.x);
        if axis == (0.0, 0.0) {
            return false;
        }
        let range = |polygon: &[NativeReductionAreaPoint]| {
            polygon.iter().fold(
                (f64::INFINITY, f64::NEG_INFINITY),
                |(minimum, maximum), point| {
                    let projection = (point.x * axis.0) + (point.y * axis.1);
                    (minimum.min(projection), maximum.max(projection))
                },
            )
        };
        let (source_minimum, source_maximum) = range(source);
        let (other_minimum, other_maximum) = range(other);
        source_maximum <= other_minimum || other_maximum <= source_minimum
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeReductionOverlapDisposition {
    MirrorAccepted,
    CompatibleAccepted,
    BelowIou,
    BeyondRightEdge,
    PreciseRejected,
    StandardHeadStemAccepted,
    ExistingExclusion,
    SupportAccepted,
    ExclusionInserted,
}

/// One pair visited by Java's stable left-abscissa nested scan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeReductionOverlapPair {
    pub left: NativeSigVertexId,
    pub right: NativeSigVertexId,
    pub iou: Option<f64>,
    pub disposition: NativeReductionOverlapDisposition,
    pub exclusion: Option<NativeSigEdgeId>,
}

/// Exact scheduling and insertion result of `SigReducer.detectOverlaps()`.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeReductionOverlapTransaction {
    pub system_id: usize,
    /// Active non-header/non-disabled vertices, stable-sorted by bounds x.
    pub scan_order: Vec<NativeSigVertexId>,
    pub pairs: Vec<NativeReductionOverlapPair>,
    pub inserted_exclusions: Vec<NativeSigEdgeId>,
}

/// Result of `SigReducer.contextualizeAndPurge()` with `purgeWeaks=true`.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeReductionWeakPurgeTransaction {
    pub system_id: usize,
    pub contextualization: NativeSigContextualization,
    /// Weak vertices snapshotted in SIG order before any removal.
    pub removed_vertices: Vec<NativeSigVertexId>,
    /// Additional unique ensemble members removed by extensive cascade.
    pub cascaded_members: Vec<NativeSigVertexId>,
}

/// Consume terminal native STEMS state and reduce every exclusion which is
/// already present, in sheet system order.
///
/// A later production boundary will run Java's overlap discovery before this
/// solver and then continue with the foundation-specific consistency epochs.
pub fn reduce_native_stems_existing_exclusions(
    mut stems: NativeStemsRecognition,
) -> Result<NativeReductionExistingExclusionsRecognition, NativeSigError> {
    let mut initial_contextualizations = Vec::with_capacity(stems.systems.len());
    let mut transactions = Vec::with_capacity(stems.systems.len());
    for system in &mut stems.systems {
        let sig = &mut system.transaction.state_after.beam_state.sig;
        initial_contextualizations.push(sig.contextualize());
        transactions.push(reduce_native_sig_exclusions(sig)?);
    }
    Ok(NativeReductionExistingExclusionsRecognition {
        stems,
        initial_contextualizations,
        transactions,
    })
}

/// Port of Java `SIGraph.reduceExclusions()` over the terminal native SIG.
///
/// The caller must contextualize the graph before entry, as `SigReducer`
/// does. A branch's contextual grade is preferred when present, otherwise its
/// intrinsic grade is used. Equal branches remove the target, matching Java's
/// `(source < target) ? source : target` expression.
pub fn reduce_native_sig_exclusions(
    sig: &mut NativeSigSystem,
) -> Result<NativeReductionExclusionTransaction, NativeSigError> {
    sig.validate_integrity()?;
    let system_id = sig.system_id;
    let mut exclusions = sig
        .edges
        .iter()
        .filter(|edge| edge.active && edge.kind == NativeSigRelationKind::Exclusion)
        .map(|edge| NativeSigEdgeId(edge.ordinal))
        .collect::<Vec<_>>();
    let mut decisions = Vec::new();
    let mut removed_vertices = Vec::new();
    let mut contextualizations = Vec::new();

    loop {
        exclusions.retain(|id| sig.edges.get(id.0).is_some_and(|edge| edge.active));
        let mut best_grade = 0.0_f64;
        let mut best = None;
        for &id in &exclusions {
            let edge = &sig.edges[id.0];
            let source_grade = best_grade_of(sig, edge.source);
            let target_grade = best_grade_of(sig, edge.target);
            let contribution = source_grade.max(target_grade);
            if best_grade < contribution {
                best_grade = contribution;
                best = Some((
                    id,
                    NativeSigVertexId(edge.source),
                    source_grade,
                    NativeSigVertexId(edge.target),
                    target_grade,
                ));
            }
        }
        let Some((exclusion, source, source_grade, target, target_grade)) = best else {
            break;
        };
        let weaker = if source_grade < target_grade {
            source
        } else {
            target
        };
        let dying_ensembles = dying_ensembles(sig, weaker);

        // AbstractInter.remove(true) marks the weaker first, removes any dying
        // ensemble non-extensively, then removes the weaker from the graph.
        remove_vertex_extensively(sig, weaker, &dying_ensembles)?;

        decisions.push(NativeReductionExclusionDecision {
            exclusion,
            source,
            source_best_grade: source_grade,
            target,
            target_best_grade: target_grade,
            removed: weaker,
        });
        push_unique(&mut removed_vertices, weaker);
        for ensemble in dying_ensembles {
            push_unique(&mut removed_vertices, ensemble);
        }
        contextualizations.push(sig.contextualize());
        exclusions.retain(|id| *id != exclusion);
    }

    sig.validate_integrity()?;
    Ok(NativeReductionExclusionTransaction {
        system_id,
        decisions,
        removed_vertices,
        contextualizations,
    })
}

/// Java `SigReducer.checkHooksHaveStem()`.
///
/// A hook survives only when at least one live `BeamStemRelation` reaches a
/// non-center beam portion. The check snapshots hook insertion order before
/// mutation, exactly like `SIGraph.inters(BeamHookInter.class)`.
pub fn prune_native_foundation_hooks(
    sig: &mut NativeSigSystem,
) -> Result<NativeReductionBeamPruneTransaction, NativeSigError> {
    let hooks = sig
        .vertices
        .iter()
        .filter(|vertex| vertex.active && vertex.kind == NativeSigInterKind::BeamHook)
        .map(|vertex| NativeSigVertexId(vertex.ordinal))
        .collect::<Vec<_>>();
    prune_beams(sig, hooks, |sig, hook| {
        sig.edges.iter().any(|edge| {
            edge.active
                && edge.kind == NativeSigRelationKind::BeamStem
                && (edge.source == hook.0 || edge.target == hook.0)
                && edge.beam_portion != Some(NativeBeamPortion::Center)
        })
    })
}

/// Java `SigReducer.checkBeamsHaveBothStems()`.
///
/// Only standard `BeamInter` vertices are checked. Hooks and small beams have
/// their own Java paths. A standard beam must have a live BeamStem relation on
/// both horizontal portions; center relations do not satisfy either side.
pub fn prune_native_foundation_beams(
    sig: &mut NativeSigSystem,
) -> Result<NativeReductionBeamPruneTransaction, NativeSigError> {
    let beams = sig
        .vertices
        .iter()
        .filter(|vertex| vertex.active && vertex.kind == NativeSigInterKind::Beam)
        .map(|vertex| NativeSigVertexId(vertex.ordinal))
        .collect::<Vec<_>>();
    prune_beams(sig, beams, |sig, beam| {
        let has_left = sig.edges.iter().any(|edge| {
            edge.active
                && edge.kind == NativeSigRelationKind::BeamStem
                && (edge.source == beam.0 || edge.target == beam.0)
                && edge.beam_portion == Some(NativeBeamPortion::Left)
        });
        let has_right = sig.edges.iter().any(|edge| {
            edge.active
                && edge.kind == NativeSigRelationKind::BeamStem
                && (edge.source == beam.0 || edge.target == beam.0)
                && edge.beam_portion == Some(NativeBeamPortion::Right)
        });
        has_left && has_right
    })
}

/// Java `checkHeads()`'s `headHasStem` removal branch.
///
/// This is intentionally a sub-primitive rather than the complete check: the
/// subsequent `checkHeadSide` geometry pass remains a separate boundary.
pub fn prune_native_foundation_heads_without_stems(
    sig: &mut NativeSigSystem,
) -> Result<NativeReductionOrphanPruneTransaction, NativeSigError> {
    prune_orphans_without_relation_matching(
        sig,
        NativeSigInterKind::Head,
        NativeSigRelationKind::HeadStem,
        |vertex| vertex.shape.as_deref().is_some_and(is_stem_head_shape),
    )
}

/// Port Java `SigReducer.checkHeads()` including direction-aware wrong-side
/// pruning and the merged-grand-staff lookup exception.
pub fn prune_native_foundation_heads(
    sig: &mut NativeSigSystem,
    stem_medians: &BTreeMap<NativeSigVertexId, NativeStemLine>,
    head_identities: &BTreeMap<NativeSigVertexId, NativeReductionHeadIdentity>,
    merged_staff_pairs: &[(usize, usize)],
) -> Result<NativeReductionHeadCheckTransaction, NativeReductionFoundationPrefixError> {
    sig.validate_integrity()?;
    let head_order = sig
        .vertices
        .iter()
        .filter(|vertex| vertex.active && vertex.kind == NativeSigInterKind::Head)
        .map(|vertex| NativeSigVertexId(vertex.ordinal))
        .collect::<Vec<_>>();
    let mut mutations = Vec::new();
    let mut removed_ensembles = Vec::new();
    let mut removed_head_head_supports = Vec::new();

    for &head in &head_order {
        if sig.vertex(head.0).is_none() {
            continue;
        }
        let relations = active_head_stem_relations(sig, head);
        if relations.is_empty() {
            let dying = dying_ensembles(sig, head);
            remove_vertex_extensively(sig, head, &dying)?;
            mutations.push(NativeReductionHeadMutation::OrphanRemoved { head });
            for ensemble in dying {
                push_unique(&mut removed_ensembles, ensemble);
            }
            continue;
        }

        // Java snapshots getRelations(head, HeadStemRelation.class) before
        // visiting it in insertion order.
        for relation in relations {
            if !sig.edges.get(relation.0).is_some_and(|edge| edge.active) {
                continue;
            }
            let edge = sig.edges[relation.0];
            let stem = NativeSigVertexId(edge.target);
            let direction = native_stem_direction(sig, stem, stem_medians)?;
            if direction == 0 {
                remove_native_reduction_stem(
                    sig,
                    stem,
                    &mut removed_ensembles,
                    &mut removed_head_head_supports,
                )?;
                mutations.push(NativeReductionHeadMutation::UnknownDirectionStemRemoved {
                    head,
                    stem,
                    relation,
                });
                continue;
            }

            let payload = edge.head_stem.expect("validated HeadStem payload");
            let normal_side = matches!(
                (payload.head_side, direction),
                (NativeStemHeadSide::Left, 1) | (NativeStemHeadSide::Right, -1)
            );
            if normal_side {
                continue;
            }
            let identity = *head_identities.get(&head).ok_or(
                NativeReductionFoundationPrefixError::MissingHeadIdentity {
                    system_id: sig.system_id,
                    head,
                },
            )?;
            let target_side = opposite_head_side(payload.head_side);
            let has_opposite =
                (identity.integer_pitch - 1..=identity.integer_pitch + 1).any(|pitch| {
                    lookup_native_reduction_head(
                        sig,
                        stem,
                        target_side,
                        identity.staff_id,
                        pitch,
                        head_identities,
                        merged_staff_pairs,
                    )
                    .is_some()
                });
            if has_opposite {
                continue;
            }

            sig.remove_edge(relation)?;
            let exclusion = if payload.dy <= 0.0 && payload.dx <= 0.05 {
                insert_native_overlap_exclusion(sig, head, stem)?
            } else {
                None
            };
            mutations.push(NativeReductionHeadMutation::WrongSideRelationRemoved {
                head,
                stem,
                relation,
                exclusion,
            });
        }
    }
    sig.validate_integrity()?;
    Ok(NativeReductionHeadCheckTransaction {
        system_id: sig.system_id,
        head_order,
        mutations,
        removed_ensembles,
        removed_head_head_supports,
    })
}

fn active_head_stem_relations(
    sig: &NativeSigSystem,
    head: NativeSigVertexId,
) -> Vec<NativeSigEdgeId> {
    sig.edges
        .iter()
        .filter(|edge| {
            edge.active && edge.kind == NativeSigRelationKind::HeadStem && edge.source == head.0
        })
        .map(|edge| NativeSigEdgeId(edge.ordinal))
        .collect()
}

fn native_stem_direction(
    sig: &NativeSigSystem,
    stem: NativeSigVertexId,
    stem_medians: &BTreeMap<NativeSigVertexId, NativeStemLine>,
) -> Result<i8, NativeReductionFoundationPrefixError> {
    let &median =
        stem_medians
            .get(&stem)
            .ok_or(NativeReductionFoundationPrefixError::MissingStemMedian {
                system_id: sig.system_id,
                stem,
            })?;
    let extended = extended_stem_line(sig, stem, median);
    let mut links = sig
        .edges
        .iter()
        .filter(|edge| {
            edge.active
                && edge.target == stem.0
                && matches!(
                    edge.kind,
                    NativeSigRelationKind::HeadStem | NativeSigRelationKind::BeamStem
                )
        })
        .collect::<Vec<_>>();
    // Java Collections.sort is stable, retaining SIG relation order for equal
    // source best grades.
    links.sort_by(|left, right| {
        best_grade_of(sig, right.source).total_cmp(&best_grade_of(sig, left.source))
    });
    let (_, bottom) = ordered_stem_points(extended);
    let (top, _) = ordered_stem_points(extended);
    let middle = (top.y + bottom.y) / 2.0;
    for edge in links {
        match edge.kind {
            NativeSigRelationKind::HeadStem => {
                let payload = edge.head_stem.expect("validated HeadStem payload");
                let portion = head_stem_portion(
                    sig.vertices[edge.source].bounds.height,
                    extended,
                    payload.extension_point.y,
                );
                let percussion = sig.vertices[edge.source]
                    .shape
                    .as_deref()
                    .is_some_and(is_percussion_head_shape);
                if portion == NativeReductionStemPortion::Bottom
                    && (payload.head_side == NativeStemHeadSide::Right || percussion)
                {
                    return Ok(-1);
                }
                if portion == NativeReductionStemPortion::Top
                    && (payload.head_side == NativeStemHeadSide::Left || percussion)
                {
                    return Ok(1);
                }
            }
            NativeSigRelationKind::BeamStem => {
                let extension = edge.stem_extension.expect("validated BeamStem payload");
                return Ok(if extension.y < middle { -1 } else { 1 });
            }
            _ => unreachable!("filtered stem connection"),
        }
    }
    Ok(0)
}

fn is_percussion_head_shape(shape: &str) -> bool {
    shape.contains("_CROSS")
        || shape.contains("_DIAMOND")
        || shape.contains("_TRIANGLE_DOWN")
        || shape.contains("_CIRCLE_X")
}

fn is_stem_head_shape(shape: &str) -> bool {
    matches!(
        shape,
        "NOTEHEAD_BLACK"
            | "NOTEHEAD_VOID"
            | "NOTEHEAD_BLACK_SMALL"
            | "NOTEHEAD_VOID_SMALL"
            | "NOTEHEAD_CROSS"
            | "NOTEHEAD_CROSS_VOID"
            | "NOTEHEAD_DIAMOND_FILLED"
            | "NOTEHEAD_DIAMOND_VOID"
            | "NOTEHEAD_TRIANGLE_DOWN_FILLED"
            | "NOTEHEAD_TRIANGLE_DOWN_VOID"
            | "NOTEHEAD_CIRCLE_X"
            | "NOTEHEAD_CIRCLE_X_VOID"
    )
}

const fn opposite_head_side(side: NativeStemHeadSide) -> NativeStemHeadSide {
    match side {
        NativeStemHeadSide::Left => NativeStemHeadSide::Right,
        NativeStemHeadSide::Right => NativeStemHeadSide::Left,
    }
}

fn lookup_native_reduction_head(
    sig: &NativeSigSystem,
    stem: NativeSigVertexId,
    side: NativeStemHeadSide,
    mut staff_id: usize,
    mut pitch: i32,
    head_identities: &BTreeMap<NativeSigVertexId, NativeReductionHeadIdentity>,
    merged_staff_pairs: &[(usize, usize)],
) -> Option<NativeSigVertexId> {
    sig.vertex(stem.0)?;
    for &(first, last) in merged_staff_pairs {
        if staff_id == first && pitch == 7 {
            staff_id = last;
            pitch = -5;
            break;
        }
        if staff_id == last && pitch == -6 {
            staff_id = first;
            pitch = 5;
            break;
        }
    }
    sig.edges
        .iter()
        .filter(|edge| {
            edge.active
                && edge.kind == NativeSigRelationKind::HeadStem
                && edge.target == stem.0
                && edge
                    .head_stem
                    .is_some_and(|payload| payload.head_side == side)
        })
        .find_map(|edge| {
            let head = NativeSigVertexId(edge.source);
            head_identities
                .get(&head)
                .is_some_and(|identity| {
                    identity.staff_id == staff_id && identity.integer_pitch == pitch
                })
                .then_some(head)
        })
}

fn remove_native_reduction_stem(
    sig: &mut NativeSigSystem,
    stem: NativeSigVertexId,
    removed_ensembles: &mut Vec<NativeSigVertexId>,
    removed_head_head_supports: &mut Vec<NativeSigEdgeId>,
) -> Result<(), NativeSigError> {
    if sig.vertices[stem.0].grade >= 0.4 {
        let stem_heads = active_head_stem_relations_to(sig, stem)
            .into_iter()
            .map(|relation| NativeSigVertexId(sig.edges[relation.0].source))
            .collect::<BTreeSet<_>>();
        for &head in &stem_heads {
            let head_head_relations = sig
                .edges
                .iter()
                .filter(|edge| {
                    edge.active
                        && edge.kind == NativeSigRelationKind::HeadHead
                        && (edge.source == head.0 || edge.target == head.0)
                })
                .map(|edge| NativeSigEdgeId(edge.ordinal))
                .collect::<Vec<_>>();
            for relation in head_head_relations {
                if !sig.edges[relation.0].active {
                    continue;
                }
                let edge = sig.edges[relation.0];
                let similar = NativeSigVertexId(if edge.source == head.0 {
                    edge.target
                } else {
                    edge.source
                });
                if !stem_heads.contains(&similar)
                    || heads_share_other_good_stem(sig, head, similar, stem)
                {
                    continue;
                }
                sig.remove_edge(relation)?;
                removed_head_head_supports.push(relation);
            }
        }
    }
    let dying = dying_ensembles(sig, stem);
    remove_vertex_extensively(sig, stem, &dying)?;
    for ensemble in dying {
        push_unique(removed_ensembles, ensemble);
    }
    Ok(())
}

fn active_head_stem_relations_to(
    sig: &NativeSigSystem,
    stem: NativeSigVertexId,
) -> Vec<NativeSigEdgeId> {
    sig.edges
        .iter()
        .filter(|edge| {
            edge.active && edge.kind == NativeSigRelationKind::HeadStem && edge.target == stem.0
        })
        .map(|edge| NativeSigEdgeId(edge.ordinal))
        .collect()
}

fn heads_share_other_good_stem(
    sig: &NativeSigSystem,
    one: NativeSigVertexId,
    two: NativeSigVertexId,
    removed: NativeSigVertexId,
) -> bool {
    let one_stems = active_head_stem_relations(sig, one)
        .into_iter()
        .map(|relation| NativeSigVertexId(sig.edges[relation.0].target))
        .filter(|stem| *stem != removed)
        .collect::<BTreeSet<_>>();
    active_head_stem_relations(sig, two)
        .into_iter()
        .map(|relation| NativeSigVertexId(sig.edges[relation.0].target))
        .any(|stem| {
            stem != removed && one_stems.contains(&stem) && sig.vertices[stem.0].grade >= 0.4
        })
}

/// Java `checkStems()`'s `stem.getHeads().isEmpty()` removal branch.
///
/// The later `stemHasSingleHeadEnd` geometry pass is deliberately not claimed
/// by this graph-only primitive.
pub fn prune_native_foundation_stems_without_heads(
    sig: &mut NativeSigSystem,
) -> Result<NativeReductionOrphanPruneTransaction, NativeSigError> {
    prune_orphans_without_relation(
        sig,
        NativeSigInterKind::Stem,
        NativeSigRelationKind::HeadStem,
    )
}

/// Port Java `SigReducer.checkStems()` in its snapshotted stem order.
///
/// Orphan stems are removed extensively. For every remaining directed stem,
/// all head links at the tail end are collected against one extended-line
/// snapshot, removed together, and invading pairs become overlap exclusions.
pub fn prune_native_foundation_stems(
    sig: &mut NativeSigSystem,
    stem_medians: &BTreeMap<NativeSigVertexId, NativeStemLine>,
) -> Result<NativeReductionStemCheckTransaction, NativeReductionFoundationPrefixError> {
    sig.validate_integrity()?;
    let stem_order = sig
        .vertices
        .iter()
        .filter(|vertex| vertex.active && vertex.kind == NativeSigInterKind::Stem)
        .map(|vertex| NativeSigVertexId(vertex.ordinal))
        .collect::<Vec<_>>();
    let mut removed_orphan_stems = Vec::new();
    let mut tail_prunes = Vec::new();
    let mut removed_ensembles = Vec::new();

    for &stem in &stem_order {
        if sig.vertex(stem.0).is_none() {
            continue;
        }
        let head_links = active_head_stem_relations_to(sig, stem);
        if head_links.is_empty() {
            let dying = dying_ensembles(sig, stem);
            remove_vertex_extensively(sig, stem, &dying)?;
            removed_orphan_stems.push(stem);
            for ensemble in dying {
                push_unique(&mut removed_ensembles, ensemble);
            }
            continue;
        }

        let &median = stem_medians.get(&stem).ok_or(
            NativeReductionFoundationPrefixError::MissingStemMedian {
                system_id: sig.system_id,
                stem,
            },
        )?;
        // Java computes this line before computeDirection() and uses the same
        // immutable geometry for every relation in stemHasSingleHeadEnd().
        let extended = extended_stem_line(sig, stem, median);
        let direction = native_stem_direction(sig, stem, stem_medians)?;
        if direction == 0 {
            continue;
        }
        let forbidden = if direction > 0 {
            NativeReductionStemPortion::Bottom
        } else {
            NativeReductionStemPortion::Top
        };
        let removed_head_stem_edges = head_links
            .into_iter()
            .filter(|relation| {
                let edge = sig.edges[relation.0];
                let payload = edge.head_stem.expect("validated HeadStem payload");
                head_stem_portion(
                    sig.vertices[edge.source].bounds.height,
                    extended,
                    payload.extension_point.y,
                ) == forbidden
            })
            .collect::<Vec<_>>();
        if removed_head_stem_edges.is_empty() {
            continue;
        }

        let invading_heads = removed_head_stem_edges
            .iter()
            .filter_map(|relation| {
                let edge = sig.edges[relation.0];
                let payload = edge.head_stem.expect("validated HeadStem payload");
                (payload.dy <= 0.0 && payload.dx <= 0.05).then_some(NativeSigVertexId(edge.source))
            })
            .collect::<Vec<_>>();
        for &relation in &removed_head_stem_edges {
            sig.remove_edge(relation)?;
        }
        let mut added_exclusions = Vec::new();
        for head in invading_heads {
            if let Some(exclusion) = insert_native_overlap_exclusion(sig, head, stem)? {
                added_exclusions.push(exclusion);
            }
        }
        tail_prunes.push(NativeReductionStemTailPrune {
            stem,
            direction,
            removed_head_stem_edges,
            added_exclusions,
        });
    }
    sig.validate_integrity()?;
    let modification_count = removed_orphan_stems.len() + tail_prunes.len();
    Ok(NativeReductionStemCheckTransaction {
        system_id: sig.system_id,
        stem_order,
        removed_orphan_stems,
        tail_prunes,
        removed_ensembles,
        modification_count,
    })
}

/// Port Java `checkStemEndingHeads()` / `pruneStemHeads()`.
///
/// `stem_medians` is the exact `StemInter.median` authority retained by the
/// terminal STEMS system-stem registry. Each removal restarts the relation
/// scan after recomputing the extended line from the still-live connection
/// extension points, exactly like Java's labeled `do/while` loop.
pub fn prune_native_foundation_stem_ending_heads(
    sig: &mut NativeSigSystem,
    stem_medians: &BTreeMap<NativeSigVertexId, NativeStemLine>,
) -> Result<NativeReductionStemEndingTransaction, NativeReductionStemEndingError> {
    sig.validate_integrity()?;
    let stems = sig
        .vertices
        .iter()
        .filter(|vertex| vertex.active && vertex.kind == NativeSigInterKind::Stem)
        .map(|vertex| NativeSigVertexId(vertex.ordinal))
        .collect::<Vec<_>>();
    let mut modified_stems = Vec::new();
    for stem in stems {
        let &median =
            stem_medians
                .get(&stem)
                .ok_or(NativeReductionStemEndingError::MissingStemMedian {
                    system_id: sig.system_id,
                    stem,
                })?;
        let mut removed_head_stem_edges = Vec::new();
        let mut added_exclusions = Vec::new();
        loop {
            let extended = extended_stem_line(sig, stem, median);
            let links = sig
                .edges
                .iter()
                .filter(|edge| {
                    edge.active
                        && edge.kind == NativeSigRelationKind::HeadStem
                        && edge.target == stem.0
                })
                .map(|edge| NativeSigEdgeId(edge.ordinal))
                .collect::<Vec<_>>();
            let mut removed = false;
            for link_id in links {
                let edge = sig.edges[link_id.0];
                let Some(payload) = edge.head_stem else {
                    continue;
                };
                let head = NativeSigVertexId(edge.source);
                let portion = head_stem_portion(
                    sig.vertices[head.0].bounds.height,
                    extended,
                    payload.extension_point.y,
                );
                let wrong_side = matches!(
                    (portion, payload.head_side),
                    (NativeReductionStemPortion::Bottom, NativeStemHeadSide::Left)
                        | (NativeReductionStemPortion::Top, NativeStemHeadSide::Right)
                );
                if !wrong_side {
                    continue;
                }

                sig.remove_edge(link_id)?;
                removed_head_stem_edges.push(link_id);
                if payload.dx <= 0.05
                    && payload.dy <= 0.0
                    && let Some(exclusion) = insert_native_overlap_exclusion(sig, head, stem)?
                {
                    push_unique_edge(&mut added_exclusions, exclusion);
                }
                removed = true;
                break;
            }
            if !removed {
                break;
            }
        }
        if !removed_head_stem_edges.is_empty() {
            modified_stems.push(NativeReductionStemEndingPrune {
                stem,
                removed_head_stem_edges,
                added_exclusions,
            });
        }
    }
    sig.validate_integrity()?;
    Ok(NativeReductionStemEndingTransaction {
        system_id: sig.system_id,
        modified_stems,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativeReductionStemPortion {
    Top,
    Middle,
    Bottom,
}

fn head_stem_portion(
    head_height: i32,
    stem_line: NativeStemLine,
    extension_y: f64,
) -> NativeReductionStemPortion {
    let (top, bottom) = ordered_stem_points(stem_line);
    let margin = f64::from(head_height) * 0.275;
    let middle = (top.y + bottom.y) / 2.0;
    if extension_y >= middle {
        if extension_y > bottom.y - margin {
            NativeReductionStemPortion::Bottom
        } else {
            NativeReductionStemPortion::Middle
        }
    } else if extension_y < top.y + margin {
        NativeReductionStemPortion::Top
    } else {
        NativeReductionStemPortion::Middle
    }
}

fn extended_stem_line(
    sig: &NativeSigSystem,
    stem: NativeSigVertexId,
    median: NativeStemLine,
) -> NativeStemLine {
    let (mut top, mut bottom) = ordered_stem_points(median);
    for edge in sig.edges.iter().filter(|edge| {
        edge.active
            && edge.target == stem.0
            && matches!(
                edge.kind,
                NativeSigRelationKind::HeadStem | NativeSigRelationKind::BeamStem
            )
    }) {
        let extension = edge
            .head_stem
            .map(|payload| payload.extension_point)
            .or(edge.stem_extension);
        if let Some(extension) = extension {
            if extension.y < top.y {
                top = extension;
            }
            if extension.y > bottom.y {
                bottom = extension;
            }
        }
    }
    NativeStemLine {
        start: top,
        stop: bottom,
    }
}

fn ordered_stem_points(line: NativeStemLine) -> (NativeStemPoint, NativeStemPoint) {
    if line.start.y <= line.stop.y {
        (line.start, line.stop)
    } else {
        (line.stop, line.start)
    }
}

/// Java `SigReducer.contextualizeAndPurge()` for foundation reduction.
///
/// Frozen ownership is carried by the terminal native SIG from GRID and
/// HEADERS. Ledgers are never removed by this generic grade floor; Java
/// handles them in `checkLedgers()`.
pub fn contextualize_and_purge_native_weaks(
    sig: &mut NativeSigSystem,
) -> Result<NativeReductionWeakPurgeTransaction, NativeSigError> {
    sig.validate_integrity()?;
    let contextualization = sig.contextualize();
    let removed_vertices = sig
        .vertices
        .iter()
        .filter(|vertex| {
            vertex.active
                && vertex.kind != NativeSigInterKind::Ledger
                && !vertex.frozen
                && vertex
                    .contextual_grade
                    .is_some_and(|grade| grade < MIN_REDUCTION_CONTEXTUAL_GRADE)
        })
        .map(|vertex| NativeSigVertexId(vertex.ordinal))
        .collect::<Vec<_>>();
    let mut cascaded_members = Vec::new();
    for &vertex in &removed_vertices {
        if sig.vertex(vertex.0).is_none() {
            continue;
        }
        let dying = dying_ensembles(sig, vertex);
        let cascaded = unique_ensemble_members(sig, vertex);
        remove_vertex_extensively(sig, vertex, &dying)?;
        for member in cascaded {
            if !removed_vertices.contains(&member) {
                push_unique(&mut cascaded_members, member);
            }
        }
    }
    sig.validate_integrity()?;
    Ok(NativeReductionWeakPurgeTransaction {
        system_id: sig.system_id,
        contextualization,
        removed_vertices,
        cascaded_members,
    })
}

/// Port Java `SigReducer.detectOverlaps()` without approximating its precise
/// geometry dispatch.
///
/// This owns the stable abscissa sort, disabled/header filtering, beam-family
/// compatibility, Java rectangle IOU gate, early break, standard-head/stem
/// exception, support suppression, and normalized exclusion insertion.  Only
/// mirror membership and mutual precise overlap are delegated.
pub fn detect_native_reduction_overlaps(
    sig: &mut NativeSigSystem,
    geometry: &mut impl NativeReductionOverlapGeometry,
) -> Result<NativeReductionOverlapTransaction, NativeReductionOverlapError> {
    sig.validate_integrity()?;
    let mut scan_order = sig
        .vertices
        .iter()
        .filter(|vertex| {
            vertex.active && !is_overlap_disabled(vertex.kind) && !is_header_inter(vertex.kind)
        })
        .map(|vertex| NativeSigVertexId(vertex.ordinal))
        .collect::<Vec<_>>();
    // Slice sorting is stable, matching Stream.sorted with a comparator which
    // compares only the left x ordinate.
    scan_order.sort_by_key(|id| sig.vertices[id.0].bounds.x);

    let mut pairs = Vec::new();
    let mut inserted_exclusions = Vec::new();
    for left_index in 0..scan_order.len().saturating_sub(1) {
        let left = scan_order[left_index];
        let left_vertex = &sig.vertices[left.0];
        let left_bounds = left_vertex.bounds;
        let left_max_x = f64::from(left_bounds.x) + f64::from(left_bounds.width);
        for &right in &scan_order[left_index + 1..] {
            if geometry.is_mirror_entity(left, right)? {
                pairs.push(overlap_pair(
                    left,
                    right,
                    None,
                    NativeReductionOverlapDisposition::MirrorAccepted,
                    None,
                ));
                continue;
            }
            if overlap_compatible(sig.vertices[left.0].kind, sig.vertices[right.0].kind) {
                pairs.push(overlap_pair(
                    left,
                    right,
                    None,
                    NativeReductionOverlapDisposition::CompatibleAccepted,
                    None,
                ));
                continue;
            }

            let right_bounds = sig.vertices[right.0].bounds;
            let iou = java_rectangle_iou(left_bounds, right_bounds);
            if iou < MIN_REDUCTION_OVERLAP_IOU {
                let beyond = f64::from(right_bounds.x) > left_max_x;
                pairs.push(overlap_pair(
                    left,
                    right,
                    Some(iou),
                    if beyond {
                        NativeReductionOverlapDisposition::BeyondRightEdge
                    } else {
                        NativeReductionOverlapDisposition::BelowIou
                    },
                    None,
                ));
                if beyond {
                    break;
                }
                continue;
            }
            if !geometry.mutually_overlaps(left, right)? {
                pairs.push(overlap_pair(
                    left,
                    right,
                    Some(iou),
                    NativeReductionOverlapDisposition::PreciseRejected,
                    None,
                ));
                continue;
            }
            if is_standard_head_stem_pair(&sig.vertices[left.0], &sig.vertices[right.0]) {
                pairs.push(overlap_pair(
                    left,
                    right,
                    Some(iou),
                    NativeReductionOverlapDisposition::StandardHeadStemAccepted,
                    None,
                ));
                continue;
            }

            let (source, target) = if left.0 < right.0 {
                (left, right)
            } else {
                (right, left)
            };
            if let Some(existing) = active_exclusion_between(sig, source, target) {
                pairs.push(overlap_pair(
                    left,
                    right,
                    Some(iou),
                    NativeReductionOverlapDisposition::ExistingExclusion,
                    Some(existing),
                ));
                continue;
            }
            if has_support_between(sig, source, target) {
                pairs.push(overlap_pair(
                    left,
                    right,
                    Some(iou),
                    NativeReductionOverlapDisposition::SupportAccepted,
                    None,
                ));
                continue;
            }

            let exclusion = NativeSigEdgeId(sig.edges.len());
            sig.append_edge(NativeSigEdge {
                ordinal: exclusion.0,
                active: true,
                source: source.0,
                target: target.0,
                kind: NativeSigRelationKind::Exclusion,
                origin: NativeSigRelationOrigin::BaselineGraph,
                support: None,
                beam_portion: None,
                stem_extension: None,
                head_stem: None,
            })?;
            inserted_exclusions.push(exclusion);
            pairs.push(overlap_pair(
                left,
                right,
                Some(iou),
                NativeReductionOverlapDisposition::ExclusionInserted,
                Some(exclusion),
            ));
        }
    }
    sig.validate_integrity()?;
    Ok(NativeReductionOverlapTransaction {
        system_id: sig.system_id,
        scan_order,
        pairs,
        inserted_exclusions,
    })
}

fn overlap_pair(
    left: NativeSigVertexId,
    right: NativeSigVertexId,
    iou: Option<f64>,
    disposition: NativeReductionOverlapDisposition,
    exclusion: Option<NativeSigEdgeId>,
) -> NativeReductionOverlapPair {
    NativeReductionOverlapPair {
        left,
        right,
        iou,
        disposition,
        exclusion,
    }
}

fn is_overlap_disabled(kind: NativeSigInterKind) -> bool {
    matches!(
        kind,
        NativeSigInterKind::BeamGroup | NativeSigInterKind::Ledger
    )
}

fn is_header_inter(kind: NativeSigInterKind) -> bool {
    matches!(
        kind,
        NativeSigInterKind::Clef
            | NativeSigInterKind::KeyAlter
            | NativeSigInterKind::Key
            | NativeSigInterKind::TimeWhole
            | NativeSigInterKind::TimePair
    )
}

fn is_beam_family(kind: NativeSigInterKind) -> bool {
    matches!(
        kind,
        NativeSigInterKind::Beam | NativeSigInterKind::BeamHook | NativeSigInterKind::SmallBeam
    )
}

fn overlap_compatible(left: NativeSigInterKind, right: NativeSigInterKind) -> bool {
    is_beam_family(left) && is_beam_family(right)
}

fn is_standard_head_stem_pair(
    left: &crate::native_sig::NativeSigVertex,
    right: &crate::native_sig::NativeSigVertex,
) -> bool {
    let head = if left.kind == NativeSigInterKind::Head && right.kind == NativeSigInterKind::Stem {
        left
    } else if right.kind == NativeSigInterKind::Head && left.kind == NativeSigInterKind::Stem {
        right
    } else {
        return false;
    };
    head.shape.as_deref().is_some_and(|shape| {
        !matches!(
            shape,
            "NOTEHEAD_BLACK_SMALL" | "NOTEHEAD_VOID_SMALL" | "WHOLE_NOTE_SMALL" | "BREVE_SMALL"
        )
    })
}

fn active_exclusion_between(
    sig: &NativeSigSystem,
    source: NativeSigVertexId,
    target: NativeSigVertexId,
) -> Option<NativeSigEdgeId> {
    sig.edges
        .iter()
        .find(|edge| {
            edge.active
                && edge.source == source.0
                && edge.target == target.0
                && edge.kind == NativeSigRelationKind::Exclusion
        })
        .map(|edge| NativeSigEdgeId(edge.ordinal))
}

fn insert_native_overlap_exclusion(
    sig: &mut NativeSigSystem,
    one: NativeSigVertexId,
    two: NativeSigVertexId,
) -> Result<Option<NativeSigEdgeId>, NativeSigError> {
    let (source, target) = if one.0 < two.0 {
        (one, two)
    } else {
        (two, one)
    };
    if active_exclusion_between(sig, source, target).is_some()
        || has_support_between(sig, source, target)
    {
        return Ok(None);
    }
    let exclusion = NativeSigEdgeId(sig.edges.len());
    sig.append_edge(NativeSigEdge {
        ordinal: exclusion.0,
        active: true,
        source: source.0,
        target: target.0,
        kind: NativeSigRelationKind::Exclusion,
        origin: NativeSigRelationOrigin::BaselineGraph,
        support: None,
        beam_portion: None,
        stem_extension: None,
        head_stem: None,
    })?;
    Ok(Some(exclusion))
}

fn has_support_between(
    sig: &NativeSigSystem,
    source: NativeSigVertexId,
    target: NativeSigVertexId,
) -> bool {
    sig.edges.iter().any(|edge| {
        edge.active
            && ((edge.source == source.0 && edge.target == target.0)
                || (edge.source == target.0 && edge.target == source.0))
            && is_support_relation(edge.kind)
    })
}

fn is_support_relation(kind: NativeSigRelationKind) -> bool {
    matches!(
        kind,
        NativeSigRelationKind::NoExclusion
            | NativeSigRelationKind::BarConnection
            | NativeSigRelationKind::KeyAlters
            | NativeSigRelationKind::ClefKey
            | NativeSigRelationKind::BeamBeam
            | NativeSigRelationKind::BeamStem
            | NativeSigRelationKind::BeamHead
            | NativeSigRelationKind::BeamRest
            | NativeSigRelationKind::HeadStem
            | NativeSigRelationKind::HeadHead
    )
}

/// Java `GeoUtil.iou(Rectangle, Rectangle)`, including its signed Rectangle
/// intersection dimensions rather than clamping them independently.
fn java_rectangle_iou(
    one: crate::native_sig::NativeSigBounds,
    two: crate::native_sig::NativeSigBounds,
) -> f64 {
    let inter_left = one.x.max(two.x);
    let inter_top = one.y.max(two.y);
    let inter_right = one
        .x
        .wrapping_add(one.width)
        .min(two.x.wrapping_add(two.width));
    let inter_bottom = one
        .y
        .wrapping_add(one.height)
        .min(two.y.wrapping_add(two.height));
    let inter_area = inter_right
        .wrapping_sub(inter_left)
        .wrapping_mul(inter_bottom.wrapping_sub(inter_top));
    if inter_area == 0 {
        return 0.0;
    }
    let one_area = one.width.wrapping_mul(one.height);
    let two_area = two.width.wrapping_mul(two.height);
    let union_area = one_area.wrapping_add(two_area).wrapping_sub(inter_area);
    f64::from(inter_area) / f64::from(union_area)
}

fn prune_beams(
    sig: &mut NativeSigSystem,
    candidates: Vec<NativeSigVertexId>,
    survives: impl Fn(&NativeSigSystem, NativeSigVertexId) -> bool,
) -> Result<NativeReductionBeamPruneTransaction, NativeSigError> {
    sig.validate_integrity()?;
    let mut removed_beams = Vec::new();
    let mut removed_groups = Vec::new();
    for candidate in candidates {
        if !survives(sig, candidate) {
            let dying = dying_ensembles(sig, candidate);
            remove_vertex_extensively(sig, candidate, &dying)?;
            removed_beams.push(candidate);
            for group in dying {
                push_unique(&mut removed_groups, group);
            }
        }
    }
    sig.validate_integrity()?;
    Ok(NativeReductionBeamPruneTransaction {
        system_id: sig.system_id,
        removed_beams,
        removed_groups,
    })
}

fn prune_orphans_without_relation(
    sig: &mut NativeSigSystem,
    kind: NativeSigInterKind,
    required_relation: NativeSigRelationKind,
) -> Result<NativeReductionOrphanPruneTransaction, NativeSigError> {
    prune_orphans_without_relation_matching(sig, kind, required_relation, |_| true)
}

fn prune_orphans_without_relation_matching(
    sig: &mut NativeSigSystem,
    kind: NativeSigInterKind,
    required_relation: NativeSigRelationKind,
    matches: impl Fn(&crate::native_sig::NativeSigVertex) -> bool,
) -> Result<NativeReductionOrphanPruneTransaction, NativeSigError> {
    sig.validate_integrity()?;
    let candidates = sig
        .vertices
        .iter()
        .filter(|vertex| vertex.active && vertex.kind == kind && matches(vertex))
        .map(|vertex| NativeSigVertexId(vertex.ordinal))
        .collect::<Vec<_>>();
    let mut removed_vertices = Vec::new();
    let mut removed_ensembles = Vec::new();
    for candidate in candidates {
        let linked = sig.edges.iter().any(|edge| {
            edge.active
                && edge.kind == required_relation
                && (edge.source == candidate.0 || edge.target == candidate.0)
        });
        if !linked {
            let dying = dying_ensembles(sig, candidate);
            remove_vertex_extensively(sig, candidate, &dying)?;
            removed_vertices.push(candidate);
            for ensemble in dying {
                push_unique(&mut removed_ensembles, ensemble);
            }
        }
    }
    sig.validate_integrity()?;
    Ok(NativeReductionOrphanPruneTransaction {
        system_id: sig.system_id,
        kind,
        removed_vertices,
        removed_ensembles,
    })
}

fn remove_vertex_extensively(
    sig: &mut NativeSigSystem,
    vertex: NativeSigVertexId,
    dying_ensembles: &[NativeSigVertexId],
) -> Result<(), NativeSigError> {
    let unique_members = unique_ensemble_members(sig, vertex);
    for &ensemble in dying_ensembles {
        sig.remove_vertex(ensemble)?;
    }
    for member in unique_members {
        if sig.vertex(member.0).is_some() {
            // Java removes unique members with `extensive=false`.
            sig.remove_vertex(member)?;
        }
    }
    if sig.vertex(vertex.0).is_some() {
        sig.remove_vertex(vertex)?;
    }
    Ok(())
}

fn unique_ensemble_members(
    sig: &NativeSigSystem,
    ensemble: NativeSigVertexId,
) -> Vec<NativeSigVertexId> {
    if sig
        .vertex(ensemble.0)
        .is_none_or(|vertex| vertex.kind != NativeSigInterKind::BeamGroup)
    {
        return Vec::new();
    }
    sig.edges
        .iter()
        .filter(|edge| {
            edge.active
                && edge.kind == NativeSigRelationKind::Containment
                && edge.source == ensemble.0
        })
        .filter_map(|edge| {
            let other_ensemble = sig.edges.iter().any(|candidate| {
                candidate.active
                    && candidate.kind == NativeSigRelationKind::Containment
                    && candidate.target == edge.target
                    && candidate.source != ensemble.0
                    && sig.vertices[candidate.source].active
            });
            (!other_ensemble).then_some(NativeSigVertexId(edge.target))
        })
        .collect()
}

fn best_grade_of(sig: &NativeSigSystem, ordinal: usize) -> f64 {
    let vertex = &sig.vertices[ordinal];
    vertex.contextual_grade.unwrap_or(vertex.grade)
}

fn dying_ensembles(sig: &NativeSigSystem, member: NativeSigVertexId) -> Vec<NativeSigVertexId> {
    sig.edges
        .iter()
        .filter(|edge| {
            edge.active
                && edge.kind == NativeSigRelationKind::Containment
                && edge.target == member.0
                && sig.vertices[edge.source].active
                && sig.vertices[edge.source].kind == NativeSigInterKind::BeamGroup
        })
        .filter_map(|edge| {
            let member_count = sig
                .edges
                .iter()
                .filter(|candidate| {
                    candidate.active
                        && candidate.kind == NativeSigRelationKind::Containment
                        && candidate.source == edge.source
                        && sig.vertices[candidate.target].active
                })
                .count();
            (member_count == 1).then_some(NativeSigVertexId(edge.source))
        })
        .collect()
}

fn push_unique(vertices: &mut Vec<NativeSigVertexId>, vertex: NativeSigVertexId) {
    if !vertices.contains(&vertex) {
        vertices.push(vertex);
    }
}

fn push_unique_edge(edges: &mut Vec<NativeSigEdgeId>, edge: NativeSigEdgeId) {
    if !edges.contains(&edge) {
        edges.push(edge);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_sig::{
        NativeSigBounds, NativeSigEdge, NativeSigHeadStemPayload, NativeSigReductionStaff,
        NativeSigRelationOrigin, NativeSigSupport, NativeSigSystemBindings, NativeSigVertex,
    };
    use crate::stems_step::{NativeStemHeadSide, NativeStemPoint};
    use audiveris_image::system_population::{BoundarySegment, StaffBoundary};
    use std::collections::BTreeSet;

    #[derive(Default)]
    struct ScriptedOverlapGeometry {
        mirrors: BTreeSet<(usize, usize)>,
        overlaps: BTreeSet<(usize, usize)>,
        precise_calls: Vec<(usize, usize)>,
    }

    impl NativeReductionOverlapGeometry for ScriptedOverlapGeometry {
        fn is_mirror_entity(
            &mut self,
            left: NativeSigVertexId,
            right: NativeSigVertexId,
        ) -> Result<bool, NativeReductionOverlapGeometryError> {
            Ok(self.mirrors.contains(&(left.0, right.0)))
        }

        fn mutually_overlaps(
            &mut self,
            left: NativeSigVertexId,
            right: NativeSigVertexId,
        ) -> Result<bool, NativeReductionOverlapGeometryError> {
            self.precise_calls.push((left.0, right.0));
            Ok(self.overlaps.contains(&(left.0, right.0)))
        }
    }

    fn vertex(ordinal: usize, kind: NativeSigInterKind, grade: f64) -> NativeSigVertex {
        NativeSigVertex {
            ordinal,
            active: true,
            removed: false,
            frozen: false,
            kind,
            shape: None,
            grade,
            contextual_grade: Some(grade),
            bounds: NativeSigBounds {
                x: ordinal as i32 * 10,
                y: 0,
                width: 4,
                height: 10,
            },
            abnormal: false,
            beam_geometry: None,
        }
    }

    fn edge(
        ordinal: usize,
        source: usize,
        target: usize,
        kind: NativeSigRelationKind,
    ) -> NativeSigEdge {
        NativeSigEdge {
            ordinal,
            active: true,
            source,
            target,
            kind,
            origin: NativeSigRelationOrigin::BaselineGraph,
            support: None,
            beam_portion: None,
            stem_extension: None,
            head_stem: None,
        }
    }

    fn beam_stem_edge(
        ordinal: usize,
        beam: usize,
        stem: usize,
        portion: NativeBeamPortion,
    ) -> NativeSigEdge {
        NativeSigEdge {
            support: Some(NativeSigSupport {
                grade: 0.8,
                bar_connection_impacts: None,
            }),
            beam_portion: Some(portion),
            stem_extension: Some(NativeStemPoint { x: 2.0, y: 3.0 }),
            ..edge(ordinal, beam, stem, NativeSigRelationKind::BeamStem)
        }
    }

    fn head_stem_edge(ordinal: usize, head: usize, stem: usize) -> NativeSigEdge {
        NativeSigEdge {
            support: Some(NativeSigSupport {
                grade: 0.8,
                bar_connection_impacts: None,
            }),
            head_stem: Some(NativeSigHeadStemPayload {
                dx: 0.1,
                dy: 0.2,
                head_side: NativeStemHeadSide::Left,
                extension_point: NativeStemPoint { x: 2.0, y: 3.0 },
                consistency: 1.0,
                manual: false,
            }),
            ..edge(ordinal, head, stem, NativeSigRelationKind::HeadStem)
        }
    }

    fn shaped_head(ordinal: usize, shape: &str, y: i32) -> NativeSigVertex {
        let mut head = vertex(ordinal, NativeSigInterKind::Head, 0.8);
        head.shape = Some(shape.to_owned());
        head.bounds = NativeSigBounds {
            x: 8,
            y,
            width: 6,
            height: 6,
        };
        head
    }

    fn horizontal_beam(ordinal: usize, y: f64) -> NativeSigVertex {
        let mut beam = vertex(ordinal, NativeSigInterKind::Beam, 0.8);
        beam.bounds = NativeSigBounds {
            x: 0,
            y: (y - 2.0) as i32,
            width: 100,
            height: 4,
        };
        beam.beam_geometry = Some(crate::native_sig::NativeSigBeamGeometry {
            x1: 0.0,
            y1: y,
            x2: 100.0,
            y2: y,
            height: 4.0,
        });
        beam
    }

    fn beam_support_edge(ordinal: usize, one: usize, two: usize) -> NativeSigEdge {
        NativeSigEdge {
            support: Some(NativeSigSupport {
                grade: 1.0,
                bar_connection_impacts: None,
            }),
            ..edge(ordinal, one, two, NativeSigRelationKind::BeamBeam)
        }
    }

    fn horizontal_staff_line(y: f64) -> StaffBoundary {
        StaffBoundary {
            segments: vec![BoundarySegment::Line {
                start: (0.0, y),
                end: (200.0, y),
            }],
        }
    }

    fn reduction_staff(
        staff_id: usize,
        top: f64,
        bottom: f64,
        ledger_map: BTreeMap<i32, Vec<NativeSigVertexId>>,
    ) -> NativeSigReductionStaff {
        NativeSigReductionStaff {
            staff_id,
            tablature: false,
            specific_interline: 10,
            line_count: 5,
            first_line: horizontal_staff_line(top),
            last_line: horizontal_staff_line(bottom),
            ledger_lines: BTreeMap::new(),
            ledger_map,
        }
    }

    fn reduction_bindings(
        system_id: usize,
        staffs: Vec<NativeSigReductionStaff>,
        ledger_vertices: BTreeMap<usize, NativeSigVertexId>,
        heads: BTreeMap<NativeSigVertexId, NativeReductionHeadGeometry>,
    ) -> NativeSigSystemBindings {
        NativeSigSystemBindings {
            system_id,
            beam_vertices: BTreeMap::new(),
            beam_group_vertices: BTreeMap::new(),
            stem_vertices: BTreeMap::new(),
            head_vertices: BTreeMap::new(),
            ledger_vertices,
            reduction_interline: 10,
            reduction_staffs: staffs,
            merged_staff_pairs: Vec::new(),
            overlap_geometry: heads
                .into_iter()
                .map(|(vertex, head)| {
                    let bounds = NativeSigBounds {
                        x: 0,
                        y: 0,
                        width: 1,
                        height: 1,
                    };
                    (
                        vertex,
                        NativeReductionInterGeometry {
                            bounds,
                            core_bounds: bounds,
                            implicit: false,
                            glyph: None,
                            area: None,
                            head: Some(head),
                            ensemble_members: Vec::new(),
                        },
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn ledger_pruning_repeats_until_an_inner_ledger_loses_outer_support() {
        let mut inner = vertex(0, NativeSigInterKind::Ledger, 0.8);
        inner.bounds = NativeSigBounds {
            x: 20,
            y: 50,
            width: 20,
            height: 2,
        };
        let mut outer = vertex(1, NativeSigInterKind::Ledger, 0.8);
        outer.bounds = NativeSigBounds {
            x: 25,
            y: 70,
            width: 20,
            height: 2,
        };
        let mut sig = NativeSigSystem {
            system_id: 50,
            vertices: vec![inner, outer],
            edges: Vec::new(),
        };
        let staff = reduction_staff(
            7,
            10.0,
            50.0,
            BTreeMap::from([
                (1, vec![NativeSigVertexId(0)]),
                (2, vec![NativeSigVertexId(1)]),
            ]),
        );
        let mut bindings = reduction_bindings(
            50,
            vec![staff],
            BTreeMap::from([(100, NativeSigVertexId(0)), (101, NativeSigVertexId(1))]),
            BTreeMap::new(),
        );

        let transaction = prune_native_foundation_ledgers(&mut sig, &mut bindings).unwrap();

        assert_eq!(
            transaction.removals,
            vec![
                NativeReductionLedgerRemoval {
                    pass: 1,
                    staff_id: 7,
                    ledger_index: 2,
                    ledger: NativeSigVertexId(1),
                },
                NativeReductionLedgerRemoval {
                    pass: 2,
                    staff_id: 7,
                    ledger_index: 1,
                    ledger: NativeSigVertexId(0),
                },
            ]
        );
        assert_eq!(transaction.modification_count, 2);
        assert!(sig.vertices.iter().all(|ledger| ledger.removed));
    }

    #[test]
    fn ledger_support_accepts_the_ledger_pitch_and_the_next_outward_pitch() {
        let mut ledger = vertex(0, NativeSigInterKind::Ledger, 0.8);
        ledger.bounds = NativeSigBounds {
            x: 20,
            y: 50,
            width: 20,
            height: 2,
        };
        let mut head = shaped_head(1, "NOTEHEAD_BLACK", 48);
        head.bounds.x = 25;
        let mut sig = NativeSigSystem {
            system_id: 51,
            vertices: vec![ledger, head],
            edges: Vec::new(),
        };
        let staff = reduction_staff(
            8,
            10.0,
            50.0,
            BTreeMap::from([(1, vec![NativeSigVertexId(0)])]),
        );
        let mut bindings = reduction_bindings(
            51,
            vec![staff],
            BTreeMap::from([(100, NativeSigVertexId(0))]),
            BTreeMap::from([(
                NativeSigVertexId(1),
                NativeReductionHeadGeometry {
                    staff_id: Some(8),
                    integer_pitch: 7,
                },
            )]),
        );

        let transaction = prune_native_foundation_ledgers(&mut sig, &mut bindings).unwrap();

        assert!(transaction.removals.is_empty());
        assert!(sig.vertex(0).is_some());
    }

    #[test]
    fn shared_ledger_chooses_the_farther_staff_and_recomputes_head_pitch() {
        let mut shared_ledger = vertex(0, NativeSigInterKind::Ledger, 0.8);
        shared_ledger.bounds = NativeSigBounds {
            x: 20,
            y: 58,
            width: 20,
            height: 4,
        };
        let mut lower_first = vertex(1, NativeSigInterKind::Ledger, 0.8);
        lower_first.bounds = NativeSigBounds {
            x: 120,
            y: 70,
            width: 20,
            height: 2,
        };
        let mut head = shaped_head(2, "NOTEHEAD_BLACK", 52);
        head.bounds = NativeSigBounds {
            x: 25,
            y: 52,
            width: 6,
            height: 6,
        };
        let mut sig = NativeSigSystem {
            system_id: 52,
            vertices: vec![shared_ledger, lower_first, head],
            edges: Vec::new(),
        };
        let upper = reduction_staff(
            10,
            0.0,
            40.0,
            BTreeMap::from([(1, vec![NativeSigVertexId(0)])]),
        );
        let mut lower = reduction_staff(
            11,
            80.0,
            120.0,
            BTreeMap::from([
                (-2, vec![NativeSigVertexId(0)]),
                (-1, vec![NativeSigVertexId(1)]),
            ]),
        );
        lower.ledger_lines.insert(-1, horizontal_staff_line(70.0));
        lower.ledger_lines.insert(-2, horizontal_staff_line(60.0));
        let mut bindings = reduction_bindings(
            52,
            vec![upper, lower],
            BTreeMap::from([(100, NativeSigVertexId(0)), (101, NativeSigVertexId(1))]),
            BTreeMap::from([(
                NativeSigVertexId(2),
                NativeReductionHeadGeometry {
                    staff_id: Some(10),
                    integer_pitch: 6,
                },
            )]),
        );

        let transaction = prune_native_foundation_ledgers(&mut sig, &mut bindings).unwrap();

        assert_eq!(transaction.shared_fixes.len(), 1);
        assert_eq!(transaction.shared_fixes[0].owner_staff_id, Some(11));
        assert!(
            !bindings.reduction_staffs[0]
                .ledger_map
                .values()
                .flatten()
                .any(|ledger| *ledger == NativeSigVertexId(0))
        );
        assert_eq!(
            bindings.overlap_geometry[&NativeSigVertexId(2)]
                .head
                .expect("head identity"),
            NativeReductionHeadGeometry {
                staff_id: Some(11),
                integer_pitch: -9,
            }
        );
        assert!(
            sig.vertex(0).is_some(),
            "reassigned ledger is head-supported"
        );
    }

    #[test]
    fn headless_shared_column_is_detached_from_both_staffs_without_sig_removal() {
        let mut shared_ledger = vertex(0, NativeSigInterKind::Ledger, 0.8);
        shared_ledger.bounds = NativeSigBounds {
            x: 20,
            y: 58,
            width: 20,
            height: 4,
        };
        let mut lower_first = vertex(1, NativeSigInterKind::Ledger, 0.8);
        lower_first.bounds.x = 120;
        let mut sig = NativeSigSystem {
            system_id: 53,
            vertices: vec![shared_ledger, lower_first],
            edges: Vec::new(),
        };
        let upper = reduction_staff(
            10,
            0.0,
            40.0,
            BTreeMap::from([(1, vec![NativeSigVertexId(0)])]),
        );
        let lower = reduction_staff(
            11,
            80.0,
            120.0,
            BTreeMap::from([
                (-2, vec![NativeSigVertexId(0)]),
                (-1, vec![NativeSigVertexId(1)]),
            ]),
        );
        let mut bindings = reduction_bindings(
            53,
            vec![upper, lower],
            BTreeMap::from([(100, NativeSigVertexId(0)), (101, NativeSigVertexId(1))]),
            BTreeMap::new(),
        );

        let transaction = prune_native_foundation_ledgers(&mut sig, &mut bindings).unwrap();

        assert_eq!(transaction.shared_fixes[0].owner_staff_id, None);
        assert!(sig.vertex(0).is_some());
        assert!(bindings.reduction_staffs.iter().all(|staff| {
            !staff
                .ledger_map
                .values()
                .flatten()
                .any(|ledger| *ledger == NativeSigVertexId(0))
        }));
        assert_eq!(transaction.modification_count, 2);
    }

    #[test]
    fn chord_analysis_ports_intersection_duration_support_and_beam_size_order() {
        let mut stem = vertex(0, NativeSigInterKind::Stem, 0.9);
        stem.bounds = NativeSigBounds {
            x: 10,
            y: 0,
            width: 2,
            height: 100,
        };
        let mut sig = NativeSigSystem {
            system_id: 40,
            vertices: vec![
                stem,
                shaped_head(1, "NOTEHEAD_BLACK", 10),
                shaped_head(2, "NOTEHEAD_BLACK", 30),
                shaped_head(3, "NOTEHEAD_VOID", 50),
                shaped_head(4, "NOTEHEAD_BLACK", 70),
                vertex(5, NativeSigInterKind::Beam, 0.8),
                vertex(6, NativeSigInterKind::SmallBeam, 0.8),
            ],
            edges: vec![
                head_stem_edge(0, 1, 0),
                head_stem_edge(1, 2, 0),
                head_stem_edge(2, 3, 0),
                beam_stem_edge(3, 5, 0, NativeBeamPortion::Left),
                beam_stem_edge(4, 6, 0, NativeBeamPortion::Right),
            ],
        };
        let medians = BTreeMap::from([(
            NativeSigVertexId(0),
            NativeStemLine {
                start: NativeStemPoint { x: 11.0, y: 0.0 },
                stop: NativeStemPoint { x: 11.0, y: 100.0 },
            },
        )]);

        let transaction = analyze_native_foundation_chords(&mut sig, &medians).unwrap();

        assert_eq!(transaction.scanned_stems, vec![NativeSigVertexId(0)]);
        assert_eq!(
            transaction.intersected_head_exclusions,
            vec![NativeSigEdgeId(5)]
        );
        assert_eq!(
            transaction.incompatible_exclusions,
            [6, 7, 9, 10, 11, 12]
                .map(NativeSigEdgeId)
                .into_iter()
                .collect::<Vec<_>>()
        );
        assert_eq!(transaction.head_head_supports, vec![NativeSigEdgeId(8)]);
        assert_eq!(
            sig.edges
                .iter()
                .skip(5)
                .map(|edge| (edge.source, edge.target, edge.kind))
                .collect::<Vec<_>>(),
            vec![
                (0, 4, NativeSigRelationKind::Exclusion),
                (1, 3, NativeSigRelationKind::Exclusion),
                (2, 3, NativeSigRelationKind::Exclusion),
                (1, 2, NativeSigRelationKind::HeadHead),
                (5, 6, NativeSigRelationKind::Exclusion),
                (1, 6, NativeSigRelationKind::Exclusion),
                (2, 6, NativeSigRelationKind::Exclusion),
                (3, 6, NativeSigRelationKind::Exclusion),
            ]
        );
        assert_eq!(sig.edges[8].support.expect("head support").grade, 1.0);
        assert_eq!(
            insert_native_overlap_exclusion(&mut sig, NativeSigVertexId(1), NativeSigVertexId(2))
                .unwrap(),
            None,
            "HeadHeadRelation suppresses a later exclusion"
        );
        assert_eq!(sig.edges.len(), 13);
    }

    #[test]
    fn foundation_pass_reaches_stems_and_its_weak_purge_in_java_order() {
        let mut stem = vertex(0, NativeSigInterKind::Stem, 0.9);
        stem.bounds = NativeSigBounds {
            x: 10,
            y: 0,
            width: 2,
            height: 100,
        };
        let mut sig = NativeSigSystem {
            system_id: 41,
            vertices: vec![stem, shaped_head(1, "NOTEHEAD_BLACK", 80)],
            edges: vec![head_stem_edge(0, 1, 0)],
        };
        sig.edges[0].head_stem.as_mut().expect("payload").head_side = NativeStemHeadSide::Right;
        sig.edges[0]
            .head_stem
            .as_mut()
            .expect("payload")
            .extension_point = NativeStemPoint { x: 11.0, y: 99.0 };
        let medians = BTreeMap::from([(
            NativeSigVertexId(0),
            NativeStemLine {
                start: NativeStemPoint { x: 11.0, y: 0.0 },
                stop: NativeStemPoint { x: 11.0, y: 100.0 },
            },
        )]);
        let mut geometry = ScriptedOverlapGeometry::default();
        let mut bindings = NativeSigSystemBindings {
            system_id: 41,
            beam_vertices: BTreeMap::new(),
            beam_group_vertices: BTreeMap::new(),
            stem_vertices: BTreeMap::new(),
            head_vertices: BTreeMap::new(),
            ledger_vertices: BTreeMap::new(),
            reduction_interline: 10,
            reduction_staffs: Vec::new(),
            merged_staff_pairs: Vec::new(),
            overlap_geometry: BTreeMap::new(),
        };

        let head_identities = BTreeMap::from([(
            NativeSigVertexId(1),
            NativeReductionHeadIdentity {
                staff_id: 1,
                integer_pitch: 0,
            },
        )]);
        let transaction = reduce_native_foundation_prefix(
            &mut sig,
            &mut bindings,
            &mut geometry,
            &medians,
            &head_identities,
            &[],
        )
        .expect("exact foundations prefix");

        assert_eq!(transaction.overlap.system_id, 41);
        assert_eq!(
            transaction.chord_analysis.scanned_stems,
            vec![NativeSigVertexId(0)]
        );
        assert!(transaction.initial_weak_purge.removed_vertices.is_empty());
        assert_eq!(transaction.consistency_passes.len(), 1);
        let pass = &transaction.consistency_passes[0];
        assert_eq!(pass.modification_count, 0);
        assert!(pass.stem_ending.modified_stems.is_empty());
        assert!(pass.post_stem_ending_weak_purge.removed_vertices.is_empty());
        assert!(pass.heads.mutations.is_empty(), "{:?}", pass.heads);
        assert!(pass.hooks.removed_beams.is_empty());
        assert!(pass.beams.removed_beams.is_empty());
        assert!(pass.ledgers.removals.is_empty());
        assert!(pass.stems.removed_orphan_stems.is_empty());
        assert!(pass.stems.tail_prunes.is_empty());
        assert_eq!(pass.stems.modification_count, 0);
        assert!(pass.post_stems_weak_purge.removed_vertices.is_empty());
        assert!(sig.vertex(0).is_some());
        assert!(sig.vertex(1).is_some());
    }

    #[test]
    fn foundation_fixed_point_revisits_a_head_orphaned_by_the_prior_stem_pass() {
        let mut stem = vertex(0, NativeSigInterKind::Stem, 0.9);
        stem.bounds = NativeSigBounds {
            x: 10,
            y: 0,
            width: 2,
            height: 100,
        };
        let mut top = shaped_head(1, "NOTEHEAD_BLACK", 0);
        top.grade = 0.95;
        top.contextual_grade = Some(0.95);
        let bottom = shaped_head(2, "NOTEHEAD_BLACK", 95);
        let mut top_link = head_stem_edge(0, 1, 0);
        top_link.head_stem.as_mut().unwrap().head_side = NativeStemHeadSide::Left;
        top_link.head_stem.as_mut().unwrap().extension_point = NativeStemPoint { x: 11.0, y: 2.0 };
        let mut bottom_link = head_stem_edge(1, 2, 0);
        bottom_link.head_stem.as_mut().unwrap().head_side = NativeStemHeadSide::Right;
        bottom_link.head_stem.as_mut().unwrap().extension_point =
            NativeStemPoint { x: 11.0, y: 98.0 };
        let mut sig = NativeSigSystem {
            system_id: 42,
            vertices: vec![stem, top, bottom],
            edges: vec![top_link, bottom_link],
        };
        let medians = BTreeMap::from([(
            NativeSigVertexId(0),
            NativeStemLine {
                start: NativeStemPoint { x: 11.0, y: 10.0 },
                stop: NativeStemPoint { x: 11.0, y: 90.0 },
            },
        )]);
        let identities = BTreeMap::from([
            (
                NativeSigVertexId(1),
                NativeReductionHeadIdentity {
                    staff_id: 1,
                    integer_pitch: 0,
                },
            ),
            (
                NativeSigVertexId(2),
                NativeReductionHeadIdentity {
                    staff_id: 1,
                    integer_pitch: 0,
                },
            ),
        ]);
        let mut bindings = NativeSigSystemBindings {
            system_id: 42,
            beam_vertices: BTreeMap::new(),
            beam_group_vertices: BTreeMap::new(),
            stem_vertices: BTreeMap::new(),
            head_vertices: BTreeMap::new(),
            ledger_vertices: BTreeMap::new(),
            reduction_interline: 10,
            reduction_staffs: Vec::new(),
            merged_staff_pairs: Vec::new(),
            overlap_geometry: BTreeMap::new(),
        };
        let mut geometry = ScriptedOverlapGeometry::default();

        let transaction = reduce_native_foundation_fixed_point(
            &mut sig,
            &mut bindings,
            &mut geometry,
            &medians,
            &identities,
            &[],
        )
        .unwrap();

        assert_eq!(
            transaction
                .consistency_passes
                .iter()
                .map(|pass| pass.modification_count)
                .collect::<Vec<_>>(),
            vec![1, 1, 0]
        );
        assert_eq!(
            transaction.consistency_passes[0].stems.tail_prunes[0].removed_head_stem_edges,
            vec![NativeSigEdgeId(1)]
        );
        assert_eq!(
            transaction.consistency_passes[1].heads.mutations,
            vec![NativeReductionHeadMutation::OrphanRemoved {
                head: NativeSigVertexId(2)
            }]
        );
        assert!(transaction.consistency_passes[2].heads.mutations.is_empty());
        assert!(sig.vertex(0).is_some());
        assert!(sig.vertex(1).is_some());
        assert!(sig.vertex(2).is_none());
    }

    #[test]
    fn foundation_epoch_reduces_remaining_exclusions_and_sets_outer_repeat() {
        let mut sig = NativeSigSystem {
            system_id: 43,
            vertices: vec![
                vertex(0, NativeSigInterKind::Clef, 0.9),
                vertex(1, NativeSigInterKind::Key, 0.8),
            ],
            edges: vec![edge(0, 0, 1, NativeSigRelationKind::Exclusion)],
        };
        let mut bindings = NativeSigSystemBindings {
            system_id: 43,
            beam_vertices: BTreeMap::new(),
            beam_group_vertices: BTreeMap::new(),
            stem_vertices: BTreeMap::new(),
            head_vertices: BTreeMap::new(),
            ledger_vertices: BTreeMap::new(),
            reduction_interline: 10,
            reduction_staffs: Vec::new(),
            merged_staff_pairs: Vec::new(),
            overlap_geometry: BTreeMap::new(),
        };
        let mut geometry = ScriptedOverlapGeometry::default();

        let transaction = reduce_native_foundation_epoch(
            &mut sig,
            &mut bindings,
            &mut geometry,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &[],
        )
        .unwrap();

        assert_eq!(transaction.fixed_point.consistency_passes.len(), 1);
        assert_eq!(
            transaction.remaining_exclusions.removed_vertices,
            vec![NativeSigVertexId(1)]
        );
        assert!(transaction.outer_deleted_vertices.is_empty());
        assert_eq!(
            transaction.outer_reduced_vertices,
            vec![NativeSigVertexId(1)]
        );
        assert!(transaction.requires_outer_repeat);
        assert_eq!(transaction.late_consistency.modification_count, 0);
        assert!(transaction.late_consistency.exclusions.decisions.is_empty());
    }

    #[test]
    fn foundations_repeat_until_outer_local_sets_are_empty() {
        let mut sig = NativeSigSystem {
            system_id: 45,
            vertices: vec![
                vertex(0, NativeSigInterKind::Clef, 0.9),
                vertex(1, NativeSigInterKind::Key, 0.8),
            ],
            edges: vec![edge(0, 0, 1, NativeSigRelationKind::Exclusion)],
        };
        let mut bindings = NativeSigSystemBindings {
            system_id: 45,
            beam_vertices: BTreeMap::new(),
            beam_group_vertices: BTreeMap::new(),
            stem_vertices: BTreeMap::new(),
            head_vertices: BTreeMap::new(),
            ledger_vertices: BTreeMap::new(),
            reduction_interline: 10,
            reduction_staffs: Vec::new(),
            merged_staff_pairs: Vec::new(),
            overlap_geometry: BTreeMap::new(),
        };
        let mut geometry = ScriptedOverlapGeometry::default();

        let transaction = reduce_native_foundations(
            &mut sig,
            &mut bindings,
            &mut geometry,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &[],
        )
        .unwrap();

        assert!(transaction.first_epoch.requires_outer_repeat);
        assert_eq!(transaction.continuation_epochs.len(), 1);
        let terminal = &transaction.continuation_epochs[0];
        assert!(terminal.outer_deleted_vertices.is_empty());
        assert!(terminal.outer_reduced_vertices.is_empty());
        assert!(!terminal.requires_outer_repeat);
        assert_eq!(terminal.consistency_passes.len(), 1);
        assert_eq!(terminal.consistency_passes[0].modification_count, 0);
        assert_eq!(transaction.all_removed_vertices, vec![NativeSigVertexId(1)]);
    }

    #[test]
    fn foundations_opening_purge_drives_repeat_and_all_removed_order() {
        let mut sig = NativeSigSystem {
            system_id: 46,
            vertices: vec![
                vertex(0, NativeSigInterKind::Head, 0.2),
                vertex(1, NativeSigInterKind::Clef, 0.9),
                vertex(2, NativeSigInterKind::Key, 0.8),
            ],
            edges: vec![edge(0, 1, 2, NativeSigRelationKind::Exclusion)],
        };
        let mut bindings = NativeSigSystemBindings {
            system_id: 46,
            beam_vertices: BTreeMap::new(),
            beam_group_vertices: BTreeMap::new(),
            stem_vertices: BTreeMap::new(),
            head_vertices: BTreeMap::new(),
            ledger_vertices: BTreeMap::new(),
            reduction_interline: 10,
            reduction_staffs: Vec::new(),
            merged_staff_pairs: Vec::new(),
            overlap_geometry: BTreeMap::new(),
        };
        let mut geometry = ScriptedOverlapGeometry::default();

        let transaction = reduce_native_foundations(
            &mut sig,
            &mut bindings,
            &mut geometry,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &[],
        )
        .unwrap();

        assert_eq!(
            transaction.first_epoch.outer_deleted_vertices,
            vec![NativeSigVertexId(0)]
        );
        assert_eq!(
            transaction.first_epoch.outer_reduced_vertices,
            vec![NativeSigVertexId(2)]
        );
        assert_eq!(
            transaction.all_removed_vertices,
            vec![NativeSigVertexId(0), NativeSigVertexId(2)]
        );
        assert_eq!(transaction.continuation_epochs.len(), 1);
        assert!(!transaction.continuation_epochs[0].requires_outer_repeat);
    }

    #[test]
    fn stem_head_end_refinement_uses_leading_head_anchor_and_median() {
        let mut stem = vertex(0, NativeSigInterKind::Stem, 0.9);
        stem.bounds = NativeSigBounds {
            x: 10,
            y: 10,
            width: 2,
            height: 80,
        };
        let head = shaped_head(1, "NOTEHEAD_BLACK", 8);
        let mut relation = head_stem_edge(0, 1, 0);
        relation
            .head_stem
            .as_mut()
            .expect("payload")
            .extension_point = NativeStemPoint { x: 11.0, y: 10.0 };
        let mut sig = NativeSigSystem {
            system_id: 47,
            vertices: vec![stem, head],
            edges: vec![relation],
        };
        let mut medians = BTreeMap::from([(
            NativeSigVertexId(0),
            NativeStemLine {
                start: NativeStemPoint { x: 11.0, y: 10.0 },
                stop: NativeStemPoint { x: 11.0, y: 90.0 },
            },
        )]);
        let thicknesses = BTreeMap::from([(NativeSigVertexId(0), 2.0)]);
        let anchors = BTreeMap::from([(
            (
                NativeSigVertexId(1),
                NativeStemHeadSide::Left,
                NativeStemVerticalSide::Bottom,
            ),
            NativeStemPoint { x: 9.0, y: 14.0 },
        )]);

        let transaction = refine_native_reduction_stem_head_ends(
            &mut sig,
            &mut medians,
            &thicknesses,
            &anchors,
            10,
            0.02,
        )
        .unwrap();

        assert!(transaction.no_head_stems.is_empty());
        assert_eq!(transaction.refinements.len(), 1);
        let refinement = transaction.refinements[0];
        assert_eq!(refinement.direction, 1);
        assert_eq!(refinement.leading_head, NativeSigVertexId(1));
        assert_eq!(refinement.vertical_side, NativeStemVerticalSide::Bottom);
        assert_eq!(
            refinement.reliable_line_source,
            NativeReductionReliableStemLineSource::Median
        );
        assert_eq!(
            refinement.median_after,
            NativeStemLine {
                start: NativeStemPoint { x: 11.0, y: 14.0 },
                stop: NativeStemPoint { x: 11.0, y: 90.0 },
            }
        );
        assert_eq!(sig.vertices[0].bounds, refinement.bounds_after);
        assert_eq!(medians[&NativeSigVertexId(0)], refinement.median_after);
    }

    #[test]
    fn short_stem_head_end_refinement_uses_skewed_vertical_and_is_atomic() {
        let mut stem = vertex(0, NativeSigInterKind::Stem, 0.9);
        stem.bounds = NativeSigBounds {
            x: 49,
            y: 10,
            width: 2,
            height: 8,
        };
        let mut head = shaped_head(1, "NOTEHEAD_BLACK", 14);
        head.bounds.height = 4;
        let orphan = vertex(2, NativeSigInterKind::Stem, 0.9);
        let mut relation = head_stem_edge(0, 1, 0);
        let payload = relation.head_stem.as_mut().expect("payload");
        payload.head_side = NativeStemHeadSide::Right;
        payload.extension_point = NativeStemPoint { x: 50.0, y: 17.0 };
        let mut sig = NativeSigSystem {
            system_id: 48,
            vertices: vec![stem, head, orphan],
            edges: vec![relation],
        };
        let mut medians = BTreeMap::from([(
            NativeSigVertexId(0),
            NativeStemLine {
                start: NativeStemPoint { x: 50.0, y: 10.0 },
                stop: NativeStemPoint { x: 50.0, y: 18.0 },
            },
        )]);
        let thicknesses = BTreeMap::from([(NativeSigVertexId(0), 2.0)]);
        let anchors = BTreeMap::from([(
            (
                NativeSigVertexId(1),
                NativeStemHeadSide::Right,
                NativeStemVerticalSide::Top,
            ),
            NativeStemPoint { x: 55.0, y: 17.0 },
        )]);

        let transaction = refine_native_reduction_stem_head_ends(
            &mut sig,
            &mut medians,
            &thicknesses,
            &anchors,
            10,
            0.01,
        )
        .unwrap();

        assert_eq!(transaction.no_head_stems, vec![NativeSigVertexId(2)]);
        let refinement = transaction.refinements[0];
        assert_eq!(refinement.direction, -1);
        assert_eq!(
            refinement.reliable_line_source,
            NativeReductionReliableStemLineSource::SkewedVertical
        );
        assert_eq!(refinement.reliable_line.start.x, 50.0);
        assert_eq!(refinement.reliable_line.start.y, 14.0);
        assert_eq!(refinement.median_after.stop.y, 17.0);
        assert!((refinement.median_after.stop.x - 49.97).abs() < 1.0e-12);

        let sig_before_error = sig.clone();
        let medians_before_error = medians.clone();
        let error = refine_native_reduction_stem_head_ends(
            &mut sig,
            &mut medians,
            &thicknesses,
            &BTreeMap::new(),
            10,
            0.01,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            NativeReductionFoundationPrefixError::MissingStemHeadAnchor { .. }
        ));
        assert_eq!(sig, sig_before_error);
        assert_eq!(medians, medians_before_error);
    }

    #[test]
    fn beam_group_check_splits_suffix_and_removes_cross_group_relations() {
        let mut stem_four = vertex(4, NativeSigInterKind::Stem, 0.8);
        stem_four.bounds = NativeSigBounds {
            x: 20,
            y: 8,
            width: 2,
            height: 16,
        };
        let mut stem_five = vertex(5, NativeSigInterKind::Stem, 0.8);
        stem_five.bounds = NativeSigBounds {
            x: 80,
            y: 28,
            width: 2,
            height: 12,
        };
        let mut sig = NativeSigSystem {
            system_id: 49,
            vertices: vec![
                vertex(0, NativeSigInterKind::BeamGroup, 1.0),
                horizontal_beam(1, 10.0),
                horizontal_beam(2, 20.0),
                horizontal_beam(3, 30.0),
                stem_four,
                stem_five,
            ],
            edges: vec![
                edge(0, 0, 1, NativeSigRelationKind::Containment),
                edge(1, 0, 2, NativeSigRelationKind::Containment),
                edge(2, 0, 3, NativeSigRelationKind::Containment),
                beam_support_edge(3, 1, 2),
                beam_support_edge(4, 1, 3),
                beam_support_edge(5, 2, 3),
                beam_stem_edge(6, 1, 4, NativeBeamPortion::Center),
                beam_stem_edge(7, 2, 4, NativeBeamPortion::Center),
                beam_stem_edge(8, 2, 5, NativeBeamPortion::Center),
                beam_stem_edge(9, 3, 5, NativeBeamPortion::Center),
            ],
        };
        let medians = BTreeMap::from([
            (
                NativeSigVertexId(4),
                NativeStemLine {
                    start: NativeStemPoint { x: 21.0, y: 8.0 },
                    stop: NativeStemPoint { x: 21.0, y: 24.0 },
                },
            ),
            (
                NativeSigVertexId(5),
                NativeStemLine {
                    start: NativeStemPoint { x: 81.0, y: 28.0 },
                    stop: NativeStemPoint { x: 81.0, y: 40.0 },
                },
            ),
        ]);

        let transaction = check_native_reduction_beam_groups_in_sig(
            &mut sig,
            &medians,
            10,
            HeadlessSkew::new(0.0, 100, 100),
        )
        .unwrap();

        assert_eq!(transaction.initial_groups, vec![NativeSigVertexId(0)]);
        assert_eq!(transaction.splits.len(), 1);
        let split = &transaction.splits[0];
        assert_eq!(split.original_group, NativeSigVertexId(0));
        assert_eq!(split.alien_group, NativeSigVertexId(6));
        assert_eq!(split.upper_beam, NativeSigVertexId(2));
        assert_eq!(split.lower_beam, NativeSigVertexId(3));
        assert_eq!(split.moved_beams, vec![NativeSigVertexId(3)]);
        assert_eq!(split.removed_containments, vec![NativeSigEdgeId(2)]);
        assert_eq!(split.added_containments, vec![NativeSigEdgeId(10)]);
        assert!(split.added_beam_supports.is_empty());
        assert_eq!(split.removed_cross_stem_relations, vec![NativeSigEdgeId(8)]);
        assert_eq!(
            split.removed_cross_beam_relations,
            vec![NativeSigEdgeId(4), NativeSigEdgeId(5)]
        );
        assert_eq!(
            native_beam_group_members(&sig, NativeSigVertexId(0)),
            vec![NativeSigVertexId(1), NativeSigVertexId(2)]
        );
        assert_eq!(
            native_beam_group_members(&sig, NativeSigVertexId(6)),
            vec![NativeSigVertexId(3)]
        );
        assert_eq!(transaction.checks.last().unwrap().beam_index, Some(2));
        assert!(sig.edges[7].active);
        assert!(!sig.edges[8].active);

        let mut invalid = sig.clone();
        invalid.vertices[1].beam_geometry = None;
        let invalid_before = invalid.clone();
        let error = check_native_reduction_beam_groups_in_sig(
            &mut invalid,
            &medians,
            10,
            HeadlessSkew::new(0.0, 100, 100),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            NativeReductionFoundationPrefixError::MissingBeamGeometry { .. }
        ));
        assert_eq!(invalid, invalid_before);
    }

    #[test]
    fn free_stem_lengths_skip_beams_and_no_heads_then_use_opposite_anchor_side() {
        let mut down_head = shaped_head(3, "NOTEHEAD_BLACK", 8);
        down_head.bounds.height = 6;
        let mut up_head = shaped_head(5, "NOTEHEAD_BLACK", 44);
        up_head.bounds.height = 6;
        let mut down_relation = head_stem_edge(1, 3, 2);
        let down_payload = down_relation.head_stem.as_mut().unwrap();
        down_payload.head_side = NativeStemHeadSide::Left;
        down_payload.extension_point = NativeStemPoint { x: 20.0, y: 10.0 };
        let mut up_relation = head_stem_edge(2, 5, 4);
        let up_payload = up_relation.head_stem.as_mut().unwrap();
        up_payload.head_side = NativeStemHeadSide::Right;
        up_payload.extension_point = NativeStemPoint { x: 40.0, y: 50.0 };
        let sig = NativeSigSystem {
            system_id: 50,
            vertices: vec![
                vertex(0, NativeSigInterKind::Stem, 0.8),
                vertex(1, NativeSigInterKind::Stem, 0.8),
                vertex(2, NativeSigInterKind::Stem, 0.8),
                down_head,
                vertex(4, NativeSigInterKind::Stem, 0.8),
                up_head,
                horizontal_beam(6, 20.0),
            ],
            edges: vec![
                beam_stem_edge(0, 6, 0, NativeBeamPortion::Center),
                down_relation,
                up_relation,
            ],
        };
        let medians = BTreeMap::from([
            (
                NativeSigVertexId(2),
                NativeStemLine {
                    start: NativeStemPoint { x: 20.0, y: 10.0 },
                    stop: NativeStemPoint { x: 20.0, y: 50.0 },
                },
            ),
            (
                NativeSigVertexId(4),
                NativeStemLine {
                    start: NativeStemPoint { x: 40.0, y: 10.0 },
                    stop: NativeStemPoint { x: 40.0, y: 50.0 },
                },
            ),
        ]);
        let anchors = BTreeMap::from([
            (
                (
                    NativeSigVertexId(3),
                    NativeStemHeadSide::Left,
                    NativeStemVerticalSide::Top,
                ),
                NativeStemPoint { x: 20.0, y: 14.0 },
            ),
            (
                (
                    NativeSigVertexId(5),
                    NativeStemHeadSide::Right,
                    NativeStemVerticalSide::Bottom,
                ),
                NativeStemPoint { x: 40.0, y: 42.0 },
            ),
        ]);

        let transaction =
            measure_native_reduction_system_stem_free_lengths(&sig, &medians, &anchors).unwrap();

        assert_eq!(
            transaction.skips,
            vec![
                (
                    NativeSigVertexId(0),
                    NativeReductionStemFreeLengthSkip::BeamAttached
                ),
                (
                    NativeSigVertexId(1),
                    NativeReductionStemFreeLengthSkip::NoHeads
                ),
            ]
        );
        assert_eq!(transaction.lengths.len(), 2);
        assert_eq!(transaction.lengths[0].direction, 1);
        assert_eq!(
            transaction.lengths[0].reference_vertical_side,
            NativeStemVerticalSide::Top
        );
        assert_eq!(transaction.lengths[0].pixels, 36);
        assert_eq!(transaction.lengths[1].direction, -1);
        assert_eq!(
            transaction.lengths[1].reference_vertical_side,
            NativeStemVerticalSide::Bottom
        );
        assert_eq!(transaction.lengths[1].pixels, 32);
    }

    #[test]
    fn foundation_late_consistency_analyzes_then_reduces_fresh_exclusion() {
        let stem = vertex(0, NativeSigInterKind::Stem, 0.9);
        let standard = vertex(1, NativeSigInterKind::Beam, 0.95);
        let small = vertex(2, NativeSigInterKind::SmallBeam, 0.8);
        let mut sig = NativeSigSystem {
            system_id: 44,
            vertices: vec![stem, standard, small],
            edges: vec![
                beam_stem_edge(0, 1, 0, NativeBeamPortion::Left),
                beam_stem_edge(1, 2, 0, NativeBeamPortion::Left),
            ],
        };
        let medians = BTreeMap::from([(
            NativeSigVertexId(0),
            NativeStemLine {
                start: NativeStemPoint { x: 2.0, y: 0.0 },
                stop: NativeStemPoint { x: 2.0, y: 100.0 },
            },
        )]);

        let transaction = reduce_native_foundation_late_consistency(&mut sig, &medians).unwrap();

        assert_eq!(
            transaction.chord_analysis.incompatible_exclusions,
            vec![NativeSigEdgeId(2)]
        );
        assert_eq!(
            transaction.exclusions.decisions,
            vec![NativeReductionExclusionDecision {
                exclusion: NativeSigEdgeId(2),
                source: NativeSigVertexId(1),
                source_best_grade: 0.95,
                target: NativeSigVertexId(2),
                target_best_grade: 0.8,
                removed: NativeSigVertexId(2),
            }]
        );
        assert!(transaction.weak_purge.removed_vertices.is_empty());
        assert_eq!(transaction.modification_count, 0);
        assert!(sig.vertex(1).is_some());
        assert!(sig.vertex(2).is_none());
    }

    #[test]
    fn chord_analysis_fails_closed_before_guessing_a_good_stem_median() {
        let mut sig = NativeSigSystem {
            system_id: 42,
            vertices: vec![vertex(0, NativeSigInterKind::Stem, 0.9)],
            edges: Vec::new(),
        };

        let error = analyze_native_foundation_chords(&mut sig, &BTreeMap::new()).unwrap_err();

        assert_eq!(
            error,
            NativeReductionFoundationPrefixError::MissingStemMedian {
                system_id: 42,
                stem: NativeSigVertexId(0),
            }
        );
        assert!(sig.edges.is_empty());
    }

    #[test]
    fn stem_median_rectangle_intersection_keeps_java_boundary_and_outcode_rules() {
        let bounds = NativeSigBounds {
            x: 10,
            y: 20,
            width: 8,
            height: 6,
        };
        assert!(line_intersects_rectangle(
            NativeStemLine {
                start: NativeStemPoint { x: 14.0, y: 0.0 },
                stop: NativeStemPoint { x: 14.0, y: 40.0 },
            },
            bounds,
        ));
        assert!(line_intersects_rectangle(
            NativeStemLine {
                start: NativeStemPoint { x: 0.0, y: 20.0 },
                stop: NativeStemPoint { x: 30.0, y: 20.0 },
            },
            bounds,
        ));
        assert!(!line_intersects_rectangle(
            NativeStemLine {
                start: NativeStemPoint { x: 0.0, y: 19.0 },
                stop: NativeStemPoint { x: 30.0, y: 19.0 },
            },
            bounds,
        ));
        assert!(!line_intersects_rectangle(
            NativeStemLine {
                start: NativeStemPoint { x: 14.0, y: 0.0 },
                stop: NativeStemPoint { x: 14.0, y: 40.0 },
            },
            NativeSigBounds { width: 0, ..bounds },
        ));
    }

    #[test]
    fn overlap_scan_filters_headers_and_disabled_kinds_and_stably_sorts_by_x() {
        let mut vertices = vec![
            vertex(0, NativeSigInterKind::Head, 0.8),
            vertex(1, NativeSigInterKind::Ledger, 0.8),
            vertex(2, NativeSigInterKind::Beam, 0.8),
            vertex(3, NativeSigInterKind::BeamHook, 0.8),
            vertex(4, NativeSigInterKind::Clef, 0.8),
            vertex(5, NativeSigInterKind::Stem, 0.8),
            vertex(6, NativeSigInterKind::BeamGroup, 0.8),
        ];
        for &ordinal in &[1, 2, 3, 4, 6] {
            vertices[ordinal].bounds.x = 0;
        }
        vertices[0].bounds.x = 20;
        vertices[5].bounds.x = 100;
        let mut sig = NativeSigSystem {
            system_id: 4,
            vertices,
            edges: Vec::new(),
        };
        let mut geometry = ScriptedOverlapGeometry::default();

        let result = detect_native_reduction_overlaps(&mut sig, &mut geometry).unwrap();

        assert_eq!(
            result.scan_order,
            vec![
                NativeSigVertexId(2),
                NativeSigVertexId(3),
                NativeSigVertexId(0),
                NativeSigVertexId(5)
            ]
        );
        assert_eq!(
            result.pairs[0].disposition,
            NativeReductionOverlapDisposition::CompatibleAccepted
        );
        assert!(result.pairs.iter().any(|pair| {
            pair.left == NativeSigVertexId(0)
                && pair.right == NativeSigVertexId(5)
                && pair.disposition == NativeReductionOverlapDisposition::BeyondRightEdge
        }));
        assert!(geometry.precise_calls.is_empty());
        assert!(result.inserted_exclusions.is_empty());
    }

    #[test]
    fn overlap_scan_uses_inclusive_java_iou_and_normalizes_new_exclusion_ids() {
        let mut vertices = vec![
            vertex(0, NativeSigInterKind::Brace, 0.8),
            vertex(1, NativeSigInterKind::Barline, 0.8),
        ];
        // Intersection 2x10=20, union 200+220-20=400: exactly 0.05.
        vertices[0].bounds = NativeSigBounds {
            x: 18,
            y: 0,
            width: 22,
            height: 10,
        };
        vertices[1].bounds = NativeSigBounds {
            x: 0,
            y: 0,
            width: 20,
            height: 10,
        };
        let mut sig = NativeSigSystem {
            system_id: 5,
            vertices,
            edges: Vec::new(),
        };
        let mut geometry = ScriptedOverlapGeometry::default();
        geometry.overlaps.insert((1, 0));

        let result = detect_native_reduction_overlaps(&mut sig, &mut geometry).unwrap();

        assert_eq!(geometry.precise_calls, vec![(1, 0)]);
        assert_eq!(result.pairs[0].iou, Some(0.05));
        assert_eq!(
            result.pairs[0].disposition,
            NativeReductionOverlapDisposition::ExclusionInserted
        );
        assert_eq!(result.inserted_exclusions, vec![NativeSigEdgeId(0)]);
        assert_eq!((sig.edges[0].source, sig.edges[0].target), (0, 1));
        assert_eq!(sig.edges[0].kind, NativeSigRelationKind::Exclusion);
    }

    #[test]
    fn overlap_scan_delegates_mirrors_and_precise_rejection_without_mutation() {
        let mut sig = NativeSigSystem {
            system_id: 6,
            vertices: vec![
                vertex(0, NativeSigInterKind::Brace, 0.8),
                vertex(1, NativeSigInterKind::Barline, 0.8),
                vertex(2, NativeSigInterKind::Bracket, 0.8),
            ],
            edges: Vec::new(),
        };
        for item in &mut sig.vertices {
            item.bounds.x = 0;
            item.bounds.width = 10;
        }
        let mut geometry = ScriptedOverlapGeometry::default();
        geometry.mirrors.insert((0, 1));

        let result = detect_native_reduction_overlaps(&mut sig, &mut geometry).unwrap();

        assert_eq!(
            result.pairs[0].disposition,
            NativeReductionOverlapDisposition::MirrorAccepted
        );
        assert_eq!(geometry.precise_calls, vec![(0, 2), (1, 2)]);
        assert!(result.pairs[1..].iter().all(|pair| {
            pair.disposition == NativeReductionOverlapDisposition::PreciseRejected
        }));
        assert!(sig.edges.is_empty());
    }

    #[test]
    fn overlap_exclusion_respects_head_stem_support_and_existing_relation_exceptions() {
        fn disposition(
            mut vertices: Vec<NativeSigVertex>,
            edges: Vec<NativeSigEdge>,
        ) -> (NativeReductionOverlapDisposition, usize) {
            for vertex in &mut vertices {
                vertex.bounds.x = 0;
                vertex.bounds.width = 10;
            }
            let mut sig = NativeSigSystem {
                system_id: 7,
                vertices,
                edges,
            };
            let mut geometry = ScriptedOverlapGeometry::default();
            geometry.overlaps.insert((0, 1));
            let result = detect_native_reduction_overlaps(&mut sig, &mut geometry).unwrap();
            (result.pairs[0].disposition, sig.edges.len())
        }

        let mut head = vertex(0, NativeSigInterKind::Head, 0.8);
        head.shape = Some("NOTEHEAD_BLACK".to_owned());
        assert_eq!(
            disposition(
                vec![head.clone(), vertex(1, NativeSigInterKind::Stem, 0.8)],
                Vec::new()
            ),
            (
                NativeReductionOverlapDisposition::StandardHeadStemAccepted,
                0
            )
        );
        head.shape = Some("NOTEHEAD_BLACK_SMALL".to_owned());
        assert_eq!(
            disposition(
                vec![head, vertex(1, NativeSigInterKind::Stem, 0.8)],
                Vec::new()
            )
            .0,
            NativeReductionOverlapDisposition::ExclusionInserted
        );

        let mut support = edge(0, 0, 1, NativeSigRelationKind::NoExclusion);
        support.support = Some(NativeSigSupport {
            grade: 1.0,
            bar_connection_impacts: None,
        });
        assert_eq!(
            disposition(
                vec![
                    vertex(0, NativeSigInterKind::Brace, 0.8),
                    vertex(1, NativeSigInterKind::Barline, 0.8)
                ],
                vec![support]
            ),
            (NativeReductionOverlapDisposition::SupportAccepted, 1)
        );
        assert_eq!(
            disposition(
                vec![
                    vertex(0, NativeSigInterKind::Brace, 0.8),
                    vertex(1, NativeSigInterKind::Barline, 0.8)
                ],
                vec![edge(0, 0, 1, NativeSigRelationKind::Exclusion)]
            ),
            (NativeReductionOverlapDisposition::ExistingExclusion, 1)
        );
    }

    #[test]
    fn orphan_head_prune_keeps_only_heads_with_a_live_head_stem_relation() {
        let mut inactive = head_stem_edge(1, 2, 3);
        inactive.active = false;
        let mut sig = NativeSigSystem {
            system_id: 8,
            vertices: vec![
                shaped_head(0, "NOTEHEAD_BLACK", 0),
                shaped_head(1, "NOTEHEAD_BLACK", 0),
                shaped_head(2, "NOTEHEAD_BLACK", 0),
                vertex(3, NativeSigInterKind::Stem, 0.8),
                shaped_head(4, "WHOLE_NOTE", 0),
            ],
            edges: vec![head_stem_edge(0, 1, 3), inactive],
        };

        let result = prune_native_foundation_heads_without_stems(&mut sig).unwrap();

        assert_eq!(result.kind, NativeSigInterKind::Head);
        assert_eq!(
            result.removed_vertices,
            vec![NativeSigVertexId(0), NativeSigVertexId(2)]
        );
        assert!(sig.vertex(0).is_none());
        assert!(sig.vertex(1).is_some());
        assert!(sig.vertex(2).is_none());
        assert!(sig.vertex(3).is_some());
        assert!(
            sig.vertex(4).is_some(),
            "stemless heads are outside checkHeads"
        );
    }

    #[test]
    fn orphan_stem_prune_snapshots_stem_order_and_ignores_other_relations() {
        let mut sig = NativeSigSystem {
            system_id: 9,
            vertices: vec![
                vertex(0, NativeSigInterKind::Head, 0.8),
                vertex(1, NativeSigInterKind::Stem, 0.8),
                vertex(2, NativeSigInterKind::Stem, 0.8),
                vertex(3, NativeSigInterKind::Stem, 0.8),
            ],
            edges: vec![
                head_stem_edge(0, 0, 2),
                edge(1, 1, 3, NativeSigRelationKind::Exclusion),
            ],
        };

        let result = prune_native_foundation_stems_without_heads(&mut sig).unwrap();

        assert_eq!(result.kind, NativeSigInterKind::Stem);
        assert_eq!(
            result.removed_vertices,
            vec![NativeSigVertexId(1), NativeSigVertexId(3)]
        );
        assert!(sig.vertex(1).is_none());
        assert!(sig.vertex(2).is_some());
        assert!(sig.vertex(3).is_none());
    }

    #[test]
    fn stem_check_removes_orphan_and_all_forbidden_tail_links_in_java_order() {
        let orphan = vertex(0, NativeSigInterKind::Stem, 0.8);
        let directed = vertex(1, NativeSigInterKind::Stem, 0.8);
        let mut top = shaped_head(2, "NOTEHEAD_BLACK", 0);
        top.grade = 0.95;
        let bottom = shaped_head(3, "NOTEHEAD_BLACK", 95);
        let undecidable = vertex(4, NativeSigInterKind::Stem, 0.8);
        let middle = shaped_head(5, "NOTEHEAD_BLACK", 47);
        let mut top_link = head_stem_edge(0, 2, 1);
        top_link.head_stem.as_mut().unwrap().head_side = NativeStemHeadSide::Left;
        top_link.head_stem.as_mut().unwrap().extension_point = NativeStemPoint { x: 11.0, y: 2.0 };
        let mut forbidden_link = head_stem_edge(1, 3, 1);
        forbidden_link.head_stem.as_mut().unwrap().extension_point =
            NativeStemPoint { x: 11.0, y: 98.0 };
        forbidden_link.head_stem.as_mut().unwrap().dx = 0.05;
        forbidden_link.head_stem.as_mut().unwrap().dy = 0.0;
        let mut middle_link = head_stem_edge(2, 5, 4);
        middle_link.head_stem.as_mut().unwrap().extension_point =
            NativeStemPoint { x: 41.0, y: 50.0 };
        let mut sig = NativeSigSystem {
            system_id: 10,
            vertices: vec![orphan, directed, top, bottom, undecidable, middle],
            edges: vec![top_link, forbidden_link, middle_link],
        };
        let medians = BTreeMap::from([
            (
                NativeSigVertexId(1),
                NativeStemLine {
                    start: NativeStemPoint { x: 11.0, y: 10.0 },
                    stop: NativeStemPoint { x: 11.0, y: 90.0 },
                },
            ),
            (
                NativeSigVertexId(4),
                NativeStemLine {
                    start: NativeStemPoint { x: 41.0, y: 10.0 },
                    stop: NativeStemPoint { x: 41.0, y: 90.0 },
                },
            ),
        ]);

        let result = prune_native_foundation_stems(&mut sig, &medians).unwrap();

        assert_eq!(
            result.stem_order,
            vec![
                NativeSigVertexId(0),
                NativeSigVertexId(1),
                NativeSigVertexId(4)
            ]
        );
        assert_eq!(result.removed_orphan_stems, vec![NativeSigVertexId(0)]);
        assert_eq!(
            result.tail_prunes,
            vec![NativeReductionStemTailPrune {
                stem: NativeSigVertexId(1),
                direction: 1,
                removed_head_stem_edges: vec![NativeSigEdgeId(1)],
                added_exclusions: vec![NativeSigEdgeId(3)],
            }]
        );
        assert_eq!(result.modification_count, 2);
        assert!(sig.vertex(0).is_none());
        assert!(sig.edges[0].active);
        assert!(!sig.edges[1].active);
        assert!(sig.edges[2].active, "direction zero must remain undecided");
        assert_eq!(sig.edges[3].kind, NativeSigRelationKind::Exclusion);
        assert_eq!((sig.edges[3].source, sig.edges[3].target), (1, 3));
    }

    #[test]
    fn stem_ending_prune_removes_wrong_bottom_side_restarts_and_adds_invading_exclusion() {
        let mut wrong = head_stem_edge(0, 0, 2);
        wrong.head_stem = Some(NativeSigHeadStemPayload {
            dx: 0.05,
            dy: 0.0,
            head_side: NativeStemHeadSide::Left,
            extension_point: NativeStemPoint { x: 5.0, y: 99.0 },
            consistency: 1.0,
            manual: false,
        });
        let mut correct = head_stem_edge(1, 1, 2);
        correct.head_stem = Some(NativeSigHeadStemPayload {
            dx: 0.2,
            dy: 0.0,
            head_side: NativeStemHeadSide::Right,
            extension_point: NativeStemPoint { x: 5.0, y: 98.0 },
            consistency: 1.0,
            manual: false,
        });
        let mut sig = NativeSigSystem {
            system_id: 10,
            vertices: vec![
                vertex(0, NativeSigInterKind::Head, 0.8),
                vertex(1, NativeSigInterKind::Head, 0.8),
                vertex(2, NativeSigInterKind::Stem, 0.8),
            ],
            edges: vec![wrong, correct],
        };
        let medians = BTreeMap::from([(
            NativeSigVertexId(2),
            NativeStemLine {
                start: NativeStemPoint { x: 5.0, y: 10.0 },
                stop: NativeStemPoint { x: 5.0, y: 90.0 },
            },
        )]);

        let result = prune_native_foundation_stem_ending_heads(&mut sig, &medians).unwrap();

        assert_eq!(result.modified_stems.len(), 1);
        assert_eq!(
            result.modified_stems[0].removed_head_stem_edges,
            vec![NativeSigEdgeId(0)]
        );
        assert_eq!(
            result.modified_stems[0].added_exclusions,
            vec![NativeSigEdgeId(2)]
        );
        assert!(!sig.edges[0].active);
        assert!(sig.edges[1].active);
        assert_eq!(sig.edges[2].kind, NativeSigRelationKind::Exclusion);
        assert_eq!((sig.edges[2].source, sig.edges[2].target), (0, 2));
    }

    #[test]
    fn stem_ending_prune_keeps_middle_and_correct_top() {
        let mut top_left = head_stem_edge(0, 0, 3);
        top_left.head_stem.as_mut().unwrap().extension_point.y = 1.0;
        let mut middle_right = head_stem_edge(1, 1, 3);
        middle_right.head_stem.as_mut().unwrap().head_side = NativeStemHeadSide::Right;
        middle_right.head_stem.as_mut().unwrap().extension_point.y = 50.0;
        let missing_median = head_stem_edge(2, 2, 4);
        let mut sig = NativeSigSystem {
            system_id: 11,
            vertices: vec![
                vertex(0, NativeSigInterKind::Head, 0.8),
                vertex(1, NativeSigInterKind::Head, 0.8),
                vertex(2, NativeSigInterKind::Head, 0.8),
                vertex(3, NativeSigInterKind::Stem, 0.8),
                vertex(4, NativeSigInterKind::Stem, 0.8),
            ],
            edges: vec![top_left, middle_right, missing_median],
        };
        let median = NativeStemLine {
            start: NativeStemPoint { x: 5.0, y: 10.0 },
            stop: NativeStemPoint { x: 5.0, y: 90.0 },
        };
        let medians = BTreeMap::from([
            (NativeSigVertexId(3), median),
            (NativeSigVertexId(4), median),
        ]);

        let result = prune_native_foundation_stem_ending_heads(&mut sig, &medians).unwrap();

        assert!(result.modified_stems.is_empty());
        assert!(sig.edges.iter().all(|edge| edge.active));
    }

    #[test]
    fn stem_ending_prune_fails_closed_when_a_live_stem_lacks_its_median() {
        let mut sig = NativeSigSystem {
            system_id: 12,
            vertices: vec![vertex(0, NativeSigInterKind::Stem, 0.8)],
            edges: Vec::new(),
        };

        let error = prune_native_foundation_stem_ending_heads(&mut sig, &BTreeMap::new())
            .expect_err("missing geometry must not be guessed");

        assert_eq!(
            error,
            NativeReductionStemEndingError::MissingStemMedian {
                system_id: 12,
                stem: NativeSigVertexId(0)
            }
        );
    }

    #[test]
    fn reduces_strongest_exclusion_first_and_target_on_tie() {
        let mut sig = NativeSigSystem {
            system_id: 7,
            vertices: vec![
                vertex(0, NativeSigInterKind::Head, 0.9),
                vertex(1, NativeSigInterKind::Beam, 0.2),
                vertex(2, NativeSigInterKind::Beam, 0.6),
                vertex(3, NativeSigInterKind::BeamHook, 0.6),
            ],
            edges: vec![
                edge(0, 0, 1, NativeSigRelationKind::Exclusion),
                edge(1, 2, 3, NativeSigRelationKind::Exclusion),
            ],
        };

        let result = reduce_native_sig_exclusions(&mut sig).unwrap();

        assert_eq!(
            result
                .decisions
                .iter()
                .map(|decision| (decision.exclusion.0, decision.removed.0))
                .collect::<Vec<_>>(),
            vec![(0, 1), (1, 3)]
        );
        assert_eq!(
            result.removed_vertices,
            vec![NativeSigVertexId(1), NativeSigVertexId(3)]
        );
        assert!(sig.vertices[0].active);
        assert!(sig.vertices[2].active);
        assert!(sig.vertices[1].removed);
        assert!(sig.vertices[3].removed);
        assert!(sig.edges.iter().all(|edge| !edge.active));
    }

    #[test]
    fn removes_a_dying_ensemble_after_its_weaker_member() {
        let mut sig = NativeSigSystem {
            system_id: 3,
            vertices: vec![
                vertex(0, NativeSigInterKind::BeamGroup, 1.0),
                vertex(1, NativeSigInterKind::Beam, 0.3),
                vertex(2, NativeSigInterKind::Head, 0.8),
            ],
            edges: vec![
                edge(0, 0, 1, NativeSigRelationKind::Containment),
                edge(1, 1, 2, NativeSigRelationKind::Exclusion),
            ],
        };

        let result = reduce_native_sig_exclusions(&mut sig).unwrap();

        assert_eq!(
            result.removed_vertices,
            vec![NativeSigVertexId(1), NativeSigVertexId(0)]
        );
        assert!(sig.vertices[0].removed);
        assert!(sig.vertices[1].removed);
        assert!(sig.vertices[2].active);
    }

    #[test]
    fn recomputes_support_context_before_selecting_the_next_conflict() {
        let mut sig = NativeSigSystem {
            system_id: 11,
            vertices: vec![
                vertex(0, NativeSigInterKind::Head, 0.8),
                vertex(1, NativeSigInterKind::Stem, 0.2),
                vertex(2, NativeSigInterKind::Beam, 0.45),
                vertex(3, NativeSigInterKind::BeamHook, 0.4),
            ],
            edges: vec![
                edge(0, 0, 1, NativeSigRelationKind::Exclusion),
                NativeSigEdge {
                    support: Some(NativeSigSupport {
                        grade: 1.0,
                        bar_connection_impacts: None,
                    }),
                    ..edge(1, 1, 2, NativeSigRelationKind::BeamBeam)
                },
                edge(2, 2, 3, NativeSigRelationKind::Exclusion),
            ],
        };
        sig.contextualize();

        let result = reduce_native_sig_exclusions(&mut sig).unwrap();

        assert_eq!(result.decisions.len(), 2);
        assert_eq!(result.decisions[0].removed, NativeSigVertexId(1));
        assert_eq!(result.decisions[1].removed, NativeSigVertexId(3));
        assert_eq!(result.contextualizations.len(), 2);
    }

    #[test]
    fn hook_requires_a_non_center_stem_and_removes_its_dying_group() {
        let mut sig = NativeSigSystem {
            system_id: 4,
            vertices: vec![
                vertex(0, NativeSigInterKind::BeamGroup, 1.0),
                vertex(1, NativeSigInterKind::BeamHook, 0.7),
                vertex(2, NativeSigInterKind::Stem, 0.8),
                vertex(3, NativeSigInterKind::BeamHook, 0.7),
                vertex(4, NativeSigInterKind::Stem, 0.8),
            ],
            edges: vec![
                edge(0, 0, 1, NativeSigRelationKind::Containment),
                beam_stem_edge(1, 1, 2, NativeBeamPortion::Center),
                beam_stem_edge(2, 3, 4, NativeBeamPortion::Left),
            ],
        };

        let result = prune_native_foundation_hooks(&mut sig).unwrap();

        assert_eq!(result.removed_beams, vec![NativeSigVertexId(1)]);
        assert_eq!(result.removed_groups, vec![NativeSigVertexId(0)]);
        assert!(sig.vertices[0].removed);
        assert!(sig.vertices[1].removed);
        assert!(sig.vertices[3].active);
    }

    #[test]
    fn standard_beam_requires_both_side_stems_but_small_beam_is_not_scanned() {
        let mut sig = NativeSigSystem {
            system_id: 5,
            vertices: vec![
                vertex(0, NativeSigInterKind::Beam, 0.8),
                vertex(1, NativeSigInterKind::Stem, 0.8),
                vertex(2, NativeSigInterKind::Stem, 0.8),
                vertex(3, NativeSigInterKind::Beam, 0.8),
                vertex(4, NativeSigInterKind::Stem, 0.8),
                vertex(5, NativeSigInterKind::SmallBeam, 0.8),
            ],
            edges: vec![
                beam_stem_edge(0, 0, 1, NativeBeamPortion::Left),
                beam_stem_edge(1, 0, 2, NativeBeamPortion::Right),
                beam_stem_edge(2, 3, 4, NativeBeamPortion::Center),
            ],
        };

        let result = prune_native_foundation_beams(&mut sig).unwrap();

        assert_eq!(result.removed_beams, vec![NativeSigVertexId(3)]);
        assert!(sig.vertices[0].active);
        assert!(sig.vertices[3].removed);
        assert!(sig.vertices[5].active);
    }

    #[test]
    fn head_check_removes_invading_wrong_side_relation_in_java_order() {
        let mut beam = vertex(0, NativeSigInterKind::Beam, 0.95);
        beam.contextual_grade = Some(0.95);
        let stem = vertex(1, NativeSigInterKind::Stem, 0.8);
        let mut head = shaped_head(2, "NOTEHEAD_BLACK", 80);
        head.contextual_grade = Some(0.8);
        let mut wrong = head_stem_edge(1, 2, 1);
        wrong.head_stem.as_mut().unwrap().dx = 0.05;
        wrong.head_stem.as_mut().unwrap().dy = 0.0;
        wrong.head_stem.as_mut().unwrap().head_side = NativeStemHeadSide::Left;
        wrong.head_stem.as_mut().unwrap().extension_point = NativeStemPoint { x: 11.0, y: 82.0 };
        let mut beam_link = beam_stem_edge(0, 0, 1, NativeBeamPortion::Left);
        beam_link.stem_extension = Some(NativeStemPoint { x: 11.0, y: 2.0 });
        let mut sig = NativeSigSystem {
            system_id: 7,
            vertices: vec![beam, stem, head],
            edges: vec![beam_link, wrong],
        };
        let medians = BTreeMap::from([(
            NativeSigVertexId(1),
            NativeStemLine {
                start: NativeStemPoint { x: 11.0, y: 0.0 },
                stop: NativeStemPoint { x: 11.0, y: 100.0 },
            },
        )]);
        let identities = BTreeMap::from([(
            NativeSigVertexId(2),
            NativeReductionHeadIdentity {
                staff_id: 1,
                integer_pitch: 0,
            },
        )]);

        let transaction =
            prune_native_foundation_heads(&mut sig, &medians, &identities, &[]).unwrap();

        assert_eq!(transaction.head_order, vec![NativeSigVertexId(2)]);
        assert_eq!(
            transaction.mutations,
            vec![NativeReductionHeadMutation::WrongSideRelationRemoved {
                head: NativeSigVertexId(2),
                stem: NativeSigVertexId(1),
                relation: NativeSigEdgeId(1),
                exclusion: Some(NativeSigEdgeId(2)),
            }]
        );
        assert!(!sig.edges[1].active);
        assert_eq!(sig.edges[2].kind, NativeSigRelationKind::Exclusion);
    }

    #[test]
    fn head_check_lookup_remaps_merged_grand_staff_gutter_pitch() {
        let mut beam = vertex(0, NativeSigInterKind::Beam, 0.95);
        beam.contextual_grade = Some(0.95);
        let stem = vertex(1, NativeSigInterKind::Stem, 0.8);
        let first_head = shaped_head(2, "NOTEHEAD_BLACK", 80);
        let second_head = shaped_head(3, "NOTEHEAD_BLACK", 80);
        let mut first_link = head_stem_edge(1, 2, 1);
        first_link.head_stem.as_mut().unwrap().head_side = NativeStemHeadSide::Left;
        first_link.head_stem.as_mut().unwrap().extension_point =
            NativeStemPoint { x: 11.0, y: 82.0 };
        let mut second_link = head_stem_edge(2, 3, 1);
        second_link.head_stem.as_mut().unwrap().head_side = NativeStemHeadSide::Right;
        second_link.head_stem.as_mut().unwrap().extension_point =
            NativeStemPoint { x: 11.0, y: 82.0 };
        let mut beam_link = beam_stem_edge(0, 0, 1, NativeBeamPortion::Left);
        beam_link.stem_extension = Some(NativeStemPoint { x: 11.0, y: 2.0 });
        let mut sig = NativeSigSystem {
            system_id: 8,
            vertices: vec![beam, stem, first_head, second_head],
            edges: vec![beam_link, first_link, second_link],
        };
        let medians = BTreeMap::from([(
            NativeSigVertexId(1),
            NativeStemLine {
                start: NativeStemPoint { x: 11.0, y: 0.0 },
                stop: NativeStemPoint { x: 11.0, y: 100.0 },
            },
        )]);
        let identities = BTreeMap::from([
            (
                NativeSigVertexId(2),
                NativeReductionHeadIdentity {
                    staff_id: 10,
                    integer_pitch: 6,
                },
            ),
            (
                NativeSigVertexId(3),
                NativeReductionHeadIdentity {
                    staff_id: 11,
                    integer_pitch: -5,
                },
            ),
        ]);

        let transaction =
            prune_native_foundation_heads(&mut sig, &medians, &identities, &[(10, 11)]).unwrap();

        assert!(transaction.mutations.is_empty());
        assert!(
            sig.edges[1].active,
            "merged opposite-side head preserves link"
        );
    }

    #[test]
    fn unknown_direction_stem_removal_drops_based_head_support_then_orphan() {
        let stem = vertex(0, NativeSigInterKind::Stem, 0.8);
        let first = shaped_head(1, "NOTEHEAD_BLACK", 45);
        let second = shaped_head(2, "NOTEHEAD_BLACK", 50);
        let mut first_link = head_stem_edge(0, 1, 0);
        first_link.head_stem.as_mut().unwrap().extension_point =
            NativeStemPoint { x: 11.0, y: 48.0 };
        let mut second_link = head_stem_edge(1, 2, 0);
        second_link.head_stem.as_mut().unwrap().extension_point =
            NativeStemPoint { x: 11.0, y: 52.0 };
        let mut support = edge(2, 1, 2, NativeSigRelationKind::HeadHead);
        support.support = Some(NativeSigSupport {
            grade: 1.0,
            bar_connection_impacts: None,
        });
        let mut sig = NativeSigSystem {
            system_id: 9,
            vertices: vec![stem, first, second],
            edges: vec![first_link, second_link, support],
        };
        let medians = BTreeMap::from([(
            NativeSigVertexId(0),
            NativeStemLine {
                start: NativeStemPoint { x: 11.0, y: 0.0 },
                stop: NativeStemPoint { x: 11.0, y: 100.0 },
            },
        )]);

        let transaction =
            prune_native_foundation_heads(&mut sig, &medians, &BTreeMap::new(), &[]).unwrap();

        assert_eq!(
            transaction.mutations,
            vec![
                NativeReductionHeadMutation::UnknownDirectionStemRemoved {
                    head: NativeSigVertexId(1),
                    stem: NativeSigVertexId(0),
                    relation: NativeSigEdgeId(0),
                },
                NativeReductionHeadMutation::OrphanRemoved {
                    head: NativeSigVertexId(2),
                },
            ]
        );
        assert_eq!(
            transaction.removed_head_head_supports,
            vec![NativeSigEdgeId(2)]
        );
        assert!(sig.vertices[0].removed);
        assert!(sig.vertices[2].removed);
    }

    #[test]
    fn weak_purge_skips_frozen_and_ledgers() {
        let mut sig = NativeSigSystem {
            system_id: 6,
            vertices: vec![
                vertex(0, NativeSigInterKind::Head, 0.2),
                vertex(1, NativeSigInterKind::Clef, 0.1),
                vertex(2, NativeSigInterKind::Ledger, 0.1),
            ],
            edges: vec![],
        };
        sig.vertices[1].frozen = true;

        let result = contextualize_and_purge_native_weaks(&mut sig).unwrap();

        assert_eq!(result.removed_vertices, vec![NativeSigVertexId(0)]);
        assert!(result.cascaded_members.is_empty());
        assert!(sig.vertices[0].removed);
        assert!(sig.vertices[1].active);
        assert!(sig.vertices[2].active);
    }

    #[test]
    fn excluding_an_ensemble_cascades_only_uniquely_owned_members() {
        let mut sig = NativeSigSystem {
            system_id: 8,
            vertices: vec![
                vertex(0, NativeSigInterKind::BeamGroup, 0.1),
                vertex(1, NativeSigInterKind::Beam, 0.9),
                vertex(2, NativeSigInterKind::Head, 0.8),
            ],
            edges: vec![
                edge(0, 0, 1, NativeSigRelationKind::Containment),
                edge(1, 0, 2, NativeSigRelationKind::Exclusion),
            ],
        };

        let result = reduce_native_sig_exclusions(&mut sig).unwrap();

        assert_eq!(result.removed_vertices, vec![NativeSigVertexId(0)]);
        assert!(sig.vertices[0].removed);
        assert!(sig.vertices[1].removed);
        assert!(sig.vertices[2].active);
    }

    fn geometry_bounds(x: i32, y: i32, width: i32, height: i32) -> NativeSigBounds {
        NativeSigBounds {
            x,
            y,
            width,
            height,
        }
    }

    fn horizontal_glyph(
        left: i32,
        top: i32,
        width: usize,
        height: usize,
        runs: &[(usize, usize, usize)],
    ) -> NativeReductionGlyphGeometry {
        use audiveris_image::run_table::Run;

        let mut table = RunTable::new(Orientation::Horizontal, width, height).unwrap();
        for &(row, start, length) in runs {
            table.add_run(row, Run::new(start, length)).unwrap();
        }
        NativeReductionGlyphGeometry {
            left,
            top,
            run_table: table,
        }
    }

    fn glyph_geometry(
        bounds: NativeSigBounds,
        glyph: NativeReductionGlyphGeometry,
    ) -> NativeReductionInterGeometry {
        NativeReductionInterGeometry {
            bounds,
            core_bounds: bounds,
            implicit: false,
            glyph: Some(glyph),
            area: None,
            head: None,
            ensemble_members: Vec::new(),
        }
    }

    #[test]
    fn lossless_glyph_overlap_rejects_intersecting_boxes_without_shared_ink() {
        let bounds = geometry_bounds(10, 20, 5, 5);
        let one = glyph_geometry(bounds, horizontal_glyph(10, 20, 5, 5, &[(0, 0, 5)]));
        let two = glyph_geometry(bounds, horizontal_glyph(10, 20, 5, 5, &[(4, 0, 5)]));
        let mut resolver = NativeReductionLosslessOverlapResolver::new([
            (NativeSigVertexId(0), one),
            (NativeSigVertexId(1), two),
        ]);

        assert!(
            !resolver
                .mutually_overlaps(NativeSigVertexId(0), NativeSigVertexId(1))
                .unwrap()
        );
    }

    #[test]
    fn lossless_glyph_overlap_uses_absolute_foreground_pixels() {
        let bounds = geometry_bounds(10, 20, 5, 5);
        let one = glyph_geometry(bounds, horizontal_glyph(10, 20, 5, 5, &[(2, 0, 4)]));
        let two = glyph_geometry(bounds, horizontal_glyph(12, 22, 3, 3, &[(0, 0, 3)]));
        let mut resolver = NativeReductionLosslessOverlapResolver::new([
            (NativeSigVertexId(0), one),
            (NativeSigVertexId(1), two),
        ]);

        assert!(
            resolver
                .mutually_overlaps(NativeSigVertexId(0), NativeSigVertexId(1))
                .unwrap()
        );
    }

    #[test]
    fn lossless_area_glyph_overlap_uses_run_rectangles_not_glyph_bounds() {
        let area = NativeReductionAreaGeometry {
            components: vec![vec![
                NativeReductionAreaPoint { x: 12.0, y: 20.0 },
                NativeReductionAreaPoint { x: 14.0, y: 20.0 },
                NativeReductionAreaPoint { x: 14.0, y: 25.0 },
                NativeReductionAreaPoint { x: 12.0, y: 25.0 },
            ]],
        };
        let bounds = geometry_bounds(10, 20, 5, 5);
        let area_inter = NativeReductionInterGeometry {
            area: Some(area),
            ..NativeReductionInterGeometry::bounds_only(bounds)
        };
        let missed = glyph_geometry(bounds, horizontal_glyph(10, 20, 5, 5, &[(2, 0, 2)]));
        let hit = glyph_geometry(bounds, horizontal_glyph(10, 20, 5, 5, &[(2, 2, 2)]));

        let mut missed_resolver = NativeReductionLosslessOverlapResolver::new([
            (NativeSigVertexId(0), area_inter.clone()),
            (NativeSigVertexId(1), missed),
        ]);
        assert!(
            !missed_resolver
                .mutually_overlaps(NativeSigVertexId(0), NativeSigVertexId(1))
                .unwrap()
        );

        let mut hit_resolver = NativeReductionLosslessOverlapResolver::new([
            (NativeSigVertexId(0), area_inter),
            (NativeSigVertexId(1), hit),
        ]);
        assert!(
            hit_resolver
                .mutually_overlaps(NativeSigVertexId(0), NativeSigVertexId(1))
                .unwrap()
        );
    }

    #[test]
    fn head_overlap_preserves_staff_pitch_and_java_ratio_thresholds() {
        let one_bounds = geometry_bounds(10, 20, 10, 10);
        let two_bounds = geometry_bounds(12, 20, 10, 10);
        let glyph = |bounds: NativeSigBounds| {
            horizontal_glyph(
                bounds.x,
                bounds.y,
                bounds.width as usize,
                bounds.height as usize,
                &[(4, 0, bounds.width as usize)],
            )
        };
        let one = NativeReductionInterGeometry::head(one_bounds, glyph(one_bounds), Some(1), 0);
        let far_pitch =
            NativeReductionInterGeometry::head(two_bounds, glyph(two_bounds), Some(1), 2);
        let near_pitch =
            NativeReductionInterGeometry::head(two_bounds, glyph(two_bounds), Some(1), 1);

        let mut far = NativeReductionLosslessOverlapResolver::new([
            (NativeSigVertexId(0), one.clone()),
            (NativeSigVertexId(1), far_pitch),
        ]);
        assert!(
            !far.mutually_overlaps(NativeSigVertexId(0), NativeSigVertexId(1))
                .unwrap()
        );

        let mut near = NativeReductionLosslessOverlapResolver::new([
            (NativeSigVertexId(0), one),
            (NativeSigVertexId(1), near_pitch),
        ]);
        assert!(
            near.mutually_overlaps(NativeSigVertexId(0), NativeSigVertexId(1))
                .unwrap()
        );
    }

    #[test]
    fn missing_geometry_fails_before_rectangle_fallback() {
        let mut resolver = NativeReductionLosslessOverlapResolver::new([(
            NativeSigVertexId(0),
            NativeReductionInterGeometry::bounds_only(geometry_bounds(0, 0, 10, 10)),
        )]);

        assert_eq!(
            resolver
                .mutually_overlaps(NativeSigVertexId(0), NativeSigVertexId(1))
                .unwrap_err(),
            NativeReductionOverlapGeometryError::MissingGeometry(NativeSigVertexId(1))
        );
    }
}
