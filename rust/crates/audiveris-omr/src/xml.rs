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
const SCORE_ELEMENT: &[u8] = b"score";
const LOGICAL_PART_ELEMENT: &[u8] = b"logical-part";
const INPUT_ELEMENT: &[u8] = b"input";
const PATH_ELEMENT: &[u8] = b"path";
const INPUT_NUMBER_ELEMENT: &[u8] = b"number";
const STEPS_ELEMENT: &[u8] = b"steps";
const PAGE_ELEMENT: &[u8] = b"page";
const LAST_TIME_RATIONAL_ELEMENT: &[u8] = b"last-time-rational";
const SYSTEM_ELEMENT: &[u8] = b"system";
const PART_ELEMENT: &[u8] = b"part";
const STAFF_CONFIGURATION_ELEMENT: &[u8] = b"staff-configuration";
const DEPRECATED_LINE_COUNT_ELEMENT: &[u8] = b"line-count";
const SOFTWARE_VERSION_ATTRIBUTE: &[u8] = b"software-version";
const NUMBER_ATTRIBUTE: &[u8] = b"number";
const VERSION_ATTRIBUTE: &[u8] = b"version";
const INVALID_ATTRIBUTE: &[u8] = b"invalid";
const ID_ATTRIBUTE: &[u8] = b"id";
const MOVEMENT_START_ATTRIBUTE: &[u8] = b"movement-start";
const DELTA_MEASURE_ID_ATTRIBUTE: &[u8] = b"delta-measure-id";
const NUM_ATTRIBUTE: &[u8] = b"num";
const DEN_ATTRIBUTE: &[u8] = b"den";
const NAME_ATTRIBUTE: &[u8] = b"name";
const STAFF_COUNT_ATTRIBUTE: &[u8] = b"staff-count";
const ABBREVIATION_ATTRIBUTE: &[u8] = b"abbreviation";
const MIDI_PROGRAM_ATTRIBUTE: &[u8] = b"midi-program";
const LOGICAL_ID_ATTRIBUTE: &[u8] = b"logical-id";
const MANUAL_ATTRIBUTE: &[u8] = b"manual";
const LINE_COUNT_ATTRIBUTE: &[u8] = b"line-count";
const SMALL_ATTRIBUTE: &[u8] = b"small";
const LOGICALS_LOCKED_ATTRIBUTE: &[u8] = b"logicals-locked";
const SHEET_NUMBER_ATTRIBUTE: &[u8] = b"sheet-number";
const SHEET_PAGE_ID_ATTRIBUTE: &[u8] = b"sheet-page-id";

/// A lossless, read-only view of an Audiveris `book.xml` document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BookXml {
    original: Vec<u8>,
    root_element: String,
    software_version: Option<String>,
    sheet_stubs: Vec<SheetStub>,
    score_refs: Vec<ScoreRef>,
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
        let mut score_refs = Vec::new();
        let mut sheet_numbers = HashSet::new();
        let mut active_sheet = None;
        let mut active_input: Option<(usize, SheetInputBuilder)> = None;
        let mut active_input_scalar: Option<InputScalarCapture> = None;
        let mut active_steps: Option<(usize, String)> = None;
        let mut active_page = None;
        let mut active_time_rational = None;
        let mut active_system = None;
        let mut active_part = None;
        let mut active_staff_leaf: Option<StaffLeafCapture> = None;
        let mut active_score = None;
        let mut active_logical_part = None;
        let mut active_logical_staff_leaf: Option<LogicalStaffLeafCapture> = None;
        let mut active_score_page: Option<(u32, ScorePageRef)> = None;
        let mut root_closed = false;

        loop {
            let event = reader.read_event_into(&mut buffer).map_err(|error| {
                BookXmlError::malformed(reader.error_position(), error.to_string())
            })?;

            match event {
                Event::Start(element) => {
                    if let Some((score_index, page)) = active_score_page {
                        return Err(BookXmlError::UnexpectedScorePageContent {
                            score_index,
                            sheet_number: page.sheet_number,
                            sheet_page_id: page.sheet_page_id,
                        });
                    }
                    if let Some(capture) = active_logical_staff_leaf.as_ref() {
                        return Err(unexpected_logical_staff_config_content(capture));
                    }
                    if let Some(capture) = active_staff_leaf.as_ref() {
                        return Err(unexpected_staff_config_content(&sheet_stubs, capture));
                    }
                    if let Some((sheet_index, page_index)) = active_time_rational {
                        return Err(unexpected_time_rational_content(
                            &sheet_stubs,
                            sheet_index,
                            page_index,
                        ));
                    }
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
                    } else if depth == 1 && element.name().as_ref() == SCORE_ELEMENT {
                        let score_index = push_score_ref(&reader, &element, &mut score_refs)?;
                        active_score = Some(score_index);
                    } else if depth == 2
                        && element.name().as_ref() == LOGICAL_PART_ELEMENT
                        && let Some(score_index) = active_score
                    {
                        let logical_index =
                            push_logical_part(&reader, &element, &mut score_refs[score_index])?;
                        active_logical_part = Some((score_index, logical_index));
                    } else if depth == 3
                        && let Some((score_index, logical_index)) = active_logical_part
                        && let Some(kind) = staff_leaf_kind(element.name().as_ref())
                    {
                        active_logical_staff_leaf = Some(begin_logical_staff_leaf(
                            &reader,
                            &element,
                            &mut score_refs[score_index],
                            logical_index,
                            kind,
                        )?);
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
                        let page_index =
                            push_page_ref(&reader, &element, &mut sheet_stubs[sheet_index])?;
                        active_page = Some((sheet_index, page_index));
                    } else if depth == 3
                        && element.name().as_ref() == LAST_TIME_RATIONAL_ELEMENT
                        && let Some((sheet_index, page_index)) = active_page
                    {
                        push_last_time_rational(
                            &reader,
                            &element,
                            &mut sheet_stubs[sheet_index],
                            page_index,
                        )?;
                        active_time_rational = Some((sheet_index, page_index));
                    } else if depth == 3
                        && element.name().as_ref() == SYSTEM_ELEMENT
                        && let Some((sheet_index, page_index)) = active_page
                    {
                        let system_index =
                            push_system_ref(&mut sheet_stubs[sheet_index], page_index)?;
                        active_system = Some((sheet_index, page_index, system_index));
                    } else if depth == 4
                        && element.name().as_ref() == PART_ELEMENT
                        && let Some((sheet_index, page_index, system_index)) = active_system
                    {
                        let part_index = push_part_ref(
                            &reader,
                            &element,
                            &mut sheet_stubs[sheet_index],
                            page_index,
                            system_index,
                        )?;
                        active_part = Some((sheet_index, page_index, system_index, part_index));
                    } else if depth == 5
                        && let Some((sheet_index, page_index, system_index, part_index)) =
                            active_part
                        && let Some(kind) = staff_leaf_kind(element.name().as_ref())
                    {
                        active_staff_leaf = Some(begin_staff_leaf(
                            &reader,
                            &element,
                            sheet_index,
                            &mut sheet_stubs[sheet_index],
                            page_index,
                            system_index,
                            part_index,
                            kind,
                        )?);
                    } else if depth == 2
                        && element.name().as_ref() == PAGE_ELEMENT
                        && let Some(score_index) = active_score
                    {
                        push_score_page(&reader, &element, &mut score_refs[score_index])?;
                        active_score_page = Some((
                            score_refs[score_index].index,
                            *score_refs[score_index].pages.last().unwrap(),
                        ));
                    }
                    depth = depth.checked_add(1).ok_or_else(|| {
                        BookXmlError::malformed(reader.buffer_position(), "XML nesting overflow")
                    })?;
                }
                Event::Empty(element) => {
                    if let Some((score_index, page)) = active_score_page {
                        return Err(BookXmlError::UnexpectedScorePageContent {
                            score_index,
                            sheet_number: page.sheet_number,
                            sheet_page_id: page.sheet_page_id,
                        });
                    }
                    if let Some(capture) = active_logical_staff_leaf.as_ref() {
                        return Err(unexpected_logical_staff_config_content(capture));
                    }
                    if let Some(capture) = active_staff_leaf.as_ref() {
                        return Err(unexpected_staff_config_content(&sheet_stubs, capture));
                    }
                    if let Some((sheet_index, page_index)) = active_time_rational {
                        return Err(unexpected_time_rational_content(
                            &sheet_stubs,
                            sheet_index,
                            page_index,
                        ));
                    }
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
                    } else if depth == 1 && element.name().as_ref() == SCORE_ELEMENT {
                        push_score_ref(&reader, &element, &mut score_refs)?;
                    } else if depth == 2
                        && element.name().as_ref() == LOGICAL_PART_ELEMENT
                        && let Some(score_index) = active_score
                    {
                        push_logical_part(&reader, &element, &mut score_refs[score_index])?;
                    } else if depth == 3
                        && let Some((score_index, logical_index)) = active_logical_part
                        && let Some(kind) = staff_leaf_kind(element.name().as_ref())
                    {
                        finish_empty_logical_staff_leaf(
                            &reader,
                            &element,
                            &mut score_refs[score_index],
                            logical_index,
                            kind,
                        )?;
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
                    } else if depth == 3
                        && element.name().as_ref() == LAST_TIME_RATIONAL_ELEMENT
                        && let Some((sheet_index, page_index)) = active_page
                    {
                        push_last_time_rational(
                            &reader,
                            &element,
                            &mut sheet_stubs[sheet_index],
                            page_index,
                        )?;
                    } else if depth == 3
                        && element.name().as_ref() == SYSTEM_ELEMENT
                        && let Some((sheet_index, page_index)) = active_page
                    {
                        push_system_ref(&mut sheet_stubs[sheet_index], page_index)?;
                    } else if depth == 4
                        && element.name().as_ref() == PART_ELEMENT
                        && let Some((sheet_index, page_index, system_index)) = active_system
                    {
                        push_part_ref(
                            &reader,
                            &element,
                            &mut sheet_stubs[sheet_index],
                            page_index,
                            system_index,
                        )?;
                    } else if depth == 5
                        && let Some((sheet_index, page_index, system_index, part_index)) =
                            active_part
                        && let Some(kind) = staff_leaf_kind(element.name().as_ref())
                    {
                        finish_empty_staff_leaf(
                            &reader,
                            &element,
                            &mut sheet_stubs[sheet_index],
                            page_index,
                            system_index,
                            part_index,
                            kind,
                        )?;
                    } else if depth == 2
                        && element.name().as_ref() == PAGE_ELEMENT
                        && let Some(score_index) = active_score
                    {
                        push_score_page(&reader, &element, &mut score_refs[score_index])?;
                    }
                }
                Event::End(element) => {
                    if element.name().as_ref() == PAGE_ELEMENT && depth == 3 {
                        active_score_page = None;
                    }
                    if depth == 6
                        && let Some(capture) = active_staff_leaf.take()
                    {
                        finish_staff_leaf(&mut sheet_stubs, capture)?;
                    }
                    if depth == 4
                        && let Some(capture) = active_logical_staff_leaf.take()
                    {
                        finish_logical_staff_leaf(&mut score_refs, capture)?;
                    }
                    if element.name().as_ref() == LAST_TIME_RATIONAL_ELEMENT && depth == 4 {
                        active_time_rational = None;
                    }
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
                    if depth == 2 && active_score.is_some() {
                        active_score = None;
                    }
                    if depth == 3 && active_logical_part.is_some() {
                        active_logical_part = None;
                    }
                    if depth == 3 && active_page.is_some() {
                        active_page = None;
                    }
                    if depth == 4 && active_system.is_some() {
                        active_system = None;
                    }
                    if depth == 5 && active_part.is_some() {
                        active_part = None;
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
                Event::Text(text) if active_score_page.is_some() => {
                    let decoded = text.xml_content().map_err(|error| {
                        BookXmlError::malformed(reader.error_position(), error.to_string())
                    })?;
                    if !decoded.trim().is_empty() {
                        let (score_index, page) = active_score_page.unwrap();
                        return Err(BookXmlError::UnexpectedScorePageContent {
                            score_index,
                            sheet_number: page.sheet_number,
                            sheet_page_id: page.sheet_page_id,
                        });
                    }
                }
                Event::Text(text) if active_logical_staff_leaf.is_some() => {
                    let decoded = text.xml_content().map_err(|error| {
                        BookXmlError::malformed(reader.error_position(), error.to_string())
                    })?;
                    let capture = active_logical_staff_leaf.as_mut().unwrap();
                    if capture.kind == StaffLeafKind::Current && !decoded.trim().is_empty() {
                        return Err(unexpected_logical_staff_config_content(capture));
                    }
                    capture.text.push_str(&decoded);
                }
                Event::Text(text) if active_staff_leaf.is_some() => {
                    let decoded = text.xml_content().map_err(|error| {
                        BookXmlError::malformed(reader.error_position(), error.to_string())
                    })?;
                    let capture = active_staff_leaf.as_mut().unwrap();
                    if capture.kind == StaffLeafKind::Current && !decoded.trim().is_empty() {
                        return Err(unexpected_staff_config_content(&sheet_stubs, capture));
                    }
                    capture.text.push_str(&decoded);
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
                Event::Text(text) if active_time_rational.is_some() => {
                    let decoded = text.xml_content().map_err(|error| {
                        BookXmlError::malformed(reader.error_position(), error.to_string())
                    })?;
                    if !decoded.trim().is_empty() {
                        let (sheet_index, page_index) = active_time_rational.unwrap();
                        return Err(unexpected_time_rational_content(
                            &sheet_stubs,
                            sheet_index,
                            page_index,
                        ));
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
                Event::CData(_) if active_score_page.is_some() => {
                    let (score_index, page) = active_score_page.unwrap();
                    return Err(BookXmlError::UnexpectedScorePageContent {
                        score_index,
                        sheet_number: page.sheet_number,
                        sheet_page_id: page.sheet_page_id,
                    });
                }
                Event::CData(_) if active_logical_staff_leaf.is_some() => {
                    return Err(unexpected_logical_staff_config_content(
                        active_logical_staff_leaf.as_ref().unwrap(),
                    ));
                }
                Event::CData(_) if active_staff_leaf.is_some() => {
                    return Err(unexpected_staff_config_content(
                        &sheet_stubs,
                        active_staff_leaf.as_ref().unwrap(),
                    ));
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
                Event::CData(_) if active_time_rational.is_some() => {
                    let (sheet_index, page_index) = active_time_rational.unwrap();
                    return Err(unexpected_time_rational_content(
                        &sheet_stubs,
                        sheet_index,
                        page_index,
                    ));
                }
                Event::GeneralRef(_) if depth == 0 => {
                    return Err(BookXmlError::ContentOutsideRoot);
                }
                Event::GeneralRef(_) if active_score_page.is_some() => {
                    let (score_index, page) = active_score_page.unwrap();
                    return Err(BookXmlError::UnexpectedScorePageContent {
                        score_index,
                        sheet_number: page.sheet_number,
                        sheet_page_id: page.sheet_page_id,
                    });
                }
                Event::GeneralRef(_) if active_logical_staff_leaf.is_some() => {
                    return Err(unexpected_logical_staff_config_content(
                        active_logical_staff_leaf.as_ref().unwrap(),
                    ));
                }
                Event::GeneralRef(_) if active_staff_leaf.is_some() => {
                    return Err(unexpected_staff_config_content(
                        &sheet_stubs,
                        active_staff_leaf.as_ref().unwrap(),
                    ));
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
                Event::GeneralRef(_) if active_time_rational.is_some() => {
                    let (sheet_index, page_index) = active_time_rational.unwrap();
                    return Err(unexpected_time_rational_content(
                        &sheet_stubs,
                        sheet_index,
                        page_index,
                    ));
                }
                Event::Comment(_) | Event::PI(_) | Event::DocType(_)
                    if active_score_page.is_some() =>
                {
                    let (score_index, page) = active_score_page.unwrap();
                    return Err(BookXmlError::UnexpectedScorePageContent {
                        score_index,
                        sheet_number: page.sheet_number,
                        sheet_page_id: page.sheet_page_id,
                    });
                }
                Event::Comment(_) | Event::PI(_) | Event::DocType(_)
                    if active_logical_staff_leaf.is_some() =>
                {
                    return Err(unexpected_logical_staff_config_content(
                        active_logical_staff_leaf.as_ref().unwrap(),
                    ));
                }
                Event::Comment(_) | Event::PI(_) | Event::DocType(_)
                    if active_staff_leaf.is_some() =>
                {
                    return Err(unexpected_staff_config_content(
                        &sheet_stubs,
                        active_staff_leaf.as_ref().unwrap(),
                    ));
                }
                Event::Comment(_) | Event::PI(_) | Event::DocType(_)
                    if active_time_rational.is_some() =>
                {
                    let (sheet_index, page_index) = active_time_rational.unwrap();
                    return Err(unexpected_time_rational_content(
                        &sheet_stubs,
                        sheet_index,
                        page_index,
                    ));
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
            score_refs,
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

    /// Direct book scores (movements) in persisted document order.
    #[must_use]
    pub fn score_refs(&self) -> &[ScoreRef] {
        &self.score_refs
    }

    /// The exact input bytes, without XML normalization or reserialization.
    #[must_use]
    pub fn original_bytes(&self) -> &[u8] {
        &self.original
    }
}

/// Lightweight book-level movement metadata from one direct `score` child.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoreRef {
    index: u32,
    logicals_locked: Option<bool>,
    logical_parts: Vec<LogicalPartRef>,
    pages: Vec<ScorePageRef>,
}

impl ScoreRef {
    /// Zero-based position in Java Book's persisted score list.
    #[must_use]
    pub const fn index(&self) -> u32 {
        self.index
    }

    /// Explicit state of the optional JAXB boolean-positive lock attribute.
    #[must_use]
    pub const fn logicals_locked_attribute(&self) -> Option<bool> {
        self.logicals_locked
    }

    /// Effective Java lock state; absent defaults to false.
    #[must_use]
    pub fn logicals_locked(&self) -> bool {
        self.logicals_locked.unwrap_or(false)
    }

    /// Direct logical parts in persisted document order.
    #[must_use]
    pub fn logical_parts(&self) -> &[LogicalPartRef] {
        &self.logical_parts
    }

    /// Soft page references in persisted document order.
    #[must_use]
    pub fn pages(&self) -> &[ScorePageRef] {
        &self.pages
    }
}

/// Persisted scalar metadata for one score-level logical part.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogicalPartRef {
    index: u32,
    id: u32,
    staff_count: u32,
    name: Option<String>,
    abbreviation: Option<String>,
    midi_program: Option<i32>,
    staff_configs: Vec<PersistedStaffConfig>,
}

impl LogicalPartRef {
    /// Zero-based position, matching Java `LogicalPart.getIndex(score)`.
    #[must_use]
    pub const fn index(&self) -> u32 {
        self.index
    }

    /// Positive logical-part ID persisted by Java.
    #[must_use]
    pub const fn id(&self) -> u32 {
        self.id
    }

    /// Deprecated but still persisted positive number of staves.
    #[must_use]
    pub const fn staff_count(&self) -> u32 {
        self.staff_count
    }

    /// Optional decoded part name; explicit empty remains distinct from absent.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Optional decoded abbreviation; explicit empty remains distinct from absent.
    #[must_use]
    pub fn abbreviation(&self) -> Option<&str> {
        self.abbreviation.as_deref()
    }

    /// Optional raw Java `int` MIDI program.
    #[must_use]
    pub const fn midi_program(&self) -> Option<i32> {
        self.midi_program
    }

    /// Direct current and deprecated staff-config spellings in document order.
    #[must_use]
    pub fn staff_configs(&self) -> &[PersistedStaffConfig] {
        &self.staff_configs
    }
}

/// Stable sheet/page coordinates persisted by Java `PageNumber`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ScorePageRef {
    sheet_number: u32,
    sheet_page_id: u32,
}

impl ScorePageRef {
    /// One-based containing-sheet rank in the book.
    #[must_use]
    pub const fn sheet_number(self) -> u32 {
        self.sheet_number
    }

    /// One-based page rank in the containing sheet.
    #[must_use]
    pub const fn sheet_page_id(self) -> u32 {
        self.sheet_page_id
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

/// Lightweight metadata from one direct sheet-stub `page` reference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageRef {
    id: u32,
    movement_start: Option<bool>,
    delta_measure_id: Option<i32>,
    last_time_rational: Option<TimeRational>,
    system_refs: Vec<SystemRef>,
}

impl PageRef {
    /// One-based page rank within the containing sheet.
    #[must_use]
    pub const fn id(&self) -> u32 {
        self.id
    }

    /// Explicit state of the optional JAXB boolean-positive attribute.
    #[must_use]
    pub const fn movement_start_attribute(&self) -> Option<bool> {
        self.movement_start
    }

    /// Effective Java movement-start state; absent defaults to false.
    #[must_use]
    pub fn is_movement_start(&self) -> bool {
        self.movement_start.unwrap_or(false)
    }

    /// Optional measure-ID increment recorded for the page.
    #[must_use]
    pub const fn delta_measure_id(&self) -> Option<i32> {
        self.delta_measure_id
    }

    /// Last effective time signature in this page, when persisted.
    #[must_use]
    pub fn last_time_rational(&self) -> Option<TimeRational> {
        self.last_time_rational
    }

    /// Direct system references in persisted document order.
    ///
    /// Java stores no system ID; [`SystemRef::id`] is the same one-based list
    /// position returned by Java `SystemRef.getId()`.
    #[must_use]
    pub fn system_refs(&self) -> &[SystemRef] {
        &self.system_refs
    }
}

/// Order-only view of one direct PageRef `system` child.
///
/// Java persists no scalar SystemRef fields. Only a narrow PartRef scalar view
/// is exposed; the ID is derived solely from the element's list position.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemRef {
    id: u32,
    part_refs: Vec<PartRef>,
}

impl SystemRef {
    /// One-based position, matching Java `SystemRef.getId()`.
    #[must_use]
    pub const fn id(&self) -> u32 {
        self.id
    }

    /// Direct part references in persisted document order.
    #[must_use]
    pub fn part_refs(&self) -> &[PartRef] {
        &self.part_refs
    }
}

/// Persisted scalar metadata for one direct SystemRef `part` child.
///
/// Java exposes only a derived zero-based list index, not a part ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartRef {
    index: u32,
    name: Option<String>,
    logical_id: Option<i32>,
    manual: Option<bool>,
    staff_configs: Vec<PersistedStaffConfig>,
}

impl PartRef {
    /// Zero-based position, matching Java `PartRef.getIndex()`.
    #[must_use]
    pub const fn index(&self) -> u32 {
        self.index
    }

    /// Optional decoded part name; explicit empty remains distinct from absent.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Optional manually assigned logical part ID.
    #[must_use]
    pub const fn logical_id(&self) -> Option<i32> {
        self.logical_id
    }

    /// Explicit state of the optional JAXB boolean-positive `manual` attribute.
    #[must_use]
    pub const fn manual_attribute(&self) -> Option<bool> {
        self.manual
    }

    /// Effective Java manual-mapping state; absent defaults to false.
    #[must_use]
    pub fn is_manual(&self) -> bool {
        self.manual.unwrap_or(false)
    }

    /// Direct current and deprecated staff-config spellings in document order.
    #[must_use]
    pub fn staff_configs(&self) -> &[PersistedStaffConfig] {
        &self.staff_configs
    }
}

/// One persisted staff configuration without normalizing legacy spelling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistedStaffConfig {
    /// Current attribute-based `<staff-configuration>` JAXB object.
    Current(StaffConfig),
    /// Deprecated scalar `<line-count>` entry migrated by Java after unmarshal.
    DeprecatedLineCount(i32),
}

/// Current `StaffConfig` scalar fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaffConfig {
    line_count: i32,
    small: Option<bool>,
}

impl StaffConfig {
    /// Raw Java `int` line count persisted in the required attribute.
    #[must_use]
    pub const fn line_count(self) -> i32 {
        self.line_count
    }

    /// Explicit state of the optional JAXB boolean-positive `small` attribute.
    #[must_use]
    pub const fn small_attribute(self) -> Option<bool> {
        self.small
    }

    /// Effective Java small-staff state; absent defaults to false.
    #[must_use]
    pub fn is_small(self) -> bool {
        self.small.unwrap_or(false)
    }
}

/// Non-reduced numerator and denominator persisted by Java `TimeRational`.
///
/// The JAXB object uses raw Java `int` fields and does not normalize, impose
/// positivity, or reject a zero denominator while unmarshalling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimeRational {
    numerator: i32,
    denominator: i32,
}

impl TimeRational {
    #[must_use]
    pub const fn numerator(self) -> i32 {
        self.numerator
    }

    #[must_use]
    pub const fn denominator(self) -> i32 {
        self.denominator
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StaffLeafKind {
    Current,
    DeprecatedLineCount,
}

#[derive(Debug)]
struct StaffLeafCapture {
    sheet_index: usize,
    page_index: usize,
    system_index: usize,
    part_index: usize,
    kind: StaffLeafKind,
    text: String,
}

#[derive(Debug)]
struct LogicalStaffLeafCapture {
    score_index: usize,
    logical_index: usize,
    logical_source_index: u32,
    kind: StaffLeafKind,
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
    /// The book contains too many scores to expose a Java `int` list index.
    TooManyScores,
    /// A score lock flag has an invalid XML Schema boolean spelling.
    InvalidScoreBoolean {
        score_index: u32,
        field: &'static str,
        value: String,
    },
    /// A score contains too many logical parts to expose a Java `int` index.
    TooManyLogicalParts { score_index: u32 },
    /// A direct logical part lacks one of its required scalar attributes.
    MissingLogicalPartField {
        score_index: u32,
        logical_index: u32,
        field: &'static str,
    },
    /// A logical-part integer is malformed or outside its supported Java range.
    InvalidLogicalPartInteger {
        score_index: u32,
        logical_index: u32,
        field: &'static str,
        value: String,
    },
    /// Two direct logical parts in one score declare the same stable ID.
    DuplicateLogicalPartId { score_index: u32, id: u32 },
    /// A current logical-part staff configuration lacks a required attribute.
    MissingLogicalStaffConfigField {
        score_index: u32,
        logical_index: u32,
        field: &'static str,
    },
    /// A logical-part staff integer is malformed or outside Java `int` range.
    InvalidLogicalStaffConfigInteger {
        score_index: u32,
        logical_index: u32,
        field: &'static str,
        value: String,
    },
    /// A logical-part staff boolean has an invalid XML Schema spelling.
    InvalidLogicalStaffConfigBoolean {
        score_index: u32,
        logical_index: u32,
        field: &'static str,
        value: String,
    },
    /// A typed logical-part staff configuration contains unsupported content.
    UnexpectedLogicalStaffConfigContent {
        score_index: u32,
        logical_index: u32,
    },
    /// A score page link lacks one of its required coordinate attributes.
    MissingScorePageField {
        score_index: u32,
        field: &'static str,
    },
    /// A score page coordinate is malformed or outside positive Java `int` range.
    InvalidScorePageInteger {
        score_index: u32,
        field: &'static str,
        value: String,
    },
    /// A score repeats the same physical sheet/page coordinate.
    DuplicateScorePage {
        score_index: u32,
        sheet_number: u32,
        sheet_page_id: u32,
    },
    /// An attribute-only score page link contains nested or scalar content.
    UnexpectedScorePageContent {
        score_index: u32,
        sheet_number: u32,
        sheet_page_id: u32,
    },
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
    /// A page contains more than one direct `last-time-rational` element.
    DuplicateLastTimeRational { sheet_number: u32, page_id: u32 },
    /// A present time rational lacks one of its required integer attributes.
    MissingTimeRationalField {
        sheet_number: u32,
        page_id: u32,
        field: &'static str,
    },
    /// A time-rational attribute is malformed or outside Java `int` range.
    InvalidTimeRationalInteger {
        sheet_number: u32,
        page_id: u32,
        field: &'static str,
        value: String,
    },
    /// An attribute-only time rational contains nested or scalar content.
    UnexpectedTimeRationalContent { sheet_number: u32, page_id: u32 },
    /// A page contains too many systems to represent Java's positive `int` ID.
    TooManySystems { sheet_number: u32, page_id: u32 },
    /// A system contains too many parts to represent Java's `int` index.
    TooManyParts {
        sheet_number: u32,
        page_id: u32,
        system_id: u32,
    },
    /// A persisted PartRef integer is malformed or outside Java `int` range.
    InvalidPartInteger {
        sheet_number: u32,
        page_id: u32,
        system_id: u32,
        field: &'static str,
        value: String,
    },
    /// A persisted PartRef boolean has an invalid XML Schema spelling.
    InvalidPartBoolean {
        sheet_number: u32,
        page_id: u32,
        system_id: u32,
        field: &'static str,
        value: String,
    },
    /// A current staff configuration lacks its required line-count attribute.
    MissingStaffConfigField {
        sheet_number: u32,
        page_id: u32,
        system_id: u32,
        part_index: u32,
        field: &'static str,
    },
    /// A staff-configuration integer is malformed or outside Java `int` range.
    InvalidStaffConfigInteger {
        sheet_number: u32,
        page_id: u32,
        system_id: u32,
        part_index: u32,
        field: &'static str,
        value: String,
    },
    /// A current staff-configuration boolean has an invalid XML Schema spelling.
    InvalidStaffConfigBoolean {
        sheet_number: u32,
        page_id: u32,
        system_id: u32,
        part_index: u32,
        field: &'static str,
        value: String,
    },
    /// A typed staff configuration contains nested, entity, or scalar markup.
    UnexpectedStaffConfigContent {
        sheet_number: u32,
        page_id: u32,
        system_id: u32,
        part_index: u32,
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
            Self::TooManyScores => write!(formatter, "book has too many score references"),
            Self::InvalidScoreBoolean {
                score_index,
                field,
                value,
            } => write!(
                formatter,
                "score index {score_index} has invalid boolean {field}: {value:?}"
            ),
            Self::TooManyLogicalParts { score_index } => {
                write!(
                    formatter,
                    "score index {score_index} has too many logical parts"
                )
            }
            Self::MissingLogicalPartField {
                score_index,
                logical_index,
                field,
            } => write!(
                formatter,
                "score index {score_index} logical part index {logical_index} is missing {field}"
            ),
            Self::InvalidLogicalPartInteger {
                score_index,
                logical_index,
                field,
                value,
            } => write!(
                formatter,
                "score index {score_index} logical part index {logical_index} has invalid integer {field}: {value:?}"
            ),
            Self::DuplicateLogicalPartId { score_index, id } => write!(
                formatter,
                "score index {score_index} has duplicate logical part ID {id}"
            ),
            Self::MissingLogicalStaffConfigField {
                score_index,
                logical_index,
                field,
            } => write!(
                formatter,
                "score index {score_index} logical part index {logical_index} staff config is missing {field}"
            ),
            Self::InvalidLogicalStaffConfigInteger {
                score_index,
                logical_index,
                field,
                value,
            } => write!(
                formatter,
                "score index {score_index} logical part index {logical_index} has invalid staff integer {field}: {value:?}"
            ),
            Self::InvalidLogicalStaffConfigBoolean {
                score_index,
                logical_index,
                field,
                value,
            } => write!(
                formatter,
                "score index {score_index} logical part index {logical_index} has invalid staff boolean {field}: {value:?}"
            ),
            Self::UnexpectedLogicalStaffConfigContent {
                score_index,
                logical_index,
            } => write!(
                formatter,
                "score index {score_index} logical part index {logical_index} staff config contains content"
            ),
            Self::MissingScorePageField { score_index, field } => {
                write!(
                    formatter,
                    "score index {score_index} page is missing {field}"
                )
            }
            Self::InvalidScorePageInteger {
                score_index,
                field,
                value,
            } => write!(
                formatter,
                "score index {score_index} has invalid page integer {field}: {value:?}"
            ),
            Self::DuplicateScorePage {
                score_index,
                sheet_number,
                sheet_page_id,
            } => write!(
                formatter,
                "score index {score_index} repeats sheet {sheet_number} page {sheet_page_id}"
            ),
            Self::UnexpectedScorePageContent {
                score_index,
                sheet_number,
                sheet_page_id,
            } => write!(
                formatter,
                "score index {score_index} sheet {sheet_number} page {sheet_page_id} link contains content"
            ),
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
            Self::DuplicateLastTimeRational {
                sheet_number,
                page_id,
            } => write!(
                formatter,
                "sheet stub {sheet_number} page {page_id} repeats last-time-rational"
            ),
            Self::MissingTimeRationalField {
                sheet_number,
                page_id,
                field,
            } => write!(
                formatter,
                "sheet stub {sheet_number} page {page_id} time rational is missing {field}"
            ),
            Self::InvalidTimeRationalInteger {
                sheet_number,
                page_id,
                field,
                value,
            } => write!(
                formatter,
                "sheet stub {sheet_number} page {page_id} has invalid time-rational integer {field}: {value:?}"
            ),
            Self::UnexpectedTimeRationalContent {
                sheet_number,
                page_id,
            } => write!(
                formatter,
                "sheet stub {sheet_number} page {page_id} time rational contains content"
            ),
            Self::TooManySystems {
                sheet_number,
                page_id,
            } => write!(
                formatter,
                "sheet stub {sheet_number} page {page_id} has too many system references"
            ),
            Self::TooManyParts {
                sheet_number,
                page_id,
                system_id,
            } => write!(
                formatter,
                "sheet stub {sheet_number} page {page_id} system {system_id} has too many part references"
            ),
            Self::InvalidPartInteger {
                sheet_number,
                page_id,
                system_id,
                field,
                value,
            } => write!(
                formatter,
                "sheet stub {sheet_number} page {page_id} system {system_id} has invalid part integer {field}: {value:?}"
            ),
            Self::InvalidPartBoolean {
                sheet_number,
                page_id,
                system_id,
                field,
                value,
            } => write!(
                formatter,
                "sheet stub {sheet_number} page {page_id} system {system_id} has invalid part boolean {field}: {value:?}"
            ),
            Self::MissingStaffConfigField {
                sheet_number,
                page_id,
                system_id,
                part_index,
                field,
            } => write!(
                formatter,
                "sheet stub {sheet_number} page {page_id} system {system_id} part index {part_index} staff config is missing {field}"
            ),
            Self::InvalidStaffConfigInteger {
                sheet_number,
                page_id,
                system_id,
                part_index,
                field,
                value,
            } => write!(
                formatter,
                "sheet stub {sheet_number} page {page_id} system {system_id} part index {part_index} has invalid staff integer {field}: {value:?}"
            ),
            Self::InvalidStaffConfigBoolean {
                sheet_number,
                page_id,
                system_id,
                part_index,
                field,
                value,
            } => write!(
                formatter,
                "sheet stub {sheet_number} page {page_id} system {system_id} part index {part_index} has invalid staff boolean {field}: {value:?}"
            ),
            Self::UnexpectedStaffConfigContent {
                sheet_number,
                page_id,
                system_id,
                part_index,
            } => write!(
                formatter,
                "sheet stub {sheet_number} page {page_id} system {system_id} part index {part_index} staff config contains content"
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

fn push_score_ref(
    reader: &Reader<Cursor<&[u8]>>,
    element: &BytesStart<'_>,
    score_refs: &mut Vec<ScoreRef>,
) -> Result<usize, BookXmlError> {
    let index = u32::try_from(score_refs.len())
        .ok()
        .filter(|index| *index <= i32::MAX as u32)
        .ok_or(BookXmlError::TooManyScores)?;
    let mut logicals_locked = None;
    for attribute in element.attributes() {
        let attribute = attribute
            .map_err(|error| BookXmlError::malformed(reader.error_position(), error.to_string()))?;
        if attribute.key.as_ref() == LOGICALS_LOCKED_ATTRIBUTE {
            let value = decode_attribute(reader, &attribute)?;
            logicals_locked = Some(parse_jaxb_boolean(&value).ok_or_else(|| {
                BookXmlError::InvalidScoreBoolean {
                    score_index: index,
                    field: "book/score/@logicals-locked",
                    value: value.clone(),
                }
            })?);
        }
    }
    score_refs.push(ScoreRef {
        index,
        logicals_locked,
        logical_parts: Vec::new(),
        pages: Vec::new(),
    });
    Ok(score_refs.len() - 1)
}

fn push_logical_part(
    reader: &Reader<Cursor<&[u8]>>,
    element: &BytesStart<'_>,
    score_ref: &mut ScoreRef,
) -> Result<usize, BookXmlError> {
    let logical_index = u32::try_from(score_ref.logical_parts.len())
        .ok()
        .filter(|index| *index <= i32::MAX as u32)
        .ok_or(BookXmlError::TooManyLogicalParts {
            score_index: score_ref.index,
        })?;
    let mut id = None;
    let mut staff_count = None;
    let mut name = None;
    let mut abbreviation = None;
    let mut midi_program = None;

    for attribute in element.attributes() {
        let attribute = attribute
            .map_err(|error| BookXmlError::malformed(reader.error_position(), error.to_string()))?;
        let key = attribute.key.as_ref();
        if key == ID_ATTRIBUTE {
            let value = decode_attribute(reader, &attribute)?;
            id = Some(parse_positive_logical_part_integer(
                &value,
                score_ref.index,
                logical_index,
                "book/score/logical-part/@id",
            )?);
        } else if key == STAFF_COUNT_ATTRIBUTE {
            let value = decode_attribute(reader, &attribute)?;
            staff_count = Some(parse_positive_logical_part_integer(
                &value,
                score_ref.index,
                logical_index,
                "book/score/logical-part/@staff-count",
            )?);
        } else if key == NAME_ATTRIBUTE {
            name = Some(decode_attribute(reader, &attribute)?);
        } else if key == ABBREVIATION_ATTRIBUTE {
            abbreviation = Some(decode_attribute(reader, &attribute)?);
        } else if key == MIDI_PROGRAM_ATTRIBUTE {
            let value = decode_attribute(reader, &attribute)?;
            midi_program = Some(value.trim().parse::<i32>().map_err(|_| {
                BookXmlError::InvalidLogicalPartInteger {
                    score_index: score_ref.index,
                    logical_index,
                    field: "book/score/logical-part/@midi-program",
                    value: value.clone(),
                }
            })?);
        }
    }

    let id = id.ok_or(BookXmlError::MissingLogicalPartField {
        score_index: score_ref.index,
        logical_index,
        field: "book/score/logical-part/@id",
    })?;
    let staff_count = staff_count.ok_or(BookXmlError::MissingLogicalPartField {
        score_index: score_ref.index,
        logical_index,
        field: "book/score/logical-part/@staff-count",
    })?;
    if score_ref.logical_parts.iter().any(|part| part.id == id) {
        return Err(BookXmlError::DuplicateLogicalPartId {
            score_index: score_ref.index,
            id,
        });
    }
    score_ref.logical_parts.push(LogicalPartRef {
        index: logical_index,
        id,
        staff_count,
        name,
        abbreviation,
        midi_program,
        staff_configs: Vec::new(),
    });
    Ok(score_ref.logical_parts.len() - 1)
}

fn parse_positive_logical_part_integer(
    value: &str,
    score_index: u32,
    logical_index: u32,
    field: &'static str,
) -> Result<u32, BookXmlError> {
    value
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|number| *number > 0 && *number <= i32::MAX as u32)
        .ok_or_else(|| BookXmlError::InvalidLogicalPartInteger {
            score_index,
            logical_index,
            field,
            value: value.to_owned(),
        })
}

fn begin_logical_staff_leaf(
    reader: &Reader<Cursor<&[u8]>>,
    element: &BytesStart<'_>,
    score_ref: &mut ScoreRef,
    logical_index: usize,
    kind: StaffLeafKind,
) -> Result<LogicalStaffLeafCapture, BookXmlError> {
    if kind == StaffLeafKind::Current {
        push_current_logical_staff_config(reader, element, score_ref, logical_index)?;
    }
    Ok(LogicalStaffLeafCapture {
        score_index: score_ref.index as usize,
        logical_index,
        logical_source_index: score_ref.logical_parts[logical_index].index,
        kind,
        text: String::new(),
    })
}

fn finish_empty_logical_staff_leaf(
    reader: &Reader<Cursor<&[u8]>>,
    element: &BytesStart<'_>,
    score_ref: &mut ScoreRef,
    logical_index: usize,
    kind: StaffLeafKind,
) -> Result<(), BookXmlError> {
    match kind {
        StaffLeafKind::Current => {
            push_current_logical_staff_config(reader, element, score_ref, logical_index)
        }
        StaffLeafKind::DeprecatedLineCount => {
            push_deprecated_logical_line_count(score_ref, logical_index, "")
        }
    }
}

fn finish_logical_staff_leaf(
    score_refs: &mut [ScoreRef],
    capture: LogicalStaffLeafCapture,
) -> Result<(), BookXmlError> {
    if capture.kind == StaffLeafKind::DeprecatedLineCount {
        push_deprecated_logical_line_count(
            &mut score_refs[capture.score_index],
            capture.logical_index,
            &capture.text,
        )?;
    }
    Ok(())
}

fn push_current_logical_staff_config(
    reader: &Reader<Cursor<&[u8]>>,
    element: &BytesStart<'_>,
    score_ref: &mut ScoreRef,
    logical_index: usize,
) -> Result<(), BookXmlError> {
    let logical_source_index = score_ref.logical_parts[logical_index].index;
    let mut line_count = None;
    let mut small = None;
    for attribute in element.attributes() {
        let attribute = attribute
            .map_err(|error| BookXmlError::malformed(reader.error_position(), error.to_string()))?;
        let key = attribute.key.as_ref();
        if key == LINE_COUNT_ATTRIBUTE {
            let value = decode_attribute(reader, &attribute)?;
            line_count = Some(parse_logical_staff_integer(
                &value,
                score_ref.index,
                logical_source_index,
                "book/score/logical-part/staff-configuration/@line-count",
            )?);
        } else if key == SMALL_ATTRIBUTE {
            let value = decode_attribute(reader, &attribute)?;
            small = Some(parse_jaxb_boolean(&value).ok_or_else(|| {
                BookXmlError::InvalidLogicalStaffConfigBoolean {
                    score_index: score_ref.index,
                    logical_index: logical_source_index,
                    field: "book/score/logical-part/staff-configuration/@small",
                    value: value.clone(),
                }
            })?);
        }
    }
    let line_count = line_count.ok_or(BookXmlError::MissingLogicalStaffConfigField {
        score_index: score_ref.index,
        logical_index: logical_source_index,
        field: "book/score/logical-part/staff-configuration/@line-count",
    })?;
    score_ref.logical_parts[logical_index]
        .staff_configs
        .push(PersistedStaffConfig::Current(StaffConfig {
            line_count,
            small,
        }));
    Ok(())
}

fn push_deprecated_logical_line_count(
    score_ref: &mut ScoreRef,
    logical_index: usize,
    text: &str,
) -> Result<(), BookXmlError> {
    let logical_source_index = score_ref.logical_parts[logical_index].index;
    let count = parse_logical_staff_integer(
        text,
        score_ref.index,
        logical_source_index,
        "book/score/logical-part/line-count",
    )?;
    score_ref.logical_parts[logical_index]
        .staff_configs
        .push(PersistedStaffConfig::DeprecatedLineCount(count));
    Ok(())
}

fn parse_logical_staff_integer(
    value: &str,
    score_index: u32,
    logical_index: u32,
    field: &'static str,
) -> Result<i32, BookXmlError> {
    value
        .trim()
        .parse::<i32>()
        .map_err(|_| BookXmlError::InvalidLogicalStaffConfigInteger {
            score_index,
            logical_index,
            field,
            value: value.to_owned(),
        })
}

fn unexpected_logical_staff_config_content(capture: &LogicalStaffLeafCapture) -> BookXmlError {
    BookXmlError::UnexpectedLogicalStaffConfigContent {
        score_index: capture.score_index as u32,
        logical_index: capture.logical_source_index,
    }
}

fn push_score_page(
    reader: &Reader<Cursor<&[u8]>>,
    element: &BytesStart<'_>,
    score_ref: &mut ScoreRef,
) -> Result<(), BookXmlError> {
    let mut sheet_number = None;
    let mut sheet_page_id = None;
    for attribute in element.attributes() {
        let attribute = attribute
            .map_err(|error| BookXmlError::malformed(reader.error_position(), error.to_string()))?;
        let key = attribute.key.as_ref();
        if key == SHEET_NUMBER_ATTRIBUTE {
            let value = decode_attribute(reader, &attribute)?;
            sheet_number = Some(parse_score_page_integer(
                &value,
                score_ref.index,
                "book/score/page/@sheet-number",
            )?);
        } else if key == SHEET_PAGE_ID_ATTRIBUTE {
            let value = decode_attribute(reader, &attribute)?;
            sheet_page_id = Some(parse_score_page_integer(
                &value,
                score_ref.index,
                "book/score/page/@sheet-page-id",
            )?);
        }
    }
    let sheet_number = sheet_number.ok_or(BookXmlError::MissingScorePageField {
        score_index: score_ref.index,
        field: "book/score/page/@sheet-number",
    })?;
    let sheet_page_id = sheet_page_id.ok_or(BookXmlError::MissingScorePageField {
        score_index: score_ref.index,
        field: "book/score/page/@sheet-page-id",
    })?;
    let page = ScorePageRef {
        sheet_number,
        sheet_page_id,
    };
    if score_ref.pages.contains(&page) {
        return Err(BookXmlError::DuplicateScorePage {
            score_index: score_ref.index,
            sheet_number,
            sheet_page_id,
        });
    }
    score_ref.pages.push(page);
    Ok(())
}

fn parse_score_page_integer(
    value: &str,
    score_index: u32,
    field: &'static str,
) -> Result<u32, BookXmlError> {
    value
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|number| *number > 0 && *number <= i32::MAX as u32)
        .ok_or_else(|| BookXmlError::InvalidScorePageInteger {
            score_index,
            field,
            value: value.to_owned(),
        })
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
) -> Result<usize, BookXmlError> {
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
        last_time_rational: None,
        system_refs: Vec::new(),
    });
    Ok(sheet_stub.page_refs.len() - 1)
}

fn push_system_ref(sheet_stub: &mut SheetStub, page_index: usize) -> Result<usize, BookXmlError> {
    let page = &mut sheet_stub.page_refs[page_index];
    let id = page
        .system_refs
        .len()
        .checked_add(1)
        .ok_or(BookXmlError::TooManySystems {
            sheet_number: sheet_stub.number,
            page_id: page.id,
        })?;
    let id = u32::try_from(id)
        .ok()
        .filter(|id| *id <= i32::MAX as u32)
        .ok_or(BookXmlError::TooManySystems {
            sheet_number: sheet_stub.number,
            page_id: page.id,
        })?;
    page.system_refs.push(SystemRef {
        id,
        part_refs: Vec::new(),
    });
    Ok(page.system_refs.len() - 1)
}

fn push_part_ref(
    reader: &Reader<Cursor<&[u8]>>,
    element: &BytesStart<'_>,
    sheet_stub: &mut SheetStub,
    page_index: usize,
    system_index: usize,
) -> Result<usize, BookXmlError> {
    let page_id = sheet_stub.page_refs[page_index].id;
    let system = &mut sheet_stub.page_refs[page_index].system_refs[system_index];
    let index = u32::try_from(system.part_refs.len())
        .ok()
        .filter(|index| *index <= i32::MAX as u32)
        .ok_or(BookXmlError::TooManyParts {
            sheet_number: sheet_stub.number,
            page_id,
            system_id: system.id,
        })?;

    let mut name = None;
    let mut logical_id = None;
    let mut manual = None;
    for attribute in element.attributes() {
        let attribute = attribute
            .map_err(|error| BookXmlError::malformed(reader.error_position(), error.to_string()))?;
        let key = attribute.key.as_ref();
        if key == NAME_ATTRIBUTE {
            name = Some(decode_attribute(reader, &attribute)?);
        } else if key == LOGICAL_ID_ATTRIBUTE {
            let value = decode_attribute(reader, &attribute)?;
            logical_id = Some(value.trim().parse::<i32>().map_err(|_| {
                BookXmlError::InvalidPartInteger {
                    sheet_number: sheet_stub.number,
                    page_id,
                    system_id: system.id,
                    field: "sheet/page/system/part/@logical-id",
                    value: value.clone(),
                }
            })?);
        } else if key == MANUAL_ATTRIBUTE {
            let value = decode_attribute(reader, &attribute)?;
            manual = Some(parse_jaxb_boolean(&value).ok_or_else(|| {
                BookXmlError::InvalidPartBoolean {
                    sheet_number: sheet_stub.number,
                    page_id,
                    system_id: system.id,
                    field: "sheet/page/system/part/@manual",
                    value: value.clone(),
                }
            })?);
        }
    }

    system.part_refs.push(PartRef {
        index,
        name,
        logical_id,
        manual,
        staff_configs: Vec::new(),
    });
    Ok(system.part_refs.len() - 1)
}

fn staff_leaf_kind(name: &[u8]) -> Option<StaffLeafKind> {
    match name {
        STAFF_CONFIGURATION_ELEMENT => Some(StaffLeafKind::Current),
        DEPRECATED_LINE_COUNT_ELEMENT => Some(StaffLeafKind::DeprecatedLineCount),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn begin_staff_leaf(
    reader: &Reader<Cursor<&[u8]>>,
    element: &BytesStart<'_>,
    sheet_index: usize,
    sheet_stub: &mut SheetStub,
    page_index: usize,
    system_index: usize,
    part_index: usize,
    kind: StaffLeafKind,
) -> Result<StaffLeafCapture, BookXmlError> {
    if kind == StaffLeafKind::Current {
        push_current_staff_config(
            reader,
            element,
            sheet_stub,
            page_index,
            system_index,
            part_index,
        )?;
    }
    Ok(StaffLeafCapture {
        sheet_index,
        page_index,
        system_index,
        part_index,
        kind,
        text: String::new(),
    })
}

#[allow(clippy::too_many_arguments)]
fn finish_empty_staff_leaf(
    reader: &Reader<Cursor<&[u8]>>,
    element: &BytesStart<'_>,
    sheet_stub: &mut SheetStub,
    page_index: usize,
    system_index: usize,
    part_index: usize,
    kind: StaffLeafKind,
) -> Result<(), BookXmlError> {
    match kind {
        StaffLeafKind::Current => push_current_staff_config(
            reader,
            element,
            sheet_stub,
            page_index,
            system_index,
            part_index,
        ),
        StaffLeafKind::DeprecatedLineCount => {
            push_deprecated_line_count(sheet_stub, page_index, system_index, part_index, "")
        }
    }
}

fn finish_staff_leaf(
    sheet_stubs: &mut [SheetStub],
    capture: StaffLeafCapture,
) -> Result<(), BookXmlError> {
    if capture.kind == StaffLeafKind::DeprecatedLineCount {
        push_deprecated_line_count(
            &mut sheet_stubs[capture.sheet_index],
            capture.page_index,
            capture.system_index,
            capture.part_index,
            &capture.text,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn push_current_staff_config(
    reader: &Reader<Cursor<&[u8]>>,
    element: &BytesStart<'_>,
    sheet_stub: &mut SheetStub,
    page_index: usize,
    system_index: usize,
    part_index: usize,
) -> Result<(), BookXmlError> {
    let (sheet_number, page_id, system_id, part_source_index) =
        staff_context(sheet_stub, page_index, system_index, part_index);
    let mut line_count = None;
    let mut small = None;
    for attribute in element.attributes() {
        let attribute = attribute
            .map_err(|error| BookXmlError::malformed(reader.error_position(), error.to_string()))?;
        let key = attribute.key.as_ref();
        if key == LINE_COUNT_ATTRIBUTE {
            let value = decode_attribute(reader, &attribute)?;
            line_count = Some(parse_staff_integer(
                &value,
                sheet_number,
                page_id,
                system_id,
                part_source_index,
                "sheet/page/system/part/staff-configuration/@line-count",
            )?);
        } else if key == SMALL_ATTRIBUTE {
            let value = decode_attribute(reader, &attribute)?;
            small = Some(parse_jaxb_boolean(&value).ok_or_else(|| {
                BookXmlError::InvalidStaffConfigBoolean {
                    sheet_number,
                    page_id,
                    system_id,
                    part_index: part_source_index,
                    field: "sheet/page/system/part/staff-configuration/@small",
                    value: value.clone(),
                }
            })?);
        }
    }
    let line_count = line_count.ok_or(BookXmlError::MissingStaffConfigField {
        sheet_number,
        page_id,
        system_id,
        part_index: part_source_index,
        field: "sheet/page/system/part/staff-configuration/@line-count",
    })?;
    part_mut(sheet_stub, page_index, system_index, part_index)
        .staff_configs
        .push(PersistedStaffConfig::Current(StaffConfig {
            line_count,
            small,
        }));
    Ok(())
}

fn push_deprecated_line_count(
    sheet_stub: &mut SheetStub,
    page_index: usize,
    system_index: usize,
    part_index: usize,
    text: &str,
) -> Result<(), BookXmlError> {
    let (sheet_number, page_id, system_id, part_source_index) =
        staff_context(sheet_stub, page_index, system_index, part_index);
    let count = parse_staff_integer(
        text,
        sheet_number,
        page_id,
        system_id,
        part_source_index,
        "sheet/page/system/part/line-count",
    )?;
    part_mut(sheet_stub, page_index, system_index, part_index)
        .staff_configs
        .push(PersistedStaffConfig::DeprecatedLineCount(count));
    Ok(())
}

fn parse_staff_integer(
    value: &str,
    sheet_number: u32,
    page_id: u32,
    system_id: u32,
    part_index: u32,
    field: &'static str,
) -> Result<i32, BookXmlError> {
    value
        .trim()
        .parse::<i32>()
        .map_err(|_| BookXmlError::InvalidStaffConfigInteger {
            sheet_number,
            page_id,
            system_id,
            part_index,
            field,
            value: value.to_owned(),
        })
}

fn staff_context(
    sheet_stub: &SheetStub,
    page_index: usize,
    system_index: usize,
    part_index: usize,
) -> (u32, u32, u32, u32) {
    let page = &sheet_stub.page_refs[page_index];
    let system = &page.system_refs[system_index];
    let part = &system.part_refs[part_index];
    (sheet_stub.number, page.id, system.id, part.index)
}

fn part_mut(
    sheet_stub: &mut SheetStub,
    page_index: usize,
    system_index: usize,
    part_index: usize,
) -> &mut PartRef {
    &mut sheet_stub.page_refs[page_index].system_refs[system_index].part_refs[part_index]
}

fn unexpected_staff_config_content(
    sheet_stubs: &[SheetStub],
    capture: &StaffLeafCapture,
) -> BookXmlError {
    let (sheet_number, page_id, system_id, part_index) = staff_context(
        &sheet_stubs[capture.sheet_index],
        capture.page_index,
        capture.system_index,
        capture.part_index,
    );
    BookXmlError::UnexpectedStaffConfigContent {
        sheet_number,
        page_id,
        system_id,
        part_index,
    }
}

fn push_last_time_rational(
    reader: &Reader<Cursor<&[u8]>>,
    element: &BytesStart<'_>,
    sheet_stub: &mut SheetStub,
    page_index: usize,
) -> Result<(), BookXmlError> {
    let page = &sheet_stub.page_refs[page_index];
    if page.last_time_rational.is_some() {
        return Err(BookXmlError::DuplicateLastTimeRational {
            sheet_number: sheet_stub.number,
            page_id: page.id,
        });
    }

    let mut numerator = None;
    let mut denominator = None;
    for attribute in element.attributes() {
        let attribute = attribute
            .map_err(|error| BookXmlError::malformed(reader.error_position(), error.to_string()))?;
        let key = attribute.key.as_ref();
        if key == NUM_ATTRIBUTE {
            let value = decode_attribute(reader, &attribute)?;
            numerator = Some(parse_time_integer(
                &value,
                sheet_stub.number,
                page.id,
                "sheet/page/last-time-rational/@num",
            )?);
        } else if key == DEN_ATTRIBUTE {
            let value = decode_attribute(reader, &attribute)?;
            denominator = Some(parse_time_integer(
                &value,
                sheet_stub.number,
                page.id,
                "sheet/page/last-time-rational/@den",
            )?);
        }
    }

    let numerator = numerator.ok_or(BookXmlError::MissingTimeRationalField {
        sheet_number: sheet_stub.number,
        page_id: page.id,
        field: "sheet/page/last-time-rational/@num",
    })?;
    let denominator = denominator.ok_or(BookXmlError::MissingTimeRationalField {
        sheet_number: sheet_stub.number,
        page_id: page.id,
        field: "sheet/page/last-time-rational/@den",
    })?;
    sheet_stub.page_refs[page_index].last_time_rational = Some(TimeRational {
        numerator,
        denominator,
    });
    Ok(())
}

fn parse_time_integer(
    value: &str,
    sheet_number: u32,
    page_id: u32,
    field: &'static str,
) -> Result<i32, BookXmlError> {
    value
        .trim()
        .parse::<i32>()
        .map_err(|_| BookXmlError::InvalidTimeRationalInteger {
            sheet_number,
            page_id,
            field,
            value: value.to_owned(),
        })
}

fn unexpected_time_rational_content(
    sheet_stubs: &[SheetStub],
    sheet_index: usize,
    page_index: usize,
) -> BookXmlError {
    BookXmlError::UnexpectedTimeRationalContent {
        sheet_number: sheet_stubs[sheet_index].number,
        page_id: sheet_stubs[sheet_index].page_refs[page_index].id,
    }
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
  <score>
    <logical-part id="1" staff-count="2"><staff-configuration line-count="5"/><staff-configuration line-count="5"/></logical-part>
    <page sheet-number="1" sheet-page-id="1"/>
  </score>
</book>"#;
        let book = BookXml::parse(xml).unwrap();
        let pages = book.sheet_stubs()[0].page_refs();

        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].id(), 1);
        assert_eq!(pages[0].movement_start_attribute(), Some(true));
        assert!(pages[0].is_movement_start());
        assert_eq!(pages[0].delta_measure_id(), Some(12));
        assert_eq!(
            pages[0].last_time_rational(),
            Some(TimeRational {
                numerator: 4,
                denominator: 4,
            })
        );
        assert_eq!(pages[0].system_refs().len(), 1);
        assert_eq!(pages[0].system_refs()[0].id(), 1);
        let parts = pages[0].system_refs()[0].part_refs();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].index(), 0);
        assert_eq!(parts[0].logical_id(), Some(1));
        assert_eq!(parts[0].name(), None);
        assert_eq!(parts[0].manual_attribute(), None);
        assert!(!parts[0].is_manual());
        assert_eq!(parts[0].staff_configs().len(), 1);
        let PersistedStaffConfig::Current(staff) = parts[0].staff_configs()[0] else {
            panic!("real 5.11 spelling must stay current")
        };
        assert_eq!(staff.line_count(), 5);
        assert_eq!(staff.small_attribute(), None);
        assert!(!staff.is_small());
        let scores = book.score_refs();
        assert_eq!(scores.len(), 1);
        assert_eq!(scores[0].index(), 0);
        assert_eq!(scores[0].logicals_locked_attribute(), None);
        assert!(!scores[0].logicals_locked());
        assert_eq!(scores[0].logical_parts().len(), 1);
        let logical = &scores[0].logical_parts()[0];
        assert_eq!(logical.index(), 0);
        assert_eq!(logical.id(), 1);
        assert_eq!(logical.staff_count(), 2);
        assert_eq!(logical.name(), None);
        assert_eq!(logical.abbreviation(), None);
        assert_eq!(logical.midi_program(), None);
        assert_eq!(logical.staff_configs().len(), 2);
        assert!(logical.staff_configs().iter().all(|config| matches!(
            config,
            PersistedStaffConfig::Current(staff) if staff.line_count() == 5
        )));
        assert_eq!(scores[0].pages().len(), 1);
        assert_eq!(scores[0].pages()[0].sheet_number(), 1);
        assert_eq!(scores[0].pages()[0].sheet_page_id(), 1);
        assert_eq!(book.original_bytes(), xml);
    }

    #[test]
    fn reads_ordered_score_page_links_and_lock_states() {
        let xml = br#"<book><score/><score logicals-locked="false"><logical-part id="1" staff-count="1"/><page sheet-number="2" sheet-page-id="1"/><page sheet-number="1" sheet-page-id="2"> </page></score><score logicals-locked="1"><page sheet-number="3" sheet-page-id="1"/></score></book>"#;
        let book = BookXml::parse(xml).unwrap();
        let scores = book.score_refs();

        assert_eq!(
            scores.iter().map(|score| score.index()).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert!(scores[0].pages().is_empty());
        assert_eq!(scores[0].logicals_locked_attribute(), None);
        assert_eq!(scores[1].logicals_locked_attribute(), Some(false));
        assert!(!scores[1].logicals_locked());
        assert_eq!(
            scores[1]
                .pages()
                .iter()
                .map(|page| (page.sheet_number(), page.sheet_page_id()))
                .collect::<Vec<_>>(),
            [(2, 1), (1, 2)]
        );
        assert_eq!(scores[2].logicals_locked_attribute(), Some(true));
        assert!(scores[2].logicals_locked());
        assert_eq!(book.original_bytes(), xml);
    }

    #[test]
    fn reads_ordered_logical_part_scalars_and_preserves_optional_states() {
        let xml = br#"<book><score><logical-part id="2" staff-count="1" name="P&amp;1" abbreviation="" midi-program=" -1 "/><logical-part id="1" staff-count="2"><line-count>1</line-count><staff-configuration line-count="5" small="true"/><line-count> 0 </line-count><staff-configuration line-count="-2" small="false"> </staff-configuration></logical-part></score><score/></book>"#;
        let book = BookXml::parse(xml).unwrap();
        let parts = book.score_refs()[0].logical_parts();

        assert_eq!(parts.len(), 2);
        assert_eq!(
            (parts[0].index(), parts[0].id(), parts[0].staff_count()),
            (0, 2, 1)
        );
        assert_eq!(parts[0].name(), Some("P&1"));
        assert_eq!(parts[0].abbreviation(), Some(""));
        assert_eq!(parts[0].midi_program(), Some(-1));
        assert!(parts[0].staff_configs().is_empty());
        assert_eq!(
            (parts[1].index(), parts[1].id(), parts[1].staff_count()),
            (1, 1, 2)
        );
        assert_eq!(parts[1].name(), None);
        assert_eq!(parts[1].abbreviation(), None);
        assert_eq!(parts[1].midi_program(), None);
        assert_eq!(
            parts[1].staff_configs()[0],
            PersistedStaffConfig::DeprecatedLineCount(1)
        );
        assert!(matches!(
            parts[1].staff_configs()[1],
            PersistedStaffConfig::Current(staff)
                if staff.line_count() == 5 && staff.small_attribute() == Some(true)
        ));
        assert_eq!(
            parts[1].staff_configs()[2],
            PersistedStaffConfig::DeprecatedLineCount(0)
        );
        assert!(matches!(
            parts[1].staff_configs()[3],
            PersistedStaffConfig::Current(staff)
                if staff.line_count() == -2 && staff.small_attribute() == Some(false)
        ));
        assert!(book.score_refs()[1].logical_parts().is_empty());
        assert_eq!(book.original_bytes(), xml);
    }

    #[test]
    fn ignores_nested_and_namespaced_logical_part_lookalikes() {
        let book = BookXml::parse(
            br#"<book xmlns:f="urn:future"><f:score><logical-part id="9" staff-count="9"/></f:score><score><future><logical-part id="8" staff-count="8"/></future><f:logical-part id="7" staff-count="7"/><logical-part f:id="6" id="1" staff-count="2"><f:staff-configuration line-count="8"/><future><staff-configuration line-count="7"/><line-count>6</line-count></future><staff-configuration f:line-count="5" line-count="3"/></logical-part></score></book>"#,
        )
        .unwrap();

        assert_eq!(book.score_refs().len(), 1);
        let parts = book.score_refs()[0].logical_parts();
        assert_eq!(parts.len(), 1);
        assert_eq!((parts[0].id(), parts[0].staff_count()), (1, 2));
        assert_eq!(parts[0].staff_configs().len(), 1);
        assert!(matches!(
            parts[0].staff_configs()[0],
            PersistedStaffConfig::Current(staff) if staff.line_count() == 3
        ));
    }

    #[test]
    fn rejects_missing_invalid_and_duplicate_logical_part_scalars() {
        for (xml, field) in [
            (
                br#"<book><score><logical-part staff-count="1"/></score></book>"#.as_slice(),
                "book/score/logical-part/@id",
            ),
            (
                br#"<book><score><logical-part id="1"/></score></book>"#.as_slice(),
                "book/score/logical-part/@staff-count",
            ),
        ] {
            assert_eq!(
                BookXml::parse(xml).unwrap_err(),
                BookXmlError::MissingLogicalPartField {
                    score_index: 0,
                    logical_index: 0,
                    field,
                }
            );
        }

        for (field, attrs, value) in [
            (
                "book/score/logical-part/@id",
                "id=\"0\" staff-count=\"1\"",
                "0",
            ),
            (
                "book/score/logical-part/@staff-count",
                "id=\"1\" staff-count=\"2147483648\"",
                "2147483648",
            ),
            (
                "book/score/logical-part/@midi-program",
                "id=\"1\" staff-count=\"1\" midi-program=\"bad\"",
                "bad",
            ),
        ] {
            let xml = format!("<book><score><logical-part {attrs}/></score></book>");
            assert_eq!(
                BookXml::parse(xml).unwrap_err(),
                BookXmlError::InvalidLogicalPartInteger {
                    score_index: 0,
                    logical_index: 0,
                    field,
                    value: value.to_owned(),
                }
            );
        }

        assert_eq!(
            BookXml::parse(br#"<book><score><logical-part id="1" staff-count="1"/><logical-part id="1" staff-count="2"/></score></book>"#).unwrap_err(),
            BookXmlError::DuplicateLogicalPartId {
                score_index: 0,
                id: 1,
            }
        );
    }

    #[test]
    fn rejects_invalid_logical_part_staff_configs() {
        assert_eq!(
            BookXml::parse(br#"<book><score><logical-part id="1" staff-count="1"><staff-configuration/></logical-part></score></book>"#).unwrap_err(),
            BookXmlError::MissingLogicalStaffConfigField {
                score_index: 0,
                logical_index: 0,
                field: "book/score/logical-part/staff-configuration/@line-count",
            }
        );
        for (field, child, value) in [
            (
                "book/score/logical-part/staff-configuration/@line-count",
                "<staff-configuration line-count=\"bad\"/>",
                "bad",
            ),
            (
                "book/score/logical-part/line-count",
                "<line-count>2147483648</line-count>",
                "2147483648",
            ),
        ] {
            let xml = format!(
                "<book><score><logical-part id=\"1\" staff-count=\"1\">{child}</logical-part></score></book>"
            );
            assert_eq!(
                BookXml::parse(xml).unwrap_err(),
                BookXmlError::InvalidLogicalStaffConfigInteger {
                    score_index: 0,
                    logical_index: 0,
                    field,
                    value: value.to_owned(),
                }
            );
        }
        assert_eq!(
            BookXml::parse(br#"<book><score><logical-part id="1" staff-count="1"><staff-configuration line-count="5" small="yes"/></logical-part></score></book>"#).unwrap_err(),
            BookXmlError::InvalidLogicalStaffConfigBoolean {
                score_index: 0,
                logical_index: 0,
                field: "book/score/logical-part/staff-configuration/@small",
                value: "yes".to_owned(),
            }
        );
    }

    #[test]
    fn rejects_logical_part_staff_markup_and_duplicate_attributes() {
        for child in [
            "<staff-configuration line-count=\"5\">text</staff-configuration>",
            "<staff-configuration line-count=\"5\"><future/></staff-configuration>",
            "<line-count><![CDATA[5]]></line-count>",
            "<line-count>&#53;</line-count>",
            "<line-count><!--5--></line-count>",
        ] {
            let xml = format!(
                "<book><score><logical-part id=\"1\" staff-count=\"1\">{child}</logical-part></score></book>"
            );
            assert_eq!(
                BookXml::parse(xml).unwrap_err(),
                BookXmlError::UnexpectedLogicalStaffConfigContent {
                    score_index: 0,
                    logical_index: 0,
                }
            );
        }

        for xml in [
            br#"<book><score><logical-part id="1" id="2" staff-count="1"/></score></book>"#.as_slice(),
            br#"<book><score><logical-part id="1" staff-count="1"><staff-configuration line-count="5" line-count="4"/></logical-part></score></book>"#.as_slice(),
        ] {
            assert!(matches!(
                BookXml::parse(xml).unwrap_err(),
                BookXmlError::Malformed { .. }
            ));
        }
    }

    #[test]
    fn ignores_nested_and_namespaced_score_page_lookalikes() {
        let book = BookXml::parse(
            br#"<book xmlns:f="urn:future"><f:score><page sheet-number="9" sheet-page-id="9"/></f:score><score><future><page sheet-number="8" sheet-page-id="8"/></future><f:page sheet-number="7" sheet-page-id="7"/><page f:sheet-number="6" sheet-number="1" sheet-page-id="2"/></score></book>"#,
        )
        .unwrap();

        assert_eq!(book.score_refs().len(), 1);
        assert_eq!(book.score_refs()[0].pages().len(), 1);
        assert_eq!(book.score_refs()[0].pages()[0].sheet_number(), 1);
        assert_eq!(book.score_refs()[0].pages()[0].sheet_page_id(), 2);
    }

    #[test]
    fn preserves_non_reduced_and_raw_java_int_time_rationals() {
        let book = BookXml::parse(
            br#"<book><sheet number="1"><page id="1"><last-time-rational num="6" den="8"/></page><page id="2"><last-time-rational num=" -2147483648 " den="0">  </last-time-rational></page><page id="3"/></sheet></book>"#,
        )
        .unwrap();
        let pages = book.sheet_stubs()[0].page_refs();

        let compound = pages[0].last_time_rational().unwrap();
        assert_eq!(compound.numerator(), 6);
        assert_eq!(compound.denominator(), 8);
        let raw = pages[1].last_time_rational().unwrap();
        assert_eq!(raw.numerator(), i32::MIN);
        assert_eq!(raw.denominator(), 0);
        assert_eq!(pages[2].last_time_rational(), None);
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
        assert!(pages[0].system_refs().is_empty());
    }

    #[test]
    fn derives_java_system_ids_from_direct_document_order_only() {
        let xml = br#"<book><sheet number="1"><page id="1"><system id="not-persisted"><part logical-id="3"><staff-configuration line-count="5"/></part></system><system future="opaque"/></page><page id="2"/></sheet></book>"#;
        let book = BookXml::parse(xml).unwrap();
        let pages = book.sheet_stubs()[0].page_refs();

        assert_eq!(
            pages[0]
                .system_refs()
                .iter()
                .map(|system| system.id())
                .collect::<Vec<_>>(),
            [1, 2]
        );
        assert!(pages[1].system_refs().is_empty());
        assert_eq!(book.original_bytes(), xml);
    }

    #[test]
    fn reads_part_scalars_and_preserves_order_and_absent_states() {
        let xml = br#"<book><sheet number="1"><page id="1"><system><part name="P&amp;1" logical-id=" -2 " manual="false"><staff-configuration line-count="5"/></part><part name="" manual="1"/><part/></system></page></sheet></book>"#;
        let book = BookXml::parse(xml).unwrap();
        let parts = book.sheet_stubs()[0].page_refs()[0].system_refs()[0].part_refs();

        assert_eq!(
            parts.iter().map(|part| part.index()).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert_eq!(parts[0].name(), Some("P&1"));
        assert_eq!(parts[0].logical_id(), Some(-2));
        assert_eq!(parts[0].manual_attribute(), Some(false));
        assert!(!parts[0].is_manual());
        assert_eq!(parts[1].name(), Some(""));
        assert_eq!(parts[1].logical_id(), None);
        assert_eq!(parts[1].manual_attribute(), Some(true));
        assert!(parts[1].is_manual());
        assert_eq!(parts[2].name(), None);
        assert_eq!(parts[2].manual_attribute(), None);
        assert_eq!(book.original_bytes(), xml);
    }

    #[test]
    fn preserves_current_and_deprecated_staff_spellings_in_document_order() {
        let xml = br#"<book><sheet number="1"><page id="1"><system><part><line-count>1</line-count><staff-configuration line-count="5" small="true"/><line-count> 0 </line-count><staff-configuration line-count="-2" small="false"> </staff-configuration></part><part/></system></page></sheet></book>"#;
        let book = BookXml::parse(xml).unwrap();
        let parts = book.sheet_stubs()[0].page_refs()[0].system_refs()[0].part_refs();
        let configs = parts[0].staff_configs();

        assert_eq!(configs[0], PersistedStaffConfig::DeprecatedLineCount(1));
        let PersistedStaffConfig::Current(first) = configs[1] else {
            panic!("second spelling must stay current")
        };
        assert_eq!(first.line_count(), 5);
        assert_eq!(first.small_attribute(), Some(true));
        assert!(first.is_small());
        assert_eq!(configs[2], PersistedStaffConfig::DeprecatedLineCount(0));
        let PersistedStaffConfig::Current(last) = configs[3] else {
            panic!("fourth spelling must stay current")
        };
        assert_eq!(last.line_count(), -2);
        assert_eq!(last.small_attribute(), Some(false));
        assert!(!last.is_small());
        assert!(parts[1].staff_configs().is_empty());
        assert_eq!(book.original_bytes(), xml);
    }

    #[test]
    fn ignores_nested_and_namespaced_page_lookalikes() {
        let book = BookXml::parse(
            br#"<book xmlns:av="urn:audiveris" xmlns:f="urn:future"><av:sheet number="1"><future><av:page id="99"/></future><av:page id="3" f:id="88" f:movement-start="true"><f:last-time-rational num="3" den="4"/><f:system/><future><av:page id="77"/><last-time-rational num="2" den="2"/><system/></future><system><f:part logical-id="88"/><future><part logical-id="77"/></future><part f:logical-id="66" future="opaque"><f:staff-configuration line-count="88"/><future><staff-configuration line-count="77"/><line-count>66</line-count></future><staff-configuration f:line-count="55" line-count="3"/></part></system></av:page></av:sheet><score><av:page id="66"/></score></book>"#,
        )
        .unwrap();

        let pages = book.sheet_stubs()[0].page_refs();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].id(), 3);
        assert_eq!(pages[0].movement_start_attribute(), None);
        assert_eq!(pages[0].last_time_rational(), None);
        assert_eq!(pages[0].system_refs().len(), 1);
        assert_eq!(pages[0].system_refs()[0].id(), 1);
        let parts = pages[0].system_refs()[0].part_refs();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].index(), 0);
        assert_eq!(parts[0].logical_id(), None);
        assert_eq!(parts[0].staff_configs().len(), 1);
        let PersistedStaffConfig::Current(config) = parts[0].staff_configs()[0] else {
            panic!("direct unqualified spelling must be current")
        };
        assert_eq!(config.line_count(), 3);
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
            <score><page sheet-number="2" sheet-page-id="1"/></score>
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
    fn rejects_invalid_score_lock_and_page_coordinates() {
        assert_eq!(
            BookXml::parse(br#"<book><score logicals-locked="yes"/></book>"#).unwrap_err(),
            BookXmlError::InvalidScoreBoolean {
                score_index: 0,
                field: "book/score/@logicals-locked",
                value: "yes".to_owned(),
            }
        );
        assert_eq!(
            BookXml::parse(br#"<book><score><page sheet-page-id="1"/></score></book>"#)
                .unwrap_err(),
            BookXmlError::MissingScorePageField {
                score_index: 0,
                field: "book/score/page/@sheet-number",
            }
        );
        assert_eq!(
            BookXml::parse(br#"<book><score><page sheet-number="1"/></score></book>"#).unwrap_err(),
            BookXmlError::MissingScorePageField {
                score_index: 0,
                field: "book/score/page/@sheet-page-id",
            }
        );

        for (field, attrs, value) in [
            (
                "book/score/page/@sheet-number",
                "sheet-number=\"0\" sheet-page-id=\"1\"",
                "0",
            ),
            (
                "book/score/page/@sheet-page-id",
                "sheet-number=\"1\" sheet-page-id=\"2147483648\"",
                "2147483648",
            ),
        ] {
            let xml = format!("<book><score><page {attrs}/></score></book>");
            assert_eq!(
                BookXml::parse(xml).unwrap_err(),
                BookXmlError::InvalidScorePageInteger {
                    score_index: 0,
                    field,
                    value: value.to_owned(),
                }
            );
        }
    }

    #[test]
    fn rejects_duplicate_or_nonempty_score_page_links() {
        assert_eq!(
            BookXml::parse(
                br#"<book><score><page sheet-number="1" sheet-page-id="2"/><page sheet-number="1" sheet-page-id="2"/></score></book>"#
            )
            .unwrap_err(),
            BookXmlError::DuplicateScorePage {
                score_index: 0,
                sheet_number: 1,
                sheet_page_id: 2,
            }
        );

        for content in ["text", "<![CDATA[]]>", "&#32;", "<future/>"] {
            let xml = format!(
                "<book><score><page sheet-number=\"1\" sheet-page-id=\"2\">{content}</page></score></book>"
            );
            assert_eq!(
                BookXml::parse(xml).unwrap_err(),
                BookXmlError::UnexpectedScorePageContent {
                    score_index: 0,
                    sheet_number: 1,
                    sheet_page_id: 2,
                }
            );
        }
    }

    #[test]
    fn rejects_duplicate_score_attributes_as_malformed_xml() {
        for xml in [
            br#"<book><score logicals-locked="true" logicals-locked="false"/></book>"#.as_slice(),
            br#"<book><score><page sheet-number="1" sheet-number="2" sheet-page-id="1"/></score></book>"#.as_slice(),
        ] {
            assert!(matches!(
                BookXml::parse(xml).unwrap_err(),
                BookXmlError::Malformed { .. }
            ));
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
    fn rejects_invalid_part_scalars() {
        for invalid in ["", "not-a-number", "2147483648", "-2147483649"] {
            let xml = format!(
                "<book><sheet number=\"6\"><page id=\"1\"><system><part logical-id=\"{invalid}\"/></system></page></sheet></book>"
            );
            assert_eq!(
                BookXml::parse(xml).unwrap_err(),
                BookXmlError::InvalidPartInteger {
                    sheet_number: 6,
                    page_id: 1,
                    system_id: 1,
                    field: "sheet/page/system/part/@logical-id",
                    value: invalid.to_owned(),
                }
            );
        }

        for invalid in ["", "TRUE", "yes", "2"] {
            let xml = format!(
                "<book><sheet number=\"6\"><page id=\"1\"><system><part manual=\"{invalid}\"/></system></page></sheet></book>"
            );
            assert_eq!(
                BookXml::parse(xml).unwrap_err(),
                BookXmlError::InvalidPartBoolean {
                    sheet_number: 6,
                    page_id: 1,
                    system_id: 1,
                    field: "sheet/page/system/part/@manual",
                    value: invalid.to_owned(),
                }
            );
        }
    }

    #[test]
    fn rejects_duplicate_part_attributes_as_malformed_xml() {
        for attrs in [
            "name=\"a\" name=\"b\"",
            "logical-id=\"1\" logical-id=\"2\"",
            "manual=\"true\" manual=\"false\"",
        ] {
            let xml = format!(
                "<book><sheet number=\"1\"><page id=\"1\"><system><part {attrs}/></system></page></sheet></book>"
            );
            assert!(matches!(
                BookXml::parse(xml).unwrap_err(),
                BookXmlError::Malformed { .. }
            ));
        }
    }

    #[test]
    fn rejects_missing_and_invalid_staff_config_scalars() {
        assert_eq!(
            BookXml::parse(
                br#"<book><sheet number="2"><page id="1"><system><part><staff-configuration small="true"/></part></system></page></sheet></book>"#
            )
            .unwrap_err(),
            BookXmlError::MissingStaffConfigField {
                sheet_number: 2,
                page_id: 1,
                system_id: 1,
                part_index: 0,
                field: "sheet/page/system/part/staff-configuration/@line-count",
            }
        );

        for (element, field, value) in [
            (
                "<staff-configuration line-count=\"2147483648\"/>",
                "sheet/page/system/part/staff-configuration/@line-count",
                "2147483648",
            ),
            (
                "<line-count>not-a-number</line-count>",
                "sheet/page/system/part/line-count",
                "not-a-number",
            ),
        ] {
            let xml = format!(
                "<book><sheet number=\"2\"><page id=\"1\"><system><part>{element}</part></system></page></sheet></book>"
            );
            assert_eq!(
                BookXml::parse(xml).unwrap_err(),
                BookXmlError::InvalidStaffConfigInteger {
                    sheet_number: 2,
                    page_id: 1,
                    system_id: 1,
                    part_index: 0,
                    field,
                    value: value.to_owned(),
                }
            );
        }

        assert_eq!(
            BookXml::parse(
                br#"<book><sheet number="2"><page id="1"><system><part><staff-configuration line-count="5" small="yes"/></part></system></page></sheet></book>"#
            )
            .unwrap_err(),
            BookXmlError::InvalidStaffConfigBoolean {
                sheet_number: 2,
                page_id: 1,
                system_id: 1,
                part_index: 0,
                field: "sheet/page/system/part/staff-configuration/@small",
                value: "yes".to_owned(),
            }
        );
    }

    #[test]
    fn rejects_duplicate_staff_attributes_and_leaf_markup() {
        assert!(matches!(
            BookXml::parse(
                br#"<book><sheet number="2"><page id="1"><system><part><staff-configuration line-count="5" line-count="1"/></part></system></page></sheet></book>"#
            )
            .unwrap_err(),
            BookXmlError::Malformed { .. }
        ));

        for leaf in [
            "<staff-configuration line-count=\"5\">text</staff-configuration>",
            "<staff-configuration line-count=\"5\"><future/></staff-configuration>",
            "<line-count><![CDATA[5]]></line-count>",
            "<line-count>&#53;</line-count>",
        ] {
            let xml = format!(
                "<book><sheet number=\"2\"><page id=\"1\"><system><part>{leaf}</part></system></page></sheet></book>"
            );
            assert_eq!(
                BookXml::parse(xml).unwrap_err(),
                BookXmlError::UnexpectedStaffConfigContent {
                    sheet_number: 2,
                    page_id: 1,
                    system_id: 1,
                    part_index: 0,
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
    fn rejects_duplicate_or_incomplete_time_rationals() {
        assert_eq!(
            BookXml::parse(
                br#"<book><sheet number="2"><page id="1"><last-time-rational num="3" den="4"/><last-time-rational num="2" den="2"/></page></sheet></book>"#
            )
            .unwrap_err(),
            BookXmlError::DuplicateLastTimeRational {
                sheet_number: 2,
                page_id: 1,
            }
        );
        assert_eq!(
            BookXml::parse(
                br#"<book><sheet number="2"><page id="1"><last-time-rational den="4"/></page></sheet></book>"#
            )
            .unwrap_err(),
            BookXmlError::MissingTimeRationalField {
                sheet_number: 2,
                page_id: 1,
                field: "sheet/page/last-time-rational/@num",
            }
        );
        assert_eq!(
            BookXml::parse(
                br#"<book><sheet number="2"><page id="1"><last-time-rational num="3"/></page></sheet></book>"#
            )
            .unwrap_err(),
            BookXmlError::MissingTimeRationalField {
                sheet_number: 2,
                page_id: 1,
                field: "sheet/page/last-time-rational/@den",
            }
        );
    }

    #[test]
    fn rejects_invalid_time_rational_integers_and_duplicate_attributes() {
        for (field, attribute, invalid) in [
            ("sheet/page/last-time-rational/@num", "num", "2147483648"),
            ("sheet/page/last-time-rational/@den", "den", "not-a-number"),
        ] {
            let attrs = if attribute == "num" {
                format!("num=\"{invalid}\" den=\"4\"")
            } else {
                format!("num=\"3\" den=\"{invalid}\"")
            };
            let xml = format!(
                "<book><sheet number=\"2\"><page id=\"1\"><last-time-rational {attrs}/></page></sheet></book>"
            );
            assert_eq!(
                BookXml::parse(xml).unwrap_err(),
                BookXmlError::InvalidTimeRationalInteger {
                    sheet_number: 2,
                    page_id: 1,
                    field,
                    value: invalid.to_owned(),
                }
            );
        }

        assert!(matches!(
            BookXml::parse(
                br#"<book><sheet number="2"><page id="1"><last-time-rational num="3" num="4" den="4"/></page></sheet></book>"#
            )
            .unwrap_err(),
            BookXmlError::Malformed { .. }
        ));
    }

    #[test]
    fn rejects_time_rational_text_entities_and_markup() {
        for content in [
            "text",
            "&#32;",
            "<![CDATA[]]>",
            "<!--comment-->",
            "<future/>",
        ] {
            let xml = format!(
                "<book><sheet number=\"2\"><page id=\"1\"><last-time-rational num=\"3\" den=\"4\">{content}</last-time-rational></page></sheet></book>"
            );
            assert_eq!(
                BookXml::parse(xml).unwrap_err(),
                BookXmlError::UnexpectedTimeRationalContent {
                    sheet_number: 2,
                    page_id: 1,
                }
            );
        }
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
