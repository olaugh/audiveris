//! A one-bit-per-pixel bitmap, laid out as jbig2-imageio lays one out.
//!
//! Rows are padded to whole bytes and a pixel outside the bitmap reads as
//! zero. That last part is not incidental: the generic region's context is
//! gathered from a neighbourhood that hangs off the top and left edges of the
//! first rows, and every one of those reads has to be zero for the arithmetic
//! decoder to stay in step.

/// A packed bilevel image. One means black, as JBIG2 codes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bitmap {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// Bytes per row, `width` rounded up to a byte.
    pub stride: usize,
    /// The packed rows, most significant bit leftmost.
    pub bytes: Vec<u8>,
}

impl Bitmap {
    /// An all-white bitmap.
    #[must_use]
    pub fn new(width: usize, height: usize) -> Self {
        let stride = width.div_ceil(8);
        Self {
            width,
            height,
            stride,
            bytes: vec![0; stride * height],
        }
    }

    /// Sets every pixel, including the padding bits beyond `width`.
    ///
    /// The padding matters: a page whose default pixel is one has its whole
    /// buffer filled, padding and all, and the conversion to a raster masks
    /// those bits off again at the very end rather than here.
    pub fn fill(&mut self, value: u8) {
        self.bytes.fill(value);
    }

    /// The pixel at `(x, y)`, or zero outside the bitmap.
    #[must_use]
    pub fn pixel(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return 0;
        }
        let (x, y) = (x as usize, y as usize);
        self.bytes[y * self.stride + (x >> 3)] >> (7 - (x & 7)) & 1
    }

    /// Sets the pixel at `(x, y)`, ignoring anything outside the bitmap.
    pub fn set(&mut self, x: usize, y: usize, value: u8) {
        if x >= self.width || y >= self.height {
            return;
        }
        let index = y * self.stride + (x >> 3);
        let shift = 7 - (x & 7);
        self.bytes[index] = self.bytes[index] & !(1 << shift) | (value & 1) << shift;
    }
}

/// How a region's pixels combine with what is already on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combine {
    /// Bitwise or, the only operator the corpus uses.
    Or,
    /// Bitwise and.
    And,
    /// Bitwise exclusive or.
    Xor,
    /// The complement of exclusive or.
    Xnor,
    /// Overwrite.
    Replace,
}

impl Combine {
    /// The operator a three-bit code stands for.
    #[must_use]
    pub const fn from_code(code: u8) -> Self {
        match code {
            1 => Self::And,
            2 => Self::Xor,
            3 => Self::Xnor,
            4 => Self::Replace,
            _ => Self::Or,
        }
    }

    const fn apply(self, source: u8, destination: u8) -> u8 {
        match self {
            Self::Or => source | destination,
            Self::And => source & destination,
            Self::Xor => source ^ destination,
            Self::Xnor => !(source ^ destination) & 1,
            Self::Replace => source,
        }
    }
}

/// Draws `source` onto `destination` with its top-left corner at `(x, y)`.
///
/// Anything that would land outside `destination` is dropped. jbig2-imageio
/// does this a byte at a time with a shift, which is faster and, for a source
/// that fits, identical.
pub fn blit(source: &Bitmap, destination: &mut Bitmap, x: i32, y: i32, operator: Combine) {
    for row in 0..source.height {
        let target_y = y + row as i32;
        if target_y < 0 || target_y as usize >= destination.height {
            continue;
        }
        for column in 0..source.width {
            let target_x = x + column as i32;
            if target_x < 0 || target_x as usize >= destination.width {
                continue;
            }
            let value = operator.apply(
                source.pixel(column as i32, row as i32),
                destination.pixel(target_x, target_y),
            );
            destination.set(target_x as usize, target_y as usize, value);
        }
    }
}

/// Packs a bitmap the way `Bitmaps.buildRaster` does at scale one.
///
/// Two things happen here and both are load-bearing. The bytes are
/// **inverted**, because the raster's colour model has index zero as black
/// while JBIG2 codes black as one; and the bits past `width` in each row's
/// final byte are **cleared**, so a bitmap that was filled with ones does not
/// leave stray set bits in its padding. The result is what `JBIG2Filter` writes
/// out, which is what the oracle hashes.
#[must_use]
pub fn to_raster(bitmap: &Bitmap) -> Vec<u8> {
    let whole = bitmap.width / 8;
    // The mask keeping only the bits that are inside the row.
    let mask = ((!0xffi32 >> (bitmap.width & 7)) & 0xff) as u8;
    let mut raster = vec![0u8; bitmap.height * bitmap.stride];
    let mut index = 0usize;
    for _ in 0..bitmap.height {
        for _ in 0..whole {
            raster[index] = !bitmap.bytes[index];
            index += 1;
        }
        if mask != 0 {
            raster[index] = !bitmap.bytes[index] & mask;
            index += 1;
        }
    }
    raster
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clears_the_padding_bits_and_inverts() {
        // Five pixels wide: three padding bits in the only byte of each row.
        let mut bitmap = Bitmap::new(5, 1);
        bitmap.fill(0xff);
        // Every bit is set, so inverting gives zero and the mask changes
        // nothing; the point is the reverse case below.
        assert_eq!(to_raster(&bitmap), vec![0x00]);

        let bitmap = Bitmap::new(5, 1);
        // All white inverts to all black, but only within the row.
        assert_eq!(to_raster(&bitmap), vec![0xf8]);
    }

    #[test]
    fn reads_zero_outside_its_bounds() {
        let mut bitmap = Bitmap::new(3, 3);
        bitmap.set(0, 0, 1);
        assert_eq!(bitmap.pixel(0, 0), 1);
        assert_eq!(bitmap.pixel(-1, 0), 0);
        assert_eq!(bitmap.pixel(0, -1), 0);
        assert_eq!(bitmap.pixel(3, 0), 0);
        assert_eq!(bitmap.pixel(0, 3), 0);
    }
}
