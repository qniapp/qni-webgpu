use std::time::{Duration, Instant};

use ratatui::layout::Rect;
use ratatui_testlib::{CommandBuilder, KeyCode, Result, TermTestError, TuiTestHarness};

use qni_webgpu_tui::{circuit_layout, hit_test_palette, layout_regions, Gate};

const TERM_WIDTH: u16 = 120;
const TERM_HEIGHT: u16 = 40;

fn palette_coord(area: Rect, gate: Gate) -> Option<(u16, u16)> {
    let regions = layout_regions(area, 2);
    for y in regions.palette.y..regions.palette.y + regions.palette.height {
        for x in regions.palette.x..regions.palette.x + regions.palette.width {
            if hit_test_palette(x, y, area) == Some(gate) {
                return Some((x, y));
            }
        }
    }
    None
}

fn mouse_down(harness: &mut TuiTestHarness, x: u16, y: u16) -> Result<()> {
    // SGR 1006 mouse press (1-based coordinates).
    let seq = format!("\x1b[<0;{};{}M", x + 1, y + 1);
    harness.send_text(&seq)
}

fn mouse_drag(harness: &mut TuiTestHarness, x: u16, y: u16) -> Result<()> {
    // SGR 1006 mouse drag with left button (32 + 0).
    let seq = format!("\x1b[<32;{};{}M", x + 1, y + 1);
    harness.send_text(&seq)
}

fn mouse_up(harness: &mut TuiTestHarness, x: u16, y: u16) -> Result<()> {
    // SGR 1006 mouse release (1-based coordinates).
    let seq = format!("\x1b[<0;{};{}m", x + 1, y + 1);
    harness.send_text(&seq)
}

fn wait_for_exit(harness: &mut TuiTestHarness, timeout: Duration) -> Result<()> {
    let start = Instant::now();
    while harness.is_running() {
        if start.elapsed() >= timeout {
            return Err(TermTestError::Timeout {
                timeout_ms: timeout.as_millis() as u64,
            });
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

#[test]
fn drag_h_gate_to_first_slot() -> Result<()> {
    let mut harness = TuiTestHarness::builder()
        .with_size(TERM_WIDTH, TERM_HEIGHT)
        .with_timeout(Duration::from_secs(4))
        .with_poll_interval(Duration::from_millis(50))
        .build()?;

    let mut cmd = CommandBuilder::new("timeout");
    cmd.arg("5s");
    cmd.arg(env!("CARGO_BIN_EXE_qni-webgpu-tui"));
    harness.spawn(cmd)?;
    harness.wait_for_text("q0:")?;
    harness.wait_for_text("q1:")?;

    let area = Rect::new(0, 0, TERM_WIDTH, TERM_HEIGHT);
    let (start_x, start_y) = palette_coord(area, Gate::H).expect("H gate in palette");

    // Dragging adds a temporary row, so use 3-qubit layout for drop coordinates.
    let drag_layout = circuit_layout(area, 3);
    let target = drag_layout.slots[0][0];
    let target_x = target.x + target.width / 2;
    let target_y = target.y + target.height / 2;

    mouse_down(&mut harness, start_x, start_y)?;
    mouse_drag(&mut harness, target_x, target_y)?;
    mouse_up(&mut harness, target_x, target_y)?;

    // After dropping, the empty row is trimmed, so the 2-qubit layout applies.
    let drop_layout = circuit_layout(area, 2);
    let slot = drop_layout.slots[0][0];
    let label_x = slot.x + slot.width / 2;
    let label_y = slot.y + slot.height / 2;

    harness.wait_for(|state| state.text_at(label_y, label_x) == Some('H'))?;

    harness.send_key(KeyCode::Char('q'))?;
    harness.send_key(KeyCode::Char('y'))?;
    harness.send_key(KeyCode::Enter)?;
    wait_for_exit(&mut harness, Duration::from_secs(6))?;
    Ok(())
}
