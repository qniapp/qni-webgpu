mod amplitude;
mod app_adapter;
mod bloch;
mod density;
mod probability;
mod session;
mod test_hooks;
mod transport;

pub(crate) use session::{ExternalGpuSession, Invalidation};

#[cfg(all(test, not(target_arch = "wasm32")))]
mod parser_fixtures;

pub(crate) use qni_web_external_gpu_model::{
    format_gpu_duration, qiskit_run_payload_with_display_outputs, ExternalGpuStatus, GpuFailure,
    Shots,
};
#[cfg(target_arch = "wasm32")]
pub(crate) use qni_web_external_gpu_model::{short_failure_label, unsupported_gate_from_message};

use super::{ExecMode, QniApp};

#[cfg(any(target_arch = "wasm32", test))]
fn display_defined_marker(defined: Option<bool>) -> f32 {
    if defined == Some(false) {
        -1.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod defined_tests {
    #[test]
    fn external_gpu_defined_marker_preserves_undefined_state() {
        assert_eq!(
            [Some(false), Some(true), None].map(super::display_defined_marker),
            [-1.0, 0.0, 0.0]
        );
    }
}
