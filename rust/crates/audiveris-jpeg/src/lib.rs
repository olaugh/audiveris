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
//! # Damaged input
//!
//! Truncation is reproduced: once the scan runs out, libjpeg stops decoding and
//! leaves every later block zeroed, which the transform renders as flat
//! mid-grey, and so does this decoder. Padding with zero bits and carrying on
//! instead yields plausible-looking noise, and a corpus of scans is full of
//! truncated files.
//!
//! Mid-scan corruption is reproduced too. Two of libjpeg's habits there are
//! more permissive than a reading of the standard suggests: it consumes a
//! coefficient's magnitude bits even when the preceding run has pushed the
//! index past the end of the block, and it writes that stranded coefficient to
//! index 63 through sixteen padding entries in its zig-zag table rather than
//! discarding it. Get either wrong and the bitstream falls one field out of
//! step with libjpeg's, so a single corrupt run silently rewrites the rest of
//! the scan.
//!
//! Restart markers recover the same way, including the part that is easy to
//! miss: a successful restart *clears* the out-of-data flag, so a segment that
//! ran short stops rendering flat grey as soon as the next marker arrives.
//!
//! This was the long tail called out when the decoder was proposed: matching
//! libjpeg on clean input is mechanical, matching its error recovery is not.
//! It is nonetheless matched, on every input the fuzzer has produced.
//!
//! # What parity turned out to mean
//!
//! Every divergence differential fuzzing found after the first build was in one
//! of three places, and none of them was an algorithm. They are worth naming,
//! because each is invisible to reading the transform code:
//!
//! - **Which method libjpeg selects.** Both of its two-times upsamplers are
//!   chosen only when a component's downsampled width exceeds two, and it
//!   replicates instead below that. Filtering unconditionally is wrong for 4:2:0
//!   on any image at most four pixels wide -- while the filter itself is right.
//! - **The integer widths.** Dequantization is an `int` multiply, the transform
//!   that follows is `JLONG`, and the result narrows back to `int` twice. On a
//!   file whose coefficients approach the coding limit that is the difference
//!   between agreement and 496 wrong samples.
//! - **Which files are accepted at all.** libjpeg validates scan components,
//!   segment lengths, Huffman tables, and duplicate markers, and a decoder that
//!   skips those checks produces an image where Audiveris produces an error. No
//!   sample comparison can see that, so `oracle/jpeg-verdicts.txt` pins it
//!   separately, against Java rather than against libjpeg.
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
    /// A Huffman table that over-subscribes the code space, or carries more
    /// symbols than a table can hold. libjpeg validates this when the scan
    /// starts, so such a file fails there even if the table is never used.
    BadHuffmanTable,
    /// A coefficient magnitude category outside the standard's range, which
    /// only a corrupt Huffman table can produce.
    InvalidMagnitudeCategory(u8),
    ZeroDimension,
    /// A scan selected a component the frame never declared, or selected one
    /// twice. libjpeg treats this as fatal.
    UnknownScanComponent(u8),
    /// A scan covering fewer components than the frame: the non-interleaved
    /// sequential process, which has a different MCU layout.
    NonInterleavedScan {
        scan: usize,
        frame: usize,
    },
    /// A second frame header. libjpeg allows exactly one.
    DuplicateFrame,
    /// A second start-of-image marker, which libjpeg calls fatal.
    DuplicateStartOfImage,
    /// A second scan in a sequential file, where libjpeg expects end-of-image.
    ExpectedEndOfImage,
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
            Self::BadHuffmanTable => f.write_str("bogus Huffman table definition"),
            Self::InvalidMagnitudeCategory(category) => {
                write!(
                    f,
                    "coefficient magnitude category {category} is out of range"
                )
            }
            Self::ZeroDimension => f.write_str("frame declared a zero dimension"),
            Self::UnknownScanComponent(id) => {
                write!(
                    f,
                    "scan selected component {id}, which the frame does not declare exactly once"
                )
            }
            Self::NonInterleavedScan { scan, frame } => write!(
                f,
                "scan covers {scan} of the frame's {frame} components; only the fully \
                 interleaved sequential scan is decoded here"
            ),
            Self::DuplicateFrame => f.write_str("a second frame header"),
            Self::DuplicateStartOfImage => f.write_str("a second start-of-image marker"),
            Self::ExpectedEndOfImage => {
                f.write_str("a second scan where end-of-image was expected")
            }
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

/// Zig-zag scan position to natural row-major position, with sixteen entries of
/// padding.
///
/// A corrupt run length can push the coefficient index past the end of a block.
/// libjpeg carries the same padding, every entry of it the last coefficient, so
/// an over-long run lands on coefficient 63 instead of being discarded. That is
/// observable: dropping the value instead leaves a block one coefficient short
/// of libjpeg's. A run adds at most fifteen to an index below sixty-four, so
/// sixteen spare entries cover every reachable case.
const NATURAL_ORDER: [usize; 80] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63,
    63, 63, 63, 63, 63, 63,
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

/// Widest plane for which libjpeg declines the fancy upsamplers.
///
/// Both two-times paths test `downsampled_width > 2` before choosing the
/// triangle filter, so a plane of one or two samples is replicated instead.
const FANCY_UPSAMPLE_MINIMUM_WIDTH: usize = 2;

/// Fractional bits in the fixed-point multipliers.
const CONST_BITS: i32 = 13;
/// Extra fractional bits carried between the two passes.
const PASS1_BITS: i32 = 2;

/// `round(value * 2^CONST_BITS)`, the multipliers of the integer inverse DCT.
///
/// Each is the scaled form of an exact trigonometric quantity of the
/// even/odd decomposition, listed with the real number it comes from.
const FIX_0_298631336: i64 = 2446; // 0.298631336
const FIX_0_390180644: i64 = 3196; // 0.390180644
const FIX_0_541196100: i64 = 4433; // 0.541196100
const FIX_0_765366865: i64 = 6270; // 0.765366865
const FIX_0_899976223: i64 = 7373; // 0.899976223
const FIX_1_175875602: i64 = 9633; // 1.175875602
const FIX_1_501321110: i64 = 12299; // 1.501321110
const FIX_1_847759065: i64 = 15137; // 1.847759065
const FIX_1_961570560: i64 = 16069; // 1.961570560
const FIX_2_053119869: i64 = 16819; // 2.053119869
const FIX_2_562915447: i64 = 20995; // 2.562915447
const FIX_3_072711026: i64 = 25172; // 3.072711026

/// Round-to-nearest right shift, the descaling step of the integer transform.
#[must_use]
const fn descale(value: i64, bits: i32) -> i64 {
    value.wrapping_add(1 << (bits - 1)) >> bits
}

/// Odd-part butterfly shared by both passes.
///
/// Returns the four odd-index outputs in coefficient order.
#[must_use]
fn odd_part(mut tmp0: i64, mut tmp1: i64, mut tmp2: i64, mut tmp3: i64) -> [i64; 4] {
    let mut z1 = tmp0.wrapping_add(tmp3);
    let mut z2 = tmp1.wrapping_add(tmp2);
    let mut z3 = tmp0.wrapping_add(tmp2);
    let mut z4 = tmp1.wrapping_add(tmp3);
    let z5 = z3.wrapping_add(z4).wrapping_mul(FIX_1_175875602);

    tmp0 = tmp0.wrapping_mul(FIX_0_298631336);
    tmp1 = tmp1.wrapping_mul(FIX_2_053119869);
    tmp2 = tmp2.wrapping_mul(FIX_3_072711026);
    tmp3 = tmp3.wrapping_mul(FIX_1_501321110);
    z1 = z1.wrapping_mul(-FIX_0_899976223);
    z2 = z2.wrapping_mul(-FIX_2_562915447);
    z3 = z3.wrapping_mul(-FIX_1_961570560);
    z4 = z4.wrapping_mul(-FIX_0_390180644);

    z3 = z3.wrapping_add(z5);
    z4 = z4.wrapping_add(z5);

    [
        tmp0.wrapping_add(z1).wrapping_add(z3),
        tmp1.wrapping_add(z2).wrapping_add(z4),
        tmp2.wrapping_add(z2).wrapping_add(z3),
        tmp3.wrapping_add(z1).wrapping_add(z4),
    ]
}

/// Even-part butterfly shared by both passes.
///
/// Returns the four running sums in the order the outputs pair with the odd
/// part: `[tmp10, tmp11, tmp12, tmp13]`.
#[must_use]
fn even_part(in0: i64, in2: i64, in4: i64, in6: i64) -> [i64; 4] {
    let z1 = in2.wrapping_add(in6).wrapping_mul(FIX_0_541196100);
    let tmp2 = z1.wrapping_add(in6.wrapping_mul(-FIX_1_847759065));
    let tmp3 = z1.wrapping_add(in2.wrapping_mul(FIX_0_765366865));

    let tmp0 = in0.wrapping_add(in4).wrapping_shl(CONST_BITS as u32);
    let tmp1 = in0.wrapping_sub(in4).wrapping_shl(CONST_BITS as u32);

    [
        tmp0.wrapping_add(tmp3),
        tmp1.wrapping_add(tmp2),
        tmp1.wrapping_sub(tmp2),
        tmp0.wrapping_sub(tmp3),
    ]
}

/// libjpeg's `jpeg_idct_islow`: dequantize and inverse transform one block.
///
/// The integer widths here are copied from libjpeg deliberately, because on a
/// malformed file they are observable. libjpeg dequantizes in `int` and carries
/// the butterflies in `JLONG`, which is `long` -- 64 bits on the platforms that
/// matter here -- then narrows back to `int` for the inter-pass workspace and
/// again for the range-limit index. So a coefficient/quantizer product wraps at
/// 32 bits, the transform that follows does not wrap at all, and the pass-1
/// result is truncated to 32 bits on its way into the workspace. Carrying the
/// whole thing in 32 bits looks equivalent and is not: differential fuzzing
/// found a 1x9 file whose chroma blocks hold coefficients near 8191, where the
/// products reach far enough past 2^31 for the difference to reach the output.
///
/// All of it wraps rather than panicking, which is both what C does and what a
/// decoder reading untrusted scans has to do.
///
/// Both passes take the shortcut for an all-zero AC set, which is an algebraic
/// identity rather than an approximation.
fn inverse_dct(coefficients: &[i16; 64], quantizers: &[u16; 64], output: &mut [u8; 64]) {
    // `int workspace[DCTSIZE2]` in libjpeg: 32 bits, narrower than the
    // arithmetic that produces it.
    let mut workspace = [0i32; 64];

    // Pass 1: columns, leaving PASS1_BITS of extra fraction.
    for column in 0..8 {
        // Dequantization is an `int` multiply in libjpeg, so it wraps at 32
        // bits before the transform widens it.
        let at = |row: usize| -> i32 {
            i32::from(coefficients[row * 8 + column])
                .wrapping_mul(i32::from(quantizers[row * 8 + column]))
        };
        let wide = |row: usize| i64::from(at(row));
        if (1..8).all(|row| coefficients[row * 8 + column] == 0) {
            let dc = at(0).wrapping_shl(PASS1_BITS as u32);
            for row in 0..8 {
                workspace[row * 8 + column] = dc;
            }
            continue;
        }

        let [tmp10, tmp11, tmp12, tmp13] = even_part(wide(0), wide(2), wide(4), wide(6));
        let [odd0, odd1, odd2, odd3] = odd_part(wide(7), wide(5), wide(3), wide(1));

        let shift = CONST_BITS - PASS1_BITS;
        let mut store = |row: usize, value: i64| {
            workspace[row * 8 + column] = descale(value, shift) as i32;
        };
        store(0, tmp10.wrapping_add(odd3));
        store(7, tmp10.wrapping_sub(odd3));
        store(1, tmp11.wrapping_add(odd2));
        store(6, tmp11.wrapping_sub(odd2));
        store(2, tmp12.wrapping_add(odd1));
        store(5, tmp12.wrapping_sub(odd1));
        store(3, tmp13.wrapping_add(odd0));
        store(4, tmp13.wrapping_sub(odd0));
    }

    // Pass 2: rows, descaling the whole accumulated fraction away.
    let shift = CONST_BITS + PASS1_BITS + 3;
    for row in 0..8 {
        let line = &workspace[row * 8..row * 8 + 8];
        let wide = |column: usize| i64::from(line[column]);
        if line[1..].iter().all(|value| *value == 0) {
            let dc = range_limit_idct(descale(wide(0), PASS1_BITS + 3) as i32);
            for column in 0..8 {
                output[row * 8 + column] = dc;
            }
            continue;
        }

        let [tmp10, tmp11, tmp12, tmp13] = even_part(wide(0), wide(2), wide(4), wide(6));
        let [odd0, odd1, odd2, odd3] = odd_part(wide(7), wide(5), wide(3), wide(1));

        let mut emit = |column: usize, value: i64| {
            output[row * 8 + column] = range_limit_idct(descale(value, shift) as i32);
        };
        emit(0, tmp10.wrapping_add(odd3));
        emit(7, tmp10.wrapping_sub(odd3));
        emit(1, tmp11.wrapping_add(odd2));
        emit(6, tmp11.wrapping_sub(odd2));
        emit(2, tmp12.wrapping_add(odd1));
        emit(5, tmp12.wrapping_sub(odd1));
        emit(3, tmp13.wrapping_add(odd0));
        emit(4, tmp13.wrapping_sub(odd0));
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

/// Plain replication, which libjpeg uses instead of the triangle filter when a
/// plane is at most two samples wide.
///
/// Both of libjpeg's two-times paths select the fancy filter only when
/// `downsampled_width > 2`, and fall back here otherwise. It is not a rounding
/// detail: replication and the triangle filter give visibly different answers,
/// and a decoder that always filters disagrees on any image whose chroma plane
/// is that narrow -- which for 4:2:0 means any image at most four pixels wide.
fn box_upsample_h2(input: &[u8], output: &mut [u8], width: usize) {
    for column in 0..width {
        output[column * 2] = input[column];
        output[column * 2 + 1] = input[column];
    }
}

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
    /// Codes per length, kept for the scan-start validation below.
    counts: [u8; 16],
}

impl HuffmanTable {
    /// Builds the decoding table from a `DHT` segment.
    ///
    /// Only the symbol-count limit is checked here, because that is the only
    /// one libjpeg makes while *parsing* the segment. The rest of its
    /// validation happens when a scan starts, and applies to referenced tables
    /// only; see [`HuffmanTable::validate`].
    fn build(counts: &[u8; 16], values: Vec<u8>) -> Result<Self, JpegError> {
        if values.len() > 256 {
            return Err(JpegError::BadHuffmanTable);
        }
        let mut table = Self {
            counts: *counts,
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
            code = code.wrapping_shl(1);
        }
        Ok(table)
    }

    /// The checks libjpeg defers to the start of a scan.
    ///
    /// These are parity, not defensive tidiness: libjpeg makes them before
    /// decoding a single MCU, so a file carrying an impossible table fails even
    /// when none of its codes would ever have been read. Accepting one means
    /// producing an image where Audiveris raises `Bogus Huffman table
    /// definition`. Equally, the checks are *not* made on a table the scan
    /// never names, so validating at parse time would reject files libjpeg
    /// accepts. Both halves of that are load-bearing.
    fn validate(&self, is_dc: bool) -> Result<(), JpegError> {
        let mut code = 0i32;
        for length in 1..=16usize {
            code += i32::from(self.counts[length - 1]);
            // The all-ones code of any length is reserved, so the codes
            // assigned so far must leave at least one spare. A table that fills
            // the space over-subscribes it.
            if code >= 1i32 << length {
                return Err(JpegError::BadHuffmanTable);
            }
            code <<= 1;
        }
        // A DC symbol is a coefficient magnitude category, which the standard
        // caps at 15. The AC side has no equivalent limit: its symbols are a
        // run and a size packed into the byte, so every value is in range.
        if is_dc && self.values.iter().any(|value| *value > 15) {
            return Err(JpegError::BadHuffmanTable);
        }
        Ok(())
    }
}

/// Entropy-coded segment reader: MSB-first bits with byte stuffing removed.
struct BitReader<'a> {
    data: &'a [u8],
    position: usize,
    bits: u32,
    count: u32,
    /// The marker that ended the entropy data, held unread.
    ///
    /// libjpeg keeps the same field (`cinfo->unread_marker`) because a restart
    /// needs to know whether the decoder stopped *at* a marker or merely ran
    /// short of bits: the two resume differently.
    pending: Option<u8>,
    /// Where `pending`'s run of `FF` bytes began, so the caller can hand the
    /// marker back to its own walk.
    marker_at: usize,
    /// Which `RSTn` the next restart expects, counting 0..7 and wrapping.
    next_restart: u8,
    /// Set once the reader has been asked for a bit the scan does not contain.
    ///
    /// libjpeg tracks the same condition and stops decoding entirely once it
    /// holds, leaving every later block zeroed, which an inverse transform turns
    /// into a flat mid-grey. Padding with zero bits and carrying on instead
    /// decodes plausible-looking noise, so matching this is what parity on a
    /// truncated file means -- and truncated files are exactly what a corpus of
    /// scans contains.
    insufficient: bool,
}

impl<'a> BitReader<'a> {
    const fn new(data: &'a [u8], position: usize) -> Self {
        Self {
            data,
            position,
            bits: 0,
            count: 0,
            pending: None,
            marker_at: 0,
            next_restart: 0,
            insufficient: false,
        }
    }

    /// Consumes one byte of entropy data, or reports the marker that ends it.
    ///
    /// Running out of input is not distinguished from meeting an end-of-image:
    /// libjpeg's source manager fabricates `FF D9` when the data is exhausted,
    /// so the rest of the decoder sees the two identically, and restart
    /// handling depends on that.
    fn next_byte(&mut self) -> Option<u8> {
        if self.pending.is_some() {
            return None;
        }
        let start = self.position;
        let fabricate = |reader: &mut Self| {
            reader.pending = Some(0xD9);
            reader.marker_at = reader.data.len();
            None
        };
        let Some(&byte) = self.data.get(self.position) else {
            return fabricate(self);
        };
        self.position += 1;
        if byte != 0xFF {
            return Some(byte);
        }
        // Multiple FFs followed by a zero count as one stuffed data byte, which
        // is not valid per the standard but is what libjpeg accepts.
        loop {
            let Some(&code) = self.data.get(self.position) else {
                return fabricate(self);
            };
            self.position += 1;
            match code {
                0xFF => {}
                0x00 => return Some(0xFF),
                _ => {
                    self.pending = Some(code);
                    self.marker_at = start;
                    return None;
                }
            }
        }
    }

    /// Feeds one bit, treating a marker or the end of data as an endless run of
    /// zero bits, which is how libjpeg pads a truncated final block.
    fn bit(&mut self) -> u32 {
        if self.count == 0 {
            let byte = match self.next_byte() {
                Some(byte) => byte,
                None => {
                    self.insufficient = true;
                    0
                }
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
            value = value.wrapping_shl(1) | self.bit() as i32;
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
            code = code.wrapping_shl(1) | self.bit() as i32;
        }
        Err(JpegError::BadHuffmanCode)
    }

    /// Scans forward to the next marker, as libjpeg's `next_marker` does.
    ///
    /// Anything that is not a marker is discarded -- libjpeg counts those bytes
    /// and warns about them -- and `FF 00` is stuffed data rather than a marker,
    /// so the search continues past it.
    fn find_marker(&mut self) {
        // `next_byte` yields data bytes until it meets a marker, which it
        // records in `pending` and reports by returning nothing.
        while self.next_byte().is_some() {}
    }

    /// Steps over a restart marker, recovering the way libjpeg recovers.
    ///
    /// Three details here are behaviour, not bookkeeping. The restart markers
    /// are numbered, and meeting an unexpected one puts libjpeg into a
    /// resynchronisation policy rather than a plain skip. Partial bits are
    /// discarded. And most visibly, a successful restart *clears* the
    /// out-of-data flag: a scan that ran short before the marker resumes
    /// decoding after it, instead of rendering flat grey to the end of the
    /// image. A file with a restart interval and a damaged segment therefore
    /// recovers here exactly where it recovers there.
    fn restart(&mut self) {
        self.count = 0;
        if self.pending.is_none() {
            self.find_marker();
        }
        if self.pending == Some(0xD0 + self.next_restart) {
            // The expected marker: swallow it and carry on.
            self.pending = None;
        } else {
            self.resync();
        }
        self.next_restart = (self.next_restart + 1) & 7;
        // Left set if the restart landed on a marker it did not consume, which
        // means the next segment is empty and its pixels would be invented.
        if self.pending.is_none() {
            self.insufficient = false;
        }
    }

    /// libjpeg's `jpeg_resync_to_restart`, the default recovery policy.
    ///
    /// The question it answers is whether the marker in hand is worth stopping
    /// for. One of the next two expected restarts means the encoder skipped
    /// ahead, so it is left unread for the decoder to reach in order. One of the
    /// previous two means this one is stale, so it advances past it and asks
    /// again. Anything else is treated as the restart that was wanted.
    fn resync(&mut self) {
        /// The three outcomes libjpeg's policy can reach, in its own terms.
        enum Action {
            /// Treat this as the wanted restart: discard it and resume.
            Resume,
            /// Stale or unusable: step to the next marker and decide again.
            Advance,
            /// Worth stopping for: leave it unread.
            Stop,
        }

        let desired = self.next_restart;
        let restart = |offset: u8| 0xD0 + ((desired + offset) & 7);
        loop {
            let Some(marker) = self.pending else { return };
            let action = if marker < 0xC0 {
                // Below SOF0 is not a marker any recovery can use.
                Action::Advance
            } else if !(0xD0..=0xD7).contains(&marker) {
                Action::Stop
            } else if marker == restart(1) || marker == restart(2) {
                // The encoder skipped ahead; reach these in order instead.
                Action::Stop
            } else if marker == restart(7) || marker == restart(6) {
                // Already passed, so this one is stale.
                Action::Advance
            } else {
                Action::Resume
            };
            match action {
                Action::Stop => return,
                Action::Resume => {
                    self.pending = None;
                    return;
                }
                Action::Advance => {
                    self.pending = None;
                    self.find_marker();
                }
            }
        }
    }

    /// Where the caller's marker walk should resume.
    ///
    /// A marker the entropy decoder stopped at has not been consumed by anyone,
    /// so the walk has to see it; otherwise the walk starts from wherever the
    /// entropy data left off.
    const fn scan_end(&self) -> usize {
        match self.pending {
            Some(_) => self.marker_at,
            None => self.position,
        }
    }
}

/// Sign-extends an `Sn` magnitude as the standard's `EXTEND` procedure does.
///
/// Shifts wrap rather than overflow. Callers reject out-of-range categories
/// before reaching here, but a decoder that reads untrusted scans should not
/// depend on a caller's check to stay panic-free.
#[must_use]
const fn extend(value: i32, length: u32) -> i32 {
    if length == 0 {
        return 0;
    }
    if value < 1i32.wrapping_shl(length - 1) {
        value
            .wrapping_sub(1i32.wrapping_shl(length))
            .wrapping_add(1)
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
    let decoded = decode_inner(bytes, false)?;
    Ok(Decoded {
        width: decoded.width,
        height: decoded.height,
        components: decoded.components,
        samples: decoded.samples,
    })
}

/// Shared body of [`decode`] and [`decode_component_planes`].
struct DecodedInner {
    width: usize,
    height: usize,
    components: usize,
    samples: Vec<u8>,
    planes: Vec<Vec<u8>>,
}

fn decode_inner(bytes: &[u8], keep_planes: bool) -> Result<DecodedInner, JpegError> {
    if bytes.len() < 2 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return Err(JpegError::MissingSoi);
    }

    let mut quantizers: Vec<Option<[u16; 64]>> = vec![None; 4];
    let mut dc_tables: Vec<Option<HuffmanTable>> = vec![None; 4];
    let mut ac_tables: Vec<Option<HuffmanTable>> = vec![None; 4];
    let mut frame: Option<(usize, usize, Vec<Component>)> = None;
    let mut restart_interval = 0usize;

    // Set once the scan has been decoded. Everything after it is still parsed,
    // because libjpeg parses it too and can fail there; see `next_marker`.
    let mut scanned: Option<Vec<Plane>> = None;

    let mut at = 2usize;
    loop {
        let Some((marker, next)) = next_marker(bytes, at) else {
            // Running out of data is not itself a failure: libjpeg's source
            // manager fabricates an end-of-image when the input is exhausted,
            // which is how a truncated scan still yields an image. Before the
            // scan there is nothing to yield, so it stays an error.
            if scanned.is_some() {
                break;
            }
            return Err(JpegError::Truncated);
        };
        at = next;
        match marker {
            // The opening SOI was consumed before this loop, so any SOI reached
            // here is a second one, which libjpeg calls fatal.
            0xD8 => return Err(JpegError::DuplicateStartOfImage),
            0xD9 => break,
            // Standalone markers.
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
                // libjpeg requires the tables to consume the declared length
                // exactly, and treats any remainder as fatal.
                if cursor != segment.len() {
                    return Err(JpegError::BadSegmentLength(marker));
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
                    let table = HuffmanTable::build(&counts, values)?;
                    if class == 0 {
                        dc_tables[id] = Some(table);
                    } else {
                        ac_tables[id] = Some(table);
                    }
                }
                if cursor != segment.len() {
                    return Err(JpegError::BadSegmentLength(marker));
                }
            }
            // DRI
            0xDD => {
                // Fixed length in libjpeg, and a disagreement is fatal there.
                if segment.len() != 2 {
                    return Err(JpegError::BadSegmentLength(marker));
                }
                restart_interval = usize::from(u16::from_be_bytes([segment[0], segment[1]]));
            }
            // SOF0 baseline, SOF1 extended sequential: same decoding procedure.
            0xC0 | 0xC1 => {
                if frame.is_some() {
                    return Err(JpegError::DuplicateFrame);
                }
                // Precision, dimensions, and component count occupy six bytes
                // before the per-component descriptors.
                let header: &[u8; 6] = segment
                    .get(..6)
                    .and_then(|header| header.try_into().ok())
                    .ok_or(JpegError::Truncated)?;
                let precision = header[0];
                if precision != 8 {
                    return Err(JpegError::UnsupportedPrecision(precision));
                }
                let height = usize::from(u16::from_be_bytes([header[1], header[2]]));
                let width = usize::from(u16::from_be_bytes([header[3], header[4]]));
                if width == 0 || height == 0 {
                    return Err(JpegError::ZeroDimension);
                }
                let count = usize::from(header[5]);
                if count != 1 && count != 3 {
                    return Err(JpegError::UnsupportedComponentCount(count));
                }
                if segment.len() != 6 + count * 3 {
                    return Err(JpegError::BadSegmentLength(marker));
                }
                let mut components = Vec::with_capacity(count);
                for descriptor in segment[6..].chunks_exact(3) {
                    let spec = descriptor[1];
                    components.push(Component {
                        id: descriptor[0],
                        horizontal: usize::from(spec >> 4),
                        vertical: usize::from(spec & 15),
                        quantizer: usize::from(descriptor[2]),
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
                // A sequential file has exactly one scan, so libjpeg treats a
                // second as fatal rather than as another pass to accumulate.
                if scanned.is_some() {
                    return Err(JpegError::ExpectedEndOfImage);
                }
                let (width, height, components) = frame.as_mut().ok_or(JpegError::MissingFrame)?;
                let scan_count = usize::from(*segment.first().ok_or(JpegError::Truncated)?);
                // libjpeg checks the declared length against the component
                // count and treats a disagreement as fatal, so accepting it
                // here would mean decoding a file it refuses.
                if segment.len() != 4 + scan_count * 2 {
                    return Err(JpegError::BadSegmentLength(marker));
                }
                // Only the fully interleaved scan is decoded. A scan that names
                // fewer components than the frame is the non-interleaved
                // sequential process: valid JPEG, a different MCU layout, and
                // several scans to assemble. It is refused rather than decoded
                // as if it were interleaved.
                if scan_count != components.len() {
                    return Err(JpegError::NonInterleavedScan {
                        scan: scan_count,
                        frame: components.len(),
                    });
                }
                let mut selected = vec![false; components.len()];
                for index in 0..scan_count {
                    let id = segment[1 + index * 2];
                    let tables = segment[2 + index * 2];
                    // First match wins, as in libjpeg, which matters only when
                    // a malformed frame repeats a component id.
                    let position = components
                        .iter()
                        .position(|component| component.id == id)
                        .ok_or(JpegError::UnknownScanComponent(id))?;
                    // Each frame component exactly once; anything else is a
                    // layout this decoder does not reproduce.
                    if std::mem::replace(&mut selected[position], true) {
                        return Err(JpegError::UnknownScanComponent(id));
                    }
                    components[position].dc_table = usize::from(tables >> 4);
                    components[position].ac_table = usize::from(tables & 15);
                }
                let (planes, end) = decode_scan(
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
                scanned = Some(planes);
                at = end;
            }
            // APPn, COM, and other skippable segments.
            0xC8 | 0xCC | 0xDC | 0xDE | 0xDF | 0xE0..=0xEF | 0xFE => {}
            _ => return Err(JpegError::UnsupportedMarker(marker)),
        }
    }

    let planes = scanned.ok_or(JpegError::MissingFrame)?;
    let (width, height, components) = frame.as_ref().ok_or(JpegError::MissingFrame)?;
    assemble(*width, *height, components, &planes, keep_planes)
}

/// Advances to the next marker, exactly as libjpeg's `next_marker` does.
///
/// Two behaviours here are load-bearing rather than tidy. Bytes that are not
/// part of a marker are skipped -- libjpeg counts them and warns "extraneous
/// bytes before marker", which is how a scan with corruption in it still
/// resynchronises. And `FF 00` is stuffed data, not a marker, so the search
/// continues past it; treating it as one would end the file early.
///
/// Returns the marker and the offset just past it, or `None` once the data runs
/// out. libjpeg's source manager fabricates an end-of-image at that point, so
/// the caller treats exhaustion the same way it treats `D9`.
fn next_marker(bytes: &[u8], mut at: usize) -> Option<(u8, usize)> {
    loop {
        while *bytes.get(at)? != 0xFF {
            at += 1;
        }
        // Any number of fill bytes may precede the marker code.
        while *bytes.get(at)? == 0xFF {
            at += 1;
        }
        let marker = *bytes.get(at)?;
        at += 1;
        if marker != 0x00 {
            return Some((marker, at));
        }
    }
}

/// Decodes the entropy-coded segment into one plane per component.
///
/// Returns the planes and the offset the entropy decoder stopped at, so the
/// caller can resume its marker walk where libjpeg's would.
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
) -> Result<(Vec<Plane>, usize), JpegError> {
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

    // Resolved and checked once, before any MCU, because that is when libjpeg
    // does it. Deferring a table's validation until a code is read from it
    // would let a file with an impossible-but-unused table decode here and fail
    // there.
    let mut selected = Vec::with_capacity(components.len());
    for component in components {
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
        dc.validate(true)?;
        ac.validate(false)?;
        selected.push((quantizer, dc, ac));
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

            // libjpeg decides this once per MCU: having run out of scan data, it
            // stops decoding and leaves the rest of the image zeroed rather than
            // reading meaning into padding.
            let exhausted = reader.insufficient;

            for (index, component) in components.iter().enumerate() {
                let (quantizer, dc, ac) = selected[index];

                for block_y in 0..component.vertical {
                    for block_x in 0..component.horizontal {
                        let mut coefficients = [0i16; 64];

                        if exhausted {
                            // Left zeroed, which the transform renders as the
                            // flat mid-grey libjpeg produces here.
                            let mut block = [0u8; 64];
                            inverse_dct(&coefficients, quantizer, &mut block);
                            write_block(
                                &mut planes[index],
                                component,
                                mcu_x,
                                mcu_y,
                                block_x,
                                block_y,
                                &block,
                            );
                            continue;
                        }

                        let symbol = reader.decode(dc)?;
                        // The standard caps a DC magnitude category at 15; a
                        // corrupt Huffman table can hand back any byte.
                        if symbol > 15 {
                            return Err(JpegError::InvalidMagnitudeCategory(symbol));
                        }
                        let magnitude = u32::from(symbol);
                        let difference = extend(reader.receive(magnitude), magnitude);
                        predictors[index] = predictors[index].wrapping_add(difference);
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
                            // The magnitude bits are consumed whether or not the
                            // coefficient lands inside the block. A run can push
                            // the index past the end on corrupt input, and
                            // libjpeg still reads the bits and discards the
                            // value; stopping short instead leaves the bitstream
                            // one field out of step with libjpeg's for the rest
                            // of the scan.
                            let value = extend(reader.receive(size), size);
                            let target = NATURAL_ORDER.get(position).copied().unwrap_or(63);
                            coefficients[target] = value as i16;
                            position += 1;
                        }

                        let mut block = [0u8; 64];
                        inverse_dct(&coefficients, quantizer, &mut block);
                        write_block(
                            &mut planes[index],
                            component,
                            mcu_x,
                            mcu_y,
                            block_x,
                            block_y,
                            &block,
                        );
                    }
                }
            }
        }
    }

    Ok((planes, reader.scan_end()))
}

/// Places one transformed block into its component plane.
#[allow(clippy::too_many_arguments)]
fn write_block(
    plane: &mut Plane,
    component: &Component,
    mcu_x: usize,
    mcu_y: usize,
    block_x: usize,
    block_y: usize,
    block: &[u8; 64],
) {
    let origin_x = (mcu_x * component.horizontal + block_x) * 8;
    let origin_y = (mcu_y * component.vertical + block_y) * 8;
    for row in 0..8 {
        let target = (origin_y + row) * plane.stride + origin_x;
        plane.samples[target..target + 8].copy_from_slice(&block[row * 8..row * 8 + 8]);
    }
}

/// Upsamples every plane to full resolution and converts to output samples.
fn assemble(
    width: usize,
    height: usize,
    components: &[Component],
    planes: &[Plane],
    keep_planes: bool,
) -> Result<DecodedInner, JpegError> {
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

    Ok(DecodedInner {
        width,
        height,
        components: count,
        samples,
        planes: if keep_planes { upsampled } else { Vec::new() },
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
            let fancy = plane.width > FANCY_UPSAMPLE_MINIMUM_WIDTH;
            for row in 0..height {
                if fancy {
                    fancy_upsample_h2(row_of(row), &mut expanded, plane.width);
                } else {
                    box_upsample_h2(row_of(row), &mut expanded, plane.width);
                }
                let target = &mut output[row * width..row * width + width];
                let take = width.min(expanded.len());
                target[..take].copy_from_slice(&expanded[..take]);
            }
            Ok(output)
        }
        (2, 2) => {
            let mut output = vec![0u8; width * height];
            let mut expanded = vec![0u8; plane.width * 2];
            let fancy = plane.width > FANCY_UPSAMPLE_MINIMUM_WIDTH;
            for row in 0..height {
                if !fancy {
                    // Replication in both directions: no vertical mixing at all,
                    // so the row's own samples are simply doubled.
                    box_upsample_h2(row_of(row / 2), &mut expanded, plane.width);
                    let target = &mut output[row * width..row * width + width];
                    let take = width.min(expanded.len());
                    target[..take].copy_from_slice(&expanded[..take]);
                    continue;
                }
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

/// Decodes to upsampled component planes, before colour conversion.
///
/// Exists for parity diagnosis: when output samples disagree with libjpeg it is
/// far quicker to see whether the disagreement is already present in Y, Cb, or
/// Cr than to reason backwards through the colour transform.
///
/// # Errors
///
/// Propagates any [`JpegError`] from the scan.
pub fn decode_component_planes(bytes: &[u8]) -> Result<(usize, usize, Vec<Vec<u8>>), JpegError> {
    let decoded = decode_inner(bytes, true)?;
    Ok((decoded.width, decoded.height, decoded.planes))
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
