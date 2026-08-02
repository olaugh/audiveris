// SPDX-License-Identifier: AGPL-3.0-or-later

//! Conservative, read-only metadata extracted from an Audiveris per-sheet XML document.
//!
//! The musical object graph, including all SIG content, remains opaque. This
//! view retains the complete input byte sequence and exposes only a few stable
//! JAXB fields needed to identify a sheet and its raster/scale geometry.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::io::Cursor;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

const SHEET_ELEMENT: &[u8] = b"sheet";
const PICTURE_ELEMENT: &[u8] = b"picture";
const SCALE_ELEMENT: &[u8] = b"scale";
const PAGE_ELEMENT: &[u8] = b"page";
const LINE_ELEMENT: &[u8] = b"line";
const INTERLINE_ELEMENT: &[u8] = b"interline";
const SMALL_INTERLINE_ELEMENT: &[u8] = b"small-interline";

/// A lossless, read-only view of one `sheet#N/sheet#N.xml` document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SheetXml {
    original: Vec<u8>,
    root_element: String,
    last_persistent_id: Option<u32>,
    picture: Option<PictureDimensions>,
    scale: Option<ScaleMetadata>,
    page_ids: Vec<u32>,
}

impl SheetXml {
    /// Parse narrow metadata while retaining `original` byte-for-byte.
    pub fn parse(original: impl AsRef<[u8]>) -> Result<Self, SheetXmlError> {
        let original = original.as_ref().to_vec();
        let mut reader = Reader::from_reader(Cursor::new(original.as_slice()));
        reader.config_mut().trim_text(false);

        let mut buffer = Vec::new();
        let mut stack: Vec<Vec<u8>> = Vec::new();
        let mut root_element = None;
        let mut last_persistent_id = None;
        let mut picture = None;
        let mut scale = None;
        let mut page_ids = Vec::new();
        let mut seen_page_ids = HashSet::new();
        let mut root_closed = false;

        loop {
            let event = reader.read_event_into(&mut buffer).map_err(|error| {
                SheetXmlError::malformed(reader.error_position(), error.to_string())
            })?;

            match event {
                Event::Start(element) => {
                    if stack.is_empty() {
                        if root_element.is_some() || root_closed {
                            return Err(SheetXmlError::MultipleRootElements);
                        }
                        let (name, persistent_id) = parse_sheet_root(&reader, &element)?;
                        root_element = Some(name);
                        last_persistent_id = persistent_id;
                    } else {
                        inspect_child(
                            &reader,
                            &element,
                            &stack,
                            &mut picture,
                            &mut scale,
                            &mut page_ids,
                            &mut seen_page_ids,
                        )?;
                    }
                    stack.push(element.local_name().as_ref().to_vec());
                }
                Event::Empty(element) => {
                    if stack.is_empty() {
                        if root_element.is_some() || root_closed {
                            return Err(SheetXmlError::MultipleRootElements);
                        }
                        let (name, persistent_id) = parse_sheet_root(&reader, &element)?;
                        root_element = Some(name);
                        last_persistent_id = persistent_id;
                        root_closed = true;
                    } else {
                        inspect_child(
                            &reader,
                            &element,
                            &stack,
                            &mut picture,
                            &mut scale,
                            &mut page_ids,
                            &mut seen_page_ids,
                        )?;
                    }
                }
                Event::End(_) => {
                    stack.pop().ok_or_else(|| {
                        SheetXmlError::malformed(reader.buffer_position(), "unexpected closing tag")
                    })?;
                    if stack.is_empty() {
                        root_closed = true;
                    }
                }
                Event::Text(text) if stack.is_empty() => {
                    let text = text.xml_content().map_err(|error| {
                        SheetXmlError::malformed(reader.error_position(), error.to_string())
                    })?;
                    if !text.trim().is_empty() {
                        return Err(SheetXmlError::ContentOutsideRoot);
                    }
                }
                Event::CData(text) if stack.is_empty() => {
                    let text = text.xml_content().map_err(|error| {
                        SheetXmlError::malformed(reader.error_position(), error.to_string())
                    })?;
                    if !text.trim().is_empty() {
                        return Err(SheetXmlError::ContentOutsideRoot);
                    }
                }
                Event::GeneralRef(_) if stack.is_empty() => {
                    return Err(SheetXmlError::ContentOutsideRoot);
                }
                Event::Eof => break,
                _ => {}
            }
            buffer.clear();
        }

        let root_element = root_element.ok_or(SheetXmlError::MissingRootElement)?;
        if !stack.is_empty() || !root_closed {
            return Err(SheetXmlError::malformed(
                reader.buffer_position(),
                "unclosed root element",
            ));
        }

        Ok(Self {
            original,
            root_element,
            last_persistent_id,
            picture,
            scale,
            page_ids,
        })
    }

    /// Root element qualified spelling, including a namespace prefix when present.
    #[must_use]
    pub fn root_element(&self) -> &str {
        &self.root_element
    }

    /// Last sequential persistent entity ID recorded on the root, when present.
    #[must_use]
    pub const fn last_persistent_id(&self) -> Option<u32> {
        self.last_persistent_id
    }

    /// Source picture dimensions, when the direct `picture` element is present.
    #[must_use]
    pub const fn picture(&self) -> Option<PictureDimensions> {
        self.picture
    }

    /// Staff geometry scale, when the direct `scale` element is present.
    #[must_use]
    pub const fn scale(&self) -> Option<ScaleMetadata> {
        self.scale
    }

    /// IDs of direct `page` children, in document order.
    #[must_use]
    pub fn page_ids(&self) -> &[u32] {
        &self.page_ids
    }

    /// Exact input bytes without normalization or reserialization.
    #[must_use]
    pub fn original_bytes(&self) -> &[u8] {
        &self.original
    }
}

/// Pixel dimensions recorded by the direct `picture` element.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PictureDimensions {
    width: u32,
    height: u32,
}

impl PictureDimensions {
    /// Picture width in pixels.
    #[must_use]
    pub const fn width(self) -> u32 {
        self.width
    }

    /// Picture height in pixels.
    #[must_use]
    pub const fn height(self) -> u32 {
        self.height
    }
}

/// Stable staff-related portions of the direct `scale` element.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ScaleMetadata {
    line: Option<ScaleRange>,
    interline: Option<ScaleRange>,
    small_interline: Option<ScaleRange>,
}

impl ScaleMetadata {
    /// Staff-line thickness range.
    #[must_use]
    pub const fn line(self) -> Option<ScaleRange> {
        self.line
    }

    /// Main staff interline range.
    #[must_use]
    pub const fn interline(self) -> Option<ScaleRange> {
        self.interline
    }

    /// Small-staff interline range, when detected.
    #[must_use]
    pub const fn small_interline(self) -> Option<ScaleRange> {
        self.small_interline
    }
}

/// Minimum, typical, and maximum values emitted by Audiveris scale JAXB types.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScaleRange {
    min: u32,
    main: u32,
    max: u32,
}

impl ScaleRange {
    /// Minimum observed value.
    #[must_use]
    pub const fn min(self) -> u32 {
        self.min
    }

    /// Typical (peak) value.
    #[must_use]
    pub const fn main(self) -> u32 {
        self.main
    }

    /// Maximum observed value.
    #[must_use]
    pub const fn max(self) -> u32 {
        self.max
    }
}

/// Failure to construct the conservative per-sheet XML view.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SheetXmlError {
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
    /// The root local name is not `sheet`.
    UnexpectedRootElement(String),
    /// A required attribute of a recognized typed element is absent.
    MissingField(&'static str),
    /// A recognized typed element or attribute occurs more than once.
    DuplicateField(&'static str),
    /// A recognized integer field is malformed, out of range, or non-positive.
    InvalidField {
        /// Stable field path.
        field: &'static str,
        /// Original attribute value.
        value: String,
    },
    /// Scale range values do not satisfy `min <= main <= max`.
    InvalidScaleRange(&'static str),
    /// Two direct pages declare the same ID.
    DuplicatePageId(u32),
}

impl SheetXmlError {
    fn malformed(position: u64, message: impl Into<String>) -> Self {
        Self::Malformed {
            position,
            message: message.into(),
        }
    }
}

impl fmt::Display for SheetXmlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { position, message } => {
                write!(
                    formatter,
                    "malformed sheet XML near byte {position}: {message}"
                )
            }
            Self::MissingRootElement => write!(formatter, "sheet XML has no root element"),
            Self::MultipleRootElements => write!(formatter, "sheet XML has multiple root elements"),
            Self::ContentOutsideRoot => write!(formatter, "sheet XML has content outside its root"),
            Self::UnexpectedRootElement(name) => {
                write!(formatter, "expected sheet XML root, found {name:?}")
            }
            Self::MissingField(field) => write!(formatter, "missing sheet XML field {field}"),
            Self::DuplicateField(field) => write!(formatter, "duplicate sheet XML field {field}"),
            Self::InvalidField { field, value } => {
                write!(formatter, "invalid sheet XML field {field}: {value:?}")
            }
            Self::InvalidScaleRange(field) => write!(formatter, "invalid scale range {field}"),
            Self::DuplicatePageId(id) => write!(formatter, "duplicate sheet page ID {id}"),
        }
    }
}

impl Error for SheetXmlError {}

fn parse_sheet_root(
    reader: &Reader<Cursor<&[u8]>>,
    element: &BytesStart<'_>,
) -> Result<(String, Option<u32>), SheetXmlError> {
    let qualified_name = decode_name(element.name().as_ref(), reader.buffer_position())?;
    if element.local_name().as_ref() != SHEET_ELEMENT {
        return Err(SheetXmlError::UnexpectedRootElement(qualified_name));
    }
    let last_id = optional_integer_attribute(
        reader,
        element,
        b"last-persistent-id",
        "sheet/@last-persistent-id",
        false,
    )?;
    Ok((qualified_name, last_id))
}

#[allow(clippy::too_many_arguments)]
fn inspect_child(
    reader: &Reader<Cursor<&[u8]>>,
    element: &BytesStart<'_>,
    stack: &[Vec<u8>],
    picture: &mut Option<PictureDimensions>,
    scale: &mut Option<ScaleMetadata>,
    page_ids: &mut Vec<u32>,
    seen_page_ids: &mut HashSet<u32>,
) -> Result<(), SheetXmlError> {
    let depth = stack.len();
    let local_name = element.local_name();

    if depth == 1 {
        match local_name.as_ref() {
            PICTURE_ELEMENT => {
                if picture.is_some() {
                    return Err(SheetXmlError::DuplicateField("sheet/picture"));
                }
                *picture = Some(PictureDimensions {
                    width: required_integer_attribute(
                        reader,
                        element,
                        b"width",
                        "sheet/picture/@width",
                    )?,
                    height: required_integer_attribute(
                        reader,
                        element,
                        b"height",
                        "sheet/picture/@height",
                    )?,
                });
            }
            SCALE_ELEMENT => {
                if scale.is_some() {
                    return Err(SheetXmlError::DuplicateField("sheet/scale"));
                }
                *scale = Some(ScaleMetadata::default());
            }
            PAGE_ELEMENT => {
                let id = required_integer_attribute(reader, element, b"id", "sheet/page/@id")?;
                if !seen_page_ids.insert(id) {
                    return Err(SheetXmlError::DuplicatePageId(id));
                }
                page_ids.push(id);
            }
            _ => {}
        }
    } else if depth == 2 && stack[1].as_slice() == SCALE_ELEMENT {
        let (slot, field) = match local_name.as_ref() {
            LINE_ELEMENT => (&mut scale_mut(scale)?.line, "sheet/scale/line"),
            INTERLINE_ELEMENT => (&mut scale_mut(scale)?.interline, "sheet/scale/interline"),
            SMALL_INTERLINE_ELEMENT => (
                &mut scale_mut(scale)?.small_interline,
                "sheet/scale/small-interline",
            ),
            _ => return Ok(()),
        };
        if slot.is_some() {
            return Err(SheetXmlError::DuplicateField(field));
        }
        *slot = Some(parse_scale_range(reader, element, field)?);
    }

    Ok(())
}

fn scale_mut(scale: &mut Option<ScaleMetadata>) -> Result<&mut ScaleMetadata, SheetXmlError> {
    scale
        .as_mut()
        .ok_or(SheetXmlError::MissingField("sheet/scale"))
}

fn parse_scale_range(
    reader: &Reader<Cursor<&[u8]>>,
    element: &BytesStart<'_>,
    field: &'static str,
) -> Result<ScaleRange, SheetXmlError> {
    let min = required_integer_attribute(reader, element, b"min", range_field(field, "min"))?;
    let main = required_integer_attribute(reader, element, b"main", range_field(field, "main"))?;
    let max = required_integer_attribute(reader, element, b"max", range_field(field, "max"))?;
    if min > main || main > max {
        return Err(SheetXmlError::InvalidScaleRange(field));
    }
    Ok(ScaleRange { min, main, max })
}

fn range_field(field: &'static str, name: &str) -> &'static str {
    match (field, name) {
        ("sheet/scale/line", "min") => "sheet/scale/line/@min",
        ("sheet/scale/line", "main") => "sheet/scale/line/@main",
        ("sheet/scale/line", "max") => "sheet/scale/line/@max",
        ("sheet/scale/interline", "min") => "sheet/scale/interline/@min",
        ("sheet/scale/interline", "main") => "sheet/scale/interline/@main",
        ("sheet/scale/interline", "max") => "sheet/scale/interline/@max",
        ("sheet/scale/small-interline", "min") => "sheet/scale/small-interline/@min",
        ("sheet/scale/small-interline", "main") => "sheet/scale/small-interline/@main",
        ("sheet/scale/small-interline", "max") => "sheet/scale/small-interline/@max",
        _ => unreachable!("known scale range field"),
    }
}

fn required_integer_attribute(
    reader: &Reader<Cursor<&[u8]>>,
    element: &BytesStart<'_>,
    name: &[u8],
    field: &'static str,
) -> Result<u32, SheetXmlError> {
    optional_integer_attribute(reader, element, name, field, true)?
        .ok_or(SheetXmlError::MissingField(field))
}

fn optional_integer_attribute(
    reader: &Reader<Cursor<&[u8]>>,
    element: &BytesStart<'_>,
    name: &[u8],
    field: &'static str,
    positive: bool,
) -> Result<Option<u32>, SheetXmlError> {
    let mut result = None;
    for attribute in element.attributes() {
        let attribute = attribute.map_err(|error| {
            SheetXmlError::malformed(reader.error_position(), error.to_string())
        })?;
        if attribute.key.as_ref() != name {
            continue;
        }
        if result.is_some() {
            return Err(SheetXmlError::DuplicateField(field));
        }
        let value = attribute
            .decode_and_unescape_value(reader.decoder())
            .map_err(|error| SheetXmlError::malformed(reader.error_position(), error.to_string()))?
            .into_owned();
        let parsed = value
            .parse::<u32>()
            .ok()
            .filter(|number| *number <= i32::MAX as u32)
            .filter(|number| !positive || *number > 0)
            .ok_or_else(|| SheetXmlError::InvalidField {
                field,
                value: value.clone(),
            })?;
        result = Some(parsed);
    }
    Ok(result)
}

fn decode_name(name: &[u8], position: u64) -> Result<String, SheetXmlError> {
    std::str::from_utf8(name)
        .map(str::to_owned)
        .map_err(|error| SheetXmlError::malformed(position, error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_real_baseline_structural_spelling() {
        // Exact structural prefix from the frozen K.545 baseline's sheet#1 XML.
        let xml = br#"<?xml version="1.0" ?>
<sheet last-persistent-id="3693">
  <picture width="1700" height="1340"><images><entry><key>BINARY</key></entry></images></picture>
  <scale>
    <line min="1" main="2" max="2"/>
    <interline min="14" main="15" max="15"/>
    <small-interline min="10" main="11" max="12"/>
    <beam main-thickness="8"/>
  </scale>
  <skew slope="0"/>
  <page id="1" measure-count="12"><system id="1"><sig><vertex id="99"/></sig></system></page>
</sheet>"#;

        let sheet = SheetXml::parse(xml).unwrap();
        assert_eq!(sheet.root_element(), "sheet");
        assert_eq!(sheet.last_persistent_id(), Some(3693));
        assert_eq!(sheet.picture().unwrap().width(), 1700);
        assert_eq!(sheet.picture().unwrap().height(), 1340);
        assert_eq!(sheet.scale().unwrap().line().unwrap().main(), 2);
        assert_eq!(sheet.scale().unwrap().interline().unwrap().main(), 15);
        assert_eq!(sheet.scale().unwrap().small_interline().unwrap().main(), 11);
        assert_eq!(sheet.page_ids(), &[1]);
    }

    #[test]
    fn retains_unknown_sig_and_original_bytes_exactly() {
        let xml = b"\xef\xbb\xbf<?xml version=\"1.0\" ?>\r\n<sheet future=\"&amp;\"><sig><alien x=\"1\"/></sig></sheet>\r\n";
        let sheet = SheetXml::parse(xml).unwrap();
        assert_eq!(sheet.original_bytes(), xml);
        assert_eq!(sheet.picture(), None);
        assert_eq!(sheet.scale(), None);
        assert!(sheet.page_ids().is_empty());
    }

    #[test]
    fn accepts_namespace_prefixes_and_ignores_nested_metadata_names() {
        let xml = br#"<av:sheet xmlns:av="urn:audiveris" last-persistent-id="7">
          <av:picture width="20" height="30"/>
          <av:scale><av:line min="1" main="1" max="2"/></av:scale>
          <future><av:page id="99"/><av:picture width="1" height="1"/></future>
          <av:page id="2"/>
        </av:sheet>"#;
        let sheet = SheetXml::parse(xml).unwrap();
        assert_eq!(sheet.root_element(), "av:sheet");
        assert_eq!(sheet.page_ids(), &[2]);
        assert_eq!(sheet.picture().unwrap().width(), 20);
    }

    #[test]
    fn rejects_malformed_multiple_and_wrong_roots() {
        assert!(matches!(
            SheetXml::parse(b"<sheet><page id=\"1\"></sheet>").unwrap_err(),
            SheetXmlError::Malformed { .. }
        ));
        assert_eq!(
            SheetXml::parse(b"<sheet/><sheet/>").unwrap_err(),
            SheetXmlError::MultipleRootElements
        );
        assert_eq!(
            SheetXml::parse(b"<book/>").unwrap_err(),
            SheetXmlError::UnexpectedRootElement("book".to_owned())
        );
    }

    #[test]
    fn rejects_duplicate_typed_elements_and_page_ids() {
        assert_eq!(
            SheetXml::parse(b"<sheet><picture width=\"1\" height=\"1\"/><picture width=\"2\" height=\"2\"/></sheet>").unwrap_err(),
            SheetXmlError::DuplicateField("sheet/picture")
        );
        assert_eq!(
            SheetXml::parse(b"<sheet><page id=\"3\"/><page id=\"3\"/></sheet>").unwrap_err(),
            SheetXmlError::DuplicatePageId(3)
        );
        assert_eq!(
            SheetXml::parse(b"<sheet><scale><line min=\"1\" main=\"1\" max=\"1\"/><line min=\"1\" main=\"1\" max=\"1\"/></scale></sheet>").unwrap_err(),
            SheetXmlError::DuplicateField("sheet/scale/line")
        );
    }

    #[test]
    fn rejects_missing_invalid_and_inconsistent_typed_fields() {
        assert_eq!(
            SheetXml::parse(b"<sheet><picture width=\"1\"/></sheet>").unwrap_err(),
            SheetXmlError::MissingField("sheet/picture/@height")
        );
        assert_eq!(
            SheetXml::parse(b"<sheet><page id=\"0\"/></sheet>").unwrap_err(),
            SheetXmlError::InvalidField {
                field: "sheet/page/@id",
                value: "0".to_owned(),
            }
        );
        assert_eq!(
            SheetXml::parse(b"<sheet last-persistent-id=\"2147483648\"/>").unwrap_err(),
            SheetXmlError::InvalidField {
                field: "sheet/@last-persistent-id",
                value: "2147483648".to_owned(),
            }
        );
        assert_eq!(
            SheetXml::parse(
                b"<sheet><scale><interline min=\"12\" main=\"11\" max=\"13\"/></scale></sheet>"
            )
            .unwrap_err(),
            SheetXmlError::InvalidScaleRange("sheet/scale/interline")
        );
    }
}
