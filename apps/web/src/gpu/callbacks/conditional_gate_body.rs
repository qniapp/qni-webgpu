use eframe::egui;
use eframe::{egui_wgpu, wgpu};

use super::super::params::{
    ConditionalGateBodyInstance, ConditionalGateBodyParams, MAX_CONDITIONAL_GATE_BODIES,
};
use super::super::resources::StateVectorResources;

/// Paints one conditional gate body in the regular or the disabled fill,
/// chosen on the GPU from `measurement_aux_buffer.z`. The CPU never learns
/// whether the gate applied.
pub(crate) struct ConditionalGateBodyCallback {
    /// Per-frame instance index; unique among this frame's callbacks because
    /// egui runs every `prepare` before the first `paint`.
    pub(crate) index: usize,
    pub(crate) instance: ConditionalGateBodyInstance,
    pub(crate) params: ConditionalGateBodyParams,
}

impl ConditionalGateBodyCallback {
    /// Frame capacity; callers paint the regular fill beyond it.
    pub(crate) const MAX_PER_FRAME: usize = MAX_CONDITIONAL_GATE_BODIES;
}

impl egui_wgpu::CallbackTrait for ConditionalGateBodyCallback {
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
        if self.index >= Self::MAX_PER_FRAME {
            return Vec::new();
        }
        let body = &mut resources.conditional_gate_body;
        if body.last_params != Some(self.params) {
            queue.write_buffer(&body.params_buffer, 0, bytemuck::bytes_of(&self.params));
            body.last_params = Some(self.params);
        }
        let stride = std::mem::size_of::<ConditionalGateBodyInstance>();
        queue.write_buffer(
            &body.instance_buffer,
            (self.index * stride) as wgpu::BufferAddress,
            bytemuck::bytes_of(&self.instance),
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
        if self.index >= Self::MAX_PER_FRAME {
            return;
        }
        let body = &resources.conditional_gate_body;
        render_pass.set_pipeline(&body.pipeline);
        render_pass.set_bind_group(0, &body.bind_group, &[]);
        render_pass.set_vertex_buffer(0, resources.common.unit_quad_vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, body.instance_buffer.slice(..));
        render_pass.set_index_buffer(
            resources.common.unit_quad_index_buffer.slice(..),
            wgpu::IndexFormat::Uint16,
        );
        let index = self.index as u32;
        render_pass.draw_indexed(0..6, 0, index..index + 1);
    }
}
