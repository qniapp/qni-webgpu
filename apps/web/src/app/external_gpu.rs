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
    format_gpu_duration, qiskit_run_payload_with_display_outputs, short_failure_label,
    unsupported_gate_from_message, ExternalGpuStatus, GpuFailure, Shots,
};

use super::{ExecMode, QniApp};
