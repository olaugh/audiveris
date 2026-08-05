// SPDX-License-Identifier: AGPL-3.0-or-later

//! OpenJDK's `ScaledBlit`, which is where a scaled-up draw actually lands.
//!
//! PDFBox flips the interpolation hint to nearest-neighbour when an image is
//! scaled up (see [`crate::render`]), and that is the only thing that lets
//! `DrawImage.renderImageScale` succeed. What it reaches is not a general
//! nearest-neighbour resampler but this specific loop, and the difference is
//! entirely in how the source coordinate is rounded.
//!
//! # Why it is tiled
//!
//! The inner loop steps the source coordinate by a fixed increment per
//! destination pixel, in fixed point, so error accumulates linearly. OpenJDK's
//! own comment puts a bound on it: once the accumulated error reaches
//! `1 << shift`, or either increment, the sample is off by a whole source or
//! destination pixel. Rather than widen the arithmetic, it **re-derives the
//! source origin exactly at the start of every tile** and only accumulates
//! within one, with `findpow2tilesize` picking a tile small enough to keep the
//! drift under a fraction of a pixel.
//!
//! So the result is not "nearest neighbour" in any clean mathematical sense: it
//! is nearest-neighbour with a periodic exact resynchronisation whose period is
//! a power of two chosen from the scale. Reproducing the tiling is not optional,
//! and neither is the rounding in
//!
//! ```text
//! #define SRCLOC(id, fo, sf)   (ceil((((id) + 0.5) - (fo)) * (sf) - 0.5))
//! ```
//!
//! which is `ceil(x - 0.5)` -- a round-half-down, chosen for consistency with
//! how pixel coordinates are rounded elsewhere, and *not* the `floor(x + 0.5)`
//! that `Math.round` would give.

/// One 8-bit band, which is what `ByteGray` to `ByteGray` copies.
///
/// The registered primitive is `REGISTER_ANYBYTE_ISOSCALE_BLIT(ByteGray)`, an
/// *isomorphic* blit: source and destination formats are identical, so the body
/// is a byte copy and no colour conversion happens at all.
pub struct Surface<'a> {
    /// Row-major samples.
    pub samples: &'a [u8],
    /// Width in pixels, which is also the row stride.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
}

/// `findpow2tilesize`.
fn tile_size(mut shift: i32, sxinc: i32, syinc: i32) -> i32 {
    let smaller = if sxinc > syinc { syinc } else { sxinc };
    if smaller == 0 {
        // "Degenerate case will cause infinite loop in next loop..."
        return 1;
    }
    while (1i32 << shift) > smaller {
        shift -= 1;
    }
    if shift >= 16 {
        shift -= 8;
    } else {
        shift /= 2;
    }
    1i32 << shift
}

/// `TILESTART(id, io, ts)`: the start of the tile holding destination pixel
/// `id`, given the operation origin `io`.
const fn tile_start(id: i32, origin: i32, size: i32) -> i32 {
    origin + ((id - origin) & !(size - 1))
}

/// `SRCLOC(id, fo, sf)`.
///
/// `ceil(x - 0.5)`, deliberately, not `floor(x + 0.5)`: the two differ exactly
/// at a half, and OpenJDK's comment says the half goes to the lower integer for
/// consistency with the rest of its pixel rounding.
fn source_location(id: i32, origin: f64, scale: f64) -> f64 {
    ((f64::from(id) + 0.5 - origin) * scale - 0.5).ceil()
}

/// `refine`: the smallest destination coordinate mapping to a source
/// coordinate at or past `target`.
///
/// Searched rather than solved, because the forward and inverse mappings are
/// not exact inverses in floating point and the loop must agree with the
/// tiled iteration below rather than with algebra.
fn refine(int_origin: i32, origin: f64, size: i32, scale: f64, target: i32, increment: i32) -> i32 {
    let mut at = (origin + f64::from(target) / scale - 0.5).ceil() as i32;
    let mut was_negative = false;
    let mut was_positive = false;
    let increment = i64::from(increment);
    let target = i64::from(target);

    loop {
        let start = tile_start(at, int_origin, size);
        let mut location = source_location(start, origin, scale) as i64;
        if at > start {
            location += increment * i64::from(at - start);
        }
        if location >= target {
            if was_negative {
                break;
            }
            at -= 1;
            was_positive = true;
        } else {
            at += 1;
            if was_positive {
                break;
            }
            was_negative = true;
        }
    }
    at
}

/// The destination rectangle a scaled blit would write, in device pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    /// Inclusive left edge.
    pub x1: i32,
    /// Inclusive top edge.
    pub y1: i32,
    /// Exclusive right edge.
    pub x2: i32,
    /// Exclusive bottom edge.
    pub y2: i32,
}

/// `Java_sun_java2d_loops_ScaledBlit_Scale`, for a full-surface rectangular
/// clip.
///
/// `destination_rect` is the transformed source rectangle Java2D computed:
/// `(ddx1, ddy1, ddx2, ddy2)`. The source rectangle is the whole image, which
/// is what `tryCopyOrScale` passes.
///
/// The clip is taken to be the destination surface's own bounds. Every corpus
/// page clips to the page, so there is one rectangular span; a non-rectangular
/// clip would iterate several and is not reproduced here.
pub fn scale(
    destination: &mut [u8],
    destination_width: usize,
    destination_height: usize,
    source: &Surface<'_>,
    destination_rect: [f64; 4],
) {
    let [ddx1, ddy1, ddx2, ddy2] = destination_rect;
    let (sx2, sy2) = (source.width as i32, source.height as i32);

    // "OR together srcw and srch to get the MSB between the two, next shift it
    // up until it goes negative, count the shifts."
    let mut seed = sx2 | sy2;
    let mut shift = 0i32;
    if seed > 0 {
        loop {
            seed = seed.wrapping_shl(1);
            if seed <= 0 {
                break;
            }
            shift += 1;
        }
    }

    let y_underflow = (ddy2 - ddy1) < 1.0;
    let scale_y = (f64::from(sy2) / (ddy2 - ddy1)) * f64::from(1i32 << shift);
    let syinc = if y_underflow {
        sy2.wrapping_shl(shift as u32)
    } else {
        scale_y as i32
    };
    let x_underflow = (ddx2 - ddx1) < 1.0;
    let scale_x = (f64::from(sx2) / (ddx2 - ddx1)) * f64::from(1i32 << shift);
    let sxinc = if x_underflow {
        sx2.wrapping_shl(shift as u32)
    } else {
        scale_x as i32
    };
    let size = tile_size(shift, sxinc, syinc);

    let idx1 = (ddx1 - 0.5).ceil() as i32;
    let idy1 = (ddy1 - 0.5).ceil() as i32;

    // The source lock never clips here: the blit's source rectangle is the
    // whole image, so `srcInfo.bounds.x1 <= sx1` holds and the lower bound is
    // exactly idx1. The upper bound is always refined.
    let mut bounds = Bounds {
        x1: idx1,
        y1: idy1,
        x2: refine(
            idx1,
            ddx1,
            size,
            scale_x,
            sx2.wrapping_shl(shift as u32),
            sxinc,
        ),
        y2: refine(
            idy1,
            ddy1,
            size,
            scale_y,
            sy2.wrapping_shl(shift as u32),
            syinc,
        ),
    };
    if x_underflow {
        let x = source_location(idx1, ddx1, scale_x) / f64::from(1i32 << shift);
        bounds.x2 = bounds.x1 + i32::from(x >= 0.0 && x < f64::from(sx2));
    }
    if y_underflow {
        let y = source_location(idy1, ddy1, scale_y) / f64::from(1i32 << shift);
        bounds.y2 = bounds.y1 + i32::from(y >= 0.0 && y < f64::from(sy2));
    }

    // `SurfaceData_IntersectBounds` against the clip, which is the surface.
    bounds.x1 = bounds.x1.max(0);
    bounds.y1 = bounds.y1.max(0);
    bounds.x2 = bounds.x2.min(destination_width as i32);
    bounds.y2 = bounds.y2.min(destination_height as i32);
    if bounds.x2 <= bounds.x1 || bounds.y2 <= bounds.y1 {
        return;
    }

    if f64::from(size) >= (ddx2 - ddx1) && f64::from(size) >= (ddy2 - ddy1) {
        // "Do everything in one tile."
        let mut sxloc = source_location(idx1, ddx1, scale_x) as i32;
        let mut syloc = source_location(idy1, ddy1, scale_y) as i32;
        if bounds.y1 > idy1 {
            syloc = syloc.wrapping_add(syinc.wrapping_mul(bounds.y1 - idy1));
        }
        if bounds.x1 > idx1 {
            sxloc = sxloc.wrapping_add(sxinc.wrapping_mul(bounds.x1 - idx1));
        }
        blit(
            destination,
            destination_width,
            source,
            bounds,
            sxloc,
            syloc,
            sxinc,
            syinc,
            shift,
        );
        return;
    }

    // "Break each clip span into tiles for better accuracy."
    let mut tile_y = tile_start(bounds.y1, idy1, size);
    while tile_y < bounds.y2 {
        let y1 = tile_y.max(bounds.y1);
        let y2 = (tile_y + size).min(bounds.y2);
        let mut syloc = source_location(tile_y, ddy1, scale_y) as i32;
        if y1 > tile_y {
            syloc = syloc.wrapping_add(syinc.wrapping_mul(y1 - tile_y));
        }

        let mut tile_x = tile_start(bounds.x1, idx1, size);
        while tile_x < bounds.x2 {
            let x1 = tile_x.max(bounds.x1);
            let x2 = (tile_x + size).min(bounds.x2);
            let mut sxloc = source_location(tile_x, ddx1, scale_x) as i32;
            if x1 > tile_x {
                sxloc = sxloc.wrapping_add(sxinc.wrapping_mul(x1 - tile_x));
            }

            if x2 > x1 && y2 > y1 {
                blit(
                    destination,
                    destination_width,
                    source,
                    Bounds { x1, y1, x2, y2 },
                    sxloc,
                    syloc,
                    sxinc,
                    syinc,
                    shift,
                );
            }
            tile_x += size;
        }
        tile_y += size;
    }
}

/// `DEFINE_ISOSCALE_BLIT(AnyByte)` through `BlitLoopScaleWidthHeight`.
#[allow(
    clippy::too_many_arguments,
    reason = "these are the parameters of the C loop this reproduces"
)]
fn blit(
    destination: &mut [u8],
    destination_width: usize,
    source: &Surface<'_>,
    bounds: Bounds,
    sxloc: i32,
    mut syloc: i32,
    sxinc: i32,
    syinc: i32,
    shift: i32,
) {
    for row in bounds.y1..bounds.y2 {
        let source_row = (syloc >> shift) as usize;
        let mut at_x = sxloc;
        for column in bounds.x1..bounds.x2 {
            let source_column = (at_x >> shift) as usize;
            // The bounds refinement above is what keeps this inside the source;
            // the C loop has no check either, and a miss would mean the
            // refinement is wrong rather than that a clamp is needed.
            if let Some(sample) = source
                .samples
                .get(source_row * source.width + source_column)
                .filter(|_| source_row < source.height && source_column < source.width)
            {
                destination[row as usize * destination_width + column as usize] = *sample;
            }
            at_x = at_x.wrapping_add(sxinc);
        }
        syloc = syloc.wrapping_add(syinc);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_location_rounds_a_half_downwards() {
        // ceil(x - 0.5) sends an exact half to the lower integer, where
        // floor(x + 0.5) would send it to the higher one.
        assert_eq!(source_location(0, 0.0, 1.0), 0.0);
        // (1 + 0.5 - 0.0) * 1.0 - 0.5 == 1.0 exactly.
        assert_eq!(source_location(1, 0.0, 1.0), 1.0);
        // A case that lands on a half: (0 + 0.5 - 0.0) * 1.0 - 0.5 = 0.0.
        assert_eq!(source_location(0, 0.0, 2.0), 1.0);
    }

    #[test]
    fn a_tile_is_a_power_of_two_bounded_by_the_increment() {
        // With an increment smaller than `1 << shift`, the shift is reduced
        // until the tile no longer outruns it.
        assert_eq!(tile_size(16, 1 << 10, 1 << 12), 1 << 5);
        // Degenerate increments give a 1x1 tile rather than looping forever.
        assert_eq!(tile_size(16, 0, 0), 1);
    }

    #[test]
    fn tiles_start_at_multiples_of_the_size_from_the_origin() {
        // Boundaries sit at the origin plus multiples of the size, so 5, 21,
        // 37; a coordinate on one is its own tile start.
        assert_eq!(tile_start(5, 5, 16), 5);
        assert_eq!(tile_start(20, 5, 16), 5);
        assert_eq!(tile_start(21, 5, 16), 21);
        assert_eq!(tile_start(36, 5, 16), 21);
        assert_eq!(tile_start(37, 5, 16), 37);
    }

    #[test]
    fn an_exact_doubling_replicates_every_pixel() {
        let source = Surface {
            samples: &[10, 20, 30, 40],
            width: 2,
            height: 2,
        };
        let mut destination = vec![0u8; 16];
        scale(&mut destination, 4, 4, &source, [0.0, 0.0, 4.0, 4.0]);
        assert_eq!(
            destination,
            vec![
                10, 10, 20, 20, //
                10, 10, 20, 20, //
                30, 30, 40, 40, //
                30, 30, 40, 40,
            ]
        );
    }

    #[test]
    fn an_identity_scale_copies_the_source() {
        let source = Surface {
            samples: &[1, 2, 3, 4, 5, 6],
            width: 3,
            height: 2,
        };
        let mut destination = vec![0u8; 6];
        scale(&mut destination, 3, 2, &source, [0.0, 0.0, 3.0, 2.0]);
        assert_eq!(destination, vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn an_offset_placement_writes_only_where_it_lands() {
        let source = Surface {
            samples: &[7],
            width: 1,
            height: 1,
        };
        let mut destination = vec![0u8; 9];
        scale(&mut destination, 3, 3, &source, [1.0, 1.0, 2.0, 2.0]);
        assert_eq!(destination, vec![0, 0, 0, 0, 7, 0, 0, 0, 0]);
    }
}
