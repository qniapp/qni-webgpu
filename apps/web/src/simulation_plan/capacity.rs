use super::SimulationOp;

#[derive(Clone, Copy, Debug)]
pub(crate) struct SimulationPlanLimits {
    pub(crate) max_ops_per_variant: usize,
    pub(crate) max_step_snapshot_slots: usize,
    pub(crate) max_bloch_slots: usize,
    pub(crate) max_measurement_slots: usize,
    pub(crate) max_probability_slots: usize,
    pub(crate) max_amplitude_slots: usize,
    pub(crate) max_density_slots: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SimulationPlanCapacityError {
    message: String,
}

impl SimulationPlanCapacityError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for SimulationPlanCapacityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

pub(crate) fn validate_simulation_plan_capacity(
    ops: &[SimulationOp],
    limits: SimulationPlanLimits,
) -> Result<(), SimulationPlanCapacityError> {
    let mut gate_ops = 0usize;
    let mut bloch_ops = 0usize;
    let mut measure_reduce_ops = 0usize;
    let mut measure_collapse_ops = 0usize;
    let mut probability_ops = 0usize;
    let mut amplitude_ops = 0usize;
    let mut density_ops = 0usize;
    for op in ops {
        match op {
            SimulationOp::SnapshotState { output_slot } => {
                let slot = output_slot.as_u32() as usize;
                if slot >= limits.max_step_snapshot_slots {
                    return Err(SimulationPlanCapacityError::new(format!(
                        "step snapshot slot {slot} exceeds MAX_STEP_SNAPSHOT_SLOTS={}; reduce sparse columns or grow the GPU snapshot cache",
                        limits.max_step_snapshot_slots
                    )));
                }
            }
            SimulationOp::ApplyGate(_) => gate_ops += 1,
            SimulationOp::CaptureBloch { output_slot, .. } => {
                bloch_ops += 1;
                let slot = output_slot.as_u32() as usize;
                if slot >= limits.max_bloch_slots {
                    return Err(SimulationPlanCapacityError::new(format!(
                        "Bloch slot {slot} exceeds MAX_BLOCH_SLOTS={}; reduce Bloch displays or grow the GPU buffer",
                        limits.max_bloch_slots
                    )));
                }
            }
            SimulationOp::MeasureReduceSample { output_slot, .. } => {
                measure_reduce_ops += 1;
                let slot = output_slot.as_u32() as usize;
                if slot >= limits.max_measurement_slots {
                    return Err(SimulationPlanCapacityError::new(format!(
                        "measurement slot {slot} exceeds MAX_MEASUREMENT_SLOTS={}; reduce measurements or grow the GPU buffer",
                        limits.max_measurement_slots
                    )));
                }
            }
            SimulationOp::MeasureCollapse { aux_slot, .. } => {
                measure_collapse_ops += 1;
                let slot = aux_slot.as_u32() as usize;
                if slot >= limits.max_measurement_slots {
                    return Err(SimulationPlanCapacityError::new(format!(
                        "measurement collapse slot {slot} exceeds MAX_MEASUREMENT_SLOTS={}; reduce measurements or grow the GPU buffer",
                        limits.max_measurement_slots
                    )));
                }
            }
            SimulationOp::CaptureProbability { output_slot, .. } => {
                probability_ops += 1;
                let slot = output_slot.as_u32() as usize;
                if slot >= limits.max_probability_slots {
                    return Err(SimulationPlanCapacityError::new(format!(
                        "Probability slot {slot} exceeds MAX_PROBABILITY_SLOTS={}; reduce Probability displays or grow the GPU buffer",
                        limits.max_probability_slots
                    )));
                }
            }
            SimulationOp::CaptureAmplitude { output_slot, .. } => {
                amplitude_ops += 1;
                let slot = output_slot.as_u32() as usize;
                if slot >= limits.max_amplitude_slots {
                    return Err(SimulationPlanCapacityError::new(format!(
                        "Amplitude slot {slot} exceeds MAX_AMPLITUDE_SLOTS={}; reduce Amplitude displays or grow the GPU buffer",
                        limits.max_amplitude_slots
                    )));
                }
            }
            SimulationOp::CaptureDensity { output_slot, .. } => {
                density_ops += 1;
                let slot = output_slot.as_u32() as usize;
                if slot >= limits.max_density_slots {
                    return Err(SimulationPlanCapacityError::new(format!(
                        "Density slot {slot} exceeds MAX_DENSITY_SLOTS={}; reduce Density displays or grow the GPU buffer",
                        limits.max_density_slots
                    )));
                }
            }
        }
    }
    for (label, count) in [
        ("gate", gate_ops),
        ("bloch", bloch_ops),
        ("measure_reduce", measure_reduce_ops),
        ("measure_collapse", measure_collapse_ops),
        ("probability", probability_ops),
        ("amplitude", amplitude_ops),
        ("density", density_ops),
    ] {
        if count > limits.max_ops_per_variant {
            return Err(SimulationPlanCapacityError::new(format!(
                "{label} op count {count} exceeds MAX_OPS_PER_RECOMPUTE={}; split the circuit or grow the GPU staging buffer",
                limits.max_ops_per_variant
            )));
        }
    }
    Ok(())
}
