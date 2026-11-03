mod overlay;
mod pipeline;
mod popup;
mod reduce;

use eframe::wgpu;

use super::super::params::{BlochOverlayParams, BlochPopupValueParams};
use super::common::Common;

pub(crate) struct BlochResources {
    pub reduce_pipeline: wgpu::ComputePipeline,
    pub reduce_bind_groups: [wgpu::BindGroup; 2],
    pub params_buffer: wgpu::Buffer,
    pub params_staging_buffer: wgpu::Buffer,
    pub output_buffer: wgpu::Buffer,

    pub overlay_pipeline: wgpu::RenderPipeline,
    pub overlay_bind_group: wgpu::BindGroup,
    pub overlay_params_buffer: wgpu::Buffer,
    pub overlay_instance_buffer: wgpu::Buffer,
    pub last_overlay_params: Option<BlochOverlayParams>,
    pub last_external_upload_generation: Option<u64>,

    pub popup_value_pipeline: wgpu::RenderPipeline,
    pub popup_value_bind_group: wgpu::BindGroup,
    pub popup_value_params_buffer: wgpu::Buffer,
    pub last_popup_value_params: Option<BlochPopupValueParams>,
}

impl BlochResources {
    pub(super) fn build(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
        common: &Common,
    ) -> Self {
        let reduce = reduce::build(device, common);
        let overlay = overlay::build(device, target_format, &reduce.output_buffer);
        let popup = popup::build(device, queue, target_format, &reduce.output_buffer);

        Self {
            reduce_pipeline: reduce.pipeline,
            reduce_bind_groups: reduce.bind_groups,
            params_buffer: reduce.params_buffer,
            params_staging_buffer: reduce.params_staging_buffer,
            output_buffer: reduce.output_buffer,
            overlay_pipeline: overlay.pipeline,
            overlay_bind_group: overlay.bind_group,
            overlay_params_buffer: overlay.params_buffer,
            overlay_instance_buffer: overlay.instance_buffer,
            last_overlay_params: None,
            last_external_upload_generation: None,
            popup_value_pipeline: popup.pipeline,
            popup_value_bind_group: popup.bind_group,
            popup_value_params_buffer: popup.params_buffer,
            last_popup_value_params: None,
        }
    }
}
