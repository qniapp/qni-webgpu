use eframe::egui;

use super::pan::CircuitPan;
use super::{step_at_cursor, CircuitInputGeometry, DragController, DragPointer};
use crate::app::{
    CircuitBlockDrag, CircuitBlockEdge, CircuitBlockLabelSlot, CircuitColumnIndex, ClickCopy,
    DragState, LiveDragSnap, PlacedGate, QniApp, SpanResizeDrag, WireIndex,
};
use crate::constants::GATE_SIZE;
use crate::gates::GateSpan;
use crate::layout::{gate_visible_rect, gate_width_cols, CarouselHit, PaletteItem};
use crate::span_resize::SpanResizeHandles;

#[derive(Clone, Copy, Debug)]
enum DragStartIntent {
    CircuitBlockResize {
        index: usize,
        edge: CircuitBlockEdge,
    },
    SpanResize(SpanResizeDrag),
    ExistingGate {
        drag: DragState,
        starts_live_display_snap: bool,
    },
    CircuitBlockLabel {
        index: usize,
        slot: CircuitBlockLabelSlot,
    },
    PaletteGate {
        index: usize,
        preview_pos: egui::Pos2,
    },
    BreakpointStep(CircuitColumnIndex),
    PalettePage(usize),
    Pan(Option<CircuitColumnIndex>),
    None,
}

fn copy_click_target(gate: &PlacedGate, cursor: egui::Pos2) -> Option<LiveDragSnap> {
    let left = cursor.x < gate_visible_rect(gate, gate.committed_pos()).center().x;
    let column = if left {
        gate.column
    } else {
        gate.column
            .checked_add(gate_width_cols(gate.kind, gate.span.get()))?
    };
    // A click-copy previews at the adjacent column boundary, even when
    // the neighboring cell is empty. The column is inserted on release.
    Some(LiveDragSnap::Insert {
        column,
        wire: gate.wire,
    })
}

impl DragController {
    pub(in crate::app) fn handle_pointer_start(
        app: &mut QniApp,
        pointer: DragPointer,
        geometry: &CircuitInputGeometry,
        ctx: &egui::Context,
    ) -> bool {
        match start_intent(app, pointer, geometry, ctx) {
            DragStartIntent::CircuitBlockResize { index, edge } => {
                app.begin_circuit_commit();
                app.circuit_block_drag = Some(CircuitBlockDrag::Resize { index, edge });
                Self::clear_idle_hover(app, ctx);
                ctx.request_repaint();
                true
            }
            DragStartIntent::CircuitBlockLabel { index, slot } => {
                let Some(cursor) = pointer.local_pos else {
                    return false;
                };
                app.begin_circuit_commit();
                Self::start_circuit_block_label_drag(app, index, slot, cursor, &geometry.metrics);
                Self::clear_idle_hover(app, ctx);
                ctx.request_repaint();
                true
            }
            DragStartIntent::SpanResize(resize) => {
                if app.library.active_locked() {
                    return false;
                }
                app.begin_circuit_commit();
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
                mut drag,
                starts_live_display_snap,
            } => {
                if app.library.active_locked() {
                    return false;
                }
                app.begin_circuit_commit();
                // Latch duplication at pickup; releasing Shift during the
                // gesture must not turn the copy into a move of its source.
                if pointer
                    .shift_at_start
                    .unwrap_or_else(|| ctx.input(|input| input.modifiers.shift))
                {
                    let Some(mut copy) = app
                        .placed_gates
                        .iter()
                        .find(|gate| gate.id == drag.id)
                        .cloned()
                    else {
                        return false;
                    };
                    copy.id = app.gate_ids.allocate();
                    drag.click_copy = copy_click_target(&copy, copy.committed_pos() + drag.offset)
                        .map(|target| ClickCopy {
                            target,
                            press_pos: pointer
                                .screen_pos
                                .unwrap_or(copy.committed_pos() + drag.offset),
                            max_distance: ctx
                                .options(|options| options.input_options.max_click_dist),
                        });
                    drag.id = copy.id;
                    drag.original_column = None;
                    app.placed_gates.push(copy);
                }
                app.dragging_live_snap = app.placed_gates.iter().find_map(|gate| {
                    (gate.id == drag.id).then_some(LiveDragSnap::Slot {
                        column: gate.column,
                        wire: gate.wire,
                    })
                });
                app.dragging = Some(drag);
                app.dragging_live_display_snap = starts_live_display_snap;
                app.dragging_live_gpu_plan_touched = false;
                app.drag_state_count = Some(app.state_count());
                app.drag_cursor_pos = pointer.local_pos;
                app.hovered_gate_id = None;
                app.hovered_probability_outcome = None;
                app.hovered_amplitude_outcome = None;
                app.hovered_density_cell = None;
                app.hovered_palette_index = None;
                if drag.click_copy.is_some() {
                    Self::update_gate_drag_preview(app, pointer, &geometry.metrics);
                }
                ctx.request_repaint();
                true
            }
            DragStartIntent::PaletteGate { index, preview_pos } => {
                if app.library.active_locked() {
                    return false;
                }
                let entry = match app.palette.item(index) {
                    Some(PaletteItem::Gate(entry)) => entry,
                    Some(PaletteItem::CircuitBlock) => {
                        let Some(cursor) = pointer.local_pos else {
                            return false;
                        };
                        app.begin_circuit_commit();
                        Self::start_circuit_block_palette_drag(app, cursor, &geometry.metrics);
                        Self::clear_idle_hover(app, ctx);
                        ctx.request_repaint();
                        return true;
                    }
                    None => return false,
                };
                app.begin_circuit_commit();
                let new_id = app.gate_ids.allocate();
                let new_gate = PlacedGate::new(
                    new_id,
                    entry.kind,
                    CircuitColumnIndex::ZERO,
                    WireIndex::ZERO,
                    GateSpan::SINGLE,
                    // Palette drop: parametric gates start with their explicit
                    // angle (π/2 unless the embed palette names one) so the
                    // circuit shows the same angle label Qni shows.
                    entry.angle,
                );
                app.placed_gates.push(new_gate);
                app.dragging = Some(DragState {
                    id: new_id,
                    preview_pos,
                    offset: egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0),
                    original_column: None,
                    click_copy: None,
                });
                app.dragging_live_snap = None;
                app.dragging_live_display_snap = false;
                app.dragging_live_gpu_plan_touched = false;
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
                if app.breakpoint_step != Some(step) {
                    app.lock_step(step, ctx);
                }
                true
            }
            DragStartIntent::PalettePage(page) => {
                if app.palette_page != page {
                    app.palette_page = page;
                    app.hovered_palette_index = None;
                    ctx.request_repaint();
                }
                true
            }
            DragStartIntent::Pan(step) => {
                if let Some(pos) = pointer.screen_pos {
                    app.circuit_pan = Some(CircuitPan::new(pos, step));
                }
                true
            }
            DragStartIntent::None => false,
        }
    }
}

fn start_intent(
    app: &QniApp,
    pointer: DragPointer,
    geometry: &CircuitInputGeometry,
    ctx: &egui::Context,
) -> DragStartIntent {
    let Some(cursor) = pointer.local_pos else {
        return DragStartIntent::None;
    };
    // The vertical layout's carousel sits over the circuit area's
    // extension, so it takes presses before any gate.
    if let Some((carousel, hit)) = pointer
        .screen_pos
        .and_then(|pos| geometry.carousel_hit(app, pos))
    {
        return match hit {
            CarouselHit::Gate(index) => DragStartIntent::PaletteGate {
                index,
                preview_pos: egui::pos2(cursor.x - GATE_SIZE / 2.0, cursor.y - GATE_SIZE / 2.0),
            },
            _ => DragStartIntent::PalettePage(carousel.page_after(app.palette_page, hit)),
        };
    }
    if !geometry.circuit_hit_allowed(pointer.screen_pos) {
        return DragStartIntent::None;
    }

    let block_hover = app
        .circuit_block_editing_allowed()
        .then(|| app.circuit_block_hover_at(cursor, &geometry.metrics, ctx))
        .flatten();
    if let Some((index, edge)) =
        block_hover.and_then(|hover| hover.handle.map(|edge| (hover.index, edge)))
    {
        return DragStartIntent::CircuitBlockResize { index, edge };
    }

    // Resizable-span handles take priority over gate bodies for press events,
    // so dragging the affordance resizes the span instead of picking up the
    // whole gate.
    if let Some(resize) = app
        .placed_gates
        .iter()
        .rev()
        .filter(|_| !geometry.is_vertical())
        .filter_map(|gate| {
            SpanResizeHandles::for_gate_with_availability(
                gate,
                &app.placed_gates,
                app.wire_capacity().get(),
            )?
            .drag_at(gate, cursor)
        })
        .next()
    {
        return DragStartIntent::SpanResize(resize);
    }

    if let Some(drag) = app
        .placed_gates
        .iter()
        .rev()
        .find(|gate| {
            let gate_rect = gate_visible_rect(gate, gate.committed_pos());
            gate_rect.contains(cursor)
        })
        .map(|gate| DragStartIntent::ExistingGate {
            drag: DragState {
                id: gate.id,
                preview_pos: gate.committed_pos(),
                offset: cursor - gate.committed_pos(),
                original_column: Some(gate.column),
                click_copy: None,
            },
            starts_live_display_snap: gate.kind.is_sampled_measurement()
                || matches!(
                    gate.kind,
                    crate::gates::GateKind::SampleDisplay
                        | crate::gates::GateKind::AmplitudeDisplay
                        | crate::gates::GateKind::DensityMatrixDisplay
                        | crate::gates::GateKind::BlochDisplay
                ),
        })
    {
        return drag;
    }

    if let Some((index, slot)) =
        block_hover.and_then(|hover| hover.label.map(|slot| (hover.index, slot)))
    {
        return DragStartIntent::CircuitBlockLabel { index, slot };
    }

    if let Some(index) = pointer
        .screen_pos
        .and_then(|pos| geometry.pressed_palette_entry(app, pos))
    {
        return DragStartIntent::PaletteGate {
            index,
            preview_pos: egui::pos2(cursor.x - GATE_SIZE / 2.0, cursor.y - GATE_SIZE / 2.0),
        };
    }

    // No gate / palette under the cursor. If we're inside a step slot,
    // lock the breakpoint to that column. The vertical layout pans the
    // circuit from empty space instead and selects the step on release
    // only when the pointer did not move.
    let step = step_at_cursor(cursor, &geometry.metrics);
    if geometry.is_vertical() {
        return DragStartIntent::Pan(step);
    }
    step.map(DragStartIntent::BreakpointStep)
        .unwrap_or(DragStartIntent::None)
}
