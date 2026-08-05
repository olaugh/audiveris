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
    Jpeg(audiveris_jpeg::JpegError),
    Pdf(audiveris_pdf::Error),
    /// A sheet id outside `1..=count`, which is `AbstractLoader.checkId`.
    NoSuchImage {
        id: usize,
        count: usize,
    },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Image(error) => error.fmt(f),
            Self::Jpeg(error) => error.fmt(f),
            Self::Pdf(error) => error.fmt(f),
            Self::NoSuchImage { id, count } => {
                write!(f, "no sheet {id} in an input holding {count}")
            }
        }
    }
}

impl Error for LoadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Image(error) => Some(error),
            Self::Jpeg(error) => Some(error),
            Self::Pdf(error) => Some(error),
            Self::NoSuchImage { .. } => None,
        }
    }
}

impl From<audiveris_pdf::Error> for LoadError {
    fn from(error: audiveris_pdf::Error) -> Self {
        Self::Pdf(error)
    }
}

impl From<audiveris_jpeg::JpegError> for LoadError {
    fn from(error: audiveris_jpeg::JpegError) -> Self {
        Self::Jpeg(error)
    }
}

impl From<image::ImageError> for LoadError {
    fn from(error: image::ImageError) -> Self {
        Self::Image(error)
    }
}

/// One input file, and the images in it: `ImageLoading.Loader`.
///
/// Audiveris treats an input as a *book* of sheets rather than one image, and
/// a PDF is the only format that supplies more than one. Sheet ids are
/// **one-based**, as `Loader.getImage(int id)` is; `PdfboxLoader` renders page
/// `id - 1`.
#[derive(Debug)]
pub enum Loader {
    /// A single-image raster, which is ImageIO's job in Java.
    Raster(Box<GrayRaster>),
    /// A PDF, whose pages are rendered on demand.
    Pdf(Box<audiveris_pdf::Document>),
}

impl Loader {
    /// `ImageLoading.getLoader`.
    ///
    /// The dispatch is on the **file extension**, case-insensitively, not on
    /// the file's magic bytes. That is Java's rule, and reproducing it matters
    /// in both directions: a PDF named `.png` goes to ImageIO there and fails,
    /// and sniffing the header here would make the port accept an input
    /// Audiveris rejects.
    ///
    /// # Errors
    ///
    /// Whatever the decoder or the PDF parser raises.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, LoadError> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(image::ImageError::IoError)?;
        let is_pdf = path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"));
        if is_pdf {
            return Ok(Self::Pdf(Box::new(audiveris_pdf::Document::parse(&bytes)?)));
        }
        if is_jpeg(&bytes) {
            return Ok(Self::Raster(Box::new(decode_jpeg_max_channel_gray(
                &bytes,
            )?)));
        }
        let image = image::ImageReader::new(std::io::Cursor::new(&bytes))
            .with_guessed_format()
            .map_err(image::ImageError::IoError)?
            .decode()?;
        Ok(Self::Raster(Box::new(GrayRaster::from_dynamic(&image))))
    }

    /// `Loader.getImageCount`.
    #[must_use]
    pub fn image_count(&self) -> usize {
        match self {
            Self::Raster(_) => 1,
            Self::Pdf(document) => document.page_count(),
        }
    }

    /// `Loader.getImage(id)`, with `id` counted from one.
    ///
    /// A PDF page is *rendered*, not extracted:
    /// `renderImageWithDPI(page, 300, ImageType.GRAY)`. The result is already
    /// single-band gray, so `Picture.adjustImageFormat`'s maximum-channel rule
    /// is the identity on it -- `max(v, v, v)` is `v` -- and the samples reach
    /// binarization exactly as rendered.
    ///
    /// # Errors
    ///
    /// [`LoadError::NoSuchImage`] outside `1..=image_count`, and whatever the
    /// render raises for a page this port refuses.
    pub fn image(&self, id: usize) -> Result<GrayRaster, LoadError> {
        let count = self.image_count();
        if id < 1 || id > count {
            return Err(LoadError::NoSuchImage { id, count });
        }
        match self {
            Self::Raster(raster) => Ok((**raster).clone()),
            Self::Pdf(document) => {
                let page = document.page(id - 1)?;
                let rendered = audiveris_pdf::render::page(
                    document,
                    &page,
                    audiveris_pdf::render::AUDIVERIS_DPI,
                )?;
                Ok(GrayRaster {
                    width: rendered.image.width,
                    height: rendered.image.height,
                    pixels: rendered.image.samples,
                })
            }
        }
    }
}

/// The first sheet of `path`, which is the whole input for every format but
/// PDF.
///
/// # Errors
///
/// Whatever [`Loader::open`] and [`Loader::image`] raise.
pub fn load_max_channel_gray(path: impl AsRef<Path>) -> Result<GrayRaster, LoadError> {
    Loader::open(path)?.image(1)
}

/// JPEG start-of-image marker.
fn is_jpeg(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFF, 0xD8, 0xFF])
}

/// Decodes a JPEG with the port's own libjpeg-equivalent decoder.
///
/// Java's `ImageIO` JPEG reader is libjpeg-backed, and the pure-Rust decoders
/// in the ecosystem do not reproduce it: `zune-jpeg` differs from libjpeg on
/// 177046 of 5018112 grayscale samples on the corpus JPEG and `jpeg-decoder` on
/// 224558, which is enough to flip 844 binary pixels through the adaptive
/// threshold and perturb every GRID result derived from them.
/// [`audiveris_jpeg`] exists to close that, and its differential test requires
/// every sample to equal libjpeg's.
fn decode_jpeg_max_channel_gray(bytes: &[u8]) -> Result<GrayRaster, LoadError> {
    let (width, height, pixels) = audiveris_jpeg::decode_max_channel_gray(bytes)?;
    Ok(GrayRaster {
        width,
        height,
        pixels,
    })
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
