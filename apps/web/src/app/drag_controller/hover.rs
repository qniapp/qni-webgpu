use eframe::egui;

use super::{step_at_cursor, CircuitInputGeometry, DragController, DragPointer};
use crate::app::QniApp;
use crate::gates::GateKind;
use crate::layout::{amplitude_cell_index_at, density_matrix_cell_index_at, gate_visible_rect};
use crate::span_resize::{span_resize_body_rect, SpanResizeHandles};

impl DragController {
    pub(in crate::app) fn clear_idle_hover(app: &mut QniApp, ctx: &egui::Context) {
        app.hovered_gate_id = None;
        app.hovered_span_resize_handle = None;
        app.hovered_probability_outcome = None;
        app.hovered_amplitude_outcome = None;
        app.hovered_density_cell = None;
        app.hovered_palette_index = None;
        let step_cleared = app.hovered_step.take().is_some();
        let block_cleared = app.hovered_circuit_block.take().is_some();
        if step_cleared || block_cleared {
            ctx.request_repaint();
        }
    }

    pub(in crate::app) fn update_idle_hover(
        app: &mut QniApp,
        pointer: DragPointer,
        geometry: &CircuitInputGeometry,
        ctx: &egui::Context,
    ) {
        if let Some(cursor) = pointer.local_pos {
            let edit_hover_allowed = !app.library.active_locked();
            let circuit_hit_allowed = geometry.circuit_hit_allowed(pointer.screen_pos);
            let upright = geometry.upright();
            // Iterate top-of-stack first so a resizable-span handle wins
            // over the gate body when the cursor is on both (span handles
            // overhang the top / bottom edges).
            let previous_gate = app.hovered_gate_id;
            let previous_handle = app.hovered_span_resize_handle;
            let previous_probability_outcome = app.hovered_probability_outcome;
            let previous_amplitude_outcome = app.hovered_amplitude_outcome;
            let previous_density_cell = app.hovered_density_cell;
            let mut hovered_gate = None;
            let mut hovered_handle = None;
            let mut hovered_probability_outcome = None;
            let mut hovered_amplitude_outcome = None;
            let mut hovered_density_cell = None;
            if edit_hover_allowed && circuit_hit_allowed {
                for gate in app.placed_gates.iter().rev() {
                    let gate_rect = gate_visible_rect(gate, gate.committed_pos());
                    let resize_handles = SpanResizeHandles::for_gate_with_availability(
                        gate,
                        &app.placed_gates,
                        app.wire_capacity().get(),
                    )
                    .filter(|_| !geometry.is_vertical());
                    let body_rect = resize_handles
                        .map(SpanResizeHandles::body_rect)
                        .unwrap_or_else(|| {
                            span_resize_body_rect(gate.kind, gate.span.get(), gate_rect)
                        });
                    if let Some(handles) = resize_handles {
                        if let Some(edge) = handles.edge_at(cursor) {
                            hovered_handle = Some(handles.handle(edge));
                            hovered_gate = Some(gate.id);
                            break;
                        }
                    }
                    if body_rect.contains(cursor) {
                        hovered_gate = Some(gate.id);
                        // Display cells are laid out upright on the screen,
                        // so their hit test runs in the upright frame.
                        let gate_rect = upright.to_screen_rect(gate_rect);
                        let cursor = upright.to_screen(cursor);
                        if gate.kind == GateKind::ProbabilityDisplay
                            && !(geometry.is_vertical() && gate.span.get() > 1)
                        {
                            let row_count = 1usize << gate.span.get().clamp(1, 16);
                            let row_h = gate_rect.height() / row_count as f32;
                            let row = ((cursor.y - gate_rect.top()) / row_h)
                                .floor()
                                .clamp(0.0, (row_count - 1) as f32)
                                as u32;
                            hovered_probability_outcome = Some((gate.id, row));
                        } else if gate.kind == GateKind::AmplitudeDisplay {
                            hovered_amplitude_outcome =
                                amplitude_cell_index_at(gate_rect, gate.span.get(), cursor)
                                    .map(|outcome| (gate.id, outcome));
                        } else if gate.kind == GateKind::DensityMatrixDisplay {
                            hovered_density_cell =
                                density_matrix_cell_index_at(gate_rect, gate.span.get(), cursor)
                                    .map(|cell| (gate.id, cell));
                        }
                        break;
                    }
                }
            }
            app.hovered_gate_id = hovered_gate;
            app.hovered_span_resize_handle = hovered_handle;
            app.hovered_probability_outcome = hovered_probability_outcome;
            app.hovered_amplitude_outcome = hovered_amplitude_outcome;
            app.hovered_density_cell = hovered_density_cell;
            if hovered_gate != previous_gate
                || hovered_handle != previous_handle
                || hovered_probability_outcome != previous_probability_outcome
                || hovered_amplitude_outcome != previous_amplitude_outcome
                || hovered_density_cell != previous_density_cell
            {
                ctx.request_repaint();
            }

            let hovered_block = (app.circuit_block_editing_allowed() && circuit_hit_allowed)
                .then(|| app.circuit_block_hover_at(cursor, &geometry.metrics, ctx))
                .flatten();
            if hovered_block != app.hovered_circuit_block {
                app.hovered_circuit_block = hovered_block;
                ctx.request_repaint();
            }

            // Step preview: which column is the cursor on? Changes select a
            // cached GPU snapshot so the state-vector panel reflects the new
            // step without rerunning simulation.
            let new_hovered_step = circuit_hit_allowed
                .then(|| step_at_cursor(cursor, &geometry.metrics))
                .flatten();
            if new_hovered_step != app.hovered_step {
                app.hovered_step = new_hovered_step;
                ctx.request_repaint();
            }

            let previous_palette = app.hovered_palette_index;
            let mut hovered_palette = None;
            if edit_hover_allowed {
                if let Some(cursor_screen) = pointer.screen_pos {
                    hovered_palette = geometry.hovered_palette_entry(app, cursor_screen);
                }
            }
            app.hovered_palette_index = hovered_palette;
            if hovered_palette != previous_palette {
                ctx.request_repaint();
            }
        } else {
            Self::clear_idle_hover(app, ctx);
        }
    }

    pub(in crate::app) fn set_cursor_icon(app: &QniApp, pointer: DragPointer, ctx: &egui::Context) {
        if app.dragging.is_some() && pointer.down {
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if app.hovered_span_resize_handle.is_some() {
            ctx.set_cursor_icon(egui::CursorIcon::ResizeVertical);
        } else if app
            .hovered_circuit_block
            .is_some_and(|hover| hover.handle.is_some())
        {
            ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        } else if app
            .hovered_circuit_block
            .is_some_and(|hover| hover.label.is_some())
        {
            ctx.set_cursor_icon(egui::CursorIcon::Text);
        } else if app.hovered_gate_id.is_some() || app.hovered_palette_index.is_some() {
            ctx.set_cursor_icon(egui::CursorIcon::Grab);
        }
    }
}
