//! Circuit picker UI state and drag/rename reducer helpers.
//!
//! Kept separate from the persisted `CircuitLibrary` domain model so picker
//! interaction changes do not touch localStorage serialization code.

use eframe::egui;

use super::circuit_library::{CircuitEntry, CircuitId};

const PICKER_ROW_HEIGHT: f32 = 36.0; // h-9 = 36px picker row height.
const DEFAULT_PICKER_ITEMS_HEIGHT: f32 = 216.0; // 6 picker rows × 36px, mock §03 default.
const DEFAULT_PICKER_MIN_ITEMS_HEIGHT: f32 = 72.0; // 2 picker rows × 36px, mock §03 min.

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
        items_height: f32,
        resize_drag: Option<ResizeDrag>,
        pending_scroll_offset: Option<f32>,
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
        original_index: usize,
        current_slot: usize,
        source_row_left: f32,
        row_width: f32,
        pointer_offset_y: f32,
    },
}

impl PickerState {
    pub(crate) fn open(focused_index: usize) -> Self {
        Self::open_with_focus_state(focused_index, false)
    }

    pub(crate) fn open_with_focus(focused_index: usize) -> Self {
        Self::open_with_focus_state(focused_index, true)
    }

    fn open_with_focus_state(focused_index: usize, focus_visible: bool) -> Self {
        Self::Open {
            focused_index,
            focus_visible,
            submenu: None,
            renaming: None,
            drag: PickerDragState::Idle,
            items_height: DEFAULT_PICKER_ITEMS_HEIGHT,
            resize_drag: None,
            pending_scroll_offset: None,
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

    pub(crate) fn update_rename_draft(&mut self, entry_id: &CircuitId, draft: String) {
        if let Self::Open {
            renaming: Some(rename),
            ..
        } = self
        {
            if rename.entry_id == *entry_id {
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
            } => Some(rename.entry_id.as_str()),
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

    pub(crate) fn items_height(&self) -> f32 {
        match self {
            Self::Open { items_height, .. } => *items_height,
            Self::Closed => DEFAULT_PICKER_ITEMS_HEIGHT,
        }
    }

    pub(crate) fn clamp_items_height(&mut self, max_items_height: f32) {
        if let Self::Open { items_height, .. } = self {
            *items_height = (*items_height)
                .max(DEFAULT_PICKER_MIN_ITEMS_HEIGHT)
                .min(max_items_height.max(DEFAULT_PICKER_MIN_ITEMS_HEIGHT));
        }
    }

    pub(crate) fn start_resize_drag(&mut self, pointer_y: f32) {
        if let Self::Open {
            items_height,
            resize_drag,
            ..
        } = self
        {
            *resize_drag = Some(ResizeDrag {
                start_pointer_y: pointer_y,
                base_height: *items_height,
            });
        }
    }

    pub(crate) fn update_resize_drag(&mut self, pointer_y: f32, max_items_height: f32) {
        if let Self::Open {
            items_height,
            resize_drag: Some(resize_drag),
            ..
        } = self
        {
            let unclamped = resize_drag.base_height + pointer_y - resize_drag.start_pointer_y;
            *items_height = unclamped
                .max(DEFAULT_PICKER_MIN_ITEMS_HEIGHT)
                .min(max_items_height.max(DEFAULT_PICKER_MIN_ITEMS_HEIGHT));
        }
    }

    pub(crate) fn finish_resize_drag(&mut self) {
        if let Self::Open { resize_drag, .. } = self {
            *resize_drag = None;
        }
    }

    pub(crate) fn resize_drag_active(&self) -> bool {
        matches!(
            self,
            Self::Open {
                resize_drag: Some(_),
                ..
            }
        )
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

    pub(crate) fn active_drag_slot(&self) -> Option<usize> {
        match self {
            Self::Open {
                drag: PickerDragState::Active { current_slot, .. },
                ..
            } => Some(*current_slot),
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
                drag: PickerDragState::Pending { entry_index, .. },
                ..
            } => Some(*entry_index),
            Self::Open {
                drag: PickerDragState::Active { original_index, .. },
                ..
            } => Some(*original_index),
            _ => None,
        }
    }

    pub(crate) fn visual_row_shift(&self, index: usize) -> f32 {
        let Self::Open {
            drag:
                PickerDragState::Active {
                    original_index,
                    current_slot,
                    ..
                },
            ..
        } = self
        else {
            return 0.0;
        };
        if *current_slot > *original_index && index > *original_index && index <= *current_slot {
            -PICKER_ROW_HEIGHT
        } else if *current_slot < *original_index
            && index >= *current_slot
            && index < *original_index
        {
            PICKER_ROW_HEIGHT
        } else {
            0.0
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
                        original_index,
                        source_row_left,
                        row_width,
                        pointer_offset_y,
                        ..
                    },
                ..
            } => Some((
                *original_index,
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
                        original_index,
                        source_row_left,
                        row_width,
                        pointer_offset_y,
                        ..
                    },
                ..
            } => Some((
                *original_index,
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

    pub(crate) fn promote_pending_drag(&mut self, pointer_pos: egui::Pos2, threshold_sq: f32) {
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
                    original_index: *entry_index,
                    current_slot: *entry_index,
                    source_row_left: *source_row_left,
                    row_width: *row_width,
                    pointer_offset_y: *pointer_offset_y,
                })
            }
            _ => None,
        };
        if let (Self::Open { drag, .. }, Some(next)) = (self, next) {
            *drag = next;
        }
    }

    pub(crate) fn set_active_drag_slot(&mut self, slot: usize) -> bool {
        if let Self::Open {
            drag: PickerDragState::Active { current_slot, .. },
            ..
        } = self
        {
            let changed = *current_slot != slot;
            *current_slot = slot;
            return changed;
        }
        false
    }

    pub(crate) fn take_pending_scroll_offset(&mut self) -> Option<f32> {
        match self {
            Self::Open {
                pending_scroll_offset,
                ..
            } => pending_scroll_offset.take(),
            Self::Closed => None,
        }
    }

    pub(crate) fn set_pending_scroll_offset(&mut self, offset: f32) {
        if let Self::Open {
            pending_scroll_offset,
            ..
        } = self
        {
            *pending_scroll_offset = Some(offset);
        }
    }

    pub(crate) fn clear_pending_scroll_offset(&mut self) {
        if let Self::Open {
            pending_scroll_offset,
            ..
        } = self
        {
            *pending_scroll_offset = None;
        }
    }

    pub(crate) fn finish_drag(&mut self) -> Option<(usize, usize)> {
        match self {
            Self::Open {
                drag,
                pending_scroll_offset,
                ..
            } => {
                let move_to = match drag {
                    PickerDragState::Active {
                        original_index,
                        current_slot,
                        ..
                    } if original_index != current_slot => Some((*original_index, *current_slot)),
                    PickerDragState::Idle
                    | PickerDragState::Pending { .. }
                    | PickerDragState::Active { .. } => None,
                };
                *drag = PickerDragState::Idle;
                *pending_scroll_offset = None;
                move_to
            }
            Self::Closed => None,
        }
    }

    pub(crate) fn cancel_drag(&mut self) {
        if let Self::Open {
            drag,
            pending_scroll_offset,
            ..
        } = self
        {
            *drag = PickerDragState::Idle;
            *pending_scroll_offset = None;
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ResizeDrag {
    pub(crate) start_pointer_y: f32,
    pub(crate) base_height: f32,
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
