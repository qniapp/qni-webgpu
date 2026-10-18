//! Semantic column selection shared by the UI and embed runner.

use eframe::egui;

use super::{CircuitColumnIndex, QniApp};

impl QniApp {
    pub(crate) fn initial_breakpoint_step(&self, step_count: usize) -> Option<CircuitColumnIndex> {
        Some(if matches!(self.mode, super::AppMode::Standalone) {
            CircuitColumnIndex::new(step_count.saturating_sub(1))
        } else {
            CircuitColumnIndex::ZERO
        })
    }

    /// Embed edits show the complete result rather than the startup snapshot.
    /// Standalone editing retains its explicitly locked step.
    pub(crate) fn follow_embed_edit(&mut self) {
        if matches!(self.mode, super::AppMode::Embed { .. }) {
            self.breakpoint_step = None;
            self.hovered_step = None;
        }
    }

    /// Committed JSON columns, excluding block markers and editor padding.
    pub(crate) fn step_count(&self) -> usize {
        crate::url_circuit::summarize_circuit_json(&self.library.active().circuit_json)
            .map_or(0, |summary| summary.columns)
    }

    /// The effective locked step (-1 before all columns); unselected empty circuits return None.
    pub(crate) fn selected_step(&self) -> Option<i64> {
        if self.breakpoint_step == Some(CircuitColumnIndex::INITIAL) {
            return Some(-1);
        }
        let last = self.step_count().checked_sub(1)?;
        Some(
            self.breakpoint_step
                .map_or(last, |step| step.as_usize().min(last)) as i64,
        )
    }

    pub(crate) fn select_step(&mut self, value: f64, ctx: &egui::Context) -> Result<(), String> {
        let count = self.step_count();
        if !value.is_finite() || value.fract() != 0.0 || value < -1.0 || value >= count as f64 {
            return Err(format!(
                "selectStep index must be an integer in [-1, {count}): {value}"
            ));
        }
        self.hovered_step = None;
        let step = if value == -1.0 {
            CircuitColumnIndex::INITIAL
        } else {
            CircuitColumnIndex::new(value as usize)
        };
        self.lock_step(step, ctx);
        Ok(())
    }

    pub(crate) fn state_vector_update(&self) -> crate::gpu::StateVectorUpdate {
        crate::gpu::StateVectorUpdate {
            sim_ops: self.gpu_plan.sim_ops_for_callback(),
            state_count: self.state_count(),
            plan_revision: self.gpu_plan.revision(),
            measurement_seed: self.measurement_seed,
            preview_step: if self.dragging.is_some() {
                None
            } else {
                self.hovered_step
                    .or(self.breakpoint_step)
                    .map(CircuitColumnIndex::as_usize)
            },
            snapshot_slot_count: self.gpu_plan.snapshot_slot_count(),
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn sync_runner_state(
        &mut self,
        render: &eframe::egui_wgpu::RenderState,
        ctx: &egui::Context,
    ) -> Result<(), wasm_bindgen::JsValue> {
        if !self.local_state_vector_active() || self.dragging.is_some() {
            return Err(wasm_bindgen::JsValue::from_str(
                "state vector readback requires an idle local circuit",
            ));
        }
        let state_count = self.state_count();
        let recompute = self.gpu_plan.needs_recompute_for(state_count);
        self.process_gpu_recompute(Some(render.target_format), recompute, state_count, ctx);
        if let Some(error) = self.gpu_plan.capacity_error() {
            return Err(wasm_bindgen::JsValue::from_str(error));
        }
        self.state_vector_update().submit(render)
    }

    pub(crate) fn lock_step(&mut self, step: CircuitColumnIndex, ctx: &egui::Context) {
        self.breakpoint_step = Some(step);
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(json: &str) -> (QniApp, egui::Context) {
        let ctx = egui::Context::default();
        let cc = eframe::CreationContext::_new_kittest(ctx.clone());
        let startup = crate::app::EmbedStartup::parse(json, true, None::<&[&str]>, None).unwrap();
        (QniApp::new_with_startup(&cc, Some(startup)), ctx)
    }

    #[test]
    fn committed_edit_selects_the_final_state() {
        let (mut app, ctx) = fixture(r#"{"cols":[["H"],["X"]]}"#);
        app.select_step(-1.0, &ctx).unwrap();
        app.placed_gates[1].kind = crate::gates::GateKind::H;
        app.commit_current_circuit(&ctx);
        assert_eq!(app.selected_step(), Some(1));
    }

    #[test]
    fn committed_edit_clears_hover_preview() {
        let (mut app, ctx) = fixture(r#"{"cols":[["H"],["X"]]}"#);
        app.hovered_step = Some(CircuitColumnIndex::ZERO);
        app.placed_gates[1].kind = crate::gates::GateKind::H;
        app.commit_current_circuit(&ctx);
        assert_eq!(app.hovered_step, None);
    }

    #[test]
    fn unchanged_commit_preserves_selected_step() {
        let (mut app, ctx) = fixture(r#"{"cols":[["H"],["X"]]}"#);
        app.select_step(-1.0, &ctx).unwrap();
        app.commit_current_circuit(&ctx);
        assert_eq!(app.selected_step(), Some(-1));
    }

    #[test]
    fn undo_selects_the_restored_final_state() {
        let (mut app, ctx) = fixture(r#"{"cols":[["H"],["X"]]}"#);
        app.placed_gates[1].kind = crate::gates::GateKind::H;
        app.commit_current_circuit(&ctx);
        app.undo_circuit(&ctx);
        assert_eq!(app.selected_step(), Some(1));
    }

    #[test]
    fn redo_selects_the_restored_final_state() {
        let (mut app, ctx) = fixture(r#"{"cols":[["H"],["X"]]}"#);
        app.placed_gates[1].kind = crate::gates::GateKind::H;
        app.commit_current_circuit(&ctx);
        app.undo_circuit(&ctx);
        app.redo_circuit(&ctx);
        assert_eq!(app.selected_step(), Some(1));
    }

    #[test]
    fn standalone_edit_preserves_the_locked_initial_state() {
        let (mut app, ctx) = fixture(r#"{"cols":[["H"],["X"]]}"#);
        app.mode = super::super::AppMode::Standalone;
        app.select_step(-1.0, &ctx).unwrap();
        app.placed_gates[1].kind = crate::gates::GateKind::H;
        app.commit_current_circuit(&ctx);
        assert_eq!(app.breakpoint_step, Some(CircuitColumnIndex::INITIAL));
    }

    #[test]
    fn standalone_url_load_keeps_the_final_column_locked() {
        let (mut app, ctx) = fixture(r#"{"cols":[["H"]]}"#);
        app.mode = super::super::AppMode::Standalone;
        app.apply_url_payload(r#"{"cols":[["H"],["X"]]}"#.to_owned(), &ctx);
        assert_eq!(app.breakpoint_step, Some(CircuitColumnIndex::new(1)));
    }

    #[test]
    fn initial_selection_reports_minus_one() {
        let (mut app, ctx) = fixture(r#"{"cols":[["H"],["X"]]}"#);
        app.select_step(-1.0, &ctx).unwrap();
        assert_eq!(app.selected_step(), Some(-1));
    }

    #[test]
    fn markers_do_not_count_but_empty_columns_do() {
        let (app, _) = fixture(r#"{"cols":[["{Bell"],["H"],[1],["•","X"],["}"]]}"#);
        assert_eq!(app.step_count(), 3);
    }

    #[test]
    fn selection_locks_semantic_column() {
        let (mut app, ctx) = fixture(r#"{"cols":[["H"],["X"]]}"#);
        app.select_step(1.0, &ctx).unwrap();
        assert_eq!(app.breakpoint_step, Some(CircuitColumnIndex::new(1)));
    }

    #[test]
    fn selection_clears_hover() {
        let (mut app, ctx) = fixture(r#"{"cols":[["H"],["X"]]}"#);
        app.hovered_step = Some(CircuitColumnIndex::ZERO);
        app.select_step(1.0, &ctx).unwrap();
        assert_eq!(app.hovered_step, None);
    }

    #[test]
    fn selection_does_not_dirty_cached_simulation() {
        let (mut app, ctx) = fixture(r#"{"cols":[["H"],["X"]]}"#);
        app.gpu_plan.mark_clean_for(2);
        app.select_step(1.0, &ctx).unwrap();
        assert!(!app.gpu_plan.needs_recompute_for(2));
    }

    #[test]
    fn invalid_indices_are_rejected() {
        let (mut app, ctx) = fixture(r#"{"cols":[["H"],["X"]]}"#);
        let results: Vec<_> = [-2.0, 0.5, 2.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY]
            .into_iter()
            .map(|value| app.select_step(value, &ctx).is_err())
            .collect();
        assert_eq!(results, vec![true; 6]);
    }

    #[test]
    fn invalid_indices_leave_selection_unchanged() {
        let (mut app, ctx) = fixture(r#"{"cols":[["H"],["X"]]}"#);
        for value in [-2.0, 0.5, 2.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let _ = app.select_step(value, &ctx);
        }
        assert_eq!(app.selected_step(), Some(0));
    }

    #[test]
    fn empty_circuit_rejects_selection() {
        let (mut app, ctx) = fixture(r#"{"cols":[]}"#);
        assert!(app.select_step(0.0, &ctx).is_err());
    }

    #[test]
    fn empty_circuit_accepts_initial_selection() {
        let (mut app, ctx) = fixture(r#"{"cols":[]}"#);
        app.select_step(-1.0, &ctx).unwrap();
        assert_eq!(app.selected_step(), Some(-1));
    }

    #[test]
    fn empty_circuit_has_no_selected_step() {
        let (app, _) = fixture(r#"{"cols":[]}"#);
        assert_eq!(app.selected_step(), None);
    }

    #[test]
    fn selection_preserves_capacity_errors() {
        let (mut app, ctx) = fixture(r#"{"cols":[["H"]]}"#);
        app.gpu_plan
            .set_capacity_error("too many snapshots".to_owned());
        app.select_step(0.0, &ctx).unwrap();
        assert_eq!(app.gpu_plan.capacity_error(), Some("too many snapshots"));
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn cpu_clean_plan_still_carries_new_revision() {
        let (mut app, ctx) = fixture(r#"{"cols":[["H"]]}"#);
        let before = app.state_vector_update().plan_revision;
        app.test_process_gpu_recompute(
            Some(eframe::wgpu::TextureFormat::Rgba8Unorm),
            true,
            2,
            &ctx,
        );
        assert!(app.state_vector_update().plan_revision > before);
    }

    #[test]
    fn hover_preview_does_not_change_locked_selection() {
        let (mut app, ctx) = fixture(r#"{"cols":[["H"],["X"]]}"#);
        app.select_step(1.0, &ctx).unwrap();
        app.hovered_step = Some(CircuitColumnIndex::ZERO);
        assert_eq!(app.selected_step(), Some(1));
    }

    #[test]
    fn hover_preview_selects_the_displayed_state() {
        let (mut app, ctx) = fixture(r#"{"cols":[["H"],["X"]]}"#);
        app.select_step(1.0, &ctx).unwrap();
        app.hovered_step = Some(CircuitColumnIndex::ZERO);
        assert_eq!(app.state_vector_update().preview_step, Some(0));
    }

    #[test]
    fn padded_ui_selection_reports_effective_last_step() {
        let (mut app, ctx) = fixture(r#"{"cols":[["H"]]}"#);
        app.lock_step(CircuitColumnIndex::new(9), &ctx);
        assert_eq!(app.selected_step(), Some(0));
    }

    #[test]
    fn initial_selection_does_not_dirty_cached_simulation() {
        let (mut app, ctx) = fixture(r#"{"cols":[["H"],["X"]]}"#);
        app.gpu_plan.mark_clean_for(2);
        app.select_step(-1.0, &ctx).unwrap();
        assert!(!app.gpu_plan.needs_recompute_for(2));
    }

    #[test]
    fn initial_hover_selects_initial_gpu_preview() {
        let (mut app, _) = fixture(r#"{"cols":[["H"],["X"]]}"#);
        app.hovered_step = Some(CircuitColumnIndex::INITIAL);
        assert_eq!(app.state_vector_update().preview_step, Some(usize::MAX));
    }
}
