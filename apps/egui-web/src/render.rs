use eframe::egui;
use std::sync::Arc;

use crate::gpu::StateInstance;

pub(super) struct StatePanelLayout {
    pub(super) state_count: usize,
    pub(super) qubits: usize,
    pub(super) columns: usize,
    pub(super) size: f32,
    pub(super) gap: f32,
    pub(super) radius: f32,
    pub(super) stroke: f32,
    pub(super) inner_radius: f32,
    pub(super) base_pos: egui::Pos2,
    pub(super) state_rect: egui::Rect,
    pub(super) handle_height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct StateInstanceKey {
    pub(super) state_count: usize,
    pub(super) columns: usize,
    pub(super) size: f32,
    pub(super) gap: f32,
    pub(super) radius: f32,
    pub(super) inner_radius: f32,
    pub(super) stroke: f32,
    pub(super) origin: egui::Pos2,
}

pub(super) struct StateInstanceCache {
    pub(super) key: StateInstanceKey,
    pub(super) instances: Arc<[StateInstance]>,
}
