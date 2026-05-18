use eframe::egui;

use super::{step_at_cursor, CircuitInputGeometry, DragController, DragPointer};
use crate::app::QniApp;
use crate::app::SpanResizeHandle;
use crate::gates::GateKind;
use crate::layout::{gate_visible_rect, palette_hit_test, span_resize_handle_edge_at};

impl DragController {
    pub(in crate::app) fn clear_idle_hover(app: &mut QniApp, ctx: &egui::Context) {
        app.hovered_gate_id = None;
        app.hovered_span_resize_handle = None;
        app.hovered_chance_outcome = None;
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
            let previous_chance_outcome = app.hovered_chance_outcome;
            let mut hovered_gate = None;
            let mut hovered_handle = None;
            let mut hovered_chance_outcome = None;
            for gate in app.placed_gates.iter().rev() {
                let gate_rect = gate_visible_rect(gate, gate.pos);
                if gate.kind.is_resizable_span() {
                    if let Some(edge) = span_resize_handle_edge_at(gate_rect, cursor) {
                        hovered_handle = Some(SpanResizeHandle {
                            gate_id: gate.id,
                            edge,
                        });
                        hovered_gate = Some(gate.id);
                        break;
                    }
                }
                if gate_rect.contains(cursor) {
                    hovered_gate = Some(gate.id);
                    if gate.kind == GateKind::ChanceDisplay {
                        let row_count = 1usize << gate.span.clamp(1, 16);
                        let row_h = gate_rect.height() / row_count as f32;
                        let row = ((cursor.y - gate_rect.top()) / row_h)
                            .floor()
                            .clamp(0.0, (row_count - 1) as f32)
                            as u32;
                        hovered_chance_outcome = Some((gate.id, row));
                    }
                    break;
                }
            }
            app.hovered_gate_id = hovered_gate;
            app.hovered_span_resize_handle = hovered_handle;
            app.hovered_chance_outcome = hovered_chance_outcome;
            if hovered_chance_outcome != previous_chance_outcome {
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
