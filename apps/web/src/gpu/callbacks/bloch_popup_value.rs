use eframe::egui;
use eframe::{egui_wgpu, wgpu};

use super::super::params::BlochPopupValueParams;
use super::super::resources::StateVectorResources;

/// Renders the r / φ / θ / x / y / z value cells of the Bloch hover popup
/// straight from `bloch_output_buffer`. The popup chrome and static labels are
/// painted by egui; only the dynamic Bloch-vector-derived text is GPU-rendered.
pub(crate) struct BlochPopupValueCallback {
    /// Egui callback viewport — see `BlochOverlayCallback::viewport_min`.
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    /// Top-left of the first value cell (`r`) in egui pixels.
    pub(crate) value_anchor: [f32; 2],
    pub(crate) col_pitch: f32,
    pub(crate) row_pitch: f32,
    pub(crate) char_size: [f32; 2],
    pub(crate) text_color: [f32; 4],
    pub(crate) slot: u32,
}

impl egui_wgpu::CallbackTrait for BlochPopupValueCallback {
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
        let params = BlochPopupValueParams {
            viewport_min: self.viewport_min,
            viewport_size: self.viewport_size,
            value_anchor: self.value_anchor,
            col_pitch: self.col_pitch,
            row_pitch: self.row_pitch,
            char_size: self.char_size,
            _pad_char: [0.0; 2],
            text_color: self.text_color,
            slot: self.slot,
            _pad: [0; 3],
        };
        if resources.bloch.last_popup_value_params != Some(params) {
            queue.write_buffer(
                &resources.bloch.popup_value_params_buffer,
                0,
                bytemuck::bytes_of(&params),
            );
            resources.bloch.last_popup_value_params = Some(params);
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
        render_pass.set_pipeline(&resources.bloch.popup_value_pipeline);
        render_pass.set_bind_group(0, &resources.bloch.popup_value_bind_group, &[]);
        // 6 verts × 6 instances: r / φ / θ / x / y / z.
        render_pass.draw(0..6, 0..6);
    }
}
