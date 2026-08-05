// SPDX-License-Identifier: AGPL-3.0-or-later

//! Decoded stream bytes to image samples, as PDFBox's `SampledImageReader` does.
//!
//! This is the rung between [`crate::filter`], which turns the bytes in the file
//! into the bytes the filter chain produces, and composing the page. It answers
//! a different question from either: given those bytes, what does a *sample*
//! hold -- after bit unpacking, `/Decode`, and the colour space.
//!
//! Splitting it out is what makes the layer above debuggable. `oracle/
//! pdf-pages.txt` records four depths per page, and the third is exactly this:
//! `PDImage.getImage()`'s raster, hashed band-interleaved and row-major. So
//! sample conversion is graded on its own, and when a composed page later comes
//! out wrong, the half it is wrong in is already known.
//!
//! # What PDFBox actually does
//!
//! `PDImageXObject.getImage()` reaches `SampledImageReader.getRGBImage`, which
//! branches once, on `bitsPerComponent == 1 && colorKey == null &&
//! numComponents == 1`:
//!
//! - `from1Bit` unpacks bits straight to bytes. For `DeviceGray` it builds a
//!   `TYPE_BYTE_GRAY` image and returns it -- **one band, not three** -- with a
//!   set bit becoming 255 and a clear bit left at the raster's initial 0. That
//!   single-band result is not an optimisation detail to be normalised away: it
//!   is what the page render later draws, and `ImageType.GRAY` is a gray
//!   destination.
//! - `fromAny` reads `bitsPerComponent` at a time through an MSB-first bit
//!   reader, applies `/Decode` by interpolation, and hands the result to the
//!   colour space's `toRGBImage`.
//!
//! # Scope, measured rather than assumed
//!
//! Across all 189 corpus images there are exactly four shapes: 177 are 1-bit
//! `DeviceGray` with no `/Decode`, 11 are 1-bit `DeviceGray` with `/Decode
//! [1 0]`, and one is 4-bit `Indexed` over `DeviceRGB`. None is an
//! `/ImageMask`, and none carries a colour-key `/Mask`. Everything else is
//! refused by name through [`Error::UnsupportedImage`], on the same reasoning
//! the JBIG2 segment types were: the probe is twenty lines, so measure before
//! extending this.

use crate::document::Document;
use crate::error::{Error, Result};
use crate::object::{Dictionary, Name, Object, Stream};

/// An image decoded to samples: PDFBox's `BufferedImage.getRaster()`.
///
/// Samples are band-interleaved and row-major, which is the order
/// `Raster.getPixels` walks and therefore the order the oracle hashes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raster {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// Samples per pixel: 1 for the gray path, 3 once a colour space has run.
    pub bands: usize,
    /// `width * height * bands` samples.
    pub samples: Vec<u8>,
}

/// The colour spaces the corpus contains.
///
/// `Indexed` carries its palette already reduced to RGB, because that is the
/// form `PDIndexed` caches after `initRgbColorTable` and the form the lookup
/// uses.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ColorSpace {
    DeviceGray,
    /// The RGB lookup table, and PDFBox's `actualMaxIndex`, which is the
    /// table's last usable index after `/HiVal` is clamped by the table's own
    /// length.
    Indexed {
        table: Vec<[u8; 3]>,
    },
}

impl ColorSpace {
    /// `PDColorSpace.getNumberOfComponents`. `Indexed` reports one, because an
    /// indexed sample *is* one component; the three come out of the lookup.
    const fn components(&self) -> usize {
        1
    }

    /// `PDColorSpace.getDefaultDecode`.
    fn default_decode(&self, bits: u32) -> [f32; 2] {
        match self {
            Self::DeviceGray => [0.0, 1.0],
            // `[0, 2^bits - 1]`: an indexed decode array addresses table
            // entries, not a colour range.
            Self::Indexed { .. } => [0.0, (2f32).powi(bits as i32) - 1.0],
        }
    }
}

/// Reads the image at `stream` the way `PDImageXObject.getImage()` does.
///
/// # Errors
///
/// [`Error::UnsupportedImage`] for any image shape outside the measured corpus,
/// and whatever [`Document::stream_data`] raises for the filter chain.
pub fn decode(document: &Document, stream: &Stream) -> Result<Raster> {
    let dictionary = document.flatten(&stream.dictionary);
    let width = positive(&dictionary, "Width")?;
    let height = positive(&dictionary, "Height")?;

    // PDFBox routes stencils through `getStencilImage` with the current fill
    // colour, which is a page-composition input rather than an image property.
    // No corpus image is one.
    if document.get(&dictionary, "ImageMask") == Some(&Object::Boolean(true)) {
        return Err(Error::UnsupportedImage {
            reason: "/ImageMask stencil".to_owned(),
        });
    }
    for key in ["Mask", "SMask"] {
        if document
            .get(&dictionary, key)
            .is_some_and(|object| !object.is_null())
        {
            return Err(Error::UnsupportedImage {
                reason: format!("/{key}"),
            });
        }
    }

    let bits = u32::try_from(
        document
            .get(&dictionary, "BitsPerComponent")
            .and_then(Object::as_i64)
            .ok_or_else(|| Error::UnsupportedImage {
                reason: "no /BitsPerComponent".to_owned(),
            })?,
    )
    .map_err(|_| Error::UnsupportedImage {
        reason: "negative /BitsPerComponent".to_owned(),
    })?;

    let space = color_space(document, &dictionary)?;
    let decode_array = decode_array(document, &dictionary, &space, bits)?;
    let data = document.stream_data(stream)?;

    // `SampledImageReader.getRGBImage`'s one branch.
    if bits == 1 && space.components() == 1 {
        return from_1_bit(&data, width, height, &space, decode_array);
    }
    from_any(&data, width, height, bits, &space, decode_array)
}

/// `COSDictionary.getInt` on a key that must be a positive integer.
fn positive(dictionary: &Dictionary, key: &str) -> Result<usize> {
    let value = dictionary
        .get(&Name::new(key))
        .and_then(Object::as_i64)
        .unwrap_or(0);
    usize::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| Error::UnsupportedImage {
            reason: format!("/{key} is {value}"),
        })
}

/// `PDColorSpace.create`, over the two the corpus uses.
fn color_space(document: &Document, dictionary: &Dictionary) -> Result<ColorSpace> {
    let object = document
        .get(dictionary, "ColorSpace")
        .ok_or_else(|| Error::UnsupportedImage {
            reason: "no /ColorSpace".to_owned(),
        })?;
    match document.resolve(object) {
        Object::Name(name) if name.as_bytes() == b"DeviceGray" || name.as_bytes() == b"G" => {
            Ok(ColorSpace::DeviceGray)
        }
        Object::Array(entries) => indexed(document, entries),
        other => Err(Error::UnsupportedImage {
            reason: format!("/ColorSpace {}", describe(other)),
        }),
    }
}

/// `PDIndexed`'s constructor: `readColorTable` then `initRgbColorTable`.
fn indexed(document: &Document, entries: &[Object]) -> Result<ColorSpace> {
    let [family, base, hival, lookup] = entries else {
        return Err(Error::UnsupportedImage {
            reason: format!("/ColorSpace array of {} entries", entries.len()),
        });
    };
    if document.resolve(family).name().map(Name::as_bytes) != Some(b"Indexed".as_slice()) {
        return Err(Error::UnsupportedImage {
            reason: format!("/ColorSpace {}", describe(document.resolve(family))),
        });
    }
    // Only a `DeviceRGB` base is implemented, and only because it is what the
    // corpus has. `PDDeviceRGB.toRGBImage` is `image.setData(raster)`, a
    // straight copy into `TYPE_INT_RGB`, so the base conversion is the identity
    // and the palette bytes survive it unchanged. Any other base runs a real
    // colour transform and must not be guessed at.
    let base_components = match document.resolve(base).name().map(Name::as_bytes) {
        Some(b"DeviceRGB" | b"RGB") => 3usize,
        _ => {
            return Err(Error::UnsupportedImage {
                reason: format!("/Indexed base {}", describe(document.resolve(base))),
            });
        }
    };

    let hival = document
        .resolve(hival)
        .as_i64()
        .ok_or_else(|| Error::UnsupportedImage {
            reason: "/Indexed hival is not an integer".to_owned(),
        })?;
    let data = match document.resolve(lookup) {
        Object::String(bytes) => bytes.clone(),
        Object::Stream(stream) => document.stream_data(stream)?,
        other => {
            return Err(Error::UnsupportedImage {
                reason: format!("/Indexed lookup {}", describe(other)),
            });
        }
    };

    // `readColorTable`: `/HiVal` capped at 255, then capped again by how many
    // entries the lookup data can actually supply.
    let mut max_index = hival.clamp(0, 255) as usize;
    if data.len() / base_components < max_index + 1 {
        max_index = (data.len() / base_components).saturating_sub(1);
    }

    // `initRgbColorTable`: each byte becomes a float over 255, and then an int
    // over 255 again. The round trip is *not* the identity in `f32` for every
    // byte, and the truncating cast is Java's `(int)` rather than a round, so
    // this is written the long way on purpose.
    let mut table = Vec::with_capacity(max_index + 1);
    for index in 0..=max_index {
        let mut entry = [0u8; 3];
        for (component, slot) in entry.iter_mut().enumerate() {
            let byte = data
                .get(index * base_components + component)
                .copied()
                .unwrap_or(0);
            let normalized = f32::from(byte) / 255f32;
            *slot = (normalized * 255f32) as u8;
        }
        table.push(entry);
    }
    Ok(ColorSpace::Indexed { table })
}

/// `SampledImageReader.getDecodeArray`, narrowed to one component.
fn decode_array(
    document: &Document,
    dictionary: &Dictionary,
    space: &ColorSpace,
    bits: u32,
) -> Result<[f32; 2]> {
    let Some(object) = document.get(dictionary, "Decode") else {
        return Ok(space.default_decode(bits));
    };
    let Some(entries) = document.resolve(object).array() else {
        return Ok(space.default_decode(bits));
    };
    // PDFBox falls back to the default when the array's length disagrees with
    // the colour space, after a stencil-only special case that cannot apply
    // here because a stencil is already refused above.
    if entries.len() != space.components() * 2 {
        return Ok(space.default_decode(bits));
    }
    let mut decode = [0f32; 2];
    for (slot, entry) in decode.iter_mut().zip(entries) {
        *slot = document
            .resolve(entry)
            .as_f64()
            .ok_or_else(|| Error::UnsupportedImage {
                reason: "/Decode holds a non-number".to_owned(),
            })? as f32;
    }
    Ok(decode)
}

/// `SampledImageReader.from1Bit`, at subsampling 1 over the whole image.
///
/// The structure is kept close to the Java because two of its properties are
/// load-bearing and neither is obvious. A row short of `stride` bytes ends the
/// image *and leaves the remainder at zero* rather than truncating the raster,
/// and the inner loop stops at the image width, so the padding bits in a row's
/// last byte are skipped rather than emitted.
fn from_1_bit(
    data: &[u8],
    width: usize,
    height: usize,
    space: &ColorSpace,
    decode: [f32; 2],
) -> Result<Raster> {
    if !matches!(space, ColorSpace::DeviceGray) {
        // The non-gray branch builds a banded raster and runs it through
        // `toRGBImage`, which for an indexed space indexes with 0 and 255. No
        // corpus image takes it.
        return Err(Error::UnsupportedImage {
            reason: "1-bit image in a colour space other than DeviceGray".to_owned(),
        });
    }
    let stride = width.div_ceil(8);
    // `decode[0] < decode[1] ? 0 : -1`, which is an XOR mask over each byte.
    let invert = if decode[0] < decode[1] {
        0x00u8
    } else {
        0xFFu8
    };

    let mut samples = vec![0u8; width * height];
    let mut at = 0usize;
    for row in 0..height {
        let start = row * stride;
        let available = data.len().saturating_sub(start.min(data.len()));
        let read = available.min(stride);
        let bytes = &data[start.min(data.len())..start.min(data.len()) + read];

        let mut x = 0usize;
        for byte in bytes {
            // Java holds this in an `int` and tests the sign bit, so the byte
            // lands in the top eight bits and is shifted out to the left.
            let mut value = u32::from(byte ^ invert) << (24 + (x & 7));
            let count = (8 - (x & 7)).min(width - x);
            for _ in 0..count {
                if value & 0x8000_0000 != 0 {
                    samples[at] = 255;
                }
                at += 1;
                x += 1;
                value <<= 1;
            }
            if x >= width {
                break;
            }
        }
        if read != stride {
            // "premature EOF, image will be incomplete": PDFBox warns and stops,
            // keeping the rows it has and leaving the rest black.
            break;
        }
    }

    Ok(Raster {
        width,
        height,
        bands: 1,
        samples,
    })
}

/// `SampledImageReader.fromAny`, at subsampling 1 with no colour key.
fn from_any(
    data: &[u8],
    width: usize,
    height: usize,
    bits: u32,
    space: &ColorSpace,
    decode: [f32; 2],
) -> Result<Raster> {
    let ColorSpace::Indexed { table } = space else {
        return Err(Error::UnsupportedImage {
            reason: format!("{bits}-bit image outside the measured corpus"),
        });
    };
    if bits == 0 || bits > 8 {
        return Err(Error::UnsupportedImage {
            reason: format!("/BitsPerComponent {bits}"),
        });
    }
    let sample_max = (2f32).powi(bits as i32) - 1f32;
    // Rows are padded to a byte boundary, and the padding is *read* rather than
    // seeked past, which matters only in that it keeps the bit reader aligned.
    let row_bits = width * space.components() * bits as usize;
    let padding = if row_bits % 8 > 0 {
        8 - row_bits % 8
    } else {
        0
    };

    let max_index = table.len().saturating_sub(1);
    let mut samples = Vec::with_capacity(width * height * 3);
    let mut reader = Bits::new(data);
    for _ in 0..height {
        for _ in 0..width {
            let value = reader.read(bits)?;
            // interpolate to domain
            let output = decode[0] + (value as f32 * ((decode[1] - decode[0]) / sample_max));
            // An indexed space takes the raw value: the byte raster cannot be
            // reversed by the colour space without knowing the bit depth.
            let index = java_round(output) as usize;
            let entry = table
                .get(index.min(max_index))
                .copied()
                .unwrap_or([0, 0, 0]);
            samples.extend_from_slice(&entry);
        }
        reader.read(padding as u32)?;
    }

    Ok(Raster {
        width,
        height,
        bands: 3,
        samples,
    })
}

/// `Math.round(float)`: floor of the value plus a half, not a ties-even round.
fn java_round(value: f32) -> i32 {
    (value + 0.5).floor() as i32
}

/// An MSB-first bit reader, as `MemoryCacheImageInputStream.readBits` is.
struct Bits<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Bits<'a> {
    const fn new(data: &'a [u8]) -> Self {
        Self { data, at: 0 }
    }

    /// Reads `count` bits. Java throws at the end of the stream, and the caller
    /// here would rather say which image ran out than unwind.
    fn read(&mut self, count: u32) -> Result<u32> {
        let mut value = 0u32;
        for _ in 0..count {
            let byte = self
                .data
                .get(self.at / 8)
                .ok_or(Error::CorruptStream { filter: "image" })?;
            let bit = (byte >> (7 - (self.at % 8))) & 1;
            value = (value << 1) | u32::from(bit);
            self.at += 1;
        }
        Ok(value)
    }
}

/// Names an object for a refusal message.
fn describe(object: &Object) -> String {
    match object {
        Object::Name(name) => format!("/{}", String::from_utf8_lossy(name.as_bytes())),
        Object::Array(entries) => format!("an array of {} entries", entries.len()),
        Object::Stream(_) => "a stream".to_owned(),
        Object::Dictionary(_) => "a dictionary".to_owned(),
        Object::Null => "null".to_owned(),
        _ => "an unexpected object".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_set_bit_becomes_full_white_and_a_clear_bit_stays_black() {
        // 0b1010_0000 in a four-pixel row: two rows of `1 0 1 0`.
        let raster = from_1_bit(&[0xA0, 0xA0], 4, 2, &ColorSpace::DeviceGray, [0.0, 1.0])
            .expect("a gray raster");
        assert_eq!(raster.bands, 1);
        assert_eq!(raster.samples, vec![255, 0, 255, 0, 255, 0, 255, 0]);
    }

    #[test]
    fn a_reversed_decode_array_inverts_every_sample() {
        let raster =
            from_1_bit(&[0xA0], 4, 1, &ColorSpace::DeviceGray, [1.0, 0.0]).expect("a gray raster");
        assert_eq!(raster.samples, vec![0, 255, 0, 255]);
    }

    #[test]
    fn padding_bits_in_a_rows_last_byte_are_skipped() {
        // Three-pixel rows: one byte each, and bits 3..8 must not be emitted.
        let raster = from_1_bit(
            &[0b1110_0000, 0b0000_0000],
            3,
            2,
            &ColorSpace::DeviceGray,
            [0.0, 1.0],
        )
        .expect("a gray raster");
        assert_eq!(raster.samples, vec![255, 255, 255, 0, 0, 0]);
    }

    #[test]
    fn a_short_row_ends_the_image_and_leaves_the_rest_black() {
        // Two rows wanted, one row of data: PDFBox warns and keeps what it has.
        let raster =
            from_1_bit(&[0xFF], 8, 2, &ColorSpace::DeviceGray, [0.0, 1.0]).expect("a gray raster");
        assert_eq!(&raster.samples[..8], [255u8; 8]);
        assert_eq!(&raster.samples[8..], [0u8; 8]);
    }

    #[test]
    fn an_indexed_sample_becomes_its_palette_entry() {
        let space = ColorSpace::Indexed {
            table: vec![[10, 20, 30], [40, 50, 60], [70, 80, 90]],
        };
        // Two 4-bit samples per byte: indices 0 and 2, then 1 and 1.
        let raster = from_any(&[0x02, 0x11], 2, 2, 4, &space, [0.0, 15.0]).expect("an RGB raster");
        assert_eq!(raster.bands, 3);
        assert_eq!(
            raster.samples,
            vec![10, 20, 30, 70, 80, 90, 40, 50, 60, 40, 50, 60]
        );
    }

    #[test]
    fn an_index_past_the_palette_clamps_to_its_last_entry() {
        let space = ColorSpace::Indexed {
            table: vec![[1, 2, 3], [4, 5, 6]],
        };
        let raster = from_any(&[0x50], 1, 1, 4, &space, [0.0, 15.0]).expect("an RGB raster");
        assert_eq!(raster.samples, vec![4, 5, 6]);
    }

    #[test]
    fn java_rounds_a_half_upwards_rather_than_to_even() {
        assert_eq!(java_round(0.5), 1);
        assert_eq!(java_round(1.5), 2);
        assert_eq!(java_round(2.5), 3);
        assert_eq!(java_round(-0.5), 0);
    }

    #[test]
    fn shapes_outside_the_measured_corpus_are_refused_by_name() {
        let error = from_1_bit(
            &[0x00],
            1,
            1,
            &ColorSpace::Indexed { table: Vec::new() },
            [0.0, 1.0],
        )
        .expect_err("an unsupported shape");
        assert!(matches!(error, Error::UnsupportedImage { .. }));
    }
}
