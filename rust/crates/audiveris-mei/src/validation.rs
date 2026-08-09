// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::model::{
    BeamEvent, Chord, Duration, Event, EventContext, MeiDocument, Meter, MetricalConformance, Note,
    Rest, StableId,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MeiCounts {
    pub parts: usize,
    pub staves: usize,
    pub measures: usize,
    pub layers: usize,
    /// Includes notes inside chords and beams.
    pub notes: usize,
    pub rests: usize,
    pub chords: usize,
    pub beams: usize,
    /// Score-wide boundaries encoded as `measure@right`.
    pub measure_barlines: usize,
    /// Staff-local/internal `barLine` elements.
    pub local_barlines: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViolationCode {
    InvalidSheetId,
    EmptyText,
    InvalidXmlCharacter,
    InvalidApplicationVersion,
    InvalidSourceTarget,
    EmptyScore,
    EmptyPart,
    DuplicatePartId,
    DuplicateStaffId,
    DuplicateStaffNumber,
    InvalidStaffLines,
    MissingClefLine,
    InvalidClefLine,
    InvalidKeySignature,
    InvalidMeter,
    DuplicateMeasureId,
    EmptyMeasureNumber,
    InvalidMeasureNumber,
    MeasureStaffTopology,
    EmptyLayers,
    LayerOrder,
    DuplicateEventId,
    UnknownEventContext,
    MeasureRestNotAlone,
    MissingMeterForMetricalStatus,
    MetricalMismatch,
    FalseMetricalNonconformance,
    InvalidOctave,
    InvalidDots,
    ChordTooShort,
    BeamTooShort,
    UnbeamableDuration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationViolation {
    pub path: String,
    pub code: ViolationCode,
    pub detail: String,
}

impl fmt::Display for ValidationViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {:?}: {}", self.path, self.code, self.detail)
    }
}

pub(crate) fn validate(document: &MeiDocument) -> Result<MeiCounts, Vec<ValidationViolation>> {
    let mut state = ValidationState::default();

    if document.sheet_id == 0 {
        state.push(
            "sheet_id",
            ViolationCode::InvalidSheetId,
            "sheet identity must be positive",
        );
    }
    state.required_text("title", &document.title);
    state.required_text("source.label", &document.source.label);
    if let Some(target) = &document.source.target {
        state.required_text("source.target", target);
        let local_source = format!("#source.sheet{}", document.sheet_id);
        if !has_phase_one_uri_syntax(target)
            || !(target == &local_source
                || target.starts_with("./")
                || target.starts_with("../")
                || has_uri_scheme(target))
        {
            state.push(
                "source.target",
                ViolationCode::InvalidSourceTarget,
                "source target must be a local source ID or an ASCII RFC 3986-style relative/absolute URI with valid percent escapes",
            );
        }
    }
    state.required_text("application.name", &document.application.name);
    state.required_text("application.version", &document.application.version);
    state.required_text("application.commit", &document.application.commit);
    if !document.application.version.is_empty()
        && !document
            .application
            .version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
    {
        state.push(
            "application.version",
            ViolationCode::InvalidApplicationVersion,
            "MEI application version must be an ASCII NMTOKEN",
        );
    }

    if document.score.parts.is_empty() || document.score.measures.is_empty() {
        state.push(
            "score",
            ViolationCode::EmptyScore,
            "Phase 1 requires at least one part and one measure",
        );
    }

    let mut part_ids = HashSet::new();
    let mut staff_ids = HashSet::new();
    let mut staff_numbers = HashSet::new();
    let mut ordered_staves = Vec::new();

    for (part_index, part) in document.score.parts.iter().enumerate() {
        let part_path = format!("score.parts[{part_index}]");
        state.counts.parts += 1;
        if !part_ids.insert(part.id) {
            state.push(
                format!("{part_path}.id"),
                ViolationCode::DuplicatePartId,
                format!("part identity {} is repeated", part.id),
            );
        }
        if let Some(label) = &part.label {
            state.required_text(format!("{part_path}.label"), label);
        }
        if part.staves.is_empty() {
            state.push(
                &part_path,
                ViolationCode::EmptyPart,
                "each part must define at least one staff",
            );
        }
        for (staff_index, staff) in part.staves.iter().enumerate() {
            let staff_path = format!("{part_path}.staves[{staff_index}]");
            state.counts.staves += 1;
            ordered_staves.push((staff.id, staff.number));
            if !staff_ids.insert(staff.id) {
                state.push(
                    format!("{staff_path}.id"),
                    ViolationCode::DuplicateStaffId,
                    format!("staff identity {} is repeated", staff.id),
                );
            }
            if !staff_numbers.insert(staff.number) {
                state.push(
                    format!("{staff_path}.number"),
                    ViolationCode::DuplicateStaffNumber,
                    format!("staff number {} is repeated", staff.number),
                );
            }
            if staff.lines == 0 {
                state.push(
                    format!("{staff_path}.lines"),
                    ViolationCode::InvalidStaffLines,
                    "staff line count must be positive",
                );
            }
            if let Some(clef) = staff.clef {
                let pitched = matches!(
                    clef.shape,
                    crate::ClefShape::G
                        | crate::ClefShape::Gg
                        | crate::ClefShape::F
                        | crate::ClefShape::C
                );
                if pitched && clef.line.is_none() {
                    state.push(
                        format!("{staff_path}.clef.line"),
                        ViolationCode::MissingClefLine,
                        "G/GG/F/C clefs require an explicit line",
                    );
                }
                if let Some(line) = clef.line
                    && (line == 0 || line > staff.lines)
                {
                    state.push(
                        format!("{staff_path}.clef.line"),
                        ViolationCode::InvalidClefLine,
                        format!(
                            "clef line {line} is outside the staff's {} lines",
                            staff.lines
                        ),
                    );
                }
            }
            if let Some(fifths) = staff.key_signature
                && !(-12..=12).contains(&fifths)
            {
                state.push(
                    format!("{staff_path}.key_signature"),
                    ViolationCode::InvalidKeySignature,
                    format!("key signature {fifths} is outside -12..=12 fifths"),
                );
            }
            if let Some(meter) = staff.meter
                && (meter.count == 0 || meter.unit == 0)
            {
                state.push(
                    format!("{staff_path}.meter"),
                    ViolationCode::InvalidMeter,
                    "meter count and unit must both be positive",
                );
            }
        }
    }

    let expected_staff_order: Vec<_> = ordered_staves.iter().map(|(_, number)| *number).collect();
    let staff_id_for_number: HashMap<_, _> = ordered_staves
        .iter()
        .map(|(id, number)| (*number, *id))
        .collect();
    let meter_for_number: HashMap<_, _> = document
        .score
        .parts
        .iter()
        .flat_map(|part| &part.staves)
        .map(|staff| (staff.number, staff.meter))
        .collect();
    let mut measure_ids = HashSet::new();

    for (measure_index, measure) in document.score.measures.iter().enumerate() {
        let measure_path = format!("score.measures[{measure_index}]");
        state.counts.measures += 1;
        if !measure_ids.insert(measure.id) {
            state.push(
                format!("{measure_path}.id"),
                ViolationCode::DuplicateMeasureId,
                format!("measure identity {} is repeated", measure.id),
            );
        }
        if measure.number.trim().is_empty() {
            state.push(
                format!("{measure_path}.number"),
                ViolationCode::EmptyMeasureNumber,
                "measure number must not be empty",
            );
        } else {
            state.xml_text(format!("{measure_path}.number"), &measure.number);
            if !measure.number.chars().all(|character| {
                character.is_ascii_alphanumeric() || character.is_ascii_punctuation()
            }) {
                state.push(
                    format!("{measure_path}.number"),
                    ViolationCode::InvalidMeasureNumber,
                    "the Phase 1 measure@n profile is a nonempty ASCII WORD without whitespace or combining marks",
                );
            }
        }
        if measure.right_barline.is_some() {
            state.counts.measure_barlines += 1;
        }

        let actual_staff_order: Vec<_> = measure.staves.iter().map(|staff| staff.staff).collect();
        if actual_staff_order != expected_staff_order {
            state.push(
                format!("{measure_path}.staves"),
                ViolationCode::MeasureStaffTopology,
                format!(
                    "measure staff order {actual_staff_order:?} does not match score order {expected_staff_order:?}"
                ),
            );
        }

        let mut contexts = HashSet::new();
        let mut has_metrical_mismatch = false;
        let mut has_unresolved_meter = false;
        for (staff_index, staff) in measure.staves.iter().enumerate() {
            let staff_path = format!("{measure_path}.staves[{staff_index}]");
            if !staff_id_for_number.contains_key(&staff.staff) {
                state.push(
                    format!("{staff_path}.staff"),
                    ViolationCode::MeasureStaffTopology,
                    format!("unknown staff number {}", staff.staff),
                );
            }
            if staff.layers.is_empty() {
                state.push(
                    &staff_path,
                    ViolationCode::EmptyLayers,
                    "each measure staff must contain at least one layer",
                );
            }
            let mut previous = None;
            for layer in &staff.layers {
                if previous.is_some_and(|prior| layer.number <= prior) {
                    state.push(
                        format!("{staff_path}.layers"),
                        ViolationCode::LayerOrder,
                        "layer numbers must be strictly increasing",
                    );
                }
                previous = Some(layer.number);
                contexts.insert((staff.staff, layer.number));
            }
        }

        for (staff_index, staff) in measure.staves.iter().enumerate() {
            for (layer_index, layer) in staff.layers.iter().enumerate() {
                state.counts.layers += 1;
                if layer
                    .events
                    .iter()
                    .any(|event| matches!(event, Event::MeasureRest(_)))
                    && layer.events.len() != 1
                {
                    state.push(
                        format!(
                            "{measure_path}.staves[{staff_index}].layers[{layer_index}].events"
                        ),
                        ViolationCode::MeasureRestNotAlone,
                        "mRest must be the only event in its layer",
                    );
                }
                for (event_index, event) in layer.events.iter().enumerate() {
                    let path = format!(
                        "{measure_path}.staves[{staff_index}].layers[{layer_index}].events[{event_index}]"
                    );
                    state.event(event, &contexts, &path);
                }
                match meter_for_number.get(&staff.staff).copied().flatten() {
                    Some(meter) if meter.count != 0 && meter.unit != 0 => {
                        let actual = layer_duration(&layer.events, meter);
                        let expected =
                            Fraction::new(u128::from(meter.count), u128::from(meter.unit));
                        if actual != expected {
                            has_metrical_mismatch = true;
                            if measure.metrical_conformance == MetricalConformance::Conformant {
                                state.push(
                                    format!(
                                        "{measure_path}.staves[{staff_index}].layers[{layer_index}]"
                                    ),
                                    ViolationCode::MetricalMismatch,
                                    format!(
                                        "layer duration {}/{} does not equal meter {}/{}",
                                        actual.numerator,
                                        actual.denominator,
                                        expected.numerator,
                                        expected.denominator
                                    ),
                                );
                            }
                        }
                    }
                    Some(_) => has_unresolved_meter = true,
                    None => {
                        has_unresolved_meter = true;
                        state.push(
                            format!("{measure_path}.staves[{staff_index}]"),
                            ViolationCode::MissingMeterForMetricalStatus,
                            "a metrical-conformance declaration requires a declared staff meter",
                        );
                    }
                }
            }
        }
        if measure.metrical_conformance == MetricalConformance::NonConformant
            && !has_unresolved_meter
            && !has_metrical_mismatch
        {
            state.push(
                format!("{measure_path}.metrical_conformance"),
                ViolationCode::FalseMetricalNonconformance,
                "metcon=false requires at least one layer whose duration differs from its meter",
            );
        }
    }

    if state.violations.is_empty() {
        Ok(state.counts)
    } else {
        Err(state.violations)
    }
}

fn has_uri_scheme(value: &str) -> bool {
    let Some((scheme, _rest)) = value.split_once(':') else {
        return false;
    };
    let mut characters = scheme.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && characters.all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
        })
}

fn has_phase_one_uri_syntax(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'%' {
            if index + 2 >= bytes.len()
                || !bytes[index + 1].is_ascii_hexdigit()
                || !bytes[index + 2].is_ascii_hexdigit()
            {
                return false;
            }
            index += 3;
            continue;
        }
        if !(byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'-' | b'.'
                    | b'_'
                    | b'~'
                    | b':'
                    | b'/'
                    | b'?'
                    | b'#'
                    | b'@'
                    | b'!'
                    | b'$'
                    | b'&'
                    | b'\''
                    | b'('
                    | b')'
                    | b'*'
                    | b'+'
                    | b','
                    | b';'
                    | b'='
            ))
        {
            return false;
        }
        index += 1;
    }
    true
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Fraction {
    numerator: u128,
    denominator: u128,
}

impl Fraction {
    fn new(numerator: u128, denominator: u128) -> Self {
        debug_assert_ne!(denominator, 0);
        let divisor = gcd(numerator, denominator);
        Self {
            numerator: numerator / divisor,
            denominator: denominator / divisor,
        }
    }

    fn add(self, other: Self) -> Self {
        Self::new(
            self.numerator * other.denominator + other.numerator * self.denominator,
            self.denominator * other.denominator,
        )
    }
}

fn gcd(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left.max(1)
}

fn layer_duration(events: &[Event], meter: Meter) -> Fraction {
    events.iter().fold(Fraction::new(0, 1), |total, event| {
        total.add(event_duration(event, meter))
    })
}

fn event_duration(event: &Event, meter: Meter) -> Fraction {
    match event {
        Event::Note(note) => duration(note.duration, note.dots),
        Event::Rest(rest) => duration(rest.duration, rest.dots),
        Event::MeasureRest(_) => Fraction::new(u128::from(meter.count), u128::from(meter.unit)),
        Event::Chord(chord) => duration(chord.duration, chord.dots),
        Event::Beam(beam) => beam
            .events
            .iter()
            .fold(Fraction::new(0, 1), |total, event| {
                let duration = match event {
                    BeamEvent::Note(note) => duration(note.duration, note.dots),
                    BeamEvent::Rest(rest) => duration(rest.duration, rest.dots),
                    BeamEvent::Chord(chord) => duration(chord.duration, chord.dots),
                };
                total.add(duration)
            }),
        Event::LocalBarLine(_) => Fraction::new(0, 1),
    }
}

fn duration(duration: Duration, dots: u8) -> Fraction {
    let dots = dots.min(4);
    let dot_denominator = 1_u128 << dots;
    let dot_numerator = (1_u128 << (u32::from(dots) + 1)) - 1;
    Fraction::new(
        duration.whole_note_units() * dot_numerator,
        2048 * dot_denominator,
    )
}

#[derive(Default)]
struct ValidationState {
    counts: MeiCounts,
    event_ids: HashMap<StableId, String>,
    violations: Vec<ValidationViolation>,
}

impl ValidationState {
    fn push(&mut self, path: impl Into<String>, code: ViolationCode, detail: impl Into<String>) {
        self.violations.push(ValidationViolation {
            path: path.into(),
            code,
            detail: detail.into(),
        });
    }

    fn required_text(&mut self, path: impl Into<String>, value: &str) {
        let path = path.into();
        if value.trim().is_empty() {
            self.push(&path, ViolationCode::EmptyText, "value must not be empty");
        }
        self.xml_text(path, value);
    }

    fn xml_text(&mut self, path: impl Into<String>, value: &str) {
        if value.chars().any(|character| {
            !matches!(character, '\u{9}' | '\u{A}' | '\u{D}' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}')
        }) {
            self.push(
                path,
                ViolationCode::InvalidXmlCharacter,
                "value contains a character forbidden by XML 1.0",
            );
        }
    }

    fn register_event_id(&mut self, id: StableId, path: &str) {
        if let Some(previous) = self.event_ids.insert(id, path.to_owned()) {
            self.push(
                format!("{path}.id"),
                ViolationCode::DuplicateEventId,
                format!("semantic event identity {id} was already used at {previous}"),
            );
        }
    }

    fn context(
        &mut self,
        context: EventContext,
        contexts: &HashSet<(crate::StaffNumber, crate::LayerNumber)>,
        path: &str,
    ) {
        if !contexts.contains(&(context.staff, context.layer)) {
            self.push(
                format!("{path}.context"),
                ViolationCode::UnknownEventContext,
                format!(
                    "target staff {} layer {} does not exist in this measure",
                    context.staff, context.layer
                ),
            );
        }
    }

    fn note(
        &mut self,
        note: &Note,
        contexts: &HashSet<(crate::StaffNumber, crate::LayerNumber)>,
        path: &str,
    ) {
        self.register_event_id(note.id, path);
        self.context(note.context, contexts, path);
        self.pitch(note.octave, note.dots, path);
        self.counts.notes += 1;
    }

    fn rest(
        &mut self,
        rest: &Rest,
        contexts: &HashSet<(crate::StaffNumber, crate::LayerNumber)>,
        path: &str,
    ) {
        self.register_event_id(rest.id, path);
        self.context(rest.context, contexts, path);
        self.dots(rest.dots, path);
        self.counts.rests += 1;
    }

    fn chord(
        &mut self,
        chord: &Chord,
        contexts: &HashSet<(crate::StaffNumber, crate::LayerNumber)>,
        path: &str,
    ) {
        self.register_event_id(chord.id, path);
        self.context(chord.context, contexts, path);
        self.dots(chord.dots, path);
        self.counts.chords += 1;
        if chord.notes.len() < 2 {
            self.push(
                format!("{path}.notes"),
                ViolationCode::ChordTooShort,
                "a Phase 1 chord must contain at least two notes",
            );
        }
        for (index, note) in chord.notes.iter().enumerate() {
            let note_path = format!("{path}.notes[{index}]");
            self.register_event_id(note.id, &note_path);
            self.pitch(note.octave, 0, &note_path);
            self.counts.notes += 1;
        }
    }

    fn pitch(&mut self, octave: u8, dots: u8, path: &str) {
        if octave > 9 {
            self.push(
                format!("{path}.octave"),
                ViolationCode::InvalidOctave,
                format!("octave {octave} is outside 0..=9"),
            );
        }
        self.dots(dots, path);
    }

    fn dots(&mut self, dots: u8, path: &str) {
        if dots > 4 {
            self.push(
                format!("{path}.dots"),
                ViolationCode::InvalidDots,
                format!("dot count {dots} is outside 0..=4"),
            );
        }
    }

    fn event(
        &mut self,
        event: &Event,
        contexts: &HashSet<(crate::StaffNumber, crate::LayerNumber)>,
        path: &str,
    ) {
        match event {
            Event::Note(note) => self.note(note, contexts, path),
            Event::Rest(rest) => self.rest(rest, contexts, path),
            Event::MeasureRest(rest) => {
                self.register_event_id(rest.id, path);
                self.context(rest.context, contexts, path);
                self.counts.rests += 1;
            }
            Event::Chord(chord) => self.chord(chord, contexts, path),
            Event::Beam(beam) => {
                self.register_event_id(beam.id, path);
                self.context(beam.context, contexts, path);
                self.counts.beams += 1;
                if beam.events.len() < 2 {
                    self.push(
                        format!("{path}.events"),
                        ViolationCode::BeamTooShort,
                        "a Phase 1 beam must contain at least two note/rest/chord events",
                    );
                }
                for (index, child) in beam.events.iter().enumerate() {
                    let child_path = format!("{path}.events[{index}]");
                    match child {
                        BeamEvent::Note(note) => {
                            self.beamable(note.duration, &child_path);
                            self.note(note, contexts, &child_path);
                        }
                        BeamEvent::Rest(rest) => {
                            self.beamable(rest.duration, &child_path);
                            self.rest(rest, contexts, &child_path);
                        }
                        BeamEvent::Chord(chord) => {
                            self.beamable(chord.duration, &child_path);
                            self.chord(chord, contexts, &child_path);
                        }
                    }
                }
            }
            Event::LocalBarLine(barline) => {
                self.register_event_id(barline.id, path);
                self.counts.local_barlines += 1;
            }
        }
    }

    fn beamable(&mut self, duration: Duration, path: &str) {
        if !duration.is_beamable() {
            self.push(
                format!("{path}.duration"),
                ViolationCode::UnbeamableDuration,
                format!(
                    "duration {} is outside the Phase 1 beamed-duration profile",
                    duration.mei_token()
                ),
            );
        }
    }
}
