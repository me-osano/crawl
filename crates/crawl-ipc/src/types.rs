use serde::{Deserialize, Serialize};

// ── Bluetooth ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BtDevice {
    pub address: String,
    pub name: Option<String>,
    pub connected: bool,
    pub paired: bool,
    pub rssi: Option<i16>,
    pub battery: Option<u8>,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BtStatus {
    pub powered: bool,
    pub discovering: bool,
    pub devices: Vec<BtDevice>,
}

// ── Network ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetInterface {
    pub name: String,
    pub state: String,
    pub ip4: Option<String>,
    pub ip6: Option<String>,
    pub mac: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WifiNetwork {
    pub ssid: String,
    pub signal: u8,
    pub secured: bool,
    pub connected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NetMode {
    Station,
    Ap,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetStatus {
    pub connectivity: String,
    pub wifi_enabled: bool,
    pub network_enabled: bool,
    pub mode: NetMode,
    pub active_ssid: Option<String>,
    pub interfaces: Vec<NetInterface>,
}

// ── Notifications ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Urgency {
    Low,
    Normal,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationAction {
    pub key: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: u32,
    pub app_name: String,
    pub summary: String,
    pub body: String,
    pub icon: String,
    pub urgency: Urgency,
    pub actions: Vec<NotificationAction>,
    pub expire_timeout_ms: i32,
    pub timestamp_ms: u64,
}

// ── Clipboard ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipEntry {
    pub content: String,
    pub mime: String,
    pub timestamp_ms: u64,
}

// ── Sysmon ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuStatus {
    pub aggregate: f32,
    pub cores: Vec<f32>,
    pub frequency_mhz: Vec<u64>,
    pub load_avg: LoadAvg,
    pub temperature_c: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadAvg {
    pub one: f64,
    pub five: f64,
    pub fifteen: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemStatus {
    pub total_kb: u64,
    pub used_kb: u64,
    pub available_kb: u64,
    pub swap_total_kb: u64,
    pub swap_used_kb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskStatus {
    pub mount: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub filesystem: Option<String>,
}

// ── Brightness ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrightnessStatus {
    pub device: String,
    pub current: u64,
    pub max: u64,
    pub percent: f32,
}

// ── Processes ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_percent: f32,
    pub mem_rss_kb: u64,
    pub status: String,
    pub user: Option<String>,
    pub cmd: Vec<String>,
}

// ── Media (MPRIS) ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackStatus {
    Playing,
    Paused,
    Stopped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaPlayer {
    pub player_name: String,
    pub bus_name: String,
    pub status: PlaybackStatus,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub art_url: Option<String>,
    pub position_us: Option<i64>,
    pub length_us: Option<i64>,
    pub volume: Option<f64>,
    pub can_play: bool,
    pub can_pause: bool,
    pub can_next: bool,
    pub can_prev: bool,
}

// ── Power (UPower) ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BatteryState {
    Charging,
    Discharging,
    FullyCharged,
    Empty,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryStatus {
    pub percent: f64,
    pub state: BatteryState,
    pub time_to_empty_secs: Option<i64>,
    pub time_to_full_secs: Option<i64>,
    pub energy_rate_w: Option<f64>,
    pub voltage_v: Option<f64>,
    pub temperature_c: Option<f64>,
    pub on_ac: bool,
}

// ── Disk (UDisks2) ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockDevice {
    pub device: String,
    pub label: Option<String>,
    pub size_bytes: u64,
    pub filesystem: Option<String>,
    pub mount_point: Option<String>,
    pub mounted: bool,
    pub removable: bool,
}

// ── Audio (PipeWire/PulseAudio) ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AudioDeviceKind {
    Sink,
    Source,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    pub id: u32,
    pub name: String,
    pub description: Option<String>,
    pub kind: AudioDeviceKind,
    pub volume_percent: u32,
    pub muted: bool,
    pub is_default: bool,
}
