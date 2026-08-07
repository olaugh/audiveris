// SPDX-License-Identifier: AGPL-3.0-or-later

//! Java `KeyExtractor.Parameters`: the scale-derived constants the key alteration search runs on.
//!
//! Every value here is scaled by the **staff's specific** interline. Note the asymmetry with the
//! classifier call in the same class: `KeyExtractor` scales these from `staffSpecific` but hands
//! the classifier `sheet.getInterline()`. Both are Java's behaviour and they are not the same
//! number on a mixed-size sheet.
//!
//! The size thresholds are deliberately `f64`. Java compares an integer `Rectangle` dimension
//! against a `toPixelsDouble` threshold — `bounds.width > params.maxGlyphWidth` — so rounding the
//! threshold to an integer first changes the comparison at every half-pixel. At interline 21,
//! `maxGlyphWidth` is 42.0 and harmless, but `minGlyphWidth` is 10.5: a 10-pixel-wide glyph is too
//! small either way, while an 11-pixel one is accepted by Java and would be accepted by a `rint`ed
//! threshold of 10 as well — but a threshold of 11 would reject it. Keeping the double is free.

/// Java `Scale.Fraction` without rounding, `InterlineScale.toPixelsDouble`.
fn fraction_double(interline: i32, value: f64) -> f64 {
    f64::from(interline) * value
}

/// Java `Scale.AreaFraction`, `rint(interline^2 * value)`.
///
/// Weights really are integers in Java — `toPixels(AreaFraction)` rounds — so unlike the linear
/// dimensions these are compared as integers on both sides.
fn area_fraction(interline: i32, value: f64) -> i32 {
    (f64::from(interline) * f64::from(interline) * value).round_ties_even() as i32
}

/// The literal constants from `KeyExtractor.Constants`.
mod constants {
    /// Maximum number of glyph parts kept before combination.
    pub const MAX_PART_COUNT: usize = 8;
    /// Maximum classifier rank considered.
    pub const MAX_EVAL_RANK: usize = 3;
    /// Minimum weight for a glyph part.
    pub const MIN_PART_WEIGHT: f64 = 0.01;
    /// Maximum distance between two parts of one alteration.
    pub const MAX_PART_GAP: f64 = 1.5;
    /// Minimum glyph width.
    pub const MIN_GLYPH_WIDTH: f64 = 0.5;
    /// Maximum glyph width.
    pub const MAX_GLYPH_WIDTH: f64 = 2.0;
    /// Minimum glyph height.
    pub const MIN_GLYPH_HEIGHT: f64 = 1.0;
    /// Maximum glyph height.
    pub const MAX_GLYPH_HEIGHT: f64 = 3.8;
    /// Minimum glyph weight.
    pub const MIN_GLYPH_WEIGHT: f64 = 0.2;
    /// Maximum glyph weight.
    pub const MAX_GLYPH_WEIGHT: f64 = 3.4;
}

/// `KeyExtractor.Parameters`, scaled by one staff's specific interline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeyExtractorParameters {
    /// `minPartWeight`, below which a component is discarded before combination.
    pub minimum_part_weight: i32,
    /// `maxPartGap`, the linking distance between parts of one alteration.
    pub maximum_part_gap: f64,
    /// `minGlyphWidth`, Java's `isTooSmall` lower bound.
    pub minimum_glyph_width: f64,
    /// `maxGlyphWidth`, Java's `isTooLarge` upper bound.
    pub maximum_glyph_width: f64,
    /// `minGlyphHeight`, the other half of `isTooSmall`.
    pub minimum_glyph_height: f64,
    /// `maxGlyphHeight`, the other half of `isTooLarge`.
    pub maximum_glyph_height: f64,
    /// `minGlyphWeight`, Java's `isTooLight`.
    pub minimum_glyph_weight: i32,
    /// `maxGlyphWeight`, Java's `isTooHeavy`.
    pub maximum_glyph_weight: i32,
}

impl KeyExtractorParameters {
    /// Derives the set from a staff's specific interline.
    #[must_use]
    pub fn new(staff_interline: i32) -> Self {
        Self {
            minimum_part_weight: area_fraction(staff_interline, constants::MIN_PART_WEIGHT),
            maximum_part_gap: fraction_double(staff_interline, constants::MAX_PART_GAP),
            minimum_glyph_width: fraction_double(staff_interline, constants::MIN_GLYPH_WIDTH),
            maximum_glyph_width: fraction_double(staff_interline, constants::MAX_GLYPH_WIDTH),
            minimum_glyph_height: fraction_double(staff_interline, constants::MIN_GLYPH_HEIGHT),
            maximum_glyph_height: fraction_double(staff_interline, constants::MAX_GLYPH_HEIGHT),
            minimum_glyph_weight: area_fraction(staff_interline, constants::MIN_GLYPH_WEIGHT),
            maximum_glyph_weight: area_fraction(staff_interline, constants::MAX_GLYPH_WEIGHT),
        }
    }

    /// Java `SingleAdapter.isTooSmall` and `isTooLarge`, together.
    ///
    /// Both compare the integer bounds against the unrounded double thresholds.
    #[must_use]
    pub fn accepts_size(&self, width: i32, height: i32) -> bool {
        let width = f64::from(width);
        let height = f64::from(height);
        width >= self.minimum_glyph_width
            && height >= self.minimum_glyph_height
            && width <= self.maximum_glyph_width
            && height <= self.maximum_glyph_height
    }

    /// Java `SingleAdapter.isTooLight` and `isTooHeavy`, together.
    #[must_use]
    pub fn accepts_weight(&self, weight: i32) -> bool {
        weight >= self.minimum_glyph_weight && weight <= self.maximum_glyph_weight
    }
}

/// `maxPartCount`, an unscaled integer.
#[must_use]
pub const fn max_part_count() -> usize {
    constants::MAX_PART_COUNT
}

/// `maxEvalRank`, likewise unscaled.
#[must_use]
pub const fn max_eval_rank() -> usize {
    constants::MAX_EVAL_RANK
}

/// Java `HeaderBuilder.constants.maxHeaderWidth`, scaled by the **sheet** interline.
///
/// This is the `projectionWidth` `KeyColumn.retrieveKeys` is called with.
#[must_use]
pub fn max_header_width(sheet_interline: i32) -> i32 {
    (f64::from(sheet_interline) * 15.0).round_ties_even() as i32
}

/// Java `KeyColumn.constants.maxSliceDist`, the tolerance to a theoretical slice abscissa.
#[must_use]
pub fn max_slice_distance(sheet_interline: i32) -> i32 {
    (f64::from(sheet_interline) * 0.5).round_ties_even() as i32
}

/// Java `KeyBuilder.getBrowseRect`, returning the vertical envelope `(yMin, yMax)`.
///
/// The area spans the staff's own lines, extended two interlines above and one below, so that it
/// embraces any key signature whatever the clef.
///
/// **Reproduces a bug in Java deliberately.** The original loops `x` from `xMin` to `xMax` and then
/// evaluates `staff.getFirstLine().yAt(xMin)` inside the loop -- `xMin`, not `x`. The loop
/// therefore computes the same value every iteration and the sweep does nothing; the envelope is
/// decided entirely by the ordinates at `xMin`. This takes those two ordinates as arguments rather
/// than a range, so the signature states that fact instead of hiding a dead sweep. If Audiveris
/// ever fixes it, the envelope will widen on sloped staves and this must change with it.
#[must_use]
pub fn browse_envelope(
    first_line_y_at_x_min: i32,
    last_line_y_at_x_min: i32,
    staff_interline: i32,
) -> (i32, i32) {
    (
        first_line_y_at_x_min - (2 * staff_interline),
        last_line_y_at_x_min + staff_interline,
    )
}

/// Java `Scale.Fraction` with `rint`, `InterlineScale.toPixels`.
fn fraction_int(interline: i32, value: f64) -> i32 {
    (f64::from(interline) * value).round_ties_even() as i32
}

/// Java `KeyBuilder.Parameters`, the projection-peak subset.
///
/// Two interlines again, split exactly as Java's constructor splits them: the **sheet's** large
/// interline scales the abscissa-comparison block (`maxFirstPeakOffset`, `maxFirstSpaceWidth`,
/// `maxInnerSpace`, `maxInnerPeakGap`, `maxPeakDx`, the light-peak dx floors and the sharp/flat
/// spacing deltas), while the **staff's** specific interline scales the projection-value and
/// trail block (`maxSpaceCumul`, `minPeakDerivative`, `minPeakCumul`, `maxPeakCumul`,
/// `maxPeakWidth`, `coreStemLength`, the headings and trails). On this corpus the two are always
/// equal, so nothing grades the split — same caveat as `clef_parameters`, recorded rather than
/// hidden.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeyPipelineParameters {
    /// `maxSpaceCumul`: cumul at or below this is "space".
    pub max_space_cumul: i32,
    /// `minPeakDerivative` for the HiLo finder.
    pub min_peak_derivative: i32,
    /// `minPeakCumul`: the finder's quorum and the area baseline.
    pub min_peak_cumul: i32,
    /// `maxPeakCumul`: taller than this is not a key stem.
    pub max_peak_cumul: i32,
    /// `maxPeakWidth`: wider than this is not a key stem.
    pub max_peak_width: i32,
    /// `minGainRatio` for the HiLo finder (unscaled ratio).
    pub min_gain_ratio: f64,
    /// `coreStemLength`: rows a stem must span.
    pub core_stem_length: i32,
    /// `minBlackRatio`: fraction of black rows within the stem core (unscaled ratio).
    pub min_black_ratio: f64,
    /// `peakAreaQuorum` (unscaled ratio).
    pub peak_area_quorum: f64,
    /// `maxFirstPeakOffset` from browse start.
    pub max_first_peak_offset: i32,
    /// `maxFirstSpaceWidth` before any peak.
    pub max_first_space_width: i32,
    /// `maxInnerSpace` between peaks.
    pub max_inner_space: i32,
    /// `maxInnerPeakGap` between hilos.
    pub max_inner_peak_gap: i32,
    /// `maxPeakDx` between successive peaks.
    pub max_peak_dx: i32,
    /// `minSharpLightPeakDx` (fractional, compared against a f64 distance).
    pub min_sharp_light_peak_dx: f64,
    /// `minFlatLightPeakDx`.
    pub min_flat_light_peak_dx: f64,
    /// `maxSharpDelta`: mean short interval above this is not sharp spacing.
    pub max_sharp_delta: f64,
    /// `minFlatDelta`: mean short interval below this is not flat spacing.
    pub min_flat_delta: f64,
    /// `maxFlatHeading` before the first flat peak.
    pub max_flat_heading: i32,
    /// `stdFlatTrail` / `minFlatTrail` / `maxFlatTrail`.
    pub std_flat_trail: i32,
    /// Minimum flat trail.
    pub min_flat_trail: i32,
    /// Maximum flat trail.
    pub max_flat_trail: i32,
    /// `stdSharpTrail` / `minSharpTrail` / `maxSharpTrail`.
    pub std_sharp_trail: i32,
    /// Minimum sharp trail.
    pub min_sharp_trail: i32,
    /// Maximum sharp trail.
    pub max_sharp_trail: i32,
    /// `maxTrailingCumul` for the trailing-space check.
    pub max_trailing_cumul: i32,
    /// `minTrailingSpace` width for the trailing-space check.
    pub min_trailing_space: i32,
}

impl KeyPipelineParameters {
    /// Derives the set from the sheet interline and one staff's specific interline.
    #[must_use]
    pub fn new(sheet_interline: i32, staff_interline: i32) -> Self {
        Self {
            max_space_cumul: fraction_int(staff_interline, 0.4),
            min_peak_derivative: fraction_int(staff_interline, 0.35),
            min_peak_cumul: fraction_int(staff_interline, 1.25),
            max_peak_cumul: fraction_int(staff_interline, 4.25),
            max_peak_width: fraction_int(staff_interline, 0.45),
            min_gain_ratio: 0.2,
            core_stem_length: fraction_int(staff_interline, 2.0),
            min_black_ratio: 0.75,
            peak_area_quorum: 0.5,
            max_first_peak_offset: fraction_int(sheet_interline, 2.3),
            max_first_space_width: fraction_int(sheet_interline, 2.1),
            max_inner_space: fraction_int(sheet_interline, 0.7),
            max_inner_peak_gap: fraction_int(sheet_interline, 1.35),
            max_peak_dx: fraction_int(sheet_interline, 1.5),
            min_sharp_light_peak_dx: fraction_double(sheet_interline, 0.25),
            min_flat_light_peak_dx: fraction_double(sheet_interline, 0.5),
            max_sharp_delta: fraction_double(sheet_interline, 0.7),
            min_flat_delta: fraction_double(sheet_interline, 0.45),
            max_flat_heading: fraction_int(staff_interline, 0.2),
            std_flat_trail: fraction_int(staff_interline, 1.0),
            min_flat_trail: fraction_int(staff_interline, 0.7),
            max_flat_trail: fraction_int(staff_interline, 1.3),
            std_sharp_trail: fraction_int(staff_interline, 0.3),
            min_sharp_trail: fraction_int(staff_interline, 0.2),
            max_sharp_trail: fraction_int(staff_interline, 0.5),
            max_trailing_cumul: fraction_int(staff_interline, 0.25),
            min_trailing_space: fraction_int(staff_interline, 0.25),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameters_scale_from_the_staff_interline() {
        let parameters = KeyExtractorParameters::new(21);
        // Area fractions round: 441 * 0.01 = 4.41 -> 4, 441 * 0.2 = 88.2 -> 88,
        // 441 * 3.4 = 1499.4 -> 1499.
        assert_eq!(parameters.minimum_part_weight, 4);
        assert_eq!(parameters.minimum_glyph_weight, 88);
        assert_eq!(parameters.maximum_glyph_weight, 1499);
        // Linear fractions do not.
        assert!((parameters.maximum_part_gap - 31.5).abs() < 1e-12);
        assert!((parameters.minimum_glyph_width - 10.5).abs() < 1e-12);
        assert!((parameters.maximum_glyph_height - 79.8).abs() < 1e-12);
    }

    #[test]
    fn size_thresholds_stay_fractional_where_java_keeps_them_fractional() {
        // The point of not rounding: at interline 21 the minimum width is 10.5, so a 10-pixel
        // glyph is refused and an 11-pixel one accepted. Rounding the threshold to 10 would admit
        // the 10-pixel glyph; rounding to 11 (rint of 10.5 is 10, but `round` gives 11) would be
        // wrong the other way. Java compares against 10.5 and so does this.
        let parameters = KeyExtractorParameters::new(21);
        assert!(!parameters.accepts_size(10, 30));
        assert!(parameters.accepts_size(11, 30));
        // Height 79.8 is the upper bound: 79 passes, 80 does not.
        assert!(parameters.accepts_size(20, 79));
        assert!(!parameters.accepts_size(20, 80));
    }

    #[test]
    fn both_ends_of_each_predicate_are_enforced() {
        // Java has four predicates -- isTooSmall, isTooLarge, isTooLight, isTooHeavy -- and an
        // earlier version of the native filter carried only the upper size bounds and the lower
        // weight bound, so an over-heavy or under-sized glyph would have been accepted.
        let parameters = KeyExtractorParameters::new(20);
        assert!(!parameters.accepts_weight(parameters.minimum_glyph_weight - 1));
        assert!(parameters.accepts_weight(parameters.minimum_glyph_weight));
        assert!(parameters.accepts_weight(parameters.maximum_glyph_weight));
        assert!(!parameters.accepts_weight(parameters.maximum_glyph_weight + 1));
    }

    #[test]
    fn the_projection_width_and_slice_tolerance_scale_from_the_sheet() {
        assert_eq!(max_header_width(21), 315);
        // 21 * 0.5 = 10.5, a tie, and `Math.rint` sends it to the even 10 rather than 11. This is
        // the third distinct constant in the header stages to land on a tie at interline 21, which
        // is the corpus' most common value -- a port using `round()` fails on all three.
        assert_eq!(max_slice_distance(21), 10);
        assert_eq!(max_slice_distance(20), 10);
        assert_eq!(max_slice_distance(23), 12); // 11.5 -> 12, the even side is up here
    }

    #[test]
    fn the_browse_envelope_extends_two_interlines_above_and_one_below() {
        // A staff whose first line is at 400 and last at 480, interline 20.
        assert_eq!(browse_envelope(400, 480, 20), (360, 500));
    }
}
