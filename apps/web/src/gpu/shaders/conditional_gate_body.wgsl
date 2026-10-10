
struct ConditionalGateBodyParams {
  viewport_min: vec2<f32>,
  viewport_size: vec2<f32>,
  enabled_color: vec4<f32>,
  disabled_color: vec4<f32>,
};

@group(0) @binding(0) var<storage, read> aux_data: array<vec4<f32>>;
@group(0) @binding(1) var<uniform> params: ConditionalGateBodyParams;

// One AA fringe pixel around the body.
const FRINGE: f32 = 1.0;

struct VsIn {
  @location(0) corner: vec2<f32>,
  @location(1) center: vec2<f32>,
  @location(2) half_size: f32,
  @location(3) corner_radius: f32,
  @location(4) slot: u32,
};

struct VsOut {
  @builtin(position) clip: vec4<f32>,
  @location(0) local: vec2<f32>,
  @location(1) half_size: f32,
  @location(2) corner_radius: f32,
  @location(3) @interpolate(flat) slot: u32,
};

@vertex
fn vs_main(input: VsIn) -> VsOut {
  let local = input.corner * (input.half_size + FRINGE);
  let world = input.center + local;
  // See BLOCH_OVERLAY_SHADER: NDC maps to the egui callback viewport.
  let viewport_pos = world - params.viewport_min;
  let ndc = vec2<f32>(
    (viewport_pos.x / params.viewport_size.x) * 2.0 - 1.0,
    1.0 - (viewport_pos.y / params.viewport_size.y) * 2.0,
  );
  var out: VsOut;
  out.clip = vec4<f32>(ndc, 0.0, 1.0);
  out.local = local;
  out.half_size = input.half_size;
  out.corner_radius = input.corner_radius;
  out.slot = input.slot;
  return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
  // Rounded-box signed distance; `corner_radius == half_size` is a circle
  // (the X gate body).
  let inner = vec2<f32>(input.half_size - input.corner_radius);
  let q = abs(input.local) - inner;
  let distance = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - input.corner_radius;
  let alpha = clamp(0.5 - distance, 0.0, 1.0);
  if (alpha < 1.0e-3) {
    discard;
  }
  // qni `quantum-simulator-element.ts`: a conditional gate whose flag is not
  // 1 gets `disabled` and qni.css paints its body in the disabled fill.
  let applied = aux_data[input.slot].z >= 0.5;
  let color = select(params.disabled_color.rgb, params.enabled_color.rgb, applied);
  return vec4<f32>(color * alpha, alpha);
}
