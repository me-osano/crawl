//! crawl-disk: Block device and removable media management via UDisks2.
//!
//! Connects to org.freedesktop.UDisks2 on the system bus.
//! Watches for devices being added/removed and emits DiskEvents.
//! Supports mount, unmount, and eject operations.

use crawl_ipc::{
    events::{CrawlEvent, DiskEvent},
    types::BlockDevice,
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::{info, warn};
use zbus::{proxy, Connection};

// ── Config ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// Only emit events for removable devices
    pub removable_only: bool,
    /// Auto-mount removable devices when they appear
    pub auto_mount: bool,
}

// ── Error ────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum DiskError {
    #[error("D-Bus error: {0}")]
    DBus(#[from] zbus::Error),
    #[error("device not found: {0}")]
    NotFound(String),
    #[error("mount failed: {0}")]
    MountFailed(String),
    #[error("unmount failed: {0}")]
    UnmountFailed(String),
}

// ── D-Bus proxies ─────────────────────────────────────────────────────────────

#[proxy(
    interface = "org.freedesktop.UDisks2.Manager",
    default_service = "org.freedesktop.UDisks2",
    default_path = "/org/freedesktop/UDisks2/Manager"
)]
trait UDisks2Manager {
    fn get_block_devices(
        &self,
        options: std::collections::HashMap<String, zbus::zvariant::Value<'_>>,
    ) -> zbus::Result<Vec<zbus::zvariant::OwnedObjectPath>>;
}

#[proxy(
    interface = "org.freedesktop.UDisks2.Block",
    default_service = "org.freedesktop.UDisks2"
)]
trait UDisks2Block {
    #[zbus(property)]
    fn device(&self) -> zbus::Result<Vec<u8>>;

    #[zbus(property)]
    fn id_label(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn id_type(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn size(&self) -> zbus::Result<u64>;

    #[zbus(property)]
    fn hint_system(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn hint_ignore(&self) -> zbus::Result<bool>;
}

#[proxy(
    interface = "org.freedesktop.UDisks2.Filesystem",
    default_service = "org.freedesktop.UDisks2"
)]
trait UDisks2Filesystem {
    fn mount(
        &self,
        options: std::collections::HashMap<String, zbus::zvariant::Value<'_>>,
    ) -> zbus::Result<String>;

    fn unmount(
        &self,
        options: std::collections::HashMap<String, zbus::zvariant::Value<'_>>,
    ) -> zbus::Result<()>;

    #[zbus(property)]
    fn mount_points(&self) -> zbus::Result<Vec<Vec<u8>>>;
}

#[proxy(
    interface = "org.freedesktop.UDisks2.Drive",
    default_service = "org.freedesktop.UDisks2"
)]
trait UDisks2Drive {
    fn eject(
        &self,
        options: std::collections::HashMap<String, zbus::zvariant::Value<'_>>,
    ) -> zbus::Result<()>;

    #[zbus(property)]
    fn removable(&self) -> zbus::Result<bool>;
}

// ── Domain runner ─────────────────────────────────────────────────────────────

pub async fn run(cfg: Config, tx: broadcast::Sender<CrawlEvent>) -> anyhow::Result<()> {
    info!("crawl-disk starting");

    let conn = Connection::system().await?;

    // Watch the ObjectManager for device add/remove signals
    let obj_manager_proxy = zbus::fdo::ObjectManagerProxy::builder(&conn)
        .destination("org.freedesktop.UDisks2")?
        .path("/org/freedesktop/UDisks2")?
        .build()
        .await?;

    let mut interfaces_added   = obj_manager_proxy.receive_interfaces_added().await?;
    let mut interfaces_removed = obj_manager_proxy.receive_interfaces_removed().await?;

    info!("crawl-disk: watching UDisks2 for block device events");

    loop {
        tokio::select! {
            Some(signal) = interfaces_added.next() => {
                let args = signal.args()?;
                let path = args.object_path.to_string();
                if path.contains("/block_devices/") {
                    let dev_result = build_block_device(&conn, &path).await;
                    if let Ok(dev) = dev_result
                        && (!cfg.removable_only || dev.removable)
                    {
                        info!(device = %dev.device, "block device added");
                        let _ = tx.send(CrawlEvent::Disk(DiskEvent::DeviceAdded { device: dev.clone() }));
                        if cfg.auto_mount && dev.removable && !dev.mounted {
                            let mount_result = mount_device(&conn, &path).await;
                            if let Err(e) = mount_result {
                                warn!("auto-mount failed: {e}");
                            }
                        }
                    }
                }
            }
            Some(signal) = interfaces_removed.next() => {
                let args = signal.args()?;
                let path = args.object_path.to_string();
                if path.contains("/block_devices/") {
                    info!(path = %path, "block device removed");
                    let _ = tx.send(CrawlEvent::Disk(DiskEvent::DeviceRemoved { device_path: path }));
                }
            }
        }
    }
}

// ── Builder ───────────────────────────────────────────────────────────────────

async fn build_block_device(conn: &Connection, path: &str) -> Result<BlockDevice, DiskError> {
    let block = UDisks2BlockProxy::builder(conn).path(path)?.build().await?;
    let fs    = UDisks2FilesystemProxy::builder(conn).path(path)?.build().await;

    let device_bytes = block.device().await.unwrap_or_default();
    let device = String::from_utf8_lossy(&device_bytes)
        .trim_end_matches('\0').to_string();

    let mount_points: Vec<Vec<u8>> = match fs {
        Ok(ref fs_proxy) => fs_proxy.mount_points().await.unwrap_or_default(),
        Err(_) => Vec::new(),
    };

    let mount_point = mount_points.first()
        .map(|mp| String::from_utf8_lossy(mp).trim_end_matches('\0').to_string());

    let label = block.id_label().await.unwrap_or_default();
    let size_bytes = block.size().await.unwrap_or(0);
    let fs_type = block.id_type().await.unwrap_or_default();

    Ok(BlockDevice {
        device,
        label:       if label.is_empty() { None } else { Some(label) },
        size_bytes,
        filesystem:  if fs_type.is_empty() { None } else { Some(fs_type) },
        mount_point,
        mounted:     !mount_points.is_empty(),
        removable:   false,
    })
}

// ── Operations ────────────────────────────────────────────────────────────────

async fn mount_device(conn: &Connection, path: &str) -> Result<String, DiskError> {
    let fs = UDisks2FilesystemProxy::builder(conn).path(path)?.build().await?;
    let mount_path = fs.mount(Default::default()).await
        .map_err(|e| DiskError::MountFailed(e.to_string()))?;
    Ok(mount_path)
}

// ── Public query API ──────────────────────────────────────────────────────────

pub async fn list_devices() -> Result<Vec<BlockDevice>, DiskError> {
    let conn = Connection::system().await?;
    let manager = UDisks2ManagerProxy::new(&conn).await?;
    let paths = manager.get_block_devices(Default::default()).await?;
    let mut devices = Vec::new();
    for path in &paths {
        let dev_result = build_block_device(&conn, path.as_str()).await;
        if let Ok(dev) = dev_result {
            // Skip tiny or system devices
            if dev.size_bytes > 1_048_576 && !dev.device.ends_with("loop") {
                devices.push(dev);
            }
        }
    }
    Ok(devices)
}

pub async fn mount(device_path: &str) -> Result<String, DiskError> {
    let conn = Connection::system().await?;
    let mount_path = mount_device(&conn, device_path).await?;
    Ok(mount_path)
}

pub async fn unmount(device_path: &str) -> Result<(), DiskError> {
    let conn = Connection::system().await?;
    let fs = UDisks2FilesystemProxy::builder(&conn).path(device_path)?.build().await?;
    fs.unmount(Default::default())
        .await
        .map_err(|e| DiskError::UnmountFailed(e.to_string()))
}

pub async fn eject(drive_path: &str) -> Result<(), DiskError> {
    let conn = Connection::system().await?;
    let drive = UDisks2DriveProxy::builder(&conn).path(drive_path)?.build().await?;
    let result = drive.eject(Default::default()).await;
    result?;
    Ok(())
}
