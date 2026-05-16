//! Persistence adapter for the circuit library.

use super::CircuitLibrary;

pub(super) enum PersistedLibraryState {
    Missing,
    Loaded(CircuitLibrary),
    Invalid,
}

pub(super) fn load_persisted_library_state() -> PersistedLibraryState {
    match load_persisted_library_result() {
        Ok(Some(library)) => PersistedLibraryState::Loaded(library),
        Ok(None) => PersistedLibraryState::Missing,
        Err(message) => {
            tracing::warn!(%message, "failed to load circuit library from localStorage");
            PersistedLibraryState::Invalid
        }
    }
}

pub(crate) fn persist_library(library: &CircuitLibrary) {
    if let Err(message) = persist_library_result(library) {
        tracing::warn!(%message, "failed to persist circuit library to localStorage");
    }
}

#[cfg(target_arch = "wasm32")]
fn load_persisted_library_result() -> Result<Option<CircuitLibrary>, String> {
    crate::circuit_library::load_app_library().map_err(js_error_message)
}

#[cfg(not(target_arch = "wasm32"))]
fn load_persisted_library_result() -> Result<Option<CircuitLibrary>, String> {
    Ok(None)
}

#[cfg(target_arch = "wasm32")]
fn persist_library_result(library: &CircuitLibrary) -> Result<(), String> {
    crate::circuit_library::save_app_library(library).map_err(js_error_message)
}

#[cfg(not(target_arch = "wasm32"))]
fn persist_library_result(_library: &CircuitLibrary) -> Result<(), String> {
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub(super) fn take_external_library_dirty() -> bool {
    crate::circuit_library::take_app_library_dirty()
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn take_external_library_dirty() -> bool {
    false
}

#[cfg(target_arch = "wasm32")]
fn js_error_message(value: wasm_bindgen::JsValue) -> String {
    value.as_string().unwrap_or_else(|| format!("{value:?}"))
}
