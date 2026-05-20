use eframe::egui;

use crate::app::circuit_library::CircuitEntry;
use crate::app::circuit_picker_state::PickerState;
use crate::app::QniApp;
use crate::colors::Colors;

use super::action::PickerAction;
use super::constants::{ITEM_PAD_X, ITEM_RADIUS};

impl QniApp {
    pub(super) fn show_rename_row(
        &mut self,
        ui: &mut egui::Ui,
        colors: &Colors,
        rect: egui::Rect,
        entry: &CircuitEntry,
        actions: &mut Vec<PickerAction>,
    ) {
        ui.painter().rect(
            rect,
            egui::CornerRadius::same(ITEM_RADIUS),
            colors.surface,
            egui::Stroke::new(1.5, colors.semantic_on), // Flexoki blue-600.
            egui::StrokeKind::Outside,
        );
        let (mut draft, select_all) = match &mut self.picker {
            PickerState::Open {
                renaming: Some(rename),
                ..
            } if rename.entry_id == entry.id => {
                let select_all = rename.select_all_pending;
                rename.select_all_pending = false;
                (rename.draft.clone(), select_all)
            }
            _ => (entry.name.clone(), false),
        };
        let edit_rect = rect.shrink2(egui::vec2(ITEM_PAD_X, 0.0));
        let text_id = ui.make_persistent_id(("circuit-picker-rename", &entry.id));
        let mut edit_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("rename-child", &entry.id))
                .max_rect(edit_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        edit_ui.set_min_size(edit_rect.size());
        let output = egui::TextEdit::singleline(&mut draft)
            .id(text_id)
            .font(egui::FontId::new(14.0, egui::FontFamily::Proportional)) // text-sm = 14px.
            .frame(false)
            .desired_width(edit_rect.width())
            .show(&mut edit_ui);
        if select_all {
            output.response.request_focus();
            let mut state = output.state;
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::default(),
                    egui::text::CCursor::new(draft.chars().count()),
                )));
            state.store(ui.ctx(), output.response.id);
        }
        let (enter, escape) = ui.input(|input| {
            input
                .events
                .iter()
                .fold((false, false), |(enter, escape), event| {
                    if let egui::Event::Key {
                        key, pressed: true, ..
                    } = event
                    {
                        (
                            enter || *key == egui::Key::Enter,
                            escape || *key == egui::Key::Escape,
                        )
                    } else {
                        (enter, escape)
                    }
                })
        });
        if escape {
            actions.push(PickerAction::CancelRename);
        } else if enter || output.response.lost_focus() {
            actions.push(PickerAction::CommitRename {
                entry_id: entry.id.clone(),
                draft,
            });
        } else {
            actions.push(PickerAction::UpdateRenameDraft {
                entry_id: entry.id.clone(),
                draft,
            });
        }
    }
}
