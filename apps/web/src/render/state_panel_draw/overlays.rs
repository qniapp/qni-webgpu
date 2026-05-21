use eframe::egui;

use super::super::{state_panel_chrome, state_panel_popup};
use crate::app::{QniApp, ResizeCorner};
use crate::colors::Colors;
use crate::render::state_panel_layout::StatePanelLayout;

#[allow(clippy::too_many_arguments)]
pub(super) fn paint_state_panel_overlays(
    app: &mut QniApp,
    painter: &egui::Painter,
    colors: &Colors,
    layout: &StatePanelLayout,
    _offset: egui::Vec2,
    state_rect: egui::Rect,
    viewport_rect: egui::Rect,
    grid_origin: egui::Pos2,
    screen_rect: egui::Rect,
) {
    state_panel_chrome::draw_state_minimap(painter, colors, layout, viewport_rect, grid_origin);
    paint_aspect_popover(app, painter, colors, layout, _offset, state_rect);
    paint_resize_handles(app, painter, colors, state_rect);
    paint_cell_popup(app, painter, colors, layout, grid_origin, screen_rect);
}

fn paint_aspect_popover(
    app: &QniApp,
    painter: &egui::Painter,
    colors: &Colors,
    layout: &StatePanelLayout,
    offset: egui::Vec2,
    state_rect: egui::Rect,
) {
    // Aspect popover — only draw when open. Its chrome mirrors the Circuit
    // picker, opens below the dimensions trigger, and its right edge aligns
    // with the state-vector panel right edge.
    if !app.state_panel.aspect_popover_open {
        return;
    }

    let dims_rect = QniApp::dims_hit_rect(painter.ctx(), layout, offset);
    let (pop_rect, row_rects) = QniApp::aspect_popover_layout(
        state_rect,
        dims_rect,
        layout.qubits,
        painter.ctx().content_rect(),
    );
    state_panel_popup::draw_aspect_popover(
        painter,
        colors,
        state_rect,
        dims_rect,
        pop_rect,
        &row_rects,
        layout.qubits,
        app.state_panel.aspect_index.min(layout.qubits),
    );
}

fn paint_resize_handles(
    app: &QniApp,
    painter: &egui::Painter,
    colors: &Colors,
    state_rect: egui::Rect,
) {
    // Resize handles — 4 corner arcs concentric with the panel's rounded
    // corners (G 案 / 内側配置). Drawn after the GPU pass so they sit on top of
    // the circle grid at all zoom levels. Color follows the local background:
    // sky-tone for the top handles (on the sky-500 strip), neutral gray for
    // the bottom handles (on the white panel).
    for corner in [
        ResizeCorner::TopLeft,
        ResizeCorner::TopRight,
        ResizeCorner::BottomLeft,
        ResizeCorner::BottomRight,
    ] {
        let dragging = app.active_resize_corner() == Some(corner);
        let color = match (corner.is_top(), dragging) {
            (true, false) => colors.state_resize_handle_top_idle,
            (true, true) => colors.state_resize_handle_top_drag,
            (false, false) => colors.state_resize_handle_bottom_idle,
            (false, true) => colors.state_resize_handle_bottom_drag,
        };
        state_panel_chrome::draw_resize_handle_arc(painter, corner, state_rect, color);
    }
}

fn paint_cell_popup(
    app: &QniApp,
    painter: &egui::Painter,
    colors: &Colors,
    layout: &StatePanelLayout,
    grid_origin: egui::Pos2,
    screen_rect: egui::Rect,
) {
    // Cell hover popup (B — Paper + ui-2 1px border + shadow, with qni-style
    // amplitude / probability / phase icons). Drawn last so it lifts above the
    // resize handles + minimap + GPU circle pass. Suppressed while a capacity
    // error is visible so stale GPU state values never overlap the error card.
    if app.gpu_plan.capacity_error().is_some() {
        return;
    }
    if let Some(cell) = app.state_panel.hovered_cell {
        state_panel_popup::draw_state_cell_popup(
            painter,
            colors,
            layout,
            grid_origin,
            screen_rect,
            cell,
        );
    }
}
