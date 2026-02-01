use std::collections::HashMap;
use std::time::SystemTime;

use crate::compose::{ComposeFile, ServiceConfig};
use crate::logs::{LogBuffer, LogEntry};
use crate::metrics::ServiceMetrics;

const LOG_BUFFER_CAPACITY: usize = 1000;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ServiceStatus {
    Starting,
    Running,
    Stopped,
    Unhealthy,
    Exited,
}

impl ServiceStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ServiceStatus::Starting => "starting",
            ServiceStatus::Running => "running",
            ServiceStatus::Stopped => "stopped",
            ServiceStatus::Unhealthy => "unhealthy",
            ServiceStatus::Exited => "exited",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ServiceState {
    pub config: ServiceConfig,
    pub status: ServiceStatus,
    pub pid: Option<u32>,
    pub started_at: Option<SystemTime>,
    pub exit_code: Option<i32>,
    pub logs: LogBuffer,
    pub metrics: ServiceMetrics,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub app_name: String,
    pub compose_path: std::path::PathBuf,
    pub profiles: HashMap<String, Vec<String>>,
    pub services: HashMap<String, ServiceState>,
}

#[derive(Debug, Default)]
pub struct DaemonState {
    pub apps: HashMap<String, AppState>,
    pub system_cpu: f32,
    pub system_memory_used: u64,
    pub system_memory_total: u64,
}

impl DaemonState {
    pub fn insert_app(&mut self, compose: ComposeFile, compose_path: std::path::PathBuf) {
        let services = compose
            .services
            .into_iter()
            .map(|svc| {
                let state = ServiceState {
                    status: ServiceStatus::Stopped,
                    pid: None,
                    started_at: None,
                    exit_code: None,
                    logs: LogBuffer::new(LOG_BUFFER_CAPACITY),
                    metrics: ServiceMetrics::default(),
                    config: svc.clone(),
                };
                (svc.name.clone(), state)
            })
            .collect();

        let app = AppState {
            app_name: compose.app_name.clone(),
            compose_path,
            profiles: compose.profiles.clone(),
            services,
        };

        self.apps.insert(compose.app_name, app);
    }

    pub fn list_apps(&self) -> Vec<String> {
        self.apps.keys().cloned().collect()
    }

    pub fn update_service_status(&mut self, app: &str, service: &str, status: ServiceStatus) {
        if let Some(app_state) = self.apps.get_mut(app) {
            if let Some(service_state) = app_state.services.get_mut(service) {
                service_state.status = status;
            }
        }
    }

    pub fn set_service_pid(&mut self, app: &str, service: &str, pid: Option<u32>) {
        if let Some(app_state) = self.apps.get_mut(app) {
            if let Some(service_state) = app_state.services.get_mut(service) {
                service_state.pid = pid;
            }
        }
    }

    pub fn set_service_start_time(&mut self, app: &str, service: &str, time: Option<SystemTime>) {
        if let Some(app_state) = self.apps.get_mut(app) {
            if let Some(service_state) = app_state.services.get_mut(service) {
                service_state.started_at = time;
            }
        }
    }

    pub fn set_exit_code(&mut self, app: &str, service: &str, code: Option<i32>) {
        if let Some(app_state) = self.apps.get_mut(app) {
            if let Some(service_state) = app_state.services.get_mut(service) {
                service_state.exit_code = code;
            }
        }
    }

    pub fn push_log(&mut self, app: &str, service: &str, entry: LogEntry) {
        if let Some(app_state) = self.apps.get_mut(app) {
            if let Some(service_state) = app_state.services.get_mut(service) {
                service_state.logs.push(entry);
            }
        }
    }

    pub fn set_metrics(&mut self, app: &str, service: &str, metrics: ServiceMetrics) {
        if let Some(app_state) = self.apps.get_mut(app) {
            if let Some(service_state) = app_state.services.get_mut(service) {
                service_state.metrics = metrics;
            }
        }
    }

    pub fn set_system_metrics(&mut self, cpu: f32, used: u64, total: u64) {
        self.system_cpu = cpu;
        self.system_memory_used = used;
        self.system_memory_total = total;
    }
}

pub fn uptime_seconds(started_at: Option<SystemTime>) -> Option<u64> {
    started_at
        .and_then(|start| start.elapsed().ok())
        .map(|duration| duration.as_secs())
}
