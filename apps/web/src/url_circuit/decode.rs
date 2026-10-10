//! URL decoder (`location.hash` / qni path payload → `PlacedGate`s).

use crate::app::{CircuitBlocks, CircuitColumnIndex, GateId, GateIdAllocator, PlacedGate};
use crate::gates::GateSpan;
use crate::gates::{GateFlag, GateKind, ParametricAngle};

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
//
//  qni circuit-block markers are split out before that: a column holding
//  only `"{<label>"` (or qni's `"[<label>"`) opens a block and `"}"` (or
//  `"]"`) closes it. Marker columns are not circuit steps, so the gate
//  columns after them are renumbered as if they were absent.
// ─────────────────────────────────────────────────────────────────────

/// A decoded circuit: placed gates with sequential ids plus the circuit
/// blocks drawn around them.
#[derive(Debug, Default)]
pub(crate) struct DecodedCircuit {
    pub(crate) gates: Vec<PlacedGate>,
    pub(crate) gate_ids: GateIdAllocator,
    pub(crate) blocks: CircuitBlocks,
}

/// Decode the URL. Returns an empty circuit (with `next_gate_id = 1`) if no
/// circuit payload was found.
#[cfg(target_arch = "wasm32")]
pub(crate) fn parse_circuit_from_url() -> DecodedCircuit {
    let Some(window) = web_sys::window() else {
        return DecodedCircuit::default();
    };
    let location = window.location();
    // 1. Hash fragment (our native write path).
    if let Ok(hash) = location.hash() {
        if let Some(circuit) = try_decode(hash.strip_prefix('#').unwrap_or(&hash)) {
            return circuit;
        }
    }
    // 2. Last path segment (qni-compatible — JSON percent-encoded in
    //    the URL path, e.g. `/%7B%22cols%22:...%7D`).
    if let Ok(pathname) = location.pathname() {
        if let Some(last) = pathname.rsplit('/').next() {
            if let Some(circuit) = try_decode(last) {
                return circuit;
            }
        }
    }
    DecodedCircuit::default()
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn parse_circuit_from_url() -> DecodedCircuit {
    DecodedCircuit::default()
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
/// `next_gate_id = 1`. Malformed JSON (including malformed block markers)
/// also decodes to the empty circuit.
pub(crate) fn parse_circuit_json(json: &str) -> DecodedCircuit {
    let Some(columns) = parse_cols(json).and_then(split_block_markers) else {
        return DecodedCircuit::default();
    };
    with_gate_ids(build_gates(&columns.cols), columns.blocks)
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
    let cols = split_block_markers(parse_cols(json)?)?.cols;
    let mut qubits = 0usize;
    let mut gate_count = 0usize;
    for col in &cols {
        for (wire, entry) in col.iter().enumerate() {
            if let Some(token) = entry.as_deref() {
                let gate = token_to_gate(token)?;
                gate_count += 1;
                qubits = qubits.max(wire + gate.span);
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
        .map(|g| g.wire.as_usize() + g.span.get().saturating_sub(1) + 1)
        .max()
        .unwrap_or(0)
}

/// Try to decode `payload` (a possibly-percent-encoded `{"cols":...}`
/// snippet) into a list of `PlacedGate`. Strips a `circuit=` prefix
/// if present so Quirk URLs paste cleanly. A valid empty `cols` payload
/// is a real circuit checkpoint and must override any stale path payload.
fn try_decode(payload: &str) -> Option<DecodedCircuit> {
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
    let columns = split_block_markers(parse_cols(json)?)?;
    let gates = build_gates(&columns.cols);
    let has_gate_tokens = columns.cols.iter().flatten().any(Option::is_some);
    if gates.is_empty() && has_gate_tokens {
        None
    } else {
        Some(with_gate_ids(gates, columns.blocks))
    }
}

/// Gate columns with the circuit-block marker columns removed.
struct SplitColumns {
    cols: Vec<Vec<Option<String>>>,
    blocks: CircuitBlocks,
}

/// What a raw `cols` entry means for circuit blocks.
enum ColumnRole {
    Gates,
    OpenBlock(String),
    CloseBlock,
}

/// Separate qni circuit-block markers from the gate columns. Returns `None`
/// for malformed blocks, which rejects the whole circuit just like any
/// other malformed JSON:
///
/// * a marker sharing its column with another entry (`[1,"{a"]`),
/// * an opening marker without a label (`"{"`), as qni requires one,
/// * a block opened inside another block (qni blocks never nest),
/// * a closing marker without an open block.
///
/// A block still open at the end of `cols` is closed there, matching qni's
/// loader. Blocks without any column (`["{a"],["}"]`) are dropped, matching
/// qni's serializer, which only writes blocks around non-empty steps.
fn split_block_markers(raw: Vec<Vec<Option<String>>>) -> Option<SplitColumns> {
    let mut cols = Vec::with_capacity(raw.len());
    let mut blocks = CircuitBlocks::default();
    let mut open: Option<(String, CircuitColumnIndex)> = None;
    let close = |blocks: &mut CircuitBlocks, (label, start), end| {
        // Columns are appended in order and empty ranges are skipped, so
        // `push` can only fail for an empty block.
        let _ = blocks.push(label, start, end);
    };
    for col in raw {
        match column_role(&col)? {
            ColumnRole::Gates => cols.push(col),
            ColumnRole::OpenBlock(label) => {
                if open.is_some() {
                    return None;
                }
                open = Some((label, CircuitColumnIndex::new(cols.len())));
            }
            ColumnRole::CloseBlock => {
                let block = open.take()?;
                close(&mut blocks, block, CircuitColumnIndex::new(cols.len()));
            }
        }
    }
    if let Some(block) = open {
        close(&mut blocks, block, CircuitColumnIndex::new(cols.len()));
    }
    Some(SplitColumns { cols, blocks })
}

fn column_role(col: &[Option<String>]) -> Option<ColumnRole> {
    let is_marker = |entry: &Option<String>| {
        entry
            .as_deref()
            .is_some_and(|token| token.starts_with(['{', '[', '}', ']']))
    };
    if !col.iter().any(is_marker) {
        return Some(ColumnRole::Gates);
    }
    let [Some(token)] = col else {
        return None;
    };
    if token == "}" || token == "]" {
        return Some(ColumnRole::CloseBlock);
    }
    let label = token.strip_prefix(['{', '['])?;
    (!label.is_empty()).then(|| ColumnRole::OpenBlock(label.to_owned()))
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
            let Some(token) = token_to_gate(token) else {
                continue;
            };
            // 0 は不正なスパン。`GateSpan::try_new` を唯一の下限ゲートにし、
            // `unwrap_or(SINGLE)` のような暗黙の 0 → 1 丸めは置かない（不正な
            // トークンは復元せず読み飛ばす）。
            let Ok(span) = GateSpan::try_new(token.span) else {
                continue;
            };
            let gate = PlacedGate::new(
                GateId::from_u32(0),
                token.kind,
                crate::app::CircuitColumnIndex::new(col_idx),
                crate::app::WireIndex::new(wire_idx),
                span,
                token.angle,
            );
            let gate = match token.flag {
                Some(flag) => {
                    let Some(gate) = gate.with_flag(flag) else {
                        continue;
                    };
                    gate
                }
                None => gate,
            };
            gates.push(gate);
        }
    }
    gates
}

/// Decodes one palette entry token with the circuit JSON vocabulary, so an
/// embed palette accepts what a circuit cell accepts: `H`, `|0>`, `•`,
/// `Bloch`, and parametric `P(π/4)`. Span suffixes (`QFT3`) are rejected
/// because palette gates always start with their default span.
/// Measurement variables and conditions (`Measure>a`, `X<a`) are rejected
/// too: a palette gate is a template without per-circuit variable names.
pub(crate) fn palette_token_to_gate(token: &str) -> Option<(GateKind, Option<ParametricAngle>)> {
    match token_to_gate(token) {
        Some(DecodedToken {
            kind,
            span: 1,
            angle,
            flag: None,
        }) => Some((kind, angle)),
        Some(_) => None,
        None => GateKind::from_url_token(token).map(|kind| (kind, None)),
    }
}

/// One decoded `cols` entry.
#[derive(Debug, PartialEq)]
struct DecodedToken {
    kind: GateKind,
    span: usize,
    /// The angle value for parametric tokens (`Some(π/2)` etc.); `None` for
    /// non-parametric gates and for bare parametric tokens (which use the
    /// editor's default angle).
    angle: Option<ParametricAngle>,
    /// qni measurement variable link (`Measure>a` / `X<a`).
    flag: Option<GateFlag>,
}

/// Reverse of `gate_token`. Handles the qni measurement variable suffixes
/// (`Measure>a`, `X<a`), the `QFT<n>` / `QFT†<n>` span suffixes, and the
/// parametric `P(<angle>)` / `Rx(<angle>)` / `Ry(<angle>)` / `Rz(<angle>)`
/// forms. Returns `None` for unrecognised tokens (e.g. tokens emitted by a
/// future qni version we don't yet know about).
fn token_to_gate(token: &str) -> Option<DecodedToken> {
    let (base, flag) = GateFlag::split_token(token)?;
    let (kind, span, angle) = base_token_to_gate(base)?;
    Some(DecodedToken {
        kind,
        span,
        angle,
        flag,
    })
}

/// Decode a token without its measurement variable suffix.
fn base_token_to_gate(token: &str) -> Option<(GateKind, usize, Option<ParametricAngle>)> {
    if let Some(rest) = token.strip_prefix("QFT†") {
        let span: usize = rest.parse().ok()?;
        return Some((GateKind::QftDaggerGate, span, None));
    }
    if let Some(rest) = token.strip_prefix("QFT") {
        let span: usize = rest.parse().ok()?;
        return Some((GateKind::QftGate, span, None));
    }
    if let Some(rest) = token.strip_prefix("Probability") {
        let span = if rest.is_empty() {
            1
        } else {
            rest.parse().ok()?
        };
        return Some((GateKind::ProbabilityDisplay, span.min(16), None));
    }
    if let Some(rest) = token.strip_prefix("Amps") {
        let span: usize = rest.parse().ok()?;
        if !(1..=16).contains(&span) {
            return None;
        }
        return Some((GateKind::AmplitudeDisplay, span, None));
    }
    if let Some(rest) = token.strip_prefix("Density") {
        let span = if rest.is_empty() {
            1
        } else {
            rest.parse().ok()?
        };
        if !(1..=8).contains(&span) {
            return None;
        }
        return Some((GateKind::DensityMatrixDisplay, span, None));
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
                let angle = ParametricAngle::parse_qni(trimmed).ok()?;
                return Some((kind, 1, Some(angle)));
            }
        }
    }
    GateKind::from_url_token(token).map(|kind| (kind, 1, None))
}

/// Assign sequential gate ids starting from 1 and keep the allocator so
/// `QniApp` can resume without collision.
fn with_gate_ids(mut gates: Vec<PlacedGate>, blocks: CircuitBlocks) -> DecodedCircuit {
    let mut gate_ids = GateIdAllocator::new();
    for gate in &mut gates {
        gate.id = gate_ids.allocate();
    }
    DecodedCircuit {
        gates,
        gate_ids,
        blocks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qubit_count::QubitCount;

    fn qubit_count(value: usize) -> QubitCount {
        QubitCount::try_new(value).expect("test qubit count must be non-zero")
    }

    #[test]
    fn amplitude_span_sixteen_decodes() {
        let gates = parse_circuit_json(r#"{"cols":[["Amps16"]]}"#).gates;

        assert_eq!(
            gates.first().map(|gate| (gate.kind, gate.span.get())),
            Some((GateKind::AmplitudeDisplay, 16))
        );
    }

    #[test]
    fn decoded_gates_get_sequential_ids_from_one() {
        let gates = parse_circuit_json(r#"{"cols":[["H"],["X"]]}"#).gates;

        assert_eq!(
            gates.iter().map(|gate| gate.id).collect::<Vec<_>>(),
            vec![GateId::from_u32(1), GateId::from_u32(2)]
        );
    }

    #[test]
    fn amplitude_decode_preserves_column_index() {
        let gates = parse_circuit_json(r#"{"cols":[["H"],["Amps3"]]}"#).gates;

        assert_eq!(
            gates
                .iter()
                .find(|gate| gate.kind == GateKind::AmplitudeDisplay)
                .map(|gate| gate.column),
            Some(crate::app::CircuitColumnIndex::new(1))
        );
    }

    #[test]
    fn nonzero_column_round_trips_as_later_cols_entry() {
        let circuit = parse_circuit_json(r#"{"cols":[[1],["H"]]}"#);

        assert_eq!(
            crate::url_circuit::circuit_to_json(
                &circuit.gates,
                &circuit.blocks,
                crate::qubit_count::QubitCount::try_new(1).expect("test qubit count"),
            ),
            r#"{"cols":[[1],["H"]]}"#
        );
    }

    #[test]
    fn bare_amplitude_token_is_ignored() {
        let gates = parse_circuit_json(r#"{"cols":[["Amps"]]}"#).gates;

        assert_eq!(gates.len(), 0);
    }

    #[test]
    fn zero_span_qft_token_is_ignored() {
        // スパン 0 は不正。`GateSpan::try_new` が弾き、暗黙に 1 へ丸めず読み飛ばす。
        let gates = parse_circuit_json(r#"{"cols":[["QFT0"]]}"#).gates;

        assert_eq!(gates.len(), 0);
    }

    #[test]
    fn qft_span_decodes() {
        // `.max(1)` 削除後も正常スパンはそのまま復元される。
        let gates = parse_circuit_json(r#"{"cols":[["QFT3"]]}"#).gates;

        assert_eq!(
            gates.first().map(|gate| (gate.kind, gate.span.get())),
            Some((GateKind::QftGate, 3))
        );
    }

    #[test]
    fn probability_span_over_max_clamps_to_sixteen() {
        // `clamp(1, 16)` → `min(16)` 変更後も上限 16 への切り詰めは保たれる。
        let gates = parse_circuit_json(r#"{"cols":[["Probability20"]]}"#).gates;

        assert_eq!(
            gates.first().map(|gate| (gate.kind, gate.span.get())),
            Some((GateKind::ProbabilityDisplay, 16))
        );
    }

    #[test]
    fn probability_zero_span_is_ignored() {
        // スパン 0 は `GateSpan::try_new` が弾き、暗黙に 1 へ丸めず読み飛ばす。
        let gates = parse_circuit_json(r#"{"cols":[["Probability0"]]}"#).gates;

        assert_eq!(gates.len(), 0);
    }

    #[test]
    fn parametric_angle_decodes_normalized_label() {
        let gates = parse_circuit_json(r#"{"cols":[["P(4π_8)"]]}"#).gates;

        assert_eq!(
            gates
                .first()
                .and_then(|gate| gate.angle)
                .map(|angle| angle.label()),
            Some("π/2".to_owned())
        );
    }

    #[test]
    fn parametric_angle_round_trip_uses_normalized_url_label() {
        let circuit = parse_circuit_json(r#"{"cols":[["P(4π_8)"]]}"#);

        assert_eq!(
            crate::url_circuit::circuit_to_json(&circuit.gates, &circuit.blocks, qubit_count(1)),
            r#"{"cols":[["P(π_2)"]]}"#
        );
    }

    #[test]
    fn parametric_angle_invalid_token_is_ignored() {
        let gates = parse_circuit_json(r#"{"cols":[["P()"]]}"#).gates;

        assert_eq!(gates.len(), 0);
    }

    #[test]
    fn parametric_angle_zero_denominator_token_is_ignored() {
        let gates = parse_circuit_json(r#"{"cols":[["P(π_0)"]]}"#).gates;

        assert_eq!(gates.len(), 0);
    }

    #[test]
    fn bare_phase_token_keeps_missing_angle() {
        let gates = parse_circuit_json(r#"{"cols":[["P"]]}"#).gates;

        assert_eq!(gates.first().map(|gate| gate.angle), Some(None));
    }

    #[test]
    fn bare_rx_token_keeps_missing_angle() {
        let gates = parse_circuit_json(r#"{"cols":[["Rx"]]}"#).gates;

        assert_eq!(gates.first().map(|gate| gate.angle), Some(None));
    }

    #[test]
    fn bare_ry_token_keeps_missing_angle() {
        let gates = parse_circuit_json(r#"{"cols":[["Ry"]]}"#).gates;

        assert_eq!(gates.first().map(|gate| gate.angle), Some(None));
    }

    #[test]
    fn bare_rz_token_keeps_missing_angle() {
        let gates = parse_circuit_json(r#"{"cols":[["Rz"]]}"#).gates;

        assert_eq!(gates.first().map(|gate| gate.angle), Some(None));
    }

    #[test]
    fn density_span_eight_decodes() {
        let gates = parse_circuit_json(r#"{"cols":[["Density8"]]}"#).gates;

        assert_eq!(
            gates.first().map(|gate| (gate.kind, gate.span.get())),
            Some((GateKind::DensityMatrixDisplay, 8))
        );
    }

    #[test]
    fn density_span_nine_is_ignored() {
        let gates = parse_circuit_json(r#"{"cols":[["Density9"]]}"#).gates;

        assert_eq!(gates.len(), 0);
    }

    const BELL_BLOCK_JSON: &str = r#"{"cols":[["|0>","|0>"],["{量子もつれ"],["H"],["•","X"],["}"],["Measure"],[1,"Measure"]]}"#;

    fn block_ranges(circuit: &DecodedCircuit) -> Vec<(String, usize, usize)> {
        circuit
            .blocks
            .iter()
            .map(|block| {
                (
                    block.label().to_owned(),
                    block.start().as_usize(),
                    block.end().as_usize(),
                )
            })
            .collect()
    }

    fn round_trip(json: &str) -> String {
        let circuit = parse_circuit_json(json);
        let qubits = qubit_count_from_gates(&circuit.gates).max(1);
        crate::url_circuit::circuit_to_json(&circuit.gates, &circuit.blocks, qubit_count(qubits))
    }

    #[test]
    fn block_marker_columns_are_not_circuit_steps() {
        let circuit = parse_circuit_json(BELL_BLOCK_JSON);

        assert_eq!(
            circuit
                .gates
                .iter()
                .map(|gate| (gate.kind, gate.column.as_usize()))
                .collect::<Vec<_>>(),
            vec![
                (GateKind::Write0, 0),
                (GateKind::Write0, 0),
                (GateKind::H, 1),
                (GateKind::Control, 2),
                (GateKind::X, 2),
                (GateKind::Measurement, 3),
                (GateKind::Measurement, 4),
            ]
        );
    }

    #[test]
    fn block_decodes_label_and_column_range() {
        assert_eq!(
            block_ranges(&parse_circuit_json(BELL_BLOCK_JSON)),
            vec![("量子もつれ".to_owned(), 1, 3)]
        );
    }

    #[test]
    fn qni_bracket_block_markers_decode() {
        assert_eq!(
            block_ranges(&parse_circuit_json(r#"{"cols":[["[Bell"],["H"],["]"]]}"#)),
            vec![("Bell".to_owned(), 0, 1)]
        );
    }

    #[test]
    fn adjacent_blocks_decode_separately() {
        assert_eq!(
            block_ranges(&parse_circuit_json(
                r#"{"cols":[["{a"],["H"],["}"],["{b"],["X"],["}"]]}"#
            )),
            vec![("a".to_owned(), 0, 1), ("b".to_owned(), 1, 2)]
        );
    }

    #[test]
    fn unclosed_block_closes_at_the_last_column() {
        assert_eq!(
            block_ranges(&parse_circuit_json(
                r#"{"cols":[["H"],["{a"],["X"],["Z"]]}"#
            )),
            vec![("a".to_owned(), 1, 3)]
        );
    }

    #[test]
    fn empty_block_is_dropped() {
        assert_eq!(
            block_ranges(&parse_circuit_json(r#"{"cols":[["H"],["{a"],["}"]]}"#)),
            vec![]
        );
    }

    #[test]
    fn nested_block_rejects_the_circuit() {
        assert!(try_decode(r#"{"cols":[["{a"],["{b"],["H"],["}"],["}"]]}"#).is_none());
    }

    #[test]
    fn stray_block_close_rejects_the_circuit() {
        assert!(try_decode(r#"{"cols":[["H"],["}"]]}"#).is_none());
    }

    #[test]
    fn block_marker_sharing_a_column_rejects_the_circuit() {
        assert!(try_decode(r#"{"cols":[[1,"{a"],["H"],["}"]]}"#).is_none());
    }

    #[test]
    fn block_marker_next_to_a_gate_rejects_the_circuit() {
        assert!(try_decode(r#"{"cols":[["{a","H"],["}"]]}"#).is_none());
    }

    #[test]
    fn unlabelled_block_rejects_the_circuit() {
        assert!(try_decode(r#"{"cols":[["{"],["H"],["}"]]}"#).is_none());
    }

    #[test]
    fn malformed_block_checkpoint_decodes_to_the_empty_circuit() {
        assert_eq!(
            parse_circuit_json(r#"{"cols":[["H"],["}"]]}"#).gates.len(),
            0
        );
    }

    #[test]
    fn summary_does_not_count_block_marker_columns() {
        assert_eq!(
            summarize_circuit_json(BELL_BLOCK_JSON),
            Some(CircuitJsonSummary {
                qubits: 2,
                columns: 5,
                gate_count: 7,
            })
        );
    }

    #[test]
    fn block_circuit_round_trips_through_json() {
        assert_eq!(round_trip(BELL_BLOCK_JSON), BELL_BLOCK_JSON);
    }

    #[test]
    fn adjacent_blocks_round_trip_through_json() {
        let json = r#"{"cols":[["{a"],["H"],["}"],["{b"],["X"],["}"]]}"#;

        assert_eq!(round_trip(json), json);
    }

    #[test]
    fn block_ending_in_empty_columns_round_trips_through_json() {
        let json = r#"{"cols":[["{a"],["H"],[1],["}"]]}"#;

        assert_eq!(round_trip(json), json);
    }

    #[test]
    fn qni_bracket_block_markers_encode_as_braces() {
        assert_eq!(
            round_trip(r#"{"cols":[["[Bell"],["H"],["]"]]}"#),
            r#"{"cols":[["{Bell"],["H"],["}"]]}"#
        );
    }

    #[test]
    fn unclosed_block_encodes_its_closing_marker() {
        assert_eq!(
            round_trip(r#"{"cols":[["{a"],["H"]]}"#),
            r#"{"cols":[["{a"],["H"],["}"]]}"#
        );
    }

    #[test]
    fn block_label_with_quote_round_trips_through_json() {
        let json = r#"{"cols":[["{say \"hi\""],["H"],["}"]]}"#;

        assert_eq!(round_trip(json), json);
    }

    #[test]
    fn external_gpu_columns_omit_block_markers() {
        let circuit = parse_circuit_json(BELL_BLOCK_JSON);

        assert_eq!(
            crate::url_circuit::circuit_columns_to_json(&circuit.gates, qubit_count(2)),
            r#"[["|0>","|0>"],["H"],["•","X"],["Measure"],[1,"Measure"]]"#
        );
    }

    fn flag_name(raw: &str) -> crate::gates::FlagName {
        crate::gates::FlagName::parse(raw).expect("test flag name must be non-empty")
    }

    fn decoded_flags(json: &str) -> Vec<Option<GateFlag>> {
        parse_circuit_json(json)
            .gates
            .into_iter()
            .map(|gate| gate.flag)
            .collect()
    }

    #[test]
    fn measurement_flag_decodes() {
        assert_eq!(
            decoded_flags(r#"{"cols":[["Measure>aliceX"]]}"#),
            vec![Some(GateFlag::Set(flag_name("aliceX")))]
        );
    }

    #[test]
    fn conditional_gate_decodes_its_base_kind() {
        let gates = parse_circuit_json(r#"{"cols":[["X<aliceX"]]}"#).gates;

        assert_eq!(gates.first().map(|gate| gate.kind), Some(GateKind::X));
    }

    #[test]
    fn conditional_gate_decodes_its_condition() {
        assert_eq!(
            decoded_flags(r#"{"cols":[["H<bobH"]]}"#),
            vec![Some(GateFlag::If(flag_name("bobH")))]
        );
    }

    #[test]
    fn conditional_measurement_token_is_not_a_gate() {
        assert!(parse_circuit_json(r#"{"cols":[["Measure<a"]]}"#)
            .gates
            .is_empty());
    }

    #[test]
    fn palette_rejects_conditional_tokens() {
        assert_eq!(palette_token_to_gate("X<a"), None);
    }

    #[test]
    fn palette_rejects_measurement_variables() {
        assert_eq!(palette_token_to_gate("Measure>a"), None);
    }

    #[test]
    fn summary_counts_flagged_gates() {
        let summary = summarize_circuit_json(r#"{"cols":[["Measure>a"],[1,"X<a"]]}"#);

        assert_eq!(summary.map(|summary| summary.gate_count), Some(2));
    }

    #[test]
    fn flagged_gates_round_trip_through_json() {
        let json = r#"{"cols":[["H"],["Measure>a"],[1,"X<a"],[1,"S†<a"],[1,"X^½<a"]]}"#;

        assert_eq!(round_trip(json), json);
    }

    #[test]
    fn blank_flag_name_round_trips_as_a_plain_gate() {
        assert_eq!(
            round_trip(r#"{"cols":[["Measure> "],["X< "]]}"#),
            r#"{"cols":[["Measure"],["X"]]}"#
        );
    }

    /// qni `apps/tutorial/bb84_circuit.html` (qniapp/qni@acf87bf).
    const BB84_JSON: &str = r#"{"cols":[["{送信内容を決める2つの乱数を生成"],["|0>"],["H"],["Measure>aliceX"],["|0>"],["H"],["Measure>aliceH"],["}"],["|0>"],["{|1⟩をセット"],["X<aliceX"],["}"],["Bloch"],["{Hを適用"],["H<aliceH"],["}"],["Bloch"],["Swap","Swap"],["{🕶イブ"],[1,"Measure>eveX"],[1,"|0>"],[1,"X<eveX"],[1,"Bloch"],["}"],[1],["{Hのための乱数を生成"],[1,1,"|0>"],[1,1,"H"],[1,1,"Measure>bobH"],["}"],[1,"Swap","Swap"],["{Hを適用"],[1,1,"H<bobH"],["}"],[1,1,"Bloch"],["{測定"],[1,1,"Measure"],["}"],[1]]}"#;

    #[test]
    fn bb84_circuit_decodes_every_gate() {
        assert_eq!(parse_circuit_json(BB84_JSON).gates.len(), 25);
    }

    #[test]
    fn bb84_circuit_round_trips_through_json() {
        // qni drops the trailing empty step when it serialises the circuit.
        let expected = BB84_JSON.replace(r#",[1]]}"#, "]}");

        assert_eq!(round_trip(BB84_JSON), expected);
    }
}
