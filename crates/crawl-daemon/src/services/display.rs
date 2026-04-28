//! Display service for crawl-daemon.
//! Handles brightness and wallpaper via crawl-display.

use async_trait::async_trait;
use crawl_ipc::protocol::{Response, error_code};
use serde_json::Value;
use tracing::{info, warn};

use crate::services::Service;
use crate::state::AppState;

pub struct DisplayService {
    state: std::sync::Arc<AppState>,
    wallpaper_service: std::sync::Arc<crawl_display::WallpaperService>,
}

impl DisplayService {
    pub fn new(state: std::sync::Arc<AppState>) -> Self {
        let wallpaper_service = crawl_display::WallpaperService::new(
            state.event_bus.sender(),
            state.config.display.clone(),
        );
        Self { state, wallpaper_service: std::sync::Arc::new(wallpaper_service) }
    }
}

#[async_trait]
impl Service for DisplayService {
    fn name(&self) -> &'static str {
        "display"
    }

    async fn start(&self) -> anyhow::Result<()> {
        info!("Starting display service");

        // Initialize wallpaper defaults (non-critical)
        if let Err(e) = self.wallpaper_service.init_defaults().await {
            warn!("Failed to set default wallpaper: {e:#}");
        }

        // Start brightness service
        let brightness_cfg = self.state.config.display.clone();
        let tx = self.state.event_bus.sender();
        tokio::spawn(crawl_display::run_brightness(brightness_cfg, tx));

        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        info!("Stopping display service");
        Ok(())
    }

    async fn handle(&self, method: &str, params: &Value, id: Option<Value>) -> Option<Response> {
        match method {
            "BrightnessGet" => Some(Self::handle_brightness_get(&self.state, id)),
            "BrightnessSet" => Some(Self::handle_brightness_set(&self.state, params, id)),
            "BrightnessInc" => Some(Self::handle_brightness_inc(&self.state, params, id)),
            "BrightnessDec" => Some(Self::handle_brightness_dec(&self.state, params, id)),
            "WallpaperStatus" => Some(self.handle_wallpaper_status(id).await),
            "WallpaperSet" => Some(self.handle_wallpaper_set(params, id).await),
            "WallpaperGet" => Some(self.handle_wallpaper_get(params, id).await),
            _ => None,
        }
    }
}

impl DisplayService {
    fn handle_brightness_get(state: &AppState, id: Option<Value>) -> Response {
        let backlight = match crawl_display::Backlight::open(&state.config.display) {
            Ok(b) => b,
            Err(e) => return Response::error(id, error_code::APP_BASE, &e.to_string()),
        };
        match backlight.status() {
            Ok(status) => Response::success(id, serde_json::to_value(status).unwrap_or_default()),
            Err(e) => Response::error(id, error_code::APP_BASE, &e.to_string()),
        }
    }

    fn handle_brightness_set(state: &AppState, params: &Value, id: Option<Value>) -> Response {
        let value = params.get("value").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let backlight = match crawl_display::Backlight::open(&state.config.display) {
            Ok(b) => b,
            Err(e) => return Response::error(id, error_code::APP_BASE, &e.to_string()),
        };
        match backlight.set_percent(value as f32) {
            Ok(status) => {
                state.event_bus.publish(crawl_ipc::CrawlEvent::Brightness(
                    crawl_ipc::events::BrightnessEvent::Changed { status: status.clone() }
                ));
                Response::success(id, serde_json::to_value(status).unwrap_or_default())
            }
            Err(e) => Response::error(id, error_code::APP_BASE, &e.to_string()),
        }
    }

    fn handle_brightness_inc(state: &AppState, params: &Value, id: Option<Value>) -> Response {
        let value = params.get("value").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let backlight = match crawl_display::Backlight::open(&state.config.display) {
            Ok(b) => b,
            Err(e) => return Response::error(id, error_code::APP_BASE, &e.to_string()),
        };
        match backlight.adjust_percent(value as f32) {
            Ok(status) => {
                state.event_bus.publish(crawl_ipc::CrawlEvent::Brightness(
                    crawl_ipc::events::BrightnessEvent::Changed { status: status.clone() }
                ));
                Response::success(id, serde_json::to_value(status).unwrap_or_default())
            }
            Err(e) => Response::error(id, error_code::APP_BASE, &e.to_string()),
        }
    }

    fn handle_brightness_dec(state: &AppState, params: &Value, id: Option<Value>) -> Response {
        let value = params.get("value").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let backlight = match crawl_display::Backlight::open(&state.config.display) {
            Ok(b) => b,
            Err(e) => return Response::error(id, error_code::APP_BASE, &e.to_string()),
        };
        match backlight.adjust_percent(-(value as f32)) {
            Ok(status) => {
                state.event_bus.publish(crawl_ipc::CrawlEvent::Brightness(
                    crawl_ipc::events::BrightnessEvent::Changed { status: status.clone() }
                ));
                Response::success(id, serde_json::to_value(status).unwrap_or_default())
            }
            Err(e) => Response::error(id, error_code::APP_BASE, &e.to_string()),
        }
    }

    async fn handle_wallpaper_status(&self, id: Option<Value>) -> Response {
        let state = self.wallpaper_service.get_state().await;
        Response::success(id, serde_json::to_value(state).unwrap_or_default())
    }

    async fn handle_wallpaper_set(&self, params: &Value, id: Option<Value>) -> Response {
        let path = params.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let monitor = params.get("monitor").and_then(|v| v.as_str()).map(|s| s.to_string());
        let transition = params.get("transition").and_then(|v| v.as_str()).map(|s| s.to_string());
        let mode_str = params.get("mode").and_then(|v| v.as_str()).unwrap_or("fill");
        let mode = match mode_str {
            "fit" => crawl_display::WallpaperMode::Fit,
            "stretch" => crawl_display::WallpaperMode::Stretch,
            "center" => crawl_display::WallpaperMode::Center,
            "tile" => crawl_display::WallpaperMode::Tile,
            _ => crawl_display::WallpaperMode::Fill,
        };

        let request = crawl_display::SetWallpaperRequest {
            path,
            monitor,
            mode,
            wallpaper_transition: transition.unwrap_or_default(),
            wallpaper_transition_duration_ms: 500,
            wallpaper_transition_fps: 30,
        };
        match self.wallpaper_service.set_wallpaper(request).await {
            Ok(()) => Response::success(id, serde_json::json!({"ok": true})),
            Err(e) => Response::error(id, error_code::APP_BASE, &e.to_string()),
        }
    }

    async fn handle_wallpaper_get(&self, params: &Value, id: Option<Value>) -> Response {
        let monitor = params.get("monitor").and_then(|v| v.as_str()).map(|s| s.to_string());
        let wallpaper = self.wallpaper_service.get_wallpaper(monitor.as_deref()).await;
        Response::success(id, serde_json::json!({ "wallpaper": wallpaper }))
    }
}
