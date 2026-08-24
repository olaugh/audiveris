// SPDX-License-Identifier: AGPL-3.0-or-later

//! Native semantic primitives for Java's `ReductionStep`.
//!
//! The dependency-light lifecycle in [`crate::reduction_step`] owns sheet and
//! system ordering.  This module starts the production bridge from terminal
//! native STEMS SIGs with Java's deterministic `SIGraph.reduceExclusions()`
//! algorithm, lossless overlap discovery, chord prolog, and the contiguous
//! foundations prefix through the purge after `checkStemEndingHeads()`.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use audiveris_image::run_table::{Orientation, RunTable};

use crate::native_sig::{
    NativeSigContextualization, NativeSigEdge, NativeSigEdgeId, NativeSigError, NativeSigInterKind,
    NativeSigRelationKind, NativeSigRelationOrigin, NativeSigSystem, NativeSigVertexId,
};
use crate::native_stems::NativeStemsRecognition;
use crate::stems_step::{NativeBeamPortion, NativeStemHeadSide, NativeStemLine, NativeStemPoint};

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

/// The exact contiguous prefix of Java foundations reduction currently native:
/// overlap discovery through the purge after `checkStemEndingHeads()`.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeReductionFoundationPrefixTransaction {
    pub system_id: usize,
    pub overlap: NativeReductionOverlapTransaction,
    pub pre_prolog_contextualization: NativeSigContextualization,
    pub chord_analysis: NativeReductionChordAnalysisTransaction,
    pub initial_weak_purge: NativeReductionWeakPurgeTransaction,
    pub stem_ending: NativeReductionStemEndingTransaction,
    pub post_stem_ending_weak_purge: NativeReductionWeakPurgeTransaction,
}

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
    UnsupportedHeadShape {
        system_id: usize,
        head: NativeSigVertexId,
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
            Self::UnsupportedHeadShape { system_id, head } => write!(
                formatter,
                "REDUCTION system {system_id} chord analysis has unsupported head shape at {}",
                head.0
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
            | Self::UnsupportedHeadShape { .. } => None,
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

/// Execute the exact contiguous Java foundations prefix which is currently
/// native, stopping immediately before `checkHeads()`.
pub fn reduce_native_stems_foundation_prefix(
    stems: &mut NativeStemsRecognition,
    system_id: usize,
) -> Result<NativeReductionFoundationPrefixTransaction, NativeReductionFoundationPrefixError> {
    let mut resolver = native_stems_lossless_overlap_resolver(stems, system_id)
        .map_err(NativeReductionOverlapError::from)?;
    let medians = native_stems_terminal_medians(stems, system_id)?;
    let system = stems
        .systems
        .iter_mut()
        .find(|system| system.system_id == system_id)
        .ok_or(NativeReductionFoundationPrefixError::MissingSystem(
            system_id,
        ))?;
    reduce_native_foundation_prefix(
        &mut system.transaction.state_after.beam_state.sig,
        &mut resolver,
        &medians,
    )
}

/// Dependency-light foundations prefix used by the production STEMS adapter
/// and synthetic order/failure tests.
pub fn reduce_native_foundation_prefix(
    sig: &mut NativeSigSystem,
    geometry: &mut impl NativeReductionOverlapGeometry,
    stem_medians: &BTreeMap<NativeSigVertexId, NativeStemLine>,
) -> Result<NativeReductionFoundationPrefixTransaction, NativeReductionFoundationPrefixError> {
    let overlap = detect_native_reduction_overlaps(sig, geometry)?;
    // AdapterForFoundations.checkFrozens() is the inherited no-op.
    let pre_prolog_contextualization = sig.contextualize();
    let chord_analysis = analyze_native_foundation_chords(sig, stem_medians)?;
    let initial_weak_purge = contextualize_and_purge_native_weaks(sig)?;
    // AdapterForFoundations.checkSlurs() is the inherited empty set.
    let stem_ending = prune_native_foundation_stem_ending_heads(sig, stem_medians)?;
    let post_stem_ending_weak_purge = contextualize_and_purge_native_weaks(sig)?;
    Ok(NativeReductionFoundationPrefixTransaction {
        system_id: sig.system_id,
        overlap,
        pre_prolog_contextualization,
        chord_analysis,
        initial_weak_purge,
        stem_ending,
        post_stem_ending_weak_purge,
    })
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
        .filter(|vertex| vertex.active && vertex.kind == NativeSigInterKind::Head)
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
    prune_orphans_without_relation(
        sig,
        NativeSigInterKind::Head,
        NativeSigRelationKind::HeadStem,
    )
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
    sig.validate_integrity()?;
    let candidates = sig
        .vertices
        .iter()
        .filter(|vertex| vertex.active && vertex.kind == kind)
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
        NativeSigBounds, NativeSigEdge, NativeSigHeadStemPayload, NativeSigRelationOrigin,
        NativeSigSupport, NativeSigVertex,
    };
    use crate::stems_step::{NativeStemHeadSide, NativeStemPoint};
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
    fn foundation_prefix_stops_after_stem_ending_purge_in_java_order() {
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
            .extension_point = NativeStemPoint { x: 11.0, y: 82.0 };
        let medians = BTreeMap::from([(
            NativeSigVertexId(0),
            NativeStemLine {
                start: NativeStemPoint { x: 11.0, y: 0.0 },
                stop: NativeStemPoint { x: 11.0, y: 100.0 },
            },
        )]);
        let mut geometry = ScriptedOverlapGeometry::default();

        let transaction = reduce_native_foundation_prefix(&mut sig, &mut geometry, &medians)
            .expect("exact foundations prefix");

        assert_eq!(transaction.overlap.system_id, 41);
        assert_eq!(
            transaction.chord_analysis.scanned_stems,
            vec![NativeSigVertexId(0)]
        );
        assert!(transaction.initial_weak_purge.removed_vertices.is_empty());
        assert!(transaction.stem_ending.modified_stems.is_empty());
        assert!(
            transaction
                .post_stem_ending_weak_purge
                .removed_vertices
                .is_empty()
        );
        assert!(sig.vertex(0).is_some());
        assert!(sig.vertex(1).is_some());
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
                vertex(0, NativeSigInterKind::Head, 0.8),
                vertex(1, NativeSigInterKind::Head, 0.8),
                vertex(2, NativeSigInterKind::Head, 0.8),
                vertex(3, NativeSigInterKind::Stem, 0.8),
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
