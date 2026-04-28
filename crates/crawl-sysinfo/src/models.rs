//! Shared models for system information.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Known compositor types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CompositorType {
    #[default]
    Unknown,
    Hyprland,
    Sway,
    Niri,
    Mango,
    Labwc,
    Scroll,
}

impl CompositorType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Hyprland => "hyprland",
            Self::Sway => "sway",
            Self::Niri => "niri",
            Self::Mango => "mango",
            Self::Labwc => "labwc",
            Self::Scroll => "scroll",
        }
    }
}

impl std::fmt::Display for CompositorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Capabilities supported by a compositor.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CompositorCapabilities {
    /// Layer shell support (required for panels, wallpapers)
    pub layer_shell: bool,
    /// Blur effect support
    pub blur: bool,
    /// Screencopy/dmabuf support
    pub screencopy: bool,
    /// Direct wallpaper control (awww, hyprpaper compatible)
    pub wallpaper_control: bool,
    /// DPMS/monitor power control
    pub dpms: bool,
    /// Uses Unix socket IPC
    pub socket_ipc: bool,
    /// Uses HTTP-based IPC
    pub http_ipc: bool,
}

impl CompositorCapabilities {
    /// Get capabilities for a known compositor.
    pub fn for_compositor(compositor: CompositorType) -> Self {
        match compositor {
            CompositorType::Hyprland => Self {
                layer_shell: true,
                blur: true,
                screencopy: true,
                wallpaper_control: true,
                dpms: true,
                socket_ipc: false,
                http_ipc: true,
            },
            CompositorType::Sway => Self {
                layer_shell: true,
                blur: true,
                screencopy: true,
                wallpaper_control: true,
                dpms: true,
                socket_ipc: true,
                http_ipc: false,
            },
            CompositorType::Niri => Self {
                layer_shell: true,
                blur: true,
                screencopy: true,
                wallpaper_control: true,
                dpms: true,
                socket_ipc: true,
                http_ipc: false,
            },
            CompositorType::Mango => Self {
                layer_shell: true,
                blur: true,
                screencopy: true,
                wallpaper_control: true,
                dpms: true,
                socket_ipc: true,
                http_ipc: false,
            },
            CompositorType::Labwc => Self {
                layer_shell: true,
                blur: false,
                screencopy: true,
                wallpaper_control: true,
                dpms: true,
                socket_ipc: true,
                http_ipc: false,
            },
            CompositorType::Scroll => Self {
                layer_shell: true,
                blur: true,
                screencopy: true,
                wallpaper_control: true,
                dpms: true,
                socket_ipc: true,
                http_ipc: false,
            },
            CompositorType::Unknown => Self::default(),
        }
    }
}

/// Compositor information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositorInfo {
    /// Detected compositor type.
    #[serde(rename = "type")]
    pub compositor_type: CompositorType,
    /// Human-readable name.
    pub name: String,
    /// Compositor capabilities.
    pub capabilities: CompositorCapabilities,
}

/// Operating system information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsInfo {
    /// OS name (e.g., "Arch Linux").
    pub name: String,
    /// Kernel version (e.g., "6.8.1-arch1").
    pub kernel: String,
    /// Pretty name from os-release if available.
    pub pretty_name: String,
    /// System hostname.
    pub hostname: String,
    /// OS ID (e.g., "arch").
    pub id: String,
}

/// Session information.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SessionType {
    #[default]
    Unknown,
    Wayland,
    X11,
    Tty,
}

impl SessionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Wayland => "wayland",
            Self::X11 => "x11",
            Self::Tty => "tty",
        }
    }
}

impl std::fmt::Display for SessionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Session information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    /// Session type (Wayland/X11/TTY).
    #[serde(rename = "type")]
    pub session_type: SessionType,
    /// Current user.
    pub user: String,
    /// Seat name (e.g., "Seat0").
    pub seat: Option<String>,
    /// Home directory.
    pub home: String,
    /// Current shell (e.g., "zsh", "bash").
    pub shell: Option<String>,
    /// Terminal emulator (e.g., "kitty", "wezterm").
    pub terminal: Option<String>,
    /// System uptime in seconds.
    pub uptime_seconds: Option<u64>,
}

/// Hardware information (static snapshot).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    /// CPU model string.
    pub cpu_model: String,
    /// Number of CPU cores.
    pub cpu_cores: usize,
    /// Total memory in bytes.
    pub memory_total: u64,
    /// GPU info if detectable.
    pub gpu: Option<String>,
    /// Disk usage: total bytes (root).
    pub disk_total: Option<u64>,
    /// Disk usage: used bytes (root).
    pub disk_used: Option<u64>,
}

/// Monitor/display information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorInfo {
    /// Monitor name (e.g., "DP-1", "eDP-1").
    pub name: String,
    /// Display scale factor.
    pub scale: f32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// X position.
    pub x: i32,
    /// Y position.
    pub y: i32,
    /// Refresh rate in Hz.
    pub refresh_rate: f32,
    /// Is this the focused monitor?
    pub focused: bool,
    /// Is the monitor active?
    pub active: bool,
}

/// Display information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayInfo {
    /// List of monitors.
    pub monitors: Vec<MonitorInfo>,
    /// Scales per monitor (for backward compat).
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub scales: HashMap<String, f32>,
}

/// Complete system information snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    /// Compositor information.
    pub compositor: CompositorInfo,
    /// Operating system information.
    pub os: OsInfo,
    /// Session information.
    pub session: SessionInfo,
    /// Hardware information.
    pub hardware: HardwareInfo,
    /// Display information.
    pub display: DisplayInfo,
}

impl Default for SystemInfo {
    fn default() -> Self {
        Self {
            compositor: CompositorInfo {
                compositor_type: CompositorType::Unknown,
                name: String::from("Unknown"),
                capabilities: CompositorCapabilities::default(),
            },
            os: OsInfo {
                name: String::new(),
                kernel: String::new(),
                pretty_name: String::new(),
                hostname: String::new(),
                id: String::new(),
            },
            session: SessionInfo {
                session_type: SessionType::Unknown,
                user: String::new(),
                seat: None,
                home: String::new(),
                shell: None,
                terminal: None,
                uptime_seconds: None,
            },
            hardware: HardwareInfo {
                cpu_model: String::new(),
                cpu_cores: 0,
                memory_total: 0,
                gpu: None,
                disk_total: None,
                disk_used: None,
            },
            display: DisplayInfo {
                monitors: Vec::new(),
                scales: HashMap::new(),
            },
        }
    }
}
