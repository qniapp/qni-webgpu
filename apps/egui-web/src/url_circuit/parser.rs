//! Minimal JSON parser scoped to the `{"cols": [[...], ...]}` shape.

// ─────────────────────────────────────────────────────────────────────
//  Minimal JSON parser scoped to our `{"cols": [[entry, ...], ...]}`
//  format. Avoids pulling in serde_json (~60 KB wasm). Each entry is
//  either the integer literal `1` (empty wire) or a JSON string
//  containing a gate token. Multi-byte UTF-8 chars inside strings
//  (•, ◦, †, ½, ⟩, …) round-trip verbatim because we accumulate the
//  raw bytes and re-`String::from_utf8` at the end.
// ─────────────────────────────────────────────────────────────────────

/// Parse a single `{"cols": [[...], ...]}` document and return the
/// columns as `Vec<Vec<Option<String>>>` (outer = columns, inner =
/// per-wire entries, `None` for the `1` empty marker).
pub(super) fn parse_cols(s: &str) -> Option<Vec<Vec<Option<String>>>> {
    let bytes = s.as_bytes();
    let mut p = Parser { s: bytes, i: 0 };
    p.skip_ws();
    p.expect(b'{')?;
    p.skip_ws();
    let key = p.parse_string()?;
    if key != "cols" {
        return None;
    }
    p.skip_ws();
    p.expect(b':')?;
    p.skip_ws();
    p.expect(b'[')?;
    let mut cols = Vec::new();
    p.skip_ws();
    if p.peek() != Some(b']') {
        loop {
            cols.push(p.parse_column()?);
            p.skip_ws();
            match p.peek() {
                Some(b',') => {
                    p.advance();
                    p.skip_ws();
                }
                Some(b']') => break,
                _ => return None,
            }
        }
    }
    p.expect(b']')?;
    p.skip_ws();
    p.expect(b'}')?;
    p.skip_ws();
    if p.peek().is_some() {
        return None;
    }
    Some(cols)
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
                    match self.peek()? {
                        b'"' => out.push(b'"'),
                        b'\\' => out.push(b'\\'),
                        b'n' => out.push(b'\n'),
                        b't' => out.push(b'\t'),
                        b'/' => out.push(b'/'),
                        _ => return None,
                    }
                    self.advance();
                }
                c => {
                    out.push(c);
                    self.advance();
                }
            }
        }
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
