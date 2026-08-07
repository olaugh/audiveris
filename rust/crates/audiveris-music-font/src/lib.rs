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
/// Only the `ClefBuilder.HEADER_CLEF_SHAPES` set is present so far. Each is a single codepoint, so
/// a layout is one glyph and [`layout_bounds`] does not have to compose advances.
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

    let scale = div_fix(i64::from(point_size(staff_interline)) * 64, units_per_em);
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

    const CLEFS: [&str; 6] = [
        "F_CLEF",
        "G_CLEF",
        "G_CLEF_8VA",
        "G_CLEF_8VB",
        "C_CLEF",
        "PERCUSSION_CLEF",
    ];

    #[test]
    fn pinned_offsets_are_the_captured_java_values_to_the_bit() {
        // The whole justification for hard-coding these is that they came off the live JVM, so the
        // test compares bits rather than a tolerance: `Double.toString` and Rust's `f64` parser
        // both guarantee shortest-round-trip, so an exact match is available and anything less
        // would mean a transcription error had been rounded into invisibility.
        let oracle = oracle();
        for shape in CLEFS {
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
        for shape in CLEFS {
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
        for shape in CLEFS {
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
        assert_eq!(checked, 6 * 116, "swept rows actually compared");
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
