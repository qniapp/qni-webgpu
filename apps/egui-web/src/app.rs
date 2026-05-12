//! App root — `QniApp` struct, init / accessors, and the
//! `eframe::App::update` coordinator. Per-frame helpers live in
//! `state_panel`, `gate_input`, and `drag_controller`.

mod circuit_model;
mod drag_controller;
mod fps_hud;
mod gate_input;
mod state_panel;

use eframe::egui;
use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use crate::bloch::{
    linearize_ops, validate_simulation_plan_capacity, SimulationOp, SimulationPlanLimits,
};
use crate::colors::Colors;
use crate::constants::{
    state_circle_default_aspect_index, DRAG_REPAINT_MIN_SECS, MAX_QUBITS, MIN_QUBITS,
    STATE_VIEWPORT_DEFAULT_HEIGHT, STATE_VIEWPORT_DEFAULT_WIDTH,
};
use crate::gpu::{MAX_BLOCH_SLOTS, MAX_MEASUREMENT_SLOTS, MAX_OPS_PER_RECOMPUTE};
use crate::layout::layout_metrics;
use crate::shared::{amplitude_qubits, now_seconds};

pub(crate) use circuit_model::{DragState, PlacedGate, QftResizeDrag};

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
    /// Horizontal scroll offset for the circuit area, in egui pixels.
    /// When circuit content exceeds the canvas width, this pushes the
    /// rendered circuit left by that many pixels so the user can see
    /// the trailing gates. Always clamped to
    /// `[0, max(0, line_right - canvas_width)]` post-update.
    pub(crate) circuit_scroll_x: f32,
    pub(crate) dragging: Option<DragState>,
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
    /// Gate id whose QFT resize handle is currently hovered (drives
    /// the handle's idle → hover color). `None` when no hand is on a
    /// QFT bottom-edge handle.
    pub(crate) hovered_qft_resize_handle: Option<u32>,
    /// In-flight QFT span-resize drag (only one at a time).
    pub(crate) qft_resize_drag: Option<QftResizeDrag>,
    /// Column index the pointer is currently hovering over for the
    /// step-preview interaction. Drives the live "state-vector at step
    /// k" preview without committing — drops back to `breakpoint_step`
    /// when the pointer leaves the slot row.
    pub(crate) hovered_step: Option<usize>,
    /// Column index the user clicked to "lock in" as the step shown.
    /// `None` means: show the final-state (all columns applied), which
    /// is the default.
    pub(crate) breakpoint_step: Option<usize>,
    /// Display-index (`row * cols + col`) of the state-vector cell the
    /// pointer is currently hovering over. Drives the GPU shader's
    /// brightness(0.9) darken on fill / needle / outline for that cell.
    pub(crate) hovered_state_cell: Option<u32>,
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
        // Register Hack as a fallback for the proportional family so
        // mathematical angle brackets `⟨` `⟩` (U+27E8 / U+27E9) used in
        // ket labels render instead of falling back to tofu. egui's
        // bundled `Ubuntu-Light` proportional font does not include
        // those code points, but `Hack-Regular` does.
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "hack_fallback".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(
                epaint_default_fonts::HACK_REGULAR,
            )),
        );
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts
                .families
                .entry(family)
                .or_default()
                .push("hack_fallback".to_owned());
        }
        cc.egui_ctx.set_fonts(fonts);
        cc.egui_ctx.request_repaint();
        // Restore a shared circuit from the URL (`#{"cols":[...]}` or
        // qni-style path) so a pasted URL spins up the same circuit.
        let (initial_gates, next_gate_id) = crate::url_circuit::parse_circuit_from_url();
        let initial_qubit_count = crate::url_circuit::qubit_count_from_gates(&initial_gates)
            .clamp(MIN_QUBITS, MAX_QUBITS);
        Self {
            next_gate_id,
            placed_gates: initial_gates,
            circuit_scroll_x: 0.0,
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
            hovered_qft_resize_handle: None,
            qft_resize_drag: None,
            hovered_step: None,
            breakpoint_step: None,
            hovered_state_cell: None,
            aspect_index: state_circle_default_aspect_index(1),
            aspect_customized: false,
            aspect_popover_open: false,
            aspect_wheel_accum: 0.0,
            hovered_gate_id: None,
            hovered_palette_index: None,
            qubit_count: initial_qubit_count,
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

    /// Refresh the simulation operation list + per-gate slot lookups
    /// when something changed (qubits added, gates rearranged, etc.).
    /// Returns the (possibly updated) recompute flag — false if we
    /// punted because the GPU target wasn't ready yet.
    fn process_gpu_recompute(
        &mut self,
        target_format: Option<eframe::wgpu::TextureFormat>,
        recompute: bool,
        _screen_rect: egui::Rect,
        state_count: usize,
        ctx: &egui::Context,
    ) -> bool {
        if target_format.is_some() {
            if recompute {
                self.needs_recompute = false;
                self.last_state_count = state_count;
                let qubits = self.state_qubits();
                // hovered wins over breakpoint (live preview); `None`
                // for both = apply every column = final state.
                let step_limit = self.hovered_step.or(self.breakpoint_step);
                self.sim_ops = linearize_ops(&self.placed_gates, qubits, step_limit);
                self.bloch_slots.clear();
                self.measurement_slots.clear();
                if let Err(error) = validate_simulation_plan_capacity(
                    &self.sim_ops,
                    SimulationPlanLimits {
                        max_ops_per_variant: MAX_OPS_PER_RECOMPUTE,
                        max_bloch_slots: MAX_BLOCH_SLOTS,
                        max_measurement_slots: MAX_MEASUREMENT_SLOTS,
                    },
                ) {
                    self.log_gpu_plan_capacity_error(&error.to_string());
                    self.sim_ops.clear();
                    return recompute;
                }
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
            recompute
        } else if recompute {
            ctx.request_repaint();
            false
        } else {
            false
        }
    }

    fn log_gpu_plan_capacity_error(&self, message: &str) {
        #[cfg(target_arch = "wasm32")]
        {
            let message = format!("qni-webgpu recompute skipped: {message}");
            web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(&message));
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = message;
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

            // Decide whether wheel-over-the-panel should suppress the
            // surrounding ScrollArea's page-scroll. If pointer is on the
            // state panel (or its popover), we want wheel to route to
            // our handlers (aspect dims, viewport zoom) instead.
            let pointer_over_state_panel = self.compute_state_panel_input_gate(ctx, screen_rect);

            let mut dragging_gate_id = None;
            let mut content_rect = None;
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
                    let content_changed = self.last_content_rect != Some(rect);
                    self.last_content_rect = Some(rect);
                    content_rect = Some(rect);
                    if content_changed {
                        ctx.request_repaint();
                    }

                    let metrics = layout_metrics(
                        rect.width(),
                        self.layout_qubits(),
                        self.min_circuit_slots(),
                    );
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
                        self.circuit_scroll_x,
                    );
                });

            // Resolve the state count / aspect / layout for this frame.
            // While a gate is mid-drag, an extra phantom qubit is added
            // (`drag_state_count`) so the layout doesn't reflow underneath
            // the user during the drag.
            let base_state_count = self.state_count();
            let state_count = if self.dragging.is_some() {
                self.drag_state_count.unwrap_or(base_state_count)
            } else {
                base_state_count
            };
            let recompute = self.needs_recompute || state_count != self.last_state_count;
            self.clamp_state_viewport_size();
            // Sync aspect_index with the current qubit count. While the
            // user hasn't customised, follow qni's per-qubit default;
            // once customised, only clamp to the valid [0, qubits] range
            // so the choice is sticky across qubit changes.
            let aspect_qubits = amplitude_qubits(state_count).clamp(1, MAX_QUBITS);
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

            // State panel interactions. Order matters: resize handles are
            // registered last so they take priority over the strip and
            // viewport interacts at overlapping pointer hits.
            self.process_state_panel_strip_drag(ui, &state_layout, screen_rect, handle_rect);
            self.process_state_panel_viewport_pan_and_zoom(
                ctx,
                ui,
                &state_layout,
                screen_rect,
                state_count,
            );
            let dims_hit = QniApp::dims_hit_rect(ctx, &state_layout, self.state_panel_offset);
            self.process_aspect_dims(ctx, ui, aspect_qubits, dims_hit);
            self.process_aspect_popover(ctx, ui, aspect_qubits, dims_hit);
            self.process_resize_handles(ctx, ui, &state_layout);

            // GPU recompute (sim_ops + per-gate slot lookups) if anything
            // changed since the last frame and the target is ready.
            let overlay_painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("overlay"),
            ));
            let target_format = frame.wgpu_render_state().map(|state| state.target_format);
            let recompute =
                self.process_gpu_recompute(target_format, recompute, screen_rect, state_count, ctx);

            // Draw palette + state vector + (optional) drag preview.
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
                self.draw_drag_preview(
                    &overlay_painter,
                    content_rect,
                    &colors,
                    dragging_gate_id,
                    self.circuit_scroll_x,
                );
            }

            // Tooltip is drawn last so it sits on top of the drag
            // preview / state panel / everything else in the overlay
            // layer. `draw_palette_tooltip` is a no-op when no palette
            // gate is hovered or a drag is in progress.
            self.draw_palette_tooltip(&overlay_painter, screen_rect, &colors);
        });

        // Per-frame repaint scheduling: drag throttle + startup priming.
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

        // Debug FPS HUD (backtick toggles).
        if ctx.input(|i| i.key_pressed(egui::Key::Backtick)) {
            self.fps_hud_visible = !self.fps_hud_visible;
            if !self.fps_hud_visible {
                self.fps_hud_history.clear();
                self.fps_hud_cpu_history.clear();
                self.fps_hud_svp_history.clear();
            }
        }
        if self.fps_hud_visible {
            self.draw_fps_hud(ctx, frame_secs);
        }
    }
}
