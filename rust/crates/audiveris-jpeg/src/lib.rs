// SPDX-License-Identifier: AGPL-3.0-or-later

//! A baseline JPEG decoder that reproduces libjpeg sample for sample.
//!
//! # Why this exists
//!
//! Java's `ImageIO` JPEG reader is libjpeg-backed, so parity with Audiveris
//! means parity with libjpeg's arithmetic. The pure-Rust decoders do not
//! provide it. Measured against libjpeg on the example corpus:
//!
//! | input | stages exercised | `zune-jpeg` | `jpeg-decoder` |
//! | --- | --- | --- | --- |
//! | grayscale | Huffman, dequantize, IDCT | 0.7% of samples | 2.7% |
//! | 4:4:4 colour | + colour conversion | 0.8% | 3.1% |
//! | 4:2:0 colour | + chroma upsampling | 5.1% | 5.1% |
//!
//! Entropy decoding and dequantization are fixed by the standard, so the
//! grayscale row isolates the inverse DCT: it already differs. Each later stage
//! adds more. That rules out a small patch to either crate and is why the
//! reconstruction path here is written to libjpeg's exact integer arithmetic.
//!
//! Those differences are small -- at most a few counts per sample -- but the
//! adaptive binarization downstream turns them into flipped pixels, and GRID
//! amplifies single flipped pixels into structural differences.
//!
//! # Scope
//!
//! Baseline sequential, 8-bit, Huffman-coded, one or three components, with
//! 4:4:4, 4:2:2, and 4:2:0 sampling. Progressive, arithmetic coding, 12-bit,
//! and CMYK are rejected rather than approximated, so an unsupported file is a
//! clear error and never a silently different image.
//!
//! # Bit-exactness
//!
//! Every constant below is derived from its defining real number, and the
//! rounding, shifts, and biases follow libjpeg's integer formulation. The claim
//! that this matches is not an argument, it is a test: `tests/parity.rs` decodes
//! against libjpeg-turbo and requires every sample to agree.

#![forbid(unsafe_code)]

use std::fmt;

/// Failure to decode, always specific about what was unsupported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JpegError {
    Truncated,
    MissingSoi,
    /// A marker segment declared a length shorter than its own header.
    BadSegmentLength(u8),
    UnsupportedMarker(u8),
    /// Progressive, arithmetic, lossless, or hierarchical.
    UnsupportedProcess(u8),
    UnsupportedPrecision(u8),
    UnsupportedComponentCount(usize),
    UnsupportedSampling {
        horizontal: usize,
        vertical: usize,
    },
    MissingFrame,
    MissingQuantizationTable(usize),
    MissingHuffmanTable {
        class: usize,
        id: usize,
    },
    BadHuffmanCode,
    ZeroDimension,
}

impl fmt::Display for JpegError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => f.write_str("JPEG data ended mid-stream"),
            Self::MissingSoi => f.write_str("no JPEG start-of-image marker"),
            Self::BadSegmentLength(marker) => {
                write!(f, "marker {marker:#04x} declared an impossible length")
            }
            Self::UnsupportedMarker(marker) => write!(f, "unsupported marker {marker:#04x}"),
            Self::UnsupportedProcess(marker) => write!(
                f,
                "unsupported JPEG process (frame marker {marker:#04x}); only baseline \
                 sequential Huffman is decoded here"
            ),
            Self::UnsupportedPrecision(bits) => write!(f, "unsupported sample precision {bits}"),
            Self::UnsupportedComponentCount(count) => {
                write!(f, "unsupported component count {count}")
            }
            Self::UnsupportedSampling {
                horizontal,
                vertical,
            } => write!(f, "unsupported sampling factors {horizontal}x{vertical}"),
            Self::MissingFrame => f.write_str("scan appeared before the frame header"),
            Self::MissingQuantizationTable(id) => write!(f, "missing quantization table {id}"),
            Self::MissingHuffmanTable { class, id } => {
                write!(f, "missing Huffman table class {class} id {id}")
            }
            Self::BadHuffmanCode => f.write_str("undecodable Huffman code"),
            Self::ZeroDimension => f.write_str("frame declared a zero dimension"),
        }
    }
}

impl std::error::Error for JpegError {}

/// A decoded image, samples interleaved by component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    pub width: usize,
    pub height: usize,
    pub components: usize,
    pub samples: Vec<u8>,
}

/// Zig-zag scan position to natural row-major position.
const NATURAL_ORDER: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

// ---------------------------------------------------------------------------
// Sample range limiting
// ---------------------------------------------------------------------------

/// libjpeg's `sample_range_limit`, as a function rather than a table.
///
/// The shape matters as much as the clamping. Indices above the sample range
/// saturate high, but a second band returns to zero and a third replays the low
/// samples, so that a masked, wrapped-around index lands back on the right
/// value. The inverse DCT relies on that wrap; a plain clamp would differ.
#[must_use]
fn range_limit(value: i32) -> u8 {
    match value {
        -256..=-1 => 0,
        0..=255 => value as u8,
        256..=639 => 255,
        640..=1023 => 0,
        1024..=1151 => (value - 1024) as u8,
        // Colour conversion cannot reach outside the table; clamp defensively
        // rather than panic on a malformed file.
        _ if value < -256 => 0,
        _ => 255,
    }
}

/// Range limit for inverse-DCT output, which arrives centred and possibly
/// negative and is masked into the wrap-around table.
#[must_use]
fn range_limit_idct(value: i32) -> u8 {
    range_limit((value & 1023) + 128)
}

// ---------------------------------------------------------------------------
// Inverse DCT
// ---------------------------------------------------------------------------

/// Fractional bits in the fixed-point multipliers.
const CONST_BITS: i32 = 13;
/// Extra fractional bits carried between the two passes.
const PASS1_BITS: i32 = 2;

/// `round(value * 2^CONST_BITS)`, the multipliers of the integer inverse DCT.
///
/// Each is the scaled form of an exact trigonometric quantity of the
/// even/odd decomposition, listed with the real number it comes from.
const FIX_0_298631336: i32 = 2446; // 0.298631336
const FIX_0_390180644: i32 = 3196; // 0.390180644
const FIX_0_541196100: i32 = 4433; // 0.541196100
const FIX_0_765366865: i32 = 6270; // 0.765366865
const FIX_0_899976223: i32 = 7373; // 0.899976223
const FIX_1_175875602: i32 = 9633; // 1.175875602
const FIX_1_501321110: i32 = 12299; // 1.501321110
const FIX_1_847759065: i32 = 15137; // 1.847759065
const FIX_1_961570560: i32 = 16069; // 1.961570560
const FIX_2_053119869: i32 = 16819; // 2.053119869
const FIX_2_562915447: i32 = 20995; // 2.562915447
const FIX_3_072711026: i32 = 25172; // 3.072711026

/// Round-to-nearest right shift, the descaling step of the integer transform.
#[must_use]
const fn descale(value: i32, bits: i32) -> i32 {
    (value + (1 << (bits - 1))) >> bits
}

/// Odd-part butterfly shared by both passes.
///
/// Returns the four odd-index outputs in coefficient order.
#[must_use]
fn odd_part(mut tmp0: i32, mut tmp1: i32, mut tmp2: i32, mut tmp3: i32) -> [i32; 4] {
    let mut z1 = tmp0 + tmp3;
    let mut z2 = tmp1 + tmp2;
    let mut z3 = tmp0 + tmp2;
    let mut z4 = tmp1 + tmp3;
    let z5 = (z3 + z4) * FIX_1_175875602;

    tmp0 *= FIX_0_298631336;
    tmp1 *= FIX_2_053119869;
    tmp2 *= FIX_3_072711026;
    tmp3 *= FIX_1_501321110;
    z1 *= -FIX_0_899976223;
    z2 *= -FIX_2_562915447;
    z3 *= -FIX_1_961570560;
    z4 *= -FIX_0_390180644;

    z3 += z5;
    z4 += z5;

    [
        tmp0 + z1 + z3,
        tmp1 + z2 + z4,
        tmp2 + z2 + z3,
        tmp3 + z1 + z4,
    ]
}

/// Even-part butterfly shared by both passes.
///
/// Returns the four running sums in the order the outputs pair with the odd
/// part: `[tmp10, tmp11, tmp12, tmp13]`.
#[must_use]
fn even_part(in0: i32, in2: i32, in4: i32, in6: i32) -> [i32; 4] {
    let z1 = (in2 + in6) * FIX_0_541196100;
    let tmp2 = z1 + in6 * -FIX_1_847759065;
    let tmp3 = z1 + in2 * FIX_0_765366865;

    let tmp0 = (in0 + in4) << CONST_BITS;
    let tmp1 = (in0 - in4) << CONST_BITS;

    [tmp0 + tmp3, tmp1 + tmp2, tmp1 - tmp2, tmp0 - tmp3]
}

/// libjpeg's `jpeg_idct_islow`: dequantize and inverse transform one block.
///
/// Both passes take the shortcut for an all-zero AC set, which is an algebraic
/// identity rather than an approximation.
fn inverse_dct(coefficients: &[i16; 64], quantizers: &[u16; 64], output: &mut [u8; 64]) {
    let mut workspace = [0i32; 64];

    // Pass 1: columns, leaving PASS1_BITS of extra fraction.
    for column in 0..8 {
        let at = |row: usize| -> i32 {
            i32::from(coefficients[row * 8 + column]) * i32::from(quantizers[row * 8 + column])
        };
        if (1..8).all(|row| coefficients[row * 8 + column] == 0) {
            let dc = at(0) << PASS1_BITS;
            for row in 0..8 {
                workspace[row * 8 + column] = dc;
            }
            continue;
        }

        let [tmp10, tmp11, tmp12, tmp13] = even_part(at(0), at(2), at(4), at(6));
        let [odd0, odd1, odd2, odd3] = odd_part(at(7), at(5), at(3), at(1));

        let shift = CONST_BITS - PASS1_BITS;
        workspace[column] = descale(tmp10 + odd3, shift);
        workspace[7 * 8 + column] = descale(tmp10 - odd3, shift);
        workspace[8 + column] = descale(tmp11 + odd2, shift);
        workspace[6 * 8 + column] = descale(tmp11 - odd2, shift);
        workspace[2 * 8 + column] = descale(tmp12 + odd1, shift);
        workspace[5 * 8 + column] = descale(tmp12 - odd1, shift);
        workspace[3 * 8 + column] = descale(tmp13 + odd0, shift);
        workspace[4 * 8 + column] = descale(tmp13 - odd0, shift);
    }

    // Pass 2: rows, descaling the whole accumulated fraction away.
    let shift = CONST_BITS + PASS1_BITS + 3;
    for row in 0..8 {
        let line = &workspace[row * 8..row * 8 + 8];
        if line[1..].iter().all(|value| *value == 0) {
            let dc = range_limit_idct(descale(line[0], PASS1_BITS + 3));
            for column in 0..8 {
                output[row * 8 + column] = dc;
            }
            continue;
        }

        let [tmp10, tmp11, tmp12, tmp13] = even_part(line[0], line[2], line[4], line[6]);
        let [odd0, odd1, odd2, odd3] = odd_part(line[7], line[5], line[3], line[1]);

        output[row * 8] = range_limit_idct(descale(tmp10 + odd3, shift));
        output[row * 8 + 7] = range_limit_idct(descale(tmp10 - odd3, shift));
        output[row * 8 + 1] = range_limit_idct(descale(tmp11 + odd2, shift));
        output[row * 8 + 6] = range_limit_idct(descale(tmp11 - odd2, shift));
        output[row * 8 + 2] = range_limit_idct(descale(tmp12 + odd1, shift));
        output[row * 8 + 5] = range_limit_idct(descale(tmp12 - odd1, shift));
        output[row * 8 + 3] = range_limit_idct(descale(tmp13 + odd0, shift));
        output[row * 8 + 4] = range_limit_idct(descale(tmp13 - odd0, shift));
    }
}

// ---------------------------------------------------------------------------
// Colour conversion
// ---------------------------------------------------------------------------

/// Fractional bits in the colour-conversion multipliers.
const SCALEBITS: i32 = 16;
/// Rounding bias for a `SCALEBITS` descale.
const ONE_HALF: i32 = 1 << (SCALEBITS - 1);

/// `round(value * 2^SCALEBITS)`.
#[must_use]
fn colour_fix(value: f64) -> i32 {
    (value * f64::from(1i32 << SCALEBITS) + 0.5) as i32
}

/// libjpeg's precomputed YCbCr to RGB tables.
///
/// The per-channel contributions are tabulated at full precision and only the
/// green channel descales at use, which is what makes the rounding reproduce.
struct ColourTables {
    cr_r: [i32; 256],
    cb_b: [i32; 256],
    cr_g: [i32; 256],
    cb_g: [i32; 256],
}

impl ColourTables {
    fn new() -> Self {
        let mut tables = Self {
            cr_r: [0; 256],
            cb_b: [0; 256],
            cr_g: [0; 256],
            cb_g: [0; 256],
        };
        for i in 0..256 {
            // Centre the sample: index i represents the value i - 128.
            let x = i as i32 - 128;
            tables.cr_r[i] = (colour_fix(1.402_00) * x + ONE_HALF) >> SCALEBITS;
            tables.cb_b[i] = (colour_fix(1.772_00) * x + ONE_HALF) >> SCALEBITS;
            // Green accumulates both chroma terms before descaling once, so the
            // half-ulp bias is carried here and applied to the sum.
            tables.cr_g[i] = -colour_fix(0.714_14) * x;
            tables.cb_g[i] = -colour_fix(0.344_14) * x + ONE_HALF;
        }
        tables
    }

    #[must_use]
    fn convert(&self, y: u8, cb: u8, cr: u8) -> [u8; 3] {
        let y = i32::from(y);
        let (cb, cr) = (usize::from(cb), usize::from(cr));
        [
            range_limit(y + self.cr_r[cr]),
            range_limit(y + ((self.cb_g[cb] + self.cr_g[cr]) >> SCALEBITS)),
            range_limit(y + self.cb_b[cb]),
        ]
    }
}

// ---------------------------------------------------------------------------
// Upsampling
// ---------------------------------------------------------------------------

/// Triangle-filter upsampling by two horizontally, libjpeg's "fancy" variant.
///
/// The output weights are three parts near sample to one part far, and the two
/// halves of each pair carry different rounding biases. Edge columns replicate.
fn fancy_upsample_h2(input: &[u8], output: &mut [u8], width: usize) {
    if width == 1 {
        output[0] = input[0];
        output[1] = input[0];
        return;
    }
    output[0] = input[0];
    output[1] = ((i32::from(input[0]) * 3 + i32::from(input[1]) + 2) >> 2) as u8;
    for column in 1..width - 1 {
        let near = i32::from(input[column]) * 3;
        output[column * 2] = ((near + i32::from(input[column - 1]) + 1) >> 2) as u8;
        output[column * 2 + 1] = ((near + i32::from(input[column + 1]) + 2) >> 2) as u8;
    }
    let last = width - 1;
    output[last * 2] = ((i32::from(input[last]) * 3 + i32::from(input[last - 1]) + 1) >> 2) as u8;
    output[last * 2 + 1] = input[last];
}

/// Triangle-filter upsampling by two in both directions.
///
/// Each output row mixes the source row three-to-one with whichever vertical
/// neighbour it sits nearer, then applies the same horizontal filter. The
/// column sums are formed first so the vertical and horizontal weights compose
/// in one rounding step, which is what libjpeg does and what the biases below
/// depend on.
fn fancy_upsample_h2v2_row(near: &[u8], far: &[u8], output: &mut [u8], width: usize) {
    let column_sum = |column: usize| i32::from(near[column]) * 3 + i32::from(far[column]);
    if width == 1 {
        let value = ((column_sum(0) * 4 + 8) >> 4) as u8;
        output[0] = value;
        output[1] = value;
        return;
    }
    let mut previous = column_sum(0);
    let mut current = previous;
    let mut next = column_sum(1);
    output[0] = ((current * 4 + 8) >> 4) as u8;
    output[1] = ((current * 3 + next + 7) >> 4) as u8;
    previous = current;
    current = next;
    for column in 1..width - 1 {
        next = column_sum(column + 1);
        output[column * 2] = ((current * 3 + previous + 8) >> 4) as u8;
        output[column * 2 + 1] = ((current * 3 + next + 7) >> 4) as u8;
        previous = current;
        current = next;
    }
    let last = width - 1;
    output[last * 2] = ((current * 3 + previous + 8) >> 4) as u8;
    output[last * 2 + 1] = ((current * 4 + 7) >> 4) as u8;
}

// ---------------------------------------------------------------------------
// Huffman decoding
// ---------------------------------------------------------------------------

/// A canonical Huffman table in the form the standard's decoding procedure
/// wants: per code length, the smallest code, the largest code, and where its
/// values start.
#[derive(Clone, Default)]
struct HuffmanTable {
    min_code: [i32; 17],
    max_code: [i32; 17],
    value_offset: [i32; 17],
    values: Vec<u8>,
}

impl HuffmanTable {
    fn build(counts: &[u8; 16], values: Vec<u8>) -> Self {
        let mut table = Self {
            values,
            ..Self::default()
        };
        let mut code = 0i32;
        let mut index = 0i32;
        for length in 1..=16usize {
            let count = i32::from(counts[length - 1]);
            if count == 0 {
                // No code of this length; mark it unmatchable.
                table.max_code[length] = -1;
            } else {
                table.value_offset[length] = index - code;
                table.min_code[length] = code;
                code += count;
                index += count;
                table.max_code[length] = code - 1;
            }
            // The canonical code advances a bit at every length, including the
            // lengths that carry no codes at all.
            code <<= 1;
        }
        table
    }
}

/// Entropy-coded segment reader: MSB-first bits with byte stuffing removed.
struct BitReader<'a> {
    data: &'a [u8],
    position: usize,
    bits: u32,
    count: u32,
}

impl<'a> BitReader<'a> {
    const fn new(data: &'a [u8], position: usize) -> Self {
        Self {
            data,
            position,
            bits: 0,
            count: 0,
        }
    }

    /// Feeds one bit, treating a marker or the end of data as an endless run of
    /// zero bits, which is how libjpeg pads a truncated final block.
    fn bit(&mut self) -> u32 {
        if self.count == 0 {
            let byte = match self.data.get(self.position) {
                Some(0xFF) => {
                    // A stuffed zero means a literal 0xFF; anything else is a
                    // marker, so stop consuming and pad.
                    match self.data.get(self.position + 1) {
                        Some(0x00) => {
                            self.position += 2;
                            0xFF
                        }
                        _ => 0,
                    }
                }
                Some(byte) => {
                    self.position += 1;
                    *byte
                }
                None => 0,
            };
            self.bits = u32::from(byte);
            self.count = 8;
        }
        self.count -= 1;
        (self.bits >> self.count) & 1
    }

    fn receive(&mut self, length: u32) -> i32 {
        let mut value = 0i32;
        for _ in 0..length {
            value = (value << 1) | self.bit() as i32;
        }
        value
    }

    fn decode(&mut self, table: &HuffmanTable) -> Result<u8, JpegError> {
        let mut code = self.bit() as i32;
        for length in 1..=16usize {
            if table.max_code[length] >= code && code >= table.min_code[length] {
                let index = code + table.value_offset[length];
                return table
                    .values
                    .get(usize::try_from(index).map_err(|_| JpegError::BadHuffmanCode)?)
                    .copied()
                    .ok_or(JpegError::BadHuffmanCode);
            }
            code = (code << 1) | self.bit() as i32;
        }
        Err(JpegError::BadHuffmanCode)
    }

    /// Discards buffered bits and steps over a restart marker.
    fn restart(&mut self) {
        self.count = 0;
        while self.position + 1 < self.data.len() {
            if self.data[self.position] == 0xFF {
                let marker = self.data[self.position + 1];
                if (0xD0..=0xD7).contains(&marker) {
                    self.position += 2;
                    return;
                }
                if marker != 0x00 && marker != 0xFF {
                    return;
                }
            }
            self.position += 1;
        }
    }
}

/// Sign-extends an `Sn` magnitude as the standard's `EXTEND` procedure does.
#[must_use]
const fn extend(value: i32, length: u32) -> i32 {
    if length == 0 {
        return 0;
    }
    if value < (1 << (length - 1)) {
        value - (1 << length) + 1
    } else {
        value
    }
}

// ---------------------------------------------------------------------------
// Frame structures
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Component {
    id: u8,
    horizontal: usize,
    vertical: usize,
    quantizer: usize,
    dc_table: usize,
    ac_table: usize,
}

/// One decoded component plane at its own sampling resolution.
struct Plane {
    width: usize,
    height: usize,
    stride: usize,
    samples: Vec<u8>,
}

/// Decodes a baseline JPEG.
///
/// # Errors
///
/// Returns [`JpegError`] for malformed data and for any process outside the
/// documented scope, never a best-effort approximation.
pub fn decode(bytes: &[u8]) -> Result<Decoded, JpegError> {
    if bytes.len() < 2 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return Err(JpegError::MissingSoi);
    }

    let mut quantizers: Vec<Option<[u16; 64]>> = vec![None; 4];
    let mut dc_tables: Vec<Option<HuffmanTable>> = vec![None; 4];
    let mut ac_tables: Vec<Option<HuffmanTable>> = vec![None; 4];
    let mut frame: Option<(usize, usize, Vec<Component>)> = None;
    let mut restart_interval = 0usize;

    let mut at = 2usize;
    loop {
        // Markers may be preceded by fill bytes.
        while bytes.get(at) == Some(&0xFF) && bytes.get(at + 1) == Some(&0xFF) {
            at += 1;
        }
        let (Some(0xFF), Some(&marker)) = (bytes.get(at), bytes.get(at + 1)) else {
            return Err(JpegError::Truncated);
        };
        at += 2;
        match marker {
            // Standalone markers.
            0xD8 => continue,
            0xD9 => break,
            0x01 | 0xD0..=0xD7 => continue,
            _ => {}
        }
        let length = usize::from(u16::from_be_bytes([
            *bytes.get(at).ok_or(JpegError::Truncated)?,
            *bytes.get(at + 1).ok_or(JpegError::Truncated)?,
        ]));
        if length < 2 {
            return Err(JpegError::BadSegmentLength(marker));
        }
        let segment = bytes
            .get(at + 2..at + length)
            .ok_or(JpegError::Truncated)?
            .to_vec();
        at += length;

        match marker {
            // DQT
            0xDB => {
                let mut cursor = 0usize;
                while cursor < segment.len() {
                    let spec = segment[cursor];
                    cursor += 1;
                    let (precision, id) = (usize::from(spec >> 4), usize::from(spec & 15));
                    if id >= 4 {
                        return Err(JpegError::MissingQuantizationTable(id));
                    }
                    let mut table = [0u16; 64];
                    for zigzag in 0..64 {
                        let value = if precision == 0 {
                            let byte = *segment.get(cursor).ok_or(JpegError::Truncated)?;
                            cursor += 1;
                            u16::from(byte)
                        } else {
                            let high = *segment.get(cursor).ok_or(JpegError::Truncated)?;
                            let low = *segment.get(cursor + 1).ok_or(JpegError::Truncated)?;
                            cursor += 2;
                            u16::from_be_bytes([high, low])
                        };
                        // Stored in natural order so the transform can index
                        // coefficients and quantizers identically.
                        table[NATURAL_ORDER[zigzag]] = value;
                    }
                    quantizers[id] = Some(table);
                }
            }
            // DHT
            0xC4 => {
                let mut cursor = 0usize;
                while cursor < segment.len() {
                    let spec = *segment.get(cursor).ok_or(JpegError::Truncated)?;
                    cursor += 1;
                    let (class, id) = (usize::from(spec >> 4), usize::from(spec & 15));
                    if id >= 4 || class >= 2 {
                        return Err(JpegError::MissingHuffmanTable { class, id });
                    }
                    let mut counts = [0u8; 16];
                    counts.copy_from_slice(
                        segment
                            .get(cursor..cursor + 16)
                            .ok_or(JpegError::Truncated)?,
                    );
                    cursor += 16;
                    let total: usize = counts.iter().map(|count| usize::from(*count)).sum();
                    let values = segment
                        .get(cursor..cursor + total)
                        .ok_or(JpegError::Truncated)?
                        .to_vec();
                    cursor += total;
                    let table = HuffmanTable::build(&counts, values);
                    if class == 0 {
                        dc_tables[id] = Some(table);
                    } else {
                        ac_tables[id] = Some(table);
                    }
                }
            }
            // DRI
            0xDD => {
                restart_interval = usize::from(u16::from_be_bytes([
                    *segment.first().ok_or(JpegError::Truncated)?,
                    *segment.get(1).ok_or(JpegError::Truncated)?,
                ]));
            }
            // SOF0 baseline, SOF1 extended sequential: same decoding procedure.
            0xC0 | 0xC1 => {
                let precision = *segment.first().ok_or(JpegError::Truncated)?;
                if precision != 8 {
                    return Err(JpegError::UnsupportedPrecision(precision));
                }
                let height = usize::from(u16::from_be_bytes([segment[1], segment[2]]));
                let width = usize::from(u16::from_be_bytes([segment[3], segment[4]]));
                if width == 0 || height == 0 {
                    return Err(JpegError::ZeroDimension);
                }
                let count = usize::from(segment[5]);
                if count != 1 && count != 3 {
                    return Err(JpegError::UnsupportedComponentCount(count));
                }
                let mut components = Vec::with_capacity(count);
                for index in 0..count {
                    let base = 6 + index * 3;
                    let spec = *segment.get(base + 1).ok_or(JpegError::Truncated)?;
                    components.push(Component {
                        id: segment[base],
                        horizontal: usize::from(spec >> 4),
                        vertical: usize::from(spec & 15),
                        quantizer: usize::from(*segment.get(base + 2).ok_or(JpegError::Truncated)?),
                        dc_table: 0,
                        ac_table: 0,
                    });
                }
                frame = Some((width, height, components));
            }
            // Any other frame marker is a process this decoder will not fake.
            0xC2 | 0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF => {
                return Err(JpegError::UnsupportedProcess(marker));
            }
            // SOS
            0xDA => {
                let (width, height, components) = frame.as_mut().ok_or(JpegError::MissingFrame)?;
                let scan_count = usize::from(*segment.first().ok_or(JpegError::Truncated)?);
                for index in 0..scan_count {
                    let id = *segment.get(1 + index * 2).ok_or(JpegError::Truncated)?;
                    let tables = *segment.get(2 + index * 2).ok_or(JpegError::Truncated)?;
                    for component in components.iter_mut() {
                        if component.id == id {
                            component.dc_table = usize::from(tables >> 4);
                            component.ac_table = usize::from(tables & 15);
                        }
                    }
                }
                let planes = decode_scan(
                    bytes,
                    at,
                    *width,
                    *height,
                    components,
                    &quantizers,
                    &dc_tables,
                    &ac_tables,
                    restart_interval,
                )?;
                return assemble(*width, *height, components, &planes);
            }
            // APPn, COM, and other skippable segments.
            0xC8 | 0xCC | 0xDC | 0xDE | 0xDF | 0xE0..=0xEF | 0xFE => {}
            _ => return Err(JpegError::UnsupportedMarker(marker)),
        }
    }
    Err(JpegError::MissingFrame)
}

/// Decodes the entropy-coded segment into one plane per component.
#[allow(clippy::too_many_arguments)]
fn decode_scan(
    bytes: &[u8],
    start: usize,
    width: usize,
    height: usize,
    components: &[Component],
    quantizers: &[Option<[u16; 64]>],
    dc_tables: &[Option<HuffmanTable>],
    ac_tables: &[Option<HuffmanTable>],
    restart_interval: usize,
) -> Result<Vec<Plane>, JpegError> {
    let max_h = components
        .iter()
        .map(|component| component.horizontal)
        .max()
        .unwrap_or(1);
    let max_v = components
        .iter()
        .map(|component| component.vertical)
        .max()
        .unwrap_or(1);
    for component in components {
        if !matches!(component.horizontal, 1 | 2) || !matches!(component.vertical, 1 | 2) {
            return Err(JpegError::UnsupportedSampling {
                horizontal: component.horizontal,
                vertical: component.vertical,
            });
        }
    }

    let mcus_across = width.div_ceil(8 * max_h);
    let mcus_down = height.div_ceil(8 * max_v);

    let mut planes = Vec::with_capacity(components.len());
    for component in components {
        // Full MCU coverage, so a partial edge MCU still has somewhere to land.
        let stride = mcus_across * component.horizontal * 8;
        let rows = mcus_down * component.vertical * 8;
        planes.push(Plane {
            width: (width * component.horizontal).div_ceil(max_h),
            height: (height * component.vertical).div_ceil(max_v),
            stride,
            samples: vec![0u8; stride * rows],
        });
    }

    let mut reader = BitReader::new(bytes, start);
    let mut predictors = vec![0i32; components.len()];
    let mut until_restart = restart_interval;

    for mcu_y in 0..mcus_down {
        for mcu_x in 0..mcus_across {
            if restart_interval != 0 && until_restart == 0 {
                reader.restart();
                predictors.iter_mut().for_each(|value| *value = 0);
                until_restart = restart_interval;
            }
            if restart_interval != 0 {
                until_restart -= 1;
            }

            for (index, component) in components.iter().enumerate() {
                let quantizer = quantizers
                    .get(component.quantizer)
                    .and_then(Option::as_ref)
                    .ok_or(JpegError::MissingQuantizationTable(component.quantizer))?;
                let dc = dc_tables
                    .get(component.dc_table)
                    .and_then(Option::as_ref)
                    .ok_or(JpegError::MissingHuffmanTable {
                        class: 0,
                        id: component.dc_table,
                    })?;
                let ac = ac_tables
                    .get(component.ac_table)
                    .and_then(Option::as_ref)
                    .ok_or(JpegError::MissingHuffmanTable {
                        class: 1,
                        id: component.ac_table,
                    })?;

                for block_y in 0..component.vertical {
                    for block_x in 0..component.horizontal {
                        let mut coefficients = [0i16; 64];

                        let symbol = reader.decode(dc)?;
                        let magnitude = u32::from(symbol);
                        let difference = extend(reader.receive(magnitude), magnitude);
                        predictors[index] += difference;
                        coefficients[0] = predictors[index] as i16;

                        let mut position = 1usize;
                        while position < 64 {
                            let symbol = reader.decode(ac)?;
                            let run = usize::from(symbol >> 4);
                            let size = u32::from(symbol & 15);
                            if size == 0 {
                                if run != 15 {
                                    break;
                                }
                                position += 16;
                                continue;
                            }
                            position += run;
                            if position >= 64 {
                                break;
                            }
                            coefficients[NATURAL_ORDER[position]] =
                                extend(reader.receive(size), size) as i16;
                            position += 1;
                        }

                        let mut block = [0u8; 64];
                        inverse_dct(&coefficients, quantizer, &mut block);

                        let plane = &mut planes[index];
                        let origin_x = (mcu_x * component.horizontal + block_x) * 8;
                        let origin_y = (mcu_y * component.vertical + block_y) * 8;
                        for row in 0..8 {
                            let target = (origin_y + row) * plane.stride + origin_x;
                            plane.samples[target..target + 8]
                                .copy_from_slice(&block[row * 8..row * 8 + 8]);
                        }
                    }
                }
            }
        }
    }

    Ok(planes)
}

/// Upsamples every plane to full resolution and converts to output samples.
fn assemble(
    width: usize,
    height: usize,
    components: &[Component],
    planes: &[Plane],
) -> Result<Decoded, JpegError> {
    let max_h = components
        .iter()
        .map(|component| component.horizontal)
        .max()
        .unwrap_or(1);
    let max_v = components
        .iter()
        .map(|component| component.vertical)
        .max()
        .unwrap_or(1);

    let mut upsampled: Vec<Vec<u8>> = Vec::with_capacity(planes.len());
    for (component, plane) in components.iter().zip(planes) {
        upsampled.push(upsample_plane(
            plane,
            width,
            height,
            max_h / component.horizontal,
            max_v / component.vertical,
        )?);
    }

    let count = components.len();
    let mut samples = vec![0u8; width * height * count];
    if count == 1 {
        samples.copy_from_slice(&upsampled[0]);
    } else {
        let tables = ColourTables::new();
        for index in 0..width * height {
            let rgb = tables.convert(
                upsampled[0][index],
                upsampled[1][index],
                upsampled[2][index],
            );
            samples[index * 3..index * 3 + 3].copy_from_slice(&rgb);
        }
    }

    Ok(Decoded {
        width,
        height,
        components: count,
        samples,
    })
}

/// Expands one plane to the full image grid.
fn upsample_plane(
    plane: &Plane,
    width: usize,
    height: usize,
    horizontal: usize,
    vertical: usize,
) -> Result<Vec<u8>, JpegError> {
    let row_of = |row: usize| -> &[u8] {
        let clamped = row.min(plane.height - 1);
        &plane.samples[clamped * plane.stride..clamped * plane.stride + plane.width]
    };

    match (horizontal, vertical) {
        (1, 1) => {
            let mut output = vec![0u8; width * height];
            for row in 0..height {
                let source = row_of(row);
                let target = &mut output[row * width..row * width + width];
                for (column, sample) in target.iter_mut().enumerate() {
                    *sample = source[column.min(plane.width - 1)];
                }
            }
            Ok(output)
        }
        (2, 1) => {
            let mut output = vec![0u8; width * height];
            let mut expanded = vec![0u8; plane.width * 2];
            for row in 0..height {
                fancy_upsample_h2(row_of(row), &mut expanded, plane.width);
                let target = &mut output[row * width..row * width + width];
                let take = width.min(expanded.len());
                target[..take].copy_from_slice(&expanded[..take]);
            }
            Ok(output)
        }
        (2, 2) => {
            let mut output = vec![0u8; width * height];
            let mut expanded = vec![0u8; plane.width * 2];
            for row in 0..height {
                // Each output row pairs its source row with the vertical
                // neighbour it lies nearer; edges replicate.
                let source_row = row / 2;
                let neighbour = if row % 2 == 0 {
                    source_row.saturating_sub(1)
                } else {
                    (source_row + 1).min(plane.height - 1)
                };
                fancy_upsample_h2v2_row(
                    row_of(source_row),
                    row_of(neighbour),
                    &mut expanded,
                    plane.width,
                );
                let target = &mut output[row * width..row * width + width];
                let take = width.min(expanded.len());
                target[..take].copy_from_slice(&expanded[..take]);
            }
            Ok(output)
        }
        _ => Err(JpegError::UnsupportedSampling {
            horizontal,
            vertical,
        }),
    }
}

/// Decodes and reduces to Audiveris's grayscale rule: the maximum RGB channel.
///
/// # Errors
///
/// Propagates any [`JpegError`] from [`decode`].
pub fn decode_max_channel_gray(bytes: &[u8]) -> Result<(usize, usize, Vec<u8>), JpegError> {
    let decoded = decode(bytes)?;
    let gray = match decoded.components {
        1 => decoded.samples,
        _ => decoded
            .samples
            .chunks_exact(decoded.components)
            .map(|sample| sample[0].max(sample[1]).max(sample[2]))
            .collect(),
    };
    Ok((decoded.width, decoded.height, gray))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_limit_wraps_the_way_the_transform_relies_on() {
        // Ordinary centred values pass through.
        assert_eq!(range_limit_idct(0), 128);
        assert_eq!(range_limit_idct(100), 228);
        // Negative values arrive as large masked indices and must come back.
        assert_eq!(range_limit_idct(-10), 118);
        assert_eq!(range_limit_idct(-128), 0);
        // Overflow saturates rather than wrapping to a dark value.
        assert_eq!(range_limit_idct(200), 255);
        // Far overflow lands in the zero band, as libjpeg's table does.
        assert_eq!(range_limit_idct(-200), 0);
    }

    #[test]
    fn extend_matches_the_standard_procedure() {
        assert_eq!(extend(0, 0), 0);
        assert_eq!(extend(1, 1), 1);
        assert_eq!(extend(0, 1), -1);
        assert_eq!(extend(0b101, 3), 5);
        assert_eq!(extend(0b001, 3), -6);
    }

    #[test]
    fn flat_block_inverse_transforms_to_its_dc_level() {
        let mut coefficients = [0i16; 64];
        coefficients[0] = 16;
        let mut quantizers = [1u16; 64];
        quantizers[0] = 8;
        let mut block = [0u8; 64];
        inverse_dct(&coefficients, &quantizers, &mut block);
        // DC of 16*8 spreads to 16 above the centre across the whole block.
        assert!(block.iter().all(|sample| *sample == 144), "{block:?}");
    }
}
