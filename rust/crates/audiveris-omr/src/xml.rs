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

use audiveris_core::step::OmrStep;
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

const BOOK_ELEMENT: &[u8] = b"book";
const SHEET_ELEMENT: &[u8] = b"sheet";
const INPUT_ELEMENT: &[u8] = b"input";
const PATH_ELEMENT: &[u8] = b"path";
const INPUT_NUMBER_ELEMENT: &[u8] = b"number";
const STEPS_ELEMENT: &[u8] = b"steps";
const PAGE_ELEMENT: &[u8] = b"page";
const SOFTWARE_VERSION_ATTRIBUTE: &[u8] = b"software-version";
const NUMBER_ATTRIBUTE: &[u8] = b"number";
const VERSION_ATTRIBUTE: &[u8] = b"version";
const INVALID_ATTRIBUTE: &[u8] = b"invalid";
const ID_ATTRIBUTE: &[u8] = b"id";
const MOVEMENT_START_ATTRIBUTE: &[u8] = b"movement-start";
const DELTA_MEASURE_ID_ATTRIBUTE: &[u8] = b"delta-measure-id";

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
        let mut sheet_stubs: Vec<SheetStub> = Vec::new();
        let mut sheet_numbers = HashSet::new();
        let mut active_sheet = None;
        let mut active_input: Option<(usize, SheetInputBuilder)> = None;
        let mut active_input_scalar: Option<InputScalarCapture> = None;
        let mut active_steps: Option<(usize, String)> = None;
        let mut root_closed = false;

        loop {
            let event = reader.read_event_into(&mut buffer).map_err(|error| {
                BookXmlError::malformed(reader.error_position(), error.to_string())
            })?;

            match event {
                Event::Start(element) => {
                    if let Some((sheet_index, _)) = active_steps.as_ref() {
                        return Err(BookXmlError::UnexpectedStepsContent(
                            sheet_stubs[*sheet_index].number,
                        ));
                    }
                    if let Some(capture) = active_input_scalar.as_ref() {
                        return Err(BookXmlError::UnexpectedInputScalarContent {
                            sheet_number: sheet_stubs[capture.sheet_index].number,
                            field: capture.scalar.field(),
                        });
                    }
                    if depth == 0 {
                        if root_element.is_some() || root_closed {
                            return Err(BookXmlError::MultipleRootElements);
                        }
                        let (name, version) = parse_book_root(&reader, &element)?;
                        root_element = Some(name);
                        software_version = version;
                    } else if depth == 1 && element.local_name().as_ref() == SHEET_ELEMENT {
                        push_sheet_stub(&reader, &element, &mut sheet_numbers, &mut sheet_stubs)?;
                        active_sheet = Some(sheet_stubs.len() - 1);
                    } else if depth == 2
                        && element.local_name().as_ref() == INPUT_ELEMENT
                        && let Some(sheet_index) = active_sheet
                    {
                        begin_input(&sheet_stubs, sheet_index)?;
                        active_input = Some((sheet_index, SheetInputBuilder::default()));
                    } else if depth == 3
                        && let Some((sheet_index, builder)) = active_input.as_ref()
                        && let Some(scalar) = input_scalar(element.local_name().as_ref())
                    {
                        begin_input_scalar(&sheet_stubs, *sheet_index, builder, scalar)?;
                        active_input_scalar = Some(InputScalarCapture {
                            sheet_index: *sheet_index,
                            scalar,
                            text: String::new(),
                        });
                    } else if depth == 2
                        && element.local_name().as_ref() == STEPS_ELEMENT
                        && let Some(sheet_index) = active_sheet
                    {
                        begin_steps(&sheet_stubs, sheet_index)?;
                        active_steps = Some((sheet_index, String::new()));
                    } else if depth == 2
                        && element.local_name().as_ref() == PAGE_ELEMENT
                        && let Some(sheet_index) = active_sheet
                    {
                        push_page_ref(&reader, &element, &mut sheet_stubs[sheet_index])?;
                    }
                    depth = depth.checked_add(1).ok_or_else(|| {
                        BookXmlError::malformed(reader.buffer_position(), "XML nesting overflow")
                    })?;
                }
                Event::Empty(element) => {
                    if let Some((sheet_index, _)) = active_steps.as_ref() {
                        return Err(BookXmlError::UnexpectedStepsContent(
                            sheet_stubs[*sheet_index].number,
                        ));
                    }
                    if let Some(capture) = active_input_scalar.as_ref() {
                        return Err(BookXmlError::UnexpectedInputScalarContent {
                            sheet_number: sheet_stubs[capture.sheet_index].number,
                            field: capture.scalar.field(),
                        });
                    }
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
                    } else if depth == 2
                        && element.local_name().as_ref() == INPUT_ELEMENT
                        && let Some(sheet_index) = active_sheet
                    {
                        begin_input(&sheet_stubs, sheet_index)?;
                        return Err(BookXmlError::MissingInputField {
                            sheet_number: sheet_stubs[sheet_index].number,
                            field: "sheet/input/path",
                        });
                    } else if depth == 3
                        && let Some((sheet_index, builder)) = active_input.as_mut()
                        && let Some(scalar) = input_scalar(element.local_name().as_ref())
                    {
                        begin_input_scalar(&sheet_stubs, *sheet_index, builder, scalar)?;
                        finish_input_scalar(
                            &sheet_stubs,
                            *sheet_index,
                            builder,
                            scalar,
                            String::new(),
                        )?;
                    } else if depth == 2
                        && element.local_name().as_ref() == STEPS_ELEMENT
                        && let Some(sheet_index) = active_sheet
                    {
                        begin_steps(&sheet_stubs, sheet_index)?;
                        sheet_stubs[sheet_index].done_steps = Some(Vec::new());
                    } else if depth == 2
                        && element.local_name().as_ref() == PAGE_ELEMENT
                        && let Some(sheet_index) = active_sheet
                    {
                        push_page_ref(&reader, &element, &mut sheet_stubs[sheet_index])?;
                    }
                }
                Event::End(element) => {
                    if depth == 4
                        && let Some(capture) = active_input_scalar.take()
                    {
                        let (_, builder) = active_input.as_mut().ok_or_else(|| {
                            BookXmlError::malformed(
                                reader.buffer_position(),
                                "input scalar closed outside input",
                            )
                        })?;
                        finish_input_scalar(
                            &sheet_stubs,
                            capture.sheet_index,
                            builder,
                            capture.scalar,
                            capture.text,
                        )?;
                    }
                    if element.local_name().as_ref() == STEPS_ELEMENT
                        && depth == 3
                        && let Some((sheet_index, text)) = active_steps.take()
                    {
                        sheet_stubs[sheet_index].done_steps =
                            Some(parse_steps(&text, sheet_stubs[sheet_index].number)?);
                    }
                    if element.local_name().as_ref() == INPUT_ELEMENT
                        && depth == 3
                        && let Some((sheet_index, builder)) = active_input.take()
                    {
                        sheet_stubs[sheet_index].input =
                            Some(finish_input(&sheet_stubs, sheet_index, builder)?);
                    }
                    if depth == 2 && active_sheet.is_some() {
                        active_sheet = None;
                    }
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
                Event::Text(text) if active_steps.is_some() => {
                    let decoded = text.xml_content().map_err(|error| {
                        BookXmlError::malformed(reader.error_position(), error.to_string())
                    })?;
                    active_steps.as_mut().unwrap().1.push_str(&decoded);
                }
                Event::Text(text) if active_input_scalar.is_some() => {
                    let decoded = text.xml_content().map_err(|error| {
                        BookXmlError::malformed(reader.error_position(), error.to_string())
                    })?;
                    active_input_scalar
                        .as_mut()
                        .unwrap()
                        .text
                        .push_str(&decoded);
                }
                Event::CData(text) if depth == 0 => {
                    let text = text.xml_content().map_err(|error| {
                        BookXmlError::malformed(reader.error_position(), error.to_string())
                    })?;
                    if !text.trim().is_empty() {
                        return Err(BookXmlError::ContentOutsideRoot);
                    }
                }
                Event::CData(text) if active_steps.is_some() => {
                    let decoded = text.xml_content().map_err(|error| {
                        BookXmlError::malformed(reader.error_position(), error.to_string())
                    })?;
                    active_steps.as_mut().unwrap().1.push_str(&decoded);
                }
                Event::CData(text) if active_input_scalar.is_some() => {
                    let decoded = text.xml_content().map_err(|error| {
                        BookXmlError::malformed(reader.error_position(), error.to_string())
                    })?;
                    active_input_scalar
                        .as_mut()
                        .unwrap()
                        .text
                        .push_str(&decoded);
                }
                Event::GeneralRef(_) if depth == 0 => {
                    return Err(BookXmlError::ContentOutsideRoot);
                }
                Event::GeneralRef(_) if active_steps.is_some() => {
                    let sheet_index = active_steps.as_ref().unwrap().0;
                    return Err(BookXmlError::UnexpectedStepsContent(
                        sheet_stubs[sheet_index].number,
                    ));
                }
                Event::GeneralRef(_) if active_input_scalar.is_some() => {
                    let capture = active_input_scalar.as_ref().unwrap();
                    return Err(BookXmlError::UnexpectedInputScalarContent {
                        sheet_number: sheet_stubs[capture.sheet_index].number,
                        field: capture.scalar.field(),
                    });
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
    version: Option<String>,
    invalid: Option<bool>,
    input: Option<SheetInput>,
    done_steps: Option<Vec<OmrStep>>,
    page_refs: Vec<PageRef>,
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

    /// Sheet-specific Audiveris version override, when explicitly persisted.
    ///
    /// Java falls back to the book version when this attribute is absent.
    #[must_use]
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// Explicit lexical state of the JAXB boolean-positive `invalid` attribute.
    ///
    /// The adapter normally omits false during marshalling, but accepts an
    /// explicitly persisted false value while unmarshalling.
    #[must_use]
    pub const fn invalid_attribute(&self) -> Option<bool> {
        self.invalid
    }

    /// Effective Java invalidity state; an absent attribute defaults to false.
    #[must_use]
    pub fn is_invalid(&self) -> bool {
        self.invalid.unwrap_or(false)
    }

    /// Explicit source-image provenance from the optional direct `input` child.
    ///
    /// When absent, Java falls back to the book input path and sheet number.
    #[must_use]
    pub const fn input(&self) -> Option<&SheetInput> {
        self.input.as_ref()
    }

    /// Completed stages recorded by the optional direct `steps` XML list.
    ///
    /// `None` distinguishes an absent element from an explicitly empty list.
    #[must_use]
    pub fn done_steps(&self) -> Option<&[OmrStep]> {
        self.done_steps.as_deref()
    }

    /// Latest completed stage by Java enum declaration order.
    #[must_use]
    pub fn latest_done_step(&self) -> Option<OmrStep> {
        self.done_steps
            .as_deref()
            .and_then(|steps| steps.iter().copied().max())
    }

    /// Direct JAXB page references in persisted document order.
    ///
    /// JAXB uses an unwrapped repeated element, so an absent sequence is
    /// represented by the same empty slice as Java's initially empty list.
    #[must_use]
    pub fn page_refs(&self) -> &[PageRef] {
        &self.page_refs
    }
}

/// Lightweight attributes of one direct sheet-stub `page` reference.
///
/// Nested time, system, part, and staff structures deliberately remain opaque.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageRef {
    id: u32,
    movement_start: Option<bool>,
    delta_measure_id: Option<i32>,
}

impl PageRef {
    /// One-based page rank within the containing sheet.
    #[must_use]
    pub const fn id(self) -> u32 {
        self.id
    }

    /// Explicit state of the optional JAXB boolean-positive attribute.
    #[must_use]
    pub const fn movement_start_attribute(self) -> Option<bool> {
        self.movement_start
    }

    /// Effective Java movement-start state; absent defaults to false.
    #[must_use]
    pub fn is_movement_start(self) -> bool {
        self.movement_start.unwrap_or(false)
    }

    /// Optional measure-ID increment recorded for the page.
    #[must_use]
    pub const fn delta_measure_id(self) -> Option<i32> {
        self.delta_measure_id
    }
}

/// Explicit image source associated with one persisted sheet stub.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SheetInput {
    path: String,
    number: u32,
}

impl SheetInput {
    /// Path spelling passed through Java's `Jaxb.PathAdapter`.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// One-based image rank within the input file.
    #[must_use]
    pub const fn number(&self) -> u32 {
        self.number
    }
}

#[derive(Debug, Default)]
struct SheetInputBuilder {
    path: Option<String>,
    number: Option<u32>,
}

#[derive(Clone, Copy, Debug)]
enum InputScalar {
    Path,
    Number,
}

impl InputScalar {
    const fn field(self) -> &'static str {
        match self {
            Self::Path => "sheet/input/path",
            Self::Number => "sheet/input/number",
        }
    }
}

#[derive(Debug)]
struct InputScalarCapture {
    sheet_index: usize,
    scalar: InputScalar,
    text: String,
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
    /// A direct sheet stub contains more than one `input` element.
    DuplicateSheetInput(u32),
    /// A required scalar is absent from a present `input` element.
    MissingInputField {
        /// Sheet number containing the incomplete input.
        sheet_number: u32,
        /// Stable typed field path.
        field: &'static str,
    },
    /// A direct `input` scalar occurs more than once.
    DuplicateInputField {
        /// Sheet number containing the duplicate.
        sheet_number: u32,
        /// Stable typed field path.
        field: &'static str,
    },
    /// The input image rank is not a positive Java `int`.
    InvalidInputNumber {
        /// Sheet number containing the value.
        sheet_number: u32,
        /// Exact scalar text after XML decoding.
        value: String,
    },
    /// A typed `input` scalar contains nested markup or an entity reference.
    UnexpectedInputScalarContent {
        /// Sheet number containing the scalar.
        sheet_number: u32,
        /// Stable typed field path.
        field: &'static str,
    },
    /// The sheet `invalid` attribute is not an XML Schema boolean.
    InvalidSheetBoolean {
        /// Stable typed attribute path.
        field: &'static str,
        /// Exact decoded attribute value.
        value: String,
    },
    /// A direct page reference has no unqualified `id` attribute.
    MissingPageId(u32),
    /// A direct page reference integer is malformed or out of Java `int` range.
    InvalidPageInteger {
        /// Containing sheet number.
        sheet_number: u32,
        /// Stable typed attribute path.
        field: &'static str,
        /// Exact decoded attribute value.
        value: String,
    },
    /// A direct page reference boolean has an invalid XML Schema spelling.
    InvalidPageBoolean {
        /// Containing sheet number.
        sheet_number: u32,
        /// Stable typed attribute path.
        field: &'static str,
        /// Exact decoded attribute value.
        value: String,
    },
    /// Two direct page references in one sheet declare the same ID.
    DuplicatePageId {
        /// Containing sheet number.
        sheet_number: u32,
        /// Repeated page rank.
        page_id: u32,
    },
    /// A direct sheet stub contains more than one `steps` element.
    DuplicateSheetSteps(u32),
    /// A `steps` XML list contains an element or entity rather than plain text.
    UnexpectedStepsContent(u32),
    /// A `steps` XML list names a token absent from the current Java enum.
    UnknownOmrStep {
        /// Sheet number containing the token.
        sheet_number: u32,
        /// Exact unrecognized token.
        token: String,
    },
    /// One step token occurs more than once in a sheet's XML list.
    DuplicateOmrStep {
        /// Sheet number containing the duplicate.
        sheet_number: u32,
        /// Repeated stage.
        step: OmrStep,
    },
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
            Self::DuplicateSheetInput(number) => {
                write!(
                    formatter,
                    "sheet stub {number} has duplicate input elements"
                )
            }
            Self::MissingInputField {
                sheet_number,
                field,
            } => write!(
                formatter,
                "sheet stub {sheet_number} input is missing {field}"
            ),
            Self::DuplicateInputField {
                sheet_number,
                field,
            } => write!(formatter, "sheet stub {sheet_number} input repeats {field}"),
            Self::InvalidInputNumber {
                sheet_number,
                value,
            } => write!(
                formatter,
                "sheet stub {sheet_number} has invalid input image number {value:?}"
            ),
            Self::UnexpectedInputScalarContent {
                sheet_number,
                field,
            } => write!(
                formatter,
                "sheet stub {sheet_number} input field {field} contains non-text content"
            ),
            Self::InvalidSheetBoolean { field, value } => {
                write!(formatter, "invalid sheet boolean {field}: {value:?}")
            }
            Self::MissingPageId(sheet_number) => {
                write!(formatter, "sheet stub {sheet_number} page has no id")
            }
            Self::InvalidPageInteger {
                sheet_number,
                field,
                value,
            } => write!(
                formatter,
                "sheet stub {sheet_number} has invalid page integer {field}: {value:?}"
            ),
            Self::InvalidPageBoolean {
                sheet_number,
                field,
                value,
            } => write!(
                formatter,
                "sheet stub {sheet_number} has invalid page boolean {field}: {value:?}"
            ),
            Self::DuplicatePageId {
                sheet_number,
                page_id,
            } => write!(
                formatter,
                "sheet stub {sheet_number} has duplicate page ID {page_id}"
            ),
            Self::DuplicateSheetSteps(number) => {
                write!(
                    formatter,
                    "sheet stub {number} has duplicate steps elements"
                )
            }
            Self::UnexpectedStepsContent(number) => {
                write!(
                    formatter,
                    "sheet stub {number} steps contain non-text content"
                )
            }
            Self::UnknownOmrStep {
                sheet_number,
                token,
            } => write!(
                formatter,
                "sheet stub {sheet_number} has unknown OMR step {token:?}"
            ),
            Self::DuplicateOmrStep { sheet_number, step } => write!(
                formatter,
                "sheet stub {sheet_number} repeats OMR step {}",
                step.as_str()
            ),
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
    let mut version = None;
    let mut invalid = None;
    for attribute in element.attributes() {
        let attribute = attribute
            .map_err(|error| BookXmlError::malformed(reader.error_position(), error.to_string()))?;
        let key = attribute.key.as_ref();
        if key == NUMBER_ATTRIBUTE {
            let value = decode_attribute(reader, &attribute)?;
            number = Some(
                value
                    .parse::<u32>()
                    .ok()
                    .filter(|candidate| *candidate > 0)
                    .ok_or_else(|| BookXmlError::InvalidSheetNumber(value.clone()))?,
            );
        } else if key == VERSION_ATTRIBUTE {
            version = Some(decode_attribute(reader, &attribute)?);
        } else if key == INVALID_ATTRIBUTE {
            let value = decode_attribute(reader, &attribute)?;
            invalid = Some(parse_jaxb_boolean(&value).ok_or_else(|| {
                BookXmlError::InvalidSheetBoolean {
                    field: "sheet/@invalid",
                    value: value.clone(),
                }
            })?);
        }
    }

    let number = number.ok_or(BookXmlError::MissingSheetNumber)?;
    if !sheet_numbers.insert(number) {
        return Err(BookXmlError::DuplicateSheetNumber(number));
    }
    sheet_stubs.push(SheetStub {
        number,
        archive_path: format!("sheet#{number}/sheet#{number}.xml"),
        version,
        invalid,
        input: None,
        done_steps: None,
        page_refs: Vec::new(),
    });
    Ok(())
}

fn decode_attribute(
    reader: &Reader<Cursor<&[u8]>>,
    attribute: &quick_xml::events::attributes::Attribute<'_>,
) -> Result<String, BookXmlError> {
    attribute
        .decode_and_unescape_value(reader.decoder())
        .map(Into::into)
        .map_err(|error| BookXmlError::malformed(reader.error_position(), error.to_string()))
}

fn parse_jaxb_boolean(value: &str) -> Option<bool> {
    match value.trim() {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

fn push_page_ref(
    reader: &Reader<Cursor<&[u8]>>,
    element: &BytesStart<'_>,
    sheet_stub: &mut SheetStub,
) -> Result<(), BookXmlError> {
    let mut id = None;
    let mut movement_start = None;
    let mut delta_measure_id = None;

    for attribute in element.attributes() {
        let attribute = attribute
            .map_err(|error| BookXmlError::malformed(reader.error_position(), error.to_string()))?;
        let key = attribute.key.as_ref();
        if key == ID_ATTRIBUTE {
            let value = decode_attribute(reader, &attribute)?;
            id = Some(
                value
                    .trim()
                    .parse::<u32>()
                    .ok()
                    .filter(|id| *id > 0 && *id <= i32::MAX as u32)
                    .ok_or_else(|| BookXmlError::InvalidPageInteger {
                        sheet_number: sheet_stub.number,
                        field: "sheet/page/@id",
                        value: value.clone(),
                    })?,
            );
        } else if key == MOVEMENT_START_ATTRIBUTE {
            let value = decode_attribute(reader, &attribute)?;
            movement_start = Some(parse_jaxb_boolean(&value).ok_or_else(|| {
                BookXmlError::InvalidPageBoolean {
                    sheet_number: sheet_stub.number,
                    field: "sheet/page/@movement-start",
                    value: value.clone(),
                }
            })?);
        } else if key == DELTA_MEASURE_ID_ATTRIBUTE {
            let value = decode_attribute(reader, &attribute)?;
            delta_measure_id = Some(value.trim().parse::<i32>().map_err(|_| {
                BookXmlError::InvalidPageInteger {
                    sheet_number: sheet_stub.number,
                    field: "sheet/page/@delta-measure-id",
                    value: value.clone(),
                }
            })?);
        }
    }

    let id = id.ok_or(BookXmlError::MissingPageId(sheet_stub.number))?;
    if sheet_stub.page_refs.iter().any(|page| page.id == id) {
        return Err(BookXmlError::DuplicatePageId {
            sheet_number: sheet_stub.number,
            page_id: id,
        });
    }
    sheet_stub.page_refs.push(PageRef {
        id,
        movement_start,
        delta_measure_id,
    });
    Ok(())
}

fn begin_steps(sheet_stubs: &[SheetStub], sheet_index: usize) -> Result<(), BookXmlError> {
    if sheet_stubs[sheet_index].done_steps.is_some() {
        return Err(BookXmlError::DuplicateSheetSteps(
            sheet_stubs[sheet_index].number,
        ));
    }
    Ok(())
}

fn begin_input(sheet_stubs: &[SheetStub], sheet_index: usize) -> Result<(), BookXmlError> {
    if sheet_stubs[sheet_index].input.is_some() {
        return Err(BookXmlError::DuplicateSheetInput(
            sheet_stubs[sheet_index].number,
        ));
    }
    Ok(())
}

fn input_scalar(local_name: &[u8]) -> Option<InputScalar> {
    match local_name {
        PATH_ELEMENT => Some(InputScalar::Path),
        INPUT_NUMBER_ELEMENT => Some(InputScalar::Number),
        _ => None,
    }
}

fn begin_input_scalar(
    sheet_stubs: &[SheetStub],
    sheet_index: usize,
    builder: &SheetInputBuilder,
    scalar: InputScalar,
) -> Result<(), BookXmlError> {
    let populated = match scalar {
        InputScalar::Path => builder.path.is_some(),
        InputScalar::Number => builder.number.is_some(),
    };
    if populated {
        return Err(BookXmlError::DuplicateInputField {
            sheet_number: sheet_stubs[sheet_index].number,
            field: scalar.field(),
        });
    }
    Ok(())
}

fn finish_input_scalar(
    sheet_stubs: &[SheetStub],
    sheet_index: usize,
    builder: &mut SheetInputBuilder,
    scalar: InputScalar,
    text: String,
) -> Result<(), BookXmlError> {
    match scalar {
        InputScalar::Path => builder.path = Some(text),
        InputScalar::Number => {
            let number = text
                .trim()
                .parse::<u32>()
                .ok()
                .filter(|number| *number > 0 && *number <= i32::MAX as u32)
                .ok_or_else(|| BookXmlError::InvalidInputNumber {
                    sheet_number: sheet_stubs[sheet_index].number,
                    value: text.clone(),
                })?;
            builder.number = Some(number);
        }
    }
    Ok(())
}

fn finish_input(
    sheet_stubs: &[SheetStub],
    sheet_index: usize,
    builder: SheetInputBuilder,
) -> Result<SheetInput, BookXmlError> {
    let sheet_number = sheet_stubs[sheet_index].number;
    let path = builder.path.ok_or(BookXmlError::MissingInputField {
        sheet_number,
        field: "sheet/input/path",
    })?;
    let number = builder.number.ok_or(BookXmlError::MissingInputField {
        sheet_number,
        field: "sheet/input/number",
    })?;
    Ok(SheetInput { path, number })
}

fn parse_steps(text: &str, sheet_number: u32) -> Result<Vec<OmrStep>, BookXmlError> {
    let mut steps = Vec::new();
    for token in text.split_whitespace() {
        let step = OmrStep::ALL
            .into_iter()
            .find(|step| step.as_str() == token)
            .ok_or_else(|| BookXmlError::UnknownOmrStep {
                sheet_number,
                token: token.to_owned(),
            })?;
        if steps.contains(&step) {
            return Err(BookXmlError::DuplicateOmrStep { sheet_number, step });
        }
        steps.push(step);
    }
    // JAXB stores this XML list in an EnumSet, whose iteration order is the
    // Java enum's declaration order rather than the source token order.
    steps.sort_unstable();
    Ok(steps)
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
              <av:sheet number="1" version="5&amp;10" invalid="true"/>
              <av:sheet number="7"><unknown/></av:sheet>
            </av:book>"#;

        let book = BookXml::parse(xml).unwrap();

        assert_eq!(book.root_element(), "av:book");
        assert_eq!(book.software_version(), Some("5.11.0"));
        assert_eq!(book.sheet_stubs().len(), 2);
        assert_eq!(book.sheet_stubs()[0].number(), 1);
        assert_eq!(book.sheet_stubs()[0].archive_path(), "sheet#1/sheet#1.xml");
        assert_eq!(book.sheet_stubs()[0].version(), Some("5&10"));
        assert_eq!(book.sheet_stubs()[0].invalid_attribute(), Some(true));
        assert!(book.sheet_stubs()[0].is_invalid());
        assert_eq!(book.sheet_stubs()[1].number(), 7);
        assert_eq!(book.sheet_stubs()[1].archive_path(), "sheet#7/sheet#7.xml");
        assert_eq!(book.sheet_stubs()[1].version(), None);
        assert_eq!(book.sheet_stubs()[1].invalid_attribute(), None);
        assert!(!book.sheet_stubs()[1].is_invalid());
        assert_eq!(book.sheet_stubs()[0].done_steps(), None);
        assert_eq!(book.sheet_stubs()[0].latest_done_step(), None);
    }

    #[test]
    fn reads_all_steps_from_real_baseline_spelling() {
        // Exact JAXB list spelling from the frozen 5.11.0 K.545 baseline.
        let xml = br#"<?xml version="1.0" ?>
<book software-version="5.11.0" future="keep">
  <sheet number="1">
    <input><path>page-001.png</path><number>1</number></input>
    <steps>LOAD BINARY SCALE GRID HEADERS STEM_SEEDS BEAMS LEDGERS HEADS STEMS REDUCTION CUE_BEAMS TEXTS MEASURES CHORDS CURVES SYMBOLS LINKS RHYTHMS PAGE</steps>
    <page id="1"><future/></page>
  </sheet>
</book>"#;
        let book = BookXml::parse(xml).unwrap();
        let steps = book.sheet_stubs()[0].done_steps().unwrap();
        let input = book.sheet_stubs()[0].input().unwrap();

        assert_eq!(input.path(), "page-001.png");
        assert_eq!(input.number(), 1);
        assert_eq!(
            steps,
            &[
                OmrStep::Load,
                OmrStep::Binary,
                OmrStep::Scale,
                OmrStep::Grid,
                OmrStep::Headers,
                OmrStep::StemSeeds,
                OmrStep::Beams,
                OmrStep::Ledgers,
                OmrStep::Heads,
                OmrStep::Stems,
                OmrStep::Reduction,
                OmrStep::CueBeams,
                OmrStep::Texts,
                OmrStep::Measures,
                OmrStep::Chords,
                OmrStep::Curves,
                OmrStep::Symbols,
                OmrStep::Links,
                OmrStep::Rhythms,
                OmrStep::Page,
            ]
        );
        assert_eq!(
            book.sheet_stubs()[0].latest_done_step(),
            Some(OmrStep::Page)
        );
        assert_eq!(book.original_bytes(), xml);
        assert_eq!(
            steps
                .iter()
                .map(|step| step.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            "LOAD BINARY SCALE GRID HEADERS STEM_SEEDS BEAMS LEDGERS HEADS STEMS REDUCTION CUE_BEAMS TEXTS MEASURES CHORDS CURVES SYMBOLS LINKS RHYTHMS PAGE"
        );
    }

    #[test]
    fn distinguishes_absent_and_explicitly_empty_steps() {
        let book = BookXml::parse(
            br#"<book><sheet number="1"/><sheet number="2"><steps/></sheet><sheet number="3"><steps>  </steps></sheet></book>"#,
        )
        .unwrap();

        assert_eq!(book.sheet_stubs()[0].done_steps(), None);
        assert_eq!(book.sheet_stubs()[1].done_steps(), Some(&[][..]));
        assert_eq!(book.sheet_stubs()[2].done_steps(), Some(&[][..]));
    }

    #[test]
    fn canonicalizes_steps_to_java_enum_set_order() {
        let book = BookXml::parse(
            br#"<book><sheet number="1"><steps>PAGE GRID LOAD</steps></sheet></book>"#,
        )
        .unwrap();

        assert_eq!(
            book.sheet_stubs()[0].done_steps(),
            Some(&[OmrStep::Load, OmrStep::Grid, OmrStep::Page][..])
        );
        assert_eq!(
            book.sheet_stubs()[0].latest_done_step(),
            Some(OmrStep::Page)
        );
    }

    #[test]
    fn ignores_steps_outside_a_direct_sheet_stub() {
        let book = BookXml::parse(
            br#"<book><steps>PAGE</steps><sheet number="1"><future><steps>PAGE</steps></future><steps>LOAD GRID</steps></sheet></book>"#,
        )
        .unwrap();

        assert_eq!(
            book.sheet_stubs()[0].done_steps(),
            Some(&[OmrStep::Load, OmrStep::Grid][..])
        );
        assert_eq!(
            book.sheet_stubs()[0].latest_done_step(),
            Some(OmrStep::Grid)
        );
    }

    #[test]
    fn reads_real_input_provenance_and_preserves_path_text() {
        let xml = br#"<?xml version="1.0" ?>
<book software-version="5.11.0">
  <sheet number="7">
    <input future="keep">
      <path>/Users/john/sources/jul10-charter/omr/data/synth/k545-movement1-exposition/page-001.png</path>
      <number>  1  </number>
      <future/>
    </input>
  </sheet>
</book>"#;
        let book = BookXml::parse(xml).unwrap();
        let input = book.sheet_stubs()[0].input().unwrap();

        assert_eq!(
            input.path(),
            "/Users/john/sources/jul10-charter/omr/data/synth/k545-movement1-exposition/page-001.png"
        );
        assert_eq!(input.number(), 1);
        assert_eq!(book.original_bytes(), xml);
    }

    #[test]
    fn reads_real_page_reference_spelling_without_interpreting_children() {
        // Exact page attributes and representative child spelling from the
        // frozen Audiveris 5.11.0 K.545 archive.
        let xml = br#"<?xml version="1.0" ?>
<book software-version="5.11.0">
  <sheet number="1">
    <page id="1" movement-start="true" delta-measure-id="12">
      <last-time-rational num="4" den="4"/>
      <system><part logical-id="1"><staff-configuration line-count="5"/></part></system>
    </page>
  </sheet>
  <score><page sheet-number="1" sheet-page-id="1"/></score>
</book>"#;
        let book = BookXml::parse(xml).unwrap();
        let pages = book.sheet_stubs()[0].page_refs();

        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].id(), 1);
        assert_eq!(pages[0].movement_start_attribute(), Some(true));
        assert!(pages[0].is_movement_start());
        assert_eq!(pages[0].delta_measure_id(), Some(12));
        assert_eq!(book.original_bytes(), xml);
    }

    #[test]
    fn preserves_page_order_optional_states_and_empty_list() {
        let book = BookXml::parse(
            br#"<book><sheet number="1"/><sheet number="2"><page id=" 2 " movement-start="false" delta-measure-id="-3"/><page id="1" movement-start="0"/></sheet></book>"#,
        )
        .unwrap();

        assert!(book.sheet_stubs()[0].page_refs().is_empty());
        let pages = book.sheet_stubs()[1].page_refs();
        assert_eq!(
            pages.iter().map(|page| page.id()).collect::<Vec<_>>(),
            [2, 1]
        );
        assert_eq!(pages[0].movement_start_attribute(), Some(false));
        assert!(!pages[0].is_movement_start());
        assert_eq!(pages[0].delta_measure_id(), Some(-3));
        assert_eq!(pages[1].movement_start_attribute(), Some(false));
        assert_eq!(pages[1].delta_measure_id(), None);
    }

    #[test]
    fn ignores_nested_and_namespaced_page_lookalikes() {
        let book = BookXml::parse(
            br#"<book xmlns:av="urn:audiveris" xmlns:f="urn:future"><av:sheet number="1"><future><av:page id="99"/></future><av:page id="3" f:id="88" f:movement-start="true"><future><av:page id="77"/></future></av:page></av:sheet><score><av:page id="66"/></score></book>"#,
        )
        .unwrap();

        let pages = book.sheet_stubs()[0].page_refs();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].id(), 3);
        assert_eq!(pages[0].movement_start_attribute(), None);
    }

    #[test]
    fn distinguishes_absent_input_and_accepts_explicit_empty_path() {
        let book = BookXml::parse(
            br#"<book><sheet number="1"/><sheet number="2"><input><path/><number>2</number></input></sheet></book>"#,
        )
        .unwrap();

        assert_eq!(book.sheet_stubs()[0].input(), None);
        assert_eq!(book.sheet_stubs()[1].input().unwrap().path(), "");
        assert_eq!(book.sheet_stubs()[1].input().unwrap().number(), 2);
    }

    #[test]
    fn preserves_explicit_false_and_empty_version_attribute_states() {
        let book = BookXml::parse(
            br#"<book xmlns:f="urn:future"><sheet number="1"/><sheet number="2" version="" invalid="false"/><sheet number="3" invalid="0"/><sheet number="4" invalid=" 1 "/><sheet number="5" f:invalid="true"/></book>"#,
        )
        .unwrap();

        assert_eq!(book.sheet_stubs()[0].version(), None);
        assert_eq!(book.sheet_stubs()[0].invalid_attribute(), None);
        assert_eq!(book.sheet_stubs()[1].version(), Some(""));
        assert_eq!(book.sheet_stubs()[1].invalid_attribute(), Some(false));
        assert!(!book.sheet_stubs()[1].is_invalid());
        assert_eq!(book.sheet_stubs()[2].invalid_attribute(), Some(false));
        assert_eq!(book.sheet_stubs()[3].invalid_attribute(), Some(true));
        assert!(book.sheet_stubs()[3].is_invalid());
        assert_eq!(book.sheet_stubs()[4].invalid_attribute(), None);
    }

    #[test]
    fn ignores_input_names_outside_the_direct_jaxb_positions() {
        let book = BookXml::parse(
            br#"<book><input><path>book</path><number>9</number></input><sheet number="1"><future><input><path>nested</path><number>8</number></input></future><input><path>direct</path><number>3</number><future><path>ignored</path><number>99</number></future></input></sheet></book>"#,
        )
        .unwrap();

        let input = book.sheet_stubs()[0].input().unwrap();
        assert_eq!(input.path(), "direct");
        assert_eq!(input.number(), 3);
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
    fn rejects_duplicate_typed_sheet_attributes_as_malformed_xml() {
        for xml in [
            br#"<book><sheet number="1" invalid="true" invalid="false"/></book>"#.as_slice(),
            br#"<book><sheet number="1" version="5.10" version="5.11"/></book>"#.as_slice(),
        ] {
            assert!(matches!(
                BookXml::parse(xml).unwrap_err(),
                BookXmlError::Malformed { .. }
            ));
        }
    }

    #[test]
    fn rejects_non_jaxb_boolean_spellings() {
        for invalid in ["", "TRUE", "False", "yes", "2"] {
            let xml = format!("<book><sheet number=\"1\" invalid=\"{invalid}\"/></book>");
            assert_eq!(
                BookXml::parse(xml).unwrap_err(),
                BookXmlError::InvalidSheetBoolean {
                    field: "sheet/@invalid",
                    value: invalid.to_owned(),
                }
            );
        }
    }

    #[test]
    fn rejects_missing_invalid_and_duplicate_page_ids() {
        assert_eq!(
            BookXml::parse(br#"<book><sheet number="4"><page future-id="1"/></sheet></book>"#)
                .unwrap_err(),
            BookXmlError::MissingPageId(4)
        );

        for invalid in ["0", "-1", "not-a-number", "2147483648"] {
            let xml = format!("<book><sheet number=\"4\"><page id=\"{invalid}\"/></sheet></book>");
            assert_eq!(
                BookXml::parse(xml).unwrap_err(),
                BookXmlError::InvalidPageInteger {
                    sheet_number: 4,
                    field: "sheet/page/@id",
                    value: invalid.to_owned(),
                }
            );
        }

        assert_eq!(
            BookXml::parse(
                br#"<book><sheet number="4"><page id="1"/><page id="1"/></sheet></book>"#
            )
            .unwrap_err(),
            BookXmlError::DuplicatePageId {
                sheet_number: 4,
                page_id: 1,
            }
        );
    }

    #[test]
    fn rejects_invalid_optional_page_attributes() {
        for invalid in ["", "TRUE", "yes", "2"] {
            let xml = format!(
                "<book><sheet number=\"6\"><page id=\"1\" movement-start=\"{invalid}\"/></sheet></book>"
            );
            assert_eq!(
                BookXml::parse(xml).unwrap_err(),
                BookXmlError::InvalidPageBoolean {
                    sheet_number: 6,
                    field: "sheet/page/@movement-start",
                    value: invalid.to_owned(),
                }
            );
        }

        for invalid in ["", "not-a-number", "2147483648", "-2147483649"] {
            let xml = format!(
                "<book><sheet number=\"6\"><page id=\"1\" delta-measure-id=\"{invalid}\"/></sheet></book>"
            );
            assert_eq!(
                BookXml::parse(xml).unwrap_err(),
                BookXmlError::InvalidPageInteger {
                    sheet_number: 6,
                    field: "sheet/page/@delta-measure-id",
                    value: invalid.to_owned(),
                }
            );
        }
    }

    #[test]
    fn rejects_duplicate_page_attributes_as_malformed_xml() {
        let error =
            BookXml::parse(br#"<book><sheet number="1"><page id="1" id="2"/></sheet></book>"#)
                .unwrap_err();
        assert!(matches!(error, BookXmlError::Malformed { .. }));
    }

    #[test]
    fn rejects_duplicate_steps_elements_and_tokens() {
        assert_eq!(
            BookXml::parse(
                br#"<book><sheet number="4"><steps>LOAD</steps><steps>GRID</steps></sheet></book>"#
            )
            .unwrap_err(),
            BookXmlError::DuplicateSheetSteps(4)
        );
        assert_eq!(
            BookXml::parse(
                br#"<book><sheet number="4"><steps>LOAD GRID LOAD</steps></sheet></book>"#
            )
            .unwrap_err(),
            BookXmlError::DuplicateOmrStep {
                sheet_number: 4,
                step: OmrStep::Load,
            }
        );
    }

    #[test]
    fn rejects_duplicate_input_and_direct_scalar_fields() {
        assert_eq!(
            BookXml::parse(
                br#"<book><sheet number="4"><input><path>a</path><number>1</number></input><input><path>b</path><number>2</number></input></sheet></book>"#
            )
            .unwrap_err(),
            BookXmlError::DuplicateSheetInput(4)
        );
        assert_eq!(
            BookXml::parse(
                br#"<book><sheet number="4"><input><path>a</path><path>b</path><number>1</number></input></sheet></book>"#
            )
            .unwrap_err(),
            BookXmlError::DuplicateInputField {
                sheet_number: 4,
                field: "sheet/input/path",
            }
        );
        assert_eq!(
            BookXml::parse(
                br#"<book><sheet number="4"><input><path>a</path><number>1</number><number>2</number></input></sheet></book>"#
            )
            .unwrap_err(),
            BookXmlError::DuplicateInputField {
                sheet_number: 4,
                field: "sheet/input/number",
            }
        );
    }

    #[test]
    fn rejects_partial_input_and_invalid_image_numbers() {
        assert_eq!(
            BookXml::parse(
                br#"<book><sheet number="5"><input><number>1</number></input></sheet></book>"#
            )
            .unwrap_err(),
            BookXmlError::MissingInputField {
                sheet_number: 5,
                field: "sheet/input/path",
            }
        );
        assert_eq!(
            BookXml::parse(
                br#"<book><sheet number="5"><input><path>page.png</path></input></sheet></book>"#
            )
            .unwrap_err(),
            BookXmlError::MissingInputField {
                sheet_number: 5,
                field: "sheet/input/number",
            }
        );

        for invalid in ["", "0", "-1", "not-a-number", "2147483648"] {
            let xml = format!(
                "<book><sheet number=\"5\"><input><path>page.png</path><number>{invalid}</number></input></sheet></book>"
            );
            assert_eq!(
                BookXml::parse(xml).unwrap_err(),
                BookXmlError::InvalidInputNumber {
                    sheet_number: 5,
                    value: invalid.to_owned(),
                }
            );
        }
    }

    #[test]
    fn rejects_markup_and_entities_inside_input_scalars() {
        assert_eq!(
            BookXml::parse(
                br#"<book><sheet number="6"><input><path>page<future/>.png</path><number>1</number></input></sheet></book>"#
            )
            .unwrap_err(),
            BookXmlError::UnexpectedInputScalarContent {
                sheet_number: 6,
                field: "sheet/input/path",
            }
        );
        assert_eq!(
            BookXml::parse(
                br#"<book><sheet number="6"><input><path>page.png</path><number>&#49;</number></input></sheet></book>"#
            )
            .unwrap_err(),
            BookXmlError::UnexpectedInputScalarContent {
                sheet_number: 6,
                field: "sheet/input/number",
            }
        );
    }

    #[test]
    fn rejects_unknown_steps_and_non_text_list_content() {
        assert_eq!(
            BookXml::parse(
                br#"<book><sheet number="8"><steps>LOAD FUTURE_STEP</steps></sheet></book>"#
            )
            .unwrap_err(),
            BookXmlError::UnknownOmrStep {
                sheet_number: 8,
                token: "FUTURE_STEP".to_owned(),
            }
        );
        assert_eq!(
            BookXml::parse(
                br#"<book><sheet number="8"><steps>LOAD<future/>GRID</steps></sheet></book>"#
            )
            .unwrap_err(),
            BookXmlError::UnexpectedStepsContent(8)
        );
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
