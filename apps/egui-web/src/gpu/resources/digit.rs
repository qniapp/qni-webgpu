//! Measurement digit overlay render pipeline.
//!
//! Renders the "0" or "1" character above each measured qubit by
//! reading the `outcome` field of `measure::aux_buffer` in the
//! fragment shader and sampling the pre-rasterised Geist Bold digit
//! atlas. Static meter chrome is still painted by egui.
//!
//! The instance draw geometry is the shared `Common::unit_quad_*`
//! quad; per-slot offsets/sizes live in `instance_buffer`.

mod atlas;
mod bindings;
mod pipeline;

use eframe::wgpu;

use super::super::params::MeasurementDigitParams;
use super::measure::MeasureResources;

pub(crate) struct DigitResources {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group: wgpu::BindGroup,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub params_buffer: wgpu::Buffer,
    pub instance_buffer: wgpu::Buffer,
    pub last_params: Option<MeasurementDigitParams>,
}

impl DigitResources {
    pub(super) fn build(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
        measure: &MeasureResources,
    ) -> Self {
        let atlas = atlas::create_digit_atlas(device, queue);
        let bind_group_layout = bindings::create_bind_group_layout(device);
        let params_buffer = bindings::create_params_buffer(device);
        let instance_buffer = bindings::create_instance_buffer(device);
        let bind_group = bindings::create_bind_group(
            device,
            &bind_group_layout,
            measure,
            &params_buffer,
            &atlas,
        );
        let pipeline = pipeline::build_pipeline(device, target_format, &bind_group_layout);

        Self {
            pipeline,
            bind_group,
            bind_group_layout,
            params_buffer,
            instance_buffer,
            last_params: None,
        }
    }

    pub(super) fn update_target_format(
        &mut self,
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
    ) {
        self.pipeline = pipeline::build_pipeline(device, target_format, &self.bind_group_layout);
    }
}
