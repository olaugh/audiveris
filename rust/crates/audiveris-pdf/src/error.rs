//! What can go wrong reading a PDF.
//!
//! Deliberately coarse. PDFBox recovers from nearly everything -- a broken
//! cross-reference table sends it off to scan the whole file for `obj`
//! keywords -- so a variant here is not "the file is invalid", it is "this port
//! has met something it does not yet reproduce". Each one should be readable
//! enough to say which of those it is.

use std::fmt;

/// A failure reading a PDF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The input ended in the middle of something.
    Truncated,
    /// A byte appeared where the grammar allows no object to start.
    UnexpectedByte {
        /// The offending byte.
        found: u8,
        /// Where it was.
        offset: usize,
    },
    /// A keyword was expected and something else was there.
    ExpectedKeyword {
        /// What the grammar required.
        expected: &'static str,
        /// Where it was required.
        offset: usize,
    },
    /// No `%PDF-` header within the first kilobyte.
    MissingHeader,
    /// No `startxref` before the end of the file.
    MissingStartXref,
    /// A cross-reference section was malformed beyond recovery.
    BadCrossReference {
        /// Where the section began.
        offset: usize,
    },
    /// The trailer named no `/Root`, so there is no catalog to render from.
    MissingCatalog,
    /// The catalog's page tree was missing, cyclic, or held no pages.
    BadPageTree,
    /// A page index past the end of the document.
    NoSuchPage {
        /// The index asked for.
        index: usize,
    },
    /// An object number that no cross-reference entry covers.
    MissingObject {
        /// The object number asked for.
        number: u32,
    },
    /// An object stream that is not one, or whose index is inconsistent.
    BadObjectStream {
        /// The object stream's own object number.
        number: u32,
    },
    /// The file is encrypted. Even an empty-password file needs the standard
    /// security handler before anything decodes, and that is not written yet.
    Encrypted,
    /// A filter this port does not implement.
    UnsupportedFilter {
        /// The filter's name, as it appeared in `/Filter`.
        name: String,
    },
    /// A filter's input was malformed.
    CorruptStream {
        /// Which filter rejected it.
        filter: &'static str,
    },
    /// An image whose sample layout this port does not reproduce.
    ///
    /// Refused by name rather than approximated, on the same reasoning as the
    /// JBIG2 segment types: the corpus was measured first, and everything it
    /// actually contains is implemented. A silently wrong raster would be
    /// indistinguishable from a correct one until a staff line moved.
    UnsupportedImage {
        /// What the image asked for, in the terms the dictionary uses.
        reason: String,
    },
    /// A structure exceeded a limit that guards against pathological input.
    TooDeep,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => write!(formatter, "input ended mid-object"),
            Self::UnexpectedByte { found, offset } => write!(
                formatter,
                "no object can start with {found:#04x} at offset {offset}"
            ),
            Self::ExpectedKeyword { expected, offset } => {
                write!(formatter, "expected `{expected}` at offset {offset}")
            }
            Self::MissingHeader => write!(formatter, "no %PDF- header"),
            Self::MissingStartXref => write!(formatter, "no startxref"),
            Self::BadCrossReference { offset } => {
                write!(formatter, "malformed cross-reference at offset {offset}")
            }
            Self::MissingCatalog => write!(formatter, "trailer names no /Root"),
            Self::BadPageTree => write!(formatter, "unusable page tree"),
            Self::NoSuchPage { index } => write!(formatter, "no page at index {index}"),
            Self::MissingObject { number } => {
                write!(formatter, "object {number} is not in the file")
            }
            Self::BadObjectStream { number } => {
                write!(formatter, "object {number} is not a usable object stream")
            }
            Self::Encrypted => write!(formatter, "encrypted PDFs are not supported yet"),
            Self::UnsupportedFilter { name } => write!(formatter, "unsupported filter /{name}"),
            Self::CorruptStream { filter } => write!(formatter, "corrupt {filter} stream"),
            Self::UnsupportedImage { reason } => {
                write!(formatter, "unsupported image: {reason}")
            }
            Self::TooDeep => write!(formatter, "nesting is too deep"),
        }
    }
}

impl std::error::Error for Error {}

/// The crate's result type.
pub type Result<T> = std::result::Result<T, Error>;
