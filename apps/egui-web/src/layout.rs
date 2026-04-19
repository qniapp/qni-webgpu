use crate::app::PlacedGate;
use crate::{GATE_SIZE, LINE_GAP, LINE_LEFT_OFFSET, LINE_RIGHT_OFFSET, LINE_Y, SLOT_SPACING};

#[derive(Clone, Debug)]
pub(crate) struct LayoutMetrics {
    pub(crate) line_left: f32,
    pub(crate) line_right: f32,
    pub(crate) line_ys: Vec<f32>,
    pub(crate) slot_left: f32,
    pub(crate) slot_right: f32,
    pub(crate) slot_centers: Vec<f32>,
}

pub(crate) fn layout_metrics(width: f32, qubit_count: usize) -> LayoutMetrics {
    let line_left = LINE_LEFT_OFFSET;
    let line_right = width - LINE_RIGHT_OFFSET;
    let line_ys = (0..qubit_count)
        .map(|index| LINE_Y + LINE_GAP * index as f32)
        .collect::<Vec<f32>>();
    let slot_left = line_left + GATE_SIZE;
    let slot_right = line_right - GATE_SIZE;
    let slot_count = if SLOT_SPACING > 0.0 {
        ((slot_right - slot_left) / SLOT_SPACING).floor() as i32 + 1
    } else {
        0
    };
    let slot_centers = if slot_count > 0 {
        (0..slot_count)
            .map(|index| slot_left + SLOT_SPACING * index as f32)
            .collect()
    } else {
        Vec::new()
    };
    LayoutMetrics {
        line_left,
        line_right,
        line_ys,
        slot_left,
        slot_right,
        slot_centers,
    }
}

fn nearest_slot_center(x: f32, slot_centers: &[f32]) -> (f32, f32) {
    let mut nearest = x;
    let mut nearest_distance = f32::MAX;
    for &slot in slot_centers {
        let distance = (x - slot).abs();
        if distance < nearest_distance {
            nearest = slot;
            nearest_distance = distance;
        }
    }
    (nearest, nearest_distance)
}

pub(crate) fn nearest_slot_index(x: f32, slot_centers: &[f32]) -> Option<(usize, f32)> {
    let mut nearest_index = None;
    let mut nearest_distance = f32::MAX;
    for (index, &slot) in slot_centers.iter().enumerate() {
        let distance = (x - slot).abs();
        if distance < nearest_distance {
            nearest_distance = distance;
            nearest_index = Some(index);
        }
    }
    nearest_index.map(|index| (index, nearest_distance))
}

pub(crate) fn nearest_available_slot(
    x: f32,
    wire_index: usize,
    ignore_id: Option<u32>,
    gates: &[PlacedGate],
    slot_centers: &[f32],
) -> Option<(f32, f32)> {
    let mut occupied = Vec::new();
    for gate in gates {
        if gate.wire != wire_index {
            continue;
        }
        if ignore_id == Some(gate.id) {
            continue;
        }
        let center_x = gate.pos.x + GATE_SIZE / 2.0;
        let (snapped, _) = nearest_slot_center(center_x, slot_centers);
        occupied.push(snapped);
    }

    let mut nearest = x;
    let mut nearest_distance = f32::MAX;
    let mut found = false;
    for &slot in slot_centers {
        if occupied
            .iter()
            .any(|&value| (value - slot).abs() < f32::EPSILON)
        {
            continue;
        }
        let distance = (x - slot).abs();
        if !found || distance < nearest_distance {
            nearest = slot;
            nearest_distance = distance;
            found = true;
        }
    }
    if found {
        Some((nearest, nearest_distance))
    } else {
        None
    }
}

pub(crate) fn nearest_line(y: f32, line_ys: &[f32]) -> (f32, f32, usize) {
    let mut nearest = line_ys[0];
    let mut nearest_distance = (y - line_ys[0]).abs();
    let mut nearest_index = 0;
    for (index, &line_y) in line_ys.iter().enumerate() {
        let distance = (y - line_y).abs();
        if distance < nearest_distance {
            nearest = line_y;
            nearest_distance = distance;
            nearest_index = index;
        }
    }
    (nearest, nearest_distance, nearest_index)
}
