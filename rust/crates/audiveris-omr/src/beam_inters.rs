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
pub use audiveris_image::beam_structure::{beam_grade, clamp_impact, clamped};
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

/// One graded interpretation of one beam item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawBeam {
    pub kind: BeamKind,
    pub item: BeamItem,
    pub impacts: BeamImpacts,
    pub grade: f64,
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
                    // Java stores the clamped terms, not the raw ones, so the
                    // impacts a reader sees are the impacts that were graded.
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

/// `buildHooks`: a second pass over the spots that produced no beam.
///
/// Java runs this after `createBeams` and `extendBeams`, over `sortedBeamSpots`
/// with every spot that produced a beam removed -- so a spot `checkBeamGlyph`
/// refused is still a hook candidate, which is where 11 of chula's 31 hooks
/// come from.
///
/// Each beam is browsed above and below at 1.5 heights away, and a candidate
/// must clear a width floor, a mean-height band, and an overlap test against
/// every beam found so far before its impacts are computed. The base beam's own
/// jitter impact is reused rather than measured again.
#[must_use]
pub fn build_hooks(
    beams: &[RawBeam],
    spots: &[audiveris_image::beam_hooks::HookGlyph],
    pixels: BeamRaster<'_>,
    item_parameters: &ItemParameters,
    sheet: &SheetParameters,
) -> Vec<RawBeam> {
    use audiveris_image::beam_extension::{BeamExtensionClass, ExtensionBeam};
    use audiveris_image::beam_hooks::{
        HookParameters, HookSearchInput, HookSide, hook_search_evidence,
    };

    // Java browses `sig.inters(BeamInter.class)`, which excludes hooks: a hook
    // is never a base for another hook.
    let bases: Vec<ExtensionBeam> = beams
        .iter()
        .enumerate()
        .filter(|(_, raw)| raw.kind == BeamKind::Beam)
        .map(|(index, raw)| ExtensionBeam {
            id: index,
            median: raw.item.median,
            height: raw.item.height,
            distance_impact: raw.impacts.distance,
            class: BeamExtensionClass::Standard,
            glyph_id: None,
            removed: false,
        })
        .collect();

    // The overlap test runs against `rawSystemBeams`, which is every beam and
    // hook created so far -- including hooks added earlier in this very pass,
    // so the list grows as it goes.
    let mut raw_beams: Vec<ExtensionBeam> = beams
        .iter()
        .enumerate()
        .map(|(index, raw)| ExtensionBeam {
            id: index,
            median: raw.item.median,
            height: raw.item.height,
            distance_impact: raw.impacts.distance,
            class: BeamExtensionClass::Standard,
            glyph_id: None,
            removed: false,
        })
        .collect();

    let parameters = HookParameters {
        min_hook_width_low: item_parameters.min_hook_width_low,
        minimum_grade: MIN_INTER_GRADE,
        impacts: item_parameters.hook_impacts(sheet),
    };

    let mut hooks = Vec::new();
    let mut remaining: Vec<audiveris_image::beam_hooks::HookGlyph> = spots.to_vec();

    for base in &bases {
        for side in [HookSide::Top, HookSide::Bottom] {
            let evidence = hook_search_evidence(
                HookSearchInput {
                    base: *base,
                    spots: &remaining,
                    raw_beams: &raw_beams,
                    raster: pixels,
                    parameters,
                },
                side,
            );
            for found in evidence {
                let (Some(item), Some(impacts), Some(grade)) =
                    (found.item, found.impacts, found.grade)
                else {
                    continue;
                };
                if found.rejection.is_some() {
                    continue;
                }
                let impacts = clamped(impacts);
                hooks.push(RawBeam {
                    kind: BeamKind::Hook,
                    item,
                    impacts,
                    grade,
                });
                raw_beams.push(ExtensionBeam {
                    id: raw_beams.len(),
                    median: item.median,
                    height: item.height,
                    distance_impact: impacts.distance,
                    class: BeamExtensionClass::Standard,
                    glyph_id: None,
                    removed: false,
                });
                // A spot that became a hook is assigned and cannot become
                // another one.
                remaining.retain(|spot| spot.id != found.glyph_id);
            }
        }
    }

    hooks
}

/// Java `Area.getBounds()` for a beam: the integer box enclosing its
/// parallelogram.
///
/// `floor` on the near edges and `ceil` on the far ones, which is what
/// `Rectangle` does and not what rounding would do. The grouping lookup grows
/// and intersects these as integers, so being half a pixel out changes which
/// beams are considered neighbours at all.
#[must_use]
pub fn beam_bounds(item: BeamItem) -> audiveris_image::beam_groups::BeamBounds {
    let half = item.height / 2.0;
    let left = item.median.x1.floor();
    let right = item.median.x2.ceil();
    let top = (item.median.y1.min(item.median.y2) - half).floor();
    let bottom = (item.median.y1.max(item.median.y2) + half).ceil();
    audiveris_image::beam_groups::BeamBounds {
        x: left as i32,
        y: top as i32,
        width: (right - left) as i32,
        height: (bottom - top) as i32,
    }
}

/// `BeamGroupInter.populateSystem`: gather beams into groups.
///
/// The parameters are Java's own defaults, scaled: a horizontal overlap of 0.7
/// interlines, a vertical distance of 1.2, and a slope difference of 0.065 --
/// the last a bare ratio rather than a scaled length.
#[must_use]
pub fn group_beams(
    beams: &[RawBeam],
    interline: i32,
) -> audiveris_image::beam_groups::BeamGroupEvidence {
    use audiveris_image::beam_groups::{BeamGroupParameters, GroupingBeam, group_beam_evidence};

    let members: Vec<GroupingBeam> = beams
        .iter()
        .enumerate()
        .map(|(index, raw)| GroupingBeam {
            id: index,
            median: raw.item.median,
            height: raw.item.height,
            bounds: beam_bounds(raw.item),
        })
        .collect();

    group_beam_evidence(
        &members,
        BeamGroupParameters {
            min_x_overlap: f64::from(interline) * 0.7,
            max_y_distance: f64::from(interline) * 1.2,
            max_slope_diff: 0.065,
        },
    )
}

/// The two glyph pools `extendBeams` may extend a beam into.
///
/// Separate from the beams themselves because they come from different places:
/// spots are what `createBeams` left over, seeds are STEM_SEEDS' vertical
/// geometry.
#[derive(Clone, Copy, Debug)]
pub struct ExtensionSources<'a> {
    pub spots: &'a [audiveris_image::beam_extension::ExtensionGlyph],
    /// Empty explicitly disables Java `extendToStem` for compatibility callers.
    pub seeds: &'a [audiveris_image::beam_extension::ExtensionGlyph],
}

/// The scaled parameters every beam stage needs, gathered.
#[derive(Clone, Copy, Debug)]
pub struct BeamScaling<'a> {
    pub item_parameters: &'a ItemParameters,
    pub sheet: &'a SheetParameters,
    pub interline: i32,
}

/// `extendBeams`: merge, or lengthen towards a stem seed or a leftover spot.
///
/// Java runs this between `createBeams` and `buildHooks`, and it is the one
/// stage of BEAMS whose worth is a question about pages rather than about
/// source. Measured across the eight example sheets -- 30 systems, by comparing
/// the beam medians before and after -- it fires exactly **once**, merging two
/// beams into one on BachInvention5's sixth system. `extendToStem` and
/// `extendToSpot` never fire at all.
///
/// Passing no seeds disables exactly `extendToStem`, which the compatibility
/// BEAMS entry point retains. The composed production entry point adapts
/// STEM_SEEDS' accepted free glyphs into this source in per-system order.
///
/// Returns the surviving beams, with merged pairs replaced by their merger.
#[must_use]
pub fn extend_beams(
    beams: &[RawBeam],
    sources: ExtensionSources<'_>,
    pixels: BeamRaster<'_>,
    scaling: &BeamScaling<'_>,
    mut in_system: impl FnMut((f64, f64), f64) -> bool,
) -> Vec<RawBeam> {
    let ExtensionSources { spots, seeds } = sources;
    let &BeamScaling {
        item_parameters,
        sheet,
        interline,
    } = scaling;
    use audiveris_image::beam_extension::{
        BeamExtensionClass, BeamExtensionInput, BeamExtensionMode, BeamExtensionParameters,
        ExtensionBeam, ExtensionClassParameters, beam_extension_evidence,
    };

    let members: Vec<ExtensionBeam> = beams
        .iter()
        .enumerate()
        .map(|(index, raw)| ExtensionBeam {
            id: index,
            median: raw.item.median,
            height: raw.item.height,
            distance_impact: raw.impacts.distance,
            class: BeamExtensionClass::Standard,
            glyph_id: None,
            removed: false,
        })
        .collect();

    let parameters = BeamExtensionParameters {
        standard: ExtensionClassParameters {
            impacts: item_parameters.impacts(sheet),
            minimum_grade: MIN_INTER_GRADE,
        },
        small: None,
        max_side_beam_dx: f64::from(sheet.max_side_beam_dx),
        min_beams_gap_x: f64::from(sheet.min_beams_gap_x),
        max_beams_gap_y: f64::from(sheet.max_beams_gap_y),
        beams_x_margin: f64::from(sheet.beams_x_margin),
        max_extension_to_stem: f64::from(sheet.max_extension_to_stem),
        max_extension_to_spot: f64::from(sheet.max_extension_to_spot),
        max_stem_beam_gap_x: f64::from(sheet.max_stem_beam_gap_x),
        max_stem_beam_gap_y: f64::from(sheet.max_stem_beam_gap_y),
        min_extension_black_ratio: sheet.min_ext_black_ratio,
        min_neighbor_x_overlap: f64::from(interline) * 0.7,
        max_neighbor_y_distance: f64::from(interline) * 1.2,
        max_neighbor_slope_diff: 0.065,
    };

    let evidence = beam_extension_evidence(
        BeamExtensionInput {
            beams: &members,
            seeds,
            spots,
            raster: pixels,
            parameters,
        },
        &mut in_system,
    );

    let mut survivors: Vec<RawBeam> = beams.to_vec();
    let mut removed = vec![false; beams.len()];
    for found in &evidence {
        if found.rejection.is_some() {
            continue;
        }
        let (Some(item), Some(impacts), Some(grade)) =
            (found.resulting_item, found.impacts, found.grade)
        else {
            continue;
        };
        removed[found.beam_id] = true;
        if let BeamExtensionMode::Merge { other_beam_id } = found.mode {
            removed[other_beam_id] = true;
        }
        survivors.push(RawBeam {
            kind: BeamKind::Beam,
            item,
            impacts: clamped(impacts),
            grade,
        });
    }

    survivors
        .into_iter()
        .enumerate()
        .filter(|(index, _)| *index >= removed.len() || !removed[*index])
        .map(|(_, raw)| raw)
        .collect()
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
