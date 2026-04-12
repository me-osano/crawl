//! crawl-bluetooth: Bluetooth domain via BlueZ/bluer.
//!
//! Runs as a long-lived tokio task, publishing BtEvents to the broadcast channel.
//! All BlueZ communication is async via the bluer crate.

use bluer::{AdapterEvent, DeviceEvent, DeviceProperty};
use crawl_ipc::{
    events::{BtEvent, CrawlEvent},
    types::BtDevice,
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

// ── Config ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Power the adapter on startup if false
    pub auto_power: bool,
    /// Scan timeout in seconds (0 = no timeout)
    pub scan_timeout_secs: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self { auto_power: false, scan_timeout_secs: 30 }
    }
}

// ── Error ────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum BtError {
    #[error("BlueZ session error: {0}")]
    Session(#[from] bluer::Error),
    #[error("no default adapter found")]
    NoAdapter,
    #[error("device not found: {0}")]
    DeviceNotFound(String),
}

// ── Domain runner ─────────────────────────────────────────────────────────────

/// Entry point called by crawl-daemon. Runs indefinitely, publishing events.
pub async fn run(cfg: Config, tx: broadcast::Sender<CrawlEvent>) -> anyhow::Result<()> {
    info!("crawl-bluetooth starting");

    let session = bluer::Session::new().await?;
    let adapter = session.default_adapter().await.map_err(|_| BtError::NoAdapter)?;

    info!(adapter = %adapter.name(), "using Bluetooth adapter");

    if cfg.auto_power && !adapter.is_powered().await? {
        adapter.set_powered(true).await?;
        info!("adapter powered on");
    }

    // Publish adapter power state
    let powered = adapter.is_powered().await.unwrap_or(false);
    let _ = tx.send(CrawlEvent::Bluetooth(BtEvent::AdapterPowered { on: powered }));

    // Watch for adapter-level events (device added/removed)
    let mut adapter_events = adapter.events().await?;

    while let Some(event) = adapter_events.next().await {
        match event {
            AdapterEvent::DeviceAdded(addr) => {
                let dev_result = adapter.device(addr);
                match dev_result {
                    Ok(dev) => {
                        let bt_dev = device_to_ipc(&dev).await;
                        info!(address = %addr, name = ?bt_dev.name, "device discovered");
                        let _ = tx.send(CrawlEvent::Bluetooth(BtEvent::DeviceDiscovered { device: bt_dev }));

                        // Watch device-level events in a subtask
                        let tx2 = tx.clone();
                        tokio::spawn(watch_device(dev, addr.to_string(), tx2));
                    }
                    Err(e) => warn!("failed to get device {addr}: {e}"),
                }
            }
            AdapterEvent::DeviceRemoved(addr) => {
                info!(address = %addr, "device removed");
                let _ = tx.send(CrawlEvent::Bluetooth(BtEvent::DeviceRemoved { address: addr.to_string() }));
            }
            _ => {}
        }
    }

    Ok(())
}

/// Watch property changes on a single Bluetooth device.
async fn watch_device(
    device: bluer::Device,
    address: String,
    tx: broadcast::Sender<CrawlEvent>,
) {
    let mut events = match device.events().await {
        Ok(e) => e,
        Err(e) => { error!("device event stream failed: {e}"); return; }
    };

    while let Some(event) = events.next().await {
        let DeviceEvent::PropertyChanged(prop) = event;
        if let DeviceProperty::Connected(connected) = prop {
            let bt_dev = device_to_ipc(&device).await;
            let evt = if connected {
                BtEvent::DeviceConnected { device: bt_dev }
            } else {
                BtEvent::DeviceDisconnected { address: address.clone() }
            };
            let _ = tx.send(CrawlEvent::Bluetooth(evt));
        }
    }
}

/// Convert a bluer Device into the shared IPC BtDevice type.
async fn device_to_ipc(device: &bluer::Device) -> BtDevice {
    BtDevice {
        address:   device.address().to_string(),
        name:      device.name().await.ok().flatten(),
        connected: device.is_connected().await.unwrap_or(false),
        paired:    device.is_paired().await.unwrap_or(false),
        rssi:      device.rssi().await.ok().flatten(),
        battery:   None, // TODO: org.bluez.Battery1 interface
        icon:      device.icon().await.ok().flatten(),
    }
}

// ── Public query API (called by crawl-daemon router) ─────────────────────────

pub async fn get_devices() -> Result<Vec<BtDevice>, BtError> {
    let session = bluer::Session::new().await?;
    let adapter = session.default_adapter().await.map_err(|_| BtError::NoAdapter)?;
    let addrs   = adapter.device_addresses().await?;

    let mut devices = Vec::new();
    for addr in addrs {
        let dev_result = adapter.device(addr);
        if let Ok(dev) = dev_result {
            devices.push(device_to_ipc(&dev).await);
        }
    }
    Ok(devices)
}

pub async fn connect(address: &str) -> Result<(), BtError> {
    let session = bluer::Session::new().await?;
    let adapter = session.default_adapter().await.map_err(|_| BtError::NoAdapter)?;
    let addr: bluer::Address = address.parse()
        .map_err(|_| BtError::DeviceNotFound(address.to_string()))?;
    let device = adapter.device(addr)?;
    device.connect().await?;
    Ok(())
}

pub async fn disconnect(address: &str) -> Result<(), BtError> {
    let session = bluer::Session::new().await?;
    let adapter = session.default_adapter().await.map_err(|_| BtError::NoAdapter)?;
    let addr: bluer::Address = address.parse()
        .map_err(|_| BtError::DeviceNotFound(address.to_string()))?;
    let device = adapter.device(addr)?;
    device.disconnect().await?;
    Ok(())
}

pub async fn set_powered(on: bool) -> Result<(), BtError> {
    let session = bluer::Session::new().await?;
    let adapter = session.default_adapter().await.map_err(|_| BtError::NoAdapter)?;
    adapter.set_powered(on).await?;
    Ok(())
}
