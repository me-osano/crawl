# Network

Network management via NetworkManager D-Bus. Watches for connectivity changes, active connection updates, and WiFi state.

## Architecture

```
crawl-network/
├── lib.rs              # Public API + domain runner
├── config.rs           # Configuration
├── dbus.rs             # D-Bus proxy types for NetworkManager
├── wifi.rs             # WiFi scanning, connection, management
├── ethernet.rs         # Ethernet interface management
├── hotspot.rs          # WiFi hotspot creation/management
├── state.rs            # Network state tracking
└── sysfs.rs            # Sysfs operations
```

## Configuration

Location: `~/.config/crawl/config.toml` (or `$XDG_CONFIG_HOME/crawl/config.toml`)

```toml
[network]
auto_enable = false                          # Enable networking and WiFi on startup
wifi_scan_on_start = false                   # Scan for WiFi networks on daemon start
wifi_scan_finish_delay_ms = 30000            # Delay after scan before reading results (ms)
hotspot_backend = "networkmanager"            # networkmanager | hostapd
hotspot_virtual_iface = true                 # Use virtual interface for hotspot
```

Environment variables (prefix `CRAWL_NETWORK__`):

```bash
CRAWL_NETWORK__AUTO_ENABLE=true
CRAWL_NETWORK__WIFI_SCAN_ON_START=true
CRAWL_NETWORK__WIFI_SCAN_FINISH_DELAY_MS=30000
CRAWL_NETWORK__HOTSPOT_BACKEND=networkmanager
CRAWL_NETWORK__HOTSPOT_VIRTUAL_IFACE=true
```

## Features

- **Network Status**: Connectivity state (full/limited/portal/none), active SSID, interfaces
- **WiFi Management**: Scan, list, connect, disconnect, signal strength
- **Ethernet**: List interfaces, connection details, speed
- **Hotspot**: Create/stop WiFi hotspot via NetworkManager or hostapd
- **Auto-Enable**: Optionally enable networking and WiFi on daemon start
- **Event-Driven**: Emits events on status changes, scans, connections

## Refresh Intervals

| Task | Interval | Description |
|------|-----------|-------------|
| Fast refresh | 5s | Snapshot + ethernet + interfaces |
| Slow refresh | 30s | WiFi scan + hotspot status |

## IPC Types

```rust
pub struct NetStatus {
    pub connectivity: String,         // "full" | "limited" | "portal" | "none" | "unknown"
    pub wifi_enabled: bool,
    pub network_enabled: bool,
    pub wifi_available: bool,
    pub ethernet_available: bool,
    pub mode: NetMode,                // Station | Ap | Unknown
    pub active_ssid: Option<String>,
    pub interfaces: Vec<NetInterface>,
}

pub struct NetInterface {
    pub name: String,
    pub state: String,
    pub ip4: Option<String>,
    pub ip6: Option<String>,
    pub mac: Option<String>,
}

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

pub struct EthernetInterface {
    pub ifname: String,
    pub connected: bool,
    pub mac: Option<String>,
    pub ip4: Option<String>,
    pub ip6: Vec<String>,
}

pub struct HotspotConfig {
    pub ssid: String,
    pub password: Option<String>,
    pub iface: Option<String>,
    pub band: Option<String>,
    pub channel: Option<u32>,
    pub backend: Option<HotspotBackend>,  // NetworkManager | Hostapd
}

pub struct HotspotStatus {
    pub active: bool,
    pub ssid: Option<String>,
    pub iface: Option<String>,
    pub band: Option<String>,
    pub channel: Option<u32>,
    pub clients: Vec<HotspotClient>,
    pub backend: HotspotBackend,
    pub supports_virtual_ap: bool,
}

pub enum NetMode {
    Station,
    Ap,
    Unknown,
}

pub enum HotspotBackend {
    NetworkManager,
    Hostapd,
}
```

## Events Emitted

```rust
pub enum NetEvent {
    // Connection
    Connected { ssid: Option<String>, iface: String },
    Disconnected { iface: String },
    IpChanged { iface: String, ip: String },

    // WiFi
    WifiEnabled,
    WifiDisabled,
    WifiScanStarted,
    WifiScanFinished,
    WifiListUpdated { networks: Vec<WifiNetwork> },
    ActiveWifiDetailsChanged { details: ActiveWifiDetails },

    // Ethernet
    EthernetInterfacesChanged { interfaces: Vec<EthernetInterface> },
    ActiveEthernetDetailsChanged { details: ActiveEthernetDetails },

    // State
    ModeChanged { mode: NetMode },
    ConnectivityChanged { state: String },

    // Hotspot
    HotspotStarted { status: HotspotStatus },
    HotspotStopped,
    HotspotStatusChanged { status: HotspotStatus },
    HotspotClientJoined { client: HotspotClient },
    HotspotClientLeft { mac: String },
}
```

## Public API

```rust
// Status and info
pub async fn get_status() -> Result<NetStatus, NetError>
pub async fn get_wifi_details() -> Result<ActiveWifiDetails, NetError>
pub async fn list_wifi() -> Result<Vec<WifiNetwork>, NetError>

// WiFi controls
pub async fn scan_wifi() -> Result<(), NetError>
pub async fn connect_wifi(ssid: &str, password: Option<&str>) -> Result<(), NetError>
pub async fn disconnect_wifi() -> Result<(), NetError>
pub async fn delete_wifi_connection(ssid: &str) -> Result<(), NetError>

// Network controls
pub async fn set_network_enabled(enabled: bool) -> Result<(), NetError>

// Hotspot
pub async fn start_hotspot(config: &HotspotConfig, use_virtual_iface: bool) -> Result<HotspotStatus, NetError>
pub async fn stop_hotspot() -> Result<(), NetError>
pub async fn hotspot_status() -> Result<HotspotStatus, NetError>

// Ethernet
pub fn list_ethernet() -> Result<Vec<EthernetInterface>, NetError>
pub fn connect_ethernet(iface: &str) -> Result<(), NetError>
pub fn disconnect_ethernet(iface: &str) -> Result<(), NetError>
pub fn get_ethernet_details(iface: &str) -> Result<ActiveEthernetDetails, NetError>
```

## Device State Strings

| Code | State |
|------|-------|
| 100 | activated |
| 90 | secondaries |
| 80 | ip-check |
| 70 | ip-config |
| 60 | need-auth |
| 50 | config |
| 40 | prepare |
| 30 | disconnected |
| 20 | unavailable |
| 10 | unmanaged |

## Connectivity States

| Code | State |
|------|-------|
| 4 | full |
| 3 | limited |
| 2 | portal |
| 1 | none |
| _ | unknown |
