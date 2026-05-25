//! Amplitude display hover-popup value text shader.

pub(in crate::gpu) const AMPLITUDE_POPUP_VALUE_SHADER: &str = r#"
const MAX_OUTCOMES: u32 = 65536u;
const VALUES_PER_SLOT: u32 = MAX_OUTCOMES * 3u;
const GLYPH_COUNT: u32 = 20u;
const CHARS_PER_ROW: u32 = 18u;

struct PopupParams {
    viewport_min: vec2<f32>,
    viewport_size: vec2<f32>,
    value_anchor: vec2<f32>,
    row_pitch: f32,
    _pad_row: f32,
    char_size: vec2<f32>,
    _pad_char: vec2<f32>,
    text_color: vec4<f32>,
    slot: u32,
    outcome: u32,
    _pad0: u32,
    _pad1: u32,
};

struct VertexOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>,
};

@group(0) @binding(0) var<storage, read> amplitude_data: array<f32>;
@group(0) @binding(1) var<storage, read> amplitude_meta: array<vec4<f32>>;
@group(0) @binding(2) var<uniform> params: PopupParams;
@group(0) @binding(3) var glyph_tex: texture_2d<f32>;
@group(0) @binding(4) var glyph_sampler: sampler;

fn glyph_digit(value: u32) -> u32 { return min(value, 9u); }
fn glyph_dot() -> u32 { return 10u; }
fn glyph_plus() -> u32 { return 11u; }
fn glyph_minus() -> u32 { return 12u; }
fn glyph_i() -> u32 { return 13u; }
fn glyph_percent() -> u32 { return 14u; }
fn glyph_degree() -> u32 { return 15u; }

fn digit_from_scaled(value: u32, divisor: u32) -> u32 {
    return (value / divisor) % 10u;
}

fn row0_glyph(col: u32, re: f32, im: f32) -> u32 {
    // ±d.ddddd±d.dddddi, padded to 18 monospace cells.
    let re_neg = re < 0.0;
    let im_neg = im < 0.0;
    let re_scaled = u32(round(clamp(abs(re), 0.0, 9.99999) * 100000.0));
    let im_scaled = u32(round(clamp(abs(im), 0.0, 9.99999) * 100000.0));
    switch col {
        case 0u: { return select(glyph_plus(), glyph_minus(), re_neg); }
        case 1u: { return digit_from_scaled(re_scaled, 100000u); }
        case 2u: { return glyph_dot(); }
        case 3u: { return digit_from_scaled(re_scaled, 10000u); }
        case 4u: { return digit_from_scaled(re_scaled, 1000u); }
        case 5u: { return digit_from_scaled(re_scaled, 100u); }
        case 6u: { return digit_from_scaled(re_scaled, 10u); }
        case 7u: { return digit_from_scaled(re_scaled, 1u); }
        case 8u: { return select(glyph_plus(), glyph_minus(), im_neg); }
        case 9u: { return digit_from_scaled(im_scaled, 100000u); }
        case 10u: { return glyph_dot(); }
        case 11u: { return digit_from_scaled(im_scaled, 10000u); }
        case 12u: { return digit_from_scaled(im_scaled, 1000u); }
        case 13u: { return digit_from_scaled(im_scaled, 100u); }
        case 14u: { return digit_from_scaled(im_scaled, 10u); }
        case 15u: { return digit_from_scaled(im_scaled, 1u); }
        case 16u: { return glyph_i(); }
        default: { return 0u; }
    }
}

fn row1_glyph(col: u32, percent: f32) -> u32 {
    // ddd.dddd%, right-sized for 9 cells; repeated columns after that are transparent.
    let scaled = u32(round(clamp(percent, 0.0, 999.9999) * 10000.0));
    switch col {
        case 0u: { return digit_from_scaled(scaled, 1000000u); }
        case 1u: { return digit_from_scaled(scaled, 100000u); }
        case 2u: { return digit_from_scaled(scaled, 10000u); }
        case 3u: { return glyph_dot(); }
        case 4u: { return digit_from_scaled(scaled, 1000u); }
        case 5u: { return digit_from_scaled(scaled, 100u); }
        case 6u: { return digit_from_scaled(scaled, 10u); }
        case 7u: { return digit_from_scaled(scaled, 1u); }
        case 8u: { return glyph_percent(); }
        default: { return 0u; }
    }
}

fn row2_glyph(col: u32, phase_deg: f32) -> u32 {
    // ±ddd.dd°
    let neg = phase_deg < 0.0;
    let scaled = u32(round(clamp(abs(phase_deg), 0.0, 999.99) * 100.0));
    switch col {
        case 0u: { return select(glyph_plus(), glyph_minus(), neg); }
        case 1u: { return digit_from_scaled(scaled, 10000u); }
        case 2u: { return digit_from_scaled(scaled, 1000u); }
        case 3u: { return digit_from_scaled(scaled, 100u); }
        case 4u: { return glyph_dot(); }
        case 5u: { return digit_from_scaled(scaled, 10u); }
        case 6u: { return digit_from_scaled(scaled, 1u); }
        case 7u: { return glyph_degree(); }
        default: { return 0u; }
    }
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOut {
    let total = vec2<f32>(params.char_size.x * f32(CHARS_PER_ROW), params.row_pitch * 2.0 + params.char_size.y);
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 1.0)
    );
    let local = corners[vertex_index] * total;
    let pixel = params.value_anchor + local;
    let ndc = ((pixel - params.viewport_min) / params.viewport_size) * 2.0 - vec2<f32>(1.0, 1.0);
    var out: VertexOut;
    out.pos = vec4<f32>(ndc.x, -ndc.y, 0.0, 1.0);
    out.local = local;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let row = u32(floor(in.local.y / params.row_pitch));
    if (row > 2u) {
        discard;
    }
    let row_y = in.local.y - f32(row) * params.row_pitch;
    if (row_y < 0.0 || row_y >= params.char_size.y) {
        discard;
    }
    let col = u32(floor(in.local.x / params.char_size.x));
    if (col >= CHARS_PER_ROW) {
        discard;
    }

    let base = params.slot * VALUES_PER_SLOT;
    let amp_meta = amplitude_meta[params.slot];
    let incoherent = amp_meta.x < 0.99;
    var re = amplitude_data[base + 2u * params.outcome];
    var im = amplitude_data[base + 2u * params.outcome + 1u];
    if (incoherent) {
        re = amplitude_data[base + 2u * MAX_OUTCOMES + params.outcome];
        im = 0.0;
    }
    let mag2 = re * re + im * im;
    var phase_deg = atan2(im, re) * 57.29577951308232;
    if (mag2 <= 0.00000001) {
        phase_deg = 0.0;
    }

    var glyph = 0u;
    if (row == 0u) {
        glyph = row0_glyph(col, re, im);
    } else if (row == 1u) {
        glyph = row1_glyph(col, mag2 * 100.0);
    } else {
        glyph = row2_glyph(col, phase_deg);
    }

    if ((row == 0u && col >= 17u) || (row == 1u && col >= 9u) || (row == 2u && col >= 8u)) {
        discard;
    }
    let uv = vec2<f32>((f32(glyph) + fract(in.local.x / params.char_size.x)) / f32(GLYPH_COUNT), row_y / params.char_size.y);
    let alpha = textureSample(glyph_tex, glyph_sampler, uv).r;
    if (alpha <= 0.01) {
        discard;
    }
    return vec4<f32>(params.text_color.rgb, params.text_color.a * alpha);
}
"#;
