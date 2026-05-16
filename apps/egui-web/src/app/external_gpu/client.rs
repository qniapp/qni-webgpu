use std::cell::RefCell;

use eframe::egui;

use super::{short_failure_label, unsupported_gate_from_message, GpuFailure};

#[cfg(target_arch = "wasm32")]
thread_local! {
    static QISKIT_RUN_RESULT: RefCell<Option<Result<String, GpuFailure>>> = const { RefCell::new(None) };
}

#[cfg(target_arch = "wasm32")]
pub(super) fn start_qiskit_run(payload: String, ctx: egui::Context) -> Result<(), GpuFailure> {
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
pub(super) fn start_qiskit_run(_payload: String, _ctx: egui::Context) -> Result<(), GpuFailure> {
    Err(GpuFailure::Other(
        "Qiskit backend fetch is only available in wasm".into(),
    ))
}

#[cfg(target_arch = "wasm32")]
pub(super) fn take_qiskit_run_result() -> Option<Result<String, GpuFailure>> {
    QISKIT_RUN_RESULT.with(|slot| slot.borrow_mut().take())
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn take_qiskit_run_result() -> Option<Result<String, GpuFailure>> {
    None
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

#[cfg(target_arch = "wasm32")]
fn is_fetch_failure(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("failed to fetch")
        || lower.contains("networkerror")
        || lower.contains("load failed")
        || lower.contains("econnrefused")
}
