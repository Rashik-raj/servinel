mod cli;
mod compose;
mod daemon;
mod error;
mod ipc;
mod logs;
mod metrics;
mod tui;
mod util;

use clap::Parser;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = cli::Cli::parse();
    if let Err(err) = cli::execute(cli).await {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}
