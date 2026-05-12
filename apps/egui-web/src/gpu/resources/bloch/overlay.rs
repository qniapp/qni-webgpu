use eframe::wgpu;

use super::super::super::params::{BlochOverlayInstance, BlochOverlayParams, MAX_BLOCH_SLOTS};
use super::pipeline::build_overlay_pipeline;

pub(super) struct OverlayResources {
    pub(super) pipeline: wgpu::RenderPipeline,
    pub(super) bind_group: wgpu::BindGroup,
    pub(super) bind_group_layout: wgpu::BindGroupLayout,
    pub(super) params_buffer: wgpu::Buffer,
    pub(super) instance_buffer: wgpu::Buffer,
}

pub(super) fn build(
    device: &wgpu::Device,
    target_format: wgpu::TextureFormat,
    output_buffer: &wgpu::Buffer,
) -> OverlayResources {
    let bind_group_layout = create_overlay_bind_group_layout(device);
    let params_buffer = create_overlay_params_buffer(device);
    let instance_buffer = create_overlay_instance_buffer(device);
    let bind_group =
        create_overlay_bind_group(device, &bind_group_layout, output_buffer, &params_buffer);
    let pipeline = build_overlay_pipeline(device, target_format, &bind_group_layout);

    OverlayResources {
        pipeline,
        bind_group,
        bind_group_layout,
        params_buffer,
        instance_buffer,
    }
}

fn create_overlay_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("bloch_overlay_layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    })
}

fn create_overlay_params_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("bloch_overlay_params"),
        size: std::mem::size_of::<BlochOverlayParams>() as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_overlay_instance_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("bloch_overlay_instances"),
        size: (MAX_BLOCH_SLOTS * std::mem::size_of::<BlochOverlayInstance>())
            as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_overlay_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    output_buffer: &wgpu::Buffer,
    params_buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("bloch_overlay_bind_group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: output_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: params_buffer.as_entire_binding(),
            },
        ],
    })
}
