use std::collections::BTreeSet;

use super::geometry::{gate_rect_at_grid, gate_width_cols};
use crate::app::{CircuitColumnIndex, PlacedGate, WireIndex};
use crate::constants::SLOT_SPACING;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SlotSnap {
    pub(crate) index: usize,
    pub(crate) center: f32,
    pub(crate) distance: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct InsertSnap {
    /// Column index that will be created when the gate is dropped. `0` means
    /// before the first occupied step; `N + 1` means after step `N`.
    pub(crate) index: usize,
    /// Temporary qni-style snap target between step centers.
    pub(crate) center: f32,
    pub(crate) distance: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum CircuitSnap {
    Slot(SlotSnap),
    Insert(InsertSnap),
}

impl CircuitSnap {
    pub(crate) fn center(self) -> f32 {
        match self {
            CircuitSnap::Slot(snap) => snap.center,
            CircuitSnap::Insert(snap) => snap.center,
        }
    }

    pub(crate) fn distance(self) -> f32 {
        match self {
            CircuitSnap::Slot(snap) => snap.distance,
            CircuitSnap::Insert(snap) => snap.distance,
        }
    }

    pub(crate) fn column(self) -> CircuitColumnIndex {
        match self {
            CircuitSnap::Slot(snap) => CircuitColumnIndex::new(snap.index),
            CircuitSnap::Insert(snap) => CircuitColumnIndex::new(snap.index),
        }
    }
}

fn candidate_intersects_gate(
    moving_gate: &PlacedGate,
    candidate_column: usize,
    candidate_wire: usize,
    existing_gate: &PlacedGate,
    existing_column: usize,
) -> bool {
    let candidate_rect = gate_rect_at_grid(
        moving_gate.kind,
        CircuitColumnIndex::new(candidate_column),
        WireIndex::new(candidate_wire),
        moving_gate.span,
    );
    let existing_rect = gate_rect_at_grid(
        existing_gate.kind,
        CircuitColumnIndex::new(existing_column),
        existing_gate.wire,
        existing_gate.span,
    );
    candidate_rect.intersects(existing_rect)
}

fn candidate_available_at_slot(
    moving_gate: &PlacedGate,
    column: usize,
    wire_index: usize,
    ignore_id: Option<u32>,
    gates: &[PlacedGate],
) -> bool {
    gates.iter().all(|gate| {
        ignore_id == Some(gate.id)
            || !candidate_intersects_gate(
                moving_gate,
                column,
                wire_index,
                gate,
                gate.column.as_usize(),
            )
    })
}

fn candidate_available_after_insert(
    moving_gate: &PlacedGate,
    insert_index: usize,
    wire_index: usize,
    ignore_id: Option<u32>,
    gates: &[PlacedGate],
) -> bool {
    let moving_width = gate_width_cols(moving_gate.kind, moving_gate.span);
    gates.iter().all(|gate| {
        if ignore_id == Some(gate.id) {
            return true;
        }
        let shifted_column = if gate.column.as_usize() >= insert_index {
            let Some(shifted_column) = gate.column.checked_add(moving_width) else {
                return false;
            };
            shifted_column.as_usize()
        } else {
            gate.column.as_usize()
        };
        !candidate_intersects_gate(moving_gate, insert_index, wire_index, gate, shifted_column)
    })
}

fn nearest_available_slot(
    x: f32,
    wire_index: usize,
    moving_gate: &PlacedGate,
    ignore_id: Option<u32>,
    gates: &[PlacedGate],
    slot_centers: &[f32],
) -> Option<SlotSnap> {
    let mut nearest = None;
    let mut nearest_distance = f32::MAX;
    for (index, &center) in slot_centers.iter().enumerate() {
        if !candidate_available_at_slot(moving_gate, index, wire_index, ignore_id, gates) {
            continue;
        }
        let distance = (x - center).abs();
        if nearest.is_none() || distance < nearest_distance {
            nearest = Some(SlotSnap {
                index,
                center,
                distance,
            });
            nearest_distance = distance;
        }
    }
    nearest
}

fn nearest_insert_slot(
    x: f32,
    wire_index: usize,
    moving_gate: &PlacedGate,
    ignore_id: Option<u32>,
    gates: &[PlacedGate],
    slot_centers: &[f32],
) -> Option<InsertSnap> {
    if slot_centers.is_empty() {
        return None;
    }

    let occupied_columns: BTreeSet<usize> = gates
        .iter()
        .filter(|gate| ignore_id != Some(gate.id))
        .map(|gate| gate.column.as_usize())
        .collect();
    if occupied_columns.is_empty() {
        return None;
    }

    let mut insert_indices: BTreeSet<usize> = BTreeSet::new();
    insert_indices.insert(0);
    for column in occupied_columns {
        if let Some(index) = column.checked_add(1) {
            insert_indices.insert(index);
        }
    }

    let mut nearest = None;
    let mut nearest_distance = f32::MAX;
    for index in insert_indices {
        if !candidate_available_after_insert(moving_gate, index, wire_index, ignore_id, gates) {
            continue;
        }
        let center = if index == 0 {
            slot_centers[0] - SLOT_SPACING * 0.5
        } else if index - 1 < slot_centers.len() {
            slot_centers[index - 1] + SLOT_SPACING * 0.5
        } else {
            continue;
        };
        let distance = (x - center).abs();
        if nearest.is_none() || distance < nearest_distance {
            nearest = Some(InsertSnap {
                index,
                center,
                distance,
            });
            nearest_distance = distance;
        }
    }
    nearest
}

pub(crate) fn nearest_circuit_snap(
    x: f32,
    wire_index: usize,
    moving_gate: &PlacedGate,
    ignore_id: Option<u32>,
    gates: &[PlacedGate],
    slot_centers: &[f32],
) -> Option<CircuitSnap> {
    let slot = nearest_available_slot(x, wire_index, moving_gate, ignore_id, gates, slot_centers)
        .map(CircuitSnap::Slot);
    let insert = nearest_insert_slot(x, wire_index, moving_gate, ignore_id, gates, slot_centers)
        .map(CircuitSnap::Insert);
    match (slot, insert) {
        (Some(slot), Some(insert)) => {
            if slot.distance() <= insert.distance() {
                Some(slot)
            } else {
                Some(insert)
            }
        }
        (Some(slot), None) => Some(slot),
        (None, Some(insert)) => Some(insert),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gates::GateKind;

    #[test]
    fn amplitude_slot_snap_skips_intersecting_neighbor_column() {
        let existing = PlacedGate::new(
            1,
            GateKind::H,
            crate::app::CircuitColumnIndex::new(1),
            crate::app::WireIndex::new(0),
            1,
            None,
        );
        let moving = PlacedGate::new(
            2,
            GateKind::AmplitudeDisplay,
            crate::app::CircuitColumnIndex::new(0),
            crate::app::WireIndex::new(0),
            3,
            None,
        );
        let snap = nearest_circuit_snap(
            126.0,
            0,
            &moving,
            Some(2),
            &[existing, moving.clone()],
            &[126.0, 182.0, 238.0],
        );

        assert!(!matches!(snap, Some(CircuitSnap::Slot(slot)) if slot.index == 0));
    }
}
