//! A most-significant-bit-first reader for JBIG2's headers.
//!
//! Segment and region headers are bit-packed, and the arithmetic decoder picks
//! up at the next whole byte after them, so the reader has to report where it
//! stopped as well as what it read.

/// A cursor over bits, then over the bytes that follow them.
pub struct Reader<'a> {
    data: &'a [u8],
    /// The next bit to read, counted from the start of `data`.
    position: usize,
}

impl<'a> Reader<'a> {
    /// A reader at the start of `data`.
    #[must_use]
    pub const fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    /// Reads one bit, or zero past the end.
    pub fn bit(&mut self) -> u32 {
        let byte = self.data.get(self.position / 8).copied().unwrap_or(0);
        let bit = u32::from(byte >> (7 - self.position % 8) & 1);
        self.position += 1;
        bit
    }

    /// Reads `count` bits, most significant first.
    pub fn bits(&mut self, count: u32) -> u64 {
        let mut value = 0u64;
        for _ in 0..count {
            value = value << 1 | u64::from(self.bit());
        }
        value
    }

    /// Everything from the next whole byte onwards.
    #[must_use]
    pub fn rest(&self) -> &'a [u8] {
        let at = self.position.div_ceil(8).min(self.data.len());
        &self.data[at..]
    }
}
