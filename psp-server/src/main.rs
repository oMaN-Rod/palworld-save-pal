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
    #[arg(long, default_value_t = 7257)]
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

    // App debug logs stay at DEBUG in dev, but sqlx's per-query debug lines
    // (full SQL text) are dropped to INFO to keep dev output readable.
    let filters = if cli.dev {
        tracing_subscriber::filter::Targets::new()
            .with_default(tracing::Level::DEBUG)
            .with_target("sqlx", tracing::Level::INFO)
    } else {
        tracing_subscriber::filter::Targets::new().with_default(tracing::Level::INFO)
    };
    use tracing_subscriber::layer::{Layer, SubscriberExt};
    use tracing_subscriber::util::SubscriberInitExt;
    let layer = tracing_subscriber::fmt::layer().with_filter(filters);
    tracing_subscriber::registry().with(layer).init();

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
