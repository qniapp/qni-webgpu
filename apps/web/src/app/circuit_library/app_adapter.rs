//! QniApp integration for circuit library operations.

use eframe::egui;

use super::storage::{
    load_persisted_library_state, persist_library, take_external_library_dirty,
    PersistedLibraryState,
};
use super::test_hooks::{publish_library_snapshot, take_pending_url_payload, take_seeded_library};
use crate::app::circuit_history::CircuitRevision;
use crate::app::QniApp;

impl QniApp {
    pub(crate) fn active_circuit_name(&self) -> &str {
        &self.library.active().name
    }

    pub(crate) fn apply_pending_circuit_library_seed(&mut self, ctx: &egui::Context) {
        if let Some(library) = take_seeded_library() {
            self.library = library;
            let active_json = self.library.active().circuit_json.clone();
            self.load_selected_circuit(active_json, ctx);
        }
    }

    pub(crate) fn apply_external_circuit_library_update(&mut self, ctx: &egui::Context) {
        if !take_external_library_dirty() {
            return;
        }
        if let PersistedLibraryState::Loaded(library) = load_persisted_library_state() {
            self.library = library;
            let active_json = self.library.active().circuit_json.clone();
            self.load_selected_circuit(active_json, ctx);
        }
    }

    pub(crate) fn apply_pending_url_payload(&mut self, ctx: &egui::Context) {
        if let Some(payload) = take_pending_url_payload() {
            self.apply_url_payload(payload, ctx);
        }
    }

    pub(crate) fn publish_circuit_library_snapshot(&self) {
        publish_library_snapshot(&self.library);
    }

    pub(crate) fn select_circuit_entry(&mut self, index: usize, ctx: &egui::Context) {
        let circuit_json = self.library.set_active_index(index).circuit_json.clone();
        self.picker.close();
        self.load_selected_circuit(circuit_json, ctx);
        persist_library(&self.library);
    }

    pub(crate) fn create_new_circuit(&mut self, ctx: &egui::Context) {
        let circuit_json = self.library.create_new().circuit_json.clone();
        self.picker.close();
        self.load_selected_circuit(circuit_json, ctx);
        persist_library(&self.library);
    }

    pub(crate) fn duplicate_circuit_entry(&mut self, index: usize, ctx: &egui::Context) {
        if let Some(entry) = self.library.duplicate(index) {
            let circuit_json = entry.circuit_json.clone();
            let focused_index = self.library.active_index();
            self.picker.set_focused_index(focused_index);
            self.suppress_picker_hover_until_pointer_moves(ctx);
            self.load_selected_circuit(circuit_json, ctx);
            persist_library(&self.library);
        }
        self.picker.close_submenu();
    }

    pub(crate) fn duplicate_active_circuit(&mut self, ctx: &egui::Context) {
        self.library.duplicate_active();
        let circuit_json = self.library.active().circuit_json.clone();
        self.picker.close();
        self.load_selected_circuit(circuit_json, ctx);
        persist_library(&self.library);
    }

    pub(crate) fn toggle_circuit_lock(&mut self) {
        if self.library.toggle_active_lock() {
            persist_library(&self.library);
        }
    }

    pub(crate) fn move_circuit_entry_up(&mut self, index: usize, ctx: &egui::Context) {
        let moved_id = self
            .library
            .entries
            .get(index)
            .map(|entry| entry.id.clone());
        self.library.move_up(index);
        if let Some(focused_index) =
            moved_id.and_then(|id| self.library.entries.iter().position(|entry| entry.id == id))
        {
            self.picker.set_focused_index(focused_index);
        }
        self.suppress_picker_hover_until_pointer_moves(ctx);
        persist_library(&self.library);
        self.picker.close_submenu();
    }

    pub(crate) fn move_circuit_entry_down(&mut self, index: usize, ctx: &egui::Context) {
        let moved_id = self
            .library
            .entries
            .get(index)
            .map(|entry| entry.id.clone());
        self.library.move_down(index);
        if let Some(focused_index) =
            moved_id.and_then(|id| self.library.entries.iter().position(|entry| entry.id == id))
        {
            self.picker.set_focused_index(focused_index);
        }
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
                self.load_selected_circuit(circuit_json, ctx);
            }
            persist_library(&self.library);
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
            if !entry.locked() {
                self.picker.start_rename(entry);
            }
        }
    }

    pub(crate) fn commit_circuit_rename(&mut self, entry_id: &str, next_name: String) {
        self.library.rename(entry_id, &next_name);
        persist_library(&self.library);
        self.picker.finish_rename();
    }

    fn replace_editor_circuit(&mut self, circuit_json: String, ctx: &egui::Context) {
        self.circuit_revision = CircuitRevision::starting_at(circuit_json.clone());
        self.load_circuit_json_into_editor(&circuit_json, ctx);
    }

    fn load_selected_circuit(&mut self, circuit_json: String, ctx: &egui::Context) {
        self.replace_editor_circuit(circuit_json.clone(), ctx);
        crate::url_circuit::write_circuit_to_url(&circuit_json);
    }
}
