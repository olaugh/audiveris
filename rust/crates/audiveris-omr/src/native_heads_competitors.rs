// SPDX-License-Identifier: AGPL-3.0-or-later

//! Native materialization of the SIG competitors consumed by HEADS.
//!
//! Java filters the live SIG vertex set in insertion order, applies `isGood`,
//! thin-vertical, and short-beam-group decisions, then stable-sorts accepted
//! competitors by their integer bounds ordinate. This adapter starts from the
//! retained GRID and BEAMS products and preserves both orders. It allocates no
//! synthetic SIG identities.

use std::{collections::BTreeSet, error::Error, fmt};

use audiveris_image::{
    beam_structure::Segment,
    run_table::{Orientation, RunTable, RunTableError},
    staff_peak::StaffPeak,
};

use crate::{
    beam_inters::{BeamKind, RawBeam, beam_bounds},
    head_scanner_slices::{JavaRectangle, VerticalRibbonArea},
    native_heads_obstacles::{
        NativeHeadsBarClass, NativeHeadsBarObstacle, NativeHeadsBarObstacleError,
        NativeHeadsBarShape, materialize_native_heads_bar_obstacles,
    },
    recognize::{GridLinesRecognition, NativeBeamRecognition, NativeMultipleRestDescriptor},
    stem_seeds_step::{StemScaleComputation, compute_stem_scale},
};

const GOOD_BARLINE_GRADE: f64 = 0.6;
const GOOD_BAR_CONNECTOR_GRADE: f64 = 0.65;
const GOOD_BEAM_GRADE: f64 = 0.35;
const GOOD_INTER_GRADE: f64 = 0.4;
const MIN_BEAM_WIDTH: f64 = 2.5;

/// Java class of a currently materializable competing SIG node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeHeadsCompetitorClass {
    BarlineInter,
    BarConnectorInter,
    BeamInter,
    BeamHookInter,
    SmallBeamInter,
    MultipleRestInter,
    VerticalSerifInter,
}

impl NativeHeadsCompetitorClass {
    #[must_use]
    pub const fn java_name(self) -> &'static str {
        match self {
            Self::BarlineInter => "BarlineInter",
            Self::BarConnectorInter => "BarConnectorInter",
            Self::BeamInter => "BeamInter",
            Self::BeamHookInter => "BeamHookInter",
            Self::SmallBeamInter => "SmallBeamInter",
            Self::MultipleRestInter => "MultipleRestInter",
            Self::VerticalSerifInter => "VerticalSerifInter",
        }
    }

    const fn good_grade(self) -> f64 {
        match self {
            Self::BarlineInter => GOOD_BARLINE_GRADE,
            Self::BarConnectorInter => GOOD_BAR_CONNECTOR_GRADE,
            Self::BeamInter | Self::BeamHookInter | Self::SmallBeamInter => GOOD_BEAM_GRADE,
            Self::MultipleRestInter | Self::VerticalSerifInter => GOOD_INTER_GRADE,
        }
    }
}

/// Java shape of a currently materializable competing SIG node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeHeadsCompetitorShape {
    ThinBarline,
    ThickBarline,
    ThinConnector,
    ThickConnector,
    Beam,
    BeamHook,
    BeamSmall,
    MultipleRest,
    VerticalSerif,
}

impl NativeHeadsCompetitorShape {
    #[must_use]
    pub const fn java_name(self) -> &'static str {
        match self {
            Self::ThinBarline => "THIN_BARLINE",
            Self::ThickBarline => "THICK_BARLINE",
            Self::ThinConnector => "THIN_CONNECTOR",
            Self::ThickConnector => "THICK_CONNECTOR",
            Self::Beam => "BEAM",
            Self::BeamHook => "BEAM_HOOK",
            Self::BeamSmall => "BEAM_SMALL",
            Self::MultipleRest => "MULTIPLE_REST",
            Self::VerticalSerif => "VERTICAL_SERIF",
        }
    }
}

/// Retained producer identity, without inventing a Java `InterIndex` value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeHeadsCompetitorSource {
    GridInter(usize),
    RawBeam(usize),
    Hook(usize),
    MultipleRest(usize),
    MultipleRestSerif {
        rest_ordinal: usize,
        side: NativeHeadsSerifSide,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeHeadsSerifSide {
    Left,
    Right,
}

/// Geometry read by HEADS and recorded by the frozen differential oracle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NativeHeadsCompetitorGeometry {
    Vertical { median: Segment, width: f64 },
    Horizontal { median: Segment, height: f64 },
}

/// Evidence used by Java's post-`isGood` competitor predicate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeHeadsCompetitorEvidence {
    None,
    VerticalFloor(i32),
    BeamGroup {
        has_long_beam: bool,
        member_widths: Vec<i32>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeHeadsCompetitorDecision {
    Accept,
    NotGood,
    ThinVertical,
    ShortBeamGroup,
}

/// Identity-free summary of the fixed glyph attached during BEAMS.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeHeadsCompetitorGlyph {
    pub bounds: JavaRectangle,
    pub weight: usize,
    pub run_digest: u64,
}

/// One live competing SIG node before Java's final ordinate sort.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadsCompetitor {
    pub source: NativeHeadsCompetitorSource,
    pub source_ordinal: usize,
    pub accepted_ordinal: Option<usize>,
    pub class: NativeHeadsCompetitorClass,
    pub shape: NativeHeadsCompetitorShape,
    pub staff_id: Option<usize>,
    pub bounds: JavaRectangle,
    pub grade: f64,
    pub best_grade: f64,
    pub good: bool,
    pub frozen: bool,
    pub geometry: NativeHeadsCompetitorGeometry,
    pub glyph: Option<NativeHeadsCompetitorGlyph>,
    pub evidence: NativeHeadsCompetitorEvidence,
    pub decision: NativeHeadsCompetitorDecision,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadsCompetitorSystem {
    pub system_id: usize,
    pub max_stem: i32,
    pub min_beam_width: i32,
    pub candidates_in_sig_order: Vec<NativeHeadsCompetitor>,
    pub accepted_by_ordinate: Vec<NativeHeadsCompetitor>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeHeadsCompetitorPool {
    pub systems: Vec<NativeHeadsCompetitorSystem>,
}

#[derive(Debug)]
pub enum NativeHeadsCompetitorError {
    BarObstacles(NativeHeadsBarObstacleError),
    RunTable(RunTableError),
    InvalidMaximumStem(i32),
    MissingGridSystem(usize),
    MissingGridInter {
        system_id: usize,
        inter_id: usize,
    },
    MissingBeamGroupSystem(usize),
    MissingBeamGroup {
        system_id: usize,
        source: NativeHeadsCompetitorSource,
    },
    MissingHookBase {
        system_id: usize,
        hook_ordinal: usize,
    },
    InvalidBeamGroupMember {
        system_id: usize,
        member_ordinal: usize,
    },
    InconsistentPostRestBeams,
    InconsistentBeamGlyphs,
    MissingSerifImpacts {
        rest_ordinal: usize,
        side: NativeHeadsSerifSide,
    },
}

impl fmt::Display for NativeHeadsCompetitorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BarObstacles(error) => write!(formatter, "cannot materialize GRID bars: {error}"),
            Self::RunTable(error) => {
                write!(formatter, "cannot measure the HEADS stem scale: {error}")
            }
            Self::InvalidMaximumStem(value) => {
                write!(formatter, "invalid maximum stem width {value}")
            }
            Self::MissingGridSystem(system_id) => {
                write!(formatter, "missing GRID system {system_id}")
            }
            Self::MissingGridInter {
                system_id,
                inter_id,
            } => write!(
                formatter,
                "missing GRID inter {inter_id} in system {system_id}"
            ),
            Self::MissingBeamGroupSystem(system_id) => {
                write!(formatter, "missing beam-group state for system {system_id}")
            }
            Self::MissingBeamGroup { system_id, source } => {
                write!(
                    formatter,
                    "{source:?} in system {system_id} has no beam group"
                )
            }
            Self::MissingHookBase {
                system_id,
                hook_ordinal,
            } => write!(
                formatter,
                "hook {hook_ordinal} in system {system_id} has no geometric source beam"
            ),
            Self::InvalidBeamGroupMember {
                system_id,
                member_ordinal,
            } => write!(
                formatter,
                "beam group in system {system_id} references missing member {member_ordinal}"
            ),
            Self::InconsistentPostRestBeams => {
                formatter.write_str("post-MultipleRest beam state does not match source removals")
            }
            Self::InconsistentBeamGlyphs => formatter.write_str(
                "registered beam glyph state does not match raw beam and hook identities",
            ),
            Self::MissingSerifImpacts { rest_ordinal, side } => {
                write!(
                    formatter,
                    "multiple rest {rest_ordinal} {side:?} serif lacks impacts"
                )
            }
        }
    }
}

impl Error for NativeHeadsCompetitorError {}

impl From<NativeHeadsBarObstacleError> for NativeHeadsCompetitorError {
    fn from(value: NativeHeadsBarObstacleError) -> Self {
        Self::BarObstacles(value)
    }
}

impl From<RunTableError> for NativeHeadsCompetitorError {
    fn from(value: RunTableError) -> Self {
        Self::RunTable(value)
    }
}

/// Materialize Java `NoteHeadsBuilder.getSystemCompetitors` from production
/// GRID and BEAMS state.
pub fn materialize_native_heads_competitors(
    grid: &GridLinesRecognition,
    beams: &NativeBeamRecognition,
) -> Result<NativeHeadsCompetitorPool, NativeHeadsCompetitorError> {
    let bars = materialize_native_heads_bar_obstacles(grid)?;
    let max_stem = maximum_stem_width(grid)?;
    let min_beam_width =
        (f64::from(grid.scale.scale.interline.main) * MIN_BEAM_WIDTH).round_ties_even() as i32;
    validate_post_rest_beams(beams)?;
    validate_beam_glyphs(beams)?;

    let systems = bars
        .systems
        .iter()
        .map(|bar_system| {
            let system_id = bar_system.system_id;
            let grid_system = grid
                .peak_graph
                .sig
                .systems
                .iter()
                .find(|system| system.system_id == system_id)
                .ok_or(NativeHeadsCompetitorError::MissingGridSystem(system_id))?;
            let group_members = grouping_members(beams, system_id);
            let group_state = beams
                .group_memberships
                .iter()
                .find(|membership| membership.system_id == system_id)
                .ok_or(NativeHeadsCompetitorError::MissingBeamGroupSystem(
                    system_id,
                ))?;
            let removed = beams
                .multiple_rests
                .iter()
                .map(|rest| rest.source_beam_ordinal)
                .chain(
                    beams
                        .high_precision_rejected_raw_beam_ordinals
                        .iter()
                        .copied(),
                )
                .collect::<BTreeSet<_>>();
            let mut candidates = Vec::new();

            for obstacle in &bar_system.candidates_in_sig_order {
                let node = grid_system
                    .sig
                    .nodes_in_order()
                    .find_map(|(id, node)| (id.value() == obstacle.source_inter_id).then_some(node))
                    .ok_or(NativeHeadsCompetitorError::MissingGridInter {
                        system_id,
                        inter_id: obstacle.source_inter_id,
                    })?;
                let grade = node.intrinsic_grade();
                let best_grade = node.contextual_grade().unwrap_or(grade);
                candidates.push(bar_candidate(*obstacle, grade, best_grade));
            }

            for (raw_ordinal, beam) in raw_beams_in_sig_order(beams, system_id, &removed) {
                let (_, glyph) = &beams.raw_beam_glyphs[raw_ordinal];
                candidates.push(beam_candidate(
                    NativeHeadsCompetitorSource::RawBeam(raw_ordinal),
                    beam,
                    glyph,
                    beam_group_evidence(
                        system_id,
                        NativeHeadsCompetitorSource::RawBeam(raw_ordinal),
                        &group_members,
                        &group_state.groups,
                        &removed,
                        min_beam_width,
                    )?,
                ));
            }
            for (hook_ordinal, hook) in hooks_in_sig_order(beams, system_id)? {
                let (_, glyph) = &beams.hook_glyphs[hook_ordinal];
                candidates.push(beam_candidate(
                    NativeHeadsCompetitorSource::Hook(hook_ordinal),
                    hook,
                    glyph,
                    beam_group_evidence(
                        system_id,
                        NativeHeadsCompetitorSource::Hook(hook_ordinal),
                        &group_members,
                        &group_state.groups,
                        &removed,
                        min_beam_width,
                    )?,
                ));
            }

            for (rest_ordinal, rest) in beams.multiple_rests.iter().enumerate() {
                if rest.system_id != system_id {
                    continue;
                }
                candidates.push(rest_candidate(rest_ordinal, rest));
                candidates.push(serif_candidate(
                    rest_ordinal,
                    NativeHeadsSerifSide::Left,
                    &rest.left_peak,
                    grid.scale.scale.line.main,
                )?);
                candidates.push(serif_candidate(
                    rest_ordinal,
                    NativeHeadsSerifSide::Right,
                    &rest.right_peak,
                    grid.scale.scale.line.main,
                )?);
            }

            finalize_system(system_id, max_stem, min_beam_width, candidates)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(NativeHeadsCompetitorPool { systems })
}

fn maximum_stem_width(grid: &GridLinesRecognition) -> Result<i32, NativeHeadsCompetitorError> {
    let pixels = grid.no_staff.to_pixels();
    let horizontal = RunTable::from_pixels(
        Orientation::Horizontal,
        grid.scale.width,
        grid.scale.height,
        &pixels,
    )?;
    let lengths = (0..horizontal.sequence_count())
        .filter_map(|index| horizontal.sequence(index))
        .flat_map(|runs| runs.iter().map(|run| run.length as i32))
        .collect::<Vec<_>>();
    let scale = &grid.scale.scale;
    let stem = compute_stem_scale(
        &lengths,
        StemScaleComputation {
            interline: scale.interline.main,
            foreground_main: scale.line.main,
            foreground_maximum: scale.line.max,
            minimum_value_ratio: 0.1,
            minimum_derivative_ratio: 0.05,
            minimum_gain_ratio: 0.1,
            stem_as_foreground_ratio: 1.0,
        },
    );
    if stem.maximum <= 0 {
        return Err(NativeHeadsCompetitorError::InvalidMaximumStem(stem.maximum));
    }
    Ok(stem.maximum)
}

fn validate_post_rest_beams(
    beams: &NativeBeamRecognition,
) -> Result<(), NativeHeadsCompetitorError> {
    let removed = beams
        .multiple_rests
        .iter()
        .map(|rest| rest.source_beam_ordinal)
        .chain(
            beams
                .high_precision_rejected_raw_beam_ordinals
                .iter()
                .copied(),
        )
        .collect::<BTreeSet<_>>();
    let expected = beams
        .raw_beams
        .iter()
        .enumerate()
        .filter_map(|(ordinal, beam)| (!removed.contains(&ordinal)).then_some(*beam))
        .collect::<Vec<_>>();
    if expected != beams.beams_after_multiple_rests {
        return Err(NativeHeadsCompetitorError::InconsistentPostRestBeams);
    }
    Ok(())
}

fn validate_beam_glyphs(beams: &NativeBeamRecognition) -> Result<(), NativeHeadsCompetitorError> {
    let raw_valid = beams.raw_beams.len() == beams.raw_beam_glyphs.len()
        && beams
            .raw_beams
            .iter()
            .zip(&beams.raw_beam_glyphs)
            .all(|((beam_system, _), (glyph_system, _))| beam_system == glyph_system);
    let hooks_valid = beams.hooks.len() == beams.hook_glyphs.len()
        && beams
            .hooks
            .iter()
            .zip(&beams.hook_glyphs)
            .all(|((beam_system, _), (glyph_system, _))| beam_system == glyph_system);
    if !raw_valid || !hooks_valid {
        return Err(NativeHeadsCompetitorError::InconsistentBeamGlyphs);
    }
    Ok(())
}

fn raw_beams_in_sig_order(
    beams: &NativeBeamRecognition,
    system_id: usize,
    removed: &BTreeSet<usize>,
) -> Vec<(usize, RawBeam)> {
    let mut entries = beams
        .raw_beams
        .iter()
        .enumerate()
        .filter(|(ordinal, (id, _))| *id == system_id && !removed.contains(ordinal))
        .map(|(ordinal, (_, beam))| {
            let spot = source_spot_key(beams, *beam);
            (ordinal, *beam, spot)
        })
        .collect::<Vec<_>>();
    // Java sorts source spots by top, left, and glyph id before creating any
    // inters. `extendBeams` products have no single source spot and are added
    // only after that initial pass. Stable sorting retains hook-before-beam and
    // multi-item creation order within one spot.
    entries.sort_by_key(|(ordinal, _, spot)| match spot {
        Some((top, left, source_index)) => (false, *top, *left, *source_index, *ordinal),
        None => (true, 0, 0, 0, *ordinal),
    });
    entries
        .into_iter()
        .map(|(ordinal, beam, _)| (ordinal, beam))
        .collect()
}

fn source_spot_key(beams: &NativeBeamRecognition, beam: RawBeam) -> Option<(i32, i32, usize)> {
    let bounds = beam_bounds(beam.item);
    let beam_right = bounds.x + bounds.width;
    let beam_bottom = bounds.y + bounds.height;
    beams
        .black_head_sizing
        .traces
        .iter()
        .filter_map(|trace| {
            let spot = &trace.source_component;
            let spot_right = spot.left + spot.width as i32;
            let spot_bottom = spot.top + spot.height as i32;
            // An initial item's full horizontal span belongs to its spot. Area
            // rounding can extend the item by one pixel at either side. A
            // merger spans two source spots and deliberately matches neither.
            if bounds.x < spot.left - 1
                || beam_right > spot_right + 1
                || bounds.y >= spot_bottom
                || beam_bottom <= spot.top
            {
                return None;
            }
            let overlap_top = bounds.y.max(spot.top);
            let overlap_bottom = beam_bottom.min(spot_bottom);
            let overlap_height = overlap_bottom - overlap_top;
            Some((
                overlap_height,
                -(bounds.width - spot.width as i32).abs(),
                trace.source_index,
                spot.top,
                spot.left,
            ))
        })
        .max_by_key(|(overlap, width_fit, source_index, _, _)| {
            (*overlap, *width_fit, std::cmp::Reverse(*source_index))
        })
        .map(|(_, _, source_index, top, left)| (top, left, source_index))
}

fn hooks_in_sig_order(
    beams: &NativeBeamRecognition,
    system_id: usize,
) -> Result<Vec<(usize, RawBeam)>, NativeHeadsCompetitorError> {
    // Hooks are created while browsing BeamInter bases in their current SIG
    // order, TOP before BOTTOM. Recover that base order from the initial spot
    // identities above; the retained hook ordinal remains the grouping identity.
    let ordered_bases = raw_beams_in_sig_order(beams, system_id, &BTreeSet::new())
        .into_iter()
        .filter(|(_, beam)| beam.kind == BeamKind::Beam)
        .collect::<Vec<_>>();
    let mut hooks = beams
        .hooks
        .iter()
        .enumerate()
        .filter(|(_, (id, _))| *id == system_id)
        .map(|(hook_ordinal, (_, hook))| {
            let hook_bounds = beam_bounds(hook.item);
            let hook_center_y = hook.item.median.y1 + hook.item.median.y2;
            let base = ordered_bases
                .iter()
                .enumerate()
                .filter_map(|(base_rank, (_, base))| {
                    let base_bounds = beam_bounds(base.item);
                    let overlap = (hook_bounds.x + hook_bounds.width)
                        .min(base_bounds.x + base_bounds.width)
                        - hook_bounds.x.max(base_bounds.x);
                    if overlap <= 0 {
                        return None;
                    }
                    let base_center_y = base.item.median.y1 + base.item.median.y2;
                    let center_distance = (hook_center_y - base_center_y).abs();
                    Some((
                        std::cmp::Reverse(center_distance.to_bits()),
                        overlap,
                        std::cmp::Reverse(base_rank),
                        base_rank,
                        hook_center_y < base_center_y,
                    ))
                })
                .max_by_key(|(distance, overlap, rank, ..)| (*distance, *overlap, *rank))
                .ok_or(NativeHeadsCompetitorError::MissingHookBase {
                    system_id,
                    hook_ordinal,
                })?;
            let base_rank = base.3;
            let side_rank = if base.4 { 0_u8 } else { 1_u8 };
            let source_spot = source_spot_key(beams, *hook);
            Ok::<_, NativeHeadsCompetitorError>((
                hook_ordinal,
                *hook,
                base_rank,
                side_rank,
                source_spot,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    hooks.sort_by_key(|(hook_ordinal, _, base_rank, side_rank, source_spot)| {
        let (missing_spot, top, left, source_index) = match source_spot {
            Some((top, left, source_index)) => (false, *top, *left, *source_index),
            None => (true, 0, 0, 0),
        };
        (
            *base_rank,
            *side_rank,
            missing_spot,
            top,
            left,
            source_index,
            *hook_ordinal,
        )
    });
    Ok(hooks
        .into_iter()
        .map(|(ordinal, hook, ..)| (ordinal, hook))
        .collect())
}

fn bar_candidate(
    obstacle: NativeHeadsBarObstacle,
    grade: f64,
    best_grade: f64,
) -> NativeHeadsCompetitor {
    let (class, shape) = match (obstacle.class, obstacle.shape) {
        (NativeHeadsBarClass::BarlineInter, NativeHeadsBarShape::ThinBarline) => (
            NativeHeadsCompetitorClass::BarlineInter,
            NativeHeadsCompetitorShape::ThinBarline,
        ),
        (NativeHeadsBarClass::BarlineInter, NativeHeadsBarShape::ThickBarline) => (
            NativeHeadsCompetitorClass::BarlineInter,
            NativeHeadsCompetitorShape::ThickBarline,
        ),
        (NativeHeadsBarClass::BarConnectorInter, NativeHeadsBarShape::ThinConnector) => (
            NativeHeadsCompetitorClass::BarConnectorInter,
            NativeHeadsCompetitorShape::ThinConnector,
        ),
        (NativeHeadsBarClass::BarConnectorInter, NativeHeadsBarShape::ThickConnector) => (
            NativeHeadsCompetitorClass::BarConnectorInter,
            NativeHeadsCompetitorShape::ThickConnector,
        ),
        _ => unreachable!("bar obstacle class and shape are internally consistent"),
    };
    let width_floor = obstacle.thickness.floor() as i32;
    NativeHeadsCompetitor {
        source: NativeHeadsCompetitorSource::GridInter(obstacle.source_inter_id),
        source_ordinal: 0,
        accepted_ordinal: None,
        class,
        shape,
        staff_id: obstacle.staff_id,
        bounds: obstacle.area_bounds,
        grade,
        best_grade,
        good: grade >= class.good_grade(),
        frozen: obstacle.frozen,
        geometry: NativeHeadsCompetitorGeometry::Vertical {
            median: obstacle.median,
            width: obstacle.thickness,
        },
        glyph: None,
        evidence: NativeHeadsCompetitorEvidence::VerticalFloor(width_floor),
        decision: NativeHeadsCompetitorDecision::Accept,
    }
}

fn beam_candidate(
    source: NativeHeadsCompetitorSource,
    beam: RawBeam,
    glyph: &crate::beam_inters::RegisteredBeamGlyph,
    evidence: NativeHeadsCompetitorEvidence,
) -> NativeHeadsCompetitor {
    let (class, shape) = match beam.kind {
        BeamKind::Beam => (
            NativeHeadsCompetitorClass::BeamInter,
            NativeHeadsCompetitorShape::Beam,
        ),
        BeamKind::Hook => (
            NativeHeadsCompetitorClass::BeamHookInter,
            NativeHeadsCompetitorShape::BeamHook,
        ),
        BeamKind::SmallBeam => (
            NativeHeadsCompetitorClass::SmallBeamInter,
            NativeHeadsCompetitorShape::BeamSmall,
        ),
    };
    let bounds = glyph.bounds;
    NativeHeadsCompetitor {
        source,
        source_ordinal: 0,
        accepted_ordinal: None,
        class,
        shape,
        staff_id: None,
        bounds: JavaRectangle {
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: bounds.height,
        },
        grade: beam.grade,
        best_grade: beam.grade,
        good: beam.grade >= class.good_grade(),
        frozen: false,
        geometry: NativeHeadsCompetitorGeometry::Horizontal {
            median: beam.item.median,
            height: beam.item.height,
        },
        glyph: Some(NativeHeadsCompetitorGlyph {
            bounds: JavaRectangle {
                x: glyph.bounds.x,
                y: glyph.bounds.y,
                width: glyph.bounds.width,
                height: glyph.bounds.height,
            },
            weight: glyph.weight(),
            run_digest: glyph.run_digest(),
        }),
        evidence,
        decision: NativeHeadsCompetitorDecision::Accept,
    }
}

fn rest_candidate(
    rest_ordinal: usize,
    rest: &NativeMultipleRestDescriptor,
) -> NativeHeadsCompetitor {
    NativeHeadsCompetitor {
        source: NativeHeadsCompetitorSource::MultipleRest(rest_ordinal),
        source_ordinal: 0,
        accepted_ordinal: None,
        class: NativeHeadsCompetitorClass::MultipleRestInter,
        shape: NativeHeadsCompetitorShape::MultipleRest,
        staff_id: Some(rest.staff_id),
        bounds: JavaRectangle {
            x: rest.source_bounds.x,
            y: rest.source_bounds.y,
            width: rest.source_bounds.width,
            height: rest.source_bounds.height,
        },
        grade: rest.grade,
        best_grade: rest.grade,
        good: rest.grade >= GOOD_INTER_GRADE,
        frozen: false,
        geometry: NativeHeadsCompetitorGeometry::Horizontal {
            median: rest.source_median,
            height: rest.height,
        },
        glyph: None,
        evidence: NativeHeadsCompetitorEvidence::None,
        decision: NativeHeadsCompetitorDecision::Accept,
    }
}

fn serif_candidate(
    rest_ordinal: usize,
    side: NativeHeadsSerifSide,
    peak: &StaffPeak,
    foreground_thickness: i32,
) -> Result<NativeHeadsCompetitor, NativeHeadsCompetitorError> {
    let impacts = peak
        .impacts()
        .ok_or(NativeHeadsCompetitorError::MissingSerifImpacts { rest_ordinal, side })?;
    let half_line = f64::from(foreground_thickness) / 2.0;
    let width = f64::from(peak.width());
    let x = f64::from(peak.start()) + (width / 2.0);
    let median = Segment {
        x1: x,
        y1: f64::from(peak.top()) - half_line + 0.5,
        x2: x,
        y2: f64::from(peak.bottom()) + half_line + 0.5,
    };
    let bounds = VerticalRibbonArea::new(median, width).integer_bounds();
    let grade = impacts.grade();
    Ok(NativeHeadsCompetitor {
        source: NativeHeadsCompetitorSource::MultipleRestSerif { rest_ordinal, side },
        source_ordinal: 0,
        accepted_ordinal: None,
        class: NativeHeadsCompetitorClass::VerticalSerifInter,
        shape: NativeHeadsCompetitorShape::VerticalSerif,
        staff_id: None,
        bounds,
        grade,
        best_grade: grade,
        good: grade >= GOOD_INTER_GRADE,
        frozen: false,
        geometry: NativeHeadsCompetitorGeometry::Vertical { median, width },
        glyph: None,
        evidence: NativeHeadsCompetitorEvidence::VerticalFloor(width.floor() as i32),
        decision: NativeHeadsCompetitorDecision::Accept,
    })
}

#[derive(Clone, Copy)]
struct GroupingMember {
    source: NativeHeadsCompetitorSource,
    width: i32,
}

fn grouping_members(beams: &NativeBeamRecognition, system_id: usize) -> Vec<GroupingMember> {
    beams
        .raw_beams
        .iter()
        .enumerate()
        .filter(|(_, (id, _))| *id == system_id)
        .map(|(ordinal, (_, beam))| GroupingMember {
            source: NativeHeadsCompetitorSource::RawBeam(ordinal),
            width: beam_bounds(beam.item).width,
        })
        .chain(
            beams
                .hooks
                .iter()
                .enumerate()
                .filter(|(_, (id, _))| *id == system_id)
                .map(|(ordinal, (_, beam))| GroupingMember {
                    source: NativeHeadsCompetitorSource::Hook(ordinal),
                    width: beam_bounds(beam.item).width,
                }),
        )
        .collect()
}

fn beam_group_evidence(
    system_id: usize,
    source: NativeHeadsCompetitorSource,
    members: &[GroupingMember],
    groups: &[Vec<usize>],
    removed_raw_beams: &BTreeSet<usize>,
    min_beam_width: i32,
) -> Result<NativeHeadsCompetitorEvidence, NativeHeadsCompetitorError> {
    let source_member = members
        .iter()
        .position(|member| member.source == source)
        .ok_or(NativeHeadsCompetitorError::MissingBeamGroup { system_id, source })?;
    let group = groups
        .iter()
        .find(|group| group.contains(&source_member))
        .ok_or(NativeHeadsCompetitorError::MissingBeamGroup { system_id, source })?;
    let mut member_widths = Vec::new();
    for member_ordinal in group {
        let member = members.get(*member_ordinal).ok_or(
            NativeHeadsCompetitorError::InvalidBeamGroupMember {
                system_id,
                member_ordinal: *member_ordinal,
            },
        )?;
        if matches!(member.source, NativeHeadsCompetitorSource::RawBeam(ordinal) if removed_raw_beams.contains(&ordinal))
        {
            continue;
        }
        member_widths.push(member.width);
    }
    let has_long_beam = member_widths.iter().any(|width| *width >= min_beam_width);
    Ok(NativeHeadsCompetitorEvidence::BeamGroup {
        has_long_beam,
        member_widths,
    })
}

fn finalize_system(
    system_id: usize,
    max_stem: i32,
    min_beam_width: i32,
    mut candidates: Vec<NativeHeadsCompetitor>,
) -> Result<NativeHeadsCompetitorSystem, NativeHeadsCompetitorError> {
    for (source_ordinal, candidate) in candidates.iter_mut().enumerate() {
        candidate.source_ordinal = source_ordinal;
        candidate.decision = if !candidate.good {
            NativeHeadsCompetitorDecision::NotGood
        } else {
            match &candidate.evidence {
                NativeHeadsCompetitorEvidence::None => NativeHeadsCompetitorDecision::Accept,
                NativeHeadsCompetitorEvidence::VerticalFloor(width) if *width <= max_stem => {
                    NativeHeadsCompetitorDecision::ThinVertical
                }
                NativeHeadsCompetitorEvidence::VerticalFloor(_) => {
                    NativeHeadsCompetitorDecision::Accept
                }
                NativeHeadsCompetitorEvidence::BeamGroup {
                    has_long_beam: true,
                    ..
                } => NativeHeadsCompetitorDecision::Accept,
                NativeHeadsCompetitorEvidence::BeamGroup {
                    has_long_beam: false,
                    ..
                } => NativeHeadsCompetitorDecision::ShortBeamGroup,
            }
        };
    }
    let mut accepted_sources = candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            (candidate.decision == NativeHeadsCompetitorDecision::Accept).then_some(index)
        })
        .collect::<Vec<_>>();
    accepted_sources.sort_by_key(|index| candidates[*index].bounds.y);
    for (accepted_ordinal, source) in accepted_sources.iter().copied().enumerate() {
        candidates[source].accepted_ordinal = Some(accepted_ordinal);
    }
    let accepted_by_ordinate = accepted_sources
        .into_iter()
        .map(|source| candidates[source].clone())
        .collect();
    Ok(NativeHeadsCompetitorSystem {
        system_id,
        max_stem,
        min_beam_width,
        candidates_in_sig_order: candidates,
        accepted_by_ordinate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(
        y: i32,
        good: bool,
        evidence: NativeHeadsCompetitorEvidence,
    ) -> NativeHeadsCompetitor {
        NativeHeadsCompetitor {
            source: NativeHeadsCompetitorSource::GridInter(0),
            source_ordinal: 0,
            accepted_ordinal: None,
            class: NativeHeadsCompetitorClass::BarlineInter,
            shape: NativeHeadsCompetitorShape::ThinBarline,
            staff_id: Some(1),
            bounds: JavaRectangle::new(0, y, 10, 10),
            grade: if good { 0.8 } else { 0.2 },
            best_grade: if good { 0.9 } else { 0.3 },
            good,
            frozen: true,
            geometry: NativeHeadsCompetitorGeometry::Vertical {
                median: Segment {
                    x1: 5.0,
                    y1: f64::from(y),
                    x2: 5.0,
                    y2: f64::from(y + 10),
                },
                width: 10.0,
            },
            glyph: None,
            evidence,
            decision: NativeHeadsCompetitorDecision::Accept,
        }
    }

    #[test]
    fn decisions_apply_good_first_and_accepted_sort_is_stable() {
        let system = finalize_system(
            1,
            4,
            50,
            vec![
                candidate(0, false, NativeHeadsCompetitorEvidence::VerticalFloor(10)),
                candidate(5, true, NativeHeadsCompetitorEvidence::VerticalFloor(4)),
                candidate(
                    4,
                    true,
                    NativeHeadsCompetitorEvidence::BeamGroup {
                        has_long_beam: false,
                        member_widths: vec![20, 30],
                    },
                ),
                candidate(3, true, NativeHeadsCompetitorEvidence::None),
                candidate(3, true, NativeHeadsCompetitorEvidence::VerticalFloor(5)),
            ],
        )
        .unwrap();
        assert_eq!(
            system
                .candidates_in_sig_order
                .iter()
                .map(|candidate| candidate.decision)
                .collect::<Vec<_>>(),
            [
                NativeHeadsCompetitorDecision::NotGood,
                NativeHeadsCompetitorDecision::ThinVertical,
                NativeHeadsCompetitorDecision::ShortBeamGroup,
                NativeHeadsCompetitorDecision::Accept,
                NativeHeadsCompetitorDecision::Accept,
            ]
        );
        assert_eq!(
            system
                .accepted_by_ordinate
                .iter()
                .map(|candidate| candidate.source_ordinal)
                .collect::<Vec<_>>(),
            [3, 4]
        );
        assert_eq!(
            system
                .candidates_in_sig_order
                .iter()
                .map(|candidate| candidate.accepted_ordinal)
                .collect::<Vec<_>>(),
            [None, None, None, Some(0), Some(1)]
        );
    }

    #[test]
    fn removed_multiple_rest_source_no_longer_makes_group_long() {
        let members = [
            GroupingMember {
                source: NativeHeadsCompetitorSource::RawBeam(10),
                width: 100,
            },
            GroupingMember {
                source: NativeHeadsCompetitorSource::RawBeam(11),
                width: 30,
            },
        ];
        let evidence = beam_group_evidence(
            1,
            NativeHeadsCompetitorSource::RawBeam(11),
            &members,
            &[vec![0, 1]],
            &BTreeSet::from([10]),
            50,
        )
        .unwrap();
        assert_eq!(
            evidence,
            NativeHeadsCompetitorEvidence::BeamGroup {
                has_long_beam: false,
                member_widths: vec![30],
            }
        );
    }
}
