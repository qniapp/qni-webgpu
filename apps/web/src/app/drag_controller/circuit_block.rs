//! Comment-block gestures: carrying the palette's block onto gate columns,
//! resizing a block from its left / right handle, and dragging a label to
//! move the block or, outside the circuit band, to remove it. A block never
//! moves gates; it only changes which columns it wraps.

use eframe::egui;

use super::{reset_drag_frame_state, DragController, DragPointer};
use crate::app::{
    BlockEdges, CircuitBlockDrag, CircuitBlockEdge, CircuitBlockLabelSlot, CircuitColumnIndex,
    QniApp,
};
use crate::constants::{
    CIRCUIT_BLOCK_BORDER_WIDTH, CIRCUIT_BLOCK_MARGIN_Y, CIRCUIT_BLOCK_PADDING_Y, LINE_GAP,
    SLOT_SPACING,
};
use crate::layout::{nearest_slot_index, LayoutMetrics};
use crate::render::{circuit_block_boundary_x, circuit_block_rect};

pub(crate) const CIRCUIT_BLOCK_DEFAULT_LABEL: &str = "コメント";

type ColumnRange = (CircuitColumnIndex, CircuitColumnIndex);

impl DragController {
    pub(in crate::app) fn start_circuit_block_palette_drag(
        app: &mut QniApp,
        cursor: egui::Pos2,
        metrics: &LayoutMetrics,
    ) {
        app.circuit_block_drag = Some(CircuitBlockDrag::Palette {
            cursor,
            target: palette_target(app, cursor, metrics),
        });
    }

    pub(in crate::app) fn start_circuit_block_label_drag(
        app: &mut QniApp,
        index: usize,
        slot: CircuitBlockLabelSlot,
        cursor: egui::Pos2,
        metrics: &LayoutMetrics,
    ) {
        let Some(block) = app.circuit_blocks.get(index) else {
            return;
        };
        let grab_dx = cursor.x - circuit_block_boundary_x(metrics, block.start().as_usize());
        app.circuit_block_drag = Some(CircuitBlockDrag::Label {
            index,
            slot,
            press_pos: cursor,
            grab_dx,
            cursor,
            moved: false,
            removing: false,
        });
    }

    /// Advance the in-flight block gesture. Returns `false` when there is
    /// none, so the rest of the input pipeline runs.
    pub(in crate::app) fn update_circuit_block_drag(
        app: &mut QniApp,
        pointer: DragPointer,
        metrics: &LayoutMetrics,
        ctx: &egui::Context,
    ) -> bool {
        let Some(drag) = app.circuit_block_drag else {
            return false;
        };
        let cursor = pointer
            .local_pos
            .filter(|_| pointer.down || pointer.released);
        let drag = match (drag, cursor) {
            (CircuitBlockDrag::Palette { .. }, Some(cursor)) => CircuitBlockDrag::Palette {
                cursor,
                target: palette_target(app, cursor, metrics),
            },
            (CircuitBlockDrag::Resize { index, edge }, Some(cursor)) => {
                if let Some((start, end)) = resize_target(app, index, edge, cursor.x, metrics) {
                    let _ = app.circuit_blocks.resize(index, start, end);
                }
                drag
            }
            (
                CircuitBlockDrag::Label {
                    index,
                    slot,
                    press_pos,
                    grab_dx,
                    moved,
                    ..
                },
                Some(cursor),
            ) => {
                let max_click_dist = ctx.options(|options| options.input_options.max_click_dist);
                let moved = moved || cursor.distance(press_pos) > max_click_dist;
                let removing = moved && outside_block_band(app, index, cursor, metrics);
                let mut index = index;
                if moved && !removing {
                    if let Some(start) = move_target(app, index, cursor.x - grab_dx, metrics) {
                        index = app.circuit_blocks.move_to(index, start).unwrap_or(index);
                    }
                }
                CircuitBlockDrag::Label {
                    index,
                    slot,
                    press_pos,
                    grab_dx,
                    cursor,
                    moved,
                    removing,
                }
            }
            (drag, None) => drag,
        };
        app.circuit_block_drag = Some(drag);
        if pointer.released {
            Self::finish_circuit_block_drag(app, drag, ctx);
        }
        ctx.set_cursor_icon(match drag {
            CircuitBlockDrag::Resize { .. } => egui::CursorIcon::ResizeHorizontal,
            CircuitBlockDrag::Label { moved: false, .. } => egui::CursorIcon::Text,
            _ => egui::CursorIcon::Grabbing,
        });
        ctx.request_repaint();
        true
    }

    fn finish_circuit_block_drag(app: &mut QniApp, drag: CircuitBlockDrag, ctx: &egui::Context) {
        app.circuit_block_drag = None;
        reset_drag_frame_state(app);
        let mut edit = None;
        match drag {
            CircuitBlockDrag::Palette {
                target: Some((start, end)),
                ..
            } => {
                edit = app
                    .circuit_blocks
                    .insert(CIRCUIT_BLOCK_DEFAULT_LABEL.to_owned(), start, end)
                    .ok()
                    .map(|index| (index, CircuitBlockLabelSlot::Top));
            }
            CircuitBlockDrag::Label {
                index,
                removing: true,
                ..
            } => {
                app.circuit_blocks.remove(index);
            }
            CircuitBlockDrag::Label {
                index,
                slot,
                moved: false,
                ..
            } => edit = Some((index, slot)),
            _ => {}
        }
        app.commit_current_circuit(ctx);
        if let Some((index, slot)) = edit {
            app.open_circuit_block_editor(index, slot, ctx);
        }
    }
}

/// Columns a block dropped at `cursor` wraps: the gate column nearest the
/// pointer, widened to whole multi-column gates, when the pointer is over
/// the circuit band and no block holds those columns yet.
fn palette_target(
    app: &QniApp,
    cursor: egui::Pos2,
    metrics: &LayoutMetrics,
) -> Option<ColumnRange> {
    let pad_y = LINE_GAP * 0.5 + CIRCUIT_BLOCK_PADDING_Y + CIRCUIT_BLOCK_BORDER_WIDTH;
    let top = metrics.line_ys.first()? - pad_y;
    let bottom = metrics.line_ys.last()? + pad_y;
    if !(top..=bottom).contains(&cursor.y) {
        return None;
    }
    let (column, distance) = nearest_slot_index(cursor.x, &metrics.slot_centers)?;
    if distance > SLOT_SPACING * 0.5 {
        return None;
    }
    let (start, end) =
        BlockEdges::for_gates(&app.placed_gates).range_around(CircuitColumnIndex::new(column))?;
    app.circuit_blocks
        .is_free(start.as_usize(), end.as_usize())
        .then_some((start, end))
}

/// New range for block `index` with its `edge` moved to the allowed
/// boundary nearest `x`, staying between the neighbouring blocks, within the
/// gate columns, and wrapping at least one gate.
fn resize_target(
    app: &QniApp,
    index: usize,
    edge: CircuitBlockEdge,
    x: f32,
    metrics: &LayoutMetrics,
) -> Option<ColumnRange> {
    let block = app.circuit_blocks.get(index)?;
    let (start, end) = (block.start().as_usize(), block.end().as_usize());
    let edges = BlockEdges::for_gates(&app.placed_gates);
    let previous_end = index
        .checked_sub(1)
        .and_then(|previous| app.circuit_blocks.get(previous))
        .map_or(0, |previous| previous.end().as_usize());
    let next_start = app
        .circuit_blocks
        .get(index + 1)
        .map_or(usize::MAX, |next| next.start().as_usize());
    let candidates: Vec<usize> = match edge {
        CircuitBlockEdge::Left => (previous_end..end)
            .filter(|&boundary| {
                boundary == start || (edges.allows(boundary) && edges.has_gate_in(boundary, end))
            })
            .collect(),
        CircuitBlockEdge::Right => (start + 1..=end.max(edges.gate_end().min(next_start)))
            .filter(|&boundary| {
                boundary == end || (edges.allows(boundary) && edges.has_gate_in(start, boundary))
            })
            .collect(),
    };
    let boundary = nearest_boundary(candidates, x, metrics)?;
    Some(match edge {
        CircuitBlockEdge::Left => (CircuitColumnIndex::new(boundary), block.end()),
        CircuitBlockEdge::Right => (block.start(), CircuitColumnIndex::new(boundary)),
    })
}

/// Start column for block `index` moved so its left edge lands nearest
/// `left_x`, keeping its width, on allowed boundaries over gate columns that
/// no other block holds.
fn move_target(
    app: &QniApp,
    index: usize,
    left_x: f32,
    metrics: &LayoutMetrics,
) -> Option<CircuitColumnIndex> {
    let block = app.circuit_blocks.get(index)?;
    let width = block.end().as_usize() - block.start().as_usize();
    let edges = BlockEdges::for_gates(&app.placed_gates);
    let mut others = app.circuit_blocks.clone();
    others.remove(index);
    let candidates: Vec<usize> = (0..=edges.gate_end().saturating_sub(width))
        .filter(|&start| {
            let end = start + width;
            edges.allows(start)
                && edges.allows(end)
                && edges.has_gate_in(start, end)
                && others.is_free(start, end)
        })
        .collect();
    nearest_boundary(candidates, left_x, metrics).map(CircuitColumnIndex::new)
}

fn nearest_boundary(candidates: Vec<usize>, x: f32, metrics: &LayoutMetrics) -> Option<usize> {
    candidates.into_iter().min_by(|&a, &b| {
        let distance = |boundary| (circuit_block_boundary_x(metrics, boundary) - x).abs();
        distance(a).total_cmp(&distance(b))
    })
}

/// Whether `cursor` left the band of block `index`: its body plus the label
/// margins above and below.
fn outside_block_band(
    app: &QniApp,
    index: usize,
    cursor: egui::Pos2,
    metrics: &LayoutMetrics,
) -> bool {
    app.circuit_blocks
        .get(index)
        .and_then(|block| circuit_block_rect(block, metrics))
        .is_some_and(|body| {
            !(body.top() - CIRCUIT_BLOCK_MARGIN_Y..=body.bottom() + CIRCUIT_BLOCK_MARGIN_Y)
                .contains(&cursor.y)
        })
}
