//! Batched state-vector recompute facade.
//!
//! This module owns the GPU-only recompute path: submodules pack the already
//! linearised simulation ops, encode compute dispatches into a single command
//! buffer, and update test-only readback slot maps. No quantum state is
//! computed or read on the CPU here.

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
    MAX_BLOCH_SLOTS, MAX_CHANCE_SLOTS, MAX_MEASUREMENT_SLOTS, MAX_OPS_PER_RECOMPUTE,
    STATE_WORKGROUP_SIZE,
};
use super::readback::{BLOCH_SLOT_MAP, CHANCE_SLOT_MAP, MEASUREMENT_SLOT_MAP};
use super::resources::StateVectorResources;

pub(super) fn recompute_state_vector(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    resources: &mut StateVectorResources,
    sim_ops: &[SimulationOp],
    state_count: usize,
) -> bool {
    resources.state_count = state_count;
    if state_count == 0 {
        resources.active_state = 0;
        return true;
    }

    // Initialize to |0…0⟩ then dispatch each op on the GPU. The init itself
    // happens in `clear.rs` inside the same recompute encoder — no CPU state
    // allocation and no GPU → CPU readback.
    resources.active_state = 0;
    let pair_count = (state_count / 2) as u32;
    let dispatch_x = pair_count.div_ceil(STATE_WORKGROUP_SIZE);

    if validate_simulation_plan_capacity(
        sim_ops,
        SimulationPlanLimits {
            max_ops_per_variant: MAX_OPS_PER_RECOMPUTE,
            max_bloch_slots: MAX_BLOCH_SLOTS,
            max_measurement_slots: MAX_MEASUREMENT_SLOTS,
            max_chance_slots: MAX_CHANCE_SLOTS,
        },
    )
    .is_err()
    {
        return false;
    }

    let packed = PackedRecomputeParams::from_ops(sim_ops, state_count, pair_count);
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

    // Production path never reads back. The slot mappings are stashed in
    // thread-locals so the test-only on-demand readback APIs can copy + map the
    // GPU buffers when JS asks for them.
    BLOCH_SLOT_MAP.with(|cell| {
        *cell.borrow_mut() = encoded.bloch_slot_to_gate_id;
    });
    MEASUREMENT_SLOT_MAP.with(|cell| {
        *cell.borrow_mut() = encoded.measurement_slot_to_gate_id;
    });
    CHANCE_SLOT_MAP.with(|cell| {
        *cell.borrow_mut() = encoded.chance_slot_to_gate_id;
    });

    true
}
