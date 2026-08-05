//! DEFLATE, written out rather than linked.
//!
//! Same reasoning as the JPEG decoder: the target is not "a correct inflater",
//! it is *the bytes `java.util.zip.Inflater` produces*, and those differ from a
//! textbook implementation in exactly one respect that matters -- what happens
//! when the stream is damaged.
//!
//! PDFBox's `FlateFilter` catches `DataFormatException` and keeps whatever was
//! written before it, so a truncated or corrupt stream yields a *prefix* rather
//! than nothing. Real PDFs rely on this: a stream whose final block is cut off
//! still renders everything above the cut. So every failure path here stops and
//! returns what it has, and only a header that cannot be read at all is an
//! error.
//!
//! The zlib wrapper is checked but not enforced. PDFBox does not verify the
//! Adler-32 either -- `Inflater` reports the mismatch through `needsInput`
//! rather than throwing, and PDFBox does not ask.

use crate::error::{Error, Result};

/// The maximum number of bits in any code, for both alphabets.
const MAXIMUM_BITS: usize = 15;

/// Extra bits and base values for the length codes, 257..=285.
const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];

/// Extra bits and base values for the distance codes, 0..=29.
const DISTANCE_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DISTANCE_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

/// The order the code-length alphabet's own lengths arrive in.
const LENGTH_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// A canonical Huffman decoder, in the counts-and-symbols form zlib uses.
struct Huffman {
    /// How many codes are of each length, indexed by length.
    counts: [u16; MAXIMUM_BITS + 1],
    /// The symbols, ordered by code length and then by symbol.
    symbols: Vec<u16>,
}

impl Huffman {
    /// Builds the canonical code for `lengths`, where index is the symbol.
    fn new(lengths: &[u8]) -> Self {
        let mut counts = [0u16; MAXIMUM_BITS + 1];
        for &length in lengths {
            counts[length as usize] += 1;
        }
        counts[0] = 0;
        let mut offsets = [0u16; MAXIMUM_BITS + 2];
        for length in 1..=MAXIMUM_BITS {
            offsets[length + 1] = offsets[length] + counts[length];
        }
        let mut symbols = vec![0u16; lengths.len()];
        for (symbol, &length) in lengths.iter().enumerate() {
            if length != 0 {
                symbols[offsets[length as usize] as usize] = symbol as u16;
                offsets[length as usize] += 1;
            }
        }
        Self { counts, symbols }
    }
}

/// A bit reader in DEFLATE's order: least significant bit of each byte first.
struct Bits<'a> {
    data: &'a [u8],
    position: usize,
    accumulator: u32,
    held: u32,
}

impl<'a> Bits<'a> {
    const fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            position: 0,
            accumulator: 0,
            held: 0,
        }
    }

    /// Reads `count` bits, or `None` once the input is exhausted.
    fn take(&mut self, count: u32) -> Option<u32> {
        while self.held < count {
            let byte = *self.data.get(self.position)?;
            self.position += 1;
            self.accumulator |= u32::from(byte) << self.held;
            self.held += 8;
        }
        let value = self.accumulator & ((1u32 << count) - 1);
        self.accumulator >>= count;
        self.held -= count;
        Some(value)
    }

    /// Discards the rest of the current byte.
    const fn align(&mut self) {
        let drop = self.held % 8;
        self.accumulator >>= drop;
        self.held -= drop;
    }

    /// Decodes one symbol, walking the canonical code one bit at a time.
    fn symbol(&mut self, table: &Huffman) -> Option<u16> {
        let mut code = 0i32;
        let mut first = 0i32;
        let mut index = 0i32;
        for length in 1..=MAXIMUM_BITS {
            code |= self.take(1)? as i32;
            let count = i32::from(table.counts[length]);
            if code - first < count {
                return table
                    .symbols
                    .get((index + (code - first)) as usize)
                    .copied();
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        None
    }
}

/// Inflates a zlib-wrapped stream, which is what `/FlateDecode` carries.
///
/// A stream whose first two bytes are not a plausible zlib header is inflated
/// as raw DEFLATE instead. PDFBox does the same thing, in
/// `FlateFilter.decode`'s retry, because writers do emit headerless streams.
///
/// # Errors
///
/// [`Error::CorruptStream`] only when nothing at all could be read; a stream
/// that starts well and then breaks returns its good prefix.
pub fn decode(data: &[u8]) -> Result<Vec<u8>> {
    let body = if is_zlib_header(data) {
        &data[2..]
    } else {
        data
    };
    let output = inflate(body);
    if output.is_empty() && !body.is_empty() {
        return Err(Error::CorruptStream { filter: "Flate" });
    }
    Ok(output)
}

/// Whether the first two bytes are a well-formed zlib header.
///
/// The check is the specification's: the low nibble of the first byte is the
/// compression method and must be 8, the window size must be at most 7, and the
/// two bytes together must be a multiple of 31.
fn is_zlib_header(data: &[u8]) -> bool {
    let [first, second, ..] = data else {
        return false;
    };
    first & 0x0f == 8 && first >> 4 <= 7 && (u16::from(*first) << 8 | u16::from(*second)) % 31 == 0
}

/// Inflates raw DEFLATE, stopping at the first thing it cannot read.
#[must_use]
pub fn inflate(data: &[u8]) -> Vec<u8> {
    let mut bits = Bits::new(data);
    let mut output = Vec::new();
    loop {
        let Some(final_block) = bits.take(1) else {
            return output;
        };
        let Some(kind) = bits.take(2) else {
            return output;
        };
        let completed = match kind {
            0 => stored(&mut bits, &mut output),
            1 => {
                let (literals, distances) = fixed_tables();
                block(&mut bits, &mut output, &literals, &distances)
            }
            2 => match dynamic_tables(&mut bits) {
                Some((literals, distances)) => block(&mut bits, &mut output, &literals, &distances),
                None => false,
            },
            // Code 3 is reserved; zlib stops here and so does this.
            _ => false,
        };
        if !completed || final_block == 1 {
            return output;
        }
    }
}

/// A stored block: byte-aligned, with a length and its complement.
fn stored(bits: &mut Bits<'_>, output: &mut Vec<u8>) -> bool {
    bits.align();
    let Some(length) = bits.take(16) else {
        return false;
    };
    // The complement is read and ignored, exactly as zlib's `inflate` does not
    // check it in its fast path.
    if bits.take(16).is_none() {
        return false;
    }
    for _ in 0..length {
        let Some(byte) = bits.take(8) else {
            return false;
        };
        output.push(byte as u8);
    }
    true
}

/// The fixed alphabets of section 3.2.6.
fn fixed_tables() -> (Huffman, Huffman) {
    let mut lengths = [0u8; 288];
    for (symbol, slot) in lengths.iter_mut().enumerate() {
        *slot = match symbol {
            0..=143 => 8,
            144..=255 => 9,
            256..=279 => 7,
            _ => 8,
        };
    }
    (Huffman::new(&lengths), Huffman::new(&[5u8; 30]))
}

/// Reads the code-length alphabet and then the two alphabets it describes.
fn dynamic_tables(bits: &mut Bits<'_>) -> Option<(Huffman, Huffman)> {
    let literal_count = bits.take(5)? as usize + 257;
    let distance_count = bits.take(5)? as usize + 1;
    let code_count = bits.take(4)? as usize + 4;
    if literal_count > 286 || distance_count > 30 {
        return None;
    }
    let mut code_lengths = [0u8; 19];
    for &index in LENGTH_ORDER.iter().take(code_count) {
        code_lengths[index] = bits.take(3)? as u8;
    }
    let codes = Huffman::new(&code_lengths);

    let total = literal_count + distance_count;
    let mut lengths = vec![0u8; total];
    let mut filled = 0;
    while filled < total {
        let symbol = bits.symbol(&codes)?;
        match symbol {
            0..=15 => {
                lengths[filled] = symbol as u8;
                filled += 1;
            }
            16 => {
                let previous = if filled == 0 {
                    return None;
                } else {
                    lengths[filled - 1]
                };
                let repeat = 3 + bits.take(2)? as usize;
                for _ in 0..repeat.min(total - filled) {
                    lengths[filled] = previous;
                    filled += 1;
                }
            }
            17 => filled += (3 + bits.take(3)? as usize).min(total - filled),
            18 => filled += (11 + bits.take(7)? as usize).min(total - filled),
            _ => return None,
        }
    }
    Some((
        Huffman::new(&lengths[..literal_count]),
        Huffman::new(&lengths[literal_count..]),
    ))
}

/// Decodes one compressed block. Returns whether it ended properly.
fn block(
    bits: &mut Bits<'_>,
    output: &mut Vec<u8>,
    literals: &Huffman,
    distances: &Huffman,
) -> bool {
    loop {
        let Some(symbol) = bits.symbol(literals) else {
            return false;
        };
        match symbol {
            0..=255 => output.push(symbol as u8),
            256 => return true,
            257..=285 => {
                let index = symbol as usize - 257;
                let Some(extra) = bits.take(u32::from(LENGTH_EXTRA[index])) else {
                    return false;
                };
                let length = LENGTH_BASE[index] as usize + extra as usize;
                let Some(code) = bits.symbol(distances) else {
                    return false;
                };
                let code = code as usize;
                if code >= DISTANCE_BASE.len() {
                    return false;
                }
                let Some(extra) = bits.take(u32::from(DISTANCE_EXTRA[code])) else {
                    return false;
                };
                let distance = DISTANCE_BASE[code] as usize + extra as usize;
                if distance > output.len() {
                    return false;
                }
                // Overlapping copies are the point of the format, so this
                // copies byte by byte rather than slicing.
                let from = output.len() - distance;
                for offset in 0..length {
                    output.push(output[from + offset]);
                }
            }
            _ => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// zlib's own deflation of "hello hello hello", at level 9.
    const HELLO: [u8; 16] = [
        0x78, 0xda, 0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0xc8, 0x40, 0x90, 0x00, 0x3a, 0x2e, 0x06,
        0x7d,
    ];

    #[test]
    fn inflates_a_stored_block() {
        // final block, stored, length 3, complement, then the bytes.
        let data = [0x01, 0x03, 0x00, 0xfc, 0xff, b'a', b'b', b'c'];
        assert_eq!(inflate(&data), b"abc");
    }

    #[test]
    fn inflates_the_fixed_alphabet() {
        assert_eq!(decode(&HELLO).expect("inflates"), b"hello hello hello");
    }

    #[test]
    fn keeps_the_prefix_of_a_truncated_stream() {
        // Every truncation point, against what PDFBox produces for the same
        // bytes. It keeps whatever `Inflater` wrote before the exception, so a
        // cut stream decodes to a prefix rather than to nothing -- and note
        // that cutting up to four bytes changes nothing, because those are the
        // Adler-32 that neither PDFBox nor this checks.
        const PREFIXES: [&str; 11] = [
            "hello hello hello",
            "hello hello hello",
            "hello hello hello",
            "hello hello hello",
            "hello hello hello",
            "hello h",
            "hello ",
            "hello",
            "hell",
            "hel",
            "he",
        ];
        for (cut, expected) in (1..=11).zip(PREFIXES) {
            let decoded = decode(&HELLO[..HELLO.len() - cut]).expect("inflates a prefix");
            assert_eq!(decoded, expected.as_bytes(), "cutting {cut} bytes");
        }
    }

    #[test]
    fn inflates_a_headerless_stream() {
        let data = [0x01, 0x03, 0x00, 0xfc, 0xff, b'a', b'b', b'c'];
        assert_eq!(decode(&data).expect("inflates"), b"abc");
    }
}
