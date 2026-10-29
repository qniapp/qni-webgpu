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
    pub render_bind_groups: [wgpu::BindGroup; 3],
    pub render_bind_group_layout: wgpu::BindGroupLayout,
    pub render_params_buffer: wgpu::Buffer,
    pub last_render_params: Option<RenderParams>,

    pub gate_params_buffer: wgpu::Buffer,
    pub gate_params_staging_buffer: wgpu::Buffer,
}

impl StateResources {
    pub(super) fn build(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        common: &Common,
        measurement_aux_buffer: &wgpu::Buffer,
    ) -> Self {
        let compute = compute::build(device, common, measurement_aux_buffer);
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
