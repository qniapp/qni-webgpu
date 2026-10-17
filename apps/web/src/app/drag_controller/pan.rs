use eframe::egui;

use super::{DragController, DragPointer};
use crate::app::{CircuitColumnIndex, QniApp};

/// A press on empty circuit space in the vertical layout. Moving past the
/// click distance pans the circuit along its time axis; releasing without
/// moving selects the step under the press.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CircuitPan {
    press: egui::Pos2,
    last: egui::Pos2,
    step: Option<CircuitColumnIndex>,
    panning: bool,
}

impl CircuitPan {
    pub(in crate::app) fn new(press: egui::Pos2, step: Option<CircuitColumnIndex>) -> Self {
        Self {
            press,
            last: press,
            step,
            panning: false,
        }
    }
}

impl DragController {
    /// Advances an in-flight pan; `false` when there is none.
    pub(in crate::app) fn update_circuit_pan(
        app: &mut QniApp,
        pointer: DragPointer,
        max_scroll: f32,
        ctx: &egui::Context,
    ) -> bool {
        let Some(mut pan) = app.circuit_pan else {
            return false;
        };
        if let Some(pos) = pointer.screen_pos {
            let max_click_dist = ctx.options(|options| options.input_options.max_click_dist);
            let was_panning = pan.panning;
            pan.panning |= pos.distance(pan.press) > max_click_dist;
            if pan.panning {
                // Count the movement inside the click distance too, so the
                // circuit stays under the finger.
                let from = if was_panning { pan.last } else { pan.press };
                app.circuit_scroll_time =
                    (app.circuit_scroll_time - (pos.y - from.y)).clamp(0.0, max_scroll);
                ctx.request_repaint();
            }
            pan.last = pos;
        }
        if pointer.down && !pointer.released {
            app.circuit_pan = Some(pan);
            return true;
        }
        app.circuit_pan = None;
        if let Some(step) = pan.step.filter(|_| !pan.panning) {
            if app.breakpoint_step != Some(step) {
                app.lock_step(step, ctx);
            }
        }
        true
    }
}
