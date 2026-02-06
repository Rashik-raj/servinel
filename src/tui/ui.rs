use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Tabs, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;
use tui_piechart::{PieChart, PieSlice};

use crate::tui::app::TuiApp;

pub fn draw(frame: &mut Frame<'_>, app: &TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(frame.area());

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
    let visible_height = log_area.height.saturating_sub(2) as usize; // Borders
    let total_lines = app.logs.len();
    let max_scroll = total_lines.saturating_sub(visible_height);

    let effective_scroll = if app.autoscroll {
        max_scroll
    } else {
        app.scroll.min(max_scroll)
    };

    let log_text = Text::from(app.logs.join("\n"));
    let logs = Paragraph::new(log_text)
        .block(Block::default().borders(Borders::ALL).title("Logs"))
        .scroll((effective_scroll as u16, app.scroll_x)); // Use scroll_x here
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
    let max_width = app.logs.iter().map(|l| l.len()).max().unwrap_or(0);
    // Arbitrary reasonable max scroll view width, or try to detect visible width. 
    // Usually we want max_width - visible_width.
    let visible_width = log_area.width.saturating_sub(2) as usize; 
    let max_scroll_x = max_width.saturating_sub(visible_width);
    
    let scrollbar_x = Scrollbar::default()
        .orientation(ScrollbarOrientation::HorizontalBottom)
        .thumb_symbol("■")
        .begin_symbol(Some("←"))
        .end_symbol(Some("→"));
    let mut scrollbar_x_state = ScrollbarState::new(max_scroll_x).position(app.scroll_x as usize);
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
            Line::from(format!("Memory: {:.1} MB", service.metrics.memory as f64 / 1024.0 / 1024.0)),
        ]
    } else {
        vec![Line::from("No service selected")]
    };

    let status_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(9), Constraint::Min(0)])
        .split(body[1]);

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
    let cpu_chart = pie_widget("CPU", cpu_percent, Color::LightRed, Color::DarkGray);
    let mem_chart = pie_widget("RAM", mem_percent, Color::LightGreen, Color::DarkGray);
    frame.render_widget(cpu_chart, pie_chunks[0]);
    frame.render_widget(mem_chart, pie_chunks[1]);

    let help = Paragraph::new(
        "Keys: Tab/Shift+Tab apps  Left/Right services  Up/Down scroll  Shift+Up/Down scroll horiz  s start  x stop  r restart  q quit",
    )
    .block(Block::default().borders(Borders::ALL).title("Help"));
    frame.render_widget(help, chunks[3]);
}

fn pie_widget<'a>(
    title: &'a str,
    percent: f64,
    used_color: Color,
    free_color: Color,
) -> PieChart<'a> {
    let percent = percent.clamp(0.0, 100.0);
    let used = percent as f64;
    let free = 100.0 - percent;
    let slices = vec![
        PieSlice::new("Used", used, used_color),
        PieSlice::new("Free", free, free_color),
    ];
    PieChart::new(slices)
        .block(Block::default().borders(Borders::ALL).title(title))
        .show_percentages(true)
        .show_legend(true)
        .resolution(tui_piechart::Resolution::Braille)
}
