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
    gate_params, gate_params_controlled, phase_params, rx_params, ry_params, rz_params,
    ColumnControls, GateKind,
};
use crate::qubit_bit::QubitBit;
use crate::qubit_count::QubitCount;

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
    qubits: QubitCount,
    snapshot_slot_count: usize,
) -> Vec<SimulationOp> {
    let n = qubits.get();
    let state_count = 1u32 << n;

    let analysis = ColumnAnalysis::from_gates(placed_gates, |gate| {
        if !gate.wire.is_within(qubits) {
            return None;
        }
        Some(gate.column.as_usize())
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
        let mut controls = ColumnControls::NONE;
        let mut targets: Vec<&PlacedGate> = Vec::new();
        let mut bloch_targets: Vec<&PlacedGate> = Vec::new();
        let mut measurement_targets: Vec<&PlacedGate> = Vec::new();
        let mut probability_targets: Vec<&PlacedGate> = Vec::new();
        let mut amplitude_targets: Vec<&PlacedGate> = Vec::new();
        let mut density_targets: Vec<&PlacedGate> = Vec::new();
        let mut swap_targets: Vec<&PlacedGate> = Vec::new();

        let mut qft_gates: Vec<&PlacedGate> = Vec::new();
        for gate in column_gates {
            let bit = gate
                .wire
                .to_qubit_bit(qubits)
                .expect("column gates are filtered to wires within the register");
            match gate.kind {
                GateKind::Control => controls.add_control(bit),
                GateKind::AntiControl => controls.add_anti_control(bit),
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
            let bit_a = swap_targets[0]
                .wire
                .to_qubit_bit(qubits)
                .expect("column gates are within the register");
            let bit_b = swap_targets[1]
                .wire
                .to_qubit_bit(qubits)
                .expect("column gates are within the register");
            let cx_a_to_b = gate_params_controlled(
                GateKind::X,
                bit_b,
                controls.with_control(bit_a),
                state_count,
            );
            let cx_b_to_a = gate_params_controlled(
                GateKind::X,
                bit_a,
                controls.with_control(bit_b),
                state_count,
            );
            ops.push(SimulationOp::ApplyGate(cx_a_to_b));
            ops.push(SimulationOp::ApplyGate(cx_b_to_a));
            ops.push(SimulationOp::ApplyGate(cx_a_to_b));
        }

        targets.sort_by(|a, b| a.id.cmp(&b.id));
        for target in &targets {
            let bit = target
                .wire
                .to_qubit_bit(qubits)
                .expect("column gates are within the register");
            // Parametric gates carry an optional normalized angle. When
            // present we route through the matching `*_params(θ, …)` builder
            // so the matrix carries that angle; when absent we fall back to
            // the editor's pre-parametric π/2 default (the gate's hard-coded
            // matrix in `gate_matrix`). qni would instead error out at
            // simulate time for a bare `P` / `Rx` / `Ry` / `Rz`.
            type ParametricBuilder =
                fn(f32, QubitBit, ColumnControls, u32) -> crate::gates::GateParams;
            let parametric_builder: Option<ParametricBuilder> = match target.kind {
                GateKind::Phase => Some(phase_params),
                GateKind::Rx => Some(rx_params),
                GateKind::Ry => Some(ry_params),
                GateKind::Rz => Some(rz_params),
                _ => None,
            };
            let params = if let Some(build) = parametric_builder {
                if let Some(angle) = target.angle {
                    build(angle.radians_f32(), bit, controls, state_count)
                } else if controls.is_uncontrolled() {
                    gate_params(target.kind, bit, state_count)
                } else {
                    gate_params_controlled(target.kind, bit, controls, state_count)
                }
            } else if controls.is_uncontrolled() {
                gate_params(target.kind, bit, state_count)
            } else {
                gate_params_controlled(target.kind, bit, controls, state_count)
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
            && controls.control_count() >= 2
        {
            let target_bit = controls
                .highest_control()
                .expect("control_count >= 2 guarantees a value bit");
            // Match qni and drop anti-control bits entirely: only the
            // remaining `Control` wires gate the Z. `controls_only` keeps
            // just the value (Control) bits, then `without` removes the Z
            // target. (Mixing anti-control bits into the mask would change
            // the semantics from qni's `{type:'•', targets:[control bits]}` form.)
            let cz_controls = controls.controls_only().without(target_bit);
            let params = gate_params_controlled(GateKind::Z, target_bit, cz_controls, state_count);
            ops.push(SimulationOp::ApplyGate(params));
        }

        // QFT-family gates expand to their textbook decomposition (H +
        // controlled-phase rotations). Column controls are threaded through
        // every decomposed operation, making the whole QFT conditional while
        // preserving the usual internal QFT controlled-phase gates.
        qft_gates.sort_by(|a, b| a.id.cmp(&b.id));
        for qft in &qft_gates {
            let dagger = qft.kind == GateKind::QftDaggerGate;
            let qft_controls = qft_external_controls(qft, qubits, controls);
            ops.extend(linearize_qft(
                qft,
                qubits,
                state_count,
                dagger,
                qft_controls,
            ));
        }

        // Measurements run after the column's unitaries: reduce + sample,
        // then collapse. Each consumes one aux slot.
        measurement_targets.sort_by(|a, b| a.id.cmp(&b.id));
        for measurement in &measurement_targets {
            let qubit_bit = measurement
                .wire
                .to_qubit_bit(qubits)
                .expect("column gates are within the register");
            ops.push(SimulationOp::MeasureReduceSample {
                gate_id: measurement.id.as_u32(),
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
            let qubit_bit = display
                .wire
                .to_qubit_bit(qubits)
                .expect("column gates are within the register");
            ops.push(SimulationOp::CaptureBloch {
                gate_id: display.id.as_u32(),
                qubit_bit,
                output_slot: bloch_slot,
                controls,
            });
            bloch_slot += 1;
        }

        // Probability displays are also read-only displays. They capture the
        // current GPU state into a per-display probability buffer; rendering
        // samples that buffer directly, no CPU-side probabilities.
        probability_targets.sort_by(|a, b| a.id.cmp(&b.id));
        for display in &probability_targets {
            if !display.wire.is_within(qubits) {
                continue;
            }
            let span = display
                .span
                .clamped_for(display.kind, n - display.wire.as_usize());
            let base_bit = display
                .wire
                .offset(span.get() - 1)
                .to_qubit_bit(qubits)
                .expect("span is clamped to the register");
            ops.push(SimulationOp::CaptureProbability {
                gate_id: display.id.as_u32(),
                base_bit,
                span: span.get() as u32,
                output_slot: probability_slot,
                controls,
            });
            probability_slot += 1;
        }

        amplitude_targets.sort_by(|a, b| a.id.cmp(&b.id));
        for display in &amplitude_targets {
            if !display.wire.is_within(qubits) {
                continue;
            }
            let span = display
                .span
                .clamped_for(display.kind, n - display.wire.as_usize());
            let base_bit = display
                .wire
                .offset(span.get() - 1)
                .to_qubit_bit(qubits)
                .expect("span is clamped to the register");
            ops.push(SimulationOp::CaptureAmplitude {
                gate_id: display.id.as_u32(),
                base_bit,
                span: span.get() as u32,
                output_slot: amplitude_slot,
                controls,
            });
            amplitude_slot += 1;
        }

        density_targets.sort_by(|a, b| a.id.cmp(&b.id));
        for display in &density_targets {
            if !display.wire.is_within(qubits) {
                continue;
            }
            let span = display
                .span
                .clamped_for(display.kind, n - display.wire.as_usize());
            let base_bit = display
                .wire
                .offset(span.get() - 1)
                .to_qubit_bit(qubits)
                .expect("span is clamped to the register");
            ops.push(SimulationOp::CaptureDensity {
                gate_id: display.id.as_u32(),
                base_bit,
                span: span.get() as u32,
                output_slot: density_slot,
                controls,
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

/// Return only controls outside the QFT span. A decoded URL can contain a
/// control token on a row covered by the QFT body; treating it as an external
/// control would create self-controlled H / duplicate controlled-phase ops.
fn qft_external_controls(
    gate: &PlacedGate,
    qubits: QubitCount,
    controls: ColumnControls,
) -> ColumnControls {
    if !gate.wire.is_within(qubits) {
        return controls;
    }
    let span = gate
        .span
        .clamped_for(gate.kind, qubits.get() - gate.wire.as_usize())
        .get();
    let mut qft_span_mask = 0u32;
    for offset in 0..span {
        qft_span_mask |= gate
            .wire
            .offset(offset)
            .to_qubit_bit(qubits)
            .expect("offset is within the clamped span")
            .mask();
    }
    controls.restricted_outside(qft_span_mask)
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
    qubits: QubitCount,
    state_count: u32,
    dagger: bool,
    controls: ColumnControls,
) -> Vec<SimulationOp> {
    if !gate.wire.is_within(qubits) {
        return Vec::new();
    }
    // Clamp the span so the QFT never reaches past the qubit register;
    // a user-resized QFT can momentarily extend beyond the placed
    // bottom wire before `update_qubit_count` catches up.
    let span = gate
        .span
        .clamped_for(gate.kind, qubits.get() - gate.wire.as_usize())
        .get();
    if span == 0 {
        return Vec::new();
    }
    let bit_of = |idx: usize| {
        gate.wire
            .offset(idx)
            .to_qubit_bit(qubits)
            .expect("idx is within the clamped span")
    };
    let mut ops = Vec::new();

    if !dagger {
        // QFT: for i in 0..span: H(i), then controlled-Phase π/2^j with
        // control=(i+j) for j in 1..span-i.
        for i in 0..span {
            ops.push(SimulationOp::ApplyGate(qft_h_params(
                bit_of(i),
                state_count,
                controls,
            )));
            for j in 1..(span - i) {
                let phase = std::f32::consts::PI / (1u32 << j) as f32;
                ops.push(SimulationOp::ApplyGate(qft_phase_params(
                    bit_of(i),
                    bit_of(i + j),
                    phase,
                    state_count,
                    controls,
                )));
            }
        }
    } else {
        // QFT†: reverse loop, negated phases, H *after* the phases.
        for i in (0..span).rev() {
            for j in (1..(span - i)).rev() {
                let phase = -std::f32::consts::PI / (1u32 << j) as f32;
                ops.push(SimulationOp::ApplyGate(qft_phase_params(
                    bit_of(i),
                    bit_of(i + j),
                    phase,
                    state_count,
                    controls,
                )));
            }
            ops.push(SimulationOp::ApplyGate(qft_h_params(
                bit_of(i),
                state_count,
                controls,
            )));
        }
    }
    ops
}

fn qft_h_params(
    bit: QubitBit,
    state_count: u32,
    controls: ColumnControls,
) -> crate::gates::GateParams {
    if controls.is_uncontrolled() {
        gate_params(GateKind::H, bit, state_count)
    } else {
        gate_params_controlled(GateKind::H, bit, controls, state_count)
    }
}

fn qft_phase_params(
    target_bit: QubitBit,
    qft_control_bit: QubitBit,
    phase: f32,
    state_count: u32,
    controls: ColumnControls,
) -> crate::gates::GateParams {
    phase_params(
        phase,
        target_bit,
        controls.with_control(qft_control_bit),
        state_count,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qubit_count::QubitCount;

    const EPSILON: f32 = 0.000_001;

    fn qubit_count(value: usize) -> QubitCount {
        QubitCount::try_new(value).expect("test qubit count must be non-zero")
    }

    fn bare_parametric_matrix(kind: GateKind) -> [[f32; 2]; 4] {
        let gate = PlacedGate::new(
            crate::app::GateId::from_u32(1),
            kind,
            crate::app::CircuitColumnIndex::new(0),
            crate::app::WireIndex::new(0),
            crate::gates::GateSpan::SINGLE,
            None,
        );
        let ops = linearize_ops(&[gate], qubit_count(1), 0);
        match ops.first().expect("expected an apply-gate op") {
            SimulationOp::ApplyGate(params) => params.matrix(),
            _ => panic!("expected apply-gate op"),
        }
    }

    fn apply_gate_params(ops: &[SimulationOp], index: usize) -> crate::gates::GateParams {
        match ops.get(index).expect("expected an apply-gate op") {
            SimulationOp::ApplyGate(params) => *params,
            _ => panic!("expected apply-gate op"),
        }
    }

    #[test]
    fn bare_phase_uses_pi_over_two_default() {
        assert!((bare_parametric_matrix(GateKind::Phase)[3][1] - 1.0).abs() < EPSILON);
    }

    #[test]
    fn bare_rx_uses_pi_over_two_default() {
        assert!(
            (bare_parametric_matrix(GateKind::Rx)[1][1] + std::f32::consts::FRAC_1_SQRT_2).abs()
                < EPSILON
        );
    }

    #[test]
    fn bare_ry_uses_pi_over_two_default() {
        assert!(
            (bare_parametric_matrix(GateKind::Ry)[2][0] - std::f32::consts::FRAC_1_SQRT_2).abs()
                < EPSILON
        );
    }

    #[test]
    fn bare_rz_uses_pi_over_two_default() {
        assert!(
            (bare_parametric_matrix(GateKind::Rz)[0][1] + std::f32::consts::FRAC_1_SQRT_2).abs()
                < EPSILON
        );
    }

    #[test]
    fn two_controls_emit_controlled_z() {
        // 3 qubits, Controls on the top two wires (no target gate) → CCZ.
        // wire 0 → bit 2 (highest = Z target); wire 1 → bit 1 (control).
        let gates = [
            PlacedGate::new(
                crate::app::GateId::from_u32(1),
                GateKind::Control,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(0),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
            PlacedGate::new(
                crate::app::GateId::from_u32(2),
                GateKind::Control,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(1),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
        ];

        let params = apply_gate_params(&linearize_ops(&gates, qubit_count(3), 0), 0);

        assert_eq!(
            (params.bit(), params.control_mask(), params.control_value()),
            (2, 0b010, 0b010)
        );
    }

    #[test]
    fn controlled_z_drops_anti_control_bits() {
        // 3 qubits: Controls on wires 0, 1 and an AntiControl on wire 2.
        // The Z keeps only the remaining Control (bit 1); the anti-control
        // (bit 0) must not leak into the Z's control mask.
        let gates = [
            PlacedGate::new(
                crate::app::GateId::from_u32(1),
                GateKind::Control,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(0),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
            PlacedGate::new(
                crate::app::GateId::from_u32(2),
                GateKind::Control,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(1),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
            PlacedGate::new(
                crate::app::GateId::from_u32(3),
                GateKind::AntiControl,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(2),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
        ];

        let params = apply_gate_params(&linearize_ops(&gates, qubit_count(3), 0), 0);

        assert_eq!(
            (params.bit(), params.control_mask(), params.control_value()),
            (2, 0b010, 0b010)
        );
    }

    #[test]
    fn controlled_qft_applies_external_control_to_hadamard() {
        let gates = [
            PlacedGate::new(
                crate::app::GateId::from_u32(1),
                GateKind::Control,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(0),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
            PlacedGate::new(
                crate::app::GateId::from_u32(2),
                GateKind::QftGate,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(1),
                crate::gates::GateSpan::try_new(2).unwrap(),
                None,
            ),
        ];

        let params = apply_gate_params(&linearize_ops(&gates, qubit_count(3), 0), 0);

        assert_eq!(
            (params.bit(), params.control_mask(), params.control_value()),
            (1, 0b100, 0b100)
        );
    }

    #[test]
    fn controlled_qft_combines_external_control_with_internal_phase_control() {
        let gates = [
            PlacedGate::new(
                crate::app::GateId::from_u32(1),
                GateKind::Control,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(0),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
            PlacedGate::new(
                crate::app::GateId::from_u32(2),
                GateKind::QftGate,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(1),
                crate::gates::GateSpan::try_new(2).unwrap(),
                None,
            ),
        ];

        let params = apply_gate_params(&linearize_ops(&gates, qubit_count(3), 0), 1);

        assert_eq!(
            (params.bit(), params.control_mask(), params.control_value()),
            (1, 0b101, 0b101)
        );
    }

    #[test]
    fn anti_controlled_qft_keeps_zero_external_control_value() {
        let gates = [
            PlacedGate::new(
                crate::app::GateId::from_u32(1),
                GateKind::AntiControl,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(0),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
            PlacedGate::new(
                crate::app::GateId::from_u32(2),
                GateKind::QftGate,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(1),
                crate::gates::GateSpan::try_new(2).unwrap(),
                None,
            ),
        ];

        let params = apply_gate_params(&linearize_ops(&gates, qubit_count(3), 0), 0);

        assert_eq!(
            (params.bit(), params.control_mask(), params.control_value()),
            (1, 0b100, 0)
        );
    }

    #[test]
    fn qft_ignores_control_tokens_inside_its_span() {
        let gates = [
            PlacedGate::new(
                crate::app::GateId::from_u32(1),
                GateKind::QftGate,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(0),
                crate::gates::GateSpan::try_new(2).unwrap(),
                None,
            ),
            PlacedGate::new(
                crate::app::GateId::from_u32(2),
                GateKind::Control,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(1),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
        ];

        let params = apply_gate_params(&linearize_ops(&gates, qubit_count(2), 0), 0);

        assert_eq!(
            (params.bit(), params.control_mask(), params.control_value()),
            (1, 0, 0)
        );
    }

    #[test]
    fn probability_display_captures_column_controls() {
        let gates = [
            PlacedGate::new(
                crate::app::GateId::from_u32(1),
                GateKind::Control,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(0),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
            PlacedGate::new(
                crate::app::GateId::from_u32(2),
                GateKind::ProbabilityDisplay,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(1),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
        ];

        let ops = linearize_ops(&gates, qubit_count(2), 0);

        assert!(matches!(
            ops.first(),
            Some(SimulationOp::CaptureProbability { controls, .. })
                if controls.mask() == 0b10 && controls.value() == 0b10
        ));
    }

    #[test]
    fn probability_display_base_bit_is_span_bottom() {
        // 3 qubits, top wire, span 2 → base_bit = qubits - wire - span = 1,
        // exercising `wire.offset(span - 1).to_qubit_bit(qubits)`.
        let gates = [PlacedGate::new(
            crate::app::GateId::from_u32(1),
            GateKind::ProbabilityDisplay,
            crate::app::CircuitColumnIndex::new(0),
            crate::app::WireIndex::new(0),
            crate::gates::GateSpan::try_new(2).unwrap(),
            None,
        )];

        let ops = linearize_ops(&gates, qubit_count(3), 0);

        assert!(matches!(
            ops.first(),
            Some(SimulationOp::CaptureProbability { base_bit, .. }) if base_bit.as_u32() == 1
        ));
    }

    #[test]
    fn density_display_caps_span_at_eight() {
        // A density display stored with span 10 lowers with span clamped to the
        // GateKind cap of 8 via `GateSpan::clamped_for`, even at 16 qubits.
        let gates = [PlacedGate::new(
            crate::app::GateId::from_u32(1),
            GateKind::DensityMatrixDisplay,
            crate::app::CircuitColumnIndex::new(0),
            crate::app::WireIndex::new(0),
            crate::gates::GateSpan::try_new(10).unwrap(),
            None,
        )];

        let ops = linearize_ops(&gates, qubit_count(16), 0);

        assert!(matches!(
            ops.first(),
            Some(SimulationOp::CaptureDensity { span: 8, .. })
        ));
    }

    #[test]
    fn bloch_display_captures_column_controls() {
        let gates = [
            PlacedGate::new(
                crate::app::GateId::from_u32(1),
                GateKind::AntiControl,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(0),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
            PlacedGate::new(
                crate::app::GateId::from_u32(2),
                GateKind::BlochDisplay,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(1),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
        ];

        let ops = linearize_ops(&gates, qubit_count(2), 0);

        assert!(matches!(
            ops.first(),
            Some(SimulationOp::CaptureBloch { controls, .. })
                if controls.mask() == 0b10 && controls.value() == 0
        ));
    }
}
