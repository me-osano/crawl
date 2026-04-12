//! crawl-brightness: Display brightness via /sys/class/backlight.
//!
//! Requires write permission on brightness sysfs node.
//! On Arch: add user to `video` group and add a udev rule (see README).

use crawl_ipc::{
    events::{BrightnessEvent, CrawlEvent},
    types::BrightnessStatus,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::info;

// ── Config ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Specific device name under /sys/class/backlight, or empty to auto-detect
    pub device: String,
    /// Minimum allowed brightness percent
    pub min_percent: f32,
    /// Maximum allowed brightness percent
    pub max_percent: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self { device: String::new(), min_percent: 1.0, max_percent: 100.0 }
    }
}

// ── Error ────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum BrightnessError {
    #[error("no backlight device found in /sys/class/backlight")]
    NoDevice,
    #[error("failed to read {path}: {source}")]
    ReadError { path: String, source: std::io::Error },
    #[error("failed to write brightness: {0}")]
    WriteError(#[from] std::io::Error),
    #[error("value out of range: {0}")]
    OutOfRange(String),
}

// ── Backlight device ──────────────────────────────────────────────────────────

const BACKLIGHT_BASE: &str = "/sys/class/backlight";

pub struct Backlight {
    path: PathBuf,
    pub device: String,
}

impl Backlight {
    /// Find and open a backlight device. Uses `cfg.device` if set, else auto-detects.
    pub fn open(cfg: &Config) -> Result<Self, BrightnessError> {
        let device = if cfg.device.is_empty() {
            auto_detect_device()?
        } else {
            cfg.device.clone()
        };
        let path = PathBuf::from(BACKLIGHT_BASE).join(&device);
        if !path.exists() {
            return Err(BrightnessError::NoDevice);
        }
        info!(device = %device, "using backlight device");
        Ok(Self { path, device })
    }

    pub fn max_brightness(&self) -> Result<u64, BrightnessError> {
        read_u64(self.path.join("max_brightness"))
    }

    pub fn current_brightness(&self) -> Result<u64, BrightnessError> {
        read_u64(self.path.join("actual_brightness"))
            .or_else(|_| read_u64(self.path.join("brightness")))
    }

    pub fn set_brightness(&self, raw: u64) -> Result<(), BrightnessError> {
        std::fs::write(self.path.join("brightness"), raw.to_string())?;
        Ok(())
    }

    pub fn status(&self) -> Result<BrightnessStatus, BrightnessError> {
        let max     = self.max_brightness()?;
        let current = self.current_brightness()?;
        let percent = if max > 0 { current as f32 / max as f32 * 100.0 } else { 0.0 };
        Ok(BrightnessStatus { device: self.device.clone(), current, max, percent })
    }

    pub fn set_percent(&self, pct: f32, cfg: &Config) -> Result<BrightnessStatus, BrightnessError> {
        let pct = pct.clamp(cfg.min_percent, cfg.max_percent);
        let max = self.max_brightness()?;
        let raw = ((pct / 100.0) * max as f32).round() as u64;
        self.set_brightness(raw)?;
        self.status()
    }

    pub fn adjust_percent(&self, delta: f32, cfg: &Config) -> Result<BrightnessStatus, BrightnessError> {
        let current = self.status()?.percent;
        self.set_percent(current + delta, cfg)
    }
}

// ── Domain runner ─────────────────────────────────────────────────────────────

pub async fn run(cfg: Config, tx: broadcast::Sender<CrawlEvent>) -> anyhow::Result<()> {
    info!("crawl-brightness starting");

    let backlight = Backlight::open(&cfg)?;
    let status = backlight.status()?;

    info!(device = %status.device, percent = %status.percent, "brightness initialized");
    let _ = tx.send(CrawlEvent::Brightness(BrightnessEvent::Changed { status }));

    // The brightness domain is reactive — it doesn't poll.
    // Events are emitted when the HTTP handler calls set/inc/dec.
    // We park here so the task stays alive.
    std::future::pending::<()>().await;
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn auto_detect_device() -> Result<String, BrightnessError> {
    let base = Path::new(BACKLIGHT_BASE);
    if !base.exists() { return Err(BrightnessError::NoDevice); }

    // Prefer intel_backlight, then amdgpu_*, then anything
    let mut entries: Vec<String> = std::fs::read_dir(base)
        .map_err(|e| BrightnessError::ReadError { path: BACKLIGHT_BASE.into(), source: e })?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    entries.sort_by_key(|name| {
        if name.contains("intel")  { 0 }
        else if name.contains("amd") { 1 }
        else { 2 }
    });

    entries.into_iter().next().ok_or(BrightnessError::NoDevice)
}

fn read_u64(path: impl AsRef<Path>) -> Result<u64, BrightnessError> {
    let p = path.as_ref();
    std::fs::read_to_string(p)
        .map_err(|e| BrightnessError::ReadError { path: p.to_string_lossy().into(), source: e })
        .and_then(|s| s.trim().parse::<u64>().map_err(|_| BrightnessError::ReadError {
            path: p.to_string_lossy().into(),
            source: std::io::Error::new(std::io::ErrorKind::InvalidData, "not a number"),
        }))
}
