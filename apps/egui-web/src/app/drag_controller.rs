//! Gate drag controller — transforms pointer snapshots into drag / resize /
//! drop state mutations. Keeps qni's SNAP / DROP / RESIZE style event
//! boundaries explicit while `gate_input` stays a thin egui adapter.

use eframe::egui;

use super::{DragState, PlacedGate, QftResizeDrag, QniApp};
use crate::constants::{
    CIRCUIT_PADDING, GATE_SIZE, LINE_GAP, PALETTE_ROW_Y, QFT_MAX_SPAN, SLOT_SPACING, SNAP_DISTANCE,
};
use crate::gates::PALETTE_GATES;
use crate::layout::{
    gate_visible_rect, layout_metrics, nearest_available_slot, nearest_line, nearest_slot_index,
    palette_hit_test, palette_layout, qft_resize_handle_rect, LayoutMetrics, PaletteLayout,
};

#[derive(Clone, Copy, Debug)]
pub(super) struct DragPointer {
    pub(super) screen_pos: Option<egui::Pos2>,
    pub(super) local_pos: Option<egui::Pos2>,
    pub(super) down: bool,
    pub(super) start: bool,
    pub(super) released: bool,
}

#[derive(Clone, Debug)]
pub(super) struct CircuitInputGeometry {
    pub(super) metrics: LayoutMetrics,
    palette_origin: egui::Pos2,
    palette_rect: egui::Rect,
    palette_layout: PaletteLayout,
}

impl CircuitInputGeometry {
    pub(super) fn new(
        content_rect: egui::Rect,
        screen_rect: egui::Rect,
        layout_qubits: usize,
        min_slots: usize,
    ) -> Self {
        let palette_layout = palette_layout();
        let palette_start_x = screen_rect.width() / 2.0 - palette_layout.total_width / 2.0;
        let palette_origin = egui::pos2(
            screen_rect.min.x + palette_start_x,
            screen_rect.min.y + PALETTE_ROW_Y,
        );
        let palette_rect = egui::Rect::from_min_size(
            palette_origin,
            egui::vec2(palette_layout.total_width, palette_layout.total_height),
        );
        let metrics = layout_metrics(content_rect.width(), layout_qubits, min_slots);
        Self {
            metrics,
            palette_origin,
            palette_rect,
            palette_layout,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum DragStartIntent {
    QftResize(QftResizeDrag),
    ExistingGate(DragState),
    PaletteGate {
        index: usize,
        preview_pos: egui::Pos2,
    },
    BreakpointStep(usize),
    None,
}

pub(super) struct DragController;

impl DragController {
    pub(super) fn update_circuit_scroll(
        app: &mut QniApp,
        ctx: &egui::Context,
        content_rect: egui::Rect,
        screen_pos: Option<egui::Pos2>,
        metrics: &LayoutMetrics,
    ) {
        // Horizontal scroll: trackpad horizontal swipes come through
        // as `smooth_scroll_delta.x`; desktop mice typically only
        // produce a `delta.y`, so we treat shift+wheel-y as wheel-x
        // when the cursor is inside the circuit area. Scroll right
        // (positive delta) → reveal trailing gates → `scroll_x`
        // grows. Always clamp to `[0, max(0, line_right -
        // canvas_width + CIRCUIT_PADDING)]` so the rightmost slot
        // stops just past the canvas edge.
        let cursor_in_circuit = screen_pos.is_some_and(|p| content_rect.contains(p));
        if cursor_in_circuit {
            let (raw_dx, raw_dy, shift) = ctx.input(|i| {
                (
                    i.smooth_scroll_delta.x,
                    i.smooth_scroll_delta.y,
                    i.modifiers.shift,
                )
            });
            let dx = if raw_dx.abs() > raw_dy.abs() || !shift {
                raw_dx
            } else {
                raw_dy
            };
            if dx != 0.0 {
                let max_scroll = max_circuit_scroll(metrics, content_rect.width());
                app.circuit_scroll_x = (app.circuit_scroll_x - dx).clamp(0.0, max_scroll);
                ctx.request_repaint();
            }
        }
        // After the scroll update, force a clamp so newly-loaded
        // circuits or window resizes never leave us scrolled past the
        // current content extent.
        let max_scroll = max_circuit_scroll(metrics, content_rect.width());
        if app.circuit_scroll_x > max_scroll {
            app.circuit_scroll_x = max_scroll;
        }
    }

    pub(super) fn handle_pointer_start(
        app: &mut QniApp,
        pointer: DragPointer,
        geometry: &CircuitInputGeometry,
        ctx: &egui::Context,
    ) -> bool {
        match start_intent(app, pointer, geometry) {
            DragStartIntent::QftResize(resize) => {
                app.qft_resize_drag = Some(resize);
                app.hovered_gate_id = None;
                app.hovered_palette_index = None;
                ctx.request_repaint();
                true
            }
            DragStartIntent::ExistingGate(drag) => {
                app.dragging = Some(drag);
                app.drag_state_count = Some(app.state_count());
                app.drag_cursor_pos = pointer.local_pos;
                app.hovered_gate_id = None;
                app.hovered_palette_index = None;
                ctx.request_repaint();
                true
            }
            DragStartIntent::PaletteGate { index, preview_pos } => {
                let new_id = app.next_gate_id;
                let mut new_gate = PlacedGate::new(
                    new_id,
                    PALETTE_GATES[index],
                    0,
                    0,
                    1,
                    // Palette drop: no explicit angle yet — Phase falls
                    // back to its π/2 default until a future angle picker
                    // lets the user set one.
                    None,
                );
                new_gate.pos = preview_pos;
                app.next_gate_id += 1;
                app.placed_gates.push(new_gate);
                app.dragging = Some(DragState {
                    id: new_id,
                    offset: egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0),
                });
                app.drag_state_count = Some(app.state_count());
                app.drag_cursor_pos = pointer.local_pos;
                app.hovered_palette_index = None;
                app.hovered_gate_id = None;
                ctx.request_repaint();
                true
            }
            DragStartIntent::BreakpointStep(step) => {
                if app.breakpoint_step != Some(step) {
                    app.breakpoint_step = Some(step);
                    app.gpu_plan.mark_dirty();
                    ctx.request_repaint();
                }
                true
            }
            DragStartIntent::None => false,
        }
    }

    pub(super) fn update_active_qft_resize(
        app: &mut QniApp,
        pointer: DragPointer,
        ctx: &egui::Context,
    ) -> bool {
        let Some(drag) = app.qft_resize_drag else {
            return false;
        };
        if pointer.down || pointer.released {
            if let Some(cursor) = pointer.local_pos {
                let delta_y = cursor.y - drag.start_pointer_y;
                // One LINE_GAP of drag = one extra wire. Round to the
                // nearest integer so the snap feels positive.
                let span_delta = (delta_y / LINE_GAP).round() as i32;
                let new_span =
                    (drag.start_span as i32 + span_delta).clamp(1, QFT_MAX_SPAN as i32) as usize;
                if let Some(index) = app
                    .placed_gates
                    .iter()
                    .position(|gate| gate.id == drag.gate_id)
                {
                    if app.placed_gates[index].span != new_span {
                        app.placed_gates[index].span = new_span;
                        app.update_qubit_count();
                        app.gpu_plan.mark_dirty();
                        ctx.request_repaint();
                    }
                }
            }
        }
        if pointer.released {
            app.qft_resize_drag = None;
            reset_drag_frame_state(app);
        }
        // Keep the cursor in a "resize" mode while the drag is active.
        ctx.set_cursor_icon(egui::CursorIcon::ResizeVertical);
        true
    }

    pub(super) fn update_gate_drag_preview(
        app: &mut QniApp,
        pointer: DragPointer,
        metrics: &LayoutMetrics,
    ) {
        let Some(drag) = app.dragging else {
            return;
        };
        if !(pointer.down || pointer.released) {
            return;
        }
        let cursor = pointer.local_pos.or(app.drag_cursor_pos);
        let Some(cursor) = cursor else {
            return;
        };
        app.drag_cursor_pos = Some(cursor);
        let Some(index) = app.placed_gates.iter().position(|gate| gate.id == drag.id) else {
            return;
        };

        let mut next_pos = cursor - drag.offset;
        let mut next_wire = app.placed_gates[index].wire;
        let mut next_column = app.placed_gates[index].column;
        let center_y = next_pos.y + GATE_SIZE / 2.0;
        let (line_y, distance, line_index) = nearest_line(center_y, &metrics.line_ys);
        if distance <= SNAP_DISTANCE {
            next_pos.y = line_y - GATE_SIZE / 2.0;
            next_wire = line_index;
            let center_x = next_pos.x + GATE_SIZE / 2.0;
            if let Some(snap) = nearest_available_slot(
                center_x,
                line_index,
                Some(drag.id),
                &app.placed_gates,
                &metrics.slot_centers,
            ) {
                next_pos.x = snap.center - GATE_SIZE / 2.0;
                next_column = snap.index;
            }
        }
        let gate = &mut app.placed_gates[index];
        gate.pos = next_pos;
        gate.wire = next_wire;
        gate.column = next_column;
    }

    pub(super) fn update_idle_hover(
        app: &mut QniApp,
        pointer: DragPointer,
        geometry: &CircuitInputGeometry,
        ctx: &egui::Context,
    ) {
        if let Some(cursor) = pointer.local_pos {
            // Iterate top-of-stack first so the QFT resize handle wins
            // over the gate body when the cursor is on both (the handle
            // overhangs the gate's bottom edge).
            let mut hovered_gate = None;
            let mut hovered_handle = None;
            for gate in app.placed_gates.iter().rev() {
                let gate_rect = gate_visible_rect(gate, gate.pos);
                if gate.kind.is_resizable_span()
                    && qft_resize_handle_rect(gate_rect).contains(cursor)
                {
                    hovered_handle = Some(gate.id);
                    hovered_gate = Some(gate.id);
                    break;
                }
                if gate_rect.contains(cursor) {
                    hovered_gate = Some(gate.id);
                    break;
                }
            }
            app.hovered_gate_id = hovered_gate;
            app.hovered_qft_resize_handle = hovered_handle;

            // Step preview: which column is the cursor on? Changes
            // trigger a recompute so the state-vector panel reflects
            // the new step in real time.
            let new_hovered_step = step_at_cursor(cursor, &geometry.metrics);
            if new_hovered_step != app.hovered_step {
                app.hovered_step = new_hovered_step;
                app.gpu_plan.mark_dirty();
                ctx.request_repaint();
            }

            let mut hovered_palette = None;
            if let Some(cursor_screen) = pointer.screen_pos {
                if geometry.palette_rect.contains(cursor_screen) {
                    let local = egui::pos2(
                        cursor_screen.x - geometry.palette_origin.x,
                        cursor_screen.y - geometry.palette_origin.y,
                    );
                    hovered_palette = palette_hit_test(local, &geometry.palette_layout);
                }
            }
            app.hovered_palette_index = hovered_palette;
        } else {
            app.hovered_gate_id = None;
            app.hovered_qft_resize_handle = None;
            app.hovered_palette_index = None;
            if app.hovered_step.is_some() {
                app.hovered_step = None;
                app.gpu_plan.mark_dirty();
            }
        }
    }

    pub(super) fn commit_gate_drop(
        app: &mut QniApp,
        pointer: DragPointer,
        metrics: &LayoutMetrics,
        ctx: &egui::Context,
    ) {
        if !pointer.released {
            return;
        }
        if let Some(drag) = app.dragging.take() {
            if let Some(index) = app.placed_gates.iter().position(|gate| gate.id == drag.id) {
                let gate_pos = app.placed_gates[index].pos;
                let gate_id = app.placed_gates[index].id;
                let center_x = gate_pos.x + GATE_SIZE / 2.0;
                let center_y = gate_pos.y + GATE_SIZE / 2.0;
                let (_line_y, distance, line_index) = nearest_line(center_y, &metrics.line_ys);
                let snapped = nearest_available_slot(
                    center_x,
                    line_index,
                    Some(gate_id),
                    &app.placed_gates,
                    &metrics.slot_centers,
                );
                let on_circuit = center_x >= metrics.slot_left
                    && center_x <= metrics.slot_right
                    && distance <= SNAP_DISTANCE
                    && snapped
                        .as_ref()
                        .map(|snap| snap.distance <= SNAP_DISTANCE)
                        .unwrap_or(false);

                if !on_circuit {
                    app.placed_gates.remove(index);
                } else if let Some(snap) = snapped {
                    let gate = &mut app.placed_gates[index];
                    gate.column = snap.index;
                    gate.wire = line_index;
                    gate.sync_pos_from_grid();
                }
                // Mirror qni's post-drop `resize()`: remove empty
                // columns and shift trailing gates left for both branches.
                app.compact_empty_steps();
                app.update_qubit_count();
                // Mirror qni / Quirk: every committed circuit change
                // syncs to the URL hash. We use Quirk's readable-JSON
                // format (`#circuit={"cols":[...]}`) instead of qni's
                // percent-encoded path.
                let json = crate::url_circuit::circuit_to_json(&app.placed_gates, app.qubit_count);
                crate::url_circuit::write_circuit_to_url(&json);
                app.gpu_plan.mark_dirty();
                ctx.request_repaint();
            }
        }
        app.drag_state_count = None;
        reset_drag_frame_state(app);
        app.drag_cursor_pos = None;
    }

    pub(super) fn set_cursor_icon(app: &QniApp, pointer: DragPointer, ctx: &egui::Context) {
        if app.dragging.is_some() && pointer.down {
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if app.hovered_qft_resize_handle.is_some() {
            ctx.set_cursor_icon(egui::CursorIcon::ResizeVertical);
        } else if app.hovered_gate_id.is_some() || app.hovered_palette_index.is_some() {
            ctx.set_cursor_icon(egui::CursorIcon::Grab);
        }
    }
}

fn start_intent(
    app: &QniApp,
    pointer: DragPointer,
    geometry: &CircuitInputGeometry,
) -> DragStartIntent {
    let Some(cursor) = pointer.local_pos else {
        return DragStartIntent::None;
    };

    // QFT resize handle takes priority over gate body for press events,
    // so dragging the bottom-edge chevron resizes the span instead of
    // picking up the whole gate.
    if let Some(resize) = app
        .placed_gates
        .iter()
        .rev()
        .find(|gate| {
            if !gate.kind.is_resizable_span() {
                return false;
            }
            let gate_rect = gate_visible_rect(gate, gate.pos);
            qft_resize_handle_rect(gate_rect).contains(cursor)
        })
        .map(|gate| QftResizeDrag {
            gate_id: gate.id,
            start_pointer_y: cursor.y,
            start_span: gate.span.max(1),
        })
    {
        return DragStartIntent::QftResize(resize);
    }

    if let Some(drag) = app
        .placed_gates
        .iter()
        .rev()
        .find(|gate| {
            let gate_rect = gate_visible_rect(gate, gate.pos);
            gate_rect.contains(cursor)
        })
        .map(|gate| DragState {
            id: gate.id,
            offset: cursor - gate.pos,
        })
    {
        return DragStartIntent::ExistingGate(drag);
    }

    if let Some(cursor_screen) = pointer.screen_pos {
        let local = egui::pos2(
            cursor_screen.x - geometry.palette_origin.x,
            cursor_screen.y - geometry.palette_origin.y,
        );
        if let Some(index) = palette_hit_test(local, &geometry.palette_layout) {
            return DragStartIntent::PaletteGate {
                index,
                preview_pos: egui::pos2(cursor.x - GATE_SIZE / 2.0, cursor.y - GATE_SIZE / 2.0),
            };
        }
    }

    // No gate / palette under the cursor. If we're inside a step slot,
    // lock the breakpoint to that column.
    step_at_cursor(cursor, &geometry.metrics)
        .map(DragStartIntent::BreakpointStep)
        .unwrap_or(DragStartIntent::None)
}

/// Column index the pointer is hovering over for step-preview.
/// Returns `None` when outside the slot row / range.
fn step_at_cursor(cursor: egui::Pos2, metrics: &LayoutMetrics) -> Option<usize> {
    if metrics.slot_centers.is_empty() || metrics.line_ys.is_empty() {
        return None;
    }
    let top = metrics.line_ys[0] - LINE_GAP * 0.5;
    let bottom = metrics.line_ys[metrics.line_ys.len() - 1] + LINE_GAP * 0.5;
    if cursor.y < top || cursor.y > bottom {
        return None;
    }
    if cursor.x < metrics.slot_left - SLOT_SPACING * 0.5
        || cursor.x > metrics.slot_right + SLOT_SPACING * 0.5
    {
        return None;
    }
    let (slot, dist) = nearest_slot_index(cursor.x, &metrics.slot_centers)?;
    if dist <= SLOT_SPACING * 0.5 {
        Some(slot)
    } else {
        None
    }
}

fn max_circuit_scroll(metrics: &LayoutMetrics, content_width: f32) -> f32 {
    (metrics.line_right + CIRCUIT_PADDING - content_width).max(0.0)
}

fn reset_drag_frame_state(app: &mut QniApp) {
    app.drag_repaint_deadline = None;
    app.drag_repaint_pending = false;
}
