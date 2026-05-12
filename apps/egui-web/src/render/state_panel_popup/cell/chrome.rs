use eframe::egui;

use super::geometry::PopupGeometry;
use crate::colors::Colors;

pub(super) fn paint_popup_chrome(painter: &egui::Painter, colors: &Colors, popup: &PopupGeometry) {
    paint_card(painter, colors, popup.rect);
    paint_tail(painter, colors, popup);
}

fn paint_card(painter: &egui::Painter, colors: &Colors, rect: egui::Rect) {
    // Drop shadow → paper fill → ui-2 hairline border (B variant).
    let corner = egui::CornerRadius::same(10);
    let shadow = egui::epaint::Shadow {
        offset: [0, 10],
        blur: 28,
        spread: 0,
        color: egui::Color32::from_rgba_unmultiplied(0, 0, 0, 36),
    };
    painter.add(egui::Shape::Rect(shadow.as_shape(rect, corner)));
    painter.rect_filled(rect, corner, colors.surface);
    painter.rect_stroke(
        rect,
        corner,
        egui::Stroke::new(1.0, colors.state_outline_zero),
        egui::StrokeKind::Inside,
    );
}

fn paint_tail(painter: &egui::Painter, colors: &Colors, popup: &PopupGeometry) {
    // Tail (small triangle pointing at the cell). Filled paper + matching ui-2
    // stroke on the two slanted sides so it reads as part of the bordered card.
    // Uses the un-clamped horizontal anchor so the apex always lands on the
    // cell.
    let apex = popup.tail_apex();
    let base_l = popup.tail_base_l();
    let base_r = popup.tail_base_r();
    painter.add(egui::Shape::convex_polygon(
        vec![apex, base_l, base_r],
        colors.surface,
        egui::Stroke::NONE,
    ));
    let border_stroke = egui::Stroke::new(1.0, colors.state_outline_zero);
    painter.line_segment([base_l, apex], border_stroke);
    painter.line_segment([apex, base_r], border_stroke);

    // Repaint the popup-body edge between the tail bases so the border doesn't
    // show through under the tail.
    painter.line_segment(
        [
            egui::pos2(base_l.x + 0.5, base_l.y),
            egui::pos2(base_r.x - 0.5, base_r.y),
        ],
        egui::Stroke::new(1.0, colors.surface),
    );
}
