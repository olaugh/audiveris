// SPDX-License-Identifier: AGPL-3.0-or-later

//! Identity-free STEMS seed purge and head-corner seed selection.
//!
//! Java purges `VERTICAL_SEED` glyphs that substantially overlap connected
//! barline ribbons before it constructs any `HeadLinker`. Each corner then
//! looks for the first nearby surviving seed that is horizontally admissible
//! and extends far enough away from the head. This module stops when Java
//! would call the mutating `buildStump()` fallback.

use std::{cmp::Ordering, collections::BTreeMap, error::Error, fmt};

use audiveris_image::{
    bars_logic::VerticalInterKind,
    beam_structure::Segment,
    grid_sig::{GridInterId, GridSigNode, GridSigRelation},
    section::Bounds,
};

use crate::{
    beam_recognizer::run_table_center_line,
    head_scanner_slices::{JavaRectangle, population_system_area_integer_bounds},
    native_stem_seeds::{NativeStemSeedGlyph, NativeStemSeedRecognition},
    native_stems_head_corners::{
        NativeStemsHeadCorner, NativeStemsHeadCornerHead, NativeStemsHeadCornerRecognition,
    },
    recognize::GridLinesRecognition,
    stems_step::{NativeStemHeadSide, NativeStemPoint, NativeStemRect, NativeStemVerticalSide},
};

const MAX_BAR_OVERLAP: f64 = 0.25;

/// Complete production boundary immediately before section-based stump build.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadSeedRecognition {
    pub systems: Vec<NativeStemsHeadSeedSystem>,
    pub input_seed_count: usize,
    pub kept_seed_count: usize,
    pub purged_seed_count: usize,
    pub corner_count: usize,
    pub selected_seed_count: usize,
    pub section_fallback_count: usize,
    pub visited_candidate_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadSeedSystem {
    pub system_id: usize,
    pub no_stem_areas: Vec<NativeStemsNoStemArea>,
    pub seeds_in_free_order: Vec<NativeStemsSeedPurge>,
    pub kept_seed_ordinals: Vec<usize>,
    /// Java `inspectStems` head-construction order: stable by abscissa.
    pub heads_by_abscissa: Vec<NativeStemsHeadSeedHead>,
}

/// One top, bottom, or middle connected-bar ribbon after Java's stable x sort.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsNoStemArea {
    pub ordinal: usize,
    pub source_bar_inter_id: usize,
    pub target_bar_inter_id: usize,
    pub kind: NativeStemsNoStemAreaKind,
    pub median: Segment,
    pub thickness: f64,
    pub bounds: NativeStemRect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeStemsNoStemAreaKind {
    Top,
    Bottom,
    Middle,
}

/// Java's evidence for retaining or removing one free seed.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsSeedPurge {
    pub free_glyph_ordinal: usize,
    pub source_ordinal: usize,
    pub bounds: JavaRectangle,
    pub weight: usize,
    pub run_digest: u64,
    pub intersections: Vec<NativeStemsSeedBarIntersection>,
    pub kept_ordinal: Option<usize>,
    pub first_purging_area: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsSeedBarIntersection {
    pub area_ordinal: usize,
    pub intersection_bounds: NativeStemRect,
    pub seed_size: f64,
    pub bar_bounds_size: f64,
    pub ratio: f64,
    pub purges: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadSeedHead {
    pub x_ordinal: usize,
    pub sig_ordinal: usize,
    pub system_creation_ordinal: usize,
    pub corners_in_constructor_order: Vec<NativeStemsHeadSeedCorner>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStemsHeadSeedCorner {
    pub constructor_ordinal: usize,
    pub seed_area: NativeStemRect,
    /// Kept free-glyph ordinals after the head-vicinity filter.
    pub neighbor_seed_ordinals: Vec<usize>,
    /// Kept free-glyph ordinals before the stable distance sort.
    pub intersected_seed_ordinals: Vec<usize>,
    pub checks: Vec<NativeStemsHeadSeedCheck>,
    pub selected_seed_ordinal: Option<usize>,
    /// True means Java proceeds to the mutating section-based `buildStump()`.
    pub section_fallback_required: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeStemsHeadSeedCheck {
    pub free_glyph_ordinal: usize,
    pub pre_sort_ordinal: usize,
    pub distance_sq: f64,
    pub seed_x: f64,
    pub signed_delta: f64,
    pub rounded_dx: i32,
    pub ref_y: i32,
    pub extension_dy: i32,
    pub horizontal_gate: bool,
    pub stands_out: bool,
    pub stands_out_evaluated: bool,
    pub accepted: bool,
    pub visited: bool,
    pub selected: bool,
}

#[derive(Debug)]
pub enum NativeStemsHeadSeedError {
    SystemOrder {
        grid: Vec<usize>,
        seeds: Vec<usize>,
        corners: Vec<usize>,
    },
    MissingGridBar {
        system_id: usize,
        inter_id: usize,
    },
    MissingSystemArea {
        system_id: usize,
    },
    MissingSeedCenterLine {
        system_id: usize,
        free_glyph_ordinal: usize,
    },
    NonBarlineConnection {
        system_id: usize,
        inter_id: usize,
    },
    SeedBoundsOutOfRange {
        system_id: usize,
        free_glyph_ordinal: usize,
    },
    HeadOrder {
        system_id: usize,
    },
    InvalidParameters {
        system_id: usize,
        interline: i32,
    },
}

impl fmt::Display for NativeStemsHeadSeedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SystemOrder {
                grid,
                seeds,
                corners,
            } => write!(
                formatter,
                "STEMS seed system order differs: GRID {grid:?}, seeds {seeds:?}, corners \
                 {corners:?}"
            ),
            Self::MissingGridBar {
                system_id,
                inter_id,
            } => write!(
                formatter,
                "STEMS seed system {system_id} connection references missing bar {inter_id}"
            ),
            Self::MissingSystemArea { system_id } => {
                write!(formatter, "STEMS seed system {system_id} has no GRID area")
            }
            Self::MissingSeedCenterLine {
                system_id,
                free_glyph_ordinal,
            } => write!(
                formatter,
                "STEMS seed system {system_id} free glyph {free_glyph_ordinal} has no center line"
            ),
            Self::NonBarlineConnection {
                system_id,
                inter_id,
            } => write!(
                formatter,
                "STEMS seed system {system_id} connection endpoint {inter_id} is not a barline"
            ),
            Self::SeedBoundsOutOfRange {
                system_id,
                free_glyph_ordinal,
            } => write!(
                formatter,
                "STEMS seed system {system_id} free glyph {free_glyph_ordinal} exceeds Java \
                 rectangle coordinates"
            ),
            Self::HeadOrder { system_id } => write!(
                formatter,
                "STEMS seed system {system_id} head order differs from the corner product"
            ),
            Self::InvalidParameters {
                system_id,
                interline,
            } => write!(
                formatter,
                "STEMS seed system {system_id} has invalid interline {interline}"
            ),
        }
    }
}

impl Error for NativeStemsHeadSeedError {}

/// Purge no-stem seeds and select existing seeds for all four corners.
pub fn materialize_native_stems_head_seeds(
    grid: &GridLinesRecognition,
    stem_seeds: &NativeStemSeedRecognition,
    corners: &NativeStemsHeadCornerRecognition,
) -> Result<NativeStemsHeadSeedRecognition, NativeStemsHeadSeedError> {
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
    let corner_ids = corners
        .systems
        .iter()
        .map(|system| system.system_id)
        .collect::<Vec<_>>();
    if grid_ids != seed_ids || grid_ids != corner_ids {
        return Err(NativeStemsHeadSeedError::SystemOrder {
            grid: grid_ids,
            seeds: seed_ids,
            corners: corner_ids,
        });
    }

    let mut systems = Vec::with_capacity(corners.systems.len());
    let mut totals = [0_usize; 7];
    for ((grid_system, seed_system), corner_system) in grid
        .peak_graph
        .sig
        .systems
        .iter()
        .zip(&stem_seeds.systems)
        .zip(&corners.systems)
    {
        let system_id = corner_system.system_id;
        let interline = corner_system.interline;
        if interline <= 0 {
            return Err(NativeStemsHeadSeedError::InvalidParameters {
                system_id,
                interline,
            });
        }
        let no_stem_areas = no_stem_areas(grid_system)?;
        let (seed_rows, kept_seed_ordinals) =
            purge_seeds(system_id, &seed_system.free_glyphs, &no_stem_areas)?;
        let kept = kept_seed_ordinals
            .iter()
            .map(|&ordinal| {
                let seed = &seed_system.free_glyphs[ordinal];
                let left = i32::try_from(seed.bounds.x).map_err(|_| {
                    NativeStemsHeadSeedError::SeedBoundsOutOfRange {
                        system_id,
                        free_glyph_ordinal: ordinal,
                    }
                })?;
                let top = i32::try_from(seed.bounds.y).map_err(|_| {
                    NativeStemsHeadSeedError::SeedBoundsOutOfRange {
                        system_id,
                        free_glyph_ordinal: ordinal,
                    }
                })?;
                let center_line = run_table_center_line(&seed.run_table, left, top).ok_or(
                    NativeStemsHeadSeedError::MissingSeedCenterLine {
                        system_id,
                        free_glyph_ordinal: ordinal,
                    },
                )?;
                Ok(KeptSeed {
                    free_glyph_ordinal: ordinal,
                    seed,
                    center_line,
                })
            })
            .collect::<Result<Vec<_>, NativeStemsHeadSeedError>>()?;

        let system_area = grid
            .system_areas
            .iter()
            .find(|area| area.system_id == system_id)
            .ok_or(NativeStemsHeadSeedError::MissingSystemArea { system_id })?;
        let system_area_bounds = population_system_area_integer_bounds(system_area);

        let min_head_stump_dy = to_pixels(interline, 0.5);
        let max_head_seed_dy = to_pixels(interline, 0.25);
        let mut system_heads = Vec::with_capacity(corner_system.heads_in_sig_order.len());
        for (x_ordinal, &sig_ordinal) in corner_system.heads_by_abscissa.iter().enumerate() {
            let head = corner_system
                .heads_in_sig_order
                .get(sig_ordinal)
                .ok_or(NativeStemsHeadSeedError::HeadOrder { system_id })?;
            let corners_in_constructor_order = head
                .corners_in_constructor_order
                .iter()
                .map(|corner| {
                    select_corner_seed(
                        head,
                        corner,
                        &kept,
                        system_area_bounds,
                        interline,
                        max_head_seed_dy,
                        min_head_stump_dy,
                        corner_system.max_head_in_dx,
                        corner_system.max_head_out_dx,
                    )
                })
                .collect::<Vec<_>>();
            totals[3] += corners_in_constructor_order.len();
            totals[4] += corners_in_constructor_order
                .iter()
                .filter(|corner| corner.selected_seed_ordinal.is_some())
                .count();
            totals[5] += corners_in_constructor_order
                .iter()
                .filter(|corner| corner.section_fallback_required)
                .count();
            totals[6] += corners_in_constructor_order
                .iter()
                .flat_map(|corner| &corner.checks)
                .filter(|check| check.visited)
                .count();
            system_heads.push(NativeStemsHeadSeedHead {
                x_ordinal,
                sig_ordinal,
                system_creation_ordinal: head.system_creation_ordinal,
                corners_in_constructor_order,
            });
        }

        totals[0] += seed_rows.len();
        totals[1] += kept_seed_ordinals.len();
        totals[2] += seed_rows.len() - kept_seed_ordinals.len();
        systems.push(NativeStemsHeadSeedSystem {
            system_id,
            no_stem_areas,
            seeds_in_free_order: seed_rows,
            kept_seed_ordinals,
            heads_by_abscissa: system_heads,
        });
    }

    Ok(NativeStemsHeadSeedRecognition {
        systems,
        input_seed_count: totals[0],
        kept_seed_count: totals[1],
        purged_seed_count: totals[2],
        corner_count: totals[3],
        selected_seed_count: totals[4],
        section_fallback_count: totals[5],
        visited_candidate_count: totals[6],
    })
}

fn no_stem_areas(
    system: &crate::grid_executor::HeadlessSystemSigState,
) -> Result<Vec<NativeStemsNoStemArea>, NativeStemsHeadSeedError> {
    let system_id = system.system_id;
    let mut bars = BTreeMap::new();
    for (inter_id, node) in system.sig.nodes_in_order() {
        if let GridSigNode::Vertical { plan, .. } = node {
            let VerticalInterKind::Barline { .. } = plan.kind else {
                continue;
            };
            bars.insert(
                inter_id,
                (
                    Segment {
                        x1: plan.median.x,
                        y1: plan.median.top,
                        x2: plan.median.x,
                        y2: plan.median.bottom,
                    },
                    plan.width,
                ),
            );
        }
    }

    let mut unsorted = Vec::new();
    for (source_id, _) in system.sig.nodes_in_order() {
        for edge in system.sig.edges() {
            if edge.source != source_id
                || !matches!(edge.relation, GridSigRelation::BarConnectionSupport { .. })
            {
                continue;
            }
            let (top_median, top_width) = bar(&bars, system_id, edge.source)?;
            let (bottom_median, bottom_width) = bar(&bars, system_id, edge.target)?;
            unsorted.push(area(
                edge.source,
                edge.target,
                NativeStemsNoStemAreaKind::Top,
                top_median,
                top_width,
            ));
            unsorted.push(area(
                edge.source,
                edge.target,
                NativeStemsNoStemAreaKind::Bottom,
                bottom_median,
                bottom_width,
            ));
            unsorted.push(area(
                edge.source,
                edge.target,
                NativeStemsNoStemAreaKind::Middle,
                Segment {
                    x1: top_median.x2,
                    y1: top_median.y2,
                    x2: bottom_median.x1,
                    y2: bottom_median.y1,
                },
                (top_width + bottom_width) / 2.0,
            ));
        }
    }
    unsorted.sort_by(|left, right| {
        left.bounds
            .x
            .partial_cmp(&right.bounds.x)
            .unwrap_or(Ordering::Equal)
    });
    for (ordinal, area) in unsorted.iter_mut().enumerate() {
        area.ordinal = ordinal;
    }
    Ok(unsorted)
}

fn bar(
    bars: &BTreeMap<GridInterId, (Segment, f64)>,
    system_id: usize,
    inter_id: GridInterId,
) -> Result<(Segment, f64), NativeStemsHeadSeedError> {
    bars.get(&inter_id)
        .copied()
        .ok_or(NativeStemsHeadSeedError::MissingGridBar {
            system_id,
            inter_id: inter_id.value(),
        })
}

fn area(
    source: GridInterId,
    target: GridInterId,
    kind: NativeStemsNoStemAreaKind,
    median: Segment,
    thickness: f64,
) -> NativeStemsNoStemArea {
    NativeStemsNoStemArea {
        ordinal: 0,
        source_bar_inter_id: source.value(),
        target_bar_inter_id: target.value(),
        kind,
        median,
        thickness,
        bounds: ribbon_bounds(median, thickness),
    }
}

fn purge_seeds(
    system_id: usize,
    seeds: &[NativeStemSeedGlyph],
    areas: &[NativeStemsNoStemArea],
) -> Result<(Vec<NativeStemsSeedPurge>, Vec<usize>), NativeStemsHeadSeedError> {
    let mut rows = Vec::with_capacity(seeds.len());
    let mut kept = Vec::new();
    for (free_glyph_ordinal, seed) in seeds.iter().enumerate() {
        let bounds =
            java_bounds(seed.bounds).ok_or(NativeStemsHeadSeedError::SeedBoundsOutOfRange {
                system_id,
                free_glyph_ordinal,
            })?;
        // Java performs `int * int` before widening the product to double.
        let seed_size = f64::from(bounds.width.wrapping_mul(bounds.height));
        let mut intersections = Vec::new();
        let mut first_purging_area = None;
        for area in areas {
            let Some(intersection_bounds) = clipped_ribbon_bounds(area, bounds) else {
                continue;
            };
            let intersection_size = intersection_bounds.width * intersection_bounds.height;
            let bar_bounds_size = area.bounds.width * area.bounds.height;
            let ratio = intersection_size / bar_bounds_size.min(seed_size);
            let purges = ratio > MAX_BAR_OVERLAP;
            intersections.push(NativeStemsSeedBarIntersection {
                area_ordinal: area.ordinal,
                intersection_bounds,
                seed_size,
                bar_bounds_size,
                ratio,
                purges,
            });
            if purges {
                first_purging_area = Some(area.ordinal);
                break;
            }
        }
        let kept_ordinal = first_purging_area.is_none().then(|| {
            let ordinal = kept.len();
            kept.push(free_glyph_ordinal);
            ordinal
        });
        rows.push(NativeStemsSeedPurge {
            free_glyph_ordinal,
            source_ordinal: seed.source_ordinal,
            bounds,
            weight: seed.weight,
            run_digest: seed.run_digest(),
            intersections,
            kept_ordinal,
            first_purging_area,
        });
    }
    Ok((rows, kept))
}

#[allow(clippy::too_many_arguments)]
fn select_corner_seed(
    head: &NativeStemsHeadCornerHead,
    corner: &NativeStemsHeadCorner,
    kept: &[KeptSeed<'_>],
    system_bounds: JavaRectangle,
    interline: i32,
    max_head_seed_dy: i32,
    min_head_stump_dy: i32,
    max_head_in_dx: i32,
    max_head_out_dx: i32,
) -> NativeStemsHeadSeedCorner {
    let (left, right) = match corner.horizontal {
        NativeStemHeadSide::Right => (corner.in_point.x, corner.out_point.x),
        NativeStemHeadSide::Left => (corner.out_point.x, corner.in_point.x),
    };
    let seed_area = NativeStemRect {
        x: left,
        y: corner.reference.y - f64::from(max_head_seed_dy),
        width: right - left,
        height: f64::from(2 * max_head_seed_dy),
    };
    let vicinity_margin = to_pixels(interline, 1.0);
    let vicinity = JavaRectangle::new(
        head.bounds.x.wrapping_sub(vicinity_margin),
        system_bounds.y,
        head.bounds
            .width
            .wrapping_add(vicinity_margin.wrapping_mul(2)),
        system_bounds.height,
    );
    let neighbors = kept
        .iter()
        .filter(|candidate| {
            java_bounds(candidate.seed.bounds).is_some_and(|bounds| vicinity.intersects(bounds))
        })
        .collect::<Vec<_>>();
    let neighbor_seed_ordinals = neighbors
        .iter()
        .map(|candidate| candidate.free_glyph_ordinal)
        .collect::<Vec<_>>();
    let mut candidates = neighbors
        .iter()
        .filter(|candidate| rectangle2d_intersects(seed_area, candidate.seed.bounds))
        .enumerate()
        .map(|(pre_sort_ordinal, candidate)| SeedCandidate {
            free_glyph_ordinal: candidate.free_glyph_ordinal,
            pre_sort_ordinal,
            seed: candidate.seed,
            center_line: candidate.center_line,
            distance_sq: point_segment_distance_sq(
                (candidate.center_line.x1, candidate.center_line.y1),
                (candidate.center_line.x2, candidate.center_line.y2),
                corner.reference,
            ),
        })
        .collect::<Vec<_>>();
    let intersected_seed_ordinals = candidates
        .iter()
        .map(|candidate| candidate.free_glyph_ordinal)
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.distance_sq.total_cmp(&right.distance_sq));

    let x_direction = match corner.horizontal {
        NativeStemHeadSide::Left => -1.0,
        NativeStemHeadSide::Right => 1.0,
    };
    let ref_y = corner.reference.y.round_ties_even() as i32;
    let mut checks = Vec::new();
    let mut selected_seed_ordinal = None;
    for candidate in candidates {
        let seed_x = java_line_x_at_y(
            (candidate.center_line.x1, candidate.center_line.y1),
            (candidate.center_line.x2, candidate.center_line.y2),
            corner.reference.y,
        );
        let signed_delta = x_direction * (seed_x - corner.reference.x);
        let rounded_dx = java_round_to_i32(signed_delta);
        let horizontal_gate = (rounded_dx >= 0 && rounded_dx <= max_head_out_dx)
            || (rounded_dx <= 0 && -rounded_dx <= max_head_in_dx);
        let bounds = candidate.seed.bounds;
        let extension_dy = match corner.vertical {
            NativeStemVerticalSide::Bottom => bounds.y as i32 + bounds.height as i32 - 1 - ref_y,
            NativeStemVerticalSide::Top => ref_y - bounds.y as i32,
        };
        let stands_out = extension_dy >= min_head_stump_dy;
        let accepted = horizontal_gate && stands_out;
        let visited = selected_seed_ordinal.is_none();
        let selected = visited && accepted;
        checks.push(NativeStemsHeadSeedCheck {
            free_glyph_ordinal: candidate.free_glyph_ordinal,
            pre_sort_ordinal: candidate.pre_sort_ordinal,
            distance_sq: candidate.distance_sq,
            seed_x,
            signed_delta,
            rounded_dx,
            ref_y,
            extension_dy,
            horizontal_gate,
            stands_out,
            stands_out_evaluated: visited && horizontal_gate,
            accepted,
            visited,
            selected,
        });
        if selected {
            selected_seed_ordinal = Some(candidate.free_glyph_ordinal);
        }
    }

    NativeStemsHeadSeedCorner {
        constructor_ordinal: corner.constructor_ordinal,
        seed_area,
        neighbor_seed_ordinals,
        intersected_seed_ordinals,
        checks,
        selected_seed_ordinal,
        section_fallback_required: selected_seed_ordinal.is_none(),
    }
}

struct SeedCandidate<'a> {
    free_glyph_ordinal: usize,
    pre_sort_ordinal: usize,
    seed: &'a NativeStemSeedGlyph,
    center_line: Segment,
    distance_sq: f64,
}

struct KeptSeed<'a> {
    free_glyph_ordinal: usize,
    seed: &'a NativeStemSeedGlyph,
    center_line: Segment,
}

fn java_bounds(bounds: Bounds) -> Option<JavaRectangle> {
    Some(JavaRectangle::new(
        i32::try_from(bounds.x).ok()?,
        i32::try_from(bounds.y).ok()?,
        i32::try_from(bounds.width).ok()?,
        i32::try_from(bounds.height).ok()?,
    ))
}

fn to_pixels(interline: i32, fraction: f64) -> i32 {
    (f64::from(interline) * fraction).round_ties_even() as i32
}

fn rectangle2d_intersects(rectangle: NativeStemRect, bounds: Bounds) -> bool {
    rectangle.width > 0.0
        && rectangle.height > 0.0
        && rectangle.x + rectangle.width > bounds.x as f64
        && rectangle.y + rectangle.height > bounds.y as f64
        && rectangle.x < (bounds.x + bounds.width) as f64
        && rectangle.y < (bounds.y + bounds.height) as f64
}

fn ribbon_bounds(median: Segment, thickness: f64) -> NativeStemRect {
    let half = thickness / 2.0;
    let left = median.x1.min(median.x2) - half;
    let right = median.x1.max(median.x2) + half;
    let top = median.y1.min(median.y2);
    let bottom = median.y1.max(median.y2);
    NativeStemRect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    }
}

fn clipped_ribbon_bounds(
    area: &NativeStemsNoStemArea,
    rectangle: JavaRectangle,
) -> Option<NativeStemRect> {
    if rectangle.is_empty() {
        return None;
    }
    let half = area.thickness / 2.0;
    let mut polygon = vec![
        NativeStemPoint {
            x: area.median.x1 - half,
            y: area.median.y1,
        },
        NativeStemPoint {
            x: area.median.x2 - half,
            y: area.median.y2,
        },
        NativeStemPoint {
            x: area.median.x2 + half,
            y: area.median.y2,
        },
        NativeStemPoint {
            x: area.median.x1 + half,
            y: area.median.y1,
        },
    ];
    let left = f64::from(rectangle.x);
    let top = f64::from(rectangle.y);
    let right = left + f64::from(rectangle.width);
    let bottom = top + f64::from(rectangle.height);
    for edge in [
        ClipEdge::Left(left),
        ClipEdge::Right(right),
        ClipEdge::Top(top),
        ClipEdge::Bottom(bottom),
    ] {
        polygon = clip_polygon(&polygon, edge);
        if polygon.is_empty() {
            return None;
        }
    }
    let twice_area = polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .map(|(one, two)| (one.x * two.y) - (two.x * one.y))
        .sum::<f64>()
        .abs();
    if twice_area == 0.0 {
        return None;
    }
    let min_x = polygon
        .iter()
        .map(|point| point.x)
        .fold(f64::INFINITY, f64::min);
    let max_x = polygon
        .iter()
        .map(|point| point.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = polygon
        .iter()
        .map(|point| point.y)
        .fold(f64::INFINITY, f64::min);
    let max_y = polygon
        .iter()
        .map(|point| point.y)
        .fold(f64::NEG_INFINITY, f64::max);
    Some(NativeStemRect {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    })
}

#[derive(Clone, Copy)]
enum ClipEdge {
    Left(f64),
    Right(f64),
    Top(f64),
    Bottom(f64),
}

fn clip_polygon(input: &[NativeStemPoint], edge: ClipEdge) -> Vec<NativeStemPoint> {
    let mut output = Vec::new();
    let Some(mut previous) = input.last().copied() else {
        return output;
    };
    let mut previous_inside = clip_inside(previous, edge);
    for &current in input {
        let current_inside = clip_inside(current, edge);
        if current_inside != previous_inside {
            output.push(clip_intersection(previous, current, edge));
        }
        if current_inside {
            output.push(current);
        }
        previous = current;
        previous_inside = current_inside;
    }
    output
}

fn clip_inside(point: NativeStemPoint, edge: ClipEdge) -> bool {
    match edge {
        ClipEdge::Left(x) => point.x >= x,
        ClipEdge::Right(x) => point.x <= x,
        ClipEdge::Top(y) => point.y >= y,
        ClipEdge::Bottom(y) => point.y <= y,
    }
}

fn clip_intersection(
    start: NativeStemPoint,
    stop: NativeStemPoint,
    edge: ClipEdge,
) -> NativeStemPoint {
    match edge {
        ClipEdge::Left(x) | ClipEdge::Right(x) => {
            let ratio = (x - start.x) / (stop.x - start.x);
            NativeStemPoint {
                x,
                y: start.y + ((stop.y - start.y) * ratio),
            }
        }
        ClipEdge::Top(y) | ClipEdge::Bottom(y) => {
            let ratio = (y - start.y) / (stop.y - start.y);
            NativeStemPoint {
                x: start.x + ((stop.x - start.x) * ratio),
                y,
            }
        }
    }
}

/// OpenJDK `Line2D.ptSegDistSq`, retaining its operation order.
fn point_segment_distance_sq(start: (f64, f64), stop: (f64, f64), point: NativeStemPoint) -> f64 {
    let mut x = point.x - start.0;
    let mut y = point.y - start.1;
    let px = stop.0 - start.0;
    let py = stop.1 - start.1;
    let mut dot_product = (x * px) + (y * py);
    let projection_sq = if dot_product <= 0.0 {
        0.0
    } else {
        x = px - x;
        y = py - y;
        dot_product = (x * px) + (y * py);
        if dot_product <= 0.0 {
            0.0
        } else {
            (dot_product * dot_product) / ((px * px) + (py * py))
        }
    };
    let distance_sq = ((x * x) + (y * y)) - projection_sq;
    if distance_sq < 0.0 { 0.0 } else { distance_sq }
}

/// Java `LineUtil.xAtY` delegates to its determinant intersection helper.
fn java_line_x_at_y(start: (f64, f64), stop: (f64, f64), y: f64) -> f64 {
    let (x1, y1) = start;
    let (x2, y2) = stop;
    let x3 = 0.0;
    let y3 = y;
    let x4 = 1_000.0;
    let y4 = y;
    let denominator = ((x1 - x2) * (y3 - y4)) - ((y1 - y2) * (x3 - x4));
    let value12 = (x1 * y2) - (y1 * x2);
    let value34 = (x3 * y4) - (y3 * x4);
    ((value12 * (x3 - x4)) - ((x1 - x2) * value34)) / denominator
}

fn java_round_to_i32(value: f64) -> i32 {
    ((value + 0.5).floor() as i64) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ribbon_clip_retains_positive_intersection_bounds() {
        let area = NativeStemsNoStemArea {
            ordinal: 0,
            source_bar_inter_id: 1,
            target_bar_inter_id: 2,
            kind: NativeStemsNoStemAreaKind::Middle,
            median: Segment {
                x1: 5.0,
                y1: 0.0,
                x2: 7.0,
                y2: 10.0,
            },
            thickness: 2.0,
            bounds: NativeStemRect {
                x: 4.0,
                y: 0.0,
                width: 4.0,
                height: 10.0,
            },
        };
        let clipped = clipped_ribbon_bounds(&area, JavaRectangle::new(5, 3, 2, 4))
            .expect("positive ribbon/rectangle intersection");
        assert_eq!(clipped.y, 3.0);
        assert_eq!(clipped.height, 4.0);
        assert!(clipped.width > 0.0);
    }

    #[test]
    fn point_segment_distance_matches_endpoint_and_projection_cases() {
        let line = ((0.0, 0.0), (0.0, 10.0));
        assert_eq!(
            point_segment_distance_sq(line.0, line.1, NativeStemPoint { x: 3.0, y: 5.0 }),
            9.0
        );
        assert_eq!(
            point_segment_distance_sq(line.0, line.1, NativeStemPoint { x: 3.0, y: 14.0 }),
            25.0
        );
    }

    #[test]
    fn java_round_differs_from_rust_negative_half_rounding() {
        assert_eq!(java_round_to_i32(-0.5), 0);
        assert_eq!(java_round_to_i32(-0.51), -1);
        assert_eq!(java_round_to_i32(0.5), 1);
    }
}
