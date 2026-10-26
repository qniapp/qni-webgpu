//! Bloch sphere and center dot icon drawing.

use eframe::egui;

use crate::colors::Colors;

pub(super) fn draw_bloch_sphere(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let center = rect.center();
    let radius = rect.width().min(rect.height()) * 0.5 - 1.0;
    let stroke = egui::Stroke::new(1.0_f32, color);
    // Decorative wireframe matches the static SVG in
    // `packages/elements/src/bloch-display-element.ts`:
    //   horizontal x-axis line, vertical z-axis line, NE/SW diagonal y-axis,
    //   then a vertical thin ellipse and a horizontal thin ellipse.
    painter.line_segment(
        [
            egui::pos2(center.x - radius, center.y),
            egui::pos2(center.x + radius, center.y),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(center.x, center.y - radius),
            egui::pos2(center.x, center.y + radius),
        ],
        stroke,
    );
    let diag = radius * 0.30; // matches qni's 35%/65% endpoints (offset 15%)
    painter.line_segment(
        [
            egui::pos2(center.x - diag, center.y + diag),
            egui::pos2(center.x + diag, center.y - diag),
        ],
        stroke,
    );
    let thin = radius * 0.36; // matches qni's rx=18%, ry=18% (radius from cx/cy)
    let ellipse = |rx: f32, ry: f32| -> Vec<egui::Pos2> {
        (0..=48)
            .map(|i| {
                let t = i as f32 / 48.0 * std::f32::consts::TAU;
                egui::pos2(center.x + rx * t.cos(), center.y + ry * t.sin())
            })
            .collect()
    };
    painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
        ellipse(thin, radius),
        stroke,
    )));
    painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
        ellipse(radius, thin),
        stroke,
    )));
}

pub(crate) fn draw_bloch_center_dot(painter: &egui::Painter, rect: egui::Rect, colors: &Colors) {
    let center = rect.center();
    painter.circle_filled(center, 4.0, colors.bloch_vector_zero);
    painter.circle_stroke(
        center,
        4.0,
        egui::Stroke::new(1.0_f32, colors.bloch_vector_tip_outline),
    );
}
