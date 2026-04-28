//! Sysmon service for crawl-daemon.
//! Polls system metrics and broadcasts SysmonEvents.

use async_trait::async_trait;
use crawl_ipc::protocol::Response;
use serde_json::Value;
use tracing::{info, error};

use crate::services::Service;
use crate::state::AppState;

pub struct SysmonService {
    state: std::sync::Arc<AppState>,
}

impl SysmonService {
    pub fn new(state: std::sync::Arc<AppState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl Service for SysmonService {
    fn name(&self) -> &'static str {
        "sysmon"
    }

    async fn start(&self) -> anyhow::Result<()> {
        info!("Starting sysmon service");
        let cfg = self.state.config.sysmon.clone();
        let tx = self.state.event_bus.sender();

        tokio::spawn(async move {
            if let Err(e) = crawl_sysmon::run(cfg, tx).await {
                error!(domain = "sysmon", "Sysmon service failed: {e:#}");
            }
        });

        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        info!("Stopping sysmon service");
        Ok(())
    }

    async fn handle(&self, method: &str, _params: &Value, id: Option<Value>) -> Option<Response> {
        match method {
            "SysmonCpu" => Some(Response::success(id, serde_json::to_value(crawl_sysmon::get_cpu()).unwrap_or_default())),
            "SysmonMem" => Some(Response::success(id, serde_json::to_value(crawl_sysmon::get_mem()).unwrap_or_default())),
            "SysmonDisk" => Some(Response::success(id, serde_json::to_value(crawl_sysmon::get_disks()).unwrap_or_default())),
            "SysmonNet" => Some(Response::success(id, serde_json::to_value(crawl_sysmon::get_net()).unwrap_or_default())),
            "SysmonGpu" => Some(Response::success(id, serde_json::to_value(crawl_sysmon::get_gpu()).unwrap_or_default())),
            _ => None,
        }
    }
}
