use crate::theme::{ThemeState, Variant};
use crate::types::*;
use serde::{Deserialize, Serialize};

/// All events broadcast over the SSE `/events` stream.
/// Quickshell and CLI --watch consumers filter by domain.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "domain", content = "data", rename_all = "snake_case")]
pub enum CrawlEvent {
    // Bluetooth
    Bluetooth(BtEvent),
    // Network
    Network(NetEvent),
    // Notifications
    Notify(NotifyEvent),
    // Clipboard
    Clipboard(ClipboardEvent),
    // Sysmon
    Sysmon(SysmonEvent),
    // Brightness
    Brightness(BrightnessEvent),
    // Processes
    Proc(ProcEvent),
    // Media
    Media(MediaEvent),
    // Power
    Power(PowerEvent),
    // Disk
    Disk(DiskEvent),
    // Audio
    Audio(AudioEvent),
    // Theme
    Theme(ThemeEvent),
    // Daemon lifecycle
    Daemon(DaemonEvent),
}

// ── Per-domain event types ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum BtEvent {
    DeviceDiscovered { device: BtDevice },
    DeviceConnected { device: BtDevice },
    DeviceDisconnected { address: String },
    DeviceRemoved { address: String },
    AdapterPowered { on: bool },
    ScanStarted,
    ScanStopped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum NetEvent {
    Connected { ssid: Option<String>, iface: String },
    Disconnected { iface: String },
    IpChanged { iface: String, ip: String },
    WifiEnabled,
    WifiDisabled,
    ModeChanged { mode: NetMode },
    ConnectivityChanged { state: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum NotifyEvent {
    New { notification: Notification },
    Closed { id: u32, reason: u32 },
    ActionInvoked { id: u32, action_key: String },
    Replaced { notification: Notification },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ClipboardEvent {
    Changed { entry: ClipEntry },
    PrimaryChanged { entry: ClipEntry },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum SysmonEvent {
    CpuUpdate { cpu: CpuStatus },
    MemUpdate { mem: MemStatus },
    CpuSpike { usage: f32, threshold: f32 },
    MemPressure { used_percent: f32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum BrightnessEvent {
    Changed { status: BrightnessStatus },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ProcEvent {
    Spawned {
        pid: u32,
        name: String,
    },
    Exited {
        pid: u32,
        name: String,
        exit_code: Option<i32>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum MediaEvent {
    PlayerAppeared {
        player: MediaPlayer,
    },
    PlayerVanished {
        bus_name: String,
    },
    TrackChanged {
        bus_name: String,
        player: MediaPlayer,
    },
    PlaybackChanged {
        bus_name: String,
        status: PlaybackStatus,
    },
    VolumeChanged {
        bus_name: String,
        volume: f64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum PowerEvent {
    BatteryUpdate { status: BatteryStatus },
    AcConnected,
    AcDisconnected,
    LowBattery { percent: f64 },
    Critical { percent: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum DiskEvent {
    DeviceMounted { device: BlockDevice },
    DeviceUnmounted { device_path: String },
    DeviceAdded { device: BlockDevice },
    DeviceRemoved { device_path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum AudioEvent {
    VolumeChanged { device: AudioDevice },
    MuteToggled { device: AudioDevice },
    DefaultSinkChanged { device: AudioDevice },
    DefaultSourceChanged { device: AudioDevice },
    DeviceAdded { device: AudioDevice },
    DeviceRemoved { id: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum DaemonEvent {
    Started,
    Stopping,
    DomainError { domain: String, message: String },
}

// ── Theme event types ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ThemeEvent {
    PaletteChanged { state: ThemeState },
    WallpaperChanged { path: String },
    Generating { wallpaper: String },
    Error { reason: String },
    VariantChanged { variant: Variant },
}
