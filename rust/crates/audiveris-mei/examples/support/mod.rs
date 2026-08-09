// SPDX-License-Identifier: AGPL-3.0-or-later

use audiveris_mei::{
    ApplicationInfo, BarRendition, Beam, BeamEvent, Chord, ChordNote, Clef, ClefShape, Duration,
    Event, EventContext, Layer, LayerNumber, LocalBarLine, Measure, MeasureRest, MeasureStaff,
    MeiDocument, Meter, MetricalConformance, Note, PartDefinition, PitchName, Score,
    SourceDescription, StableId, StaffDefinition, StaffNumber, WrittenAccidental,
};

pub fn stable(value: u64) -> StableId {
    StableId::new(value).expect("sample identities are positive")
}

pub fn staff(value: u32) -> StaffNumber {
    StaffNumber::new(value).expect("sample staff numbers are positive")
}

pub fn layer(value: u32) -> LayerNumber {
    LayerNumber::new(value).expect("sample layer numbers are positive")
}

pub fn context(staff_number: u32, layer_number: u32) -> EventContext {
    EventContext {
        staff: staff(staff_number),
        layer: layer(layer_number),
    }
}

pub fn phase_one_document() -> MeiDocument {
    MeiDocument {
        sheet_id: 1,
        title: "Étude <One> & Two".to_owned(),
        source: SourceDescription {
            label: "Scan & source".to_owned(),
            target: Some("file:///scores/etude-01.png".to_owned()),
        },
        application: ApplicationInfo {
            name: "Audiveris Rust".to_owned(),
            version: "0.1.0".to_owned(),
            commit: "abc123".to_owned(),
            isodate: None,
        },
        score: Score {
            parts: vec![PartDefinition {
                id: stable(10),
                label: Some("Piano".to_owned()),
                staves: vec![
                    StaffDefinition {
                        id: stable(11),
                        number: staff(1),
                        lines: 5,
                        clef: Some(Clef {
                            shape: ClefShape::G,
                            line: Some(2),
                        }),
                        key_signature: Some(0),
                        meter: Some(Meter { count: 4, unit: 4 }),
                    },
                    StaffDefinition {
                        id: stable(12),
                        number: staff(2),
                        lines: 5,
                        clef: Some(Clef {
                            shape: ClefShape::F,
                            line: Some(4),
                        }),
                        key_signature: Some(0),
                        meter: Some(Meter { count: 4, unit: 4 }),
                    },
                ],
            }],
            measures: vec![
                Measure {
                    id: stable(20),
                    number: "1".to_owned(),
                    metrical_conformance: MetricalConformance::Conformant,
                    right_barline: Some(BarRendition::Double),
                    staves: vec![
                        MeasureStaff {
                            staff: staff(1),
                            layers: vec![Layer {
                                number: layer(1),
                                events: vec![
                                    Event::Note(Note {
                                        id: stable(100),
                                        context: context(1, 1),
                                        pitch: PitchName::C,
                                        octave: 4,
                                        written_accidental: None,
                                        duration: Duration::Quarter,
                                        dots: 0,
                                    }),
                                    Event::Beam(Beam {
                                        id: stable(101),
                                        context: context(1, 1),
                                        events: vec![
                                            BeamEvent::Note(Note {
                                                id: stable(102),
                                                context: context(1, 1),
                                                pitch: PitchName::E,
                                                octave: 4,
                                                written_accidental: None,
                                                duration: Duration::Eighth,
                                                dots: 0,
                                            }),
                                            BeamEvent::Note(Note {
                                                id: stable(103),
                                                context: context(1, 1),
                                                pitch: PitchName::F,
                                                octave: 4,
                                                written_accidental: Some(WrittenAccidental::Sharp),
                                                duration: Duration::Eighth,
                                                dots: 0,
                                            }),
                                        ],
                                    }),
                                    Event::Chord(Chord {
                                        id: stable(104),
                                        context: context(1, 1),
                                        duration: Duration::Half,
                                        dots: 0,
                                        notes: vec![
                                            ChordNote {
                                                id: stable(105),
                                                pitch: PitchName::A,
                                                octave: 4,
                                                written_accidental: None,
                                            },
                                            ChordNote {
                                                id: stable(106),
                                                pitch: PitchName::C,
                                                octave: 5,
                                                written_accidental: None,
                                            },
                                        ],
                                    }),
                                    Event::LocalBarLine(LocalBarLine {
                                        id: stable(107),
                                        form: BarRendition::DoubleDashed,
                                    }),
                                ],
                            }],
                        },
                        MeasureStaff {
                            staff: staff(2),
                            layers: vec![Layer {
                                number: layer(1),
                                events: vec![Event::MeasureRest(MeasureRest {
                                    id: stable(200),
                                    context: context(2, 1),
                                })],
                            }],
                        },
                    ],
                },
                Measure {
                    id: stable(21),
                    number: "2".to_owned(),
                    metrical_conformance: MetricalConformance::Conformant,
                    right_barline: Some(BarRendition::End),
                    staves: vec![
                        MeasureStaff {
                            staff: staff(1),
                            layers: vec![Layer {
                                number: layer(1),
                                events: vec![Event::Note(Note {
                                    id: stable(300),
                                    context: context(2, 1),
                                    pitch: PitchName::D,
                                    octave: 5,
                                    written_accidental: None,
                                    duration: Duration::Whole,
                                    dots: 0,
                                })],
                            }],
                        },
                        MeasureStaff {
                            staff: staff(2),
                            layers: vec![Layer {
                                number: layer(1),
                                events: vec![Event::MeasureRest(MeasureRest {
                                    id: stable(301),
                                    context: context(2, 1),
                                })],
                            }],
                        },
                    ],
                },
            ],
        },
    }
}
