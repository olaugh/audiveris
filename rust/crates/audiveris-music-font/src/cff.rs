// SPDX-License-Identifier: AGPL-3.0-or-later

//! CFF outlines: enough of the Compact Font Format and the Type 2 charstring language to walk a
//! glyph's contours and take their exact bounding box.
//!
//! The bounding box is the whole point, and it is the *outline* box rather than the control box:
//! a cubic Bezier reaches beyond its control points only at an interior extremum, so
//! [`Outline::bounds`] solves for those roots instead of taking the hull. Getting that wrong is
//! invisible on most glyphs and wrong by a fraction of a pixel on curved ones, which is exactly
//! the error `TextLayout.getBounds()` parity would not survive.

use crate::sfnt::{FontError, u8_at, u16_at, u32_at};

/// A CFF INDEX: a count, an offset array, and the bytes the offsets slice.
#[derive(Debug, Clone)]
struct Index<'a> {
    data: &'a [u8],
    offsets: Vec<usize>,
    base: usize,
}

impl<'a> Index<'a> {
    /// Parses an INDEX at `position`, returning it and the position just past its end.
    fn parse(data: &'a [u8], position: usize) -> Result<(Self, usize), FontError> {
        let count = usize::from(u16_at(data, position, "index count")?);
        if count == 0 {
            return Ok((
                Self {
                    data,
                    offsets: Vec::new(),
                    base: 0,
                },
                position + 2,
            ));
        }
        let offset_size = usize::from(u8_at(data, position + 2, "index offSize")?);
        if !(1..=4).contains(&offset_size) {
            return Err(FontError::Cff("index offSize out of range"));
        }
        let mut offsets = Vec::with_capacity(count + 1);
        let mut cursor = position + 3;
        for _ in 0..=count {
            let bytes = data
                .get(cursor..cursor + offset_size)
                .ok_or(FontError::Truncated("index offsets"))?;
            offsets.push(
                bytes
                    .iter()
                    .fold(0usize, |value, &byte| (value << 8) | usize::from(byte)),
            );
            cursor += offset_size;
        }
        // Offsets are one-based from the byte before the data area, per the CFF spec.
        let base = cursor - 1;
        let end = base + offsets[count];
        Ok((
            Self {
                data,
                offsets,
                base,
            },
            end,
        ))
    }

    fn len(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }

    fn get(&self, index: usize) -> Option<&'a [u8]> {
        let start = self.base + *self.offsets.get(index)?;
        let end = self.base + *self.offsets.get(index + 1)?;
        self.data.get(start..end)
    }
}

/// A CFF DICT, kept as operator -> operand list.
///
/// Operands are `f64` because DICTs mix integers with real numbers; every value this code reads
/// back out is an offset or size that is integral in practice.
#[derive(Debug, Clone, Default)]
struct Dict {
    entries: Vec<(u16, Vec<f64>)>,
}

impl Dict {
    fn parse(data: &[u8]) -> Result<Self, FontError> {
        let mut entries = Vec::new();
        let mut operands: Vec<f64> = Vec::new();
        let mut cursor = 0;
        while cursor < data.len() {
            let byte = data[cursor];
            match byte {
                // Operators. Two-byte operators are keyed as 0x0c00 | second byte.
                0..=21 => {
                    let operator = if byte == 12 {
                        cursor += 2;
                        0x0c00 | u16::from(*data.get(cursor - 1).unwrap_or(&0))
                    } else {
                        cursor += 1;
                        u16::from(byte)
                    };
                    entries.push((operator, std::mem::take(&mut operands)));
                }
                28 => {
                    operands.push(f64::from(u16_at(data, cursor + 1, "dict short")? as i16));
                    cursor += 3;
                }
                29 => {
                    operands.push(f64::from(u32_at(data, cursor + 1, "dict long")? as i32));
                    cursor += 5;
                }
                30 => {
                    let (value, next) = parse_real(data, cursor + 1)?;
                    operands.push(value);
                    cursor = next;
                }
                32..=246 => {
                    operands.push(f64::from(i16::from(byte) - 139));
                    cursor += 1;
                }
                247..=250 => {
                    let low = i16::from(u8_at(data, cursor + 1, "dict operand")?);
                    operands.push(f64::from((i16::from(byte) - 247) * 256 + low + 108));
                    cursor += 2;
                }
                251..=254 => {
                    let low = i16::from(u8_at(data, cursor + 1, "dict operand")?);
                    operands.push(f64::from(-(i16::from(byte) - 251) * 256 - low - 108));
                    cursor += 2;
                }
                _ => return Err(FontError::Cff("reserved DICT operand byte")),
            }
        }
        Ok(Self { entries })
    }

    fn get(&self, operator: u16) -> Option<&[f64]> {
        self.entries
            .iter()
            .find(|(candidate, _)| *candidate == operator)
            .map(|(_, operands)| operands.as_slice())
    }
}

/// Decodes a CFF nibble-packed real number, returning it and the position after its terminator.
fn parse_real(data: &[u8], start: usize) -> Result<(f64, usize), FontError> {
    let mut text = String::new();
    let mut cursor = start;
    loop {
        let byte = u8_at(data, cursor, "dict real")?;
        cursor += 1;
        for nibble in [byte >> 4, byte & 0x0f] {
            match nibble {
                0..=9 => text.push(char::from(b'0' + nibble)),
                0xa => text.push('.'),
                0xb => text.push('E'),
                0xc => text.push_str("E-"),
                0xe => text.push('-'),
                0xf => {
                    return text
                        .parse()
                        .map(|value| (value, cursor))
                        .map_err(|_| FontError::Cff("unparseable DICT real"));
                }
                _ => return Err(FontError::Cff("reserved DICT real nibble")),
            }
        }
    }
}

/// A parsed CFF table, ready to produce glyph outlines.
#[derive(Debug, Clone)]
pub struct Cff<'a> {
    charstrings: Index<'a>,
    global_subrs: Index<'a>,
    local_subrs: Option<Index<'a>>,
}

impl<'a> Cff<'a> {
    /// Parses a `CFF ` table.
    ///
    /// CID-keyed fonts are rejected rather than mishandled: they select a Private DICT per glyph
    /// through FDArray/FDSelect, so a single local-subr index would be the wrong one. None of the
    /// shipped music fonts are CID-keyed.
    pub fn parse(data: &'a [u8]) -> Result<Self, FontError> {
        let header_size = usize::from(u8_at(data, 2, "CFF hdrSize")?);
        let (_names, position) = Index::parse(data, header_size)?;
        let (tops, position) = Index::parse(data, position)?;
        let (_strings, position) = Index::parse(data, position)?;
        let (global_subrs, _) = Index::parse(data, position)?;

        let top = Dict::parse(tops.get(0).ok_or(FontError::Cff("no Top DICT"))?)?;
        if top.get(0x0c1e).is_some() {
            return Err(FontError::Cff("CID-keyed fonts are not supported"));
        }
        let charstrings_offset = top
            .get(17)
            .and_then(<[f64]>::first)
            .ok_or(FontError::Cff("Top DICT has no CharStrings offset"))?;
        let (charstrings, _) = Index::parse(data, *charstrings_offset as usize)?;

        // The Private DICT is `[size, offset]`, and its Subrs offset is relative to it.
        let local_subrs = match top.get(18) {
            Some([size, offset]) => {
                let start = *offset as usize;
                let end = start + *size as usize;
                let private = Dict::parse(
                    data.get(start..end)
                        .ok_or(FontError::Truncated("Private DICT"))?,
                )?;
                match private.get(19).and_then(<[f64]>::first) {
                    Some(subrs) => Some(Index::parse(data, start + *subrs as usize)?.0),
                    None => None,
                }
            }
            _ => None,
        };

        Ok(Self {
            charstrings,
            global_subrs,
            local_subrs,
        })
    }

    /// Number of glyphs the CharStrings INDEX holds.
    pub fn glyph_count(&self) -> usize {
        self.charstrings.len()
    }

    /// Interprets one glyph's charstring into an outline in font units.
    pub fn outline(&self, glyph: u16) -> Result<Outline, FontError> {
        let charstring = self
            .charstrings
            .get(usize::from(glyph))
            .ok_or(FontError::Cff("glyph index out of range"))?;
        let mut interpreter = Interpreter {
            cff: self,
            outline: Outline::default(),
            stack: Vec::with_capacity(48),
            x: 0.0,
            y: 0.0,
            stem_count: 0,
            width_parsed: false,
            depth: 0,
        };
        interpreter.run(charstring)?;
        Ok(interpreter.outline)
    }
}

/// Subroutine index bias, per the Type 2 charstring specification.
fn bias(count: usize) -> i32 {
    if count < 1240 {
        107
    } else if count < 33900 {
        1131
    } else {
        32768
    }
}

/// One glyph outline: a flat list of segments in font units.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Outline {
    /// Every segment, in charstring order.
    pub segments: Vec<Segment>,
}

/// A single outline segment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Segment {
    /// Start a new contour at this point.
    MoveTo(f64, f64),
    /// Straight line to this point.
    LineTo(f64, f64),
    /// Cubic Bezier with two control points, ending at the third.
    CurveTo(f64, f64, f64, f64, f64, f64),
}

/// An axis-aligned bounding box in font units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox {
    /// Smallest x reached by the outline.
    pub min_x: f64,
    /// Smallest y reached by the outline.
    pub min_y: f64,
    /// Largest x reached by the outline.
    pub max_x: f64,
    /// Largest y reached by the outline.
    pub max_y: f64,
}

impl Outline {
    /// The exact bounding box of the outline, including interior curve extrema.
    ///
    /// Returns `None` for an empty outline, which is a real case: SMuFL fonts carry blank glyphs,
    /// and a zero-size box is not the same answer as no box at all.
    #[must_use]
    pub fn bounds(&self) -> Option<BoundingBox> {
        let mut bounds: Option<BoundingBox> = None;
        let mut include = |x: f64, y: f64| {
            let box_ = bounds.get_or_insert(BoundingBox {
                min_x: x,
                min_y: y,
                max_x: x,
                max_y: y,
            });
            box_.min_x = box_.min_x.min(x);
            box_.min_y = box_.min_y.min(y);
            box_.max_x = box_.max_x.max(x);
            box_.max_y = box_.max_y.max(y);
        };

        let mut current = (0.0, 0.0);
        for segment in &self.segments {
            match *segment {
                Segment::MoveTo(x, y) | Segment::LineTo(x, y) => {
                    include(x, y);
                    current = (x, y);
                }
                Segment::CurveTo(x1, y1, x2, y2, x3, y3) => {
                    include(x3, y3);
                    for (p0, p1, p2, p3, horizontal) in [
                        (current.0, x1, x2, x3, true),
                        (current.1, y1, y2, y3, false),
                    ] {
                        for t in cubic_extrema(p0, p1, p2, p3) {
                            let value = cubic_at(p0, p1, p2, p3, t);
                            if horizontal {
                                include(value, current.1.min(y3));
                                include(value, current.1.max(y3));
                            } else {
                                include(current.0.min(x3), value);
                                include(current.0.max(x3), value);
                            }
                        }
                    }
                    current = (x3, y3);
                }
            }
        }
        bounds
    }
}

/// Parameters in `(0, 1)` where a cubic's derivative vanishes on one axis.
fn cubic_extrema(p0: f64, p1: f64, p2: f64, p3: f64) -> Vec<f64> {
    // B'(t)/3 = a t^2 + b t + c with the coefficients below.
    let a = -p0 + 3.0 * p1 - 3.0 * p2 + p3;
    let b = 2.0 * (p0 - 2.0 * p1 + p2);
    let c = p1 - p0;
    let mut roots = Vec::new();
    let mut push = |t: f64| {
        if t > 0.0 && t < 1.0 {
            roots.push(t);
        }
    };
    if a.abs() < f64::EPSILON {
        if b.abs() > f64::EPSILON {
            push(-c / b);
        }
    } else {
        let discriminant = b * b - 4.0 * a * c;
        if discriminant >= 0.0 {
            let root = discriminant.sqrt();
            push((-b + root) / (2.0 * a));
            push((-b - root) / (2.0 * a));
        }
    }
    roots
}

/// Evaluates a cubic Bezier on one axis.
fn cubic_at(p0: f64, p1: f64, p2: f64, p3: f64, t: f64) -> f64 {
    let s = 1.0 - t;
    s * s * s * p0 + 3.0 * s * s * t * p1 + 3.0 * s * t * t * p2 + t * t * t * p3
}

/// The Type 2 charstring machine.
struct Interpreter<'a, 'b> {
    cff: &'b Cff<'a>,
    outline: Outline,
    stack: Vec<f64>,
    x: f64,
    y: f64,
    stem_count: usize,
    width_parsed: bool,
    depth: u8,
}

impl Interpreter<'_, '_> {
    /// The first stack-clearing operator may carry a leading width delta.
    ///
    /// It is present exactly when the operand count is odd for operators taking pairs (and even
    /// for `endchar`/`*moveto` variants taking an odd count), so the test is on parity against the
    /// operator's own arity rather than on any marker in the data.
    fn take_width(&mut self, even_arity: bool) {
        if !self.width_parsed {
            self.width_parsed = true;
            if (self.stack.len() % 2 == 1) == even_arity {
                self.stack.remove(0);
            }
        }
    }

    fn move_to(&mut self, dx: f64, dy: f64) {
        self.x += dx;
        self.y += dy;
        self.outline.segments.push(Segment::MoveTo(self.x, self.y));
    }

    fn line_to(&mut self, dx: f64, dy: f64) {
        self.x += dx;
        self.y += dy;
        self.outline.segments.push(Segment::LineTo(self.x, self.y));
    }

    fn curve_to(&mut self, dx1: f64, dy1: f64, dx2: f64, dy2: f64, dx3: f64, dy3: f64) {
        let x1 = self.x + dx1;
        let y1 = self.y + dy1;
        let x2 = x1 + dx2;
        let y2 = y1 + dy2;
        self.x = x2 + dx3;
        self.y = y2 + dy3;
        self.outline
            .segments
            .push(Segment::CurveTo(x1, y1, x2, y2, self.x, self.y));
    }

    fn run(&mut self, code: &[u8]) -> Result<bool, FontError> {
        if self.depth > 10 {
            return Err(FontError::Charstring("subroutine recursion too deep"));
        }
        let mut cursor = 0;
        while cursor < code.len() {
            let byte = code[cursor];
            cursor += 1;
            match byte {
                // Operands.
                28 => {
                    self.stack
                        .push(f64::from(u16_at(code, cursor, "charstring short")? as i16));
                    cursor += 2;
                }
                32..=246 => self.stack.push(f64::from(i16::from(byte) - 139)),
                247..=250 => {
                    let low = i16::from(u8_at(code, cursor, "charstring operand")?);
                    self.stack
                        .push(f64::from((i16::from(byte) - 247) * 256 + low + 108));
                    cursor += 1;
                }
                251..=254 => {
                    let low = i16::from(u8_at(code, cursor, "charstring operand")?);
                    self.stack
                        .push(f64::from(-(i16::from(byte) - 251) * 256 - low - 108));
                    cursor += 1;
                }
                255 => {
                    // 16.16 fixed point.
                    let raw = u32_at(code, cursor, "charstring fixed")? as i32;
                    self.stack.push(f64::from(raw) / 65536.0);
                    cursor += 4;
                }

                // Hints. Their operands are discarded, but they must be counted, because
                // hintmask's operand bytes follow inline and their number depends on the count.
                1 | 3 | 18 | 23 => {
                    self.take_width(true);
                    self.stem_count += self.stack.len() / 2;
                    self.stack.clear();
                }
                19 | 20 => {
                    self.take_width(true);
                    self.stem_count += self.stack.len() / 2;
                    self.stack.clear();
                    cursor += self.stem_count.div_ceil(8);
                }

                21 => {
                    self.take_width(true);
                    let (dx, dy) = (self.stack[0], self.stack[1]);
                    self.move_to(dx, dy);
                    self.stack.clear();
                }
                22 => {
                    self.take_width(false);
                    let dx = self.stack[0];
                    self.move_to(dx, 0.0);
                    self.stack.clear();
                }
                4 => {
                    self.take_width(false);
                    let dy = self.stack[0];
                    self.move_to(0.0, dy);
                    self.stack.clear();
                }

                5 => {
                    for pair in self.stack.clone().chunks_exact(2) {
                        self.line_to(pair[0], pair[1]);
                    }
                    self.stack.clear();
                }
                6 | 7 => {
                    // Alternating h/v lines; the opcode picks which axis starts.
                    let mut horizontal = byte == 6;
                    for value in self.stack.clone() {
                        if horizontal {
                            self.line_to(value, 0.0);
                        } else {
                            self.line_to(0.0, value);
                        }
                        horizontal = !horizontal;
                    }
                    self.stack.clear();
                }
                8 => {
                    for group in self.stack.clone().chunks_exact(6) {
                        self.curve_to(group[0], group[1], group[2], group[3], group[4], group[5]);
                    }
                    self.stack.clear();
                }
                24 => {
                    // rcurveline: curves, then one closing line.
                    let values = self.stack.clone();
                    let curves = (values.len() - 2) / 6 * 6;
                    for group in values[..curves].chunks_exact(6) {
                        self.curve_to(group[0], group[1], group[2], group[3], group[4], group[5]);
                    }
                    self.line_to(values[curves], values[curves + 1]);
                    self.stack.clear();
                }
                25 => {
                    // rlinecurve: lines, then one closing curve.
                    let values = self.stack.clone();
                    let lines = (values.len() - 6) / 2 * 2;
                    for pair in values[..lines].chunks_exact(2) {
                        self.line_to(pair[0], pair[1]);
                    }
                    let tail = &values[lines..];
                    self.curve_to(tail[0], tail[1], tail[2], tail[3], tail[4], tail[5]);
                    self.stack.clear();
                }
                26 | 27 => {
                    // vvcurveto / hhcurveto, with an optional leading cross-axis delta.
                    let values = self.stack.clone();
                    let mut index = 0;
                    let mut cross = 0.0;
                    if values.len() % 4 == 1 {
                        cross = values[0];
                        index = 1;
                    }
                    while index + 3 < values.len() {
                        if byte == 26 {
                            self.curve_to(
                                cross,
                                values[index],
                                values[index + 1],
                                values[index + 2],
                                0.0,
                                values[index + 3],
                            );
                        } else {
                            self.curve_to(
                                values[index],
                                cross,
                                values[index + 1],
                                values[index + 2],
                                values[index + 3],
                                0.0,
                            );
                        }
                        cross = 0.0;
                        index += 4;
                    }
                    self.stack.clear();
                }
                30 | 31 => {
                    // vhcurveto / hvcurveto: alternating tangents, with an optional final
                    // cross-axis delta on the last curve.
                    let values = self.stack.clone();
                    let mut horizontal = byte == 31;
                    let mut index = 0;
                    while index + 3 < values.len() {
                        let last = index + 8 > values.len();
                        let extra = if last && values.len() - index == 5 {
                            values[index + 4]
                        } else {
                            0.0
                        };
                        if horizontal {
                            self.curve_to(
                                values[index],
                                0.0,
                                values[index + 1],
                                values[index + 2],
                                extra,
                                values[index + 3],
                            );
                        } else {
                            self.curve_to(
                                0.0,
                                values[index],
                                values[index + 1],
                                values[index + 2],
                                values[index + 3],
                                extra,
                            );
                        }
                        horizontal = !horizontal;
                        index += 4;
                    }
                    self.stack.clear();
                }

                10 | 29 => {
                    let subrs = if byte == 10 {
                        self.cff.local_subrs.as_ref()
                    } else {
                        Some(&self.cff.global_subrs)
                    };
                    let subrs = subrs.ok_or(FontError::Charstring("call to absent subrs"))?;
                    let raw = self
                        .stack
                        .pop()
                        .ok_or(FontError::Charstring("callsubr with empty stack"))?;
                    let index = raw as i32 + bias(subrs.len());
                    let code = usize::try_from(index)
                        .ok()
                        .and_then(|index| subrs.get(index))
                        .ok_or(FontError::Charstring("subr index out of range"))?;
                    self.depth += 1;
                    let done = self.run(code)?;
                    self.depth -= 1;
                    if done {
                        return Ok(true);
                    }
                }
                11 => return Ok(false),
                14 => {
                    self.take_width(true);
                    return Ok(true);
                }

                12 => {
                    let second = u8_at(code, cursor, "escape operator")?;
                    cursor += 1;
                    self.escape(second)?;
                }

                _ => return Err(FontError::Charstring("unsupported charstring operator")),
            }
        }
        Ok(false)
    }

    /// The two-byte (`12 x`) operators. Only the flex family moves the pen.
    fn escape(&mut self, operator: u8) -> Result<(), FontError> {
        let values = self.stack.clone();
        match operator {
            // flex: two curves, plus a flex depth that does not affect geometry.
            35 if values.len() >= 13 => {
                self.curve_to(
                    values[0], values[1], values[2], values[3], values[4], values[5],
                );
                self.curve_to(
                    values[6], values[7], values[8], values[9], values[10], values[11],
                );
            }
            // hflex: both curves horizontal, sharing one y displacement.
            34 if values.len() >= 7 => {
                let y = self.y;
                self.curve_to(values[0], 0.0, values[1], values[2], values[3], 0.0);
                self.curve_to(values[4], 0.0, values[5], y - self.y, values[6], 0.0);
            }
            // hflex1: as hflex, but the first curve carries its own y deltas.
            36 if values.len() >= 9 => {
                let y = self.y;
                self.curve_to(values[0], values[1], values[2], values[3], values[4], 0.0);
                self.curve_to(
                    values[5],
                    0.0,
                    values[6],
                    values[7],
                    values[8],
                    y - (self.y + values[7]),
                );
            }
            // flex1: the final point returns to the start on whichever axis moved less.
            37 if values.len() >= 11 => {
                let (start_x, start_y) = (self.x, self.y);
                let dx: f64 = values[..10].iter().step_by(2).sum();
                let dy: f64 = values[1..10].iter().step_by(2).sum();
                self.curve_to(
                    values[0], values[1], values[2], values[3], values[4], values[5],
                );
                if dx.abs() > dy.abs() {
                    let end_y = start_y - (self.y + values[7] + values[9]);
                    self.curve_to(
                        values[6], values[7], values[8], values[9], values[10], end_y,
                    );
                } else {
                    let end_x = start_x - (self.x + values[6] + values[8]);
                    self.curve_to(
                        values[6], values[7], values[8], values[9], end_x, values[10],
                    );
                }
            }
            // Arithmetic and storage operators cannot appear in a well-formed outline-only
            // charstring; treating them as a clean error beats silently producing a wrong box.
            _ => return Err(FontError::Charstring("unsupported escape operator")),
        }
        self.stack.clear();
        Ok(())
    }
}
