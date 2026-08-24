// SPDX-License-Identifier: AGPL-3.0-or-later

//! Beam-origin `StemBuilder` construction through item and length retrieval.
//!
//! Java calls this once for every non-anchor beam `VLinker`, after reachable
//! beam/head targets have been found.  This product owns the local collection
//! mutations (`seeds`, target linkers, chunks and items), plus a deliberately
//! bounded model of the sheet glyph registry.  It stops before `VLinker.expand`
//! can make a `StemInter`, alter SIG, or attach a link.

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use audiveris_image::{
    beam_structure::Segment,
    run_table::{BACKGROUND, FOREGROUND, Orientation, RunTable, RunTableError},
    section::{Bounds, Section},
    stick_factory::{StraightStickError, VerticalStickFactory, VerticalStickParameters},
};

use crate::{
    beam_recognizer::run_table_center_line,
    grid_executor::{HeadlessSkew, HeadlessStaffLine},
    head_glyph_retrieval::RetrievedHeadGlyph,
    head_scanner_slices::JavaRectangle,
    native_heads::NativeHeadsRecognition,
    native_ledgers::NativeLedgerRecognition,
    native_stem_seeds::{
        NativeStemSeedDecision, NativeStemSeedGlyph, NativeStemSeedRecognition,
        contains_section_centroid,
    },
    native_stems_beam_reachability::{
        NativeStemsBeamArenaEntry, NativeStemsBeamHeadCornerRef,
        NativeStemsBeamReachabilityRecognition, NativeStemsBeamReachabilityTarget,
        NativeStemsBeamVInspection,
    },
    native_stems_beam_stumps::{
        NativeStemsBeamGlyph, NativeStemsBeamRegistration, NativeStemsBeamSource,
        NativeStemsBeamStumpBeam, NativeStemsBeamStumpRecognition, NativeStemsBeamStumpRef,
    },
    native_stems_beam_vlinkers::{
        NativeStemsBeamBLinker, NativeStemsBeamBLinkerRef, NativeStemsBeamVLinker,
        NativeStemsBeamVLinkerRecognition, NativeStemsBeamVLinkerRef,
        convex_quad_intersects_rectangle,
    },
    native_stems_head_corners::{NativeStemsHeadCorner, NativeStemsHeadCornerRecognition},
    native_stems_head_stumps::{
        NativeStemsHeadStumpBuild, NativeStemsHeadStumpOutcome, NativeStemsHeadStumpRecognition,
        NativeStemsHeadStumpRegistration,
    },
    recognize::{GridLinesRecognition, NativeBeamRecognition},
    stems_step::{NativeStemHeadSide, NativeStemLine, NativeStemPoint},
};

const MAX_LINE_SECTION_DX_RATIO: f64 = 0.3;
const MAX_STEM_ALIGNMENT_DX_RATIO: f64 = 0.15;
const MAX_STEM_ALIGNMENT_DY_RATIO: f64 = 4.0;
const MIN_SIDE_RATIO: f64 = 0.4;
const GAP_RATIOS: [f64; 5] = [0.0, 0.3, 0.6, 2.0, 4.0];

/// Full immutable product of every beam-origin Java `StemBuilder` constructor.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamBuilderRecognition {
    pub systems: Vec<NativeStemsBeamBuilderSystem>,
    pub builder_count: usize,
    pub item_count: usize,
    pub gap_count: usize,
    pub chunk_registration_count: usize,
    /// Exact structural sources seeded into the bounded glyph registry.
    pub registry_baseline: NativeStemsBeamBuilderRegistryBaseline,
}

/// Sheet-wide glyphs available before STEMS begins. Beam/head stump attempts
/// are deliberately excluded: each system exposes them in
/// `pre_builder_glyph_registrations` at its actual construction point.
///
/// The native product does not fabricate a complete Java `GlyphIndex` id map.
/// `NewInModeledRegistry` therefore means only that no supplied baseline or
/// earlier staged/chunk event has identical fixed glyph content.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NativeStemsBeamBuilderRegistryBaseline {
    pub staff_line_glyphs: usize,
    /// All raw StickFactory candidates, including candidates gated before
    /// `toGlyph`/registration.
    pub stem_seed_raw_candidates: usize,
    pub stem_seed_skipped_candidates: usize,
    /// Every checked candidate registered before the grade decision.
    pub stem_seed_glyphs: usize,
    pub stem_seed_rejected_glyphs: usize,
    /// Distinct fixed contents contributed by checked candidates. Structural
    /// duplicates still count separately in `stem_seed_glyphs` above.
    pub stem_seed_unique_contents: usize,
    pub beam_glyphs: usize,
    pub ledger_glyphs: usize,
    pub head_glyphs: usize,
    pub complete_java_glyph_index: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamBuilderSystem {
    pub system_id: usize,
    pub interline: i32,
    pub max_stem_thickness: i32,
    pub max_line_section_dx: f64,
    pub max_stem_alignment_dx: f64,
    pub max_stem_alignment_dy: f64,
    pub gap_map: BTreeMap<i32, i32>,
    /// Complete `SystemInfo.getVerticalSections()` dispatch order.
    pub vertical_section_source_ordinals: Vec<usize>,
    /// Complete `SystemInfo.getHorizontalSections()` dispatch order.
    pub horizontal_section_source_ordinals: Vec<usize>,
    /// Beam-side then head-corner registrations that Java made immediately
    /// before this system's beam-origin builders. This includes rejected head
    /// stump candidates, because registration precedes the extension check.
    pub pre_builder_glyph_registrations: Vec<NativeStemsBeamBuilderPreBuilderGlyphRegistration>,
    pub builders: Vec<NativeStemsBeamBuilder>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamBuilder {
    pub builder_ordinal: usize,
    /// Immutable equivalent of `VLinker.sb = this`; no input VLinker is
    /// mutated by this boundary.
    pub v_builder_assignment: NativeStemsBeamVLinkerRef,
    pub start: NativeStemsBeamVLinkerRef,
    pub max_stem_profile: i32,
    /// Direction retained from the originating `VLinker` inspection.
    pub v_y_direction: i32,
    /// Direction recomputed by the Java `StemBuilder` constructor from the
    /// theoretical line endpoints and used by every builder kernel.
    pub y_direction: i32,
    pub theoretical_line: NativeStemLine,
    pub lookup_quadrilateral: [NativeStemPoint; 4],
    pub lookup_bounds: JavaRectangle,
    pub y_range: JavaRectangle,
    pub start_stump: Option<NativeStemsBeamBuilderGlyphRef>,
    /// Seed free-glyph aliases as `VLinker` supplied them.
    pub input_seed_ordinals: Vec<usize>,
    /// The mutable seed collection after `filterUnaligned` / last-head purge.
    pub seeds_after_filter: Vec<usize>,
    pub target_input: Vec<NativeStemsBeamBuilderTargetRef>,
    pub targets_after_filter: Vec<NativeStemsBeamBuilderTargetRef>,
    pub alignment: Vec<NativeStemsBeamBuilderAlignmentPass>,
    /// One source-ordered lifecycle row for every input seed.
    pub seed_trace: Vec<NativeStemsBeamBuilderSeedTrace>,
    pub seed_filter: Vec<NativeStemsBeamBuilderSeedDecision>,
    pub target_filter: Vec<NativeStemsBeamBuilderTargetDecision>,
    /// Stable Java sort from `filter()` before `lastHeadY` is inspected.
    pub target_sort: Vec<NativeStemsBeamBuilderSortEntry>,
    pub last_head_y: Option<f64>,
    pub vertical_sections: Vec<NativeStemsBeamBuilderSectionScan>,
    pub horizontal_sections: Vec<NativeStemsBeamBuilderSectionScan>,
    pub filaments: Vec<NativeStemsBeamBuilderFilament>,
    pub glyph_registrations: Vec<NativeStemsBeamBuilderGlyphRegistration>,
    pub chunks: Vec<NativeStemsBeamBuilderChunk>,
    pub items_before_sort: Vec<NativeStemsBeamBuilderItem>,
    pub sort: Vec<NativeStemsBeamBuilderSortEntry>,
    pub gaps: Vec<NativeStemsBeamBuilderGapEvent>,
    pub items: Vec<NativeStemsBeamBuilderItem>,
    pub lengths: BTreeMap<i32, i32>,
    /// The boundary has no later mutable production state by construction.
    pub sig_mutation_count: usize,
    pub system_stem_mutation_count: usize,
    pub link_mutation_count: usize,
    pub head_builder_mutation_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamBuilderTargetRef {
    Beam(NativeStemsBeamBLinkerRef),
    Head(NativeStemsBeamHeadCornerRef),
}

impl From<NativeStemsBeamReachabilityTarget> for NativeStemsBeamBuilderTargetRef {
    fn from(value: NativeStemsBeamReachabilityTarget) -> Self {
        match value {
            NativeStemsBeamReachabilityTarget::Beam(value) => Self::Beam(value),
            NativeStemsBeamReachabilityTarget::Head(value) => Self::Head(value),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamBuilderGlyphRef {
    StemSeed {
        free_glyph_ordinal: usize,
    },
    BeamStump {
        b_linker: NativeStemsBeamBLinkerRef,
    },
    HeadStump {
        corner: NativeStemsBeamHeadCornerRef,
    },
    Chunk {
        builder_ordinal: usize,
        filament_ordinal: usize,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamBuilderAlignmentPass {
    pub subject: NativeStemsBeamBuilderAlignmentSubject,
    pub sorted_before: Vec<NativeStemsBeamBuilderGlyphRef>,
    pub comparisons: Vec<NativeStemsBeamBuilderAlignmentCheck>,
    pub removed: Vec<NativeStemsBeamBuilderGlyphRef>,
    pub retained: Vec<NativeStemsBeamBuilderGlyphRef>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamBuilderAlignmentSubject {
    Seeds,
    Chunks,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamBuilderAlignmentCheck {
    pub first: NativeStemsBeamBuilderGlyphRef,
    pub second: NativeStemsBeamBuilderGlyphRef,
    pub first_centroid: (f64, f64),
    pub second_centroid: (f64, f64),
    pub first_deskewed: (f64, f64),
    pub second_deskewed: (f64, f64),
    pub dx: f64,
    pub dy: f64,
    pub dy_bypasses_dx: bool,
    pub aligned: bool,
    /// The shorter adjacent glyph selected as `alien`.
    pub removed: Option<NativeStemsBeamBuilderGlyphRef>,
    /// `ArrayList.remove(Object)` removes the first content-equal occurrence.
    pub removed_list_occurrence: Option<NativeStemsBeamBuilderGlyphRef>,
    pub equal_height_removed_second: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamBuilderSeedDecision {
    pub glyph: NativeStemsBeamBuilderGlyphRef,
    pub retained_in_mutable_seed_set: bool,
    pub included_as_item: bool,
    pub action: NativeStemsBeamBuilderSeedAction,
    pub contribution: Option<i32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamBuilderSeedTrace {
    pub input_ordinal: usize,
    pub glyph: NativeStemsBeamBuilderGlyphRef,
    pub survives_alignment: bool,
    pub survives_last_head_mutation: bool,
    pub item_action: Option<NativeStemsBeamBuilderSeedAction>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamBuilderSeedAction {
    AlignmentRemoved,
    PastLastHead,
    DuplicateTargetIdentity,
    OverlapsStart,
    ZeroContribution,
    Item,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamBuilderTargetDecision {
    pub target: NativeStemsBeamBuilderTargetRef,
    pub stump: Option<NativeStemsBeamBuilderGlyphRef>,
    pub removed_by_content: bool,
    pub included: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamBuilderSectionScan {
    pub source_ordinal: usize,
    pub bounds: Bounds,
    pub intersects_lookup: bool,
    pub width_accepted: Option<bool>,
    pub stump_overlap_accepted: Option<bool>,
    pub before_last_head: Option<bool>,
    pub line_distance: Option<f64>,
    pub distance_accepted: Option<bool>,
    pub accepted: bool,
    pub accepted_sorted_ordinal: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamBuilderFilament {
    pub filament_ordinal: usize,
    pub creation_id: u64,
    pub member_section_source_ordinals: Vec<NativeStemsBeamBuilderSectionRef>,
    pub bounds: Bounds,
    pub weight: usize,
    pub start: (f64, f64),
    pub stop: (f64, f64),
    pub mean_thickness: f64,
    pub mean_distance: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamBuilderSectionRef {
    pub orientation: Orientation,
    pub source_ordinal: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamBuilderGlyphRegistration {
    pub glyph: NativeStemsBeamBuilderGlyphRef,
    pub bounds: Bounds,
    pub weight: usize,
    pub run_table: RunTable,
    /// Dense semantic alias for the structural canonical object in this
    /// bounded registry. It is deliberately not a Java `GlyphIndex` number.
    pub modeled_canonical_ordinal: usize,
    pub action: NativeStemsBeamBuilderRegistrationAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamBuilderRegistrationAction {
    ReusedModeledCanonical,
    /// The bounded registry had no equal glyph. Its Java-global novelty is
    /// explicitly unresolved unless the supplied baseline is complete.
    NewInModeledRegistry {
        global_novelty: NativeStemsBeamBuilderGlobalNovelty,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamBuilderGlobalNovelty {
    ProvenWithCompleteBaseline,
    UnresolvedIncompleteBaseline,
}

/// A registration already performed by a beam/head linker constructor before
/// the `StemBuilder` constructor begins for a system.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamBuilderPreBuilderGlyphRegistration {
    /// System-local, beam-side then head-corner constructor event order.
    pub event_ordinal: usize,
    pub source: NativeStemsBeamBuilderPreBuilderGlyphSource,
    pub bounds: Bounds,
    pub run_table: RunTable,
    pub modeled_canonical_ordinal: usize,
    /// The stump product's actual recorded `registerOriginal` outcome and
    /// origin, kept separate from the bounded-registry replay action.
    pub upstream_registration: NativeStemsBeamBuilderUpstreamRegistration,
    pub attachment: NativeStemsBeamBuilderPreBuilderAttachment,
    pub action: NativeStemsBeamBuilderRegistrationAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamBuilderPreBuilderGlyphSource {
    BeamStump {
        beam_x_ordinal: usize,
        beam: NativeStemsBeamSource,
        side: NativeStemHeadSide,
    },
    HeadStump {
        head_x_ordinal: usize,
        head_sig_ordinal: usize,
        constructor_ordinal: usize,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamBuilderUpstreamRegistration {
    Beam(NativeStemsBeamRegistration),
    Head(NativeStemsHeadStumpRegistration),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamBuilderPreBuilderAttachment {
    Attached,
    RejectedAfterRegistration,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamBuilderChunk {
    pub glyph: NativeStemsBeamBuilderGlyphRef,
    pub bounds: Bounds,
    pub run_table: RunTable,
    pub removed_by_seed_content: bool,
    pub removed_by_alignment_content: bool,
    pub removed_as_start_content: bool,
    pub retained: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamBuilderSortEntry {
    pub before_ordinal: usize,
    pub after_ordinal: usize,
    pub item: NativeStemsBeamBuilderItem,
    pub tied_with_previous: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamBuilderGapEvent {
    pub item_ordinal_before_insert: usize,
    pub previous_stop: NativeStemPoint,
    pub next_start: NativeStemPoint,
    pub gap: f64,
    pub max_gap: i32,
    pub action: NativeStemsBeamBuilderGapAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamBuilderGapAction {
    None,
    Inserted,
    Truncated,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamBuilderItem {
    pub kind: NativeStemsBeamBuilderItemKind,
    pub glyph: Option<NativeStemsBeamBuilderGlyphRef>,
    pub target: Option<NativeStemsBeamBuilderTargetRef>,
    /// The `HalfLinkerItem` reference point used for half-linker pair sorting.
    pub reference_point: Option<NativeStemPoint>,
    /// Java `getLengthAt` unions this only for head `LinkerItem`s.
    pub head_bounds: Option<JavaRectangle>,
    pub line: NativeStemLine,
    pub contribution: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamBuilderItemKind {
    StartHalfLinker,
    BeamLinker,
    HeadHalfLinker,
    SeedGlyph,
    ChunkGlyph,
    Gap,
}

#[derive(Debug)]
pub enum NativeStemsBeamBuilderError {
    SystemOrder,
    InvalidParameters {
        system_id: usize,
    },
    MissingSystemArea(usize),
    MissingSystemProduct {
        system_id: usize,
    },
    MissingVLinker {
        system_id: usize,
        reference: NativeStemsBeamVLinkerRef,
    },
    MissingBLinker {
        system_id: usize,
        reference: NativeStemsBeamBLinkerRef,
    },
    MissingBeam {
        system_id: usize,
        source: NativeStemsBeamSource,
    },
    MissingHeadCorner {
        system_id: usize,
        corner: NativeStemsBeamHeadCornerRef,
    },
    MissingHeadStump {
        system_id: usize,
        corner: NativeStemsBeamHeadCornerRef,
    },
    MissingSeed {
        system_id: usize,
        free_glyph_ordinal: usize,
    },
    MissingStumpGlyph {
        system_id: usize,
    },
    /// Java-global glyph-index novelty cannot be proven from this bounded
    /// structural baseline; callers must supply a complete snapshot for it.
    UnsupportedBaselineCollision {
        system_id: usize,
    },
    /// The frozen corpus never enters JDK TimSort's merge path. Refuse a
    /// larger list rather than silently substitute Rust's sort while the full
    /// object-TimSort port remains outside this boundary.
    UnsupportedJdkTimSortLength {
        system_id: usize,
        length: usize,
    },
    Geometry {
        system_id: usize,
    },
    StickFactory {
        system_id: usize,
        source: StraightStickError,
    },
    RunTable(RunTableError),
}

impl fmt::Display for NativeStemsBeamBuilderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid beam StemBuilder production boundary: {self:?}"
        )
    }
}

impl Error for NativeStemsBeamBuilderError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::StickFactory { source, .. } => Some(source),
            Self::RunTable(source) => Some(source),
            _ => None,
        }
    }
}

impl From<RunTableError> for NativeStemsBeamBuilderError {
    fn from(source: RunTableError) -> Self {
        Self::RunTable(source)
    }
}

/// Materialize all real beam `VLinker` builders. Inputs remain immutable: the
/// Java-local mutable collections are exposed on each returned builder.
#[allow(clippy::too_many_arguments)]
pub fn materialize_native_stems_beam_builders(
    grid: &GridLinesRecognition,
    beams: &NativeBeamRecognition,
    ledgers: &NativeLedgerRecognition,
    heads: &NativeHeadsRecognition,
    stem_seeds: &NativeStemSeedRecognition,
    beam_stumps: &NativeStemsBeamStumpRecognition,
    beam_vlinkers: &NativeStemsBeamVLinkerRecognition,
    head_corners: &NativeStemsHeadCornerRecognition,
    head_stumps: &NativeStemsHeadStumpRecognition,
    reachability: &NativeStemsBeamReachabilityRecognition,
) -> Result<NativeStemsBeamBuilderRecognition, NativeStemsBeamBuilderError> {
    let ids = grid
        .peak_graph
        .sig
        .systems
        .iter()
        .map(|system| system.system_id)
        .collect::<Vec<_>>();
    if ids
        != stem_seeds
            .systems
            .iter()
            .map(|system| system.raw.system_id)
            .collect::<Vec<_>>()
        || ids
            != beam_stumps
                .systems
                .iter()
                .map(|system| system.system_id)
                .collect::<Vec<_>>()
        || ids
            != beam_vlinkers
                .systems
                .iter()
                .map(|system| system.system_id)
                .collect::<Vec<_>>()
        || ids
            != head_corners
                .systems
                .iter()
                .map(|system| system.system_id)
                .collect::<Vec<_>>()
        || ids
            != head_stumps
                .systems
                .iter()
                .map(|system| system.system_id)
                .collect::<Vec<_>>()
        || ids
            != reachability
                .systems
                .iter()
                .map(|system| system.system_id)
                .collect::<Vec<_>>()
    {
        return Err(NativeStemsBeamBuilderError::SystemOrder);
    }

    // Only products that predate STEMS are available sheet-wide. Beam/head
    // stumps are registered during each system's linker construction, so they
    // are replayed in that system immediately before its builders below.
    let mut registry = GlyphRegistry::seeded(grid, beams, ledgers, heads, stem_seeds)?;
    let baseline = registry.baseline.clone();
    let mut systems = Vec::with_capacity(ids.len());
    for (
        ((((grid_system, seed_system), stump_system), v_system), corner_system),
        (head_stump_system, reach_system),
    ) in grid
        .peak_graph
        .sig
        .systems
        .iter()
        .zip(&stem_seeds.systems)
        .zip(&beam_stumps.systems)
        .zip(&beam_vlinkers.systems)
        .zip(&head_corners.systems)
        .zip(head_stumps.systems.iter().zip(&reachability.systems))
    {
        let system_id = grid_system.system_id;
        let interline = seed_system.raw.interline;
        if interline <= 0 || seed_system.raw.maximum_stem_thickness <= 0 {
            return Err(NativeStemsBeamBuilderError::InvalidParameters { system_id });
        }
        let area = grid
            .system_areas
            .iter()
            .find(|area| area.system_id == system_id)
            .ok_or(NativeStemsBeamBuilderError::MissingSystemArea(system_id))?;
        let vertical_section_source_ordinals =
            dispatch_section_ordinals(&grid.peak_graph.vertical_sections, area);
        let horizontal_section_source_ordinals =
            dispatch_section_ordinals(&grid.peak_graph.horizontal_sections, area);
        let params = BuilderParams::new(interline, seed_system.raw.maximum_stem_thickness)?;
        let context = BuilderContext {
            system_id,
            grid,
            seed_system,
            stump_system,
            v_system,
            corner_system,
            head_stump_system,
            reach_system,
            params,
            vertical_section_source_ordinals: &vertical_section_source_ordinals,
            horizontal_section_source_ordinals: &horizontal_section_source_ordinals,
        };
        let pre_builder_glyph_registrations =
            registry.replay_system_stumps(stump_system, head_stump_system);
        let mut builders = Vec::new();
        for beam_inspection in &reach_system.beam_inspections {
            for b_visit in &beam_inspection.b_visits {
                if b_visit.skipped_anchor {
                    continue;
                }
                for inspection in &b_visit.v_inspections {
                    let builder =
                        materialize_builder(&context, inspection, builders.len(), &mut registry)?;
                    builders.push(builder);
                }
            }
        }
        systems.push(NativeStemsBeamBuilderSystem {
            system_id,
            interline,
            max_stem_thickness: seed_system.raw.maximum_stem_thickness,
            max_line_section_dx: params.max_line_section_dx,
            max_stem_alignment_dx: params.max_stem_alignment_dx,
            max_stem_alignment_dy: params.max_stem_alignment_dy,
            gap_map: params.map(),
            vertical_section_source_ordinals,
            horizontal_section_source_ordinals,
            pre_builder_glyph_registrations,
            builders,
        });
    }
    let builder_count = systems.iter().map(|system| system.builders.len()).sum();
    let item_count = systems
        .iter()
        .flat_map(|system| &system.builders)
        .map(|builder| builder.items.len())
        .sum();
    let gap_count = systems
        .iter()
        .flat_map(|system| &system.builders)
        .flat_map(|builder| &builder.items)
        .filter(|item| item.kind == NativeStemsBeamBuilderItemKind::Gap)
        .count();
    let chunk_registration_count = systems
        .iter()
        .flat_map(|system| &system.builders)
        .map(|builder| builder.glyph_registrations.len())
        .sum();
    Ok(NativeStemsBeamBuilderRecognition {
        systems,
        builder_count,
        item_count,
        gap_count,
        chunk_registration_count,
        registry_baseline: baseline,
    })
}

#[derive(Clone, Copy)]
struct BuilderParams {
    max_stem_thickness: i32,
    max_line_section_dx: f64,
    max_stem_alignment_dx: f64,
    max_stem_alignment_dy: f64,
    gap_map: [i32; 5],
}

impl BuilderParams {
    fn new(interline: i32, max_stem_thickness: i32) -> Result<Self, NativeStemsBeamBuilderError> {
        if interline <= 0 || max_stem_thickness <= 0 {
            return Err(NativeStemsBeamBuilderError::InvalidParameters { system_id: 0 });
        }
        let to_pixels = |ratio: f64| java_rint(f64::from(interline) * ratio);
        Ok(Self {
            max_stem_thickness,
            max_line_section_dx: f64::from(interline) * MAX_LINE_SECTION_DX_RATIO,
            max_stem_alignment_dx: f64::from(interline) * MAX_STEM_ALIGNMENT_DX_RATIO,
            max_stem_alignment_dy: f64::from(interline) * MAX_STEM_ALIGNMENT_DY_RATIO,
            gap_map: GAP_RATIOS.map(to_pixels),
        })
    }

    fn map(self) -> BTreeMap<i32, i32> {
        self.gap_map
            .into_iter()
            .enumerate()
            .map(|(profile, value)| (profile as i32, value))
            .collect()
    }
}

struct BuilderContext<'a> {
    system_id: usize,
    grid: &'a GridLinesRecognition,
    seed_system: &'a crate::native_stem_seeds::NativeStemSeedSystemRecognition,
    stump_system: &'a crate::native_stems_beam_stumps::NativeStemsBeamStumpSystem,
    v_system: &'a crate::native_stems_beam_vlinkers::NativeStemsBeamVLinkerSystem,
    corner_system: &'a crate::native_stems_head_corners::NativeStemsHeadCornerSystem,
    head_stump_system: &'a crate::native_stems_head_stumps::NativeStemsHeadStumpSystem,
    reach_system: &'a crate::native_stems_beam_reachability::NativeStemsBeamReachabilitySystem,
    params: BuilderParams,
    vertical_section_source_ordinals: &'a [usize],
    horizontal_section_source_ordinals: &'a [usize],
}

fn materialize_builder(
    context: &BuilderContext<'_>,
    inspection: &NativeStemsBeamVInspection,
    builder_ordinal: usize,
    registry: &mut GlyphRegistry,
) -> Result<NativeStemsBeamBuilder, NativeStemsBeamBuilderError> {
    let v_linker = find_v_linker(context.v_system, inspection.reference).ok_or(
        NativeStemsBeamBuilderError::MissingVLinker {
            system_id: context.system_id,
            reference: inspection.reference,
        },
    )?;
    if inspection.y_direction != v_linker.y_direction
        || inspection.theoretical_line != v_linker.final_geometry.theoretical_line
        || inspection.lookup_quadrilateral != v_linker.final_geometry.quadrilateral
    {
        return Err(NativeStemsBeamBuilderError::Geometry {
            system_id: context.system_id,
        });
    }
    let y_direction = stem_builder_y_direction(inspection.theoretical_line);
    let y_range = line_bounds(inspection.theoretical_line);
    let start_stump = stump_for_b(context, inspection.reference.b_linker)?;
    let start_glyph = start_stump
        .map(|reference| resolve_registered_glyph(context, registry, reference))
        .transpose()?;
    let input_seed_ordinals = v_linker.reachable_seed_ordinals.clone();
    let mut seeds = input_seed_ordinals
        .iter()
        .copied()
        .map(|ordinal| {
            let reference = NativeStemsBeamBuilderGlyphRef::StemSeed {
                free_glyph_ordinal: ordinal,
            };
            resolve_registered_glyph(context, registry, reference).map(|glyph| (reference, glyph))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let input_seed_glyphs = seeds.clone();
    let targets = inspection
        .ordered_targets
        .iter()
        .copied()
        .map(Into::into)
        .collect::<Vec<NativeStemsBeamBuilderTargetRef>>();
    let mut target_states = targets
        .iter()
        .copied()
        .map(|target| {
            let stump = stump_for_target(context, target)?
                .map(|reference| resolve_registered_glyph(context, registry, reference))
                .transpose()?;
            Ok(TargetState { target, stump })
        })
        .collect::<Result<Vec<_>, NativeStemsBeamBuilderError>>()?;
    let skew = HeadlessSkew::new(
        context.grid.global_slope,
        i32::try_from(context.grid.no_staff.width()).map_err(|_| {
            NativeStemsBeamBuilderError::Geometry {
                system_id: context.system_id,
            }
        })?,
        i32::try_from(context.grid.no_staff.height()).map_err(|_| {
            NativeStemsBeamBuilderError::Geometry {
                system_id: context.system_id,
            }
        })?,
    );

    let mut alignment = Vec::new();
    let (seed_alignment, removed_seed_content) = filter_unaligned(
        &mut seeds,
        start_stump.map(|reference| {
            (
                reference,
                start_glyph.clone().expect("resolved start glyph"),
            )
        }),
        y_direction,
        &skew,
        context.params,
        NativeStemsBeamBuilderAlignmentSubject::Seeds,
    );
    let removed_seed_glyphs = seed_alignment.removed.clone();
    alignment.push(seed_alignment);
    let mut target_filter = Vec::with_capacity(target_states.len());
    target_states.retain(|state| {
        let removed = state.stump.as_ref().is_some_and(|stump| {
            removed_seed_content
                .iter()
                .any(|removed| same_content(removed, stump))
        });
        target_filter.push(NativeStemsBeamBuilderTargetDecision {
            target: state.target,
            stump: state.stump.as_ref().map(|glyph| glyph.reference),
            removed_by_content: removed,
            included: !removed,
        });
        !removed
    });
    let mut target_items = target_states
        .iter()
        .map(|state| target_item(context, state, y_range))
        .collect::<Result<Vec<_>, _>>()?;
    let target_items_before_sort = target_items.clone();
    stable_sort_items(&mut target_items, y_direction, context.system_id)?;
    let target_sort = sort_permutation(&target_items_before_sort, &target_items, y_direction);
    let last_head_y = target_items.iter().rev().find_map(|item| {
        (item.kind == NativeStemsBeamBuilderItemKind::HeadHalfLinker).then_some(
            item.reference_point
                .map_or_else(|| item_reference_y(item), |point| point.y),
        )
    });
    let mut seed_filter = Vec::new();
    if let Some(last_head_y) = last_head_y {
        let mut removed = Vec::new();
        for (reference, glyph) in &seeds {
            let center = integer_center(glyph.bounds);
            if y_direction * java_double_compare(f64::from(center.1), last_head_y) >= 0 {
                removed.push((*reference, glyph.clone()));
            }
        }
        seeds.retain(|(_, glyph)| {
            !removed
                .iter()
                .any(|(_, removed)| same_content(glyph, removed))
        });
        for (reference, _) in removed {
            seed_filter.push(NativeStemsBeamBuilderSeedDecision {
                glyph: reference,
                retained_in_mutable_seed_set: false,
                included_as_item: false,
                action: NativeStemsBeamBuilderSeedAction::PastLastHead,
                contribution: None,
            });
        }
    }
    for reference in &removed_seed_glyphs {
        seed_filter.push(NativeStemsBeamBuilderSeedDecision {
            glyph: *reference,
            retained_in_mutable_seed_set: false,
            included_as_item: false,
            action: NativeStemsBeamBuilderSeedAction::AlignmentRemoved,
            contribution: None,
        });
    }
    let start_box = start_glyph.as_ref().map(|glyph| glyph.bounds);
    let mut filtered_seed_items = Vec::new();
    for (reference, glyph) in &seeds {
        let duplicate = target_states.iter().any(|target| {
            target
                .stump
                .as_ref()
                .is_some_and(|stump| same_identity(stump, glyph))
        });
        if duplicate {
            seed_filter.push(NativeStemsBeamBuilderSeedDecision {
                glyph: *reference,
                retained_in_mutable_seed_set: true,
                included_as_item: false,
                action: NativeStemsBeamBuilderSeedAction::DuplicateTargetIdentity,
                contribution: None,
            });
            continue;
        }
        if start_box.is_some_and(|start_box| y_overlap(start_box, glyph.bounds) > 0) {
            seed_filter.push(NativeStemsBeamBuilderSeedDecision {
                glyph: *reference,
                retained_in_mutable_seed_set: true,
                included_as_item: false,
                action: NativeStemsBeamBuilderSeedAction::OverlapsStart,
                contribution: None,
            });
            continue;
        }
        let contribution = contribution(y_range, glyph.bounds);
        if contribution <= 0 {
            seed_filter.push(NativeStemsBeamBuilderSeedDecision {
                glyph: *reference,
                retained_in_mutable_seed_set: true,
                included_as_item: false,
                action: NativeStemsBeamBuilderSeedAction::ZeroContribution,
                contribution: Some(contribution),
            });
            continue;
        }
        seed_filter.push(NativeStemsBeamBuilderSeedDecision {
            glyph: *reference,
            retained_in_mutable_seed_set: true,
            included_as_item: true,
            action: NativeStemsBeamBuilderSeedAction::Item,
            contribution: Some(contribution),
        });
        filtered_seed_items.push(glyph_item(
            NativeStemsBeamBuilderItemKind::SeedGlyph,
            *reference,
            glyph,
            contribution,
        )?);
    }

    let (vertical_sections, selected_vertical) = scan_vertical_sections(
        context,
        inspection,
        y_direction,
        start_glyph.as_ref(),
        last_head_y,
    )?;
    let (horizontal_sections, selected_horizontal) =
        scan_horizontal_sections(context, inspection, y_direction, last_head_y)?;
    let factory = VerticalStickFactory::new(VerticalStickParameters {
        interline: usize::try_from(context.seed_system.raw.interline).map_err(|_| {
            NativeStemsBeamBuilderError::Geometry {
                system_id: context.system_id,
            }
        })?,
        maximum_stick_thickness: usize::try_from(context.params.max_stem_thickness).map_err(
            |_| NativeStemsBeamBuilderError::Geometry {
                system_id: context.system_id,
            },
        )?,
        minimum_core_section_length: 0,
        minimum_side_ratio: MIN_SIDE_RATIO,
    });
    let outcome = factory.retrieve_sticks(&selected_vertical, &selected_horizontal, 1);
    if let Some(source) = outcome.error {
        return Err(NativeStemsBeamBuilderError::StickFactory {
            system_id: context.system_id,
            source,
        });
    }
    let mut filaments = Vec::new();
    let mut registrations = Vec::new();
    let mut chunk_work = Vec::new();
    for (filament_ordinal, (filament, creation_id)) in outcome
        .result
        .survivors()
        .iter()
        .zip(outcome.result.creation_ids())
        .enumerate()
    {
        let geometry = filament.straight_geometry().map_err(|source| {
            NativeStemsBeamBuilderError::StickFactory {
                system_id: context.system_id,
                source,
            }
        })?;
        let reference = NativeStemsBeamBuilderGlyphRef::Chunk {
            builder_ordinal,
            filament_ordinal,
        };
        let mut glyph = glyph_from_sections(reference, filament.sections(), geometry)?;
        let registration = registry.register(&mut glyph);
        registrations.push(NativeStemsBeamBuilderGlyphRegistration {
            glyph: reference,
            bounds: glyph.bounds,
            weight: glyph.weight,
            run_table: glyph.run_table.clone(),
            modeled_canonical_ordinal: registration.modeled_canonical_ordinal,
            action: registration.action,
        });
        filaments.push(NativeStemsBeamBuilderFilament {
            filament_ordinal,
            creation_id: *creation_id,
            member_section_source_ordinals: filament
                .sections()
                .iter()
                .map(|section| source_section_ref(context, section))
                .collect::<Result<_, _>>()?,
            bounds: geometry.bounds,
            weight: filament.weight(),
            start: geometry.start,
            stop: geometry.stop,
            mean_thickness: geometry.mean_thickness,
            mean_distance: geometry.mean_distance,
        });
        chunk_work.push((reference, glyph));
    }
    let seeds_before_chunks = seeds.clone();
    let mut chunks = Vec::new();
    chunk_work.retain(|(_, glyph)| {
        let removed = seeds_before_chunks
            .iter()
            .any(|(_, seed)| same_content(seed, glyph));
        chunks.push(NativeStemsBeamBuilderChunk {
            glyph: glyph.reference,
            bounds: glyph.bounds,
            run_table: glyph.run_table.clone(),
            removed_by_seed_content: removed,
            removed_by_alignment_content: false,
            removed_as_start_content: false,
            retained: !removed,
        });
        !removed
    });
    let (chunk_alignment, removed_chunk_content) = filter_unaligned(
        &mut chunk_work,
        start_stump.map(|reference| {
            (
                reference,
                start_glyph.clone().expect("resolved start glyph"),
            )
        }),
        y_direction,
        &skew,
        context.params,
        NativeStemsBeamBuilderAlignmentSubject::Chunks,
    );
    for removed in &removed_chunk_content {
        for chunk in &mut chunks {
            if chunk.bounds == removed.bounds && chunk.run_table == removed.run_table {
                chunk.removed_by_alignment_content = true;
                chunk.retained = false;
            }
        }
    }
    alignment.push(chunk_alignment);
    if let Some(start) = &start_glyph {
        if let Some(index) = chunk_work
            .iter()
            .position(|(_, chunk)| same_content(chunk, start))
        {
            let (reference, _) = chunk_work.remove(index);
            if let Some(chunk) = chunks.iter_mut().find(|chunk| chunk.glyph == reference) {
                chunk.removed_as_start_content = true;
                chunk.retained = false;
            }
        }
    }
    stable_sort_glyphs(&mut chunk_work, y_direction);
    let mut items = Vec::new();
    let start_reference = find_b_linker(context.v_system, inspection.reference.b_linker)
        .ok_or(NativeStemsBeamBuilderError::MissingBLinker {
            system_id: context.system_id,
            reference: inspection.reference.b_linker,
        })?
        .reference_point;
    items.push(start_item(
        start_stump,
        start_glyph.as_ref(),
        start_reference,
        y_range,
    )?);
    items.extend(target_items);
    items.extend(filtered_seed_items);
    for (reference, chunk) in &chunk_work {
        items.push(glyph_item(
            NativeStemsBeamBuilderItemKind::ChunkGlyph,
            *reference,
            chunk,
            contribution(y_range, chunk.bounds),
        )?);
    }
    let items_before_sort = items.clone();
    stable_sort_items(&mut items[1..], y_direction, context.system_id)?;
    let sort = sort_permutation(&items_before_sort[1..], &items[1..], y_direction);
    let gaps = insert_gaps(
        &mut items,
        y_direction,
        context.params.gap_map[usize::try_from(inspection.max_stem_profile).map_err(|_| {
            NativeStemsBeamBuilderError::Geometry {
                system_id: context.system_id,
            }
        })?],
    );
    let lengths = retrieve_lengths(
        &items,
        y_direction,
        inspection.theoretical_line,
        context.params.map(),
        inspection.max_stem_profile,
    )?;
    let seed_trace = input_seed_glyphs
        .iter()
        .enumerate()
        .map(|(input_ordinal, (glyph, input_glyph))| {
            let survives_alignment = !removed_seed_content
                .iter()
                .any(|removed| same_content(input_glyph, removed));
            let survives_last_head_mutation = survives_alignment
                && seeds
                    .iter()
                    .any(|(_, retained)| same_content(input_glyph, retained));
            let item_action = if !survives_alignment {
                Some(NativeStemsBeamBuilderSeedAction::AlignmentRemoved)
            } else if !survives_last_head_mutation {
                Some(NativeStemsBeamBuilderSeedAction::PastLastHead)
            } else {
                seed_filter
                    .iter()
                    .find(|decision| decision.glyph == *glyph)
                    .map(|decision| decision.action)
            };
            NativeStemsBeamBuilderSeedTrace {
                input_ordinal,
                glyph: *glyph,
                survives_alignment,
                survives_last_head_mutation,
                item_action,
            }
        })
        .collect();
    Ok(NativeStemsBeamBuilder {
        builder_ordinal,
        v_builder_assignment: inspection.reference,
        start: inspection.reference,
        max_stem_profile: inspection.max_stem_profile,
        v_y_direction: inspection.y_direction,
        y_direction,
        theoretical_line: inspection.theoretical_line,
        lookup_quadrilateral: inspection.lookup_quadrilateral,
        lookup_bounds: inspection.lookup_bounds,
        y_range,
        start_stump,
        input_seed_ordinals,
        seeds_after_filter: seeds
            .iter()
            .map(|(reference, _)| match reference {
                NativeStemsBeamBuilderGlyphRef::StemSeed { free_glyph_ordinal } => {
                    *free_glyph_ordinal
                }
                _ => unreachable!(),
            })
            .collect(),
        target_input: targets,
        targets_after_filter: target_states.iter().map(|state| state.target).collect(),
        alignment,
        seed_trace,
        seed_filter,
        target_filter,
        target_sort,
        last_head_y,
        vertical_sections,
        horizontal_sections,
        filaments,
        glyph_registrations: registrations,
        chunks,
        items_before_sort,
        sort,
        gaps,
        items,
        lengths,
        sig_mutation_count: 0,
        system_stem_mutation_count: 0,
        link_mutation_count: 0,
        head_builder_mutation_count: 0,
    })
}

#[derive(Clone)]
struct FixedGlyph {
    reference: NativeStemsBeamBuilderGlyphRef,
    /// `GlyphIndex.registerOriginal` canonicalizes exact fixed glyph content.
    identity: GlyphKey,
    /// Dense modeled object identity, assigned when an input glyph is found in
    /// the staged registry or when a chunk is registered.
    modeled_canonical_ordinal: Option<usize>,
    bounds: Bounds,
    weight: usize,
    run_table: RunTable,
    line: Segment,
}

#[derive(Clone)]
struct TargetState {
    target: NativeStemsBeamBuilderTargetRef,
    stump: Option<FixedGlyph>,
}

fn find_v_linker(
    system: &crate::native_stems_beam_vlinkers::NativeStemsBeamVLinkerSystem,
    reference: NativeStemsBeamVLinkerRef,
) -> Option<&NativeStemsBeamVLinker> {
    system
        .constructors
        .iter()
        .find(|constructor| constructor.source == reference.b_linker.beam)
        .and_then(|constructor| {
            constructor
                .b_linkers
                .iter()
                .find(|b| b.reference == reference.b_linker)
        })
        .and_then(|b| b.v_linkers.iter().find(|v| v.reference == reference))
}

fn find_b_linker(
    system: &crate::native_stems_beam_vlinkers::NativeStemsBeamVLinkerSystem,
    reference: NativeStemsBeamBLinkerRef,
) -> Option<&NativeStemsBeamBLinker> {
    system
        .constructors
        .iter()
        .find(|constructor| constructor.source == reference.beam)
        .and_then(|constructor| {
            constructor
                .b_linkers
                .iter()
                .find(|b| b.reference == reference)
        })
}

fn find_stump_beam(
    system: &crate::native_stems_beam_stumps::NativeStemsBeamStumpSystem,
    source: NativeStemsBeamSource,
) -> Option<&NativeStemsBeamStumpBeam> {
    system
        .beams_by_abscissa
        .iter()
        .find(|beam| beam.source == source)
}

fn stump_for_b(
    context: &BuilderContext<'_>,
    reference: NativeStemsBeamBLinkerRef,
) -> Result<Option<NativeStemsBeamBuilderGlyphRef>, NativeStemsBeamBuilderError> {
    if let Some(b) = find_b_linker(context.v_system, reference) {
        return match &b.stump {
            Some(_) => Ok(Some(NativeStemsBeamBuilderGlyphRef::BeamStump {
                b_linker: reference,
            })),
            None => Ok(None),
        };
    }
    let arena = context
        .reach_system
        .final_beam_arenas
        .iter()
        .find(|arena| arena.beam == reference.beam)
        .ok_or(NativeStemsBeamBuilderError::MissingBLinker {
            system_id: context.system_id,
            reference,
        })?;
    let entry = arena
        .all_b_linkers
        .iter()
        .find(|entry| entry.reference == reference)
        .ok_or(NativeStemsBeamBuilderError::MissingBLinker {
            system_id: context.system_id,
            reference,
        })?;
    if entry.is_anchor {
        Ok(None)
    } else {
        Err(NativeStemsBeamBuilderError::MissingBLinker {
            system_id: context.system_id,
            reference,
        })
    }
}

fn stump_for_target(
    context: &BuilderContext<'_>,
    target: NativeStemsBeamBuilderTargetRef,
) -> Result<Option<NativeStemsBeamBuilderGlyphRef>, NativeStemsBeamBuilderError> {
    match target {
        NativeStemsBeamBuilderTargetRef::Beam(reference) => stump_for_b(context, reference),
        NativeStemsBeamBuilderTargetRef::Head(corner) => {
            let head = context
                .head_stump_system
                .heads_by_abscissa
                .iter()
                .find(|head| head.sig_ordinal == corner.sig_ordinal)
                .ok_or(NativeStemsBeamBuilderError::MissingHeadStump {
                    system_id: context.system_id,
                    corner,
                })?;
            let constructor_ordinal = corner_constructor_ordinal(context, corner)?;
            let stump = head
                .corners_in_constructor_order
                .iter()
                .find(|value| value.constructor_ordinal == constructor_ordinal)
                .ok_or(NativeStemsBeamBuilderError::MissingHeadStump {
                    system_id: context.system_id,
                    corner,
                })?;
            match stump.outcome {
                NativeStemsHeadStumpOutcome::Seed { .. }
                | NativeStemsHeadStumpOutcome::Built { .. } => {
                    Ok(Some(NativeStemsBeamBuilderGlyphRef::HeadStump { corner }))
                }
                NativeStemsHeadStumpOutcome::None => Ok(None),
            }
        }
    }
}

fn corner_constructor_ordinal(
    context: &BuilderContext<'_>,
    reference: NativeStemsBeamHeadCornerRef,
) -> Result<usize, NativeStemsBeamBuilderError> {
    context
        .corner_system
        .heads_in_sig_order
        .get(reference.sig_ordinal)
        .and_then(|head| {
            head.corners_in_constructor_order.iter().find(|corner| {
                corner.horizontal == reference.horizontal && corner.vertical == reference.vertical
            })
        })
        .map(|corner| corner.constructor_ordinal)
        .ok_or(NativeStemsBeamBuilderError::MissingHeadCorner {
            system_id: context.system_id,
            corner: reference,
        })
}

fn resolve_glyph(
    context: &BuilderContext<'_>,
    reference: NativeStemsBeamBuilderGlyphRef,
) -> Result<FixedGlyph, NativeStemsBeamBuilderError> {
    match reference {
        NativeStemsBeamBuilderGlyphRef::StemSeed { free_glyph_ordinal } => {
            let glyph = context
                .seed_system
                .free_glyphs
                .get(free_glyph_ordinal)
                .ok_or(NativeStemsBeamBuilderError::MissingSeed {
                    system_id: context.system_id,
                    free_glyph_ordinal,
                })?;
            glyph_from_seed(reference, glyph)
        }
        NativeStemsBeamBuilderGlyphRef::BeamStump { b_linker } => {
            let b = find_b_linker(context.v_system, b_linker).ok_or(
                NativeStemsBeamBuilderError::MissingBLinker {
                    system_id: context.system_id,
                    reference: b_linker,
                },
            )?;
            let stump = b
                .stump
                .as_ref()
                .ok_or(NativeStemsBeamBuilderError::MissingStumpGlyph {
                    system_id: context.system_id,
                })?;
            let beam = find_stump_beam(context.stump_system, b_linker.beam).ok_or(
                NativeStemsBeamBuilderError::MissingBeam {
                    system_id: context.system_id,
                    source: b_linker.beam,
                },
            )?;
            let glyph = beam_stump_glyph(context, beam, stump)?;
            glyph_from_beam(reference, &glyph)
        }
        NativeStemsBeamBuilderGlyphRef::HeadStump { corner } => {
            let head = context
                .head_stump_system
                .heads_by_abscissa
                .iter()
                .find(|head| head.sig_ordinal == corner.sig_ordinal)
                .ok_or(NativeStemsBeamBuilderError::MissingHeadStump {
                    system_id: context.system_id,
                    corner,
                })?;
            let constructor_ordinal = corner_constructor_ordinal(context, corner)?;
            let stump = head
                .corners_in_constructor_order
                .iter()
                .find(|value| value.constructor_ordinal == constructor_ordinal)
                .ok_or(NativeStemsBeamBuilderError::MissingHeadStump {
                    system_id: context.system_id,
                    corner,
                })?;
            match stump.outcome {
                NativeStemsHeadStumpOutcome::Seed { free_glyph_ordinal } => {
                    resolve_glyph(
                        context,
                        NativeStemsBeamBuilderGlyphRef::StemSeed { free_glyph_ordinal },
                    )
                    .map(|mut glyph| {
                        // Content/identity comes from the registered seed,
                        // while the output occurrence remains a C-stump.
                        glyph.reference = reference;
                        glyph
                    })
                }
                NativeStemsHeadStumpOutcome::Built { .. } => {
                    let build = stump.build.as_ref().ok_or(
                        NativeStemsBeamBuilderError::MissingHeadStump {
                            system_id: context.system_id,
                            corner,
                        },
                    )?;
                    glyph_from_head_build(reference, build)
                }
                NativeStemsHeadStumpOutcome::None => {
                    Err(NativeStemsBeamBuilderError::MissingStumpGlyph {
                        system_id: context.system_id,
                    })
                }
            }
        }
        NativeStemsBeamBuilderGlyphRef::Chunk { .. } => {
            Err(NativeStemsBeamBuilderError::MissingStumpGlyph {
                system_id: context.system_id,
            })
        }
    }
}

fn resolve_registered_glyph(
    context: &BuilderContext<'_>,
    registry: &GlyphRegistry,
    reference: NativeStemsBeamBuilderGlyphRef,
) -> Result<FixedGlyph, NativeStemsBeamBuilderError> {
    let mut glyph = resolve_glyph(context, reference)?;
    registry.bind_existing(&mut glyph, context.system_id)?;
    Ok(glyph)
}

fn beam_stump_glyph(
    context: &BuilderContext<'_>,
    beam: &NativeStemsBeamStumpBeam,
    stump: &NativeStemsBeamStumpRef,
) -> Result<NativeStemsBeamGlyph, NativeStemsBeamBuilderError> {
    match stump {
        NativeStemsBeamStumpRef::Seed {
            free_glyph_ordinal, ..
        } => {
            let glyph = context
                .seed_system
                .free_glyphs
                .get(*free_glyph_ordinal)
                .ok_or(NativeStemsBeamBuilderError::MissingSeed {
                    system_id: context.system_id,
                    free_glyph_ordinal: *free_glyph_ordinal,
                })?;
            Ok(NativeStemsBeamGlyph {
                bounds: glyph.bounds,
                weight: glyph.weight,
                run_table: glyph.run_table.clone(),
            })
        }
        NativeStemsBeamStumpRef::Built { .. } => beam
            .sides
            .iter()
            .filter_map(|side| side.build.as_ref())
            .find(|build| {
                build.candidate.is_some()
                    && build.canonical_glyph_index == stump_canonical_index(stump)
            })
            .and_then(|build| build.candidate.clone())
            .ok_or(NativeStemsBeamBuilderError::MissingStumpGlyph {
                system_id: context.system_id,
            }),
    }
}

fn stump_canonical_index(stump: &NativeStemsBeamStumpRef) -> Option<usize> {
    match stump {
        NativeStemsBeamStumpRef::Seed {
            canonical_glyph_index,
            ..
        }
        | NativeStemsBeamStumpRef::Built {
            canonical_glyph_index,
        } => Some(*canonical_glyph_index),
    }
}

fn target_item(
    context: &BuilderContext<'_>,
    state: &TargetState,
    y_range: JavaRectangle,
) -> Result<NativeStemsBeamBuilderItem, NativeStemsBeamBuilderError> {
    match state.target {
        NativeStemsBeamBuilderTargetRef::Beam(reference) => {
            let line = if let Some(glyph) = &state.stump {
                glyph.line
            } else {
                beam_linker_line(context, reference)?
            };
            let contribution = state
                .stump
                .as_ref()
                .map_or(0, |glyph| glyph.bounds.height as i32);
            Ok(NativeStemsBeamBuilderItem {
                kind: NativeStemsBeamBuilderItemKind::BeamLinker,
                glyph: state.stump.as_ref().map(|glyph| glyph.reference),
                target: Some(state.target),
                reference_point: None,
                head_bounds: None,
                line: line_from_segment(line),
                contribution,
            })
        }
        NativeStemsBeamBuilderTargetRef::Head(corner) => {
            let corner_geometry = head_corner(context, corner)?;
            let line = if let Some(glyph) = &state.stump {
                glyph.line
            } else {
                let reference = corner_geometry.reference;
                Segment {
                    x1: reference.x,
                    y1: reference.y,
                    x2: reference.x,
                    y2: reference.y,
                }
            };
            let contribution = state
                .stump
                .as_ref()
                .map_or(0, |glyph| contribution(y_range, glyph.bounds));
            let head_bounds = context
                .corner_system
                .heads_in_sig_order
                .get(corner.sig_ordinal)
                .map(|head| head.bounds)
                .ok_or(NativeStemsBeamBuilderError::MissingHeadCorner {
                    system_id: context.system_id,
                    corner,
                })?;
            Ok(NativeStemsBeamBuilderItem {
                kind: NativeStemsBeamBuilderItemKind::HeadHalfLinker,
                glyph: state.stump.as_ref().map(|glyph| glyph.reference),
                target: Some(state.target),
                reference_point: Some(corner_geometry.reference),
                head_bounds: Some(head_bounds),
                line: line_from_segment(line),
                contribution,
            })
        }
    }
}

fn beam_linker_line(
    context: &BuilderContext<'_>,
    reference: NativeStemsBeamBLinkerRef,
) -> Result<Segment, NativeStemsBeamBuilderError> {
    let (point, source) = if let Some(b) = find_b_linker(context.v_system, reference) {
        (b.reference_point, reference.beam)
    } else {
        let entry = arena_entry(context, reference)?;
        (entry.reference_point, reference.beam)
    };
    let beam = find_stump_beam(context.stump_system, source).ok_or(
        NativeStemsBeamBuilderError::MissingBeam {
            system_id: context.system_id,
            source,
        },
    )?;
    let half = beam.height / 2.0;
    Ok(Segment {
        x1: point.x,
        y1: point.y - half,
        x2: point.x,
        y2: point.y + half,
    })
}

fn arena_entry<'a>(
    context: &'a BuilderContext<'_>,
    reference: NativeStemsBeamBLinkerRef,
) -> Result<&'a NativeStemsBeamArenaEntry, NativeStemsBeamBuilderError> {
    context
        .reach_system
        .final_beam_arenas
        .iter()
        .find(|arena| arena.beam == reference.beam)
        .and_then(|arena| {
            arena
                .all_b_linkers
                .iter()
                .find(|entry| entry.reference == reference)
        })
        .ok_or(NativeStemsBeamBuilderError::MissingBLinker {
            system_id: context.system_id,
            reference,
        })
}

fn head_corner<'a>(
    context: &'a BuilderContext<'_>,
    reference: NativeStemsBeamHeadCornerRef,
) -> Result<&'a NativeStemsHeadCorner, NativeStemsBeamBuilderError> {
    context
        .corner_system
        .heads_in_sig_order
        .get(reference.sig_ordinal)
        .and_then(|head| {
            head.corners_in_constructor_order.iter().find(|corner| {
                corner.horizontal == reference.horizontal && corner.vertical == reference.vertical
            })
        })
        .ok_or(NativeStemsBeamBuilderError::MissingHeadCorner {
            system_id: context.system_id,
            corner: reference,
        })
}

fn glyph_from_seed(
    reference: NativeStemsBeamBuilderGlyphRef,
    glyph: &NativeStemSeedGlyph,
) -> Result<FixedGlyph, NativeStemsBeamBuilderError> {
    glyph_from_parts(
        reference,
        glyph.bounds,
        glyph.weight,
        glyph.run_table.clone(),
    )
}

fn glyph_from_beam(
    reference: NativeStemsBeamBuilderGlyphRef,
    glyph: &NativeStemsBeamGlyph,
) -> Result<FixedGlyph, NativeStemsBeamBuilderError> {
    glyph_from_parts(
        reference,
        glyph.bounds,
        glyph.weight,
        glyph.run_table.clone(),
    )
}

fn glyph_from_head_build(
    reference: NativeStemsBeamBuilderGlyphRef,
    build: &NativeStemsHeadStumpBuild,
) -> Result<FixedGlyph, NativeStemsBeamBuilderError> {
    let glyph = build
        .candidate
        .as_ref()
        .ok_or(NativeStemsBeamBuilderError::MissingStumpGlyph { system_id: 0 })?;
    glyph_from_parts(
        reference,
        glyph.bounds,
        glyph.weight,
        glyph.run_table.clone(),
    )
}

fn glyph_from_sections(
    reference: NativeStemsBeamBuilderGlyphRef,
    sections: &[Section],
    geometry: audiveris_image::stick_factory::StraightStickGeometry,
) -> Result<FixedGlyph, NativeStemsBeamBuilderError> {
    let bounds = geometry.bounds;
    let mut pixels = vec![
        BACKGROUND;
        bounds
            .width
            .checked_mul(bounds.height)
            .ok_or(RunTableError::InvalidDimensions)?
    ];
    for section in sections {
        paint_section(&mut pixels, bounds, section)?;
    }
    let orientation = if bounds.width > bounds.height {
        Orientation::Horizontal
    } else {
        Orientation::Vertical
    };
    let run_table = RunTable::from_pixels(orientation, bounds.width, bounds.height, &pixels)?;
    glyph_from_parts(
        reference,
        bounds,
        pixels.iter().filter(|&&pixel| pixel == FOREGROUND).count(),
        run_table,
    )
}

fn glyph_from_parts(
    reference: NativeStemsBeamBuilderGlyphRef,
    bounds: Bounds,
    weight: usize,
    run_table: RunTable,
) -> Result<FixedGlyph, NativeStemsBeamBuilderError> {
    let left = i32::try_from(bounds.x)
        .map_err(|_| NativeStemsBeamBuilderError::Geometry { system_id: 0 })?;
    let top = i32::try_from(bounds.y)
        .map_err(|_| NativeStemsBeamBuilderError::Geometry { system_id: 0 })?;
    let line = run_table_center_line(&run_table, left, top)
        .ok_or(NativeStemsBeamBuilderError::Geometry { system_id: 0 })?;
    let identity = GlyphKey {
        bounds,
        run_table: run_table.clone(),
    };
    Ok(FixedGlyph {
        reference,
        identity,
        modeled_canonical_ordinal: None,
        bounds,
        weight,
        run_table,
        line,
    })
}

fn scan_vertical_sections(
    context: &BuilderContext<'_>,
    inspection: &NativeStemsBeamVInspection,
    y_direction: i32,
    start: Option<&FixedGlyph>,
    last_head_y: Option<f64>,
) -> Result<(Vec<NativeStemsBeamBuilderSectionScan>, Vec<Section>), NativeStemsBeamBuilderError> {
    let mut scans = Vec::new();
    let mut selected = Vec::new();
    for &source_ordinal in context.vertical_section_source_ordinals {
        let section = context
            .grid
            .peak_graph
            .vertical_sections
            .get(source_ordinal)
            .ok_or(NativeStemsBeamBuilderError::Geometry {
                system_id: context.system_id,
            })?;
        let bounds = section.bounds();
        let java_bounds = java_bounds(bounds)?;
        let intersects_lookup =
            quad_intersects_bounds(inspection.lookup_quadrilateral, java_bounds);
        let mut scan = NativeStemsBeamBuilderSectionScan {
            source_ordinal,
            bounds,
            intersects_lookup,
            width_accepted: None,
            stump_overlap_accepted: None,
            before_last_head: None,
            line_distance: None,
            distance_accepted: None,
            accepted: false,
            accepted_sorted_ordinal: None,
        };
        if !intersects_lookup {
            scans.push(scan);
            continue;
        }
        let width_accepted = i32::try_from(bounds.width)
            .is_ok_and(|width| width <= context.params.max_stem_thickness);
        scan.width_accepted = Some(width_accepted);
        if !width_accepted {
            scans.push(scan);
            continue;
        }
        if let Some(start) = start {
            if y_overlap(bounds, start.bounds) > 0 {
                let accepted = bounds.height >= start.bounds.height;
                scan.stump_overlap_accepted = Some(accepted);
                if !accepted {
                    scans.push(scan);
                    continue;
                }
            }
        }
        let center = section.centroid_2d();
        if let Some(last_head_y) = last_head_y {
            let accepted = y_direction * java_double_compare(center.1, last_head_y) < 0;
            scan.before_last_head = Some(accepted);
            if !accepted {
                scans.push(scan);
                continue;
            }
        }
        let distance = line_pt_distance(inspection.theoretical_line, center);
        let accepted = distance <= context.params.max_line_section_dx;
        scan.line_distance = Some(distance);
        scan.distance_accepted = Some(accepted);
        if accepted {
            selected.push(section.clone());
        }
        scan.accepted = accepted;
        scans.push(scan);
    }
    selected.sort_by(section_full_position_cmp);
    for scan in &mut scans {
        if scan.accepted {
            scan.accepted_sorted_ordinal = selected.iter().position(|section| {
                section.bounds() == scan.bounds
                    && section.id()
                        == context.grid.peak_graph.vertical_sections[scan.source_ordinal].id()
            });
        }
    }
    Ok((scans, selected))
}

fn scan_horizontal_sections(
    context: &BuilderContext<'_>,
    inspection: &NativeStemsBeamVInspection,
    y_direction: i32,
    last_head_y: Option<f64>,
) -> Result<(Vec<NativeStemsBeamBuilderSectionScan>, Vec<Section>), NativeStemsBeamBuilderError> {
    let mut scans = Vec::new();
    let mut selected = Vec::new();
    for &source_ordinal in context.horizontal_section_source_ordinals {
        let section = context
            .grid
            .peak_graph
            .horizontal_sections
            .get(source_ordinal)
            .ok_or(NativeStemsBeamBuilderError::Geometry {
                system_id: context.system_id,
            })?;
        let bounds = section.bounds();
        let java_bounds = java_bounds(bounds)?;
        let intersects_lookup =
            quad_intersects_bounds(inspection.lookup_quadrilateral, java_bounds);
        let mut scan = NativeStemsBeamBuilderSectionScan {
            source_ordinal,
            bounds,
            intersects_lookup,
            width_accepted: None,
            stump_overlap_accepted: None,
            before_last_head: None,
            line_distance: None,
            distance_accepted: None,
            accepted: false,
            accepted_sorted_ordinal: None,
        };
        if !intersects_lookup {
            scans.push(scan);
            continue;
        }
        let width_accepted = bounds.width <= 1;
        scan.width_accepted = Some(width_accepted);
        if !width_accepted {
            scans.push(scan);
            continue;
        }
        let center = section.centroid_2d();
        if let Some(last_head_y) = last_head_y {
            let accepted = y_direction * java_double_compare(center.1, last_head_y) < 0;
            scan.before_last_head = Some(accepted);
            if !accepted {
                scans.push(scan);
                continue;
            }
        }
        scan.accepted = true;
        selected.push(section.clone());
        scans.push(scan);
    }
    selected.sort_by(section_full_position_cmp);
    for scan in &mut scans {
        if scan.accepted {
            scan.accepted_sorted_ordinal = selected.iter().position(|section| {
                section.bounds() == scan.bounds
                    && section.id()
                        == context.grid.peak_graph.horizontal_sections[scan.source_ordinal].id()
            });
        }
    }
    Ok((scans, selected))
}

fn filter_unaligned(
    glyphs: &mut Vec<(NativeStemsBeamBuilderGlyphRef, FixedGlyph)>,
    start: Option<(NativeStemsBeamBuilderGlyphRef, FixedGlyph)>,
    y_direction: i32,
    skew: &HeadlessSkew,
    params: BuilderParams,
    subject: NativeStemsBeamBuilderAlignmentSubject,
) -> (NativeStemsBeamBuilderAlignmentPass, Vec<FixedGlyph>) {
    let mut ordered = glyphs.clone();
    stable_sort_glyphs(&mut ordered, y_direction);
    if let Some((reference, stump)) = start {
        if let Some(index) = ordered
            .iter()
            .position(|(_, glyph)| same_content(glyph, &stump))
        {
            ordered.remove(index);
        }
        ordered.insert(0, (reference, stump));
    }
    let sorted_before = ordered.iter().map(|(reference, _)| *reference).collect();
    let mut comparisons = Vec::new();
    let mut removed = Vec::new();
    let mut removed_content = Vec::new();
    let mut index = 0_usize;
    while index + 1 < ordered.len() {
        let (first_ref, first) = ordered[index].clone();
        let (second_ref, second) = ordered[index + 1].clone();
        let first_centroid = glyph_centroid(&first);
        let second_centroid = glyph_centroid(&second);
        let first_deskewed = deskew(skew, first_centroid);
        let second_deskewed = deskew(skew, second_centroid);
        let dy = (second_deskewed.1 - first_deskewed.1).abs();
        let dx = (second_deskewed.0 - first_deskewed.0).abs();
        let bypass = dy > params.max_stem_alignment_dy;
        let aligned = bypass || dx <= params.max_stem_alignment_dx;
        let removed_ref = if aligned {
            None
        } else if first.bounds.height < second.bounds.height {
            Some(first_ref)
        } else {
            Some(second_ref)
        };
        let (removed_list_occurrence, remove_index) = if let Some(removed_ref) = removed_ref {
            let alien = if removed_ref == first_ref {
                &first
            } else {
                &second
            };
            let remove_index = ordered
                .iter()
                .position(|(_, glyph)| same_content(glyph, alien))
                .expect("selected aligned glyph remains in copied list");
            (Some(ordered[remove_index].0), Some(remove_index))
        } else {
            (None, None)
        };
        comparisons.push(NativeStemsBeamBuilderAlignmentCheck {
            first: first_ref,
            second: second_ref,
            first_centroid,
            second_centroid,
            first_deskewed,
            second_deskewed,
            dx,
            dy,
            dy_bypasses_dx: bypass,
            aligned,
            removed: removed_ref,
            removed_list_occurrence,
            equal_height_removed_second: !aligned && first.bounds.height == second.bounds.height,
        });
        if let Some(removed_ref) = removed_ref {
            removed.push(removed_ref);
            removed_content.push(
                ordered
                    .remove(remove_index.expect("aligned removal index"))
                    .1,
            );
            // Java decrements `i` in the body, then the `for` loop update
            // increments it again. The net index is unchanged after removal.
        } else {
            index += 1;
        }
    }
    glyphs.retain(|(_, glyph)| {
        !removed_content
            .iter()
            .any(|removed| same_content(glyph, removed))
    });
    (
        NativeStemsBeamBuilderAlignmentPass {
            subject,
            sorted_before,
            comparisons,
            removed,
            retained: glyphs.iter().map(|(reference, _)| *reference).collect(),
        },
        removed_content,
    )
}

fn stable_sort_glyphs(
    glyphs: &mut [(NativeStemsBeamBuilderGlyphRef, FixedGlyph)],
    y_direction: i32,
) {
    glyphs.sort_by(|(_, left), (_, right)| {
        if y_direction > 0 {
            left.bounds.y.cmp(&right.bounds.y)
        } else {
            (right.bounds.y + right.bounds.height).cmp(&(left.bounds.y + left.bounds.height))
        }
    });
}

fn stable_sort_items(
    items: &mut [NativeStemsBeamBuilderItem],
    y_direction: i32,
    system_id: usize,
) -> Result<(), NativeStemsBeamBuilderError> {
    // The comparator below is pair-dependent and has observed cycles, so the
    // exact OpenJDK run discovery, binary insertion, merge, and gallop control
    // flow is part of the visible result.
    crate::jdk25_timsort::sort_by(items, |left, right| item_cmp(left, right, y_direction))
        .then_some(())
        .ok_or(NativeStemsBeamBuilderError::UnsupportedJdkTimSortLength {
            system_id,
            length: items.len(),
        })
}

fn sort_permutation(
    before: &[NativeStemsBeamBuilderItem],
    after: &[NativeStemsBeamBuilderItem],
    y_direction: i32,
) -> Vec<NativeStemsBeamBuilderSortEntry> {
    let mut used = vec![false; before.len()];
    after
        .iter()
        .enumerate()
        .map(|(after_ordinal, item)| {
            let before_ordinal = before
                .iter()
                .enumerate()
                .find_map(|(before_ordinal, candidate)| {
                    (!used[before_ordinal] && candidate == item).then_some(before_ordinal)
                })
                .unwrap_or(after_ordinal);
            if before_ordinal < used.len() {
                used[before_ordinal] = true;
            }
            NativeStemsBeamBuilderSortEntry {
                before_ordinal,
                after_ordinal,
                item: item.clone(),
                tied_with_previous: after_ordinal > 0
                    && item_cmp(&after[after_ordinal - 1], item, y_direction) == Ordering::Equal,
            }
        })
        .collect()
}

fn item_cmp(
    left: &NativeStemsBeamBuilderItem,
    right: &NativeStemsBeamBuilderItem,
    y_direction: i32,
) -> Ordering {
    let half = |kind| {
        matches!(
            kind,
            NativeStemsBeamBuilderItemKind::StartHalfLinker
                | NativeStemsBeamBuilderItemKind::HeadHalfLinker
        )
    };
    if half(left.kind) && half(right.kind) {
        let left = left
            .reference_point
            .map_or(left.line.start.y, |point| point.y);
        let right = right
            .reference_point
            .map_or(right.line.start.y, |point| point.y);
        return (y_direction * java_double_compare(left, right)).cmp(&0);
    }
    if y_direction > 0 {
        left.line.start.y.total_cmp(&right.line.start.y)
    } else {
        right.line.stop.y.total_cmp(&left.line.stop.y)
    }
}

fn start_item(
    reference: Option<NativeStemsBeamBuilderGlyphRef>,
    glyph: Option<&FixedGlyph>,
    reference_point: NativeStemPoint,
    y_range: JavaRectangle,
) -> Result<NativeStemsBeamBuilderItem, NativeStemsBeamBuilderError> {
    let line = glyph.map_or(
        NativeStemLine {
            start: reference_point,
            stop: reference_point,
        },
        |glyph| line_from_segment(glyph.line),
    );
    let contribution = glyph.map_or(0, |glyph| contribution(y_range, glyph.bounds));
    Ok(NativeStemsBeamBuilderItem {
        kind: NativeStemsBeamBuilderItemKind::StartHalfLinker,
        glyph: reference,
        target: None,
        reference_point: Some(reference_point),
        head_bounds: None,
        line,
        contribution,
    })
}

fn glyph_item(
    kind: NativeStemsBeamBuilderItemKind,
    reference: NativeStemsBeamBuilderGlyphRef,
    glyph: &FixedGlyph,
    contribution: i32,
) -> Result<NativeStemsBeamBuilderItem, NativeStemsBeamBuilderError> {
    Ok(NativeStemsBeamBuilderItem {
        kind,
        glyph: Some(reference),
        target: None,
        reference_point: None,
        head_bounds: None,
        line: line_from_segment(glyph.line),
        contribution,
    })
}

fn insert_gaps(
    items: &mut Vec<NativeStemsBeamBuilderItem>,
    y_direction: i32,
    max_gap: i32,
) -> Vec<NativeStemsBeamBuilderGapEvent> {
    let mut events = Vec::new();
    let mut last: Option<NativeStemPoint> = None;
    let mut index = 0_usize;
    while index < items.len() {
        let item = &items[index];
        let start = if y_direction > 0 {
            item.line.start
        } else {
            item.line.stop
        };
        let stop = if y_direction > 0 {
            item.line.stop
        } else {
            item.line.start
        };
        if let Some(last_point) = last {
            let gap = f64::from(y_direction) * (start.y - last_point.y);
            if gap > f64::from(max_gap) {
                events.push(NativeStemsBeamBuilderGapEvent {
                    item_ordinal_before_insert: index,
                    previous_stop: last_point,
                    next_start: start,
                    gap,
                    max_gap,
                    action: NativeStemsBeamBuilderGapAction::Truncated,
                });
                items.truncate(index);
                break;
            }
            if gap > 0.01 {
                let line = if y_direction > 0 {
                    NativeStemLine {
                        start: last_point,
                        stop: start,
                    }
                } else {
                    NativeStemLine {
                        start,
                        stop: last_point,
                    }
                };
                let contribution = line_bounds(line).height;
                events.push(NativeStemsBeamBuilderGapEvent {
                    item_ordinal_before_insert: index,
                    previous_stop: last_point,
                    next_start: start,
                    gap,
                    max_gap,
                    action: NativeStemsBeamBuilderGapAction::Inserted,
                });
                items.insert(
                    index,
                    NativeStemsBeamBuilderItem {
                        kind: NativeStemsBeamBuilderItemKind::Gap,
                        glyph: None,
                        target: None,
                        reference_point: None,
                        head_bounds: None,
                        line,
                        contribution,
                    },
                );
                index += 1;
            } else {
                events.push(NativeStemsBeamBuilderGapEvent {
                    item_ordinal_before_insert: index,
                    previous_stop: last_point,
                    next_start: start,
                    gap,
                    max_gap,
                    action: NativeStemsBeamBuilderGapAction::None,
                });
            }
        }
        if last.is_none_or(|last_point: NativeStemPoint| {
            f64::from(y_direction) * (stop.y - last_point.y) > 0.01
        }) {
            last = Some(stop);
        }
        index += 1;
    }
    events
}

fn retrieve_lengths(
    items: &[NativeStemsBeamBuilderItem],
    y_direction: i32,
    theoretical_line: NativeStemLine,
    gap_map: BTreeMap<i32, i32>,
    max_profile: i32,
) -> Result<BTreeMap<i32, i32>, NativeStemsBeamBuilderError> {
    let mut lengths = BTreeMap::new();
    let max_gap = *gap_map
        .get(&max_profile)
        .ok_or(NativeStemsBeamBuilderError::Geometry { system_id: 0 })?;
    for (index, item) in items.iter().enumerate() {
        if item.kind != NativeStemsBeamBuilderItemKind::Gap {
            continue;
        }
        for (&profile, &threshold) in &gap_map {
            if item.contribution > threshold {
                lengths.entry(profile).or_insert_with(|| {
                    length_at(
                        items,
                        index.saturating_sub(1),
                        y_direction,
                        theoretical_line,
                    )
                });
            } else {
                break;
            }
        }
        if item.contribution > max_gap {
            lengths.entry(max_profile).or_insert_with(|| {
                length_at(
                    items,
                    index.saturating_sub(1),
                    y_direction,
                    theoretical_line,
                )
            });
            return Ok(lengths);
        }
    }
    let last = items.len().saturating_sub(1);
    for &profile in gap_map.keys() {
        lengths
            .entry(profile)
            .or_insert_with(|| length_at(items, last, y_direction, theoretical_line));
    }
    Ok(lengths)
}

fn length_at(
    items: &[NativeStemsBeamBuilderItem],
    last: usize,
    y_direction: i32,
    theoretical_line: NativeStemLine,
) -> i32 {
    let mut bounds = None;
    for item in items.iter().take(last.saturating_add(1)) {
        if item.kind == NativeStemsBeamBuilderItemKind::Gap {
            continue;
        }
        let line = line_bounds(item.line);
        bounds = Some(bounds.map_or(line, |prior| rectangle_union(prior, line)));
        if let Some(head_bounds) = item.head_bounds {
            bounds = Some(bounds.map_or(head_bounds, |prior| rectangle_union(prior, head_bounds)));
        }
    }
    let Some(bounds) = bounds else {
        return 0;
    };
    let start_y = theoretical_line.start.y as i32;
    if y_direction > 0 {
        bounds.y + bounds.height - start_y
    } else {
        start_y - bounds.y
    }
}

fn line_bounds(line: NativeStemLine) -> JavaRectangle {
    let min_x = line.start.x.min(line.stop.x).floor() as i32;
    let min_y = line.start.y.min(line.stop.y).floor() as i32;
    let max_x = line.start.x.max(line.stop.x).ceil() as i32;
    let max_y = line.start.y.max(line.stop.y).ceil() as i32;
    JavaRectangle::new(min_x, min_y, max_x - min_x, max_y - min_y)
}

fn stem_builder_y_direction(theoretical_line: NativeStemLine) -> i32 {
    // StemBuilder.java recomputes this in its constructor instead of using
    // the originating VLinker's side/direction. A horizontal or NaN line
    // follows Java's false branch and therefore points upward.
    if theoretical_line.stop.y > theoretical_line.start.y {
        1
    } else {
        -1
    }
}

fn line_from_segment(line: Segment) -> NativeStemLine {
    NativeStemLine {
        start: NativeStemPoint {
            x: line.x1,
            y: line.y1,
        },
        stop: NativeStemPoint {
            x: line.x2,
            y: line.y2,
        },
    }
}

fn item_reference_y(item: &NativeStemsBeamBuilderItem) -> f64 {
    item.line.start.y
}

fn contribution(range: JavaRectangle, bounds: Bounds) -> i32 {
    y_overlap(
        range,
        java_bounds(bounds).unwrap_or(JavaRectangle::new(0, 0, 0, 0)),
    )
    .max(0)
}

fn y_overlap<A: IntoJavaBounds, B: IntoJavaBounds>(left: A, right: B) -> i32 {
    let left = left.into_java_bounds();
    let right = right.into_java_bounds();
    let top = left.y.max(right.y);
    let bottom = left
        .y
        .wrapping_add(left.height)
        .min(right.y.wrapping_add(right.height));
    bottom.wrapping_sub(top)
}

trait IntoJavaBounds {
    fn into_java_bounds(self) -> JavaRectangle;
}
impl IntoJavaBounds for JavaRectangle {
    fn into_java_bounds(self) -> JavaRectangle {
        self
    }
}
impl IntoJavaBounds for Bounds {
    fn into_java_bounds(self) -> JavaRectangle {
        java_bounds(self).unwrap_or(JavaRectangle::new(0, 0, 0, 0))
    }
}

fn rectangle_union(left: JavaRectangle, right: JavaRectangle) -> JavaRectangle {
    let x = left.x.min(right.x);
    let y = left.y.min(right.y);
    let max_x = left
        .x
        .wrapping_add(left.width)
        .max(right.x.wrapping_add(right.width));
    let max_y = left
        .y
        .wrapping_add(left.height)
        .max(right.y.wrapping_add(right.height));
    JavaRectangle::new(x, y, max_x.wrapping_sub(x), max_y.wrapping_sub(y))
}

fn java_bounds(bounds: Bounds) -> Result<JavaRectangle, NativeStemsBeamBuilderError> {
    Ok(JavaRectangle::new(
        i32::try_from(bounds.x)
            .map_err(|_| NativeStemsBeamBuilderError::Geometry { system_id: 0 })?,
        i32::try_from(bounds.y)
            .map_err(|_| NativeStemsBeamBuilderError::Geometry { system_id: 0 })?,
        i32::try_from(bounds.width)
            .map_err(|_| NativeStemsBeamBuilderError::Geometry { system_id: 0 })?,
        i32::try_from(bounds.height)
            .map_err(|_| NativeStemsBeamBuilderError::Geometry { system_id: 0 })?,
    ))
}

fn dispatch_section_ordinals(
    sections: &[Section],
    area: &audiveris_image::system_population::PopulationSystemArea,
) -> Vec<usize> {
    sections
        .iter()
        .enumerate()
        .filter_map(|(ordinal, section)| {
            let (x, y) = section.centroid();
            contains_section_centroid(area, x as f64, y as f64).then_some(ordinal)
        })
        .collect()
}

fn source_section_ref(
    context: &BuilderContext<'_>,
    section: &Section,
) -> Result<NativeStemsBeamBuilderSectionRef, NativeStemsBeamBuilderError> {
    let (orientation, ordinals, sections) = match section.orientation() {
        Orientation::Vertical => (
            Orientation::Vertical,
            context.vertical_section_source_ordinals,
            &context.grid.peak_graph.vertical_sections,
        ),
        Orientation::Horizontal => (
            Orientation::Horizontal,
            context.horizontal_section_source_ordinals,
            &context.grid.peak_graph.horizontal_sections,
        ),
    };
    let source_ordinal = ordinals
        .iter()
        .copied()
        .find(|&ordinal| sections.get(ordinal) == Some(section))
        .ok_or(NativeStemsBeamBuilderError::Geometry {
            system_id: context.system_id,
        })?;
    Ok(NativeStemsBeamBuilderSectionRef {
        orientation,
        source_ordinal,
    })
}

fn section_full_position_cmp(left: &Section, right: &Section) -> Ordering {
    left.first_pos()
        .cmp(&right.first_pos())
        .then_with(|| left.start_coord().cmp(&right.start_coord()))
}

fn paint_section(
    pixels: &mut [u8],
    bounds: Bounds,
    section: &Section,
) -> Result<(), NativeStemsBeamBuilderError> {
    for (offset, run) in section.runs().iter().enumerate() {
        let position = section.first_pos() + offset;
        for coordinate in run.start..=run.stop() {
            let (x, y) = match section.orientation() {
                Orientation::Horizontal => (coordinate, position),
                Orientation::Vertical => (position, coordinate),
            };
            let x = x.checked_sub(bounds.x).ok_or(RunTableError::OutOfBounds)?;
            let y = y.checked_sub(bounds.y).ok_or(RunTableError::OutOfBounds)?;
            let index = y
                .checked_mul(bounds.width)
                .and_then(|value| value.checked_add(x))
                .filter(|&index| index < pixels.len())
                .ok_or(RunTableError::OutOfBounds)?;
            pixels[index] = FOREGROUND;
        }
    }
    Ok(())
}

fn glyph_centroid(glyph: &FixedGlyph) -> (f64, f64) {
    let mut count = 0_usize;
    let mut x_total = 0_f64;
    let mut y_total = 0_f64;
    // Java's `Glyph.getCentroid()` accumulates its materialized point list in
    // reverse order. The order affects the raw floating-point sum even though
    // the mathematical centroid is unchanged.
    for sequence in (0..glyph.run_table.sequence_count()).rev() {
        for run in glyph
            .run_table
            .sequence(sequence)
            .unwrap_or_default()
            .iter()
            .rev()
        {
            for coordinate in (run.start..=run.stop()).rev() {
                match glyph.run_table.orientation() {
                    Orientation::Horizontal => {
                        x_total += glyph.bounds.x as f64 + coordinate as f64;
                        y_total += glyph.bounds.y as f64 + sequence as f64;
                    }
                    Orientation::Vertical => {
                        x_total += glyph.bounds.x as f64 + sequence as f64;
                        y_total += glyph.bounds.y as f64 + coordinate as f64;
                    }
                }
                count += 1;
            }
        }
    }
    (x_total / count as f64, y_total / count as f64)
}

fn deskew(skew: &HeadlessSkew, point: (f64, f64)) -> (f64, f64) {
    let point = skew.deskewed(audiveris_image::staff_peak::PeakPoint::new(
        point.0, point.1,
    ));
    (point.x, point.y)
}

fn integer_center(bounds: Bounds) -> (i32, i32) {
    (
        (bounds.x + bounds.width / 2) as i32,
        (bounds.y + bounds.height / 2) as i32,
    )
}

fn line_pt_distance(line: NativeStemLine, point: (f64, f64)) -> f64 {
    let x2 = line.stop.x - line.start.x;
    let y2 = line.stop.y - line.start.y;
    let px = point.0 - line.start.x;
    let py = point.1 - line.start.y;
    let product = (px * x2) + (py * y2);
    let projection_sq = product * product / ((x2 * x2) + (y2 * y2));
    let mut length_sq = (px * px) + (py * py) - projection_sq;
    if length_sq < 0.0 {
        length_sq = 0.0;
    }
    length_sq.sqrt()
}

fn quad_intersects_bounds(quad: [NativeStemPoint; 4], bounds: JavaRectangle) -> bool {
    convex_quad_intersects_rectangle(quad, bounds)
}

fn java_rint(value: f64) -> i32 {
    value.round_ties_even() as i32
}
pub(crate) fn java_double_compare(left: f64, right: f64) -> i32 {
    if left < right {
        -1
    } else if left > right {
        1
    } else {
        // `Double.compare` falls back to signed `doubleToLongBits`, which
        // distinguishes -0.0 from +0.0 and canonicalizes every NaN payload.
        let bits = |value: f64| {
            if value.is_nan() {
                0x7ff8_0000_0000_0000_u64
            } else {
                value.to_bits()
            }
        };
        match (bits(left) as i64).cmp(&(bits(right) as i64)) {
            Ordering::Less => -1,
            Ordering::Equal => 0,
            Ordering::Greater => 1,
        }
    }
}
fn same_content(left: &FixedGlyph, right: &FixedGlyph) -> bool {
    left.bounds == right.bounds && left.run_table == right.run_table
}
fn same_identity(left: &FixedGlyph, right: &FixedGlyph) -> bool {
    left.modeled_canonical_ordinal == right.modeled_canonical_ordinal
        && left.modeled_canonical_ordinal.is_some()
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GlyphKey {
    pub(crate) bounds: Bounds,
    pub(crate) run_table: RunTable,
}

#[derive(Clone)]
struct RegisteredGlyph {
    key: GlyphKey,
    modeled_canonical_ordinal: usize,
}

/// One exact canonical glyph in the final native registry replay.
///
/// The ordinal is native registry identity, not a Java persistent ID.  It is
/// stable because the replay uses Java's registration order and equality.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsModeledCanonicalGlyph {
    pub modeled_canonical_ordinal: usize,
    pub bounds: Bounds,
    pub weight: usize,
    pub run_table: RunTable,
}

pub(crate) struct RegistryRegistration {
    pub(crate) modeled_canonical_ordinal: usize,
    pub(crate) action: NativeStemsBeamBuilderRegistrationAction,
}

pub(crate) struct GlyphRegistry {
    entries: Vec<RegisteredGlyph>,
    next_canonical_ordinal: usize,
    pub(crate) baseline: NativeStemsBeamBuilderRegistryBaseline,
}
impl GlyphRegistry {
    pub(crate) fn modeled_canonical_glyphs(&self) -> Vec<NativeStemsModeledCanonicalGlyph> {
        self.entries
            .iter()
            .map(|entry| NativeStemsModeledCanonicalGlyph {
                modeled_canonical_ordinal: entry.modeled_canonical_ordinal,
                bounds: entry.key.bounds,
                weight: entry.key.run_table.weight(),
                run_table: entry.key.run_table.clone(),
            })
            .collect()
    }
    pub(crate) fn seeded(
        grid: &GridLinesRecognition,
        beams: &NativeBeamRecognition,
        ledgers: &NativeLedgerRecognition,
        heads: &NativeHeadsRecognition,
        seeds: &NativeStemSeedRecognition,
    ) -> Result<Self, NativeStemsBeamBuilderError> {
        Self::seeded_without_raw_beams(grid, beams, ledgers, heads, seeds, &BTreeSet::new())
    }

    /// Seed the bounded registry visible to the source-ordered head-builder
    /// replay. Java walks the live SIG here, after `MultipleRestsBuilder` has
    /// removed each replaced source beam. The standalone beam-builder boundary
    /// deliberately retains its earlier pre-replacement baseline.
    pub(crate) fn seeded_for_head_builders(
        grid: &GridLinesRecognition,
        beams: &NativeBeamRecognition,
        ledgers: &NativeLedgerRecognition,
        heads: &NativeHeadsRecognition,
        seeds: &NativeStemSeedRecognition,
    ) -> Result<Self, NativeStemsBeamBuilderError> {
        let mut removed = BTreeSet::new();
        for rest in &beams.multiple_rests {
            let Some((owner, _)) = beams.raw_beam_glyphs.get(rest.source_beam_ordinal) else {
                return Err(NativeStemsBeamBuilderError::Geometry {
                    system_id: rest.system_id,
                });
            };
            if *owner != rest.system_id || !removed.insert(rest.source_beam_ordinal) {
                return Err(NativeStemsBeamBuilderError::Geometry {
                    system_id: rest.system_id,
                });
            }
        }
        Self::seeded_without_raw_beams(grid, beams, ledgers, heads, seeds, &removed)
    }

    fn seeded_without_raw_beams(
        grid: &GridLinesRecognition,
        beams: &NativeBeamRecognition,
        ledgers: &NativeLedgerRecognition,
        heads: &NativeHeadsRecognition,
        seeds: &NativeStemSeedRecognition,
        removed_raw_beams: &BTreeSet<usize>,
    ) -> Result<Self, NativeStemsBeamBuilderError> {
        let mut value = Self {
            entries: Vec::new(),
            next_canonical_ordinal: 0,
            baseline: NativeStemsBeamBuilderRegistryBaseline::default(),
        };
        for staff in &grid.peak_graph.sheet_staffs {
            for staff_line in &staff.lines {
                let HeadlessStaffLine::Persistent { line, .. } = staff_line else {
                    continue;
                };
                value.insert(
                    Bounds {
                        x: line.glyph.x,
                        y: line.glyph.y,
                        width: line.glyph.runs.width(),
                        height: line.glyph.runs.height(),
                    },
                    line.glyph.runs.clone(),
                );
                value.baseline.staff_line_glyphs += 1;
            }
        }
        let mut seed_keys = Vec::new();
        for system in &seeds.systems {
            value.baseline.stem_seed_raw_candidates += system.decisions.len();
            value.baseline.stem_seed_skipped_candidates += system
                .decisions
                .iter()
                .filter(|decision| matches!(decision, NativeStemSeedDecision::Skipped { .. }))
                .count();
            value.baseline.stem_seed_rejected_glyphs += system
                .decisions
                .iter()
                .filter(|decision| {
                    matches!(
                        decision,
                        NativeStemSeedDecision::Checked {
                            accepted: false,
                            ..
                        }
                    )
                })
                .count();
            for glyph in &system.registered_glyphs {
                let key = GlyphKey {
                    bounds: glyph.bounds,
                    run_table: glyph.run_table.clone(),
                };
                if !seed_keys.contains(&key) {
                    seed_keys.push(key.clone());
                    value.baseline.stem_seed_unique_contents += 1;
                }
                value.insert_key(key);
                value.baseline.stem_seed_glyphs += 1;
            }
        }
        for (ordinal, (_, glyph)) in beams.raw_beam_glyphs.iter().enumerate() {
            if removed_raw_beams.contains(&ordinal) {
                continue;
            }
            value.insert(beam_bounds(glyph)?, glyph.run_table.clone());
            value.baseline.beam_glyphs += 1;
        }
        for (_, glyph) in &beams.hook_glyphs {
            value.insert(beam_bounds(glyph)?, glyph.run_table.clone());
            value.baseline.beam_glyphs += 1;
        }
        for glyph in &ledgers.ledger_glyphs {
            value.insert(glyph.bounds, glyph.run_table.clone());
            value.baseline.ledger_glyphs += 1;
        }
        for system in &heads.seed_glyphs.systems {
            for staff in &system.staffs {
                for head in &staff.heads {
                    value.insert(
                        head_glyph_bounds(&head.glyph)?,
                        head.glyph.run_table.clone(),
                    );
                    value.baseline.head_glyphs += 1;
                }
            }
        }
        for system in &heads.range_glyphs.systems {
            for staff in &system.staffs {
                for head in &staff.heads {
                    value.insert(
                        head_glyph_bounds(&head.glyph)?,
                        head.glyph.run_table.clone(),
                    );
                    value.baseline.head_glyphs += 1;
                }
            }
        }
        Ok(value)
    }

    pub(crate) fn replay_system_stumps(
        &mut self,
        beam_stumps: &crate::native_stems_beam_stumps::NativeStemsBeamStumpSystem,
        head_stumps: &crate::native_stems_head_stumps::NativeStemsHeadStumpSystem,
    ) -> Vec<NativeStemsBeamBuilderPreBuilderGlyphRegistration> {
        let mut rows = Vec::new();
        // Beam linkers are equipped before head linkers. Keep even candidates
        // later rejected by direction/extension: `registerOriginal` happened
        // before either of those predicates.
        for beam in &beam_stumps.beams_by_abscissa {
            for side in &beam.sides {
                let Some(build) = &side.build else {
                    continue;
                };
                let (Some(candidate), Some(upstream_registration)) =
                    (&build.candidate, &build.registration)
                else {
                    continue;
                };
                let registration =
                    self.register_parts(candidate.bounds, candidate.run_table.clone());
                rows.push(NativeStemsBeamBuilderPreBuilderGlyphRegistration {
                    event_ordinal: rows.len(),
                    source: NativeStemsBeamBuilderPreBuilderGlyphSource::BeamStump {
                        beam_x_ordinal: beam.x_ordinal,
                        beam: beam.source,
                        side: side.side,
                    },
                    bounds: candidate.bounds,
                    run_table: candidate.run_table.clone(),
                    modeled_canonical_ordinal: registration.modeled_canonical_ordinal,
                    upstream_registration: NativeStemsBeamBuilderUpstreamRegistration::Beam(
                        upstream_registration.clone(),
                    ),
                    attachment: if side.final_stump.is_some() {
                        NativeStemsBeamBuilderPreBuilderAttachment::Attached
                    } else {
                        NativeStemsBeamBuilderPreBuilderAttachment::RejectedAfterRegistration
                    },
                    action: registration.action,
                });
            }
        }
        for head in &head_stumps.heads_by_abscissa {
            for corner in &head.corners_in_constructor_order {
                let Some(build) = &corner.build else {
                    continue;
                };
                let (Some(candidate), Some(upstream_registration)) =
                    (&build.candidate, &build.registration)
                else {
                    continue;
                };
                let registration =
                    self.register_parts(candidate.bounds, candidate.run_table.clone());
                rows.push(NativeStemsBeamBuilderPreBuilderGlyphRegistration {
                    event_ordinal: rows.len(),
                    source: NativeStemsBeamBuilderPreBuilderGlyphSource::HeadStump {
                        head_x_ordinal: head.x_ordinal,
                        head_sig_ordinal: head.sig_ordinal,
                        constructor_ordinal: corner.constructor_ordinal,
                    },
                    bounds: candidate.bounds,
                    run_table: candidate.run_table.clone(),
                    modeled_canonical_ordinal: registration.modeled_canonical_ordinal,
                    upstream_registration: NativeStemsBeamBuilderUpstreamRegistration::Head(
                        upstream_registration.clone(),
                    ),
                    attachment: if matches!(
                        corner.outcome,
                        NativeStemsHeadStumpOutcome::Built { .. }
                    ) {
                        NativeStemsBeamBuilderPreBuilderAttachment::Attached
                    } else {
                        NativeStemsBeamBuilderPreBuilderAttachment::RejectedAfterRegistration
                    },
                    action: registration.action,
                });
            }
        }
        rows
    }

    pub(crate) fn find(&self, key: &GlyphKey) -> Option<usize> {
        self.entries
            .iter()
            .find(|entry| entry.key == *key)
            .map(|entry| entry.modeled_canonical_ordinal)
    }

    fn insert(&mut self, bounds: Bounds, run_table: RunTable) -> usize {
        let key = GlyphKey { bounds, run_table };
        self.insert_key(key)
    }

    fn insert_key(&mut self, key: GlyphKey) -> usize {
        if let Some(ordinal) = self.find(&key) {
            return ordinal;
        }
        let modeled_canonical_ordinal = self.next_canonical_ordinal;
        self.next_canonical_ordinal += 1;
        self.entries.push(RegisteredGlyph {
            key,
            modeled_canonical_ordinal,
        });
        modeled_canonical_ordinal
    }

    pub(crate) fn register_parts(
        &mut self,
        bounds: Bounds,
        run_table: RunTable,
    ) -> RegistryRegistration {
        let key = GlyphKey { bounds, run_table };
        if let Some(modeled_canonical_ordinal) = self.find(&key) {
            RegistryRegistration {
                modeled_canonical_ordinal,
                action: NativeStemsBeamBuilderRegistrationAction::ReusedModeledCanonical,
            }
        } else {
            let modeled_canonical_ordinal = self.insert_key(key);
            let global_novelty = if self.baseline.complete_java_glyph_index {
                NativeStemsBeamBuilderGlobalNovelty::ProvenWithCompleteBaseline
            } else {
                NativeStemsBeamBuilderGlobalNovelty::UnresolvedIncompleteBaseline
            };
            RegistryRegistration {
                modeled_canonical_ordinal,
                action: NativeStemsBeamBuilderRegistrationAction::NewInModeledRegistry {
                    global_novelty,
                },
            }
        }
    }

    pub(crate) fn modeled_count(&self) -> usize {
        self.entries.len()
    }

    fn register(&mut self, glyph: &mut FixedGlyph) -> RegistryRegistration {
        let registration = self.register_parts(glyph.bounds, glyph.run_table.clone());
        glyph.modeled_canonical_ordinal = Some(registration.modeled_canonical_ordinal);
        registration
    }

    fn bind_existing(
        &self,
        glyph: &mut FixedGlyph,
        system_id: usize,
    ) -> Result<(), NativeStemsBeamBuilderError> {
        let Some(modeled_canonical_ordinal) = self.find(&glyph.identity) else {
            // A supplied source glyph is supposed to have passed through the
            // staged Java registry already. Treat an absent object as an
            // explicit unsupported baseline collision rather than replacing
            // Java identity with content equality.
            return Err(NativeStemsBeamBuilderError::UnsupportedBaselineCollision { system_id });
        };
        glyph.modeled_canonical_ordinal = Some(modeled_canonical_ordinal);
        Ok(())
    }
}
fn beam_bounds(
    glyph: &crate::beam_inters::RegisteredBeamGlyph,
) -> Result<Bounds, NativeStemsBeamBuilderError> {
    Ok(Bounds {
        x: usize::try_from(glyph.bounds.x)
            .map_err(|_| NativeStemsBeamBuilderError::Geometry { system_id: 0 })?,
        y: usize::try_from(glyph.bounds.y)
            .map_err(|_| NativeStemsBeamBuilderError::Geometry { system_id: 0 })?,
        width: usize::try_from(glyph.bounds.width)
            .map_err(|_| NativeStemsBeamBuilderError::Geometry { system_id: 0 })?,
        height: usize::try_from(glyph.bounds.height)
            .map_err(|_| NativeStemsBeamBuilderError::Geometry { system_id: 0 })?,
    })
}
fn head_glyph_bounds(glyph: &RetrievedHeadGlyph) -> Result<Bounds, NativeStemsBeamBuilderError> {
    Ok(Bounds {
        x: usize::try_from(glyph.glyph_bounds.x)
            .map_err(|_| NativeStemsBeamBuilderError::Geometry { system_id: 0 })?,
        y: usize::try_from(glyph.glyph_bounds.y)
            .map_err(|_| NativeStemsBeamBuilderError::Geometry { system_id: 0 })?,
        width: usize::try_from(glyph.glyph_bounds.width)
            .map_err(|_| NativeStemsBeamBuilderError::Geometry { system_id: 0 })?,
        height: usize::try_from(glyph.glyph_bounds.height)
            .map_err(|_| NativeStemsBeamBuilderError::Geometry { system_id: 0 })?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(y: f64) -> NativeStemPoint {
        NativeStemPoint { x: 0.0, y }
    }

    fn item(kind: NativeStemsBeamBuilderItemKind, y: f64) -> NativeStemsBeamBuilderItem {
        NativeStemsBeamBuilderItem {
            kind,
            glyph: None,
            target: None,
            reference_point: None,
            head_bounds: None,
            line: NativeStemLine {
                start: point(y),
                stop: point(y),
            },
            contribution: 0,
        }
    }

    fn chunk_glyph(
        filament_ordinal: usize,
        x: usize,
        y: usize,
        height: usize,
    ) -> (NativeStemsBeamBuilderGlyphRef, FixedGlyph) {
        let reference = NativeStemsBeamBuilderGlyphRef::Chunk {
            builder_ordinal: 0,
            filament_ordinal,
        };
        let pixels = vec![FOREGROUND; height];
        let run_table = RunTable::from_pixels(Orientation::Vertical, 1, height, &pixels).unwrap();
        let glyph = glyph_from_parts(
            reference,
            Bounds {
                x,
                y,
                width: 1,
                height,
            },
            height,
            run_table,
        )
        .unwrap();
        (reference, glyph)
    }

    #[test]
    fn sort_permutation_keeps_duplicate_occurrences_distinct() {
        let duplicate = item(NativeStemsBeamBuilderItemKind::SeedGlyph, 12.0);
        let before = vec![duplicate.clone(), duplicate.clone()];
        let rows = sort_permutation(&before, &before, 1);

        assert_eq!(
            rows.iter()
                .map(|row| row.before_ordinal)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        assert_eq!(
            rows.iter().map(|row| row.after_ordinal).collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    #[test]
    fn jdk25_mini_timsort_keeps_the_adversarial_cycle_permutation() {
        let mut first = item(NativeStemsBeamBuilderItemKind::HeadHalfLinker, 30.0);
        first.reference_point = Some(point(0.0));
        let mut second = item(NativeStemsBeamBuilderItemKind::HeadHalfLinker, 10.0);
        second.reference_point = Some(point(1.0));
        let third = item(NativeStemsBeamBuilderItemKind::BeamLinker, 20.0);
        let fourth = item(NativeStemsBeamBuilderItemKind::SeedGlyph, 40.0);
        // first < second by half-linker reference y; second < third and
        // third < first by mixed-pair line y, a strict 3-cycle.
        assert_eq!(item_cmp(&first, &second, 1), Ordering::Less);
        assert_eq!(item_cmp(&second, &third, 1), Ordering::Less);
        assert_eq!(item_cmp(&third, &first, 1), Ordering::Less);
        let expected = vec![first, second, third, fourth];
        let mut actual = expected.clone();

        stable_sort_items(&mut actual, 1, 3).unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn jdk25_merge_path_sorts_thirty_two_items() {
        let mut items = (0..32)
            .rev()
            .map(|y| item(NativeStemsBeamBuilderItemKind::SeedGlyph, f64::from(y)))
            .collect::<Vec<_>>();

        stable_sort_items(&mut items, 1, 9).expect("JDK TimSort merge path");
        assert_eq!(
            items
                .iter()
                .map(|item| item.line.start.y)
                .collect::<Vec<_>>(),
            (0..32).map(f64::from).collect::<Vec<_>>()
        );
    }

    #[test]
    fn java_double_compare_preserves_zero_and_nan_rules() {
        let payload_nan = f64::from_bits(0x7ff8_0000_0000_0001);

        assert_eq!(java_double_compare(-0.0, 0.0), -1);
        assert_eq!(java_double_compare(0.0, -0.0), 1);
        assert_eq!(java_double_compare(payload_nan, f64::NAN), 0);
        assert_eq!(java_double_compare(0.0, payload_nan), -1);
    }

    #[test]
    fn upward_half_linkers_with_equal_references_compare_equal() {
        let mut first = item(NativeStemsBeamBuilderItemKind::HeadHalfLinker, 10.0);
        let mut second = item(NativeStemsBeamBuilderItemKind::HeadHalfLinker, 30.0);
        first.reference_point = Some(point(20.0));
        second.reference_point = Some(point(20.0));

        assert_eq!(item_cmp(&first, &second, -1), Ordering::Equal);
    }

    #[test]
    fn unaligned_removal_keeps_the_java_for_loop_index() {
        let mut glyphs = (0..5)
            .map(|ordinal| chunk_glyph(ordinal, 0, ordinal * 10, 2))
            .collect::<Vec<_>>();
        glyphs.push(chunk_glyph(5, 10, 50, 3));
        let skew = HeadlessSkew::new(0.0, 100, 100);
        let parameters = BuilderParams::new(10, 2).unwrap();

        let (pass, _) = filter_unaligned(
            &mut glyphs,
            None,
            1,
            &skew,
            parameters,
            NativeStemsBeamBuilderAlignmentSubject::Chunks,
        );

        assert_eq!(pass.comparisons.len(), 5);
        assert_eq!(pass.comparisons[4].first, chunk_glyph(4, 0, 40, 2).0);
        assert_eq!(pass.comparisons[4].second, chunk_glyph(5, 10, 50, 3).0);
        assert_eq!(pass.removed, [chunk_glyph(4, 0, 40, 2).0]);
        assert_eq!(
            pass.retained,
            [0, 1, 2, 3, 5].map(|filament_ordinal| NativeStemsBeamBuilderGlyphRef::Chunk {
                builder_ordinal: 0,
                filament_ordinal,
            })
        );
    }

    #[test]
    fn length_at_unions_head_inter_bounds() {
        let mut head = item(NativeStemsBeamBuilderItemKind::HeadHalfLinker, 10.0);
        head.head_bounds = Some(JavaRectangle::new(0, 0, 4, 30));
        let theoretical = NativeStemLine {
            start: point(10.0),
            stop: point(80.0),
        };

        assert_eq!(length_at(&[head], 0, 1, theoretical), 20);
    }

    #[test]
    fn builder_direction_comes_from_theoretical_line_not_v_linker() {
        // Frozen Carmen system 2 / builder 56 is the one corpus case where
        // the V points down but its theoretical line ends above its start.
        let theoretical = NativeStemLine {
            start: NativeStemPoint {
                x: f64::from_bits(0x40a0_4500_0000_0000),
                y: f64::from_bits(0x4099_d253_0a1f_10a7),
            },
            stop: NativeStemPoint {
                x: f64::from_bits(0x40a0_4503_fa40_0394),
                y: f64::from_bits(0x4099_c000_0000_0000),
            },
        };
        let v_y_direction = 1;
        let y_direction = stem_builder_y_direction(theoretical);
        let mut start = item(
            NativeStemsBeamBuilderItemKind::StartHalfLinker,
            theoretical.start.y,
        );
        start.line = NativeStemLine {
            start: theoretical.start,
            stop: theoretical.start,
        };

        assert_eq!(v_y_direction, 1);
        assert_eq!(y_direction, -1);
        assert_eq!(length_at(&[start], 0, y_direction, theoretical), 0);
    }

    #[test]
    fn staged_future_system_glyph_is_not_visible_before_its_registration() {
        let run_table =
            RunTable::from_pixels(Orientation::Vertical, 1, 2, &[FOREGROUND, FOREGROUND]).unwrap();
        let mut glyph = glyph_from_parts(
            NativeStemsBeamBuilderGlyphRef::Chunk {
                builder_ordinal: 1,
                filament_ordinal: 0,
            },
            Bounds {
                x: 3,
                y: 5,
                width: 1,
                height: 2,
            },
            2,
            run_table,
        )
        .unwrap();
        let mut registry = GlyphRegistry {
            entries: Vec::new(),
            next_canonical_ordinal: 0,
            baseline: NativeStemsBeamBuilderRegistryBaseline::default(),
        };

        assert!(matches!(
            registry.bind_existing(&mut glyph, 7),
            Err(NativeStemsBeamBuilderError::UnsupportedBaselineCollision { system_id: 7 })
        ));
        let registration = registry.register(&mut glyph);
        assert_eq!(registration.modeled_canonical_ordinal, 0);
        assert_eq!(
            registration.action,
            NativeStemsBeamBuilderRegistrationAction::NewInModeledRegistry {
                global_novelty: NativeStemsBeamBuilderGlobalNovelty::UnresolvedIncompleteBaseline,
            }
        );
        registry.bind_existing(&mut glyph, 7).unwrap();
        assert_eq!(glyph.modeled_canonical_ordinal, Some(0));
    }
}
