struct DigitParams {
  viewport_min: vec2<f32>,
  viewport_size: vec2<f32>,
  zero_color: vec4<f32>,
};

@group(0) @binding(0) var<storage, read> aux_data: array<vec4<f32>>;
@group(0) @binding(1) var<uniform> params: DigitParams;
@group(0) @binding(2) var digit_atlas: texture_2d<f32>;
@group(0) @binding(3) var digit_sampler: sampler;

struct VsIn {
  @location(0) corner: vec2<f32>,
  @location(1) center: vec2<f32>,
  @location(2) half_extent: f32,
  @location(3) first_slot: u32,
  @location(4) last_slot: u32,
  @location(5) mode: u32,
};

struct VsOut {
  @builtin(position) clip: vec4<f32>,
  @location(0) local: vec2<f32>,
  @location(1) @interpolate(flat) first_slot: u32,
  @location(2) @interpolate(flat) last_slot: u32,
  @location(3) @interpolate(flat) mode: u32,
};

@vertex
fn vs_main(input: VsIn) -> VsOut {
  let local = input.corner * vec2<f32>(input.half_extent, select(20.0, 10.0, input.mode == 1u));
  let world = input.center + local;
  let viewport_pos = world - params.viewport_min;
  var out: VsOut;
  out.clip = vec4<f32>(
    (viewport_pos.x / params.viewport_size.x) * 2.0 - 1.0,
    1.0 - (viewport_pos.y / params.viewport_size.y) * 2.0, 0.0, 1.0);
  out.local = local;
  out.first_slot = input.first_slot;
  out.last_slot = input.last_slot;
  out.mode = input.mode;
  return out;
}

struct Text {
  glyphs: array<u32, 16>,
  count: u32,
};

fn append_number(text: Text, number: u32) -> Text {
  var result = text;
  var divisor = 1u;
  for (var n = number; n >= 10u; n /= 10u) { divisor *= 10u; }
  loop {
    result.glyphs[result.count] = (number / divisor) % 10u;
    result.count++;
    if (divisor == 1u) { break; }
    divisor /= 10u;
  }
  return result;
}

fn label_text(rate: f32, marginal: f32, mode: u32, line: u32) -> Text {
  var text: Text;
  if (mode == 1u) {
    text.glyphs[0] = 21u;
    text.glyphs[1] = 22u;
    text.glyphs[2] = 23u;
    text.glyphs[3] = 16u;
    text.glyphs[4] = 24u;
    text.glyphs[5] = 99u;
    text.count = 6u;
    let percent = u32(floor(max(rate, 0.0) * 100.0 + 0.5));
    if (abs(rate) > 1e-12 && percent == 0u) {
      text.glyphs[text.count] = 11u;
      text.count++;
      text = append_number(text, 1u);
    } else {
      text = append_number(text, percent);
    }
  } else if (line == 0u) {
    if (marginal < 0.0) {
      text.glyphs[0] = 13u;
      text.glyphs[1] = 14u;
      text.glyphs[2] = 15u;
      text.glyphs[3] = 16u;
      text.glyphs[4] = 17u;
    } else {
      text.glyphs[0] = 18u;
      text.glyphs[1] = 19u;
      text.glyphs[2] = 15u;
      text.glyphs[3] = 20u;
      text.glyphs[4] = 17u;
    }
    text.count = 5u;
    return text;
  } else if (marginal < 0.0) {
    let percent = u32(floor(-marginal * 100.0 + 0.5));
    if (rate > 0.0 && percent >= 100u) {
      text.glyphs[0] = 12u;
      text.count = 1u;
      text = append_number(text, 99u);
    } else {
      text = append_number(text, percent);
    }
  } else {
    text = append_number(text, u32(floor(marginal * 100.0 + 100.5)));
  }
  text.glyphs[text.count] = 10u;
  text.count++;
  return text;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
  let rate = aux_data[input.last_slot].x;
  var previous = 1.0;
  if (input.first_slot > 0u) { previous = aux_data[input.first_slot - 1u].x; }
  let marginal = (rate - previous) / previous;
  if (input.mode == 0u) {
    if (!(abs(marginal) > 0.005)) { discard; }
  } else {
    if (!(abs(rate - 1.0) > 0.01)) { discard; }
  }
  let y = input.local.y + select(20.0, 10.0, input.mode == 1u);
  let line = u32(floor(y / 20.0));
  let text = label_text(rate, marginal, input.mode, line);
  let width = f32(text.count) * 9.0;
  let x = input.local.x + width * 0.5;
  if (x < 0.0 || x >= width) { discard; }
  let cell = u32(floor(x / 9.0));
  let glyph = text.glyphs[cell];
  if (glyph == 99u) { discard; }
  let uv = vec2<f32>((f32(glyph) + fract(x / 9.0)) / 25.0, fract(y / 20.0));
  let alpha = textureSampleLevel(digit_atlas, digit_sampler, uv, 0.0).r * params.zero_color.a;
  return vec4<f32>(params.zero_color.rgb * alpha, alpha);
}
