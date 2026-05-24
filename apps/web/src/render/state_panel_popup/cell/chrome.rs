use eframe::egui;

use super::geometry::PopupGeometry;
use crate::colors::Colors;

const TAIL_SEAM_CARD_OVERLAP: f32 = 2.0;
const TAIL_SEAM_TAIL_OVERLAP: f32 = 0.5;
const TAIL_SEAM_X_PAD: f32 = 0.75;

pub(super) fn paint_popup_chrome(painter: &egui::Painter, colors: &Colors, popup: &PopupGeometry) {
    paint_card(painter, colors, popup.rect);
    paint_tail(painter, colors, popup);
}

fn paint_card(painter: &egui::Painter, colors: &Colors, rect: egui::Rect) {
    // Drop shadow → paper fill → tx-3 hairline border so the paper tail stays readable.
    let corner = egui::CornerRadius::same(10);
    let shadow = egui::epaint::Shadow {
        offset: [0, 10],
        blur: 28,
        spread: 0,
        color: colors.state_cell_popup_shadow,
    };
    painter.add(egui::Shape::Rect(shadow.as_shape(rect, corner)));
    painter.rect_filled(rect, corner, colors.surface);
    painter.rect_stroke(
        rect,
        corner,
        egui::Stroke::new(1.0, colors.popover_outline),
        egui::StrokeKind::Inside,
    );
}

fn paint_tail(painter: &egui::Painter, colors: &Colors, popup: &PopupGeometry) {
    // Tail (small triangle pointing at the cell). Filled paper + matching tx-3
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

    // The card border is painted before the tail. Cover the border segment
    // hidden by the tail with a tiny surface patch that overlaps the card and
    // tail; a 1px line at the exact base can miss the anti-aliased inside
    // stroke and leave a hairline seam.
    let base_y = base_l.y;
    let cover_y = if apex.y > base_y {
        (base_y - TAIL_SEAM_CARD_OVERLAP)..=(base_y + TAIL_SEAM_TAIL_OVERLAP)
    } else {
        (base_y - TAIL_SEAM_TAIL_OVERLAP)..=(base_y + TAIL_SEAM_CARD_OVERLAP)
    };
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(base_l.x - TAIL_SEAM_X_PAD, *cover_y.start()),
            egui::pos2(base_r.x + TAIL_SEAM_X_PAD, *cover_y.end()),
        ),
        egui::CornerRadius::ZERO,
        colors.surface,
    );

    let border_stroke = egui::Stroke::new(1.0, colors.popover_outline);
    painter.line_segment([base_l, apex], border_stroke);
    painter.line_segment([apex, base_r], border_stroke);
}
