//! Gate input — egui pointer adapter for `DragController` plus the
//! drag-time repaint throttle. Independent of the state panel.

use eframe::egui;
use std::time::Duration;

use super::drag_controller::{CircuitInputGeometry, DragController, DragPointer};
use super::QniApp;
use crate::constants::{
    DRAG_REPAINT_BASE_SECS, DRAG_REPAINT_MAX_SECS, DRAG_REPAINT_MIN_SECS, DRAG_REPAINT_PUMP_FACTOR,
};
use crate::shared::now_seconds;

impl QniApp {
    pub(crate) fn handle_input(
        &mut self,
        content_rect: egui::Rect,
        ctx: &egui::Context,
        screen_rect: egui::Rect,
        pointer_over_state_panel: bool,
    ) {
        let pointer = ctx.input(|input| input.pointer.clone());
        let pos = pointer.latest_pos();
        let pointer_down = pointer.primary_down();
        let pointer_pressed = pointer.primary_pressed();
        // An earlier release in this frame must not drop a newer held press.
        let pointer_released = pointer.primary_released() && !pointer_down;

        let pointer_start = pointer_pressed || (pointer_down && !self.pointer_was_down);
        self.pointer_was_down = pointer_down;
        if self.dragging.is_some() && ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            // Undo while a gesture is in progress restores its checkpoint
            // without moving back through completed edits. Keep the fixed
            // column selection, which the circuit reload otherwise clears.
            let breakpoint_step = self.breakpoint_step;
            self.undo_circuit(ctx);
            self.breakpoint_step = breakpoint_step;
            return;
        }
        // `local_pos` is the cursor in *circuit space* — the same
        // coordinate frame `gate.pos` lives in. We undo the horizontal
        // scroll here once so every downstream hit-test (gate body,
        // slot snap, step preview, palette drop on circuit) gets a
        // cursor it can compare directly against `gate.pos.x`.
        // Palette pickup itself uses `pos` (screen) so it isn't
        // affected by the offset.
        let local_pos = pos.map(|p| {
            egui::pos2(
                p.x - content_rect.min.x + self.circuit_scroll_x,
                p.y - content_rect.min.y,
            )
        });
        let geometry = CircuitInputGeometry::new(
            content_rect,
            screen_rect,
            self.layout_qubits(),
            self.min_circuit_slots(),
        );
        let drag_pointer = DragPointer {
            screen_pos: pos,
            local_pos,
            down: pointer_down,
            start: pointer_start,
            released: pointer_released,
            shift_at_start: None,
        };

        let angle_label_gate_id = (!self.library.active_locked())
            .then(|| {
                self.angle_label_at_local_pos(
                    local_pos,
                    &geometry.metrics,
                    self.dragging.map(|drag| drag.id),
                )
            })
            .flatten();
        if let Some(affordance) = self.angle_affordance {
            if drag_pointer.start
                && affordance.open_editor_after_delay
                && angle_label_gate_id != Some(affordance.gate_id)
            {
                self.angle_affordance = None;
            }
        }
        if drag_pointer.start && self.angle_editor.is_some() {
            let active_editor_gate = self.angle_editor.as_ref().map(|editor| editor.gate_id);
            if angle_label_gate_id != active_editor_gate {
                if let Some(editor) = self.angle_editor.as_mut() {
                    editor.commit_after_frame = true;
                }
            }
        }

        let pointer_over_picker = self
            .picker_overlay_rect
            .is_some_and(|rect| pos.is_some_and(|pos| rect.contains(pos)));
        if (pointer_over_picker || pointer_over_state_panel)
            && self.dragging.is_none()
            && self.span_resize_drag.is_none()
        {
            DragController::clear_idle_hover(self, ctx);
            return;
        }

        DragController::update_circuit_scroll(
            self,
            ctx,
            content_rect,
            drag_pointer.screen_pos,
            &geometry.metrics,
        );
        if self.angle_editor.is_some() && self.dragging.is_none() && self.span_resize_drag.is_none()
        {
            if drag_pointer.start {
                let active_editor_gate = self.angle_editor.as_ref().map(|editor| editor.gate_id);
                if angle_label_gate_id != active_editor_gate {
                    if let Some(editor) = self.angle_editor.as_mut() {
                        editor.commit_after_frame = true;
                    }
                }
            }
            DragController::clear_idle_hover(self, ctx);
            if angle_label_gate_id.is_some() {
                ctx.set_cursor_icon(egui::CursorIcon::Text);
            }
            return;
        }
        if angle_label_gate_id.is_some()
            && self.dragging.is_none()
            && self.span_resize_drag.is_none()
        {
            DragController::clear_idle_hover(self, ctx);
            ctx.set_cursor_icon(egui::CursorIcon::Text);
            return;
        }

        self.clear_click_copy_after_motion(ctx, false);
        let press = ctx.input(|input| {
            input.events.iter().rev().find_map(|event| match event {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers,
                } => Some((*pos, modifiers.shift)),
                _ => None,
            })
        });
        let press_pos = press.map(|(pos, _)| pos);
        let start_pointer = DragPointer {
            shift_at_start: press.map(|(_, shift)| shift),
            screen_pos: press_pos.or(drag_pointer.screen_pos),
            local_pos: press_pos
                .map(|p| {
                    egui::pos2(
                        p.x - content_rect.min.x + self.circuit_scroll_x,
                        p.y - content_rect.min.y,
                    )
                })
                .or(drag_pointer.local_pos),
            ..drag_pointer
        };
        if drag_pointer.start
            && self.dragging.is_none()
            && self.span_resize_drag.is_none()
            && DragController::handle_pointer_start(self, start_pointer, &geometry, ctx)
        {
            self.clear_click_copy_after_motion(ctx, true);
            if self.dragging.is_some() {
                DragController::update_gate_drag_preview(self, drag_pointer, &geometry.metrics);
            }
            if !drag_pointer.released {
                return;
            }
        }

        // Active resizable-span drag → update span from total Δy and skip
        // the rest of the input pipeline (gate drag, hover) for this frame.
        if DragController::update_active_span_resize(self, drag_pointer, ctx) {
            return;
        }

        if self.dragging.is_some() {
            DragController::update_gate_drag_preview(self, drag_pointer, &geometry.metrics);
        } else {
            DragController::update_idle_hover(self, drag_pointer, &geometry, ctx);
        }

        DragController::commit_gate_drop(self, drag_pointer, &geometry.metrics, ctx);
        DragController::set_cursor_icon(self, drag_pointer, ctx);
    }

    fn clear_click_copy_after_motion(&mut self, ctx: &egui::Context, just_started: bool) {
        let Some(click) = self.dragging.and_then(|drag| drag.click_copy) else {
            return;
        };
        let crossed = ctx.input(|input| {
            let events = &input.events;
            let start = if just_started {
                events.iter().rposition(|event| matches!(event, egui::Event::PointerButton { button: egui::PointerButton::Primary, pressed: true, .. })).map_or(0, |index| index + 1)
            } else { 0 };
            events[start..].iter().any(|event| matches!(event, egui::Event::PointerMoved(pos) if pos.distance(click.press_pos) > click.max_distance))
        });
        if crossed {
            if let Some(drag) = self.dragging.as_mut() {
                drag.click_copy = None;
            }
        }
    }

    pub(crate) fn schedule_drag_repaint(&mut self, ctx: &egui::Context, frame_secs: f64) {
        let now = now_seconds();
        let deadline = self.drag_repaint_deadline.unwrap_or(now);
        if now >= deadline {
            let delay = (DRAG_REPAINT_BASE_SECS + frame_secs * DRAG_REPAINT_PUMP_FACTOR)
                .clamp(DRAG_REPAINT_MIN_SECS, DRAG_REPAINT_MAX_SECS);
            self.drag_repaint_deadline = Some(now + delay);
            self.drag_repaint_pending = false;
            ctx.request_repaint();
        } else if !self.drag_repaint_pending {
            self.drag_repaint_pending = true;
            let remaining = (deadline - now).max(0.0);
            ctx.request_repaint_after(Duration::from_secs_f64(remaining));
        }
    }
}
