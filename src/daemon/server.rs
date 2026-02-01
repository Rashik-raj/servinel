use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::UnixListener;
use tokio::sync::RwLock;

use crate::compose::{load_compose, ComposeFile};
use crate::daemon::state::{uptime_seconds, DaemonState};
use crate::daemon::supervisor::Supervisor;
use crate::error::{Result, ServinelError};
use crate::ipc::protocol::{
    AppSnapshot, LogChunk, ServiceSelector, ServiceSnapshot, StatusSnapshot,
};
use crate::logs::LogEntry;
use crate::util::{ensure_app_dir, socket_path};

pub struct Daemon {
    state: Arc<RwLock<DaemonState>>,
    supervisor: Supervisor,
}

impl Daemon {
    pub fn new() -> Self {
        let state = Arc::new(RwLock::new(DaemonState::default()));
        let supervisor = Supervisor::new(state.clone());
        Self { state, supervisor }
    }

    pub async fn up(&self, file: PathBuf, profile: Option<String>) -> Result<()> {
        tracing::info!(?file, ?profile, "daemon: up start");
        let compose = load_compose(&file)?;
        let app_name = compose.app_name.clone();
        self.register_app(compose, file).await;
        let selector = profile
            .map(ServiceSelector::Profile)
            .unwrap_or(ServiceSelector::All);
        let services = self.resolve_services(&app_name, &selector).await?;
        for service in services {
            self.supervisor.start_service(&app_name, &service).await?;
        }
        tracing::info!(?app_name, "daemon: up done");
        Ok(())
    }

    pub async fn start(
        &self,
        file: Option<PathBuf>,
        app: Option<String>,
        selector: ServiceSelector,
    ) -> Result<()> {
        let app_name = if let Some(file) = file {
            tracing::info!(?file, "daemon: start with file");
            let compose = load_compose(&file)?;
            let app_name = compose.app_name.clone();
            self.register_app(compose, file).await;
            app_name
        } else {
            self.resolve_app(app).await?
        };
        let services = self.resolve_services(&app_name, &selector).await?;
        for service in services {
            self.supervisor.start_service(&app_name, &service).await?;
        }
        Ok(())
    }

    pub async fn stop(&self, app: Option<String>, selector: ServiceSelector) -> Result<()> {
        let app_name = self.resolve_app(app).await?;
        let services = self.resolve_services(&app_name, &selector).await?;
        for service in services {
            self.supervisor.stop_service(&app_name, &service).await?;
        }
        Ok(())
    }

    pub async fn restart(&self, app: Option<String>, selector: ServiceSelector) -> Result<()> {
        let app_name = self.resolve_app(app).await?;
        let services = self.resolve_services(&app_name, &selector).await?;
        for service in services.iter() {
            self.supervisor.stop_service(&app_name, service).await?;
        }
        for service in services {
            self.supervisor.start_service(&app_name, &service).await?;
        }
        Ok(())
    }

    pub async fn status(&self, app: Option<String>, selector: ServiceSelector) -> Result<StatusSnapshot> {
        let mut apps = Vec::new();

        if app.is_none() {
            if !matches!(selector, ServiceSelector::All) {
                return Err(ServinelError::Usage(
                    "--app is required for profiles or specific services".to_string(),
                ));
            }
            let state = self.state.read().await;
            for app_state in state.apps.values() {
                apps.push(build_snapshot(app_state, app_state.services.keys().cloned().collect()));
            }
            return Ok(StatusSnapshot {
                apps,
                system_cpu: state.system_cpu,
                system_memory_used: state.system_memory_used,
                system_memory_total: state.system_memory_total,
            });
        }

        let app_name = app.unwrap();
        let services = self.resolve_services(&app_name, &selector).await?;
        let state = self.state.read().await;
        let app_state = state
            .apps
            .get(&app_name)
            .ok_or_else(|| ServinelError::AppNotFound(app_name.clone()))?;
        apps.push(build_snapshot(app_state, services));
        Ok(StatusSnapshot {
            apps,
            system_cpu: state.system_cpu,
            system_memory_used: state.system_memory_used,
            system_memory_total: state.system_memory_total,
        })
    }

    pub async fn profiles(&self, app: Option<String>) -> Result<Vec<String>> {
        let app_name = self.resolve_app(app).await?;
        let state = self.state.read().await;
        let app_state = state
            .apps
            .get(&app_name)
            .ok_or_else(|| ServinelError::AppNotFound(app_name.clone()))?;
        let mut profiles: Vec<String> = app_state.profiles.keys().cloned().collect();
        profiles.sort();
        Ok(profiles)
    }

    pub async fn logs(
        &self,
        app: Option<String>,
        selector: ServiceSelector,
        tail: Option<usize>,
    ) -> Result<(Vec<LogChunk>, Vec<LogSubscription>)> {
        let app_name = self.resolve_app(app).await?;
        let services = self.resolve_services(&app_name, &selector).await?;
        
        // 1. Collect historical logs while holding state lock
        let mut chunks = Vec::new();
        {
            let state = self.state.read().await;
            let app_state = state
                .apps
                .get(&app_name)
                .ok_or_else(|| ServinelError::AppNotFound(app_name.clone()))?;

            for service in &services {
                if let Some(service_state) = app_state.services.get(service) {
                    let entries = match tail {
                        Some(count) => service_state.logs.tail(count),
                        None => service_state.logs.all(),
                    };
                    for entry in entries {
                        chunks.push(LogChunk {
                            app: app_name.clone(),
                            service: service.clone(),
                            entry,
                        });
                    }
                }
            }
        }

        // 2. Collect log senders without holding state lock
        let mut subs = Vec::new();
        for service in services {
            if let Some(sender) = self.supervisor.log_sender(&app_name, &service).await {
                subs.push(LogSubscription {
                    app: app_name.clone(),
                    service: service.clone(),
                    receiver: sender.subscribe(),
                });
            }
        }

        Ok((chunks, subs))
    }

    pub async fn register_app(&self, compose: ComposeFile, path: PathBuf) {
        let mut state = self.state.write().await;
        state.insert_app(compose, path);
    }

    pub async fn resolve_app(&self, app: Option<String>) -> Result<String> {
        if let Some(app) = app {
            return Ok(app);
        }

        let state = self.state.read().await;
        let apps = state.list_apps();
        if apps.len() == 1 {
            Ok(apps[0].clone())
        } else {
            Err(ServinelError::Usage(
                "Multiple apps running, use --app".to_string(),
            ))
        }
    }

    pub async fn resolve_services(
        &self,
        app: &str,
        selector: &ServiceSelector,
    ) -> Result<Vec<String>> {
        let state = self.state.read().await;
        let app_state = state
            .apps
            .get(app)
            .ok_or_else(|| ServinelError::AppNotFound(app.to_string()))?;

        let services = match selector {
            ServiceSelector::All => app_state.services.keys().cloned().collect(),
            ServiceSelector::Service(name) => vec![name.clone()],
            ServiceSelector::Services(names) => names.clone(),
            ServiceSelector::Profile(profile) => app_state
                .profiles
                .get(profile)
                .ok_or_else(|| ServinelError::ProfileNotFound(profile.clone()))?
                .clone(),
        };

        let known: HashSet<_> = app_state.services.keys().cloned().collect();
        for name in &services {
            if !known.contains(name) {
                return Err(ServinelError::ServiceNotFound(name.clone()));
            }
        }

        Ok(services)
    }

    pub async fn tick_loop(&self) {
        let mut interval = tokio::time::interval(Duration::from_millis(800));
        loop {
            interval.tick().await;
            let _ = self.supervisor.refresh().await;
        }
    }
}

pub struct LogSubscription {
    pub app: String,
    pub service: String,
    pub receiver: tokio::sync::broadcast::Receiver<LogEntry>,
}

fn build_snapshot(app_state: &crate::daemon::state::AppState, services: Vec<String>) -> AppSnapshot {
    let mut service_snapshots = Vec::new();
    for name in services {
        if let Some(service) = app_state.services.get(&name) {
            service_snapshots.push(ServiceSnapshot {
                name: service.config.name.clone(),
                status: service.status.as_str().to_string(),
                pid: service.pid,
                uptime_secs: uptime_seconds(service.started_at),
                exit_code: service.exit_code,
                metrics: service.metrics.clone(),
            });
        }
    }
    AppSnapshot {
        app_name: app_state.app_name.clone(),
        services: service_snapshots,
    }
}

pub async fn run_daemon() -> Result<()> {
    ensure_app_dir()?;
    let socket = socket_path()?;
    if socket.exists() {
        let _ = std::fs::remove_file(&socket);
    }
    let listener = UnixListener::bind(socket)?;
    let daemon = Arc::new(Daemon::new());
    let daemon_clone = daemon.clone();
    tokio::spawn(async move {
        daemon_clone.tick_loop().await;
    });
    crate::ipc::server::serve(listener, daemon).await
}
