use crate::config::Config;
use crawl_ipc::CrawlEvent;
use crawl_theme::ThemeState;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::Mutex;

/// Shared application state — cloned into every axum handler via `.with_state()`.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub event_tx: broadcast::Sender<CrawlEvent>,
    pub theme_state: Arc<Mutex<ThemeState>>,
    pub notify_store: Arc<crawl_notify::NotifyStore>,
}

impl AppState {
    pub fn new(
        config: Config,
        event_tx: broadcast::Sender<CrawlEvent>,
        theme_state: ThemeState,
        notify_store: Arc<crawl_notify::NotifyStore>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            event_tx,
            theme_state: Arc::new(Mutex::new(theme_state)),
            notify_store,
        }
    }
}
