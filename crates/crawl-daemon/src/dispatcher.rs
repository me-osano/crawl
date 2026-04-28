//! Command dispatcher for crawl-daemon.
//! Routes JSON-RPC requests to appropriate service handlers.
//! Now delegates to ServiceRegistry for domain-specific commands.

use serde_json::Value;
use std::sync::Arc;

use crawl_ipc::protocol::{Response, error_code};

use crate::services::ServiceRegistry;

/// Dispatcher that routes commands to services.
#[derive(Clone)]
pub struct Dispatcher {
    pub(crate) services: Arc<ServiceRegistry>,
}

impl Dispatcher {
    pub fn new() -> Self {
        Self {
            services: Arc::new(ServiceRegistry::new()),
        }
    }

    /// Get a mutable reference to the service registry (for registering services).
    pub fn registry(&mut self) -> &mut ServiceRegistry {
        Arc::get_mut(&mut self.services).unwrap()
    }

    pub async fn dispatch(
        &self,
        method: String,
        params: Value,
        id: Option<Value>,
    ) -> Response {
        // Try services first
        if let Some(response) = self.services.dispatch(&method, &params, id.clone()).await {
            return response;
        }

        // Fallback to built-in commands
        match method.as_str() {
            "Ping" => Response::success(id, serde_json::json!({"time_ms": crawl_ipc::protocol::now_ms()})),
            "Hello" => Response::success(id, serde_json::json!({"version": env!("CARGO_PKG_VERSION"), "time_ms": crawl_ipc::protocol::now_ms()})),
            "Health" => self.handle_health(id).await,
            "Subscribe" => Response::success(id, serde_json::json!({ "subscribed": true })),
            _ => Response::error(id, error_code::METHOD_NOT_FOUND, &format!("Unknown method: {}", method)),
        }
    }

    async fn handle_health(&self, id: Option<Value>) -> Response {
        let services = self.services.health_check();
        Response::success(id, serde_json::json!({
            "healthy": services.values().all(|&v| v),
            "services": services
        }))
    }
}
