// SPDX-License-Identifier: AGPL-3.0-or-later

//! A conservative persistence boundary for Audiveris `.omr` files.
//!
//! Audiveris books are ZIP archives whose root contains `book.xml`. This crate
//! keeps XML opaque unless a caller explicitly requests a narrow read-only view.
//! It validates every member name, retains every member's uncompressed bytes
//! (including unknown members), and can emit a content-equivalent archive. Typed
//! views never replace the retained bytes. The crate never extracts an archive
//! to the filesystem.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use std::path::Path;

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub mod beam_inters;
pub mod beam_parameters;
pub mod beam_recognizer;
pub mod beams_step;
pub mod black_head_sizer;
pub mod brace_filament;
pub mod brace_portions;
pub mod brace_sig;
pub mod bracket_serif;
pub mod chords_step;
pub mod clef_classifier;
pub mod clef_column;
pub mod clef_parameters;
pub mod cue_beams_step;
pub mod curves_step;
pub mod grid_executor;
pub mod head_glyph_retrieval;
pub mod head_pair_predicates;
pub mod head_purge;
pub mod head_range_postprocess;
pub mod head_scanner;
pub mod head_scanner_geometry;
pub mod head_scanner_slices;
pub mod head_seed_tally_analysis;
pub mod head_small_beam_purge;
pub mod head_template;
pub mod head_template_catalog;
pub mod head_template_overlap;
pub mod header_builder;
pub mod header_time_builder;
pub mod header_time_column;
pub mod headers_step;
pub mod heads_post_range_corpus;
pub mod heads_step;
pub mod key_classifier;
pub mod key_column;
pub mod key_parameters;
pub mod key_peaks;
pub mod ledgers_step;
pub mod links_step;
pub mod measures_step;
pub mod native_headers;
pub mod native_heads;
pub mod native_heads_bar_slices;
pub mod native_heads_competitor_slices;
pub mod native_heads_competitors;
pub mod native_heads_epilog;
pub mod native_heads_obstacles;
pub mod native_heads_range_glyphs;
pub mod native_heads_range_lookup;
pub mod native_heads_scanner;
pub mod native_heads_scanner_pools;
pub mod native_heads_seed_glyphs;
pub mod native_heads_seed_lookup;
pub mod native_heads_small_beams;
pub mod native_heads_staff_epilog;
pub mod native_ledgers;
pub mod native_stem_seeds;
pub mod native_stems_head_corners;
pub mod native_stems_head_seeds;
pub mod native_stems_head_stumps;
pub mod page_step;
pub mod production_stages;
pub mod raw_ledger_filter;
pub mod raw_projector_adapter;
pub mod recognize;
pub mod reduction_step;
pub mod report;
pub mod rhythms_step;
pub mod score_update;
pub mod sheet_xml;
pub mod staff_header;
pub mod stem_seeds_step;
pub mod stems_step;
pub mod symbols_step;
pub mod system_grouping;
pub mod texts_step;
pub mod time_classifier;
pub mod xml;

use sheet_xml::{SheetXml, SheetXmlError};
use xml::{BookXml, BookXmlError};

/// The required root member written by Audiveris.
pub const BOOK_XML_PATH: &str = "book.xml";

/// Resource limits applied before and while member contents are decompressed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveLimits {
    /// Maximum number of ZIP members.
    pub max_entries: usize,
    /// Maximum uncompressed size of one member.
    pub max_entry_bytes: u64,
    /// Maximum total uncompressed size of all members.
    pub max_total_bytes: u64,
}

impl Default for ArchiveLimits {
    fn default() -> Self {
        Self {
            max_entries: 100_000,
            max_entry_bytes: 512 * 1024 * 1024,
            max_total_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

/// Read-only metadata for one validated ZIP member.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntryInventory {
    path: String,
    is_directory: bool,
    compressed_size: u64,
    uncompressed_size: u64,
    crc32: u32,
}

impl EntryInventory {
    /// Validated, relative, UTF-8 ZIP member path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Whether this member is a directory marker.
    #[must_use]
    pub const fn is_directory(&self) -> bool {
        self.is_directory
    }

    /// Compressed size recorded in the source archive.
    #[must_use]
    pub const fn compressed_size(&self) -> u64 {
        self.compressed_size
    }

    /// Uncompressed size recorded in the source archive.
    #[must_use]
    pub const fn uncompressed_size(&self) -> u64 {
        self.uncompressed_size
    }

    /// CRC-32 recorded in the source archive.
    #[must_use]
    pub const fn crc32(&self) -> u32 {
        self.crc32
    }
}

#[derive(Clone, Debug)]
struct OmrEntry {
    inventory: EntryInventory,
    bytes: Vec<u8>,
}

/// A validated `.omr` archive held without interpreting its musical content.
#[derive(Clone, Debug)]
pub struct OmrArchive {
    entries: Vec<OmrEntry>,
    book_xml_index: usize,
}

/// Result of resolving a requested sheet through a direct `book.xml` stub.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SheetXmlMember {
    /// No direct book sheet stub declares the requested one-based number.
    Undeclared,
    /// The stub exists, but its exact conventional archive member is absent.
    Missing {
        /// Conventional `sheet#N/sheet#N.xml` member path from the stub.
        archive_path: String,
    },
    /// The conventional member exists and produced a fresh lossless typed view.
    Present {
        /// Exact member path used for the lookup.
        archive_path: String,
        /// Narrow typed view retaining the member's original bytes.
        xml: SheetXml,
    },
}

/// Failure to parse a fresh archive-level typed XML view.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArchiveXmlError {
    /// The required root `book.xml` member is malformed or semantically invalid.
    Book(BookXmlError),
    /// A resolved per-sheet member is malformed or semantically invalid.
    Sheet {
        /// Exact conventional member path that was parsed.
        archive_path: String,
        /// Per-sheet parser failure.
        source: SheetXmlError,
    },
}

impl fmt::Display for ArchiveXmlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Book(error) => write!(formatter, "invalid archive book XML: {error}"),
            Self::Sheet {
                archive_path,
                source,
            } => write!(
                formatter,
                "invalid sheet XML member {archive_path:?}: {source}"
            ),
        }
    }
}

impl Error for ArchiveXmlError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Book(error) => Some(error),
            Self::Sheet { source, .. } => Some(source),
        }
    }
}

impl OmrArchive {
    /// Open a `.omr` archive from a file using [`ArchiveLimits::default`].
    pub fn open(path: impl AsRef<Path>) -> Result<Self, OmrError> {
        Self::read_from(File::open(path)?)
    }

    /// Read an archive from a seekable source using [`ArchiveLimits::default`].
    pub fn read_from<R: Read + Seek>(reader: R) -> Result<Self, OmrError> {
        Self::read_from_with_limits(reader, ArchiveLimits::default())
    }

    /// Read an archive with caller-supplied decompression limits.
    pub fn read_from_with_limits<R: Read + Seek>(
        mut reader: R,
        limits: ArchiveLimits,
    ) -> Result<Self, OmrError> {
        let central_directory_start = {
            let probe = ZipArchive::new(&mut reader)?;
            probe.central_directory_start()
        };
        let central_entries = validate_unique_central_paths(&mut reader, central_directory_start)?;
        if central_entries > limits.max_entries {
            return Err(OmrError::LimitExceeded {
                resource: "entry count",
                limit: u64::try_from(limits.max_entries).unwrap_or(u64::MAX),
                actual: u64::try_from(central_entries).unwrap_or(u64::MAX),
            });
        }
        reader.rewind()?;
        let mut zip = ZipArchive::new(reader)?;

        let mut entries = Vec::with_capacity(zip.len());
        let mut seen = HashSet::with_capacity(zip.len());
        let mut total_size = 0_u64;
        let mut book_xml_index = None;

        for index in 0..zip.len() {
            let mut member = zip.by_index(index)?;
            let path = validated_member_path(member.name_raw())?;

            if !seen.insert(path.clone()) {
                return Err(OmrError::DuplicatePath(path));
            }

            let uncompressed_size = member.size();
            if uncompressed_size > limits.max_entry_bytes {
                return Err(OmrError::LimitExceeded {
                    resource: "entry bytes",
                    limit: limits.max_entry_bytes,
                    actual: uncompressed_size,
                });
            }
            total_size =
                total_size
                    .checked_add(uncompressed_size)
                    .ok_or(OmrError::LimitExceeded {
                        resource: "total bytes",
                        limit: limits.max_total_bytes,
                        actual: u64::MAX,
                    })?;
            if total_size > limits.max_total_bytes {
                return Err(OmrError::LimitExceeded {
                    resource: "total bytes",
                    limit: limits.max_total_bytes,
                    actual: total_size,
                });
            }

            let is_directory = member.is_dir();
            let inventory = EntryInventory {
                path: path.clone(),
                is_directory,
                compressed_size: member.compressed_size(),
                uncompressed_size,
                crc32: member.crc32(),
            };

            let bytes = if is_directory {
                Vec::new()
            } else {
                read_limited(&mut member, limits.max_entry_bytes)?
            };

            if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != uncompressed_size {
                return Err(OmrError::SizeMismatch {
                    path,
                    declared: uncompressed_size,
                    decoded: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                });
            }

            if inventory.path == BOOK_XML_PATH && !is_directory {
                book_xml_index = Some(entries.len());
            }
            entries.push(OmrEntry { inventory, bytes });
        }

        let book_xml_index = book_xml_index.ok_or(OmrError::MissingBookXml)?;
        Ok(Self {
            entries,
            book_xml_index,
        })
    }

    /// Read an archive from in-memory ZIP bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, OmrError> {
        Self::read_from(Cursor::new(bytes))
    }

    /// Inventory of all source members, in source order.
    pub fn inventory(&self) -> impl ExactSizeIterator<Item = &EntryInventory> {
        self.entries.iter().map(|entry| &entry.inventory)
    }

    /// Raw, uninterpreted bytes of the required root `book.xml` member.
    #[must_use]
    pub fn book_xml(&self) -> &[u8] {
        &self.entries[self.book_xml_index].bytes
    }

    /// Parse a fresh narrow view of `book.xml` while retaining its exact bytes.
    ///
    /// The view is not cached and never replaces or reserializes the opaque
    /// archive member.
    pub fn typed_book_xml(&self) -> Result<BookXml, ArchiveXmlError> {
        BookXml::parse(self.book_xml()).map_err(ArchiveXmlError::Book)
    }

    /// Resolve and parse one sheet declared by a direct `book.xml` sheet stub.
    ///
    /// Only the stub's exact conventional `sheet#N/sheet#N.xml` path is
    /// consulted. A similarly named member is unrelated. The returned typed
    /// view is created on demand and retains the member bytes exactly.
    pub fn typed_sheet_xml(&self, number: u32) -> Result<SheetXmlMember, ArchiveXmlError> {
        let book = self.typed_book_xml()?;
        let Some(stub) = book
            .sheet_stubs()
            .iter()
            .find(|stub| stub.number() == number)
        else {
            return Ok(SheetXmlMember::Undeclared);
        };

        let archive_path = stub.archive_path().to_owned();
        let Some(bytes) = self.member(&archive_path) else {
            return Ok(SheetXmlMember::Missing { archive_path });
        };
        let xml = SheetXml::parse(bytes).map_err(|source| ArchiveXmlError::Sheet {
            archive_path: archive_path.clone(),
            source,
        })?;
        Ok(SheetXmlMember::Present { archive_path, xml })
    }

    /// Raw, uninterpreted bytes of a validated member, or `None` for a directory.
    #[must_use]
    pub fn member(&self, path: &str) -> Option<&[u8]> {
        self.entries
            .iter()
            .find(|entry| entry.inventory.path == path && !entry.inventory.is_directory)
            .map(|entry| entry.bytes.as_slice())
    }

    /// Write all members to a content-equivalent ZIP archive.
    ///
    /// Member order and compression are deliberately not part of the contract.
    /// Every source member path and uncompressed byte sequence is retained.
    pub fn write_to<W: Write + Seek>(&self, writer: W) -> Result<W, OmrError> {
        let mut zip = ZipWriter::new(writer);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

        for entry in &self.entries {
            if entry.inventory.is_directory {
                zip.add_directory(&entry.inventory.path, options)?;
            } else {
                zip.start_file(&entry.inventory.path, options)?;
                zip.write_all(&entry.bytes)?;
            }
        }

        Ok(zip.finish()?)
    }

    /// Encode a content-equivalent archive in memory.
    pub fn to_bytes(&self) -> Result<Vec<u8>, OmrError> {
        Ok(self.write_to(Cursor::new(Vec::new()))?.into_inner())
    }
}

fn validate_unique_central_paths(
    reader: &mut (impl Read + Seek),
    central_directory_start: u64,
) -> Result<usize, OmrError> {
    const CENTRAL_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x01, 0x02];
    reader.seek(SeekFrom::Start(central_directory_start))?;
    let mut seen = HashSet::new();

    loop {
        let mut signature = [0_u8; 4];
        reader.read_exact(&mut signature)?;
        if signature != CENTRAL_SIGNATURE {
            break;
        }

        let mut fixed = [0_u8; 42];
        reader.read_exact(&mut fixed)?;
        let name_len = usize::from(u16::from_le_bytes([fixed[24], fixed[25]]));
        let extra_len = u64::from(u16::from_le_bytes([fixed[26], fixed[27]]));
        let comment_len = u64::from(u16::from_le_bytes([fixed[28], fixed[29]]));
        let mut raw_name = vec![0_u8; name_len];
        reader.read_exact(&mut raw_name)?;
        let path = validated_member_path(&raw_name)?;
        if !seen.insert(path.clone()) {
            return Err(OmrError::DuplicatePath(path));
        }
        let skip = extra_len
            .checked_add(comment_len)
            .and_then(|value| i64::try_from(value).ok())
            .ok_or(OmrError::LimitExceeded {
                resource: "central directory metadata bytes",
                limit: i64::MAX as u64,
                actual: u64::MAX,
            })?;
        reader.seek(SeekFrom::Current(skip))?;
    }

    Ok(seen.len())
}

fn validated_member_path(raw: &[u8]) -> Result<String, OmrError> {
    let path = std::str::from_utf8(raw)
        .map_err(|_| OmrError::UnsafePath {
            path: String::from_utf8_lossy(raw).into_owned(),
            reason: "member name is not UTF-8",
        })?
        .to_owned();

    let unsafe_reason = if path.is_empty() {
        Some("member name is empty")
    } else if path.starts_with('/') {
        Some("member path is absolute")
    } else if path.contains('\0') {
        Some("member path contains NUL")
    } else if path.contains('\\') {
        Some("member path contains a backslash")
    } else {
        let path_without_directory_slash = path.strip_suffix('/').unwrap_or(&path);
        let mut components = path_without_directory_slash.split('/');
        let first = components.next().unwrap_or_default();
        let has_windows_drive = first.len() == 2
            && first.as_bytes()[0].is_ascii_alphabetic()
            && first.as_bytes()[1] == b':';
        if path_without_directory_slash.is_empty() {
            Some("member path has no component")
        } else if has_windows_drive {
            Some("member path has a Windows drive prefix")
        } else if std::iter::once(first)
            .chain(components)
            .any(|component| component.is_empty() || component == "." || component == "..")
        {
            Some("member path contains an empty, current, or parent component")
        } else {
            None
        }
    };

    match unsafe_reason {
        Some(reason) => Err(OmrError::UnsafePath { path, reason }),
        None => Ok(path),
    }
}

fn read_limited(reader: &mut impl Read, limit: u64) -> Result<Vec<u8>, OmrError> {
    let capacity = usize::try_from(limit.min(1024 * 1024)).unwrap_or(0);
    let mut bytes = Vec::with_capacity(capacity);
    let mut bounded = reader.take(limit.saturating_add(1));
    bounded.read_to_end(&mut bytes)?;
    let actual = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if actual > limit {
        return Err(OmrError::LimitExceeded {
            resource: "decoded entry bytes",
            limit,
            actual,
        });
    }
    Ok(bytes)
}

/// Failures produced while validating, decoding, or re-encoding an archive.
#[derive(Debug)]
pub enum OmrError {
    /// Underlying filesystem or stream error.
    Io(io::Error),
    /// Invalid or unsupported ZIP data.
    Zip(zip::result::ZipError),
    /// A member path is unsafe or cannot be represented without ambiguity.
    UnsafePath {
        /// Path rendered lossily when its original name was not UTF-8.
        path: String,
        /// Validation failure.
        reason: &'static str,
    },
    /// Two central-directory entries use the same validated path.
    DuplicatePath(String),
    /// The required root `book.xml` file is absent.
    MissingBookXml,
    /// A resource bound was exceeded.
    LimitExceeded {
        /// Resource that exceeded its bound.
        resource: &'static str,
        /// Configured maximum.
        limit: u64,
        /// Observed or declared value.
        actual: u64,
    },
    /// A decoded member disagreed with its central-directory size.
    SizeMismatch {
        /// Validated member path.
        path: String,
        /// Declared uncompressed size.
        declared: u64,
        /// Number of decoded bytes.
        decoded: u64,
    },
}

impl fmt::Display for OmrError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "I/O error: {error}"),
            Self::Zip(error) => write!(formatter, "ZIP error: {error}"),
            Self::UnsafePath { path, reason } => {
                write!(formatter, "unsafe ZIP member path {path:?}: {reason}")
            }
            Self::DuplicatePath(path) => write!(formatter, "duplicate ZIP member path {path:?}"),
            Self::MissingBookXml => write!(formatter, "archive has no root {BOOK_XML_PATH}"),
            Self::LimitExceeded {
                resource,
                limit,
                actual,
            } => write!(
                formatter,
                "archive {resource} limit exceeded: {actual} is greater than {limit}"
            ),
            Self::SizeMismatch {
                path,
                declared,
                decoded,
            } => write!(
                formatter,
                "ZIP member {path:?} declares {declared} bytes but decoded {decoded}"
            ),
        }
    }
}

impl Error for OmrError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Zip(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for OmrError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<zip::result::ZipError> for OmrError {
    fn from(error: zip::result::ZipError) -> Self {
        Self::Zip(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_zip(entries: &[(&str, &[u8], bool)]) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for &(path, bytes, deflated) in entries {
            let method = if deflated {
                CompressionMethod::Deflated
            } else {
                CompressionMethod::Stored
            };
            let options = SimpleFileOptions::default().compression_method(method);
            if path.ends_with('/') {
                writer.add_directory(path, options).unwrap();
            } else {
                writer.start_file(path, options).unwrap();
                writer.write_all(bytes).unwrap();
            }
        }
        writer.finish().unwrap().into_inner()
    }

    fn replace_member_name(bytes: &mut [u8], from: &[u8], to: &[u8]) {
        assert_eq!(from.len(), to.len());
        let mut replacements = 0;
        for index in 0..=bytes.len() - from.len() {
            if &bytes[index..index + from.len()] == from {
                bytes[index..index + to.len()].copy_from_slice(to);
                replacements += 1;
            }
        }
        assert_eq!(replacements, 2, "local and central ZIP names must change");
    }

    #[test]
    fn inventories_and_preserves_known_and_unknown_members() {
        let source = make_zip(&[
            (BOOK_XML_PATH, b"<book version=\"5.11\"/>", true),
            ("sheet#1/", b"", false),
            ("sheet#1/sheet.xml", b"<sheet/>", true),
            ("vendor/private.bin", &[0, 255, 1, 254], false),
        ]);

        let archive = OmrArchive::from_bytes(&source).unwrap();
        let paths: Vec<_> = archive
            .inventory()
            .map(|entry| (entry.path(), entry.is_directory()))
            .collect();

        assert_eq!(archive.book_xml(), b"<book version=\"5.11\"/>");
        assert_eq!(
            archive.member("vendor/private.bin"),
            Some(&[0, 255, 1, 254][..])
        );
        assert_eq!(archive.member("sheet#1/"), None);
        assert_eq!(
            paths,
            vec![
                ("book.xml", false),
                ("sheet#1/", true),
                ("sheet#1/sheet.xml", false),
                ("vendor/private.bin", false),
            ]
        );
    }

    #[test]
    fn resolves_declared_sheet_without_interpreting_unrelated_members() {
        let sheet = br#"<sheet last-persistent-id="8"><picture width="20" height="30"/><page id="1"/><sig><future/></sig></sheet>"#;
        let unrelated = b"opaque\0vendor\xff";
        let source = make_zip(&[
            (
                BOOK_XML_PATH,
                b"<book><sheet number=\"1\"/><sheet number=\"2\"/></book>",
                false,
            ),
            ("sheet#1/sheet#1.xml", sheet, true),
            ("sheet#2/sheet#2.xml", b"<not-even-sheet>", false),
            ("vendor/private.bin", unrelated, false),
        ]);
        let archive = OmrArchive::from_bytes(&source).unwrap();

        let book = archive.typed_book_xml().unwrap();
        assert_eq!(book.original_bytes(), archive.book_xml());
        assert_eq!(book.sheet_stubs().len(), 2);

        let resolved = archive.typed_sheet_xml(1).unwrap();
        let SheetXmlMember::Present { archive_path, xml } = resolved else {
            panic!("declared sheet member should be present");
        };
        assert_eq!(archive_path, "sheet#1/sheet#1.xml");
        assert_eq!(xml.original_bytes(), sheet);
        assert_eq!(xml.last_persistent_id(), Some(8));
        assert_eq!(xml.picture().unwrap().width(), 20);
        assert_eq!(xml.page_ids(), &[1]);
        assert_eq!(archive.member("vendor/private.bin"), Some(&unrelated[..]));
    }

    #[test]
    fn distinguishes_missing_conventional_member_and_undeclared_sheet() {
        let source = make_zip(&[
            (BOOK_XML_PATH, b"<book><sheet number=\"4\"/></book>", false),
            ("sheet#4/sheet.xml", b"<sheet/>", false),
            ("unrelated/sheet#4.xml", b"<sheet/>", false),
        ]);
        let archive = OmrArchive::from_bytes(&source).unwrap();

        assert_eq!(
            archive.typed_sheet_xml(4).unwrap(),
            SheetXmlMember::Missing {
                archive_path: "sheet#4/sheet#4.xml".to_owned(),
            }
        );
        assert_eq!(
            archive.typed_sheet_xml(99).unwrap(),
            SheetXmlMember::Undeclared
        );
        assert_eq!(archive.member("sheet#4/sheet.xml"), Some(&b"<sheet/>"[..]));
    }

    #[test]
    fn reports_malformed_resolved_sheet_with_exact_member_path() {
        let source = make_zip(&[
            (BOOK_XML_PATH, b"<book><sheet number=\"7\"/></book>", false),
            (
                "sheet#7/sheet#7.xml",
                b"<sheet><page id=\"1\"></sheet>",
                false,
            ),
        ]);
        let archive = OmrArchive::from_bytes(&source).unwrap();

        let error = archive.typed_sheet_xml(7).unwrap_err();
        assert!(matches!(
            error,
            ArchiveXmlError::Sheet {
                ref archive_path,
                source: SheetXmlError::Malformed { .. },
            } if archive_path == "sheet#7/sheet#7.xml"
        ));
    }

    #[test]
    fn reports_malformed_book_before_resolving_any_sheet_member() {
        let source = make_zip(&[
            (BOOK_XML_PATH, b"<book><sheet number=\"1\"></book>", false),
            ("sheet#1/sheet#1.xml", b"<sheet/>", false),
        ]);
        let archive = OmrArchive::from_bytes(&source).unwrap();

        assert!(matches!(
            archive.typed_sheet_xml(1),
            Err(ArchiveXmlError::Book(BookXmlError::Malformed { .. }))
        ));
    }

    #[test]
    fn round_trip_is_member_content_equivalent() {
        let source = make_zip(&[
            (BOOK_XML_PATH, b"<book/>", true),
            ("unknown.dat", &[9, 8, 7, 0], true),
            ("nested/opaque.xml", b"<future-extension/>", false),
        ]);
        let first = OmrArchive::from_bytes(&source).unwrap();
        let encoded = first.to_bytes().unwrap();
        let second = OmrArchive::from_bytes(&encoded).unwrap();

        let first_inventory: Vec<_> = first
            .inventory()
            .map(|entry| {
                (
                    entry.path(),
                    entry.is_directory(),
                    entry.uncompressed_size(),
                    entry.crc32(),
                )
            })
            .collect();
        let second_inventory: Vec<_> = second
            .inventory()
            .map(|entry| {
                (
                    entry.path(),
                    entry.is_directory(),
                    entry.uncompressed_size(),
                    entry.crc32(),
                )
            })
            .collect();
        assert_eq!(first_inventory, second_inventory);
        for entry in first.inventory().filter(|entry| !entry.is_directory()) {
            assert_eq!(first.member(entry.path()), second.member(entry.path()));
        }
        assert_ne!(
            source, encoded,
            "compression is intentionally allowed to change"
        );
    }

    #[test]
    fn requires_book_xml_at_archive_root() {
        let source = make_zip(&[("nested/book.xml", b"<book/>", false)]);
        assert!(matches!(
            OmrArchive::from_bytes(&source),
            Err(OmrError::MissingBookXml)
        ));
    }

    #[test]
    fn rejects_duplicate_paths() {
        let mut source = make_zip(&[
            (BOOK_XML_PATH, b"first", false),
            ("copy.xml", b"second", false),
        ]);
        replace_member_name(&mut source, b"copy.xml", b"book.xml");
        let result = OmrArchive::from_bytes(&source);
        assert!(
            matches!(result, Err(OmrError::DuplicatePath(ref path)) if path == BOOK_XML_PATH),
            "unexpected duplicate-path result: {result:?}"
        );
    }

    #[test]
    fn rejects_unsafe_member_paths() {
        for unsafe_path in [
            "../escape",
            "/absolute",
            "C:/windows",
            "dir\\windows",
            "./relative",
            "dir//empty",
            "dir/../escape",
            "/",
        ] {
            let source = make_zip(&[
                (BOOK_XML_PATH, b"<book/>", false),
                (unsafe_path, b"bad", false),
            ]);
            assert!(
                matches!(
                    OmrArchive::from_bytes(&source),
                    Err(OmrError::UnsafePath { .. })
                ),
                "accepted unsafe path {unsafe_path:?}"
            );
        }
    }

    #[test]
    fn enforces_declared_entry_and_total_limits() {
        let source = make_zip(&[(BOOK_XML_PATH, b"1234", false), ("other", b"5678", false)]);
        let entry_limit = ArchiveLimits {
            max_entries: 2,
            max_entry_bytes: 3,
            max_total_bytes: 100,
        };
        assert!(matches!(
            OmrArchive::read_from_with_limits(Cursor::new(&source), entry_limit),
            Err(OmrError::LimitExceeded {
                resource: "entry bytes",
                ..
            })
        ));

        let total_limit = ArchiveLimits {
            max_entries: 2,
            max_entry_bytes: 10,
            max_total_bytes: 7,
        };
        assert!(matches!(
            OmrArchive::read_from_with_limits(Cursor::new(&source), total_limit),
            Err(OmrError::LimitExceeded {
                resource: "total bytes",
                ..
            })
        ));
    }

    #[test]
    fn enforces_entry_count_limit_before_decoding() {
        let source = make_zip(&[(BOOK_XML_PATH, b"<book/>", false), ("one", b"1", false)]);
        let limits = ArchiveLimits {
            max_entries: 1,
            ..ArchiveLimits::default()
        };
        assert!(matches!(
            OmrArchive::read_from_with_limits(Cursor::new(source), limits),
            Err(OmrError::LimitExceeded {
                resource: "entry count",
                ..
            })
        ));
    }
}
