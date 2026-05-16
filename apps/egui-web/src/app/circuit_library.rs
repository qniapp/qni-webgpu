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

const DEFAULT_CIRCUIT_NAME_PREFIX: &str = "Circuit ";

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
        self.duplicate_at_index(index)?;
        Some(self.active())
    }

    /// Insert a copy of the active entry right after it; switch active to the
    /// new entry and bump its timestamp. Copy names follow the picker/toolbar
    /// contract: "Name (copy)", then "Name (copy 2)", "Name (copy 3)", …
    pub(crate) fn duplicate_active(&mut self) -> CircuitId {
        let index = self.active_index();
        self.duplicate_at_index(index)
            .expect("active circuit entry should always exist")
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

    #[allow(dead_code)]
    pub(crate) fn reorder(&mut self, src: usize, target: usize) {
        if src >= self.entries.len() || target == src || target == src + 1 {
            return;
        }
        let entry = self.entries.remove(src);
        let adjusted = if target > src { target - 1 } else { target };
        self.entries.insert(adjusted.min(self.entries.len()), entry);
        self.bump_updated_at();
    }

    pub(crate) fn swap_adjacent(&mut self, a: usize, b: usize) {
        debug_assert!(a.abs_diff(b) == 1);
        if a < self.entries.len() && b < self.entries.len() && a.abs_diff(b) == 1 {
            self.entries.swap(a, b);
        }
    }

    pub(crate) fn bump_updated_at(&mut self) {
        let active_id = self.active_id.clone();
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == active_id) {
            entry.updated_at = now_millis();
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
            name: self.next_default_circuit_name(None),
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
            name: self.next_default_circuit_name(Some(&id)),
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

    pub(crate) fn migrate_legacy_default_names(&mut self) -> bool {
        let mut changed = false;
        let now = now_millis();
        for index in 0..self.entries.len() {
            if self.entries[index].name == "Untitled"
                && is_auto_generated_circuit_id(&self.entries[index].id)
            {
                let id = self.entries[index].id.clone();
                self.entries[index].name = self.next_default_circuit_name(Some(&id));
                self.entries[index].updated_at = now;
                changed = true;
            }
        }
        changed
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

    fn duplicate_at_index(&mut self, index: usize) -> Option<CircuitId> {
        let source = self.entries.get(index)?.clone();
        let entry = CircuitEntry {
            id: self.fresh_id("circuit"),
            name: self.unique_copy_name(&source.name),
            circuit_json: source.circuit_json,
            updated_at: now_millis(),
        };
        let id = entry.id.clone();
        self.entries
            .insert((index + 1).min(self.entries.len()), entry);
        self.active_id = id.clone();
        self.bump_updated_at();
        Some(id)
    }

    fn unique_copy_name(&self, source_name: &str) -> String {
        let root = copy_name_root(source_name);
        let first = format!("{root} (copy)");
        if !self.entries.iter().any(|entry| entry.name == first) {
            return first;
        }
        let mut suffix = 2;
        loop {
            let candidate = format!("{root} (copy {suffix})");
            if !self.entries.iter().any(|entry| entry.name == candidate) {
                return candidate;
            }
            suffix += 1;
        }
    }

    fn ensure_non_empty(&mut self) {
        if self.entries.is_empty() {
            self.entries.push(CircuitEntry {
                id: "circuit-1".to_owned(),
                name: "Circuit 1".to_owned(),
                circuit_json: EMPTY_CIRCUIT_JSON.to_owned(),
                updated_at: now_millis(),
            });
        }
        if self.active_id.is_empty() {
            self.active_id = self.entries[0].id.clone();
        }
    }

    fn next_default_circuit_name(&self, excluding_id: Option<&str>) -> String {
        let next_index = self
            .entries
            .iter()
            .filter(|entry| excluding_id != Some(entry.id.as_str()))
            .filter_map(|entry| default_circuit_number(&entry.name))
            .max()
            .unwrap_or(0)
            + 1;
        format!("{DEFAULT_CIRCUIT_NAME_PREFIX}{next_index}")
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
        if library.migrate_legacy_default_names() {
            changed = true;
        }
        let active_json = if url_has_payload {
            if library.active().circuit_json == url_json {
                // Preserve the persisted active entry when the URL hash mirrors it.
                // Duplicates can intentionally share identical circuit JSON.
            } else if let Some(entry) = library
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
        if !take_external_library_dirty() {
            return;
        }
        if let PersistedLibraryState::Loaded(library) = load_persisted_library_state() {
            self.library = library;
            let active_json = self.library.active().circuit_json.clone();
            self.replace_editor_circuit(active_json, ctx);
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
            self.suppress_picker_hover_until_pointer_moves(ctx);
            self.replace_editor_circuit(circuit_json, ctx);
        }
        self.picker.close_submenu();
    }

    pub(crate) fn duplicate_active_circuit(&mut self, ctx: &egui::Context) {
        self.library.duplicate_active();
        let circuit_json = self.library.active().circuit_json.clone();
        self.picker.close();
        self.replace_editor_circuit(circuit_json, ctx);
    }

    pub(crate) fn move_circuit_entry_up(&mut self, index: usize, ctx: &egui::Context) {
        let focused_index = index.saturating_sub(1);
        self.library.move_up(index);
        self.picker.set_focused_index(focused_index);
        self.suppress_picker_hover_until_pointer_moves(ctx);
        persist_library(&self.library);
        self.picker.close_submenu();
    }

    pub(crate) fn move_circuit_entry_down(&mut self, index: usize, ctx: &egui::Context) {
        let focused_index = (index + 1).min(self.library.entries.len().saturating_sub(1));
        self.library.move_down(index);
        self.picker.set_focused_index(focused_index);
        self.suppress_picker_hover_until_pointer_moves(ctx);
        persist_library(&self.library);
        self.picker.close_submenu();
    }

    pub(crate) fn delete_circuit_entry(&mut self, index: usize, ctx: &egui::Context) {
        let was_active = self.library.active_index() == index;
        if self.library.delete(index).is_some() {
            let focused_index = index.min(self.library.entries.len().saturating_sub(1));
            self.picker.set_focused_index(focused_index);
            self.suppress_picker_hover_until_pointer_moves(ctx);
            if was_active {
                let circuit_json = self.library.active().circuit_json.clone();
                self.replace_editor_circuit(circuit_json, ctx);
            } else {
                persist_library(&self.library);
            }
        }
        self.picker.close_submenu();
    }

    pub(crate) fn suppress_picker_hover_until_pointer_moves(&mut self, ctx: &egui::Context) {
        self.picker_hover_suppressed_at = ctx.input(|input| {
            input
                .pointer
                .hover_pos()
                .or_else(|| input.pointer.interact_pos())
        });
    }

    pub(crate) fn picker_pointer_hover_suppressed(&mut self, ctx: &egui::Context) -> bool {
        let Some(suppressed_at) = self.picker_hover_suppressed_at else {
            return false;
        };
        let Some(current_pos) = ctx.input(|input| input.pointer.hover_pos()) else {
            self.picker_hover_suppressed_at = None;
            return false;
        };
        if current_pos.distance_sq(suppressed_at) > 1.0 {
            self.picker_hover_suppressed_at = None;
            false
        } else {
            true
        }
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

fn is_auto_generated_circuit_id(id: &str) -> bool {
    if id == "current" {
        return true;
    }
    id.strip_prefix("circuit-")
        .is_some_and(|suffix| suffix.parse::<usize>().is_ok())
}

fn default_circuit_number(name: &str) -> Option<usize> {
    let number = name.strip_prefix(DEFAULT_CIRCUIT_NAME_PREFIX)?;
    if number.is_empty() || !number.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    match number.parse::<usize>() {
        Ok(0) | Err(_) => None,
        Ok(number) => Some(number),
    }
}

fn copy_name_root(name: &str) -> &str {
    if let Some(root) = name.strip_suffix(" (copy)") {
        return root;
    }
    if let Some((root, suffix)) = name.rsplit_once(" (copy ") {
        if let Some(number) = suffix.strip_suffix(')') {
            if number.parse::<usize>().is_ok() {
                return root;
            }
        }
    }
    name
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
    use super::{CircuitEntry, CircuitLibrary, EMPTY_CIRCUIT_JSON};

    #[test]
    fn seed_contains_three_named_samples() {
        let library = CircuitLibrary::seed();

        assert_eq!(
            (
                library.entries.len(),
                library.active_id.as_str(),
                library.entries[0].name.as_str(),
                library.entries[1].name.as_str(),
                library.entries[2].name.as_str(),
            ),
            (3, "bell", "Bell state", "GHZ state", "QFT 4-qubit")
        );
    }

    #[test]
    fn current_and_new_circuits_use_incrementing_default_names() {
        let mut library = CircuitLibrary::seed();

        library.set_active_current_circuit(EMPTY_CIRCUIT_JSON.to_owned());
        let initial_name = library.active().name.clone();

        library.set_active_current_circuit(r#"{"cols":[["H"]]}"#.to_owned());
        let edited_name = library.active().name.clone();

        let first = library.create_new().clone();
        let second = library.create_new().clone();

        assert_eq!(
            (
                initial_name.as_str(),
                edited_name.as_str(),
                first.name.as_str(),
                second.name.as_str()
            ),
            ("Circuit 1", "Circuit 1", "Circuit 2", "Circuit 3")
        );
    }

    #[test]
    fn legacy_auto_untitled_entries_migrate_to_numbered_circuits() {
        let mut library = CircuitLibrary::from_entries(
            vec![
                CircuitEntry {
                    id: "current".to_owned(),
                    name: "Untitled".to_owned(),
                    circuit_json: EMPTY_CIRCUIT_JSON.to_owned(),
                    updated_at: 0,
                },
                CircuitEntry {
                    id: "circuit-8".to_owned(),
                    name: "Untitled".to_owned(),
                    circuit_json: EMPTY_CIRCUIT_JSON.to_owned(),
                    updated_at: 0,
                },
                CircuitEntry {
                    id: "ckt_saved".to_owned(),
                    name: "Untitled".to_owned(),
                    circuit_json: EMPTY_CIRCUIT_JSON.to_owned(),
                    updated_at: 0,
                },
            ],
            "current".to_owned(),
        );

        let migrated = library.migrate_legacy_default_names();
        assert_eq!(
            (
                migrated,
                library.entries[0].name.as_str(),
                library.entries[1].name.as_str(),
                library.entries[2].name.as_str(),
            ),
            (true, "Circuit 1", "Circuit 2", "Untitled")
        );
    }

    #[test]
    fn update_and_set_active_keep_canonical_json() {
        let mut library = CircuitLibrary::seed();

        library.set_active("ghz".to_owned());
        library.update_active(EMPTY_CIRCUIT_JSON.to_owned());

        assert_eq!(
            (
                library.active().id.as_str(),
                library.active().circuit_json.as_str()
            ),
            ("ghz", EMPTY_CIRCUIT_JSON)
        );
    }

    #[test]
    fn duplicate_move_and_delete_preserve_active_invariant() {
        let mut library = CircuitLibrary::seed();

        let duplicated = library.duplicate(1).expect("duplicate").clone();
        let after_duplicate = (
            duplicated.name.clone(),
            duplicated.updated_at != 0,
            library.active_id.clone(),
            library.entries[2].id.clone(),
        );

        library.move_up(2);
        let after_move_up_id = library.entries[1].id.clone();
        library.move_down(1);
        let after_move_down_id = library.entries[2].id.clone();

        library.delete(2);
        assert_eq!(
            (
                after_duplicate.0.as_str(),
                after_duplicate.1,
                after_duplicate.2.as_str(),
                after_duplicate.3.as_str(),
                after_move_up_id.as_str(),
                after_move_down_id.as_str(),
                library.active_id.as_str(),
                library.entries.len(),
            ),
            (
                "GHZ state (copy)",
                true,
                duplicated.id.as_str(),
                duplicated.id.as_str(),
                duplicated.id.as_str(),
                duplicated.id.as_str(),
                "bell",
                3,
            )
        );
    }

    #[test]
    fn duplicate_active_inserts_after_active_and_numbers_copy_names() {
        let mut library = CircuitLibrary::seed();
        library.set_active("bell".to_owned());
        if let Some(entry) = library.entries.iter_mut().find(|entry| entry.id == "bell") {
            entry.updated_at = 0;
        }

        let first_id = library.duplicate_active();
        let first_snapshot = (
            library.entries[1].id.clone(),
            library.active_id.clone(),
            library.entries[1].name.clone(),
            library.entries[1].circuit_json.clone(),
            library.entries[0].circuit_json.clone(),
            library.active().updated_at != 0,
        );

        let second_id = library.duplicate_active();
        let second_snapshot = (
            library.entries[2].id.clone(),
            library.entries[2].name.clone(),
        );

        let third_id = library.duplicate_active();
        assert_eq!(
            (
                first_snapshot.0.as_str(),
                first_snapshot.1.as_str(),
                first_snapshot.2.as_str(),
                first_snapshot.3.as_str(),
                first_snapshot.4.as_str(),
                first_snapshot.5,
                second_snapshot.0.as_str(),
                second_snapshot.1.as_str(),
                library.entries[3].id.as_str(),
                library.entries[3].name.as_str(),
            ),
            (
                first_id.as_str(),
                first_id.as_str(),
                "Bell state (copy)",
                first_snapshot.4.as_str(),
                first_snapshot.4.as_str(),
                true,
                second_id.as_str(),
                "Bell state (copy 2)",
                third_id.as_str(),
                "Bell state (copy 3)",
            )
        );
    }

    #[test]
    fn duplicate_active_skips_existing_copy_name_collisions() {
        let mut library = CircuitLibrary::seed();
        library.entries[1].name = "Bell state (copy)".to_owned();
        library.entries[2].name = "Bell state (copy 2)".to_owned();
        library.set_active("bell".to_owned());

        let id = library.duplicate_active();

        assert_eq!(
            (library.active_id.as_str(), library.entries[1].name.as_str()),
            (id.as_str(), "Bell state (copy 3)")
        );
    }

    #[test]
    fn reorder_moves_by_insertion_index_and_preserves_active_id() {
        let mut library = CircuitLibrary::seed();
        library.set_active("ghz".to_owned());

        library.reorder(0, 3);

        assert_eq!(
            (
                library
                    .entries
                    .iter()
                    .map(|entry| entry.id.as_str())
                    .collect::<Vec<_>>(),
                library.active_id.as_str(),
            ),
            (vec!["ghz", "qft-4", "bell"], "ghz")
        );
    }

    #[test]
    fn reorder_ignores_no_ops_and_out_of_bounds_source() {
        let mut library = CircuitLibrary::seed();
        let original = library.clone();

        library.reorder(2, 2);
        let after_same_index = library.clone();

        library.reorder(2, 3);
        let after_endpoint_noop = library.clone();

        library.reorder(99, 0);
        assert_eq!(
            (after_same_index, after_endpoint_noop, library),
            (original.clone(), original.clone(), original)
        );
    }

    #[test]
    fn swap_adjacent_swaps_without_touching_active_timestamp() {
        let mut library = CircuitLibrary::seed();
        library.set_active("ghz".to_owned());
        if let Some(entry) = library.entries.iter_mut().find(|entry| entry.id == "ghz") {
            entry.updated_at = 0;
        }

        library.swap_adjacent(1, 2);

        assert_eq!(
            (
                library
                    .entries
                    .iter()
                    .map(|entry| entry.id.as_str())
                    .collect::<Vec<_>>(),
                library.active_id.as_str(),
                library.active().updated_at,
            ),
            (vec!["bell", "qft-4", "ghz"], "ghz", 0)
        );
    }
}
