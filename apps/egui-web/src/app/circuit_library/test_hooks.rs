//! Browser debug hooks for Playwright/Cucumber circuit-library tests.

use super::CircuitLibrary;

#[cfg(all(target_arch = "wasm32", debug_assertions))]
mod wasm_debug {
    use std::cell::RefCell;

    use eframe::egui;
    use wasm_bindgen::JsCast;

    use super::super::{CircuitEntry, CircuitLibrary};
    use crate::test_hooks::{
        set_property, QNI_CIRCUIT_LIBRARY_DELETE, QNI_CIRCUIT_LIBRARY_LOAD,
        QNI_CIRCUIT_LIBRARY_RENAME, QNI_CIRCUIT_LIBRARY_SAVE, QNI_CIRCUIT_PICKER_SNAPSHOT,
        QNI_SEED_CIRCUITS,
    };

    thread_local! {
        static PENDING_LIBRARY_SEED: RefCell<Option<CircuitLibrary>> = const { RefCell::new(None) };
        static LIBRARY_SNAPSHOT: RefCell<String> = RefCell::new(CircuitLibrary::seed().to_test_json());
    }

    pub(crate) fn wire(ctx: &egui::Context) {
        let Some(window) = web_sys::window() else {
            return;
        };

        let seed_ctx = ctx.clone();
        let seed =
            wasm_bindgen::closure::Closure::wrap(Box::new(move |value: wasm_bindgen::JsValue| {
                if let Some(library) = parse_library(value) {
                    PENDING_LIBRARY_SEED.with(|slot| {
                        *slot.borrow_mut() = Some(library);
                    });
                    seed_ctx.request_repaint();
                } else {
                    tracing::warn!("ignored invalid __seedCircuits payload");
                }
            })
                as Box<dyn FnMut(wasm_bindgen::JsValue)>);
        set_property(
            window.as_ref(),
            QNI_SEED_CIRCUITS,
            seed.as_ref().unchecked_ref(),
        );
        seed.forget();

        let snapshot = wasm_bindgen::closure::Closure::wrap(Box::new(move || -> String {
            LIBRARY_SNAPSHOT.with(|snapshot| snapshot.borrow().clone())
        })
            as Box<dyn FnMut() -> String>);
        set_property(
            window.as_ref(),
            QNI_CIRCUIT_PICKER_SNAPSHOT,
            snapshot.as_ref().unchecked_ref(),
        );
        snapshot.forget();

        wrap_library_mutation_hooks(window.as_ref(), ctx.clone());
    }

    fn wrap_library_mutation_hooks(window: &wasm_bindgen::JsValue, ctx: egui::Context) {
        wrap_library_hook_1(window, QNI_CIRCUIT_LIBRARY_DELETE, ctx.clone());
        wrap_library_hook_1(window, QNI_CIRCUIT_LIBRARY_LOAD, ctx.clone());
        wrap_library_hook_2(window, QNI_CIRCUIT_LIBRARY_RENAME, ctx.clone());
        wrap_library_hook_2(window, QNI_CIRCUIT_LIBRARY_SAVE, ctx);
    }

    fn library_hook(window: &wasm_bindgen::JsValue, name: &str) -> Option<js_sys::Function> {
        js_sys::Reflect::get(window, &wasm_bindgen::JsValue::from_str(name))
            .ok()?
            .dyn_into::<js_sys::Function>()
            .ok()
    }

    fn wrap_library_hook_1(window: &wasm_bindgen::JsValue, name: &str, ctx: egui::Context) {
        let Some(original) = library_hook(window, name) else {
            return;
        };
        let hook =
            wasm_bindgen::closure::Closure::wrap(Box::new(move |arg: wasm_bindgen::JsValue| {
                let result = original.call1(&wasm_bindgen::JsValue::NULL, &arg);
                ctx.request_repaint();
                result.unwrap_or_else(|error| wasm_bindgen::throw_val(error))
            })
                as Box<dyn FnMut(wasm_bindgen::JsValue) -> wasm_bindgen::JsValue>);
        set_property(window, name, hook.as_ref().unchecked_ref());
        hook.forget();
    }

    fn wrap_library_hook_2(window: &wasm_bindgen::JsValue, name: &str, ctx: egui::Context) {
        let Some(original) = library_hook(window, name) else {
            return;
        };
        let hook = wasm_bindgen::closure::Closure::wrap(Box::new(
            move |first: wasm_bindgen::JsValue, second: wasm_bindgen::JsValue| {
                let result = original.call2(&wasm_bindgen::JsValue::NULL, &first, &second);
                ctx.request_repaint();
                result.unwrap_or_else(|error| wasm_bindgen::throw_val(error))
            },
        )
            as Box<
                dyn FnMut(wasm_bindgen::JsValue, wasm_bindgen::JsValue) -> wasm_bindgen::JsValue,
            >);
        set_property(window, name, hook.as_ref().unchecked_ref());
        hook.forget();
    }

    pub(super) fn take_seeded_library() -> Option<CircuitLibrary> {
        PENDING_LIBRARY_SEED.with(|slot| slot.borrow_mut().take())
    }

    pub(super) fn publish_snapshot(library: &CircuitLibrary) {
        LIBRARY_SNAPSHOT.with(|snapshot| {
            *snapshot.borrow_mut() = library.to_test_json();
        });
    }

    fn parse_library(value: wasm_bindgen::JsValue) -> Option<CircuitLibrary> {
        let value = if let Some(text) = value.as_string() {
            js_sys::JSON::parse(&text).ok()?
        } else {
            value
        };
        let entries_value = prop(&value, "entries")?;
        if !js_sys::Array::is_array(&entries_value) {
            return None;
        }
        let entries_array = entries_value.unchecked_into::<js_sys::Array>();
        let mut entries = Vec::with_capacity(entries_array.length() as usize);
        for index in 0..entries_array.length() {
            let entry = entries_array.get(index);
            entries.push(CircuitEntry {
                id: string_prop(&entry, "id")?,
                name: string_prop(&entry, "name")?,
                circuit_json: string_prop(&entry, "circuit_json")
                    .or_else(|| string_prop(&entry, "circuitJson"))
                    .or_else(|| string_prop(&entry, "json"))?,
                updated_at: number_prop(&entry, "updated_at")
                    .or_else(|| number_prop(&entry, "updatedAt"))
                    .unwrap_or_else(super::super::now_millis),
            });
        }
        let active_id = string_prop(&value, "active_id")
            .or_else(|| string_prop(&value, "activeId"))
            .or_else(|| entries.first().map(|entry| entry.id.clone()))?;
        Some(CircuitLibrary::from_entries(entries, active_id))
    }

    fn prop(value: &wasm_bindgen::JsValue, name: &str) -> Option<wasm_bindgen::JsValue> {
        js_sys::Reflect::get(value, &wasm_bindgen::JsValue::from_str(name)).ok()
    }

    fn string_prop(value: &wasm_bindgen::JsValue, name: &str) -> Option<String> {
        prop(value, name).and_then(|value| value.as_string())
    }

    fn number_prop(value: &wasm_bindgen::JsValue, name: &str) -> Option<u64> {
        prop(value, name)
            .and_then(|value| value.as_f64())
            .map(|n| n as u64)
    }
}

#[cfg(all(target_arch = "wasm32", debug_assertions))]
pub(crate) use wasm_debug::wire as wire_test_hooks;

#[cfg(all(target_arch = "wasm32", debug_assertions))]
pub(super) fn take_seeded_library() -> Option<CircuitLibrary> {
    wasm_debug::take_seeded_library()
}

#[cfg(all(target_arch = "wasm32", debug_assertions))]
pub(super) fn publish_library_snapshot(library: &CircuitLibrary) {
    wasm_debug::publish_snapshot(library);
}

#[cfg(any(not(target_arch = "wasm32"), not(debug_assertions)))]
pub(crate) fn wire_test_hooks(_ctx: &eframe::egui::Context) {}

#[cfg(any(not(target_arch = "wasm32"), not(debug_assertions)))]
pub(super) fn take_seeded_library() -> Option<CircuitLibrary> {
    None
}

#[cfg(any(not(target_arch = "wasm32"), not(debug_assertions)))]
pub(super) fn publish_library_snapshot(_library: &CircuitLibrary) {}
