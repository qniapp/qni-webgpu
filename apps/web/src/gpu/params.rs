//! Plain-old-data types and capacity constants.
//!
//! All `#[repr(C)]` structs the GPU pipelines read (uniforms, instance
//! buffers, vertex layouts) and the per-feature capacity constants live
//! here. No shader strings, no buffer creation, no render logic — just
//! the data shapes shared across `resources.rs` and `callbacks.rs`.

use std::sync::Arc;

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
/// Maximum number of semantic step snapshots cached on the GPU for hover /
/// breakpoint preview. `+ 1` lets the existing 257-gate capacity test report
/// the gate-op limit first while still bounding sparse URL columns before any
/// snapshot buffer allocation.
pub(crate) const MAX_STEP_SNAPSHOT_SLOTS: usize = MAX_OPS_PER_RECOMPUTE + 1;

/// Maximum number of Bloch displays whose vectors can be captured in a single
/// recompute. Each placed `BlochDisplay` occupies one slot in the GPU's
/// `bloch_output_buffer` (a vec4 per slot, .xyz used).
pub(crate) const MAX_BLOCH_SLOTS: usize = 64;

/// Maximum number of measurement gates whose outcomes can be captured in a
/// single recompute. Each placed `Measurement` occupies one slot in the GPU's
/// `measurement_aux_buffer` (a vec4 per slot — pZero, r, outcome, √p_kept).
pub(crate) const MAX_MEASUREMENT_SLOTS: usize = 64;

/// Maximum Probability display slots whose probability distributions can be
/// captured in a single recompute. Each slot reserves `2^16` f32 values so a
/// Probability16 can render every outcome without reallocating GPU buffers.
pub(crate) const MAX_PROBABILITY_SLOTS: usize = 32;
pub(crate) const MAX_PROBABILITY_OUTCOMES: usize = 1 << 16;
/// Maximum CSS-pixel height of a Probability display body. Probability16 is currently
/// `(16 - 1) * LINE_GAP + GATE_SIZE = 880px`; 1024 leaves room for style
/// tweaks while keeping the aggregate buffer small.
pub(crate) const MAX_PROBABILITY_AGGREGATE_ROWS: usize = 1024;
pub(crate) const PROBABILITY_AGGREGATE_MIN_SPAN: u32 = 13;

/// Maximum Amplitude display slots whose complex matrices can be captured in
/// a single recompute. Each slot reserves coherent (re/im) and incoherent
/// magnitude values for `2^16` outcomes.
pub(crate) const MAX_AMPLITUDE_SLOTS: usize = 32;
pub(crate) const MAX_AMPLITUDE_OUTCOMES: usize = 1 << 16;
pub(crate) const AMPLITUDE_VALUES_PER_SLOT: usize = MAX_AMPLITUDE_OUTCOMES * 3;

#[derive(Clone)]
pub(crate) struct ExternalBlochUpload {
    pub(crate) slot: u32,
    pub(crate) vector: [f32; 4],
}

#[derive(Clone)]
pub(crate) struct ExternalBlochUploadBatch {
    pub(crate) generation: u64,
    pub(crate) slot_to_gate_id: Arc<[u32]>,
    pub(crate) uploads: Arc<[ExternalBlochUpload]>,
}

impl ExternalBlochUploadBatch {
    pub(crate) fn slot_for_gate(&self, gate_id: u32) -> Option<u32> {
        self.slot_to_gate_id
            .iter()
            .position(|id| *id == gate_id)
            .map(|slot| slot as u32)
    }
}

#[derive(Clone)]
pub(crate) struct ExternalAmplitudeUpload {
    pub(crate) slot: u32,
    pub(crate) coherent: Arc<[f32]>,
    pub(crate) incoherent: Arc<[f32]>,
    pub(crate) meta: [f32; 4],
}

#[derive(Clone)]
pub(crate) struct ExternalAmplitudeUploadBatch {
    pub(crate) generation: u64,
    pub(crate) slot_to_gate_id: Arc<[u32]>,
    pub(crate) uploads: Arc<[ExternalAmplitudeUpload]>,
}

impl ExternalAmplitudeUploadBatch {
    pub(crate) fn slot_for_gate(&self, gate_id: u32) -> Option<u32> {
        self.slot_to_gate_id
            .iter()
            .position(|id| *id == gate_id)
            .map(|slot| slot as u32)
    }
}

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
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ProbabilityReduceParams {
    /// Lowest state-vector bit covered by the contiguous Probability span.
    pub(crate) base_bit: u32,
    pub(crate) span: u32,
    pub(crate) rest_count: u32,
    pub(crate) output_slot: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct AmplitudeCaptureParams {
    pub(crate) base_bit: u32,
    pub(crate) span: u32,
    pub(crate) output_slot: u32,
    pub(crate) state_count: u32,
    pub(crate) control_mask: u32,
    pub(crate) control_value: u32,
    pub(crate) phase_lock_enabled: u32,
    pub(crate) total_qubits: u32,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct AmplitudeRenderParams {
    /// Egui callback viewport — see `BlochOverlayParams::viewport_min`.
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    /// Flexoki bg / paper #FFFCF0 cell background.
    pub(crate) background: [f32; 4],
    /// Flexoki purple-600 #5E409D dragged cell background.
    pub(crate) drag_background: [f32; 4],
    /// Flexoki ui-2 #DAD8CE grid / outer border.
    pub(crate) border: [f32; 4],
    /// Flexoki blue-200 #92BFDB amplitude disk.
    pub(crate) disk: [f32; 4],
    /// Flexoki blue-400 #4385BE amplitude disk inset border.
    pub(crate) disk_border: [f32; 4],
    /// Flexoki tx-2 #6F6E69 coherent non-zero outline.
    pub(crate) outline: [f32; 4],
    /// Flexoki ui-2 #DAD8CE zero outline.
    pub(crate) outline_zero: [f32; 4],
    /// Flexoki tx #100F0F phase needle.
    pub(crate) needle: [f32; 4],
    /// Flexoki purple-400 #8B7EC8 hovered cell outline.
    pub(crate) hover_border: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct AmplitudeInstance {
    pub(crate) rect_min: [f32; 2],
    pub(crate) rect_size: [f32; 2],
    pub(crate) slot: u32,
    pub(crate) span: u32,
    pub(crate) hovered_outcome: i32,
    pub(crate) use_drag_background: u32,
    /// Draw the exact Amplitude display chrome with zero amplitudes instead
    /// of sampling a captured slot. Used only for the unsnapped Amps1 drag
    /// preview so it matches the live GPU display without CPU amplitude math.
    pub(crate) force_zero_amplitude: u32,
}

/// Uniform for the Amplitude hover popup value text shader. The shader reads
/// coherent/incoherent values directly from the Amplitude storage buffers.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct AmplitudePopupValueParams {
    /// Egui callback viewport — see `BlochOverlayParams::viewport_min`.
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    /// Top-left of the row-0 value text in egui pixels.
    pub(crate) value_anchor: [f32; 2],
    /// Vertical pitch between value rows.
    pub(crate) row_pitch: f32,
    pub(crate) _pad_row: f32,
    /// Size of one atlas character cell on screen (egui pixels).
    pub(crate) char_size: [f32; 2],
    pub(crate) _pad_char: [f32; 2],
    /// RGBA text colour (premultiplied at sample time).
    pub(crate) text_color: [f32; 4],
    /// Amplitude output slot and hovered outcome cell.
    pub(crate) slot: u32,
    pub(crate) outcome: u32,
    pub(crate) _pad: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ProbabilityRenderParams {
    /// Egui callback viewport — see `BlochOverlayParams::viewport_min`.
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    pub(crate) background: [f32; 4],
    /// Flexoki ui-2 #DAD8CE outer border / row separators.
    pub(crate) border: [f32; 4],
    /// Flexoki tx-3 #B7B5AC logarithm hints for Probability5..16.
    pub(crate) log_hint: [f32; 4],
    pub(crate) bar: [f32; 4],
    pub(crate) bar_edge: [f32; 4],
    pub(crate) hover_border: [f32; 4],
    pub(crate) text_color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ProbabilityInstance {
    pub(crate) rect_min: [f32; 2],
    pub(crate) rect_size: [f32; 2],
    pub(crate) slot: u32,
    pub(crate) span: u32,
    pub(crate) hovered_outcome: i32,
    pub(crate) _pad: u32,
}

/// Uniform for the Probability hover popup value text shader. The shader reads
/// raw / log probability values directly from `probability_output`.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ProbabilityPopupValueParams {
    /// Egui callback viewport — see `BlochOverlayParams::viewport_min`.
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    /// Top-left of the row-0 (`raw`) value text in egui pixels.
    pub(crate) value_anchor: [f32; 2],
    /// Vertical pitch between `raw` and `log` rows.
    pub(crate) row_pitch: f32,
    /// Pad to vec2 alignment.
    pub(crate) _pad_row: f32,
    /// Size of one atlas character cell on screen (egui pixels).
    pub(crate) char_size: [f32; 2],
    /// Padding before the next vec4-aligned field (`text_color`).
    pub(crate) _pad_char: [f32; 2],
    /// RGBA text colour (premultiplied at sample time).
    pub(crate) text_color: [f32; 4],
    /// Probability output slot and hovered outcome row.
    pub(crate) slot: u32,
    pub(crate) outcome: u32,
    pub(crate) _pad: [u32; 2],
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
