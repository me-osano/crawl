mod config;
mod router;
mod sse;
mod state;

use anyhow::Context;
use std::path::PathBuf;
use tower::Service;
use tokio::net::UnixListener;
use tokio::sync::broadcast;
use tracing::{error, info};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use crawl_ipc::CrawlEvent;
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── Tracing ──────────────────────────────────────────────────────────────
    let log_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(log_filter)
        .init();

    info!("crawl-daemon starting");

    // ── Config ───────────────────────────────────────────────────────────────
    let cfg = config::load().context("failed to load crawl config")?;
    info!("config loaded from {:?}", cfg.config_path);

    // ── Broadcast channel ────────────────────────────────────────────────────
    // All domain tasks publish CrawlEvents here; SSE handler fans out to clients.
    let (event_tx, _) = broadcast::channel::<CrawlEvent>(512);

    // ── Spawn domain tasks ───────────────────────────────────────────────────
    let theme_state = crawl_theme::initial_state(&cfg.theme).await;
    let state = AppState::new(cfg.clone(), event_tx.clone(), theme_state);

    spawn_domains(&state).await;

    // ── Unix socket cleanup ──────────────────────────────────────────────────
    let socket_path = PathBuf::from(&cfg.daemon.socket_path);
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)
            .with_context(|| format!("failed to remove stale socket {:?}", socket_path))?;
    }

    // ── axum router ─────────────────────────────────────────────────────────
    let app = router::build(state);

    let listener = UnixListener::bind(&socket_path)
        .with_context(|| format!("failed to bind Unix socket {:?}", socket_path))?;

    info!("listening on {:?}", socket_path);
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let app = app.clone();
                tokio::spawn(async move {
                    let io = hyper_util::rt::TokioIo::new(stream);
                    let hyper_service = hyper::service::service_fn(move |req| {
                        let mut app = app.clone();
                        app.call(req)
                    });
                    let conn = hyper::server::conn::http1::Builder::new();
                    let serve_result = conn.serve_connection(io, hyper_service).await;
                    if let Err(e) = serve_result {
                        tracing::warn!("connection error: {e}");
                    }
                });
            }
            Err(e) => {
                tracing::error!("failed to accept connection: {e}");
            }
        }
    }
}

async fn spawn_domains(state: &AppState) {
    let tx = state.event_tx.clone();
    let cfg = state.config.clone();

    macro_rules! spawn_domain {
        ($name:expr, $domain:ident, $crate_name:ident, $cfg_field:expr) => {{
            let tx = tx.clone();
            let domain_cfg = $cfg_field.clone();
            tokio::spawn(async move {
                if let Err(e) = $crate_name::run(domain_cfg, tx).await {
                    error!(domain = $name, "domain task failed: {e:#}");
                }
            });
        }};
    }

    spawn_domain!("bluetooth",         bluetooth,         crawl_bluetooth,         cfg.bluetooth);
    spawn_domain!("network",        network,        crawl_network,        cfg.network);
    spawn_domain!("notify",     notify,     crawl_notify,     cfg.notifications);
    spawn_domain!("clipboard",  clipboard,  crawl_clipboard,  cfg.clipboard);
    spawn_domain!("sysmon",     sysmon,     crawl_sysmon,     cfg.sysmon);
    spawn_domain!("brightness", brightness, crawl_brightness, cfg.brightness);
    spawn_domain!("proc",       proc_,      crawl_proc,       cfg.processes);
    spawn_domain!("media",      media,      crawl_media,      cfg.media);
    spawn_domain!("power",      power,      crawl_power,      cfg.power);
    spawn_domain!("disk",       disk,       crawl_disk,       cfg.disk);
    spawn_domain!("audio",      audio,      crawl_audio,      cfg.audio);
    spawn_domain!("theme",      theme,      crawl_theme,      cfg.theme);
}
