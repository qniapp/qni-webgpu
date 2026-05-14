use std::cell::RefCell;
use std::time::Duration;

use eframe::egui;

use super::{ExecMode, QniApp};
use crate::gates::GateKind;
use crate::shared::now_seconds;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) enum ExternalGpuStatus {
    #[default]
    Idle,
    Running,
    Completed {
        duration: Duration,
    },
    Failed(GpuFailure),
}

impl ExternalGpuStatus {
    pub(crate) fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum GpuFailure {
    BackendOffline { endpoint: String },
    UnsupportedGate(String),
    Http(u16),
    Other(String),
}

impl GpuFailure {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::BackendOffline { .. } => "Backend unreachable",
            Self::UnsupportedGate(_) => "Unsupported gate",
            Self::Http(_) => "Backend error",
            Self::Other(_) => "GPU failed",
        }
    }

    pub(crate) fn detail(&self) -> String {
        match self {
            Self::BackendOffline { endpoint } => endpoint.clone(),
            Self::UnsupportedGate(name) => name.clone(),
            Self::Http(status) => format!("HTTP {status}"),
            Self::Other(label) => label.clone(),
        }
    }
}

pub(crate) fn format_gpu_duration(duration: Duration) -> String {
    let millis = duration.as_millis();
    if millis < 1_000 {
        format!("{millis} ms")
    } else {
        format!("{:.1} s", duration.as_secs_f32())
    }
}

pub(crate) fn qiskit_run_payload(qubits: usize, columns_json: &str, shots: usize) -> String {
    format!(
        r#"{{"qubits":{qubits},"columns":{columns_json},"shots":{shots},"outputs":{{"histogram":true}}}}"#
    )
}

impl QniApp {
    pub(crate) fn external_gpu_status(&self) -> &ExternalGpuStatus {
        &self.external_gpu_status
    }

    pub(crate) fn poll_external_gpu_run(&mut self, ctx: &egui::Context) {
        if let Some(status) = take_external_gpu_status_override() {
            self.apply_external_gpu_status(status, ctx);
        }
        if let Some(result) = take_qiskit_run_result() {
            let status = match result {
                Ok(_message) => {
                    let duration = self.take_external_gpu_duration();
                    ExternalGpuStatus::Completed { duration }
                }
                Err(failure) => {
                    self.external_gpu_started_at = None;
                    ExternalGpuStatus::Failed(failure)
                }
            };
            self.apply_external_gpu_status(status, ctx);
        }
    }

    fn apply_external_gpu_status(&mut self, status: ExternalGpuStatus, ctx: &egui::Context) {
        match &status {
            ExternalGpuStatus::Idle | ExternalGpuStatus::Failed(_) => {
                self.external_gpu_started_at = None;
            }
            ExternalGpuStatus::Running => {
                if self.external_gpu_started_at.is_none() {
                    self.external_gpu_started_at = Some(now_seconds());
                }
            }
            ExternalGpuStatus::Completed { .. } => {
                self.external_gpu_started_at = None;
                self.request_external_gpu_state_panel_refresh();
            }
        }
        self.external_gpu_status = status;
        ctx.request_repaint();
    }

    fn take_external_gpu_duration(&mut self) -> Duration {
        let started_at = self
            .external_gpu_started_at
            .take()
            .unwrap_or_else(now_seconds);
        Duration::from_secs_f64((now_seconds() - started_at).max(0.0))
    }

    fn request_external_gpu_state_panel_refresh(&mut self) {
        if !self.local_exec_mode_available() {
            return;
        }
        if self.exec_mode == ExecMode::Gpu {
            self.external_gpu_state_refresh_pending = true;
        }
        self.gpu_plan.mark_dirty();
    }

    pub(crate) fn start_external_gpu_run(&mut self, ctx: &egui::Context) {
        if self.external_gpu_status.is_running() {
            return;
        }
        if let Some(gate_name) = self.unsupported_external_gpu_gate() {
            self.external_gpu_status =
                ExternalGpuStatus::Failed(GpuFailure::UnsupportedGate(gate_name.to_owned()));
            ctx.request_repaint();
            return;
        }

        let payload = self.external_gpu_run_payload();
        self.external_gpu_started_at = Some(now_seconds());
        match start_qiskit_run(payload, ctx.clone()) {
            Ok(()) => self.external_gpu_status = ExternalGpuStatus::Running,
            Err(failure) => {
                self.external_gpu_started_at = None;
                self.external_gpu_status = ExternalGpuStatus::Failed(failure);
            }
        }
        ctx.request_repaint();
    }

    fn unsupported_external_gpu_gate(&self) -> Option<&'static str> {
        for gate in &self.placed_gates {
            let name = match gate.kind {
                GateKind::AntiControl => Some("Anti-control"),
                GateKind::BlochDisplay => Some("Bloch"),
                GateKind::Measurement => Some("Measurement"),
                GateKind::Spacer => Some("Spacer"),
                GateKind::Write0 => Some("|0⟩"),
                GateKind::Write1 => Some("|1⟩"),
                GateKind::Swap => Some("Swap"),
                GateKind::QftGate => Some("QFT"),
                GateKind::QftDaggerGate => Some("QFT†"),
                _ => None,
            };
            if let Some(name) = name {
                return Some(name);
            }
        }

        let max_column = self.placed_gates.iter().map(|gate| gate.column).max()?;
        for column in 0..=max_column {
            let mut has_control = false;
            let mut has_x_target = false;
            for gate in self
                .placed_gates
                .iter()
                .filter(|gate| gate.column == column)
            {
                if gate.kind == GateKind::Control {
                    has_control = true;
                } else if gate.kind == GateKind::X {
                    has_x_target = true;
                } else if has_control {
                    return Some(gate.kind.label());
                }
            }
            if has_control && !has_x_target {
                return Some("Control");
            }
        }
        None
    }

    fn external_gpu_run_payload(&self) -> String {
        let qubits = self.external_execution_qubits();
        let columns_json = crate::url_circuit::circuit_columns_to_json(&self.placed_gates, qubits);
        qiskit_run_payload(qubits, &columns_json, 1024)
    }

    pub(crate) fn wire_test_hooks(ctx: &egui::Context) {
        wire_external_gpu_test_hooks(ctx);
        super::circuit_library::wire_test_hooks(ctx);
    }
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static QISKIT_RUN_RESULT: RefCell<Option<Result<String, GpuFailure>>> = const { RefCell::new(None) };
    static TEST_EXTERNAL_GPU_STATUS: RefCell<Option<ExternalGpuStatus>> = const { RefCell::new(None) };
}

#[cfg(target_arch = "wasm32")]
fn start_qiskit_run(payload: String, ctx: egui::Context) -> Result<(), GpuFailure> {
    use wasm_bindgen::JsCast;

    let window = web_sys::window().ok_or_else(|| GpuFailure::Other("window not found".into()))?;
    let endpoint = qiskit_backend_endpoint(&window);
    let function = js_sys::Reflect::get(
        window.as_ref(),
        &wasm_bindgen::JsValue::from_str("__qniRunQiskitBackend"),
    )
    .map_err(|_| GpuFailure::Other("backend helper lookup failed".into()))?
    .dyn_into::<js_sys::Function>()
    .map_err(|_| GpuFailure::Other("backend helper is not installed".into()))?;
    let promise = function
        .call1(
            &wasm_bindgen::JsValue::NULL,
            &wasm_bindgen::JsValue::from_str(&payload),
        )
        .map_err(|err| js_gpu_failure(&err, &endpoint))?
        .dyn_into::<js_sys::Promise>()
        .map_err(|_| GpuFailure::Other("backend helper did not return a Promise".into()))?;

    wasm_bindgen_futures::spawn_local(async move {
        let result = wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map(|value| js_json_string(&value))
            .map_err(|err| js_gpu_failure(&err, &endpoint));
        QISKIT_RUN_RESULT.with(|slot| {
            *slot.borrow_mut() = Some(result);
        });
        ctx.request_repaint();
    });
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn start_qiskit_run(_payload: String, _ctx: egui::Context) -> Result<(), GpuFailure> {
    Err(GpuFailure::Other(
        "Qiskit backend fetch is only available in wasm".into(),
    ))
}

#[cfg(target_arch = "wasm32")]
fn take_qiskit_run_result() -> Option<Result<String, GpuFailure>> {
    QISKIT_RUN_RESULT.with(|slot| slot.borrow_mut().take())
}

#[cfg(not(target_arch = "wasm32"))]
fn take_qiskit_run_result() -> Option<Result<String, GpuFailure>> {
    None
}

#[cfg(target_arch = "wasm32")]
fn take_external_gpu_status_override() -> Option<ExternalGpuStatus> {
    TEST_EXTERNAL_GPU_STATUS.with(|slot| slot.borrow_mut().take())
}

#[cfg(not(target_arch = "wasm32"))]
fn take_external_gpu_status_override() -> Option<ExternalGpuStatus> {
    None
}

#[cfg(all(target_arch = "wasm32", debug_assertions))]
fn wire_external_gpu_test_hooks(ctx: &egui::Context) {
    use wasm_bindgen::JsCast;

    let Some(window) = web_sys::window() else {
        return;
    };
    let ctx = ctx.clone();
    let closure =
        wasm_bindgen::closure::Closure::wrap(Box::new(move |value: wasm_bindgen::JsValue| {
            let json = value.as_string().or_else(|| {
                js_sys::JSON::stringify(&value)
                    .ok()
                    .and_then(|text| text.as_string())
            });
            let status = json
                .as_deref()
                .and_then(parse_external_gpu_status_json)
                .unwrap_or_else(|| {
                    ExternalGpuStatus::Failed(GpuFailure::Other("bad test status".into()))
                });
            TEST_EXTERNAL_GPU_STATUS.with(|slot| {
                *slot.borrow_mut() = Some(status);
            });
            ctx.request_repaint();
        }) as Box<dyn FnMut(wasm_bindgen::JsValue)>);
    let _ = js_sys::Reflect::set(
        window.as_ref(),
        &wasm_bindgen::JsValue::from_str("__setExternalGpuStatus"),
        closure.as_ref().unchecked_ref(),
    );
    closure.forget();
}

#[cfg(any(not(target_arch = "wasm32"), not(debug_assertions)))]
fn wire_external_gpu_test_hooks(_ctx: &egui::Context) {}

#[cfg(target_arch = "wasm32")]
fn parse_external_gpu_status_json(json: &str) -> Option<ExternalGpuStatus> {
    let value = js_sys::JSON::parse(json).ok()?;
    let status = js_string_prop(&value, "status")?;
    match status.as_str() {
        "idle" => Some(ExternalGpuStatus::Idle),
        "running" => Some(ExternalGpuStatus::Running),
        "completed" => {
            let millis = js_f64_prop(&value, "durationMs").unwrap_or(1_400.0);
            Some(ExternalGpuStatus::Completed {
                duration: Duration::from_millis(millis.max(0.0) as u64),
            })
        }
        "failed" => Some(ExternalGpuStatus::Failed(parse_gpu_failure(&value))),
        _ => None,
    }
}

#[cfg(target_arch = "wasm32")]
fn parse_gpu_failure(value: &wasm_bindgen::JsValue) -> GpuFailure {
    match js_string_prop(value, "failure").as_deref() {
        Some("backend_offline") | Some("offline") => GpuFailure::BackendOffline {
            endpoint: js_string_prop(value, "url").unwrap_or_else(|| "127.0.0.1:4184".to_owned()),
        },
        Some("unsupported_gate") | Some("gate") => GpuFailure::UnsupportedGate(
            js_string_prop(value, "gate").unwrap_or_else(|| "Spacer".to_owned()),
        ),
        Some("http") => GpuFailure::Http(js_f64_prop(value, "statusCode").unwrap_or(502.0) as u16),
        Some("other") => GpuFailure::Other(
            js_string_prop(value, "message").unwrap_or_else(|| "unknown error".to_owned()),
        ),
        _ => GpuFailure::Other("unknown error".to_owned()),
    }
}

#[cfg(target_arch = "wasm32")]
fn js_string_prop(value: &wasm_bindgen::JsValue, name: &str) -> Option<String> {
    js_sys::Reflect::get(value, &wasm_bindgen::JsValue::from_str(name))
        .ok()
        .and_then(|value| value.as_string())
}

#[cfg(target_arch = "wasm32")]
fn js_f64_prop(value: &wasm_bindgen::JsValue, name: &str) -> Option<f64> {
    js_sys::Reflect::get(value, &wasm_bindgen::JsValue::from_str(name))
        .ok()
        .and_then(|value| value.as_f64())
}

#[cfg(target_arch = "wasm32")]
fn js_json_string(value: &wasm_bindgen::JsValue) -> String {
    js_sys::JSON::stringify(value)
        .ok()
        .and_then(|text| text.as_string())
        .unwrap_or_else(|| format!("{value:?}"))
}

#[cfg(target_arch = "wasm32")]
fn js_gpu_failure(value: &wasm_bindgen::JsValue, endpoint: &str) -> GpuFailure {
    let message = js_error_message(value);
    if let Some(name) = unsupported_gate_from_message(&message) {
        return GpuFailure::UnsupportedGate(name);
    }
    if let Some(status) = js_f64_prop(value, "qniHttpStatus") {
        return GpuFailure::Http(status as u16);
    }
    if is_fetch_failure(&message) {
        return GpuFailure::BackendOffline {
            endpoint: endpoint.to_owned(),
        };
    }
    tracing::warn!(target: "qni_egui_web::external_gpu", error = %message, "external GPU run failed");
    GpuFailure::Other(short_failure_label(&message))
}

#[cfg(target_arch = "wasm32")]
fn qiskit_backend_endpoint(window: &web_sys::Window) -> String {
    let raw = js_sys::Reflect::get(
        window.as_ref(),
        &wasm_bindgen::JsValue::from_str("__qniQiskitBackendUrl"),
    )
    .ok()
    .and_then(|value| value.as_string())
    .unwrap_or_else(|| "http://127.0.0.1:4184/run".to_owned());
    display_endpoint(&raw)
}

#[cfg(target_arch = "wasm32")]
fn display_endpoint(url: &str) -> String {
    let without_scheme = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or(url);
    without_scheme
        .strip_suffix("/run")
        .unwrap_or(without_scheme)
        .to_owned()
}

#[cfg(target_arch = "wasm32")]
fn js_error_message(value: &wasm_bindgen::JsValue) -> String {
    if let Some(message) = value.as_string() {
        return message;
    }
    js_sys::Reflect::get(value, &wasm_bindgen::JsValue::from_str("message"))
        .ok()
        .and_then(|message| message.as_string())
        .unwrap_or_else(|| format!("{value:?}"))
}

fn unsupported_gate_from_message(message: &str) -> Option<String> {
    let lower = message.to_ascii_lowercase();
    if let Some((_, token)) = message.split_once("unsupported gate token:") {
        return Some(normalize_gate_name(token.trim()));
    }
    if lower.contains("anti-control") {
        return Some("Anti-control".to_owned());
    }
    if lower.contains("qft") {
        return Some("QFT".to_owned());
    }
    if lower.contains("controlled non-x") {
        return Some("controlled gate".to_owned());
    }
    None
}

fn normalize_gate_name(token: &str) -> String {
    match token {
        "…" => "Spacer".to_owned(),
        "Measure" => "Measurement".to_owned(),
        "Bloch" => "Bloch".to_owned(),
        other => other.to_owned(),
    }
}

#[cfg(target_arch = "wasm32")]
fn is_fetch_failure(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("failed to fetch")
        || lower.contains("networkerror")
        || lower.contains("load failed")
        || lower.contains("econnrefused")
}

fn short_failure_label(message: &str) -> String {
    const MAX_CHARS: usize = 32;
    let trimmed = message.trim();
    if trimmed.chars().count() <= MAX_CHARS {
        return trimmed.to_owned();
    }
    let mut label: String = trimmed.chars().take(MAX_CHARS - 1).collect();
    label.push('…');
    label
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{format_gpu_duration, qiskit_run_payload, unsupported_gate_from_message};

    #[test]
    fn payload_requests_histogram_without_full_vectors() {
        let payload = qiskit_run_payload(2, r#"[["H",1]]"#, 256);
        assert_eq!(
            payload,
            r#"{"qubits":2,"columns":[["H",1]],"shots":256,"outputs":{"histogram":true}}"#
        );
        assert!(!payload.contains("statevector"));
        assert!(!payload.contains("probabilities"));
    }

    #[test]
    fn duration_uses_seconds_or_milliseconds() {
        assert_eq!(format_gpu_duration(Duration::from_millis(420)), "420 ms");
        assert_eq!(format_gpu_duration(Duration::from_millis(1_400)), "1.4 s");
    }

    #[test]
    fn unsupported_gate_messages_are_mapped_to_gate_names() {
        assert_eq!(
            unsupported_gate_from_message("unsupported gate token: …"),
            Some("Spacer".to_owned())
        );
        assert_eq!(
            unsupported_gate_from_message(
                "QFT tokens are not supported by the dev Qiskit runner yet"
            ),
            Some("QFT".to_owned())
        );
    }
}
