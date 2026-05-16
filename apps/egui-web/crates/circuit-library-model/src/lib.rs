//! Domain model for the local circuit library.

use serde::{Deserialize, Serialize};

pub const EMPTY_CIRCUIT_JSON: &str = r#"{"cols":[]}"#;

pub type CircuitId = String;

const DEFAULT_CIRCUIT_NAME_PREFIX: &str = "Circuit ";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CircuitEntry {
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
pub struct CircuitLibrary {
    pub entries: Vec<CircuitEntry>,
    pub active_id: CircuitId,
}

impl Default for CircuitLibrary {
    fn default() -> Self {
        Self::seed()
    }
}

impl CircuitLibrary {
    pub fn seed() -> Self {
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

    pub fn from_entries(entries: Vec<CircuitEntry>, active_id: CircuitId) -> Self {
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

    pub fn active_index(&self) -> usize {
        self.entries
            .iter()
            .position(|entry| entry.id == self.active_id)
            .unwrap_or(0)
    }

    pub fn active(&self) -> &CircuitEntry {
        &self.entries[self.active_index()]
    }

    pub fn update_active(&mut self, circuit_json: String) {
        let active_id = self.active_id.clone();
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == active_id) {
            entry.circuit_json = circuit_json;
            entry.updated_at = now_millis();
        }
    }

    pub fn set_active(&mut self, id: CircuitId) -> &CircuitEntry {
        if self.entries.iter().any(|entry| entry.id == id) {
            self.active_id = id;
        }
        self.active()
    }

    pub fn set_active_index(&mut self, index: usize) -> &CircuitEntry {
        if let Some(entry) = self.entries.get(index) {
            self.active_id = entry.id.clone();
        }
        self.active()
    }

    pub fn rename(&mut self, id: &str, name: &str) {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return;
        }
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == id) {
            entry.name = trimmed.to_owned();
            entry.updated_at = now_millis();
        }
    }

    pub fn duplicate(&mut self, index: usize) -> Option<&CircuitEntry> {
        self.duplicate_at_index(index)?;
        Some(self.active())
    }

    /// Insert a copy of the active entry right after it; switch active to the
    /// new entry and bump its timestamp. Copy names follow the picker/toolbar
    /// contract: "Name (copy)", then "Name (copy 2)", "Name (copy 3)", …
    pub fn duplicate_active(&mut self) -> CircuitId {
        let index = self.active_index();
        self.duplicate_at_index(index)
            .expect("active circuit entry should always exist")
    }

    pub fn move_up(&mut self, index: usize) {
        if index > 0 && index < self.entries.len() {
            self.entries.swap(index - 1, index);
        }
    }

    pub fn move_down(&mut self, index: usize) {
        if index + 1 < self.entries.len() {
            self.entries.swap(index, index + 1);
        }
    }

    #[allow(dead_code)]
    pub fn reorder(&mut self, src: usize, target: usize) {
        if src >= self.entries.len() || target == src || target == src + 1 {
            return;
        }
        let entry = self.entries.remove(src);
        let adjusted = if target > src { target - 1 } else { target };
        self.entries.insert(adjusted.min(self.entries.len()), entry);
        self.bump_updated_at();
    }

    pub fn move_to_slot(&mut self, src: usize, slot: usize) {
        if src >= self.entries.len() || slot >= self.entries.len() || src == slot {
            return;
        }
        let entry = self.entries.remove(src);
        self.entries.insert(slot, entry);
    }

    pub fn swap_adjacent(&mut self, a: usize, b: usize) {
        debug_assert!(a.abs_diff(b) == 1);
        if a < self.entries.len() && b < self.entries.len() && a.abs_diff(b) == 1 {
            self.entries.swap(a, b);
        }
    }

    pub fn bump_updated_at(&mut self) {
        let active_id = self.active_id.clone();
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == active_id) {
            entry.updated_at = now_millis();
        }
    }

    pub fn delete(&mut self, index: usize) -> Option<&CircuitEntry> {
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

    pub fn create_new(&mut self) -> &CircuitEntry {
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

    pub fn set_active_current_circuit(&mut self, circuit_json: String) {
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

    pub fn migrate_legacy_default_names(&mut self) -> bool {
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

    pub fn to_test_json(&self) -> String {
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
pub fn now_millis() -> u64 {
    js_sys::Date::now().max(0.0) as u64
}

#[cfg(not(target_arch = "wasm32"))]
pub fn now_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
