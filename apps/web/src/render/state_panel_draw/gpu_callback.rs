use eframe::egui;
use eframe::{egui_wgpu, wgpu};

use crate::app::QniApp;
use crate::colors::Colors;
use crate::gpu::{RenderColors, StateVectorCallback};
use crate::render::state_panel_layout::StatePanelLayout;

#[allow(clippy::too_many_arguments)]
pub(super) fn paint_state_vector_gpu(
    app: &mut QniApp,
    painter: &egui::Painter,
    colors: &Colors,
    layout: &StatePanelLayout,
    viewport_rect: egui::Rect,
    grid_origin: egui::Pos2,
    screen_rect: egui::Rect,
    _recompute: bool,
    target_format: Option<wgpu::TextureFormat>,
) {
    let Some(target_format) = target_format else {
        return;
    };

    let render_colors = RenderColors::new(colors);
    let callback_rect = screen_rect;
    let cell_pitch = layout.size + layout.gap;
    let cols = layout.columns as u32;
    let rows = (layout.state_count / layout.columns.max(1)) as u32;
    let render_params = crate::gpu::RenderParams {
        viewport_min: [callback_rect.min.x, callback_rect.min.y],
        viewport_size: [callback_rect.width(), callback_rect.height()],
        panel_origin: [grid_origin.x, grid_origin.y],
        panel_size: [cols as f32 * cell_pitch, rows as f32 * cell_pitch],
        cell_pitch,
        radius: layout.radius,
        inner_radius: layout.inner_radius,
        stroke: layout.stroke,
        cols,
        rows,
        qubits: layout.qubits as u32,
        hovered_cell: app.state_panel.hovered_cell.map_or(-1, |c| c as i32),
        fill: render_colors.fill,
        disk_border: render_colors.disk_border,
        outline: render_colors.outline,
        outline_zero: render_colors.outline_zero,
        needle: render_colors.needle,
    };
    let mut update = app.state_vector_update();
    // A live gesture can retain the pickup state count until its next snap.
    update.state_count = layout.state_count;
    let callback = StateVectorCallback {
        update,
        target_format,
        render_params,
    };
    let clipped = painter.with_clip_rect(viewport_rect);
    let paint_callback = egui_wgpu::Callback::new_paint_callback(callback_rect, callback);
    clipped.add(egui::Shape::Callback(paint_callback));
}
