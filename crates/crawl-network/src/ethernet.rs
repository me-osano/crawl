//! Ethernet interface listing and details.

use std::collections::HashMap;
use zbus::Connection;

use crate::dbus::{
    NMDeviceProxy, NMDeviceWiredProxy, NMIP4ConfigProxy, NMIP6ConfigProxy, NetworkManagerProxy,
    NM_DEVICE_TYPE_ETHERNET,
};
use crate::NetError;
use crawl_ipc::types::{ActiveEthernetDetails, EthernetInterface};

pub async fn list_ethernet_interfaces(
    conn: &Connection,
    nm: &NetworkManagerProxy<'_>,
) -> Result<Vec<EthernetInterface>, NetError> {
    let mut interfaces = Vec::new();
    for path in nm.get_devices().await? {
        let dev = NMDeviceProxy::builder(conn).path(path.clone())?.build().await?;
        if dev.device_type().await.unwrap_or(0) != NM_DEVICE_TYPE_ETHERNET {
            continue;
        }
        let ifname = dev.interface().await.unwrap_or_default();
        let active = dev.active_connection().await.ok();
        let connected = active.map(|p| p.as_str() != "/").unwrap_or(false);
        let mac = dev.hw_address().await.ok();
        let ip4_raw = dev.ip4_address().await.unwrap_or(0);
        let ip4 = if ip4_raw == 0 { None } else { Some(std::net::Ipv4Addr::from(ip4_raw).to_string()) };
        interfaces.push(EthernetInterface {
            ifname,
            connected,
            mac,
            ip4,
            ip6: Vec::new(),
        });
    }
    Ok(interfaces)
}

pub async fn list_ethernet() -> Result<Vec<EthernetInterface>, NetError> {
    let conn = Connection::system().await?;
    let nm = NetworkManagerProxy::new(&conn).await?;
    list_ethernet_interfaces(&conn, &nm).await
}

pub async fn refresh_ethernet_details(
    conn: &Connection,
    nm: &NetworkManagerProxy<'_>,
    interface: Option<&str>,
) -> Result<ActiveEthernetDetails, NetError> {
    let devices = nm.get_devices().await?;
    let (eth_device, ifname) = match interface {
        Some(name) => (
            find_ethernet_device(conn, &devices, name).await
                .ok_or_else(|| NetError::NotFound(format!("ethernet interface '{name}'")))?,
            name.to_string(),
        ),
        None => find_active_ethernet_device(conn, &devices).await
            .ok_or_else(|| NetError::NotFound("active ethernet device".into()))?,
    };

    let dev = NMDeviceProxy::builder(conn).path(eth_device.clone())?.build().await?;
    let mut details = ActiveEthernetDetails {
        ifname: ifname.clone(),
        speed: None,
        ipv4: None,
        ipv6: Vec::new(),
        gateway4: None,
        gateway6: Vec::new(),
        dns4: Vec::new(),
        dns6: Vec::new(),
        mac: dev.hw_address().await.ok(),
    };

    if let Ok(wired) = NMDeviceWiredProxy::builder(conn).path(eth_device.clone())?.build().await {
        if let Ok(speed) = wired.speed().await {
            if speed > 0 {
                details.speed = Some(format!("{speed} Mb/s"));
            }
        }
    }

    if details.speed.is_none() && !details.ifname.is_empty() {
        if let Some(sysfs_speed) = crate::sysfs::read_sysfs_speed(&details.ifname) {
            details.speed = Some(format!("{sysfs_speed} Mb/s"));
        } else {
            details.speed = Some("Unknown".to_string());
        }
    }

    if let Ok(ip4_path) = dev.ip4_config().await {
        if ip4_path.as_str() != "/" {
            let ip4 = NMIP4ConfigProxy::builder(conn).path(ip4_path)?.build().await?;
            if let Ok(addr) = ip4.address_data().await {
                if let Some(first) = addr.first() {
                    if let Some(value) = first.get("address") {
                        details.ipv4 = owned_value_str(value);
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
                            details.ipv6.push(addr);
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

    Ok(details)
}

pub async fn get_ethernet_details(interface: Option<&str>) -> Result<ActiveEthernetDetails, NetError> {
    let conn = Connection::system().await?;
    let nm = NetworkManagerProxy::new(&conn).await?;
    refresh_ethernet_details(&conn, &nm, interface).await
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
    let root = zbus::zvariant::OwnedObjectPath::try_from("/")?;
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

fn owned_value_str(value: &zbus::zvariant::OwnedValue) -> Option<String> {
    if let Ok(s) = value.downcast_ref::<zbus::zvariant::Str>() {
        return Some(s.as_str().to_string());
    }
    if let Ok(s) = value.downcast_ref::<String>() {
        return Some(s.clone());
    }
    None
}

fn build_ethernet_settings(interface: &str) -> HashMap<String, HashMap<String, zbus::zvariant::OwnedValue>> {
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

fn owned_value<V>(value: V) -> zbus::zvariant::OwnedValue
where
    V: Into<zbus::zvariant::Value<'static>>,
{
    zbus::zvariant::OwnedValue::try_from(value.into())
        .expect("owned value conversion should not fail")
}

async fn find_ethernet_device(
    conn: &Connection,
    devices: &[zbus::zvariant::OwnedObjectPath],
    interface: &str,
) -> Option<zbus::zvariant::OwnedObjectPath> {
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
    devices: &[zbus::zvariant::OwnedObjectPath],
) -> Option<(zbus::zvariant::OwnedObjectPath, String)> {
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
    devices: &[zbus::zvariant::OwnedObjectPath],
) -> Option<(zbus::zvariant::OwnedObjectPath, String)> {
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