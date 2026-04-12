//! crawl-sysmon: System monitoring via sysinfo.
//!
//! Polls CPU, memory, and disk at a configurable interval and broadcasts
//! SysmonEvents. Also exposes synchronous query functions for the HTTP router.

use crawl_ipc::{
    events::{CrawlEvent, SysmonEvent},
    types::{CpuStatus, DiskStatus, LoadAvg, MemStatus},
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::info;

// ── Config ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Polling interval in milliseconds
    pub poll_interval_ms: u64,
    /// Publish a CpuSpike event when aggregate exceeds this percent
    pub cpu_spike_threshold: f32,
    /// Publish a MemPressure event when usage exceeds this percent
    pub mem_pressure_threshold: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            poll_interval_ms: 1000,
            cpu_spike_threshold: 90.0,
            mem_pressure_threshold: 85.0,
        }
    }
}

// ── Error ────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum SysmonError {
    #[error("failed to read system info: {0}")]
    ReadError(String),
}

// ── Domain runner ─────────────────────────────────────────────────────────────

pub async fn run(cfg: Config, tx: broadcast::Sender<CrawlEvent>) -> anyhow::Result<()> {
    info!("crawl-sysmon starting (interval={}ms)", cfg.poll_interval_ms);

    let mut sys = System::new_with_specifics(
        RefreshKind::new()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything()),
    );

    // First refresh to populate baseline
    sys.refresh_all();
    tokio::time::sleep(Duration::from_millis(200)).await;

    let interval = Duration::from_millis(cfg.poll_interval_ms);

    loop {
        sys.refresh_cpu_all();
        sys.refresh_memory();

        // CPU
        let cpu = build_cpu_status(&sys);
        let agg = cpu.aggregate;
        let _ = tx.send(CrawlEvent::Sysmon(SysmonEvent::CpuUpdate { cpu }));

        if agg > cfg.cpu_spike_threshold {
            let _ = tx.send(CrawlEvent::Sysmon(SysmonEvent::CpuSpike {
                usage: agg,
                threshold: cfg.cpu_spike_threshold,
            }));
        }

        // Memory
        let mem = build_mem_status(&sys);
        let used_pct = if mem.total_kb > 0 {
            mem.used_kb as f32 / mem.total_kb as f32 * 100.0
        } else { 0.0 };
        let _ = tx.send(CrawlEvent::Sysmon(SysmonEvent::MemUpdate { mem }));

        if used_pct > cfg.mem_pressure_threshold {
            let _ = tx.send(CrawlEvent::Sysmon(SysmonEvent::MemPressure { used_percent: used_pct }));
        }

        tokio::time::sleep(interval).await;
    }
}

// ── Builders ─────────────────────────────────────────────────────────────────

pub fn build_cpu_status(sys: &System) -> CpuStatus {
    let cpus = sys.cpus();
    let cores: Vec<f32> = cpus.iter().map(|c| c.cpu_usage()).collect();
    let freq:  Vec<u64> = cpus.iter().map(|c| c.frequency()).collect();
    let aggregate = if cores.is_empty() { 0.0 }
                    else { cores.iter().sum::<f32>() / cores.len() as f32 };

    let load = System::load_average();

    CpuStatus {
        aggregate,
        cores,
        frequency_mhz: freq,
        load_avg: LoadAvg {
            one:     load.one,
            five:    load.five,
            fifteen: load.fifteen,
        },
        temperature_c: None, // TODO: sysinfo Component API
    }
}

pub fn build_mem_status(sys: &System) -> MemStatus {
    MemStatus {
        total_kb:      sys.total_memory() / 1024,
        used_kb:       sys.used_memory()  / 1024,
        available_kb:  sys.available_memory() / 1024,
        swap_total_kb: sys.total_swap() / 1024,
        swap_used_kb:  sys.used_swap()  / 1024,
    }
}

pub fn build_disk_status() -> Vec<DiskStatus> {
    use sysinfo::Disks;
    let disks = Disks::new_with_refreshed_list();
    disks.iter().map(|d| DiskStatus {
        mount:       d.mount_point().to_string_lossy().to_string(),
        total_bytes: d.total_space(),
        used_bytes:  d.total_space().saturating_sub(d.available_space()),
        available_bytes: d.available_space(),
        filesystem:  Some(d.file_system().to_string_lossy().to_string()),
    }).collect()
}

// ── Public query API ──────────────────────────────────────────────────────────

pub fn get_cpu() -> CpuStatus {
    let mut sys = System::new_with_specifics(
        RefreshKind::new().with_cpu(CpuRefreshKind::everything()),
    );
    sys.refresh_cpu_all();
    build_cpu_status(&sys)
}

pub fn get_mem() -> MemStatus {
    let mut sys = System::new_with_specifics(
        RefreshKind::new().with_memory(MemoryRefreshKind::everything()),
    );
    sys.refresh_memory();
    build_mem_status(&sys)
}

pub fn get_disks() -> Vec<DiskStatus> {
    build_disk_status()
}
