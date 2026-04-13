//! crawl-network: Network management via NetworkManager D-Bus.
//!
//! Talks directly to org.freedesktop.NetworkManager over the system bus.
//! Watches for connectivity changes, active connection updates, and WiFi state.

use crawl_ipc::{
    events::{CrawlEvent, NetEvent},
    types::{NetInterface, NetMode, NetStatus, WifiNetwork},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::Ipv4Addr;
use thiserror::Error;
use tokio::sync::broadcast;
use tokio::time::{interval, Duration};
use tracing::info;
use zbus::{proxy, Connection};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

// ── Config ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// Enable WiFi scanning on startup
    pub wifi_scan_on_start: bool,
}

// ── Error ────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum NetError {
    #[error("D-Bus error: {0}")]
    DBus(#[from] zbus::Error),
    #[error("Zvariant error: {0}")]
    ZVariant(#[from] zbus::zvariant::Error),
    #[error("NetworkManager unavailable")]
    Unavailable,
    #[error("not found: {0}")]
    NotFound(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
}

// ── D-Bus Proxies ─────────────────────────────────────────────────────────────

/// Minimal proxy for org.freedesktop.NetworkManager
#[proxy(
    interface = "org.freedesktop.NetworkManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager"
)]
trait NetworkManager {
    #[zbus(property)]
    fn connectivity(&self) -> zbus::Result<u32>;

    #[zbus(property)]
    fn wireless_enabled(&self) -> zbus::Result<bool>;


    #[zbus(property)]
    fn networking_enabled(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn set_networking_enabled(&self, enabled: bool) -> zbus::Result<()>;

    #[zbus(property)]
    fn active_connections(&self) -> zbus::Result<Vec<zbus::zvariant::OwnedObjectPath>>;

    fn get_devices(&self) -> zbus::Result<Vec<zbus::zvariant::OwnedObjectPath>>;

    fn add_and_activate_connection(
        &self,
        connection: HashMap<String, HashMap<String, OwnedValue>>,
        device: zbus::zvariant::OwnedObjectPath,
        specific_object: zbus::zvariant::OwnedObjectPath,
    ) -> zbus::Result<(zbus::zvariant::OwnedObjectPath, zbus::zvariant::OwnedObjectPath, zbus::zvariant::OwnedObjectPath)>;

    fn deactivate_connection(
        &self,
        active_connection: zbus::zvariant::OwnedObjectPath,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    fn state_changed(&self, state: u32) -> zbus::Result<()>;
}

/// Proxy for a NM Device object
#[proxy(
    interface = "org.freedesktop.NetworkManager.Device",
    default_service = "org.freedesktop.NetworkManager"
)]
trait NMDevice {
    #[zbus(property)]
    fn interface(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;

    #[zbus(property, name = "Ip4Address")]
    fn ip4_address(&self) -> zbus::Result<u32>;

    #[zbus(property, name = "DeviceType")]
    fn device_type(&self) -> zbus::Result<u32>;

    #[zbus(property, name = "HwAddress")]
    fn hw_address(&self) -> zbus::Result<String>;

    #[zbus(property, name = "ActiveConnection")]
    fn active_connection(&self) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.Device.Wireless",
    default_service = "org.freedesktop.NetworkManager"
)]
trait NMDeviceWireless {
    #[zbus(property)]
    fn active_access_point(&self) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;

    #[zbus(property)]
    fn access_points(&self) -> zbus::Result<Vec<zbus::zvariant::OwnedObjectPath>>;

    fn request_scan(&self, options: HashMap<&str, Value<'_>>) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.AccessPoint",
    default_service = "org.freedesktop.NetworkManager"
)]
trait NMAccessPoint {
    #[zbus(property)]
    fn ssid(&self) -> zbus::Result<Vec<u8>>;

    #[zbus(property)]
    fn strength(&self) -> zbus::Result<u8>;

    #[zbus(property)]
    fn flags(&self) -> zbus::Result<u32>;

    #[zbus(property, name = "WpaFlags")]
    fn wpa_flags(&self) -> zbus::Result<u32>;

    #[zbus(property, name = "RsnFlags")]
    fn rsn_flags(&self) -> zbus::Result<u32>;
}

// ── Domain runner ─────────────────────────────────────────────────────────────

pub async fn run(cfg: Config, tx: broadcast::Sender<CrawlEvent>) -> anyhow::Result<()> {
    info!("crawl-network starting");

    let conn = Connection::system().await?;
    let nm = NetworkManagerProxy::new(&conn).await?;
    let mut state = NetworkState::new();

    if cfg.wifi_scan_on_start {
        let _ = request_wifi_scan(&conn, &nm).await;
    }

    let mut ticker = interval(Duration::from_secs(3));
    loop {
        ticker.tick().await;
        if let Ok(snapshot) = refresh_snapshot(&conn, &nm).await {
            info!("network status: connectivity={}", snapshot.status.connectivity);
            for evt in state.diff_events(&snapshot) {
                let _ = tx.send(CrawlEvent::Network(evt));
            }
            state.set_snapshot(snapshot);
        }
    }
}

const NM_DEVICE_TYPE_WIFI: u32 = 2;
const NM_DEVICE_TYPE_ETHERNET: u32 = 1;

// ── Connectivity state mapping ────────────────────────────────────────────────

/// Map NM connectivity integer to a human-readable string.
/// 0=unknown, 1=none, 2=portal, 3=limited, 4=full
pub fn nm_connectivity_str(state: u32) -> &'static str {
    match state {
        4 => "full",
        3 => "limited",
        2 => "portal",
        1 => "none",
        _ => "unknown",
    }
}

/// Map NM device state integer to string.
/// 100=activated, 70=ip-config, 50=prepare, 30=disconnected, 20=unavailable
pub fn nm_device_state_str(state: u32) -> &'static str {
    match state {
        100 => "activated",
        90  => "secondaries",
        80  => "ip-check",
        70  => "ip-config",
        60  => "need-auth",
        50  => "config",
        40  => "prepare",
        30  => "disconnected",
        20  => "unavailable",
        10  => "unmanaged",
        _   => "unknown",
    }
}

// ── Public query API ──────────────────────────────────────────────────────────

pub async fn get_status() -> Result<NetStatus, NetError> {
    let conn = Connection::system().await?;
    let nm = NetworkManagerProxy::new(&conn).await?;
    let snapshot = refresh_snapshot(&conn, &nm).await?;
    Ok(snapshot.status)
}

pub async fn list_wifi() -> Result<Vec<WifiNetwork>, NetError> {
    let conn = Connection::system().await?;
    let nm = NetworkManagerProxy::new(&conn).await?;
    let mut seen: HashMap<String, WifiNetwork> = HashMap::new();

    for path in nm.get_devices().await? {
        let dev = NMDeviceProxy::builder(&conn).path(path.clone())?.build().await?;
        if dev.device_type().await.unwrap_or(0) != NM_DEVICE_TYPE_WIFI {
            continue;
        }

        let wifi = NMDeviceWirelessProxy::builder(&conn).path(path)?.build().await?;
        let active_ap = wifi.active_access_point().await.ok();
        let aps = wifi.access_points().await.unwrap_or_default();

        for ap_path in aps {
            let ap = NMAccessPointProxy::builder(&conn).path(ap_path.clone())?.build().await?;
            let ssid = ssid_to_string(ap.ssid().await.unwrap_or_default());
            if ssid.is_empty() {
                continue;
            }
            let signal = ap.strength().await.unwrap_or(0);
            let secured = ap.flags().await.unwrap_or(0) != 0
                || ap.wpa_flags().await.unwrap_or(0) != 0
                || ap.rsn_flags().await.unwrap_or(0) != 0;
            let connected = active_ap.as_ref().map(|p| p == &ap_path).unwrap_or(false);

            let entry = WifiNetwork { ssid: ssid.clone(), signal, secured, connected };
            match seen.get(&ssid) {
                Some(existing) if existing.connected => {
                    if connected && signal > existing.signal {
                        seen.insert(ssid, entry);
                    }
                }
                Some(existing) if connected && !existing.connected => {
                    seen.insert(ssid, entry);
                }
                Some(existing) if existing.signal >= signal => {}
                _ => {
                    seen.insert(ssid, entry);
                }
            }
        }
    }

    let mut list: Vec<WifiNetwork> = seen.into_values().collect();
    list.sort_by(|a, b| b.signal.cmp(&a.signal));
    Ok(list)
}

pub async fn scan_wifi() -> Result<(), NetError> {
    let conn = Connection::system().await?;
    let nm = NetworkManagerProxy::new(&conn).await?;
    request_wifi_scan(&conn, &nm).await
}

pub async fn connect_wifi(_ssid: &str, _password: Option<&str>) -> Result<(), NetError> {
    let conn = Connection::system().await?;
    let nm = NetworkManagerProxy::new(&conn).await?;

    let devices = nm.get_devices().await?;
    let wifi_device = find_wifi_device(&conn, &devices).await
        .ok_or_else(|| NetError::NotFound("wifi device".into()))?;

    // TODO(crawl-network): Optionally request a scan before searching access points.
    let ap_path = find_wifi_ap(&conn, &wifi_device, _ssid).await
        .ok_or_else(|| NetError::NotFound(format!("ssid '{_ssid}'")))?;

    let settings = build_wifi_settings(_ssid, _password);
    nm.add_and_activate_connection(settings, wifi_device, ap_path).await?;
    Ok(())
}

pub async fn disconnect_wifi() -> Result<(), NetError> {
    let conn = Connection::system().await?;
    let nm = NetworkManagerProxy::new(&conn).await?;

    let devices = nm.get_devices().await?;
    let wifi_device = find_active_wifi_device(&conn, &devices).await
        .ok_or_else(|| NetError::NotFound("active wifi connection".into()))?;

    let dev = NMDeviceProxy::builder(&conn).path(wifi_device)?.build().await?;
    let active = dev.active_connection().await?;
    if active.as_str() == "/" {
        return Err(NetError::NotFound("active wifi connection".into()));
    }

    nm.deactivate_connection(active).await?;
    Ok(())
}

pub async fn set_network_enabled(enabled: bool) -> Result<(), NetError> {
    let conn = Connection::system().await?;
    let nm = NetworkManagerProxy::new(&conn).await?;
    nm.set_networking_enabled(enabled).await?;
    Ok(())
}

pub async fn connect_ethernet(interface: Option<&str>) -> Result<String, NetError> {
    let conn = Connection::system().await?;
    let nm = NetworkManagerProxy::new(&conn).await?;

    let devices = nm.get_devices().await?;
    let (eth_device, iface_name) = match interface {
        Some(name) => (
            find_ethernet_device(&conn, &devices, name).await
                .ok_or_else(|| NetError::NotFound(format!("ethernet interface '{name}'")))?,
            name.to_string(),
        ),
        None => find_first_ethernet_device(&conn, &devices).await
            .ok_or_else(|| NetError::NotFound("ethernet device".into()))?,
    };

    let settings = build_ethernet_settings(&iface_name);
    let root = OwnedObjectPath::try_from("/")?;
    nm.add_and_activate_connection(settings, eth_device, root).await?;
    Ok(iface_name)
}

pub async fn disconnect_ethernet(interface: Option<&str>) -> Result<String, NetError> {
    let conn = Connection::system().await?;
    let nm = NetworkManagerProxy::new(&conn).await?;
    let devices = nm.get_devices().await?;
    let (eth_device, iface_name) = match interface {
        Some(name) => (
            find_ethernet_device(&conn, &devices, name).await
                .ok_or_else(|| NetError::NotFound(format!("ethernet interface '{name}'")))?,
            name.to_string(),
        ),
        None => find_active_ethernet_device(&conn, &devices).await
            .ok_or_else(|| NetError::NotFound("active ethernet device".into()))?,
    };

    let dev = NMDeviceProxy::builder(&conn).path(eth_device)?.build().await?;
    let active = dev.active_connection().await?;
    if active.as_str() == "/" {
        return Err(NetError::NotFound(format!("no active connection for '{iface_name}'")));
    }
    nm.deactivate_connection(active).await?;
    Ok(iface_name)
}

fn ssid_to_string(bytes: Vec<u8>) -> String {
    String::from_utf8_lossy(&bytes).trim_matches(char::from(0)).to_string()
}

#[derive(Clone)]
struct NetworkSnapshot {
    status: NetStatus,
}

struct NetworkState {
    last: Option<NetworkSnapshot>,
}

impl NetworkState {
    fn new() -> Self {
        Self { last: None }
    }

    fn set_snapshot(&mut self, snapshot: NetworkSnapshot) {
        self.last = Some(snapshot);
    }

    fn diff_events(&self, snapshot: &NetworkSnapshot) -> Vec<NetEvent> {
        let mut events = Vec::new();
        let status = &snapshot.status;

        if let Some(prev) = &self.last {
            let prev_status = &prev.status;
            if prev_status.connectivity != status.connectivity {
                events.push(NetEvent::ConnectivityChanged { state: status.connectivity.clone() });
            }
            if prev_status.wifi_enabled != status.wifi_enabled {
                events.push(if status.wifi_enabled {
                    NetEvent::WifiEnabled
                } else {
                    NetEvent::WifiDisabled
                });
            }
            if prev_status.mode != status.mode {
                events.push(NetEvent::ModeChanged { mode: status.mode.clone() });
            }
        } else {
            events.push(NetEvent::ConnectivityChanged { state: status.connectivity.clone() });
            events.push(if status.wifi_enabled { NetEvent::WifiEnabled } else { NetEvent::WifiDisabled });
            events.push(NetEvent::ModeChanged { mode: status.mode.clone() });
        }

        if let Some(ssid) = status.active_ssid.clone() {
            if let Some(iface) = status.interfaces.first().map(|i| i.name.clone()) {
                events.push(NetEvent::Connected { ssid: Some(ssid), iface });
            }
        }

        events
    }
}

async fn refresh_snapshot(
    conn: &Connection,
    nm: &NetworkManagerProxy<'_>,
) -> Result<NetworkSnapshot, NetError> {
    let connectivity = nm_connectivity_str(nm.connectivity().await?).to_string();
    // TODO(crawl-network): Log D-Bus property read failures instead of silent defaults.
    let wifi_enabled = nm.wireless_enabled().await.unwrap_or(false);
    let network_enabled = nm.networking_enabled().await.unwrap_or(true);

    let mut active_ssid = None;
    let mut interfaces = Vec::new();
    let mut mode = NetMode::Unknown;

    for path in nm.get_devices().await? {
        let dev = NMDeviceProxy::builder(conn).path(path.clone())?.build().await?;
        let iface = dev.interface().await.unwrap_or_default();
        let state = nm_device_state_str(dev.state().await.unwrap_or_default()).to_string();
        let ip4_raw = dev.ip4_address().await.unwrap_or(0);
        let ip4 = if ip4_raw == 0 { None } else { Some(Ipv4Addr::from(ip4_raw).to_string()) };
        let mac = dev.hw_address().await.ok();
        let device_type = dev.device_type().await.unwrap_or(0);

        if device_type == NM_DEVICE_TYPE_WIFI || device_type == NM_DEVICE_TYPE_ETHERNET {
            interfaces.push(NetInterface {
                name: iface.clone(),
                state,
                ip4,
                ip6: None,
                mac,
            });
        }

        if device_type == NM_DEVICE_TYPE_WIFI {
            if let Ok(wifi) = NMDeviceWirelessProxy::builder(conn).path(path)?.build().await {
                if let Ok(active_ap) = wifi.active_access_point().await {
                    if let Ok(ap) = NMAccessPointProxy::builder(conn).path(active_ap)?.build().await {
                        let ssid = ssid_to_string(ap.ssid().await.unwrap_or_default());
                        if !ssid.is_empty() {
                            active_ssid = Some(ssid);
                            mode = NetMode::Station;
                        }
                    }
                }
            }
        }
    }

    Ok(NetworkSnapshot {
        status: NetStatus {
            connectivity,
            wifi_enabled,
            network_enabled,
            mode,
            active_ssid,
            interfaces,
        },
    })
}

async fn request_wifi_scan(conn: &Connection, nm: &NetworkManagerProxy<'_>) -> Result<(), NetError> {
    for path in nm.get_devices().await? {
        let dev = NMDeviceProxy::builder(conn).path(path.clone())?.build().await?;
        if dev.device_type().await.unwrap_or(0) != NM_DEVICE_TYPE_WIFI {
            continue;
        }
        let wifi = NMDeviceWirelessProxy::builder(conn).path(path)?.build().await?;
        let options: HashMap<&str, Value<'_>> = HashMap::new();
        let _ = wifi.request_scan(options).await;
    }
    Ok(())
}

fn build_wifi_settings(ssid: &str, password: Option<&str>) -> HashMap<String, HashMap<String, OwnedValue>> {
    let mut connection = HashMap::new();
    connection.insert("id".to_string(), owned_value(ssid.to_string()));
    connection.insert("type".to_string(), owned_value("802-11-wireless".to_string()));
    connection.insert("autoconnect".to_string(), owned_value(true));

    let mut wifi = HashMap::new();
    wifi.insert("ssid".to_string(), owned_value(ssid.as_bytes().to_vec()));
    wifi.insert("mode".to_string(), owned_value("infrastructure".to_string()));

    let mut ipv4 = HashMap::new();
    ipv4.insert("method".to_string(), owned_value("auto".to_string()));

    let mut ipv6 = HashMap::new();
    ipv6.insert("method".to_string(), owned_value("auto".to_string()));

    let mut settings = HashMap::new();
    settings.insert("connection".to_string(), connection);
    settings.insert("802-11-wireless".to_string(), wifi);
    settings.insert("ipv4".to_string(), ipv4);
    settings.insert("ipv6".to_string(), ipv6);

    if let Some(psk) = password {
        let mut security = HashMap::new();
        security.insert("key-mgmt".to_string(), owned_value("wpa-psk".to_string()));
        security.insert("psk".to_string(), owned_value(psk.to_string()));
        settings.insert("802-11-wireless-security".to_string(), security);
    }

    settings
}

fn build_ethernet_settings(interface: &str) -> HashMap<String, HashMap<String, OwnedValue>> {
    let mut connection = HashMap::new();
    connection.insert("id".to_string(), owned_value(interface.to_string()));
    connection.insert("type".to_string(), owned_value("802-3-ethernet".to_string()));
    connection.insert("autoconnect".to_string(), owned_value(true));

    let ethernet = HashMap::new();
    let mut ipv4 = HashMap::new();
    ipv4.insert("method".to_string(), owned_value("auto".to_string()));
    let mut ipv6 = HashMap::new();
    ipv6.insert("method".to_string(), owned_value("auto".to_string()));

    let mut settings = HashMap::new();
    settings.insert("connection".to_string(), connection);
    settings.insert("802-3-ethernet".to_string(), ethernet);
    settings.insert("ipv4".to_string(), ipv4);
    settings.insert("ipv6".to_string(), ipv6);
    settings
}

fn owned_value<V>(value: V) -> OwnedValue
where
    V: Into<Value<'static>>,
{
    OwnedValue::try_from(value.into())
        .expect("owned value conversion should not fail")
}

async fn find_wifi_device(
    conn: &Connection,
    devices: &[OwnedObjectPath],
) -> Option<OwnedObjectPath> {
    for path in devices {
        let dev = NMDeviceProxy::builder(conn).path(path.clone()).ok()?.build().await.ok()?;
        if dev.device_type().await.ok()? == NM_DEVICE_TYPE_WIFI {
            return Some(path.clone());
        }
    }
    None
}

async fn find_ethernet_device(
    conn: &Connection,
    devices: &[OwnedObjectPath],
    interface: &str,
) -> Option<OwnedObjectPath> {
    for path in devices {
        let dev = NMDeviceProxy::builder(conn).path(path.clone()).ok()?.build().await.ok()?;
        if dev.device_type().await.ok()? != NM_DEVICE_TYPE_ETHERNET {
            continue;
        }
        let name = dev.interface().await.ok()?;
        if name == interface {
            return Some(path.clone());
        }
    }
    None
}

async fn find_first_ethernet_device(
    conn: &Connection,
    devices: &[OwnedObjectPath],
) -> Option<(OwnedObjectPath, String)> {
    for path in devices {
        let dev = NMDeviceProxy::builder(conn).path(path.clone()).ok()?.build().await.ok()?;
        if dev.device_type().await.ok()? != NM_DEVICE_TYPE_ETHERNET {
            continue;
        }
        let name = dev.interface().await.ok()?;
        return Some((path.clone(), name));
    }
    None
}

async fn find_active_ethernet_device(
    conn: &Connection,
    devices: &[OwnedObjectPath],
) -> Option<(OwnedObjectPath, String)> {
    for path in devices {
        let dev = NMDeviceProxy::builder(conn).path(path.clone()).ok()?.build().await.ok()?;
        if dev.device_type().await.ok()? != NM_DEVICE_TYPE_ETHERNET {
            continue;
        }
        let active = dev.active_connection().await.ok()?;
        if active.as_str() != "/" {
            let name = dev.interface().await.ok()?;
            return Some((path.clone(), name));
        }
    }
    None
}

async fn find_active_wifi_device(
    conn: &Connection,
    devices: &[OwnedObjectPath],
) -> Option<OwnedObjectPath> {
    for path in devices {
        let dev = NMDeviceProxy::builder(conn).path(path.clone()).ok()?.build().await.ok()?;
        if dev.device_type().await.ok()? != NM_DEVICE_TYPE_WIFI {
            continue;
        }
        let active = dev.active_connection().await.ok()?;
        if active.as_str() != "/" {
            return Some(path.clone());
        }
    }
    None
}

async fn find_wifi_ap(
    conn: &Connection,
    wifi_device: &OwnedObjectPath,
    ssid: &str,
) -> Option<OwnedObjectPath> {
    let wifi = NMDeviceWirelessProxy::builder(conn).path(wifi_device.clone()).ok()?.build().await.ok()?;
    let aps = wifi.access_points().await.ok()?;
    for ap_path in aps {
        let ap = NMAccessPointProxy::builder(conn).path(ap_path.clone()).ok()?.build().await.ok()?;
        let ap_ssid = ssid_to_string(ap.ssid().await.unwrap_or_default());
        if ap_ssid == ssid {
            return Some(ap_path);
        }
    }
    None
}
