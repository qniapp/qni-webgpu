//! Plain-old-data types and capacity constants.
//!
//! All `#[repr(C)]` structs the GPU pipelines read (uniforms, instance
//! buffers, vertex layouts) and the per-feature capacity constants live
//! here. No shader strings, no buffer creation, no render logic — just
//! the data shapes shared across `resources.rs` and `callbacks.rs`.

use crate::colors::Colors;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct RenderParams {
    /// See `BlochOverlayParams::viewport_min`. NDC -1..1 maps to the egui
    /// callback viewport, not the full canvas.
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    /// Top-left of the state-circle grid in egui pixels — the unit quad
    /// gets stretched from here over `panel_size`.
    pub(crate) panel_origin: [f32; 2],
    /// Total grid extent (`cols * cell_pitch`, `rows * cell_pitch`).
    pub(crate) panel_size: [f32; 2],
    /// Distance between adjacent cell origins (`size + gap`). Used in the
    /// fragment shader to map a panel-local pixel to its `(col, row)` cell.
    pub(crate) cell_pitch: f32,
    pub(crate) radius: f32,
    pub(crate) inner_radius: f32,
    pub(crate) stroke: f32,
    pub(crate) cols: u32,
    pub(crate) rows: u32,
    /// Number of qubits. The fragment shader bit-reverses the display index
    /// over `qubits` bits to produce the state-vector index — qni's
    /// row-major-to-state-vector mapping.
    pub(crate) qubits: u32,
    /// Cell display-index (= `row * cols + col`) the pointer is currently
    /// hovering over, or `-1` for "no hover". The fragment shader
    /// multiplies fill / needle / outline RGB by 0.9 on the matching cell
    /// — qni's `:host(:hover) #border { filter: brightness(0.9) }` style
    /// extended to all three layers.
    pub(crate) hovered_cell: i32,
    pub(crate) surface: [f32; 4],
    pub(crate) fill: [f32; 4],
    pub(crate) outline: [f32; 4],
    pub(crate) outline_zero: [f32; 4],
    pub(crate) needle: [f32; 4],
}

#[derive(Clone, Copy)]
pub(crate) struct RenderColors {
    pub(crate) surface: [f32; 4],
    pub(crate) fill: [f32; 4],
    pub(crate) outline: [f32; 4],
    pub(crate) outline_zero: [f32; 4],
    pub(crate) needle: [f32; 4],
}

impl RenderColors {
    pub(crate) fn new(colors: &Colors) -> Self {
        // The framebuffer is rgba8unorm (non-sRGB), so egui paints sRGB
        // Color32 bytes straight in. Match that with
        // `to_normalized_gamma_f32` here — `Rgba::from(Color32)` would
        // convert to linear and the GPU-rendered circles would come out
        // visibly darker than the egui-painted handle / palette colours.
        Self {
            surface: colors.surface.to_normalized_gamma_f32(),
            fill: colors.state_fill.to_normalized_gamma_f32(),
            outline: colors.state_outline.to_normalized_gamma_f32(),
            outline_zero: colors.state_outline_zero.to_normalized_gamma_f32(),
            needle: colors.state_needle.to_normalized_gamma_f32(),
        }
    }
}

pub(crate) const STATE_WORKGROUP_SIZE: u32 = 64;

/// Upper bound on the number of `SimulationOp`s of any single variant we can
/// batch into one recompute encoder. Issue A's fix packs all per-op params
/// into staging buffers up-front, then issues `copy_buffer_to_buffer` from
/// staging slots into the existing small uniform buffers between dispatches
/// inside a single encoder. Each variant has its own staging buffer sized to
/// `MAX_OPS_PER_RECOMPUTE` slots; capacity is validated before packing so
/// release builds fail explicitly instead of truncating the plan.
pub(crate) const MAX_OPS_PER_RECOMPUTE: usize = 256;

/// Maximum number of Bloch displays whose vectors can be captured in a single
/// recompute. Each placed `BlochDisplay` occupies one slot in the GPU's
/// `bloch_output_buffer` (a vec4 per slot, .xyz used).
pub(crate) const MAX_BLOCH_SLOTS: usize = 64;

/// Maximum number of measurement gates whose outcomes can be captured in a
/// single recompute. Each placed `Measurement` occupies one slot in the GPU's
/// `measurement_aux_buffer` (a vec4 per slot — pZero, r, outcome, √p_kept).
pub(crate) const MAX_MEASUREMENT_SLOTS: usize = 64;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct BlochParams {
    pub(crate) qubit_bit: u32,
    pub(crate) state_count: u32,
    pub(crate) output_slot: u32,
    pub(crate) _pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct MeasureReduceParams {
    pub(crate) qubit_bit: u32,
    pub(crate) state_count: u32,
    pub(crate) output_slot: u32,
    pub(crate) seed: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct MeasureCollapseParams {
    pub(crate) qubit_bit: u32,
    pub(crate) state_count: u32,
    pub(crate) aux_slot: u32,
    pub(crate) _pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct BlochOverlayParams {
    /// Egui callback viewport in CSS pixels (= the rect we passed to
    /// `Callback::new_paint_callback`). NDC -1..1 maps to this viewport,
    /// NOT to the full canvas — using `screen_descriptor.size_in_pixels`
    /// here would shift everything by `viewport_min`.
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    pub(crate) line_color: [f32; 4],
    pub(crate) tip_color: [f32; 4],
    pub(crate) zero_color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct BlochOverlayInstance {
    pub(crate) center: [f32; 2],
    pub(crate) radius: f32,
    pub(crate) outer: f32,
    pub(crate) slot: u32,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct MeasurementDigitParams {
    /// See `BlochOverlayParams::viewport_min` — same NDC story.
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    pub(crate) zero_color: [f32; 4],
    pub(crate) one_color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct MeasurementDigitInstance {
    pub(crate) center: [f32; 2],
    pub(crate) half_extent: f32,
    pub(crate) slot: u32,
}

/// Uniform for the state-cell hover popup value text shader
/// (`POPUP_VALUE_SHADER`). Layout matches the WGSL `PopupParams` struct
/// byte-for-byte — implicit WGSL alignment padding is made explicit
/// here via `_pad_*` fields so `bytemuck::cast_slice` writes the exact
/// shape the shader expects.
///
/// Total size: 80 bytes (multiple of 16 for uniform-buffer alignment).
#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct PopupValueParams {
    /// Egui callback viewport — see `BlochOverlayParams::viewport_min`.
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    /// Top-left of the row-0 (amplitude) value text in egui pixels.
    pub(crate) value_anchor: [f32; 2],
    /// Vertical pitch between value rows (= ROW_H from popup geometry).
    pub(crate) row_pitch: f32,
    /// Pad to vec2 alignment.
    pub(crate) _pad_row: f32,
    /// Size of one atlas character cell on screen (egui pixels).
    pub(crate) char_size: [f32; 2],
    /// Padding before the next vec4-aligned field (`text_color`).
    pub(crate) _pad_char: [f32; 2],
    /// RGBA text colour (premultiplied at sample time).
    pub(crate) text_color: [f32; 4],
    /// Which state-vector cell the user is hovering — display-space
    /// index, the shader applies the bit-reverse to get the state-space
    /// index (same convention as `STATE_RENDER_SHADER`).
    pub(crate) hovered_display_index: u32,
    pub(crate) qubits: u32,
    pub(crate) _pad: [u32; 2],
}
