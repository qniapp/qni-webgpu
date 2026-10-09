mod app;
mod circuit_library;
mod colors;
mod constants;
mod gates;
mod gpu;
mod grid_cell;
mod icons;
mod layout;
mod qubit_bit;
mod qubit_count;
mod render;
mod shared;
mod simulation_plan;
mod span_resize;
mod test_hooks;
mod url_circuit;

use crate::app::QniApp;

#[cfg(any(target_arch = "wasm32", test))]
fn web_wgpu_setup() -> eframe::egui_wgpu::WgpuSetup {
    let mut setup = eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    // Quantum compute shaders require WebGPU; WebGL cannot run this app.
    setup.instance_descriptor.backends = eframe::wgpu::Backends::BROWSER_WEBGPU;
    eframe::egui_wgpu::WgpuSetup::CreateNew(setup)
}

#[cfg(test)]
mod web_backend_tests {
    #[test]
    fn web_setup_requires_webgpu_without_webgl_fallback() {
        let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = super::web_wgpu_setup() else {
            panic!("expected a new WebGPU instance");
        };
        assert_eq!(
            setup.instance_descriptor.backends,
            eframe::wgpu::Backends::BROWSER_WEBGPU
        );
    }
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// Owns one canvas runner. Call `destroy` before removing its canvas.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct QniRunner {
    runner: eframe::WebRunner,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl QniRunner {
    pub fn destroy(&self) {
        self.runner.destroy();
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn start(canvas: web_sys::HtmlCanvasElement) -> Result<QniRunner, JsValue> {
    start_runner(canvas, None).await
}

/// Starts an isolated, local-WebGPU-only editor without URL or browser storage.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn start_embed(
    canvas: web_sys::HtmlCanvasElement,
    circuit_json: &str,
    show_state_panel: bool,
) -> Result<QniRunner, JsValue> {
    let startup =
        app::EmbedStartup::parse(circuit_json, show_state_panel).map_err(JsValue::from_str)?;
    start_runner(canvas, Some(startup)).await
}

#[cfg(target_arch = "wasm32")]
async fn start_runner(
    canvas: web_sys::HtmlCanvasElement,
    embed: Option<app::EmbedStartup>,
) -> Result<QniRunner, JsValue> {
    let standalone = embed.is_none();
    if standalone {
        crate::test_hooks::set_startup_stage("runner-start");
    }
    let web_options = eframe::WebOptions {
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            wgpu_setup: web_wgpu_setup(),
            ..Default::default()
        },
        ..Default::default()
    };
    let runner = eframe::WebRunner::new();
    let result = runner
        .start(
            canvas,
            web_options,
            Box::new(move |cc| {
                if standalone {
                    crate::test_hooks::set_startup_stage("app-new");
                }
                Ok(Box::new(match embed {
                    Some(startup) => QniApp::new_with_startup(cc, Some(startup)),
                    None => QniApp::new(cc),
                }))
            }),
        )
        .await;
    if let Err(error) = result {
        runner.destroy();
        return Err(error);
    }
    Ok(QniRunner { runner })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn read_state_vector() -> Result<js_sys::Float32Array, wasm_bindgen::JsValue> {
    gpu::read_state_vector_impl().await
}

/// Test-only on-demand readback for Bloch vectors. Triggers a fresh
/// staging-buffer copy + `map_async` against `bloch_output_buffer` and
/// returns `[gate_id, x, y, z, …]` once the GPU finishes. Production code
/// never calls this — the rendering shaders read the same buffer directly.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn read_bloch_vectors() -> Result<js_sys::Float32Array, wasm_bindgen::JsValue> {
    gpu::read_bloch_vectors_impl().await
}

/// Test-only on-demand readback for measurement outcomes. Returns
/// `[gate_id, outcome, …]` (outcome is `0.0` or `1.0`).
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn read_measurement_outcomes() -> Result<js_sys::Float32Array, wasm_bindgen::JsValue> {
    gpu::read_measurement_outcomes_impl().await
}

/// Test-only on-demand readback for Probability display probabilities. Returns
/// `[gate_id, p0, p1, ..., p65535, ...]` for each live Probability slot.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn read_probability_distributions() -> Result<js_sys::Float32Array, wasm_bindgen::JsValue>
{
    gpu::read_probability_distributions_impl().await
}

/// Test-only on-demand readback for one Amplitude display cell. Returns
/// `[gate_id, outcome, re, im, incoherent, quality, phaseLockIndex, span]`.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn read_amplitude_cell(
    gate_id: u32,
    outcome: u32,
) -> Result<js_sys::Float64Array, wasm_bindgen::JsValue> {
    gpu::read_amplitude_cell_impl(gate_id, outcome).await
}

/// Test-only on-demand readback for one Density Matrix display cell. Returns
/// `[gate_id, row, col, re, im, unity, span]` with `re`/`im` normalized by trace.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn read_density_matrix_cell(
    gate_id: u32,
    row: u32,
    col: u32,
) -> Result<js_sys::Float64Array, wasm_bindgen::JsValue> {
    gpu::read_density_matrix_cell_impl(gate_id, row, col).await
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn circuit_library_list() -> Result<String, wasm_bindgen::JsValue> {
    circuit_library::list()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn circuit_library_save(
    name: &str,
    circuit_json: &str,
) -> Result<String, wasm_bindgen::JsValue> {
    circuit_library::save(name, circuit_json)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn circuit_library_load(id: &str) -> Result<String, wasm_bindgen::JsValue> {
    circuit_library::load(id)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn circuit_library_rename(id: &str, name: &str) -> Result<(), wasm_bindgen::JsValue> {
    circuit_library::rename(id, name)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn circuit_library_delete(id: &str) -> Result<(), wasm_bindgen::JsValue> {
    circuit_library::delete(id)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn circuit_library_clear() -> Result<(), wasm_bindgen::JsValue> {
    circuit_library::clear()
}
