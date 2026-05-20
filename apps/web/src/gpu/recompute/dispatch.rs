use eframe::wgpu;

use crate::gates::GateParams;
use crate::simulation_plan::SimulationOp;

use super::super::params::{
    AmplitudeCaptureParams, BlochParams, ChanceReduceParams, MeasureCollapseParams,
    MeasureReduceParams, MAX_AMPLITUDE_SLOTS, MAX_BLOCH_SLOTS, MAX_CHANCE_SLOTS,
    MAX_MEASUREMENT_SLOTS,
};
use super::super::resources::StateVectorResources;
use super::clear::encode_ground_state_init;

pub(super) struct EncodedRecompute {
    pub(super) command_buffer: wgpu::CommandBuffer,
    pub(super) active_state: usize,
    pub(super) bloch_slot_to_gate_id: Vec<u32>,
    pub(super) measurement_slot_to_gate_id: Vec<u32>,
    pub(super) chance_slot_to_gate_id: Vec<u32>,
    pub(super) amplitude_slot_to_gate_id: Vec<u32>,
}

/// Encode one batched recompute command buffer. Each per-op param update is an
/// in-encoder staging-slot copy immediately followed by the dispatch that reads
/// it. WebGPU preserves command order and inserts the needed barriers, so every
/// dispatch sees its own params while the whole recompute still uses one submit.
pub(super) fn encode_batched_recompute(
    device: &wgpu::Device,
    resources: &StateVectorResources,
    sim_ops: &[SimulationOp],
    state_count: usize,
    pair_count: u32,
    dispatch_x: u32,
) -> EncodedRecompute {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("recompute_batched_encoder"),
    });
    encode_ground_state_init(&mut encoder, resources, state_count);

    let mut state = DispatchState::new(pair_count, dispatch_x);
    for op in sim_ops {
        match op {
            SimulationOp::SnapshotState { output_slot } => {
                state.encode_snapshot_state(&mut encoder, resources, state_count, *output_slot)
            }
            SimulationOp::ApplyGate(_) => state.encode_apply_gate(&mut encoder, resources),
            SimulationOp::CaptureBloch { gate_id, .. } => {
                state.encode_capture_bloch(&mut encoder, resources, *gate_id);
            }
            SimulationOp::MeasureReduceSample { gate_id, .. } => {
                state.encode_measure_reduce(&mut encoder, resources, *gate_id);
            }
            SimulationOp::MeasureCollapse { .. } => {
                state.encode_measure_collapse(&mut encoder, resources);
            }
            SimulationOp::CaptureChance { gate_id, span, .. } => {
                state.encode_capture_chance(&mut encoder, resources, *gate_id, *span);
            }
            SimulationOp::CaptureAmplitude { gate_id, .. } => {
                state.encode_capture_amplitude(&mut encoder, resources, *gate_id);
            }
        }
    }

    let active_state = state.render_state_index();

    EncodedRecompute {
        command_buffer: encoder.finish(),
        active_state,
        bloch_slot_to_gate_id: state.bloch_slot_to_gate_id,
        measurement_slot_to_gate_id: state.measurement_slot_to_gate_id,
        chance_slot_to_gate_id: state.chance_slot_to_gate_id,
        amplitude_slot_to_gate_id: state.amplitude_slot_to_gate_id,
    }
}

struct DispatchState {
    in_index: usize,
    render_state_index: Option<usize>,
    pair_count: u32,
    dispatch_x: u32,
    gate_slot: u64,
    bloch_slot: u64,
    measure_reduce_slot: u64,
    measure_collapse_slot: u64,
    chance_slot: u64,
    amplitude_slot: u64,
    bloch_slot_to_gate_id: Vec<u32>,
    measurement_slot_to_gate_id: Vec<u32>,
    chance_slot_to_gate_id: Vec<u32>,
    amplitude_slot_to_gate_id: Vec<u32>,
}

impl DispatchState {
    fn new(pair_count: u32, dispatch_x: u32) -> Self {
        Self {
            in_index: 0,
            render_state_index: None,
            pair_count,
            dispatch_x,
            gate_slot: 0,
            bloch_slot: 0,
            measure_reduce_slot: 0,
            measure_collapse_slot: 0,
            chance_slot: 0,
            amplitude_slot: 0,
            bloch_slot_to_gate_id: Vec::with_capacity(MAX_BLOCH_SLOTS),
            measurement_slot_to_gate_id: Vec::with_capacity(MAX_MEASUREMENT_SLOTS),
            chance_slot_to_gate_id: Vec::with_capacity(MAX_CHANCE_SLOTS),
            amplitude_slot_to_gate_id: Vec::with_capacity(MAX_AMPLITUDE_SLOTS),
        }
    }

    fn render_state_index(&self) -> usize {
        self.render_state_index.unwrap_or(self.in_index)
    }

    fn encode_snapshot_state(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        resources: &StateVectorResources,
        state_count: usize,
        output_slot: u32,
    ) {
        let byte_len = (state_count * std::mem::size_of::<[f32; 2]>()) as wgpu::BufferAddress;
        if byte_len == 0 {
            return;
        }
        let dst_offset = resources.common.snapshot_cache_offset(output_slot as usize);
        encoder.copy_buffer_to_buffer(
            &resources.common.state_buffers[self.in_index],
            0,
            &resources.common.state_snapshot_cache_buffer,
            dst_offset,
            byte_len,
        );
    }

    fn encode_apply_gate(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        resources: &StateVectorResources,
    ) {
        if self.pair_count == 0 {
            return;
        }
        let size = std::mem::size_of::<GateParams>() as wgpu::BufferAddress;
        encoder.copy_buffer_to_buffer(
            &resources.state.gate_params_staging_buffer,
            self.gate_slot * size,
            &resources.state.gate_params_buffer,
            0,
            size,
        );
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("state_vector_compute_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&resources.state.compute_pipeline);
            pass.set_bind_group(0, &resources.state.compute_bind_groups[self.in_index], &[]);
            pass.dispatch_workgroups(self.dispatch_x, 1, 1);
        }
        self.gate_slot += 1;
        self.in_index = 1 - self.in_index;
    }

    fn encode_capture_bloch(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        resources: &StateVectorResources,
        gate_id: u32,
    ) {
        let size = std::mem::size_of::<BlochParams>() as wgpu::BufferAddress;
        encoder.copy_buffer_to_buffer(
            &resources.bloch.params_staging_buffer,
            self.bloch_slot * size,
            &resources.bloch.params_buffer,
            0,
            size,
        );
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("bloch_reduce_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&resources.bloch.reduce_pipeline);
            // The current state lives in `state_buffers[in_index]`, which is
            // the read side of the next gate dispatch.
            pass.set_bind_group(0, &resources.bloch.reduce_bind_groups[self.in_index], &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        self.bloch_slot += 1;
        self.bloch_slot_to_gate_id.push(gate_id);
    }

    fn encode_measure_reduce(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        resources: &StateVectorResources,
        gate_id: u32,
    ) {
        let size = std::mem::size_of::<MeasureReduceParams>() as wgpu::BufferAddress;
        encoder.copy_buffer_to_buffer(
            &resources.measure.reduce_params_staging_buffer,
            self.measure_reduce_slot * size,
            &resources.measure.reduce_params_buffer,
            0,
            size,
        );
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("measure_reduce_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&resources.measure.reduce_pipeline);
            pass.set_bind_group(0, &resources.measure.reduce_bind_groups[self.in_index], &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        self.measure_reduce_slot += 1;
        self.measurement_slot_to_gate_id.push(gate_id);
    }

    fn encode_measure_collapse(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        resources: &StateVectorResources,
    ) {
        if self.pair_count == 0 {
            return;
        }
        let size = std::mem::size_of::<MeasureCollapseParams>() as wgpu::BufferAddress;
        encoder.copy_buffer_to_buffer(
            &resources.measure.collapse_params_staging_buffer,
            self.measure_collapse_slot * size,
            &resources.measure.collapse_params_buffer,
            0,
            size,
        );
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("measure_collapse_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&resources.measure.collapse_pipeline);
            pass.set_bind_group(
                0,
                &resources.measure.collapse_bind_groups[self.in_index],
                &[],
            );
            pass.dispatch_workgroups(self.dispatch_x, 1, 1);
        }
        self.measure_collapse_slot += 1;
        self.in_index = 1 - self.in_index;
    }

    fn encode_capture_chance(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        resources: &StateVectorResources,
        gate_id: u32,
        span: u32,
    ) {
        let size = std::mem::size_of::<ChanceReduceParams>() as wgpu::BufferAddress;
        encoder.copy_buffer_to_buffer(
            &resources.chance.reduce_params_staging_buffer,
            self.chance_slot * size,
            &resources.chance.reduce_params_buffer,
            0,
            size,
        );
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("chance_reduce_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&resources.chance.reduce_pipeline);
            pass.set_bind_group(0, &resources.chance.reduce_bind_groups[self.in_index], &[]);
            let outcomes = 1u32 << span;
            let dispatch_x = outcomes.min(256);
            let dispatch_y = outcomes.div_ceil(dispatch_x);
            pass.dispatch_workgroups(dispatch_x, dispatch_y, 1);
        }
        self.chance_slot += 1;
        self.chance_slot_to_gate_id.push(gate_id);
    }

    fn encode_capture_amplitude(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        resources: &StateVectorResources,
        gate_id: u32,
    ) {
        let size = std::mem::size_of::<AmplitudeCaptureParams>() as wgpu::BufferAddress;
        encoder.copy_buffer_to_buffer(
            &resources.amplitude.capture_params_staging_buffer,
            self.amplitude_slot * size,
            &resources.amplitude.capture_params_buffer,
            0,
            size,
        );
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("amplitude_capture_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&resources.amplitude.capture_pipeline);
            pass.set_bind_group(
                0,
                &resources.amplitude.capture_bind_groups[self.in_index],
                &[],
            );
            pass.dispatch_workgroups(1, 1, 1);
        }
        self.amplitude_slot += 1;
        self.amplitude_slot_to_gate_id.push(gate_id);
    }
}
