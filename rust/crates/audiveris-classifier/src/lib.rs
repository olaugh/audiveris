// SPDX-License-Identifier: AGPL-3.0-or-later

//! Immutable inference for the bundled Audiveris basic glyph classifier.
//!
//! This crate intentionally stops at the classifier boundary: callers provide the 110 raw
//! descriptor values. It does not extract ART/geometric features, consult user files, or apply
//! any post-classification musical policy.

use std::fmt;
use std::io::{Cursor, Read};

use quick_xml::Reader;
use quick_xml::events::Event;
use zip::ZipArchive;

/// Number of raw features accepted by Audiveris' shipped basic classifier.
pub const INPUT_SIZE: usize = 110;
/// Number of hidden cells in Audiveris' shipped basic classifier.
pub const HIDDEN_SIZE: usize = 149;
/// Number of ordered physical shape grades emitted by the classifier.
pub const OUTPUT_SIZE: usize = 149;

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
            let sum = row[0]
                + inputs
                    .iter()
                    .zip(&row[1..])
                    .map(|(input, weight)| input * weight)
                    .sum::<f64>();
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
}
