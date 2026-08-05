// SPDX-License-Identifier: AGPL-3.0-or-later

//! Structured output for what GRID recognised, including *why*.
//!
//! The text reports are for reading a run; this is for consuming one. It exists
//! because the interesting part of an Audiveris-shaped recogniser is not its
//! answer but its evidence: every promoted inter carries the impacts it was
//! graded from, the relations that support it, and whether it was frozen. That
//! is what makes a wrong answer *diagnosable* rather than merely wrong, and
//! until now the port computed all of it and threw it away at the report
//! boundary.
//!
//! Concretely: the six staff-vertical impacts behind a barline are what located
//! a `Math.rint` versus `f64::round` divergence that six sessions of reading
//! the two sources had missed. A consumer -- an evaluation harness, a
//! model that proposes corrections, a human -- can only use that if it is
//! emitted.
//!
//! # Format
//!
//! JSON, hand-written rather than derived. The workspace carries no
//! serialization dependency and this is not the place to introduce one: the
//! schema is small, it is a published interface that should change
//! deliberately, and writing it out makes every field's provenance visible at
//! the point it is emitted.
//!
//! Numbers are emitted at full `f64` precision, because the whole value of the
//! grades and impacts is that they are exact against Java. Rounding them for
//! looks would throw away the only property that makes them checkable.

use std::fmt::Write as _;

use audiveris_image::bars_logic::{PeakWidthClass, VerticalInterKind};
use audiveris_image::grid_sig::{GridSigNode, GridSigRelation};
use audiveris_image::lines_coordinator::StaffCandidateKind;

use crate::grid_executor::HeadlessStaffLine;
use crate::recognize::{GridLinesRecognition, ScaleRecognition};

/// A minimal JSON writer.
///
/// Deliberately not a general one: it emits the shapes this module needs and
/// nothing else, so there is no dependency and no question about how a value
/// was encoded.
#[derive(Default)]
struct Json {
    out: String,
    needs_comma: bool,
}

impl Json {
    fn open(&mut self, bracket: char) {
        self.punctuate();
        self.out.push(bracket);
        self.needs_comma = false;
    }

    fn close(&mut self, bracket: char) {
        self.out.push(bracket);
        self.needs_comma = true;
    }

    fn punctuate(&mut self) {
        if self.needs_comma {
            self.out.push(',');
        }
        self.needs_comma = false;
    }

    fn key(&mut self, name: &str) {
        self.punctuate();
        self.string(name);
        self.out.push(':');
        // A key is not a value: the next thing written is its value and must
        // not be separated from it by a comma.
        self.needs_comma = false;
    }

    fn string(&mut self, value: &str) {
        self.punctuate();
        self.out.push('"');
        for character in value.chars() {
            match character {
                '"' => self.out.push_str("\\\""),
                '\\' => self.out.push_str("\\\\"),
                '\n' => self.out.push_str("\\n"),
                '\r' => self.out.push_str("\\r"),
                '\t' => self.out.push_str("\\t"),
                control if (control as u32) < 0x20 => {
                    let _ = write!(self.out, "\\u{:04x}", control as u32);
                }
                other => self.out.push(other),
            }
        }
        self.out.push('"');
        self.needs_comma = true;
    }

    fn number(&mut self, value: f64) {
        self.punctuate();
        if value.is_finite() {
            // `{:?}` is Rust's shortest round-tripping form for `f64`, which is
            // what keeps an emitted grade comparable to the one computed.
            let _ = write!(self.out, "{value:?}");
        } else {
            // JSON has no infinities or NaN, and silently emitting `0` would be
            // a lie about a value that means "not computed".
            self.out.push_str("null");
        }
        self.needs_comma = true;
    }

    fn integer(&mut self, value: i64) {
        self.punctuate();
        let _ = write!(self.out, "{value}");
        self.needs_comma = true;
    }

    fn boolean(&mut self, value: bool) {
        self.punctuate();
        self.out.push_str(if value { "true" } else { "false" });
        self.needs_comma = true;
    }

    fn null(&mut self) {
        self.punctuate();
        self.out.push_str("null");
        self.needs_comma = true;
    }

    fn field_number(&mut self, name: &str, value: f64) {
        self.key(name);
        self.number(value);
    }

    fn field_integer(&mut self, name: &str, value: i64) {
        self.key(name);
        self.integer(value);
    }

    fn field_string(&mut self, name: &str, value: &str) {
        self.key(name);
        self.string(value);
    }

    fn field_boolean(&mut self, name: &str, value: bool) {
        self.key(name);
        self.boolean(value);
    }
}

/// Emits everything GRID recognised on one sheet, as JSON.
///
/// `input` and `sheet` identify the source; a PDF supplies more than one sheet
/// and the id counts from one, as `ImageLoading.Loader.getImage(int)` does.
#[must_use]
pub fn grid_json(recognition: &GridLinesRecognition, input: &str, sheet: usize) -> String {
    let mut json = Json::default();
    json.open('{');

    // A consensus front end diffs these across producers, so the envelope
    // names who produced it and which schema it speaks. The geometry and
    // labels below are meant to be comparable against a system that is not
    // Audiveris at all; anything Audiveris-shaped sits under `evidence`, where
    // a reader can consume it per-producer or ignore it.
    json.field_integer("schema", 1);
    json.key("producer");
    json.open('{');
    json.field_string("name", "audiveris-rust");
    json.field_string("version", env!("CARGO_PKG_VERSION"));
    json.field_string("stage", "GRID");
    json.close('}');
    json.field_string("input", input);
    json.field_integer("sheet", sheet as i64);

    image(&mut json, &recognition.scale);
    scale(&mut json, &recognition.scale);

    json.field_number("slope", recognition.global_slope);

    systems(&mut json, recognition);
    staves(&mut json, recognition);
    inters(&mut json, recognition);
    relations(&mut json, recognition);

    json.close('}');
    json.out.push('\n');
    json.out
}

fn image(json: &mut Json, scale: &ScaleRecognition) {
    json.key("image");
    json.open('{');
    json.field_integer("width", scale.width as i64);
    json.field_integer("height", scale.height as i64);
    // The digest of the gray raster as binarization received it. For a PDF this
    // equals the FNV-1a-64 of the page PDFBox rendered, which is what the
    // ingest test asserts, so it doubles as a provenance stamp.
    json.field_string("gray_digest", &format!("{:016x}", scale.gray_digest));
    json.close('}');
}

fn scale(json: &mut Json, recognition: &ScaleRecognition) {
    let scale = &recognition.scale;
    json.key("scale");
    json.open('{');
    for (name, value) in [
        (
            "line",
            Some((scale.line.min, scale.line.main, scale.line.max)),
        ),
        (
            "interline",
            Some((
                scale.interline.min,
                scale.interline.main,
                scale.interline.max,
            )),
        ),
        (
            "small_interline",
            scale
                .small_interline
                .map(|value| (value.min, value.main, value.max)),
        ),
    ] {
        json.key(name);
        match value {
            Some((min, main, max)) => {
                json.open('{');
                json.field_integer("min", i64::from(min));
                json.field_integer("main", i64::from(main));
                json.field_integer("max", i64::from(max));
                json.close('}');
            }
            None => json.null(),
        }
    }
    json.field_integer("beam", i64::from(scale.beam.main));
    json.key("small_beam");
    match scale.small_beam {
        Some(beam) => json.integer(i64::from(beam.main)),
        None => json.null(),
    }
    json.field_string("resolution", &format!("{:?}", scale.resolution));
    json.close('}');
}

fn systems(json: &mut Json, recognition: &GridLinesRecognition) {
    json.key("systems");
    json.open('[');
    for (index, staff_ids) in recognition.peak_graph.systems.iter().enumerate() {
        json.open('{');
        json.field_integer("id", index as i64 + 1);
        json.key("staves");
        json.open('[');
        for id in staff_ids {
            json.integer(*id as i64);
        }
        json.close(']');
        json.close('}');
    }
    json.close(']');
}

fn staves(json: &mut Json, recognition: &GridLinesRecognition) {
    json.key("staves");
    json.open('[');
    for staff in &recognition.peak_graph.sheet_staffs {
        json.open('{');
        json.field_integer("id", staff.id as i64);
        json.field_string(
            "kind",
            match staff.kind {
                StaffCandidateKind::Standard => "standard",
                StaffCandidateKind::OneLine => "one-line",
                StaffCandidateKind::Tablature => "tablature",
            },
        );
        json.field_number("left", staff.left);
        json.field_number("right", staff.right);
        json.field_integer("interline", staff.interline as i64);
        json.field_boolean("small", staff.small);
        json.field_boolean("short", staff.short);
        json.field_integer("line_count", staff.lines.len() as i64);
        json.key("lines");
        json.open('[');
        for line in &staff.lines {
            json.open('{');
            match line {
                HeadlessStaffLine::Filament { line_id, .. } => {
                    json.field_integer("line_id", *line_id as i64);
                    // A filament line means `completeLines` has not converted
                    // this staff yet, which is a state worth being able to see.
                    json.field_string("source", "filament");
                }
                HeadlessStaffLine::Persistent {
                    source_line_id,
                    line,
                } => {
                    json.field_integer("line_id", *source_line_id as i64);
                    json.field_string("source", "persistent");
                    json.key("points");
                    json.open('[');
                    for (x, y) in &line.points {
                        json.open('{');
                        json.field_number("x", *x);
                        json.field_number("y", *y);
                        json.close('}');
                    }
                    json.close(']');
                    json.field_number("thickness", line.thickness);
                }
            }
            json.close('}');
        }
        json.close(']');
        json.close('}');
    }
    json.close(']');
}

/// The promoted inters, each with the evidence it was graded from.
fn inters(json: &mut Json, recognition: &GridLinesRecognition) {
    json.key("inters");
    json.open('[');
    for system in &recognition.peak_graph.sig.systems {
        for (id, node) in system.sig.nodes_in_order() {
            json.open('{');
            json.field_integer("id", id.value() as i64);
            json.field_integer("system", system.system_id as i64);
            match node {
                GridSigNode::Vertical {
                    plan,
                    frozen,
                    contextual_grade,
                    ..
                } => {
                    let (kind, staff_end) = match plan.kind {
                        VerticalInterKind::Barline {
                            width_class,
                            left_staff_end,
                            right_staff_end,
                        } => (
                            match width_class {
                                PeakWidthClass::Thin => "THIN_BARLINE",
                                PeakWidthClass::Thick => "THICK_BARLINE",
                            },
                            if left_staff_end {
                                "LEFT"
                            } else if right_staff_end {
                                "RIGHT"
                            } else {
                                "NONE"
                            },
                        ),
                        VerticalInterKind::Bracket(_) => ("BRACKET", "NONE"),
                    };
                    json.field_string("kind", kind);
                    json.field_integer("staff", plan.peak.staff_id().value() as i64);
                    json.field_number("width", plan.width);
                    json.field_string("staff_end", staff_end);
                    json.key("median");
                    json.open('{');
                    json.field_number("x", plan.median.x);
                    json.field_number("top", plan.median.top);
                    json.field_number("bottom", plan.median.bottom);
                    json.close('}');
                    json.field_number("grade", node.intrinsic_grade());
                    json.key("contextual_grade");
                    match contextual_grade {
                        Some(grade) => json.number(*grade),
                        None => json.null(),
                    }
                    // The point of this file, and the part that is Audiveris's
                    // rather than universal. A grade is a weighted geometric
                    // mean of these six terms, and only the terms say *why* it
                    // came out where it did.
                    json.key("evidence");
                    json.open('{');
                    json.field_boolean("frozen", *frozen);
                    json.key("impacts");
                    match plan.impacts {
                        Some(impacts) => {
                            json.open('{');
                            json.field_number("core", impacts.core());
                            json.field_number("gap", impacts.gap());
                            json.field_number("start_derivative", impacts.start());
                            json.field_number("stop_derivative", impacts.stop());
                            json.field_number("left_chunk", impacts.left());
                            json.field_number("right_chunk", impacts.right());
                            json.close('}');
                        }
                        None => json.null(),
                    }
                    json.close('}');
                }
                GridSigNode::Connector {
                    plan,
                    frozen,
                    contextual_grade,
                } => {
                    json.field_string("kind", "BAR_CONNECTOR");
                    json.field_number("grade", plan.grade);
                    json.key("contextual_grade");
                    match contextual_grade {
                        Some(grade) => json.number(*grade),
                        None => json.null(),
                    }
                    json.key("evidence");
                    json.open('{');
                    json.field_boolean("frozen", *frozen);
                    json.close('}');
                }
            }
            json.close('}');
        }
    }
    json.close(']');
}

/// The support and exclusion edges, which are what a contextual grade is made
/// of and the first thing to look at when one is wrong.
fn relations(json: &mut Json, recognition: &GridLinesRecognition) {
    json.key("relations");
    json.open('[');
    for system in &recognition.peak_graph.sig.systems {
        for edge in system.sig.edges() {
            json.open('{');
            json.field_integer("system", system.system_id as i64);
            json.field_integer("source", edge.source.value() as i64);
            json.field_integer("target", edge.target.value() as i64);
            match edge.relation {
                GridSigRelation::NoExclusion => json.field_string("kind", "no-exclusion"),
                GridSigRelation::BarConnectionSupport { grade } => {
                    json.field_string("kind", "bar-connection-support");
                    json.field_number("grade", grade);
                }
                GridSigRelation::BarGroup { gap_fraction } => {
                    json.field_string("kind", "bar-group");
                    json.field_number("gap_fraction", gap_fraction);
                }
            }
            json.close('}');
        }
    }
    json.close(']');
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn escapes_what_json_requires_and_nothing_else() {
        let mut json = Json::default();
        json.string("a\"b\\c\nd\te\u{1}f/g");
        assert_eq!(json.out, "\"a\\\"b\\\\c\\nd\\te\\u0001f/g\"");
    }

    #[test]
    fn emits_numbers_that_round_trip() {
        let mut json = Json::default();
        json.number(0.786_172_622_123_456_8);
        let parsed: f64 = json.out.parse().expect("a number");
        assert_eq!(parsed, 0.786_172_622_123_456_8);
    }

    #[test]
    fn a_non_finite_number_is_null_rather_than_a_wrong_number() {
        // A contextual grade that was never computed must not read as zero,
        // which is a legitimate grade.
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut json = Json::default();
            json.number(value);
            assert_eq!(json.out, "null");
        }
    }

    /// Scans for the shapes a stateful writer gets wrong: a comma where a
    /// value should be, or an unbalanced bracket.
    ///
    /// Kept as a helper rather than a parser because the point is to catch
    /// malformed output, and a parser that accepted it would defeat that.
    pub(crate) fn structural_faults(json: &str) -> Vec<String> {
        let mut faults = Vec::new();
        let mut depth = 0i32;
        let mut in_string = false;
        let mut escaped = false;
        let bytes: Vec<char> = json.chars().collect();
        for (index, character) in bytes.iter().enumerate() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if *character == '\\' {
                    escaped = true;
                } else if *character == '"' {
                    in_string = false;
                }
                continue;
            }
            match character {
                '"' => in_string = true,
                '{' | '[' => depth += 1,
                '}' | ']' => {
                    depth -= 1;
                    if depth < 0 {
                        faults.push(format!("unbalanced close at {index}"));
                    }
                }
                ',' => {
                    let previous = bytes[..index].iter().rev().find(|c| !c.is_whitespace());
                    let next = bytes[index + 1..].iter().find(|c| !c.is_whitespace());
                    if matches!(previous, Some(':' | ',' | '{' | '[')) {
                        faults.push(format!("comma after {previous:?} at {index}"));
                    }
                    if matches!(next, Some('}' | ']' | ',')) {
                        faults.push(format!("comma before {next:?} at {index}"));
                    }
                }
                ':' => {
                    let next = bytes[index + 1..].iter().find(|c| !c.is_whitespace());
                    if matches!(next, Some(',')) {
                        faults.push(format!("colon followed by comma at {index}"));
                    }
                }
                _ => {}
            }
        }
        if depth != 0 {
            faults.push(format!("unbalanced by {depth}"));
        }
        if in_string {
            faults.push("unterminated string".to_owned());
        }
        faults
    }

    #[test]
    fn the_fault_scanner_catches_what_a_stateful_writer_gets_wrong() {
        assert!(structural_faults(r#"{"a":1,"b":[2,3]}"#).is_empty());
        // The exact shape this writer emitted before `key` stopped leaving a
        // pending comma behind.
        assert!(!structural_faults(r#"{"a":,1}"#).is_empty());
        assert!(!structural_faults(r#"{"a":1,}"#).is_empty());
        assert!(!structural_faults(r#"{"a":1"#).is_empty());
        assert!(!structural_faults(r#"{"a":1}}"#).is_empty());
        // A comma inside a string is not a fault.
        assert!(structural_faults(r#"{"a":"x,:y"}"#).is_empty());
    }

    #[test]
    fn commas_separate_only_between_values() {
        let mut json = Json::default();
        json.open('{');
        json.field_integer("a", 1);
        json.field_integer("b", 2);
        json.key("c");
        json.open('[');
        json.integer(3);
        json.integer(4);
        json.close(']');
        json.close('}');
        assert_eq!(json.out, r#"{"a":1,"b":2,"c":[3,4]}"#);
    }
}
