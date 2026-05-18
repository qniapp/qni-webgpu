//! Browser-local named circuit library backed by localStorage.
//!
//! This is intentionally UI-free. Future menu/command surfaces call these
//! functions; the editor state, URL, undo stack, and GPU plan remain owned by
//! `QniApp`. The stored circuit body is the same canonical `{"cols":[...]}`
//! JSON used in URL hashes, so the library does not introduce a second circuit
//! format.

#[cfg(target_arch = "wasm32")]
mod wasm {
    use std::cell::Cell;

    use crate::app::circuit_library::{CircuitEntry, CircuitLibrary};
    use wasm_bindgen::{JsCast, JsValue};

    const STORAGE_KEY: &str = "qni.circuit_library.v1";
    const VERSION: f64 = 1.0;
    const SAMPLE_VERSION: f64 = 2.0;

    thread_local! {
        static APP_LIBRARY_DIRTY: Cell<bool> = const { Cell::new(false) };
    }

    pub(crate) fn list() -> Result<String, JsValue> {
        let document = read_document()?;
        stringify(&document)
    }

    pub(crate) fn save(name: &str, circuit_json: &str) -> Result<String, JsValue> {
        let name = normalize_name(name)?;
        let summary = crate::url_circuit::summarize_circuit_json(circuit_json)
            .ok_or_else(|| error("invalid circuit json"))?;
        let document = read_document()?;
        let circuits = array_prop(&document, "circuits")?;
        let id = fresh_id(&circuits);
        let now = js_sys::Date::now();

        let entry = js_sys::Object::new();
        set_string(entry.as_ref(), "id", &id)?;
        set_string(entry.as_ref(), "name", &name)?;
        set_string(entry.as_ref(), "json", circuit_json)?;
        set_number(entry.as_ref(), "createdAt", now)?;
        set_number(entry.as_ref(), "updatedAt", now)?;
        set_value(entry.as_ref(), "meta", &meta_object(summary)?)?;

        let next = js_sys::Array::new();
        next.push(&entry);
        for index in 0..circuits.length() {
            next.push(&circuits.get(index));
        }
        set_value(&document, "circuits", next.as_ref())?;
        set_value(&document, "activeId", &JsValue::from_str(&id))?;
        write_document(&document)?;
        mark_app_library_dirty();
        Ok(id)
    }

    pub(crate) fn load(id: &str) -> Result<String, JsValue> {
        let document = read_document()?;
        let circuits = array_prop(&document, "circuits")?;
        let entry = find_entry(&circuits, id).ok_or_else(|| error("saved circuit not found"))?;
        let json = string_prop(&entry, "json").ok_or_else(|| error("saved circuit has no json"))?;
        set_value(&document, "activeId", &JsValue::from_str(id))?;
        write_document(&document)?;
        mark_app_library_dirty();
        Ok(json)
    }

    pub(crate) fn rename(id: &str, name: &str) -> Result<(), JsValue> {
        let name = normalize_name(name)?;
        let document = read_document()?;
        let circuits = array_prop(&document, "circuits")?;
        let entry = find_entry(&circuits, id).ok_or_else(|| error("saved circuit not found"))?;
        set_string(&entry, "name", &name)?;
        set_number(&entry, "updatedAt", js_sys::Date::now())?;
        write_document(&document)?;
        mark_app_library_dirty();
        Ok(())
    }

    pub(crate) fn delete(id: &str) -> Result<(), JsValue> {
        let document = read_document()?;
        let circuits = array_prop(&document, "circuits")?;
        let next = js_sys::Array::new();
        let mut removed = false;
        for index in 0..circuits.length() {
            let entry = circuits.get(index);
            if string_prop(&entry, "id").as_deref() == Some(id) {
                removed = true;
            } else {
                next.push(&entry);
            }
        }
        if !removed {
            return Err(error("saved circuit not found"));
        }
        set_value(&document, "circuits", next.as_ref())?;
        if string_prop(&document, "activeId").as_deref() == Some(id) {
            set_value(&document, "activeId", &JsValue::NULL)?;
        }
        write_document(&document)?;
        mark_app_library_dirty();
        Ok(())
    }

    pub(crate) fn clear() -> Result<(), JsValue> {
        storage()?.remove_item(STORAGE_KEY).map_err(storage_error)
    }

    pub(crate) fn load_app_library() -> Result<Option<CircuitLibrary>, JsValue> {
        let Some(raw) = storage()?.get_item(STORAGE_KEY).map_err(storage_error)? else {
            return Ok(None);
        };
        let document =
            js_sys::JSON::parse(&raw).map_err(|_| error("circuit library is corrupted"))?;
        validate_document(&document)?;
        if migrate_document_samples(&document)? {
            if let Err(error) = write_document(&document) {
                tracing::warn!(?error, "failed to persist circuit library sample migration");
            }
        }
        document_to_app_library(&document)
    }

    pub(crate) fn save_app_library(library: &CircuitLibrary) -> Result<(), JsValue> {
        let existing_document = read_document().ok();
        let existing_circuits = existing_document
            .as_ref()
            .and_then(|document| array_prop(document, "circuits").ok());
        let document = default_document();
        set_number(document.as_ref(), "sampleVersion", SAMPLE_VERSION)?;
        let circuits = js_sys::Array::new();
        for entry in &library.entries {
            let summary = crate::url_circuit::summarize_circuit_json(&entry.circuit_json)
                .ok_or_else(|| error("invalid circuit json"))?;
            let stored = js_sys::Object::new();
            set_string(stored.as_ref(), "id", &entry.id)?;
            set_string(stored.as_ref(), "name", &entry.name)?;
            set_string(stored.as_ref(), "json", &entry.circuit_json)?;
            let created_at = existing_circuits
                .as_ref()
                .and_then(|circuits| created_at_for_id(circuits, &entry.id))
                .unwrap_or(entry.updated_at as f64);
            set_number(stored.as_ref(), "createdAt", created_at)?;
            set_number(stored.as_ref(), "updatedAt", entry.updated_at as f64)?;
            set_value(stored.as_ref(), "meta", &meta_object(summary)?)?;
            circuits.push(stored.as_ref());
        }
        let active_id = if library
            .entries
            .iter()
            .any(|entry| entry.id == library.active_id)
        {
            JsValue::from_str(&library.active_id)
        } else {
            JsValue::NULL
        };
        set_value(document.as_ref(), "activeId", &active_id)?;
        set_value(document.as_ref(), "circuits", circuits.as_ref())?;
        write_document(document.as_ref())
    }

    pub(crate) fn take_app_library_dirty() -> bool {
        APP_LIBRARY_DIRTY.with(|dirty| dirty.replace(false))
    }

    fn mark_app_library_dirty() {
        APP_LIBRARY_DIRTY.with(|dirty| dirty.set(true));
    }

    fn created_at_for_id(circuits: &js_sys::Array, id: &str) -> Option<f64> {
        find_entry(circuits, id).and_then(|entry| number_prop(&entry, "createdAt"))
    }

    fn read_document() -> Result<JsValue, JsValue> {
        let Some(raw) = storage()?.get_item(STORAGE_KEY).map_err(storage_error)? else {
            return Ok(default_document().into());
        };
        let document =
            js_sys::JSON::parse(&raw).map_err(|_| error("circuit library is corrupted"))?;
        validate_document(&document)?;
        Ok(document)
    }

    fn write_document(document: &JsValue) -> Result<(), JsValue> {
        let text = stringify(document)?;
        storage()?
            .set_item(STORAGE_KEY, &text)
            .map_err(storage_error)
    }

    fn default_document() -> js_sys::Object {
        let document = js_sys::Object::new();
        let circuits = js_sys::Array::new();
        let _ = set_number(document.as_ref(), "version", VERSION);
        let _ = set_value(document.as_ref(), "activeId", &JsValue::NULL);
        let _ = set_value(document.as_ref(), "circuits", circuits.as_ref());
        document
    }

    fn document_to_app_library(document: &JsValue) -> Result<Option<CircuitLibrary>, JsValue> {
        let circuits = array_prop(document, "circuits")?;
        if circuits.length() == 0 {
            return Ok(None);
        }
        let mut entries = Vec::with_capacity(circuits.length() as usize);
        for index in 0..circuits.length() {
            let entry = circuits.get(index);
            entries.push(CircuitEntry {
                id: string_prop(&entry, "id")
                    .ok_or_else(|| error("circuit library is corrupted"))?,
                name: string_prop(&entry, "name")
                    .ok_or_else(|| error("circuit library is corrupted"))?,
                circuit_json: string_prop(&entry, "json")
                    .ok_or_else(|| error("circuit library is corrupted"))?,
                updated_at: number_prop(&entry, "updatedAt")
                    .ok_or_else(|| error("circuit library is corrupted"))?
                    .max(0.0) as u64,
            });
        }
        let active_id = js_sys::Reflect::get(document, &JsValue::from_str("activeId"))
            .ok()
            .and_then(|value| value.as_string())
            .unwrap_or_else(|| entries[0].id.clone());
        Ok(Some(CircuitLibrary::from_entries(entries, active_id)))
    }

    fn migrate_document_samples(document: &JsValue) -> Result<bool, JsValue> {
        if number_prop(document, "sampleVersion").is_some_and(|version| version >= SAMPLE_VERSION) {
            return Ok(false);
        }
        let circuits = array_prop(document, "circuits")?;
        let has_grover = find_entry(&circuits, "grover-search").is_some();
        let has_legacy_samples = ["bell", "ghz", "qft-4"]
            .iter()
            .all(|id| find_entry(&circuits, id).is_some());
        if !has_legacy_samples {
            return Ok(false);
        }
        set_number(document, "sampleVersion", SAMPLE_VERSION)?;
        if !has_grover {
            append_grover_sample(&circuits)?;
        }
        Ok(true)
    }

    fn append_grover_sample(circuits: &js_sys::Array) -> Result<(), JsValue> {
        let sample = CircuitLibrary::seed()
            .entries
            .into_iter()
            .find(|entry| entry.id == "grover-search")
            .ok_or_else(|| error("Grover Search sample missing"))?;
        let summary = crate::url_circuit::summarize_circuit_json(&sample.circuit_json)
            .ok_or_else(|| error("invalid circuit json"))?;
        let stored = js_sys::Object::new();
        set_string(stored.as_ref(), "id", &sample.id)?;
        set_string(stored.as_ref(), "name", &sample.name)?;
        set_string(stored.as_ref(), "json", &sample.circuit_json)?;
        set_number(stored.as_ref(), "createdAt", sample.updated_at as f64)?;
        set_number(stored.as_ref(), "updatedAt", sample.updated_at as f64)?;
        set_value(stored.as_ref(), "meta", &meta_object(summary)?)?;
        circuits.push(stored.as_ref());
        Ok(())
    }

    fn validate_document(document: &JsValue) -> Result<(), JsValue> {
        if number_prop(document, "version") != Some(VERSION) {
            return Err(error("unsupported circuit library version"));
        }
        let active_id_value = js_sys::Reflect::get(document, &JsValue::from_str("activeId"))
            .map_err(|_| error("circuit library is corrupted"))?;
        let active_id = if active_id_value.is_null() {
            None
        } else {
            Some(
                active_id_value
                    .as_string()
                    .ok_or_else(|| error("circuit library is corrupted"))?,
            )
        };
        let circuits = array_prop(document, "circuits")?;
        let mut ids = Vec::new();
        for index in 0..circuits.length() {
            let entry = circuits.get(index);
            let id =
                string_prop(&entry, "id").ok_or_else(|| error("circuit library is corrupted"))?;
            let name =
                string_prop(&entry, "name").ok_or_else(|| error("circuit library is corrupted"))?;
            let json =
                string_prop(&entry, "json").ok_or_else(|| error("circuit library is corrupted"))?;
            let created_at = number_prop(&entry, "createdAt")
                .ok_or_else(|| error("circuit library is corrupted"))?;
            let updated_at = number_prop(&entry, "updatedAt")
                .ok_or_else(|| error("circuit library is corrupted"))?;
            if id.is_empty()
                || ids.contains(&id)
                || name.trim().is_empty()
                || updated_at < created_at
            {
                return Err(error("circuit library is corrupted"));
            }
            let summary = crate::url_circuit::summarize_circuit_json(&json)
                .ok_or_else(|| error("circuit library is corrupted"))?;
            let meta = js_sys::Reflect::get(&entry, &JsValue::from_str("meta"))
                .map_err(|_| error("circuit library is corrupted"))?;
            if number_prop(&meta, "qubits") != Some(summary.qubits as f64)
                || number_prop(&meta, "columns") != Some(summary.columns as f64)
                || number_prop(&meta, "gateCount") != Some(summary.gate_count as f64)
            {
                return Err(error("circuit library is corrupted"));
            }
            ids.push(id);
        }
        if let Some(active_id) = active_id {
            if !ids.contains(&active_id) {
                return Err(error("circuit library is corrupted"));
            }
        }
        Ok(())
    }

    fn meta_object(summary: crate::url_circuit::CircuitJsonSummary) -> Result<JsValue, JsValue> {
        let meta = js_sys::Object::new();
        set_number(meta.as_ref(), "qubits", summary.qubits as f64)?;
        set_number(meta.as_ref(), "columns", summary.columns as f64)?;
        set_number(meta.as_ref(), "gateCount", summary.gate_count as f64)?;
        Ok(meta.into())
    }

    fn fresh_id(circuits: &js_sys::Array) -> String {
        loop {
            let millis = js_sys::Date::now() as u64;
            let suffix = (js_sys::Math::random() * 0xFF_FFFF as f64) as u32;
            let id = format!("ckt_{millis}_{suffix:06x}");
            if find_entry(circuits, &id).is_none() {
                return id;
            }
        }
    }

    fn find_entry(circuits: &js_sys::Array, id: &str) -> Option<JsValue> {
        for index in 0..circuits.length() {
            let entry = circuits.get(index);
            if string_prop(&entry, "id").as_deref() == Some(id) {
                return Some(entry);
            }
        }
        None
    }

    fn normalize_name(name: &str) -> Result<String, JsValue> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            Err(error("circuit name is empty"))
        } else {
            Ok(trimmed.to_owned())
        }
    }

    fn storage() -> Result<web_sys::Storage, JsValue> {
        web_sys::window()
            .ok_or_else(|| error("window not found"))?
            .local_storage()
            .map_err(storage_error)?
            .ok_or_else(|| error("localStorage is unavailable"))
    }

    fn array_prop(value: &JsValue, name: &str) -> Result<js_sys::Array, JsValue> {
        let prop = js_sys::Reflect::get(value, &JsValue::from_str(name))
            .map_err(|_| error("circuit library is corrupted"))?;
        if !js_sys::Array::is_array(&prop) {
            return Err(error("circuit library is corrupted"));
        }
        Ok(prop.unchecked_into::<js_sys::Array>())
    }

    fn string_prop(value: &JsValue, name: &str) -> Option<String> {
        js_sys::Reflect::get(value, &JsValue::from_str(name))
            .ok()
            .and_then(|value| value.as_string())
    }

    fn number_prop(value: &JsValue, name: &str) -> Option<f64> {
        js_sys::Reflect::get(value, &JsValue::from_str(name))
            .ok()
            .and_then(|value| value.as_f64())
    }

    fn set_string(object: &JsValue, name: &str, value: &str) -> Result<(), JsValue> {
        set_value(object, name, &JsValue::from_str(value))
    }

    fn set_number(object: &JsValue, name: &str, value: f64) -> Result<(), JsValue> {
        set_value(object, name, &JsValue::from_f64(value))
    }

    fn set_value(object: &JsValue, name: &str, value: &JsValue) -> Result<(), JsValue> {
        js_sys::Reflect::set(object, &JsValue::from_str(name), value)
            .map_err(|_| error("failed to update circuit library"))?;
        Ok(())
    }

    fn stringify(value: &JsValue) -> Result<String, JsValue> {
        js_sys::JSON::stringify(value)
            .map_err(|_| error("failed to encode circuit library"))?
            .as_string()
            .ok_or_else(|| error("failed to encode circuit library"))
    }

    fn storage_error(value: JsValue) -> JsValue {
        let message = js_sys::Reflect::get(&value, &JsValue::from_str("name"))
            .ok()
            .and_then(|name| name.as_string())
            .or_else(|| {
                js_sys::Reflect::get(&value, &JsValue::from_str("message"))
                    .ok()
                    .and_then(|message| message.as_string())
            })
            .unwrap_or_else(|| format!("{value:?}"));
        error(&format!("localStorage error: {message}"))
    }

    fn error(message: &str) -> JsValue {
        JsValue::from_str(message)
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) use wasm::{
    clear, delete, list, load, load_app_library, rename, save, save_app_library,
    take_app_library_dirty,
};
