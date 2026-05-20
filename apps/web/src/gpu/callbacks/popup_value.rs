use eframe::egui;
use eframe::{egui_wgpu, wgpu};

use super::super::params::PopupValueParams;
use super::super::resources::StateVectorResources;

/// Renders the amplitude / probability / phase numeric rows of the
/// state-cell hover popup straight from the live state buffer. Three
/// instances (one per row), no vertex buffer — the shader uses
/// `@builtin(vertex_index)` to lay out the row quad and reads the
/// hovered cell's amplitude directly from `state_buffers[active]`.
/// The chrome / icons / labels are still painted by egui on the CPU
/// side; only the digit text is GPU-rendered.
pub(crate) struct PopupValueCallback {
    /// Egui callback viewport — see `BlochOverlayCallback::viewport_min`.
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    /// Top-left of the row-0 (amplitude) value text in egui pixels.
    pub(crate) value_anchor: [f32; 2],
    /// Vertical pitch between value rows (same `ROW_H` constant used by
    /// the popup chrome).
    pub(crate) row_pitch: f32,
    /// Atlas cell size in egui pixels — should match
    /// `(POPUP_GLYPH_CELL_W, POPUP_GLYPH_CELL_H)`.
    pub(crate) char_size: [f32; 2],
    pub(crate) text_color: [f32; 4],
    pub(crate) hovered_display_index: u32,
    pub(crate) qubits: u32,
}

impl egui_wgpu::CallbackTrait for PopupValueCallback {
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
        let params = PopupValueParams {
            viewport_min: self.viewport_min,
            viewport_size: self.viewport_size,
            value_anchor: self.value_anchor,
            row_pitch: self.row_pitch,
            _pad_row: 0.0,
            char_size: self.char_size,
            _pad_char: [0.0; 2],
            text_color: self.text_color,
            hovered_display_index: self.hovered_display_index,
            qubits: self.qubits,
            _pad: [0; 2],
        };
        if resources.popup_value.last_params != Some(params) {
            queue.write_buffer(
                &resources.popup_value.params_buffer,
                0,
                bytemuck::bytes_of(&params),
            );
            resources.popup_value.last_params = Some(params);
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
        let active = resources.active_state;
        render_pass.set_pipeline(&resources.popup_value.pipeline);
        render_pass.set_bind_group(0, &resources.popup_value.bind_groups[active], &[]);
        // 6 verts × 3 instances (one quad per row, no vertex buffer —
        // verts come from `@builtin(vertex_index)`).
        render_pass.draw(0..6, 0..3);
    }
}
