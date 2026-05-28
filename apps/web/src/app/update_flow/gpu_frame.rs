use std::borrow::Cow;

use eframe::egui;

use crate::app::{LiveDragSnap, PlacedGate, QniApp};
use crate::gpu::{
    MAX_AMPLITUDE_SLOTS, MAX_BLOCH_SLOTS, MAX_DENSITY_SLOTS, MAX_MEASUREMENT_SLOTS,
    MAX_OPS_PER_RECOMPUTE, MAX_PROBABILITY_SLOTS, MAX_STEP_SNAPSHOT_SLOTS,
};
use crate::layout::gate_width_cols;
use crate::simulation_plan::{
    linearize_ops, validate_simulation_plan_capacity, SimulationPlanLimits,
};

impl QniApp {
    fn gpu_plan_gates(&self) -> Cow<'_, [PlacedGate]> {
        let Some(drag) = self.dragging else {
            return Cow::Borrowed(&self.placed_gates);
        };
        let Some(LiveDragSnap::Insert {
            column: insert_index,
            wire,
        }) = self.dragging_live_snap
        else {
            return Cow::Borrowed(&self.placed_gates);
        };
        let Some(gate_index) = self.placed_gates.iter().position(|gate| gate.id == drag.id) else {
            return Cow::Borrowed(&self.placed_gates);
        };

        let mut gates = self.placed_gates.clone();
        // Mirror `insert_gate_at_column` for GPU planning only: drawing and URL
        // state stay on the transient drag preview until the drop commits.
        let moving_width = gate_width_cols(gates[gate_index].kind, gates[gate_index].span.get());
        let mut adjusted_insert = insert_index;
        if let Some(old_column) = drag.original_column {
            let old_column_still_occupied = gates
                .iter()
                .any(|gate| gate.id != drag.id && gate.column == old_column);
            if !old_column_still_occupied {
                for gate in &mut gates {
                    if gate.id != drag.id && gate.column > old_column {
                        gate.column = gate.column.saturating_sub(moving_width);
                    }
                }
                if old_column < adjusted_insert {
                    adjusted_insert = adjusted_insert.saturating_sub(moving_width);
                }
            }
        }

        if gates.iter().any(|gate| {
            gate.id != drag.id
                && gate.column >= adjusted_insert
                && gate.column.checked_add(moving_width).is_none()
        }) {
            return Cow::Borrowed(&self.placed_gates);
        }
        for gate in &mut gates {
            if gate.id != drag.id && gate.column >= adjusted_insert {
                let Some(column) = gate.column.checked_add(moving_width) else {
                    return Cow::Borrowed(&self.placed_gates);
                };
                gate.column = column;
            }
        }

        let capacity = self.exec_mode.qubit_capacity();
        let gate = &mut gates[gate_index];
        gate.column = adjusted_insert;
        gate.wire = wire;
        gate.clamp_span_to_qubit_capacity(capacity);
        Cow::Owned(gates)
    }

    fn step_snapshot_slot_count_for(gates: &[PlacedGate]) -> usize {
        gates
            .iter()
            .filter_map(|gate| {
                gate.column
                    .checked_add(1)
                    .map(crate::app::CircuitColumnIndex::as_usize)
            })
            .max()
            .unwrap_or(0)
    }

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
        let external_gpu_state_refresh = self.external_gpu_state_refresh_pending
            && self.local_exec_mode_available()
            && !self.local_state_vector_active();
        let state_vector_active = self.local_state_vector_active() || external_gpu_state_refresh;
        if !state_vector_active {
            if recompute {
                self.gpu_plan.clear_ops();
                self.publish_gpu_plan_capacity_error(None);
                self.gpu_plan.mark_clean_for(state_count);
            }
            return false;
        }
        if target_format.is_some() {
            if recompute {
                self.gpu_plan.mark_clean_for(state_count);
                let qubits = self.state_qubits();
                let gpu_gates = self.gpu_plan_gates();
                // Cache every semantic step snapshot on the GPU, qni-style.
                // Hover / breakpoint changes later select a cached slot via
                // copy-only preview updates instead of rerunning simulation.
                let snapshot_slot_count = Self::step_snapshot_slot_count_for(gpu_gates.as_ref());
                if snapshot_slot_count > MAX_STEP_SNAPSHOT_SLOTS {
                    let message = format!(
                        "step snapshot slot count {snapshot_slot_count} exceeds MAX_STEP_SNAPSHOT_SLOTS={MAX_STEP_SNAPSHOT_SLOTS}; reduce sparse columns or grow the GPU snapshot cache"
                    );
                    self.log_gpu_plan_capacity_error(&message);
                    self.publish_gpu_plan_capacity_error(Some(&message));
                    self.gpu_plan.set_capacity_error(message);
                    return false;
                }
                let sim_ops = linearize_ops(gpu_gates.as_ref(), qubits, snapshot_slot_count);
                if let Err(error) = validate_simulation_plan_capacity(
                    &sim_ops,
                    SimulationPlanLimits {
                        max_ops_per_variant: MAX_OPS_PER_RECOMPUTE,
                        max_step_snapshot_slots: MAX_STEP_SNAPSHOT_SLOTS,
                        max_bloch_slots: MAX_BLOCH_SLOTS,
                        max_measurement_slots: MAX_MEASUREMENT_SLOTS,
                        max_probability_slots: MAX_PROBABILITY_SLOTS,
                        max_amplitude_slots: MAX_AMPLITUDE_SLOTS,
                        max_density_slots: MAX_DENSITY_SLOTS,
                    },
                ) {
                    let message = error.to_string();
                    self.log_gpu_plan_capacity_error(&message);
                    self.publish_gpu_plan_capacity_error(Some(&message));
                    self.gpu_plan.set_capacity_error(message);
                    return false;
                }
                self.gpu_plan.replace_ops(sim_ops, snapshot_slot_count);
                self.publish_gpu_plan_capacity_error(None);
                if external_gpu_state_refresh {
                    self.external_gpu_state_refresh_pending = false;
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
            let js_message = wasm_bindgen::JsValue::from_str(&message);
            web_sys::console::error_1(&js_message);
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = message;
        }
    }

    pub(crate) fn clear_gpu_plan_capacity_error(&self) {
        self.publish_gpu_plan_capacity_error(None);
    }

    fn publish_gpu_plan_capacity_error(&self, message: Option<&str>) {
        #[cfg(all(target_arch = "wasm32", debug_assertions))]
        {
            let value = message
                .map(wasm_bindgen::JsValue::from_str)
                .unwrap_or(wasm_bindgen::JsValue::NULL);
            crate::test_hooks::set_window_value(
                crate::test_hooks::QNI_GPU_PLAN_CAPACITY_ERROR,
                &value,
            );
        }
        #[cfg(any(not(target_arch = "wasm32"), not(debug_assertions)))]
        {
            let _ = message;
        }
    }
}
