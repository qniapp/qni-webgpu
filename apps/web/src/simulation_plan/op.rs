use crate::gates::{ColumnControls, GateParams};
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
        output_slot: u32,
    },
    ApplyGate(GateParams),
    CaptureBloch {
        gate_id: u32,
        qubit_bit: QubitBit,
        output_slot: u32,
        controls: ColumnControls,
    },
    MeasureReduceSample {
        gate_id: u32,
        qubit_bit: QubitBit,
        output_slot: u32,
    },
    MeasureCollapse {
        qubit_bit: QubitBit,
        aux_slot: u32,
    },
    CaptureProbability {
        gate_id: u32,
        base_bit: QubitBit,
        span: u32,
        output_slot: u32,
        controls: ColumnControls,
    },
    CaptureAmplitude {
        gate_id: u32,
        base_bit: QubitBit,
        span: u32,
        output_slot: u32,
        controls: ColumnControls,
    },
    CaptureDensity {
        gate_id: u32,
        base_bit: QubitBit,
        span: u32,
        output_slot: u32,
        controls: ColumnControls,
    },
}
