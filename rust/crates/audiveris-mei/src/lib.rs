// SPDX-License-Identifier: AGPL-3.0-or-later

//! Deterministic MEI 5.1 CMN serialization for native Audiveris score data.
//!
//! This crate owns serialization and its validated semantic input contract. It
//! deliberately contains no recognition logic and has no dependency on
//! `audiveris-omr`. A future PAGE-stage adapter can populate [`MeiDocument`]
//! once native recognition exposes resolved measures, layers, pitches, and
//! durations. Until then, callers must provide those semantics explicitly;
//! this crate never guesses them from HEADS geometry or opaque `.omr` XML.

#![forbid(unsafe_code)]

use std::error::Error;
use std::fmt;
use std::io::{self, Write};

mod model;
mod validation;
mod writer;

pub use model::*;
pub use validation::{MeiCounts, ValidationViolation, ViolationCode};

/// A fully validated, deterministic MEI document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeiOutput {
    bytes: Vec<u8>,
    counts: MeiCounts,
}

impl MeiOutput {
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub const fn counts(&self) -> MeiCounts {
        self.counts
    }

    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

#[derive(Debug)]
pub enum MeiExportError {
    Validation(Vec<ValidationViolation>),
    Io(io::Error),
}

impl fmt::Display for MeiExportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(violations) => {
                write!(
                    formatter,
                    "MEI input has {} validation error(s)",
                    violations.len()
                )?;
                for violation in violations {
                    write!(formatter, "; {violation}")?;
                }
                Ok(())
            }
            Self::Io(error) => write!(formatter, "MEI output failed: {error}"),
        }
    }
}

impl Error for MeiExportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Validation(_) => None,
            Self::Io(error) => Some(error),
        }
    }
}

impl From<io::Error> for MeiExportError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Validate and serialize one sheet in the deterministic Audiveris MEI 5.1 CMN
/// Phase 1 profile.
///
/// The result is XML 1.0 UTF-8 without a BOM, uses LF plus two-space
/// indentation, has a final newline, and never consults wall-clock time.
pub fn serialize_mei(document: &MeiDocument) -> Result<MeiOutput, MeiExportError> {
    let counts = validation::validate(document).map_err(MeiExportError::Validation)?;
    let bytes = writer::serialize(document)?;
    Ok(MeiOutput { bytes, counts })
}

/// Validate completely, then write one deterministic MEI document.
///
/// Invalid input performs no writes. An underlying I/O failure is propagated;
/// as with [`Write::write_all`], bytes accepted before that external failure
/// cannot be rolled back.
pub fn write_mei<W: Write>(
    document: &MeiDocument,
    mut destination: W,
) -> Result<MeiCounts, MeiExportError> {
    let output = serialize_mei(document)?;
    destination.write_all(output.as_bytes())?;
    Ok(output.counts())
}
