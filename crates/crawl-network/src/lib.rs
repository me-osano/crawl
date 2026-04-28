//! crawl-network: Network management via NetworkManager D-Bus.
//!
//! Talks directly to org.freedesktop.NetworkManager over the system bus.
//! Watches for connectivity changes, active connection updates, and WiFi state.

pub mod config;
pub mod dbus;
pub mod ethernet;
pub mod hotspot;
pub mod state;
pub mod sysfs;
pub mod wifi;

pub use config::Config;
pub use crawl_ipc::{events::{CrawlEvent, NetEvent}, types::*};
pub use crawl_scheduler::{Scheduler, ShouldRun};
pub use serde::{Deserialize, Serialize};
pub use std::collections::HashMap;
pub use std::net::Ipv4Addr;
pub use thiserror::Error;
pub use tokio::sync::broadcast;
pub use tokio::time::{interval, Duration};
pub use tracing::{info, warn};

use dbus::{
    NMAccessPointProxy, NMDeviceProxy, NMDeviceWirelessProxy,
    NetworkManagerProxy, NMSettingsProxy, NMSettingsConnectionProxy, NM_DEVICE_TYPE_ETHERNET, NM_DEVICE_TYPE_WIFI,
};
use state::{NetworkSnapshot, NetworkState};

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

pub async fn run(cfg: Config, tx: broadcast::Sender<CrawlEvent>) -> anyhow::Result<()> {
    run_with_scheduler(cfg, tx, None).await
}

pub async fn run_with_scheduler(
    cfg: Config,
    tx: broadcast::Sender<CrawlEvent>,
    scheduler: Option<Scheduler>,
) -> anyhow::Result<()> {
    let task_id_fast = "network_fast";
    let task_id_slow = "network_slow";

    if let Some(ref sched) = scheduler {
        sched.register(task_id_fast, Duration::from_secs(5), 0.1).await;
        sched.register(task_id_slow, Duration::from_secs(30), 0.15).await;
    }

    info!("crawl-network starting (scheduler={})", scheduler.is_some());

    let conn = Connection::system().await?;
    let nm = NetworkManagerProxy::new(&conn).await?;
    let mut network_state = NetworkState::new();
    let mut cache = wifi::NetworkCache::new();

    if cfg.auto_enable {
        let _ = nm.set_networking_enabled(true).await;
        let _ = nm.set_wireless_enabled(true).await;
        info!("auto-enabled networking and wifi");
    }

    if cfg.wifi_scan_on_start {
        let _ = wifi::request_wifi_scan(&conn, &nm).await;
    }

    let mut fast_ticker = interval(Duration::from_secs(5));
    let mut slow_ticker = interval(Duration::from_secs(30));
    let wifi_finish_delay = Duration::from_millis(cfg.wifi_scan_finish_delay_ms.max(200));
    let mut last_hotspot_status: Option<HotspotStatus> = None;

    fast_ticker.tick().await;
    slow_ticker.tick().await;

    loop {
        tokio::select! {
            _ = fast_ticker.tick() => {
                if let Some(ref sched) = scheduler {
                    if sched.should_run(task_id_fast).await == ShouldRun::No {
                        continue;
                    }
                    sched.mark_ran(task_id_fast).await;
                }
                // Fast refresh: snapshot + ethernet + interfaces (5s)
                if let Ok(snapshot) = refresh_snapshot(&conn, &nm).await {
                    info!("network status: connectivity={}", snapshot.status.connectivity);
                    for evt in network_state.diff_events(&snapshot) {
                        let _ = tx.send(CrawlEvent::Network(evt));
                    }
                    network_state.set_snapshot(snapshot);
                }

                // Combined ethernet + interfaces (was separate eth_refresh)
                if let Ok(interfaces) = ethernet::list_ethernet_interfaces(&conn, &nm).await {
                    let _ = tx.send(CrawlEvent::Network(NetEvent::EthernetInterfacesChanged { interfaces }));
                }
                if let Ok(details) = ethernet::refresh_ethernet_details(&conn, &nm, None).await {
                    let _ = tx.send(CrawlEvent::Network(NetEvent::ActiveEthernetDetailsChanged { details }));
                }
            }
            _ = slow_ticker.tick() => {
                if let Some(ref sched) = scheduler {
                    if sched.should_run(task_id_slow).await == ShouldRun::No {
                        continue;
                    }
                    sched.mark_ran(task_id_slow).await;
                }
                // Slow refresh: WiFi + hotspot (30s)

                // WiFi list with scan
                let _ = tx.send(CrawlEvent::Network(NetEvent::WifiScanStarted));
                let _ = wifi::request_wifi_scan(&conn, &nm).await;
                tokio::time::sleep(wifi_finish_delay).await;
                if let Ok(wifi) = wifi::list_wifi_with_cache(&conn, &nm, &mut cache).await {
                    let _ = tx.send(CrawlEvent::Network(NetEvent::WifiListUpdated { networks: wifi }));
                }
                let _ = tx.send(CrawlEvent::Network(NetEvent::WifiScanFinished));

                // WiFi details (was in wifi_details_refresh)
                if let Ok(details) = wifi::refresh_wifi_details_with_cache(&conn, &nm, &mut cache).await {
                    let _ = tx.send(CrawlEvent::Network(NetEvent::ActiveWifiDetailsChanged { details }));
                }

                // Hotspot status (10s -> 30s, less critical)
                if let Ok(status) = hotspot::hotspot_status().await {
                    let prev = last_hotspot_status.clone();
                    last_hotspot_status = Some(status.clone());

                    if !status.active {
                        if prev.as_ref().map(|p| p.active).unwrap_or(false) {
                            let _ = tx.send(CrawlEvent::Network(NetEvent::HotspotStopped));
                        }
                    } else {
                        if prev.is_none() || !prev.as_ref().unwrap().active {
                            let _ = tx.send(CrawlEvent::Network(NetEvent::HotspotStarted { status: status.clone() }));
                        }

                        let prev_clients: std::collections::HashSet<_> = prev
                            .as_ref()
                            .map(|p| p.clients.iter().map(|c| c.mac.clone()).collect())
                            .unwrap_or_default();
                        let curr_clients: std::collections::HashSet<_> = status.clients.iter().map(|c| c.mac.clone()).collect();

                        for mac in curr_clients.difference(&prev_clients) {
                            if let Some(client) = status.clients.iter().find(|c| &c.mac == mac) {
                                let _ = tx.send(CrawlEvent::Network(NetEvent::HotspotClientJoined { client: client.clone() }));
                            }
                        }
                        for mac in prev_clients.difference(&curr_clients) {
                            let _ = tx.send(CrawlEvent::Network(NetEvent::HotspotClientLeft { mac: mac.clone() }));
                        }

                        if status != *prev.as_ref().unwrap_or(&HotspotStatus {
                            active: false, ssid: None, iface: None,
                            band: None, channel: None, clients: Vec::new(),
                            backend: HotspotBackend::default(), supports_virtual_ap: false,
                        }) {
                            let _ = tx.send(CrawlEvent::Network(NetEvent::HotspotStatusChanged { status }));
                        }
                    }
                }
            }
        }
    }
}

pub fn nm_connectivity_str(state: u32) -> &'static str {
    match state {
        4 => "full",
        3 => "limited",
        2 => "portal",
        1 => "none",
        _ => "unknown",
    }
}

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

pub async fn get_status() -> Result<NetStatus, NetError> {
    let conn = Connection::system().await?;
    let nm = NetworkManagerProxy::new(&conn).await?;
    let snapshot = refresh_snapshot(&conn, &nm).await?;
    Ok(snapshot.status)
}

pub async fn get_wifi_details() -> Result<ActiveWifiDetails, NetError> {
    let conn = Connection::system().await?;
    let nm = NetworkManagerProxy::new(&conn).await?;
    let details = wifi::refresh_wifi_details(&conn, &nm).await?;
    Ok(details)
}

pub async fn list_wifi() -> Result<Vec<WifiNetwork>, NetError> {
    let conn = Connection::system().await?;
    let nm = NetworkManagerProxy::new(&conn).await?;
    wifi::list_wifi_from_conn(&conn, &nm).await
}

pub async fn scan_wifi() -> Result<(), NetError> {
    wifi::scan_wifi().await
}

pub async fn connect_wifi(ssid: &str, password: Option<&str>) -> Result<(), NetError> {
    wifi::connect_wifi(ssid, password).await
}

pub async fn disconnect_wifi() -> Result<(), NetError> {
    wifi::disconnect_wifi().await
}

pub async fn set_network_enabled(enabled: bool) -> Result<(), NetError> {
    let conn = Connection::system().await?;
    let nm = NetworkManagerProxy::new(&conn).await?;
    nm.set_networking_enabled(enabled).await?;
    Ok(())
}

pub async fn start_hotspot(config: &HotspotConfig, use_virtual_iface: bool) -> Result<HotspotStatus, NetError> {
    hotspot::start_hotspot(config, use_virtual_iface).await
}

pub async fn stop_hotspot() -> Result<(), NetError> {
    hotspot::stop_hotspot().await
}

pub async fn delete_wifi_connection(ssid: &str) -> Result<(), NetError> {
    let conn = Connection::system().await?;
    let settings = NMSettingsProxy::new(&conn).await?;
    let paths = settings.list_connections().await?;

    for path in paths {
        if let Ok(conn_obj) = NMSettingsConnectionProxy::builder(&conn)
            .path(path.clone())?
            .build()
            .await
        {
            if let Ok(settings_map) = conn_obj.get_settings().await {
                let _ = settings_map;
                return Err(NetError::NotFound(format!("wifi connection '{ssid}' not found")));
            }
        }
    }

    Err(NetError::NotFound(format!("wifi connection '{ssid}' not found")))
}

pub use ethernet::{list_ethernet, connect_ethernet, disconnect_ethernet, get_ethernet_details};
pub use hotspot::hotspot_status;

async fn refresh_snapshot(
    conn: &Connection,
    nm: &NetworkManagerProxy<'_>,
) -> Result<NetworkSnapshot, NetError> {
    let connectivity = nm_connectivity_str(nm.connectivity().await?).to_string();
    let wifi_enabled = nm.wireless_enabled().await.unwrap_or(false);
    let network_enabled = nm.networking_enabled().await.unwrap_or(true);

    let mut active_ssid = None;
    let mut interfaces = Vec::new();
    let mut mode = NetMode::Unknown;
    let mut wifi_available = false;
    let mut ethernet_available = false;

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
            wifi_available = true;
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
        if device_type == NM_DEVICE_TYPE_ETHERNET {
            ethernet_available = true;
        }
    }

    Ok(NetworkSnapshot {
        status: NetStatus {
            connectivity,
            wifi_enabled,
            network_enabled,
            wifi_available,
            ethernet_available,
            mode,
            active_ssid,
            interfaces,
        },
    })
}

fn ssid_to_string(bytes: Vec<u8>) -> String {
    String::from_utf8_lossy(&bytes).trim_matches(char::from(0)).to_string()
}

use zbus::Connection;