//! Resizable-span handle drawing for QFT-family and Chance gates.
//!
//! QFT / QFT† labels are rendered alongside every other typographic gate by
//! `gate_glyphs::draw_gate_icon`; this module only owns the small resize
//! affordances.

use eframe::egui;

/// Chance gate's top / bottom resize pill. `rect` is the spec-sized 24×6 px
/// handle box; scale / alpha model docs/chance-display.html §11 states.
pub(crate) fn draw_chance_resize_handle(
    painter: &egui::Painter,
    rect: egui::Rect,
    bg: egui::Color32,
    scale: f32,
    alpha: f32,
) {
    if alpha <= 0.0 || scale <= 0.0 {
        return;
    }
    let color = crate::colors::with_alpha(bg, (alpha.clamp(0.0, 1.0) * 255.0).round() as u8);
    let visual = egui::Rect::from_center_size(rect.center(), rect.size() * scale.max(0.0));
    painter.rect_filled(
        visual,
        egui::CornerRadius::same((3.0 * scale).round().clamp(1.0, 8.0) as u8),
        color,
    );
}

/// QFT gate's bottom-edge resize handle — a small horizontal chevron
/// (▽/△ stacked) shown on hover. The `rect` is the handle's bounding
/// box at the bottom of the gate body; `bg` colours the strip behind
/// the chevron (idle / hover variants).
pub(crate) fn draw_qft_resize_handle(
    painter: &egui::Painter,
    rect: egui::Rect,
    bg: egui::Color32,
    arrow_color: egui::Color32,
) {
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
