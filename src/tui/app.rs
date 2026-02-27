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
    pub service: String,
    pub message: String,
}

impl LogLine {
    pub fn metadata_width(&self) -> usize {
        // [YYYY-MM-DD HH:MM:SS] [service] 
        // 2 (brackets) + timestamp.len() + 2 (space+bracket) + service.len() + 2 (bracket+space)
        self.timestamp.len() + self.service.len() + 6
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
    pub scroll_x: u16,
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
            scroll_x: 0,
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
        if self.scroll_x > 0 {
            self.scroll_x = self.scroll_x.saturating_sub(5);
        }
    }

    pub fn scroll_right(&mut self) {
        self.scroll_x = self.scroll_x.saturating_add(5);
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
        (sy, self.scroll_x)
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
                        format!("[{}] [{}] ", line.timestamp, line.service).chars().collect()
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
        }

        let result = lines.join("\n");
        if result.trim().is_empty() { None } else { Some(result) }
    }
}
