use eframe::egui;

use crate::app::QniApp;
use crate::colors::Colors;
use crate::constants::{
    PALETTE_CORNER_RADIUS, PALETTE_PADDING_X, PALETTE_PADDING_Y, PALETTE_ROW_Y,
    PALETTE_SEPARATOR_WIDTH, PALETTE_SIZE,
};
use crate::gates::{palette_gate_kind, GateKind, PALETTE_GATE_COUNT};
use crate::icons::{draw_bloch_vector, draw_density_palette_icon, draw_gate_body};
use crate::layout::{palette_gate_local_pos, palette_layout, palette_start_x};

use super::super::hover_frame::{hover_frame_corner_radius, hover_frame_inner_corner_radius};

impl QniApp {
    pub(crate) fn draw_palette(&self, painter: &egui::Painter, rect: egui::Rect, colors: &Colors) {
        let layout = palette_layout();
        let palette_start_x = palette_start_x(rect.width(), &layout);
        let palette_rect = egui::Rect::from_min_size(
            rect.min
                + egui::vec2(
                    palette_start_x - PALETTE_PADDING_X,
                    PALETTE_ROW_Y - PALETTE_PADDING_Y,
                ),
            egui::vec2(
                layout.total_width + PALETTE_PADDING_X * 2.0,
                layout.total_height + PALETTE_PADDING_Y * 2.0,
            ),
        );
        let palette_corner = egui::CornerRadius::same(PALETTE_CORNER_RADIUS);
        let shadow = egui::epaint::Shadow {
            offset: [0, 6],
            blur: 16,
            spread: 0,
            color: colors.palette_shadow,
        };
        painter.add(egui::Shape::Rect(
            shadow.as_shape(palette_rect, palette_corner),
        ));
        painter.rect_filled(palette_rect, palette_corner, colors.surface);

        let palette_origin = rect.min + egui::vec2(palette_start_x, PALETTE_ROW_Y);
        let separator_rect = egui::Rect::from_min_size(
            palette_origin + egui::vec2(layout.separator_x, 0.0),
            egui::vec2(PALETTE_SEPARATOR_WIDTH, layout.total_height),
        );
        // Flexoki ui-2: section separator between operation gates and displays.
        painter.rect_filled(separator_rect, egui::CornerRadius::ZERO, colors.line);

        for index in 0..PALETTE_GATE_COUNT {
            let Some(gate) = palette_gate_kind(index) else {
                continue;
            };
            let Some(local) = palette_gate_local_pos(index, &layout) else {
                continue;
            };
            let gate_rect = egui::Rect::from_min_size(
                palette_origin + local.to_vec2(),
                egui::vec2(PALETTE_SIZE, PALETTE_SIZE),
            );
            if self.hovered_palette_index == Some(index) {
                let hover_outer = gate_rect.expand(4.0);
                let hover_inner = gate_rect.expand(2.0);
                painter.rect_filled(
                    hover_outer,
                    hover_frame_corner_radius(gate),
                    colors.gate_hover_border,
                );
                // ホバー枠の内側はパレット面と同じ色にし、Measurement / Spacer のような
                // 本体 fill を持たないゲートでも背景が灰色へ変わらないようにする。
                // 角ばり表示ブロックは内側の抜きも角丸ゼロにして、90° の枠を保つ。
                painter.rect_filled(
                    hover_inner,
                    hover_frame_inner_corner_radius(gate),
                    colors.surface,
                );
            }
            if gate == GateKind::DensityMatrixDisplay {
                draw_density_palette_icon(painter, gate_rect, colors);
            } else {
                draw_gate_body(painter, gate_rect, gate, colors);
            }
            if gate == GateKind::BlochDisplay {
                // Palette has no associated state: render qni's d=0 blue center dot.
                draw_bloch_vector(painter, gate_rect, [0.0, 0.0, 0.0], colors);
            }
        }
    }
}
