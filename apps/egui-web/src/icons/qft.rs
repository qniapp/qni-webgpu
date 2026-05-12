//! QFT lettering and resize handle drawing.

use eframe::egui;

/// QFT / QFT† lettering — a stylised "QFT" (and optional dagger mark)
/// drawn with the same line-segment primitives as the other gate
/// icons (H / Y / Z / etc.), translated directly from qni's
/// `qft-gate.svg` and `qft-dagger-gate.svg`. Renders into a square
/// `rect` (typically a GATE_SIZE square centred in the gate body);
/// stroke width tracks `rect.width()` so the lettering scales with
/// the body and stays visually consistent with other gates.
pub(super) fn draw_qft_lettering(
    painter: &egui::Painter,
    rect: egui::Rect,
    dagger: bool,
    color: egui::Color32,
) {
    // Stroke width is *always* derived from a 48-unit reference
    // viewbox so the line thickness matches every other gate icon
    // (H / Y / Z / …) regardless of which SVG's viewBox a sub-letter
    // came from. qni's SVGs all use `non-scaling-stroke` for the same
    // reason.
    let stroke = egui::Stroke::new(2.0 * rect.width() / 48.0, color);
    if dagger {
        // qni/packages/elements/icon/qft-dagger-gate.svg (viewBox 32×32).
        // The QFT lettering is shifted left to make room for a small †
        // mark at the top-right.
        let scale = rect.width() / 32.0;
        let p = |x: f32, y: f32| egui::pos2(rect.min.x + x * scale, rect.min.y + y * scale);
        // Q (open circle + short diagonal tail). The original SVG's
        // tail uses a transformed line; the endpoints below are the
        // pre-computed result of `matrix(...) * (1, -1)..(3.34, -1)`.
        painter.circle_stroke(p(5.97, 16.0), 2.97 * scale, stroke);
        painter.line_segment([p(7.34, 18.76), p(8.68, 20.69)], stroke);
        // F (vertical + top bar + middle bar).
        painter.line_segment([p(13.0, 12.0), p(13.0, 20.0)], stroke);
        painter.line_segment([p(13.0, 12.0), p(16.45, 12.0)], stroke);
        painter.line_segment([p(14.0, 16.0), p(16.0, 16.0)], stroke);
        // T (top bar + vertical).
        painter.line_segment([p(20.10, 12.0), p(25.04, 12.0)], stroke);
        painter.line_segment([p(22.7, 13.0), p(22.7, 20.0)], stroke);
        // † small cross at top-right.
        painter.line_segment([p(26.2, 7.48), p(29.2, 7.48)], stroke);
        painter.line_segment([p(27.7, 6.0), p(27.7, 11.0)], stroke);
    } else {
        // qni/packages/elements/icon/qft-gate.svg (viewBox 48×48).
        let scale = rect.width() / 48.0;
        let p = |x: f32, y: f32| egui::pos2(rect.min.x + x * scale, rect.min.y + y * scale);
        // Q (open circle + tail).
        painter.circle_stroke(p(11.5, 23.5), 5.5 * scale, stroke);
        painter.line_segment([p(13.39, 27.28), p(16.28, 31.61)], stroke);
        // F (vertical + top + middle).
        painter.line_segment([p(21.0, 17.0), p(21.0, 30.0)], stroke);
        painter.line_segment([p(21.0, 17.0), p(28.0, 17.0)], stroke);
        painter.line_segment([p(21.0, 23.0), p(27.0, 23.0)], stroke);
        // T (top + vertical).
        painter.line_segment([p(32.0, 17.0), p(42.0, 17.0)], stroke);
        painter.line_segment([p(37.0, 18.0), p(37.0, 30.0)], stroke);
    }
}

/// QFT gate's bottom-edge resize handle — a small horizontal chevron
/// (▽/△ stacked) shown on hover. The `rect` is the handle's bounding
/// box at the bottom of the gate body; `bg` colours the strip behind
/// the chevron (idle / hover variants).
pub(crate) fn draw_qft_resize_handle(painter: &egui::Painter, rect: egui::Rect, bg: egui::Color32) {
    painter.rect_filled(rect, egui::CornerRadius::same(6), bg);
    // Two stacked triangles forming a vertical chevron selector.
    let cx = rect.center().x;
    let cy = rect.center().y;
    let h = rect.height();
    let tri_h = (h * 0.22).max(3.0);
    let tri_half_w = tri_h * 1.1;
    let gap = (h * 0.08).max(1.5);
    let up_tip = egui::pos2(cx, cy - gap - tri_h);
    let up_l = egui::pos2(cx - tri_half_w, cy - gap);
    let up_r = egui::pos2(cx + tri_half_w, cy - gap);
    let down_tip = egui::pos2(cx, cy + gap + tri_h);
    let down_l = egui::pos2(cx - tri_half_w, cy + gap);
    let down_r = egui::pos2(cx + tri_half_w, cy + gap);
    let arrow_color = egui::Color32::WHITE;
    painter.add(egui::Shape::convex_polygon(
        vec![up_l, up_r, up_tip],
        arrow_color,
        egui::Stroke::NONE,
    ));
    painter.add(egui::Shape::convex_polygon(
        vec![down_l, down_r, down_tip],
        arrow_color,
        egui::Stroke::NONE,
    ));
}
