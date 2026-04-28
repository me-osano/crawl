//! Crawlbg - Native Wayland Wallpaper Backend
//!
//! A native Wayland wallpaper backend using wlr-layer-shell.
//! Supports animated transitions with Lanczos3 scaling and LRU cache.
//!
//! Module structure:
//! - `models`: Domain types, IPC envelopes, error types
//! - `cache`: LRU image cache with eviction
//! - `image`: Image loading, Lanczos3 resizing
//! - `outputs`: Monitor/output management with scale tracking
//! - `renderer`: Per-output surface management, texture upload
//! - `transition`: Animation engine with SIMD blending
//! - `wayland`: Wayland backend using smithay-client-toolkit

pub mod models;
pub mod cache;
pub mod image;
pub mod outputs;
pub mod renderer;
pub mod transition;
mod wayland;

pub use cache::ImageCache;
pub use image::{load_image, apply_wallpaper_mode, DynamicImage};
pub use outputs::OutputManager;
pub use renderer::Renderer;
pub use transition::{TransitionConfig, TransitionKind};
pub use wayland::WaylandBackend;
pub use models::{SetWallpaperRequest, WallpaperMode, WallpaperState};

use std::path::Path;
use std::sync::Mutex;
// anyhow::Context is used via anyhow macros

/// Crawlbg backend implementation.
pub struct CrawlbgBackend {
    backend: Mutex<Option<WaylandBackend>>,
    current_path: Mutex<Option<String>>,
    transition_config: Mutex<TransitionConfig>,
    /// LRU cache of loaded images
    cache: ImageCache,
}

impl CrawlbgBackend {
    pub fn new() -> Self {
        Self {
            backend: Mutex::new(None),
            current_path: Mutex::new(None),
            transition_config: Mutex::new(TransitionConfig::default()),
            cache: ImageCache::new(),
        }
    }

    /// Create with display config to set transition settings.
    pub fn with_config(config: &crate::config::DisplayConfig) -> Self {
        let transition_kind = match config.wallpaper_transition.as_str() {
            "fade" => TransitionKind::Fade,
            "wipe" => TransitionKind::Wipe,
            "wave" => TransitionKind::Wave,
            "center" => TransitionKind::Center,
            "outer" => TransitionKind::Outer,
            "random" => TransitionKind::Random,
            "none" => TransitionKind::None,
            _ => TransitionKind::Fade,
        };
        let transition_config = TransitionConfig {
            kind: transition_kind,
            fps: config.wallpaper_transition_fps,
            duration_ms: config.wallpaper_transition_duration_ms as u32,
        };
        Self {
            backend: Mutex::new(None),
            current_path: Mutex::new(None),
            transition_config: Mutex::new(transition_config),
            cache: ImageCache::new(),
        }
    }

    pub fn name(&self) -> &'static str {
        "crawlbg"
    }

    pub fn init(&mut self) -> anyhow::Result<()> {
        if self.is_available() {
            self.ensure_backend().ok();
        }
        Ok(())
    }

    pub fn is_available(&self) -> bool {
        std::env::var("WAYLAND_DISPLAY").is_ok()
    }

    pub fn set_wallpaper(&self, request: SetWallpaperRequest) -> anyhow::Result<()> {
        // Note: path validation is done in the service layer (wallpaper.rs)
        tracing::info!("crawlbg: setting wallpaper to {}", request.path);

        let mode = request.mode;
        let fps = request.wallpaper_transition_fps;
        self.set_wallpaper_internal(Path::new(&request.path), mode, fps)?;

        Ok(())
    }

    pub fn supports_animations(&self) -> bool {
        true
    }

    /// Preload a wallpaper into the cache.
    pub fn preload(&self, path: &Path) -> anyhow::Result<()> {
        self.cache.preload(path)
    }

    /// Get a cached image or load it.
    fn get_or_load_image(&self, path: &Path) -> anyhow::Result<DynamicImage> {
        self.cache.get_or_load(path)
    }

    fn ensure_backend(&self) -> anyhow::Result<()> {
        let mut backend = self.backend.lock().unwrap();
        if backend.is_none() {
            *backend = Some(WaylandBackend::new());
        }
        Ok(())
    }

    fn set_wallpaper_internal(&self, path: &Path, mode: WallpaperMode, fps: u32) -> anyhow::Result<()> {
        self.ensure_backend()?;

        let cfg = self.transition_config.lock().unwrap().clone();

        // Load image from cache or disk (no double load)
        let img = self.get_or_load_image(path)?;

        let mut backend_guard = self.backend.lock().unwrap();
        if let Some(ref mut backend) = *backend_guard {
            // Pass the pre-loaded image to avoid double loading in Wayland thread
            backend.display_image("*", path, &cfg, mode, fps, Some(img));
        }

        if let Ok(mut current) = self.current_path.lock() {
            *current = Some(path.to_string_lossy().to_string());
        }

        Ok(())
    }
}

impl Default for CrawlbgBackend {
    fn default() -> Self {
        Self::new()
    }
}
