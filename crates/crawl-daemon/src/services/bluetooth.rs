//! Bluetooth service for crawl-daemon.
//! Handles Bluetooth management via crawl-bluetooth.

use async_trait::async_trait;
use crawl_ipc::protocol::{Response, error_code};
use serde_json::Value;
use tracing::{info, error};

use crate::services::Service;
use crate::state::AppState;

pub struct BluetoothService {
    state: std::sync::Arc<AppState>,
}

impl BluetoothService {
    pub fn new(state: std::sync::Arc<AppState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl Service for BluetoothService {
    fn name(&self) -> &'static str {
        "bluetooth"
    }

    async fn start(&self) -> anyhow::Result<()> {
        info!("Starting bluetooth service");
        let cfg = self.state.config.bluetooth.clone();
        let tx = self.state.event_bus.sender();
        
        tokio::spawn(async move {
            if let Err(e) = crawl_bluetooth::run(cfg, tx).await {
                error!(domain = "bluetooth", "Bluetooth service failed: {e:#}");
            }
        });
        
        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        info!("Stopping bluetooth service");
        // crawl-bluetooth doesn't have explicit stop
        Ok(())
    }

    async fn handle(&self, method: &str, params: &Value, id: Option<Value>) -> Option<Response> {
        match method {
            "BtStatus" => {
                match crawl_bluetooth::get_status().await {
                    Ok(s) => Some(Response::success(id, serde_json::to_value(s).unwrap_or_default())),
                    Err(e) => Some(Response::error(id, error_code::APP_BASE, &e.to_string())),
                }
            }
            "BtDevices" => {
                match crawl_bluetooth::get_devices().await {
                    Ok(d) => Some(Response::success(id, serde_json::to_value(d).unwrap_or_default())),
                    Err(e) => Some(Response::error(id, error_code::APP_BASE, &e.to_string())),
                }
            }
            "BtConnect" => {
                let address = params.get("address").and_then(|v| v.as_str()).unwrap_or("");
                match crawl_bluetooth::connect(address).await {
                    Ok(()) => Some(Response::success(id, serde_json::json!({"ok": true}))),
                    Err(e) => Some(Response::error(id, error_code::APP_BASE, &e.to_string())),
                }
            }
            "BtDisconnect" => {
                let address = params.get("address").and_then(|v| v.as_str()).unwrap_or("");
                match crawl_bluetooth::disconnect(address).await {
                    Ok(()) => Some(Response::success(id, serde_json::json!({"ok": true}))),
                    Err(e) => Some(Response::error(id, error_code::APP_BASE, &e.to_string())),
                }
            }
            "BtPower" => {
                let enabled = params.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
                match crawl_bluetooth::set_powered(enabled).await {
                    Ok(()) => Some(Response::success(id, serde_json::json!({"ok": true}))),
                    Err(e) => Some(Response::error(id, error_code::APP_BASE, &e.to_string())),
                }
            }
            _ => None,
        }
    }
}
