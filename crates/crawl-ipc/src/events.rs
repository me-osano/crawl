use serde::{Deserialize, Serialize};
use crate::types::*;

/// All events broadcast over the Unix socket JSON-RPC event stream.
/// Quickshell and CLI --watch consumers filter by domain.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "domain", content = "data", rename_all = "snake_case")]
pub enum CrawlEvent {
    Audio(AudioEvent),
    Bluetooth(BtEvent),
    Brightness(BrightnessEvent),
    Daemon(DaemonEvent),
    Network(NetEvent),
    Proc(ProcEvent),
    Sysmon(SysmonEvent),
    Sysinfo(SysinfoEvent),
    Wallpaper(WallpaperEvent),
}

// ---- Audio ---------
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

// ---- Bluetooth ---------
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

// ---- Network ---------
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum NetEvent {
    Connected { ssid: Option<String>, iface: String },
    Disconnected { iface: String },
    IpChanged { iface: String, ip: String },
    WifiEnabled,
    WifiDisabled,
    WifiScanStarted,
    WifiScanFinished,
    WifiListUpdated { networks: Vec<WifiNetwork> },
    ActiveWifiDetailsChanged { details: ActiveWifiDetails },
    EthernetInterfacesChanged { interfaces: Vec<EthernetInterface> },
    ActiveEthernetDetailsChanged { details: ActiveEthernetDetails },
    ModeChanged { mode: NetMode },
    ConnectivityChanged { state: String },
    HotspotStarted { status: HotspotStatus },
    HotspotStopped,
    HotspotStatusChanged { status: HotspotStatus },
    HotspotClientJoined { client: HotspotClient },
    HotspotClientLeft { mac: String },
}


// ---- Sysinfo -----
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum SysinfoEvent {
    Changed,
}

// ---- Daemon -------
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum DaemonEvent {
    Ready,
    Started,
    Stopping,
    DomainError { domain: String, message: String },
}

// ---- Brightness -----
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum BrightnessEvent {
    Changed { status: BrightnessStatus },
}

// ── Wallpaper ───────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum WallpaperEvent {
    Changed { screen: String, path: String },
    BackendChanged { backend: WallpaperBackend },
    BackendNotAvailable { backend: WallpaperBackend },
    Error { message: String },
}

// --- Sysmon ---
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum SysmonEvent {
    CpuUpdate { cpu: CpuStatus },
    MemUpdate { mem: MemStatus },
    NetUpdate { traffic: NetTraffic },
    GpuUpdate { gpu: GpuStatus },
    CpuSpike { usage: f32, threshold: f32 },
    MemPressure { used_percent: f32 },
}

// --- Processes ---
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
    TopUpdate {
        top_by_cpu: Vec<ProcessInfo>,
        top_by_mem: Vec<ProcessInfo>,
    },
}