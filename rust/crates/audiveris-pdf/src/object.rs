//! The eight basic object types, plus streams and indirect references.
//!
//! The shape follows PDFBox's `COS` layer rather than the specification's
//! grouping, because PDFBox is the behaviour being reproduced and it makes two
//! distinctions the specification does not:
//!
//! - Integers and reals are separate types that never unify. `/Width 8.0` is a
//!   real, and PDFBox's `getInt` on it returns the default, not 8.
//! - A stream is a dictionary *subtype*, not a dictionary with a payload
//!   beside it, so anything that reads a dictionary reads a stream's too.
//!
//! Names are stored decoded -- `#20` has already become a space -- because that
//! is the form every lookup uses, and comparing raw would make `/A#42` and
//! `/AB` different keys when the specification says they are the same.

use std::collections::BTreeMap;

/// A reference to an object that lives elsewhere in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Reference {
    /// The object number. Zero is reserved for the free-list head.
    pub number: u32,
    /// The generation number, bumped when an object number is reused.
    pub generation: u16,
}

impl Reference {
    /// A reference to `number` at generation zero, the overwhelmingly common
    /// case in files that have never been incrementally updated.
    #[must_use]
    pub const fn new(number: u32) -> Self {
        Self {
            number,
            generation: 0,
        }
    }
}

/// A PDF name, stored with its `#xx` escapes already resolved.
///
/// Names are not text: the specification allows any byte, and while everything
/// a score PDF contains is ASCII, decoding to `String` would make the parser
/// reject a file PDFBox accepts.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Name(pub Vec<u8>);

impl Name {
    /// A name from a string literal, for the constants below and for lookups.
    #[must_use]
    pub fn new(text: &str) -> Self {
        Self(text.as_bytes().to_vec())
    }

    /// The name's bytes, for comparison against a literal.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl PartialEq<str> for Name {
    fn eq(&self, other: &str) -> bool {
        self.0 == other.as_bytes()
    }
}

/// A dictionary, keyed by name.
///
/// Ordered rather than hashed so that iteration is deterministic, which matters
/// for the diffs this port is checked with. A duplicate key takes its last
/// value, as PDFBox's `setItem` into a `LinkedHashMap` does.
pub type Dictionary = BTreeMap<Name, Object>;

/// A stream: its dictionary, and its bytes exactly as they sit in the file.
///
/// The bytes are *not* decoded here. Which filters apply, and whether a filter
/// even can be applied without knowing the image's geometry, is a question for
/// the layer above; keeping the raw form means a file can be parsed and
/// inspected before any filter exists.
#[derive(Debug, Clone, PartialEq)]
pub struct Stream {
    /// The stream's dictionary, including `/Length` and `/Filter`.
    pub dictionary: Dictionary,
    /// The raw bytes between `stream` and `endstream`.
    pub data: Vec<u8>,
}

/// Any PDF object.
#[derive(Debug, Clone, PartialEq)]
pub enum Object {
    /// The `null` keyword, and also what a dangling reference resolves to.
    Null,
    /// `true` or `false`.
    Boolean(bool),
    /// An integer. Kept distinct from [`Object::Real`], as PDFBox keeps it.
    Integer(i64),
    /// A real. PDF has no exponent notation, though PDFBox tolerates it.
    Real(f64),
    /// A string, literal or hexadecimal, with escapes already resolved.
    String(Vec<u8>),
    /// A name, with `#xx` escapes already resolved.
    Name(Name),
    /// An array. Heterogeneous, and may contain references.
    Array(Vec<Object>),
    /// A dictionary.
    Dictionary(Dictionary),
    /// A stream.
    Stream(Stream),
    /// An indirect reference, `n g R`.
    Reference(Reference),
}

impl Object {
    /// The object's dictionary, whether it is one or is a stream.
    #[must_use]
    pub fn dictionary(&self) -> Option<&Dictionary> {
        match self {
            Self::Dictionary(dictionary) => Some(dictionary),
            Self::Stream(stream) => Some(&stream.dictionary),
            _ => None,
        }
    }

    /// The object as a stream.
    #[must_use]
    pub fn stream(&self) -> Option<&Stream> {
        match self {
            Self::Stream(stream) => Some(stream),
            _ => None,
        }
    }

    /// The object as an array.
    #[must_use]
    pub fn array(&self) -> Option<&[Object]> {
        match self {
            Self::Array(entries) => Some(entries),
            _ => None,
        }
    }

    /// The object as a name.
    #[must_use]
    pub fn name(&self) -> Option<&Name> {
        match self {
            Self::Name(name) => Some(name),
            _ => None,
        }
    }

    /// The object as an integer.
    ///
    /// A real does *not* convert, matching `COSDictionary.getInt`, which
    /// returns its default for anything that is not a `COSNumber`... and a
    /// `COSFloat` is a `COSNumber`, so it does convert there. The difference is
    /// deliberate: callers here ask for [`Object::as_f64`] when either is
    /// acceptable, so that a file writing `/Length 512.0` cannot silently take
    /// a path meant for integers.
    #[must_use]
    pub const fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Integer(value) => Some(*value),
            _ => None,
        }
    }

    /// The object as a number, integer or real.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Integer(value) => Some(*value as f64),
            Self::Real(value) => Some(*value),
            _ => None,
        }
    }

    /// The object as a reference.
    #[must_use]
    pub const fn as_reference(&self) -> Option<Reference> {
        match self {
            Self::Reference(reference) => Some(*reference),
            _ => None,
        }
    }

    /// Whether this is [`Object::Null`].
    #[must_use]
    pub const fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
}

/// Reads `key` from `dictionary` without resolving references.
///
/// Most callers want [`crate::document::Document::get`] instead, which
/// resolves; this exists for the places that must not, such as reading
/// `/Length` while the cross-reference table is still being built.
#[must_use]
pub fn direct<'a>(dictionary: &'a Dictionary, key: &str) -> Option<&'a Object> {
    dictionary.get(&Name::new(key))
}
