// SPDX-License-Identifier: AGPL-3.0-or-later

//! Immutable inference for the bundled Audiveris basic glyph classifier.
//!
//! This crate intentionally stops at the classifier boundary: callers provide the 110 raw
//! descriptor values. It does not extract ART/geometric features, consult user files, or apply
//! any post-classification musical policy.

use std::fmt;
use std::io::{Cursor, Read};
use std::sync::OnceLock;

use quick_xml::Reader;
use quick_xml::events::Event;
use zip::ZipArchive;

/// Number of raw features accepted by Audiveris' shipped basic classifier.
pub const INPUT_SIZE: usize = 110;
/// Number of hidden cells in Audiveris' shipped basic classifier.
pub const HIDDEN_SIZE: usize = 149;
/// Number of ordered physical shape grades emitted by the classifier.
pub const OUTPUT_SIZE: usize = 149;

/// The angular order used by Audiveris' `BasicARTExtractor`.
pub const ART_ANGULAR: usize = 20;
/// The radial order used by Audiveris' `BasicARTExtractor`.
pub const ART_RADIAL: usize = 5;
/// The number of ART modules retained by `MixGlyphDescriptor` (all but F00).
pub const ART_FEATURE_COUNT: usize = ART_ANGULAR * ART_RADIAL - 1;
/// The number of leading geometric values retained by `MixGlyphDescriptor`.
pub const GEOMETRIC_FEATURE_COUNT: usize = 10;

const LUT_RADIUS: usize = 50;
const LUT_SIZE: usize = LUT_RADIUS * 2 + 1;

/// One raw basic-classifier result, in the model's exact output-label order.
#[derive(Clone, Debug, PartialEq)]
pub struct ShapeGrade {
    /// Audiveris `Shape` enum name, exactly as stored in `model.xml`.
    pub shape: String,
    /// Sigmoid output grade. This is deliberately not re-ranked or thresholded.
    pub grade: f64,
}

/// Failure while loading or validating the checked-in model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelLoadError(String);

impl fmt::Display for ModelLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ModelLoadError {}

/// Failure while extracting the bundled classifier's raw glyph descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FeatureExtractionError {
    /// The descriptor requires at least one foreground pixel.
    EmptyPixels,
    /// Audiveris normalizes geometric values by the staff interline, which must be positive.
    InvalidInterline,
}

impl fmt::Display for FeatureExtractionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPixels => formatter.write_str("cannot extract a descriptor from no pixels"),
            Self::InvalidInterline => formatter.write_str("interline must be positive"),
        }
    }
}

impl std::error::Error for FeatureExtractionError {}

/// Extracts the 110 `MixGlyphDescriptor` inputs expected by [`BasicClassifier`].
///
/// `pixels` is Audiveris' foreground-point list in its original iteration order. The feature
/// values are translation-independent except for the geometric centroid values which the Java
/// descriptor deliberately omits. Consequently the coordinates may be either glyph-local or
/// absolute sheet coordinates, as long as their bounding box matches the glyph's bounds.
///
/// This ports `RunTable.computeArtMoments`, `BasicARTExtractor`, `GeometricMoments`, and
/// `MixGlyphDescriptor`, but intentionally does not assume a RunTable/Glyph representation.
pub fn mix_glyph_features(
    pixels: &[(i32, i32)],
    interline: i32,
) -> Result<[f64; INPUT_SIZE], FeatureExtractionError> {
    if pixels.is_empty() {
        return Err(FeatureExtractionError::EmptyPixels);
    }
    if interline <= 0 {
        return Err(FeatureExtractionError::InvalidInterline);
    }

    let art = art_modules(pixels);
    let geometric = geometric_moments(pixels, interline);
    let (min_x, max_x, min_y, max_y) = bounds(pixels);
    let width = f64::from(max_x - min_x + 1);
    let height = f64::from(max_y - min_y + 1);
    let mut features = [0.0; INPUT_SIZE];
    features[..ART_FEATURE_COUNT].copy_from_slice(&art);
    features[ART_FEATURE_COUNT..ART_FEATURE_COUNT + GEOMETRIC_FEATURE_COUNT]
        .copy_from_slice(&geometric[..GEOMETRIC_FEATURE_COUNT]);
    features[INPUT_SIZE - 1] = height / width;
    Ok(features)
}

fn bounds(pixels: &[(i32, i32)]) -> (i32, i32, i32, i32) {
    pixels.iter().fold(
        (i32::MAX, i32::MIN, i32::MAX, i32::MIN),
        |(min_x, max_x, min_y, max_y), &(x, y)| {
            (min_x.min(x), max_x.max(x), min_y.min(y), max_y.max(y))
        },
    )
}

fn art_modules(pixels: &[(i32, i32)]) -> [f64; ART_FEATURE_COUNT] {
    // Preserve Java's `AbstractExtractor.findCenterOfMass` operation ordering.
    let (sum_x, sum_y) = pixels
        .iter()
        .fold((0_i64, 0_i64), |(sum_x, sum_y), &(x, y)| {
            (sum_x + i64::from(x), sum_y + i64::from(y))
        });
    let mass = pixels.len() as f64;
    let center_x = sum_x as f64 / mass;
    let center_y = sum_y as f64 / mass;
    let (dx_max, dy_max) = pixels
        .iter()
        .fold((f64::MIN, f64::MIN), |(dx, dy), &(x, y)| {
            (
                dx.max((f64::from(x) - center_x).abs()),
                dy.max((f64::from(y) - center_y).abs()),
            )
        });
    let radius = dx_max.hypot(dy_max);
    let tables = art_luts();
    let mut real = [[0.0; ART_RADIAL]; ART_ANGULAR];
    let mut imaginary = [[0.0; ART_RADIAL]; ART_ANGULAR];

    for &(x, y) in pixels {
        let lx = ((f64::from(x) - center_x) * LUT_RADIUS as f64 / radius) + LUT_RADIUS as f64;
        let ly = ((f64::from(y) - center_y) * LUT_RADIUS as f64 / radius) + LUT_RADIUS as f64;
        if !(0.0..LUT_SIZE as f64).contains(&lx) || !(0.0..LUT_SIZE as f64).contains(&ly) {
            continue;
        }
        for p in 0..ART_ANGULAR {
            for r in 0..ART_RADIAL {
                let index = p * ART_RADIAL + r;
                real[p][r] += interpolate(&tables.real[index], lx, ly);
                // Java accumulates `coeffImag -= imagLut.interpolate(...)`.
                imaginary[p][r] -= interpolate(&tables.imaginary[index], lx, ly);
            }
        }
    }

    let mut result = [0.0; ART_FEATURE_COUNT];
    let mut output = 0;
    for p in 0..ART_ANGULAR {
        for r in 0..ART_RADIAL {
            if p != 0 || r != 0 {
                result[output] = (imaginary[p][r] / mass).hypot(real[p][r] / mass);
                output += 1;
            }
        }
    }
    result
}

fn geometric_moments(pixels: &[(i32, i32)], interline: i32) -> [f64; 12] {
    let dim = pixels.len() as f64;
    let mut mean_x = 0.0;
    let mut mean_y = 0.0;
    // Java traverses the supplied collector arrays backwards for both passes.
    for &(x, y) in pixels.iter().rev() {
        mean_x += f64::from(x);
        mean_y += f64::from(y);
    }
    mean_x /= dim;
    mean_y /= dim;

    let mut n11 = 0.0;
    let mut n12 = 0.0;
    let mut n21 = 0.0;
    let mut n20 = 0.0;
    let mut n02 = 0.0;
    let mut n30 = 0.0;
    let mut n03 = 0.0;
    for &(px, py) in pixels.iter().rev() {
        let x = f64::from(px) - mean_x;
        let y = f64::from(py) - mean_y;
        n11 += x * y;
        n12 += x * y * y;
        n21 += x * x * y;
        n20 += x * x;
        n02 += y * y;
        n30 += x * x * x;
        n03 += y * y * y;
    }
    let w2 = dim * dim;
    let w3 = (dim * dim * dim * dim * dim).sqrt();
    let (min_x, max_x, min_y, max_y) = bounds(pixels);
    let unit = f64::from(interline);
    [
        dim / (unit * unit),
        f64::from(max_x - min_x + 1) / unit,
        f64::from(max_y - min_y + 1) / unit,
        n20 / w2,
        n11 / w2,
        n02 / w2,
        n30 / w3,
        n21 / w3,
        n12 / w3,
        n03 / w3,
        mean_x,
        mean_y,
    ]
}

struct ArtLuts {
    real: Vec<Vec<f64>>,
    imaginary: Vec<Vec<f64>>,
}

fn art_luts() -> &'static ArtLuts {
    static TABLES: OnceLock<ArtLuts> = OnceLock::new();
    TABLES.get_or_init(|| {
        let count = ART_ANGULAR * ART_RADIAL;
        let mut real = vec![vec![0.0; LUT_SIZE * LUT_SIZE]; count];
        let mut imaginary = vec![vec![0.0; LUT_SIZE * LUT_SIZE]; count];
        for x in 0..LUT_SIZE {
            let tx = (x as f64 - LUT_RADIUS as f64) / LUT_RADIUS as f64;
            for y in 0..LUT_SIZE {
                let ty = (y as f64 - LUT_RADIUS as f64) / LUT_RADIUS as f64;
                let radial = tx.hypot(ty);
                if radial < 1.0 {
                    let angle = ty.atan2(tx);
                    for p in 0..ART_ANGULAR {
                        for r in 0..ART_RADIAL {
                            let index = p * ART_RADIAL + r;
                            let basis = (radial * std::f64::consts::PI * r as f64).cos();
                            let cell = x * LUT_SIZE + y;
                            real[index][cell] = basis * (angle * p as f64).cos();
                            imaginary[index][cell] = basis * (angle * p as f64).sin();
                        }
                    }
                }
            }
        }
        ArtLuts { real, imaginary }
    })
}

fn interpolate(table: &[f64], px: f64, py: f64) -> f64 {
    // Inputs have been range checked and are non-negative, so Java's truncation is floor here.
    let x = px as usize;
    let y = py as usize;
    let ix = px - x as f64;
    let iy = py - y as f64;
    let max = LUT_SIZE - 1;
    let value = table[x * LUT_SIZE + y];
    if x == max {
        if y == max {
            value
        } else {
            value + iy * (table[x * LUT_SIZE + y + 1] - value)
        }
    } else {
        let x_next = table[(x + 1) * LUT_SIZE + y];
        let vpxy = value + ix * (x_next - value);
        if y == max {
            vpxy
        } else {
            let xy_next = table[x * LUT_SIZE + y + 1];
            let vpxy_next = xy_next + ix * (table[(x + 1) * LUT_SIZE + y + 1] - xy_next);
            vpxy + iy * (vpxy_next - vpxy)
        }
    }
}

/// Immutable, bundled 110 -> 149 -> 149 sigmoid network and its feature norms.
#[derive(Clone, Debug)]
pub struct BasicClassifier {
    input_labels: Vec<String>,
    output_labels: Vec<String>,
    means: Vec<f64>,
    stds: Vec<f64>,
    hidden_weights: Vec<Vec<f64>>,
    output_weights: Vec<Vec<f64>>,
}

impl BasicClassifier {
    /// Loads only the repository's bundled `app/res/basic-classifier.zip` model.
    ///
    /// There is intentionally no environment, home-directory, or command-line override here:
    /// the result is a reproducible production artifact.
    pub fn bundled() -> Result<Self, ModelLoadError> {
        Self::from_zip_bytes(include_bytes!("../../../../app/res/basic-classifier.zip"))
    }

    /// Parses a basic-classifier archive. Exposed for fixtures; production callers should use
    /// [`Self::bundled`] to preserve the no-user-override contract.
    pub fn from_zip_bytes(bytes: &[u8]) -> Result<Self, ModelLoadError> {
        let mut archive = ZipArchive::new(Cursor::new(bytes))
            .map_err(|error| ModelLoadError(format!("invalid basic-classifier zip: {error}")))?;
        let model = read_entry(&mut archive, "model.xml")?;
        let means = read_entry(&mut archive, "means.xml")?;
        let stds = read_entry(&mut archive, "stds.xml")?;
        let mut classifier = parse_model(&model)?;
        classifier.means = parse_vector(&means, "means.xml")?;
        classifier.stds = parse_vector(&stds, "stds.xml")?;
        classifier.validate()?;
        Ok(classifier)
    }

    /// Exact model input feature labels, in input-vector order.
    #[must_use]
    pub fn input_labels(&self) -> &[String] {
        &self.input_labels
    }

    /// Exact Audiveris physical-shape names, in output-vector order.
    #[must_use]
    pub fn output_labels(&self) -> &[String] {
        &self.output_labels
    }

    /// Normalizes raw descriptor values with the bundled `(value - mean) / std` transform.
    pub fn normalize(&self, features: &[f64; INPUT_SIZE]) -> [f64; INPUT_SIZE] {
        std::array::from_fn(|index| (features[index] - self.means[index]) / self.stds[index])
    }

    /// Runs normalized input through the exact two sigmoid layers and preserves model order.
    #[must_use]
    pub fn evaluate(&self, features: &[f64; INPUT_SIZE]) -> Vec<ShapeGrade> {
        let normalized = self.normalize(features);
        let hidden = forward(&normalized, &self.hidden_weights);
        let grades = forward(&hidden, &self.output_weights);
        self.output_labels
            .iter()
            .cloned()
            .zip(grades)
            .map(|(shape, grade)| ShapeGrade { shape, grade })
            .collect()
    }

    fn validate(&self) -> Result<(), ModelLoadError> {
        if self.input_labels.len() != INPUT_SIZE
            || self.means.len() != INPUT_SIZE
            || self.stds.len() != INPUT_SIZE
            || self.hidden_weights.len() != HIDDEN_SIZE
            || self
                .hidden_weights
                .iter()
                .any(|row| row.len() != INPUT_SIZE + 1)
        {
            return Err(ModelLoadError(
                "basic classifier input layer is not 110 -> 149".into(),
            ));
        }
        if self.output_labels.len() != OUTPUT_SIZE
            || self.output_weights.len() != OUTPUT_SIZE
            || self
                .output_weights
                .iter()
                .any(|row| row.len() != HIDDEN_SIZE + 1)
        {
            return Err(ModelLoadError(
                "basic classifier output layer is not 149 -> 149".into(),
            ));
        }
        if self.stds.iter().any(|std| !std.is_finite() || *std == 0.0) {
            return Err(ModelLoadError(
                "basic classifier has invalid normalization deviations".into(),
            ));
        }
        Ok(())
    }
}

fn forward(inputs: &[f64], weights: &[Vec<f64>]) -> Vec<f64> {
    weights
        .iter()
        .map(|row| {
            // `NeuralNetwork.forward` iterates the input cells from the last
            // index down to zero, then adds the bias. Keep that accumulation
            // order: raw classifier vectors are a numerical compatibility
            // boundary, not merely an approximate neural-network result.
            let mut sum = 0.0;
            for index in (0..inputs.len()).rev() {
                sum += row[index + 1] * inputs[index];
            }
            sum += row[0];
            1.0 / (1.0 + (-sum).exp())
        })
        .collect()
}

fn read_entry(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    name: &str,
) -> Result<Vec<u8>, ModelLoadError> {
    let mut entry = archive
        .by_name(name)
        .map_err(|error| ModelLoadError(format!("missing {name}: {error}")))?;
    let mut bytes = Vec::new();
    entry
        .read_to_end(&mut bytes)
        .map_err(|error| ModelLoadError(format!("cannot read {name}: {error}")))?;
    Ok(bytes)
}

fn parse_model(bytes: &[u8]) -> Result<BasicClassifier, ModelLoadError> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut input_labels = Vec::new();
    let mut output_labels = Vec::new();
    let mut hidden_weights = Vec::new();
    let mut output_weights = Vec::new();
    let mut matrix = None::<bool>;
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start)) => match start.name().as_ref() {
                b"neural-network" => {
                    let input = attribute_usize(&start, b"input-size")?;
                    let hidden = attribute_usize(&start, b"hidden-size")?;
                    let output = attribute_usize(&start, b"output-size")?;
                    if (input, hidden, output) != (INPUT_SIZE, HIDDEN_SIZE, OUTPUT_SIZE) {
                        return Err(ModelLoadError(format!(
                            "unsupported basic classifier dimensions {input} -> {hidden} -> {output}"
                        )));
                    }
                }
                b"input-labels" => input_labels = split_text(&mut reader, b"input-labels")?,
                b"output-labels" => output_labels = split_text(&mut reader, b"output-labels")?,
                b"hidden-weights" => matrix = Some(true),
                b"output-weights" => matrix = Some(false),
                b"row" => {
                    let row = parse_row(&mut reader)?;
                    match matrix {
                        Some(true) => hidden_weights.push(row),
                        Some(false) => output_weights.push(row),
                        None => return Err(ModelLoadError("weight row outside a matrix".into())),
                    }
                }
                _ => {}
            },
            Ok(Event::End(end)) => match end.name().as_ref() {
                b"hidden-weights" | b"output-weights" => matrix = None,
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(error) => return Err(ModelLoadError(format!("invalid model.xml: {error}"))),
            _ => {}
        }
        buffer.clear();
    }
    Ok(BasicClassifier {
        input_labels,
        output_labels,
        means: Vec::new(),
        stds: Vec::new(),
        hidden_weights,
        output_weights,
    })
}

fn parse_vector(bytes: &[u8], name: &str) -> Result<Vec<f64>, ModelLoadError> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut values = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start)) if start.name().as_ref() == b"value" => {
                values.push(parse_number(&read_text(&mut reader, b"value")?, name)?);
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(ModelLoadError(format!("invalid {name}: {error}"))),
            _ => {}
        }
        buffer.clear();
    }
    Ok(values)
}

fn parse_row(reader: &mut Reader<&[u8]>) -> Result<Vec<f64>, ModelLoadError> {
    let mut buffer = Vec::new();
    let mut row = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start)) if start.name().as_ref() == b"item" => {
                row.push(parse_number(&read_text(reader, b"item")?, "model.xml")?);
            }
            Ok(Event::End(end)) if end.name().as_ref() == b"row" => break,
            Ok(Event::Eof) => return Err(ModelLoadError("unexpected end in weight row".into())),
            Err(error) => return Err(ModelLoadError(format!("invalid model.xml: {error}"))),
            _ => {}
        }
        buffer.clear();
    }
    Ok(row)
}

fn split_text(reader: &mut Reader<&[u8]>, tag: &[u8]) -> Result<Vec<String>, ModelLoadError> {
    Ok(read_text(reader, tag)?
        .split_ascii_whitespace()
        .map(str::to_owned)
        .collect())
}

fn read_text(reader: &mut Reader<&[u8]>, tag: &[u8]) -> Result<String, ModelLoadError> {
    let mut buffer = Vec::new();
    let mut text = String::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Text(value)) => {
                let decoded = value
                    .decode()
                    .map_err(|error| ModelLoadError(format!("invalid XML text: {error}")))?;
                text.push_str(&decoded);
            }
            Ok(Event::End(end)) if end.name().as_ref() == tag => return Ok(text),
            Ok(Event::Eof) => return Err(ModelLoadError("unexpected end in XML text".into())),
            Err(error) => return Err(ModelLoadError(format!("invalid XML: {error}"))),
            _ => {}
        }
        buffer.clear();
    }
}

fn attribute_usize(
    start: &quick_xml::events::BytesStart<'_>,
    name: &[u8],
) -> Result<usize, ModelLoadError> {
    for attribute in start.attributes() {
        let attribute =
            attribute.map_err(|error| ModelLoadError(format!("invalid attribute: {error}")))?;
        if attribute.key.as_ref() == name {
            let value = attribute
                .unescape_value()
                .map_err(|error| ModelLoadError(format!("invalid attribute value: {error}")))?;
            return value.parse().map_err(|error| {
                ModelLoadError(format!(
                    "invalid {} attribute: {error}",
                    String::from_utf8_lossy(name)
                ))
            });
        }
    }
    Err(ModelLoadError(format!(
        "missing {} attribute",
        String::from_utf8_lossy(name)
    )))
}

fn parse_number(value: &str, source: &str) -> Result<f64, ModelLoadError> {
    value
        .parse()
        .map_err(|error| ModelLoadError(format!("invalid number in {source}: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_metadata_is_the_checked_in_java_model() {
        let classifier = BasicClassifier::bundled().expect("bundled model");
        assert_eq!(classifier.input_labels().len(), INPUT_SIZE);
        assert_eq!(classifier.output_labels().len(), OUTPUT_SIZE);
        assert_eq!(classifier.input_labels()[0], "F001");
        assert_eq!(classifier.input_labels()[109], "aspect");
        assert_eq!(classifier.output_labels()[0], "DOT_set");
        assert_eq!(classifier.output_labels()[148], "CLUTTER");
    }

    #[test]
    fn normalization_is_the_java_in_situ_formula() {
        let classifier = BasicClassifier::bundled().expect("bundled model");
        let raw = std::array::from_fn(|index| classifier.means[index]);
        assert!(classifier.normalize(&raw).iter().all(|value| *value == 0.0));
    }

    #[test]
    fn zero_feature_vector_has_stable_ordered_sigmoid_results() {
        let classifier = BasicClassifier::bundled().expect("bundled model");
        let grades = classifier.evaluate(&[0.0; INPUT_SIZE]);
        assert_eq!(grades.len(), OUTPUT_SIZE);
        assert_eq!(grades[0].shape, "DOT_set");
        assert_eq!(grades[148].shape, "CLUTTER");
        assert!(
            grades
                .iter()
                .all(|grade| grade.grade.is_finite() && (0.0..=1.0).contains(&grade.grade))
        );
        // A model fingerprint at two positions guards the weight orientation and bias convention.
        assert!(
            (grades[0].grade - 0.065_407_191_044_490_08).abs() < 1e-15,
            "actual first grade: {}",
            grades[0].grade
        );
        assert!((grades[148].grade - 0.000_001_605_550_197_838_038_7).abs() < 1e-18);
    }

    #[test]
    fn synthetic_raw_vector_matches_the_production_java_oracle() {
        // Kept in lockstep with `RustParityProbe`; the full 149-output oracle
        // lives in `xtask vectors`, while these separated positions make a
        // classifier-only regression immediately legible.
        let classifier = BasicClassifier::bundled().expect("bundled model");
        let raw: [f64; INPUT_SIZE] =
            std::array::from_fn(|index| ((index * 17 % 23) as f64 - 11.0) / 7.0);
        let grades = classifier.evaluate(&raw);
        for (index, shape, expected) in [
            (0, "DOT_set", 0.000_005_931_338_051_75),
            (45, "TIME_TWO_FOUR", 0.001_736_790_154_075_68),
            (95, "DYNAMICS_P", 0.002_043_468_506_365_16),
            (148, "CLUTTER", 0.000_303_653_776_463_52),
        ] {
            assert_eq!(grades[index].shape, shape);
            assert!(
                (grades[index].grade - expected).abs() < 5e-18,
                "grade for {shape}: {}",
                grades[index].grade
            );
        }
    }

    #[test]
    fn mix_descriptor_rejects_empty_pixels_and_invalid_interline() {
        assert_eq!(
            mix_glyph_features(&[], 10),
            Err(FeatureExtractionError::EmptyPixels)
        );
        assert_eq!(
            mix_glyph_features(&[(0, 0)], 0),
            Err(FeatureExtractionError::InvalidInterline)
        );
    }

    #[test]
    fn mix_descriptor_preserves_art_geometry_and_aspect_layout() {
        // This is the same deliberately asymmetric point list as the live Java vector. These
        // positions make a zero/one-based coordinate mistake, an ART order mistake, or a signed
        // third-order geometric-moment mistake immediately observable.
        let pixels = [
            (2, 1),
            (3, 1),
            (5, 2),
            (2, 3),
            (3, 3),
            (4, 3),
            (4, 4),
            (6, 5),
            (3, 6),
            (7, 6),
        ];
        let features = mix_glyph_features(&pixels, 11).expect("valid asymmetric glyph");
        assert_eq!(features.len(), INPUT_SIZE);
        for (index, expected) in [
            (0, 0.008_405_864_717),
            (1, 0.151_875_526_046),
            (98, 0.205_662_345_252),  // final ART module (F194)
            (99, 0.082_644_628_099),  // normalized weight
            (100, 0.545_454_545_455), // normalized width
            (101, 0.545_454_545_455), // normalized height
            (103, 0.154),             // n11
            (106, 0.063_498_535_416), // n21
            (107, 0.018_594_192_642), // n12
            (109, 1.0),
        ] {
            assert!(
                (features[index] - expected).abs() < 5e-13,
                "feature {index}: {}",
                features[index]
            );
        }
    }
}
