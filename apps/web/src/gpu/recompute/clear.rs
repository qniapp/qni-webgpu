use eframe::wgpu;

use super::super::resources::StateVectorResources;

pub(super) fn encode_ground_state_init(
    encoder: &mut wgpu::CommandEncoder,
    resources: &StateVectorResources,
    state_count: usize,
) {
    encoder.clear_buffer(&resources.postselection.output_buffer, 0, None);
    encoder.copy_buffer_to_buffer(
        &resources.common.state_seed_buffer,
        0,
        &resources.postselection.survival_buffer,
        0,
        4,
    );
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
