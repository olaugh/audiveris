// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact `BeamLinker.inspectVLinkers` reachability before `StemBuilder`.
//!
//! All beam and head linkers already exist at this boundary.  Java visits live
//! beams in stable abscissa order, then each constructor `BLinker` and its
//! `TOP`/`BOTTOM` V linkers in insertion/enum order.  A V lookup can append an
//! anchor B linker to a *different* beam; the append is immediately visible to
//! later lookups.  This module models that shared append-only arena and stops
//! before a `StemBuilder`, seed mutation, or SIG relation is created.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use audiveris_image::beam_structure::Segment;

use crate::{
    beam_inters::{BeamKind, openjdk_order1_crosses},
    head_scanner_slices::JavaRectangle,
    head_template::HeadTemplateShape,
    native_heads_staff_epilog::NativeHeadStaffEpilogRef,
    native_stems_beam_stumps::{
        NativeStemsBeamSource, NativeStemsBeamStumpBeam, NativeStemsBeamStumpRecognition,
    },
    native_stems_beam_vlinkers::{
        NativeStemsBeamBLinkerOrigin, NativeStemsBeamBLinkerRef, NativeStemsBeamVLinkerRecognition,
        NativeStemsBeamVLinkerRef, beam_border, convex_quad_intersects_rectangle,
        generic_intersection,
    },
    native_stems_head_corners::{NativeStemsHeadCorner, NativeStemsHeadCornerRecognition},
    recognize::NativeBeamRecognition,
    stems_step::{NativeStemHeadSide, NativeStemLine, NativeStemPoint, NativeStemVerticalSide},
};

const BEAM_SEED_PROFILE: i32 = 3;
const BEAM_SIDE_PROFILE: i32 = 4;
const MAX_BEAM_LINKER_DX_RATIO: f64 = 0.25;
const MIN_BEAM_HEAD_DY_RATIO: f64 = 1.0;
const ALLOW_SMALL_HEAD_ON_STANDARD_BEAM: bool = false;

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamReachabilityRecognition {
    pub systems: Vec<NativeStemsBeamReachabilitySystem>,
    pub beam_inspection_count: usize,
    pub b_visit_count: usize,
    pub v_inspection_count: usize,
    pub sibling_check_count: usize,
    pub sibling_hit_count: usize,
    pub find_linker_request_count: usize,
    pub reused_b_linker_count: usize,
    pub reused_anchor_count: usize,
    pub created_anchor_count: usize,
    pub head_scan_count: usize,
    pub head_candidate_count: usize,
    pub head_corner_check_count: usize,
    pub accepted_head_corner_count: usize,
    pub small_head_count: usize,
    pub small_beam_count: usize,
    pub size_rejection_count: usize,
    pub distance_rejection_count: usize,
    pub competing_removal_count: usize,
    pub lookup_contained_corner_count: usize,
    pub void_side_rejection_count: usize,
    pub target_count: usize,
    pub beam_target_count: usize,
    pub head_target_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamReachabilitySystem {
    pub system_id: usize,
    pub interline: i32,
    pub global_slope: f64,
    pub max_beam_side_dx: i32,
    pub max_beam_linker_dx: i32,
    pub min_beam_head_dy: i32,
    pub allow_small_head_on_standard_beam: bool,
    pub beam_sources_in_inspection_order: Vec<NativeStemsBeamSource>,
    pub groups_in_source_order: Vec<Vec<NativeStemsBeamSource>>,
    pub heads_in_sig_order: Vec<NativeStemsBeamHeadCandidateRef>,
    pub heads_in_abscissa_order: Vec<NativeStemsBeamHeadCandidateRef>,
    pub beam_inspections: Vec<NativeStemsBeamInspection>,
    pub final_beam_arenas: Vec<NativeStemsBeamArena>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamArena {
    pub beam: NativeStemsBeamSource,
    pub sig_ordinal: usize,
    pub initial_b_count: usize,
    pub all_b_linkers: Vec<NativeStemsBeamArenaEntry>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamArenaEntry {
    pub reference: NativeStemsBeamBLinkerRef,
    pub reference_point: NativeStemPoint,
    pub horizontal_side: Option<NativeStemHeadSide>,
    pub is_anchor: bool,
    pub origin: NativeStemsBeamArenaOrigin,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemsBeamArenaOrigin {
    Constructor(NativeStemsBeamBLinkerOrigin),
    Anchor {
        requesting_v: NativeStemsBeamVLinkerRef,
        sibling_ordinal: usize,
        request_ordinal: usize,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamInspection {
    pub inspection_ordinal: usize,
    pub beam: NativeStemsBeamSource,
    pub kind: BeamKind,
    pub is_small_beam: bool,
    pub all_b_before: Vec<NativeStemsBeamBLinkerRef>,
    /// Arena state immediately after this beam's own V inspections. Later
    /// beams may still append anchors back to this beam.
    pub all_b_after: Vec<NativeStemsBeamBLinkerRef>,
    pub b_visits: Vec<NativeStemsBeamBVisit>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamBVisit {
    pub visit_ordinal: usize,
    pub reference: NativeStemsBeamBLinkerRef,
    pub is_anchor: bool,
    pub skipped_anchor: bool,
    pub max_stem_profile: Option<i32>,
    pub v_inspections: Vec<NativeStemsBeamVInspection>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamVInspection {
    pub inspection_ordinal: usize,
    pub reference: NativeStemsBeamVLinkerRef,
    pub y_direction: i32,
    pub stopping_head_side: NativeStemHeadSide,
    pub max_stem_profile: i32,
    pub theoretical_line: NativeStemLine,
    pub lookup_quadrilateral: [NativeStemPoint; 4],
    pub lookup_bounds: JavaRectangle,
    pub sibling_checks: Vec<NativeStemsBeamReachabilitySiblingCheck>,
    pub siblings: Vec<NativeStemsBeamReachabilitySiblingHit>,
    pub beam_decisions: Vec<NativeStemsBeamTargetDecision>,
    pub head_search: NativeStemsBeamHeadSearch,
    pub beam_targets: Vec<NativeStemsBeamBLinkerRef>,
    pub head_targets: Vec<NativeStemsBeamHeadCornerRef>,
    pub ordered_targets: Vec<NativeStemsBeamReachabilityTarget>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsBeamReachabilitySiblingCheck {
    pub input_ordinal: usize,
    pub beam: NativeStemsBeamSource,
    pub cross: NativeStemPoint,
    pub left_limit: f64,
    pub right_limit: f64,
    pub accepted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsBeamReachabilitySiblingHit {
    pub input_ordinal: usize,
    pub sorted_ordinal: usize,
    pub beam: NativeStemsBeamSource,
    pub cross: NativeStemPoint,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamTargetDecision {
    pub sibling_ordinal: usize,
    pub beam: NativeStemsBeamSource,
    pub same_beam: bool,
    pub glyph_present: Option<bool>,
    pub same_glyph: Option<bool>,
    pub action: NativeStemsBeamTargetAction,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeStemsBeamTargetAction {
    SelfBeam,
    MissingGlyph,
    SameGlyph,
    FindLinker(Box<NativeStemsBeamFindLinker>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamFindLinker {
    pub request_ordinal: usize,
    pub requesting_v: NativeStemsBeamVLinkerRef,
    pub target_beam: NativeStemsBeamSource,
    pub theoretical_line: NativeStemLine,
    pub cross: NativeStemPoint,
    pub arena_count_before: usize,
    pub candidates: Vec<NativeStemsBeamFindCandidate>,
    pub selected_before_threshold: Option<NativeStemsBeamBLinkerRef>,
    pub best_dx: f64,
    pub max_beam_linker_dx: i32,
    pub result: NativeStemsBeamFindResult,
    pub reused_anchor: bool,
    pub arena_count_after: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsBeamFindCandidate {
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
pub enum NativeStemsBeamFindResult {
    Reused(NativeStemsBeamBLinkerRef),
    CreatedAnchor(NativeStemsBeamBLinkerRef),
}

impl NativeStemsBeamFindResult {
    #[must_use]
    pub const fn reference(self) -> NativeStemsBeamBLinkerRef {
        match self {
            Self::Reused(reference) | Self::CreatedAnchor(reference) => reference,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamHeadSearch {
    pub skipped_for_empty_siblings: bool,
    pub last_sibling: Option<NativeStemsBeamSource>,
    pub last_border_side: Option<NativeStemVerticalSide>,
    pub last_border: Option<Segment>,
    pub y_last_border: Option<f64>,
    pub scans: Vec<NativeStemsBeamHeadScan>,
    pub candidates_before_competing_removal: Vec<NativeStemsBeamHeadCandidateRef>,
    pub competing_removals: Vec<NativeStemsBeamCompetingRemoval>,
    pub candidates_after_competing_removal: Vec<NativeStemsBeamHeadCandidateRef>,
    pub decisions: Vec<NativeStemsBeamHeadDecision>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamHeadCandidateRef {
    pub head: NativeHeadStaffEpilogRef,
    pub sig_ordinal: usize,
    pub x_ordinal: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamHeadScan {
    pub x_ordinal: usize,
    pub head: NativeHeadStaffEpilogRef,
    pub sig_ordinal: usize,
    pub bounds: JavaRectangle,
    pub intersects_lookup_area: bool,
    pub beyond_lookup_max_x: bool,
    pub breaks_after: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamCompetingRemoval {
    pub sibling_ordinal: usize,
    pub sibling: NativeStemsBeamSource,
    /// Empty at this live pre-StemBuilder boundary: HEADS never registers a
    /// head which overlapped a beam, and beam exclusions address beams/hooks.
    pub removed_heads: Vec<NativeStemsBeamHeadCandidateRef>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamHeadDecision {
    pub candidate: NativeStemsBeamHeadCandidateRef,
    pub shape: HeadTemplateShape,
    pub bounds: JavaRectangle,
    pub is_small_head: bool,
    pub is_small_beam: bool,
    pub allow_small_head_on_standard_beam: bool,
    pub size_accepted: bool,
    pub center_y: Option<i32>,
    pub y_last_border: Option<f64>,
    pub directed_dy: Option<f64>,
    pub min_beam_head_dy: Option<i32>,
    pub distance_accepted: Option<bool>,
    pub imposed_void_head_side: NativeStemHeadSide,
    pub action: NativeStemsBeamHeadDecisionAction,
    pub corner_checks: Vec<NativeStemsBeamHeadCornerCheck>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamHeadDecisionAction {
    SizeRejected,
    DistanceRejected,
    CornersChecked,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsBeamHeadCornerCheck {
    pub inspection_ordinal: usize,
    pub reference: NativeStemsBeamHeadCornerRef,
    pub point: NativeStemPoint,
    pub lookup_contains: bool,
    pub is_half_head: bool,
    pub imposed_void_head_side: NativeStemHeadSide,
    pub half_head_side_accepted: Option<bool>,
    pub action: NativeStemsBeamHeadCornerAction,
    pub accepted: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamHeadCornerAction {
    OutsideLookup,
    VoidSideRejected,
    Accepted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamHeadCornerRef {
    pub head: NativeHeadStaffEpilogRef,
    pub sig_ordinal: usize,
    pub x_ordinal: usize,
    pub horizontal: NativeStemHeadSide,
    pub vertical: NativeStemVerticalSide,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamReachabilityTarget {
    Beam(NativeStemsBeamBLinkerRef),
    Head(NativeStemsBeamHeadCornerRef),
}

#[derive(Debug, Clone, PartialEq)]
pub enum NativeStemsBeamReachabilityError {
    SystemOrder {
        beams: Vec<usize>,
        stumps: Vec<usize>,
        constructors: Vec<usize>,
        heads: Vec<usize>,
    },
    MissingGroupSystem(usize),
    InvalidGroupMember {
        system_id: usize,
        member_ordinal: usize,
    },
    MissingBeam {
        system_id: usize,
        source: NativeStemsBeamSource,
    },
    DuplicateBeam {
        system_id: usize,
        source: NativeStemsBeamSource,
    },
    MissingConstructor {
        system_id: usize,
        source: NativeStemsBeamSource,
    },
    DuplicateConstructor {
        system_id: usize,
        source: NativeStemsBeamSource,
    },
    BeamGroupMismatch {
        system_id: usize,
        source: NativeStemsBeamSource,
    },
    InvalidBLinker {
        system_id: usize,
        reference: NativeStemsBeamBLinkerRef,
    },
    InvalidVLinker {
        system_id: usize,
        reference: NativeStemsBeamVLinkerRef,
    },
    DuplicateVLinker {
        system_id: usize,
        reference: NativeStemsBeamVLinkerRef,
    },
    InvalidHeadOrder {
        system_id: usize,
    },
    InvalidHeadCorner {
        system_id: usize,
        head: NativeHeadStaffEpilogRef,
        horizontal: NativeStemHeadSide,
        vertical: NativeStemVerticalSide,
    },
    UnsupportedHeadShape {
        system_id: usize,
        head: NativeHeadStaffEpilogRef,
        shape: HeadTemplateShape,
    },
    InvalidParameters {
        system_id: usize,
        interline: i32,
        max_beam_side_dx: i32,
    },
    InvalidGeometry {
        system_id: usize,
    },
}

impl fmt::Display for NativeStemsBeamReachabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid native beam reachability input: {self:?}"
        )
    }
}

impl Error for NativeStemsBeamReachabilityError {}

/// Materialize Java's beam-driven V-linker inspection and no later mutation.
pub fn materialize_native_stems_beam_reachability(
    beams: &NativeBeamRecognition,
    beam_stumps: &NativeStemsBeamStumpRecognition,
    constructors: &NativeStemsBeamVLinkerRecognition,
    head_corners: &NativeStemsHeadCornerRecognition,
) -> Result<NativeStemsBeamReachabilityRecognition, NativeStemsBeamReachabilityError> {
    let beam_ids = beams
        .group_memberships
        .iter()
        .map(|system| system.system_id)
        .collect::<Vec<_>>();
    let stump_ids = beam_stumps
        .systems
        .iter()
        .map(|system| system.system_id)
        .collect::<Vec<_>>();
    let constructor_ids = constructors
        .systems
        .iter()
        .map(|system| system.system_id)
        .collect::<Vec<_>>();
    let head_ids = head_corners
        .systems
        .iter()
        .map(|system| system.system_id)
        .collect::<Vec<_>>();
    if beam_ids != stump_ids || stump_ids != constructor_ids || constructor_ids != head_ids {
        return Err(NativeStemsBeamReachabilityError::SystemOrder {
            beams: beam_ids,
            stumps: stump_ids,
            constructors: constructor_ids,
            heads: head_ids,
        });
    }

    let mut totals = ReachabilityTotals::default();
    let mut systems = Vec::with_capacity(stump_ids.len());
    for (((stump_system, constructor_system), head_system), group_state) in beam_stumps
        .systems
        .iter()
        .zip(&constructors.systems)
        .zip(&head_corners.systems)
        .zip(&beams.group_memberships)
    {
        systems.push(materialize_system(
            beams,
            stump_system,
            constructor_system,
            head_system,
            group_state,
            &mut totals,
        )?);
    }

    Ok(NativeStemsBeamReachabilityRecognition {
        systems,
        beam_inspection_count: totals.beam_inspections,
        b_visit_count: totals.b_visits,
        v_inspection_count: totals.v_inspections,
        sibling_check_count: totals.sibling_checks,
        sibling_hit_count: totals.sibling_hits,
        find_linker_request_count: totals.find_requests,
        reused_b_linker_count: totals.reused_b_linkers,
        reused_anchor_count: totals.reused_anchors,
        created_anchor_count: totals.created_anchors,
        head_scan_count: totals.head_scans,
        head_candidate_count: totals.head_candidates,
        head_corner_check_count: totals.head_corner_checks,
        accepted_head_corner_count: totals.accepted_head_corners,
        small_head_count: totals.small_heads,
        small_beam_count: totals.small_beams,
        size_rejection_count: totals.size_rejections,
        distance_rejection_count: totals.distance_rejections,
        competing_removal_count: totals.competing_removals,
        lookup_contained_corner_count: totals.lookup_contained_corners,
        void_side_rejection_count: totals.void_side_rejections,
        target_count: totals.beam_targets + totals.head_targets,
        beam_target_count: totals.beam_targets,
        head_target_count: totals.head_targets,
    })
}

#[derive(Default)]
struct ReachabilityTotals {
    beam_inspections: usize,
    b_visits: usize,
    v_inspections: usize,
    sibling_checks: usize,
    sibling_hits: usize,
    find_requests: usize,
    reused_b_linkers: usize,
    reused_anchors: usize,
    created_anchors: usize,
    head_scans: usize,
    head_candidates: usize,
    head_corner_checks: usize,
    accepted_head_corners: usize,
    small_heads: usize,
    small_beams: usize,
    size_rejections: usize,
    distance_rejections: usize,
    competing_removals: usize,
    lookup_contained_corners: usize,
    void_side_rejections: usize,
    beam_targets: usize,
    head_targets: usize,
}

fn materialize_system(
    beams: &NativeBeamRecognition,
    stump_system: &crate::native_stems_beam_stumps::NativeStemsBeamStumpSystem,
    constructor_system: &crate::native_stems_beam_vlinkers::NativeStemsBeamVLinkerSystem,
    head_system: &crate::native_stems_head_corners::NativeStemsHeadCornerSystem,
    group_state: &crate::recognize::NativeBeamGroupMembership,
    totals: &mut ReachabilityTotals,
) -> Result<NativeStemsBeamReachabilitySystem, NativeStemsBeamReachabilityError> {
    let system_id = stump_system.system_id;
    let interline = stump_system.interline;
    if interline <= 0
        || stump_system.max_beam_side_dx < 0
        || constructor_system.interline != interline
        || head_system.interline != interline
        || !constructor_system.global_slope.is_finite()
    {
        return Err(NativeStemsBeamReachabilityError::InvalidParameters {
            system_id,
            interline,
            max_beam_side_dx: stump_system.max_beam_side_dx,
        });
    }
    let max_beam_linker_dx = to_pixels(interline, MAX_BEAM_LINKER_DX_RATIO);
    let min_beam_head_dy = to_pixels(interline, MIN_BEAM_HEAD_DY_RATIO);

    let live_sources = constructor_system
        .constructors
        .iter()
        .filter(|constructor| constructor.survives_constructor_loop)
        .map(|constructor| constructor.source)
        .collect::<Vec<_>>();
    let live_set = live_sources.iter().copied().collect::<BTreeSet<_>>();
    if live_set.len() != live_sources.len() {
        let duplicate = live_sources
            .iter()
            .copied()
            .find(|source| {
                live_sources
                    .iter()
                    .filter(|&&other| other == *source)
                    .count()
                    > 1
            })
            .unwrap_or(NativeStemsBeamSource::RawBeam(usize::MAX));
        return Err(NativeStemsBeamReachabilityError::DuplicateConstructor {
            system_id,
            source: duplicate,
        });
    }

    let mut beam_map = BTreeMap::new();
    for beam in &stump_system.beams_by_abscissa {
        if beam_map.insert(beam.source, beam).is_some() {
            return Err(NativeStemsBeamReachabilityError::DuplicateBeam {
                system_id,
                source: beam.source,
            });
        }
    }
    for &source in &live_sources {
        if !beam_map.contains_key(&source) {
            return Err(NativeStemsBeamReachabilityError::MissingBeam { system_id, source });
        }
    }

    let raw_groups = beam_group_sources(beams, system_id, group_state)?;
    let groups = raw_groups
        .into_iter()
        .map(|group| {
            group
                .into_iter()
                .filter(|source| live_set.contains(source))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    for &source in &live_sources {
        let beam = beam_map[&source];
        let occurrences = groups
            .iter()
            .enumerate()
            .filter(|(_, group)| group.contains(&source))
            .map(|(ordinal, _)| ordinal)
            .collect::<Vec<_>>();
        if occurrences.as_slice() != [beam.group_ordinal] {
            return Err(NativeStemsBeamReachabilityError::BeamGroupMismatch { system_id, source });
        }
    }

    let constructor_map = constructor_system
        .constructors
        .iter()
        .map(|constructor| (constructor.source, constructor))
        .collect::<BTreeMap<_, _>>();
    if constructor_map.len() != constructor_system.constructors.len() {
        let duplicate = constructor_system
            .constructors
            .iter()
            .map(|constructor| constructor.source)
            .find(|source| {
                constructor_system
                    .constructors
                    .iter()
                    .filter(|constructor| constructor.source == *source)
                    .count()
                    > 1
            })
            .unwrap_or(NativeStemsBeamSource::RawBeam(usize::MAX));
        return Err(NativeStemsBeamReachabilityError::DuplicateConstructor {
            system_id,
            source: duplicate,
        });
    }

    validate_head_order(system_id, head_system)?;
    let heads_in_sig_order = head_system
        .heads_in_sig_order
        .iter()
        .enumerate()
        .map(|(sig_ordinal, head)| NativeStemsBeamHeadCandidateRef {
            head: head.reference,
            sig_ordinal,
            x_ordinal: head_system
                .heads_by_abscissa
                .iter()
                .position(|&index| index == sig_ordinal)
                .expect("validated head permutation"),
        })
        .collect::<Vec<_>>();
    let heads_in_abscissa_order = head_system
        .heads_by_abscissa
        .iter()
        .enumerate()
        .map(
            |(x_ordinal, &sig_ordinal)| NativeStemsBeamHeadCandidateRef {
                head: head_system.heads_in_sig_order[sig_ordinal].reference,
                sig_ordinal,
                x_ordinal,
            },
        )
        .collect::<Vec<_>>();

    // The current native HEADS product contains exactly the standard oval
    // stem-head classes.  Do not silently reinterpret a stemless class as a
    // small/standard head if an upstream producer violates that boundary.
    for head in &head_system.heads_in_sig_order {
        if !matches!(
            head.shape,
            HeadTemplateShape::NoteheadBlack | HeadTemplateShape::NoteheadVoid
        ) {
            return Err(NativeStemsBeamReachabilityError::UnsupportedHeadShape {
                system_id,
                head: head.reference,
                shape: head.shape,
            });
        }
    }

    let mut arenas = Vec::with_capacity(live_sources.len());
    let mut arena_indices = BTreeMap::new();
    let mut v_linkers = BTreeMap::new();
    for &source in &live_sources {
        let constructor = constructor_map
            .get(&source)
            .copied()
            .ok_or(NativeStemsBeamReachabilityError::MissingConstructor { system_id, source })?;
        let mut entries = Vec::with_capacity(constructor.b_linkers.len());
        for (index, b_linker) in constructor.b_linkers.iter().enumerate() {
            if b_linker.reference.beam != source || b_linker.reference.id != index + 1 {
                return Err(NativeStemsBeamReachabilityError::InvalidBLinker {
                    system_id,
                    reference: b_linker.reference,
                });
            }
            let mut sides = BTreeSet::new();
            for v_linker in &b_linker.v_linkers {
                if !matches!(v_linker.y_direction, -1 | 1)
                    || v_linker.reference.b_linker != b_linker.reference
                    || v_linker.reference.side
                        != if v_linker.y_direction < 0 {
                            NativeStemVerticalSide::Top
                        } else {
                            NativeStemVerticalSide::Bottom
                        }
                    || v_linker.stopping_head_side != imposed_void_head_side(v_linker.y_direction)
                    || !sides.insert(v_linker.reference.side)
                {
                    return Err(NativeStemsBeamReachabilityError::InvalidVLinker {
                        system_id,
                        reference: v_linker.reference,
                    });
                }
                if v_linkers
                    .insert(
                        (source, b_linker.reference.id, v_linker.reference.side),
                        v_linker,
                    )
                    .is_some()
                {
                    return Err(NativeStemsBeamReachabilityError::DuplicateVLinker {
                        system_id,
                        reference: v_linker.reference,
                    });
                }
            }
            entries.push(NativeStemsBeamArenaEntry {
                reference: b_linker.reference,
                reference_point: b_linker.reference_point,
                horizontal_side: b_linker.horizontal_side,
                is_anchor: false,
                origin: NativeStemsBeamArenaOrigin::Constructor(b_linker.origin.clone()),
            });
        }
        let index = arenas.len();
        arena_indices.insert(source, index);
        arenas.push(NativeStemsBeamArena {
            beam: source,
            sig_ordinal: beam_map[&source].sig_ordinal,
            initial_b_count: entries.len(),
            all_b_linkers: entries,
        });
    }

    let mut beam_inspections = Vec::with_capacity(live_sources.len());
    let mut system_v_ordinal = 0_usize;
    let mut request_ordinal = 0_usize;
    for (inspection_ordinal, &source) in live_sources.iter().enumerate() {
        let beam = beam_map[&source];
        let is_small_beam = beam.kind == BeamKind::SmallBeam;
        totals.small_beams += usize::from(is_small_beam);
        let arena_index = arena_indices[&source];
        let all_b_before = arenas[arena_index]
            .all_b_linkers
            .iter()
            .map(|entry| entry.reference)
            .collect::<Vec<_>>();
        // This beam cannot append an anchor to itself: filterBeams rejects the
        // source and aliases sharing its glyph.  Iterate the exact start list,
        // including anchors appended by earlier beam inspections.
        let current_entries = arenas[arena_index].all_b_linkers.clone();
        let mut b_visits = Vec::with_capacity(current_entries.len());
        for (visit_ordinal, entry) in current_entries.iter().enumerate() {
            totals.b_visits += 1;
            if entry.is_anchor {
                b_visits.push(NativeStemsBeamBVisit {
                    visit_ordinal,
                    reference: entry.reference,
                    is_anchor: true,
                    skipped_anchor: true,
                    max_stem_profile: None,
                    v_inspections: Vec::new(),
                });
                continue;
            }
            let max_stem_profile = if entry.horizontal_side.is_some() {
                BEAM_SIDE_PROFILE
            } else {
                BEAM_SEED_PROFILE
            };
            let mut inspections = Vec::new();
            for side in [NativeStemVerticalSide::Top, NativeStemVerticalSide::Bottom] {
                let Some(v_linker) = v_linkers.get(&(source, entry.reference.id, side)).copied()
                else {
                    continue;
                };
                let inspection = inspect_v_linker(
                    system_id,
                    system_v_ordinal,
                    v_linker,
                    max_stem_profile,
                    beam,
                    &groups,
                    &beam_map,
                    head_system,
                    &heads_in_abscissa_order,
                    max_beam_linker_dx,
                    min_beam_head_dy,
                    constructor_system.global_slope,
                    stump_system.max_beam_side_dx,
                    &mut arenas,
                    &arena_indices,
                    &mut request_ordinal,
                    totals,
                )?;
                system_v_ordinal += 1;
                inspections.push(inspection);
            }
            b_visits.push(NativeStemsBeamBVisit {
                visit_ordinal,
                reference: entry.reference,
                is_anchor: false,
                skipped_anchor: false,
                max_stem_profile: Some(max_stem_profile),
                v_inspections: inspections,
            });
        }
        totals.beam_inspections += 1;
        let all_b_after = arenas[arena_index]
            .all_b_linkers
            .iter()
            .map(|entry| entry.reference)
            .collect();
        beam_inspections.push(NativeStemsBeamInspection {
            inspection_ordinal,
            beam: source,
            kind: beam.kind,
            is_small_beam,
            all_b_before,
            all_b_after,
            b_visits,
        });
    }

    Ok(NativeStemsBeamReachabilitySystem {
        system_id,
        interline,
        global_slope: constructor_system.global_slope,
        max_beam_side_dx: stump_system.max_beam_side_dx,
        max_beam_linker_dx,
        min_beam_head_dy,
        allow_small_head_on_standard_beam: ALLOW_SMALL_HEAD_ON_STANDARD_BEAM,
        beam_sources_in_inspection_order: live_sources,
        groups_in_source_order: groups,
        heads_in_sig_order,
        heads_in_abscissa_order,
        beam_inspections,
        final_beam_arenas: arenas,
    })
}

#[allow(clippy::too_many_arguments)]
fn inspect_v_linker(
    system_id: usize,
    inspection_ordinal: usize,
    v_linker: &crate::native_stems_beam_vlinkers::NativeStemsBeamVLinker,
    max_stem_profile: i32,
    source_beam: &NativeStemsBeamStumpBeam,
    groups: &[Vec<NativeStemsBeamSource>],
    beam_map: &BTreeMap<NativeStemsBeamSource, &NativeStemsBeamStumpBeam>,
    head_system: &crate::native_stems_head_corners::NativeStemsHeadCornerSystem,
    heads_in_abscissa_order: &[NativeStemsBeamHeadCandidateRef],
    max_beam_linker_dx: i32,
    min_beam_head_dy: i32,
    global_slope: f64,
    max_beam_side_dx: i32,
    arenas: &mut [NativeStemsBeamArena],
    arena_indices: &BTreeMap<NativeStemsBeamSource, usize>,
    request_ordinal: &mut usize,
    totals: &mut ReachabilityTotals,
) -> Result<NativeStemsBeamVInspection, NativeStemsBeamReachabilityError> {
    totals.v_inspections += 1;
    let geometry = &v_linker.final_geometry;
    if !geometry
        .quadrilateral
        .iter()
        .flat_map(|point| [point.x, point.y])
        .chain([
            geometry.theoretical_line.start.x,
            geometry.theoretical_line.start.y,
            geometry.theoretical_line.stop.x,
            geometry.theoretical_line.stop.y,
        ])
        .all(f64::is_finite)
    {
        return Err(NativeStemsBeamReachabilityError::InvalidGeometry { system_id });
    }
    let reference_point = geometry.theoretical_line.start;
    let group = groups.get(source_beam.group_ordinal).ok_or(
        NativeStemsBeamReachabilityError::BeamGroupMismatch {
            system_id,
            source: source_beam.source,
        },
    )?;
    let (sibling_checks, siblings) = sibling_beams_at(
        system_id,
        reference_point,
        group,
        beam_map,
        global_slope,
        max_beam_side_dx,
    )?;
    totals.sibling_checks += sibling_checks.len();
    totals.sibling_hits += siblings.len();

    let mut beam_decisions = Vec::with_capacity(siblings.len());
    let mut beam_targets = Vec::new();
    for (sibling_ordinal, sibling) in siblings.iter().enumerate() {
        let target = beam_map[&sibling.beam];
        let same_beam = sibling.beam == source_beam.source;
        // Every production beam at this stage owns the fixed glyph registered
        // during BEAMS; preserve Java's null branch explicitly in evidence.
        let glyph_present = (!same_beam).then_some(true);
        let same_glyph = (!same_beam).then_some(target.beam_glyph == source_beam.beam_glyph);
        let action = if same_beam {
            NativeStemsBeamTargetAction::SelfBeam
        } else if glyph_present == Some(false) {
            NativeStemsBeamTargetAction::MissingGlyph
        } else if same_glyph == Some(true) {
            NativeStemsBeamTargetAction::SameGlyph
        } else {
            let target_arena_index = *arena_indices.get(&sibling.beam).ok_or(
                NativeStemsBeamReachabilityError::MissingBeam {
                    system_id,
                    source: sibling.beam,
                },
            )?;
            let find = find_linker(
                v_linker.reference,
                sibling_ordinal,
                *request_ordinal,
                geometry.theoretical_line,
                target.source,
                target.median,
                max_beam_linker_dx,
                &mut arenas[target_arena_index],
            );
            *request_ordinal += 1;
            totals.find_requests += 1;
            match find.result {
                NativeStemsBeamFindResult::Reused(_) => {
                    totals.reused_b_linkers += 1;
                    totals.reused_anchors += usize::from(find.reused_anchor);
                }
                NativeStemsBeamFindResult::CreatedAnchor(_) => totals.created_anchors += 1,
            }
            beam_targets.push(find.result.reference());
            NativeStemsBeamTargetAction::FindLinker(Box::new(find))
        };
        beam_decisions.push(NativeStemsBeamTargetDecision {
            sibling_ordinal,
            beam: sibling.beam,
            same_beam,
            glyph_present,
            same_glyph,
            action,
        });
    }
    totals.beam_targets += beam_targets.len();

    let (head_search, head_targets) = filter_heads(
        system_id,
        v_linker,
        source_beam,
        &siblings,
        beam_map,
        head_system,
        heads_in_abscissa_order,
        min_beam_head_dy,
        totals,
    )?;
    totals.head_targets += head_targets.len();
    let ordered_targets = ordered_targets(&beam_targets, &head_targets);

    Ok(NativeStemsBeamVInspection {
        inspection_ordinal,
        reference: v_linker.reference,
        y_direction: v_linker.y_direction,
        stopping_head_side: v_linker.stopping_head_side,
        max_stem_profile,
        theoretical_line: geometry.theoretical_line,
        lookup_quadrilateral: geometry.quadrilateral,
        lookup_bounds: geometry.bounds,
        sibling_checks,
        siblings,
        beam_decisions,
        head_search,
        beam_targets,
        head_targets,
        ordered_targets,
    })
}

fn beam_group_sources(
    beams: &NativeBeamRecognition,
    system_id: usize,
    state: &crate::recognize::NativeBeamGroupMembership,
) -> Result<Vec<Vec<NativeStemsBeamSource>>, NativeStemsBeamReachabilityError> {
    if state.system_id != system_id {
        return Err(NativeStemsBeamReachabilityError::MissingGroupSystem(
            system_id,
        ));
    }
    let members = beams
        .raw_beams
        .iter()
        .enumerate()
        .filter_map(|(ordinal, (owner, _))| {
            (*owner == system_id).then_some(NativeStemsBeamSource::RawBeam(ordinal))
        })
        .chain(
            beams
                .hooks
                .iter()
                .enumerate()
                .filter_map(|(ordinal, (owner, _))| {
                    (*owner == system_id).then_some(NativeStemsBeamSource::Hook(ordinal))
                }),
        )
        .collect::<Vec<_>>();
    state
        .groups
        .iter()
        .map(|group| {
            group
                .iter()
                .map(|&member_ordinal| {
                    members.get(member_ordinal).copied().ok_or(
                        NativeStemsBeamReachabilityError::InvalidGroupMember {
                            system_id,
                            member_ordinal,
                        },
                    )
                })
                .collect()
        })
        .collect()
}

fn validate_head_order(
    system_id: usize,
    heads: &crate::native_stems_head_corners::NativeStemsHeadCornerSystem,
) -> Result<(), NativeStemsBeamReachabilityError> {
    if heads.heads_by_abscissa.len() != heads.heads_in_sig_order.len() {
        return Err(NativeStemsBeamReachabilityError::InvalidHeadOrder { system_id });
    }
    let mut seen = BTreeSet::new();
    let mut prior_x = None;
    for &index in &heads.heads_by_abscissa {
        let Some(head) = heads.heads_in_sig_order.get(index) else {
            return Err(NativeStemsBeamReachabilityError::InvalidHeadOrder { system_id });
        };
        if !seen.insert(index) || prior_x.is_some_and(|x| head.bounds.x < x) {
            return Err(NativeStemsBeamReachabilityError::InvalidHeadOrder { system_id });
        }
        prior_x = Some(head.bounds.x);
    }
    Ok(())
}

fn sibling_beams_at(
    system_id: usize,
    point: NativeStemPoint,
    group: &[NativeStemsBeamSource],
    beam_map: &BTreeMap<NativeStemsBeamSource, &NativeStemsBeamStumpBeam>,
    global_slope: f64,
    margin: i32,
) -> Result<
    (
        Vec<NativeStemsBeamReachabilitySiblingCheck>,
        Vec<NativeStemsBeamReachabilitySiblingHit>,
    ),
    NativeStemsBeamReachabilityError,
> {
    let vertical = Segment {
        x1: point.x,
        y1: point.y,
        x2: point.x - (1000.0 * global_slope),
        y2: point.y + 1000.0,
    };
    let mut checks = Vec::with_capacity(group.len());
    let mut hits = Vec::new();
    for (input_ordinal, &source) in group.iter().enumerate() {
        let Some(beam) = beam_map.get(&source).copied() else {
            // `groups` was already filtered to the live set, so a dangling
            // source means constructor/stump topology is inconsistent.
            return Err(NativeStemsBeamReachabilityError::MissingBeam { system_id, source });
        };
        let cross = generic_intersection(vertical, beam.median);
        let left_limit = beam.median.x1 - f64::from(margin);
        let right_limit = beam.median.x2 + f64::from(margin);
        let accepted = left_limit <= cross.x && cross.x <= right_limit;
        checks.push(NativeStemsBeamReachabilitySiblingCheck {
            input_ordinal,
            beam: source,
            cross,
            left_limit,
            right_limit,
            accepted,
        });
        if accepted {
            hits.push(NativeStemsBeamReachabilitySiblingHit {
                input_ordinal,
                sorted_ordinal: 0,
                beam: source,
                cross,
            });
        }
    }
    sort_sibling_hits(&mut hits);
    for (sorted_ordinal, hit) in hits.iter_mut().enumerate() {
        hit.sorted_ordinal = sorted_ordinal;
    }
    Ok((checks, hits))
}

fn sort_sibling_hits(hits: &mut [NativeStemsBeamReachabilitySiblingHit]) {
    // Java `Collections.sort` is stable, so equal ordinates retain beam-group
    // insertion order.
    hits.sort_by(|one, two| one.cross.y.total_cmp(&two.cross.y));
}

#[allow(clippy::too_many_arguments)]
fn find_linker(
    requesting_v: NativeStemsBeamVLinkerRef,
    sibling_ordinal: usize,
    request_ordinal: usize,
    theoretical_line: NativeStemLine,
    target_beam: NativeStemsBeamSource,
    target_median: Segment,
    max_beam_linker_dx: i32,
    arena: &mut NativeStemsBeamArena,
) -> NativeStemsBeamFindLinker {
    let cross = generic_intersection(line_segment(theoretical_line), target_median);
    let before = arena.all_b_linkers.len();
    let mut best = None;
    let mut best_dx = f64::MAX;
    let mut candidates = Vec::with_capacity(before);
    for (insertion_ordinal, entry) in arena.all_b_linkers.iter().enumerate() {
        let dx = (entry.reference_point.x - cross.x).abs();
        let best_dx_before = best_dx;
        // Exact Java tie rule: only a strictly closer linker replaces the
        // current one, so first insertion order wins equal distances.
        let replaces_best = best_dx > dx;
        if replaces_best {
            best = Some(entry.reference);
            best_dx = dx;
        }
        candidates.push(NativeStemsBeamFindCandidate {
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
        NativeStemsBeamFindResult::Reused(best.expect("finite threshold implies a candidate"))
    } else {
        let reference = NativeStemsBeamBLinkerRef {
            beam: target_beam,
            id: before + 1,
        };
        arena.all_b_linkers.push(NativeStemsBeamArenaEntry {
            reference,
            reference_point: cross,
            horizontal_side: None,
            is_anchor: true,
            origin: NativeStemsBeamArenaOrigin::Anchor {
                requesting_v,
                sibling_ordinal,
                request_ordinal,
            },
        });
        NativeStemsBeamFindResult::CreatedAnchor(reference)
    };
    let reused_anchor = match result {
        NativeStemsBeamFindResult::Reused(reference) => arena
            .all_b_linkers
            .get(reference.id - 1)
            .is_some_and(|entry| entry.reference == reference && entry.is_anchor),
        NativeStemsBeamFindResult::CreatedAnchor(_) => false,
    };
    NativeStemsBeamFindLinker {
        request_ordinal,
        requesting_v,
        target_beam,
        theoretical_line,
        cross,
        arena_count_before: before,
        candidates,
        selected_before_threshold,
        best_dx,
        max_beam_linker_dx,
        result,
        reused_anchor,
        arena_count_after: arena.all_b_linkers.len(),
    }
}

#[allow(clippy::too_many_arguments)]
fn filter_heads(
    system_id: usize,
    v_linker: &crate::native_stems_beam_vlinkers::NativeStemsBeamVLinker,
    source_beam: &NativeStemsBeamStumpBeam,
    siblings: &[NativeStemsBeamReachabilitySiblingHit],
    beam_map: &BTreeMap<NativeStemsBeamSource, &NativeStemsBeamStumpBeam>,
    head_system: &crate::native_stems_head_corners::NativeStemsHeadCornerSystem,
    heads_in_abscissa_order: &[NativeStemsBeamHeadCandidateRef],
    min_beam_head_dy: i32,
    totals: &mut ReachabilityTotals,
) -> Result<
    (NativeStemsBeamHeadSearch, Vec<NativeStemsBeamHeadCornerRef>),
    NativeStemsBeamReachabilityError,
> {
    if siblings.is_empty() {
        return Ok((
            NativeStemsBeamHeadSearch {
                skipped_for_empty_siblings: true,
                last_sibling: None,
                last_border_side: None,
                last_border: None,
                y_last_border: None,
                scans: Vec::new(),
                candidates_before_competing_removal: Vec::new(),
                competing_removals: Vec::new(),
                candidates_after_competing_removal: Vec::new(),
                decisions: Vec::new(),
            },
            Vec::new(),
        ));
    }

    let (last_hit, border_side) = if v_linker.y_direction > 0 {
        (
            siblings.last().expect("nonempty siblings"),
            NativeStemVerticalSide::Bottom,
        )
    } else {
        (
            siblings.first().expect("nonempty siblings"),
            NativeStemVerticalSide::Top,
        )
    };
    let last_beam = beam_map[&last_hit.beam];
    let last_border = beam_border(last_beam, border_side);
    let vertical_at_reference = Segment {
        x1: v_linker.final_geometry.theoretical_line.start.x,
        y1: 0.0,
        x2: v_linker.final_geometry.theoretical_line.start.x,
        y2: 1000.0,
    };
    let y_last_border = generic_intersection(last_border, vertical_at_reference).y;

    let mut scans = Vec::new();
    let mut candidates = Vec::new();
    let lookup_max_x = f64::from(v_linker.final_geometry.bounds.x)
        + f64::from(v_linker.final_geometry.bounds.width);
    for &candidate in heads_in_abscissa_order {
        let head = &head_system.heads_in_sig_order[candidate.sig_ordinal];
        let intersects_lookup_area =
            convex_quad_intersects_rectangle(v_linker.final_geometry.quadrilateral, head.bounds);
        let beyond_lookup_max_x = f64::from(head.bounds.x) > lookup_max_x;
        let breaks_after = !intersects_lookup_area && beyond_lookup_max_x;
        scans.push(NativeStemsBeamHeadScan {
            x_ordinal: candidate.x_ordinal,
            head: candidate.head,
            sig_ordinal: candidate.sig_ordinal,
            bounds: head.bounds,
            intersects_lookup_area,
            beyond_lookup_max_x,
            breaks_after,
        });
        if intersects_lookup_area {
            candidates.push(candidate);
        } else if breaks_after {
            break;
        }
    }
    totals.head_scans += scans.len();
    totals.head_candidates += candidates.len();

    // At this boundary the only exclusions incident to live beams were
    // created by BEAMS between a beam and hook which share a glyph.  HEADS
    // rejects beam-overlapping candidates before SIG insertion; its retained
    // overlap exclusions are head-to-head.  The Java removal loop therefore
    // executes once per sibling but cannot remove a live system head.
    let competing_removals = siblings
        .iter()
        .enumerate()
        .map(
            |(sibling_ordinal, sibling)| NativeStemsBeamCompetingRemoval {
                sibling_ordinal,
                sibling: sibling.beam,
                removed_heads: Vec::new(),
            },
        )
        .collect::<Vec<_>>();
    let candidates_after_competing_removal = candidates.clone();

    let imposed_void_head_side = imposed_void_head_side(v_linker.y_direction);
    let target_vertical = opposite_vertical_side(v_linker.reference.side);
    let is_small_beam = source_beam.kind == BeamKind::SmallBeam;
    let mut decisions = Vec::with_capacity(candidates.len());
    let mut targets = Vec::new();
    for candidate in candidates.iter().copied() {
        let head = &head_system.heads_in_sig_order[candidate.sig_ordinal];
        let is_small_head = false;
        totals.small_heads += usize::from(is_small_head);
        let size_accepted = ALLOW_SMALL_HEAD_ON_STANDARD_BEAM || !is_small_head || is_small_beam;
        if !size_accepted {
            totals.size_rejections += 1;
            decisions.push(NativeStemsBeamHeadDecision {
                candidate,
                shape: head.shape,
                bounds: head.bounds,
                is_small_head,
                is_small_beam,
                allow_small_head_on_standard_beam: ALLOW_SMALL_HEAD_ON_STANDARD_BEAM,
                size_accepted,
                center_y: None,
                y_last_border: None,
                directed_dy: None,
                min_beam_head_dy: None,
                distance_accepted: None,
                imposed_void_head_side,
                action: NativeStemsBeamHeadDecisionAction::SizeRejected,
                corner_checks: Vec::new(),
            });
            continue;
        }
        let center_y = head.bounds.y.wrapping_add(head.bounds.height / 2);
        let directed_dy = directed_head_distance(v_linker.y_direction, center_y, y_last_border);
        let distance_accepted = directed_dy >= f64::from(min_beam_head_dy);
        if !distance_accepted {
            totals.distance_rejections += 1;
            decisions.push(NativeStemsBeamHeadDecision {
                candidate,
                shape: head.shape,
                bounds: head.bounds,
                is_small_head,
                is_small_beam,
                allow_small_head_on_standard_beam: ALLOW_SMALL_HEAD_ON_STANDARD_BEAM,
                size_accepted,
                center_y: Some(center_y),
                y_last_border: Some(y_last_border),
                directed_dy: Some(directed_dy),
                min_beam_head_dy: Some(min_beam_head_dy),
                distance_accepted: Some(false),
                imposed_void_head_side,
                action: NativeStemsBeamHeadDecisionAction::DistanceRejected,
                corner_checks: Vec::new(),
            });
            continue;
        }

        let mut corner_checks = Vec::with_capacity(2);
        for horizontal in [NativeStemHeadSide::Left, NativeStemHeadSide::Right] {
            let corner = find_corner(system_id, head, horizontal, target_vertical)?;
            let reference = NativeStemsBeamHeadCornerRef {
                head: head.reference,
                sig_ordinal: candidate.sig_ordinal,
                x_ordinal: candidate.x_ordinal,
                horizontal,
                vertical: target_vertical,
            };
            let lookup_contains =
                lu_area_contains(v_linker.final_geometry.quadrilateral, corner.reference);
            let is_half_head = head.shape == HeadTemplateShape::NoteheadVoid;
            let half_head_side_accepted = void_side_ok(
                lookup_contains,
                is_half_head,
                horizontal,
                imposed_void_head_side,
            );
            let accepted = lookup_contains && half_head_side_accepted == Some(true);
            let action = if !lookup_contains {
                NativeStemsBeamHeadCornerAction::OutsideLookup
            } else if !half_head_side_accepted.unwrap_or(true) {
                NativeStemsBeamHeadCornerAction::VoidSideRejected
            } else {
                NativeStemsBeamHeadCornerAction::Accepted
            };
            if accepted {
                targets.push(reference);
                totals.accepted_head_corners += 1;
            }
            totals.lookup_contained_corners += usize::from(lookup_contains);
            totals.void_side_rejections +=
                usize::from(action == NativeStemsBeamHeadCornerAction::VoidSideRejected);
            totals.head_corner_checks += 1;
            corner_checks.push(NativeStemsBeamHeadCornerCheck {
                inspection_ordinal: corner.inspection_ordinal,
                reference,
                point: corner.reference,
                lookup_contains,
                is_half_head,
                imposed_void_head_side,
                half_head_side_accepted,
                action,
                accepted,
            });
        }
        decisions.push(NativeStemsBeamHeadDecision {
            candidate,
            shape: head.shape,
            bounds: head.bounds,
            is_small_head,
            is_small_beam,
            allow_small_head_on_standard_beam: ALLOW_SMALL_HEAD_ON_STANDARD_BEAM,
            size_accepted,
            center_y: Some(center_y),
            y_last_border: Some(y_last_border),
            directed_dy: Some(directed_dy),
            min_beam_head_dy: Some(min_beam_head_dy),
            distance_accepted: Some(true),
            imposed_void_head_side,
            action: NativeStemsBeamHeadDecisionAction::CornersChecked,
            corner_checks,
        });
    }

    Ok((
        NativeStemsBeamHeadSearch {
            skipped_for_empty_siblings: false,
            last_sibling: Some(last_hit.beam),
            last_border_side: Some(border_side),
            last_border: Some(last_border),
            y_last_border: Some(y_last_border),
            scans,
            candidates_before_competing_removal: candidates,
            competing_removals,
            candidates_after_competing_removal,
            decisions,
        },
        targets,
    ))
}

fn find_corner(
    system_id: usize,
    head: &crate::native_stems_head_corners::NativeStemsHeadCornerHead,
    horizontal: NativeStemHeadSide,
    vertical: NativeStemVerticalSide,
) -> Result<&NativeStemsHeadCorner, NativeStemsBeamReachabilityError> {
    let mut matching = head
        .corners_in_constructor_order
        .iter()
        .filter(|corner| corner.horizontal == horizontal && corner.vertical == vertical);
    let Some(corner) = matching.next() else {
        return Err(NativeStemsBeamReachabilityError::InvalidHeadCorner {
            system_id,
            head: head.reference,
            horizontal,
            vertical,
        });
    };
    if matching.next().is_some() {
        return Err(NativeStemsBeamReachabilityError::InvalidHeadCorner {
            system_id,
            head: head.reference,
            horizontal,
            vertical,
        });
    }
    Ok(corner)
}

fn lu_area_contains(quadrilateral: [NativeStemPoint; 4], point: NativeStemPoint) -> bool {
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
    // OpenJDK Area first checks its cached Rectangle2D bounds (half-open at
    // max X/Y), then sums Curve crossings.  Keep Order1.XforY operation order.
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

fn ordered_targets(
    beams: &[NativeStemsBeamBLinkerRef],
    heads: &[NativeStemsBeamHeadCornerRef],
) -> Vec<NativeStemsBeamReachabilityTarget> {
    beams
        .iter()
        .copied()
        .map(NativeStemsBeamReachabilityTarget::Beam)
        .chain(
            heads
                .iter()
                .copied()
                .map(NativeStemsBeamReachabilityTarget::Head),
        )
        .collect()
}

const fn imposed_void_head_side(y_direction: i32) -> NativeStemHeadSide {
    if y_direction < 0 {
        NativeStemHeadSide::Left
    } else {
        NativeStemHeadSide::Right
    }
}

fn directed_head_distance(y_direction: i32, center_y: i32, y_last_border: f64) -> f64 {
    f64::from(y_direction) * (f64::from(center_y) - y_last_border)
}

fn void_side_ok(
    lookup_contains: bool,
    is_half_head: bool,
    horizontal: NativeStemHeadSide,
    imposed: NativeStemHeadSide,
) -> Option<bool> {
    if lookup_contains {
        Some(!is_half_head || horizontal == imposed)
    } else {
        None
    }
}

const fn opposite_vertical_side(side: NativeStemVerticalSide) -> NativeStemVerticalSide {
    match side {
        NativeStemVerticalSide::Top => NativeStemVerticalSide::Bottom,
        NativeStemVerticalSide::Bottom => NativeStemVerticalSide::Top,
    }
}

const fn line_segment(line: NativeStemLine) -> Segment {
    Segment {
        x1: line.start.x,
        y1: line.start.y,
        x2: line.stop.x,
        y2: line.stop.y,
    }
}

fn to_pixels(interline: i32, ratio: f64) -> i32 {
    (f64::from(interline) * ratio).round_ties_even() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn point(x: f64, y: f64) -> NativeStemPoint {
        NativeStemPoint { x, y }
    }

    const fn b_ref(id: usize) -> NativeStemsBeamBLinkerRef {
        NativeStemsBeamBLinkerRef {
            beam: NativeStemsBeamSource::RawBeam(7),
            id,
        }
    }

    fn arena_entry(id: usize, x: f64) -> NativeStemsBeamArenaEntry {
        NativeStemsBeamArenaEntry {
            reference: b_ref(id),
            reference_point: point(x, 0.0),
            horizontal_side: None,
            is_anchor: false,
            origin: NativeStemsBeamArenaOrigin::Constructor(NativeStemsBeamBLinkerOrigin::Orphan {
                side: NativeStemHeadSide::Left,
            }),
        }
    }

    const fn requesting_v() -> NativeStemsBeamVLinkerRef {
        NativeStemsBeamVLinkerRef {
            b_linker: NativeStemsBeamBLinkerRef {
                beam: NativeStemsBeamSource::RawBeam(2),
                id: 1,
            },
            side: NativeStemVerticalSide::Top,
        }
    }

    #[test]
    fn find_linker_keeps_first_tie_and_reuses_at_inclusive_threshold() {
        let mut arena = NativeStemsBeamArena {
            beam: NativeStemsBeamSource::RawBeam(7),
            sig_ordinal: 3,
            initial_b_count: 2,
            all_b_linkers: vec![arena_entry(1, 4.0), arena_entry(2, 6.0)],
        };
        let result = find_linker(
            requesting_v(),
            0,
            0,
            NativeStemLine {
                start: point(5.0, -10.0),
                stop: point(5.0, 10.0),
            },
            arena.beam,
            Segment {
                x1: 0.0,
                y1: 0.0,
                x2: 10.0,
                y2: 0.0,
            },
            1,
            &mut arena,
        );
        assert_eq!(result.cross, point(5.0, 0.0));
        assert!(result.candidates[0].replaces_best);
        assert!(!result.candidates[1].replaces_best);
        assert_eq!(result.selected_before_threshold, Some(b_ref(1)));
        assert_eq!(result.best_dx, 1.0);
        assert_eq!(result.result, NativeStemsBeamFindResult::Reused(b_ref(1)));
        assert_eq!(arena.all_b_linkers.len(), 2);
    }

    #[test]
    fn newly_appended_anchor_is_visible_to_next_request() {
        let mut arena = NativeStemsBeamArena {
            beam: NativeStemsBeamSource::RawBeam(7),
            sig_ordinal: 3,
            initial_b_count: 1,
            all_b_linkers: vec![arena_entry(1, 4.0)],
        };
        let line = NativeStemLine {
            start: point(20.0, -10.0),
            stop: point(20.0, 10.0),
        };
        let median = Segment {
            x1: 0.0,
            y1: 0.0,
            x2: 30.0,
            y2: 0.0,
        };
        let created = find_linker(
            requesting_v(),
            1,
            4,
            line,
            arena.beam,
            median,
            1,
            &mut arena,
        );
        assert_eq!(
            created.result,
            NativeStemsBeamFindResult::CreatedAnchor(b_ref(2))
        );
        assert!(arena.all_b_linkers[1].is_anchor);

        let reused = find_linker(
            requesting_v(),
            0,
            5,
            line,
            arena.beam,
            median,
            1,
            &mut arena,
        );
        assert_eq!(reused.result, NativeStemsBeamFindResult::Reused(b_ref(2)));
        assert_eq!(reused.candidates.len(), 2);
        assert_eq!(arena.all_b_linkers.len(), 2);
    }

    #[test]
    fn sibling_sort_is_top_down_and_stable_on_equal_y() {
        let mut hits = vec![
            NativeStemsBeamReachabilitySiblingHit {
                input_ordinal: 0,
                sorted_ordinal: 0,
                beam: NativeStemsBeamSource::RawBeam(0),
                cross: point(3.0, 8.0),
            },
            NativeStemsBeamReachabilitySiblingHit {
                input_ordinal: 1,
                sorted_ordinal: 0,
                beam: NativeStemsBeamSource::RawBeam(1),
                cross: point(4.0, 2.0),
            },
            NativeStemsBeamReachabilitySiblingHit {
                input_ordinal: 2,
                sorted_ordinal: 0,
                beam: NativeStemsBeamSource::RawBeam(2),
                cross: point(5.0, 2.0),
            },
        ];
        sort_sibling_hits(&mut hits);
        assert_eq!(
            hits.iter().map(|hit| hit.input_ordinal).collect::<Vec<_>>(),
            [1, 2, 0]
        );
    }

    #[test]
    fn lu_area_contains_uses_openjdk_half_open_crossings() {
        let square = [
            point(0.0, 0.0),
            point(10.0, 0.0),
            point(10.0, 10.0),
            point(0.0, 10.0),
        ];
        assert!(lu_area_contains(square, point(0.0, 5.0)));
        assert!(lu_area_contains(square, point(5.0, 0.0)));
        assert!(!lu_area_contains(square, point(10.0, 5.0)));
        assert!(!lu_area_contains(square, point(5.0, 10.0)));
    }

    #[test]
    fn determinant_intersection_preserves_vertical_cross() {
        let cross = generic_intersection(
            Segment {
                x1: 3.0,
                y1: -20.0,
                x2: 3.0,
                y2: 20.0,
            },
            Segment {
                x1: 0.0,
                y1: 2.0,
                x2: 10.0,
                y2: 7.0,
            },
        );
        assert_eq!(cross.x.to_bits(), 3.0_f64.to_bits());
        assert_eq!(cross.y.to_bits(), 3.5_f64.to_bits());
    }

    #[test]
    fn head_direction_threshold_and_target_partition_are_exact() {
        assert_eq!(imposed_void_head_side(-1), NativeStemHeadSide::Left);
        assert_eq!(imposed_void_head_side(1), NativeStemHeadSide::Right);
        assert_eq!(directed_head_distance(1, 120, 100.0), 20.0);
        assert!(directed_head_distance(1, 120, 100.0) >= 20.0);
        assert_eq!(
            void_side_ok(
                false,
                true,
                NativeStemHeadSide::Left,
                NativeStemHeadSide::Right,
            ),
            None
        );
        assert_eq!(
            void_side_ok(
                true,
                false,
                NativeStemHeadSide::Left,
                NativeStemHeadSide::Right,
            ),
            Some(true)
        );
        assert_eq!(
            void_side_ok(
                true,
                true,
                NativeStemHeadSide::Left,
                NativeStemHeadSide::Right,
            ),
            Some(false)
        );

        let head = NativeStemsBeamHeadCornerRef {
            head: NativeHeadStaffEpilogRef {
                staff_index: 0,
                head_index: 4,
            },
            sig_ordinal: 6,
            x_ordinal: 3,
            horizontal: NativeStemHeadSide::Right,
            vertical: NativeStemVerticalSide::Bottom,
        };
        assert_eq!(
            ordered_targets(&[b_ref(1), b_ref(2)], &[head]),
            [
                NativeStemsBeamReachabilityTarget::Beam(b_ref(1)),
                NativeStemsBeamReachabilityTarget::Beam(b_ref(2)),
                NativeStemsBeamReachabilityTarget::Head(head),
            ]
        );
    }
}
