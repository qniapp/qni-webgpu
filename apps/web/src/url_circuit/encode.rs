//! Circuit JSON encoder (`PlacedGate` → `{"cols":[...]}`).

use crate::app::PlacedGate;
use crate::gates::GateKind;

use super::EMPTY_CIRCUIT_JSON;

/// Serialise the circuit to qni's `{"cols":[...]}` JSON shape. Returns
/// `EMPTY_CIRCUIT_JSON` if there are no gates.
///
/// Each entry of `cols` is one column; each column is an array indexed
/// by wire (qubit) number, with `1` for an empty wire and the gate's
/// token otherwise. Trailing `1`s in a column are stripped to match
/// Quirk / qni's compact JSON; a column with no gates at all (which
/// shouldn't happen post-`compact_empty_steps`) becomes `[1]`.
pub(crate) fn circuit_to_json(placed_gates: &[PlacedGate], qubit_count: usize) -> String {
    if placed_gates.is_empty() {
        return EMPTY_CIRCUIT_JSON.to_string();
    }
    format!(
        r#"{{"cols":{}}}"#,
        circuit_columns_to_json(placed_gates, qubit_count)
    )
}

pub(crate) fn circuit_columns_to_json(placed_gates: &[PlacedGate], qubit_count: usize) -> String {
    if placed_gates.is_empty() {
        return "[]".to_string();
    }
    // Bucket gates by semantic column. After `compact_empty_steps` the
    // occupied indices are dense from 0..N-1, but be defensive in case this
    // is ever called pre-compaction.
    let max_slot = placed_gates
        .iter()
        .map(|gate| gate.column)
        .max()
        .unwrap_or(0);
    let mut buckets: Vec<Vec<&PlacedGate>> = vec![Vec::new(); max_slot + 1];
    for gate in placed_gates {
        buckets[gate.column].push(gate);
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
    format!("[{}]", cols.join(","))
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
    let spec = kind.spec();
    let s = match kind {
        GateKind::Phase | GateKind::Rx | GateKind::Ry | GateKind::Rz => {
            format_parametric(spec.url_token, angle)
        }
        GateKind::QftGate | GateKind::QftDaggerGate => {
            format!("{}{}", spec.url_token, span.max(1))
        }
        GateKind::ProbabilityDisplay => {
            let span = span.clamp(1, 16);
            if span == 1 {
                spec.url_token.to_string()
            } else {
                format!("{}{}", spec.url_token, span)
            }
        }
        GateKind::AmplitudeDisplay => format!("{}{}", spec.url_token, span.clamp(1, 16)),
        GateKind::DensityMatrixDisplay => {
            let span = span.clamp(1, 8);
            if span == 1 {
                spec.url_token.to_string()
            } else {
                format!("{}{}", spec.url_token, span)
            }
        }
        _ => spec.url_token.to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::PlacedGate;

    #[test]
    fn amplitude_span_one_serializes_with_suffix() {
        let gate = PlacedGate::new(1, GateKind::AmplitudeDisplay, 0, 0, 1, None);

        assert_eq!(circuit_to_json(&[gate], 1), r#"{"cols":[["Amps1"]]}"#);
    }

    #[test]
    fn amplitude_span_sixteen_serializes_with_suffix() {
        let gate = PlacedGate::new(1, GateKind::AmplitudeDisplay, 0, 0, 16, None);

        assert_eq!(circuit_to_json(&[gate], 16), r#"{"cols":[["Amps16"]]}"#);
    }

    #[test]
    fn density_span_one_serializes_without_suffix() {
        let gate = PlacedGate::new(1, GateKind::DensityMatrixDisplay, 0, 0, 1, None);

        assert_eq!(circuit_to_json(&[gate], 1), r#"{"cols":[["Density"]]}"#);
    }

    #[test]
    fn density_span_eight_serializes_with_suffix() {
        let gate = PlacedGate::new(1, GateKind::DensityMatrixDisplay, 0, 0, 8, None);

        assert_eq!(circuit_to_json(&[gate], 8), r#"{"cols":[["Density8"]]}"#);
    }
}
