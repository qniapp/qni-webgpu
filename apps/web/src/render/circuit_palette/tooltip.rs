use eframe::egui;

mod amplitude_display;
mod diagram;
mod layout;
mod text;

use crate::app::QniApp;
use crate::colors::Colors;
use crate::constants::{PALETTE_ROW_Y, PALETTE_SIZE};
use crate::gates::GateInfo;
use crate::layout::{palette_start_x, PaletteItem};

const CIRCUIT_BLOCK_INFO: GateInfo = GateInfo {
    name: "Comment Block",
    paragraphs: &[
        "Wraps circuit steps and labels them with a comment.",
        "Does not change the state.",
    ],
    transitions: &[],
};

impl QniApp {
    /// Hover tooltip painted over the palette: a card with the
    /// gate's full name, qni-style description paragraphs, and a mini
    /// transformation diagram (input amplitudes → gate → output
    /// amplitudes). Anchored below the hovered palette button, clamped
    /// to the screen rect. No-op when nothing is hovered or while a
    /// gate drag is in progress.
    pub(crate) fn draw_palette_tooltip(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        colors: &Colors,
    ) {
        if self.library.active_locked() {
            return;
        }
        let Some(index) = self.hovered_palette_index else {
            return;
        };
        if self.dragging.is_some() {
            return;
        }
        let Some(item) = self.palette.item(index) else {
            return;
        };
        let palette_layout = self.palette.layout(rect.width());
        let Some(local) = self.palette.local_pos(index, &palette_layout) else {
            return;
        };

        let palette_start_x = palette_start_x(rect.width(), &palette_layout);
        let palette_origin = rect.min + egui::vec2(palette_start_x, PALETTE_ROW_Y);
        let gate_rect = egui::Rect::from_min_size(
            palette_origin + local.to_vec2(),
            egui::vec2(PALETTE_SIZE, PALETTE_SIZE),
        );

        let (info, gate) = match item {
            PaletteItem::Gate(entry) => (entry.kind.info(), Some(entry.kind)),
            PaletteItem::CircuitBlock => (CIRCUIT_BLOCK_INFO, None),
        };
        let text = text::layout_tooltip_text(painter, &info, colors);
        let diagram_metrics = diagram::DiagramMetrics::for_transition_count(info.transitions.len());
        let card =
            layout::place_tooltip_card(rect, gate_rect, text.content_size(diagram_metrics.size()));

        layout::paint_tooltip_card(painter, card, colors);
        let rect = card.placement.rect;
        let text_end_y = text::paint_tooltip_text(painter, rect, &text, colors);
        if let Some(gate) = gate {
            diagram::paint_tooltip_diagram(
                painter,
                rect,
                text_end_y,
                &info,
                gate,
                colors,
                diagram_metrics,
            );
        }
    }
}
