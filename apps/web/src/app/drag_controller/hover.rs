use eframe::egui;

use super::{step_at_cursor, CircuitInputGeometry, DragController, DragPointer};
use crate::app::QniApp;
use crate::gates::GateKind;
use crate::layout::{
    amplitude_cell_index_at, density_matrix_cell_index_at, gate_visible_rect, palette_hit_test,
};
use crate::span_resize::{span_resize_body_rect, SpanResizeHandles};

impl DragController {
    pub(in crate::app) fn clear_idle_hover(app: &mut QniApp, ctx: &egui::Context) {
        app.hovered_gate_id = None;
        app.hovered_span_resize_handle = None;
        app.hovered_probability_outcome = None;
        app.hovered_amplitude_outcome = None;
        app.hovered_density_cell = None;
        app.hovered_palette_index = None;
        if app.hovered_step.take().is_some() {
            app.gpu_plan.mark_step_preview_dirty();
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
            // Iterate top-of-stack first so a resizable-span handle wins
            // over the gate body when the cursor is on both (span handles
            // overhang the top / bottom edges).
            let previous_probability_outcome = app.hovered_probability_outcome;
            let previous_amplitude_outcome = app.hovered_amplitude_outcome;
            let previous_density_cell = app.hovered_density_cell;
            let mut hovered_gate = None;
            let mut hovered_handle = None;
            let mut hovered_probability_outcome = None;
            let mut hovered_amplitude_outcome = None;
            let mut hovered_density_cell = None;
            for gate in app.placed_gates.iter().rev() {
                let gate_rect = gate_visible_rect(gate, gate.pos);
                let resize_handles = SpanResizeHandles::for_gate_with_availability(
                    gate,
                    &app.placed_gates,
                    app.exec_mode.qubit_capacity().get(),
                );
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
                    if gate.kind == GateKind::ProbabilityDisplay {
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
            app.hovered_gate_id = hovered_gate;
            app.hovered_span_resize_handle = hovered_handle;
            app.hovered_probability_outcome = hovered_probability_outcome;
            app.hovered_amplitude_outcome = hovered_amplitude_outcome;
            app.hovered_density_cell = hovered_density_cell;
            if hovered_probability_outcome != previous_probability_outcome
                || hovered_amplitude_outcome != previous_amplitude_outcome
                || hovered_density_cell != previous_density_cell
            {
                ctx.request_repaint();
            }

            // Step preview: which column is the cursor on? Changes select a
            // cached GPU snapshot so the state-vector panel reflects the new
            // step without rerunning simulation.
            let new_hovered_step = step_at_cursor(cursor, &geometry.metrics);
            if new_hovered_step != app.hovered_step {
                app.hovered_step = new_hovered_step;
                app.gpu_plan.mark_step_preview_dirty();
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
            Self::clear_idle_hover(app, ctx);
        }
    }

    pub(in crate::app) fn set_cursor_icon(app: &QniApp, pointer: DragPointer, ctx: &egui::Context) {
        if app.dragging.is_some() && pointer.down {
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if app.hovered_span_resize_handle.is_some() {
            ctx.set_cursor_icon(egui::CursorIcon::ResizeVertical);
        } else if app.hovered_gate_id.is_some() || app.hovered_palette_index.is_some() {
            ctx.set_cursor_icon(egui::CursorIcon::Grab);
        }
    }
}
