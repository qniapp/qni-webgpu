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
    use qni_egui_web_circuit_library_model::{CircuitOrigin, EMPTY_CIRCUIT_JSON};
    use wasm_bindgen::{JsCast, JsValue};

    const STORAGE_KEY: &str = "qni.circuit_library.v2";
    const LEGACY_STORAGE_KEY: &str = "qni.circuit_library.v1";
    const VERSION: f64 = 2.0;

    thread_local! {
        static APP_LIBRARY_DIRTY: Cell<bool> = const { Cell::new(false) };
    }

    pub(crate) fn list() -> Result<String, JsValue> {
        let document = read_document()?;
        stringify(&document)
    }

    pub(crate) fn save(name: &str, circuit_json: &str) -> Result<String, JsValue> {
        let name = normalize_name(name)?;
        crate::url_circuit::summarize_circuit_json(circuit_json)
            .ok_or_else(|| error("invalid circuit json"))?;
        let document = read_document()?;
        let entries = array_prop(&document, "entries")?;
        let id = fresh_id(&entries);
        let now = js_sys::Date::now();

        let entry = js_sys::Object::new();
        set_string(entry.as_ref(), "id", &id)?;
        set_string(entry.as_ref(), "name", &name)?;
        set_string(entry.as_ref(), "circuit_json", circuit_json)?;
        set_number(entry.as_ref(), "updated_at", now)?;
        set_value(
            entry.as_ref(),
            "origin",
            &origin_object(&CircuitOrigin::User { locked: false })?,
        )?;

        let next = js_sys::Array::new();
        next.push(&entry);
        for index in 0..entries.length() {
            next.push(&entries.get(index));
        }
        set_value(&document, "entries", next.as_ref())?;
        set_value(&document, "active_id", &JsValue::from_str(&id))?;
        write_document(&document)?;
        mark_app_library_dirty();
        Ok(id)
    }

    pub(crate) fn load(id: &str) -> Result<String, JsValue> {
        let document = read_document()?;
        let entries = array_prop(&document, "entries")?;
        let entry = find_entry(&entries, id).ok_or_else(|| error("saved circuit not found"))?;
        let json = string_prop(&entry, "circuit_json")
            .ok_or_else(|| error("saved circuit has no json"))?;
        set_value(&document, "active_id", &JsValue::from_str(id))?;
        write_document(&document)?;
        mark_app_library_dirty();
        Ok(json)
    }

    pub(crate) fn rename(id: &str, name: &str) -> Result<(), JsValue> {
        let name = normalize_name(name)?;
        let document = read_document()?;
        let entries = array_prop(&document, "entries")?;
        let entry = find_entry(&entries, id).ok_or_else(|| error("saved circuit not found"))?;
        if entry_locked(&entry)? {
            return Err(error("saved circuit is locked"));
        }
        set_string(&entry, "name", &name)?;
        set_number(&entry, "updated_at", js_sys::Date::now())?;
        write_document(&document)?;
        mark_app_library_dirty();
        Ok(())
    }

    pub(crate) fn delete(id: &str) -> Result<(), JsValue> {
        let document = read_document()?;
        let entries = array_prop(&document, "entries")?;
        let next = js_sys::Array::new();
        let mut removed = false;
        for index in 0..entries.length() {
            let entry = entries.get(index);
            if string_prop(&entry, "id").as_deref() == Some(id) {
                if entry_locked(&entry)? {
                    return Err(error("saved circuit is locked"));
                }
                removed = true;
            } else {
                next.push(&entry);
            }
        }
        if !removed {
            return Err(error("saved circuit not found"));
        }
        set_value(&document, "entries", next.as_ref())?;
        if string_prop(&document, "active_id").as_deref() == Some(id) {
            set_value(&document, "active_id", &JsValue::NULL)?;
        }
        write_document(&document)?;
        mark_app_library_dirty();
        Ok(())
    }

    pub(crate) fn clear() -> Result<(), JsValue> {
        storage()?.remove_item(STORAGE_KEY).map_err(storage_error)?;
        storage()?
            .remove_item(LEGACY_STORAGE_KEY)
            .map_err(storage_error)
    }

    pub(crate) fn load_app_library() -> Result<Option<CircuitLibrary>, JsValue> {
        load_app_library_result()
    }

    pub(crate) fn save_app_library(library: &CircuitLibrary) -> Result<(), JsValue> {
        let document = app_library_to_document(library)?;
        validate_v2_document(document.as_ref())?;
        write_document(document.as_ref())
    }

    pub(crate) fn take_app_library_dirty() -> bool {
        APP_LIBRARY_DIRTY.with(|dirty| dirty.replace(false))
    }

    fn load_app_library_result() -> Result<Option<CircuitLibrary>, JsValue> {
        let storage = storage()?;
        let v2_raw = storage.get_item(STORAGE_KEY).map_err(storage_error)?;
        let v1_raw = storage
            .get_item(LEGACY_STORAGE_KEY)
            .map_err(storage_error)?;
        if let Some(raw) = v2_raw {
            match parse_v2_document(&raw) {
                Ok(document) => {
                    if v1_raw.is_some() {
                        if let Err(error) = storage
                            .remove_item(LEGACY_STORAGE_KEY)
                            .map_err(storage_error)
                        {
                            tracing::warn!(?error, "failed to remove migrated v1 circuit library");
                        }
                    }
                    return document_to_app_library(&document);
                }
                Err(v2_error) if v1_raw.is_some() => {
                    tracing::warn!(?v2_error, "falling back to v1 circuit library migration");
                    backup_broken_v2(&raw);
                    let document = migrate_legacy_raw_to_v2(v1_raw.as_deref().unwrap_or_default())?;
                    write_document(document.as_ref())?;
                    validate_v2_document(document.as_ref())?;
                    return document_to_app_library(document.as_ref());
                }
                Err(error) => return Err(error),
            }
        }
        if let Some(raw) = v1_raw {
            let document = migrate_legacy_raw_to_v2(&raw)?;
            write_document(document.as_ref())?;
            validate_v2_document(document.as_ref())?;
            return document_to_app_library(document.as_ref());
        }
        Ok(None)
    }

    fn parse_v2_document(raw: &str) -> Result<JsValue, JsValue> {
        let document =
            js_sys::JSON::parse(raw).map_err(|_| error("circuit library is corrupted"))?;
        validate_v2_document(&document)?;
        Ok(document)
    }

    fn migrate_legacy_raw_to_v2(raw: &str) -> Result<js_sys::Object, JsValue> {
        let document =
            js_sys::JSON::parse(raw).map_err(|_| error("circuit library is corrupted"))?;
        validate_v1_document(&document)?;
        let circuits = array_prop(&document, "circuits")?;
        let mut v1_entries = Vec::with_capacity(circuits.length() as usize);
        for index in 0..circuits.length() {
            let entry = circuits.get(index);
            v1_entries.push(CircuitEntry::user(
                string_prop(&entry, "id").ok_or_else(|| error("circuit library is corrupted"))?,
                string_prop(&entry, "name").ok_or_else(|| error("circuit library is corrupted"))?,
                string_prop(&entry, "json").ok_or_else(|| error("circuit library is corrupted"))?,
                number_prop(&entry, "updatedAt")
                    .ok_or_else(|| error("circuit library is corrupted"))?
                    .max(0.0) as u64,
                false,
            ));
        }
        let active_id = js_sys::Reflect::get(&document, &JsValue::from_str("activeId"))
            .ok()
            .and_then(|value| value.as_string());
        let migrated = CircuitLibrary::migrate_v1_entries(v1_entries, active_id);
        tracing::info!(
            entry_count = migrated.entries.len(),
            active_id = %migrated.active_id,
            "migrated circuit library from v1 to v2"
        );
        app_library_to_document(&migrated)
    }

    fn backup_broken_v2(raw: &str) {
        let key = format!(
            "{STORAGE_KEY}.broken-{}",
            js_sys::Date::now().max(0.0) as u64
        );
        if let Err(error) =
            storage().and_then(|storage| storage.set_item(&key, raw).map_err(storage_error))
        {
            tracing::warn!(?error, "failed to back up broken v2 circuit library");
        }
    }

    fn mark_app_library_dirty() {
        APP_LIBRARY_DIRTY.with(|dirty| dirty.set(true));
    }

    fn read_document() -> Result<JsValue, JsValue> {
        let Some(raw) = storage()?.get_item(STORAGE_KEY).map_err(storage_error)? else {
            return Ok(default_document().into());
        };
        parse_v2_document(&raw)
    }

    fn write_document(document: &JsValue) -> Result<(), JsValue> {
        let text = stringify(document)?;
        storage()?
            .set_item(STORAGE_KEY, &text)
            .map_err(storage_error)
    }

    fn default_document() -> js_sys::Object {
        let document = js_sys::Object::new();
        let entries = js_sys::Array::new();
        let _ = set_number(document.as_ref(), "version", VERSION);
        let _ = set_value(document.as_ref(), "active_id", &JsValue::NULL);
        let _ = set_value(document.as_ref(), "entries", entries.as_ref());
        document
    }

    fn app_library_to_document(library: &CircuitLibrary) -> Result<js_sys::Object, JsValue> {
        let document = default_document();
        let entries = js_sys::Array::new();
        for entry in &library.entries {
            crate::url_circuit::summarize_circuit_json(&entry.circuit_json)
                .ok_or_else(|| error("invalid circuit json"))?;
            let stored = js_sys::Object::new();
            set_string(stored.as_ref(), "id", &entry.id)?;
            set_string(stored.as_ref(), "name", &entry.name)?;
            set_string(stored.as_ref(), "circuit_json", &entry.circuit_json)?;
            set_number(stored.as_ref(), "updated_at", entry.updated_at as f64)?;
            set_value(stored.as_ref(), "origin", &origin_object(&entry.origin)?)?;
            entries.push(stored.as_ref());
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
        set_value(document.as_ref(), "active_id", &active_id)?;
        set_value(document.as_ref(), "entries", entries.as_ref())?;
        Ok(document)
    }

    fn document_to_app_library(document: &JsValue) -> Result<Option<CircuitLibrary>, JsValue> {
        let entries_value = array_prop(document, "entries")?;
        if entries_value.length() == 0 {
            return Ok(None);
        }
        let mut entries = Vec::with_capacity(entries_value.length() as usize);
        for index in 0..entries_value.length() {
            let entry = entries_value.get(index);
            entries.push(CircuitEntry {
                id: string_prop(&entry, "id")
                    .ok_or_else(|| error("circuit library is corrupted"))?,
                name: string_prop(&entry, "name")
                    .ok_or_else(|| error("circuit library is corrupted"))?,
                circuit_json: string_prop(&entry, "circuit_json")
                    .ok_or_else(|| error("circuit library is corrupted"))?,
                updated_at: number_prop(&entry, "updated_at")
                    .ok_or_else(|| error("circuit library is corrupted"))?
                    .max(0.0) as u64,
                origin: origin_prop(&entry)?,
            });
        }
        let active_id = js_sys::Reflect::get(document, &JsValue::from_str("active_id"))
            .ok()
            .and_then(|value| value.as_string())
            .unwrap_or_else(|| entries[0].id.clone());
        Ok(Some(CircuitLibrary::from_entries(entries, active_id)))
    }

    fn validate_v2_document(document: &JsValue) -> Result<(), JsValue> {
        if number_prop(document, "version") != Some(VERSION) {
            return Err(error("unsupported circuit library version"));
        }
        let active_id_value = js_sys::Reflect::get(document, &JsValue::from_str("active_id"))
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
        let entries = array_prop(document, "entries")?;
        let mut ids = Vec::new();
        for index in 0..entries.length() {
            let entry = entries.get(index);
            validate_v2_entry(&entry, &mut ids)?;
        }
        if let Some(active_id) = active_id {
            if !ids.contains(&active_id) {
                return Err(error("circuit library is corrupted"));
            }
        }
        Ok(())
    }

    fn validate_v2_entry(entry: &JsValue, ids: &mut Vec<String>) -> Result<(), JsValue> {
        let id = string_prop(entry, "id").ok_or_else(|| error("circuit library is corrupted"))?;
        let name =
            string_prop(entry, "name").ok_or_else(|| error("circuit library is corrupted"))?;
        let json = string_prop(entry, "circuit_json")
            .ok_or_else(|| error("circuit library is corrupted"))?;
        let updated_at = number_prop(entry, "updated_at")
            .ok_or_else(|| error("circuit library is corrupted"))?;
        if id.is_empty() || ids.contains(&id) || name.trim().is_empty() || updated_at < 0.0 {
            return Err(error("circuit library is corrupted"));
        }
        crate::url_circuit::summarize_circuit_json(&json)
            .ok_or_else(|| error("circuit library is corrupted"))?;
        let _ = origin_prop(entry)?;
        ids.push(id);
        Ok(())
    }

    fn validate_v1_document(document: &JsValue) -> Result<(), JsValue> {
        if number_prop(document, "version") != Some(1.0) {
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
                || crate::url_circuit::summarize_circuit_json(&json).is_none()
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

    fn origin_object(origin: &CircuitOrigin) -> Result<JsValue, JsValue> {
        let object = js_sys::Object::new();
        match origin {
            CircuitOrigin::Sample { origin_id } => {
                set_string(object.as_ref(), "kind", "sample")?;
                set_string(object.as_ref(), "origin_id", origin_id)?;
            }
            CircuitOrigin::User { locked } => {
                set_string(object.as_ref(), "kind", "user")?;
                set_bool(object.as_ref(), "locked", *locked)?;
            }
        }
        Ok(object.into())
    }

    fn origin_prop(entry: &JsValue) -> Result<CircuitOrigin, JsValue> {
        let origin = js_sys::Reflect::get(entry, &JsValue::from_str("origin"))
            .map_err(|_| error("circuit library is corrupted"))?;
        let kind =
            string_prop(&origin, "kind").ok_or_else(|| error("circuit library is corrupted"))?;
        match kind.as_str() {
            "sample" => {
                if !has_exact_props(&origin, &["kind", "origin_id"]) {
                    return Err(error("circuit library is corrupted"));
                }
                Ok(CircuitOrigin::Sample {
                    origin_id: string_prop(&origin, "origin_id")
                        .ok_or_else(|| error("circuit library is corrupted"))?,
                })
            }
            "user" => {
                if !has_exact_props(&origin, &["kind", "locked"]) {
                    return Err(error("circuit library is corrupted"));
                }
                Ok(CircuitOrigin::User {
                    locked: bool_prop(&origin, "locked")
                        .ok_or_else(|| error("circuit library is corrupted"))?,
                })
            }
            _ => Err(error("circuit library is corrupted")),
        }
    }

    fn entry_locked(entry: &JsValue) -> Result<bool, JsValue> {
        Ok(match origin_prop(entry)? {
            CircuitOrigin::Sample { .. } => true,
            CircuitOrigin::User { locked } => locked,
        })
    }

    fn fresh_id(entries: &js_sys::Array) -> String {
        loop {
            let millis = js_sys::Date::now() as u64;
            let suffix = (js_sys::Math::random() * 0xFF_FFFF as f64) as u32;
            let id = format!("ckt_{millis}_{suffix:06x}");
            if find_entry(entries, &id).is_none() {
                return id;
            }
        }
    }

    fn find_entry(entries: &js_sys::Array, id: &str) -> Option<JsValue> {
        for index in 0..entries.length() {
            let entry = entries.get(index);
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

    fn bool_prop(value: &JsValue, name: &str) -> Option<bool> {
        js_sys::Reflect::get(value, &JsValue::from_str(name))
            .ok()
            .and_then(|value| value.as_bool())
    }

    fn has_exact_props(value: &JsValue, allowed: &[&str]) -> bool {
        let Some(object) = value.dyn_ref::<js_sys::Object>() else {
            return false;
        };
        let keys = js_sys::Object::keys(object);
        if keys.length() as usize != allowed.len() {
            return false;
        }
        for index in 0..keys.length() {
            let Some(key) = keys.get(index).as_string() else {
                return false;
            };
            if !allowed.contains(&key.as_str()) {
                return false;
            }
        }
        true
    }

    fn set_string(object: &JsValue, name: &str, value: &str) -> Result<(), JsValue> {
        set_value(object, name, &JsValue::from_str(value))
    }

    fn set_number(object: &JsValue, name: &str, value: f64) -> Result<(), JsValue> {
        set_value(object, name, &JsValue::from_f64(value))
    }

    fn set_bool(object: &JsValue, name: &str, value: bool) -> Result<(), JsValue> {
        set_value(object, name, &JsValue::from_bool(value))
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

    #[allow(dead_code)]
    fn empty_user_entry(id: &str, name: &str) -> CircuitEntry {
        CircuitEntry::user(
            id.to_owned(),
            name.to_owned(),
            EMPTY_CIRCUIT_JSON.to_owned(),
            js_sys::Date::now().max(0.0) as u64,
            false,
        )
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) use wasm::{
    clear, delete, list, load, load_app_library, rename, save, save_app_library,
    take_app_library_dirty,
};
