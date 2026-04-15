use eframe::egui;
use eframe::wgpu;
use wgpu::util::DeviceExt as _;

use crate::{Colors, GateParams, MAX_STATE_COUNT};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct StateInstance {
    pub(crate) center: [f32; 2],
    pub(crate) radius: f32,
    pub(crate) inner_radius: f32,
    pub(crate) stroke: f32,
    pub(crate) state_index: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct RenderParams {
    pub(crate) screen_size: [f32; 2],
    pub(crate) _pad0: [f32; 2],
    pub(crate) surface: [f32; 4],
    pub(crate) fill: [f32; 4],
    pub(crate) outline: [f32; 4],
    pub(crate) outline_zero: [f32; 4],
    pub(crate) needle: [f32; 4],
}

#[derive(Clone, Copy)]
pub(crate) struct RenderColors {
    pub(crate) surface: [f32; 4],
    pub(crate) fill: [f32; 4],
    pub(crate) outline: [f32; 4],
    pub(crate) outline_zero: [f32; 4],
    pub(crate) needle: [f32; 4],
}

impl RenderColors {
    pub(crate) fn new(colors: &Colors) -> Self {
        Self {
            surface: egui::Rgba::from(colors.surface).to_array(),
            fill: egui::Rgba::from(colors.state_fill).to_array(),
            outline: egui::Rgba::from(colors.state_outline).to_array(),
            outline_zero: egui::Rgba::from(colors.state_outline_zero).to_array(),
            needle: egui::Rgba::from(colors.state_needle).to_array(),
        }
    }
}

pub(crate) const STATE_WORKGROUP_SIZE: u32 = 64;

const STATE_COMPUTE_SHADER: &str = r#"
struct GateParams {
  m00: vec2<f32>,
  m01: vec2<f32>,
  m10: vec2<f32>,
  m11: vec2<f32>,
  bit: u32,
  state_count: u32,
  control_mask: u32,
  control_value: u32,
};

@group(0) @binding(0) var<storage, read> state_in: array<vec2<f32>>;
@group(0) @binding(1) var<storage, read_write> state_out: array<vec2<f32>>;
@group(0) @binding(2) var<uniform> params: GateParams;

fn cmul(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
  return vec2<f32>(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let pair = gid.x;
  let total_pairs = params.state_count / 2u;
  if (pair >= total_pairs) {
    return;
  }
  let bit = params.bit;
  let mask = (1u << bit) - 1u;
  let low = pair & mask;
  let high = pair >> bit;
  let i0 = (high << (bit + 1u)) | low;
  let i1 = i0 | (1u << bit);
  let a0 = state_in[i0];
  let a1 = state_in[i1];
  if (params.control_mask != 0u) {
    if ((i0 & params.control_mask) != params.control_value) {
      state_out[i0] = a0;
      state_out[i1] = a1;
      return;
    }
  }
  state_out[i0] = cmul(params.m00, a0) + cmul(params.m01, a1);
  state_out[i1] = cmul(params.m10, a0) + cmul(params.m11, a1);
}
"#;

const STATE_RENDER_SHADER: &str = r#"
struct RenderParams {
  screen_size: vec2<f32>,
  _pad0: vec2<f32>,
  surface: vec4<f32>,
  fill: vec4<f32>,
  outline: vec4<f32>,
  outline_zero: vec4<f32>,
  needle: vec4<f32>,
};

@group(0) @binding(0) var<storage, read> state: array<vec2<f32>>;
@group(0) @binding(1) var<uniform> params: RenderParams;

struct VsIn {
  @location(0) position: vec2<f32>,
  @location(1) center: vec2<f32>,
  @location(2) radius: f32,
  @location(3) inner_radius: f32,
  @location(4) stroke: f32,
  @location(5) state_index: u32,
};

struct VsOut {
  @builtin(position) clip: vec4<f32>,
  @location(0) local: vec2<f32>,
  @location(1) radius: f32,
  @location(2) inner_radius: f32,
  @location(3) stroke: f32,
  @location(4) @interpolate(flat) state_index: u32,
};

@vertex
fn vs_main(input: VsIn) -> VsOut {
  let outer = input.radius + input.stroke * 0.5;
  // Pad the quad so the AA edge has coverage; avoids flat/clipped circle edges.
  let local = input.position * (outer + 1.0);
  let world = input.center + local;
  let ndc = vec2<f32>(
    (world.x / params.screen_size.x) * 2.0 - 1.0,
    1.0 - (world.y / params.screen_size.y) * 2.0
  );
  var out: VsOut;
  out.clip = vec4<f32>(ndc, 0.0, 1.0);
  out.local = local;
  out.radius = input.radius;
  out.inner_radius = input.inner_radius;
  out.stroke = input.stroke;
  out.state_index = input.state_index;
  return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
  let dist = length(input.local);
  let half_stroke = input.stroke * 0.5;
  let outer = input.radius + half_stroke;
  let edge = fwidth(dist);
  if (dist > outer + edge) {
    discard;
  }
  let amp = state[input.state_index];
  let prob = clamp(amp.x * amp.x + amp.y * amp.y, 0.0, 1.0);
  let fill_radius = input.inner_radius * sqrt(prob);
  var color = params.surface;
  let fill_alpha = 1.0 - smoothstep(fill_radius - edge, fill_radius + edge, dist);
  color = mix(color, params.fill, fill_alpha);
  if (prob > 0.0) {
    let phase = atan2(amp.y, amp.x);
    let dir = vec2<f32>(-sin(phase), -cos(phase));
    let t = clamp(dot(input.local, dir), 0.0, input.inner_radius);
    let closest = dir * t;
    let d = length(input.local - closest);
    let needle_alpha = 1.0 - smoothstep(input.stroke * 0.5 - edge, input.stroke * 0.5 + edge, d);
    color = mix(color, params.needle, needle_alpha);
  }
  let outline_color = select(params.outline_zero, params.outline, prob > 0.0);
  let outline_inner = 1.0 - smoothstep(input.radius - half_stroke - edge, input.radius - half_stroke + edge, dist);
  let outline_outer = 1.0 - smoothstep(input.radius + half_stroke - edge, input.radius + half_stroke + edge, dist);
  let outline_alpha = max(0.0, outline_outer - outline_inner);
  color = mix(color, outline_color, outline_alpha);
  let outer_alpha = 1.0 - smoothstep(outer - edge, outer + edge, dist);
  return color * outer_alpha;
}
"#;

pub(crate) struct StateVectorResources {
    pub(crate) compute_pipeline: wgpu::ComputePipeline,
    pub(crate) render_pipeline: wgpu::RenderPipeline,
    pub(crate) compute_bind_groups: [wgpu::BindGroup; 2],
    pub(crate) render_bind_groups: [wgpu::BindGroup; 2],
    pub(crate) render_bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) gate_params_buffer: wgpu::Buffer,
    pub(crate) render_params_buffer: wgpu::Buffer,
    pub(crate) state_buffers: [wgpu::Buffer; 2],
    pub(crate) instance_buffer: wgpu::Buffer,
    pub(crate) vertex_buffer: wgpu::Buffer,
    pub(crate) index_buffer: wgpu::Buffer,
    pub(crate) index_count: u32,
    pub(crate) target_format: wgpu::TextureFormat,
    pub(crate) state_count: usize,
    pub(crate) active_state: usize,
}

impl StateVectorResources {
    pub(crate) fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("state_vector_compute"),
            source: wgpu::ShaderSource::Wgsl(STATE_COMPUTE_SHADER.into()),
        });
        let render_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("state_vector_render"),
            source: wgpu::ShaderSource::Wgsl(STATE_RENDER_SHADER.into()),
        });

        let compute_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("state_vector_compute_layout"),
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

        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("state_vector_compute_pipeline_layout"),
                bind_group_layouts: &[&compute_bind_group_layout],
                push_constant_ranges: &[],
            });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("state_vector_compute_pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let render_bind_group_layout =
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
            });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("state_vector_render_pipeline_layout"),
                bind_group_layouts: &[&render_bind_group_layout],
                push_constant_ranges: &[],
            });

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("state_vector_quad_vertices"),
            contents: bytemuck::cast_slice(&[
                [-1.0f32, -1.0],
                [1.0, -1.0],
                [1.0, 1.0],
                [-1.0, 1.0],
            ]),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_data: [u16; 6] = [0, 1, 2, 0, 2, 3];
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("state_vector_quad_indices"),
            contents: bytemuck::cast_slice(&index_data),
            usage: wgpu::BufferUsages::INDEX,
        });

        let state_buffer_size =
            (MAX_STATE_COUNT * std::mem::size_of::<[f32; 2]>()) as wgpu::BufferAddress;
        let state_buffers = [
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("state_vector_buffer_a"),
                size: state_buffer_size,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("state_vector_buffer_b"),
                size: state_buffer_size,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
        ];

        let gate_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("state_vector_gate_params"),
            size: std::mem::size_of::<GateParams>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let render_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("state_vector_render_params"),
            size: std::mem::size_of::<RenderParams>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let compute_bind_groups = [
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("state_vector_compute_a_to_b"),
                layout: &compute_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: state_buffers[0].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: state_buffers[1].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: gate_params_buffer.as_entire_binding(),
                    },
                ],
            }),
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("state_vector_compute_b_to_a"),
                layout: &compute_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: state_buffers[1].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: state_buffers[0].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: gate_params_buffer.as_entire_binding(),
                    },
                ],
            }),
        ];

        let render_bind_groups = [
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("state_vector_render_a"),
                layout: &render_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: state_buffers[0].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: render_params_buffer.as_entire_binding(),
                    },
                ],
            }),
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("state_vector_render_b"),
                layout: &render_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: state_buffers[1].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: render_params_buffer.as_entire_binding(),
                    },
                ],
            }),
        ];

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
            array_stride: std::mem::size_of::<StateInstance>() as wgpu::BufferAddress,
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
                    format: wgpu::VertexFormat::Float32,
                    offset: 16,
                    shader_location: 4,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Uint32,
                    offset: 20,
                    shader_location: 5,
                },
            ],
        };

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("state_vector_render_pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &render_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[vertex_layout, instance_layout],
            },
            fragment: Some(wgpu::FragmentState {
                module: &render_shader,
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
        });

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("state_vector_instances"),
            size: (MAX_STATE_COUNT * std::mem::size_of::<StateInstance>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            compute_pipeline,
            render_pipeline,
            compute_bind_groups,
            render_bind_groups,
            render_bind_group_layout,
            gate_params_buffer,
            render_params_buffer,
            state_buffers,
            instance_buffer,
            vertex_buffer,
            index_buffer,
            index_count: index_data.len() as u32,
            target_format,
            state_count: 0,
            active_state: 0,
        }
    }

    pub(crate) fn update_render_pipeline(
        &mut self,
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
    ) {
        if self.target_format == target_format {
            return;
        }
        let render_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("state_vector_render"),
            source: wgpu::ShaderSource::Wgsl(STATE_RENDER_SHADER.into()),
        });
        let render_bind_group_layout = &self.render_bind_group_layout;
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("state_vector_render_pipeline_layout"),
            bind_group_layouts: &[render_bind_group_layout],
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
            array_stride: std::mem::size_of::<StateInstance>() as wgpu::BufferAddress,
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
                    format: wgpu::VertexFormat::Float32,
                    offset: 16,
                    shader_location: 4,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Uint32,
                    offset: 20,
                    shader_location: 5,
                },
            ],
        };
        self.render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("state_vector_render_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &render_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[vertex_layout, instance_layout],
            },
            fragment: Some(wgpu::FragmentState {
                module: &render_shader,
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
        });
        self.target_format = target_format;
    }
}
