use eframe::egui;

use crate::colors::Colors;

pub(super) fn paint_panel_background(
    painter: &egui::Painter,
    colors: &Colors,
    state_rect: egui::Rect,
) {
    let state_corner = egui::CornerRadius::same(14);
    let state_shadow = egui::epaint::Shadow {
        offset: [0, 6],
        blur: 16,
        spread: 0,
        color: egui::Color32::from_rgba_unmultiplied(0, 0, 0, 25),
    };
    painter.add(egui::Shape::Rect(
        state_shadow.as_shape(state_rect, state_corner),
    ));
    painter.rect_filled(state_rect, state_corner, colors.surface);
}
