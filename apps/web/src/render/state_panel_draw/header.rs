use eframe::egui;

use crate::colors::Colors;
use crate::constants::STATE_PANEL_CORNER_RADIUS;
use crate::render::state_panel_layout::StatePanelLayout;

pub(super) fn paint_header_strip(
    painter: &egui::Painter,
    colors: &Colors,
    layout: &StatePanelLayout,
    state_rect: egui::Rect,
    handle_height: f32,
) -> egui::Rect {
    // G-2 header strip: zinc-100 bar with qubit count on the left and
    // "cols × rows = N states" on the right. Top corners follow the
    // panel's corner radius; the bottom edge is flat where the strip
    // meets the white panel body.
    let handle_rect = egui::Rect::from_min_size(
        state_rect.min,
        egui::vec2(state_rect.width(), handle_height.max(6.0)),
    );
    let handle_corner = egui::CornerRadius {
        nw: 14,
        ne: 14,
        sw: 0,
        se: 0,
    };
    painter.rect_filled(handle_rect, handle_corner, colors.state_handle_bg);

    paint_header_text(painter, colors, layout, handle_rect);
    handle_rect
}

fn paint_header_text(
    painter: &egui::Painter,
    colors: &Colors,
    layout: &StatePanelLayout,
    handle_rect: egui::Rect,
) {
    // Strip text starts past the corner resize-handle area (panel rounded R +
    // breathing). Keeps "16 qubits" / "256 × 256 = …" from touching the
    // curved handle marks at the top corners.
    let strip_padding_x = STATE_PANEL_CORNER_RADIUS + 6.0;
    // text-sm (14px) — Tailwind default. Matches the popup header so both
    // blue-on-paper "card chrome" text sits at the same step on the type scale.
    let strip_font = egui::FontId::monospace(14.0);
    let qubits_label = if layout.qubits == 1 {
        "qubit"
    } else {
        "qubits"
    };
    let states_label = if layout.state_count == 1 {
        "state"
    } else {
        "states"
    };
    let qubits_text = format!("{} {}", layout.qubits, qubits_label);
    let rows = layout.state_count / layout.columns.max(1);
    // " ▾" indicates the dimensions text opens the aspect popover.
    let states_text = format!(
        "{} × {} = {} {} ▾",
        layout.columns, rows, layout.state_count, states_label
    );
    // sky-500 strip → white text for legibility.
    painter.text(
        handle_rect.left_center() + egui::vec2(strip_padding_x, 0.0),
        egui::Align2::LEFT_CENTER,
        qubits_text,
        strip_font.clone(),
        colors.surface,
    );
    painter.text(
        handle_rect.right_center() - egui::vec2(strip_padding_x, 0.0),
        egui::Align2::RIGHT_CENTER,
        states_text,
        strip_font,
        colors.surface,
    );
}
