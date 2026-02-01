use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use crate::error::{Result, ServinelError};
use crate::ipc::protocol::{LogChunk, Request, Response};
use crate::util::{ensure_app_dir, socket_path};

const DAEMON_RETRY_ATTEMPTS: usize = 15;
const DAEMON_RETRY_DELAY_MS: u64 = 300;

pub async fn ensure_daemon() -> Result<()> {
    ensure_app_dir()?;
    if ping_daemon().await.is_ok() {
        return Ok(());
    }

    cleanup_socket_if_stale()?;
    if !daemon_process_running() {
        spawn_daemon()?;
    }
    for _ in 0..DAEMON_RETRY_ATTEMPTS {
        if ping_daemon().await.is_ok() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(DAEMON_RETRY_DELAY_MS)).await;
    }

    Err(ServinelError::DaemonNotRunning)
}

pub async fn connect() -> Result<UnixStream> {
    let path = socket_path()?;
    Ok(UnixStream::connect(path).await?)
}

async fn ping_daemon() -> Result<()> {
    let request = Request::DashAttach;
    let response = tokio::time::timeout(Duration::from_secs(1), request_response(&request))
        .await
        .map_err(|_| ServinelError::DaemonNotRunning)?
        ?;
    match response {
        Response::Ack | Response::StatusSnapshot(_) | Response::ProfilesList(_) => Ok(()),
        Response::Error(message) => Err(ServinelError::Usage(message)),
        _ => Ok(()),
    }
}

fn cleanup_socket_if_stale() -> Result<()> {
    let socket = socket_path()?;
    if socket.exists() && !daemon_process_running() {
        let _ = std::fs::remove_file(&socket);
    }
    Ok(())
}

fn daemon_process_running() -> bool {
    let mut system = sysinfo::System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    system.processes().values().any(|process| {
        let cmd = process
            .cmd()
            .iter()
            .map(|part| part.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(" ");
        cmd.contains("servinel") && cmd.contains("daemon")
    })
}

fn spawn_daemon() -> Result<()> {
    let exe = std::env::current_exe()?;
    let mut command = std::process::Command::new(exe);
    command.arg("daemon");
    let verbose = std::env::var("SERVINEL_VERBOSE_DAEMON")
        .map(|val| val == "1" || val.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if verbose {
        command.stdin(std::process::Stdio::null());
        command.stdout(std::process::Stdio::inherit());
        command.stderr(std::process::Stdio::inherit());
    } else {
        command.stdin(std::process::Stdio::null());
        command.stdout(std::process::Stdio::null());
        command.stderr(std::process::Stdio::null());
    }
    command.spawn()?;
    Ok(())
}

pub async fn request_response(request: &Request) -> Result<Response> {
    let mut stream = connect().await?;
    write_request(&mut stream, request).await?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let bytes = reader.read_line(&mut line).await?;
    if bytes == 0 {
        return Err(ServinelError::DaemonNotRunning);
    }
    let response: Response = serde_json::from_str(line.trim_end())?;
    Ok(response)
}

pub async fn stream_logs(
    request: &Request,
    mut on_chunk: impl FnMut(LogChunk),
) -> Result<()> {
    let mut stream = connect().await?;
    write_request(&mut stream, request).await?;
    let mut reader = BufReader::new(stream);
    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line).await?;
        if bytes == 0 {
            break;
        }
        let response: Response = serde_json::from_str(line.trim_end())?;
        match response {
            Response::LogChunk(chunk) => on_chunk(chunk),
            Response::Ack => break,
            Response::Error(message) => return Err(ServinelError::Usage(message)),
            _ => {}
        }
    }
    Ok(())
}

async fn write_request(stream: &mut UnixStream, request: &Request) -> Result<()> {
    let payload = serde_json::to_string(request)?;
    stream.write_all(payload.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    Ok(())
}
