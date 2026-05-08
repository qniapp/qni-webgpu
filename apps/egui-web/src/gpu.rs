use eframe::egui;
use eframe::{egui_wgpu, wgpu};
use std::cell::RefCell;
use std::sync::Arc;
use wgpu::util::DeviceExt as _;

#[cfg(target_arch = "wasm32")]
use futures_channel::oneshot;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;

use crate::bloch::SimulationOp;
use crate::colors::Colors;
use crate::constants::MAX_STATE_COUNT;
use crate::gates::GateParams;

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
  mode: u32,
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
  // Write |0> / |1>: per-pair conditional X. Matches qni's CPU semantics —
  // X iff the qubit sits in the opposite basis state, no-op otherwise. For
  // unentangled product states each pair's |a0|/|a1| ratio is consistent,
  // so the local decision agrees with the global one.
  if (params.mode == 1u || params.mode == 2u) {
    let mag0 = a0.x * a0.x + a0.y * a0.y;
    let mag1 = a1.x * a1.x + a1.y * a1.y;
    let eps = 1.0e-6;
    var swap = false;
    if (params.mode == 1u) {
      swap = mag1 > mag0 + eps;
    } else {
      swap = mag0 > mag1 + eps;
    }
    if (swap) {
      state_out[i0] = a1;
      state_out[i1] = a0;
    } else {
      state_out[i0] = a0;
      state_out[i1] = a1;
    }
    return;
  }
  state_out[i0] = cmul(params.m00, a0) + cmul(params.m01, a1);
  state_out[i1] = cmul(params.m10, a0) + cmul(params.m11, a1);
}
"#;

/// Maximum number of Bloch displays whose vectors can be captured in a single
/// recompute. Each placed `BlochDisplay` occupies one slot in the GPU's
/// `bloch_output_buffer` (a vec4 per slot, .xyz used).
pub(crate) const MAX_BLOCH_SLOTS: usize = 64;

// Workgroup size for the Bloch reduction shader is hard-coded to 64 (matches
// `@workgroup_size(64)` in `BLOCH_REDUCE_SHADER`). One workgroup processes the
// entire state vector for a single qubit and reduces (ρ_00, ρ_11, Re(ρ_01),
// Im(ρ_01)) via shared memory.

const BLOCH_REDUCE_SHADER: &str = r#"
struct BlochParams {
  qubit_bit: u32,
  state_count: u32,
  output_slot: u32,
  _pad: u32,
};

@group(0) @binding(0) var<storage, read> state: array<vec2<f32>>;
@group(0) @binding(1) var<storage, read_write> bloch_out: array<vec4<f32>>;
@group(0) @binding(2) var<uniform> params: BlochParams;

var<workgroup> shared_rho00: array<f32, 64>;
var<workgroup> shared_rho11: array<f32, 64>;
var<workgroup> shared_rho01_re: array<f32, 64>;
var<workgroup> shared_rho01_im: array<f32, 64>;

@compute @workgroup_size(64)
fn main(@builtin(local_invocation_id) lid: vec3<u32>) {
  let tid = lid.x;
  let qubit_mask: u32 = 1u << params.qubit_bit;
  let total: u32 = params.state_count;

  var rho_00: f32 = 0.0;
  var rho_11: f32 = 0.0;
  var rho_01_re: f32 = 0.0;
  var rho_01_im: f32 = 0.0;

  // Each thread handles state indices striding by workgroup size. Only loop
  // over indices whose `qubit_bit` is 0 — the matching index_with_one is
  // looked up directly so we accumulate ρ_01 in the same iteration.
  var i: u32 = tid;
  loop {
    if (i >= total) { break; }
    let bit_is_zero: bool = (i & qubit_mask) == 0u;
    let amp = state[i];
    let mag2 = amp.x * amp.x + amp.y * amp.y;
    if (bit_is_zero) {
      rho_00 = rho_00 + mag2;
      let j: u32 = i | qubit_mask;
      let amp_j = state[j];
      // ρ_01 = Σ_rest amp_i · conj(amp_j)
      //   amp_i · conj(amp_j) = (a + bi)(c - di) = (ac + bd) + (bc - ad)i.
      rho_01_re = rho_01_re + (amp.x * amp_j.x + amp.y * amp_j.y);
      rho_01_im = rho_01_im + (amp.y * amp_j.x - amp.x * amp_j.y);
    } else {
      rho_11 = rho_11 + mag2;
    }
    i = i + 64u;
  }

  shared_rho00[tid] = rho_00;
  shared_rho11[tid] = rho_11;
  shared_rho01_re[tid] = rho_01_re;
  shared_rho01_im[tid] = rho_01_im;
  workgroupBarrier();

  // Tree reduction: 64 → 32 → 16 → 8 → 4 → 2 → 1.
  for (var step: u32 = 32u; step > 0u; step = step >> 1u) {
    if (tid < step) {
      shared_rho00[tid] = shared_rho00[tid] + shared_rho00[tid + step];
      shared_rho11[tid] = shared_rho11[tid] + shared_rho11[tid + step];
      shared_rho01_re[tid] = shared_rho01_re[tid] + shared_rho01_re[tid + step];
      shared_rho01_im[tid] = shared_rho01_im[tid] + shared_rho01_im[tid + step];
    }
    workgroupBarrier();
  }

  if (tid == 0u) {
    // qni convention (`packages/simulator/src/matrix.ts`):
    //   x = 2·Re(ρ_01), y = -2·Im(ρ_01), z = ρ_00 - ρ_11.
    let x: f32 =  2.0 * shared_rho01_re[0];
    let y: f32 = -2.0 * shared_rho01_im[0];
    let z: f32 = shared_rho00[0] - shared_rho11[0];
    bloch_out[params.output_slot] = vec4<f32>(x, y, z, 0.0);
  }
}
"#;

/// Maximum number of measurement gates whose outcomes can be captured in a
/// single recompute. Each placed `Measurement` occupies one slot in the GPU's
/// `measurement_aux_buffer` (a vec4 per slot — pZero, r, outcome, √p_kept).
pub(crate) const MAX_MEASUREMENT_SLOTS: usize = 64;

// MEASURE_REDUCE_SAMPLE — workgroup reduces pZero across the state vector,
// samples a deterministic [0, 1) value with a PCG-style hash seeded by the
// placed gate's id, and writes `(pZero, r, outcome, sqrt_p_kept)` into
// `aux_out[output_slot]`.  qni reference: `simulator.ts:measure`.
const MEASURE_REDUCE_SHADER: &str = r#"
struct MeasureReduceParams {
  qubit_bit: u32,
  state_count: u32,
  output_slot: u32,
  seed: u32,
};

@group(0) @binding(0) var<storage, read> state: array<vec2<f32>>;
@group(0) @binding(1) var<storage, read_write> aux_out: array<vec4<f32>>;
@group(0) @binding(2) var<uniform> params: MeasureReduceParams;

var<workgroup> shared_pzero: array<f32, 64>;

fn rand_unit(seed: u32) -> f32 {
  // xorshift-mix; matches the CPU mirror in `bloch::simulate`.
  var s: u32 = seed * 0x9E3779B9u + 0x85EBCA6Bu;
  s = s ^ (s >> 13u);
  s = s * 0xC2B2AE35u;
  s = s ^ (s >> 16u);
  return f32(s) / 4294967295.0;
}

@compute @workgroup_size(64)
fn main(@builtin(local_invocation_id) lid: vec3<u32>) {
  let tid: u32 = lid.x;
  let qubit_mask: u32 = 1u << params.qubit_bit;
  let total: u32 = params.state_count;

  var p_zero_partial: f32 = 0.0;
  var i: u32 = tid;
  loop {
    if (i >= total) { break; }
    if ((i & qubit_mask) == 0u) {
      let amp = state[i];
      p_zero_partial = p_zero_partial + (amp.x * amp.x + amp.y * amp.y);
    }
    i = i + 64u;
  }
  shared_pzero[tid] = p_zero_partial;
  workgroupBarrier();

  for (var step: u32 = 32u; step > 0u; step = step >> 1u) {
    if (tid < step) {
      shared_pzero[tid] = shared_pzero[tid] + shared_pzero[tid + step];
    }
    workgroupBarrier();
  }

  if (tid == 0u) {
    let p_zero: f32 = shared_pzero[0];
    let r: f32 = rand_unit(params.seed);
    var outcome: f32 = 1.0;
    var sqrt_p_kept: f32 = sqrt(max(1.0 - p_zero, 1.0e-30));
    if (r <= p_zero) {
      outcome = 0.0;
      sqrt_p_kept = sqrt(max(p_zero, 1.0e-30));
    }
    aux_out[params.output_slot] = vec4<f32>(p_zero, r, outcome, sqrt_p_kept);
  }
}
"#;

// MEASURE_COLLAPSE — per-pair shader that reads the previously-sampled
// outcome + sqrt_p_kept from the aux buffer, zeroes the unobserved branch,
// and renormalizes the surviving amplitudes.
const MEASURE_COLLAPSE_SHADER: &str = r#"
struct MeasureCollapseParams {
  qubit_bit: u32,
  state_count: u32,
  aux_slot: u32,
  _pad: u32,
};

@group(0) @binding(0) var<storage, read> state_in: array<vec2<f32>>;
@group(0) @binding(1) var<storage, read_write> state_out: array<vec2<f32>>;
@group(0) @binding(2) var<storage, read> aux: array<vec4<f32>>;
@group(0) @binding(3) var<uniform> params: MeasureCollapseParams;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let pair: u32 = gid.x;
  let total_pairs: u32 = params.state_count / 2u;
  if (pair >= total_pairs) { return; }
  let bit: u32 = params.qubit_bit;
  let mask: u32 = (1u << bit) - 1u;
  let low: u32 = pair & mask;
  let high: u32 = pair >> bit;
  let i0: u32 = (high << (bit + 1u)) | low;
  let i1: u32 = i0 | (1u << bit);

  let aux_v = aux[params.aux_slot];
  let outcome: f32 = aux_v.z;
  let inv_norm: f32 = 1.0 / aux_v.w;
  let a0 = state_in[i0];
  let a1 = state_in[i1];
  if (outcome < 0.5) {
    state_out[i0] = a0 * inv_norm;
    state_out[i1] = vec2<f32>(0.0, 0.0);
  } else {
    state_out[i0] = vec2<f32>(0.0, 0.0);
    state_out[i1] = a1 * inv_norm;
  }
}
"#;

// BLOCH_OVERLAY_RENDER_SHADER renders the dynamic arrow + tip dot of every
// placed Bloch display directly from `bloch_output_buffer`. Static decoration
// (sphere bg, axis lines, equator/meridian ellipses) is still painted by
// egui — it doesn't depend on quantum state — but the part that does depend
// stays on the GPU end-to-end (no CPU readback). Projection mirrors
// `icons::bloch_project` (qni's `rotateY(phi) rotateX(-theta)` axis swap +
// `perspective: 4rem` pinhole at top-right).
const BLOCH_OVERLAY_SHADER: &str = r#"
struct OverlayParams {
  screen_size: vec2<f32>,
  _pad0: vec2<f32>,
  line_color: vec4<f32>,
  tip_color: vec4<f32>,
  zero_color: vec4<f32>,
};

@group(0) @binding(0) var<storage, read> bloch_data: array<vec4<f32>>;
@group(0) @binding(1) var<uniform> params: OverlayParams;

struct VsIn {
  @location(0) corner: vec2<f32>,
  @location(1) center: vec2<f32>,
  @location(2) radius: f32,
  @location(3) outer: f32,
  @location(4) slot: u32,
};

struct VsOut {
  @builtin(position) clip: vec4<f32>,
  @location(0) local: vec2<f32>,
  @location(1) radius: f32,
  @location(2) outer: f32,
  @location(3) @interpolate(flat) slot: u32,
};

@vertex
fn vs_main(input: VsIn) -> VsOut {
  let local = input.corner * input.outer;
  let world = input.center + local;
  let ndc = vec2<f32>(
    (world.x / params.screen_size.x) * 2.0 - 1.0,
    1.0 - (world.y / params.screen_size.y) * 2.0,
  );
  var out: VsOut;
  out.clip = vec4<f32>(ndc, 0.0, 1.0);
  out.local = local;
  out.radius = input.radius;
  out.outer = input.outer;
  out.slot = input.slot;
  return out;
}

fn bloch_project(b: vec3<f32>) -> vec2<f32> {
  let p = 4.0;
  let px = 1.0;
  let py = -1.0;
  let x_3d: f32 = b.y;
  let y_3d: f32 = -b.z;
  let z_3d: f32 = b.x;
  let factor: f32 = p / (p - z_3d);
  let sx: f32 = px + factor * (x_3d - px);
  let sy: f32 = py + factor * (y_3d - py);
  return vec2<f32>(sx, sy);
}

fn line_distance(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
  let pa = p - a;
  let ba = b - a;
  let denom = max(dot(ba, ba), 1.0e-12);
  let t = clamp(dot(pa, ba) / denom, 0.0, 1.0);
  return length(pa - ba * t);
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
  let bloch = bloch_data[input.slot].xyz;
  let mag2 = dot(bloch, bloch);
  let mag = sqrt(mag2);
  let proj = bloch_project(bloch);
  let tip = proj * input.radius;

  let line_half: f32 = 0.75;
  let tip_radius: f32 = 3.0;
  let edge: f32 = 0.75;

  let dist_line = line_distance(input.local, vec2<f32>(0.0, 0.0), tip);
  let dist_tip = length(input.local - tip);

  var color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
  if (mag > 1.0e-3) {
    let line_alpha = 1.0 - smoothstep(line_half - edge, line_half + edge, dist_line);
    let line_rgb = params.line_color.rgb;
    color = vec4<f32>(line_rgb * line_alpha, line_alpha);
  }
  let tip_rgb = select(params.tip_color.rgb, params.zero_color.rgb, mag < 1.0e-3);
  let tip_alpha = 1.0 - smoothstep(tip_radius - edge, tip_radius + edge, dist_tip);
  color = vec4<f32>(
    color.rgb * (1.0 - tip_alpha) + tip_rgb * tip_alpha,
    color.a * (1.0 - tip_alpha) + tip_alpha,
  );

  if (color.a < 1.0e-3) {
    discard;
  }
  return color;
}
"#;

// MEASUREMENT_DIGIT_SHADER renders the `0` / `1` digit of every placed
// measurement directly from `measurement_aux_buffer`. The aux layout is
// `(pZero, r, outcome, sqrt_p_kept)` per slot; we sample `.z` to pick the
// glyph and the colour. Procedural SDFs: `0` is a stroked circle, `1` is a
// vertical bar — close enough to qni's `font-mono text-lg` look without
// needing a glyph atlas.
const MEASUREMENT_DIGIT_SHADER: &str = r#"
struct DigitParams {
  screen_size: vec2<f32>,
  _pad0: vec2<f32>,
  zero_color: vec4<f32>,
  one_color: vec4<f32>,
};

@group(0) @binding(0) var<storage, read> aux_data: array<vec4<f32>>;
@group(0) @binding(1) var<uniform> params: DigitParams;

struct VsIn {
  @location(0) corner: vec2<f32>,
  @location(1) center: vec2<f32>,
  @location(2) half_extent: f32,
  @location(3) slot: u32,
};

struct VsOut {
  @builtin(position) clip: vec4<f32>,
  @location(0) local: vec2<f32>,
  @location(1) half_extent: f32,
  @location(2) @interpolate(flat) slot: u32,
};

@vertex
fn vs_main(input: VsIn) -> VsOut {
  let local = input.corner * input.half_extent;
  let world = input.center + local;
  let ndc = vec2<f32>(
    (world.x / params.screen_size.x) * 2.0 - 1.0,
    1.0 - (world.y / params.screen_size.y) * 2.0,
  );
  var out: VsOut;
  out.clip = vec4<f32>(ndc, 0.0, 1.0);
  out.local = local;
  out.half_extent = input.half_extent;
  out.slot = input.slot;
  return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
  let aux = aux_data[input.slot];
  let outcome: f32 = aux.z;

  // Glyph metrics — tuned to roughly match an 18pt monospace digit on a
  // 32px gate.  Edge softness uses fwidth-style screen-space derivatives so
  // antialiasing scales with DPR.
  let half_h: f32 = 7.0;
  let half_w: f32 = 5.0;
  let thickness: f32 = 2.0;
  let edge: f32 = 0.75;

  var alpha: f32 = 0.0;
  var color: vec3<f32>;
  if (outcome < 0.5) {
    // "0" — stroked ellipse-ish circle.
    let dist = length(input.local);
    let outer = half_h;
    let inner = half_h - thickness;
    let outer_alpha = 1.0 - smoothstep(outer - edge, outer + edge, dist);
    let inner_alpha = 1.0 - smoothstep(inner - edge, inner + edge, dist);
    alpha = max(0.0, outer_alpha - inner_alpha);
    color = params.zero_color.rgb;
  } else {
    // "1" — vertical bar with a tiny diagonal serif so it doesn't read as a
    // bare line.
    let bar_alpha_x = 1.0 - smoothstep(thickness * 0.5 - edge, thickness * 0.5 + edge, abs(input.local.x));
    let bar_alpha_y = 1.0 - smoothstep(half_h - edge, half_h + edge, abs(input.local.y));
    let bar = bar_alpha_x * bar_alpha_y;
    // Serif: from top-left toward the bar.
    let serif_dir = normalize(vec2<f32>(1.0, 1.0));
    let serif_p = input.local - vec2<f32>(-half_w * 0.6, -half_h * 0.7);
    let serif_along = dot(serif_p, serif_dir);
    let serif_perp = dot(serif_p, vec2<f32>(-serif_dir.y, serif_dir.x));
    let serif_alpha_along = 1.0 - smoothstep(half_w * 0.6, half_w * 0.6 + edge, abs(serif_along));
    let serif_alpha_perp = 1.0 - smoothstep(thickness * 0.4 - edge, thickness * 0.4 + edge, abs(serif_perp));
    let serif = serif_alpha_along * serif_alpha_perp;
    alpha = max(bar, serif);
    color = params.one_color.rgb;
  }

  if (alpha < 1.0e-3) {
    discard;
  }
  return vec4<f32>(color * alpha, alpha);
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

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct BlochParams {
    pub(crate) qubit_bit: u32,
    pub(crate) state_count: u32,
    pub(crate) output_slot: u32,
    pub(crate) _pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct MeasureReduceParams {
    pub(crate) qubit_bit: u32,
    pub(crate) state_count: u32,
    pub(crate) output_slot: u32,
    pub(crate) seed: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct MeasureCollapseParams {
    pub(crate) qubit_bit: u32,
    pub(crate) state_count: u32,
    pub(crate) aux_slot: u32,
    pub(crate) _pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct BlochOverlayParams {
    pub(crate) screen_size: [f32; 2],
    pub(crate) _pad0: [f32; 2],
    pub(crate) line_color: [f32; 4],
    pub(crate) tip_color: [f32; 4],
    pub(crate) zero_color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct BlochOverlayInstance {
    pub(crate) center: [f32; 2],
    pub(crate) radius: f32,
    pub(crate) outer: f32,
    pub(crate) slot: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct MeasurementDigitParams {
    pub(crate) screen_size: [f32; 2],
    pub(crate) _pad0: [f32; 2],
    pub(crate) zero_color: [f32; 4],
    pub(crate) one_color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct MeasurementDigitInstance {
    pub(crate) center: [f32; 2],
    pub(crate) half_extent: f32,
    pub(crate) slot: u32,
}

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
    /// Bloch reduction pipeline + buffers. Two bind groups so we can read from
    /// either ping-pong state buffer (whichever holds the current state at
    /// capture time). The output buffer is GPU-only — readback uses a fresh
    /// MAP_READ staging buffer per dispatch so we never re-issue commands
    /// against a buffer that is still mapped from an earlier readback.
    pub(crate) bloch_pipeline: wgpu::ComputePipeline,
    pub(crate) bloch_bind_groups: [wgpu::BindGroup; 2],
    pub(crate) bloch_params_buffer: wgpu::Buffer,
    pub(crate) bloch_output_buffer: wgpu::Buffer,
    /// Measurement reduce + sample shader and its bind groups (one per ping-
    /// pong state buffer). Writes `(pZero, r, outcome, sqrt_p_kept)` to
    /// `measurement_aux_buffer`.
    pub(crate) measure_reduce_pipeline: wgpu::ComputePipeline,
    pub(crate) measure_reduce_bind_groups: [wgpu::BindGroup; 2],
    pub(crate) measure_reduce_params_buffer: wgpu::Buffer,
    /// Measurement collapse shader. Four bind groups: two ping-pong
    /// directions × ?, actually two bind groups (state_in side selects which
    /// buffer to read; the other is the write target).
    pub(crate) measure_collapse_pipeline: wgpu::ComputePipeline,
    pub(crate) measure_collapse_bind_groups: [wgpu::BindGroup; 2],
    pub(crate) measure_collapse_params_buffer: wgpu::Buffer,
    pub(crate) measurement_aux_buffer: wgpu::Buffer,
    /// GPU render pass that draws the dynamic Bloch arrow + tip dot directly
    /// from `bloch_output_buffer`. No CPU readback in production.
    pub(crate) bloch_overlay_pipeline: wgpu::RenderPipeline,
    pub(crate) bloch_overlay_bind_group: wgpu::BindGroup,
    pub(crate) bloch_overlay_params_buffer: wgpu::Buffer,
    pub(crate) bloch_overlay_instance_buffer: wgpu::Buffer,
    pub(crate) bloch_overlay_vertex_buffer: wgpu::Buffer,
    pub(crate) bloch_overlay_index_buffer: wgpu::Buffer,
    /// Renders the 0/1 measurement digit straight from
    /// `measurement_aux_buffer`. Static meter icon is still painted by egui.
    pub(crate) measurement_digit_pipeline: wgpu::RenderPipeline,
    pub(crate) measurement_digit_bind_group: wgpu::BindGroup,
    pub(crate) measurement_digit_params_buffer: wgpu::Buffer,
    pub(crate) measurement_digit_instance_buffer: wgpu::Buffer,
}

impl StateVectorResources {
    pub(crate) fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("state_vector_compute"),
            source: wgpu::ShaderSource::Wgsl(STATE_COMPUTE_SHADER.into()),
        });
        let bloch_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bloch_reduce"),
            source: wgpu::ShaderSource::Wgsl(BLOCH_REDUCE_SHADER.into()),
        });
        let measure_reduce_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("measure_reduce"),
            source: wgpu::ShaderSource::Wgsl(MEASURE_REDUCE_SHADER.into()),
        });
        let measure_collapse_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("measure_collapse"),
            source: wgpu::ShaderSource::Wgsl(MEASURE_COLLAPSE_SHADER.into()),
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

        let bloch_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let bloch_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("bloch_reduce_pipeline_layout"),
            bind_group_layouts: &[&bloch_bind_group_layout],
            push_constant_ranges: &[],
        });
        let bloch_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("bloch_reduce_pipeline"),
            layout: Some(&bloch_pipeline_layout),
            module: &bloch_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // Measurement reduce/sample shares the bloch bind-group layout shape
        // (state in, aux out, params uniform).
        let measure_reduce_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        let measure_reduce_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("measure_reduce_pipeline_layout"),
                bind_group_layouts: &[&measure_reduce_bind_group_layout],
                push_constant_ranges: &[],
            });
        let measure_reduce_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("measure_reduce_pipeline"),
                layout: Some(&measure_reduce_pipeline_layout),
                module: &measure_reduce_shader,
                entry_point: Some("main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            });

        // Measurement collapse needs a 4-binding layout: state_in (read),
        // state_out (read_write), aux (read), params (uniform).
        let measure_collapse_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        let measure_collapse_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("measure_collapse_pipeline_layout"),
                bind_group_layouts: &[&measure_collapse_bind_group_layout],
                push_constant_ranges: &[],
            });
        let measure_collapse_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("measure_collapse_pipeline"),
                layout: Some(&measure_collapse_pipeline_layout),
                module: &measure_collapse_shader,
                entry_point: Some("main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            });

        let bloch_overlay_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bloch_overlay"),
            source: wgpu::ShaderSource::Wgsl(BLOCH_OVERLAY_SHADER.into()),
        });
        let measurement_digit_shader =
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("measurement_digit"),
                source: wgpu::ShaderSource::Wgsl(MEASUREMENT_DIGIT_SHADER.into()),
            });
        let bloch_overlay_bind_group_layout =
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
        let measurement_digit_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                ],
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

        let bloch_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bloch_reduce_params"),
            size: std::mem::size_of::<BlochParams>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bloch_buffer_size = (MAX_BLOCH_SLOTS * 4 * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
        let bloch_output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bloch_output"),
            size: bloch_buffer_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let measure_reduce_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("measure_reduce_params"),
            size: std::mem::size_of::<MeasureReduceParams>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let measure_collapse_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("measure_collapse_params"),
            size: std::mem::size_of::<MeasureCollapseParams>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let measurement_aux_size =
            (MAX_MEASUREMENT_SLOTS * 4 * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
        let measurement_aux_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("measurement_aux"),
            size: measurement_aux_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let bloch_overlay_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bloch_overlay_params"),
            size: std::mem::size_of::<BlochOverlayParams>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bloch_overlay_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bloch_overlay_instances"),
            size: (MAX_BLOCH_SLOTS * std::mem::size_of::<BlochOverlayInstance>())
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bloch_overlay_vertex_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("bloch_overlay_quad_vertices"),
                contents: bytemuck::cast_slice(&[
                    [-1.0f32, -1.0],
                    [1.0, -1.0],
                    [1.0, 1.0],
                    [-1.0, 1.0],
                ]),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let bloch_overlay_index_data: [u16; 6] = [0, 1, 2, 0, 2, 3];
        let bloch_overlay_index_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("bloch_overlay_quad_indices"),
                contents: bytemuck::cast_slice(&bloch_overlay_index_data),
                usage: wgpu::BufferUsages::INDEX,
            });

        let measurement_digit_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("measurement_digit_params"),
            size: std::mem::size_of::<MeasurementDigitParams>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let measurement_digit_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("measurement_digit_instances"),
            size: (MAX_MEASUREMENT_SLOTS * std::mem::size_of::<MeasurementDigitInstance>())
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
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

        let bloch_bind_groups = [
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("bloch_reduce_read_a"),
                layout: &bloch_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: state_buffers[0].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: bloch_output_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: bloch_params_buffer.as_entire_binding(),
                    },
                ],
            }),
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("bloch_reduce_read_b"),
                layout: &bloch_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: state_buffers[1].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: bloch_output_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: bloch_params_buffer.as_entire_binding(),
                    },
                ],
            }),
        ];

        let measure_reduce_bind_groups = [
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("measure_reduce_read_a"),
                layout: &measure_reduce_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: state_buffers[0].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: measurement_aux_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: measure_reduce_params_buffer.as_entire_binding(),
                    },
                ],
            }),
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("measure_reduce_read_b"),
                layout: &measure_reduce_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: state_buffers[1].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: measurement_aux_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: measure_reduce_params_buffer.as_entire_binding(),
                    },
                ],
            }),
        ];

        let measure_collapse_bind_groups = [
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("measure_collapse_a_to_b"),
                layout: &measure_collapse_bind_group_layout,
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
                        resource: measurement_aux_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: measure_collapse_params_buffer.as_entire_binding(),
                    },
                ],
            }),
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("measure_collapse_b_to_a"),
                layout: &measure_collapse_bind_group_layout,
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
                        resource: measurement_aux_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: measure_collapse_params_buffer.as_entire_binding(),
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

        let bloch_overlay_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("bloch_overlay_bind_group"),
                layout: &bloch_overlay_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: bloch_output_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: bloch_overlay_params_buffer.as_entire_binding(),
                    },
                ],
            });
        let bloch_overlay_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("bloch_overlay_pipeline_layout"),
                bind_group_layouts: &[&bloch_overlay_bind_group_layout],
                push_constant_ranges: &[],
            });
        let bloch_overlay_vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            }],
        };
        let bloch_overlay_instance_layout = wgpu::VertexBufferLayout {
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
        let bloch_overlay_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("bloch_overlay_pipeline"),
                layout: Some(&bloch_overlay_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &bloch_overlay_shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[bloch_overlay_vertex_layout, bloch_overlay_instance_layout],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &bloch_overlay_shader,
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

        let measurement_digit_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("measurement_digit_bind_group"),
                layout: &measurement_digit_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: measurement_aux_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: measurement_digit_params_buffer.as_entire_binding(),
                    },
                ],
            });
        let measurement_digit_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("measurement_digit_pipeline_layout"),
                bind_group_layouts: &[&measurement_digit_bind_group_layout],
                push_constant_ranges: &[],
            });
        let measurement_digit_vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            }],
        };
        let measurement_digit_instance_layout = wgpu::VertexBufferLayout {
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
        let measurement_digit_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("measurement_digit_pipeline"),
                layout: Some(&measurement_digit_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &measurement_digit_shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[
                        measurement_digit_vertex_layout,
                        measurement_digit_instance_layout,
                    ],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &measurement_digit_shader,
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
            bloch_pipeline,
            bloch_bind_groups,
            bloch_params_buffer,
            bloch_output_buffer,
            measure_reduce_pipeline,
            measure_reduce_bind_groups,
            measure_reduce_params_buffer,
            measure_collapse_pipeline,
            measure_collapse_bind_groups,
            measure_collapse_params_buffer,
            measurement_aux_buffer,
            bloch_overlay_pipeline,
            bloch_overlay_bind_group,
            bloch_overlay_params_buffer,
            bloch_overlay_instance_buffer,
            bloch_overlay_vertex_buffer,
            bloch_overlay_index_buffer,
            measurement_digit_pipeline,
            measurement_digit_bind_group,
            measurement_digit_params_buffer,
            measurement_digit_instance_buffer,
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

/// Renders the dynamic Bloch arrow + tip dot for every placed Bloch display
/// directly from `bloch_output_buffer`. No CPU readback in production —
/// `BlochOverlayInstance` carries (screen center, radius, output_slot) and
/// the fragment shader reads (x, y, z) straight from the GPU buffer the
/// reduction shader just wrote.
pub(crate) struct BlochOverlayCallback {
    pub(crate) instances: Arc<[BlochOverlayInstance]>,
    pub(crate) line_color: [f32; 4],
    pub(crate) tip_color: [f32; 4],
    pub(crate) zero_color: [f32; 4],
}

impl egui_wgpu::CallbackTrait for BlochOverlayCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(resources) = callback_resources.get_mut::<StateVectorResources>() else {
            return Vec::new();
        };
        if self.instances.is_empty() {
            return Vec::new();
        }
        let screen_size = [
            screen_descriptor.size_in_pixels[0] as f32 / screen_descriptor.pixels_per_point,
            screen_descriptor.size_in_pixels[1] as f32 / screen_descriptor.pixels_per_point,
        ];
        let params = BlochOverlayParams {
            screen_size,
            _pad0: [0.0, 0.0],
            line_color: self.line_color,
            tip_color: self.tip_color,
            zero_color: self.zero_color,
        };
        queue.write_buffer(
            &resources.bloch_overlay_params_buffer,
            0,
            bytemuck::bytes_of(&params),
        );
        queue.write_buffer(
            &resources.bloch_overlay_instance_buffer,
            0,
            bytemuck::cast_slice(self.instances.as_ref()),
        );
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
        render_pass.set_pipeline(&resources.bloch_overlay_pipeline);
        render_pass.set_bind_group(0, &resources.bloch_overlay_bind_group, &[]);
        render_pass.set_vertex_buffer(0, resources.bloch_overlay_vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, resources.bloch_overlay_instance_buffer.slice(..));
        render_pass.set_index_buffer(
            resources.bloch_overlay_index_buffer.slice(..),
            wgpu::IndexFormat::Uint16,
        );
        render_pass.draw_indexed(0..6, 0, 0..self.instances.len() as u32);
    }
}

/// Renders the 0/1 measurement digit straight from
/// `measurement_aux_buffer.z` (the GPU-sampled outcome). Static meter icon
/// (purple or zinc-200 ring) is still painted by egui — only the digit is
/// quantum-state-derived.
pub(crate) struct MeasurementDigitCallback {
    pub(crate) instances: Arc<[MeasurementDigitInstance]>,
    pub(crate) zero_color: [f32; 4],
    pub(crate) one_color: [f32; 4],
}

impl egui_wgpu::CallbackTrait for MeasurementDigitCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(resources) = callback_resources.get_mut::<StateVectorResources>() else {
            return Vec::new();
        };
        if self.instances.is_empty() {
            return Vec::new();
        }
        let screen_size = [
            screen_descriptor.size_in_pixels[0] as f32 / screen_descriptor.pixels_per_point,
            screen_descriptor.size_in_pixels[1] as f32 / screen_descriptor.pixels_per_point,
        ];
        let params = MeasurementDigitParams {
            screen_size,
            _pad0: [0.0, 0.0],
            zero_color: self.zero_color,
            one_color: self.one_color,
        };
        queue.write_buffer(
            &resources.measurement_digit_params_buffer,
            0,
            bytemuck::bytes_of(&params),
        );
        queue.write_buffer(
            &resources.measurement_digit_instance_buffer,
            0,
            bytemuck::cast_slice(self.instances.as_ref()),
        );
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
        render_pass.set_pipeline(&resources.measurement_digit_pipeline);
        render_pass.set_bind_group(0, &resources.measurement_digit_bind_group, &[]);
        // Reuse the bloch overlay's quad geometry — both render full-rect
        // quads with `[-1..1]` corners.
        render_pass.set_vertex_buffer(0, resources.bloch_overlay_vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, resources.measurement_digit_instance_buffer.slice(..));
        render_pass.set_index_buffer(
            resources.bloch_overlay_index_buffer.slice(..),
            wgpu::IndexFormat::Uint16,
        );
        render_pass.draw_indexed(0..6, 0, 0..self.instances.len() as u32);
    }
}

pub(crate) struct StateVectorCallback {
    pub(crate) instances: Arc<[StateInstance]>,
    pub(crate) instances_dirty: bool,
    /// Linearised simulation ops for the GPU pipeline. Includes all four
    /// op kinds: `ApplyGate`, `CaptureBloch`, `MeasureReduceSample`, and
    /// `MeasureCollapse`. The GPU dispatches them in order; ping-pong of
    /// the state buffers happens for any op that mutates state (gates and
    /// `MeasureCollapse`).
    pub(crate) sim_ops: Vec<SimulationOp>,
    pub(crate) state_count: usize,
    pub(crate) recompute: bool,
    pub(crate) target_format: wgpu::TextureFormat,
    pub(crate) colors: RenderColors,
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
                // Initialize to |0...0⟩ then dispatch each op on the GPU.
                let mut initial = vec![[0.0f32, 0.0f32]; self.state_count];
                initial[0] = [1.0, 0.0];
                queue.write_buffer(
                    &resources.state_buffers[0],
                    0,
                    bytemuck::cast_slice(&initial),
                );
                resources.active_state = 0;
                let pair_count = (self.state_count / 2) as u32;
                let dispatch_x = pair_count.div_ceil(STATE_WORKGROUP_SIZE);
                let mut in_index = 0usize;
                let mut bloch_capture_count: u32 = 0;
                let mut bloch_slot_to_gate_id: Vec<u32> = Vec::new();
                let mut measurement_count: u32 = 0;
                let mut measurement_slot_to_gate_id: Vec<u32> = Vec::new();
                for op in &self.sim_ops {
                    match op {
                        SimulationOp::ApplyGate(params) => {
                            if pair_count == 0 {
                                continue;
                            }
                            queue.write_buffer(
                                &resources.gate_params_buffer,
                                0,
                                bytemuck::bytes_of(params),
                            );
                            let mut encoder = device.create_command_encoder(
                                &wgpu::CommandEncoderDescriptor {
                                    label: Some("state_vector_compute_encoder"),
                                },
                            );
                            {
                                let mut pass = encoder.begin_compute_pass(
                                    &wgpu::ComputePassDescriptor {
                                        label: Some("state_vector_compute_pass"),
                                        timestamp_writes: None,
                                    },
                                );
                                pass.set_pipeline(&resources.compute_pipeline);
                                pass.set_bind_group(
                                    0,
                                    &resources.compute_bind_groups[in_index],
                                    &[],
                                );
                                pass.dispatch_workgroups(dispatch_x, 1, 1);
                            }
                            queue.submit(Some(encoder.finish()));
                            in_index = 1 - in_index;
                        }
                        SimulationOp::CaptureBloch {
                            gate_id,
                            qubit_bit,
                            output_slot,
                        } => {
                            if (*output_slot as usize) >= MAX_BLOCH_SLOTS {
                                continue;
                            }
                            let bloch_params = BlochParams {
                                qubit_bit: *qubit_bit,
                                state_count: self.state_count as u32,
                                output_slot: *output_slot,
                                _pad: 0,
                            };
                            queue.write_buffer(
                                &resources.bloch_params_buffer,
                                0,
                                bytemuck::bytes_of(&bloch_params),
                            );
                            let mut encoder = device.create_command_encoder(
                                &wgpu::CommandEncoderDescriptor {
                                    label: Some("bloch_reduce_encoder"),
                                },
                            );
                            {
                                let mut pass = encoder.begin_compute_pass(
                                    &wgpu::ComputePassDescriptor {
                                        label: Some("bloch_reduce_pass"),
                                        timestamp_writes: None,
                                    },
                                );
                                pass.set_pipeline(&resources.bloch_pipeline);
                                // The current state lives in `state_buffers[in_index]`,
                                // which is the read side of the next gate dispatch.
                                pass.set_bind_group(
                                    0,
                                    &resources.bloch_bind_groups[in_index],
                                    &[],
                                );
                                pass.dispatch_workgroups(1, 1, 1);
                            }
                            queue.submit(Some(encoder.finish()));
                            bloch_slot_to_gate_id.push(*gate_id);
                            bloch_capture_count += 1;
                        }
                        SimulationOp::MeasureReduceSample {
                            gate_id,
                            qubit_bit,
                            output_slot,
                        } => {
                            if (*output_slot as usize) >= MAX_MEASUREMENT_SLOTS {
                                continue;
                            }
                            let measure_params = MeasureReduceParams {
                                qubit_bit: *qubit_bit,
                                state_count: self.state_count as u32,
                                output_slot: *output_slot,
                                seed: *gate_id,
                            };
                            queue.write_buffer(
                                &resources.measure_reduce_params_buffer,
                                0,
                                bytemuck::bytes_of(&measure_params),
                            );
                            let mut encoder = device.create_command_encoder(
                                &wgpu::CommandEncoderDescriptor {
                                    label: Some("measure_reduce_encoder"),
                                },
                            );
                            {
                                let mut pass = encoder.begin_compute_pass(
                                    &wgpu::ComputePassDescriptor {
                                        label: Some("measure_reduce_pass"),
                                        timestamp_writes: None,
                                    },
                                );
                                pass.set_pipeline(&resources.measure_reduce_pipeline);
                                pass.set_bind_group(
                                    0,
                                    &resources.measure_reduce_bind_groups[in_index],
                                    &[],
                                );
                                pass.dispatch_workgroups(1, 1, 1);
                            }
                            queue.submit(Some(encoder.finish()));
                            measurement_slot_to_gate_id.push(*gate_id);
                            measurement_count += 1;
                        }
                        SimulationOp::MeasureCollapse {
                            qubit_bit,
                            aux_slot,
                        } => {
                            if pair_count == 0 {
                                continue;
                            }
                            let collapse_params = MeasureCollapseParams {
                                qubit_bit: *qubit_bit,
                                state_count: self.state_count as u32,
                                aux_slot: *aux_slot,
                                _pad: 0,
                            };
                            queue.write_buffer(
                                &resources.measure_collapse_params_buffer,
                                0,
                                bytemuck::bytes_of(&collapse_params),
                            );
                            let mut encoder = device.create_command_encoder(
                                &wgpu::CommandEncoderDescriptor {
                                    label: Some("measure_collapse_encoder"),
                                },
                            );
                            {
                                let mut pass = encoder.begin_compute_pass(
                                    &wgpu::ComputePassDescriptor {
                                        label: Some("measure_collapse_pass"),
                                        timestamp_writes: None,
                                    },
                                );
                                pass.set_pipeline(&resources.measure_collapse_pipeline);
                                pass.set_bind_group(
                                    0,
                                    &resources.measure_collapse_bind_groups[in_index],
                                    &[],
                                );
                                pass.dispatch_workgroups(dispatch_x, 1, 1);
                            }
                            queue.submit(Some(encoder.finish()));
                            in_index = 1 - in_index;
                        }
                    }
                }
                resources.active_state = in_index;

                // Production path never reads back. The slot mappings are
                // stashed in thread-locals so the test-only on-demand
                // readback APIs (`read_bloch_vectors_impl` /
                // `read_measurement_outcomes_impl`) can copy + map the
                // GPU buffers when JS asks for them.
                BLOCH_SLOT_MAP.with(|cell| {
                    *cell.borrow_mut() = bloch_slot_to_gate_id;
                });
                MEASUREMENT_SLOT_MAP.with(|cell| {
                    *cell.borrow_mut() = measurement_slot_to_gate_id;
                });
                let _ = bloch_capture_count;
                let _ = measurement_count;
            } else {
                resources.active_state = 0;
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
        BLOCH_GPU_HANDLE.with(|slot| {
            *slot.borrow_mut() = Some(BlochGpuHandle {
                device: device.clone(),
                queue: queue.clone(),
                output_buffer: resources.bloch_output_buffer.clone(),
            });
        });
        MEASUREMENT_GPU_HANDLE.with(|slot| {
            *slot.borrow_mut() = Some(MeasurementGpuHandle {
                device: device.clone(),
                queue: queue.clone(),
                aux_buffer: resources.measurement_aux_buffer.clone(),
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

#[derive(Clone)]
pub(crate) struct GpuReadbackState {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) state_buffers: [wgpu::Buffer; 2],
    pub(crate) state_count: usize,
    pub(crate) active_state: usize,
}

#[derive(Clone)]
pub(crate) struct BlochGpuHandle {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) output_buffer: wgpu::Buffer,
}

#[derive(Clone)]
pub(crate) struct MeasurementGpuHandle {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) aux_buffer: wgpu::Buffer,
}

thread_local! {
    pub(crate) static GPU_READBACK: RefCell<Option<GpuReadbackState>> = const { RefCell::new(None) };
    /// Latest GPU buffer + queue handle for the bloch overlay output. Set in
    /// `prepare()`; consumed by the test-only async API
    /// `read_bloch_vectors_impl`. No production code touches it — production
    /// rendering reads `bloch_output_buffer` directly inside the GPU shader.
    pub(crate) static BLOCH_GPU_HANDLE: RefCell<Option<BlochGpuHandle>> =
        const { RefCell::new(None) };
    /// gate_id list ordered by output_slot. Parallel to the contents of
    /// `bloch_output_buffer`; the test API joins this with the read-back
    /// floats to produce `[gate_id, x, y, z, …]`.
    pub(crate) static BLOCH_SLOT_MAP: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    /// Same as `BLOCH_GPU_HANDLE` for the measurement aux buffer.
    pub(crate) static MEASUREMENT_GPU_HANDLE: RefCell<Option<MeasurementGpuHandle>> =
        const { RefCell::new(None) };
    pub(crate) static MEASUREMENT_SLOT_MAP: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn read_state_vector_impl() -> Result<js_sys::Float32Array, JsValue> {
    let Some(state) = GPU_READBACK.with(|slot| slot.borrow().clone()) else {
        return Err(JsValue::from_str("state vector not ready"));
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
        .map_err(|_| JsValue::from_str("readback dropped"))?
        .map_err(|err| JsValue::from_str(&format!("map_async failed: {err:?}")))?;
    let data = slice.get_mapped_range();
    let floats: &[f32] = bytemuck::cast_slice(&data);
    let output = js_sys::Float32Array::new_with_length(floats.len() as u32);
    output.copy_from(floats);
    drop(data);
    staging.unmap();
    Ok(output)
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn read_bloch_vectors_impl() -> Result<js_sys::Float32Array, JsValue> {
    let Some(handle) = BLOCH_GPU_HANDLE.with(|slot| slot.borrow().clone()) else {
        return Ok(js_sys::Float32Array::new_with_length(0));
    };
    let slot_map = BLOCH_SLOT_MAP.with(|cell| cell.borrow().clone());
    if slot_map.is_empty() {
        return Ok(js_sys::Float32Array::new_with_length(0));
    }
    let copy_bytes = slot_map.len() * 4 * std::mem::size_of::<f32>();
    let staging = handle.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("bloch_readback"),
        size: copy_bytes as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = handle
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("bloch_readback_encoder"),
        });
    encoder.copy_buffer_to_buffer(
        &handle.output_buffer,
        0,
        &staging,
        0,
        copy_bytes as wgpu::BufferAddress,
    );
    handle.queue.submit(Some(encoder.finish()));

    let slice = staging.slice(..);
    let (sender, receiver) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    receiver
        .await
        .map_err(|_| JsValue::from_str("readback dropped"))?
        .map_err(|err| JsValue::from_str(&format!("map_async failed: {err:?}")))?;
    let data = slice.get_mapped_range();
    let floats: &[f32] = bytemuck::cast_slice(&data);
    let output = js_sys::Float32Array::new_with_length((slot_map.len() * 4) as u32);
    for (slot, gate_id) in slot_map.iter().enumerate() {
        let base = slot * 4;
        if base + 2 >= floats.len() {
            break;
        }
        output.set_index((slot * 4) as u32, *gate_id as f32);
        output.set_index((slot * 4 + 1) as u32, floats[base]);
        output.set_index((slot * 4 + 2) as u32, floats[base + 1]);
        output.set_index((slot * 4 + 3) as u32, floats[base + 2]);
    }
    drop(data);
    staging.unmap();
    Ok(output)
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn read_measurement_outcomes_impl() -> Result<js_sys::Float32Array, JsValue> {
    let Some(handle) = MEASUREMENT_GPU_HANDLE.with(|slot| slot.borrow().clone()) else {
        return Ok(js_sys::Float32Array::new_with_length(0));
    };
    let slot_map = MEASUREMENT_SLOT_MAP.with(|cell| cell.borrow().clone());
    if slot_map.is_empty() {
        return Ok(js_sys::Float32Array::new_with_length(0));
    }
    let copy_bytes = slot_map.len() * 4 * std::mem::size_of::<f32>();
    let staging = handle.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("measurement_readback"),
        size: copy_bytes as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = handle
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("measurement_readback_encoder"),
        });
    encoder.copy_buffer_to_buffer(
        &handle.aux_buffer,
        0,
        &staging,
        0,
        copy_bytes as wgpu::BufferAddress,
    );
    handle.queue.submit(Some(encoder.finish()));

    let slice = staging.slice(..);
    let (sender, receiver) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    receiver
        .await
        .map_err(|_| JsValue::from_str("readback dropped"))?
        .map_err(|err| JsValue::from_str(&format!("map_async failed: {err:?}")))?;
    let data = slice.get_mapped_range();
    let floats: &[f32] = bytemuck::cast_slice(&data);
    let output = js_sys::Float32Array::new_with_length((slot_map.len() * 2) as u32);
    for (slot, gate_id) in slot_map.iter().enumerate() {
        // aux layout (.x, .y, .z, .w) = (pZero, r, outcome, sqrt_p_kept).
        let outcome_idx = slot * 4 + 2;
        if outcome_idx >= floats.len() {
            break;
        }
        output.set_index((slot * 2) as u32, *gate_id as f32);
        output.set_index((slot * 2 + 1) as u32, floats[outcome_idx]);
    }
    drop(data);
    staging.unmap();
    Ok(output)
}
