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
//!   Chance       — 1-qubit Chance probability display
//!   Chance<n>    — n-qubit Chance display, n = 2..16
//!   QFT<n>       — n-qubit QFT (span suffix)
//!   QFT†<n>      — n-qubit inverse QFT
//!
//! Multi-qubit gates (CNOT, swap, controls) are split per wire across
//! the same column; the wire's array index is its qubit number. The
//! resizable-span gates (Chance, QFT / QFT†) emit their token only at
//! the top wire of the span; the lower wires within the span are left
//! as `1` (the empty marker) — matching qni / Quirk display-gate style,
//! where the element owns just the top dropzone.

mod decode;
mod encode;
mod history;
mod parser;

/// JSON `circuit=` payload for an empty circuit. Quirk uses the same
/// literal (`{"cols":[]}`) when the circuit is cleared.
pub(crate) const EMPTY_CIRCUIT_JSON: &str = r#"{"cols":[]}"#;

pub(crate) use decode::{
    current_url_has_circuit_payload, parse_circuit_from_url, parse_circuit_json,
    qubit_count_from_gates, summarize_circuit_json,
};
pub(crate) use encode::{circuit_columns_to_json, circuit_to_json};
pub(crate) use history::write_circuit_to_url;
