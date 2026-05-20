//! URL decoder (`location.hash` / qni path payload → `PlacedGate`s).

use crate::app::PlacedGate;
use crate::gates::GateKind;

use super::parser::parse_cols;

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
//  via `token_to_gate`. The semantic column index (= qni step index)
//  and wire index (= qubit number) are restored directly; the derived
//  draw position is then synchronised from that grid.
// ─────────────────────────────────────────────────────────────────────

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

#[cfg(target_arch = "wasm32")]
pub(crate) fn current_url_has_circuit_payload() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let location = window.location();
    if let Ok(hash) = location.hash() {
        if try_decode(hash.strip_prefix('#').unwrap_or(&hash)).is_some() {
            return true;
        }
    }
    if let Ok(pathname) = location.pathname() {
        if let Some(last) = pathname.rsplit('/').next() {
            return try_decode(last).is_some();
        }
    }
    false
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn current_url_has_circuit_payload() -> bool {
    false
}

/// Decode one canonical circuit JSON checkpoint. Unlike URL parsing,
/// `{"cols":[]}` is a valid empty circuit and returns no gates with
/// `next_gate_id = 1`.
pub(crate) fn parse_circuit_json(json: &str) -> (Vec<PlacedGate>, u32) {
    let cols = parse_cols(json).unwrap_or_default();
    assign_ids(build_gates(&cols))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CircuitJsonSummary {
    pub(crate) qubits: usize,
    pub(crate) columns: usize,
    pub(crate) gate_count: usize,
}

/// Summarise a canonical circuit JSON payload without simulating it.
/// Used by browser-local persistence metadata; state-vector / Bloch /
/// measurement values remain GPU-only.
pub(crate) fn summarize_circuit_json(json: &str) -> Option<CircuitJsonSummary> {
    let cols = parse_cols(json)?;
    let mut qubits = 0usize;
    let mut gate_count = 0usize;
    for col in &cols {
        for (wire, entry) in col.iter().enumerate() {
            if let Some(token) = entry.as_deref() {
                let (_, span, _) = token_to_gate(token)?;
                gate_count += 1;
                qubits = qubits.max(wire + span);
            }
        }
    }
    Some(CircuitJsonSummary {
        qubits,
        columns: cols.len(),
        gate_count,
    })
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
/// if present so Quirk URLs paste cleanly. A valid empty `cols` payload
/// is a real circuit checkpoint and must override any stale path payload.
fn try_decode(payload: &str) -> Option<Vec<PlacedGate>> {
    if payload.is_empty() {
        return None;
    }
    let payload = payload
        .strip_prefix("circuit=")
        .map(|value| value.split('&').next().unwrap_or(value))
        .unwrap_or(payload);
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
    let has_gate_tokens = cols.iter().flatten().any(Option::is_some);
    if gates.is_empty() && has_gate_tokens {
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
/// `PlacedGate`. The URL columns become semantic gate columns; pixel
/// position is derived by `PlacedGate::new`.
fn build_gates(cols: &[Vec<Option<String>>]) -> Vec<PlacedGate> {
    let mut gates = Vec::new();
    for (col_idx, col) in cols.iter().enumerate() {
        for (wire_idx, entry) in col.iter().enumerate() {
            let Some(token) = entry.as_deref() else {
                continue;
            };
            let Some((kind, span, angle)) = token_to_gate(token) else {
                continue;
            };
            gates.push(PlacedGate::new(0, kind, col_idx, wire_idx, span, angle));
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
    if let Some(rest) = token.strip_prefix("Chance") {
        let span = if rest.is_empty() {
            1
        } else {
            rest.parse().ok()?
        };
        return Some((GateKind::ChanceDisplay, span.clamp(1, 16), None));
    }
    if let Some(rest) = token.strip_prefix("Amps") {
        let span: usize = rest.parse().ok()?;
        if !(1..=16).contains(&span) {
            return None;
        }
        return Some((GateKind::AmplitudeDisplay, span, None));
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
    GateKind::from_url_token(token).map(|kind| (kind, 1, None))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amplitude_span_sixteen_decodes() {
        let (gates, _) = parse_circuit_json(r#"{"cols":[["Amps16"]]}"#);

        assert_eq!(
            gates.first().map(|gate| (gate.kind, gate.span)),
            Some((GateKind::AmplitudeDisplay, 16))
        );
    }

    #[test]
    fn amplitude_decode_preserves_column_index() {
        let (gates, _) = parse_circuit_json(r#"{"cols":[["H"],["Amps3"]]}"#);

        assert_eq!(
            gates
                .iter()
                .find(|gate| gate.kind == GateKind::AmplitudeDisplay)
                .map(|gate| gate.column),
            Some(1)
        );
    }

    #[test]
    fn bare_amplitude_token_is_ignored() {
        let (gates, _) = parse_circuit_json(r#"{"cols":[["Amps"]]}"#);

        assert_eq!(gates.len(), 0);
    }
}
