use std::sync::Arc;

use eframe::egui;
use eframe::{egui_wgpu, wgpu};

use super::super::params::{AmplitudeInstance, AmplitudeRenderParams};
use super::super::resources::StateVectorResources;

/// Renders Amplitude display cells straight from GPU amplitude buffers.
/// The CPU supplies only geometry, colours, and hovered cell index.
pub(crate) struct AmplitudeDisplayCallback {
    pub(crate) instances: Arc<[AmplitudeInstance]>,
    pub(crate) use_drag_preview_buffer: bool,
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    pub(crate) background: [f32; 4],
    pub(crate) drag_background: [f32; 4],
    pub(crate) border: [f32; 4],
    pub(crate) disk: [f32; 4],
    pub(crate) outline: [f32; 4],
    pub(crate) outline_zero: [f32; 4],
    pub(crate) needle: [f32; 4],
    pub(crate) hover_border: [f32; 4],
}

impl egui_wgpu::CallbackTrait for AmplitudeDisplayCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(resources) = callback_resources.get_mut::<StateVectorResources>() else {
            return Vec::new();
        };
        if self.instances.is_empty() {
            return Vec::new();
        }
        let params = AmplitudeRenderParams {
            viewport_min: self.viewport_min,
            viewport_size: self.viewport_size,
            background: self.background,
            drag_background: self.drag_background,
            border: self.border,
            disk: self.disk,
            outline: self.outline,
            outline_zero: self.outline_zero,
            needle: self.needle,
            hover_border: self.hover_border,
        };
        if self.use_drag_preview_buffer {
            queue.write_buffer(
                &resources.amplitude.render_drag_params_buffer,
                0,
                bytemuck::bytes_of(&params),
            );
        } else if resources.amplitude.last_render_params != Some(params) {
            queue.write_buffer(
                &resources.amplitude.render_params_buffer,
                0,
                bytemuck::bytes_of(&params),
            );
            resources.amplitude.last_render_params = Some(params);
        }
        let instance_buffer = if self.use_drag_preview_buffer {
            &resources.amplitude.render_drag_instance_buffer
        } else {
            &resources.amplitude.render_instance_buffer
        };
        queue.write_buffer(
            instance_buffer,
            0,
            bytemuck::cast_slice(self.instances.as_ref()),
        );
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        let Some(resources) = callback_resources.get::<StateVectorResources>() else {
            return;
        };
        if self.instances.is_empty() {
            return;
        }
        render_pass.set_pipeline(&resources.amplitude.render_pipeline);
        let bind_group = if self.use_drag_preview_buffer {
            &resources.amplitude.render_drag_bind_group
        } else {
            &resources.amplitude.render_bind_group
        };
        render_pass.set_bind_group(0, bind_group, &[]);
        let instance_buffer = if self.use_drag_preview_buffer {
            &resources.amplitude.render_drag_instance_buffer
        } else {
            &resources.amplitude.render_instance_buffer
        };
        render_pass.set_vertex_buffer(0, resources.common.unit_quad_vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
        render_pass.set_index_buffer(
            resources.common.unit_quad_index_buffer.slice(..),
            wgpu::IndexFormat::Uint16,
        );
        render_pass.draw_indexed(0..6, 0, 0..self.instances.len() as u32);
    }
}
