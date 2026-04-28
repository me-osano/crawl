mod config;
mod daemon;
mod state;
mod event_bus;
mod services;
mod dispatcher;

use anyhow::Result;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    let log_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(log_filter)
        .init();

    let mut daemon = daemon::Daemon::new()?;
    
    // Register services
    daemon.register_services();

    // Run (blocks until shutdown)
    daemon.run().await?;

    Ok(())
}