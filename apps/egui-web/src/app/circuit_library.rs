//! In-memory named circuit library for the toolbar circuit picker.
//!
//! The stored circuit body is the canonical `{"cols":[...]}` JSON shared with
//! URL hashes and undo checkpoints. The app owns picker/editor state while the
//! root `crate::circuit_library` module adapts this shape to localStorage.

use eframe::egui;
use serde::{Deserialize, Serialize};

use super::circuit_history::CircuitRevision;
use super::QniApp;
use crate::url_circuit::EMPTY_CIRCUIT_JSON;

pub(crate) type CircuitId = String;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct CircuitEntry {
    pub id: CircuitId,
    pub name: String,
    pub circuit_json: String,
    pub updated_at: u64,
}

impl Default for CircuitEntry {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            circuit_json: EMPTY_CIRCUIT_JSON.to_owned(),
            updated_at: 0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct CircuitLibrary {
    pub entries: Vec<CircuitEntry>,
    pub active_id: CircuitId,
}

impl Default for CircuitLibrary {
    fn default() -> Self {
        Self::seed()
    }
}

impl CircuitLibrary {
    pub(crate) fn seed() -> Self {
        let now = now_millis();
        let entries = vec![
            CircuitEntry {
                id: "bell".to_owned(),
                name: "Bell state".to_owned(),
                circuit_json: r#"{"cols":[["H"],["•","X"]]}"#.to_owned(),
                updated_at: now,
            },
            CircuitEntry {
                id: "ghz".to_owned(),
                name: "GHZ state".to_owned(),
                circuit_json: r#"{"cols":[["H"],["•","X"],["•",1,"X"]]}"#.to_owned(),
                updated_at: now,
            },
            CircuitEntry {
                id: "qft-4".to_owned(),
                name: "QFT 4-qubit".to_owned(),
                circuit_json: r#"{"cols":[["QFT4"]]}"#.to_owned(),
                updated_at: now,
            },
        ];
        Self {
            active_id: entries[0].id.clone(),
            entries,
        }
    }

    pub(crate) fn from_entries(entries: Vec<CircuitEntry>, active_id: CircuitId) -> Self {
        let mut library = Self { entries, active_id };
        library.ensure_non_empty();
        if !library
            .entries
            .iter()
            .any(|entry| entry.id == library.active_id)
        {
            library.active_id = library.entries[0].id.clone();
        }
        library
    }

    pub(crate) fn active_index(&self) -> usize {
        self.entries
            .iter()
            .position(|entry| entry.id == self.active_id)
            .unwrap_or(0)
    }

    pub(crate) fn active(&self) -> &CircuitEntry {
        &self.entries[self.active_index()]
    }

    pub(crate) fn update_active(&mut self, circuit_json: String) {
        let active_id = self.active_id.clone();
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == active_id) {
            entry.circuit_json = circuit_json;
            entry.updated_at = now_millis();
        }
    }

    pub(crate) fn set_active(&mut self, id: CircuitId) -> &CircuitEntry {
        if self.entries.iter().any(|entry| entry.id == id) {
            self.active_id = id;
        }
        self.active()
    }

    pub(crate) fn set_active_index(&mut self, index: usize) -> &CircuitEntry {
        if let Some(entry) = self.entries.get(index) {
            self.active_id = entry.id.clone();
        }
        self.active()
    }

    pub(crate) fn rename(&mut self, id: &str, name: &str) {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return;
        }
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == id) {
            entry.name = trimmed.to_owned();
            entry.updated_at = now_millis();
        }
    }

    pub(crate) fn duplicate(&mut self, index: usize) -> Option<&CircuitEntry> {
        let source = self.entries.get(index)?.clone();
        let entry = CircuitEntry {
            id: self.fresh_id("circuit"),
            name: format!("{} (copy)", source.name),
            circuit_json: source.circuit_json,
            updated_at: now_millis(),
        };
        let id = entry.id.clone();
        self.entries
            .insert((index + 1).min(self.entries.len()), entry);
        Some(self.set_active(id))
    }

    pub(crate) fn move_up(&mut self, index: usize) {
        if index > 0 && index < self.entries.len() {
            self.entries.swap(index - 1, index);
        }
    }

    pub(crate) fn move_down(&mut self, index: usize) {
        if index + 1 < self.entries.len() {
            self.entries.swap(index, index + 1);
        }
    }

    pub(crate) fn delete(&mut self, index: usize) -> Option<&CircuitEntry> {
        if self.entries.len() <= 1 || index >= self.entries.len() {
            return None;
        }
        let removed_id = self.entries[index].id.clone();
        self.entries.remove(index);
        if self.active_id == removed_id {
            self.active_id = self.entries[0].id.clone();
        }
        Some(self.active())
    }

    pub(crate) fn create_new(&mut self) -> &CircuitEntry {
        let entry = CircuitEntry {
            id: self.fresh_id("circuit"),
            name: "Untitled".to_owned(),
            circuit_json: EMPTY_CIRCUIT_JSON.to_owned(),
            updated_at: now_millis(),
        };
        let id = entry.id.clone();
        self.entries.push(entry);
        self.set_active(id)
    }

    pub(crate) fn set_active_current_circuit(&mut self, circuit_json: String) {
        let id = "current".to_owned();
        let entry = CircuitEntry {
            id: id.clone(),
            name: "Untitled".to_owned(),
            circuit_json,
            updated_at: now_millis(),
        };
        if let Some(existing) = self.entries.iter_mut().find(|entry| entry.id == id) {
            *existing = entry;
        } else {
            self.entries.insert(0, entry);
        }
        self.active_id = id;
    }

    pub(crate) fn to_test_json(&self) -> String {
        let entries = self
            .entries
            .iter()
            .map(|entry| {
                format!(
                    r#"{{"id":"{}","name":"{}","circuit_json":"{}","updated_at":{}}}"#,
                    json_escape(&entry.id),
                    json_escape(&entry.name),
                    json_escape(&entry.circuit_json),
                    entry.updated_at
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            r#"{{"entries":[{}],"active_id":"{}"}}"#,
            entries,
            json_escape(&self.active_id)
        )
    }

    fn ensure_non_empty(&mut self) {
        if self.entries.is_empty() {
            self.entries.push(CircuitEntry {
                id: "circuit-1".to_owned(),
                name: "Untitled".to_owned(),
                circuit_json: EMPTY_CIRCUIT_JSON.to_owned(),
                updated_at: now_millis(),
            });
        }
        if self.active_id.is_empty() {
            self.active_id = self.entries[0].id.clone();
        }
    }

    fn fresh_id(&self, prefix: &str) -> CircuitId {
        let mut index = self.entries.len() + 1;
        loop {
            let id = format!("{prefix}-{index}");
            if !self.entries.iter().any(|entry| entry.id == id) {
                return id;
            }
            index += 1;
        }
    }

    pub(crate) fn for_startup(url_json: String, url_has_payload: bool) -> (Self, String) {
        let load_state = load_persisted_library_state();
        let loaded = matches!(&load_state, PersistedLibraryState::Loaded(_));
        let can_persist = !matches!(&load_state, PersistedLibraryState::Invalid);
        let mut changed = false;
        let mut library = match load_state {
            PersistedLibraryState::Loaded(library) => library,
            PersistedLibraryState::Missing | PersistedLibraryState::Invalid => Self::seed(),
        };
        let active_json = if url_has_payload {
            if let Some(entry) = library
                .entries
                .iter()
                .find(|entry| entry.circuit_json == url_json)
            {
                if library.active_id != entry.id {
                    library.active_id = entry.id.clone();
                    changed = true;
                }
            } else {
                library.set_active_current_circuit(url_json.clone());
                changed = true;
            }
            url_json
        } else if loaded {
            library.active().circuit_json.clone()
        } else {
            library.set_active_current_circuit(url_json.clone());
            changed = true;
            url_json
        };
        if can_persist && changed {
            persist_library(&library);
        }
        (library, active_json)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum PickerState {
    #[default]
    Closed,
    Open {
        focused_index: usize,
        submenu: Option<PickerSubmenuState>,
        renaming: Option<RenameState>,
    },
}

impl PickerState {
    pub(crate) fn open(focused_index: usize) -> Self {
        Self::Open {
            focused_index,
            submenu: None,
            renaming: None,
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        matches!(self, Self::Open { .. })
    }

    pub(crate) fn close(&mut self) {
        *self = Self::Closed;
    }

    pub(crate) fn open_submenu(&mut self, entry_index: usize) {
        if let Self::Open { submenu, .. } = self {
            *submenu = Some(PickerSubmenuState { entry_index });
        }
    }

    pub(crate) fn close_submenu(&mut self) {
        if let Self::Open { submenu, .. } = self {
            *submenu = None;
        }
    }

    pub(crate) fn start_rename(&mut self, entry: &CircuitEntry) {
        if let Self::Open {
            submenu, renaming, ..
        } = self
        {
            *submenu = None;
            *renaming = Some(RenameState {
                entry_id: entry.id.clone(),
                draft: entry.name.clone(),
                select_all_pending: true,
            });
        }
    }

    pub(crate) fn update_rename_draft(&mut self, entry_id: &str, draft: String) {
        if let Self::Open {
            renaming: Some(rename),
            ..
        } = self
        {
            if rename.entry_id == entry_id {
                rename.draft = draft;
            }
        }
    }

    pub(crate) fn cancel_rename(&mut self) {
        if let Self::Open { renaming, .. } = self {
            *renaming = None;
        }
    }

    pub(crate) fn finish_rename(&mut self) {
        if let Self::Open { renaming, .. } = self {
            *renaming = None;
        }
    }

    pub(crate) fn renaming_id(&self) -> Option<&str> {
        match self {
            Self::Open {
                renaming: Some(rename),
                ..
            } => Some(&rename.entry_id),
            _ => None,
        }
    }

    pub(crate) fn submenu_index(&self) -> Option<usize> {
        match self {
            Self::Open {
                submenu: Some(submenu),
                ..
            } => Some(submenu.entry_index),
            _ => None,
        }
    }

    pub(crate) fn focused_index(&self) -> Option<usize> {
        match self {
            Self::Open { focused_index, .. } => Some(*focused_index),
            _ => None,
        }
    }

    pub(crate) fn set_focused_index(&mut self, index: usize) {
        if let Self::Open { focused_index, .. } = self {
            *focused_index = index;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PickerSubmenuState {
    pub(crate) entry_index: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RenameState {
    pub(crate) entry_id: CircuitId,
    pub(crate) draft: String,
    pub(crate) select_all_pending: bool,
}

impl QniApp {
    pub(crate) fn active_circuit_name(&self) -> &str {
        &self.library.active().name
    }

    pub(crate) fn apply_pending_circuit_library_seed(&mut self, ctx: &egui::Context) {
        if let Some(library) = take_seeded_library() {
            self.library = library;
            let active_json = self.library.active().circuit_json.clone();
            self.replace_editor_circuit(active_json, ctx);
        }
    }

    pub(crate) fn apply_external_circuit_library_update(&mut self, ctx: &egui::Context) {
        if take_external_library_dirty() {
            if let Some(library) = load_persisted_library() {
                self.library = library;
                let active_json = self.library.active().circuit_json.clone();
                self.replace_editor_circuit(active_json, ctx);
            }
        }
    }

    pub(crate) fn publish_circuit_library_snapshot(&self) {
        publish_library_snapshot(&self.library);
    }

    pub(crate) fn select_circuit_entry(&mut self, index: usize, ctx: &egui::Context) {
        let circuit_json = self.library.set_active_index(index).circuit_json.clone();
        self.picker.close();
        self.replace_editor_circuit(circuit_json, ctx);
    }

    pub(crate) fn create_new_circuit(&mut self, ctx: &egui::Context) {
        let circuit_json = self.library.create_new().circuit_json.clone();
        self.picker.close();
        self.replace_editor_circuit(circuit_json, ctx);
    }

    pub(crate) fn duplicate_circuit_entry(&mut self, index: usize, ctx: &egui::Context) {
        if let Some(entry) = self.library.duplicate(index) {
            let circuit_json = entry.circuit_json.clone();
            let focused_index = self.library.active_index();
            self.picker.set_focused_index(focused_index);
            self.replace_editor_circuit(circuit_json, ctx);
        }
        self.picker.close_submenu();
    }

    pub(crate) fn move_circuit_entry_up(&mut self, index: usize) {
        let focused_index = index.saturating_sub(1);
        self.library.move_up(index);
        self.picker.set_focused_index(focused_index);
        persist_library(&self.library);
        self.picker.close_submenu();
    }

    pub(crate) fn move_circuit_entry_down(&mut self, index: usize) {
        let focused_index = (index + 1).min(self.library.entries.len().saturating_sub(1));
        self.library.move_down(index);
        self.picker.set_focused_index(focused_index);
        persist_library(&self.library);
        self.picker.close_submenu();
    }

    pub(crate) fn delete_circuit_entry(&mut self, index: usize, ctx: &egui::Context) {
        let was_active = self.library.active_index() == index;
        if self.library.delete(index).is_some() {
            let focused_index = index.min(self.library.entries.len().saturating_sub(1));
            self.picker.set_focused_index(focused_index);
            if was_active {
                let circuit_json = self.library.active().circuit_json.clone();
                self.replace_editor_circuit(circuit_json, ctx);
            } else {
                persist_library(&self.library);
            }
        }
        self.picker.close_submenu();
    }

    pub(crate) fn start_circuit_rename(&mut self, index: usize) {
        if let Some(entry) = self.library.entries.get(index) {
            self.picker.start_rename(entry);
        }
    }

    pub(crate) fn commit_circuit_rename(&mut self, entry_id: &str, next_name: String) {
        self.library.rename(entry_id, &next_name);
        persist_library(&self.library);
        self.picker.finish_rename();
    }

    fn replace_editor_circuit(&mut self, circuit_json: String, ctx: &egui::Context) {
        self.circuit_revision = CircuitRevision::starting_at(circuit_json.clone());
        self.apply_circuit_json(&circuit_json, ctx);
    }
}

enum PersistedLibraryState {
    Missing,
    Loaded(CircuitLibrary),
    Invalid,
}

fn load_persisted_library() -> Option<CircuitLibrary> {
    match load_persisted_library_state() {
        PersistedLibraryState::Loaded(library) => Some(library),
        PersistedLibraryState::Missing | PersistedLibraryState::Invalid => None,
    }
}

fn load_persisted_library_state() -> PersistedLibraryState {
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
fn take_external_library_dirty() -> bool {
    crate::circuit_library::take_app_library_dirty()
}

#[cfg(not(target_arch = "wasm32"))]
fn take_external_library_dirty() -> bool {
    false
}

#[cfg(target_arch = "wasm32")]
fn js_error_message(value: wasm_bindgen::JsValue) -> String {
    value.as_string().unwrap_or_else(|| format!("{value:?}"))
}

fn json_escape(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(target_arch = "wasm32")]
fn now_millis() -> u64 {
    js_sys::Date::now().max(0.0) as u64
}

#[cfg(not(target_arch = "wasm32"))]
fn now_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(all(target_arch = "wasm32", debug_assertions))]
mod test_hooks {
    use std::cell::RefCell;

    use eframe::egui;
    use wasm_bindgen::JsCast;

    use super::{CircuitEntry, CircuitLibrary};

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
        let _ = js_sys::Reflect::set(
            window.as_ref(),
            &wasm_bindgen::JsValue::from_str("__seedCircuits"),
            seed.as_ref().unchecked_ref(),
        );
        seed.forget();

        let snapshot = wasm_bindgen::closure::Closure::wrap(Box::new(move || -> String {
            LIBRARY_SNAPSHOT.with(|snapshot| snapshot.borrow().clone())
        })
            as Box<dyn FnMut() -> String>);
        let _ = js_sys::Reflect::set(
            window.as_ref(),
            &wasm_bindgen::JsValue::from_str("__qniCircuitPickerSnapshot"),
            snapshot.as_ref().unchecked_ref(),
        );
        snapshot.forget();
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
                    .unwrap_or_else(super::now_millis),
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
pub(crate) use test_hooks::wire as wire_test_hooks;

#[cfg(all(target_arch = "wasm32", debug_assertions))]
fn take_seeded_library() -> Option<CircuitLibrary> {
    test_hooks::take_seeded_library()
}

#[cfg(all(target_arch = "wasm32", debug_assertions))]
fn publish_library_snapshot(library: &CircuitLibrary) {
    test_hooks::publish_snapshot(library);
}

#[cfg(any(not(target_arch = "wasm32"), not(debug_assertions)))]
pub(crate) fn wire_test_hooks(_ctx: &egui::Context) {}

#[cfg(any(not(target_arch = "wasm32"), not(debug_assertions)))]
fn take_seeded_library() -> Option<CircuitLibrary> {
    None
}

#[cfg(any(not(target_arch = "wasm32"), not(debug_assertions)))]
fn publish_library_snapshot(_library: &CircuitLibrary) {}

#[cfg(test)]
mod tests {
    use super::{CircuitLibrary, EMPTY_CIRCUIT_JSON};

    #[test]
    fn seed_contains_three_named_samples() {
        let library = CircuitLibrary::seed();

        assert_eq!(library.entries.len(), 3);
        assert_eq!(library.active_id, "bell");
        assert_eq!(library.entries[0].name, "Bell state");
        assert_eq!(library.entries[1].name, "GHZ state");
        assert_eq!(library.entries[2].name, "QFT 4-qubit");
    }

    #[test]
    fn update_and_set_active_keep_canonical_json() {
        let mut library = CircuitLibrary::seed();

        library.set_active("ghz".to_owned());
        library.update_active(EMPTY_CIRCUIT_JSON.to_owned());

        assert_eq!(library.active().id, "ghz");
        assert_eq!(library.active().circuit_json, EMPTY_CIRCUIT_JSON);
    }

    #[test]
    fn duplicate_move_and_delete_preserve_active_invariant() {
        let mut library = CircuitLibrary::seed();

        let duplicated = library.duplicate(1).expect("duplicate").clone();
        assert_eq!(duplicated.name, "GHZ state (copy)");
        assert_eq!(library.active_id, duplicated.id);
        assert_eq!(library.entries[2].id, duplicated.id);

        library.move_up(2);
        assert_eq!(library.entries[1].id, duplicated.id);
        library.move_down(1);
        assert_eq!(library.entries[2].id, duplicated.id);

        library.delete(2);
        assert_eq!(library.active_id, "bell");
        assert_eq!(library.entries.len(), 3);
    }
}
