use eframe::egui;

use crate::app::QniApp;
use crate::constants::ASPECT_WHEEL_PER_STEP;

impl QniApp {
    /// Aspect dims text (right side of the strip): wheel accumulates
    /// into `aspect_wheel_accum` and steps the aspect each time the
    /// sum crosses ±`ASPECT_WHEEL_PER_STEP`; click toggles the popover.
    pub(crate) fn process_aspect_dims(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        aspect_qubits: usize,
        dims_hit: egui::Rect,
    ) {
        let dims_resp = ui.interact(dims_hit, egui::Id::new("state_dims"), egui::Sense::click());
        if dims_resp.hovered() {
            let plain_scroll = ctx.input(|i| {
                if i.modifiers.ctrl || i.modifiers.command {
                    0.0
                } else {
                    i.smooth_scroll_delta.y
                }
            });
            if plain_scroll.abs() > f32::EPSILON {
                self.state_panel.aspect_wheel_accum += plain_scroll;
                let mut steps: i32 = 0;
                while self.state_panel.aspect_wheel_accum >= ASPECT_WHEEL_PER_STEP {
                    self.state_panel.aspect_wheel_accum -= ASPECT_WHEEL_PER_STEP;
                    steps -= 1; // positive scroll → taller (cols −1)
                }
                while self.state_panel.aspect_wheel_accum <= -ASPECT_WHEEL_PER_STEP {
                    self.state_panel.aspect_wheel_accum += ASPECT_WHEEL_PER_STEP;
                    steps += 1; // negative scroll → wider (cols +1)
                }
                if steps != 0 {
                    let new_aspect = (self.state_panel.aspect_index as i32 + steps)
                        .clamp(0, aspect_qubits as i32)
                        as usize;
                    if new_aspect != self.state_panel.aspect_index {
                        self.state_panel.aspect_index = new_aspect;
                        self.state_panel.aspect_customized = true;
                        ctx.request_repaint();
                    }
                }
            } else {
                // Wheel stopped this frame — drop any sub-step residue
                // so the next notch starts from zero.
                self.state_panel.aspect_wheel_accum = 0.0;
            }
            ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
        } else {
            // Pointer left the dims area — discard pending accum so
            // re-entering doesn't fire a stale step.
            self.state_panel.aspect_wheel_accum = 0.0;
        }
        if dims_resp.clicked() {
            self.state_panel.aspect_popover_open = !self.state_panel.aspect_popover_open;
        }
    }

    /// Aspect popover row clicks + outside-click / ESC close.
    pub(crate) fn process_aspect_popover(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        aspect_qubits: usize,
        dims_hit: egui::Rect,
        panel_rect: egui::Rect,
    ) {
        if self.state_panel.aspect_popover_open {
            let (popover_rect, row_rects) =
                QniApp::aspect_popover_layout(panel_rect, dims_hit, aspect_qubits);
            for (i, row_rect) in row_rects.iter().enumerate() {
                let resp = ui.interact(
                    *row_rect,
                    egui::Id::new(("state_aspect_row", i)),
                    egui::Sense::click(),
                );
                if resp.hovered() {
                    ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    self.state_panel.aspect_index = i;
                    self.state_panel.aspect_customized = true;
                    self.state_panel.aspect_popover_open = false;
                }
            }
            // Outside click closes. `any_pressed` catches a click that
            // initiated outside the popover this frame.
            let pressed = ctx.input(|i| i.pointer.any_pressed());
            if pressed {
                if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                    if !dims_hit.contains(pos) && !popover_rect.contains(pos) {
                        self.state_panel.aspect_popover_open = false;
                    }
                }
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && self.state_panel.aspect_popover_open {
            self.state_panel.aspect_popover_open = false;
        }
    }
}
