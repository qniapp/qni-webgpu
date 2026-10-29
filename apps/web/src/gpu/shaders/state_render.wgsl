
struct RenderParams {
  viewport_min: vec2<f32>,
  viewport_size: vec2<f32>,
  panel_origin: vec2<f32>,
  panel_size: vec2<f32>,
  cell_pitch: f32,
  radius: f32,
  inner_radius: f32,
  stroke: f32,
  cols: u32,
  rows: u32,
  qubits: u32,
  hovered_cell: i32,
  fill: vec4<f32>,
  disk_border: vec4<f32>,
  outline: vec4<f32>,
  outline_zero: vec4<f32>,
  needle: vec4<f32>,
};

@group(0) @binding(0) var<storage, read> state: array<vec2<f32>>;
@group(0) @binding(1) var<uniform> params: RenderParams;
@group(0) @binding(2) var<storage, read> selected_state_norm: array<vec4<f32>>;

struct VsIn {
  @location(0) position: vec2<f32>,
};

struct VsOut {
  @builtin(position) clip: vec4<f32>,
  @location(0) panel_local: vec2<f32>,
};

@vertex
fn vs_main(input: VsIn) -> VsOut {
  let pad = params.stroke * 0.5 + 1.0;
  let pad_v = vec2<f32>(pad);
  let panel_local =
    (input.position * 0.5 + 0.5) * (params.panel_size + 2.0 * pad_v) - pad_v;
  let world = params.panel_origin + panel_local;
  let viewport_pos = world - params.viewport_min;
  let ndc = vec2<f32>(
    (viewport_pos.x / params.viewport_size.x) * 2.0 - 1.0,
    1.0 - (viewport_pos.y / params.viewport_size.y) * 2.0
  );
  var out: VsOut;
  out.clip = vec4<f32>(ndc, 0.0, 1.0);
  out.panel_local = panel_local;
  return out;
}

const AMPLITUDE_CUTOFF: f32 = 0.000001;

// `edge` is the pixel-sized fwidth value pre-computed by the caller in
// uniform control flow — must not be re-derived inside this function
// because the bounds checks in fs_main break uniform flow before reaching here.
fn cell_contribution(col: u32, row: u32, panel_local: vec2<f32>, edge: f32) -> vec4<f32> {
  let cell_origin = vec2<f32>(f32(col), f32(row)) * params.cell_pitch;
  let cell_center = cell_origin + vec2<f32>(params.radius);
  let local = panel_local - cell_center;
  let dist = length(local);
  let half_stroke = params.stroke * 0.5;
  let outer = params.radius + half_stroke;
  if (dist > outer + edge) {
    return vec4<f32>(0.0);
  }
  let display_index = row * params.cols + col;
  let amp = state[display_index];
  let prob = clamp(amp.x * amp.x + amp.y * amp.y, 0.0, 1.0);
  let has_visible_phase = prob > AMPLITUDE_CUTOFF * AMPLITUDE_CUTOFF;

  let hover_mult = select(1.0, 0.9, i32(display_index) == params.hovered_cell);

  let fill_radius = params.inner_radius * sqrt(prob);
  let fill_alpha = 1.0 - smoothstep(fill_radius - edge, fill_radius + edge, dist);
  var color = vec4<f32>(params.fill.rgb * hover_mult * fill_alpha, fill_alpha);
  if (fill_radius >= 1.5) {
    let disk_border_radius = fill_radius - 0.5;
    let disk_border_inner = 1.0 - smoothstep(
      disk_border_radius - 0.5 - edge,
      disk_border_radius - 0.5 + edge,
      dist
    );
    let disk_border_outer = 1.0 - smoothstep(
      disk_border_radius + 0.5 - edge,
      disk_border_radius + 0.5 + edge,
      dist
    );
    let disk_border_alpha = min(max(0.0, disk_border_outer - disk_border_inner), fill_alpha);
    let disk_border_pre = vec4<f32>(params.disk_border.rgb * hover_mult * disk_border_alpha, disk_border_alpha);
    color = disk_border_pre + color * (1.0 - disk_border_pre.a);
  }

  if (has_visible_phase) {
    let phase = atan2(amp.y, amp.x);
    let dir = vec2<f32>(-sin(phase), -cos(phase));
    let t = clamp(dot(local, dir), 0.0, params.inner_radius);
    let closest = dir * t;
    let d = length(local - closest);
    let needle_alpha = 1.0 - smoothstep(half_stroke - edge, half_stroke + edge, d);
    let needle_pre = vec4<f32>(params.needle.rgb * hover_mult * needle_alpha, needle_alpha);
    color = needle_pre + color * (1.0 - needle_pre.a);
  }

  let outline_rgba = select(params.outline_zero, params.outline, has_visible_phase);
  let outline_inner =
    1.0 - smoothstep(params.radius - half_stroke - edge, params.radius - half_stroke + edge, dist);
  let outline_outer =
    1.0 - smoothstep(params.radius + half_stroke - edge, params.radius + half_stroke + edge, dist);
  let outline_alpha = max(0.0, outline_outer - outline_inner);
  let outline_pre = vec4<f32>(outline_rgba.rgb * hover_mult * outline_alpha, outline_alpha);
  color = outline_pre + color * (1.0 - outline_pre.a);

  return color;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
  // `edge` is computed once here in uniform control flow and passed down
  // to `cell_contribution`; the per-cell function must not call fwidth
  // itself because the bounds-check below would break uniform flow.
  let edge = max(0.5, length(fwidth(input.panel_local)) * 0.65);
  if (!(selected_state_norm[0].x > 0.000000000001)) {
    if (any(input.panel_local < vec2<f32>(0.0)) || any(input.panel_local >= params.panel_size)) {
      discard;
    }
    if (fract((input.panel_local.x + input.panel_local.y) / 8.0) >= 0.125) {
      discard;
    }
    return vec4<f32>(params.outline_zero.rgb * params.outline_zero.a, params.outline_zero.a);
  }
  let half_gap = (params.cell_pitch - 2.0 * params.radius) * 0.5;
  let col_f = floor((input.panel_local.x + half_gap) / params.cell_pitch);
  let row_f = floor((input.panel_local.y + half_gap) / params.cell_pitch);
  let col = u32(clamp(col_f, 0.0, f32(params.cols - 1u)));
  let row = u32(clamp(row_f, 0.0, f32(params.rows - 1u)));
  let pre = cell_contribution(col, row, input.panel_local, edge);
  if (pre.a < 0.001) {
    discard;
  }
  return pre;
}
