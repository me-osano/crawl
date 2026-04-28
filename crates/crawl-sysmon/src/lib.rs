//! crawl-sysmon: System monitoring via sysinfo.
//!
//! Polls CPU, memory, and disk at a configurable interval and broadcasts
//! SysmonEvents. Also exposes synchronous query functions for the HTTP router.

pub mod config;

pub use config::Config;
use crawl_ipc::{
    events::{CrawlEvent, SysmonEvent},
    types::{CpuStatus, DiskStatus, LoadAvg, MemStatus, NetTraffic, GpuStatus},
};
use crawl_scheduler::{Scheduler, ShouldRun};
use std::time::Duration;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::info;
use std::collections::HashMap;
use std::fs;

// ── Error ────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum SysmonError {
    #[error("failed to read system info: {0}")]
    ReadError(String),
}

// ── State for change detection ──────────────────────────────────────────────

#[derive(Clone, Default)]
struct SysmonState {
    last_cpu: Option<CpuStatus>,
    last_mem: Option<MemStatus>,
    last_net: Option<NetTraffic>,
    last_gpu: Option<GpuStatus>,
    cpu_spike_sent: bool,
    mem_pressure_sent: bool,
}

impl SysmonState {
    fn cpu_changed(&self, new: &CpuStatus, threshold: f32) -> bool {
        match &self.last_cpu {
            Some(old) => (new.aggregate - old.aggregate).abs() > threshold,
            None => true,
        }
    }

    fn mem_changed(&self, new: &MemStatus, threshold: f32) -> bool {
        let old_pct = self.last_mem.as_ref().map(|m| {
            if m.total_kb > 0 { m.used_kb as f32 / m.total_kb as f32 * 100.0 } else { 0.0 }
        }).unwrap_or(0.0);
        let new_pct = if new.total_kb > 0 {
            new.used_kb as f32 / new.total_kb as f32 * 100.0
        } else { 0.0 };
        (new_pct - old_pct).abs() > threshold
    }

    fn net_changed(&self, new: &NetTraffic, threshold: u64) -> bool {
        match &self.last_net {
            Some(old) => {
                (new.rx_bytes.saturating_sub(old.rx_bytes)) > threshold
                    || (new.tx_bytes.saturating_sub(old.tx_bytes)) > threshold
                    || (new.rx_bps.saturating_sub(old.rx_bps)) > threshold
                    || (new.tx_bps.saturating_sub(old.tx_bps)) > threshold
            }
            None => true,
        }
    }

    fn gpu_changed(&self, new: &Option<GpuStatus>) -> bool {
        match (&self.last_gpu, new) {
            (Some(old), Some(new)) => {
                old.name != new.name || old.temperature_c != new.temperature_c
            }
            (None, Some(_)) | (Some(_), None) | (None, None) => {
                new.is_some() || self.last_gpu.is_some()
            }
        }
    }
}

// ── Domain runner ─────────────────────────────────────────────────────────────

pub async fn run(cfg: Config, tx: broadcast::Sender<CrawlEvent>) -> anyhow::Result<()> {
    run_with_scheduler(cfg, tx, None).await
}

pub async fn run_with_scheduler(
    cfg: Config,
    tx: broadcast::Sender<CrawlEvent>,
    scheduler: Option<Scheduler>,
) -> anyhow::Result<()> {
    let task_id = "sysmon";
    if let Some(ref sched) = scheduler {
        sched.register(task_id, Duration::from_secs(1), 0.05).await;
    }

    info!("crawl-sysmon starting (interval={}ms, scheduler={})",
        cfg.poll_interval_ms, scheduler.is_some());

    let mut sys = System::new_with_specifics(
        RefreshKind::new()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything()),
    );

    let mut net_state = NetState::new();
    let mut state = SysmonState::default();

    sys.refresh_all();
    tokio::time::sleep(Duration::from_millis(200)).await;

    let poll_interval = Duration::from_millis(cfg.poll_interval_ms);

    loop {
        if let Some(ref sched) = scheduler {
            sched.wait_interval(task_id, 500).await;
            if sched.should_run(task_id).await == ShouldRun::Yes {
                sched.mark_ran(task_id).await;
            } else {
                continue;
            }
        } else {
            tokio::time::sleep(poll_interval).await;
        }

        sys.refresh_cpu_all();
        sys.refresh_memory();

        let cpu = build_cpu_status(&sys);
        let mem = build_mem_status(&sys);
        let traffic = net_state.sample();
        let gpu = read_gpu_status();

        let used_pct = if mem.total_kb > 0 {
            mem.used_kb as f32 / mem.total_kb as f32 * 100.0
        } else { 0.0 };

        if state.cpu_changed(&cpu, cfg.cpu_change_threshold) {
            let _ = tx.send(CrawlEvent::Sysmon(SysmonEvent::CpuUpdate { cpu: cpu.clone() }));
            state.last_cpu = Some(cpu.clone());
        }

        if cpu.aggregate > cfg.cpu_spike_threshold && !state.cpu_spike_sent {
            let _ = tx.send(CrawlEvent::Sysmon(SysmonEvent::CpuSpike {
                usage: cpu.aggregate,
                threshold: cfg.cpu_spike_threshold,
            }));
            state.cpu_spike_sent = true;
        } else if cpu.aggregate < cfg.cpu_spike_threshold - 10.0 {
            state.cpu_spike_sent = false;
        }

        if state.mem_changed(&mem, cfg.mem_change_threshold) {
            let _ = tx.send(CrawlEvent::Sysmon(SysmonEvent::MemUpdate { mem: mem.clone() }));
            state.last_mem = Some(mem);
        }

        if used_pct > cfg.mem_pressure_threshold && !state.mem_pressure_sent {
            let _ = tx.send(CrawlEvent::Sysmon(SysmonEvent::MemPressure { used_percent: used_pct }));
            state.mem_pressure_sent = true;
        } else if used_pct < cfg.mem_pressure_threshold - 10.0 {
            state.mem_pressure_sent = false;
        }

        if state.net_changed(&traffic, cfg.net_change_threshold) {
            let _ = tx.send(CrawlEvent::Sysmon(SysmonEvent::NetUpdate { traffic: traffic.clone() }));
            state.last_net = Some(traffic);
        }

        if state.gpu_changed(&gpu) {
            if let Some(ref gpu) = gpu {
                let _ = tx.send(CrawlEvent::Sysmon(SysmonEvent::GpuUpdate { gpu: gpu.clone() }));
            }
            state.last_gpu = gpu;
        }
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

pub fn get_net() -> NetTraffic {
    let mut net_state = NetState::new();
    net_state.sample()
}

pub fn get_gpu() -> Option<GpuStatus> {
    read_gpu_status()
}

struct NetState {
    prev: HashMap<String, (u64, u64)>,
    last_sample_ms: u128,
}

impl NetState {
    fn new() -> Self {
        Self { prev: HashMap::new(), last_sample_ms: now_ms() }
    }

    fn sample(&mut self) -> NetTraffic {
        let now = now_ms();
        let elapsed_ms = (now - self.last_sample_ms).max(1);
        self.last_sample_ms = now;

        let current = read_net_bytes();
        let mut rx_total: u64 = 0;
        let mut tx_total: u64 = 0;
        let mut rx_bps: u64 = 0;
        let mut tx_bps: u64 = 0;

        for (iface, (rx, tx)) in current.iter() {
            rx_total = rx_total.saturating_add(*rx);
            tx_total = tx_total.saturating_add(*tx);
            if let Some((prev_rx, prev_tx)) = self.prev.get(iface) {
                rx_bps = rx_bps.saturating_add((rx.saturating_sub(*prev_rx)) * 1000 / elapsed_ms as u64);
                tx_bps = tx_bps.saturating_add((tx.saturating_sub(*prev_tx)) * 1000 / elapsed_ms as u64);
            }
        }

        self.prev = current;
        NetTraffic { rx_bytes: rx_total, tx_bytes: tx_total, rx_bps, tx_bps }
    }
}

fn read_net_bytes() -> HashMap<String, (u64, u64)> {
    let mut map = HashMap::new();
    if let Ok(content) = fs::read_to_string("/proc/net/dev") {
        for line in content.lines().skip(2) {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() != 2 { continue; }
            let iface = parts[0].trim().to_string();
            if iface == "lo" { continue; }
            let data: Vec<&str> = parts[1].split_whitespace().collect();
            if data.len() < 9 { continue; }
            let rx = data[0].parse::<u64>().unwrap_or(0);
            let tx = data[8].parse::<u64>().unwrap_or(0);
            map.insert(iface, (rx, tx));
        }
    }
    map
}

fn read_gpu_status() -> Option<GpuStatus> {
    // Best effort: try DRM card hwmon paths for temp, and /sys/class/drm/card*/device/uevent for name.
    let mut name: Option<String> = None;
    let mut temp_c: Option<f32> = None;

    if let Ok(entries) = fs::read_dir("/sys/class/drm") {
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = path.file_name()?.to_string_lossy().to_string();
            if !file_name.starts_with("card") || file_name.contains("-") { continue; }

            let uevent = path.join("device/uevent");
            if name.is_none() {
                if let Ok(content) = fs::read_to_string(uevent) {
                    for line in content.lines() {
                        if let Some(val) = line.strip_prefix("DRIVER=") {
                            name = Some(val.to_string());
                            break;
                        }
                    }
                }
            }

            if temp_c.is_none() {
                let hwmon_dir = path.join("device/hwmon");
                if let Ok(hwmons) = fs::read_dir(hwmon_dir) {
                    for h in hwmons.flatten() {
                        let temp_path = h.path().join("temp1_input");
                        if let Ok(raw) = fs::read_to_string(temp_path) {
                            if let Ok(milli) = raw.trim().parse::<f32>() {
                                temp_c = Some(milli / 1000.0);
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    if name.is_none() && temp_c.is_none() {
        return None;
    }
    Some(GpuStatus { name, temperature_c: temp_c })
}

fn now_ms() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}
