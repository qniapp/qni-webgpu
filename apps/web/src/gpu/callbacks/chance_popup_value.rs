use eframe::egui;
use eframe::{egui_wgpu, wgpu};

use super::super::params::ChancePopupValueParams;
use super::super::resources::StateVectorResources;

/// Renders the raw/log value rows of the Chance hover popup straight from
/// `chance_probability_output`. The popup chrome and static labels are painted
/// by egui; only the dynamic probability-derived text is GPU-rendered.
pub(crate) struct ChancePopupValueCallback {
    /// Egui callback viewport — see `BlochOverlayCallback::viewport_min`.
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    /// Top-left of the row-0 (`raw`) value text in egui pixels.
    pub(crate) value_anchor: [f32; 2],
    pub(crate) row_pitch: f32,
    pub(crate) char_size: [f32; 2],
    pub(crate) text_color: [f32; 4],
    pub(crate) slot: u32,
    pub(crate) outcome: u32,
}

impl egui_wgpu::CallbackTrait for ChancePopupValueCallback {
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
        let params = ChancePopupValueParams {
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
        if resources.chance.last_popup_value_params != Some(params) {
            queue.write_buffer(
                &resources.chance.popup_value_params_buffer,
                0,
                bytemuck::bytes_of(&params),
            );
            resources.chance.last_popup_value_params = Some(params);
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
        render_pass.set_pipeline(&resources.chance.popup_value_pipeline);
        render_pass.set_bind_group(0, &resources.chance.popup_value_bind_group, &[]);
        // 6 verts × 2 instances: raw row and log row.
        render_pass.draw(0..6, 0..2);
    }
}
