use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::logs::{LogEntry, LogStream};
use crate::metrics::ServiceMetrics;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServiceSelector {
    All,
    Service(String),
    Services(Vec<String>),
    Profile(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Request {
    Up {
        file: PathBuf,
        profile: Option<String>,
    },
    Start {
        file: Option<PathBuf>,
        app: Option<String>,
        selector: ServiceSelector,
    },
    Stop {
        app: Option<String>,
        selector: ServiceSelector,
    },
    Restart {
        app: Option<String>,
        selector: ServiceSelector,
    },
    Status {
        app: Option<String>,
        selector: ServiceSelector,
    },
    Logs {
        app: Option<String>,
        selector: ServiceSelector,
        follow: bool,
        tail: Option<usize>,
        merged: bool,
    },
    Profiles {
        app: Option<String>,
    },
    Down {
        app: Option<String>,
    },
    DashAttach,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Response {
    Ack,
    Error(String),
    StatusSnapshot(StatusSnapshot),
    ProfilesList(Vec<String>),
    LogChunk(LogChunk),
    DaemonShutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusSnapshot {
    pub apps: Vec<AppSnapshot>,
    #[serde(default)]
    pub system_cpu: f32,
    #[serde(default)]
    pub system_memory_used: u64,
    #[serde(default)]
    pub system_memory_total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSnapshot {
    pub app_name: String,
    pub services: Vec<ServiceSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceSnapshot {
    pub name: String,
    pub status: String,
    pub pid: Option<u32>,
    pub uptime_secs: Option<u64>,
    pub exit_code: Option<i32>,
    pub metrics: ServiceMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogChunk {
    pub app: String,
    pub service: String,
    pub entry: LogEntry,
}

pub fn format_log_entry(entry: &LogEntry, merged: bool, service: &str) -> String {
    let prefix = match entry.stream {
        LogStream::Stdout => "stdout",
        LogStream::Stderr => "stderr",
    };
    
    let time = chrono::DateTime::from_timestamp(entry.timestamp as i64, 0)
        .map(|dt| dt.with_timezone(&chrono::Local))
        .unwrap_or_default();
    let time_str = time.format("%Y-%m-%d %H:%M:%S");

    if merged {
        format!("[{}] [{}] {}", time_str, service, entry.line)
    } else {
        format!("[{}] [{}:{}] {}", time_str, service, prefix, entry.line)
    }
}
