struct Params { state_count: u32, output_slot: u32, pad: vec2<u32> };
@group(0) @binding(0) var<storage, read> state: array<vec2<f32>>;
@group(0) @binding(1) var<storage, read_write> output: array<vec2<f32>>;
@group(0) @binding(2) var<storage, read> rates: array<vec4<f32>>;
@group(0) @binding(3) var<uniform> params: Params;
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= params.state_count) { return; }
  let scale = rates[params.output_slot].y;
  if (scale > 0.0) { output[id.x] = state[id.x] * scale; }
  else { output[id.x] = vec2<f32>(0.0); }
}
