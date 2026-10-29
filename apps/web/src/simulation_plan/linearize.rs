use std::collections::HashMap;

use super::arithmetic_field::ArithmeticField;
use super::{FlagSource, FlagSources, PhaseGroupKey, SimulationColumnAnalysis, SimulationOp};
use crate::app::{GateId, PlacedGate};
use crate::gates::{
    gate_params, gate_params_controlled, phase_params, rx_params, ry_params, rz_params,
    ColumnControls, GateKind,
};
use crate::gpu::{Amplitude, Bloch, Density, Measurement, Probability, SlotAllocator, SlotIndex};
use crate::qubit_bit::QubitBit;
use crate::qubit_count::QubitCount;

pub(crate) fn linearize_ops(
    placed_gates: &[PlacedGate],
    qubits: QubitCount,
    snapshot_slot_count: usize,
) -> Vec<SimulationOp> {
    let n = qubits.get();
    let state_count = qubits
        .local_state_count_u32()
        .expect("linearize runs only on the local dispatch path within local capacity");

    let analysis = SimulationColumnAnalysis::from_gates(placed_gates, qubits);
    let flag_sources = FlagSources::from_columns(&analysis);
    let mut measurement_slot_by_gate: HashMap<GateId, SlotIndex<Measurement>> = HashMap::new();

    let mut ops: Vec<SimulationOp> = Vec::new();
    let mut next_snapshot_slot = 0usize;
    let mut bloch_slots = SlotAllocator::<Bloch>::new();
    let mut measurement_slots = SlotAllocator::<Measurement>::new();
    let mut probability_slots = SlotAllocator::<Probability>::new();
    let mut amplitude_slots = SlotAllocator::<Amplitude>::new();
    let mut postselection_slots = SlotAllocator::<crate::gpu::Postselection>::new();
    let mut density_slots = SlotAllocator::<Density>::new();
    for column in analysis.columns() {
        while next_snapshot_slot < column.slot && next_snapshot_slot < snapshot_slot_count {
            ops.push(SimulationOp::SnapshotState {
                output_slot: SlotIndex::new(next_snapshot_slot as u32),
            });
            next_snapshot_slot += 1;
        }
        let column_gates = column.gates();
        let mut controls = column.controls();
        let detector_targets = column.displays(GateKind::ZDetectControlReset);
        for detector in &detector_targets {
            push_measurement(
                &mut ops,
                detector,
                qubits,
                &mut measurement_slots,
                ColumnControls::NONE,
                true,
            );
        }
        let mut targets: Vec<&PlacedGate> = Vec::new();
        let mut bloch_targets: Vec<&PlacedGate> = Vec::new();
        let mut measurement_targets: Vec<&PlacedGate> = Vec::new();
        let mut probability_targets: Vec<&PlacedGate> = Vec::new();
        let mut amplitude_targets: Vec<&PlacedGate> = Vec::new();
        let mut density_targets: Vec<&PlacedGate> = Vec::new();
        let mut swap_targets: Vec<&PlacedGate> = Vec::new();
        let mut parity_targets: Vec<&PlacedGate> = Vec::new();

        let mut register_gates: Vec<&PlacedGate> = Vec::new();
        for gate in column_gates {
            match gate.kind {
                GateKind::Control | GateKind::AntiControl | GateKind::ZDetectControlReset => {}
                GateKind::ZParityControl | GateKind::XParityControl => parity_targets.push(gate),
                GateKind::Swap => swap_targets.push(gate),
                GateKind::Spacer => {}
                GateKind::Measurement | GateKind::XDetector | GateKind::YDetector => {
                    measurement_targets.push(gate);
                }
                GateKind::BlochDisplay => bloch_targets.push(gate),
                GateKind::ProbabilityDisplay | GateKind::SampleDisplay => {
                    probability_targets.push(gate)
                }
                GateKind::AmplitudeDisplay => amplitude_targets.push(gate),
                GateKind::DensityMatrixDisplay => density_targets.push(gate),
                GateKind::QftGate
                | GateKind::QftDaggerGate
                | GateKind::Increment
                | GateKind::Decrement => register_gates.push(gate),
                _ => targets.push(gate),
            }
        }

        parity_targets.retain(|parity| {
            !column_gates.iter().any(|gate| {
                gate.kind.is_resizable_span()
                    && gate.wire.as_usize() <= parity.wire.as_usize()
                    && parity.wire.as_usize()
                        < gate.wire.as_usize()
                            + gate
                                .span
                                .clamped_for(gate.kind, n - gate.wire.as_usize())
                                .get()
            })
        });
        parity_targets.sort_by_key(|gate| gate.wire);
        let mut parity_setup = Vec::new();
        for gate in &parity_targets {
            if gate.kind == GateKind::XParityControl {
                let bit = gate
                    .wire
                    .to_qubit_bit(qubits)
                    .expect("column gates are within the register");
                parity_setup.push(gate_params(GateKind::H, bit, state_count));
            }
        }
        if let Some(anchor) = parity_targets.first() {
            let anchor_bit = anchor
                .wire
                .to_qubit_bit(qubits)
                .expect("column gates are within the register");
            controls.add_control(anchor_bit);
            for gate in parity_targets.iter().skip(1) {
                let bit = gate
                    .wire
                    .to_qubit_bit(qubits)
                    .expect("column gates are within the register");
                parity_setup.push(gate_params_controlled(
                    GateKind::X,
                    anchor_bit,
                    ColumnControls::NONE.with_control(bit),
                    state_count,
                ));
            }
        }
        ops.extend(parity_setup.iter().copied().map(SimulationOp::ApplyGate));
        measurement_targets.sort_by_key(|gate| gate.id);
        for detector in &measurement_targets {
            push_detector_basis(&mut ops, detector, qubits, state_count, false);
        }

        swap_targets.sort_by_key(|a| a.id);
        if swap_targets.len() == 2 {
            let bit_a = swap_targets[0]
                .wire
                .to_qubit_bit(qubits)
                .expect("column gates are within the register");
            let bit_b = swap_targets[1]
                .wire
                .to_qubit_bit(qubits)
                .expect("column gates are within the register");
            push_swap_3cnot(&mut ops, bit_a, bit_b, controls, state_count);
        }

        targets.sort_by_key(|a| a.id);
        let phase_roles = phase_group_roles(&targets, qubits);
        for target in &targets {
            let bit = target
                .wire
                .to_qubit_bit(qubits)
                .expect("column gates are within the register");
            let controls = match phase_roles.get(&target.id) {
                None => controls,
                Some(PhaseRole::Partner) => continue,
                Some(PhaseRole::Target { partner_bits }) => partner_bits
                    .iter()
                    .fold(controls, |controls, bit| controls.with_control(*bit)),
            };
            if target.kind == GateKind::Write0 {
                push_reset(
                    &mut ops,
                    target,
                    qubits,
                    state_count,
                    &mut measurement_slots,
                    controls,
                );
                continue;
            }
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
            let params = match flag_sources.source(target.id) {
                None => params,
                Some(FlagSource::Measurement(source)) => {
                    let Some(slot) = measurement_slot_by_gate.get(&source) else {
                        continue;
                    };
                    params.conditioned_on(*slot)
                }
                Some(FlagSource::NeverSet) => continue,
            };
            ops.push(SimulationOp::ApplyGate(params));
            if target.kind.is_postselection() {
                ops.push(SimulationOp::Postselect {
                    gate_id: target.id,
                    output_slot: postselection_slots.allocate(),
                });
            }
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
        // `control_mask & !control_value`). We pick the highest set bit of
        // `control_value` (now the bottommost wire, since our bit numbering
        // runs 0..qubits-1 top→bottom with q0=LSB) as the Z target. CZ is
        // physically symmetric so the choice only affects the GateParams
        // shape, not the resulting state.
        if detector_targets.is_empty()
            && parity_targets.is_empty()
            && targets.is_empty()
            && register_gates.is_empty()
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
            let cz_controls = controls.controls_only().without(target_bit);
            let params = gate_params_controlled(GateKind::Z, target_bit, cz_controls, state_count);
            ops.push(SimulationOp::ApplyGate(params));
        }

        register_gates.sort_by_key(|a| (a.wire, a.id));
        for gate in &register_gates {
            if matches!(gate.kind, GateKind::Increment | GateKind::Decrement) {
                let span = gate
                    .span
                    .clamped_for(gate.kind, n - gate.wire.as_usize())
                    .get();
                let field = ArithmeticField::new(gate.wire.as_usize() as u32, span as u32);
                let gate_controls = field.external_controls(controls);
                ops.push(SimulationOp::Arithmetic(crate::gpu::ArithmeticParams {
                    state_count,
                    target_shift: field.shift(),
                    target_mask: field.mask(),
                    amount: ArithmeticField::amount(gate.kind == GateKind::Increment),
                    control_mask: gate_controls.mask(),
                    control_value: gate_controls.value(),
                    _pad: [0; 2],
                }));
                continue;
            }
            let gate_controls = register_external_controls(gate, qubits, controls);
            let dagger = gate.kind == GateKind::QftDaggerGate;
            ops.extend(linearize_qft(
                gate,
                qubits,
                state_count,
                dagger,
                gate_controls,
            ));
        }

        for measurement in &measurement_targets {
            let measurement_controls = if measurement.kind == GateKind::Measurement {
                ColumnControls::NONE
            } else {
                controls
            };
            let slot = push_measurement(
                &mut ops,
                measurement,
                qubits,
                &mut measurement_slots,
                measurement_controls,
                true,
            );
            measurement_slot_by_gate.insert(measurement.id, slot);
        }

        for detector in measurement_targets.iter().rev() {
            push_detector_basis(&mut ops, detector, qubits, state_count, true);
        }

        bloch_targets.sort_by_key(|a| a.id);
        for display in &bloch_targets {
            let qubit_bit = display
                .wire
                .to_qubit_bit(qubits)
                .expect("column gates are within the register");
            ops.push(SimulationOp::CaptureBloch {
                gate_id: display.id,
                qubit_bit,
                output_slot: bloch_slots.allocate(),
                controls,
            });
        }

        probability_targets.sort_by_key(|a| a.id);
        for display in &probability_targets {
            let span = display
                .span
                .clamped_for(display.kind, n - display.wire.as_usize());
            // span 内の最下位ビット = 上端ワイヤ（q0=LSB なので wire がそのままビット位置）。
            // outcome の bit j は上端から j 本目のワイヤに対応し、ケットでは q0=上端が右端になる。
            let base_bit = display
                .wire
                .to_qubit_bit(qubits)
                .expect("display wire is within the register");
            ops.push(SimulationOp::CaptureProbability {
                gate_id: display.id,
                base_bit,
                span: span.get() as u32,
                output_slot: probability_slots.allocate(),
                controls,
            });
        }

        amplitude_targets.sort_by_key(|a| a.id);
        for display in &amplitude_targets {
            let span = display
                .span
                .clamped_for(display.kind, n - display.wire.as_usize());
            // span 内の最下位ビット = 上端ワイヤ（q0=LSB なので wire がそのままビット位置）。
            // outcome の bit j は上端から j 本目のワイヤに対応し、ケットでは q0=上端が右端になる。
            let base_bit = display
                .wire
                .to_qubit_bit(qubits)
                .expect("display wire is within the register");
            ops.push(SimulationOp::CaptureAmplitude {
                gate_id: display.id,
                base_bit,
                span: span.get() as u32,
                output_slot: amplitude_slots.allocate(),
                controls,
            });
        }

        density_targets.sort_by_key(|a| a.id);
        for display in &density_targets {
            let span = display
                .span
                .clamped_for(display.kind, n - display.wire.as_usize());
            // span 内の最下位ビット = 上端ワイヤ（q0=LSB なので wire がそのままビット位置）。
            // outcome の bit j は上端から j 本目のワイヤに対応し、ケットでは q0=上端が右端になる。
            let base_bit = display
                .wire
                .to_qubit_bit(qubits)
                .expect("display wire is within the register");
            ops.push(SimulationOp::CaptureDensity {
                gate_id: display.id,
                base_bit,
                span: span.get() as u32,
                output_slot: density_slots.allocate(),
                controls,
            });
        }

        for detector in &detector_targets {
            push_reset(
                &mut ops,
                detector,
                qubits,
                state_count,
                &mut measurement_slots,
                ColumnControls::NONE,
            );
        }
        ops.extend(
            parity_setup
                .iter()
                .rev()
                .copied()
                .map(SimulationOp::ApplyGate),
        );

        if next_snapshot_slot == column.slot && next_snapshot_slot < snapshot_slot_count {
            ops.push(SimulationOp::SnapshotState {
                output_slot: SlotIndex::new(next_snapshot_slot as u32),
            });
            next_snapshot_slot += 1;
        }
    }
    while next_snapshot_slot < snapshot_slot_count {
        ops.push(SimulationOp::SnapshotState {
            output_slot: SlotIndex::new(next_snapshot_slot as u32),
        });
        next_snapshot_slot += 1;
    }
    ops
}

fn push_measurement(
    ops: &mut Vec<SimulationOp>,
    gate: &PlacedGate,
    qubits: QubitCount,
    slots: &mut SlotAllocator<Measurement>,
    controls: ColumnControls,
    visible: bool,
) -> SlotIndex<Measurement> {
    let qubit_bit = gate
        .wire
        .to_qubit_bit(qubits)
        .expect("column gates are within the register");
    let slot = slots.allocate();
    ops.push(SimulationOp::MeasureReduceSample {
        visible,
        gate_id: gate.id,
        qubit_bit,
        output_slot: slot,
        controls,
    });
    ops.push(SimulationOp::MeasureCollapse {
        qubit_bit,
        aux_slot: slot,
        controls,
    });
    slot
}

fn push_reset(
    ops: &mut Vec<SimulationOp>,
    gate: &PlacedGate,
    qubits: QubitCount,
    state_count: u32,
    slots: &mut SlotAllocator<Measurement>,
    controls: ColumnControls,
) {
    let slot = push_measurement(ops, gate, qubits, slots, controls, false);
    let bit = gate
        .wire
        .to_qubit_bit(qubits)
        .expect("reset within register");
    ops.push(SimulationOp::ApplyGate(
        gate_params_controlled(GateKind::X, bit, controls, state_count).conditioned_on(slot),
    ));
}

fn push_detector_basis(
    ops: &mut Vec<SimulationOp>,
    detector: &PlacedGate,
    qubits: QubitCount,
    state_count: u32,
    inverse: bool,
) {
    let kinds: &[GateKind] = match (detector.kind, inverse) {
        (GateKind::XDetector, _) => &[GateKind::H],
        (GateKind::YDetector, false) => &[GateKind::SDagger, GateKind::H],
        (GateKind::YDetector, true) => &[GateKind::H, GateKind::S],
        _ => &[],
    };
    for &kind in kinds {
        let bit = detector
            .wire
            .to_qubit_bit(qubits)
            .expect("column gates are within the register");
        ops.push(SimulationOp::ApplyGate(gate_params_controlled(
            kind,
            bit,
            ColumnControls::NONE,
            state_count,
        )));
    }
}

enum PhaseRole {
    Target { partner_bits: Vec<QubitBit> },
    Partner,
}

/// Splits a column's `P` gates into `PhaseGroupKey` groups. A phase on
/// |1…1⟩ is symmetric in its wires, so any member can be the target; the
/// top-most one is picked for a stable op stream.
fn phase_group_roles(targets: &[&PlacedGate], qubits: QubitCount) -> HashMap<GateId, PhaseRole> {
    let mut groups: HashMap<PhaseGroupKey<'_>, Vec<&PlacedGate>> = HashMap::new();
    for gate in targets {
        if let Some(key) = PhaseGroupKey::of(gate) {
            groups.entry(key).or_default().push(gate);
        }
    }
    let mut roles = HashMap::new();
    for mut members in groups.into_values() {
        members.sort_by_key(|gate| gate.wire);
        let (target, partners) = members
            .split_first()
            .expect("a phase group has at least one member");
        let partner_bits = partners
            .iter()
            .map(|gate| {
                gate.wire
                    .to_qubit_bit(qubits)
                    .expect("column gates are within the register")
            })
            .collect();
        roles.insert(target.id, PhaseRole::Target { partner_bits });
        for partner in partners {
            roles.insert(partner.id, PhaseRole::Partner);
        }
    }
    roles
}

fn register_external_controls(
    gate: &PlacedGate,
    qubits: QubitCount,
    controls: ColumnControls,
) -> ColumnControls {
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

/// Expand a placed QFT (or QFT†) gate into the textbook decomposition:
/// a bit-reversal SWAP network plus `span` Hadamards interleaved with
/// controlled-phase rotations `R_k = diag(1, exp(iπ/2^j))`.
///
/// The bare H/phase ladder (H on the top wire first, controlled phases
/// running downward) realizes the DFT acting on bit-reversed input
/// (`F·BR`). Putting the bit-reversal swaps *before* the ladder feeds it the
/// reversed input, so the net transform is the true (symmetric) discrete
/// Fourier transform `F[r,c] = (1/√N)·exp(+i2πrc/N)` — matching Quirk's
/// `FourierTransformGates`, which likewise reverses first. A Quirk QFT circuit
/// ports 1:1: e.g. a which-path mark on the QFT register's bit-2 wire yields
/// 4 interference fringes, as in Quirk.
///
/// `QFT` = swaps then ladder; `QFT†` is its adjoint = inverse ladder then
/// swaps, so the swap network comes *after* the (inverse) ladder.
///
/// Wire-to-bit mapping: `gate.wire + idx` (wire index of the i-th
/// qubit in the QFT register) → `bit = wire` (the simulator convention
/// where the top wire q0 is the LSB; identity map).
fn linearize_qft(
    gate: &PlacedGate,
    qubits: QubitCount,
    state_count: u32,
    dagger: bool,
    controls: ColumnControls,
) -> Vec<SimulationOp> {
    // Clamp the span so the QFT never reaches past the qubit register;
    // a user-resized QFT can momentarily extend beyond the placed
    // bottom wire before `update_qubit_count` catches up.
    let span = gate
        .span
        .clamped_for(gate.kind, qubits.get() - gate.wire.as_usize())
        .get();
    let bit_of = |idx: usize| {
        gate.wire
            .offset(idx)
            .to_qubit_bit(qubits)
            .expect("idx is within the clamped span")
    };
    let mut ops = Vec::new();
    let reversal = qft_reversal_swap_ops(gate, qubits, span, state_count, controls);

    if !dagger {
        ops.extend(reversal);
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
        ops.extend(reversal);
    }
    ops
}

fn qft_reversal_swap_ops(
    gate: &PlacedGate,
    qubits: QubitCount,
    span: usize,
    state_count: u32,
    controls: ColumnControls,
) -> Vec<SimulationOp> {
    let bit_of = |idx: usize| {
        gate.wire
            .offset(idx)
            .to_qubit_bit(qubits)
            .expect("idx is within the clamped span")
    };
    let mut ops = Vec::new();
    for i in 0..(span / 2) {
        push_swap_3cnot(
            &mut ops,
            bit_of(i),
            bit_of(span - 1 - i),
            controls,
            state_count,
        );
    }
    ops
}

fn push_swap_3cnot(
    ops: &mut Vec<SimulationOp>,
    bit_a: QubitBit,
    bit_b: QubitBit,
    controls: ColumnControls,
    state_count: u32,
) {
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

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum QftExpandedKind {
        Hadamard,
        Phase,
        PauliX,
    }

    fn reference_matrix(kind: GateKind) -> [[f32; 2]; 4] {
        gate_params(kind, QubitBit::new(0), 0).matrix()
    }

    fn qft_expanded_kind(params: crate::gates::GateParams) -> Option<QftExpandedKind> {
        let matrix = params.matrix();
        if matrix == reference_matrix(GateKind::H) {
            Some(QftExpandedKind::Hadamard)
        } else if matrix == reference_matrix(GateKind::X) {
            Some(QftExpandedKind::PauliX)
        } else if matrix[0] == [1.0, 0.0]
            && matrix[1] == [0.0, 0.0]
            && matrix[2] == [0.0, 0.0]
            && matrix[3][1] != 0.0
        {
            Some(QftExpandedKind::Phase)
        } else {
            None
        }
    }

    fn apply_qft_expanded(
        ops: &[SimulationOp],
        kind: QftExpandedKind,
        bit: u32,
    ) -> crate::gates::GateParams {
        ops.iter()
            .find_map(|op| match op {
                SimulationOp::ApplyGate(params)
                    if qft_expanded_kind(*params) == Some(kind) && params.bit() == bit =>
                {
                    Some(*params)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing {kind:?} on bit {bit}"))
    }

    fn apply_trailing_qft_swap(ops: &[SimulationOp], bit: u32) -> crate::gates::GateParams {
        let ladder_end = ops
            .iter()
            .rposition(|op| match op {
                SimulationOp::ApplyGate(params) => matches!(
                    qft_expanded_kind(*params),
                    Some(QftExpandedKind::Hadamard | QftExpandedKind::Phase)
                ),
                _ => false,
            })
            .expect("QFT ladder");
        ops.iter()
            .skip(ladder_end + 1)
            .find_map(|op| match op {
                SimulationOp::ApplyGate(params)
                    if qft_expanded_kind(*params) == Some(QftExpandedKind::PauliX)
                        && params.bit() == bit =>
                {
                    Some(*params)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing trailing swap CNOT on bit {bit}"))
    }

    fn detector_column(kind: GateKind) -> Vec<SimulationOp> {
        let kinds = [
            GateKind::Control,
            GateKind::AntiControl,
            kind,
            GateKind::Z,
            GateKind::Measurement,
            GateKind::BlochDisplay,
        ];
        let gates: Vec<_> = kinds
            .into_iter()
            .enumerate()
            .map(|(wire, kind)| {
                PlacedGate::new(
                    GateId::from_u32(wire as u32 + 1),
                    kind,
                    crate::app::CircuitColumnIndex::new(0),
                    crate::app::WireIndex::new(wire),
                    crate::gates::GateSpan::SINGLE,
                    None,
                )
            })
            .collect();
        linearize_ops(&gates, qubit_count(6), 0)
    }

    fn expected_detector_column(setup: &[GateKind], cleanup: &[GateKind]) -> Vec<SimulationOp> {
        let mut controls = ColumnControls::NONE.with_control(QubitBit::new(0));
        controls.add_anti_control(QubitBit::new(1));
        let basis = |kind| {
            SimulationOp::ApplyGate(gate_params_controlled(
                kind,
                QubitBit::new(2),
                ColumnControls::NONE,
                64,
            ))
        };
        let mut ops: Vec<_> = setup.iter().copied().map(basis).collect();
        ops.push(SimulationOp::ApplyGate(gate_params_controlled(
            GateKind::Z,
            QubitBit::new(3),
            controls,
            64,
        )));
        for (id, bit, slot, controls) in [(3, 2, 0, controls), (5, 4, 1, ColumnControls::NONE)] {
            ops.push(SimulationOp::MeasureReduceSample {
                visible: true,
                gate_id: GateId::from_u32(id),
                qubit_bit: QubitBit::new(bit),
                output_slot: SlotIndex::new(slot),
                controls,
            });
            ops.push(SimulationOp::MeasureCollapse {
                qubit_bit: QubitBit::new(bit),
                aux_slot: SlotIndex::new(slot),
                controls,
            });
        }
        ops.extend(cleanup.iter().copied().map(basis));
        ops.push(SimulationOp::CaptureBloch {
            gate_id: GateId::from_u32(6),
            qubit_bit: QubitBit::new(5),
            output_slot: SlotIndex::new(0),
            controls,
        });
        ops
    }

    #[test]
    fn x_detector_wraps_column_in_uncontrolled_hadamards() {
        assert_eq!(
            format!("{:?}", detector_column(GateKind::XDetector)),
            format!(
                "{:?}",
                expected_detector_column(&[GateKind::H], &[GateKind::H])
            )
        );
    }

    #[test]
    fn y_detector_wraps_column_in_uncontrolled_inverse_bases() {
        assert_eq!(
            format!("{:?}", detector_column(GateKind::YDetector)),
            format!(
                "{:?}",
                expected_detector_column(
                    &[GateKind::SDagger, GateKind::H],
                    &[GateKind::H, GateKind::S]
                )
            )
        );
    }

    #[test]
    fn parity_controls_detectors_and_sample_before_scattering() {
        let actual: Vec<_> = [GateKind::XDetector, GateKind::YDetector]
            .into_iter()
            .map(|kind| {
                let gates: Vec<_> = [
                    GateKind::XParityControl,
                    GateKind::ZParityControl,
                    kind,
                    GateKind::Z,
                    GateKind::Measurement,
                    GateKind::SampleDisplay,
                ]
                .into_iter()
                .enumerate()
                .map(|(wire, kind)| {
                    PlacedGate::new(
                        GateId::from_u32(wire as u32 + 1),
                        kind,
                        crate::app::CircuitColumnIndex::new(0),
                        crate::app::WireIndex::new(wire),
                        crate::gates::GateSpan::SINGLE,
                        None,
                    )
                })
                .collect();
                format!("{:?}", linearize_ops(&gates, qubit_count(6), 1))
            })
            .collect();
        let expected: Vec<_> = [
            (&[GateKind::H][..], &[GateKind::H][..]),
            (
                &[GateKind::SDagger, GateKind::H][..],
                &[GateKind::H, GateKind::S][..],
            ),
        ]
        .into_iter()
        .map(|(setup, cleanup)| {
            let controls = ColumnControls::NONE.with_control(QubitBit::new(0));
            let h = gate_params(GateKind::H, QubitBit::new(0), 64);
            let gather = gate_params_controlled(
                GateKind::X,
                QubitBit::new(0),
                ColumnControls::NONE.with_control(QubitBit::new(1)),
                64,
            );
            let mut ops = vec![SimulationOp::ApplyGate(h), SimulationOp::ApplyGate(gather)];
            // Reuse the detector expectation, replacing ordinary controls with parity.
            let mut detector_ops = expected_detector_column(setup, cleanup);
            detector_ops.pop(); // The combined column has Sample instead of Bloch.
            for op in &mut detector_ops {
                match op {
                    SimulationOp::ApplyGate(params) if params.bit() == 3 => {
                        *params =
                            gate_params_controlled(GateKind::Z, QubitBit::new(3), controls, 64);
                    }
                    SimulationOp::MeasureReduceSample {
                        qubit_bit,
                        controls: c,
                        ..
                    }
                    | SimulationOp::MeasureCollapse {
                        qubit_bit,
                        controls: c,
                        ..
                    } if qubit_bit.as_u32() == 2 => *c = controls,
                    _ => {}
                }
            }
            ops.extend(detector_ops);
            ops.push(SimulationOp::CaptureProbability {
                gate_id: GateId::from_u32(6),
                base_bit: QubitBit::new(5),
                span: 1,
                output_slot: SlotIndex::new(0),
                controls,
            });
            ops.extend([SimulationOp::ApplyGate(gather), SimulationOp::ApplyGate(h)]);
            ops.push(SimulationOp::SnapshotState {
                output_slot: SlotIndex::new(0),
            });
            format!("{:?}", ops)
        })
        .collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn z_measurement_ignores_column_controls() {
        let ops = detector_column(GateKind::Measurement);
        assert!(ops
            .iter()
            .filter_map(|op| match op {
                SimulationOp::MeasureReduceSample { controls, .. }
                | SimulationOp::MeasureCollapse { controls, .. } => Some(controls),
                _ => None,
            })
            .all(|controls| controls.is_uncontrolled()));
    }

    #[test]
    fn parity_inside_qft_span_does_not_conjugate_the_transform() {
        let overlapping =
            crate::url_circuit::parse_circuit_json(r#"{"cols":[["QFT3","zpar","xpar"]]}"#);
        let reference = crate::url_circuit::parse_circuit_json(r#"{"cols":[["QFT3"]]}"#);
        assert_eq!(
            format!("{:?}", linearize_ops(&overlapping.gates, qubit_count(3), 1)),
            format!("{:?}", linearize_ops(&reference.gates, qubit_count(3), 1))
        );
    }

    #[test]
    fn parity_inside_sample_span_does_not_permute_display_outcomes() {
        let overlapping =
            crate::url_circuit::parse_circuit_json(r#"{"cols":[["Sample3","zpar","xpar"]]}"#);
        let reference = crate::url_circuit::parse_circuit_json(r#"{"cols":[["Sample3"]]}"#);
        assert_eq!(
            format!("{:?}", linearize_ops(&overlapping.gates, qubit_count(3), 1)),
            format!("{:?}", linearize_ops(&reference.gates, qubit_count(3), 1))
        );
    }

    #[derive(Debug, PartialEq)]
    enum ParityStep {
        Gate(u32, u32, u32, [[f32; 2]; 4]),
        Probability(u32, u32),
        Bloch(u32, u32, u32),
        Amplitude(u32, u32),
        Density(u32, u32),
        Snapshot,
    }

    fn parity_step(kind: GateKind, bit: u32, mask: u32, value: u32) -> ParityStep {
        ParityStep::Gate(
            bit,
            mask,
            value,
            gate_params(kind, QubitBit::new(bit), 64).matrix(),
        )
    }

    fn parity_stream(kinds: &[(GateKind, usize)], snapshots: usize) -> Vec<ParityStep> {
        let gates: Vec<_> = kinds
            .iter()
            .enumerate()
            .map(|(id, &(kind, wire))| {
                PlacedGate::new(
                    GateId::from_u32(id as u32),
                    kind,
                    crate::app::CircuitColumnIndex::new(0),
                    crate::app::WireIndex::new(wire),
                    crate::gates::GateSpan::SINGLE,
                    None,
                )
            })
            .collect();
        linearize_ops(&gates, qubit_count(6), snapshots)
            .into_iter()
            .map(|op| match op {
                SimulationOp::ApplyGate(p) => {
                    ParityStep::Gate(p.bit(), p.control_mask(), p.control_value(), p.matrix())
                }
                SimulationOp::CaptureProbability { controls, .. } => {
                    ParityStep::Probability(controls.mask(), controls.value())
                }
                SimulationOp::CaptureBloch {
                    qubit_bit,
                    controls,
                    ..
                } => ParityStep::Bloch(qubit_bit.as_u32(), controls.mask(), controls.value()),
                SimulationOp::CaptureAmplitude { controls, .. } => {
                    ParityStep::Amplitude(controls.mask(), controls.value())
                }
                SimulationOp::CaptureDensity { controls, .. } => {
                    ParityStep::Density(controls.mask(), controls.value())
                }
                SimulationOp::SnapshotState { .. } => ParityStep::Snapshot,
                _ => panic!("unexpected parity operation"),
            })
            .collect()
    }

    #[test]
    fn parity_z_gathers_and_scatters_before_snapshot() {
        use GateKind::*;
        assert_eq!(
            parity_stream(&[(ZParityControl, 0), (ZParityControl, 1), (X, 2)], 1),
            vec![
                parity_step(X, 0, 2, 2),
                parity_step(X, 2, 1, 1),
                parity_step(X, 0, 2, 2),
                ParityStep::Snapshot
            ]
        );
    }

    #[test]
    fn parity_x_rotates_all_wires_before_gather_and_after_scatter() {
        use GateKind::*;
        assert_eq!(
            parity_stream(&[(XParityControl, 0), (XParityControl, 1), (X, 2)], 1),
            vec![
                parity_step(H, 0, 0, 0),
                parity_step(H, 1, 0, 0),
                parity_step(X, 0, 2, 2),
                parity_step(X, 2, 1, 1),
                parity_step(X, 0, 2, 2),
                parity_step(H, 1, 0, 0),
                parity_step(H, 0, 0, 0),
                ParityStep::Snapshot
            ]
        );
    }

    #[test]
    fn parity_mixed_axes_choose_top_wire_independent_of_gate_order() {
        use GateKind::*;
        assert_eq!(
            parity_stream(&[(XParityControl, 2), (ZParityControl, 0), (X, 3)], 0),
            vec![
                parity_step(H, 2, 0, 0),
                parity_step(X, 0, 4, 4),
                parity_step(X, 3, 1, 1),
                parity_step(X, 0, 4, 4),
                parity_step(H, 2, 0, 0)
            ]
        );
    }

    #[test]
    fn parity_is_anded_with_ordinary_and_anti_controls() {
        use GateKind::*;
        assert_eq!(
            parity_stream(
                &[
                    (ZParityControl, 0),
                    (ZParityControl, 1),
                    (Control, 2),
                    (AntiControl, 3),
                    (X, 4)
                ],
                0
            ),
            vec![
                parity_step(X, 0, 2, 2),
                parity_step(X, 4, 13, 5),
                parity_step(X, 0, 2, 2)
            ]
        );
    }

    #[test]
    fn parity_three_wires_condition_on_odd_not_all_set() {
        use GateKind::*;
        assert_eq!(
            parity_stream(
                &[
                    (ZParityControl, 0),
                    (ZParityControl, 1),
                    (ZParityControl, 2),
                    (X, 3)
                ],
                0
            ),
            vec![
                parity_step(X, 0, 2, 2),
                parity_step(X, 0, 4, 4),
                parity_step(X, 3, 1, 1),
                parity_step(X, 0, 4, 4),
                parity_step(X, 0, 2, 2)
            ]
        );
    }

    #[test]
    fn parity_markers_without_targets_do_not_create_controlled_z() {
        use GateKind::*;
        assert_eq!(
            parity_stream(
                &[
                    (ZParityControl, 0),
                    (ZParityControl, 1),
                    (Control, 2),
                    (Control, 3)
                ],
                0
            ),
            vec![parity_step(X, 0, 2, 2), parity_step(X, 0, 2, 2)]
        );
    }

    #[test]
    fn parity_single_z_marker_is_only_a_control() {
        use GateKind::*;
        assert_eq!(
            parity_stream(&[(ZParityControl, 0), (X, 1)], 0),
            vec![parity_step(X, 1, 1, 1)]
        );
    }

    #[test]
    fn parity_sample_captures_gathered_condition_before_cleanup() {
        use GateKind::*;
        assert_eq!(
            parity_stream(
                &[
                    (XParityControl, 0),
                    (ZParityControl, 1),
                    (Control, 2),
                    (SampleDisplay, 3)
                ],
                1
            ),
            vec![
                parity_step(H, 0, 0, 0),
                parity_step(X, 0, 2, 2),
                ParityStep::Probability(5, 5),
                parity_step(X, 0, 2, 2),
                parity_step(H, 0, 0, 0),
                ParityStep::Snapshot
            ]
        );
    }

    #[test]
    fn parity_read_only_displays_capture_before_scatter() {
        use GateKind::*;
        assert_eq!(
            parity_stream(
                &[
                    (ZParityControl, 0),
                    (ZParityControl, 1),
                    (BlochDisplay, 2),
                    (ProbabilityDisplay, 3),
                    (AmplitudeDisplay, 4),
                    (DensityMatrixDisplay, 5)
                ],
                0
            ),
            vec![
                parity_step(X, 0, 2, 2),
                ParityStep::Bloch(2, 1, 1),
                ParityStep::Probability(1, 1),
                ParityStep::Amplitude(1, 1),
                ParityStep::Density(1, 1),
                parity_step(X, 0, 2, 2)
            ]
        );
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

    fn detector_gate(id: u32, kind: GateKind, column: usize, wire: usize) -> PlacedGate {
        PlacedGate::new(
            GateId::from_u32(id),
            kind,
            crate::app::CircuitColumnIndex::new(column),
            crate::app::WireIndex::new(wire),
            crate::gates::GateSpan::SINGLE,
            None,
        )
    }

    #[test]
    fn detector_measures_before_controlled_ops_and_resets_before_snapshot() {
        let gates = [
            detector_gate(1, GateKind::ZDetectControlReset, 0, 0),
            detector_gate(2, GateKind::X, 0, 1),
        ];
        let ops = linearize_ops(&gates, qubit_count(2), 1);
        assert!(matches!(
            ops.as_slice(),
            [SimulationOp::MeasureReduceSample { output_slot, .. },
             SimulationOp::MeasureCollapse { aux_slot, .. },
             SimulationOp::ApplyGate(target),
             SimulationOp::MeasureReduceSample { visible: false, output_slot: reset_slot, .. },
             SimulationOp::MeasureCollapse { aux_slot: reset_aux, .. },
             SimulationOp::ApplyGate(reset),
             SimulationOp::SnapshotState { .. }]
            if output_slot == aux_slot
                && target.control_mask() == 1
                && target.control_value() == 1
                && reset.condition_slot() == reset_slot.as_u32()
                && reset_slot == reset_aux && reset_slot != output_slot
                && reset.mode() == 0 && reset.control_mask() == 0
        ));
    }

    #[test]
    fn detector_control_does_not_leak_to_later_column() {
        let gates = [
            detector_gate(1, GateKind::ZDetectControlReset, 0, 0),
            detector_gate(2, GateKind::X, 0, 1),
            detector_gate(3, GateKind::X, 1, 1),
        ];
        let ops = linearize_ops(&gates, qubit_count(2), 0);
        let params = match ops.last().expect("later column op") {
            SimulationOp::ApplyGate(params) => *params,
            _ => panic!("expected later column gate"),
        };
        assert_eq!(
            (params.control_mask(), params.condition_slot()),
            (0, crate::gates::GATE_UNCONDITIONAL)
        );
    }

    #[test]
    fn detector_without_target_does_not_emit_controls_only_phase() {
        let gates = [
            detector_gate(1, GateKind::ZDetectControlReset, 0, 0),
            detector_gate(2, GateKind::Control, 0, 1),
        ];
        let ops = linearize_ops(&gates, qubit_count(2), 0);
        assert!(matches!(
            ops.as_slice(),
            [SimulationOp::MeasureReduceSample { visible: true, .. },
             SimulationOp::MeasureCollapse { .. },
             SimulationOp::MeasureReduceSample { visible: false, output_slot, .. },
             SimulationOp::MeasureCollapse { .. },
             SimulationOp::ApplyGate(reset)]
            if reset.condition_slot() == output_slot.as_u32() && reset.control_mask() == 0
        ));
    }

    #[test]
    fn write0_lowers_at_target_position_to_measure_collapse_and_conditional_x() {
        let gates = flagged_gates(r#"{"cols":[["•","|0>","X"]]}"#);
        let ops = linearize_ops(&gates, qubit_count(3), 1);
        assert!(matches!(ops.as_slice(), [
            SimulationOp::MeasureReduceSample { visible: false, output_slot, controls, .. },
            SimulationOp::MeasureCollapse { aux_slot, .. },
            SimulationOp::ApplyGate(reset), SimulationOp::ApplyGate(other),
            SimulationOp::SnapshotState { .. }
        ] if output_slot == aux_slot && controls.mask() == 1
            && reset.mode() == 0 && reset.bit() == 1
            && reset.control_mask() == 1 && reset.condition_slot() == output_slot.as_u32()
            && other.bit() == 2));
    }

    #[test]
    fn measurement_pair_shares_one_slot() {
        let gates = [PlacedGate::new(
            crate::app::GateId::from_u32(1),
            GateKind::Measurement,
            crate::app::CircuitColumnIndex::new(0),
            crate::app::WireIndex::new(0),
            crate::gates::GateSpan::SINGLE,
            None,
        )];
        let ops = linearize_ops(&gates, qubit_count(1), 0);
        let reduce_slot = ops.iter().find_map(|op| match op {
            SimulationOp::MeasureReduceSample { output_slot, .. } => Some(*output_slot),
            _ => None,
        });
        let collapse_slot = ops.iter().find_map(|op| match op {
            SimulationOp::MeasureCollapse { aux_slot, .. } => Some(*aux_slot),
            _ => None,
        });

        assert_eq!(
            (reduce_slot, collapse_slot),
            (Some(SlotIndex::new(0)), Some(SlotIndex::new(0)))
        );
    }

    #[test]
    fn two_controls_emit_controlled_z() {
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
            (1, 0b001, 0b001)
        );
    }

    #[test]
    fn controlled_z_drops_anti_control_bits() {
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
            (1, 0b001, 0b001)
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

        let params = apply_qft_expanded(
            &linearize_ops(&gates, qubit_count(3), 0),
            QftExpandedKind::Hadamard,
            1,
        );

        assert_eq!(
            (params.bit(), params.control_mask(), params.control_value()),
            (1, 0b001, 0b001)
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

        let params = apply_qft_expanded(
            &linearize_ops(&gates, qubit_count(3), 0),
            QftExpandedKind::Phase,
            1,
        );

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

        let params = apply_qft_expanded(
            &linearize_ops(&gates, qubit_count(3), 0),
            QftExpandedKind::Hadamard,
            1,
        );

        assert_eq!(
            (params.bit(), params.control_mask(), params.control_value()),
            (1, 0b001, 0)
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

        let params = apply_qft_expanded(
            &linearize_ops(&gates, qubit_count(2), 0),
            QftExpandedKind::Hadamard,
            0,
        );

        assert_eq!(
            (params.bit(), params.control_mask(), params.control_value()),
            (0, 0, 0)
        );
    }

    #[test]
    fn qft_emits_leading_bit_reversal_swap() {
        let gates = [PlacedGate::new(
            crate::app::GateId::from_u32(1),
            GateKind::QftGate,
            crate::app::CircuitColumnIndex::new(0),
            crate::app::WireIndex::new(0),
            crate::gates::GateSpan::try_new(2).unwrap(),
            None,
        )];

        let params = apply_gate_params(&linearize_ops(&gates, qubit_count(2), 0), 0);

        assert_eq!(
            (params.bit(), params.control_mask(), params.control_value()),
            (1, 0b01, 0b01)
        );
    }

    #[test]
    fn qft_dagger_emits_trailing_bit_reversal_swap() {
        let gates = [PlacedGate::new(
            crate::app::GateId::from_u32(1),
            GateKind::QftDaggerGate,
            crate::app::CircuitColumnIndex::new(0),
            crate::app::WireIndex::new(0),
            crate::gates::GateSpan::try_new(2).unwrap(),
            None,
        )];

        let params = apply_trailing_qft_swap(&linearize_ops(&gates, qubit_count(2), 0), 1);

        assert_eq!(
            (params.bit(), params.control_mask(), params.control_value()),
            (1, 0b01, 0b01)
        );
    }

    #[derive(Debug, PartialEq, Eq)]
    enum PhysicsOp {
        Apply(u32, u32, u32, u32, [i64; 8]),
        Arithmetic(u32, u32, u32, u32, u32),
        MeasureReduceSample(crate::qubit_bit::QubitBit, ColumnControls),
        MeasureCollapse(crate::qubit_bit::QubitBit, ColumnControls),
    }

    fn physics_summary(json: &str, qubits: usize) -> Vec<PhysicsOp> {
        let gates = crate::url_circuit::parse_circuit_json(json).gates;
        linearize_ops(&gates, qubit_count(qubits), 0)
            .iter()
            .filter_map(|op| match op {
                SimulationOp::ApplyGate(params) => {
                    let m = params.matrix();
                    let q = |value: f32| (value * 1_000_000.0).round() as i64;
                    Some(PhysicsOp::Apply(
                        params.bit(),
                        params.control_mask(),
                        params.control_value(),
                        params.mode(),
                        [
                            q(m[0][0]),
                            q(m[0][1]),
                            q(m[1][0]),
                            q(m[1][1]),
                            q(m[2][0]),
                            q(m[2][1]),
                            q(m[3][0]),
                            q(m[3][1]),
                        ],
                    ))
                }
                SimulationOp::MeasureReduceSample {
                    qubit_bit,
                    controls,
                    ..
                } => Some(PhysicsOp::MeasureReduceSample(*qubit_bit, *controls)),
                SimulationOp::MeasureCollapse {
                    qubit_bit,
                    controls,
                    ..
                } => Some(PhysicsOp::MeasureCollapse(*qubit_bit, *controls)),
                SimulationOp::Arithmetic(params) => Some(PhysicsOp::Arithmetic(
                    params.target_shift,
                    params.target_mask,
                    params.amount,
                    params.control_mask,
                    params.control_value,
                )),
                SimulationOp::SnapshotState { .. }
                | SimulationOp::CaptureBloch { .. }
                | SimulationOp::CaptureProbability { .. }
                | SimulationOp::CaptureAmplitude { .. }
                | SimulationOp::CaptureDensity { .. } => None,
                SimulationOp::Postselect { .. } => {
                    panic!("sample unexpectedly contains postselection")
                }
            })
            .collect()
    }

    #[test]
    fn physics_summary_distinguishes_arithmetic_directions() {
        assert_ne!(
            physics_summary(r#"{"cols":[["inc2"]]}"#, 2),
            physics_summary(r#"{"cols":[["dec2"]]}"#, 2),
        );
    }

    #[test]
    fn physics_summary_distinguishes_written_values() {
        assert_ne!(
            physics_summary(r#"{"cols":[["|0>"]]}"#, 1),
            physics_summary(r#"{"cols":[["|1>"]]}"#, 1),
        );
    }

    #[test]
    fn bell_sample_preserves_physics() {
        let baseline = r#"{"cols":[["H"],["•","X"]]}"#;
        assert_eq!(
            physics_summary(qni_web_circuit_library_model::BELL_JSON, 2),
            physics_summary(baseline, 2),
        );
    }

    #[test]
    fn ghz_sample_preserves_physics() {
        let baseline = r#"{"cols":[["H"],["•","X"],["•",1,"X"]]}"#;
        assert_eq!(
            physics_summary(qni_web_circuit_library_model::GHZ_JSON, 3),
            physics_summary(baseline, 3),
        );
    }

    #[test]
    fn teleportation_sample_preserves_physics() {
        let baseline = r#"{"cols":[[1,"H"],[1,"•","X"],["H"],["T"],["•","X"],["H"],["Measure","Measure"],[1,"•","X"],["•",1,"Z"]]}"#;
        assert_eq!(
            physics_summary(qni_web_circuit_library_model::TELEPORTATION_JSON, 3),
            physics_summary(baseline, 3),
        );
    }

    #[test]
    fn superdense_coding_sample_preserves_physics() {
        let baseline = r#"{"cols":[[1,1,1,"H"],[1,1,1,"•","X"],[1,1,"Swap","Swap"],[1,1,1,1,"Swap","Swap"],["|1>","|1>"],["•",1,"Z"],[1,"•","X"],[1,1,"Swap",1,"Swap"],[1,1,1,1,"•","X"],[1,1,1,1,"H"],[1,1,1,1,"Measure","Measure"]]}"#;
        assert_eq!(
            physics_summary(qni_web_circuit_library_model::SUPERDENSE_CODING_JSON, 6),
            physics_summary(baseline, 6),
        );
    }

    #[test]
    fn qft4_sample_matches_input_plus_native_qft4() {
        let baseline = r#"{"cols":[[1,"H","H","H"],["QFT4"]]}"#;
        assert_eq!(
            physics_summary(qni_web_circuit_library_model::QFT4_DECOMPOSED_JSON, 4),
            physics_summary(baseline, 4),
        );
    }

    #[test]
    fn symmetry_breaking_sample_preserves_physics() {
        let baseline = r#"{"cols":[["H",1,"H"],["•","X"],[1,1,"•","X"],[1,"Swap",1,"Swap"],["•","X"],[1,1,"•","X"],["X^½","◦"],[1,1,"X^½","◦"],[1,"X^½",1,"X^½"],["Measure","Measure","Measure","Measure"]]}"#;
        assert_eq!(
            physics_summary(qni_web_circuit_library_model::SYMMETRY_BREAKING_JSON, 4),
            physics_summary(baseline, 4),
        );
    }

    #[test]
    fn grover_search_sample_preserves_physics() {
        let baseline = r#"{"cols":[["X","X","X","X","X"],["H","H","H","H","H"],["Z","•","◦","•","•"],["H","H","H","H"],["•","•","•","•","X"],["H","H","H","H"],["Z","•","◦","•","•"],["H","H","H","H"],["•","•","•","•","X"],["H","H","H","H"],["Z","•","◦","•","•"],["H","H","H","H"],["•","•","•","•","X"],["H","H","H","H"],["Z","•","◦","•","•"],["H","H","H","H"],["•","•","•","•","X"],["H","H","H","H"]]}"#;
        assert_eq!(
            physics_summary(qni_web_circuit_library_model::GROVER_SEARCH_JSON, 5),
            physics_summary(baseline, 5),
        );
    }

    #[test]
    fn reversible_addition_sample_preserves_physics() {
        let baseline = r#"{"cols":[["X",1,"X",1,1,"X","X"],["X","X","X","X","•","X","X","X","X","X"],[1,1,1,1,"•","X"],["Swap",1,1,1,"Swap","•"],[1,1,1,1,"•",1,"X"],[1,"Swap",1,1,"Swap",1,"•"],[1,1,1,1,"•",1,1,"X"],[1,1,"Swap",1,"Swap",1,1,"•"],[1,1,1,1,"•",1,1,1,"X"],[1,1,1,"Swap","Swap",1,1,1,"•"],[1,1,1,1,"•",1,1,1,1,"X"],[1,1,1,"Swap","Swap",1,1,1,"•"],[1,1,1,"•",1,1,1,1,"X"],[1,1,"Swap",1,"Swap",1,1,"•"],[1,1,"•",1,1,1,1,"X"],[1,"Swap",1,1,"Swap",1,"•"],[1,"•",1,1,1,1,"X"],["Swap",1,1,1,"Swap","•"],["•",1,1,1,1,"X"],["X","X","X","X","•","X","X","X","X","X"]]}"#;
        assert_eq!(
            physics_summary(qni_web_circuit_library_model::REVERSIBLE_ADDITION_JSON, 10),
            physics_summary(baseline, 10),
        );
    }

    #[test]
    fn delayed_choice_eraser_sample_preserves_physics() {
        let baseline = r#"{"cols":[[1,"H"],[1,"•",1,1,"X"],[1,1,"QFT7"],["H"],["•","X^½"]]}"#;
        assert_eq!(
            physics_summary(qni_web_circuit_library_model::DELAYED_CHOICE_ERASER_JSON, 9),
            physics_summary(baseline, 9),
        );
    }

    #[test]
    fn decomposed_qft4_matches_native_qft4_ops() {
        let native = physics_summary(r#"{"cols":[[1,"H","H","H"],["QFT4"]]}"#, 4);
        let decomposed = physics_summary(qni_web_circuit_library_model::QFT4_DECOMPOSED_JSON, 4);

        assert_eq!(decomposed, native);
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
                if controls.mask() == 0b1 && controls.value() == 0b1
        ));
    }

    #[test]
    fn probability_display_base_bit_is_span_top() {
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
            Some(SimulationOp::CaptureProbability { base_bit, .. }) if base_bit.as_u32() == 0
        ));
    }

    #[test]
    fn amplitude_display_captures_column_controls() {
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
                GateKind::AmplitudeDisplay,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(1),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
        ];

        let ops = linearize_ops(&gates, qubit_count(2), 0);

        assert!(matches!(
            ops.first(),
            Some(SimulationOp::CaptureAmplitude { controls, .. })
                if controls.mask() == 0b1 && controls.value() == 0b1
        ));
    }

    #[test]
    fn density_display_captures_column_controls() {
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
                GateKind::DensityMatrixDisplay,
                crate::app::CircuitColumnIndex::new(0),
                crate::app::WireIndex::new(1),
                crate::gates::GateSpan::SINGLE,
                None,
            ),
        ];

        let ops = linearize_ops(&gates, qubit_count(2), 0);

        assert!(matches!(
            ops.first(),
            Some(SimulationOp::CaptureDensity { controls, .. })
                if controls.mask() == 0b1 && controls.value() == 0
        ));
    }

    #[test]
    fn density_display_caps_span_at_eight() {
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
                if controls.mask() == 0b1 && controls.value() == 0
        ));
    }

    #[test]
    fn bloch_display_op_carries_gate_id() {
        let gates = [PlacedGate::new(
            crate::app::GateId::from_u32(5),
            GateKind::BlochDisplay,
            crate::app::CircuitColumnIndex::new(0),
            crate::app::WireIndex::new(0),
            crate::gates::GateSpan::SINGLE,
            None,
        )];

        let ops = linearize_ops(&gates, qubit_count(1), 0);

        assert!(matches!(
            ops.first(),
            Some(SimulationOp::CaptureBloch { gate_id, .. })
                if *gate_id == crate::app::GateId::from_u32(5)
        ));
    }

    fn linearized(json: &str) -> String {
        let gates = crate::url_circuit::parse_circuit_json(json).gates;
        format!("{:?}", linearize_ops(&gates, qubit_count(2), 0))
    }

    #[test]
    fn circuit_blocks_do_not_change_the_simulation_ops() {
        assert_eq!(
            linearized(
                r#"{"cols":[["|0>","|0>"],["{量子もつれ"],["H"],["•","X"],["}"],["Measure"],[1,"Measure"]]}"#
            ),
            linearized(r#"{"cols":[["|0>","|0>"],["H"],["•","X"],["Measure"],[1,"Measure"]]}"#)
        );
    }

    fn flagged_gates(json: &str) -> Vec<PlacedGate> {
        crate::url_circuit::parse_circuit_json(json).gates
    }

    fn apply_gate_count(ops: &[SimulationOp]) -> usize {
        ops.iter()
            .filter(|op| matches!(op, SimulationOp::ApplyGate(_)))
            .count()
    }

    #[test]
    fn conditional_gate_reads_the_measurement_aux_slot() {
        let gates = flagged_gates(r#"{"cols":[["Measure>a"],[1,"X<a"]]}"#);
        let ops = linearize_ops(&gates, qubit_count(2), 0);

        assert_eq!(apply_gate_params(&ops, 2).condition_slot(), 0);
    }

    #[test]
    fn conditional_gate_reads_the_latest_measurement_slot() {
        let gates = flagged_gates(r#"{"cols":[["Measure>a"],[1,"Measure>a"],[1,1,"X<a"]]}"#);
        let ops = linearize_ops(&gates, qubit_count(3), 0);

        assert_eq!(apply_gate_params(&ops, 4).condition_slot(), 1);
    }

    #[test]
    fn plain_gate_is_unconditional() {
        let gates = flagged_gates(r#"{"cols":[["Measure>a"],[1,"X"]]}"#);
        let ops = linearize_ops(&gates, qubit_count(2), 0);

        assert_eq!(
            apply_gate_params(&ops, 2).condition_slot(),
            crate::gates::GATE_UNCONDITIONAL
        );
    }

    #[test]
    fn conditional_gate_without_a_measurement_is_dropped() {
        let gates = flagged_gates(r#"{"cols":[["H"],["X<a"]]}"#);
        let ops = linearize_ops(&gates, qubit_count(1), 0);

        assert_eq!(apply_gate_count(&ops), 1);
    }

    #[test]
    fn controlled_conditional_gate_keeps_its_controls() {
        let gates = flagged_gates(r#"{"cols":[["Measure>a"],["•","X<a"]]}"#);
        let ops = linearize_ops(&gates, qubit_count(2), 0);

        assert_eq!(apply_gate_params(&ops, 2).control_mask(), 0b01);
    }

    fn phase_ops(json: &str, qubits: usize) -> Vec<SimulationOp> {
        linearize_ops(&flagged_gates(json), qubit_count(qubits), 0)
    }

    fn phase_controls(json: &str, qubits: usize) -> (u32, u32, u32) {
        let params = apply_gate_params(&phase_ops(json, qubits), 0);
        (params.bit(), params.control_mask(), params.control_value())
    }

    #[test]
    fn same_angle_phases_emit_one_controlled_phase() {
        assert_eq!(
            apply_gate_count(&phase_ops(r#"{"cols":[["P(π_4)","P(π_4)"]]}"#, 2)),
            1
        );
    }

    #[test]
    fn cphase_targets_the_top_wire_and_controls_the_others() {
        assert_eq!(
            phase_controls(r#"{"cols":[["P(π_4)",1,"P(π_4)"]]}"#, 3),
            (0, 0b100, 0b100)
        );
    }

    #[test]
    fn ccphase_controls_every_other_phase_wire() {
        assert_eq!(
            phase_controls(r#"{"cols":[["P(π_4)","P(π_4)","P(π_4)"]]}"#, 3),
            (0, 0b110, 0b110)
        );
    }

    #[test]
    fn cphase_joins_column_controls() {
        assert_eq!(
            phase_controls(r#"{"cols":[["P(π_4)","•","P(π_4)"]]}"#, 3),
            (0, 0b110, 0b110)
        );
    }

    #[test]
    fn cphase_joins_column_anti_controls() {
        assert_eq!(
            phase_controls(r#"{"cols":[["P(π_4)","◦","P(π_4)"]]}"#, 3),
            (0, 0b110, 0b100)
        );
    }

    #[test]
    fn bare_phase_groups_with_explicit_default_angle() {
        assert_eq!(
            apply_gate_count(&phase_ops(r#"{"cols":[["P","P(π_2)"]]}"#, 2)),
            1
        );
    }

    #[test]
    fn different_angles_form_separate_phase_groups() {
        assert_eq!(
            apply_gate_count(&phase_ops(r#"{"cols":[["P(π_4)","P(π_2)","P(π_4)"]]}"#, 3)),
            2
        );
    }

    #[test]
    fn lone_phase_of_another_angle_keeps_only_column_controls() {
        assert_eq!(
            phase_controls(r#"{"cols":[["•","P(π_2)","P(π_4)","P(π_4)"]]}"#, 4),
            (1, 0b0001, 0b0001)
        );
    }

    fn conditional_phase_ops(
        json: &str,
        qubits: usize,
        flags: &[(usize, &str)],
    ) -> Vec<SimulationOp> {
        let mut gates = flagged_gates(json);
        for (index, name) in flags {
            gates[*index].flag = Some(crate::gates::GateFlag::If(
                crate::gates::FlagName::parse(name).expect("flag name"),
            ));
        }
        linearize_ops(&gates, qubit_count(qubits), 0)
    }

    #[test]
    fn different_conditions_form_separate_phase_groups() {
        let ops = conditional_phase_ops(
            r#"{"cols":[["Measure>a","Measure>b"],[1,1,"P(π_4)","P(π_4)"]]}"#,
            4,
            &[(2, "a"), (3, "b")],
        );

        assert_eq!(apply_gate_count(&ops), 2);
    }

    #[test]
    fn same_condition_phases_emit_one_conditional_cphase() {
        let ops = conditional_phase_ops(
            r#"{"cols":[["Measure>a"],[1,"P(π_4)","P(π_4)"]]}"#,
            3,
            &[(1, "a"), (2, "a")],
        );
        let params = apply_gate_params(&ops, 2);

        assert_eq!(
            (params.bit(), params.control_mask(), params.condition_slot()),
            (1, 0b100, 0)
        );
    }
    #[test]
    fn projector_normalizes_before_snapshot() {
        let gates = [PlacedGate::new(
            crate::app::GateId::from_u32(7),
            GateKind::PostSelectOn,
            crate::app::CircuitColumnIndex::new(0),
            crate::app::WireIndex::new(0),
            crate::gates::GateSpan::SINGLE,
            None,
        )];
        assert!(
            matches!(linearize_ops(&gates, qubit_count(1), 1).as_slice(),
            [SimulationOp::ApplyGate(_), SimulationOp::Postselect { gate_id, output_slot }, SimulationOp::SnapshotState { .. }]
            if gate_id.as_u32() == 7 && output_slot.as_u32() == 0)
        );
    }
    #[test]
    fn successive_projectors_have_dedicated_slots() {
        let gates = flagged_gates(r#"{"cols":[["|0⟩⟨0|"],["|1⟩⟨1|"]]}"#);
        let slots: Vec<_> = linearize_ops(&gates, qubit_count(1), 0)
            .iter()
            .filter_map(|op| match op {
                SimulationOp::Postselect { output_slot, .. } => Some(output_slot.as_u32()),
                _ => None,
            })
            .collect();
        assert_eq!(slots, vec![0, 1]);
    }
    #[test]
    fn same_column_projectors_normalize_sequentially_before_measurement_and_capture() {
        let gates = flagged_gates(r#"{"cols":[["|0⟩⟨0|","|1⟩⟨1|","Measure","Probability"]]}"#);
        assert!(
            matches!(linearize_ops(&gates, qubit_count(4), 1).as_slice(), [
            SimulationOp::ApplyGate(_), SimulationOp::Postselect { output_slot: first, .. },
            SimulationOp::ApplyGate(_), SimulationOp::Postselect { output_slot: second, .. },
            SimulationOp::MeasureReduceSample { .. }, SimulationOp::MeasureCollapse { .. },
            SimulationOp::CaptureProbability { .. }, SimulationOp::SnapshotState { .. },
        ] if first.as_u32() == 0 && second.as_u32() == 1)
        );
    }

    #[test]
    fn parity_projection_normalizes_before_scattering_and_snapshot() {
        let gates = flagged_gates(r#"{"cols":[["zpar","zpar","|0⟩⟨0|"]]}"#);
        assert!(matches!(
            linearize_ops(&gates, qubit_count(3), 1).as_slice(),
            [SimulationOp::ApplyGate(gather), SimulationOp::ApplyGate(projector),
             SimulationOp::Postselect { .. }, SimulationOp::ApplyGate(scatter),
             SimulationOp::SnapshotState { .. }]
            if gather.bit() == 0 && gather.control_mask() == 2
                && projector.bit() == 2 && projector.control_mask() == 1
                && scatter.bit() == gather.bit()
                && scatter.control_mask() == gather.control_mask()
                && scatter.control_value() == gather.control_value()
                && scatter.matrix() == gather.matrix()
        ));
    }

    #[test]
    fn all_projector_kinds_emit_normalization() {
        let gates = flagged_gates(r#"{"cols":[["|0⟩⟨0|"],["|1⟩⟨1|"],["|+⟩⟨+|"],["|-⟩⟨-|"]]}"#);
        assert_eq!(
            linearize_ops(&gates, qubit_count(1), 0)
                .iter()
                .filter(|op| matches!(op, SimulationOp::Postselect { .. }))
                .count(),
            4
        );
    }
}
