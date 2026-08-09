// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fmt;

/// A stable, non-zero semantic identity supplied by the recognition pipeline.
///
/// The serializer never allocates process-order identities. MEI `xml:id` values
/// are derived from these values and a fixed element-kind prefix.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StableId(u64);

impl StableId {
    /// Construct a caller-owned identity.
    ///
    /// Non-zero is enforced here. Stability across recognition runs is a
    /// caller contract and cannot be inferred by the serializer.
    #[must_use]
    pub const fn new(value: u64) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for StableId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// A positive MEI staff number.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StaffNumber(u32);

impl StaffNumber {
    #[must_use]
    pub const fn new(value: u32) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl fmt::Display for StaffNumber {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// A positive MEI layer (voice) number.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LayerNumber(u32);

impl LayerNumber {
    #[must_use]
    pub const fn new(value: u32) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl fmt::Display for LayerNumber {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// A caller-pinned ISO calendar date. No current time is consulted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PinnedDate(String);

impl PinnedDate {
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidPinnedDate> {
        let value = value.into();
        if is_calendar_date(&value) {
            Ok(Self(value))
        } else {
            Err(InvalidPinnedDate(value))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvalidPinnedDate(pub String);

impl fmt::Display for InvalidPinnedDate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid pinned ISO date {:?}", self.0)
    }
}

impl std::error::Error for InvalidPinnedDate {}

fn is_calendar_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    if bytes
        .iter()
        .enumerate()
        .any(|(index, byte)| index != 4 && index != 7 && !byte.is_ascii_digit())
    {
        return false;
    }
    let year = value[0..4].parse::<u32>().ok();
    let month = value[5..7].parse::<u32>().ok();
    let day = value[8..10].parse::<u32>().ok();
    let (Some(year), Some(month), Some(day)) = (year, month, day) else {
        return false;
    };
    if year == 0 || !(1..=12).contains(&month) {
        return false;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max_day = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    (1..=max_day).contains(&day)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeiDocument {
    pub sheet_id: u32,
    pub title: String,
    pub source: SourceDescription,
    pub application: ApplicationInfo,
    pub score: Score,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceDescription {
    pub label: String,
    pub target: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationInfo {
    pub name: String,
    pub version: String,
    pub commit: String,
    pub isodate: Option<PinnedDate>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Score {
    pub parts: Vec<PartDefinition>,
    pub measures: Vec<Measure>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartDefinition {
    pub id: StableId,
    pub label: Option<String>,
    pub staves: Vec<StaffDefinition>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaffDefinition {
    pub id: StableId,
    pub number: StaffNumber,
    pub lines: u8,
    pub clef: Option<Clef>,
    /// Key signature in fifths, from 12 flats through 12 sharps.
    pub key_signature: Option<i8>,
    pub meter: Option<Meter>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Clef {
    pub shape: ClefShape,
    /// Required for G/GG/F/C; optional for percussion and tablature.
    pub line: Option<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClefShape {
    G,
    Gg,
    F,
    C,
    Percussion,
    Tab,
}

impl ClefShape {
    pub(crate) const fn mei_token(self) -> &'static str {
        match self {
            Self::G => "G",
            Self::Gg => "GG",
            Self::F => "F",
            Self::C => "C",
            Self::Percussion => "perc",
            Self::Tab => "TAB",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Meter {
    pub count: u16,
    pub unit: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Measure {
    pub id: StableId,
    pub number: String,
    pub metrical_conformance: MetricalConformance,
    pub right_barline: Option<BarRendition>,
    /// Must contain every score staff exactly once and in score-definition order.
    pub staves: Vec<MeasureStaff>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetricalConformance {
    /// Every layer must add up exactly to its staff's declared meter.
    Conformant,
    /// The source is intentionally underfilled or overfilled; emit `metcon="false"`.
    NonConformant,
}

impl MetricalConformance {
    pub(crate) const fn mei_token(self) -> &'static str {
        match self {
            Self::Conformant => "true",
            Self::NonConformant => "false",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeasureStaff {
    pub staff: StaffNumber,
    pub layers: Vec<Layer>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Layer {
    pub number: LayerNumber,
    /// Musical source order. The serializer never sorts simultaneous events.
    pub events: Vec<Event>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Event {
    Note(Note),
    Rest(Rest),
    MeasureRest(MeasureRest),
    Chord(Chord),
    Beam(Beam),
    LocalBarLine(LocalBarLine),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventContext {
    /// The musical target staff, including a real cross-staff target.
    pub staff: StaffNumber,
    /// The musical target voice/layer.
    pub layer: LayerNumber,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Note {
    pub id: StableId,
    pub context: EventContext,
    pub pitch: PitchName,
    pub octave: u8,
    /// A visibly written accidental (`@accid`), not a sounding alteration.
    pub written_accidental: Option<WrittenAccidental>,
    pub duration: Duration,
    pub dots: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rest {
    pub id: StableId,
    pub context: EventContext,
    pub duration: Duration,
    pub dots: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeasureRest {
    pub id: StableId,
    pub context: EventContext,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Chord {
    pub id: StableId,
    pub context: EventContext,
    pub duration: Duration,
    pub dots: u8,
    pub notes: Vec<ChordNote>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChordNote {
    pub id: StableId,
    pub pitch: PitchName,
    pub octave: u8,
    /// A visibly written accidental (`@accid`), not a sounding alteration.
    pub written_accidental: Option<WrittenAccidental>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Beam {
    pub id: StableId,
    pub context: EventContext,
    pub events: Vec<BeamEvent>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BeamEvent {
    Note(Note),
    Rest(Rest),
    Chord(Chord),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalBarLine {
    pub id: StableId,
    pub form: BarRendition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PitchName {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
}

impl PitchName {
    pub(crate) const fn mei_token(self) -> &'static str {
        match self {
            Self::A => "a",
            Self::B => "b",
            Self::C => "c",
            Self::D => "d",
            Self::E => "e",
            Self::F => "f",
            Self::G => "g",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WrittenAccidental {
    Sharp,
    Flat,
    DoubleSharp,
    DoubleFlat,
    Natural,
    NaturalFlat,
    NaturalSharp,
}

impl WrittenAccidental {
    pub(crate) const fn mei_token(self) -> &'static str {
        match self {
            Self::Sharp => "s",
            Self::Flat => "f",
            Self::DoubleSharp => "ss",
            Self::DoubleFlat => "ff",
            Self::Natural => "n",
            Self::NaturalFlat => "nf",
            Self::NaturalSharp => "ns",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Duration {
    Long,
    Breve,
    Whole,
    Half,
    Quarter,
    Eighth,
    Sixteenth,
    ThirtySecond,
    SixtyFourth,
    OneTwentyEighth,
    TwoFiftySixth,
    FiveHundredTwelfth,
    OneThousandTwentyFourth,
    TwoThousandFortyEighth,
}

impl Duration {
    pub(crate) const fn mei_token(self) -> &'static str {
        match self {
            Self::Long => "long",
            Self::Breve => "breve",
            Self::Whole => "1",
            Self::Half => "2",
            Self::Quarter => "4",
            Self::Eighth => "8",
            Self::Sixteenth => "16",
            Self::ThirtySecond => "32",
            Self::SixtyFourth => "64",
            Self::OneTwentyEighth => "128",
            Self::TwoFiftySixth => "256",
            Self::FiveHundredTwelfth => "512",
            Self::OneThousandTwentyFourth => "1024",
            Self::TwoThousandFortyEighth => "2048",
        }
    }

    pub(crate) const fn whole_note_units(self) -> u128 {
        match self {
            Self::Long => 8192,
            Self::Breve => 4096,
            Self::Whole => 2048,
            Self::Half => 1024,
            Self::Quarter => 512,
            Self::Eighth => 256,
            Self::Sixteenth => 128,
            Self::ThirtySecond => 64,
            Self::SixtyFourth => 32,
            Self::OneTwentyEighth => 16,
            Self::TwoFiftySixth => 8,
            Self::FiveHundredTwelfth => 4,
            Self::OneThousandTwentyFourth => 2,
            Self::TwoThousandFortyEighth => 1,
        }
    }

    pub(crate) const fn is_beamable(self) -> bool {
        matches!(
            self,
            Self::Eighth
                | Self::Sixteenth
                | Self::ThirtySecond
                | Self::SixtyFourth
                | Self::OneTwentyEighth
                | Self::TwoFiftySixth
                | Self::FiveHundredTwelfth
                | Self::OneThousandTwentyFourth
                | Self::TwoThousandFortyEighth
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BarRendition {
    Dashed,
    Dotted,
    Double,
    DoubleDashed,
    DoubleDotted,
    DoubleHeavy,
    DoubleSegno,
    End,
    Heavy,
    Invisible,
    RepeatStart,
    RepeatBoth,
    RepeatEnd,
    Segno,
    Single,
}

impl BarRendition {
    pub(crate) const fn mei_token(self) -> &'static str {
        match self {
            Self::Dashed => "dashed",
            Self::Dotted => "dotted",
            Self::Double => "dbl",
            Self::DoubleDashed => "dbldashed",
            Self::DoubleDotted => "dbldotted",
            Self::DoubleHeavy => "dblheavy",
            Self::DoubleSegno => "dblsegno",
            Self::End => "end",
            Self::Heavy => "heavy",
            Self::Invisible => "invis",
            Self::RepeatStart => "rptstart",
            Self::RepeatBoth => "rptboth",
            Self::RepeatEnd => "rptend",
            Self::Segno => "segno",
            Self::Single => "single",
        }
    }
}
