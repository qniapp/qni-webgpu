use eframe::egui;

use super::{step_at_cursor, CircuitInputGeometry, DragController, DragPointer};
use crate::app::QniApp;
use crate::layout::{gate_visible_rect, palette_hit_test, qft_resize_handle_rect};

impl DragController {
    pub(in crate::app) fn clear_idle_hover(app: &mut QniApp, ctx: &egui::Context) {
        app.hovered_gate_id = None;
        app.hovered_qft_resize_handle = None;
        app.hovered_palette_index = None;
        if app.hovered_step.take().is_some() {
            app.gpu_plan.mark_dirty();
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
            Self::clear_idle_hover(app, ctx);
        }
    }

    pub(in crate::app) fn set_cursor_icon(app: &QniApp, pointer: DragPointer, ctx: &egui::Context) {
        if app.dragging.is_some() && pointer.down {
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if app.hovered_qft_resize_handle.is_some() {
            ctx.set_cursor_icon(egui::CursorIcon::ResizeVertical);
        } else if app.hovered_gate_id.is_some() || app.hovered_palette_index.is_some() {
            ctx.set_cursor_icon(egui::CursorIcon::Grab);
        }
    }
}
