// SPDX-License-Identifier: AGPL-3.0-or-later

//! `BeamsBuilder.createBeamInters`: from a beam structure to graded beams.
//!
//! This is the last step of per-spot recognition and the one that produces the
//! numbers `oracle/beams-chula.txt` records. On chula all 91 of the page's
//! final beams come from here -- `extendBeams` adds none -- so getting it right
//! is most of BEAMS' output, and the stages after it only grow the hooks.
//!
//! Two things about it are easy to get subtly wrong. The jitter impact is
//! computed **once per structure**, from the outermost lines, and shared by
//! every item in it; it is not a per-item measurement. And a single item can
//! produce both a hook and a beam -- Java tries the hook first, on different
//! width thresholds and with both belt sides always checked -- which is why the
//! oracle contains pairs sharing a median and disagreeing only in `wdth`.

use audiveris_image::beam_structure::{
    BeamBeltSides, BeamImpacts, BeamItem, BeamRaster, BeamStructureAnalysis, JitterSide,
    compute_jitter,
};
use audiveris_image::run_table::RunTable;

use crate::beam_parameters::{ItemParameters, SheetParameters};

/// Audiveris `Grades.intrinsicRatio`.
pub const INTRINSIC_RATIO: f64 = 0.8;

/// Audiveris `Grades.minInterGrade`, already scaled by the intrinsic ratio.
///
/// `AbstractInter.getMinGrade` returns this for beams and hooks alike; neither
/// overrides it.
pub const MIN_INTER_GRADE: f64 = INTRINSIC_RATIO * 0.1;

/// What a beam item was interpreted as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeamKind {
    Beam,
    Hook,
    /// Java's `SmallBeamInter`, for the small-beam size class.
    SmallBeam,
}

impl BeamKind {
    /// The Java class name, as the oracle records it.
    #[must_use]
    pub fn class_name(self) -> &'static str {
        match self {
            Self::Beam => "BeamInter",
            Self::Hook => "BeamHookInter",
            Self::SmallBeam => "SmallBeamInter",
        }
    }

    /// The Java shape name.
    #[must_use]
    pub fn shape(self) -> &'static str {
        match self {
            Self::Beam => "BEAM",
            Self::Hook => "BEAM_HOOK",
            Self::SmallBeam => "BEAM_SMALL",
        }
    }
}

/// Java `GradeUtil.clamp`: every impact is squeezed into `[0, 1]` on the way in.
///
/// `GradeImpacts.setImpact` applies this to each term, so a width impact of
/// 1.79 -- which happens whenever an item is wider than the *hook* thresholds
/// expect -- is stored as 1, not as itself. Skipping the clamp leaves the
/// grades plausible and wrong: on chula it moved 110 of 111 of them, always
/// upward, because a term above one inflates the geometric mean.
#[must_use]
pub fn clamp_impact(value: f64) -> f64 {
    // `f64::clamp` panics on a NaN bound and returns NaN for a NaN value, which
    // is Java's behaviour here too: its two comparisons both fail, so a NaN
    // falls through unchanged.
    value.clamp(0.0, 1.0)
}

/// Applies Java's clamp to all six terms, as `Impacts`' constructor does.
#[must_use]
pub fn clamped(impacts: BeamImpacts) -> BeamImpacts {
    BeamImpacts {
        width: clamp_impact(impacts.width),
        min_height: clamp_impact(impacts.min_height),
        max_height: clamp_impact(impacts.max_height),
        core: clamp_impact(impacts.core),
        belt: clamp_impact(impacts.belt),
        distance: clamp_impact(impacts.distance),
        raster: impacts.raster,
    }
}

/// One graded interpretation of one beam item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawBeam {
    pub kind: BeamKind,
    pub item: BeamItem,
    pub impacts: BeamImpacts,
    pub grade: f64,
}

/// `GradeImpacts.getGrade`: the weighted geometric mean, times the ratio.
///
/// A zero impact zeroes the product outright rather than contributing
/// `0^weight`, which is Java's own short-circuit and matters because `powf`
/// would otherwise be asked for `0.0_f64.powf(0.5)`.
#[must_use]
pub fn beam_grade(impacts: BeamImpacts) -> f64 {
    const WEIGHTS: [f64; 6] = [0.5, 1.0, 1.0, 2.0, 2.0, 2.0];
    let values = [
        impacts.width,
        impacts.min_height,
        impacts.max_height,
        impacts.core,
        impacts.belt,
        impacts.distance,
    ];
    let mut product = 1.0;
    let mut total_weight = 0.0;
    for (impact, weight) in values.into_iter().zip(WEIGHTS) {
        total_weight += weight;
        if impact == 0.0 {
            product = 0.0;
        } else if weight != 0.0 {
            product *= impact.powf(weight);
        }
    }
    INTRINSIC_RATIO * product.powf(1.0 / total_weight)
}

/// The jitter impact one structure contributes to all of its items.
///
/// `1 - meanJitter / maxJitterRatio`, where the mean is over the top border of
/// the first line and the bottom border of the last. Java catches any failure
/// here and abandons the whole structure, so this returns `None` rather than a
/// value that would quietly grade every item in it.
#[must_use]
pub fn distance_impact(
    structure: &BeamStructureAnalysis,
    glyph: &RunTable,
    offset_x: i32,
    offset_y: i32,
    item: &ItemParameters,
    sheet: &SheetParameters,
) -> Option<f64> {
    let first = structure.lines.first()?;
    let last = structure.lines.last()?;
    let top = compute_jitter(
        glyph,
        offset_x,
        offset_y,
        first.median,
        JitterSide::Top,
        item.corner_margin,
    );
    let bottom = compute_jitter(
        glyph,
        offset_x,
        offset_y,
        last.median,
        JitterSide::Bottom,
        item.corner_margin,
    );
    let mean = 0.5 * (top + bottom);
    Some(1.0 - (mean / sheet.max_jitter_ratio))
}

/// `createBeamInters`: grade every item of a structure, as beam and as hook.
///
/// Returns them in Java's own order -- line by line, item by item, hook before
/// beam -- because the exclusion Java inserts between a hook and the beam
/// sharing its item depends on both having been created.
#[must_use]
pub fn create_beam_inters(
    structure: &BeamStructureAnalysis,
    glyph: &RunTable,
    offset_x: i32,
    offset_y: i32,
    pixels: BeamRaster<'_>,
    item_parameters: &ItemParameters,
    sheet: &SheetParameters,
) -> Vec<RawBeam> {
    let Some(distance) =
        distance_impact(structure, glyph, offset_x, offset_y, item_parameters, sheet)
    else {
        return Vec::new();
    };

    let line_count = structure.lines.len();
    let mut beams = Vec::new();

    for (index, line) in structure.lines.iter().enumerate() {
        for item in &line.items {
            let width = item.median.x2 - item.median.x1;

            // Hooks first, and only for the standard size class. Java checks
            // both belt sides unconditionally here, unlike the beam below.
            if !item_parameters.is_small && width <= item_parameters.max_hook_width {
                if let Ok(impacts) = audiveris_image::beam_structure::compute_beam_impacts(
                    *item,
                    BeamBeltSides {
                        above: true,
                        below: true,
                    },
                    pixels,
                    distance,
                    item_parameters.hook_impacts(sheet),
                ) {
                    let impacts = clamped(impacts);
                    let grade = beam_grade(impacts);
                    if grade >= MIN_INTER_GRADE {
                        beams.push(RawBeam {
                            kind: BeamKind::Hook,
                            item: *item,
                            impacts,
                            grade,
                        });
                    }
                }
            }

            // The belt above is only checked for the topmost line and the belt
            // below only for the bottom one, so an inner beam of a stack is not
            // penalised for touching its neighbours. Java's own comment calls
            // this test not correct; it is reproduced as written.
            if let Ok(impacts) = audiveris_image::beam_structure::compute_beam_impacts(
                *item,
                BeamBeltSides {
                    above: index == 0,
                    below: index == line_count - 1,
                },
                pixels,
                distance,
                item_parameters.impacts(sheet),
            ) {
                let impacts = clamped(impacts);
                let grade = beam_grade(impacts);
                if grade >= MIN_INTER_GRADE {
                    beams.push(RawBeam {
                        kind: if item_parameters.is_small {
                            BeamKind::SmallBeam
                        } else {
                            BeamKind::Beam
                        },
                        item: *item,
                        impacts,
                        grade,
                    });
                }
            }
        }
    }

    beams
}

#[cfg(test)]
mod tests {
    use super::*;

    fn impacts(values: [f64; 6]) -> BeamImpacts {
        BeamImpacts {
            width: values[0],
            min_height: values[1],
            max_height: values[2],
            core: values[3],
            belt: values[4],
            distance: values[5],
            raster: audiveris_image::beam_structure::BeamRasterEvidence {
                core_foreground: 0,
                core_count: 0,
                belt_foreground: 0,
                belt_count: 0,
                core_ratio: 0.0,
                belt_ratio: 0.0,
                rounded_width: 0,
            },
        }
    }

    #[test]
    fn grade_is_the_weighted_geometric_mean_times_the_ratio() {
        // All ones: the mean is one and the grade is the ratio itself.
        assert!((beam_grade(impacts([1.0; 6])) - INTRINSIC_RATIO).abs() < 1e-15);

        // A value from the oracle, weights [0.5, 1, 1, 2, 2, 2] over 8.5.
        let grade = beam_grade(impacts([
            1.0,
            0.898_775_895,
            1.0,
            0.857_549_858,
            0.780_898_876,
            0.862_375_249,
        ]));
        assert!((grade - 0.694_274_858).abs() < 5e-10, "{grade}");
    }

    #[test]
    fn a_zero_impact_zeroes_the_grade() {
        // Java short-circuits rather than evaluating 0^0.5, and the difference
        // is visible: powf would give 0 here too, but only by luck of the
        // exponent being positive.
        assert_eq!(beam_grade(impacts([0.0, 1.0, 1.0, 1.0, 1.0, 1.0])), 0.0);
    }

    #[test]
    fn the_minimum_grade_carries_the_intrinsic_ratio() {
        // 0.8 * 0.1, not 0.1. Both beams and hooks use AbstractInter's default.
        assert!((MIN_INTER_GRADE - 0.08).abs() < 1e-15);
    }
}
