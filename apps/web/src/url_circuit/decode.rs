use crate::app::{
    CircuitBlockError, CircuitBlocks, CircuitColumnIndex, GateId, GateIdAllocator, PlacedGate,
};
use crate::gates::GateSpan;
use crate::gates::{GateFlag, GateKind, ParametricAngle, SAMPLE_MAX_SPAN};
use crate::layout::gate_width_cols;

use super::parser::{parse_circuit_document, CircuitDocument};

#[derive(Debug, Default)]
pub(crate) struct DecodedCircuit {
    pub(crate) gates: Vec<PlacedGate>,
    pub(crate) gate_ids: GateIdAllocator,
    pub(crate) blocks: CircuitBlocks,
    pub(crate) title: String,
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn parse_circuit_from_url() -> DecodedCircuit {
    let Some(window) = web_sys::window() else {
        return DecodedCircuit::default();
    };
    let location = window.location();
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

pub(crate) fn parse_circuit_json(json: &str) -> DecodedCircuit {
    let Some((mut columns, title)) = parse_circuit_document(json).and_then(split_document) else {
        return DecodedCircuit::default();
    };
    let Some((gates, _)) = build_gates(&columns.cols, &mut columns.blocks) else {
        return DecodedCircuit::default();
    };
    with_gate_ids(gates, columns.blocks, title)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CircuitJsonSummary {
    pub(crate) qubits: usize,
    pub(crate) columns: usize,
    pub(crate) gate_count: usize,
}

pub(crate) fn summarize_circuit_json(json: &str) -> Option<CircuitJsonSummary> {
    let mut parsed = split_block_markers(parse_circuit_document(json)?.cols)?;
    let cols = &parsed.cols;
    let mut qubits = 0usize;
    let mut gate_count = 0usize;
    for col in cols {
        for (wire, entry) in col.iter().enumerate() {
            if let Some(token) = entry.as_deref() {
                let gate = token_to_gate(token)?;
                gate_count += 1;
                qubits = qubits.max(wire + gate.span);
            }
        }
    }
    let (_, columns) = build_gates(cols, &mut parsed.blocks)?;
    Some(CircuitJsonSummary {
        qubits,
        columns,
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
    let (mut columns, title) = split_document(parse_circuit_document(json)?)?;
    let (gates, _) = build_gates(&columns.cols, &mut columns.blocks)?;
    let has_gate_tokens = columns.cols.iter().flatten().any(Option::is_some);
    if gates.is_empty() && has_gate_tokens {
        None
    } else {
        Some(with_gate_ids(gates, columns.blocks, title))
    }
}

fn split_document(document: CircuitDocument) -> Option<(SplitColumns, String)> {
    Some((split_block_markers(document.cols)?, document.title))
}

struct SplitColumns {
    cols: Vec<Vec<Option<String>>>,
    blocks: CircuitBlocks,
}

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
                let (label, start) = open.take()?;
                push_omitting_empty(
                    &mut blocks,
                    label,
                    start,
                    CircuitColumnIndex::new(cols.len()),
                )?;
            }
        }
    }
    if let Some((label, start)) = open {
        push_omitting_empty(
            &mut blocks,
            label,
            start,
            CircuitColumnIndex::new(cols.len()),
        )?;
    }
    Some(SplitColumns { cols, blocks })
}

fn push_omitting_empty(
    blocks: &mut CircuitBlocks,
    label: String,
    start: CircuitColumnIndex,
    end: CircuitColumnIndex,
) -> Option<()> {
    match blocks.push(label, start, end) {
        Ok(()) => Some(()),
        Err(CircuitBlockError::Empty) => Some(()),
        Err(CircuitBlockError::Overlap | CircuitBlockError::Missing) => None,
    }
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

fn build_gates(
    cols: &[Vec<Option<String>>],
    blocks: &mut CircuitBlocks,
) -> Option<(Vec<PlacedGate>, usize)> {
    let mut gates = Vec::new();
    let mut shift = WideDisplayShift::default();
    for (col_idx, col) in cols.iter().enumerate() {
        let mut group = column_gates(col, col_idx);
        shift.place(col_idx, &mut group);
        gates.extend(group);
    }
    let (column_map, columns) = shift.into_column_map(cols.len());
    remap_blocks(blocks, &column_map)?;
    Some((gates, columns))
}

fn column_gates(col: &[Option<String>], col_idx: usize) -> Vec<PlacedGate> {
    let mut gates = Vec::new();
    for (wire_idx, entry) in col.iter().enumerate() {
        let Some(token) = entry.as_deref() else {
            continue;
        };
        let Some(token) = token_to_gate(token) else {
            continue;
        };
        let Ok(span) = GateSpan::try_new(token.span) else {
            continue;
        };
        let gate = PlacedGate::new(
            GateId::from_u32(0),
            token.kind,
            CircuitColumnIndex::new(col_idx),
            crate::app::WireIndex::new(wire_idx),
            span,
            token.angle.stored(),
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
    gates
}

#[derive(Default)]
struct WideDisplayShift {
    wire_ends: Vec<usize>,
    columns: Vec<usize>,
    offset: usize,
}

impl WideDisplayShift {
    fn place(&mut self, col_idx: usize, gates: &mut [PlacedGate]) {
        let column = self.column_for(col_idx, gates);
        self.offset = column - col_idx;
        self.columns.push(column);
        self.record(column, gates);
    }

    fn column_for(&mut self, col_idx: usize, gates: &[PlacedGate]) -> usize {
        let mut column = col_idx + self.offset;
        for gate in gates {
            let end_wire = gate.wire.as_usize() + gate.span.get();
            self.wire_ends.resize(self.wire_ends.len().max(end_wire), 0);
            for &end in &self.wire_ends[gate.wire.as_usize()..end_wire] {
                column = column.max(end);
            }
        }
        column
    }

    fn record(&mut self, column: usize, gates: &mut [PlacedGate]) {
        for gate in gates {
            gate.column = CircuitColumnIndex::new(column);
            let occupied = column + gate_width_cols(gate.kind, gate.span.get());
            let end_wire = gate.wire.as_usize() + gate.span.get();
            for wire_end in &mut self.wire_ends[gate.wire.as_usize()..end_wire] {
                *wire_end = (*wire_end).max(occupied);
            }
        }
    }

    fn into_column_map(mut self, source_columns: usize) -> (Vec<usize>, usize) {
        let columns = source_columns + self.offset;
        self.columns.push(columns);
        (self.columns, columns)
    }
}

fn remap_blocks(blocks: &mut CircuitBlocks, column_map: &[usize]) -> Option<()> {
    let mut remapped = CircuitBlocks::default();
    for block in blocks.iter() {
        if remapped
            .push(
                block.label().to_owned(),
                CircuitColumnIndex::new(column_map[block.start().as_usize()]),
                CircuitColumnIndex::new(column_map[block.end().as_usize()]),
            )
            .is_err()
        {
            return None;
        }
    }
    *blocks = remapped;
    Some(())
}

pub(crate) fn palette_token_to_gate(token: &str) -> Option<(GateKind, Option<ParametricAngle>)> {
    match token_to_gate(token) {
        Some(DecodedToken {
            kind,
            span: 1,
            angle,
            flag: None,
        }) => Some((kind, angle.stored())),
        Some(_) => None,
        None => GateKind::from_url_token(token).map(|kind| (kind, None)),
    }
}

#[derive(Debug, PartialEq)]
struct DecodedToken {
    kind: GateKind,
    span: usize,
    angle: DecodedAngle,
    /// qni measurement variable link (`Measure>a` / `X<a`).
    flag: Option<GateFlag>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DecodedAngle {
    NotParametric,
    Bare,
    Explicit(ParametricAngle),
}

impl DecodedAngle {
    fn from_parsed(kind: GateKind, angle: Option<ParametricAngle>) -> Self {
        match angle {
            Some(angle) => Self::Explicit(angle),
            None => match kind {
                GateKind::Phase | GateKind::Rx | GateKind::Ry | GateKind::Rz => Self::Bare,
                _ => Self::NotParametric,
            },
        }
    }

    fn stored(self) -> Option<ParametricAngle> {
        match self {
            Self::Explicit(angle) => Some(angle),
            Self::NotParametric | Self::Bare => None,
        }
    }
}

fn token_to_gate(token: &str) -> Option<DecodedToken> {
    let (base, flag) = GateFlag::split_token(token)?;
    let (kind, span, angle) = base_token_to_gate(base)?;
    Some(DecodedToken {
        kind,
        span,
        angle: DecodedAngle::from_parsed(kind, angle),
        flag,
    })
}

/// Quirk defines only `Chance` and the canonical decimal suffixes 2 through 16.
fn chance_span(suffix: &str) -> Option<usize> {
    if suffix.is_empty() {
        return Some(1);
    }
    if suffix.starts_with('0') || !suffix.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    suffix.parse().ok().filter(|span| (2..=16).contains(span))
}

fn base_token_to_gate(token: &str) -> Option<(GateKind, usize, Option<ParametricAngle>)> {
    for (prefix, kind) in [("inc", GateKind::Increment), ("dec", GateKind::Decrement)] {
        if let Some(rest) = token.strip_prefix(prefix) {
            if rest.starts_with('0') || !rest.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            let span: usize = rest.parse().ok()?;
            return (1..=16).contains(&span).then_some((kind, span, None));
        }
    }
    if let Some(rest) = token.strip_prefix("QFT†") {
        let span: usize = rest.parse().ok()?;
        return Some((GateKind::QftDaggerGate, span, None));
    }
    if let Some(rest) = token.strip_prefix("QFT") {
        let span: usize = rest.parse().ok()?;
        return Some((GateKind::QftGate, span, None));
    }
    if let Some(rest) = token.strip_prefix("Chance") {
        return Some((GateKind::ProbabilityDisplay, chance_span(rest)?, None));
    }
    if let Some(rest) = token.strip_prefix("Probability") {
        let span = if rest.is_empty() {
            1
        } else {
            rest.parse().ok()?
        };
        return Some((GateKind::ProbabilityDisplay, span.min(16), None));
    }
    if let Some(rest) = token.strip_prefix("Sample") {
        let span: usize = rest.parse().ok()?;
        if !(1..=SAMPLE_MAX_SPAN).contains(&span) {
            return None;
        }
        return Some((GateKind::SampleDisplay, span, None));
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

fn with_gate_ids(
    mut gates: Vec<PlacedGate>,
    blocks: CircuitBlocks,
    title: String,
) -> DecodedCircuit {
    let mut gate_ids = GateIdAllocator::new();
    for gate in &mut gates {
        gate.id = gate_ids.allocate();
    }
    DecodedCircuit {
        gates,
        gate_ids,
        blocks,
        title,
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
    fn adjacent_amplitude_shifts_probability() {
        let circuit = parse_circuit_json(r#"{"cols":[["Amps1"],["Probability"]]}"#);
        assert_eq!(
            circuit
                .gates
                .iter()
                .map(|gate| gate.column.as_usize())
                .collect::<Vec<_>>(),
            vec![0, 2]
        );
    }

    #[test]
    fn chance_decodes_as_single_qubit_probability() {
        assert_eq!(
            base_token_to_gate("Chance"),
            Some((GateKind::ProbabilityDisplay, 1, None))
        );
    }

    #[test]
    fn amplitude_keeps_disjoint_wire_column() {
        let circuit = parse_circuit_json(r#"{"cols":[["Amps1"],[1,"H"]]}"#);
        assert_eq!(
            circuit
                .gates
                .iter()
                .map(|gate| gate.column.as_usize())
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    #[test]
    fn chance_five_decodes() {
        assert_eq!(
            base_token_to_gate("Chance5"),
            Some((GateKind::ProbabilityDisplay, 5, None))
        );
    }

    #[test]
    fn amplitude_padding_keeps_columns() {
        let circuit = parse_circuit_json(r#"{"cols":[["Amps1"],[1],["Probability"]]}"#);
        assert_eq!(
            circuit
                .gates
                .iter()
                .map(|gate| gate.column.as_usize())
                .collect::<Vec<_>>(),
            vec![0, 2]
        );
    }

    #[test]
    fn chance_sixteen_decodes() {
        assert_eq!(
            base_token_to_gate("Chance16"),
            Some((GateKind::ProbabilityDisplay, 16, None))
        );
    }

    #[test]
    fn density_shifts_intersecting_span() {
        let circuit = parse_circuit_json(r#"{"cols":[["Density2"],[1,"H"],["X"]]}"#);
        assert_eq!(
            circuit
                .gates
                .iter()
                .map(|gate| gate.column.as_usize())
                .collect::<Vec<_>>(),
            vec![0, 2, 3]
        );
    }

    #[test]
    fn chance_one_is_ignored() {
        assert_eq!(base_token_to_gate("Chance1"), None);
    }

    #[test]
    fn chance_seventeen_is_ignored() {
        assert_eq!(base_token_to_gate("Chance17"), None);
    }

    #[test]
    fn chance_zero_is_ignored() {
        assert_eq!(base_token_to_gate("Chance0"), None);
    }

    #[test]
    fn chance_leading_zero_is_ignored() {
        assert_eq!(base_token_to_gate("Chance02"), None);
    }

    #[test]
    fn chance_plus_sign_is_ignored() {
        assert_eq!(base_token_to_gate("Chance+2"), None);
    }

    #[test]
    fn chance_zero_one_is_ignored() {
        assert_eq!(base_token_to_gate("Chance01"), None);
    }

    #[test]
    fn chance_non_numeric_suffix_is_ignored() {
        assert_eq!(base_token_to_gate("Chancex"), None);
    }

    #[test]
    fn chance_two_round_trips_as_probability_two() {
        let circuit = parse_circuit_json(r#"{"cols":[["H"],["Chance2"]]}"#);
        assert_eq!(
            crate::url_circuit::circuit_to_json(
                &circuit.gates,
                &circuit.blocks,
                "",
                qubit_count(2)
            ),
            r#"{"cols":[["H"],["Probability2"]]}"#
        );
    }

    #[test]
    fn collision_shifts_whole_column() {
        let circuit = parse_circuit_json(r#"{"cols":[["Amps1"],["Probability","H"],[1,"X"]]}"#);
        assert_eq!(
            circuit
                .gates
                .iter()
                .map(|gate| gate.column.as_usize())
                .collect::<Vec<_>>(),
            vec![0, 2, 2, 3]
        );
    }

    #[test]
    fn clear_lower_wire_shifts_with_the_intersecting_gate() {
        let circuit = parse_circuit_json(r#"{"cols":[[1,"Amps1"],["Amps1","X"],["Z"]]}"#);
        assert_eq!(
            circuit
                .gates
                .iter()
                .map(|gate| gate.column.as_usize())
                .collect::<Vec<_>>(),
            vec![0, 2, 2, 4]
        );
    }

    #[test]
    fn chance_is_accepted_in_palette() {
        assert_eq!(
            palette_token_to_gate("Chance"),
            Some((GateKind::ProbabilityDisplay, None))
        );
    }

    #[test]
    fn amplitude_multiwire_shifts_intersecting_gate() {
        let circuit = parse_circuit_json(r#"{"cols":[["Amps3"],[1,1,"H"]]}"#);
        assert_eq!(
            circuit
                .gates
                .iter()
                .map(|gate| gate.column.as_usize())
                .collect::<Vec<_>>(),
            vec![0, 2]
        );
    }

    #[test]
    fn summary_counts_shifted_columns_and_trailing_empty_columns() {
        let summary =
            summarize_circuit_json(r#"{"cols":[["Amps1"],["Probability"],[1],[1]]}"#).unwrap();
        assert_eq!(summary.columns, 5);
    }

    #[test]
    fn padded_amplitude_round_trips_unchanged() {
        let json = r#"{"cols":[["Amps1"],[1],["Probability"]]}"#;
        assert_eq!(round_trip(json), json);
    }

    #[test]
    fn amplitude_shifts_incoming_intersecting_span() {
        let circuit = parse_circuit_json(r#"{"cols":[[1,"Amps4"],["QFT2"]]}"#);
        assert_eq!(circuit.gates[1].column.as_usize(), 4);
    }

    #[test]
    fn url_payload_shifts_after_wide_display() {
        let circuit = try_decode(r#"{"cols":[["Amps1"],["Probability"]]}"#).unwrap();
        assert_eq!(circuit.gates[1].column.as_usize(), 2);
    }

    #[test]
    fn wide_collision_remaps_block_boundaries() {
        let circuit = parse_circuit_json(
            r#"{"cols":[["{a"],["Amps1"],["}"],["{b"],["Probability"],["H"],["}"]]}"#,
        );
        assert_eq!(
            block_ranges(&circuit),
            vec![("a".to_owned(), 0, 2), ("b".to_owned(), 2, 4)]
        );
    }

    #[test]
    fn chance_two_is_rejected_in_palette() {
        assert_eq!(palette_token_to_gate("Chance2"), None);
    }

    #[test]
    fn chance_measurement_condition_matches_probability() {
        assert_eq!(token_to_gate("Chance2<a"), token_to_gate("Probability2<a"));
    }

    #[test]
    fn sample_and_parity_ids_round_trip() {
        let json = r#"{"cols":[["Sample1"],["Sample2"],["Sample3"],["zpar","xpar","X"]]}"#;
        let circuit = parse_circuit_json(json);
        assert_eq!(
            crate::url_circuit::circuit_to_json(
                &circuit.gates,
                &circuit.blocks,
                "",
                qubit_count(3)
            ),
            json
        );
    }

    #[test]
    fn sample_spans_outside_one_to_eight_are_rejected() {
        assert!(parse_circuit_json(
            r#"{"cols":[["Sample0"],["Sample9"],["Sample16"],["Sample"]]}"#
        )
        .gates
        .is_empty());
    }

    #[test]
    fn sample_span_eight_round_trips() {
        let json = r#"{"cols":[["Sample4"],["Sample8"]]}"#;
        let circuit = parse_circuit_json(json);
        assert_eq!(
            crate::url_circuit::circuit_to_json(
                &circuit.gates,
                &circuit.blocks,
                "",
                qubit_count(8)
            ),
            json
        );
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
                "",
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
        let gates = parse_circuit_json(r#"{"cols":[["QFT0"]]}"#).gates;

        assert_eq!(gates.len(), 0);
    }

    #[test]
    fn qft_span_decodes() {
        let gates = parse_circuit_json(r#"{"cols":[["QFT3"]]}"#).gates;

        assert_eq!(
            gates.first().map(|gate| (gate.kind, gate.span.get())),
            Some((GateKind::QftGate, 3))
        );
    }

    #[test]
    fn probability_span_over_max_clamps_to_sixteen() {
        let gates = parse_circuit_json(r#"{"cols":[["Probability20"]]}"#).gates;

        assert_eq!(
            gates.first().map(|gate| (gate.kind, gate.span.get())),
            Some((GateKind::ProbabilityDisplay, 16))
        );
    }

    #[test]
    fn probability_zero_span_is_ignored() {
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
            crate::url_circuit::circuit_to_json(
                &circuit.gates,
                &circuit.blocks,
                "",
                qubit_count(1)
            ),
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
    fn bare_phase_token_is_bare_not_explicit() {
        assert_eq!(
            token_to_gate("P").map(|token| token.angle),
            Some(DecodedAngle::Bare)
        );
    }

    #[test]
    fn bare_rx_token_is_bare_not_explicit() {
        assert_eq!(
            token_to_gate("Rx").map(|token| token.angle),
            Some(DecodedAngle::Bare)
        );
    }

    #[test]
    fn bare_ry_token_is_bare_not_explicit() {
        assert_eq!(
            token_to_gate("Ry").map(|token| token.angle),
            Some(DecodedAngle::Bare)
        );
    }

    #[test]
    fn bare_rz_token_is_bare_not_explicit() {
        assert_eq!(
            token_to_gate("Rz").map(|token| token.angle),
            Some(DecodedAngle::Bare)
        );
    }

    #[test]
    fn hadamard_token_is_not_parametric() {
        assert_eq!(
            token_to_gate("H").map(|token| token.angle),
            Some(DecodedAngle::NotParametric)
        );
    }

    #[test]
    fn explicit_phase_token_keeps_parsed_angle() {
        assert_eq!(
            token_to_gate("P(π/2)").map(|token| token.angle),
            Some(DecodedAngle::Explicit(ParametricAngle::default()))
        );
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

    #[test]
    fn z_detect_control_reset_round_trips() {
        let json = r#"{"cols":[["H"],["ZDetectControlReset","X"]]}"#;
        assert_eq!(round_trip(json), json);
    }

    #[test]
    fn bell_sample_json_is_canonical() {
        let json = qni_web_circuit_library_model::BELL_JSON;
        assert_eq!(round_trip(json), json);
    }

    #[test]
    fn ghz_sample_json_is_canonical() {
        let json = qni_web_circuit_library_model::GHZ_JSON;
        assert_eq!(round_trip(json), json);
    }

    #[test]
    fn teleportation_sample_json_is_canonical() {
        let json = qni_web_circuit_library_model::TELEPORTATION_JSON;
        assert_eq!(round_trip(json), json);
    }

    #[test]
    fn superdense_coding_sample_json_is_canonical() {
        let json = qni_web_circuit_library_model::SUPERDENSE_CODING_JSON;
        assert_eq!(round_trip(json), json);
    }

    #[test]
    fn qft4_sample_json_is_canonical() {
        let json = qni_web_circuit_library_model::QFT4_DECOMPOSED_JSON;
        assert_eq!(round_trip(json), json);
    }

    #[test]
    fn symmetry_breaking_sample_json_is_canonical() {
        let json = qni_web_circuit_library_model::SYMMETRY_BREAKING_JSON;
        assert_eq!(round_trip(json), json);
    }

    #[test]
    fn grover_search_sample_json_is_canonical() {
        let json = qni_web_circuit_library_model::GROVER_SEARCH_JSON;
        assert_eq!(round_trip(json), json);
    }

    #[test]
    fn reversible_addition_sample_json_is_canonical() {
        let json = qni_web_circuit_library_model::REVERSIBLE_ADDITION_JSON;
        assert_eq!(round_trip(json), json);
    }

    #[test]
    fn delayed_choice_eraser_sample_json_is_canonical() {
        let json = qni_web_circuit_library_model::DELAYED_CHOICE_ERASER_JSON;
        assert_eq!(round_trip(json), json);
    }

    fn round_trip(json: &str) -> String {
        let circuit = parse_circuit_json(json);
        let qubits = qubit_count_from_gates(&circuit.gates).max(1);
        crate::url_circuit::circuit_to_json(
            &circuit.gates,
            &circuit.blocks,
            &circuit.title,
            qubit_count(qubits),
        )
    }

    #[test]
    fn x_detector_json_round_trip() {
        let json = r#"{"cols":[["XDetector"]]}"#;
        assert_eq!(round_trip(json), json);
    }

    #[test]
    fn x_detector_url_round_trip() {
        let circuit = try_decode(r#"circuit={"cols":[["XDetector"]]}"#).unwrap();
        assert_eq!(
            crate::url_circuit::circuit_to_json(
                &circuit.gates,
                &circuit.blocks,
                &circuit.title,
                qubit_count(1)
            ),
            r#"{"cols":[["XDetector"]]}"#
        );
    }

    #[test]
    fn y_detector_json_round_trip() {
        let json = r#"{"cols":[["YDetector"]]}"#;
        assert_eq!(round_trip(json), json);
    }

    #[test]
    fn y_detector_url_round_trip() {
        let circuit = try_decode(r#"circuit={"cols":[["YDetector"]]}"#).unwrap();
        assert_eq!(
            crate::url_circuit::circuit_to_json(
                &circuit.gates,
                &circuit.blocks,
                &circuit.title,
                qubit_count(1)
            ),
            r#"{"cols":[["YDetector"]]}"#
        );
    }

    #[test]
    fn title_round_trips_after_cols() {
        assert_eq!(
            round_trip(r#"{"title":"  Superdense Coding ","cols":[["|0>"]]}"#),
            r#"{"cols":[["|0>"]],"title":"Superdense Coding"}"#
        );
    }

    #[test]
    fn title_of_empty_circuit_round_trips() {
        assert_eq!(
            round_trip(r#"{"cols":[],"title":"Bell"}"#),
            r#"{"cols":[],"title":"Bell"}"#
        );
    }

    #[test]
    fn blank_title_is_dropped() {
        assert_eq!(
            round_trip(r#"{"cols":[["H"]],"title":"   "}"#),
            r#"{"cols":[["H"]]}"#
        );
    }

    #[test]
    fn title_with_quote_and_newline_round_trips_as_valid_json() {
        assert_eq!(
            round_trip(r#"{"cols":[],"title":"a \"b\"\nc"}"#),
            r#"{"cols":[],"title":"a \"b\"\nc"}"#
        );
    }

    #[test]
    fn titled_circuit_keeps_its_gates() {
        assert_eq!(
            parse_circuit_json(r#"{"cols":[["|0>"]],"title":"Superdense Coding"}"#)
                .gates
                .len(),
            1
        );
    }

    #[test]
    fn titled_circuit_with_unknown_root_key_is_rejected() {
        assert_eq!(
            summarize_circuit_json(r#"{"cols":[["H"]],"title":"Bell","mode":"gpu"}"#),
            None
        );
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
    fn ordered_block_closes_drop_only_empty_ranges() {
        assert_eq!(first_ordered_block_mismatch(), None);
    }

    #[test]
    fn empty_block_range_is_not_a_push_failure() {
        let mut blocks = CircuitBlocks::default();
        assert_eq!(
            push_omitting_empty(
                &mut blocks,
                "a".to_owned(),
                CircuitColumnIndex::new(2),
                CircuitColumnIndex::new(2),
            ),
            Some(())
        );
    }

    #[test]
    fn overlapping_block_range_is_a_push_failure() {
        let mut blocks = CircuitBlocks::default();
        accept_block(&mut blocks, "a", 0, 2);
        assert_eq!(
            push_omitting_empty(
                &mut blocks,
                "b".to_owned(),
                CircuitColumnIndex::new(1),
                CircuitColumnIndex::new(3),
            ),
            None
        );
    }

    fn accept_block(blocks: &mut CircuitBlocks, label: &str, start: usize, end: usize) {
        push_omitting_empty(
            blocks,
            label.to_owned(),
            CircuitColumnIndex::new(start),
            CircuitColumnIndex::new(end),
        )
        .expect("setup range is accepted");
    }

    fn first_ordered_block_mismatch() -> Option<String> {
        let mut checked = 0usize;
        for len in 0..=7 {
            let total = 3usize.pow(u32::try_from(len).expect("script length fits u32"));
            for index in 0..total {
                let ops = block_script(index, len);
                let Some(expected) = expected_ordered_blocks(&ops) else {
                    continue;
                };
                checked += 1;
                let json = block_script_json(&ops);
                let actual = block_ranges(&parse_circuit_json(&json));
                if actual != expected {
                    return Some(format!("{json} => {actual:?} != {expected:?}"));
                }
            }
        }
        (checked == 0).then(|| "no sequence checked".to_owned())
    }

    fn block_script(index: usize, len: usize) -> Vec<u8> {
        let mut index = index;
        let mut ops = Vec::with_capacity(len);
        for _ in 0..len {
            ops.push(b"GOC"[index % 3]);
            index /= 3;
        }
        ops
    }

    fn block_script_json(ops: &[u8]) -> String {
        let mut cols = Vec::new();
        let mut opened = 0usize;
        for op in ops {
            match op {
                b'G' => cols.push(r#"["H"]"#.to_string()),
                b'O' => {
                    cols.push(format!(r#"["{{b{opened}"]"#));
                    opened += 1;
                }
                b'C' => cols.push(r#"["}"]"#.to_string()),
                _ => unreachable!("block script only uses G, O, and C"),
            }
        }
        format!(r#"{{"cols":[{}]}}"#, cols.join(","))
    }

    fn expected_ordered_blocks(ops: &[u8]) -> Option<Vec<(String, usize, usize)>> {
        let mut columns = 0usize;
        let mut open: Option<(String, usize)> = None;
        let mut blocks = Vec::new();
        let mut opened = 0usize;
        for op in ops {
            match op {
                b'G' => columns += 1,
                b'O' => {
                    if open.is_some() {
                        return None;
                    }
                    open = Some((format!("b{opened}"), columns));
                    opened += 1;
                }
                b'C' => {
                    let (label, start) = open.take()?;
                    if start < columns {
                        blocks.push((label, start, columns));
                    }
                }
                _ => unreachable!("block script only uses G, O, and C"),
            }
        }
        if let Some((label, start)) = open {
            if start < columns {
                blocks.push((label, start, columns));
            }
        }
        Some(blocks)
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
    fn web_embed_doc_block_example_round_trips_byte_for_byte() {
        let json = r#"{"cols":[["{量子もつれ"],["H"],["•","X"],["}"]]}"#;

        assert_eq!(round_trip(json), json);
    }

    fn label_round_trip(label: &str) -> Option<String> {
        let mut blocks = crate::app::CircuitBlocks::default();
        blocks
            .push(
                label.to_owned(),
                crate::app::CircuitColumnIndex::new(0),
                crate::app::CircuitColumnIndex::new(1),
            )
            .expect("one block is valid");
        let circuit = parse_circuit_json(r#"{"cols":[["H"]]}"#);
        let json = crate::url_circuit::circuit_to_json(&circuit.gates, &blocks, "", qubit_count(1));
        parse_circuit_json(&json)
            .blocks
            .iter()
            .next()
            .map(|block| block.label().to_owned())
    }

    #[test]
    fn block_label_with_quote_and_backslash_round_trips() {
        assert_eq!(
            label_round_trip(r#"say "a" \ b"#).as_deref(),
            Some(r#"say "a" \ b"#)
        );
    }

    #[test]
    fn block_label_with_japanese_and_emoji_round_trips() {
        assert_eq!(
            label_round_trip("量子もつれ 🎉").as_deref(),
            Some("量子もつれ 🎉")
        );
    }

    #[test]
    fn block_label_with_braces_round_trips() {
        assert_eq!(label_round_trip("{a}b").as_deref(), Some("{a}b"));
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

    #[test]
    fn arithmetic_suffix_families_round_trip() {
        let columns = (1..=16)
            .flat_map(|span| [format!(r#"["inc{span}"]"#), format!(r#"["dec{span}"]"#)])
            .collect::<Vec<_>>()
            .join(",");
        let json = format!(r#"{{"cols":[{columns}]}}"#);
        let circuit = parse_circuit_json(&json);
        assert_eq!(
            crate::url_circuit::circuit_to_json(
                &circuit.gates,
                &circuit.blocks,
                "",
                qubit_count(16)
            ),
            json
        );
    }

    #[test]
    fn arithmetic_requires_valid_span_suffix() {
        assert_eq!(
            ["inc", "dec", "inc0", "dec0", "inc17", "dec17", "inc-1", "decx", "inc01", "dec+1"]
                .map(base_token_to_gate),
            [None; 10]
        );
    }

    #[test]
    fn arithmetic_decodes_gate_kinds_and_spans() {
        assert_eq!(
            ["inc1", "inc16", "dec1", "dec16"].map(base_token_to_gate),
            [
                Some((GateKind::Increment, 1, None)),
                Some((GateKind::Increment, 16, None)),
                Some((GateKind::Decrement, 1, None)),
                Some((GateKind::Decrement, 16, None)),
            ]
        );
    }

    #[test]
    fn arithmetic_palette_templates_use_single_wire_defaults() {
        assert_eq!(
            ["inc", "dec", "inc1", "dec1"].map(palette_token_to_gate),
            [
                Some((GateKind::Increment, None)),
                Some((GateKind::Decrement, None)),
                Some((GateKind::Increment, None)),
                Some((GateKind::Decrement, None)),
            ]
        );
    }

    #[test]
    fn arithmetic_palette_rejects_multiwire_templates() {
        assert_eq!(["inc2", "dec16"].map(palette_token_to_gate), [None, None]);
    }
}
