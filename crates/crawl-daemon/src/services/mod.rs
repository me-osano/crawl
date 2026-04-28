//! Service abstraction for crawl-daemon.
//! All feature services implement this trait for uniform lifecycle management.

pub mod audio;
pub mod bluetooth;
pub mod display;
pub mod network;
pub mod proc;
pub mod sysmon;
pub mod sysinfo;

use async_trait::async_trait;
use anyhow::Result;
use crawl_ipc::protocol::Response;
use serde_json::Value;
use std::sync::Arc;

/// Core service trait that all daemon services must implement.
/// This allows uniform lifecycle management and service discovery.
#[async_trait]
pub trait Service: Send + Sync {
    /// Human-readable service name for logging/debugging.
    fn name(&self) -> &'static str;

    /// Start the service. Called during daemon startup.
    async fn start(&self) -> Result<()>;

    /// Stop the service. Called during daemon shutdown.
    /// Default implementation does nothing (graceful no-op).
    async fn stop(&self) -> Result<()> {
        Ok(())
    }

    /// Handle a command. Returns Some(Response) if this service handles the method.
    /// Default returns None (not handled).
    async fn handle(&self, _method: &str, _params: &Value, _id: Option<Value>) -> Option<Response> {
        None
    }

    #[allow(dead_code)]
    /// Health check. Override if service has health state.
    fn is_healthy(&self) -> bool {
        true
    }
}

#[derive(Clone)]
/// Registry for discovering and managing services.
pub struct ServiceRegistry {
    pub(crate) services: std::collections::HashMap<String, Arc<dyn Service>>,
}

impl ServiceRegistry {
    pub fn new() -> Self {
        Self {
            services: std::collections::HashMap::new(),
        }
    }

    /// Register a service with the registry.
    pub fn register(&mut self, service: Arc<dyn Service>) {
        let name = service.name().to_string();
        tracing::info!("Registering service: {}", name);
        self.services.insert(name, service);
    }

    #[allow(dead_code)]
    /// Get a service by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn Service>> {
        self.services.get(name).cloned()
    }

    /// Start all registered services in registration order.
    pub async fn start_all(&self) -> Result<()> {
        for (name, service) in &self.services {
            tracing::info!("Starting service: {}", name);
            service.start().await?;
        }
        Ok(())
    }

    /// Stop all registered services (reverse order).
    pub async fn stop_all(&self) -> Result<()> {
        let mut names: Vec<String> = self.services.keys().cloned().collect();
        names.sort_by(|a, b| b.cmp(a));
        for name in names {
            let service = self.services.get(&name).unwrap();
            tracing::info!("Stopping service: {}", name);
            if let Err(e) = service.stop().await {
                tracing::error!("Failed to stop service {}: {}", name, e);
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    /// Check health of all services.
    pub fn health_check(&self) -> std::collections::HashMap<String, bool> {
        self.services
            .iter()
            .map(|(name, svc)| (name.clone(), svc.is_healthy()))
            .collect()
    }

    /// Dispatch a command to the appropriate service.
    /// Returns Some(Response) if handled, None otherwise.
    pub async fn dispatch(&self, method: &str, params: &Value, id: Option<Value>) -> Option<Response> {
        for service in self.services.values() {
            if let Some(response) = service.handle(method, params, id.clone()).await {
                return Some(response);
            }
        }
        None
    }
}

impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}
