//! Sysinfo service for crawl-daemon.
//! Provides system information collection.

use async_trait::async_trait;
use crawl_ipc::protocol::Response;
use serde_json::Value;
use tracing::{info, error};

use crate::services::Service;
use crate::state::AppState;

pub struct SysinfoService {
    state: std::sync::Arc<AppState>,
}

impl SysinfoService {
    pub fn new(state: std::sync::Arc<AppState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl Service for SysinfoService {
    fn name(&self) -> &'static str {
        "sysinfo"
    }

    async fn start(&self) -> anyhow::Result<()> {
        info!("Starting sysinfo service");
        let cfg = self.state.config.sysinfo.clone();
        let tx = self.state.event_bus.sender();
        
        tokio::spawn(async move {
            if let Err(e) = cfg.run(tx).await {
                error!(domain = "sysinfo", "Sysinfo service failed: {e:#}");
            }
        });
        
        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        info!("Stopping sysinfo service");
        // sysinfo doesn't have explicit stop
        Ok(())
    }

    async fn handle(&self, method: &str, _params: &Value, id: Option<Value>) -> Option<Response> {
        if method == "Sysinfo" {
            Some(Self::handle_sysinfo(id))
        } else {
            None
        }
    }
}

impl SysinfoService {
    fn handle_sysinfo(id: Option<Value>) -> Response {
        let info = crawl_sysinfo::get_info();
        Response::success(id, serde_json::to_value(info).unwrap_or_default())
    }
}
