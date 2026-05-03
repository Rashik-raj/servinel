use std::collections::HashMap;

use crate::ipc::protocol::{AppSnapshot, ServiceSnapshot};
use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogSelectionZone {
    None,
    Metadata,
    Message,
}

#[derive(Debug, Clone)]
pub struct LogLine {
    pub timestamp: String,
    pub message: String,
}

impl LogLine {
    pub fn metadata_width(&self) -> usize {
        // [YYYY-MM-DD HH:MM:SS]
        // 2 (brackets) + timestamp.len() + 1 (space)
        self.timestamp.len() + 3
    }
}

#[derive(Debug)]
pub struct TuiApp {
    pub apps: Vec<AppSnapshot>,
    pub selected_app: usize,
    pub selected_service: usize,
    pub logs: Vec<LogLine>,
    pub system_cpu: f32,
    pub system_memory_used: u64,
    pub system_memory_total: u64,
    pub scroll: usize,
    pub autoscroll: bool,
    /// Last effective scroll used during rendering, for coordinate conversion
    pub last_effective_scroll: usize,
    pub last_effective_scroll_x: u16,
    /// Stored layout areas for mouse click detection and panel-constrained selection
    pub app_tab_area: Rect,
    pub service_tab_area: Rect,
    pub log_area: Rect,
    pub status_area: Rect,
    pub help_area: Rect,
    /// Screen buffer captured after each draw, for text extraction
    pub screen_buffer: Vec<String>,
    /// The panel rect that the current selection is constrained to
    pub selection_panel: Option<Rect>,
    /// Selection anchor: (row, col). For logs, this is (line_idx, char_idx).
    /// For others, it's screen coordinates.
    pub selection_anchor: Option<(usize, usize)>,
    /// Selection end: (row, col). For logs, this is (line_idx, char_idx).
    /// For others, it's screen coordinates.
    pub selection_end: Option<(usize, usize)>,
    /// Whether the current selection is anchored to log data coordinates
    pub selection_is_log: bool,
    /// Which zone within the log line is being selected
    pub selection_zone: LogSelectionZone,
    /// Whether a drag selection is in progress
    pub selecting: bool,
    /// Per-service horizontal scroll: (app_name, service_name) -> scroll_x
    pub per_service_scroll_x: HashMap<(String, String), u16>,
}

impl Default for TuiApp {
    fn default() -> Self {
        Self {
            apps: Vec::new(),
            selected_app: 0,
            selected_service: 0,
            logs: Vec::new(),
            system_cpu: 0.0,
            system_memory_used: 0,
            system_memory_total: 0,
            scroll: 0,
            autoscroll: true,
            last_effective_scroll: 0,
            last_effective_scroll_x: 0,
            app_tab_area: Rect::default(),
            service_tab_area: Rect::default(),
            log_area: Rect::default(),
            status_area: Rect::default(),
            help_area: Rect::default(),
            screen_buffer: Vec::new(),
            selection_panel: None,
            selection_anchor: None,
            selection_end: None,
            selection_is_log: false,
            selection_zone: LogSelectionZone::None,
            selecting: false,
            per_service_scroll_x: HashMap::new(),
        }
    }
}

impl TuiApp {
    pub fn update_snapshot(
        &mut self,
        snapshot: Vec<AppSnapshot>,
        system_cpu: f32,
        system_memory_used: u64,
        system_memory_total: u64,
    ) {
        self.apps = snapshot;
        self.system_cpu = system_cpu;
        self.system_memory_used = system_memory_used;
        self.system_memory_total = system_memory_total;
        if self.selected_app >= self.apps.len() {
            self.selected_app = self.apps.len().saturating_sub(1);
        }
        if let Some(app) = self.apps.get(self.selected_app) {
            if self.selected_service >= app.services.len() {
                self.selected_service = app.services.len().saturating_sub(1);
            }
        } else {
            self.selected_service = 0;
        }
    }

    pub fn next_app(&mut self) {
        if !self.apps.is_empty() {
            self.selected_app = (self.selected_app + 1) % self.apps.len();
            self.selected_service = 0;
            self.reset_scroll();
        }
    }

    pub fn prev_app(&mut self) {
        if !self.apps.is_empty() {
            if self.selected_app == 0 {
                self.selected_app = self.apps.len() - 1;
            } else {
                self.selected_app -= 1;
            }
            self.selected_service = 0;
            self.reset_scroll();
        }
    }

    pub fn next_service(&mut self) {
        if let Some(app) = self.apps.get(self.selected_app) {
            if !app.services.is_empty() {
                self.selected_service = (self.selected_service + 1) % app.services.len();
                self.reset_scroll();
            }
        }
    }

    pub fn prev_service(&mut self) {
        if let Some(app) = self.apps.get(self.selected_app) {
            if !app.services.is_empty() {
                if self.selected_service == 0 {
                    self.selected_service = app.services.len() - 1;
                } else {
                    self.selected_service -= 1;
                }
                self.reset_scroll();
            }
        }
    }

    pub fn selected_app_name(&self) -> Option<String> {
        self.apps
            .get(self.selected_app)
            .map(|app| app.app_name.clone())
    }

    pub fn selected_service_name(&self) -> Option<String> {
        self.apps
            .get(self.selected_app)
            .and_then(|app| app.services.get(self.selected_service))
            .map(|svc| svc.name.clone())
    }

    pub fn selected_service(&self) -> Option<ServiceSnapshot> {
        self.apps
            .get(self.selected_app)
            .and_then(|app| app.services.get(self.selected_service))
            .cloned()
    }

    pub fn scroll_up(&mut self) {
        if self.autoscroll {
            self.autoscroll = false;
            self.scroll = self.logs.len().saturating_sub(1);
        } else if self.scroll > 0 {
            self.scroll -= 1;
        }
    }

    pub fn scroll_down(&mut self) {
        if !self.autoscroll {
            self.scroll += 1;
            if self.scroll >= self.logs.len() {
                self.autoscroll = true;
            }
        }
    }

    pub fn scroll_left(&mut self) {
        let scroll_x = self.effective_scroll_x();
        let new_val = if scroll_x > 0 { scroll_x.saturating_sub(5) } else { 0 };
        self.set_effective_scroll_x(new_val);
    }

    pub fn scroll_right(&mut self) {
        let scroll_x = self.effective_scroll_x();
        self.set_effective_scroll_x(scroll_x.saturating_add(5));
    }

    pub fn page_up(&mut self) {
        let page_size = 15;
        if self.autoscroll {
            self.autoscroll = false;
            self.scroll = self.logs.len().saturating_sub(page_size);
        } else {
            self.scroll = self.scroll.saturating_sub(page_size);
        }
    }

    pub fn page_down(&mut self) {
        let page_size = 15;
        if !self.autoscroll {
            self.scroll += page_size;
            if self.scroll >= self.logs.len() {
                self.autoscroll = true;
            }
        }
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll = 0;
        self.autoscroll = false;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.autoscroll = true;
    }

    pub fn calculate_effective_scroll(&self) -> (usize, u16) {
        let visible_height = self.log_area.height.saturating_sub(2) as usize;
        let total_lines = self.logs.len();
        let max_scroll = total_lines.saturating_sub(visible_height);

        let sy = if self.autoscroll {
            max_scroll
        } else {
            self.scroll.min(max_scroll)
        };
        let sx = self.effective_scroll_x();
        (sy, sx)
    }

    /// Get the effective horizontal scroll for the currently selected service.
    fn effective_scroll_x(&self) -> u16 {
        if let (Some(app_name), Some(service_name)) = (self.selected_app_name(), self.selected_service_name()) {
            self.per_service_scroll_x
                .get(&(app_name, service_name))
                .copied()
                .unwrap_or(0)
        } else {
            0
        }
    }

    /// Set the horizontal scroll for the currently selected service.
    fn set_effective_scroll_x(&mut self, value: u16) {
        if let (Some(app_name), Some(service_name)) = (self.selected_app_name(), self.selected_service_name()) {
            self.per_service_scroll_x.insert((app_name, service_name), value);
        }
    }

    fn reset_scroll(&mut self) {
        self.scroll = 0;
        self.autoscroll = true;
    }

    // ── Tab click handling ──────────────────────────────────────────────

    pub fn click_app_tab(&mut self, column: u16, row: u16) -> bool {
        let area = self.app_tab_area;
        if row < area.y || row >= area.y + area.height
            || column < area.x || column >= area.x + area.width
        {
            return false;
        }
        let names: Vec<String> = self.apps.iter().map(|a| a.app_name.clone()).collect();
        if let Some(idx) = Self::tab_index_at(column, area, &names) {
            if idx < self.apps.len() && idx != self.selected_app {
                self.selected_app = idx;
                self.selected_service = 0;
                self.reset_scroll();
                return true;
            }
        }
        false
    }

    pub fn click_service_tab(&mut self, column: u16, row: u16) -> bool {
        let area = self.service_tab_area;
        if row < area.y || row >= area.y + area.height
            || column < area.x || column >= area.x + area.width
        {
            return false;
        }
        let names: Vec<String> = self
            .apps
            .get(self.selected_app)
            .map(|app| app.services.iter().map(|s| s.name.clone()).collect())
            .unwrap_or_default();
        if let Some(idx) = Self::tab_index_at(column, area, &names) {
            if idx < names.len() && idx != self.selected_service {
                self.selected_service = idx;
                self.reset_scroll();
                return true;
            }
        }
        false
    }

    fn tab_index_at(column: u16, area: Rect, names: &[String]) -> Option<usize> {
        if names.is_empty() {
            return None;
        }
        let rel_x = column.saturating_sub(area.x + 1) as usize;
        let mut pos = 0;
        for (i, name) in names.iter().enumerate() {
            let tab_width = name.len() + 2;
            if rel_x < pos + tab_width {
                return Some(i);
            }
            pos += tab_width;
            if i < names.len() - 1 {
                pos += 1;
            }
        }
        None
    }

    // ── Panel-constrained text selection (screen coordinates) ───────────

    /// Check if a point is inside a rect.
    fn point_in_rect(col: u16, row: u16, r: Rect) -> bool {
        col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height
    }

    /// Find which panel a click belongs to.
    fn panel_at(&self, col: u16, row: u16) -> Option<Rect> {
        let panels = [
            self.app_tab_area,
            self.service_tab_area,
            self.log_area,
            self.status_area,
            self.help_area,
        ];
        panels.into_iter().find(|r| Self::point_in_rect(col, row, *r))
    }

    /// Clamp a coordinate to stay within a rect (inner area, excluding borders).
    fn clamp_to_panel(col: u16, row: u16, panel: Rect) -> (u16, u16) {
        let min_x = panel.x;
        let max_x = panel.x + panel.width.saturating_sub(1);
        let min_y = panel.y;
        let max_y = panel.y + panel.height.saturating_sub(1);
        (col.clamp(min_x, max_x), row.clamp(min_y, max_y))
    }

    /// Begin a new text selection at the given screen position.
    pub fn start_selection(&mut self, column: u16, row: u16) {
        if let Some(panel) = self.panel_at(column, row) {
            self.selection_panel = Some(panel);
            self.selecting = true;
            
            if panel == self.log_area {
                self.selection_is_log = true;
                let sy = self.last_effective_scroll;
                let sx = self.last_effective_scroll_x;
                let log_row = (row.saturating_sub(panel.y + 1) as usize) + sy;
                let log_col_screen = column.saturating_sub(panel.x + 1) as usize + sx as usize;
                
                if let Some(line) = self.logs.get(log_row) {
                    let m_width = line.metadata_width();
                    if log_col_screen < m_width {
                        self.selection_zone = LogSelectionZone::Metadata;
                        self.selection_anchor = Some((log_row, log_col_screen));
                        self.selection_end = Some((log_row, log_col_screen));
                    } else {
                        self.selection_zone = LogSelectionZone::Message;
                        let msg_col = log_col_screen - m_width;
                        self.selection_anchor = Some((log_row, msg_col));
                        self.selection_end = Some((log_row, msg_col));
                    }
                } else {
                    self.selection_zone = LogSelectionZone::Message;
                    self.selection_anchor = Some((log_row, 0));
                    self.selection_end = Some((log_row, 0));
                }
            } else {
                self.selection_is_log = false;
                self.selection_zone = LogSelectionZone::None;
                self.selection_anchor = Some((row as usize, column as usize));
                self.selection_end = Some((row as usize, column as usize));
            }
        }
    }

    /// Extend the current selection, clamped to the originating panel.
    pub fn update_selection(&mut self, column: u16, row: u16) {
        if !self.selecting {
            return;
        }
        if let Some(panel) = self.selection_panel {
            let (c, r) = Self::clamp_to_panel(column, row, panel);
            
            if self.selection_is_log {
                let sy = self.last_effective_scroll;
                let sx = self.last_effective_scroll_x;
                let log_row = (r.saturating_sub(panel.y + 1) as usize) + sy;
                let log_col_screen = c.saturating_sub(panel.x + 1) as usize + sx as usize;
                
                match self.selection_zone {
                    LogSelectionZone::Metadata => {
                        self.selection_end = Some((log_row, log_col_screen));
                    }
                    LogSelectionZone::Message => {
                        // Clamp to message area if possible
                        let m_width = self.logs.get(log_row).map(|l| l.metadata_width()).unwrap_or(0);
                        let msg_col = log_col_screen.saturating_sub(m_width);
                        self.selection_end = Some((log_row, msg_col));
                    }
                    LogSelectionZone::None => {
                        self.selection_end = Some((log_row, log_col_screen));
                    }
                }
            } else {
                self.selection_end = Some((r as usize, c as usize));
            }
        }
    }

    /// Finalize the selection (mouse released).
    pub fn finish_selection(&mut self) {
        self.selecting = false;
    }

    /// Clear any active selection.
    pub fn clear_selection(&mut self) {
        self.selection_panel = None;
        self.selection_anchor = None;
        self.selection_end = None;
        self.selection_is_log = false;
        self.selection_zone = LogSelectionZone::None;
        self.selecting = false;
    }

    /// Returns the normalized selection range: (start_row, start_col, end_row, end_col).
    pub fn selection_range(&self) -> Option<(usize, usize, usize, usize)> {
        match (self.selection_anchor, self.selection_end) {
            (Some((sr, sc)), Some((er, ec))) => {
                if (sr, sc) == (er, ec) {
                    return None;
                }
                if sr < er || (sr == er && sc <= ec) {
                    Some((sr, sc, er, ec))
                } else {
                    Some((er, ec, sr, sc))
                }
            }
            _ => None,
        }
    }

    /// Extract selected text from the screen buffer or logs history.
    pub fn get_selected_text(&self) -> Option<String> {
        let (sr, sc, er, ec) = self.selection_range()?;
        
        let mut lines: Vec<String> = Vec::new();
        
        if self.selection_is_log {
            for row in sr..=er {
                if row >= self.logs.len() {
                    break;
                }
                
                let line = &self.logs[row];
                let line_chars: Vec<char> = match self.selection_zone {
                    LogSelectionZone::Metadata => {
                        format!("[{}] ", line.timestamp).chars().collect()
                    }
                    LogSelectionZone::Message => {
                        line.message.chars().collect()
                    }
                    LogSelectionZone::None => {
                        Vec::new()
                    }
                };
                
                let line_len = line_chars.len();

                let extracted = if sr == er {
                    let s = sc.min(line_len);
                    let e = ec.min(line_len);
                    line_chars[s..e].iter().collect::<String>()
                } else if row == sr {
                    let s = sc.min(line_len);
                    line_chars[s..].iter().collect::<String>()
                } else if row == er {
                    let e = ec.min(line_len);
                    line_chars[..e].iter().collect::<String>()
                } else {
                    line_chars.iter().collect::<String>()
                };

                lines.push(extracted.trim_end().to_string());
            }
        } else {
            for row in sr..=er {
                if row >= self.screen_buffer.len() {
                    break;
                }
                let line_chars: Vec<char> = self.screen_buffer[row].chars().collect();
                let line_len = line_chars.len();

                let panel = self.selection_panel.unwrap_or(Rect::default());
                let inner_left = (panel.x + 1) as usize;
                let inner_right = (panel.x + panel.width).saturating_sub(1) as usize;

                let (row_start, row_end) = if sr == er {
                    (sc, ec)
                } else if row == sr {
                    (sc, inner_right)
                } else if row == er {
                    (inner_left, ec)
                } else {
                    (inner_left, inner_right)
                };

                let s = row_start.clamp(inner_left, inner_right).min(line_len);
                let e = row_end.clamp(inner_left, inner_right).min(line_len);
                let extracted = if s < e {
                    line_chars[s..e].iter().collect::<String>()
                } else {
                    String::new()
                };

                lines.push(extracted.trim_end().to_string());
            }
        }

        let result = lines.join("\n");
        if result.trim().is_empty() { None } else { Some(result) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::protocol::{AppSnapshot, ServiceSnapshot};
    use crate::metrics::ServiceMetrics;

    fn setup_app() -> TuiApp {
        let mut app = TuiApp::default();
        app.apps = vec![AppSnapshot {
            app_name: "test-app".to_string(),
            services: vec![
                ServiceSnapshot {
                    name: "svc1".to_string(),
                    status: "running".to_string(),
                    pid: Some(123),
                    uptime_secs: Some(10),
                    exit_code: None,
                    metrics: ServiceMetrics::default(),
                },
                ServiceSnapshot {
                    name: "svc2".to_string(),
                    status: "running".to_string(),
                    pid: Some(124),
                    uptime_secs: Some(5),
                    exit_code: None,
                    metrics: ServiceMetrics::default(),
                },
            ],
        }];
        app.selected_app = 0;
        app.selected_service = 0;
        app.logs = (0..100)
            .map(|i| LogLine {
                timestamp: "2024-01-01 00:00:00".to_string(),
                message: format!("Log line {}", i),
            })
            .collect();
        app
    }

    #[test]
    fn test_vertical_scroll() {
        let mut app = setup_app();
        assert!(app.autoscroll);

        // Scroll up should disable autoscroll
        app.scroll_up();
        assert!(!app.autoscroll);
        assert_eq!(app.scroll, 99);

        app.scroll_up();
        assert_eq!(app.scroll, 98);

        app.scroll_down();
        assert_eq!(app.scroll, 99);

        // Scrolling past bottom should re-enable autoscroll
        app.scroll_down();
        assert!(app.autoscroll);
    }

    #[test]
    fn test_horizontal_scroll() {
        let mut app = setup_app();
        
        // Initial scroll_x should be 0
        assert_eq!(app.effective_scroll_x(), 0);

        app.scroll_right();
        assert_eq!(app.effective_scroll_x(), 5);

        app.scroll_right();
        assert_eq!(app.effective_scroll_x(), 10);

        app.scroll_left();
        assert_eq!(app.effective_scroll_x(), 5);

        app.scroll_left();
        assert_eq!(app.effective_scroll_x(), 0);

        // Should not go below 0
        app.scroll_left();
        assert_eq!(app.effective_scroll_x(), 0);
    }

    #[test]
    fn test_per_service_horizontal_scroll() {
        let mut app = setup_app();
        
        // svc1 scroll_right
        app.scroll_right();
        assert_eq!(app.effective_scroll_x(), 5);

        // Switch to svc2
        app.next_service();
        assert_eq!(app.selected_service, 1);
        assert_eq!(app.effective_scroll_x(), 0);

        app.scroll_right();
        app.scroll_right();
        assert_eq!(app.effective_scroll_x(), 10);

        // Switch back to svc1
        app.prev_service();
        assert_eq!(app.selected_service, 0);
        assert_eq!(app.effective_scroll_x(), 5);
    }

    #[test]
    fn test_page_scroll() {
        let mut app = setup_app();
        app.autoscroll = false;
        app.scroll = 50;

        app.page_up();
        assert_eq!(app.scroll, 35);

        app.page_down();
        assert_eq!(app.scroll, 50);

        app.scroll_to_top();
        assert_eq!(app.scroll, 0);
        assert!(!app.autoscroll);

        app.scroll_to_bottom();
        assert!(app.autoscroll);
    }

    #[test]
    fn test_selection_extraction_constrained() {
        let mut app = TuiApp::default();
        // 20 wide terminal for simplicity
        // 01234567890123456789
        // LLLLLLLLLL|SSSSSSSSSS
        app.screen_buffer = vec![
            "LLLLLLLLLLSSSSSSSSSS".to_string(),
            "LLLLLLLLLLSSSSSSSSSS".to_string(),
        ];
        
        // Status panel is at x=10, width=10
        let status_panel = Rect { x: 10, y: 0, width: 10, height: 2 };
        app.selection_panel = Some(status_panel);
        app.selection_is_log = false;
        
        // Select from (0, 10) to (1, 15)
        // Row 0, Col 10 (start of status)
        // Row 1, Col 15 (middle of status)
        app.selection_anchor = Some((0, 10));
        app.selection_end = Some((1, 15));
        
        let selected = app.get_selected_text().unwrap();
        // Line 0: index 11 to 20 (clamped to 19) -> "SSSSSSSSS"
        // Line 1: index 10 (clamped to 11) to 15 -> "SSSS"
        // Note: inner_left=11, inner_right=19. 
        // Row 0 start 10 -> clamped to 11. End is inner_right=19. 11..19 is 8 chars.
        // Row 1 start inner_left=11. End is 15. 11..15 is 4 chars.
        assert_eq!(selected, "SSSSSSSS\nSSSS");
    }

    #[test]
    fn test_selection_extraction_no_borders() {
        let mut app = TuiApp::default();
        // 20 wide terminal
        // |LLLLLLLL||SSSSSSSS|
        app.screen_buffer = vec![
            "|LLLLLLLL||SSSSSSSS|".to_string(),
            "|LLLLLLLL||SSSSSSSS|".to_string(),
        ];
        
        let status_panel = Rect { x: 10, y: 0, width: 10, height: 2 };
        app.selection_panel = Some(status_panel);
        app.selection_is_log = false;
        
        // Select including the borders: from (0, 10) to (1, 19)
        app.selection_anchor = Some((0, 10));
        app.selection_end = Some((1, 19));
        
        let selected = app.get_selected_text().unwrap();
        // Should exclude borders at indices 10 and 19.
        // Inner range is 11..19.
        assert_eq!(selected, "SSSSSSSS\nSSSSSSSS");
    }

    #[test]
    fn test_log_selection_zones() {
        let mut app = TuiApp::default();
        app.logs = vec![LogLine {
            timestamp: "2024-01-01 12:00:00".to_string(),
            message: "Hello World".to_string(),
        }];
        // metadata_width = 20 (brackets + timestamp + space)
        // [2024-01-01 12:00:00] 
        
        app.log_area = Rect { x: 0, y: 0, width: 50, height: 10 };
        
        // 1. Select in Metadata zone (first 5 chars of timestamp)
        app.start_selection(1, 1); // column 1, row 1 (inside log area, account for border)
        assert_eq!(app.selection_zone, LogSelectionZone::Metadata);
        app.update_selection(6, 1);
        let text = app.get_selected_text().unwrap();
        assert_eq!(text, "[2024");
        
        // 2. Select in Message zone
        app.clear_selection();
        // click at column 23. Metadata width is 22.
        // 23 - 1 (border) = 22. 22 - 22 = 0.
        app.start_selection(23, 1); 
        assert_eq!(app.selection_zone, LogSelectionZone::Message);
        app.update_selection(28, 1); // 28 - 1 - 22 = 5. index 0..5 is "Hello"
        let text = app.get_selected_text().unwrap();
        assert_eq!(text, "Hello");
    }

    #[test]
    fn test_selection_with_horizontal_scroll() {
        let mut app = TuiApp::default();
        app.logs = vec![LogLine {
            timestamp: "2024-01-01".to_string(),
            message: "Very long message that is scrolled".to_string(),
        }];
        // metadata_width = 13 (brackets + timestamp + space)
        app.log_area = Rect { x: 0, y: 0, width: 20, height: 10 };
        
        // Scroll right by 10
        app.last_effective_scroll_x = 10;
        
        // Click at screen column 4. 
        // Data column should be 4 - 1 (border) + 10 (scroll) = 13.
        // Data column 13 is the start of Message zone (13 - 13 = 0).
        app.start_selection(4, 1);
        assert_eq!(app.selection_zone, LogSelectionZone::Message);
        assert_eq!(app.selection_anchor, Some((0, 0))); // message index 0
    }

    #[test]
    fn test_autoscroll_stickiness() {
        let mut app = TuiApp::default();
        app.log_area = Rect { x: 0, y: 0, width: 50, height: 5 }; // 3 visible lines
        
        // Add 10 logs
        for i in 0..10 {
            app.logs.push(LogLine { timestamp: "".to_string(), message: i.to_string() });
        }
        
        assert!(app.autoscroll);
        let (sy, _) = app.calculate_effective_scroll();
        assert_eq!(sy, 7); // max_scroll = 10 - 3 = 7
        
        // User scrolls up (one step up from max_scroll)
        app.scroll = 6;
        app.autoscroll = false;
        
        // New logs arrive
        app.logs.push(LogLine { timestamp: "".to_string(), message: "new".to_string() });
        let (sy_new, _) = app.calculate_effective_scroll();
        assert_eq!(sy_new, 6); // Still at user's scroll position
        assert!(!app.autoscroll);
        
        // User scrolls back to bottom
        app.scroll_to_bottom();
        assert!(app.autoscroll);
    }

    #[test]
    fn test_empty_apps_navigation() {
        let mut app = TuiApp::default();
        assert!(app.apps.is_empty());
        
        // Should not crash
        app.next_app();
        app.prev_app();
        app.next_service();
        app.prev_service();
        
        assert_eq!(app.selected_app, 0);
        assert_eq!(app.selected_service, 0);
    }

    #[test]
    fn test_tab_click_precision() {
        let mut app = TuiApp::default();
        app.apps = vec![
            AppSnapshot { app_name: "A".to_string(), services: vec![] },
            AppSnapshot { app_name: "B".to_string(), services: vec![] },
        ];
        app.app_tab_area = Rect { x: 0, y: 0, width: 50, height: 3 };
        
        // Set selected to 1, then click on tab 0 ("A")
        app.selected_app = 1;
        assert!(app.click_app_tab(1, 1));
        assert_eq!(app.selected_app, 0);
        
        // Click on tab 1 ("B")
        assert!(app.click_app_tab(5, 1));
        assert_eq!(app.selected_app, 1);
        
        // Click in between (rel_x 3)
        // Tab A (0,1,2), Gap (3), Tab B (4,5,6)
        // rel_x = col - 1. So col 4 is rel_x 3.
        let prev = app.selected_app;
        assert!(!app.click_app_tab(4, 1));
        assert_eq!(app.selected_app, prev);
    }

    #[test]
    fn test_selection_extraction_single_line() {
        let mut app = TuiApp::default();
        app.screen_buffer = vec!["|0123456789|".to_string()];
        let panel = Rect { x: 0, y: 0, width: 12, height: 1 };
        app.selection_panel = Some(panel);
        app.selection_is_log = false;
        
        // Select "234" (indices 3..6)
        app.selection_anchor = Some((0, 3));
        app.selection_end = Some((0, 6));
        
        let selected = app.get_selected_text().unwrap();
        assert_eq!(selected, "234");
    }

    #[test]
    fn test_selection_extraction_bottom_up() {
        let mut app = TuiApp::default();
        app.screen_buffer = vec![
            "|LINE_0____|".to_string(),
            "|LINE_1____|".to_string(),
        ];
        let panel = Rect { x: 0, y: 0, width: 12, height: 2 };
        app.selection_panel = Some(panel);
        app.selection_is_log = false;
        
        // Select from bottom-right (1, 10) to top-left (0, 1)
        app.selection_anchor = Some((1, 10));
        app.selection_end = Some((0, 1));
        
        let selected = app.get_selected_text().unwrap();
        // Normalized range: (0, 1, 1, 10)
        // Line 0: index 1 to 11 -> "LINE_0____"
        // Line 1: index 1 to 10 -> "LINE_1___"
        assert_eq!(selected, "LINE_0____\nLINE_1___");
    }

    #[test]
    fn test_selection_extraction_out_of_bounds_clamping() {
        let mut app = TuiApp::default();
        app.screen_buffer = vec!["|CONTENT|".to_string()];
        let panel = Rect { x: 0, y: 0, width: 9, height: 1 };
        app.selection_panel = Some(panel);
        app.selection_is_log = false;
        
        // Select from index 0 (border) to 100 (way past terminal)
        app.selection_anchor = Some((0, 0));
        app.selection_end = Some((0, 100));
        
        let selected = app.get_selected_text().unwrap();
        // Inner range is 1..8. 
        // sc=0 -> clamped to 1. ec=100 -> clamped to 8.
        // index 1 to 8 is "CONTENT"
        assert_eq!(selected, "CONTENT");
    }
}
