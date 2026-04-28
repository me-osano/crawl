//! Proc service for crawl-daemon.
//! Manages process listing, search, and monitoring.

use async_trait::async_trait;
use crawl_ipc::protocol::{Response, error_code};
use serde_json::Value;
use tracing::{info, error};

use crate::services::Service;
use crate::state::AppState;

pub struct ProcService {
    state: std::sync::Arc<AppState>,
}

impl ProcService {
    pub fn new(state: std::sync::Arc<AppState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl Service for ProcService {
    fn name(&self) -> &'static str {
        "proc"
    }

    async fn start(&self) -> anyhow::Result<()> {
        info!("Starting proc service");
        let cfg = self.state.config.processes.clone();
        let tx = self.state.event_bus.sender();

        tokio::spawn(async move {
            if let Err(e) = crawl_proc::run(cfg, tx).await {
                error!(domain = "proc", "Proc service failed: {e:#}");
            }
        });

        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        info!("Stopping proc service");
        Ok(())
    }

    async fn handle(&self, method: &str, params: &Value, id: Option<Value>) -> Option<Response> {
        match method {
            "ProcList" => {
                let sort = params.get("sort").and_then(|v| v.as_str()).unwrap_or(&self.state.config.processes.sort_by);
                let top = params.get("top").and_then(|v| v.as_u64()).unwrap_or(self.state.config.processes.top as u64) as usize;
                Some(Response::success(id, serde_json::to_value(crawl_proc::list_processes(&sort, top)).unwrap_or_default()))
            }
            "ProcTop" => {
                let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
                let top_cpu = crawl_proc::list_processes("cpu", limit);
                let top_mem = crawl_proc::list_processes("mem", limit);
                Some(Response::success(id, serde_json::json!({
                    "top_by_cpu": top_cpu,
                    "top_by_mem": top_mem
                })))
            }
            "ProcFind" => {
                let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if name.is_empty() {
                    return Some(Response::error(id, error_code::INVALID_PARAMS, "name is required"));
                }
                Some(Response::success(id, serde_json::to_value(crawl_proc::find_processes(&name)).unwrap_or_default()))
            }
            "ProcKill" => {
                let pid = params.get("pid").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                let force = params.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
                match crawl_proc::kill_process(pid, force) {
                    Ok(()) => Some(Response::success(id, serde_json::json!({"ok": true}))),
                    Err(e) => Some(Response::error(id, error_code::APP_BASE, &e.to_string())),
                }
            }
            "ProcWatch" => {
                let pid = params.get("pid").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                match crawl_proc::watch_pid(pid).await {
                    Ok(name) => Some(Response::success(id, serde_json::json!({"pid": pid, "name": name, "exit_code": serde_json::Value::Null}))),
                    Err(e) => Some(Response::error(id, error_code::APP_BASE, &e.to_string())),
                }
            }
            _ => None,
        }
    }
}
