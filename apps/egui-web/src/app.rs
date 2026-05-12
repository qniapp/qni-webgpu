//! App root — `QniApp` state, initialization, and small accessors.
//! Per-frame update order lives in `update_flow`.

mod circuit_model;
mod drag_controller;
mod fps_hud;
mod gate_input;
mod gpu_plan_state;
mod state_panel;
mod state_panel_state;
mod update_flow;

use eframe::egui;
use std::collections::VecDeque;

use crate::constants::{MAX_QUBITS, MIN_QUBITS};
use crate::shared::now_seconds;

pub(crate) use circuit_model::{DragState, PlacedGate, QftResizeDrag};
pub(crate) use gpu_plan_state::GpuPlanState;
pub(crate) use state_panel_state::{ResizeCorner, ResizeDrag, StatePanelState};

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
    pub(crate) state_panel: StatePanelState,
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
    pub(crate) hovered_gate_id: Option<u32>,
    pub(crate) hovered_palette_index: Option<usize>,
    qubit_count: usize,
    pub(crate) gpu_plan: GpuPlanState,
    last_content_rect: Option<egui::Rect>,
    drag_cursor_pos: Option<egui::Pos2>,
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
            state_panel: StatePanelState::default(),
            hovered_qft_resize_handle: None,
            qft_resize_drag: None,
            hovered_step: None,
            breakpoint_step: None,
            hovered_gate_id: None,
            hovered_palette_index: None,
            qubit_count: initial_qubit_count,
            gpu_plan: GpuPlanState::default(),
            last_content_rect: None,
            drag_cursor_pos: None,
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
}
