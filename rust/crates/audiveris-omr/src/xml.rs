// SPDX-License-Identifier: AGPL-3.0-or-later

//! Conservative, read-only metadata extracted from Audiveris `book.xml`.
//!
//! This view deliberately owns and returns the original byte document. It does
//! not deserialize the document into a writable object model, so unknown
//! attributes and elements cannot be lost through a parse/serialize cycle.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::io::Cursor;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

const BOOK_ELEMENT: &[u8] = b"book";
const SHEET_ELEMENT: &[u8] = b"sheet";
const SOFTWARE_VERSION_ATTRIBUTE: &[u8] = b"software-version";
const NUMBER_ATTRIBUTE: &[u8] = b"number";

/// A lossless, read-only view of an Audiveris `book.xml` document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BookXml {
    original: Vec<u8>,
    root_element: String,
    software_version: Option<String>,
    sheet_stubs: Vec<SheetStub>,
}

impl BookXml {
    /// Parse the narrow metadata view while retaining `original` byte-for-byte.
    pub fn parse(original: impl AsRef<[u8]>) -> Result<Self, BookXmlError> {
        let original = original.as_ref().to_vec();
        let mut reader = Reader::from_reader(Cursor::new(original.as_slice()));
        reader.config_mut().trim_text(false);

        let mut buffer = Vec::new();
        let mut depth = 0_usize;
        let mut root_element = None;
        let mut software_version = None;
        let mut sheet_stubs = Vec::new();
        let mut sheet_numbers = HashSet::new();
        let mut root_closed = false;

        loop {
            let event = reader.read_event_into(&mut buffer).map_err(|error| {
                BookXmlError::malformed(reader.error_position(), error.to_string())
            })?;

            match event {
                Event::Start(element) => {
                    if depth == 0 {
                        if root_element.is_some() || root_closed {
                            return Err(BookXmlError::MultipleRootElements);
                        }
                        let (name, version) = parse_book_root(&reader, &element)?;
                        root_element = Some(name);
                        software_version = version;
                    } else if depth == 1 && element.local_name().as_ref() == SHEET_ELEMENT {
                        push_sheet_stub(&reader, &element, &mut sheet_numbers, &mut sheet_stubs)?;
                    }
                    depth = depth.checked_add(1).ok_or_else(|| {
                        BookXmlError::malformed(reader.buffer_position(), "XML nesting overflow")
                    })?;
                }
                Event::Empty(element) => {
                    if depth == 0 {
                        if root_element.is_some() || root_closed {
                            return Err(BookXmlError::MultipleRootElements);
                        }
                        let (name, version) = parse_book_root(&reader, &element)?;
                        root_element = Some(name);
                        software_version = version;
                        root_closed = true;
                    } else if depth == 1 && element.local_name().as_ref() == SHEET_ELEMENT {
                        push_sheet_stub(&reader, &element, &mut sheet_numbers, &mut sheet_stubs)?;
                    }
                }
                Event::End(_) => {
                    depth = depth.checked_sub(1).ok_or_else(|| {
                        BookXmlError::malformed(reader.buffer_position(), "unexpected closing tag")
                    })?;
                    if depth == 0 {
                        root_closed = true;
                    }
                }
                Event::Text(text) if depth == 0 => {
                    let text = text.xml_content().map_err(|error| {
                        BookXmlError::malformed(reader.error_position(), error.to_string())
                    })?;
                    if !text.trim().is_empty() {
                        return Err(BookXmlError::ContentOutsideRoot);
                    }
                }
                Event::CData(text) if depth == 0 => {
                    let text = text.xml_content().map_err(|error| {
                        BookXmlError::malformed(reader.error_position(), error.to_string())
                    })?;
                    if !text.trim().is_empty() {
                        return Err(BookXmlError::ContentOutsideRoot);
                    }
                }
                Event::GeneralRef(_) if depth == 0 => {
                    return Err(BookXmlError::ContentOutsideRoot);
                }
                Event::Eof => break,
                _ => {}
            }
            buffer.clear();
        }

        let root_element = root_element.ok_or(BookXmlError::MissingRootElement)?;
        if depth != 0 || !root_closed {
            return Err(BookXmlError::malformed(
                reader.buffer_position(),
                "unclosed root element",
            ));
        }

        Ok(Self {
            original,
            root_element,
            software_version,
            sheet_stubs,
        })
    }

    /// The root element's qualified spelling, including any namespace prefix.
    #[must_use]
    pub fn root_element(&self) -> &str {
        &self.root_element
    }

    /// The root `software-version` attribute, when present.
    #[must_use]
    pub fn software_version(&self) -> Option<&str> {
        self.software_version.as_deref()
    }

    /// Direct child sheet stubs in document order.
    #[must_use]
    pub fn sheet_stubs(&self) -> &[SheetStub] {
        &self.sheet_stubs
    }

    /// The exact input bytes, without XML normalization or reserialization.
    #[must_use]
    pub fn original_bytes(&self) -> &[u8] {
        &self.original
    }
}

/// Narrow metadata for one direct `<sheet>` child of `<book>`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SheetStub {
    number: u32,
    archive_path: String,
}

impl SheetStub {
    /// One-based sheet number recorded in the stub's `number` attribute.
    #[must_use]
    pub const fn number(&self) -> u32 {
        self.number
    }

    /// Conventional path of this sheet's JAXB document inside the `.omr` ZIP.
    #[must_use]
    pub fn archive_path(&self) -> &str {
        &self.archive_path
    }
}

/// Failure to construct the conservative `book.xml` metadata view.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BookXmlError {
    /// The XML parser rejected malformed input.
    Malformed {
        /// Approximate byte offset at which parsing failed.
        position: u64,
        /// Parser diagnostic.
        message: String,
    },
    /// The document has no root element.
    MissingRootElement,
    /// The document contains more than one top-level element.
    MultipleRootElements,
    /// Non-whitespace character data appears outside the root element.
    ContentOutsideRoot,
    /// The document root is not an Audiveris `book` element.
    UnexpectedRootElement(String),
    /// A direct child `sheet` has no unqualified `number` attribute.
    MissingSheetNumber,
    /// A sheet number is not a positive decimal integer representable as `u32`.
    InvalidSheetNumber(String),
    /// Two direct child sheet stubs declare the same number.
    DuplicateSheetNumber(u32),
}

impl BookXmlError {
    fn malformed(position: u64, message: impl Into<String>) -> Self {
        Self::Malformed {
            position,
            message: message.into(),
        }
    }
}

impl fmt::Display for BookXmlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { position, message } => {
                write!(
                    formatter,
                    "malformed book XML near byte {position}: {message}"
                )
            }
            Self::MissingRootElement => write!(formatter, "book XML has no root element"),
            Self::MultipleRootElements => write!(formatter, "book XML has multiple root elements"),
            Self::ContentOutsideRoot => {
                write!(formatter, "book XML has content outside its root element")
            }
            Self::UnexpectedRootElement(name) => {
                write!(formatter, "expected book XML root, found {name:?}")
            }
            Self::MissingSheetNumber => write!(formatter, "sheet stub has no number attribute"),
            Self::InvalidSheetNumber(number) => {
                write!(formatter, "invalid sheet stub number {number:?}")
            }
            Self::DuplicateSheetNumber(number) => {
                write!(formatter, "duplicate sheet stub number {number}")
            }
        }
    }
}

impl Error for BookXmlError {}

fn parse_book_root(
    reader: &Reader<Cursor<&[u8]>>,
    element: &BytesStart<'_>,
) -> Result<(String, Option<String>), BookXmlError> {
    let qualified_name = decode_name(element.name().as_ref(), reader.buffer_position())?;
    if element.local_name().as_ref() != BOOK_ELEMENT {
        return Err(BookXmlError::UnexpectedRootElement(qualified_name));
    }

    let mut software_version = None;
    for attribute in element.attributes() {
        let attribute = attribute
            .map_err(|error| BookXmlError::malformed(reader.error_position(), error.to_string()))?;
        if attribute.key.as_ref() == SOFTWARE_VERSION_ATTRIBUTE {
            software_version = Some(
                attribute
                    .decode_and_unescape_value(reader.decoder())
                    .map_err(|error| {
                        BookXmlError::malformed(reader.error_position(), error.to_string())
                    })?
                    .into_owned(),
            );
        }
    }

    Ok((qualified_name, software_version))
}

fn push_sheet_stub(
    reader: &Reader<Cursor<&[u8]>>,
    element: &BytesStart<'_>,
    sheet_numbers: &mut HashSet<u32>,
    sheet_stubs: &mut Vec<SheetStub>,
) -> Result<(), BookXmlError> {
    let mut number = None;
    for attribute in element.attributes() {
        let attribute = attribute
            .map_err(|error| BookXmlError::malformed(reader.error_position(), error.to_string()))?;
        if attribute.key.as_ref() == NUMBER_ATTRIBUTE {
            let value = attribute
                .decode_and_unescape_value(reader.decoder())
                .map_err(|error| {
                    BookXmlError::malformed(reader.error_position(), error.to_string())
                })?
                .into_owned();
            let parsed = value
                .parse::<u32>()
                .ok()
                .filter(|candidate| *candidate > 0)
                .ok_or_else(|| BookXmlError::InvalidSheetNumber(value.clone()))?;
            number = Some(parsed);
        }
    }

    let number = number.ok_or(BookXmlError::MissingSheetNumber)?;
    if !sheet_numbers.insert(number) {
        return Err(BookXmlError::DuplicateSheetNumber(number));
    }
    sheet_stubs.push(SheetStub {
        number,
        archive_path: format!("sheet#{number}/sheet#{number}.xml"),
    });
    Ok(())
}

fn decode_name(name: &[u8], position: u64) -> Result<String, BookXmlError> {
    std::str::from_utf8(name)
        .map(str::to_owned)
        .map_err(|error| BookXmlError::malformed(position, error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_namespaced_book_and_direct_sheet_stubs() {
        let xml = br#"<?xml version="1.0"?>
            <av:book xmlns:av="urn:audiveris" software-version="5.11.0">
              <av:sheet number="1"/>
              <av:sheet number="7"><unknown/></av:sheet>
            </av:book>"#;

        let book = BookXml::parse(xml).unwrap();

        assert_eq!(book.root_element(), "av:book");
        assert_eq!(book.software_version(), Some("5.11.0"));
        assert_eq!(book.sheet_stubs().len(), 2);
        assert_eq!(book.sheet_stubs()[0].number(), 1);
        assert_eq!(book.sheet_stubs()[0].archive_path(), "sheet#1/sheet#1.xml");
        assert_eq!(book.sheet_stubs()[1].number(), 7);
        assert_eq!(book.sheet_stubs()[1].archive_path(), "sheet#7/sheet#7.xml");
    }

    #[test]
    fn ignores_unknown_attributes_nodes_and_nested_sheet_names() {
        let xml = br#"<book software-version="5&amp;11" future="yes">
            <future><sheet number="99"/></future>
            <sheet number="2" alien="preserve-me"><anything/></sheet>
            <score><page sheet-number="2"/></score>
        </book>"#;

        let book = BookXml::parse(xml).unwrap();

        assert_eq!(book.software_version(), Some("5&11"));
        assert_eq!(book.sheet_stubs().len(), 1);
        assert_eq!(book.sheet_stubs()[0].number(), 2);
    }

    #[test]
    fn retains_original_document_exactly() {
        let xml = b"\xef\xbb\xbf<?xml version=\"1.0\" ?>\r\n<book odd=\"&amp;\"><!-- keep -->\r\n  <sheet number=\"3\"/>\r\n</book>\r\n";

        let book = BookXml::parse(xml).unwrap();

        assert_eq!(book.original_bytes(), xml);
    }

    #[test]
    fn rejects_malformed_xml() {
        let error = BookXml::parse(b"<book><sheet number=\"1\"></book>").unwrap_err();
        assert!(matches!(error, BookXmlError::Malformed { .. }));
    }

    #[test]
    fn rejects_duplicate_sheet_numbers() {
        let error =
            BookXml::parse(b"<book><sheet number=\"4\"/><sheet number=\"4\"/></book>").unwrap_err();
        assert_eq!(error, BookXmlError::DuplicateSheetNumber(4));
    }

    #[test]
    fn rejects_invalid_sheet_numbers() {
        for invalid in ["0", "-1", "not-a-number", "4294967296"] {
            let xml = format!("<book><sheet number=\"{invalid}\"/></book>");
            let error = BookXml::parse(xml).unwrap_err();
            assert_eq!(error, BookXmlError::InvalidSheetNumber(invalid.to_owned()));
        }
    }

    #[test]
    fn rejects_missing_sheet_number() {
        let error = BookXml::parse(b"<book><sheet future-number=\"1\"/></book>").unwrap_err();
        assert_eq!(error, BookXmlError::MissingSheetNumber);
    }

    #[test]
    fn rejects_non_book_and_multiple_roots() {
        assert_eq!(
            BookXml::parse(b"<score/>").unwrap_err(),
            BookXmlError::UnexpectedRootElement("score".to_owned())
        );
        assert_eq!(
            BookXml::parse(b"<book/><book/>").unwrap_err(),
            BookXmlError::MultipleRootElements
        );
    }
}
