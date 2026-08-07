// SPDX-License-Identifier: AGPL-3.0-or-later

//! The production [`KeyShapeClassifier`], Java's `KeyExtractor.SingleAdapter.evaluateGlyph`.
//!
//! Simpler than its clef counterpart in one respect and different in another.
//!
//! Simpler: `KeyShapeEvaluation` carries no bounds, so this seam needs nothing from `MusicFont`.
//! `KeyBuilder` positions alterations from the slice geometry it already has, not from a font box.
//!
//! Different, and worth stating because it is easy to copy the wrong one across: `KeyExtractor`
//! passes **`sheet.getInterline()`** to the classifier, where `ClefBuilder` passes
//! `staff.getSpecificInterline()`. Both feed the same descriptor, so on a sheet with two staff
//! sizes the two stages normalize the *same glyph* differently. That is Java's behaviour, not an
//! oversight to tidy up, and the caller is responsible for handing this the sheet value.

use audiveris_classifier::{
    BasicClassifier, Evaluation, FeatureExtractionError, ModelLoadError,
    mix_glyph_features_from_run_table, rank_evaluations,
};

use crate::key_column::{
    KeyShapeClassifier, KeyShapeEvaluation, NativeKeyGlyph, NeutralKeyAlterShape,
};

/// Java `AbstractClassifier.constants.minWeight`.
const MINIMUM_NORMALIZED_WEIGHT: f64 = 0.04;

/// A failure while classifying a key alteration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyClassificationError {
    /// The bundled classifier archive could not be read.
    Model(ModelLoadError),
    /// The glyph could not produce a descriptor.
    Features(FeatureExtractionError),
}

impl core::fmt::Display for KeyClassificationError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Model(error) => write!(formatter, "classifier model: {error}"),
            Self::Features(error) => write!(formatter, "glyph descriptor: {error}"),
        }
    }
}

impl std::error::Error for KeyClassificationError {}

/// Audiveris' shipped `BasicClassifier`, restricted to the two alteration shapes a key is built
/// from.
#[derive(Debug)]
pub struct BundledKeyClassifier {
    model: BasicClassifier,
}

impl BundledKeyClassifier {
    /// Loads the bundled model.
    pub fn bundled() -> Result<Self, KeyClassificationError> {
        Ok(Self {
            model: BasicClassifier::bundled().map_err(KeyClassificationError::Model)?,
        })
    }
}

/// Maps a classifier label onto the alteration vocabulary `key_column` speaks.
///
/// `NATURAL` is deliberately **not** mapped. A key signature is built from sharps or from flats;
/// naturals appear only as a *cancel*, which `KeyBuilder` handles through its own path rather than
/// as an alteration of the key being read. Mapping it here would let a cancel be counted as a key
/// member.
fn alteration(shape: &str) -> Option<NeutralKeyAlterShape> {
    match shape {
        "SHARP" => Some(NeutralKeyAlterShape::Sharp),
        "FLAT" => Some(NeutralKeyAlterShape::Flat),
        _ => None,
    }
}

impl KeyShapeClassifier for BundledKeyClassifier {
    type Error = KeyClassificationError;

    fn evaluate(
        &mut self,
        glyph: &NativeKeyGlyph,
        interline: i32,
        maximum_rank: usize,
        minimum_grade: f64,
    ) -> Result<Vec<KeyShapeEvaluation>, Self::Error> {
        // Java's pre-inference noise gate, shared with the clef path: below the threshold
        // `getSortedEvaluations` returns a lone `NOISE` evaluation, which no target shape set
        // contains, so nothing survives the filter.
        let normalized = glyph.weight as f64 / f64::from(interline * interline);
        if normalized < MINIMUM_NORMALIZED_WEIGHT {
            return Ok(Vec::new());
        }

        let features = mix_glyph_features_from_run_table(
            &glyph.raster,
            (glyph.bounds.x, glyph.bounds.y),
            interline,
        )
        .map_err(KeyClassificationError::Features)?;

        // As for clefs, ranking runs over all 149 shapes *before* the alteration filter, so
        // `maximum_rank` counts every shape the network preferred.
        let candidates = self
            .model
            .evaluate(&features)
            .into_iter()
            .map(|grade| Evaluation::new(grade.shape, grade.grade))
            .collect();
        Ok(
            rank_evaluations(candidates, maximum_rank, minimum_grade, None)
                .into_iter()
                .filter_map(|evaluation| {
                    alteration(&evaluation.shape).map(|shape| KeyShapeEvaluation {
                        shape,
                        grade: evaluation.grade,
                    })
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::staff_header::HeaderBounds;
    use audiveris_image::run_table::{FOREGROUND, Orientation, RunTable};

    fn glyph(width: usize, height: usize, weight: usize) -> NativeKeyGlyph {
        let mut pixels = vec![255u8; width * height];
        for pixel in pixels.iter_mut().take(weight) {
            *pixel = FOREGROUND;
        }
        NativeKeyGlyph {
            id: 1,
            part_ids: vec![1],
            bounds: HeaderBounds {
                x: 0,
                y: 0,
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
    fn a_glyph_below_the_noise_weight_is_never_shown_to_the_network() {
        // interline 20 puts the threshold at 0.04 * 400 = 16 pixels.
        let mut classifier = BundledKeyClassifier::bundled().expect("bundled model");
        assert_eq!(
            classifier.evaluate(&glyph(8, 8, 15), 20, 5, 0.0125),
            Ok(Vec::new())
        );
        assert!(classifier.evaluate(&glyph(8, 8, 16), 20, 5, 0.0125).is_ok());
    }

    #[test]
    fn only_sharps_and_flats_are_alterations_and_naturals_are_not() {
        assert_eq!(alteration("SHARP"), Some(NeutralKeyAlterShape::Sharp));
        assert_eq!(alteration("FLAT"), Some(NeutralKeyAlterShape::Flat));
        // A natural cancels a key rather than belonging to one; letting it through here would let
        // a cancel be counted as a key member.
        assert_eq!(alteration("NATURAL"), None);
        assert_eq!(alteration("NOISE"), None);
        assert_eq!(alteration("G_CLEF"), None);
    }

    #[test]
    fn the_noise_gate_reads_the_interline_it_is_given() {
        // The same glyph passes at one interline and is refused at another, which is what makes
        // the sheet-versus-staff distinction observable rather than academic: 40 pixels is above
        // 0.04 * 20^2 = 16 but below 0.04 * 40^2 = 64.
        let mut classifier = BundledKeyClassifier::bundled().expect("bundled model");
        let sample = glyph(10, 10, 40);
        assert_eq!(
            classifier.evaluate(&sample, 40, 5, 0.0125),
            Ok(Vec::new()),
            "40 pixels is below 0.04 * 40^2 = 64, so the network is never reached"
        );
        // The same glyph at interline 20 clears 0.04 * 20^2 = 16 and does reach the network. What
        // it decides is not the point and is not asserted -- only that the gate let it through,
        // which is what makes the sheet-versus-staff interline choice observable.
        assert!(
            classifier.evaluate(&sample, 20, 5, 0.0125).is_ok(),
            "the gate must not reject at interline 20"
        );
    }
}
