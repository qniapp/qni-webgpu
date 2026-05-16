mod app_adapter;
mod client;
mod test_hooks;

pub(crate) use qni_egui_web_external_gpu_model::{
    format_gpu_duration, qiskit_run_payload, short_failure_label, unsupported_gate_from_message,
    ExternalGpuStatus, GpuFailure,
};

use super::{ExecMode, QniApp};
