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
    /// gate_id → output_slot mapping derived from the latest `sim_ops` so
    /// the GPU Bloch overlay can pick the right slot in `bloch_output_buffer`.
    bloch_slots: HashMap<u32, u32>,
    /// Same idea for measurement gates → `measurement_aux_buffer` slot.
    measurement_slots: HashMap<u32, u32>,
    /// Chance displays → `chance_probability_output` slot.
    chance_slots: HashMap<u32, u32>,
    capacity_error: Option<String>,
}

impl Default for GpuPlanState {
    fn default() -> Self {
        Self {
            needs_recompute: true,
            last_state_count: 2,
            sim_ops: Vec::new(),
            bloch_slots: HashMap::new(),
            measurement_slots: HashMap::new(),
            chance_slots: HashMap::new(),
            capacity_error: None,
        }
    }
}

impl GpuPlanState {
    pub(crate) fn mark_dirty(&mut self) {
        self.needs_recompute = true;
        self.bloch_slots.clear();
        self.measurement_slots.clear();
        self.chance_slots.clear();
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
        self.bloch_slots.clear();
        self.measurement_slots.clear();
        self.chance_slots.clear();
        self.capacity_error = None;
    }

    pub(crate) fn set_capacity_error(&mut self, message: String) {
        self.sim_ops.clear();
        self.bloch_slots.clear();
        self.measurement_slots.clear();
        self.chance_slots.clear();
        self.capacity_error = Some(message);
    }

    pub(crate) fn capacity_error(&self) -> Option<&str> {
        self.capacity_error.as_deref()
    }

    pub(crate) fn replace_ops(&mut self, sim_ops: Vec<SimulationOp>) {
        self.capacity_error = None;
        self.sim_ops = sim_ops;
        self.rebuild_slot_maps();
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

    pub(crate) fn chance_slot(&self, gate_id: u32) -> Option<u32> {
        self.chance_slots.get(&gate_id).copied()
    }

    fn rebuild_slot_maps(&mut self) {
        self.bloch_slots.clear();
        self.measurement_slots.clear();
        self.chance_slots.clear();
        for op in &self.sim_ops {
            match op {
                SimulationOp::SnapshotState => {}
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
                SimulationOp::CaptureChance {
                    gate_id,
                    output_slot,
                    ..
                } => {
                    self.chance_slots.insert(*gate_id, *output_slot);
                }
                _ => {}
            }
        }
    }
}
