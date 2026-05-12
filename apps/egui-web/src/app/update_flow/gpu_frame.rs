use eframe::egui;

use crate::app::QniApp;
use crate::gpu::{MAX_BLOCH_SLOTS, MAX_MEASUREMENT_SLOTS, MAX_OPS_PER_RECOMPUTE};
use crate::simulation_plan::{
    linearize_ops, validate_simulation_plan_capacity, SimulationPlanLimits,
};

impl QniApp {
    /// Refresh the simulation operation list + per-gate slot lookups when
    /// something changed (qubits added, gates rearranged, etc.). Returns the
    /// recompute flag to pass into rendering; false if we punted because the GPU
    /// target wasn't ready yet.
    pub(super) fn process_gpu_recompute(
        &mut self,
        target_format: Option<eframe::wgpu::TextureFormat>,
        recompute: bool,
        state_count: usize,
        ctx: &egui::Context,
    ) -> bool {
        if target_format.is_some() {
            if recompute {
                self.gpu_plan.mark_clean_for(state_count);
                let qubits = self.state_qubits();
                // Hovered wins over breakpoint (live preview); `None` for both
                // = apply every column = final state.
                let step_limit = self.hovered_step.or(self.breakpoint_step);
                let sim_ops = linearize_ops(&self.placed_gates, qubits, step_limit);
                if let Err(error) = validate_simulation_plan_capacity(
                    &sim_ops,
                    SimulationPlanLimits {
                        max_ops_per_variant: MAX_OPS_PER_RECOMPUTE,
                        max_bloch_slots: MAX_BLOCH_SLOTS,
                        max_measurement_slots: MAX_MEASUREMENT_SLOTS,
                    },
                ) {
                    self.log_gpu_plan_capacity_error(&error.to_string());
                    self.gpu_plan.clear_ops();
                    return recompute;
                }
                self.gpu_plan.replace_ops(sim_ops);
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
