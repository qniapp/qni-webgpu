//! Probability display probability reduce + gate-body render pipeline.
//!
//! The compute pass marginalizes the current state vector into a per-display
//! probability buffer. The render pass samples that storage buffer directly to
//! paint Quirk-style bars inside each placed Probability display. No production
//! GPU→CPU readback is involved.

use eframe::wgpu;

use super::super::params::{
    ProbabilityInstance, ProbabilityPopupValueParams, ProbabilityReduceParams,
    ProbabilityRenderParams, MAX_OPS_PER_RECOMPUTE, MAX_PROBABILITY_AGGREGATE_ROWS,
    MAX_PROBABILITY_OUTCOMES, MAX_PROBABILITY_SLOTS,
};
use super::super::popup_glyph_atlas::{
    rasterize_popup_glyph_atlas, POPUP_GLYPH_ATLAS_HEIGHT, POPUP_GLYPH_ATLAS_WIDTH,
};
use super::super::shaders::{
    PROBABILITY_AGGREGATE_SHADER, PROBABILITY_POPUP_VALUE_SHADER, PROBABILITY_REDUCE_SHADER,
    PROBABILITY_RENDER_SHADER,
};
use super::common::Common;

struct ProbabilityGlyphAtlas {
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
}

pub(crate) struct ProbabilityResources {
    pub reduce_pipeline: wgpu::ComputePipeline,
    pub reduce_bind_groups: [wgpu::BindGroup; 2],
    pub reduce_params_buffer: wgpu::Buffer,
    pub reduce_params_staging_buffer: wgpu::Buffer,
    pub output_buffer: wgpu::Buffer,

    pub aggregate_pipeline: wgpu::ComputePipeline,
    pub aggregate_bind_group: wgpu::BindGroup,
    _aggregate_buffer: wgpu::Buffer,

    pub render_pipeline: wgpu::RenderPipeline,
    pub render_bind_group: wgpu::BindGroup,
    pub render_bind_group_layout: wgpu::BindGroupLayout,
    pub render_params_buffer: wgpu::Buffer,
    pub render_instance_buffer: wgpu::Buffer,
    pub last_render_params: Option<ProbabilityRenderParams>,

    pub popup_value_pipeline: wgpu::RenderPipeline,
    pub popup_value_bind_group: wgpu::BindGroup,
    pub popup_value_params_buffer: wgpu::Buffer,
    pub last_popup_value_params: Option<ProbabilityPopupValueParams>,
    pub last_external_upload_generation: Option<u64>,
}

impl ProbabilityResources {
    pub(super) fn build(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
        common: &Common,
    ) -> Self {
        let output_buffer = create_output_buffer(device);
        let reduce_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("probability_reduce"),
            source: wgpu::ShaderSource::Wgsl(PROBABILITY_REDUCE_SHADER.into()),
        });
        let reduce_layout = create_reduce_bind_group_layout(device);
        let reduce_pipeline = create_reduce_pipeline(device, &reduce_shader, &reduce_layout);
        let reduce_params_buffer = create_reduce_params_buffer(device);
        let reduce_params_staging_buffer = create_reduce_params_staging_buffer(device);
        let reduce_bind_groups = create_reduce_bind_groups(
            device,
            common,
            &reduce_layout,
            &reduce_params_buffer,
            &output_buffer,
        );

        let aggregate_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("probability_aggregate"),
            source: wgpu::ShaderSource::Wgsl(PROBABILITY_AGGREGATE_SHADER.into()),
        });
        let render_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("probability_render"),
            source: wgpu::ShaderSource::Wgsl(PROBABILITY_RENDER_SHADER.into()),
        });
        let popup_value_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("probability_popup_value"),
            source: wgpu::ShaderSource::Wgsl(PROBABILITY_POPUP_VALUE_SHADER.into()),
        });
        let aggregate_layout = create_aggregate_bind_group_layout(device);
        let aggregate_pipeline =
            create_aggregate_pipeline(device, &aggregate_shader, &aggregate_layout);
        let render_bind_group_layout = create_render_bind_group_layout(device);
        let render_params_buffer = create_render_params_buffer(device);
        let render_instance_buffer = create_render_instance_buffer(device);
        let aggregate_buffer = create_aggregate_buffer(device);
        let popup_value_params_buffer = create_popup_value_params_buffer(device);
        let glyph_atlas = create_glyph_atlas(device, queue);
        let aggregate_bind_group = create_aggregate_bind_group(
            device,
            &aggregate_layout,
            &output_buffer,
            &aggregate_buffer,
            &render_instance_buffer,
        );
        let render_bind_group = create_render_bind_group(
            device,
            &render_bind_group_layout,
            &output_buffer,
            &render_params_buffer,
            &glyph_atlas,
            &aggregate_buffer,
        );
        let popup_value_bind_group = create_render_bind_group(
            device,
            &render_bind_group_layout,
            &output_buffer,
            &popup_value_params_buffer,
            &glyph_atlas,
            &aggregate_buffer,
        );
        let render_pipeline = create_render_pipeline(
            device,
            target_format,
            &render_shader,
            &render_bind_group_layout,
        );
        let popup_value_pipeline = create_popup_value_pipeline(
            device,
            target_format,
            &popup_value_shader,
            &render_bind_group_layout,
        );

        Self {
            reduce_pipeline,
            reduce_bind_groups,
            reduce_params_buffer,
            reduce_params_staging_buffer,
            output_buffer,
            aggregate_pipeline,
            aggregate_bind_group,
            _aggregate_buffer: aggregate_buffer,
            render_pipeline,
            render_bind_group,
            render_bind_group_layout,
            render_params_buffer,
            render_instance_buffer,
            last_render_params: None,
            popup_value_pipeline,
            popup_value_bind_group,
            popup_value_params_buffer,
            last_popup_value_params: None,
            last_external_upload_generation: None,
        }
    }

    pub(super) fn update_target_format(
        &mut self,
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
    ) {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("probability_render"),
            source: wgpu::ShaderSource::Wgsl(PROBABILITY_RENDER_SHADER.into()),
        });
        self.render_pipeline = create_render_pipeline(
            device,
            target_format,
            &shader,
            &self.render_bind_group_layout,
        );
        let popup_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("probability_popup_value"),
            source: wgpu::ShaderSource::Wgsl(PROBABILITY_POPUP_VALUE_SHADER.into()),
        });
        self.popup_value_pipeline = create_popup_value_pipeline(
            device,
            target_format,
            &popup_shader,
            &self.render_bind_group_layout,
        );
    }
}

fn create_output_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("probability_output"),
        size: (MAX_PROBABILITY_SLOTS * MAX_PROBABILITY_OUTCOMES * std::mem::size_of::<f32>())
            as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_reduce_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("probability_reduce_layout"),
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
        label: Some("probability_reduce_pipeline_layout"),
        bind_group_layouts: &[layout],
        push_constant_ranges: &[],
    });
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("probability_reduce_pipeline"),
        layout: Some(&pipeline_layout),
        module: shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    })
}

fn create_reduce_params_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("probability_reduce_params"),
        size: std::mem::size_of::<ProbabilityReduceParams>() as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::UNIFORM
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

fn create_reduce_params_staging_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("probability_reduce_params_staging"),
        size: (MAX_OPS_PER_RECOMPUTE * std::mem::size_of::<ProbabilityReduceParams>())
            as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

fn create_reduce_bind_groups(
    device: &wgpu::Device,
    common: &Common,
    layout: &wgpu::BindGroupLayout,
    params_buffer: &wgpu::Buffer,
    output_buffer: &wgpu::Buffer,
) -> [wgpu::BindGroup; 2] {
    [
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("probability_reduce_read_a"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: common.state_buffers[0].as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        }),
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("probability_reduce_read_b"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: common.state_buffers[1].as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        }),
    ]
}

fn create_aggregate_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("probability_aggregate_rows"),
        size: (MAX_PROBABILITY_SLOTS * MAX_PROBABILITY_AGGREGATE_ROWS * std::mem::size_of::<f32>())
            as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::STORAGE,
        mapped_at_creation: false,
    })
}

fn create_aggregate_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("probability_aggregate_layout"),
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
        ],
    })
}

fn create_aggregate_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::BindGroupLayout,
) -> wgpu::ComputePipeline {
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("probability_aggregate_pipeline_layout"),
        bind_group_layouts: &[layout],
        push_constant_ranges: &[],
    });
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("probability_aggregate_pipeline"),
        layout: Some(&pipeline_layout),
        module: shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    })
}

fn create_aggregate_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    output_buffer: &wgpu::Buffer,
    aggregate_buffer: &wgpu::Buffer,
    render_instance_buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("probability_aggregate_bind_group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: output_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: aggregate_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: render_instance_buffer.as_entire_binding(),
            },
        ],
    })
}

fn create_render_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("probability_render_layout"),
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
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    })
}

fn create_render_params_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("probability_render_params"),
        size: std::mem::size_of::<ProbabilityRenderParams>() as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_popup_value_params_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("probability_popup_value_params"),
        size: std::mem::size_of::<ProbabilityPopupValueParams>() as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_render_instance_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("probability_render_instances"),
        size: (MAX_PROBABILITY_SLOTS * std::mem::size_of::<ProbabilityInstance>())
            as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::VERTEX
            | wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_glyph_atlas(device: &wgpu::Device, queue: &wgpu::Queue) -> ProbabilityGlyphAtlas {
    let atlas_data = rasterize_popup_glyph_atlas();
    let texture = wgpu::util::DeviceExt::create_texture_with_data(
        device,
        queue,
        &wgpu::TextureDescriptor {
            label: Some("probability_percent_glyph_atlas"),
            size: wgpu::Extent3d {
                width: POPUP_GLYPH_ATLAS_WIDTH,
                height: POPUP_GLYPH_ATLAS_HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::default(),
        &atlas_data,
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("probability_percent_glyph_atlas_sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    ProbabilityGlyphAtlas { view, sampler }
}

fn create_render_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    output_buffer: &wgpu::Buffer,
    params_buffer: &wgpu::Buffer,
    atlas: &ProbabilityGlyphAtlas,
    aggregate_buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("probability_render_bind_group"),
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
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&atlas.view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(&atlas.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: aggregate_buffer.as_entire_binding(),
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
        label: Some("probability_render_pipeline_layout"),
        bind_group_layouts: &[layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("probability_render_pipeline"),
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
                    array_stride: std::mem::size_of::<ProbabilityInstance>() as wgpu::BufferAddress,
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

fn create_popup_value_pipeline(
    device: &wgpu::Device,
    target_format: wgpu::TextureFormat,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("probability_popup_value_pipeline_layout"),
        bind_group_layouts: &[layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("probability_popup_value_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[],
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
