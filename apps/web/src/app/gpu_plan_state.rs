//! GPU plan state owned by `QniApp`.
//!
//! This keeps the dirty flag, latest linearised op stream, and per-gate GPU
//! output slots together. The actual simulation still runs only in WebGPU;
//! this state only tracks what the next recompute should dispatch.

use std::collections::HashMap;

use crate::simulation_plan::SimulationOp;

#[derive(Debug)]
pub(crate) struct GpuPlanState {
    needs_recompute: bool,
    last_state_count: usize,
    sim_ops: Vec<SimulationOp>,
    snapshot_slot_count: usize,
    /// gate_id → output_slot mapping derived from the latest `sim_ops` so
    /// the GPU Bloch overlay can pick the right slot in `bloch_output_buffer`.
    bloch_slots: HashMap<u32, u32>,
    /// Same idea for measurement gates → `measurement_aux_buffer` slot.
    measurement_slots: HashMap<u32, u32>,
    /// Probability displays → `probability_output` slot.
    probability_slots: HashMap<u32, u32>,
    /// Amplitude displays → `amplitude_output` slot.
    amplitude_slots: HashMap<u32, u32>,
    /// Density Matrix displays → `density_output` slot.
    density_slots: HashMap<u32, u32>,
    capacity_error: Option<String>,
}

impl Default for GpuPlanState {
    fn default() -> Self {
        Self {
            needs_recompute: true,
            last_state_count: 2,
            sim_ops: Vec::new(),
            snapshot_slot_count: 0,
            bloch_slots: HashMap::new(),
            measurement_slots: HashMap::new(),
            probability_slots: HashMap::new(),
            amplitude_slots: HashMap::new(),
            density_slots: HashMap::new(),
            capacity_error: None,
        }
    }
}

impl GpuPlanState {
    pub(crate) fn mark_dirty(&mut self) {
        self.needs_recompute = true;
        self.bloch_slots.clear();
        self.measurement_slots.clear();
        self.probability_slots.clear();
        self.amplitude_slots.clear();
        self.density_slots.clear();
        self.capacity_error = None;
    }

    pub(crate) fn mark_live_drag_dirty(&mut self) {
        // A snapped drag needs a fresh GPU plan, but the circuit is drawn once
        // before the recompute callback replaces slot maps. Keep the previous
        // display slots so Probability / Amplitude / Density / Bloch blocks
        // keep showing the last GPU result instead of flashing to their static
        // fallback icon for that frame.
        self.needs_recompute = true;
        self.capacity_error = None;
    }

    pub(crate) fn mark_step_preview_dirty(&mut self) {
        // Qni keeps per-step results cached and only switches the displayed
        // snapshot on hover. Do the same: changing the active preview column
        // must not dirty the simulation plan or rerun WebGPU compute.
        self.capacity_error = None;
    }

    pub(crate) fn needs_recompute_for(&self, state_count: usize) -> bool {
        self.needs_recompute || state_count != self.last_state_count
    }

    pub(crate) fn mark_clean_for(&mut self, state_count: usize) {
        self.needs_recompute = false;
        self.last_state_count = state_count;
    }

    pub(crate) fn clear_ops(&mut self) {
        self.sim_ops.clear();
        self.snapshot_slot_count = 0;
        self.bloch_slots.clear();
        self.measurement_slots.clear();
        self.probability_slots.clear();
        self.amplitude_slots.clear();
        self.density_slots.clear();
        self.capacity_error = None;
    }

    pub(crate) fn set_capacity_error(&mut self, message: String) {
        self.sim_ops.clear();
        self.snapshot_slot_count = 0;
        self.bloch_slots.clear();
        self.measurement_slots.clear();
        self.probability_slots.clear();
        self.amplitude_slots.clear();
        self.density_slots.clear();
        self.capacity_error = Some(message);
    }

    pub(crate) fn capacity_error(&self) -> Option<&str> {
        self.capacity_error.as_deref()
    }

    pub(crate) fn replace_ops(&mut self, sim_ops: Vec<SimulationOp>, snapshot_slot_count: usize) {
        self.capacity_error = None;
        self.sim_ops = sim_ops;
        self.snapshot_slot_count = snapshot_slot_count;
        self.rebuild_slot_maps();
    }

    pub(crate) fn replace_external_display_slots(
        &mut self,
        amplitude_slot_to_gate_id: &[u32],
        bloch_slot_to_gate_id: &[u32],
        probability_slot_to_gate_id: &[u32],
        density_slot_to_gate_id: &[u32],
        state_count: usize,
    ) {
        self.needs_recompute = false;
        self.last_state_count = state_count;
        self.sim_ops.clear();
        self.snapshot_slot_count = 0;
        self.bloch_slots.clear();
        self.measurement_slots.clear();
        self.probability_slots.clear();
        self.amplitude_slots.clear();
        self.density_slots.clear();
        self.capacity_error = None;
        for (slot, gate_id) in amplitude_slot_to_gate_id.iter().enumerate() {
            self.amplitude_slots.insert(*gate_id, slot as u32);
        }
        for (slot, gate_id) in bloch_slot_to_gate_id.iter().enumerate() {
            self.bloch_slots.insert(*gate_id, slot as u32);
        }
        for (slot, gate_id) in probability_slot_to_gate_id.iter().enumerate() {
            self.probability_slots.insert(*gate_id, slot as u32);
        }
        for (slot, gate_id) in density_slot_to_gate_id.iter().enumerate() {
            self.density_slots.insert(*gate_id, slot as u32);
        }
    }

    pub(crate) fn snapshot_slot_count(&self) -> usize {
        self.snapshot_slot_count
    }

    pub(crate) fn sim_ops_for_callback(&self, recompute: bool) -> Vec<SimulationOp> {
        if recompute {
            self.sim_ops.clone()
        } else {
            Vec::new()
        }
    }

    pub(crate) fn has_measurement_slot(&self, gate_id: u32) -> bool {
        self.measurement_slots.contains_key(&gate_id)
    }

    pub(crate) fn bloch_slot(&self, gate_id: u32) -> Option<u32> {
        self.bloch_slots.get(&gate_id).copied()
    }

    pub(crate) fn measurement_slot(&self, gate_id: u32) -> Option<u32> {
        self.measurement_slots.get(&gate_id).copied()
    }

    pub(crate) fn probability_slot(&self, gate_id: u32) -> Option<u32> {
        self.probability_slots.get(&gate_id).copied()
    }

    pub(crate) fn amplitude_slot(&self, gate_id: u32) -> Option<u32> {
        self.amplitude_slots.get(&gate_id).copied()
    }

    pub(crate) fn density_slot(&self, gate_id: u32) -> Option<u32> {
        self.density_slots.get(&gate_id).copied()
    }

    fn rebuild_slot_maps(&mut self) {
        self.bloch_slots.clear();
        self.measurement_slots.clear();
        self.probability_slots.clear();
        self.amplitude_slots.clear();
        self.density_slots.clear();
        for op in &self.sim_ops {
            match op {
                SimulationOp::SnapshotState { .. } => {}
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
                SimulationOp::CaptureProbability {
                    gate_id,
                    output_slot,
                    ..
                } => {
                    self.probability_slots.insert(*gate_id, *output_slot);
                }
                SimulationOp::CaptureAmplitude {
                    gate_id,
                    output_slot,
                    ..
                } => {
                    self.amplitude_slots.insert(*gate_id, *output_slot);
                }
                SimulationOp::CaptureDensity {
                    gate_id,
                    output_slot,
                    ..
                } => {
                    self.density_slots.insert(*gate_id, *output_slot);
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GpuPlanState;

    #[test]
    fn step_preview_dirty_keeps_cached_gpu_plan_clean() {
        let mut state = GpuPlanState::default();
        state.mark_clean_for(4);
        state.mark_step_preview_dirty();

        assert!(!state.needs_recompute_for(4));
    }
}
