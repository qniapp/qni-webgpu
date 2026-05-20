use eframe::egui;
use eframe::egui_wgpu;

use super::geometry::{popup_glyph_char_size, PopupGeometry};
use crate::colors::Colors;
use crate::gpu::PopupValueCallback;
use crate::render::state_panel_layout::StatePanelLayout;

pub(super) fn paint_popup_values(
    painter: &egui::Painter,
    colors: &Colors,
    layout: &StatePanelLayout,
    screen_rect: egui::Rect,
    display_index: u32,
    popup: &PopupGeometry,
) {
    // Numeric values come from the GPU state buffer via a dedicated render pass
    // — see `PopupValueCallback`. The CPU only computes the anchor / row pitch /
    // colour and hands them off; no readback.
    let value_color = colors.state_needle;
    let value_anchor = popup.value_anchor();
    let popup_value_callback = PopupValueCallback {
        viewport_min: [screen_rect.min.x, screen_rect.min.y],
        viewport_size: [screen_rect.width(), screen_rect.height()],
        value_anchor: [value_anchor.x, value_anchor.y],
        row_pitch: popup.row_pitch(),
        char_size: popup_glyph_char_size(),
        text_color: [
            value_color.r() as f32 / 255.0,
            value_color.g() as f32 / 255.0,
            value_color.b() as f32 / 255.0,
            1.0,
        ],
        hovered_display_index: display_index,
        qubits: layout.qubits as u32,
    };

    // Clip the GPU pass to the popup body so the value text never bleeds past
    // the border / tail.
    let paint_callback = egui_wgpu::Callback::new_paint_callback(screen_rect, popup_value_callback);
    let clipped = painter.with_clip_rect(popup.value_clip_rect());
    clipped.add(egui::Shape::Callback(paint_callback));
}
