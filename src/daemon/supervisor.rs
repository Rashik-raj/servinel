use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::SystemTime;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;
use tokio::sync::{broadcast, Mutex, RwLock};

use crate::error::{Result, ServinelError};
use crate::logs::{LogEntry, LogStream};
use crate::metrics::ServiceMetrics;
use crate::daemon::state::{DaemonState, ServiceStatus};

type ServiceKey = (String, String);

struct ServiceRuntime {
    child: Child,
    log_tx: broadcast::Sender<LogEntry>,
}

#[derive(Clone)]
pub struct Supervisor {
    state: Arc<RwLock<DaemonState>>,
    runtimes: Arc<Mutex<HashMap<ServiceKey, ServiceRuntime>>>,
    system: Arc<Mutex<sysinfo::System>>,
}

impl Supervisor {
    pub fn new(state: Arc<RwLock<DaemonState>>) -> Self {
        Self {
            state,
            runtimes: Arc::new(Mutex::new(HashMap::new())),
            system: Arc::new(Mutex::new(sysinfo::System::new())),
        }
    }

    pub async fn start_service(&self, app: &str, service: &str) -> Result<()> {
        let (command, workdir, pid) = {
            let state = self.state.read().await;
            let app_state = state
                .apps
                .get(app)
                .ok_or_else(|| ServinelError::AppNotFound(app.to_string()))?;
            let svc_state = app_state
                .services
                .get(service)
                .ok_or_else(|| ServinelError::ServiceNotFound(service.to_string()))?;
            let base_dir = app_state
                .compose_path
                .parent()
                .map(|dir| dir.to_path_buf())
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
            let workdir = svc_state
                .config
                .working_directory
                .clone()
                .unwrap_or(base_dir);
            (svc_state.config.command.clone(), workdir, svc_state.pid)
        };

        if let Some(p) = pid {
            // Try to kill any existing process group before starting
            unsafe {
                libc::kill(-(p as i32), libc::SIGKILL);
            }
        }

        let mut runtimes = self.runtimes.lock().await;
        if runtimes.contains_key(&(app.to_string(), service.to_string())) {
            return Ok(());
        }

        let final_command = if command.trim().starts_with("exec ") {
            format!("cd {} && {}", workdir.display(), command)
        } else {
            format!("cd {} && exec {}", workdir.display(), command)
        };
        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c")
            .arg(final_command)
            .current_dir(workdir)
            .process_group(0) // Start in a new process group
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn()?;
        let pid = child.id();
        let (log_tx, _) = broadcast::channel(1024);

        if let Some(stdout) = child.stdout.take() {
            self.spawn_log_task(app, service, LogStream::Stdout, stdout, log_tx.clone());
        }
        if let Some(stderr) = child.stderr.take() {
            self.spawn_log_task(app, service, LogStream::Stderr, stderr, log_tx.clone());
        }

        runtimes.insert(
            (app.to_string(), service.to_string()),
            ServiceRuntime { child, log_tx },
        );

        let mut state = self.state.write().await;
        state.update_service_status(app, service, ServiceStatus::Running);
        state.set_service_pid(app, service, pid);
        state.set_service_start_time(app, service, Some(SystemTime::now()));
        state.set_exit_code(app, service, None);
        Ok(())
    }

    pub async fn stop_service(&self, app: &str, service: &str) -> Result<()> {
        let pid = {
            let state = self.state.read().await;
            state.apps.get(app)
                .and_then(|a| a.services.get(service))
                .and_then(|s| s.pid)
        };

        if let Some(p) = pid {
            // Always try to kill the process group first to ensure all descendants are gone
            unsafe {
                libc::kill(-(p as i32), libc::SIGKILL);
            }
        }

        let mut runtimes = self.runtimes.lock().await;
        if let Some(mut runtime) = runtimes.remove(&(app.to_string(), service.to_string())) {
            tokio::spawn(async move {
                let _ = runtime.child.wait().await;
            });
        }

        let mut state = self.state.write().await;
        state.update_service_status(app, service, ServiceStatus::Stopped);
        state.set_service_pid(app, service, None);
        state.set_service_start_time(app, service, None);
        state.set_exit_code(app, service, None);
        Ok(())
    }

    pub async fn refresh(&self) -> Result<()> {
        let mut updates = Vec::new();
        let system_metrics;
        
        {
            let mut runtimes = self.runtimes.lock().await;
            let mut system = self.system.lock().await;
            system.refresh_cpu_all();
            system.refresh_memory();
            system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
            
            let system_cpu = system.global_cpu_usage();
            let system_used = system.used_memory();
            let system_total = system.total_memory();
            system_metrics = (system_cpu, system_used, system_total);

            let mut to_remove = Vec::new();

            for ((app, service), runtime) in runtimes.iter_mut() {
                if let Some(status) = runtime.child.try_wait()? {
                    let exit_code = status.code();
                    updates.push(RefreshUpdate::Exited {
                        app: app.clone(),
                        service: service.clone(),
                        exit_code,
                    });
                    to_remove.push((app.clone(), service.clone()));
                    continue;
                }

                if let Some(pid) = runtime.child.id() {
                    if let Some(proc) = system.process(sysinfo::Pid::from_u32(pid)) {
                        let metrics = ServiceMetrics {
                            cpu: proc.cpu_usage(),
                            memory: proc.memory(),
                            memory_total: system_total,
                        };
                        updates.push(RefreshUpdate::Metrics {
                            app: app.clone(),
                            service: service.clone(),
                            metrics,
                        });
                    }
                }
            }

            for key in to_remove {
                runtimes.remove(&key);
            }
        }

        // Apply updates to state
        let mut state = self.state.write().await;
        state.set_system_metrics(system_metrics.0, system_metrics.1, system_metrics.2);
        for update in &updates {
            match update {
                RefreshUpdate::Exited { app, service, exit_code } => {
                    state.update_service_status(&app, &service, ServiceStatus::Exited);
                    state.set_service_pid(&app, &service, None);
                    state.set_service_start_time(&app, &service, None);
                    state.set_exit_code(&app, &service, *exit_code);
                }
                RefreshUpdate::Metrics { app, service, metrics } => {
                    state.set_metrics(&app, &service, metrics.clone());
                }
            }
        }

        if !updates.is_empty() {
            let _ = state.save();
        }

        Ok(())
    }

    pub async fn log_sender(&self, app: &str, service: &str) -> Option<broadcast::Sender<LogEntry>> {
        let runtimes = self.runtimes.lock().await;
        runtimes
            .get(&(app.to_string(), service.to_string()))
            .map(|runtime| runtime.log_tx.clone())
    }

    fn spawn_log_task(
        &self,
        app: &str,
        service: &str,
        stream: LogStream,
        reader: impl tokio::io::AsyncRead + Unpin + Send + 'static,
        log_tx: broadcast::Sender<LogEntry>,
    ) {
        let app = app.to_string();
        let service = service.to_string();
        let state = self.state.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(reader).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let entry = LogEntry {
                    timestamp: current_timestamp(),
                    stream,
                    line,
                    };
                let mut state = state.write().await;
                state.push_log(&app, &service, entry.clone());
                let _ = log_tx.send(entry);
                }
            });
        }
}

enum RefreshUpdate {
    Exited {
        app: String,
        service: String,
        exit_code: Option<i32>,
    },
    Metrics {
        app: String,
        service: String,
        metrics: ServiceMetrics,
    },
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}
