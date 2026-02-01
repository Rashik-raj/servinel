use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, ServinelError>;

#[derive(Error, Debug)]
pub enum ServinelError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Compose file not found: {0}")]
    ComposeNotFound(PathBuf),
    #[error("Invalid compose file: {0}")]
    InvalidCompose(String),
    #[error("App not found: {0}")]
    AppNotFound(String),
    #[error("Service not found: {0}")]
    ServiceNotFound(String),
    #[error("Profile not found: {0}")]
    ProfileNotFound(String),
    #[error("Daemon is not running")]
    DaemonNotRunning,
    #[error("CLI usage error: {0}")]
    Usage(String),
}
