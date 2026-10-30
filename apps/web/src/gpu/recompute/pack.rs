use eframe::wgpu;

use crate::app::GateId;
use crate::gates::GateParams;
use crate::simulation_plan::SimulationOp;

use super::super::params::{
    AmplitudeCaptureParams, BlochParams, DensityCaptureParams, MeasureCollapseParams,
    MeasureReduceParams, ProbabilityReduceParams,
};
use super::super::resources::StateVectorResources;

/// ランナーがこの実行のあいだ保持する測定シード。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub(crate) struct RunnerMeasurementSeed(u32);

impl RunnerMeasurementSeed {
    pub(crate) const fn new(seed: u32) -> Self {
        Self(seed)
    }

    const fn bits(self) -> u32 {
        self.0
    }
}

/// 1 回の標本測定に渡すシード。ランナーシード、ゲート id、非表示フラグだけを取る。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MeasurementSampleSeed(u32);

impl MeasurementSampleSeed {
    fn new(runner_seed: RunnerMeasurementSeed, gate_id: GateId, hidden: bool) -> Self {
        Self(gate_id.as_u32() ^ runner_seed.bits() ^ if hidden { 0x8000_0000 } else { 0 })
    }

    fn as_u32(self) -> u32 {
        self.0
    }
}

pub(super) struct PackedRecomputeParams {
    gate: Vec<GateParams>,
    arithmetic: Vec<super::super::params::ArithmeticParams>,
    postselection: Vec<super::super::params::PostselectionParams>,
    bloch: Vec<BlochParams>,
    measure_reduce: Vec<MeasureReduceParams>,
    measure_collapse: Vec<MeasureCollapseParams>,
    probability: Vec<ProbabilityReduceParams>,
    amplitude: Vec<AmplitudeCaptureParams>,
    density: Vec<DensityCaptureParams>,
}

impl PackedRecomputeParams {
    pub(super) fn from_ops(
        sim_ops: &[SimulationOp],
        state_count: usize,
        pair_count: u32,
        runner_seed: RunnerMeasurementSeed,
    ) -> Self {
        let mut packed = Self {
            gate: Vec::with_capacity(sim_ops.len()),
            arithmetic: Vec::new(),
            postselection: Vec::new(),
            bloch: Vec::with_capacity(sim_ops.len()),
            measure_reduce: Vec::with_capacity(sim_ops.len()),
            measure_collapse: Vec::with_capacity(sim_ops.len()),
            probability: Vec::with_capacity(sim_ops.len()),
            amplitude: Vec::with_capacity(sim_ops.len()),
            density: Vec::with_capacity(sim_ops.len()),
        };

        for op in sim_ops {
            match op {
                SimulationOp::Postselect { output_slot, .. } => {
                    packed
                        .postselection
                        .push(super::super::params::PostselectionParams {
                            state_count: state_count as u32,
                            output_slot: output_slot.as_u32(),
                            _pad: [0; 2],
                        })
                }
                SimulationOp::SnapshotState { .. } => {}
                SimulationOp::ApplyGate(params) => packed.gate.push(*params),
                SimulationOp::Arithmetic(params) => packed.arithmetic.push(*params),
                SimulationOp::CaptureBloch {
                    qubit_bit,
                    output_slot,
                    controls,
                    ..
                } => packed.bloch.push(BlochParams {
                    qubit_bit: qubit_bit.as_u32(),
                    state_count: state_count as u32,
                    output_slot: output_slot.as_u32(),
                    control_mask: controls.mask(),
                    control_value: controls.value(),
                    _pad: [0; 3],
                }),
                SimulationOp::MeasureReduceSample {
                    visible,
                    gate_id,
                    qubit_bit,
                    output_slot,
                    controls,
                } => packed.measure_reduce.push(MeasureReduceParams {
                    qubit_bit: qubit_bit.as_u32(),
                    state_count: state_count as u32,
                    output_slot: output_slot.as_u32(),
                    seed: MeasurementSampleSeed::new(runner_seed, *gate_id, !*visible).as_u32(),
                    control_mask: controls.mask(),
                    control_value: controls.value(),
                    _pad: [0; 2],
                }),
                SimulationOp::MeasureCollapse {
                    qubit_bit,
                    aux_slot,
                    controls,
                } => {
                    if pair_count == 0 {
                        continue;
                    }
                    packed.measure_collapse.push(MeasureCollapseParams {
                        qubit_bit: qubit_bit.as_u32(),
                        state_count: state_count as u32,
                        aux_slot: aux_slot.as_u32(),
                        _pad: 0,
                        control_mask: controls.mask(),
                        control_value: controls.value(),
                        _pad_controls: [0; 2],
                    });
                }
                SimulationOp::CaptureProbability {
                    base_bit,
                    span,
                    output_slot,
                    controls,
                    ..
                } => {
                    let rest_count = (state_count as u32) >> *span;
                    packed.probability.push(ProbabilityReduceParams {
                        base_bit: base_bit.as_u32(),
                        span: *span,
                        rest_count,
                        output_slot: output_slot.as_u32(),
                        control_mask: controls.mask(),
                        control_value: controls.value(),
                        _pad: [0; 2],
                    });
                }
                SimulationOp::CaptureAmplitude {
                    base_bit,
                    span,
                    output_slot,
                    controls,
                    ..
                } => {
                    let total_qubits = usize::BITS - state_count.leading_zeros() - 1;
                    packed.amplitude.push(AmplitudeCaptureParams {
                        base_bit: base_bit.as_u32(),
                        span: *span,
                        output_slot: output_slot.as_u32(),
                        state_count: state_count as u32,
                        control_mask: controls.mask(),
                        control_value: controls.value(),
                        phase_lock_enabled: u32::from(*span != total_qubits),
                        total_qubits,
                    });
                }
                SimulationOp::CaptureDensity {
                    base_bit,
                    span,
                    output_slot,
                    controls,
                    ..
                } => {
                    packed.density.push(DensityCaptureParams {
                        base_bit: base_bit.as_u32(),
                        span: *span,
                        output_slot: output_slot.as_u32(),
                        state_count: state_count as u32,
                        control_mask: controls.mask(),
                        control_value: controls.value(),
                        _pad: [0; 2],
                    });
                }
            }
        }

        packed
    }

    pub(super) fn upload(&self, queue: &wgpu::Queue, resources: &StateVectorResources) {
        if !self.postselection.is_empty() {
            queue.write_buffer(
                &resources.postselection.params_staging_buffer,
                0,
                bytemuck::cast_slice(&self.postselection),
            );
        }
        if !self.arithmetic.is_empty() {
            queue.write_buffer(
                &resources.arithmetic.params_staging_buffer,
                0,
                bytemuck::cast_slice(&self.arithmetic),
            );
        }
        if !self.gate.is_empty() {
            queue.write_buffer(
                &resources.state.gate_params_staging_buffer,
                0,
                bytemuck::cast_slice(&self.gate),
            );
        }
        if !self.bloch.is_empty() {
            queue.write_buffer(
                &resources.bloch.params_staging_buffer,
                0,
                bytemuck::cast_slice(&self.bloch),
            );
        }
        if !self.measure_reduce.is_empty() {
            queue.write_buffer(
                &resources.measure.reduce_params_staging_buffer,
                0,
                bytemuck::cast_slice(&self.measure_reduce),
            );
        }
        if !self.measure_collapse.is_empty() {
            queue.write_buffer(
                &resources.measure.collapse_params_staging_buffer,
                0,
                bytemuck::cast_slice(&self.measure_collapse),
            );
        }
        if !self.probability.is_empty() {
            queue.write_buffer(
                &resources.probability.reduce_params_staging_buffer,
                0,
                bytemuck::cast_slice(&self.probability),
            );
        }
        if !self.amplitude.is_empty() {
            queue.write_buffer(
                &resources.amplitude.capture_params_staging_buffer,
                0,
                bytemuck::cast_slice(&self.amplitude),
            );
        }
        if !self.density.is_empty() {
            queue.write_buffer(
                &resources.density.capture_params_staging_buffer,
                0,
                bytemuck::cast_slice(&self.density),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gates::ColumnControls;
    use crate::gpu::SlotIndex;
    use crate::qubit_bit::QubitBit;

    fn packed_measurement(measurement_seed: u32) -> PackedRecomputeParams {
        let mut controls = ColumnControls::NONE.with_control(QubitBit::new(0));
        controls.add_anti_control(QubitBit::new(1));
        PackedRecomputeParams::from_ops(
            &[
                SimulationOp::MeasureReduceSample {
                    visible: true,
                    gate_id: GateId::from_u32(7),
                    qubit_bit: QubitBit::new(2),
                    output_slot: SlotIndex::new(3),
                    controls,
                },
                SimulationOp::MeasureCollapse {
                    qubit_bit: QubitBit::new(2),
                    aux_slot: SlotIndex::new(3),
                    controls,
                },
            ],
            8,
            4,
            RunnerMeasurementSeed::new(measurement_seed),
        )
    }

    #[test]
    fn detector_reset_has_distinct_sample_seed_from_original_outcome() {
        let gates =
            crate::url_circuit::parse_circuit_json(r#"{"cols":[["ZDetectControlReset"]]}"#).gates;
        let ops = crate::simulation_plan::linearize_ops(
            &gates,
            crate::qubit_count::QubitCount::try_new(1).unwrap(),
            0,
        );
        let runner_seed = RunnerMeasurementSeed::new(0x12345678);
        let packed = PackedRecomputeParams::from_ops(&ops, 2, 1, runner_seed);
        assert_eq!(
            packed
                .measure_reduce
                .iter()
                .map(|params| params.seed)
                .collect::<Vec<_>>(),
            vec![
                gates[0].id.as_u32() ^ 0x12345678,
                gates[0].id.as_u32() ^ 0x12345678 ^ 0x8000_0000,
            ]
        );
    }

    #[test]
    fn hidden_reset_seed_is_independent_of_measurement_slot() {
        let seeds = [0, 7].map(|slot| {
            let ops = [SimulationOp::MeasureReduceSample {
                visible: false,
                gate_id: GateId::from_u32(7),
                qubit_bit: QubitBit::new(0),
                output_slot: SlotIndex::new(slot),
                controls: ColumnControls::NONE,
            }];
            PackedRecomputeParams::from_ops(&ops, 2, 1, RunnerMeasurementSeed::new(0))
                .measure_reduce[0]
                .seed
        });
        assert_eq!(seeds, [7 ^ 0x8000_0000; 2]);
    }

    #[test]
    fn reduce_uniform_preserves_joint_predicate_and_seed() {
        let packed = packed_measurement(0);
        assert_eq!(
            bytemuck::cast_slice::<_, u32>(&packed.measure_reduce),
            &[2, 8, 3, 7, 3, 1, 0, 0]
        );
    }

    #[test]
    fn reduce_uniform_mixes_runner_seed_with_gate_identity() {
        let packed = packed_measurement(0x12345678);
        assert_eq!(packed.measure_reduce[0].seed, 7 ^ 0x12345678);
    }

    #[test]
    fn collapse_uniform_preserves_joint_predicate_and_aux_slot() {
        let packed = packed_measurement(0);
        assert_eq!(
            bytemuck::cast_slice::<_, u32>(&packed.measure_collapse),
            &[2, 8, 3, 0, 3, 1, 0, 0]
        );
    }
}
