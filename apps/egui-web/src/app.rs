use eframe::egui;
use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use crate::bloch::{linearize_ops, SimulationOp};
use crate::colors::Colors;
use crate::constants::{
    DRAG_REPAINT_BASE_SECS, DRAG_REPAINT_MAX_SECS, DRAG_REPAINT_MIN_SECS,
    DRAG_REPAINT_PUMP_FACTOR, GATE_SIZE, MAX_QUBITS, MIN_QUBITS, PALETTE_GATES, PALETTE_ROW_Y,
    SNAP_DISTANCE,
};
use crate::gates::GateKind;
use crate::layout::{
    layout_metrics, nearest_available_slot, nearest_line, palette_hit_test, palette_layout,
};
use crate::shared::now_seconds;

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
    pub(crate) sim_ops: Vec<SimulationOp>,
    /// gate_id → output_slot mapping derived from the latest `sim_ops` so
    /// the GPU Bloch overlay can pick the right slot in `bloch_output_buffer`.
    pub(crate) bloch_slots: HashMap<u32, u32>,
    /// Same idea for measurement gates → `measurement_aux_buffer` slot.
    pub(crate) measurement_slots: HashMap<u32, u32>,
    drag_repaint_deadline: Option<f64>,
    drag_repaint_pending: bool,
    startup_repaint_until: f64,
    pointer_was_down: bool,
    /// Debug HUD: backtick (`) toggles a small top-right overlay showing
    /// smoothed FPS + frame ms. Off by default — when on, forces continuous
    /// repaint so the reading stays responsive (which itself costs perf,
    /// hence the toggle). F12 is avoided because Chrome reserves it for
    /// DevTools.
    fps_hud_visible: bool,
    fps_hud_history: VecDeque<f32>,
    /// Per-frame CPU time spent inside `update()` (seconds). Lets the HUD
    /// split total frame ms (= stable_dt) into CPU vs GPU/sync.
    fps_hud_cpu_history: VecDeque<f32>,
    /// CPU time spent specifically inside `draw_state_vector` (seconds).
    /// Isolates the state-panel cost so the HUD can show it next to the
    /// total CPU time — useful for "is the state panel scaling badly?"
    /// diagnostics.
    fps_hud_svp_history: VecDeque<f32>,
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
            sim_ops: Vec::new(),
            bloch_slots: HashMap::new(),
            measurement_slots: HashMap::new(),
            drag_repaint_deadline: None,
            drag_repaint_pending: false,
            startup_repaint_until: now_seconds() + 0.5,
            pointer_was_down: false,
            fps_hud_visible: false,
            fps_hud_history: VecDeque::with_capacity(120),
            fps_hud_cpu_history: VecDeque::with_capacity(120),
            fps_hud_svp_history: VecDeque::with_capacity(120),
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
        let palette_geom = palette_layout();
        let palette_start_x = screen_rect.width() / 2.0 - palette_geom.total_width / 2.0;
        let palette_origin = egui::pos2(
            screen_rect.min.x + palette_start_x,
            screen_rect.min.y + PALETTE_ROW_Y,
        );
        let palette_rect = egui::Rect::from_min_size(
            palette_origin,
            egui::vec2(palette_geom.total_width, palette_geom.total_height),
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
                    self.dragging = Some(DragState {
                        id: gate_id,
                        offset,
                    });
                    self.drag_state_count = Some(self.state_count());
                    self.drag_cursor_pos = Some(cursor);
                    ctx.request_repaint();
                    self.hovered_gate_id = None;
                    self.hovered_palette_index = None;
                    return;
                }

                if let Some(cursor_screen) = pos {
                    let local = egui::pos2(
                        cursor_screen.x - palette_origin.x,
                        cursor_screen.y - palette_origin.y,
                    );
                    if let Some(index) = palette_hit_test(local, &palette_geom) {
                        let new_id = self.next_gate_id;
                        let new_gate = PlacedGate {
                            id: new_id,
                            kind: PALETTE_GATES[index],
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
                    let local = egui::pos2(
                        cursor_screen.x - palette_origin.x,
                        cursor_screen.y - palette_origin.y,
                    );
                    hovered_palette = palette_hit_test(local, &palette_geom);
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

            let mut dragging_gate_id = None;
            let mut content_rect = None;
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
                    let content_changed = self.last_content_rect.map_or(true, |last| last != rect);
                    self.last_content_rect = Some(rect);
                    content_rect = Some(rect);
                    if content_changed {
                        ctx.request_repaint();
                    }

                    let metrics = layout_metrics(rect.width(), self.layout_qubits());
                    let painter = ui.painter_at(rect);
                    let fast_drag = self.dragging.is_some();
                    dragging_gate_id = self.dragging.map(|drag| drag.id);
                    self.draw_circuit(
                        &painter,
                        rect,
                        &metrics,
                        &colors,
                        fast_drag,
                        dragging_gate_id,
                    );
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
                    let sim_metrics = layout_metrics(screen_rect.width(), self.layout_qubits());
                    let qubits = self.state_qubits();
                    self.sim_ops = linearize_ops(&self.placed_gates, qubits, &sim_metrics);
                    self.bloch_slots.clear();
                    self.measurement_slots.clear();
                    for op in &self.sim_ops {
                        match op {
                            SimulationOp::CaptureBloch {
                                gate_id,
                                output_slot,
                                ..
                            } => {
                                self.bloch_slots.insert(*gate_id, *output_slot);
                            }
                            SimulationOp::MeasureReduceSample {
                                gate_id,
                                output_slot,
                                ..
                            } => {
                                self.measurement_slots.insert(*gate_id, *output_slot);
                            }
                            _ => {}
                        }
                    }
                }
            } else if recompute {
                ctx.request_repaint();
                recompute = false;
            }
            self.draw_palette(&overlay_painter, screen_rect, &colors);
            let svp_t0 = now_seconds();
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
            if self.fps_hud_visible {
                let svp_secs = (now_seconds() - svp_t0).max(0.0) as f32;
                self.fps_hud_svp_history.push_back(svp_secs);
                while self.fps_hud_svp_history.len() > 120 {
                    self.fps_hud_svp_history.pop_front();
                }
            }
            if let (Some(content_rect), Some(dragging_gate_id)) = (content_rect, dragging_gate_id) {
                self.draw_drag_preview(&overlay_painter, content_rect, &colors, dragging_gate_id);
            }
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

        if ctx.input(|i| i.key_pressed(egui::Key::Backtick)) {
            self.fps_hud_visible = !self.fps_hud_visible;
            if !self.fps_hud_visible {
                self.fps_hud_history.clear();
                self.fps_hud_cpu_history.clear();
                self.fps_hud_svp_history.clear();
            }
        }
        if self.fps_hud_visible {
            let dt = ctx.input(|i| i.stable_dt);
            self.fps_hud_history.push_back(dt);
            while self.fps_hud_history.len() > 120 {
                self.fps_hud_history.pop_front();
            }
            self.fps_hud_cpu_history.push_back(frame_secs as f32);
            while self.fps_hud_cpu_history.len() > 120 {
                self.fps_hud_cpu_history.pop_front();
            }
            let avg_dt = self.fps_hud_history.iter().sum::<f32>()
                / self.fps_hud_history.len().max(1) as f32;
            let avg_cpu = self.fps_hud_cpu_history.iter().sum::<f32>()
                / self.fps_hud_cpu_history.len().max(1) as f32;
            let avg_svp = if self.fps_hud_svp_history.is_empty() {
                0.0
            } else {
                self.fps_hud_svp_history.iter().sum::<f32>()
                    / self.fps_hud_svp_history.len() as f32
            };
            let fps = if avg_dt > 1e-6 { 1.0 / avg_dt } else { 0.0 };
            egui::Window::new("perf_hud")
                .anchor(egui::Align2::RIGHT_TOP, [-8.0, 8.0])
                .interactable(false)
                .resizable(false)
                .title_bar(false)
                // Above the foreground-layer state panel; otherwise the
                // panel header strip occludes the cpu/svp lines on big
                // viewports.
                .order(egui::Order::Tooltip)
                .frame(
                    egui::Frame::popup(&ctx.style())
                        .inner_margin(egui::Margin::symmetric(8, 6))
                        .fill(egui::Color32::from_rgba_unmultiplied(10, 10, 14, 235))
                        .stroke(egui::Stroke::new(
                            1.0,
                            egui::Color32::from_rgb(60, 60, 75),
                        )),
                )
                .show(ctx, |ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    ui.colored_label(
                        egui::Color32::WHITE,
                        egui::RichText::new(format!("{:5.1} FPS", fps))
                            .monospace()
                            .size(13.0),
                    );
                    let (graph_rect, _) = ui.allocate_exact_size(
                        egui::vec2(150.0, 50.0),
                        egui::Sense::hover(),
                    );
                    let painter = ui.painter_at(graph_rect);
                    // Graph background.
                    painter.rect_filled(
                        graph_rect,
                        0.0,
                        egui::Color32::from_rgba_unmultiplied(0, 0, 0, 180),
                    );
                    // Axes.
                    let axis_color = egui::Color32::from_gray(110);
                    painter.line_segment(
                        [graph_rect.left_top(), graph_rect.left_bottom()],
                        egui::Stroke::new(1.0, axis_color),
                    );
                    painter.line_segment(
                        [graph_rect.left_bottom(), graph_rect.right_bottom()],
                        egui::Stroke::new(1.0, axis_color),
                    );
                    // Y-axis ceiling: round up to next 30 fps step, clamped
                    // to [60, 150] so a single startup spike (sub-ms first
                    // frame → 600+ fps) doesn't wreck the scale.
                    let history_max = self
                        .fps_hud_history
                        .iter()
                        .map(|dt| if *dt > 1e-6 { 1.0 / *dt } else { 0.0 })
                        .fold(0.0f32, f32::max)
                        .min(150.0);
                    let y_max = ((history_max / 30.0).ceil() * 30.0)
                        .clamp(60.0, 150.0);
                    // Plot line.
                    let n = self.fps_hud_history.len();
                    if n >= 2 {
                        let denom = (n - 1) as f32;
                        let points: Vec<egui::Pos2> = self
                            .fps_hud_history
                            .iter()
                            .enumerate()
                            .map(|(i, dt)| {
                                let inst_fps =
                                    if *dt > 1e-6 { 1.0 / *dt } else { 0.0 };
                                let x = graph_rect.left()
                                    + (i as f32 / denom) * graph_rect.width();
                                let y = graph_rect.bottom()
                                    - (inst_fps / y_max).clamp(0.0, 1.0)
                                        * graph_rect.height();
                                egui::pos2(x, y)
                            })
                            .collect();
                        painter.add(egui::Shape::line(
                            points,
                            egui::Stroke::new(
                                1.5,
                                egui::Color32::from_rgb(80, 220, 120),
                            ),
                        ));
                    }
                    // Y-axis labels.
                    let label_color = egui::Color32::from_gray(160);
                    painter.text(
                        egui::pos2(graph_rect.left() + 3.0, graph_rect.top() + 1.0),
                        egui::Align2::LEFT_TOP,
                        format!("{:.0}", y_max),
                        egui::FontId::monospace(9.0),
                        label_color,
                    );
                    painter.text(
                        egui::pos2(graph_rect.left() + 3.0, graph_rect.bottom() - 1.0),
                        egui::Align2::LEFT_BOTTOM,
                        "0",
                        egui::FontId::monospace(9.0),
                        label_color,
                    );
                    let ms_color = egui::Color32::from_rgb(168, 163, 179);
                    let svp_color = egui::Color32::from_rgb(232, 199, 158);
                    let cpu_color = egui::Color32::from_rgb(140, 200, 220);
                    ui.colored_label(
                        ms_color,
                        egui::RichText::new(format!(
                            "{:5.2} ms total",
                            avg_dt * 1000.0
                        ))
                        .monospace()
                        .size(11.0),
                    );
                    ui.colored_label(
                        cpu_color,
                        egui::RichText::new(format!(
                            "{:5.2} ms cpu",
                            avg_cpu * 1000.0
                        ))
                        .monospace()
                        .size(11.0),
                    );
                    ui.colored_label(
                        svp_color,
                        egui::RichText::new(format!(
                            "{:5.2} ms svp",
                            avg_svp * 1000.0
                        ))
                        .monospace()
                        .size(11.0),
                    );
                });
            // Force continuous repaint so the reading stays live.
            ctx.request_repaint();
        }
    }
}
