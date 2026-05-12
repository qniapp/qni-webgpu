//! State-vector compute + render pipelines.
//!
//! * `compute_pipeline` applies a single `GateParams` matrix (or
//!   `Write0/Write1` permutation) to the active state buffer,
//!   ping-ponging between the two `Common::state_buffers`.
//! * `render_pipeline` paints the state-vector circle grid (fill +
//!   needle + outline) — the GPU side of the state panel.
//!
//! `update_target_format` rebuilds *only* the render pipeline when the
//! egui surface format changes; the compute pipeline isn't format-
//! dependent and stays untouched.

mod compute;
mod pipeline;
mod render;

use eframe::wgpu;

use super::super::params::RenderParams;
use super::common::Common;

pub(crate) struct StateResources {
    pub compute_pipeline: wgpu::ComputePipeline,
    pub compute_bind_groups: [wgpu::BindGroup; 2],

    pub render_pipeline: wgpu::RenderPipeline,
    pub render_bind_groups: [wgpu::BindGroup; 2],
    pub render_bind_group_layout: wgpu::BindGroupLayout,
    pub render_params_buffer: wgpu::Buffer,
    pub last_render_params: Option<RenderParams>,

    pub gate_params_buffer: wgpu::Buffer,
    /// Staging buffer holding all `GateParams` for a recompute, packed
    /// contiguously. Filled once via `queue.write_buffer` before the dispatch
    /// loop; each per-gate dispatch then sources its params via
    /// `encoder.copy_buffer_to_buffer` from the matching slot into
    /// `gate_params_buffer`. Lets us keep the existing uniform binding while
    /// collapsing N per-gate `queue.submit` round trips into a single submit.
    pub gate_params_staging_buffer: wgpu::Buffer,
}

impl StateResources {
    pub(super) fn build(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        common: &Common,
    ) -> Self {
        let compute = compute::build(device, common);
        let render = render::build(device, target_format, common);

        Self {
            compute_pipeline: compute.pipeline,
            compute_bind_groups: compute.bind_groups,
            render_pipeline: render.pipeline,
            render_bind_groups: render.bind_groups,
            render_bind_group_layout: render.bind_group_layout,
            render_params_buffer: render.params_buffer,
            last_render_params: None,
            gate_params_buffer: compute.gate_params_buffer,
            gate_params_staging_buffer: compute.gate_params_staging_buffer,
        }
    }

    pub(super) fn update_target_format(
        &mut self,
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
    ) {
        self.render_pipeline =
            pipeline::build_render_pipeline(device, target_format, &self.render_bind_group_layout);
    }
}
