//! Display configuration types.

use serde::{Deserialize, Serialize};

/// Unified display configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    #[serde(default)]
    pub brightness_min: f32,
    #[serde(default)]
    pub brightness_max: f32,
    #[serde(default)]
    pub brightness_device: String,
    #[serde(default = "default_transition")]
    pub wallpaper_transition: String,
    #[serde(default = "default_duration")]
    pub wallpaper_transition_duration_ms: u64,
    #[serde(default = "default_fps")]
    pub wallpaper_transition_fps: u32,
    #[serde(default)]
    pub wallpaper: Option<String>,
}

fn default_transition() -> String {
    "fade".to_string()
}

fn default_duration() -> u64 {
    500
}

fn default_fps() -> u32 {
    30
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            brightness_min: 1.0,
            brightness_max: 100.0,
            brightness_device: String::new(),
            wallpaper_transition: default_transition(),
            wallpaper_transition_duration_ms: default_duration(),
            wallpaper_transition_fps: default_fps(),
            wallpaper: None,  // empty = use default asset
        }
    }
}