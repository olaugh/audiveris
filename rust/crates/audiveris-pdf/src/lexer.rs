//! The file syntax: one cursor over the bytes, producing [`Object`]s.
//!
//! This reproduces PDFBox's `BaseParser`, including several places where PDFBox
//! is more permissive than the specification. That permissiveness is not
//! optional -- real scans are written by real tools, and a parser that holds to
//! the specification rejects files Audiveris reads without complaint. The ones
//! that are deliberate rather than incidental are commented where they happen.
//!
//! The one structural decision worth stating: a stream's extent is settled here
//! rather than by the layer above, because `/Length` is frequently an indirect
//! reference and frequently wrong. The caller supplies a resolver if it has one
//! and the length is *validated* -- the bytes after it must be `endstream` --
//! with a scan for the keyword as the fallback. That is what PDFBox does, and
//! files in the wild need both halves.

use crate::error::{Error, Result};
use crate::object::{Dictionary, Name, Object, Reference, Stream};

/// The whitespace bytes, per the specification's table 1.
const fn is_space(byte: u8) -> bool {
    matches!(byte, b'\0' | b'\t' | b'\n' | 0x0c | b'\r' | b' ')
}

/// The delimiter bytes, per the specification's table 2.
const fn is_delimiter(byte: u8) -> bool {
    matches!(
        byte,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

/// A "regular" byte: anything that is neither whitespace nor a delimiter.
const fn is_regular(byte: u8) -> bool {
    !is_space(byte) && !is_delimiter(byte)
}

const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// How deep objects may nest before the parse is abandoned.
///
/// Nothing legitimate approaches this; it exists so a crafted file cannot
/// recurse the parser into a stack overflow, which is not something a
/// `Result` can catch.
const MAXIMUM_DEPTH: usize = 256;

/// Resolves an indirect `/Length` to a value.
///
/// Implemented by the document layer, which can look the object up; a caller
/// that has no cross-reference table yet passes `|_| None` and gets the scan.
pub trait LengthResolver {
    /// The integer `reference` holds, if it holds one.
    fn resolve_length(&mut self, reference: Reference) -> Option<i64>;
}

impl<F: FnMut(Reference) -> Option<i64>> LengthResolver for F {
    fn resolve_length(&mut self, reference: Reference) -> Option<i64> {
        self(reference)
    }
}

/// A resolver that never resolves, forcing the `endstream` scan.
pub struct NoLengths;

impl LengthResolver for NoLengths {
    fn resolve_length(&mut self, _reference: Reference) -> Option<i64> {
        None
    }
}

/// A cursor over PDF bytes.
#[derive(Debug, Clone)]
pub struct Lexer<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> Lexer<'a> {
    /// A cursor at the start of `data`.
    #[must_use]
    pub const fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    /// A cursor at `position` within `data`.
    #[must_use]
    pub const fn at(data: &'a [u8], position: usize) -> Self {
        Self { data, position }
    }

    /// The current offset.
    #[must_use]
    pub const fn position(&self) -> usize {
        self.position
    }

    /// Moves the cursor to `position`.
    pub const fn seek(&mut self, position: usize) {
        self.position = position;
    }

    /// The bytes being read.
    #[must_use]
    pub const fn data(&self) -> &'a [u8] {
        self.data
    }

    /// Whether the cursor is at or past the end.
    #[must_use]
    pub const fn at_end(&self) -> bool {
        self.position >= self.data.len()
    }

    /// The byte under the cursor, without advancing.
    #[must_use]
    pub fn peek(&self) -> Option<u8> {
        self.data.get(self.position).copied()
    }

    /// Advances past whitespace and comments.
    ///
    /// A comment runs from `%` to the end of the line, and comments may be
    /// separated by whitespace, so this alternates rather than doing each once.
    pub fn skip_space(&mut self) {
        loop {
            while self.peek().is_some_and(is_space) {
                self.position += 1;
            }
            if self.peek() != Some(b'%') {
                return;
            }
            while self
                .peek()
                .is_some_and(|byte| byte != b'\n' && byte != b'\r')
            {
                self.position += 1;
            }
        }
    }

    /// Whether the bytes at the cursor are `keyword`.
    #[must_use]
    pub fn looking_at(&self, keyword: &str) -> bool {
        self.data[self.position..].starts_with(keyword.as_bytes())
    }

    /// Consumes `keyword` if it is at the cursor.
    pub fn consume(&mut self, keyword: &str) -> bool {
        if self.looking_at(keyword) {
            self.position += keyword.len();
            return true;
        }
        false
    }

    /// Consumes `keyword`, or fails.
    ///
    /// # Errors
    ///
    /// [`Error::ExpectedKeyword`] if something else is there.
    pub fn expect(&mut self, keyword: &'static str) -> Result<()> {
        self.skip_space();
        if self.consume(keyword) {
            return Ok(());
        }
        Err(Error::ExpectedKeyword {
            expected: keyword,
            offset: self.position,
        })
    }

    /// Reads a run of regular bytes as a keyword.
    ///
    /// Returns an empty slice at a delimiter, which callers use to notice that
    /// the thing they expected is not there without consuming anything.
    pub fn keyword(&mut self) -> &'a [u8] {
        let start = self.position;
        while self.peek().is_some_and(is_regular) {
            self.position += 1;
        }
        &self.data[start..self.position]
    }

    /// Reads a decimal integer, skipping leading whitespace.
    ///
    /// # Errors
    ///
    /// [`Error::UnexpectedByte`] if no digits are there.
    pub fn integer(&mut self) -> Result<i64> {
        self.skip_space();
        let start = self.position;
        if matches!(self.peek(), Some(b'+' | b'-')) {
            self.position += 1;
        }
        let digits = self.position;
        while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
            self.position += 1;
        }
        if self.position == digits {
            return Err(Error::UnexpectedByte {
                found: self.peek().unwrap_or(0),
                offset: self.position,
            });
        }
        // A number long enough to overflow is corrupt rather than large; the
        // largest legitimate value here is a file offset.
        let text = std::str::from_utf8(&self.data[start..self.position])
            .map_err(|_| Error::BadCrossReference { offset: start })?;
        text.parse::<i64>()
            .map_err(|_| Error::BadCrossReference { offset: start })
    }

    /// Reads the next object, scanning for `endstream` on any stream it meets.
    ///
    /// # Errors
    ///
    /// Propagates whatever [`Lexer::object_with`] reports.
    pub fn object(&mut self) -> Result<Object> {
        self.object_with(&mut NoLengths)
    }

    /// Reads the next object, resolving indirect stream lengths through
    /// `lengths`.
    ///
    /// # Errors
    ///
    /// [`Error::Truncated`] at the end of input, [`Error::UnexpectedByte`] at a
    /// byte no object can start with, or [`Error::TooDeep`] on absurd nesting.
    pub fn object_with(&mut self, lengths: &mut impl LengthResolver) -> Result<Object> {
        self.object_at_depth(lengths, 0)
    }

    fn object_at_depth(
        &mut self,
        lengths: &mut impl LengthResolver,
        depth: usize,
    ) -> Result<Object> {
        if depth > MAXIMUM_DEPTH {
            return Err(Error::TooDeep);
        }
        self.skip_space();
        let Some(byte) = self.peek() else {
            return Err(Error::Truncated);
        };
        match byte {
            b'/' => Ok(Object::Name(self.name())),
            b'(' => Ok(Object::String(self.literal_string())),
            b'[' => self.array(lengths, depth),
            b'<' => {
                if self.data.get(self.position + 1) == Some(&b'<') {
                    self.dictionary_or_stream(lengths, depth)
                } else {
                    Ok(Object::String(self.hex_string()))
                }
            }
            b't' | b'f' | b'n' => {
                let start = self.position;
                match self.keyword() {
                    b"true" => Ok(Object::Boolean(true)),
                    b"false" => Ok(Object::Boolean(false)),
                    b"null" => Ok(Object::Null),
                    _ => Err(Error::UnexpectedByte {
                        found: byte,
                        offset: start,
                    }),
                }
            }
            b'0'..=b'9' | b'+' | b'-' | b'.' => Ok(self.number_or_reference()),
            _ => Err(Error::UnexpectedByte {
                found: byte,
                offset: self.position,
            }),
        }
    }

    /// Reads a name, resolving `#xx` escapes.
    ///
    /// A `#` not followed by two hexadecimal digits stays a literal `#`, which
    /// is what PDFBox does; treating it as an error loses files that merely
    /// have a `#` in a resource name.
    pub fn name(&mut self) -> Name {
        debug_assert_eq!(self.peek(), Some(b'/'));
        self.position += 1;
        let mut bytes = Vec::new();
        while let Some(byte) = self.peek() {
            if !is_regular(byte) {
                break;
            }
            self.position += 1;
            if byte != b'#' {
                bytes.push(byte);
                continue;
            }
            let high = self.data.get(self.position).copied().and_then(hex_value);
            let low = self
                .data
                .get(self.position + 1)
                .copied()
                .and_then(hex_value);
            match (high, low) {
                (Some(high), Some(low)) => {
                    bytes.push(high << 4 | low);
                    self.position += 2;
                }
                _ => bytes.push(b'#'),
            }
        }
        Name(bytes)
    }

    /// Reads a `(...)` string.
    ///
    /// End-of-line bytes inside the string are kept as they are rather than
    /// normalised to a newline. The specification asks for normalisation but
    /// PDFBox does not do it, and nothing this port reads is a string whose
    /// bytes it interprets.
    fn literal_string(&mut self) -> Vec<u8> {
        debug_assert_eq!(self.peek(), Some(b'('));
        self.position += 1;
        let mut bytes = Vec::new();
        let mut depth = 1usize;
        while let Some(byte) = self.peek() {
            self.position += 1;
            match byte {
                b'(' => {
                    depth += 1;
                    bytes.push(byte);
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    bytes.push(byte);
                }
                b'\\' => {
                    let Some(escaped) = self.peek() else { break };
                    self.position += 1;
                    match escaped {
                        b'n' => bytes.push(b'\n'),
                        b'r' => bytes.push(b'\r'),
                        b't' => bytes.push(b'\t'),
                        b'b' => bytes.push(0x08),
                        b'f' => bytes.push(0x0c),
                        b'\n' => {}
                        // A backslash before CRLF swallows both bytes.
                        b'\r' => {
                            if self.peek() == Some(b'\n') {
                                self.position += 1;
                            }
                        }
                        b'0'..=b'7' => {
                            let mut value = u32::from(escaped - b'0');
                            for _ in 0..2 {
                                match self.peek() {
                                    Some(digit @ b'0'..=b'7') => {
                                        value = value * 8 + u32::from(digit - b'0');
                                        self.position += 1;
                                    }
                                    _ => break,
                                }
                            }
                            bytes.push((value & 0xff) as u8);
                        }
                        other => bytes.push(other),
                    }
                }
                other => bytes.push(other),
            }
        }
        bytes
    }

    /// Reads a `<...>` string. An odd digit count pads with a trailing zero.
    fn hex_string(&mut self) -> Vec<u8> {
        debug_assert_eq!(self.peek(), Some(b'<'));
        self.position += 1;
        let mut bytes = Vec::new();
        let mut pending: Option<u8> = None;
        while let Some(byte) = self.peek() {
            self.position += 1;
            if byte == b'>' {
                break;
            }
            // Anything that is not a hexadecimal digit is dropped, whitespace
            // and stray punctuation alike, as PDFBox drops it.
            let Some(value) = hex_value(byte) else {
                continue;
            };
            match pending.take() {
                Some(high) => bytes.push(high << 4 | value),
                None => pending = Some(value),
            }
        }
        if let Some(high) = pending {
            bytes.push(high << 4);
        }
        bytes
    }

    fn array(&mut self, lengths: &mut impl LengthResolver, depth: usize) -> Result<Object> {
        debug_assert_eq!(self.peek(), Some(b'['));
        self.position += 1;
        let mut entries = Vec::new();
        loop {
            self.skip_space();
            match self.peek() {
                None => return Err(Error::Truncated),
                Some(b']') => {
                    self.position += 1;
                    return Ok(Object::Array(entries));
                }
                // A `>>` or `}` inside an array is a broken writer, not the end
                // of the array; PDFBox drops the byte and carries on rather
                // than losing every object after it.
                Some(b'>' | b'}' | b')') => {
                    self.position += 1;
                }
                Some(byte) => {
                    // A bare keyword -- a content-stream operator that a
                    // caller has handed here, or junk -- is discarded rather
                    // than ending the parse, as PDFBox discards it.
                    if is_regular(byte) && !byte.is_ascii_digit() && !b"+-.tfn".contains(&byte) {
                        self.keyword();
                        continue;
                    }
                    entries.push(self.object_at_depth(lengths, depth + 1)?);
                }
            }
        }
    }

    fn dictionary_or_stream(
        &mut self,
        lengths: &mut impl LengthResolver,
        depth: usize,
    ) -> Result<Object> {
        self.position += 2;
        let mut dictionary = Dictionary::new();
        loop {
            self.skip_space();
            match self.peek() {
                None => return Err(Error::Truncated),
                Some(b'>') => {
                    self.position += 1;
                    // A single `>` where `>>` belongs still ends the dictionary.
                    if self.peek() == Some(b'>') {
                        self.position += 1;
                    }
                    break;
                }
                Some(b'/') => {
                    let key = self.name();
                    let value = self.object_at_depth(lengths, depth + 1)?;
                    dictionary.insert(key, value);
                }
                // Junk between entries: skip a byte and try again, which is how
                // PDFBox survives a dictionary with a stray token in it.
                Some(_) => self.position += 1,
            }
        }
        let after_dictionary = self.position;
        self.skip_space();
        if !self.consume("stream") {
            self.position = after_dictionary;
            return Ok(Object::Dictionary(dictionary));
        }
        // The specification requires CRLF or LF here and forbids a bare CR.
        // PDFBox accepts the bare CR, and files rely on that.
        if self.peek() == Some(b'\r') {
            self.position += 1;
        }
        if self.peek() == Some(b'\n') {
            self.position += 1;
        }
        let data = self.stream_data(&dictionary, lengths);
        self.skip_space();
        self.consume("endstream");
        Ok(Object::Stream(Stream { dictionary, data }))
    }

    /// Extracts a stream's bytes, preferring a `/Length` that checks out.
    fn stream_data(
        &mut self,
        dictionary: &Dictionary,
        lengths: &mut impl LengthResolver,
    ) -> Vec<u8> {
        let start = self.position;
        let declared = match dictionary.get(&Name::new("Length")) {
            Some(Object::Integer(value)) => Some(*value),
            Some(Object::Reference(reference)) => lengths.resolve_length(*reference),
            _ => None,
        };
        if let Some(length) = declared
            && let Ok(length) = usize::try_from(length)
            && let Some(end) = start.checked_add(length)
            && end <= self.data.len()
        {
            // Validate: `endstream` has to be what follows, or the length is
            // one of the many that are simply wrong and the scan is better.
            let mut probe = Self::at(self.data, end);
            probe.skip_space();
            if probe.looking_at("endstream") {
                self.position = end;
                return self.data[start..end].to_vec();
            }
        }
        let end = find(&self.data[start..], b"endstream").map_or(self.data.len(), |at| start + at);
        self.position = end;
        // The end-of-line before `endstream` belongs to the syntax, not to the
        // data. Only one, and CRLF counts as one.
        let mut trimmed = end;
        if trimmed > start && self.data[trimmed - 1] == b'\n' {
            trimmed -= 1;
        }
        if trimmed > start && self.data[trimmed - 1] == b'\r' {
            trimmed -= 1;
        }
        self.data[start..trimmed].to_vec()
    }

    /// Reads a number, or `n g R` if that is what follows.
    ///
    /// The lookahead has to be undone when it fails: `1 2` inside an array is
    /// two integers, and only the trailing `R` makes it a reference.
    fn number_or_reference(&mut self) -> Object {
        let number = self.number();
        let Object::Integer(value) = number else {
            return number;
        };
        if value < 0 || u32::try_from(value).is_err() {
            return number;
        }
        let rewind = self.position;
        self.skip_space();
        let generation_start = self.position;
        if !self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
            self.position = rewind;
            return number;
        }
        let Object::Integer(generation) = self.number() else {
            self.position = rewind;
            return number;
        };
        let _ = generation_start;
        self.skip_space();
        // `R` must stand alone: `1 0 Rg` is an operator, not a reference.
        let is_reference = self.peek() == Some(b'R')
            && !self
                .data
                .get(self.position + 1)
                .copied()
                .is_some_and(is_regular);
        if !is_reference || u16::try_from(generation).is_err() {
            self.position = rewind;
            return number;
        }
        self.position += 1;
        Object::Reference(Reference {
            number: value as u32,
            generation: generation as u16,
        })
    }

    /// Reads a number, integer or real.
    ///
    /// PDF has no exponent notation, but PDFBox parses one through `Float`, so
    /// `e` and `E` are collected. A run that will not parse becomes zero rather
    /// than an error, matching `COSFloat`'s own fallback -- a malformed number
    /// in a resource dictionary should not lose the page.
    fn number(&mut self) -> Object {
        let start = self.position;
        while self.peek().is_some_and(|byte| {
            byte.is_ascii_digit() || matches!(byte, b'+' | b'-' | b'.' | b'e' | b'E')
        }) {
            self.position += 1;
        }
        let text = String::from_utf8_lossy(&self.data[start..self.position]);
        if text.contains(['.', 'e', 'E']) {
            return Object::Real(parse_real(&text));
        }
        text.parse::<i64>()
            .map_or_else(|_| Object::Real(parse_real(&text)), Object::Integer)
    }
}

/// Parses a real the lenient way, falling back to zero.
///
/// Doubles up on the leniency PDFBox needs: `--12` and `12-3` both appear in
/// files that PDFBox reads, and it recovers by keeping the leading sign and
/// truncating at the second one.
fn parse_real(text: &str) -> f64 {
    if let Ok(value) = text.parse::<f64>() {
        return value;
    }
    let negative = text.starts_with('-');
    let body: String = text
        .trim_start_matches(['+', '-'])
        .chars()
        .take_while(|character| character.is_ascii_digit() || *character == '.')
        .collect();
    let magnitude = body.parse::<f64>().unwrap_or(0.0);
    if negative { -magnitude } else { magnitude }
}

/// The first offset at which `needle` occurs in `haystack`.
#[must_use]
pub fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// The last offset at which `needle` occurs in `haystack`.
#[must_use]
pub fn rfind(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .rposition(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Object {
        Lexer::new(text.as_bytes()).object().expect("parses")
    }

    #[test]
    fn reads_the_basic_types() {
        assert_eq!(parse("true"), Object::Boolean(true));
        assert_eq!(parse("false"), Object::Boolean(false));
        assert_eq!(parse("null"), Object::Null);
        assert_eq!(parse("42"), Object::Integer(42));
        assert_eq!(parse("-3"), Object::Integer(-3));
        assert_eq!(parse("4."), Object::Real(4.0));
        assert_eq!(parse(".5"), Object::Real(0.5));
        assert_eq!(parse("/Name"), Object::Name(Name::new("Name")));
    }

    #[test]
    fn resolves_name_escapes_and_leaves_lone_hashes_alone() {
        assert_eq!(parse("/A#20B"), Object::Name(Name::new("A B")));
        assert_eq!(parse("/A#zz"), Object::Name(Name::new("A#zz")));
        assert_eq!(parse("/A#"), Object::Name(Name::new("A#")));
    }

    #[test]
    fn reads_strings_with_their_escapes() {
        assert_eq!(parse(r"(a\(b\)c)"), Object::String(b"a(b)c".to_vec()));
        assert_eq!(parse("(a(b)c)"), Object::String(b"a(b)c".to_vec()));
        assert_eq!(parse(r"(\101\10\1)"), Object::String(vec![b'A', 8, 1]));
        assert_eq!(parse("(a\\\nb)"), Object::String(b"ab".to_vec()));
        assert_eq!(parse("<414 2>"), Object::String(b"AB".to_vec()));
        assert_eq!(parse("<4>"), Object::String(vec![0x40]));
    }

    #[test]
    fn tells_a_reference_from_two_integers() {
        assert_eq!(
            parse("12 0 R"),
            Object::Reference(Reference {
                number: 12,
                generation: 0
            })
        );
        assert_eq!(
            parse("[1 2]"),
            Object::Array(vec![Object::Integer(1), Object::Integer(2)])
        );
        // `Rg` is a content-stream operator; the lookahead must not eat it.
        assert_eq!(
            parse("[1 0 Rg]"),
            Object::Array(vec![Object::Integer(1), Object::Integer(0)])
        );
    }

    #[test]
    fn reads_a_dictionary_and_a_stream() {
        let object = parse("<< /Type /Page /Count 3 >>");
        let dictionary = object.dictionary().expect("a dictionary");
        assert_eq!(dictionary[&Name::new("Count")], Object::Integer(3));

        let object = parse("<< /Length 5 >>\nstream\nHELLO\nendstream");
        let stream = object.stream().expect("a stream");
        assert_eq!(stream.data, b"HELLO");
    }

    #[test]
    fn falls_back_to_scanning_when_the_length_is_wrong() {
        // /Length 2 is a lie; the bytes after it are not `endstream`, so the
        // scan has to take over and recover the whole payload.
        let object = parse("<< /Length 2 >>\nstream\nHELLO\nendstream");
        assert_eq!(object.stream().expect("a stream").data, b"HELLO");
    }

    #[test]
    fn resolves_an_indirect_length_when_it_can() {
        let text = b"<< /Length 9 0 R >>\nstream\nHELLOendstream";
        let mut lexer = Lexer::new(text);
        let object = lexer
            .object_with(&mut |reference: Reference| (reference.number == 9).then_some(5))
            .expect("parses");
        assert_eq!(object.stream().expect("a stream").data, b"HELLO");
    }

    #[test]
    fn keeps_the_last_value_of_a_duplicated_key() {
        let object = parse("<< /A 1 /A 2 >>");
        let dictionary = object.dictionary().expect("a dictionary");
        assert_eq!(dictionary[&Name::new("A")], Object::Integer(2));
    }
}
