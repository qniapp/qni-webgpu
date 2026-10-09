use super::session::{CircuitInput, PlanChange, RefreshEnvironment, SessionPublication};
use super::test_hooks::wire_external_gpu_test_hooks;
use super::QniApp;
use crate::app::circuit_library;
use eframe::egui;

impl QniApp {
    pub(crate) fn start_external_gpu_run(&mut self, ctx: &egui::Context) {
        if !self.mode.uses_browser_state() {
            return;
        }
        let input = CircuitInput {
            gates: &self.placed_gates,
            qubits: self.external_execution_qubits(),
        };
        if self.external_gpu.start(input, ctx) {
            ctx.request_repaint();
        }
    }

    pub(crate) fn poll_external_gpu_run(&mut self, ctx: &egui::Context) {
        let environment = RefreshEnvironment {
            local_available: self.local_exec_mode_available(),
            mode: self.exec_mode,
        };
        let publication = self.external_gpu.poll(environment);
        self.apply_external_publication(publication, ctx);
    }

    fn apply_external_publication(&mut self, publication: SessionPublication, ctx: &egui::Context) {
        for change in publication.plan_changes {
            match change {
                PlanChange::MarkDirty => self.gpu_plan.mark_dirty(),
                PlanChange::ReplaceExternalSlots(slots) => {
                    self.gpu_plan.replace_external_display_slots(
                        &slots.amplitude,
                        &slots.bloch,
                        &slots.probability,
                        &slots.density,
                        self.state_count(),
                    )
                }
            }
        }
        if publication.repaint {
            ctx.request_repaint();
        }
    }

    pub(crate) fn wire_test_hooks(ctx: &egui::Context) {
        wire_external_gpu_test_hooks(ctx);
        circuit_library::wire_test_hooks(ctx);
    }
}
#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "lifecycle_tests.rs"]
mod lifecycle_tests;
