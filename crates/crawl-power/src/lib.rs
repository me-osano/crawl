//! crawl-power: Battery and power status via UPower D-Bus.
//!
//! Connects to org.freedesktop.UPower on the system bus, finds the primary
//! battery device, watches for property changes, and emits PowerEvents.

use crawl_ipc::{
    events::{CrawlEvent, PowerEvent},
    types::{BatteryState, BatteryStatus},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::{info, warn};
use zbus::{proxy, Connection};

// ── Config ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Emit a LowBattery event below this percent
    pub low_battery_threshold: f64,
    /// Emit a Critical event below this percent
    pub critical_threshold: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self { low_battery_threshold: 20.0, critical_threshold: 5.0 }
    }
}

// ── Error ────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum PowerError {
    #[error("D-Bus error: {0}")]
    DBus(#[from] zbus::Error),
    #[error("no battery device found")]
    NoBattery,
}

// ── D-Bus proxies ─────────────────────────────────────────────────────────────

#[proxy(
    interface = "org.freedesktop.UPower",
    default_service = "org.freedesktop.UPower",
    default_path = "/org/freedesktop/UPower"
)]
trait UPower {
    fn enumerate_devices(&self) -> zbus::Result<Vec<zbus::zvariant::OwnedObjectPath>>;

    #[zbus(property)]
    fn on_battery(&self) -> zbus::Result<bool>;

    #[zbus(signal)]
    fn device_added(&self, device: zbus::zvariant::ObjectPath<'_>) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.freedesktop.UPower.Device",
    default_service = "org.freedesktop.UPower"
)]
trait UPowerDevice {
    fn refresh(&self) -> zbus::Result<()>;

    #[zbus(property, name = "Type")]
    fn device_type(&self) -> zbus::Result<u32>;

    #[zbus(property)]
    fn percentage(&self) -> zbus::Result<f64>;

    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;

    #[zbus(property)]
    fn time_to_empty(&self) -> zbus::Result<i64>;

    #[zbus(property)]
    fn time_to_full(&self) -> zbus::Result<i64>;

    #[zbus(property)]
    fn energy_rate(&self) -> zbus::Result<f64>;

    #[zbus(property)]
    fn voltage(&self) -> zbus::Result<f64>;

    #[zbus(property)]
    fn temperature(&self) -> zbus::Result<f64>;

    #[zbus(property)]
    fn is_present(&self) -> zbus::Result<bool>;
}

// ── State mapping ─────────────────────────────────────────────────────────────

/// UPower device type 2 = battery
const UPOWER_TYPE_BATTERY: u32 = 2;

fn upower_state_to_ipc(state: u32) -> BatteryState {
    match state {
        1 => BatteryState::Charging,
        2 => BatteryState::Discharging,
        4 => BatteryState::FullyCharged,
        3 => BatteryState::Empty,
        _ => BatteryState::Unknown,
    }
}

// ── Domain runner ─────────────────────────────────────────────────────────────

pub async fn run(cfg: Config, tx: broadcast::Sender<CrawlEvent>) -> anyhow::Result<()> {
    info!("crawl-power starting");

    let conn = Connection::system().await?;
    let upower = UPowerProxy::new(&conn).await?;

    // Find the primary battery device
    let devices = upower.enumerate_devices().await?;
    let battery_path = find_battery(&conn, &devices).await;

    let bat_path = match battery_path {
        Some(path) => path,
        None => {
            warn!("no battery found — power domain will report AC-only status");
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
            }
        }
    };

    info!(path = %bat_path, "using battery device");

    let bat_proxy = UPowerDeviceProxy::builder(&conn)
        .path(bat_path)?
        .build()
        .await?;

    // Poll battery status every 30 seconds
    loop {
        let status_result = read_battery_status(&bat_proxy, &upower).await;
        if let Ok(status) = status_result {
            let pct = status.percent;

            let _ = tx.send(CrawlEvent::Power(PowerEvent::BatteryUpdate { status }));

            if pct <= cfg.critical_threshold {
                let _ = tx.send(CrawlEvent::Power(PowerEvent::Critical { percent: pct }));
            } else if pct <= cfg.low_battery_threshold {
                let _ = tx.send(CrawlEvent::Power(PowerEvent::LowBattery { percent: pct }));
            }
        }

        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    }
}

async fn find_battery(
    conn: &Connection,
    devices: &[zbus::zvariant::OwnedObjectPath],
) -> Option<zbus::zvariant::OwnedObjectPath> {
    for path in devices {
        let dev_result = UPowerDeviceProxy::builder(conn)
            .path(path.clone())
            .ok()?
            .build()
            .await;
        if let Ok(dev) = dev_result
            && dev.device_type().await.ok() == Some(UPOWER_TYPE_BATTERY)
            && dev.is_present().await.ok() == Some(true)
        {
            return Some(path.clone());
        }
    }
    None
}

async fn read_battery_status(
    bat: &UPowerDeviceProxy<'_>,
    upower: &UPowerProxy<'_>,
) -> Result<BatteryStatus, PowerError> {
    let percent = bat.percentage().await.unwrap_or(0.0);
    let state_raw = bat.state().await.unwrap_or(0);
    let time_to_empty_secs = bat.time_to_empty().await.unwrap_or(0);
    let time_to_full_secs = bat.time_to_full().await.unwrap_or(0);
    let energy_rate_w = bat.energy_rate().await.ok();
    let voltage_v = bat.voltage().await.ok();
    let temperature_c = bat.temperature().await.ok();
    let on_ac = !upower.on_battery().await.unwrap_or(true);

    Ok(BatteryStatus {
        percent,
        state:               upower_state_to_ipc(state_raw),
        time_to_empty_secs:  Some(time_to_empty_secs),
        time_to_full_secs:   Some(time_to_full_secs),
        energy_rate_w,
        voltage_v,
        temperature_c,
        on_ac,
    })
}

// ── Public query API ──────────────────────────────────────────────────────────

pub async fn get_battery() -> Result<BatteryStatus, PowerError> {
    let conn = Connection::system().await?;
    let upower = UPowerProxy::new(&conn).await?;
    let devices = upower.enumerate_devices().await?;
    let bat_path = find_battery(&conn, &devices).await.ok_or(PowerError::NoBattery)?;
    let bat = UPowerDeviceProxy::builder(&conn).path(bat_path)?.build().await?;
    let status = read_battery_status(&bat, &upower).await?;
    Ok(status)
}
