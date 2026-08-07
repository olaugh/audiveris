// SPDX-License-Identifier: AGPL-3.0-or-later

//! The production [`HeaderTimeShapeClassifier`]: Java's `WholeAdapter` / `HalfAdapter`
//! `evaluateGlyph` bodies over the bundled network, plus the symbol-bounds arithmetic the
//! selected candidate feeds `staff.setTimeStop` with.
//!
//! Unlike the key stage — whose classifier reads the *sheet* interline — the time adapters hand
//! the classifier `staff.getSpecificInterline()` (HeaderTimeBuilder, both adapters), the same
//! choice the clef stage makes. The three stages genuinely differ and each Rust seam records its
//! own.
//!
//! The symbol bounds have two constructions, mirroring `TimeWholeInter.getSymbolBounds`:
//!
//! - `SingleWholeTimes` (COMMON_TIME, CUT_TIME) go through `AbstractInter.getSymbolBounds` — the
//!   font box centred on the glyph's area centre, half-extents `rint`ed *separately* from the
//!   dimensions, precisely the clef-stage subtlety again;
//! - num/den composites use `NumDenSymbol`'s dimension centred with **integer-division** halves —
//!   `dim.width / 2`, not `rint(width / 2.0)` — because Java builds a `Dimension` first and halves
//!   the `int`.

use audiveris_classifier::{
    BasicClassifier, Evaluation, FeatureExtractionError, ModelLoadError,
    mix_glyph_features_from_run_table, rank_evaluations,
};
use audiveris_music_font::sfnt::FontError;
use audiveris_music_font::{MusicFamily, layout_bounds, num_den_dimension};

use crate::header_time_builder::{
    HeaderTimeGlyph, HeaderTimeGlyphKind, HeaderTimeShapeClassifier, HeaderTimeShapeEvaluation,
};
use crate::header_time_column::{NeutralSpecificTimeShape, NeutralTimeValue};
use crate::staff_header::HeaderBounds;

/// Java `AbstractClassifier.constants.minWeight`.
const MINIMUM_NORMALIZED_WEIGHT: f64 = 0.04;

/// A failure while classifying a time candidate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TimeClassificationError {
    /// The bundled classifier archive could not be read.
    Model(ModelLoadError),
    /// The glyph could not produce a descriptor.
    Features(FeatureExtractionError),
    /// The music font could not produce a ranked shape's box.
    ///
    /// Loud on purpose: this fires for shapes whose box needs ungraded machinery (multi-digit
    /// numbers), and a silent skip would change how rank slots are consumed relative to Java.
    Font(FontError),
}

impl core::fmt::Display for TimeClassificationError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Model(error) => write!(formatter, "classifier model: {error}"),
            Self::Features(error) => write!(formatter, "glyph descriptor: {error}"),
            Self::Font(error) => write!(formatter, "music font: {error}"),
        }
    }
}

impl std::error::Error for TimeClassificationError {}

/// Audiveris' shipped `BasicClassifier`, filtered to the time shape sets.
#[derive(Debug)]
pub struct BundledTimeClassifier {
    model: BasicClassifier,
    family: MusicFamily,
}

impl BundledTimeClassifier {
    /// Loads the bundled model.
    pub fn bundled() -> Result<Self, TimeClassificationError> {
        Ok(Self {
            model: BasicClassifier::bundled().map_err(TimeClassificationError::Model)?,
            family: MusicFamily::Bravura,
        })
    }
}

/// `ShapeSet.WholeTimes` label -> `(specific, numerator, denominator)`.
fn whole_value(shape: &str) -> Option<NeutralTimeValue> {
    let (specific, numerator, denominator) = match shape {
        "COMMON_TIME" => (Some(NeutralSpecificTimeShape::Common), 4, 4),
        "CUT_TIME" => (Some(NeutralSpecificTimeShape::Cut), 2, 2),
        "TIME_FOUR_FOUR" => (None, 4, 4),
        "TIME_TWO_TWO" => (None, 2, 2),
        "TIME_TWO_FOUR" => (None, 2, 4),
        "TIME_THREE_FOUR" => (None, 3, 4),
        "TIME_FIVE_FOUR" => (None, 5, 4),
        "TIME_SIX_FOUR" => (None, 6, 4),
        "TIME_THREE_EIGHT" => (None, 3, 8),
        "TIME_SIX_EIGHT" => (None, 6, 8),
        "TIME_TWELVE_EIGHT" => (None, 12, 8),
        _ => return None,
    };
    Some(NeutralTimeValue {
        specific_shape: specific,
        numerator,
        denominator,
    })
}

/// `ShapeSet.PartialTimes` label -> the number it denotes.
fn partial_value(shape: &str) -> Option<i32> {
    match shape {
        "TIME_TWO" => Some(2),
        "TIME_THREE" => Some(3),
        "TIME_FOUR" => Some(4),
        "TIME_FIVE" => Some(5),
        "TIME_SIX" => Some(6),
        "TIME_SEVEN" => Some(7),
        "TIME_EIGHT" => Some(8),
        "TIME_NINE" => Some(9),
        "TIME_TWELVE" => Some(12),
        "TIME_SIXTEEN" => Some(16),
        _ => None,
    }
}

/// The digit shape naming a single-digit number.
fn digit_shape(number: i32) -> Option<&'static str> {
    match number {
        2 => Some("TIME_TWO"),
        3 => Some("TIME_THREE"),
        4 => Some("TIME_FOUR"),
        5 => Some("TIME_FIVE"),
        6 => Some("TIME_SIX"),
        7 => Some("TIME_SEVEN"),
        8 => Some("TIME_EIGHT"),
        9 => Some("TIME_NINE"),
        _ => None,
    }
}

/// Java's `(int) Math.rint(value)`.
fn java_rint(value: f64) -> i32 {
    value.round_ties_even() as i32
}

/// `AbstractInter.getSymbolBounds`: the font box centred on the glyph's area centre.
fn single_symbol_bounds(
    family: MusicFamily,
    shape: &str,
    staff_interline: i32,
    glyph: HeaderBounds,
) -> Result<HeaderBounds, TimeClassificationError> {
    let bounds = layout_bounds(family, shape, staff_interline)
        .map_err(TimeClassificationError::Font)?
        .ok_or(TimeClassificationError::Font(FontError::MissingTable(
            "time glyph",
        )))?;
    let center_x = glyph.x + glyph.width / 2;
    let center_y = glyph.y + glyph.height / 2;
    Ok(HeaderBounds {
        x: center_x - java_rint(bounds.width / 2.0),
        y: center_y - java_rint(bounds.height / 2.0),
        width: java_rint(bounds.width),
        height: java_rint(bounds.height),
    })
}

/// `TimeWholeInter.getSymbolBounds`, num/den branch: the composite dimension centred with
/// integer-division halves.
fn num_den_symbol_bounds(
    family: MusicFamily,
    value: NeutralTimeValue,
    staff_interline: i32,
    glyph: HeaderBounds,
) -> Result<HeaderBounds, TimeClassificationError> {
    let (width, height) =
        num_den_dimension(family, value.numerator, value.denominator, staff_interline)
            .map_err(TimeClassificationError::Font)?;
    let center_x = glyph.x + glyph.width / 2;
    let center_y = glyph.y + glyph.height / 2;
    Ok(HeaderBounds {
        x: center_x - width / 2,
        y: center_y - height / 2,
        width,
        height,
    })
}

impl HeaderTimeShapeClassifier for BundledTimeClassifier {
    type Error = TimeClassificationError;

    fn evaluate(
        &mut self,
        glyph: &HeaderTimeGlyph,
        kind: HeaderTimeGlyphKind,
        staff_interline: i32,
        maximum_rank: usize,
        minimum_grade: f64,
    ) -> Result<Vec<HeaderTimeShapeEvaluation>, Self::Error> {
        // Java's pre-inference noise gate: below the threshold the classifier answers NOISE,
        // which neither time shape set contains.
        let normalized = glyph.weight as f64 / f64::from(staff_interline * staff_interline);
        if normalized < MINIMUM_NORMALIZED_WEIGHT {
            return Ok(Vec::new());
        }

        let features = mix_glyph_features_from_run_table(
            &glyph.raster,
            (glyph.bounds.x, glyph.bounds.y),
            staff_interline,
        )
        .map_err(TimeClassificationError::Features)?;

        // Rank over all 149 shapes before any time filter, so `maximum_rank` counts every shape
        // the network preferred, exactly as in the clef and key stages.
        let candidates = self
            .model
            .evaluate(&features)
            .into_iter()
            .map(|grade| Evaluation::new(grade.shape, grade.grade))
            .collect();
        let ranked = rank_evaluations(candidates, maximum_rank, minimum_grade, None);

        let mut evaluations = Vec::new();
        for evaluation in ranked {
            let produced = match kind {
                HeaderTimeGlyphKind::Whole => match whole_value(&evaluation.shape) {
                    Some(value) => {
                        let symbol_bounds = if value.specific_shape.is_some() {
                            single_symbol_bounds(
                                self.family,
                                &evaluation.shape,
                                staff_interline,
                                glyph.bounds,
                            )?
                        } else {
                            num_den_symbol_bounds(
                                self.family,
                                value,
                                staff_interline,
                                glyph.bounds,
                            )?
                        };
                        HeaderTimeShapeEvaluation::Whole {
                            value,
                            grade: evaluation.grade,
                            symbol_bounds,
                        }
                    }
                    None => HeaderTimeShapeEvaluation::Other {
                        grade: evaluation.grade,
                    },
                },
                HeaderTimeGlyphKind::Numerator | HeaderTimeGlyphKind::Denominator => {
                    match partial_value(&evaluation.shape) {
                        Some(value) => {
                            let Some(shape) = digit_shape(value) else {
                                // TIME_TWELVE / TIME_SIXTEEN are two glyphs in one string; their
                                // boxes need advance composition that nothing grades yet.
                                return Err(TimeClassificationError::Font(
                                    FontError::UnsupportedNumber,
                                ));
                            };
                            let symbol_bounds = single_symbol_bounds(
                                self.family,
                                shape,
                                staff_interline,
                                glyph.bounds,
                            )?;
                            HeaderTimeShapeEvaluation::Number {
                                value,
                                grade: evaluation.grade,
                                symbol_bounds,
                            }
                        }
                        None => HeaderTimeShapeEvaluation::Other {
                            grade: evaluation.grade,
                        },
                    }
                }
            };
            evaluations.push(produced);
        }
        Ok(evaluations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use audiveris_image::run_table::{FOREGROUND, Orientation, RunTable};

    fn glyph(width: usize, height: usize, weight: usize) -> HeaderTimeGlyph {
        let mut pixels = vec![255u8; width * height];
        for pixel in pixels.iter_mut().take(weight) {
            *pixel = FOREGROUND;
        }
        HeaderTimeGlyph {
            id: 1,
            part_ids: vec![1],
            bounds: HeaderBounds {
                x: 100,
                y: 50,
                width: width as i32,
                height: height as i32,
            },
            weight,
            centroid_x: 0.0,
            centroid_y: 0.0,
            raster: RunTable::from_pixels(Orientation::Horizontal, width, height, &pixels)
                .expect("a well-formed raster"),
        }
    }

    #[test]
    fn shape_tables_cover_javas_sets_and_nothing_else() {
        assert_eq!(
            whole_value("COMMON_TIME"),
            Some(NeutralTimeValue {
                specific_shape: Some(NeutralSpecificTimeShape::Common),
                numerator: 4,
                denominator: 4,
            })
        );
        assert_eq!(
            whole_value("TIME_TWO_FOUR").map(|value| (value.numerator, value.denominator)),
            Some((2, 4))
        );
        assert_eq!(whole_value("TIME_TWO"), None, "a digit is not a whole");
        assert_eq!(partial_value("TIME_TWO"), Some(2));
        assert_eq!(partial_value("TIME_SIXTEEN"), Some(16));
        assert_eq!(partial_value("COMMON_TIME"), None);
        assert_eq!(
            digit_shape(12),
            None,
            "multi-digit numbers have no single shape"
        );
    }

    #[test]
    fn composite_symbol_bounds_use_integer_halves_and_the_corpus_dimension() {
        // Glyph area centre: (100 + 36/2, 50 + 88/2) = (118, 94). The 2/4 composite measures
        // (36, 87) at interline 21, and Java halves the *Dimension*: 36/2 = 18, 87/2 = 43 --
        // integer division, so the odd height loses its half downward, where `rint(43.5)` would
        // have gone to 44.
        let bounds = num_den_symbol_bounds(
            MusicFamily::Bravura,
            NeutralTimeValue {
                specific_shape: None,
                numerator: 2,
                denominator: 4,
            },
            21,
            HeaderBounds {
                x: 100,
                y: 50,
                width: 36,
                height: 88,
            },
        )
        .expect("digits exist");
        assert_eq!((bounds.width, bounds.height), (36, 87));
        assert_eq!((bounds.x, bounds.y), (118 - 18, 94 - 43));
    }

    #[test]
    fn a_glyph_below_the_noise_weight_is_never_shown_to_the_network() {
        let mut classifier = BundledTimeClassifier::bundled().expect("bundled model");
        assert_eq!(
            classifier.evaluate(&glyph(8, 8, 15), HeaderTimeGlyphKind::Whole, 20, 3, 0.05),
            Ok(Vec::new())
        );
        assert!(
            classifier
                .evaluate(&glyph(8, 8, 16), HeaderTimeGlyphKind::Whole, 20, 3, 0.05)
                .is_ok()
        );
    }
}
