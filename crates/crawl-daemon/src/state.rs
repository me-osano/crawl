use crate::config::Config;
use crate::event_bus::EventBus;
use std::sync::Arc;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub event_bus: EventBus,
}

impl AppState {
    pub fn new(config: Config, event_bus: EventBus) -> Self {
        Self {
            config: Arc::new(config),
            event_bus,
        }
    }
}