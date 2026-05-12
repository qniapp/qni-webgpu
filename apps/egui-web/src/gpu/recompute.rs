//! Batched state-vector recompute encoder.
//!
//! This module owns the GPU-only recompute path: it packs the already
//! linearised simulation ops, encodes compute dispatches into a single
//! command encoder, and updates the test-only readback slot maps. No quantum
//! state is computed or read on the CPU here.

use eframe::wgpu;

use crate::bloch::{validate_simulation_plan_capacity, SimulationOp, SimulationPlanLimits};
use crate::gates::GateParams;

use super::params::{
    BlochParams, MeasureCollapseParams, MeasureReduceParams, MAX_BLOCH_SLOTS,
    MAX_MEASUREMENT_SLOTS, MAX_OPS_PER_RECOMPUTE, STATE_WORKGROUP_SIZE,
};
use super::readback::{BLOCH_SLOT_MAP, MEASUREMENT_SLOT_MAP};
use super::resources::StateVectorResources;

pub(super) fn recompute_state_vector(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    resources: &mut StateVectorResources,
    sim_ops: &[SimulationOp],
    state_count: usize,
) -> bool {
    resources.state_count = state_count;
    if state_count > 0 {
        // Initialize to |0…0⟩ then dispatch each op on the GPU.
        // The init itself happens on the GPU: `clear_buffer` zeros
        // the state range, then `copy_buffer_to_buffer` writes the
        // ground-state amplitude (1.0, 0.0) into slot 0. Both are
        // encoded into the recompute encoder below — no CPU
        // allocation, no `queue.write_buffer` upload (Issue C).
        resources.active_state = 0;
        let pair_count = (state_count / 2) as u32;
        let dispatch_x = pair_count.div_ceil(STATE_WORKGROUP_SIZE);

        if validate_simulation_plan_capacity(
            sim_ops,
            SimulationPlanLimits {
                max_ops_per_variant: MAX_OPS_PER_RECOMPUTE,
                max_bloch_slots: MAX_BLOCH_SLOTS,
                max_measurement_slots: MAX_MEASUREMENT_SLOTS,
            },
        )
        .is_err()
        {
            return false;
        }

        // ─── Issue A pre-pass ─────────────────────────────────────
        // Classify every op by variant and pack their params
        // contiguously into the per-variant staging buffers via a
        // single `queue.write_buffer` per variant. The dispatch loop
        // below will then source each op's params via
        // `encoder.copy_buffer_to_buffer` from these staging buffers
        // instead of re-uploading per gate, so all dispatches can
        // live in one encoder + one submit.
        let mut packed_gate_params: Vec<GateParams> = Vec::with_capacity(sim_ops.len());
        let mut packed_bloch_params: Vec<BlochParams> = Vec::with_capacity(sim_ops.len());
        let mut packed_measure_reduce_params: Vec<MeasureReduceParams> =
            Vec::with_capacity(sim_ops.len());
        let mut packed_measure_collapse_params: Vec<MeasureCollapseParams> =
            Vec::with_capacity(sim_ops.len());
        for op in sim_ops {
            match op {
                SimulationOp::ApplyGate(params) => {
                    packed_gate_params.push(*params);
                }
                SimulationOp::CaptureBloch {
                    qubit_bit,
                    output_slot,
                    ..
                } => {
                    packed_bloch_params.push(BlochParams {
                        qubit_bit: *qubit_bit,
                        state_count: state_count as u32,
                        output_slot: *output_slot,
                        _pad: 0,
                    });
                }
                SimulationOp::MeasureReduceSample {
                    gate_id,
                    qubit_bit,
                    output_slot,
                } => {
                    packed_measure_reduce_params.push(MeasureReduceParams {
                        qubit_bit: *qubit_bit,
                        state_count: state_count as u32,
                        output_slot: *output_slot,
                        seed: *gate_id,
                    });
                }
                SimulationOp::MeasureCollapse {
                    qubit_bit,
                    aux_slot,
                } => {
                    if pair_count == 0 {
                        continue;
                    }
                    packed_measure_collapse_params.push(MeasureCollapseParams {
                        qubit_bit: *qubit_bit,
                        state_count: state_count as u32,
                        aux_slot: *aux_slot,
                        _pad: 0,
                    });
                }
            }
        }
        if !packed_gate_params.is_empty() {
            queue.write_buffer(
                &resources.state.gate_params_staging_buffer,
                0,
                bytemuck::cast_slice(&packed_gate_params),
            );
        }
        if !packed_bloch_params.is_empty() {
            queue.write_buffer(
                &resources.bloch.params_staging_buffer,
                0,
                bytemuck::cast_slice(&packed_bloch_params),
            );
        }
        if !packed_measure_reduce_params.is_empty() {
            queue.write_buffer(
                &resources.measure.reduce_params_staging_buffer,
                0,
                bytemuck::cast_slice(&packed_measure_reduce_params),
            );
        }
        if !packed_measure_collapse_params.is_empty() {
            queue.write_buffer(
                &resources.measure.collapse_params_staging_buffer,
                0,
                bytemuck::cast_slice(&packed_measure_collapse_params),
            );
        }
        // ──────────────────────────────────────────────────────────

        let mut in_index = 0usize;
        let mut bloch_slot_to_gate_id: Vec<u32> = Vec::with_capacity(MAX_BLOCH_SLOTS);
        let mut measurement_slot_to_gate_id: Vec<u32> = Vec::with_capacity(MAX_MEASUREMENT_SLOTS);
        // Single encoder for the entire recompute. Each per-op param
        // update is encoded as `copy_buffer_to_buffer` from the
        // matching staging slot into the existing tiny uniform
        // buffer, immediately followed by the dispatch that reads it.
        // WebGPU guarantees in-order execution within one encoder,
        // and inserts the necessary memory barriers automatically,
        // so each dispatch sees its own params even though all
        // dispatches share `gate_params_buffer` etc. Issue A: this
        // replaces N per-op `queue.submit` round trips with one.
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("recompute_batched_encoder"),
        });

        // Issue C: GPU-only |0…0⟩ initialization. clear_buffer zeros
        // the active state range, then copy_buffer_to_buffer writes
        // the ground-state amplitude into slot 0. Both live in the
        // same encoder as the gate dispatches, so the auto-inserted
        // memory barriers make the first ApplyGate read this fresh
        // |0…0⟩ vector.
        let state_active_bytes =
            (state_count * std::mem::size_of::<[f32; 2]>()) as wgpu::BufferAddress;
        encoder.clear_buffer(
            &resources.common.state_buffers[0],
            0,
            Some(state_active_bytes),
        );
        encoder.copy_buffer_to_buffer(
            &resources.common.state_seed_buffer,
            0,
            &resources.common.state_buffers[0],
            0,
            std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
        );

        let gate_param_size = std::mem::size_of::<GateParams>() as wgpu::BufferAddress;
        let bloch_param_size = std::mem::size_of::<BlochParams>() as wgpu::BufferAddress;
        let measure_reduce_param_size =
            std::mem::size_of::<MeasureReduceParams>() as wgpu::BufferAddress;
        let measure_collapse_param_size =
            std::mem::size_of::<MeasureCollapseParams>() as wgpu::BufferAddress;
        let mut gate_slot: u64 = 0;
        let mut bloch_slot: u64 = 0;
        let mut measure_reduce_slot: u64 = 0;
        let mut measure_collapse_slot: u64 = 0;
        for op in sim_ops {
            match op {
                SimulationOp::ApplyGate(_) => {
                    if pair_count == 0 {
                        continue;
                    }
                    encoder.copy_buffer_to_buffer(
                        &resources.state.gate_params_staging_buffer,
                        gate_slot * gate_param_size,
                        &resources.state.gate_params_buffer,
                        0,
                        gate_param_size,
                    );
                    {
                        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                            label: Some("state_vector_compute_pass"),
                            timestamp_writes: None,
                        });
                        pass.set_pipeline(&resources.state.compute_pipeline);
                        pass.set_bind_group(0, &resources.state.compute_bind_groups[in_index], &[]);
                        pass.dispatch_workgroups(dispatch_x, 1, 1);
                    }
                    gate_slot += 1;
                    in_index = 1 - in_index;
                }
                SimulationOp::CaptureBloch { gate_id, .. } => {
                    encoder.copy_buffer_to_buffer(
                        &resources.bloch.params_staging_buffer,
                        bloch_slot * bloch_param_size,
                        &resources.bloch.params_buffer,
                        0,
                        bloch_param_size,
                    );
                    {
                        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                            label: Some("bloch_reduce_pass"),
                            timestamp_writes: None,
                        });
                        pass.set_pipeline(&resources.bloch.reduce_pipeline);
                        // The current state lives in `state_buffers[in_index]`,
                        // which is the read side of the next gate dispatch.
                        pass.set_bind_group(0, &resources.bloch.reduce_bind_groups[in_index], &[]);
                        pass.dispatch_workgroups(1, 1, 1);
                    }
                    bloch_slot += 1;
                    bloch_slot_to_gate_id.push(*gate_id);
                }
                SimulationOp::MeasureReduceSample { gate_id, .. } => {
                    encoder.copy_buffer_to_buffer(
                        &resources.measure.reduce_params_staging_buffer,
                        measure_reduce_slot * measure_reduce_param_size,
                        &resources.measure.reduce_params_buffer,
                        0,
                        measure_reduce_param_size,
                    );
                    {
                        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                            label: Some("measure_reduce_pass"),
                            timestamp_writes: None,
                        });
                        pass.set_pipeline(&resources.measure.reduce_pipeline);
                        pass.set_bind_group(
                            0,
                            &resources.measure.reduce_bind_groups[in_index],
                            &[],
                        );
                        pass.dispatch_workgroups(1, 1, 1);
                    }
                    measure_reduce_slot += 1;
                    measurement_slot_to_gate_id.push(*gate_id);
                }
                SimulationOp::MeasureCollapse { .. } => {
                    if pair_count == 0 {
                        continue;
                    }
                    encoder.copy_buffer_to_buffer(
                        &resources.measure.collapse_params_staging_buffer,
                        measure_collapse_slot * measure_collapse_param_size,
                        &resources.measure.collapse_params_buffer,
                        0,
                        measure_collapse_param_size,
                    );
                    {
                        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                            label: Some("measure_collapse_pass"),
                            timestamp_writes: None,
                        });
                        pass.set_pipeline(&resources.measure.collapse_pipeline);
                        pass.set_bind_group(
                            0,
                            &resources.measure.collapse_bind_groups[in_index],
                            &[],
                        );
                        pass.dispatch_workgroups(dispatch_x, 1, 1);
                    }
                    measure_collapse_slot += 1;
                    in_index = 1 - in_index;
                }
            }
        }
        queue.submit(Some(encoder.finish()));
        resources.active_state = in_index;

        // Production path never reads back. The slot mappings are
        // stashed in thread-locals so the test-only on-demand
        // readback APIs (`read_bloch_vectors_impl` /
        // `read_measurement_outcomes_impl`) can copy + map the
        // GPU buffers when JS asks for them.
        BLOCH_SLOT_MAP.with(|cell| {
            *cell.borrow_mut() = bloch_slot_to_gate_id;
        });
        MEASUREMENT_SLOT_MAP.with(|cell| {
            *cell.borrow_mut() = measurement_slot_to_gate_id;
        });
    } else {
        resources.active_state = 0;
    }
    true
}
