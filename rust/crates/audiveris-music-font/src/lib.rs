// SPDX-License-Identifier: AGPL-3.0-or-later

//! Music-font metrics that Audiveris' recognition stages read back into the SIG.
//!
//! Two quantities from `MusicFont` reach a real decision, and they are not the same kind of
//! problem:
//!
//! 1. `ShapeSymbol.getCentroid`, whose per-shape offset is **pinned data** — see
//!    [`centroid_offset`] for why it cannot be computed here;
//! 2. `AbstractInter.getSymbolBounds`, which needs `TextLayout.getBounds()` — an outline box, and
//!    not yet ported.
//!
//! Everything here is graded against `rust/oracle/music-font.txt`, captured from the live JVM by
//! `MusicFontScout`.

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
