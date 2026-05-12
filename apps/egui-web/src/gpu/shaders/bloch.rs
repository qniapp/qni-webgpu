//! Bloch vector reduction and overlay WGSL sources.

pub(in crate::gpu) const BLOCH_REDUCE_SHADER: &str = r#"
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
pub(in crate::gpu) const BLOCH_OVERLAY_SHADER: &str = r#"
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
