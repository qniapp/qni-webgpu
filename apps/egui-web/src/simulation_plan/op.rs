use crate::gates::GateParams;

/// One step the GPU dispatcher should run during a recompute.
///   * `ApplyGate`: unitary / write gate via `STATE_COMPUTE_SHADER`.
///   * `CaptureBloch`: per-qubit reduction (Bloch x, y, z) via
///     `BLOCH_REDUCE_SHADER`.
///   * `MeasureReduceSample`: pZero reduction + deterministic PCG sample,
///     writes `(pZero, r, outcome, sqrt_p_kept)` to the measurement aux
///     buffer (`MEASURE_REDUCE_SHADER`).
///   * `MeasureCollapse`: per-pair zero+normalize using the previously
///     written aux slot (`MEASURE_COLLAPSE_SHADER`).
///   * `CaptureChance`: marginalizes the live state into per-outcome
///     probabilities for a Chance display (`CHANCE_REDUCE_SHADER`).
///   * `CaptureAmplitude`: captures coherent amplitudes, incoherent
///     magnitudes, quality, and phase-lock metadata for an Amplitude display.
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
