use eframe::wgpu;

use super::super::super::params::RenderParams;
use super::super::common::Common;
use super::pipeline::build_render_pipeline;

pub(super) struct RenderResources {
    pub(super) pipeline: wgpu::RenderPipeline,
    pub(super) bind_groups: [wgpu::BindGroup; 3],
    pub(super) bind_group_layout: wgpu::BindGroupLayout,
    pub(super) params_buffer: wgpu::Buffer,
}

pub(super) fn build(
    device: &wgpu::Device,
    target_format: wgpu::TextureFormat,
    common: &Common,
) -> RenderResources {
    let params_buffer = create_render_params_buffer(device);
    let bind_group_layout = create_render_bind_group_layout(device);
    let bind_groups = create_render_bind_groups(device, common, &bind_group_layout, &params_buffer);
    let pipeline = build_render_pipeline(device, target_format, &bind_group_layout);

    RenderResources {
        pipeline,
        bind_groups,
        bind_group_layout,
        params_buffer,
    }
}

fn create_render_params_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("state_vector_render_params"),
        size: std::mem::size_of::<RenderParams>() as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_render_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("state_vector_render_layout"),
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

fn create_render_bind_groups(
    device: &wgpu::Device,
    common: &Common,
    layout: &wgpu::BindGroupLayout,
    params_buffer: &wgpu::Buffer,
) -> [wgpu::BindGroup; 3] {
    let create = |label: &'static str, buffer: &wgpu::Buffer| {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        })
    };
    [
        create("state_vector_render_a", &common.state_buffers[0]),
        create("state_vector_render_b", &common.state_buffers[1]),
        create("state_vector_render_preview", &common.state_preview_buffer),
    ]
}
