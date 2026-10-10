//! Per-instance embedding options and browser-state isolation.

use super::CircuitColumnIndex;
use crate::layout::Palette;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum AppMode {
    #[default]
    Standalone,
    Embed {
        show_state_panel: bool,
    },
}

impl AppMode {
    pub(crate) fn uses_browser_state(self) -> bool {
        matches!(self, Self::Standalone)
    }

    /// Step shown after a circuit loads. Embeds start at step 0 like qni's
    /// tutorial simulator; the standalone editor shows the final state.
    pub(crate) fn initial_breakpoint_step(self) -> Option<CircuitColumnIndex> {
        match self {
            Self::Standalone => None,
            Self::Embed { .. } => Some(CircuitColumnIndex::ZERO),
        }
    }

    pub(crate) fn shows_state_panel(self) -> bool {
        !matches!(
            self,
            Self::Embed {
                show_state_panel: false
            }
        )
    }
}

pub(crate) struct EmbedStartup {
    pub(crate) circuit_json: String,
    pub(crate) mode: AppMode,
    pub(crate) palette: Palette,
}

impl EmbedStartup {
    /// `palette` lists gate tokens as qni's `mini_qni` filter arguments do
    /// (`["|0>", "|1>", "H"]`); `None` keeps the full palette.
    pub(crate) fn parse<S: AsRef<str>>(
        circuit_json: &str,
        show_state_panel: bool,
        palette: Option<&[S]>,
    ) -> Result<Self, String> {
        validate_circuit_json(circuit_json)?;
        let palette = palette.map(Palette::restricted).transpose()?;
        Ok(Self {
            circuit_json: circuit_json.to_owned(),
            mode: AppMode::Embed { show_state_panel },
            palette: palette.unwrap_or_default(),
        })
    }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_full(json: &str) -> Result<EmbedStartup, String> {
        EmbedStartup::parse::<&str>(json, true, None)
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
            show_state_panel: true
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
            show_state_panel: false
        }
        .shows_state_panel());
    }
    fn fixture(show_state_panel: bool) -> (super::super::QniApp, eframe::egui::Context) {
        let ctx = eframe::egui::Context::default();
        let cc = eframe::CreationContext::_new_kittest(ctx.clone());
        let startup =
            EmbedStartup::parse::<&str>(r#"{"cols":[["H"]]}"#, show_state_panel, None).unwrap();
        (
            super::super::QniApp::new_with_startup(&cc, Some(startup)),
            ctx,
        )
    }

    #[test]
    fn embed_starts_at_step_zero() {
        let (app, _) = fixture(true);
        assert_eq!(app.breakpoint_step, Some(CircuitColumnIndex::ZERO));
    }

    #[test]
    fn standalone_starts_with_final_state() {
        let ctx = eframe::egui::Context::default();
        let app = super::super::QniApp::new(&eframe::CreationContext::_new_kittest(ctx));
        assert_eq!(app.breakpoint_step, None);
    }

    #[test]
    fn embed_circuit_reload_pins_step_zero() {
        let (mut app, ctx) = fixture(true);
        app.breakpoint_step = Some(CircuitColumnIndex::new(2));
        app.load_circuit_json_into_editor(r#"{"cols":[["X"],["H"],["Z"]]}"#, &ctx);
        assert_eq!(app.breakpoint_step, Some(CircuitColumnIndex::ZERO));
    }

    #[test]
    fn standalone_circuit_reload_shows_final_state() {
        let ctx = eframe::egui::Context::default();
        let mut app =
            super::super::QniApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()));
        app.breakpoint_step = Some(CircuitColumnIndex::new(2));
        app.load_circuit_json_into_editor(r#"{"cols":[["X"],["H"],["Z"]]}"#, &ctx);
        assert_eq!(app.breakpoint_step, None);
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
            EmbedStartup::parse(r#"{"cols":[["H"]]}"#, true, Some(&["H", "X"][..])).unwrap();
        let app = super::super::QniApp::new_with_startup(&cc, Some(startup));
        assert_eq!(app.palette, Palette::restricted(&["H", "X"]).unwrap());
    }

    #[test]
    fn rejects_unknown_palette_gate() {
        assert!(EmbedStartup::parse(r#"{"cols":[]}"#, true, Some(&["H", "Foo"][..])).is_err());
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
}
