//! Popup value render pipeline.
//!
//! Three-instance draw painting the amplitude / probability / phase
//! numeric rows of the state-cell hover popup. The fragment shader
//! reads the active `Common::state_buffers` directly and looks up
//! glyphs in the baked popup atlas. No CPU readback.
//!
//! Three bind groups so we can pick the active ping-pong index or the
//! state-panel preview snapshot when the popup paints.

mod atlas;
mod bindings;
mod pipeline;

use eframe::wgpu;

use super::super::params::PopupValueParams;
use super::common::Common;

pub(crate) struct PopupValueResources {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_groups: [wgpu::BindGroup; 3],
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub params_buffer: wgpu::Buffer,
    pub last_params: Option<PopupValueParams>,
}

impl PopupValueResources {
    pub(super) fn build(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
        common: &Common,
    ) -> Self {
        let atlas = atlas::create_popup_glyph_atlas(device, queue);
        let bind_group_layout = bindings::create_bind_group_layout(device);
        let params_buffer = bindings::create_params_buffer(device);
        let bind_groups = bindings::create_bind_groups(
            device,
            common,
            &bind_group_layout,
            &params_buffer,
            &atlas,
        );
        let pipeline = pipeline::build_pipeline(device, target_format, &bind_group_layout);

        Self {
            pipeline,
            bind_groups,
            bind_group_layout,
            params_buffer,
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
