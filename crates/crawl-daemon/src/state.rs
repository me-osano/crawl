use std::sync::Arc;
use tokio::sync::broadcast;
use crawl_ipc::CrawlEvent;
use crate::config::Config;

/// Shared application state — cloned into every axum handler via `.with_state()`.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub event_tx: broadcast::Sender<CrawlEvent>,
}

impl AppState {
    pub fn new(config: Config, event_tx: broadcast::Sender<CrawlEvent>) -> Self {
        Self {
            config: Arc::new(config),
            event_tx,
        }
    }
}
