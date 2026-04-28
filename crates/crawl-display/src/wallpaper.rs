//! Wallpaper management subsystem.
//!
//! Architecture:
//! - `crawlbg/` - Native Wayland backend (wlr-layer-shell)
//! - `models.rs` - Shared domain types
//!
//! Service layer for wallpaper management.
//!
//! The service owns the state and uses crawlbg backend directly.

pub use crate::crawlbg::CrawlbgBackend;
pub use crate::crawlbg::models::{SetWallpaperRequest, WallpaperMode, WallpaperState};
use crate::config::DisplayConfig;
use crawl_ipc::events::{CrawlEvent, WallpaperEvent};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};
use serde_json;

/// Wallpaper service - owns state and uses crawlbg backend.
pub struct WallpaperService {
    backend: CrawlbgBackend,
    state: Arc<RwLock<WallpaperState>>,
    event_tx: tokio::sync::broadcast::Sender<CrawlEvent>,
    config: DisplayConfig,
}

impl WallpaperService {
    /// Create a new wallpaper service with config.
    pub fn new(
        event_tx: tokio::sync::broadcast::Sender<CrawlEvent>,
        config: DisplayConfig,
    ) -> Self {
        let mut backend = CrawlbgBackend::with_config(&config);
        if let Err(e) = backend.init() {
            warn!("Failed to init wallpaper backend: {}", e);
        }

        let service = Self {
            backend,
            state: Arc::new(RwLock::new(WallpaperState::default())),
            event_tx,
            config,
        };

        service
    }

    /// Load persisted wallpaper state from disk.
    pub async fn load_state(&self) -> anyhow::Result<()> {
        let state_path = self.get_state_path();
        if state_path.exists() {
            let contents = tokio::fs::read_to_string(&state_path).await?;
            let loaded_state: WallpaperState = serde_json::from_str(&contents)?;
            let mut state = self.state.write().await;
            *state = loaded_state;
            info!("Loaded wallpaper state from {}", state_path.display());
        }
        Ok(())
    }

    /// Save current wallpaper state to disk.
    pub async fn save_state(&self) -> anyhow::Result<()> {
        let state = self.state.read().await;
        let state_path = self.get_state_path();
        if let Some(parent) = state_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let json = serde_json::to_string_pretty(&*state)?;
        tokio::fs::write(&state_path, json).await?;
        Ok(())
    }

    fn get_state_path(&self) -> std::path::PathBuf {
        // Store in XDG_STATE_HOME or ~/.local/state/crawl/wallpaper_state.json
        let base = std::env::var("XDG_STATE_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                home::home_dir()
                    .unwrap_or_default()
                    .join(".local/state")
            });
        base.join("crawl/wallpaper_state.json")
    }

    /// Initialize with default wallpaper if configured.
    pub async fn init_defaults(&self) -> anyhow::Result<()> {
        // Try to restore previous wallpaper first
        self.load_state().await.ok();

        let saved_path: Option<String> = {
            let current = self.state.read().await;
            current.current.clone()
        };
        
        if let Some(ref path) = saved_path {
            if std::path::Path::new(path.as_str()).exists() {
                let request = SetWallpaperRequest {
                    path: path.clone(),
                    monitor: None,
                    mode: WallpaperMode::default(),
                    wallpaper_transition: self.config.wallpaper_transition.clone(),
                    wallpaper_transition_duration_ms: self.config.wallpaper_transition_duration_ms,
                    wallpaper_transition_fps: self.config.wallpaper_transition_fps,
                };
                self.set_wallpaper(request).await?;
                info!("Restored wallpaper: {}", path);
                return Ok(());
            }
        }

        // Fall back to default wallpaper
        let default_path = self.get_default_wallpaper_path();

        if let Some(path) = default_path {
            if path.exists() {
                let request = SetWallpaperRequest {
                    path: path.to_string_lossy().into(),
                    monitor: None,
                    mode: WallpaperMode::default(),
                    wallpaper_transition: self.config.wallpaper_transition.clone(),
                    wallpaper_transition_duration_ms: self.config.wallpaper_transition_duration_ms,
                    wallpaper_transition_fps: self.config.wallpaper_transition_fps,
                };
                self.set_wallpaper(request).await?;
                info!("Set default wallpaper: {}", path.display());
            }
        }
        Ok(())
    }

    fn get_default_wallpaper_path(&self) -> Option<std::path::PathBuf> {
        // Check user-configured wallpaper first
        if let Some(ref cfg_path) = self.config.wallpaper {
            if !cfg_path.is_empty() {
                return Some(std::path::PathBuf::from(cfg_path));
            }
        }
        // Fall back to asset
        Some(Self::resolve_asset_path("assets/wallpaper.png"))
    }

    fn resolve_asset_path(path: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
    }

    /// Set wallpaper.
    pub async fn set_wallpaper(&self, request: SetWallpaperRequest) -> anyhow::Result<()> {
        // Validate path exists (single validation point)
        if !std::path::Path::new(&request.path).exists() {
            let msg = format!("Wallpaper file not found: {}", request.path);
            self.send_error_event(&msg).await;
            return Err(anyhow::anyhow!(msg));
        }

        // Execute via backend
        if let Err(e) = self.backend.set_wallpaper(request.clone()) {
            let msg = format!("Failed to set wallpaper: {}", e);
            self.send_error_event(&msg).await;
            return Err(e);
        }

        // Update state
        let mut state_guard = self.state.write().await;
        state_guard.set(request.path.clone(), request.monitor.clone());

        // Send event
        self.send_event(WallpaperEvent::Changed {
            screen: request.monitor.unwrap_or_else(|| "*".to_string()),
            path: request.path.clone(),
        }).await;

        // Persist state
        drop(state_guard);
        if let Err(e) = self.save_state().await {
            warn!("Failed to save wallpaper state: {}", e);
        }

        Ok(())
    }

    /// Get current wallpaper state.
    pub async fn get_state(&self) -> WallpaperState {
        let state_guard = self.state.read().await;
        state_guard.clone()
    }

    /// Get wallpaper for specific monitor or global.
    pub async fn get_wallpaper(&self, monitor: Option<&str>) -> Option<String> {
        let state_guard = self.state.read().await;
        state_guard.get(monitor).map(|s| s.to_string())
    }

    /// Preload a wallpaper.
    pub async fn preload(&self, path: &str) -> anyhow::Result<()> {
        self.backend.preload(std::path::Path::new(path))
    }

    /// Clear wallpaper state.
    pub async fn clear_state(&self) {
        let mut state = self.state.write().await;
        *state = WallpaperState::default();
        // Also clear persisted state
        let state_path = self.get_state_path();
        if state_path.exists() {
            let _ = tokio::fs::remove_file(&state_path).await;
        }
    }

    async fn send_event(&self, event: WallpaperEvent) {
        let crawl_event = CrawlEvent::Wallpaper(event);
        let _ = self.event_tx.send(crawl_event);
    }

    async fn send_error_event(&self, message: &str) {
        let event = WallpaperEvent::Error {
            message: message.to_string(),
        };
        let crawl_event = CrawlEvent::Wallpaper(event);
        let _ = self.event_tx.send(crawl_event);
    }
}