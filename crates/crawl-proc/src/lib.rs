//! crawl-proc: Process listing and management via sysinfo.
//!
//! Exposes process enumeration, search, kill, and PID watching.
//! Uses caching and incremental updates for efficiency.

pub mod config;

pub use config::Config;
use crawl_ipc::events::{CrawlEvent, ProcEvent};
use crawl_ipc::types::ProcessInfo;
use std::collections::HashMap;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, Signal, System};
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::{debug, info};

use crate::cache::ProcessCache;

// ── Error ───────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ProcError {
    #[error("process not found: PID {0}")]
    NotFound(u32),
    #[error("permission denied killing PID {0}")]
    PermissionDenied(u32),
    #[error("signal failed: {0}")]
    SignalFailed(String),
}

// ── Domain runner ─────────────────────────────────────────────────────────────

pub async fn run(cfg: Config, tx: broadcast::Sender<CrawlEvent>) -> anyhow::Result<()> {
    info!(
        "crawl-proc starting (top: {}ms, full: {}ms, include_cmd: {})",
        cfg.top_interval_ms, cfg.full_interval_ms, cfg.include_cmd
    );

    let mut cache = ProcessCache::new(cfg.include_cmd);
    let top_interval = std::time::Duration::from_millis(cfg.top_interval_ms);
    let full_interval = std::time::Duration::from_millis(cfg.full_interval_ms);

    let mut last_full = tokio::time::Instant::now();
    let mut last_top = tokio::time::Instant::now();

    loop {
        let now = tokio::time::Instant::now();

        // Full scan every full_interval
        if now.duration_since(last_full) >= full_interval {
            cache.full_refresh();
            last_full = now;
            debug!("full process scan done, {} processes", cache.len());
        }

        // Top-N update every top_interval
        if now.duration_since(last_top) >= top_interval {
            cache.refresh_top();
            last_top = now;

            let top_cpu = cache.top_by_cpu(10);
            let top_mem = cache.top_by_mem(10);

            if !top_cpu.is_empty() || !top_mem.is_empty() {
                let _ = tx.send(CrawlEvent::Proc(ProcEvent::TopUpdate {
                    top_by_cpu: top_cpu,
                    top_by_mem: top_mem,
                }));
            }
        }

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

// ── Public query API ──────────────────────────────────────────────────────────────────

/// List processes sorted by specified field. Uses cached data when available.
pub fn list_processes(sort_by: &str, top: usize) -> Vec<ProcessInfo> {
    let cache = PROCESS_CACHE.lock().unwrap();
    cache.list(sort_by, top)
}

/// Find processes by name. Uses cached data.
pub fn find_processes(name: &str) -> Vec<ProcessInfo> {
    let cache = PROCESS_CACHE.lock().unwrap();
    cache.find(name)
}

/// Force a full refresh and get list.
pub fn list_processes_fresh(sort_by: &str, top: usize) -> Vec<ProcessInfo> {
    let mut cache = PROCESS_CACHE.lock().unwrap();
    cache.full_refresh();
    cache.list(sort_by, top)
}

/// Kill a process by PID.
pub fn kill_process(pid: u32, force: bool) -> Result<(), ProcError> {
    let mut sys = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
    );
    sys.refresh_processes(ProcessesToUpdate::All);

    let sysinfo_pid = sysinfo::Pid::from_u32(pid);
    let process = sys.process(sysinfo_pid).ok_or(ProcError::NotFound(pid))?;

    let signal = if force { Signal::Kill } else { Signal::Term };
    process.kill_with(signal).ok_or_else(|| ProcError::SignalFailed(format!("kill({pid}) failed")))?;
    Ok(())
}

/// Watch a PID and return when it exits. Polls every 500ms.
pub async fn watch_pid(pid: u32) -> Result<String, ProcError> {
    let mut sys = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
    );
    sys.refresh_processes(ProcessesToUpdate::All);
    let name = match sys.process(sysinfo::Pid::from_u32(pid)) {
        Some(proc_) => proc_.name().to_string_lossy().to_string(),
        None => {
            return Err(ProcError::NotFound(pid));
        }
    };

    if name.is_empty() {
        return Err(ProcError::NotFound(pid));
    }

    loop {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        sys.refresh_processes(ProcessesToUpdate::All);
        if sys.process(sysinfo::Pid::from_u32(pid)).is_none() {
            break;
        }
    }

    Ok(name)
}

// ── Global process cache ──────────────────────────────────────────────────────

use once_cell::sync::Lazy;
use std::sync::Mutex;

static PROCESS_CACHE: Lazy<Mutex<ProcessCache>> = Lazy::new(|| Mutex::new(ProcessCache::new(false)));

mod cache {
    use super::*;
    use std::collections::VecDeque;

    /// Process cache with incremental updates and top-N tracking.
    pub struct ProcessCache {
        by_pid: HashMap<u32, ProcessInfo>,
        sorted_by_cpu: VecDeque<ProcessInfo>,
        sorted_by_mem: VecDeque<ProcessInfo>,
        include_cmd: bool,
    }

    impl ProcessCache {
        pub fn new(include_cmd: bool) -> Self {
            Self {
                by_pid: HashMap::new(),
                sorted_by_cpu: VecDeque::new(),
                sorted_by_mem: VecDeque::new(),
                include_cmd,
            }
        }

        /// Number of cached processes.
        pub fn len(&self) -> usize {
            self.by_pid.len()
        }

        /// Full refresh - rescans all processes.
        pub fn full_refresh(&mut self) {
            let mut sys = System::new_with_specifics(
                RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
            );
            sys.refresh_processes(ProcessesToUpdate::All);

            let mut new_by_pid = HashMap::with_capacity(sys.processes().len());
            let mut new_sorted_cpu = Vec::new();
            let mut new_sorted_mem = Vec::new();

            for (pid, p) in sys.processes() {
                let pid_u32 = pid.as_u32();
                let info = ProcessInfo {
                    pid: pid_u32,
                    ppid: p.parent().map(|par| par.as_u32()),
                    name: p.name().to_string_lossy().to_string(),
                    exe_path: p.exe().map(|e| e.to_string_lossy().to_string()),
                    cpu_percent: p.cpu_usage(),
                    cpu_ticks: None,
                    mem_rss_kb: p.memory() / 1024,
                    status: format!("{:?}", p.status()),
                    user: None,
                    cmd: if self.include_cmd {
                        p.cmd().iter().map(|s| s.to_string_lossy().to_string()).collect()
                    } else {
                        vec![]
                    },
                };
                new_by_pid.insert(pid_u32, info.clone());
                new_sorted_cpu.push(info.clone());
                new_sorted_mem.push(info);
            }

            // Sort top 100 by CPU
            sort_and_truncate(&mut new_sorted_cpu, "cpu", 100);
            sort_and_truncate(&mut new_sorted_mem, "mem", 100);

            self.by_pid = new_by_pid;
            self.sorted_by_cpu = VecDeque::from(new_sorted_cpu);
            self.sorted_by_mem = VecDeque::from(new_sorted_mem);
        }

        /// Refresh top-N only (incremental, cheaper).
        pub fn refresh_top(&mut self) {
            let mut sys = System::new_with_specifics(
                RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
            );
            sys.refresh_processes(ProcessesToUpdate::All);

            // Update cached values for existing PIDs
            for (pid, p) in sys.processes() {
                let pid_u32 = pid.as_u32();
                if let Some(info) = self.by_pid.get_mut(&pid_u32) {
                    info.cpu_percent = p.cpu_usage();
                    info.mem_rss_kb = p.memory() / 1024;
                    info.status = format!("{:?}", p.status());
                }
            }

            // Remove exited processes
            self.by_pid.retain(|pid, _| {
                sys.process(Pid::from_u32(*pid)).is_some()
            });

            // Re-sort top 100
            let mut all_cpu: Vec<_> = self.by_pid.values().cloned().collect();
            let mut all_mem = all_cpu.clone();

            sort_and_truncate(&mut all_cpu, "cpu", 100);
            sort_and_truncate(&mut all_mem, "mem", 100);

            self.sorted_by_cpu = VecDeque::from(all_cpu);
            self.sorted_by_mem = VecDeque::from(all_mem);
        }

        /// Get processes sorted by field, limited to top n.
        pub fn list(&self, sort_by: &str, top: usize) -> Vec<ProcessInfo> {
            match sort_by {
                "mem" => self.sorted_by_mem.iter().take(top).cloned().collect(),
                _ => self.sorted_by_cpu.iter().take(top).cloned().collect(),
            }
        }

        /// Find processes by name.
        pub fn find(&self, name: &str) -> Vec<ProcessInfo> {
            let name_lower = name.to_lowercase();
            self.by_pid
                .values()
                .filter(|p| p.name.to_lowercase().contains(&name_lower))
                .cloned()
                .collect()
        }

        /// Top processes by CPU usage.
        pub fn top_by_cpu(&self, n: usize) -> Vec<ProcessInfo> {
            self.sorted_by_cpu.iter().take(n).cloned().collect()
        }

        /// Top processes by memory usage.
        pub fn top_by_mem(&self, n: usize) -> Vec<ProcessInfo> {
            self.sorted_by_mem.iter().take(n).cloned().collect()
        }
    }

    fn sort_and_truncate(v: &mut Vec<ProcessInfo>, sort_by: &str, truncate_at: usize) {
        match sort_by {
            "mem" => v.sort_by(|a, b| b.mem_rss_kb.cmp(&a.mem_rss_kb)),
            "pid" => v.sort_by_key(|p| p.pid),
            "name" => v.sort_by(|a, b| a.name.cmp(&b.name)),
            _ => v.sort_by(|a, b| {
                b.cpu_percent
                    .partial_cmp(&a.cpu_percent)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
        }
        v.truncate(truncate_at);
    }
}