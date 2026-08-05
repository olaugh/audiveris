//! `CCITTFaxDecode`: Group 3 one- and two-dimensional, and Group 4.
//!
//! Ported from PDFBox's `CCITTFaxDecoderStream`, which is TwelveMonkeys'
//! decoder vendored in, not from T.4 and T.6 directly. The difference is not
//! cosmetic. Three behaviours below are this decoder's rather than the
//! standard's, and each one changes the output on real files:
//!
//! - **An unrecognised two-dimensional mode code restarts the mode read**
//!   instead of failing. `n.walk(bit)` returning null does `continue mode`,
//!   which throws away the bits consumed so far and begins a new code at the
//!   next bit. A decoder that errors here loses pages this one renders.
//! - **A run code that decodes to a negative value returns the full row
//!   width.** That is how an end-of-line met inside a run terminates the row,
//!   and it silently pads the row to `columns` rather than reporting anything.
//! - **A row that hits the end of the data is dropped whole.** The stream sets
//!   its decoded length before the row is complete, so a partial row is
//!   discarded and every row after it reads as zero -- which, after the
//!   inversion below, is a white page bottom rather than a torn one.
//!
//! And the two that come from `CCITTFaxFilter` rather than the stream:
//!
//! - **`/Rows` is not trusted.** When the image dictionary also carries a
//!   `/Height`, the height wins outright: PDFBOX-771 and PDFBOX-3727 are files
//!   whose `/Rows` is simply wrong.
//! - **The output is inverted unless `/BlackIs1`.** CCITT codes white as zero
//!   bits, `DeviceGray` reads one as white, so the default case flips every
//!   byte. Getting this backwards produces a photographic negative that still
//!   binarizes into something staff-line-shaped, which is a slow way to find
//!   out.
//!
//! With `/K` at zero the encoding is ambiguous and PDFBox *sniffs* it: it reads
//! the first twenty bytes looking for an end-of-line code, and treats the
//! stream as T.4 if it finds one and as modified Huffman if it does not.

use std::sync::OnceLock;

use crate::error::{Error, Result};

/// Which of the three encodings a stream carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// Modified Huffman: one-dimensional rows, no end-of-line codes.
    ModifiedHuffman,
    /// T.4, Group 3. One- or two-dimensional, every row preceded by an EOL.
    Group3,
    /// T.6, Group 4. Two-dimensional throughout, no EOLs.
    Group4,
}

/// The decode parameters, already resolved the way `CCITTFaxFilter` resolves
/// them.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// `/Columns`, defaulting to 1728.
    pub columns: usize,
    /// How many rows to produce, after `/Rows` and `/Height` are reconciled.
    pub rows: usize,
    /// Which encoding, after the `/K` zero case has been sniffed.
    pub encoding: Encoding,
    /// Whether rows are two-dimensional, which for Group 3 is `/K` above zero.
    pub two_dimensional: bool,
    /// `/EncodedByteAlign`: each row starts on a byte boundary.
    pub byte_aligned: bool,
    /// `/BlackIs1`: leave the bitmap as CCITT codes it rather than inverting.
    pub black_is_one: bool,
}

/// Decodes a CCITT stream into packed one-bit rows.
///
/// The result is always `(columns + 7) / 8 * rows` bytes: short input is padded
/// with zeroes before inversion, which is what PDFBox's fixed-size destination
/// buffer produces.
///
/// # Errors
///
/// [`Error::CorruptStream`] for a run code that is not in the alphabet, or a
/// row whose runs do not add up to `columns`. Both are errors in PDFBox too,
/// and both abort the whole image rather than the row.
pub fn decode(data: &[u8], options: &Options) -> Result<Vec<u8>> {
    let row_bytes = options.columns.div_ceil(8);
    let mut output = vec![0u8; row_bytes * options.rows];

    let mut decoder = Decoder {
        bits: Bits::new(data),
        columns: options.columns,
        options: *options,
        reference: vec![0i32; options.columns + 2],
        current: vec![0i32; options.columns + 2],
        reference_count: 0,
        current_count: 0,
        last_changing_element: 0,
    };

    for row in 0..options.rows {
        match decoder.row() {
            Ok(packed) => {
                output[row * row_bytes..(row + 1) * row_bytes].copy_from_slice(&packed);
            }
            // Running out of data ends the image; the rows already written
            // stand and the rest stay zero, exactly as the Java stream's
            // `decodedLength = -1` makes every later read return zero.
            Err(Error::Truncated) => break,
            Err(other) => return Err(other),
        }
    }

    if !options.black_is_one {
        for byte in &mut output {
            *byte = !*byte;
        }
    }
    Ok(output)
}

/// Chooses the encoding for a stream, as `CCITTFaxFilter.decode` does.
///
/// `/K` below zero is Group 4 and above zero is two-dimensional Group 3. At
/// zero the file has not said, and PDFBox looks: if `/EndOfLine` is present it
/// is believed, and otherwise the first twenty bytes are searched for an
/// end-of-line code, which is eleven zero bits and a one.
#[must_use]
pub fn choose(data: &[u8], k: i64, end_of_line: Option<bool>) -> (Encoding, bool) {
    if k < 0 {
        return (Encoding::Group4, false);
    }
    if k > 0 {
        return (Encoding::Group3, true);
    }
    if let Some(declared) = end_of_line {
        return (
            if declared {
                Encoding::Group3
            } else {
                Encoding::ModifiedHuffman
            },
            false,
        );
    }

    // PDFBox reads into a twenty-byte buffer that starts zeroed, so a shorter
    // stream is searched as if padded.
    let mut head = [0u8; 20];
    let read = data.len().min(20);
    head[..read].copy_from_slice(&data[..read]);
    if read == 0 {
        return (Encoding::Group3, false);
    }
    if head[0] == 0 && (i32::from(head[1] as i8) >> 4 == 1 || head[1] == 1) {
        return (Encoding::Group3, false);
    }
    // No leading end-of-line, so search the rest of the window for one before
    // settling on modified Huffman.
    let mut window = ((i32::from(head[0] as i8) << 8) + i32::from(head[1])) >> 4;
    for index in 12..read * 8 {
        let bit = (i32::from(head[index / 8] as i8) >> (7 - index % 8)) & 1;
        window = (window << 1) + bit;
        if window & 0xfff == 1 {
            return (Encoding::Group3, false);
        }
    }
    (Encoding::ModifiedHuffman, false)
}

/// The value a leaf carries when it is an end-of-line rather than a run.
const VALUE_EOL: i32 = -2000;
/// The value the fill node carries.
const VALUE_FILL: i32 = -1000;
/// The two-dimensional pass mode.
const VALUE_PASS: i32 = -3000;
/// The two-dimensional horizontal mode.
const VALUE_HORIZONTAL: i32 = -4000;

/// No child.
const NONE: usize = usize::MAX;

/// One node of a code tree.
#[derive(Debug, Clone, Copy)]
struct Node {
    left: usize,
    right: usize,
    value: i32,
    leaf: bool,
}

impl Node {
    const EMPTY: Self = Self {
        left: NONE,
        right: NONE,
        value: 0,
        leaf: false,
    };
}

/// A binary code tree in an arena, so the fill node can point at itself.
#[derive(Debug, Clone)]
struct Tree {
    nodes: Vec<Node>,
    /// The shared fill node, which loops on zero and yields end-of-line on one.
    fill: usize,
    /// The shared end-of-line leaf.
    eol: usize,
}

impl Tree {
    fn new() -> Self {
        let mut nodes = vec![Node::EMPTY];
        let eol = nodes.len();
        nodes.push(Node {
            value: VALUE_EOL,
            leaf: true,
            ..Node::EMPTY
        });
        let fill = nodes.len();
        nodes.push(Node {
            left: fill,
            right: eol,
            value: VALUE_FILL,
            leaf: false,
        });
        Self { nodes, fill, eol }
    }

    /// Adds a leaf for the `depth`-bit code `path`.
    fn insert(&mut self, depth: u32, path: u32, value: i32) {
        let leaf = self.nodes.len();
        self.nodes.push(Node {
            value,
            leaf: true,
            ..Node::EMPTY
        });
        self.link(depth, path, leaf);
    }

    /// Links an existing node in as the `depth`-bit code `path`.
    ///
    /// An intermediate node that already exists is reused rather than replaced,
    /// which is why the fill and end-of-line nodes have to be added *after* the
    /// run codes: added first they would own prefixes the run codes need.
    fn link(&mut self, depth: u32, path: u32, node: usize) {
        let mut current = 0usize;
        for step in 0..depth {
            let set = (path >> (depth - 1 - step)) & 1 == 1;
            let existing = if set {
                self.nodes[current].right
            } else {
                self.nodes[current].left
            };
            let next = if existing == NONE {
                let fresh = if step == depth - 1 {
                    node
                } else {
                    self.nodes.push(Node::EMPTY);
                    self.nodes.len() - 1
                };
                if set {
                    self.nodes[current].right = fresh;
                } else {
                    self.nodes[current].left = fresh;
                }
                fresh
            } else {
                existing
            };
            current = next;
        }
    }

    /// Follows one bit. `None` means the code is not in the tree.
    fn walk(&self, at: usize, bit: bool) -> Option<usize> {
        let next = if bit {
            self.nodes[at].right
        } else {
            self.nodes[at].left
        };
        (next != NONE).then_some(next)
    }
}

/// The white and black run-length alphabets, and the two-dimensional modes.
struct Alphabets {
    white: Tree,
    black: Tree,
    modes: Tree,
    end_of_line: Tree,
}

/// Black run codes by length, starting at two bits.
const BLACK_CODES: [&[u16]; 12] = [
    &[0x2, 0x3],
    &[0x2, 0x3],
    &[0x2, 0x3],
    &[0x3],
    &[0x4, 0x5],
    &[0x4, 0x5, 0x7],
    &[0x4, 0x7],
    &[0x18],
    &[0x17, 0x18, 0x37, 0x8, 0xf],
    &[0x17, 0x18, 0x28, 0x37, 0x67, 0x68, 0x6c, 0x8, 0xc, 0xd],
    &[
        0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x1c, 0x1d, 0x1e, 0x1f, 0x24, 0x27, 0x28, 0x2b, 0x2c,
        0x33, 0x34, 0x35, 0x37, 0x38, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x5b,
        0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x6b, 0x6c, 0x6d, 0xc8, 0xc9, 0xca, 0xcb, 0xcc,
        0xcd, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xda, 0xdb,
    ],
    &[
        0x4a, 0x4b, 0x4c, 0x4d, 0x52, 0x53, 0x54, 0x55, 0x5a, 0x5b, 0x64, 0x65, 0x6c, 0x6d, 0x72,
        0x73, 0x74, 0x75, 0x76, 0x77,
    ],
];

/// The run lengths those codes stand for.
const BLACK_RUNS: [&[u16]; 12] = [
    &[3, 2],
    &[1, 4],
    &[6, 5],
    &[7],
    &[9, 8],
    &[10, 11, 12],
    &[13, 14],
    &[15],
    &[16, 17, 0, 18, 64],
    &[24, 25, 23, 22, 19, 20, 21, 1792, 1856, 1920],
    &[
        1984, 2048, 2112, 2176, 2240, 2304, 2368, 2432, 2496, 2560, 52, 55, 56, 59, 60, 320, 384,
        448, 53, 54, 50, 51, 44, 45, 46, 47, 57, 58, 61, 256, 48, 49, 62, 63, 30, 31, 32, 33, 40,
        41, 128, 192, 26, 27, 28, 29, 34, 35, 36, 37, 38, 39, 42, 43,
    ],
    &[
        640, 704, 768, 832, 1280, 1344, 1408, 1472, 1536, 1600, 1664, 1728, 512, 576, 896, 960,
        1024, 1088, 1152, 1216,
    ],
];

/// White run codes by length, starting at four bits.
const WHITE_CODES: [&[u16]; 9] = [
    &[0x7, 0x8, 0xb, 0xc, 0xe, 0xf],
    &[0x12, 0x13, 0x14, 0x1b, 0x7, 0x8],
    &[0x17, 0x18, 0x2a, 0x2b, 0x3, 0x34, 0x35, 0x7, 0x8],
    &[
        0x13, 0x17, 0x18, 0x24, 0x27, 0x28, 0x2b, 0x3, 0x37, 0x4, 0x8, 0xc,
    ],
    &[
        0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x1a, 0x1b, 0x2, 0x24, 0x25, 0x28, 0x29, 0x2a, 0x2b,
        0x2c, 0x2d, 0x3, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x4, 0x4a, 0x4b, 0x5, 0x52, 0x53,
        0x54, 0x55, 0x58, 0x59, 0x5a, 0x5b, 0x64, 0x65, 0x67, 0x68, 0xa, 0xb,
    ],
    &[
        0x98, 0x99, 0x9a, 0x9b, 0xcc, 0xcd, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda,
        0xdb,
    ],
    &[],
    &[0x8, 0xc, 0xd],
    &[0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x1c, 0x1d, 0x1e, 0x1f],
];

/// The run lengths those codes stand for.
const WHITE_RUNS: [&[u16]; 9] = [
    &[2, 3, 4, 5, 6, 7],
    &[128, 8, 9, 64, 10, 11],
    &[192, 1664, 16, 17, 13, 14, 15, 1, 12],
    &[26, 21, 28, 27, 18, 24, 25, 22, 256, 23, 20, 19],
    &[
        33, 34, 35, 36, 37, 38, 31, 32, 29, 53, 54, 39, 40, 41, 42, 43, 44, 30, 61, 62, 63, 0, 320,
        384, 45, 59, 60, 46, 49, 50, 51, 52, 55, 56, 57, 58, 448, 512, 640, 576, 47, 48,
    ],
    &[
        1472, 1536, 1600, 1728, 704, 768, 832, 896, 960, 1024, 1088, 1152, 1216, 1280, 1344, 1408,
    ],
    &[],
    &[1792, 1856, 1920],
    &[1984, 2048, 2112, 2176, 2240, 2304, 2368, 2432, 2496, 2560],
];

fn alphabets() -> &'static Alphabets {
    static ALPHABETS: OnceLock<Alphabets> = OnceLock::new();
    ALPHABETS.get_or_init(|| {
        let mut white = Tree::new();
        for (index, (codes, runs)) in WHITE_CODES.iter().zip(WHITE_RUNS).enumerate() {
            for (code, run) in codes.iter().zip(runs) {
                white.insert(index as u32 + 4, u32::from(*code), i32::from(*run));
            }
        }
        white.link(12, 0, white.fill);
        white.link(12, 1, white.eol);

        let mut black = Tree::new();
        for (index, (codes, runs)) in BLACK_CODES.iter().zip(BLACK_RUNS).enumerate() {
            for (code, run) in codes.iter().zip(runs) {
                black.insert(index as u32 + 2, u32::from(*code), i32::from(*run));
            }
        }
        black.link(12, 0, black.fill);
        black.link(12, 1, black.eol);

        let mut end_of_line = Tree::new();
        end_of_line.link(12, 0, end_of_line.fill);
        end_of_line.link(12, 1, end_of_line.eol);

        let mut modes = Tree::new();
        modes.insert(4, 1, VALUE_PASS);
        modes.insert(3, 1, VALUE_HORIZONTAL);
        modes.insert(1, 1, 0);
        modes.insert(3, 3, 1);
        modes.insert(6, 3, 2);
        modes.insert(7, 3, 3);
        modes.insert(3, 2, -1);
        modes.insert(6, 2, -2);
        modes.insert(7, 2, -3);

        Alphabets {
            white,
            black,
            modes,
            end_of_line,
        }
    })
}

/// A bit reader that can be re-aligned to a byte boundary.
struct Bits<'a> {
    data: &'a [u8],
    position: usize,
    buffer: u32,
    held: i32,
}

impl<'a> Bits<'a> {
    const fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            position: 0,
            buffer: 0,
            held: -1,
        }
    }

    /// Reads one bit, most significant first.
    fn bit(&mut self) -> Result<bool> {
        if !(0..=7).contains(&self.held) {
            let Some(&byte) = self.data.get(self.position) else {
                return Err(Error::Truncated);
            };
            self.position += 1;
            self.buffer = u32::from(byte);
            self.held = 0;
        }
        let set = self.buffer & 0x80 != 0;
        self.buffer <<= 1;
        self.held += 1;
        Ok(set)
    }

    /// Discards the rest of the current byte.
    const fn align(&mut self) {
        self.held = -1;
    }
}

struct Decoder<'a> {
    bits: Bits<'a>,
    columns: usize,
    options: Options,
    /// The changing elements of the row above, for two-dimensional coding.
    reference: Vec<i32>,
    /// The changing elements of the row being decoded.
    current: Vec<i32>,
    reference_count: usize,
    current_count: usize,
    /// Where the last two-dimensional lookup left off, carried between calls.
    last_changing_element: usize,
}

impl Decoder<'_> {
    /// Decodes one row into packed bits.
    fn row(&mut self) -> Result<Vec<u8>> {
        if self.options.byte_aligned {
            self.bits.align();
        }
        match self.options.encoding {
            Encoding::ModifiedHuffman => self.one_dimensional()?,
            Encoding::Group3 => {
                self.skip_to_end_of_line()?;
                // The bit after an EOL says which coding the row uses, but only
                // when the stream declared itself two-dimensional.
                if !self.options.two_dimensional || self.bits.bit()? {
                    self.one_dimensional()?;
                } else {
                    self.two_dimensional()?;
                }
            }
            Encoding::Group4 => self.two_dimensional()?,
        }
        self.pack()
    }

    /// Consumes bits until an end-of-line code has been read.
    fn skip_to_end_of_line(&mut self) -> Result<()> {
        let tree = &alphabets().end_of_line;
        loop {
            let mut at = 0usize;
            // A walk that runs out means it was not an end-of-line after all:
            // throw away what was read and start again at the next bit.
            while let Some(next) = tree.walk(at, self.bits.bit()?) {
                if tree.nodes[next].leaf {
                    return Ok(());
                }
                at = next;
            }
        }
    }

    /// A row of alternating white and black runs.
    fn one_dimensional(&mut self) -> Result<()> {
        let mut index = 0i32;
        let mut white = true;
        self.current_count = 0;
        loop {
            let run = self.run(white)?;
            index += run;
            self.current[self.current_count] = index;
            self.current_count += 1;
            white = !white;
            if index >= self.columns as i32 {
                return Ok(());
            }
        }
    }

    /// A row coded against the one above it.
    fn two_dimensional(&mut self) -> Result<()> {
        std::mem::swap(&mut self.current, &mut self.reference);
        self.reference_count = self.current_count;
        self.current_count = 0;

        let modes = &alphabets().modes;
        let mut white = true;
        let mut index = 0i32;
        while index < self.columns as i32 {
            let mut at = 0usize;
            // A walk that runs out is an unrecognised mode code, and that
            // restarts the mode read rather than ending the row. Reproducing
            // this is not optional: it is what keeps damaged rows decoding.
            while let Some(next) = modes.walk(at, self.bits.bit()?) {
                at = next;
                if !modes.nodes[at].leaf {
                    continue;
                }
                match modes.nodes[at].value {
                    VALUE_HORIZONTAL => {
                        index += self.run(white)?;
                        self.current[self.current_count] = index;
                        self.current_count += 1;
                        index += self.run(!white)?;
                        self.current[self.current_count] = index;
                        self.current_count += 1;
                    }
                    VALUE_PASS => {
                        let element = self.next_changing_element(index, white) + 1;
                        index = if element as usize >= self.reference_count || element < 0 {
                            self.columns as i32
                        } else {
                            self.reference[element as usize]
                        };
                    }
                    vertical => {
                        let element = self.next_changing_element(index, white);
                        index = if element < 0 || element as usize >= self.reference_count {
                            self.columns as i32 + vertical
                        } else {
                            self.reference[element as usize] + vertical
                        };
                        self.current[self.current_count] = index;
                        self.current_count += 1;
                        white = !white;
                    }
                }
                break;
            }
        }
        Ok(())
    }

    /// The reference row's next changing element at or after `a0`.
    ///
    /// The search starts from where the previous one stopped, rounded down to
    /// the right colour's parity. That carried state is why the two-dimensional
    /// decoder cannot be made stateless.
    fn next_changing_element(&mut self, a0: i32, white: bool) -> i32 {
        let mut start = (self.last_changing_element & !1) + usize::from(!white);
        if start > 2 {
            start -= 2;
        }
        if a0 == 0 {
            return start as i32;
        }
        let mut index = start;
        while index < self.reference_count {
            if a0 < self.reference[index] {
                self.last_changing_element = index;
                return index as i32;
            }
            index += 2;
        }
        -1
    }

    /// One run length, following makeup codes to their terminating code.
    fn run(&mut self, white: bool) -> Result<i32> {
        let alphabets = alphabets();
        let tree = if white {
            &alphabets.white
        } else {
            &alphabets.black
        };
        let mut total = 0i32;
        let mut at = 0usize;
        loop {
            let Some(next) = tree.walk(at, self.bits.bit()?) else {
                return Err(Error::CorruptStream { filter: "CCITTFax" });
            };
            at = next;
            if !tree.nodes[at].leaf {
                continue;
            }
            let value = tree.nodes[at].value;
            total += value;
            if value >= 64 {
                // A makeup code; the run continues with a terminating code.
                at = 0;
            } else if value >= 0 {
                return Ok(total);
            } else {
                // An end-of-line met inside a run ends the row by filling it.
                return Ok(self.columns as i32);
            }
        }
    }

    /// Turns the row's changing elements into packed bits.
    fn pack(&mut self) -> Result<Vec<u8>> {
        let mut row = vec![0u8; self.columns.div_ceil(8)];
        let mut index = 0usize;
        let mut white = true;
        self.last_changing_element = 0;

        for element in 0..=self.current_count {
            let next = if element == self.current_count {
                self.columns
            } else {
                (self.current[element].max(0) as usize).min(self.columns)
            };
            let mut byte = index / 8;
            while index % 8 != 0 && next > index {
                row[byte] |= if white { 0 } else { 1 << (7 - index % 8) };
                index += 1;
            }
            if index % 8 == 0 {
                byte = index / 8;
                let value = if white { 0x00 } else { 0xff };
                while next.saturating_sub(index) > 7 {
                    row[byte] = value;
                    index += 8;
                    byte += 1;
                }
            }
            while next > index {
                if index % 8 == 0 {
                    row[byte] = 0;
                }
                row[byte] |= if white { 0 } else { 1 << (7 - index % 8) };
                index += 1;
            }
            white = !white;
        }

        if index != self.columns {
            return Err(Error::CorruptStream { filter: "CCITTFax" });
        }
        Ok(row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Group 4 encoding of an 8x2 image, all white.
    ///
    /// Two rows of V(0) against an imaginary all-white reference: the first
    /// changing element of a blank reference row is the row width, so a single
    /// `1` bit codes each row.
    #[test]
    fn decodes_blank_group_four_rows() {
        let options = Options {
            columns: 8,
            rows: 2,
            encoding: Encoding::Group4,
            two_dimensional: false,
            byte_aligned: false,
            black_is_one: true,
        };
        // V(0) is a single set bit; two of them, then padding.
        let decoded = decode(&[0b1100_0000], &options).expect("decodes");
        assert_eq!(decoded, vec![0x00, 0x00]);
    }

    #[test]
    fn inverts_unless_black_is_one() {
        let mut options = Options {
            columns: 8,
            rows: 1,
            encoding: Encoding::Group4,
            two_dimensional: false,
            byte_aligned: false,
            black_is_one: false,
        };
        assert_eq!(
            decode(&[0b1000_0000], &options).expect("decodes"),
            vec![0xff]
        );
        options.black_is_one = true;
        assert_eq!(
            decode(&[0b1000_0000], &options).expect("decodes"),
            vec![0x00]
        );
    }

    #[test]
    fn pads_the_rows_it_never_reached() {
        let options = Options {
            columns: 8,
            rows: 4,
            encoding: Encoding::Group4,
            two_dimensional: false,
            byte_aligned: false,
            black_is_one: true,
        };
        // One row's worth of bits for a four-row image: the rest stay zero
        // rather than the decode failing.
        assert_eq!(decode(&[0b1000_0000], &options).expect("decodes").len(), 4);
    }

    #[test]
    fn sniffs_the_encoding_when_k_is_zero() {
        // A leading end-of-line: eleven zeroes and a one.
        assert_eq!(choose(&[0x00, 0x10, 0x00], 0, None).0, Encoding::Group3);
        // White run codes with no end-of-line anywhere in the window.
        assert_eq!(choose(&[0xff; 20], 0, None).0, Encoding::ModifiedHuffman);
        // An explicit /EndOfLine is believed without looking at the data.
        assert_eq!(choose(&[0xff; 20], 0, Some(true)).0, Encoding::Group3);
        assert_eq!(choose(&[], -1, None), (Encoding::Group4, false));
        assert_eq!(choose(&[], 4, None), (Encoding::Group3, true));
    }
}
