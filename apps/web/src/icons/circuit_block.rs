//! Comment-block palette icon: a circuit block shrunk into a 40 px cell,
//! with short comment bars above and below and two wires inside.

use eframe::egui;

use crate::colors::Colors;

const ICON_SIZE: f32 = 40.0;
const RULE: (f32, f32) = (4.0, 36.0);
const COMMENT: (f32, f32) = (10.0, 30.0);
const STROKE: f32 = 2.0;
const COMMENT_YS: [f32; 2] = [4.0, 34.0];
const RULE_YS: [f32; 2] = [9.0, 29.0];
const WIRE_YS: [f32; 2] = [15.0, 23.0];

fn band(rect: egui::Rect, (left, right): (f32, f32), top: f32, height: f32) -> egui::Rect {
    let scale = rect.width() / ICON_SIZE;
    egui::Rect::from_min_size(
        rect.min + egui::vec2(left, top) * scale,
        egui::vec2(right - left, height) * scale,
    )
}

pub(crate) fn draw_circuit_block_palette_icon(
    painter: &egui::Painter,
    rect: egui::Rect,
    colors: &Colors,
) {
    painter.rect_filled(
        band(rect, RULE, RULE_YS[0], RULE_YS[1] - RULE_YS[0] + STROKE),
        0.0,
        colors.circuit_block_fill,
    );
    for y in WIRE_YS {
        painter.rect_filled(band(rect, RULE, y, STROKE), 0.0, colors.line);
    }
    for y in RULE_YS {
        painter.rect_filled(
            band(rect, RULE, y, STROKE),
            0.0,
            colors.circuit_block_border,
        );
    }
    for y in COMMENT_YS {
        painter.rect_filled(
            band(rect, COMMENT, y, STROKE),
            1.0,
            colors.circuit_block_label,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circuit_block_palette_icon_vector_snapshot() {
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            draw_circuit_block_palette_icon(
                ui.painter(),
                egui::Rect::from_min_size(egui::pos2(100.0, 20.0), egui::vec2(40.0, 40.0)),
                &Colors::new(),
            );
        });
        insta::assert_debug_snapshot!(output.shapes);
    }
}
