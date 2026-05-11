//! Measurement digit overlay render pipeline.
//!
//! Renders the "0" or "1" character above each measured qubit by
//! reading the `outcome` field of `measure::aux_buffer` in the
//! fragment shader and sampling the pre-rasterised Hack-font digit
//! atlas. Static meter chrome is still painted by egui.
//!
//! The instance draw geometry is the shared `Common::unit_quad_*`
//! quad; per-slot offsets/sizes live in `instance_buffer`.

use eframe::wgpu;

use super::super::digit_atlas::{rasterize_digit_atlas, DIGIT_ATLAS_HEIGHT, DIGIT_ATLAS_WIDTH};
use super::super::params::{
    MeasurementDigitInstance, MeasurementDigitParams, MAX_MEASUREMENT_SLOTS,
};
use super::super::shaders::MEASUREMENT_DIGIT_SHADER;
use super::measure::MeasureResources;

pub(crate) struct DigitResources {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group: wgpu::BindGroup,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub params_buffer: wgpu::Buffer,
    pub instance_buffer: wgpu::Buffer,
    pub last_params: Option<MeasurementDigitParams>,
}

impl DigitResources {
    pub(super) fn build(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
        measure: &MeasureResources,
    ) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("measurement_digit_layout"),
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
            ],
        });

        let atlas_data = rasterize_digit_atlas();
        let atlas_texture = wgpu::util::DeviceExt::create_texture_with_data(
            device,
            queue,
            &wgpu::TextureDescriptor {
                label: Some("measurement_digit_atlas"),
                size: wgpu::Extent3d {
                    width: DIGIT_ATLAS_WIDTH,
                    height: DIGIT_ATLAS_HEIGHT,
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
        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("measurement_digit_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("measurement_digit_params"),
            size: std::mem::size_of::<MeasurementDigitParams>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("measurement_digit_instances"),
            size: (MAX_MEASUREMENT_SLOTS * std::mem::size_of::<MeasurementDigitInstance>())
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("measurement_digit_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: measure.aux_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });

        let pipeline = build_pipeline(device, target_format, &bind_group_layout);

        Self {
            pipeline,
            bind_group,
            bind_group_layout,
            params_buffer,
            instance_buffer,
            last_params: None,
        }
    }

    pub(super) fn update_target_format(
        &mut self,
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
    ) {
        self.pipeline = build_pipeline(device, target_format, &self.bind_group_layout);
    }
}

fn build_pipeline(
    device: &wgpu::Device,
    target_format: wgpu::TextureFormat,
    layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("measurement_digit"),
        source: wgpu::ShaderSource::Wgsl(MEASUREMENT_DIGIT_SHADER.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("measurement_digit_pipeline_layout"),
        bind_group_layouts: &[layout],
        push_constant_ranges: &[],
    });
    let vertex_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x2,
            offset: 0,
            shader_location: 0,
        }],
    };
    let instance_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<MeasurementDigitInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32,
                offset: 8,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Uint32,
                offset: 12,
                shader_location: 3,
            },
        ],
    };
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("measurement_digit_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[vertex_layout, instance_layout],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}
