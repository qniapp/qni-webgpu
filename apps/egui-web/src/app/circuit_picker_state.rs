//! Circuit picker UI state and drag/rename reducer helpers.
//!
//! Kept separate from the persisted `CircuitLibrary` domain model so picker
//! interaction changes do not touch localStorage serialization code.

use eframe::egui;

use super::circuit_library::{CircuitEntry, CircuitId};

#[derive(Clone, Debug, Default)]
pub(crate) enum PickerState {
    #[default]
    Closed,
    Open {
        focused_index: usize,
        focus_visible: bool,
        submenu: Option<PickerSubmenuState>,
        renaming: Option<RenameState>,
        drag: PickerDragState,
    },
}

#[derive(Clone, Debug, Default)]
pub(crate) enum PickerDragState {
    #[default]
    Idle,
    Pending {
        entry_index: usize,
        started_pos: egui::Pos2,
        source_row_left: f32,
        row_width: f32,
        pointer_offset_y: f32,
    },
    Active {
        entry_index: usize,
        source_row_left: f32,
        row_width: f32,
        pointer_offset_y: f32,
        original_entries: Vec<CircuitEntry>,
        did_reorder: bool,
    },
}

impl PickerState {
    pub(crate) fn open(focused_index: usize) -> Self {
        Self::Open {
            focused_index,
            focus_visible: false,
            submenu: None,
            renaming: None,
            drag: PickerDragState::Idle,
        }
    }

    pub(crate) fn open_with_focus(focused_index: usize) -> Self {
        Self::Open {
            focused_index,
            focus_visible: true,
            submenu: None,
            renaming: None,
            drag: PickerDragState::Idle,
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        matches!(self, Self::Open { .. })
    }

    pub(crate) fn close(&mut self) {
        *self = Self::Closed;
    }

    pub(crate) fn toggle_submenu(&mut self, entry_index: usize) {
        if let Self::Open { submenu, .. } = self {
            if submenu
                .as_ref()
                .is_some_and(|submenu| submenu.entry_index == entry_index)
            {
                *submenu = None;
            } else {
                *submenu = Some(PickerSubmenuState { entry_index });
            }
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

    pub(crate) fn focus_visible(&self) -> bool {
        match self {
            Self::Open { focus_visible, .. } => *focus_visible,
            Self::Closed => false,
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

    pub(crate) fn drag_in_progress(&self) -> bool {
        matches!(
            self,
            Self::Open {
                drag: PickerDragState::Pending { .. } | PickerDragState::Active { .. },
                ..
            }
        )
    }

    pub(crate) fn active_drag_index(&self) -> Option<usize> {
        match self {
            Self::Open {
                drag: PickerDragState::Active { entry_index, .. },
                ..
            } => Some(*entry_index),
            _ => None,
        }
    }

    pub(crate) fn pending_drag_index(&self) -> Option<usize> {
        match self {
            Self::Open {
                drag: PickerDragState::Pending { entry_index, .. },
                ..
            } => Some(*entry_index),
            _ => None,
        }
    }

    pub(crate) fn drag_source_index(&self) -> Option<usize> {
        match self {
            Self::Open {
                drag:
                    PickerDragState::Pending { entry_index, .. }
                    | PickerDragState::Active { entry_index, .. },
                ..
            } => Some(*entry_index),
            _ => None,
        }
    }

    pub(crate) fn drag_paint_row(&self) -> Option<(usize, f32, f32, f32, Option<f32>)> {
        match self {
            Self::Open {
                drag:
                    PickerDragState::Pending {
                        entry_index,
                        started_pos,
                        source_row_left,
                        row_width,
                        pointer_offset_y,
                    },
                ..
            } => Some((
                *entry_index,
                *source_row_left,
                *row_width,
                *pointer_offset_y,
                Some(started_pos.y - *pointer_offset_y),
            )),
            Self::Open {
                drag:
                    PickerDragState::Active {
                        entry_index,
                        source_row_left,
                        row_width,
                        pointer_offset_y,
                        ..
                    },
                ..
            } => Some((
                *entry_index,
                *source_row_left,
                *row_width,
                *pointer_offset_y,
                None,
            )),
            _ => None,
        }
    }

    pub(crate) fn dragged_row(&self) -> Option<(usize, f32, f32, f32)> {
        match self {
            Self::Open {
                drag:
                    PickerDragState::Active {
                        entry_index,
                        source_row_left,
                        row_width,
                        pointer_offset_y,
                        ..
                    },
                ..
            } => Some((
                *entry_index,
                *source_row_left,
                *row_width,
                *pointer_offset_y,
            )),
            _ => None,
        }
    }

    pub(crate) fn start_drag_pending(
        &mut self,
        entry_index: usize,
        started_pos: egui::Pos2,
        row_rect: egui::Rect,
    ) -> bool {
        if let Self::Open { drag, submenu, .. } = self {
            if matches!(drag, PickerDragState::Idle) {
                *submenu = None;
                *drag = PickerDragState::Pending {
                    entry_index,
                    started_pos,
                    source_row_left: row_rect.left(),
                    row_width: row_rect.width(),
                    pointer_offset_y: started_pos.y - row_rect.top(),
                };
                return true;
            }
        }
        false
    }

    pub(crate) fn promote_pending_drag(
        &mut self,
        pointer_pos: egui::Pos2,
        threshold_sq: f32,
        original_entries: &[CircuitEntry],
    ) {
        let next = match self {
            Self::Open {
                drag:
                    PickerDragState::Pending {
                        entry_index,
                        started_pos,
                        source_row_left,
                        row_width,
                        pointer_offset_y,
                    },
                ..
            } if pointer_pos.distance_sq(*started_pos) >= threshold_sq => {
                Some(PickerDragState::Active {
                    entry_index: *entry_index,
                    source_row_left: *source_row_left,
                    row_width: *row_width,
                    pointer_offset_y: *pointer_offset_y,
                    original_entries: original_entries.to_vec(),
                    did_reorder: false,
                })
            }
            _ => None,
        };
        if let (Self::Open { drag, .. }, Some(next)) = (self, next) {
            *drag = next;
        }
    }

    pub(crate) fn set_active_drag_index(&mut self, index: usize) {
        if let Self::Open {
            drag: PickerDragState::Active { entry_index, .. },
            ..
        } = self
        {
            *entry_index = index;
        }
    }

    pub(crate) fn mark_drag_reordered(&mut self) {
        if let Self::Open {
            drag: PickerDragState::Active { did_reorder, .. },
            ..
        } = self
        {
            *did_reorder = true;
        }
    }

    pub(crate) fn finish_drag(&mut self) -> bool {
        match self {
            Self::Open { drag, .. } => {
                let did_reorder = matches!(
                    drag,
                    PickerDragState::Active {
                        did_reorder: true,
                        ..
                    }
                );
                *drag = PickerDragState::Idle;
                did_reorder
            }
            Self::Closed => false,
        }
    }

    pub(crate) fn cancel_drag(&mut self) -> Option<Vec<CircuitEntry>> {
        match self {
            Self::Open { drag, .. } => {
                let original_entries = match drag {
                    PickerDragState::Active {
                        original_entries, ..
                    } => Some(original_entries.clone()),
                    PickerDragState::Idle | PickerDragState::Pending { .. } => None,
                };
                *drag = PickerDragState::Idle;
                original_entries
            }
            Self::Closed => None,
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
