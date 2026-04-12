//! crawl-network: Network management via NetworkManager D-Bus.
//!
//! Talks directly to org.freedesktop.NetworkManager over the system bus.
//! Watches for connectivity changes, active connection updates, and WiFi state.

use crawl_ipc::{
    events::CrawlEvent,
    types::{NetStatus, WifiNetwork},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::info;
use zbus::{proxy, Connection};

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
    #[error("NetworkManager unavailable")]
    Unavailable,
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
    fn active_connections(&self) -> zbus::Result<Vec<zbus::zvariant::OwnedObjectPath>>;

    fn get_devices(&self) -> zbus::Result<Vec<zbus::zvariant::OwnedObjectPath>>;

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
}

// ── Domain runner ─────────────────────────────────────────────────────────────

pub async fn run(_cfg: Config, _tx: broadcast::Sender<CrawlEvent>) -> anyhow::Result<()> {
    info!("crawl-network starting");

    let _conn = Connection::system().await?;

    // TODO: Replace with full NetworkManager proxy wiring.
    // Pattern: create NetworkManagerProxy, subscribe to StateChanged signal,
    // map NM connectivity states (4 = full, 3 = limited, etc.) to strings,
    // iterate active connections for SSID, iterate devices for IPs/MACs.

    // Emit initial status
    let status = NetStatus {
        connectivity: "unknown".into(),
        wifi_enabled: false,
        active_ssid: None,
        interfaces: vec![],
    };

    info!("network status: connectivity={}", status.connectivity);

    // TODO: watch NM StateChanged signal and emit NetEvents
    // Example structure:
    //
    // let proxy = NetworkManagerProxy::new(&conn).await?;
    // let mut stream = proxy.receive_state_changed().await?;
    // while let Some(signal) = stream.next().await {
    //     let state = signal.args()?.state;
    //     let evt = map_nm_state(state);
    //     let _ = tx.send(CrawlEvent::Network(evt));
    // }

    std::future::pending::<()>().await;
    Ok(())
}

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
    // TODO: implement via NM proxy
    Ok(NetStatus {
        connectivity: "unknown".into(),
        wifi_enabled: false,
        active_ssid: None,
        interfaces: vec![],
    })
}

pub async fn list_wifi() -> Result<Vec<WifiNetwork>, NetError> {
    // TODO: scan via NM AccessPoint objects
    Ok(vec![])
}

pub async fn connect_wifi(_ssid: &str, _password: Option<&str>) -> Result<(), NetError> {
    // TODO: NM AddAndActivateConnection
    Ok(())
}
