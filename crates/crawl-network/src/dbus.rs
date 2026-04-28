//! D-Bus proxy traits for NetworkManager interfaces.

use std::collections::HashMap;
use zbus::proxy;
use zbus::zvariant::{OwnedValue, Value};

/// Minimal proxy for org.freedesktop.NetworkManager
#[proxy(
    interface = "org.freedesktop.NetworkManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager"
)]
pub trait NetworkManager {
    #[zbus(property)]
    fn connectivity(&self) -> zbus::Result<u32>;

    #[zbus(property)]
    fn wireless_enabled(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn set_wireless_enabled(&self, enabled: bool) -> zbus::Result<()>;

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
    ) -> zbus::Result<(
        zbus::zvariant::OwnedObjectPath,
        zbus::zvariant::OwnedObjectPath,
        zbus::zvariant::OwnedObjectPath,
    )>;

    fn deactivate_connection(
        &self,
        active_connection: zbus::zvariant::OwnedObjectPath,
    ) -> zbus::Result<()>;

    #[zbus(property)]
    fn primary_connection(&self) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;

    #[zbus(signal)]
    fn state_changed(&self, state: u32) -> zbus::Result<()>;
}

/// Proxy for a NM Device object
#[proxy(
    interface = "org.freedesktop.NetworkManager.Device",
    default_service = "org.freedesktop.NetworkManager"
)]
pub trait NMDevice {
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

    #[zbus(property, name = "Ip6Config")]
    fn ip6_config(&self) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;

    #[zbus(property, name = "Ip4Config")]
    fn ip4_config(&self) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.Device.Wireless",
    default_service = "org.freedesktop.NetworkManager"
)]
pub trait NMDeviceWireless {
    #[zbus(property)]
    fn active_access_point(&self) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;

    #[zbus(property)]
    fn access_points(&self) -> zbus::Result<Vec<zbus::zvariant::OwnedObjectPath>>;

    #[zbus(property)]
    fn hw_address(&self) -> zbus::Result<String>;

    fn request_scan(&self, options: HashMap<&str, Value<'_>>) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.AccessPoint",
    default_service = "org.freedesktop.NetworkManager"
)]
pub trait NMAccessPoint {
    #[zbus(property)]
    fn ssid(&self) -> zbus::Result<Vec<u8>>;

    #[zbus(property)]
    fn strength(&self) -> zbus::Result<u8>;

    #[zbus(property, name = "Frequency")]
    fn frequency(&self) -> zbus::Result<u32>;

    #[zbus(property, name = "HwAddress")]
    fn hw_address(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn flags(&self) -> zbus::Result<u32>;

    #[zbus(property, name = "WpaFlags")]
    fn wpa_flags(&self) -> zbus::Result<u32>;

    #[zbus(property, name = "RsnFlags")]
    fn rsn_flags(&self) -> zbus::Result<u32>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.Device.Wired",
    default_service = "org.freedesktop.NetworkManager"
)]
pub trait NMDeviceWired {
    #[zbus(property, name = "Speed")]
    fn speed(&self) -> zbus::Result<u32>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.IP4Config",
    default_service = "org.freedesktop.NetworkManager"
)]
pub trait NMIP4Config {
    #[zbus(property, name = "AddressData")]
    fn address_data(&self) -> zbus::Result<Vec<HashMap<String, OwnedValue>>>;

    #[zbus(property, name = "Gateway")]
    fn gateway(&self) -> zbus::Result<String>;

    #[zbus(property, name = "NameserverData")]
    fn nameserver_data(&self) -> zbus::Result<Vec<HashMap<String, OwnedValue>>>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.IP6Config",
    default_service = "org.freedesktop.NetworkManager"
)]
pub trait NMIP6Config {
    #[zbus(property, name = "AddressData")]
    fn address_data(&self) -> zbus::Result<Vec<HashMap<String, OwnedValue>>>;

    #[zbus(property, name = "Gateway")]
    fn gateway(&self) -> zbus::Result<String>;

    #[zbus(property, name = "NameserverData")]
    fn nameserver_data(&self) -> zbus::Result<Vec<HashMap<String, OwnedValue>>>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.Settings",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager/Settings"
)]
pub trait NMSettings {
    fn list_connections(&self) -> zbus::Result<Vec<zbus::zvariant::OwnedObjectPath>>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.Settings.Connection",
    default_service = "org.freedesktop.NetworkManager"
)]
pub trait NMSettingsConnection {
    fn get_settings(&self) -> zbus::Result<HashMap<String, HashMap<String, OwnedValue>>>;
    fn delete(&self) -> zbus::Result<()>;
}

pub const NM_DEVICE_TYPE_WIFI: u32 = 2;
pub const NM_DEVICE_TYPE_ETHERNET: u32 = 1;
