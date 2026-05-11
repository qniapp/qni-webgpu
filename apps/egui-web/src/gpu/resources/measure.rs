//! Measurement reduce (sample) + collapse compute pipelines.
//!
//! * `reduce_pipeline` reads the state vector and writes
//!   `(pZero, r, outcome, sqrt_p_kept)` into `aux_buffer`. Two bind
//!   groups so we can read from either ping-pong state buffer.
//! * `collapse_pipeline` reads `aux_buffer` plus the state-in buffer
//!   and writes the renormalised post-measurement state to the
//!   state-out buffer. Layout has *four* bindings (state_in,
//!   state_out, aux, params).
//!
//! No render pipelines here — the on-screen digit lives in `digit.rs`
//! and only reads `aux_buffer`.

use eframe::wgpu;

use super::super::params::{
    MeasureCollapseParams, MeasureReduceParams, MAX_MEASUREMENT_SLOTS, MAX_OPS_PER_RECOMPUTE,
};
use super::super::shaders::{MEASURE_COLLAPSE_SHADER, MEASURE_REDUCE_SHADER};
use super::common::Common;

pub(crate) struct MeasureResources {
    pub reduce_pipeline: wgpu::ComputePipeline,
    pub reduce_bind_groups: [wgpu::BindGroup; 2],
    pub reduce_params_buffer: wgpu::Buffer,
    pub reduce_params_staging_buffer: wgpu::Buffer,

    pub collapse_pipeline: wgpu::ComputePipeline,
    pub collapse_bind_groups: [wgpu::BindGroup; 2],
    pub collapse_params_buffer: wgpu::Buffer,
    pub collapse_params_staging_buffer: wgpu::Buffer,

    pub aux_buffer: wgpu::Buffer,
}

impl MeasureResources {
    pub(super) fn build(device: &wgpu::Device, common: &Common) -> Self {
        let reduce_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("measure_reduce"),
            source: wgpu::ShaderSource::Wgsl(MEASURE_REDUCE_SHADER.into()),
        });
        let collapse_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("measure_collapse"),
            source: wgpu::ShaderSource::Wgsl(MEASURE_COLLAPSE_SHADER.into()),
        });

        let reduce_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        });
        let reduce_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("measure_reduce_pipeline_layout"),
                bind_group_layouts: &[&reduce_layout],
                push_constant_ranges: &[],
            });
        let reduce_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("measure_reduce_pipeline"),
            layout: Some(&reduce_pipeline_layout),
            module: &reduce_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // 4-binding layout: state_in (read), state_out (read_write), aux (read), params (uniform).
        let collapse_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("measure_collapse_layout"),
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
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let collapse_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("measure_collapse_pipeline_layout"),
                bind_group_layouts: &[&collapse_layout],
                push_constant_ranges: &[],
            });
        let collapse_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("measure_collapse_pipeline"),
            layout: Some(&collapse_pipeline_layout),
            module: &collapse_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let reduce_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("measure_reduce_params"),
            size: std::mem::size_of::<MeasureReduceParams>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let reduce_params_staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("measure_reduce_params_staging"),
            size: (MAX_OPS_PER_RECOMPUTE * std::mem::size_of::<MeasureReduceParams>())
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let collapse_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("measure_collapse_params"),
            size: std::mem::size_of::<MeasureCollapseParams>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let collapse_params_staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("measure_collapse_params_staging"),
            size: (MAX_OPS_PER_RECOMPUTE * std::mem::size_of::<MeasureCollapseParams>())
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let aux_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("measurement_aux"),
            size: (MAX_MEASUREMENT_SLOTS * 4 * std::mem::size_of::<f32>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let reduce_bind_groups = [
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("measure_reduce_read_a"),
                layout: &reduce_layout,
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
                        resource: reduce_params_buffer.as_entire_binding(),
                    },
                ],
            }),
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("measure_reduce_read_b"),
                layout: &reduce_layout,
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
                        resource: reduce_params_buffer.as_entire_binding(),
                    },
                ],
            }),
        ];

        let collapse_bind_groups = [
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("measure_collapse_a_to_b"),
                layout: &collapse_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: common.state_buffers[0].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: common.state_buffers[1].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: aux_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: collapse_params_buffer.as_entire_binding(),
                    },
                ],
            }),
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("measure_collapse_b_to_a"),
                layout: &collapse_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: common.state_buffers[1].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: common.state_buffers[0].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: aux_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: collapse_params_buffer.as_entire_binding(),
                    },
                ],
            }),
        ];

        Self {
            reduce_pipeline,
            reduce_bind_groups,
            reduce_params_buffer,
            reduce_params_staging_buffer,
            collapse_pipeline,
            collapse_bind_groups,
            collapse_params_buffer,
            collapse_params_staging_buffer,
            aux_buffer,
        }
    }
}
