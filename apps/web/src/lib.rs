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
    render_state: eframe::egui_wgpu::RenderState,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl QniRunner {
    pub fn destroy(&self) {
        self.runner.destroy();
    }

    /// Current committed circuit metadata, scoped to this editor. No GPU readback.
    pub fn circuit_json(&self) -> Result<String, JsValue> {
        self.runner
            .app_mut::<QniApp>()
            .map(|app| app.library.active().circuit_json.clone())
            .ok_or_else(|| JsValue::from_str("Circuit runner is unavailable"))
    }

    /// Test-only, on-demand readback of this runner, never another canvas.
    pub async fn read_state_vector(&self) -> Result<js_sys::Float32Array, JsValue> {
        gpu::read_runner_state_vector(&self.render_state).await
    }
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static SHARED_GPU: std::cell::RefCell<Option<eframe::egui_wgpu::WgpuSetupExisting>> = const { std::cell::RefCell::new(None) };
}

// embed.mjs serializes startup, including failed attempts, so concurrent
// elements cannot race this asynchronous one-device initialization.
#[cfg(target_arch = "wasm32")]
async fn shared_gpu_setup() -> Result<eframe::egui_wgpu::WgpuSetup, JsValue> {
    if let Some(existing) = SHARED_GPU.with(|slot| slot.borrow().clone()) {
        return Ok(existing.into());
    }
    let setup = web_wgpu_setup();
    let instance = setup.new_instance().await;
    let eframe::egui_wgpu::WgpuSetup::CreateNew(create) = setup else {
        unreachable!()
    };
    let adapter = instance
        .request_adapter(&eframe::wgpu::RequestAdapterOptions {
            power_preference: create.power_preference,
            ..Default::default()
        })
        .await
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let (device, queue) = adapter
        .request_device(&(create.device_descriptor)(&adapter))
        .await
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    device.set_device_lost_callback(|reason, message| {
        SHARED_GPU.with(|slot| *slot.borrow_mut() = None);
        web_sys::console::error_1(&JsValue::from_str(&format!(
            "Qni shared GPU device lost: {reason:?}: {message}"
        )));
        if let (Some(window), Ok(event)) =
            (web_sys::window(), web_sys::Event::new("qni-device-lost"))
        {
            let _ = window.dispatch_event(&event);
        }
    });
    let existing = eframe::egui_wgpu::WgpuSetupExisting {
        instance,
        adapter,
        device,
        queue,
    };
    SHARED_GPU.with(|slot| *slot.borrow_mut() = Some(existing.clone()));
    Ok(existing.into())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn start(canvas: web_sys::HtmlCanvasElement) -> Result<QniRunner, JsValue> {
    start_runner(canvas, None).await
}

/// Starts an isolated, local-WebGPU-only editor without URL or browser storage.
/// `palette` restricts the palette to the listed gate tokens; `undefined`
/// keeps the full palette. `max_wire_count` is qni's `data-max-wire-count`.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn start_embed(
    canvas: web_sys::HtmlCanvasElement,
    circuit_json: &str,
    show_state_panel: bool,
    palette: Option<Vec<String>>,
    max_wire_count: Option<f64>,
) -> Result<QniRunner, JsValue> {
    let startup = app::EmbedStartup::parse(
        circuit_json,
        show_state_panel,
        palette.as_deref(),
        max_wire_count,
    )
    .map_err(|error| JsValue::from_str(&error))?;
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
    let setup = if standalone {
        web_wgpu_setup()
    } else {
        shared_gpu_setup().await?
    };
    let render_state = std::rc::Rc::new(std::cell::RefCell::new(None));
    let captured_render_state = render_state.clone();
    let web_options = eframe::WebOptions {
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            wgpu_setup: setup,
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
                *captured_render_state.borrow_mut() = cc.wgpu_render_state.clone();
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
    let Some(render_state) = render_state.borrow_mut().take() else {
        runner.destroy();
        return Err(JsValue::from_str("WebGPU render state missing"));
    };
    Ok(QniRunner {
        runner,
        render_state,
    })
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
