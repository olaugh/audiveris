// SPDX-License-Identifier: AGPL-3.0-or-later

//! Structured output for native recognition products, including *why*.
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
//! emitted. GRID remains the stable schema-1 core; downstream stages add
//! their interpretations and stage-owned summaries without replacing it.
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

use std::{collections::BTreeMap, fmt::Write as _};

use audiveris_image::bars_logic::{PeakWidthClass, VerticalInterKind};
use audiveris_image::beam_structure::BeamImpacts;
use audiveris_image::grid_sig::{GridSigNode, GridSigRelation};
use audiveris_image::lines_coordinator::StaffCandidateKind;
use audiveris_image::system_population::BoundarySegment;

use crate::beam_inters::{RawBeam, beam_bounds};
use crate::clef_column::{NeutralClefCandidate, NeutralClefKind};
use crate::grid_executor::HeadlessStaffLine;
use crate::header_time_column::{
    NeutralSpecificTimeShape, NeutralTimeCandidate, NeutralTimeCandidateKind,
};
use crate::key_column::NeutralKeyCandidate;
use crate::native_headers::{NativeHeaderRecognition, NativeHeaderStaffRecognition};
use crate::native_ledgers::{NativeLedgerLine, NativeLedgerRecognition};
use crate::native_stem_seeds::{
    NativeStemSeedDecision, NativeStemSeedGate, NativeStemSeedGlyph, NativeStemSeedRecognition,
};
use crate::raw_ledger_filter::MaterializedLedgerInter;
use crate::recognize::{GridLinesRecognition, NativeBeamRecognition, ScaleRecognition};
use crate::staff_header::{HeaderBounds, StaffHeaderRange};
use crate::stem_seeds_step::{NativeStemCheckResult, NativeStemCounts, NativeStemImpacts};

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
    recognition_json(
        "GRID",
        recognition,
        RecognitionProducts::default(),
        input,
        sheet,
    )
}

/// Emits native GRID and selected HEADERS products on one sheet using schema 1.
#[must_use]
pub fn headers_json(
    recognition: &GridLinesRecognition,
    headers: &NativeHeaderRecognition,
    input: &str,
    sheet: usize,
) -> String {
    recognition_json(
        "HEADERS",
        recognition,
        RecognitionProducts {
            headers: Some(headers),
            ..RecognitionProducts::default()
        },
        input,
        sheet,
    )
}

/// Emits native GRID, HEADERS, and accepted STEM_SEEDS products on one sheet
/// using schema 1.
#[must_use]
pub fn stem_seeds_json(
    recognition: &GridLinesRecognition,
    headers: &NativeHeaderRecognition,
    stem_seeds: &NativeStemSeedRecognition,
    input: &str,
    sheet: usize,
) -> String {
    recognition_json(
        "STEM_SEEDS",
        recognition,
        RecognitionProducts {
            headers: Some(headers),
            stem_seeds: Some(stem_seeds),
            ..RecognitionProducts::default()
        },
        input,
        sheet,
    )
}

/// Emits native GRID, HEADERS, STEM_SEEDS, and BEAMS products on one sheet
/// using schema 1.
#[must_use]
pub fn beams_json(
    recognition: &GridLinesRecognition,
    headers: &NativeHeaderRecognition,
    stem_seeds: &NativeStemSeedRecognition,
    beams: &NativeBeamRecognition,
    input: &str,
    sheet: usize,
) -> String {
    recognition_json(
        "BEAMS",
        recognition,
        RecognitionProducts {
            headers: Some(headers),
            stem_seeds: Some(stem_seeds),
            beams: Some(beams),
            ..RecognitionProducts::default()
        },
        input,
        sheet,
    )
}

/// Emits native GRID, HEADERS, STEM_SEEDS, BEAMS, and final LEDGERS products on
/// one sheet using schema 1.
#[must_use]
pub fn ledgers_json(
    recognition: &GridLinesRecognition,
    headers: &NativeHeaderRecognition,
    stem_seeds: &NativeStemSeedRecognition,
    beams: &NativeBeamRecognition,
    ledgers: &NativeLedgerRecognition,
    input: &str,
    sheet: usize,
) -> String {
    recognition_json(
        "LEDGERS",
        recognition,
        RecognitionProducts {
            headers: Some(headers),
            stem_seeds: Some(stem_seeds),
            beams: Some(beams),
            ledgers: Some(ledgers),
        },
        input,
        sheet,
    )
}

#[derive(Clone, Copy, Default)]
struct RecognitionProducts<'a> {
    headers: Option<&'a NativeHeaderRecognition>,
    stem_seeds: Option<&'a NativeStemSeedRecognition>,
    beams: Option<&'a NativeBeamRecognition>,
    ledgers: Option<&'a NativeLedgerRecognition>,
}

fn recognition_json(
    stage: &str,
    recognition: &GridLinesRecognition,
    products: RecognitionProducts<'_>,
    input: &str,
    sheet: usize,
) -> String {
    let RecognitionProducts {
        headers,
        stem_seeds,
        beams,
        ledgers,
    } = products;
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
    json.field_string("stage", stage);
    json.close('}');
    json.field_string("input", input);
    json.field_integer("sheet", sheet as i64);

    image(&mut json, &recognition.scale);
    scale(&mut json, &recognition.scale);

    json.field_number("slope", recognition.global_slope);

    systems(&mut json, recognition);
    staves(&mut json, recognition);
    if let Some(headers) = headers {
        staff_headers(&mut json, headers);
        header_erases(&mut json, headers);
    }
    let publication = inters(&mut json, recognition, headers, beams, ledgers);
    candidates(&mut json, recognition);
    relations(&mut json, recognition, ledgers, &publication.ledger_ids);
    if let Some(stem_seeds) = stem_seeds {
        stem_scale(&mut json, stem_seeds);
        stem_seeds_records(&mut json, stem_seeds);
    }
    if let Some(beams) = beams {
        beam_groups(&mut json, beams);
    }
    if let Some(ledgers) = ledgers {
        ledger_lines(&mut json, &ledgers.ledger_lines);
    }

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

/// The staff-level HEADERS product. Selected symbols are emitted as inters;
/// this parallel summary keeps the range that positioned them and makes an
/// absent selection distinguishable from an absent staff header.
fn staff_headers(json: &mut Json, headers: &NativeHeaderRecognition) {
    json.key("staff_headers");
    json.open('[');
    for system in &headers.systems {
        for staff in &system.staffs {
            json.open('{');
            json.field_integer("system", system.system_id as i64);
            json.field_integer("staff", staff.staff_id as i64);
            json.field_integer("interline", i64::from(staff.specific_interline));
            match &staff.header {
                Some(header) => {
                    json.field_integer("start", i64::from(header.start));
                    json.field_integer("stop", i64::from(header.stop));
                    optional_id(json, "clef", staff.selected_clef_id);
                    optional_id(json, "key", staff.selected_key_id);
                    optional_id(json, "time", staff.selected_time_id);
                    optional_range(json, "clef_range", header.clef_range.as_ref());
                    optional_range(json, "key_range", header.key_range.as_ref());
                    optional_range(json, "time_range", header.time_range.as_ref());
                }
                None => {
                    json.key("start");
                    json.null();
                    json.key("stop");
                    json.null();
                    optional_id(json, "clef", None);
                    optional_id(json, "key", None);
                    optional_id(json, "time", None);
                    optional_range(json, "clef_range", None);
                    optional_range(json, "key_range", None);
                    optional_range(json, "time_range", None);
                }
            }
            json.close('}');
        }
    }
    json.close(']');
}

fn optional_id(json: &mut Json, name: &str, id: Option<usize>) {
    json.key(name);
    match id {
        Some(id) => json.integer(id as i64),
        None => json.null(),
    }
}

fn optional_range(json: &mut Json, name: &str, range: Option<&StaffHeaderRange>) {
    json.key(name);
    let Some(range) = range else {
        json.null();
        return;
    };
    json.open('{');
    json.field_boolean("valid", range.valid);
    json.field_integer("browse_start", i64::from(range.browse_start));
    json.field_integer("browse_stop", i64::from(range.browse_stop));
    json.key("start");
    match range.start() {
        Ok(start) => json.integer(i64::from(start)),
        Err(_) => json.null(),
    }
    json.key("stop");
    match range.precise_stop() {
        Some(stop) => json.integer(i64::from(stop)),
        None => json.null(),
    }
    json.close('}');
}

/// The exact rectangles the next native stage consumes. Keeping them in the
/// report makes BEAMS' changed pixels attributable to HEADERS rather than to a
/// hidden preprocessing step.
fn header_erases(json: &mut Json, headers: &NativeHeaderRecognition) {
    json.key("header_erases");
    json.open('[');
    for item in &headers.header_erases {
        let erase = item.erase;
        json.open('{');
        json.field_integer("system", item.system_id as i64);
        json.field_integer("x", i64::from(erase.x));
        json.field_integer("stop", i64::from(erase.stop));
        json.field_integer("top", i64::from(erase.top));
        json.field_integer("bottom", i64::from(erase.bottom));
        json.close('}');
    }
    json.close(']');
}

/// The promoted inters, each with the evidence it was graded from.
#[derive(Default)]
struct PublicationIds {
    ledger_ids: BTreeMap<(usize, usize), usize>,
}

fn inters(
    json: &mut Json,
    recognition: &GridLinesRecognition,
    headers: Option<&NativeHeaderRecognition>,
    beams: Option<&NativeBeamRecognition>,
    ledgers: Option<&NativeLedgerRecognition>,
) -> PublicationIds {
    json.key("inters");
    json.open('[');
    let mut next_ids = BTreeMap::<usize, usize>::new();
    for system in &recognition.peak_graph.sig.systems {
        for (id, node) in system.sig.nodes_in_order() {
            next_ids
                .entry(system.system_id)
                .and_modify(|next| *next = (*next).max(id.value()))
                .or_insert(id.value());
            json.open('{');
            json.field_integer("id", id.value() as i64);
            json.field_integer("system", system.system_id as i64);
            json.field_string("status", "accepted");
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
    if let Some(headers) = headers {
        header_inters(json, headers, &mut next_ids);
    }
    if let Some(beams) = beams {
        for (system_id, beam) in beams.raw_beams.iter().chain(&beams.hooks) {
            let id = allocate_publication_id(&mut next_ids, *system_id);
            beam_inter(json, id, *system_id, beam);
        }
    }
    let mut publication = PublicationIds::default();
    if let Some(ledgers) = ledgers {
        for ledger in ledgers.ledgers() {
            let id = allocate_publication_id(&mut next_ids, ledger.system_id);
            publication
                .ledger_ids
                .insert((ledger.system_id, ledger.id), id);
            ledger_inter(json, id, ledger);
        }
    }
    json.close(']');
    publication
}

fn header_inters(
    json: &mut Json,
    headers: &NativeHeaderRecognition,
    next_ids: &mut BTreeMap<usize, usize>,
) {
    for system in &headers.systems {
        for staff in &system.staffs {
            if let Some(id) = staff.selected_clef_id
                && let Some(candidate) = staff.clef_candidates.iter().find(|item| item.id == id)
            {
                observe_publication_id(next_ids, system.system_id, id);
                clef_inter(json, system.system_id, staff, candidate);
            }
            if let Some(id) = staff.selected_key_id
                && let Some(candidate) = staff.key_candidates.iter().find(|item| item.id == id)
            {
                observe_publication_id(next_ids, system.system_id, id);
                key_inter(json, system.system_id, staff, candidate);
            }
            if let Some(id) = staff.selected_time_id
                && let Some(candidate) = staff.time_candidates.iter().find(|item| item.id == id)
            {
                observe_publication_id(next_ids, system.system_id, id);
                time_inter(json, system.system_id, staff, candidate);
            }
        }
    }
}

fn observe_publication_id(next_ids: &mut BTreeMap<usize, usize>, system_id: usize, id: usize) {
    next_ids
        .entry(system_id)
        .and_modify(|next| *next = (*next).max(id))
        .or_insert(id);
}

fn selected_component_is_frozen(
    staff: &NativeHeaderStaffRecognition,
    id: usize,
    component: fn(
        &crate::staff_header::StaffHeader,
    ) -> Option<&crate::staff_header::HeaderComponent>,
) -> bool {
    staff
        .header
        .as_ref()
        .and_then(component)
        .filter(|selected| selected.id == id)
        .is_some_and(crate::staff_header::HeaderComponent::is_frozen)
}

fn clef_component(
    header: &crate::staff_header::StaffHeader,
) -> Option<&crate::staff_header::HeaderComponent> {
    header.clef.as_ref()
}

fn key_component(
    header: &crate::staff_header::StaffHeader,
) -> Option<&crate::staff_header::HeaderComponent> {
    header.key.as_ref()
}

fn time_component(
    header: &crate::staff_header::StaffHeader,
) -> Option<&crate::staff_header::HeaderComponent> {
    header.time.as_ref()
}

struct HeaderInter<'a> {
    id: usize,
    system_id: usize,
    staff_id: usize,
    kind: &'a str,
    bounds_value: HeaderBounds,
    grade: f64,
    contextual_grade: Option<f64>,
}

fn header_inter_start(json: &mut Json, inter: HeaderInter<'_>) {
    json.open('{');
    json.field_integer("id", inter.id as i64);
    json.field_integer("system", inter.system_id as i64);
    json.field_integer("staff", inter.staff_id as i64);
    json.field_string("status", "accepted");
    json.field_string("kind", inter.kind);
    bounds(
        json,
        f64::from(inter.bounds_value.x),
        f64::from(inter.bounds_value.y),
        f64::from(inter.bounds_value.width),
        f64::from(inter.bounds_value.height),
    );
    json.field_number("grade", inter.grade);
    json.key("contextual_grade");
    match inter.contextual_grade {
        Some(grade) => json.number(grade),
        None => json.null(),
    }
}

fn clef_inter(
    json: &mut Json,
    system_id: usize,
    staff: &NativeHeaderStaffRecognition,
    candidate: &NeutralClefCandidate,
) {
    let (kind, clef_kind) = match candidate.kind {
        NeutralClefKind::Treble => ("G_CLEF", "treble"),
        NeutralClefKind::Bass => ("F_CLEF", "bass"),
        NeutralClefKind::Baritone => ("C_CLEF", "baritone"),
        NeutralClefKind::Tenor => ("C_CLEF", "tenor"),
        NeutralClefKind::Alto => ("C_CLEF", "alto"),
        NeutralClefKind::MezzoSoprano => ("C_CLEF", "mezzo-soprano"),
        NeutralClefKind::Soprano => ("C_CLEF", "soprano"),
        NeutralClefKind::Percussion => ("PERCUSSION_CLEF", "percussion"),
    };
    header_inter_start(
        json,
        HeaderInter {
            id: candidate.id,
            system_id,
            staff_id: staff.staff_id,
            kind,
            bounds_value: candidate.bounds,
            grade: candidate.grade,
            contextual_grade: candidate.contextual_grade,
        },
    );
    json.key("evidence");
    json.open('{');
    json.field_boolean(
        "frozen",
        selected_component_is_frozen(staff, candidate.id, clef_component),
    );
    json.field_string("clef_kind", clef_kind);
    optional_id(json, "glyph_id", candidate.glyph_id);
    json.key("glyph_bounds");
    match candidate.glyph_bounds {
        Some(value) => header_bounds(json, value),
        None => json.null(),
    }
    json.field_boolean(
        "original_glyph_registered",
        candidate.original_glyph_registered,
    );
    json.field_boolean("in_sig", candidate.in_sig);
    json.close('}');
    json.close('}');
}

fn key_inter(
    json: &mut Json,
    system_id: usize,
    staff: &NativeHeaderStaffRecognition,
    candidate: &NeutralKeyCandidate,
) {
    header_inter_start(
        json,
        HeaderInter {
            id: candidate.id,
            system_id,
            staff_id: staff.staff_id,
            kind: "KEY_SIGNATURE",
            bounds_value: candidate.bounds,
            grade: candidate.grade,
            contextual_grade: candidate.contextual_grade,
        },
    );
    json.key("evidence");
    json.open('{');
    json.field_boolean(
        "frozen",
        candidate.frozen || selected_component_is_frozen(staff, candidate.id, key_component),
    );
    json.field_integer("fifths", i64::from(candidate.fifths));
    json.field_boolean("in_sig", candidate.in_sig);
    json.key("slices");
    json.open('[');
    for slice in &candidate.slices {
        json.open('{');
        json.field_integer("start", i64::from(slice.start));
        json.field_integer("width", i64::from(slice.width));
        optional_id(json, "alter_id", slice.alter_id);
        json.key("alter_bounds");
        match slice.alter_bounds {
            Some(value) => header_bounds(json, value),
            None => json.null(),
        }
        json.close('}');
    }
    json.close(']');
    json.close('}');
    json.close('}');
}

fn time_inter(
    json: &mut Json,
    system_id: usize,
    staff: &NativeHeaderStaffRecognition,
    candidate: &NeutralTimeCandidate,
) {
    let kind = match candidate.value.specific_shape {
        Some(NeutralSpecificTimeShape::Common) => "COMMON_TIME",
        Some(NeutralSpecificTimeShape::Cut) => "CUT_TIME",
        None => "TIME_SIGNATURE",
    };
    header_inter_start(
        json,
        HeaderInter {
            id: candidate.id,
            system_id,
            staff_id: staff.staff_id,
            kind,
            bounds_value: candidate.symbol_bounds,
            grade: candidate.grade,
            contextual_grade: None,
        },
    );
    json.key("evidence");
    json.open('{');
    json.field_boolean(
        "frozen",
        selected_component_is_frozen(staff, candidate.id, time_component),
    );
    json.field_string(
        "recognition",
        match candidate.kind {
            NeutralTimeCandidateKind::Whole => "whole",
            NeutralTimeCandidateKind::Pair => "pair",
        },
    );
    json.field_integer("numerator", i64::from(candidate.value.numerator));
    json.field_integer("denominator", i64::from(candidate.value.denominator));
    json.key("members");
    json.open('[');
    for id in &candidate.member_ids {
        json.integer(*id as i64);
    }
    json.close(']');
    json.field_boolean(
        "original_glyphs_registered",
        candidate.original_glyphs_registered,
    );
    json.field_boolean("in_sig", candidate.in_sig);
    json.close('}');
    json.close('}');
}

fn header_bounds(json: &mut Json, value: HeaderBounds) {
    json.open('{');
    json.field_integer("x", i64::from(value.x));
    json.field_integer("y", i64::from(value.y));
    json.field_integer("width", i64::from(value.width));
    json.field_integer("height", i64::from(value.height));
    json.close('}');
}

fn allocate_publication_id(next_ids: &mut BTreeMap<usize, usize>, system_id: usize) -> usize {
    let next = next_ids.entry(system_id).or_default();
    *next += 1;
    *next
}

fn bounds(json: &mut Json, x: f64, y: f64, width: f64, height: f64) {
    json.key("bounds");
    json.open('{');
    json.field_number("x", x);
    json.field_number("y", y);
    json.field_number("width", width);
    json.field_number("height", height);
    json.close('}');
}

fn horizontal_median(json: &mut Json, x1: f64, y1: f64, x2: f64, y2: f64) {
    json.key("median");
    json.open('{');
    json.field_number("x1", x1);
    json.field_number("y1", y1);
    json.field_number("x2", x2);
    json.field_number("y2", y2);
    json.close('}');
}

fn beam_inter(json: &mut Json, id: usize, system_id: usize, beam: &RawBeam) {
    json.open('{');
    json.field_integer("id", id as i64);
    json.field_integer("system", system_id as i64);
    json.field_string("status", "accepted");
    json.field_string("kind", beam.kind.shape());
    let beam_box = beam_bounds(beam.item);
    bounds(
        json,
        f64::from(beam_box.x),
        f64::from(beam_box.y),
        f64::from(beam_box.width),
        f64::from(beam_box.height),
    );
    horizontal_median(
        json,
        beam.item.median.x1,
        beam.item.median.y1,
        beam.item.median.x2,
        beam.item.median.y2,
    );
    json.field_number("thickness", beam.item.height);
    json.field_number("grade", beam.grade);
    json.key("contextual_grade");
    json.null();
    json.key("evidence");
    json.open('{');
    json.field_boolean("frozen", false);
    beam_impacts(json, beam.impacts);
    json.close('}');
    json.close('}');
}

fn beam_impacts(json: &mut Json, impacts: BeamImpacts) {
    json.key("impacts");
    json.open('{');
    json.field_number("width", impacts.width);
    json.field_number("min_height", impacts.min_height);
    json.field_number("max_height", impacts.max_height);
    json.field_number("core", impacts.core);
    json.field_number("belt", impacts.belt);
    // Java names this the border-jitter impact. The Rust recognition kernel
    // calls the normalized value `distance`, but publishing the internal name
    // would make the same Audiveris evidence look like two different terms.
    json.field_number("jitter", impacts.distance);
    json.close('}');
}

fn ledger_inter(json: &mut Json, id: usize, ledger: &MaterializedLedgerInter) {
    json.open('{');
    json.field_integer("id", id as i64);
    json.field_integer("system", ledger.system_id as i64);
    json.field_integer("staff", ledger.staff_id as i64);
    json.field_string("status", "accepted");
    json.field_string("kind", "LEDGER");
    json.field_integer("ledger_index", i64::from(ledger.ledger_index));
    json.field_integer("pitch", i64::from(ledger.pitch));
    bounds(
        json,
        ledger.bounds.x,
        ledger.bounds.y,
        ledger.bounds.width,
        ledger.bounds.height,
    );
    horizontal_median(
        json,
        ledger.median.0.0,
        ledger.median.0.1,
        ledger.median.1.0,
        ledger.median.1.1,
    );
    json.field_number("thickness", ledger.thickness);
    json.field_number("grade", ledger.grade);
    json.key("contextual_grade");
    json.null();
    json.key("evidence");
    json.open('{');
    json.field_boolean("frozen", false);
    json.key("impacts");
    json.open('{');
    for (name, impact) in [
        "min_thickness",
        "max_thickness",
        "length",
        "convexity",
        "straightness",
        "left_pitch",
        "right_pitch",
    ]
    .into_iter()
    .zip(ledger.impacts)
    {
        json.field_number(name, impact.grade);
    }
    json.close('}');
    json.close('}');
    json.close('}');
}

/// The candidates that lost, and which stage rejected them.
///
/// A recogniser that emits only its answer cannot be judged on what it missed.
/// Every barline in `inters` beat some number of these, and a consumer asking
/// "should there have been a barline here?" needs the near-miss and the reason
/// it was dropped -- `Unaligned` and `CClef` are very different claims about
/// the same missing barline.
///
/// These are rejections from the `BarsRetriever` purges specifically. They are
/// not a complete n-best list: a peak that never reached the purges, because it
/// failed core validation or graded below `Grades.minInterGrade`, is not here.
/// That is a real limit of this schema version, not an assertion that no other
/// candidates existed.
fn candidates(json: &mut Json, recognition: &GridLinesRecognition) {
    json.key("candidates");
    json.open('[');
    for rejection in &recognition.peak_graph.rejections {
        json.open('{');
        json.field_string("kind", "BARLINE");
        json.field_string("status", "rejected");
        json.field_integer("system", rejection.system as i64);
        json.field_integer("staff", rejection.staff as i64);
        json.key("span");
        json.open('{');
        json.field_integer("start", i64::from(rejection.start));
        json.field_integer("stop", i64::from(rejection.stop));
        json.close('}');
        json.key("evidence");
        json.open('{');
        json.field_string("rejected_by", &format!("{:?}", rejection.stage));
        json.close('}');
        json.close('}');
    }
    json.close(']');
}

/// The support and exclusion edges, which are what a contextual grade is made
/// of and the first thing to look at when one is wrong.
fn relations(
    json: &mut Json,
    recognition: &GridLinesRecognition,
    ledgers: Option<&NativeLedgerRecognition>,
    ledger_ids: &BTreeMap<(usize, usize), usize>,
) {
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
    if let Some(ledgers) = ledgers {
        for relation in ledgers
            .materializer
            .relations()
            .iter()
            .filter(|relation| !relation.removed)
        {
            let Some(source) = ledger_ids
                .get(&(relation.system_id, relation.source_inter_id))
                .copied()
            else {
                continue;
            };
            let Some(target) = ledger_ids
                .get(&(relation.system_id, relation.target_inter_id))
                .copied()
            else {
                continue;
            };
            json.open('{');
            json.field_integer("system", relation.system_id as i64);
            json.field_integer("source", source as i64);
            json.field_integer("target", target as i64);
            json.field_string("kind", "exclusion");
            json.close('}');
        }
    }
    json.close(']');
}

fn stem_scale(json: &mut Json, recognition: &NativeStemSeedRecognition) {
    json.key("stem_scale");
    json.open('{');
    json.field_integer("maximum", i64::from(recognition.maximum_stem_thickness));
    json.close('}');
}

struct AcceptedStemSeed<'a> {
    system_id: usize,
    gate: &'a NativeStemSeedGate,
    threshold: f64,
    weights: NativeStemImpacts,
    check: &'a NativeStemCheckResult,
    registered_glyph_index: usize,
    free_glyph_index: usize,
    glyph: &'a NativeStemSeedGlyph,
}

/// Accepted free glyphs, in system order and then raw-candidate order.
///
/// STEM_SEEDS does not create SIG inters, and the native boundary deliberately
/// has no invented glyph identifier. `{system, ordinal}` is therefore the
/// published identity; adding these to `inters` would misrepresent Java state
/// and perturb the stage-local IDs later serializers allocate.
fn stem_seeds_records(json: &mut Json, recognition: &NativeStemSeedRecognition) {
    json.key("stem_seeds");
    json.open('[');
    for system in &recognition.systems {
        for decision in &system.decisions {
            let NativeStemSeedDecision::Checked {
                gate,
                threshold,
                weights,
                check,
                registered_glyph_index,
                accepted,
                free_glyph_index,
            } = decision
            else {
                continue;
            };
            if !accepted {
                continue;
            }
            let free_glyph_index = (*free_glyph_index)
                .expect("accepted native stem seed must have a free-glyph index");
            let glyph = system
                .free_glyphs
                .get(free_glyph_index)
                .expect("accepted native stem seed must resolve its free glyph");
            assert_eq!(
                glyph.source_ordinal, gate.ordinal,
                "accepted native stem seed must preserve its raw ordinal"
            );
            stem_seed_record(
                json,
                AcceptedStemSeed {
                    system_id: system.raw.system_id,
                    gate,
                    threshold: *threshold,
                    weights: *weights,
                    check,
                    registered_glyph_index: *registered_glyph_index,
                    free_glyph_index,
                    glyph,
                },
            );
        }
    }
    json.close(']');
}

fn stem_seed_record(json: &mut Json, seed: AcceptedStemSeed<'_>) {
    json.open('{');
    json.field_integer("system", seed.system_id as i64);
    json.field_integer("ordinal", seed.gate.ordinal as i64);
    json.field_string("status", "accepted");
    json.field_string("kind", "VERTICAL_SEED");
    optional_id(json, "staff", seed.gate.staff_id);
    bounds(
        json,
        seed.glyph.bounds.x as f64,
        seed.glyph.bounds.y as f64,
        seed.glyph.bounds.width as f64,
        seed.glyph.bounds.height as f64,
    );
    horizontal_median(
        json,
        seed.glyph.start.0,
        seed.glyph.start.1,
        seed.glyph.stop.0,
        seed.glyph.stop.1,
    );
    json.field_number("thickness", seed.glyph.mean_thickness);
    json.field_integer("weight", seed.glyph.weight as i64);
    json.field_number("grade", seed.check.grade);
    json.key("evidence");
    json.open('{');
    json.field_number("threshold", seed.threshold);
    json.key("center");
    json.open('{');
    json.field_number("x", seed.gate.center.0);
    json.field_number("y", seed.gate.center.1);
    json.close('}');
    json.key("header_stop");
    match seed.gate.header_stop {
        Some(stop) => json.integer(i64::from(stop)),
        None => json.null(),
    }
    json.field_boolean("tablature", seed.gate.tablature);
    json.field_integer("registered_glyph_index", seed.registered_glyph_index as i64);
    json.field_integer("free_glyph_index", seed.free_glyph_index as i64);
    stem_impacts(json, "values", seed.check.values);
    stem_impacts(json, "weights", seed.weights);
    stem_impacts(json, "impacts", seed.check.impacts);
    stem_counts(json, seed.check.counts);
    json.field_number("mean_distance", seed.glyph.mean_distance);
    json.field_integer("run_count", seed.glyph.run_count() as i64);
    json.field_string("run_digest", &format!("{:016x}", seed.glyph.run_digest()));
    json.field_boolean("vertical_seed_group", seed.glyph.vertical_seed_group);
    json.field_boolean("free", seed.glyph.free);
    json.close('}');
    json.close('}');
}

fn stem_impacts(json: &mut Json, name: &str, impacts: NativeStemImpacts) {
    json.key(name);
    json.open('{');
    for (name, value) in [
        ("slope", impacts.slope),
        ("straight", impacts.straight),
        ("length", impacts.length),
        ("clean", impacts.clean),
        ("black", impacts.black),
        ("black_ratio", impacts.black_ratio),
        ("gap", impacts.gap),
    ] {
        json.field_number(name, value);
    }
    json.close('}');
}

fn stem_counts(json: &mut Json, counts: NativeStemCounts) {
    json.key("counts");
    json.open('{');
    json.field_integer("largest_gap", i64::from(counts.largest_gap));
    json.field_integer("white", i64::from(counts.white));
    json.field_integer("black", i64::from(counts.black));
    json.field_integer("left", i64::from(counts.left));
    json.field_integer("right", i64::from(counts.right));
    json.field_integer("both", i64::from(counts.both));
    json.field_integer("clean", i64::from(counts.clean));
    json.close('}');
}

fn beam_groups(json: &mut Json, beams: &NativeBeamRecognition) {
    json.key("beam_groups");
    json.open('[');
    for (system_id, count) in &beams.group_counts {
        json.open('{');
        json.field_integer("system", *system_id as i64);
        json.field_integer("count", *count as i64);
        json.close('}');
    }
    json.close(']');
}

fn ledger_lines(json: &mut Json, lines: &[NativeLedgerLine]) {
    json.key("ledger_lines");
    json.open('[');
    for line in lines {
        json.open('{');
        json.field_integer("system", line.system_id as i64);
        json.field_integer("staff", line.staff_id as i64);
        json.field_integer("ledger_index", i64::from(line.index));
        json.field_number("translation_y", line.translation_y);
        json.key("path");
        json.open('[');
        for segment in &line.geometry.segments {
            boundary_segment(json, *segment);
        }
        json.close(']');
        json.close('}');
    }
    json.close(']');
}

fn point(json: &mut Json, name: &str, point: (f64, f64)) {
    json.key(name);
    json.open('{');
    json.field_number("x", point.0);
    json.field_number("y", point.1);
    json.close('}');
}

fn boundary_segment(json: &mut Json, segment: BoundarySegment) {
    json.open('{');
    match segment {
        BoundarySegment::Line { start, end } => {
            json.field_string("kind", "line");
            point(json, "start", start);
            point(json, "end", end);
        }
        BoundarySegment::Quadratic {
            start,
            control,
            end,
        } => {
            json.field_string("kind", "quadratic");
            point(json, "start", start);
            point(json, "control", control);
            point(json, "end", end);
        }
        BoundarySegment::Cubic {
            start,
            control1,
            control2,
            end,
        } => {
            json.field_string("kind", "cubic");
            point(json, "start", start);
            point(json, "control1", control1);
            point(json, "control2", control2);
            point(json, "end", end);
        }
    }
    json.close('}');
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{
        beam_inters::BeamKind,
        header_time_column::NeutralTimeValue,
        key_column::NeutralKeySlice,
        native_headers::{
            NativeHeaderErase, NativeHeaderStaffRecognition, NativeHeaderSystemRecognition,
        },
        raw_ledger_filter::{LedgerCandidateImpact, LedgerFloatBounds},
        staff_header::{HeaderComponent, StaffHeader},
    };
    use audiveris_image::{
        beam_structure::{BeamItem, BeamRasterEvidence, Segment},
        run_table::{BACKGROUND, FOREGROUND, Orientation, RunTable},
        section::Bounds,
        spots::HeaderErase,
    };

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

    #[test]
    fn accepted_stem_seed_publication_keeps_identity_geometry_and_exact_evidence() {
        let run_table = RunTable::from_pixels(
            Orientation::Vertical,
            2,
            3,
            &[
                FOREGROUND, BACKGROUND, FOREGROUND, FOREGROUND, BACKGROUND, FOREGROUND,
            ],
        )
        .expect("stem glyph run table");
        let glyph = NativeStemSeedGlyph {
            source_ordinal: 17,
            vertical_seed_group: true,
            free: true,
            bounds: Bounds {
                x: 20,
                y: 30,
                width: 2,
                height: 3,
            },
            weight: 4,
            start: (20.25, 30.5),
            stop: (21.0, 32.75),
            mean_thickness: 1.5,
            mean_distance: 0.125,
            run_table,
        };
        let gate = NativeStemSeedGate {
            ordinal: 17,
            center: (21.0, 31.5),
            staff_id: Some(5),
            header_stop: Some(19),
            tablature: false,
        };
        let values = NativeStemImpacts {
            slope: 0.1,
            straight: 0.2,
            length: 0.3,
            clean: 0.4,
            black: 0.5,
            black_ratio: 0.6,
            gap: 0.7,
        };
        let weights = NativeStemImpacts {
            slope: 1.0,
            straight: 2.0,
            length: 3.0,
            clean: 4.0,
            black: 5.0,
            black_ratio: 6.0,
            gap: 7.0,
        };
        let impacts = NativeStemImpacts {
            slope: 0.11,
            straight: 0.22,
            length: 0.33,
            clean: 0.44,
            black: 0.55,
            black_ratio: 0.66,
            gap: 0.77,
        };
        let check = NativeStemCheckResult {
            values,
            impacts,
            counts: NativeStemCounts {
                largest_gap: 1,
                white: 2,
                black: 3,
                left: 4,
                right: 5,
                both: 6,
                clean: 7,
            },
            grade: 0.875,
        };
        let expected_digest = format!("{:016x}", glyph.run_digest());
        let mut json = Json::default();
        stem_seed_record(
            &mut json,
            AcceptedStemSeed {
                system_id: 2,
                gate: &gate,
                threshold: 0.2,
                weights,
                check: &check,
                registered_glyph_index: 11,
                free_glyph_index: 9,
                glyph: &glyph,
            },
        );

        assert!(structural_faults(&json.out).is_empty(), "{}", json.out);
        assert!(json.out.starts_with(
            r#"{"system":2,"ordinal":17,"status":"accepted","kind":"VERTICAL_SEED","staff":5"#
        ));
        assert!(
            json.out
                .contains(r#""bounds":{"x":20.0,"y":30.0,"width":2.0,"height":3.0}"#)
        );
        assert!(
            json.out
                .contains(r#""median":{"x1":20.25,"y1":30.5,"x2":21.0,"y2":32.75}"#)
        );
        assert!(json.out.contains(
            r#""values":{"slope":0.1,"straight":0.2,"length":0.3,"clean":0.4,"black":0.5,"black_ratio":0.6,"gap":0.7}"#
        ));
        assert!(json.out.contains(
            r#""weights":{"slope":1.0,"straight":2.0,"length":3.0,"clean":4.0,"black":5.0,"black_ratio":6.0,"gap":7.0}"#
        ));
        assert!(json.out.contains(
            r#""impacts":{"slope":0.11,"straight":0.22,"length":0.33,"clean":0.44,"black":0.55,"black_ratio":0.66,"gap":0.77}"#
        ));
        assert!(json.out.contains(
            r#""counts":{"largest_gap":1,"white":2,"black":3,"left":4,"right":5,"both":6,"clean":7}"#
        ));
        assert!(
            json.out
                .contains(&format!(r#""run_digest":"{expected_digest}""#))
        );
        assert!(
            !json.out.contains(r#""id":"#),
            "no glyph/inter id is invented"
        );
    }

    #[test]
    fn selected_header_publication_keeps_ranges_and_classifier_evidence() {
        let bounds = HeaderBounds {
            x: 20,
            y: 30,
            width: 12,
            height: 40,
        };
        let mut clef_component = HeaderComponent::new(10, bounds);
        clef_component.freeze();
        let mut key_component = HeaderComponent::new(110, bounds);
        key_component.freeze();
        let mut time_component = HeaderComponent::new(210, bounds);
        time_component.freeze();
        let mut range = StaffHeaderRange::default();
        range.valid = true;
        range.browse_start = 12;
        range.browse_stop = 99;
        range.set_start(20);
        range.set_stop(80);
        let staff = NativeHeaderStaffRecognition {
            staff_id: 3,
            specific_interline: 20,
            header: Some(StaffHeader {
                start: 12,
                stop: 91,
                clef: Some(clef_component),
                key: Some(key_component),
                time: Some(time_component),
                clef_range: Some(range.clone()),
                key_range: Some(range.clone()),
                alter_starts: Some(vec![40, 50, 60]),
                time_range: Some(range.clone()),
            }),
            clef_candidates: vec![NeutralClefCandidate {
                id: 10,
                kind: NeutralClefKind::Bass,
                grade: 0.7,
                contextual_grade: Some(0.8),
                bounds,
                glyph_id: Some(501),
                glyph_bounds: Some(bounds),
                in_sig: true,
                staff_id: Some(3),
                original_glyph_registered: true,
                removed: false,
            }],
            selected_clef_id: Some(10),
            key_candidates: vec![NeutralKeyCandidate {
                id: 110,
                fifths: -3,
                grade: 0.6,
                contextual_grade: Some(0.75),
                bounds,
                range: range.clone(),
                slices: vec![NeutralKeySlice {
                    start: 40,
                    width: 5,
                    alter_id: Some(111),
                    alter_bounds: Some(bounds),
                }],
                in_sig: true,
                staff_id: Some(3),
                frozen: true,
                removed: false,
            }],
            selected_key_id: Some(110),
            time_candidates: vec![NeutralTimeCandidate {
                id: 210,
                kind: NeutralTimeCandidateKind::Pair,
                value: NeutralTimeValue {
                    specific_shape: Some(NeutralSpecificTimeShape::Common),
                    numerator: 4,
                    denominator: 4,
                },
                grade: 0.9,
                symbol_bounds: bounds,
                member_ids: vec![211, 212],
                staff_id: Some(3),
                in_sig: true,
                original_glyphs_registered: true,
                removed: false,
            }],
            selected_time_id: Some(210),
        };
        let headers = NativeHeaderRecognition {
            sheet_interline: 20,
            systems: vec![NativeHeaderSystemRecognition {
                system_id: 2,
                staffs: vec![staff],
                time_value: None,
            }],
            header_erases: vec![NativeHeaderErase {
                system_id: 2,
                erase: HeaderErase {
                    x: 10,
                    stop: 91,
                    top: 5,
                    bottom: 120,
                },
            }],
        };
        let mut json = Json::default();
        json.open('{');
        staff_headers(&mut json, &headers);
        header_erases(&mut json, &headers);
        json.key("inters");
        json.open('[');
        let mut next_ids = BTreeMap::new();
        header_inters(&mut json, &headers, &mut next_ids);
        json.close(']');
        json.close('}');

        assert!(structural_faults(&json.out).is_empty(), "{}", json.out);
        assert!(
            json.out
                .contains(r#""start":12,"stop":91,"clef":10,"key":110,"time":210"#)
        );
        assert!(json.out.contains(
            r#""clef_range":{"valid":true,"browse_start":12,"browse_stop":99,"start":20,"stop":80}"#
        ));
        assert!(
            json.out.contains(
                r#""header_erases":[{"system":2,"x":10,"stop":91,"top":5,"bottom":120}]"#
            )
        );
        assert!(json.out.contains(r#""kind":"F_CLEF""#));
        assert!(json.out.contains(r#""clef_kind":"bass","glyph_id":501"#));
        assert!(json.out.contains(r#""kind":"KEY_SIGNATURE""#));
        assert!(json.out.contains(r#""fifths":-3"#));
        assert!(json.out.contains(r#""kind":"COMMON_TIME""#));
        assert!(
            json.out.contains(
                r#""recognition":"pair","numerator":4,"denominator":4,"members":[211,212]"#
            )
        );
        assert_eq!(next_ids, BTreeMap::from([(2, 210)]));
        assert_eq!(allocate_publication_id(&mut next_ids, 2), 211);
    }

    #[test]
    fn beam_publication_keeps_horizontal_geometry_and_all_six_impacts() {
        let beam = RawBeam {
            kind: BeamKind::Hook,
            item: BeamItem {
                median: Segment {
                    x1: 10.25,
                    y1: 20.5,
                    x2: 30.75,
                    y2: 21.5,
                },
                height: 4.0,
            },
            impacts: BeamImpacts {
                width: 0.1,
                min_height: 0.2,
                max_height: 0.3,
                core: 0.4,
                belt: 0.5,
                distance: 0.6,
                raster: BeamRasterEvidence {
                    core_foreground: 1,
                    core_count: 2,
                    belt_foreground: 3,
                    belt_count: 4,
                    core_ratio: 0.5,
                    belt_ratio: 0.75,
                    rounded_width: 21,
                },
            },
            grade: 0.7,
        };
        let mut json = Json::default();
        beam_inter(&mut json, 9, 2, &beam);

        assert!(structural_faults(&json.out).is_empty(), "{}", json.out);
        assert!(json.out.contains(r#""kind":"BEAM_HOOK""#));
        assert!(
            json.out
                .contains(r#""median":{"x1":10.25,"y1":20.5,"x2":30.75,"y2":21.5}"#)
        );
        assert!(json.out.contains(
            r#""impacts":{"width":0.1,"min_height":0.2,"max_height":0.3,"core":0.4,"belt":0.5,"jitter":0.6}"#
        ));
    }

    #[test]
    fn ledger_publication_uses_normalized_impacts_and_stage_local_identity() {
        let impacts = [
            LedgerCandidateImpact {
                value: 10.0,
                grade: 0.1,
                weight: 1.0,
            },
            LedgerCandidateImpact {
                value: 20.0,
                grade: 0.2,
                weight: 1.0,
            },
            LedgerCandidateImpact {
                value: 30.0,
                grade: 0.3,
                weight: 1.0,
            },
            LedgerCandidateImpact {
                value: 40.0,
                grade: 0.4,
                weight: 1.0,
            },
            LedgerCandidateImpact {
                value: 50.0,
                grade: 0.5,
                weight: 1.0,
            },
            LedgerCandidateImpact {
                value: 60.0,
                grade: 0.6,
                weight: 1.0,
            },
            LedgerCandidateImpact {
                value: 70.0,
                grade: 0.7,
                weight: 1.0,
            },
        ];
        let ledger = MaterializedLedgerInter {
            id: 3,
            glyph_id: 4,
            filament_id: 5,
            system_id: 2,
            staff_id: 7,
            ledger_index: -2,
            pitch: -8,
            bounds: LedgerFloatBounds {
                x: 100.0,
                y: 200.0,
                width: 30.0,
                height: 3.0,
            },
            median: ((100.0, 201.0), (129.0, 202.0)),
            thickness: 2.5,
            grade: 0.75,
            impacts,
            removed: false,
        };
        let mut json = Json::default();
        ledger_inter(&mut json, 42, &ledger);

        assert!(structural_faults(&json.out).is_empty(), "{}", json.out);
        assert!(json.out.contains(r#""id":42,"system":2,"staff":7"#));
        assert!(json.out.contains(r#""ledger_index":-2,"pitch":-8"#));
        assert!(json.out.contains(
            r#""impacts":{"min_thickness":0.1,"max_thickness":0.2,"length":0.3,"convexity":0.4,"straightness":0.5,"left_pitch":0.6,"right_pitch":0.7}"#
        ));
        assert!(
            !json.out.contains("10.0"),
            "raw measurements are not impacts"
        );

        let mut next = BTreeMap::from([(2, 41)]);
        assert_eq!(allocate_publication_id(&mut next, 2), 42);
        assert_eq!(allocate_publication_id(&mut next, 1), 1);
    }

    #[test]
    fn ledger_line_publication_preserves_curve_controls() {
        let lines = [NativeLedgerLine {
            system_id: 3,
            staff_id: 6,
            index: -1,
            translation_y: -20.25,
            geometry: audiveris_image::system_population::StaffBoundary {
                segments: vec![BoundarySegment::Cubic {
                    start: (1.0, 2.0),
                    control1: (3.0, 4.0),
                    control2: (5.0, 6.0),
                    end: (7.0, 8.0),
                }],
            },
        }];
        let mut json = Json::default();
        ledger_lines(&mut json, &lines);

        assert!(structural_faults(&json.out).is_empty(), "{}", json.out);
        assert!(json.out.contains(r#""translation_y":-20.25"#));
        assert!(json.out.contains(r#""kind":"cubic""#));
        assert!(json.out.contains(r#""control1":{"x":3.0,"y":4.0}"#));
        assert!(json.out.contains(r#""control2":{"x":5.0,"y":6.0}"#));
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
