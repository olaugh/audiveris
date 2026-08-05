//! The stream filters, and the chain that applies them.
//!
//! Reproduces PDFBox's `org.apache.pdfbox.filter` package: the same set, the
//! same abbreviations, the same defaults, and -- where it shows -- the same
//! behaviour on input that is not quite right.
//!
//! Two details that are easy to get wrong and expensive to debug:
//!
//! - **A predictor is not part of the filter.** `/DecodeParms` sits beside the
//!   filter and is applied *after* it, and the same code serves `FlateDecode`
//!   and `LZWDecode`. A PNG predictor prepends a tag byte to every row, so
//!   getting the row length wrong shears the whole image rather than corrupting
//!   it locally.
//! - **`/DecodeParms` is positional.** With a filter *array* it is an array
//!   too, one entry per filter, and entries may be null. Reading it as a single
//!   dictionary works until a file uses two filters, at which point the
//!   parameters land on the wrong one.
//!
//! The image filters -- `DCTDecode`, `CCITTFaxDecode`, `JBIG2Decode` -- are
//! dispatched from here as well, because PDFBox applies them in
//! `COSStream.createInputStream` like any other filter, and the port's oracle
//! hashes that stream. They need the image's geometry, so they read the stream
//! dictionary as well as their own parameters.

use crate::ccitt;
use crate::error::{Error, Result};
use crate::flate;
use crate::jbig2;
use crate::object::{Dictionary, Name, Object};

/// Everything a filter needs beyond its own bytes.
///
/// A filter's parameters come from two places: its `/DecodeParms` entry, and
/// the stream dictionary itself, which is where `CCITTFaxDecode` finds the
/// image `/Height` when its parameters omit `/Rows`.
#[derive(Debug, Clone, Copy)]
pub struct Parameters<'a> {
    /// The filter's own `/DecodeParms` entry, already selected by position.
    pub parms: Option<&'a Dictionary>,
    /// The stream's dictionary.
    pub stream: &'a Dictionary,
    /// The `/JBIG2Globals` stream's decoded bytes, if the parameters name one.
    ///
    /// PDFBox concatenates it in front of the image's own data rather than
    /// passing it separately, so it is a filter input like any other. None of
    /// the corpus uses one.
    pub globals: &'a [u8],
}

impl Parameters<'_> {
    /// An integer from `/DecodeParms`, or `fallback`.
    #[must_use]
    pub fn integer(&self, key: &str, fallback: i64) -> i64 {
        self.parms
            .and_then(|parms| parms.get(&Name::new(key)))
            .and_then(Object::as_i64)
            .unwrap_or(fallback)
    }

    /// A boolean from `/DecodeParms`, or `fallback`.
    #[must_use]
    pub fn boolean(&self, key: &str, fallback: bool) -> bool {
        match self.parms.and_then(|parms| parms.get(&Name::new(key))) {
            Some(Object::Boolean(value)) => *value,
            _ => fallback,
        }
    }

    /// An integer from the stream dictionary, or `fallback`.
    #[must_use]
    pub fn stream_integer(&self, key: &str, fallback: i64) -> i64 {
        self.stream
            .get(&Name::new(key))
            .and_then(Object::as_i64)
            .unwrap_or(fallback)
    }
}

/// The filter names, with the abbreviations inline images may use.
///
/// The abbreviations are legal only in an inline image's dictionary, but PDFBox
/// accepts them anywhere, so a stream written by a tool that reuses its inline
/// -image writer still decodes.
fn canonical(name: &[u8]) -> &'static str {
    match name {
        b"FlateDecode" | b"Fl" => "FlateDecode",
        b"LZWDecode" | b"LZW" => "LZWDecode",
        b"ASCII85Decode" | b"A85" => "ASCII85Decode",
        b"ASCIIHexDecode" | b"AHx" => "ASCIIHexDecode",
        b"RunLengthDecode" | b"RL" => "RunLengthDecode",
        b"CCITTFaxDecode" | b"CCF" => "CCITTFaxDecode",
        b"DCTDecode" | b"DCT" => "DCTDecode",
        b"JBIG2Decode" => "JBIG2Decode",
        b"JPXDecode" => "JPXDecode",
        b"Crypt" => "Crypt",
        _ => "",
    }
}

/// The filter names a stream declares, in application order.
///
/// `/Filter` is either a name or an array of names; anything else, including
/// its absence, means the stream is not filtered.
#[must_use]
pub fn names(dictionary: &Dictionary) -> Vec<Name> {
    match dictionary.get(&Name::new("Filter")) {
        Some(Object::Name(name)) => vec![name.clone()],
        Some(Object::Array(entries)) => entries
            .iter()
            .filter_map(|entry| entry.name().cloned())
            .collect(),
        _ => Vec::new(),
    }
}

/// The `/DecodeParms` entry that goes with filter `index`.
///
/// Also accepts `/DP`, the inline-image abbreviation, as PDFBox does.
#[must_use]
fn parameters_at(dictionary: &Dictionary, index: usize) -> Option<&Dictionary> {
    let parms = dictionary
        .get(&Name::new("DecodeParms"))
        .or_else(|| dictionary.get(&Name::new("DP")))?;
    match parms {
        Object::Dictionary(single) if index == 0 => Some(single),
        Object::Array(entries) => entries.get(index).and_then(Object::dictionary),
        _ => None,
    }
}

/// Applies a stream's whole filter chain.
///
/// # Errors
///
/// [`Error::UnsupportedFilter`] for a filter this port has not written, and
/// whatever a filter reports for input it cannot use at all.
pub fn decode(dictionary: &Dictionary, data: &[u8]) -> Result<Vec<u8>> {
    let mut current = data.to_vec();
    for (index, name) in names(dictionary).iter().enumerate() {
        let parameters = Parameters {
            parms: parameters_at(dictionary, index),
            stream: dictionary,
            globals: &[],
        };
        current = apply(name, &current, parameters)?;
    }
    Ok(current)
}

/// Applies one filter.
///
/// # Errors
///
/// [`Error::UnsupportedFilter`] for anything not implemented here.
pub fn apply(name: &Name, data: &[u8], parameters: Parameters<'_>) -> Result<Vec<u8>> {
    match canonical(name.as_bytes()) {
        "FlateDecode" => Ok(predict(&flate::decode(data)?, parameters)),
        "LZWDecode" => Ok(predict(
            &lzw(data, parameters.integer("EarlyChange", 1)),
            parameters,
        )),
        "ASCII85Decode" => Ok(ascii85(data)),
        "ASCIIHexDecode" => Ok(ascii_hex(data)),
        "RunLengthDecode" => Ok(run_length(data)),
        "CCITTFaxDecode" => ccitt::decode(data, &fax_options(data, parameters)),
        "JBIG2Decode" => jbig2::decode(data, parameters.globals),
        other => Err(Error::UnsupportedFilter {
            name: if other.is_empty() {
                String::from_utf8_lossy(name.as_bytes()).into_owned()
            } else {
                other.to_owned()
            },
        }),
    }
}

/// Gathers `CCITTFaxDecode`'s parameters the way `CCITTFaxFilter` gathers them.
///
/// The `/Rows` rule is the surprising one: when the image dictionary also
/// carries a `/Height`, the height wins outright rather than the smaller of the
/// two being taken. PDFBOX-771 and PDFBOX-3727 are files whose `/Rows` is
/// simply wrong, and believing it truncates or overruns the image.
fn fax_options(data: &[u8], parameters: Parameters<'_>) -> ccitt::Options {
    let rows = parameters.integer("Rows", 0);
    let height = parameters
        .stream
        .get(&Name::new("Height"))
        .or_else(|| parameters.stream.get(&Name::new("H")))
        .and_then(Object::as_i64)
        .unwrap_or(0);
    let rows = if rows > 0 && height > 0 {
        height
    } else {
        rows.max(height)
    };
    let end_of_line = parameters
        .parms
        .and_then(|parms| parms.get(&Name::new("EndOfLine")))
        .map(|_| parameters.boolean("EndOfLine", false));
    let (encoding, two_dimensional) = ccitt::choose(data, parameters.integer("K", 0), end_of_line);
    ccitt::Options {
        columns: parameters.integer("Columns", 1728).max(0) as usize,
        rows: rows.max(0) as usize,
        encoding,
        two_dimensional,
        byte_aligned: parameters.boolean("EncodedByteAlign", false),
        black_is_one: parameters.boolean("BlackIs1", false),
    }
}

/// Undoes a `/Predictor`, if the parameters ask for one.
///
/// Predictor 1 and anything absent is a pass-through, 2 is TIFF, and 10 and
/// above are the PNG per-row filters. The declared value above 10 is the
/// *encoder's* choice and is ignored on decode: each row carries its own tag.
#[must_use]
fn predict(data: &[u8], parameters: Parameters<'_>) -> Vec<u8> {
    let predictor = parameters.integer("Predictor", 1);
    if predictor <= 1 {
        return data.to_vec();
    }
    let colours = parameters.integer("Colors", 1).clamp(1, 32) as usize;
    let depth = parameters.integer("BitsPerComponent", 8).clamp(1, 16) as usize;
    let columns = parameters.integer("Columns", 1).max(1) as usize;
    let sample_bytes = (colours * depth).div_ceil(8);
    let row_bytes = (colours * depth * columns).div_ceil(8);
    if predictor == 2 {
        return tiff_predictor(data, colours, depth, columns);
    }
    png_predictor(data, sample_bytes, row_bytes)
}

/// The PNG per-row filters, each row tagged with the one that produced it.
fn png_predictor(data: &[u8], sample_bytes: usize, row_bytes: usize) -> Vec<u8> {
    let mut output = Vec::with_capacity(data.len());
    let mut previous = vec![0u8; row_bytes];
    let mut current = vec![0u8; row_bytes];
    let mut at = 0;
    while at < data.len() {
        let tag = data[at];
        at += 1;
        // A final row may be short; PDFBox decodes what is there rather than
        // discarding it, so a truncated stream keeps its last partial row.
        let available = row_bytes.min(data.len() - at);
        current[..available].copy_from_slice(&data[at..at + available]);
        current[available..].fill(0);
        at += available;
        for index in 0..row_bytes {
            let raw = i32::from(current[index]);
            let left = if index >= sample_bytes {
                i32::from(current[index - sample_bytes])
            } else {
                0
            };
            let up = i32::from(previous[index]);
            let up_left = if index >= sample_bytes {
                i32::from(previous[index - sample_bytes])
            } else {
                0
            };
            current[index] = match tag {
                1 => (raw + left) as u8,
                2 => (raw + up) as u8,
                3 => (raw + (left + up) / 2) as u8,
                4 => (raw + paeth(left, up, up_left)) as u8,
                _ => raw as u8,
            };
        }
        output.extend_from_slice(&current[..available]);
        previous.copy_from_slice(&current);
        if available < row_bytes {
            break;
        }
    }
    output
}

/// The PNG Paeth predictor of section 6.6.
const fn paeth(left: i32, up: i32, up_left: i32) -> i32 {
    let estimate = left + up - up_left;
    let to_left = (estimate - left).abs();
    let to_up = (estimate - up).abs();
    let to_corner = (estimate - up_left).abs();
    if to_left <= to_up && to_left <= to_corner {
        left
    } else if to_up <= to_corner {
        up
    } else {
        up_left
    }
}

/// TIFF predictor 2: each sample is a delta from the one a pixel to its left.
///
/// Only eight-bit components are handled, which is also the only case PDFBox
/// implements; anything else passes through, because getting it wrong quietly
/// is worse than not doing it.
fn tiff_predictor(data: &[u8], colours: usize, depth: usize, columns: usize) -> Vec<u8> {
    if depth != 8 {
        return data.to_vec();
    }
    let row_bytes = colours * columns;
    let mut output = data.to_vec();
    for row in output.chunks_mut(row_bytes) {
        for index in colours..row.len() {
            row[index] = row[index].wrapping_add(row[index - colours]);
        }
    }
    output
}

/// LZW as PDF uses it: 9 to 12 bit codes, most significant bit first.
///
/// `early` is `/EarlyChange`, which decides whether the code width grows one
/// code before the table is actually full. It defaults to 1, and a file that
/// sets it to 0 and is decoded as if it were 1 goes wrong a few hundred bytes
/// in rather than immediately.
#[must_use]
fn lzw(data: &[u8], early: i64) -> Vec<u8> {
    const CLEAR: u16 = 256;
    const END: u16 = 257;
    let early = u32::from(early != 0);

    let mut table: Vec<Vec<u8>> = Vec::with_capacity(4096);
    let reset = |table: &mut Vec<Vec<u8>>| {
        table.clear();
        for byte in 0u16..=255 {
            table.push(vec![byte as u8]);
        }
        table.push(Vec::new());
        table.push(Vec::new());
    };
    reset(&mut table);

    let mut output = Vec::new();
    let mut width = 9u32;
    let mut previous: Option<u16> = None;
    let mut accumulator = 0u32;
    let mut held = 0u32;

    for &byte in data {
        accumulator = accumulator << 8 | u32::from(byte);
        held += 8;
        while held >= width {
            let code = ((accumulator >> (held - width)) & ((1 << width) - 1)) as u16;
            held -= width;
            if code == END {
                return output;
            }
            if code == CLEAR {
                reset(&mut table);
                width = 9;
                previous = None;
                continue;
            }
            let entry = match (code as usize).cmp(&table.len()) {
                std::cmp::Ordering::Less => table[code as usize].clone(),
                // The one self-referential case: a code for an entry that this
                // very step is about to create.
                std::cmp::Ordering::Equal => match previous {
                    Some(previous) => {
                        let mut entry = table[previous as usize].clone();
                        let Some(&first) = entry.first() else {
                            return output;
                        };
                        entry.push(first);
                        entry
                    }
                    None => return output,
                },
                std::cmp::Ordering::Greater => return output,
            };
            if let Some(previous) = previous
                && table.len() < 4096
            {
                let mut grown = table[previous as usize].clone();
                grown.push(entry[0]);
                table.push(grown);
            }
            output.extend_from_slice(&entry);
            previous = Some(code);
            width = match table.len() as u32 + early {
                0..=511 => 9,
                512..=1023 => 10,
                1024..=2047 => 11,
                _ => 12,
            };
        }
    }
    output
}

/// ASCII85, ending at `~>`.
///
/// `z` stands for four zero bytes, and a partial final group is padded with
/// `u` and then truncated.
#[must_use]
fn ascii85(data: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    let mut group = [0u8; 5];
    let mut filled = 0;
    let mut at = 0;
    // A leading `<~` is not in the specification but writers emit it.
    if data.starts_with(b"<~") {
        at = 2;
    }
    while at < data.len() {
        let byte = data[at];
        at += 1;
        if byte == b'~' {
            break;
        }
        if byte.is_ascii_whitespace() || byte == 0 {
            continue;
        }
        if byte == b'z' && filled == 0 {
            output.extend_from_slice(&[0, 0, 0, 0]);
            continue;
        }
        if !(b'!'..=b'u').contains(&byte) {
            continue;
        }
        group[filled] = byte - b'!';
        filled += 1;
        if filled == 5 {
            let value = group.iter().fold(0u32, |value, &digit| {
                value.wrapping_mul(85).wrapping_add(u32::from(digit))
            });
            output.extend_from_slice(&value.to_be_bytes());
            filled = 0;
        }
    }
    if filled > 1 {
        let mut padded = group;
        padded[filled..].fill(84);
        let value = padded.iter().fold(0u32, |value, &digit| {
            value.wrapping_mul(85).wrapping_add(u32::from(digit))
        });
        output.extend_from_slice(&value.to_be_bytes()[..filled - 1]);
    }
    output
}

/// ASCIIHex, ending at `>`, with an odd final digit padded with zero.
#[must_use]
fn ascii_hex(data: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    let mut pending: Option<u8> = None;
    for &byte in data {
        if byte == b'>' {
            break;
        }
        let value = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => continue,
        };
        match pending.take() {
            Some(high) => output.push(high << 4 | value),
            None => pending = Some(value),
        }
    }
    if let Some(high) = pending {
        output.push(high << 4);
    }
    output
}

/// Run length: a byte under 128 introduces that many literals plus one, a byte
/// over 128 repeats the next byte, and 128 ends the stream.
#[must_use]
fn run_length(data: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    let mut at = 0;
    while at < data.len() {
        let control = data[at];
        at += 1;
        if control == 128 {
            break;
        }
        if control < 128 {
            let count = control as usize + 1;
            let available = count.min(data.len() - at);
            output.extend_from_slice(&data[at..at + available]);
            at += available;
        } else {
            let Some(&byte) = data.get(at) else { break };
            at += 1;
            output.extend(std::iter::repeat_n(byte, 257 - control as usize));
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::Object;

    fn dictionary(pairs: &[(&str, Object)]) -> Dictionary {
        pairs
            .iter()
            .map(|(key, value)| (Name::new(key), value.clone()))
            .collect()
    }

    fn bare(data: &[u8], parms: &Dictionary) -> Vec<u8> {
        let empty = Dictionary::new();
        predict(
            data,
            Parameters {
                parms: Some(parms),
                stream: &empty,
                globals: &[],
            },
        )
    }

    #[test]
    fn decodes_ascii_hex_and_ascii85() {
        assert_eq!(ascii_hex(b"48656C6C6F>"), b"Hello");
        assert_eq!(ascii_hex(b"4 8 6 5>"), b"He");
        assert_eq!(ascii_hex(b"4>"), vec![0x40]);
        assert_eq!(ascii85(b"87cURD_*#4DfTZ)+T~>"), b"Hello, World!");
        assert_eq!(ascii85(b"z~>"), vec![0, 0, 0, 0]);
    }

    #[test]
    fn decodes_run_length() {
        // Two literals, then `x` five times, then the end marker.
        assert_eq!(run_length(&[1, b'a', b'b', 252, b'x', 128]), b"abxxxxx");
    }

    #[test]
    fn undoes_the_png_up_predictor() {
        // Two rows of three bytes: the first raw, the second a delta upward.
        let parms = dictionary(&[
            ("Predictor", Object::Integer(12)),
            ("Colors", Object::Integer(1)),
            ("BitsPerComponent", Object::Integer(8)),
            ("Columns", Object::Integer(3)),
        ]);
        let data = [0, 10, 20, 30, 2, 1, 1, 1];
        assert_eq!(bare(&data, &parms), vec![10, 20, 30, 11, 21, 31]);
    }

    #[test]
    fn undoes_the_tiff_predictor() {
        let parms = dictionary(&[
            ("Predictor", Object::Integer(2)),
            ("Colors", Object::Integer(1)),
            ("BitsPerComponent", Object::Integer(8)),
            ("Columns", Object::Integer(4)),
        ]);
        assert_eq!(bare(&[10, 1, 1, 1], &parms), vec![10, 11, 12, 13]);
    }

    #[test]
    fn decodes_the_specifications_lzw_example() {
        // Table 7.8 of the specification: this encodes 45 45 45 45 45 65 45 45
        // 45 66, and exercises the self-referential code.
        let encoded = [0x80, 0x0b, 0x60, 0x50, 0x22, 0x0c, 0x0c, 0x85, 0x01];
        assert_eq!(
            lzw(&encoded, 1),
            vec![45, 45, 45, 45, 45, 65, 45, 45, 45, 66]
        );
    }

    #[test]
    fn selects_decode_parameters_by_position() {
        let stream = dictionary(&[
            (
                "Filter",
                Object::Array(vec![
                    Object::Name(Name::new("ASCII85Decode")),
                    Object::Name(Name::new("FlateDecode")),
                ]),
            ),
            (
                "DecodeParms",
                Object::Array(vec![
                    Object::Null,
                    Object::Dictionary(dictionary(&[("Predictor", Object::Integer(12))])),
                ]),
            ),
        ]);
        assert!(parameters_at(&stream, 0).is_none());
        assert_eq!(
            parameters_at(&stream, 1)
                .and_then(|parms| parms.get(&Name::new("Predictor")))
                .and_then(Object::as_i64),
            Some(12)
        );
    }
}
