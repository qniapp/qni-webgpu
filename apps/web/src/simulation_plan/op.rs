use crate::app::GateId;
use crate::gates::{ColumnControls, GateParams};
use crate::gpu::{Amplitude, Bloch, Density, Measurement, Probability, SlotIndex, Snapshot};
use crate::qubit_bit::QubitBit;

/// One step the GPU dispatcher should run during a recompute.
///   * `ApplyGate`: unitary / write gate via `STATE_COMPUTE_SHADER`.
///   * `CaptureBloch`: per-qubit reduction (Bloch x, y, z) via
///     `BLOCH_REDUCE_SHADER`.
///   * `MeasureReduceSample`: pZero reduction + deterministic PCG sample,
///     writes `(pZero, r, outcome, sqrt_p_kept)` to the measurement aux
///     buffer (`MEASURE_REDUCE_SHADER`).
///   * `MeasureCollapse`: per-pair zero+normalize using the previously
///     written aux slot (`MEASURE_COLLAPSE_SHADER`).
///   * `CaptureProbability`: marginalizes the live state into per-outcome
///     probabilities for a Probability display (`PROBABILITY_REDUCE_SHADER`).
///   * `CaptureAmplitude`: captures coherent amplitudes, incoherent
///     magnitudes, quality, and phase-lock metadata for an Amplitude display.
///   * `CaptureDensity`: captures a reduced density matrix for a Density
///     Matrix display.
///   * `SnapshotState`: copies the live state into a GPU-resident step-cache
///     slot. Hovering a circuit column later copies the cached slot into the
///     preview buffer without rerunning the simulation.
#[derive(Clone, Copy, Debug)]
pub(crate) enum SimulationOp {
    SnapshotState {
        output_slot: SlotIndex<Snapshot>,
    },
    ApplyGate(GateParams),
    CaptureBloch {
        gate_id: GateId,
        qubit_bit: QubitBit,
        output_slot: SlotIndex<Bloch>,
        controls: ColumnControls,
    },
    MeasureReduceSample {
        gate_id: GateId,
        qubit_bit: QubitBit,
        output_slot: SlotIndex<Measurement>,
    },
    MeasureCollapse {
        qubit_bit: QubitBit,
        aux_slot: SlotIndex<Measurement>,
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
