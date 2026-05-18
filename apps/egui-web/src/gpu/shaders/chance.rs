//! Chance display marginalize + render WGSL sources.

pub(in crate::gpu) const CHANCE_REDUCE_SHADER: &str = r#"
struct ChanceReduceParams {
  base_bit: u32,
  span: u32,
  rest_count: u32,
  output_slot: u32,
};

@group(0) @binding(0) var<storage, read> state: array<vec2<f32>>;
@group(0) @binding(1) var<storage, read_write> chance_out: array<f32>;
@group(0) @binding(2) var<uniform> params: ChanceReduceParams;

const MAX_CHANCE_OUTCOMES: u32 = 65536u;

var<workgroup> shared_sum: array<f32, 64>;

fn insert_outcome(rest: u32, outcome: u32) -> u32 {
  let low_mask = (1u << params.base_bit) - 1u;
  let low = rest & low_mask;
  let high = rest >> params.base_bit;
  return low | (outcome << params.base_bit) | (high << (params.base_bit + params.span));
}

@compute @workgroup_size(64)
fn main(
  @builtin(workgroup_id) wid: vec3<u32>,
  @builtin(local_invocation_id) lid: vec3<u32>,
) {
  let outcome = wid.x + wid.y * 256u;
  if (outcome >= (1u << params.span)) { return; }
  let tid = lid.x;
  var sum = 0.0;
  var rest = tid;
  loop {
    if (rest >= params.rest_count) { break; }
    let state_index = insert_outcome(rest, outcome);
    let amp = state[state_index];
    sum = sum + amp.x * amp.x + amp.y * amp.y;
    rest = rest + 64u;
  }
  shared_sum[tid] = sum;
  workgroupBarrier();

  for (var step: u32 = 32u; step > 0u; step = step >> 1u) {
    if (tid < step) {
      shared_sum[tid] = shared_sum[tid] + shared_sum[tid + step];
    }
    workgroupBarrier();
  }

  if (tid == 0u) {
    chance_out[params.output_slot * MAX_CHANCE_OUTCOMES + outcome] = shared_sum[0];
  }
}
"#;

pub(in crate::gpu) const CHANCE_RENDER_SHADER: &str = r#"
struct ChanceRenderParams {
  viewport_min: vec2<f32>,
  viewport_size: vec2<f32>,
  background: vec4<f32>,
  border: vec4<f32>,
  bar: vec4<f32>,
  bar_edge: vec4<f32>,
  hover_border: vec4<f32>,
  text_color: vec4<f32>,
};

@group(0) @binding(0) var<storage, read> chance_data: array<f32>;
@group(0) @binding(1) var<uniform> params: ChanceRenderParams;
@group(0) @binding(2) var atlas: texture_2d<f32>;
@group(0) @binding(3) var atlas_sampler: sampler;

const MAX_CHANCE_OUTCOMES: u32 = 65536u;
const GLYPH_COUNT: u32 = 19u;
const GLYPH_DOT: u32 = 10u;
const GLYPH_PERCENT: u32 = 14u;
const GLYPH_BLANK: u32 = 0xFFFFu;
const CHANCE_TEXT_CHARS: u32 = 6u;
const CHANCE_TEXT_DIGIT_W: f32 = 7.0;
// docs/chance-display.html §06: Geist Mono dot uses the normal 9px cell
// squeezed by −3px side margins, so it still gets a centre sample column.
const CHANCE_TEXT_DOT_W: f32 = 3.0;
const CHANCE_TEXT_CHAR_H: f32 = 16.0;

struct VsIn {
  @location(0) corner: vec2<f32>,
  @location(1) rect_min: vec2<f32>,
  @location(2) rect_size: vec2<f32>,
  @location(3) slot: u32,
  @location(4) span: u32,
  @location(5) hovered_outcome: i32,
};

struct VsOut {
  @builtin(position) clip: vec4<f32>,
  @location(0) local: vec2<f32>,
  @location(1) rect_size: vec2<f32>,
  @location(2) @interpolate(flat) slot: u32,
  @location(3) @interpolate(flat) span: u32,
  @location(4) @interpolate(flat) hovered_outcome: i32,
};

@vertex
fn vs_main(input: VsIn) -> VsOut {
  let local = (input.corner * 0.5 + vec2<f32>(0.5)) * input.rect_size;
  let world = input.rect_min + local;
  let viewport_pos = world - params.viewport_min;
  let ndc = vec2<f32>(
    (viewport_pos.x / params.viewport_size.x) * 2.0 - 1.0,
    1.0 - (viewport_pos.y / params.viewport_size.y) * 2.0,
  );
  var out: VsOut;
  out.clip = vec4<f32>(ndc, 0.0, 1.0);
  out.local = local;
  out.rect_size = input.rect_size;
  out.slot = input.slot;
  out.span = input.span;
  out.hovered_outcome = input.hovered_outcome;
  return out;
}

fn over(top: vec4<f32>, bottom: vec4<f32>) -> vec4<f32> {
  return top + bottom * (1.0 - top.a);
}

fn rounded_percent_tenths(prob_01: f32) -> u32 {
  return min(u32(floor(clamp(prob_01, 0.0, 1.0) * 1000.0 + 0.5001)), 1000u);
}

fn glyph_chance_percent(idx: u32, prob_01: f32) -> u32 {
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

fn chance_glyph_width(glyph: u32) -> f32 {
  if (glyph == GLYPH_BLANK) { return 0.0; }
  if (glyph == GLYPH_DOT) { return CHANCE_TEXT_DOT_W; }
  return CHANCE_TEXT_DIGIT_W;
}

fn chance_text_width(prob_01: f32) -> f32 {
  var width = 0.0;
  for (var idx = 0u; idx < CHANCE_TEXT_CHARS; idx = idx + 1u) {
    width = width + chance_glyph_width(glyph_chance_percent(idx, prob_01));
  }
  return width;
}

fn chance_text_color(local: vec2<f32>, rect_size: vec2<f32>, row: u32, row_count: u32, row_h: f32, prob: f32, base: vec4<f32>) -> vec4<f32> {
  // qni-webgpu policy: Chance1..4 only. Chance5 has >8px rows after the
  // 40px migration, but the 12px label is still too dense for 32 outcomes.
  if (row_count > 16u || row_h <= 8.0) { return base; }
  let text_w = chance_text_width(prob);
  let text_left = rect_size.x - 2.0 - text_w;
  let text_top = f32(row) * row_h + (row_h - CHANCE_TEXT_CHAR_H) * 0.5;
  let p = local - vec2<f32>(text_left, text_top);
  if (p.x < 0.0 || p.y < 0.0 || p.x >= text_w || p.y >= CHANCE_TEXT_CHAR_H) {
    return base;
  }

  var cursor = 0.0;
  for (var idx = 0u; idx < CHANCE_TEXT_CHARS; idx = idx + 1u) {
    let glyph = glyph_chance_percent(idx, prob);
    let char_w = chance_glyph_width(glyph);
    if (char_w > 0.0 && p.x >= cursor && p.x < cursor + char_w) {
      let cell_uv = vec2<f32>((p.x - cursor) / char_w, p.y / CHANCE_TEXT_CHAR_H);
      let atlas_u = (f32(glyph) + cell_uv.x) / f32(GLYPH_COUNT);
      let alpha = textureSampleLevel(atlas, atlas_sampler, vec2<f32>(atlas_u, cell_uv.y), 0.0).r;
      if (alpha < 0.001) { return base; }
      return over(vec4<f32>(params.text_color.rgb * alpha, alpha), base);
    }
    cursor = cursor + char_w;
  }
  return base;
}

fn chance_prob(slot: u32, row: u32) -> f32 {
  return clamp(chance_data[slot * MAX_CHANCE_OUTCOMES + row], 0.0, 1.0);
}

fn chance_log_hint_x(prob: f32, span: u32, width: f32) -> f32 {
  if (prob <= 0.0) { return 0.0; }
  let s = 1.0 / (4.0 + max(8.0, f32(span)));
  return clamp(1.0 + log(prob) * s, 0.0, 1.0) * width;
}

fn chance_aggregate_prob_for_pixel(slot: u32, span: u32, row_count: u32, row_h: f32, local_y: f32, fallback_row: u32) -> f32 {
  let pixel_h = max(fwidth(local_y), row_h);
  if (span < 13u) { return chance_prob(slot, fallback_row); }
  let row_lo = u32(clamp(floor((local_y - pixel_h * 0.5) / row_h), 0.0, f32(row_count - 1u)));
  let row_hi = u32(clamp(floor((local_y + pixel_h * 0.5) / row_h), 0.0, f32(row_count - 1u)));
  var p_max = 0.0;
  var row = row_lo;
  loop {
    p_max = max(p_max, chance_prob(slot, row));
    if (row >= row_hi) { break; }
    row = row + 1u;
  }
  return p_max;
}

fn on_log_hint(local_x: f32, local_y: f32, slot: u32, span: u32, row: u32, row_h: f32, prob: f32, width: f32) -> bool {
  let hint_x = chance_log_hint_x(prob, span, width);
  if (abs(local_x - hint_x) < 0.5) { return true; }
  if (span >= 13u || row == 0u) { return false; }
  let row_y = local_y - f32(row) * row_h;
  if (row_y >= 1.0) { return false; }
  let prev_x = chance_log_hint_x(chance_prob(slot, row - 1u), span, width);
  return local_x >= min(prev_x, hint_x) && local_x <= max(prev_x, hint_x);
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
  let row_count = 1u << input.span;
  let row_h = input.rect_size.y / f32(row_count);
  let raw_row = u32(clamp(floor(input.local.y / max(row_h, 1.0e-6)), 0.0, f32(row_count - 1u)));
  let prob = chance_prob(input.slot, raw_row);
  let draw_prob = chance_aggregate_prob_for_pixel(input.slot, input.span, row_count, row_h, input.local.y, raw_row);
  let hover = i32(raw_row) == input.hovered_outcome;
  let bar_right = draw_prob * input.rect_size.x;
  let show_text = row_count <= 16u && row_h > 8.0;

  var color = params.background;
  if (!show_text && on_log_hint(input.local.x, input.local.y, input.slot, input.span, raw_row, row_h, draw_prob, input.rect_size.x)) {
    color = params.border;
  }
  if (input.local.x <= bar_right) {
    color = params.bar;
  }
  if (draw_prob > 0.0 && draw_prob < 1.0 && abs(input.local.x - bar_right) < 0.5) {
    color = params.bar_edge;
  }

  let border_px = 1.0;
  let on_border =
    input.local.x < border_px ||
    input.local.y < border_px ||
    input.local.x >= input.rect_size.x - border_px ||
    input.local.y >= input.rect_size.y - border_px;
  let row_pos = input.local.y / max(row_h, 1.0e-6);
  let row_frac = fract(row_pos);
  var on_separator = false;
  if (row_h > border_px * 2.0 && row_count > 1u && raw_row > 0u && row_frac * row_h < border_px) {
    let prev_prob = clamp(chance_data[input.slot * MAX_CHANCE_OUTCOMES + raw_row - 1u], 0.0, 1.0);
    let separator_x = max(prev_prob, prob) * input.rect_size.x;
    on_separator = input.local.x >= separator_x;
  }
  if (on_border || on_separator) {
    color = params.border;
  }

  let row_y = input.local.y - f32(raw_row) * row_h;
  let hover_border_px = 2.0;
  let on_hover_border = hover && (
    input.local.x < hover_border_px ||
    input.local.x >= input.rect_size.x - hover_border_px ||
    row_y < hover_border_px ||
    row_y >= row_h - hover_border_px
  );
  if (on_hover_border) {
    color = params.hover_border;
  }

  return chance_text_color(input.local, input.rect_size, raw_row, row_count, row_h, prob, color);
}
"#;
