//! Audio service for crawl-daemon.
//! Wraps crawl-audio functionality behind the Service trait.

use async_trait::async_trait;
use crawl_ipc::protocol::{Response, error_code};
use serde_json::Value;
use tracing::{info, error};

use crate::services::Service;
use crate::state::AppState;

pub struct AudioService {
    state: std::sync::Arc<AppState>,
}

impl AudioService {
    pub fn new(state: std::sync::Arc<AppState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl Service for AudioService {
    fn name(&self) -> &'static str {
        "audio"
    }

    async fn start(&self) -> anyhow::Result<()> {
        info!("Starting audio service");
        let cfg = self.state.config.audio.clone();
        let tx = self.state.event_bus.sender();
        
        tokio::spawn(async move {
            if let Err(e) = crawl_audio::run(cfg, tx).await {
                error!(domain = "audio", "Audio service failed: {e:#}");
            }
        });
        
        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        info!("Stopping audio service");
        // crawl-audio doesn't have an explicit stop, it will end when the task is dropped
        Ok(())
    }

    async fn handle(&self, method: &str, params: &Value, id: Option<Value>) -> Option<Response> {
        if method != "Audio" {
            return None;
        }

        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("sinks");
        let cfg = &self.state.config.audio;
        
        let result = match action {
            "sinks" => {
                match crawl_audio::list_sinks(cfg).await {
                    Ok(devs) => serde_json::json!({ "devices": devs }),
                    Err(e) => return Some(Response::error(id, error_code::APP_BASE, &e.to_string())),
                }
            }
            "sources" => {
                match crawl_audio::list_sources(cfg).await {
                    Ok(devs) => serde_json::json!({ "devices": devs }),
                    Err(e) => return Some(Response::error(id, error_code::APP_BASE, &e.to_string())),
                }
            }
            "volume" => {
                let percent = params.get("percent").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                match crawl_audio::set_output_volume(cfg, percent).await {
                    Ok(()) => serde_json::json!({ "ok": true, "volume_percent": percent }),
                    Err(e) => return Some(Response::error(id, error_code::APP_BASE, &e.to_string())),
                }
            }
            "input_volume" => {
                let percent = params.get("percent").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                match crawl_audio::set_input_volume(cfg, percent).await {
                    Ok(()) => serde_json::json!({ "ok": true, "volume_percent": percent }),
                    Err(e) => return Some(Response::error(id, error_code::APP_BASE, &e.to_string())),
                }
            }
            "mute" => {
                match crawl_audio::toggle_output_mute(cfg).await {
                    Ok(muted) => serde_json::json!({ "ok": true, "muted": muted }),
                    Err(e) => return Some(Response::error(id, error_code::APP_BASE, &e.to_string())),
                }
            }
            "unmute" => {
                match crawl_audio::toggle_output_mute(cfg).await {
                    Ok(_) => serde_json::json!({ "ok": true, "muted": false }),
                    Err(e) => return Some(Response::error(id, error_code::APP_BASE, &e.to_string())),
                }
            }
            _ => return Some(Response::error(id, error_code::METHOD_NOT_FOUND, &format!("Unknown audio action: {}", action))),
        };
        
        Some(Response::success(id, result))
    }
}
