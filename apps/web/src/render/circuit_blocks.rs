//! Circuit-block drawing (qni `circuit-block`): a tinted band behind the
//! block's columns, ruled top and bottom, with the label centred above and
//! below it, as in qni's desktop (`min-width: 768px`) layout.

use eframe::egui;

use crate::app::{CircuitBlock, QniApp};
use crate::colors::Colors;
use crate::constants::{
    CIRCUIT_BLOCK_BORDER_WIDTH, CIRCUIT_BLOCK_LABEL_FONT_SIZE, CIRCUIT_BLOCK_LABEL_LINE_HEIGHT,
    CIRCUIT_BLOCK_MARGIN_Y, CIRCUIT_BLOCK_PADDING_Y, LINE_GAP, SLOT_SPACING,
};
use crate::layout::LayoutMetrics;

/// Circuit-local rect of a block body, borders included. Spans the block's
/// columns edge to edge horizontally and, vertically, the step-preview bars
/// plus padding and rules.
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
    let pad_y = LINE_GAP * 0.5 + CIRCUIT_BLOCK_PADDING_Y + CIRCUIT_BLOCK_BORDER_WIDTH;
    Some(egui::Rect::from_min_max(
        egui::pos2(first_center - SLOT_SPACING * 0.5, first_line - pad_y),
        egui::pos2(last_center + SLOT_SPACING * 0.5, last_line + pad_y),
    ))
}

/// Centres of the upper and lower labels of a block `body`. As in qni, each
/// label's line box starts at the outer edge of the margin beyond its rule
/// and the text is centred in it.
fn circuit_block_label_centers(body: egui::Rect) -> [egui::Pos2; 2] {
    let offset = CIRCUIT_BLOCK_MARGIN_Y - CIRCUIT_BLOCK_LABEL_LINE_HEIGHT * 0.5;
    [
        egui::pos2(body.center().x, body.top() - offset),
        egui::pos2(body.center().x, body.bottom() + offset),
    ]
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
            for center in circuit_block_label_centers(body) {
                painter.text(
                    center,
                    egui::Align2::CENTER_CENTER,
                    block.label(),
                    font.clone(),
                    colors.circuit_block_label,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::LINE_Y;
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
        // Half a wire gap to the step-preview bar end, qni's 32 px padding,
        // then the 2 px rule.
        let pad_y = 28.0 + 32.0 + 2.0;

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

    fn body() -> egui::Rect {
        egui::Rect::from_min_max(egui::pos2(100.0, 200.0), egui::pos2(300.0, 400.0))
    }

    #[test]
    fn upper_label_centres_its_line_box_in_the_margin_above() {
        // qni: a 28 px line box at the top of the 24 px margin.
        assert_eq!(
            circuit_block_label_centers(body())[0],
            egui::pos2(200.0, 200.0 - 24.0 + 14.0)
        );
    }

    #[test]
    fn lower_label_centres_its_line_box_in_the_margin_below() {
        assert_eq!(
            circuit_block_label_centers(body())[1],
            egui::pos2(200.0, 400.0 + 24.0 - 14.0)
        );
    }

    fn app(json: &str) -> QniApp {
        let ctx = egui::Context::default();
        let startup = crate::app::EmbedStartup::parse(json, true, None::<&[&str]>, None).unwrap();
        QniApp::new_with_startup(&eframe::CreationContext::_new_kittest(ctx), Some(startup))
    }

    const BELL: &str = r#"{"cols":[["|0>","|0>"],["H"],["•","X"]]}"#;
    const BELL_BLOCK: &str = r#"{"cols":[["|0>","|0>"],["{a"],["H"],["•","X"],["}"]]}"#;

    fn screen() -> egui::Rect {
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(960.0, 560.0))
    }

    #[test]
    fn blocks_push_the_circuit_down_by_their_outset() {
        let shift = app(BELL_BLOCK).circuit_origin(screen(), 0.0).y
            - app(BELL).circuit_origin(screen(), 0.0).y;

        assert_eq!(shift, 32.0 + 2.0 + 24.0);
    }

    #[test]
    fn blocks_extend_the_scrollable_circuit_height_by_their_outset() {
        // A short screen so the content height is not clamped up to it.
        let short = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(960.0, 100.0));
        let growth = app(BELL_BLOCK).circuit_content_height(2, short)
            - app(BELL).circuit_content_height(2, short);

        assert_eq!(growth, 32.0 + 2.0 + 24.0);
    }

    #[test]
    fn circuit_bottom_includes_the_lower_label_band() {
        let app = app(BELL_BLOCK);
        let last_line_y = app.circuit_origin(screen(), 0.0).y + LINE_Y + LINE_GAP;

        assert_eq!(
            app.circuit_bottom_y(screen()),
            last_line_y + 28.0 + 32.0 + 2.0 + 24.0
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
