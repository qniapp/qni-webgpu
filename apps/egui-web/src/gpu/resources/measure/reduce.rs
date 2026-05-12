use eframe::wgpu;

use super::super::super::params::{MeasureReduceParams, MAX_OPS_PER_RECOMPUTE};
use super::super::super::shaders::MEASURE_REDUCE_SHADER;
use super::super::common::Common;

pub(super) struct ReduceResources {
    pub(super) pipeline: wgpu::ComputePipeline,
    pub(super) bind_groups: [wgpu::BindGroup; 2],
    pub(super) params_buffer: wgpu::Buffer,
    pub(super) params_staging_buffer: wgpu::Buffer,
}

pub(super) fn build(
    device: &wgpu::Device,
    common: &Common,
    aux_buffer: &wgpu::Buffer,
) -> ReduceResources {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("measure_reduce"),
        source: wgpu::ShaderSource::Wgsl(MEASURE_REDUCE_SHADER.into()),
    });
    let layout = create_reduce_bind_group_layout(device);
    let pipeline = create_reduce_pipeline(device, &shader, &layout);
    let params_buffer = create_reduce_params_buffer(device);
    let params_staging_buffer = create_reduce_params_staging_buffer(device);
    let bind_groups =
        create_reduce_bind_groups(device, common, &layout, aux_buffer, &params_buffer);

    ReduceResources {
        pipeline,
        bind_groups,
        params_buffer,
        params_staging_buffer,
    }
}

fn create_reduce_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("measure_reduce_layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
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

fn create_reduce_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::BindGroupLayout,
) -> wgpu::ComputePipeline {
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("measure_reduce_pipeline_layout"),
        bind_group_layouts: &[layout],
        push_constant_ranges: &[],
    });

    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("measure_reduce_pipeline"),
        layout: Some(&pipeline_layout),
        module: shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    })
}

fn create_reduce_params_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("measure_reduce_params"),
        size: std::mem::size_of::<MeasureReduceParams>() as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::UNIFORM
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

fn create_reduce_params_staging_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("measure_reduce_params_staging"),
        size: (MAX_OPS_PER_RECOMPUTE * std::mem::size_of::<MeasureReduceParams>())
            as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

fn create_reduce_bind_groups(
    device: &wgpu::Device,
    common: &Common,
    layout: &wgpu::BindGroupLayout,
    aux_buffer: &wgpu::Buffer,
    params_buffer: &wgpu::Buffer,
) -> [wgpu::BindGroup; 2] {
    [
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("measure_reduce_read_a"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: common.state_buffers[0].as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: aux_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        }),
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("measure_reduce_read_b"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: common.state_buffers[1].as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: aux_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        }),
    ]
}
