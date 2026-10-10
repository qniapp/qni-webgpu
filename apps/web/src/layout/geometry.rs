use eframe::egui;

use crate::app::{CircuitColumnIndex, PlacedGate, WireIndex};
use crate::constants::{
    CIRCUIT_PADDING, COMPACT_CIRCUIT_MAX_WIDTH, COMPACT_CIRCUIT_PADDING, COMPACT_LINE_LEFT_OFFSET,
    COMPACT_QUBIT_LABEL_GAP, GATE_SIZE, LINE_GAP, LINE_LEFT_OFFSET, LINE_RIGHT_OFFSET, LINE_Y,
    SLOT_SPACING,
};
use crate::gates::GateKind;
use crate::grid_cell::GridCell;

pub(crate) fn amplitude_display_width_cols(span: usize) -> usize {
    let span = span.clamp(1, 16);
    if span == 1 {
        2
    } else if span.is_multiple_of(2) {
        span
    } else {
        span.div_ceil(2)
    }
}

pub(crate) fn amplitude_grid_dims(span: usize) -> (usize, usize) {
    let span = span.clamp(1, 16);
    let outcomes = 1usize << span;
    let width = if outcomes == 2 {
        2
    } else {
        1usize << (span / 2)
    };
    (width, outcomes / width)
}

pub(crate) fn density_matrix_width_cols(span: usize) -> usize {
    span.clamp(1, 8)
}

pub(crate) fn gate_width_cols(kind: GateKind, span: usize) -> usize {
    match kind {
        GateKind::AmplitudeDisplay => amplitude_display_width_cols(span),
        GateKind::DensityMatrixDisplay => density_matrix_width_cols(span),
        _ => 1,
    }
}

pub(crate) fn gate_size(kind: GateKind, span: usize) -> egui::Vec2 {
    let width_cols = gate_width_cols(kind, span).max(1);
    let width = (width_cols - 1) as f32 * SLOT_SPACING + GATE_SIZE;
    let height = if kind.is_resizable_span() {
        let span = span.max(1);
        (span - 1) as f32 * LINE_GAP + GATE_SIZE
    } else {
        GATE_SIZE
    };
    egui::vec2(width, height)
}

pub(crate) fn gate_rect_at_grid(
    kind: GateKind,
    column: CircuitColumnIndex,
    wire: WireIndex,
    span: usize,
) -> egui::Rect {
    egui::Rect::from_min_size(PlacedGate::grid_pos(column, wire), gate_size(kind, span))
}

/// Visible rect of a placed gate, accounting for the multi-qubit `span`
/// of resizable-span gates and the variable-width Amplitude display body.
/// `origin` is the top-left of the gate body (= rect.min + gate.pos
/// in the circuit's local coordinate space).
pub(crate) fn gate_visible_rect(gate: &PlacedGate, origin: egui::Pos2) -> egui::Rect {
    egui::Rect::from_min_size(origin, gate_size(gate.kind, gate.span.get()))
}

pub(crate) fn amplitude_grid_rect(gate_rect: egui::Rect, span: usize) -> egui::Rect {
    let (cols, rows) = amplitude_grid_dims(span);
    let cell = (gate_rect.width() / cols as f32).min(gate_rect.height() / rows as f32);
    let size = egui::vec2(cell * cols as f32, cell * rows as f32);
    egui::Rect::from_center_size(gate_rect.center(), size)
}

pub(crate) fn amplitude_cell_index_at(
    gate_rect: egui::Rect,
    span: usize,
    cursor: egui::Pos2,
) -> Option<u32> {
    let grid_rect = amplitude_grid_rect(gate_rect, span);
    if !grid_rect.contains(cursor) {
        return None;
    }
    let (cols, rows) = amplitude_grid_dims(span);
    let cell = grid_rect.width() / cols as f32;
    let col = ((cursor.x - grid_rect.left()) / cell)
        .floor()
        .clamp(0.0, (cols - 1) as f32) as usize;
    let row = ((cursor.y - grid_rect.top()) / cell)
        .floor()
        .clamp(0.0, (rows - 1) as f32) as usize;
    Some(GridCell::new(col, row).to_index(cols) as u32)
}

fn density_matrix_grid_rect(gate_rect: egui::Rect, span: usize) -> egui::Rect {
    let dim = 1usize << span.clamp(1, 8);
    let cell = gate_rect.width().min(gate_rect.height()) / dim as f32;
    let size = egui::vec2(cell * dim as f32, cell * dim as f32);
    egui::Rect::from_min_size(gate_rect.min + (gate_rect.size() - size) * 0.5, size)
}

pub(crate) fn density_matrix_cell_index_at(
    gate_rect: egui::Rect,
    span: usize,
    cursor: egui::Pos2,
) -> Option<u32> {
    let grid_rect = density_matrix_grid_rect(gate_rect, span);
    if !grid_rect.contains(cursor) {
        return None;
    }
    let dim = 1usize << span.clamp(1, 8);
    let cell = grid_rect.width() / dim as f32;
    let col = ((cursor.x - grid_rect.left()) / cell)
        .floor()
        .clamp(0.0, (dim - 1) as f32) as usize;
    let row = ((cursor.y - grid_rect.top()) / cell)
        .floor()
        .clamp(0.0, (dim - 1) as f32) as usize;
    Some(GridCell::new(col, row).to_index(dim) as u32)
}

/// Horizontal gutters of the circuit on a canvas. Circuit-space geometry
/// (`LINE_LEFT_OFFSET`, slot centres, gate positions) never changes; a
/// narrow canvas instead slides the circuit left by `shift_x` and draws the
/// qubit labels right-aligned against the wires, so the space left of the
/// first gate shrinks from 142px to 60px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CircuitGutters {
    /// Screen distance the circuit moves left, before horizontal scroll.
    pub(crate) shift_x: f32,
    /// Space kept right of the wire end when scrolled to the end.
    pub(crate) right_padding: f32,
    /// Circuit-space anchor of the "qN:" labels.
    pub(crate) label_x: f32,
    pub(crate) label_align: egui::Align2,
}

impl CircuitGutters {
    pub(crate) fn for_canvas_width(canvas_width: f32) -> Self {
        if canvas_width < COMPACT_CIRCUIT_MAX_WIDTH {
            Self {
                shift_x: LINE_LEFT_OFFSET - COMPACT_LINE_LEFT_OFFSET,
                right_padding: COMPACT_CIRCUIT_PADDING,
                label_x: LINE_LEFT_OFFSET - COMPACT_QUBIT_LABEL_GAP,
                label_align: egui::Align2::RIGHT_TOP,
            }
        } else {
            Self {
                shift_x: 0.0,
                right_padding: LINE_RIGHT_OFFSET,
                label_x: CIRCUIT_PADDING,
                label_align: egui::Align2::LEFT_TOP,
            }
        }
    }

    /// Largest horizontal scroll that still keeps `right_padding` after the
    /// wire end; `0` when the wires fit.
    pub(crate) fn max_scroll(&self, metrics: &LayoutMetrics, canvas_width: f32) -> f32 {
        (metrics.line_right - self.shift_x + self.right_padding - canvas_width).max(0.0)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct LayoutMetrics {
    pub(crate) line_left: f32,
    pub(crate) line_right: f32,
    pub(crate) line_ys: Vec<f32>,
    pub(crate) slot_left: f32,
    pub(crate) slot_right: f32,
    pub(crate) slot_centers: Vec<f32>,
}

/// Compute layout metrics for the circuit area.
///
/// `min_slots` ensures the wire extends far enough to cover every
/// placed gate even when the rightmost gate sits past the canvas's
/// natural `width - LINE_RIGHT_OFFSET` boundary. Callers compute it
/// from `placed_gates` (e.g. `max_slot_index + 2` so the trailing
/// empty drop-target slot stays visible). Passing `0` keeps the old
/// canvas-width-only behaviour.
pub(crate) fn layout_metrics(width: f32, qubit_count: usize, min_slots: usize) -> LayoutMetrics {
    let line_left = LINE_LEFT_OFFSET;
    let gutters = CircuitGutters::for_canvas_width(width);
    let canvas_line_right = width + gutters.shift_x - gutters.right_padding;
    let line_ys = (0..qubit_count)
        .map(|index| LINE_Y + LINE_GAP * index as f32)
        .collect::<Vec<f32>>();
    let slot_left = line_left + GATE_SIZE;
    let canvas_slot_right = canvas_line_right - GATE_SIZE;
    let canvas_slots = if SLOT_SPACING > 0.0 {
        (((canvas_slot_right - slot_left) / SLOT_SPACING).floor() as i32 + 1).max(0) as usize
    } else {
        0
    };
    // Take whichever is larger: the slots that naturally fit in the
    // canvas, or the slots demanded by the placed-gate set. Wires +
    // slot_centers grow with the larger number.
    let slot_count = canvas_slots.max(min_slots);
    let slot_centers = if slot_count > 0 {
        (0..slot_count)
            .map(|index| slot_left + SLOT_SPACING * index as f32)
            .collect::<Vec<f32>>()
    } else {
        Vec::new()
    };
    let slot_right = slot_centers.last().copied().unwrap_or(slot_left);
    // Wires terminate one GATE_SIZE past the rightmost slot center so
    // the last gate's body sits comfortably inside the line, mirroring
    // the original canvas-width-based formula.
    let line_right = slot_right + GATE_SIZE;
    LayoutMetrics {
        line_left,
        line_right,
        line_ys,
        slot_left,
        slot_right,
        slot_centers,
    }
}

pub(crate) fn nearest_slot_index(x: f32, slot_centers: &[f32]) -> Option<(usize, f32)> {
    let mut nearest_index = None;
    let mut nearest_distance = f32::MAX;
    for (index, &slot) in slot_centers.iter().enumerate() {
        let distance = (x - slot).abs();
        if distance < nearest_distance {
            nearest_distance = distance;
            nearest_index = Some(index);
        }
    }
    nearest_index.map(|index| (index, nearest_distance))
}

pub(crate) fn nearest_line(y: f32, line_ys: &[f32]) -> (f32, f32, usize) {
    let mut nearest = line_ys[0];
    let mut nearest_distance = (y - line_ys[0]).abs();
    let mut nearest_index = 0;
    for (index, &line_y) in line_ys.iter().enumerate() {
        let distance = (y - line_y).abs();
        if distance < nearest_distance {
            nearest = line_y;
            nearest_distance = distance;
            nearest_index = index;
        }
    }
    (nearest, nearest_distance, nearest_index)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Circuit-area widths of the qni tutorial embeds on 1440 px and 390 px
    // pages: the 1068 px / 354 px canvases minus the central panel's 8 px
    // margins.
    const WIDE: f32 = 1052.0;
    const NARROW: f32 = 338.0;
    const PANEL_MARGIN: f32 = 8.0;

    /// Screen x of the right edge of the gate in `column`, before scroll.
    fn gate_right_on_screen(column: usize, canvas_width: f32) -> f32 {
        let metrics = layout_metrics(canvas_width, 2, column + 2);
        metrics.slot_centers[column] + GATE_SIZE / 2.0
            - CircuitGutters::for_canvas_width(canvas_width).shift_x
    }

    #[test]
    fn wide_canvas_keeps_circuit_gutters() {
        assert_eq!(CircuitGutters::for_canvas_width(WIDE).shift_x, 0.0);
    }

    #[test]
    fn narrow_canvas_shows_fifth_column_spacing_4_inside_canvas_edge() {
        let canvas_width = NARROW + 2.0 * PANEL_MARGIN;
        let gate_right_on_canvas = PANEL_MARGIN + gate_right_on_screen(4, NARROW);

        // spacing-4 = 16 px between the gate and the canvas edge.
        assert!(gate_right_on_canvas <= canvas_width - 16.0);
    }

    #[test]
    fn narrow_canvas_wires_start_at_compact_offset() {
        let metrics = layout_metrics(NARROW, 2, 0);

        assert_eq!(
            metrics.line_left - CircuitGutters::for_canvas_width(NARROW).shift_x,
            COMPACT_LINE_LEFT_OFFSET
        );
    }

    #[test]
    fn narrow_canvas_wires_fill_canvas_inside_compact_gutter() {
        let metrics = layout_metrics(NARROW, 2, 0);
        let gutters = CircuitGutters::for_canvas_width(NARROW);
        let slack = NARROW - gutters.right_padding - (metrics.line_right - gutters.shift_x);

        assert!((0.0..SLOT_SPACING).contains(&slack));
    }

    #[test]
    fn fitting_wires_do_not_scroll() {
        let metrics = layout_metrics(NARROW, 2, 0);

        assert_eq!(
            CircuitGutters::for_canvas_width(NARROW).max_scroll(&metrics, NARROW),
            0.0
        );
    }

    #[test]
    fn overflowing_wires_scroll_to_compact_right_gutter() {
        let metrics = layout_metrics(NARROW, 2, 8);
        let gutters = CircuitGutters::for_canvas_width(NARROW);
        let scroll = gutters.max_scroll(&metrics, NARROW);

        assert_eq!(
            metrics.line_right - gutters.shift_x - scroll,
            NARROW - COMPACT_CIRCUIT_PADDING
        );
    }

    #[test]
    fn wide_canvas_scroll_keeps_circuit_padding() {
        let metrics = layout_metrics(WIDE, 2, 30);
        let scroll = CircuitGutters::for_canvas_width(WIDE).max_scroll(&metrics, WIDE);

        assert_eq!(metrics.line_right - scroll, WIDE - CIRCUIT_PADDING);
    }

    #[test]
    fn amplitude_width_cols_follow_spec() {
        let widths = (1..=5)
            .map(amplitude_display_width_cols)
            .collect::<Vec<_>>();

        assert_eq!(widths, vec![2, 2, 2, 4, 3]);
    }

    #[test]
    fn amplitude_span_five_size_is_three_columns_by_five_wires() {
        let size = gate_size(GateKind::AmplitudeDisplay, 5);

        assert_eq!(
            (size.x, size.y),
            (GATE_SIZE + SLOT_SPACING * 2.0, GATE_SIZE + LINE_GAP * 4.0)
        );
    }

    #[test]
    fn amplitude_span_three_grid_is_two_by_four() {
        assert_eq!(amplitude_grid_dims(3), (2, 4));
    }

    #[test]
    fn density_span_eight_size_is_eight_columns_by_eight_wires() {
        let size = gate_size(GateKind::DensityMatrixDisplay, 8);

        assert_eq!(
            (size.x, size.y),
            (GATE_SIZE + SLOT_SPACING * 7.0, GATE_SIZE + LINE_GAP * 7.0)
        );
    }
}
