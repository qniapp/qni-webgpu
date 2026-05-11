//! Bloch reduction (compute) + dynamic Bloch arrow overlay (render).
//!
//! * `reduce_pipeline` — compute pass that reads the active state
//!   buffer and writes `(x, y, z, len)` per qubit into
//!   `output_buffer`. Two bind groups so we can read from either
//!   ping-pong state buffer.
//! * `overlay_pipeline` — render pass that samples `output_buffer`
//!   in the fragment shader and draws the arrow + tip dot directly,
//!   no CPU readback. Per-frame instance data lives in
//!   `overlay_instance_buffer`.
//!
//! `output_buffer` is the bridge between the two: written by the
//! compute pipeline, read by the overlay's fragment stage.
//!
//! Bug fix vs the previous monolithic `update_render_pipeline`: this
//! module's `update_target_format` *also* rebuilds the overlay
//! pipeline, not just the state-vector render pipeline. The earlier
//! code silently kept the overlay pipeline pinned to the original
//! surface format.

use eframe::wgpu;

use super::super::params::{
    BlochOverlayInstance, BlochOverlayParams, BlochParams, MAX_BLOCH_SLOTS, MAX_OPS_PER_RECOMPUTE,
};
use super::super::shaders::{BLOCH_OVERLAY_SHADER, BLOCH_REDUCE_SHADER};
use super::common::Common;

pub(crate) struct BlochResources {
    // --- reduce (compute) ---
    pub reduce_pipeline: wgpu::ComputePipeline,
    pub reduce_bind_groups: [wgpu::BindGroup; 2],
    pub params_buffer: wgpu::Buffer,
    /// See `state::gate_params_staging_buffer` — same staging pattern.
    pub params_staging_buffer: wgpu::Buffer,
    pub output_buffer: wgpu::Buffer,

    // --- overlay (render) ---
    pub overlay_pipeline: wgpu::RenderPipeline,
    pub overlay_bind_group: wgpu::BindGroup,
    pub overlay_bind_group_layout: wgpu::BindGroupLayout,
    pub overlay_params_buffer: wgpu::Buffer,
    pub overlay_instance_buffer: wgpu::Buffer,
    pub last_overlay_params: Option<BlochOverlayParams>,
}

impl BlochResources {
    pub(super) fn build(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        common: &Common,
    ) -> Self {
        let reduce_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bloch_reduce"),
            source: wgpu::ShaderSource::Wgsl(BLOCH_REDUCE_SHADER.into()),
        });

        let reduce_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bloch_reduce_layout"),
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
                label: Some("bloch_reduce_pipeline_layout"),
                bind_group_layouts: &[&reduce_layout],
                push_constant_ranges: &[],
            });
        let reduce_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("bloch_reduce_pipeline"),
            layout: Some(&reduce_pipeline_layout),
            module: &reduce_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bloch_reduce_params"),
            size: std::mem::size_of::<BlochParams>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let params_staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bloch_reduce_params_staging"),
            size: (MAX_OPS_PER_RECOMPUTE * std::mem::size_of::<BlochParams>())
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bloch_output"),
            size: (MAX_BLOCH_SLOTS * 4 * std::mem::size_of::<f32>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let reduce_bind_groups = [
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("bloch_reduce_read_a"),
                layout: &reduce_layout,
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
                label: Some("bloch_reduce_read_b"),
                layout: &reduce_layout,
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
        ];

        let overlay_bind_group_layout =
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
            });
        let overlay_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bloch_overlay_params"),
            size: std::mem::size_of::<BlochOverlayParams>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let overlay_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bloch_overlay_instances"),
            size: (MAX_BLOCH_SLOTS * std::mem::size_of::<BlochOverlayInstance>())
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let overlay_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bloch_overlay_bind_group"),
            layout: &overlay_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: output_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: overlay_params_buffer.as_entire_binding(),
                },
            ],
        });

        let overlay_pipeline =
            build_overlay_pipeline(device, target_format, &overlay_bind_group_layout);

        Self {
            reduce_pipeline,
            reduce_bind_groups,
            params_buffer,
            params_staging_buffer,
            output_buffer,
            overlay_pipeline,
            overlay_bind_group,
            overlay_bind_group_layout,
            overlay_params_buffer,
            overlay_instance_buffer,
            last_overlay_params: None,
        }
    }

    pub(super) fn update_target_format(
        &mut self,
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
    ) {
        self.overlay_pipeline =
            build_overlay_pipeline(device, target_format, &self.overlay_bind_group_layout);
    }
}

fn build_overlay_pipeline(
    device: &wgpu::Device,
    target_format: wgpu::TextureFormat,
    layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("bloch_overlay"),
        source: wgpu::ShaderSource::Wgsl(BLOCH_OVERLAY_SHADER.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("bloch_overlay_pipeline_layout"),
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
        array_stride: std::mem::size_of::<BlochOverlayInstance>() as wgpu::BufferAddress,
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
                format: wgpu::VertexFormat::Float32,
                offset: 12,
                shader_location: 3,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Uint32,
                offset: 16,
                shader_location: 4,
            },
        ],
    };
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("bloch_overlay_pipeline"),
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
