use super::{linearize_ops, SimulationOp};
use crate::{qubit_count::QubitCount, url_circuit::parse_circuit_json};

fn params(json: &str, qubits: usize) -> Vec<(u32, u32, u32, u32, u32, u32)> {
    let parsed = parse_circuit_json(json);
    linearize_ops(&parsed.gates, QubitCount::try_new(qubits).unwrap(), 0)
        .into_iter()
        .filter_map(|op| match op {
            SimulationOp::Arithmetic(p) => Some((
                p.state_count,
                p.target_shift,
                p.target_mask,
                p.amount,
                p.control_mask,
                p.control_value,
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn increment_uses_one_permutation_op() {
    assert_eq!(
        params(r#"{"cols":[["inc3"]]}"#, 3),
        vec![(8, 0, 7, 1, 0, 0)]
    );
}

#[test]
fn decrement_encodes_modular_negative_amount() {
    assert_eq!(
        params(r#"{"cols":[[1,"dec2"]]}"#, 3),
        vec![(8, 1, 3, u32::MAX, 0, 0)]
    );
}

#[test]
fn arithmetic_excludes_internal_controls() {
    assert_eq!(
        params(r#"{"cols":[["•","inc3","◦","•","◦"]]}"#, 5),
        vec![(32, 1, 7, 1, 17, 1)]
    );
}

#[test]
fn arithmetic_excludes_internal_parity_anchors() {
    assert_eq!(
        params(r#"{"cols":[["inc3","zpar","xpar"]]}"#, 3),
        vec![(8, 0, 7, 1, 0, 0)]
    );
}

#[test]
fn arithmetic_uses_external_parity_anchor() {
    assert_eq!(
        params(r#"{"cols":[["inc2",1,"zpar","xpar"]]}"#, 4),
        vec![(16, 0, 3, 1, 4, 4)]
    );
}

#[test]
fn arithmetic_span_clamps_to_register() {
    assert_eq!(
        params(r#"{"cols":[[1,"inc16"]]}"#, 3),
        vec![(8, 1, 3, 1, 0, 0)]
    );
}

#[test]
fn register_gate_order_does_not_depend_on_editor_insertion_ids() {
    let mut parsed = parse_circuit_json(r#"{"cols":[["inc1","QFT1"]]}"#);
    parsed.gates[0].id = crate::app::GateId::from_u32(20);
    parsed.gates[1].id = crate::app::GateId::from_u32(10);
    let ops = linearize_ops(&parsed.gates, QubitCount::try_new(2).unwrap(), 0);
    assert!(matches!(ops.first(), Some(SimulationOp::Arithmetic(_))));
}

#[test]
fn arithmetic_runs_after_ordinary_targets_before_qft_in_row_order() {
    let parsed = parse_circuit_json(r#"{"cols":[["inc1","X","QFT1"]]}"#);
    let kinds: Vec<_> = linearize_ops(&parsed.gates, QubitCount::try_new(3).unwrap(), 0)
        .iter()
        .map(|op| match op {
            SimulationOp::ApplyGate(_) => "matrix",
            SimulationOp::Arithmetic(_) => "arithmetic",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, ["matrix", "arithmetic", "matrix"]);
}
