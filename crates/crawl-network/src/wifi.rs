//! WiFi scanning, listing, and connection logic.

use std::collections::HashMap;
use tokio::time::Instant;
use zbus::Connection;

use crate::dbus::{
    NMAccessPointProxy, NMDeviceProxy, NMDeviceWirelessProxy, NMIP4ConfigProxy, NMIP6ConfigProxy,
    NetworkManagerProxy, NMSettingsConnectionProxy, NM_DEVICE_TYPE_WIFI,
};
use crate::NetError;
use crawl_ipc::types::{ActiveWifiDetails, WifiNetwork};

pub struct NetworkCache {
    wifi_list: Option<CachedWifiList>,
    wifi_details: Option<CachedWifiDetails>,
}

impl Default for NetworkCache {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkCache {
    pub fn new() -> Self {
        Self {
            wifi_list: None,
            wifi_details: None,
        }
    }
}

pub struct CachedWifiList {
    data: Vec<WifiNetwork>,
    fetched_at: Instant,
}

pub struct CachedWifiDetails {
    data: ActiveWifiDetails,
    fetched_at: Instant,
}

pub const WIFI_LIST_TTL: tokio::time::Duration = tokio::time::Duration::from_secs(5);
pub const WIFI_DETAILS_TTL: tokio::time::Duration = tokio::time::Duration::from_secs(5);

pub async fn list_wifi_with_cache(
    conn: &Connection,
    nm: &NetworkManagerProxy<'_>,
    cache: &mut NetworkCache,
) -> Result<Vec<WifiNetwork>, NetError> {
    if let Some(entry) = &cache.wifi_list {
        if entry.fetched_at.elapsed() < WIFI_LIST_TTL {
            return Ok(entry.data.clone());
        }
    }
    let list = list_wifi_from_conn(conn, nm).await?;
    cache.wifi_list = Some(CachedWifiList {
        data: list.clone(),
        fetched_at: Instant::now(),
    });
    Ok(list)
}

pub async fn refresh_wifi_details_with_cache(
    conn: &Connection,
    nm: &NetworkManagerProxy<'_>,
    cache: &mut NetworkCache,
) -> Result<ActiveWifiDetails, NetError> {
    if let Some(entry) = &cache.wifi_details {
        if entry.fetched_at.elapsed() < WIFI_DETAILS_TTL {
            return Ok(entry.data.clone());
        }
    }
    let details = refresh_wifi_details(conn, nm).await?;
    cache.wifi_details = Some(CachedWifiDetails {
        data: details.clone(),
        fetched_at: Instant::now(),
    });
    Ok(details)
}

pub async fn list_wifi_from_conn(
    conn: &Connection,
    nm: &NetworkManagerProxy<'_>,
) -> Result<Vec<WifiNetwork>, NetError> {
    let mut seen: HashMap<String, WifiNetwork> = HashMap::new();

    let existing = list_known_wifi_ssids(conn, nm).await.unwrap_or_default();

    for path in nm.get_devices().await? {
        let dev = NMDeviceProxy::builder(conn).path(path.clone())?.build().await?;
        if dev.device_type().await.unwrap_or(0) != NM_DEVICE_TYPE_WIFI {
            continue;
        }

        let wifi = NMDeviceWirelessProxy::builder(conn).path(path.clone())?.build().await?;
        let active_ap = wifi.active_access_point().await.ok();
        let aps = wifi.access_points().await.unwrap_or_default();

        for ap_path in aps {
            let ap = NMAccessPointProxy::builder(conn).path(ap_path.clone())?.build().await?;
            let ssid = ssid_to_string(ap.ssid().await.unwrap_or_default());
            if ssid.is_empty() {
                continue;
            }
            let signal = ap.strength().await.unwrap_or(0);
            let secured = ap.flags().await.unwrap_or(0) != 0
                || ap.wpa_flags().await.unwrap_or(0) != 0
                || ap.rsn_flags().await.unwrap_or(0) != 0;
            let connected = active_ap.as_ref().map(|p| p == &ap_path).unwrap_or(false);

            let frequency_mhz = ap.frequency().await.ok();
            let bssid = ap.hw_address().await.ok();
            let security = wifi_security_label(
                ap.flags().await.unwrap_or(0),
                ap.wpa_flags().await.unwrap_or(0),
                ap.rsn_flags().await.unwrap_or(0),
            );
            let password_required = secured;
            let is_existing = existing.contains(&ssid);

            let entry = WifiNetwork {
                ssid: ssid.clone(),
                signal,
                secured,
                connected,
                existing: is_existing,
                cached: !is_existing,
                password_required,
                security,
                frequency_mhz,
                bssid,
                last_seen_ms: None,
            };

            let needs_update = match seen.get(&ssid) {
                Some(existing) => {
                    if existing.connected {
                        false
                    } else if entry.connected {
                        true
                    } else {
                        entry.signal > existing.signal
                    }
                }
                None => true,
            };
            if needs_update {
                seen.insert(ssid, entry);
            }
        }
    }

    let mut out: Vec<WifiNetwork> = seen.into_values().collect();
    out.sort_by(|a, b| b.signal.cmp(&a.signal));
    Ok(out)
}

pub async fn request_wifi_scan(conn: &Connection, nm: &NetworkManagerProxy<'_>) -> Result<(), NetError> {
    for path in nm.get_devices().await? {
        let dev = NMDeviceProxy::builder(conn).path(path.clone())?.build().await?;
        if dev.device_type().await.unwrap_or(0) != NM_DEVICE_TYPE_WIFI {
            continue;
        }
        let wifi = NMDeviceWirelessProxy::builder(conn).path(path)?.build().await?;
        let options: HashMap<&str, zbus::zvariant::Value<'_>> = HashMap::new();
        let _ = wifi.request_scan(options).await;
    }
    Ok(())
}

pub async fn scan_wifi() -> Result<(), NetError> {
    let conn = Connection::system().await?;
    let nm = NetworkManagerProxy::new(&conn).await?;
    request_wifi_scan(&conn, &nm).await
}

pub async fn connect_wifi(_ssid: &str, _password: Option<&str>) -> Result<(), NetError> {
    let conn = Connection::system().await?;
    let nm = NetworkManagerProxy::new(&conn).await?;

    if !nm.wireless_enabled().await.unwrap_or(false) {
        return Err(NetError::Unsupported("wifi disabled".into()));
    }

    let devices = nm.get_devices().await?;
    let wifi_device = find_wifi_device(&conn, &devices).await
        .ok_or_else(|| NetError::NotFound("wifi device".into()))?;

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

pub async fn refresh_wifi_details(
    conn: &Connection,
    nm: &NetworkManagerProxy<'_>,
) -> Result<ActiveWifiDetails, NetError> {
    let devices = nm.get_devices().await?;
    let wifi_device = find_active_wifi_device(conn, &devices).await
        .ok_or_else(|| NetError::NotFound("active wifi connection".into()))?;

    let dev = NMDeviceProxy::builder(conn).path(wifi_device.clone())?.build().await?;
    let iface = dev.interface().await.unwrap_or_default();

    let mut details = ActiveWifiDetails {
        ifname: if iface.is_empty() { None } else { Some(iface.clone()) },
        ssid: None,
        signal: None,
        frequency_mhz: None,
        band: None,
        channel: None,
        rate_mbps: None,
        ip4: None,
        ip6: Vec::new(),
        gateway4: None,
        gateway6: Vec::new(),
        dns4: Vec::new(),
        dns6: Vec::new(),
        security: None,
        bssid: None,
        mac: None,
    };

    if let Ok(wifi) = NMDeviceWirelessProxy::builder(conn).path(wifi_device.clone())?.build().await {
        details.mac = wifi.hw_address().await.ok();
        if let Ok(active_ap) = wifi.active_access_point().await {
            if let Ok(ap) = NMAccessPointProxy::builder(conn).path(active_ap)?.build().await {
                let ssid = ssid_to_string(ap.ssid().await.unwrap_or_default());
                details.ssid = if ssid.is_empty() { None } else { Some(ssid) };
                details.signal = ap.strength().await.ok();
                let freq = ap.frequency().await.ok();
                details.frequency_mhz = freq;
                details.band = freq.and_then(frequency_band_label);
                details.channel = freq.and_then(frequency_channel);
                details.bssid = ap.hw_address().await.ok();
                let security = wifi_security_label(
                    ap.flags().await.unwrap_or(0),
                    ap.wpa_flags().await.unwrap_or(0),
                    ap.rsn_flags().await.unwrap_or(0),
                );
                details.security = Some(security);
            }
        }
    }

    if let Ok(ip4_path) = dev.ip4_config().await {
        if ip4_path.as_str() != "/" {
            let ip4 = NMIP4ConfigProxy::builder(conn).path(ip4_path)?.build().await?;
            if let Ok(addr) = ip4.address_data().await {
                if let Some(first) = addr.first() {
                    if let Some(value) = first.get("address") {
                        details.ip4 = owned_value_str(value);
                    }
                }
            }
            if let Ok(gw) = ip4.gateway().await {
                if !gw.is_empty() {
                    details.gateway4 = Some(gw);
                }
            }
            if let Ok(names) = ip4.nameserver_data().await {
                for entry in names {
                    if let Some(value) = entry.get("address") {
                        if let Some(addr) = owned_value_str(value) {
                            details.dns4.push(addr);
                        }
                    }
                }
            }
        }
    }

    if let Ok(ip6_path) = dev.ip6_config().await {
        if ip6_path.as_str() != "/" {
            let ip6 = NMIP6ConfigProxy::builder(conn).path(ip6_path)?.build().await?;
            if let Ok(addr) = ip6.address_data().await {
                for entry in addr {
                    if let Some(value) = entry.get("address") {
                        if let Some(addr) = owned_value_str(value) {
                            details.ip6.push(addr);
                        }
                    }
                }
            }
            if let Ok(gw) = ip6.gateway().await {
                if !gw.is_empty() {
                    details.gateway6.push(gw);
                }
            }
            if let Ok(names) = ip6.nameserver_data().await {
                for entry in names {
                    if let Some(value) = entry.get("address") {
                        if let Some(addr) = owned_value_str(value) {
                            details.dns6.push(addr);
                        }
                    }
                }
            }
        }
    }

    if details.ssid.is_none() && !iface.is_empty() {
        details.ssid = Some(iface);
    }

    Ok(details)
}

fn ssid_to_string(bytes: Vec<u8>) -> String {
    String::from_utf8_lossy(&bytes).trim_matches(char::from(0)).to_string()
}

fn wifi_security_label(flags: u32, wpa_flags: u32, rsn_flags: u32) -> String {
    if flags == 0 && wpa_flags == 0 && rsn_flags == 0 {
        return "open".to_string();
    }
    if rsn_flags != 0 {
        return "wpa2".to_string();
    }
    if wpa_flags != 0 {
        return "wpa".to_string();
    }
    "secured".to_string()
}

fn frequency_band_label(freq: u32) -> Option<String> {
    match freq {
        2400..=2499 => Some("2.4GHz".to_string()),
        5000..=5899 => Some("5GHz".to_string()),
        5925..=7125 => Some("6GHz".to_string()),
        _ => None,
    }
}

fn frequency_channel(freq: u32) -> Option<u32> {
    if (2412..=2472).contains(&freq) {
        return Some(((freq - 2407) / 5) as u32);
    }
    if freq == 2484 {
        return Some(14);
    }
    if (5000..=5895).contains(&freq) {
        return Some(((freq - 5000) / 5) as u32);
    }
    if (5925..=7125).contains(&freq) {
        return Some(((freq - 5950) / 5) as u32);
    }
    None
}

fn owned_value_str(value: &zbus::zvariant::OwnedValue) -> Option<String> {
    if let Ok(s) = value.downcast_ref::<zbus::zvariant::Str>() {
        return Some(s.as_str().to_string());
    }
    if let Ok(s) = value.downcast_ref::<String>() {
        return Some(s.clone());
    }
    None
}

fn build_wifi_settings(ssid: &str, password: Option<&str>) -> HashMap<String, HashMap<String, zbus::zvariant::OwnedValue>> {
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

fn owned_value<V>(value: V) -> zbus::zvariant::OwnedValue
where
    V: Into<zbus::zvariant::Value<'static>>,
{
    zbus::zvariant::OwnedValue::try_from(value.into())
        .expect("owned value conversion should not fail")
}

async fn list_known_wifi_ssids(
    conn: &Connection,
    _nm: &NetworkManagerProxy<'_>,
) -> Result<Vec<String>, NetError> {
    use crate::dbus::NMSettingsProxy;

    let settings = NMSettingsProxy::new(conn).await?;
    let paths = settings.list_connections().await?;
    let ssids = Vec::new();

    for path in paths {
        if let Ok(conn_obj) = NMSettingsConnectionProxy::builder(conn)
            .path(path.clone())?
            .build()
            .await
        {
            if let Ok(settings_map) = conn_obj.get_settings().await {
                let _ = settings_map;
            }
        }
    }

    Ok(ssids)
}

async fn find_wifi_device(
    conn: &Connection,
    devices: &[zbus::zvariant::OwnedObjectPath],
) -> Option<zbus::zvariant::OwnedObjectPath> {
    for path in devices {
        let dev = NMDeviceProxy::builder(conn).path(path.clone()).ok()?.build().await.ok()?;
        if dev.device_type().await.unwrap_or(0) != NM_DEVICE_TYPE_WIFI {
            continue;
        }
        let state = dev.state().await.unwrap_or(0);
        if state == 100 || state == 70 || state == 30 {
            return Some(path.clone());
        }
    }
    None
}

async fn find_active_wifi_device(
    conn: &Connection,
    devices: &[zbus::zvariant::OwnedObjectPath],
) -> Option<zbus::zvariant::OwnedObjectPath> {
    for path in devices {
        let dev = NMDeviceProxy::builder(conn).path(path.clone()).ok()?.build().await.ok()?;
        if dev.device_type().await.unwrap_or(0) != NM_DEVICE_TYPE_WIFI {
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
    wifi_device: &zbus::zvariant::OwnedObjectPath,
    ssid: &str,
) -> Option<zbus::zvariant::OwnedObjectPath> {
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