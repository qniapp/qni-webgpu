use eframe::egui;

use crate::colors::Colors;

pub(super) const PAD_X: f32 = 16.0; // px-4
pub(super) const PAD_Y: f32 = 12.0; // py-3

pub(super) struct TooltipCard {
    pub(super) rect: egui::Rect,
}

pub(super) fn place_tooltip_card(
    screen_rect: egui::Rect,
    gate_rect: egui::Rect,
    content_size: egui::Vec2,
) -> TooltipCard {
    let card_size = egui::vec2(content_size.x + PAD_X * 2.0, content_size.y + PAD_Y * 2.0);

    // Anchor below the gate, clamped to the screen rect.
    let anchor = egui::pos2(gate_rect.left(), gate_rect.bottom() + 8.0);
    let max_left = screen_rect.right() - card_size.x - 8.0;
    let max_top = screen_rect.bottom() - card_size.y - 8.0;
    let card_min = egui::pos2(
        anchor.x.min(max_left).max(screen_rect.left() + 8.0),
        anchor.y.min(max_top),
    );

    TooltipCard {
        rect: egui::Rect::from_min_size(card_min, card_size),
    }
}

pub(super) fn paint_tooltip_card(painter: &egui::Painter, card_rect: egui::Rect, colors: &Colors) {
    let corner = egui::CornerRadius::same(8); // Tailwind rounded-lg
    let shadow = egui::epaint::Shadow {
        offset: [0, 6],
        blur: 16,
        spread: 0,
        color: egui::Color32::from_rgba_unmultiplied(0, 0, 0, 25),
    };

    painter.add(egui::Shape::Rect(shadow.as_shape(card_rect, corner)));
    painter.rect_filled(card_rect, corner, colors.surface);
    painter.rect_stroke(
        card_rect,
        corner,
        egui::Stroke::new(1.0, colors.box_border),
        egui::StrokeKind::Inside,
    );
}
