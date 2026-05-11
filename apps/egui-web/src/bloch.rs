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
use crate::gates::{
    controlled_phase_params, gate_params, gate_params_controlled, GateKind, GateParams,
};
use crate::layout::{nearest_slot_index, LayoutMetrics};

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
/// `step_limit`: inclusive column index up to which to apply gates.
/// `None` = apply everything (= final state). `Some(k)` truncates the
/// linearisation after column k, so the GPU only runs the dispatches
/// for slots `0..=k` — this powers the per-step state preview.
pub(crate) fn linearize_ops(
    placed_gates: &[PlacedGate],
    qubits: usize,
    metrics: &LayoutMetrics,
    step_limit: Option<usize>,
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
    if let Some(limit) = step_limit {
        slot_indices.retain(|&slot| slot <= limit);
    }

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

        let mut qft_gates: Vec<&PlacedGate> = Vec::new();
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
                GateKind::QftGate | GateKind::QftDaggerGate => qft_gates.push(gate),
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

        // Controls-only step ⇒ multi-controlled-Z (CZ / CCZ / …).
        // Mirrors qni's `circuit-step-element.ts:1303-1310`: a step with
        // ≥ 2 `ControlGateElement`s and no controllable target emits a
        // single `'•'` operation whose first wire becomes the Z target
        // and whose remaining wires become the controls. AntiControls
        // and lone single controls stay no-ops (qni :1312-1316 and the
        // `< 2` guard at :1306).
        //
        // `control_value` bits = Controls only (AntiControls live in
        // `control_mask & !control_value`). We pick the topmost wire
        // (= the highest set bit of `control_value`, since our bit
        // numbering runs qubits-1..0 top→bottom) as the Z target so the
        // dispatch matches qni's "first in target list" convention. CZ
        // is physically symmetric so the choice only affects the
        // GateParams shape, not the resulting state.
        if targets.is_empty()
            && qft_gates.is_empty()
            && measurement_targets.is_empty()
            && bloch_targets.is_empty()
            && control_value.count_ones() >= 2
        {
            let target_bit = 31 - control_value.leading_zeros();
            let target_bit_mask = 1u32 << target_bit;
            let cz_control_value = control_value & !target_bit_mask;
            // Match qni and drop anti-control bits entirely: only the
            // remaining `Control` wires gate the Z. (Mixing anti-control
            // bits into the mask would change the semantics from qni's
            // `{type:'•', targets:[control bits]}` form.)
            let cz_control_mask = cz_control_value;
            let params = gate_params_controlled(
                GateKind::Z,
                target_bit,
                cz_control_mask,
                cz_control_value,
                state_count,
            );
            ops.push(SimulationOp::ApplyGate(params));
        }

        // QFT-family gates expand to their textbook decomposition (H +
        // controlled-phase rotations). Column controls are ignored here
        // — qni's simulator doesn't model controlled-QFT either.
        qft_gates.sort_by(|a, b| a.id.cmp(&b.id));
        for qft in &qft_gates {
            let dagger = qft.kind == GateKind::QftDaggerGate;
            ops.extend(linearize_qft(qft, qubits, state_count, dagger));
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

/// Expand a placed QFT (or QFT†) gate into its textbook decomposition:
/// `span` Hadamards interleaved with controlled-phase rotations
/// `R_k = diag(1, exp(iπ/2^j))`. Translated 1:1 from qni's
/// `simulator.ts::qftSingleTargetBit` / `qftDaggerSingleTargetBit`.
/// No final bit-reversal SWAPs — qni's simulator skips them too, so
/// the output is in bit-reversed order, but this matches qni exactly.
///
/// Wire-to-bit mapping: `gate.wire + idx` (wire index of the i-th
/// qubit in the QFT register) → `bit = qubits − 1 − wire` (the
/// simulator convention where the top wire is the MSB).
fn linearize_qft(
    gate: &PlacedGate,
    qubits: usize,
    state_count: u32,
    dagger: bool,
) -> Vec<SimulationOp> {
    if gate.wire >= qubits {
        return Vec::new();
    }
    // Clamp the span so the QFT never reaches past the qubit register;
    // a user-resized QFT can momentarily extend beyond the placed
    // bottom wire before `update_qubit_count` catches up.
    let span = gate.span.max(1).min(qubits - gate.wire);
    if span == 0 {
        return Vec::new();
    }
    let bit_of = |idx: usize| (qubits - 1 - gate.wire - idx) as u32;
    let mut ops = Vec::new();

    if !dagger {
        // QFT: for i in 0..span: H(i), then controlled-Phase π/2^j with
        // control=(i+j) for j in 1..span-i.
        for i in 0..span {
            ops.push(SimulationOp::ApplyGate(gate_params(
                GateKind::H,
                bit_of(i),
                state_count,
            )));
            for j in 1..(span - i) {
                let phase = std::f32::consts::PI / (1u32 << j) as f32;
                ops.push(SimulationOp::ApplyGate(controlled_phase_params(
                    bit_of(i),
                    bit_of(i + j),
                    phase,
                    state_count,
                )));
            }
        }
    } else {
        // QFT†: reverse loop, negated phases, H *after* the phases.
        for i in (0..span).rev() {
            for j in (1..(span - i)).rev() {
                let phase = -std::f32::consts::PI / (1u32 << j) as f32;
                ops.push(SimulationOp::ApplyGate(controlled_phase_params(
                    bit_of(i),
                    bit_of(i + j),
                    phase,
                    state_count,
                )));
            }
            ops.push(SimulationOp::ApplyGate(gate_params(
                GateKind::H,
                bit_of(i),
                state_count,
            )));
        }
    }
    ops
}
