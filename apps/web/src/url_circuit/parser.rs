//! Minimal JSON parser scoped to the `{"cols": [[...], ...], "title": "..."}` shape.

// ─────────────────────────────────────────────────────────────────────
//  Minimal JSON parser scoped to our `{"cols": [[entry, ...], ...]}`
//  format. Avoids pulling in serde_json (~60 KB wasm). Each entry is
//  either the integer literal `1` (empty wire) or a JSON string
//  containing a gate token. Multi-byte UTF-8 chars inside strings
//  (•, ◦, †, ½, ⟩, …) round-trip verbatim because we accumulate the
//  raw bytes and re-`String::from_utf8` at the end.
//
//  The root object may also carry qni's optional `"title"` string.
//  Keys may appear in any order (qni reads the payload with
//  `JSON.parse`); any other key rejects the document.
// ─────────────────────────────────────────────────────────────────────

/// A parsed circuit document: the raw columns plus qni's circuit title.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct CircuitDocument {
    /// Outer = columns, inner = per-wire entries, `None` for the `1`
    /// empty marker.
    pub(super) cols: Vec<Vec<Option<String>>>,
    /// Trimmed like qni's `(circuit.title || '').trim()`; empty when the
    /// payload has no title.
    pub(super) title: String,
}

/// Parse a single `{"cols": [[...], ...], "title": "..."}` document.
/// `cols` is required; `title` is optional.
pub(super) fn parse_circuit_document(s: &str) -> Option<CircuitDocument> {
    let bytes = s.as_bytes();
    let mut p = Parser { s: bytes, i: 0 };
    let mut cols = None;
    let mut title = None;
    p.skip_ws();
    p.expect(b'{')?;
    loop {
        p.skip_ws();
        // `JSON.parse` keeps the last value of a duplicated key.
        match p.parse_string()?.as_str() {
            "cols" => {
                p.skip_ws();
                p.expect(b':')?;
                p.skip_ws();
                cols = Some(p.parse_cols()?);
            }
            "title" => {
                p.skip_ws();
                p.expect(b':')?;
                p.skip_ws();
                title = Some(p.parse_string()?);
            }
            _ => return None,
        }
        p.skip_ws();
        match p.peek() {
            Some(b',') => p.advance(),
            Some(b'}') => break,
            _ => return None,
        }
    }
    p.expect(b'}')?;
    p.skip_ws();
    if p.peek().is_some() {
        return None;
    }
    Some(CircuitDocument {
        cols: cols?,
        title: title.as_deref().map(trim_js).unwrap_or_default().to_owned(),
    })
}

/// JavaScript's `String.prototype.trim`. Rust's `char::is_whitespace`
/// (Unicode `White_Space`) also matches NEL (U+0085), which JavaScript
/// keeps, and misses the byte order mark, which JavaScript trims.
fn trim_js(s: &str) -> &str {
    s.trim_matches(|c: char| (c.is_whitespace() && c != '\u{85}') || c == '\u{FEFF}')
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }
    fn advance(&mut self) {
        self.i += 1;
    }
    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.advance();
        }
    }
    fn expect(&mut self, c: u8) -> Option<()> {
        if self.peek() == Some(c) {
            self.advance();
            Some(())
        } else {
            None
        }
    }
    fn parse_string(&mut self) -> Option<String> {
        self.expect(b'"')?;
        let mut out: Vec<u8> = Vec::new();
        loop {
            match self.peek()? {
                b'"' => {
                    self.advance();
                    return String::from_utf8(out).ok();
                }
                b'\\' => {
                    self.advance();
                    let escape = self.peek()?;
                    self.advance();
                    let escaped = match escape {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => self.parse_unicode_escape()?,
                        _ => return None,
                    };
                    out.extend_from_slice(escaped.encode_utf8(&mut [0; 4]).as_bytes());
                }
                c => {
                    out.push(c);
                    self.advance();
                }
            }
        }
    }
    /// The code point of a `\uXXXX` escape whose `\u` was already consumed.
    /// A high surrogate must be followed by a `\uXXXX` low surrogate.
    fn parse_unicode_escape(&mut self) -> Option<char> {
        let unit = self.parse_hex4()?;
        if !(0xD800..0xDC00).contains(&unit) {
            return char::from_u32(unit);
        }
        self.expect(b'\\')?;
        self.expect(b'u')?;
        let low = self.parse_hex4()?;
        if !(0xDC00..0xE000).contains(&low) {
            return None;
        }
        char::from_u32(0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00))
    }
    fn parse_hex4(&mut self) -> Option<u32> {
        let digits = self.s.get(self.i..self.i + 4)?;
        if !digits.iter().all(u8::is_ascii_hexdigit) {
            return None;
        }
        self.i += 4;
        u32::from_str_radix(std::str::from_utf8(digits).ok()?, 16).ok()
    }
    fn parse_cols(&mut self) -> Option<Vec<Vec<Option<String>>>> {
        self.expect(b'[')?;
        let mut cols = Vec::new();
        self.skip_ws();
        if self.peek() != Some(b']') {
            loop {
                cols.push(self.parse_column()?);
                self.skip_ws();
                match self.peek() {
                    Some(b',') => {
                        self.advance();
                        self.skip_ws();
                    }
                    Some(b']') => break,
                    _ => return None,
                }
            }
        }
        self.expect(b']')?;
        Some(cols)
    }
    fn parse_column(&mut self) -> Option<Vec<Option<String>>> {
        self.expect(b'[')?;
        let mut entries = Vec::new();
        self.skip_ws();
        if self.peek() != Some(b']') {
            loop {
                self.skip_ws();
                entries.push(self.parse_entry()?);
                self.skip_ws();
                match self.peek() {
                    Some(b',') => self.advance(),
                    Some(b']') => break,
                    _ => return None,
                }
            }
        }
        self.expect(b']')?;
        Some(entries)
    }
    fn parse_entry(&mut self) -> Option<Option<String>> {
        match self.peek()? {
            b'"' => Some(Some(self.parse_string()?)),
            b'1' => {
                self.advance();
                Some(None)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn title(json: &str) -> Option<String> {
        parse_circuit_document(json).map(|document| document.title)
    }

    #[test]
    fn title_after_cols_is_accepted() {
        assert_eq!(
            title(r#"{"cols":[["|0>"]],"title":"Superdense Coding"}"#),
            Some("Superdense Coding".to_owned())
        );
    }

    #[test]
    fn title_before_cols_is_accepted() {
        assert_eq!(
            parse_circuit_document(r#"{ "title" : "Bell", "cols" : [["H"]] }"#),
            Some(CircuitDocument {
                cols: vec![vec![Some("H".to_owned())]],
                title: "Bell".to_owned(),
            })
        );
    }

    #[test]
    fn missing_title_is_empty() {
        assert_eq!(title(r#"{"cols":[]}"#), Some(String::new()));
    }

    #[test]
    fn title_is_trimmed_like_javascript() {
        assert_eq!(
            title("{\"cols\":[],\"title\":\" \\t\u{3000}\u{FEFF}Bell\\n \"}"),
            Some("Bell".to_owned())
        );
    }

    #[test]
    fn title_keeps_next_line_like_javascript() {
        assert_eq!(
            title("{\"cols\":[],\"title\":\"\u{85}Bell\"}"),
            Some("\u{85}Bell".to_owned())
        );
    }

    #[test]
    fn title_decodes_json_escapes() {
        assert_eq!(
            title(r#"{"cols":[],"title":"\"q\"\\\/\u00e9\ud83d\ude00"}"#),
            Some("\"q\"\\/é😀".to_owned())
        );
    }

    #[test]
    fn unpaired_high_surrogate_is_rejected() {
        assert_eq!(title(r#"{"cols":[],"title":"\ud83d"}"#), None);
    }

    #[test]
    fn duplicated_key_keeps_last_value_like_json_parse() {
        assert_eq!(
            title(r#"{"title":"a","cols":[],"title":"b"}"#),
            Some("b".to_owned())
        );
    }

    #[test]
    fn non_string_title_is_rejected() {
        assert_eq!(title(r#"{"cols":[],"title":1}"#), None);
    }

    #[test]
    fn unknown_key_is_rejected() {
        assert_eq!(title(r#"{"cols":[],"mode":"gpu"}"#), None);
    }

    #[test]
    fn title_without_cols_is_rejected() {
        assert_eq!(title(r#"{"title":"Bell"}"#), None);
    }

    #[test]
    fn trailing_comma_is_rejected() {
        assert_eq!(title(r#"{"cols":[],}"#), None);
    }
}
