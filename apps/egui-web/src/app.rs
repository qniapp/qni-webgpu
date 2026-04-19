use eframe::egui;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

use crate::gates::{gate_params, gate_params_controlled, GateKind, GateParams};
use crate::layout::{
    layout_metrics, nearest_available_slot, nearest_line, nearest_slot_index, LayoutMetrics,
};
use crate::render::StateInstanceCache;
use crate::{
    now_seconds, Colors, DRAG_REPAINT_BASE_SECS, DRAG_REPAINT_MAX_SECS, DRAG_REPAINT_MIN_SECS,
    DRAG_REPAINT_PUMP_FACTOR, GATE_SIZE, MAX_QUBITS, MIN_QUBITS, PALETTE_GAP, PALETTE_GATES,
    PALETTE_ROW_Y, PALETTE_SIZE, SNAP_DISTANCE,
};

#[derive(Clone, Debug)]
pub(crate) struct PlacedGate {
    pub(crate) id: u32,
    pub(crate) kind: GateKind,
    pub(crate) pos: egui::Pos2,
    pub(crate) wire: usize,
}

#[derive(Clone, Copy, Debug)]
struct DragState {
    id: u32,
    offset: egui::Vec2,
}

pub(crate) struct QniApp {
    next_gate_id: u32,
    pub(crate) placed_gates: Vec<PlacedGate>,
    dragging: Option<DragState>,
    drag_state_count: Option<usize>,
    state_panel_drag: Option<egui::Vec2>,
    pub(crate) state_panel_offset: egui::Vec2,
    pub(crate) hovered_gate_id: Option<u32>,
    pub(crate) hovered_palette_index: Option<usize>,
    qubit_count: usize,
    last_state_count: usize,
    needs_recompute: bool,
    last_content_rect: Option<egui::Rect>,
    drag_cursor_pos: Option<egui::Pos2>,
    pub(crate) state_instance_cache: Option<StateInstanceCache>,
    drag_repaint_deadline: Option<f64>,
    drag_repaint_pending: bool,
    startup_repaint_until: f64,
    pointer_was_down: bool,
}

impl QniApp {
    pub(crate) fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        cc.egui_ctx.style_mut(|style| {
            style.spacing.window_margin = egui::Margin::same(0);
        });
        cc.egui_ctx.request_repaint();
        Self {
            next_gate_id: 1,
            placed_gates: Vec::new(),
            dragging: None,
            drag_state_count: None,
            state_panel_drag: None,
            state_panel_offset: egui::Vec2::ZERO,
            hovered_gate_id: None,
            hovered_palette_index: None,
            qubit_count: MIN_QUBITS,
            last_state_count: 2,
            needs_recompute: true,
            last_content_rect: None,
            drag_cursor_pos: None,
            state_instance_cache: None,
            drag_repaint_deadline: None,
            drag_repaint_pending: false,
            startup_repaint_until: now_seconds() + 0.5,
            pointer_was_down: false,
        }
    }

    fn layout_qubits(&self) -> usize {
        let mut count = self.qubit_count.clamp(MIN_QUBITS, MAX_QUBITS);
        if self.dragging.is_some() && count < MAX_QUBITS {
            count += 1;
        }
        count
    }

    fn state_qubits(&self) -> usize {
        let mut max_wire: Option<usize> = None;
        for gate in &self.placed_gates {
            max_wire = Some(match max_wire {
                Some(current) => current.max(gate.wire),
                None => gate.wire,
            });
        }
        let count = max_wire.map_or(1, |wire| wire + 1);
        count.clamp(1, MAX_QUBITS)
    }

    fn update_qubit_count(&mut self) {
        let mut max_wire = MIN_QUBITS - 1;
        for gate in &self.placed_gates {
            max_wire = max_wire.max(gate.wire);
        }
        self.qubit_count = (max_wire + 1).clamp(MIN_QUBITS, MAX_QUBITS);
    }

    fn state_count(&self) -> usize {
        1usize << self.state_qubits()
    }

    pub(crate) fn collect_gate_params(
        &self,
        qubits: usize,
        state_count: usize,
        metrics: &LayoutMetrics,
    ) -> Vec<GateParams> {
        let mut gates: Vec<&PlacedGate> = self.placed_gates.iter().collect();
        gates.sort_by(|a, b| {
            a.pos
                .x
                .partial_cmp(&b.pos.x)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.id.cmp(&b.id))
        });

        struct GateGroup<'a> {
            controls: Vec<&'a PlacedGate>,
            targets: Vec<&'a PlacedGate>,
            slot_x: f32,
            min_id: u32,
        }

        let mut groups: HashMap<usize, GateGroup<'_>> = HashMap::new();
        for gate in &gates {
            let center_x = gate.pos.x + GATE_SIZE / 2.0;
            let Some((slot_index, distance)) = nearest_slot_index(center_x, &metrics.slot_centers)
            else {
                continue;
            };
            if distance > SNAP_DISTANCE {
                continue;
            }
            let entry = groups.entry(slot_index).or_insert_with(|| GateGroup {
                controls: Vec::new(),
                targets: Vec::new(),
                slot_x: metrics.slot_centers[slot_index],
                min_id: gate.id,
            });
            entry.min_id = entry.min_id.min(gate.id);
            if gate.kind == GateKind::Control {
                entry.controls.push(*gate);
            } else {
                entry.targets.push(*gate);
            }
        }

        let mut used_ids = HashSet::new();
        let mut ops: Vec<(f32, u32, GateParams)> = Vec::new();
        for group in groups.values() {
            let mut control_mask = 0u32;
            let mut control_value = 0u32;
            for control in &group.controls {
                if control.wire >= qubits {
                    continue;
                }
                let control_bit = (qubits.saturating_sub(1).saturating_sub(control.wire)) as u32;
                let bit_mask = 1u32 << control_bit;
                control_mask |= bit_mask;
                control_value |= bit_mask;
                used_ids.insert(control.id);
            }
            for target in &group.targets {
                if target.wire >= qubits {
                    continue;
                }
                if target.kind == GateKind::Swap {
                    continue;
                }
                let bit = (qubits.saturating_sub(1).saturating_sub(target.wire)) as u32;
                let params = if control_mask == 0 {
                    gate_params(target.kind, bit, state_count as u32)
                } else {
                    gate_params_controlled(
                        target.kind,
                        bit,
                        control_mask,
                        control_value,
                        state_count as u32,
                    )
                };
                ops.push((group.slot_x, group.min_id.min(target.id), params));
                used_ids.insert(target.id);
            }
        }

        for gate in gates {
            if gate.wire >= qubits {
                continue;
            }
            if gate.kind == GateKind::Swap {
                continue;
            }
            if gate.kind == GateKind::Control {
                continue;
            }
            if used_ids.contains(&gate.id) {
                continue;
            }
            let bit = (qubits.saturating_sub(1).saturating_sub(gate.wire)) as u32;
            ops.push((gate.pos.x, gate.id, gate_params(gate.kind, bit, state_count as u32)));
        }

        ops.sort_by(|a, b| {
            a.0.partial_cmp(&b.0)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.1.cmp(&b.1))
        });
        ops.into_iter().map(|(_, _, params)| params).collect()
    }

    fn handle_input(
        &mut self,
        content_rect: egui::Rect,
        ctx: &egui::Context,
        screen_rect: egui::Rect,
    ) {
        let pointer = ctx.input(|input| input.pointer.clone());
        let pos = pointer.latest_pos();
        let pointer_down = pointer.primary_down();
        let pointer_pressed = pointer.primary_pressed();
        let pointer_released = pointer.primary_released();

        let pointer_start = pointer_pressed || (pointer_down && !self.pointer_was_down);
        self.pointer_was_down = pointer_down;
        let local_pos = pos.map(|p| egui::pos2(p.x - content_rect.min.x, p.y - content_rect.min.y));
        let palette_width = PALETTE_GATES.len() as f32 * PALETTE_SIZE
            + (PALETTE_GATES.len() as f32 - 1.0) * PALETTE_GAP;
        let palette_start_x = screen_rect.width() / 2.0 - palette_width / 2.0;
        let palette_rect = egui::Rect::from_min_size(
            egui::pos2(
                screen_rect.min.x + palette_start_x,
                screen_rect.min.y + PALETTE_ROW_Y,
            ),
            egui::vec2(palette_width, PALETTE_SIZE),
        );
        let metrics = layout_metrics(content_rect.width(), self.layout_qubits());

        if pointer_start {
            if let Some(cursor) = local_pos {
                if let Some((gate_id, offset)) = self
                    .placed_gates
                    .iter()
                    .rev()
                    .find(|gate| {
                        let gate_rect =
                            egui::Rect::from_min_size(gate.pos, egui::vec2(GATE_SIZE, GATE_SIZE));
                        gate_rect.contains(cursor)
                    })
                    .map(|gate| (gate.id, cursor - gate.pos))
                {
                    self.dragging = Some(DragState { id: gate_id, offset });
                    self.drag_state_count = Some(self.state_count());
                    self.drag_cursor_pos = Some(cursor);
                    ctx.request_repaint();
                    self.hovered_gate_id = None;
                    self.hovered_palette_index = None;
                    return;
                }

                if let Some(cursor_screen) = pos {
                    if palette_rect.contains(cursor_screen) {
                        let local_x = cursor_screen.x - (screen_rect.min.x + palette_start_x);
                        let index = (local_x / (PALETTE_SIZE + PALETTE_GAP)).floor() as i32;
                        if index >= 0 && (index as usize) < PALETTE_GATES.len() {
                            let in_box = local_x - index as f32 * (PALETTE_SIZE + PALETTE_GAP)
                                <= PALETTE_SIZE;
                            if in_box {
                                let new_id = self.next_gate_id;
                                let new_gate = PlacedGate {
                                    id: new_id,
                                    kind: PALETTE_GATES[index as usize],
                                    pos: egui::pos2(
                                        cursor.x - GATE_SIZE / 2.0,
                                        cursor.y - GATE_SIZE / 2.0,
                                    ),
                                    wire: 0,
                                };
                                self.next_gate_id += 1;
                                self.placed_gates.push(new_gate);
                                self.dragging = Some(DragState {
                                    id: new_id,
                                    offset: egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0),
                                });
                                self.drag_state_count = Some(self.state_count());
                                self.drag_cursor_pos = Some(cursor);
                                ctx.request_repaint();
                                self.hovered_palette_index = None;
                                self.hovered_gate_id = None;
                                return;
                            }
                        }
                    }
                }
            }
        }

        if let Some(drag) = self.dragging.as_ref() {
            if pointer_down || pointer_released {
                let cursor = local_pos.or(self.drag_cursor_pos);
                if let Some(cursor) = cursor {
                    self.drag_cursor_pos = Some(cursor);
                    if let Some(index) =
                        self.placed_gates.iter().position(|gate| gate.id == drag.id)
                    {
                        let mut next_pos = cursor - drag.offset;
                        let mut next_wire = self.placed_gates[index].wire;
                        let center_y = next_pos.y + GATE_SIZE / 2.0;
                        let (line_y, distance, line_index) =
                            nearest_line(center_y, &metrics.line_ys);
                        if distance <= SNAP_DISTANCE {
                            next_pos.y = line_y - GATE_SIZE / 2.0;
                            next_wire = line_index;
                            let center_x = next_pos.x + GATE_SIZE / 2.0;
                            if let Some((slot_center, _)) = nearest_available_slot(
                                center_x,
                                line_index,
                                Some(drag.id),
                                &self.placed_gates,
                                &metrics.slot_centers,
                            ) {
                                next_pos.x = slot_center - GATE_SIZE / 2.0;
                            }
                        }
                        let gate = &mut self.placed_gates[index];
                        gate.pos = next_pos;
                        gate.wire = next_wire;
                    }
                }
            }
        } else if let Some(cursor) = local_pos {
            let mut hovered_gate = None;
            for gate in &self.placed_gates {
                let gate_rect =
                    egui::Rect::from_min_size(gate.pos, egui::vec2(GATE_SIZE, GATE_SIZE));
                if gate_rect.contains(cursor) {
                    hovered_gate = Some(gate.id);
                    break;
                }
            }
            self.hovered_gate_id = hovered_gate;

            let mut hovered_palette = None;
            if let Some(cursor_screen) = pos {
                if palette_rect.contains(cursor_screen) {
                    let local_x = cursor_screen.x - (screen_rect.min.x + palette_start_x);
                    let index = (local_x / (PALETTE_SIZE + PALETTE_GAP)).floor() as i32;
                    if index >= 0 && (index as usize) < PALETTE_GATES.len() {
                        let in_box = local_x - index as f32 * (PALETTE_SIZE + PALETTE_GAP)
                            <= PALETTE_SIZE;
                        if in_box {
                            hovered_palette = Some(index as usize);
                        }
                    }
                }
            }
            self.hovered_palette_index = hovered_palette;
        } else {
            self.hovered_gate_id = None;
            self.hovered_palette_index = None;
        }

        if pointer_released {
            if let Some(drag) = self.dragging.take() {
                if let Some(index) = self.placed_gates.iter().position(|gate| gate.id == drag.id) {
                    let gate_pos = self.placed_gates[index].pos;
                    let gate_id = self.placed_gates[index].id;
                    let center_x = gate_pos.x + GATE_SIZE / 2.0;
                    let center_y = gate_pos.y + GATE_SIZE / 2.0;
                    let (line_y, distance, line_index) = nearest_line(center_y, &metrics.line_ys);
                    let snapped = nearest_available_slot(
                        center_x,
                        line_index,
                        Some(gate_id),
                        &self.placed_gates,
                        &metrics.slot_centers,
                    );
                    let on_circuit = center_x >= metrics.slot_left
                        && center_x <= metrics.slot_right
                        && distance <= SNAP_DISTANCE
                        && snapped.map(|(_, d)| d <= SNAP_DISTANCE).unwrap_or(false);

                    if !on_circuit {
                        self.placed_gates.remove(index);
                    } else if let Some((slot_center, _)) = snapped {
                        let gate = &mut self.placed_gates[index];
                        gate.pos.x = slot_center - GATE_SIZE / 2.0;
                        gate.pos.y = line_y - GATE_SIZE / 2.0;
                        gate.wire = line_index;
                    }
                    self.update_qubit_count();
                    self.needs_recompute = true;
                    ctx.request_repaint();
                }
            }
            self.drag_state_count = None;
            self.drag_repaint_deadline = None;
            self.drag_repaint_pending = false;
            self.drag_cursor_pos = None;
        }

        if self.dragging.is_some() && pointer_down {
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if self.hovered_gate_id.is_some() || self.hovered_palette_index.is_some() {
            ctx.set_cursor_icon(egui::CursorIcon::Grab);
        }
    }

    fn schedule_drag_repaint(&mut self, ctx: &egui::Context, frame_secs: f64) {
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

impl eframe::App for QniApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let frame_start = now_seconds();
        egui::CentralPanel::default().show(ctx, |ui| {
            let screen_rect = ui.max_rect();
            let colors = Colors::new();
            let content_height =
                self.circuit_content_height(self.layout_qubits(), screen_rect.height());

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .scroll_source(egui::scroll_area::ScrollSource {
                    drag: false,
                    ..egui::scroll_area::ScrollSource::default()
                })
                .show(ui, |ui| {
                    let (rect, _response) = ui.allocate_exact_size(
                        egui::vec2(screen_rect.width(), content_height),
                        egui::Sense::click_and_drag(),
                    );
                    self.handle_input(rect, ctx, screen_rect);
                    let content_changed =
                        self.last_content_rect.map_or(true, |last| last != rect);
                    self.last_content_rect = Some(rect);
                    if content_changed {
                        ctx.request_repaint();
                    }

                    let metrics = layout_metrics(rect.width(), self.layout_qubits());
                    let painter = ui.painter_at(rect);
                    let fast_drag = self.dragging.is_some();
                    self.draw_circuit(&painter, rect, &metrics, &colors, fast_drag);
                });

            let base_state_count = self.state_count();
            let state_count = if self.dragging.is_some() {
                self.drag_state_count.unwrap_or(base_state_count)
            } else {
                base_state_count
            };
            let mut recompute = self.needs_recompute || state_count != self.last_state_count;
            let state_layout = self.state_panel_layout(screen_rect, state_count);
            self.clamp_state_panel_offset(&state_layout, screen_rect);
            let state_rect = state_layout.state_rect.translate(self.state_panel_offset);
            let handle_rect = egui::Rect::from_min_size(
                state_rect.min,
                egui::vec2(state_rect.width(), state_layout.handle_height.max(6.0)),
            );
            let handle_response = ui.interact(
                handle_rect,
                egui::Id::new("state_panel_handle"),
                egui::Sense::drag(),
            );
            if handle_response.drag_started() {
                if let Some(pos) = handle_response.interact_pointer_pos() {
                    self.state_panel_drag = Some(pos - handle_rect.min);
                }
            }
            if handle_response.dragged() {
                if let (Some(pos), Some(offset)) = (
                    handle_response.interact_pointer_pos(),
                    self.state_panel_drag,
                ) {
                    let desired_min = pos - offset;
                    self.state_panel_offset = desired_min - state_layout.state_rect.min;
                    self.clamp_state_panel_offset(&state_layout, screen_rect);
                }
            }
            if handle_response.drag_stopped() {
                self.state_panel_drag = None;
            }

            let overlay_painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("overlay"),
            ));
            let target_format = frame.wgpu_render_state().map(|state| state.target_format);
            if target_format.is_some() {
                if recompute {
                    self.needs_recompute = false;
                    self.last_state_count = state_count;
                }
            } else if recompute {
                ctx.request_repaint();
                recompute = false;
            }
            self.draw_palette(&overlay_painter, screen_rect, &colors);
            self.draw_state_vector(
                &overlay_painter,
                &colors,
                &state_layout,
                self.state_panel_offset,
                state_layout.handle_height,
                screen_rect,
                recompute,
                target_format,
            );
        });

        let now = now_seconds();
        let frame_secs = (now - frame_start).max(0.0);
        if self.dragging.is_some() {
            self.schedule_drag_repaint(ctx, frame_secs);
        } else if self.drag_repaint_deadline.is_some() || self.drag_repaint_pending {
            self.drag_repaint_deadline = None;
            self.drag_repaint_pending = false;
        }
        if now < self.startup_repaint_until {
            ctx.request_repaint_after(Duration::from_secs_f64(DRAG_REPAINT_MIN_SECS));
        }
    }
}

