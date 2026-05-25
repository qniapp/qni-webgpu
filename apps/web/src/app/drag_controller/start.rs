use eframe::egui;

use super::{step_at_cursor, CircuitInputGeometry, DragController, DragPointer};
use crate::app::{DragState, PlacedGate, QniApp, SpanResizeDrag};
use crate::constants::GATE_SIZE;
use crate::gates::palette_gate_kind;
use crate::layout::{gate_visible_rect, palette_hit_test};
use crate::span_resize::SpanResizeHandles;

#[derive(Clone, Copy, Debug)]
enum DragStartIntent {
    SpanResize(SpanResizeDrag),
    ExistingGate {
        drag: DragState,
        starts_live_display_snap: bool,
    },
    PaletteGate {
        index: usize,
        preview_pos: egui::Pos2,
    },
    BreakpointStep(usize),
    None,
}

impl DragController {
    pub(in crate::app) fn handle_pointer_start(
        app: &mut QniApp,
        pointer: DragPointer,
        geometry: &CircuitInputGeometry,
        ctx: &egui::Context,
    ) -> bool {
        match start_intent(app, pointer, geometry) {
            DragStartIntent::SpanResize(resize) => {
                if app.library.active_locked() {
                    return false;
                }
                app.begin_circuit_commit();
                app.selected_gate_id = Some(resize.gate_id);
                app.span_resize_drag = Some(resize);
                app.hovered_gate_id = None;
                app.hovered_probability_outcome = None;
                app.hovered_amplitude_outcome = None;
                app.hovered_density_cell = None;
                app.hovered_palette_index = None;
                ctx.request_repaint();
                true
            }
            DragStartIntent::ExistingGate {
                drag,
                starts_live_display_snap,
            } => {
                if app.library.active_locked() {
                    return false;
                }
                app.begin_circuit_commit();
                app.selected_gate_id = Some(drag.id);
                app.dragging = Some(drag);
                app.dragging_live_display_snap = starts_live_display_snap;
                app.dragging_live_display_plan_touched = false;
                app.drag_state_count = Some(app.state_count());
                app.drag_cursor_pos = pointer.local_pos;
                app.hovered_gate_id = None;
                app.hovered_probability_outcome = None;
                app.hovered_amplitude_outcome = None;
                app.hovered_density_cell = None;
                app.hovered_palette_index = None;
                ctx.request_repaint();
                true
            }
            DragStartIntent::PaletteGate { index, preview_pos } => {
                if app.library.active_locked() {
                    return false;
                }
                app.begin_circuit_commit();
                let new_id = app.next_gate_id;
                let Some(kind) = palette_gate_kind(index) else {
                    return false;
                };
                let mut new_gate = PlacedGate::new(
                    new_id, kind, 0, 0, 1,
                    // Palette drop: no explicit angle yet — Phase falls
                    // back to its π/2 default until a future angle picker
                    // lets the user set one.
                    None,
                );
                new_gate.pos = preview_pos;
                app.next_gate_id += 1;
                app.placed_gates.push(new_gate);
                app.selected_gate_id = Some(new_id);
                app.dragging = Some(DragState {
                    id: new_id,
                    offset: egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0),
                    original_column: None,
                });
                app.dragging_live_display_snap = false;
                app.dragging_live_display_plan_touched = false;
                app.drag_state_count = Some(app.state_count());
                app.drag_cursor_pos = pointer.local_pos;
                app.hovered_palette_index = None;
                app.hovered_gate_id = None;
                app.hovered_probability_outcome = None;
                app.hovered_amplitude_outcome = None;
                app.hovered_density_cell = None;
                ctx.request_repaint();
                true
            }
            DragStartIntent::BreakpointStep(step) => {
                let changed_selection = app.selected_gate_id.take().is_some();
                if app.breakpoint_step != Some(step) {
                    app.breakpoint_step = Some(step);
                    app.gpu_plan.mark_step_preview_dirty();
                    ctx.request_repaint();
                } else if changed_selection {
                    ctx.request_repaint();
                }
                true
            }
            DragStartIntent::None => {
                if app.selected_gate_id.take().is_some() {
                    ctx.request_repaint();
                    return true;
                }
                false
            }
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

    // Resizable-span handles take priority over gate bodies for press events,
    // so dragging the affordance resizes the span instead of picking up the
    // whole gate.
    if let Some(resize) = app
        .placed_gates
        .iter()
        .rev()
        .filter_map(|gate| SpanResizeHandles::for_gate(gate)?.drag_at(gate, cursor))
        .next()
    {
        return DragStartIntent::SpanResize(resize);
    }

    if let Some(drag) = app
        .placed_gates
        .iter()
        .rev()
        .find(|gate| {
            let gate_rect = gate_visible_rect(gate, gate.pos);
            gate_rect.contains(cursor)
        })
        .map(|gate| DragStartIntent::ExistingGate {
            drag: DragState {
                id: gate.id,
                offset: cursor - gate.pos,
                original_column: Some(gate.column),
            },
            starts_live_display_snap: matches!(
                gate.kind,
                crate::gates::GateKind::AmplitudeDisplay
                    | crate::gates::GateKind::DensityMatrixDisplay
                    | crate::gates::GateKind::BlochDisplay
                    | crate::gates::GateKind::Measurement
            ),
        })
    {
        return drag;
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
