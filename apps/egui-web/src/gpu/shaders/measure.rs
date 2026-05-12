//! Measurement reduction and collapse WGSL sources.

pub(in crate::gpu) const MEASURE_REDUCE_SHADER: &str = r#"
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
pub(in crate::gpu) const MEASURE_COLLAPSE_SHADER: &str = r#"
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
