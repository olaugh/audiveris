//! Java2D's image transform, reproduced sample for sample.
//!
//! # Why this came before any PDF parsing
//!
//! Audiveris does not extract images from a PDF, it *renders* the page:
//! `ImageLoading.PdfboxLoader.getImage` calls PDFBox's
//! `renderImageWithDPI(page, 300, ImageType.GRAY)` under `ANTIALIAS_OFF` and
//! `INTERPOLATION_BICUBIC`. So a native ingest has to reproduce a rasterizer,
//! and every decoder it would need -- CCITT G4, JBIG2, Flate, and the JPEG
//! decoder already written -- is worth nothing without this piece.
//!
//! Measured on real IMSLP sources, every page is a single full-page *bilevel*
//! image resampled to 8-bit gray at a non-integer ratio (2315x2848 to 2479x3299,
//! 6109x7676 to 3106x3903, 4064x5732 to 2681x3767). All 256 gray levels appear
//! in the output and 2-4% of the pixels are interpolated rather than copied.
//! Those pixels are the anti-aliased edge of every staff line, and they feed
//! adaptive binarization, where a one-count difference flips a pixel and GRID
//! amplifies it. Hence: build this first, verify it, then parse PDFs.
//!
//! # What is reproduced
//!
//! OpenJDK's `TransformHelper.c` and the `DEFINE_TRANSFORMHELPER_BC` macro in
//! `LoopMacros.h`. The pieces that matter, none of which are guessable from the
//! phrase "bicubic interpolation":
//!
//! - The kernel is Mitchell-Netravali with `A = -0.5`, tabulated at 513 entries
//!   over `|x|` in `[0, 2]`, and the table's upper half is *not* evaluated from
//!   the polynomial: entries above 384 are derived so each group of four sums to
//!   exactly one.
//! - The arithmetic is fixed point, not floating point. Coefficients are scaled
//!   by 256, the accumulator starts at `1 << 15` as a rounding bias, and the
//!   result is `>> 16` and then saturated. OpenJDK also carries `double`
//!   variants, selected by `#define`; the shipping build uses the integer one.
//! - Source coordinates step in 32.32 fixed point, and both the neighbourhood
//!   gather and the interpolation subtract exactly half a pixel first.
//! - Out-of-range neighbours are clamped by branchless sign-bit arithmetic that
//!   duplicates the edge row or column, so a 4x4 window at a corner reads the
//!   corner pixel repeatedly.
//! - Destination pixels whose centre maps outside the source are not written at
//!   all. They keep whatever the destination already held, which is why a page
//!   render leaves a black margin rather than an extrapolated one.
//!
//! # Verification
//!
//! `tests/parity.rs` drives Java itself and requires equality: 7 geometries by 8
//! scales, including the three real IMSLP ratios, upscale and downscale,
//! identity, and 1x1. All 112 agree bit for bit.
//!
//! # Status against whole pages
//!
//! Settled. [`draw_bicubic_into`] is one of the three primitives a page render
//! reaches, and all 189 corpus pages now reproduce PDFBox's rendered raster bit
//! for bit. See [`crate::render`] for how the primitive is chosen and
//! [`crate::scaledblit`] for the other one.
//!
//! Reading PDFBox's transform rather than reconstructing it is what closed the
//! last gap here. Composed in closed form from the page box, the DPI and the
//! content stream's `cm`, the horizontal terms came out bit-identical but the
//! vertical ones did not -- `scale_y` differed in its last bits -- and that
//! alone put 924 samples wrong, confined to 13 destination rows. The cause is
//! now understood and ported: `java.awt.geom.AffineTransform` is a state
//! machine whose branches drop terms known to be zero, and `crate::affine`
//! reproduces it.
//!
//! Sheared placements matter more than they look. Surveying all 189 pages found
//! that **70 draws carry a shear** -- the scans were deskewed by baking a
//! rotation of up to 1.2e-2, about 0.7 degrees, into the content stream's `cm`.
//! An axis-aligned placement cannot express those, and twelve pages of testing
//! had missed the regime entirely.
//!
//! An earlier note here recorded that `-Dsun.java2d.trace=count` showed some
//! pages reaching `ScaledBlit(ByteGray, SrcNoEa, ByteGray)` instead of this
//! code, and guessed that `DrawImage` was dispatching on `sg.transformState`.
//! That guess was wrong, and the answer is in PDFBox rather than Java2D:
//! `PageDrawer.drawImage` flips the interpolation hint to nearest-neighbour for
//! a scaled-up image, which is the only thing that lets a `ScaledBlit` be
//! reached at all. [`crate::render`] carries the detail.
//!

/// Mitchell-Netravali coefficient for the shipping build's integer math.
///
/// `A` is `-0.5`, and the table is 513 entries: 0..256 covers `|x| < 1`,
/// 256..384 covers `1 <= |x| < 2`, and the rest is *derived* rather than
/// evaluated so that the four coefficients applied to any sample position sum
/// to exactly `256`. Reconstructing that tail from the polynomial gives visibly
/// different output.
#[must_use]
fn coefficients(a: f64) -> [i32; 513] {
    let mut table = [0i32; 513];
    for (index, slot) in table.iter_mut().enumerate().take(256) {
        // r(x) = (A + 2)|x|^3 - (A + 3)|x|^2 + 1, for 0 <= |x| < 1
        let x = index as f64 / 256.0;
        *slot = ((((a + 2.0) * x - (a + 3.0)) * x * x + 1.0) * 256.0) as i32;
    }
    for (index, slot) in table.iter_mut().enumerate().take(384).skip(256) {
        // r(x) = A|x|^3 - 5A|x|^2 + 8A|x| - 4A, for 1 <= |x| < 2
        let x = index as f64 / 256.0;
        *slot = ((((a * x - 5.0 * a) * x + 8.0 * a) * x - 4.0 * a) * 256.0) as i32;
    }
    table[384] = (COEFFICIENT_ONE - table[128] * 2) / 2;
    for index in 385..=512 {
        table[index] =
            COEFFICIENT_ONE - (table[512 - index] + table[index - 256] + table[768 - index]);
    }
    table
}

/// Unit coefficient in the fixed-point scale, OpenJDK's `BC_COEFF_ONE`.
const COEFFICIENT_ONE: i32 = 256;
/// Half a pixel in the 32.32 fixed-point coordinate space.
const ONE_HALF: i64 = 1i64 << 31;
/// Rounding bias the accumulator starts at, OpenJDK's `BC_V_HALF`.
const ACCUMULATOR_BIAS: i32 = 1 << 15;

/// A single-band 8-bit image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrayImage {
    pub width: usize,
    pub height: usize,
    pub samples: Vec<u8>,
}

/// Scales `source` into a `width` by `height` destination, as Java2D does.
///
/// `background` is what an unwritten destination pixel holds; Java2D leaves
/// those untouched, and PDFBox hands it a destination that starts black.
///
/// # Panics
///
/// Panics if `source.samples` is shorter than its declared geometry.
#[must_use]
pub fn scale_bicubic(
    source: &GrayImage,
    width: usize,
    height: usize,
    scale_x: f64,
    scale_y: f64,
    background: u8,
) -> GrayImage {
    assert!(
        source.samples.len() >= source.width * source.height,
        "source samples are shorter than its geometry"
    );
    let table = coefficients(-0.5);
    let (source_width, source_height) = (source.width as i32, source.height as i32);
    // Java2D's `xorig`: the inverse transform of the *centre* of the first
    // destination pixel, then a constant step per pixel and per row.
    let base_x = fixed(0.5 / scale_x);
    let base_y = fixed(0.5 / scale_y);
    let step_x = fixed(1.0 / scale_x);
    let step_y = fixed(1.0 / scale_y);

    let mut samples = vec![background; width * height];
    for down in 0..height {
        let row_y = base_y + down as i64 * step_y;
        for across in 0..width {
            let at_x = base_x + across as i64 * step_x;
            // `calculateEdges`: only pixels whose centre lands inside the
            // source are rendered; the rest keep the destination's own value.
            if whole(at_x) < 0
                || whole(at_x) >= source_width
                || whole(row_y) < 0
                || whole(row_y) >= source_height
            {
                continue;
            }
            let gathered = neighbourhood(source, at_x - ONE_HALF, row_y - ONE_HALF);
            samples[down * width + across] =
                interpolate(&table, &gathered, at_x - ONE_HALF, row_y - ONE_HALF);
        }
    }
    GrayImage {
        width,
        height,
        samples,
    }
}

/// Where a source image lands in the destination: a full affine placement.
///
/// The six terms are the *forward* transform, device from source, in the order
/// `AffineTransform.getMatrix` reports them, which is also what PDFBox hands
/// Java2D. Java2D itself receives the inverse; this type inverts internally so
/// a caller can pass what it read.
///
/// A scale and a translation are not enough. Across 189 real IMSLP pages, **70
/// carry a shear** of up to 1.2e-2 -- about 0.7 degrees -- because the scans
/// were deskewed by baking a small rotation into the content stream's `cm`
/// rather than re-rasterizing. An axis-aligned placement cannot express those at
/// all. OpenJDK's loop was general here; this port had simplified it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// `m00`: device x per source x.
    pub scale_x: f64,
    /// `m10`: device y per source x.
    pub shear_y: f64,
    /// `m01`: device x per source y.
    pub shear_x: f64,
    /// `m11`: device y per source y.
    pub scale_y: f64,
    /// `m02`: device x translation.
    pub left: f64,
    /// `m12`: device y translation.
    pub top: f64,
}

impl Placement {
    /// An axis-aligned placement, the common case.
    #[must_use]
    pub const fn axis_aligned(left: f64, top: f64, scale_x: f64, scale_y: f64) -> Self {
        Self {
            scale_x,
            shear_y: 0.0,
            shear_x: 0.0,
            scale_y,
            left,
            top,
        }
    }

    /// The inverse transform, source from device, as Java2D receives it.
    ///
    /// `None` for a singular placement, which draws nothing.
    #[must_use]
    fn inverse(self) -> Option<[f64; 6]> {
        let determinant = self
            .scale_x
            .mul_add(self.scale_y, -(self.shear_y * self.shear_x));
        if determinant == 0.0 || !determinant.is_finite() {
            return None;
        }
        Some([
            self.scale_y / determinant,
            -self.shear_y / determinant,
            -self.shear_x / determinant,
            self.scale_x / determinant,
            self.shear_x.mul_add(self.top, -(self.scale_y * self.left)) / determinant,
            self.shear_y.mul_add(self.left, -(self.scale_x * self.top)) / determinant,
        ])
    }
}

/// Draws `source` into a `width` by `height` destination at `placement`.
///
/// This is the general form; [`scale_bicubic`] is the special case of a
/// placement at the origin covering the whole destination.
///
/// # Panics
///
/// Panics if `source.samples` is shorter than its declared geometry.
#[must_use]
pub fn draw_bicubic(
    source: &GrayImage,
    width: usize,
    height: usize,
    placement: Placement,
    background: u8,
) -> GrayImage {
    let mut destination = GrayImage {
        width,
        height,
        samples: vec![background; width * height],
    };
    draw_bicubic_into(&mut destination, source, placement);
    destination
}

/// Draws `source` into an existing destination, writing only the pixels Java2D
/// writes.
///
/// A page can carry more than one draw, and each lands on whatever the previous
/// ones left. Pixels whose centre maps outside the source keep the destination's
/// current value, which is how a page render ends up with margins holding the
/// background rather than an extrapolated edge.
///
/// # Panics
///
/// Panics if either image's samples are shorter than its declared geometry.
pub fn draw_bicubic_into(destination: &mut GrayImage, source: &GrayImage, placement: Placement) {
    assert!(
        source.samples.len() >= source.width * source.height,
        "source samples are shorter than its geometry"
    );
    assert!(
        destination.samples.len() >= destination.width * destination.height,
        "destination samples are shorter than its geometry"
    );
    let (width, height) = (destination.width, destination.height);
    let samples = &mut destination.samples;
    let table = coefficients(-0.5);
    let (source_width, source_height) = (source.width as i32, source.height as i32);
    // Java2D's `xorig`: the inverse transform of the centre of the first
    // destination pixel, then a constant per-pixel step. Accumulating from a
    // base rather than inverting per pixel is what keeps the fixed-point
    // arithmetic identical.
    let Some(inverse) = placement.inverse() else {
        return;
    };
    // Java2D steps in both axes on both loops, because a sheared placement moves
    // the source y as the destination x advances. `xbase`/`ybase` are the
    // inverse transform of the first destination pixel's centre and everything
    // after is accumulation: inverting per pixel instead drifts in the low bits
    // of the fixed-point coordinate.
    let base_x = fixed(inverse[0].mul_add(0.5, inverse[2] * 0.5) + inverse[4]);
    let base_y = fixed(inverse[1].mul_add(0.5, inverse[3] * 0.5) + inverse[5]);
    let step_x_across = fixed(inverse[0]);
    let step_y_across = fixed(inverse[1]);
    let step_x_down = fixed(inverse[2]);
    let step_y_down = fixed(inverse[3]);

    for down in 0..height {
        let row_x = base_x + down as i64 * step_x_down;
        let row_y = base_y + down as i64 * step_y_down;
        for across in 0..width {
            let at_x = row_x + across as i64 * step_x_across;
            let at_y = row_y + across as i64 * step_y_across;
            if whole(at_x) < 0
                || whole(at_x) >= source_width
                || whole(at_y) < 0
                || whole(at_y) >= source_height
            {
                continue;
            }
            let gathered = neighbourhood(source, at_x - ONE_HALF, at_y - ONE_HALF);
            samples[down * width + across] =
                interpolate(&table, &gathered, at_x - ONE_HALF, at_y - ONE_HALF);
        }
    }
}

/// `DblToLong`: a coordinate in 32.32 fixed point.
#[must_use]
fn fixed(value: f64) -> i64 {
    (value * 65536.0 * 65536.0) as i64
}

/// `WholeOfLong`: the integer part of a 32.32 coordinate.
#[must_use]
const fn whole(value: i64) -> i32 {
    (value >> 32) as i32
}

/// `FractOfLong`: the fractional part, as OpenJDK's raw low word.
#[must_use]
const fn fraction(value: i64) -> i32 {
    value as i32
}

/// The 4x4 window, edge-clamped exactly as `DEFINE_TRANSFORMHELPER_BC` is.
///
/// The clamping is branchless sign-bit arithmetic in OpenJDK and is kept that
/// way here: the shape of the result at a border -- which row or column gets
/// duplicated, and how a negative whole part shifts the window -- is what has
/// to match, and rewriting it as `min`/`max` invites getting one of those wrong.
#[must_use]
fn neighbourhood(source: &GrayImage, at_x: i64, at_y: i64) -> [i32; 16] {
    neighbourhood_band(
        &source.samples,
        source.width,
        source.height,
        1,
        0,
        at_x,
        at_y,
    )
}

/// The same window, for one band of an interleaved source.
///
/// Split out rather than duplicated so the single-band path -- the one pinned
/// by 112 cases in `oracle/java2d-bicubic.txt` -- keeps running exactly this
/// code with `bands` at one.
#[must_use]
fn neighbourhood_band(
    samples: &[u8],
    width: usize,
    height: usize,
    bands: usize,
    band: usize,
    at_x: i64,
    at_y: i64,
) -> [i32; 16] {
    let (source_width, source_height) = (width as i32, height as i32);
    let (mut column, mut row) = (whole(at_x), whole(at_y));

    let column_before = (-column) >> 31;
    let mut column_after = (((column + 1 - source_width) as u32) >> 31) as i32;
    let mut column_far = (((column + 2 - source_width) as u32) >> 31) as i32;
    let negative = column >> 31;
    column -= negative;
    column_after += negative;
    column_far += column_after;

    let row_before = (-row) >> 31;
    let mut row_after = (row + 1 - source_height) >> 31 & 1;
    let row_far = (row + 2 - source_height) >> 31 & 1;
    let negative = row >> 31;
    row -= negative;
    row_after += negative;

    let rows = [
        row + row_before,
        row,
        row + row_after,
        row + row_after + row_far,
    ];
    let columns = [
        column + column_before,
        column,
        column + column_after,
        column + column_far,
    ];
    let mut window = [0i32; 16];
    for (slot, (down, across)) in window.iter_mut().zip(
        rows.iter()
            .flat_map(|r| columns.iter().map(move |c| (r, c))),
    ) {
        *slot = i32::from(samples[(down * source_width + across) as usize * bands + band]);
    }
    window
}

/// A three-band 8-bit image, samples interleaved as `PDIndexed.toRGBImage`
/// leaves them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbImage {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// `width * height * 3` samples, band-interleaved and row-major.
    pub samples: Vec<u8>,
}

/// Draws a three-band source into a gray destination, as Java2D does it.
///
/// The order matters and is not the obvious one. `renderImageXform` transforms
/// into an `IntArgbPre` intermediate and only then blits that to `ByteGray`, so
/// **each channel is interpolated in colour and the reduction to gray happens
/// afterwards** -- not the other way round. The reduction is OpenJDK's
/// fixed-point luma from `ByteGray.h`:
///
/// ```text
/// #define ComposeByteGrayFrom3ByteRgb(r, g, b) \
///     (ByteGrayDataType)(((77*(r)) + (150*(g)) + (29*(b)) + 128) / 256)
/// ```
///
/// which is also why a gray source survives the same round trip untouched: at
/// `r == g == b == v` it is `(256v + 128) / 256`, and that is `v` for every
/// byte.
///
/// # Panics
///
/// Panics if either image's samples are shorter than its declared geometry.
pub fn draw_bicubic_rgb_into(destination: &mut GrayImage, source: &RgbImage, placement: Placement) {
    assert!(
        source.samples.len() >= source.width * source.height * 3,
        "source samples are shorter than its geometry"
    );
    assert!(
        destination.samples.len() >= destination.width * destination.height,
        "destination samples are shorter than its geometry"
    );
    let (width, height) = (destination.width, destination.height);
    let table = coefficients(-0.5);
    let (source_width, source_height) = (source.width as i32, source.height as i32);
    let Some(inverse) = placement.inverse() else {
        return;
    };
    let base_x = fixed(inverse[0].mul_add(0.5, inverse[2] * 0.5) + inverse[4]);
    let base_y = fixed(inverse[1].mul_add(0.5, inverse[3] * 0.5) + inverse[5]);
    let step_x_across = fixed(inverse[0]);
    let step_y_across = fixed(inverse[1]);
    let step_x_down = fixed(inverse[2]);
    let step_y_down = fixed(inverse[3]);

    for down in 0..height {
        let row_x = base_x + down as i64 * step_x_down;
        let row_y = base_y + down as i64 * step_y_down;
        for across in 0..width {
            let at_x = row_x + across as i64 * step_x_across;
            let at_y = row_y + across as i64 * step_y_across;
            if whole(at_x) < 0
                || whole(at_x) >= source_width
                || whole(at_y) < 0
                || whole(at_y) >= source_height
            {
                continue;
            }
            let mut channels = [0i32; 3];
            for (band, channel) in channels.iter_mut().enumerate() {
                let gathered = neighbourhood_band(
                    &source.samples,
                    source.width,
                    source.height,
                    3,
                    band,
                    at_x - ONE_HALF,
                    at_y - ONE_HALF,
                );
                *channel = i32::from(interpolate(
                    &table,
                    &gathered,
                    at_x - ONE_HALF,
                    at_y - ONE_HALF,
                ));
            }
            destination.samples[down * width + across] =
                compose_gray(channels[0], channels[1], channels[2]);
        }
    }
}

/// `ComposeByteGrayFrom3ByteRgb`.
#[must_use]
const fn compose_gray(red: i32, green: i32, blue: i32) -> u8 {
    ((77 * red + 150 * green + 29 * blue + 128) / 256) as u8
}

/// `BicubicInterp` with `BICUBIC_USE_INT_MATH`.
#[must_use]
fn interpolate(table: &[i32; 513], window: &[i32; 16], at_x: i64, at_y: i64) -> u8 {
    let factor_x = ((fraction(at_x) as u32) >> 24) as i32;
    let factor_y = ((fraction(at_y) as u32) >> 24) as i32;
    let vertical = [factor_y + 256, factor_y, 256 - factor_y, 512 - factor_y];
    let horizontal = [factor_x + 256, factor_x, 256 - factor_x, 512 - factor_x];

    let mut accumulator = ACCUMULATOR_BIAS;
    for (index, sample) in window.iter().enumerate() {
        let factor =
            table[horizontal[index % 4] as usize].wrapping_mul(table[vertical[index / 4] as usize]);
        accumulator = accumulator.wrapping_add(sample.wrapping_mul(factor));
    }
    (accumulator >> 16).clamp(0, 255) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gray_reduction_leaves_a_neutral_pixel_alone() {
        // `(256v + 128) / 256` is `v` for every byte, which is why a gray
        // source survives the IntArgbPre round trip Java2D puts it through and
        // why 188 of the corpus's pages never needed this path at all.
        for value in 0..=255i32 {
            assert_eq!(compose_gray(value, value, value), value as u8);
        }
    }

    #[test]
    fn the_gray_reduction_weights_the_channels_as_openjdk_does() {
        // The weights are 77/150/29, but the division truncates, so a fully
        // saturated channel does not come back as its own weight: green is
        // (150 * 255 + 128) / 256 == 149, not 150.
        assert_eq!(compose_gray(255, 0, 0), 77);
        assert_eq!(compose_gray(0, 255, 0), 149);
        assert_eq!(compose_gray(0, 0, 255), 29);
        // The three weights sum to exactly 256, so white is exactly white.
        assert_eq!(compose_gray(255, 255, 255), 255);
    }
}
