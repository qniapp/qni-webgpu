//! Domain model for the local circuit library.

use serde::{Deserialize, Serialize};

pub const EMPTY_CIRCUIT_JSON: &str = r#"{"cols":[]}"#;
const GROVER_SEARCH_JSON: &str = concat!(
    r#"{"cols":["#,
    r#"["X","X","X","X","X"],["H","H","H","H","H"],["Chance5"],"#,
    r#"["Z","•","◦","•","•"],["H","H","H","H",1],["•","•","•","•","X"],["H","H","H","H",1],["Chance5"],"#,
    r#"["Z","•","◦","•","•"],["H","H","H","H",1],["•","•","•","•","X"],["H","H","H","H",1],["Chance5"],"#,
    r#"["Z","•","◦","•","•"],["H","H","H","H",1],["•","•","•","•","X"],["H","H","H","H",1],["Chance5"],"#,
    r#"["Z","•","◦","•","•"],["H","H","H","H",1],["•","•","•","•","X"],["H","H","H","H",1],["Chance5"]"#,
    r#"]}"#,
);

pub type CircuitId = String;

const DEFAULT_CIRCUIT_NAME_PREFIX: &str = "Circuit ";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CircuitEntry {
    pub id: CircuitId,
    pub name: String,
    pub circuit_json: String,
    pub updated_at: u64,
    pub origin: CircuitOrigin,
}

impl Default for CircuitEntry {
    fn default() -> Self {
        Self::user(
            String::new(),
            String::new(),
            EMPTY_CIRCUIT_JSON.to_owned(),
            0,
            false,
        )
    }
}

impl CircuitEntry {
    pub fn sample(id: &str, name: &str, circuit_json: &str, updated_at: u64) -> Self {
        Self {
            id: id.to_owned(),
            name: name.to_owned(),
            circuit_json: circuit_json.to_owned(),
            updated_at,
            origin: CircuitOrigin::Sample {
                origin_id: id.to_owned(),
            },
        }
    }

    pub fn user(
        id: CircuitId,
        name: String,
        circuit_json: String,
        updated_at: u64,
        locked: bool,
    ) -> Self {
        Self {
            id,
            name,
            circuit_json,
            updated_at,
            origin: CircuitOrigin::User { locked },
        }
    }

    pub fn kind(&self) -> CircuitKind {
        match self.origin {
            CircuitOrigin::Sample { .. } => CircuitKind::Example,
            CircuitOrigin::User { .. } => CircuitKind::My,
        }
    }

    pub fn locked(&self) -> bool {
        match self.origin {
            CircuitOrigin::Sample { .. } => true,
            CircuitOrigin::User { locked } => locked,
        }
    }

    pub fn is_sample(&self) -> bool {
        matches!(self.origin, CircuitOrigin::Sample { .. })
    }

    pub fn is_user(&self) -> bool {
        matches!(self.origin, CircuitOrigin::User { .. })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CircuitOrigin {
    Sample { origin_id: String },
    User { locked: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CircuitKind {
    Example,
    My,
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
        let entries = sample_entries(now_millis());
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

    pub fn migrate_v1_entries(entries: Vec<CircuitEntry>, active_id: Option<CircuitId>) -> Self {
        migrate_v1_entries(entries, active_id)
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

    pub fn active_kind(&self) -> CircuitKind {
        self.active().kind()
    }

    pub fn active_locked(&self) -> bool {
        self.active().locked()
    }

    pub fn entry_locked_by_id(&self, id: &str) -> bool {
        self.entries
            .iter()
            .find(|entry| entry.id == id)
            .is_some_and(CircuitEntry::locked)
    }

    pub fn update_active(&mut self, circuit_json: String) {
        if self.active_locked() {
            return;
        }
        self.update_active_unchecked(circuit_json);
    }

    pub fn update_active_unchecked(&mut self, circuit_json: String) {
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
        if self.entry_locked_by_id(id) {
            return;
        }
        self.rename_unchecked(id, name);
    }

    pub fn rename_unchecked(&mut self, id: &str, name: &str) {
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

    /// Insert a copy of the active entry into the My section; switch active to
    /// the new unlocked User entry and bump its timestamp. Copy names follow
    /// the picker/toolbar contract: "Name (copy)", then "Name (copy 2)", …
    pub fn duplicate_active(&mut self) -> CircuitId {
        let index = self.active_index();
        self.duplicate_at_index(index)
            .expect("active circuit entry should always exist")
    }

    pub fn move_up(&mut self, index: usize) {
        let Some((kind, slot)) = self.section_slot_for_index(index) else {
            return;
        };
        if slot == 0 {
            return;
        }
        self.move_section_entry_to_slot(index, slot - 1, kind);
    }

    pub fn move_down(&mut self, index: usize) {
        let Some((kind, slot)) = self.section_slot_for_index(index) else {
            return;
        };
        if slot + 1 >= self.section_indices(kind).len() {
            return;
        }
        self.move_section_entry_to_slot(index, slot + 1, kind);
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
        let Some((kind, source_slot)) = self.section_slot_for_index(src) else {
            return;
        };
        let section_len = self.section_indices(kind).len();
        if slot >= self.entries.len() || src == slot || section_len == 0 {
            return;
        }
        let target_slot = self
            .section_slot_for_index(slot)
            .and_then(|(target_kind, target_slot)| (target_kind == kind).then_some(target_slot))
            .unwrap_or_else(|| source_slot.min(section_len.saturating_sub(1)));
        self.move_section_entry_to_slot(src, target_slot, kind);
    }

    pub fn move_user_to_slot(&mut self, src_index: usize, target_user_slot: usize) {
        self.move_section_entry_to_slot(src_index, target_user_slot, CircuitKind::My);
    }

    pub fn swap_adjacent(&mut self, a: usize, b: usize) {
        debug_assert!(a.abs_diff(b) == 1);
        if a < self.entries.len()
            && b < self.entries.len()
            && a.abs_diff(b) == 1
            && self.entries[a].is_user()
            && self.entries[b].is_user()
        {
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
        let id = self.entries.get(index)?.id.clone();
        self.delete_by_id(&id)
    }

    pub fn delete_by_id(&mut self, id: &str) -> Option<&CircuitEntry> {
        if self.entries.len() <= 1 || self.entry_locked_by_id(id) {
            return None;
        }
        let index = self.entries.iter().position(|entry| entry.id == id)?;
        self.entries.remove(index);
        if self.active_id == id {
            self.active_id = self
                .entries
                .iter()
                .find(|entry| entry.is_user())
                .or_else(|| self.entries.first())
                .map(|entry| entry.id.clone())
                .unwrap_or_default();
        }
        Some(self.active())
    }

    pub fn create_new(&mut self) -> &CircuitEntry {
        let entry = CircuitEntry::user(
            self.fresh_id("circuit"),
            self.next_default_circuit_name(None),
            EMPTY_CIRCUIT_JSON.to_owned(),
            now_millis(),
            false,
        );
        let id = entry.id.clone();
        self.entries.push(entry);
        self.set_active(id)
    }

    pub fn set_active_current_circuit(&mut self, circuit_json: String) {
        self.set_active_current_circuit_with_lock_policy(circuit_json, false);
    }

    pub fn set_active_current_circuit_preserving_locked(&mut self, circuit_json: String) {
        self.set_active_current_circuit_with_lock_policy(circuit_json, true);
    }

    pub fn toggle_active_lock(&mut self) -> bool {
        let active_id = self.active_id.clone();
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == active_id) {
            if let CircuitOrigin::User { locked } = &mut entry.origin {
                *locked = !*locked;
                entry.updated_at = now_millis();
                return true;
            }
        }
        false
    }

    pub fn migrate_legacy_default_names(&mut self) -> bool {
        let mut changed = false;
        let now = now_millis();
        for index in 0..self.entries.len() {
            if self.entries[index].is_user()
                && self.entries[index].name == "Untitled"
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

    pub fn resolve_startup_url_payload(&mut self, url_json: String) -> bool {
        if self.active().circuit_json == url_json {
            return false;
        }
        if let Some(sample_id) = self.find_canonical_sample(&url_json) {
            let changed = self.active_id != sample_id;
            self.active_id = sample_id;
            return changed;
        }
        if let Some(user_id) = self.find_user_entry_with_json(&url_json) {
            let changed = self.active_id != user_id;
            self.active_id = user_id;
            return changed;
        }
        let old_active = self.active_id.clone();
        self.set_active_current_circuit_preserving_locked(url_json);
        self.active_id != old_active
    }

    pub fn find_canonical_sample(&self, circuit_json: &str) -> Option<CircuitId> {
        sample_entries(0)
            .into_iter()
            .find(|entry| entry.circuit_json == circuit_json)
            .map(|entry| entry.id)
    }

    pub fn find_user_entry_with_json(&self, circuit_json: &str) -> Option<CircuitId> {
        self.entries
            .iter()
            .find(|entry| entry.is_user() && entry.circuit_json == circuit_json)
            .map(|entry| entry.id.clone())
    }

    pub fn user_indices(&self) -> Vec<usize> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| entry.is_user().then_some(index))
            .collect()
    }

    pub fn sample_indices(&self) -> Vec<usize> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| entry.is_sample().then_some(index))
            .collect()
    }

    pub fn user_slot_for_index(&self, index: usize) -> Option<usize> {
        self.user_indices()
            .iter()
            .position(|user_index| *user_index == index)
    }

    pub fn section_slot_for_index(&self, index: usize) -> Option<(CircuitKind, usize)> {
        let kind = self.entries.get(index)?.kind();
        self.section_indices(kind)
            .iter()
            .position(|entry_index| *entry_index == index)
            .map(|slot| (kind, slot))
    }

    pub fn to_test_json(&self) -> String {
        let entries = self
            .entries
            .iter()
            .map(|entry| {
                format!(
                    r#"{{"id":"{}","name":"{}","circuit_json":"{}","updated_at":{},"locked":{},"origin":{}}}"#,
                    json_escape(&entry.id),
                    json_escape(&entry.name),
                    json_escape(&entry.circuit_json),
                    entry.updated_at,
                    entry.locked(),
                    origin_test_json(&entry.origin),
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            r#"{{"entries":[{}],"active_id":"{}","active_locked":{},"active_kind":"{}"}}"#,
            entries,
            json_escape(&self.active_id),
            self.active_locked(),
            match self.active_kind() {
                CircuitKind::Example => "example",
                CircuitKind::My => "my",
            },
        )
    }

    fn duplicate_at_index(&mut self, index: usize) -> Option<CircuitId> {
        let source = self.entries.get(index)?.clone();
        let entry = CircuitEntry::user(
            self.fresh_id("circuit"),
            self.unique_copy_name(&source.name),
            source.circuit_json,
            now_millis(),
            false,
        );
        let id = entry.id.clone();
        self.entries.push(entry);
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
            self.entries.push(CircuitEntry::user(
                "circuit-1".to_owned(),
                "Circuit 1".to_owned(),
                EMPTY_CIRCUIT_JSON.to_owned(),
                now_millis(),
                false,
            ));
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

    fn set_active_current_circuit_with_lock_policy(
        &mut self,
        circuit_json: String,
        preserve_locked_current: bool,
    ) {
        let mut id = "current".to_owned();
        if preserve_locked_current && self.entry_locked_by_id(&id) {
            id = self.fresh_id("current");
        }
        let entry = CircuitEntry::user(
            id.clone(),
            self.next_default_circuit_name(Some(&id)),
            circuit_json,
            now_millis(),
            false,
        );
        if let Some(existing) = self.entries.iter_mut().find(|entry| entry.id == id) {
            *existing = entry;
        } else {
            self.entries.push(entry);
        }
        self.active_id = id;
        self.normalize_sample_user_order();
    }

    fn normalize_sample_user_order(&mut self) {
        let samples = self.section_entries(CircuitKind::Example);
        let users = self.section_entries(CircuitKind::My);
        self.entries = samples.into_iter().chain(users).collect();
    }

    fn move_section_entry_to_slot(
        &mut self,
        src_index: usize,
        target_slot: usize,
        kind: CircuitKind,
    ) {
        let section_indices = self.section_indices(kind);
        let Some(source_slot) = section_indices.iter().position(|index| *index == src_index) else {
            return;
        };
        if target_slot >= section_indices.len() || source_slot == target_slot {
            return;
        }
        let mut samples = self.section_entries(CircuitKind::Example);
        let mut users = self.section_entries(CircuitKind::My);
        let section = match kind {
            CircuitKind::Example => &mut samples,
            CircuitKind::My => &mut users,
        };
        let moved = section.remove(source_slot);
        section.insert(target_slot, moved);
        self.entries = samples.into_iter().chain(users).collect();
    }

    fn section_indices(&self, kind: CircuitKind) -> Vec<usize> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| (entry.kind() == kind).then_some(index))
            .collect()
    }

    fn section_entries(&self, kind: CircuitKind) -> Vec<CircuitEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.kind() == kind)
            .cloned()
            .collect()
    }
}

fn sample_entries(updated_at: u64) -> Vec<CircuitEntry> {
    vec![
        CircuitEntry::sample(
            "bell",
            "Bell state",
            r#"{"cols":[["H"],["•","X"]]}"#,
            updated_at,
        ),
        CircuitEntry::sample(
            "ghz",
            "GHZ state",
            r#"{"cols":[["H"],["•","X"],["•",1,"X"]]}"#,
            updated_at,
        ),
        CircuitEntry::sample("qft-4", "QFT 4-qubit", r#"{"cols":[["QFT4"]]}"#, updated_at),
        grover_search_entry(updated_at),
    ]
}

fn grover_search_entry(updated_at: u64) -> CircuitEntry {
    CircuitEntry::sample(
        "grover-search",
        "Grover Search",
        GROVER_SEARCH_JSON,
        updated_at,
    )
}

fn migrate_v1_entries(entries: Vec<CircuitEntry>, active_id: Option<CircuitId>) -> CircuitLibrary {
    let mut migrated = CircuitLibrary::seed();
    let mut active_remap = Vec::<(CircuitId, CircuitId)>::new();
    let samples = sample_entries(0);
    for entry in entries {
        if let Some(sample) = samples.iter().find(|sample| sample.id == entry.id) {
            if sample.name == entry.name && sample.circuit_json == entry.circuit_json {
                active_remap.push((entry.id.clone(), sample.id.clone()));
                continue;
            }
            let next_id = unique_id(&migrated.entries, &format!("{}-user-edit", entry.id), "-");
            let next_name = unique_edited_name(&migrated.entries, &entry.name);
            active_remap.push((entry.id.clone(), next_id.clone()));
            migrated.entries.push(CircuitEntry::user(
                next_id,
                next_name,
                entry.circuit_json,
                entry.updated_at,
                false,
            ));
        } else {
            let next_id = unique_id(&migrated.entries, &entry.id, "-");
            active_remap.push((entry.id.clone(), next_id.clone()));
            migrated.entries.push(CircuitEntry::user(
                next_id,
                entry.name,
                entry.circuit_json,
                entry.updated_at,
                false,
            ));
        }
    }
    if let Some(active_id) = active_id {
        if let Some((_, remapped)) = active_remap.iter().find(|(old, _)| old == &active_id) {
            migrated.active_id = remapped.clone();
        }
    }
    if !migrated
        .entries
        .iter()
        .any(|entry| entry.id == migrated.active_id)
    {
        migrated.active_id = migrated.entries[0].id.clone();
    }
    migrated
}

fn unique_id(entries: &[CircuitEntry], preferred: &str, separator: &str) -> String {
    if !entries.iter().any(|entry| entry.id == preferred) {
        return preferred.to_owned();
    }
    let mut suffix = 2;
    loop {
        let candidate = format!("{preferred}{separator}{suffix}");
        if !entries.iter().any(|entry| entry.id == candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

fn unique_edited_name(entries: &[CircuitEntry], name: &str) -> String {
    let first = format!("{name} (edited)");
    if !entries.iter().any(|entry| entry.name == first) {
        return first;
    }
    let mut suffix = 2;
    loop {
        let candidate = format!("{name} (edited {suffix})");
        if !entries.iter().any(|entry| entry.name == candidate) {
            return candidate;
        }
        suffix += 1;
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

fn origin_test_json(origin: &CircuitOrigin) -> String {
    match origin {
        CircuitOrigin::Sample { origin_id } => format!(
            r#"{{"kind":"sample","origin_id":"{}"}}"#,
            json_escape(origin_id)
        ),
        CircuitOrigin::User { locked } => format!(r#"{{"kind":"user","locked":{locked}}}"#),
    }
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
