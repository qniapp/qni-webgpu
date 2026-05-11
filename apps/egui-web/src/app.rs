//! App root — `QniApp` struct, init / accessors, and the
//! `eframe::App::update` coordinator. The bulk of per-frame work lives
//! in two submodules:
//!   * `state_panel` — state-panel interactions + small state helpers
//!   * `gate_input`  — gate pickup / drag / drop / hover + repaint throttle

mod state_panel;
mod gate_input;

use eframe::egui;
use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use crate::bloch::{linearize_ops, SimulationOp};
use crate::colors::Colors;
use crate::constants::{
    state_circle_default_aspect_index, ASPECT_WHEEL_PER_STEP, DRAG_REPAINT_MIN_SECS, MAX_QUBITS,
    MIN_QUBITS, STATE_GRID_ZOOM_MAX, STATE_GRID_ZOOM_MIN, STATE_VIEWPORT_DEFAULT_HEIGHT,
    STATE_VIEWPORT_DEFAULT_WIDTH,
};
use crate::gates::GateKind;
use crate::layout::layout_metrics;
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

/// Which corner of the state panel a resize drag is anchored to. The
/// opposite corner stays fixed during the drag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResizeCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl ResizeCorner {
    pub(crate) fn is_top(self) -> bool {
        matches!(self, ResizeCorner::TopLeft | ResizeCorner::TopRight)
    }
}

/// In-flight resize drag. `start_*` fields snapshot the panel state at
/// drag-start so the new size is computed relative to the cursor's total
/// movement (avoiding the integrator drift you'd get from per-frame deltas).
#[derive(Clone, Copy, Debug)]
struct ResizeDrag {
    corner: ResizeCorner,
    start_pointer: egui::Pos2,
    start_viewport_size: egui::Vec2,
    start_panel_offset: egui::Vec2,
}

pub(crate) struct QniApp {
    next_gate_id: u32,
    pub(crate) placed_gates: Vec<PlacedGate>,
    dragging: Option<DragState>,
    drag_state_count: Option<usize>,
    state_panel_drag: Option<egui::Vec2>,
    pub(crate) state_panel_offset: egui::Vec2,
    /// Pan offset of the circle grid INSIDE the state panel viewport.
    /// Independent of `state_panel_offset` (which moves the whole panel).
    /// Only used when the grid is bigger than the viewport on a given axis;
    /// when it fits, the grid is centred and this offset is ignored.
    pub(crate) state_grid_offset: egui::Vec2,
    /// Zoom factor for the state panel circle grid (1.0 = qni's natural
    /// per-qubit cell sizes). Ctrl+wheel inside the viewport adjusts this;
    /// `clamp_state_grid_zoom` keeps it inside `STATE_GRID_ZOOM_RANGE`.
    pub(crate) state_grid_zoom: f32,
    /// User-controlled size of the state panel viewport (the circle area
    /// below the header strip). Initialized to `STATE_VIEWPORT_DEFAULT_*`
    /// and changed by dragging the 4 corner L-handles. Always clamped to
    /// `STATE_VIEWPORT_MIN/MAX_*`.
    pub(crate) state_viewport_size: egui::Vec2,
    /// In-flight resize drag (one of the 4 corner handles).
    state_resize_drag: Option<ResizeDrag>,
    /// Currently-hovered resize corner (for cursor / paint state). Set
    /// each frame by `ui.interact` hovered checks.
    pub(crate) hovered_resize_corner: Option<ResizeCorner>,
    /// `aspect_index = log2(cols)`. Determines (cols, rows) =
    /// (2^aspect_index, 2^(qubits − aspect_index)) for the state-vector
    /// circle grid. Mutated by wheel-on-dims (A 案) or popover (D 案).
    pub(crate) aspect_index: usize,
    /// Has the user explicitly chosen an aspect (vs. following qni
    /// defaults)? While `false`, aspect_index auto-tracks the qni
    /// default for the current qubit count. Once `true`, the user's
    /// pick is sticky and only clamped when it goes out of range.
    pub(crate) aspect_customized: bool,
    /// Whether the aspect-pick popover (D 案) is currently open.
    pub(crate) aspect_popover_open: bool,
    /// Accumulator for wheel deltas while the dims text is hovered. We
    /// step aspect only when the running sum crosses
    /// `ASPECT_WHEEL_PER_STEP`, so one wheel notch ≈ one step instead of
    /// per-frame firing. Reset when the pointer leaves the dims or the
    /// wheel stops for a frame so a half-step doesn't sit waiting.
    aspect_wheel_accum: f32,
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
            state_grid_offset: egui::Vec2::ZERO,
            state_grid_zoom: 1.0,
            state_viewport_size: egui::vec2(
                STATE_VIEWPORT_DEFAULT_WIDTH,
                STATE_VIEWPORT_DEFAULT_HEIGHT,
            ),
            state_resize_drag: None,
            hovered_resize_corner: None,
            aspect_index: state_circle_default_aspect_index(1),
            aspect_customized: false,
            aspect_popover_open: false,
            aspect_wheel_accum: 0.0,
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
            // Pre-compute the state panel rect (and any open popover rect)
            // so we can disable wheel-as-scroll-input when the pointer is
            // over them — otherwise the ScrollArea consumes the wheel
            // before the dims aspect handler (or viewport zoom) sees it,
            // and the circuit scrolls up/down underneath. We use the
            // current frame's state_count / aspect / offset; small drag
            // intra-frame jitter doesn't matter for a pointer-in-rect
            // check.
            let state_count_for_input_gate = self.state_count();
            let pre_state_layout =
                self.state_panel_layout(screen_rect, state_count_for_input_gate);
            let pre_state_rect = pre_state_layout
                .state_rect
                .translate(self.state_panel_offset);
            let pre_popover_rect = if self.aspect_popover_open {
                let dims_hit = QniApp::dims_hit_rect(ctx, &pre_state_layout, self.state_panel_offset);
                let (rect, _) = QniApp::aspect_popover_layout(
                    dims_hit,
                    crate::shared::amplitude_qubits(state_count_for_input_gate)
                        .clamp(1, MAX_QUBITS),
                );
                Some(rect)
            } else {
                None
            };
            let pointer_over_state_panel = ctx
                .input(|i| i.pointer.hover_pos())
                .map(|p| {
                    pre_state_rect.contains(p)
                        || pre_popover_rect.map_or(false, |r| r.contains(p))
                })
                .unwrap_or(false);
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .scroll_source(egui::scroll_area::ScrollSource {
                    drag: false,
                    mouse_wheel: !pointer_over_state_panel,
                    scroll_bar: true,
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
            self.clamp_state_viewport_size();
            // Sync aspect_index with the current qubit count. While the
            // user hasn't customised, follow qni's per-qubit default;
            // once customised, only clamp to the valid [0, qubits] range
            // so the choice is sticky across qubit changes.
            let aspect_qubits = crate::shared::amplitude_qubits(state_count).clamp(1, MAX_QUBITS);
            if !self.aspect_customized {
                self.aspect_index = state_circle_default_aspect_index(aspect_qubits);
            } else {
                self.aspect_index = self.aspect_index.min(aspect_qubits);
            }
            let state_layout = self.state_panel_layout(screen_rect, state_count);
            self.clamp_state_panel_offset(&state_layout, screen_rect);
            self.clamp_state_grid_offset(&state_layout);
            let state_rect = state_layout.state_rect.translate(self.state_panel_offset);
            let handle_rect = egui::Rect::from_min_size(
                state_rect.min,
                egui::vec2(state_rect.width(), state_layout.handle_height.max(6.0)),
            );

            // Strip drag (move-the-panel) area excludes the corner-handle
            // strip on both sides so dragging from a corner triggers
            // resize, not move. The corner resize interacts are registered
            // AFTER the strip / viewport so they take priority for
            // overlapping pointer hits.
            const STRIP_CORNER_EXCLUDE: f32 = 16.0;
            let strip_drag_rect = egui::Rect::from_min_max(
                handle_rect.min + egui::vec2(STRIP_CORNER_EXCLUDE, 0.0),
                handle_rect.max - egui::vec2(STRIP_CORNER_EXCLUDE, 0.0),
            );
            let handle_response = ui.interact(
                strip_drag_rect,
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

            // Pan the grid inside the viewport. Disabled axes (= grid fits
            // on that axis) ignore the delta — see `clamp_state_grid_offset`.
            let viewport_rect = state_layout.viewport_rect.translate(self.state_panel_offset);
            let viewport_response = ui.interact(
                viewport_rect,
                egui::Id::new("state_panel_viewport"),
                egui::Sense::drag(),
            );
            if viewport_response.dragged() {
                self.state_grid_offset += viewport_response.drag_delta();
                self.clamp_state_grid_offset(&state_layout);
            }

            // Ctrl+wheel inside the viewport zooms the grid. Plain wheel is
            // intentionally NOT consumed — the page / scroll area still
            // scrolls. Zoom is anchored at the cursor: we adjust
            // `state_grid_offset` so the cell under the pointer stays put.
            if viewport_response.hovered() {
                let scroll = ctx.input(|i| {
                    if i.modifiers.ctrl || i.modifiers.command {
                        i.smooth_scroll_delta.y
                    } else {
                        0.0
                    }
                });
                if scroll.abs() > f32::EPSILON {
                    let pointer = ctx.input(|i| i.pointer.hover_pos());
                    let old_zoom = self.state_grid_zoom;
                    let new_zoom = (old_zoom * (scroll * 0.005).exp())
                        .clamp(STATE_GRID_ZOOM_MIN, STATE_GRID_ZOOM_MAX);
                    if (new_zoom - old_zoom).abs() > f32::EPSILON {
                        // Keep the cell under the cursor (or viewport
                        // centre, fallback) anchored across the zoom.
                        let anchor = pointer.unwrap_or(viewport_rect.center());
                        let pre_origin = QniApp::grid_origin(
                            &state_layout,
                            self.state_panel_offset,
                            self.state_grid_offset,
                        );
                        let from_origin = anchor - pre_origin;
                        let scale = new_zoom / old_zoom;
                        let drift = from_origin * (scale - 1.0);
                        self.state_grid_zoom = new_zoom;
                        self.state_grid_offset -= drift;
                        // Layout recomputes on the next frame with the new
                        // zoom, but clamp now so we don't render a 1-frame
                        // out-of-bounds pan.
                        let zoomed = self.state_panel_layout(screen_rect, state_count);
                        self.clamp_state_grid_offset(&zoomed);
                    }
                }
            }

            // Aspect dims (A 案: wheel / click on the "C × R = N states ▾"
            // text in the strip). Registered after the strip drag so its
            // hit rect takes priority for clicks that land on it.
            let dims_hit = QniApp::dims_hit_rect(ctx, &state_layout, self.state_panel_offset);
            let dims_resp = ui.interact(
                dims_hit,
                egui::Id::new("state_dims"),
                egui::Sense::click(),
            );
            if dims_resp.hovered() {
                // Plain wheel: accumulate scroll delta into
                // `aspect_wheel_accum` and step the aspect each time the
                // sum crosses ±ASPECT_WHEEL_PER_STEP. Without the
                // accumulator the previous "fire every frame |scroll|>1"
                // logic stepped tens of times per wheel notch and was
                // unusable for fine adjustment. Ctrl+wheel is reserved
                // for viewport zoom and is NOT consumed here.
                let plain_scroll = ctx.input(|i| {
                    if i.modifiers.ctrl || i.modifiers.command {
                        0.0
                    } else {
                        i.smooth_scroll_delta.y
                    }
                });
                if plain_scroll.abs() > f32::EPSILON {
                    self.aspect_wheel_accum += plain_scroll;
                    let mut steps: i32 = 0;
                    while self.aspect_wheel_accum >= ASPECT_WHEEL_PER_STEP {
                        self.aspect_wheel_accum -= ASPECT_WHEEL_PER_STEP;
                        steps -= 1; // positive scroll → taller (cols −1)
                    }
                    while self.aspect_wheel_accum <= -ASPECT_WHEEL_PER_STEP {
                        self.aspect_wheel_accum += ASPECT_WHEEL_PER_STEP;
                        steps += 1; // negative scroll → wider (cols +1)
                    }
                    if steps != 0 {
                        let new_aspect = (self.aspect_index as i32 + steps)
                            .clamp(0, aspect_qubits as i32)
                            as usize;
                        if new_aspect != self.aspect_index {
                            self.aspect_index = new_aspect;
                            self.aspect_customized = true;
                            ctx.request_repaint();
                        }
                    }
                } else {
                    // Wheel stopped this frame — drop any sub-step
                    // residue so the next notch starts from zero.
                    self.aspect_wheel_accum = 0.0;
                }
                ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
            } else {
                // Pointer left the dims area — discard pending accum so
                // re-entering doesn't fire a stale step.
                self.aspect_wheel_accum = 0.0;
            }
            if dims_resp.clicked() {
                self.aspect_popover_open = !self.aspect_popover_open;
            }

            // Aspect popover (D 案) — row clicks + outside-click / ESC close.
            if self.aspect_popover_open {
                let (popover_rect, row_rects) =
                    QniApp::aspect_popover_layout(dims_hit, aspect_qubits);
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
                        self.aspect_index = i;
                        self.aspect_customized = true;
                        self.aspect_popover_open = false;
                    }
                }
                // Outside click closes. Use any_pressed to catch the click
                // that initiated outside the popover this frame.
                let pressed = ctx.input(|i| i.pointer.any_pressed());
                if pressed {
                    if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                        if !dims_hit.contains(pos) && !popover_rect.contains(pos) {
                            self.aspect_popover_open = false;
                        }
                    }
                }
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && self.aspect_popover_open {
                self.aspect_popover_open = false;
            }

            // Resize handles last so they take priority over the strip
            // and viewport interactions on pointer hits (egui's last-added
            // widget wins for overlapping rects). The hit rect is the
            // visible L plus `STATE_RESIZE_HIT_PAD` on each side.
            self.hovered_resize_corner = None;
            for corner in [
                ResizeCorner::TopLeft,
                ResizeCorner::TopRight,
                ResizeCorner::BottomLeft,
                ResizeCorner::BottomRight,
            ] {
                let hit = QniApp::resize_handle_hit_rect(
                    &state_layout,
                    self.state_panel_offset,
                    corner,
                );
                let id_label = match corner {
                    ResizeCorner::TopLeft => "state_resize_tl",
                    ResizeCorner::TopRight => "state_resize_tr",
                    ResizeCorner::BottomLeft => "state_resize_bl",
                    ResizeCorner::BottomRight => "state_resize_br",
                };
                let resp = ui.interact(hit, egui::Id::new(id_label), egui::Sense::drag());
                if resp.hovered() {
                    self.hovered_resize_corner = Some(corner);
                }
                if resp.drag_started() {
                    if let Some(p) = resp.interact_pointer_pos() {
                        self.begin_resize_drag(corner, p);
                    }
                }
                if resp.dragged() && self.active_resize_corner() == Some(corner) {
                    if let Some(p) = resp.interact_pointer_pos() {
                        self.apply_resize_drag(p);
                    }
                }
                if resp.drag_stopped() && self.active_resize_corner() == Some(corner) {
                    self.end_resize_drag();
                }
                if resp.hovered() || self.active_resize_corner() == Some(corner) {
                    let cursor = match corner {
                        ResizeCorner::TopLeft | ResizeCorner::BottomRight => {
                            egui::CursorIcon::ResizeNwSe
                        }
                        ResizeCorner::TopRight | ResizeCorner::BottomLeft => {
                            egui::CursorIcon::ResizeNeSw
                        }
                    };
                    ctx.set_cursor_icon(cursor);
                }
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
