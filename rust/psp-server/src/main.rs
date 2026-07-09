use std::net::IpAddr;
use std::path::PathBuf;

use clap::Parser;

use psp_server::{start_server, ServerConfig};

/// Palworld Save Pal server.
#[derive(Parser, Debug)]
#[command(name = "psp-server", version)]
struct Cli {
    /// Host to bind (web default 0.0.0.0; desktop uses 127.0.0.1).
    #[arg(long, default_value = "0.0.0.0")]
    host: IpAddr,
    /// Port to run the server on.
    #[arg(long, default_value_t = 5174)]
    port: u16,
    /// Directory containing json/ game data.
    #[arg(long, default_value = "data")]
    data_dir: PathBuf,
    /// Directory containing the built SvelteKit UI.
    #[arg(long, default_value = "ui")]
    ui_dir: PathBuf,
    /// SQLite database file.
    #[arg(long, default_value = "psp-rs.db")]
    db: PathBuf,
    /// Development mode (debug logging).
    #[arg(long)]
    dev: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let log_level = if cli.dev {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };
    tracing_subscriber::fmt().with_max_level(log_level).init();

    let handle = start_server(ServerConfig {
        host: cli.host,
        port: cli.port,
        ui_dir: cli.ui_dir,
        data_dir: cli.data_dir,
        db_path: cli.db,
        desktop_mode: false,
    })
    .await?;
    handle.wait().await;
    Ok(())
}
