//! WGSL shader source strings.
//!
//! All shaders the GPU module hands to wgpu live here as `&'static str`
//! constants. They are referenced by `StateVectorResources::new` (compute
//! and render pipelines) and by the three `CallbackTrait` impls in
//! `callbacks.rs`. Pure data — no Rust imports, no types.

pub(super) const STATE_COMPUTE_SHADER: &str = r#"
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
pub(super) const BLOCH_REDUCE_SHADER: &str = r#"
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
pub(super) const MEASURE_REDUCE_SHADER: &str = r#"
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
pub(super) const MEASURE_COLLAPSE_SHADER: &str = r#"
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
pub(super) const BLOCH_OVERLAY_SHADER: &str = r#"
struct OverlayParams {
  viewport_min: vec2<f32>,
  viewport_size: vec2<f32>,
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
  // egui_wgpu sets the GL viewport to the rect we passed to
  // `Callback::new_paint_callback`, so NDC -1..1 maps to that rect (not the
  // full canvas). World coords already include `rect.min`, so subtract it.
  let viewport_pos = world - params.viewport_min;
  let ndc = vec2<f32>(
    (viewport_pos.x / params.viewport_size.x) * 2.0 - 1.0,
    1.0 - (viewport_pos.y / params.viewport_size.y) * 2.0,
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
pub(super) const MEASUREMENT_DIGIT_SHADER: &str = r#"
struct DigitParams {
  viewport_min: vec2<f32>,
  viewport_size: vec2<f32>,
  zero_color: vec4<f32>,
  one_color: vec4<f32>,
};

@group(0) @binding(0) var<storage, read> aux_data: array<vec4<f32>>;
@group(0) @binding(1) var<uniform> params: DigitParams;
@group(0) @binding(2) var digit_atlas: texture_2d<f32>;
@group(0) @binding(3) var digit_sampler: sampler;

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
  // See BLOCH_OVERLAY_SHADER — NDC maps to the egui callback viewport.
  let viewport_pos = world - params.viewport_min;
  let ndc = vec2<f32>(
    (viewport_pos.x / params.viewport_size.x) * 2.0 - 1.0,
    1.0 - (viewport_pos.y / params.viewport_size.y) * 2.0,
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
  // Map local coords ([-half_extent, +half_extent]^2) into the right cell
  // of a 1x2 atlas (top half = "0", bottom half = "1"). UV.y picks the row.
  let cell_u = (input.local.x / input.half_extent) * 0.5 + 0.5;
  let cell_v = (input.local.y / input.half_extent) * 0.5 + 0.5;
  let row = select(0.0, 1.0, outcome >= 0.5);
  let uv = vec2<f32>(cell_u, (cell_v + row) * 0.5);
  let alpha = textureSample(digit_atlas, digit_sampler, uv).r;
  let color = select(params.zero_color.rgb, params.one_color.rgb, outcome >= 0.5);
  if (alpha < 1.0e-3) {
    discard;
  }
  return vec4<f32>(color * alpha, alpha);
}
"#;
pub(super) const STATE_RENDER_SHADER: &str = r#"
struct RenderParams {
  // Egui callback viewport in CSS pixels. NDC -1..1 maps to this rect, NOT
  // the full canvas — see BLOCH_OVERLAY_SHADER for the explanation.
  viewport_min: vec2<f32>,
  viewport_size: vec2<f32>,
  // Top-left of the state-circle grid (egui pixels).
  panel_origin: vec2<f32>,
  // Total grid extent (cols * cell_pitch, rows * cell_pitch).
  panel_size: vec2<f32>,
  cell_pitch: f32,
  radius: f32,
  inner_radius: f32,
  stroke: f32,
  cols: u32,
  rows: u32,
  qubits: u32,
  _pad: u32,
  surface: vec4<f32>,
  fill: vec4<f32>,
  outline: vec4<f32>,
  outline_zero: vec4<f32>,
  needle: vec4<f32>,
};

@group(0) @binding(0) var<storage, read> state: array<vec2<f32>>;
@group(0) @binding(1) var<uniform> params: RenderParams;

struct VsIn {
  @location(0) position: vec2<f32>,  // unit quad in -1..1
};

struct VsOut {
  @builtin(position) clip: vec4<f32>,
  @location(0) panel_local: vec2<f32>,  // 0..panel_size in egui pixels
};

// Single quad covering the entire grid. The fragment shader figures out
// which (col, row) cell each pixel belongs to and draws the corresponding
// state circle. This replaces an N-instance instanced draw (one quad per
// cell) — N == 2^qubits, up to 65 536 — and avoids the per-instance
// dispatch overhead that dominates frame time on contended GPUs.
@vertex
fn vs_main(input: VsIn) -> VsOut {
  // Expand the quad outward by `pad` so the edge cells' stroke + 1 px AA
  // fringe (which sits half_stroke past the cell box, plus an fwidth-worth
  // of edge fade) has somewhere to render. Without this the top / left
  // pixels of the (col=0, row=0) cell get clipped at panel_origin.
  let pad = params.stroke * 0.5 + 1.0;
  let pad_v = vec2<f32>(pad);
  let panel_local =
    (input.position * 0.5 + 0.5) * (params.panel_size + 2.0 * pad_v) - pad_v;
  let world = params.panel_origin + panel_local;
  let viewport_pos = world - params.viewport_min;
  let ndc = vec2<f32>(
    (viewport_pos.x / params.viewport_size.x) * 2.0 - 1.0,
    1.0 - (viewport_pos.y / params.viewport_size.y) * 2.0
  );
  var out: VsOut;
  out.clip = vec4<f32>(ndc, 0.0, 1.0);
  out.panel_local = panel_local;
  return out;
}

// Pre-multiplied colour contribution from a single (col, row) cell at this
// pixel. Returns zero outside everything the cell renders (i.e. outside
// the outline, outside the fill, off the needle). The egui `rect_filled`
// panel surface is drawn underneath in a separate pass, so the cell does
// NOT need to fill its interior with `params.surface`; doing so produced a
// halo of alpha-1 white that occluded neighbouring cells' strokes through
// the 2x2 "over" composite.
//
// `edge` is the pixel-sized fwidth value pre-computed by the caller in
// uniform control flow — must not be re-derived inside this function
// because the 2x2 neighbourhood loop in fs_main already breaks uniform
// flow before reaching here.
fn cell_contribution(col: u32, row: u32, panel_local: vec2<f32>, edge: f32) -> vec4<f32> {
  let cell_origin = vec2<f32>(f32(col), f32(row)) * params.cell_pitch;
  let cell_center = cell_origin + vec2<f32>(params.radius);
  let local = panel_local - cell_center;
  let dist = length(local);
  let half_stroke = params.stroke * 0.5;
  let outer = params.radius + half_stroke;
  if (dist > outer + edge) {
    return vec4<f32>(0.0);
  }
  let display_index = row * params.cols + col;
  let state_index = reverseBits(display_index) >> (32u - params.qubits);
  let amp = state[state_index];
  let prob = clamp(amp.x * amp.x + amp.y * amp.y, 0.0, 1.0);

  // Layer 1: probability fill — solid disc whose radius is √prob × inner_radius.
  let fill_radius = params.inner_radius * sqrt(prob);
  let fill_alpha = 1.0 - smoothstep(fill_radius - edge, fill_radius + edge, dist);
  var color = vec4<f32>(params.fill.rgb * fill_alpha, fill_alpha);

  // Layer 2: phase needle (only when prob > 0).
  if (prob > 0.0) {
    let phase = atan2(amp.y, amp.x);
    let dir = vec2<f32>(-sin(phase), -cos(phase));
    let t = clamp(dot(local, dir), 0.0, params.inner_radius);
    let closest = dir * t;
    let d = length(local - closest);
    let needle_alpha = 1.0 - smoothstep(half_stroke - edge, half_stroke + edge, d);
    let needle_pre = vec4<f32>(params.needle.rgb * needle_alpha, needle_alpha);
    color = needle_pre + color * (1.0 - needle_pre.a);
  }

  // Layer 3: outline ring at radius=params.radius, width=stroke.
  let outline_rgba = select(params.outline_zero, params.outline, prob > 0.0);
  let outline_inner =
    1.0 - smoothstep(params.radius - half_stroke - edge, params.radius - half_stroke + edge, dist);
  let outline_outer =
    1.0 - smoothstep(params.radius + half_stroke - edge, params.radius + half_stroke + edge, dist);
  let outline_alpha = max(0.0, outline_outer - outline_inner);
  let outline_pre = vec4<f32>(outline_rgba.rgb * outline_alpha, outline_alpha);
  color = outline_pre + color * (1.0 - outline_pre.a);

  return color;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
  // Compute the AA edge width once, in uniform control flow, before the
  // per-cell sampling diverges. `length(fwidth(panel_local))` is the size
  // of one fragment in panel-local units (≈ 1 at 1:1 zoom, ≈ 0.5 at
  // DPR=2) and stays valid even when threads in the same 2x2 quad sample
  // different cells.
  let edge = length(fwidth(input.panel_local));
  // qni's layout has gap == stroke, so adjacent cells' strokes meet at a
  // 1-px boundary. The old per-instance render produced ~75 % alpha there
  // by overdrawing both cell quads; we replicate that by sampling the
  // 2x2 cell neighbourhood whose centres surround this pixel and
  // composing their contributions with pre-multiplied "over" alpha.
  // Shift the floor by `radius` so col0 picks the left neighbour when the
  // pixel is in the left half of cell col, the right neighbour when it
  // is in the right half — either way the 2x2 covers the four cells
  // whose centres are nearest the pixel.
  let col0 = i32(floor((input.panel_local.x - params.radius) / params.cell_pitch));
  let row0 = i32(floor((input.panel_local.y - params.radius) / params.cell_pitch));
  let cols_i = i32(params.cols);
  let rows_i = i32(params.rows);
  let s00 = sample_cell(col0,     row0,     cols_i, rows_i, input.panel_local, edge);
  let s10 = sample_cell(col0 + 1, row0,     cols_i, rows_i, input.panel_local, edge);
  let s01 = sample_cell(col0,     row0 + 1, cols_i, rows_i, input.panel_local, edge);
  let s11 = sample_cell(col0 + 1, row0 + 1, cols_i, rows_i, input.panel_local, edge);
  // Composite via pre-multiplied "over": dst = src + dst * (1 - src.a).
  var color = s00;
  color = s10 + color * (1.0 - s10.a);
  color = s01 + color * (1.0 - s01.a);
  color = s11 + color * (1.0 - s11.a);
  if (color.a < 0.001) {
    discard;
  }
  return color;
}

// Bounds-checked wrapper around `cell_contribution`. Returns zero for
// out-of-range indices.
fn sample_cell(
  col: i32,
  row: i32,
  cols: i32,
  rows: i32,
  panel_local: vec2<f32>,
  edge: f32,
) -> vec4<f32> {
  if (col < 0 || col >= cols || row < 0 || row >= rows) {
    return vec4<f32>(0.0);
  }
  return cell_contribution(u32(col), u32(row), panel_local, edge);
}
"#;
