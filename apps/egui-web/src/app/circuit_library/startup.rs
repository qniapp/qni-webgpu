//! Startup reconciliation between URL payloads and persisted library state.

use super::storage::{load_persisted_library_state, persist_library, PersistedLibraryState};
use super::CircuitLibrary;

impl CircuitLibrary {
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
