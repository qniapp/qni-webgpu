//! Startup reconciliation between URL payloads and persisted library state.

use super::storage::{load_persisted_library_state, persist_library, PersistedLibraryState};
use super::CircuitLibrary;

pub(crate) fn for_startup(url_json: String, url_has_payload: bool) -> (CircuitLibrary, String) {
    let load_state = load_persisted_library_state();
    let loaded = matches!(&load_state, PersistedLibraryState::Loaded(_));
    let can_persist = !matches!(&load_state, PersistedLibraryState::Invalid);
    let mut changed = false;
    let mut library = match load_state {
        PersistedLibraryState::Loaded(library) => library,
        PersistedLibraryState::Missing | PersistedLibraryState::Invalid => CircuitLibrary::seed(),
    };
    if library.migrate_legacy_default_names() {
        changed = true;
    }
    // 組み込みサンプルの定義（JSON / 名前）をコード側で更新したとき、初回シード後に
    // 保存された既存ライブラリにも反映させる。ユーザー回路には触れない。
    if library.reconcile_samples() {
        changed = true;
    }
    let active_json = if url_has_payload {
        if library.resolve_startup_url_payload(url_json.clone()) {
            changed = true;
        }
        library.active().circuit_json.clone()
    } else if loaded {
        library.active().circuit_json.clone()
    } else {
        library.set_active_current_circuit(url_json.clone());
        changed = true;
        library.active().circuit_json.clone()
    };
    if can_persist && changed {
        persist_library(&library);
    }
    (library, active_json)
}
