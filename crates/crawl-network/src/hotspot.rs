//! Hotspot control via NetworkManager or hostapd/dnsmasq.

use std::collections::HashMap;
use std::path::PathBuf;
use tokio::process::Command;
use zbus::Connection;

use crate::dbus::{
    NMAccessPointProxy, NMDeviceProxy, NMDeviceWirelessProxy, NetworkManagerProxy,
    NM_DEVICE_TYPE_WIFI,
};
use crate::sysfs;
use crate::NetError;
use crawl_ipc::types::{HotspotBackend, HotspotClient, HotspotConfig, HotspotStatus};

pub const HOSTAPD_CONFIG_PATH: &str = "/run/crawl/hostapd.conf";
pub const DNSMASQ_CONFIG_PATH: &str = "/run/crawl/dnsmasq.conf";

pub async fn start_hotspot(
    config: &HotspotConfig,
    use_virtual_iface: bool,
) -> Result<HotspotStatus, NetError> {
    let upstream = sysfs::detect_upstream_type();
    let backend = config.backend.as_ref().cloned().unwrap_or_else(|| {
        if upstream != "unknown" && upstream != "lo" && upstream != config.iface.as_deref().unwrap_or("wlan0") {
            if upstream.starts_with("en") || upstream.starts_with("eth") {
                HotspotBackend::NetworkManager
            } else {
                HotspotBackend::Hostapd
            }
        } else {
            HotspotBackend::Hostapd
        }
    });

    match backend {
        HotspotBackend::NetworkManager => start_nm_hotspot(config).await,
        HotspotBackend::Hostapd => start_hostapd_hotspot(config, use_virtual_iface).await,
    }
}

pub async fn stop_hotspot() -> Result<(), NetError> {
    let status = hotspot_status().await.ok();
    if let Some(ref s) = status {
        if s.active {
            match s.backend {
                HotspotBackend::NetworkManager => stop_nm_hotspot().await,
                HotspotBackend::Hostapd => stop_hostapd_hotspot().await,
            }
        } else {
            stop_hostapd_hotspot().await
        }
    } else {
        let _ = stop_hostapd_hotspot().await;
        let _ = stop_nm_hotspot().await;
        Ok(())
    }
}

pub async fn hotspot_status() -> Result<HotspotStatus, NetError> {
    let conn = Connection::system().await?;
    let nm = NetworkManagerProxy::new(&conn).await?;
    let devices = nm.get_devices().await?;

    for path in devices {
        let dev = NMDeviceProxy::builder(&conn).path(path.clone())?.build().await?;
        if dev.device_type().await.unwrap_or(0) != NM_DEVICE_TYPE_WIFI {
            continue;
        }
        let active = dev.active_connection().await?;
        if active.as_str() == "/" {
            continue;
        }
        if let Ok(wifi) = NMDeviceWirelessProxy::builder(&conn).path(path.clone())?.build().await {
            if let Ok(active_ap) = wifi.active_access_point().await {
                if let Ok(ap) = NMAccessPointProxy::builder(&conn).path(active_ap)?.build().await {
                    let ssid_raw = ap.ssid().await.unwrap_or_default();
                    let ssid = ssid_to_string(ssid_raw.clone());
                    if !ssid.is_empty() && ssid.starts_with("hotspot-") {
                        let real_ssid = ssid.strip_prefix("hotspot-").unwrap_or(&ssid).to_string();
                        let band = ap.frequency().await.ok().and_then(frequency_band_label);
                        let channel = ap.frequency().await.ok().and_then(frequency_channel);
                        let iface = dev.interface().await.unwrap_or_default();
                        let clients = read_hotspot_clients(&iface).unwrap_or_default();

                        return Ok(HotspotStatus {
                            active: true,
                            ssid: Some(real_ssid),
                            iface: Some(iface),
                            band,
                            channel,
                            clients,
                            backend: HotspotBackend::NetworkManager,
                            supports_virtual_ap: false,
                        });
                    }
                }
            }
        }
    }

    let hostapd_running = std::process::Command::new("pidof")
        .arg("hostapd")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if hostapd_running {
        let ap_iface = std::process::Command::new("sh")
            .args(["-c", "ip link show | awk '/.*ap:.*UP/ {print $2}' | tr -d ':' | head -1"])
            .output()
            .ok()
            .and_then(|o| {
                let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if s.is_empty() { None } else { Some(s) }
            })
            .unwrap_or_else(|| "wlan0ap".to_string());

        let clients = read_hotspot_clients(&ap_iface).unwrap_or_default();

        return Ok(HotspotStatus {
            active: true,
            ssid: None,
            iface: Some(ap_iface),
            band: None,
            channel: None,
            clients,
            backend: HotspotBackend::Hostapd,
            supports_virtual_ap: true,
        });
    }

    Ok(HotspotStatus {
        active: false,
        ssid: None,
        iface: None,
        band: None,
        channel: None,
        clients: Vec::new(),
        backend: HotspotBackend::default(),
        supports_virtual_ap: sysfs::supports_virtual_ap("wlan0"),
    })
}

async fn start_hostapd_hotspot(
    config: &HotspotConfig,
    use_virtual_iface: bool,
) -> Result<HotspotStatus, NetError> {
    let upstream = sysfs::detect_upstream_type();
    tracing::warn!("hotspot upstream detected: {}", upstream);

    let station_iface = config.iface.clone()
        .or_else(|| {
            std::process::Command::new("sh")
                .args(["-c", "nmcli -t -f DEVICE,TYPE device | awk -F: '$2==\"wifi\" {print $1; exit}'"])
                .output().ok()
                .and_then(|o| {
                    let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    if s.is_empty() { None } else { Some(s) }
                })
        })
        .unwrap_or_else(|| "wlan0".to_string());

    let phy = sysfs::detect_wifi_phy(&station_iface).await?;

    let supports_vap = use_virtual_iface && sysfs::supports_virtual_ap(&phy);
    let ap_iface = if supports_vap {
        format!("{}ap", station_iface)
    } else {
        station_iface.clone()
    };

    let channel = config.channel
        .or_else(|| sysfs::detect_current_channel(&station_iface))
        .unwrap_or(1);

    let band = config.band.clone()
        .unwrap_or_else(|| {
            if channel > 14 { "5".to_string() } else { "2.4".to_string() }
        });

    let gateway = "10.10.10.1".to_string();
    let (dhcp_start, dhcp_end) = ("10.10.10.10".to_string(), "10.10.10.250".to_string());

    let hc = HotspotHostapdConfig {
        ap_iface: ap_iface.clone(),
        phy_iface: station_iface.clone(),
        ssid: config.ssid.clone(),
        password: config.password.clone(),
        band: Some(band.clone()),
        channel: Some(channel),
        gateway: gateway.clone(),
        dhcp_range: (dhcp_start, dhcp_end),
    };

    if supports_vap {
        sysfs::teardown_interface(&ap_iface);
        sysfs::run_shell(&format!("iw dev {} interface add {} type __ap", station_iface, ap_iface)).await?;
        sysfs::nm_mark_unmanaged(&ap_iface).await?;
        sysfs::run_shell(&format!("ip addr add {}/24 dev {}", gateway, ap_iface)).await?;
        sysfs::run_shell(&format!("ip link set {} up", ap_iface)).await?;
    } else {
        sysfs::run_shell(&format!("ip addr add {}/24 dev {}", gateway, station_iface)).await?;
        sysfs::run_shell(&format!("ip link set {} up", station_iface)).await?;
        sysfs::nm_mark_unmanaged(&station_iface).await?;
    }

    let hostapd_conf = write_hostapd_conf(&hc);
    write_file(HOSTAPD_CONFIG_PATH, &hostapd_conf).map_err(|_e| NetError::Unsupported(_e.to_string()))?;

    let dnsmasq_conf = write_dnsmasq_conf(&hc);
    write_file(DNSMASQ_CONFIG_PATH, &dnsmasq_conf).map_err(|_e| NetError::Unsupported(_e.to_string()))?;

    if upstream != "unknown" && upstream != ap_iface {
        let _ = sysfs::setup_ip_forward(&ap_iface, &upstream).await;
    }

    tokio::spawn(async move {
        let _ = Command::new("hostapd").args([HOSTAPD_CONFIG_PATH]).spawn();
    });
    tokio::spawn(async move {
        let _ = Command::new("dnsmasq")
            .args(["-C", DNSMASQ_CONFIG_PATH, "-d"])
            .spawn();
    });

    Ok(HotspotStatus {
        active: true,
        ssid: Some(config.ssid.clone()),
        iface: Some(ap_iface),
        band: Some(band),
        channel: Some(channel),
        clients: Vec::new(),
        backend: HotspotBackend::Hostapd,
        supports_virtual_ap: supports_vap,
    })
}

async fn stop_hostapd_hotspot() -> Result<(), NetError> {
    let _ = sysfs::run_shell("pkill -f hostapd.*hostapd.conf 2>/dev/null || true").await;
    let _ = sysfs::run_shell("pkill -f dnsmasq.*dnsmasq.conf 2>/dev/null || true").await;

    let upstream = sysfs::detect_upstream_type();
    sysfs::teardown_nat(&upstream);

    let _ = std::process::Command::new("sh")
        .args(["-c", &format!(
            "for iface in wlan0ap wlp*ap; do iw dev $iface del 2>/dev/null; done"
        )])
        .output();

    let _ = std::process::Command::new("sh")
        .args(["-c", "nmcli device set wlan0 managed yes 2>/dev/null || \
                           nmcli device set wlp* managed yes 2>/dev/null || true"])
        .output();

    let _ = std::fs::remove_file(HOSTAPD_CONFIG_PATH);
    let _ = std::fs::remove_file(DNSMASQ_CONFIG_PATH);

    Ok(())
}

async fn start_nm_hotspot(config: &HotspotConfig) -> Result<HotspotStatus, NetError> {
    let conn = Connection::system().await?;
    let nm = NetworkManagerProxy::new(&conn).await?;
    let devices = nm.get_devices().await?;

    let wifi_device = find_available_wifi_device(&conn, &devices).await
        .ok_or_else(|| NetError::Unavailable)?;

    let dev = NMDeviceProxy::builder(&conn).path(wifi_device.clone())?.build().await?;
    let iface = dev.interface().await?;

    let mut settings = HashMap::new();

    let mut connection = HashMap::new();
    connection.insert("id".to_string(), owned_value(format!("hotspot-{}", config.ssid)));
    connection.insert("type".to_string(), owned_value("802-11-wireless".to_string()));
    connection.insert("autoconnect".to_string(), owned_value(false));
    connection.insert("interface-name".to_string(), owned_value(iface.clone()));
    settings.insert("connection".to_string(), connection);

    let mut wireless = HashMap::new();
    wireless.insert("ssid".to_string(), owned_value(config.ssid.as_bytes().to_vec()));
    wireless.insert("mode".to_string(), owned_value("ap".to_string()));

    if let Some(band) = &config.band {
        if band != "auto" {
            wireless.insert("band".to_string(), owned_value(band.clone()));
        }
    }
    if let Some(channel) = config.channel {
        wireless.insert("channel".to_string(), owned_value(channel));
    }
    settings.insert("802-11-wireless".to_string(), wireless);

    let mut ipv4 = HashMap::new();
    ipv4.insert("method".to_string(), owned_value("shared".to_string()));
    settings.insert("ipv4".to_string(), ipv4);

    let mut ipv6 = HashMap::new();
    ipv6.insert("method".to_string(), owned_value("ignore".to_string()));
    settings.insert("ipv6".to_string(), ipv6);

    if let Some(password) = &config.password {
        let mut security = HashMap::new();
        security.insert("key-mgmt".to_string(), owned_value("wpa-psk".to_string()));
        security.insert("psk".to_string(), owned_value(password.clone()));
        settings.insert("802-11-wireless-security".to_string(), security);
    }

    let root = zbus::zvariant::OwnedObjectPath::try_from("/")?;
    nm.add_and_activate_connection(settings, wifi_device, root).await?;

    Ok(HotspotStatus {
        active: true,
        ssid: Some(config.ssid.clone()),
        iface: Some(iface),
        band: config.band.clone(),
        channel: config.channel,
        clients: Vec::new(),
        backend: HotspotBackend::NetworkManager,
        supports_virtual_ap: false,
    })
}

async fn stop_nm_hotspot() -> Result<(), NetError> {
    let conn = Connection::system().await?;
    let nm = NetworkManagerProxy::new(&conn).await?;
    let devices = nm.get_devices().await?;

    for path in devices {
        let dev = NMDeviceProxy::builder(&conn).path(path.clone())?.build().await?;
        if dev.device_type().await.unwrap_or(0) != NM_DEVICE_TYPE_WIFI {
            continue;
        }
        let active = dev.active_connection().await?;
        if active.as_str() == "/" {
            continue;
        }
        if let Ok(wifi) = NMDeviceWirelessProxy::builder(&conn).path(path)?.build().await {
            if let Ok(aps) = wifi.access_points().await {
                for ap_path in aps {
                    if let Ok(ap) = NMAccessPointProxy::builder(&conn).path(ap_path)?.build().await {
                        let ssid_raw = ap.ssid().await.unwrap_or_default();
                        let ssid = ssid_to_string(ssid_raw.clone());
                        if !ssid.is_empty() && ssid.contains("hotspot-") {
                            nm.deactivate_connection(active.clone()).await?;
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    Err(NetError::NotFound("no active hotspot connection".into()))
}

fn read_hotspot_clients(iface: &str) -> Option<Vec<HotspotClient>> {
    let path = format!("/sys/kernel/debug/ieee80211/{}/stations", iface);
    let Ok(entries) = std::fs::read_dir(&path) else {
        return None;
    };
    let mut clients = Vec::new();
    for entry in entries.flatten() {
        let mac = entry.file_name().to_string_lossy().to_string();
        if mac == "head" || mac == "total" {
            continue;
        }
        let ip = std::fs::read_to_string(entry.path().join("last_ip4"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        clients.push(HotspotClient { mac, ip });
    }
    Some(clients)
}

async fn find_available_wifi_device(
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

fn ssid_to_string(bytes: Vec<u8>) -> String {
    String::from_utf8_lossy(&bytes).trim_matches(char::from(0)).to_string()
}

fn write_file(path: &str, contents: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(PathBuf::from(path).parent().unwrap_or(&PathBuf::from("/tmp")))?;
    std::fs::write(path, contents)
}

fn write_hostapd_conf(cfg: &HotspotHostapdConfig) -> String {
    let mut lines = vec![
        format!("interface={}", cfg.ap_iface),
        format!("driver=nl80211"),
        format!("ssid={}", cfg.ssid),
        "hw_mode=g".to_string(),
    ];

    if let Some(ref band) = cfg.band {
        if band == "5" {
            lines.push("hw_mode=a".to_string());
        } else {
            lines.push("hw_mode=g".to_string());
        }
    }

    if let Some(ch) = cfg.channel {
        lines.push(format!("channel={}", ch));
    } else {
        lines.push("channel=1".to_string());
    }

    lines.push("ieee80211n=1".to_string());

    if let Some(ref pwd) = cfg.password {
        lines.push("wpa=2".to_string());
        lines.push("wpa_key_mgmt=WPA-PSK".to_string());
        lines.push(format!("wpa_passphrase={}", pwd));
        lines.push("rsn_pairwise=CCMP".to_string());
    }

    lines.push("ctrl_interface=/run/hostapd".to_string());
    lines.push("ctrl_interface_group=0".to_string());
    lines.push("auth_algs=1".to_string());
    lines.push("disassoc_low_ack=1".to_string());
    lines.push("ignore_broadcast_ssid=0".to_string());
    lines.push("country_code=00".to_string());

    lines.join("\n")
}

fn write_dnsmasq_conf(cfg: &HotspotHostapdConfig) -> String {
    format!(
        "interface={}\n\
         dhcp-range={},{}\n\
         dhcp-option=3,{}\n\
         dhcp-option=6,{}\n\
         address=/#/10.10.10.1\n\
         port=0\n",
        cfg.ap_iface,
        cfg.dhcp_range.0,
        cfg.dhcp_range.1,
        cfg.gateway,
        cfg.gateway,
    )
}

fn owned_value<V>(value: V) -> zbus::zvariant::OwnedValue
where
    V: Into<zbus::zvariant::Value<'static>>,
{
    zbus::zvariant::OwnedValue::try_from(value.into())
        .expect("owned value conversion should not fail")
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct HotspotHostapdConfig {
    pub ap_iface: String,
    pub phy_iface: String,
    pub ssid: String,
    pub password: Option<String>,
    pub band: Option<String>,
    pub channel: Option<u32>,
    pub gateway: String,
    pub dhcp_range: (String, String),
}