//! The file structure: cross-references, object streams, and the page tree.
//!
//! # Why the objects are parsed eagerly
//!
//! PDFBox parses lazily and caches, which it must, because it hands out live
//! `COSBase` handles that can be mutated. Nothing here mutates, so parsing the
//! whole object graph up front buys a much simpler API -- every lookup returns
//! a borrow, no interior mutability, no cache invalidation -- at a cost of one
//! pass over a file the caller has already read into memory. The corpus's
//! largest source is 12 MB.
//!
//! # Where this is deliberately lenient
//!
//! Every leniency below is one PDFBox has and the corpus needs:
//!
//! - **A `/Length` that lies.** Three of the seven corpus sources declare
//!   `/Length 0` on streams that are not empty; PDFBox logs "Suspicious stream
//!   length" and scans for `endstream`. That fallback lives in [`crate::lexer`].
//! - **Bytes before `%PDF-`.** Offsets in the cross-reference table are then
//!   relative to the header rather than to the file, so the header's own offset
//!   is added when the recorded one does not land on the right object.
//! - **Hybrid files.** A classic table may carry `/XRefStm` pointing at a
//!   cross-reference *stream* that covers objects the table does not. Ignoring
//!   it loses every compressed object in the file.
//! - **A cross-reference chain that does not lead to a catalog.** Then the file
//!   is scanned for `obj` keywords, which is PDFBox's `BruteForceParser`. Any
//!   file needing this is damaged, but PDFBox still renders it.
//!
//! What is *not* lenient: an encrypted file is refused rather than guessed at.
//! Even an empty-password file needs the standard security handler, and half a
//! decryption produces plausible garbage instead of an error.

use std::collections::{BTreeMap, BTreeSet};

use crate::error::{Error, Result};
use crate::filter;
use crate::lexer::{Lexer, find, rfind};
use crate::object::{Dictionary, Name, Object, Reference, Stream};

/// How far back from the end of the file `startxref` is looked for.
const TAIL: usize = 2048;

/// How far into the file the `%PDF-` header may be.
const HEADER_WINDOW: usize = 1024;

/// Where an object lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Location {
    /// At a byte offset, written out as `n g obj ... endobj`.
    Offset(usize),
    /// Inside an object stream, at an index within it.
    Compressed {
        /// The object stream's own object number.
        container: u32,
        /// This object's position in the container's index.
        index: u32,
    },
}

/// A rectangle, in the `float` PDFBox stores it as.
///
/// `PDRectangle` holds floats, and every coordinate the renderer derives comes
/// through them. A 595.20001220703125 page box is a float that reads as
/// 595.2 in the file, and computing in `double` from the decimal text gives a
/// different last bit -- which is worth a whole row of destination pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rectangle {
    /// `/MediaBox` entry 0.
    pub lower_left_x: f32,
    /// Entry 1.
    pub lower_left_y: f32,
    /// Entry 2.
    pub upper_right_x: f32,
    /// Entry 3.
    pub upper_right_y: f32,
}

impl Rectangle {
    /// US Letter, which is what `PDPage` falls back to.
    pub const LETTER: Self = Self {
        lower_left_x: 0.0,
        lower_left_y: 0.0,
        upper_right_x: 612.0,
        upper_right_y: 792.0,
    };

    /// The width, as `PDRectangle.getWidth` computes it.
    #[must_use]
    pub fn width(self) -> f32 {
        self.upper_right_x - self.lower_left_x
    }

    /// The height, as `PDRectangle.getHeight` computes it.
    #[must_use]
    pub fn height(self) -> f32 {
        self.upper_right_y - self.lower_left_y
    }
}

/// A page, with its inheritable attributes already resolved up the tree.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    /// The page's own dictionary.
    pub dictionary: Dictionary,
    /// `/Resources`, inherited if the page does not carry its own.
    pub resources: Dictionary,
    /// `/MediaBox`, inherited, defaulting to US Letter.
    pub media_box: Rectangle,
    /// `/CropBox`, inherited, defaulting to the media box.
    pub crop_box: Rectangle,
    /// `/Rotate`, inherited, defaulting to zero and left unnormalised.
    pub rotation: i64,
}

/// A parsed PDF.
#[derive(Debug, Clone)]
pub struct Document {
    objects: BTreeMap<u32, Object>,
    trailer: Dictionary,
    pages: Vec<u32>,
}

impl Document {
    /// Parses `data`.
    ///
    /// # Errors
    ///
    /// [`Error::MissingHeader`] if there is no `%PDF-` marker,
    /// [`Error::Encrypted`] for an encrypted file, [`Error::MissingCatalog`] or
    /// [`Error::BadPageTree`] if the structure does not lead to any pages.
    pub fn parse(data: &[u8]) -> Result<Self> {
        let origin =
            find(&data[..data.len().min(HEADER_WINDOW)], b"%PDF-").ok_or(Error::MissingHeader)?;

        let (mut locations, mut trailer) = read_cross_references(data, origin);
        if !leads_to_a_catalog(data, &locations, &trailer) {
            let (scanned, recovered) = scan_for_objects(data);
            // A brute-force scan sees the newest definition of every object,
            // so it takes precedence over a table that did not work.
            locations = scanned;
            if !trailer.contains_key(&Name::new("Root")) {
                trailer = recovered;
            }
        }
        if trailer.contains_key(&Name::new("Encrypt")) {
            return Err(Error::Encrypted);
        }

        let mut objects = read_direct_objects(data, &locations);
        expand_object_streams(&locations, &mut objects);

        // A file whose trailer was lost still has its catalog somewhere.
        if !trailer.contains_key(&Name::new("Root"))
            && let Some(number) = objects.iter().find_map(|(number, object)| {
                let dictionary = object.dictionary()?;
                (dictionary.get(&Name::new("Type"))? == &Object::Name(Name::new("Catalog")))
                    .then_some(*number)
            })
        {
            trailer.insert(Name::new("Root"), Object::Reference(Reference::new(number)));
        }

        let mut document = Self {
            objects,
            trailer,
            pages: Vec::new(),
        };
        document.pages = document.collect_pages()?;
        Ok(document)
    }

    /// The trailer dictionary.
    #[must_use]
    pub const fn trailer(&self) -> &Dictionary {
        &self.trailer
    }

    /// The document catalog.
    ///
    /// # Errors
    ///
    /// [`Error::MissingCatalog`] if `/Root` is absent or is not a dictionary.
    pub fn catalog(&self) -> Result<&Dictionary> {
        self.get(&self.trailer, "Root")
            .and_then(Object::dictionary)
            .ok_or(Error::MissingCatalog)
    }

    /// How many pages the document has.
    #[must_use]
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// The object `reference` names, or [`Object::Null`] if it names nothing.
    ///
    /// A dangling reference is null rather than an error, which is what the
    /// specification requires and what PDFBox does.
    #[must_use]
    pub fn object(&self, reference: Reference) -> &Object {
        self.objects.get(&reference.number).unwrap_or(&Object::Null)
    }

    /// Follows `object` through as many references as it takes.
    #[must_use]
    pub fn resolve<'a>(&'a self, object: &'a Object) -> &'a Object {
        let mut current = object;
        let mut seen = BTreeSet::new();
        while let Object::Reference(reference) = current {
            if !seen.insert(reference.number) {
                return &Object::Null;
            }
            current = self.object(*reference);
        }
        current
    }

    /// Reads `key` from `dictionary`, resolving what it finds.
    #[must_use]
    pub fn get<'a>(&'a self, dictionary: &'a Dictionary, key: &str) -> Option<&'a Object> {
        let value = self.resolve(dictionary.get(&Name::new(key))?);
        (!value.is_null()).then_some(value)
    }

    /// A stream's bytes with its filter chain applied.
    ///
    /// # Errors
    ///
    /// Whatever [`filter::decode`] reports, which for the corpus means an
    /// unimplemented image filter.
    pub fn stream_data(&self, stream: &Stream) -> Result<Vec<u8>> {
        filter::decode(&self.flatten(&stream.dictionary), &stream.data)
    }

    /// A copy of `dictionary` with its values resolved one level deep.
    ///
    /// `/Filter` and `/DecodeParms` are routinely indirect, and the filter
    /// layer has no document to resolve them with.
    #[must_use]
    pub fn flatten(&self, dictionary: &Dictionary) -> Dictionary {
        dictionary
            .iter()
            .map(|(key, value)| {
                let resolved = match self.resolve(value) {
                    Object::Array(entries) => Object::Array(
                        entries
                            .iter()
                            .map(|entry| self.resolve(entry).clone())
                            .collect(),
                    ),
                    other => other.clone(),
                };
                (key.clone(), resolved)
            })
            .collect()
    }

    /// The page at `index`, zero based.
    ///
    /// # Errors
    ///
    /// [`Error::NoSuchPage`] past the end of the document.
    pub fn page(&self, index: usize) -> Result<Page> {
        let number = *self.pages.get(index).ok_or(Error::NoSuchPage { index })?;
        let dictionary = self
            .object(Reference::new(number))
            .dictionary()
            .ok_or(Error::BadPageTree)?;

        let media_box = self
            .inherited(dictionary, "MediaBox")
            .and_then(|object| self.rectangle(object))
            .unwrap_or(Rectangle::LETTER);
        Ok(Page {
            resources: self
                .inherited(dictionary, "Resources")
                .and_then(Object::dictionary)
                .cloned()
                .unwrap_or_default(),
            crop_box: self
                .inherited(dictionary, "CropBox")
                .and_then(|object| self.rectangle(object))
                .unwrap_or(media_box),
            media_box,
            rotation: self
                .inherited(dictionary, "Rotate")
                .and_then(Object::as_i64)
                .unwrap_or(0),
            dictionary: dictionary.clone(),
        })
    }

    /// Reads `key` from `dictionary` or, failing that, from its ancestors.
    #[must_use]
    pub fn inherited<'a>(&'a self, dictionary: &'a Dictionary, key: &str) -> Option<&'a Object> {
        let mut current = dictionary;
        let mut seen = BTreeSet::new();
        loop {
            if let Some(value) = self.get(current, key) {
                return Some(value);
            }
            let parent = self.get(current, "Parent")?.dictionary()?;
            // A `/Parent` cycle is rare but a stack overflow is unrecoverable.
            if !seen.insert(std::ptr::from_ref(parent) as usize) {
                return None;
            }
            current = parent;
        }
    }

    /// A four-number array as a [`Rectangle`], through `float` as PDFBox does.
    fn rectangle(&self, object: &Object) -> Option<Rectangle> {
        let entries = object.array()?;
        if entries.len() < 4 {
            return None;
        }
        let mut values = [0f32; 4];
        for (slot, entry) in values.iter_mut().zip(entries) {
            *slot = self.resolve(entry).as_f64()? as f32;
        }
        Some(Rectangle {
            lower_left_x: values[0],
            lower_left_y: values[1],
            upper_right_x: values[2],
            upper_right_y: values[3],
        })
    }

    /// Walks the page tree, depth first, collecting leaf object numbers.
    fn collect_pages(&self) -> Result<Vec<u32>> {
        let root = self
            .get(self.catalog()?, "Pages")
            .and_then(Object::as_reference)
            .or_else(|| {
                self.catalog()
                    .ok()?
                    .get(&Name::new("Pages"))
                    .and_then(Object::as_reference)
            });
        let mut pages = Vec::new();
        let mut seen = BTreeSet::new();
        if let Some(root) = root {
            self.walk_page_tree(root.number, &mut pages, &mut seen, 0);
        }
        if pages.is_empty() {
            // A broken tree still has page objects in it. PDFBox recovers the
            // same way, and file order is the only ordering left.
            pages = self
                .objects
                .iter()
                .filter(|(_, object)| {
                    object.dictionary().is_some_and(|dictionary| {
                        dictionary.get(&Name::new("Type")) == Some(&Object::Name(Name::new("Page")))
                    })
                })
                .map(|(number, _)| *number)
                .collect();
        }
        if pages.is_empty() {
            return Err(Error::BadPageTree);
        }
        Ok(pages)
    }

    fn walk_page_tree(
        &self,
        number: u32,
        pages: &mut Vec<u32>,
        seen: &mut BTreeSet<u32>,
        depth: usize,
    ) {
        if depth > 64 || !seen.insert(number) {
            return;
        }
        let Some(dictionary) = self.object(Reference::new(number)).dictionary() else {
            return;
        };
        let Some(kids) = self.get(dictionary, "Kids").and_then(Object::array) else {
            // No `/Kids` makes it a leaf, whatever `/Type` claims: files do
            // omit `/Type` on pages, and PDFBox treats the structure as
            // authoritative over the label.
            pages.push(number);
            return;
        };
        for kid in kids {
            if let Some(reference) = kid.as_reference() {
                self.walk_page_tree(reference.number, pages, seen, depth + 1);
            }
        }
    }
}

/// Parses every object that sits at its own offset.
fn read_direct_objects(data: &[u8], locations: &BTreeMap<u32, Location>) -> BTreeMap<u32, Object> {
    let mut objects = BTreeMap::new();
    for (&number, location) in locations {
        let Location::Offset(offset) = *location else {
            continue;
        };
        if let Some(object) = parse_indirect(data, offset, Some(number), locations) {
            objects.insert(number, object);
        }
    }
    objects
}

/// Parses `n g obj ... endobj` at `offset`.
///
/// `expected` is the object number the cross-reference table promised. A
/// mismatch means the table is wrong about this object, and the parse is
/// abandoned rather than trusted.
fn parse_indirect(
    data: &[u8],
    offset: usize,
    expected: Option<u32>,
    locations: &BTreeMap<u32, Location>,
) -> Option<Object> {
    if offset >= data.len() {
        return None;
    }
    let mut lexer = Lexer::at(data, offset);
    let number = lexer.integer().ok()?;
    let _generation = lexer.integer().ok()?;
    lexer.skip_space();
    if !lexer.consume("obj") {
        return None;
    }
    if expected.is_some_and(|expected| i64::from(expected) != number) {
        return None;
    }
    let mut lengths = |reference: Reference| shallow_integer(data, locations, reference);
    lexer.object_with(&mut lengths).ok()
}

/// Reads an integer that an object holds, without parsing anything else.
///
/// This exists for `/Length`, which is an indirect reference often enough that
/// streams cannot be parsed without it, and which always points at a bare
/// integer. Resolving it this way keeps stream parsing non-recursive.
fn shallow_integer(
    data: &[u8],
    locations: &BTreeMap<u32, Location>,
    reference: Reference,
) -> Option<i64> {
    let Location::Offset(offset) = *locations.get(&reference.number)? else {
        return None;
    };
    let mut lexer = Lexer::at(data, offset);
    let number = lexer.integer().ok()?;
    if number != i64::from(reference.number) {
        return None;
    }
    lexer.integer().ok()?;
    lexer.skip_space();
    lexer.consume("obj").then(|| lexer.integer().ok())?
}

/// Expands every object stream, adding the objects it carries.
fn expand_object_streams(locations: &BTreeMap<u32, Location>, objects: &mut BTreeMap<u32, Object>) {
    let containers: BTreeSet<u32> = locations
        .values()
        .filter_map(|location| match location {
            Location::Compressed { container, .. } => Some(*container),
            Location::Offset(_) => None,
        })
        .collect();
    for container in containers {
        let Some(Object::Stream(stream)) = objects.get(&container) else {
            continue;
        };
        // An object stream's own dictionary never has an indirect `/Length`
        // left to resolve by this point, and its filter is Flate in practice.
        let Ok(payload) = filter::decode(&stream.dictionary, &stream.data) else {
            continue;
        };
        let count = stream
            .dictionary
            .get(&Name::new("N"))
            .and_then(Object::as_i64)
            .unwrap_or(0);
        let first = stream
            .dictionary
            .get(&Name::new("First"))
            .and_then(Object::as_i64)
            .unwrap_or(0);
        let Ok(first) = usize::try_from(first) else {
            continue;
        };

        let mut index = Lexer::new(&payload);
        let mut entries = Vec::new();
        for _ in 0..count {
            let Ok(number) = index.integer() else { break };
            let Ok(offset) = index.integer() else { break };
            entries.push((number, offset));
        }
        for (number, offset) in entries {
            let Ok(number) = u32::try_from(number) else {
                continue;
            };
            // Only take an object the table actually places in this container:
            // a stale object stream may still hold an older copy.
            if locations.get(&number)
                != Some(&Location::Compressed {
                    container,
                    index: 0,
                })
                && !matches!(
                    locations.get(&number),
                    Some(Location::Compressed { container: c, .. }) if *c == container
                )
            {
                continue;
            }
            let Ok(offset) = usize::try_from(offset) else {
                continue;
            };
            let Some(at) = first.checked_add(offset) else {
                continue;
            };
            if at >= payload.len() {
                continue;
            }
            if let Ok(object) = Lexer::at(&payload, at).object() {
                objects.insert(number, object);
            }
        }
    }
}

/// Walks the cross-reference chain from `startxref` backwards.
///
/// Returns the merged table and the newest trailer. Entries and trailer keys
/// from earlier (newer) sections win, because the chain runs newest first.
fn read_cross_references(data: &[u8], origin: usize) -> (BTreeMap<u32, Location>, Dictionary) {
    let mut locations = BTreeMap::new();
    let mut trailer = Dictionary::new();
    let Some(start) = read_startxref(data) else {
        return (locations, trailer);
    };

    let mut pending = vec![start];
    let mut seen = BTreeSet::new();
    while let Some(offset) = pending.pop() {
        if !seen.insert(offset) || offset >= data.len() {
            continue;
        }
        let Some(section) = read_section(data, offset, origin) else {
            continue;
        };
        for (number, location) in section.locations {
            locations.entry(number).or_insert(location);
        }
        for (key, value) in &section.trailer {
            trailer.entry(key.clone()).or_insert_with(|| value.clone());
        }
        // `/XRefStm` first: in a hybrid file it holds the objects the classic
        // table cannot express, and it describes the same revision.
        for key in ["XRefStm", "Prev"] {
            if let Some(Object::Integer(next)) = section.trailer.get(&Name::new(key))
                && let Ok(next) = usize::try_from(*next)
            {
                pending.push(next);
            }
        }
    }
    (locations, trailer)
}

/// One cross-reference section: its entries and its trailer.
struct Section {
    locations: BTreeMap<u32, Location>,
    trailer: Dictionary,
}

fn read_section(data: &[u8], offset: usize, origin: usize) -> Option<Section> {
    read_section_at(data, offset).or_else(|| {
        // Junk before the header shifts every recorded offset by its length.
        (origin != 0).then(|| read_section_at(data, offset + origin))?
    })
}

fn read_section_at(data: &[u8], offset: usize) -> Option<Section> {
    if offset >= data.len() {
        return None;
    }
    let mut lexer = Lexer::at(data, offset);
    lexer.skip_space();
    if lexer.consume("xref") {
        return read_classic_section(&mut lexer);
    }
    read_stream_section(data, offset)
}

/// The classic table: subsections of twenty-byte entries, then `trailer`.
fn read_classic_section(lexer: &mut Lexer<'_>) -> Option<Section> {
    let mut locations = BTreeMap::new();
    loop {
        lexer.skip_space();
        if lexer.looking_at("trailer") {
            lexer.seek(lexer.position() + "trailer".len());
            let trailer = lexer.object().ok()?.dictionary()?.clone();
            return Some(Section { locations, trailer });
        }
        let start = lexer.integer().ok()?;
        let count = lexer.integer().ok()?;
        let Ok(start) = u32::try_from(start) else {
            return None;
        };
        for index in 0..count.max(0) {
            lexer.skip_space();
            let offset = lexer.integer().ok()?;
            let _generation = lexer.integer().ok()?;
            lexer.skip_space();
            let kind = lexer.peek()?;
            lexer.seek(lexer.position() + 1);
            if kind != b'n' {
                continue;
            }
            let Ok(offset) = usize::try_from(offset) else {
                continue;
            };
            // Object zero is the free-list head and never has a body.
            let Some(number) = start
                .checked_add(index as u32)
                .filter(|number| *number != 0)
            else {
                continue;
            };
            locations.entry(number).or_insert(Location::Offset(offset));
        }
    }
}

/// A cross-reference stream: `/W` field widths over `/Index` ranges.
fn read_stream_section(data: &[u8], offset: usize) -> Option<Section> {
    let object = parse_indirect(data, offset, None, &BTreeMap::new())?;
    let stream = object.stream()?;
    let payload = filter::decode(&stream.dictionary, &stream.data).ok()?;

    let widths: Vec<usize> = stream
        .dictionary
        .get(&Name::new("W"))?
        .array()?
        .iter()
        .map(|entry| entry.as_i64().unwrap_or(0).clamp(0, 8) as usize)
        .collect();
    if widths.len() < 3 {
        return None;
    }
    let size = stream
        .dictionary
        .get(&Name::new("Size"))
        .and_then(Object::as_i64)
        .unwrap_or(0);
    let ranges: Vec<(i64, i64)> = match stream.dictionary.get(&Name::new("Index")) {
        Some(Object::Array(entries)) => entries
            .chunks(2)
            .filter_map(|pair| match pair {
                [start, count] => Some((start.as_i64()?, count.as_i64()?)),
                _ => None,
            })
            .collect(),
        _ => vec![(0, size)],
    };

    let entry_width: usize = widths.iter().sum();
    if entry_width == 0 {
        return None;
    }
    let mut locations = BTreeMap::new();
    let mut at = 0;
    for (start, count) in ranges {
        for index in 0..count.max(0) {
            if at + entry_width > payload.len() {
                break;
            }
            let mut fields = [0u64; 3];
            for (field, width) in fields.iter_mut().zip(&widths) {
                for _ in 0..*width {
                    *field = *field << 8 | u64::from(payload[at]);
                    at += 1;
                }
            }
            // A zero-width type field means type 1, per the specification.
            let kind = if widths[0] == 0 { 1 } else { fields[0] };
            let Ok(number) = u32::try_from(start + index) else {
                continue;
            };
            if number == 0 {
                continue;
            }
            match kind {
                1 => {
                    if let Ok(offset) = usize::try_from(fields[1]) {
                        locations.entry(number).or_insert(Location::Offset(offset));
                    }
                }
                2 => {
                    if let (Ok(container), Ok(index)) =
                        (u32::try_from(fields[1]), u32::try_from(fields[2]))
                    {
                        locations
                            .entry(number)
                            .or_insert(Location::Compressed { container, index });
                    }
                }
                _ => {}
            }
        }
    }
    Some(Section {
        locations,
        trailer: stream.dictionary.clone(),
    })
}

/// Reads the offset `startxref` names, from the tail of the file.
fn read_startxref(data: &[u8]) -> Option<usize> {
    let window = data.len().saturating_sub(TAIL);
    let at = rfind(&data[window..], b"startxref")? + window;
    let mut lexer = Lexer::at(data, at + "startxref".len());
    usize::try_from(lexer.integer().ok()?).ok()
}

/// Scans the whole file for `obj` keywords, PDFBox's last resort.
///
/// Later definitions win, which is what an incrementally updated file needs.
fn scan_for_objects(data: &[u8]) -> (BTreeMap<u32, Location>, Dictionary) {
    let mut locations = BTreeMap::new();
    let mut at = 0;
    while let Some(found) = find(&data[at..], b"obj") {
        let keyword = at + found;
        at = keyword + 3;
        // Walk back over `g` and `n`, which must be digits with whitespace
        // between them, to find where the object actually starts.
        let Some(start) = start_of_indirect(data, keyword) else {
            continue;
        };
        let mut lexer = Lexer::at(data, start);
        let Ok(number) = lexer.integer() else {
            continue;
        };
        if let Ok(number) = u32::try_from(number) {
            locations.insert(number, Location::Offset(start));
        }
    }
    let mut trailer = Dictionary::new();
    if let Some(found) = rfind(data, b"trailer") {
        let mut lexer = Lexer::at(data, found + "trailer".len());
        if let Ok(object) = lexer.object()
            && let Some(dictionary) = object.dictionary()
        {
            trailer = dictionary.clone();
        }
    }
    (locations, trailer)
}

/// Where the `n g obj` that ends at `keyword` begins.
fn start_of_indirect(data: &[u8], keyword: usize) -> Option<usize> {
    let mut at = keyword;
    for _ in 0..2 {
        while at > 0 && matches!(data[at - 1], b' ' | b'\r' | b'\n' | b'\t' | b'\0' | 0x0c) {
            at -= 1;
        }
        let end = at;
        while at > 0 && data[at - 1].is_ascii_digit() {
            at -= 1;
        }
        if at == end {
            return None;
        }
    }
    Some(at)
}

/// Whether the table found so far actually resolves to a catalog.
///
/// A cross-reference table can parse cleanly and still be wrong -- offsets off
/// by the length of a prepended banner, say -- and the symptom is a `/Root`
/// that resolves to nothing. Checking here is what decides between trusting the
/// table and scanning the file.
fn leads_to_a_catalog(
    data: &[u8],
    locations: &BTreeMap<u32, Location>,
    trailer: &Dictionary,
) -> bool {
    let Some(root) = trailer
        .get(&Name::new("Root"))
        .and_then(Object::as_reference)
    else {
        return false;
    };
    match locations.get(&root.number) {
        Some(Location::Offset(offset)) => {
            parse_indirect(data, *offset, Some(root.number), locations)
                .as_ref()
                .and_then(Object::dictionary)
                .is_some()
        }
        // A catalog inside an object stream cannot be checked without decoding
        // it, and a file that puts it there has a cross-reference stream, which
        // is not the structure that goes wrong this way.
        Some(Location::Compressed { .. }) => true,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal one-page file with a classic cross-reference table.
    fn minimal() -> Vec<u8> {
        let mut body = String::new();
        let mut offsets = Vec::new();
        let push = |body: &mut String, offsets: &mut Vec<usize>, text: &str| {
            offsets.push(body.len());
            body.push_str(text);
        };
        body.push_str("%PDF-1.4\n");
        push(
            &mut body,
            &mut offsets,
            "1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
        );
        push(
            &mut body,
            &mut offsets,
            "2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 595.2 841.8] >>\nendobj\n",
        );
        push(
            &mut body,
            &mut offsets,
            "3 0 obj\n<< /Type /Page /Parent 2 0 R >>\nendobj\n",
        );
        let xref = body.len();
        body.push_str("xref\n0 4\n0000000000 65535 f \n");
        for offset in &offsets {
            body.push_str(&format!("{offset:010} 00000 n \n"));
        }
        body.push_str(&format!(
            "trailer\n<< /Size 4 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n"
        ));
        body.into_bytes()
    }

    #[test]
    fn reads_a_classic_table_and_its_page_tree() {
        let document = Document::parse(&minimal()).expect("parses");
        assert_eq!(document.page_count(), 1);
        let page = document.page(0).expect("a page");
        assert_eq!(page.rotation, 0);
        // Inherited from /Pages, and through float, so 841.8 is 841.79999.
        assert!((page.media_box.height() - 841.8).abs() < 1e-3);
        assert_eq!(page.media_box.upper_right_x, 595.2f32);
    }

    #[test]
    fn recovers_from_offsets_that_are_all_wrong() {
        // Prepending a banner shifts every recorded offset, which is exactly
        // the shape of file the header-relative retry exists for.
        let mut shifted = b"junk before the header\n".to_vec();
        shifted.extend_from_slice(&minimal());
        let document = Document::parse(&shifted).expect("parses");
        assert_eq!(document.page_count(), 1);
    }

    #[test]
    fn falls_back_to_scanning_when_the_table_is_useless() {
        let mut broken = minimal();
        // Point startxref at nothing at all.
        let text = String::from_utf8(broken.clone()).expect("ascii");
        let at = text.rfind("startxref\n").expect("a startxref") + "startxref\n".len();
        broken.splice(at..at + 1, b"9".iter().copied());
        let document = Document::parse(&broken).expect("parses");
        assert_eq!(document.page_count(), 1);
    }

    #[test]
    fn refuses_an_encrypted_file() {
        let text = String::from_utf8(minimal()).expect("ascii");
        let encrypted = text.replace("/Size 4 /Root 1 0 R", "/Size 4 /Root 1 0 R /Encrypt 9 0 R");
        assert_eq!(
            Document::parse(encrypted.as_bytes()).expect_err("refuses"),
            Error::Encrypted
        );
    }
}
