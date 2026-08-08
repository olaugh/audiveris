// SPDX-License-Identifier: AGPL-3.0-or-later

//! Identity-free reachability for Java `HeadLinker.CLinker.inspect`.
//!
//! This boundary starts with the append-only B-linker arenas produced by beam
//! reachability, visits live heads in stable abscissa order and corners in
//! `HeadCorner.values()` order (`TR, BL, TL, BR`), assigns each C linker's
//! retrieved seed list, and appends head-origin anchor B linkers immediately.
//! It deliberately stops before constructing a C-origin `StemBuilder`.

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use audiveris_image::{beam_structure::Segment, run_table::Orientation, section::Bounds};

use crate::{
    beam_inters::openjdk_order1_crosses,
    head_scanner_slices::JavaRectangle,
    head_template::HeadTemplateShape,
    native_heads::NativeHeadsRecognition,
    native_heads_staff_epilog::NativeHeadStaffEpilogRef,
    native_stem_seeds::{NativeStemSeedGlyph, NativeStemSeedRecognition},
    native_stems_beam_reachability::{
        NativeStemsBeamArenaOrigin, NativeStemsBeamReachabilityRecognition,
    },
    native_stems_beam_stumps::{
        NativeStemsBeamSource, NativeStemsBeamStumpBeam, NativeStemsBeamStumpRecognition,
    },
    native_stems_beam_vlinkers::{
        NativeStemsBeamBLinkerRef, NativeStemsBeamVLinkerRecognition, beam_border,
        convex_quad_intersects_rectangle, generic_intersection,
    },
    native_stems_head_corners::{
        NativeStemsHeadCorner, NativeStemsHeadCornerRecognition, NativeStemsHeadCornerSystem,
    },
    native_stems_head_seeds::{NativeStemsHeadSeedRecognition, NativeStemsHeadSeedSystem},
    native_stems_head_stumps::{
        NativeStemsHeadStumpOutcome, NativeStemsHeadStumpRecognition, NativeStemsHeadStumpSystem,
    },
    recognize::GridLinesRecognition,
    stems_step::{NativeStemHeadSide, NativeStemLine, NativeStemPoint, NativeStemVerticalSide},
};

const MAX_BEAM_LINKER_DX_RATIO: f64 = 0.25;
const MAX_BEAM_SIDE_DX_RATIO: f64 = 0.25;
const MIN_BEAM_HEAD_DY_RATIO: f64 = 1.0;
const MIN_HEAD_HEAD_DY_RATIO: f64 = 0.25;
const MIN_SEED_CONTRIBUTION_RATIO: f64 = 0.5;
const MAX_LINE_SEED_DX_RATIO: f64 = 0.15;
const SLOPE_MARGIN: f64 = 0.015;

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadCornerReachabilityRecognition {
    pub systems: Vec<NativeStemsHeadCornerReachabilitySystem>,
    pub head_count: usize,
    pub corner_count: usize,
    pub seed_scan_count: usize,
    pub kept_seed_count: usize,
    pub head_scan_count: usize,
    pub head_target_count: usize,
    pub sibling_scan_count: usize,
    pub beam_target_count: usize,
    pub created_anchor_count: usize,
    pub c_seed_assignment_count: usize,
    pub beam_prefix_builder_check_count: usize,
    pub builder_assignment_check_count: usize,
    /// Page-persistent glyph/filament registries are outside this boundary and
    /// are never changed by reachability.
    pub page_persistent_registration_mutation_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadCornerReachabilitySystem {
    pub system_id: usize,
    pub profile: i32,
    pub interline: i32,
    pub system_bounds: JavaRectangle,
    pub max_beam_linker_dx: i32,
    pub max_beam_side_dx: i32,
    pub min_beam_head_dy: i32,
    pub min_head_head_dy: i32,
    pub min_seed_contribution: i32,
    pub max_line_seed_dx: f64,
    pub max_head_in_dx: i32,
    pub max_head_out_dx: i32,
    pub slope_margin: f64,
    pub kept_seed_ordinals: Vec<usize>,
    pub beam_sources_in_sig_order: Vec<NativeStemsBeamSource>,
    pub beam_sources_by_abscissa: Vec<NativeStemsBeamSource>,
    /// Fresh `BeamGroupInter.getMembers()` relation/insertion order.
    pub beam_groups_in_source_order: Vec<Vec<NativeStemsBeamSource>>,
    pub head_sig_ordinals: Vec<usize>,
    pub head_sig_ordinals_by_abscissa: Vec<usize>,
    /// Head-x × `HorizontalSide` LEFT/RIGHT × `VerticalSide` TOP/BOTTOM.
    pub c_construction_order: Vec<NativeStemsHeadCornerRef>,
    /// Head-x × `HeadCorner.values()` (`TR, BL, TL, BR`).
    pub c_inspection_order: Vec<NativeStemsHeadCornerRef>,
    pub competing_heads: Vec<NativeStemsHeadCompetingPair>,
    pub after_beam_arenas: Vec<NativeStemsHeadBeamArena>,
    pub heads: Vec<NativeStemsHeadReachabilityHead>,
    pub final_beam_arenas: Vec<NativeStemsHeadBeamArena>,
    pub c_seed_assignment_count: usize,
    pub beam_prefix_builder_check_count: usize,
    pub builder_assignment_check_count: usize,
    pub c_builder_assignment_count: usize,
    pub forbidden_sig_mutation_count: usize,
    pub forbidden_link_state_mutation_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct NativeStemsHeadCornerRef {
    pub head: NativeHeadStaffEpilogRef,
    pub sig_ordinal: usize,
    pub x_ordinal: usize,
    pub horizontal: NativeStemHeadSide,
    pub vertical: NativeStemVerticalSide,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct NativeStemsHeadCompetingPair {
    pub first_sig_ordinal: usize,
    pub second_sig_ordinal: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadReachabilityHead {
    pub x_ordinal: usize,
    pub sig_ordinal: usize,
    pub reference: NativeHeadStaffEpilogRef,
    pub staff_id: usize,
    pub shape: HeadTemplateShape,
    pub duration: NativeStemsHeadDuration,
    pub bounds: JavaRectangle,
    pub center: (i32, i32),
    pub grade: f64,
    pub corners: Vec<NativeStemsHeadReachabilityCorner>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsHeadDuration {
    Quarter,
    Half,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadReachabilityCorner {
    pub constructor_ordinal: usize,
    pub inspection_ordinal: usize,
    pub reference: NativeStemsHeadCornerRef,
    pub x_direction: i32,
    pub y_direction: i32,
    pub reference_point: NativeStemPoint,
    pub out_point: NativeStemPoint,
    pub in_point: NativeStemPoint,
    pub stump: Option<NativeStemsHeadReachabilityStump>,
    pub part_y_limit: f64,
    pub initial_lookup: NativeStemsHeadLookupGeometry,
    pub beam_scans: Vec<NativeStemsHeadBeamScan>,
    pub beam_direction_decisions: Vec<NativeStemsHeadBeamDirectionDecision>,
    pub beam_group_decisions: Vec<NativeStemsHeadBeamGroupDecision>,
    pub beam_groups: Vec<usize>,
    pub target_group_states: Vec<NativeStemsHeadTargetGroupState>,
    pub target_beam_scans: Vec<NativeStemsHeadTargetBeamScan>,
    pub target_beam: Option<NativeStemsBeamSource>,
    pub target_point: NativeStemPoint,
    pub final_lookup: NativeStemsHeadLookupGeometry,
    pub theoretical_line: NativeStemLine,
    pub y_range: JavaRectangle,
    pub seed_scans: Vec<NativeStemsHeadSeedScan>,
    pub seed_overlap_decisions: Vec<NativeStemsHeadSeedOverlapDecision>,
    pub assigned_seed_ordinals: Vec<usize>,
    pub head_scans: Vec<NativeStemsHeadScan>,
    pub head_targets: Vec<NativeStemsHeadCornerRef>,
    pub beam_action: NativeStemsHeadBeamAction,
    pub sibling_lookup_cross: Option<NativeStemPoint>,
    pub sibling_scans: Vec<NativeStemsHeadSiblingScan>,
    pub find_linkers: Vec<NativeStemsHeadFindLinker>,
    pub beam_targets: Vec<NativeStemsBeamBLinkerRef>,
    /// Java appends all C targets before all B targets.
    pub ordered_targets: Vec<NativeStemsHeadReachabilityTarget>,
    pub c_seeds_assigned: bool,
    pub c_builder_assigned: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeStemsHeadReachabilityStump {
    pub source: NativeStemsHeadStumpRef,
    pub bounds: Bounds,
    pub weight: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsHeadStumpRef {
    Seed {
        free_glyph_ordinal: usize,
    },
    Built {
        head_x_ordinal: usize,
        constructor_ordinal: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsHeadLookupGeometry {
    pub y_limit: f64,
    pub quadrilateral: [NativeStemPoint; 4],
    pub bounds: JavaRectangle,
    pub double_bounds: NativeStemsHeadDoubleBounds,
    pub theoretical_line: NativeStemLine,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsHeadDoubleBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsHeadBeamScanAction {
    Removed,
    AcceptedArea,
    Outside,
    BreakX,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsHeadBeamScan {
    pub neighbor_ordinal: usize,
    pub beam: NativeStemsBeamSource,
    pub bounds: JavaRectangle,
    pub action: NativeStemsHeadBeamScanAction,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsHeadBeamDirectionDecision {
    pub candidate_ordinal: usize,
    pub beam: NativeStemsBeamSource,
    pub near_border: Segment,
    pub target: NativeStemPoint,
    pub directed_distance: f64,
    pub accepted: bool,
    pub sorted_ordinal: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsHeadBeamGroupDecision {
    pub sorted_ordinal: usize,
    pub beam: NativeStemsBeamSource,
    pub group_ordinal: usize,
    pub near_gate_evaluated: bool,
    pub near_target: Option<NativeStemPoint>,
    pub directed_distance: Option<f64>,
    pub accepted: bool,
    pub inserted_group: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsHeadTargetBeamScan {
    pub group_list_ordinal: usize,
    pub group_ordinal: usize,
    pub member_ordinal: usize,
    pub beam: NativeStemsBeamSource,
    pub median: Segment,
    pub intersects_initial_theoretical_line: bool,
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadTargetGroupState {
    pub group_list_ordinal: usize,
    pub group_ordinal: usize,
    pub members_before_sort: Vec<NativeStemsBeamSource>,
    pub members_after_sort: Vec<NativeStemsBeamSource>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsHeadSeedScanAction {
    OutsideLookup,
    OverlapsStump,
    InsufficientContribution,
    TooFarFromLine,
    Preliminary,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsHeadSeedScan {
    pub neighbor_ordinal: usize,
    pub free_glyph_ordinal: usize,
    pub bounds: Bounds,
    pub centroid: (i32, i32),
    pub contribution: Option<i32>,
    pub line_distance: Option<f64>,
    pub preliminary_ordinal: Option<usize>,
    pub sorted_preliminary_ordinal: Option<usize>,
    pub action: NativeStemsHeadSeedScanAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsHeadSeedOverlapAction {
    Kept,
    RejectedOverlap,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsHeadSeedOverlapDecision {
    pub sorted_preliminary_ordinal: usize,
    pub free_glyph_ordinal: usize,
    pub contribution: i32,
    pub first_overlapping_kept_seed: Option<usize>,
    pub action: NativeStemsHeadSeedOverlapAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsHeadScanAction {
    Removed,
    OutsideLookup,
    BreakX,
    SelfHead,
    CompetingHead,
    DurationMismatch,
    TooNear,
    CornersChecked,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsHeadCornerContainment {
    pub horizontal: NativeStemHeadSide,
    pub target: NativeStemsHeadCornerRef,
    pub reference_point: NativeStemPoint,
    pub contained: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadScan {
    pub x_ordinal: usize,
    pub sig_ordinal: usize,
    pub reference: NativeHeadStaffEpilogRef,
    pub bounds: JavaRectangle,
    pub intersects_lookup: bool,
    pub candidate_after_area: bool,
    pub competing: Option<bool>,
    pub duration: NativeStemsHeadDuration,
    pub directed_distance: Option<f64>,
    pub corner_checks: Vec<NativeStemsHeadCornerContainment>,
    pub action: NativeStemsHeadScanAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsHeadBeamAction {
    NoTargetBeam,
    VoidSideSkipped,
    Inspected,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsHeadSiblingScan {
    pub input_ordinal: usize,
    pub beam: NativeStemsBeamSource,
    pub cross: NativeStemPoint,
    pub left_limit: f64,
    pub right_limit: f64,
    pub accepted: bool,
    pub sorted_ordinal: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsHeadFindCandidate {
    pub insertion_ordinal: usize,
    pub reference: NativeStemsBeamBLinkerRef,
    pub reference_point: NativeStemPoint,
    pub dx: f64,
    pub best_dx_before: f64,
    pub replaces_best: bool,
    pub best_after: Option<NativeStemsBeamBLinkerRef>,
    pub best_dx_after: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsHeadFindResult {
    Reused(NativeStemsBeamBLinkerRef),
    CreatedAnchor(NativeStemsBeamBLinkerRef),
}

impl NativeStemsHeadFindResult {
    #[must_use]
    pub const fn reference(self) -> NativeStemsBeamBLinkerRef {
        match self {
            Self::Reused(reference) | Self::CreatedAnchor(reference) => reference,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadFindLinker {
    pub find_ordinal: usize,
    pub requesting_corner: NativeStemsHeadCornerRef,
    pub sibling_ordinal: usize,
    pub target_beam: NativeStemsBeamSource,
    pub theoretical_line: NativeStemLine,
    pub cross: NativeStemPoint,
    pub arena_before: Vec<NativeStemsBeamBLinkerRef>,
    pub candidates: Vec<NativeStemsHeadFindCandidate>,
    pub selected_before_threshold: Option<NativeStemsBeamBLinkerRef>,
    pub best_dx: f64,
    pub max_beam_linker_dx: i32,
    pub result: NativeStemsHeadFindResult,
    pub reused_anchor: bool,
    pub arena_after: Vec<NativeStemsBeamBLinkerRef>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsHeadReachabilityTarget {
    Head(NativeStemsHeadCornerRef),
    Beam(NativeStemsBeamBLinkerRef),
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadBeamArena {
    pub beam: NativeStemsBeamSource,
    pub sig_ordinal: usize,
    pub initial_b_count: usize,
    pub entries: Vec<NativeStemsHeadBeamArenaEntry>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadBeamArenaEntry {
    pub reference: NativeStemsBeamBLinkerRef,
    pub reference_point: NativeStemPoint,
    pub horizontal_side: Option<NativeStemHeadSide>,
    pub origin: NativeStemsHeadBeamArenaOrigin,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemsHeadBeamArenaOrigin {
    Initial,
    BeamReachabilityAnchor(NativeStemsBeamArenaOrigin),
    HeadReachabilityAnchor {
        requesting_corner: NativeStemsHeadCornerRef,
        sibling_ordinal: usize,
        find_ordinal: usize,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum NativeStemsHeadCornerReachabilityError {
    SystemOrder,
    MissingSystem {
        system_id: usize,
        product: &'static str,
    },
    InvalidParameters {
        system_id: usize,
    },
    InvalidHeadOrder {
        system_id: usize,
    },
    InvalidCornerOrder {
        system_id: usize,
        sig_ordinal: usize,
    },
    MissingHead {
        system_id: usize,
        sig_ordinal: usize,
    },
    MissingPart {
        system_id: usize,
        staff_id: usize,
    },
    MissingSeed {
        system_id: usize,
        free_glyph_ordinal: usize,
    },
    MissingStump {
        system_id: usize,
        sig_ordinal: usize,
        constructor_ordinal: usize,
    },
    MissingBeam {
        system_id: usize,
        source: NativeStemsBeamSource,
    },
    MissingBeamArena {
        system_id: usize,
        source: NativeStemsBeamSource,
    },
    InvalidBeamArena {
        system_id: usize,
        source: NativeStemsBeamSource,
    },
    MissingBeamGroup {
        system_id: usize,
        group_ordinal: usize,
    },
    UnsupportedHeadShape {
        system_id: usize,
        head: NativeHeadStaffEpilogRef,
        shape: HeadTemplateShape,
    },
    UnsupportedCompetingTopology {
        system_id: usize,
    },
    InvalidGeometry {
        system_id: usize,
    },
}

impl fmt::Display for NativeStemsHeadCornerReachabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "native head-corner reachability error: {self:?}")
    }
}

impl Error for NativeStemsHeadCornerReachabilityError {}

/// Materialize C-linker reachability without constructing any C-origin stem
/// builder or touching page-persistent registration state.
#[allow(clippy::too_many_arguments)]
pub fn materialize_native_stems_head_corner_reachability(
    grid: &GridLinesRecognition,
    stem_seeds: &NativeStemSeedRecognition,
    heads: &NativeHeadsRecognition,
    head_corners: &NativeStemsHeadCornerRecognition,
    head_seeds: &NativeStemsHeadSeedRecognition,
    head_stumps: &NativeStemsHeadStumpRecognition,
    beam_stumps: &NativeStemsBeamStumpRecognition,
    beam_vlinkers: &NativeStemsBeamVLinkerRecognition,
    beam_reachability: &NativeStemsBeamReachabilityRecognition,
) -> Result<NativeStemsHeadCornerReachabilityRecognition, NativeStemsHeadCornerReachabilityError> {
    let grid_ids = grid
        .peak_graph
        .sig
        .systems
        .iter()
        .map(|system| system.system_id)
        .collect::<Vec<_>>();
    let products = [
        stem_seeds
            .systems
            .iter()
            .map(|system| system.raw.system_id)
            .collect::<Vec<_>>(),
        heads
            .epilog
            .systems
            .iter()
            .map(|system| system.system_id)
            .collect(),
        head_corners
            .systems
            .iter()
            .map(|system| system.system_id)
            .collect(),
        head_seeds
            .systems
            .iter()
            .map(|system| system.system_id)
            .collect(),
        head_stumps
            .systems
            .iter()
            .map(|system| system.system_id)
            .collect(),
        beam_stumps
            .systems
            .iter()
            .map(|system| system.system_id)
            .collect(),
        beam_vlinkers
            .systems
            .iter()
            .map(|system| system.system_id)
            .collect(),
        beam_reachability
            .systems
            .iter()
            .map(|system| system.system_id)
            .collect(),
    ];
    if products.iter().any(|ids| ids != &grid_ids) {
        return Err(NativeStemsHeadCornerReachabilityError::SystemOrder);
    }

    let mut totals = ReachabilityTotals::default();
    let mut systems = Vec::with_capacity(grid_ids.len());
    for (system_index, &system_id) in grid_ids.iter().enumerate() {
        let staff_epilog = heads
            .epilog
            .staff_epilog
            .systems
            .iter()
            .find(|system| system.system_id == system_id)
            .ok_or(NativeStemsHeadCornerReachabilityError::MissingSystem {
                system_id,
                product: "HEADS staff epilog",
            })?;
        systems.push(materialize_system(SystemInputs {
            system_id,
            grid,
            stem_seed_system: &stem_seeds.systems[system_index],
            final_head_system: &heads.epilog.systems[system_index],
            staff_epilog,
            corner_system: &head_corners.systems[system_index],
            head_seed_system: &head_seeds.systems[system_index],
            head_stump_system: &head_stumps.systems[system_index],
            beam_stump_system: &beam_stumps.systems[system_index],
            beam_vlinker_system: &beam_vlinkers.systems[system_index],
            beam_reachability_system: &beam_reachability.systems[system_index],
            totals: &mut totals,
        })?);
    }
    Ok(NativeStemsHeadCornerReachabilityRecognition {
        systems,
        head_count: totals.heads,
        corner_count: totals.corners,
        seed_scan_count: totals.seed_scans,
        kept_seed_count: totals.kept_seeds,
        head_scan_count: totals.head_scans,
        head_target_count: totals.head_targets,
        sibling_scan_count: totals.sibling_scans,
        beam_target_count: totals.beam_targets,
        created_anchor_count: totals.created_anchors,
        c_seed_assignment_count: totals.seed_assignments,
        beam_prefix_builder_check_count: totals.beam_builder_checks,
        builder_assignment_check_count: totals.beam_builder_checks + totals.seed_assignments,
        page_persistent_registration_mutation_count: 0,
    })
}

#[derive(Default)]
struct ReachabilityTotals {
    heads: usize,
    corners: usize,
    seed_scans: usize,
    kept_seeds: usize,
    head_scans: usize,
    head_targets: usize,
    sibling_scans: usize,
    beam_targets: usize,
    created_anchors: usize,
    seed_assignments: usize,
    beam_builder_checks: usize,
}

struct SystemInputs<'a> {
    system_id: usize,
    grid: &'a GridLinesRecognition,
    stem_seed_system: &'a crate::native_stem_seeds::NativeStemSeedSystemRecognition,
    final_head_system: &'a crate::native_heads_epilog::NativeHeadsEpilogSystem,
    staff_epilog: &'a crate::native_heads_staff_epilog::NativeHeadsStaffEpilogSystem,
    corner_system: &'a NativeStemsHeadCornerSystem,
    head_seed_system: &'a NativeStemsHeadSeedSystem,
    head_stump_system: &'a NativeStemsHeadStumpSystem,
    beam_stump_system: &'a crate::native_stems_beam_stumps::NativeStemsBeamStumpSystem,
    beam_vlinker_system: &'a crate::native_stems_beam_vlinkers::NativeStemsBeamVLinkerSystem,
    beam_reachability_system:
        &'a crate::native_stems_beam_reachability::NativeStemsBeamReachabilitySystem,
    totals: &'a mut ReachabilityTotals,
}

#[derive(Clone)]
struct CornerGeometry {
    reference: NativeStemsHeadCornerRef,
    x_direction: i32,
    y_direction: i32,
    reference_point: NativeStemPoint,
    out_point: NativeStemPoint,
    in_point: NativeStemPoint,
    stump: Option<NativeStemsHeadReachabilityStump>,
    part_y_limit: f64,
    initial_lookup: NativeStemsHeadLookupGeometry,
    beam_scans: Vec<NativeStemsHeadBeamScan>,
    beam_direction_decisions: Vec<NativeStemsHeadBeamDirectionDecision>,
    beam_group_decisions: Vec<NativeStemsHeadBeamGroupDecision>,
    beam_groups: Vec<usize>,
    target_group_states: Vec<NativeStemsHeadTargetGroupState>,
    target_beam_scans: Vec<NativeStemsHeadTargetBeamScan>,
    target_beam: Option<NativeStemsBeamSource>,
    target_point: NativeStemPoint,
    final_lookup: NativeStemsHeadLookupGeometry,
    theoretical_line: NativeStemLine,
    y_range: JavaRectangle,
}

fn materialize_system(
    inputs: SystemInputs<'_>,
) -> Result<NativeStemsHeadCornerReachabilitySystem, NativeStemsHeadCornerReachabilityError> {
    let system_id = inputs.system_id;
    let corner_system = inputs.corner_system;
    let seed_system = inputs.stem_seed_system;
    let head_seed_system = inputs.head_seed_system;
    let head_stump_system = inputs.head_stump_system;
    let beam_system = inputs.beam_stump_system;
    let v_system = inputs.beam_vlinker_system;
    let reach_system = inputs.beam_reachability_system;
    if corner_system.system_id != system_id
        || seed_system.raw.system_id != system_id
        || head_seed_system.system_id != system_id
        || head_stump_system.system_id != system_id
        || beam_system.system_id != system_id
        || v_system.system_id != system_id
        || reach_system.system_id != system_id
    {
        return Err(NativeStemsHeadCornerReachabilityError::SystemOrder);
    }
    let interline = corner_system.interline;
    if interline <= 0
        || beam_system.interline != interline
        || v_system.interline != interline
        || reach_system.interline != interline
        || beam_system.profile != corner_system.profile
        || v_system.profile != corner_system.profile
        || beam_system.system_bounds != v_system.system_bounds
        || inputs.grid.global_slope.to_bits() != v_system.global_slope.to_bits()
        || inputs.grid.global_slope.to_bits() != reach_system.global_slope.to_bits()
        || !inputs.grid.global_slope.is_finite()
    {
        return Err(NativeStemsHeadCornerReachabilityError::InvalidParameters { system_id });
    }
    validate_head_order(corner_system)?;

    let max_beam_linker_dx = to_pixels(interline, MAX_BEAM_LINKER_DX_RATIO);
    let max_beam_side_dx = to_pixels(interline, MAX_BEAM_SIDE_DX_RATIO);
    let min_beam_head_dy = to_pixels(interline, MIN_BEAM_HEAD_DY_RATIO);
    let min_head_head_dy = to_pixels(interline, MIN_HEAD_HEAD_DY_RATIO);
    let min_seed_contribution = to_pixels(interline, MIN_SEED_CONTRIBUTION_RATIO);
    let max_line_seed_dx = f64::from(interline) * MAX_LINE_SEED_DX_RATIO;
    if beam_system.max_beam_side_dx != max_beam_side_dx
        || v_system.max_beam_side_dx != max_beam_side_dx
        || reach_system.max_beam_side_dx != max_beam_side_dx
        || reach_system.max_beam_linker_dx != max_beam_linker_dx
        || reach_system.min_beam_head_dy != min_beam_head_dy
    {
        return Err(NativeStemsHeadCornerReachabilityError::InvalidParameters { system_id });
    }

    let beam_map = beam_system
        .beams_by_abscissa
        .iter()
        .map(|beam| (beam.source, beam))
        .collect::<BTreeMap<_, _>>();
    if beam_map.len() != beam_system.beams_by_abscissa.len() {
        return Err(NativeStemsHeadCornerReachabilityError::InvalidParameters { system_id });
    }
    let beam_sources_by_abscissa = beam_system
        .beams_by_abscissa
        .iter()
        .map(|beam| beam.source)
        .collect::<Vec<_>>();
    if beam_system.beams_by_abscissa.windows(2).any(|pair| {
        pair[1].bounds.x < pair[0].bounds.x
            || (pair[1].bounds.x == pair[0].bounds.x && pair[1].sig_ordinal <= pair[0].sig_ordinal)
    }) {
        return Err(NativeStemsHeadCornerReachabilityError::InvalidParameters { system_id });
    }
    if reach_system.beam_sources_in_inspection_order != beam_sources_by_abscissa {
        return Err(NativeStemsHeadCornerReachabilityError::InvalidParameters { system_id });
    }
    let mut beams_by_sig = beam_system.beams_by_abscissa.iter().collect::<Vec<_>>();
    beams_by_sig.sort_by_key(|beam| beam.sig_ordinal);
    if beams_by_sig
        .iter()
        .enumerate()
        .any(|(ordinal, beam)| beam.sig_ordinal != ordinal)
    {
        return Err(NativeStemsHeadCornerReachabilityError::InvalidParameters { system_id });
    }
    let beam_sources_in_sig_order = beams_by_sig.iter().map(|beam| beam.source).collect();
    let mut grouped_sources = BTreeSet::new();
    let mut grouped_source_count = 0_usize;
    for (group_ordinal, group) in reach_system.groups_in_source_order.iter().enumerate() {
        for &source in group {
            grouped_source_count += 1;
            if !grouped_sources.insert(source) {
                return Err(NativeStemsHeadCornerReachabilityError::MissingBeamGroup {
                    system_id,
                    group_ordinal,
                });
            }
            let Some(beam) = beam_map.get(&source) else {
                return Err(NativeStemsHeadCornerReachabilityError::MissingBeam {
                    system_id,
                    source,
                });
            };
            if beam.group_ordinal != group_ordinal {
                return Err(NativeStemsHeadCornerReachabilityError::MissingBeamGroup {
                    system_id,
                    group_ordinal,
                });
            }
        }
    }
    if grouped_source_count != beam_map.len()
        || grouped_sources != beam_map.keys().copied().collect::<BTreeSet<_>>()
    {
        return Err(NativeStemsHeadCornerReachabilityError::InvalidParameters { system_id });
    }

    let seed_map = seed_system
        .free_glyphs
        .iter()
        .enumerate()
        .collect::<BTreeMap<_, _>>();
    for &ordinal in &head_seed_system.kept_seed_ordinals {
        if !seed_map.contains_key(&ordinal) {
            return Err(NativeStemsHeadCornerReachabilityError::MissingSeed {
                system_id,
                free_glyph_ordinal: ordinal,
            });
        }
    }

    let final_heads = inputs
        .final_head_system
        .final_heads
        .iter()
        .copied()
        .filter(|reference| {
            staff_head(inputs.staff_epilog, *reference).is_some_and(|head| {
                matches!(
                    head.shape,
                    HeadTemplateShape::NoteheadBlack | HeadTemplateShape::NoteheadVoid
                )
            })
        })
        .collect::<BTreeSet<_>>();
    let corner_heads = corner_system
        .heads_in_sig_order
        .iter()
        .map(|head| head.reference)
        .collect::<BTreeSet<_>>();
    if inputs.final_head_system.system_id != system_id || final_heads != corner_heads {
        return Err(NativeStemsHeadCornerReachabilityError::InvalidHeadOrder { system_id });
    }
    let competing_heads = competing_head_pairs(system_id, corner_system, inputs.staff_epilog)?;
    let competing_set = competing_heads.iter().copied().collect::<BTreeSet<_>>();

    let mut arenas = reach_system
        .final_beam_arenas
        .iter()
        .map(|arena| convert_beam_arena(system_id, arena))
        .collect::<Result<Vec<_>, _>>()?;
    if arenas.iter().map(|arena| arena.beam).collect::<Vec<_>>() != beam_sources_by_abscissa {
        return Err(NativeStemsHeadCornerReachabilityError::InvalidParameters { system_id });
    }
    let after_beam_arenas = arenas.clone();
    let arena_indices = arenas
        .iter()
        .enumerate()
        .map(|(index, arena)| (arena.beam, index))
        .collect::<BTreeMap<_, _>>();
    if arena_indices.len() != arenas.len() {
        return Err(NativeStemsHeadCornerReachabilityError::InvalidParameters { system_id });
    }

    let head_sig_ordinals = (0..corner_system.heads_in_sig_order.len()).collect::<Vec<_>>();
    let head_sig_ordinals_by_abscissa = corner_system.heads_by_abscissa.clone();
    let mut geometry = BTreeMap::new();
    let mut c_construction_order = Vec::new();
    for (x_ordinal, &sig_ordinal) in corner_system.heads_by_abscissa.iter().enumerate() {
        let head = &corner_system.heads_in_sig_order[sig_ordinal];
        let head_seed = head_seed_system
            .heads_by_abscissa
            .get(x_ordinal)
            .filter(|candidate| candidate.sig_ordinal == sig_ordinal)
            .ok_or(NativeStemsHeadCornerReachabilityError::MissingHead {
                system_id,
                sig_ordinal,
            })?;
        let stump_head = head_stump_system
            .heads_by_abscissa
            .get(x_ordinal)
            .filter(|candidate| candidate.sig_ordinal == sig_ordinal)
            .ok_or(NativeStemsHeadCornerReachabilityError::MissingHead {
                system_id,
                sig_ordinal,
            })?;
        validate_corner_order(system_id, sig_ordinal, &head.corners_in_constructor_order)?;
        for corner in &head.corners_in_constructor_order {
            let reference = corner_reference(head.reference, sig_ordinal, x_ordinal, corner);
            c_construction_order.push(reference);
            let head_seed_corner = head_seed
                .corners_in_constructor_order
                .iter()
                .find(|candidate| candidate.constructor_ordinal == corner.constructor_ordinal)
                .ok_or(NativeStemsHeadCornerReachabilityError::InvalidCornerOrder {
                    system_id,
                    sig_ordinal,
                })?;
            let stump_corner = stump_head
                .corners_in_constructor_order
                .iter()
                .find(|candidate| candidate.constructor_ordinal == corner.constructor_ordinal)
                .ok_or(NativeStemsHeadCornerReachabilityError::InvalidCornerOrder {
                    system_id,
                    sig_ordinal,
                })?;
            let value = materialize_geometry(GeometryInputs {
                system_id,
                grid_slope: inputs.grid.global_slope,
                system_bounds: v_system.system_bounds,
                v_system,
                beam_system,
                beam_map: &beam_map,
                groups: &reach_system.groups_in_source_order,
                head,
                corner,
                reference,
                head_seed_corner,
                stump_corner,
                seed_map: &seed_map,
                min_beam_head_dy,
            })?;
            if geometry.insert(reference, value).is_some() {
                return Err(NativeStemsHeadCornerReachabilityError::InvalidCornerOrder {
                    system_id,
                    sig_ordinal,
                });
            }
        }
    }

    let mut c_inspection_order = Vec::with_capacity(c_construction_order.len());
    let mut materialized_heads = Vec::with_capacity(corner_system.heads_by_abscissa.len());
    let mut inspection_ordinal = 0_usize;
    let mut find_ordinal = 0_usize;
    for (x_ordinal, &sig_ordinal) in corner_system.heads_by_abscissa.iter().enumerate() {
        let head = &corner_system.heads_in_sig_order[sig_ordinal];
        let duration = head_duration(system_id, head.reference, head.shape)?;
        let mut corners = Vec::with_capacity(4);
        let mut inspect_corners = head.corners_in_constructor_order.iter().collect::<Vec<_>>();
        inspect_corners.sort_by_key(|corner| corner.inspection_ordinal);
        for corner in inspect_corners {
            let reference = corner_reference(head.reference, sig_ordinal, x_ordinal, corner);
            c_inspection_order.push(reference);
            let geometry = geometry.get(&reference).cloned().ok_or(
                NativeStemsHeadCornerReachabilityError::InvalidCornerOrder {
                    system_id,
                    sig_ordinal,
                },
            )?;
            corners.push(materialize_inspection(InspectionInputs {
                system_id,
                inspection_ordinal,
                grid_slope: inputs.grid.global_slope,
                corner_system,
                head_seed_system,
                beam_map: &beam_map,
                groups: &reach_system.groups_in_source_order,
                seed_map: &seed_map,
                competing: &competing_set,
                geometry,
                duration,
                min_head_head_dy,
                min_seed_contribution,
                max_line_seed_dx,
                max_beam_side_dx,
                max_beam_linker_dx,
                arena_indices: &arena_indices,
                arenas: &mut arenas,
                find_ordinal: &mut find_ordinal,
                totals: inputs.totals,
            })?);
            inspection_ordinal += 1;
        }
        materialized_heads.push(NativeStemsHeadReachabilityHead {
            x_ordinal,
            sig_ordinal,
            reference: head.reference,
            staff_id: head.staff_id,
            shape: head.shape,
            duration,
            bounds: head.bounds,
            center: rectangle_center(head.bounds),
            grade: f64::from_bits(head.grade_bits),
            corners,
        });
    }
    let system_seed_assignments = inspection_ordinal;
    let beam_prefix_builder_check_count = reach_system
        .beam_inspections
        .iter()
        .flat_map(|inspection| &inspection.b_visits)
        .flat_map(|visit| &visit.v_inspections)
        .count();
    inputs.totals.heads += materialized_heads.len();
    inputs.totals.corners += inspection_ordinal;
    inputs.totals.seed_assignments += system_seed_assignments;
    inputs.totals.beam_builder_checks += beam_prefix_builder_check_count;

    Ok(NativeStemsHeadCornerReachabilitySystem {
        system_id,
        profile: corner_system.profile,
        interline,
        system_bounds: v_system.system_bounds,
        max_beam_linker_dx,
        max_beam_side_dx,
        min_beam_head_dy,
        min_head_head_dy,
        min_seed_contribution,
        max_line_seed_dx,
        max_head_in_dx: corner_system.max_head_in_dx,
        max_head_out_dx: corner_system.max_head_out_dx,
        slope_margin: SLOPE_MARGIN,
        kept_seed_ordinals: head_seed_system.kept_seed_ordinals.clone(),
        beam_sources_in_sig_order,
        beam_sources_by_abscissa,
        beam_groups_in_source_order: reach_system.groups_in_source_order.clone(),
        head_sig_ordinals,
        head_sig_ordinals_by_abscissa,
        c_construction_order,
        c_inspection_order,
        competing_heads,
        after_beam_arenas,
        heads: materialized_heads,
        final_beam_arenas: arenas,
        c_seed_assignment_count: system_seed_assignments,
        beam_prefix_builder_check_count,
        builder_assignment_check_count: beam_prefix_builder_check_count + system_seed_assignments,
        c_builder_assignment_count: 0,
        forbidden_sig_mutation_count: 0,
        forbidden_link_state_mutation_count: 0,
    })
}

struct GeometryInputs<'a> {
    system_id: usize,
    grid_slope: f64,
    system_bounds: JavaRectangle,
    v_system: &'a crate::native_stems_beam_vlinkers::NativeStemsBeamVLinkerSystem,
    beam_system: &'a crate::native_stems_beam_stumps::NativeStemsBeamStumpSystem,
    beam_map: &'a BTreeMap<NativeStemsBeamSource, &'a NativeStemsBeamStumpBeam>,
    groups: &'a [Vec<NativeStemsBeamSource>],
    head: &'a crate::native_stems_head_corners::NativeStemsHeadCornerHead,
    corner: &'a NativeStemsHeadCorner,
    reference: NativeStemsHeadCornerRef,
    head_seed_corner: &'a crate::native_stems_head_seeds::NativeStemsHeadSeedCorner,
    stump_corner: &'a crate::native_stems_head_stumps::NativeStemsHeadStumpCorner,
    seed_map: &'a BTreeMap<usize, &'a NativeStemSeedGlyph>,
    min_beam_head_dy: i32,
}

struct InspectionInputs<'a> {
    system_id: usize,
    inspection_ordinal: usize,
    grid_slope: f64,
    corner_system: &'a NativeStemsHeadCornerSystem,
    head_seed_system: &'a NativeStemsHeadSeedSystem,
    beam_map: &'a BTreeMap<NativeStemsBeamSource, &'a NativeStemsBeamStumpBeam>,
    groups: &'a [Vec<NativeStemsBeamSource>],
    seed_map: &'a BTreeMap<usize, &'a NativeStemSeedGlyph>,
    competing: &'a BTreeSet<NativeStemsHeadCompetingPair>,
    geometry: CornerGeometry,
    duration: NativeStemsHeadDuration,
    min_head_head_dy: i32,
    min_seed_contribution: i32,
    max_line_seed_dx: f64,
    max_beam_side_dx: i32,
    max_beam_linker_dx: i32,
    arena_indices: &'a BTreeMap<NativeStemsBeamSource, usize>,
    arenas: &'a mut [NativeStemsHeadBeamArena],
    find_ordinal: &'a mut usize,
    totals: &'a mut ReachabilityTotals,
}

fn validate_head_order(
    system: &NativeStemsHeadCornerSystem,
) -> Result<(), NativeStemsHeadCornerReachabilityError> {
    if system.heads_by_abscissa.len() != system.heads_in_sig_order.len() {
        return Err(NativeStemsHeadCornerReachabilityError::InvalidHeadOrder {
            system_id: system.system_id,
        });
    }
    let mut seen = BTreeSet::new();
    let mut prior = None;
    for &sig_ordinal in &system.heads_by_abscissa {
        let Some(head) = system.heads_in_sig_order.get(sig_ordinal) else {
            return Err(NativeStemsHeadCornerReachabilityError::InvalidHeadOrder {
                system_id: system.system_id,
            });
        };
        if !seen.insert(sig_ordinal)
            || prior.is_some_and(|(x, prior_sig)| {
                head.bounds.x < x || (head.bounds.x == x && sig_ordinal <= prior_sig)
            })
        {
            return Err(NativeStemsHeadCornerReachabilityError::InvalidHeadOrder {
                system_id: system.system_id,
            });
        }
        prior = Some((head.bounds.x, sig_ordinal));
    }
    Ok(())
}

fn validate_corner_order(
    system_id: usize,
    sig_ordinal: usize,
    corners: &[NativeStemsHeadCorner],
) -> Result<(), NativeStemsHeadCornerReachabilityError> {
    let expected = [
        (NativeStemHeadSide::Left, NativeStemVerticalSide::Top),
        (NativeStemHeadSide::Left, NativeStemVerticalSide::Bottom),
        (NativeStemHeadSide::Right, NativeStemVerticalSide::Top),
        (NativeStemHeadSide::Right, NativeStemVerticalSide::Bottom),
    ];
    if corners.len() != expected.len()
        || corners.iter().zip(expected).enumerate().any(
            |(constructor_ordinal, (corner, (horizontal, vertical)))| {
                corner.constructor_ordinal != constructor_ordinal
                    || corner.inspection_ordinal != inspection_ordinal(horizontal, vertical)
                    || corner.horizontal != horizontal
                    || corner.vertical != vertical
            },
        )
    {
        return Err(NativeStemsHeadCornerReachabilityError::InvalidCornerOrder {
            system_id,
            sig_ordinal,
        });
    }
    Ok(())
}

fn corner_reference(
    head: NativeHeadStaffEpilogRef,
    sig_ordinal: usize,
    x_ordinal: usize,
    corner: &NativeStemsHeadCorner,
) -> NativeStemsHeadCornerRef {
    NativeStemsHeadCornerRef {
        head,
        sig_ordinal,
        x_ordinal,
        horizontal: corner.horizontal,
        vertical: corner.vertical,
    }
}

fn competing_head_pairs(
    system_id: usize,
    corners: &NativeStemsHeadCornerSystem,
    staff_epilog: &crate::native_heads_staff_epilog::NativeHeadsStaffEpilogSystem,
) -> Result<Vec<NativeStemsHeadCompetingPair>, NativeStemsHeadCornerReachabilityError> {
    let current = corners
        .heads_in_sig_order
        .iter()
        .enumerate()
        .map(|(sig_ordinal, head)| (head.reference, sig_ordinal))
        .collect::<BTreeMap<_, _>>();
    if current.len() != corners.heads_in_sig_order.len() {
        return Err(
            NativeStemsHeadCornerReachabilityError::UnsupportedCompetingTopology { system_id },
        );
    }
    let mut pairs = BTreeSet::new();
    for (staff_index, staff) in staff_epilog.staffs.iter().enumerate() {
        for decision in &staff.purge.overlap.decisions {
            if staff.heads.get(decision.left_index).is_none()
                || staff.heads.get(decision.right_index).is_none()
            {
                return Err(
                    NativeStemsHeadCornerReachabilityError::UnsupportedCompetingTopology {
                        system_id,
                    },
                );
            }
            let first = NativeHeadStaffEpilogRef {
                staff_index,
                head_index: decision.left_index,
            };
            let second = NativeHeadStaffEpilogRef {
                staff_index,
                head_index: decision.right_index,
            };
            let (Some(&first_sig_ordinal), Some(&second_sig_ordinal)) =
                (current.get(&first), current.get(&second))
            else {
                // The staff epilog precedes HEADS' beam-overlap purge. An
                // exclusion incident to a head removed there cannot occur in
                // the live `systemHeads` candidate list.
                continue;
            };
            let (first_sig_ordinal, second_sig_ordinal) = if first_sig_ordinal < second_sig_ordinal
            {
                (first_sig_ordinal, second_sig_ordinal)
            } else {
                (second_sig_ordinal, first_sig_ordinal)
            };
            pairs.insert(NativeStemsHeadCompetingPair {
                first_sig_ordinal,
                second_sig_ordinal,
            });
        }
    }
    Ok(pairs.into_iter().collect())
}

fn staff_head(
    system: &crate::native_heads_staff_epilog::NativeHeadsStaffEpilogSystem,
    reference: NativeHeadStaffEpilogRef,
) -> Option<&crate::native_heads_staff_epilog::NativeHeadStaffEpilogHead> {
    system
        .staffs
        .get(reference.staff_index)?
        .heads
        .get(reference.head_index)
}

fn convert_beam_arena(
    system_id: usize,
    arena: &crate::native_stems_beam_reachability::NativeStemsBeamArena,
) -> Result<NativeStemsHeadBeamArena, NativeStemsHeadCornerReachabilityError> {
    if arena.initial_b_count > arena.all_b_linkers.len() {
        return Err(NativeStemsHeadCornerReachabilityError::InvalidBeamArena {
            system_id,
            source: arena.beam,
        });
    }
    let mut entries = Vec::with_capacity(arena.all_b_linkers.len());
    for (insertion_ordinal, entry) in arena.all_b_linkers.iter().enumerate() {
        if entry.reference.beam != arena.beam || entry.reference.id != insertion_ordinal + 1 {
            return Err(NativeStemsHeadCornerReachabilityError::InvalidBeamArena {
                system_id,
                source: arena.beam,
            });
        }
        let origin = if insertion_ordinal < arena.initial_b_count {
            if entry.is_anchor
                || !matches!(entry.origin, NativeStemsBeamArenaOrigin::Constructor(_))
            {
                return Err(NativeStemsHeadCornerReachabilityError::InvalidBeamArena {
                    system_id,
                    source: arena.beam,
                });
            }
            NativeStemsHeadBeamArenaOrigin::Initial
        } else {
            if !entry.is_anchor
                || !matches!(entry.origin, NativeStemsBeamArenaOrigin::Anchor { .. })
            {
                return Err(NativeStemsHeadCornerReachabilityError::InvalidBeamArena {
                    system_id,
                    source: arena.beam,
                });
            }
            NativeStemsHeadBeamArenaOrigin::BeamReachabilityAnchor(entry.origin.clone())
        };
        entries.push(NativeStemsHeadBeamArenaEntry {
            reference: entry.reference,
            reference_point: entry.reference_point,
            horizontal_side: entry.horizontal_side,
            origin,
        });
    }
    Ok(NativeStemsHeadBeamArena {
        beam: arena.beam,
        sig_ordinal: arena.sig_ordinal,
        initial_b_count: arena.initial_b_count,
        entries,
    })
}

fn head_duration(
    system_id: usize,
    head: NativeHeadStaffEpilogRef,
    shape: HeadTemplateShape,
) -> Result<NativeStemsHeadDuration, NativeStemsHeadCornerReachabilityError> {
    match shape {
        HeadTemplateShape::NoteheadBlack => Ok(NativeStemsHeadDuration::Quarter),
        HeadTemplateShape::NoteheadVoid => Ok(NativeStemsHeadDuration::Half),
        _ => Err(
            NativeStemsHeadCornerReachabilityError::UnsupportedHeadShape {
                system_id,
                head,
                shape,
            },
        ),
    }
}

fn materialize_geometry(
    input: GeometryInputs<'_>,
) -> Result<CornerGeometry, NativeStemsHeadCornerReachabilityError> {
    let x_direction = horizontal_direction(input.corner.horizontal);
    let y_direction = vertical_direction(input.corner.vertical);
    let part = input
        .v_system
        .parts
        .iter()
        .find(|part| part.staff_ids.contains(&input.head.staff_id))
        .ok_or(NativeStemsHeadCornerReachabilityError::MissingPart {
            system_id: input.system_id,
            staff_id: input.head.staff_id,
        })?;
    let part_y_limit = if y_direction > 0 {
        f64::from(
            part.bounds
                .y
                .wrapping_add(part.bounds.height)
                .wrapping_sub(1),
        )
    } else {
        f64::from(part.bounds.y)
    };
    let initial_lookup = lookup_geometry(
        input.system_id,
        input.corner.reference,
        input.corner.out_point,
        input.corner.in_point,
        x_direction,
        y_direction,
        input.grid_slope,
        part_y_limit,
    )?;
    let stump = resolve_stump(
        input.system_id,
        input.reference.x_ordinal,
        input.reference.sig_ordinal,
        input.corner.constructor_ordinal,
        input.stump_corner,
        input.seed_map,
    )?;

    let vicinity = JavaRectangle::new(
        input
            .head
            .bounds
            .x
            .wrapping_sub(input.beam_system.vicinity_margin),
        input.system_bounds.y,
        input
            .head
            .bounds
            .width
            .wrapping_add(input.beam_system.vicinity_margin.wrapping_mul(2)),
        input.system_bounds.height,
    );
    let neighbors = input
        .beam_system
        .beams_by_abscissa
        .iter()
        .filter(|beam| vicinity.intersects(beam.bounds))
        .collect::<Vec<_>>();
    let lookup_max_x = initial_lookup
        .bounds
        .x
        .wrapping_add(initial_lookup.bounds.width);
    let mut beam_scans = Vec::new();
    let mut candidates = Vec::new();
    for (neighbor_ordinal, beam) in neighbors.iter().enumerate() {
        let action = if convex_quad_intersects_rectangle(initial_lookup.quadrilateral, beam.bounds)
        {
            candidates.push(*beam);
            NativeStemsHeadBeamScanAction::AcceptedArea
        } else if beam.bounds.x > lookup_max_x {
            NativeStemsHeadBeamScanAction::BreakX
        } else {
            NativeStemsHeadBeamScanAction::Outside
        };
        beam_scans.push(NativeStemsHeadBeamScan {
            neighbor_ordinal,
            beam: beam.source,
            bounds: beam.bounds,
            action,
        });
        if action == NativeStemsHeadBeamScanAction::BreakX {
            break;
        }
    }

    let mut beam_direction_decisions = Vec::with_capacity(candidates.len());
    let mut directed = Vec::new();
    for (candidate_ordinal, beam) in candidates.iter().enumerate() {
        let near_border = beam_border(beam, opposite_vertical(input.corner.vertical));
        let target = target_point(input.corner.reference, near_border, input.grid_slope);
        let directed_distance = f64::from(y_direction) * (target.y - input.corner.reference.y);
        let accepted = directed_distance > 0.0;
        if accepted {
            directed.push(*beam);
        }
        beam_direction_decisions.push(NativeStemsHeadBeamDirectionDecision {
            candidate_ordinal,
            beam: beam.source,
            near_border,
            target,
            directed_distance,
            accepted,
            sorted_ordinal: None,
        });
    }
    sort_beams_from_ref(
        &mut directed,
        input.corner.reference,
        y_direction,
        input.grid_slope,
    );
    for (sorted_ordinal, beam) in directed.iter().enumerate() {
        if let Some(decision) = beam_direction_decisions
            .iter_mut()
            .find(|decision| decision.beam == beam.source)
        {
            decision.sorted_ordinal = Some(sorted_ordinal);
        }
    }

    let mut beam_group_decisions = Vec::with_capacity(directed.len());
    let mut beam_groups = Vec::new();
    let mut seen_groups = BTreeSet::new();
    for (sorted_ordinal, beam) in directed.iter().enumerate() {
        let near_gate_evaluated = beam_groups.is_empty();
        let (near_target, directed_distance, accepted) = if near_gate_evaluated {
            let border = beam_border(beam, opposite_vertical(input.corner.vertical));
            let target = target_point(input.corner.reference, border, input.grid_slope);
            let distance = f64::from(y_direction) * (target.y - input.corner.reference.y);
            (
                Some(target),
                Some(distance),
                distance >= f64::from(input.min_beam_head_dy),
            )
        } else {
            (None, None, true)
        };
        let inserted_group = accepted && seen_groups.insert(beam.group_ordinal);
        if inserted_group {
            beam_groups.push(beam.group_ordinal);
        }
        beam_group_decisions.push(NativeStemsHeadBeamGroupDecision {
            sorted_ordinal,
            beam: beam.source,
            group_ordinal: beam.group_ordinal,
            near_gate_evaluated,
            near_target,
            directed_distance,
            accepted,
            inserted_group,
        });
    }

    let mut target_group_states = Vec::new();
    let mut target_beam_scans = Vec::new();
    let mut target_beam = None;
    let mut target_point_value = initial_lookup.theoretical_line.stop;
    let mut final_lookup = initial_lookup;
    'groups: for (group_list_ordinal, &group_ordinal) in beam_groups.iter().enumerate() {
        let members_before_sort = input.groups.get(group_ordinal).cloned().ok_or(
            NativeStemsHeadCornerReachabilityError::MissingBeamGroup {
                system_id: input.system_id,
                group_ordinal,
            },
        )?;
        let mut members = members_before_sort
            .iter()
            .map(|source| {
                input.beam_map.get(source).copied().ok_or(
                    NativeStemsHeadCornerReachabilityError::MissingBeam {
                        system_id: input.system_id,
                        source: *source,
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        sort_beams_from_ref(
            &mut members,
            input.corner.reference,
            y_direction,
            input.grid_slope,
        );
        let members_after_sort = members.iter().map(|beam| beam.source).collect::<Vec<_>>();
        target_group_states.push(NativeStemsHeadTargetGroupState {
            group_list_ordinal,
            group_ordinal,
            members_before_sort,
            members_after_sort,
        });
        for (member_ordinal, beam) in members.iter().enumerate() {
            let intersects =
                segments_intersect(beam.median, line_segment(initial_lookup.theoretical_line));
            let selected = intersects;
            target_beam_scans.push(NativeStemsHeadTargetBeamScan {
                group_list_ordinal,
                group_ordinal,
                member_ordinal,
                beam: beam.source,
                median: beam.median,
                intersects_initial_theoretical_line: intersects,
                selected,
            });
            if !intersects {
                continue;
            }
            let farthest = *members.last().expect("group containing scanned member");
            target_beam = Some(farthest.source);
            let border = beam_border(farthest, input.corner.vertical);
            let shifted_limit = Segment {
                x1: border.x1,
                y1: border.y1 + f64::from(y_direction) * farthest.height,
                x2: border.x2,
                y2: border.y2 + f64::from(y_direction) * farthest.height,
            };
            let y_limit = y_at_x(shifted_limit, input.corner.reference.x);
            final_lookup = lookup_geometry(
                input.system_id,
                input.corner.reference,
                input.corner.out_point,
                input.corner.in_point,
                x_direction,
                y_direction,
                input.grid_slope,
                y_limit,
            )?;
            target_point_value = target_point(input.corner.reference, border, input.grid_slope);
            break 'groups;
        }
    }
    let theoretical_line = NativeStemLine {
        start: input.corner.reference,
        stop: target_point_value,
    };
    let y_range = JavaRectangle::new(
        0,
        java_rint(if y_direction > 0 {
            input.corner.reference.y
        } else {
            target_point_value.y
        }),
        0,
        java_rint((target_point_value.y - input.corner.reference.y).abs()),
    );
    if input.head_seed_corner.constructor_ordinal != input.corner.constructor_ordinal {
        return Err(NativeStemsHeadCornerReachabilityError::InvalidCornerOrder {
            system_id: input.system_id,
            sig_ordinal: input.reference.sig_ordinal,
        });
    }
    Ok(CornerGeometry {
        reference: input.reference,
        x_direction,
        y_direction,
        reference_point: input.corner.reference,
        out_point: input.corner.out_point,
        in_point: input.corner.in_point,
        stump,
        part_y_limit,
        initial_lookup,
        beam_scans,
        beam_direction_decisions,
        beam_group_decisions,
        beam_groups,
        target_group_states,
        target_beam_scans,
        target_beam,
        target_point: target_point_value,
        final_lookup,
        theoretical_line,
        y_range,
    })
}

fn materialize_inspection(
    input: InspectionInputs<'_>,
) -> Result<NativeStemsHeadReachabilityCorner, NativeStemsHeadCornerReachabilityError> {
    let (seed_scans, seed_overlap_decisions, assigned_seed_ordinals) = retrieve_seeds(&input)?;
    input.totals.seed_scans += seed_scans.len();
    input.totals.kept_seeds += assigned_seed_ordinals.len();
    let (head_scans, head_targets) = lookup_other_heads(&input)?;
    input.totals.head_scans += head_scans.len();
    input.totals.head_targets += head_targets.len();

    let mut sibling_lookup_cross = None;
    let mut sibling_scans = Vec::new();
    let mut find_linkers = Vec::new();
    let mut beam_targets = Vec::new();
    let beam_action = beam_action_for(
        input.geometry.target_beam,
        input.duration,
        input.geometry.x_direction,
        input.geometry.y_direction,
    );
    if beam_action == NativeStemsHeadBeamAction::Inspected {
        if let Some(target_source) = input.geometry.target_beam {
            let target_beam = input.beam_map.get(&target_source).copied().ok_or(
                NativeStemsHeadCornerReachabilityError::MissingBeam {
                    system_id: input.system_id,
                    source: target_source,
                },
            )?;
            let lookup_cross = generic_intersection(
                target_beam.median,
                line_segment(input.geometry.theoretical_line),
            );
            sibling_lookup_cross = Some(lookup_cross);
            let group = input.groups.get(target_beam.group_ordinal).ok_or(
                NativeStemsHeadCornerReachabilityError::MissingBeamGroup {
                    system_id: input.system_id,
                    group_ordinal: target_beam.group_ordinal,
                },
            )?;
            sibling_scans = sibling_beams_at(
                input.system_id,
                lookup_cross,
                group,
                input.beam_map,
                input.grid_slope,
                input.max_beam_side_dx,
            )?;
            input.totals.sibling_scans += sibling_scans.len();
            let accepted = sibling_scans
                .iter()
                .filter_map(|scan| scan.sorted_ordinal.map(|ordinal| (ordinal, scan.beam)))
                .collect::<BTreeMap<_, _>>();
            for (sibling_ordinal, source) in accepted {
                let arena_index = *input.arena_indices.get(&source).ok_or(
                    NativeStemsHeadCornerReachabilityError::MissingBeamArena {
                        system_id: input.system_id,
                        source,
                    },
                )?;
                let beam = input.beam_map.get(&source).copied().ok_or(
                    NativeStemsHeadCornerReachabilityError::MissingBeam {
                        system_id: input.system_id,
                        source,
                    },
                )?;
                let find = find_linker(
                    input.geometry.reference,
                    sibling_ordinal,
                    *input.find_ordinal,
                    input.geometry.theoretical_line,
                    beam,
                    input.max_beam_linker_dx,
                    &mut input.arenas[arena_index],
                );
                if matches!(find.result, NativeStemsHeadFindResult::CreatedAnchor(_)) {
                    input.totals.created_anchors += 1;
                }
                *input.find_ordinal += 1;
                beam_targets.push(find.result.reference());
                find_linkers.push(find);
            }
        }
    }
    input.totals.beam_targets += beam_targets.len();
    let ordered_targets = head_targets
        .iter()
        .copied()
        .map(NativeStemsHeadReachabilityTarget::Head)
        .chain(
            beam_targets
                .iter()
                .copied()
                .map(NativeStemsHeadReachabilityTarget::Beam),
        )
        .collect();
    Ok(NativeStemsHeadReachabilityCorner {
        constructor_ordinal: constructor_ordinal(
            input.geometry.reference.horizontal,
            input.geometry.reference.vertical,
        ),
        inspection_ordinal: input.inspection_ordinal,
        reference: input.geometry.reference,
        x_direction: input.geometry.x_direction,
        y_direction: input.geometry.y_direction,
        reference_point: input.geometry.reference_point,
        out_point: input.geometry.out_point,
        in_point: input.geometry.in_point,
        stump: input.geometry.stump,
        part_y_limit: input.geometry.part_y_limit,
        initial_lookup: input.geometry.initial_lookup,
        beam_scans: input.geometry.beam_scans,
        beam_direction_decisions: input.geometry.beam_direction_decisions,
        beam_group_decisions: input.geometry.beam_group_decisions,
        beam_groups: input.geometry.beam_groups,
        target_group_states: input.geometry.target_group_states,
        target_beam_scans: input.geometry.target_beam_scans,
        target_beam: input.geometry.target_beam,
        target_point: input.geometry.target_point,
        final_lookup: input.geometry.final_lookup,
        theoretical_line: input.geometry.theoretical_line,
        y_range: input.geometry.y_range,
        seed_scans,
        seed_overlap_decisions,
        assigned_seed_ordinals,
        head_scans,
        head_targets,
        beam_action,
        sibling_lookup_cross,
        sibling_scans,
        find_linkers,
        beam_targets,
        ordered_targets,
        c_seeds_assigned: true,
        c_builder_assigned: false,
    })
}

fn beam_action_for(
    target_beam: Option<NativeStemsBeamSource>,
    duration: NativeStemsHeadDuration,
    x_direction: i32,
    y_direction: i32,
) -> NativeStemsHeadBeamAction {
    if target_beam.is_none() {
        NativeStemsHeadBeamAction::NoTargetBeam
    } else if duration == NativeStemsHeadDuration::Half && y_direction == x_direction {
        NativeStemsHeadBeamAction::VoidSideSkipped
    } else {
        NativeStemsHeadBeamAction::Inspected
    }
}

type SeedRetrieval = (
    Vec<NativeStemsHeadSeedScan>,
    Vec<NativeStemsHeadSeedOverlapDecision>,
    Vec<usize>,
);

fn retrieve_seeds(
    input: &InspectionInputs<'_>,
) -> Result<SeedRetrieval, NativeStemsHeadCornerReachabilityError> {
    let head_seed = input
        .head_seed_system
        .heads_by_abscissa
        .get(input.geometry.reference.x_ordinal)
        .filter(|candidate| candidate.sig_ordinal == input.geometry.reference.sig_ordinal)
        .ok_or(NativeStemsHeadCornerReachabilityError::MissingHead {
            system_id: input.system_id,
            sig_ordinal: input.geometry.reference.sig_ordinal,
        })?;
    let corner_seed = head_seed
        .corners_in_constructor_order
        .iter()
        .find(|corner| {
            corner.constructor_ordinal
                == constructor_ordinal(
                    input.geometry.reference.horizontal,
                    input.geometry.reference.vertical,
                )
        })
        .ok_or(NativeStemsHeadCornerReachabilityError::InvalidCornerOrder {
            system_id: input.system_id,
            sig_ordinal: input.geometry.reference.sig_ordinal,
        })?;
    if head_seed
        .corners_in_constructor_order
        .iter()
        .any(|corner| corner.neighbor_seed_ordinals != corner_seed.neighbor_seed_ordinals)
    {
        return Err(NativeStemsHeadCornerReachabilityError::InvalidCornerOrder {
            system_id: input.system_id,
            sig_ordinal: input.geometry.reference.sig_ordinal,
        });
    }
    let kept_positions = input
        .head_seed_system
        .kept_seed_ordinals
        .iter()
        .enumerate()
        .map(|(position, &ordinal)| (ordinal, position))
        .collect::<BTreeMap<_, _>>();
    if kept_positions.len() != input.head_seed_system.kept_seed_ordinals.len() {
        return Err(NativeStemsHeadCornerReachabilityError::InvalidParameters {
            system_id: input.system_id,
        });
    }
    let mut prior_position = None;
    let mut neighbor_seen = BTreeSet::new();
    for ordinal in &corner_seed.neighbor_seed_ordinals {
        let Some(&position) = kept_positions.get(ordinal) else {
            return Err(NativeStemsHeadCornerReachabilityError::MissingSeed {
                system_id: input.system_id,
                free_glyph_ordinal: *ordinal,
            });
        };
        if !neighbor_seen.insert(*ordinal) || prior_position.is_some_and(|prior| prior >= position)
        {
            return Err(NativeStemsHeadCornerReachabilityError::InvalidParameters {
                system_id: input.system_id,
            });
        }
        prior_position = Some(position);
    }

    let mut scans = Vec::with_capacity(corner_seed.neighbor_seed_ordinals.len());
    let mut preliminary = Vec::new();
    for (neighbor_ordinal, &free_glyph_ordinal) in
        corner_seed.neighbor_seed_ordinals.iter().enumerate()
    {
        let glyph = input.seed_map.get(&free_glyph_ordinal).copied().ok_or(
            NativeStemsHeadCornerReachabilityError::MissingSeed {
                system_id: input.system_id,
                free_glyph_ordinal,
            },
        )?;
        let bounds = glyph.bounds;
        let java_bounds = bounds_to_java(bounds).ok_or(
            NativeStemsHeadCornerReachabilityError::InvalidGeometry {
                system_id: input.system_id,
            },
        )?;
        let centroid = glyph_centroid(glyph);
        let mut row = NativeStemsHeadSeedScan {
            neighbor_ordinal,
            free_glyph_ordinal,
            bounds,
            centroid,
            contribution: None,
            line_distance: None,
            preliminary_ordinal: None,
            sorted_preliminary_ordinal: None,
            action: NativeStemsHeadSeedScanAction::OutsideLookup,
        };
        if !convex_quad_intersects_rectangle(input.geometry.final_lookup.quadrilateral, java_bounds)
        {
            scans.push(row);
            continue;
        }
        if let Some(stump) = input.geometry.stump {
            let stump_bounds = bounds_to_java(stump.bounds).ok_or(
                NativeStemsHeadCornerReachabilityError::InvalidGeometry {
                    system_id: input.system_id,
                },
            )?;
            if y_overlap(java_bounds, stump_bounds) > 0 {
                row.action = NativeStemsHeadSeedScanAction::OverlapsStump;
                scans.push(row);
                continue;
            }
        }
        let contribution = y_overlap(input.geometry.y_range, java_bounds);
        row.contribution = Some(contribution);
        if contribution < input.min_seed_contribution {
            row.action = NativeStemsHeadSeedScanAction::InsufficientContribution;
            scans.push(row);
            continue;
        }
        let line_distance = line_pt_distance(
            input.geometry.theoretical_line,
            (f64::from(centroid.0), f64::from(centroid.1)),
        );
        row.line_distance = Some(line_distance);
        if line_distance > input.max_line_seed_dx {
            row.action = NativeStemsHeadSeedScanAction::TooFarFromLine;
            scans.push(row);
            continue;
        }
        let preliminary_ordinal = preliminary.len();
        row.preliminary_ordinal = Some(preliminary_ordinal);
        row.action = NativeStemsHeadSeedScanAction::Preliminary;
        preliminary.push((scans.len(), free_glyph_ordinal, contribution, java_bounds));
        scans.push(row);
    }
    stable_sort_seed_preliminary(&mut preliminary);
    for (sorted_ordinal, &(scan_index, _, _, _)) in preliminary.iter().enumerate() {
        scans[scan_index].sorted_preliminary_ordinal = Some(sorted_ordinal);
    }
    let (overlap_decisions, kept) = select_seed_overlaps(&preliminary);
    Ok((
        scans,
        overlap_decisions,
        kept.into_iter().map(|(ordinal, _)| ordinal).collect(),
    ))
}

type PreliminarySeed = (usize, usize, i32, JavaRectangle);

fn stable_sort_seed_preliminary(preliminary: &mut [PreliminarySeed]) {
    // Java's object sort is stable; the comparator reads contribution only.
    preliminary.sort_by_key(|entry| std::cmp::Reverse(entry.2));
}

fn select_seed_overlaps(
    preliminary: &[PreliminarySeed],
) -> (
    Vec<NativeStemsHeadSeedOverlapDecision>,
    Vec<(usize, JavaRectangle)>,
) {
    let mut decisions = Vec::with_capacity(preliminary.len());
    let mut kept = Vec::new();
    for (sorted_preliminary_ordinal, &(_, free_glyph_ordinal, contribution, bounds)) in
        preliminary.iter().enumerate()
    {
        let first_overlap = kept
            .iter()
            .find(|(_, kept_bounds)| y_overlap(bounds, *kept_bounds) > 0)
            .map(|(ordinal, _)| *ordinal);
        let action = if first_overlap.is_some() {
            NativeStemsHeadSeedOverlapAction::RejectedOverlap
        } else {
            kept.push((free_glyph_ordinal, bounds));
            NativeStemsHeadSeedOverlapAction::Kept
        };
        decisions.push(NativeStemsHeadSeedOverlapDecision {
            sorted_preliminary_ordinal,
            free_glyph_ordinal,
            contribution,
            first_overlapping_kept_seed: first_overlap,
            action,
        });
    }
    (decisions, kept)
}

fn lookup_other_heads(
    input: &InspectionInputs<'_>,
) -> Result<
    (Vec<NativeStemsHeadScan>, Vec<NativeStemsHeadCornerRef>),
    NativeStemsHeadCornerReachabilityError,
> {
    let mut scans = Vec::new();
    let mut targets = Vec::new();
    let lookup_max_x = input
        .geometry
        .final_lookup
        .bounds
        .x
        .wrapping_add(input.geometry.final_lookup.bounds.width);
    let y_last = input.geometry.reference_point.y
        + f64::from(input.geometry.y_direction * input.min_head_head_dy);
    for (x_ordinal, &sig_ordinal) in input.corner_system.heads_by_abscissa.iter().enumerate() {
        let candidate = &input.corner_system.heads_in_sig_order[sig_ordinal];
        let duration = head_duration(input.system_id, candidate.reference, candidate.shape)?;
        let intersects_lookup = convex_quad_intersects_rectangle(
            input.geometry.final_lookup.quadrilateral,
            candidate.bounds,
        );
        let mut row = NativeStemsHeadScan {
            x_ordinal,
            sig_ordinal,
            reference: candidate.reference,
            bounds: candidate.bounds,
            intersects_lookup,
            candidate_after_area: intersects_lookup,
            competing: None,
            duration,
            directed_distance: None,
            corner_checks: Vec::new(),
            action: NativeStemsHeadScanAction::OutsideLookup,
        };
        let is_self = sig_ordinal == input.geometry.reference.sig_ordinal;
        let competing = input.competing.contains(&ordered_pair(
            sig_ordinal,
            input.geometry.reference.sig_ordinal,
        ));
        let center = rectangle_center(candidate.bounds);
        let directed_distance =
            f64::from(input.geometry.y_direction) * (f64::from(center.1) - y_last);
        let action = head_scan_action(
            false,
            intersects_lookup,
            candidate.bounds.x,
            lookup_max_x,
            is_self,
            competing,
            duration == input.duration,
            directed_distance,
        );
        row.action = action;
        if intersects_lookup && !is_self {
            row.competing = Some(competing);
        }
        if intersects_lookup && !is_self && !competing && duration == input.duration {
            row.directed_distance = Some(directed_distance);
        }
        if action != NativeStemsHeadScanAction::CornersChecked {
            scans.push(row);
            if action == NativeStemsHeadScanAction::BreakX {
                break;
            }
            continue;
        }
        for horizontal in [NativeStemHeadSide::Left, NativeStemHeadSide::Right] {
            let corner = candidate
                .corners_in_constructor_order
                .iter()
                .find(|corner| {
                    corner.horizontal == horizontal
                        && corner.vertical == input.geometry.reference.vertical
                })
                .ok_or(NativeStemsHeadCornerReachabilityError::InvalidCornerOrder {
                    system_id: input.system_id,
                    sig_ordinal,
                })?;
            let target = NativeStemsHeadCornerRef {
                head: candidate.reference,
                sig_ordinal,
                x_ordinal,
                horizontal,
                vertical: input.geometry.reference.vertical,
            };
            let contained =
                quad_contains_point(input.geometry.final_lookup.quadrilateral, corner.reference);
            row.corner_checks.push(NativeStemsHeadCornerContainment {
                horizontal,
                target,
                reference_point: corner.reference,
                contained,
            });
            if contained {
                targets.push(target);
            }
        }
        row.action = NativeStemsHeadScanAction::CornersChecked;
        scans.push(row);
    }
    Ok((scans, targets))
}

#[allow(clippy::too_many_arguments)]
fn head_scan_action(
    removed: bool,
    intersects_lookup: bool,
    candidate_x: i32,
    lookup_max_x: i32,
    is_self: bool,
    competing: bool,
    duration_matches: bool,
    directed_distance: f64,
) -> NativeStemsHeadScanAction {
    if removed {
        NativeStemsHeadScanAction::Removed
    } else if !intersects_lookup && candidate_x > lookup_max_x {
        NativeStemsHeadScanAction::BreakX
    } else if !intersects_lookup {
        NativeStemsHeadScanAction::OutsideLookup
    } else if is_self {
        NativeStemsHeadScanAction::SelfHead
    } else if competing {
        NativeStemsHeadScanAction::CompetingHead
    } else if !duration_matches {
        NativeStemsHeadScanAction::DurationMismatch
    } else if directed_distance < 0.0 {
        NativeStemsHeadScanAction::TooNear
    } else {
        NativeStemsHeadScanAction::CornersChecked
    }
}

fn sibling_beams_at(
    system_id: usize,
    point: NativeStemPoint,
    group: &[NativeStemsBeamSource],
    beam_map: &BTreeMap<NativeStemsBeamSource, &NativeStemsBeamStumpBeam>,
    global_slope: f64,
    margin: i32,
) -> Result<Vec<NativeStemsHeadSiblingScan>, NativeStemsHeadCornerReachabilityError> {
    let vertical = Segment {
        x1: point.x,
        y1: point.y,
        x2: point.x - (1_000.0 * global_slope),
        y2: point.y + 1_000.0,
    };
    let mut scans = Vec::with_capacity(group.len());
    let mut accepted = Vec::new();
    for (input_ordinal, &source) in group.iter().enumerate() {
        let beam = beam_map
            .get(&source)
            .copied()
            .ok_or(NativeStemsHeadCornerReachabilityError::MissingBeam { system_id, source })?;
        let cross = generic_intersection(vertical, beam.median);
        let left_limit = beam.median.x1 - f64::from(margin);
        let right_limit = beam.median.x2 + f64::from(margin);
        let is_accepted = left_limit <= cross.x && cross.x <= right_limit;
        if is_accepted {
            accepted.push((scans.len(), cross.y));
        }
        scans.push(NativeStemsHeadSiblingScan {
            input_ordinal,
            beam: source,
            cross,
            left_limit,
            right_limit,
            accepted: is_accepted,
            sorted_ordinal: None,
        });
    }
    accepted.sort_by(|left, right| java_double_order(left.1, right.1));
    for (sorted_ordinal, (scan_index, _)) in accepted.into_iter().enumerate() {
        scans[scan_index].sorted_ordinal = Some(sorted_ordinal);
    }
    Ok(scans)
}

fn find_linker(
    requesting_corner: NativeStemsHeadCornerRef,
    sibling_ordinal: usize,
    find_ordinal: usize,
    theoretical_line: NativeStemLine,
    target_beam: &NativeStemsBeamStumpBeam,
    max_beam_linker_dx: i32,
    arena: &mut NativeStemsHeadBeamArena,
) -> NativeStemsHeadFindLinker {
    let cross = generic_intersection(line_segment(theoretical_line), target_beam.median);
    find_linker_at_cross(
        requesting_corner,
        sibling_ordinal,
        find_ordinal,
        theoretical_line,
        target_beam.source,
        cross,
        max_beam_linker_dx,
        arena,
    )
}

#[allow(clippy::too_many_arguments)]
fn find_linker_at_cross(
    requesting_corner: NativeStemsHeadCornerRef,
    sibling_ordinal: usize,
    find_ordinal: usize,
    theoretical_line: NativeStemLine,
    target_beam: NativeStemsBeamSource,
    cross: NativeStemPoint,
    max_beam_linker_dx: i32,
    arena: &mut NativeStemsHeadBeamArena,
) -> NativeStemsHeadFindLinker {
    let arena_before = arena
        .entries
        .iter()
        .map(|entry| entry.reference)
        .collect::<Vec<_>>();
    let mut best = None;
    let mut best_dx = f64::MAX;
    let mut candidates = Vec::with_capacity(arena.entries.len());
    for (insertion_ordinal, entry) in arena.entries.iter().enumerate() {
        let dx = (entry.reference_point.x - cross.x).abs();
        let best_dx_before = best_dx;
        let replaces_best = best_dx > dx;
        if replaces_best {
            best = Some(entry.reference);
            best_dx = dx;
        }
        candidates.push(NativeStemsHeadFindCandidate {
            insertion_ordinal,
            reference: entry.reference,
            reference_point: entry.reference_point,
            dx,
            best_dx_before,
            replaces_best,
            best_after: best,
            best_dx_after: best_dx,
        });
    }
    let selected_before_threshold = best;
    let result = if best_dx <= f64::from(max_beam_linker_dx) {
        NativeStemsHeadFindResult::Reused(best.expect("finite distance requires an arena entry"))
    } else {
        let reference = NativeStemsBeamBLinkerRef {
            beam: target_beam,
            id: arena.entries.len() + 1,
        };
        arena.entries.push(NativeStemsHeadBeamArenaEntry {
            reference,
            reference_point: cross,
            horizontal_side: None,
            origin: NativeStemsHeadBeamArenaOrigin::HeadReachabilityAnchor {
                requesting_corner,
                sibling_ordinal,
                find_ordinal,
            },
        });
        NativeStemsHeadFindResult::CreatedAnchor(reference)
    };
    let reused_anchor = match result {
        NativeStemsHeadFindResult::Reused(reference) => {
            arena.entries.get(reference.id - 1).is_some_and(|entry| {
                entry.reference == reference
                    && !matches!(entry.origin, NativeStemsHeadBeamArenaOrigin::Initial)
            })
        }
        NativeStemsHeadFindResult::CreatedAnchor(_) => false,
    };
    NativeStemsHeadFindLinker {
        find_ordinal,
        requesting_corner,
        sibling_ordinal,
        target_beam,
        theoretical_line,
        cross,
        arena_before,
        candidates,
        selected_before_threshold,
        best_dx,
        max_beam_linker_dx,
        result,
        reused_anchor,
        arena_after: arena.entries.iter().map(|entry| entry.reference).collect(),
    }
}

fn resolve_stump(
    system_id: usize,
    head_x_ordinal: usize,
    sig_ordinal: usize,
    constructor_ordinal: usize,
    corner: &crate::native_stems_head_stumps::NativeStemsHeadStumpCorner,
    seeds: &BTreeMap<usize, &NativeStemSeedGlyph>,
) -> Result<Option<NativeStemsHeadReachabilityStump>, NativeStemsHeadCornerReachabilityError> {
    match corner.outcome {
        NativeStemsHeadStumpOutcome::None => Ok(None),
        NativeStemsHeadStumpOutcome::Seed { free_glyph_ordinal } => {
            let glyph = seeds.get(&free_glyph_ordinal).copied().ok_or(
                NativeStemsHeadCornerReachabilityError::MissingSeed {
                    system_id,
                    free_glyph_ordinal,
                },
            )?;
            Ok(Some(NativeStemsHeadReachabilityStump {
                source: NativeStemsHeadStumpRef::Seed { free_glyph_ordinal },
                bounds: glyph.bounds,
                weight: glyph.weight,
            }))
        }
        NativeStemsHeadStumpOutcome::Built { .. } => {
            let candidate = corner
                .build
                .as_ref()
                .and_then(|build| build.candidate.as_ref())
                .ok_or(NativeStemsHeadCornerReachabilityError::MissingStump {
                    system_id,
                    sig_ordinal,
                    constructor_ordinal,
                })?;
            Ok(Some(NativeStemsHeadReachabilityStump {
                source: NativeStemsHeadStumpRef::Built {
                    head_x_ordinal,
                    constructor_ordinal,
                },
                bounds: candidate.bounds,
                weight: candidate.weight,
            }))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn lookup_geometry(
    system_id: usize,
    reference: NativeStemPoint,
    out_point: NativeStemPoint,
    in_point: NativeStemPoint,
    x_direction: i32,
    y_direction: i32,
    global_slope: f64,
    y_limit: f64,
) -> Result<NativeStemsHeadLookupGeometry, NativeStemsHeadCornerReachabilityError> {
    let slope = -global_slope;
    let delta_slope = f64::from(x_direction * y_direction) * SLOPE_MARGIN;
    let delta_y = y_limit - out_point.y;
    let inner_limit = NativeStemPoint {
        x: in_point.x + ((slope - delta_slope) * delta_y),
        y: y_limit,
    };
    let outer_limit = NativeStemPoint {
        x: out_point.x + ((slope + delta_slope) * delta_y),
        y: y_limit,
    };
    let quadrilateral = [out_point, in_point, inner_limit, outer_limit];
    let double_bounds = quadrilateral_bounds(quadrilateral)
        .ok_or(NativeStemsHeadCornerReachabilityError::InvalidGeometry { system_id })?;
    let bounds = double_bounds_to_integer(double_bounds);
    let target = target_point(
        reference,
        Segment {
            x1: 0.0,
            y1: y_limit,
            x2: 100.0,
            y2: y_limit,
        },
        global_slope,
    );
    Ok(NativeStemsHeadLookupGeometry {
        y_limit,
        quadrilateral,
        bounds,
        double_bounds,
        theoretical_line: NativeStemLine {
            start: reference,
            stop: target,
        },
    })
}

fn target_point(reference: NativeStemPoint, limit: Segment, global_slope: f64) -> NativeStemPoint {
    generic_intersection(
        Segment {
            x1: reference.x,
            y1: reference.y,
            x2: reference.x - (100.0 * global_slope),
            y2: reference.y + 100.0,
        },
        limit,
    )
}

fn y_at_x(line: Segment, x: f64) -> f64 {
    generic_intersection(
        line,
        Segment {
            x1: x,
            y1: 0.0,
            x2: x,
            y2: 1_000.0,
        },
    )
    .y
}

fn sort_beams_from_ref(
    beams: &mut [&NativeStemsBeamStumpBeam],
    reference: NativeStemPoint,
    y_direction: i32,
    global_slope: f64,
) {
    beams.sort_by(|left, right| {
        let distance = |beam: &NativeStemsBeamStumpBeam| {
            let limit = beam_border(
                beam,
                if y_direction > 0 {
                    NativeStemVerticalSide::Bottom
                } else {
                    NativeStemVerticalSide::Top
                },
            );
            f64::from(y_direction) * (target_point(reference, limit, global_slope).y - reference.y)
        };
        java_double_order(distance(left), distance(right))
    });
}

fn quadrilateral_bounds(
    quadrilateral: [NativeStemPoint; 4],
) -> Option<NativeStemsHeadDoubleBounds> {
    if !quadrilateral
        .iter()
        .flat_map(|point| [point.x, point.y])
        .all(f64::is_finite)
    {
        return None;
    }
    let minimum_x = quadrilateral
        .iter()
        .map(|point| point.x)
        .fold(f64::INFINITY, f64::min);
    let maximum_x = quadrilateral
        .iter()
        .map(|point| point.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let minimum_y = quadrilateral
        .iter()
        .map(|point| point.y)
        .fold(f64::INFINITY, f64::min);
    let maximum_y = quadrilateral
        .iter()
        .map(|point| point.y)
        .fold(f64::NEG_INFINITY, f64::max);
    Some(NativeStemsHeadDoubleBounds {
        x: minimum_x,
        y: minimum_y,
        width: maximum_x - minimum_x,
        height: maximum_y - minimum_y,
    })
}

fn double_bounds_to_integer(bounds: NativeStemsHeadDoubleBounds) -> JavaRectangle {
    let left = bounds.x.floor() as i32;
    let top = bounds.y.floor() as i32;
    let right = (bounds.x + bounds.width).ceil() as i32;
    let bottom = (bounds.y + bounds.height).ceil() as i32;
    JavaRectangle::new(
        left,
        top,
        right.wrapping_sub(left),
        bottom.wrapping_sub(top),
    )
}

const fn horizontal_direction(side: NativeStemHeadSide) -> i32 {
    match side {
        NativeStemHeadSide::Left => -1,
        NativeStemHeadSide::Right => 1,
    }
}

const fn vertical_direction(side: NativeStemVerticalSide) -> i32 {
    match side {
        NativeStemVerticalSide::Top => -1,
        NativeStemVerticalSide::Bottom => 1,
    }
}

const fn opposite_vertical(side: NativeStemVerticalSide) -> NativeStemVerticalSide {
    match side {
        NativeStemVerticalSide::Top => NativeStemVerticalSide::Bottom,
        NativeStemVerticalSide::Bottom => NativeStemVerticalSide::Top,
    }
}

const fn constructor_ordinal(
    horizontal: NativeStemHeadSide,
    vertical: NativeStemVerticalSide,
) -> usize {
    match (horizontal, vertical) {
        (NativeStemHeadSide::Left, NativeStemVerticalSide::Top) => 0,
        (NativeStemHeadSide::Left, NativeStemVerticalSide::Bottom) => 1,
        (NativeStemHeadSide::Right, NativeStemVerticalSide::Top) => 2,
        (NativeStemHeadSide::Right, NativeStemVerticalSide::Bottom) => 3,
    }
}

const fn inspection_ordinal(
    horizontal: NativeStemHeadSide,
    vertical: NativeStemVerticalSide,
) -> usize {
    match (horizontal, vertical) {
        (NativeStemHeadSide::Right, NativeStemVerticalSide::Top) => 0,
        (NativeStemHeadSide::Left, NativeStemVerticalSide::Bottom) => 1,
        (NativeStemHeadSide::Left, NativeStemVerticalSide::Top) => 2,
        (NativeStemHeadSide::Right, NativeStemVerticalSide::Bottom) => 3,
    }
}

fn to_pixels(interline: i32, ratio: f64) -> i32 {
    java_rint(f64::from(interline) * ratio)
}

fn java_rint(value: f64) -> i32 {
    value.round_ties_even() as i32
}

fn bounds_to_java(bounds: Bounds) -> Option<JavaRectangle> {
    Some(JavaRectangle::new(
        i32::try_from(bounds.x).ok()?,
        i32::try_from(bounds.y).ok()?,
        i32::try_from(bounds.width).ok()?,
        i32::try_from(bounds.height).ok()?,
    ))
}

fn rectangle_center(bounds: JavaRectangle) -> (i32, i32) {
    (
        bounds.x.wrapping_add(bounds.width / 2),
        bounds.y.wrapping_add(bounds.height / 2),
    )
}

fn y_overlap(one: JavaRectangle, two: JavaRectangle) -> i32 {
    let common_top = one.y.max(two.y);
    let common_bottom = one
        .y
        .wrapping_add(one.height)
        .min(two.y.wrapping_add(two.height));
    common_bottom.wrapping_sub(common_top)
}

fn glyph_centroid(glyph: &NativeStemSeedGlyph) -> (i32, i32) {
    let mut count = 0_usize;
    let mut x_total = 0_f64;
    let mut y_total = 0_f64;
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
    (
        java_rint(x_total / count as f64),
        java_rint(y_total / count as f64),
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

fn line_segment(line: NativeStemLine) -> Segment {
    Segment {
        x1: line.start.x,
        y1: line.start.y,
        x2: line.stop.x,
        y2: line.stop.y,
    }
}

fn segments_intersect(one: Segment, two: Segment) -> bool {
    relative_ccw(one.x1, one.y1, one.x2, one.y2, two.x1, two.y1)
        * relative_ccw(one.x1, one.y1, one.x2, one.y2, two.x2, two.y2)
        <= 0
        && relative_ccw(two.x1, two.y1, two.x2, two.y2, one.x1, one.y1)
            * relative_ccw(two.x1, two.y1, two.x2, two.y2, one.x2, one.y2)
            <= 0
}

fn relative_ccw(x1: f64, y1: f64, x2: f64, y2: f64, mut px: f64, mut py: f64) -> i32 {
    let x2 = x2 - x1;
    let y2 = y2 - y1;
    px -= x1;
    py -= y1;
    let mut ccw = (px * y2) - (py * x2);
    if ccw == 0.0 {
        ccw = (px * x2) + (py * y2);
        if ccw > 0.0 {
            px -= x2;
            py -= y2;
            ccw = (px * x2) + (py * y2);
            if ccw < 0.0 {
                ccw = 0.0;
            }
        }
    }
    if ccw < 0.0 {
        -1
    } else if ccw > 0.0 {
        1
    } else {
        0
    }
}

fn quad_contains_point(quadrilateral: [NativeStemPoint; 4], point: NativeStemPoint) -> bool {
    let vertices = quadrilateral.map(|point| (point.x, point.y));
    let min_x = vertices
        .iter()
        .map(|point| point.0)
        .fold(f64::INFINITY, f64::min);
    let max_x = vertices
        .iter()
        .map(|point| point.0)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = vertices
        .iter()
        .map(|point| point.1)
        .fold(f64::INFINITY, f64::min);
    let max_y = vertices
        .iter()
        .map(|point| point.1)
        .fold(f64::NEG_INFINITY, f64::max);
    if !(point.x >= min_x && point.x < max_x && point.y >= min_y && point.y < max_y) {
        return false;
    }
    let crossings = vertices
        .iter()
        .copied()
        .zip(vertices.iter().copied().cycle().skip(1))
        .take(vertices.len())
        .filter(|&(start, stop)| openjdk_order1_crosses(start, stop, point.x, point.y))
        .count();
    crossings & 1 != 0
}

fn java_double_order(left: f64, right: f64) -> Ordering {
    if left < right {
        Ordering::Less
    } else if left > right {
        Ordering::Greater
    } else {
        let canonical = |value: f64| {
            if value.is_nan() {
                0x7ff8_0000_0000_0000_u64 as i64
            } else {
                value.to_bits() as i64
            }
        };
        canonical(left).cmp(&canonical(right))
    }
}

fn ordered_pair(first: usize, second: usize) -> NativeStemsHeadCompetingPair {
    if first < second {
        NativeStemsHeadCompetingPair {
            first_sig_ordinal: first,
            second_sig_ordinal: second,
        }
    } else {
        NativeStemsHeadCompetingPair {
            first_sig_ordinal: second,
            second_sig_ordinal: first,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BEAM: NativeStemsBeamSource = NativeStemsBeamSource::RawBeam(7);

    const fn point(x: f64, y: f64) -> NativeStemPoint {
        NativeStemPoint { x, y }
    }

    const fn line() -> NativeStemLine {
        NativeStemLine {
            start: point(5.0, 0.0),
            stop: point(5.0, 10.0),
        }
    }

    const fn corner_ref() -> NativeStemsHeadCornerRef {
        NativeStemsHeadCornerRef {
            head: NativeHeadStaffEpilogRef {
                staff_index: 0,
                head_index: 0,
            },
            sig_ordinal: 0,
            x_ordinal: 0,
            horizontal: NativeStemHeadSide::Right,
            vertical: NativeStemVerticalSide::Top,
        }
    }

    const fn b_ref(id: usize) -> NativeStemsBeamBLinkerRef {
        NativeStemsBeamBLinkerRef { beam: BEAM, id }
    }

    fn initial_entry(id: usize, x: f64) -> NativeStemsHeadBeamArenaEntry {
        NativeStemsHeadBeamArenaEntry {
            reference: b_ref(id),
            reference_point: point(x, 0.0),
            horizontal_side: None,
            origin: NativeStemsHeadBeamArenaOrigin::Initial,
        }
    }

    fn initial_arena() -> NativeStemsHeadBeamArena {
        NativeStemsHeadBeamArena {
            beam: BEAM,
            sig_ordinal: 0,
            initial_b_count: 2,
            entries: vec![initial_entry(1, 0.0), initial_entry(2, 10.0)],
        }
    }

    #[test]
    fn seed_contribution_ties_are_stable_and_overlap_keeps_first() {
        let mut preliminary = vec![
            (0, 10, 5, JavaRectangle::new(0, 0, 1, 5)),
            (1, 11, 5, JavaRectangle::new(0, 3, 1, 4)),
            (2, 12, 7, JavaRectangle::new(0, 10, 1, 2)),
        ];

        stable_sort_seed_preliminary(&mut preliminary);
        assert_eq!(
            preliminary.iter().map(|entry| entry.1).collect::<Vec<_>>(),
            vec![12, 10, 11]
        );

        let (decisions, kept) = select_seed_overlaps(&preliminary);
        assert_eq!(
            kept.iter().map(|entry| entry.0).collect::<Vec<_>>(),
            vec![12, 10]
        );
        assert_eq!(
            decisions[2].action,
            NativeStemsHeadSeedOverlapAction::RejectedOverlap
        );
        assert_eq!(decisions[2].first_overlapping_kept_seed, Some(10));
    }

    #[test]
    fn head_filter_short_circuits_in_java_order() {
        let action =
            |removed, intersects, candidate_x, is_self, competing, duration_matches, distance| {
                head_scan_action(
                    removed,
                    intersects,
                    candidate_x,
                    10,
                    is_self,
                    competing,
                    duration_matches,
                    distance,
                )
            };
        assert_eq!(
            action(true, false, 11, true, true, false, -1.0),
            NativeStemsHeadScanAction::Removed
        );
        assert_eq!(
            action(false, false, 11, true, true, false, -1.0),
            NativeStemsHeadScanAction::BreakX
        );
        assert_eq!(
            action(false, false, 10, true, true, false, -1.0),
            NativeStemsHeadScanAction::OutsideLookup
        );
        assert_eq!(
            action(false, true, 11, true, true, false, -1.0),
            NativeStemsHeadScanAction::SelfHead
        );
        assert_eq!(
            action(false, true, 11, false, true, false, -1.0),
            NativeStemsHeadScanAction::CompetingHead
        );
        assert_eq!(
            action(false, true, 11, false, false, false, -1.0),
            NativeStemsHeadScanAction::DurationMismatch
        );
        assert_eq!(
            action(false, true, 11, false, false, true, -f64::MIN_POSITIVE),
            NativeStemsHeadScanAction::TooNear
        );
        assert_eq!(
            action(false, true, 11, false, false, true, 0.0),
            NativeStemsHeadScanAction::CornersChecked
        );
    }

    #[test]
    fn void_side_skips_only_the_imposed_direction() {
        assert_eq!(
            beam_action_for(Some(BEAM), NativeStemsHeadDuration::Half, 1, 1),
            NativeStemsHeadBeamAction::VoidSideSkipped
        );
        assert_eq!(
            beam_action_for(Some(BEAM), NativeStemsHeadDuration::Half, 1, -1),
            NativeStemsHeadBeamAction::Inspected
        );
        assert_eq!(
            beam_action_for(Some(BEAM), NativeStemsHeadDuration::Quarter, 1, 1),
            NativeStemsHeadBeamAction::Inspected
        );
        assert_eq!(
            beam_action_for(None, NativeStemsHeadDuration::Half, 1, -1),
            NativeStemsHeadBeamAction::NoTargetBeam
        );
    }

    #[test]
    fn find_linker_uses_strict_ties_inclusive_threshold_and_immediate_anchor() {
        let mut inclusive_arena = initial_arena();
        let inclusive = find_linker_at_cross(
            corner_ref(),
            0,
            0,
            line(),
            BEAM,
            point(5.0, 0.0),
            5,
            &mut inclusive_arena,
        );
        assert_eq!(
            inclusive.result,
            NativeStemsHeadFindResult::Reused(b_ref(1))
        );
        assert!(inclusive.candidates[0].replaces_best);
        assert!(!inclusive.candidates[1].replaces_best);
        assert_eq!(inclusive_arena.entries.len(), 2);

        let mut created_arena = initial_arena();
        let created = find_linker_at_cross(
            corner_ref(),
            0,
            0,
            line(),
            BEAM,
            point(5.0, 0.0),
            4,
            &mut created_arena,
        );
        assert_eq!(
            created.result,
            NativeStemsHeadFindResult::CreatedAnchor(b_ref(3))
        );
        assert_eq!(created_arena.entries.len(), 3);

        let reused = find_linker_at_cross(
            corner_ref(),
            1,
            1,
            line(),
            BEAM,
            point(5.0, 0.0),
            0,
            &mut created_arena,
        );
        assert_eq!(reused.result, NativeStemsHeadFindResult::Reused(b_ref(3)));
        assert!(reused.reused_anchor);
        assert_eq!(created_arena.entries.len(), 3);
    }

    #[test]
    fn corner_inspection_order_is_tr_bl_tl_br() {
        let mut sides = vec![
            (NativeStemHeadSide::Left, NativeStemVerticalSide::Top),
            (NativeStemHeadSide::Left, NativeStemVerticalSide::Bottom),
            (NativeStemHeadSide::Right, NativeStemVerticalSide::Top),
            (NativeStemHeadSide::Right, NativeStemVerticalSide::Bottom),
        ];
        assert_eq!(
            sides
                .iter()
                .map(|&(horizontal, vertical)| constructor_ordinal(horizontal, vertical))
                .collect::<Vec<_>>(),
            vec![0, 1, 2, 3]
        );
        sides.sort_by_key(|&(horizontal, vertical)| inspection_ordinal(horizontal, vertical));
        assert_eq!(
            sides,
            vec![
                (NativeStemHeadSide::Right, NativeStemVerticalSide::Top),
                (NativeStemHeadSide::Left, NativeStemVerticalSide::Bottom),
                (NativeStemHeadSide::Left, NativeStemVerticalSide::Top),
                (NativeStemHeadSide::Right, NativeStemVerticalSide::Bottom),
            ]
        );
    }

    #[test]
    fn corner_lookup_contains_uses_openjdk_half_open_crossings() {
        let square = [
            point(0.0, 0.0),
            point(10.0, 0.0),
            point(10.0, 10.0),
            point(0.0, 10.0),
        ];
        assert!(quad_contains_point(square, point(0.0, 5.0)));
        assert!(quad_contains_point(square, point(5.0, 0.0)));
        assert!(!quad_contains_point(square, point(10.0, 5.0)));
        assert!(!quad_contains_point(square, point(5.0, 10.0)));
    }

    #[test]
    fn beam_arena_origin_and_anchor_flags_must_match_chronology() {
        let malformed = crate::native_stems_beam_reachability::NativeStemsBeamArena {
            beam: BEAM,
            sig_ordinal: 0,
            initial_b_count: 0,
            all_b_linkers: vec![
                crate::native_stems_beam_reachability::NativeStemsBeamArenaEntry {
                    reference: b_ref(1),
                    reference_point: point(0.0, 0.0),
                    horizontal_side: None,
                    is_anchor: false,
                    origin: NativeStemsBeamArenaOrigin::Constructor(
                        crate::native_stems_beam_vlinkers::NativeStemsBeamBLinkerOrigin::Orphan {
                            side: NativeStemHeadSide::Left,
                        },
                    ),
                },
            ],
        };
        assert_eq!(
            convert_beam_arena(3, &malformed),
            Err(NativeStemsHeadCornerReachabilityError::InvalidBeamArena {
                system_id: 3,
                source: BEAM,
            })
        );
    }
}
