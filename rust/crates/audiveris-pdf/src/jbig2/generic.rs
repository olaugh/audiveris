//! The generic region decoding procedure of 6.2.5.7.
//!
//! jbig2-imageio writes this a byte at a time, keeping two sliding windows over
//! the rows above and patching the adaptive pixels back in afterwards when they
//! are not in their nominal positions. That is faster and much harder to read,
//! and it is *equivalent*: the fast path's shifts place each context bit at the
//! same index this does, its edge cases shift in zeros exactly where a pixel
//! lookup here falls outside the bitmap, and the special case it needs for
//! adaptive pixels inside the byte being built disappears when every pixel is
//! stored as soon as it is decoded.
//!
//! So this is the straightforward form, with the adaptive pixels simply sitting
//! in the offset tables at the positions the fast path's masks reserve for
//! them: bits 4, 10, 11 and 15 for template 0, bit 3 for template 1, bit 2 for
//! template 2, bit 4 for template 3.

use super::arithmetic::{Arithmetic, Contexts};
use super::bitmap::Bitmap;

/// Where each context bit comes from, most significant bit first.
///
/// `None` marks an adaptive pixel, filled from the segment's AT values in
/// order. The nominal AT positions -- which reconstruct the standard's
/// figures -- are `(3, -1)`, `(-3, -1)`, `(2, -2)` and `(-2, -2)` for template
/// 0, and `(3, -1)`, `(2, -1)`, `(2, -1)` for templates 1 to 3.
type Offsets = &'static [Option<(i32, i32)>];

const TEMPLATE_0: Offsets = &[
    None, // A4, nominally (-2, -2)
    Some((-1, -2)),
    Some((0, -2)),
    Some((1, -2)),
    None, // A3, nominally (2, -2)
    None, // A2, nominally (-3, -1)
    Some((-2, -1)),
    Some((-1, -1)),
    Some((0, -1)),
    Some((1, -1)),
    Some((2, -1)),
    None, // A1, nominally (3, -1)
    Some((-4, 0)),
    Some((-3, 0)),
    Some((-2, 0)),
    Some((-1, 0)),
];

const TEMPLATE_1: Offsets = &[
    Some((-1, -2)),
    Some((0, -2)),
    Some((1, -2)),
    Some((2, -2)),
    Some((-2, -1)),
    Some((-1, -1)),
    Some((0, -1)),
    Some((1, -1)),
    Some((2, -1)),
    None, // A1, nominally (3, -1)
    Some((-3, 0)),
    Some((-2, 0)),
    Some((-1, 0)),
];

const TEMPLATE_2: Offsets = &[
    Some((-1, -2)),
    Some((0, -2)),
    Some((1, -2)),
    Some((-2, -1)),
    Some((-1, -1)),
    Some((0, -1)),
    Some((1, -1)),
    None, // A1, nominally (2, -1)
    Some((-2, 0)),
    Some((-1, 0)),
];

const TEMPLATE_3: Offsets = &[
    Some((-3, -1)),
    Some((-2, -1)),
    Some((-1, -1)),
    Some((0, -1)),
    Some((1, -1)),
    None, // A1, nominally (2, -1)
    Some((-4, 0)),
    Some((-3, 0)),
    Some((-2, 0)),
    Some((-1, 0)),
];

/// The context each template uses for the typical-prediction bit, from 6.2.5.7.
const TYPICAL_CONTEXT: [usize; 4] = [0x9b25, 0x0795, 0x00e5, 0x0195];

/// The AT slot order within template 0's offset table.
///
/// The table above lists the adaptive pixels in bit order -- A4, A3, A2, A1 --
/// while the segment header supplies them as A1 first, so the mapping is not
/// the identity and getting it wrong swaps two neighbourhood pixels in a way
/// that still decodes plausible-looking noise.
const TEMPLATE_0_AT_ORDER: [usize; 4] = [3, 2, 1, 0];

/// Decodes a generic region into a bitmap.
///
/// `at` holds the adaptive pixels in the order the segment declared them:
/// four for template 0 and one for the others. `typical_prediction` is TPGDON,
/// which lets a row be coded as "the same as the one above".
pub fn decode(
    decoder: &mut Arithmetic<'_>,
    contexts: &mut Contexts,
    width: usize,
    height: usize,
    template: u8,
    at: &[(i32, i32)],
    typical_prediction: bool,
) -> Bitmap {
    let template = template.min(3);
    let offsets: Offsets = match template {
        0 => TEMPLATE_0,
        1 => TEMPLATE_1,
        2 => TEMPLATE_2,
        _ => TEMPLATE_3,
    };
    // Resolve the adaptive slots once, so the inner loop is a plain lookup.
    let mut resolved: Vec<(i32, i32)> = Vec::with_capacity(offsets.len());
    let mut adaptive = 0usize;
    for offset in offsets {
        match offset {
            Some(fixed) => resolved.push(*fixed),
            None => {
                let slot = if template == 0 {
                    TEMPLATE_0_AT_ORDER[adaptive.min(3)]
                } else {
                    0
                };
                resolved.push(at.get(slot).copied().unwrap_or((0, 0)));
                adaptive += 1;
            }
        }
    }

    let mut bitmap = Bitmap::new(width, height);
    let mut typical = false;
    for y in 0..height {
        if typical_prediction {
            let bit = decoder.decode(contexts, TYPICAL_CONTEXT[usize::from(template)]);
            typical ^= bit == 1;
        }
        if typical {
            if y > 0 {
                let (above, current) = bitmap.bytes.split_at_mut(y * bitmap.stride);
                current[..bitmap.stride]
                    .copy_from_slice(&above[(y - 1) * bitmap.stride..y * bitmap.stride]);
            }
            continue;
        }
        for x in 0..width {
            let mut context = 0usize;
            for (dx, dy) in &resolved {
                let pixel = bitmap.pixel(x as i32 + dx, y as i32 + dy);
                context = context << 1 | usize::from(pixel);
            }
            let bit = decoder.decode(contexts, context);
            bitmap.set(x, y, bit);
        }
    }
    bitmap
}
