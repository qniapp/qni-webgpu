//! App root — `QniApp` state, initialization, and small accessors.
//! Per-frame update order lives in `update_flow`.

mod circuit_history;
pub(crate) mod circuit_library;
mod circuit_model;
pub(crate) mod circuit_picker_state;
mod drag_controller;
mod exec_mode;
mod external_gpu;
mod fps_hud;
mod gate_input;
mod gpu_plan_state;
mod state_panel;
mod state_panel_state;
mod update_flow;

use eframe::egui;
use std::collections::VecDeque;
use std::sync::LazyLock;

use crate::colors::{Colors, Theme, ThemeKind};
use crate::constants::{LOCAL_MAX_QUBITS, MIN_QUBITS};
use crate::shared::now_seconds;
use circuit_history::CircuitRevision;
use circuit_library::CircuitLibrary;
use circuit_picker_state::PickerState;

/// Named font family rendering the remaining heavyweight text labels — i.e.
/// RX / RY / RZ / QFT / QFT† at body × 0.40 px. At that small size Geist
/// Bold (700) is what reads as the "normal" weight. Large solo gate glyphs
/// now use the shared SVG/SDF icon path.
pub(crate) static GATE_LABEL_FAMILY: LazyLock<egui::FontFamily> =
    LazyLock::new(|| egui::FontFamily::Name("geist".into()));

pub(crate) use circuit_model::{DragState, PlacedGate, SpanResizeDrag};
pub(crate) use exec_mode::ExecMode;
pub(crate) use external_gpu::{format_gpu_duration, ExternalGpuStatus};
pub(crate) use gpu_plan_state::GpuPlanState;
pub(crate) use state_panel_state::{ResizeCorner, ResizeDrag, StatePanelState};

pub(crate) struct QniApp {
    theme: ThemeKind,
    circuit_revision: CircuitRevision,
    pub(crate) library: CircuitLibrary,
    pub(crate) picker: PickerState,
    picker_hover_suppressed_at: Option<egui::Pos2>,
    pub(crate) picker_drag_suppressed_until_release: bool,
    pub(crate) picker_submenu_toggle_suppressed_until_release: bool,
    pub(crate) picker_drag_animation_epoch: u64,
    pub(crate) picker_overlay_rect: Option<egui::Rect>,
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
    /// Gate id whose resizable-span handle is currently hovered (drives
    /// the handle's idle → hover color). `None` when no hand is on a
    /// QFT / Chance bottom-edge handle.
    pub(crate) hovered_span_resize_handle: Option<u32>,
    /// In-flight resizable-span drag (only one at a time).
    pub(crate) span_resize_drag: Option<SpanResizeDrag>,
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
    /// `(gate_id, outcome)` for the Chance row under the pointer. The
    /// outcome index is geometry-only; probability values remain GPU-only.
    pub(crate) hovered_chance_outcome: Option<(u32, u32)>,
    pub(crate) hovered_palette_index: Option<usize>,
    qubit_count: usize,
    pub(crate) exec_mode: ExecMode,
    pub(crate) exec_mode_keyboard_focus: bool,
    pub(crate) external_gpu_status: ExternalGpuStatus,
    pub(crate) external_gpu_started_at: Option<f64>,
    /// One-shot local WebGPU refresh for the state-vector panel after an
    /// explicit external GPU run completes. Keeps GPU mode from live-
    /// recomputing on every edit while still making <=16-qubit runs visible.
    pub(crate) external_gpu_state_refresh_pending: bool,
    pub(crate) gpu_plan: GpuPlanState,
    last_content_rect: Option<egui::Rect>,
    drag_cursor_pos: Option<egui::Pos2>,
    drag_repaint_deadline: Option<f64>,
    drag_repaint_pending: bool,
    startup_repaint_until: f64,
    pointer_was_down: bool,
    /// Debug HUD: backtick (`) toggles a small bottom-right overlay showing
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
        let theme = Theme::default();
        theme.apply_to_context(&cc.egui_ctx);
        cc.egui_ctx.style_mut(|style| {
            style.spacing.window_margin = egui::Margin::same(0);
        });
        // Font setup:
        // Start from `empty` so egui doesn't parse unused bundled fonts
        // (Ubuntu / Noto Emoji / default Hack) after we replace both
        // public font families.
        // 1. Register a CP932/JIS Japanese subset of Noto Sans CJK JP
        //    Regular as the UI fallback, so circuit names entered during
        //    Rename render Japanese text instead of tofu. Noto CJK is SIL
        //    OFL 1.1; the subset covers hiragana, katakana, common kanji,
        //    full-width forms, and Windows Japanese name variants.
        // 2. Register Hack only as the final fallback so mathematical
        //    angle brackets `⟨` `⟩` (U+27E8 / U+27E9) used in ket labels
        //    render instead of falling back to tofu if Geist lacks them.
        // 3. Register Geist Sans weights for gate labels and all
        //    proportional UI text. Register Geist Mono for monospace UI
        //    text (state headers, popups, phase labels, FPS HUD, toggle).
        //    Geist is SIL OFL 1.1 (vercel/geist-font) and embedded via
        //    `include_bytes!`.
        let mut fonts = egui::FontDefinitions::empty();
        fonts.font_data.insert(
            "hack_fallback".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(
                epaint_default_fonts::HACK_REGULAR,
            )),
        );
        fonts.font_data.insert(
            "geist_bold".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
                "../assets/Geist-Bold.ttf"
            ))),
        );
        fonts.font_data.insert(
            "geist_regular".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
                "../assets/Geist-Regular.ttf"
            ))),
        );
        fonts.font_data.insert(
            "geist_medium".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
                "../assets/Geist-Medium.ttf"
            ))),
        );
        fonts.font_data.insert(
            "geist_mono".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
                "../assets/GeistMono-Regular.ttf"
            ))),
        );
        fonts.font_data.insert(
            "qni_japanese_fallback".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
                "../assets/QniJapaneseFallback-Regular.otf"
            ))),
        );
        fonts.families.insert(
            egui::FontFamily::Proportional,
            vec![
                "geist_regular".to_owned(),
                "qni_japanese_fallback".to_owned(),
                "hack_fallback".to_owned(),
            ],
        );
        fonts.families.insert(
            egui::FontFamily::Monospace,
            vec![
                "geist_mono".to_owned(),
                "qni_japanese_fallback".to_owned(),
                "hack_fallback".to_owned(),
            ],
        );
        fonts
            .families
            .insert(GATE_LABEL_FAMILY.clone(), vec!["geist_bold".to_owned()]);
        fonts.families.insert(
            egui::FontFamily::Name("geist-medium".into()),
            vec!["geist_medium".to_owned()],
        );
        cc.egui_ctx.set_fonts(fonts);
        Self::wire_test_hooks(&cc.egui_ctx);
        cc.egui_ctx.request_repaint();
        // Restore a shared circuit from the URL (`#{"cols":[...]}` or
        // qni-style path) first. If no URL payload is present, use the
        // persisted active localStorage circuit; if no persisted library
        // exists, keep the seeded samples and a separate "Circuit 1" current
        // entry so examples are not overwritten by the empty editor.
        let (url_gates, url_next_gate_id) = crate::url_circuit::parse_circuit_from_url();
        let url_required_qubits = crate::url_circuit::qubit_count_from_gates(&url_gates);
        let url_exec_mode = if url_required_qubits > LOCAL_MAX_QUBITS {
            ExecMode::Gpu
        } else {
            ExecMode::default()
        };
        let url_qubit_count = url_required_qubits.clamp(MIN_QUBITS, url_exec_mode.qubit_capacity());
        let url_json = crate::url_circuit::circuit_to_json(&url_gates, url_qubit_count);
        let (library, initial_json) = circuit_library::for_startup(
            url_json.clone(),
            crate::url_circuit::current_url_has_circuit_payload(),
        );
        let (initial_gates, next_gate_id) = if initial_json == url_json {
            (url_gates, url_next_gate_id)
        } else {
            crate::url_circuit::parse_circuit_json(&initial_json)
        };
        let initial_required_qubits = crate::url_circuit::qubit_count_from_gates(&initial_gates);
        let exec_mode = if initial_required_qubits > LOCAL_MAX_QUBITS {
            ExecMode::Gpu
        } else {
            ExecMode::default()
        };
        let initial_qubit_count =
            initial_required_qubits.clamp(MIN_QUBITS, exec_mode.qubit_capacity());
        let initial_json = crate::url_circuit::circuit_to_json(&initial_gates, initial_qubit_count);
        crate::url_circuit::write_circuit_to_url(&initial_json);
        Self {
            theme: theme.kind,
            circuit_revision: CircuitRevision::starting_at(initial_json),
            library,
            picker: PickerState::default(),
            picker_hover_suppressed_at: None,
            picker_drag_suppressed_until_release: false,
            picker_submenu_toggle_suppressed_until_release: false,
            picker_drag_animation_epoch: 0,
            picker_overlay_rect: None,
            next_gate_id,
            placed_gates: initial_gates,
            circuit_scroll_x: 0.0,
            dragging: None,
            drag_state_count: None,
            state_panel: StatePanelState::default(),
            hovered_span_resize_handle: None,
            span_resize_drag: None,
            hovered_step: None,
            breakpoint_step: None,
            hovered_gate_id: None,
            hovered_chance_outcome: None,
            hovered_palette_index: None,
            qubit_count: initial_qubit_count,
            exec_mode,
            exec_mode_keyboard_focus: false,
            external_gpu_status: ExternalGpuStatus::default(),
            external_gpu_started_at: None,
            external_gpu_state_refresh_pending: false,
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

    pub(crate) fn colors(&self) -> Colors {
        Colors::for_theme(self.theme)
    }

    fn layout_qubits(&self) -> usize {
        let capacity = self.exec_mode.qubit_capacity();
        let mut count = self.qubit_count.clamp(MIN_QUBITS, capacity);
        if self.dragging.is_some() && count < capacity {
            count += 1;
        }
        count
    }

    pub(crate) fn local_state_vector_active(&self) -> bool {
        self.exec_mode == ExecMode::Local && self.required_qubit_count() <= LOCAL_MAX_QUBITS
    }

    pub(crate) fn local_exec_mode_available(&self) -> bool {
        self.required_qubit_count() <= LOCAL_MAX_QUBITS
    }
}
