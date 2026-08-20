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
use audiveris_image::{
    beam_groups::BeamBounds,
    run_table::{BACKGROUND, FOREGROUND, Orientation, RunTable, RunTableError},
};

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

/// Exact fixed glyph Java's `BeamsBuilder.registerBeam` attaches to a beam.
///
/// This is not the threshold-140 spot which originally led to an
/// interpretation. Java rebuilds a vertical run table from `NO_STAFF` inside
/// the final beam parallelogram every time it registers an inter, including
/// for hooks and extension/merge products. Retaining that table therefore
/// avoids inventing a source identity for geometry which spans more than one
/// spot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisteredBeamGlyph {
    /// `beam.getBounds().intersection(sheetBox)`, in sheet coordinates.
    pub bounds: BeamBounds,
    /// Cropped vertical table registered with Java's `GlyphIndex`.
    pub run_table: RunTable,
}

impl RegisteredBeamGlyph {
    #[must_use]
    pub fn weight(&self) -> usize {
        self.run_table.weight()
    }

    /// FNV-1a-64 over the oracle's canonical orientation/dimensions/run rows.
    #[must_use]
    pub fn run_digest(&self) -> u64 {
        run_table_digest(&self.run_table)
    }
}

/// Port `BeamsBuilder.retrieveGlyph` for one already-graded beam.
///
/// `binary` is the row-major `Picture.SourceKey.NO_STAFF` raster. Pixels are
/// kept only when they are foreground and the Java2D beam parallelogram
/// contains their integer coordinate; the result is cropped to the beam's
/// sheet-clipped bounds and encoded vertically.
pub fn retrieve_beam_glyph(
    item: BeamItem,
    raster_width: usize,
    raster_height: usize,
    binary: &[u8],
) -> Result<RegisteredBeamGlyph, RunTableError> {
    let expected = raster_width
        .checked_mul(raster_height)
        .ok_or(RunTableError::InvalidDimensions)?;
    if binary.len() != expected {
        return Err(RunTableError::InvalidPixels);
    }
    let sheet_width = i32::try_from(raster_width).map_err(|_| RunTableError::InvalidDimensions)?;
    let sheet_height =
        i32::try_from(raster_height).map_err(|_| RunTableError::InvalidDimensions)?;
    let raw = beam_bounds(item);
    let left = raw.x.clamp(0, sheet_width);
    let top = raw.y.clamp(0, sheet_height);
    let right = i64::from(raw.x)
        .saturating_add(i64::from(raw.width))
        .min(i64::from(sheet_width))
        .max(i64::from(left));
    let bottom = i64::from(raw.y)
        .saturating_add(i64::from(raw.height))
        .min(i64::from(sheet_height))
        .max(i64::from(top));
    let width =
        usize::try_from(right - i64::from(left)).map_err(|_| RunTableError::InvalidDimensions)?;
    let height =
        usize::try_from(bottom - i64::from(top)).map_err(|_| RunTableError::InvalidDimensions)?;
    if width == 0 || height == 0 {
        return Err(RunTableError::InvalidDimensions);
    }

    let mut cropped = vec![BACKGROUND; width * height];
    for dy in 0..height {
        let y = top + i32::try_from(dy).map_err(|_| RunTableError::InvalidDimensions)?;
        for dx in 0..width {
            let x = left + i32::try_from(dx).map_err(|_| RunTableError::InvalidDimensions)?;
            let source = usize::try_from(y).expect("clipped beam ordinate") * raster_width
                + usize::try_from(x).expect("clipped beam abscissa");
            if binary[source] == FOREGROUND
                && beam_parallelogram_contains(item, f64::from(x), f64::from(y))
            {
                cropped[dy * width + dx] = FOREGROUND;
            }
        }
    }
    let run_table = RunTable::from_pixels(Orientation::Vertical, width, height, &cropped)?;
    Ok(RegisteredBeamGlyph {
        bounds: BeamBounds {
            x: left,
            y: top,
            width: i32::try_from(width).map_err(|_| RunTableError::InvalidDimensions)?,
            height: i32::try_from(height).map_err(|_| RunTableError::InvalidDimensions)?,
        },
        run_table,
    })
}

fn beam_parallelogram_contains(item: BeamItem, x: f64, y: f64) -> bool {
    let half = item.height / 2.0;
    let vertices = [
        (item.median.x1, item.median.y1 - half),
        (item.median.x2, item.median.y2 - half),
        (item.median.x2, item.median.y2 + half),
        (item.median.x1, item.median.y1 + half),
    ];
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

    // `AbstractBeamInter.contains` reaches `Area.contains`, not
    // `LineUtil.yAtX`.  OpenJDK first applies its cached Rectangle2D bounds,
    // which are half-open on the far edges, then sums the y-monotone Curve
    // crossings and tests their parity.  Keep the operation order of
    // `Curve.crossingsFor` and `Order1.XforY`: the algebraically equivalent
    // determinant y-at-x form differs by a last bit on boundary ink.
    if !(x >= min_x && x < max_x && y >= min_y && y < max_y) {
        return false;
    }
    let crossings = vertices
        .iter()
        .copied()
        .zip(vertices.iter().copied().cycle().skip(1))
        .take(vertices.len())
        .filter(|&(start, stop)| openjdk_order1_crosses(start, stop, x, y))
        .count();
    crossings & 1 != 0
}

/// OpenJDK 25 `Curve.crossingsFor` + `Order1.XforY` for one line segment.
pub(crate) fn openjdk_order1_crosses(
    mut start: (f64, f64),
    mut stop: (f64, f64),
    x: f64,
    y: f64,
) -> bool {
    if start.1 == stop.1 {
        // `Curve.insertLine` drops horizontal segments.
        return false;
    }
    if start.1 > stop.1 {
        std::mem::swap(&mut start, &mut stop);
    }
    let (x0, y0) = start;
    let (x1, y1) = stop;
    if !(y >= y0 && y < y1) {
        return false;
    }
    let x_min = x0.min(x1);
    let x_max = x0.max(x1);
    let x_for_y = if x0 == x1 || y <= y0 {
        x0
    } else if y >= y1 {
        x1
    } else {
        x0 + ((y - y0) * (x1 - x0) / (y1 - y0))
    };
    x < x_max && (x < x_min || x < x_for_y)
}

fn run_table_digest(table: &RunTable) -> u64 {
    let orientation = match table.orientation() {
        Orientation::Horizontal => "HORIZONTAL",
        Orientation::Vertical => "VERTICAL",
    };
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    hash_run_row(
        &mut hash,
        &format!("{orientation} {} {}", table.width(), table.height()),
    );
    for sequence in 0..table.sequence_count() {
        let mut row = sequence.to_string();
        for run in table.sequence(sequence).unwrap_or_default() {
            row.push_str(&format!(" {}:{}", run.start, run.length));
        }
        hash_run_row(&mut hash, &row);
    }
    hash
}

fn hash_run_row(hash: &mut u64, row: &str) {
    for byte in row.bytes().chain(std::iter::once(b'\n')) {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(0x100_0000_01b3);
    }
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
    // Fused noteheads and stem stubs sit exactly on the outermost borders the
    // jitter measures; the headroom gate trims their endpoints out of the fit
    // (see `beam_veto::fused_beam_headroom_enabled`).
    let trimmed = crate::beam_veto::fused_beam_headroom_enabled();
    let jitter = |median: audiveris_image::beam_structure::Segment, side, height: f64| {
        if trimmed {
            let shift = match side {
                JitterSide::Top => -height / 2.0,
                JitterSide::Bottom => height / 2.0,
            };
            let reference = audiveris_image::beam_structure::Segment {
                x1: median.x1,
                y1: median.y1 + shift,
                x2: median.x2,
                y2: median.y2 + shift,
            };
            audiveris_image::beam_structure::compute_jitter_trimmed(
                glyph,
                offset_x,
                offset_y,
                median,
                side,
                item.corner_margin,
                reference,
                sheet.max_distance_to_border,
            )
        } else {
            compute_jitter(glyph, offset_x, offset_y, median, side, item.corner_margin)
        }
    };
    let top = jitter(first.median, JitterSide::Top, first.height);
    let bottom = jitter(last.median, JitterSide::Bottom, last.height);
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
    create_beam_inters_recording(
        structure,
        glyph,
        offset_x,
        offset_y,
        pixels,
        item_parameters,
        sheet,
        &mut Vec::new(),
    )
}

/// [`create_beam_inters`] that also reports why each item failed.
///
/// Java drops the impact rejection on the floor, so a beam-shaped spot that
/// produced a structure and then graded to nothing is indistinguishable from
/// one that was never examined. The returned beams are identical either way.
#[allow(clippy::too_many_arguments)]
pub fn create_beam_inters_recording(
    structure: &BeamStructureAnalysis,
    glyph: &RunTable,
    offset_x: i32,
    offset_y: i32,
    pixels: BeamRaster<'_>,
    item_parameters: &ItemParameters,
    sheet: &SheetParameters,
    item_rejections: &mut Vec<(&'static str, Option<String>)>,
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
            match audiveris_image::beam_structure::compute_beam_impacts(
                *item,
                BeamBeltSides {
                    above: index == 0,
                    below: index == line_count - 1,
                },
                pixels,
                distance,
                item_parameters.impacts(sheet),
            ) {
                Err(rejection) => item_rejections.push((
                    match rejection {
                    audiveris_image::beam_structure::BeamImpactRejection::Width => "item width",
                    audiveris_image::beam_structure::BeamImpactRejection::HeightBelow => {
                        "item too thin"
                    }
                    audiveris_image::beam_structure::BeamImpactRejection::HeightAbove => {
                        "item too thick"
                    }
                    audiveris_image::beam_structure::BeamImpactRejection::CoreRatio => {
                        "item core too pale"
                    }
                    audiveris_image::beam_structure::BeamImpactRejection::BeltRatio => {
                        "item belt too inky"
                    }
                    },
                    None,
                )),
                Ok(impacts) => {
                let impacts = clamped(impacts);
                let grade = beam_grade(impacts);
                if grade < MIN_INTER_GRADE {
                    item_rejections.push((
                        "item grade below floor",
                        Some(format!(
                            "grade {grade:.3} w {:.2} minh {:.2} maxh {:.2} core {:.2} belt {:.2} dist {:.2}",
                            impacts.width,
                            impacts.min_height,
                            impacts.max_height,
                            impacts.core,
                            impacts.belt,
                            impacts.distance,
                        )),
                    ));
                }
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

    #[test]
    fn registered_glyph_masks_no_staff_inside_the_final_beam_area() {
        let mut binary = vec![FOREGROUND; 7 * 5];
        binary[2 * 7 + 3] = BACKGROUND;
        let glyph = retrieve_beam_glyph(
            BeamItem {
                median: audiveris_image::beam_structure::Segment {
                    x1: 1.0,
                    y1: 2.0,
                    x2: 5.0,
                    y2: 2.0,
                },
                height: 2.0,
            },
            7,
            5,
            &binary,
        )
        .expect("valid registered beam glyph");

        assert_eq!(
            glyph.bounds,
            BeamBounds {
                x: 1,
                y: 1,
                width: 4,
                height: 2,
            }
        );
        assert_eq!(glyph.weight(), 7);
        assert_eq!(
            glyph.run_table.to_pixels(),
            vec![
                FOREGROUND, FOREGROUND, FOREGROUND, FOREGROUND, FOREGROUND, FOREGROUND, BACKGROUND,
                FOREGROUND,
            ]
        );
        assert_ne!(glyph.run_digest(), 0);
    }

    #[test]
    fn registered_glyph_uses_openjdk_area_crossings_on_batuque_boundary_ink() {
        // Batuque system 1, beam-only ordinal 1.  The determinant form of
        // LineUtil.yAtX puts (1649, 308) inside, while the exact JDK25 Area
        // Order1.XforY crossing puts it on the exterior.  That one pixel was
        // enough to change the registered fixed-glyph digest without changing
        // its 117 vertical-run count.
        let item = BeamItem {
            median: audiveris_image::beam_structure::Segment {
                x1: f64::from_bits(0x4098_dc00_0000_0000),
                y1: f64::from_bits(0x4073_7bd6_4a47_f016),
                x2: f64::from_bits(0x409a_b000_0000_0000),
                y2: f64::from_bits(0x4073_be9c_a677_b957),
            },
            height: f64::from_bits(0x4027_3c1a_b68a_0530),
        };
        let determinant_y = item.median.y_at_x(1649.0);
        assert!(308.0 >= determinant_y - (item.height / 2.0));
        assert!(!beam_parallelogram_contains(item, 1649.0, 308.0));
        assert!(beam_parallelogram_contains(item, 1649.0, 309.0));

        let (width, height) = (1709, 323);
        let mut binary = vec![BACKGROUND; width * height];
        binary[(308 * width) + 1649] = FOREGROUND;
        binary[(309 * width) + 1649] = FOREGROUND;
        let glyph = retrieve_beam_glyph(item, width, height, &binary)
            .expect("valid Batuque boundary fixture");

        assert_eq!(
            glyph.bounds,
            BeamBounds {
                x: 1591,
                y: 305,
                width: 117,
                height: 17
            }
        );
        assert_eq!(glyph.weight(), 1);
        assert_eq!(
            glyph.run_table.sequence(1649 - 1591),
            Some(
                &[audiveris_image::run_table::Run {
                    start: 309 - 305,
                    length: 1,
                }][..]
            )
        );
    }

    #[test]
    fn registered_glyph_clips_the_beam_box_to_the_sheet() {
        let glyph = retrieve_beam_glyph(
            BeamItem {
                median: audiveris_image::beam_structure::Segment {
                    x1: -2.0,
                    y1: 0.5,
                    x2: 2.0,
                    y2: 0.5,
                },
                height: 3.0,
            },
            3,
            2,
            &[FOREGROUND; 6],
        )
        .expect("clipped registered beam glyph");

        assert_eq!(
            glyph.bounds,
            BeamBounds {
                x: 0,
                y: 0,
                width: 2,
                height: 2,
            }
        );
        assert_eq!(glyph.weight(), 4);
    }
}
