
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
  let local = input.corner * vec2<f32>(input.half_extent, 8.0);
  let world = input.center + local;
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

const GLYPH_BLANK: u32 = 99u;
const GLYPH_DOT: u32 = 10u;
const GLYPH_PERCENT: u32 = 14u;
fn rounded_percent_tenths(prob_01: f32) -> u32 {
  return min(u32(floor(clamp(prob_01, 0.0, 1.0) * 1000.0 + 0.5001)), 1000u);
}

fn glyph_probability_percent(idx: u32, prob_01: f32) -> u32 {
  let tenths = rounded_percent_tenths(prob_01);
  if (idx == 0u) {
    if (tenths < 1000u) { return GLYPH_BLANK; }
    return tenths / 1000u;
  }
  if (idx == 1u) {
    if (tenths < 100u) { return GLYPH_BLANK; }
    return (tenths / 100u) % 10u;
  }
  if (idx == 2u) { return (tenths / 10u) % 10u; }
  if (idx == 3u) { return GLYPH_DOT; }
  if (idx == 4u) { return tenths % 10u; }
  if (idx == 5u) { return GLYPH_PERCENT; }
  return GLYPH_BLANK;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
  let rate = aux_data[input.slot].x;
  var width = 0.0;
  for (var i = 0u; i < 6u; i++) {
    let g = glyph_probability_percent(i, rate);
    if (g != GLYPH_BLANK) { width += select(7.0, 3.0, g == GLYPH_DOT); }
  }
  let p = input.local + vec2<f32>(width * 0.5, 8.0);
  if (p.x < 0.0 || p.x >= width || p.y < 0.0 || p.y >= 16.0) { discard; }
  var cursor = 0.0;
  for (var i = 0u; i < 6u; i++) {
    let g = glyph_probability_percent(i, rate);
    if (g == GLYPH_BLANK) { continue; }
    let w = select(7.0, 3.0, g == GLYPH_DOT);
    if (p.x >= cursor && p.x < cursor + w) {
      let uv = vec2<f32>((f32(g) + (p.x - cursor) / w) / 20.0, p.y / 16.0);
      let alpha = textureSampleLevel(digit_atlas, digit_sampler, uv, 0.0).r;
      return vec4<f32>(params.zero_color.rgb * alpha, alpha);
    }
    cursor += w;
  }
  discard;
  return vec4<f32>(0.0);
}
