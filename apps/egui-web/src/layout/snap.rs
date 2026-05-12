use std::collections::BTreeSet;

use crate::app::PlacedGate;
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

    pub(crate) fn column(self) -> usize {
        match self {
            CircuitSnap::Slot(snap) => snap.index,
            CircuitSnap::Insert(snap) => snap.index,
        }
    }
}

fn nearest_available_slot(
    x: f32,
    wire_index: usize,
    ignore_id: Option<u32>,
    gates: &[PlacedGate],
    slot_centers: &[f32],
) -> Option<SlotSnap> {
    let mut occupied_columns = Vec::new();
    for gate in gates {
        if gate.wire != wire_index {
            continue;
        }
        if ignore_id == Some(gate.id) {
            continue;
        }
        occupied_columns.push(gate.column);
    }

    let mut nearest = None;
    let mut nearest_distance = f32::MAX;
    for (index, &center) in slot_centers.iter().enumerate() {
        if occupied_columns.contains(&index) {
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
        .map(|gate| gate.column)
        .collect();
    if occupied_columns.is_empty() {
        return None;
    }

    let mut insert_indices: BTreeSet<usize> = BTreeSet::new();
    insert_indices.insert(0);
    for column in occupied_columns {
        insert_indices.insert(column + 1);
    }

    let mut nearest = None;
    let mut nearest_distance = f32::MAX;
    for index in insert_indices {
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
    ignore_id: Option<u32>,
    gates: &[PlacedGate],
    slot_centers: &[f32],
) -> Option<CircuitSnap> {
    let slot = nearest_available_slot(x, wire_index, ignore_id, gates, slot_centers)
        .map(CircuitSnap::Slot);
    let insert = nearest_insert_slot(x, ignore_id, gates, slot_centers).map(CircuitSnap::Insert);
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
