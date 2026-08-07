// SPDX-License-Identifier: AGPL-3.0-or-later

//! The slice of SFNT container parsing the music fonts need: the table directory, `head`, and
//! enough of `cmap` to turn a SMuFL codepoint into a glyph index.
//!
//! This is deliberately not a general font library. Audiveris asks its music fonts exactly one
//! question — "what is the outline box of the glyph for this codepoint at this size" — and this
//! parses only what that question needs.

/// A failure to make sense of a font file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontError {
    /// The file is too short for a structure its own offsets promise.
    Truncated(&'static str),
    /// The file is not an SFNT, or not one with CFF outlines.
    NotOpenType,
    /// A required table is absent.
    MissingTable(&'static str),
    /// No `cmap` subtable this code can read.
    UnsupportedCmap,
    /// A CFF structure violated the specification.
    Cff(&'static str),
    /// A charstring did something the Type 2 interpreter cannot represent.
    Charstring(&'static str),
    /// A multi-digit time number, whose multi-glyph advances are not yet swept or graded.
    UnsupportedNumber,
    /// The glyph's box is set by a curve interior, a case the swept oracle does not grade.
    ///
    /// Java's scaler quantizes outline points to 26.6 and solves for extrema in that space, which
    /// is not the same as solving in font units and scaling afterwards. Every clef box lands on an
    /// on-curve point, so the sweep cannot tell the two apart; the first shape that needs it has
    /// to be measured rather than assumed.
    UngradedOutline,
}

impl core::fmt::Display for FontError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Truncated(what) => write!(formatter, "font is truncated reading {what}"),
            Self::NotOpenType => formatter.write_str("not an OpenType font with CFF outlines"),
            Self::MissingTable(tag) => write!(formatter, "font has no {tag} table"),
            Self::UnsupportedCmap => formatter.write_str("font has no readable cmap subtable"),
            Self::Cff(what) => write!(formatter, "malformed CFF: {what}"),
            Self::Charstring(what) => write!(formatter, "malformed charstring: {what}"),
            Self::UnsupportedNumber => {
                formatter.write_str("multi-digit time numbers need ungraded glyph advances")
            }
            Self::UngradedOutline => {
                formatter.write_str("glyph box is set by a curve interior, which is not graded")
            }
        }
    }
}

impl std::error::Error for FontError {}

pub(crate) fn u8_at(data: &[u8], offset: usize, what: &'static str) -> Result<u8, FontError> {
    data.get(offset).copied().ok_or(FontError::Truncated(what))
}

pub(crate) fn u16_at(data: &[u8], offset: usize, what: &'static str) -> Result<u16, FontError> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or(FontError::Truncated(what))?;
    Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
}

pub(crate) fn u32_at(data: &[u8], offset: usize, what: &'static str) -> Result<u32, FontError> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or(FontError::Truncated(what))?;
    Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

/// A parsed SFNT table directory, holding borrowed table slices.
#[derive(Debug, Clone)]
pub struct Sfnt<'a> {
    data: &'a [u8],
    tables: Vec<([u8; 4], usize, usize)>,
}

impl<'a> Sfnt<'a> {
    /// Parses the table directory of an OpenType/CFF (`OTTO`) font.
    ///
    /// TrueType-outline fonts are rejected rather than half-supported: `glyf` outlines are
    /// quadratic and would need a different extrema computation, and none of the shipped music
    /// fonts use them.
    pub fn parse(data: &'a [u8]) -> Result<Self, FontError> {
        let tag = data.get(0..4).ok_or(FontError::Truncated("sfnt tag"))?;
        if tag != b"OTTO" {
            return Err(FontError::NotOpenType);
        }
        let count = usize::from(u16_at(data, 4, "table count")?);
        let mut tables = Vec::with_capacity(count);
        for index in 0..count {
            let record = 12 + 16 * index;
            let tag = data
                .get(record..record + 4)
                .ok_or(FontError::Truncated("table record"))?;
            let offset = u32_at(data, record + 8, "table offset")? as usize;
            let length = u32_at(data, record + 12, "table length")? as usize;
            tables.push(([tag[0], tag[1], tag[2], tag[3]], offset, length));
        }
        Ok(Self { data, tables })
    }

    /// Returns a table's bytes by its four-character tag.
    pub fn table(&self, tag: &[u8; 4]) -> Option<&'a [u8]> {
        self.tables
            .iter()
            .find(|(candidate, _, _)| candidate == tag)
            .and_then(|&(_, offset, length)| self.data.get(offset..offset + length))
    }

    /// `head.unitsPerEm`, the denominator every outline coordinate is expressed against.
    pub fn units_per_em(&self) -> Result<u16, FontError> {
        let head = self.table(b"head").ok_or(FontError::MissingTable("head"))?;
        u16_at(head, 18, "unitsPerEm")
    }

    /// Maps a Unicode codepoint to a glyph index.
    ///
    /// Only `cmap` format 4 is read, and only from a Unicode-BMP subtable. That is sufficient and
    /// checked: SMuFL assigns music symbols to the Basic Multilingual Plane's Private Use Area, so
    /// every codepoint Audiveris asks for is by construction in range. A font that needed format
    /// 12 for these would report [`FontError::UnsupportedCmap`] rather than silently miss glyphs.
    pub fn glyph_index(&self, codepoint: u32) -> Result<Option<u16>, FontError> {
        let cmap = self.table(b"cmap").ok_or(FontError::MissingTable("cmap"))?;
        let Ok(codepoint) = u16::try_from(codepoint) else {
            return Err(FontError::UnsupportedCmap);
        };
        let count = usize::from(u16_at(cmap, 2, "cmap subtable count")?);
        let mut chosen = None;
        for index in 0..count {
            let record = 4 + 8 * index;
            let platform = u16_at(cmap, record, "cmap platform")?;
            let encoding = u16_at(cmap, record + 2, "cmap encoding")?;
            let offset = u32_at(cmap, record + 4, "cmap subtable offset")? as usize;
            let format = u16_at(cmap, offset, "cmap subtable format")?;
            // (3,1) is the Windows BMP subtable every OpenType font is required to carry, and
            // (0,3) is its Unicode-platform equivalent. Preferring them keeps the lookup on the
            // one subtable a font is most likely to have validated.
            let usable = format == 4 && ((platform == 3 && encoding == 1) || (platform == 0));
            if usable {
                chosen = Some(offset);
                if platform == 3 {
                    break;
                }
            }
        }
        let Some(subtable) = chosen else {
            return Err(FontError::UnsupportedCmap);
        };
        lookup_format4(&cmap[subtable..], codepoint)
    }
}

/// Binary-search a `cmap` format 4 subtable, following its segment arrays.
fn lookup_format4(table: &[u8], codepoint: u16) -> Result<Option<u16>, FontError> {
    let segment_count = usize::from(u16_at(table, 6, "segCountX2")?) / 2;
    let ends = 14;
    let starts = ends + segment_count * 2 + 2;
    let deltas = starts + segment_count * 2;
    let ranges = deltas + segment_count * 2;

    for segment in 0..segment_count {
        let end = u16_at(table, ends + segment * 2, "endCode")?;
        if codepoint > end {
            continue;
        }
        let start = u16_at(table, starts + segment * 2, "startCode")?;
        if codepoint < start {
            return Ok(None);
        }
        let range_offset = u16_at(table, ranges + segment * 2, "idRangeOffset")?;
        let delta = u16_at(table, deltas + segment * 2, "idDelta")?;
        if range_offset == 0 {
            // The spec's arithmetic is deliberately modulo 65536.
            let glyph = codepoint.wrapping_add(delta);
            return Ok((glyph != 0).then_some(glyph));
        }
        let address =
            ranges + segment * 2 + usize::from(range_offset) + usize::from(codepoint - start) * 2;
        let glyph = u16_at(table, address, "glyphIdArray")?;
        if glyph == 0 {
            return Ok(None);
        }
        return Ok(Some(glyph.wrapping_add(delta)));
    }
    Ok(None)
}
