use serde::{Deserialize, Serialize};

// === System Information ===
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub compositor: CompositorInfo,
    pub os: OsInfo,
    pub session: SessionInfo,
    pub hardware: HardwareInfo,
    pub display: DisplayInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositorInfo {
    #[serde(rename = "type")]
    pub compositor_type: String,
    pub name: String,
    pub capabilities: CompositorCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositorCapabilities {
    pub layer_shell: bool,
    pub blur: bool,
    pub screencopy: bool,
    pub wallpaper_control: bool,
    pub dpms: bool,
    pub socket_ipc: bool,
    pub http_ipc: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsInfo {
    pub name: String,
    pub kernel: String,
    pub pretty_name: String,
    pub hostname: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    #[serde(rename = "type")]
    pub session_type: String,
    pub user: String,
    pub seat: Option<String>,
    pub home: String,
    pub shell: Option<String>,
    pub terminal: Option<String>,
    pub uptime_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    pub cpu_model: String,
    pub cpu_cores: usize,
    pub memory_total: u64,
    pub gpu: Option<String>,
    pub disk_total: Option<u64>,
    pub disk_used: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayInfo {
    pub monitors: Vec<MonitorInfo>,
    pub scales: std::collections::HashMap<String, f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorInfo {
    pub name: String,
    pub scale: f32,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub refresh_rate: f32,
    pub focused: bool,
    pub active: bool,
}

// === Audio (PipeWire/PulseAudio) ===

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

// === Bluetooth ===

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

// === Network ===

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
    pub existing: bool,
    pub cached: bool,
    pub password_required: bool,
    pub security: String,
    pub frequency_mhz: Option<u32>,
    pub bssid: Option<String>,
    pub last_seen_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveWifiDetails {
    pub ifname: Option<String>,
    pub ssid: Option<String>,
    pub signal: Option<u8>,
    pub frequency_mhz: Option<u32>,
    pub band: Option<String>,
    pub channel: Option<u32>,
    pub rate_mbps: Option<u32>,
    pub ip4: Option<String>,
    pub ip6: Vec<String>,
    pub gateway4: Option<String>,
    pub gateway6: Vec<String>,
    pub dns4: Vec<String>,
    pub dns6: Vec<String>,
    pub security: Option<String>,
    pub bssid: Option<String>,
    pub mac: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EthernetInterface {
    pub ifname: String,
    pub connected: bool,
    pub mac: Option<String>,
    pub ip4: Option<String>,
    pub ip6: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveEthernetDetails {
    pub ifname: String,
    pub speed: Option<String>,
    pub ipv4: Option<String>,
    pub ipv6: Vec<String>,
    pub gateway4: Option<String>,
    pub gateway6: Vec<String>,
    pub dns4: Vec<String>,
    pub dns6: Vec<String>,
    pub mac: Option<String>,
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
    pub wifi_available: bool,
    pub ethernet_available: bool,
    pub mode: NetMode,
    pub active_ssid: Option<String>,
    pub interfaces: Vec<NetInterface>,
}

// ── Hotspot ─────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HotspotBackend {
    NetworkManager,
    Hostapd,
}

impl Default for HotspotBackend {
    fn default() -> Self {
        Self::NetworkManager
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotspotConfig {
    pub ssid: String,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub iface: Option<String>,
    #[serde(default)]
    pub band: Option<String>,
    #[serde(default)]
    pub channel: Option<u32>,
    #[serde(default)]
    pub backend: Option<HotspotBackend>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HotspotClient {
    pub mac: String,
    pub ip: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HotspotStatus {
    pub active: bool,
    pub ssid: Option<String>,
    pub iface: Option<String>,
    pub band: Option<String>,
    pub channel: Option<u32>,
    pub clients: Vec<HotspotClient>,
    #[serde(default)]
    pub backend: HotspotBackend,
    pub supports_virtual_ap: bool,
}

// === Display ===
// ── Brightness ─────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrightnessStatus {
    pub device: String,
    pub current: u64,
    pub max: u64,
    pub percent: f32,
}

// ── Wallpaper ──────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WallpaperBackend {
    Awww,
    Unknown,
}

impl Default for WallpaperBackend {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallpaperMonitorState {
    pub name: String,
    pub current: Option<String>,
    pub transition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallpaperStatus {
    pub backend: WallpaperBackend,
    pub backend_available: bool,
    pub monitors: Vec<WallpaperMonitorState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallpaperSetOptions {
    pub path: String,
    #[serde(default)]
    pub monitor: Option<String>,
    #[serde(default)]
    pub transition: Option<String>,
}

impl WallpaperSetOptions {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            monitor: None,
            transition: None,
        }
    }

    pub fn with_monitor(mut self, monitor: impl Into<String>) -> Self {
        self.monitor = Some(monitor.into());
        self
    }

    pub fn with_transition(mut self, transition: impl Into<String>) -> Self {
        self.transition = Some(transition.into());
        self
    }
}

// === Sysmon ===

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
pub struct NetTraffic {
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_bps: u64,
    pub tx_bps: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuStatus {
    pub name: Option<String>,
    pub temperature_c: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskStatus {
    pub mount: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub filesystem: Option<String>,
}

// === Processes ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: Option<u32>,
    pub name: String,
    pub exe_path: Option<String>,
    pub cpu_percent: f32,
    pub cpu_ticks: Option<f64>,
    pub mem_rss_kb: u64,
    pub status: String,
    pub user: Option<String>,
    pub cmd: Vec<String>,
}