use eframe::wgpu;

use super::common::Common;
use crate::gpu::params::{ArithmeticParams, MAX_OPS_PER_RECOMPUTE};

pub(crate) struct ArithmeticResources {
    pub pipeline: wgpu::ComputePipeline,
    pub bind_groups: [wgpu::BindGroup; 2],
    pub params_buffer: wgpu::Buffer,
    pub params_staging_buffer: wgpu::Buffer,
}

impl ArithmeticResources {
    pub(super) fn build(device: &wgpu::Device, common: &Common) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("arithmetic"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/arithmetic.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("arithmetic_layout"),
            entries: &[
                entry(0, wgpu::BufferBindingType::Storage { read_only: true }),
                entry(1, wgpu::BufferBindingType::Storage { read_only: false }),
                entry(2, wgpu::BufferBindingType::Uniform),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("arithmetic_pipeline_layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("arithmetic_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        let size = std::mem::size_of::<ArithmeticParams>() as u64;
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("arithmetic_params"),
            size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let params_staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("arithmetic_params_staging"),
            size: size * MAX_OPS_PER_RECOMPUTE as u64,
            usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_groups = std::array::from_fn(|index| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("arithmetic_ping_pong"),
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: common.state_buffers[index].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: common.state_buffers[1 - index].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: params_buffer.as_entire_binding(),
                    },
                ],
            })
        });
        Self {
            pipeline,
            bind_groups,
            params_buffer,
            params_staging_buffer,
        }
    }
}

fn entry(binding: u32, ty: wgpu::BufferBindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}
