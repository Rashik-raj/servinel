use crate::ipc::protocol::{AppSnapshot, ServiceSnapshot};

#[derive(Debug, Default)]
pub struct TuiApp {
    pub apps: Vec<AppSnapshot>,
    pub selected_app: usize,
    pub selected_service: usize,
    pub logs: Vec<String>,
    pub system_cpu: f32,
    pub system_memory_used: u64,
    pub system_memory_total: u64,
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
        }
    }

    pub fn next_service(&mut self) {
        if let Some(app) = self.apps.get(self.selected_app) {
            if !app.services.is_empty() {
                self.selected_service = (self.selected_service + 1) % app.services.len();
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
}
