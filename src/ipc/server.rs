use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;

use crate::daemon::{Daemon, LogSubscription};
use crate::error::{Result, ServinelError};
use crate::ipc::protocol::{LogChunk, Request, Response};

pub async fn serve(listener: UnixListener, daemon: Arc<Daemon>) -> Result<()> {
    loop {
        let (stream, _) = listener.accept().await?;
        let daemon = daemon.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_connection(stream, daemon).await {
                tracing::error!("IPC connection failed: {err}");
            }
        });
    }
}

async fn handle_connection(stream: UnixStream, daemon: Arc<Daemon>) -> Result<()> {
    let (read, mut write) = stream.into_split();
    let mut reader = BufReader::new(read);
    let mut line = String::new();
    let bytes = reader.read_line(&mut line).await?;
    if bytes == 0 {
        return Ok(());
    }
    let request: Request = serde_json::from_str(line.trim_end())?;
    tracing::info!(?request, "ipc: received request");
    match request {
        Request::Up { file, profile } => {
            if let Err(err) = daemon.up(file.clone(), profile.clone()).await {
                tracing::error!(?err, ?file, ?profile, "daemon up failed");
                write_response(&mut write, &Response::Error(err.to_string())).await?;
                return Ok(());
            }
            write_response(&mut write, &Response::Ack).await?;
        }
        Request::Start { file, app, selector } => {
            if let Err(err) = daemon.start(file, app, selector).await {
                write_response(&mut write, &Response::Error(err.to_string())).await?;
                return Ok(());
            }
            write_response(&mut write, &Response::Ack).await?;
        }
        Request::Stop { app, selector } => {
            if let Err(err) = daemon.stop(app, selector).await {
                write_response(&mut write, &Response::Error(err.to_string())).await?;
                return Ok(());
            }
            write_response(&mut write, &Response::Ack).await?;
        }
        Request::Restart { app, selector } => {
            if let Err(err) = daemon.restart(app, selector).await {
                write_response(&mut write, &Response::Error(err.to_string())).await?;
                return Ok(());
            }
            write_response(&mut write, &Response::Ack).await?;
        }
        Request::Status { app, selector } => {
            match daemon.status(app, selector).await {
                Ok(snapshot) => {
                    write_response(&mut write, &Response::StatusSnapshot(snapshot)).await?;
                }
                Err(err) => {
                    write_response(&mut write, &Response::Error(err.to_string())).await?;
                }
            }
        }
        Request::Profiles { app } => {
            match daemon.profiles(app).await {
                Ok(profiles) => {
                    write_response(&mut write, &Response::ProfilesList(profiles)).await?;
                }
                Err(err) => {
                    write_response(&mut write, &Response::Error(err.to_string())).await?;
                }
            }
        }
        Request::Logs {
            app,
            selector,
            follow,
            tail,
            merged: _,
        } => {
            let (chunks, subs) = match daemon.logs(app, selector, tail).await {
                Ok(result) => result,
                Err(err) => {
                    write_response(&mut write, &Response::Error(err.to_string())).await?;
                    return Ok(());
                }
            };
            for chunk in chunks {
                write_response(&mut write, &Response::LogChunk(chunk)).await?;
            }
            if follow {
                stream_logs(write, subs).await?;
            } else {
                write_response(&mut write, &Response::Ack).await?;
            }
        }
        Request::DashAttach => {
            write_response(&mut write, &Response::Ack).await?;
        }
        Request::Down { app } => {
            match daemon.down(app).await {
                Ok(true) => {
                    write_response(&mut write, &Response::DaemonShutdown).await?;
                }
                Ok(false) => {
                    write_response(&mut write, &Response::Ack).await?;
                }
                Err(err) => {
                    write_response(&mut write, &Response::Error(err.to_string())).await?;
                }
            }
        }
    }

    Ok(())
}

async fn stream_logs(
    mut write: tokio::net::unix::OwnedWriteHalf,
    subs: Vec<LogSubscription>,
) -> Result<()> {
    let (tx, mut rx) = mpsc::unbounded_channel::<LogChunk>();
    for mut sub in subs {
        let tx = tx.clone();
        tokio::spawn(async move {
            while let Ok(entry) = sub.receiver.recv().await {
                let _ = tx.send(LogChunk {
                    app: sub.app.clone(),
                    service: sub.service.clone(),
                    entry,
                });
            }
        });
    }
    drop(tx);

    while let Some(chunk) = rx.recv().await {
        if let Err(err) = write_response(&mut write, &Response::LogChunk(chunk)).await {
            tracing::error!("Failed to write log chunk: {err}");
            return Err(ServinelError::DaemonNotRunning);
        }
    }
    Ok(())
}

async fn write_response(
    write: &mut tokio::net::unix::OwnedWriteHalf,
    response: &Response,
) -> Result<()> {
    let payload = serde_json::to_string(response)?;
    write.write_all(payload.as_bytes()).await?;
    write.write_all(b"\n").await?;
    Ok(())
}
