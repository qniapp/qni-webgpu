mod simulation_plan {
    #[allow(dead_code)]
    #[derive(Clone, Debug)]
    pub(crate) enum SimulationOp {
        SnapshotState {
            output_slot: u32,
        },
        ApplyGate(()),
        CaptureBloch {
            gate_id: u32,
            qubit_bit: u32,
            output_slot: u32,
        },
        MeasureReduceSample {
            gate_id: u32,
            qubit_bit: u32,
            output_slot: u32,
        },
        MeasureCollapse {
            qubit_bit: u32,
            aux_slot: u32,
        },
        CaptureChance {
            gate_id: u32,
            base_bit: u32,
            span: u32,
            output_slot: u32,
        },
        CaptureAmplitude {
            gate_id: u32,
            base_bit: u32,
            span: u32,
            output_slot: u32,
            control_mask: u32,
            control_value: u32,
        },
    }

    pub(crate) mod capacity {
        include!("../src/simulation_plan/capacity.rs");
    }
}

use simulation_plan::capacity::{validate_simulation_plan_capacity, SimulationPlanLimits};
use simulation_plan::SimulationOp;

fn tiny_limits() -> SimulationPlanLimits {
    SimulationPlanLimits {
        max_ops_per_variant: 1,
        max_step_snapshot_slots: 1,
        max_bloch_slots: 1,
        max_measurement_slots: 1,
        max_chance_slots: 1,
        max_amplitude_slots: 1,
    }
}

fn gate_op_limit_reports_staging_capacity() {
    let ops = vec![SimulationOp::ApplyGate(()), SimulationOp::ApplyGate(())];
    let error = validate_simulation_plan_capacity(&ops, tiny_limits()).unwrap_err();
    assert_eq!(
        error.to_string(),
        "gate op count 2 exceeds MAX_OPS_PER_RECOMPUTE=1; split the circuit or grow the GPU staging buffer",
    );
}

fn bloch_slot_limit_reports_buffer_capacity() {
    let ops = vec![SimulationOp::CaptureBloch {
        gate_id: 1,
        qubit_bit: 0,
        output_slot: 1,
    }];
    let error = validate_simulation_plan_capacity(&ops, tiny_limits()).unwrap_err();
    assert_eq!(
        error.to_string(),
        "Bloch slot 1 exceeds MAX_BLOCH_SLOTS=1; reduce Bloch displays or grow the GPU buffer",
    );
}

fn measurement_slot_limit_reports_buffer_capacity() {
    let ops = vec![SimulationOp::MeasureReduceSample {
        gate_id: 1,
        qubit_bit: 0,
        output_slot: 1,
    }];
    let error = validate_simulation_plan_capacity(&ops, tiny_limits()).unwrap_err();
    assert_eq!(
        error.to_string(),
        "measurement slot 1 exceeds MAX_MEASUREMENT_SLOTS=1; reduce measurements or grow the GPU buffer",
    );
}

fn chance_slot_limit_reports_buffer_capacity() {
    let ops = vec![SimulationOp::CaptureChance {
        gate_id: 1,
        base_bit: 0,
        span: 1,
        output_slot: 1,
    }];

    let error = validate_simulation_plan_capacity(&ops, tiny_limits()).unwrap_err();
    assert_eq!(
        error.to_string(),
        "Chance slot 1 exceeds MAX_CHANCE_SLOTS=1; reduce Chance displays or grow the GPU buffer",
    );
}

fn amplitude_slot_limit_reports_buffer_capacity() {
    let ops = vec![SimulationOp::CaptureAmplitude {
        gate_id: 1,
        base_bit: 0,
        span: 1,
        output_slot: 1,
        control_mask: 0,
        control_value: 0,
    }];

    let error = validate_simulation_plan_capacity(&ops, tiny_limits()).unwrap_err();
    assert_eq!(
        error.to_string(),
        "Amplitude slot 1 exceeds MAX_AMPLITUDE_SLOTS=1; reduce Amplitude displays or grow the GPU buffer",
    );
}

fn main() {
    gate_op_limit_reports_staging_capacity();
    bloch_slot_limit_reports_buffer_capacity();
    measurement_slot_limit_reports_buffer_capacity();
    chance_slot_limit_reports_buffer_capacity();
    amplitude_slot_limit_reports_buffer_capacity();
}
