//! The MQ arithmetic decoder and the integer decoders built on it.
//!
//! Ported from jbig2-imageio's `ArithmeticDecoder` rather than from T.88, and
//! the difference shows in one place: at the end of the data. `ImageInputStream
//! .read()` returns -1 past the end, and jbig2-imageio does not check, so the
//! byte it folds into the code register is -1 rather than the 0xFF the standard
//! specifies. `C` is a `long` there, so that subtracts 256 instead of adding
//! 0xFF00. Anything that decodes past its data -- which a damaged file does --
//! diverges immediately from a standards-following decoder, so the sign is
//! reproduced rather than corrected.
//!
//! `byteIn` is also written around a stream cursor that seeks backwards, and
//! after a 0xFF marker it leaves the cursor one byte earlier than the marker
//! rather than at it. That turns out not to matter: the two states it then
//! alternates between both add 0xFF00 with a count of 8, which is what the
//! standard does anyway. Reproducing the cursor arithmetic literally is still
//! cheaper than proving that every time.

/// The probability estimation table of T.88, with its next-state and switch
/// columns: `(Qe, NMPS, NLPS, SWITCH)`.
const QE: [(u32, u8, u8, u8); 47] = [
    (0x5601, 1, 1, 1),
    (0x3401, 2, 6, 0),
    (0x1801, 3, 9, 0),
    (0x0ac1, 4, 12, 0),
    (0x0521, 5, 29, 0),
    (0x0221, 38, 33, 0),
    (0x5601, 7, 6, 1),
    (0x5401, 8, 14, 0),
    (0x4801, 9, 14, 0),
    (0x3801, 10, 14, 0),
    (0x3001, 11, 17, 0),
    (0x2401, 12, 18, 0),
    (0x1c01, 13, 20, 0),
    (0x1601, 29, 21, 0),
    (0x5601, 15, 14, 1),
    (0x5401, 16, 14, 0),
    (0x5101, 17, 15, 0),
    (0x4801, 18, 16, 0),
    (0x3801, 19, 17, 0),
    (0x3401, 20, 18, 0),
    (0x3001, 21, 19, 0),
    (0x2801, 22, 19, 0),
    (0x2401, 23, 20, 0),
    (0x2201, 24, 21, 0),
    (0x1c01, 25, 22, 0),
    (0x1801, 26, 23, 0),
    (0x1601, 27, 24, 0),
    (0x1401, 28, 25, 0),
    (0x1201, 29, 26, 0),
    (0x1101, 30, 27, 0),
    (0x0ac1, 31, 28, 0),
    (0x09c1, 32, 29, 0),
    (0x08a1, 33, 30, 0),
    (0x0521, 34, 31, 0),
    (0x0441, 35, 32, 0),
    (0x02a1, 36, 33, 0),
    (0x0221, 37, 34, 0),
    (0x0141, 38, 35, 0),
    (0x0111, 39, 36, 0),
    (0x0085, 40, 37, 0),
    (0x0049, 41, 38, 0),
    (0x0025, 42, 39, 0),
    (0x0015, 43, 40, 0),
    (0x0009, 44, 41, 0),
    (0x0005, 45, 42, 0),
    (0x0001, 45, 43, 0),
    (0x5601, 46, 46, 0),
];

/// A bank of adaptive contexts: a state index and a most-probable symbol each.
#[derive(Debug, Clone)]
pub struct Contexts {
    state: Vec<u8>,
    mps: Vec<u8>,
}

impl Contexts {
    /// A bank of `size` contexts, all in state zero with an MPS of zero.
    #[must_use]
    pub fn new(size: usize) -> Self {
        Self {
            state: vec![0; size],
            mps: vec![0; size],
        }
    }
}

/// The MQ decoder.
#[derive(Debug)]
pub struct Arithmetic<'a> {
    data: &'a [u8],
    /// The stream cursor, in the same units as jbig2-imageio's.
    position: usize,
    a: u32,
    /// The code register. A signed 64-bit value, because Java's is a `long`
    /// and the end-of-data path can make it negative.
    c: i64,
    ct: i32,
}

impl<'a> Arithmetic<'a> {
    /// Starts decoding at the beginning of `data`.
    #[must_use]
    pub fn new(data: &'a [u8]) -> Self {
        let mut decoder = Self {
            data,
            position: 0,
            a: 0,
            c: 0,
            ct: 0,
        };
        let byte = decoder.read();
        decoder.c = i64::from(byte << 16);
        decoder.byte_in();
        decoder.c <<= 7;
        decoder.ct -= 7;
        decoder.a = 0x8000;
        decoder
    }

    /// One byte, or -1 past the end, leaving the cursor where Java leaves it.
    fn read(&mut self) -> i32 {
        match self.data.get(self.position) {
            Some(&byte) => {
                self.position += 1;
                i32::from(byte)
            }
            None => -1,
        }
    }

    fn byte_in(&mut self) {
        if self.position > 0 {
            self.position -= 1;
        }
        let byte = self.read();
        if byte == 0xff {
            let next = self.read();
            if next > 0x8f {
                self.c += 0xff00;
                self.ct = 8;
                self.position = self.position.saturating_sub(2);
            } else {
                self.c += i64::from(next << 9);
                self.ct = 7;
            }
        } else {
            let byte = self.read();
            self.c += i64::from(byte << 8);
            self.ct = 8;
        }
        self.c &= 0xffff_ffff;
    }

    /// Decodes one bit against context `index` of `contexts`.
    pub fn decode(&mut self, contexts: &mut Contexts, index: usize) -> u8 {
        let state = usize::from(contexts.state[index] & 0x7f);
        let (qe, nmps, nlps, switch) = QE[state];
        self.a -= qe;
        if (self.c >> 16) < i64::from(qe) {
            let bit = self.lps_exchange(contexts, index, qe, nmps, nlps, switch);
            self.renormalize();
            return bit;
        }
        self.c -= i64::from(qe) << 16;
        if self.a & 0x8000 == 0 {
            let bit = self.mps_exchange(contexts, index, qe, nmps, nlps, switch);
            self.renormalize();
            return bit;
        }
        contexts.mps[index]
    }

    fn renormalize(&mut self) {
        loop {
            if self.ct == 0 {
                self.byte_in();
            }
            self.a <<= 1;
            self.c <<= 1;
            self.ct -= 1;
            if self.a & 0x8000 != 0 {
                break;
            }
        }
        self.c &= 0xffff_ffff;
    }

    fn mps_exchange(
        &mut self,
        contexts: &mut Contexts,
        index: usize,
        qe: u32,
        nmps: u8,
        nlps: u8,
        switch: u8,
    ) -> u8 {
        let mps = contexts.mps[index];
        if self.a < qe {
            if switch == 1 {
                contexts.mps[index] ^= 1;
            }
            contexts.state[index] = nlps & 0x7f;
            1 - mps
        } else {
            contexts.state[index] = nmps & 0x7f;
            mps
        }
    }

    fn lps_exchange(
        &mut self,
        contexts: &mut Contexts,
        index: usize,
        qe: u32,
        nmps: u8,
        nlps: u8,
        switch: u8,
    ) -> u8 {
        let mps = contexts.mps[index];
        if self.a < qe {
            contexts.state[index] = nmps & 0x7f;
            self.a = qe;
            mps
        } else {
            if switch == 1 {
                contexts.mps[index] ^= 1;
            }
            contexts.state[index] = nlps & 0x7f;
            self.a = qe;
            1 - mps
        }
    }
}

/// Decodes an integer with the IAx procedure of annex A.
///
/// `None` is the out-of-band value, which is how a symbol dictionary's width
/// loop and a text region's strip loop learn they are finished. Note that the
/// sign bit set with a magnitude of zero is *also* out of band, not negative
/// zero.
pub fn integer(decoder: &mut Arithmetic<'_>, contexts: &mut Contexts) -> Option<i64> {
    let mut previous = 1usize;
    let advance = |decoder: &mut Arithmetic<'_>, contexts: &mut Contexts, previous: &mut usize| {
        let bit = decoder.decode(contexts, *previous);
        // A.2: the context index tracks the bits decoded so far, saturating
        // into the upper half of the bank once nine bits have gone by.
        *previous = if *previous < 256 {
            (*previous << 1 | usize::from(bit)) & 0x1ff
        } else {
            (((*previous << 1 | usize::from(bit)) & 511) | 256) & 0x1ff
        };
        bit
    };

    let sign = advance(decoder, contexts, &mut previous);
    let (count, offset) = if advance(decoder, contexts, &mut previous) == 0 {
        (2, 0)
    } else if advance(decoder, contexts, &mut previous) == 0 {
        (4, 4)
    } else if advance(decoder, contexts, &mut previous) == 0 {
        (6, 20)
    } else if advance(decoder, contexts, &mut previous) == 0 {
        (8, 84)
    } else if advance(decoder, contexts, &mut previous) == 0 {
        (12, 340)
    } else {
        (32, 4436)
    };

    let mut value = 0i64;
    for _ in 0..count {
        let bit = advance(decoder, contexts, &mut previous);
        // Java accumulates into an `int`, so a 32-bit magnitude wraps rather
        // than widening. Nothing legitimate reaches that, but a damaged file
        // can, and it must wrap the same way.
        value = i64::from((value as i32) << 1 | i32::from(bit));
    }
    value = i64::from((value as i32).wrapping_add(offset));

    if sign == 0 {
        Some(value)
    } else if value > 0 {
        Some(-value)
    } else {
        None
    }
}

/// Decodes a symbol identifier with the IAID procedure of A.3.
pub fn identifier(
    decoder: &mut Arithmetic<'_>,
    contexts: &mut Contexts,
    code_length: u32,
) -> usize {
    let mut previous = 1usize;
    for _ in 0..code_length {
        previous = previous << 1 | usize::from(decoder.decode(contexts, previous));
    }
    previous - (1 << code_length)
}
