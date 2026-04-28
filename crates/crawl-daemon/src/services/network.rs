//! Network service for crawl-daemon.
//! Handles network management via crawl-network.

use async_trait::async_trait;
use crawl_ipc::protocol::{Response, error_code};
use serde_json::Value;
use tracing::{info, error};

use crate::services::Service;
use crate::state::AppState;

pub struct NetworkService {
    state: std::sync::Arc<AppState>,
}

impl NetworkService {
    pub fn new(state: std::sync::Arc<AppState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl Service for NetworkService {
    fn name(&self) -> &'static str {
        "network"
    }

    async fn start(&self) -> anyhow::Result<()> {
        info!("Starting network service");
        let cfg = self.state.config.network.clone();
        let tx = self.state.event_bus.sender();
        
        tokio::spawn(async move {
            if let Err(e) = crawl_network::run(cfg, tx).await {
                error!(domain = "network", "Network service failed: {e:#}");
            }
        });
        
        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        info!("Stopping network service");
        // crawl-network doesn't have explicit stop
        Ok(())
    }

    async fn handle(&self, method: &str, _params: &Value, id: Option<Value>) -> Option<Response> {
        match method {
            "NetStatus" => {
                match crawl_network::get_status().await {
                    Ok(status) => Some(Response::success(id, serde_json::to_value(status).unwrap_or_default())),
                    Err(e) => Some(Response::error(id, error_code::APP_BASE, &e.to_string())),
                }
            }
            "NetWifiList" => {
                match crawl_network::list_wifi().await {
                    Ok(list) => Some(Response::success(id, serde_json::to_value(list).unwrap_or_default())),
                    Err(e) => Some(Response::error(id, error_code::APP_BASE, &e.to_string())),
                }
            }
            "NetWifiConnect" => {
                let params = _params;
                let ssid = params.get("ssid").and_then(|v| v.as_str()).unwrap_or("");
                let password = params.get("password").and_then(|v| v.as_str());
                match crawl_network::connect_wifi(ssid, password).await {
                    Ok(()) => Some(Response::success(id, serde_json::json!({"ok": true}))),
                    Err(e) => Some(Response::error(id, error_code::APP_BASE, &e.to_string())),
                }
            }
            "NetWifiDisconnect" => {
                match crawl_network::disconnect_wifi().await {
                    Ok(()) => Some(Response::success(id, serde_json::json!({"ok": true}))),
                    Err(e) => Some(Response::error(id, error_code::APP_BASE, &e.to_string())),
                }
            }
            _ => None,
        }
    }
}
