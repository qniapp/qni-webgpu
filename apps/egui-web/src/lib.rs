mod layout;

use eframe::egui;
use eframe::{egui_wgpu, wgpu};
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use wgpu::util::DeviceExt as _;

use crate::layout::{
    layout_metrics, nearest_available_slot, nearest_line, nearest_slot_center,
    nearest_slot_index,
};

const REM: f32 = 32.0;
const STATE_CIRCLE_SIZE: f32 = 1.25 * REM;
const STATE_CIRCLE_GAP: f32 = 0.5 * REM;
const STATE_CIRCLE_BOTTOM_MARGIN: f32 = 2.0 * REM;
const STATE_CIRCLE_STROKE: f32 = 2.0;

const MIN_QUBITS: usize = 2;
const MAX_QUBITS: usize = 16;
const MAX_STATE_COUNT: usize = 1 << MAX_QUBITS;

const LINE_Y: f32 = 6.5 * REM;
const LINE_GAP: f32 = 1.5 * REM;
const CIRCUIT_PADDING: f32 = 2.0 * REM; // Same as PALETTE_ROW_Y for visual consistency
const QUBIT_LABEL_WIDTH: f32 = 3.0 * 14.0; // "qN:" at font size 14
const QUBIT_LABEL_GAP: f32 = 0.5 * REM; // Gap between label and line (0.5rem)
const LINE_LEFT_OFFSET: f32 = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP;
const LINE_RIGHT_OFFSET: f32 = CIRCUIT_PADDING;

const GATE_SIZE: f32 = 1.0 * REM;
const SLOT_SPACING: f32 = GATE_SIZE * 1.5;
const SNAP_DISTANCE: f32 = 0.5625 * REM;
const DRAG_REPAINT_BASE_SECS: f64 = 0.01;
const DRAG_REPAINT_MIN_SECS: f64 = 0.004;
const DRAG_REPAINT_MAX_SECS: f64 = 1.0 / 30.0;
const DRAG_REPAINT_PUMP_FACTOR: f64 = 0.1;
const PALETTE_SIZE: f32 = GATE_SIZE;
const PALETTE_GAP: f32 = 0.5 * REM;
const PALETTE_ROW_Y: f32 = 2.0 * REM;

thread_local! {
    static GPU_READBACK: RefCell<Option<GpuReadbackState>> = const { RefCell::new(None) };
}

#[cfg(target_arch = "wasm32")]
fn now_seconds() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map(|performance| performance.now() / 1000.0)
        .unwrap_or(0.0)
}

#[cfg(not(target_arch = "wasm32"))]
fn now_seconds() -> f64 {
    use std::sync::OnceLock;
    use std::time::Instant;

    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GateKind {
    H,
    Control,
    X,
    Y,
    Z,
    SqrtX,
    S,
    SDagger,
    T,
    TDagger,
    Phase,
    Rx,
    Ry,
    Rz,
    Swap,
}

impl GateKind {
    fn label(self) -> &'static str {
        match self {
            GateKind::H => "H",
            GateKind::Control => "C",
            GateKind::X => "X",
            GateKind::Y => "Y",
            GateKind::Z => "Z",
            GateKind::SqrtX => "√X",
            GateKind::S => "S",
            GateKind::SDagger => "S†",
            GateKind::T => "T",
            GateKind::TDagger => "T†",
            GateKind::Phase => "P",
            GateKind::Rx => "Rx",
            GateKind::Ry => "Ry",
            GateKind::Rz => "Rz",
            GateKind::Swap => "SWAP",
        }
    }
}

const PALETTE_GATES: [GateKind; 15] = [
    GateKind::H,
    GateKind::Control,
    GateKind::X,
    GateKind::Y,
    GateKind::Z,
    GateKind::SqrtX,
    GateKind::S,
    GateKind::SDagger,
    GateKind::T,
    GateKind::TDagger,
    GateKind::Phase,
    GateKind::Rx,
    GateKind::Ry,
    GateKind::Rz,
    GateKind::Swap,
];

#[derive(Clone, Copy, Debug)]
struct GateMatrix {
    m00: [f32; 2],
    m01: [f32; 2],
    m10: [f32; 2],
    m11: [f32; 2],
}

#[derive(Clone, Debug)]
struct PlacedGate {
    id: u32,
    kind: GateKind,
    pos: egui::Pos2,
    wire: usize,
}

#[derive(Clone, Copy, Debug)]
struct DragState {
    id: u32,
    offset: egui::Vec2,
}

fn should_use_fast_gate_body(fast_drag: bool, dragging: Option<DragState>, gate_id: u32) -> bool {
    fast_drag && dragging.map(|drag| drag.id) != Some(gate_id)
}

fn gate_matrix(kind: GateKind) -> GateMatrix {
    let inv_sqrt2 = std::f32::consts::FRAC_1_SQRT_2;
    let default_angle = std::f32::consts::FRAC_PI_2;
    let half_angle = default_angle * 0.5;
    let cos_half = half_angle.cos();
    let sin_half = half_angle.sin();
    let exp_i = |angle: f32| [angle.cos(), angle.sin()];
    match kind {
        GateKind::H => GateMatrix {
            m00: [inv_sqrt2, 0.0],
            m01: [inv_sqrt2, 0.0],
            m10: [inv_sqrt2, 0.0],
            m11: [-inv_sqrt2, 0.0],
        },
        GateKind::Control => GateMatrix {
            m00: [1.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [1.0, 0.0],
        },
        GateKind::X => GateMatrix {
            m00: [0.0, 0.0],
            m01: [1.0, 0.0],
            m10: [1.0, 0.0],
            m11: [0.0, 0.0],
        },
        GateKind::Y => GateMatrix {
            m00: [0.0, 0.0],
            m01: [0.0, -1.0],
            m10: [0.0, 1.0],
            m11: [0.0, 0.0],
        },
        GateKind::Z => GateMatrix {
            m00: [1.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [-1.0, 0.0],
        },
        GateKind::SqrtX => GateMatrix {
            m00: [0.5, 0.5],
            m01: [0.5, -0.5],
            m10: [0.5, -0.5],
            m11: [0.5, 0.5],
        },
        GateKind::S => GateMatrix {
            m00: [1.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [0.0, 1.0],
        },
        GateKind::SDagger => GateMatrix {
            m00: [1.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [0.0, -1.0],
        },
        GateKind::T => GateMatrix {
            m00: [1.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [inv_sqrt2, inv_sqrt2],
        },
        GateKind::TDagger => GateMatrix {
            m00: [1.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [inv_sqrt2, -inv_sqrt2],
        },
        GateKind::Phase => GateMatrix {
            m00: [1.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: exp_i(default_angle),
        },
        GateKind::Rx => GateMatrix {
            m00: [cos_half, 0.0],
            m01: [0.0, -sin_half],
            m10: [0.0, -sin_half],
            m11: [cos_half, 0.0],
        },
        GateKind::Ry => GateMatrix {
            m00: [cos_half, 0.0],
            m01: [-sin_half, 0.0],
            m10: [sin_half, 0.0],
            m11: [cos_half, 0.0],
        },
        GateKind::Rz => GateMatrix {
            m00: [cos_half, -sin_half],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [cos_half, sin_half],
        },
        GateKind::Swap => GateMatrix {
            m00: [1.0, 0.0],
            m01: [0.0, 0.0],
            m10: [0.0, 0.0],
            m11: [1.0, 0.0],
        },
    }
}

fn display_index_to_state_index(mut display_index: usize, qubits: usize) -> usize {
    let mut value = 0usize;
    for _ in 0..qubits {
        value = (value << 1) | (display_index & 1);
        display_index >>= 1;
    }
    value
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GateParams {
    m00: [f32; 2],
    m01: [f32; 2],
    m10: [f32; 2],
    m11: [f32; 2],
    bit: u32,
    state_count: u32,
    control_mask: u32,
    control_value: u32,
}

fn gate_params(kind: GateKind, bit: u32, state_count: u32) -> GateParams {
    let matrix = gate_matrix(kind);
    GateParams {
        m00: matrix.m00,
        m01: matrix.m01,
        m10: matrix.m10,
        m11: matrix.m11,
        bit,
        state_count,
        control_mask: 0,
        control_value: 0,
    }
}

fn gate_params_controlled(
    kind: GateKind,
    bit: u32,
    control_mask: u32,
    control_value: u32,
    state_count: u32,
) -> GateParams {
    let matrix = gate_matrix(kind);
    GateParams {
        m00: matrix.m00,
        m01: matrix.m01,
        m10: matrix.m10,
        m11: matrix.m11,
        bit,
        state_count,
        control_mask,
        control_value,
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct StateInstance {
    center: [f32; 2],
    radius: f32,
    inner_radius: f32,
    stroke: f32,
    state_index: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct RenderParams {
    screen_size: [f32; 2],
    _pad0: [f32; 2],
    surface: [f32; 4],
    fill: [f32; 4],
    outline: [f32; 4],
    outline_zero: [f32; 4],
    needle: [f32; 4],
}

#[derive(Clone, Copy)]
struct RenderColors {
    surface: [f32; 4],
    fill: [f32; 4],
    outline: [f32; 4],
    outline_zero: [f32; 4],
    needle: [f32; 4],
}

impl RenderColors {
    fn new(colors: &Colors) -> Self {
        Self {
            surface: egui::Rgba::from(colors.surface).to_array(),
            fill: egui::Rgba::from(colors.state_fill).to_array(),
            outline: egui::Rgba::from(colors.state_outline).to_array(),
            outline_zero: egui::Rgba::from(colors.state_outline_zero).to_array(),
            needle: egui::Rgba::from(colors.state_needle).to_array(),
        }
    }
}

const STATE_WORKGROUP_SIZE: u32 = 64;

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

struct StateVectorResources {
    compute_pipeline: wgpu::ComputePipeline,
    render_pipeline: wgpu::RenderPipeline,
    compute_bind_groups: [wgpu::BindGroup; 2],
    render_bind_groups: [wgpu::BindGroup; 2],
    render_bind_group_layout: wgpu::BindGroupLayout,
    gate_params_buffer: wgpu::Buffer,
    render_params_buffer: wgpu::Buffer,
    state_buffers: [wgpu::Buffer; 2],
    instance_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    target_format: wgpu::TextureFormat,
    state_count: usize,
    active_state: usize,
}

impl StateVectorResources {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
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

    fn update_render_pipeline(
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

struct StateVectorCallback {
    instances: Arc<[StateInstance]>,
    instances_dirty: bool,
    gate_params: Vec<GateParams>,
    state_count: usize,
    recompute: bool,
    target_format: wgpu::TextureFormat,
    colors: RenderColors,
}

impl egui_wgpu::CallbackTrait for StateVectorCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let resources = if callback_resources.contains::<StateVectorResources>() {
            callback_resources
                .get_mut::<StateVectorResources>()
                .expect("StateVectorResources missing")
        } else {
            callback_resources.insert(StateVectorResources::new(device, self.target_format));
            callback_resources
                .get_mut::<StateVectorResources>()
                .expect("StateVectorResources just inserted")
        };

        resources.update_render_pipeline(device, self.target_format);

        let screen_size = [
            screen_descriptor.size_in_pixels[0] as f32 / screen_descriptor.pixels_per_point,
            screen_descriptor.size_in_pixels[1] as f32 / screen_descriptor.pixels_per_point,
        ];
        let render_params = RenderParams {
            screen_size,
            _pad0: [0.0, 0.0],
            surface: self.colors.surface,
            fill: self.colors.fill,
            outline: self.colors.outline,
            outline_zero: self.colors.outline_zero,
            needle: self.colors.needle,
        };
        queue.write_buffer(
            &resources.render_params_buffer,
            0,
            bytemuck::bytes_of(&render_params),
        );

        let should_update_instances = self.instances_dirty || resources.state_count == 0;
        if should_update_instances && !self.instances.is_empty() {
            queue.write_buffer(
                &resources.instance_buffer,
                0,
                bytemuck::cast_slice(self.instances.as_ref()),
            );
        }

        if self.recompute || resources.state_count != self.state_count {
            resources.state_count = self.state_count;
            if self.state_count > 0 {
                let mut initial = vec![[0.0f32, 0.0f32]; self.state_count];
                initial[0] = [1.0, 0.0];
                queue.write_buffer(
                    &resources.state_buffers[0],
                    0,
                    bytemuck::cast_slice(&initial),
                );
            }
            resources.active_state = 0;
            let pair_count = (self.state_count / 2) as u32;
            if pair_count > 0 && !self.gate_params.is_empty() {
                let dispatch_x = pair_count.div_ceil(STATE_WORKGROUP_SIZE);
                let mut in_index = 0usize;
                for gate in &self.gate_params {
                    queue.write_buffer(&resources.gate_params_buffer, 0, bytemuck::bytes_of(gate));
                    let mut encoder =
                        device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("state_vector_compute_encoder"),
                        });
                    {
                        let mut pass =
                            encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                                label: Some("state_vector_compute_pass"),
                                timestamp_writes: None,
                            });
                        pass.set_pipeline(&resources.compute_pipeline);
                        pass.set_bind_group(0, &resources.compute_bind_groups[in_index], &[]);
                        pass.dispatch_workgroups(dispatch_x, 1, 1);
                    }
                    queue.submit(Some(encoder.finish()));
                    in_index = 1 - in_index;
                }
                resources.active_state = in_index;
            }
        }

        GPU_READBACK.with(|slot| {
            *slot.borrow_mut() = Some(GpuReadbackState {
                device: device.clone(),
                queue: queue.clone(),
                state_buffers: [
                    resources.state_buffers[0].clone(),
                    resources.state_buffers[1].clone(),
                ],
                state_count: resources.state_count,
                active_state: resources.active_state,
            });
        });

        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        let Some(resources) = callback_resources.get::<StateVectorResources>() else {
            return;
        };
        if self.instances.is_empty() {
            return;
        }
        render_pass.set_pipeline(&resources.render_pipeline);
        render_pass.set_bind_group(
            0,
            &resources.render_bind_groups[resources.active_state],
            &[],
        );
        render_pass.set_vertex_buffer(0, resources.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, resources.instance_buffer.slice(..));
        render_pass.set_index_buffer(resources.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..resources.index_count, 0, 0..self.instances.len() as u32);
    }
}

fn amplitude_qubits(len: usize) -> usize {
    let mut qubits = 0;
    let mut size = 1usize;
    if len == 0 {
        return 1;
    }
    while size < len {
        size <<= 1;
        qubits += 1;
    }
    qubits.max(1)
}

fn color_rgba(r: f32, g: f32, b: f32, a: f32) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
        (a * 255.0).round() as u8,
    )
}

struct QniApp {
    next_gate_id: u32,
    placed_gates: Vec<PlacedGate>,
    dragging: Option<DragState>,
    drag_state_count: Option<usize>,
    state_panel_drag: Option<egui::Vec2>,
    state_panel_offset: egui::Vec2,
    hovered_gate_id: Option<u32>,
    hovered_palette_index: Option<usize>,
    qubit_count: usize,
    last_state_count: usize,
    needs_recompute: bool,
    last_content_rect: Option<egui::Rect>,
    drag_cursor_pos: Option<egui::Pos2>,
    state_instance_cache: Option<StateInstanceCache>,
    drag_repaint_deadline: Option<f64>,
    drag_repaint_pending: bool,
    startup_repaint_until: f64,
    pointer_was_down: bool,
}

impl QniApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        cc.egui_ctx.style_mut(|style| {
            style.spacing.window_margin = egui::Margin::same(0);
        });
        cc.egui_ctx.request_repaint();
        Self {
            next_gate_id: 1,
            placed_gates: Vec::new(),
            dragging: None,
            drag_state_count: None,
            state_panel_drag: None,
            state_panel_offset: egui::Vec2::ZERO,
            hovered_gate_id: None,
            hovered_palette_index: None,
            qubit_count: MIN_QUBITS,
            last_state_count: 2,
            needs_recompute: true,
            last_content_rect: None,
            drag_cursor_pos: None,
            state_instance_cache: None,
            drag_repaint_deadline: None,
            drag_repaint_pending: false,
            startup_repaint_until: now_seconds() + 0.5,
            pointer_was_down: false,
        }
    }

    fn layout_qubits(&self) -> usize {
        let mut count = self.qubit_count.clamp(MIN_QUBITS, MAX_QUBITS);
        if self.dragging.is_some() && count < MAX_QUBITS {
            count += 1;
        }
        count
    }

    fn state_qubits(&self) -> usize {
        let mut max_wire: Option<usize> = None;
        for gate in &self.placed_gates {
            max_wire = Some(match max_wire {
                Some(current) => current.max(gate.wire),
                None => gate.wire,
            });
        }
        let count = max_wire.map_or(1, |wire| wire + 1);
        count.clamp(1, MAX_QUBITS)
    }

    fn update_qubit_count(&mut self) {
        let mut max_wire = MIN_QUBITS - 1;
        for gate in &self.placed_gates {
            max_wire = max_wire.max(gate.wire);
        }
        self.qubit_count = (max_wire + 1).clamp(MIN_QUBITS, MAX_QUBITS);
    }

    fn state_count(&self) -> usize {
        1usize << self.state_qubits()
    }

    fn collect_gate_params(
        &self,
        qubits: usize,
        state_count: usize,
        metrics: &LayoutMetrics,
    ) -> Vec<GateParams> {
        let mut gates: Vec<&PlacedGate> = self.placed_gates.iter().collect();
        gates.sort_by(|a, b| {
            a.pos
                .x
                .partial_cmp(&b.pos.x)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.id.cmp(&b.id))
        });

        struct GateGroup<'a> {
            controls: Vec<&'a PlacedGate>,
            targets: Vec<&'a PlacedGate>,
            slot_x: f32,
            min_id: u32,
        }

        let mut groups: HashMap<usize, GateGroup<'_>> = HashMap::new();
        for gate in &gates {
            let center_x = gate.pos.x + GATE_SIZE / 2.0;
            let Some((slot_index, distance)) = nearest_slot_index(center_x, &metrics.slot_centers)
            else {
                continue;
            };
            if distance > SNAP_DISTANCE {
                continue;
            }
            let entry = groups.entry(slot_index).or_insert_with(|| GateGroup {
                controls: Vec::new(),
                targets: Vec::new(),
                slot_x: metrics.slot_centers[slot_index],
                min_id: gate.id,
            });
            entry.min_id = entry.min_id.min(gate.id);
            if gate.kind == GateKind::Control {
                entry.controls.push(*gate);
            } else {
                entry.targets.push(*gate);
            }
        }

        let mut used_ids = HashSet::new();
        let mut ops: Vec<(f32, u32, GateParams)> = Vec::new();
        for group in groups.values() {
            let mut control_mask = 0u32;
            let mut control_value = 0u32;
            for control in &group.controls {
                if control.wire >= qubits {
                    continue;
                }
                let control_bit = (qubits.saturating_sub(1).saturating_sub(control.wire)) as u32;
                let bit_mask = 1u32 << control_bit;
                control_mask |= bit_mask;
                control_value |= bit_mask;
                used_ids.insert(control.id);
            }
            for target in &group.targets {
                if target.wire >= qubits {
                    continue;
                }
                if target.kind == GateKind::Swap {
                    continue;
                }
                let bit = (qubits.saturating_sub(1).saturating_sub(target.wire)) as u32;
                let params = if control_mask == 0 {
                    gate_params(target.kind, bit, state_count as u32)
                } else {
                    gate_params_controlled(
                        target.kind,
                        bit,
                        control_mask,
                        control_value,
                        state_count as u32,
                    )
                };
                ops.push((group.slot_x, group.min_id.min(target.id), params));
                used_ids.insert(target.id);
            }
        }

        for gate in gates {
            if gate.wire >= qubits {
                continue;
            }
            if gate.kind == GateKind::Swap {
                continue;
            }
            if gate.kind == GateKind::Control {
                continue;
            }
            if used_ids.contains(&gate.id) {
                continue;
            }
            let bit = (qubits.saturating_sub(1).saturating_sub(gate.wire)) as u32;
            ops.push((gate.pos.x, gate.id, gate_params(gate.kind, bit, state_count as u32)));
        }

        ops.sort_by(|a, b| {
            a.0.partial_cmp(&b.0)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.1.cmp(&b.1))
        });
        ops.into_iter().map(|(_, _, params)| params).collect()
    }

    fn handle_input(
        &mut self,
        content_rect: egui::Rect,
        ctx: &egui::Context,
        screen_rect: egui::Rect,
    ) {
        let pointer = ctx.input(|input| input.pointer.clone());
        let pos = pointer.latest_pos();
        let pointer_down = pointer.primary_down();
        let pointer_pressed = pointer.primary_pressed();
        let pointer_released = pointer.primary_released();

        let pointer_start = pointer_pressed || (pointer_down && !self.pointer_was_down);
        self.pointer_was_down = pointer_down;
        let local_pos = pos.map(|p| egui::pos2(p.x - content_rect.min.x, p.y - content_rect.min.y));
        let palette_width = PALETTE_GATES.len() as f32 * PALETTE_SIZE
            + (PALETTE_GATES.len() as f32 - 1.0) * PALETTE_GAP;
        let palette_start_x = screen_rect.width() / 2.0 - palette_width / 2.0;
        let palette_rect = egui::Rect::from_min_size(
            egui::pos2(
                screen_rect.min.x + palette_start_x,
                screen_rect.min.y + PALETTE_ROW_Y,
            ),
            egui::vec2(palette_width, PALETTE_SIZE),
        );
        let metrics = layout_metrics(content_rect.width(), self.layout_qubits());

        if pointer_start {
            if let Some(cursor) = local_pos {
                if let Some((gate_id, offset)) = self
                    .placed_gates
                    .iter()
                    .rev()
                    .find(|gate| {
                        let gate_rect =
                            egui::Rect::from_min_size(gate.pos, egui::vec2(GATE_SIZE, GATE_SIZE));
                        gate_rect.contains(cursor)
                    })
                    .map(|gate| (gate.id, cursor - gate.pos))
                {
                    self.dragging = Some(DragState { id: gate_id, offset });
                    self.drag_state_count = Some(self.state_count());
                    self.drag_cursor_pos = Some(cursor);
                    ctx.request_repaint();
                    self.hovered_gate_id = None;
                    self.hovered_palette_index = None;
                    return;
                }

                if let Some(cursor_screen) = pos {
                    if palette_rect.contains(cursor_screen) {
                        let local_x = cursor_screen.x - (screen_rect.min.x + palette_start_x);
                        let index = (local_x / (PALETTE_SIZE + PALETTE_GAP)).floor() as i32;
                        if index >= 0 && (index as usize) < PALETTE_GATES.len() {
                            let in_box = local_x - index as f32 * (PALETTE_SIZE + PALETTE_GAP)
                                <= PALETTE_SIZE;
                            if in_box {
                                let new_id = self.next_gate_id;
                                let new_gate = PlacedGate {
                                    id: new_id,
                                    kind: PALETTE_GATES[index as usize],
                                    pos: egui::pos2(
                                        cursor.x - GATE_SIZE / 2.0,
                                        cursor.y - GATE_SIZE / 2.0,
                                    ),
                                    wire: 0,
                                };
                                self.next_gate_id += 1;
                                self.placed_gates.push(new_gate);
                                self.dragging = Some(DragState {
                                    id: new_id,
                                    offset: egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0),
                                });
                                self.drag_state_count = Some(self.state_count());
                                self.drag_cursor_pos = Some(cursor);
                                ctx.request_repaint();
                                self.hovered_palette_index = None;
                                self.hovered_gate_id = None;
                                return;
                            }
                        }
                    }
                }
            }
        }

        if let Some(drag) = self.dragging.as_ref() {
            if pointer_down || pointer_released {
                let cursor = local_pos.or(self.drag_cursor_pos);
                if let Some(cursor) = cursor {
                    self.drag_cursor_pos = Some(cursor);
                    if let Some(index) =
                        self.placed_gates.iter().position(|gate| gate.id == drag.id)
                    {
                        let mut next_pos = cursor - drag.offset;
                        let mut next_wire = self.placed_gates[index].wire;
                        let center_y = next_pos.y + GATE_SIZE / 2.0;
                        let (line_y, distance, line_index) =
                            nearest_line(center_y, &metrics.line_ys);
                        if distance <= SNAP_DISTANCE {
                            next_pos.y = line_y - GATE_SIZE / 2.0;
                            next_wire = line_index;
                            let center_x = next_pos.x + GATE_SIZE / 2.0;
                            if let Some((slot_center, _)) = nearest_available_slot(
                                center_x,
                                line_index,
                                Some(drag.id),
                                &self.placed_gates,
                                &metrics.slot_centers,
                            ) {
                                next_pos.x = slot_center - GATE_SIZE / 2.0;
                            }
                        }
                        let gate = &mut self.placed_gates[index];
                        gate.pos = next_pos;
                        gate.wire = next_wire;
                    }
                }
            }
        } else if let Some(cursor) = local_pos {
            let mut hovered_gate = None;
            for gate in &self.placed_gates {
                let gate_rect =
                    egui::Rect::from_min_size(gate.pos, egui::vec2(GATE_SIZE, GATE_SIZE));
                if gate_rect.contains(cursor) {
                    hovered_gate = Some(gate.id);
                    break;
                }
            }
            self.hovered_gate_id = hovered_gate;

            let mut hovered_palette = None;
            if let Some(cursor_screen) = pos {
                if palette_rect.contains(cursor_screen) {
                    let local_x = cursor_screen.x - (screen_rect.min.x + palette_start_x);
                    let index = (local_x / (PALETTE_SIZE + PALETTE_GAP)).floor() as i32;
                    if index >= 0 && (index as usize) < PALETTE_GATES.len() {
                        let in_box = local_x - index as f32 * (PALETTE_SIZE + PALETTE_GAP)
                            <= PALETTE_SIZE;
                        if in_box {
                            hovered_palette = Some(index as usize);
                        }
                    }
                }
            }
            self.hovered_palette_index = hovered_palette;
        } else {
            self.hovered_gate_id = None;
            self.hovered_palette_index = None;
        }

        if pointer_released {
            if let Some(drag) = self.dragging.take() {
                if let Some(index) = self.placed_gates.iter().position(|gate| gate.id == drag.id) {
                    let gate_pos = self.placed_gates[index].pos;
                    let gate_id = self.placed_gates[index].id;
                    let center_x = gate_pos.x + GATE_SIZE / 2.0;
                    let center_y = gate_pos.y + GATE_SIZE / 2.0;
                    let (line_y, distance, line_index) = nearest_line(center_y, &metrics.line_ys);
                    let snapped = nearest_available_slot(
                        center_x,
                        line_index,
                        Some(gate_id),
                        &self.placed_gates,
                        &metrics.slot_centers,
                    );
                    let on_circuit = center_x >= metrics.slot_left
                        && center_x <= metrics.slot_right
                        && distance <= SNAP_DISTANCE
                        && snapped.map(|(_, d)| d <= SNAP_DISTANCE).unwrap_or(false);

                    if !on_circuit {
                        self.placed_gates.remove(index);
                    } else if let Some((slot_center, _)) = snapped {
                        let gate = &mut self.placed_gates[index];
                        gate.pos.x = slot_center - GATE_SIZE / 2.0;
                        gate.pos.y = line_y - GATE_SIZE / 2.0;
                        gate.wire = line_index;
                    }
                    self.update_qubit_count();
                    self.needs_recompute = true;
                    ctx.request_repaint();
                }
            }
            self.drag_state_count = None;
            self.drag_repaint_deadline = None;
            self.drag_repaint_pending = false;
            self.drag_cursor_pos = None;
        }

        if self.dragging.is_some() && pointer_down {
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if self.hovered_gate_id.is_some() || self.hovered_palette_index.is_some() {
            ctx.set_cursor_icon(egui::CursorIcon::Grab);
        }
    }

    fn circuit_content_height(&self, qubit_count: usize, screen_height: f32) -> f32 {
        let line_count = qubit_count.max(1);
        let last_line_y = LINE_Y + LINE_GAP * (line_count.saturating_sub(1)) as f32;
        let content_height = last_line_y + GATE_SIZE + 4.0 * REM;
        content_height.max(screen_height)
    }

    fn draw_circuit(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        metrics: &LayoutMetrics,
        colors: &Colors,
        fast_drag: bool,
    ) {
        for &line_y in &metrics.line_ys {
            let start = rect.min + egui::vec2(metrics.line_left, line_y);
            let end = rect.min + egui::vec2(metrics.line_right, line_y);
            painter.line_segment([start, end], egui::Stroke::new(2.0, colors.line));
        }

        if !fast_drag {
            let mut control_groups: HashMap<usize, (Vec<egui::Pos2>, Vec<egui::Pos2>)> =
                HashMap::new();
            for gate in &self.placed_gates {
                if gate.kind == GateKind::Swap {
                    continue;
                }
                let is_control = gate.kind == GateKind::Control;
                let center_x = gate.pos.x + GATE_SIZE / 2.0;
                if let Some((slot_index, distance)) =
                    nearest_slot_index(center_x, &metrics.slot_centers)
                {
                    if distance > SNAP_DISTANCE {
                        continue;
                    }
                    let center = rect.min
                        + gate.pos.to_vec2()
                        + egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0);
                    let entry = control_groups.entry(slot_index).or_insert((Vec::new(), Vec::new()));
                    if is_control {
                        entry.0.push(center);
                    } else {
                        entry.1.push(center);
                    }
                }
            }

            for (_, (controls, targets)) in control_groups {
                if controls.is_empty() || targets.is_empty() {
                    continue;
                }
                let mut min_y = f32::INFINITY;
                let mut max_y = f32::NEG_INFINITY;
                let mut xs = Vec::with_capacity(controls.len() + targets.len());
                for point in controls.iter().chain(targets.iter()) {
                    min_y = min_y.min(point.y);
                    max_y = max_y.max(point.y);
                    xs.push(point.x);
                }
                let x = if xs.is_empty() {
                    continue;
                } else {
                    xs.iter().sum::<f32>() / xs.len() as f32
                };
                let stroke = egui::Stroke::new(GATE_SIZE / 12.0, colors.box_fill);
                painter.line_segment([egui::pos2(x, min_y), egui::pos2(x, max_y)], stroke);
            }

            let mut swap_groups: HashMap<usize, Vec<&PlacedGate>> = HashMap::new();
            for gate in &self.placed_gates {
                if gate.kind != GateKind::Swap {
                    continue;
                }
                let center_x = gate.pos.x + GATE_SIZE / 2.0;
                if let Some((slot_index, distance)) =
                    nearest_slot_index(center_x, &metrics.slot_centers)
                {
                    if distance <= SNAP_DISTANCE {
                        swap_groups.entry(slot_index).or_default().push(gate);
                    }
                }
            }

            for (_, gates) in swap_groups {
                if gates.len() < 2 {
                    continue;
                }
                let mut centers = gates
                    .iter()
                    .map(|gate| {
                        rect.min + gate.pos.to_vec2() + egui::vec2(GATE_SIZE / 2.0, GATE_SIZE / 2.0)
                    })
                    .collect::<Vec<_>>();
                centers.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap_or(Ordering::Equal));
                let top = centers.first().copied();
                let bottom = centers.last().copied();
                if let (Some(top), Some(bottom)) = (top, bottom) {
                    let swap_stroke = egui::Stroke::new(GATE_SIZE / 12.0, colors.box_fill);
                    painter.line_segment([top, bottom], swap_stroke);
                }
            }
        }

        for gate in &self.placed_gates {
            let gate_rect = egui::Rect::from_min_size(
                rect.min + gate.pos.to_vec2(),
                egui::vec2(GATE_SIZE, GATE_SIZE),
            );
            if !fast_drag && self.hovered_gate_id == Some(gate.id) {
                let hover_outer = gate_rect.expand(4.0);
                let hover_inner = gate_rect.expand(2.0);
                painter.rect_filled(hover_outer, egui::CornerRadius::same(10), colors.box_border);
                painter.rect_filled(hover_inner, egui::CornerRadius::same(8), colors.background);
            }
            if should_use_fast_gate_body(fast_drag, self.dragging, gate.id) {
                draw_gate_body_fast(painter, gate_rect, gate.kind, colors);
            } else {
                draw_gate_body(painter, gate_rect, gate.kind, colors);
            }
        }

        for (index, &line_y) in metrics.line_ys.iter().enumerate() {
            let label_pos = rect.min + egui::vec2(CIRCUIT_PADDING, line_y - 7.0);
            painter.text(
                label_pos,
                egui::Align2::LEFT_TOP,
                format!("q{index}:"),
                egui::FontId::proportional(14.0),
                colors.text,
            );
        }
    }

    fn draw_palette(&self, painter: &egui::Painter, rect: egui::Rect, colors: &Colors) {
        let palette_width = PALETTE_GATES.len() as f32 * PALETTE_SIZE
            + (PALETTE_GATES.len() as f32 - 1.0) * PALETTE_GAP;
        let palette_start_x = rect.width() / 2.0 - palette_width / 2.0;
        let palette_padding = 1.0 * REM;
        let palette_rect = egui::Rect::from_min_size(
            rect.min
                + egui::vec2(
                    palette_start_x - palette_padding,
                    PALETTE_ROW_Y - palette_padding,
                ),
            egui::vec2(
                palette_width + palette_padding * 2.0,
                PALETTE_SIZE + palette_padding * 2.0,
            ),
        );
        let palette_corner = egui::CornerRadius::same(14);
        let shadow = egui::epaint::Shadow {
            offset: [0, 6],
            blur: 16,
            spread: 0,
            color: egui::Color32::from_rgba_unmultiplied(0, 0, 0, 25),
        };
        painter.add(egui::Shape::Rect(
            shadow.as_shape(palette_rect, palette_corner),
        ));
        painter.rect_filled(palette_rect, palette_corner, colors.surface);

        for (index, gate) in PALETTE_GATES.iter().enumerate() {
            let gate_x = palette_start_x + index as f32 * (PALETTE_SIZE + PALETTE_GAP);
            let gate_rect = egui::Rect::from_min_size(
                rect.min + egui::vec2(gate_x, PALETTE_ROW_Y),
                egui::vec2(PALETTE_SIZE, PALETTE_SIZE),
            );
            if self.hovered_palette_index == Some(index) {
                let hover_outer = gate_rect.expand(4.0);
                let hover_inner = gate_rect.expand(2.0);
                painter.rect_filled(hover_outer, egui::CornerRadius::same(10), colors.box_border);
                painter.rect_filled(hover_inner, egui::CornerRadius::same(8), colors.background);
            }
            draw_gate_body(painter, gate_rect, *gate, colors);
        }
    }

    fn state_panel_layout(&self, rect: egui::Rect, state_count: usize) -> StatePanelLayout {
        let state_count = state_count.max(1);
        let qubits = amplitude_qubits(state_count);
        let gap_ratio = STATE_CIRCLE_GAP / STATE_CIRCLE_SIZE;
        let state_padding = (1.0 * REM)
            .min(rect.width() * 0.05)
            .min(rect.height() * 0.05);
        let top_limit = rect.min.y + PALETTE_ROW_Y + PALETTE_SIZE + 2.0 * REM;
        let mut available_width = rect.width() - state_padding * 2.0;
        let mut available_height = rect.max.y - STATE_CIRCLE_BOTTOM_MARGIN - top_limit;
        if available_width <= 0.0 {
            available_width = rect.width().max(1.0);
        }
        if available_height <= 0.0 {
            available_height = (rect.height() - STATE_CIRCLE_BOTTOM_MARGIN).max(1.0);
        }
        let max_fraction = if state_count <= 4 {
            0.4
        } else if state_count <= 16 {
            0.3
        } else {
            0.25
        };
        let max_height = rect.height() * max_fraction;
        if available_height > max_height {
            available_height = max_height.max(1.0);
        }

        let aspect = (available_width / available_height).max(0.1);
        let mut columns = 1usize;
        let mut rows = state_count;
        let mut best_size = 0.0;
        let mut best_score = f32::INFINITY;
        for candidate in 1..=state_count {
            if !state_count.is_multiple_of(candidate) {
                continue;
            }
            let candidate_rows = state_count / candidate;
            let size_w = available_width / (candidate as f32 + (candidate - 1) as f32 * gap_ratio);
            let size_h = available_height
                / (candidate_rows as f32 + (candidate_rows - 1) as f32 * gap_ratio);
            let size = size_w.min(size_h).clamp(0.5, STATE_CIRCLE_SIZE);
            let ratio = candidate as f32 / candidate_rows as f32;
            let score = (ratio - aspect).abs();
            if size > best_size + 0.01 || ((size - best_size).abs() <= 0.01 && score < best_score) {
                columns = candidate;
                rows = candidate_rows;
                best_size = size;
                best_score = score;
            }
        }
        let size = best_size.max(0.5);
        let gap = size * gap_ratio;
        let total_width = size * columns as f32 + gap * (columns.saturating_sub(1)) as f32;
        let total_height = size * rows as f32 + gap * (rows.saturating_sub(1)) as f32;
        let base_x = rect.width() / 2.0 - total_width / 2.0;
        let base_y = rect.height() - STATE_CIRCLE_BOTTOM_MARGIN - total_height;
        let radius = size * 0.5;
        let stroke = STATE_CIRCLE_STROKE.min(size * 0.25).max(0.5);
        let scale = size / STATE_CIRCLE_SIZE;
        let inner_radius = (radius - stroke * 0.5 + 0.5 * scale).max(0.0);

        // Calculate handle height based on content height (before adding handle to panel)
        let content_height = total_height + state_padding * 2.0;
        let handle_height = (0.4 * REM)
            .min(content_height * 0.4)
            .max(10.0);

        // Add extra padding below handle to visually balance top and bottom.
        // The handle adds visual weight at the top, so we compensate by adding
        // half the handle height as extra space below it.
        let handle_padding = handle_height * 0.5;

        // base_pos is shifted down by handle_height + handle_padding so circles
        // start below the handle with visually balanced padding
        let base_pos = rect.min + egui::vec2(base_x, base_y);
        let state_rect = egui::Rect::from_min_size(
            base_pos - egui::vec2(state_padding, state_padding + handle_height + handle_padding),
            egui::vec2(
                total_width + state_padding * 2.0,
                total_height + state_padding * 2.0 + handle_height + handle_padding,
            ),
        );

        StatePanelLayout {
            state_count,
            qubits,
            columns,
            size,
            gap,
            radius,
            stroke,
            inner_radius,
            base_pos,
            state_rect,
            handle_height,
        }
    }

    fn clamp_state_panel_offset(&mut self, layout: &StatePanelLayout, rect: egui::Rect) {
        let min_x = rect.min.x;
        let max_x = rect.max.x - layout.state_rect.width();
        let min_y = rect.min.y;
        let max_y = rect.max.y - layout.state_rect.height();
        let base_min = layout.state_rect.min;
        let min_offset_x = min_x - base_min.x;
        let max_offset_x = max_x - base_min.x;
        let min_offset_y = min_y - base_min.y;
        let max_offset_y = max_y - base_min.y;

        self.state_panel_offset.x = if max_offset_x < min_offset_x {
            min_offset_x
        } else {
            self.state_panel_offset.x.clamp(min_offset_x, max_offset_x)
        };
        self.state_panel_offset.y = if max_offset_y < min_offset_y {
            min_offset_y
        } else {
            self.state_panel_offset.y.clamp(min_offset_y, max_offset_y)
        };
    }

    fn state_instances_for(
        &mut self,
        layout: &StatePanelLayout,
        origin: egui::Pos2,
    ) -> (Arc<[StateInstance]>, bool) {
        let key = StateInstanceKey {
            state_count: layout.state_count,
            columns: layout.columns,
            size: layout.size,
            gap: layout.gap,
            radius: layout.radius,
            inner_radius: layout.inner_radius,
            stroke: layout.stroke,
            origin,
        };
        if let Some(cache) = &self.state_instance_cache {
            if cache.key == key {
                return (cache.instances.clone(), false);
            }
        }

        let mut instances = Vec::with_capacity(layout.state_count);
        for i in 0..layout.state_count {
            let state_index = display_index_to_state_index(i, layout.qubits) as u32;
            let row = i / layout.columns;
            let col = i % layout.columns;
            let x = origin.x + col as f32 * (layout.size + layout.gap);
            let y = origin.y + row as f32 * (layout.size + layout.gap);
            instances.push(StateInstance {
                center: [x + layout.radius, y + layout.radius],
                radius: layout.radius,
                inner_radius: layout.inner_radius,
                stroke: layout.stroke,
                state_index,
            });
        }
        let instances: Arc<[StateInstance]> = instances.into();
        self.state_instance_cache = Some(StateInstanceCache {
            key,
            instances: instances.clone(),
        });
        (instances, true)
    }

    fn schedule_drag_repaint(&mut self, ctx: &egui::Context, frame_secs: f64) {
        let now = now_seconds();
        let deadline = self.drag_repaint_deadline.unwrap_or(now);
        if now >= deadline {
            let delay = (DRAG_REPAINT_BASE_SECS + frame_secs * DRAG_REPAINT_PUMP_FACTOR)
                .clamp(DRAG_REPAINT_MIN_SECS, DRAG_REPAINT_MAX_SECS);
            self.drag_repaint_deadline = Some(now + delay);
            self.drag_repaint_pending = false;
            ctx.request_repaint();
        } else if !self.drag_repaint_pending {
            self.drag_repaint_pending = true;
            let remaining = (deadline - now).max(0.0);
            ctx.request_repaint_after(Duration::from_secs_f64(remaining));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_state_vector(
        &mut self,
        painter: &egui::Painter,
        colors: &Colors,
        layout: &StatePanelLayout,
        offset: egui::Vec2,
        handle_height: f32,
        screen_rect: egui::Rect,
        recompute: bool,
        target_format: Option<wgpu::TextureFormat>,
    ) -> egui::Rect {
        let state_rect = layout.state_rect.translate(offset);
        let base_pos = layout.base_pos + offset;
        let state_corner = egui::CornerRadius::same(14);
        let state_shadow = egui::epaint::Shadow {
            offset: [0, 6],
            blur: 16,
            spread: 0,
            color: egui::Color32::from_rgba_unmultiplied(0, 0, 0, 25),
        };
        painter.add(egui::Shape::Rect(
            state_shadow.as_shape(state_rect, state_corner),
        ));
        painter.rect_filled(state_rect, state_corner, colors.surface);

        let handle_rect = egui::Rect::from_min_size(
            state_rect.min,
            egui::vec2(state_rect.width(), handle_height.max(6.0)),
        );
        painter.rect_filled(handle_rect, state_corner, colors.box_border);
        let grip_width = handle_rect.width() * 0.25;
        let grip_height = handle_height * 0.25;
        let grip_rect = egui::Rect::from_center_size(
            handle_rect.center(),
            egui::vec2(grip_width, grip_height.max(2.0)),
        );
        painter.rect_filled(grip_rect, egui::CornerRadius::same(4), colors.surface);

        if let Some(target_format) = target_format {
            let (instances, instances_dirty) = self.state_instances_for(layout, base_pos);
            let gate_params = if recompute {
                let metrics = layout_metrics(screen_rect.width(), layout.qubits);
                self.collect_gate_params(layout.qubits, layout.state_count, &metrics)
            } else {
                Vec::new()
            };
            let render_colors = RenderColors::new(colors);
            let callback = StateVectorCallback {
                instances,
                instances_dirty,
                gate_params,
                state_count: layout.state_count,
                recompute,
                target_format,
                colors: render_colors,
            };
            let callback_rect = screen_rect;
            let clipped = painter.with_clip_rect(state_rect);
            let paint_callback = egui_wgpu::Callback::new_paint_callback(callback_rect, callback);
            clipped.add(egui::Shape::Callback(paint_callback));
        }

        handle_rect
    }
}

struct StatePanelLayout {
    state_count: usize,
    qubits: usize,
    columns: usize,
    size: f32,
    gap: f32,
    radius: f32,
    stroke: f32,
    inner_radius: f32,
    base_pos: egui::Pos2,
    state_rect: egui::Rect,
    handle_height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct StateInstanceKey {
    state_count: usize,
    columns: usize,
    size: f32,
    gap: f32,
    radius: f32,
    inner_radius: f32,
    stroke: f32,
    origin: egui::Pos2,
}

struct StateInstanceCache {
    key: StateInstanceKey,
    instances: Arc<[StateInstance]>,
}

#[derive(Clone, Copy)]
struct SvgPoint {
    x: f32,
    y: f32,
}

impl SvgPoint {
    fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

fn map_svg_point_in_rect(rect: egui::Rect, point: SvgPoint, viewbox: f32) -> egui::Pos2 {
    let scale_x = rect.width() / viewbox;
    let scale_y = rect.height() / viewbox;
    egui::pos2(
        rect.min.x + point.x * scale_x,
        rect.min.y + point.y * scale_y,
    )
}

fn push_cubic_points_viewbox(
    points: &mut Vec<egui::Pos2>,
    rect: egui::Rect,
    p0: SvgPoint,
    p1: SvgPoint,
    p2: SvgPoint,
    p3: SvgPoint,
    steps: usize,
    viewbox: f32,
) {
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let u = 1.0 - t;
        let uu = u * u;
        let tt = t * t;
        let uuu = uu * u;
        let ttt = tt * t;
        let x = uuu * p0.x + 3.0 * uu * t * p1.x + 3.0 * u * tt * p2.x + ttt * p3.x;
        let y = uuu * p0.y + 3.0 * uu * t * p1.y + 3.0 * u * tt * p2.y + ttt * p3.y;
        points.push(map_svg_point_in_rect(rect, SvgPoint::new(x, y), viewbox));
    }
}

fn draw_gate_body(painter: &egui::Painter, gate_rect: egui::Rect, kind: GateKind, colors: &Colors) {
    let is_swap = kind == GateKind::Swap;
    if !is_swap {
        painter.rect_filled(gate_rect, egui::CornerRadius::same(6), colors.box_fill);
    }
    let icon_color = if is_swap {
        colors.box_fill
    } else {
        colors.label
    };
    if !draw_gate_icon(painter, gate_rect, kind, icon_color) {
        painter.text(
            gate_rect.center(),
            egui::Align2::CENTER_CENTER,
            kind.label(),
            egui::FontId::proportional(18.0),
            colors.label,
        );
    }
}

fn draw_gate_body_fast(
    painter: &egui::Painter,
    gate_rect: egui::Rect,
    kind: GateKind,
    colors: &Colors,
) {
    if kind != GateKind::Swap {
        painter.rect_filled(gate_rect, egui::CornerRadius::same(6), colors.box_fill);
    }
    painter.text(
        gate_rect.center(),
        egui::Align2::CENTER_CENTER,
        kind.label(),
        egui::FontId::proportional(16.0),
        colors.label,
    );
}

fn draw_gate_icon(
    painter: &egui::Painter,
    rect: egui::Rect,
    kind: GateKind,
    color: egui::Color32,
) -> bool {
    let viewbox = 48.0;
    let scale = rect.width() / viewbox;
    let stroke = egui::Stroke::new(2.0 * scale, color);
    let p = |x: f32, y: f32| map_svg_point_in_rect(rect, SvgPoint::new(x, y), viewbox);

    match kind {
        GateKind::H => {
            painter.line_segment([p(17.0, 13.0), p(17.0, 35.0)], stroke);
            painter.line_segment([p(17.0, 24.0), p(31.0, 24.0)], stroke);
            painter.line_segment([p(31.0, 13.0), p(31.0, 35.0)], stroke);
            true
        }
        GateKind::Control => {
            painter.circle_filled(p(24.0, 24.0), 5.5 * scale, color);
            true
        }
        GateKind::X => {
            painter.line_segment([p(15.0, 24.0), p(33.0, 24.0)], stroke);
            painter.line_segment([p(24.0, 15.0), p(24.0, 33.0)], stroke);
            true
        }
        GateKind::Y => {
            painter.line_segment([p(17.0, 13.0), p(24.0, 24.0)], stroke);
            painter.line_segment([p(24.0, 24.0), p(31.0, 13.0)], stroke);
            painter.line_segment([p(24.0, 24.0), p(24.0, 35.0)], stroke);
            true
        }
        GateKind::Z => {
            let points = vec![p(17.5, 13.0), p(31.0, 13.0), p(17.5, 35.0), p(31.0, 35.0)];
            painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
                points, stroke,
            )));
            true
        }
        GateKind::S => {
            draw_s_curve(painter, rect, stroke);
            true
        }
        GateKind::SDagger => {
            draw_s_curve(painter, rect, stroke);
            painter.line_segment([p(37.0, 10.0), p(43.0, 10.0)], stroke);
            painter.line_segment([p(40.0, 6.0), p(40.0, 20.0)], stroke);
            true
        }
        GateKind::T => {
            painter.line_segment([p(15.0, 13.0), p(33.0, 13.0)], stroke);
            painter.line_segment([p(24.0, 13.0), p(24.0, 35.0)], stroke);
            true
        }
        GateKind::TDagger => {
            painter.line_segment([p(15.0, 13.0), p(33.0, 13.0)], stroke);
            painter.line_segment([p(24.0, 13.0), p(24.0, 35.0)], stroke);
            painter.line_segment([p(37.0, 10.0), p(43.0, 10.0)], stroke);
            painter.line_segment([p(40.0, 6.0), p(40.0, 20.0)], stroke);
            true
        }
        GateKind::SqrtX => {
            let points = vec![
                p(10.0, 24.0),
                p(13.0, 24.0),
                p(14.0, 36.0),
                p(17.0, 36.0),
                p(18.0, 12.0),
                p(38.0, 12.0),
            ];
            painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
                points, stroke,
            )));
            painter.line_segment([p(24.0, 32.0), p(34.0, 18.0)], stroke);
            painter.line_segment([p(34.0, 32.0), p(24.0, 18.0)], stroke);
            true
        }
        GateKind::Swap => {
            let scale = rect.width() / 48.0;
            let swap_stroke = egui::Stroke::new(4.0 * scale, color);
            painter.line_segment([p(12.0, 36.0), p(36.0, 12.0)], swap_stroke);
            painter.line_segment([p(12.0, 12.0), p(36.0, 36.0)], swap_stroke);
            true
        }
        GateKind::Phase => {
            painter.line_segment([p(18.2857, 36.0), p(29.7143, 12.0)], stroke);
            painter.circle_stroke(p(24.0, 24.5714), 8.0 * scale, stroke);
            true
        }
        GateKind::Rx => {
            draw_r_letter(painter, rect, stroke);
            painter.line_segment([p(34.6093, 13.0016), p(24.7475, 35.0)], stroke);
            painter.line_segment([p(24.8187, 13.0016), p(34.6093, 35.0)], stroke);
            true
        }
        GateKind::Ry => {
            draw_r_letter(painter, rect, stroke);
            painter.line_segment([p(34.6093, 13.0016), p(29.5, 23.5)], stroke);
            painter.line_segment([p(29.5, 23.5), p(29.5, 35.0)], stroke);
            painter.line_segment([p(24.5, 13.0), p(29.5, 23.5)], stroke);
            true
        }
        GateKind::Rz => {
            draw_r_letter(painter, rect, stroke);
            let points = vec![p(24.5, 13.0), p(34.5, 13.0), p(24.5, 35.0), p(34.5, 35.0)];
            painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
                points, stroke,
            )));
            true
        }
    }
}

fn draw_r_letter(painter: &egui::Painter, rect: egui::Rect, stroke: egui::Stroke) {
    let viewbox = 48.0;
    let p = |x: f32, y: f32| map_svg_point_in_rect(rect, SvgPoint::new(x, y), viewbox);
    painter.line_segment([p(12.3214, 35.0), p(12.3214, 24.0)], stroke);
    painter.line_segment([p(18.0, 24.5), p(21.7303, 35.0)], stroke);

    let mut points = Vec::new();
    let start = SvgPoint::new(12.3214, 24.0);
    points.push(p(start.x, start.y));
    points.push(p(12.3214, 13.0));
    push_cubic_points_viewbox(
        &mut points,
        rect,
        SvgPoint::new(12.3214, 13.0),
        SvgPoint::new(21.0, 13.0),
        SvgPoint::new(22.0, 15.5),
        SvgPoint::new(22.0, 18.5),
        10,
        viewbox,
    );
    push_cubic_points_viewbox(
        &mut points,
        rect,
        SvgPoint::new(22.0, 18.5),
        SvgPoint::new(22.0, 21.5),
        SvgPoint::new(21.0, 24.0),
        SvgPoint::new(12.3214, 24.0),
        10,
        viewbox,
    );
    painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
        points, stroke,
    )));
}

fn draw_s_curve(painter: &egui::Painter, rect: egui::Rect, stroke: egui::Stroke) {
    let viewbox = 48.0;
    let mut points = Vec::new();
    let start = SvgPoint::new(30.0, 15.5982);
    points.push(map_svg_point_in_rect(rect, start, viewbox));

    push_cubic_points_viewbox(
        &mut points,
        rect,
        start,
        SvgPoint::new(30.0, 15.5982),
        SvgPoint::new(29.0, 13.5893),
        SvgPoint::new(25.0, 13.3512),
        12,
        viewbox,
    );
    push_cubic_points_viewbox(
        &mut points,
        rect,
        SvgPoint::new(25.0, 13.3512),
        SvgPoint::new(21.5, 13.1429),
        SvgPoint::new(16.5, 13.8029),
        SvgPoint::new(17.0, 19.1515),
        12,
        viewbox,
    );
    push_cubic_points_viewbox(
        &mut points,
        rect,
        SvgPoint::new(17.0, 19.1515),
        SvgPoint::new(17.5, 24.5001),
        SvgPoint::new(31.0, 23.1432),
        SvgPoint::new(31.0, 29.035),
        12,
        viewbox,
    );
    push_cubic_points_viewbox(
        &mut points,
        rect,
        SvgPoint::new(31.0, 29.035),
        SvgPoint::new(31.0, 34.9268),
        SvgPoint::new(25.5934, 35.2343),
        SvgPoint::new(21.5, 34.9268),
        12,
        viewbox,
    );
    push_cubic_points_viewbox(
        &mut points,
        rect,
        SvgPoint::new(21.5, 34.9268),
        SvgPoint::new(19.0063, 34.7396),
        SvgPoint::new(17.0, 33.2578),
        SvgPoint::new(17.0, 33.2578),
        10,
        viewbox,
    );

    painter.add(egui::Shape::Path(egui::epaint::PathShape::line(
        points, stroke,
    )));
}

impl eframe::App for QniApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let frame_start = now_seconds();
        egui::CentralPanel::default().show(ctx, |ui| {
            let screen_rect = ui.max_rect();
            let colors = Colors::new();
            let content_height =
                self.circuit_content_height(self.layout_qubits(), screen_rect.height());

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .scroll_source(egui::scroll_area::ScrollSource {
                    drag: false,
                    ..egui::scroll_area::ScrollSource::default()
                })
                .show(ui, |ui| {
                    let (rect, _response) = ui.allocate_exact_size(
                        egui::vec2(screen_rect.width(), content_height),
                        egui::Sense::click_and_drag(),
                    );
                    self.handle_input(rect, ctx, screen_rect);
                    let content_changed =
                        self.last_content_rect.map_or(true, |last| last != rect);
                    self.last_content_rect = Some(rect);
                    if content_changed {
                        ctx.request_repaint();
                    }

                    let metrics = layout_metrics(rect.width(), self.layout_qubits());
                    let painter = ui.painter_at(rect);
                    let fast_drag = self.dragging.is_some();
                    self.draw_circuit(&painter, rect, &metrics, &colors, fast_drag);
                });

            let base_state_count = self.state_count();
            let state_count = if self.dragging.is_some() {
                self.drag_state_count.unwrap_or(base_state_count)
            } else {
                base_state_count
            };
            let mut recompute = self.needs_recompute || state_count != self.last_state_count;
            let state_layout = self.state_panel_layout(screen_rect, state_count);
            self.clamp_state_panel_offset(&state_layout, screen_rect);
            let state_rect = state_layout.state_rect.translate(self.state_panel_offset);
            let handle_rect = egui::Rect::from_min_size(
                state_rect.min,
                egui::vec2(state_rect.width(), state_layout.handle_height.max(6.0)),
            );
            let handle_response = ui.interact(
                handle_rect,
                egui::Id::new("state_panel_handle"),
                egui::Sense::drag(),
            );
            if handle_response.drag_started() {
                if let Some(pos) = handle_response.interact_pointer_pos() {
                    self.state_panel_drag = Some(pos - handle_rect.min);
                }
            }
            if handle_response.dragged() {
                if let (Some(pos), Some(offset)) = (
                    handle_response.interact_pointer_pos(),
                    self.state_panel_drag,
                ) {
                    let desired_min = pos - offset;
                    self.state_panel_offset = desired_min - state_layout.state_rect.min;
                    self.clamp_state_panel_offset(&state_layout, screen_rect);
                }
            }
            if handle_response.drag_stopped() {
                self.state_panel_drag = None;
            }

            let overlay_painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("overlay"),
            ));
            let target_format = frame.wgpu_render_state().map(|state| state.target_format);
            if target_format.is_some() {
                if recompute {
                    self.needs_recompute = false;
                    self.last_state_count = state_count;
                }
            } else if recompute {
                ctx.request_repaint();
                recompute = false;
            }
            self.draw_palette(&overlay_painter, screen_rect, &colors);
            self.draw_state_vector(
                &overlay_painter,
                &colors,
                &state_layout,
                self.state_panel_offset,
                state_layout.handle_height,
                screen_rect,
                recompute,
                target_format,
            );
        });

        let now = now_seconds();
        let frame_secs = (now - frame_start).max(0.0);
        if self.dragging.is_some() {
            self.schedule_drag_repaint(ctx, frame_secs);
        } else if self.drag_repaint_deadline.is_some() || self.drag_repaint_pending {
            self.drag_repaint_deadline = None;
            self.drag_repaint_pending = false;
        }
        if now < self.startup_repaint_until {
            ctx.request_repaint_after(Duration::from_secs_f64(DRAG_REPAINT_MIN_SECS));
        }
    }
}

struct Colors {
    background: egui::Color32,
    surface: egui::Color32,
    line: egui::Color32,
    box_fill: egui::Color32,
    box_border: egui::Color32,
    label: egui::Color32,
    text: egui::Color32,
    state_fill: egui::Color32,
    state_outline: egui::Color32,
    state_outline_zero: egui::Color32,
    state_needle: egui::Color32,
}

impl Colors {
    fn new() -> Self {
        Self {
            background: color_rgba(0.976, 0.98, 0.984, 1.0),
            surface: color_rgba(1.0, 1.0, 1.0, 1.0),
            line: color_rgba(0.72, 0.72, 0.72, 1.0),
            box_fill: color_rgba(0.2, 0.62, 0.55, 1.0),
            box_border: color_rgba(0.82, 0.82, 0.82, 1.0),
            label: color_rgba(1.0, 1.0, 1.0, 1.0),
            text: color_rgba(0.45, 0.45, 0.45, 1.0),
            state_fill: color_rgba(0.055, 0.647, 0.914, 1.0), // Tailwind sky-500: rgb(14, 165, 233)
            state_outline: color_rgba(0.0, 0.0, 0.0, 1.0),
            state_outline_zero: color_rgba(0.75, 0.75, 0.75, 1.0),
            state_needle: color_rgba(0.0, 0.0, 0.0, 1.0),
        }
    }
}

#[derive(Clone)]
struct GpuReadbackState {
    device: wgpu::Device,
    queue: wgpu::Queue,
    state_buffers: [wgpu::Buffer; 2],
    state_count: usize,
    active_state: usize,
}

#[cfg(target_arch = "wasm32")]
use futures_channel::oneshot;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn start(canvas_id: &str) -> Result<(), wasm_bindgen::JsValue> {
    let window =
        web_sys::window().ok_or_else(|| wasm_bindgen::JsValue::from_str("window not found"))?;
    let document = window
        .document()
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("document not found"))?;
    let canvas = document
        .get_element_by_id(canvas_id)
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("canvas not found"))?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;

    let web_options = eframe::WebOptions::default();
    eframe::WebRunner::new()
        .start(
            canvas,
            web_options,
            Box::new(|cc| Ok(Box::new(QniApp::new(cc)))),
        )
        .await
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn read_state_vector() -> Result<js_sys::Float32Array, wasm_bindgen::JsValue> {
    let Some(state) = GPU_READBACK.with(|slot| slot.borrow().clone()) else {
        return Err(wasm_bindgen::JsValue::from_str("state vector not ready"));
    };
    let byte_len = state.state_count * 2 * std::mem::size_of::<f32>();
    let staging = state.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("state_vector_readback"),
        size: byte_len as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = state
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("state_vector_readback_encoder"),
        });
    encoder.copy_buffer_to_buffer(
        &state.state_buffers[state.active_state],
        0,
        &staging,
        0,
        byte_len as wgpu::BufferAddress,
    );
    state.queue.submit(Some(encoder.finish()));

    let slice = staging.slice(..);
    let (sender, receiver) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    receiver
        .await
        .map_err(|_| wasm_bindgen::JsValue::from_str("readback dropped"))?
        .map_err(|err| wasm_bindgen::JsValue::from_str(&format!("map_async failed: {err:?}")))?;
    let data = slice.get_mapped_range();
    let floats: &[f32] = bytemuck::cast_slice(&data);
    let output = js_sys::Float32Array::new_with_length(floats.len() as u32);
    output.copy_from(floats);
    drop(data);
    staging.unmap();
    Ok(output)
}
