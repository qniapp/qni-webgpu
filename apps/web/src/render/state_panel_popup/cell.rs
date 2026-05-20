use eframe::egui;

mod chrome;
mod geometry;
mod rows;
mod value_callback;

use crate::colors::Colors;
use crate::render::state_panel_layout::StatePanelLayout;

/// Hover popup showing the |ket⟩ + amplitude / probability / phase for
/// the cell under the pointer. Paper background + ui-2 1px border +
/// soft shadow (B variant from `docs/state-cell-popup-mockups.html`),
/// with qni-style icons rendered as egui primitives.
///
/// Numeric amplitude / probability / phase values are rendered by
/// `PopupValueCallback` on the GPU. The CPU computes only layout,
/// colours, and glyph atlas placement; it performs no value readback.
pub(crate) fn draw_state_cell_popup(
    painter: &egui::Painter,
    colors: &Colors,
    layout: &StatePanelLayout,
    grid_origin: egui::Pos2,
    viewport_rect: egui::Rect,
    screen_rect: egui::Rect,
    display_index: u32,
) {
    let qubits = layout.qubits.max(1) as u32;
    let popup = geometry::layout_popup(layout, grid_origin, viewport_rect, display_index);

    chrome::paint_popup_chrome(painter, colors, &popup);
    rows::paint_popup_rows(painter, colors, &popup, display_index, qubits);
    value_callback::paint_popup_values(painter, colors, layout, screen_rect, display_index, &popup);
}
