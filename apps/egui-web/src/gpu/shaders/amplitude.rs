//! Amplitude display capture + rendering shaders.
//!
//! Both the gate body and popup values consume GPU storage buffers directly;
//! production rendering never maps amplitude values back to the CPU.

pub(in crate::gpu) const AMPLITUDE_CAPTURE_SHADER: &str = r#"
const MAX_OUTCOMES: u32 = 65536u;
const VALUES_PER_SLOT: u32 = MAX_OUTCOMES * 3u;

struct Params {
    base_bit: u32,
    span: u32,
    output_slot: u32,
    state_count: u32,
    control_mask: u32,
    control_value: u32,
    phase_lock_enabled: u32,
    total_qubits: u32,
};

@group(0) @binding(0) var<storage, read> state: array<vec2<f32>>;
@group(0) @binding(1) var<storage, read_write> amplitude_data: array<f32>;
@group(0) @binding(2) var<storage, read_write> amplitude_meta: array<vec4<f32>>;
@group(0) @binding(3) var<uniform> params: Params;

fn mag2(v: vec2<f32>) -> f32 {
    return dot(v, v);
}

fn insert_outcome(rest: u32, outcome: u32) -> u32 {
    let span_mask = (1u << params.span) - 1u;
    let low_mask = (1u << params.base_bit) - 1u;
    let low = rest & low_mask;
    let high = rest & ~low_mask;
    return (high << params.span) | ((outcome & span_mask) << params.base_bit) | low;
}

fn state_amp(rest: u32, outcome: u32) -> vec2<f32> {
    let idx = insert_outcome(rest, outcome);
    if (idx >= params.state_count) {
        return vec2<f32>(0.0, 0.0);
    }
    if ((idx & params.control_mask) != params.control_value) {
        return vec2<f32>(0.0, 0.0);
    }
    return state[idx];
}

fn cmul_conj_raw(raw: vec2<f32>, value: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(raw.x * value.x + raw.y * value.y,
                     raw.x * value.y - raw.y * value.x);
}

fn rotate_minus_theta(v: vec2<f32>, theta: f32) -> vec2<f32> {
    let c = cos(theta);
    let s = sin(theta);
    return vec2<f32>(v.x * c + v.y * s, v.y * c - v.x * s);
}

@compute @workgroup_size(1)
fn main() {
    let outcomes = 1u << params.span;
    let rest_count = params.state_count >> params.span;
    let base = params.output_slot * VALUES_PER_SLOT;
    let ket_base = base;
    let incoherent_base = base + 2u * MAX_OUTCOMES;

    var best_rest = 0u;
    var best_mag = -1.0f;
    var incoherent_unity = 0.0f;

    for (var k = 0u; k < outcomes; k = k + 1u) {
        amplitude_data[ket_base + 2u * k] = 0.0;
        amplitude_data[ket_base + 2u * k + 1u] = 0.0;
        amplitude_data[incoherent_base + k] = 0.0;
    }

    for (var rest = 0u; rest < rest_count; rest = rest + 1u) {
        var slice_mag = 0.0f;
        for (var outcome = 0u; outcome < outcomes; outcome = outcome + 1u) {
            let amp = state_amp(rest, outcome);
            let p = mag2(amp);
            slice_mag = slice_mag + p;
            amplitude_data[incoherent_base + outcome] = amplitude_data[incoherent_base + outcome] + p;
        }
        incoherent_unity = incoherent_unity + slice_mag;
        if (slice_mag > best_mag) {
            best_mag = slice_mag;
            best_rest = rest;
        }
    }

    var unity = 0.0f;
    for (var outcome = 0u; outcome < outcomes; outcome = outcome + 1u) {
        let amp = state_amp(best_rest, outcome);
        amplitude_data[ket_base + 2u * outcome] = amp.x;
        amplitude_data[ket_base + 2u * outcome + 1u] = amp.y;
        unity = unity + mag2(amp);
    }

    var denormalized_quality = 0.0f;
    for (var rest = 0u; rest < rest_count; rest = rest + 1u) {
        var dot_value = vec2<f32>(0.0, 0.0);
        for (var outcome = 0u; outcome < outcomes; outcome = outcome + 1u) {
            let raw = vec2<f32>(
                amplitude_data[ket_base + 2u * outcome],
                amplitude_data[ket_base + 2u * outcome + 1u]
            );
            let amp = state_amp(rest, outcome);
            dot_value = dot_value + cmul_conj_raw(raw, amp);
        }
        denormalized_quality = denormalized_quality + mag2(dot_value);
    }

    var phase_index = -1.0f;
    var theta = 0.0f;
    if (unity > 0.000000000001 && params.phase_lock_enabled == 1u) {
        var strongest = 0.0f;
        var strongest_index = 0u;
        for (var outcome = 0u; outcome < outcomes; outcome = outcome + 1u) {
            let raw = vec2<f32>(
                amplitude_data[ket_base + 2u * outcome],
                amplitude_data[ket_base + 2u * outcome + 1u]
            );
            let p = mag2(raw);
            if (p > strongest * 10000.0) {
                strongest = p;
                strongest_index = outcome;
            }
        }
        if (strongest > 0.00000001) {
            let lock = vec2<f32>(
                amplitude_data[ket_base + 2u * strongest_index],
                amplitude_data[ket_base + 2u * strongest_index + 1u]
            );
            theta = atan2(lock.y, lock.x);
            phase_index = f32(strongest_index);
        }
    }

    let unity_norm = inverseSqrt(max(unity, 0.000000000001));
    let incoherent_norm = inverseSqrt(max(incoherent_unity, 0.000000000001));
    for (var outcome = 0u; outcome < outcomes; outcome = outcome + 1u) {
        var raw = vec2<f32>(
            amplitude_data[ket_base + 2u * outcome],
            amplitude_data[ket_base + 2u * outcome + 1u]
        );
        raw = rotate_minus_theta(raw * unity_norm, theta);
        amplitude_data[ket_base + 2u * outcome] = raw.x;
        amplitude_data[ket_base + 2u * outcome + 1u] = raw.y;
        let incoherent_prob = amplitude_data[incoherent_base + outcome];
        amplitude_data[incoherent_base + outcome] = sqrt(max(incoherent_prob, 0.0)) * incoherent_norm;
    }

    var quality = 0.0f;
    if (unity > 0.000000000001 && incoherent_unity > 0.000000000001) {
        quality = denormalized_quality / (unity * incoherent_unity);
    }
    amplitude_meta[params.output_slot] = vec4<f32>(clamp(quality, 0.0, 1.0), phase_index, f32(params.span), 0.0);
}
"#;

pub(in crate::gpu) const AMPLITUDE_RENDER_SHADER: &str = r#"
const MAX_OUTCOMES: u32 = 65536u;
const VALUES_PER_SLOT: u32 = MAX_OUTCOMES * 3u;

struct RenderParams {
    viewport_min: vec2<f32>,
    viewport_size: vec2<f32>,
    background: vec4<f32>,
    border: vec4<f32>,
    disk: vec4<f32>,
    outline: vec4<f32>,
    outline_zero: vec4<f32>,
    needle: vec4<f32>,
    hover_border: vec4<f32>,
};

struct VertexOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) rect_min: vec2<f32>,
    @location(1) rect_size: vec2<f32>,
    @location(2) local: vec2<f32>,
    @location(3) @interpolate(flat) slot: u32,
    @location(4) @interpolate(flat) span: u32,
    @location(5) @interpolate(flat) hovered_outcome: i32,
};

@group(0) @binding(0) var<storage, read> amplitude_data: array<f32>;
@group(0) @binding(1) var<storage, read> amplitude_meta: array<vec4<f32>>;
@group(0) @binding(2) var<uniform> params: RenderParams;

fn grid_cols(span: u32) -> u32 {
    let outcomes = 1u << span;
    if (outcomes == 2u) {
        return 2u;
    }
    return 1u << (span / 2u);
}

fn sd_rect(p: vec2<f32>, half_size: vec2<f32>) -> f32 {
    let d = abs(p) - half_size;
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0);
}

fn blend_over(dst: vec4<f32>, src: vec4<f32>) -> vec4<f32> {
    let src_pre = vec4<f32>(src.rgb * src.a, src.a);
    return src_pre + dst * (1.0 - src_pre.a);
}

@vertex
fn vs_main(
    @location(0) unit: vec2<f32>,
    @location(1) rect_min: vec2<f32>,
    @location(2) rect_size: vec2<f32>,
    @location(3) slot: u32,
    @location(4) span: u32,
    @location(5) hovered_outcome: i32,
) -> VertexOut {
    let pixel = rect_min + (unit * 0.5 + vec2<f32>(0.5)) * rect_size;
    let ndc = ((pixel - params.viewport_min) / params.viewport_size) * 2.0 - vec2<f32>(1.0, 1.0);
    var out: VertexOut;
    out.pos = vec4<f32>(ndc.x, -ndc.y, 0.0, 1.0);
    out.rect_min = rect_min;
    out.rect_size = rect_size;
    out.local = pixel - rect_min;
    out.slot = slot;
    out.span = span;
    out.hovered_outcome = hovered_outcome;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let body_border = 1.0;
    var color = params.background;
    let aa_edge = length(fwidth(in.local));

    let outer_p = in.local - in.rect_size * 0.5;
    let outer_d = sd_rect(outer_p, in.rect_size * 0.5 - vec2<f32>(0.5));
    if (outer_d > 0.0) {
        discard;
    }
    if (outer_d > -body_border) {
        color = blend_over(color, params.border);
    }

    let cols = grid_cols(in.span);
    let outcomes = 1u << in.span;
    let rows = outcomes / cols;
    let cell = min(in.rect_size.x / f32(cols), in.rect_size.y / f32(rows));
    let grid_size = vec2<f32>(f32(cols) * cell, f32(rows) * cell);
    let grid_origin = (in.rect_size - grid_size) * 0.5;
    let grid_local = in.local - grid_origin;
    if (grid_local.x < 0.0 || grid_local.y < 0.0 || grid_local.x >= grid_size.x || grid_local.y >= grid_size.y) {
        return color;
    }

    let col = u32(clamp(floor(grid_local.x / cell), 0.0, f32(cols - 1u)));
    let row = u32(clamp(floor(grid_local.y / cell), 0.0, f32(rows - 1u)));
    let outcome = row * cols + col;
    let cell_local = grid_local - vec2<f32>(f32(col), f32(row)) * cell;
    let cell_centered = cell_local - vec2<f32>(cell * 0.5);
    let base = in.slot * VALUES_PER_SLOT;
    let amp_meta = amplitude_meta[in.slot];
    let incoherent = amp_meta.x < 0.99;

    var re = amplitude_data[base + 2u * outcome];
    var im = amplitude_data[base + 2u * outcome + 1u];
    var mag = sqrt(max(re * re + im * im, 0.0));
    if (incoherent) {
        mag = amplitude_data[base + 2u * MAX_OUTCOMES + outcome];
        re = mag;
        im = 0.0;
    }

    if (cell < 3.0) {
        let heat = params.disk * vec4<f32>(1.0, 1.0, 1.0, clamp(mag * mag, 0.0, 1.0));
        color = blend_over(color, heat);
    } else {
        let stroke = select(1.0, 2.0, cell >= 24.0);
        let half_stroke = stroke * 0.5;
        // docs/amplitude-display.html §05 pseudocode: r_outline =
        // cell_size / 2 - stroke. The 1px slack keeps the circle stroke
        // inside the rectangular matrix frame instead of sharing its edge.
        let outline_radius = max(0.0, cell * 0.5 - stroke);
        let inner_radius = max(0.0, outline_radius - half_stroke);
        let centered_len = length(cell_centered);
        // Keep derivative-based SDF AA, but tighten the transition for the
        // circuit body so placed Amplitude circles read sharper than the
        // palette icon while still avoiding hard jaggies.
        let edge = max(0.5, aa_edge * 0.65);

        if (cell >= 3.0) {
            let outline_inner = 1.0 - smoothstep(
                outline_radius - half_stroke - edge,
                outline_radius - half_stroke + edge,
                centered_len
            );
            let outline_outer = 1.0 - smoothstep(
                outline_radius + half_stroke - edge,
                outline_radius + half_stroke + edge,
                centered_len
            );
            let outline_alpha = max(0.0, outline_outer - outline_inner);
            if (outline_alpha > 0.001) {
                var outline = select(params.outline_zero, params.outline, mag > 0.000001);
                outline.a = outline.a * outline_alpha;
                color = blend_over(color, outline);
            }
        }

        let radius = inner_radius * mag;
        if (radius > 0.3) {
            let disk_alpha = 1.0 - smoothstep(radius - edge, radius + edge, centered_len);
            if (disk_alpha > 0.001) {
                var disk = params.disk;
                disk.a = disk.a * disk_alpha * select(1.0, 0.45, incoherent);
                color = blend_over(color, disk);
            }
        }

        if (!incoherent && cell >= 12.0 && mag > 0.001) {
            let angle = atan2(im, re);
            let dir = vec2<f32>(-sin(angle), -cos(angle));
            let along = clamp(dot(cell_centered, dir), 0.0, inner_radius);
            let closest = dir * along;
            let needle_dist = length(cell_centered - closest);
            let needle_alpha = 1.0 - smoothstep(half_stroke - edge, half_stroke + edge, needle_dist);
            if (needle_alpha > 0.001) {
                var needle = params.needle;
                needle.a = needle.a * needle_alpha;
                color = blend_over(color, needle);
            }
        }

        if (in.hovered_outcome == i32(outcome) && cell >= 3.0) {
            let hover_inner = 1.0 - smoothstep(
                outline_radius - half_stroke - edge,
                outline_radius - half_stroke + edge,
                centered_len
            );
            let hover_outer = 1.0 - smoothstep(
                outline_radius + half_stroke - edge,
                outline_radius + half_stroke + edge,
                centered_len
            );
            let hover_alpha = max(0.0, hover_outer - hover_inner);
            if (hover_alpha > 0.001) {
                var hover = params.hover_border;
                hover.a = hover.a * hover_alpha;
                color = blend_over(color, hover);
            }
        }
    }
    return color;
}
"#;
