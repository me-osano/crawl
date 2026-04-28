//! Domain models for wallpaper management.
//!
//! Contains unified error types, IPC envelopes, and domain types
//! for the crawlbg wallpaper backend.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Unified error type for crawlbg operations.
#[derive(Debug, Error)]
pub enum CrawlbgError {
    #[error("Image not found: {0}")]
    ImageNotFound(String),
    #[error("Failed to load image {path}: {source}")]
    ImageLoad {
        path: String,
        #[source]
        source: image::ImageError,
    },
    #[error("Wayland error: {0}")]
    Wayland(String),
    #[error("Output not found: {0}")]
    OutputNotFound(String),
    #[error("Transition error: {0}")]
    Transition(String),
    #[error("Cache error: {0}")]
    Cache(String),
}

/// Request to set wallpaper on one or all monitors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetWallpaperRequest {
    /// Path to the wallpaper image.
    pub path: String,
    /// Target monitor name (None = all monitors).
    #[serde(default)]
    pub monitor: Option<String>,
    /// Fill mode for the wallpaper.
    #[serde(default)]
    pub mode: WallpaperMode,
    /// Transition type for the wallpaper change.
    #[serde(default = "default_transition")]
    pub wallpaper_transition: String,
    /// Transition duration in milliseconds.
    #[serde(default = "default_duration_ms")]
    pub wallpaper_transition_duration_ms: u64,
    /// Frames per second for transition animation.
    #[serde(default = "default_fps")]
    pub wallpaper_transition_fps: u32,
}

fn default_transition() -> String {
    "fade".to_string()
}

fn default_duration_ms() -> u64 {
    500
}

fn default_fps() -> u32 {
    30
}

/// Fill mode for wallpaper display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum WallpaperMode {
    /// Fill the entire screen, potentially cropping edges.
    #[default]
    Fill,
    /// Fit the image within screen bounds, preserving aspect ratio.
    Fit,
    /// Stretch to fill screen (distorts aspect ratio).
    Stretch,
    /// Center the image at original size.
    Center,
    /// Tile the image to fill screen.
    Tile,
}

/// Current wallpaper state.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WallpaperState {
    /// Current wallpaper path (None if not set).
    pub current: Option<String>,
    /// Per-monitor wallpaper paths.
    #[serde(default)]
    pub per_monitor: std::collections::HashMap<String, String>,
    /// Whether wallpaper varies per monitor.
    #[serde(default)]
    pub per_monitor_mode: bool,
}

impl WallpaperState {
    /// Get wallpaper for a specific monitor, or the global wallpaper.
    pub fn get(&self, monitor: Option<&str>) -> Option<&str> {
        if let Some(mon) = monitor {
            self.per_monitor.get(mon).map(|s| s.as_str())
        } else {
            self.current.as_deref()
        }
    }

    /// Set wallpaper for a specific monitor or globally.
    pub fn set(&mut self, path: String, monitor: Option<String>) {
        if let Some(mon) = monitor {
            self.per_monitor.insert(mon, path);
            self.per_monitor_mode = true;
        } else {
            self.current = Some(path);
        }
    }
}



/// IPC request envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum IpcRequest {
    /// Set wallpaper.
    #[serde(rename = "set_wallpaper")]
    SetWallpaper {
        path: String,
        #[serde(default)]
        monitor: Option<String>,
        #[serde(default)]
        mode: Option<WallpaperMode>,
        #[serde(default)]
        transition: Option<String>,
    },
    /// Get current wallpaper state.
    #[serde(rename = "get_state")]
    GetState,
    /// Preload a wallpaper into cache.
    #[serde(rename = "preload")]
    Preload { path: String },
    /// Query wallpaper for specific monitor.
    #[serde(rename = "get_wallpaper")]
    GetWallpaper {
        #[serde(default)]
        monitor: Option<String>,
    },
}

/// IPC response envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcResponse {
    pub status: ResponseStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResponseStatus {
    Ok,
    Error,
}

impl IpcResponse {
    pub fn ok() -> Self {
        Self {
            status: ResponseStatus::Ok,
            data: None,
            error: None,
        }
    }

    pub fn with_data(data: impl Serialize) -> Self {
        Self {
            status: ResponseStatus::Ok,
            data: Some(serde_json::to_value(data).unwrap_or_default()),
            error: None,
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            status: ResponseStatus::Error,
            data: None,
            error: Some(message.into()),
        }
    }
}
