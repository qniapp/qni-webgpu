use std::cell::RefCell;
use std::time::Duration;

use eframe::egui;

use super::status::{ExternalGpuStatus, GpuFailure};

#[cfg(target_arch = "wasm32")]
thread_local! {
    static TEST_EXTERNAL_GPU_STATUS: RefCell<Option<ExternalGpuStatus>> = const { RefCell::new(None) };
}

#[cfg(target_arch = "wasm32")]
pub(super) fn take_external_gpu_status_override() -> Option<ExternalGpuStatus> {
    TEST_EXTERNAL_GPU_STATUS.with(|slot| slot.borrow_mut().take())
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn take_external_gpu_status_override() -> Option<ExternalGpuStatus> {
    None
}

#[cfg(all(target_arch = "wasm32", debug_assertions))]
pub(super) fn wire_external_gpu_test_hooks(ctx: &egui::Context) {
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
    crate::test_hooks::set_property(
        window.as_ref(),
        crate::test_hooks::QNI_SET_EXTERNAL_GPU_STATUS,
        closure.as_ref().unchecked_ref(),
    );
    closure.forget();
}

#[cfg(any(not(target_arch = "wasm32"), not(debug_assertions)))]
pub(super) fn wire_external_gpu_test_hooks(_ctx: &egui::Context) {}

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
