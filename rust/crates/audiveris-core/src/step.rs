// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{fmt, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OmrStep {
    Load,
    Binary,
    Scale,
    Grid,
    Headers,
    StemSeeds,
    Beams,
    Ledgers,
    Heads,
    Stems,
    Reduction,
    CueBeams,
    Texts,
    Measures,
    Chords,
    Curves,
    Symbols,
    Links,
    Rhythms,
    Page,
}

impl OmrStep {
    pub const ALL: [Self; 20] = [
        Self::Load,
        Self::Binary,
        Self::Scale,
        Self::Grid,
        Self::Headers,
        Self::StemSeeds,
        Self::Beams,
        Self::Ledgers,
        Self::Heads,
        Self::Stems,
        Self::Reduction,
        Self::CueBeams,
        Self::Texts,
        Self::Measures,
        Self::Chords,
        Self::Curves,
        Self::Symbols,
        Self::Links,
        Self::Rhythms,
        Self::Page,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Load => "LOAD",
            Self::Binary => "BINARY",
            Self::Scale => "SCALE",
            Self::Grid => "GRID",
            Self::Headers => "HEADERS",
            Self::StemSeeds => "STEM_SEEDS",
            Self::Beams => "BEAMS",
            Self::Ledgers => "LEDGERS",
            Self::Heads => "HEADS",
            Self::Stems => "STEMS",
            Self::Reduction => "REDUCTION",
            Self::CueBeams => "CUE_BEAMS",
            Self::Texts => "TEXTS",
            Self::Measures => "MEASURES",
            Self::Chords => "CHORDS",
            Self::Curves => "CURVES",
            Self::Symbols => "SYMBOLS",
            Self::Links => "LINKS",
            Self::Rhythms => "RHYTHMS",
            Self::Page => "PAGE",
        }
    }
}

impl fmt::Display for OmrStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for OmrStep {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|step| step.as_str().eq_ignore_ascii_case(value))
            .ok_or_else(|| format!("unknown OMR step: {value}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn freezes_java_pipeline_order() {
        assert_eq!(OmrStep::ALL[0], OmrStep::Load);
        assert_eq!(OmrStep::ALL[17], OmrStep::Links);
        assert_eq!(OmrStep::ALL[19], OmrStep::Page);
        assert_eq!("STEM_SEEDS".parse(), Ok(OmrStep::StemSeeds));
    }
}
