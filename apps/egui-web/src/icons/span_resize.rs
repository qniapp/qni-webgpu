//! Resizable-span handle drawing for Chance / QFT / QFT† gates.

use eframe::egui;

/// Top / bottom resize pill shared by all resizable-span gates. `rect` is
/// the spec-sized 24×6 px handle box; scale / alpha model
/// docs/chance-display.html §11 states.
pub(crate) fn draw_span_resize_handle(
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
