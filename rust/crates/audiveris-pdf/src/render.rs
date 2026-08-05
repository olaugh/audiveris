// SPDX-License-Identifier: AGPL-3.0-or-later

//! The whole render: `renderImageWithDPI(page, 300, ImageType.GRAY)`.
//!
//! This is the layer everything else was built for. Audiveris never extracts an
//! image from a PDF; it renders the page and binarizes the result, so this
//! function's output is what GRID eventually reads.
//!
//! # The destination
//!
//! `ImageType.GRAY` is `TYPE_BYTE_GRAY`, one band. `renderImage` fills it with
//! `Color.WHITE` through `clearRect` before drawing, so an unwritten pixel is
//! **255**, and the margins a placement leaves uncovered stay white rather than
//! extrapolating the image's edge.
//!
//! # Which resampler, and how that was settled
//!
//! `-Dsun.java2d.trace=count` reported `ScaledBlit(ByteGray, SrcNoEa, ByteGray)`
//! on some pages and `TransformHelper(ByteGray, SrcNoEa, IntArgbPre)` on
//! others, and the note left with that observation guessed at `DrawImage`'s
//! `transformState` ladder. Reading the source says otherwise, and it is worth
//! recording because the ladder is a dead end:
//!
//! `DrawImage.renderImageScale` -- the only route to a `ScaledBlit` -- returns
//! false immediately unless the interpolation hint is nearest-neighbour:
//!
//! ```text
//! // Currently only NEAREST_NEIGHBOR interpolation is implemented
//! // for ScaledBlit operations.
//! if (interpType != AffineTransformOp.TYPE_NEAREST_NEIGHBOR) return false;
//! ```
//!
//! So with a bicubic hint Java2D can never reach a `ScaledBlit`, whatever the
//! transform looks like. The hint is what changes, and **PDFBox changes it**.
//! `PageDrawer.drawImage` flips it, per draw, when the image is being scaled
//! *up*:
//!
//! ```text
//! boolean isScaledUp =
//!     bim.getWidth()  <= abs(round(ctm.getScalingFactorX() * xformScalingFactorX)) ||
//!     bim.getHeight() <= abs(round(ctm.getScalingFactorY() * xformScalingFactorY));
//! if (isScaledUp) graphics.setRenderingHint(KEY_INTERPOLATION, NEAREST_NEIGHBOR);
//! ```
//!
//! and restores the hints afterwards, which is exactly why the earlier
//! observation found the hint reading `Bicubic` both before and after the draw.
//! The flip lives entirely inside one `drawImage` call.
//!
//! # The workaround that does not fire, and why that has to be checked
//!
//! `PageDrawer.drawBufferedImage` carries a second path: when a draw scales
//! down past `imageDownscalingOptimizationThreshold`, PDFBox abandons
//! `drawImage` and pre-scales through `Image.getScaledInstance(w, h,
//! SCALE_SMOOTH)`, a completely different resampler. The threshold defaults to
//! **0.5**, and every corpus page downscales by about 0.93, so the threshold
//! alone does not save us -- but the branch also requires
//!
//! ```text
//! VALUE_RENDER_QUALITY.equals(graphics.getRenderingHint(KEY_RENDERING))
//! ```
//!
//! and Audiveris builds its own `RenderingHints` holding only `ANTIALIASING`
//! and `INTERPOLATION` (`ImageLoading.PdfboxLoader.getImage`). `KEY_RENDERING`
//! is therefore unset, the equality fails, and the workaround never runs. That
//! is load-bearing and fragile: if Audiveris ever adds `KEY_RENDERING`, or
//! renders at a resolution where a scale drops below 0.5 *and* that hint is
//! present, the resampler changes completely and none of `transform.rs` applies.
//! [`Renderer::hints_reach_the_downscaling_workaround`] exists to say so.

use crate::content::{self, Matrix};
use crate::document::{Document, Page};
use crate::error::{Error, Result};
use crate::object::Object;
use crate::raster;
use crate::transform::{GrayImage, Placement, draw_bicubic_into};

/// Audiveris's `pdfResolution` default.
pub const AUDIVERIS_DPI: f32 = 300.0;

/// What a draw was resampled with, which is worth reporting rather than hiding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interpolation {
    /// Java2D's bicubic transform, through `TransformHelper`.
    Bicubic,
    /// Nearest neighbour, through a `ScaledBlit`, because PDFBox decided the
    /// image was being scaled up.
    NearestNeighbour,
}

/// A rendered page, and how it got that way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    /// The page raster, one band, row-major.
    pub image: GrayImage,
    /// One entry per draw, in order.
    pub interpolations: Vec<Interpolation>,
}

/// Renders one page exactly as `ImageLoading.PdfboxLoader.getImage` does.
///
/// # Errors
///
/// [`Error::UnsupportedImage`] for anything outside the measured corpus --
/// a rotated page, an operator this port does not interpret, an image whose
/// samples it cannot produce -- and whatever the filter chain raises.
pub fn page(document: &Document, page: &Page, dpi: f32) -> Result<Rendered> {
    let device = content::device(page, dpi / 72f32)?;
    let draws = content::image_draws(document, page)?;

    // `g.setBackground(Color.WHITE); g.clearRect(...)`.
    let mut image = GrayImage {
        width: device.width as usize,
        height: device.height as usize,
        samples: vec![255u8; device.width as usize * device.height as usize],
    };
    let mut interpolations = Vec::with_capacity(draws.len());

    let xobjects = document
        .get(&page.resources, "XObject")
        .and_then(Object::dictionary)
        .cloned()
        .unwrap_or_default();

    for draw in &draws {
        let stream = xobjects
            .get(&draw.resource)
            .map(|object| document.resolve(object))
            .and_then(Object::stream)
            .ok_or_else(|| Error::UnsupportedImage {
                reason: format!(
                    "/{} is not an image stream",
                    String::from_utf8_lossy(draw.resource.as_bytes())
                ),
            })?;
        let decoded = raster::decode(document, stream)?;
        if decoded.bands != 1 {
            // A three-band source draws through a different set of Java2D
            // loops, and the gray destination then applies a colour reduction
            // that has its own parity question. One corpus page is like this.
            return Err(Error::UnsupportedImage {
                reason: format!("a {}-band source into a gray destination", decoded.bands),
            });
        }
        let source = GrayImage {
            width: decoded.width,
            height: decoded.height,
            samples: decoded.samples,
        };

        let interpolation = interpolation(&device.transform, draw.ctm, &source);
        interpolations.push(interpolation);
        if interpolation == Interpolation::NearestNeighbour {
            return Err(Error::UnsupportedImage {
                reason: "a scaled-up draw, which PDFBox resamples with a nearest-neighbour \
                         ScaledBlit"
                    .to_owned(),
            });
        }

        let composed =
            content::draw_transform(&device.transform, draw.ctm, source.width, source.height);
        let [scale_x, shear_y, shear_x, scale_y, left, top] = composed.matrix();
        draw_bicubic_into(
            &mut image,
            &source,
            Placement {
                scale_x,
                shear_y,
                shear_x,
                scale_y,
                left,
                top,
            },
        );
    }

    Ok(Rendered {
        image,
        interpolations,
    })
}

/// `PageDrawer.drawImage`'s per-draw interpolation decision.
///
/// Only reached because no corpus image sets `/Interpolate`; PDFBox skips the
/// whole test when it is true.
fn interpolation(device: &crate::affine::Affine, ctm: Matrix, source: &GrayImage) -> Interpolation {
    // `drawPage` stores the device factors already absolute; `drawImage` takes
    // the absolute value again, of the *rounded product*, not of the CTM's own
    // factor. Both are kept where Java puts them.
    let device_matrix = Matrix::from_affine(*device);
    let across = java_round(ctm.scaling_factor_x() * device_matrix.scaling_factor_x().abs()).abs();
    let down = java_round(ctm.scaling_factor_y() * device_matrix.scaling_factor_y().abs()).abs();
    let scaled_up = i64::try_from(source.width).unwrap_or(i64::MAX) <= i64::from(across)
        || i64::try_from(source.height).unwrap_or(i64::MAX) <= i64::from(down);
    if scaled_up {
        Interpolation::NearestNeighbour
    } else {
        Interpolation::Bicubic
    }
}

/// `Math.round(float)`, which is a floor of the value plus a half.
fn java_round(value: f32) -> i32 {
    (value + 0.5).floor() as i32
}

/// Whether a set of rendering hints would reach PDFBox's downscaling
/// workaround, which replaces Java2D's resampler with
/// `Image.getScaledInstance`.
///
/// Audiveris's hints do not, and this port only reproduces the Java2D path. It
/// is exposed so the assumption is checkable rather than buried in a comment.
#[must_use]
pub const fn hints_reach_the_downscaling_workaround(
    rendering_quality_hint_set: bool,
    interpolation_is_bicubic: bool,
    smallest_scale: f32,
    threshold: f32,
) -> bool {
    rendering_quality_hint_set && interpolation_is_bicubic && smallest_scale < threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audiveris_hints_do_not_reach_the_downscaling_workaround() {
        // The corpus's real scale, well under the 0.5 default threshold's
        // reach only because the rendering hint is absent.
        assert!(!hints_reach_the_downscaling_workaround(
            false, true, 0.3, 0.5
        ));
        // With the hint present it would fire, and none of `transform.rs`
        // would describe the output.
        assert!(hints_reach_the_downscaling_workaround(true, true, 0.3, 0.5));
        // A corpus-typical 0.93 downscale is above the threshold either way.
        assert!(!hints_reach_the_downscaling_workaround(
            true, true, 0.93, 0.5
        ));
    }

    #[test]
    fn java_rounds_a_half_upwards() {
        assert_eq!(java_round(0.5), 1);
        assert_eq!(java_round(-0.5), 0);
        assert_eq!(java_round(2145.6), 2146);
    }
}
