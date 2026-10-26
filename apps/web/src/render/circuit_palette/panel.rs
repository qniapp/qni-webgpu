use eframe::egui;

use crate::app::QniApp;
use crate::colors::Colors;
use crate::constants::{
    CAROUSEL_DOT_SIZE, CAROUSEL_GATES_PER_PAGE, PALETTE_CORNER_RADIUS, PALETTE_PADDING_X,
    PALETTE_PADDING_Y, PALETTE_ROW_Y, PALETTE_SIZE,
};
use crate::gates::GateKind;
use crate::icons::{
    draw_bloch_center_dot, draw_circuit_block_palette_icon, draw_density_palette_icon,
    draw_gate_body,
};
use crate::layout::{palette_start_x, PaletteCarousel, PaletteItem};

use super::super::hover_frame::{hover_frame_corner_radius, hover_frame_inner_corner_radius};

impl QniApp {
    pub(crate) fn draw_palette(&self, painter: &egui::Painter, rect: egui::Rect, colors: &Colors) {
        if self.library.active_locked() || self.palette.is_hidden() {
            return;
        }
        if self.orientation.is_vertical() {
            self.draw_palette_carousel(painter, rect, colors);
            return;
        }
        let layout = self.palette.layout(rect.width());
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
        painter.add(egui::Shape::Rect(
            palette_shadow(colors).as_shape(palette_rect, palette_corner),
        ));
        painter.rect_filled(palette_rect, palette_corner, colors.surface);

        let palette_origin = rect.min + egui::vec2(palette_start_x, PALETTE_ROW_Y);
        if let Some(separator) = layout.separator {
            painter.rect_filled(
                separator.translate(palette_origin.to_vec2()),
                egui::CornerRadius::ZERO,
                colors.line,
            );
        }

        for index in 0..self.palette.len() {
            let Some(local) = self.palette.local_pos(index, &layout) else {
                continue;
            };
            let gate_rect = egui::Rect::from_min_size(
                palette_origin + local.to_vec2(),
                egui::vec2(PALETTE_SIZE, PALETTE_SIZE),
            );
            self.draw_palette_entry(painter, index, gate_rect, colors);
        }
    }

    fn draw_palette_entry(
        &self,
        painter: &egui::Painter,
        index: usize,
        gate_rect: egui::Rect,
        colors: &Colors,
    ) {
        let gate = match self.palette.item(index) {
            Some(PaletteItem::Gate(entry)) => entry.kind,
            Some(PaletteItem::CircuitBlock) => {
                if self.hovered_palette_index == Some(index) {
                    painter.rect_filled(
                        gate_rect.expand(4.0),
                        egui::CornerRadius::ZERO,
                        colors.gate_hover_border,
                    );
                    painter.rect_filled(
                        gate_rect.expand(2.0),
                        egui::CornerRadius::ZERO,
                        colors.surface,
                    );
                }
                draw_circuit_block_palette_icon(painter, gate_rect, colors);
                return;
            }
            None => return,
        };
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
            draw_bloch_center_dot(painter, gate_rect, colors);
        }
    }

    fn draw_palette_carousel(&self, painter: &egui::Painter, rect: egui::Rect, colors: &Colors) {
        let carousel = PaletteCarousel::new(rect, self.palette.len());
        let card = carousel.rect();
        let corner = egui::CornerRadius::same(PALETTE_CORNER_RADIUS);
        painter.add(egui::Shape::Rect(
            palette_shadow(colors).as_shape(card, corner),
        ));
        painter.rect_filled(card, corner, colors.surface);
        let page = carousel.clamp_page(self.palette_page);
        for slot in 0..CAROUSEL_GATES_PER_PAGE {
            if let Some(index) = self
                .palette
                .carousel_entry(page * CAROUSEL_GATES_PER_PAGE + slot)
            {
                self.draw_palette_entry(painter, index, carousel.slot_rect(slot), colors);
            }
        }
        if !carousel.paged() {
            return;
        }
        let arrow_color = |enabled: bool| {
            if enabled {
                colors.carousel_arrow
            } else {
                colors.carousel_arrow_disabled
            }
        };
        draw_chevron(
            painter,
            carousel.prev_rect().center(),
            -1.0,
            arrow_color(page > 0),
        );
        draw_chevron(
            painter,
            carousel.next_rect().center(),
            1.0,
            arrow_color(page + 1 < carousel.page_count()),
        );
        for dot in 0..carousel.page_count() {
            painter.circle_filled(
                carousel.dot_center(dot),
                CAROUSEL_DOT_SIZE / 2.0,
                if dot == page {
                    colors.carousel_dot_current
                } else {
                    colors.carousel_dot
                },
            );
        }
    }
}

fn palette_shadow(colors: &Colors) -> egui::epaint::Shadow {
    egui::epaint::Shadow {
        offset: [0, 6],
        blur: 16,
        spread: 0,
        color: colors.palette_shadow,
    }
}

fn draw_chevron(painter: &egui::Painter, center: egui::Pos2, direction: f32, color: egui::Color32) {
    let half = crate::tailwind::SPACING_2;
    let tip = center + egui::vec2(direction * half / 2.0, 0.0);
    let back = center - egui::vec2(direction * half / 2.0, 0.0);
    painter.add(egui::Shape::line(
        vec![
            back - egui::vec2(0.0, half),
            tip,
            back + egui::vec2(0.0, half),
        ],
        egui::Stroke::new(crate::tailwind::STROKE_2, color),
    ));
}
