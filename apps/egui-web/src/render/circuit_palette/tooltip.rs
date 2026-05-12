use eframe::egui;

mod amplitude;
mod diagram;
mod layout;
mod text;

use crate::app::QniApp;
use crate::colors::Colors;
use crate::constants::{PALETTE_ROW_Y, PALETTE_SIZE};
use crate::gates::PALETTE_GATES;
use crate::layout::{palette_gate_local_pos, palette_layout};

impl QniApp {
    /// Hover tooltip painted over the palette: a paper card with the
    /// gate's full name, qni-style description paragraphs, and a mini
    /// transformation diagram (input amplitudes → gate → output
    /// amplitudes). Anchored below the hovered palette button, clamped
    /// to the screen rect. No-op when nothing is hovered or while a
    /// gate drag is in progress.
    ///
    /// Chrome matches the state panel (paper bg + ui-2 1 px border +
    /// soft shadow). Typography follows the Tailwind scale: title
    /// text-sm (14 px) in tx, description text-xs (12 px) in tx-2.
    pub(crate) fn draw_palette_tooltip(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        colors: &Colors,
    ) {
        let Some(index) = self.hovered_palette_index else {
            return;
        };
        if self.dragging.is_some() {
            return;
        }
        let Some(&gate) = PALETTE_GATES.get(index) else {
            return;
        };
        let palette_layout = palette_layout();
        let Some(local) = palette_gate_local_pos(index, &palette_layout) else {
            return;
        };

        let palette_start_x = rect.width() / 2.0 - palette_layout.total_width / 2.0;
        let palette_origin = rect.min + egui::vec2(palette_start_x, PALETTE_ROW_Y);
        let gate_rect = egui::Rect::from_min_size(
            palette_origin + local.to_vec2(),
            egui::vec2(PALETTE_SIZE, PALETTE_SIZE),
        );

        let info = gate.info();
        let text = text::layout_tooltip_text(painter, &info, colors);
        let diagram_metrics = diagram::DiagramMetrics::for_transition_count(info.transitions.len());
        let card =
            layout::place_tooltip_card(rect, gate_rect, text.content_size(diagram_metrics.size()));

        layout::paint_tooltip_card(painter, card.rect, colors);
        let text_end_y = text::paint_tooltip_text(painter, card.rect, &text, colors);
        diagram::paint_tooltip_diagram(
            painter,
            card.rect,
            text_end_y,
            &info,
            gate,
            colors,
            diagram_metrics,
        );
    }
}
