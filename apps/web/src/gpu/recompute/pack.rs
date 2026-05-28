use eframe::wgpu;

use crate::gates::GateParams;
use crate::simulation_plan::SimulationOp;

use super::super::params::{
    AmplitudeCaptureParams, BlochParams, DensityCaptureParams, MeasureCollapseParams,
    MeasureReduceParams, ProbabilityReduceParams,
};
use super::super::resources::StateVectorResources;

/// Per-variant params packed in the same order each op variant appears in the
/// linear op stream. The dispatch encoder later copies each packed slot into
/// the tiny uniform buffer immediately before its dispatch.
pub(super) struct PackedRecomputeParams {
    gate: Vec<GateParams>,
    bloch: Vec<BlochParams>,
    measure_reduce: Vec<MeasureReduceParams>,
    measure_collapse: Vec<MeasureCollapseParams>,
    probability: Vec<ProbabilityReduceParams>,
    amplitude: Vec<AmplitudeCaptureParams>,
    density: Vec<DensityCaptureParams>,
}

impl PackedRecomputeParams {
    pub(super) fn from_ops(sim_ops: &[SimulationOp], state_count: usize, pair_count: u32) -> Self {
        let mut packed = Self {
            gate: Vec::with_capacity(sim_ops.len()),
            bloch: Vec::with_capacity(sim_ops.len()),
            measure_reduce: Vec::with_capacity(sim_ops.len()),
            measure_collapse: Vec::with_capacity(sim_ops.len()),
            probability: Vec::with_capacity(sim_ops.len()),
            amplitude: Vec::with_capacity(sim_ops.len()),
            density: Vec::with_capacity(sim_ops.len()),
        };

        for op in sim_ops {
            match op {
                SimulationOp::SnapshotState { .. } => {}
                SimulationOp::ApplyGate(params) => packed.gate.push(*params),
                SimulationOp::CaptureBloch {
                    qubit_bit,
                    output_slot,
                    control_mask,
                    control_value,
                    ..
                } => packed.bloch.push(BlochParams {
                    qubit_bit: qubit_bit.as_u32(),
                    state_count: state_count as u32,
                    output_slot: *output_slot,
                    control_mask: *control_mask,
                    control_value: *control_value,
                    _pad: [0; 3],
                }),
                SimulationOp::MeasureReduceSample {
                    gate_id,
                    qubit_bit,
                    output_slot,
                } => packed.measure_reduce.push(MeasureReduceParams {
                    qubit_bit: qubit_bit.as_u32(),
                    state_count: state_count as u32,
                    output_slot: *output_slot,
                    seed: *gate_id,
                }),
                SimulationOp::MeasureCollapse {
                    qubit_bit,
                    aux_slot,
                } => {
                    if pair_count == 0 {
                        continue;
                    }
                    packed.measure_collapse.push(MeasureCollapseParams {
                        qubit_bit: qubit_bit.as_u32(),
                        state_count: state_count as u32,
                        aux_slot: *aux_slot,
                        _pad: 0,
                    });
                }
                SimulationOp::CaptureProbability {
                    base_bit,
                    span,
                    output_slot,
                    control_mask,
                    control_value,
                    ..
                } => {
                    let rest_count = (state_count as u32) >> *span;
                    packed.probability.push(ProbabilityReduceParams {
                        base_bit: base_bit.as_u32(),
                        span: *span,
                        rest_count,
                        output_slot: *output_slot,
                        control_mask: *control_mask,
                        control_value: *control_value,
                        _pad: [0; 2],
                    });
                }
                SimulationOp::CaptureAmplitude {
                    base_bit,
                    span,
                    output_slot,
                    control_mask,
                    control_value,
                    ..
                } => {
                    let total_qubits = usize::BITS - state_count.leading_zeros() - 1;
                    packed.amplitude.push(AmplitudeCaptureParams {
                        base_bit: base_bit.as_u32(),
                        span: *span,
                        output_slot: *output_slot,
                        state_count: state_count as u32,
                        control_mask: *control_mask,
                        control_value: *control_value,
                        phase_lock_enabled: u32::from(*span != total_qubits),
                        total_qubits,
                    });
                }
                SimulationOp::CaptureDensity {
                    base_bit,
                    span,
                    output_slot,
                    control_mask,
                    control_value,
                    ..
                } => {
                    packed.density.push(DensityCaptureParams {
                        base_bit: base_bit.as_u32(),
                        span: *span,
                        output_slot: *output_slot,
                        state_count: state_count as u32,
                        control_mask: *control_mask,
                        control_value: *control_value,
                        _pad: [0; 2],
                    });
                }
            }
        }

        packed
    }

    /// Issue A pre-pass: upload all per-op params in at most one
    /// `queue.write_buffer` per variant. The recompute encoder then sources
    /// these staging slots via `copy_buffer_to_buffer`, so all dispatches can
    /// stay in one encoder + one submit.
    pub(super) fn upload(&self, queue: &wgpu::Queue, resources: &StateVectorResources) {
        if !self.gate.is_empty() {
            queue.write_buffer(
                &resources.state.gate_params_staging_buffer,
                0,
                bytemuck::cast_slice(&self.gate),
            );
        }
        if !self.bloch.is_empty() {
            queue.write_buffer(
                &resources.bloch.params_staging_buffer,
                0,
                bytemuck::cast_slice(&self.bloch),
            );
        }
        if !self.measure_reduce.is_empty() {
            queue.write_buffer(
                &resources.measure.reduce_params_staging_buffer,
                0,
                bytemuck::cast_slice(&self.measure_reduce),
            );
        }
        if !self.measure_collapse.is_empty() {
            queue.write_buffer(
                &resources.measure.collapse_params_staging_buffer,
                0,
                bytemuck::cast_slice(&self.measure_collapse),
            );
        }
        if !self.probability.is_empty() {
            queue.write_buffer(
                &resources.probability.reduce_params_staging_buffer,
                0,
                bytemuck::cast_slice(&self.probability),
            );
        }
        if !self.amplitude.is_empty() {
            queue.write_buffer(
                &resources.amplitude.capture_params_staging_buffer,
                0,
                bytemuck::cast_slice(&self.amplitude),
            );
        }
        if !self.density.is_empty() {
            queue.write_buffer(
                &resources.density.capture_params_staging_buffer,
                0,
                bytemuck::cast_slice(&self.density),
            );
        }
    }
}
