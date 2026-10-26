use eframe::egui;

mod chrome;
mod geometry;
mod rows;
mod value_callback;

use crate::colors::Colors;
use crate::render::state_panel_layout::StatePanelLayout;

pub(crate) fn draw_state_cell_popup(
    painter: &egui::Painter,
    colors: &Colors,
    layout: &StatePanelLayout,
    grid_origin: egui::Pos2,
    screen_rect: egui::Rect,
    display_index: u32,
) {
    let qubits = layout.qubits.max(1) as u32;
    let popup = geometry::layout_popup(layout, grid_origin, screen_rect, display_index);

    chrome::paint_popup_chrome(painter, colors, &popup);
    rows::paint_popup_rows(painter, colors, &popup, display_index, qubits);
    value_callback::paint_popup_values(painter, colors, layout, screen_rect, display_index, &popup);
}
