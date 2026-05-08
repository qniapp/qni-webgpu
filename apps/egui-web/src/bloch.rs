//! Linearises placed gates into the GPU op stream consumed by `gpu.rs`.
//! No quantum math runs here — the column-by-column walk only decides which
//! WGSL dispatches to issue (and in what order). Per AGENTS.md the simulation
//! is GPU-only; this module is purely an orchestration helper.
//!
//! qni references for the per-column semantics:
//! - `packages/simulator/src/simulator.ts:runStep` — controls + targets in
//!   each step, then any measurement / display readouts on the post-step
//!   state.
//! - `packages/simulator/src/state-vector.ts` and `matrix.ts` — the math each
//!   shader implements (kept in `gpu.rs`).

use std::collections::HashMap;

use crate::app::PlacedGate;
use crate::constants::{GATE_SIZE, SNAP_DISTANCE};
use crate::gates::{gate_params, gate_params_controlled, GateKind, GateParams};
use crate::layout::{nearest_slot_index, LayoutMetrics};

/// Bloch vector for a single qubit, in qni's convention. The values are
/// always produced by `BLOCH_REDUCE_SHADER`; this alias just names the shape
/// of the cached read-back triple.
pub(crate) type BlochVector = [f32; 3];

/// One step the GPU dispatcher should run during a recompute.
///   * `ApplyGate`: unitary / write gate via `STATE_COMPUTE_SHADER`.
///   * `CaptureBloch`: per-qubit reduction (Bloch x, y, z) via
///     `BLOCH_REDUCE_SHADER`.
///   * `MeasureReduceSample`: pZero reduction + deterministic PCG sample,
///     writes `(pZero, r, outcome, sqrt_p_kept)` to the measurement aux
///     buffer (`MEASURE_REDUCE_SHADER`).
///   * `MeasureCollapse`: per-pair zero+normalize using the previously
///     written aux slot (`MEASURE_COLLAPSE_SHADER`).
#[derive(Clone, Copy, Debug)]
pub(crate) enum SimulationOp {
    ApplyGate(GateParams),
    CaptureBloch {
        gate_id: u32,
        qubit_bit: u32,
        output_slot: u32,
    },
    MeasureReduceSample {
        gate_id: u32,
        qubit_bit: u32,
        output_slot: u32,
    },
    MeasureCollapse {
        qubit_bit: u32,
        aux_slot: u32,
    },
}

/// Walks placed gates column by column and emits ops in the exact order the
/// GPU should run them. Non-mutating decoration (Spacer / Swap) is dropped.
/// Within each column the order is: column unitaries / writes → measurements
/// (reduce-sample then collapse) → bloch captures, mirroring qni's
/// `simulator.ts:runStep` semantics — bloch reads the post-collapse state.
pub(crate) fn linearize_ops(
    placed_gates: &[PlacedGate],
    qubits: usize,
    metrics: &LayoutMetrics,
) -> Vec<SimulationOp> {
    if qubits == 0 || metrics.slot_centers.is_empty() {
        return Vec::new();
    }
    let state_count = (1u32 << qubits) as u32;

    let mut by_slot: HashMap<usize, Vec<&PlacedGate>> = HashMap::new();
    for gate in placed_gates {
        let center_x = gate.pos.x + GATE_SIZE / 2.0;
        let Some((slot, distance)) = nearest_slot_index(center_x, &metrics.slot_centers) else {
            continue;
        };
        if distance > SNAP_DISTANCE {
            continue;
        }
        by_slot.entry(slot).or_default().push(gate);
    }

    let mut slot_indices: Vec<usize> = by_slot.keys().copied().collect();
    slot_indices.sort();

    let mut ops: Vec<SimulationOp> = Vec::new();
    let mut bloch_slot: u32 = 0;
    let mut measurement_slot: u32 = 0;
    for slot in slot_indices {
        let column = by_slot.get(&slot).expect("slot exists");
        let mut control_mask = 0u32;
        let mut control_value = 0u32;
        let mut targets: Vec<&PlacedGate> = Vec::new();
        let mut bloch_targets: Vec<&PlacedGate> = Vec::new();
        let mut measurement_targets: Vec<&PlacedGate> = Vec::new();

        for gate in column {
            if gate.wire >= qubits {
                continue;
            }
            let bit = (qubits - 1 - gate.wire) as u32;
            let bit_mask = 1u32 << bit;
            match gate.kind {
                GateKind::Control => {
                    control_mask |= bit_mask;
                    control_value |= bit_mask;
                }
                GateKind::AntiControl => {
                    control_mask |= bit_mask;
                }
                GateKind::Swap | GateKind::Spacer => {
                    // Non-mutating decoration.
                }
                GateKind::Measurement => measurement_targets.push(gate),
                GateKind::BlochDisplay => bloch_targets.push(gate),
                _ => targets.push(gate),
            }
        }

        targets.sort_by(|a, b| a.id.cmp(&b.id));
        for target in &targets {
            let bit = (qubits - 1 - target.wire) as u32;
            let params = if control_mask == 0 {
                gate_params(target.kind, bit, state_count)
            } else {
                gate_params_controlled(target.kind, bit, control_mask, control_value, state_count)
            };
            ops.push(SimulationOp::ApplyGate(params));
        }

        // Measurements run after the column's unitaries: reduce + sample,
        // then collapse. Each consumes one aux slot.
        measurement_targets.sort_by(|a, b| a.id.cmp(&b.id));
        for measurement in &measurement_targets {
            let qubit_bit = (qubits - 1 - measurement.wire) as u32;
            ops.push(SimulationOp::MeasureReduceSample {
                gate_id: measurement.id,
                qubit_bit,
                output_slot: measurement_slot,
            });
            ops.push(SimulationOp::MeasureCollapse {
                qubit_bit,
                aux_slot: measurement_slot,
            });
            measurement_slot += 1;
        }

        // Bloch captures see the post-measurement state.
        bloch_targets.sort_by(|a, b| a.id.cmp(&b.id));
        for display in &bloch_targets {
            let qubit_bit = (qubits - 1 - display.wire) as u32;
            ops.push(SimulationOp::CaptureBloch {
                gate_id: display.id,
                qubit_bit,
                output_slot: bloch_slot,
            });
            bloch_slot += 1;
        }
    }
    ops
}
