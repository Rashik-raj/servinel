use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{Result, ServinelError};

#[derive(Debug, Clone, Deserialize)]
pub struct ComposeFile {
    pub app_name: String,
    pub services: Vec<ServiceConfig>,
    #[serde(default)]
    pub profiles: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    pub command: String,
    pub working_directory: Option<PathBuf>,
    #[serde(default)]
    #[allow(dead_code)]
    pub restart: Option<String>,
}

pub fn load_compose(path: &Path) -> Result<ComposeFile> {
    let content = std::fs::read_to_string(path)?;
    let mut compose: ComposeFile = serde_yaml::from_str(&content)?;
    normalize_compose(&mut compose, path)?;
    validate_compose(&compose)?;
    Ok(compose)
}

fn normalize_compose(compose: &mut ComposeFile, path: &Path) -> Result<()> {
    let base_dir = path
        .parent()
        .ok_or_else(|| ServinelError::InvalidCompose("Invalid compose path".to_string()))?;

    for service in &mut compose.services {
        if let Some(dir) = &service.working_directory {
            if dir.is_relative() {
                service.working_directory = Some(base_dir.join(dir));
            }
        }
    }
    Ok(())
}

fn validate_compose(compose: &ComposeFile) -> Result<()> {
    if compose.app_name.trim().is_empty() {
        return Err(ServinelError::InvalidCompose(
            "app_name is required".to_string(),
        ));
    }

    let mut names = HashSet::new();
    for service in &compose.services {
        if service.name.trim().is_empty() {
            return Err(ServinelError::InvalidCompose(
                "service name cannot be empty".to_string(),
            ));
        }
        if !names.insert(service.name.clone()) {
            return Err(ServinelError::InvalidCompose(format!(
                "duplicate service name: {}",
                service.name
            )));
        }
    }

    let service_names: HashSet<_> = compose
        .services
        .iter()
        .map(|svc| svc.name.as_str())
        .collect();
    for (profile, services) in &compose.profiles {
        for svc in services {
            if !service_names.contains(svc.as_str()) {
                return Err(ServinelError::InvalidCompose(format!(
                    "profile '{}' references unknown service '{}'",
                    profile, svc
                )));
            }
        }
    }

    Ok(())
}
