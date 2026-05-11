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
//!   `circuit=` key, only falling back to percent-encoding when the
//!   JSON itself contains `%` or `&`. Browsers tolerate `{ } [ ] " : ,`
//!   in `location.hash` and display them literally, so the address bar
//!   shows e.g. `#circuit={"cols":[["H"],["•","X"]]}`.
//!
//! We want the Quirk readability while keeping the qni JSON shape so
//! the URL is portable between projects. This module produces the
//! shared `{"cols":[...]}` JSON and pushes it to `location.hash`.
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
            let Some(token) = gate_token(gate.kind, gate.span) else {
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

/// Map a gate kind + span to its URL token. `None` for kinds that
/// shouldn't appear in the serialised circuit at all (currently no
/// such kinds — every `GateKind` is serialisable).
fn gate_token(kind: GateKind, span: usize) -> Option<String> {
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
        GateKind::Rx => "Rx".to_string(),
        GateKind::Ry => "Ry".to_string(),
        GateKind::Rz => "Rz".to_string(),
        GateKind::Phase => "P".to_string(),
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

/// Push the serialised circuit into `location.hash`, Quirk-style:
///   #circuit={"cols":[...]}
///
/// JSON is written raw; falls back to `encodeURIComponent` only when
/// the payload contains `%` or `&` (which would break hash parsing).
/// Uses `history.replaceState` so navigation back/forward doesn't fill
/// up with one entry per drop.
#[cfg(target_arch = "wasm32")]
pub(crate) fn write_circuit_to_url(json: &str) {
    use wasm_bindgen::JsValue;
    let Some(window) = web_sys::window() else {
        return;
    };
    let payload = if json.contains('%') || json.contains('&') {
        js_sys::encode_uri_component(json).as_string().unwrap_or_else(|| json.to_string())
    } else {
        json.to_string()
    };
    let hash = format!("#circuit={payload}");
    // Compose a full URL so `replaceState` doesn't rewrite the path.
    let location = window.location();
    let base = match (location.origin(), location.pathname(), location.search()) {
        (Ok(o), Ok(p), Ok(s)) => format!("{o}{p}{s}"),
        _ => return,
    };
    let url = format!("{base}{hash}");
    let _ = window
        .history()
        .ok()
        .and_then(|h| h.replace_state_with_url(&JsValue::NULL, "", Some(&url)).ok());
}

/// Non-wasm stub so callers don't need to `cfg`-gate.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn write_circuit_to_url(_json: &str) {}
