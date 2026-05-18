use std::sync::Arc;

use eframe::egui;
use eframe::{egui_wgpu, wgpu};

use super::super::params::{ChanceInstance, ChanceRenderParams};
use super::super::resources::StateVectorResources;

/// Renders Chance display bars straight from the GPU probability buffer
/// produced during recompute. The CPU supplies only geometry, colors, and the
/// hovered row index; probabilities never leave GPU memory in production.
pub(crate) struct ChanceDisplayCallback {
    pub(crate) instances: Arc<[ChanceInstance]>,
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    pub(crate) background: [f32; 4],
    pub(crate) border: [f32; 4],
    pub(crate) bar: [f32; 4],
    pub(crate) bar_edge: [f32; 4],
    pub(crate) hover_border: [f32; 4],
    pub(crate) text_color: [f32; 4],
}

impl egui_wgpu::CallbackTrait for ChanceDisplayCallback {
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
        let params = ChanceRenderParams {
            viewport_min: self.viewport_min,
            viewport_size: self.viewport_size,
            background: self.background,
            border: self.border,
            bar: self.bar,
            bar_edge: self.bar_edge,
            hover_border: self.hover_border,
            text_color: self.text_color,
        };
        if resources.chance.last_render_params != Some(params) {
            queue.write_buffer(
                &resources.chance.render_params_buffer,
                0,
                bytemuck::bytes_of(&params),
            );
            resources.chance.last_render_params = Some(params);
        }
        queue.write_buffer(
            &resources.chance.render_instance_buffer,
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
        render_pass.set_pipeline(&resources.chance.render_pipeline);
        render_pass.set_bind_group(0, &resources.chance.render_bind_group, &[]);
        render_pass.set_vertex_buffer(0, resources.common.unit_quad_vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, resources.chance.render_instance_buffer.slice(..));
        render_pass.set_index_buffer(
            resources.common.unit_quad_index_buffer.slice(..),
            wgpu::IndexFormat::Uint16,
        );
        render_pass.draw_indexed(0..6, 0, 0..self.instances.len() as u32);
    }
}
