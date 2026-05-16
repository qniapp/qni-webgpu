//! QniApp integration for circuit library operations.

use eframe::egui;

use super::storage::{
    load_persisted_library_state, persist_library, take_external_library_dirty,
    PersistedLibraryState,
};
use super::test_hooks::{publish_library_snapshot, take_seeded_library};
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
