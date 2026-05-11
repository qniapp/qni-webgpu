//! Serialise the current circuit into a Quirk-style URL hash fragment.
//!
//! qni and Quirk both encode a circuit as a JSON object of the form
//! `{"cols":[...]}` — but they differ in how that JSON gets into the
//! browser's URL bar:
//!
//! * **qni**: writes the JSON to the URL path with
//!   `encodeURIComponent`, producing a `%7B%22cols%22%3A...` blob that
//!   is impossible to read at a glance.
//! * **Quirk**: writes the raw JSON to the *hash fragment* under a
//!   `circuit=` key (so the hash can fit multiple `key=value` params
//!   joined with `&`), only falling back to percent-encoding when the
//!   JSON itself contains `%` or `&`. Browsers tolerate `{ } [ ] : ,`
//!   in `location.hash` and display them literally.
//!
//! We keep Quirk's "raw JSON in the hash" idea but drop the `circuit=`
//! prefix — this project never plans to layer other key-value params
//! into the hash, so the bare `#{"cols":[...]}` form is shorter and
//! parses unambiguously. (Importing a Quirk URL needs the `circuit=`
//! prefix stripped first; doable but not a workflow we ship today.)
//!
//! Token vocabulary (must match qni's `Serialized*Type` constants):
//!   H X Y Z S T  — text-book single-qubit gates
//!   S† T†        — daggers; literal Unicode dagger
//!   X^½          — √X (rnot)
//!   Rx Ry Rz P   — rotations and phase (angles TBD)
//!   Swap         — both wires of a swap each carry "Swap"
//!   •            — control (U+2022)
//!   ◦            — anti-control (U+25E6)
//!   |0> |1>      — write gates (literal ASCII as in qni)
//!   Measure      — measurement gate
//!   Bloch        — Bloch sphere display
//!   …            — spacer ellipsis
//!   QFT<n>       — n-qubit QFT (span suffix)
//!   QFT†<n>      — n-qubit inverse QFT
//!
//! Multi-qubit gates (CNOT, swap, controls) are split per wire across
//! the same column; the wire's array index is its qubit number. The
//! resizable-span gates (QFT / QFT†) emit their token only at the top
//! wire of the span; the lower wires within the span are left as `1`
//! (the empty marker) — matching qni, where the QFT element owns just
//! the top dropzone.

use crate::app::PlacedGate;
use crate::constants::GATE_SIZE;
use crate::gates::GateKind;
use crate::layout::nearest_slot_index;

/// JSON `circuit=` payload for an empty circuit. Quirk uses the same
/// literal (`{"cols":[]}`) when the circuit is cleared.
pub(crate) const EMPTY_CIRCUIT_JSON: &str = r#"{"cols":[]}"#;

/// Serialise the circuit to qni's `{"cols":[...]}` JSON shape. Returns
/// `EMPTY_CIRCUIT_JSON` if there are no gates.
///
/// Each entry of `cols` is one column; each column is an array indexed
/// by wire (qubit) number, with `1` for an empty wire and the gate's
/// token otherwise. Trailing `1`s in a column are stripped to match
/// Quirk / qni's compact JSON; a column with no gates at all (which
/// shouldn't happen post-`compact_empty_steps`) becomes `[1]`.
pub(crate) fn circuit_to_json(
    placed_gates: &[PlacedGate],
    slot_centers: &[f32],
    qubit_count: usize,
) -> String {
    if placed_gates.is_empty() || slot_centers.is_empty() {
        return EMPTY_CIRCUIT_JSON.to_string();
    }
    // Bucket gates by slot index (column). After `compact_empty_steps`
    // the occupied indices are dense from 0..N-1, but be defensive in
    // case this is ever called pre-compaction.
    let mut max_slot: i64 = -1;
    let mut buckets: Vec<Vec<&PlacedGate>> = Vec::new();
    for gate in placed_gates {
        let center_x = gate.pos.x + GATE_SIZE / 2.0;
        let Some((slot_index, _)) = nearest_slot_index(center_x, slot_centers) else {
            continue;
        };
        max_slot = max_slot.max(slot_index as i64);
        if buckets.len() <= slot_index {
            buckets.resize(slot_index + 1, Vec::new());
        }
        buckets[slot_index].push(gate);
    }
    if max_slot < 0 {
        return EMPTY_CIRCUIT_JSON.to_string();
    }

    let mut cols: Vec<String> = Vec::with_capacity(buckets.len());
    for bucket in &buckets {
        // Build the wire-indexed token vector for this column. Empty
        // wires are the `1` literal; gates emit their token.
        let mut entries: Vec<String> = (0..qubit_count).map(|_| "1".to_string()).collect();
        for gate in bucket {
            let Some(token) = gate_token(gate.kind, gate.span, gate.angle.as_deref()) else {
                continue;
            };
            if gate.wire < entries.len() {
                entries[gate.wire] = format!("\"{}\"", json_escape(&token));
            }
        }
        // Strip trailing empties so the JSON stays compact; a column
        // that turned out fully empty collapses to "[1]" (matches qni).
        while entries.last().is_some_and(|s| s == "1") {
            entries.pop();
        }
        if entries.is_empty() {
            entries.push("1".to_string());
        }
        cols.push(format!("[{}]", entries.join(",")));
    }
    format!(r#"{{"cols":[{}]}}"#, cols.join(","))
}

/// Map a gate kind + span + optional angle string to its URL token.
/// `None` for kinds that shouldn't appear in the serialised circuit
/// at all (currently no such kinds — every `GateKind` is
/// serialisable).
///
/// `angle` is `Some` only for parametric `GateKind::Phase` and is
/// emitted as `P(<angle>)` with `/` replaced by `_` so the literal
/// fits cleanly into URL fragments — mirroring qni's
/// `phase-gate-element.ts::toJson`. `None` (= "use the gate's
/// default") emits the bare `"P"` token, matching qni's editor
/// placeholder.
fn gate_token(kind: GateKind, span: usize, angle: Option<&str>) -> Option<String> {
    let s = match kind {
        GateKind::H => "H".to_string(),
        GateKind::X => "X".to_string(),
        GateKind::Y => "Y".to_string(),
        GateKind::Z => "Z".to_string(),
        GateKind::S => "S".to_string(),
        GateKind::SDagger => "S†".to_string(),
        GateKind::T => "T".to_string(),
        GateKind::TDagger => "T†".to_string(),
        GateKind::SqrtX => "X^½".to_string(),
        GateKind::Phase => format_parametric("P", angle),
        GateKind::Rx => format_parametric("Rx", angle),
        GateKind::Ry => format_parametric("Ry", angle),
        GateKind::Rz => format_parametric("Rz", angle),
        GateKind::Swap => "Swap".to_string(),
        GateKind::Control => "•".to_string(),
        GateKind::AntiControl => "◦".to_string(),
        GateKind::Measurement => "Measure".to_string(),
        GateKind::BlochDisplay => "Bloch".to_string(),
        GateKind::Write0 => "|0>".to_string(),
        GateKind::Write1 => "|1>".to_string(),
        GateKind::Spacer => "…".to_string(),
        GateKind::QftGate => format!("QFT{}", span.max(1)),
        GateKind::QftDaggerGate => format!("QFT†{}", span.max(1)),
    };
    Some(s)
}

/// Emit a parametric gate token: `Base(<angle>)` when `angle` is set,
/// otherwise bare `Base`. The angle string is URL-safed by replacing
/// the first `/` with `_` to match qni's
/// `phase-gate-element.ts::toJson` substitution.
fn format_parametric(base: &str, angle: Option<&str>) -> String {
    match angle {
        Some(a) if !a.is_empty() => format!("{}({})", base, a.replacen('/', "_", 1)),
        _ => base.to_string(),
    }
}

/// Escape a token for JSON string embedding. The token vocabulary only
/// contains the `"` character via the write gates (none of which use
/// it) and the backslash (likewise unused), so the only escape we
/// actually need is for `"` itself — included for safety.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(ch),
        }
    }
    out
}

/// Push the serialised circuit into `location.hash`, Quirk-style but
/// without the `circuit=` key prefix:
///   #{"cols":[...]}
///
/// JSON is written raw; falls back to `encodeURIComponent` only when
/// the payload contains `%` (which would otherwise be interpreted as
/// the start of a percent-escape sequence on read-back). `&` is no
/// longer a hazard since we don't split the hash into key=value pairs.
/// Uses `history.replaceState` so navigation back/forward doesn't fill
/// up with one entry per drop.
#[cfg(target_arch = "wasm32")]
pub(crate) fn write_circuit_to_url(json: &str) {
    use wasm_bindgen::JsValue;
    let Some(window) = web_sys::window() else {
        return;
    };
    let payload = if json.contains('%') {
        js_sys::encode_uri_component(json)
            .as_string()
            .unwrap_or_else(|| json.to_string())
    } else {
        json.to_string()
    };
    let hash = format!("#{payload}");
    // Compose a full URL so `replaceState` doesn't rewrite the path.
    let location = window.location();
    let base = match (location.origin(), location.pathname(), location.search()) {
        (Ok(o), Ok(p), Ok(s)) => format!("{o}{p}{s}"),
        _ => return,
    };
    let url = format!("{base}{hash}");
    let _ = window.history().ok().and_then(|h| {
        h.replace_state_with_url(&JsValue::NULL, "", Some(&url))
            .ok()
    });
}

/// Non-wasm stub so callers don't need to `cfg`-gate.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn write_circuit_to_url(_json: &str) {}

// ─────────────────────────────────────────────────────────────────────
//  URL → circuit decoder. Restores a circuit on page load so the URL
//  is shareable: copy the URL → paste into a new tab → same circuit.
//  Two URL shapes are accepted:
//
//    * Our native format: `#{"cols":[...]}` (or `#circuit={...}` for
//      Quirk URLs the user pasted in).
//    * qni's path format: `/{...}` with the JSON percent-encoded.
//
//  Tokens (`"H"`, `"•"`, `"QFT3"`, …) are mapped back to `GateKind`
//  via `token_to_gate`. The gate's screen position is reconstructed
//  from the *column index* (= slot index) and *wire index* (= qubit
//  number) via the same `LINE_LEFT_OFFSET + GATE_SIZE + SLOT_SPACING
//  * i` and `LINE_Y + LINE_GAP * w` formulas the layout uses, so no
//  canvas-width knowledge is required at startup.
// ─────────────────────────────────────────────────────────────────────

use crate::constants::{LINE_GAP, LINE_LEFT_OFFSET, LINE_Y, SLOT_SPACING};

/// Decode the URL and return the placed gates plus a recommended
/// `next_gate_id`. Empty `Vec` (with `next_gate_id = 1`) if no circuit
/// payload was found.
#[cfg(target_arch = "wasm32")]
pub(crate) fn parse_circuit_from_url() -> (Vec<PlacedGate>, u32) {
    let Some(window) = web_sys::window() else {
        return (Vec::new(), 1);
    };
    let location = window.location();
    // 1. Hash fragment (our native write path).
    if let Ok(hash) = location.hash() {
        if let Some(gates) = try_decode(hash.strip_prefix('#').unwrap_or(&hash)) {
            return assign_ids(gates);
        }
    }
    // 2. Last path segment (qni-compatible — JSON percent-encoded in
    //    the URL path, e.g. `/%7B%22cols%22:...%7D`).
    if let Ok(pathname) = location.pathname() {
        if let Some(last) = pathname.rsplit('/').next() {
            if let Some(gates) = try_decode(last) {
                return assign_ids(gates);
            }
        }
    }
    (Vec::new(), 1)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn parse_circuit_from_url() -> (Vec<PlacedGate>, u32) {
    (Vec::new(), 1)
}

/// Largest wire index seen across the gates' spans, plus one — i.e.
/// the qubit count needed to host them all. `MIN_QUBITS` floor is
/// applied by the caller (the app's clamp).
pub(crate) fn qubit_count_from_gates(gates: &[PlacedGate]) -> usize {
    gates
        .iter()
        .map(|g| g.wire + g.span.saturating_sub(1) + 1)
        .max()
        .unwrap_or(0)
}

/// Try to decode `payload` (a possibly-percent-encoded `{"cols":...}`
/// snippet) into a list of `PlacedGate`. Strips a `circuit=` prefix
/// if present so Quirk URLs paste cleanly. Returns `None` for any
/// payload that doesn't decode + parse + non-empty-resolve.
fn try_decode(payload: &str) -> Option<Vec<PlacedGate>> {
    if payload.is_empty() {
        return None;
    }
    let decoded = decode_percent(payload)?;
    let json = decoded
        .strip_prefix("circuit=")
        .unwrap_or(&decoded)
        .trim_start();
    if !json.starts_with('{') {
        return None;
    }
    let cols = parse_cols(json)?;
    let gates = build_gates(&cols);
    if gates.is_empty() {
        None
    } else {
        Some(gates)
    }
}

/// `decodeURIComponent` via js_sys on wasm; pure-Rust passthrough
/// otherwise (native builds never call this in practice).
#[cfg(target_arch = "wasm32")]
fn decode_percent(s: &str) -> Option<String> {
    js_sys::decode_uri_component(s).ok()?.as_string()
}

#[cfg(not(target_arch = "wasm32"))]
fn decode_percent(s: &str) -> Option<String> {
    Some(s.to_string())
}

/// Walk the parsed columns and place each non-empty entry as a
/// `PlacedGate`. Position is reconstructed from the column index
/// (slot) and wire index using the same formulas `layout_metrics`
/// uses — independent of canvas width so this works at startup
/// before any frame has rendered.
fn build_gates(cols: &[Vec<Option<String>>]) -> Vec<PlacedGate> {
    let slot_left = LINE_LEFT_OFFSET + crate::constants::GATE_SIZE;
    let mut gates = Vec::new();
    for (col_idx, col) in cols.iter().enumerate() {
        let slot_center_x = slot_left + SLOT_SPACING * col_idx as f32;
        for (wire_idx, entry) in col.iter().enumerate() {
            let Some(token) = entry.as_deref() else {
                continue;
            };
            let Some((kind, span, angle)) = token_to_gate(token) else {
                continue;
            };
            let line_y = LINE_Y + LINE_GAP * wire_idx as f32;
            gates.push(PlacedGate {
                id: 0, // assigned by assign_ids
                kind,
                pos: eframe::egui::pos2(
                    slot_center_x - crate::constants::GATE_SIZE / 2.0,
                    line_y - crate::constants::GATE_SIZE / 2.0,
                ),
                wire: wire_idx,
                span,
                angle,
            });
        }
    }
    gates
}

/// Reverse of `gate_token`. Handles the `QFT<n>` / `QFT†<n>` span
/// suffixes and the parametric `P(<angle>)` form. Returns `None` for
/// unrecognised tokens (e.g. tokens emitted by a future qni version
/// we don't yet know about).
///
/// The third tuple slot is the angle string — `Some("π/2")` etc. —
/// `None` for non-parametric gates and for bare `"P"` (which uses
/// the editor's default angle).
fn token_to_gate(token: &str) -> Option<(GateKind, usize, Option<String>)> {
    if let Some(rest) = token.strip_prefix("QFT†") {
        let span: usize = rest.parse().ok()?;
        return Some((GateKind::QftDaggerGate, span.max(1), None));
    }
    if let Some(rest) = token.strip_prefix("QFT") {
        let span: usize = rest.parse().ok()?;
        return Some((GateKind::QftGate, span.max(1), None));
    }
    // Parametric `P(...)` / `Rx(...)` / `Ry(...)` / `Rz(...)` —
    // mirrors qni's `quantum-circuit-element.ts::angleParameter`:
    // strip the outer parens, trim, replace the first `_` with `/`
    // so the URL-safe `"π_2"` becomes the canonical `"π/2"`.
    for (prefix, kind) in [
        ("P(", GateKind::Phase),
        ("Rx(", GateKind::Rx),
        ("Ry(", GateKind::Ry),
        ("Rz(", GateKind::Rz),
    ] {
        if let Some(rest) = token.strip_prefix(prefix) {
            if let Some(inner) = rest.strip_suffix(')') {
                let trimmed = inner.trim();
                let normalized = if let Some(idx) = trimmed.find('_') {
                    let mut s = String::with_capacity(trimmed.len());
                    s.push_str(&trimmed[..idx]);
                    s.push('/');
                    s.push_str(&trimmed[idx + 1..]);
                    s
                } else {
                    trimmed.to_string()
                };
                return Some((kind, 1, Some(normalized)));
            }
        }
    }
    let kind = match token {
        "H" => GateKind::H,
        "X" => GateKind::X,
        "Y" => GateKind::Y,
        "Z" => GateKind::Z,
        "S" => GateKind::S,
        "S†" => GateKind::SDagger,
        "T" => GateKind::T,
        "T†" => GateKind::TDagger,
        "X^½" => GateKind::SqrtX,
        "Rx" => GateKind::Rx,
        "Ry" => GateKind::Ry,
        "Rz" => GateKind::Rz,
        "P" => GateKind::Phase,
        "Swap" => GateKind::Swap,
        "•" => GateKind::Control,
        "◦" => GateKind::AntiControl,
        "Measure" => GateKind::Measurement,
        "Bloch" => GateKind::BlochDisplay,
        "|0>" => GateKind::Write0,
        "|1>" => GateKind::Write1,
        "…" => GateKind::Spacer,
        _ => return None,
    };
    Some((kind, 1, None))
}

/// Assign sequential ids starting from 1 and return the next available
/// id (so `QniApp::next_gate_id` can resume without collision).
fn assign_ids(mut gates: Vec<PlacedGate>) -> (Vec<PlacedGate>, u32) {
    let mut next = 1u32;
    for gate in &mut gates {
        gate.id = next;
        next += 1;
    }
    (gates, next)
}

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
fn parse_cols(s: &str) -> Option<Vec<Vec<Option<String>>>> {
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
