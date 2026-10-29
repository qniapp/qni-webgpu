use std::sync::Arc;

use super::probability_distribution::ProbabilityDistribution;
use crate::colors::Colors;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ArithmeticParams {
    pub(crate) state_count: u32,
    pub(crate) target_shift: u32,
    pub(crate) target_mask: u32,
    pub(crate) amount: u32,
    pub(crate) control_mask: u32,
    pub(crate) control_value: u32,
    pub(crate) _pad: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct RenderParams {
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    pub(crate) panel_origin: [f32; 2],
    pub(crate) panel_size: [f32; 2],
    pub(crate) cell_pitch: f32,
    pub(crate) radius: f32,
    pub(crate) inner_radius: f32,
    pub(crate) stroke: f32,
    pub(crate) cols: u32,
    pub(crate) rows: u32,
    pub(crate) qubits: u32,
    pub(crate) hovered_cell: i32,
    pub(crate) surface: [f32; 4],
    pub(crate) fill: [f32; 4],
    pub(crate) disk_border: [f32; 4],
    pub(crate) outline: [f32; 4],
    pub(crate) outline_zero: [f32; 4],
    pub(crate) needle: [f32; 4],
}

#[derive(Clone, Copy)]
pub(crate) struct RenderColors {
    pub(crate) surface: [f32; 4],
    pub(crate) fill: [f32; 4],
    pub(crate) disk_border: [f32; 4],
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
            disk_border: colors.amplitude_disk_border.to_normalized_gamma_f32(),
            outline: colors.state_outline.to_normalized_gamma_f32(),
            outline_zero: colors.state_outline_zero.to_normalized_gamma_f32(),
            needle: colors.state_needle.to_normalized_gamma_f32(),
        }
    }
}

pub(crate) const STATE_WORKGROUP_SIZE: u32 = 64;

pub(crate) const MAX_OPS_PER_RECOMPUTE: usize = 256;
/// Maximum number of semantic step snapshots cached on the GPU for hover /
/// breakpoint preview. `+ 1` lets the existing 257-gate capacity test report
/// the gate-op limit first while still bounding sparse URL columns before any
/// snapshot buffer allocation.
pub(crate) const MAX_STEP_SNAPSHOT_SLOTS: usize = MAX_OPS_PER_RECOMPUTE + 1;

pub(crate) const MAX_BLOCH_SLOTS: usize = 64;

pub(crate) const MAX_MEASUREMENT_SLOTS: usize = 64;

pub(crate) const MAX_PROBABILITY_SLOTS: usize = 32;
pub(crate) const MAX_PROBABILITY_OUTCOMES: usize = 1 << 16;
pub(crate) const MAX_PROBABILITY_AGGREGATE_ROWS: usize = 1024;
pub(crate) const PROBABILITY_AGGREGATE_MIN_SPAN: u32 = 13;
pub(crate) const PROBABILITY_RENDER_MODE_SAMPLE: u32 = 0;
pub(crate) const PROBABILITY_RENDER_MODE_PLACEHOLDER: u32 = 1;

#[derive(Clone)]
pub(crate) struct ExternalProbabilityUpload {
    pub(crate) slot: u32,
    pub(crate) probabilities: ProbabilityDistribution,
}

#[derive(Clone)]
pub(crate) struct ExternalProbabilityUploadBatch {
    pub(crate) generation: u64,
    pub(crate) slot_to_gate_id: Arc<[u32]>,
    pub(crate) uploads: Arc<[ExternalProbabilityUpload]>,
}

impl ExternalProbabilityUploadBatch {
    pub(crate) fn slot_for_gate(&self, gate_id: u32) -> Option<u32> {
        self.slot_to_gate_id
            .iter()
            .position(|id| *id == gate_id)
            .map(|slot| slot as u32)
    }
}

pub(crate) const MAX_AMPLITUDE_SLOTS: usize = 32;
pub(crate) const MAX_AMPLITUDE_OUTCOMES: usize = 1 << 16;
pub(crate) const AMPLITUDE_VALUES_PER_SLOT: usize = MAX_AMPLITUDE_OUTCOMES * 3;

pub(crate) const MAX_DENSITY_SLOTS: usize = 16;
pub(crate) const MAX_DENSITY_SPAN: usize = 8;
pub(crate) const MAX_DENSITY_DIM: usize = 1 << MAX_DENSITY_SPAN;
pub(crate) const DENSITY_VALUES_PER_SLOT: usize = MAX_DENSITY_DIM * MAX_DENSITY_DIM;

#[derive(Clone)]
pub(crate) struct ExternalDensityUpload {
    pub(crate) slot: u32,
    pub(crate) cells: Arc<[f32]>,
    pub(crate) meta: [f32; 4],
}

#[derive(Clone)]
pub(crate) struct ExternalDensityUploadBatch {
    pub(crate) generation: u64,
    pub(crate) slot_to_gate_id: Arc<[u32]>,
    pub(crate) uploads: Arc<[ExternalDensityUpload]>,
}

impl ExternalDensityUploadBatch {
    pub(crate) fn slot_for_gate(&self, gate_id: u32) -> Option<u32> {
        self.slot_to_gate_id
            .iter()
            .position(|id| *id == gate_id)
            .map(|slot| slot as u32)
    }
}

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
    pub(crate) control_mask: u32,
    pub(crate) control_value: u32,
    pub(crate) _pad: [u32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct MeasureReduceParams {
    pub(crate) qubit_bit: u32,
    pub(crate) state_count: u32,
    pub(crate) output_slot: u32,
    pub(crate) seed: u32,
    pub(crate) control_mask: u32,
    pub(crate) control_value: u32,
    pub(crate) _pad: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct MeasureCollapseParams {
    pub(crate) qubit_bit: u32,
    pub(crate) state_count: u32,
    pub(crate) aux_slot: u32,
    pub(crate) _pad: u32,
    pub(crate) control_mask: u32,
    pub(crate) control_value: u32,
    pub(crate) _pad_controls: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ProbabilityReduceParams {
    pub(crate) base_bit: u32,
    pub(crate) span: u32,
    pub(crate) rest_count: u32,
    pub(crate) output_slot: u32,
    pub(crate) control_mask: u32,
    pub(crate) control_value: u32,
    pub(crate) _pad: [u32; 2],
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
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct DensityCaptureParams {
    pub(crate) base_bit: u32,
    pub(crate) span: u32,
    pub(crate) output_slot: u32,
    pub(crate) state_count: u32,
    pub(crate) control_mask: u32,
    pub(crate) control_value: u32,
    pub(crate) _pad: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct DensityRenderParams {
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    pub(crate) background: [f32; 4],
    pub(crate) drag_background: [f32; 4],
    pub(crate) border: [f32; 4],
    pub(crate) disk: [f32; 4],
    pub(crate) disk_border: [f32; 4],
    pub(crate) outline: [f32; 4],
    pub(crate) outline_zero: [f32; 4],
    pub(crate) needle: [f32; 4],
    pub(crate) hover_border: [f32; 4],
    pub(crate) placeholder_background: [f32; 4],
}

pub(crate) const DENSITY_RENDER_MODE_SAMPLE: u32 = 0;
pub(crate) const DENSITY_RENDER_MODE_PLACEHOLDER: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct DensityInstance {
    pub(crate) rect_min: [f32; 2],
    pub(crate) rect_size: [f32; 2],
    pub(crate) slot: u32,
    pub(crate) span: u32,
    pub(crate) hovered_cell: i32,
    pub(crate) use_drag_background: u32,
    pub(crate) render_mode: u32,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct AmplitudeRenderParams {
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    pub(crate) background: [f32; 4],
    pub(crate) drag_background: [f32; 4],
    pub(crate) border: [f32; 4],
    pub(crate) disk: [f32; 4],
    pub(crate) disk_border: [f32; 4],
    pub(crate) outline: [f32; 4],
    pub(crate) outline_zero: [f32; 4],
    pub(crate) needle: [f32; 4],
    pub(crate) hover_border: [f32; 4],
    pub(crate) placeholder_background: [f32; 4],
}

pub(crate) const AMPLITUDE_FORCE_NONE: u32 = 0;
pub(crate) const AMPLITUDE_FORCE_ZERO: u32 = 1;
pub(crate) const AMPLITUDE_FORCE_PLACEHOLDER: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct AmplitudeInstance {
    pub(crate) rect_min: [f32; 2],
    pub(crate) rect_size: [f32; 2],
    pub(crate) slot: u32,
    pub(crate) span: u32,
    pub(crate) hovered_outcome: i32,
    pub(crate) use_drag_background: u32,
    pub(crate) force_zero_amplitude: u32,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct AmplitudePopupValueParams {
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    pub(crate) value_anchor: [f32; 2],
    pub(crate) row_pitch: f32,
    pub(crate) _pad_row: f32,
    pub(crate) char_size: [f32; 2],
    pub(crate) _pad_char: [f32; 2],
    pub(crate) text_color: [f32; 4],
    pub(crate) muted_text_color: [f32; 4],
    pub(crate) slot: u32,
    pub(crate) outcome: u32,
    pub(crate) _pad: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ProbabilityRenderParams {
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    /// Device pixels per logical point. The fragment shader maps its framebuffer
    /// `@builtin(position)` back to a logical row so the outcome row is decided
    /// from the (seam-stable) physical pixel centre rather than the interpolated
    /// `local.y`, which is not bit-exact across the unit quad's triangle seam.
    pub(crate) pixels_per_point: f32,
    pub(crate) _pad_ppp: [f32; 3],
    pub(crate) background: [f32; 4],
    pub(crate) placeholder_background: [f32; 4],
    pub(crate) border: [f32; 4],
    pub(crate) log_hint: [f32; 4],
    pub(crate) bar: [f32; 4],
    pub(crate) sample_on_core: [f32; 4],
    pub(crate) sample_on_fill: [f32; 4],
    pub(crate) sample_on_glow: [f32; 4],
    pub(crate) sample_off_fill: [f32; 4],
    pub(crate) sample_off_edge: [f32; 4],
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
    pub(crate) render_mode: u32,
    pub(crate) seed: u32,
    pub(crate) _pad: u32,
}

impl ProbabilityInstance {
    pub(crate) const SAMPLED_BITSTRING: u32 = 2;
    pub(crate) const SAMPLE_PREVIEW: u32 = 3;
    pub(crate) const SAMPLE_GLOW_PAD: f32 = crate::tailwind::SPACING_2;
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ProbabilityPopupValueParams {
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    pub(crate) value_anchor: [f32; 2],
    pub(crate) row_pitch: f32,
    pub(crate) _pad_row: f32,
    pub(crate) char_size: [f32; 2],
    pub(crate) _pad_char: [f32; 2],
    pub(crate) text_color: [f32; 4],
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
    pub(crate) tip_0_color: [f32; 4],
    pub(crate) tip_mid_color: [f32; 4],
    pub(crate) tip_1_color: [f32; 4],
    pub(crate) tip_outline_color: [f32; 4],
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
pub(crate) struct BlochPopupValueParams {
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    pub(crate) value_anchor: [f32; 2],
    pub(crate) col_pitch: f32,
    pub(crate) row_pitch: f32,
    pub(crate) char_size: [f32; 2],
    pub(crate) _pad_char: [f32; 2],
    pub(crate) text_color: [f32; 4],
    pub(crate) slot: u32,
    pub(crate) _pad: [u32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct MeasurementDigitParams {
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    pub(crate) zero_color: [f32; 4],
    pub(crate) one_color: [f32; 4],
    pub(crate) grabbed_color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct MeasurementDigitInstance {
    pub(crate) center: [f32; 2],
    pub(crate) half_extent: f32,
    pub(crate) slot: u32,
    pub(crate) grabbed: u32,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct SurvivalLabelParams {
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    pub(crate) zero_color: [f32; 4],
    pub(crate) one_color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct SurvivalLabelInstance {
    pub(crate) center: [f32; 2],
    pub(crate) half_extent: f32,
    pub(crate) first_slot: u32,
    pub(crate) last_slot: u32,
    pub(crate) mode: u32,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ConditionalGateBodyParams {
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    pub(crate) enabled_color: [f32; 4],
    pub(crate) disabled_color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ConditionalGateBodyInstance {
    pub(crate) center: [f32; 2],
    pub(crate) half_size: f32,
    pub(crate) corner_radius: f32,
    pub(crate) slot: u32,
}

pub(crate) const MAX_CONDITIONAL_GATE_BODIES: usize = MAX_OPS_PER_RECOMPUTE;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct PopupValueParams {
    pub(crate) viewport_min: [f32; 2],
    pub(crate) viewport_size: [f32; 2],
    pub(crate) value_anchor: [f32; 2],
    pub(crate) row_pitch: f32,
    pub(crate) _pad_row: f32,
    pub(crate) char_size: [f32; 2],
    pub(crate) _pad_char: [f32; 2],
    pub(crate) text_color: [f32; 4],
    pub(crate) hovered_display_index: u32,
    pub(crate) qubits: u32,
    pub(crate) _pad: [u32; 2],
}

#[cfg(test)]
mod sample_display_tests {
    use super::ProbabilityInstance;

    #[test]
    fn probability_instance_matches_storage_stride() {
        assert_eq!(std::mem::size_of::<ProbabilityInstance>(), 40);
    }

    #[test]
    fn sample_glow_pad_matches_shader() {
        let declaration = format!(
            "const SAMPLE_GLOW_PAD: f32 = {:.1};",
            ProbabilityInstance::SAMPLE_GLOW_PAD
        );
        assert!(include_str!("shaders/probability_render.wgsl").contains(&declaration));
    }
}

pub(crate) const MAX_POSTSELECTION_SLOTS: usize = 256;
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct PostselectionParams {
    pub(crate) state_count: u32,
    pub(crate) output_slot: u32,
    pub(crate) _pad: [u32; 2],
}
