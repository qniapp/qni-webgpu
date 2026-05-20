use eframe::egui;

use crate::app::QniApp;

#[derive(Clone, Debug)]
pub(super) enum PickerAction {
    Select(usize),
    Create,
    OpenSubmenu(usize),
    CloseSubmenuFromTrigger { suppress_click: bool },
    StartRename(usize),
    UpdateRenameDraft { entry_id: String, draft: String },
    CommitRename { entry_id: String, draft: String },
    CancelRename,
    Duplicate(usize),
    MoveUp(usize),
    MoveDown(usize),
    Delete(usize),
}

impl QniApp {
    pub(super) fn apply_picker_action(&mut self, action: PickerAction, ctx: &egui::Context) {
        match action {
            PickerAction::Select(index) => self.select_circuit_entry(index, ctx),
            PickerAction::Create => self.create_new_circuit(ctx),
            PickerAction::OpenSubmenu(index) => self.picker.toggle_submenu(index),
            PickerAction::CloseSubmenuFromTrigger { suppress_click } => {
                self.picker.close_submenu();
                if suppress_click {
                    self.picker_submenu_toggle_suppressed_until_release = true;
                }
            }
            PickerAction::StartRename(index) => self.start_circuit_rename(index),
            PickerAction::UpdateRenameDraft { entry_id, draft } => {
                self.picker.update_rename_draft(&entry_id, draft);
            }
            PickerAction::CommitRename { entry_id, draft } => {
                self.commit_circuit_rename(&entry_id, draft);
            }
            PickerAction::CancelRename => self.picker.cancel_rename(),
            PickerAction::Duplicate(index) => self.duplicate_circuit_entry(index, ctx),
            PickerAction::MoveUp(index) => self.move_circuit_entry_up(index, ctx),
            PickerAction::MoveDown(index) => self.move_circuit_entry_down(index, ctx),
            PickerAction::Delete(index) => self.delete_circuit_entry(index, ctx),
        }
        ctx.request_repaint();
    }
}
