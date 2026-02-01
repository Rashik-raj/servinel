use std::path::{Path, PathBuf};

use crate::error::{Result, ServinelError};

pub const DEFAULT_COMPOSE_FILE: &str = "servinel-compose.yaml";

pub fn find_compose_file() -> Result<Option<PathBuf>> {
    let cwd = std::env::current_dir()?;
    let candidate = cwd.join(DEFAULT_COMPOSE_FILE);
    if candidate.exists() {
        Ok(Some(candidate))
    } else {
        Ok(None)
    }
}

pub fn require_compose_file(path: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = path {
        return Ok(normalize_path(path)?);
    }

    match find_compose_file()? {
        Some(path) => Ok(path),
        None => Err(ServinelError::ComposeNotFound(PathBuf::from(
            DEFAULT_COMPOSE_FILE,
        ))),
    }
}

pub fn normalize_path(path: PathBuf) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

pub fn app_data_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::NotFound, "HOME not set"))?;
    Ok(Path::new(&home).join(".servinel"))
}

pub fn socket_path() -> Result<PathBuf> {
    Ok(app_data_dir()?.join("servinel.sock"))
}

pub fn ensure_app_dir() -> Result<PathBuf> {
    let path = app_data_dir()?;
    if !path.exists() {
        std::fs::create_dir_all(&path)?;
    }
    Ok(path)
}
