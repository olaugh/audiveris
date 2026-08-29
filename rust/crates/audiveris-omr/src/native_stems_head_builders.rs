// SPDX-License-Identifier: AGPL-3.0-or-later

//! Head-origin `StemBuilder` construction through item and length retrieval.
//!
//! This is the read-only boundary assigned by `CLinker.inspect`: it consumes
//! the already materialized head reachability product, replays the page glyph
//! registry in the real system-interleaved order, and stops before expansion
//! can create a stem or mutate the SIG/link graph.

use std::{cmp::Ordering, collections::BTreeMap, error::Error, fmt};

use audiveris_image::{
    beam_structure::Segment,
    run_table::{BACKGROUND, FOREGROUND, Orientation, RunTable, RunTableError},
    section::{Bounds, Section},
    stick_factory::{StraightStickError, VerticalStickFactory, VerticalStickParameters},
};

use crate::{
    beam_recognizer::run_table_center_line,
    grid_executor::HeadlessSkew,
    head_glyph_retrieval::RetrievedHeadGlyph,
    head_scanner_slices::JavaRectangle,
    native_heads::NativeHeadsRecognition,
    native_heads_staff_epilog::{
        NativeHeadStaffEpilogHead, NativeHeadStaffEpilogOrigin, NativeHeadStaffEpilogRef,
        NativeHeadsStaffEpilogSystem,
    },
    native_ledgers::NativeLedgerRecognition,
    native_stem_seeds::{NativeStemSeedDecision, NativeStemSeedRecognition},
    native_stems_beam_builders::{
        GlyphKey, GlyphRegistry, NativeStemsBeamBuilderError,
        NativeStemsBeamBuilderPreBuilderGlyphSource, NativeStemsBeamBuilderRecognition,
        NativeStemsBeamBuilderRegistrationAction, NativeStemsBeamBuilderRegistryBaseline,
        NativeStemsModeledCanonicalGlyph, java_double_compare,
    },
    native_stems_beam_stumps::{
        NativeStemsBeamSource, NativeStemsBeamStumpBeam, NativeStemsBeamStumpRecognition,
        NativeStemsBeamStumpRef,
    },
    native_stems_beam_vlinkers::{
        NativeStemsBeamBLinker, NativeStemsBeamBLinkerRef, NativeStemsBeamVLinkerRecognition,
        convex_quad_intersects_rectangle,
    },
    native_stems_head_corner_reachability::{
        NativeStemsHeadCornerReachabilityRecognition, NativeStemsHeadCornerReachabilitySystem,
        NativeStemsHeadCornerRef, NativeStemsHeadReachabilityCorner,
        NativeStemsHeadReachabilityHead, NativeStemsHeadReachabilityTarget,
        NativeStemsHeadStumpRef,
    },
    native_stems_head_stumps::NativeStemsHeadStumpRecognition,
    recognize::{GridLinesRecognition, NativeBeamRecognition},
    stems_step::{NativeStemLine, NativeStemPoint},
};

const MIN_SIDE_RATIO: f64 = 0.4;
const HEAD_PART_MIN_REMAINING_WEIGHT: usize = 15;

/// Full immutable result of every head-origin Java `StemBuilder` constructor.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadBuilderRecognition {
    /// Explicit page stub profile passed to every Java `CLinker.inspect`.
    pub inspect_profile: i32,
    pub systems: Vec<NativeStemsHeadBuilderSystem>,
    pub builder_count: usize,
    pub item_count: usize,
    pub gap_count: usize,
    pub head_chunk_registration_count: usize,
    pub registry_event_count: usize,
    pub recomputed_beam_action_change_count: usize,
    pub low_remain_non_vip_keep_count: usize,
    pub vip_head_count: usize,
    /// Bounded page registry at the live post-BEAMS SIG boundary. Unlike the
    /// standalone beam-builder baseline, this excludes raw beams already
    /// replaced by `MultipleRestsBuilder`.
    pub registry_baseline: NativeStemsBeamBuilderRegistryBaseline,
    /// Complete modeled page registry after the final head-builder replay.
    pub modeled_canonical_glyphs: Vec<NativeStemsModeledCanonicalGlyph>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadBuilderSystem {
    pub system_id: usize,
    /// Effective `SystemInfo.getProfile()` used by reachability parameters.
    pub system_profile: i32,
    /// Page stub profile used as the builder's `maxStemProfile`.
    pub inspect_profile: i32,
    /// The current boundary fails before construction when this is true.
    pub inspect_profile_diverges: bool,
    pub interline: i32,
    pub max_stem_thickness: i32,
    pub max_line_section_dx: f64,
    pub max_stem_alignment_dx: f64,
    pub max_stem_alignment_dy: f64,
    /// `StickFactory` constructor input. Java uses zero because the lookup has
    /// already enforced the useful vertical-section length.
    pub minimum_core_section_length: usize,
    /// Exact `VerticalsBuilder.getMinSideRatio()` value passed to the factory.
    pub minimum_side_ratio: f64,
    pub gap_map: BTreeMap<i32, i32>,
    pub vertical_section_source_ordinals: Vec<usize>,
    pub horizontal_section_source_ordinals: Vec<usize>,
    /// Stumps, then all beam chunks, then all C chunks for this system.
    pub registry_events: Vec<NativeStemsHeadBuilderRegistryEvent>,
    pub builders: Vec<NativeStemsHeadBuilder>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsHeadBuilderRegistryEvent {
    pub system_event_ordinal: usize,
    pub occurrence: NativeStemsHeadBuilderRegistryOccurrence,
    pub bounds: Bounds,
    pub weight: usize,
    pub run_table: RunTable,
    pub modeled_canonical_ordinal: usize,
    pub action: NativeStemsBeamBuilderRegistrationAction,
    /// Present only on replayed beam events; never trusted as page chronology.
    pub isolated_beam_action: Option<NativeStemsBeamBuilderRegistrationAction>,
    pub action_changed_from_isolated_beam: bool,
    pub modeled_count_before: usize,
    pub modeled_count_after: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsHeadBuilderRegistryOccurrence {
    PreBuilder {
        event_ordinal: usize,
        source: NativeStemsBeamBuilderPreBuilderGlyphSource,
    },
    BeamChunk {
        builder_ordinal: usize,
        filament_ordinal: usize,
    },
    HeadChunk {
        builder_ordinal: usize,
        filament_ordinal: usize,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadBuilder {
    pub builder_ordinal: usize,
    pub c_builder_assignment: NativeStemsHeadCornerRef,
    pub start: NativeStemsHeadCornerRef,
    pub source_is_vip: bool,
    pub max_stem_profile: i32,
    pub c_y_direction: i32,
    pub y_direction: i32,
    pub theoretical_line: NativeStemLine,
    pub lookup_quadrilateral: [NativeStemPoint; 4],
    pub lookup_bounds: JavaRectangle,
    /// `StemBuilder.yRange = theoretical_line.getBounds()`, distinct from
    /// the C linker's rounded retrieveSeeds range retained upstream.
    pub y_range: JavaRectangle,
    pub start_stump: Option<NativeStemsHeadBuilderGlyphRef>,
    pub input_seed_ordinals: Vec<usize>,
    pub seeds_after_filter: Vec<NativeStemsHeadBuilderGlyphRef>,
    pub target_input: Vec<NativeStemsHeadBuilderTargetRef>,
    pub targets_after_filter: Vec<NativeStemsHeadBuilderTargetRef>,
    pub alignment: Vec<NativeStemsHeadBuilderAlignmentPass>,
    pub seed_filter: Vec<NativeStemsHeadBuilderSeedDecision>,
    pub target_filter: Vec<NativeStemsHeadBuilderTargetDecision>,
    pub target_sort: Vec<NativeStemsHeadBuilderSortEntry>,
    /// C-origin builders cannot establish `lastHeadY`.
    pub last_head_y: Option<f64>,
    pub vertical_sections: Vec<NativeStemsHeadBuilderSectionScan>,
    pub horizontal_sections: Vec<NativeStemsHeadBuilderSectionScan>,
    pub filaments: Vec<NativeStemsHeadBuilderFilament>,
    pub glyph_registrations: Vec<NativeStemsHeadBuilderGlyphRegistration>,
    pub chunks: Vec<NativeStemsHeadBuilderChunk>,
    pub items_before_sort: Vec<NativeStemsHeadBuilderItem>,
    pub sort: Vec<NativeStemsHeadBuilderSortEntry>,
    pub gaps: Vec<NativeStemsHeadBuilderGapEvent>,
    pub items: Vec<NativeStemsHeadBuilderItem>,
    pub lengths: BTreeMap<i32, i32>,
    pub sig_mutation_count: usize,
    pub system_stem_mutation_count: usize,
    pub link_mutation_count: usize,
    pub beam_arena_mutation_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsHeadBuilderTargetRef {
    Head(NativeStemsHeadCornerRef),
    Beam(NativeStemsBeamBLinkerRef),
}

impl From<NativeStemsHeadReachabilityTarget> for NativeStemsHeadBuilderTargetRef {
    fn from(value: NativeStemsHeadReachabilityTarget) -> Self {
        match value {
            NativeStemsHeadReachabilityTarget::Head(value) => Self::Head(value),
            NativeStemsHeadReachabilityTarget::Beam(value) => Self::Beam(value),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsHeadBuilderGlyphRef {
    StemSeed {
        free_glyph_ordinal: usize,
    },
    HeadStump {
        corner: NativeStemsHeadCornerRef,
    },
    BeamStump {
        b_linker: NativeStemsBeamBLinkerRef,
    },
    Chunk {
        builder_ordinal: usize,
        filament_ordinal: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsHeadBuilderAlignmentSubject {
    Seeds,
    Chunks,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadBuilderAlignmentPass {
    pub subject: NativeStemsHeadBuilderAlignmentSubject,
    pub sorted_before: Vec<NativeStemsHeadBuilderGlyphRef>,
    pub comparisons: Vec<NativeStemsHeadBuilderAlignmentCheck>,
    pub removed_structural_keys: Vec<NativeStemsHeadBuilderGlyphRef>,
    pub retained_occurrences: Vec<NativeStemsHeadBuilderGlyphRef>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadBuilderAlignmentCheck {
    pub first: NativeStemsHeadBuilderGlyphRef,
    pub second: NativeStemsHeadBuilderGlyphRef,
    pub first_deskewed: (f64, f64),
    pub second_deskewed: (f64, f64),
    pub dx: Option<f64>,
    pub dy: f64,
    pub dy_bypasses_dx: bool,
    pub aligned: bool,
    pub selected_alien: Option<NativeStemsHeadBuilderGlyphRef>,
    pub actual_removed_occurrence: Option<NativeStemsHeadBuilderGlyphRef>,
    pub equal_height_removed_second: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsHeadBuilderSeedAction {
    AlignmentRemoved,
    DuplicateTargetIdentity,
    OverlapsStart,
    ZeroContribution,
    Item,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadBuilderSeedDecision {
    pub glyph: NativeStemsHeadBuilderGlyphRef,
    pub retained_in_mutable_seed_set: bool,
    pub included_as_item: bool,
    pub action: NativeStemsHeadBuilderSeedAction,
    pub contribution: Option<i32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadBuilderTargetDecision {
    pub target: NativeStemsHeadBuilderTargetRef,
    pub stump: Option<NativeStemsHeadBuilderGlyphRef>,
    pub removed_by_structural_seed: bool,
    pub included: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadBuilderSectionScan {
    pub source_ordinal: usize,
    pub bounds: Bounds,
    pub intersects_lookup: bool,
    pub width_accepted: Option<bool>,
    pub stump_overlap_accepted: Option<bool>,
    /// `None` for C builders because the V-only `lastHeadY` predicate is skipped.
    pub before_last_head: Option<bool>,
    pub line_distance: Option<f64>,
    pub distance_accepted: Option<bool>,
    pub accepted: bool,
    pub accepted_sorted_ordinal: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeStemsHeadBuilderSectionRef {
    pub orientation: Orientation,
    /// Ordinal in the system-dispatched section vector, matching Java probe aliases.
    pub source_ordinal: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadBuilderFilament {
    pub filament_ordinal: usize,
    pub creation_id: u64,
    pub member_section_source_ordinals: Vec<NativeStemsHeadBuilderSectionRef>,
    pub bounds: Bounds,
    pub weight: usize,
    pub start: (f64, f64),
    pub stop: (f64, f64),
    pub mean_thickness: f64,
    pub mean_distance: f64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStemsHeadBuilderGlyphRegistration {
    pub glyph: NativeStemsHeadBuilderGlyphRef,
    pub bounds: Bounds,
    pub weight: usize,
    pub run_table: RunTable,
    pub modeled_canonical_ordinal: usize,
    pub action: NativeStemsBeamBuilderRegistrationAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsHeadBuilderHeadPartsAction {
    Keep,
    KeepNonVipJavaBehavior,
    RemoveVipOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsHeadBuilderChunkAction {
    Keep,
    SeedStructural,
    HeadPartsVipOnly,
    UnalignedStructural,
    StartFirstStructural,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadBuilderChunk {
    pub glyph: NativeStemsHeadBuilderGlyphRef,
    pub bounds: Bounds,
    pub run_table: RunTable,
    pub modeled_canonical_ordinal: usize,
    pub head_y_overlap: i32,
    pub head_pixels_removed: usize,
    pub remaining_weight: usize,
    pub head_parts_action: NativeStemsHeadBuilderHeadPartsAction,
    pub final_ordinal: Option<usize>,
    pub action: NativeStemsHeadBuilderChunkAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsHeadBuilderItemKind {
    StartHeadHalfLinker,
    HeadHalfLinker,
    BeamLinker,
    SeedGlyph,
    ChunkGlyph,
    Gap,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadBuilderItem {
    pub kind: NativeStemsHeadBuilderItemKind,
    pub glyph: Option<NativeStemsHeadBuilderGlyphRef>,
    pub target: Option<NativeStemsHeadBuilderTargetRef>,
    pub reference_point: Option<NativeStemPoint>,
    pub head_bounds: Option<JavaRectangle>,
    pub line: NativeStemLine,
    pub contribution: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadBuilderSortEntry {
    pub before_ordinal: usize,
    pub after_ordinal: usize,
    pub item: NativeStemsHeadBuilderItem,
    pub tied_with_previous: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsHeadBuilderGapAction {
    None,
    Inserted,
    Truncated,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadBuilderGapEvent {
    pub item_ordinal_before_insert: usize,
    pub previous_stop: NativeStemPoint,
    pub next_start: NativeStemPoint,
    pub gap: f64,
    pub max_gap: i32,
    pub action: NativeStemsHeadBuilderGapAction,
}

#[derive(Debug)]
pub enum NativeStemsHeadBuilderError {
    SystemOrder,
    MissingSystemProduct {
        system_id: usize,
        product: &'static str,
    },
    InvalidParameters {
        system_id: usize,
    },
    UnsupportedInspectProfileDivergence {
        system_id: usize,
        inspect_profile: i32,
        system_profile: i32,
    },
    HeadProvenance {
        system_id: usize,
        head: NativeHeadStaffEpilogRef,
    },
    MissingSeed {
        system_id: usize,
        free_glyph_ordinal: usize,
    },
    MissingStump {
        system_id: usize,
    },
    MissingTarget {
        system_id: usize,
    },
    RegistryBaseline {
        system_id: usize,
    },
    ReachabilityInvariant {
        system_id: usize,
        corner: Option<NativeStemsHeadCornerRef>,
        phase: &'static str,
    },
    JdkTimSortContractViolation {
        system_id: usize,
        phase: &'static str,
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
    BeamBuilderBaseline(NativeStemsBeamBuilderError),
}

impl fmt::Display for NativeStemsHeadBuilderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SystemOrder => formatter.write_str("STEMS head-builder system order differs"),
            Self::MissingSystemProduct { system_id, product } => write!(
                formatter,
                "STEMS head-builder system {system_id} has no {product}"
            ),
            Self::InvalidParameters { system_id } => write!(
                formatter,
                "STEMS head-builder system {system_id} has invalid parameters"
            ),
            Self::UnsupportedInspectProfileDivergence {
                system_id,
                inspect_profile,
                system_profile,
            } => write!(
                formatter,
                "STEMS head-builder system {system_id} page inspect profile {inspect_profile} differs from effective system profile {system_profile}"
            ),
            Self::HeadProvenance { system_id, head } => write!(
                formatter,
                "STEMS head-builder system {system_id} head {head:?} provenance differs"
            ),
            Self::MissingSeed {
                system_id,
                free_glyph_ordinal,
            } => write!(
                formatter,
                "STEMS head-builder system {system_id} seed {free_glyph_ordinal} is missing"
            ),
            Self::MissingStump { system_id } => write!(
                formatter,
                "STEMS head-builder system {system_id} stump is missing"
            ),
            Self::MissingTarget { system_id } => write!(
                formatter,
                "STEMS head-builder system {system_id} target is missing"
            ),
            Self::RegistryBaseline { system_id } => write!(
                formatter,
                "STEMS head-builder system {system_id} source glyph is absent from the modeled registry"
            ),
            Self::ReachabilityInvariant {
                system_id,
                corner,
                phase,
            } => match corner {
                Some(corner) => write!(
                    formatter,
                    "STEMS head-builder system {system_id} corner {corner:?} reachability invariant differs during {phase}"
                ),
                None => write!(
                    formatter,
                    "STEMS head-builder system {system_id} reachability invariant differs during {phase}"
                ),
            },
            Self::JdkTimSortContractViolation {
                system_id,
                phase,
                length,
            } => write!(
                formatter,
                "STEMS head-builder system {system_id} {phase} sort length {length} triggers Java's comparator-contract exception"
            ),
            Self::Geometry { system_id } => write!(
                formatter,
                "STEMS head-builder system {system_id} geometry is invalid"
            ),
            Self::StickFactory { system_id, source } => write!(
                formatter,
                "STEMS head-builder system {system_id} StickFactory: {source}"
            ),
            Self::RunTable(source) => write!(formatter, "STEMS head-builder run table: {source}"),
            Self::BeamBuilderBaseline(source) => {
                write!(formatter, "STEMS head-builder registry baseline: {source}")
            }
        }
    }
}

impl Error for NativeStemsHeadBuilderError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::StickFactory { source, .. } => Some(source),
            Self::RunTable(source) => Some(source),
            Self::BeamBuilderBaseline(source) => Some(source),
            _ => None,
        }
    }
}

impl From<RunTableError> for NativeStemsHeadBuilderError {
    fn from(source: RunTableError) -> Self {
        Self::RunTable(source)
    }
}

#[derive(Clone)]
struct FixedGlyph {
    reference: NativeStemsHeadBuilderGlyphRef,
    bounds: Bounds,
    weight: usize,
    run_table: RunTable,
    line: Segment,
    modeled_canonical_ordinal: usize,
}

#[derive(Clone)]
struct TargetState {
    target: NativeStemsHeadBuilderTargetRef,
    stump: Option<FixedGlyph>,
    reference_point: NativeStemPoint,
    head_bounds: Option<JavaRectangle>,
    beam_height: Option<f64>,
}

struct SystemContext<'a> {
    system_id: usize,
    inspect_profile: i32,
    grid: &'a GridLinesRecognition,
    heads: &'a NativeHeadsRecognition,
    seed_system: &'a crate::native_stem_seeds::NativeStemSeedSystemRecognition,
    beam_stump_system: &'a crate::native_stems_beam_stumps::NativeStemsBeamStumpSystem,
    beam_v_system: &'a crate::native_stems_beam_vlinkers::NativeStemsBeamVLinkerSystem,
    head_stump_system: &'a crate::native_stems_head_stumps::NativeStemsHeadStumpSystem,
    reach_system: &'a NativeStemsHeadCornerReachabilitySystem,
    head_system: &'a NativeHeadsStaffEpilogSystem,
    beam_builder_system: &'a crate::native_stems_beam_builders::NativeStemsBeamBuilderSystem,
    skew: HeadlessSkew,
}

/// Materialize every C-origin constructor while preserving the real page
/// registry order: `system1 stumps -> beam builders -> C builders -> system2`.
#[allow(clippy::too_many_arguments)]
pub fn materialize_native_stems_head_builders(
    grid: &GridLinesRecognition,
    beams: &NativeBeamRecognition,
    ledgers: &NativeLedgerRecognition,
    heads: &NativeHeadsRecognition,
    stem_seeds: &NativeStemSeedRecognition,
    beam_stumps: &NativeStemsBeamStumpRecognition,
    beam_vlinkers: &NativeStemsBeamVLinkerRecognition,
    head_stumps: &NativeStemsHeadStumpRecognition,
    beam_builders: &NativeStemsBeamBuilderRecognition,
    reachability: &NativeStemsHeadCornerReachabilityRecognition,
    inspect_profile: i32,
) -> Result<NativeStemsHeadBuilderRecognition, NativeStemsHeadBuilderError> {
    if !(0..=4).contains(&inspect_profile) {
        return Err(NativeStemsHeadBuilderError::InvalidParameters { system_id: 0 });
    }
    let ids = grid
        .peak_graph
        .sig
        .systems
        .iter()
        .map(|s| s.system_id)
        .collect::<Vec<_>>();
    let same = |actual: Vec<usize>| actual == ids;
    if !same(stem_seeds.systems.iter().map(|s| s.raw.system_id).collect())
        || !same(beam_stumps.systems.iter().map(|s| s.system_id).collect())
        || !same(beam_vlinkers.systems.iter().map(|s| s.system_id).collect())
        || !same(head_stumps.systems.iter().map(|s| s.system_id).collect())
        || !same(beam_builders.systems.iter().map(|s| s.system_id).collect())
        || !same(reachability.systems.iter().map(|s| s.system_id).collect())
        || !same(
            heads
                .epilog
                .staff_epilog
                .systems
                .iter()
                .map(|s| s.system_id)
                .collect(),
        )
    {
        return Err(NativeStemsHeadBuilderError::SystemOrder);
    }

    let mut registry =
        GlyphRegistry::seeded_for_head_builders(grid, beams, ledgers, heads, stem_seeds)
            .map_err(NativeStemsHeadBuilderError::BeamBuilderBaseline)?;
    let baseline = registry.baseline.clone();
    let mut expected_baseline = beam_builders.registry_baseline.clone();
    expected_baseline.beam_glyphs = expected_baseline
        .beam_glyphs
        .checked_sub(beams.multiple_rests.len())
        .ok_or(NativeStemsHeadBuilderError::RegistryBaseline { system_id: 0 })?;
    if baseline != expected_baseline {
        return Err(NativeStemsHeadBuilderError::RegistryBaseline { system_id: 0 });
    }
    let width = i32::try_from(grid.no_staff.width())
        .map_err(|_| NativeStemsHeadBuilderError::Geometry { system_id: 0 })?;
    let height = i32::try_from(grid.no_staff.height())
        .map_err(|_| NativeStemsHeadBuilderError::Geometry { system_id: 0 })?;
    let mut systems = Vec::with_capacity(ids.len());
    let mut recomputed_changes = 0_usize;
    let mut vip_head_count = 0_usize;

    for (index, &system_id) in ids.iter().enumerate() {
        let seed_system = &stem_seeds.systems[index];
        let beam_stump_system = &beam_stumps.systems[index];
        let beam_v_system = &beam_vlinkers.systems[index];
        let head_stump_system = &head_stumps.systems[index];
        let beam_builder_system = &beam_builders.systems[index];
        let reach_system = &reachability.systems[index];
        let head_system = &heads.epilog.staff_epilog.systems[index];
        if seed_system.raw.interline <= 0 || beam_builder_system.max_stem_thickness <= 0 {
            return Err(NativeStemsHeadBuilderError::InvalidParameters { system_id });
        }
        if inspect_profile != reach_system.profile {
            return Err(
                NativeStemsHeadBuilderError::UnsupportedInspectProfileDivergence {
                    system_id,
                    inspect_profile,
                    system_profile: reach_system.profile,
                },
            );
        }
        let context = SystemContext {
            system_id,
            inspect_profile,
            grid,
            heads,
            seed_system,
            beam_stump_system,
            beam_v_system,
            head_stump_system,
            reach_system,
            head_system,
            beam_builder_system,
            skew: HeadlessSkew::new(grid.global_slope, width, height),
        };
        validate_system(&context)?;
        vip_head_count += reach_system
            .heads
            .iter()
            .filter(|head| {
                staff_head(head_system, head.reference).is_some_and(|source| source.is_vip)
            })
            .count();
        let mut registry_events = Vec::new();

        let count_before_stumps = registry.modeled_count();
        let stump_rows = registry.replay_system_stumps(beam_stump_system, head_stump_system);
        let mut modeled_count = count_before_stumps;
        for row in stump_rows {
            let before = modeled_count;
            if is_new(row.action) {
                modeled_count += 1;
            }
            registry_events.push(NativeStemsHeadBuilderRegistryEvent {
                system_event_ordinal: registry_events.len(),
                occurrence: NativeStemsHeadBuilderRegistryOccurrence::PreBuilder {
                    event_ordinal: row.event_ordinal,
                    source: row.source,
                },
                bounds: row.bounds,
                weight: run_table_weight(&row.run_table),
                run_table: row.run_table,
                modeled_canonical_ordinal: row.modeled_canonical_ordinal,
                action: row.action,
                isolated_beam_action: None,
                action_changed_from_isolated_beam: false,
                modeled_count_before: before,
                modeled_count_after: modeled_count,
            });
        }
        if modeled_count != registry.modeled_count() {
            return Err(NativeStemsHeadBuilderError::RegistryBaseline { system_id });
        }

        // The existing beam product carries exact candidates, but its actions
        // came from an all-beam-only page. Re-register each candidate here.
        for beam_builder in &beam_builder_system.builders {
            for (filament_ordinal, row) in beam_builder.glyph_registrations.iter().enumerate() {
                let before = registry.modeled_count();
                let replay = registry.register_parts(row.bounds, row.run_table.clone());
                let after = registry.modeled_count();
                let changed = replay.action != row.action;
                recomputed_changes += usize::from(changed);
                registry_events.push(NativeStemsHeadBuilderRegistryEvent {
                    system_event_ordinal: registry_events.len(),
                    occurrence: NativeStemsHeadBuilderRegistryOccurrence::BeamChunk {
                        builder_ordinal: beam_builder.builder_ordinal,
                        filament_ordinal,
                    },
                    bounds: row.bounds,
                    weight: row.weight,
                    run_table: row.run_table.clone(),
                    modeled_canonical_ordinal: replay.modeled_canonical_ordinal,
                    action: replay.action,
                    isolated_beam_action: Some(row.action),
                    action_changed_from_isolated_beam: changed,
                    modeled_count_before: before,
                    modeled_count_after: after,
                });
            }
        }

        let mut builders = Vec::new();
        for corner_ref in &reach_system.c_inspection_order {
            let (head, corner) = find_reach_corner(reach_system, *corner_ref)
                .ok_or(NativeStemsHeadBuilderError::MissingTarget { system_id })?;
            let builder = materialize_builder(
                &context,
                head,
                corner,
                builders.len(),
                &mut registry,
                &mut registry_events,
            )?;
            builders.push(builder);
        }
        systems.push(NativeStemsHeadBuilderSystem {
            system_id,
            system_profile: reach_system.profile,
            inspect_profile,
            inspect_profile_diverges: false,
            interline: seed_system.raw.interline,
            max_stem_thickness: beam_builder_system.max_stem_thickness,
            max_line_section_dx: beam_builder_system.max_line_section_dx,
            max_stem_alignment_dx: beam_builder_system.max_stem_alignment_dx,
            max_stem_alignment_dy: beam_builder_system.max_stem_alignment_dy,
            minimum_core_section_length: 0,
            minimum_side_ratio: MIN_SIDE_RATIO,
            gap_map: beam_builder_system.gap_map.clone(),
            vertical_section_source_ordinals: beam_builder_system
                .vertical_section_source_ordinals
                .clone(),
            horizontal_section_source_ordinals: beam_builder_system
                .horizontal_section_source_ordinals
                .clone(),
            registry_events,
            builders,
        });
    }

    let builder_count = systems.iter().map(|s| s.builders.len()).sum();
    let item_count = systems
        .iter()
        .flat_map(|s| &s.builders)
        .map(|b| b.items.len())
        .sum();
    let gap_count = systems
        .iter()
        .flat_map(|s| &s.builders)
        .flat_map(|b| &b.items)
        .filter(|i| i.kind == NativeStemsHeadBuilderItemKind::Gap)
        .count();
    let head_chunk_registration_count = systems
        .iter()
        .flat_map(|s| &s.builders)
        .map(|b| b.glyph_registrations.len())
        .sum();
    let registry_event_count = systems.iter().map(|s| s.registry_events.len()).sum();
    let low_remain_non_vip_keep_count = systems
        .iter()
        .flat_map(|s| &s.builders)
        .flat_map(|b| &b.chunks)
        .filter(|c| {
            c.head_parts_action == NativeStemsHeadBuilderHeadPartsAction::KeepNonVipJavaBehavior
        })
        .count();
    let modeled_canonical_glyphs = registry.modeled_canonical_glyphs();
    Ok(NativeStemsHeadBuilderRecognition {
        inspect_profile,
        systems,
        builder_count,
        item_count,
        gap_count,
        head_chunk_registration_count,
        registry_event_count,
        recomputed_beam_action_change_count: recomputed_changes,
        low_remain_non_vip_keep_count,
        vip_head_count,
        registry_baseline: baseline,
        modeled_canonical_glyphs,
    })
}

fn is_new(action: NativeStemsBeamBuilderRegistrationAction) -> bool {
    matches!(
        action,
        NativeStemsBeamBuilderRegistrationAction::NewInModeledRegistry { .. }
    )
}

fn validate_system(context: &SystemContext<'_>) -> Result<(), NativeStemsHeadBuilderError> {
    let system_id = context.system_id;
    if context
        .beam_builder_system
        .gap_map
        .keys()
        .copied()
        .collect::<Vec<_>>()
        != vec![0, 1, 2, 3, 4]
        || !context
            .beam_builder_system
            .gap_map
            .values()
            .copied()
            .is_sorted()
    {
        return Err(NativeStemsHeadBuilderError::InvalidParameters { system_id });
    }
    let mut checked = 0_usize;
    let mut accepted_checked = 0_usize;
    for decision in &context.seed_system.decisions {
        if let NativeStemSeedDecision::Checked {
            gate,
            registered_glyph_index,
            accepted,
            free_glyph_index,
            ..
        } = decision
        {
            let glyph = context
                .seed_system
                .registered_glyphs
                .get(*registered_glyph_index)
                .ok_or(NativeStemsHeadBuilderError::RegistryBaseline { system_id })?;
            if *registered_glyph_index != checked
                || glyph.source_ordinal != gate.ordinal
                || *accepted != free_glyph_index.is_some()
            {
                return Err(NativeStemsHeadBuilderError::RegistryBaseline { system_id });
            }
            if let Some(free_glyph_index) = free_glyph_index {
                if *free_glyph_index != accepted_checked {
                    return Err(NativeStemsHeadBuilderError::RegistryBaseline { system_id });
                }
                let free = context
                    .seed_system
                    .free_glyphs
                    .get(*free_glyph_index)
                    .ok_or(NativeStemsHeadBuilderError::RegistryBaseline { system_id })?;
                if !glyph.vertical_seed_group
                    || !glyph.free
                    || free.bounds != glyph.bounds
                    || free.weight != glyph.weight
                    || free.run_table != glyph.run_table
                {
                    return Err(NativeStemsHeadBuilderError::RegistryBaseline { system_id });
                }
                accepted_checked += 1;
            } else if glyph.vertical_seed_group || glyph.free {
                return Err(NativeStemsHeadBuilderError::RegistryBaseline { system_id });
            }
            checked += 1;
        }
    }
    if checked != context.seed_system.registered_glyphs.len()
        || accepted_checked != context.seed_system.free_glyphs.len()
    {
        return Err(NativeStemsHeadBuilderError::RegistryBaseline { system_id });
    }
    if context.reach_system.c_builder_assignment_count != 0
        || context.reach_system.c_inspection_order.len() != context.reach_system.heads.len() * 4
        || context.reach_system.c_construction_order.len() != context.reach_system.heads.len() * 4
        || context.reach_system.profile != context.seed_system.raw.profile
        || context.reach_system.profile != context.beam_stump_system.profile
        || context.reach_system.profile != context.beam_v_system.profile
        || context.reach_system.interline != context.seed_system.raw.interline
        || context.beam_stump_system.interline != context.seed_system.raw.interline
        || context.beam_v_system.interline != context.seed_system.raw.interline
        || context.beam_builder_system.interline != context.seed_system.raw.interline
        || context.beam_builder_system.max_stem_thickness
            != context.seed_system.raw.maximum_stem_thickness
    {
        return Err(NativeStemsHeadBuilderError::ReachabilityInvariant {
            system_id,
            corner: None,
            phase: "system parameters",
        });
    }
    for (x_ordinal, reach_head) in context.reach_system.heads.iter().enumerate() {
        let source = staff_head(context.head_system, reach_head.reference).ok_or(
            NativeStemsHeadBuilderError::HeadProvenance {
                system_id,
                head: reach_head.reference,
            },
        )?;
        let source_staff = context
            .head_system
            .staffs
            .get(reach_head.reference.staff_index)
            .ok_or(NativeStemsHeadBuilderError::HeadProvenance {
                system_id,
                head: reach_head.reference,
            })?;
        if reach_head.x_ordinal != x_ordinal
            || context
                .reach_system
                .head_sig_ordinals_by_abscissa
                .get(x_ordinal)
                != Some(&reach_head.sig_ordinal)
            || source_staff.staff_id != reach_head.staff_id
        {
            return Err(NativeStemsHeadBuilderError::HeadProvenance {
                system_id,
                head: reach_head.reference,
            });
        }
        if source.shape != reach_head.shape
            || source.bounds != reach_head.bounds
            || source.attachment_ordinal.is_none()
            || source.duplicate_removed
        {
            return Err(NativeStemsHeadBuilderError::HeadProvenance {
                system_id,
                head: reach_head.reference,
            });
        }
        let glyph = source_head_glyph(context, reach_head.reference, source)?;
        if source.glyph_bounds.x != glyph.glyph_bounds.x
            || source.glyph_bounds.y != glyph.glyph_bounds.y
            || source.glyph_bounds.width != glyph.glyph_bounds.width
            || source.glyph_bounds.height != glyph.glyph_bounds.height
            || source.glyph_weight != glyph.weight
            || source.glyph_run_digest != glyph.run_digest
        {
            return Err(NativeStemsHeadBuilderError::HeadProvenance {
                system_id,
                head: reach_head.reference,
            });
        }
        for (head_inspection_ordinal, corner) in reach_head.corners.iter().enumerate() {
            if corner.c_builder_assigned
                || !corner.c_seeds_assigned
                || corner.reference.head != reach_head.reference
                || corner.inspection_ordinal != (x_ordinal * 4) + head_inspection_ordinal
                || context
                    .reach_system
                    .c_inspection_order
                    .get(corner.inspection_ordinal)
                    != Some(&corner.reference)
                || context
                    .reach_system
                    .c_construction_order
                    .get((x_ordinal * 4) + corner.constructor_ordinal)
                    != Some(&corner.reference)
            {
                return Err(NativeStemsHeadBuilderError::ReachabilityInvariant {
                    system_id,
                    corner: Some(corner.reference),
                    phase: "corner order",
                });
            }
            validate_seed_retrieval(system_id, corner)?;
            let mut seen_beam = false;
            let mut head_targets = Vec::new();
            let mut beam_targets = Vec::new();
            for target in &corner.ordered_targets {
                match target {
                    NativeStemsHeadReachabilityTarget::Head(_) if seen_beam => {
                        return Err(NativeStemsHeadBuilderError::ReachabilityInvariant {
                            system_id,
                            corner: Some(corner.reference),
                            phase: "target kind order",
                        });
                    }
                    NativeStemsHeadReachabilityTarget::Beam(reference) => {
                        seen_beam = true;
                        beam_targets.push(*reference);
                    }
                    NativeStemsHeadReachabilityTarget::Head(reference) => {
                        head_targets.push(*reference);
                    }
                }
            }
            if head_targets != corner.head_targets || beam_targets != corner.beam_targets {
                return Err(NativeStemsHeadBuilderError::ReachabilityInvariant {
                    system_id,
                    corner: Some(corner.reference),
                    phase: "target source order",
                });
            }
        }
    }
    Ok(())
}

fn validate_seed_retrieval(
    system_id: usize,
    corner: &NativeStemsHeadReachabilityCorner,
) -> Result<(), NativeStemsHeadBuilderError> {
    if corner.seed_scans.iter().any(|scan| {
        let preliminary = matches!(
            scan.action,
            crate::native_stems_head_corner_reachability::NativeStemsHeadSeedScanAction::Preliminary
        );
        preliminary
            != (scan.preliminary_ordinal.is_some()
                && scan.sorted_preliminary_ordinal.is_some()
                && scan.contribution.is_some())
    }) {
        return Err(NativeStemsHeadBuilderError::ReachabilityInvariant {
            system_id,
            corner: Some(corner.reference),
            phase: "retrieveSeeds source fields",
        });
    }
    let mut preliminary = corner
        .seed_scans
        .iter()
        .filter_map(|scan| {
            scan.preliminary_ordinal.map(|ordinal| {
                (
                    ordinal,
                    scan.free_glyph_ordinal,
                    scan.contribution,
                    scan.sorted_preliminary_ordinal,
                )
            })
        })
        .collect::<Vec<_>>();
    preliminary.sort_by_key(|row| row.0);
    if preliminary
        .iter()
        .enumerate()
        .any(|(ordinal, row)| row.0 != ordinal || row.2.is_none() || row.3.is_none())
    {
        return Err(NativeStemsHeadBuilderError::ReachabilityInvariant {
            system_id,
            corner: Some(corner.reference),
            phase: "retrieveSeeds source order",
        });
    }
    // The retrieveSeeds comparator is contribution-only and stable. Replay
    // the complete pre-sort occurrence sequence before trusting assignments.
    let mut sorted = preliminary.clone();
    if !jdk25_sort_seed_preliminary(&mut sorted) {
        return Err(NativeStemsHeadBuilderError::JdkTimSortContractViolation {
            system_id,
            phase: "retrieveSeeds",
            length: preliminary.len(),
        });
    }
    if sorted
        .iter()
        .enumerate()
        .any(|(ordinal, row)| row.3 != Some(ordinal))
    {
        return Err(NativeStemsHeadBuilderError::ReachabilityInvariant {
            system_id,
            corner: Some(corner.reference),
            phase: "retrieveSeeds sort permutation",
        });
    }
    if corner.seed_overlap_decisions.len() != sorted.len()
        || corner
            .seed_overlap_decisions
            .iter()
            .zip(&sorted)
            .enumerate()
            .any(|(ordinal, (decision, source))| {
                decision.sorted_preliminary_ordinal != ordinal
                    || decision.free_glyph_ordinal != source.1
                    || decision.contribution != source.2.expect("validated contribution")
            })
    {
        return Err(NativeStemsHeadBuilderError::ReachabilityInvariant {
            system_id,
            corner: Some(corner.reference),
            phase: "retrieveSeeds overlap rows",
        });
    }
    let mut overlap_kept = Vec::<(usize, Bounds)>::new();
    for (decision, source) in corner.seed_overlap_decisions.iter().zip(&sorted) {
        let bounds = corner
            .seed_scans
            .iter()
            .find(|scan| {
                scan.free_glyph_ordinal == source.1
                    && scan.sorted_preliminary_ordinal == Some(decision.sorted_preliminary_ordinal)
            })
            .map(|scan| scan.bounds)
            .ok_or(NativeStemsHeadBuilderError::ReachabilityInvariant {
                system_id,
                corner: Some(corner.reference),
                phase: "retrieveSeeds overlap source",
            })?;
        let first_overlap = overlap_kept
            .iter()
            .find(|(_, kept_bounds)| y_overlap_bounds(bounds, *kept_bounds) > 0)
            .map(|(ordinal, _)| *ordinal);
        let expected_action = if first_overlap.is_some() {
            crate::native_stems_head_corner_reachability::NativeStemsHeadSeedOverlapAction::RejectedOverlap
        } else {
            overlap_kept.push((decision.free_glyph_ordinal, bounds));
            crate::native_stems_head_corner_reachability::NativeStemsHeadSeedOverlapAction::Kept
        };
        if decision.first_overlapping_kept_seed != first_overlap
            || decision.action != expected_action
        {
            return Err(NativeStemsHeadBuilderError::ReachabilityInvariant {
                system_id,
                corner: Some(corner.reference),
                phase: "retrieveSeeds overlap replay",
            });
        }
    }
    let kept = corner
        .seed_overlap_decisions
        .iter()
        .filter_map(|decision| {
            matches!(decision.action,
            crate::native_stems_head_corner_reachability::NativeStemsHeadSeedOverlapAction::Kept)
            .then_some(decision.free_glyph_ordinal)
        })
        .collect::<Vec<_>>();
    if kept != corner.assigned_seed_ordinals {
        return Err(NativeStemsHeadBuilderError::ReachabilityInvariant {
            system_id,
            corner: Some(corner.reference),
            phase: "retrieveSeeds assignment",
        });
    }
    Ok(())
}

type SeedValidationRow = (usize, usize, Option<i32>, Option<usize>);

fn jdk25_sort_seed_preliminary(values: &mut [SeedValidationRow]) -> bool {
    crate::jdk25_timsort::sort_by(values, |left, right| {
        right
            .2
            .expect("validated contribution")
            .cmp(&left.2.expect("validated contribution"))
    })
}

#[allow(clippy::too_many_arguments)]
fn materialize_builder(
    context: &SystemContext<'_>,
    head: &NativeStemsHeadReachabilityHead,
    corner: &NativeStemsHeadReachabilityCorner,
    builder_ordinal: usize,
    registry: &mut GlyphRegistry,
    registry_events: &mut Vec<NativeStemsHeadBuilderRegistryEvent>,
) -> Result<NativeStemsHeadBuilder, NativeStemsHeadBuilderError> {
    let system_id = context.system_id;
    if builder_ordinal != corner.inspection_ordinal {
        return Err(NativeStemsHeadBuilderError::ReachabilityInvariant {
            system_id,
            corner: Some(corner.reference),
            phase: "builder inspection ordinal",
        });
    }
    let source_head = staff_head(context.head_system, head.reference).ok_or(
        NativeStemsHeadBuilderError::HeadProvenance {
            system_id,
            head: head.reference,
        },
    )?;
    let head_glyph = source_head_glyph(context, head.reference, source_head)?;
    let head_bounds = glyph_bounds(head_glyph)?;
    if head_bounds
        != JavaRectangle::new(
            source_head.glyph_bounds.x,
            source_head.glyph_bounds.y,
            source_head.glyph_bounds.width,
            source_head.glyph_bounds.height,
        )
    {
        return Err(NativeStemsHeadBuilderError::HeadProvenance {
            system_id,
            head: head.reference,
        });
    }
    let y_direction = stem_builder_y_direction(corner.theoretical_line);
    // `CLinker.yRange` (used by retrieveSeeds upstream) is independently
    // rounded. The StemBuilder constructor instead calls
    // `theoLine.getBounds()` and uses that rectangle for all item contribs.
    let builder_y_range = line_bounds(corner.theoretical_line);
    let start_stump = corner
        .stump
        .as_ref()
        .map(|_| resolve_head_stump(context, corner.reference, registry))
        .transpose()?;
    let start_stump_ref = start_stump.as_ref().map(|glyph| glyph.reference);

    let input_seed_ordinals = corner.assigned_seed_ordinals.clone();
    let mut seeds = input_seed_ordinals
        .iter()
        .copied()
        .map(|ordinal| resolve_seed(context, ordinal, registry))
        .collect::<Result<Vec<_>, _>>()?;

    let mut alignment = Vec::new();
    let (seed_alignment, removed_seed_content) = filter_unaligned(
        &mut seeds,
        start_stump.as_ref(),
        y_direction,
        &context.skew,
        context.beam_builder_system.max_stem_alignment_dx,
        context.beam_builder_system.max_stem_alignment_dy,
        NativeStemsHeadBuilderAlignmentSubject::Seeds,
    );
    alignment.push(seed_alignment);

    let target_input = corner
        .ordered_targets
        .iter()
        .copied()
        .map(Into::into)
        .collect::<Vec<_>>();
    let mut target_states = target_input
        .iter()
        .copied()
        .map(|target| resolve_target(context, target, registry))
        .collect::<Result<Vec<_>, _>>()?;
    let mut target_filter = Vec::with_capacity(target_states.len());
    target_states.retain(|state| {
        let removed = state.stump.as_ref().is_some_and(|stump| {
            removed_seed_content
                .iter()
                .any(|key| same_content(stump, key))
        });
        target_filter.push(NativeStemsHeadBuilderTargetDecision {
            target: state.target,
            stump: state.stump.as_ref().map(|glyph| glyph.reference),
            removed_by_structural_seed: removed,
            included: !removed,
        });
        !removed
    });
    let mut target_items = target_states
        .iter()
        .map(|state| target_item(state, builder_y_range))
        .collect::<Vec<_>>();
    let target_before_sort = target_items.clone();
    stable_sort_items(&mut target_items, y_direction, system_id, "targets")?;
    let target_sort = sort_permutation(&target_before_sort, &target_items, y_direction);
    // Java only computes this field for a V-linker start.
    let last_head_y = None;

    let mut seed_filter = Vec::new();
    for removed in &removed_seed_content {
        seed_filter.push(NativeStemsHeadBuilderSeedDecision {
            glyph: removed.reference,
            retained_in_mutable_seed_set: false,
            included_as_item: false,
            action: NativeStemsHeadBuilderSeedAction::AlignmentRemoved,
            contribution: None,
        });
    }
    let mut seed_items = Vec::new();
    for glyph in &seeds {
        let duplicate = target_states.iter().any(|state| {
            state
                .stump
                .as_ref()
                .is_some_and(|stump| same_identity(glyph, stump))
        });
        if duplicate {
            seed_filter.push(seed_decision(
                glyph,
                false,
                NativeStemsHeadBuilderSeedAction::DuplicateTargetIdentity,
                None,
            ));
            continue;
        }
        if start_stump
            .as_ref()
            .is_some_and(|start| y_overlap_bounds(start.bounds, glyph.bounds) > 0)
        {
            seed_filter.push(seed_decision(
                glyph,
                false,
                NativeStemsHeadBuilderSeedAction::OverlapsStart,
                None,
            ));
            continue;
        }
        let contrib = contribution(builder_y_range, glyph.bounds);
        if contrib <= 0 {
            seed_filter.push(seed_decision(
                glyph,
                false,
                NativeStemsHeadBuilderSeedAction::ZeroContribution,
                Some(contrib),
            ));
            continue;
        }
        seed_filter.push(seed_decision(
            glyph,
            true,
            NativeStemsHeadBuilderSeedAction::Item,
            Some(contrib),
        ));
        seed_items.push(glyph_item(
            NativeStemsHeadBuilderItemKind::SeedGlyph,
            glyph,
            contrib,
        ));
    }

    let (vertical_sections, selected_vertical) =
        scan_vertical_sections(context, corner, start_stump.as_ref())?;
    let (horizontal_sections, selected_horizontal) = scan_horizontal_sections(context, corner)?;
    let factory = VerticalStickFactory::new(VerticalStickParameters {
        interline: usize::try_from(context.seed_system.raw.interline)
            .map_err(|_| NativeStemsHeadBuilderError::Geometry { system_id })?,
        maximum_stick_thickness: usize::try_from(context.beam_builder_system.max_stem_thickness)
            .map_err(|_| NativeStemsHeadBuilderError::Geometry { system_id })?,
        minimum_core_section_length: 0,
        minimum_side_ratio: MIN_SIDE_RATIO,
    });
    let outcome = factory.retrieve_sticks(
        &selected_vertical
            .iter()
            .map(|(_, section)| section.clone())
            .collect::<Vec<_>>(),
        &selected_horizontal
            .iter()
            .map(|(_, section)| section.clone())
            .collect::<Vec<_>>(),
        1,
    );
    if let Some(source) = outcome.error {
        return Err(NativeStemsHeadBuilderError::StickFactory { system_id, source });
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
        let geometry = filament
            .construction_geometry()
            .map_err(|source| NativeStemsHeadBuilderError::StickFactory { system_id, source })?;
        let reference = NativeStemsHeadBuilderGlyphRef::Chunk {
            builder_ordinal,
            filament_ordinal,
        };
        let mut glyph = glyph_from_sections(reference, filament.sections(), geometry)?;
        let before = registry.modeled_count();
        let registration = registry.register_parts(glyph.bounds, glyph.run_table.clone());
        glyph.modeled_canonical_ordinal = registration.modeled_canonical_ordinal;
        let after = registry.modeled_count();
        registrations.push(NativeStemsHeadBuilderGlyphRegistration {
            glyph: reference,
            bounds: glyph.bounds,
            weight: glyph.weight,
            run_table: glyph.run_table.clone(),
            modeled_canonical_ordinal: registration.modeled_canonical_ordinal,
            action: registration.action,
        });
        registry_events.push(NativeStemsHeadBuilderRegistryEvent {
            system_event_ordinal: registry_events.len(),
            occurrence: NativeStemsHeadBuilderRegistryOccurrence::HeadChunk {
                builder_ordinal,
                filament_ordinal,
            },
            bounds: glyph.bounds,
            weight: glyph.weight,
            run_table: glyph.run_table.clone(),
            modeled_canonical_ordinal: registration.modeled_canonical_ordinal,
            action: registration.action,
            isolated_beam_action: None,
            action_changed_from_isolated_beam: false,
            modeled_count_before: before,
            modeled_count_after: after,
        });
        filaments.push(NativeStemsHeadBuilderFilament {
            filament_ordinal,
            creation_id: *creation_id,
            member_section_source_ordinals: filament
                .sections()
                .iter()
                .map(|section| source_section_ref(context, section))
                .collect::<Result<Vec<_>, _>>()?,
            bounds: geometry.bounds,
            weight: filament.weight(),
            start: geometry.start,
            stop: geometry.stop,
            mean_thickness: geometry.mean_thickness,
            mean_distance: geometry.mean_distance,
        });
        chunk_work.push(glyph);
    }

    let mut chunks = chunk_work
        .iter()
        .map(|glyph| NativeStemsHeadBuilderChunk {
            glyph: glyph.reference,
            bounds: glyph.bounds,
            run_table: glyph.run_table.clone(),
            modeled_canonical_ordinal: glyph.modeled_canonical_ordinal,
            head_y_overlap: y_overlap_java(
                java_bounds(glyph.bounds).unwrap_or(JavaRectangle::new(0, 0, 0, 0)),
                head_bounds,
            ),
            head_pixels_removed: 0,
            remaining_weight: glyph.weight,
            head_parts_action: NativeStemsHeadBuilderHeadPartsAction::Keep,
            final_ordinal: None,
            action: NativeStemsHeadBuilderChunkAction::Keep,
        })
        .collect::<Vec<_>>();

    // `chunks.removeAll(seeds)`: all structurally equal occurrences.
    for (index, glyph) in chunk_work.iter().enumerate() {
        if seeds.iter().any(|seed| same_content(seed, glyph)) {
            chunks[index].action = NativeStemsHeadBuilderChunkAction::SeedStructural;
        }
    }
    // Preserve Java's VIP-only removal nesting, including the source bug that
    // keeps the same low-remain chunk for an otherwise identical non-VIP head.
    for (index, glyph) in chunk_work.iter().enumerate() {
        if chunks[index].action != NativeStemsHeadBuilderChunkAction::Keep {
            continue;
        }
        let removed = removed_head_pixels(glyph, head_glyph)?;
        let remain = glyph
            .weight
            .checked_sub(removed)
            .ok_or(NativeStemsHeadBuilderError::Geometry { system_id })?;
        chunks[index].head_pixels_removed = removed;
        chunks[index].remaining_weight = remain;
        let (head_parts_action, chunk_action) = head_parts_decision(source_head.is_vip, remain);
        chunks[index].head_parts_action = head_parts_action;
        chunks[index].action = chunk_action;
    }

    let mut active_chunks = chunk_work
        .iter()
        .enumerate()
        .filter(|(index, _)| chunks[*index].action == NativeStemsHeadBuilderChunkAction::Keep)
        .map(|(_, glyph)| glyph.clone())
        .collect::<Vec<_>>();
    let (chunk_alignment, removed_chunk_content) = filter_unaligned(
        &mut active_chunks,
        start_stump.as_ref(),
        y_direction,
        &context.skew,
        context.beam_builder_system.max_stem_alignment_dx,
        context.beam_builder_system.max_stem_alignment_dy,
        NativeStemsHeadBuilderAlignmentSubject::Chunks,
    );
    alignment.push(chunk_alignment);
    for (index, glyph) in chunk_work.iter().enumerate() {
        if chunks[index].action == NativeStemsHeadBuilderChunkAction::Keep
            && removed_chunk_content
                .iter()
                .any(|removed| same_content(removed, glyph))
        {
            chunks[index].action = NativeStemsHeadBuilderChunkAction::UnalignedStructural;
        }
    }
    if let Some(start) = &start_stump
        && let Some(index) = chunk_work.iter().enumerate().find_map(|(index, glyph)| {
            (chunks[index].action == NativeStemsHeadBuilderChunkAction::Keep
                && same_content(glyph, start))
            .then_some(index)
        })
    {
        chunks[index].action = NativeStemsHeadBuilderChunkAction::StartFirstStructural;
    }
    let mut kept_chunks = chunk_work
        .iter()
        .enumerate()
        .filter(|(index, _)| chunks[*index].action == NativeStemsHeadBuilderChunkAction::Keep)
        .map(|(index, glyph)| (index, glyph.clone()))
        .collect::<Vec<_>>();
    kept_chunks.sort_by(|(_, left), (_, right)| glyph_order(left, right, y_direction));
    for (final_ordinal, (index, _)) in kept_chunks.iter().enumerate() {
        chunks[*index].final_ordinal = Some(final_ordinal);
    }

    let mut items_before_sort = Vec::new();
    items_before_sort.push(start_item(
        corner.reference,
        start_stump.as_ref(),
        corner.reference_point,
        builder_y_range,
        head.bounds,
    ));
    items_before_sort.extend(target_items.iter().cloned());
    items_before_sort.extend(seed_items);
    items_before_sort.extend(kept_chunks.iter().map(|(_, glyph)| {
        glyph_item(
            NativeStemsHeadBuilderItemKind::ChunkGlyph,
            glyph,
            contribution(builder_y_range, glyph.bounds),
        )
    }));
    let creation_order_items = items_before_sort.clone();
    let mut sorted_tail = items_before_sort
        .iter()
        .skip(1)
        .cloned()
        .collect::<Vec<_>>();
    let tail_before_sort = sorted_tail.clone();
    stable_sort_items(&mut sorted_tail, y_direction, system_id, "items")?;
    let sort = sort_permutation(&tail_before_sort, &sorted_tail, y_direction);
    items_before_sort.splice(1.., sorted_tail);
    let mut items = items_before_sort.clone();
    let max_gap = *context
        .beam_builder_system
        .gap_map
        .get(&context.inspect_profile)
        .ok_or(NativeStemsHeadBuilderError::InvalidParameters { system_id })?;
    let gaps = insert_gaps(&mut items, y_direction, max_gap);
    let lengths = retrieve_lengths(
        &items,
        y_direction,
        corner.theoretical_line,
        &context.beam_builder_system.gap_map,
        context.inspect_profile,
        system_id,
    )?;

    Ok(NativeStemsHeadBuilder {
        builder_ordinal,
        c_builder_assignment: corner.reference,
        start: corner.reference,
        source_is_vip: source_head.is_vip,
        max_stem_profile: context.inspect_profile,
        c_y_direction: corner.y_direction,
        y_direction,
        theoretical_line: corner.theoretical_line,
        lookup_quadrilateral: corner.final_lookup.quadrilateral,
        lookup_bounds: corner.final_lookup.bounds,
        y_range: builder_y_range,
        start_stump: start_stump_ref,
        input_seed_ordinals,
        seeds_after_filter: seeds.iter().map(|glyph| glyph.reference).collect(),
        target_input,
        targets_after_filter: target_states.iter().map(|state| state.target).collect(),
        alignment,
        seed_filter,
        target_filter,
        target_sort,
        last_head_y,
        vertical_sections,
        horizontal_sections,
        filaments,
        glyph_registrations: registrations,
        chunks,
        items_before_sort: creation_order_items,
        sort,
        gaps,
        items,
        lengths,
        sig_mutation_count: 0,
        system_stem_mutation_count: 0,
        link_mutation_count: 0,
        beam_arena_mutation_count: 0,
    })
}

fn seed_decision(
    glyph: &FixedGlyph,
    included: bool,
    action: NativeStemsHeadBuilderSeedAction,
    contribution: Option<i32>,
) -> NativeStemsHeadBuilderSeedDecision {
    NativeStemsHeadBuilderSeedDecision {
        glyph: glyph.reference,
        retained_in_mutable_seed_set: true,
        included_as_item: included,
        action,
        contribution,
    }
}

fn head_parts_decision(
    source_is_vip: bool,
    remaining_weight: usize,
) -> (
    NativeStemsHeadBuilderHeadPartsAction,
    NativeStemsHeadBuilderChunkAction,
) {
    if remaining_weight >= HEAD_PART_MIN_REMAINING_WEIGHT {
        (
            NativeStemsHeadBuilderHeadPartsAction::Keep,
            NativeStemsHeadBuilderChunkAction::Keep,
        )
    } else if source_is_vip {
        (
            NativeStemsHeadBuilderHeadPartsAction::RemoveVipOnly,
            NativeStemsHeadBuilderChunkAction::HeadPartsVipOnly,
        )
    } else {
        (
            NativeStemsHeadBuilderHeadPartsAction::KeepNonVipJavaBehavior,
            NativeStemsHeadBuilderChunkAction::Keep,
        )
    }
}

fn staff_head(
    system: &NativeHeadsStaffEpilogSystem,
    reference: NativeHeadStaffEpilogRef,
) -> Option<&NativeHeadStaffEpilogHead> {
    system
        .staffs
        .get(reference.staff_index)?
        .heads
        .get(reference.head_index)
}

fn source_head_glyph<'a>(
    context: &'a SystemContext<'_>,
    reference: NativeHeadStaffEpilogRef,
    head: &NativeHeadStaffEpilogHead,
) -> Result<&'a RetrievedHeadGlyph, NativeStemsHeadBuilderError> {
    let system_id = context.system_id;
    let staff_id = context
        .head_system
        .staffs
        .get(reference.staff_index)
        .ok_or(NativeStemsHeadBuilderError::HeadProvenance {
            system_id,
            head: reference,
        })?
        .staff_id;
    match head.origin {
        NativeHeadStaffEpilogOrigin::Seed(ordinal) => context
            .heads
            .seed_glyphs
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .and_then(|system| {
                system
                    .staffs
                    .iter()
                    .find(|staff| staff.staff_id == staff_id)
            })
            .and_then(|staff| staff.heads.iter().find(|source| source.ordinal == ordinal))
            .map(|source| &source.glyph),
        NativeHeadStaffEpilogOrigin::Range(ordinal) => context
            .heads
            .range_glyphs
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .and_then(|system| {
                system
                    .staffs
                    .iter()
                    .find(|staff| staff.staff_id == staff_id)
            })
            .and_then(|staff| staff.heads.iter().find(|source| source.ordinal == ordinal))
            .map(|source| &source.glyph),
    }
    .ok_or(NativeStemsHeadBuilderError::HeadProvenance {
        system_id,
        head: reference,
    })
}

fn find_reach_corner(
    system: &NativeStemsHeadCornerReachabilitySystem,
    reference: NativeStemsHeadCornerRef,
) -> Option<(
    &NativeStemsHeadReachabilityHead,
    &NativeStemsHeadReachabilityCorner,
)> {
    let head = system.heads.iter().find(|head| {
        head.reference == reference.head
            && head.sig_ordinal == reference.sig_ordinal
            && head.x_ordinal == reference.x_ordinal
    })?;
    let corner = head
        .corners
        .iter()
        .find(|corner| corner.reference == reference)?;
    Some((head, corner))
}

fn resolve_seed(
    context: &SystemContext<'_>,
    free_glyph_ordinal: usize,
    registry: &GlyphRegistry,
) -> Result<FixedGlyph, NativeStemsHeadBuilderError> {
    let glyph = context
        .seed_system
        .free_glyphs
        .get(free_glyph_ordinal)
        .ok_or(NativeStemsHeadBuilderError::MissingSeed {
            system_id: context.system_id,
            free_glyph_ordinal,
        })?;
    fixed_glyph(
        NativeStemsHeadBuilderGlyphRef::StemSeed { free_glyph_ordinal },
        glyph.bounds,
        glyph.weight,
        glyph.run_table.clone(),
        registry,
        context.system_id,
    )
}

fn resolve_head_stump(
    context: &SystemContext<'_>,
    reference: NativeStemsHeadCornerRef,
    registry: &GlyphRegistry,
) -> Result<FixedGlyph, NativeStemsHeadBuilderError> {
    let (_, corner) = find_reach_corner(context.reach_system, reference).ok_or(
        NativeStemsHeadBuilderError::MissingStump {
            system_id: context.system_id,
        },
    )?;
    let stump = corner
        .stump
        .as_ref()
        .ok_or(NativeStemsHeadBuilderError::MissingStump {
            system_id: context.system_id,
        })?;
    let occurrence = NativeStemsHeadBuilderGlyphRef::HeadStump { corner: reference };
    match stump.source {
        NativeStemsHeadStumpRef::Seed { free_glyph_ordinal } => {
            let mut glyph = resolve_seed(context, free_glyph_ordinal, registry)?;
            if glyph.bounds != stump.bounds || glyph.weight != stump.weight {
                return Err(NativeStemsHeadBuilderError::ReachabilityInvariant {
                    system_id: context.system_id,
                    corner: Some(reference),
                    phase: "seed head stump provenance",
                });
            }
            glyph.reference = occurrence;
            Ok(glyph)
        }
        NativeStemsHeadStumpRef::Built {
            head_x_ordinal,
            constructor_ordinal,
        } => {
            if head_x_ordinal != reference.x_ordinal
                || constructor_ordinal != corner.constructor_ordinal
            {
                return Err(NativeStemsHeadBuilderError::ReachabilityInvariant {
                    system_id: context.system_id,
                    corner: Some(reference),
                    phase: "built head stump reference",
                });
            }
            let source = context
                .head_stump_system
                .heads_by_abscissa
                .iter()
                .find(|head| {
                    head.x_ordinal == head_x_ordinal && head.sig_ordinal == reference.sig_ordinal
                })
                .and_then(|head| {
                    head.corners_in_constructor_order
                        .iter()
                        .find(|corner| corner.constructor_ordinal == constructor_ordinal)
                })
                .and_then(|corner| corner.build.as_ref())
                .and_then(|build| build.candidate.as_ref())
                .ok_or(NativeStemsHeadBuilderError::MissingStump {
                    system_id: context.system_id,
                })?;
            if source.bounds != stump.bounds || source.weight != stump.weight {
                return Err(NativeStemsHeadBuilderError::ReachabilityInvariant {
                    system_id: context.system_id,
                    corner: Some(reference),
                    phase: "built head stump provenance",
                });
            }
            fixed_glyph(
                occurrence,
                source.bounds,
                source.weight,
                source.run_table.clone(),
                registry,
                context.system_id,
            )
        }
    }
}

fn resolve_target(
    context: &SystemContext<'_>,
    target: NativeStemsHeadBuilderTargetRef,
    registry: &GlyphRegistry,
) -> Result<TargetState, NativeStemsHeadBuilderError> {
    match target {
        NativeStemsHeadBuilderTargetRef::Head(reference) => {
            let (head, corner) = find_reach_corner(context.reach_system, reference).ok_or(
                NativeStemsHeadBuilderError::MissingTarget {
                    system_id: context.system_id,
                },
            )?;
            let stump = corner
                .stump
                .as_ref()
                .map(|_| resolve_head_stump(context, reference, registry))
                .transpose()?;
            Ok(TargetState {
                target,
                stump,
                reference_point: corner.reference_point,
                head_bounds: Some(head.bounds),
                beam_height: None,
            })
        }
        NativeStemsHeadBuilderTargetRef::Beam(reference) => {
            let entry = context
                .reach_system
                .final_beam_arenas
                .iter()
                .find(|arena| arena.beam == reference.beam)
                .and_then(|arena| {
                    arena
                        .entries
                        .iter()
                        .find(|entry| entry.reference == reference)
                })
                .ok_or(NativeStemsHeadBuilderError::MissingTarget {
                    system_id: context.system_id,
                })?;
            let beam = find_beam(context.beam_stump_system, reference.beam).ok_or(
                NativeStemsHeadBuilderError::MissingTarget {
                    system_id: context.system_id,
                },
            )?;
            let stump = find_b_linker(context.beam_v_system, reference)
                .and_then(|b| b.stump.as_ref())
                .map(|stump| resolve_beam_stump(context, reference, stump, registry))
                .transpose()?;
            Ok(TargetState {
                target,
                stump,
                reference_point: entry.reference_point,
                head_bounds: None,
                beam_height: Some(beam.height),
            })
        }
    }
}

fn resolve_beam_stump(
    context: &SystemContext<'_>,
    reference: NativeStemsBeamBLinkerRef,
    stump: &NativeStemsBeamStumpRef,
    registry: &GlyphRegistry,
) -> Result<FixedGlyph, NativeStemsHeadBuilderError> {
    let occurrence = NativeStemsHeadBuilderGlyphRef::BeamStump {
        b_linker: reference,
    };
    let source = match stump {
        NativeStemsBeamStumpRef::Seed {
            free_glyph_ordinal, ..
        } => {
            let mut glyph = resolve_seed(context, *free_glyph_ordinal, registry)?;
            glyph.reference = occurrence;
            return Ok(glyph);
        }
        NativeStemsBeamStumpRef::Built {
            canonical_glyph_index,
        } => find_beam(context.beam_stump_system, reference.beam)
            .and_then(|beam| {
                beam.sides
                    .iter()
                    .filter_map(|side| side.build.as_ref())
                    .find(|build| build.canonical_glyph_index == Some(*canonical_glyph_index))
            })
            .and_then(|build| build.candidate.as_ref())
            .ok_or(NativeStemsHeadBuilderError::MissingStump {
                system_id: context.system_id,
            })?,
    };
    fixed_glyph(
        occurrence,
        source.bounds,
        source.weight,
        source.run_table.clone(),
        registry,
        context.system_id,
    )
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

fn find_beam(
    system: &crate::native_stems_beam_stumps::NativeStemsBeamStumpSystem,
    source: NativeStemsBeamSource,
) -> Option<&NativeStemsBeamStumpBeam> {
    system
        .beams_by_abscissa
        .iter()
        .find(|beam| beam.source == source)
}

fn fixed_glyph(
    reference: NativeStemsHeadBuilderGlyphRef,
    bounds: Bounds,
    weight: usize,
    run_table: RunTable,
    registry: &GlyphRegistry,
    system_id: usize,
) -> Result<FixedGlyph, NativeStemsHeadBuilderError> {
    if run_table_weight(&run_table) != weight {
        return Err(NativeStemsHeadBuilderError::Geometry { system_id });
    }
    let modeled_canonical_ordinal = registry
        .find(&GlyphKey {
            bounds,
            run_table: run_table.clone(),
        })
        .ok_or(NativeStemsHeadBuilderError::RegistryBaseline { system_id })?;
    let left =
        i32::try_from(bounds.x).map_err(|_| NativeStemsHeadBuilderError::Geometry { system_id })?;
    let top =
        i32::try_from(bounds.y).map_err(|_| NativeStemsHeadBuilderError::Geometry { system_id })?;
    let line = run_table_center_line(&run_table, left, top)
        .ok_or(NativeStemsHeadBuilderError::Geometry { system_id })?;
    Ok(FixedGlyph {
        reference,
        bounds,
        weight,
        run_table,
        line,
        modeled_canonical_ordinal,
    })
}

fn target_item(state: &TargetState, y_range: JavaRectangle) -> NativeStemsHeadBuilderItem {
    match state.target {
        NativeStemsHeadBuilderTargetRef::Head(_) => {
            let line = state.stump.as_ref().map_or(
                NativeStemLine {
                    start: state.reference_point,
                    stop: state.reference_point,
                },
                |glyph| line_from_segment(glyph.line),
            );
            NativeStemsHeadBuilderItem {
                kind: NativeStemsHeadBuilderItemKind::HeadHalfLinker,
                glyph: state.stump.as_ref().map(|glyph| glyph.reference),
                target: Some(state.target),
                reference_point: Some(state.reference_point),
                head_bounds: state.head_bounds,
                line,
                contribution: state
                    .stump
                    .as_ref()
                    .map_or(0, |glyph| contribution(y_range, glyph.bounds)),
            }
        }
        NativeStemsHeadBuilderTargetRef::Beam(_) => {
            let line = state.stump.as_ref().map_or_else(
                || {
                    let half = state.beam_height.unwrap_or(0.0) / 2.0;
                    NativeStemLine {
                        start: NativeStemPoint {
                            x: state.reference_point.x,
                            y: state.reference_point.y - half,
                        },
                        stop: NativeStemPoint {
                            x: state.reference_point.x,
                            y: state.reference_point.y + half,
                        },
                    }
                },
                |glyph| line_from_segment(glyph.line),
            );
            NativeStemsHeadBuilderItem {
                kind: NativeStemsHeadBuilderItemKind::BeamLinker,
                glyph: state.stump.as_ref().map(|glyph| glyph.reference),
                target: Some(state.target),
                reference_point: None,
                head_bounds: None,
                line,
                contribution: state
                    .stump
                    .as_ref()
                    .map_or(0, |glyph| glyph.bounds.height as i32),
            }
        }
    }
}

fn start_item(
    _reference: NativeStemsHeadCornerRef,
    glyph: Option<&FixedGlyph>,
    reference_point: NativeStemPoint,
    y_range: JavaRectangle,
    head_bounds: JavaRectangle,
) -> NativeStemsHeadBuilderItem {
    NativeStemsHeadBuilderItem {
        kind: NativeStemsHeadBuilderItemKind::StartHeadHalfLinker,
        glyph: glyph.map(|glyph| glyph.reference),
        target: None,
        reference_point: Some(reference_point),
        // `HalfLinkerItem` extends `LinkerItem`; `getLengthAt` therefore adds
        // the source head bounds for the start C exactly as for target Cs.
        head_bounds: Some(head_bounds),
        line: glyph.map_or(
            NativeStemLine {
                start: reference_point,
                stop: reference_point,
            },
            |glyph| line_from_segment(glyph.line),
        ),
        contribution: glyph.map_or(0, |glyph| contribution(y_range, glyph.bounds)),
    }
}

fn glyph_item(
    kind: NativeStemsHeadBuilderItemKind,
    glyph: &FixedGlyph,
    contribution: i32,
) -> NativeStemsHeadBuilderItem {
    NativeStemsHeadBuilderItem {
        kind,
        glyph: Some(glyph.reference),
        target: None,
        reference_point: None,
        head_bounds: None,
        line: line_from_segment(glyph.line),
        contribution,
    }
}

type SectionScanResult = (
    Vec<NativeStemsHeadBuilderSectionScan>,
    Vec<(usize, Section)>,
);

fn scan_vertical_sections(
    context: &SystemContext<'_>,
    corner: &NativeStemsHeadReachabilityCorner,
    start: Option<&FixedGlyph>,
) -> Result<SectionScanResult, NativeStemsHeadBuilderError> {
    let mut scans = Vec::new();
    let mut selected = Vec::new();
    for (source_ordinal, &global_ordinal) in context
        .beam_builder_system
        .vertical_section_source_ordinals
        .iter()
        .enumerate()
    {
        let section = context
            .grid
            .peak_graph
            .vertical_sections
            .get(global_ordinal)
            .ok_or(NativeStemsHeadBuilderError::Geometry {
                system_id: context.system_id,
            })?;
        let bounds = section.bounds();
        let intersects_lookup = convex_quad_intersects_rectangle(
            corner.final_lookup.quadrilateral,
            java_bounds(bounds).ok_or(NativeStemsHeadBuilderError::Geometry {
                system_id: context.system_id,
            })?,
        );
        let mut scan = NativeStemsHeadBuilderSectionScan {
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
            .is_ok_and(|width| width <= context.beam_builder_system.max_stem_thickness);
        scan.width_accepted = Some(width_accepted);
        if !width_accepted {
            scans.push(scan);
            continue;
        }
        if let Some(start) = start
            && y_overlap_bounds(bounds, start.bounds) > 0
        {
            let accepted = bounds.height >= start.bounds.height;
            scan.stump_overlap_accepted = Some(accepted);
            if !accepted {
                scans.push(scan);
                continue;
            }
        }
        // `lastHeadY` is null for a C start, so Java skips this predicate.
        let distance = line_pt_distance(corner.theoretical_line, section.centroid_2d());
        let accepted = java_passes_greater_than_reject(
            distance,
            context.beam_builder_system.max_line_section_dx,
        );
        scan.line_distance = Some(distance);
        scan.distance_accepted = Some(accepted);
        scan.accepted = accepted;
        if accepted {
            selected.push((source_ordinal, section.clone()));
        }
        scans.push(scan);
    }
    selected.sort_by(|(_, left), (_, right)| section_full_position_cmp(left, right));
    for (sorted, (source, _)) in selected.iter().enumerate() {
        scans[*source].accepted_sorted_ordinal = Some(sorted);
    }
    Ok((scans, selected))
}

fn scan_horizontal_sections(
    context: &SystemContext<'_>,
    corner: &NativeStemsHeadReachabilityCorner,
) -> Result<SectionScanResult, NativeStemsHeadBuilderError> {
    let mut scans = Vec::new();
    let mut selected = Vec::new();
    for (source_ordinal, &global_ordinal) in context
        .beam_builder_system
        .horizontal_section_source_ordinals
        .iter()
        .enumerate()
    {
        let section = context
            .grid
            .peak_graph
            .horizontal_sections
            .get(global_ordinal)
            .ok_or(NativeStemsHeadBuilderError::Geometry {
                system_id: context.system_id,
            })?;
        let bounds = section.bounds();
        let intersects_lookup = convex_quad_intersects_rectangle(
            corner.final_lookup.quadrilateral,
            java_bounds(bounds).ok_or(NativeStemsHeadBuilderError::Geometry {
                system_id: context.system_id,
            })?,
        );
        let mut scan = NativeStemsHeadBuilderSectionScan {
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
        scan.accepted = true;
        selected.push((source_ordinal, section.clone()));
        scans.push(scan);
    }
    selected.sort_by(|(_, left), (_, right)| section_full_position_cmp(left, right));
    for (sorted, (source, _)) in selected.iter().enumerate() {
        scans[*source].accepted_sorted_ordinal = Some(sorted);
    }
    Ok((scans, selected))
}

fn filter_unaligned(
    glyphs: &mut Vec<FixedGlyph>,
    start: Option<&FixedGlyph>,
    y_direction: i32,
    skew: &HeadlessSkew,
    max_dx: f64,
    max_dy: f64,
    subject: NativeStemsHeadBuilderAlignmentSubject,
) -> (NativeStemsHeadBuilderAlignmentPass, Vec<FixedGlyph>) {
    let mut ordered = glyphs.clone();
    ordered.sort_by(|left, right| glyph_order(left, right, y_direction));
    if let Some(start) = start {
        if let Some(index) = ordered.iter().position(|glyph| same_content(glyph, start)) {
            ordered.remove(index);
        }
        ordered.insert(0, start.clone());
    }
    let sorted_before = ordered.iter().map(|glyph| glyph.reference).collect();
    let mut comparisons = Vec::new();
    let mut removed_content = Vec::new();
    let mut index = 0_usize;
    while index + 1 < ordered.len() {
        let first = ordered[index].clone();
        let second = ordered[index + 1].clone();
        let first_deskewed = deskew(skew, glyph_centroid(&first));
        let second_deskewed = deskew(skew, glyph_centroid(&second));
        let dy = (second_deskewed.1 - first_deskewed.1).abs();
        let bypass = dy > max_dy;
        let dx = (!bypass).then(|| (second_deskewed.0 - first_deskewed.0).abs());
        let aligned = bypass || dx.is_some_and(|dx| dx <= max_dx);
        let selected = if aligned {
            None
        } else if first.bounds.height < second.bounds.height {
            Some(first.clone())
        } else {
            Some(second.clone())
        };
        let actual_index = selected
            .as_ref()
            .and_then(|alien| ordered.iter().position(|glyph| same_content(glyph, alien)));
        let actual_removed_occurrence = actual_index.map(|position| ordered[position].reference);
        comparisons.push(NativeStemsHeadBuilderAlignmentCheck {
            first: first.reference,
            second: second.reference,
            first_deskewed,
            second_deskewed,
            dx,
            dy,
            dy_bypasses_dx: bypass,
            aligned,
            selected_alien: selected.as_ref().map(|glyph| glyph.reference),
            actual_removed_occurrence,
            equal_height_removed_second: !aligned && first.bounds.height == second.bounds.height,
        });
        if let Some(position) = actual_index {
            removed_content.push(ordered.remove(position));
            // Java decrements the loop index after removal; net unchanged.
        } else {
            index += 1;
        }
    }
    glyphs.retain(|glyph| {
        !removed_content
            .iter()
            .any(|removed| same_content(glyph, removed))
    });
    let removed_structural_keys = removed_content
        .iter()
        .map(|glyph| glyph.reference)
        .collect();
    (
        NativeStemsHeadBuilderAlignmentPass {
            subject,
            sorted_before,
            comparisons,
            removed_structural_keys,
            retained_occurrences: glyphs.iter().map(|glyph| glyph.reference).collect(),
        },
        removed_content,
    )
}

fn glyph_order(left: &FixedGlyph, right: &FixedGlyph, y_direction: i32) -> Ordering {
    if y_direction > 0 {
        left.bounds.y.cmp(&right.bounds.y)
    } else {
        (right.bounds.y + right.bounds.height).cmp(&(left.bounds.y + left.bounds.height))
    }
}

fn same_content(left: &FixedGlyph, right: &FixedGlyph) -> bool {
    left.bounds == right.bounds && left.run_table == right.run_table
}

fn same_identity(left: &FixedGlyph, right: &FixedGlyph) -> bool {
    left.modeled_canonical_ordinal == right.modeled_canonical_ordinal
}

fn glyph_centroid(glyph: &FixedGlyph) -> (f64, f64) {
    let mut count = 0_usize;
    let mut x_total = 0_f64;
    let mut y_total = 0_f64;
    // `RunTable.cumulate` materializes sequences/runs forward and coordinates
    // within each run backward. `computeCentroidDouble` then traverses that
    // point list backward, hence this reverse/reverse/forward order.
    for sequence in (0..glyph.run_table.sequence_count()).rev() {
        for run in glyph
            .run_table
            .sequence(sequence)
            .unwrap_or_default()
            .iter()
            .rev()
        {
            for coordinate in run.start..=run.stop() {
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
    let value = skew.deskewed(audiveris_image::staff_peak::PeakPoint::new(
        point.0, point.1,
    ));
    (value.x, value.y)
}

fn stable_sort_items(
    items: &mut [NativeStemsHeadBuilderItem],
    y_direction: i32,
    system_id: usize,
    phase: &'static str,
) -> Result<(), NativeStemsHeadBuilderError> {
    // StemBuilder now assigns each item one context-independent ordinate
    // before comparing it. Preserve Java's stable TimSort control flow while
    // avoiding the former mixed half-linker comparator cycle.
    crate::jdk25_timsort::sort_by(items, |left, right| item_cmp(left, right, y_direction))
        .then_some(())
        .ok_or(NativeStemsHeadBuilderError::JdkTimSortContractViolation {
            system_id,
            phase,
            length: items.len(),
        })
}

fn item_cmp(
    left: &NativeStemsHeadBuilderItem,
    right: &NativeStemsHeadBuilderItem,
    y_direction: i32,
) -> Ordering {
    let left = item_ordinate_key(left, y_direction);
    let right = item_ordinate_key(right, y_direction);
    (y_direction * java_double_compare(left, right)).cmp(&0)
}

fn item_ordinate_key(item: &NativeStemsHeadBuilderItem, y_direction: i32) -> f64 {
    if matches!(
        item.kind,
        NativeStemsHeadBuilderItemKind::StartHeadHalfLinker
            | NativeStemsHeadBuilderItemKind::HeadHalfLinker
    ) {
        item.reference_point
            .map_or(item.line.start.y, |point| point.y)
    } else if y_direction > 0 {
        item.line.start.y
    } else {
        item.line.stop.y
    }
}

fn sort_permutation(
    before: &[NativeStemsHeadBuilderItem],
    after: &[NativeStemsHeadBuilderItem],
    y_direction: i32,
) -> Vec<NativeStemsHeadBuilderSortEntry> {
    let mut used = vec![false; before.len()];
    after
        .iter()
        .enumerate()
        .map(|(after_ordinal, item)| {
            let before_ordinal = before
                .iter()
                .enumerate()
                .find_map(|(ordinal, candidate)| {
                    (!used[ordinal] && item_occurrence_eq(candidate, item)).then_some(ordinal)
                })
                .unwrap_or(after_ordinal);
            if before_ordinal < used.len() {
                used[before_ordinal] = true;
            }
            NativeStemsHeadBuilderSortEntry {
                before_ordinal,
                after_ordinal,
                item: item.clone(),
                tied_with_previous: after_ordinal > 0
                    && item_cmp(&after[after_ordinal - 1], item, y_direction) == Ordering::Equal,
            }
        })
        .collect()
}

fn item_occurrence_eq(
    left: &NativeStemsHeadBuilderItem,
    right: &NativeStemsHeadBuilderItem,
) -> bool {
    let point_eq = |left: NativeStemPoint, right: NativeStemPoint| {
        left.x.to_bits() == right.x.to_bits() && left.y.to_bits() == right.y.to_bits()
    };
    left.kind == right.kind
        && left.glyph == right.glyph
        && left.target == right.target
        && match (left.reference_point, right.reference_point) {
            (Some(left), Some(right)) => point_eq(left, right),
            (None, None) => true,
            _ => false,
        }
        && left.head_bounds == right.head_bounds
        && point_eq(left.line.start, right.line.start)
        && point_eq(left.line.stop, right.line.stop)
        && left.contribution == right.contribution
}

fn insert_gaps(
    items: &mut Vec<NativeStemsHeadBuilderItem>,
    y_direction: i32,
    max_gap: i32,
) -> Vec<NativeStemsHeadBuilderGapEvent> {
    let mut events = Vec::new();
    let mut last: Option<NativeStemPoint> = None;
    let mut index = 0_usize;
    while index < items.len() {
        let start = if y_direction > 0 {
            items[index].line.start
        } else {
            items[index].line.stop
        };
        let stop = if y_direction > 0 {
            items[index].line.stop
        } else {
            items[index].line.start
        };
        if let Some(last_point) = last {
            let gap = f64::from(y_direction) * (start.y - last_point.y);
            if gap > f64::from(max_gap) {
                events.push(NativeStemsHeadBuilderGapEvent {
                    item_ordinal_before_insert: index,
                    previous_stop: last_point,
                    next_start: start,
                    gap,
                    max_gap,
                    action: NativeStemsHeadBuilderGapAction::Truncated,
                });
                items.truncate(index);
                break;
            } else if gap > 0.01 {
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
                events.push(NativeStemsHeadBuilderGapEvent {
                    item_ordinal_before_insert: index,
                    previous_stop: last_point,
                    next_start: start,
                    gap,
                    max_gap,
                    action: NativeStemsHeadBuilderGapAction::Inserted,
                });
                items.insert(
                    index,
                    NativeStemsHeadBuilderItem {
                        kind: NativeStemsHeadBuilderItemKind::Gap,
                        glyph: None,
                        target: None,
                        reference_point: None,
                        head_bounds: None,
                        line,
                        contribution: line_bounds(line).height,
                    },
                );
                index += 1;
            } else {
                events.push(NativeStemsHeadBuilderGapEvent {
                    item_ordinal_before_insert: index,
                    previous_stop: last_point,
                    next_start: start,
                    gap,
                    max_gap,
                    action: NativeStemsHeadBuilderGapAction::None,
                });
            }
        }
        if last
            .is_none_or(|point: NativeStemPoint| f64::from(y_direction) * (stop.y - point.y) > 0.01)
        {
            last = Some(stop);
        }
        index += 1;
    }
    events
}

fn retrieve_lengths(
    items: &[NativeStemsHeadBuilderItem],
    y_direction: i32,
    theoretical_line: NativeStemLine,
    gap_map: &BTreeMap<i32, i32>,
    max_profile: i32,
    system_id: usize,
) -> Result<BTreeMap<i32, i32>, NativeStemsHeadBuilderError> {
    let max_gap = *gap_map
        .get(&max_profile)
        .ok_or(NativeStemsHeadBuilderError::InvalidParameters { system_id })?;
    let mut lengths = BTreeMap::new();
    for (index, item) in items.iter().enumerate() {
        if item.kind != NativeStemsHeadBuilderItemKind::Gap {
            continue;
        }
        for (&profile, &threshold) in gap_map {
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
    items: &[NativeStemsHeadBuilderItem],
    last: usize,
    y_direction: i32,
    theoretical_line: NativeStemLine,
) -> i32 {
    let mut bounds = None;
    for item in items.iter().take(last.saturating_add(1)) {
        if item.kind == NativeStemsHeadBuilderItemKind::Gap {
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

fn glyph_from_sections(
    reference: NativeStemsHeadBuilderGlyphRef,
    sections: &[Section],
    geometry: audiveris_image::stick_factory::StraightStickGeometry,
) -> Result<FixedGlyph, NativeStemsHeadBuilderError> {
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
    let left = i32::try_from(bounds.x)
        .map_err(|_| NativeStemsHeadBuilderError::Geometry { system_id: 0 })?;
    let top = i32::try_from(bounds.y)
        .map_err(|_| NativeStemsHeadBuilderError::Geometry { system_id: 0 })?;
    let line = run_table_center_line(&run_table, left, top)
        .ok_or(NativeStemsHeadBuilderError::Geometry { system_id: 0 })?;
    Ok(FixedGlyph {
        reference,
        bounds,
        weight: pixels.iter().filter(|&&pixel| pixel == FOREGROUND).count(),
        run_table,
        line,
        modeled_canonical_ordinal: usize::MAX,
    })
}

fn paint_section(
    pixels: &mut [u8],
    bounds: Bounds,
    section: &Section,
) -> Result<(), NativeStemsHeadBuilderError> {
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

fn source_section_ref(
    context: &SystemContext<'_>,
    section: &Section,
) -> Result<NativeStemsHeadBuilderSectionRef, NativeStemsHeadBuilderError> {
    let (orientation, ordinals, all) = match section.orientation() {
        Orientation::Vertical => (
            Orientation::Vertical,
            &context.beam_builder_system.vertical_section_source_ordinals,
            &context.grid.peak_graph.vertical_sections,
        ),
        Orientation::Horizontal => (
            Orientation::Horizontal,
            &context
                .beam_builder_system
                .horizontal_section_source_ordinals,
            &context.grid.peak_graph.horizontal_sections,
        ),
    };
    let source_ordinal = ordinals
        .iter()
        .position(|&global| all.get(global) == Some(section))
        .ok_or(NativeStemsHeadBuilderError::Geometry {
            system_id: context.system_id,
        })?;
    Ok(NativeStemsHeadBuilderSectionRef {
        orientation,
        source_ordinal,
    })
}

fn removed_head_pixels(
    chunk: &FixedGlyph,
    head: &RetrievedHeadGlyph,
) -> Result<usize, NativeStemsHeadBuilderError> {
    let head_bounds = glyph_bounds(head)?;
    let chunk_bounds =
        java_bounds(chunk.bounds).ok_or(NativeStemsHeadBuilderError::Geometry { system_id: 0 })?;
    if y_overlap_java(chunk_bounds, head_bounds) <= 0 {
        return Ok(0);
    }
    let mut removed = 0_usize;
    for y in 0..chunk.run_table.height() {
        for x in 0..chunk.run_table.width() {
            if chunk.run_table.get(x, y) != FOREGROUND {
                continue;
            }
            let absolute_x = chunk
                .bounds
                .x
                .checked_add(x)
                .ok_or(NativeStemsHeadBuilderError::Geometry { system_id: 0 })?;
            let absolute_y = chunk
                .bounds
                .y
                .checked_add(y)
                .ok_or(NativeStemsHeadBuilderError::Geometry { system_id: 0 })?;
            let absolute_x = i32::try_from(absolute_x)
                .map_err(|_| NativeStemsHeadBuilderError::Geometry { system_id: 0 })?;
            let absolute_y = i32::try_from(absolute_y)
                .map_err(|_| NativeStemsHeadBuilderError::Geometry { system_id: 0 })?;
            if absolute_x < head_bounds.x
                || absolute_y < head_bounds.y
                || absolute_x >= head_bounds.x.wrapping_add(head_bounds.width)
                || absolute_y >= head_bounds.y.wrapping_add(head_bounds.height)
            {
                continue;
            }
            let local_x = usize::try_from(absolute_x - head_bounds.x)
                .map_err(|_| NativeStemsHeadBuilderError::Geometry { system_id: 0 })?;
            let local_y = usize::try_from(absolute_y - head_bounds.y)
                .map_err(|_| NativeStemsHeadBuilderError::Geometry { system_id: 0 })?;
            if head.run_table.get(local_x, local_y) == FOREGROUND {
                removed += 1;
            }
        }
    }
    Ok(removed)
}

fn glyph_bounds(glyph: &RetrievedHeadGlyph) -> Result<JavaRectangle, NativeStemsHeadBuilderError> {
    if glyph.glyph_bounds.width <= 0
        || glyph.glyph_bounds.height <= 0
        || usize::try_from(glyph.glyph_bounds.width).ok() != Some(glyph.run_table.width())
        || usize::try_from(glyph.glyph_bounds.height).ok() != Some(glyph.run_table.height())
        || run_table_weight(&glyph.run_table) != glyph.weight
    {
        return Err(NativeStemsHeadBuilderError::Geometry { system_id: 0 });
    }
    Ok(JavaRectangle::new(
        glyph.glyph_bounds.x,
        glyph.glyph_bounds.y,
        glyph.glyph_bounds.width,
        glyph.glyph_bounds.height,
    ))
}

fn run_table_weight(table: &RunTable) -> usize {
    (0..table.sequence_count())
        .map(|ordinal| {
            table
                .sequence(ordinal)
                .unwrap_or_default()
                .iter()
                .map(|run| run.length)
                .sum::<usize>()
        })
        .sum()
}

fn section_full_position_cmp(left: &Section, right: &Section) -> Ordering {
    left.first_pos()
        .cmp(&right.first_pos())
        .then_with(|| left.start_coord().cmp(&right.start_coord()))
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

fn java_passes_greater_than_reject(value: f64, maximum: f64) -> bool {
    value.partial_cmp(&maximum) != Some(Ordering::Greater)
}

fn contribution(range: JavaRectangle, bounds: Bounds) -> i32 {
    java_bounds(bounds).map_or(0, |bounds| y_overlap_java(range, bounds).max(0))
}

fn y_overlap_bounds(left: Bounds, right: Bounds) -> i32 {
    match (java_bounds(left), java_bounds(right)) {
        (Some(left), Some(right)) => y_overlap_java(left, right),
        _ => 0,
    }
}

fn y_overlap_java(left: JavaRectangle, right: JavaRectangle) -> i32 {
    let top = left.y.max(right.y);
    let bottom = left
        .y
        .wrapping_add(left.height)
        .min(right.y.wrapping_add(right.height));
    bottom.wrapping_sub(top)
}

fn java_bounds(bounds: Bounds) -> Option<JavaRectangle> {
    Some(JavaRectangle::new(
        i32::try_from(bounds.x).ok()?,
        i32::try_from(bounds.y).ok()?,
        i32::try_from(bounds.width).ok()?,
        i32::try_from(bounds.height).ok()?,
    ))
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

fn line_bounds(line: NativeStemLine) -> JavaRectangle {
    let min_x = line.start.x.min(line.stop.x).floor() as i32;
    let min_y = line.start.y.min(line.stop.y).floor() as i32;
    let max_x = line.start.x.max(line.stop.x).ceil() as i32;
    let max_y = line.start.y.max(line.stop.y).ceil() as i32;
    JavaRectangle::new(min_x, min_y, max_x - min_x, max_y - min_y)
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

fn stem_builder_y_direction(line: NativeStemLine) -> i32 {
    if line.stop.y > line.start.y { 1 } else { -1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid_table(width: usize, height: usize) -> RunTable {
        RunTable::from_pixels(
            Orientation::Vertical,
            width,
            height,
            &vec![FOREGROUND; width * height],
        )
        .expect("solid table")
    }

    fn glyph(reference: NativeStemsHeadBuilderGlyphRef, x: usize, y: usize) -> FixedGlyph {
        let run_table = solid_table(1, 3);
        FixedGlyph {
            reference,
            bounds: Bounds {
                x,
                y,
                width: 1,
                height: 3,
            },
            weight: 3,
            line: run_table_center_line(&run_table, x as i32, y as i32).expect("center line"),
            run_table,
            modeled_canonical_ordinal: 0,
        }
    }

    fn item(kind: NativeStemsHeadBuilderItemKind, y1: f64, y2: f64) -> NativeStemsHeadBuilderItem {
        NativeStemsHeadBuilderItem {
            kind,
            glyph: None,
            target: None,
            reference_point: matches!(
                kind,
                NativeStemsHeadBuilderItemKind::StartHeadHalfLinker
                    | NativeStemsHeadBuilderItemKind::HeadHalfLinker
            )
            .then_some(NativeStemPoint { x: 0.0, y: y1 }),
            head_bounds: None,
            line: NativeStemLine {
                start: NativeStemPoint { x: 0.0, y: y1 },
                stop: NativeStemPoint { x: 0.0, y: y2 },
            },
            contribution: 0,
        }
    }

    #[test]
    fn alignment_preserves_selected_vs_actual_occurrence_and_removes_all_equals() {
        let first_ref = NativeStemsHeadBuilderGlyphRef::Chunk {
            builder_ordinal: 0,
            filament_ordinal: 0,
        };
        let duplicate_ref = NativeStemsHeadBuilderGlyphRef::Chunk {
            builder_ordinal: 0,
            filament_ordinal: 1,
        };
        let alien_ref = NativeStemsHeadBuilderGlyphRef::Chunk {
            builder_ordinal: 0,
            filament_ordinal: 2,
        };
        let first = glyph(first_ref, 0, 0);
        let mut duplicate = first.clone();
        duplicate.reference = duplicate_ref;
        let mut alien = glyph(alien_ref, 10, 10);
        alien.bounds.height = 4;
        alien.weight = 4;
        alien.run_table = solid_table(1, 4);
        alien.line = run_table_center_line(&alien.run_table, 10, 10).expect("center line");
        let mut values = vec![first, duplicate, alien];
        let skew = HeadlessSkew::new(0.0, 100, 100);
        let (pass, removed) = filter_unaligned(
            &mut values,
            None,
            1,
            &skew,
            1.0,
            100.0,
            NativeStemsHeadBuilderAlignmentSubject::Chunks,
        );
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].reference, alien_ref);
        assert_eq!(removed.len(), 1);
        let comparison = pass
            .comparisons
            .iter()
            .find(|row| row.selected_alien == Some(duplicate_ref))
            .expect("duplicate occurrence selected");
        assert_eq!(comparison.actual_removed_occurrence, Some(first_ref));
    }

    #[test]
    fn item_comparator_matches_java_signed_zero_and_canonical_nan() {
        let negative_zero = item(NativeStemsHeadBuilderItemKind::SeedGlyph, -0.0, 1.0);
        let positive_zero = item(NativeStemsHeadBuilderItemKind::SeedGlyph, 0.0, 1.0);
        assert_eq!(item_cmp(&negative_zero, &positive_zero, 1), Ordering::Less);
        let payload_nan = f64::from_bits(0x7ff8_0000_0000_0042);
        let left = item(NativeStemsHeadBuilderItemKind::SeedGlyph, payload_nan, 1.0);
        let right = item(NativeStemsHeadBuilderItemKind::SeedGlyph, f64::NAN, 1.0);
        assert_eq!(item_cmp(&left, &right, 1), Ordering::Equal);
        assert_eq!(java_double_compare(0.0, payload_nan), -1);
        assert!(java_passes_greater_than_reject(f64::NAN, 3.0));
        assert!(!java_passes_greater_than_reject(3.1, 3.0));
    }

    #[test]
    fn mixed_stem_items_sort_beyond_the_timsort_merge_threshold() {
        for y_direction in [1, -1] {
            let mut items = (0..40)
                .rev()
                .map(|ordinal| {
                    let y = f64::from(ordinal * 11 % 97);
                    if ordinal % 2 == 0 {
                        let mut item = item(
                            NativeStemsHeadBuilderItemKind::HeadHalfLinker,
                            200.0 - y,
                            210.0 - y,
                        );
                        item.reference_point = Some(NativeStemPoint { x: 0.0, y });
                        item
                    } else {
                        item(NativeStemsHeadBuilderItemKind::Gap, y, y + 10.0)
                    }
                })
                .collect::<Vec<_>>();

            stable_sort_items(&mut items, y_direction, 7, "items")
                .expect("transitive mixed-item sort");
            assert!(
                items
                    .windows(2)
                    .all(|pair| { item_cmp(&pair[0], &pair[1], y_direction) != Ordering::Greater })
            );
        }
    }

    #[test]
    fn retrieve_seed_jdk_sort_is_descending_and_stable() {
        let mut rows = vec![
            (0, 10, Some(1), None),
            (1, 11, Some(3), None),
            (2, 12, Some(3), None),
            (3, 13, Some(2), None),
        ];
        assert!(jdk25_sort_seed_preliminary(&mut rows));
        assert_eq!(
            rows.iter().map(|row| row.1).collect::<Vec<_>>(),
            vec![11, 12, 13, 10]
        );
    }

    #[test]
    fn retrieve_seed_jdk_sort_carries_the_merge_path() {
        let mut rows = (0..64)
            .map(|ordinal| {
                let contribution = ((ordinal * 17) % 11) as i32;
                (ordinal, ordinal + 100, Some(contribution), None)
            })
            .collect::<Vec<_>>();
        let before = rows.clone();

        assert!(jdk25_sort_seed_preliminary(&mut rows));
        assert!(rows.windows(2).all(|pair| pair[0].2 >= pair[1].2));
        for contribution in 0..=10 {
            let expected = before
                .iter()
                .filter(|row| row.2 == Some(contribution))
                .map(|row| row.1)
                .collect::<Vec<_>>();
            let actual = rows
                .iter()
                .filter(|row| row.2 == Some(contribution))
                .map(|row| row.1)
                .collect::<Vec<_>>();
            assert_eq!(
                actual, expected,
                "contribution tie {contribution} stays stable"
            );
        }
    }

    #[test]
    fn builder_y_range_is_line_bounds_not_c_retrieve_seed_rounding() {
        let line = NativeStemLine {
            start: NativeStemPoint { x: 7.2, y: 10.6 },
            stop: NativeStemPoint { x: 7.8, y: 20.4 },
        };
        assert_eq!(line_bounds(line), JavaRectangle::new(7, 10, 1, 11));
        let c_seed_range = JavaRectangle::new(
            0,
            line.start.y.round_ties_even() as i32,
            0,
            (line.stop.y - line.start.y).abs().round_ties_even() as i32,
        );
        assert_eq!(c_seed_range, JavaRectangle::new(0, 11, 0, 10));
        assert_ne!(line_bounds(line), c_seed_range);
    }

    #[test]
    fn builder_y_range_uses_refined_line_not_lookup_snapshot() {
        // Chula system 1, head x0, Right/Bottom: lookup geometry retains the
        // earlier part-limit line while the C linker carries a closer target.
        let lookup_snapshot = NativeStemLine {
            start: NativeStemPoint { x: 415.0, y: 447.0 },
            stop: NativeStemPoint {
                x: 411.235_402_897_741_84,
                y: 922.605_566_998_636_7,
            },
        };
        let refined = NativeStemLine {
            start: NativeStemPoint { x: 415.0, y: 447.0 },
            stop: NativeStemPoint {
                x: 411.327_944_632_489_33,
                y: 910.914_179_306_926_7,
            },
        };
        assert_ne!(lookup_snapshot, refined);
        assert_eq!(line_bounds(lookup_snapshot).height, 476);
        assert_eq!(line_bounds(refined).height, 464);
    }

    #[test]
    fn head_pixel_filter_uses_exact_run_table_membership() {
        let chunk = glyph(
            NativeStemsHeadBuilderGlyphRef::Chunk {
                builder_ordinal: 0,
                filament_ordinal: 0,
            },
            10,
            9,
        );
        let run_table = solid_table(2, 2);
        let bounds = crate::head_template::HeadTemplateBounds {
            x: 10,
            y: 10,
            width: 2,
            height: 2,
        };
        let head = RetrievedHeadGlyph {
            template_bounds: bounds,
            foreground_bounds: crate::head_template::HeadTemplateBounds {
                x: 0,
                y: 0,
                width: 2,
                height: 2,
            },
            glyph_bounds: bounds,
            weight: 4,
            run_digest: 0,
            run_table,
            new_inter_bounds: bounds,
        };
        assert_eq!(
            removed_head_pixels(&chunk, &head).expect("pixel overlap"),
            2
        );
    }

    #[test]
    fn head_parts_preserves_the_java_vip_only_removal_branch() {
        assert_eq!(
            head_parts_decision(true, HEAD_PART_MIN_REMAINING_WEIGHT - 1),
            (
                NativeStemsHeadBuilderHeadPartsAction::RemoveVipOnly,
                NativeStemsHeadBuilderChunkAction::HeadPartsVipOnly,
            )
        );
        assert_eq!(
            head_parts_decision(false, HEAD_PART_MIN_REMAINING_WEIGHT - 1),
            (
                NativeStemsHeadBuilderHeadPartsAction::KeepNonVipJavaBehavior,
                NativeStemsHeadBuilderChunkAction::Keep,
            )
        );
        assert_eq!(
            head_parts_decision(true, HEAD_PART_MIN_REMAINING_WEIGHT),
            (
                NativeStemsHeadBuilderHeadPartsAction::Keep,
                NativeStemsHeadBuilderChunkAction::Keep,
            )
        );
    }

    #[test]
    fn start_and_target_head_bounds_expand_length() {
        let theoretical = NativeStemLine {
            start: NativeStemPoint { x: 5.0, y: 10.0 },
            stop: NativeStemPoint { x: 5.0, y: 30.0 },
        };
        let mut start = item(
            NativeStemsHeadBuilderItemKind::StartHeadHalfLinker,
            10.0,
            10.0,
        );
        start.head_bounds = Some(JavaRectangle::new(2, 8, 6, 6));
        let mut target = item(NativeStemsHeadBuilderItemKind::HeadHalfLinker, 20.0, 20.0);
        target.head_bounds = Some(JavaRectangle::new(2, 18, 6, 8));
        assert_eq!(length_at(&[start.clone()], 0, 1, theoretical), 4);
        assert_eq!(length_at(&[start, target], 1, 1, theoretical), 16);
    }
}
