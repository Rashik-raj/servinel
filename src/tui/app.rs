use crate::ipc::protocol::{AppSnapshot, ServiceSnapshot};

#[derive(Debug)]
pub struct TuiApp {
    pub apps: Vec<AppSnapshot>,
    pub selected_app: usize,
    pub selected_service: usize,
    pub logs: Vec<String>,
    pub system_cpu: f32,
    pub system_memory_used: u64,
    pub system_memory_total: u64,
    pub scroll: usize,
    pub scroll_x: u16,
    pub autoscroll: bool,
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
            // Best guess: start from bottom. 
            // We don't know window height here so we might jump a bit. 
            // Ideally UI handles this but this is a reasonable default.
            self.scroll = self.logs.len().saturating_sub(1);
        } else if self.scroll > 0 {
            self.scroll -= 1;
        }
    }

    pub fn scroll_down(&mut self) {
        if !self.autoscroll {
            self.scroll += 1;
            // If we scroll past end, enable autoscroll? 
            // Hard to detect without height, but reasonable bounds check:
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
        // Approximate page size
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

    fn reset_scroll(&mut self) {
        self.scroll = 0;
        self.autoscroll = true;
    }
}
