// SPDX-License-Identifier: AGPL-3.0-or-later

//! Music-font metrics that Audiveris' recognition stages read back into the SIG.
//!
//! Two quantities from `MusicFont` reach a real decision, and they are not the same kind of
//! problem:
//!
//! 1. `ShapeSymbol.getCentroid`, whose per-shape offset is **pinned data** — see
//!    [`centroid_offset`] for why it cannot be computed here;
//! 2. `AbstractInter.getSymbolBounds`, which needs `TextLayout.getBounds()` — an outline box,
//!    computed here from the font's own CFF outlines by [`layout_bounds`].
//!
//! Everything here is graded against `rust/oracle/music-font.txt`, captured from the live JVM by
//! `MusicFontScout`.

pub mod cff;
pub mod sfnt;

use cff::Cff;
use sfnt::{FontError, Sfnt};

#[cfg(test)]
use std::collections::HashMap;

/// Java `SampleRepository.STANDARD_INTERLINE`.
///
/// `ShapeSymbol.computeImage` renders at this size whatever font it was asked for, which is the
/// single fact that makes [`centroid_offset`] a constant rather than sheet data.
pub const STANDARD_INTERLINE: i32 = 20;

/// Java `OmrFont.getPointSize`, which is exactly `4 * staffInterline`.
#[must_use]
pub fn point_size(staff_interline: i32) -> i32 {
    4 * staff_interline
}

/// A music font family, as Audiveris' `MusicFamily` enum names them.
///
/// Only the default is present. Audiveris resolves a missing symbol through a chain of backup
/// families, so adding one means adding its fallbacks too, not just its metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MusicFamily {
    /// The SMuFL-compliant default, `Bravura.otf`.
    Bravura,
}

/// The normalized centroid offset of a rendered symbol, as a fraction of its own image size.
///
/// Java stores this as a `Point2D.Double` on `ShapeSymbol`, computed once and cached.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CentroidOffset {
    /// Horizontal offset from the image centre, in units of image width.
    pub x: f64,
    /// Vertical offset from the image centre, in units of image height.
    pub y: f64,
}

/// A `java.awt.Rectangle`, carrying only what [`symbol_centroid`] reads from one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rectangle {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width; Java allows this to be negative and does not normalize it.
    pub width: i32,
    /// Height; likewise.
    pub height: i32,
}

impl Rectangle {
    /// Java `Rectangle2D.getCenterX`, which is `x + width / 2.0` in double arithmetic.
    #[must_use]
    pub fn center_x(&self) -> f64 {
        f64::from(self.x) + f64::from(self.width) / 2.0
    }

    /// Java `Rectangle2D.getCenterY`.
    #[must_use]
    pub fn center_y(&self) -> f64 {
        f64::from(self.y) + f64::from(self.height) / 2.0
    }
}

/// The pinned `(family, shape) -> centroid offset` table.
///
/// **This is measured data, not a computation, and it is deliberately not derived here.** Java
/// obtains it from `ShapeSymbol.computeCentroidOffset`, which takes an alpha-weighted centroid of
/// a *rendered* glyph. The rendering is antialiased — `buildImage` sets `KEY_ANTIALIASING` to
/// `VALUE_ANTIALIAS_OFF`, but the symbol is drawn through `TextLayout.draw`, which obeys
/// `KEY_TEXT_ANTIALIASING` instead, and the measured images carry ~200 distinct alpha values.
/// Reproducing these by computation would mean reproducing the platform's glyph rasteriser.
///
/// Pinning is sound because the value does not vary with anything the sheet controls:
/// `computeImage` renders at [`STANDARD_INTERLINE`] regardless of the font it was handed, so the
/// offset is a function of `(family, shape)` alone. `MusicFontScout` asserts that by asking at
/// seven interlines from 10 to 48 and refusing to emit a value unless all seven agree bit for bit.
///
/// Two limits on how far this data travels:
///
/// - it was captured on **macOS/aarch64**, and glyph rasterisation goes through the platform font
///   scaler — FreeType on Linux against CoreText on macOS — so these are not known to hold on
///   Linux, and the CI matrix cannot tell you, since the Java oracle runs only locally;
/// - it is JDK-stable across at least one major version: identical under Temurin 25.0.3+9 and
///   OpenJDK 26.0.1.
#[must_use]
pub fn centroid_offset(family: MusicFamily, shape: &str) -> Option<CentroidOffset> {
    let MusicFamily::Bravura = family;
    let (x, y) = match shape {
        "F_CLEF" => (-0.038_840_011_073_927_594, -0.133_943_099_331_174_15),
        "G_CLEF" => (0.002_057_250_828_453_538_5, 0.018_883_063_668_162_947),
        "G_CLEF_8VA" => (0.013_368_687_583_753_869, 0.043_817_910_025_551_8),
        "G_CLEF_8VB" => (5.249_130_899_418_475e-4, -0.013_285_699_185_629_44),
        "C_CLEF" => (-0.065_804_711_271_528_7, -0.017_314_097_660_159_4),
        "PERCUSSION_CLEF" => (-0.023_092_249_738_606_407, -0.012_492_505_937_602_705),
        "FLAT" => (-0.190_418_948_238_114_12, 0.045_228_146_163_764_915),
        "NATURAL" => (-0.058_041_393_978_811_25, -0.014_796_444_578_689_172),
        "SHARP" => (-0.027_324_019_122_980_99, -0.012_605_241_156_798_785),
        "COMMON_TIME" => (-0.078_741_903_680_102_81, -0.046_499_610_814_653_7),
        "CUT_TIME" => (-0.085_979_469_418_984_77, -0.022_231_619_145_085_146),
        "TIME_TWO" => (-0.033_012_494_753_659_805, -0.025_986_954_549_849_84),
        "TIME_THREE" => (-0.038_714_658_758_456_755, -0.003_265_495_319_516_753_5),
        "TIME_FOUR" => (-0.006_907_040_244_451_879, 4.711_093_707_945_313e-4),
        _ => return None,
    };
    Some(CentroidOffset { x, y })
}

/// Ports Java `ShapeSymbol.getCentroid`: the symbol's mass centre placed inside a given box.
///
/// Returns `None` for a shape absent from the pinned table rather than guessing, since a wrong
/// centroid here silently shifts a clef's bounds instead of failing.
#[must_use]
pub fn symbol_centroid(family: MusicFamily, shape: &str, box_: Rectangle) -> Option<(i32, i32)> {
    let offset = centroid_offset(family, shape)?;
    Some((
        java_rint_to_int(box_.center_x() + f64::from(box_.width) * offset.x),
        java_rint_to_int(box_.center_y() + f64::from(box_.height) * offset.y),
    ))
}

/// The SMuFL codepoint Audiveris' `BravuraSymbols` assigns to a shape.
///
/// Every shape here is a **single** codepoint, so a layout is one glyph and [`layout_bounds`] does
/// not have to compose advances. That is a real restriction rather than a stage of completeness:
/// `TIME_TWELVE` and `TIME_SIXTEEN` are two codepoints in one string, and the num-over-den shapes
/// such as `TIME_TWO_FOUR` are two separate layouts stacked by `NumDenSymbol`. Those need
/// composition ported before they can appear here, and returning `None` keeps them from being
/// silently mistaken for absent shapes.
#[must_use]
pub fn codepoint(family: MusicFamily, shape: &str) -> Option<u32> {
    let MusicFamily::Bravura = family;
    match shape {
        "G_CLEF" => Some(0xE050),
        "G_CLEF_8VB" => Some(0xE052),
        "G_CLEF_8VA" => Some(0xE053),
        "C_CLEF" => Some(0xE05C),
        "F_CLEF" => Some(0xE062),
        "PERCUSSION_CLEF" => Some(0xE069),
        "COMMON_TIME" => Some(0xE08A),
        "CUT_TIME" => Some(0xE08B),
        // `MusicFont.layoutNumberByCode`: digit d is TIME_ZERO's code plus d.
        "TIME_ZERO" => Some(0xE080),
        "TIME_ONE" => Some(0xE081),
        "TIME_TWO" => Some(0xE082),
        "TIME_THREE" => Some(0xE083),
        "TIME_FOUR" => Some(0xE084),
        "TIME_FIVE" => Some(0xE085),
        "TIME_SIX" => Some(0xE086),
        "TIME_SEVEN" => Some(0xE087),
        "TIME_EIGHT" => Some(0xE088),
        "TIME_NINE" => Some(0xE089),
        "FLAT" => Some(0xE260),
        "NATURAL" => Some(0xE261),
        "SHARP" => Some(0xE262),
        "BRACKET_UPPER_SERIF" => Some(0xE003),
        "BRACKET_LOWER_SERIF" => Some(0xE004),
        // `BravuraSymbols`: the filled oval that `BlackHeadSizer` measures when deriving the
        // sheet's head-specific music-font scale.
        "NOTEHEAD_BLACK" => Some(0xE0A4),
        // `BravuraSymbols.LINE_CODE` and `STAFF_CODE`: a one-line and a five-line staff segment.
        // Not musical symbols -- they exist so `AbstractPitchedInter` can measure how tall one
        // pitch step is in a given font, which is what turns a glyph box into a pitch offset.
        "STAFF_LINE" => Some(0xE010),
        "STAFF_FIVE_LINES" => Some(0xE01A),
        _ => None,
    }
}

/// The bytes of a family's font file.
fn font_bytes(family: MusicFamily) -> &'static [u8] {
    match family {
        MusicFamily::Bravura => include_bytes!("../../../../app/res/Bravura.otf"),
    }
}

/// A `Rectangle2D` in user space, as `TextLayout.getBounds()` returns one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    /// Left edge, relative to the layout origin.
    pub x: f64,
    /// Top edge. Positive y is *down*, so this is normally negative for a glyph above the
    /// baseline.
    pub y: f64,
    /// Right edge minus left edge, both already quantized.
    pub width: f64,
    /// Bottom edge minus top edge, both already quantized.
    pub height: f64,
}

/// Ports `TextLayout.getBounds()` for a single-glyph music symbol at a staff interline.
///
/// The model, recovered by sweeping 116 interlines against the live JVM and checked by
/// `layout_bounds_match_the_java_sweep_on_every_shape_and_size`:
///
/// 1. take the glyph's **exact** outline box in font units, interior curve extrema included;
/// 2. scale each edge with [`mul_fix`], flipping y because font units point up and layout
///    coordinates point down;
/// 3. the result is already on the 1/64 grid, so the width is `right - left` *after* scaling.
///
/// Step 2 is **integer fixed-point arithmetic, not floating point**, and that is the whole
/// subtlety. Scaling by `pointSize / unitsPerEm` in `f64` and rounding to 1/64 reproduces 692 of
/// the 696 swept values and misses four — and no floating-point model fixes them, because Java's
/// scaler rounds twice: once building a 16.16 scale factor, once applying it. The two roundings
/// can disagree with a single rounding of the exact product by one 1/64 step, which is exactly
/// what those four rows are. `f32` does not explain it either; only the fixed-point pipeline does.
///
/// Rounding per-edge rather than per-dimension also matters: the widths alone fit no clean law,
/// because they are differences of two independently rounded edges.
pub fn layout_bounds(
    family: MusicFamily,
    shape: &str,
    staff_interline: i32,
) -> Result<Option<Bounds>, FontError> {
    layout_bounds_at_point_size(family, shape, point_size(staff_interline))
}

/// Ports `TextLayout.getBounds()` for a single-glyph music symbol at an integer point size.
///
/// Most recognition callers start from a staff interline and should use [`layout_bounds`].
/// `BlackHeadSizer` is the exception: Java's `MusicFont.computePointSize` samples the black
/// notehead at two arbitrary integer point sizes before secant interpolation, so restricting this
/// operation to `4 * interline` would skip observable sizes.
pub fn layout_bounds_at_point_size(
    family: MusicFamily,
    shape: &str,
    point_size: i32,
) -> Result<Option<Bounds>, FontError> {
    let Some(codepoint) = codepoint(family, shape) else {
        return Ok(None);
    };
    let data = font_bytes(family);
    let sfnt = Sfnt::parse(data)?;
    let units_per_em = i64::from(sfnt.units_per_em()?);
    let Some(glyph) = sfnt.glyph_index(codepoint)? else {
        return Ok(None);
    };
    let table = sfnt
        .table(b"CFF ")
        .ok_or(sfnt::FontError::MissingTable("CFF "))?;
    let Some(box_) = Cff::parse(table)?.outline(glyph)?.bounds() else {
        return Ok(None);
    };

    let scale = div_fix(i64::from(point_size) * 64, units_per_em);
    // Every edge of every clef box lands on an on-curve point, so all six are whole font units.
    // A fractional edge would mean a curve *interior* sets the box, and then the order of
    // operations becomes observable — Java's scaler quantizes points to 26.6 first and solves for
    // extrema in that space, which is not the same as solving in font units and scaling after.
    // Nothing in the swept oracle exercises that, so refuse it rather than pick an order and hope.
    let units = |value: f64| -> Result<i64, FontError> {
        if value.fract() == 0.0 {
            Ok(value as i64)
        } else {
            Err(FontError::UngradedOutline)
        }
    };
    let left = mul_fix(units(box_.min_x)?, scale);
    let right = mul_fix(units(box_.max_x)?, scale);
    // Font y points up, layout y points down, so the outline's max becomes the box's top.
    let top = -mul_fix(units(box_.max_y)?, scale);
    let bottom = -mul_fix(units(box_.min_y)?, scale);

    let grid = |value: i64| value as f64 / 64.0;
    Ok(Some(Bounds {
        x: grid(left),
        y: grid(top),
        width: grid(right - left),
        height: grid(bottom - top),
    }))
}

/// The width Java's `TextLayout.getBounds()` reports for Bravura's `NOTEHEAD_BLACK` at one
/// integer point size.
///
/// This is the exact metric sampled by `MusicFont.computePointSize`; no glyph rasterisation is
/// involved. The function remains family-parametric because Java stores the selected family in
/// `Scale.MusicFontScale`, even though Bravura is currently the only ported family.
pub fn black_head_layout_width(
    family: MusicFamily,
    point_size: i32,
) -> Result<Option<f64>, FontError> {
    Ok(layout_bounds_at_point_size(family, "NOTEHEAD_BLACK", point_size)?.map(|box_| box_.width))
}

/// Ports Java `MusicFont.computePointSize`, including both `Math.rint` calls, the direction of
/// the second sample, and the near-zero secant fallback.
///
/// The input is `BlackHeadScale.widthMean`, so production calls are finite and non-negative.
/// The optional result preserves the font-family fallback boundary: a family without a black-head
/// codepoint reports absence rather than silently deriving a size from another glyph.
pub fn black_head_point_size(family: MusicFamily, width: f64) -> Result<Option<i32>, FontError> {
    // Very rough first value, exactly as Java computes it.
    let v1 = java_rint_to_int(width * 3.3);
    let Some(w1) = black_head_layout_width(family, v1)? else {
        return Ok(None);
    };

    // A second point far enough away to make the secant useful. `dv == 0` is observable for
    // tiny widths and deliberately falls through to Java's `abs(dw) < 0.01` branch below.
    let dv = java_rint_to_int(width * 0.25);
    let v2 = if w1 < width { v1 + dv } else { v1 - dv };
    let Some(w2) = black_head_layout_width(family, v2)? else {
        return Ok(None);
    };

    let dw = w2 - w1;
    let point_size = if dw.abs() < 0.01 {
        v1
    } else {
        java_rint_to_int(f64::from(v1) + f64::from(v2 - v1) * ((width - w1) / (w2 - w1)))
    };
    Ok(Some(point_size))
}

/// Ports `MusicFont.getHeadPointSize` without coupling this metric crate to the sheet model.
///
/// `music_font_point_size` is the sheet-wide value produced by [`black_head_point_size`]. When it
/// is present, Java scales it by this staff's interline relative to the sheet interline and rounds
/// once. When absent, Java's current `headRatio` is exactly `1.0`, then `getPointSize` multiplies
/// the rounded staff interline by four.
#[must_use]
pub fn head_point_size(
    music_font_point_size: Option<i32>,
    sheet_interline: i32,
    staff_interline: f64,
) -> i32 {
    match music_font_point_size {
        Some(point_size) => {
            java_rint_to_int((staff_interline / f64::from(sheet_interline)) * f64::from(point_size))
        }
        None => point_size(java_rint_to_int(staff_interline)),
    }
}

/// FreeType's `FT_DivFix`: `a / b` as a 16.16 fixed-point value, rounded to nearest.
///
/// Used once per size to turn a 26.6 point size and the font's units-per-em into the scale factor
/// [`mul_fix`] applies. Both arguments are positive here.
fn div_fix(a: i64, b: i64) -> i64 {
    (a * 65536 + b / 2) / b
}

/// FreeType's `FT_MulFix`: `a * b / 65536`, rounded half away from zero.
///
/// Rounding is applied to the magnitude and the sign restored, which is what makes a coordinate
/// and its negation round symmetrically — the reason this must be applied to the font-space value
/// *before* the y flip rather than after.
fn mul_fix(a: i64, b: i64) -> i64 {
    let sign = if (a < 0) != (b < 0) { -1 } else { 1 };
    sign * ((a.abs() * b.abs() + 0x8000) >> 16)
}

/// The point size Java measures its pitch offsets at, via `MusicFont.getMusicFont(family, 200)`.
///
/// Java calls it "arbitrary", and it is -- but it is also load-bearing, because the offset is a
/// ratio of two quantized boxes and the quantization does not cancel. Measuring at a different
/// size gives a slightly different constant.
const PITCH_OFFSET_INTERLINE: i32 = 200 / 4;

/// Java `AbstractPitchedInter.getAreaPitchOffset`: a shape's pitch delta from area centre to focus
/// line.
///
/// Derived from the font rather than tabulated, exactly as Java derives it:
///
/// 1. one pitch step is `(five-line staff height - one-line height) / 8`;
/// 2. the shape's offset is `(-box.y - box.height / 2) / pitch step`.
///
/// Returns 0 for any shape Java does not populate the map for, which is its `default -> 0` branch
/// rather than an absence — a shape with no entry genuinely has no offset.
pub fn area_pitch_offset(family: MusicFamily, shape: &str) -> Result<f64, FontError> {
    let offset_shapes = matches!(
        shape,
        "G_CLEF" | "G_CLEF_8VA" | "G_CLEF_8VB" | "F_CLEF" | "FLAT"
    );
    if !offset_shapes {
        return Ok(0.0);
    }
    let height = |shape: &str| -> Result<f64, FontError> {
        Ok(layout_bounds(family, shape, PITCH_OFFSET_INTERLINE)?
            .map_or(0.0, |bounds| bounds.height))
    };
    let staff = height("STAFF_FIVE_LINES")?;
    let line = height("STAFF_LINE")?;
    // Four interlines span eight pitch steps.
    let pitch_height = (staff - line) / 8.0;
    let Some(bounds) = layout_bounds(family, shape, PITCH_OFFSET_INTERLINE)? else {
        return Ok(0.0);
    };
    Ok((-bounds.y - bounds.height / 2.0) / pitch_height)
}

/// Java `AlterInter.constants.flatMassPitchOffset`, the heuristic applied to a flat's mass pitch.
pub const FLAT_MASS_PITCH_OFFSET: f64 = 0.65;

/// Java `MusicFont.getStaffInterline`: the interline a font of a given point size "belongs to".
///
/// Note the `+ 2`: Java computes `rint((size + 2) / 4.0)`, not `size / 4`. For the usual
/// `size = 4 * interline` the two agree only because `rint((4i + 2)/4) = rint(i + 0.5)` lands on
/// ties — and ties go to even, so odd interlines round *up* through the tie and even ones stay.
/// At point size 84 (interline 21) this gives 22, not 21, and the num/den gap inherits the
/// difference. Faithful, not tidy.
#[must_use]
pub fn staff_interline_of_point_size(point_size: i32) -> i32 {
    (f64::from(point_size + 2) / 4.0).round_ties_even() as i32
}

/// Java `NumDenSymbol.getParams` + `ShapeSymbol.getDimension`: the drawn size of a stacked
/// numerator-over-denominator time signature at a staff interline.
///
/// The two digit layouts are measured with the ported [`layout_bounds`]; the vertical gap between
/// their centres is `2 * getStaffInterline(font)`; and `getDimension` `rint`s the *raw* composite
/// rectangle, so the rounding happens once at the end, not per digit.
///
/// Only single-digit numerator and denominator are supported: `layoutNumberByCode` builds a
/// multi-codepoint string for numbers >= 10, whose bounds need glyph *advances* — swept and graded
/// nowhere yet. [`FontError::UnsupportedNumber`] keeps that an explicit gap rather than a wrong
/// box.
pub fn num_den_dimension(
    family: MusicFamily,
    numerator: i32,
    denominator: i32,
    staff_interline: i32,
) -> Result<(i32, i32), FontError> {
    let digit_shape = |digit: i32| -> Result<&'static str, FontError> {
        match digit {
            0 => Ok("TIME_ZERO"),
            1 => Ok("TIME_ONE"),
            2 => Ok("TIME_TWO"),
            3 => Ok("TIME_THREE"),
            4 => Ok("TIME_FOUR"),
            5 => Ok("TIME_FIVE"),
            6 => Ok("TIME_SIX"),
            7 => Ok("TIME_SEVEN"),
            8 => Ok("TIME_EIGHT"),
            9 => Ok("TIME_NINE"),
            _ => Err(FontError::UnsupportedNumber),
        }
    };
    let bounds_of = |digit: i32| -> Result<Bounds, FontError> {
        layout_bounds(family, digit_shape(digit)?, staff_interline)?
            .ok_or(FontError::MissingTable("digit glyph"))
    };
    let numerator = bounds_of(numerator)?;
    let denominator = bounds_of(denominator)?;
    let dy = 2 * staff_interline_of_point_size(point_size(staff_interline));
    let width = numerator.width.max(denominator.width).round_ties_even() as i32;
    let height =
        (f64::from(dy) + numerator.height.max(denominator.height)).round_ties_even() as i32;
    Ok((width, height))
}

/// Java's `(int) Math.rint(value)`.
///
/// `Math.rint` is ties-to-even, matching `f64::round_ties_even`. The narrowing cast then clamps to
/// `Integer.MIN_VALUE`/`MAX_VALUE` and maps NaN to zero, which is exactly what Rust's saturating
/// `as` conversion does — so this is one operation in each language, not a special case.
fn java_rint_to_int(value: f64) -> i32 {
    value.round_ties_even() as i32
}

/// The `MusicFontScout` capture, parsed into `key -> value` pairs.
///
/// Only tests use this: the pinned constants above are the production path, and this is what
/// grades them.
#[cfg(test)]
fn oracle() -> HashMap<String, String> {
    include_str!("../../../oracle/music-font.txt")
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every shape the scout sweeps, which is every single-glyph shape the header stages need.
    const SHAPES: [&str; 14] = [
        "F_CLEF",
        "G_CLEF",
        "G_CLEF_8VA",
        "G_CLEF_8VB",
        "C_CLEF",
        "PERCUSSION_CLEF",
        "FLAT",
        "NATURAL",
        "SHARP",
        "COMMON_TIME",
        "CUT_TIME",
        "TIME_TWO",
        "TIME_THREE",
        "TIME_FOUR",
    ];

    #[test]
    fn pinned_offsets_are_the_captured_java_values_to_the_bit() {
        // The whole justification for hard-coding these is that they came off the live JVM, so the
        // test compares bits rather than a tolerance: `Double.toString` and Rust's `f64` parser
        // both guarantee shortest-round-trip, so an exact match is available and anything less
        // would mean a transcription error had been rounded into invisibility.
        let oracle = oracle();
        for shape in SHAPES {
            let row = oracle
                .get(&format!("musicfont.offset.{shape}"))
                .unwrap_or_else(|| panic!("oracle has no offset for {shape}"));
            let (x, y) = row.split_once(' ').expect("offset row is 'x y'");
            let expected = CentroidOffset {
                x: x.parse().expect("finite x"),
                y: y.parse().expect("finite y"),
            };
            let pinned = centroid_offset(MusicFamily::Bravura, shape)
                .unwrap_or_else(|| panic!("no pinned offset for {shape}"));
            assert_eq!(
                pinned.x.to_bits(),
                expected.x.to_bits(),
                "{shape} x: pinned {pinned:?} vs oracle {expected:?}"
            );
            assert_eq!(
                pinned.y.to_bits(),
                expected.y.to_bits(),
                "{shape} y: pinned {pinned:?} vs oracle {expected:?}"
            );
        }
    }

    #[test]
    fn the_oracle_was_captured_under_the_manifest_jdk_and_the_default_family() {
        // A capture from the wrong runtime would still parse and still compare equal to whatever
        // was pinned from it, so provenance is asserted rather than assumed.
        let oracle = oracle();
        assert_eq!(
            oracle.get("musicfont.runtime").map(String::as_str),
            Some("OpenJDK 64-Bit Server VM 25.0.3+9-LTS")
        );
        assert_eq!(
            oracle.get("musicfont.family").map(String::as_str),
            Some("Bravura")
        );
        assert_eq!(
            oracle
                .get("musicfont.standard-interline")
                .map(String::as_str),
            Some(STANDARD_INTERLINE.to_string().as_str())
        );
    }

    #[test]
    fn the_rendered_symbols_really_are_antialiased() {
        // This is why the table is pinned instead of computed, so it is asserted rather than left
        // as a claim in a doc comment: if a future capture ever reports two-valued alpha, the
        // rasteriser stops being the obstacle and this decision should be revisited.
        let oracle = oracle();
        for shape in SHAPES {
            let row = oracle
                .get(&format!("musicfont.coverage.{shape}"))
                .unwrap_or_else(|| panic!("oracle has no coverage for {shape}"));
            assert!(
                row.ends_with("opaque=false"),
                "{shape} coverage is binary after all: {row}"
            );
        }
    }

    #[test]
    fn point_size_is_four_interlines() {
        let oracle = oracle();
        let ratio: i32 = oracle["musicfont.pointsize-per-interline"]
            .parse()
            .expect("integer ratio");
        assert_eq!(point_size(1), ratio);
        assert_eq!(point_size(STANDARD_INTERLINE), 80);
    }

    #[test]
    fn symbol_centroid_matches_java_rint_of_the_offset_box() {
        // Worked by hand from the pinned G_CLEF offset rather than from this implementation:
        //   centerX = 100 + 54/2 = 127.0;  127.0 + 54 * 0.0020572508284535385 = 127.1110915...
        //   centerY =  40 + 140/2 = 110.0; 110.0 + 140 * 0.018883063668162947 = 112.6436289...
        let centroid = symbol_centroid(
            MusicFamily::Bravura,
            "G_CLEF",
            Rectangle {
                x: 100,
                y: 40,
                width: 54,
                height: 140,
            },
        );
        assert_eq!(centroid, Some((127, 113)));
    }

    #[test]
    fn a_half_way_centroid_rounds_to_even_as_java_rint_does() {
        // `Math.rint` is ties-to-even, not ties-away, and the difference is reachable here: a zero
        // offset puts the centroid exactly on `centerX`, which is a half-integer for odd widths.
        // Widths 55 and 57 straddle the two directions ties-to-even can go.
        let offset = centroid_offset(MusicFamily::Bravura, "G_CLEF").expect("pinned");
        assert!(offset.x.abs() < 0.005, "test assumes a near-zero offset");
        assert_eq!(java_rint_to_int(126.5), 126);
        assert_eq!(java_rint_to_int(127.5), 128);
        assert_eq!(java_rint_to_int(-126.5), -126);
    }

    #[test]
    fn layout_bounds_match_the_java_sweep_on_every_shape_and_size() {
        // The end-to-end grade: 6 shapes x 116 interlines x 4 numbers, every one from the live
        // JVM. Nothing intermediate is pinned, so this simultaneously grades the cmap lookup, the
        // Type 2 interpreter, the exact-extrema box and the 1/64 quantization -- if the outline
        // were the control box instead of the true box, the curved clefs would miss here.
        let oracle = oracle();
        let mut checked = 0;
        for shape in SHAPES {
            for interline in 5..=120 {
                let Some(row) = oracle.get(&format!("musicfont.bounds.{shape}.{interline}")) else {
                    continue;
                };
                let expected: Vec<f64> = row
                    .split(' ')
                    .map(|value| value.parse().expect("finite bound"))
                    .collect();
                let actual = layout_bounds(MusicFamily::Bravura, shape, interline)
                    .expect("Bravura parses")
                    .expect("clef has an outline");
                assert_eq!(
                    [actual.x, actual.y, actual.width, actual.height],
                    [expected[0], expected[1], expected[2], expected[3]],
                    "{shape} at interline {interline}"
                );
                checked += 1;
            }
        }
        // Guards against the whole loop silently grading nothing, which a key-format slip would
        // otherwise turn into a green run.
        assert_eq!(checked, SHAPES.len() * 116, "swept rows actually compared");
    }

    #[test]
    fn the_font_is_the_one_the_oracle_was_captured_from() {
        let sfnt = Sfnt::parse(font_bytes(MusicFamily::Bravura)).expect("Bravura parses");
        assert_eq!(sfnt.units_per_em().expect("head table"), 1000);
        assert_eq!(
            sfnt.glyph_index(0xE050).expect("cmap is readable"),
            sfnt.glyph_index(u32::from('\u{E050}'))
                .expect("same lookup")
        );
        assert!(sfnt.glyph_index(0xE050).expect("cmap").is_some(), "G clef");
        // A codepoint outside SMuFL's private-use block must report absence, not glyph 0.
        assert_eq!(sfnt.glyph_index(0x0041_0000).ok(), None);
    }

    #[test]
    fn unknown_shapes_and_families_report_absence_rather_than_a_default() {
        assert_eq!(
            centroid_offset(MusicFamily::Bravura, "NOTEHEAD_BLACK"),
            None
        );
        assert_eq!(
            symbol_centroid(
                MusicFamily::Bravura,
                "NOTEHEAD_BLACK",
                Rectangle {
                    x: 0,
                    y: 0,
                    width: 10,
                    height: 10,
                },
            ),
            None
        );
    }
}

#[cfg(test)]
mod num_den_tests {
    use super::*;

    #[test]
    fn num_den_dimension_matches_the_corpus_observed_time_box() {
        // Cross-checked three ways: the digit boxes come from the 116-size sweep (graded), the
        // gap is 2 * rint((84 + 2) / 4) = 44 -- note the `+ 2` and the tie at 21.5 going to the
        // even 22 -- and the result equals the (36, 87) box Java's HEADERS stores for every
        // TIME_TWO_FOUR staff on the corpus.
        assert_eq!(staff_interline_of_point_size(84), 22, "rint(21.5) is even");
        assert_eq!(
            staff_interline_of_point_size(80),
            20,
            "rint(20.5) goes to the even 20"
        );
        assert_eq!(
            num_den_dimension(MusicFamily::Bravura, 2, 4, 21).expect("digits exist"),
            (36, 87)
        );
        assert_eq!(
            num_den_dimension(MusicFamily::Bravura, 12, 8, 21),
            Err(FontError::UnsupportedNumber),
            "multi-digit numbers refuse rather than guess"
        );
    }
}

#[cfg(test)]
mod pitch_offset_tests {
    use super::*;

    #[test]
    fn asymmetric_area_pitch_offsets_are_derived_from_the_font() {
        // Java measures this at point size 200 and uses it to correct a flat's pitch, which is why
        // every flat key on the corpus was rejected while sharps passed: a flat's area centre sits
        // well below its focus line, and without the correction the measured pitch misses the
        // expected one by more than the 0.5 tolerance a single alteration allows.
        let offset = area_pitch_offset(MusicFamily::Bravura, "FLAT").expect("Bravura parses");
        assert!(
            offset > 0.3 && offset < 1.5,
            "flat offset {offset} should be a fraction of a pitch step, not zero or huge"
        );
        let f_clef =
            area_pitch_offset(MusicFamily::Bravura, "F_CLEF").expect("Bravura F clef parses");
        assert!(
            f_clef < -0.5 && f_clef > -2.0,
            "F-clef focus offset {f_clef} should lift the reference pitch by about one step"
        );
        // Sharps and naturals are not in Java's map at all: their area centre *is* their focus.
        assert_eq!(
            area_pitch_offset(MusicFamily::Bravura, "SHARP").expect("Bravura parses"),
            0.0
        );
        assert_eq!(
            area_pitch_offset(MusicFamily::Bravura, "NATURAL").expect("Bravura parses"),
            0.0
        );
    }
}

#[cfg(test)]
mod black_head_size_tests {
    use super::*;

    #[test]
    fn black_head_width_matches_java_at_arbitrary_point_sizes() {
        // Captured directly from Temurin 25.0.3+9-LTS `TextLayout.getBounds()` with Bravura's
        // NOTEHEAD_BLACK U+E0A4. These sizes are deliberately not all divisible by four:
        // `computePointSize` samples arbitrary integer sizes, unlike ordinary interline layouts.
        let rows = [
            (0, 0.0),
            (1, 0.296875),
            (4, 1.1875),
            (52, 15.34375),
            (53, 15.640625),
            (54, 15.9375),
            (55, 16.21875),
            (56, 16.515625),
            (57, 16.8125),
            (58, 17.109375),
            (59, 17.40625),
            (60, 17.703125),
            (61, 18.0),
            (62, 18.296875),
            (63, 18.578125),
            (64, 18.875),
            (100, 29.5),
        ];
        for (point_size, expected) in rows {
            assert_eq!(
                black_head_layout_width(MusicFamily::Bravura, point_size).expect("Bravura parses"),
                Some(expected),
                "point size {point_size}"
            );
        }
    }

    #[test]
    fn black_head_point_size_matches_java_two_point_interpolation() {
        // Same pinned-JDK capture, this time through the complete Java `computePointSize` body.
        // The tiny rows cover its `abs(dw) < 0.01` fallback; the corpus-sized rows exercise
        // independent `rint`s for v1/dv and the final secant result.
        let rows = [
            (0.0, 0),
            (0.125, 0),
            (0.5, 2),
            (4.0, 14),
            (8.0, 27),
            (10.0, 34),
            (15.0, 51),
            (15.5, 53),
            (16.0, 54),
            (16.5, 56),
            (17.0, 58),
            (17.25, 58),
            (17.5, 59),
            (18.0, 61),
            (18.25, 62),
            (18.5, 63),
            (19.0, 64),
            (20.0, 68),
            (21.0, 71),
            (22.0, 75),
            (24.0, 81),
            (32.0, 108),
        ];
        for (width, expected) in rows {
            assert_eq!(
                black_head_point_size(MusicFamily::Bravura, width).expect("Bravura parses"),
                Some(expected),
                "measured width {width}"
            );
        }
    }

    #[test]
    fn per_staff_head_size_uses_java_rounding_and_fallback() {
        assert_eq!(head_point_size(Some(61), 20, 20.0), 61);
        assert_eq!(
            head_point_size(Some(61), 20, 10.0),
            30,
            "30.5 rounds to the even 30"
        );
        assert_eq!(
            head_point_size(Some(61), 20, 30.0),
            92,
            "91.5 rounds to the even 92"
        );
        assert_eq!(
            head_point_size(None, 20, 20.5),
            80,
            "fallback rounds staff interline before multiplying by four"
        );
        assert_eq!(head_point_size(None, 20, 21.5), 88);
    }
}
