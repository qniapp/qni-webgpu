//! Amplitude display capture + GPU-resident gate-body / popup rendering.
//!
//! Capture, render, and hover-popup value text all read/write WebGPU buffers.
//! The only GPU→CPU path is the test-only readback API in `gpu::readback`.

use eframe::wgpu;
use wgpu::util::DeviceExt;

use super::super::params::{
    AmplitudeCaptureParams, AmplitudeInstance, AmplitudePopupValueParams, AmplitudeRenderParams,
    AMPLITUDE_VALUES_PER_SLOT, MAX_AMPLITUDE_SLOTS, MAX_OPS_PER_RECOMPUTE,
};
use super::super::popup_glyph_atlas::{
    rasterize_popup_glyph_atlas, POPUP_GLYPH_ATLAS_HEIGHT, POPUP_GLYPH_ATLAS_WIDTH,
};
use super::super::shaders::{
    AMPLITUDE_CAPTURE_SHADER, AMPLITUDE_POPUP_VALUE_SHADER, AMPLITUDE_RENDER_SHADER,
};
use super::common::Common;

struct GlyphAtlas {
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
}

pub(crate) struct AmplitudeResources {
    pub capture_pipeline: wgpu::ComputePipeline,
    pub capture_bind_groups: [wgpu::BindGroup; 2],
    pub capture_params_buffer: wgpu::Buffer,
    pub capture_params_staging_buffer: wgpu::Buffer,
    pub output_buffer: wgpu::Buffer,
    pub meta_buffer: wgpu::Buffer,

    pub render_pipeline: wgpu::RenderPipeline,
    pub render_bind_group: wgpu::BindGroup,
    pub render_drag_bind_group: wgpu::BindGroup,
    pub render_bind_group_layout: wgpu::BindGroupLayout,
    pub render_params_buffer: wgpu::Buffer,
    pub render_drag_params_buffer: wgpu::Buffer,
    pub render_instance_buffer: wgpu::Buffer,
    pub render_drag_instance_buffer: wgpu::Buffer,
    pub last_render_params: Option<AmplitudeRenderParams>,

    pub popup_value_pipeline: wgpu::RenderPipeline,
    pub popup_value_bind_group: wgpu::BindGroup,
    pub popup_value_bind_group_layout: wgpu::BindGroupLayout,
    pub popup_value_params_buffer: wgpu::Buffer,
    pub last_popup_value_params: Option<AmplitudePopupValueParams>,
    pub last_external_upload_generation: Option<u64>,
}

impl AmplitudeResources {
    pub(super) fn build(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
        common: &Common,
    ) -> Self {
        let output_buffer = create_output_buffer(device);
        let meta_buffer = create_meta_buffer(device);

        let capture_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("amplitude_capture"),
            source: wgpu::ShaderSource::Wgsl(AMPLITUDE_CAPTURE_SHADER.into()),
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
            label: Some("amplitude_render"),
            source: wgpu::ShaderSource::Wgsl(AMPLITUDE_RENDER_SHADER.into()),
        });
        let popup_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("amplitude_popup_value"),
            source: wgpu::ShaderSource::Wgsl(AMPLITUDE_POPUP_VALUE_SHADER.into()),
        });
        let render_bind_group_layout = create_render_bind_group_layout(device);
        let popup_value_bind_group_layout = create_popup_value_bind_group_layout(device);
        let render_params_buffer = create_render_params_buffer(device);
        let render_drag_params_buffer = create_render_drag_params_buffer(device);
        let render_instance_buffer = create_render_instance_buffer(device);
        let render_drag_instance_buffer = create_render_drag_instance_buffer(device);
        let popup_value_params_buffer = create_popup_value_params_buffer(device);
        let glyph_atlas = create_glyph_atlas(device, queue);
        let render_bind_group = create_render_bind_group(
            device,
            &render_bind_group_layout,
            &output_buffer,
            &meta_buffer,
            &render_params_buffer,
        );
        let render_drag_bind_group = create_render_bind_group(
            device,
            &render_bind_group_layout,
            &output_buffer,
            &meta_buffer,
            &render_drag_params_buffer,
        );
        let popup_value_bind_group = create_popup_value_bind_group(
            device,
            &popup_value_bind_group_layout,
            &output_buffer,
            &meta_buffer,
            &popup_value_params_buffer,
            &glyph_atlas,
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
            &popup_shader,
            &popup_value_bind_group_layout,
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
            render_drag_bind_group,
            render_bind_group_layout,
            render_params_buffer,
            render_drag_params_buffer,
            render_instance_buffer,
            render_drag_instance_buffer,
            last_render_params: None,
            popup_value_pipeline,
            popup_value_bind_group,
            popup_value_bind_group_layout,
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
        let render_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("amplitude_render"),
            source: wgpu::ShaderSource::Wgsl(AMPLITUDE_RENDER_SHADER.into()),
        });
        self.render_pipeline = create_render_pipeline(
            device,
            target_format,
            &render_shader,
            &self.render_bind_group_layout,
        );
        let popup_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("amplitude_popup_value"),
            source: wgpu::ShaderSource::Wgsl(AMPLITUDE_POPUP_VALUE_SHADER.into()),
        });
        self.popup_value_pipeline = create_popup_value_pipeline(
            device,
            target_format,
            &popup_shader,
            &self.popup_value_bind_group_layout,
        );
    }
}

fn create_output_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("amplitude_output"),
        size: (MAX_AMPLITUDE_SLOTS * AMPLITUDE_VALUES_PER_SLOT * std::mem::size_of::<f32>())
            as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_meta_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("amplitude_meta"),
        size: (MAX_AMPLITUDE_SLOTS * std::mem::size_of::<[f32; 4]>()) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_capture_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("amplitude_capture_layout"),
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
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
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
    })
}

fn create_capture_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::BindGroupLayout,
) -> wgpu::ComputePipeline {
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("amplitude_capture_pipeline_layout"),
        bind_group_layouts: &[layout],
        push_constant_ranges: &[],
    });
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("amplitude_capture_pipeline"),
        layout: Some(&pipeline_layout),
        module: shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    })
}

fn create_capture_params_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("amplitude_capture_params"),
        size: std::mem::size_of::<AmplitudeCaptureParams>() as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::UNIFORM
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

fn create_capture_params_staging_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("amplitude_capture_params_staging"),
        size: (MAX_OPS_PER_RECOMPUTE * std::mem::size_of::<AmplitudeCaptureParams>())
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
            label: Some("amplitude_capture_read_a"),
            layout,
            entries: &capture_entries(
                &common.state_buffers[0],
                output_buffer,
                meta_buffer,
                params_buffer,
            ),
        }),
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("amplitude_capture_read_b"),
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
        label: Some("amplitude_render_layout"),
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

fn create_popup_value_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("amplitude_popup_value_layout"),
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
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
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
    create_named_render_params_buffer(device, "amplitude_render_params")
}

fn create_render_drag_params_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    create_named_render_params_buffer(device, "amplitude_render_drag_params")
}

fn create_named_render_params_buffer(device: &wgpu::Device, label: &'static str) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: std::mem::size_of::<AmplitudeRenderParams>() as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_render_instance_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("amplitude_render_instances"),
        size: (MAX_AMPLITUDE_SLOTS * std::mem::size_of::<AmplitudeInstance>())
            as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_render_drag_instance_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("amplitude_render_drag_instance"),
        size: std::mem::size_of::<AmplitudeInstance>() as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_popup_value_params_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("amplitude_popup_value_params"),
        size: std::mem::size_of::<AmplitudePopupValueParams>() as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_glyph_atlas(device: &wgpu::Device, queue: &wgpu::Queue) -> GlyphAtlas {
    let atlas_data = rasterize_popup_glyph_atlas();
    let texture = device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            label: Some("amplitude_popup_glyph_atlas"),
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
        label: Some("amplitude_popup_glyph_atlas_sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    GlyphAtlas { view, sampler }
}

fn create_render_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    output_buffer: &wgpu::Buffer,
    meta_buffer: &wgpu::Buffer,
    params_buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("amplitude_render_bind_group"),
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

fn create_popup_value_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    output_buffer: &wgpu::Buffer,
    meta_buffer: &wgpu::Buffer,
    params_buffer: &wgpu::Buffer,
    atlas: &GlyphAtlas,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("amplitude_popup_value_bind_group"),
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
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&atlas.view),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::Sampler(&atlas.sampler),
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
        label: Some("amplitude_render_pipeline_layout"),
        bind_group_layouts: &[layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("amplitude_render_pipeline"),
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
                    array_stride: std::mem::size_of::<AmplitudeInstance>() as wgpu::BufferAddress,
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
                        // AmplitudeInstance::use_drag_background at byte 28.
                        wgpu::VertexAttribute {
                            offset: 28,
                            shader_location: 6,
                            format: wgpu::VertexFormat::Uint32,
                        },
                        // AmplitudeInstance::force_zero_amplitude at byte 32.
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

fn create_popup_value_pipeline(
    device: &wgpu::Device,
    target_format: wgpu::TextureFormat,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("amplitude_popup_value_pipeline_layout"),
        bind_group_layouts: &[layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("amplitude_popup_value_pipeline"),
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
