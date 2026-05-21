//! Density Matrix display capture + GPU-resident gate-body rendering.
//!
//! The capture pass writes reduced density matrices to storage buffers; the
//! render pass samples those buffers directly. Test-only readback lives in
//! `gpu::readback` and is not used by production rendering.

use eframe::wgpu;

use super::super::params::{
    DensityCaptureParams, DensityInstance, DensityRenderParams, DENSITY_VALUES_PER_SLOT,
    MAX_DENSITY_SLOTS, MAX_OPS_PER_RECOMPUTE,
};
use super::super::shaders::{DENSITY_CAPTURE_SHADER, DENSITY_RENDER_SHADER};
use super::common::Common;

pub(crate) struct DensityResources {
    pub capture_pipeline: wgpu::ComputePipeline,
    pub capture_bind_groups: [wgpu::BindGroup; 2],
    pub capture_params_buffer: wgpu::Buffer,
    pub capture_params_staging_buffer: wgpu::Buffer,
    pub output_buffer: wgpu::Buffer,
    pub meta_buffer: wgpu::Buffer,

    pub render_pipeline: wgpu::RenderPipeline,
    pub render_bind_group: wgpu::BindGroup,
    pub render_bind_group_layout: wgpu::BindGroupLayout,
    pub render_params_buffer: wgpu::Buffer,
    pub render_instance_buffer: wgpu::Buffer,
    pub last_render_params: Option<DensityRenderParams>,
    pub last_external_upload_generation: Option<u64>,
}

impl DensityResources {
    pub(super) fn build(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        common: &Common,
    ) -> Self {
        let output_buffer = create_output_buffer(device);
        let meta_buffer = create_meta_buffer(device);

        let capture_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("density_capture"),
            source: wgpu::ShaderSource::Wgsl(DENSITY_CAPTURE_SHADER.into()),
        });
        let capture_layout = create_capture_bind_group_layout(device);
        let capture_pipeline = create_capture_pipeline(device, &capture_shader, &capture_layout);
        let capture_params_buffer = create_capture_params_buffer(device);
        let capture_params_staging_buffer = create_capture_params_staging_buffer(device);
        let capture_bind_groups = create_capture_bind_groups(
            device,
            common,
            &capture_layout,
            &capture_params_buffer,
            &output_buffer,
            &meta_buffer,
        );

        let render_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("density_render"),
            source: wgpu::ShaderSource::Wgsl(DENSITY_RENDER_SHADER.into()),
        });
        let render_bind_group_layout = create_render_bind_group_layout(device);
        let render_params_buffer = create_render_params_buffer(device);
        let render_instance_buffer = create_render_instance_buffer(device);
        let render_bind_group = create_render_bind_group(
            device,
            &render_bind_group_layout,
            &output_buffer,
            &meta_buffer,
            &render_params_buffer,
        );
        let render_pipeline = create_render_pipeline(
            device,
            target_format,
            &render_shader,
            &render_bind_group_layout,
        );

        Self {
            capture_pipeline,
            capture_bind_groups,
            capture_params_buffer,
            capture_params_staging_buffer,
            output_buffer,
            meta_buffer,
            render_pipeline,
            render_bind_group,
            render_bind_group_layout,
            render_params_buffer,
            render_instance_buffer,
            last_render_params: None,
            last_external_upload_generation: None,
        }
    }

    pub(super) fn update_target_format(
        &mut self,
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
    ) {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("density_render"),
            source: wgpu::ShaderSource::Wgsl(DENSITY_RENDER_SHADER.into()),
        });
        self.render_pipeline = create_render_pipeline(
            device,
            target_format,
            &shader,
            &self.render_bind_group_layout,
        );
    }
}

fn create_output_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("density_output"),
        size: (MAX_DENSITY_SLOTS * DENSITY_VALUES_PER_SLOT * std::mem::size_of::<[f32; 2]>())
            as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_meta_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("density_meta"),
        size: (MAX_DENSITY_SLOTS * std::mem::size_of::<[f32; 4]>()) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_capture_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("density_capture_layout"),
        entries: &[
            storage_entry(0, wgpu::ShaderStages::COMPUTE, true),
            storage_entry(1, wgpu::ShaderStages::COMPUTE, false),
            storage_entry(2, wgpu::ShaderStages::COMPUTE, false),
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
    })
}

fn create_capture_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::BindGroupLayout,
) -> wgpu::ComputePipeline {
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("density_capture_pipeline_layout"),
        bind_group_layouts: &[layout],
        push_constant_ranges: &[],
    });
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("density_capture_pipeline"),
        layout: Some(&pipeline_layout),
        module: shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    })
}

fn create_capture_params_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("density_capture_params"),
        size: std::mem::size_of::<DensityCaptureParams>() as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::UNIFORM
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

fn create_capture_params_staging_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("density_capture_params_staging"),
        size: (MAX_OPS_PER_RECOMPUTE * std::mem::size_of::<DensityCaptureParams>())
            as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

fn create_capture_bind_groups(
    device: &wgpu::Device,
    common: &Common,
    layout: &wgpu::BindGroupLayout,
    params_buffer: &wgpu::Buffer,
    output_buffer: &wgpu::Buffer,
    meta_buffer: &wgpu::Buffer,
) -> [wgpu::BindGroup; 2] {
    [
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("density_capture_read_a"),
            layout,
            entries: &capture_entries(
                &common.state_buffers[0],
                output_buffer,
                meta_buffer,
                params_buffer,
            ),
        }),
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("density_capture_read_b"),
            layout,
            entries: &capture_entries(
                &common.state_buffers[1],
                output_buffer,
                meta_buffer,
                params_buffer,
            ),
        }),
    ]
}

fn capture_entries<'a>(
    state_buffer: &'a wgpu::Buffer,
    output_buffer: &'a wgpu::Buffer,
    meta_buffer: &'a wgpu::Buffer,
    params_buffer: &'a wgpu::Buffer,
) -> [wgpu::BindGroupEntry<'a>; 4] {
    [
        wgpu::BindGroupEntry {
            binding: 0,
            resource: state_buffer.as_entire_binding(),
        },
        wgpu::BindGroupEntry {
            binding: 1,
            resource: output_buffer.as_entire_binding(),
        },
        wgpu::BindGroupEntry {
            binding: 2,
            resource: meta_buffer.as_entire_binding(),
        },
        wgpu::BindGroupEntry {
            binding: 3,
            resource: params_buffer.as_entire_binding(),
        },
    ]
}

fn create_render_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("density_render_layout"),
        entries: &[
            storage_entry(0, wgpu::ShaderStages::FRAGMENT, true),
            storage_entry(1, wgpu::ShaderStages::FRAGMENT, true),
            wgpu::BindGroupLayoutEntry {
                binding: 2,
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

fn storage_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
    read_only: bool,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn create_render_params_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("density_render_params"),
        size: std::mem::size_of::<DensityRenderParams>() as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_render_instance_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("density_render_instances"),
        size: (MAX_DENSITY_SLOTS * std::mem::size_of::<DensityInstance>()) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_render_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    output_buffer: &wgpu::Buffer,
    meta_buffer: &wgpu::Buffer,
    params_buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("density_render_bind_group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: output_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: meta_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: params_buffer.as_entire_binding(),
            },
        ],
    })
}

fn create_render_pipeline(
    device: &wgpu::Device,
    target_format: wgpu::TextureFormat,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("density_render_pipeline_layout"),
        bind_group_layouts: &[layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("density_render_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        offset: 0,
                        shader_location: 0,
                        format: wgpu::VertexFormat::Float32x2,
                    }],
                },
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<DensityInstance>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Uint32,
                        },
                        wgpu::VertexAttribute {
                            offset: 20,
                            shader_location: 4,
                            format: wgpu::VertexFormat::Uint32,
                        },
                        wgpu::VertexAttribute {
                            offset: 24,
                            shader_location: 5,
                            format: wgpu::VertexFormat::Sint32,
                        },
                        wgpu::VertexAttribute {
                            offset: 28,
                            shader_location: 6,
                            format: wgpu::VertexFormat::Uint32,
                        },
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 7,
                            format: wgpu::VertexFormat::Uint32,
                        },
                    ],
                },
            ],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}
