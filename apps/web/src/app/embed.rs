//! Per-instance embedding options and browser-state isolation.

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
}

impl EmbedStartup {
    pub(crate) fn parse(circuit_json: &str, show_state_panel: bool) -> Result<Self, &'static str> {
        let summary = crate::url_circuit::summarize_circuit_json(circuit_json)
            .ok_or("invalid circuit JSON: expected {\"cols\":[...]} with supported gates")?;
        if summary.qubits > crate::constants::LOCAL_MAX_QUBITS {
            return Err("circuit exceeds local WebGPU qubit capacity");
        }
        Ok(Self {
            circuit_json: circuit_json.to_owned(),
            mode: AppMode::Embed { show_state_panel },
        })
    }
}

impl super::QniApp {
    pub(crate) fn accepts_circuit_json(&self, json: &str) -> bool {
        self.mode.uses_browser_state()
            || EmbedStartup::parse(json, self.mode.shows_state_panel()).is_ok()
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

    #[test]
    fn accepts_empty_circuit() {
        assert!(EmbedStartup::parse(r#"{"cols":[]}"#, true).is_ok());
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(EmbedStartup::parse("not json", true).is_err());
    }

    #[test]
    fn rejects_unknown_gate() {
        assert!(EmbedStartup::parse(r#"{"cols":[["unknown"]]}"#, true).is_err());
    }

    #[test]
    fn rejects_over_local_capacity() {
        let column = std::iter::repeat_n("\"H\"", crate::constants::LOCAL_MAX_QUBITS + 1)
            .collect::<Vec<_>>()
            .join(",");
        assert!(EmbedStartup::parse(&format!("{{\"cols\":[[{column}]]}}"), true).is_err());
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
        let startup = EmbedStartup::parse(r#"{"cols":[["H"]]}"#, show_state_panel).unwrap();
        (
            super::super::QniApp::new_with_startup(&cc, Some(startup)),
            ctx,
        )
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
        assert!(EmbedStartup::parse(&format!("{{\"cols\":[[{col}]]}}"), true).is_ok());
    }

    #[test]
    fn span_past_local_capacity_is_rejected() {
        let col = std::iter::repeat_n("1", crate::constants::LOCAL_MAX_QUBITS - 1)
            .collect::<Vec<_>>()
            .join(",");
        assert!(EmbedStartup::parse(&format!("{{\"cols\":[[{col},\"QFT2\"]]}}"), true).is_err());
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
