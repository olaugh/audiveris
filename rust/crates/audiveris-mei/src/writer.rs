// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io;

use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesPI, BytesStart, BytesText, Event as XmlEvent};

use crate::model::{
    Beam, BeamEvent, Chord, ChordNote, Event, EventContext, MeiDocument, Note, Rest,
    StaffDefinition,
};

const MEI_NAMESPACE: &str = "http://www.music-encoding.org/ns/mei";
const MEI_VERSION: &str = "5.1+CMN";
const APPLICATION_ID: &str = "app.audiveris-rust";
const APPLICATION_REF: &str = "#app.audiveris-rust";

pub(crate) fn serialize(document: &MeiDocument) -> io::Result<Vec<u8>> {
    let mut writer = Writer::new_with_indent(Vec::new(), b' ', 2);
    writer.write_event(XmlEvent::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    writer.write_event(XmlEvent::PI(BytesPI::new(
        "xml-model href=\"https://music-encoding.org/schema/5.1/mei-CMN.rng\" schematypens=\"http://relaxng.org/ns/structure/1.0\" type=\"application/xml\"",
    )))?;

    let root_id = format!("mei.sheet{}", document.sheet_id);
    start(
        &mut writer,
        "mei",
        &[
            ("xmlns", MEI_NAMESPACE),
            ("meiversion", MEI_VERSION),
            ("xml:id", &root_id),
        ],
    )?;
    write_header(&mut writer, document)?;
    write_music(&mut writer, document)?;
    end(&mut writer, "mei")?;
    writer.get_mut().push(b'\n');
    Ok(writer.into_inner())
}

fn write_header(writer: &mut Writer<Vec<u8>>, document: &MeiDocument) -> io::Result<()> {
    start(writer, "meiHead", &[])?;
    start(writer, "fileDesc", &[])?;
    start(writer, "titleStmt", &[])?;
    text_element(writer, "title", &document.title)?;
    end(writer, "titleStmt")?;
    empty(writer, "pubStmt", &[])?;
    start(writer, "sourceDesc", &[])?;
    let source_id = format!("source.sheet{}", document.sheet_id);
    let mut source_attributes = vec![("xml:id", source_id.as_str())];
    if let Some(target) = &document.source.target {
        source_attributes.push(("target", target.as_str()));
    }
    start(writer, "source", &source_attributes)?;
    start(writer, "bibl", &[])?;
    text_element(writer, "title", &document.source.label)?;
    end(writer, "bibl")?;
    end(writer, "source")?;
    end(writer, "sourceDesc")?;
    end(writer, "fileDesc")?;

    start(writer, "encodingDesc", &[])?;
    start(writer, "appInfo", &[])?;
    let mut application_attributes = vec![
        ("xml:id", APPLICATION_ID),
        ("version", document.application.version.as_str()),
    ];
    if let Some(date) = &document.application.isodate {
        application_attributes.push(("isodate", date.as_str()));
    }
    start(writer, "application", &application_attributes)?;
    text_element(writer, "name", &document.application.name)?;
    let commit = format!("Source commit {}", document.application.commit);
    text_element(writer, "p", &commit)?;
    end(writer, "application")?;
    end(writer, "appInfo")?;
    end(writer, "encodingDesc")?;
    end(writer, "meiHead")
}

fn write_music(writer: &mut Writer<Vec<u8>>, document: &MeiDocument) -> io::Result<()> {
    start(writer, "music", &[])?;
    start(writer, "body", &[])?;

    let mdiv_id = format!("mdiv.sheet{}", document.sheet_id);
    start(writer, "mdiv", &[("xml:id", &mdiv_id)])?;
    let score_id = format!("score.sheet{}", document.sheet_id);
    start(writer, "score", &[("xml:id", &score_id)])?;
    let score_def_id = format!("scoredef.sheet{}", document.sheet_id);
    start(writer, "scoreDef", &[("xml:id", &score_def_id)])?;
    let staff_group_id = format!("staffgrp.sheet{}", document.sheet_id);
    start(writer, "staffGrp", &[("xml:id", &staff_group_id)])?;

    for part in &document.score.parts {
        let part_id = format!("part.i{}", part.id);
        start(writer, "staffGrp", &[("xml:id", &part_id)])?;
        if let Some(label) = &part.label {
            text_element(writer, "label", label)?;
        }
        for staff in &part.staves {
            write_staff_definition(writer, staff)?;
        }
        end(writer, "staffGrp")?;
    }

    end(writer, "staffGrp")?;
    end(writer, "scoreDef")?;
    let section_id = format!("section.sheet{}", document.sheet_id);
    start(writer, "section", &[("xml:id", &section_id)])?;

    for measure in &document.score.measures {
        let measure_id = format!("measure.i{}", measure.id);
        let mut attributes = vec![
            ("xml:id", measure_id.as_str()),
            ("n", measure.number.as_str()),
            ("metcon", measure.metrical_conformance.mei_token()),
        ];
        if let Some(right) = measure.right_barline {
            attributes.push(("right", right.mei_token()));
        }
        start(writer, "measure", &attributes)?;
        for staff in &measure.staves {
            let staff_definition_id = document
                .score
                .parts
                .iter()
                .flat_map(|part| &part.staves)
                .find(|definition| definition.number == staff.staff)
                .map(|definition| definition.id)
                .expect("validated staff topology");
            let staff_id = format!("measurestaff.m{}.s{}", measure.id, staff_definition_id);
            let staff_number = staff.staff.to_string();
            start(
                writer,
                "staff",
                &[("xml:id", &staff_id), ("n", &staff_number)],
            )?;
            for layer in &staff.layers {
                let layer_id = format!(
                    "layer.m{}.s{}.n{}",
                    measure.id, staff_definition_id, layer.number
                );
                let layer_number = layer.number.to_string();
                start(
                    writer,
                    "layer",
                    &[("xml:id", &layer_id), ("n", &layer_number)],
                )?;
                for event in &layer.events {
                    write_event(
                        writer,
                        event,
                        EventContext {
                            staff: staff.staff,
                            layer: layer.number,
                        },
                    )?;
                }
                end(writer, "layer")?;
            }
            end(writer, "staff")?;
        }
        end(writer, "measure")?;
    }

    end(writer, "section")?;
    end(writer, "score")?;
    end(writer, "mdiv")?;
    end(writer, "body")?;
    end(writer, "music")
}

fn write_staff_definition(writer: &mut Writer<Vec<u8>>, staff: &StaffDefinition) -> io::Result<()> {
    let id = format!("staff.i{}", staff.id);
    let number = staff.number.to_string();
    let lines = staff.lines.to_string();
    let clef_line = staff
        .clef
        .and_then(|clef| clef.line)
        .map(|line| line.to_string());
    let key_signature = staff.key_signature.map(key_signature_token);
    let meter_count = staff.meter.map(|meter| meter.count.to_string());
    let meter_unit = staff.meter.map(|meter| meter.unit.to_string());

    let mut attributes = vec![
        ("xml:id", id.as_str()),
        ("n", number.as_str()),
        ("lines", lines.as_str()),
    ];
    if let Some(clef) = staff.clef {
        attributes.push(("clef.shape", clef.shape.mei_token()));
        if let Some(clef_line) = &clef_line {
            attributes.push(("clef.line", clef_line));
        }
    }
    if let Some(key_signature) = &key_signature {
        attributes.push(("keysig", key_signature));
    }
    if let Some(meter_count) = &meter_count {
        attributes.push(("meter.count", meter_count));
    }
    if let Some(meter_unit) = &meter_unit {
        attributes.push(("meter.unit", meter_unit));
    }
    empty(writer, "staffDef", &attributes)
}

fn key_signature_token(fifths: i8) -> String {
    match fifths.cmp(&0) {
        std::cmp::Ordering::Less => format!("{}f", fifths.unsigned_abs()),
        std::cmp::Ordering::Equal => "0".to_owned(),
        std::cmp::Ordering::Greater => format!("{fifths}s"),
    }
}

fn write_event(
    writer: &mut Writer<Vec<u8>>,
    event: &Event,
    container: EventContext,
) -> io::Result<()> {
    match event {
        Event::Note(note) => write_note(writer, note, container),
        Event::Rest(rest) => write_rest(writer, rest, container),
        Event::MeasureRest(rest) => write_measure_rest(writer, rest, container),
        Event::Chord(chord) => write_chord(writer, chord, container),
        Event::Beam(beam) => write_beam(writer, beam, container),
        Event::LocalBarLine(barline) => {
            let id = format!("barline.i{}", barline.id);
            empty(
                writer,
                "barLine",
                &[
                    ("xml:id", &id),
                    ("form", barline.form.mei_token()),
                    ("resp", APPLICATION_REF),
                ],
            )
        }
    }
}

fn write_note(
    writer: &mut Writer<Vec<u8>>,
    note: &Note,
    container: EventContext,
) -> io::Result<()> {
    let id = format!("note.i{}", note.id);
    let octave = note.octave.to_string();
    let dots = note.dots.to_string();
    let staff = note.context.staff.to_string();
    let layer = note.context.layer.to_string();
    let mut attributes = vec![
        ("xml:id", id.as_str()),
        ("pname", note.pitch.mei_token()),
        ("oct", octave.as_str()),
    ];
    if let Some(accidental) = note.written_accidental {
        attributes.push(("accid", accidental.mei_token()));
    }
    attributes.push(("dur", note.duration.mei_token()));
    if note.dots != 0 {
        attributes.push(("dots", dots.as_str()));
    }
    context_attributes(&mut attributes, note.context, container, &staff, &layer);
    empty(writer, "note", &attributes)
}

fn write_measure_rest(
    writer: &mut Writer<Vec<u8>>,
    rest: &crate::MeasureRest,
    container: EventContext,
) -> io::Result<()> {
    let id = format!("mrest.i{}", rest.id);
    let staff = rest.context.staff.to_string();
    let layer = rest.context.layer.to_string();
    let mut attributes = vec![("xml:id", id.as_str())];
    context_attributes(&mut attributes, rest.context, container, &staff, &layer);
    empty(writer, "mRest", &attributes)
}

fn write_rest(
    writer: &mut Writer<Vec<u8>>,
    rest: &Rest,
    container: EventContext,
) -> io::Result<()> {
    let id = format!("rest.i{}", rest.id);
    let dots = rest.dots.to_string();
    let staff = rest.context.staff.to_string();
    let layer = rest.context.layer.to_string();
    let mut attributes = vec![("xml:id", id.as_str()), ("dur", rest.duration.mei_token())];
    if rest.dots != 0 {
        attributes.push(("dots", dots.as_str()));
    }
    context_attributes(&mut attributes, rest.context, container, &staff, &layer);
    empty(writer, "rest", &attributes)
}

fn write_chord(
    writer: &mut Writer<Vec<u8>>,
    chord: &Chord,
    container: EventContext,
) -> io::Result<()> {
    let id = format!("chord.i{}", chord.id);
    let dots = chord.dots.to_string();
    let staff = chord.context.staff.to_string();
    let layer = chord.context.layer.to_string();
    let mut attributes = vec![("xml:id", id.as_str()), ("dur", chord.duration.mei_token())];
    if chord.dots != 0 {
        attributes.push(("dots", dots.as_str()));
    }
    context_attributes(&mut attributes, chord.context, container, &staff, &layer);
    start(writer, "chord", &attributes)?;
    for note in &chord.notes {
        write_chord_note(writer, note, chord.context)?;
    }
    end(writer, "chord")
}

fn write_chord_note(
    writer: &mut Writer<Vec<u8>>,
    note: &ChordNote,
    context: EventContext,
) -> io::Result<()> {
    let id = format!("note.i{}", note.id);
    let octave = note.octave.to_string();
    let staff = context.staff.to_string();
    let layer = context.layer.to_string();
    let mut attributes = vec![
        ("xml:id", id.as_str()),
        ("pname", note.pitch.mei_token()),
        ("oct", octave.as_str()),
    ];
    if let Some(accidental) = note.written_accidental {
        attributes.push(("accid", accidental.mei_token()));
    }
    context_attributes(&mut attributes, context, context, &staff, &layer);
    empty(writer, "note", &attributes)
}

fn write_beam(
    writer: &mut Writer<Vec<u8>>,
    beam: &Beam,
    container: EventContext,
) -> io::Result<()> {
    let id = format!("beam.i{}", beam.id);
    let staff = beam.context.staff.to_string();
    let layer = beam.context.layer.to_string();
    let mut attributes = vec![("xml:id", id.as_str())];
    context_attributes(&mut attributes, beam.context, container, &staff, &layer);
    start(writer, "beam", &attributes)?;
    for event in &beam.events {
        match event {
            BeamEvent::Note(note) => write_note(writer, note, beam.context)?,
            BeamEvent::Rest(rest) => write_rest(writer, rest, beam.context)?,
            BeamEvent::Chord(chord) => write_chord(writer, chord, beam.context)?,
        }
    }
    end(writer, "beam")
}

fn context_attributes<'a>(
    attributes: &mut Vec<(&'static str, &'a str)>,
    context: EventContext,
    container: EventContext,
    staff: &'a str,
    layer: &'a str,
) {
    if context.staff != container.staff {
        attributes.push(("staff", staff));
    }
    if context.layer != container.layer {
        attributes.push(("layer", layer));
    }
    attributes.push(("resp", APPLICATION_REF));
}

fn start(writer: &mut Writer<Vec<u8>>, name: &str, attributes: &[(&str, &str)]) -> io::Result<()> {
    let mut element = BytesStart::new(name);
    for &(key, value) in attributes {
        element.push_attribute((key, value));
    }
    writer.write_event(XmlEvent::Start(element))
}

fn empty(writer: &mut Writer<Vec<u8>>, name: &str, attributes: &[(&str, &str)]) -> io::Result<()> {
    let mut element = BytesStart::new(name);
    for &(key, value) in attributes {
        element.push_attribute((key, value));
    }
    writer.write_event(XmlEvent::Empty(element))
}

fn text_element(writer: &mut Writer<Vec<u8>>, name: &str, value: &str) -> io::Result<()> {
    start(writer, name, &[])?;
    writer.write_event(XmlEvent::Text(BytesText::new(value)))?;
    end(writer, name)
}

fn end(writer: &mut Writer<Vec<u8>>, name: &str) -> io::Result<()> {
    writer.write_event(XmlEvent::End(BytesEnd::new(name)))
}
