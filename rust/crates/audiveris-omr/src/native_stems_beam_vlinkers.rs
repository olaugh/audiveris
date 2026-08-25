// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact constructor-time `BeamLinker` B/V topology and lookup geometry.
//!
//! This boundary continues from [`crate::native_stems_beam_stumps`] through
//! Java's `equipStumps`, `equipOrphanSides`, `VLinker.buildGeometry`,
//! `getCloserLimit`, and the final neighboring-seed lookup.  It deliberately
//! stops before head linkers exist and before `inspectVLinkers` can create
//! cross-beam anchors or a `StemBuilder`.

use std::{cmp::Ordering, error::Error, fmt};

use audiveris_image::beam_structure::{Segment, line_util_intersection};
use audiveris_image::system_population::{BoundarySegment, StaffBoundary};

use crate::{
    head_scanner_slices::{JavaRectangle, population_system_area_integer_bounds},
    native_stem_seeds::NativeStemSeedRecognition,
    native_stems_beam_stumps::{
        NativeStemsBeamDirectionEvidence, NativeStemsBeamSibling, NativeStemsBeamSource,
        NativeStemsBeamStumpBeam, NativeStemsBeamStumpRecognition, NativeStemsBeamStumpRef,
        NativeStemsBeamStumpSystem,
    },
    recognize::{GridLinesRecognition, NativeBeamRecognition},
    stems_step::{NativeStemHeadSide, NativeStemLine, NativeStemPoint, NativeStemVerticalSide},
};

const SLOPE_MARGIN: f64 = 0.015;
const HALF_BEAM_LOOKUP_DX_RATIO: f64 = 0.3;
const MAX_BEAM_GROUP_DY_RATIO: f64 = 5.0;
const MAX_BEAM_SEED_DY_RATIO: f64 = 0.25;
const GOOD_BEAM_GRADE: f64 = 0.35;
const STAFF_VERTICAL_AREA_MARGIN_RATIO: f64 = 0.9;

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamVLinkerRecognition {
    pub systems: Vec<NativeStemsBeamVLinkerSystem>,
    pub constructor_count: usize,
    pub surviving_beam_count: usize,
    pub b_linker_count: usize,
    pub v_linker_count: usize,
    pub stump_b_linker_count: usize,
    pub orphan_b_linker_count: usize,
    pub alien_candidate_count: usize,
    pub alien_limiter_count: usize,
    pub neighbor_seed_check_count: usize,
    pub reachable_seed_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamVLinkerSystem {
    pub system_id: usize,
    pub profile: i32,
    pub interline: i32,
    pub main_stem_thickness: i32,
    pub vicinity_margin: i32,
    pub max_beam_side_dx: i32,
    pub max_beam_group_dy: i32,
    pub max_beam_seed_dy_ratio: f64,
    pub half_beam_lookup_dx: f64,
    pub slope_margin: f64,
    pub global_slope: f64,
    pub system_bounds: JavaRectangle,
    pub staff_ids: Vec<usize>,
    pub parts: Vec<NativeStemsBeamPart>,
    pub constructors: Vec<NativeStemsBeamVLinkerConstructor>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeStemsBeamPart {
    pub part_ordinal: usize,
    pub first_staff_id: usize,
    pub last_staff_id: usize,
    pub staff_ids: Vec<usize>,
    pub bounds: JavaRectangle,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct NativeStemsBeamBLinkerRef {
    pub beam: NativeStemsBeamSource,
    /// Java's one-based `allBLinkers` insertion id, local to the beam.
    pub id: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamVLinkerRef {
    pub b_linker: NativeStemsBeamBLinkerRef,
    pub side: NativeStemVerticalSide,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamVLinkerConstructor {
    pub x_ordinal: usize,
    pub source: NativeStemsBeamSource,
    /// The sorted `systemBeams` visible while this constructor runs.  Earlier
    /// tremolo candidates have already been removed; later ones remain.
    pub visible_sources: Vec<NativeStemsBeamSource>,
    pub looks_like_tremolo: bool,
    pub survives_constructor_loop: bool,
    pub stump_equipments: Vec<NativeStemsBeamStumpEquipment>,
    pub orphan_decisions: Vec<NativeStemsBeamOrphanDecision>,
    pub b_linkers: Vec<NativeStemsBeamBLinker>,
    pub side_b_linkers: Vec<NativeStemsBeamSideBLinker>,
    pub stump_v_linkers: Vec<NativeStemsBeamVLinkerRef>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamStumpEquipment {
    pub stump_list_ordinal: usize,
    pub stump: NativeStemsBeamStumpRef,
    pub horizontal_side: Option<NativeStemHeadSide>,
    pub direction_evidence: NativeStemsBeamDirectionEvidence,
    pub b_linker: NativeStemsBeamBLinkerRef,
    pub v_linkers: Vec<NativeStemsBeamVLinkerRef>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeStemsBeamSideBLinker {
    pub side: NativeStemHeadSide,
    pub b_linker: Option<NativeStemsBeamBLinkerRef>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamOrphanDecision {
    pub side: NativeStemHeadSide,
    /// Java does not ask for the endpoint when this side is already equipped.
    pub endpoint: Option<NativeStemPoint>,
    pub sibling_checks: Vec<NativeStemsBeamSiblingCheck>,
    pub siblings: Vec<NativeStemsBeamSiblingHit>,
    pub first_sibling: Option<NativeStemsBeamSource>,
    pub last_sibling: Option<NativeStemsBeamSource>,
    pub beam_glyph_is_first: bool,
    pub beam_glyph_is_last: bool,
    pub outcome: NativeStemsBeamOrphanOutcome,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamOrphanOutcome {
    ExistingSideBLinker,
    NoSiblings,
    InteriorBeam,
    Created(NativeStemsBeamBLinkerRef),
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamBLinker {
    pub reference: NativeStemsBeamBLinkerRef,
    pub origin: NativeStemsBeamBLinkerOrigin,
    pub horizontal_side: Option<NativeStemHeadSide>,
    pub stump: Option<NativeStemsBeamStumpRef>,
    pub reference_point: NativeStemPoint,
    pub v_linkers: Vec<NativeStemsBeamVLinker>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamBLinkerOrigin {
    Stump { list_ordinal: usize },
    Orphan { side: NativeStemHeadSide },
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamVLinker {
    pub reference: NativeStemsBeamVLinkerRef,
    pub y_direction: i32,
    pub stopping_head_side: NativeStemHeadSide,
    pub is_stump_linker: bool,
    pub initial_geometry: NativeStemsBeamLuGeometry,
    pub closer_search: NativeStemsBeamCloserSearch,
    pub final_geometry: NativeStemsBeamLuGeometry,
    pub seed_checks: Vec<NativeStemsBeamNeighborSeedCheck>,
    pub reachable_seed_ordinals: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamLuGeometry {
    pub limit: NativeStemsBeamLuLimit,
    pub slope: f64,
    pub delta_slope: f64,
    pub effective_profile: i32,
    pub gap_pixels: i32,
    pub border: Segment,
    pub left_border_point: NativeStemPoint,
    pub right_border_point: NativeStemPoint,
    pub y_offset: f64,
    pub y_limit: f64,
    pub delta_y: f64,
    pub quadrilateral: [NativeStemPoint; 4],
    pub double_bounds: NativeStemsBeamDoubleBounds,
    pub bounds: JavaRectangle,
    pub theoretical_line: NativeStemLine,
    pub system_limit: Option<NativeStemsBeamSystemLimitEvidence>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsBeamDoubleBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamLuLimit {
    SystemAndParts,
    Alien {
        source: NativeStemsBeamSource,
        facing_side: NativeStemVerticalSide,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamSystemLimitEvidence {
    pub beam_center: (i32, i32),
    pub staff_candidates: Vec<NativeStemsBeamClosestStaffCandidate>,
    pub closest_staff_id: Option<usize>,
    pub closest_to_top: Option<f64>,
    pub closest_to_bottom: Option<f64>,
    pub closest_contains_core: bool,
    pub around_staff_ids: Vec<usize>,
    pub initial_y_limit: f64,
    pub part_folds: Vec<NativeStemsBeamPartLimitFold>,
    pub final_y_limit: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsBeamClosestStaffCandidate {
    pub staff_id: usize,
    pub area_contains: bool,
    /// `Staff.distanceTo` truncates the spline distance toward zero.
    pub truncated_distance: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsBeamPartLimitFold {
    pub staff_id: usize,
    pub part_ordinal: usize,
    pub staff_bounds: JavaRectangle,
    pub part_bounds: JavaRectangle,
    pub before_y_limit: f64,
    pub candidate_y_limit: f64,
    pub after_y_limit: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamCloserSearch {
    pub fat_bounds: JavaRectangle,
    pub neighbor_scan: Vec<NativeStemsBeamNeighborScan>,
    pub aliens: Vec<NativeStemsBeamAlienEvidence>,
    pub sorted_survivors: Vec<NativeStemsBeamSource>,
    pub selected: Option<NativeStemsBeamSource>,
    pub selected_limit: Option<Segment>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsBeamNeighborScan {
    pub source: NativeStemsBeamSource,
    pub bounds: JavaRectangle,
    pub intersects: bool,
    pub breaks_after: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsBeamAlienEvidence {
    pub neighbor_ordinal: usize,
    pub input_ordinal: Option<usize>,
    pub survivor_pre_sort_ordinal: Option<usize>,
    pub survivor_sorted_ordinal: Option<usize>,
    pub source: NativeStemsBeamSource,
    pub grade: f64,
    pub kind_is_hook: bool,
    pub median_intersects_theoretical: bool,
    pub cross: Option<NativeStemPoint>,
    pub absolute_delta_y: Option<f64>,
    pub aligned_endpoint_x: Option<f64>,
    pub absolute_delta_x: Option<f64>,
    pub sort_target: Option<NativeStemPoint>,
    pub sort_key: Option<f64>,
    pub action: NativeStemsBeamAlienAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsBeamAlienAction {
    GroupMember,
    BadGrade,
    Hook,
    NoTheoreticalIntersection,
    AlignedSide,
    Survives,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsBeamNeighborSeedCheck {
    pub free_glyph_ordinal: usize,
    pub bounds: JavaRectangle,
    pub intersects_final_area: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsBeamSiblingCheck {
    pub source: NativeStemsBeamSource,
    pub cross: NativeStemPoint,
    pub within_horizontal_margin: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsBeamSiblingHit {
    pub source: NativeStemsBeamSource,
    pub cross: NativeStemPoint,
}

#[derive(Debug)]
pub enum NativeStemsBeamVLinkerError {
    SystemOrder,
    MissingGridSystem(usize),
    MissingSystemArea(usize),
    MissingStaffArea {
        system_id: usize,
        staff_id: usize,
    },
    MissingStaffLines {
        system_id: usize,
        staff_id: usize,
    },
    InvalidPartRange {
        system_id: usize,
        part_ordinal: usize,
    },
    StaffPartCardinality {
        system_id: usize,
        staff_id: usize,
        count: usize,
    },
    InvalidParameters {
        system_id: usize,
    },
    MissingBeamSource {
        system_id: usize,
        source: NativeStemsBeamSource,
    },
    BeamSourceMismatch {
        system_id: usize,
        source: NativeStemsBeamSource,
    },
    MissingBeamGroup {
        system_id: usize,
        source: NativeStemsBeamSource,
    },
    InvalidGroupMember {
        system_id: usize,
        member_ordinal: usize,
    },
    MissingNeighborSeed {
        system_id: usize,
        free_glyph_ordinal: usize,
    },
    InvalidSeedBounds {
        system_id: usize,
        free_glyph_ordinal: usize,
    },
    InvalidGeometry,
}

impl fmt::Display for NativeStemsBeamVLinkerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid STEMS BeamVLinker constructor boundary: {self:?}"
        )
    }
}

impl Error for NativeStemsBeamVLinkerError {}

/// Materialize every live beam constructor through final neighbor-seed reachability.
pub fn materialize_native_stems_beam_vlinkers(
    grid: &GridLinesRecognition,
    beams: &NativeBeamRecognition,
    stem_seeds: &NativeStemSeedRecognition,
    beam_stumps: &NativeStemsBeamStumpRecognition,
) -> Result<NativeStemsBeamVLinkerRecognition, NativeStemsBeamVLinkerError> {
    materialize(grid, beams, stem_seeds, beam_stumps)
}

fn materialize(
    grid: &GridLinesRecognition,
    beams: &NativeBeamRecognition,
    stem_seeds: &NativeStemSeedRecognition,
    beam_stumps: &NativeStemsBeamStumpRecognition,
) -> Result<NativeStemsBeamVLinkerRecognition, NativeStemsBeamVLinkerError> {
    let grid_ids = grid
        .peak_graph
        .sig
        .systems
        .iter()
        .map(|system| system.system_id)
        .collect::<Vec<_>>();
    let seed_ids = stem_seeds
        .systems
        .iter()
        .map(|system| system.raw.system_id)
        .collect::<Vec<_>>();
    let stump_ids = beam_stumps
        .systems
        .iter()
        .map(|system| system.system_id)
        .collect::<Vec<_>>();
    if grid_ids != seed_ids || grid_ids != stump_ids {
        return Err(NativeStemsBeamVLinkerError::SystemOrder);
    }

    let mut systems = Vec::with_capacity(grid_ids.len());
    let mut totals = [0_usize; 9];
    for ((grid_system, seed_system), stump_system) in grid
        .peak_graph
        .sig
        .systems
        .iter()
        .zip(&stem_seeds.systems)
        .zip(&beam_stumps.systems)
    {
        let system_id = stump_system.system_id;
        if stump_system.interline <= 0
            || stem_seeds.main_stem_thickness <= 0
            || stump_system.vicinity_margin < 0
            || stump_system.max_beam_side_dx < 0
        {
            return Err(NativeStemsBeamVLinkerError::InvalidParameters { system_id });
        }
        let system_area = grid
            .system_areas
            .iter()
            .find(|area| area.system_id == system_id)
            .ok_or(NativeStemsBeamVLinkerError::MissingSystemArea(system_id))?;
        let system_bounds = population_system_area_integer_bounds(system_area);
        if system_bounds != stump_system.system_bounds {
            return Err(NativeStemsBeamVLinkerError::InvalidGeometry);
        }
        let parts = build_parts(
            grid,
            system_id,
            &grid_system.staff_ids,
            &grid_system.bar_tail.parts,
        )?;
        let group_sources = beam_group_sources(beams, system_id)?;
        let context = SystemContext {
            grid,
            beams,
            seed_system,
            stump_system,
            staff_ids: &grid_system.staff_ids,
            parts: &parts,
            group_sources: &group_sources,
            main_stem_thickness: stem_seeds.main_stem_thickness,
            max_beam_group_dy: to_pixels(stump_system.interline, MAX_BEAM_GROUP_DY_RATIO),
            half_beam_lookup_dx: f64::from(stump_system.interline) * HALF_BEAM_LOOKUP_DX_RATIO,
        };

        let mut constructors = Vec::with_capacity(stump_system.beams_by_abscissa.len());
        let mut removed_tremolos = Vec::new();
        for beam in &stump_system.beams_by_abscissa {
            let visible_sources = stump_system
                .beams_by_abscissa
                .iter()
                .filter(|candidate| !removed_tremolos.contains(&candidate.source))
                .map(|candidate| candidate.source)
                .collect::<Vec<_>>();
            let constructor = build_constructor(beam, &visible_sources, &context)?;
            totals[0] += 1;
            totals[1] += usize::from(constructor.survives_constructor_loop);
            totals[2] += constructor.b_linkers.len();
            totals[3] += constructor
                .b_linkers
                .iter()
                .map(|linker| linker.v_linkers.len())
                .sum::<usize>();
            totals[4] += constructor.stump_equipments.len();
            totals[5] += constructor
                .orphan_decisions
                .iter()
                .filter(|decision| {
                    matches!(decision.outcome, NativeStemsBeamOrphanOutcome::Created(_))
                })
                .count();
            for linker in &constructor.b_linkers {
                for v_linker in &linker.v_linkers {
                    totals[6] += v_linker.closer_search.aliens.len();
                    totals[7] += usize::from(v_linker.closer_search.selected.is_some());
                    totals[8] += v_linker.seed_checks.len();
                }
            }
            if beam.looks_like_tremolo {
                removed_tremolos.push(beam.source);
            }
            constructors.push(constructor);
        }
        systems.push(NativeStemsBeamVLinkerSystem {
            system_id,
            profile: stump_system.profile,
            interline: stump_system.interline,
            main_stem_thickness: stem_seeds.main_stem_thickness,
            vicinity_margin: stump_system.vicinity_margin,
            max_beam_side_dx: stump_system.max_beam_side_dx,
            max_beam_group_dy: context.max_beam_group_dy,
            max_beam_seed_dy_ratio: MAX_BEAM_SEED_DY_RATIO,
            half_beam_lookup_dx: context.half_beam_lookup_dx,
            slope_margin: SLOPE_MARGIN,
            global_slope: grid.global_slope,
            system_bounds,
            staff_ids: grid_system.staff_ids.clone(),
            parts,
            constructors,
        });
    }
    let reachable_seed_count = systems
        .iter()
        .flat_map(|system| &system.constructors)
        .flat_map(|constructor| &constructor.b_linkers)
        .flat_map(|linker| &linker.v_linkers)
        .map(|linker| linker.reachable_seed_ordinals.len())
        .sum();

    Ok(NativeStemsBeamVLinkerRecognition {
        systems,
        constructor_count: totals[0],
        surviving_beam_count: totals[1],
        b_linker_count: totals[2],
        v_linker_count: totals[3],
        stump_b_linker_count: totals[4],
        orphan_b_linker_count: totals[5],
        alien_candidate_count: totals[6],
        alien_limiter_count: totals[7],
        neighbor_seed_check_count: totals[8],
        reachable_seed_count,
    })
}

struct SystemContext<'a> {
    grid: &'a GridLinesRecognition,
    beams: &'a NativeBeamRecognition,
    seed_system: &'a crate::native_stem_seeds::NativeStemSeedSystemRecognition,
    stump_system: &'a NativeStemsBeamStumpSystem,
    staff_ids: &'a [usize],
    parts: &'a [NativeStemsBeamPart],
    group_sources: &'a [Vec<NativeStemsBeamSource>],
    main_stem_thickness: i32,
    max_beam_group_dy: i32,
    half_beam_lookup_dx: f64,
}

fn build_parts(
    grid: &GridLinesRecognition,
    system_id: usize,
    staff_ids: &[usize],
    plans: &[audiveris_image::bars_logic::PlannedPart],
) -> Result<Vec<NativeStemsBeamPart>, NativeStemsBeamVLinkerError> {
    let mut parts = Vec::with_capacity(plans.len());
    for (part_ordinal, plan) in plans.iter().enumerate() {
        let (Ok(first_staff_id), Ok(last_staff_id)) = (
            usize::try_from(plan.first_staff_id),
            usize::try_from(plan.last_staff_id),
        ) else {
            return Err(NativeStemsBeamVLinkerError::InvalidPartRange {
                system_id,
                part_ordinal,
            });
        };
        let Some(first) = staff_ids.iter().position(|&staff| staff == first_staff_id) else {
            return Err(NativeStemsBeamVLinkerError::InvalidPartRange {
                system_id,
                part_ordinal,
            });
        };
        let Some(last) = staff_ids.iter().position(|&staff| staff == last_staff_id) else {
            return Err(NativeStemsBeamVLinkerError::InvalidPartRange {
                system_id,
                part_ordinal,
            });
        };
        if first > last {
            return Err(NativeStemsBeamVLinkerError::InvalidPartRange {
                system_id,
                part_ordinal,
            });
        }
        let members = staff_ids[first..=last].to_vec();
        let mut bounds = None;
        for &staff_id in &members {
            let staff_bounds = java_staff_area_integer_bounds(grid, system_id, staff_id)?;
            bounds =
                Some(bounds.map_or(staff_bounds, |prior| rectangle_union(prior, staff_bounds)));
        }
        parts.push(NativeStemsBeamPart {
            part_ordinal,
            first_staff_id,
            last_staff_id,
            staff_ids: members,
            bounds: bounds.ok_or(NativeStemsBeamVLinkerError::InvalidPartRange {
                system_id,
                part_ordinal,
            })?,
        });
    }
    for &staff_id in staff_ids {
        let count = parts
            .iter()
            .filter(|part| part.staff_ids.contains(&staff_id))
            .count();
        if count != 1 {
            return Err(NativeStemsBeamVLinkerError::StaffPartCardinality {
                system_id,
                staff_id,
                count,
            });
        }
    }
    Ok(parts)
}

fn beam_group_sources(
    beams: &NativeBeamRecognition,
    system_id: usize,
) -> Result<Vec<Vec<NativeStemsBeamSource>>, NativeStemsBeamVLinkerError> {
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
    let state = beams
        .group_memberships
        .iter()
        .find(|state| state.system_id == system_id)
        .ok_or(NativeStemsBeamVLinkerError::MissingGridSystem(system_id))?;
    state
        .groups
        .iter()
        .map(|group| {
            group
                .iter()
                .map(|&ordinal| {
                    members.get(ordinal).copied().ok_or(
                        NativeStemsBeamVLinkerError::InvalidGroupMember {
                            system_id,
                            member_ordinal: ordinal,
                        },
                    )
                })
                .collect()
        })
        .collect()
}

fn build_constructor(
    beam: &NativeStemsBeamStumpBeam,
    visible_sources: &[NativeStemsBeamSource],
    context: &SystemContext<'_>,
) -> Result<NativeStemsBeamVLinkerConstructor, NativeStemsBeamVLinkerError> {
    resolve_source(context.beams, context.stump_system.system_id, beam.source)?;
    let group = context.group_sources.get(beam.group_ordinal).ok_or(
        NativeStemsBeamVLinkerError::MissingBeamGroup {
            system_id: context.stump_system.system_id,
            source: beam.source,
        },
    )?;
    if !group.contains(&beam.source) {
        return Err(NativeStemsBeamVLinkerError::MissingBeamGroup {
            system_id: context.stump_system.system_id,
            source: beam.source,
        });
    }

    let mut b_linkers = Vec::new();
    let mut side_map = [None, None];
    let mut stump_v_linkers = Vec::new();
    let mut stump_equipments = Vec::with_capacity(beam.stumps.len());
    for stump in &beam.stumps {
        let horizontal_side = beam
            .sides
            .iter()
            .filter_map(|side| {
                (side.final_stump.as_ref() == Some(&stump.reference)).then_some(side.side)
            })
            .next_back();
        if horizontal_side != stump.side {
            return Err(NativeStemsBeamVLinkerError::InvalidGeometry);
        }
        let actual_directions = stump_direction_evidence(
            beam,
            stump.directions.stump_center,
            stump.directions.stump_center_line,
            group,
            visible_sources,
            context,
        )?;
        // Java constructs beam linkers in abscissa order and removes a beam
        // classified as tremolo before constructing the next linker. The
        // pre-built stump snapshot therefore may still list a sibling that is
        // no longer live. The constructor-time result above is authoritative;
        // requiring the complete diagnostic snapshots to match would reject
        // this valid Java lifecycle transition.
        let reference_point = generic_intersection(stump.directions.stump_center_line, beam.median);
        let b_reference = NativeStemsBeamBLinkerRef {
            beam: beam.source,
            id: b_linkers.len() + 1,
        };
        if let Some(side) = horizontal_side {
            side_map[head_side_index(side)] = Some(b_reference);
        }
        let mut v_linkers = Vec::new();
        let mut v_refs = Vec::new();
        if let Some(directions) = &actual_directions.directions {
            for &side in directions {
                let v_linker = build_v_linker(
                    b_reference,
                    side,
                    horizontal_side,
                    reference_point,
                    beam,
                    group,
                    visible_sources,
                    context,
                    true,
                )?;
                v_refs.push(v_linker.reference);
                stump_v_linkers.push(v_linker.reference);
                v_linkers.push(v_linker);
            }
        }
        b_linkers.push(NativeStemsBeamBLinker {
            reference: b_reference,
            origin: NativeStemsBeamBLinkerOrigin::Stump {
                list_ordinal: stump.list_ordinal,
            },
            horizontal_side,
            stump: Some(stump.reference.clone()),
            reference_point,
            v_linkers,
        });
        stump_equipments.push(NativeStemsBeamStumpEquipment {
            stump_list_ordinal: stump.list_ordinal,
            stump: stump.reference.clone(),
            horizontal_side,
            direction_evidence: actual_directions,
            b_linker: b_reference,
            v_linkers: v_refs,
        });
    }

    let mut orphan_decisions = Vec::with_capacity(2);
    for side in [NativeStemHeadSide::Left, NativeStemHeadSide::Right] {
        let endpoint = if side == NativeStemHeadSide::Left {
            NativeStemPoint {
                x: beam.median.x1,
                y: beam.median.y1,
            }
        } else {
            NativeStemPoint {
                x: beam.median.x2,
                y: beam.median.y2,
            }
        };
        if side_map[head_side_index(side)].is_some() {
            orphan_decisions.push(NativeStemsBeamOrphanDecision {
                side,
                endpoint: None,
                sibling_checks: Vec::new(),
                siblings: Vec::new(),
                first_sibling: None,
                last_sibling: None,
                beam_glyph_is_first: false,
                beam_glyph_is_last: false,
                outcome: NativeStemsBeamOrphanOutcome::ExistingSideBLinker,
            });
            continue;
        }
        let (sibling_checks, siblings) = sibling_beams_at(
            endpoint,
            group,
            visible_sources,
            context.stump_system.max_beam_side_dx,
            context,
        )?;
        let first_sibling = siblings.first().map(|sibling| sibling.source);
        let last_sibling = siblings.last().map(|sibling| sibling.source);
        let beam_glyph_is_first = first_sibling
            .and_then(|source| find_beam(context.stump_system, source))
            .is_some_and(|sibling| sibling.beam_glyph == beam.beam_glyph);
        let beam_glyph_is_last = last_sibling
            .and_then(|source| find_beam(context.stump_system, source))
            .is_some_and(|sibling| sibling.beam_glyph == beam.beam_glyph);
        let outcome = if siblings.is_empty() {
            NativeStemsBeamOrphanOutcome::NoSiblings
        } else if !beam_glyph_is_first && !beam_glyph_is_last {
            NativeStemsBeamOrphanOutcome::InteriorBeam
        } else {
            let direction = head_side_direction(side);
            let side_x = if direction < 0 {
                beam.median.x1
            } else {
                beam.median.x2
            };
            let reference_x = side_x - (f64::from(direction * context.main_stem_thickness) / 2.0);
            let reference_point = intersection_at_x(beam.median, reference_x);
            let b_reference = NativeStemsBeamBLinkerRef {
                beam: beam.source,
                id: b_linkers.len() + 1,
            };
            side_map[head_side_index(side)] = Some(b_reference);
            let mut v_linkers = Vec::with_capacity(2);
            for v_side in [NativeStemVerticalSide::Top, NativeStemVerticalSide::Bottom] {
                v_linkers.push(build_v_linker(
                    b_reference,
                    v_side,
                    Some(side),
                    reference_point,
                    beam,
                    group,
                    visible_sources,
                    context,
                    false,
                )?);
            }
            b_linkers.push(NativeStemsBeamBLinker {
                reference: b_reference,
                origin: NativeStemsBeamBLinkerOrigin::Orphan { side },
                horizontal_side: Some(side),
                stump: None,
                reference_point,
                v_linkers,
            });
            NativeStemsBeamOrphanOutcome::Created(b_reference)
        };
        orphan_decisions.push(NativeStemsBeamOrphanDecision {
            side,
            endpoint: Some(endpoint),
            sibling_checks,
            siblings,
            first_sibling,
            last_sibling,
            beam_glyph_is_first,
            beam_glyph_is_last,
            outcome,
        });
    }

    Ok(NativeStemsBeamVLinkerConstructor {
        x_ordinal: beam.x_ordinal,
        source: beam.source,
        visible_sources: visible_sources.to_vec(),
        looks_like_tremolo: beam.looks_like_tremolo,
        survives_constructor_loop: !beam.looks_like_tremolo,
        stump_equipments,
        orphan_decisions,
        b_linkers,
        side_b_linkers: [NativeStemHeadSide::Left, NativeStemHeadSide::Right]
            .into_iter()
            .map(|side| NativeStemsBeamSideBLinker {
                side,
                b_linker: side_map[head_side_index(side)],
            })
            .collect(),
        stump_v_linkers,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_v_linker(
    b_linker: NativeStemsBeamBLinkerRef,
    side: NativeStemVerticalSide,
    horizontal_side: Option<NativeStemHeadSide>,
    reference_point: NativeStemPoint,
    beam: &NativeStemsBeamStumpBeam,
    group: &[NativeStemsBeamSource],
    visible_sources: &[NativeStemsBeamSource],
    context: &SystemContext<'_>,
    is_stump_linker: bool,
) -> Result<NativeStemsBeamVLinker, NativeStemsBeamVLinkerError> {
    let y_direction = vertical_side_direction(side);
    let reference = NativeStemsBeamVLinkerRef { b_linker, side };
    let initial_geometry = build_lu_geometry(beam, side, reference_point, None, context)?;
    let closer_search = closer_search(
        beam,
        side,
        horizontal_side,
        reference_point,
        initial_geometry.theoretical_line,
        group,
        visible_sources,
        context,
    )?;
    let final_geometry = if let (Some(source), Some(limit)) =
        (closer_search.selected, closer_search.selected_limit)
    {
        build_lu_geometry(beam, side, reference_point, Some((source, limit)), context)?
    } else {
        initial_geometry.clone()
    };

    let mut seed_checks = Vec::with_capacity(beam.neighbor_seed_ordinals.len());
    let mut reachable_seed_ordinals = Vec::new();
    for &free_glyph_ordinal in &beam.neighbor_seed_ordinals {
        let glyph = context
            .seed_system
            .free_glyphs
            .get(free_glyph_ordinal)
            .ok_or(NativeStemsBeamVLinkerError::MissingNeighborSeed {
                system_id: context.stump_system.system_id,
                free_glyph_ordinal,
            })?;
        let bounds = bounds_to_rectangle(glyph.bounds).ok_or(
            NativeStemsBeamVLinkerError::InvalidSeedBounds {
                system_id: context.stump_system.system_id,
                free_glyph_ordinal,
            },
        )?;
        let intersects_final_area =
            convex_quad_intersects_rectangle(final_geometry.quadrilateral, bounds);
        seed_checks.push(NativeStemsBeamNeighborSeedCheck {
            free_glyph_ordinal,
            bounds,
            intersects_final_area,
        });
        if intersects_final_area {
            reachable_seed_ordinals.push(free_glyph_ordinal);
        }
    }

    Ok(NativeStemsBeamVLinker {
        reference,
        y_direction,
        stopping_head_side: if y_direction < 0 {
            NativeStemHeadSide::Left
        } else {
            NativeStemHeadSide::Right
        },
        is_stump_linker,
        initial_geometry,
        closer_search,
        final_geometry,
        seed_checks,
        reachable_seed_ordinals,
    })
}

fn build_lu_geometry(
    beam: &NativeStemsBeamStumpBeam,
    side: NativeStemVerticalSide,
    reference_point: NativeStemPoint,
    alien_limit: Option<(NativeStemsBeamSource, Segment)>,
    context: &SystemContext<'_>,
) -> Result<NativeStemsBeamLuGeometry, NativeStemsBeamVLinkerError> {
    let y_direction = vertical_side_direction(side);
    let slope = -context.grid.global_slope;
    let delta_slope = f64::from(y_direction) * SLOPE_MARGIN;
    let border = beam_border(beam, side);
    let left_border_point =
        intersection_at_x(border, reference_point.x - context.half_beam_lookup_dx);
    let right_border_point =
        intersection_at_x(border, reference_point.x + context.half_beam_lookup_dx);
    let y_offset = f64::from(y_direction) * MAX_BEAM_SEED_DY_RATIO * f64::from(beam.seed_y_gap);
    let (limit, y_limit, system_limit) = if let Some((source, limit_line)) = alien_limit {
        (
            NativeStemsBeamLuLimit::Alien {
                source,
                facing_side: opposite_vertical_side(side),
            },
            intersection_at_x(limit_line, reference_point.x).y,
            None,
        )
    } else {
        let system_limit = system_limit(beam, side, context)?;
        (
            NativeStemsBeamLuLimit::SystemAndParts,
            system_limit.final_y_limit,
            Some(system_limit),
        )
    };
    let delta_y = y_limit - reference_point.y;
    let quadrilateral = [
        NativeStemPoint {
            x: left_border_point.x,
            y: left_border_point.y + y_offset,
        },
        NativeStemPoint {
            x: right_border_point.x,
            y: right_border_point.y + y_offset,
        },
        NativeStemPoint {
            x: right_border_point.x + ((slope + delta_slope) * delta_y),
            y: y_limit,
        },
        NativeStemPoint {
            x: left_border_point.x + ((slope - delta_slope) * delta_y),
            y: y_limit,
        },
    ];
    let double_bounds = quadrilateral_double_bounds(quadrilateral)?;
    let bounds = double_bounds_to_integer(double_bounds);
    let theoretical_line = theoretical_line(reference_point, y_limit, context.grid.global_slope);
    Ok(NativeStemsBeamLuGeometry {
        limit,
        slope,
        delta_slope,
        effective_profile: beam.effective_profile,
        gap_pixels: beam.seed_y_gap,
        border,
        left_border_point,
        right_border_point,
        y_offset,
        y_limit,
        delta_y,
        quadrilateral,
        double_bounds,
        bounds,
        theoretical_line,
        system_limit,
    })
}

fn system_limit(
    beam: &NativeStemsBeamStumpBeam,
    side: NativeStemVerticalSide,
    context: &SystemContext<'_>,
) -> Result<NativeStemsBeamSystemLimitEvidence, NativeStemsBeamVLinkerError> {
    let system_id = context.stump_system.system_id;
    let center = (
        beam.bounds.x.wrapping_add(beam.bounds.width / 2),
        beam.bounds.y.wrapping_add(beam.bounds.height / 2),
    );
    let center_x = f64::from(center.0);
    let center_y = f64::from(center.1);
    let mut staff_candidates = Vec::with_capacity(context.staff_ids.len());
    let mut closest_staff_id = None;
    let mut closest_distance = f64::MAX;
    for &staff_id in context.staff_ids {
        let area = context
            .grid
            .staff_areas
            .iter()
            .find(|area| area.staff_id == staff_id)
            .ok_or(NativeStemsBeamVLinkerError::MissingStaffArea {
                system_id,
                staff_id,
            })?;
        let lines = context
            .grid
            .staff_lines
            .iter()
            .find(|lines| lines.staff_id == staff_id)
            .ok_or(NativeStemsBeamVLinkerError::MissingStaffLines {
                system_id,
                staff_id,
            })?;
        let area_contains = area.area.contains(center_x, center_y);
        let truncated_distance = area_contains.then(|| {
            (lines.first_line.y_at_x_ext(center_x) - center_y)
                .max(center_y - lines.last_line.y_at_x_ext(center_x))
                .trunc()
        });
        if let Some(distance) = truncated_distance
            && distance < closest_distance
        {
            closest_distance = distance;
            closest_staff_id = Some(staff_id);
        }
        staff_candidates.push(NativeStemsBeamClosestStaffCandidate {
            staff_id,
            area_contains,
            truncated_distance,
        });
    }

    let mut around_staff_ids = Vec::new();
    let mut closest_to_top = None;
    let mut closest_to_bottom = None;
    let mut closest_contains_core = false;
    if let Some(staff_id) = closest_staff_id {
        let staff_index = context
            .staff_ids
            .iter()
            .position(|&candidate| candidate == staff_id)
            .ok_or(NativeStemsBeamVLinkerError::MissingStaffLines {
                system_id,
                staff_id,
            })?;
        let lines = context
            .grid
            .staff_lines
            .iter()
            .find(|lines| lines.staff_id == staff_id)
            .ok_or(NativeStemsBeamVLinkerError::MissingStaffLines {
                system_id,
                staff_id,
            })?;
        let to_top = lines.first_line.y_at_x_ext(center_x) - center_y;
        let to_bottom = lines.last_line.y_at_x_ext(center_x) - center_y;
        closest_to_top = Some(to_top);
        closest_to_bottom = Some(to_bottom);
        closest_contains_core = (to_top * to_bottom) <= 0.0;
        let mut first = staff_index;
        let mut last = staff_index;
        if closest_contains_core {
            // One staff only.
        } else if to_top > 0.0 {
            first = first.saturating_sub(1);
        } else if last + 1 < context.staff_ids.len() {
            last += 1;
        }
        around_staff_ids.extend_from_slice(&context.staff_ids[first..=last]);
    }

    let initial_y_limit = if side == NativeStemVerticalSide::Top {
        f64::from(
            context
                .stump_system
                .system_bounds
                .y
                .wrapping_add(context.stump_system.system_bounds.height),
        )
    } else {
        f64::from(context.stump_system.system_bounds.y)
    };
    let mut y_limit = initial_y_limit;
    let mut part_folds = Vec::with_capacity(around_staff_ids.len());
    for &staff_id in &around_staff_ids {
        let matching_parts = context
            .parts
            .iter()
            .filter(|part| part.staff_ids.contains(&staff_id))
            .collect::<Vec<_>>();
        if matching_parts.len() != 1 {
            return Err(NativeStemsBeamVLinkerError::StaffPartCardinality {
                system_id,
                staff_id,
                count: matching_parts.len(),
            });
        }
        let part = matching_parts[0];
        let staff_bounds = java_staff_area_integer_bounds(context.grid, system_id, staff_id)?;
        let candidate_y_limit = if side == NativeStemVerticalSide::Top {
            f64::from(part.bounds.y)
        } else {
            f64::from(
                part.bounds
                    .y
                    .wrapping_add(part.bounds.height)
                    .wrapping_sub(1),
            )
        };
        let before_y_limit = y_limit;
        y_limit = if side == NativeStemVerticalSide::Top {
            y_limit.min(candidate_y_limit)
        } else {
            y_limit.max(candidate_y_limit)
        };
        part_folds.push(NativeStemsBeamPartLimitFold {
            staff_id,
            part_ordinal: part.part_ordinal,
            staff_bounds,
            part_bounds: part.bounds,
            before_y_limit,
            candidate_y_limit,
            after_y_limit: y_limit,
        });
    }
    Ok(NativeStemsBeamSystemLimitEvidence {
        beam_center: center,
        staff_candidates,
        closest_staff_id,
        closest_to_top,
        closest_to_bottom,
        closest_contains_core,
        around_staff_ids,
        initial_y_limit,
        part_folds,
        final_y_limit: y_limit,
    })
}

#[allow(clippy::too_many_arguments)]
fn closer_search(
    beam: &NativeStemsBeamStumpBeam,
    side: NativeStemVerticalSide,
    horizontal_side: Option<NativeStemHeadSide>,
    reference_point: NativeStemPoint,
    theoretical_line: NativeStemLine,
    group: &[NativeStemsBeamSource],
    visible_sources: &[NativeStemsBeamSource],
    context: &SystemContext<'_>,
) -> Result<NativeStemsBeamCloserSearch, NativeStemsBeamVLinkerError> {
    let fat_bounds = JavaRectangle {
        x: beam
            .bounds
            .x
            .wrapping_sub(context.stump_system.vicinity_margin),
        y: context.stump_system.system_bounds.y,
        width: beam
            .bounds
            .width
            .wrapping_add(context.stump_system.vicinity_margin.wrapping_mul(2)),
        height: context.stump_system.system_bounds.height,
    };
    let x_max = fat_bounds.x.wrapping_add(fat_bounds.width).wrapping_sub(1);
    let mut neighbor_scan = Vec::new();
    let mut neighbors = Vec::new();
    for &source in visible_sources {
        let candidate = find_beam(context.stump_system, source).ok_or(
            NativeStemsBeamVLinkerError::MissingBeamSource {
                system_id: context.stump_system.system_id,
                source,
            },
        )?;
        let intersects = fat_bounds.intersects(candidate.bounds);
        let breaks_after = !intersects && candidate.bounds.x > x_max;
        neighbor_scan.push(NativeStemsBeamNeighborScan {
            source,
            bounds: candidate.bounds,
            intersects,
            breaks_after,
        });
        if intersects {
            neighbors.push(source);
        }
        if breaks_after {
            break;
        }
    }

    let y_direction = vertical_side_direction(side);
    let mut input_ordinal = 0_usize;
    let mut survivor_ordinal = 0_usize;
    let mut aliens = Vec::with_capacity(neighbors.len());
    for (neighbor_ordinal, source) in neighbors.into_iter().enumerate() {
        let candidate = find_beam(context.stump_system, source).ok_or(
            NativeStemsBeamVLinkerError::MissingBeamSource {
                system_id: context.stump_system.system_id,
                source,
            },
        )?;
        let raw = resolve_source(context.beams, context.stump_system.system_id, source)?;
        let in_group = group.contains(&source);
        let this_input_ordinal = (!in_group).then_some(input_ordinal);
        if !in_group {
            input_ordinal += 1;
        }
        // The source enum records backing collection provenance, not Java
        // runtime class. Built hooks can legitimately live in the raw-beam
        // collection, so `BeamHookInter` identity comes from the materialized
        // beam kind.
        let kind_is_hook = beam_kind_is_hook(candidate.kind);
        let mut median_intersects_theoretical = false;
        let mut cross = None;
        let mut absolute_delta_y = None;
        let mut aligned_endpoint_x = None;
        let mut absolute_delta_x = None;
        let mut sort_target = None;
        let mut sort_key = None;
        let mut survivor_pre_sort_ordinal = None;
        let action = if in_group {
            NativeStemsBeamAlienAction::GroupMember
        } else if !is_good_beam_grade(raw.grade) {
            NativeStemsBeamAlienAction::BadGrade
        } else if kind_is_hook {
            NativeStemsBeamAlienAction::Hook
        } else {
            median_intersects_theoretical = segment_intersects(
                candidate.median,
                Segment {
                    x1: theoretical_line.start.x,
                    y1: theoretical_line.start.y,
                    x2: theoretical_line.stop.x,
                    y2: theoretical_line.stop.y,
                },
            );
            if !median_intersects_theoretical {
                NativeStemsBeamAlienAction::NoTheoreticalIntersection
            } else {
                let crossing = generic_intersection(
                    Segment {
                        x1: theoretical_line.start.x,
                        y1: theoretical_line.start.y,
                        x2: theoretical_line.stop.x,
                        y2: theoretical_line.stop.y,
                    },
                    candidate.median,
                );
                let dy = (crossing.y - reference_point.y).abs();
                cross = Some(crossing);
                absolute_delta_y = Some(dy);
                let endpoint_x = if horizontal_side == Some(NativeStemHeadSide::Left) {
                    candidate.median.x1
                } else {
                    candidate.median.x2
                };
                let dx = (crossing.x - endpoint_x).abs();
                aligned_endpoint_x = Some(endpoint_x);
                absolute_delta_x = Some(dx);
                if is_aligned_alien(
                    dy,
                    dx,
                    context.max_beam_group_dy,
                    context.stump_system.max_beam_side_dx,
                ) {
                    NativeStemsBeamAlienAction::AlignedSide
                } else {
                    let target = get_target_point(
                        reference_point,
                        beam_border(candidate, side),
                        context.grid.global_slope,
                    );
                    let key = f64::from(y_direction) * (target.y - reference_point.y);
                    sort_target = Some(target);
                    sort_key = Some(key);
                    survivor_pre_sort_ordinal = Some(survivor_ordinal);
                    survivor_ordinal += 1;
                    NativeStemsBeamAlienAction::Survives
                }
            }
        };
        aliens.push(NativeStemsBeamAlienEvidence {
            neighbor_ordinal,
            input_ordinal: this_input_ordinal,
            survivor_pre_sort_ordinal,
            survivor_sorted_ordinal: None,
            source,
            grade: raw.grade,
            kind_is_hook,
            median_intersects_theoretical,
            cross,
            absolute_delta_y,
            aligned_endpoint_x,
            absolute_delta_x,
            sort_target,
            sort_key,
            action,
        });
    }
    let mut survivor_indices = aliens
        .iter()
        .enumerate()
        .filter_map(|(index, alien)| {
            (alien.action == NativeStemsBeamAlienAction::Survives).then_some(index)
        })
        .collect::<Vec<_>>();
    survivor_indices.sort_by(|&left, &right| {
        java_double_order(
            aliens[left].sort_key.expect("survivor has key"),
            aliens[right].sort_key.expect("survivor has key"),
        )
    });
    for (sorted_ordinal, &index) in survivor_indices.iter().enumerate() {
        aliens[index].survivor_sorted_ordinal = Some(sorted_ordinal);
    }
    let sorted_survivors = survivor_indices
        .iter()
        .map(|&index| aliens[index].source)
        .collect::<Vec<_>>();
    let selected = sorted_survivors.first().copied();
    let selected_limit = selected
        .and_then(|source| find_beam(context.stump_system, source))
        .map(|alien| beam_border(alien, opposite_vertical_side(side)));
    Ok(NativeStemsBeamCloserSearch {
        fat_bounds,
        neighbor_scan,
        aliens,
        sorted_survivors,
        selected,
        selected_limit,
    })
}

fn stump_direction_evidence(
    beam: &NativeStemsBeamStumpBeam,
    stump_center: (f64, f64),
    stump_center_line: Segment,
    group: &[NativeStemsBeamSource],
    visible_sources: &[NativeStemsBeamSource],
    context: &SystemContext<'_>,
) -> Result<NativeStemsBeamDirectionEvidence, NativeStemsBeamVLinkerError> {
    let (_, sibling_hits) = sibling_beams_at(
        NativeStemPoint {
            x: stump_center.0,
            y: stump_center.1,
        },
        group,
        visible_sources,
        context.stump_system.max_beam_side_dx,
        context,
    )?;
    let siblings = sibling_hits
        .iter()
        .map(|sibling| NativeStemsBeamSibling {
            source: sibling.source,
            cross: (sibling.cross.x, sibling.cross.y),
        })
        .collect::<Vec<_>>();
    let top_extreme = siblings.first().map(|sibling| sibling.source);
    let bottom_extreme = siblings.last().map(|sibling| sibling.source);
    let beam_is_extreme = top_extreme == Some(beam.source) || bottom_extreme == Some(beam.source);
    let beam_glyph_is_top_extreme = top_extreme
        .and_then(|source| find_beam(context.stump_system, source))
        .is_some_and(|candidate| candidate.beam_glyph == beam.beam_glyph);
    let beam_glyph_is_bottom_extreme = bottom_extreme
        .and_then(|source| find_beam(context.stump_system, source))
        .is_some_and(|candidate| candidate.beam_glyph == beam.beam_glyph);
    let beam_glyph_is_extreme = beam_glyph_is_top_extreme || beam_glyph_is_bottom_extreme;
    let mut evidence = NativeStemsBeamDirectionEvidence {
        stump_center,
        stump_center_line,
        siblings,
        top_extreme,
        bottom_extreme,
        beam_is_extreme,
        beam_glyph_is_top_extreme,
        beam_glyph_is_bottom_extreme,
        beam_glyph_is_extreme,
        top_border_y: None,
        bottom_border_y: None,
        top_dy: None,
        bottom_dy: None,
        directions: None,
    };
    let (Some(top_source), Some(bottom_source)) = (top_extreme, bottom_extreme) else {
        return Ok(evidence);
    };
    if !beam_is_extreme && !beam_glyph_is_extreme {
        return Ok(evidence);
    }
    let top_beam = find_beam(context.stump_system, top_source).ok_or(
        NativeStemsBeamVLinkerError::MissingBeamSource {
            system_id: context.stump_system.system_id,
            source: top_source,
        },
    )?;
    let bottom_beam = find_beam(context.stump_system, bottom_source).ok_or(
        NativeStemsBeamVLinkerError::MissingBeamSource {
            system_id: context.stump_system.system_id,
            source: bottom_source,
        },
    )?;
    let x = stump_center.0;
    let top_border_y = beam_border(top_beam, NativeStemVerticalSide::Top).y_at_x(x);
    let bottom_border_y = beam_border(bottom_beam, NativeStemVerticalSide::Bottom).y_at_x(x);
    let top_dy = 0.0_f64.max(top_border_y - stump_center_line.y1);
    let bottom_dy = 0.0_f64.max(stump_center_line.y2 - bottom_border_y);
    let mut directions = Vec::new();
    if top_dy >= f64::from(context.stump_system.min_beam_stump_dy) {
        directions.push(NativeStemVerticalSide::Top);
    }
    if bottom_dy >= f64::from(context.stump_system.min_beam_stump_dy) {
        directions.push(NativeStemVerticalSide::Bottom);
    }
    evidence.top_border_y = Some(top_border_y);
    evidence.bottom_border_y = Some(bottom_border_y);
    evidence.top_dy = Some(top_dy);
    evidence.bottom_dy = Some(bottom_dy);
    evidence.directions = Some(directions);
    Ok(evidence)
}

fn sibling_beams_at(
    point: NativeStemPoint,
    group: &[NativeStemsBeamSource],
    visible_sources: &[NativeStemsBeamSource],
    margin: i32,
    context: &SystemContext<'_>,
) -> Result<
    (
        Vec<NativeStemsBeamSiblingCheck>,
        Vec<NativeStemsBeamSiblingHit>,
    ),
    NativeStemsBeamVLinkerError,
> {
    let vertical = Segment {
        x1: point.x,
        y1: point.y,
        x2: point.x - (1_000.0 * context.grid.global_slope),
        y2: point.y + 1_000.0,
    };
    let mut checks = Vec::new();
    let mut hits = Vec::new();
    for &source in group {
        if !visible_sources.contains(&source) {
            continue;
        }
        let beam = find_beam(context.stump_system, source).ok_or(
            NativeStemsBeamVLinkerError::MissingBeamSource {
                system_id: context.stump_system.system_id,
                source,
            },
        )?;
        let cross = generic_intersection(vertical, beam.median);
        let within_horizontal_margin = beam.median.x1 - f64::from(margin) <= cross.x
            && cross.x <= beam.median.x2 + f64::from(margin);
        checks.push(NativeStemsBeamSiblingCheck {
            source,
            cross,
            within_horizontal_margin,
        });
        if within_horizontal_margin {
            hits.push(NativeStemsBeamSiblingHit { source, cross });
        }
    }
    hits.sort_by(|left, right| java_double_order(left.cross.y, right.cross.y));
    Ok((checks, hits))
}

fn theoretical_line(reference: NativeStemPoint, y_limit: f64, slope: f64) -> NativeStemLine {
    let skewed = Segment {
        x1: reference.x,
        y1: reference.y,
        x2: reference.x - (100.0 * slope),
        y2: reference.y + 100.0,
    };
    let horizontal = Segment {
        x1: 0.0,
        y1: y_limit,
        x2: 100.0,
        y2: y_limit,
    };
    NativeStemLine {
        start: reference,
        stop: generic_intersection(skewed, horizontal),
    }
}

fn get_target_point(reference: NativeStemPoint, limit: Segment, slope: f64) -> NativeStemPoint {
    generic_intersection(
        Segment {
            x1: reference.x,
            y1: reference.y,
            x2: reference.x - (100.0 * slope),
            y2: reference.y + 100.0,
        },
        limit,
    )
}

pub(crate) fn generic_intersection(one: Segment, two: Segment) -> NativeStemPoint {
    let denominator =
        ((one.x1 - one.x2) * (two.y1 - two.y2)) - ((one.y1 - one.y2) * (two.x1 - two.x2));
    let one_cross = (one.x1 * one.y2) - (one.y1 * one.x2);
    let two_cross = (two.x1 * two.y2) - (two.y1 * two.x2);
    NativeStemPoint {
        x: java_canonical_divide(
            (one_cross * (two.x1 - two.x2)) - ((one.x1 - one.x2) * two_cross),
            denominator,
        ),
        y: java_canonical_divide(
            (one_cross * (two.y1 - two.y2)) - ((one.y1 - one.y2) * two_cross),
            denominator,
        ),
    }
}

fn java_canonical_divide(numerator: f64, denominator: f64) -> f64 {
    let quotient = numerator / denominator;
    if quotient.is_nan() {
        f64::NAN
    } else {
        quotient
    }
}

pub(crate) fn beam_border(
    beam: &NativeStemsBeamStumpBeam,
    side: NativeStemVerticalSide,
) -> Segment {
    let delta_y = if side == NativeStemVerticalSide::Top {
        -beam.height / 2.0
    } else {
        beam.height / 2.0
    };
    Segment {
        x1: beam.median.x1,
        y1: beam.median.y1 + delta_y,
        x2: beam.median.x2,
        y2: beam.median.y2 + delta_y,
    }
}

fn find_beam(
    system: &NativeStemsBeamStumpSystem,
    source: NativeStemsBeamSource,
) -> Option<&NativeStemsBeamStumpBeam> {
    system
        .beams_by_abscissa
        .iter()
        .find(|beam| beam.source == source)
}

fn resolve_source(
    beams: &NativeBeamRecognition,
    system_id: usize,
    source: NativeStemsBeamSource,
) -> Result<&crate::beam_inters::RawBeam, NativeStemsBeamVLinkerError> {
    let pair = match source {
        NativeStemsBeamSource::RawBeam(ordinal) => beams.raw_beams.get(ordinal),
        NativeStemsBeamSource::Hook(ordinal) => beams.hooks.get(ordinal),
    }
    .ok_or(NativeStemsBeamVLinkerError::MissingBeamSource { system_id, source })?;
    if pair.0 != system_id {
        return Err(NativeStemsBeamVLinkerError::BeamSourceMismatch { system_id, source });
    }
    Ok(&pair.1)
}

/// Java `Staff.getAreaBounds()` at the BeamLinker boundary.
///
/// The live GRID area's path is intentionally also retained for
/// `StaffManager.getClosestStaff`, but its native builder currently omits the
/// `StaffManager.verticalAreaMargin` translation.  Part bounds read
/// `Staff.getAreaBounds()` instead, so reconstruct that separate observable by
/// translating every neighbor-provided north/south boundary by 0.9 of the
/// sheet interline.  Sheet-edge boundaries are the only ones Java leaves
/// untranslated.
fn java_staff_area_integer_bounds(
    grid: &GridLinesRecognition,
    system_id: usize,
    staff_id: usize,
) -> Result<JavaRectangle, NativeStemsBeamVLinkerError> {
    let staff_area = grid
        .staff_areas
        .iter()
        .find(|area| area.staff_id == staff_id)
        .ok_or(NativeStemsBeamVLinkerError::MissingStaffArea {
            system_id,
            staff_id,
        })?;
    if staff_area.area.system_id != system_id {
        return Err(NativeStemsBeamVLinkerError::MissingStaffArea {
            system_id,
            staff_id,
        });
    }
    let sheet_width = i32::try_from(grid.scale.width).unwrap_or(i32::MAX);
    let sheet_height = i32::try_from(grid.scale.height).unwrap_or(i32::MAX);
    let margin = to_pixels(
        grid.scale.scale.interline.main,
        STAFF_VERTICAL_AREA_MARGIN_RATIO,
    );
    Ok(apply_staff_area_margin(
        population_system_area_integer_bounds(&staff_area.area),
        staff_area.area.north(),
        staff_area.area.south(),
        sheet_width,
        sheet_height,
        margin,
    ))
}

fn apply_staff_area_margin(
    mut bounds: JavaRectangle,
    north: &StaffBoundary,
    south: &StaffBoundary,
    sheet_width: i32,
    sheet_height: i32,
    margin: i32,
) -> JavaRectangle {
    let north_margin = if is_sheet_edge_boundary(north, sheet_width, 0) {
        0
    } else {
        margin
    };
    let south_margin = if is_sheet_edge_boundary(south, sheet_width, sheet_height) {
        0
    } else {
        margin
    };
    bounds.y = bounds.y.wrapping_add(north_margin);
    bounds.height = bounds
        .height
        .wrapping_sub(north_margin)
        .wrapping_sub(south_margin);
    bounds
}

fn is_sheet_edge_boundary(boundary: &StaffBoundary, sheet_width: i32, y: i32) -> bool {
    matches!(
        boundary.segments.as_slice(),
        [BoundarySegment::Line {
            start: (start_x, start_y),
            end: (end_x, end_y),
        }] if *start_x == 0.0
            && *start_y == f64::from(y)
            && *end_x == f64::from(sheet_width)
            && *end_y == f64::from(y)
    )
}

fn rectangle_union(one: JavaRectangle, two: JavaRectangle) -> JavaRectangle {
    let left = one.x.min(two.x);
    let top = one.y.min(two.y);
    let right = one
        .x
        .wrapping_add(one.width)
        .max(two.x.wrapping_add(two.width));
    let bottom = one
        .y
        .wrapping_add(one.height)
        .max(two.y.wrapping_add(two.height));
    JavaRectangle {
        x: left,
        y: top,
        width: right.wrapping_sub(left),
        height: bottom.wrapping_sub(top),
    }
}

fn bounds_to_rectangle(bounds: audiveris_image::section::Bounds) -> Option<JavaRectangle> {
    Some(JavaRectangle {
        x: i32::try_from(bounds.x).ok()?,
        y: i32::try_from(bounds.y).ok()?,
        width: i32::try_from(bounds.width).ok()?,
        height: i32::try_from(bounds.height).ok()?,
    })
}

fn quadrilateral_double_bounds(
    quadrilateral: [NativeStemPoint; 4],
) -> Result<NativeStemsBeamDoubleBounds, NativeStemsBeamVLinkerError> {
    if !quadrilateral
        .iter()
        .flat_map(|point| [point.x, point.y])
        .all(f64::is_finite)
    {
        return Err(NativeStemsBeamVLinkerError::InvalidGeometry);
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
    Ok(NativeStemsBeamDoubleBounds {
        x: minimum_x,
        y: minimum_y,
        width: maximum_x - minimum_x,
        height: maximum_y - minimum_y,
    })
}

fn double_bounds_to_integer(bounds: NativeStemsBeamDoubleBounds) -> JavaRectangle {
    let left = bounds.x.floor() as i32;
    let top = bounds.y.floor() as i32;
    let right = (bounds.x + bounds.width).ceil() as i32;
    let bottom = (bounds.y + bounds.height).ceil() as i32;
    JavaRectangle {
        x: left,
        y: top,
        width: right.wrapping_sub(left),
        height: bottom.wrapping_sub(top),
    }
}

pub(crate) fn convex_quad_intersects_rectangle(
    quadrilateral: [NativeStemPoint; 4],
    rectangle: JavaRectangle,
) -> bool {
    if rectangle.is_empty() {
        return false;
    }
    let left = f64::from(rectangle.x);
    let top = f64::from(rectangle.y);
    let right = left + f64::from(rectangle.width);
    let bottom = top + f64::from(rectangle.height);
    let polygon = quadrilateral.map(|point| (point.x, point.y));
    let rectangle = [(left, top), (right, top), (right, bottom), (left, bottom)];
    if separating_axis(&polygon, &rectangle, (1.0, 0.0))
        || separating_axis(&polygon, &rectangle, (0.0, 1.0))
    {
        return false;
    }
    for index in 0..polygon.len() {
        let start = polygon[index];
        let stop = polygon[(index + 1) % polygon.len()];
        let axis = (-(stop.1 - start.1), stop.0 - start.0);
        if axis != (0.0, 0.0) && separating_axis(&polygon, &rectangle, axis) {
            return false;
        }
    }
    true
}

fn separating_axis(
    polygon: &[(f64, f64); 4],
    rectangle: &[(f64, f64); 4],
    axis: (f64, f64),
) -> bool {
    let projection = |point: (f64, f64)| (point.0 * axis.0) + (point.1 * axis.1);
    let range = |points: &[(f64, f64); 4]| {
        points.iter().copied().map(projection).fold(
            (f64::INFINITY, f64::NEG_INFINITY),
            |(minimum, maximum), value| (minimum.min(value), maximum.max(value)),
        )
    };
    let (polygon_minimum, polygon_maximum) = range(polygon);
    let (rectangle_minimum, rectangle_maximum) = range(rectangle);
    !polygon_minimum.is_finite()
        || !polygon_maximum.is_finite()
        || polygon_maximum <= rectangle_minimum
        || rectangle_maximum <= polygon_minimum
}

fn segment_intersects(one: Segment, two: Segment) -> bool {
    relative_ccw(one.x1, one.y1, one.x2, one.y2, two.x1, two.y1)
        * relative_ccw(one.x1, one.y1, one.x2, one.y2, two.x2, two.y2)
        <= 0
        && relative_ccw(two.x1, two.y1, two.x2, two.y2, one.x1, one.y1)
            * relative_ccw(two.x1, two.y1, two.x2, two.y2, one.x2, one.y2)
            <= 0
}

pub(crate) fn relative_ccw(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    mut point_x: f64,
    mut point_y: f64,
) -> i32 {
    let delta_x = x2 - x1;
    let delta_y = y2 - y1;
    point_x -= x1;
    point_y -= y1;
    let mut ccw = (point_x * delta_y) - (point_y * delta_x);
    if ccw == 0.0 {
        ccw = (point_x * delta_x) + (point_y * delta_y);
        if ccw > 0.0 {
            point_x -= delta_x;
            point_y -= delta_y;
            ccw = (point_x * delta_x) + (point_y * delta_y);
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

const fn vertical_side_direction(side: NativeStemVerticalSide) -> i32 {
    match side {
        NativeStemVerticalSide::Top => -1,
        NativeStemVerticalSide::Bottom => 1,
    }
}

const fn opposite_vertical_side(side: NativeStemVerticalSide) -> NativeStemVerticalSide {
    match side {
        NativeStemVerticalSide::Top => NativeStemVerticalSide::Bottom,
        NativeStemVerticalSide::Bottom => NativeStemVerticalSide::Top,
    }
}

const fn head_side_direction(side: NativeStemHeadSide) -> i32 {
    match side {
        NativeStemHeadSide::Left => -1,
        NativeStemHeadSide::Right => 1,
    }
}

const fn head_side_index(side: NativeStemHeadSide) -> usize {
    match side {
        NativeStemHeadSide::Left => 0,
        NativeStemHeadSide::Right => 1,
    }
}

fn to_pixels(interline: i32, ratio: f64) -> i32 {
    (f64::from(interline) * ratio).round_ties_even() as i32
}

fn is_aligned_alien(delta_y: f64, delta_x: f64, maximum_y: i32, maximum_x: i32) -> bool {
    delta_y <= f64::from(maximum_y) && delta_x < f64::from(maximum_x)
}

fn is_good_beam_grade(grade: f64) -> bool {
    grade
        .partial_cmp(&GOOD_BEAM_GRADE)
        .is_some_and(Ordering::is_ge)
}

fn beam_kind_is_hook(kind: crate::beam_inters::BeamKind) -> bool {
    kind == crate::beam_inters::BeamKind::Hook
}

// Keep the exact determinant-specialized helper visible to the unit pins that
// guard low-bit Java operation order in this module.
fn intersection_at_x(line: Segment, x: f64) -> NativeStemPoint {
    let (x, y) = line_util_intersection(line.x1, line.y1, line.x2, line.y2, x);
    NativeStemPoint { x, y }
}

#[allow(dead_code)]
fn java_double_order(one: f64, two: f64) -> Ordering {
    one.total_cmp(&two)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn determinant_intersection_retains_java_low_bits() {
        let line = Segment {
            x1: 17.3,
            y1: -4.2,
            x2: 102.7,
            y2: 33.9,
        };
        let point = intersection_at_x(line, 41.25);
        assert_eq!(point.x.to_bits(), 0x4044_a000_0000_0001);
        assert_eq!(point.y.to_bits(), 0x4019_f097_8fc4_86c5);

        let crossing = generic_intersection(
            Segment {
                x1: 23.125,
                y1: 71.75,
                x2: 10.625,
                y2: 171.75,
            },
            Segment {
                x1: 0.0,
                y1: 127.375,
                x2: 100.0,
                y2: 127.375,
            },
        );
        assert_eq!(crossing.x.to_bits(), 0x4030_2c00_0000_0000);
        assert_eq!(crossing.y.to_bits(), 0x405f_d800_0000_0000);

        let parallel = generic_intersection(
            Segment {
                x1: 10.0,
                y1: 10.0,
                x2: 30.0,
                y2: 10.0,
            },
            Segment {
                x1: 10.0,
                y1: 22.0,
                x2: 30.0,
                y2: 22.0,
            },
        );
        assert_eq!(parallel.x.to_bits(), 0xfff0_0000_0000_0000);
        assert_eq!(parallel.y.to_bits(), 0x7ff8_0000_0000_0000);
    }

    #[test]
    fn staff_area_bounds_apply_java_margin_only_to_neighbor_lines() {
        let horizontal = |y: f64| StaffBoundary {
            segments: vec![BoundarySegment::Line {
                start: (0.0, y),
                end: (2450.0, y),
            }],
        };
        let top_edge = horizontal(0.0);
        let upper_neighbor = horizontal(436.0);
        let lower_neighbor = horizontal(609.0);
        let next_neighbor = horizontal(952.0);

        assert_eq!(
            apply_staff_area_margin(
                JavaRectangle::new(0, 0, 2450, 609),
                &top_edge,
                &lower_neighbor,
                2450,
                952,
                19,
            ),
            JavaRectangle::new(0, 0, 2450, 590),
        );
        assert_eq!(
            apply_staff_area_margin(
                JavaRectangle::new(0, 436, 2450, 516),
                &upper_neighbor,
                &next_neighbor,
                2450,
                3508,
                19,
            ),
            JavaRectangle::new(0, 455, 2450, 478),
        );
    }

    #[test]
    fn java_line_intersection_includes_endpoint_and_collinear_overlap() {
        let horizontal = Segment {
            x1: 0.0,
            y1: 0.0,
            x2: 10.0,
            y2: 0.0,
        };
        assert!(segment_intersects(
            horizontal,
            Segment {
                x1: 10.0,
                y1: 0.0,
                x2: 10.0,
                y2: 7.0,
            }
        ));
        assert!(segment_intersects(
            horizontal,
            Segment {
                x1: 4.0,
                y1: 0.0,
                x2: 12.0,
                y2: 0.0,
            }
        ));
        assert!(!segment_intersects(
            horizontal,
            Segment {
                x1: 11.0,
                y1: 0.0,
                x2: 11.0,
                y2: 7.0,
            }
        ));
    }

    #[test]
    fn lookup_quad_requires_positive_area_and_uses_floor_ceil_bounds() {
        let quadrilateral = [
            NativeStemPoint { x: 0.25, y: 1.75 },
            NativeStemPoint { x: 10.25, y: 1.75 },
            NativeStemPoint { x: 12.5, y: 11.125 },
            NativeStemPoint { x: -2.5, y: 11.125 },
        ];
        let double = quadrilateral_double_bounds(quadrilateral).expect("finite quad");
        assert_eq!(
            double,
            NativeStemsBeamDoubleBounds {
                x: -2.5,
                y: 1.75,
                width: 15.0,
                height: 9.375,
            }
        );
        assert_eq!(
            double_bounds_to_integer(double),
            JavaRectangle::new(-3, 1, 16, 11)
        );
        assert!(convex_quad_intersects_rectangle(
            quadrilateral,
            JavaRectangle::new(0, 2, 1, 1)
        ));
        assert!(!convex_quad_intersects_rectangle(
            [
                NativeStemPoint { x: 0.0, y: 0.0 },
                NativeStemPoint { x: 10.0, y: 0.0 },
                NativeStemPoint { x: 10.0, y: 10.0 },
                NativeStemPoint { x: 0.0, y: 10.0 },
            ],
            JavaRectangle::new(10, 2, 2, 2)
        ));
    }

    #[test]
    fn stable_double_order_and_alien_thresholds_match_java() {
        let mut rows = [(0_usize, 3.0), (1, 3.0), (2, -0.0), (3, 0.0)];
        rows.sort_by(|left, right| java_double_order(left.1, right.1));
        assert_eq!(rows.map(|row| row.0), [2, 3, 0, 1]);

        assert!(is_aligned_alien(100.0, 4.999_999_999, 100, 5));
        assert!(!is_aligned_alien(100.0, 5.0, 100, 5));
        assert!(!is_aligned_alien(100.000_000_001, 0.0, 100, 5));
    }

    #[test]
    fn alien_hook_class_comes_from_beam_kind_not_source_store() {
        use crate::beam_inters::BeamKind;

        assert!(beam_kind_is_hook(BeamKind::Hook));
        assert!(!beam_kind_is_hook(BeamKind::Beam));
        assert!(!beam_kind_is_hook(BeamKind::SmallBeam));
    }
}
