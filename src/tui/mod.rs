use std::io::{self, Stdout};
use std::sync::mpsc;
use std::time::Duration;

use crossterm::event::{Event, KeyCode, MouseEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;
use crossterm::event::{EnableMouseCapture, DisableMouseCapture};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::error::Result;
use crate::ipc::client::{request_response, stream_logs};
use crate::ipc::protocol::{format_log_entry, Request, Response, ServiceSelector};
use crate::tui::app::TuiApp;

mod app;
mod ui;

pub async fn run() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || loop {
        if let Ok(event) = crossterm::event::read() {
            if tx.send(event).is_err() {
                break;
            }
        }
    });

    let mut app = TuiApp::default();
    let mut interval = tokio::time::interval(Duration::from_millis(50));
    let mut should_quit = false;

    terminal.draw(|frame| ui::draw(frame, &mut app))?;

    while !should_quit {
        interval.tick().await;
        while let Ok(event) = rx.try_recv() {
            match event {
                Event::Key(key) => {
                    match key.code {
                        KeyCode::Char('q') => {
                            should_quit = true;
                        }
                        KeyCode::Tab => app.next_app(),
                        KeyCode::BackTab => app.prev_app(),
                        KeyCode::Left => app.prev_service(),
                        KeyCode::Right => app.next_service(),
                        KeyCode::Up => {
                            if key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) {
                                app.scroll_right();
                            } else {
                                app.scroll_up();
                            }
                        }
                        KeyCode::Down => {
                            if key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) {
                                app.scroll_left();
                            } else {
                                app.scroll_down();
                            }
                        }
                        KeyCode::PageUp => app.page_up(),
                        KeyCode::PageDown => app.page_down(),
                        KeyCode::Home => app.scroll_to_top(),
                        KeyCode::End => app.scroll_to_bottom(),
                        KeyCode::Char('s') => {
                            if let (Some(app_name), Some(service)) =
                                (app.selected_app_name(), app.selected_service_name())
                            {
                                let _ = request_response(&Request::Start {
                                    file: None,
                                    app: Some(app_name),
                                    selector: ServiceSelector::Service(service),
                                })
                                .await;
                            }
                        }
                        KeyCode::Char('x') => {
                            if let (Some(app_name), Some(service)) =
                                (app.selected_app_name(), app.selected_service_name())
                            {
                                let _ = request_response(&Request::Stop {
                                    app: Some(app_name),
                                    selector: ServiceSelector::Service(service),
                                })
                                .await;
                            }
                        }
                        KeyCode::Char('r') => {
                            if let (Some(app_name), Some(service)) =
                                (app.selected_app_name(), app.selected_service_name())
                            {
                                let _ = request_response(&Request::Restart {
                                    app: Some(app_name),
                                    selector: ServiceSelector::Service(service),
                                })
                                .await;
                            }
                        }
                        _ => {}
                    }
                }
                Event::Mouse(mouse) => {
                    match mouse.kind {
                        MouseEventKind::ScrollUp => {
                            if mouse.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) 
                               || mouse.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                                app.scroll_left();
                            } else {
                                app.scroll_up();
                            }
                        }
                        MouseEventKind::ScrollDown => {
                            if mouse.modifiers.contains(crossterm::event::KeyModifiers::SHIFT)
                               || mouse.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                                app.scroll_right();
                            } else {
                                app.scroll_down();
                            }
                        }
                        MouseEventKind::ScrollLeft => {
                            app.scroll_left();
                        }
                        MouseEventKind::ScrollRight => {
                            app.scroll_right();
                        }
                        MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                            app.click_app_tab(mouse.column, mouse.row);
                            app.click_service_tab(mouse.column, mouse.row);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        refresh_status(&mut app).await?;
        refresh_logs(&mut app).await?;

        terminal.draw(|frame| ui::draw(frame, &mut app))?;
    }

    restore_terminal(terminal)?;
    Ok(())
}

async fn refresh_status(app: &mut TuiApp) -> Result<()> {
    let request = Request::Status {
        app: None,
        selector: ServiceSelector::All,
    };
    let response = tokio::time::timeout(Duration::from_millis(500), request_response(&request)).await;
    let Ok(Ok(Response::StatusSnapshot(snapshot))) = response else {
        return Ok(());
    };
    app.update_snapshot(
        snapshot.apps,
        snapshot.system_cpu,
        snapshot.system_memory_used,
        snapshot.system_memory_total,
    );
    Ok(())
}

async fn refresh_logs(app: &mut TuiApp) -> Result<()> {
    let (app_name, service) = match (app.selected_app_name(), app.selected_service_name()) {
        (Some(app_name), Some(service)) => (app_name, service),
        _ => {
            app.logs.clear();
            return Ok(());
        }
    };

    let request = Request::Logs {
        app: Some(app_name.clone()),
        selector: ServiceSelector::Service(service.clone()),
        follow: false,
        tail: Some(200),
        merged: true,
    };

    let mut lines = Vec::new();
    let response = tokio::time::timeout(
        Duration::from_millis(600),
        stream_logs(&request, |chunk| {
            lines.push(format_log_entry(&chunk.entry, true, &chunk.service));
        }),
    )
    .await;
    if response.is_err() {
        return Ok(());
    }
    if response.unwrap().is_err() {
        return Ok(());
    }
    app.logs = lines;
    Ok(())
}

fn restore_terminal(mut terminal: Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    Ok(())
}
