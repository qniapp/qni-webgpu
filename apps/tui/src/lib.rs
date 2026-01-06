use ratatui::style::Color;

pub mod input;
pub mod layout;
pub mod model;
pub mod render;

pub use input::{
    confirm_hovered_column, handle_mouse_down, handle_mouse_move, handle_mouse_up,
    handle_phase_edit_key, update_hovered_slot,
};
pub use layout::{
    circuit_layout, column_line_x, hit_test_circuit_slot, hit_test_palette, hovered_column_at,
    hovered_start_at, insertion_snap_rect, is_empty_drop, layout_regions, start_line_x,
    CircuitLayout, PaletteItem, Regions,
};
pub use model::{
    apply_gate_to_state, apply_gates_to_zero, build_state_line, format_complex, parse_args,
    AppState, Complex, DragOrigin, DragState, Gate,
};
pub use render::{render_to_buffer, render_to_buffer_with_drag, DragVisual};

pub(crate) const PALETTE_LABEL: &str = "";
pub(crate) const GATE_BOX_WIDTH: u16 = 5;
pub(crate) const GATE_BOX_HEIGHT: u16 = 3;
pub(crate) const SHADOW_OUTSET: u16 = 0;
pub(crate) const GATE_DRAW_HEIGHT: u16 = GATE_BOX_HEIGHT + SHADOW_OUTSET * 2;
pub(crate) const PALETTE_HEIGHT: u16 = GATE_DRAW_HEIGHT;
pub(crate) const PALETTE_GAP: u16 = 1;
pub(crate) const SEPARATOR_TO_CIRCUIT_GAP: u16 = 1;
pub(crate) const GATE_GAP: u16 = 1;
pub(crate) const SLOT_GAP: u16 = 3;
pub(crate) const SNAP_DISTANCE: u16 = 1;
pub(crate) const MIN_QUBIT_COUNT: usize = 2;
pub(crate) const ROW_GAP: u16 = 1;
pub(crate) const UI_BACKGROUND: Color = Color::Black;

#[cfg(test)]
mod tests {
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    use super::*;
    use crate::layout::palette_items;
    use crate::model::{ensure_slots, parse_phase_label, qubit_count};
    use pretty_assertions::assert_eq;

    fn state_with_gate(gate: Gate) -> AppState {
        let mut state = AppState::new();
        state.placed[0].push(Some(gate));
        state.phase_values[0].push(
            if gate == Gate::Phase {
                Some(crate::model::default_phase_value())
            } else {
                None
            },
        );
        state
    }

    fn parse_args_from(args: &[&str]) -> Gate {
        let owned: Vec<String> = args.iter().map(|value| value.to_string()).collect();
        parse_args(&owned)
    }

    #[test]
    fn build_state_line_defaults_to_zero_state() {
        let line = build_state_line(&[]);
        assert_eq!(line, "State: [(1+0i), (0+0i), (0+0i), (0+0i)]");
    }

    #[test]
    fn build_state_line_expands_for_three_qubits() {
        let line = build_state_line(&[vec![None], vec![None], vec![None]]);
        assert_eq!(
            line,
            "State: [(1+0i), (0+0i), (0+0i), (0+0i), (0+0i), (0+0i), (0+0i), (0+0i)]"
        );
    }

    #[test]
    fn build_state_line_updates_third_qubit() {
        let line = build_state_line(&[vec![None], vec![None], vec![Some(Gate::X)]]);
        assert_eq!(
            line,
            "State: [(0+0i), (1+0i), (0+0i), (0+0i), (0+0i), (0+0i), (0+0i), (0+0i)]"
        );
    }

    #[test]
    fn build_state_line_for_h_gate() {
        let line = build_state_line(&[vec![Some(Gate::H)]]);
        assert_eq!(
            line,
            "State: [(0.7071067811865475+0i), (0+0i), (0.7071067811865475+0i), (0+0i)]"
        );
    }

    #[test]
    fn build_state_line_for_y_gate() {
        let line = build_state_line(&[vec![Some(Gate::Y)]]);
        assert_eq!(line, "State: [(0+0i), (0+0i), (0+1i), (0+0i)]");
    }

    #[test]
    fn build_state_line_for_phase_gate() {
        let line = build_state_line(&[vec![Some(Gate::Phase)]]);
        assert_eq!(line, "State: [(1+0i), (0+0i), (0+0i), (0+0i)]");
    }

    #[test]
    fn parse_args_handles_rotation_gates() {
        assert_eq!(parse_args_from(&["--gate", "rx"]), Gate::Rx);
        assert_eq!(parse_args_from(&["--gate", "ry"]), Gate::Ry);
        assert_eq!(parse_args_from(&["--gate", "rz"]), Gate::Rz);
    }

    #[test]
    fn parse_phase_label_accepts_pi_fractions() {
        let value = parse_phase_label("3π/4").expect("phase should parse");
        assert_eq!(value.label, "3π/4");
    }

    #[test]
    fn build_state_line_for_cnot_gate() {
        let line = build_state_line(&[
            vec![Some(Gate::X), Some(Gate::Control)],
            vec![None, Some(Gate::X)],
        ]);
        assert_eq!(line, "State: [(0+0i), (0+0i), (0+0i), (1+0i)]");
    }

    #[test]
    fn build_state_line_measures_qubit() {
        let line = build_state_line(&[vec![Some(Gate::H), Some(Gate::Measure)]]);
        let expected_zero = [
            "State: [(1+0i), (0+0i), (0+0i), (0+0i)]",
            "State: [(0.9999999999999998+0i), (0+0i), (0+0i), (0+0i)]",
        ];
        let expected_one = [
            "State: [(0+0i), (0+0i), (1+0i), (0+0i)]",
            "State: [(0+0i), (0+0i), (0.9999999999999998+0i), (0+0i)]",
        ];
        let ok_zero = expected_zero.iter().any(|value| *value == line);
        let ok_one = expected_one.iter().any(|value| *value == line);
        assert!(ok_zero || ok_one);
    }

    #[test]
    fn swap_gate_swaps_qubits_in_column() {
        let line = build_state_line(&[
            vec![Some(Gate::X), Some(Gate::Swap)],
            vec![None, Some(Gate::Swap)],
        ]);
        assert_eq!(line, "State: [(0+0i), (1+0i), (0+0i), (0+0i)]");
    }

    #[test]
    fn parse_args_handles_gate_flags() {
        assert_eq!(parse_args_from(&[]), Gate::H);
        assert_eq!(parse_args_from(&["--gate", "y"]), Gate::Y);
        assert_eq!(parse_args_from(&["--gate=Z"]), Gate::Z);
        assert_eq!(parse_args_from(&["--gate", "control"]), Gate::Control);
        assert_eq!(parse_args_from(&["--gate", "measure"]), Gate::Measure);
        assert_eq!(parse_args_from(&["--gate", "phase"]), Gate::Phase);
        assert_eq!(parse_args_from(&["--gate", "sqrtx"]), Gate::SqrtX);
        assert_eq!(parse_args_from(&["--gate", "sdg"]), Gate::Sdg);
        assert_eq!(parse_args_from(&["--gate", "tdg"]), Gate::Tdg);
        assert_eq!(parse_args_from(&["--gate", "swap"]), Gate::Swap);
        assert_eq!(parse_args_from(&["--gate", "nope"]), Gate::H);
    }

    #[test]
    fn render_to_buffer_writes_spaced_lines() {
        let area = Rect::new(0, 0, 100, 24);
        let mut state = state_with_gate(Gate::H);
        let buffer = render_to_buffer(&mut state, area, None);
        let layout = circuit_layout(area, qubit_count(&state));
        let regions = layout_regions(area, qubit_count(&state));
        let gate_rect = layout.slots[0][0];
        let top = buffer_to_string(&buffer, gate_rect.x, gate_rect.y, GATE_BOX_WIDTH);
        let mid = buffer_to_string(&buffer, gate_rect.x, gate_rect.y + 1, GATE_BOX_WIDTH);
        let state_line = buffer_to_line(&buffer, area, regions.state.y);
        let wire_line = buffer_to_line(&buffer, area, layout.wire_rows[0]);
        assert_eq!(top, "▔▔▔▔▔");
        assert_eq!(mid, "  H  ");
        assert!(wire_line.starts_with("q0: "));
        assert!(state_line.starts_with("|00>"));
        assert!(state_line.contains("1.000"));
    }

    #[test]
    fn state_line_respects_hovered_column() {
        let area = Rect::new(0, 0, 100, 24);
        let mut state = AppState::new();
        let regions = layout_regions(area, qubit_count(&state));
        state.placed[0] = vec![Some(Gate::H), Some(Gate::Control)];
        state.placed[1] = vec![None, Some(Gate::X)];
        state.hovered_column = Some((0, 0));
        let buffer = render_to_buffer(&mut state, area, None);
        let line_10 = buffer_to_line(&buffer, area, regions.state.y + 2);
        let line_11 = buffer_to_line(&buffer, area, regions.state.y + 3);
        assert!(line_10.contains("|10>"));
        assert!(line_10.contains("0.500"));
        assert!(line_11.contains("|11>"));
        assert!(line_11.contains("0.000"));
        state.hovered_column = Some((0, 1));
        let buffer = render_to_buffer(&mut state, area, None);
        let line_10 = buffer_to_line(&buffer, area, regions.state.y + 2);
        let line_11 = buffer_to_line(&buffer, area, regions.state.y + 3);
        assert!(line_10.contains("|10>"));
        assert!(line_10.contains("0.000"));
        assert!(line_11.contains("|11>"));
        assert!(line_11.contains("0.500"));
    }

    #[test]
    fn hover_start_shows_start_line() {
        let area = Rect::new(0, 0, 80, 24);
        let mut state = AppState::new();
        state.hovered_start = true;
        let buffer = render_to_buffer(&mut state, area, None);
        let layout = circuit_layout(area, qubit_count(&state));
        let line_x = start_line_x(&layout).unwrap();
        let symbol = buffer.get(line_x, layout.wire_rows[0]).symbol();
        assert_eq!(symbol, "│");
    }

    #[test]
    fn confirm_start_persists() {
        let area = Rect::new(0, 0, 80, 24);
        let mut state = AppState::new();
        state.hovered_start = true;
        confirm_hovered_column(&mut state);
        state.hovered_start = false;
        let buffer = render_to_buffer(&mut state, area, None);
        let layout = circuit_layout(area, qubit_count(&state));
        let line_x = start_line_x(&layout).unwrap();
        let symbol = buffer.get(line_x, layout.wire_rows[0]).symbol();
        assert_eq!(symbol, "│");
    }

    #[test]
    fn render_to_buffer_shows_drag_overlay() {
        let area = Rect::new(0, 0, 20, 24);
        let drag = DragVisual {
            gate: Gate::H,
            x: 5,
            y: 3,
        };
        let mut state = AppState::new();
        let buffer = render_to_buffer_with_drag(&mut state, area, None, Some(drag));
        let overlay = buffer_to_string(&buffer, 5, 3, 5);
        assert_eq!(overlay, "▔▔▔▔▔");
    }

    #[test]
    fn swap_gate_renders_large_x() {
        let area = Rect::new(0, 0, 20, 20);
        let state = AppState::new();
        let layout = circuit_layout(area, qubit_count(&state));
        let slot = layout.slots[0][0];
        let mut buffer = Buffer::empty(area);
        render::draw_gate_box(&mut buffer, slot, Gate::Swap, None, None, false);
        let top = buffer_to_string(&buffer, slot.x, slot.y, GATE_BOX_WIDTH);
        let mid = buffer_to_string(&buffer, slot.x, slot.y + 1, GATE_BOX_WIDTH);
        let bottom = buffer_to_string(&buffer, slot.x, slot.y + 2, GATE_BOX_WIDTH);
        assert_eq!(top, "\\   /");
        assert_eq!(mid, "▶   ◀");
        assert_eq!(bottom, "/   \\");
    }

    #[test]
    fn swap_pair_draws_vertical_connection() {
        let area = Rect::new(0, 0, 40, 30);
        let mut state = AppState::new();
        state.placed.resize(3, Vec::new());
        state.phase_values.resize(3, Vec::new());
        state.placed[0] = vec![Some(Gate::Swap)];
        state.placed[2] = vec![Some(Gate::Swap)];
        let buffer = render_to_buffer(&mut state, area, None);
        let layout = circuit_layout(area, qubit_count(&state));
        let rect0 = layout.slots[0][0];
        let rect1 = layout.slots[2][0];
        let line_x = rect0.x + rect0.width / 2;
        let line_y = rect0
            .y
            .saturating_add(rect0.height)
            .saturating_add(ROW_GAP / 2)
            .min(rect1.y.saturating_sub(1));
        let symbol = buffer.get(line_x, line_y).symbol();
        assert_eq!(symbol, "│");
    }

    #[test]
    fn hover_slot_draws_outline() {
        let area = Rect::new(0, 0, 60, 20);
        let mut state = AppState::new();
        state.hovered_slot = Some(1);
        state.hovered_row = Some(0);
        state.dragging = Some(DragState {
            gate: Gate::X,
            origin: DragOrigin::Palette,
        });
        let buffer = render_to_buffer_with_drag(&mut state, area, None, None);
        let layout = circuit_layout(area, qubit_count(&state));
        let slot = layout.slots[0][1];
        let outline = buffer_to_string(&buffer, slot.x, slot.y, GATE_BOX_WIDTH);
        assert_eq!(outline, "     ");
    }

    #[test]
    fn drag_visual_snaps_to_hovered_slot() {
        let area = Rect::new(0, 0, 60, 20);
        let mut state = AppState::new();
        state.hovered_slot = Some(1);
        state.hovered_row = Some(0);
        state.dragging = Some(DragState {
            gate: Gate::H,
            origin: DragOrigin::Palette,
        });
        let drag = DragVisual {
            gate: Gate::H,
            x: 1,
            y: 1,
        };
        let buffer = render_to_buffer_with_drag(&mut state, area, None, Some(drag));
        let layout = circuit_layout(area, qubit_count(&state));
        let slot = layout.slots[0][1];
        let top = buffer_to_string(&buffer, slot.x, slot.y, GATE_BOX_WIDTH);
        assert_eq!(top, "▔▔▔▔▔");
    }

    #[test]
    fn drag_visual_does_not_snap_to_occupied_slot() {
        let area = Rect::new(0, 0, 60, 20);
        let mut state = AppState::new();
        state.hovered_slot = Some(1);
        state.hovered_row = Some(0);
        state.dragging = Some(DragState {
            gate: Gate::H,
            origin: DragOrigin::Palette,
        });
        ensure_slots(&mut state, &[3, 3]);
        state.placed[0][1] = Some(Gate::Z);
        let drag = DragVisual {
            gate: Gate::H,
            x: 1,
            y: 1,
        };
        let buffer = render_to_buffer_with_drag(&mut state, area, None, Some(drag));
        let overlay_top = buffer_to_string(&buffer, 1, 1, GATE_BOX_WIDTH);
        let overlay_mid = buffer_to_string(&buffer, 1, 2, GATE_BOX_WIDTH);
        assert_eq!(overlay_top, "▔▔▔▔▔");
        assert_eq!(overlay_mid, "  H  ");
        let layout = circuit_layout(area, qubit_count(&state));
        let slot = layout.slots[0][1];
        let slot_mid = buffer_to_string(&buffer, slot.x, slot.y + 1, GATE_BOX_WIDTH);
        assert_eq!(slot_mid, "  Z  ");
    }

    #[test]
    fn hit_test_palette_finds_gate() {
        let area = Rect::new(0, 0, 60, 20);
        let state = AppState::new();
        let regions = layout_regions(area, qubit_count(&state));
        let items = palette_items(regions.palette);
        let target = items.iter().find(|item| item.gate == Gate::X).unwrap();
        let x = target.rect.x;
        let gate = hit_test_palette(x, target.rect.y, area);
        assert_eq!(gate, Some(Gate::X));
    }

    #[test]
    fn grabbing_gate_adds_empty_qubit_row() {
        let area = Rect::new(0, 0, 60, 20);
        let mut state = AppState::new();
        let initial_rows = state.placed.len();
        let regions = layout_regions(area, qubit_count(&state));
        let items = palette_items(regions.palette);
        let target = items.iter().find(|item| item.gate == Gate::H).unwrap();
        handle_mouse_down(&mut state, target.rect.x, target.rect.y, area);
        assert_eq!(state.placed.len(), initial_rows + 1);
    }

    #[test]
    fn drop_clears_empty_trailing_qubit() {
        let area = Rect::new(0, 0, 60, 20);
        let mut state = AppState::new();
        let regions = layout_regions(area, qubit_count(&state));
        let items = palette_items(regions.palette);
        let target = items.iter().find(|item| item.gate == Gate::Z).unwrap();
        handle_mouse_down(&mut state, target.rect.x, target.rect.y, area);
        let layout = circuit_layout(area, qubit_count(&state));
        let slot = layout.slots[0][0];
        handle_mouse_up(&mut state, slot.x, slot.y, area);
        assert_eq!(state.placed.len(), MIN_QUBIT_COUNT);
    }

    #[test]
    fn drop_on_bottom_qubit_keeps_row() {
        let area = Rect::new(0, 0, 60, 24);
        let mut state = AppState::new();
        let regions = layout_regions(area, qubit_count(&state));
        let items = palette_items(regions.palette);
        let target = items.iter().find(|item| item.gate == Gate::Z).unwrap();
        handle_mouse_down(&mut state, target.rect.x, target.rect.y, area);
        let layout = circuit_layout(area, qubit_count(&state));
        let last_row = layout.slots.len() - 1;
        let slot = layout.slots[last_row][0];
        handle_mouse_up(&mut state, slot.x, slot.y, area);
        assert_eq!(state.placed.len(), MIN_QUBIT_COUNT + 1);
    }

    #[test]
    fn drop_compacts_empty_columns() {
        let area = Rect::new(0, 0, 80, 24);
        let mut state = state_with_gate(Gate::H);
        let layout = circuit_layout(area, qubit_count(&state));
        let slot0 = layout.slots[0][0];
        let slot1 = layout.slots[0][1];
        handle_mouse_down(&mut state, slot0.x, slot0.y, area);
        handle_mouse_up(&mut state, slot1.x, slot1.y, area);
        assert_eq!(state.placed[0].get(0).and_then(|gate| *gate), Some(Gate::H));
    }

    #[test]
    fn drop_between_adjacent_gates_inserts_column() {
        let area = Rect::new(0, 0, 100, 24);
        let mut state = AppState::new();
        let layout = circuit_layout(area, qubit_count(&state));
        let slot0 = layout.slots[0][0];
        state.placed[0] = vec![Some(Gate::H), Some(Gate::X)];
        state.dragging = Some(DragState {
            gate: Gate::Z,
            origin: DragOrigin::Palette,
        });
        state.drag_pos = Some((slot0.x, slot0.y));
        let gap_x = slot0.x.saturating_add(GATE_BOX_WIDTH);
        let gap_y = slot0.y + 1;
        handle_mouse_up(&mut state, gap_x, gap_y, area);
        assert_eq!(state.placed[0].get(0).and_then(|gate| *gate), Some(Gate::H));
        assert_eq!(state.placed[0].get(1).and_then(|gate| *gate), Some(Gate::Z));
        assert_eq!(state.placed[0].get(2).and_then(|gate| *gate), Some(Gate::X));
    }

    #[test]
    fn drop_uses_hovered_insert_even_if_cursor_is_off() {
        let area = Rect::new(0, 0, 100, 24);
        let mut state = AppState::new();
        state.placed[0] = vec![Some(Gate::H), Some(Gate::X)];
        state.dragging = Some(DragState {
            gate: Gate::Z,
            origin: DragOrigin::Palette,
        });
        state.hovered_insert = Some((0, 1));
        handle_mouse_up(&mut state, area.x, area.y, area);
        assert_eq!(state.placed[0].get(1).and_then(|gate| *gate), Some(Gate::Z));
        assert_eq!(state.placed[0].get(2).and_then(|gate| *gate), Some(Gate::X));
    }

    #[test]
    fn insert_snap_preferred_over_existing_gate() {
        let area = Rect::new(0, 0, 100, 24);
        let mut state = AppState::new();
        let layout = circuit_layout(area, qubit_count(&state));
        let slot0 = layout.slots[0][0];
        state.placed[0] = vec![Some(Gate::H), Some(Gate::X)];
        state.dragging = Some(DragState {
            gate: Gate::Z,
            origin: DragOrigin::Palette,
        });
        let gap_x = slot0.x.saturating_add(GATE_BOX_WIDTH);
        let gap_y = slot0.y + 1;
        update_hovered_slot(&mut state, gap_x, gap_y, area);
        assert_eq!(state.hovered_insert, Some((0, 1)));
        assert_eq!(state.hovered_slot, None);
    }

    #[test]
    fn drag_from_palette_to_circuit_sets_gate() {
        let area = Rect::new(0, 0, 60, 20);
        let mut state = AppState::new();
        let regions = layout_regions(area, qubit_count(&state));
        let items = palette_items(regions.palette);
        let target = items.iter().find(|item| item.gate == Gate::Z).unwrap();
        handle_mouse_down(&mut state, target.rect.x, target.rect.y, area);
        let layout = circuit_layout(area, qubit_count(&state));
        let slot = layout.slots[0][0];
        handle_mouse_up(&mut state, slot.x, slot.y, area);
        assert_eq!(state.placed[0][0], Some(Gate::Z));
    }

    #[test]
    fn drag_from_circuit_to_empty_removes_gate() {
        let area = Rect::new(0, 0, 60, 20);
        let mut state = state_with_gate(Gate::H);
        let layout = circuit_layout(area, qubit_count(&state));
        let slot = layout.slots[0][0];
        handle_mouse_down(&mut state, slot.x, slot.y, area);
        handle_mouse_up(&mut state, area.x, area.y + 8, area);
        assert!(state.placed[0].is_empty());
    }

    #[test]
    fn dragging_from_circuit_hides_gate_until_drop() {
        let area = Rect::new(0, 0, 60, 20);
        let mut state = state_with_gate(Gate::H);
        let layout = circuit_layout(area, qubit_count(&state));
        let slot = layout.slots[0][0];
        handle_mouse_down(&mut state, slot.x, slot.y, area);
        assert_eq!(state.placed[0][0], None);
        handle_mouse_up(&mut state, slot.x, slot.y, area);
        assert_eq!(state.placed[0][0], Some(Gate::H));
    }

    fn buffer_to_line(buffer: &Buffer, area: Rect, y: u16) -> String {
        let mut line = String::new();
        for x in 0..area.width {
            let cell = buffer.get(area.x + x, area.y + y);
            line.push_str(cell.symbol());
        }
        line.trim_end().to_string()
    }

    fn buffer_to_string(buffer: &Buffer, x: u16, y: u16, len: u16) -> String {
        let mut line = String::new();
        for offset in 0..len {
            let cell = buffer.get(x + offset, y);
            line.push_str(cell.symbol());
        }
        line
    }
}
