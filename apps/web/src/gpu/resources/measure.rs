//! Measurement reduce (sample) + collapse compute pipelines.
//!
//! * `reduce_pipeline` reads the state vector and writes
//!   `(pZero, r, outcome, sqrt_p_kept)` into `aux_buffer`. Two bind
//!   groups so we can read from either ping-pong state buffer.
//! * `collapse_pipeline` reads `aux_buffer` plus the state-in buffer
//!   and writes the renormalised post-measurement state to the
//!   state-out buffer. Layout has *four* bindings (state_in,
//!   state_out, aux, params).
//!
//! No render pipelines here — the on-screen digit lives in `digit.rs`
//! and only reads `aux_buffer`.

mod buffers;
mod collapse;
mod reduce;

use eframe::wgpu;

use super::common::Common;

pub(crate) struct MeasureResources {
    pub reduce_pipeline: wgpu::ComputePipeline,
    pub reduce_bind_groups: [wgpu::BindGroup; 2],
    pub reduce_params_buffer: wgpu::Buffer,
    pub reduce_params_staging_buffer: wgpu::Buffer,

    pub collapse_pipeline: wgpu::ComputePipeline,
    pub collapse_bind_groups: [wgpu::BindGroup; 2],
    pub collapse_params_buffer: wgpu::Buffer,
    pub collapse_params_staging_buffer: wgpu::Buffer,

    pub aux_buffer: wgpu::Buffer,
}

impl MeasureResources {
    pub(super) fn build(device: &wgpu::Device, common: &Common) -> Self {
        let aux_buffer = buffers::create_aux_buffer(device);
        let reduce = reduce::build(device, common, &aux_buffer);
        let collapse = collapse::build(device, common, &aux_buffer);

        Self {
            reduce_pipeline: reduce.pipeline,
            reduce_bind_groups: reduce.bind_groups,
            reduce_params_buffer: reduce.params_buffer,
            reduce_params_staging_buffer: reduce.params_staging_buffer,
            collapse_pipeline: collapse.pipeline,
            collapse_bind_groups: collapse.bind_groups,
            collapse_params_buffer: collapse.params_buffer,
            collapse_params_staging_buffer: collapse.params_staging_buffer,
            aux_buffer,
        }
    }
}
