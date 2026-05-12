use eframe::wgpu;

use super::super::super::params::MAX_MEASUREMENT_SLOTS;

pub(super) fn create_aux_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("measurement_aux"),
        size: (MAX_MEASUREMENT_SLOTS * 4 * std::mem::size_of::<f32>()) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}
