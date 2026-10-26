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
    if layout.show_restart_button {
        super::super::restart_button::paint(
            painter,
            layout.restart_rect(state_rect.min - layout.state_rect.min),
            colors,
        );
    }
    handle_rect
}

fn paint_header_text(
    painter: &egui::Painter,
    colors: &Colors,
    layout: &StatePanelLayout,
    handle_rect: egui::Rect,
) {
    let strip_padding_x = STATE_PANEL_CORNER_RADIUS + 6.0;
    let strip_font = egui::FontId::monospace(crate::tailwind::TEXT_SM);
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
    let states_text = format!(
        "{} × {} = {} {} ▾",
        layout.columns, rows, layout.state_count, states_label
    );
    let labels_width = painter.ctx().fonts_mut(|fonts| {
        fonts
            .layout_no_wrap(qubits_text.clone(), strip_font.clone(), colors.surface)
            .size()
            .x
            + fonts
                .layout_no_wrap(states_text.clone(), strip_font.clone(), colors.surface)
                .size()
                .x
    });
    if labels_width + strip_padding_x * 2.0 + layout.restart_space() + crate::tailwind::SPACING_4
        <= handle_rect.width()
    {
        painter.text(
            handle_rect.left_center() + egui::vec2(strip_padding_x, 0.0),
            egui::Align2::LEFT_CENTER,
            qubits_text,
            strip_font.clone(),
            colors.surface,
        );
    }
    let text_painter =
        painter.with_clip_rect(painter.clip_rect().intersect(egui::Rect::from_min_max(
            handle_rect.min,
            handle_rect.max - egui::vec2(strip_padding_x + layout.restart_space(), 0.0),
        )));
    text_painter.text(
        handle_rect.right_center() - egui::vec2(strip_padding_x + layout.restart_space(), 0.0),
        egui::Align2::RIGHT_CENTER,
        states_text,
        strip_font,
        colors.surface,
    );
}
