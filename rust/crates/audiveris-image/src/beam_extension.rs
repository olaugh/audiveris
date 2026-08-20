// SPDX-License-Identifier: AGPL-3.0-or-later

//! Geometry/evidence kernel for Java `BeamsBuilder.extendBeams`.
//!
//! The caller retains identity allocation and SIG mutation. This module emits
//! the first successful mode, or the ordered rejected attempts, for every
//! beam/side pair using the Java order: side beam/merge, stem, spot, parallel.

use crate::beam_structure::{
    BeamBeltSides, BeamImpactParameters, BeamImpacts, BeamItem, BeamRaster, Segment, beam_grade,
    compute_beam_impacts, sample_beam_core,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtensionSide {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeamExtensionMode {
    Merge { other_beam_id: usize },
    Stem { seed_id: usize },
    Spot { spot_id: usize },
    Parallel { other_beam_id: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeamExtensionRejection {
    MiddleCore,
    NoConcreteExtension,
    ExtensionCore,
    Impacts,
    Grade,
    NotNeighbor,
    IncrementTooSmall,
    OutsideSystem,
    IncludedBeam,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamExtensionEvidence {
    pub beam_id: usize,
    pub side: ExtensionSide,
    pub mode: BeamExtensionMode,
    pub target: (f64, f64),
    pub extension_dx: f64,
    pub core_foreground: usize,
    pub core_count: usize,
    pub core_ratio: f64,
    pub resulting_item: Option<BeamItem>,
    pub impacts: Option<BeamImpacts>,
    pub grade: Option<f64>,
    pub rejection: Option<BeamExtensionRejection>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeamExtensionClass {
    Standard,
    Small,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExtensionBeam {
    pub id: usize,
    pub median: Segment,
    pub height: f64,
    pub distance_impact: f64,
    pub class: BeamExtensionClass,
    pub glyph_id: Option<usize>,
    pub removed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExtensionGlyph {
    pub id: usize,
    pub left: i32,
    pub top: i32,
    pub width: usize,
    pub height: usize,
    /// Exact vertical seed median. Ignored for spots.
    pub vertical_median: Option<Segment>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExtensionClassParameters {
    pub impacts: BeamImpactParameters,
    pub minimum_grade: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamExtensionParameters {
    pub standard: ExtensionClassParameters,
    pub small: Option<ExtensionClassParameters>,
    pub max_side_beam_dx: f64,
    pub min_beams_gap_x: f64,
    pub max_beams_gap_y: f64,
    pub beams_x_margin: f64,
    pub max_extension_to_stem: f64,
    pub max_extension_to_spot: f64,
    pub max_stem_beam_gap_x: f64,
    pub max_stem_beam_gap_y: f64,
    pub min_extension_black_ratio: f64,
    pub min_neighbor_x_overlap: f64,
    pub max_neighbor_y_distance: f64,
    pub max_neighbor_slope_diff: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct BeamExtensionInput<'a> {
    pub beams: &'a [ExtensionBeam],
    pub seeds: &'a [ExtensionGlyph],
    pub spots: &'a [ExtensionGlyph],
    pub raster: BeamRaster<'a>,
    pub parameters: BeamExtensionParameters,
}

/// Emit native extension evidence in Java beam order and LEFT/RIGHT order.
#[must_use]
pub fn beam_extension_evidence(
    input: BeamExtensionInput<'_>,
    mut side_in_system: impl FnMut((f64, f64), f64) -> bool,
) -> Vec<BeamExtensionEvidence> {
    let mut evidence = Vec::new();
    for beam in input.beams.iter().copied().filter(|beam| !beam.removed) {
        for side in [ExtensionSide::Left, ExtensionSide::Right] {
            let side_beam = get_side_beam(beam, side, None, input.beams, input.parameters);
            let mut max_dx = None;
            if let Some(other) = side_beam {
                if side == ExtensionSide::Left {
                    let attempt = merge_attempt(beam, other, side, input);
                    let accepted = attempt.rejection.is_none();
                    evidence.push(attempt);
                    if accepted {
                        break;
                    }
                }
                let gap = match side {
                    ExtensionSide::Left => beam.median.x1 - other.median.x2,
                    ExtensionSide::Right => other.median.x1 - beam.median.x2,
                };
                max_dx = Some(
                    (gap - input.parameters.beams_x_margin)
                        .max(0.0)
                        .round_ties_even(),
                );
            }

            if let Some(attempt) = glyph_attempt(beam, side, max_dx, input.seeds, true, input) {
                let accepted = attempt.rejection.is_none();
                evidence.push(attempt);
                if accepted {
                    break;
                }
            }
            if let Some(attempt) = glyph_attempt(beam, side, max_dx, input.spots, false, input) {
                let accepted = attempt.rejection.is_none();
                evidence.push(attempt);
                if accepted {
                    break;
                }
            }
            if side_beam.is_none() {
                let attempts = parallel_attempts(beam, side, input, &mut side_in_system);
                let accepted = attempts
                    .last()
                    .is_some_and(|attempt| attempt.rejection.is_none());
                evidence.extend(attempts);
                if accepted {
                    break;
                }
            }
        }
    }
    evidence
}

fn merge_attempt(
    one: ExtensionBeam,
    two: ExtensionBeam,
    side: ExtensionSide,
    input: BeamExtensionInput<'_>,
) -> BeamExtensionEvidence {
    let (left, right) = if one.median.x1 < two.median.x1 {
        (one, two)
    } else {
        (two, one)
    };
    let gap = right.median.x1 - left.median.x2;
    let height = weighted_height(one, two);
    let middle = BeamItem {
        median: Segment {
            x1: left.median.x2,
            y1: left.median.y2,
            x2: right.median.x1,
            y2: right.median.y1,
        },
        height,
    };
    let (foreground, count) = if gap >= input.parameters.min_beams_gap_x {
        sample_beam_core(middle, input.raster)
    } else {
        (0, 0)
    };
    let ratio = if count == 0 {
        f64::NAN
    } else {
        foreground as f64 / count as f64
    };
    let merged = BeamItem {
        median: Segment {
            x1: left.median.x1,
            y1: left.median.y1,
            x2: right.median.x2,
            y2: right.median.y2,
        },
        height,
    };
    let mode = BeamExtensionMode::Merge {
        other_beam_id: two.id,
    };
    if gap >= input.parameters.min_beams_gap_x && ratio < input.parameters.min_extension_black_ratio
    {
        return rejected(
            one.id,
            side,
            mode,
            (merged.median.x2, merged.median.y2),
            gap,
            foreground,
            count,
            ratio,
            BeamExtensionRejection::MiddleCore,
        );
    }
    evaluate_item(
        one,
        side,
        mode,
        merged,
        CoreEvidence {
            foreground,
            count,
            ratio,
        },
        input,
    )
}

fn glyph_attempt(
    beam: ExtensionBeam,
    side: ExtensionSide,
    maximum_dx: Option<f64>,
    glyphs: &[ExtensionGlyph],
    stems: bool,
    input: BeamExtensionInput<'_>,
) -> Option<BeamExtensionEvidence> {
    let limit = maximum_dx.unwrap_or(f64::INFINITY).min(if stems {
        input.parameters.max_extension_to_stem
    } else {
        input.parameters.max_extension_to_spot
    });
    let vertical_margin = if stems {
        input.parameters.max_stem_beam_gap_y
    } else {
        0.0
    };
    let area = side_item(beam, side, vertical_margin, limit, 0.0);
    let mut candidates = glyphs
        .iter()
        .copied()
        .filter(|glyph| beam.glyph_id != Some(glyph.id) && item_intersects_rect(area, *glyph))
        .collect::<Vec<_>>();
    candidates.sort_by_key(|glyph| glyph.left);
    let glyph = match side {
        ExtensionSide::Left => candidates.last().copied(),
        ExtensionSide::Right => candidates.first().copied(),
    }?;
    let target_x = if stems {
        line_intersection(beam.median, glyph.vertical_median?)?.0
    } else if side == ExtensionSide::Left {
        f64::from(glyph.left)
    } else {
        f64::from(glyph.left) + glyph.width as f64 - 1.0
    };
    let target = (target_x, beam.median.y_at_x(target_x));
    Some(extension_to_point(
        beam,
        side,
        if stems {
            BeamExtensionMode::Stem { seed_id: glyph.id }
        } else {
            BeamExtensionMode::Spot { spot_id: glyph.id }
        },
        target,
        input,
    ))
}

fn parallel_attempts(
    beam: ExtensionBeam,
    side: ExtensionSide,
    input: BeamExtensionInput<'_>,
    side_in_system: &mut impl FnMut((f64, f64), f64) -> bool,
) -> Vec<BeamExtensionEvidence> {
    let mut attempts = Vec::new();
    let lookup = BeamItem {
        median: beam.median,
        height: 2.0 * input.parameters.max_neighbor_y_distance,
    };
    for other in input.beams.iter().copied() {
        if other.id == beam.id
            || other.removed
            || other.glyph_id == beam.glyph_id
            || !items_intersect(
                lookup,
                BeamItem {
                    median: other.median,
                    height: other.height,
                },
            )
        {
            continue;
        }
        let mode = BeamExtensionMode::Parallel {
            other_beam_id: other.id,
        };
        if !can_be_neighbors(beam, other, input.parameters) {
            attempts.push(rejected(
                beam.id,
                side,
                mode,
                endpoint(beam, side),
                0.0,
                0,
                0,
                f64::NAN,
                BeamExtensionRejection::NotNeighbor,
            ));
            continue;
        }
        let end = endpoint(beam, side);
        let other_end = endpoint(other, side);
        let extension_dx = match side {
            ExtensionSide::Left => end.0 - other_end.0,
            ExtensionSide::Right => other_end.0 - end.0,
        };
        if extension_dx < 2.0 * input.parameters.max_stem_beam_gap_x {
            attempts.push(rejected(
                beam.id,
                side,
                mode,
                other_end,
                extension_dx,
                0,
                0,
                f64::NAN,
                BeamExtensionRejection::IncrementTooSmall,
            ));
            continue;
        }
        let target = (other_end.0, beam.median.y_at_x(other_end.0));
        if !side_in_system(target, beam.height) {
            attempts.push(rejected(
                beam.id,
                side,
                mode,
                target,
                extension_dx,
                0,
                0,
                f64::NAN,
                BeamExtensionRejection::OutsideSystem,
            ));
            continue;
        }
        if get_side_beam(
            beam,
            side,
            Some(extension_dx),
            input.beams,
            input.parameters,
        )
        .is_some()
        {
            attempts.push(rejected(
                beam.id,
                side,
                mode,
                target,
                extension_dx,
                0,
                0,
                f64::NAN,
                BeamExtensionRejection::IncludedBeam,
            ));
            continue;
        }
        attempts.push(extension_to_point(beam, side, mode, target, input));
        break;
    }
    attempts
}

fn extension_to_point(
    beam: ExtensionBeam,
    side: ExtensionSide,
    mode: BeamExtensionMode,
    target: (f64, f64),
    input: BeamExtensionInput<'_>,
) -> BeamExtensionEvidence {
    let end = endpoint(beam, side);
    let extension_dx = match side {
        ExtensionSide::Left => end.0 - target.0,
        ExtensionSide::Right => target.0 - end.0,
    };
    if extension_dx <= 1.0 {
        return rejected(
            beam.id,
            side,
            mode,
            target,
            extension_dx,
            0,
            0,
            f64::NAN,
            BeamExtensionRejection::NoConcreteExtension,
        );
    }
    let extension = side_item(beam, side, 0.0, extension_dx, 0.0);
    let (foreground, count) = sample_beam_core(extension, input.raster);
    let ratio = foreground as f64 / count as f64;
    if ratio < input.parameters.min_extension_black_ratio {
        return rejected(
            beam.id,
            side,
            mode,
            target,
            extension_dx,
            foreground,
            count,
            ratio,
            BeamExtensionRejection::ExtensionCore,
        );
    }
    let item = BeamItem {
        median: match side {
            ExtensionSide::Left => Segment {
                x1: target.0,
                y1: target.1,
                x2: beam.median.x2,
                y2: beam.median.y2,
            },
            ExtensionSide::Right => Segment {
                x1: beam.median.x1,
                y1: beam.median.y1,
                x2: target.0,
                y2: target.1,
            },
        },
        height: beam.height,
    };
    evaluate_item(
        beam,
        side,
        mode,
        item,
        CoreEvidence {
            foreground,
            count,
            ratio,
        },
        input,
    )
}

#[derive(Clone, Copy)]
struct CoreEvidence {
    foreground: usize,
    count: usize,
    ratio: f64,
}

fn evaluate_item(
    beam: ExtensionBeam,
    side: ExtensionSide,
    mode: BeamExtensionMode,
    item: BeamItem,
    core: CoreEvidence,
    input: BeamExtensionInput<'_>,
) -> BeamExtensionEvidence {
    let class = class_parameters(beam, input.parameters);
    // A merge inherits the *mean* of the two beams' jitter impacts, where a
    // one-sided extension inherits its single beam's. Java's `mergeOf` averages
    // them, and using only the base beam's leaves the merged beam graded on
    // half the evidence -- on BachInvention5 that moved `jit` from 0.7349 to
    // 0.7431 and the grade with it.
    let distance_impact = match mode {
        BeamExtensionMode::Merge { other_beam_id }
        | BeamExtensionMode::Parallel { other_beam_id } => input
            .beams
            .iter()
            .find(|other| other.id == other_beam_id)
            .map_or(beam.distance_impact, |other| {
                (beam.distance_impact + other.distance_impact) / 2.0
            }),
        _ => beam.distance_impact,
    };
    let impacts = compute_beam_impacts(
        item,
        BeamBeltSides {
            above: true,
            below: true,
                    neutral: false,
        },
        input.raster,
        distance_impact,
        class.impacts,
    )
    .ok();
    let grade = impacts.map(beam_grade);
    let rejection = if impacts.is_none() {
        Some(BeamExtensionRejection::Impacts)
    } else if grade.is_none_or(|grade| grade < class.minimum_grade) {
        Some(BeamExtensionRejection::Grade)
    } else {
        None
    };
    BeamExtensionEvidence {
        beam_id: beam.id,
        side,
        mode,
        target: endpoint_item(item, side),
        extension_dx: (item.median.x2 - item.median.x1) - (beam.median.x2 - beam.median.x1),
        core_foreground: core.foreground,
        core_count: core.count,
        core_ratio: core.ratio,
        resulting_item: Some(item),
        impacts,
        grade,
        rejection,
    }
}

fn get_side_beam(
    beam: ExtensionBeam,
    side: ExtensionSide,
    max_gap: Option<f64>,
    beams: &[ExtensionBeam],
    parameters: BeamExtensionParameters,
) -> Option<ExtensionBeam> {
    let area = side_item(
        beam,
        side,
        0.0,
        max_gap.unwrap_or(parameters.max_side_beam_dx),
        0.0,
    );
    let endpoint = endpoint(beam, side);
    let mut candidates = beams
        .iter()
        .copied()
        .filter(|other| {
            other.id != beam.id
                && !other.removed
                && items_intersect(
                    area,
                    BeamItem {
                        median: other.median,
                        height: other.height,
                    },
                )
                && (other.median.slope() - beam.median.slope()).abs()
                    <= parameters.max_neighbor_slope_diff
                && point_line_distance(endpoint, other.median) <= parameters.max_beams_gap_y
        })
        .collect::<Vec<_>>();
    if candidates.len() > 1 {
        candidates
            .sort_by(|one, two| side_gap(beam, *one, side).total_cmp(&side_gap(beam, *two, side)));
    }
    candidates.first().copied()
}

fn side_item(
    beam: ExtensionBeam,
    side: ExtensionSide,
    ext_dy: f64,
    ext_dx: f64,
    int_dx: f64,
) -> BeamItem {
    let inner_x = match side {
        ExtensionSide::Left => beam.median.x1 - 1.0 + int_dx,
        ExtensionSide::Right => beam.median.x2 + 1.0 - int_dx,
    };
    let outer_x = match side {
        ExtensionSide::Left => beam.median.x1 - ext_dx,
        ExtensionSide::Right => beam.median.x2 + ext_dx,
    };
    let (left, right) = match side {
        ExtensionSide::Left => (outer_x, inner_x),
        ExtensionSide::Right => (inner_x, outer_x),
    };
    BeamItem {
        median: Segment {
            x1: left,
            y1: beam.median.y_at_x(left),
            x2: right,
            y2: beam.median.y_at_x(right),
        },
        height: beam.height + 2.0 * ext_dy,
    }
}

fn class_parameters(
    beam: ExtensionBeam,
    parameters: BeamExtensionParameters,
) -> ExtensionClassParameters {
    match beam.class {
        BeamExtensionClass::Standard => parameters.standard,
        BeamExtensionClass::Small => parameters.small.unwrap_or(parameters.standard),
    }
}
fn endpoint(beam: ExtensionBeam, side: ExtensionSide) -> (f64, f64) {
    match side {
        ExtensionSide::Left => (beam.median.x1, beam.median.y1),
        ExtensionSide::Right => (beam.median.x2, beam.median.y2),
    }
}
fn endpoint_item(item: BeamItem, side: ExtensionSide) -> (f64, f64) {
    match side {
        ExtensionSide::Left => (item.median.x1, item.median.y1),
        ExtensionSide::Right => (item.median.x2, item.median.y2),
    }
}
fn side_gap(beam: ExtensionBeam, other: ExtensionBeam, side: ExtensionSide) -> f64 {
    match side {
        ExtensionSide::Left => beam.median.x1 - other.median.x2,
        ExtensionSide::Right => other.median.x1 - beam.median.x2,
    }
}
fn weighted_height(one: ExtensionBeam, two: ExtensionBeam) -> f64 {
    let w1 = one.median.x2 - one.median.x1;
    let w2 = two.median.x2 - two.median.x1;
    ((one.height * w1) + (two.height * w2)) / (w1 + w2)
}
fn point_line_distance(point: (f64, f64), line: Segment) -> f64 {
    ((line.y2 - line.y1) * point.0 - (line.x2 - line.x1) * point.1 + line.x2 * line.y1
        - line.y2 * line.x1)
        .abs()
        / ((line.y2 - line.y1).hypot(line.x2 - line.x1))
}
fn line_intersection(one: Segment, two: Segment) -> Option<(f64, f64)> {
    // Keep LineUtil.intersection's determinant form and operation order. The
    // algebraically equivalent parametric form differs by one ULP on the D039
    // natural stem-extension case, and this point becomes persisted beam
    // geometry as well as an input to the impact masks.
    let denominator =
        ((one.x1 - one.x2) * (two.y1 - two.y2)) - ((one.y1 - one.y2) * (two.x1 - two.x2));
    if denominator == 0.0 {
        return None;
    }
    let one_determinant = (one.x1 * one.y2) - (one.y1 * one.x2);
    let two_determinant = (two.x1 * two.y2) - (two.y1 * two.x2);
    let x = ((one_determinant * (two.x1 - two.x2)) - ((one.x1 - one.x2) * two_determinant))
        / denominator;
    let y = ((one_determinant * (two.y1 - two.y2)) - ((one.y1 - one.y2) * two_determinant))
        / denominator;
    Some((x, y))
}
fn can_be_neighbors(one: ExtensionBeam, two: ExtensionBeam, p: BeamExtensionParameters) -> bool {
    let left = one.median.x1.max(two.median.x1);
    let right = one.median.x2.min(two.median.x2);
    if right - left < p.min_neighbor_x_overlap {
        return false;
    }
    let x = (left + right) / 2.0;
    (one.median.y_at_x(x) - two.median.y_at_x(x)).abs() <= p.max_neighbor_y_distance
        && (one.median.slope() - two.median.slope()).abs() <= p.max_neighbor_slope_diff
}

fn item_intersects_rect(item: BeamItem, rect: ExtensionGlyph) -> bool {
    let left = f64::from(rect.left);
    let right = left + rect.width as f64;
    let top = f64::from(rect.top);
    let bottom = top + rect.height as f64;
    convex_polygons_intersect(
        &item_polygon(item),
        &[(left, top), (right, top), (right, bottom), (left, bottom)],
    )
}
fn items_intersect(one: BeamItem, two: BeamItem) -> bool {
    convex_polygons_intersect(&item_polygon(one), &item_polygon(two))
}

fn item_polygon(item: BeamItem) -> [(f64, f64); 4] {
    let half = item.height / 2.0;
    [
        (item.median.x1, item.median.y1 - half),
        (item.median.x2, item.median.y2 - half),
        (item.median.x2, item.median.y2 + half),
        (item.median.x1, item.median.y1 + half),
    ]
}

fn convex_polygons_intersect(one: &[(f64, f64)], two: &[(f64, f64)]) -> bool {
    one.iter()
        .zip(one.iter().cycle().skip(1))
        .chain(two.iter().zip(two.iter().cycle().skip(1)))
        .all(|(start, stop)| {
            let axis = (-(stop.1 - start.1), stop.0 - start.0);
            let project = |polygon: &[(f64, f64)]| {
                polygon
                    .iter()
                    .map(|point| (point.0 * axis.0) + (point.1 * axis.1))
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |range, value| {
                        (range.0.min(value), range.1.max(value))
                    })
            };
            let one_range = project(one);
            let two_range = project(two);
            one_range.0 <= two_range.1 && two_range.0 <= one_range.1
        })
}

#[allow(clippy::too_many_arguments)]
fn rejected(
    beam_id: usize,
    side: ExtensionSide,
    mode: BeamExtensionMode,
    target: (f64, f64),
    extension_dx: f64,
    foreground: usize,
    count: usize,
    ratio: f64,
    rejection: BeamExtensionRejection,
) -> BeamExtensionEvidence {
    BeamExtensionEvidence {
        beam_id,
        side,
        mode,
        target,
        extension_dx,
        core_foreground: foreground,
        core_count: count,
        core_ratio: ratio,
        resulting_item: None,
        impacts: None,
        grade: None,
        rejection: Some(rejection),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_table::{FOREGROUND, Orientation, RunTable};

    fn beam(id: usize, x1: f64, x2: f64, y: f64) -> ExtensionBeam {
        ExtensionBeam {
            id,
            median: Segment {
                x1,
                y1: y,
                x2,
                y2: y,
            },
            height: 4.0,
            distance_impact: 1.0,
            class: BeamExtensionClass::Standard,
            glyph_id: Some(id),
            removed: false,
        }
    }
    fn parameters() -> BeamExtensionParameters {
        let class = ExtensionClassParameters {
            impacts: BeamImpactParameters {
                belt_margin_dx: 1,
                belt_margin_dy: 1,
                min_core_black_ratio: 0.5,
                max_belt_black_ratio: 2.0,
                min_width_low: 3.0,
                min_width_high: 8.0,
                min_height_low: 2.0,
                typical_height: 4.0,
                max_height_high: 6.0,
            },
            minimum_grade: 0.08,
        };
        BeamExtensionParameters {
            standard: class,
            small: None,
            max_side_beam_dx: 12.0,
            min_beams_gap_x: 2.0,
            max_beams_gap_y: 2.0,
            beams_x_margin: 1.0,
            max_extension_to_stem: 8.0,
            max_extension_to_spot: 10.0,
            max_stem_beam_gap_x: 2.0,
            max_stem_beam_gap_y: 2.0,
            min_extension_black_ratio: 0.5,
            min_neighbor_x_overlap: 2.0,
            max_neighbor_y_distance: 8.0,
            max_neighbor_slope_diff: 0.1,
        }
    }
    fn raster() -> RunTable {
        RunTable::from_pixels(Orientation::Horizontal, 40, 20, &[FOREGROUND; 800]).unwrap()
    }

    #[test]
    fn left_merge_precedes_other_modes_and_uses_middle_ratio() {
        let table = raster();
        let beams = [beam(1, 10.0, 15.0, 8.0), beam(2, 3.0, 8.0, 8.0)];
        let evidence = beam_extension_evidence(
            BeamExtensionInput {
                beams: &beams,
                seeds: &[],
                spots: &[],
                raster: BeamRaster {
                    table: &table,
                    offset_x: 0,
                    offset_y: 0,
                },
                parameters: parameters(),
            },
            |_, _| true,
        );
        assert_eq!(
            evidence[0].mode,
            BeamExtensionMode::Merge { other_beam_id: 2 }
        );
        assert!(evidence[0].rejection.is_none());
    }

    #[test]
    fn stem_is_nearest_by_abscissa_and_precedes_spot() {
        let table = raster();
        let beams = [beam(1, 10.0, 15.0, 8.0)];
        let seeds = [ExtensionGlyph {
            id: 7,
            left: 17,
            top: 4,
            width: 1,
            height: 9,
            vertical_median: Some(Segment {
                x1: 17.0,
                y1: 4.0,
                x2: 17.0,
                y2: 12.0,
            }),
        }];
        let spots = [ExtensionGlyph {
            id: 8,
            left: 16,
            top: 7,
            width: 3,
            height: 4,
            vertical_median: None,
        }];
        let evidence = beam_extension_evidence(
            BeamExtensionInput {
                beams: &beams,
                seeds: &seeds,
                spots: &spots,
                raster: BeamRaster {
                    table: &table,
                    offset_x: 0,
                    offset_y: 0,
                },
                parameters: parameters(),
            },
            |_, _| true,
        );
        assert_eq!(evidence[0].mode, BeamExtensionMode::Stem { seed_id: 7 });
        assert!(evidence[0].rejection.is_none());
    }

    #[test]
    fn concrete_extension_requires_more_than_one_pixel() {
        let table = raster();
        let beams = [beam(1, 10.0, 15.0, 8.0)];
        let spots = [ExtensionGlyph {
            id: 8,
            left: 15,
            top: 7,
            width: 1,
            height: 4,
            vertical_median: None,
        }];
        let evidence = beam_extension_evidence(
            BeamExtensionInput {
                beams: &beams,
                seeds: &[],
                spots: &spots,
                raster: BeamRaster {
                    table: &table,
                    offset_x: 0,
                    offset_y: 0,
                },
                parameters: parameters(),
            },
            |_, _| true,
        );
        assert_eq!(
            evidence[0].rejection,
            Some(BeamExtensionRejection::NoConcreteExtension)
        );
    }
}
