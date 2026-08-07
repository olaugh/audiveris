// SPDX-License-Identifier: AGPL-3.0-or-later

//! The production [`ClefShapeClassifier`], joining the two crates HEADERS was waiting on.
//!
//! `clef_column` has always had the seam; what it lacked was anything to put behind it. This is
//! Java's `ClefBuilder.ClefAdapter.evaluateGlyph` together with the part of
//! `AbstractInter.getSymbolBounds` that turns a chosen shape into a box:
//!
//! - `audiveris-classifier` supplies the 110 `MixGlyphDescriptor` features, the bundled MLP, and
//!   Java's ranking policy;
//! - `audiveris-music-font` supplies `TextLayout.getBounds()` for the winning shape.

use audiveris_classifier::{
    BasicClassifier, Evaluation, FeatureExtractionError, ModelLoadError,
    mix_glyph_features_from_run_table, rank_evaluations,
};
use audiveris_music_font::sfnt::FontError;
use audiveris_music_font::{MusicFamily, layout_bounds};

use crate::clef_column::{
    ClefShapeClassifier, ClefShapeEvaluation, NativeClefGlyph, NeutralClefShape,
};
use crate::staff_header::HeaderBounds;

/// Java `AbstractClassifier.constants.minWeight`: the normalized weight below which a glyph is
/// classified as `NOISE` without the network being consulted at all.
const MINIMUM_NORMALIZED_WEIGHT: f64 = 0.04;

/// A failure while classifying a clef candidate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClefClassificationError {
    /// The bundled classifier archive could not be read.
    Model(ModelLoadError),
    /// The glyph could not produce a descriptor.
    Features(FeatureExtractionError),
    /// The music font could not produce the chosen shape's box.
    Font(FontError),
    /// A shape the classifier ranked has no music-font symbol.
    ///
    /// Java would throw here rather than place an unsized box, and so does this: a clef with no
    /// bounds silently loses its `clefStop`, which is the value the whole header depends on.
    MissingSymbol(&'static str),
}

impl core::fmt::Display for ClefClassificationError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Model(error) => write!(formatter, "classifier model: {error}"),
            Self::Features(error) => write!(formatter, "glyph descriptor: {error}"),
            Self::Font(error) => write!(formatter, "music font: {error}"),
            Self::MissingSymbol(shape) => write!(formatter, "no music symbol for {shape}"),
        }
    }
}

impl std::error::Error for ClefClassificationError {}

/// Audiveris' shipped `BasicClassifier`, restricted to the clef shapes HEADERS considers.
#[derive(Debug)]
pub struct BundledClefClassifier {
    model: BasicClassifier,
    family: MusicFamily,
    one_line_staff: bool,
}

impl BundledClefClassifier {
    /// Loads the bundled model for a normal five-line staff.
    ///
    /// The model is parsed once and reused: Java holds `BasicClassifier` as a singleton, and
    /// re-reading a 1.5 MB archive per glyph would be the difference between a step and a stall.
    pub fn bundled() -> Result<Self, ClefClassificationError> {
        Ok(Self {
            model: BasicClassifier::bundled().map_err(ClefClassificationError::Model)?,
            family: MusicFamily::Bravura,
            one_line_staff: false,
        })
    }

    /// Restricts candidates to `ClefBuilder.ONE_LINE_CLEF_SHAPES`, as Java does for a drum staff.
    #[must_use]
    pub fn for_one_line_staff(mut self) -> Self {
        self.one_line_staff = true;
        self
    }

    /// The allowed shape set, Java's `staff.isDrum() ? ONE_LINE_CLEF_SHAPES : HEADER_CLEF_SHAPES`.
    fn allows(&self, shape: &str) -> bool {
        if self.one_line_staff {
            shape == "PERCUSSION_CLEF"
        } else {
            neutral_shape(shape).is_some()
        }
    }
}

/// Maps a classifier output label onto the clef vocabulary `clef_column` speaks.
///
/// Returning `None` is how a non-clef shape is dropped, which is Java's
/// `clefShapes.contains(shape)` test.
fn neutral_shape(shape: &str) -> Option<NeutralClefShape> {
    match shape {
        "G_CLEF" => Some(NeutralClefShape::Treble),
        "G_CLEF_8VA" => Some(NeutralClefShape::TrebleOttavaAlta),
        "G_CLEF_8VB" => Some(NeutralClefShape::TrebleOttavaBassa),
        "F_CLEF" => Some(NeutralClefShape::Bass),
        "C_CLEF" => Some(NeutralClefShape::C),
        "PERCUSSION_CLEF" => Some(NeutralClefShape::Percussion),
        _ => None,
    }
}

/// Ports `AbstractInter.getSymbolBounds`: the font's box for a shape, centred on the glyph.
///
/// Java takes the *area centre* of the inter — whose bounds are the glyph's — with integer
/// division, then subtracts a separately rounded half-extent. The two roundings are independent:
/// `rint(w / 2)` is not `rint(w) / 2`, and using the latter moves the box by a pixel on odd
/// widths.
fn symbol_bounds(
    family: MusicFamily,
    shape: &str,
    staff_interline: i32,
    glyph: HeaderBounds,
) -> Result<HeaderBounds, ClefClassificationError> {
    let bounds = layout_bounds(family, shape, staff_interline)
        .map_err(ClefClassificationError::Font)?
        .ok_or(ClefClassificationError::MissingSymbol("clef shape"))?;
    let center_x = glyph.x + (glyph.width / 2);
    let center_y = glyph.y + (glyph.height / 2);
    Ok(HeaderBounds {
        x: center_x - java_rint(bounds.width / 2.0),
        y: center_y - java_rint(bounds.height / 2.0),
        width: java_rint(bounds.width),
        height: java_rint(bounds.height),
    })
}

/// Java's `(int) Math.rint(value)`.
fn java_rint(value: f64) -> i32 {
    value.round_ties_even() as i32
}

impl ClefShapeClassifier for BundledClefClassifier {
    type Error = ClefClassificationError;

    fn evaluate(
        &mut self,
        glyph: &NativeClefGlyph,
        staff_interline: i32,
        maximum_rank: usize,
        minimum_grade: f64,
    ) -> Result<Vec<ClefShapeEvaluation>, Self::Error> {
        // Java's noise gate, in `getSortedEvaluations`. Below the threshold it returns a single
        // `NOISE` evaluation instead of running the network -- and `NOISE` is not a clef shape, so
        // `evaluateGlyph` discards it. The observable result is no candidates, but the reason
        // matters: this is a short circuit before inference, not an empty ranking after it.
        let normalized = glyph.weight as f64 / f64::from(staff_interline * staff_interline);
        if normalized < MINIMUM_NORMALIZED_WEIGHT {
            return Ok(Vec::new());
        }

        let features = mix_glyph_features_from_run_table(
            &glyph.raster,
            (glyph.bounds.x, glyph.bounds.y),
            staff_interline,
        )
        .map_err(ClefClassificationError::Features)?;

        // Rank over *all* 149 shapes before filtering to clefs, which is the order Java uses:
        // `maximum_rank` counts every shape the network preferred, so a glyph whose top three are
        // note heads yields no clef even if a clef ranks fourth. Filtering first would quietly
        // raise the effective rank.
        let candidates = self
            .model
            .evaluate(&features)
            .into_iter()
            .map(|grade| Evaluation::new(grade.shape, grade.grade))
            .collect();
        let ranked = rank_evaluations(candidates, maximum_rank, minimum_grade, None);

        let mut evaluations = Vec::new();
        for evaluation in ranked {
            if !self.allows(&evaluation.shape) {
                continue;
            }
            let Some(shape) = neutral_shape(&evaluation.shape) else {
                continue;
            };
            evaluations.push(ClefShapeEvaluation {
                shape,
                grade: evaluation.grade,
                symbol_bounds: symbol_bounds(
                    self.family,
                    &evaluation.shape,
                    staff_interline,
                    glyph.bounds,
                )?,
            });
        }
        Ok(evaluations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use audiveris_image::run_table::RunTable;

    /// A glyph of `weight` foreground pixels in a `width x height` box at `origin`.
    ///
    /// `FOREGROUND` is 0, following Audiveris' convention that ink is black.
    fn glyph(origin: (i32, i32), width: usize, height: usize, weight: usize) -> NativeClefGlyph {
        let mut pixels = vec![255u8; width * height];
        for pixel in pixels.iter_mut().take(weight) {
            *pixel = audiveris_image::run_table::FOREGROUND;
        }
        let raster = RunTable::from_pixels(
            audiveris_image::run_table::Orientation::Horizontal,
            width,
            height,
            &pixels,
        )
        .expect("a well-formed raster");
        NativeClefGlyph {
            id: 1,
            part_ids: vec![1],
            bounds: HeaderBounds {
                x: origin.0,
                y: origin.1,
                width: width as i32,
                height: height as i32,
            },
            weight,
            centroid_x: f64::from(origin.0),
            centroid_y: f64::from(origin.1),
            raster,
        }
    }

    #[test]
    fn a_glyph_below_the_noise_weight_is_never_shown_to_the_network() {
        // interline 20 puts the threshold at 0.04 * 400 = 16 pixels.
        let mut classifier = BundledClefClassifier::bundled().expect("bundled model");
        let light = glyph((0, 0), 8, 8, 15);
        assert_eq!(
            classifier.evaluate(&light, 20, 5, 0.0375),
            Ok(Vec::new()),
            "15 pixels is below Java's 0.04 normalized weight"
        );
        // One pixel more and inference runs; whatever it decides, it is no longer short-circuited.
        let heavy = glyph((0, 0), 8, 8, 16);
        assert!(classifier.evaluate(&heavy, 20, 5, 0.0375).is_ok());
    }

    #[test]
    fn symbol_bounds_centre_the_font_box_on_the_glyph_with_java_rounding() {
        // G_CLEF at interline 20 measures 53.6875 x 140.484375, so the box is 54 x 140 and the
        // half-extents are rint(26.84375)=27 and rint(70.2421875)=70. Note 54/2 would give 27 too,
        // but 140/2 = 70 only agrees by luck -- the widths that matter are the odd ones, covered
        // below.
        let bounds = symbol_bounds(
            MusicFamily::Bravura,
            "G_CLEF",
            20,
            HeaderBounds {
                x: 100,
                y: 40,
                width: 40,
                height: 120,
            },
        )
        .expect("G clef has a symbol");
        assert_eq!(bounds.width, 54);
        assert_eq!(bounds.height, 140);
        assert_eq!(bounds.x, 120 - 27);
        assert_eq!(bounds.y, 100 - 70);
    }

    #[test]
    fn the_half_extent_is_rounded_separately_from_the_width() {
        // C_CLEF at interline 23 measures 64.3125 x 93.09375: width rints to 64 but the half
        // extent is rint(32.15625) = 32, and the height rints to 93 while rint(46.546875) = 47.
        // Halving the rounded height would give 46, one pixel off, which is the mistake this
        // guards.
        let bounds = symbol_bounds(
            MusicFamily::Bravura,
            "C_CLEF",
            23,
            HeaderBounds {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
        )
        .expect("C clef has a symbol");
        assert_eq!((bounds.width, bounds.height), (64, 93));
        assert_eq!((bounds.x, bounds.y), (-32, -47));
    }

    #[test]
    fn non_clef_shapes_are_dropped_and_drum_staves_keep_only_percussion() {
        assert_eq!(neutral_shape("NOTEHEAD_BLACK"), None);
        assert_eq!(neutral_shape("NOISE"), None);
        assert_eq!(neutral_shape("G_CLEF"), Some(NeutralClefShape::Treble));

        let normal = BundledClefClassifier::bundled().expect("bundled model");
        assert!(normal.allows("F_CLEF") && normal.allows("PERCUSSION_CLEF"));
        let drum = BundledClefClassifier::bundled()
            .expect("bundled model")
            .for_one_line_staff();
        assert!(!drum.allows("F_CLEF") && drum.allows("PERCUSSION_CLEF"));
    }
}
