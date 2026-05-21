//! Bloch hover popup value text WGSL source.

pub(in crate::gpu) const BLOCH_POPUP_VALUE_SHADER: &str = r#"
struct BlochPopupParams {
  // Egui callback viewport in CSS pixels.
  viewport_min: vec2<f32>,
  viewport_size: vec2<f32>,
  // Top-left of the first value cell (`r`), in egui pixels.
  value_anchor: vec2<f32>,
  col_pitch: f32,
  row_pitch: f32,
  char_size: vec2<f32>,
  _pad_char: vec2<f32>,
  text_color: vec4<f32>,
  slot: u32,
  _pad0: u32,
  _pad1: u32,
  _pad2: u32,
};

@group(0) @binding(0) var<storage, read> bloch_data: array<vec4<f32>>;
@group(0) @binding(1) var<uniform> params: BlochPopupParams;
@group(0) @binding(2) var atlas: texture_2d<f32>;
@group(0) @binding(3) var atlas_sampler: sampler;

const GLYPH_COUNT: u32 = 25u;
const GLYPH_DOT: u32 = 10u;
const GLYPH_PLUS: u32 = 11u;
const GLYPH_MINUS: u32 = 12u;
const GLYPH_DEGREE: u32 = 15u;
const GLYPH_EM_DASH: u32 = 24u;
const GLYPH_BLANK: u32 = 0xFFFFu;

const VALUE_CHARS: u32 = 8u;
const RAD_TO_DEG: f32 = 57.29577951308232;

fn pow10_table(exp: i32) -> f32 {
  switch (exp) {
    case 4: { return 10000.0; }
    case 3: { return 1000.0; }
    case 2: { return 100.0; }
    case 1: { return 10.0; }
    case 0: { return 1.0; }
    case -1: { return 0.1; }
    case -2: { return 0.01; }
    case -3: { return 0.001; }
    case -4: { return 0.0001; }
    default: { return 1.0; }
  }
}

fn digit_at(value: f32, exp: i32) -> u32 {
  var scaled: f32;
  if (exp >= 0) {
    scaled = value / pow10_table(exp);
  } else {
    scaled = value * pow10_table(-exp);
  }
  return u32(floor(scaled)) % 10u;
}

fn round_to(value: f32, frac_digits: u32) -> f32 {
  let m = pow10_table(i32(frac_digits));
  return round(value * m) / m;
}

fn sign_glyph(value: f32) -> u32 {
  if (value < 0.0) { return GLYPH_MINUS; }
  return GLYPH_PLUS;
}

fn glyph_signed_fixed4(idx: u32, value: f32) -> u32 {
  // forceSign · toFixed(4): +0.0000 through +1.0000 / −1.0000.
  let v = round_to(abs(value), 4u);
  if (idx == 0u) { return sign_glyph(value); }
  if (idx == 1u) { return digit_at(v, 0); }
  if (idx == 2u) { return GLYPH_DOT; }
  if (idx >= 3u && idx <= 6u) { return digit_at(v, -(i32(idx) - 2)); }
  return GLYPH_BLANK;
}

fn angle_digit_count(value_abs: f32) -> u32 {
  if (value_abs >= 100.0) { return 3u; }
  if (value_abs >= 10.0) { return 2u; }
  return 1u;
}

fn glyph_signed_degrees(idx: u32, degrees: f32) -> u32 {
  // forceSign · toFixed(2) + "°". Left-aligned inside an 8-cell value box.
  let v = round_to(abs(degrees), 2u);
  let digits = angle_digit_count(v);
  if (idx == 0u) { return sign_glyph(degrees); }
  if (idx >= 1u && idx < 1u + digits) {
    let pos = idx - 1u;
    return digit_at(v, i32(digits - 1u - pos));
  }
  let dot_idx = 1u + digits;
  if (idx == dot_idx) { return GLYPH_DOT; }
  if (idx == dot_idx + 1u) { return digit_at(v, -1); }
  if (idx == dot_idx + 2u) { return digit_at(v, -2); }
  if (idx == dot_idx + 3u) { return GLYPH_DEGREE; }
  return GLYPH_BLANK;
}

fn glyph_dash(idx: u32) -> u32 {
  if (idx == 0u) { return GLYPH_EM_DASH; }
  return GLYPH_BLANK;
}

fn glyph_for_cell(cell: u32, idx: u32, bloch: vec3<f32>) -> u32 {
  let r = sqrt(dot(bloch, bloch));
  if (cell == 0u) { return glyph_signed_fixed4(idx, r); }
  if (cell == 1u) {
    if (r <= 1.0e-6) { return glyph_dash(idx); }
    return glyph_signed_degrees(idx, atan2(bloch.y, bloch.x) * RAD_TO_DEG);
  }
  if (cell == 2u) {
    if (r <= 1.0e-6) { return glyph_dash(idx); }
    let theta = acos(clamp(bloch.z / max(r, 1.0e-6), -1.0, 1.0)) * RAD_TO_DEG;
    return glyph_signed_degrees(idx, theta);
  }
  if (cell == 3u) { return glyph_signed_fixed4(idx, bloch.x); }
  if (cell == 4u) { return glyph_signed_fixed4(idx, bloch.y); }
  if (cell == 5u) { return glyph_signed_fixed4(idx, bloch.z); }
  return GLYPH_BLANK;
}

struct VsOut {
  @builtin(position) clip: vec4<f32>,
  @location(0) cell_uv: vec2<f32>,
  @location(1) @interpolate(flat) cell: u32,
};

@vertex
fn vs_main(
  @builtin(vertex_index) vi: u32,
  @builtin(instance_index) ii: u32,
) -> VsOut {
  var corners = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 0.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(0.0, 1.0),
  );
  let corner = corners[vi];
  let row = ii / 3u;
  let col = ii - row * 3u;
  let origin = params.value_anchor + vec2<f32>(
    f32(col) * params.col_pitch,
    f32(row) * params.row_pitch,
  );
  let size = vec2<f32>(f32(VALUE_CHARS) * params.char_size.x, params.char_size.y);
  let world = origin + corner * size;
  let vp = world - params.viewport_min;
  let ndc = vec2<f32>(
    vp.x / params.viewport_size.x * 2.0 - 1.0,
    1.0 - vp.y / params.viewport_size.y * 2.0,
  );
  var out: VsOut;
  out.clip = vec4<f32>(ndc, 0.0, 1.0);
  out.cell_uv = corner * vec2<f32>(f32(VALUE_CHARS), 1.0);
  out.cell = ii;
  return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
  let char_index = u32(floor(input.cell_uv.x));
  let cell_local = vec2<f32>(fract(input.cell_uv.x), input.cell_uv.y);
  let bloch = bloch_data[params.slot].xyz;
  let glyph = glyph_for_cell(input.cell, char_index, bloch);
  if (glyph == GLYPH_BLANK) { discard; }

  let atlas_u = (f32(glyph) + cell_local.x) / f32(GLYPH_COUNT);
  let alpha = textureSampleLevel(atlas, atlas_sampler, vec2<f32>(atlas_u, cell_local.y), 0.0).r;
  if (alpha < 0.001) { discard; }
  return vec4<f32>(params.text_color.rgb * alpha, alpha);
}
"#;
