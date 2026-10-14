use eframe::egui;

use crate::colors::{with_alpha, Colors};

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
        color: colors.state_panel_shadow,
    };
    painter.add(egui::Shape::Rect(
        state_shadow.as_shape(state_rect, state_corner),
    ));
    painter.rect_filled(state_rect, state_corner, colors.surface);
}

pub(super) fn paint_capacity_error(
    painter: &egui::Painter,
    colors: &Colors,
    viewport_rect: egui::Rect,
    message: &str,
) {
    let center = viewport_rect.center();
    let card = egui::Rect::from_center_size(
        center,
        egui::vec2(
            (viewport_rect.width() - crate::tailwind::SPACING_8).clamp(160.0, 420.0),
            crate::tailwind::SPACING_16,
        ),
    );
    painter.rect_filled(
        card,
        egui::CornerRadius::same(crate::tailwind::ROUNDED_XL),
        with_alpha(colors.semantic_off, 18), // Flexoki red-600 alpha.
    );
    painter.rect_stroke(
        card,
        egui::CornerRadius::same(crate::tailwind::ROUNDED_XL),
        egui::Stroke::new(1.0_f32, colors.semantic_off), // Flexoki red-600.
        egui::StrokeKind::Outside,
    );
    painter.text(
        egui::pos2(center.x, center.y - crate::tailwind::SPACING_3),
        egui::Align2::CENTER_CENTER,
        "GPU capacity limit exceeded",
        egui::FontId::new(crate::tailwind::TEXT_SM, egui::FontFamily::Proportional),
        colors.semantic_off, // Flexoki red-600.
    );
    painter.text(
        egui::pos2(center.x, center.y + crate::tailwind::SPACING_3),
        egui::Align2::CENTER_CENTER,
        truncate_capacity_message(message),
        egui::FontId::new(crate::tailwind::TEXT_XS, egui::FontFamily::Proportional),
        colors.text, // Flexoki tx-2.
    );
}

fn truncate_capacity_message(message: &str) -> String {
    const MAX_CHARS: usize = 72;
    if message.chars().count() <= MAX_CHARS {
        return message.to_owned();
    }
    let mut truncated = message.chars().take(MAX_CHARS - 1).collect::<String>();
    truncated.push('…');
    truncated
}
