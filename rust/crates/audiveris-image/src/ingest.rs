// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{error::Error, fmt, path::Path};

use image::DynamicImage;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrayRaster {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

impl GrayRaster {
    #[must_use]
    pub fn from_dynamic(image: &DynamicImage) -> Self {
        let rgba = image.to_rgba8();
        let width = usize::try_from(rgba.width()).expect("image width fits usize");
        let height = usize::try_from(rgba.height()).expect("image height fits usize");
        let pixels = rgba
            .pixels()
            // Audiveris Picture.adjustImageFormat uses the maximum RGB channel
            // and deliberately discards alpha for RGB(A) inputs.
            .map(|pixel| pixel.0[0].max(pixel.0[1]).max(pixel.0[2]))
            .collect();
        Self {
            width,
            height,
            pixels,
        }
    }

    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> usize {
        self.height
    }

    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Stable non-cryptographic summary for cross-runtime fixture diagnostics.
    #[must_use]
    pub fn fnv1a64(&self) -> u64 {
        fnv1a64_bytes(&self.pixels)
    }
}

/// Stable non-cryptographic summary for cross-runtime fixture diagnostics.
#[must_use]
pub fn fnv1a64_bytes(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

#[derive(Debug)]
pub enum LoadError {
    Image(image::ImageError),
    /// libjpeg produced a component count the max-channel rule does not cover,
    /// such as a CMYK or YCCK Adobe JPEG.
    UnsupportedJpegComponents(usize),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Image(error) => error.fmt(f),
            Self::UnsupportedJpegComponents(components) => write!(
                f,
                "JPEG decoded to {components} components; only grayscale and RGB are supported"
            ),
        }
    }
}

impl Error for LoadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Image(error) => Some(error),
            Self::UnsupportedJpegComponents(_) => None,
        }
    }
}

impl From<image::ImageError> for LoadError {
    fn from(error: image::ImageError) -> Self {
        Self::Image(error)
    }
}

pub fn load_max_channel_gray(path: impl AsRef<Path>) -> Result<GrayRaster, LoadError> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).map_err(image::ImageError::IoError)?;
    if is_jpeg(&bytes) {
        return decode_jpeg_max_channel_gray(&bytes);
    }
    let image = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(image::ImageError::IoError)?
        .decode()?;
    Ok(GrayRaster::from_dynamic(&image))
}

/// JPEG start-of-image marker.
fn is_jpeg(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFF, 0xD8, 0xFF])
}

/// Decodes a JPEG through libjpeg-turbo with libjpeg's own defaults.
///
/// Java's `ImageIO` JPEG reader is libjpeg-backed, and the pure-Rust decoders
/// do not reproduce it: on the one JPEG in the example corpus `zune-jpeg`
/// differs from libjpeg on 177046 of 5018112 grayscale samples by up to 4, and
/// `jpeg-decoder` on 224558 by up to 3. That is enough to flip 844 binary
/// pixels through the adaptive threshold and perturb every GRID result derived
/// from them, so JPEG decoding goes through the same library Java uses.
///
/// `JDCT_ISLOW` and fancy upsampling are libjpeg's defaults; they are set
/// explicitly because they are what parity depends on, not incidental.
#[allow(
    unsafe_code,
    reason = "libjpeg is a C library; this is the only FFI in the workspace"
)]
fn decode_jpeg_max_channel_gray(bytes: &[u8]) -> Result<GrayRaster, LoadError> {
    use mozjpeg_sys::{
        J_DCT_METHOD, boolean, jpeg_create_decompress, jpeg_decompress_struct,
        jpeg_destroy_decompress, jpeg_error_mgr, jpeg_finish_decompress, jpeg_mem_src,
        jpeg_read_header, jpeg_read_scanlines, jpeg_start_decompress, jpeg_std_error,
    };

    // SAFETY: every raw pointer handed to libjpeg points at a live local or at
    // `bytes`, which outlives the decompress object; the struct is zeroed before
    // `jpeg_create_decompress` initializes it, and destroyed on both exits.
    unsafe {
        let mut err: jpeg_error_mgr = std::mem::zeroed();
        let mut cinfo: jpeg_decompress_struct = std::mem::zeroed();
        cinfo.common.err = jpeg_std_error(&mut err);
        jpeg_create_decompress(&mut cinfo);
        jpeg_mem_src(&mut cinfo, bytes.as_ptr(), bytes.len() as _);
        jpeg_read_header(&mut cinfo, boolean::from(true));
        cinfo.dct_method = J_DCT_METHOD::JDCT_ISLOW;
        cinfo.do_fancy_upsampling = boolean::from(true);
        jpeg_start_decompress(&mut cinfo);

        let width = cinfo.output_width as usize;
        let height = cinfo.output_height as usize;
        let components = cinfo.output_components as usize;
        if !matches!(components, 1 | 3) {
            jpeg_destroy_decompress(&mut cinfo);
            return Err(LoadError::UnsupportedJpegComponents(components));
        }

        let stride = width * components;
        let mut buffer = vec![0u8; stride * height];
        while cinfo.output_scanline < cinfo.output_height {
            let offset = cinfo.output_scanline as usize * stride;
            let mut rows = [buffer.as_mut_ptr().add(offset)];
            jpeg_read_scanlines(&mut cinfo, rows.as_mut_ptr(), 1);
        }
        jpeg_finish_decompress(&mut cinfo);
        jpeg_destroy_decompress(&mut cinfo);

        // Same rule as `GrayRaster::from_dynamic`: Audiveris
        // `Picture.adjustImageFormat` takes the maximum RGB channel.
        let pixels = buffer
            .chunks_exact(components)
            .map(|sample| match sample {
                [gray] => *gray,
                [red, green, blue] => *red.max(green).max(blue),
                _ => unreachable!("component count validated above"),
            })
            .collect();
        Ok(GrayRaster {
            width,
            height,
            pixels,
        })
    }
}

#[cfg(test)]
mod tests {
    use image::{ImageBuffer, Luma, Rgba};

    use super::*;

    #[test]
    fn preserves_gray_samples() {
        let image = DynamicImage::ImageLuma8(
            ImageBuffer::<Luma<u8>, _>::from_raw(3, 1, vec![0, 127, 255]).unwrap(),
        );
        let raster = GrayRaster::from_dynamic(&image);
        assert_eq!((raster.width(), raster.height()), (3, 1));
        assert_eq!(raster.pixels(), [0, 127, 255]);
    }

    #[test]
    fn matches_audiveris_max_rgb_and_discards_alpha() {
        let image = DynamicImage::ImageRgba8(
            ImageBuffer::<Rgba<u8>, _>::from_raw(2, 1, vec![10, 80, 30, 0, 200, 20, 100, 255])
                .unwrap(),
        );
        let raster = GrayRaster::from_dynamic(&image);
        assert_eq!(raster.pixels(), [80, 200]);
        assert_eq!(raster.fnv1a64(), 0x0941_a007_b5d1_18e5);
    }
}
