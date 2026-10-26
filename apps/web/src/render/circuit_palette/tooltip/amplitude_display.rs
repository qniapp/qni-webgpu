use eframe::egui;

use crate::colors::Colors;
use crate::gates::Amp;

pub(super) const CIRCLE: f32 = crate::tailwind::SPACING_8;
pub(super) const CIRCLE_GAP: f32 = 8.0;

pub(super) fn draw_tooltip_amps(
    painter: &egui::Painter,
    left_x: f32,
    top_y: f32,
    amps: &[Amp; 2],
    colors: &Colors,
) {
    for (basis, amp) in amps.iter().enumerate() {
        let center = egui::pos2(
            left_x + CIRCLE / 2.0 + basis as f32 * (CIRCLE + CIRCLE_GAP),
            top_y + CIRCLE / 2.0,
        );
        let prob = amp.probability().clamp(0.0, 1.0);
        let is_zero = prob < 1e-6;
        let outline = if is_zero {
            colors.state_outline_zero
        } else {
            colors.state_outline
        };
        painter.circle_stroke(center, CIRCLE / 2.0, egui::Stroke::new(1.5_f32, outline));
        if !is_zero {
            let inner_r = (CIRCLE / 2.0) * prob.sqrt();
            painter.circle_filled(center, inner_r, colors.state_fill);
            let phase = amp.phase();
            let tip = egui::pos2(
                center.x + phase.sin() * (CIRCLE / 2.0),
                center.y - phase.cos() * (CIRCLE / 2.0),
            );
            painter.line_segment(
                [center, tip],
                egui::Stroke::new(2.0_f32, colors.state_needle),
            );
        }
        let label = if basis == 0 { "|0⟩" } else { "|1⟩" };
        painter.text(
            egui::pos2(center.x + CIRCLE / 2.0 - 7.0, center.y + CIRCLE / 2.0 - 6.0),
            egui::Align2::LEFT_TOP,
            label,
            egui::FontId::monospace(10.0),
            colors.text,
        );
    }
}
