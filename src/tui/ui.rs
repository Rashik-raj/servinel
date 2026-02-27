use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph, Tabs, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;

use crate::tui::app::TuiApp;

pub fn draw(frame: &mut Frame<'_>, app: &mut TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(frame.area());

    // Store layout areas for mouse interaction
    app.app_tab_area = chunks[0];
    app.service_tab_area = chunks[1];
    app.help_area = chunks[3];

    let app_titles: Vec<Line> = app
        .apps
        .iter()
        .map(|app| Line::from(app.app_name.clone()))
        .collect();
    let app_tabs = Tabs::new(app_titles)
        .block(Block::default().borders(Borders::ALL).title("Apps"))
        .select(app.selected_app)
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(app_tabs, chunks[0]);

    let service_titles: Vec<Line> = app
        .apps
        .get(app.selected_app)
        .map(|app| {
            app.services
                .iter()
                .map(|service| Line::from(service.name.clone()))
                .collect()
        })
        .unwrap_or_default();

    let service_tabs = Tabs::new(service_titles)
        .block(Block::default().borders(Borders::ALL).title("Services"))
        .select(app.selected_service)
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(service_tabs, chunks[1]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(chunks[2]);

    let log_area = body[0];
    app.log_area = log_area;

    let visible_height = log_area.height.saturating_sub(2) as usize;

    let (effective_scroll, scroll_x) = app.calculate_effective_scroll();
    app.last_effective_scroll = effective_scroll;
    app.last_effective_scroll_x = scroll_x;
    let max_scroll = app.logs.len().saturating_sub(visible_height);

    let mut log_lines = Vec::new();
    for log in &app.logs {
        log_lines.push(Line::from(vec![
            Span::styled(format!("[{}] ", log.timestamp), Style::default().fg(Color::DarkGray)),
            Span::styled(format!("[{}] ", log.service), Style::default().fg(Color::Blue)),
            Span::raw(&log.message),
        ]));
    }
    
    let logs = Paragraph::new(log_lines)
        .block(Block::default().borders(Borders::ALL).title("Logs"))
        .scroll((effective_scroll as u16, scroll_x));
    frame.render_widget(logs, log_area);

    let scrollbar = Scrollbar::default()
        .orientation(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"));
    let mut scrollbar_state = ScrollbarState::new(max_scroll).position(effective_scroll);
    frame.render_stateful_widget(
        scrollbar,
        log_area.inner(ratatui::layout::Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut scrollbar_state,
    );

    // Horizontal scrollbar
    // max_width should only consider the message part since selection is only on that
    let max_msg_width = app.logs.iter().map(|l| l.message.len()).max().unwrap_or(0);
    let visible_width = log_area.width.saturating_sub(2) as usize;
    let max_scroll_x = max_msg_width.saturating_sub(visible_width);

    let scrollbar_x = Scrollbar::default()
        .orientation(ScrollbarOrientation::HorizontalBottom)
        .thumb_symbol("■")
        .begin_symbol(Some("←"))
        .end_symbol(Some("→"));
    let mut scrollbar_x_state = ScrollbarState::new(max_scroll_x).position(scroll_x as usize);
    frame.render_stateful_widget(
        scrollbar_x,
        log_area.inner(ratatui::layout::Margin {
            vertical: 0,
            horizontal: 1,
        }),
        &mut scrollbar_x_state,
    );

    let stats_lines = if let Some(service) = app.selected_service() {
        vec![
            Line::from(vec![
                Span::raw("Status: "),
                Span::styled(service.status, Style::default().fg(Color::Green)),
            ]),
            Line::from(format!(
                "PID: {}",
                service
                    .pid
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "-".to_string())
            )),
            Line::from(format!(
                "Uptime: {}",
                service
                    .uptime_secs
                    .map(|u| format!("{u}s"))
                    .unwrap_or_else(|| "-".to_string())
            )),
            Line::from(format!(
                "Exit: {}",
                service
                    .exit_code
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "-".to_string())
            )),
            Line::from(format!("CPU: {:.2}%", service.metrics.cpu)),
            Line::from(format!(
                "Memory: {:.1} MB",
                service.metrics.memory as f64 / 1024.0 / 1024.0
            )),
        ]
    } else {
        vec![Line::from("No service selected")]
    };

    let status_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(9), Constraint::Min(0)])
        .split(body[1]);

    app.status_area = status_chunks[0];

    let stats =
        Paragraph::new(stats_lines).block(Block::default().borders(Borders::ALL).title("Status"));
    frame.render_widget(stats, status_chunks[0]);

    let pie_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(status_chunks[1]);

    let cpu_percent = app.system_cpu.clamp(0.0, 100.0) as f64;
    let mem_percent = if app.system_memory_total > 0 {
        (app.system_memory_used as f64 / app.system_memory_total as f64) * 100.0
    } else {
        0.0
    };
    let cpu_title = format!("CPU {:.1}%", cpu_percent);
    let cpu_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(cpu_title))
        .gauge_style(Style::default().fg(Color::LightRed))
        .percent(cpu_percent as u16);
    frame.render_widget(cpu_gauge, pie_chunks[0]);

    let mem_title = format!("RAM {:.1}%", mem_percent);
    let mem_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(mem_title))
        .gauge_style(Style::default().fg(Color::LightGreen))
        .percent(mem_percent as u16);
    frame.render_widget(mem_gauge, pie_chunks[1]);

    let help = Paragraph::new(
        "Keys: Tab/S-Tab apps  ←/→ services  ↑/↓ scroll  s start  x stop  r restart  q quit  │  drag to select & copy",
    )
    .block(Block::default().borders(Borders::ALL).title("Help"));
    frame.render_widget(help, chunks[3]);

    // ── Apply selection highlight ───────────────────
    if let Some((sr, sc, er, ec)) = app.selection_range() {
        let area = frame.area();
        let buf = frame.buffer_mut();
        let highlight = Style::default().bg(Color::White).fg(Color::Black);
        
        if app.selection_is_log {
            // Convert log line/char coordinates to screen coordinates
            let (sy, sx) = (app.last_effective_scroll, app.last_effective_scroll_x);
            let panel = app.log_area;
            
            for log_row in sr..=er {
                // Check if this log line is within the visible viewport
                if log_row < sy || log_row >= sy + (panel.height.saturating_sub(2) as usize) {
                    continue;
                }
                
                let line = match app.logs.get(log_row) {
                    Some(l) => l,
                    None => continue,
                };
                
                let screen_row = (panel.y + 1) + (log_row - sy) as u16;
                if screen_row >= area.height { break; }

                let (char_start, char_end, zone_offset) = match app.selection_zone {
                    crate::tui::app::LogSelectionZone::Metadata => {
                        let m_width = line.metadata_width();
                        let s = if log_row == sr { sc } else { 0 };
                        let e = if log_row == er { ec } else { m_width };
                        (s.min(m_width), e.min(m_width), 0usize)
                    }
                    crate::tui::app::LogSelectionZone::Message => {
                        let m_width = line.metadata_width();
                        let s = if log_row == sr { sc } else { 0 };
                        let e = if log_row == er { ec } else { line.message.len() };
                        (s, e, m_width)
                    }
                    _ => (0, 0, 0),
                };
                
                for zone_col in char_start..char_end {
                    let log_col = zone_col + zone_offset;
                    if log_col < sx as usize || log_col >= sx as usize + (panel.width.saturating_sub(2) as usize) {
                        continue;
                    }
                    let screen_col = (panel.x + 1) + (log_col - sx as usize) as u16;
                    if screen_col >= area.width { break; }
                    
                    let pos = ratatui::layout::Position { x: screen_col, y: screen_row };
                    if let Some(cell) = buf.cell_mut(pos) {
                        cell.set_style(highlight);
                    }
                }
            }
        } else {
            // General screen-based highlighting (for other panels)
            for row in sr..=er {
                let row_u16 = row as u16;
                if row_u16 >= area.height { break; }
                let col_start = if row == sr { sc as u16 } else {
                    app.selection_panel.map_or(0, |p| p.x)
                };
                let col_end = if row == er { ec as u16 } else {
                    app.selection_panel.map_or(area.width, |p| p.x + p.width)
                };
                for col in col_start..col_end {
                    if col >= area.width { break; }
                    let pos = ratatui::layout::Position { x: col, y: row_u16 };
                    if let Some(cell) = buf.cell_mut(pos) {
                        cell.set_style(highlight);
                    }
                }
            }
        }
    }
}


