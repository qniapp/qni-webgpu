mod amplitude_display;
mod arithmetic;
mod bloch_display;
mod common;
mod conditional_gate_body;
mod density_matrix_display;
mod digit;
mod measure;
mod popup_value;
mod postselection;
mod probability_display;
mod state;
mod state_status;

use eframe::wgpu;

use self::amplitude_display::AmplitudeResources;
use self::bloch_display::BlochResources;
use self::common::Common;
use self::conditional_gate_body::ConditionalGateBodyResources;
use self::density_matrix_display::DensityResources;
use self::digit::DigitResources;
use self::measure::MeasureResources;
use self::popup_value::PopupValueResources;
use self::probability_display::ProbabilityResources;
use self::state::StateResources;

pub(crate) struct StateVectorResources {
    pub(crate) common: Common,
    pub(crate) state_status: state_status::StateStatusResources,
    pub(crate) state: StateResources,
    pub(crate) arithmetic: arithmetic::ArithmeticResources,
    pub(crate) bloch: BlochResources,
    pub(crate) measure: MeasureResources,
    pub(crate) postselection: postselection::PostselectionResources,
    pub(crate) probability: ProbabilityResources,
    pub(crate) amplitude: AmplitudeResources,
    pub(crate) density: DensityResources,
    pub(crate) digit: DigitResources,
    pub(crate) conditional_gate_body: ConditionalGateBodyResources,
    pub(crate) popup_value: PopupValueResources,

    pub(crate) target_format: wgpu::TextureFormat,
    pub(crate) state_count: usize,
    pub(crate) final_state: usize,
    pub(crate) active_state: usize,
    pub(crate) last_preview_step: Option<usize>,
    pub(crate) plan_revision: Option<u64>,
}

impl StateVectorResources {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
    ) -> Self {
        let common = Common::build(device);
        let state_status = state_status::StateStatusResources::build(device, &common);
        let arithmetic = arithmetic::ArithmeticResources::build(device, &common);
        let measure = MeasureResources::build(device, &common);
        let postselection =
            postselection::PostselectionResources::build(device, queue, target_format, &common);
        let state = StateResources::build(device, target_format, &common, &measure.aux_buffer);
        let bloch = BlochResources::build(device, queue, target_format, &common);
        let probability = ProbabilityResources::build(device, queue, target_format, &common);
        let amplitude = AmplitudeResources::build(device, queue, target_format, &common);
        let density = DensityResources::build(device, target_format, &common);
        let digit = DigitResources::build(device, queue, target_format, &measure);
        let conditional_gate_body =
            ConditionalGateBodyResources::build(device, target_format, &measure);
        let popup_value = PopupValueResources::build(device, queue, target_format, &common);

        Self {
            common,
            state_status,
            state,
            arithmetic,
            bloch,
            measure,
            postselection,
            probability,
            amplitude,
            density,
            digit,
            conditional_gate_body,
            popup_value,
            target_format,
            state_count: 0,
            final_state: 0,
            active_state: 0,
            last_preview_step: None,
            plan_revision: None,
        }
    }

    pub(crate) fn update_target_format(
        &mut self,
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
    ) {
        if self.target_format == target_format {
            return;
        }
        self.state.update_target_format(device, target_format);
        self.postselection
            .update_target_format(device, target_format);
        self.bloch.update_target_format(device, target_format);
        self.probability.update_target_format(device, target_format);
        self.amplitude.update_target_format(device, target_format);
        self.density.update_target_format(device, target_format);
        self.digit.update_target_format(device, target_format);
        self.conditional_gate_body
            .update_target_format(device, target_format);
        self.popup_value.update_target_format(device, target_format);
        self.target_format = target_format;
    }
}
