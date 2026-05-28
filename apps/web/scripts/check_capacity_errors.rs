mod simulation_plan {
    // 単独コンパイル用の slot newtype スタブ。本物の `SlotIndex<F>`
    // (apps/web/src/gpu/slot.rs) はバッファ家族マーカー `F` で型を分けるが、
    // 容量チェック (capacity.rs) は `as_u32()` で数値スロットを読むだけなので、
    // capacity.rs を単独でコンパイルするにはこの非ジェネリックな写しで十分。
    #[derive(Clone, Copy, Debug)]
    pub(crate) struct SlotIndex(u32);

    impl SlotIndex {
        pub(crate) const fn new(value: u32) -> Self {
            Self(value)
        }

        pub(crate) const fn as_u32(self) -> u32 {
            self.0
        }
    }

    #[allow(dead_code)]
    #[derive(Clone, Debug)]
    pub(crate) enum SimulationOp {
        SnapshotState {
            output_slot: SlotIndex,
        },
        ApplyGate(()),
        CaptureBloch {
            gate_id: u32,
            qubit_bit: u32,
            output_slot: SlotIndex,
        },
        MeasureReduceSample {
            gate_id: u32,
            qubit_bit: u32,
            output_slot: SlotIndex,
        },
        MeasureCollapse {
            qubit_bit: u32,
            aux_slot: SlotIndex,
        },
        CaptureProbability {
            gate_id: u32,
            base_bit: u32,
            span: u32,
            output_slot: SlotIndex,
        },
        CaptureAmplitude {
            gate_id: u32,
            base_bit: u32,
            span: u32,
            output_slot: SlotIndex,
            control_mask: u32,
            control_value: u32,
        },
        CaptureDensity {
            gate_id: u32,
            base_bit: u32,
            span: u32,
            output_slot: SlotIndex,
            control_mask: u32,
            control_value: u32,
        },
    }

    pub(crate) mod capacity {
        include!("../src/simulation_plan/capacity.rs");
    }
}

use simulation_plan::capacity::{validate_simulation_plan_capacity, SimulationPlanLimits};
use simulation_plan::{SimulationOp, SlotIndex};

fn tiny_limits() -> SimulationPlanLimits {
    SimulationPlanLimits {
        max_ops_per_variant: 1,
        max_step_snapshot_slots: 1,
        max_bloch_slots: 1,
        max_measurement_slots: 1,
        max_probability_slots: 1,
        max_amplitude_slots: 1,
        max_density_slots: 1,
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
        output_slot: SlotIndex::new(1),
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
        output_slot: SlotIndex::new(1),
    }];
    let error = validate_simulation_plan_capacity(&ops, tiny_limits()).unwrap_err();
    assert_eq!(
        error.to_string(),
        "measurement slot 1 exceeds MAX_MEASUREMENT_SLOTS=1; reduce measurements or grow the GPU buffer",
    );
}

fn probability_slot_limit_reports_buffer_capacity() {
    let ops = vec![SimulationOp::CaptureProbability {
        gate_id: 1,
        base_bit: 0,
        span: 1,
        output_slot: SlotIndex::new(1),
    }];

    let error = validate_simulation_plan_capacity(&ops, tiny_limits()).unwrap_err();
    assert_eq!(
        error.to_string(),
        "Probability slot 1 exceeds MAX_PROBABILITY_SLOTS=1; reduce Probability displays or grow the GPU buffer",
    );
}

fn amplitude_slot_limit_reports_buffer_capacity() {
    let ops = vec![SimulationOp::CaptureAmplitude {
        gate_id: 1,
        base_bit: 0,
        span: 1,
        output_slot: SlotIndex::new(1),
        control_mask: 0,
        control_value: 0,
    }];

    let error = validate_simulation_plan_capacity(&ops, tiny_limits()).unwrap_err();
    assert_eq!(
        error.to_string(),
        "Amplitude slot 1 exceeds MAX_AMPLITUDE_SLOTS=1; reduce Amplitude displays or grow the GPU buffer",
    );
}

fn density_slot_limit_reports_buffer_capacity() {
    let ops = vec![SimulationOp::CaptureDensity {
        gate_id: 1,
        base_bit: 0,
        span: 1,
        output_slot: SlotIndex::new(1),
        control_mask: 0,
        control_value: 0,
    }];

    let error = validate_simulation_plan_capacity(&ops, tiny_limits()).unwrap_err();
    assert_eq!(
        error.to_string(),
        "Density slot 1 exceeds MAX_DENSITY_SLOTS=1; reduce Density displays or grow the GPU buffer",
    );
}

fn main() {
    gate_op_limit_reports_staging_capacity();
    bloch_slot_limit_reports_buffer_capacity();
    measurement_slot_limit_reports_buffer_capacity();
    probability_slot_limit_reports_buffer_capacity();
    amplitude_slot_limit_reports_buffer_capacity();
    density_slot_limit_reports_buffer_capacity();
}
