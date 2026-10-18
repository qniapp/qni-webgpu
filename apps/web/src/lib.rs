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
mod startup_error;
mod tailwind;
mod test_hooks;
mod url_circuit;

#[cfg(target_arch = "wasm32")]
use crate::app::QniApp;
#[cfg(target_arch = "wasm32")]
use crate::startup_error::{startup_error, StartupErrorCode};

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
    egui_ctx: eframe::egui::Context,
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

    /// Preferred canvas height in CSS pixels, without GPU readback.
    pub fn preferred_height(&self, width_css_px: f32) -> Result<f32, JsValue> {
        if !width_css_px.is_finite() || width_css_px <= 0.0 {
            return Err(JsValue::from_str("width must be finite and positive"));
        }
        self.runner
            .app_mut::<QniApp>()
            .map(|app| app.preferred_height(width_css_px))
            .ok_or_else(|| JsValue::from_str("Circuit runner is unavailable"))
    }

    /// Lock the zero-based semantic column, just like clicking a UI step.
    #[wasm_bindgen(js_name = selectStep)]
    pub fn select_step(&self, value: JsValue) -> Result<(), JsValue> {
        let value = value
            .as_f64()
            .ok_or_else(|| js_sys::TypeError::new("selectStep index must be a number"))?;
        let mut app = self
            .runner
            .app_mut::<QniApp>()
            .ok_or_else(|| JsValue::from_str("Circuit runner is unavailable"))?;
        app.select_step(value, &self.egui_ctx)
            .map_err(|error| js_sys::RangeError::new(&error))?;
        Ok(())
    }

    #[wasm_bindgen(js_name = selectedStep)]
    pub fn selected_step(&self) -> Result<Option<f64>, JsValue> {
        self.runner
            .app_mut::<QniApp>()
            .map(|app| app.selected_step().map(|step| step as f64))
            .ok_or_else(|| JsValue::from_str("Circuit runner is unavailable"))
    }

    #[wasm_bindgen(js_name = stepCount)]
    pub fn step_count(&self) -> Result<usize, JsValue> {
        self.runner
            .app_mut::<QniApp>()
            .map(|app| app.step_count())
            .ok_or_else(|| JsValue::from_str("Circuit runner is unavailable"))
    }

    /// Explicit on-demand readback, scoped to this runner and queue ordered
    /// after any pending circuit recompute or step selection.
    pub async fn read_state_vector(&self) -> Result<js_sys::Float32Array, JsValue> {
        {
            let mut app = self
                .runner
                .app_mut::<QniApp>()
                .ok_or_else(|| JsValue::from_str("Circuit runner is unavailable"))?;
            app.sync_runner_state(&self.render_state, &self.egui_ctx)?;
        }
        gpu::read_runner_state_vector(&self.render_state).await
    }
}

#[cfg(target_arch = "wasm32")]
type SharedGpuResult = Result<eframe::egui_wgpu::WgpuSetupExisting, JsValue>;

#[cfg(target_arch = "wasm32")]
enum SharedGpuInitialization {
    Idle,
    Pending(Vec<futures_channel::oneshot::Sender<SharedGpuResult>>),
    Ready(eframe::egui_wgpu::WgpuSetupExisting),
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static SHARED_GPU: std::cell::RefCell<SharedGpuInitialization> = const {
        std::cell::RefCell::new(SharedGpuInitialization::Idle)
    };
}

#[cfg(target_arch = "wasm32")]
async fn shared_gpu_setup() -> Result<eframe::egui_wgpu::WgpuSetup, JsValue> {
    let receiver = SHARED_GPU.with(|slot| {
        let mut state = slot.borrow_mut();
        let (sender, receiver) = futures_channel::oneshot::channel();
        match &mut *state {
            SharedGpuInitialization::Pending(waiters) => waiters.push(sender),
            SharedGpuInitialization::Idle => {
                *state = SharedGpuInitialization::Pending(vec![sender]);
                wasm_bindgen_futures::spawn_local(async {
                    let result = initialize_shared_gpu().await;
                    let next = match &result {
                        Ok(existing) => SharedGpuInitialization::Ready(existing.clone()),
                        Err(_) => SharedGpuInitialization::Idle,
                    };
                    let previous = SHARED_GPU.with(|slot| slot.replace(next));
                    if let SharedGpuInitialization::Pending(waiters) = previous {
                        for waiter in waiters {
                            let _ = waiter.send(result.clone());
                        }
                    }
                });
            }
            SharedGpuInitialization::Ready(existing) => {
                return std::ops::ControlFlow::Break(existing.clone());
            }
        }
        std::ops::ControlFlow::Continue(receiver)
    });
    match receiver {
        std::ops::ControlFlow::Break(existing) => Ok(existing.into()),
        std::ops::ControlFlow::Continue(receiver) => receiver
            .await
            .map_err(|_| JsValue::from_str("Shared GPU initialization was interrupted"))?
            .map(Into::into),
    }
}

#[cfg(target_arch = "wasm32")]
async fn initialize_shared_gpu() -> SharedGpuResult {
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
        .map_err(|error| {
            startup_error(StartupErrorCode::GpuAdapterUnavailable, &error.to_string())
        })?;
    let (device, queue) = adapter
        .request_device(&(create.device_descriptor)(&adapter))
        .await
        .map_err(|error| {
            startup_error(StartupErrorCode::GpuDeviceUnavailable, &error.to_string())
        })?;
    device.set_device_lost_callback(|reason, message| {
        SHARED_GPU.with(|slot| {
            let mut state = slot.borrow_mut();
            if matches!(*state, SharedGpuInitialization::Ready(_)) {
                *state = SharedGpuInitialization::Idle;
            }
        });
        web_sys::console::error_1(&JsValue::from_str(&format!(
            "Qni shared GPU device lost: {reason:?}: {message}"
        )));
        let detail = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&detail, &"reason".into(), &format!("{reason:?}").into());
        let _ = js_sys::Reflect::set(&detail, &"message".into(), &message.into());
        let init = web_sys::CustomEventInit::new();
        init.set_detail(&detail);
        if let (Some(window), Ok(event)) = (
            web_sys::window(),
            web_sys::CustomEvent::new_with_event_init_dict("qni-device-lost", &init),
        ) {
            let _ = window.dispatch_event(&event);
        }
    });
    let existing = eframe::egui_wgpu::WgpuSetupExisting {
        instance,
        adapter,
        device,
        queue,
    };
    Ok(existing)
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
    show_restart_button: bool,
    on_restart: Option<js_sys::Function>,
) -> Result<QniRunner, JsValue> {
    let mut startup = app::EmbedStartup::parse(
        circuit_json,
        show_state_panel,
        palette.as_deref(),
        max_wire_count,
    )
    .map_err(|error| startup_error(StartupErrorCode::InvalidEmbed, &error))?;
    if show_restart_button {
        startup.on_restart = on_restart.map(|callback| {
            Box::new(move || {
                if let Err(error) = callback.call0(&JsValue::NULL) {
                    web_sys::console::error_1(&error);
                }
            }) as Box<dyn Fn()>
        });
    }
    start_runner(canvas, Some(startup)).await
}

#[cfg(target_arch = "wasm32")]
async fn start_runner(
    canvas: web_sys::HtmlCanvasElement,
    embed: Option<app::EmbedStartup>,
) -> Result<QniRunner, JsValue> {
    use eframe::egui;

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
    let egui_ctx = std::rc::Rc::new(std::cell::RefCell::new(None::<egui::Context>));
    let captured_ctx = egui_ctx.clone();
    let touch_ctx = egui_ctx.clone();
    let web_options = eframe::WebOptions {
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            wgpu_setup: setup,
            ..Default::default()
        },
        // An autofocused INPUT scrolls the host article to the embed on load.
        text_agent_autofocus: standalone,
        should_prevent_default: Box::new(move |event| {
            if standalone {
                return true;
            }
            // Decide at touchstart, while the canvas listener can still
            // prevent native scrolling. Document touchmove may be passive.
            match event {
                egui::Event::PointerButton {
                    pos, pressed: true, ..
                } => touch_ctx
                    .borrow()
                    .as_ref()
                    .is_some_and(|ctx| QniApp::embed_captures_touch(ctx, *pos)),
                _ => true,
            }
        }),
        ..Default::default()
    };
    let runner = eframe::WebRunner::new();
    let result = runner
        .start(
            canvas,
            web_options,
            Box::new(move |cc| {
                *captured_render_state.borrow_mut() = cc.wgpu_render_state.clone();
                *captured_ctx.borrow_mut() = Some(cc.egui_ctx.clone());
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
    let egui_ctx = egui_ctx
        .borrow()
        .clone()
        .expect("context captured at startup");
    Ok(QniRunner {
        runner,
        render_state,
        egui_ctx,
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
/// Standalone test harness only: reads the buffer last published by any runner's
/// GPU preparation. Does not synchronize a pending selection or identify an embed.
/// Embedders must use their QniRunner's read_state_vector instead.
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

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn read_postselection_rates() -> Result<js_sys::Float32Array, wasm_bindgen::JsValue> {
    gpu::read_postselection_rates_impl().await
}
