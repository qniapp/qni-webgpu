use crate::app::GateId;
use crate::gates::{ColumnControls, GateParams};
use crate::gpu::{Amplitude, Bloch, Density, Measurement, Probability, SlotIndex, Snapshot};
use crate::qubit_bit::QubitBit;

#[derive(Clone, Copy, Debug)]
pub(crate) enum SimulationOp {
    SnapshotState {
        output_slot: SlotIndex<Snapshot>,
    },
    ApplyGate(GateParams),
    Arithmetic(crate::gpu::ArithmeticParams),
    Postselect {
        gate_id: GateId,
        output_slot: SlotIndex<crate::gpu::Postselection>,
    },
    CaptureBloch {
        gate_id: GateId,
        qubit_bit: QubitBit,
        output_slot: SlotIndex<Bloch>,
        controls: ColumnControls,
    },
    MeasureReduceSample {
        /// Internal reset samples must not replace a gate’s displayed outcome.
        visible: bool,
        gate_id: GateId,
        qubit_bit: QubitBit,
        output_slot: SlotIndex<Measurement>,
        controls: ColumnControls,
    },
    MeasureCollapse {
        qubit_bit: QubitBit,
        aux_slot: SlotIndex<Measurement>,
        controls: ColumnControls,
    },
    CaptureProbability {
        gate_id: GateId,
        base_bit: QubitBit,
        span: u32,
        output_slot: SlotIndex<Probability>,
        controls: ColumnControls,
    },
    CaptureAmplitude {
        gate_id: GateId,
        base_bit: QubitBit,
        span: u32,
        output_slot: SlotIndex<Amplitude>,
        controls: ColumnControls,
    },
    CaptureDensity {
        gate_id: GateId,
        base_bit: QubitBit,
        span: u32,
        output_slot: SlotIndex<Density>,
        controls: ColumnControls,
    },
}
