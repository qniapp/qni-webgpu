use eframe::wgpu;

use super::super::resources::StateVectorResources;

/// Issue C: GPU-only |0…0⟩ initialization. `clear_buffer` zeros the active
/// state range, then `copy_buffer_to_buffer` writes the ground-state amplitude
/// into slot 0. Both live in the same encoder as the gate dispatches, so the
/// auto-inserted memory barriers make the first ApplyGate read this fresh
/// |0…0⟩ vector.
pub(super) fn encode_ground_state_init(
    encoder: &mut wgpu::CommandEncoder,
    resources: &StateVectorResources,
    state_count: usize,
) {
    let state_active_bytes = (state_count * std::mem::size_of::<[f32; 2]>()) as wgpu::BufferAddress;
    encoder.clear_buffer(
        &resources.common.state_buffers[0],
        0,
        Some(state_active_bytes),
    );
    encoder.copy_buffer_to_buffer(
        &resources.common.state_seed_buffer,
        0,
        &resources.common.state_buffers[0],
        0,
        std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
    );
}
