//! Shared resizable-span handle component for Probability / QFT / QFT† / Amplitude gates.
//!
//! This module owns the small cyan top/bottom handles as one component:
//! body selection, hit testing, drag construction, animation easing, and paint.
//! Gate-specific code only decides whether a gate is resizable and how far the
//! span may grow.

use eframe::egui;

use crate::app::{PlacedGate, SpanResizeDrag, SpanResizeEdge, SpanResizeHandle};
use crate::colors::Colors;
use crate::gates::GateKind;
use crate::icons::draw_span_resize_handle;
use crate::layout::{amplitude_grid_rect, gate_visible_rect};

const SPAN_RESIZE_HANDLE_MIN_WIDTH: f32 = 24.0;
const SPAN_RESIZE_HANDLE_WIDTH_RATIO: f32 = 0.6;
const SPAN_RESIZE_HANDLE_HEIGHT: f32 = 6.0;
const SPAN_RESIZE_HANDLE_MAX_SCALE_PAD_RATIO: f32 = 0.125;
const SPAN_RESIZE_HANDLE_EDGES: [SpanResizeEdge; 2] = [SpanResizeEdge::Top, SpanResizeEdge::Bottom];

pub(crate) fn span_resize_ease_out_back(t: f32) -> f32 {
    const C1: f32 = 1.56;
    const C3: f32 = C1 + 1.0;
    let u = t - 1.0;
    1.0 + C3 * u * u * u + C1 * u * u
}

/// The visual body that owns the resize handles. Amplitude uses the centred
/// matrix body; every other resizable-span gate uses its whole visible rect.
pub(crate) fn span_resize_body_rect(
    kind: GateKind,
    span: usize,
    gate_rect: egui::Rect,
) -> egui::Rect {
    if kind == GateKind::AmplitudeDisplay {
        amplitude_grid_rect(gate_rect, span)
    } else {
        gate_rect
    }
}

/// Resize-handle bounding box for any resizable-span gate edge. The 6 px-tall
/// pill keeps the Probability-display 24 px minimum, then scales to 60% of the
/// supplied body width for wide Amplitude displays.
fn span_resize_handle_rect(body_rect: egui::Rect, edge: SpanResizeEdge) -> egui::Rect {
    let cx = body_rect.center().x;
    let cy = match edge {
        SpanResizeEdge::Top => body_rect.top() - 9.0,
        SpanResizeEdge::Bottom => body_rect.bottom() + 9.0,
    };
    let width = (body_rect.width() * SPAN_RESIZE_HANDLE_WIDTH_RATIO)
        .round()
        .max(SPAN_RESIZE_HANDLE_MIN_WIDTH);
    egui::Rect::from_center_size(
        egui::pos2(cx, cy),
        egui::vec2(width, SPAN_RESIZE_HANDLE_HEIGHT),
    )
}

fn span_resize_handle_hit_rect(body_rect: egui::Rect, edge: SpanResizeEdge) -> egui::Rect {
    let rect = span_resize_handle_rect(body_rect, edge);
    // The visible pill scales up to 1.25× while hovered/active; keep the
    // pointer target matched to the largest visual state. Horizontal padding
    // is `(1.25 - 1.0) / 2` of the unscaled width, with the old 24 px handle's
    // 3 px padding retained as a floor.
    let expanded = rect.expand2(egui::vec2(
        (rect.width() * SPAN_RESIZE_HANDLE_MAX_SCALE_PAD_RATIO).max(3.0),
        1.0,
    ));
    match edge {
        SpanResizeEdge::Top => {
            egui::Rect::from_min_max(expanded.min, egui::pos2(expanded.max.x, body_rect.top()))
        }
        SpanResizeEdge::Bottom => {
            egui::Rect::from_min_max(egui::pos2(expanded.min.x, body_rect.bottom()), expanded.max)
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SpanResizeHandles {
    gate_id: u32,
    body_rect: egui::Rect,
}

impl SpanResizeHandles {
    pub(crate) fn for_gate(gate: &PlacedGate) -> Option<Self> {
        Self::for_gate_at(gate, gate_visible_rect(gate, gate.pos))
    }

    pub(crate) fn for_gate_at(gate: &PlacedGate, gate_rect: egui::Rect) -> Option<Self> {
        if !gate.kind.is_resizable_span() {
            return None;
        }
        Some(Self {
            gate_id: gate.id,
            body_rect: span_resize_body_rect(gate.kind, gate.span, gate_rect),
        })
    }

    pub(crate) fn body_rect(self) -> egui::Rect {
        self.body_rect
    }

    pub(crate) fn edge_at(self, cursor: egui::Pos2) -> Option<SpanResizeEdge> {
        SPAN_RESIZE_HANDLE_EDGES
            .into_iter()
            .find(|&edge| span_resize_handle_hit_rect(self.body_rect, edge).contains(cursor))
    }

    pub(crate) fn handle(self, edge: SpanResizeEdge) -> SpanResizeHandle {
        SpanResizeHandle {
            gate_id: self.gate_id,
            edge,
        }
    }

    pub(crate) fn drag_at(self, gate: &PlacedGate, cursor: egui::Pos2) -> Option<SpanResizeDrag> {
        let edge = self.edge_at(cursor)?;
        Some(SpanResizeDrag {
            gate_id: gate.id,
            edge,
            start_pointer_y: cursor.y,
            start_wire: gate.wire,
            start_span: gate.span.max(1),
        })
    }

    pub(crate) fn paint(
        self,
        painter: &egui::Painter,
        colors: &Colors,
        hovered_handle: Option<SpanResizeHandle>,
        active_drag: Option<SpanResizeDrag>,
        visible_t: f32,
    ) {
        for edge in SPAN_RESIZE_HANDLE_EDGES {
            let handle = self.handle(edge);
            let hovered = hovered_handle == Some(handle);
            let active =
                active_drag.is_some_and(|drag| drag.gate_id == self.gate_id && drag.edge == edge);
            let bg = if hovered || active {
                colors.span_resize_handle_bg_hover
            } else {
                colors.span_resize_handle_bg
            };
            let scale = if active {
                1.25
            } else if hovered {
                1.15
            } else {
                0.7 + 0.3 * visible_t
            };
            let alpha = if hovered || active { 1.0 } else { visible_t };
            draw_span_resize_handle(
                painter,
                span_resize_handle_rect(self.body_rect, edge),
                bg,
                scale,
                alpha,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{GATE_SIZE, SLOT_SPACING};

    #[test]
    fn amplitude_body_rect_uses_matrix_draw_area() {
        let rect = egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(GATE_SIZE + SLOT_SPACING, GATE_SIZE),
        );

        let body = span_resize_body_rect(GateKind::AmplitudeDisplay, 1, rect);

        assert_eq!((body.width(), body.height()), (80.0, 40.0));
    }

    #[test]
    fn probability_body_rect_uses_whole_gate_rect() {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(GATE_SIZE, 264.0));

        assert_eq!(
            span_resize_body_rect(GateKind::ProbabilityDisplay, 5, rect),
            rect
        );
    }

    #[test]
    fn handle_width_keeps_probability_display_minimum() {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(GATE_SIZE, GATE_SIZE));

        assert_eq!(
            span_resize_handle_rect(rect, SpanResizeEdge::Top).width(),
            24.0
        );
    }

    #[test]
    fn handle_width_matches_mock_span_one_body() {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(80.0, GATE_SIZE));

        assert_eq!(
            span_resize_handle_rect(rect, SpanResizeEdge::Top).width(),
            48.0
        );
    }

    #[test]
    fn handle_width_matches_mock_span_four_body() {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(208.0, 208.0));

        assert_eq!(
            span_resize_handle_rect(rect, SpanResizeEdge::Top).width(),
            125.0
        );
    }

    #[test]
    fn handle_width_matches_mock_span_six_bottom_body() {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(320.0, 320.0));

        assert_eq!(
            span_resize_handle_rect(rect, SpanResizeEdge::Bottom).width(),
            192.0
        );
    }
}
