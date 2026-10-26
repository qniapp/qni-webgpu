use eframe::egui;

use crate::app::{
    CircuitBlock, CircuitBlockDrag, CircuitBlockEdge, CircuitBlockHover, CircuitBlockLabelSlot,
    CircuitColumnIndex, QniApp,
};
use crate::colors::Colors;
use crate::constants::{
    CIRCUIT_BLOCK_BORDER_WIDTH, CIRCUIT_BLOCK_LABEL_FONT_SIZE, CIRCUIT_BLOCK_LABEL_LINE_HEIGHT,
    CIRCUIT_BLOCK_MARGIN_Y, CIRCUIT_BLOCK_PADDING_Y, LINE_GAP, SLOT_SPACING,
};
pub(crate) const CIRCUIT_BLOCK_LABEL_GAP_X: f32 = crate::tailwind::SPACING_2;

use crate::icons::{draw_circuit_block_palette_icon, draw_span_resize_handle};
use crate::layout::{CircuitProjection, LayoutMetrics};
use crate::span_resize::{
    span_resize_ease_out_back, SPAN_RESIZE_HANDLE_HEIGHT, SPAN_RESIZE_HANDLE_MIN_WIDTH,
};

const CIRCUIT_BLOCK_EDGES: [CircuitBlockEdge; 2] =
    [CircuitBlockEdge::Left, CircuitBlockEdge::Right];
pub(crate) const CIRCUIT_BLOCK_LABEL_SLOTS: [CircuitBlockLabelSlot; 2] =
    [CircuitBlockLabelSlot::Top, CircuitBlockLabelSlot::Bottom];
const CIRCUIT_BLOCK_HANDLE_HIT_PAD: egui::Vec2 = egui::vec2(5.0, 4.0);

pub(crate) fn circuit_block_label_font() -> egui::FontId {
    egui::FontId::monospace(CIRCUIT_BLOCK_LABEL_FONT_SIZE)
}

/// Circuit-local rect of a block body, borders included. Spans the block's
/// columns edge to edge horizontally and, vertically, the step-preview bars
/// plus padding and rules.
/// `None` when the layout has no wires or does not reach the block yet.
pub(crate) fn circuit_block_rect(
    block: &CircuitBlock,
    metrics: &LayoutMetrics,
) -> Option<egui::Rect> {
    circuit_block_columns_rect(block.start(), block.end(), metrics)
}

pub(crate) fn circuit_block_columns_rect(
    start: CircuitColumnIndex,
    end: CircuitColumnIndex,
    metrics: &LayoutMetrics,
) -> Option<egui::Rect> {
    let first_center = metrics.slot_centers.get(start.as_usize())?;
    let last_center = metrics.slot_centers.get(end.as_usize().checked_sub(1)?)?;
    let first_line = metrics.line_ys.first()?;
    let last_line = metrics.line_ys.last()?;
    let pad_y = LINE_GAP * 0.5 + CIRCUIT_BLOCK_PADDING_Y + CIRCUIT_BLOCK_BORDER_WIDTH;
    Some(egui::Rect::from_min_max(
        egui::pos2(first_center - SLOT_SPACING * 0.5, first_line - pad_y),
        egui::pos2(last_center + SLOT_SPACING * 0.5, last_line + pad_y),
    ))
}

fn circuit_block_label_centers(body: egui::Rect) -> [egui::Pos2; 2] {
    let offset = CIRCUIT_BLOCK_MARGIN_Y - CIRCUIT_BLOCK_LABEL_LINE_HEIGHT * 0.5;
    [
        egui::pos2(body.center().x, body.top() - offset),
        egui::pos2(body.center().x, body.bottom() + offset),
    ]
}

pub(crate) fn circuit_block_label_center(
    body: egui::Rect,
    slot: CircuitBlockLabelSlot,
) -> egui::Pos2 {
    let [top, bottom] = circuit_block_label_centers(body);
    match slot {
        CircuitBlockLabelSlot::Top => top,
        CircuitBlockLabelSlot::Bottom => bottom,
    }
}

/// Clickable line box of a label: the block width, or the text width when
/// the label overflows the block.
pub(crate) fn circuit_block_label_rect(
    body: egui::Rect,
    slot: CircuitBlockLabelSlot,
    text_width: f32,
) -> egui::Rect {
    egui::Rect::from_center_size(
        circuit_block_label_center(body, slot),
        egui::vec2(
            body.width().max(text_width),
            CIRCUIT_BLOCK_LABEL_LINE_HEIGHT,
        ),
    )
}

/// Screen-space hit box for the single, upright label left of a portrait block.
/// Portrait labels use the `Top` editor slot.
pub(crate) fn circuit_block_vertical_label_rect(body: egui::Rect, text_width: f32) -> egui::Rect {
    egui::Rect::from_min_size(
        egui::pos2(
            body.left() - CIRCUIT_BLOCK_LABEL_GAP_X - text_width,
            body.top(),
        ),
        egui::vec2(text_width, CIRCUIT_BLOCK_LABEL_LINE_HEIGHT),
    )
}

pub(crate) fn circuit_block_handle_rect(body: egui::Rect, edge: CircuitBlockEdge) -> egui::Rect {
    let x = match edge {
        CircuitBlockEdge::Left => body.left(),
        CircuitBlockEdge::Right => body.right(),
    };
    egui::Rect::from_center_size(
        egui::pos2(x, body.center().y),
        egui::vec2(SPAN_RESIZE_HANDLE_HEIGHT, SPAN_RESIZE_HANDLE_MIN_WIDTH),
    )
}

pub(crate) fn circuit_block_handle_hit_rect(
    body: egui::Rect,
    edge: CircuitBlockEdge,
) -> egui::Rect {
    circuit_block_handle_rect(body, edge).expand2(CIRCUIT_BLOCK_HANDLE_HIT_PAD)
}

pub(crate) fn circuit_block_boundary_x(metrics: &LayoutMetrics, boundary: usize) -> f32 {
    metrics.slot_left + (boundary as f32 - 0.5) * SLOT_SPACING
}

pub(crate) fn circuit_block_label_width(ctx: &egui::Context, label: &str) -> f32 {
    ctx.fonts_mut(|fonts| {
        fonts
            .layout_no_wrap(
                label.to_owned(),
                circuit_block_label_font(),
                egui::Color32::PLACEHOLDER,
            )
            .size()
            .x
    })
}

impl QniApp {
    /// The block part under a circuit-local `cursor`. Handles win over
    /// labels and bodies of every block, since they overhang the block edges;
    /// where two touching blocks share an edge, the block under the cursor
    /// owns it.
    pub(crate) fn circuit_block_hover_at(
        &self,
        cursor: egui::Pos2,
        metrics: &LayoutMetrics,
        ctx: &egui::Context,
    ) -> Option<CircuitBlockHover> {
        let bodies: Vec<(usize, &CircuitBlock, egui::Rect)> = self
            .circuit_blocks
            .iter()
            .enumerate()
            .filter_map(|(index, block)| Some((index, block, circuit_block_rect(block, metrics)?)))
            .collect();
        let handles: Vec<(egui::Rect, CircuitBlockHover)> = bodies
            .iter()
            .flat_map(|&(index, _, body)| {
                CIRCUIT_BLOCK_EDGES
                    .into_iter()
                    .filter(move |&edge| circuit_block_handle_hit_rect(body, edge).contains(cursor))
                    .map(move |edge| {
                        let hover = CircuitBlockHover {
                            index,
                            handle: Some(edge),
                            label: None,
                        };
                        (body, hover)
                    })
            })
            .collect();
        let handle = handles
            .iter()
            .find(|(body, _)| body.contains(cursor))
            .or(handles.first())
            .map(|&(_, hover)| hover);
        handle.or_else(|| {
            bodies.iter().find_map(|&(index, block, body)| {
                let width = circuit_block_label_width(ctx, block.label());
                let label = if self.orientation.is_vertical() {
                    let projection = CircuitProjection::new(self.orientation, egui::Pos2::ZERO);
                    circuit_block_vertical_label_rect(projection.to_screen_rect(body), width)
                        .contains(projection.to_screen(cursor))
                        .then_some(CircuitBlockLabelSlot::Top)
                } else {
                    CIRCUIT_BLOCK_LABEL_SLOTS
                        .into_iter()
                        .find(|&slot| circuit_block_label_rect(body, slot, width).contains(cursor))
                };
                (label.is_some() || body.contains(cursor)).then_some(CircuitBlockHover {
                    index,
                    handle: None,
                    label,
                })
            })
        })
    }

    fn circuit_block_hover_shown(&self) -> Option<CircuitBlockHover> {
        self.hovered_circuit_block.filter(|_| {
            self.circuit_block_drag.is_none()
                && self.circuit_block_editor.is_none()
                && self.dragging.is_none()
                && self.span_resize_drag.is_none()
        })
    }

    fn circuit_block_hidden(&self, index: usize) -> bool {
        matches!(
            self.circuit_block_drag,
            Some(CircuitBlockDrag::Label {
                index: dragged,
                removing: true,
                ..
            }) if dragged == index
        )
    }

    pub(super) fn draw_circuit_blocks(
        &self,
        painter: &egui::Painter,
        metrics: &LayoutMetrics,
        colors: &Colors,
        projection: CircuitProjection,
    ) {
        let font = circuit_block_label_font();
        if let Some(CircuitBlockDrag::Palette {
            target: Some((start, end)),
            ..
        }) = self.circuit_block_drag
        {
            if let Some(local) = circuit_block_columns_rect(start, end, metrics) {
                painter.rect_filled(
                    projection.to_screen_rect(local),
                    0.0,
                    colors.circuit_block_fill,
                );
            }
        }
        for (index, block) in self.circuit_blocks.iter().enumerate() {
            if self.circuit_block_hidden(index) {
                continue;
            }
            let Some(local) = circuit_block_rect(block, metrics) else {
                continue;
            };
            let body = projection.to_screen_rect(local);
            painter.rect_filled(body, 0.0, colors.circuit_block_fill);
            if projection.is_vertical() {
                let rule = egui::vec2(CIRCUIT_BLOCK_BORDER_WIDTH, body.height());
                for min in [body.left_top(), body.right_top() - egui::vec2(rule.x, 0.0)] {
                    painter.rect_filled(
                        egui::Rect::from_min_size(min, rule),
                        0.0,
                        colors.circuit_block_border,
                    );
                }
                if self
                    .circuit_block_editor
                    .as_ref()
                    .is_none_or(|editor| editor.index != index)
                {
                    let text_rect = painter.text(
                        egui::pos2(body.left() - CIRCUIT_BLOCK_LABEL_GAP_X, body.top()),
                        egui::Align2::RIGHT_TOP,
                        block.label(),
                        font.clone(),
                        colors.circuit_block_label,
                    );
                    if self.circuit_block_hover_shown().is_some_and(|hover| {
                        hover.index == index && hover.label == Some(CircuitBlockLabelSlot::Top)
                    }) {
                        paint_circuit_block_underline(painter, text_rect, colors);
                    }
                }
                continue;
            }
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
            let editor = self
                .circuit_block_editor
                .as_ref()
                .filter(|editor| editor.index == index);
            let underlined = self
                .circuit_block_hover_shown()
                .filter(|hover| hover.index == index)
                .and_then(|hover| hover.label);
            for slot in CIRCUIT_BLOCK_LABEL_SLOTS {
                let (text, color) = match editor {
                    Some(editor) if editor.slot == slot => continue,
                    Some(editor) if editor.draft.is_empty() => {
                        (CIRCUIT_BLOCK_PLACEHOLDER, colors.circuit_block_placeholder)
                    }
                    Some(editor) => (editor.draft.as_str(), colors.circuit_block_label),
                    None => (block.label(), colors.circuit_block_label),
                };
                let text_rect = painter.text(
                    circuit_block_label_center(body, slot),
                    egui::Align2::CENTER_CENTER,
                    text,
                    font.clone(),
                    color,
                );
                if underlined == Some(slot) {
                    paint_circuit_block_underline(painter, text_rect, colors);
                }
            }
        }
    }

    pub(super) fn draw_circuit_block_handles(
        &self,
        painter: &egui::Painter,
        metrics: &LayoutMetrics,
        colors: &Colors,
        projection: CircuitProjection,
    ) {
        let resizing = match self.circuit_block_drag {
            Some(CircuitBlockDrag::Resize { index, edge }) => Some((index, edge)),
            _ => None,
        };
        let hovered = self.circuit_block_hover_shown();
        for (index, block) in self.circuit_blocks.iter().enumerate() {
            let visible = resizing.is_some_and(|(resized, _)| resized == index)
                || hovered.is_some_and(|hover| hover.index == index);
            let visible_t = painter.ctx().animate_bool_with_time_and_easing(
                egui::Id::new(("circuit_block_handles", index)),
                visible,
                0.25,
                span_resize_ease_out_back,
            );
            if !visible && visible_t <= 0.01 {
                continue;
            }
            let Some(local) = circuit_block_rect(block, metrics) else {
                continue;
            };
            for edge in CIRCUIT_BLOCK_EDGES {
                let active = resizing == Some((index, edge));
                let hot =
                    hovered.is_some_and(|hover| hover.index == index && hover.handle == Some(edge));
                let (bg, scale, alpha) = if active {
                    (colors.span_resize_handle_bg_hover, 1.25, 1.0)
                } else if hot {
                    (colors.span_resize_handle_bg_hover, 1.15, 1.0)
                } else {
                    (
                        colors.span_resize_handle_bg,
                        0.7 + 0.3 * visible_t,
                        visible_t,
                    )
                };
                draw_span_resize_handle(
                    painter,
                    projection.to_screen_rect(circuit_block_handle_rect(local, edge)),
                    bg,
                    scale,
                    alpha,
                );
            }
        }
    }

    pub(crate) fn draw_circuit_block_drag_overlay(
        &self,
        painter: &egui::Painter,
        projection: CircuitProjection,
        colors: &Colors,
    ) {
        match self.circuit_block_drag {
            Some(CircuitBlockDrag::Palette {
                cursor,
                target: None,
            }) => {
                draw_circuit_block_palette_icon(
                    painter,
                    egui::Rect::from_center_size(
                        projection.to_screen(cursor),
                        egui::vec2(
                            crate::constants::PALETTE_SIZE,
                            crate::constants::PALETTE_SIZE,
                        ),
                    ),
                    colors,
                );
            }
            Some(CircuitBlockDrag::Label {
                index,
                cursor,
                removing: true,
                ..
            }) => {
                if let Some(block) = self.circuit_blocks.get(index) {
                    painter.text(
                        projection.to_screen(cursor),
                        egui::Align2::CENTER_CENTER,
                        block.label(),
                        circuit_block_label_font(),
                        colors.circuit_block_label,
                    );
                }
            }
            _ => {}
        }
    }
}

pub(crate) const CIRCUIT_BLOCK_PLACEHOLDER: &str = "コメントを入力";

pub(crate) fn paint_circuit_block_underline(
    painter: &egui::Painter,
    text_rect: egui::Rect,
    colors: &Colors,
) {
    let y = text_rect.center().y + CIRCUIT_BLOCK_LABEL_FONT_SIZE * 0.5;
    painter.line_segment(
        [
            egui::pos2(text_rect.left(), y),
            egui::pos2(text_rect.right(), y),
        ],
        egui::Stroke::new(1.0_f32, colors.text),
    );
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
        let shift = app(BELL_BLOCK)
            .circuit_projection(screen(), 0.0)
            .to_screen(egui::Pos2::ZERO)
            .y
            - app(BELL)
                .circuit_projection(screen(), 0.0)
                .to_screen(egui::Pos2::ZERO)
                .y;

        assert_eq!(shift, 32.0 + 2.0 + 24.0);
    }

    #[test]
    fn blocks_extend_the_scrollable_circuit_height_by_their_outset() {
        let short = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(960.0, 100.0));
        let growth = app(BELL_BLOCK).circuit_content_height(2, short)
            - app(BELL).circuit_content_height(2, short);

        assert_eq!(growth, 2.0 * (32.0 + 2.0 + 24.0));
    }

    #[test]
    fn circuit_bottom_includes_the_lower_label_band() {
        let app = app(BELL_BLOCK);
        let last_line_y = app
            .circuit_projection(screen(), 0.0)
            .to_screen(egui::Pos2::ZERO)
            .y
            + LINE_Y
            + LINE_GAP;

        assert_eq!(
            app.circuit_bottom_y(screen()),
            last_line_y + 28.0 + 32.0 + 2.0 + 24.0
        );
    }

    #[test]
    fn left_handle_sits_on_the_vertical_middle_of_the_left_edge() {
        assert_eq!(
            circuit_block_handle_rect(body(), CircuitBlockEdge::Left).center(),
            egui::pos2(100.0, 300.0)
        );
    }

    #[test]
    fn right_handle_sits_on_the_vertical_middle_of_the_right_edge() {
        assert_eq!(
            circuit_block_handle_rect(body(), CircuitBlockEdge::Right).center(),
            egui::pos2(300.0, 300.0)
        );
    }

    #[test]
    fn handle_is_the_span_resize_pill_turned_upright() {
        assert_eq!(
            circuit_block_handle_rect(body(), CircuitBlockEdge::Left).size(),
            egui::vec2(6.0, 24.0)
        );
    }

    #[test]
    fn label_rect_spans_the_block_width() {
        assert_eq!(
            circuit_block_label_rect(body(), CircuitBlockLabelSlot::Top, 20.0),
            egui::Rect::from_center_size(egui::pos2(200.0, 190.0), egui::vec2(200.0, 28.0))
        );
    }

    #[test]
    fn label_rect_grows_with_an_overflowing_label() {
        assert_eq!(
            circuit_block_label_rect(body(), CircuitBlockLabelSlot::Bottom, 260.0).width(),
            260.0
        );
    }

    #[test]
    fn portrait_hover_targets_the_visible_left_label() {
        let mut app = app(BELL_BLOCK);
        app.orientation = crate::layout::CircuitOrientation::Vertical;
        let projection = CircuitProjection::new(app.orientation, egui::Pos2::ZERO);
        let metrics = layout_metrics(960.0, 2, 4);
        let body = circuit_block_rect(app.circuit_blocks.get(0).unwrap(), &metrics).unwrap();
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |_| {
            let width = circuit_block_label_width(&ctx, "a");
            let label = circuit_block_vertical_label_rect(projection.to_screen_rect(body), width);
            let hover =
                app.circuit_block_hover_at(projection.to_local(label.center()), &metrics, &ctx);
            assert_eq!(
                hover.and_then(|hover| hover.label),
                Some(CircuitBlockLabelSlot::Top)
            );
        });
    }

    #[test]
    fn portrait_hover_ignores_the_hidden_desktop_label() {
        let mut app = app(BELL_BLOCK);
        app.orientation = crate::layout::CircuitOrientation::Vertical;
        let metrics = layout_metrics(960.0, 2, 4);
        let body = circuit_block_rect(app.circuit_blocks.get(0).unwrap(), &metrics).unwrap();
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |_| {
            assert!(app
                .circuit_block_hover_at(
                    circuit_block_label_center(body, CircuitBlockLabelSlot::Top),
                    &metrics,
                    &ctx,
                )
                .is_none());
        });
    }

    #[test]
    fn block_boundary_sits_half_a_slot_before_its_column() {
        let metrics = layout_metrics(1200.0, 2, 4);

        assert_eq!(
            circuit_block_boundary_x(&metrics, 2),
            metrics.slot_centers[2] - SLOT_SPACING * 0.5
        );
    }

    fn hover_shapes(hover: CircuitBlockHover) -> Vec<egui::epaint::ClippedShape> {
        let mut app = app(BELL_BLOCK);
        app.hovered_circuit_block = Some(hover);
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let metrics = layout_metrics(960.0, 2, 4);
            let colors = app.colors();
            app.draw_circuit_blocks(
                ui.painter(),
                &metrics,
                &colors,
                CircuitProjection::horizontal(egui::Pos2::ZERO),
            );
            app.draw_circuit_block_handles(
                ui.painter(),
                &metrics,
                &colors,
                CircuitProjection::horizontal(egui::Pos2::ZERO),
            );
        });
        output.shapes
    }

    #[test]
    fn hovered_block_shows_resize_handles_snapshot() {
        insta::assert_debug_snapshot!(hover_shapes(CircuitBlockHover {
            index: 0,
            handle: Some(CircuitBlockEdge::Right),
            label: None,
        }));
    }

    #[test]
    fn hovered_label_is_underlined_snapshot() {
        insta::assert_debug_snapshot!(hover_shapes(CircuitBlockHover {
            index: 0,
            handle: None,
            label: Some(CircuitBlockLabelSlot::Top),
        }));
    }

    #[test]
    fn block_beyond_the_laid_out_slots_has_no_rect() {
        let metrics = layout_metrics(120.0, 2, 1);
        let block = first_block(r#"{"cols":[["H"],["{a"],["X"],["}"]]}"#);

        assert_eq!(circuit_block_rect(&block, &metrics), None);
    }
}
