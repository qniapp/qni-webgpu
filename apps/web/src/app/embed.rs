//! Per-instance embedding options and browser-state isolation.

use std::num::NonZeroUsize;

use crate::constants::MIN_QUBITS;
use crate::layout::Palette;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum AppMode {
    #[default]
    Standalone,
    Embed {
        show_state_panel: bool,
        /// qni's `data-max-wire-count`: caps the empty wires the editor adds
        /// (the `MIN_QUBITS` padding and the extra wire while dragging), never
        /// the wires a circuit already uses.
        max_wire_count: Option<NonZeroUsize>,
    },
}

impl AppMode {
    pub(crate) fn uses_browser_state(self) -> bool {
        matches!(self, Self::Standalone)
    }

    /// Only the standalone editor shows the toolbar's clear-all button;
    /// embeds keep Undo / Redo.
    pub(crate) fn shows_clear_button(self) -> bool {
        matches!(self, Self::Standalone)
    }

    pub(crate) fn shows_state_panel(self) -> bool {
        !matches!(
            self,
            Self::Embed {
                show_state_panel: false,
                ..
            }
        )
    }

    pub(crate) fn max_wire_count(self) -> Option<usize> {
        match self {
            Self::Standalone => None,
            Self::Embed { max_wire_count, .. } => max_wire_count.map(NonZeroUsize::get),
        }
    }

    /// Wires drawn even when the circuit uses fewer.
    pub(crate) fn min_visible_wire_count(self) -> usize {
        self.max_wire_count()
            .map_or(MIN_QUBITS, |max| MIN_QUBITS.min(max))
    }
}

pub(crate) struct EmbedStartup {
    pub(crate) circuit_json: String,
    pub(crate) mode: AppMode,
    pub(crate) palette: Palette,
    pub(crate) on_restart: Option<Box<dyn Fn()>>,
}

impl EmbedStartup {
    /// `palette` lists gate tokens as qni's `mini_qni` filter arguments do
    /// (`["|0>", "|1>", "H"]`); `None` keeps the full palette.
    /// `max_wire_count` is the JavaScript number of qni's `data-max-wire-count`.
    pub(crate) fn parse<S: AsRef<str>>(
        circuit_json: &str,
        show_state_panel: bool,
        palette: Option<&[S]>,
        max_wire_count: Option<f64>,
    ) -> Result<Self, String> {
        validate_circuit_json(circuit_json)?;
        let palette = palette.map(Palette::restricted).transpose()?;
        let max_wire_count = max_wire_count.map(parse_max_wire_count).transpose()?;
        Ok(Self {
            circuit_json: circuit_json.to_owned(),
            mode: AppMode::Embed {
                show_state_panel,
                max_wire_count,
            },
            palette: palette.unwrap_or_default(),
            on_restart: None,
        })
    }
}

fn parse_max_wire_count(value: f64) -> Result<NonZeroUsize, String> {
    if value.fract() != 0.0 || !(1.0..=usize::MAX as f64).contains(&value) {
        return Err(format!("maxWireCount must be a positive integer: {value}"));
    }
    Ok(NonZeroUsize::new(value as usize).expect("maxWireCount is at least one"))
}

fn validate_circuit_json(circuit_json: &str) -> Result<(), &'static str> {
    let summary = crate::url_circuit::summarize_circuit_json(circuit_json)
        .ok_or("invalid circuit JSON: expected {\"cols\":[...]} with supported gates")?;
    if summary.qubits > crate::constants::LOCAL_MAX_QUBITS {
        return Err("circuit exceeds local WebGPU qubit capacity");
    }
    Ok(())
}

impl super::QniApp {
    /// Canvas CSS pixels, independent of the previous frame and scroll position.
    /// Reserve the next editable wire so dragging does not move the host page.
    pub(crate) fn preferred_height(&self, width: f32) -> f32 {
        use crate::constants::{
            EMBED_STATE_PANEL_CIRCUIT_GAP, GATE_SIZE, LINE_GAP, LINE_Y, STATE_CIRCLE_BOTTOM_MARGIN,
            STATE_HANDLE_HEIGHT, STEP_OUTSET_Y,
        };
        // egui CentralPanel's default margin is spacing-2 (8px) on each side.
        let rect = eframe::egui::Rect::from_min_size(
            eframe::egui::pos2(8.0, 8.0),
            eframe::egui::vec2((width - 16.0).max(1.0), 1.0),
        );
        let wires = self.qubit_count + usize::from(self.qubit_count < self.wire_capacity().get());
        let last_line =
            self.circuit_origin(rect, 0.0).y + LINE_Y + LINE_GAP * wires.saturating_sub(1) as f32;
        let bottom = last_line + STEP_OUTSET_Y.max(LINE_GAP * 0.5 + self.circuit_block_outset_y());
        // Mirror the scroll content padding as well as the visible geometry.
        let content = last_line + GATE_SIZE + 16.0 + self.circuit_block_outset_y();
        let panel = if self.state_panel_visible() {
            bottom
                + EMBED_STATE_PANEL_CIRCUIT_GAP
                + STATE_HANDLE_HEIGHT
                + self.state_panel.viewport_size.y
                + STATE_CIRCLE_BOTTOM_MARGIN
        } else {
            bottom + 16.0 // spacing-4
        };
        (content.max(panel) + 8.0).ceil()
    }

    pub(crate) fn accepts_circuit_json(&self, json: &str) -> bool {
        self.mode.uses_browser_state() || validate_circuit_json(json).is_ok()
    }

    pub(crate) fn persist_library(&self) {
        if self.mode.uses_browser_state() {
            super::circuit_library::persist_library(&self.library);
        }
    }

    pub(crate) fn write_circuit_to_url(&self, json: &str) {
        if self.mode.uses_browser_state() {
            crate::url_circuit::write_circuit_to_url(json);
        }
    }

    pub(crate) fn write_exec_mode_to_url(&self) {
        if self.mode.uses_browser_state() {
            crate::url_circuit::write_exec_mode_to_url(self.exec_mode);
        }
    }

    pub(crate) fn write_document_title(&self) {
        if self.mode.uses_browser_state() {
            write_document_title(&self.circuit_title);
        }
    }
}

/// qni names the page after the circuit title and falls back to the
/// app name (`share_controller.ts` `updateDocumentTitle`).
fn document_title(circuit_title: &str) -> &str {
    if circuit_title.is_empty() {
        "Qni"
    } else {
        circuit_title
    }
}

/// Standalone only: an embed must not rename its host page.
#[cfg(target_arch = "wasm32")]
pub(super) fn write_document_title(circuit_title: &str) {
    if let Some(document) = web_sys::window().and_then(|window| window.document()) {
        document.set_title(document_title(circuit_title));
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn write_document_title(_circuit_title: &str) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::CircuitColumnIndex;

    fn parse_full(json: &str) -> Result<EmbedStartup, String> {
        EmbedStartup::parse::<&str>(json, true, None, None)
    }

    #[test]
    fn document_title_is_circuit_title() {
        assert_eq!(document_title("Superdense Coding"), "Superdense Coding");
    }

    #[test]
    fn document_title_falls_back_to_app_name() {
        assert_eq!(document_title(""), "Qni");
    }

    #[test]
    fn accepts_circuit_with_title() {
        assert!(parse_full(r#"{"cols":[["|0>"]],"title":"Superdense Coding"}"#).is_ok());
    }

    #[test]
    fn rejects_unknown_root_key() {
        assert!(parse_full(r#"{"cols":[["|0>"]],"mode":"gpu"}"#).is_err());
    }

    #[test]
    fn accepts_empty_circuit() {
        assert!(parse_full(r#"{"cols":[]}"#).is_ok());
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(parse_full("not json").is_err());
    }

    #[test]
    fn rejects_unknown_gate() {
        assert!(parse_full(r#"{"cols":[["unknown"]]}"#).is_err());
    }

    #[test]
    fn rejects_over_local_capacity() {
        let column = std::iter::repeat_n("\"H\"", crate::constants::LOCAL_MAX_QUBITS + 1)
            .collect::<Vec<_>>()
            .join(",");
        assert!(parse_full(&format!("{{\"cols\":[[{column}]]}}")).is_err());
    }

    #[test]
    fn embed_disables_browser_persistence() {
        assert!(!AppMode::Embed {
            show_state_panel: true,
            max_wire_count: None,
        }
        .uses_browser_state());
    }

    #[test]
    fn normal_app_preserves_browser_persistence() {
        assert!(AppMode::default().uses_browser_state());
    }

    #[test]
    fn embed_can_hide_state_panel() {
        assert!(!AppMode::Embed {
            show_state_panel: false,
            max_wire_count: None,
        }
        .shows_state_panel());
    }
    fn fixture(show_state_panel: bool) -> (super::super::QniApp, eframe::egui::Context) {
        let ctx = eframe::egui::Context::default();
        let cc = eframe::CreationContext::_new_kittest(ctx.clone());
        let startup =
            EmbedStartup::parse::<&str>(r#"{"cols":[["H"]]}"#, show_state_panel, None, None)
                .unwrap();
        (
            super::super::QniApp::new_with_startup(&cc, Some(startup)),
            ctx,
        )
    }

    fn height_layout_cases() -> Vec<(usize, u32, bool, f32, bool)> {
        let mut cases = Vec::new();
        for wires in [1, 2, 4, 8, 14, 16] {
            for width in [320, 390, 720, 1440] {
                for shown in [false, true] {
                    let column = std::iter::repeat_n("\"H\"", wires)
                        .collect::<Vec<_>>()
                        .join(",");
                    let cc =
                        eframe::CreationContext::_new_kittest(eframe::egui::Context::default());
                    let startup = EmbedStartup::parse::<&str>(
                        &format!("{{\"cols\":[[{column}]]}}"),
                        shown,
                        Some(&[]),
                        Some(1.0),
                    )
                    .unwrap();
                    let mut app = super::super::QniApp::new_with_startup(&cc, Some(startup));
                    app.state_panel.aspect_index =
                        crate::constants::state_circle_default_aspect_index(wires);
                    let height = app.preferred_height(width as f32);
                    let rect = eframe::egui::Rect::from_min_size(
                        eframe::egui::pos2(8.0, 8.0),
                        eframe::egui::vec2(width as f32 - 16.0, height - 16.0),
                    );
                    let layout = app.state_panel_layout(rect, 1 << wires);
                    // pitch includes a trailing gap (at most 3px), unlike grid size.
                    let fits = app.circuit_content_height(app.qubit_count, rect) <= rect.height()
                        && (!shown
                            || (rect.contains_rect(layout.state_rect)
                                && layout.viewport_rect.height()
                                    == app.state_panel.viewport_size.y
                                && layout.state_rect.top() >= app.circuit_bottom_y(rect) + 16.0
                                && layout.cell_pitch() * layout.columns() as f32
                                    <= layout.viewport_rect.width() + 3.01
                                && layout.cell_pitch() * layout.rows() as f32
                                    <= layout.viewport_rect.height() + 3.01));
                    cases.push((wires, width, shown, height, fits));
                }
            }
        }
        cases
    }

    #[test]
    fn preferred_height_and_state_grid_fit_all_wire_counts() {
        assert!(height_layout_cases().iter().all(|case| case.4));
    }

    #[test]
    fn preferred_height_layout_snapshot() {
        let cases = height_layout_cases()
            .iter()
            .map(|(wires, width, shown, height, fits)| {
                format!(
                    "wires={wires:2} width={width:4} state={shown:5} height={height:4} fits={fits}"
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        insta::assert_snapshot!(cases);
    }

    #[test]
    fn preferred_height_shrinks_without_state_panel() {
        let (shown, _) = fixture(true);
        let (hidden, _) = fixture(false);
        assert!(shown.preferred_height(390.0) > hidden.preferred_height(390.0));
    }

    #[test]
    fn preferred_height_reserves_drag_wire() {
        let (mut app, _) = fixture(true);
        let before = app.preferred_height(390.0);
        start_drag(&mut app);
        assert_eq!(app.preferred_height(390.0), before);
    }

    #[test]
    fn preferred_height_ignores_previous_scroll_rect() {
        let (mut app, _) = fixture(true);
        let before = app.preferred_height(390.0);
        app.last_content_rect = Some(eframe::egui::Rect::from_min_size(
            eframe::egui::pos2(8.0, -900.0),
            eframe::egui::vec2(1400.0, 100.0),
        ));
        assert_eq!(app.preferred_height(390.0), before);
    }

    #[test]
    fn preferred_height_tracks_wrapped_palette() {
        let cc = eframe::CreationContext::_new_kittest(eframe::egui::Context::default());
        let startup = EmbedStartup::parse(
            r#"{"cols":[["H"]]}"#,
            true,
            Some(&["H", "X", "Y", "Z", "S", "T", "Measure", "Bloch"]),
            Some(1.0),
        )
        .unwrap();
        let app = super::super::QniApp::new_with_startup(&cc, Some(startup));
        assert!(app.preferred_height(300.0) > app.preferred_height(1000.0));
    }

    #[test]
    fn preferred_height_keeps_fourteen_existing_wires() {
        let column = std::iter::repeat_n("\"H\"", 14)
            .collect::<Vec<_>>()
            .join(",");
        let app = limited(&format!("{{\"cols\":[[{column}]]}}"), 1.0);
        assert!(app.preferred_height(390.0) > 1000.0);
    }

    #[test]
    fn restart_button_is_absent_by_default() {
        let (app, _) = fixture(true);
        assert!(
            !app.state_panel_layout(
                eframe::egui::Rect::from_min_size(
                    eframe::egui::Pos2::ZERO,
                    eframe::egui::vec2(960.0, 640.0)
                ),
                2
            )
            .show_restart_button
        );
    }

    #[test]
    fn restart_callback_reaches_app() {
        let ctx = eframe::egui::Context::default();
        let cc = eframe::CreationContext::_new_kittest(ctx);
        let calls = std::rc::Rc::new(std::cell::Cell::new(0));
        let captured = calls.clone();
        let mut startup = parse_full(r#"{"cols":[["H"]]}"#).unwrap();
        startup.on_restart = Some(Box::new(move || captured.set(captured.get() + 1)));
        let app = super::super::QniApp::new_with_startup(&cc, Some(startup));
        app.on_restart.as_ref().unwrap()();
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn restart_hit_does_not_overlap_dimensions() {
        let (mut app, ctx) = fixture(true);
        app.on_restart = Some(Box::new(|| {}));
        let _ = ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let layout = app.state_panel_layout(ui.max_rect(), 256);
            assert!(
                !super::super::QniApp::dims_hit_rect(&ctx, &layout, eframe::egui::Vec2::ZERO)
                    .intersects(layout.restart_rect(eframe::egui::Vec2::ZERO))
            );
        });
    }

    #[test]
    fn embed_starts_at_step_zero() {
        let (app, _) = fixture(true);
        assert_eq!(app.breakpoint_step, Some(CircuitColumnIndex::ZERO));
    }

    #[test]
    fn standalone_starts_at_final_step() {
        let ctx = eframe::egui::Context::default();
        let app = super::super::QniApp::new(&eframe::CreationContext::_new_kittest(ctx));
        assert_eq!(
            app.breakpoint_step,
            Some(CircuitColumnIndex::new(app.step_count().saturating_sub(1)))
        );
    }

    #[test]
    fn embed_circuit_reload_pins_step_zero() {
        let (mut app, ctx) = fixture(true);
        app.breakpoint_step = Some(CircuitColumnIndex::new(2));
        app.load_circuit_json_into_editor(r#"{"cols":[["X"],["H"],["Z"]]}"#, &ctx);
        assert_eq!(app.breakpoint_step, Some(CircuitColumnIndex::ZERO));
    }

    #[test]
    fn standalone_circuit_reload_pins_final_step() {
        let ctx = eframe::egui::Context::default();
        let mut app =
            super::super::QniApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()));
        app.breakpoint_step = Some(CircuitColumnIndex::ZERO);
        app.load_circuit_json_into_editor(r#"{"cols":[["X"],["H"],["Z"]]}"#, &ctx);
        assert_eq!(app.breakpoint_step, Some(CircuitColumnIndex::new(2)));
    }

    #[test]
    fn standalone_empty_circuit_reload_pins_step_zero() {
        let ctx = eframe::egui::Context::default();
        let mut app =
            super::super::QniApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()));
        app.breakpoint_step = Some(CircuitColumnIndex::new(2));
        app.load_circuit_json_into_editor(r#"{"cols":[]}"#, &ctx);
        assert_eq!(app.breakpoint_step, Some(CircuitColumnIndex::ZERO));
    }

    #[test]
    fn standalone_reload_counts_committed_empty_columns() {
        let ctx = eframe::egui::Context::default();
        let mut app =
            super::super::QniApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()));
        app.load_circuit_json_into_editor(r#"{"cols":[["H"],[],[]]}"#, &ctx);
        assert_eq!(app.breakpoint_step, Some(CircuitColumnIndex::new(2)));
    }

    #[test]
    fn embed_startup_keeps_trimmed_title() {
        let ctx = eframe::egui::Context::default();
        let cc = eframe::CreationContext::_new_kittest(ctx);
        let startup = parse_full(r#"{"cols":[["|0>"]],"title":" Superdense Coding "}"#).unwrap();
        let app = super::super::QniApp::new_with_startup(&cc, Some(startup));
        assert_eq!(app.circuit_title, "Superdense Coding");
    }

    #[test]
    fn loading_untitled_circuit_drops_previous_title() {
        let (mut app, ctx) = fixture(true);
        app.load_circuit_json_into_editor(r#"{"cols":[["H"]],"title":"Bell"}"#, &ctx);
        app.load_circuit_json_into_editor(r#"{"cols":[["X"]]}"#, &ctx);
        assert_eq!(app.circuit_title, "");
    }

    #[test]
    fn undo_restores_title_from_checkpoint() {
        let (mut app, ctx) = fixture(true);
        app.load_circuit_json_into_editor(r#"{"cols":[["H"]],"title":"Bell"}"#, &ctx);
        app.circuit_revision = super::super::CircuitRevision::starting_at(
            r#"{"cols":[["H"]],"title":"Bell"}"#.to_owned(),
        );
        app.circuit_title.clear();
        app.placed_gates.clear();
        app.commit_current_circuit(&ctx);
        app.undo_circuit(&ctx);
        assert_eq!(app.circuit_title, "Bell");
    }

    #[test]
    fn omitted_palette_keeps_full_palette() {
        let (app, _) = fixture(true);
        assert_eq!(app.palette, Palette::Full);
    }

    #[test]
    fn palette_option_reaches_app() {
        let ctx = eframe::egui::Context::default();
        let cc = eframe::CreationContext::_new_kittest(ctx);
        let startup =
            EmbedStartup::parse(r#"{"cols":[["H"]]}"#, true, Some(&["H", "X"][..]), None).unwrap();
        let app = super::super::QniApp::new_with_startup(&cc, Some(startup));
        assert_eq!(app.palette, Palette::restricted(&["H", "X"]).unwrap());
    }

    #[test]
    fn rejects_unknown_palette_gate() {
        assert!(
            EmbedStartup::parse(r#"{"cols":[]}"#, true, Some(&["H", "Foo"][..]), None).is_err()
        );
    }

    #[test]
    fn embed_starts_in_local_mode() {
        let (app, _) = fixture(true);
        assert_eq!(app.exec_mode, super::super::ExecMode::Local);
    }

    #[test]
    fn embed_does_not_persist_egui_memory() {
        let (app, _) = fixture(true);
        assert!(!eframe::App::persist_egui_memory(&app));
    }

    #[test]
    fn hidden_state_panel_option_reaches_app() {
        let (app, _) = fixture(false);
        assert!(!app.state_panel_visible());
    }

    #[test]
    fn embed_ignores_url_payload_updates() {
        let (mut app, ctx) = fixture(true);
        assert!(!app.apply_url_payload(r#"{"cols":[["X"]]}"#.to_owned(), &ctx));
    }

    #[test]
    fn embed_rejects_oversized_library_replacement() {
        let (mut app, ctx) = fixture(true);
        let original = app.library.active().circuit_json.clone();
        let col = std::iter::repeat_n("\"H\"", crate::constants::LOCAL_MAX_QUBITS + 1)
            .collect::<Vec<_>>()
            .join(",");
        app.replace_active_circuit_json_unchecked(&format!("{{\"cols\":[[{col}]]}}"), &ctx);
        assert_eq!(app.library.active().circuit_json, original);
    }
    #[test]
    fn accepts_local_capacity_boundary() {
        let col = std::iter::repeat_n("\"H\"", crate::constants::LOCAL_MAX_QUBITS)
            .collect::<Vec<_>>()
            .join(",");
        assert!(parse_full(&format!("{{\"cols\":[[{col}]]}}")).is_ok());
    }

    #[test]
    fn span_past_local_capacity_is_rejected() {
        let col = std::iter::repeat_n("1", crate::constants::LOCAL_MAX_QUBITS - 1)
            .collect::<Vec<_>>()
            .join(",");
        assert!(parse_full(&format!("{{\"cols\":[[{col},\"QFT2\"]]}}")).is_err());
    }

    #[test]
    fn editing_one_embed_keeps_other_instance_unchanged() {
        let (mut first, ctx) = fixture(true);
        let (second, _) = fixture(true);
        first.replace_active_circuit_json_unchecked(r#"{"cols":[["X"]]}"#, &ctx);
        assert_eq!(second.library.active().circuit_json, r#"{"cols":[["H"]]}"#);
    }

    #[test]
    fn embed_history_remains_editable() {
        let (mut app, ctx) = fixture(true);
        app.placed_gates.clear();
        app.commit_current_circuit(&ctx);
        app.undo_circuit(&ctx);
        assert_eq!(app.library.active().circuit_json, r#"{"cols":[["H"]]}"#);
    }

    fn limited(json: &str, max_wire_count: f64) -> super::super::QniApp {
        let cc = eframe::CreationContext::_new_kittest(eframe::egui::Context::default());
        let startup = EmbedStartup::parse::<&str>(json, true, None, Some(max_wire_count)).unwrap();
        super::super::QniApp::new_with_startup(&cc, Some(startup))
    }

    fn start_drag(app: &mut super::super::QniApp) {
        app.dragging = Some(super::super::DragState {
            id: app.placed_gates[0].id,
            preview_pos: app.placed_gates[0].committed_pos(),
            offset: eframe::egui::Vec2::ZERO,
            original_column: None,
            click_copy: None,
        });
    }

    #[test]
    fn max_wire_count_one_draws_one_wire() {
        assert_eq!(limited(r#"{"cols":[["H"]]}"#, 1.0).layout_qubits(), 1);
    }

    #[test]
    fn max_wire_count_one_adds_no_wire_while_dragging() {
        let mut app = limited(r#"{"cols":[["H"]]}"#, 1.0);
        start_drag(&mut app);
        assert_eq!(app.layout_qubits(), 1);
    }

    #[test]
    fn max_wire_count_allows_extra_drag_wire_below_limit() {
        let mut app = limited(r#"{"cols":[["H"]]}"#, 3.0);
        start_drag(&mut app);
        assert_eq!(app.layout_qubits(), 3);
    }

    #[test]
    fn max_wire_count_keeps_wires_the_circuit_uses() {
        assert_eq!(
            limited(r#"{"cols":[["H","H","H"]]}"#, 1.0).layout_qubits(),
            3
        );
    }

    #[test]
    fn max_wire_count_limits_span_resize_to_existing_wires() {
        assert_eq!(
            limited(r#"{"cols":[["H","H"]]}"#, 1.0)
                .wire_capacity()
                .get(),
            2
        );
    }

    #[test]
    fn omitted_max_wire_count_keeps_two_wires() {
        let (app, _) = fixture(true);
        assert_eq!(app.layout_qubits(), 2);
    }

    #[test]
    fn omitted_max_wire_count_keeps_extra_drag_wire() {
        let (mut app, _) = fixture(true);
        start_drag(&mut app);
        assert_eq!(app.layout_qubits(), 3);
    }

    #[test]
    fn omitted_max_wire_count_keeps_local_capacity() {
        let (app, _) = fixture(true);
        assert_eq!(
            app.wire_capacity(),
            crate::qubit_count::QubitCapacity::local()
        );
    }

    #[test]
    fn standalone_keeps_two_visible_wires() {
        assert_eq!(AppMode::Standalone.min_visible_wire_count(), MIN_QUBITS);
    }

    #[test]
    fn rejects_zero_max_wire_count() {
        assert!(EmbedStartup::parse::<&str>(r#"{"cols":[]}"#, true, None, Some(0.0)).is_err());
    }

    #[test]
    fn rejects_fractional_max_wire_count() {
        assert!(EmbedStartup::parse::<&str>(r#"{"cols":[]}"#, true, None, Some(1.5)).is_err());
    }

    #[test]
    fn rejects_infinite_max_wire_count() {
        assert!(
            EmbedStartup::parse::<&str>(r#"{"cols":[]}"#, true, None, Some(f64::INFINITY)).is_err()
        );
    }

    #[test]
    fn rejects_nan_max_wire_count() {
        assert!(EmbedStartup::parse::<&str>(r#"{"cols":[]}"#, true, None, Some(f64::NAN)).is_err());
    }
}
