//! Column-by-column lowering from editor gates into GPU dispatch ops.
//!
//! qni references for the per-column semantics:
//! - `packages/simulator/src/simulator.ts:runStep` — controls + targets in
//!   each step, then any measurement / display readouts on the post-step
//!   state.
//! - `packages/simulator/src/state-vector.ts` and `matrix.ts` — the math each
//!   shader implements (kept in `gpu/*`).

use super::{ColumnAnalysis, SimulationOp};
use crate::app::PlacedGate;
use crate::gates::{
    controlled_phase_params, gate_params, gate_params_controlled, parse_angle_radians,
    phase_params, rx_params, ry_params, rz_params, GateKind,
};

/// Walks placed gates column by column and emits ops in the exact order the
/// GPU should run them. Non-mutating decoration (Spacer / Swap) is dropped.
/// Within each column the order is: column unitaries / writes → measurements
/// (reduce-sample then collapse) → bloch captures, mirroring qni's
/// `simulator.ts:runStep` semantics — bloch reads the post-collapse state.
/// `snapshot_slot_count`: number of semantic step snapshots to cache for the
/// state panel. Slot `k` stores the state after circuit step `k`; empty steps
/// copy the previous state. Hovering later switches to the cached slot via GPU
/// copy only, matching qni's worker-side per-step result cache.
pub(crate) fn linearize_ops(
    placed_gates: &[PlacedGate],
    qubits: usize,
    snapshot_slot_count: usize,
) -> Vec<SimulationOp> {
    if qubits == 0 {
        return Vec::new();
    }
    let state_count = 1u32 << qubits;

    let analysis = ColumnAnalysis::from_gates(placed_gates, |gate| {
        if gate.wire >= qubits {
            return None;
        }
        Some(gate.column)
    });

    let mut ops: Vec<SimulationOp> = Vec::new();
    let mut next_snapshot_slot = 0usize;
    let mut bloch_slot: u32 = 0;
    let mut measurement_slot: u32 = 0;
    let mut probability_slot: u32 = 0;
    let mut amplitude_slot: u32 = 0;
    let mut density_slot: u32 = 0;
    for column in analysis.columns() {
        while next_snapshot_slot < column.slot && next_snapshot_slot < snapshot_slot_count {
            ops.push(SimulationOp::SnapshotState {
                output_slot: next_snapshot_slot as u32,
            });
            next_snapshot_slot += 1;
        }
        let column_gates = column.gates();
        let mut control_mask = 0u32;
        let mut control_value = 0u32;
        let mut targets: Vec<&PlacedGate> = Vec::new();
        let mut bloch_targets: Vec<&PlacedGate> = Vec::new();
        let mut measurement_targets: Vec<&PlacedGate> = Vec::new();
        let mut probability_targets: Vec<&PlacedGate> = Vec::new();
        let mut amplitude_targets: Vec<&PlacedGate> = Vec::new();
        let mut density_targets: Vec<&PlacedGate> = Vec::new();
        let mut swap_targets: Vec<&PlacedGate> = Vec::new();

        let mut qft_gates: Vec<&PlacedGate> = Vec::new();
        for gate in column_gates {
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
                GateKind::Swap => swap_targets.push(gate),
                GateKind::Spacer => {
                    // Non-mutating decoration.
                }
                GateKind::Measurement => measurement_targets.push(gate),
                GateKind::BlochDisplay => bloch_targets.push(gate),
                GateKind::ProbabilityDisplay => probability_targets.push(gate),
                GateKind::AmplitudeDisplay => amplitude_targets.push(gate),
                GateKind::DensityMatrixDisplay => density_targets.push(gate),
                GateKind::QftGate | GateKind::QftDaggerGate => qft_gates.push(gate),
                _ => targets.push(gate),
            }
        }

        // Swap step → 3-CNOT decomposition. Mirrors qni's
        // `simulator.ts::swap` (`packages/simulator/src/simulator.ts:296`):
        //   X target1 controls=[target0]
        //   X target0 controls=[target1]
        //   X target1 controls=[target0]
        // Any column-level controls (Fredkin / controlled-SWAP) ride
        // along — they're OR'd into each CX's control mask the same way
        // qni does via `controlOptions.controls.concat(...)`.
        //
        // Two-target swap only; if the user dropped a single Swap with
        // no partner, or three+ Swaps in one column, the column is
        // skipped (qni dispatches only the first two `targets` and
        // disables stray swaps via `updateSwapConnections`).
        swap_targets.sort_by(|a, b| a.id.cmp(&b.id));
        if swap_targets.len() == 2 {
            let bit_a = (qubits - 1 - swap_targets[0].wire) as u32;
            let bit_b = (qubits - 1 - swap_targets[1].wire) as u32;
            let mask_a = 1u32 << bit_a;
            let mask_b = 1u32 << bit_b;
            let cx_a_to_b = gate_params_controlled(
                GateKind::X,
                bit_b,
                control_mask | mask_a,
                control_value | mask_a,
                state_count,
            );
            let cx_b_to_a = gate_params_controlled(
                GateKind::X,
                bit_a,
                control_mask | mask_b,
                control_value | mask_b,
                state_count,
            );
            ops.push(SimulationOp::ApplyGate(cx_a_to_b));
            ops.push(SimulationOp::ApplyGate(cx_b_to_a));
            ops.push(SimulationOp::ApplyGate(cx_a_to_b));
        }

        targets.sort_by(|a, b| a.id.cmp(&b.id));
        for target in &targets {
            let bit = (qubits - 1 - target.wire) as u32;
            // Parametric gates carry an optional angle string ("π/2",
            // "-π/128", …). When present we route through the matching
            // `*_params(θ, …)` builder so the matrix carries the parsed
            // angle; when absent we fall back to the editor's
            // pre-parametric π/2 default (the gate's hard-coded matrix
            // in `gate_matrix`). qni would instead error out at
            // simulate time for a bare `P` / `Rx` / `Ry` / `Rz`.
            type ParametricBuilder = fn(f32, u32, u32, u32, u32) -> crate::gates::GateParams;
            let parametric_builder: Option<ParametricBuilder> = match target.kind {
                GateKind::Phase => Some(phase_params),
                GateKind::Rx => Some(rx_params),
                GateKind::Ry => Some(ry_params),
                GateKind::Rz => Some(rz_params),
                _ => None,
            };
            let params = if let Some(build) = parametric_builder {
                if let Some(angle_str) = target.angle.as_deref() {
                    let radians =
                        parse_angle_radians(angle_str).unwrap_or(std::f32::consts::FRAC_PI_2);
                    build(radians, bit, control_mask, control_value, state_count)
                } else if control_mask == 0 {
                    gate_params(target.kind, bit, state_count)
                } else {
                    gate_params_controlled(
                        target.kind,
                        bit,
                        control_mask,
                        control_value,
                        state_count,
                    )
                }
            } else if control_mask == 0 {
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
            && probability_targets.is_empty()
            && amplitude_targets.is_empty()
            && density_targets.is_empty()
            && swap_targets.is_empty()
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

        // Probability displays are also read-only displays. They capture the
        // current GPU state into a per-display probability buffer; rendering
        // samples that buffer directly, no CPU-side probabilities.
        probability_targets.sort_by(|a, b| a.id.cmp(&b.id));
        for display in &probability_targets {
            if display.wire >= qubits {
                continue;
            }
            let span = display.span.clamp(1, 16).min(qubits - display.wire);
            let base_bit = (qubits - display.wire - span) as u32;
            ops.push(SimulationOp::CaptureProbability {
                gate_id: display.id,
                base_bit,
                span: span as u32,
                output_slot: probability_slot,
            });
            probability_slot += 1;
        }

        amplitude_targets.sort_by(|a, b| a.id.cmp(&b.id));
        for display in &amplitude_targets {
            if display.wire >= qubits {
                continue;
            }
            let span = display.span.clamp(1, 16).min(qubits - display.wire);
            let base_bit = (qubits - display.wire - span) as u32;
            ops.push(SimulationOp::CaptureAmplitude {
                gate_id: display.id,
                base_bit,
                span: span as u32,
                output_slot: amplitude_slot,
                control_mask,
                control_value,
            });
            amplitude_slot += 1;
        }

        density_targets.sort_by(|a, b| a.id.cmp(&b.id));
        for display in &density_targets {
            if display.wire >= qubits {
                continue;
            }
            let span = display.span.clamp(1, 8).min(qubits - display.wire);
            let base_bit = (qubits - display.wire - span) as u32;
            ops.push(SimulationOp::CaptureDensity {
                gate_id: display.id,
                base_bit,
                span: span as u32,
                output_slot: density_slot,
                control_mask,
                control_value,
            });
            density_slot += 1;
        }

        if next_snapshot_slot == column.slot && next_snapshot_slot < snapshot_slot_count {
            ops.push(SimulationOp::SnapshotState {
                output_slot: next_snapshot_slot as u32,
            });
            next_snapshot_slot += 1;
        }
    }
    while next_snapshot_slot < snapshot_slot_count {
        ops.push(SimulationOp::SnapshotState {
            output_slot: next_snapshot_slot as u32,
        });
        next_snapshot_slot += 1;
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
