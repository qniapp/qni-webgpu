use eframe::egui;
use eframe::{egui_wgpu, wgpu};

use super::super::params::AmplitudePopupValueParams;
use super::super::resources::StateVectorResources;

/// Renders dynamic Amplitude popup values directly from the GPU buffers.
pub(crate) struct AmplitudePopupValueCallback {
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    pub(crate) value_anchor: [f32; 2],
    pub(crate) row_pitch: f32,
    pub(crate) char_size: [f32; 2],
    pub(crate) text_color: [f32; 4],
    pub(crate) slot: u32,
    pub(crate) outcome: u32,
}

impl egui_wgpu::CallbackTrait for AmplitudePopupValueCallback {
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
        let params = AmplitudePopupValueParams {
            viewport_min: self.viewport_min,
            viewport_size: self.viewport_size,
            value_anchor: self.value_anchor,
            row_pitch: self.row_pitch,
            _pad_row: 0.0,
            char_size: self.char_size,
            _pad_char: [0.0; 2],
            text_color: self.text_color,
            slot: self.slot,
            outcome: self.outcome,
            _pad: [0; 2],
        };
        if resources.amplitude.last_popup_value_params != Some(params) {
            queue.write_buffer(
                &resources.amplitude.popup_value_params_buffer,
                0,
                bytemuck::bytes_of(&params),
            );
            resources.amplitude.last_popup_value_params = Some(params);
        }
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
        render_pass.set_pipeline(&resources.amplitude.popup_value_pipeline);
        render_pass.set_bind_group(0, &resources.amplitude.popup_value_bind_group, &[]);
        render_pass.draw(0..6, 0..1);
    }
}
