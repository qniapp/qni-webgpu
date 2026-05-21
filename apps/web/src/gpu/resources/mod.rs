//! `StateVectorResources` — the kitchen-sink GPU resource bag, split
//! by subsystem.
//!
//! The original `gpu/resources.rs` was a single 1391-line file that
//! held every pipeline, bind group, and buffer side by side. The
//! per-subsystem boundaries are now explicit:
//!
//! * [`common`] — shared state-vector ping-pong buffers + ground-state
//!   seed + the `[-1, 1]²` unit quad used by every render pipeline.
//! * [`state`]  — state-vector compute (gate dispatch) + render
//!   (state-panel grid).
//! * [`bloch_display`] — Bloch Display reduce (compute) + arrow overlay
//!   (render). Both share the bloch output buffer.
//! * [`measure`]— measurement reduce + collapse (compute only). Owns
//!   `aux_buffer` consumed by `digit`.
//! * [`probability_display`] — Probability Display marginalization (compute) +
//!   GPU-rendered bars. Both share the Probability output buffer.
//! * [`amplitude_display`] — Amplitude Display capture + GPU-rendered grid / popup.
//! * [`density_matrix_display`] — Density Matrix Display capture + GPU-rendered grid.
//! * [`digit`]  — measurement digit overlay render pipeline. Reads
//!   `measure::aux_buffer`.
//! * [`popup_value`] — popup numeric-row render pipeline. Reads the
//!   active `common::state_buffers`.
//!
//! ## Latent bug fixed by the split
//!
//! The pre-refactor `update_render_pipeline` only rebuilt the
//! state-vector render pipeline on a surface-format change; the bloch
//! overlay, Probability bars, measurement digit, and popup-value pipelines silently
//! stayed pinned to the original format. This module's
//! [`StateVectorResources::update_target_format`] now fans the call
//! out to every render-bearing subsystem.

mod amplitude_display;
mod bloch_display;
mod common;
mod density_matrix_display;
mod digit;
mod measure;
mod popup_value;
mod probability_display;
mod state;

use eframe::wgpu;

use self::amplitude_display::AmplitudeResources;
use self::bloch_display::BlochResources;
use self::common::Common;
use self::density_matrix_display::DensityResources;
use self::digit::DigitResources;
use self::measure::MeasureResources;
use self::popup_value::PopupValueResources;
use self::probability_display::ProbabilityResources;
use self::state::StateResources;

pub(crate) struct StateVectorResources {
    pub(crate) common: Common,
    pub(crate) state: StateResources,
    pub(crate) bloch: BlochResources,
    pub(crate) measure: MeasureResources,
    pub(crate) probability: ProbabilityResources,
    pub(crate) amplitude: AmplitudeResources,
    pub(crate) density: DensityResources,
    pub(crate) digit: DigitResources,
    pub(crate) popup_value: PopupValueResources,

    /// Surface format the render pipelines were last built for; used
    /// to short-circuit `update_target_format` on every frame.
    pub(crate) target_format: wgpu::TextureFormat,
    /// Number of states currently allocated (≤ MAX_STATE_COUNT). Owned
    /// here rather than inside a subsystem because every pipeline reads
    /// it indirectly.
    pub(crate) state_count: usize,
    /// Final-state ping-pong buffer index (0/1) produced by the latest full
    /// recompute. Used when no step preview is active.
    pub(crate) final_state: usize,
    /// Index into state render/readback bind groups. 0/1 are the ping-pong
    /// final-state buffers; 2 is the selected-step preview snapshot.
    pub(crate) active_state: usize,
    pub(crate) last_preview_step: Option<usize>,
}

impl StateVectorResources {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
    ) -> Self {
        let common = Common::build(device);
        let state = StateResources::build(device, target_format, &common);
        let bloch = BlochResources::build(device, target_format, &common);
        let measure = MeasureResources::build(device, &common);
        let probability = ProbabilityResources::build(device, queue, target_format, &common);
        let amplitude = AmplitudeResources::build(device, queue, target_format, &common);
        let density = DensityResources::build(device, target_format, &common);
        let digit = DigitResources::build(device, queue, target_format, &measure);
        let popup_value = PopupValueResources::build(device, queue, target_format, &common);

        Self {
            common,
            state,
            bloch,
            measure,
            probability,
            amplitude,
            density,
            digit,
            popup_value,
            target_format,
            state_count: 0,
            final_state: 0,
            active_state: 0,
            last_preview_step: None,
        }
    }

    /// Rebuild every render pipeline whose colour target depends on
    /// the surface format. No-op when the format hasn't changed.
    ///
    /// Pre-refactor name was `update_render_pipeline`, but it only
    /// rebuilt one pipeline. Now genuine — touches `state`, `bloch`,
    /// `probability`, `amplitude`, `density`, `digit`, and `popup_value`.
    pub(crate) fn update_target_format(
        &mut self,
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
    ) {
        if self.target_format == target_format {
            return;
        }
        self.state.update_target_format(device, target_format);
        self.bloch.update_target_format(device, target_format);
        self.probability.update_target_format(device, target_format);
        self.amplitude.update_target_format(device, target_format);
        self.density.update_target_format(device, target_format);
        self.digit.update_target_format(device, target_format);
        self.popup_value.update_target_format(device, target_format);
        self.target_format = target_format;
    }
}
