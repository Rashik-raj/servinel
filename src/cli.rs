use std::path::PathBuf;

use clap::{Parser, Subcommand, CommandFactory};

use crate::compose::load_compose;
use crate::error::{Result, ServinelError};
use crate::ipc::client::{ensure_daemon, request_response, stream_logs};
use crate::ipc::protocol::{
    format_log_entry, Request, Response, ServiceSelector,
};
use crate::tui;
use crate::util::{find_compose_file, require_compose_file, socket_path};

#[derive(Parser)]
#[command(name = "servinel", version, about = "Service orchestrator with TUI")]
pub struct Cli {
    #[arg(long)]
    pub verbose: bool,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Up {
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        no_tui: bool,
    },
    Start {
        service: Option<String>,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long)]
        no_tui: bool,
    },
    Stop {
        service: Option<String>,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        app: Option<String>,
    },
    Restart {
        service: Option<String>,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        app: Option<String>,
        #[arg(long)]
        no_tui: bool,
    },
    Status {
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        app: Option<String>,
    },
    Logs {
        service: Option<String>,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        app: Option<String>,
        #[arg(long)]
        follow: bool,
        #[arg(long)]
        tail: Option<usize>,
        #[arg(long)]
        merged: bool,
    },
    Profiles {
        #[arg(long)]
        app: Option<String>,
    },
    Dash,
    Doctor,
    #[command(hide = true)]
    DaemonClear,
    #[command(hide = true)]
    Daemon,
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

pub async fn execute(cli: Cli) -> Result<()> {
    if cli.verbose {
        unsafe {
            std::env::set_var("SERVINEL_VERBOSE_DAEMON", "1");
        }
    }
    match cli.command {
        Commands::Daemon => {
            crate::daemon::run_daemon().await?;
        }
        Commands::Up {
            file,
            profile,
            no_tui,
        } => {
            ensure_daemon().await?;
            let file = require_compose_file(file)?;
            let request = Request::Up { file, profile };
            handle_simple(request).await?;
            if !no_tui {
                launch_tui().await?;
            }
        }
        Commands::Start {
            service,
            profile,
            file,
            no_tui,
        } => {
            ensure_daemon().await?;
            let file = require_compose_file(file)?;
            let selector = selector_from_options(service, profile, false)?;
            let request = Request::Start {
                file: Some(file),
                app: None,
                selector,
            };
            handle_simple(request).await?;
            if !no_tui {
                launch_tui().await?;
            }
        }
        Commands::Stop { service, profile, app } => {
            ensure_daemon().await?;
            let app = resolve_app_name(app).await?;
            let selector = selector_from_options(service, profile, false)?;
            let request = Request::Stop { app: Some(app), selector };
            handle_simple(request).await?;
        }
        Commands::Restart {
            service,
            profile,
            app,
            no_tui,
        } => {
            ensure_daemon().await?;
            let app = resolve_app_name(app).await?;
            let selector = selector_from_options(service, profile, false)?;
            let request = Request::Restart { app: Some(app), selector };
            handle_simple(request).await?;
            if !no_tui {
                launch_tui().await?;
            }
        }
        Commands::Status { profile, app } => {
            ensure_daemon().await?;
            let app = resolve_app_name(app).await?;
            let selector = profile
                .map(ServiceSelector::Profile)
                .unwrap_or(ServiceSelector::All);
            let request = Request::Status { app: Some(app), selector };
            match request_response(&request).await? {
                Response::StatusSnapshot(snapshot) => {
                    print_status(snapshot);
                }
                Response::Error(message) => return Err(ServinelError::Usage(message)),
                _ => {}
            }
        }
        Commands::Logs {
            service,
            profile,
            app,
            follow,
            tail,
            merged,
        } => {
            ensure_daemon().await?;
            let app = resolve_app_name(app).await?;
            let selector = selector_from_options(service, profile, false)?;
            let request = Request::Logs {
                app: Some(app),
                selector,
                follow,
                tail,
                merged,
            };
            stream_logs(&request, |chunk| {
                println!("{}", format_log_entry(&chunk.entry, merged, &chunk.service));
            })
            .await?;
        }
        Commands::Profiles { app } => {
            ensure_daemon().await?;
            let app = resolve_app_name(app).await?;
            let request = Request::Profiles { app: Some(app) };
            match request_response(&request).await? {
                Response::ProfilesList(profiles) => {
                    for profile in profiles {
                        println!("{profile}");
                    }
                }
                Response::Error(message) => return Err(ServinelError::Usage(message)),
                _ => {}
            }
        }
        Commands::Dash => {
            ensure_daemon().await?;
            launch_tui().await?;
        }
        Commands::Doctor => {
            doctor().await?;
        }
        Commands::DaemonClear => {
            daemon_clear()?;
        }
        Commands::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "servinel", &mut std::io::stdout());
        }
    }
    Ok(())
}

async fn handle_simple(request: Request) -> Result<()> {
    let response = request_response(&request).await?;
    match response {
        Response::Ack => Ok(()),
        Response::Error(message) => Err(ServinelError::Usage(message)),
        _ => Ok(()),
    }
}

async fn launch_tui() -> Result<()> {
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    tui::run().await
}

async fn doctor() -> Result<()> {
    let socket = socket_path()?;
    let socket_exists = socket.exists();
    println!("Socket: {}", socket.display());
    println!("Socket exists: {}", socket_exists);

    let mut system = sysinfo::System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let mut daemon_pids = Vec::new();
    for process in system.processes().values() {
        let cmd = process
            .cmd()
            .iter()
            .map(|part| part.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(" ");
        if cmd.contains("servinel") && cmd.contains("daemon") {
            daemon_pids.push(process.pid().as_u32());
        }
    }
    println!("Daemon processes: {}", daemon_pids.len());
    if !daemon_pids.is_empty() {
        println!("Daemon PIDs: {:?}", daemon_pids);
    }

    let ping = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        request_response(&Request::DashAttach),
    )
    .await;
    match ping {
        Ok(Ok(Response::Ack)) => println!("Daemon ping: ok"),
        Ok(Ok(Response::Error(message))) => println!("Daemon ping error: {message}"),
        Ok(Ok(other)) => println!("Daemon ping response: {other:?}"),
        Ok(Err(err)) => println!("Daemon ping failed: {err}"),
        Err(_) => println!("Daemon ping: timeout"),
    }

    // Show running apps/services snapshot if available
    let snapshot = request_response(&Request::Status {
        app: None,
        selector: ServiceSelector::All,
    })
    .await;
    match snapshot {
        Ok(Response::StatusSnapshot(s)) => {
            println!("Apps in daemon: {}", s.apps.len());
            for app in s.apps {
                println!("- {} (services: {})", app.app_name, app.services.len());
                for svc in app.services {
                    println!("    {:<16} {:<8} pid={:?}", svc.name, svc.status, svc.pid);
                }
            }
        }
        Ok(Response::Error(message)) => println!("Status error: {message}"),
        Ok(other) => println!("Status response: {other:?}"),
        Err(err) => println!("Status failed: {err}"),
    }
    Ok(())
}

fn daemon_clear() -> Result<()> {
    let socket = socket_path()?;
    if socket.exists() {
        let _ = std::fs::remove_file(&socket);
        println!("Removed socket: {}", socket.display());
    }

    let mut system = sysinfo::System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let mut killed = Vec::new();
    for process in system.processes().values() {
        let cmd = process
            .cmd()
            .iter()
            .map(|part| part.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(" ");
        if cmd.contains("servinel") && cmd.contains("daemon") {
            let _ = process.kill();
            killed.push(process.pid().as_u32());
        }
    }
    if killed.is_empty() {
        println!("No daemon processes found.");
    } else {
        println!("Killed daemon PIDs: {:?}", killed);
    }
    Ok(())
}

fn selector_from_options(
    service: Option<String>,
    profile: Option<String>,
    allow_all: bool,
) -> Result<ServiceSelector> {
    match (service, profile) {
        (Some(service), None) => Ok(ServiceSelector::Service(service)),
        (None, Some(profile)) => Ok(ServiceSelector::Profile(profile)),
        (None, None) if allow_all => Ok(ServiceSelector::All),
        _ => Err(ServinelError::Usage(
            "Provide either a service or --profile".to_string(),
        )),
    }
}

async fn resolve_app_name(app: Option<String>) -> Result<String> {
    if let Some(app) = app {
        return Ok(app);
    }
    if let Some(path) = find_compose_file()? {
        let compose = load_compose(&path)?;
        return Ok(compose.app_name);
    }
    Err(ServinelError::Usage(
        "--app is required when no compose file is present".to_string(),
    ))
}

fn print_status(snapshot: crate::ipc::protocol::StatusSnapshot) {
    for app in snapshot.apps {
        println!("App: {}", app.app_name);
        for service in app.services {
            let uptime = service
                .uptime_secs
                .map(|u| format!("{u}s"))
                .unwrap_or_else(|| "-".to_string());
            let pid = service.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".to_string());
            let exit = service
                .exit_code
                .map(|c| c.to_string())
                .unwrap_or_else(|| "-".to_string());
            println!(
                "  {:<16} {:<10} pid={} uptime={} exit={} cpu={:.2}% mem={}KB",
                service.name,
                service.status,
                pid,
                uptime,
                exit,
                service.metrics.cpu,
                service.metrics.memory
            );
        }
    }
}