use std::sync::Arc;

use eframe::egui;
use eframe::{egui_wgpu, wgpu};

use super::super::params::{MeasurementDigitInstance, MeasurementDigitParams};
use super::super::resources::StateVectorResources;

/// Renders the 0/1 measurement digit straight from
/// `measurement_aux_buffer.z` (the GPU-sampled outcome). Static meter icon
/// (purple or zinc-200 ring) is still painted by egui — only the digit is
/// quantum-state-derived.
pub(crate) struct MeasurementDigitCallback {
    pub(crate) instances: Arc<[MeasurementDigitInstance]>,
    /// See `BlochOverlayCallback::viewport_min`.
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    pub(crate) zero_color: [f32; 4],
    pub(crate) one_color: [f32; 4],
}

impl egui_wgpu::CallbackTrait for MeasurementDigitCallback {
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
        let params = MeasurementDigitParams {
            viewport_min: self.viewport_min,
            viewport_size: self.viewport_size,
            zero_color: self.zero_color,
            one_color: self.one_color,
        };
        if resources.digit.last_params != Some(params) {
            queue.write_buffer(
                &resources.digit.params_buffer,
                0,
                bytemuck::bytes_of(&params),
            );
            resources.digit.last_params = Some(params);
        }
        queue.write_buffer(
            &resources.digit.instance_buffer,
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
        render_pass.set_pipeline(&resources.digit.pipeline);
        render_pass.set_bind_group(0, &resources.digit.bind_group, &[]);
        // Reuse the bloch overlay's quad geometry — both render full-rect
        // quads with `[-1..1]` corners.
        render_pass.set_vertex_buffer(0, resources.common.unit_quad_vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, resources.digit.instance_buffer.slice(..));
        render_pass.set_index_buffer(
            resources.common.unit_quad_index_buffer.slice(..),
            wgpu::IndexFormat::Uint16,
        );
        render_pass.draw_indexed(0..6, 0, 0..self.instances.len() as u32);
    }
}
