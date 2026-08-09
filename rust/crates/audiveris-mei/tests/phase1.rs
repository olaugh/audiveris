// SPDX-License-Identifier: AGPL-3.0-or-later

#[path = "../examples/support/mod.rs"]
mod support;

use std::collections::HashSet;
use std::io;

use audiveris_mei::{
    Duration, Event, MeiCounts, MeiExportError, MetricalConformance, PinnedDate, StaffNumber,
    ViolationCode, serialize_mei, write_mei,
};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event as XmlEvent};

const GOLDEN: &[u8] = include_bytes!("golden/phase1.mei");

#[test]
fn phase_one_bytes_and_counts_are_exact() {
    let output = serialize_mei(&support::phase_one_document()).expect("valid Phase 1 document");
    assert_eq!(output.as_bytes(), GOLDEN);
    assert_eq!(
        output.counts(),
        MeiCounts {
            parts: 1,
            staves: 2,
            measures: 2,
            layers: 4,
            notes: 6,
            rests: 2,
            chords: 1,
            beams: 1,
            measure_barlines: 2,
            local_barlines: 1,
        }
    );
}

#[test]
fn serialization_is_byte_deterministic_and_has_no_default_date() {
    let document = support::phase_one_document();
    let first = serialize_mei(&document).expect("first serialization");
    let second = serialize_mei(&document).expect("second serialization");
    assert_eq!(first, second);
    assert!(!String::from_utf8_lossy(first.as_bytes()).contains("isodate="));
    assert!(first.as_bytes().ends_with(b"\n"));
    assert!(!first.as_bytes().starts_with(&[0xEF, 0xBB, 0xBF]));

    let mut escaped = document;
    escaped.source.target = Some("file:///scan.png?sheet=1&mode=gray".to_owned());
    let xml = String::from_utf8(
        serialize_mei(&escaped)
            .expect("escaped attribute")
            .into_bytes(),
    )
    .expect("UTF-8 XML");
    assert!(xml.contains("target=\"file:///scan.png?sheet=1&amp;mode=gray\""));

    let mut invalid_target = support::phase_one_document();
    invalid_target.source.target = Some("unresolved-source".to_owned());
    assert_codes(invalid_target, &[ViolationCode::InvalidSourceTarget]);

    for malformed in ["x:<", "file://[", "./%ZZ"] {
        let mut invalid_target = support::phase_one_document();
        invalid_target.source.target = Some(malformed.to_owned());
        assert_codes(invalid_target, &[ViolationCode::InvalidSourceTarget]);
    }
}

#[test]
fn independent_xml_reader_resolves_ids_responsibility_and_namespace() {
    let mut reader = Reader::from_reader(GOLDEN);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut ids = HashSet::new();
    let mut responsibilities = Vec::new();
    let mut saw_root = false;
    let mut notes = 0;

    loop {
        match reader
            .read_event_into(&mut buffer)
            .expect("well-formed XML")
        {
            XmlEvent::Start(start) | XmlEvent::Empty(start) => {
                if start.name().as_ref() == b"mei" {
                    saw_root = true;
                    assert_eq!(
                        attribute(&start, b"xmlns").as_deref(),
                        Some("http://www.music-encoding.org/ns/mei")
                    );
                    assert_eq!(attribute(&start, b"meiversion").as_deref(), Some("5.1+CMN"));
                }
                if start.name().as_ref() == b"note" {
                    notes += 1;
                }
                if let Some(id) = attribute(&start, b"xml:id") {
                    assert!(ids.insert(id), "duplicate xml:id");
                }
                if let Some(resp) = attribute(&start, b"resp") {
                    responsibilities.push(resp);
                }
            }
            XmlEvent::Eof => break,
            _ => {}
        }
        buffer.clear();
    }

    assert!(saw_root);
    assert_eq!(notes, 6);
    assert_eq!(responsibilities.len(), 11);
    for responsibility in responsibilities {
        let target = responsibility
            .strip_prefix('#')
            .expect("local responsibility reference");
        assert!(ids.contains(target), "unresolved responsibility {target}");
    }
}

#[test]
fn cross_staff_target_is_pinned_on_the_attached_note() {
    let output = serialize_mei(&support::phase_one_document()).expect("valid document");
    let xml = String::from_utf8(output.into_bytes()).expect("UTF-8 XML");
    assert!(xml.contains(
        "<note xml:id=\"note.i300\" pname=\"d\" oct=\"5\" dur=\"1\" staff=\"2\" resp=\"#app.audiveris-rust\""
    ));
}

#[test]
fn duration_spelling_is_pinned_on_the_attached_note() {
    let mut document = support::phase_one_document();
    let Event::Note(note) = &mut document.score.measures[1].staves[0].layers[0].events[0] else {
        panic!("sample event changed")
    };
    note.duration = Duration::Quarter;
    document.score.measures[1].metrical_conformance = MetricalConformance::NonConformant;
    let output = serialize_mei(&document).expect("mutated document remains semantically valid");
    let xml = String::from_utf8(output.into_bytes()).expect("UTF-8 XML");
    assert!(xml.contains(
        "<note xml:id=\"note.i300\" pname=\"d\" oct=\"5\" dur=\"4\" staff=\"2\" resp=\"#app.audiveris-rust\""
    ));
    assert_ne!(xml.as_bytes(), GOLDEN);
}

#[test]
fn duplicate_semantic_event_identity_is_one_named_error() {
    let mut document = support::phase_one_document();
    let Event::MeasureRest(rest) = &mut document.score.measures[1].staves[1].layers[0].events[0]
    else {
        panic!("sample event changed")
    };
    rest.id = support::stable(100);
    assert_codes(document, &[ViolationCode::DuplicateEventId]);
}

#[test]
fn undersized_chord_and_beam_fail_closed() {
    let mut chord_document = support::phase_one_document();
    let Event::Chord(chord) = &mut chord_document.score.measures[0].staves[0].layers[0].events[2]
    else {
        panic!("sample chord changed")
    };
    chord.notes.truncate(1);
    assert_codes(chord_document, &[ViolationCode::ChordTooShort]);

    let mut beam_document = support::phase_one_document();
    beam_document.score.measures[0].metrical_conformance = MetricalConformance::NonConformant;
    let Event::Beam(beam) = &mut beam_document.score.measures[0].staves[0].layers[0].events[1]
    else {
        panic!("sample beam changed")
    };
    beam.events.truncate(1);
    assert_codes(beam_document, &[ViolationCode::BeamTooShort]);

    let mut duration_document = support::phase_one_document();
    duration_document.score.measures[0].metrical_conformance = MetricalConformance::NonConformant;
    let Event::Beam(beam) = &mut duration_document.score.measures[0].staves[0].layers[0].events[1]
    else {
        panic!("sample beam changed")
    };
    let audiveris_mei::BeamEvent::Note(note) = &mut beam.events[0] else {
        panic!("sample beam child changed")
    };
    note.duration = Duration::Whole;
    assert_codes(duration_document, &[ViolationCode::UnbeamableDuration]);
}

#[test]
fn rng_measure_number_word_domain_is_enforced_before_writing() {
    let mut document = support::phase_one_document();
    document.score.measures[0].number = "1 a".to_owned();
    assert_codes(document, &[ViolationCode::InvalidMeasureNumber]);

    let mut combining = support::phase_one_document();
    combining.score.measures[0].number = "e\u{301}".to_owned();
    assert_codes(combining, &[ViolationCode::InvalidMeasureNumber]);
}

#[test]
fn metrical_conformance_and_full_measure_rests_are_explicit() {
    let output = serialize_mei(&support::phase_one_document()).expect("conformant sample");
    let xml = String::from_utf8(output.into_bytes()).expect("UTF-8 XML");
    assert_eq!(xml.matches("metcon=\"true\"").count(), 2);
    assert_eq!(xml.matches("<mRest ").count(), 2);

    let mut underfilled = support::phase_one_document();
    let Event::Note(note) = &mut underfilled.score.measures[1].staves[0].layers[0].events[0] else {
        panic!("sample event changed")
    };
    note.duration = Duration::Half;
    assert_codes(underfilled, &[ViolationCode::MetricalMismatch]);

    let mut declared = support::phase_one_document();
    declared.score.measures[1].metrical_conformance = MetricalConformance::NonConformant;
    let Event::Note(note) = &mut declared.score.measures[1].staves[0].layers[0].events[0] else {
        panic!("sample event changed")
    };
    note.duration = Duration::Half;
    let xml = String::from_utf8(
        serialize_mei(&declared)
            .expect("explicitly nonconformant measure")
            .into_bytes(),
    )
    .expect("UTF-8 XML");
    assert!(xml.contains("measure.i21\" n=\"2\" metcon=\"false\""));

    let mut false_claim = support::phase_one_document();
    false_claim.score.measures[1].metrical_conformance = MetricalConformance::NonConformant;
    assert_codes(false_claim, &[ViolationCode::FalseMetricalNonconformance]);

    let mut mixed_rest = support::phase_one_document();
    mixed_rest.score.measures[1].metrical_conformance = MetricalConformance::NonConformant;
    let mut extra = mixed_rest.score.measures[1].staves[1].layers[0].events[0].clone();
    let Event::MeasureRest(rest) = &mut extra else {
        panic!("sample measure rest changed")
    };
    rest.id = support::stable(999);
    mixed_rest.score.measures[1].staves[1].layers[0]
        .events
        .push(extra);
    assert_codes(mixed_rest, &[ViolationCode::MeasureRestNotAlone]);

    let mut invalid_meter = support::phase_one_document();
    invalid_meter.score.parts[0].staves[0].meter = Some(audiveris_mei::Meter { count: 4, unit: 0 });
    assert_codes(invalid_meter, &[ViolationCode::InvalidMeter]);
}

#[test]
fn unknown_staff_target_fails_exactly_the_context_gate() {
    let mut document = support::phase_one_document();
    let Event::Note(note) = &mut document.score.measures[1].staves[0].layers[0].events[0] else {
        panic!("sample event changed")
    };
    note.context.staff = StaffNumber::new(99).expect("positive staff");
    assert_codes(document, &[ViolationCode::UnknownEventContext]);
}

#[test]
fn measure_staff_reordering_fails_exactly_the_topology_gate() {
    let mut document = support::phase_one_document();
    document.score.measures[1].staves.swap(0, 1);
    assert_codes(document, &[ViolationCode::MeasureStaffTopology]);
}

#[test]
fn invalid_input_writes_nothing_and_io_failures_propagate() {
    let mut invalid = support::phase_one_document();
    invalid.title.clear();
    let mut destination = Vec::new();
    let error = write_mei(&invalid, &mut destination).expect_err("empty title must fail");
    assert!(matches!(error, MeiExportError::Validation(_)));
    assert!(destination.is_empty());

    let error = write_mei(&support::phase_one_document(), RejectAll)
        .expect_err("destination failure must propagate");
    assert!(matches!(error, MeiExportError::Io(_)));
}

#[test]
fn pinned_dates_validate_calendar_days_without_reading_the_clock() {
    let date = PinnedDate::new("2026-08-09").expect("valid pinned date");
    assert!(PinnedDate::new("2024-02-29").is_ok());
    assert!(PinnedDate::new("2025-02-29").is_err());
    assert!(PinnedDate::new("2026-8-9").is_err());

    let mut document = support::phase_one_document();
    document.application.isodate = Some(date);
    let xml = String::from_utf8(
        serialize_mei(&document)
            .expect("explicit pinned date")
            .into_bytes(),
    )
    .expect("UTF-8 XML");
    assert!(xml.contains("isodate=\"2026-08-09\""));
}

fn attribute(start: &BytesStart<'_>, name: &[u8]) -> Option<String> {
    start
        .attributes()
        .map(|attribute| attribute.expect("valid XML attribute"))
        .find(|attribute| attribute.key.as_ref() == name)
        .map(|attribute| {
            attribute
                .unescape_value()
                .expect("valid escaped value")
                .into_owned()
        })
}

fn assert_codes(document: audiveris_mei::MeiDocument, expected: &[ViolationCode]) {
    let error = serialize_mei(&document).expect_err("mutated document must fail");
    let MeiExportError::Validation(violations) = error else {
        panic!("expected validation failure")
    };
    let actual: Vec<_> = violations.iter().map(|violation| violation.code).collect();
    assert_eq!(actual, expected);
}

struct RejectAll;

impl io::Write for RejectAll {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("synthetic output failure"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
