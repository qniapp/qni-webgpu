//! Host-owned restart action in the state panel strip.

use crate::colors::Colors;
use eframe::egui;

pub(crate) fn id() -> egui::Id {
    egui::Id::new("embed-restart")
}

pub(crate) fn interact(ui: &mut egui::Ui, rect: egui::Rect) -> bool {
    let response = ui.interact(rect, id(), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "量子回路を実行")
    });
    response
        .on_hover_text("量子回路を実行")
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

pub(crate) fn paint(painter: &egui::Painter, rect: egui::Rect, colors: &Colors) {
    let hovered = painter
        .ctx()
        .pointer_hover_pos()
        .is_some_and(|p| rect.contains(p));
    let focused = painter.ctx().memory(|memory| memory.has_focus(id()));
    if hovered || focused {
        painter.rect_stroke(
            rect,
            egui::CornerRadius::same(4),
            egui::Stroke::new(1.0, colors.surface),
            egui::StrokeKind::Inside,
        ); // Flexoki bg; rounded = 4px.
    }
    paint_icon(painter, rect.center(), colors.surface); // Flexoki bg.
}

fn paint_icon(painter: &egui::Painter, center: egui::Pos2, color: egui::Color32) {
    // spacing-4 = 16px icon, stroke-2 like the toolbar's vector icons.
    // A nearly full clockwise circle, not the toolbar's bent redo arrow.
    let points = (0..=40)
        .map(|i| {
            let angle = 0.1 + i as f32 / 40.0 * (std::f32::consts::TAU - 0.6);
            center + egui::vec2(angle.cos(), angle.sin()) * 8.0
        })
        .collect();
    let stroke = egui::Stroke::new(2.0, color);
    painter.add(egui::Shape::line(points, stroke));
    painter.add(egui::Shape::line(
        vec![
            center + egui::vec2(3.0, -7.0),
            center + egui::vec2(7.0, -4.0),
            center + egui::vec2(8.0, -9.0),
        ],
        stroke,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restart_icon_vector_snapshot() {
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            paint_icon(ui.painter(), egui::pos2(12.0, 12.0), Colors::new().surface);
        });
        insta::assert_debug_snapshot!(output.shapes);
    }
}
