use std::io;

use crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use crossterm::execute;
use crossterm::terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use qni_webgpu_tui::{
    handle_mouse_down, handle_mouse_move, handle_mouse_up, parse_args, render_to_buffer_with_drag,
    update_hovered_slot, AppState, DragVisual,
};

fn draw_once(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut AppState,
    debug_line: Option<&str>,
    drag_visual: Option<DragVisual>,
) -> io::Result<()> {
    terminal.draw(|frame| {
        let area = frame.size();
        let buffer = render_to_buffer_with_drag(state, area, debug_line, drag_visual);
        let frame_buffer = frame.buffer_mut();
        for y in 0..area.height {
            for x in 0..area.width {
                let cell = buffer.get(area.x + x, area.y + y).clone();
                *frame_buffer.get_mut(area.x + x, area.y + y) = cell;
            }
        }
    })?;
    Ok(())
}

fn run() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let gate = parse_args(&args);
    let mut app_state = AppState::new(gate);
    let mut debug_line: Option<String> = None;

    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        terminal::EnterAlternateScreen,
        event::EnableMouseCapture
    )?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    terminal.clear()?;
    loop {
        let drag_visual = app_state.dragging.and_then(|drag| {
            app_state.drag_pos.map(|(x, y)| DragVisual {
                gate: drag.gate,
                x,
                y,
            })
        });
        draw_once(
            &mut terminal,
            &mut app_state,
            debug_line.as_deref(),
            drag_visual,
        )?;
        if event::poll(std::time::Duration::from_millis(250))? {
            match event::read()? {
                Event::Key(key) => {
                    debug_line = Some(format!("key={:?} mods={:?}", key.code, key.modifiers));
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        break;
                    }
                }
                Event::Mouse(mouse) => {
                    let area = terminal.size()?;
                    debug_line = Some(format!(
                        "mouse={:?} mods={:?} x={} y={}",
                        mouse.kind, mouse.modifiers, mouse.column, mouse.row
                    ));
                    match mouse.kind {
                        MouseEventKind::Down(MouseButton::Left) => {
                            handle_mouse_down(&mut app_state, mouse.column, mouse.row, area);
                            update_hovered_slot(&mut app_state, mouse.column, mouse.row, area);
                        }
                        MouseEventKind::Drag(MouseButton::Left) => {
                            if app_state.dragging.is_none() {
                                handle_mouse_down(&mut app_state, mouse.column, mouse.row, area);
                            }
                            handle_mouse_move(&mut app_state, mouse.column, mouse.row);
                            update_hovered_slot(&mut app_state, mouse.column, mouse.row, area);
                        }
                        MouseEventKind::Moved => {
                            handle_mouse_move(&mut app_state, mouse.column, mouse.row);
                            update_hovered_slot(&mut app_state, mouse.column, mouse.row, area);
                        }
                        MouseEventKind::Up(MouseButton::Left) => {
                            handle_mouse_up(&mut app_state, mouse.column, mouse.row, area);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let result = run();
    terminal::disable_raw_mode()?;
    let _ = execute!(
        io::stdout(),
        event::DisableMouseCapture,
        terminal::LeaveAlternateScreen
    );
    result
}
