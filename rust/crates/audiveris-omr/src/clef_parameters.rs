// SPDX-License-Identifier: AGPL-3.0-or-later

//! Java `ClefBuilder.Parameters`: the scale-derived constants the clef stage runs on.
//!
//! The reason this is its own module rather than a literal in the driver is the split it encodes.
//! `ClefBuilder.Parameters` takes **two** interlines and uses each for a specific subset:
//!
//! - the *sheet's* large interline for `maxClefEnd`, `beltMargin`, `xCoreMargin`, `yCoreMargin`
//!   — the lookup geometry;
//! - the *staff's* specific interline for `aboveStaff`, `belowStaff`, `minPartWeight`,
//!   `maxPartGap`, `maxGlyphHeight`, `minGlyphWeight` — the part and glyph filters.
//!
//! On every sheet in the current corpus those two are equal, so nothing here is graded against a
//! case that would tell them apart. See `synthetic_small_staff_separates_the_two_interlines`,
//! which constructs one deliberately.

/// Java `Scale.Fraction`, `rint(interline * value)`.
fn fraction(interline: i32, value: f64) -> i32 {
    (f64::from(interline) * value).round_ties_even() as i32
}

/// Java `Scale.Fraction` without rounding, `InterlineScale.toPixelsDouble`.
fn fraction_double(interline: i32, value: f64) -> f64 {
    f64::from(interline) * value
}

/// Java `Scale.AreaFraction`, `rint(interline^2 * value)`.
fn area_fraction(interline: i32, value: f64) -> i32 {
    (f64::from(interline) * f64::from(interline) * value).round_ties_even() as i32
}

/// The literal constants from `ClefBuilder.Constants`, in declaration order.
mod constants {
    /// Maximum x distance from measure start to end of clef.
    pub const MAX_CLEF_END: f64 = 4.5;
    /// Top of lookup area above the stave.
    pub const ABOVE_STAFF: f64 = 3.0;
    /// Bottom of lookup area below the stave.
    pub const BELOW_STAFF: f64 = 3.25;
    /// White margin within the raw rectangle.
    pub const BELT_MARGIN: f64 = 0.15;
    /// Horizontal margin around the core rectangle.
    pub const X_CORE_MARGIN: f64 = 0.2;
    /// Vertical margin around the core rectangle.
    pub const Y_CORE_MARGIN: f64 = 0.5;
    /// Maximum number of glyph parts kept before combination.
    pub const MAX_PART_COUNT: usize = 8;
    /// Minimum weight for a glyph part.
    pub const MIN_PART_WEIGHT: f64 = 0.01;
    /// Maximum distance between two parts of a single clef symbol.
    pub const MAX_PART_GAP: f64 = 0.75;
    /// Maximum height for a clef glyph.
    pub const MAX_GLYPH_HEIGHT: f64 = 9.0;
    /// Minimum weight for a clef glyph.
    pub const MIN_GLYPH_WEIGHT: f64 = 1.0;
    /// Maximum classifier rank considered.
    pub const MAX_EVAL_RANK: usize = 3;
}

/// The subset of `ClefBuilder.Parameters` derived from the **sheet** interline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SheetClefParameters {
    /// `maxClefEnd`, the furthest a clef may end from the measure start.
    pub max_clef_end: i32,
    /// `beltMargin`, shrinking the outer lookup rectangle horizontally.
    pub belt_margin: i32,
    /// `xCoreMargin`, insetting the inner rectangle's left edge.
    pub x_core_margin: i32,
    /// `yCoreMargin`, insetting the inner rectangle vertically.
    pub y_core_margin: i32,
}

impl SheetClefParameters {
    /// Derives the sheet-scaled subset from the sheet's large interline.
    #[must_use]
    pub fn new(sheet_interline: i32) -> Self {
        Self {
            max_clef_end: fraction(sheet_interline, constants::MAX_CLEF_END),
            belt_margin: fraction(sheet_interline, constants::BELT_MARGIN),
            x_core_margin: fraction(sheet_interline, constants::X_CORE_MARGIN),
            y_core_margin: fraction(sheet_interline, constants::Y_CORE_MARGIN),
        }
    }
}

/// The subset of `ClefBuilder.Parameters` derived from a **staff's specific** interline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaffClefParameters {
    /// `aboveStaff`, extending the lookup area above the staff.
    pub above_staff: i32,
    /// `belowStaff`, extending it below.
    pub below_staff: i32,
    /// `minPartWeight`, below which a part is discarded outright.
    pub min_part_weight: i32,
    /// `maxPartGap`, the linking distance between parts of one symbol.
    pub max_part_gap: f64,
    /// `maxGlyphHeight`, above which a compound is rejected as too large.
    pub max_glyph_height: f64,
    /// `minGlyphWeight`, below which a compound is rejected as too light.
    pub min_glyph_weight: i32,
}

impl StaffClefParameters {
    /// Derives the staff-scaled subset from that staff's specific interline.
    #[must_use]
    pub fn new(staff_interline: i32) -> Self {
        Self {
            above_staff: fraction(staff_interline, constants::ABOVE_STAFF),
            below_staff: fraction(staff_interline, constants::BELOW_STAFF),
            min_part_weight: area_fraction(staff_interline, constants::MIN_PART_WEIGHT),
            max_part_gap: fraction_double(staff_interline, constants::MAX_PART_GAP),
            max_glyph_height: fraction_double(staff_interline, constants::MAX_GLYPH_HEIGHT),
            min_glyph_weight: area_fraction(staff_interline, constants::MIN_GLYPH_WEIGHT),
        }
    }
}

/// `maxPartCount`, which is a plain integer rather than a scaled one.
#[must_use]
pub const fn max_part_count() -> usize {
    constants::MAX_PART_COUNT
}

/// `maxEvalRank`, likewise unscaled.
#[must_use]
pub const fn max_eval_rank() -> usize {
    constants::MAX_EVAL_RANK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sheet_parameters_match_java_rounding_at_the_corpus_interlines() {
        // interline 20: belt = rint(3.0) = 3, xCore = rint(4.0) = 4, yCore = rint(10.0) = 10.
        let twenty = SheetClefParameters::new(20);
        assert_eq!(twenty.max_clef_end, 90);
        assert_eq!(twenty.belt_margin, 3);
        assert_eq!(twenty.x_core_margin, 4);
        assert_eq!(twenty.y_core_margin, 10);

        // interline 21 is the corpus' most common, and it lands on *two* ties at once, which is
        // why it is the useful case: 21 * 4.5 = 94.5 and 21 * 0.5 = 10.5. `Math.rint` sends both
        // to even -- 94 and 10 -- where `round()` would give 95 and 11. Both are wrong in the same
        // direction, so a port using `round()` fails here rather than half-passing.
        let twenty_one = SheetClefParameters::new(21);
        assert_eq!(twenty_one.max_clef_end, 94, "rint(94.5) -> 94, not 95");
        assert_eq!(twenty_one.belt_margin, 3); // 3.15, not a tie
        assert_eq!(twenty_one.x_core_margin, 4); // 4.2, not a tie
        assert_eq!(twenty_one.y_core_margin, 10, "rint(10.5) -> 10, not 11");
    }

    #[test]
    fn staff_parameters_scale_weights_by_the_square_and_keep_gaps_fractional() {
        let staff = StaffClefParameters::new(21);
        assert_eq!(staff.above_staff, 63);
        // 21 * 3.25 = 68.25 -> 68.
        assert_eq!(staff.below_staff, 68);
        // Area fractions square the interline: 441 * 0.01 = 4.41 -> 4, and 441 * 1.0 = 441.
        assert_eq!(staff.min_part_weight, 4);
        assert_eq!(staff.min_glyph_weight, 441);
        // These two stay fractional in Java (`toPixelsDouble`) and must not be rounded here.
        assert!((staff.max_part_gap - 15.75).abs() < 1e-12);
        assert!((staff.max_glyph_height - 189.0).abs() < 1e-12);
    }

    #[test]
    fn synthetic_small_staff_separates_the_two_interlines() {
        // Nothing in the corpus does this: on all 65 oracle staves the specific interline equals
        // the sheet's, so a port that used one where Java uses the other would pass unnoticed.
        // This constructs the case Java would distinguish -- a small staff on a large sheet -- and
        // asserts that the two subsets really do disagree, so the split cannot be quietly
        // collapsed later.
        let sheet = SheetClefParameters::new(24);
        let staff = StaffClefParameters::new(16);

        assert_eq!(sheet.belt_margin, 4, "sheet-scaled, from interline 24");
        assert_eq!(staff.above_staff, 48, "staff-scaled, from interline 16");

        // The same constants read against the wrong interline give visibly different pixels.
        assert_ne!(SheetClefParameters::new(16).belt_margin, sheet.belt_margin);
        assert_ne!(StaffClefParameters::new(24).above_staff, staff.above_staff);
        // The weight filters are squared, so the error there is quadratic, not linear.
        assert_eq!(staff.min_glyph_weight, 256);
        assert_eq!(StaffClefParameters::new(24).min_glyph_weight, 576);
    }
}
