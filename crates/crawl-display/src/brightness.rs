//! Brightness control via sysfs backlight.
//!
//! Requires write permission on brightness sysfs node.
//! On Arch: add user to `video` group and add a udev rule (see README).

use crawl_ipc::{
    events::{BrightnessEvent, CrawlEvent},
    types::BrightnessStatus,
};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::info;

pub use crate::config::DisplayConfig as Config;

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
    config: Config,
}

impl Backlight {
    /// Find and open a backlight device. Uses `cfg.device` if set, else auto-detects.
    pub fn open(cfg: &Config) -> Result<Self, BrightnessError> {
        let device = if cfg.brightness_device.is_empty() {
            auto_detect_device()?
        } else {
            cfg.brightness_device.clone()
        };
        let path = PathBuf::from(BACKLIGHT_BASE).join(&device);
        if !path.exists() {
            return Err(BrightnessError::NoDevice);
        }
        info!(device = %device, "using backlight device");
        Ok(Self { path, device, config: cfg.clone() })
    }

    pub fn max_brightness(&self) -> Result<u64, BrightnessError> {
        read_u64(self.path.join("max_brightness"))
    }

    pub fn current_brightness(&self) -> Result<u64, BrightnessError> {
        read_u64(self.path.join("brightness"))
            .or_else(|_| read_u64(self.path.join("actual_brightness")))
    }

    pub fn set_brightness(&self, raw: u64) -> Result<(), BrightnessError> {
        std::fs::write(self.path.join("brightness"), raw.to_string())?;
        Ok(())
    }

    pub fn status(&self) -> Result<BrightnessStatus, BrightnessError> {
        let max     = self.max_brightness()?;
        let current = self.current_brightness()?;
        let current = current.min(max);
        let percent = if max > 0 { current as f32 / max as f32 * 100.0 } else { 0.0 };
        Ok(BrightnessStatus { device: self.device.clone(), current, max, percent })
    }

    pub fn set_percent(&self, pct: f32) -> Result<BrightnessStatus, BrightnessError> {
        if !pct.is_finite() {
            return Err(BrightnessError::OutOfRange("brightness percent must be a finite number".into()));
        }

        let (min_pct, max_pct) = sanitize_percent_bounds(self.config.brightness_min, self.config.brightness_max);
        let pct = pct.clamp(min_pct, max_pct);
        let max = self.max_brightness()?;
        let min_raw = ((min_pct / 100.0) * max as f32).ceil() as u64;
        let max_raw = ((max_pct / 100.0) * max as f32).floor() as u64;

        let mut raw = ((pct / 100.0) * max as f32).round() as u64;
        if max > 0 {
            let lower = min_raw.min(max);
            let upper = max_raw.min(max).max(lower);
            raw = raw.clamp(lower, upper);
        }

        self.set_brightness(raw)?;
        self.status()
    }

    pub fn adjust_percent(&self, delta: f32) -> Result<BrightnessStatus, BrightnessError> {
        let current = self.status()?.percent;
        self.set_percent(current + delta)
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

fn sanitize_percent_bounds(min_percent: f32, max_percent: f32) -> (f32, f32) {
    let min = if min_percent.is_finite() { min_percent } else { 0.0 };
    let max = if max_percent.is_finite() { max_percent } else { 100.0 };
    let min = min.clamp(0.0, 100.0);
    let max = max.clamp(0.0, 100.0);
    if min <= max { (min, max) } else { (max, min) }
}
