//! Circuit-block drawing (qni `circuit-block`): a tinted band behind the
//! block's columns, ruled top and bottom, with the label centred above and
//! below it, as in qni's desktop layout.

use eframe::egui;

use crate::app::{CircuitBlock, QniApp};
use crate::colors::Colors;
use crate::constants::{
    CIRCUIT_BLOCK_BORDER_WIDTH, CIRCUIT_BLOCK_LABEL_FONT_SIZE, CIRCUIT_BLOCK_LABEL_GAP,
    CIRCUIT_BLOCK_PADDING_Y, LINE_GAP, SLOT_SPACING,
};
use crate::layout::LayoutMetrics;

/// Circuit-local rect of a block body, borders included. Spans the block's
/// columns edge to edge horizontally and every wire plus padding vertically.
/// `None` when the layout has no wires or does not reach the block yet.
pub(super) fn circuit_block_rect(
    block: &CircuitBlock,
    metrics: &LayoutMetrics,
) -> Option<egui::Rect> {
    let first_center = metrics.slot_centers.get(block.start().as_usize())?;
    let last_center = metrics
        .slot_centers
        .get(block.end().as_usize().checked_sub(1)?)?;
    let first_line = metrics.line_ys.first()?;
    let last_line = metrics.line_ys.last()?;
    let pad_y = LINE_GAP * 0.5 + CIRCUIT_BLOCK_PADDING_Y;
    Some(egui::Rect::from_min_max(
        egui::pos2(first_center - SLOT_SPACING * 0.5, first_line - pad_y),
        egui::pos2(last_center + SLOT_SPACING * 0.5, last_line + pad_y),
    ))
}

impl QniApp {
    pub(super) fn draw_circuit_blocks(
        &self,
        painter: &egui::Painter,
        metrics: &LayoutMetrics,
        colors: &Colors,
        circuit_origin: egui::Pos2,
    ) {
        let font = egui::FontId::monospace(CIRCUIT_BLOCK_LABEL_FONT_SIZE);
        for block in self.circuit_blocks.iter() {
            let Some(local) = circuit_block_rect(block, metrics) else {
                continue;
            };
            let body = local.translate(circuit_origin.to_vec2());
            painter.rect_filled(body, 0.0, colors.circuit_block_fill);
            let rule = egui::vec2(body.width(), CIRCUIT_BLOCK_BORDER_WIDTH);
            painter.rect_filled(
                egui::Rect::from_min_size(body.left_top(), rule),
                0.0,
                colors.circuit_block_border,
            );
            painter.rect_filled(
                egui::Rect::from_min_size(body.left_bottom() - egui::vec2(0.0, rule.y), rule),
                0.0,
                colors.circuit_block_border,
            );
            painter.text(
                egui::pos2(body.center().x, body.top() - CIRCUIT_BLOCK_LABEL_GAP),
                egui::Align2::CENTER_BOTTOM,
                block.label(),
                font.clone(),
                colors.circuit_block_label,
            );
            painter.text(
                egui::pos2(body.center().x, body.bottom() + CIRCUIT_BLOCK_LABEL_GAP),
                egui::Align2::CENTER_TOP,
                block.label(),
                font.clone(),
                colors.circuit_block_label,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::layout_metrics;

    fn first_block(json: &str) -> CircuitBlock {
        crate::url_circuit::parse_circuit_json(json)
            .blocks
            .iter()
            .next()
            .cloned()
            .expect("test JSON has a block")
    }

    #[test]
    fn block_rect_spans_its_columns_and_all_wires() {
        let metrics = layout_metrics(1200.0, 2, 4);
        let block = first_block(r#"{"cols":[["H"],["{a"],["X"],["Z"],["}"]]}"#);
        let pad_y = LINE_GAP * 0.5 + CIRCUIT_BLOCK_PADDING_Y;

        assert_eq!(
            circuit_block_rect(&block, &metrics),
            Some(egui::Rect::from_min_max(
                egui::pos2(
                    metrics.slot_centers[1] - SLOT_SPACING * 0.5,
                    metrics.line_ys[0] - pad_y
                ),
                egui::pos2(
                    metrics.slot_centers[2] + SLOT_SPACING * 0.5,
                    metrics.line_ys[1] + pad_y
                ),
            ))
        );
    }

    #[test]
    fn block_beyond_the_laid_out_slots_has_no_rect() {
        // Too narrow for any slot beyond the one `min_slots` forces.
        let metrics = layout_metrics(120.0, 2, 1);
        let block = first_block(r#"{"cols":[["H"],["{a"],["X"],["}"]]}"#);

        assert_eq!(circuit_block_rect(&block, &metrics), None);
    }
}
