struct Params { state_count: u32, pad0: u32, pad1: u32, pad2: u32 };
@group(0) @binding(0) var<storage, read> state: array<vec2<f32>>;
@group(0) @binding(1) var<storage, read_write> status: array<vec4<f32>>;
@group(0) @binding(2) var<uniform> params: Params;
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
  if (tid == 0u) { status[0] = vec4<f32>(partial[0], 0.0, 0.0, 0.0); }
}
