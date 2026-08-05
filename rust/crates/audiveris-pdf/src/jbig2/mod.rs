//! `JBIG2Decode`, for the embedded stream organisation PDF uses.
//!
//! Ported from `jbig2-imageio`, the plugin PDFBox delegates to, rather than
//! from T.88. That choice is the same one the JPEG decoder had to make: the
//! target is not "a correct JBIG2 decoder", it is the bytes Java produces, and
//! those differ from the standard wherever the plugin does.
//!
//! # What is implemented, and why that is the whole corpus
//!
//! Every one of the 95 JBIG2 images across the seven sampled IMSLP sources has
//! the same shape: a page information segment, one arithmetically-coded symbol
//! dictionary, and one immediate text region covering the whole page, with no
//! `/JBIG2Globals`. The dictionary flag word is 0 and the region's is 16, which
//! fixes template 0, no refinement, no aggregation, single-row strips and a
//! top-left reference corner.
//!
//! So generic *region segments*, Huffman coding, refinement, halftones and
//! striped pages are not implemented. Each returns
//! [`Error::UnsupportedFilter`] naming what it met. The generic region
//! *decoding procedure* is implemented in full, templates 0 to 3 with adaptive
//! pixels and typical prediction, because the symbol dictionary codes every
//! symbol's bitmap through it.
//!
//! # The output
//!
//! `JBIG2Filter` writes the raster of the `BufferedImage` the plugin returns,
//! and that raster is the page bitmap **inverted**, with the padding bits past
//! the row width cleared. JBIG2 codes black as one; the raster's colour model
//! has index zero as black. See [`bitmap::to_raster`].

pub mod arithmetic;
pub mod bitmap;
pub mod generic;
pub mod reader;
pub mod text;

use crate::error::{Error, Result};
use bitmap::{Bitmap, Combine, blit};

/// The segment types this decoder acts on. Everything else is skipped or
/// refused.
mod kind {
    /// A symbol dictionary.
    pub const SYMBOL_DICTIONARY: u8 = 0;
    /// An intermediate text region, which is stored rather than composed.
    pub const INTERMEDIATE_TEXT_REGION: u8 = 4;
    /// An immediate text region.
    pub const IMMEDIATE_TEXT_REGION: u8 = 6;
    /// An immediate lossless text region.
    pub const IMMEDIATE_LOSSLESS_TEXT_REGION: u8 = 7;
    /// Page information.
    pub const PAGE_INFORMATION: u8 = 48;
}

/// One parsed segment header and the bytes that belong to it.
struct Segment<'a> {
    number: u32,
    kind: u8,
    referred: Vec<u32>,
    data: &'a [u8],
}

/// Decodes an embedded JBIG2 stream to the bytes `JBIG2Filter` would write.
///
/// `globals` is the `/JBIG2Globals` stream, already filtered; PDFBox
/// concatenates it in front of the page's own data, so it is treated as a
/// leading run of segments here.
///
/// # Errors
///
/// [`Error::UnsupportedFilter`] for a feature this decoder does not have, and
/// [`Error::CorruptStream`] for a stream whose structure does not parse.
pub fn decode(data: &[u8], globals: &[u8]) -> Result<Vec<u8>> {
    let mut segments = Vec::new();
    parse_segments(globals, &mut segments)?;
    parse_segments(data, &mut segments)?;

    let information = segments
        .iter()
        .find(|segment| segment.kind == kind::PAGE_INFORMATION)
        .ok_or(Error::CorruptStream { filter: "JBIG2" })?;
    let mut reader = reader::Reader::new(information.data);
    let page_width = reader.bits(32) as usize;
    let page_height = reader.bits(32) as usize;
    reader.bits(64); // the two resolutions, which nothing here uses
    let flags = reader.bits(8) as u8;
    let default_pixel = flags >> 2 & 1;
    let default_operator = Combine::from_code(flags >> 3 & 3);
    let striped = reader.bits(16) as u16 & 0x8000 != 0;

    if striped && page_height == usize::try_from(u32::MAX).unwrap_or(usize::MAX) {
        return Err(Error::UnsupportedFilter {
            name: "JBIG2Decode (striped page)".to_owned(),
        });
    }
    if page_width == 0 || page_height == 0 || page_width > 1 << 20 || page_height > 1 << 20 {
        return Err(Error::CorruptStream { filter: "JBIG2" });
    }

    // Symbols accumulate as dictionaries are met, keyed by the segment that
    // exported them, because a region names the dictionaries it draws from.
    let mut dictionaries: Vec<(u32, Vec<Bitmap>)> = Vec::new();
    let mut regions: Vec<text::Region> = Vec::new();

    for segment in &segments {
        match segment.kind {
            kind::SYMBOL_DICTIONARY => {
                let imported = gather(&dictionaries, &segment.referred);
                let exported = text::symbol_dictionary(segment.data, &imported)?;
                dictionaries.push((segment.number, exported));
            }
            kind::INTERMEDIATE_TEXT_REGION
            | kind::IMMEDIATE_TEXT_REGION
            | kind::IMMEDIATE_LOSSLESS_TEXT_REGION => {
                let symbols = gather(&dictionaries, &segment.referred);
                regions.push(text::text_region(segment.data, &symbols)?);
            }
            kind::PAGE_INFORMATION => {}
            // 36..=43 are the generic and refinement regions, 20..=23 the
            // halftones. Naming them is more useful than a bare refusal.
            36..=43 => {
                return Err(Error::UnsupportedFilter {
                    name: "JBIG2Decode (generic or refinement region)".to_owned(),
                });
            }
            20..=23 => {
                return Err(Error::UnsupportedFilter {
                    name: "JBIG2Decode (halftone region)".to_owned(),
                });
            }
            _ => {}
        }
    }

    // 7.4.8: a single region that covers the page and matches it exactly
    // *becomes* the page, with no composition step at all.
    let single = regions.len() == 1
        && default_pixel == 0
        && regions[0].bitmap.width == page_width
        && regions[0].bitmap.height == page_height;
    let page = if single {
        regions.remove(0).bitmap
    } else {
        let mut page = Bitmap::new(page_width, page_height);
        if default_pixel != 0 {
            page.fill(0xff);
        }
        for region in &regions {
            // The page's own operator wins when the page says the region may
            // not choose, which is bit 6 of the page flags; the corpus never
            // reaches here, so the region's is used as jbig2-imageio uses it.
            let _ = default_operator;
            blit(
                &region.bitmap,
                &mut page,
                region.x,
                region.y,
                region.operator,
            );
        }
        page
    };

    Ok(bitmap::to_raster(&page))
}

/// The symbols exported by the dictionaries `referred` names, in order.
fn gather(dictionaries: &[(u32, Vec<Bitmap>)], referred: &[u32]) -> Vec<Bitmap> {
    let mut symbols = Vec::new();
    for number in referred {
        if let Some((_, exported)) = dictionaries.iter().find(|(at, _)| at == number) {
            symbols.extend(exported.iter().cloned());
        }
    }
    symbols
}

/// Splits a stream into segments, per 7.2's embedded organisation.
fn parse_segments<'a>(data: &'a [u8], segments: &mut Vec<Segment<'a>>) -> Result<()> {
    let mut at = 0usize;
    while at + 11 <= data.len() {
        let number = u32::from_be_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]);
        let flags = data[at + 4];
        let kind = flags & 0x3f;
        let long_page = flags & 0x40 != 0;
        let mut cursor = at + 5;

        // 7.2.4: a count below five packs into the top three bits of one byte;
        // above that it is a four-byte count followed by a retain-flag bitmap.
        let first = *data
            .get(cursor)
            .ok_or(Error::CorruptStream { filter: "JBIG2" })?;
        let count = if first >> 5 == 7 {
            let long = u32::from_be_bytes([
                data[cursor],
                *data.get(cursor + 1).unwrap_or(&0),
                *data.get(cursor + 2).unwrap_or(&0),
                *data.get(cursor + 3).unwrap_or(&0),
            ]) & 0x1fff_ffff;
            cursor += 4 + (long as usize).div_ceil(8) + 1;
            long as usize
        } else {
            cursor += 1;
            usize::from(first >> 5)
        };

        // 7.2.5: the width of a referred-to number depends on this segment's.
        let width = if number <= 256 {
            1
        } else if number <= 65536 {
            2
        } else {
            4
        };
        let mut referred = Vec::with_capacity(count);
        for index in 0..count {
            let start = cursor + index * width;
            let end = start + width;
            if end > data.len() {
                return Err(Error::CorruptStream { filter: "JBIG2" });
            }
            let value = data[start..end]
                .iter()
                .fold(0u32, |value, byte| value << 8 | u32::from(*byte));
            referred.push(value);
        }
        cursor += count * width;
        cursor += if long_page { 4 } else { 1 };

        if cursor + 4 > data.len() {
            return Err(Error::CorruptStream { filter: "JBIG2" });
        }
        let length = u32::from_be_bytes([
            data[cursor],
            data[cursor + 1],
            data[cursor + 2],
            data[cursor + 3],
        ]) as usize;
        cursor += 4;
        if length == 0xffff_ffff {
            return Err(Error::UnsupportedFilter {
                name: "JBIG2Decode (unknown segment length)".to_owned(),
            });
        }
        let end = cursor.saturating_add(length).min(data.len());
        segments.push(Segment {
            number,
            kind,
            referred,
            data: &data[cursor..end],
        });
        at = end;
    }
    Ok(())
}
