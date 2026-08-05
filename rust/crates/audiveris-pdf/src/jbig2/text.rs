//! Symbol dictionaries and text regions, arithmetically coded.
//!
//! These two carry every JBIG2 image in the corpus: 95 of 189 pages, each one a
//! symbol dictionary followed by a single immediate text region that covers the
//! whole page. Their flag words are 0 and 16, which is to say arithmetic
//! coding, no refinement, no aggregation, one-row strips, and a top-left
//! reference corner.
//!
//! Huffman-coded dictionaries and refinement are *not* implemented. That is a
//! deliberate line rather than an oversight: each is a substantial decoder of
//! its own, neither appears in the corpus, and a wrong implementation of either
//! would produce a page that looks like music rather than an error. When a file
//! needs one, [`crate::Error::UnsupportedFilter`] says so by name.

use super::arithmetic::{Arithmetic, Contexts, identifier, integer};
use super::bitmap::{Bitmap, Combine, blit};
use super::generic;
use super::reader::Reader;
use crate::error::{Error, Result};

/// The context banks a symbol dictionary and its text region share.
///
/// They are separate banks per procedure, and they persist across every symbol
/// in a dictionary -- the generic-region bank in particular is *not* reset
/// between symbols, which is what makes the second symbol of a dictionary
/// cheaper to code than the first. Resetting it decodes the first symbol
/// correctly and then diverges.
struct Banks {
    generic: Contexts,
    delta_height: Contexts,
    delta_width: Contexts,
    export: Contexts,
}

impl Banks {
    fn new() -> Self {
        Self {
            generic: Contexts::new(1 << 16),
            delta_height: Contexts::new(512),
            delta_width: Contexts::new(512),
            export: Contexts::new(512),
        }
    }
}

/// Decodes a symbol dictionary segment into the symbols it exports.
///
/// `imported` are the symbols this dictionary inherits from the dictionaries it
/// refers to; the corpus never uses any, but the export flags are indexed
/// across imports and new symbols together, so the list has to be threaded
/// through even when it is empty.
///
/// # Errors
///
/// [`Error::UnsupportedFilter`] for a Huffman-coded or refinement-aggregate
/// dictionary.
pub fn symbol_dictionary(data: &[u8], imported: &[Bitmap]) -> Result<Vec<Bitmap>> {
    let mut reader = Reader::new(data);
    // 7.4.3.1.1: three reserved bits, then the flags, most significant first.
    reader.bits(3);
    let _refinement_template = reader.bit();
    let template = reader.bits(2) as u8;
    let _context_retained = reader.bit();
    let _context_used = reader.bit();
    // Bits 7 to 2: the four Huffman table selectors, unused without /K.
    reader.bits(6);
    let refinement_aggregate = reader.bit() == 1;
    let huffman = reader.bit() == 1;

    if huffman || refinement_aggregate {
        return Err(Error::UnsupportedFilter {
            name: if huffman {
                "JBIG2Decode (Huffman symbol dictionary)".to_owned()
            } else {
                "JBIG2Decode (refinement/aggregate symbol dictionary)".to_owned()
            },
        });
    }

    let at = read_at(&mut reader, if template == 0 { 4 } else { 1 });
    let exported_count = reader.bits(32) as usize;
    let new_count = reader.bits(32) as usize;
    // A dictionary claiming more symbols than its own bytes could ever code is
    // damaged; refusing is better than allocating on its word.
    if new_count > data.len() * 8 || exported_count > new_count + imported.len() {
        return Err(Error::CorruptStream { filter: "JBIG2" });
    }

    let mut decoder = Arithmetic::new(reader.rest());
    let mut banks = Banks::new();
    let mut new_symbols: Vec<Bitmap> = Vec::with_capacity(new_count);

    // 6.5.5: symbols arrive grouped into height classes, each class coded as a
    // delta from the last and holding symbols of increasing width.
    let mut height = 0i64;
    while new_symbols.len() < new_count {
        let Some(delta_height) = integer(&mut decoder, &mut banks.delta_height) else {
            break;
        };
        height += delta_height;
        let mut width = 0i64;
        loop {
            // Out of band ends the height class.
            let Some(delta_width) = integer(&mut decoder, &mut banks.delta_width) else {
                break;
            };
            if new_symbols.len() >= new_count {
                break;
            }
            width += delta_width;
            if width <= 0
                || height <= 0
                || width > i64::from(u16::MAX)
                || height > i64::from(u16::MAX)
            {
                return Err(Error::CorruptStream { filter: "JBIG2" });
            }
            new_symbols.push(generic::decode(
                &mut decoder,
                &mut banks.generic,
                width as usize,
                height as usize,
                template,
                &at,
                false,
            ));
        }
    }

    // 6.5.10: alternating runs of "not exported" and "exported", counted over
    // the imported symbols and the new ones together. A zero-length run is
    // legal and simply flips the sense, so the loop is bounded by a step count
    // rather than by progress.
    let total = imported.len() + new_symbols.len();
    let mut flags = vec![0u8; total];
    let mut at_index = 0usize;
    let mut current = 0u8;
    for _ in 0..=total {
        if at_index >= total {
            break;
        }
        let Some(run) = integer(&mut decoder, &mut banks.export) else {
            break;
        };
        let Ok(run) = usize::try_from(run) else { break };
        let end = at_index.saturating_add(run).min(total);
        flags[at_index..end].fill(current);
        current ^= 1;
        at_index = end;
    }

    let mut exported = Vec::with_capacity(exported_count);
    for (index, flag) in flags.iter().enumerate() {
        if *flag != 1 {
            continue;
        }
        if index < imported.len() {
            exported.push(imported[index].clone());
        } else if let Some(symbol) = new_symbols.get(index - imported.len()) {
            exported.push(symbol.clone());
        }
    }
    Ok(exported)
}

/// A text region's placement on the page.
pub struct Region {
    /// The region's own bitmap.
    pub bitmap: Bitmap,
    /// Where its left edge sits on the page.
    pub x: i32,
    /// Where its top edge sits on the page.
    pub y: i32,
    /// How it combines with what is already there.
    pub operator: Combine,
}

/// Decodes a text region segment.
///
/// # Errors
///
/// [`Error::UnsupportedFilter`] for a Huffman-coded region, and
/// [`Error::CorruptStream`] for a region whose declared geometry is impossible.
pub fn text_region(data: &[u8], symbols: &[Bitmap]) -> Result<Region> {
    let mut reader = Reader::new(data);
    // 7.4.1: the region segment information field.
    let width = reader.bits(32) as usize;
    let height = reader.bits(32) as usize;
    let x = reader.bits(32) as i32;
    let y = reader.bits(32) as i32;
    reader.bits(5);
    let operator = Combine::from_code(reader.bits(3) as u8);

    // 7.4.4.1.1: the text region segment flags.
    let refinement_template = reader.bit();
    let mut offset = reader.bits(5) as i32;
    if offset > 0x0f {
        offset -= 0x20;
    }
    let default_pixel = reader.bit();
    let combine_symbols = Combine::from_code(reader.bits(2) as u8);
    let transposed = reader.bit() == 1;
    let corner = reader.bits(2) as u8;
    let log_strips = reader.bits(2);
    let strips = 1i64 << log_strips;
    let refine = reader.bit() == 1;
    let huffman = reader.bit() == 1;

    if huffman {
        return Err(Error::UnsupportedFilter {
            name: "JBIG2Decode (Huffman text region)".to_owned(),
        });
    }
    if refine && refinement_template == 0 {
        // The refinement AT pixels are present but nothing reads them yet.
        reader.bits(32);
    }
    let instances = reader.bits(32) as i64;
    // The same sanity bound jbig2-imageio applies: no more than one symbol
    // instance per pixel.
    let instances = instances.min((width as i64).saturating_mul(height as i64));

    if width == 0 || height == 0 || width > 1 << 20 || height > 1 << 20 {
        return Err(Error::CorruptStream { filter: "JBIG2" });
    }
    if refine {
        return Err(Error::UnsupportedFilter {
            name: "JBIG2Decode (refined text region)".to_owned(),
        });
    }

    // The identifier's code length is computed in floating point by
    // jbig2-imageio, and reproduced that way here: `ceil(log2(n))` through
    // `Math.log` is not always the integer answer, and the encoder and decoder
    // only agree if both make the same mistake.
    let code_length = ((symbols.len() as f64).ln() / std::f64::consts::LN_2).ceil();
    let code_length = if code_length.is_finite() && code_length > 0.0 {
        code_length as u32
    } else {
        0
    };

    let mut bitmap = Bitmap::new(width, height);
    if default_pixel != 0 {
        bitmap.fill(0xff);
    }

    let mut decoder = Arithmetic::new(reader.rest());
    let mut delta_t = Contexts::new(512);
    let mut first_s = Contexts::new(512);
    let mut delta_s = Contexts::new(512);
    let mut current_t = Contexts::new(512);
    let mut identifiers = Contexts::new(1 << code_length.min(16));

    // 6.4.5: instances arrive in strips, each strip a row band of the region.
    let mut strip_t = -integer(&mut decoder, &mut delta_t).unwrap_or(0) * strips;
    let mut first = 0i64;
    let mut placed = 0i64;
    while placed < instances {
        let Some(delta) = integer(&mut decoder, &mut delta_t) else {
            break;
        };
        strip_t += delta * strips;
        let mut current_s;
        let Some(delta_first) = integer(&mut decoder, &mut first_s) else {
            break;
        };
        first += delta_first;
        current_s = first;
        loop {
            let t = strip_t
                + if strips == 1 {
                    0
                } else {
                    integer(&mut decoder, &mut current_t).unwrap_or(0)
                };
            let id = identifier(&mut decoder, &mut identifiers, code_length);
            let Some(symbol) = symbols.get(id) else {
                return Err(Error::CorruptStream { filter: "JBIG2" });
            };
            current_s = place(
                symbol,
                &mut bitmap,
                current_s,
                t,
                transposed,
                corner,
                combine_symbols,
            );
            placed += 1;

            let Some(step) = integer(&mut decoder, &mut delta_s) else {
                break;
            };
            if placed >= instances {
                break;
            }
            current_s += step + i64::from(offset);
        }
    }

    Ok(Region {
        bitmap,
        x,
        y,
        operator,
    })
}

/// Draws one symbol instance and returns the advanced coordinate.
///
/// The reference corner decides both which corner of the symbol `(s, t)` names
/// and when the along-strip coordinate advances -- before the draw for a right
/// corner, after it for a left one -- so the two halves cannot be separated.
fn place(
    symbol: &Bitmap,
    region: &mut Bitmap,
    current_s: i64,
    t: i64,
    transposed: bool,
    corner: u8,
    operator: Combine,
) -> i64 {
    let mut current_s = current_s;
    if !transposed && (corner == 2 || corner == 3) {
        current_s += symbol.width as i64 - 1;
    } else if transposed && (corner == 0 || corner == 2) {
        current_s += symbol.height as i64 - 1;
    }

    let (mut s, mut t) = (current_s, t);
    if transposed {
        std::mem::swap(&mut s, &mut t);
    }
    match corner {
        0 => t -= symbol.height as i64 - 1,
        2 => {
            t -= symbol.height as i64 - 1;
            s -= symbol.width as i64 - 1;
        }
        3 => s -= symbol.width as i64 - 1,
        _ => {}
    }
    blit(symbol, region, s as i32, t as i32, operator);

    if !transposed && (corner == 0 || corner == 1) {
        current_s += symbol.width as i64 - 1;
    }
    if transposed && (corner == 1 || corner == 3) {
        current_s += symbol.height as i64 - 1;
    }
    current_s
}

/// Reads `count` adaptive-pixel pairs, each a pair of signed bytes.
fn read_at(reader: &mut Reader<'_>, count: usize) -> Vec<(i32, i32)> {
    (0..count)
        .map(|_| {
            let x = reader.bits(8) as u8 as i8;
            let y = reader.bits(8) as u8 as i8;
            (i32::from(x), i32::from(y))
        })
        .collect()
}
