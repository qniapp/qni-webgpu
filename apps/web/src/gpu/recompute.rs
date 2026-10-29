mod clear;
mod dispatch;
mod pack;

use eframe::wgpu;

use crate::simulation_plan::{
    validate_simulation_plan_capacity, SimulationOp, SimulationPlanLimits,
};

use self::dispatch::encode_batched_recompute;
use self::pack::PackedRecomputeParams;
use super::params::{
    MAX_AMPLITUDE_SLOTS, MAX_BLOCH_SLOTS, MAX_DENSITY_SLOTS, MAX_MEASUREMENT_SLOTS,
    MAX_OPS_PER_RECOMPUTE, MAX_PROBABILITY_SLOTS, MAX_STEP_SNAPSHOT_SLOTS, STATE_WORKGROUP_SIZE,
};
use super::readback::{
    AMPLITUDE_SLOT_MAP, BLOCH_SLOT_MAP, DENSITY_SLOT_MAP, MEASUREMENT_SLOT_MAP,
    PROBABILITY_SLOT_MAP,
};
use super::resources::StateVectorResources;

pub(super) fn recompute_state_vector(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    resources: &mut StateVectorResources,
    sim_ops: &[SimulationOp],
    state_count: usize,
    measurement_seed: u32,
) -> bool {
    resources.state_count = state_count;
    if state_count == 0 {
        resources.active_state = 0;
        return true;
    }

    resources.active_state = 0;
    let pair_count = (state_count / 2) as u32;
    let dispatch_x = pair_count.div_ceil(STATE_WORKGROUP_SIZE);

    if validate_simulation_plan_capacity(
        sim_ops,
        SimulationPlanLimits {
            max_ops_per_variant: MAX_OPS_PER_RECOMPUTE,
            max_step_snapshot_slots: MAX_STEP_SNAPSHOT_SLOTS,
            max_bloch_slots: MAX_BLOCH_SLOTS,
            max_measurement_slots: MAX_MEASUREMENT_SLOTS,
            max_postselection_slots: super::params::MAX_POSTSELECTION_SLOTS,
            max_probability_slots: MAX_PROBABILITY_SLOTS,
            max_amplitude_slots: MAX_AMPLITUDE_SLOTS,
            max_density_slots: MAX_DENSITY_SLOTS,
        },
    )
    .is_err()
    {
        return false;
    }

    let packed =
        PackedRecomputeParams::from_ops(sim_ops, state_count, pair_count, measurement_seed);
    packed.upload(queue, resources);

    let encoded = encode_batched_recompute(
        device,
        resources,
        sim_ops,
        state_count,
        pair_count,
        dispatch_x,
    );
    queue.submit(Some(encoded.command_buffer));
    resources.active_state = encoded.active_state;

    super::readback::POSTSELECTION_SLOT_MAP.with(|cell| {
        *cell.borrow_mut() = encoded.postselection_slot_to_gate_id;
    });
    BLOCH_SLOT_MAP.with(|cell| {
        *cell.borrow_mut() = encoded.bloch_slot_to_gate_id;
    });
    MEASUREMENT_SLOT_MAP.with(|cell| {
        *cell.borrow_mut() = encoded.measurement_slot_to_gate_id;
    });
    PROBABILITY_SLOT_MAP.with(|cell| {
        *cell.borrow_mut() = encoded.probability_slot_to_gate_id;
    });
    AMPLITUDE_SLOT_MAP.with(|cell| {
        *cell.borrow_mut() = encoded.amplitude_slot_to_gate_id;
    });
    DENSITY_SLOT_MAP.with(|cell| {
        *cell.borrow_mut() = encoded.density_slot_to_gate_id;
    });

    true
}
