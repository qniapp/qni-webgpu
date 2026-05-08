//! CPU-side circuit simulator that mirrors GPU per-gate semantics, captures
//! Bloch vectors at every `BlochDisplay`, and samples / collapses on every
//! `Measurement`. The result is authoritative for the visualization: callers
//! upload `final_state` straight to the GPU state buffer (see `gpu.rs`).
//!
//! qni references:
//! - Per-pair logic in `STATE_COMPUTE_SHADER` mirrors `applyControlledGate` in
//!   `packages/simulator/src/matrix.ts`.
//! - Bloch reduction follows `packages/simulator/src/state-vector.ts`
//!   (`qubitDensityMatrix`, `blochVector`).
//! - Measurement sampling/collapse follows `packages/simulator/src/simulator.ts`
//!   (`measure`): pick a uniform random `r`; if `r ≤ pZero` collapse to |0⟩,
//!   otherwise to |1⟩; in either branch divide surviving amplitudes by the
//!   square root of the kept probability so the state stays normalized.

use std::collections::HashMap;

use crate::app::PlacedGate;
use crate::constants::{GATE_SIZE, SNAP_DISTANCE};
use crate::gates::{gate_matrix, GateKind, GateMatrix};
use crate::layout::{nearest_slot_index, LayoutMetrics};

/// Bloch vector for a single qubit, in qni's convention.
pub(crate) type BlochVector = [f32; 3];

pub(crate) struct SimulationResult {
    pub(crate) final_state: Vec<[f32; 2]>,
    pub(crate) bloch_vectors: HashMap<u32, BlochVector>,
    pub(crate) measurements: HashMap<u32, u8>,
}

pub(crate) fn simulate(
    placed_gates: &[PlacedGate],
    qubits: usize,
    metrics: &LayoutMetrics,
) -> SimulationResult {
    let qubits = qubits.max(1);
    let state_count = 1usize << qubits;
    let mut state = vec![[0.0f32; 2]; state_count];
    state[0] = [1.0, 0.0];
    let mut bloch_vectors: HashMap<u32, BlochVector> = HashMap::new();
    let mut measurements: HashMap<u32, u8> = HashMap::new();

    if metrics.slot_centers.is_empty() {
        return SimulationResult {
            final_state: state,
            bloch_vectors,
            measurements,
        };
    }

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
                GateKind::Swap => {
                    // Mirror collect_gate_params: SWAP is currently unimplemented.
                }
                GateKind::BlochDisplay => bloch_targets.push(gate),
                GateKind::Measurement => measurement_targets.push(gate),
                _ => targets.push(gate),
            }
        }

        targets.sort_by(|a, b| a.id.cmp(&b.id));
        for target in &targets {
            let bit = (qubits - 1 - target.wire) as u32;
            let mat = gate_matrix(target.kind);
            apply_2x2_with_mode(
                &mut state,
                bit,
                target.kind,
                &mat,
                control_mask,
                control_value,
            );
        }

        // Apply measurements after the column's unitaries (qni runs them in
        // step order, so a measurement in the same column observes the
        // post-column state). Sort by id so a stable RNG seed gives stable
        // outcomes when the user re-renders the same circuit.
        measurement_targets.sort_by(|a, b| a.id.cmp(&b.id));
        for measurement in &measurement_targets {
            let bit = (qubits - 1 - measurement.wire) as u32;
            let outcome = measure(&mut state, bit, qubits, measurement.id);
            measurements.insert(measurement.id, outcome);
        }

        for display in &bloch_targets {
            let bit = (qubits - 1 - display.wire) as u32;
            bloch_vectors.insert(display.id, bloch_vector(&state, bit, qubits));
        }
    }

    SimulationResult {
        final_state: state,
        bloch_vectors,
        measurements,
    }
}

fn cmul(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] * b[0] - a[1] * b[1], a[0] * b[1] + a[1] * b[0]]
}

fn cadd(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] + b[0], a[1] + b[1]]
}

fn apply_2x2_with_mode(
    state: &mut [[f32; 2]],
    target_bit: u32,
    kind: GateKind,
    matrix: &GateMatrix,
    control_mask: u32,
    control_value: u32,
) {
    let total = state.len() as u32;
    let stride = 1u32 << target_bit;
    let mask = stride - 1;
    let pairs = total / 2;
    for pair in 0..pairs {
        let low = pair & mask;
        let high = pair >> target_bit;
        let i0 = ((high << (target_bit + 1)) | low) as usize;
        let i1 = i0 | (stride as usize);
        if control_mask != 0 && (i0 as u32 & control_mask) != control_value {
            continue;
        }
        let a0 = state[i0];
        let a1 = state[i1];

        match kind {
            GateKind::Write0 => {
                let mag0 = a0[0] * a0[0] + a0[1] * a0[1];
                let mag1 = a1[0] * a1[0] + a1[1] * a1[1];
                if mag1 > mag0 + 1.0e-6 {
                    state[i0] = a1;
                    state[i1] = a0;
                }
            }
            GateKind::Write1 => {
                let mag0 = a0[0] * a0[0] + a0[1] * a0[1];
                let mag1 = a1[0] * a1[0] + a1[1] * a1[1];
                if mag0 > mag1 + 1.0e-6 {
                    state[i0] = a1;
                    state[i1] = a0;
                }
            }
            _ => {
                state[i0] = cadd(cmul(matrix.m00, a0), cmul(matrix.m01, a1));
                state[i1] = cadd(cmul(matrix.m10, a0), cmul(matrix.m11, a1));
            }
        }
    }
}

fn bloch_vector(state: &[[f32; 2]], qubit_bit: u32, qubits: usize) -> BlochVector {
    let qubit_mask = 1u32 << qubit_bit;
    let mut rho_00 = 0.0f32;
    let mut rho_11 = 0.0f32;
    let mut rho_01_re = 0.0f32;
    let mut rho_01_im = 0.0f32;
    let total = 1usize << qubits;
    for i in 0..total {
        let amp = state[i];
        if (i as u32 & qubit_mask) == 0 {
            rho_00 += amp[0] * amp[0] + amp[1] * amp[1];
            let j = i | (qubit_mask as usize);
            let amp_j = state[j];
            rho_01_re += amp[0] * amp_j[0] + amp[1] * amp_j[1];
            rho_01_im += amp[1] * amp_j[0] - amp[0] * amp_j[1];
        } else {
            rho_11 += amp[0] * amp[0] + amp[1] * amp[1];
        }
    }
    let x = 2.0 * rho_01_re;
    let y = -2.0 * rho_01_im;
    let z = rho_00 - rho_11;
    [x, y, z]
}

/// Deterministic measurement on `qubit_bit`. Returns the observed outcome
/// (0 or 1) and collapses `state` in place. The RNG is seeded by `seed`
/// (gate id) so the same circuit renders the same outcome every frame.
fn measure(state: &mut [[f32; 2]], qubit_bit: u32, qubits: usize, seed: u32) -> u8 {
    let qubit_mask = 1u32 << qubit_bit;
    let total = 1usize << qubits;
    let mut p_zero = 0.0f32;
    for i in 0..total {
        if (i as u32 & qubit_mask) == 0 {
            let amp = state[i];
            p_zero += amp[0] * amp[0] + amp[1] * amp[1];
        }
    }
    let r = rand_unit(seed);
    if r <= p_zero {
        let norm = p_zero.sqrt().max(1.0e-12);
        for i in 0..total {
            if (i as u32 & qubit_mask) != 0 {
                state[i] = [0.0, 0.0];
            } else {
                state[i] = [state[i][0] / norm, state[i][1] / norm];
            }
        }
        0
    } else {
        let norm = (1.0 - p_zero).sqrt().max(1.0e-12);
        for i in 0..total {
            if (i as u32 & qubit_mask) == 0 {
                state[i] = [0.0, 0.0];
            } else {
                state[i] = [state[i][0] / norm, state[i][1] / norm];
            }
        }
        1
    }
}

/// xorshift-mix style hash → uniform [0, 1). Stable across runs for the same
/// seed; not cryptographic but good enough for visual sampling.
fn rand_unit(seed: u32) -> f32 {
    let mut state = seed
        .wrapping_mul(0x9E3779B9)
        .wrapping_add(0x85EBCA6B);
    state ^= state >> 13;
    state = state.wrapping_mul(0xC2B2AE35);
    state ^= state >> 16;
    (state as f32) / (u32::MAX as f32)
}
