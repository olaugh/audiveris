// SPDX-License-Identifier: AGPL-3.0-or-later

//! The content stream, and the transform every image draw lands under.
//!
//! Audiveris renders through `PDFRenderer.renderImageWithDPI(page, 300,
//! ImageType.GRAY)`, so reproducing a draw means reproducing two things: the
//! device transform `PDFRenderer` and `PageDrawer` install before any operator
//! runs, and the current transformation matrix the content stream has built by
//! the time it reaches a `Do`.
//!
//! # What the corpus actually contains
//!
//! Measured rather than assumed, with a probe over all 189 pages. There are
//! exactly four operators -- `q`, `Q`, `cm`, `Do` -- and exactly two page
//! shapes:
//!
//! ```text
//!   36 x  cm Do
//!  153 x  q cm Do Q
//! ```
//!
//! Every page draws exactly one image. The 36 pages that never push a state are
//! worth noting: they concatenate straight onto the initial CTM, so a reader
//! that assumed a balanced `q`/`Q` around every draw would still work here and
//! would be wrong in general. Any other operator is refused by name rather than
//! skipped, because silently ignoring an operator that moves the CTM -- a
//! clipping `re W n`, a `gs` with a matrix -- would place the image wrongly and
//! look like a rasterizer bug.
//!
//! # The two float traps
//!
//! Both are already known and both are load-bearing:
//!
//! - **The CTM is a `float` matrix.** PDFBox's `Matrix` holds `float[9]` and
//!   `cm` multiplies in `float`, so `633.5724` is not the double `633.5724` but
//!   `633.57238769531250`. [`Matrix`] here is `f32` for that reason, and only
//!   widens on the way into [`Affine`], exactly where `createAffineTransform`
//!   does.
//! - **The DPI scale is computed in `float`.** `renderImageWithDPI` passes
//!   `dpi / 72f`, so a 792 pt page is 3299.999874 and floors to **3299** rather
//!   than 3300.

use crate::affine::Affine;
use crate::document::{Document, Page};
use crate::error::{Error, Result};
use crate::lexer::Lexer;
use crate::object::{Name, Object};

/// PDFBox's `Matrix`: a 3x3 in `float`, of which six terms are used.
///
/// Stored in the same order `Matrix`'s `single[]` is, so the mapping into an
/// [`Affine`] can be read against `createAffineTransform` directly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix {
    /// `a b 0 / c d 0 / e f 1`, row-major, as `single[0..9]`.
    values: [f32; 9],
}

impl Default for Matrix {
    fn default() -> Self {
        Self::identity()
    }
}

impl Matrix {
    /// `new Matrix()`.
    #[must_use]
    pub const fn identity() -> Self {
        Self {
            values: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        }
    }

    /// `new Matrix(a, b, c, d, e, f)`.
    #[must_use]
    pub const fn new(a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) -> Self {
        Self {
            values: [a, b, 0.0, c, d, 0.0, e, f, 1.0],
        }
    }

    /// `Matrix.concatenate`, which is `this = other * this` in `float`.
    pub fn concatenate(&mut self, other: &Self) {
        let (a, b) = (&other.values, &self.values);
        let mut result = [0f32; 9];
        for row in 0..3 {
            for column in 0..3 {
                // Java's `multiplyArrays` accumulates in this term order.
                result[row * 3 + column] = a[row * 3] * b[column]
                    + a[row * 3 + 1] * b[3 + column]
                    + a[row * 3 + 2] * b[6 + column];
            }
        }
        self.values = result;
    }

    /// `new Matrix(AffineTransform)`, narrowing each term back to `float`.
    #[must_use]
    pub fn from_affine(transform: Affine) -> Self {
        let [m00, m10, m01, m11, m02, m12] = transform.matrix();
        Self::new(
            m00 as f32, m10 as f32, m01 as f32, m11 as f32, m02 as f32, m12 as f32,
        )
    }

    /// `Matrix.getScalingFactorX`.
    ///
    /// The guard is `Float.compare(single[1], 0.0f) != 0`, not `!= 0.0f`, and
    /// the difference is real: `Float.compare` orders `-0.0` below `+0.0`, so a
    /// shear term of `-0.0` takes the square-root branch rather than the plain
    /// one. `total_cmp` is Rust's spelling of the same ordering.
    #[must_use]
    pub fn scaling_factor_x(&self) -> f32 {
        if self.values[1].total_cmp(&0.0f32) == std::cmp::Ordering::Equal {
            return self.values[0];
        }
        hypotenuse(self.values[0], self.values[1])
    }

    /// `Matrix.getScalingFactorY`, which reads the *other* diagonal's pair:
    /// `single[3]` is the x shear and `single[4]` the y scale.
    #[must_use]
    pub fn scaling_factor_y(&self) -> f32 {
        if self.values[3].total_cmp(&0.0f32) == std::cmp::Ordering::Equal {
            return self.values[4];
        }
        hypotenuse(self.values[4], self.values[3])
    }

    /// `Matrix.createAffineTransform`, widening each term to `double`.
    #[must_use]
    pub fn to_affine(self) -> Affine {
        Affine::new(
            f64::from(self.values[0]),
            f64::from(self.values[1]),
            f64::from(self.values[3]),
            f64::from(self.values[4]),
            f64::from(self.values[6]),
            f64::from(self.values[7]),
        )
    }
}

/// `(float) Math.sqrt(Math.pow(a, 2) + Math.pow(b, 2))`.
///
/// Written as Java writes it: both terms widen to `double`, the sum and root
/// happen there, and only the result narrows back to `float`. Not `f32::hypot`,
/// which is a different computation with different rounding.
fn hypotenuse(a: f32, b: f32) -> f32 {
    let (a, b) = (f64::from(a), f64::from(b));
    (a * a + b * b).sqrt() as f32
}

/// One `Do` of an image XObject, and the CTM it draws under.
#[derive(Debug, Clone, PartialEq)]
pub struct Draw {
    /// The XObject's name in the page's `/XObject` resource dictionary.
    pub resource: Name,
    /// The current transformation matrix at the `Do`.
    pub ctm: Matrix,
}

/// Walks a page's content stream and returns every image draw in order.
///
/// # Errors
///
/// [`Error::UnsupportedImage`] naming any operator outside the measured set,
/// and whatever decoding the content streams raises.
pub fn image_draws(document: &Document, page: &Page) -> Result<Vec<Draw>> {
    let content = contents(document, page)?;
    let xobjects = document
        .get(&page.resources, "XObject")
        .and_then(Object::dictionary)
        .cloned()
        .unwrap_or_default();

    let mut draws = Vec::new();
    let mut ctm = Matrix::identity();
    let mut stack: Vec<Matrix> = Vec::new();
    let mut operands: Vec<Object> = Vec::new();
    let mut lexer = Lexer::new(&content);

    loop {
        lexer.skip_space();
        let Some(byte) = lexer.peek() else { break };
        if byte.is_ascii_digit() || matches!(byte, b'+' | b'-' | b'.' | b'/' | b'(' | b'<' | b'[') {
            let Ok(object) = lexer.object() else { break };
            operands.push(object);
            continue;
        }
        let operator = lexer.keyword();
        if operator.is_empty() {
            break;
        }
        match operator {
            b"q" => stack.push(ctm),
            b"Q" => {
                // An unbalanced `Q` is ignored rather than fatal, as PDFBox's
                // `Restore` does when the stack is empty.
                if let Some(saved) = stack.pop() {
                    ctm = saved;
                }
            }
            b"cm" => {
                let numbers = trailing_numbers(&operands, 6)?;
                ctm.concatenate(&Matrix::new(
                    numbers[0], numbers[1], numbers[2], numbers[3], numbers[4], numbers[5],
                ));
            }
            b"Do" => {
                let Some(Object::Name(name)) = operands.last() else {
                    return Err(Error::UnsupportedImage {
                        reason: "`Do` without a name".to_owned(),
                    });
                };
                // Only image XObjects draw here. A `/Form` would need its own
                // content stream and matrix, and none is present.
                let subtype = xobjects
                    .get(name)
                    .map(|object| document.resolve(object))
                    .and_then(Object::dictionary)
                    .and_then(|dictionary| dictionary.get(&Name::new("Subtype")))
                    .and_then(Object::name)
                    .map(Name::as_bytes)
                    .map(<[u8]>::to_vec);
                match subtype.as_deref() {
                    Some(b"Image") => draws.push(Draw {
                        resource: name.clone(),
                        ctm,
                    }),
                    other => {
                        return Err(Error::UnsupportedImage {
                            reason: format!(
                                "`Do` on /{} of subtype {}",
                                String::from_utf8_lossy(name.as_bytes()),
                                other.map_or_else(
                                    || "nothing".to_owned(),
                                    |bytes| format!("/{}", String::from_utf8_lossy(bytes))
                                )
                            ),
                        });
                    }
                }
            }
            other => {
                return Err(Error::UnsupportedImage {
                    reason: format!(
                        "content-stream operator `{}`",
                        String::from_utf8_lossy(other)
                    ),
                });
            }
        }
        operands.clear();
    }
    Ok(draws)
}

/// A page's `/Contents`, decoded and concatenated.
///
/// An array of streams is one stream logically, and PDFBox inserts a newline
/// between the parts so that a token cannot span the join.
fn contents(document: &Document, page: &Page) -> Result<Vec<u8>> {
    let mut content = Vec::new();
    match document.get(&page.dictionary, "Contents") {
        Some(Object::Stream(stream)) => content.extend_from_slice(&document.stream_data(stream)?),
        Some(Object::Array(entries)) => {
            for entry in entries {
                if let Object::Stream(stream) = document.resolve(entry) {
                    content.extend_from_slice(&document.stream_data(stream)?);
                    content.push(b'\n');
                }
            }
        }
        _ => {}
    }
    Ok(content)
}

/// The last `count` operands as floats, which is what `COSNumber.floatValue`
/// hands the operator.
fn trailing_numbers(operands: &[Object], count: usize) -> Result<Vec<f32>> {
    if operands.len() < count {
        return Err(Error::UnsupportedImage {
            reason: format!(
                "an operator wanted {count} operands, got {}",
                operands.len()
            ),
        });
    }
    operands[operands.len() - count..]
        .iter()
        .map(|operand| {
            operand
                .as_f64()
                .map(|value| value as f32)
                .ok_or_else(|| Error::UnsupportedImage {
                    reason: "a numeric operand was not a number".to_owned(),
                })
        })
        .collect()
}

/// The size in pixels `renderImage` allocates, and the transform it installs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Device {
    /// The rendered page's width in pixels.
    pub width: u32,
    /// The rendered page's height in pixels.
    pub height: u32,
    /// `Graphics2D.getTransform()` as it stands when the first operator runs.
    pub transform: Affine,
}

/// Reproduces `PDFRenderer.renderImage` and `PageDrawer.drawPage`'s setup.
///
/// `scale` is `dpi / 72f` computed in `f32`, which Audiveris reaches as
/// `300f / 72f`. Doing that division in `f64` is the difference between a 3299
/// and a 3300 pixel page.
///
/// # Errors
///
/// [`Error::UnsupportedImage`] for a rotated page. No corpus page is rotated,
/// and the rotation path also calls `Math.toRadians` and `rotate`, whose
/// transcendental terms would need their own parity argument.
pub fn device(page: &Page, scale: f32) -> Result<Device> {
    if page.rotation != 0 {
        return Err(Error::UnsupportedImage {
            reason: format!("/Rotate {}", page.rotation),
        });
    }
    let crop = page.crop_box;
    let width_pt = crop.width();
    let height_pt = crop.height();

    // `(int) Math.max(Math.floor(widthPt * scale), 1)`, with the product in
    // `float` and the floor in `double`.
    let width = f64::from(width_pt * scale).floor().max(1.0) as u32;
    let height = f64::from(height_pt * scale).floor().max(1.0) as u32;

    // `PDFRenderer.transform`, then `PageDrawer.drawPage`.
    let mut transform = Affine::identity();
    transform.scale(f64::from(scale), f64::from(scale));
    transform.translate(0.0, f64::from(height_pt));
    transform.scale(1.0, -1.0);
    transform.translate(f64::from(-crop.lower_left_x), f64::from(-crop.lower_left_y));

    Ok(Device {
        width,
        height,
        transform,
    })
}

/// The transform `PageDrawer.drawImage` hands `Graphics2D.drawImage`.
///
/// The image's own space is the unit square with y upwards, so the placement
/// normalises out of it before the CTM applies: `scale(1/w, -1/h)` then
/// `translate(0, -h)`.
#[must_use]
pub fn draw_transform(device: &Affine, ctm: Matrix, width: usize, height: usize) -> Affine {
    let mut composed = *device;
    let mut placement = ctm.to_affine();
    placement.scale(1.0 / width as f64, -1.0 / height as f64);
    placement.translate(0.0, -(height as f64));
    composed.concatenate(&placement);
    composed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_float_matrix_keeps_the_value_a_float_can_hold() {
        let matrix = Matrix::new(515.0, 0.0, 0.0, 633.5724, 45.0, 74.2138);
        let affine = matrix.to_affine();
        // Not the double 633.5724: the operand went through `float` first.
        assert_eq!(affine.matrix()[3], 633.572_387_695_312_5);
        assert_ne!(affine.matrix()[3], 633.5724f64);
    }

    #[test]
    fn concatenating_onto_the_identity_is_the_matrix_itself() {
        let mut ctm = Matrix::identity();
        let placement = Matrix::new(515.0, 0.0, 0.0, 633.5724, 45.0, 74.2138);
        ctm.concatenate(&placement);
        assert_eq!(ctm, placement);
    }

    #[test]
    fn the_dpi_scale_is_a_float_division() {
        // 792 pt at 300 DPI is 3300 in real arithmetic and 3299 in PDFBox's.
        let scale = 300f32 / 72f32;
        let pixels = f64::from(792f32 * scale).floor() as u32;
        assert_eq!(pixels, 3299);
        assert_eq!((792.0f64 * (300.0f64 / 72.0f64)).floor() as u32, 3300);
    }
}
