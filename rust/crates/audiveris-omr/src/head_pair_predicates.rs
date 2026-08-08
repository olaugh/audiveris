// SPDX-License-Identifier: AGPL-3.0-or-later

//! Context-dependent pair predicates used by the Java HEADS purge.
//!
//! `NoteHeadsBuilder.purge` first gates pairs with
//! `java.awt.Rectangle.intersects`, then delegates duplicate detection to
//! `Inter.isSameAs` and overlap detection to `HeadInter.overlaps`.  The purge
//! loop itself lives in `head_purge`; this module supplies the glyph, staff,
//! pitch, and rectangle semantics which that pure loop deliberately leaves to
//! its caller.

use audiveris_image::run_table::RunTable;

use crate::{head_glyph_retrieval::RetrievedHeadGlyph, head_scanner_slices::JavaRectangle};

/// Java `HeadInter`'s default overlap thresholds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeadOverlapParameters {
    pub maximum_horizontal_ratio: f64,
    pub minimum_pitch_shortcut_ratio: f64,
    pub maximum_area_ratio: f64,
}

impl Default for HeadOverlapParameters {
    fn default() -> Self {
        Self {
            maximum_horizontal_ratio: 0.2,
            minimum_pitch_shortcut_ratio: 0.8,
            maximum_area_ratio: 0.25,
        }
    }
}

/// The fields inspected by Java `Glyph.isIdentical`.
///
/// In particular, Java compares the complete run table, not a digest.  It
/// also rejects tables with different orientations even when they describe
/// the same pixels.
#[derive(Clone, Copy, Debug)]
pub struct HeadGlyphIdentity<'a> {
    pub left: i32,
    pub top: i32,
    pub weight: usize,
    pub run_table: &'a RunTable,
}

impl<'a> From<&'a RetrievedHeadGlyph> for HeadGlyphIdentity<'a> {
    fn from(glyph: &'a RetrievedHeadGlyph) -> Self {
        Self {
            left: glyph.glyph_bounds.x,
            top: glyph.glyph_bounds.y,
            weight: glyph.weight,
            run_table: &glyph.run_table,
        }
    }
}

/// The data inspected by Java `AbstractInter.isSameAs`.
#[derive(Clone, Copy, Debug)]
pub struct HeadSameAsInput<'a, Shape> {
    pub shape: Shape,
    pub bounds: Option<JavaRectangle>,
    pub glyph: Option<HeadGlyphIdentity<'a>>,
}

/// Port Java `AbstractInter.isSameAs` for note heads.
///
/// Equal shape and non-null, equal bounds suffice when either inter has no
/// glyph.  Only when both glyphs are present does Java require exact glyph
/// identity.
#[must_use]
pub fn inter_is_same_as<Shape: Eq>(
    this: &HeadSameAsInput<'_, Shape>,
    that: &HeadSameAsInput<'_, Shape>,
) -> bool {
    if this.shape != that.shape {
        return false;
    }

    let (Some(this_bounds), Some(that_bounds)) = (this.bounds, that.bounds) else {
        return false;
    };
    if this_bounds != that_bounds {
        return false;
    }

    match (this.glyph, that.glyph) {
        (Some(this_glyph), Some(that_glyph)) => glyph_is_identical(this_glyph, that_glyph),
        _ => true,
    }
}

/// Port Java `Glyph.isIdentical` for retrieved head glyphs.
#[must_use]
pub fn glyph_is_identical(this: HeadGlyphIdentity<'_>, that: HeadGlyphIdentity<'_>) -> bool {
    this.top == that.top
        && this.left == that.left
        && this.weight == that.weight
        && this.run_table == that.run_table
}

/// The data inspected by Java `HeadInter.overlaps(HeadInter)`.
///
/// `staff_identity` is an opaque token for Java reference identity.  Callers
/// must therefore assign a unique token to each staff object; a display-only
/// staff number that can repeat between systems is not sufficient.  `None`
/// models a null staff, and two null staffs compare identical in Java.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeadOverlapInput {
    pub staff_identity: Option<usize>,
    pub pitch: f64,
    pub bounds: JavaRectangle,
}

/// Port Java `HeadInter.overlaps(HeadInter)` with its default constants.
///
/// This does not repeat the outer `Rectangle.intersects` gate in
/// `NoteHeadsBuilder.purge`; it implements the predicate itself, including its
/// behavior if called directly with empty or disjoint rectangles.
#[must_use]
pub fn head_inter_overlaps(this: HeadOverlapInput, that: HeadOverlapInput) -> bool {
    head_inter_overlaps_with_parameters(this, that, HeadOverlapParameters::default())
}

/// Port Java `HeadInter.overlaps(HeadInter)` with explicit constant values.
#[must_use]
pub fn head_inter_overlaps_with_parameters(
    this: HeadOverlapInput,
    that: HeadOverlapInput,
    parameters: HeadOverlapParameters,
) -> bool {
    let pitch_distance = (this.staff_identity == that.staff_identity).then(|| {
        java_int_abs(java_integer_pitch(this.pitch).wrapping_sub(java_integer_pitch(that.pitch)))
    });
    if pitch_distance.is_some_and(|distance| distance > 1) {
        return false;
    }

    let common = java_rectangle_intersection(this.bounds, that.bounds);
    let minimum_width = this.bounds.width.min(that.bounds.width);
    let width_ratio = f64::from(common.width) / f64::from(minimum_width);

    if width_ratio <= parameters.maximum_horizontal_ratio {
        return false;
    }
    if width_ratio >= parameters.minimum_pitch_shortcut_ratio
        && pitch_distance.is_some_and(|distance| distance <= 1)
    {
        return true;
    }

    // Every product is Java `int` arithmetic and therefore wraps before the
    // conversion to double.  Signed minima and a zero divisor intentionally
    // produce negative ratios, infinities, or NaN exactly as in Java.
    let this_area = this.bounds.width.wrapping_mul(this.bounds.height);
    let that_area = that.bounds.width.wrapping_mul(that.bounds.height);
    let minimum_area = this_area.min(that_area);
    let common_area = common.width.wrapping_mul(common.height);
    let area_ratio = f64::from(common_area) / f64::from(minimum_area);

    area_ratio > parameters.maximum_area_ratio
}

/// Port `java.awt.Rectangle.intersection(Rectangle)`.
///
/// Unlike `Rectangle.intersects`, Java computes this with 64-bit right and
/// bottom edges.  A disjoint result can have negative dimensions, clamped at
/// `Integer.MIN_VALUE` on extreme underflow.
#[must_use]
pub fn java_rectangle_intersection(this: JavaRectangle, that: JavaRectangle) -> JavaRectangle {
    let mut this_x = this.x;
    let mut this_y = this.y;
    let that_x = that.x;
    let that_y = that.y;
    let mut this_right = i64::from(this_x) + i64::from(this.width);
    let mut this_bottom = i64::from(this_y) + i64::from(this.height);
    let that_right = i64::from(that_x) + i64::from(that.width);
    let that_bottom = i64::from(that_y) + i64::from(that.height);

    if this_x < that_x {
        this_x = that_x;
    }
    if this_y < that_y {
        this_y = that_y;
    }
    if this_right > that_right {
        this_right = that_right;
    }
    if this_bottom > that_bottom {
        this_bottom = that_bottom;
    }

    let width = (this_right - i64::from(this_x)).max(i64::from(i32::MIN));
    let height = (this_bottom - i64::from(this_y)).max(i64::from(i32::MIN));
    JavaRectangle::new(this_x, this_y, width as i32, height as i32)
}

fn java_integer_pitch(pitch: f64) -> i32 {
    // Rust's cast has the same saturating/NaN behavior as Java's narrowing
    // double-to-int conversion; `round_ties_even` is Java `Math.rint`.
    pitch.round_ties_even() as i32
}

const fn java_int_abs(value: i32) -> i32 {
    if value < 0 {
        value.wrapping_neg()
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use audiveris_image::run_table::{BACKGROUND, FOREGROUND, Orientation, RunTable};

    use super::*;

    fn table(orientation: Orientation, pixels: &[u8]) -> RunTable {
        RunTable::from_pixels(orientation, 2, 2, pixels).expect("valid test run table")
    }

    #[test]
    fn same_as_requires_shape_and_two_non_null_equal_bounds() {
        let bounds = JavaRectangle::new(10, 20, 4, 5);
        let one = HeadSameAsInput {
            shape: 1,
            bounds: Some(bounds),
            glyph: None,
        };
        assert!(inter_is_same_as(&one, &one));
        assert!(!inter_is_same_as(
            &one,
            &HeadSameAsInput { shape: 2, ..one }
        ));
        assert!(!inter_is_same_as(
            &one,
            &HeadSameAsInput {
                bounds: Some(JavaRectangle::new(10, 20, 5, 5)),
                ..one
            }
        ));
        assert!(!inter_is_same_as(
            &one,
            &HeadSameAsInput {
                bounds: None,
                ..one
            }
        ));
        assert!(!inter_is_same_as(
            &HeadSameAsInput {
                bounds: None,
                ..one
            },
            &HeadSameAsInput {
                bounds: None,
                ..one
            }
        ));
    }

    #[test]
    fn same_as_checks_glyphs_only_when_both_are_present() {
        let bounds = JavaRectangle::new(10, 20, 2, 2);
        let black = [FOREGROUND; 4];
        let one_table = table(Orientation::Vertical, &black);
        let other_table = table(Orientation::Vertical, &black);
        let different_table = table(
            Orientation::Vertical,
            &[FOREGROUND, BACKGROUND, FOREGROUND, FOREGROUND],
        );
        let glyph = HeadGlyphIdentity {
            left: 10,
            top: 20,
            weight: 4,
            run_table: &one_table,
        };
        let one = HeadSameAsInput {
            shape: 1,
            bounds: Some(bounds),
            glyph: Some(glyph),
        };

        assert!(inter_is_same_as(
            &one,
            &HeadSameAsInput { glyph: None, ..one }
        ));
        assert!(inter_is_same_as(
            &one,
            &HeadSameAsInput {
                glyph: Some(HeadGlyphIdentity {
                    run_table: &other_table,
                    ..glyph
                }),
                ..one
            }
        ));
        assert!(!inter_is_same_as(
            &one,
            &HeadSameAsInput {
                glyph: Some(HeadGlyphIdentity {
                    weight: 3,
                    run_table: &different_table,
                    ..glyph
                }),
                ..one
            }
        ));
    }

    #[test]
    fn glyph_identity_includes_position_weight_orientation_and_runs() {
        let pixels = [FOREGROUND, BACKGROUND, FOREGROUND, FOREGROUND];
        let vertical = table(Orientation::Vertical, &pixels);
        let vertical_copy = table(Orientation::Vertical, &pixels);
        let horizontal = table(Orientation::Horizontal, &pixels);
        let glyph = HeadGlyphIdentity {
            left: 7,
            top: 11,
            weight: 3,
            run_table: &vertical,
        };

        assert!(glyph_is_identical(
            glyph,
            HeadGlyphIdentity {
                run_table: &vertical_copy,
                ..glyph
            }
        ));
        assert!(!glyph_is_identical(
            glyph,
            HeadGlyphIdentity { left: 8, ..glyph }
        ));
        assert!(!glyph_is_identical(
            glyph,
            HeadGlyphIdentity { top: 12, ..glyph }
        ));
        assert!(!glyph_is_identical(
            glyph,
            HeadGlyphIdentity { weight: 2, ..glyph }
        ));
        assert!(!glyph_is_identical(
            glyph,
            HeadGlyphIdentity {
                run_table: &horizontal,
                ..glyph
            }
        ));
    }

    #[test]
    fn overlap_rejects_same_staff_integer_pitch_distance_over_one() {
        let bounds = JavaRectangle::new(0, 0, 10, 10);
        let one = HeadOverlapInput {
            staff_identity: Some(4),
            pitch: 0.5,
            bounds,
        };
        let two = HeadOverlapInput { pitch: 1.5, ..one };

        // Math.rint(0.5) = 0 and Math.rint(1.5) = 2.
        assert!(!head_inter_overlaps(one, two));
        assert!(head_inter_overlaps(
            one,
            HeadOverlapInput { pitch: 1.0, ..two }
        ));
    }

    #[test]
    fn overlap_pitch_shortcut_requires_identical_staff() {
        let one = HeadOverlapInput {
            staff_identity: Some(10),
            pitch: 0.0,
            bounds: JavaRectangle::new(0, 0, 10, 10),
        };
        let mostly_horizontal = HeadOverlapInput {
            staff_identity: Some(10),
            pitch: 1.0,
            bounds: JavaRectangle::new(1, 8, 10, 10),
        };

        assert!(head_inter_overlaps(one, mostly_horizontal));
        assert!(!head_inter_overlaps(
            one,
            HeadOverlapInput {
                staff_identity: Some(11),
                ..mostly_horizontal
            }
        ));
    }

    #[test]
    fn overlap_thresholds_are_inclusive_for_width_and_strict_for_area() {
        let one = HeadOverlapInput {
            staff_identity: Some(1),
            pitch: 0.0,
            bounds: JavaRectangle::new(0, 0, 10, 10),
        };

        // Horizontal ratio 0.2 is rejected by `<=`.
        assert!(!head_inter_overlaps(
            one,
            HeadOverlapInput {
                staff_identity: Some(2),
                bounds: JavaRectangle::new(8, 0, 10, 10),
                ..one
            }
        ));
        // Horizontal ratio 0.8 enters the `>=` pitch shortcut.
        assert!(head_inter_overlaps(
            one,
            HeadOverlapInput {
                bounds: JavaRectangle::new(2, 9, 10, 10),
                ..one
            }
        ));
        // Between the horizontal thresholds, area ratio 0.25 is accepted as
        // non-overlap because Java uses a strict `>` comparison.
        assert!(!head_inter_overlaps(
            one,
            HeadOverlapInput {
                staff_identity: Some(2),
                bounds: JavaRectangle::new(5, 5, 10, 10),
                ..one
            }
        ));
        assert!(head_inter_overlaps(
            one,
            HeadOverlapInput {
                staff_identity: Some(2),
                bounds: JavaRectangle::new(5, 4, 10, 10),
                ..one
            }
        ));
    }

    #[test]
    fn rectangle_intersection_uses_long_edges_and_clamps_underflow() {
        assert_eq!(
            java_rectangle_intersection(
                JavaRectangle::new(i32::MAX - 3, 5, 10, 10),
                JavaRectangle::new(i32::MAX - 1, 7, 4, 4),
            ),
            JavaRectangle::new(i32::MAX - 1, 7, 4, 4)
        );
        assert_eq!(
            java_rectangle_intersection(
                JavaRectangle::new(i32::MIN, 0, -1, 1),
                JavaRectangle::new(i32::MAX, 0, -1, 1),
            ),
            JavaRectangle::new(i32::MAX, 0, i32::MIN, 1)
        );
    }

    #[test]
    fn overlap_area_products_wrap_as_java_ints() {
        let one = HeadOverlapInput {
            staff_identity: Some(1),
            pitch: 0.0,
            bounds: JavaRectangle::new(0, 0, 50_000, 50_000),
        };
        let two = HeadOverlapInput {
            staff_identity: Some(2),
            pitch: 0.0,
            bounds: JavaRectangle::new(25_000, 0, 50_000, 50_000),
        };

        // Both whole-box areas wrap negative, while the half-width common
        // area remains positive.  Java therefore gets a negative ratio.
        assert!(!head_inter_overlaps(one, two));
    }

    #[test]
    fn overlap_preserves_java_nan_and_extreme_pitch_behavior() {
        let empty = HeadOverlapInput {
            staff_identity: None,
            pitch: f64::NAN,
            bounds: JavaRectangle::new(0, 0, 0, 10),
        };
        // 0/0 makes both ratios NaN; every Java comparison is false.
        assert!(!head_inter_overlaps(empty, empty));

        let bounds = JavaRectangle::new(0, 0, 10, 10);
        let minimum = HeadOverlapInput {
            staff_identity: Some(1),
            pitch: f64::from(i32::MIN),
            bounds,
        };
        let zero = HeadOverlapInput {
            pitch: 0.0,
            ..minimum
        };
        // Java subtraction and Math.abs both preserve Integer.MIN_VALUE on
        // overflow, so the nominally huge distance is not greater than one
        // and does satisfy the later `<= 1` shortcut.
        assert!(head_inter_overlaps(minimum, zero));
    }
}
