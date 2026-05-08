//! CPU-side per-step simulator that captures Bloch vectors at each
//! `BlochDisplay` placement.
//!
//! qni references:
//! - `packages/simulator/src/state-vector.ts` (`qubitDensityMatrix`,
//!   `blochVector`)
//! - `packages/simulator/src/matrix.ts` (`qubitDensityMatrixToBlochVector`)
//!
//! qni applies the Bloch reduction `M = ½(I + v·σ)` to the per-qubit reduced
//! density matrix:
//!   x =  2·Re(ρ_01)
//!   y = -2·Im(ρ_01)
//!   z =  ρ_00 - ρ_11

use std::collections::HashMap;

use crate::app::PlacedGate;
use crate::constants::{GATE_SIZE, SNAP_DISTANCE};
use crate::gates::{gate_matrix, GateKind};
use crate::layout::{nearest_slot_index, LayoutMetrics};

/// Bloch vector for a single qubit, in qni's convention.
pub(crate) type BlochVector = [f32; 3];

/// Returns a map from `BlochDisplay` gate id to its (x, y, z) Bloch vector,
/// computed from the reduced density matrix at that gate's circuit column.
pub(crate) fn compute_bloch_vectors(
    placed_gates: &[PlacedGate],
    qubits: usize,
    metrics: &LayoutMetrics,
) -> HashMap<u32, BlochVector> {
    let mut result = HashMap::new();
    if qubits == 0 || metrics.slot_centers.is_empty() {
        return result;
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

    let state_count = 1usize << qubits;
    let mut state = vec![[0.0f32; 2]; state_count];
    state[0] = [1.0, 0.0];

    let mut slot_indices: Vec<usize> = by_slot.keys().copied().collect();
    slot_indices.sort();

    for slot in slot_indices {
        let column = by_slot.get(&slot).expect("slot exists");
        let mut control_mask = 0u32;
        let mut control_value = 0u32;
        let mut targets: Vec<&PlacedGate> = Vec::new();
        let mut bloch_targets: Vec<&PlacedGate> = Vec::new();

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
                    // mirror collect_gate_params: SWAP is currently unimplemented in
                    // the GPU pipeline, so the CPU mirror keeps it as a no-op too.
                }
                GateKind::BlochDisplay => bloch_targets.push(gate),
                _ => targets.push(gate),
            }
        }

        // Apply each target gate to the CPU state. Order within a column matches
        // the order the GPU sees: ascending min_id within slot is enforced by
        // the outer simulator; here we use the gate id ordering.
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

        // Capture Bloch vectors for any displays in this column, after the column's
        // gates have been applied (matching qni's semantics where the display reads
        // the post-column state).
        for display in &bloch_targets {
            let bit = (qubits - 1 - display.wire) as u32;
            result.insert(display.id, bloch_vector(&state, bit, qubits));
        }
    }

    result
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
    matrix: &crate::gates::GateMatrix,
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

/// Computes the Bloch vector for `qubit_bit` (bit index, qni convention)
/// from the current CPU state vector. Matches qni's
/// `qubitDensityMatrixToBlochVector` reduction.
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
            // ρ_01 = Σ_rest amp_i · conj(amp_j)
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
