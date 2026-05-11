//! Popup value render pipeline.
//!
//! Three-instance draw painting the amplitude / probability / phase
//! numeric rows of the state-cell hover popup. The fragment shader
//! reads the active `Common::state_buffers` directly and looks up
//! glyphs in the baked popup atlas. No CPU readback.
//!
//! Two bind groups so we can pick the active ping-pong index when the
//! popup paints (matches the state-render bind groups).

use eframe::wgpu;

use super::super::params::PopupValueParams;
use super::super::popup_glyph_atlas::{
    rasterize_popup_glyph_atlas, POPUP_GLYPH_ATLAS_HEIGHT, POPUP_GLYPH_ATLAS_WIDTH,
};
use super::super::shaders::POPUP_VALUE_SHADER;
use super::common::Common;

pub(crate) struct PopupValueResources {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_groups: [wgpu::BindGroup; 2],
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub params_buffer: wgpu::Buffer,
    pub last_params: Option<PopupValueParams>,
}

impl PopupValueResources {
    pub(super) fn build(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
        common: &Common,
    ) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("popup_value_layout"),
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

        let atlas_data = rasterize_popup_glyph_atlas();
        let atlas_texture = wgpu::util::DeviceExt::create_texture_with_data(
            device,
            queue,
            &wgpu::TextureDescriptor {
                label: Some("popup_glyph_atlas"),
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
        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("popup_glyph_atlas_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("popup_value_params"),
            size: std::mem::size_of::<PopupValueParams>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_groups = [
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("popup_value_bind_group_0"),
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: common.state_buffers[0].as_entire_binding(),
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
            }),
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("popup_value_bind_group_1"),
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: common.state_buffers[1].as_entire_binding(),
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
            }),
        ];

        let pipeline = build_pipeline(device, target_format, &bind_group_layout);

        Self {
            pipeline,
            bind_groups,
            bind_group_layout,
            params_buffer,
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
        label: Some("popup_value"),
        source: wgpu::ShaderSource::Wgsl(POPUP_VALUE_SHADER.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("popup_value_pipeline_layout"),
        bind_group_layouts: &[layout],
        push_constant_ranges: &[],
    });
    // Vertex / instance indices come from built-ins
    // (`@builtin(vertex_index)` / `@builtin(instance_index)`); no
    // vertex buffer is needed.
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("popup_value_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
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
