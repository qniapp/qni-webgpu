struct Params { state_count: u32, output_slot: u32, pad: vec2<u32> };
@group(0) @binding(0) var<storage, read> state: array<vec2<f32>>;
@group(0) @binding(1) var<storage, read_write> rates: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> survival: array<f32>;
@group(0) @binding(3) var<uniform> params: Params;
var<workgroup> partial: array<f32, 64>;
@compute @workgroup_size(64)
fn main(@builtin(local_invocation_index) tid: u32) {
  var sum = 0.0;
  for (var i = tid; i < params.state_count; i += 64u) { sum += dot(state[i], state[i]); }
  partial[tid] = sum;
  workgroupBarrier();
  for (var step = 32u; step > 0u; step >>= 1u) {
    if (tid < step) { partial[tid] += partial[tid + step]; }
    workgroupBarrier();
  }
  if (tid == 0u) {
    let norm = partial[0];
    var scale = 0.0;
    if (norm > 1e-12) { scale = inverseSqrt(norm); }
    survival[0] *= select(0.0, clamp(norm, 0.0, 1.0), norm > 1e-12);
    rates[params.output_slot] = vec4<f32>(survival[0], scale, norm, 0.0);
  }
}
