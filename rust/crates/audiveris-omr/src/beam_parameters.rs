// SPDX-License-Identifier: AGPL-3.0-or-later

//! The scaled constants BEAMS recognises beams with.
//!
//! Ported from `BeamsBuilder.ItemParameters` and `BeamsBuilder.Parameters`.
//! Nothing here is subtle; it matters because the beam kernel already in the
//! tree takes these as inputs, and every one of them was previously supplied by
//! a test. A kernel that is exactly right and fed slightly wrong parameters
//! produces beams that are plausible and not Java's.
//!
//! Two scalings are in play and they are not the same. `toPixels` rounds to an
//! `int` with `Math.rint` -- ties to even -- and `toPixelsDouble` does not round
//! at all. Java picks one per constant, and which one it picked is preserved
//! here term by term.

use audiveris_image::beam_structure::{BeamImpactParameters, BeamStructureParameters};

/// `Scale.toPixelsDouble(Fraction)`: interlines, unrounded.
fn to_pixels_double(interline: i32, fraction: f64) -> f64 {
    f64::from(interline) * fraction
}

/// `Scale.toPixels(Fraction)`: the same, rounded half to even.
fn to_pixels(interline: i32, fraction: f64) -> i32 {
    // `Math.rint`, not `f64::round`: a value landing exactly on a half goes to
    // the even neighbour. The same distinction cost six SIG grades in GRID.
    to_pixels_double(interline, fraction).round_ties_even() as i32
}

/// `BeamsBuilder.ItemParameters`, for one beam size class.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemParameters {
    pub is_small: bool,
    pub min_beam_width_low: f64,
    pub min_beam_width_high: f64,
    pub min_hook_width_low: f64,
    pub min_hook_width_high: f64,
    pub max_hook_width: f64,
    pub min_height_low: f64,
    pub typical_height: f64,
    pub max_height_high: f64,
    pub corner_margin: f64,
    pub max_item_x_gap: f64,
    pub core_section_width: usize,
}

impl ItemParameters {
    /// As `new ItemParameters(scale, height, isSmall)`.
    #[must_use]
    pub fn new(interline: i32, height: f64, is_small: bool) -> Self {
        let typical_height = height;
        Self {
            is_small,
            min_beam_width_low: to_pixels_double(interline, 1.0),
            min_beam_width_high: to_pixels_double(interline, 4.0),
            min_hook_width_low: to_pixels_double(interline, 0.7),
            min_hook_width_high: to_pixels_double(interline, 1.0),
            max_hook_width: to_pixels_double(interline, 2.0),
            typical_height,
            min_height_low: typical_height * 0.7,
            max_height_high: typical_height * 1.4,
            corner_margin: typical_height * 0.2,
            max_item_x_gap: to_pixels_double(interline, 0.5),
            // Java's floor of two: a line slope needs two points, and a small
            // enough interline would otherwise ask for one.
            core_section_width: to_pixels(interline, 0.15).max(2) as usize,
        }
    }

    /// The subset `analyze_beam_structure` reads.
    #[must_use]
    pub fn structure(&self) -> BeamStructureParameters {
        BeamStructureParameters {
            typical_height: self.typical_height,
            core_section_width: self.core_section_width,
            min_hook_width_low: self.min_hook_width_low,
            // The item gap is compared against an integer abscissa distance, so
            // Java's double is truncated where it is used rather than here.
            max_item_x_gap: self.max_item_x_gap as i32,
            min_beam_width_low: self.min_beam_width_low,
            max_hook_width: self.max_hook_width,
        }
    }

    /// The subset `compute_beam_impacts` reads, given the sheet parameters.
    #[must_use]
    pub fn impacts(&self, sheet: &SheetParameters) -> BeamImpactParameters {
        BeamImpactParameters {
            belt_margin_dx: sheet.belt_margin_dx,
            belt_margin_dy: sheet.belt_margin_dy,
            min_core_black_ratio: sheet.min_core_black_ratio,
            max_belt_black_ratio: sheet.max_belt_black_ratio,
            min_width_low: self.min_beam_width_low,
            min_width_high: self.min_beam_width_high,
            min_height_low: self.min_height_low,
            typical_height: self.typical_height,
            max_height_high: self.max_height_high,
        }
    }

    /// The same, for a hook: the width thresholds differ and nothing else does.
    #[must_use]
    pub fn hook_impacts(&self, sheet: &SheetParameters) -> BeamImpactParameters {
        BeamImpactParameters {
            min_width_low: self.min_hook_width_low,
            min_width_high: self.min_hook_width_high,
            ..self.impacts(sheet)
        }
    }
}

/// `BeamsBuilder.Parameters`, which are per sheet rather than per size class.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SheetParameters {
    pub max_side_beam_dx: i32,
    pub min_beams_gap_x: i32,
    pub max_beams_gap_x: i32,
    pub max_beams_gap_y: i32,
    pub beams_x_margin: i32,
    pub max_stem_beam_gap_x: i32,
    pub max_stem_beam_gap_y: i32,
    pub max_extension_to_stem: i32,
    pub max_extension_to_spot: i32,
    pub belt_margin_dx: i32,
    pub belt_margin_dy: i32,
    pub max_beam_slope: f64,
    pub max_border_slope_gap: f64,
    pub max_distance_to_border: f64,
    pub max_jitter_ratio: f64,
    pub max_belt_black_ratio: f64,
    pub min_core_black_ratio: f64,
    pub min_ext_black_ratio: f64,
    pub cue_x_margin: i32,
    pub cue_y_margin: i32,
    pub cue_box_dx: i32,
    pub cue_box_dy: i32,
    pub cue_beam_ratio: f64,
    pub cue_min_beam_head_dy: i32,
}

impl SheetParameters {
    /// As `new Parameters(scale)`.
    #[must_use]
    pub fn new(interline: i32) -> Self {
        Self {
            max_side_beam_dx: to_pixels(interline, 4.0),
            min_beams_gap_x: to_pixels(interline, 0.2),
            max_beams_gap_x: to_pixels(interline, 1.0),
            max_beams_gap_y: to_pixels(interline, 0.25),
            beams_x_margin: to_pixels(interline, 0.25),
            max_stem_beam_gap_x: to_pixels(interline, 0.2),
            max_stem_beam_gap_y: to_pixels(interline, 0.8),
            max_extension_to_stem: to_pixels(interline, 1.0),
            max_extension_to_spot: to_pixels(interline, 0.5),
            belt_margin_dx: to_pixels(interline, 0.25),
            belt_margin_dy: to_pixels(interline, 0.15),
            max_beam_slope: 1.0,
            max_border_slope_gap: 0.15,
            // The one distance Java deliberately leaves unrounded.
            max_distance_to_border: to_pixels_double(interline, 0.15),
            max_jitter_ratio: 0.02,
            max_belt_black_ratio: 0.4,
            min_core_black_ratio: 0.7,
            min_ext_black_ratio: 0.6,
            cue_x_margin: to_pixels(interline, 2.0),
            cue_y_margin: to_pixels(interline, 3.0),
            cue_box_dx: to_pixels(interline, 0.25),
            cue_box_dy: to_pixels(interline, 1.0),
            cue_beam_ratio: 0.6,
            cue_min_beam_head_dy: to_pixels(interline, 1.0),
        }
    }
}

/// The cue-beam height Java falls back to when no small beam scale was measured.
///
/// `Math.rint(scale.getBeamThickness() * cueBeamRatio)`.
#[must_use]
pub fn cue_beam_height(beam_thickness: f64, cue_beam_ratio: f64) -> i32 {
    (beam_thickness * cue_beam_ratio).round_ties_even() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_the_two_ways_java_scales() {
        // 0.15 of an interline of 21 is 3.15: rounded where Java rounds, kept
        // where Java keeps it. Reading the wrong one off a constant is the
        // easiest possible way to be almost right.
        assert_eq!(to_pixels(21, 0.15), 3);
        assert!((to_pixels_double(21, 0.15) - 3.15).abs() < 1e-12);

        // A tie, which is where rint and round disagree. 0.5 * 21 = 10.5.
        assert_eq!(to_pixels(21, 0.5), 10);
        assert_eq!(to_pixels(21, 1.5), 32); // 31.5 -> 32, the even side.
    }

    #[test]
    fn core_section_width_never_falls_below_two() {
        // Two points are needed for a slope. At chula's interline this is not
        // binding; on a small enough page it is.
        assert_eq!(ItemParameters::new(21, 12.0, false).core_section_width, 3);
        assert_eq!(ItemParameters::new(6, 12.0, false).core_section_width, 2);
    }

    #[test]
    fn matches_every_scaled_constant_java_computed() {
        let text = include_str!("../../../oracle/beam-structures.txt");
        let value = |kind: &str, name: &str| -> f64 {
            text.lines()
                .filter(|line| !line.starts_with('#'))
                .map(|line| line.split_whitespace().collect::<Vec<_>>())
                .find(|fields| fields.first() == Some(&kind) && fields.get(1) == Some(&name))
                .unwrap_or_else(|| panic!("no {kind} {name}"))[2]
                .parse()
                .expect("a number")
        };

        let sheet_row: Vec<&str> = text
            .lines()
            .find(|line| line.starts_with("sheet "))
            .expect("a sheet record")
            .split_whitespace()
            .collect();
        let interline: i32 = sheet_row[3].parse().expect("an interline");
        let beam: f64 = sheet_row[5].parse().expect("a beam height");

        let item = ItemParameters::new(interline, beam, false);
        for (name, actual) in [
            ("minBeamWidthLow", item.min_beam_width_low),
            ("minBeamWidthHigh", item.min_beam_width_high),
            ("minHookWidthLow", item.min_hook_width_low),
            ("minHookWidthHigh", item.min_hook_width_high),
            ("maxHookWidth", item.max_hook_width),
            ("minHeightLow", item.min_height_low),
            ("typicalHeight", item.typical_height),
            ("maxHeightHigh", item.max_height_high),
            ("cornerMargin", item.corner_margin),
            ("maxItemXGap", item.max_item_x_gap),
            ("coreSectionWidth", item.core_section_width as f64),
        ] {
            // Compared exactly. These are products of a small integer and a
            // decimal constant, so Java's own value has a specific last bit --
            // minHeightLow is 8.399999999999999, not 8.4 -- and a port that
            // rounds it has changed a threshold.
            assert_eq!(actual, value("itemparam", name), "itemparam {name}");
        }

        let sheet = SheetParameters::new(interline);
        for (name, actual) in [
            ("maxSideBeamDx", f64::from(sheet.max_side_beam_dx)),
            ("minBeamsGapX", f64::from(sheet.min_beams_gap_x)),
            ("maxBeamsGapX", f64::from(sheet.max_beams_gap_x)),
            ("maxBeamsGapY", f64::from(sheet.max_beams_gap_y)),
            ("beamsXMargin", f64::from(sheet.beams_x_margin)),
            ("maxStemBeamGapX", f64::from(sheet.max_stem_beam_gap_x)),
            ("maxStemBeamGapY", f64::from(sheet.max_stem_beam_gap_y)),
            ("maxExtensionToStem", f64::from(sheet.max_extension_to_stem)),
            ("maxExtensionToSpot", f64::from(sheet.max_extension_to_spot)),
            ("beltMarginDx", f64::from(sheet.belt_margin_dx)),
            ("beltMarginDy", f64::from(sheet.belt_margin_dy)),
            ("cueXMargin", f64::from(sheet.cue_x_margin)),
            ("cueYMargin", f64::from(sheet.cue_y_margin)),
            ("cueBoxDx", f64::from(sheet.cue_box_dx)),
            ("cueBoxDy", f64::from(sheet.cue_box_dy)),
            ("cueMinBeamHeadDy", f64::from(sheet.cue_min_beam_head_dy)),
            ("maxDistanceToBorder", sheet.max_distance_to_border),
        ] {
            assert_eq!(actual, value("sheetparam", name), "sheetparam {name}");
        }

        // maxExtensionToSpot is 0.5 of a 21-pixel interline: exactly 10.5, the
        // one value on this page where rint and round disagree. If it reads 11
        // the scaling helper is wrong and half these constants are too.
        assert_eq!(sheet.max_extension_to_spot, 10);
    }

    #[test]
    fn hook_parameters_differ_only_in_width() {
        let sheet = SheetParameters::new(21);
        let item = ItemParameters::new(21, 12.0, false);
        let beam = item.impacts(&sheet);
        let hook = item.hook_impacts(&sheet);
        assert_ne!(
            (beam.min_width_low, beam.min_width_high),
            (hook.min_width_low, hook.min_width_high)
        );
        assert_eq!(
            (
                beam.min_height_low,
                beam.typical_height,
                beam.max_height_high
            ),
            (
                hook.min_height_low,
                hook.typical_height,
                hook.max_height_high
            )
        );
    }
}
